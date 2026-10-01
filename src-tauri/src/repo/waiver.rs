//! 보결 불필요 — 결근으로 생긴 보결 필요 교시 가운데 일정이 바뀌어
//! 더 이상 사람을 넣지 않아도 되는 칸을 기록한다.
//!
//! ## 배정 취소와 다르다
//!
//! ```text
//! 배정 취소    사람을 넣었다가 그 배정을 물렸다
//! 보결 불필요  애초에 아무도 넣을 필요가 없어졌다
//! ```
//!
//! 그래서 `substitutions` 를 빌려 쓰지 않고 표를 따로 둔다. 저쪽은
//! `sub_teacher_id` 가 NOT NULL 이라 '대신 들어가는 사람이 없는' 기록을
//! 담을 수도 없다.
//!
//! ## 칸을 가리키는 열쇠
//!
//! `(date, class_id, start_min)`. 보결 현황과 다건 배정이 이미 배정 여부를
//! 맞출 때 쓰는 것과 같다. 한 곳만 보므로 두 화면이 어긋나지 않는다.
//!
//! ## 지우지 않는다
//!
//! 되돌리기는 행을 지우는 것이 아니라 `status` 를 `REVOKED` 로 바꾸는
//! 것이다. 무엇을 왜 했는지가 남는다.

use std::collections::HashSet;

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use super::{find as repo_find, school};
use crate::domain::assign::duty_slots;
use crate::domain::time::Interval;
use crate::error::{AppError, AppResult};

pub const STATUS_ACTIVE: &str = "ACTIVE";
pub const STATUS_REVOKED: &str = "REVOKED";

/// 왜 보결이 필요 없어졌나. 학교에서 거의 이 둘뿐이라 둘만 둔다.
pub const REASON_SPECIAL_CHANGED: &str = "SPECIAL_CHANGED";
pub const REASON_OTHER: &str = "OTHER";

pub fn reason_label(code: &str) -> &'static str {
    match code {
        REASON_SPECIAL_CHANGED => "전담시간 변경",
        REASON_OTHER => "기타",
        _ => "사유 미상",
    }
}

fn is_known_reason(code: &str) -> bool {
    matches!(code, REASON_SPECIAL_CHANGED | REASON_OTHER)
}

/// 화면에 보여 줄 보결 불필요 한 건.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WaiverView {
    pub id: i64,
    pub reason_code: String,
    /// '전담시간 변경'
    pub reason_label: String,
    pub note: Option<String>,
    pub created_at: String,
}

/// 보결 불필요로 처리할 칸.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WaiverInput {
    pub date: String,
    pub class_id: i64,
    /// PERIOD | LUNCH
    pub slot_type: String,
    pub period_no: Option<i32>,
    /// 누구의 보결이었나
    pub absent_teacher_id: i64,
    pub reason_code: String,
    pub note: Option<String>,
}

/// 그 날 살아 있는 보결 불필요 칸들. `(class_id, start_min)`
pub fn active_slots(conn: &Connection, date: &str) -> AppResult<HashSet<(i64, i32)>> {
    let mut stmt = conn.prepare(
        "SELECT class_id, start_min FROM substitution_waivers
          WHERE status = ?1 AND date = ?2",
    )?;
    let mut out = HashSet::new();
    for row in stmt.query_map(params![STATUS_ACTIVE, date], |r| {
        Ok((r.get::<_, Option<i64>>(0)?, r.get::<_, i32>(1)?))
    })? {
        let (cid, start) = row?;
        if let Some(cid) = cid {
            out.insert((cid, start));
        }
    }
    Ok(out)
}

/// 여러 날짜치를 한 번에. `(date, class_id, start_min)` — 보결 현황이 쓴다.
pub fn active_slots_in(conn: &Connection, dates: &[String]) -> AppResult<HashSet<(String, i64, i32)>> {
    if dates.is_empty() {
        return Ok(HashSet::new());
    }
    let marks = vec!["?"; dates.len()].join(",");
    let sql = format!(
        "SELECT date, class_id, start_min FROM substitution_waivers
          WHERE status = '{STATUS_ACTIVE}' AND date IN ({marks})"
    );
    let mut stmt = conn.prepare(&sql)?;
    let mut out = HashSet::new();
    for row in stmt.query_map(rusqlite::params_from_iter(dates.iter()), |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, Option<i64>>(1)?,
            r.get::<_, i32>(2)?,
        ))
    })? {
        let (d, cid, start) = row?;
        if let Some(cid) = cid {
            out.insert((d, cid, start));
        }
    }
    Ok(out)
}

/// 그 날 살아 있는 보결 불필요를 칸별로. 다건 배정 화면이 사유를 보여 준다.
pub fn views_for_date(
    conn: &Connection,
    date: &str,
) -> AppResult<std::collections::HashMap<(i64, i32), WaiverView>> {
    let mut stmt = conn.prepare(
        "SELECT id, class_id, start_min, reason_code, note, created_at
           FROM substitution_waivers
          WHERE status = ?1 AND date = ?2",
    )?;
    let mut out = std::collections::HashMap::new();
    for row in stmt.query_map(params![STATUS_ACTIVE, date], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, Option<i64>>(1)?,
            r.get::<_, i32>(2)?,
            r.get::<_, String>(3)?,
            r.get::<_, Option<String>>(4)?,
            r.get::<_, String>(5)?,
        ))
    })? {
        let (id, cid, start, code, note, created) = row?;
        let Some(cid) = cid else { continue };
        out.insert(
            (cid, start),
            WaiverView {
                id,
                reason_label: reason_label(&code).to_string(),
                reason_code: code,
                note,
                created_at: created,
            },
        );
    }
    Ok(out)
}

/// 보결 불필요로 처리한다.
///
/// 막는 경우가 둘 있다.
///  - 그 칸이 애초에 보결 필요 칸이 아니다 (결근에서 나온 칸이 아니다)
///  - 이미 사람을 배정해 두었다 — 먼저 배정을 취소해야 한다
///
/// 배정을 몰래 지우고 처리하지 않는다. 한 번의 버튼으로 둘을 함께 하면
/// 나중에 기록을 읽을 때 '취소한 것인지 필요 없어진 것인지'가 흐려진다.
pub fn waive(conn: &Connection, input: &WaiverInput) -> AppResult<i64> {
    if !is_known_reason(&input.reason_code) {
        return Err(AppError::invalid("보결 불필요 사유가 올바르지 않습니다."));
    }
    let note = input
        .note
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.chars().take(200).collect::<String>());

    let (_, day_of_week) = repo_find::parse_date(&input.date)?;
    let snap = repo_find::snapshot(conn, &input.date)?;

    // 결근 창(일부 시간 결근)까지 그대로 적용해 '보결 필요 칸'을 다시 뽑는다.
    // 다건 배정 화면이 칸을 만들 때 쓰는 것과 같은 계산이다.
    let (absence_id, window) = absence_of(conn, &input.date, input.absent_teacher_id)?;
    let duty = duty_slots(&snap, input.absent_teacher_id, window)
        .into_iter()
        .find(|d| {
            d.class_id == input.class_id
                && d.slot_type == input.slot_type
                && d.period_no == input.period_no
        })
        .ok_or_else(|| {
            AppError::invalid(
                "그 시간은 보결이 필요한 시간이 아닙니다. 화면을 새로 고친 뒤 다시 시도해 주세요.",
            )
        })?;

    // 이미 사람이 들어가 있으면 막는다
    let assigned: Option<String> = conn
        .query_row(
            "SELECT t.name FROM substitutions s JOIN teachers t ON t.id = s.sub_teacher_id
              WHERE s.status = 'ASSIGNED' AND s.date = ?1 AND s.class_id = ?2 AND s.start_min = ?3",
            params![input.date, input.class_id, duty.start_min],
            |r| r.get(0),
        )
        .optional()?;
    if let Some(name) = assigned {
        return Err(AppError::invalid(format!(
            "{} {}에는 이미 {name} 선생님이 배정되어 있습니다. 먼저 배정을 취소해 주세요.",
            duty.class_label, duty.slot_label
        )));
    }

    let cls = snap
        .classes
        .iter()
        .find(|c| c.id == input.class_id)
        .ok_or_else(|| AppError::invalid("학급을 찾을 수 없습니다."))?;
    let term_id = school::current_term_id(conn)?;

    conn.execute(
        "INSERT INTO substitution_waivers(
            term_id, absence_id, date, day_of_week,
            class_id, grade, class_no, class_label,
            slot_type, period_no, slot_label, start_min, end_min,
            absent_teacher_id, reason_code, note, status)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
        params![
            term_id,
            absence_id,
            input.date,
            day_of_week,
            cls.id,
            cls.grade,
            cls.class_no,
            cls.label,
            duty.slot_type,
            duty.period_no,
            duty.slot_label,
            duty.start_min,
            duty.end_min,
            input.absent_teacher_id,
            input.reason_code,
            note,
            STATUS_ACTIVE,
        ],
    )
    .map_err(|e| match e {
        rusqlite::Error::SqliteFailure(f, _) if f.code == rusqlite::ErrorCode::ConstraintViolation => {
            AppError::invalid("이미 보결 불필요로 처리된 시간입니다.")
        }
        other => other.into(),
    })?;

    Ok(conn.last_insert_rowid())
}

/// 다시 보결이 필요한 칸으로 되돌린다. 기록은 남는다.
pub fn revoke(conn: &Connection, id: i64) -> AppResult<()> {
    let n = conn.execute(
        "UPDATE substitution_waivers
            SET status = ?1, revoked_at = datetime('now','localtime')
          WHERE id = ?2 AND status = ?3",
        params![STATUS_REVOKED, id, STATUS_ACTIVE],
    )?;
    if n == 0 {
        return Err(AppError::invalid(
            "이미 되돌렸거나 찾을 수 없는 기록입니다. 화면을 새로 고쳐 주세요.",
        ));
    }
    Ok(())
}

/// 그 날 그 교사의 결근 기록 (있으면 묶어 둔다) 과 일부 시간 결근 구간.
fn absence_of(
    conn: &Connection,
    date: &str,
    teacher_id: i64,
) -> AppResult<(Option<i64>, Option<Interval>)> {
    let row: Option<(i64, bool, Option<i32>, Option<i32>)> = conn
        .query_row(
            "SELECT id, is_all_day, start_min, end_min FROM absences
              WHERE status = 'ACTIVE' AND date = ?1 AND teacher_id = ?2
              ORDER BY id LIMIT 1",
            params![date, teacher_id],
            |r| Ok((r.get(0)?, r.get::<_, i64>(1)? != 0, r.get(2)?, r.get(3)?)),
        )
        .optional()?;

    Ok(match row {
        Some((id, all_day, s, e)) => (
            Some(id),
            (!all_day).then(|| Interval::new(s.unwrap_or(0), e.unwrap_or(1440))),
        ),
        None => (None, None),
    })
}

#[cfg(test)]
#[path = "waiver_tests.rs"]
mod tests;
