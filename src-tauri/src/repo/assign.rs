//! Phase 8 — 결근 등록 · 보결 배정 · 취소 · 배정 내역. SQL 전담.
//!
//! ## 두 가지 원칙
//!
//! 1. **지우지 않는다.** 결근도 배정도 `status`를 바꿔 취소한다. 과거 기록은
//!    그대로 남고, 취소된 것은 보결 횟수 집계에서만 빠진다.
//! 2. **저장 직전에 다시 판단한다.** 조회 결과를 믿고 저장하지 않는다.
//!    쓰기 트랜잭션 안에서 하루치 자료를 다시 읽고 `domain::assign::verify`를
//!    돌린 뒤에 넣는다. 그래서 조회와 저장 사이에 상황이 바뀌어도 안전하다.

use std::collections::HashMap;

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use super::{find as repo_find, priority as repo_priority, school};
use crate::domain::assign::{self, AssignPlan, AssignRow, DutySlot};
use crate::domain::find::{Candidate, FindRequest, SlotNotice, SubCounts};
use crate::domain::priority::rank_candidates;
use crate::domain::schedule::DaySnapshot;
use crate::domain::time::Interval;
use crate::error::{AppError, AppResult};

// ============================================================
//  결근 등록
// ============================================================

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AbsenceInput {
    pub teacher_id: i64,
    /// YYYY-MM-DD
    pub date: String,
    pub is_all_day: bool,
    /// is_all_day = false 일 때만 쓴다 (자정 기준 분)
    #[serde(default)]
    pub start_min: Option<i32>,
    #[serde(default)]
    pub end_min: Option<i32>,
    #[serde(default)]
    pub reason_code: Option<String>,
    #[serde(default)]
    pub reason_text: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AbsenceRow {
    pub id: i64,
    pub date: String,
    pub teacher_id: i64,
    pub teacher_name: String,
    pub role_label: String,
    pub is_all_day: bool,
    pub start_min: Option<i32>,
    pub end_min: Option<i32>,
    /// '연가' 등. 화면에 그대로 쓴다
    pub reason_label: String,
    pub reason_code: Option<String>,
    pub reason_text: Option<String>,
    pub status: String,
    pub created_at: String,
    pub cancelled_at: Option<String>,
    /// 이 결근으로 이미 배정된 보결 건수 (활성만)
    pub sub_count: i32,
}

fn check_absence(input: &AbsenceInput) -> AppResult<(Option<i32>, Option<i32>)> {
    repo_find::parse_date(&input.date)?;
    if input.is_all_day {
        return Ok((None, None));
    }
    let (Some(s), Some(e)) = (input.start_min, input.end_min) else {
        return Err(AppError::invalid(
            "일부 시간 결근은 시작 시각과 종료 시각을 입력해 주세요.",
        ));
    };
    if !(0..=1440).contains(&s) || !(0..=1440).contains(&e) {
        return Err(AppError::invalid("시각은 00:00~24:00 사이로 입력해 주세요."));
    }
    if e <= s {
        return Err(AppError::invalid(
            "종료 시각이 시작 시각보다 뒤여야 합니다.",
        ));
    }
    Ok((Some(s), Some(e)))
}

/// 결근을 등록한다. 등록 즉시 그 시간대 보결 조회에서 제외된다.
pub fn create_absence(conn: &Connection, input: &AbsenceInput) -> AppResult<i64> {
    let (start, end) = check_absence(input)?;
    let term_id = school::current_term_id(conn)?;

    let name: Option<String> = conn
        .query_row(
            "SELECT name FROM teachers WHERE id = ?1",
            [input.teacher_id],
            |r| r.get(0),
        )
        .optional()?;
    let name = name.ok_or_else(|| AppError::not_found("선생님을 찾을 수 없습니다."))?;

    // 같은 날 종일 결근이 이미 있으면 두 번 넣을 이유가 없다
    let all_day_exists: i64 = conn.query_row(
        "SELECT COUNT(*) FROM absences
          WHERE teacher_id = ?1 AND date = ?2 AND status = 'ACTIVE' AND is_all_day = 1",
        params![input.teacher_id, input.date],
        |r| r.get(0),
    )?;
    if all_day_exists > 0 {
        return Err(AppError::new(
            "DUPLICATE",
            format!("{name} 선생님은 이 날 이미 종일 결근으로 등록되어 있습니다."),
        ));
    }

    conn.execute(
        "INSERT INTO absences
            (term_id, teacher_id, date, is_all_day, start_min, end_min, reason_code, reason_text)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            term_id,
            input.teacher_id,
            input.date.trim(),
            if input.is_all_day { 1 } else { 0 },
            start,
            end,
            input.reason_code.as_deref().filter(|s| !s.is_empty()),
            input.reason_text.as_deref().map(str::trim).filter(|s| !s.is_empty()),
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

/// 결근을 취소한다. 지우지 않고 상태만 바꾼다.
///
/// 이 결근으로 배정된 보결이 남아 있으면 알려 준다 (자동으로 지우지 않는다).
pub fn cancel_absence(conn: &Connection, id: i64) -> AppResult<i32> {
    let n = conn.execute(
        "UPDATE absences
            SET status = 'CANCELLED', cancelled_at = datetime('now','localtime')
          WHERE id = ?1 AND status = 'ACTIVE'",
        [id],
    )?;
    if n == 0 {
        return Err(AppError::not_found(
            "취소할 결근 기록을 찾을 수 없습니다. 이미 취소되었을 수 있습니다.",
        ));
    }
    let left: i32 = conn.query_row(
        "SELECT COUNT(*) FROM substitutions WHERE absence_id = ?1 AND status = 'ASSIGNED'",
        [id],
        |r| r.get(0),
    )?;
    Ok(left)
}

/// 그 날의 결근 목록. `all=true`면 취소된 것도 함께 돌려준다.
pub fn absences_on(conn: &Connection, date: &str, all: bool) -> AppResult<Vec<AbsenceRow>> {
    repo_find::parse_date(date)?;
    let sql = "SELECT a.id, a.date, a.teacher_id, t.name, r0.label,
                      a.is_all_day, a.start_min, a.end_min,
                      a.reason_code, a.reason_text, r.label,
                      a.status, a.created_at, a.cancelled_at,
                      (SELECT COUNT(*) FROM substitutions s
                        WHERE s.absence_id = a.id AND s.status = 'ASSIGNED')
                 FROM absences a
                 JOIN teachers t       ON t.id = a.teacher_id
                 JOIN teacher_roles r0 ON r0.code = t.role_code
            LEFT JOIN absence_reasons r ON r.code = a.reason_code
                WHERE a.date = ?1 AND (?2 = 1 OR a.status = 'ACTIVE')
             ORDER BY a.status, a.is_all_day DESC, a.start_min, t.name";
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt
        .query_map(params![date, if all { 1 } else { 0 }], |r| {
            let code: Option<String> = r.get(8)?;
            let text: Option<String> = r.get(9)?;
            let label: Option<String> = r.get(10)?;
            Ok(AbsenceRow {
                id: r.get(0)?,
                date: r.get(1)?,
                teacher_id: r.get(2)?,
                teacher_name: r.get(3)?,
                role_label: r.get(4)?,
                is_all_day: r.get::<_, i64>(5)? != 0,
                start_min: r.get(6)?,
                end_min: r.get(7)?,
                reason_label: label
                    .or_else(|| text.clone())
                    .unwrap_or_else(|| "사유 없음".to_string()),
                reason_code: code,
                reason_text: text,
                status: r.get(11)?,
                created_at: r.get(12)?,
                cancelled_at: r.get(13)?,
                sub_count: r.get(14)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// 그 교사·그 날의 활성 결근 (일괄 보결에서 시간 구간과 사유를 가져온다)
fn active_absence(conn: &Connection, teacher_id: i64, date: &str) -> AppResult<Option<AbsenceRow>> {
    Ok(absences_on(conn, date, false)?
        .into_iter()
        .find(|a| a.teacher_id == teacher_id))
}

// ============================================================
//  배정
// ============================================================

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssignInput {
    pub date: String,
    pub class_id: i64,
    /// PERIOD | LUNCH
    pub slot_type: String,
    #[serde(default)]
    pub period_no: Option<i32>,
    #[serde(default)]
    pub absent_teacher_id: Option<i64>,
    pub sub_teacher_id: i64,
    /// 결근 기록과 묶을 때
    #[serde(default)]
    pub absence_id: Option<i64>,
    #[serde(default)]
    pub reason_code: Option<String>,
    #[serde(default)]
    pub reason_text: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssignSaved {
    pub id: i64,
    pub date: String,
    pub class_label: String,
    /// '5학년 가람반' — 저장 확인 문구에 쓴다
    pub class_full_label: String,
    pub slot_label: String,
    pub start_min: i32,
    pub end_min: i32,
    pub sub_teacher_name: String,
    pub absent_teacher_name: Option<String>,
    /// 저장은 되었지만 확인하면 좋은 내용 (전담 시간 등)
    pub notice: Option<SlotNotice>,
}

fn map_assign_error(e: assign::AssignError) -> AppError {
    let code = match &e {
        assign::AssignError::Slot(_) => "SLOT_CHANGED",
        assign::AssignError::UnknownTeacher => "NOT_FOUND",
        assign::AssignError::SameAsAbsent { .. } => "INVALID_INPUT",
        assign::AssignError::NotEligible { .. } => "RECHECK_FAILED",
    };
    AppError::new(code, assign::error_message(&e))
}

/// 하나의 배정을 확정한다. **호출자는 반드시 쓰기 트랜잭션 안에서 부른다.**
pub fn assign_one(conn: &Connection, input: &AssignInput) -> AppResult<AssignSaved> {
    if !matches!(input.slot_type.as_str(), "PERIOD" | "LUNCH") {
        return Err(AppError::invalid("보결 시간을 선택해 주세요."));
    }

    let mut snap = repo_find::snapshot(conn, &input.date)?;
    let counts = repo_find::counts(conn, &input.date)?;
    let settings = repo_priority::settings(conn)?;

    let plan = AssignPlan {
        class_id: input.class_id,
        slot_type: input.slot_type.clone(),
        period_no: input.period_no,
        sub_teacher_id: input.sub_teacher_id,
    };
    // 저장 직전 재검증 — 조회 이후 상황이 바뀌었으면 여기서 걸린다
    let row = assign::verify(
        &snap,
        &counts,
        &settings,
        input.absent_teacher_id,
        &plan,
    )
    .map_err(map_assign_error)?;

    // 같은 교사·겹치는 시간이 이미 있는지 마지막으로 한 번 더 본다.
    // (verify가 이미 걸러 주지만, 저장 경로를 통과하는 모든 건에 대해 확인한다)
    guard_overlap(conn, &input.date, &row)?;

    let id = insert_row(conn, &snap, input.absence_id, input, &row)?;
    assign::occupy(&mut snap, &row);

    Ok(AssignSaved {
        id,
        date: input.date.clone(),
        class_label: row.class_label.clone(),
        class_full_label: row.class_full_label.clone(),
        slot_label: row.slot_label.clone(),
        start_min: row.start_min,
        end_min: row.end_min,
        sub_teacher_name: row.sub_teacher_name.clone(),
        absent_teacher_name: row.absent_teacher_name.clone(),
        notice: row.notice.clone(),
    })
}

/// 같은 교사가 실제로 겹치는 시간에 두 번 배정되지 않도록 마지막으로 확인한다.
///
/// SQLite에는 구간 겹침을 막는 제약을 만들 수 없다 (시작 시각이 같은 경우만
/// 유니크 인덱스로 막힌다). 그래서 트랜잭션 안에서 직접 확인한다.
fn guard_overlap(conn: &Connection, date: &str, row: &AssignRow) -> AppResult<()> {
    let target = row.interval();
    let mut stmt = conn.prepare(
        "SELECT COALESCE(class_label, grade || '-' || class_no), slot_label, start_min, end_min
           FROM substitutions
          WHERE date = ?1 AND status = 'ASSIGNED' AND sub_teacher_id = ?2
            AND start_min < ?3 AND ?4 < end_min",
    )?;
    let hit: Option<(String, String, i32, i32)> = stmt
        .query_row(
            params![date, row.sub_teacher_id, target.end, target.start],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()?;

    if let Some((cls, slot, s, e)) = hit {
        return Err(AppError::new(
            "RECHECK_FAILED",
            format!(
                "{} 선생님은 이미 {} {}({}~{})에 보결이 배정되어 있어 시간이 겹칩니다.",
                row.sub_teacher_name,
                cls,
                slot,
                crate::domain::time::fmt_min(s),
                crate::domain::time::fmt_min(e)
            ),
        ));
    }

    // 같은 학급 같은 시간에 이미 보결이 있는 경우
    let mut stmt = conn.prepare(
        "SELECT sub_teacher_id FROM substitutions
          WHERE date = ?1 AND status = 'ASSIGNED' AND class_id = ?2
            AND start_min < ?3 AND ?4 < end_min",
    )?;
    let taken: Option<i64> = stmt
        .query_row(params![date, row.class_id, target.end, target.start], |r| {
            r.get(0)
        })
        .optional()?;
    if let Some(other) = taken {
        let who: String = conn
            .query_row("SELECT name FROM teachers WHERE id = ?1", [other], |r| {
                r.get(0)
            })
            .unwrap_or_else(|_| "다른 선생님".to_string());
        return Err(AppError::new(
            "RECHECK_FAILED",
            format!(
                "{} {}에는 이미 {} 선생님이 보결로 배정되어 있습니다. \
                 다시 배정하려면 기존 배정을 먼저 취소해 주세요.",
                row.class_label, row.slot_label, who
            ),
        ));
    }
    Ok(())
}

fn insert_row(
    conn: &Connection,
    snap: &DaySnapshot,
    absence_id: Option<i64>,
    input: &AssignInput,
    row: &AssignRow,
) -> AppResult<i64> {
    let term_id = school::current_term_id(conn)?;
    let subject_id: Option<i64> = match &row.subject_name {
        Some(name) => conn
            .query_row("SELECT id FROM subjects WHERE name = ?1", [name], |r| {
                r.get(0)
            })
            .optional()?,
        None => None,
    };

    // 사유는 결근 기록에서 가져오는 것을 기본으로 한다
    let (reason_code, reason_text) = match (&input.reason_code, &input.reason_text) {
        (None, None) => match absence_id {
            Some(aid) => conn
                .query_row(
                    "SELECT reason_code, reason_text FROM absences WHERE id = ?1",
                    [aid],
                    |r| Ok((r.get::<_, Option<String>>(0)?, r.get::<_, Option<String>>(1)?)),
                )
                .optional()?
                .unwrap_or((None, None)),
            None => (None, None),
        },
        (c, t) => (c.clone(), t.clone()),
    };

    let _ = snap; // 스냅샷 값은 이미 row 에 담겨 있다

    conn.execute(
        "INSERT INTO substitutions
            (term_id, absence_id, date, day_of_week,
             class_id, grade, class_no, class_label,
             slot_type, period_no, slot_label, start_min, end_min,
             absent_teacher_id, sub_teacher_id, subject_id,
             reason_code, reason_text, status,
             recommend_rank, recommend_reason)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
                 ?14, ?15, ?16, ?17, ?18, 'ASSIGNED', ?19, ?20)",
        params![
            term_id,
            absence_id,
            input.date.trim(),
            row.day_of_week,
            row.class_id,
            row.grade,
            row.class_no,
            row.class_label,
            row.slot_type,
            row.period_no,
            row.slot_label,
            row.start_min,
            row.end_min,
            row.absent_teacher_id,
            row.sub_teacher_id,
            subject_id,
            reason_code,
            reason_text,
            row.recommend_rank,
            row.recommend_reason,
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

// ============================================================
//  하루 일괄 보결
// ============================================================

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanCandidate {
    pub teacher_id: i64,
    pub name: String,
    pub role_label: String,
    pub duty: String,
    pub rank: i32,
    pub reason: String,
    pub counts: SubCounts,
}

impl From<&Candidate> for PlanCandidate {
    fn from(c: &Candidate) -> Self {
        Self {
            teacher_id: c.teacher_id,
            name: c.name.clone(),
            role_label: c.role_label.clone(),
            duty: c.duty.clone(),
            rank: c.rank,
            reason: c.reason.clone(),
            counts: c.counts,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanSlot {
    pub class_id: i64,
    pub class_label: String,
    pub slot_type: String,
    pub period_no: Option<i32>,
    pub slot_label: String,
    pub start_min: i32,
    pub end_min: i32,
    /// LESSON | HOMEROOM_LESSON | LUNCH | RECESS
    pub kind: String,
    pub kind_label: String,
    pub subject_name: Option<String>,
    /// 추천 후보 (앞에서부터 추천 순서)
    pub candidates: Vec<PlanCandidate>,
    /// 이미 배정되어 있는 경우
    pub existing_sub_id: Option<i64>,
    pub existing_sub_name: Option<String>,
    /// 확인하면 좋은 내용 (전담 시간 등)
    pub notice: Option<SlotNotice>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DayPlan {
    pub date: String,
    pub day_of_week: i32,
    pub teacher_id: i64,
    pub teacher_name: String,
    pub absence_id: Option<i64>,
    pub absence_label: Option<String>,
    pub slots: Vec<PlanSlot>,
    /// 전체를 훑을 때 알려 줄 것
    pub warnings: Vec<String>,
}

/// 결근 교사의 그 날 일정을 뽑고 시간대마다 추천 후보를 붙인다.
pub fn day_plan(conn: &Connection, date: &str, teacher_id: i64) -> AppResult<DayPlan> {
    let snap = repo_find::snapshot(conn, date)?;
    let counts = repo_find::counts(conn, date)?;
    let settings = repo_priority::settings(conn)?;

    let teacher_name = snap
        .teachers
        .iter()
        .find(|t| t.id == teacher_id)
        .map(|t| t.name.clone())
        .ok_or_else(|| AppError::not_found("선생님을 찾을 수 없습니다."))?;

    let absence = active_absence(conn, teacher_id, date)?;
    let window: Option<Interval> = absence.as_ref().and_then(|a| {
        (!a.is_all_day)
            .then(|| Interval::new(a.start_min.unwrap_or(0), a.end_min.unwrap_or(1440)))
    });

    let duties: Vec<DutySlot> = assign::duty_slots(&snap, teacher_id, window);

    // 이미 배정된 보결 (이 날, 활성) — 시작 시각 + 학급으로 찾는다
    let mut existing: HashMap<(i64, i32), (i64, String)> = HashMap::new();
    {
        let mut stmt = conn.prepare(
            "SELECT s.class_id, s.start_min, s.id, t.name
               FROM substitutions s JOIN teachers t ON t.id = s.sub_teacher_id
              WHERE s.date = ?1 AND s.status = 'ASSIGNED'",
        )?;
        for row in stmt.query_map([date], |r| {
            Ok((
                r.get::<_, Option<i64>>(0)?,
                r.get::<_, i32>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, String>(3)?,
            ))
        })? {
            let (cid, start, id, name) = row?;
            if let Some(cid) = cid {
                existing.insert((cid, start), (id, name));
            }
        }
    }

    let mut slots: Vec<PlanSlot> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();

    for d in &duties {
        let req = FindRequest {
            class_id: d.class_id,
            slot_type: d.slot_type.clone(),
            period_no: d.period_no,
            absent_teacher_id: Some(teacher_id),
        };
        let (candidates, notice) = match crate::domain::find::find_candidates(&snap, &req, &counts)
        {
            Ok(mut r) => {
                rank_candidates(&mut r.eligible, &settings, r.slot.grade);
                if r.eligible.is_empty() {
                    warnings.push(format!(
                        "{} {}에 배정할 수 있는 선생님이 없습니다.",
                        d.class_label, d.slot_label
                    ));
                }
                (
                    r.eligible.iter().map(PlanCandidate::from).collect(),
                    r.notice,
                )
            }
            Err(_) => {
                warnings.push(format!(
                    "{} {}는 시정표에서 찾을 수 없어 후보를 계산하지 못했습니다.",
                    d.class_label, d.slot_label
                ));
                (Vec::new(), None)
            }
        };

        let found = existing.get(&(d.class_id, d.start_min));
        slots.push(PlanSlot {
            class_id: d.class_id,
            class_label: d.class_label.clone(),
            slot_type: d.slot_type.clone(),
            period_no: d.period_no,
            slot_label: d.slot_label.clone(),
            start_min: d.start_min,
            end_min: d.end_min,
            kind: d.kind.clone(),
            kind_label: assign::duty_kind_label(&d.kind).to_string(),
            subject_name: d.subject_name.clone(),
            candidates,
            existing_sub_id: found.map(|f| f.0),
            existing_sub_name: found.map(|f| f.1.clone()),
            notice,
        });
    }

    if duties.is_empty() {
        warnings.push(format!(
            "{teacher_name} 선생님은 이 날 보결이 필요한 시간이 없습니다."
        ));
    }

    Ok(DayPlan {
        date: date.to_string(),
        day_of_week: snap.day_of_week,
        teacher_id,
        teacher_name,
        absence_id: absence.as_ref().map(|a| a.id),
        absence_label: absence.as_ref().map(|a| match (a.is_all_day, a.start_min, a.end_min) {
            (true, _, _) => format!("{} (종일)", a.reason_label),
            (false, Some(s), Some(e)) => format!(
                "{} {}~{}",
                a.reason_label,
                crate::domain::time::fmt_min(s),
                crate::domain::time::fmt_min(e)
            ),
            _ => a.reason_label.clone(),
        }),
        slots,
        warnings,
    })
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchPick {
    pub class_id: i64,
    pub slot_type: String,
    #[serde(default)]
    pub period_no: Option<i32>,
    pub sub_teacher_id: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchInput {
    pub date: String,
    pub absent_teacher_id: i64,
    #[serde(default)]
    pub absence_id: Option<i64>,
    pub picks: Vec<BatchPick>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchSaved {
    pub saved: Vec<AssignSaved>,
}

/// 하루 일괄 보결을 확정한다. **전체 성공 또는 전체 실패.**
///
/// 호출자가 쓰기 트랜잭션 안에서 부르므로, 중간에 하나라도 막히면 앞의 것까지
/// 모두 되돌아간다. 검증은 한 칸씩 순서대로 하며, 앞 칸의 결과를 하루치 자료에
/// 반영한 뒤 다음 칸을 본다 — 그래서 같은 시간 두 반에 같은 교사를 넣으면 걸린다.
pub fn assign_batch(conn: &Connection, input: &BatchInput) -> AppResult<BatchSaved> {
    if input.picks.is_empty() {
        return Err(AppError::invalid(
            "배정할 시간을 하나 이상 선택해 주세요. 미배정으로 두려면 [닫기]를 눌러 주세요.",
        ));
    }

    let mut snap = repo_find::snapshot(conn, &input.date)?;
    let counts = repo_find::counts(conn, &input.date)?;
    let settings = repo_priority::settings(conn)?;

    let absence_id = match input.absence_id {
        Some(id) => Some(id),
        None => active_absence(conn, input.absent_teacher_id, &input.date)?.map(|a| a.id),
    };

    let mut saved: Vec<AssignSaved> = Vec::new();

    for pick in &input.picks {
        if !matches!(pick.slot_type.as_str(), "PERIOD" | "LUNCH") {
            return Err(AppError::invalid("보결 시간이 올바르지 않습니다."));
        }
        let plan = AssignPlan {
            class_id: pick.class_id,
            slot_type: pick.slot_type.clone(),
            period_no: pick.period_no,
            sub_teacher_id: pick.sub_teacher_id,
        };
        let row = assign::verify(
            &snap,
            &counts,
            &settings,
            Some(input.absent_teacher_id),
            &plan,
        )
        .map_err(|e| {
            let base = map_assign_error(e);
            AppError::new(
                &base.code,
                format!(
                    "{}\n\n일괄 배정은 전체가 함께 저장되므로 아무것도 저장하지 않았습니다. \
                     이 시간의 선생님을 바꾼 뒤 다시 저장해 주세요.",
                    base.user_message
                ),
            )
        })?;

        guard_overlap(conn, &input.date, &row)?;

        let one = AssignInput {
            date: input.date.clone(),
            class_id: pick.class_id,
            slot_type: pick.slot_type.clone(),
            period_no: pick.period_no,
            absent_teacher_id: Some(input.absent_teacher_id),
            sub_teacher_id: pick.sub_teacher_id,
            absence_id,
            reason_code: None,
            reason_text: None,
        };
        let id = insert_row(conn, &snap, absence_id, &one, &row)?;

        // 앞 칸의 결과가 뒤 칸 검증에 보이도록 반영한다
        assign::occupy(&mut snap, &row);

        saved.push(AssignSaved {
            id,
            date: input.date.clone(),
            class_label: row.class_label.clone(),
            class_full_label: row.class_full_label.clone(),
            slot_label: row.slot_label.clone(),
            start_min: row.start_min,
            end_min: row.end_min,
            sub_teacher_name: row.sub_teacher_name.clone(),
            absent_teacher_name: row.absent_teacher_name.clone(),
            notice: row.notice.clone(),
        });
    }

    Ok(BatchSaved { saved })
}

// ============================================================
//  취소 · 배정 내역
// ============================================================

/// 배정을 취소한다. **지우지 않고 상태만 바꾼다.**
pub fn cancel(conn: &Connection, id: i64, reason: Option<&str>) -> AppResult<()> {
    let reason = reason.map(str::trim).filter(|s| !s.is_empty());
    let n = conn.execute(
        "UPDATE substitutions
            SET status = 'CANCELLED',
                cancelled_at = datetime('now','localtime'),
                cancel_reason = ?2
          WHERE id = ?1 AND status = 'ASSIGNED'",
        params![id, reason],
    )?;
    if n == 0 {
        return Err(AppError::not_found(
            "취소할 배정을 찾을 수 없습니다. 이미 취소되었을 수 있습니다.",
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryFilter {
    #[serde(default)]
    pub from: Option<String>,
    #[serde(default)]
    pub to: Option<String>,
    /// 교사 이름 일부 (결근 교사·배정 교사 모두에서 찾는다)
    #[serde(default)]
    pub keyword: Option<String>,
    /// ASSIGNED | CANCELLED | ALL
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub limit: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryRow {
    pub id: i64,
    pub date: String,
    pub day_of_week: i32,
    /// 기록 당시의 표기 ('5-가람'). 반 이름이 바뀌어도 그대로다
    pub class_label: String,
    pub grade: i32,
    pub slot_type: String,
    pub slot_label: String,
    pub start_min: i32,
    pub end_min: i32,
    pub absent_teacher_id: Option<i64>,
    pub absent_teacher_name: Option<String>,
    pub sub_teacher_id: i64,
    pub sub_teacher_name: String,
    pub subject_name: Option<String>,
    pub reason_label: Option<String>,
    pub status: String,
    pub recommend_rank: Option<i32>,
    pub recommend_reason: Option<String>,
    pub created_at: String,
    pub cancelled_at: Option<String>,
    pub cancel_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryView {
    pub rows: Vec<HistoryRow>,
    pub assigned_count: i32,
    pub cancelled_count: i32,
    /// limit 때문에 잘렸는가
    pub truncated: bool,
}

pub fn history(conn: &Connection, f: &HistoryFilter) -> AppResult<HistoryView> {
    if let Some(d) = f.from.as_deref().filter(|s| !s.is_empty()) {
        repo_find::parse_date(d)?;
    }
    if let Some(d) = f.to.as_deref().filter(|s| !s.is_empty()) {
        repo_find::parse_date(d)?;
    }
    let limit = f.limit.unwrap_or(500).clamp(1, 5000);
    let status = f.status.as_deref().unwrap_or("ALL");
    let keyword = f
        .keyword
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| format!("%{s}%"));

    let sql = "SELECT s.id, s.date, s.day_of_week,
                      COALESCE(s.class_label, s.grade || '-' || s.class_no),
                      s.grade, s.slot_type, s.slot_label, s.start_min, s.end_min,
                      s.absent_teacher_id, ta.name,
                      s.sub_teacher_id, ts.name,
                      sj.name,
                      COALESCE(ar.label, s.reason_text),
                      s.status, s.recommend_rank, s.recommend_reason,
                      s.created_at, s.cancelled_at, s.cancel_reason
                 FROM substitutions s
                 JOIN teachers ts ON ts.id = s.sub_teacher_id
            LEFT JOIN teachers ta ON ta.id = s.absent_teacher_id
            LEFT JOIN subjects sj ON sj.id = s.subject_id
            LEFT JOIN absence_reasons ar ON ar.code = s.reason_code
                WHERE (?1 IS NULL OR s.date >= ?1)
                  AND (?2 IS NULL OR s.date <= ?2)
                  AND (?3 = 'ALL' OR s.status = ?3)
                  AND (?4 IS NULL OR ts.name LIKE ?4 OR ta.name LIKE ?4)
             ORDER BY s.date DESC, s.start_min, s.grade, s.class_no
                LIMIT ?5";

    let mut stmt = conn.prepare(sql)?;
    let rows: Vec<HistoryRow> = stmt
        .query_map(
            params![
                f.from.as_deref().filter(|s| !s.is_empty()),
                f.to.as_deref().filter(|s| !s.is_empty()),
                status,
                keyword,
                limit + 1
            ],
            |r| {
                Ok(HistoryRow {
                    id: r.get(0)?,
                    date: r.get(1)?,
                    day_of_week: r.get(2)?,
                    class_label: r.get(3)?,
                    grade: r.get(4)?,
                    slot_type: r.get(5)?,
                    slot_label: r.get(6)?,
                    start_min: r.get(7)?,
                    end_min: r.get(8)?,
                    absent_teacher_id: r.get(9)?,
                    absent_teacher_name: r.get(10)?,
                    sub_teacher_id: r.get(11)?,
                    sub_teacher_name: r.get(12)?,
                    subject_name: r.get(13)?,
                    reason_label: r.get(14)?,
                    status: r.get(15)?,
                    recommend_rank: r.get(16)?,
                    recommend_reason: r.get(17)?,
                    created_at: r.get(18)?,
                    cancelled_at: r.get(19)?,
                    cancel_reason: r.get(20)?,
                })
            },
        )?
        .collect::<rusqlite::Result<_>>()?;

    let truncated = rows.len() as i64 > limit;
    let rows: Vec<HistoryRow> = rows.into_iter().take(limit as usize).collect();
    let assigned_count = rows.iter().filter(|r| r.status == "ASSIGNED").count() as i32;
    let cancelled_count = rows.len() as i32 - assigned_count;

    Ok(HistoryView {
        rows,
        assigned_count,
        cancelled_count,
        truncated,
    })
}

#[cfg(test)]
#[path = "assign_tests.rs"]
mod tests;
