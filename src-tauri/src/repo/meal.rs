//! 전담교사 식사시간 — 설정과 조회.
//!
//! ## 저장하는 것은 둘뿐이다
//!
//! 1. **학교 기본 식사시간** — `settings` 의 `meal_default_bell_id`.
//!    시각이 아니라 *어느 시정표의 점심시간에 함께 먹는가* 를 가리킨다.
//!    그래서 시정표를 고치면 식사시간도 따라 바뀌고, 요일마다 점심시간이
//!    다른 학교도 그대로 맞는다.
//! 2. **교사별·요일별 수동 지정** — `teacher_meal_overrides` 표.
//!
//! **자동 판정 결과는 저장하지 않는다.** 조회할 때마다 지금 시정표와 지금
//! 전담 시간표로 다시 계산한다 (`domain::schedule::resolve_meal`).
//!
//! ## 점심 보결과 헷갈리지 않게
//!
//! 여기서 다루는 것은 전담교사가 **밥 먹는 시간**이다. 업무 개념인 '점심
//! 보결'과 다르며, 전담교사는 자기 식사시간에도 점심 보결을 맡을 수 있다.

use std::collections::HashMap;

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use super::{find as repo_find, school, settings};
use crate::domain::meal::{manual_conflict, Meal, MealSource};
use crate::domain::schedule::{resolve_meal, teacher_lesson_intervals, ROLE_SPECIAL};
use crate::domain::time::{fmt_range, Interval};
use crate::error::{AppError, AppResult};

// ============================================================
//  점심 패턴
// ============================================================

/// 학교에 실제로 있는 점심시간 하나. 같은 시각을 쓰는 학년을 묶어 보여 준다.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MealPattern {
    /// 이 패턴을 대표하는 시정표. 학교 기본 식사시간으로 저장할 값이다.
    pub bell_schedule_id: i64,
    /// '저학년 시정표'
    pub bell_name: String,
    /// 이 시정표를 쓰는 학년들
    pub grades: Vec<i32>,
    /// '1·2·3학년'
    pub grade_label: String,
    /// 요일별 점심시간. 요일마다 다를 수 있다.
    pub by_day: Vec<DayWindow>,
    /// 모든 요일이 같으면 '12:10~13:10', 다르면 '요일마다 다름'
    pub time_label: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DayWindow {
    pub day_of_week: i32,
    pub start_min: i32,
    pub end_min: i32,
}

fn grade_label(grades: &[i32]) -> String {
    if grades.is_empty() {
        return "배정된 학년 없음".to_string();
    }
    format!(
        "{}학년",
        grades.iter().map(|g| g.to_string()).collect::<Vec<_>>().join("·")
    )
}

/// 지금 학기의 점심 패턴들. **시정표에서 그때그때 읽는다.**
pub fn patterns(conn: &Connection) -> AppResult<Vec<MealPattern>> {
    let term_id = school::current_term_id(conn).unwrap_or(0);

    let mut by_bell: HashMap<i64, (String, Vec<i32>)> = HashMap::new();
    let mut stmt = conn.prepare(
        "SELECT b.id, b.name, m.grade
           FROM bell_schedules b
           LEFT JOIN grade_bell_map m ON m.bell_schedule_id = b.id AND m.term_id = ?1
          WHERE b.term_id = ?1
          ORDER BY b.id, m.grade",
    )?;
    for row in stmt.query_map([term_id], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, Option<i32>>(2)?,
        ))
    })? {
        let (id, name, grade) = row?;
        let e = by_bell.entry(id).or_insert_with(|| (name, Vec::new()));
        if let Some(g) = grade {
            e.1.push(g);
        }
    }

    let mut lunches: HashMap<i64, Vec<DayWindow>> = HashMap::new();
    let mut stmt = conn.prepare(
        "SELECT s.bell_schedule_id, s.day_of_week, s.start_min, s.end_min
           FROM bell_slots s JOIN bell_schedules b ON b.id = s.bell_schedule_id
          WHERE b.term_id = ?1 AND s.slot_type = 'LUNCH'
          ORDER BY s.bell_schedule_id, s.day_of_week",
    )?;
    for row in stmt.query_map([term_id], |r| {
        Ok(DayWindow {
            day_of_week: r.get(1)?,
            start_min: r.get(2)?,
            end_min: r.get(3)?,
        })
        .map(|w| (r.get::<_, i64>(0).unwrap_or(0), w))
    })? {
        let (bell, w) = row?;
        lunches.entry(bell).or_default().push(w);
    }

    let mut out: Vec<MealPattern> = Vec::new();
    for (id, (name, mut grades)) in by_bell {
        let Some(by_day) = lunches.remove(&id) else {
            continue; // 점심 구간이 없는 시정표는 식사시간 후보가 아니다
        };
        grades.sort_unstable();
        let first = (by_day[0].start_min, by_day[0].end_min);
        let same = by_day.iter().all(|w| (w.start_min, w.end_min) == first);
        out.push(MealPattern {
            bell_schedule_id: id,
            bell_name: name,
            grade_label: grade_label(&grades),
            grades,
            time_label: if same {
                fmt_range(&Interval::new(first.0, first.1))
            } else {
                "요일마다 다름".to_string()
            },
            by_day,
        });
    }
    // 이른 점심부터
    out.sort_by_key(|p| {
        (
            p.by_day.iter().map(|w| w.start_min).min().unwrap_or(0),
            p.grades.first().copied().unwrap_or(99),
        )
    });
    Ok(out)
}

/// 학교 기본 식사시간으로 고른 시정표. 없거나 지워졌으면 `None`.
pub fn default_bell_id(conn: &Connection) -> AppResult<Option<i64>> {
    let id = settings::get_int(conn, settings::MEAL_DEFAULT_BELL, 0)?;
    if id <= 0 {
        return Ok(None);
    }
    // 학기가 바뀌면 시정표가 새로 만들어진다. 남아 있지 않으면 없는 것으로 본다.
    let term_id = school::current_term_id(conn).unwrap_or(0);
    let alive: Option<i64> = conn
        .query_row(
            "SELECT id FROM bell_schedules WHERE id = ?1 AND term_id = ?2",
            params![id, term_id],
            |r| r.get(0),
        )
        .optional()?;
    Ok(alive)
}

pub fn set_default_bell(conn: &Connection, bell_schedule_id: Option<i64>) -> AppResult<()> {
    let term_id = school::current_term_id(conn).unwrap_or(0);
    let v = match bell_schedule_id {
        None => 0,
        Some(id) => {
            let ok: Option<i64> = conn
                .query_row(
                    "SELECT id FROM bell_schedules WHERE id = ?1 AND term_id = ?2",
                    params![id, term_id],
                    |r| r.get(0),
                )
                .optional()?;
            ok.ok_or_else(|| AppError::invalid("고른 시정표를 찾을 수 없습니다."))?
        }
    };
    settings::put_int(conn, settings::MEAL_DEFAULT_BELL, v)
}

/// 그 요일의 학교 기본 식사시간.
pub fn default_window(conn: &Connection, day_of_week: i32) -> AppResult<Option<Interval>> {
    let Some(bell) = default_bell_id(conn)? else {
        return Ok(None);
    };
    let w: Option<(i32, i32)> = conn
        .query_row(
            "SELECT start_min, end_min FROM bell_slots
              WHERE bell_schedule_id = ?1 AND day_of_week = ?2 AND slot_type = 'LUNCH'",
            params![bell, day_of_week],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    Ok(w.map(|(a, b)| Interval::new(a, b)))
}

// ============================================================
//  교사별·요일별 수동 지정
// ============================================================

pub fn overrides_for_day(conn: &Connection, day_of_week: i32) -> AppResult<HashMap<i64, Interval>> {
    let term_id = school::current_term_id(conn).unwrap_or(0);
    let mut stmt = conn.prepare(
        "SELECT teacher_id, start_min, end_min FROM teacher_meal_overrides
          WHERE term_id = ?1 AND day_of_week = ?2",
    )?;
    let mut out = HashMap::new();
    for row in stmt.query_map(params![term_id, day_of_week], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            Interval::new(r.get(1)?, r.get(2)?),
        ))
    })? {
        let (id, iv) = row?;
        out.insert(id, iv);
    }
    Ok(out)
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MealOverrideInput {
    pub teacher_id: i64,
    pub day_of_week: i32,
    /// 비우면 수동 지정을 없애고 자동 판정으로 되돌린다
    #[serde(default)]
    pub start_min: Option<i32>,
    #[serde(default)]
    pub end_min: Option<i32>,
}

/// 수동 지정을 저장한다. **그 날 실제 수업과 겹치면 막는다** —
/// 직접 지정이라고 해서 있을 수 없는 시간까지 받아 줄 이유는 없다.
pub fn set_override(conn: &Connection, input: &MealOverrideInput) -> AppResult<()> {
    let term_id = school::current_term_id(conn)?;
    if !(1..=7).contains(&input.day_of_week) {
        return Err(AppError::invalid("요일이 올바르지 않습니다."));
    }

    let (Some(start), Some(end)) = (input.start_min, input.end_min) else {
        conn.execute(
            "DELETE FROM teacher_meal_overrides
              WHERE term_id = ?1 AND teacher_id = ?2 AND day_of_week = ?3",
            params![term_id, input.teacher_id, input.day_of_week],
        )?;
        return Ok(());
    };

    if !(0..=1439).contains(&start) || !(1..=1440).contains(&end) || end <= start {
        return Err(AppError::invalid("식사시간의 시각이 올바르지 않습니다."));
    }

    // 그 날 이 교사의 실제 수업과 겹치는지 — 판정과 같은 규칙을 쓴다
    let snap = day_snapshot_for(conn, input.day_of_week)?;
    let lessons = teacher_lesson_intervals(&snap, input.teacher_id);
    let want = Interval::new(start, end);
    if let Some(hit) = manual_conflict(&want, &lessons) {
        return Err(AppError::invalid(format!(
            "그 시간에는 실제 수업({})이 있어 식사시간으로 지정할 수 없습니다.",
            fmt_range(&hit)
        )));
    }

    conn.execute(
        "INSERT INTO teacher_meal_overrides(term_id, teacher_id, day_of_week, start_min, end_min, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, datetime('now','localtime'))
         ON CONFLICT(term_id, teacher_id, day_of_week) DO UPDATE SET
            start_min = excluded.start_min,
            end_min   = excluded.end_min,
            updated_at = datetime('now','localtime')",
        params![term_id, input.teacher_id, input.day_of_week, start, end],
    )?;
    Ok(())
}

// ============================================================
//  화면에 넘길 모양
// ============================================================

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MealDay {
    pub day_of_week: i32,
    pub start_min: Option<i32>,
    pub end_min: Option<i32>,
    /// MANUAL | AUTO | DEFAULT | UNKNOWN
    pub source: String,
    /// '직접 지정' | '자동' | '학교 기본' | '확인 필요'
    pub source_label: String,
    /// '13:10~14:00' 또는 빈 문자열
    pub time_label: String,
    /// 정하지 못했을 때 그 까닭
    pub note: Option<String>,
    /// 그 날 이 교사의 수업 시간 (직접 지정할 때 겹치는지 보여 준다)
    pub lessons: Vec<DayWindow>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MealView {
    pub patterns: Vec<MealPattern>,
    pub default_bell_id: Option<i64>,
    /// 기본 식사시간을 아직 정하지 않았고, 정해야 판단할 수 있는가
    pub needs_default: bool,
    pub school_days: Vec<i32>,
    pub teacher_id: Option<i64>,
    pub teacher_name: Option<String>,
    pub is_special: bool,
    pub days: Vec<MealDay>,
}

/// 요일 하나치 자료. 날짜가 아니라 요일로만 본다 (식사시간은 요일 규칙이다).
fn day_snapshot_for(conn: &Connection, day_of_week: i32) -> AppResult<crate::domain::schedule::DaySnapshot> {
    // 그 요일에 해당하는 가장 가까운 날짜를 만들어 스냅샷을 읽는다.
    let today = chrono::Local::now().date_naive();
    let cur = today.format("%u").to_string().parse::<i32>().unwrap_or(1);
    let delta = (day_of_week - cur).rem_euclid(7);
    let date = (today + chrono::Duration::days(delta as i64))
        .format("%Y-%m-%d")
        .to_string();
    repo_find::snapshot(conn, &date)
}

pub fn view(conn: &Connection, teacher_id: Option<i64>) -> AppResult<MealView> {
    let patterns = patterns(conn)?;
    let default_bell_id = default_bell_id(conn)?;
    let school_days = school::get(conn)?
        .map(|s| s.school_days)
        .filter(|d| !d.is_empty())
        .unwrap_or_else(|| vec![1, 2, 3, 4, 5]);

    let teacher: Option<(i64, String, String)> = match teacher_id {
        Some(id) => conn
            .query_row(
                "SELECT id, name, role_code FROM teachers WHERE id = ?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?,
        None => None,
    };

    let mut days = Vec::new();
    if let Some((tid, _, _)) = &teacher {
        for d in &school_days {
            let snap = day_snapshot_for(conn, *d)?;
            let lessons = teacher_lesson_intervals(&snap, *tid);
            let m = resolve_meal(&snap, *tid);
            let (start, end, source, source_label, note) = match m {
                Meal::Known { interval, source } => (
                    Some(interval.start),
                    Some(interval.end),
                    match source {
                        MealSource::Manual => "MANUAL",
                        MealSource::Auto => "AUTO",
                        MealSource::SchoolDefault => "DEFAULT",
                    },
                    match source {
                        MealSource::Manual => "직접 지정",
                        MealSource::Auto => "자동",
                        MealSource::SchoolDefault => "학교 기본",
                    },
                    None,
                ),
                Meal::Unknown(why) => (None, None, "UNKNOWN", "확인 필요", Some(why.message().to_string())),
            };
            days.push(MealDay {
                day_of_week: *d,
                start_min: start,
                end_min: end,
                source: source.to_string(),
                source_label: source_label.to_string(),
                time_label: match (start, end) {
                    (Some(a), Some(b)) => fmt_range(&Interval::new(a, b)),
                    _ => String::new(),
                },
                note,
                lessons: lessons
                    .iter()
                    .map(|l| DayWindow {
                        day_of_week: *d,
                        start_min: l.start,
                        end_min: l.end,
                    })
                    .collect(),
            });
        }
    }

    Ok(MealView {
        needs_default: default_bell_id.is_none() && patterns.len() > 1,
        patterns,
        default_bell_id,
        school_days,
        teacher_id: teacher.as_ref().map(|t| t.0),
        teacher_name: teacher.as_ref().map(|t| t.1.clone()),
        is_special: teacher.as_ref().map(|t| t.2 == ROLE_SPECIAL).unwrap_or(false),
        days,
    })
}


#[cfg(test)]
#[path = "meal_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "meal_real_db_tests.rs"]
mod real_db_tests;
