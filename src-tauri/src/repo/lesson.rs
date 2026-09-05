//! 전담교사 시간표.
//!
//! ## 시각은 저장하지 않고 계산한다
//!
//! 수업의 실제 시각은 `class_id → grade → grade_bell_map → bell_slots(요일, 교시)`로
//! 구한다. 시정표를 고치면 시간표의 시각도 함께 따라가야 하므로 복사해 두지 않는다.
//! (지난 보결 기록은 배정할 때 시각을 스냅샷하므로 영향받지 않는다)
//!
//! ## 겹침은 교시 번호가 아니라 시각으로 본다
//!
//! 학년마다 교시 시각이 다르기 때문이다. 예를 들어 저학년 5교시(13:00~13:40)와
//! 고학년 5교시(12:15~12:55)는 교시 번호가 같아도 시간이 겹치지 않는다.
//!
//! ## 담임 공강
//!
//! `replaces_homeroom = 1` 인 수업이 있는 (학급, 요일, 교시)에는 그 반 담임이
//! 수업하지 않는다. Phase 6의 담임 시간표 계산이 이 값을 그대로 쓴다.
//! 공동수업(CO)은 담임도 함께 들어가므로 0이다.

use std::collections::HashMap;

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use super::bell::{day_name, fmt_min};
use super::school;
use crate::error::{AppError, AppResult};
use crate::label::{class_full, class_short};

pub const TYPE_SPECIAL: &str = "SPECIAL";
pub const TYPE_CROSS: &str = "CROSS";
pub const TYPE_CO: &str = "CO";
pub const TYPE_ETC: &str = "ETC";

/// 공동수업만 담임이 함께 들어간다.
fn replaces_homeroom(lesson_type: &str) -> bool {
    lesson_type != TYPE_CO
}

// ============================================================
//  화면에 보내는 모양
// ============================================================

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LessonView {
    pub id: i64,
    pub day_of_week: i32,
    pub period_no: i32,
    pub class_id: i64,
    pub grade: i32,
    pub class_no: i32,
    /// '5-가람'
    pub class_label: String,
    pub subject_id: Option<i64>,
    pub subject_name: Option<String>,
    pub lesson_type: String,
    pub replaces_homeroom: bool,
    pub note: Option<String>,
    /// 학년별 시정표에서 계산한 실제 시각. 시정표에 없는 교시면 None
    pub start_min: Option<i32>,
    pub end_min: Option<i32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TeacherLessons {
    pub teacher_id: i64,
    pub teacher_name: String,
    pub lessons: Vec<LessonView>,
    /// 고쳐야 할 점 (시간 겹침, 시정표에 없는 교시 등)
    pub problems: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LessonTeacher {
    pub id: i64,
    pub name: String,
    pub role_code: String,
    pub role_label: String,
    pub memo: Option<String>,
    pub subjects: Vec<String>,
    pub subject_ids: Vec<i64>,
    pub lesson_count: i32,
    /// 담임인 경우 담당 학급 ('5-가람')
    pub homerooms: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassOption {
    pub class_id: i64,
    pub grade: i32,
    pub class_no: i32,
    pub name: Option<String>,
    /// '5-가람'
    pub label: String,
    /// '5학년 가람반'
    pub full_label: String,
    pub homeroom_name: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubjectOption {
    pub id: i64,
    pub name: String,
}

/// 학년·요일·교시별 실제 시각. 화면이 시각을 보여 주고
/// '그 학년에는 없는 교시'를 미리 막는 데 쓴다.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GradeSlot {
    pub grade: i32,
    pub day_of_week: i32,
    pub period_no: i32,
    pub start_min: i32,
    pub end_min: i32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LessonOverview {
    pub teachers: Vec<LessonTeacher>,
    pub classes: Vec<ClassOption>,
    pub subjects: Vec<SubjectOption>,
    pub school_days: Vec<i32>,
    /// 시간표 표의 행 수
    pub max_period: i32,
    pub grade_slots: Vec<GradeSlot>,
    /// 시정표가 아직 없으면 true (시간표를 넣어도 시각을 알 수 없음)
    pub bell_missing: bool,
}

// ============================================================
//  입력
// ============================================================

fn default_type() -> String {
    TYPE_SPECIAL.to_string()
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LessonInput {
    pub id: Option<i64>,
    pub teacher_id: i64,
    pub day_of_week: i32,
    pub period_no: i32,
    pub class_id: i64,
    #[serde(default)]
    pub subject_id: Option<i64>,
    #[serde(default = "default_type")]
    pub lesson_type: String,
    #[serde(default)]
    pub note: Option<String>,
}

/// 붙여넣기·일괄 반영에서 쓰는 한 칸.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LessonRow {
    pub day_of_week: i32,
    pub period_no: i32,
    pub class_id: i64,
    #[serde(default)]
    pub subject_id: Option<i64>,
    #[serde(default = "default_type")]
    pub lesson_type: String,
    #[serde(default)]
    pub note: Option<String>,
}

// ============================================================
//  시각 계산
// ============================================================

/// (학년, 요일, 교시) -> (시작, 종료)
type SlotMap = HashMap<(i32, i32, i32), (i32, i32)>;

fn slot_map(conn: &Connection, term_id: i64) -> AppResult<SlotMap> {
    let mut stmt = conn.prepare(
        "SELECT m.grade, s.day_of_week, s.period_no, s.start_min, s.end_min
           FROM grade_bell_map m
           JOIN bell_slots s ON s.bell_schedule_id = m.bell_schedule_id
          WHERE m.term_id = ?1 AND s.slot_type = 'PERIOD' AND s.period_no IS NOT NULL",
    )?;
    let rows: Vec<(i32, i32, i32, i32, i32)> = stmt
        .query_map([term_id], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        })?
        .collect::<rusqlite::Result<_>>()?;

    Ok(rows
        .into_iter()
        .map(|(g, d, p, s, e)| ((g, d, p), (s, e)))
        .collect())
}

struct ClassInfo {
    grade: i32,
    class_no: i32,
    name: Option<String>,
}

fn class_map(conn: &Connection, term_id: i64) -> AppResult<HashMap<i64, ClassInfo>> {
    let mut stmt = conn.prepare(
        "SELECT id, grade, class_no, name FROM classes WHERE term_id = ?1 AND active = 1",
    )?;
    let rows: Vec<(i64, i32, i32, Option<String>)> = stmt
        .query_map([term_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
        .collect::<rusqlite::Result<_>>()?;

    Ok(rows
        .into_iter()
        .map(|(id, grade, class_no, name)| {
            (
                id,
                ClassInfo {
                    grade,
                    class_no,
                    name,
                },
            )
        })
        .collect())
}

// ============================================================
//  검증 — 화면 미리보기와 저장이 같은 규칙을 쓴다
// ============================================================

struct Placed {
    day: i32,
    start: i32,
    end: i32,
    label: String,
}

/// 한 교사의 수업들을 보고 고쳐야 할 점을 모두 모은다.
pub fn collect_problems(
    conn: &Connection,
    term_id: i64,
    rows: &[LessonRow],
) -> AppResult<Vec<String>> {
    let slots = slot_map(conn, term_id)?;
    let classes = class_map(conn, term_id)?;
    let mut out: Vec<String> = Vec::new();
    let mut placed: Vec<Placed> = Vec::new();

    for r in rows {
        if !(1..=7).contains(&r.day_of_week) {
            out.push("요일이 올바르지 않습니다.".into());
            continue;
        }
        if r.period_no < 1 {
            out.push("교시가 올바르지 않습니다.".into());
            continue;
        }
        if !matches!(
            r.lesson_type.as_str(),
            TYPE_SPECIAL | TYPE_CROSS | TYPE_CO | TYPE_ETC
        ) {
            out.push("수업 구분이 올바르지 않습니다.".into());
            continue;
        }
        let Some(ci) = classes.get(&r.class_id) else {
            out.push("없는 학급이 지정되었습니다. 학급을 다시 선택해 주세요.".into());
            continue;
        };

        let short = class_short(ci.grade, ci.class_no, ci.name.as_deref());
        let dn = day_name(r.day_of_week);

        match slots.get(&(ci.grade, r.day_of_week, r.period_no)) {
            None => out.push(format!(
                "{}은 {}요일에 {}교시가 없습니다. 시정표를 확인하거나 다른 교시로 옮겨 주세요.",
                class_full(ci.grade, ci.class_no, ci.name.as_deref()),
                dn,
                r.period_no
            )),
            Some(&(start, end)) => placed.push(Placed {
                day: r.day_of_week,
                start,
                end,
                label: format!("{dn}요일 {short} {}교시", r.period_no),
            }),
        }
    }

    // 같은 요일 안에서 실제 시각이 겹치는지 본다
    for day in 1..=7 {
        let mut same: Vec<&Placed> = placed.iter().filter(|p| p.day == day).collect();
        same.sort_by_key(|p| (p.start, p.end));
        for w in same.windows(2) {
            let (a, b) = (w[0], w[1]);
            if a.end > b.start {
                out.push(format!(
                    "{}({}~{})와 {}({}~{})의 시간이 겹칩니다. 한 선생님이 같은 시간에 두 반을 맡을 수는 없습니다.",
                    a.label,
                    fmt_min(a.start),
                    fmt_min(a.end),
                    b.label,
                    fmt_min(b.start),
                    fmt_min(b.end)
                ));
            }
        }
    }

    Ok(out)
}

fn validate(conn: &Connection, term_id: i64, rows: &[LessonRow]) -> AppResult<()> {
    let problems = collect_problems(conn, term_id, rows)?;
    match problems.first() {
        None => Ok(()),
        Some(first) => Err(AppError::invalid(first.clone()).detail(problems.join(" / "))),
    }
}

fn rows_of(conn: &Connection, term_id: i64, teacher_id: i64) -> AppResult<Vec<LessonRow>> {
    let mut stmt = conn.prepare(
        "SELECT day_of_week, period_no, class_id, subject_id, lesson_type, note
           FROM lessons WHERE term_id = ?1 AND teacher_id = ?2",
    )?;
    let rows: Vec<LessonRow> = stmt
        .query_map(params![term_id, teacher_id], |r| {
            Ok(LessonRow {
                day_of_week: r.get(0)?,
                period_no: r.get(1)?,
                class_id: r.get(2)?,
                subject_id: r.get(3)?,
                lesson_type: r.get(4)?,
                note: r.get(5)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

// ============================================================
//  조회
// ============================================================

pub fn overview(conn: &Connection) -> AppResult<LessonOverview> {
    let sc = school::get(conn)?
        .ok_or_else(|| AppError::setup_required("학교 기본 설정을 먼저 완료해 주세요."))?;
    let term_id = school::current_term_id(conn)?;

    // 교사 — 전담을 먼저, 그다음 담임·기타 (담임도 교차수업을 넣을 수 있다)
    let mut stmt = conn.prepare(
        "SELECT t.id, t.name, t.role_code, r.label, t.memo
           FROM teachers t
           JOIN teacher_roles r ON r.code = t.role_code
          WHERE t.active = 1
          ORDER BY CASE t.role_code WHEN 'SPECIAL' THEN 0 ELSE 1 END, r.sort_order, t.name",
    )?;
    struct TRow {
        id: i64,
        name: String,
        role_code: String,
        role_label: String,
        memo: Option<String>,
    }
    let trows: Vec<TRow> = stmt
        .query_map([], |r| {
            Ok(TRow {
                id: r.get(0)?,
                name: r.get(1)?,
                role_code: r.get(2)?,
                role_label: r.get(3)?,
                memo: r.get(4)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;

    // 교사 -> 과목
    let mut stmt = conn.prepare(
        "SELECT ts.teacher_id, s.id, s.name FROM teacher_subjects ts
           JOIN subjects s ON s.id = ts.subject_id
          ORDER BY ts.is_primary DESC, s.id",
    )?;
    let mut subj_of: HashMap<i64, Vec<(i64, String)>> = HashMap::new();
    let srows: Vec<(i64, i64, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<rusqlite::Result<_>>()?;
    for (tid, sid, name) in srows {
        subj_of.entry(tid).or_default().push((sid, name));
    }

    // 수업 수
    let mut stmt = conn.prepare(
        "SELECT teacher_id, COUNT(*) FROM lessons WHERE term_id = ?1 GROUP BY teacher_id",
    )?;
    let counts: HashMap<i64, i32> = stmt
        .query_map([term_id], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?
        .into_iter()
        .collect();

    // 담임 학급
    let mut stmt = conn.prepare(
        "SELECT homeroom_teacher_id, grade, class_no, name FROM classes
          WHERE term_id = ?1 AND active = 1 AND homeroom_teacher_id IS NOT NULL
          ORDER BY grade, class_no",
    )?;
    let mut hr_of: HashMap<i64, Vec<String>> = HashMap::new();
    let hrows: Vec<(i64, i32, i32, Option<String>)> = stmt
        .query_map([term_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
        .collect::<rusqlite::Result<_>>()?;
    for (tid, grade, class_no, name) in hrows {
        hr_of
            .entry(tid)
            .or_default()
            .push(class_short(grade, class_no, name.as_deref()));
    }

    let teachers: Vec<LessonTeacher> = trows
        .into_iter()
        .map(|t| {
            let subs = subj_of.get(&t.id).cloned().unwrap_or_default();
            LessonTeacher {
                subjects: subs.iter().map(|(_, n)| n.clone()).collect(),
                subject_ids: subs.iter().map(|(i, _)| *i).collect(),
                lesson_count: *counts.get(&t.id).unwrap_or(&0),
                homerooms: hr_of.get(&t.id).cloned().unwrap_or_default(),
                id: t.id,
                name: t.name,
                role_code: t.role_code,
                role_label: t.role_label,
                memo: t.memo,
            }
        })
        .collect();

    // 학급
    let mut stmt = conn.prepare(
        "SELECT c.id, c.grade, c.class_no, c.name, t.name
           FROM classes c
           LEFT JOIN teachers t ON t.id = c.homeroom_teacher_id
          WHERE c.term_id = ?1 AND c.active = 1
          ORDER BY c.grade, c.class_no",
    )?;
    let classes: Vec<ClassOption> = stmt
        .query_map([term_id], |r| {
            let grade: i32 = r.get(1)?;
            let class_no: i32 = r.get(2)?;
            let name: Option<String> = r.get(3)?;
            Ok(ClassOption {
                class_id: r.get(0)?,
                grade,
                class_no,
                label: class_short(grade, class_no, name.as_deref()),
                full_label: class_full(grade, class_no, name.as_deref()),
                name,
                homeroom_name: r.get(4)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;

    // 과목
    let mut stmt = conn.prepare("SELECT id, name FROM subjects WHERE active = 1 ORDER BY id")?;
    let subjects: Vec<SubjectOption> = stmt
        .query_map([], |r| {
            Ok(SubjectOption {
                id: r.get(0)?,
                name: r.get(1)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;

    // 시각
    let slots = slot_map(conn, term_id)?;
    let mut grade_slots: Vec<GradeSlot> = slots
        .iter()
        .map(|(&(grade, day_of_week, period_no), &(start_min, end_min))| GradeSlot {
            grade,
            day_of_week,
            period_no,
            start_min,
            end_min,
        })
        .collect();
    grade_slots.sort_by_key(|g| (g.grade, g.day_of_week, g.period_no));

    let max_period = grade_slots.iter().map(|g| g.period_no).max().unwrap_or(0);

    Ok(LessonOverview {
        teachers,
        classes,
        subjects,
        school_days: sc.school_days,
        max_period,
        bell_missing: grade_slots.is_empty(),
        grade_slots,
    })
}

pub fn get(conn: &Connection, teacher_id: i64) -> AppResult<TeacherLessons> {
    let term_id = school::current_term_id(conn)?;
    let teacher_name: String = conn
        .query_row("SELECT name FROM teachers WHERE id = ?1", [teacher_id], |r| {
            r.get(0)
        })
        .optional()?
        .ok_or_else(|| AppError::not_found("교사를 찾을 수 없습니다."))?;

    let slots = slot_map(conn, term_id)?;

    let mut stmt = conn.prepare(
        "SELECT l.id, l.day_of_week, l.period_no, l.class_id, c.grade, c.class_no, c.name,
                l.subject_id, s.name, l.lesson_type, l.replaces_homeroom, l.note
           FROM lessons l
           JOIN classes c ON c.id = l.class_id
           LEFT JOIN subjects s ON s.id = l.subject_id
          WHERE l.term_id = ?1 AND l.teacher_id = ?2
          ORDER BY l.day_of_week, l.period_no",
    )?;
    let lessons: Vec<LessonView> = stmt
        .query_map(params![term_id, teacher_id], |r| {
            let grade: i32 = r.get(4)?;
            let class_no: i32 = r.get(5)?;
            let cname: Option<String> = r.get(6)?;
            let day: i32 = r.get(1)?;
            let period: i32 = r.get(2)?;
            let time = slots.get(&(grade, day, period)).copied();
            Ok(LessonView {
                id: r.get(0)?,
                day_of_week: day,
                period_no: period,
                class_id: r.get(3)?,
                grade,
                class_no,
                class_label: class_short(grade, class_no, cname.as_deref()),
                subject_id: r.get(7)?,
                subject_name: r.get(8)?,
                lesson_type: r.get(9)?,
                replaces_homeroom: r.get::<_, i64>(10)? != 0,
                note: r.get(11)?,
                start_min: time.map(|(s, _)| s),
                end_min: time.map(|(_, e)| e),
            })
        })?
        .collect::<rusqlite::Result<_>>()?;

    let problems = collect_problems(conn, term_id, &rows_of(conn, term_id, teacher_id)?)?;

    Ok(TeacherLessons {
        teacher_id,
        teacher_name,
        lessons,
        problems,
    })
}

// ============================================================
//  변경
// ============================================================

fn ensure_teacher(conn: &Connection, teacher_id: i64) -> AppResult<()> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM teachers WHERE id = ?1",
        [teacher_id],
        |r| r.get(0),
    )?;
    if n == 0 {
        return Err(AppError::not_found("교사를 찾을 수 없습니다."));
    }
    Ok(())
}

pub fn upsert(conn: &Connection, input: &LessonInput) -> AppResult<i64> {
    let term_id = school::current_term_id(conn)?;
    ensure_teacher(conn, input.teacher_id)?;

    // 이 교사의 나머지 수업 + 이번 수업을 합쳐서 검증한다
    let mut rows = rows_of(conn, term_id, input.teacher_id)?;
    if let Some(id) = input.id {
        let old: Option<(i32, i32, i64)> = conn
            .query_row(
                "SELECT day_of_week, period_no, class_id FROM lessons
                  WHERE id = ?1 AND term_id = ?2 AND teacher_id = ?3",
                params![id, term_id, input.teacher_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;
        let Some((d, p, c)) = old else {
            return Err(AppError::not_found("고칠 수업을 찾을 수 없습니다."));
        };
        // 고치는 중인 수업은 빼고 본다
        rows.retain(|r| !(r.day_of_week == d && r.period_no == p && r.class_id == c));
    }
    rows.push(LessonRow {
        day_of_week: input.day_of_week,
        period_no: input.period_no,
        class_id: input.class_id,
        subject_id: input.subject_id,
        lesson_type: input.lesson_type.clone(),
        note: input.note.clone(),
    });
    validate(conn, term_id, &rows)?;

    let repl = replaces_homeroom(&input.lesson_type) as i64;

    match input.id {
        Some(id) => {
            conn.execute(
                "UPDATE lessons
                    SET day_of_week = ?2, period_no = ?3, class_id = ?4, subject_id = ?5,
                        lesson_type = ?6, replaces_homeroom = ?7, note = ?8
                  WHERE id = ?1",
                params![
                    id,
                    input.day_of_week,
                    input.period_no,
                    input.class_id,
                    input.subject_id,
                    input.lesson_type,
                    repl,
                    input.note
                ],
            )?;
            Ok(id)
        }
        None => {
            conn.execute(
                "INSERT INTO lessons
                   (term_id, teacher_id, day_of_week, period_no, class_id,
                    subject_id, lesson_type, replaces_homeroom, note)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    term_id,
                    input.teacher_id,
                    input.day_of_week,
                    input.period_no,
                    input.class_id,
                    input.subject_id,
                    input.lesson_type,
                    repl,
                    input.note
                ],
            )?;
            Ok(conn.last_insert_rowid())
        }
    }
}

/// 수업을 지운다. 반환값은 그 수업을 맡던 교사 id (화면을 다시 그리기 위해).
pub fn delete(conn: &Connection, lesson_id: i64) -> AppResult<i64> {
    let teacher_id: i64 = conn
        .query_row(
            "SELECT teacher_id FROM lessons WHERE id = ?1",
            [lesson_id],
            |r| r.get(0),
        )
        .optional()?
        .ok_or_else(|| AppError::not_found("수업을 찾을 수 없습니다."))?;

    conn.execute("DELETE FROM lessons WHERE id = ?1", [lesson_id])?;
    Ok(teacher_id)
}

/// 한 교사의 시간표를 통째로 바꾼다. (붙여넣기 반영)
pub fn bulk_replace(conn: &Connection, teacher_id: i64, rows: &[LessonRow]) -> AppResult<()> {
    let term_id = school::current_term_id(conn)?;
    ensure_teacher(conn, teacher_id)?;
    validate(conn, term_id, rows)?;

    conn.execute(
        "DELETE FROM lessons WHERE term_id = ?1 AND teacher_id = ?2",
        params![term_id, teacher_id],
    )?;

    let mut ins = conn.prepare(
        "INSERT INTO lessons
           (term_id, teacher_id, day_of_week, period_no, class_id,
            subject_id, lesson_type, replaces_homeroom, note)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
    )?;
    for r in rows {
        ins.execute(params![
            term_id,
            teacher_id,
            r.day_of_week,
            r.period_no,
            r.class_id,
            r.subject_id,
            r.lesson_type,
            replaces_homeroom(&r.lesson_type) as i64,
            r.note
        ])?;
    }
    Ok(())
}

/// 미리보기용 검증. 저장하지 않는다.
pub fn check(conn: &Connection, rows: &[LessonRow]) -> AppResult<Vec<String>> {
    let term_id = school::current_term_id(conn)?;
    collect_problems(conn, term_id, rows)
}

// ============================================================
//  담임 공강 — Phase 6 계산을 미리 눈으로 확인할 수 있게
// ============================================================

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HomeroomFreeSlot {
    pub class_id: i64,
    pub class_label: String,
    pub homeroom_name: Option<String>,
    pub day_of_week: i32,
    pub period_no: i32,
    pub start_min: i32,
    pub end_min: i32,
    /// 이 시간에 그 반에 들어오는 교사
    pub covered_by: String,
    pub subject_name: Option<String>,
}

/// 전담 수업 때문에 담임이 비게 되는 시간 목록.
///
/// Phase 6의 담임 시간표 계산과 같은 규칙(`replaces_homeroom = 1`)을 쓰므로
/// 입력한 시간표가 담임 공강으로 제대로 이어지는지 확인할 수 있다.
pub fn homeroom_free_slots(conn: &Connection) -> AppResult<Vec<HomeroomFreeSlot>> {
    let term_id = school::current_term_id(conn)?;
    let slots = slot_map(conn, term_id)?;

    let mut stmt = conn.prepare(
        "SELECT c.id, c.grade, c.class_no, c.name, hr.name,
                l.day_of_week, l.period_no, t.name, s.name
           FROM lessons l
           JOIN classes  c ON c.id = l.class_id
           JOIN teachers t ON t.id = l.teacher_id
           LEFT JOIN teachers hr ON hr.id = c.homeroom_teacher_id
           LEFT JOIN subjects s  ON s.id = l.subject_id
          WHERE l.term_id = ?1 AND l.replaces_homeroom = 1
          ORDER BY c.grade, c.class_no, l.day_of_week, l.period_no",
    )?;

    let rows: Vec<HomeroomFreeSlot> = stmt
        .query_map([term_id], |r| {
            let grade: i32 = r.get(1)?;
            let class_no: i32 = r.get(2)?;
            let cname: Option<String> = r.get(3)?;
            let day: i32 = r.get(5)?;
            let period: i32 = r.get(6)?;
            let (start_min, end_min) = slots.get(&(grade, day, period)).copied().unwrap_or((0, 0));
            Ok(HomeroomFreeSlot {
                class_id: r.get(0)?,
                class_label: class_short(grade, class_no, cname.as_deref()),
                homeroom_name: r.get(4)?,
                day_of_week: day,
                period_no: period,
                start_min,
                end_min,
                covered_by: r.get(7)?,
                subject_name: r.get(8)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;

    Ok(rows)
}

// ============================================================
//  다음 단계로 넘어갈 수 있는지
// ============================================================

/// 반드시 고쳐야 하는 것만 돌려준다 (시간 겹침, 시정표에 없는 교시).
pub fn readiness(conn: &Connection) -> AppResult<Vec<String>> {
    let term_id = school::current_term_id(conn)?;
    let mut out = Vec::new();

    let mut stmt = conn.prepare(
        "SELECT DISTINCT l.teacher_id, t.name FROM lessons l
           JOIN teachers t ON t.id = l.teacher_id
          WHERE l.term_id = ?1 ORDER BY t.name",
    )?;
    let teachers: Vec<(i64, String)> = stmt
        .query_map([term_id], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;

    for (id, name) in teachers {
        let problems = collect_problems(conn, term_id, &rows_of(conn, term_id, id)?)?;
        for p in problems {
            out.push(format!("{name} 선생님 — {p}"));
        }
    }
    Ok(out)
}

/// 막을 정도는 아니지만 알려 주면 좋은 것.
pub fn warnings(conn: &Connection) -> AppResult<Vec<String>> {
    let term_id = school::current_term_id(conn)?;
    let mut out = Vec::new();

    let mut stmt = conn.prepare(
        "SELECT t.name FROM teachers t
          WHERE t.active = 1 AND t.role_code = 'SPECIAL'
            AND NOT EXISTS (
              SELECT 1 FROM lessons l WHERE l.teacher_id = t.id AND l.term_id = ?1
            )
          ORDER BY t.name",
    )?;
    let empty: Vec<String> = stmt
        .query_map([term_id], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;

    if !empty.is_empty() {
        out.push(format!(
            "{}의 시간표가 비어 있습니다. 시간표가 없으면 그 반 담임이 계속 수업 중인 것으로 계산되어 보결 추천이 정확하지 않습니다.",
            empty
                .iter()
                .map(|n| format!("{n} 선생님"))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    // 한 학급의 같은 교시에 두 교사가 들어오는 경우 (합반이면 정상, 실수면 확인 필요)
    let mut stmt = conn.prepare(
        "SELECT c.grade, c.class_no, c.name, l.day_of_week, l.period_no
           FROM lessons l JOIN classes c ON c.id = l.class_id
          WHERE l.term_id = ?1 AND l.replaces_homeroom = 1
          GROUP BY l.class_id, l.day_of_week, l.period_no
         HAVING COUNT(*) > 1
          ORDER BY c.grade, c.class_no",
    )?;
    let dups: Vec<String> = stmt
        .query_map([term_id], |r| {
            let grade: i32 = r.get(0)?;
            let class_no: i32 = r.get(1)?;
            let name: Option<String> = r.get(2)?;
            let day: i32 = r.get(3)?;
            let period: i32 = r.get(4)?;
            Ok(format!(
                "{} {}요일 {}교시",
                class_short(grade, class_no, name.as_deref()),
                day_name(day),
                period
            ))
        })?
        .collect::<rusqlite::Result<_>>()?;

    if !dups.is_empty() {
        out.push(format!(
            "{}에 두 선생님이 함께 들어옵니다. 합반이나 공동수업이면 괜찮지만, 실수라면 한쪽을 지워 주세요.",
            dups.join(", ")
        ));
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::memory_conn;
    use crate::repo::bell;
    use crate::repo::school::{ClassCount, ClassNaming, SchoolInput};
    use crate::repo::teacher::{self, TeacherInput};

    fn hm(h: i32, m: i32) -> i32 {
        h * 60 + m
    }

    /// 저학년(1~2)과 고학년(3~6)의 5교시 시각이 서로 다른 학교를 만든다.
    ///
    ///   저학년 : 1~5교시, 4교시 뒤 점심 -> 5교시 13:00~13:40
    ///   고학년 : 1~6교시, 5교시 뒤 점심 -> 5교시 12:20~13:00, 6교시 13:50~14:30
    fn setup(c: &Connection) -> i64 {
        let term = school::save(
            c,
            &SchoolInput {
                name: "한빛초".into(),
                school_type: "ELEMENTARY".into(),
                min_grade: 1,
                max_grade: 6,
                school_days: vec![1, 2, 3, 4, 5],
                school_year: 2026,
                semester: 2,
                class_counts: (1..=6).map(|g| ClassCount { grade: g, count: 2 }).collect(),
                naming: ClassNaming::default(),
            },
        )
        .unwrap();
        bell::apply_template(c, term, "TWO", &[1, 2, 3, 4, 5]).unwrap();
        term
    }

    fn class_id(c: &Connection, grade: i32, no: i32) -> i64 {
        c.query_row(
            "SELECT id FROM classes WHERE grade=?1 AND class_no=?2",
            params![grade, no],
            |r| r.get(0),
        )
        .unwrap()
    }

    fn subject_id(c: &Connection, name: &str) -> i64 {
        c.query_row("SELECT id FROM subjects WHERE name=?1", [name], |r| r.get(0))
            .unwrap()
    }

    fn make_teacher(c: &Connection, name: &str) -> i64 {
        teacher::upsert(
            c,
            &TeacherInput {
                id: None,
                name: name.into(),
                role_code: "SPECIAL".into(),
                is_substitutable: true,
                memo: None,
                homeroom_class_ids: vec![],
                subject_ids: vec![],
                replace_existing_homeroom: false,
            },
        )
        .unwrap()
    }

    fn row(c: &Connection, day: i32, period: i32, grade: i32, no: i32) -> LessonRow {
        LessonRow {
            day_of_week: day,
            period_no: period,
            class_id: class_id(c, grade, no),
            subject_id: None,
            lesson_type: TYPE_SPECIAL.into(),
            note: None,
        }
    }

    fn lesson(c: &Connection, tid: i64, day: i32, period: i32, grade: i32, no: i32) -> LessonInput {
        LessonInput {
            id: None,
            teacher_id: tid,
            day_of_week: day,
            period_no: period,
            class_id: class_id(c, grade, no),
            subject_id: None,
            lesson_type: TYPE_SPECIAL.into(),
            note: None,
        }
    }

    // ---------- 시각 연결 ----------

    #[test]
    fn 학년별_시정표에서_실제_시각을_가져온다() {
        let c = memory_conn();
        setup(&c);
        let t = make_teacher(&c, "박지훈");

        upsert(&c, &lesson(&c, t, 1, 5, 1, 1)).unwrap(); // 저학년 5교시
        let got = get(&c, t).unwrap();

        assert_eq!(got.lessons.len(), 1);
        let l = &got.lessons[0];
        assert_eq!((l.start_min, l.end_min), (Some(hm(13, 0)), Some(hm(13, 40))));
        assert_eq!(l.class_label, "1-1");
        assert!(l.replaces_homeroom, "전담 수업이면 담임이 비어야 한다");
    }

    #[test]
    fn 같은_교시라도_학년마다_시각이_다르다() {
        let c = memory_conn();
        setup(&c);
        let low = make_teacher(&c, "저학년담당");
        let high = make_teacher(&c, "고학년담당");

        upsert(&c, &lesson(&c, low, 1, 5, 1, 1)).unwrap();
        upsert(&c, &lesson(&c, high, 1, 5, 5, 1)).unwrap();

        let a = get(&c, low).unwrap().lessons[0].clone();
        let b = get(&c, high).unwrap().lessons[0].clone();
        assert_eq!(a.period_no, b.period_no, "둘 다 5교시");
        assert_ne!(
            (a.start_min, a.end_min),
            (b.start_min, b.end_min),
            "그래도 실제 시각은 달라야 한다"
        );
        assert_eq!(b.start_min, Some(hm(12, 20)));
    }

    // ---------- 겹침 판정 (이 단계의 핵심) ----------

    #[test]
    fn 교시_번호가_같아도_시각이_겹치지_않으면_허용한다() {
        // 저학년 5교시 13:00~13:40, 고학년 5교시 12:20~13:00 -> 접점, 겹치지 않음
        let c = memory_conn();
        setup(&c);
        let t = make_teacher(&c, "박지훈");

        upsert(&c, &lesson(&c, t, 1, 5, 5, 1)).unwrap(); // 고학년 5교시
        upsert(&c, &lesson(&c, t, 1, 5, 1, 1)).unwrap(); // 저학년 5교시

        let got = get(&c, t).unwrap();
        assert_eq!(got.lessons.len(), 2, "한 교사가 같은 교시 번호로 두 수업 가능");
        assert!(got.problems.is_empty(), "{:?}", got.problems);
    }

    #[test]
    fn 시각이_겹치면_저장을_막고_어디가_겹치는지_알려준다() {
        // 같은 학년군의 같은 교시 -> 시각이 완전히 같다
        let c = memory_conn();
        setup(&c);
        let t = make_teacher(&c, "박지훈");

        upsert(&c, &lesson(&c, t, 1, 3, 5, 1)).unwrap();
        let e = upsert(&c, &lesson(&c, t, 1, 3, 6, 1)).unwrap_err();

        assert!(e.user_message.contains("겹칩니다"), "{}", e.user_message);
        assert!(e.user_message.contains("월요일"), "{}", e.user_message);
        assert!(e.user_message.contains("5-1"), "{}", e.user_message);
        assert!(e.user_message.contains("6-1"), "{}", e.user_message);

        assert_eq!(get(&c, t).unwrap().lessons.len(), 1, "막힌 수업은 저장되지 않는다");
    }

    #[test]
    fn 다른_요일끼리는_겹치지_않는다() {
        let c = memory_conn();
        setup(&c);
        let t = make_teacher(&c, "박지훈");

        upsert(&c, &lesson(&c, t, 1, 3, 5, 1)).unwrap();
        upsert(&c, &lesson(&c, t, 2, 3, 6, 1)).unwrap();
        assert_eq!(get(&c, t).unwrap().lessons.len(), 2);
    }

    #[test]
    fn 다른_교사끼리는_같은_시간에_수업할_수_있다() {
        let c = memory_conn();
        setup(&c);
        let a = make_teacher(&c, "박지훈");
        let b = make_teacher(&c, "김민수");

        upsert(&c, &lesson(&c, a, 1, 3, 5, 1)).unwrap();
        upsert(&c, &lesson(&c, b, 1, 3, 6, 1)).unwrap();
        assert_eq!(get(&c, a).unwrap().lessons.len(), 1);
        assert_eq!(get(&c, b).unwrap().lessons.len(), 1);
    }

    #[test]
    fn 그_학년에_없는_교시는_막는다() {
        // 저학년은 5교시까지 -> 6교시가 없다
        let c = memory_conn();
        setup(&c);
        let t = make_teacher(&c, "박지훈");

        let e = upsert(&c, &lesson(&c, t, 1, 6, 1, 1)).unwrap_err();
        assert!(e.user_message.contains("6교시가 없습니다"), "{}", e.user_message);
        assert!(e.user_message.contains("1학년 1반"), "{}", e.user_message);
    }

    #[test]
    fn 고학년_6교시는_넣을_수_있다() {
        let c = memory_conn();
        setup(&c);
        let t = make_teacher(&c, "박지훈");
        upsert(&c, &lesson(&c, t, 1, 6, 5, 1)).unwrap();
        let l = get(&c, t).unwrap().lessons[0].clone();
        assert_eq!((l.start_min, l.end_min), (Some(hm(13, 50)), Some(hm(14, 30))));
    }

    // ---------- 수정 / 삭제 ----------

    #[test]
    fn 수업을_고칠_수_있고_자기_자신과는_겹치지_않는다() {
        let c = memory_conn();
        setup(&c);
        let t = make_teacher(&c, "박지훈");
        let id = upsert(&c, &lesson(&c, t, 1, 3, 5, 1)).unwrap();

        // 같은 자리에 학급만 바꿔서 저장 (자기 자신과 겹친다고 막으면 안 된다)
        let mut i = lesson(&c, t, 1, 3, 5, 2);
        i.id = Some(id);
        i.subject_id = Some(subject_id(&c, "체육"));
        upsert(&c, &i).unwrap();

        let got = get(&c, t).unwrap();
        assert_eq!(got.lessons.len(), 1);
        assert_eq!(got.lessons[0].class_label, "5-2");
        assert_eq!(got.lessons[0].subject_name.as_deref(), Some("체육"));
    }

    #[test]
    fn 수업을_지울_수_있다() {
        let c = memory_conn();
        setup(&c);
        let t = make_teacher(&c, "박지훈");
        let id = upsert(&c, &lesson(&c, t, 1, 3, 5, 1)).unwrap();

        let owner = delete(&c, id).unwrap();
        assert_eq!(owner, t);
        assert_eq!(get(&c, t).unwrap().lessons.len(), 0);
        assert!(delete(&c, id).is_err(), "이미 지운 수업은 오류");
    }

    // ---------- 붙여넣기 일괄 반영 ----------

    #[test]
    fn 일괄_반영은_기존_시간표를_완전히_대체한다() {
        let c = memory_conn();
        setup(&c);
        let t = make_teacher(&c, "박지훈");
        upsert(&c, &lesson(&c, t, 1, 1, 5, 1)).unwrap();
        upsert(&c, &lesson(&c, t, 1, 2, 5, 2)).unwrap();

        bulk_replace(&c, t, &[row(&c, 3, 4, 6, 1)]).unwrap();

        let got = get(&c, t).unwrap();
        assert_eq!(got.lessons.len(), 1);
        assert_eq!(got.lessons[0].day_of_week, 3);
        assert_eq!(got.lessons[0].class_label, "6-1");
    }

    #[test]
    fn 겹치는_일괄_반영은_거부하고_기존_시간표를_지키지_않는다() {
        let c = memory_conn();
        setup(&c);
        let t = make_teacher(&c, "박지훈");
        upsert(&c, &lesson(&c, t, 1, 1, 5, 1)).unwrap();

        let bad = vec![row(&c, 1, 3, 5, 1), row(&c, 1, 3, 6, 1)];
        assert!(bulk_replace(&c, t, &bad).is_err());
        // 명령 단위 트랜잭션에서 롤백되므로 기존 수업이 남아 있다
        assert_eq!(get(&c, t).unwrap().lessons.len(), 1);
    }

    #[test]
    fn 미리보기_검증은_저장하지_않는다() {
        let c = memory_conn();
        setup(&c);
        let t = make_teacher(&c, "박지훈");

        let problems = check(&c, &[row(&c, 1, 3, 5, 1), row(&c, 1, 3, 6, 1)]).unwrap();
        assert_eq!(problems.len(), 1);
        assert!(problems[0].contains("겹칩니다"), "{}", problems[0]);
        assert_eq!(get(&c, t).unwrap().lessons.len(), 0, "검증만 했으니 저장 안 됨");
    }

    #[test]
    fn 문제가_여러_개면_모두_모아_보여준다() {
        let c = memory_conn();
        setup(&c);
        let problems = check(
            &c,
            &[
                row(&c, 1, 6, 1, 1), // 저학년에 없는 교시
                row(&c, 1, 3, 5, 1),
                row(&c, 1, 3, 6, 1), // 시각 겹침
            ],
        )
        .unwrap();
        assert_eq!(problems.len(), 2, "{problems:?}");
    }

    // ---------- 담임 공강 ----------

    #[test]
    fn 전담_수업이_들어가면_그_반_담임이_비는_것으로_계산된다() {
        let c = memory_conn();
        setup(&c);

        // 5-1 담임을 지정한다
        let hr = teacher::upsert(
            &c,
            &TeacherInput {
                id: None,
                name: "김담임".into(),
                role_code: "HOMEROOM".into(),
                is_substitutable: true,
                memo: None,
                homeroom_class_ids: vec![class_id(&c, 5, 1)],
                subject_ids: vec![],
                replace_existing_homeroom: false,
            },
        )
        .unwrap();
        assert!(hr > 0);

        let t = make_teacher(&c, "박지훈");
        let mut i = lesson(&c, t, 1, 3, 5, 1);
        i.subject_id = Some(subject_id(&c, "체육"));
        upsert(&c, &i).unwrap();

        let free = homeroom_free_slots(&c).unwrap();
        assert_eq!(free.len(), 1);
        let f = &free[0];
        assert_eq!(f.class_label, "5-1");
        assert_eq!(f.homeroom_name.as_deref(), Some("김담임"));
        assert_eq!(f.covered_by, "박지훈");
        assert_eq!(f.subject_name.as_deref(), Some("체육"));
        assert_eq!((f.start_min, f.end_min), (hm(10, 40), hm(11, 20)));
    }

    #[test]
    fn 공동수업은_담임이_비지_않는다() {
        let c = memory_conn();
        setup(&c);
        let t = make_teacher(&c, "박지훈");

        let mut i = lesson(&c, t, 1, 3, 5, 1);
        i.lesson_type = TYPE_CO.into();
        upsert(&c, &i).unwrap();

        assert!(!get(&c, t).unwrap().lessons[0].replaces_homeroom);
        assert!(
            homeroom_free_slots(&c).unwrap().is_empty(),
            "공동수업은 담임도 함께 들어가므로 공강이 아니다"
        );
    }

    #[test]
    fn 교차수업도_담임을_비운다() {
        let c = memory_conn();
        setup(&c);
        let t = make_teacher(&c, "박지훈");

        let mut i = lesson(&c, t, 1, 3, 5, 1);
        i.lesson_type = TYPE_CROSS.into();
        upsert(&c, &i).unwrap();

        assert!(get(&c, t).unwrap().lessons[0].replaces_homeroom);
        assert_eq!(homeroom_free_slots(&c).unwrap().len(), 1);
    }

    // ---------- 조회 / 안내 ----------

    #[test]
    fn 전담교사를_목록_맨_앞에_보여준다() {
        let c = memory_conn();
        setup(&c);
        teacher::upsert(
            &c,
            &TeacherInput {
                id: None,
                name: "가담임".into(),
                role_code: "HOMEROOM".into(),
                is_substitutable: true,
                memo: None,
                homeroom_class_ids: vec![],
                subject_ids: vec![],
                replace_existing_homeroom: false,
            },
        )
        .unwrap();
        make_teacher(&c, "하전담");

        let ov = overview(&c).unwrap();
        assert_eq!(ov.teachers[0].name, "하전담", "이름 순서와 무관하게 전담이 먼저");
        assert_eq!(ov.teachers[0].role_code, "SPECIAL");
    }

    #[test]
    fn 시정표에서_최대_교시와_시각을_함께_내려준다() {
        let c = memory_conn();
        setup(&c);
        let ov = overview(&c).unwrap();

        assert_eq!(ov.max_period, 6, "고학년이 6교시까지");
        assert!(!ov.bell_missing);
        assert_eq!(ov.school_days, vec![1, 2, 3, 4, 5]);

        let low5 = ov
            .grade_slots
            .iter()
            .find(|g| g.grade == 1 && g.day_of_week == 1 && g.period_no == 5)
            .unwrap();
        assert_eq!((low5.start_min, low5.end_min), (hm(13, 0), hm(13, 40)));
    }

    #[test]
    fn 수업_수가_교사_목록에_표시된다() {
        let c = memory_conn();
        setup(&c);
        let t = make_teacher(&c, "박지훈");
        upsert(&c, &lesson(&c, t, 1, 1, 5, 1)).unwrap();
        upsert(&c, &lesson(&c, t, 1, 2, 5, 2)).unwrap();

        let ov = overview(&c).unwrap();
        assert_eq!(ov.teachers.iter().find(|x| x.id == t).unwrap().lesson_count, 2);
    }

    #[test]
    fn 시간표가_빈_전담교사는_알려준다() {
        let c = memory_conn();
        setup(&c);
        make_teacher(&c, "박지훈");

        let w = warnings(&c).unwrap();
        assert!(w.iter().any(|m| m.contains("박지훈")), "{w:?}");
        assert!(readiness(&c).unwrap().is_empty(), "비어 있는 것만으로는 막지 않는다");
    }

    #[test]
    fn 한_반에_두_전담이_들어오면_알려준다() {
        let c = memory_conn();
        setup(&c);
        let a = make_teacher(&c, "박지훈");
        let b = make_teacher(&c, "김민수");
        upsert(&c, &lesson(&c, a, 1, 3, 5, 1)).unwrap();
        upsert(&c, &lesson(&c, b, 1, 3, 5, 1)).unwrap();

        let w = warnings(&c).unwrap();
        assert!(w.iter().any(|m| m.contains("5-1") && m.contains("함께 들어옵니다")), "{w:?}");
    }

    #[test]
    fn 학교_설정_전에는_안내_오류가_난다() {
        let c = memory_conn();
        let e = overview(&c).unwrap_err();
        assert_eq!(e.code, "SETUP_REQUIRED");
    }
}

