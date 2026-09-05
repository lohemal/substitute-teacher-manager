//! Phase 10 — 학기 관리와 새 학기 시작.
//!
//! ## 지난 학기는 손대지 않는다
//!
//! 새 학기를 시작해도 지난 학기의 학급·시정표·수업·보결 기록은 그대로 남는다.
//! 새 학기는 `term_id`가 다른 **새 자료 묶음**을 만드는 일일 뿐이다.
//! 보결 기록은 학급 이름·시각을 스냅샷으로 갖고 있으므로, 새 학기에 반 이름이
//! 바뀌어도 과거 기록과 통계는 그대로다.
//!
//! ## 교사와 과목은 복사하지 않는다
//!
//! 두 자료는 학기가 아니라 **학교**에 속한다(`teachers`·`subjects`에 term_id가
//! 없다). 그래서 새 학기에도 그대로 이어지며, 복사할 것이 없다. 대신 담임
//! 배정은 학급에 붙어 있으므로 새 학기에 다시 지정해야 한다.

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::domain::period::{self, TermInfo};
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TermRow {
    pub id: i64,
    pub school_year: i32,
    pub semester: i32,
    pub name: String,
    /// 비어 있으면 학사 일정으로 계산한 값
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    /// 실제로 통계에 쓰이는 범위 (직접 넣은 값이 없으면 계산값)
    pub effective_from: String,
    pub effective_to: String,
    /// 날짜를 직접 넣었는가
    pub explicit: bool,
    pub is_current: bool,
    /// 이 학기에 딸린 자료
    pub class_count: i32,
    pub lesson_count: i32,
    pub sub_count: i32,
}

fn row_of(conn: &Connection, id: i64) -> AppResult<TermRow> {
    let (school_year, semester, name, start_date, end_date, is_current): (
        i32,
        i32,
        String,
        Option<String>,
        Option<String>,
        bool,
    ) = conn.query_row(
        "SELECT school_year, semester, name, start_date, end_date, is_current
           FROM terms WHERE id = ?1",
        [id],
        |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get::<_, i64>(5)? != 0,
            ))
        },
    )?;

    let info = TermInfo {
        school_year,
        semester,
        name: name.clone(),
        start: start_date.clone(),
        end: end_date.clone(),
    };
    let (from, to) = period::term_range(&info);

    let class_count: i32 = conn.query_row(
        "SELECT COUNT(*) FROM classes WHERE term_id = ?1 AND active = 1",
        [id],
        |r| r.get(0),
    )?;
    let lesson_count: i32 =
        conn.query_row("SELECT COUNT(*) FROM lessons WHERE term_id = ?1", [id], |r| {
            r.get(0)
        })?;
    let sub_count: i32 = conn.query_row(
        "SELECT COUNT(*) FROM substitutions WHERE term_id = ?1 AND status = 'ASSIGNED'",
        [id],
        |r| r.get(0),
    )?;

    Ok(TermRow {
        id,
        school_year,
        semester,
        name,
        explicit: start_date.is_some() && end_date.is_some(),
        start_date,
        end_date,
        effective_from: from.format("%Y-%m-%d").to_string(),
        effective_to: to.format("%Y-%m-%d").to_string(),
        is_current,
        class_count,
        lesson_count,
        sub_count,
    })
}

pub fn list(conn: &Connection) -> AppResult<Vec<TermRow>> {
    let ids: Vec<i64> = conn
        .prepare("SELECT id FROM terms ORDER BY school_year DESC, semester DESC")?
        .query_map([], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    ids.into_iter().map(|id| row_of(conn, id)).collect()
}

// ============================================================
//  학기 날짜 수정
// ============================================================

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TermDates {
    pub id: i64,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub start_date: Option<String>,
    #[serde(default)]
    pub end_date: Option<String>,
}

fn check_date(s: &str) -> AppResult<String> {
    chrono::NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d")
        .map(|d| d.format("%Y-%m-%d").to_string())
        .map_err(|_| AppError::invalid("날짜 형식이 올바르지 않습니다. 달력에서 골라 주세요."))
}

/// 학기 이름과 시작·종료일을 고친다. **보결 기록은 건드리지 않는다.**
pub fn save_dates(conn: &Connection, input: &TermDates) -> AppResult<()> {
    let start = match input.start_date.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(s) => Some(check_date(s)?),
        None => None,
    };
    let end = match input.end_date.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(s) => Some(check_date(s)?),
        None => None,
    };
    if let (Some(a), Some(b)) = (&start, &end) {
        if b < a {
            return Err(AppError::invalid("종료일이 시작일보다 뒤여야 합니다."));
        }
    }
    if start.is_some() != end.is_some() {
        return Err(AppError::invalid(
            "시작일과 종료일을 함께 넣어 주세요. 둘 다 비우면 학사 일정으로 계산합니다.",
        ));
    }

    let name = input.name.as_deref().map(str::trim).filter(|s| !s.is_empty());
    let n = conn.execute(
        "UPDATE terms
            SET name = COALESCE(?2, name), start_date = ?3, end_date = ?4
          WHERE id = ?1",
        params![input.id, name, start, end],
    )?;
    if n == 0 {
        return Err(AppError::not_found("학기를 찾을 수 없습니다."));
    }
    Ok(())
}

/// 현재 학기를 바꾼다. 하나만 현재일 수 있다.
pub fn set_current(conn: &Connection, id: i64) -> AppResult<()> {
    let exists: i64 = conn.query_row("SELECT COUNT(*) FROM terms WHERE id = ?1", [id], |r| {
        r.get(0)
    })?;
    if exists == 0 {
        return Err(AppError::not_found("학기를 찾을 수 없습니다."));
    }
    conn.execute("UPDATE terms SET is_current = 0", [])?;
    conn.execute("UPDATE terms SET is_current = 1 WHERE id = ?1", [id])?;
    Ok(())
}

// ============================================================
//  새 학기 시작
// ============================================================

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewTermInput {
    pub school_year: i32,
    pub semester: i32,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub start_date: Option<String>,
    #[serde(default)]
    pub end_date: Option<String>,
    /// 학급 구성(학년·반 수·반 이름)을 가져올지
    #[serde(default)]
    pub copy_classes: bool,
    /// 담임 배정까지 가져올지 (copy_classes 가 켜져 있을 때만)
    #[serde(default)]
    pub copy_homerooms: bool,
    /// 시정표를 가져올지
    #[serde(default)]
    pub copy_bells: bool,
    /// 전담 시간표를 가져올지 (학급·시정표를 함께 가져올 때만)
    #[serde(default)]
    pub copy_lessons: bool,
    /// 새 학기를 바로 현재 학기로 삼을지
    #[serde(default = "yes")]
    pub set_current: bool,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NewTermResult {
    pub term_id: i64,
    pub name: String,
    pub copied_classes: i32,
    pub copied_homerooms: i32,
    pub copied_bells: i32,
    pub copied_lessons: i32,
    /// 다음에 무엇을 해야 하는지
    pub next_steps: Vec<String>,
}

/// 새 학기를 만든다. **지난 학기 자료는 그대로 둔다.**
pub fn start_new(conn: &Connection, input: &NewTermInput) -> AppResult<NewTermResult> {
    if !(2000..=2100).contains(&input.school_year) {
        return Err(AppError::invalid("학년도를 확인해 주세요."));
    }
    if !(1..=2).contains(&input.semester) {
        return Err(AppError::invalid("학기는 1 또는 2로 골라 주세요."));
    }

    let dup: i64 = conn.query_row(
        "SELECT COUNT(*) FROM terms WHERE school_year = ?1 AND semester = ?2",
        params![input.school_year, input.semester],
        |r| r.get(0),
    )?;
    if dup > 0 {
        return Err(AppError::new(
            "DUPLICATE",
            format!(
                "{}학년도 {}학기는 이미 있습니다. 학기 목록에서 고르거나 날짜를 고쳐 주세요.",
                input.school_year, input.semester
            ),
        ));
    }

    let name = input
        .name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| format!("{}학년도 {}학기", input.school_year, input.semester));

    let start = match input.start_date.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(s) => Some(check_date(s)?),
        None => None,
    };
    let end = match input.end_date.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(s) => Some(check_date(s)?),
        None => None,
    };
    if let (Some(a), Some(b)) = (&start, &end) {
        if b < a {
            return Err(AppError::invalid("종료일이 시작일보다 뒤여야 합니다."));
        }
    }

    let from_term: Option<i64> = conn
        .query_row(
            "SELECT id FROM terms ORDER BY is_current DESC, school_year DESC, semester DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .optional()?;

    conn.execute(
        "INSERT INTO terms(school_year, semester, name, start_date, end_date, is_current)
         VALUES (?1, ?2, ?3, ?4, ?5, 0)",
        params![input.school_year, input.semester, name, start, end],
    )?;
    let new_id = conn.last_insert_rowid();

    let mut out = NewTermResult {
        term_id: new_id,
        name: name.clone(),
        copied_classes: 0,
        copied_homerooms: 0,
        copied_bells: 0,
        copied_lessons: 0,
        next_steps: Vec::new(),
    };

    if let Some(old) = from_term {
        // ---------- 시정표 ----------
        // bell_schedules -> bell_slots -> grade_bell_map 순으로 옮긴다
        let mut bell_map: std::collections::HashMap<i64, i64> = std::collections::HashMap::new();
        if input.copy_bells {
            let olds: Vec<(i64, String)> = conn
                .prepare("SELECT id, name FROM bell_schedules WHERE term_id = ?1 ORDER BY id")?
                .query_map([old], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect::<rusqlite::Result<_>>()?;

            for (old_id, bname) in olds {
                conn.execute(
                    "INSERT INTO bell_schedules(term_id, name) VALUES (?1, ?2)",
                    params![new_id, bname],
                )?;
                let nid = conn.last_insert_rowid();
                bell_map.insert(old_id, nid);
                conn.execute(
                    "INSERT INTO bell_slots
                        (bell_schedule_id, day_of_week, slot_type, period_no, label, start_min, end_min)
                     SELECT ?1, day_of_week, slot_type, period_no, label, start_min, end_min
                       FROM bell_slots WHERE bell_schedule_id = ?2",
                    params![nid, old_id],
                )?;
                out.copied_bells += 1;
            }

            for (old_id, nid) in &bell_map {
                conn.execute(
                    "INSERT INTO grade_bell_map(term_id, grade, bell_schedule_id)
                     SELECT ?1, grade, ?2 FROM grade_bell_map
                      WHERE term_id = ?3 AND bell_schedule_id = ?4",
                    params![new_id, nid, old, old_id],
                )?;
            }
        }

        // ---------- 학급 ----------
        let mut class_map: std::collections::HashMap<i64, i64> = std::collections::HashMap::new();
        if input.copy_classes {
            let olds: Vec<(i64, i32, i32, Option<String>, Option<i64>)> = conn
                .prepare(
                    "SELECT id, grade, class_no, name, homeroom_teacher_id FROM classes
                      WHERE term_id = ?1 AND active = 1 ORDER BY grade, class_no",
                )?
                .query_map([old], |r| {
                    Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
                })?
                .collect::<rusqlite::Result<_>>()?;

            for (old_id, grade, class_no, cname, hr) in olds {
                let keep_hr = if input.copy_homerooms { hr } else { None };
                conn.execute(
                    "INSERT INTO classes(term_id, grade, class_no, name, homeroom_teacher_id, active)
                     VALUES (?1, ?2, ?3, ?4, ?5, 1)",
                    params![new_id, grade, class_no, cname, keep_hr],
                )?;
                class_map.insert(old_id, conn.last_insert_rowid());
                out.copied_classes += 1;
                if keep_hr.is_some() {
                    out.copied_homerooms += 1;
                }
            }
        }

        // ---------- 전담 시간표 ----------
        // 학급과 시정표를 함께 가져왔을 때만 뜻이 있다
        if input.copy_lessons && input.copy_classes && input.copy_bells {
            let olds: Vec<(i64, i64, Option<i64>, i32, i32, String, i64)> = conn
                .prepare(
                    "SELECT teacher_id, class_id, subject_id, day_of_week, period_no,
                            lesson_type, replaces_homeroom
                       FROM lessons WHERE term_id = ?1",
                )?
                .query_map([old], |r| {
                    Ok((
                        r.get(0)?,
                        r.get(1)?,
                        r.get(2)?,
                        r.get(3)?,
                        r.get(4)?,
                        r.get(5)?,
                        r.get(6)?,
                    ))
                })?
                .collect::<rusqlite::Result<_>>()?;

            for (tid, old_cid, sid, day, period, ltype, replaces) in olds {
                let Some(new_cid) = class_map.get(&old_cid) else {
                    continue;
                };
                conn.execute(
                    "INSERT OR IGNORE INTO lessons
                        (term_id, teacher_id, class_id, subject_id, day_of_week, period_no,
                         lesson_type, replaces_homeroom)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    params![new_id, tid, new_cid, sid, day, period, ltype, replaces],
                )?;
                out.copied_lessons += 1;
            }
        }
    }

    if input.set_current {
        set_current(conn, new_id)?;
    }

    // ---------- 다음에 할 일 ----------
    if out.copied_classes == 0 {
        out.next_steps
            .push("학급을 만들어 주세요 (설정 → 학교 기본 설정).".into());
    } else if out.copied_homerooms == 0 {
        out.next_steps
            .push("담임 선생님을 다시 지정해 주세요 (교사 관리).".into());
    }
    if out.copied_bells == 0 {
        out.next_steps
            .push("시정표를 만들어 주세요 (시간표 관리).".into());
    }
    if out.copied_lessons == 0 {
        out.next_steps
            .push("전담 시간표를 입력해 주세요 (시간표 관리).".into());
    }
    out.next_steps.push(
        "교사와 과목 목록은 학교 전체 자료라 그대로 이어집니다. 전출·전입만 교사 관리에서 정리해 주세요."
            .into(),
    );

    Ok(out)
}

#[cfg(test)]
#[path = "term_tests.rs"]
mod tests;
