//! 보결 수당 — **남아 있는 배정 기록을 그때그때 다시 센다.**
//!
//! ## 계산 근거는 `substitutions` 하나뿐이다
//!
//! - **보결한 횟수** = `sub_teacher_id` 가 그 사람인 `ASSIGNED` 건수
//! - **본인 발생 보결** = `absent_teacher_id` 가 그 사람인 `ASSIGNED` 건수
//!
//! 두 숫자 모두 `absences` 를 세지 않는다. 그래서 **결근으로 수업이 비었지만
//! 아무도 배정되지 않은 시간은 차감에 들어가지 않는다** — 다른 사람이 실제로
//! 들어간 건만 기록으로 남기 때문이다. 취소분(`CANCELLED`)은 어느 쪽에도
//! 들어가지 않으므로, 취소 후 다시 배정한 건은 지금 유효한 배정만 세어진다.
//!
//! ## 수당 값을 저장하지 않는다
//!
//! 계산 결과를 표로 만들어 두지 않는다. 1회 수당을 15,000원에서 20,000원으로
//! 바꾸면 다음 조회에서 곧바로 20,000원으로 나온다. 지급 확정·마감 스냅샷은
//! 아직 만들지 않는다. 나중에 필요해지면 이 계산을 그대로 두고 **결과를 떠
//! 담는 기능**을 따로 얹을 수 있도록, 여기서는 조회만 한다.
//!
//! ## 배정 엔진에 끼어들지 않는다
//!
//! 여기는 후처리다. 후보 판정(`domain::find`)이나 배정(`repo::assign`)은
//! 수당을 모른다. `보결 배정 → 기록 생성 → 여기서 집계` 한 방향으로만 흐른다.

use std::collections::HashMap;

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use super::xlsx::{col, Cell, Sheet};
use super::{school, settings};
use crate::domain::pay::{self, Case, Tally};
use crate::domain::period::{self, Range, TermInfo};
use crate::error::AppResult;

pub const MODE_MONTH: &str = "MONTH";
pub const MODE_CUSTOM: &str = "CUSTOM";

// ============================================================
//  요청
// ============================================================

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PayQuery {
    /// MONTH | CUSTOM. 기본은 월별.
    #[serde(default)]
    pub mode: Option<String>,
    /// mode=MONTH 일 때 'YYYY-MM'. 없으면 이번 달.
    #[serde(default)]
    pub month: Option<String>,
    /// mode=CUSTOM 일 때
    #[serde(default)]
    pub from: Option<String>,
    #[serde(default)]
    pub to: Option<String>,
}

// ============================================================
//  결과
// ============================================================

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PayRow {
    pub teacher_id: i64,
    pub name: String,
    pub role_code: String,
    pub role_label: String,
    /// 담임이면 학급, 전담이면 과목, 기타면 담당 업무
    pub duty: String,
    pub active: bool,
    /// 다른 선생님을 대신해 들어간 횟수
    pub substituted: i32,
    /// 본인 결근으로 다른 선생님이 들어간 횟수 (참고정보)
    pub own_caused: i32,
    pub payable: i32,
    pub per_case: i64,
    pub amount: i64,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PaySummary {
    /// 지급액이 남는 선생님 수
    pub paid_teachers: i32,
    /// 표에 오른 선생님 수 (보결 기록이 하나라도 있는 사람)
    pub listed_teachers: i32,
    pub total_substituted: i32,
    pub total_own_caused: i32,
    pub total_payable: i32,
    pub total_amount: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PayView {
    /// MONTH | CUSTOM
    pub mode: String,
    /// 'YYYY-MM' — 이전/다음 달 버튼이 쓴다
    pub month: String,
    pub prev_month: String,
    pub next_month: String,
    pub from: String,
    pub to: String,
    pub range_label: String,

    pub policy: String,
    pub policy_label: String,
    pub policy_hint: String,
    pub per_case: i64,
    /// 1회 수당을 아직 정하지 않았다 — 화면에서 안내한다
    pub per_case_unset: bool,

    pub summary: PaySummary,
    pub rows: Vec<PayRow>,

    /// 고른 기간이 지금 학기 밖으로 나갔을 때의 안내. 막지는 않는다.
    pub term_note: Option<String>,
}

/// 상세에 보여 줄 보결 한 건.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaseRow {
    pub date: String,
    pub day_of_week: i32,
    /// 기록 당시 표기 그대로 (`substitutions.class_label` 스냅샷)
    pub class_label: String,
    pub slot_label: String,
    pub start_min: i32,
    pub end_min: i32,
    /// 보결한 내역이면 결근한 선생님, 본인 발생분이면 대신 들어간 선생님
    pub counterpart: String,
    pub reason_label: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PayDetail {
    pub teacher_id: i64,
    pub name: String,
    pub role_label: String,
    pub from: String,
    pub to: String,
    pub range_label: String,

    pub policy: String,
    pub policy_label: String,
    pub per_case: i64,

    /// 이 선생님이 대신 들어간 내역
    pub substituted: Vec<CaseRow>,
    /// 이 선생님의 결근으로 다른 선생님이 들어간 내역
    pub own_caused: Vec<CaseRow>,

    pub payable: i32,
    pub amount: i64,
    /// 계산 설명. 정책이 만든 문장을 그대로 보여 준다.
    pub steps: Vec<String>,
    /// 차감 정책이 아니면 '본인 발생분은 참고정보'라고 알려 준다
    pub deducts: bool,
}

// ============================================================
//  기간
// ============================================================

fn today_local() -> chrono::NaiveDate {
    chrono::Local::now().date_naive()
}

fn current_term(conn: &Connection) -> AppResult<Option<TermInfo>> {
    let t = conn
        .query_row(
            "SELECT school_year, semester, name, start_date, end_date
               FROM terms WHERE is_current = 1 LIMIT 1",
            [],
            |r| {
                Ok(TermInfo {
                    school_year: r.get(0)?,
                    semester: r.get(1)?,
                    name: r.get(2)?,
                    start: r.get(3)?,
                    end: r.get(4)?,
                })
            },
        )
        .optional()?;
    Ok(t)
}

/// 고른 기간을 실제 날짜 범위로 바꾼다.
///
/// 월별이 기본이다. 잘못된 달을 받으면 이번 달로 되돌린다 — 화면이 빈 채로
/// 멈추는 것보다 낫다 (`domain::period` 의 방침과 같다).
pub fn resolve(q: &PayQuery) -> (String, String, Range) {
    let mode = q.mode.as_deref().unwrap_or(MODE_MONTH);
    if mode == MODE_CUSTOM {
        let range = period::resolve(
            period::CUSTOM,
            today_local(),
            None,
            (q.from.as_deref(), q.to.as_deref()),
        );
        // 사용자 지정이어도 '달 옮기기'가 기준으로 삼을 달은 있어야 한다
        let month = range.from.get(0..7).unwrap_or_default().to_string();
        return (MODE_CUSTOM.to_string(), month, range);
    }

    let month = q
        .month
        .as_deref()
        .and_then(|m| period::month_of(m).map(|_| m.trim().to_string()))
        .unwrap_or_else(|| period::ym_of(today_local()));
    let range = period::month_of(&month).expect("위에서 확인한 값");
    (MODE_MONTH.to_string(), month, range)
}

/// 고른 기간이 지금 학기를 벗어나는지 알려 준다. **막지 않는다** —
/// 학기가 바뀐 뒤 지난 달 수당을 정산하는 일이 실제로 있다.
fn term_note(term: Option<&TermInfo>, range: &Range) -> Option<String> {
    let t = term?;
    let (ts, te) = period::term_range(t);
    let (ts, te) = (ts.format("%Y-%m-%d").to_string(), te.format("%Y-%m-%d").to_string());
    if range.from >= ts && range.to <= te {
        return None;
    }
    Some(format!(
        "고른 기간이 지금 학기({} {} ~ {})를 벗어납니다. \
         기록이 남아 있는 기간이면 그대로 계산합니다.",
        t.name, ts, te
    ))
}

// ============================================================
//  집계
// ============================================================

/// 기간 안의 배정 기록을 사람별로 모은다. **`ASSIGNED` 만 센다.**
fn tallies(conn: &Connection, from: &str, to: &str) -> AppResult<HashMap<i64, Tally>> {
    let mut out: HashMap<i64, Tally> = HashMap::new();

    // (1) 대신 들어간 건
    let mut stmt = conn.prepare(
        "SELECT sub_teacher_id, date, slot_type, start_min
           FROM substitutions
          WHERE status = 'ASSIGNED' AND date >= ?1 AND date <= ?2
          ORDER BY date, start_min",
    )?;
    for row in stmt.query_map(params![from, to], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            Case {
                date: r.get(1)?,
                slot_type: r.get(2)?,
                start_min: r.get(3)?,
            },
        ))
    })? {
        let (tid, case) = row?;
        out.entry(tid).or_default().cases.push(case);
    }

    // (2) 본인 결근으로 다른 사람이 들어간 건.
    //     결근자를 적어 두지 않은 기록(absent_teacher_id IS NULL)은 셀 수 없다.
    let mut stmt = conn.prepare(
        "SELECT absent_teacher_id, date, slot_type, start_min
           FROM substitutions
          WHERE status = 'ASSIGNED' AND date >= ?1 AND date <= ?2
            AND absent_teacher_id IS NOT NULL
          ORDER BY date, start_min",
    )?;
    for row in stmt.query_map(params![from, to], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            Case {
                date: r.get(1)?,
                slot_type: r.get(2)?,
                start_min: r.get(3)?,
            },
        ))
    })? {
        let (tid, case) = row?;
        out.entry(tid).or_default().own_caused.push(case);
    }

    Ok(out)
}

struct Person {
    id: i64,
    name: String,
    role_code: String,
    role_label: String,
    active: bool,
    memo: Option<String>,
}

fn people(conn: &Connection) -> AppResult<Vec<Person>> {
    let mut stmt = conn.prepare(
        "SELECT t.id, t.name, t.role_code, r.label, t.active, t.memo
           FROM teachers t JOIN teacher_roles r ON r.code = t.role_code
          ORDER BY r.sort_order, t.name",
    )?;
    let v = stmt
        .query_map([], |r| {
            Ok(Person {
                id: r.get(0)?,
                name: r.get(1)?,
                role_code: r.get(2)?,
                role_label: r.get(3)?,
                active: r.get::<_, i64>(4)? != 0,
                memo: r.get(5)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(v)
}

/// 담임이면 학급, 전담이면 과목, 기타면 메모.
fn duty_text(conn: &Connection) -> AppResult<HashMap<i64, String>> {
    let term_id = school::current_term_id(conn).unwrap_or(0);
    let mut out: HashMap<i64, Vec<String>> = HashMap::new();

    let mut stmt = conn.prepare(
        "SELECT homeroom_teacher_id, grade, class_no, name FROM classes
          WHERE term_id = ?1 AND active = 1 AND homeroom_teacher_id IS NOT NULL
          ORDER BY grade, class_no",
    )?;
    for row in stmt.query_map([term_id], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, i32>(1)?,
            r.get::<_, i32>(2)?,
            r.get::<_, Option<String>>(3)?,
        ))
    })? {
        let (tid, grade, no, name) = row?;
        out.entry(tid)
            .or_default()
            .push(crate::label::class_short(grade, no, name.as_deref()));
    }

    let mut stmt = conn.prepare(
        "SELECT ts.teacher_id, s.name
           FROM teacher_subjects ts JOIN subjects s ON s.id = ts.subject_id
          ORDER BY s.name",
    )?;
    for row in stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))? {
        let (tid, name) = row?;
        out.entry(tid).or_default().push(name);
    }

    Ok(out
        .into_iter()
        .map(|(k, v)| (k, v.join(" · ")))
        .collect())
}

// ============================================================
//  조회
// ============================================================

pub fn view(conn: &Connection, q: &PayQuery) -> AppResult<PayView> {
    let (mode, month, range) = resolve(q);
    let cfg = settings::pay_config(conn)?;
    let rule = pay::rule_of(&cfg.policy);

    let tallies = tallies(conn, &range.from, &range.to)?;
    let duties = duty_text(conn)?;

    let mut rows = Vec::new();
    let mut sum = PaySummary::default();

    for p in people(conn)? {
        let Some(t) = tallies.get(&p.id) else { continue };
        // 보결도 없고 본인 발생분도 없는 사람은 표에 올리지 않는다
        if t.substituted() == 0 && t.own_caused_count() == 0 {
            continue;
        }
        let s = pay::settle(&cfg, t);

        sum.listed_teachers += 1;
        sum.total_substituted += s.substituted;
        sum.total_own_caused += s.own_caused;
        sum.total_payable += s.payable;
        sum.total_amount += s.amount;
        if s.payable > 0 {
            sum.paid_teachers += 1;
        }

        rows.push(PayRow {
            teacher_id: p.id,
            name: p.name,
            role_code: p.role_code,
            role_label: p.role_label,
            duty: duties
                .get(&p.id)
                .cloned()
                .or_else(|| p.memo.clone())
                .unwrap_or_default(),
            active: p.active,
            substituted: s.substituted,
            own_caused: s.own_caused,
            payable: s.payable,
            per_case: s.per_case,
            amount: s.amount,
        });
    }

    // 지급액이 큰 사람부터. 같으면 횟수, 그다음 이름.
    rows.sort_by(|a, b| {
        b.amount
            .cmp(&a.amount)
            .then(b.payable.cmp(&a.payable))
            .then(b.substituted.cmp(&a.substituted))
            .then(a.name.cmp(&b.name))
    });

    let term = current_term(conn)?;

    Ok(PayView {
        mode,
        prev_month: period::shift_month(&month, -1).unwrap_or_else(|| month.clone()),
        next_month: period::shift_month(&month, 1).unwrap_or_else(|| month.clone()),
        month,
        from: range.from.clone(),
        to: range.to.clone(),
        range_label: range.label.clone(),
        policy: cfg.policy.clone(),
        policy_label: rule.label().to_string(),
        policy_hint: rule.hint().to_string(),
        per_case: cfg.per_case,
        per_case_unset: cfg.per_case <= 0,
        summary: sum,
        rows,
        term_note: term_note(term.as_ref(), &range),
    })
}

/// 어느 열에서 사람을 찾을지. 상세의 두 목록이 같은 SQL을 쓴다.
enum Side {
    /// 이 사람이 대신 들어간 건 → 상대는 결근한 선생님
    Substituted,
    /// 이 사람의 결근으로 생긴 건 → 상대는 대신 들어간 선생님
    OwnCaused,
}

fn case_rows(
    conn: &Connection,
    teacher_id: i64,
    from: &str,
    to: &str,
    side: Side,
) -> AppResult<Vec<CaseRow>> {
    // 학급 표기는 기록 당시 스냅샷을 쓴다. 반 이름을 바꿔도 과거 기록이
    // 달라지지 않아야 한다. 예전 기록에 스냅샷이 없으면 학년-반 숫자로 만든다.
    let (whose, other) = match side {
        Side::Substituted => ("s.sub_teacher_id", "s.absent_teacher_id"),
        Side::OwnCaused => ("s.absent_teacher_id", "s.sub_teacher_id"),
    };
    let sql = format!(
        "SELECT s.date, s.day_of_week,
                COALESCE(s.class_label, s.grade || '-' || s.class_no),
                s.slot_label, s.start_min, s.end_min,
                COALESCE(o.name, ''), ar.label
           FROM substitutions s
           LEFT JOIN teachers o ON o.id = {other}
           LEFT JOIN absence_reasons ar ON ar.code = s.reason_code
          WHERE s.status = 'ASSIGNED'
            AND {whose} = ?1
            AND s.date >= ?2 AND s.date <= ?3
          ORDER BY s.date, s.start_min"
    );
    let mut stmt = conn.prepare(&sql)?;
    let v = stmt
        .query_map(params![teacher_id, from, to], |r| {
            Ok(CaseRow {
                date: r.get(0)?,
                day_of_week: r.get(1)?,
                class_label: r.get(2)?,
                slot_label: r.get(3)?,
                start_min: r.get(4)?,
                end_min: r.get(5)?,
                counterpart: r.get(6)?,
                reason_label: r.get(7)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(v)
}

pub fn detail(conn: &Connection, teacher_id: i64, q: &PayQuery) -> AppResult<PayDetail> {
    let (_, _, range) = resolve(q);
    let cfg = settings::pay_config(conn)?;
    let rule = pay::rule_of(&cfg.policy);

    let (name, role_label) = conn
        .query_row(
            "SELECT t.name, r.label FROM teachers t
               JOIN teacher_roles r ON r.code = t.role_code
              WHERE t.id = ?1",
            [teacher_id],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
        )
        .optional()?
        .unwrap_or_else(|| ("(삭제된 교사)".to_string(), String::new()));

    let substituted = case_rows(conn, teacher_id, &range.from, &range.to, Side::Substituted)?;
    let own_caused = case_rows(conn, teacher_id, &range.from, &range.to, Side::OwnCaused)?;

    // 상세 목록과 계산이 어긋날 수 없도록, 같은 목록으로 집계를 만든다.
    let tally = Tally {
        cases: substituted
            .iter()
            .map(|c| Case {
                date: c.date.clone(),
                slot_type: String::new(),
                start_min: c.start_min,
            })
            .collect(),
        own_caused: own_caused
            .iter()
            .map(|c| Case {
                date: c.date.clone(),
                slot_type: String::new(),
                start_min: c.start_min,
            })
            .collect(),
    };
    let s = pay::settle(&cfg, &tally);

    Ok(PayDetail {
        teacher_id,
        name,
        role_label,
        from: range.from,
        to: range.to,
        range_label: range.label,
        policy: cfg.policy.clone(),
        policy_label: rule.label().to_string(),
        per_case: s.per_case,
        substituted,
        own_caused,
        payable: s.payable,
        amount: s.amount,
        steps: pay::explain(&cfg, &tally),
        deducts: cfg.policy == pay::DEDUCT_OWN_CAUSED,
    })
}

/// 지금 조회 결과를 엑셀 시트로 만든다. 파일로 쓰는 일은 `repo::xlsx` 가 한다.
///
/// **금액과 횟수는 숫자로 담는다.** 엑셀에서 그대로 합계를 낼 수 있어야 한다
/// (천 단위 쉼표는 서식으로 보여 준다). 지급 기준과 1회 금액은 '조회 조건'
/// 시트에 적어, 자료 시트의 머리글이 늘 1행이도록 한다.
pub fn sheets(conn: &Connection, q: &PayQuery) -> AppResult<Vec<Sheet>> {
    let v = view(conn, q)?;

    let mut table = Sheet::new(
        "교사별 수당",
        vec![
            col("교사명", 13.0),
            col("구분", 10.0),
            col("담당", 16.0),
            col("보결 횟수", 11.0),
            col("본인 발생 보결 횟수", 19.0),
            col("지급 인정 횟수", 14.0),
            col("1회 보결 수당", 15.0),
            col("지급액", 15.0),
        ],
    );
    for r in &v.rows {
        table.push(vec![
            Cell::text(r.name.clone()),
            Cell::text(r.role_label.clone()),
            Cell::text(r.duty.clone()),
            Cell::Count(r.substituted as i64),
            Cell::Count(r.own_caused as i64),
            Cell::Count(r.payable as i64),
            Cell::Money(r.per_case),
            Cell::Money(r.amount),
        ]);
    }
    let table = table.with_total(vec![
        Cell::text("합계"),
        Cell::Blank,
        Cell::Blank,
        Cell::Count(v.summary.total_substituted as i64),
        Cell::Count(v.summary.total_own_caused as i64),
        Cell::Count(v.summary.total_payable as i64),
        Cell::Blank,
        Cell::Money(v.summary.total_amount),
    ]);

    let cond = super::export::conditions(&[
        ("기간", format!("{} ~ {}", v.from, v.to)),
        ("기간 이름", v.range_label.clone()),
        ("지급 기준", v.policy_label.clone()),
        ("지급 기준 코드", v.policy.clone()),
        ("1회 보결 수당", format!("{}원", pay::won(v.per_case))),
        ("지급 대상 교사", format!("{}명", v.summary.paid_teachers)),
        ("총 지급액", format!("{}원", pay::won(v.summary.total_amount))),
    ]);

    Ok(vec![table, cond])
}

#[cfg(test)]
#[path = "pay_tests.rs"]
mod tests;
