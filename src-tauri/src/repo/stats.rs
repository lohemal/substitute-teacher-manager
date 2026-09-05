//! Phase 9 — 보결 현황. **원본에서 그때그때 다시 센다.**
//!
//! ## 통계값을 따로 저장하지 않는다
//!
//! 모든 숫자는 `substitutions`와 `absences`를 그 자리에서 세어 만든다.
//! 그래서 배정을 취소하거나 고치면 다음 조회에서 곧바로 반영된다.
//! 집계표를 따로 두면 반드시 어긋나는 날이 오는데, 학교 업무에서 숫자가
//! 어긋나는 것은 없느니만 못하다.
//!
//! ## '보결이 필요한 건수'는 조회 엔진으로 센다
//!
//! 결근한 선생님이 그 날 맡은 시간(`domain::assign::duty_slots`)이 곧 필요한
//! 건수다. 이미 배정된 것과 맞춰 보면 **아직 배정하지 않은 시간**이 나온다.
//! 배정 화면과 같은 계산을 쓰므로 두 화면의 숫자가 어긋나지 않는다.

use std::collections::{HashMap, HashSet};

use chrono::NaiveDate;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use super::{find as repo_find, school};
use crate::domain::assign::duty_slots;
use crate::domain::fairness::{self, RoleSpread, Share};
use crate::domain::period::{self, Range, TermInfo};
use crate::domain::time::Interval;
use crate::error::AppResult;

/// 날짜별 현황을 한 번에 만들 최대 일수. 학기 전체(약 6개월)도 들어간다.
const MAX_DAYS: usize = 200;

// ============================================================
//  요청 / 결과
// ============================================================

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatsQuery {
    /// TODAY | WEEK | MONTH | TERM | CUSTOM
    #[serde(default)]
    pub preset: Option<String>,
    #[serde(default)]
    pub from: Option<String>,
    #[serde(default)]
    pub to: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    /// 기간 안에 결근한 선생님 수 (사람 기준, 중복 없음)
    pub absent_teachers: i32,
    /// 결근으로 생긴 결근 건수 (기록 수)
    pub absence_count: i32,
    /// 보결이 필요한 시간 수
    pub required: i32,
    /// 그 가운데 배정을 마친 수 (required = covered + unassigned)
    pub covered: i32,
    /// 기간 안의 배정 건수 전체.
    /// 결근을 등록하지 않고 바로 배정한 건도 들어가므로 covered 보다 클 수 있다.
    pub assigned: i32,
    /// 필요하지만 아직 배정하지 않은 시간 수
    pub unassigned: i32,
    pub cancelled: i32,
    /// 보결을 맡은 선생님 수
    pub sub_teachers: i32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TeacherStat {
    pub teacher_id: i64,
    pub name: String,
    pub role_code: String,
    pub role_label: String,
    /// 담임이면 학급, 전담이면 과목, 기타면 담당 업무
    pub duty: String,
    pub active: bool,
    pub is_substitutable: bool,
    /// 고른 기간
    pub period: i32,
    pub today: i32,
    pub week: i32,
    pub month: i32,
    pub term: i32,
    pub total: i32,
    /// MORE | TYPICAL | LESS | NONE — 같은 구분 안에서의 참고 표시
    pub band: String,
    pub band_label: String,
    /// 기간 안 마지막 보결 날짜
    pub last_date: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AbsenceStat {
    pub teacher_id: i64,
    pub name: String,
    pub role_label: String,
    /// 결근 기록 수
    pub count: i32,
    pub all_day: i32,
    pub partial: i32,
    /// '연가 2 · 출장 1'
    pub reasons: String,
    /// 이 결근들로 필요한 보결 시간 수
    pub required: i32,
    pub assigned: i32,
    pub unassigned: i32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DayStat {
    pub date: String,
    pub day_of_week: i32,
    pub absent_teachers: i32,
    /// '김철수, 이영희'
    pub absent_names: String,
    pub required: i32,
    pub assigned: i32,
    pub unassigned: i32,
    pub cancelled: i32,
}

/// 아직 배정하지 않은 시간 — 담당자가 바로 찾아갈 수 있게 목록으로 준다.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenSlot {
    pub date: String,
    pub day_of_week: i32,
    pub class_id: i64,
    pub class_label: String,
    pub slot_type: String,
    pub period_no: Option<i32>,
    pub slot_label: String,
    pub start_min: i32,
    pub end_min: i32,
    pub kind_label: String,
    pub absent_teacher_id: i64,
    pub absent_teacher_name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatsView {
    pub from: String,
    pub to: String,
    pub range_label: String,
    pub preset: String,
    /// 기간이 길어 날짜별 현황을 잘랐는가
    pub days_truncated: bool,
    pub summary: Summary,
    pub teachers: Vec<TeacherStat>,
    pub spreads: Vec<RoleSpread>,
    pub fairness_note: String,
    pub absences: Vec<AbsenceStat>,
    pub days: Vec<DayStat>,
    pub open_slots: Vec<OpenSlot>,
}

// ============================================================
//  기간
// ============================================================

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

fn today_local() -> NaiveDate {
    chrono::Local::now().date_naive()
}

/// 고른 기간을 실제 날짜 범위로 바꾼다.
pub fn resolve_range(conn: &Connection, q: &StatsQuery) -> AppResult<(String, Range)> {
    let preset = q.preset.clone().unwrap_or_else(|| period::TODAY.to_string());
    let term = current_term(conn)?;
    let range = period::resolve(
        &preset,
        today_local(),
        term.as_ref(),
        (q.from.as_deref(), q.to.as_deref()),
    );
    Ok((preset, range))
}

// ============================================================
//  보결이 필요한 시간 (배정 화면과 같은 계산)
// ============================================================

struct Need {
    date: String,
    day_of_week: i32,
    class_id: i64,
    class_label: String,
    slot_type: String,
    period_no: Option<i32>,
    slot_label: String,
    start_min: i32,
    end_min: i32,
    kind_label: String,
    absent_teacher_id: i64,
    absent_teacher_name: String,
    assigned: bool,
}

/// 결근이 등록된 날만 하루치 자료를 읽어 필요한 시간을 뽑는다.
///
/// 결근이 없는 날은 보결도 필요 없으므로 건너뛴다. 그래서 한 달치를 봐도
/// 실제로 읽는 날은 몇 날뿐이다.
fn needs_in(conn: &Connection, dates: &[String]) -> AppResult<Vec<Need>> {
    if dates.is_empty() {
        return Ok(Vec::new());
    }
    let marks = vec!["?"; dates.len()].join(",");

    // 1) 기간 안의 활성 결근
    let sql = format!(
        "SELECT a.date, a.teacher_id, t.name, a.is_all_day, a.start_min, a.end_min
           FROM absences a JOIN teachers t ON t.id = a.teacher_id
          WHERE a.status = 'ACTIVE' AND a.date IN ({marks})
          ORDER BY a.date, t.name"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows: Vec<(String, i64, String, bool, Option<i32>, Option<i32>)> = stmt
        .query_map(rusqlite::params_from_iter(dates.iter()), |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get::<_, i64>(3)? != 0,
                r.get(4)?,
                r.get(5)?,
            ))
        })?
        .collect::<rusqlite::Result<_>>()?;

    if rows.is_empty() {
        return Ok(Vec::new());
    }

    // 2) 그 날짜의 배정 (학급 + 시작 시각으로 맞춘다 — 스냅샷 값이므로 안전하다)
    let sql = format!(
        "SELECT date, class_id, start_min FROM substitutions
          WHERE status = 'ASSIGNED' AND date IN ({marks})"
    );
    let mut stmt = conn.prepare(&sql)?;
    let mut booked: HashSet<(String, i64, i32)> = HashSet::new();
    for row in stmt.query_map(rusqlite::params_from_iter(dates.iter()), |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, Option<i64>>(1)?,
            r.get::<_, i32>(2)?,
        ))
    })? {
        let (d, cid, start) = row?;
        if let Some(cid) = cid {
            booked.insert((d, cid, start));
        }
    }

    // 3) 결근이 있는 날만 하루치 자료를 읽는다
    let mut out: Vec<Need> = Vec::new();
    let mut cache_date = String::new();
    let mut snap = None;

    for (date, teacher_id, name, all_day, start, end) in rows {
        if cache_date != date {
            snap = repo_find::snapshot(conn, &date).ok();
            cache_date = date.clone();
        }
        let Some(s) = snap.as_ref() else { continue };

        let window = (!all_day).then(|| Interval::new(start.unwrap_or(0), end.unwrap_or(1440)));
        for d in duty_slots(s, teacher_id, window) {
            out.push(Need {
                date: date.clone(),
                day_of_week: s.day_of_week,
                class_id: d.class_id,
                class_label: d.class_label.clone(),
                slot_type: d.slot_type.clone(),
                period_no: d.period_no,
                slot_label: d.slot_label.clone(),
                start_min: d.start_min,
                end_min: d.end_min,
                kind_label: crate::domain::assign::duty_kind_label(&d.kind).to_string(),
                absent_teacher_id: teacher_id,
                absent_teacher_name: name.clone(),
                assigned: booked.contains(&(date.clone(), d.class_id, d.start_min)),
            });
        }
    }

    Ok(out)
}

// ============================================================
//  세기
// ============================================================

/// 기간별 배정 횟수 (취소는 빠진다)
fn counts_between(conn: &Connection, from: &str, to: &str) -> AppResult<HashMap<i64, i32>> {
    let mut stmt = conn.prepare(
        "SELECT sub_teacher_id, COUNT(*) FROM substitutions
          WHERE status = 'ASSIGNED' AND date >= ?1 AND date <= ?2
          GROUP BY sub_teacher_id",
    )?;
    let mut out = HashMap::new();
    for row in stmt.query_map(params![from, to], |r| {
        Ok((r.get::<_, i64>(0)?, r.get::<_, i32>(1)?))
    })? {
        let (id, n) = row?;
        out.insert(id, n);
    }
    Ok(out)
}

fn count_all(conn: &Connection) -> AppResult<HashMap<i64, i32>> {
    let mut stmt = conn.prepare(
        "SELECT sub_teacher_id, COUNT(*) FROM substitutions
          WHERE status = 'ASSIGNED' GROUP BY sub_teacher_id",
    )?;
    let mut out = HashMap::new();
    for row in stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i32>(1)?)))? {
        let (id, n) = row?;
        out.insert(id, n);
    }
    Ok(out)
}

/// 기간 안에서 각 교사가 마지막으로 보결한 날
fn last_dates(conn: &Connection, from: &str, to: &str) -> AppResult<HashMap<i64, String>> {
    let mut stmt = conn.prepare(
        "SELECT sub_teacher_id, MAX(date) FROM substitutions
          WHERE status = 'ASSIGNED' AND date >= ?1 AND date <= ?2
          GROUP BY sub_teacher_id",
    )?;
    let mut out = HashMap::new();
    for row in stmt.query_map(params![from, to], |r| {
        Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
    })? {
        let (id, d) = row?;
        out.insert(id, d);
    }
    Ok(out)
}

// ============================================================
//  본체
// ============================================================

pub fn view(conn: &Connection, q: &StatsQuery) -> AppResult<StatsView> {
    let (preset, range) = resolve_range(conn, q)?;
    let (from, to) = (range.from.clone(), range.to.clone());

    let today = today_local().format("%Y-%m-%d").to_string();
    let week = period::resolve(period::WEEK, today_local(), None, (None, None));
    let month = period::resolve(period::MONTH, today_local(), None, (None, None));
    let term_info = current_term(conn)?;
    let term = period::resolve(
        period::TERM,
        today_local(),
        term_info.as_ref(),
        (None, None),
    );

    // ---------- 필요한 시간 ----------
    let dates = period::dates_in(&range, MAX_DAYS);
    let days_truncated = period::dates_in(&range, MAX_DAYS + 1).len() > MAX_DAYS;
    let needs = needs_in(conn, &dates)?;

    // ---------- 교사별 ----------
    let c_period = counts_between(conn, &from, &to)?;
    let c_today = counts_between(conn, &today, &today)?;
    let c_week = counts_between(conn, &week.from, &week.to)?;
    let c_month = counts_between(conn, &month.from, &month.to)?;
    let c_term = counts_between(conn, &term.from, &term.to)?;
    let c_all = count_all(conn)?;
    let last = last_dates(conn, &from, &to)?;

    let mut stmt = conn.prepare(
        "SELECT t.id, t.name, t.role_code, r.label, t.active, t.is_substitutable, t.memo
           FROM teachers t JOIN teacher_roles r ON r.code = t.role_code
          ORDER BY r.sort_order, t.name",
    )?;
    struct Row {
        id: i64,
        name: String,
        role_code: String,
        role_label: String,
        active: bool,
        is_sub: bool,
        memo: Option<String>,
    }
    let people: Vec<Row> = stmt
        .query_map([], |r| {
            Ok(Row {
                id: r.get(0)?,
                name: r.get(1)?,
                role_code: r.get(2)?,
                role_label: r.get(3)?,
                active: r.get::<_, i64>(4)? != 0,
                is_sub: r.get::<_, i64>(5)? != 0,
                memo: r.get(6)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;

    let duties = duty_text(conn)?;

    // 참고 표시는 **지금 보결 대상인 사람들끼리만** 견준다.
    // 비활성이거나 보결 대상이 아닌 사람을 섞으면 평균이 왜곡된다.
    let shares: Vec<Share> = people
        .iter()
        .filter(|p| p.active && p.is_sub)
        .map(|p| Share {
            teacher_id: p.id,
            name: p.name.clone(),
            role_code: p.role_code.clone(),
            role_label: p.role_label.clone(),
            count: c_period.get(&p.id).copied().unwrap_or(0),
        })
        .collect();
    let spreads = fairness::spreads(&shares);

    let teachers: Vec<TeacherStat> = people
        .iter()
        .map(|p| {
            let share = shares.iter().find(|s| s.teacher_id == p.id);
            let band = match share {
                Some(s) => fairness::band_of(s, &spreads),
                None => fairness::BAND_NONE.to_string(),
            };
            TeacherStat {
                teacher_id: p.id,
                name: p.name.clone(),
                role_code: p.role_code.clone(),
                role_label: p.role_label.clone(),
                duty: duties
                    .get(&p.id)
                    .cloned()
                    .or_else(|| p.memo.clone())
                    .unwrap_or_default(),
                active: p.active,
                is_substitutable: p.is_sub,
                period: c_period.get(&p.id).copied().unwrap_or(0),
                today: c_today.get(&p.id).copied().unwrap_or(0),
                week: c_week.get(&p.id).copied().unwrap_or(0),
                month: c_month.get(&p.id).copied().unwrap_or(0),
                term: c_term.get(&p.id).copied().unwrap_or(0),
                total: c_all.get(&p.id).copied().unwrap_or(0),
                band_label: fairness::band_label(&band).to_string(),
                band,
                last_date: last.get(&p.id).cloned(),
            }
        })
        .collect();

    // ---------- 요약 ----------
    let assigned: i32 = conn.query_row(
        "SELECT COUNT(*) FROM substitutions
          WHERE status = 'ASSIGNED' AND date >= ?1 AND date <= ?2",
        params![from, to],
        |r| r.get(0),
    )?;
    let cancelled: i32 = conn.query_row(
        "SELECT COUNT(*) FROM substitutions
          WHERE status = 'CANCELLED' AND date >= ?1 AND date <= ?2",
        params![from, to],
        |r| r.get(0),
    )?;
    let sub_teachers: i32 = conn.query_row(
        "SELECT COUNT(DISTINCT sub_teacher_id) FROM substitutions
          WHERE status = 'ASSIGNED' AND date >= ?1 AND date <= ?2",
        params![from, to],
        |r| r.get(0),
    )?;
    let (absent_teachers, absence_count): (i32, i32) = conn.query_row(
        "SELECT COUNT(DISTINCT teacher_id), COUNT(*) FROM absences
          WHERE status = 'ACTIVE' AND date >= ?1 AND date <= ?2",
        params![from, to],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;

    let required = needs.len() as i32;
    let covered = needs.iter().filter(|n| n.assigned).count() as i32;
    let unassigned = required - covered;

    let summary = Summary {
        absent_teachers,
        absence_count,
        required,
        covered,
        assigned,
        unassigned,
        cancelled,
        sub_teachers,
    };

    // ---------- 결근 현황 ----------
    let absences = absence_stats(conn, &from, &to, &needs)?;

    // ---------- 날짜별 ----------
    let days = day_stats(conn, &dates, &needs)?;

    // ---------- 미배정 목록 ----------
    let mut open_slots: Vec<OpenSlot> = needs
        .iter()
        .filter(|n| !n.assigned)
        .map(|n| OpenSlot {
            date: n.date.clone(),
            day_of_week: n.day_of_week,
            class_id: n.class_id,
            class_label: n.class_label.clone(),
            slot_type: n.slot_type.clone(),
            period_no: n.period_no,
            slot_label: n.slot_label.clone(),
            start_min: n.start_min,
            end_min: n.end_min,
            kind_label: n.kind_label.clone(),
            absent_teacher_id: n.absent_teacher_id,
            absent_teacher_name: n.absent_teacher_name.clone(),
        })
        .collect();
    open_slots.sort_by(|a, b| {
        a.date
            .cmp(&b.date)
            .then(a.start_min.cmp(&b.start_min))
            .then(a.class_label.cmp(&b.class_label))
    });

    Ok(StatsView {
        from,
        to,
        range_label: range.label,
        preset,
        days_truncated,
        summary,
        teachers,
        spreads,
        fairness_note: fairness::HOW_TEXT.to_string(),
        absences,
        days,
        open_slots,
    })
}

/// 담임이면 담당 학급, 전담이면 과목.
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
        "SELECT ts.teacher_id, s.name FROM teacher_subjects ts
           JOIN subjects s ON s.id = ts.subject_id
          ORDER BY ts.is_primary DESC, s.id",
    )?;
    for row in stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))? {
        let (tid, name) = row?;
        out.entry(tid).or_default().push(name);
    }

    Ok(out.into_iter().map(|(k, v)| (k, v.join(", "))).collect())
}

fn absence_stats(
    conn: &Connection,
    from: &str,
    to: &str,
    needs: &[Need],
) -> AppResult<Vec<AbsenceStat>> {
    let mut stmt = conn.prepare(
        "SELECT a.teacher_id, t.name, r0.label, a.is_all_day,
                COALESCE(r.label, a.reason_text, '사유 없음')
           FROM absences a
           JOIN teachers t       ON t.id = a.teacher_id
           JOIN teacher_roles r0 ON r0.code = t.role_code
      LEFT JOIN absence_reasons r ON r.code = a.reason_code
          WHERE a.status = 'ACTIVE' AND a.date >= ?1 AND a.date <= ?2
          ORDER BY t.name",
    )?;

    struct Acc {
        name: String,
        role_label: String,
        count: i32,
        all_day: i32,
        partial: i32,
        reasons: Vec<(String, i32)>,
    }
    let mut acc: HashMap<i64, Acc> = HashMap::new();
    let mut order: Vec<i64> = Vec::new();

    for row in stmt.query_map(params![from, to], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, i64>(3)? != 0,
            r.get::<_, String>(4)?,
        ))
    })? {
        let (tid, name, role, all_day, reason) = row?;
        let e = acc.entry(tid).or_insert_with(|| {
            order.push(tid);
            Acc {
                name,
                role_label: role,
                count: 0,
                all_day: 0,
                partial: 0,
                reasons: Vec::new(),
            }
        });
        e.count += 1;
        if all_day {
            e.all_day += 1;
        } else {
            e.partial += 1;
        }
        match e.reasons.iter_mut().find(|(r, _)| r == &reason) {
            Some((_, n)) => *n += 1,
            None => e.reasons.push((reason, 1)),
        }
    }

    Ok(order
        .into_iter()
        .map(|tid| {
            let a = acc.remove(&tid).unwrap();
            let mine: Vec<&Need> = needs
                .iter()
                .filter(|n| n.absent_teacher_id == tid)
                .collect();
            AbsenceStat {
                teacher_id: tid,
                name: a.name,
                role_label: a.role_label,
                count: a.count,
                all_day: a.all_day,
                partial: a.partial,
                reasons: a
                    .reasons
                    .iter()
                    .map(|(r, n)| if *n > 1 { format!("{r} {n}") } else { r.clone() })
                    .collect::<Vec<_>>()
                    .join(" · "),
                required: mine.len() as i32,
                assigned: mine.iter().filter(|n| n.assigned).count() as i32,
                unassigned: mine.iter().filter(|n| !n.assigned).count() as i32,
            }
        })
        .collect())
}

fn day_stats(conn: &Connection, dates: &[String], needs: &[Need]) -> AppResult<Vec<DayStat>> {
    if dates.is_empty() {
        return Ok(Vec::new());
    }
    let marks = vec!["?"; dates.len()].join(",");

    // 날짜별 배정·취소
    let sql = format!(
        "SELECT date,
                SUM(CASE WHEN status = 'ASSIGNED'  THEN 1 ELSE 0 END),
                SUM(CASE WHEN status = 'CANCELLED' THEN 1 ELSE 0 END)
           FROM substitutions WHERE date IN ({marks}) GROUP BY date"
    );
    let mut stmt = conn.prepare(&sql)?;
    let mut subs: HashMap<String, (i32, i32)> = HashMap::new();
    for row in stmt.query_map(rusqlite::params_from_iter(dates.iter()), |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, i32>(1)?, r.get::<_, i32>(2)?))
    })? {
        let (d, a, c) = row?;
        subs.insert(d, (a, c));
    }

    // 날짜별 결근 교사
    let sql = format!(
        "SELECT a.date, t.name FROM absences a JOIN teachers t ON t.id = a.teacher_id
          WHERE a.status = 'ACTIVE' AND a.date IN ({marks})
          ORDER BY a.date, t.name"
    );
    let mut stmt = conn.prepare(&sql)?;
    let mut absent: HashMap<String, Vec<String>> = HashMap::new();
    for row in stmt.query_map(rusqlite::params_from_iter(dates.iter()), |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
    })? {
        let (d, n) = row?;
        let list = absent.entry(d).or_default();
        if !list.contains(&n) {
            list.push(n);
        }
    }

    Ok(dates
        .iter()
        .map(|d| {
            let (assigned, cancelled) = subs.get(d).copied().unwrap_or((0, 0));
            let mine: Vec<&Need> = needs.iter().filter(|n| &n.date == d).collect();
            let names = absent.get(d).cloned().unwrap_or_default();
            DayStat {
                date: d.clone(),
                day_of_week: repo_find::parse_date(d).map(|(_, w)| w).unwrap_or(0),
                absent_teachers: names.len() as i32,
                absent_names: names.join(", "),
                required: mine.len() as i32,
                assigned,
                unassigned: mine.iter().filter(|n| !n.assigned).count() as i32,
                cancelled,
            }
        })
        .collect())
}

#[cfg(test)]
#[path = "stats_tests.rs"]
mod tests;
