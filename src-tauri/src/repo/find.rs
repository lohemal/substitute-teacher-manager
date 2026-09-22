//! 보결 조회에 필요한 하루치 자료를 DB에서 읽어 온다.
//!
//! 판단은 `domain::find`가 한다. 여기서는 자료를 모으는 일만 한다.

use std::collections::HashMap;

use chrono::{Datelike, NaiveDate};
use rusqlite::{params, Connection};
use serde::Serialize;

use super::school;
use crate::domain::find::SubCounts;
use crate::domain::schedule::{
    AbsenceInfo, AssignedInfo, ClassInfo, DaySnapshot, DutyInfo, EngineSettings, LessonInfo,
    SlotInfo, TeacherInfo,
};
use crate::error::{AppError, AppResult};
use crate::label::{class_full, class_short};

/// 'YYYY-MM-DD' -> 요일(1=월 … 7=일)
pub fn parse_date(date: &str) -> AppResult<(NaiveDate, i32)> {
    let d = NaiveDate::parse_from_str(date.trim(), "%Y-%m-%d").map_err(|_| {
        AppError::invalid("날짜 형식이 올바르지 않습니다. 달력에서 날짜를 선택해 주세요.")
    })?;
    Ok((d, d.weekday().number_from_monday() as i32))
}

fn load_settings(conn: &Connection) -> AppResult<EngineSettings> {
    use super::settings as st;
    Ok(EngineSettings {
        derive_homeroom_schedule: st::get_bool(conn, st::DERIVE_HOMEROOM, true)?,
        exclude_homeroom_on_own_lunch: st::get_bool(conn, st::EXCLUDE_LUNCH, true)?,
        exclude_homeroom_on_recess: st::get_bool(conn, st::EXCLUDE_RECESS, true)?,
        include_special_teachers: st::get_bool(conn, st::INCLUDE_SPECIAL, true)?,
        include_after_school_end: st::get_bool(conn, st::INCLUDE_AFTER_END, true)?,
        include_other_grade_homeroom: st::get_bool(conn, st::INCLUDE_OTHER_GRADE, true)?,
        // 값이 없는 예전 자료에서는 꺼진 것으로 읽는다 — migration 없이
        // 기존 학교가 점심 보결에 담임을 갑자기 받는 일이 없다.
        include_cross_lunch_homeroom: st::get_bool(conn, st::INCLUDE_CROSS_LUNCH, false)?,
    })
}

/// 하루치 자료를 모두 읽는다.
pub fn snapshot(conn: &Connection, date: &str) -> AppResult<DaySnapshot> {
    let (_, day_of_week) = parse_date(date)?;
    let term_id = school::current_term_id(conn)?;

    // ---------- 교사 ----------
    let mut stmt = conn.prepare(
        "SELECT t.id, t.name, t.role_code, r.label, t.memo, t.is_substitutable, t.active
           FROM teachers t JOIN teacher_roles r ON r.code = t.role_code
          ORDER BY t.name",
    )?;
    let mut teachers: Vec<TeacherInfo> = stmt
        .query_map([], |r| {
            Ok(TeacherInfo {
                id: r.get(0)?,
                name: r.get(1)?,
                role_code: r.get(2)?,
                role_label: r.get(3)?,
                memo: r.get(4)?,
                is_substitutable: r.get::<_, i64>(5)? != 0,
                active: r.get::<_, i64>(6)? != 0,
                subjects: Vec::new(),
            })
        })?
        .collect::<rusqlite::Result<_>>()?;

    let mut stmt = conn.prepare(
        "SELECT ts.teacher_id, s.name FROM teacher_subjects ts
           JOIN subjects s ON s.id = ts.subject_id
          ORDER BY ts.is_primary DESC, s.id",
    )?;
    let mut subj: HashMap<i64, Vec<String>> = HashMap::new();
    for row in stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))? {
        let (tid, name) = row?;
        subj.entry(tid).or_default().push(name);
    }
    for t in &mut teachers {
        t.subjects = subj.get(&t.id).cloned().unwrap_or_default();
    }

    // ---------- 학급 ----------
    let mut stmt = conn.prepare(
        "SELECT id, grade, class_no, name, homeroom_teacher_id FROM classes
          WHERE term_id = ?1 AND active = 1 ORDER BY grade, class_no",
    )?;
    let classes: Vec<ClassInfo> = stmt
        .query_map([term_id], |r| {
            let grade: i32 = r.get(1)?;
            let class_no: i32 = r.get(2)?;
            let name: Option<String> = r.get(3)?;
            Ok(ClassInfo {
                id: r.get(0)?,
                grade,
                class_no,
                label: class_short(grade, class_no, name.as_deref()),
                full_label: class_full(grade, class_no, name.as_deref()),
                homeroom_teacher_id: r.get(4)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;

    // ---------- 시정표 (그 요일만) ----------
    let mut stmt = conn.prepare(
        "SELECT m.grade, s.day_of_week, s.slot_type, s.period_no, s.label, s.start_min, s.end_min
           FROM grade_bell_map m
           JOIN bell_slots s ON s.bell_schedule_id = m.bell_schedule_id
          WHERE m.term_id = ?1 AND s.day_of_week = ?2",
    )?;
    let slots: Vec<SlotInfo> = stmt
        .query_map(params![term_id, day_of_week], |r| {
            Ok(SlotInfo {
                grade: r.get(0)?,
                day_of_week: r.get(1)?,
                slot_type: r.get(2)?,
                period_no: r.get(3)?,
                label: r.get(4)?,
                start_min: r.get(5)?,
                end_min: r.get(6)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;

    // ---------- 수업 (그 요일만) ----------
    let mut stmt = conn.prepare(
        "SELECT l.teacher_id, l.class_id, l.period_no, l.replaces_homeroom, s.name
           FROM lessons l LEFT JOIN subjects s ON s.id = l.subject_id
          WHERE l.term_id = ?1 AND l.day_of_week = ?2",
    )?;
    let lessons: Vec<LessonInfo> = stmt
        .query_map(params![term_id, day_of_week], |r| {
            Ok(LessonInfo {
                teacher_id: r.get(0)?,
                class_id: r.get(1)?,
                period_no: r.get(2)?,
                replaces_homeroom: r.get::<_, i64>(3)? != 0,
                subject_name: r.get(4)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;

    // ---------- 고정 일정 ----------
    let mut stmt = conn.prepare(
        "SELECT teacher_id, block_type, label, start_min, end_min
           FROM teacher_busy_blocks
          WHERE active = 1
            AND ( (recurrence = 'WEEKLY' AND day_of_week = ?1)
               OR (recurrence = 'ONCE'   AND specific_date = ?2) )",
    )?;
    let duties: Vec<DutyInfo> = stmt
        .query_map(params![day_of_week, date], |r| {
            Ok(DutyInfo {
                teacher_id: r.get(0)?,
                block_type: r.get(1)?,
                label: r.get(2)?,
                start_min: r.get(3)?,
                end_min: r.get(4)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;

    // ---------- 부재 ----------
    let mut stmt = conn.prepare(
        "SELECT a.teacher_id, a.is_all_day, a.start_min, a.end_min,
                COALESCE(r.label, a.reason_text)
           FROM absences a LEFT JOIN absence_reasons r ON r.code = a.reason_code
          WHERE a.date = ?1 AND a.status = 'ACTIVE'",
    )?;
    let absences: Vec<AbsenceInfo> = stmt
        .query_map([date], |r| {
            Ok(AbsenceInfo {
                teacher_id: r.get(0)?,
                is_all_day: r.get::<_, i64>(1)? != 0,
                start_min: r.get::<_, Option<i32>>(2)?.unwrap_or(0),
                end_min: r.get::<_, Option<i32>>(3)?.unwrap_or(1440),
                reason_label: r.get(4)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;

    // ---------- 이미 배정된 보결 ----------
    let mut stmt = conn.prepare(
        "SELECT sub_teacher_id, start_min, end_min,
                COALESCE(class_label, grade || '-' || class_no), slot_label
           FROM substitutions
          WHERE date = ?1 AND status = 'ASSIGNED'",
    )?;
    let assigned: Vec<AssignedInfo> = stmt
        .query_map([date], |r| {
            let cls: String = r.get(3)?;
            let slot: String = r.get(4)?;
            Ok(AssignedInfo {
                sub_teacher_id: r.get(0)?,
                start_min: r.get(1)?,
                end_min: r.get(2)?,
                label: format!("보결 {cls} {slot}"),
            })
        })?
        .collect::<rusqlite::Result<_>>()?;

    Ok(DaySnapshot {
        date: date.to_string(),
        day_of_week,
        teachers,
        classes,
        slots,
        lessons,
        duties,
        absences,
        assigned,
        settings: load_settings(conn)?,
        // 전담교사 식사시간에 필요한 두 가지. 자동 판정 결과가 아니라
        // **사람이 정한 값**만 읽어 온다 (판정은 domain 이 그때그때 한다).
        meal_default: super::meal::default_window(conn, day_of_week)?,
        meal_overrides: super::meal::overrides_for_day(conn, day_of_week)?,
    })
}

/// 교사별 보결 횟수 (오늘 / 이번 달 / 누적). 쿼리 3번으로 한꺼번에 센다.
pub fn counts(conn: &Connection, date: &str) -> AppResult<HashMap<i64, SubCounts>> {
    let month = &date.get(0..7).unwrap_or(date);
    let mut out: HashMap<i64, SubCounts> = HashMap::new();

    let mut stmt = conn.prepare(
        "SELECT sub_teacher_id, COUNT(*) FROM substitutions
          WHERE status = 'ASSIGNED' AND date = ?1 GROUP BY sub_teacher_id",
    )?;
    for row in stmt.query_map([date], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i32>(1)?)))? {
        let (id, n) = row?;
        out.entry(id).or_default().today = n;
    }

    let mut stmt = conn.prepare(
        "SELECT sub_teacher_id, COUNT(*) FROM substitutions
          WHERE status = 'ASSIGNED' AND substr(date,1,7) = ?1 GROUP BY sub_teacher_id",
    )?;
    for row in stmt.query_map([month], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i32>(1)?)))? {
        let (id, n) = row?;
        out.entry(id).or_default().month = n;
    }

    let mut stmt = conn.prepare(
        "SELECT sub_teacher_id, COUNT(*) FROM substitutions
          WHERE status = 'ASSIGNED' GROUP BY sub_teacher_id",
    )?;
    for row in stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i32>(1)?)))? {
        let (id, n) = row?;
        out.entry(id).or_default().total = n;
    }

    Ok(out)
}

// ============================================================
//  조회 화면이 쓰는 선택 항목
// ============================================================

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassChoice {
    pub class_id: i64,
    pub grade: i32,
    pub class_no: i32,
    pub name: Option<String>,
    pub label: String,
    pub full_label: String,
    pub homeroom_teacher_id: Option<i64>,
    pub homeroom_name: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SlotChoice {
    pub grade: i32,
    pub day_of_week: i32,
    pub slot_type: String,
    pub period_no: Option<i32>,
    pub label: String,
    pub start_min: i32,
    pub end_min: i32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TeacherChoice {
    pub id: i64,
    pub name: String,
    pub role_label: String,
    pub duty: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReasonChoice {
    pub code: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FindOptions {
    pub classes: Vec<ClassChoice>,
    /// 학년·요일별로 고를 수 있는 교시와 점심 (실제 시각 포함)
    pub slots: Vec<SlotChoice>,
    pub teachers: Vec<TeacherChoice>,
    pub reasons: Vec<ReasonChoice>,
    pub school_days: Vec<i32>,
    /// 준비가 덜 된 부분 (시정표 없음, 담임 미지정 등)
    pub blockers: Vec<String>,
}

pub fn options(conn: &Connection) -> AppResult<FindOptions> {
    let sc = school::get(conn)?
        .ok_or_else(|| AppError::setup_required("학교 기본 설정을 먼저 완료해 주세요."))?;
    let term_id = school::current_term_id(conn)?;

    let mut stmt = conn.prepare(
        "SELECT c.id, c.grade, c.class_no, c.name, t.name, c.homeroom_teacher_id
           FROM classes c LEFT JOIN teachers t ON t.id = c.homeroom_teacher_id
          WHERE c.term_id = ?1 AND c.active = 1 ORDER BY c.grade, c.class_no",
    )?;
    let classes: Vec<ClassChoice> = stmt
        .query_map([term_id], |r| {
            let grade: i32 = r.get(1)?;
            let class_no: i32 = r.get(2)?;
            let name: Option<String> = r.get(3)?;
            Ok(ClassChoice {
                class_id: r.get(0)?,
                grade,
                class_no,
                label: class_short(grade, class_no, name.as_deref()),
                full_label: class_full(grade, class_no, name.as_deref()),
                name,
                homeroom_teacher_id: r.get(5)?,
                homeroom_name: r.get(4)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;

    let mut stmt = conn.prepare(
        "SELECT m.grade, s.day_of_week, s.slot_type, s.period_no, s.label, s.start_min, s.end_min
           FROM grade_bell_map m
           JOIN bell_slots s ON s.bell_schedule_id = m.bell_schedule_id
          WHERE m.term_id = ?1 AND s.slot_type IN ('PERIOD','LUNCH')
          ORDER BY m.grade, s.day_of_week, s.start_min",
    )?;
    let slots: Vec<SlotChoice> = stmt
        .query_map([term_id], |r| {
            Ok(SlotChoice {
                grade: r.get(0)?,
                day_of_week: r.get(1)?,
                slot_type: r.get(2)?,
                period_no: r.get(3)?,
                label: r.get(4)?,
                start_min: r.get(5)?,
                end_min: r.get(6)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;

    let mut stmt = conn.prepare(
        "SELECT t.id, t.name, r.label,
                COALESCE(
                  (SELECT group_concat(
                     CASE WHEN c.name IS NULL THEN c.grade || '-' || c.class_no
                          ELSE c.grade || '-' || c.name END, ', ')
                     FROM classes c
                    WHERE c.homeroom_teacher_id = t.id AND c.term_id = ?1 AND c.active = 1),
                  (SELECT group_concat(s.name, ', ') FROM teacher_subjects ts
                     JOIN subjects s ON s.id = ts.subject_id WHERE ts.teacher_id = t.id),
                  t.memo, '')
           FROM teachers t JOIN teacher_roles r ON r.code = t.role_code
          WHERE t.active = 1
          ORDER BY r.sort_order, t.name",
    )?;
    let teachers: Vec<TeacherChoice> = stmt
        .query_map([term_id], |r| {
            Ok(TeacherChoice {
                id: r.get(0)?,
                name: r.get(1)?,
                role_label: r.get(2)?,
                duty: r.get(3)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;

    let mut stmt = conn.prepare(
        "SELECT code, label FROM absence_reasons WHERE active = 1 ORDER BY sort_order",
    )?;
    let reasons: Vec<ReasonChoice> = stmt
        .query_map([], |r| {
            Ok(ReasonChoice {
                code: r.get(0)?,
                label: r.get(1)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;

    // 준비 상태 확인
    let mut blockers = Vec::new();
    if classes.is_empty() {
        blockers.push("학급이 없습니다. 학교 기본 설정에서 학년별 반 수를 입력해 주세요.".into());
    }
    if slots.is_empty() {
        blockers.push("시정표가 없습니다. 시간표 관리에서 학년별 시정표를 먼저 설정해 주세요.".into());
    }
    if teachers.is_empty() {
        blockers.push("등록된 교사가 없습니다. 교사 관리에서 선생님을 등록해 주세요.".into());
    }
    let no_hr: Vec<String> = classes
        .iter()
        .filter(|c| c.homeroom_name.is_none())
        .map(|c| c.label.clone())
        .collect();
    if !no_hr.is_empty() {
        blockers.push(format!(
            "{}의 담임이 지정되지 않았습니다. 그 학급 수업 시간이 계산되지 않아 결과가 정확하지 않습니다.",
            no_hr.join(", ")
        ));
    }

    Ok(FindOptions {
        classes,
        slots,
        teachers,
        reasons,
        school_days: sc.school_days,
        blockers,
    })
}

#[cfg(test)]
#[path = "find_option_tests.rs"]
mod option_tests;

#[cfg(test)]
#[path = "find_real_db_tests.rs"]
mod real_db_tests;
