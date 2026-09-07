//! 조회 기간 계산. **DB를 모르는 순수 계산 계층.**
//!
//! 오늘 / 이번 주 / 이번 달 / 이번 학기 / 사용자 지정을 실제 날짜 범위로 바꾼다.
//! 통계는 이 범위로만 원본 자료를 다시 세므로, 여기가 맞으면 숫자도 맞는다.

use chrono::{Datelike, Duration, NaiveDate};

pub const TODAY: &str = "TODAY";
pub const WEEK: &str = "WEEK";
pub const MONTH: &str = "MONTH";
pub const TERM: &str = "TERM";
pub const CUSTOM: &str = "CUSTOM";

/// 학기 정보. `start`/`end`가 비어 있으면 학년도·학기로 계산한다.
#[derive(Debug, Clone)]
pub struct TermInfo {
    pub school_year: i32,
    pub semester: i32,
    pub name: String,
    pub start: Option<String>,
    pub end: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Range {
    /// YYYY-MM-DD
    pub from: String,
    pub to: String,
    /// 화면에 그대로 쓰는 문구 — '2026년 9월' 처럼
    pub label: String,
}

fn ymd(d: NaiveDate) -> String {
    d.format("%Y-%m-%d").to_string()
}

fn parse(s: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d").ok()
}

/// 그 달의 마지막 날.
fn last_of_month(y: i32, m: u32) -> NaiveDate {
    let (ny, nm) = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
    NaiveDate::from_ymd_opt(ny, nm, 1).unwrap() - Duration::days(1)
}

/// 학기 범위. 학교가 날짜를 넣어 두었으면 그것을 쓰고, 없으면 우리나라
/// 학사 일정으로 계산한다 — 1학기 3/1~8/31, 2학기 9/1~다음해 2월 말.
pub fn term_range(t: &TermInfo) -> (NaiveDate, NaiveDate) {
    let from = t.start.as_deref().and_then(parse);
    let to = t.end.as_deref().and_then(parse);
    if let (Some(a), Some(b)) = (from, to) {
        if b >= a {
            return (a, b);
        }
    }
    let y = t.school_year;
    if t.semester == 1 {
        (
            NaiveDate::from_ymd_opt(y, 3, 1).unwrap(),
            NaiveDate::from_ymd_opt(y, 8, 31).unwrap(),
        )
    } else {
        (
            NaiveDate::from_ymd_opt(y, 9, 1).unwrap(),
            last_of_month(y + 1, 2),
        )
    }
}

/// 고른 기간을 실제 날짜 범위로 바꾼다.
///
/// `custom`이 잘못되었거나 비어 있으면 오늘 하루로 되돌린다 —
/// 화면이 빈 채로 멈추는 것보다 낫다.
pub fn resolve(
    preset: &str,
    today: NaiveDate,
    term: Option<&TermInfo>,
    custom: (Option<&str>, Option<&str>),
) -> Range {
    match preset {
        WEEK => {
            // 월요일부터 일요일까지
            let back = today.weekday().num_days_from_monday() as i64;
            let from = today - Duration::days(back);
            let to = from + Duration::days(6);
            Range {
                from: ymd(from),
                to: ymd(to),
                label: format!(
                    "{}월 {}일 ~ {}월 {}일 (이번 주)",
                    from.month(),
                    from.day(),
                    to.month(),
                    to.day()
                ),
            }
        }
        MONTH => {
            let from = NaiveDate::from_ymd_opt(today.year(), today.month(), 1).unwrap();
            let to = last_of_month(today.year(), today.month());
            Range {
                from: ymd(from),
                to: ymd(to),
                label: format!("{}년 {}월", today.year(), today.month()),
            }
        }
        TERM => match term {
            Some(t) => {
                let (from, to) = term_range(t);
                Range {
                    from: ymd(from),
                    to: ymd(to),
                    label: t.name.clone(),
                }
            }
            None => resolve(MONTH, today, None, (None, None)),
        },
        CUSTOM => {
            let a = custom.0.and_then(parse);
            let b = custom.1.and_then(parse);
            match (a, b) {
                (Some(a), Some(b)) if b >= a => Range {
                    from: ymd(a),
                    to: ymd(b),
                    label: format!("{} ~ {}", ymd(a), ymd(b)),
                },
                // 하나만 넣었으면 그 하루로 본다
                (Some(a), None) | (None, Some(a)) => Range {
                    from: ymd(a),
                    to: ymd(a),
                    label: ymd(a),
                },
                // 거꾸로 넣었으면 바로잡는다
                (Some(a), Some(b)) => Range {
                    from: ymd(b),
                    to: ymd(a),
                    label: format!("{} ~ {}", ymd(b), ymd(a)),
                },
                _ => resolve(TODAY, today, term, (None, None)),
            }
        }
        // TODAY 및 알 수 없는 값
        _ => Range {
            from: ymd(today),
            to: ymd(today),
            label: format!("{}년 {}월 {}일 (오늘)", today.year(), today.month(), today.day()),
        },
    }
}

/// 'YYYY-MM' 을 그 달의 범위로 바꾼다.
///
/// `resolve(MONTH, ...)` 는 **오늘이 든 달**만 낼 수 있다. 보결 수당은 지난
/// 달을 뒤로 넘겨 보는 일이 잦으므로, 달을 직접 지정하는 길을 따로 둔다.
/// 잘못된 값이면 `None` — 부르는 쪽이 이번 달로 되돌린다.
pub fn month_of(ym: &str) -> Option<Range> {
    let t = ym.trim();
    let (y, m) = t.split_once('-')?;
    let y: i32 = y.trim().parse().ok()?;
    let m: u32 = m.trim().parse().ok()?;
    if !(1..=12).contains(&m) || !(1900..=9999).contains(&y) {
        return None;
    }
    let from = NaiveDate::from_ymd_opt(y, m, 1)?;
    let to = last_of_month(y, m);
    Some(Range {
        from: ymd(from),
        to: ymd(to),
        label: format!("{y}년 {m}월"),
    })
}

/// 그 달을 'YYYY-MM' 으로.
pub fn ym_of(d: NaiveDate) -> String {
    format!("{}-{:02}", d.year(), d.month())
}

/// 'YYYY-MM' 에서 달을 옮긴다. 화면의 이전/다음 달 버튼이 쓴다.
pub fn shift_month(ym: &str, by: i32) -> Option<String> {
    let r = month_of(ym)?;
    let first = parse(&r.from)?;
    let total = first.year() * 12 + (first.month0() as i32) + by;
    let (y, m0) = (total.div_euclid(12), total.rem_euclid(12));
    Some(format!("{}-{:02}", y, m0 + 1))
}

/// 범위 안의 날짜를 모두 만든다. 너무 길면 잘라 낸다 (화면 보호).
pub fn dates_in(range: &Range, max: usize) -> Vec<String> {
    let (Some(a), Some(b)) = (parse(&range.from), parse(&range.to)) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut d = a;
    while d <= b && out.len() < max {
        out.push(ymd(d));
        d += Duration::days(1);
    }
    out
}
