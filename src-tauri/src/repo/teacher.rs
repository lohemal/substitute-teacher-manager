//! 교사 관리.
//!
//! ## 삭제하지 않는다는 원칙
//!
//! 교사를 지우면 과거 보결 기록의 '누가 대신 들어갔는지'가 사라진다.
//! 그래서 시간표·보결·부재 기록에 한 번이라도 쓰인 교사는 **삭제를 막고
//! 비활성(active=0)으로만 바꾼다.** 비활성 교사도 id로는 그대로 조회되므로
//! 과거 기록에서 이름이 계속 정상 표시된다.
//!
//! ## 담임과 학급의 연결
//!
//! `classes.homeroom_teacher_id` 한 칸으로 관리한다. 한 학급에 담임은 한 명뿐이며,
//! 이미 담임이 있는 학급을 다른 교사에게 주려면 화면에서 확인을 받은 뒤
//! `replace_existing_homeroom`을 켜서 다시 저장한다.

use std::collections::{HashMap, HashSet};

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use super::school;
use crate::error::{AppError, AppResult};
use crate::label::{class_full, class_short, normalize_class_name};

pub const ROLE_HOMEROOM: &str = "HOMEROOM";
pub const ROLE_SPECIAL: &str = "SPECIAL";
pub const ROLE_OTHER: &str = "OTHER";

const MAX_NAME_LEN: usize = 20;
const MAX_MEMO_LEN: usize = 40;

// ============================================================
//  화면에 보내는 모양
// ============================================================

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassRef {
    pub class_id: i64,
    pub grade: i32,
    pub class_no: i32,
    /// 반 이름. 숫자로 부르는 학교면 None
    pub name: Option<String>,
    /// '5-가람' 또는 '5-1'
    pub label: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubjectRef {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoleRef {
    pub code: String,
    pub label: String,
    pub sort_order: i32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TeacherView {
    pub id: i64,
    pub name: String,
    pub role_code: String,
    pub role_label: String,
    pub is_substitutable: bool,
    pub memo: Option<String>,
    pub active: bool,
    pub homerooms: Vec<ClassRef>,
    pub subjects: Vec<SubjectRef>,
    /// 전담 시간표에 입력된 수업 수 (Phase 5에서 채워진다)
    pub lesson_count: i32,
    /// 지금까지 배정된 보결 횟수
    pub sub_count_total: i32,
    /// 기록에 쓰이고 있어 완전 삭제가 불가능한가
    pub has_records: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassSlot {
    pub class_id: i64,
    pub grade: i32,
    pub class_no: i32,
    pub name: Option<String>,
    /// '5-가람' 또는 '5-1'
    pub label: String,
    /// '5학년 가람반' 또는 '5학년 1반'
    pub full_label: String,
    pub homeroom_teacher_id: Option<i64>,
    pub homeroom_name: Option<String>,
    pub homeroom_active: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TeacherCounts {
    pub total: i32,
    pub homeroom: i32,
    pub special: i32,
    pub other: i32,
    pub substitutable: i32,
    pub inactive: i32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TeacherList {
    pub teachers: Vec<TeacherView>,
    pub roles: Vec<RoleRef>,
    pub subjects: Vec<SubjectRef>,
    pub classes: Vec<ClassSlot>,
    pub counts: TeacherCounts,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetActiveResult {
    pub list: TeacherList,
    /// 비활성으로 바꾸면서 담임이 비게 된 학급
    pub unassigned_classes: Vec<String>,
}

// ============================================================
//  입력
// ============================================================

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TeacherInput {
    pub id: Option<i64>,
    pub name: String,
    pub role_code: String,
    pub is_substitutable: bool,
    pub memo: Option<String>,
    #[serde(default)]
    pub homeroom_class_ids: Vec<i64>,
    #[serde(default)]
    pub subject_ids: Vec<i64>,
    /// 이미 담임이 있는 학급을 가져가도 되는지 (화면에서 확인받은 뒤 true)
    #[serde(default)]
    pub replace_existing_homeroom: bool,
}

/// 명단 붙여넣기·CSV 가져오기에서 쓰는 한 줄.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkTeacherRow {
    pub name: String,
    #[serde(default)]
    pub role_code: Option<String>,
    #[serde(default)]
    pub memo: Option<String>,
    #[serde(default)]
    pub grade: Option<i32>,
    #[serde(default)]
    pub class_no: Option<i32>,
    /// 반 이름으로 부르는 학교용. '가람' 또는 '가람반'
    #[serde(default)]
    pub class_name: Option<String>,
    #[serde(default)]
    pub subject_names: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuickHomeroomEntry {
    pub class_id: i64,
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkResult {
    pub created: i32,
    pub skipped: i32,
    pub problems: Vec<String>,
    pub list: TeacherList,
}

// ============================================================
//  조회
// ============================================================

pub fn list(conn: &Connection) -> AppResult<TeacherList> {
    let term_id = school::current_term_id(conn)?;

    // 구분
    let mut stmt =
        conn.prepare("SELECT code, label, sort_order FROM teacher_roles ORDER BY sort_order")?;
    let roles: Vec<RoleRef> = stmt
        .query_map([], |r| {
            Ok(RoleRef {
                code: r.get(0)?,
                label: r.get(1)?,
                sort_order: r.get(2)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    let role_label: HashMap<String, String> =
        roles.iter().map(|r| (r.code.clone(), r.label.clone())).collect();
    let role_order: HashMap<String, i32> =
        roles.iter().map(|r| (r.code.clone(), r.sort_order)).collect();

    // 과목
    let mut stmt = conn.prepare("SELECT id, name FROM subjects WHERE active = 1 ORDER BY id")?;
    let subjects: Vec<SubjectRef> = stmt
        .query_map([], |r| {
            Ok(SubjectRef {
                id: r.get(0)?,
                name: r.get(1)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;

    // 교사
    let mut stmt = conn.prepare(
        "SELECT id, name, role_code, is_substitutable, memo, active FROM teachers",
    )?;
    struct Row {
        id: i64,
        name: String,
        role_code: String,
        is_substitutable: bool,
        memo: Option<String>,
        active: bool,
    }
    let rows: Vec<Row> = stmt
        .query_map([], |r| {
            Ok(Row {
                id: r.get(0)?,
                name: r.get(1)?,
                role_code: r.get(2)?,
                is_substitutable: r.get::<_, i64>(3)? != 0,
                memo: r.get(4)?,
                active: r.get::<_, i64>(5)? != 0,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;

    // 담임 -> 학급
    let mut stmt = conn.prepare(
        "SELECT homeroom_teacher_id, id, grade, class_no, name FROM classes
          WHERE term_id = ?1 AND active = 1 AND homeroom_teacher_id IS NOT NULL
          ORDER BY grade, class_no",
    )?;
    let mut homerooms: HashMap<i64, Vec<ClassRef>> = HashMap::new();
    let hr_rows: Vec<(i64, i64, i32, i32, Option<String>)> = stmt
        .query_map([term_id], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        })?
        .collect::<rusqlite::Result<_>>()?;
    for (tid, cid, grade, class_no, name) in hr_rows {
        homerooms.entry(tid).or_default().push(ClassRef {
            class_id: cid,
            grade,
            class_no,
            label: class_short(grade, class_no, name.as_deref()),
            name,
        });
    }

    // 교사 -> 과목
    let mut stmt = conn.prepare(
        "SELECT ts.teacher_id, s.id, s.name FROM teacher_subjects ts
           JOIN subjects s ON s.id = ts.subject_id
          ORDER BY ts.is_primary DESC, s.id",
    )?;
    let mut teacher_subjects: HashMap<i64, Vec<SubjectRef>> = HashMap::new();
    let ts_rows: Vec<(i64, i64, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<rusqlite::Result<_>>()?;
    for (tid, sid, name) in ts_rows {
        teacher_subjects
            .entry(tid)
            .or_default()
            .push(SubjectRef { id: sid, name });
    }

    let lesson_counts = count_map(
        conn,
        "SELECT teacher_id, COUNT(*) FROM lessons WHERE term_id = ?1 GROUP BY teacher_id",
        Some(term_id),
    )?;
    let sub_counts = count_map(
        conn,
        "SELECT sub_teacher_id, COUNT(*) FROM substitutions WHERE status = 'ASSIGNED' GROUP BY sub_teacher_id",
        None,
    )?;

    let referenced = referenced_teacher_ids(conn)?;

    // 학급 슬롯 (담임 빠른 등록 화면에서 사용)
    let name_of: HashMap<i64, (String, bool)> = rows
        .iter()
        .map(|r| (r.id, (r.name.clone(), r.active)))
        .collect();
    let mut stmt = conn.prepare(
        "SELECT id, grade, class_no, name, homeroom_teacher_id FROM classes
          WHERE term_id = ?1 AND active = 1 ORDER BY grade, class_no",
    )?;
    let classes: Vec<ClassSlot> = stmt
        .query_map([term_id], |r| {
            let grade: i32 = r.get(1)?;
            let class_no: i32 = r.get(2)?;
            let cname: Option<String> = r.get(3)?;
            let hid: Option<i64> = r.get(4)?;
            let info = hid.and_then(|id| name_of.get(&id).cloned());
            Ok(ClassSlot {
                class_id: r.get(0)?,
                grade,
                class_no,
                label: class_short(grade, class_no, cname.as_deref()),
                full_label: class_full(grade, class_no, cname.as_deref()),
                name: cname,
                homeroom_teacher_id: hid,
                homeroom_name: info.as_ref().map(|(n, _)| n.clone()),
                homeroom_active: info.as_ref().map(|(_, a)| *a),
            })
        })?
        .collect::<rusqlite::Result<_>>()?;

    let mut teachers: Vec<TeacherView> = rows
        .into_iter()
        .map(|r| TeacherView {
            role_label: role_label.get(&r.role_code).cloned().unwrap_or_else(|| r.role_code.clone()),
            homerooms: homerooms.get(&r.id).cloned().unwrap_or_default(),
            subjects: teacher_subjects.get(&r.id).cloned().unwrap_or_default(),
            lesson_count: *lesson_counts.get(&r.id).unwrap_or(&0),
            sub_count_total: *sub_counts.get(&r.id).unwrap_or(&0),
            has_records: referenced.contains(&r.id),
            id: r.id,
            name: r.name,
            role_code: r.role_code,
            is_substitutable: r.is_substitutable,
            memo: r.memo,
            active: r.active,
        })
        .collect();

    // 활성 먼저 -> 구분 순서 -> 담임은 학년·반 순 -> 이름 순
    teachers.sort_by(|a, b| {
        b.active
            .cmp(&a.active)
            .then(
                role_order
                    .get(&a.role_code)
                    .unwrap_or(&99)
                    .cmp(role_order.get(&b.role_code).unwrap_or(&99)),
            )
            .then(homeroom_key(a).cmp(&homeroom_key(b)))
            .then(a.name.cmp(&b.name))
    });

    let counts = TeacherCounts {
        total: teachers.iter().filter(|t| t.active).count() as i32,
        homeroom: teachers
            .iter()
            .filter(|t| t.active && t.role_code == ROLE_HOMEROOM)
            .count() as i32,
        special: teachers
            .iter()
            .filter(|t| t.active && t.role_code == ROLE_SPECIAL)
            .count() as i32,
        other: teachers
            .iter()
            .filter(|t| t.active && t.role_code == ROLE_OTHER)
            .count() as i32,
        substitutable: teachers
            .iter()
            .filter(|t| t.active && t.is_substitutable)
            .count() as i32,
        inactive: teachers.iter().filter(|t| !t.active).count() as i32,
    };

    Ok(TeacherList {
        teachers,
        roles,
        subjects,
        classes,
        counts,
    })
}

fn homeroom_key(t: &TeacherView) -> (i32, i32) {
    t.homerooms
        .first()
        .map(|c| (c.grade, c.class_no))
        .unwrap_or((99, 99))
}

fn count_map(conn: &Connection, sql: &str, arg: Option<i64>) -> AppResult<HashMap<i64, i32>> {
    let mut stmt = conn.prepare(sql)?;
    let rows: Vec<(i64, i32)> = match arg {
        Some(v) => stmt
            .query_map([v], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?,
        None => stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?,
    };
    Ok(rows.into_iter().collect())
}

/// 시간표·보결·부재 기록에 쓰이고 있는 교사 id 모음.
/// 여기 있는 교사는 완전히 지울 수 없다.
fn referenced_teacher_ids(conn: &Connection) -> AppResult<HashSet<i64>> {
    let mut out = HashSet::new();
    for sql in [
        "SELECT DISTINCT teacher_id FROM lessons",
        "SELECT DISTINCT sub_teacher_id FROM substitutions",
        "SELECT DISTINCT absent_teacher_id FROM substitutions WHERE absent_teacher_id IS NOT NULL",
        "SELECT DISTINCT teacher_id FROM absences",
        "SELECT DISTINCT teacher_id FROM teacher_busy_blocks",
    ] {
        let mut stmt = conn.prepare(sql)?;
        let ids: Vec<i64> = stmt
            .query_map([], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        out.extend(ids);
    }
    Ok(out)
}

// ============================================================
//  등록 / 수정
// ============================================================

fn clean_name(name: &str) -> AppResult<String> {
    let n = name.trim();
    if n.is_empty() {
        return Err(AppError::invalid("교사 이름을 입력해 주세요."));
    }
    if n.chars().count() > MAX_NAME_LEN {
        return Err(AppError::invalid(format!(
            "교사 이름은 {MAX_NAME_LEN}자 이내로 입력해 주세요."
        )));
    }
    Ok(n.to_string())
}

fn clean_memo(memo: &Option<String>) -> AppResult<Option<String>> {
    let Some(m) = memo else { return Ok(None) };
    let m = m.trim();
    if m.is_empty() {
        return Ok(None);
    }
    if m.chars().count() > MAX_MEMO_LEN {
        return Err(AppError::invalid(format!(
            "담당 업무는 {MAX_MEMO_LEN}자 이내로 입력해 주세요."
        )));
    }
    Ok(Some(m.to_string()))
}

fn check_role(conn: &Connection, code: &str) -> AppResult<()> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM teacher_roles WHERE code = ?1",
        [code],
        |r| r.get(0),
    )?;
    if n == 0 {
        return Err(AppError::invalid("교사 구분을 선택해 주세요."));
    }
    Ok(())
}

pub fn upsert(conn: &Connection, input: &TeacherInput) -> AppResult<i64> {
    let term_id = school::current_term_id(conn)?;
    let name = clean_name(&input.name)?;
    let memo = clean_memo(&input.memo)?;
    check_role(conn, &input.role_code)?;

    if input.role_code != ROLE_HOMEROOM && !input.homeroom_class_ids.is_empty() {
        return Err(AppError::invalid(
            "담임이 아닌 교사에게는 담당 학급을 지정할 수 없습니다. 구분을 '담임'으로 바꾸거나 담당 학급을 비워 주세요.",
        ));
    }

    let id = match input.id {
        Some(id) => {
            let n = conn.execute(
                "UPDATE teachers
                    SET name = ?2, role_code = ?3, is_substitutable = ?4, memo = ?5,
                        updated_at = datetime('now','localtime')
                  WHERE id = ?1",
                params![
                    id,
                    name,
                    input.role_code,
                    input.is_substitutable as i64,
                    memo
                ],
            )?;
            if n == 0 {
                return Err(AppError::not_found("교사를 찾을 수 없습니다."));
            }
            id
        }
        None => {
            conn.execute(
                "INSERT INTO teachers(name, role_code, is_substitutable, memo)
                 VALUES (?1, ?2, ?3, ?4)",
                params![name, input.role_code, input.is_substitutable as i64, memo],
            )?;
            conn.last_insert_rowid()
        }
    };

    set_subjects(conn, id, &input.subject_ids)?;
    set_homerooms(
        conn,
        term_id,
        id,
        &name,
        &input.homeroom_class_ids,
        input.replace_existing_homeroom,
    )?;

    Ok(id)
}

fn set_subjects(conn: &Connection, teacher_id: i64, subject_ids: &[i64]) -> AppResult<()> {
    conn.execute(
        "DELETE FROM teacher_subjects WHERE teacher_id = ?1",
        [teacher_id],
    )?;
    for (i, sid) in subject_ids.iter().enumerate() {
        let exists: i64 = conn.query_row(
            "SELECT COUNT(*) FROM subjects WHERE id = ?1",
            [sid],
            |r| r.get(0),
        )?;
        if exists == 0 {
            return Err(AppError::invalid("없는 과목이 선택되었습니다. 다시 선택해 주세요."));
        }
        conn.execute(
            "INSERT OR IGNORE INTO teacher_subjects(teacher_id, subject_id, is_primary)
             VALUES (?1, ?2, ?3)",
            params![teacher_id, sid, (i == 0) as i64],
        )?;
    }
    Ok(())
}

fn set_homerooms(
    conn: &Connection,
    term_id: i64,
    teacher_id: i64,
    teacher_name: &str,
    class_ids: &[i64],
    replace: bool,
) -> AppResult<()> {
    // 다른 교사가 이미 담임인 학급이 있는지 확인
    if !replace {
        let mut taken: Vec<String> = Vec::new();
        for cid in class_ids {
            let row: Option<(i32, i32, Option<String>, Option<i64>)> = conn
                .query_row(
                    "SELECT grade, class_no, name, homeroom_teacher_id FROM classes
                      WHERE id = ?1 AND term_id = ?2",
                    params![cid, term_id],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
                )
                .optional()?;
            let Some((grade, class_no, cname, current)) = row else {
                return Err(AppError::not_found("학급을 찾을 수 없습니다."));
            };
            if let Some(other) = current {
                if other != teacher_id {
                    let other_name: String = conn
                        .query_row("SELECT name FROM teachers WHERE id = ?1", [other], |r| r.get(0))
                        .optional()?
                        .unwrap_or_else(|| "다른 교사".to_string());
                    taken.push(format!(
                        "{}은 이미 {} 선생님이 담임입니다",
                        class_full(grade, class_no, cname.as_deref()),
                        other_name
                    ));
                }
            }
        }
        if !taken.is_empty() {
            return Err(AppError::new(
                "HOMEROOM_TAKEN",
                format!(
                    "{}. {} 선생님으로 바꾸시겠습니까?",
                    taken.join(", "),
                    teacher_name
                ),
            )
            .detail(taken.join(" / ")));
        }
    }

    // 이 교사가 맡고 있던 학급을 모두 떼고
    conn.execute(
        "UPDATE classes SET homeroom_teacher_id = NULL
          WHERE term_id = ?1 AND homeroom_teacher_id = ?2",
        params![term_id, teacher_id],
    )?;
    // 새로 지정한 학급을 붙인다
    for cid in class_ids {
        conn.execute(
            "UPDATE classes SET homeroom_teacher_id = ?3
              WHERE id = ?1 AND term_id = ?2",
            params![cid, term_id, teacher_id],
        )?;
    }
    Ok(())
}

// ============================================================
//  비활성 / 삭제
// ============================================================

/// 비활성으로 바꾸면 담임 학급 연결을 끊는다(다른 교사를 담임으로 지정할 수 있도록).
/// 과거 보결 기록은 교사 id를 그대로 갖고 있어 영향받지 않는다.
pub fn set_active(conn: &Connection, id: i64, active: bool) -> AppResult<Vec<String>> {
    let exists: i64 = conn.query_row("SELECT COUNT(*) FROM teachers WHERE id = ?1", [id], |r| {
        r.get(0)
    })?;
    if exists == 0 {
        return Err(AppError::not_found("교사를 찾을 수 없습니다."));
    }

    conn.execute(
        "UPDATE teachers SET active = ?2, updated_at = datetime('now','localtime') WHERE id = ?1",
        params![id, active as i64],
    )?;

    if active {
        return Ok(Vec::new());
    }

    let term_id = school::current_term_id(conn)?;
    let mut stmt = conn.prepare(
        "SELECT grade, class_no, name FROM classes
          WHERE term_id = ?1 AND homeroom_teacher_id = ?2 AND active = 1
          ORDER BY grade, class_no",
    )?;
    let freed: Vec<String> = stmt
        .query_map(params![term_id, id], |r| {
            let grade: i32 = r.get(0)?;
            let class_no: i32 = r.get(1)?;
            let name: Option<String> = r.get(2)?;
            Ok(class_short(grade, class_no, name.as_deref()))
        })?
        .collect::<rusqlite::Result<_>>()?;

    conn.execute(
        "UPDATE classes SET homeroom_teacher_id = NULL
          WHERE term_id = ?1 AND homeroom_teacher_id = ?2",
        params![term_id, id],
    )?;

    Ok(freed)
}

/// 기록이 전혀 없는 교사만 완전히 지운다.
pub fn delete(conn: &Connection, id: i64) -> AppResult<()> {
    let name: String = conn
        .query_row("SELECT name FROM teachers WHERE id = ?1", [id], |r| r.get(0))
        .optional()?
        .ok_or_else(|| AppError::not_found("교사를 찾을 수 없습니다."))?;

    if referenced_teacher_ids(conn)?.contains(&id) {
        return Err(AppError::new(
            "IN_USE",
            format!(
                "{name} 선생님은 시간표나 보결 기록에 이미 쓰이고 있어 지울 수 없습니다. 대신 '비활성'으로 바꾸면 목록에서 숨겨지고 과거 기록은 그대로 남습니다."
            ),
        ));
    }

    conn.execute("DELETE FROM teachers WHERE id = ?1", [id])?;
    Ok(())
}

/// 여러 교사의 보결 배정 대상 여부를 한 번에 바꾼다.
pub fn set_substitutable_bulk(conn: &Connection, ids: &[i64], value: bool) -> AppResult<i32> {
    let mut changed = 0;
    for id in ids {
        changed += conn.execute(
            "UPDATE teachers
                SET is_substitutable = ?2, updated_at = datetime('now','localtime')
              WHERE id = ?1",
            params![id, value as i64],
        )?;
    }
    Ok(changed as i32)
}

/// 여러 교사를 한 번에 활성/비활성으로 바꾼다.
pub fn set_active_bulk(conn: &Connection, ids: &[i64], active: bool) -> AppResult<Vec<String>> {
    let mut freed = Vec::new();
    for id in ids {
        freed.extend(set_active(conn, *id, active)?);
    }
    freed.sort();
    freed.dedup();
    Ok(freed)
}

// ============================================================
//  빠른 입력
// ============================================================

/// 담임이 비어 있는 학급에 이름만 넣어 교사를 함께 만든다.
pub fn quick_add_homerooms(
    conn: &Connection,
    entries: &[QuickHomeroomEntry],
) -> AppResult<(i32, Vec<String>)> {
    let term_id = school::current_term_id(conn)?;
    let mut created = 0;
    let mut problems = Vec::new();

    for e in entries {
        let name = match clean_name(&e.name) {
            Ok(n) => n,
            Err(_) => continue, // 빈 칸은 그냥 건너뛴다
        };

        let row: Option<(i32, i32, Option<String>, Option<i64>)> = conn
            .query_row(
                "SELECT grade, class_no, name, homeroom_teacher_id FROM classes
                  WHERE id = ?1 AND term_id = ?2 AND active = 1",
                params![e.class_id, term_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .optional()?;

        let Some((grade, class_no, cname, current)) = row else {
            problems.push("없는 학급이 있어 건너뛰었습니다.".to_string());
            continue;
        };
        if current.is_some() {
            problems.push(format!(
                "{}은 이미 담임이 있어 건너뛰었습니다.",
                class_full(grade, class_no, cname.as_deref())
            ));
            continue;
        }

        conn.execute(
            "INSERT INTO teachers(name, role_code, is_substitutable) VALUES (?1, ?2, 1)",
            params![name, ROLE_HOMEROOM],
        )?;
        let tid = conn.last_insert_rowid();
        conn.execute(
            "UPDATE classes SET homeroom_teacher_id = ?2 WHERE id = ?1",
            params![e.class_id, tid],
        )?;
        created += 1;
    }

    Ok((created, problems))
}

/// 명단을 한 번에 등록한다. (붙여넣기 / CSV 가져오기가 이 명령을 쓴다)
pub fn bulk_create(conn: &Connection, rows: &[BulkTeacherRow]) -> AppResult<(i32, i32, Vec<String>)> {
    let term_id = school::current_term_id(conn)?;
    let mut created = 0;
    let mut skipped = 0;
    let mut problems: Vec<String> = Vec::new();

    for (i, row) in rows.iter().enumerate() {
        let line = i + 1;
        let name = match clean_name(&row.name) {
            Ok(n) => n,
            Err(_) => {
                skipped += 1;
                continue;
            }
        };

        let role = row.role_code.clone().unwrap_or_else(|| {
            if row.grade.is_some() || row.class_name.is_some() {
                ROLE_HOMEROOM.to_string()
            } else if !row.subject_names.is_empty() {
                ROLE_SPECIAL.to_string()
            } else {
                ROLE_OTHER.to_string()
            }
        });
        if check_role(conn, &role).is_err() {
            problems.push(format!("{line}번째 줄({name}): 교사 구분을 알 수 없어 건너뛰었습니다."));
            skipped += 1;
            continue;
        }

        let memo = clean_memo(&row.memo).unwrap_or(None);

        // 담당 학급 확인. 반 번호('1-2')와 반 이름('1학년 가람반')을 모두 받는다.
        let mut class_ids = Vec::new();
        if let Some(grade) = row.grade {
            let asked = match &row.class_name {
                Some(n) => class_full(grade, 0, Some(&normalize_class_name(n))),
                None => class_full(grade, row.class_no.unwrap_or(0), None),
            };

            let found: Option<(i64, Option<i64>)> = match (&row.class_name, row.class_no) {
                (Some(cn), _) => conn
                    .query_row(
                        "SELECT id, homeroom_teacher_id FROM classes
                          WHERE term_id = ?1 AND grade = ?2 AND name = ?3 AND active = 1",
                        params![term_id, grade, normalize_class_name(cn)],
                        |r| Ok((r.get(0)?, r.get(1)?)),
                    )
                    .optional()?,
                (None, Some(class_no)) => conn
                    .query_row(
                        "SELECT id, homeroom_teacher_id FROM classes
                          WHERE term_id = ?1 AND grade = ?2 AND class_no = ?3 AND active = 1",
                        params![term_id, grade, class_no],
                        |r| Ok((r.get(0)?, r.get(1)?)),
                    )
                    .optional()?,
                (None, None) => None,
            };

            match found {
                None => problems.push(format!(
                    "{line}번째 줄({name}): {asked}을 찾을 수 없어 담당 학급 없이 등록했습니다."
                )),
                Some((_, Some(_))) => problems.push(format!(
                    "{line}번째 줄({name}): {asked}은 이미 담임이 있어 담당 학급 없이 등록했습니다."
                )),
                Some((cid, None)) => class_ids.push(cid),
            }
        }

        // 과목 (없으면 만든다)
        let mut subject_ids = Vec::new();
        for sn in &row.subject_names {
            let sn = sn.trim();
            if sn.is_empty() {
                continue;
            }
            let existing: Option<i64> = conn
                .query_row("SELECT id FROM subjects WHERE name = ?1", [sn], |r| r.get(0))
                .optional()?;
            let sid = match existing {
                Some(id) => id,
                None => {
                    conn.execute("INSERT INTO subjects(name) VALUES (?1)", [sn])?;
                    conn.last_insert_rowid()
                }
            };
            subject_ids.push(sid);
        }

        conn.execute(
            "INSERT INTO teachers(name, role_code, is_substitutable, memo) VALUES (?1, ?2, 1, ?3)",
            params![name, role, memo],
        )?;
        let tid = conn.last_insert_rowid();
        set_subjects(conn, tid, &subject_ids)?;
        for cid in &class_ids {
            conn.execute(
                "UPDATE classes SET homeroom_teacher_id = ?2 WHERE id = ?1",
                params![cid, tid],
            )?;
        }
        created += 1;
    }

    Ok((created, skipped, problems))
}

/// 과목을 새로 만든다(이미 있으면 그 과목을 돌려준다).
pub fn upsert_subject(conn: &Connection, name: &str) -> AppResult<i64> {
    let n = name.trim();
    if n.is_empty() {
        return Err(AppError::invalid("과목 이름을 입력해 주세요."));
    }
    if n.chars().count() > 20 {
        return Err(AppError::invalid("과목 이름은 20자 이내로 입력해 주세요."));
    }
    let existing: Option<i64> = conn
        .query_row("SELECT id FROM subjects WHERE name = ?1", [n], |r| r.get(0))
        .optional()?;
    if let Some(id) = existing {
        conn.execute("UPDATE subjects SET active = 1 WHERE id = ?1", [id])?;
        return Ok(id);
    }
    conn.execute("INSERT INTO subjects(name) VALUES (?1)", [n])?;
    Ok(conn.last_insert_rowid())
}

// ============================================================
//  다음 단계로 넘어갈 수 있는지
// ============================================================

pub fn readiness(conn: &Connection) -> AppResult<Vec<String>> {
    let l = list(conn)?;
    let mut problems = Vec::new();

    if l.counts.total == 0 {
        problems.push("교사를 한 명 이상 등록해 주세요.".to_string());
        return Ok(problems);
    }

    let missing: Vec<String> = l
        .classes
        .iter()
        .filter(|c| c.homeroom_teacher_id.is_none())
        .map(|c| c.label.clone())
        .collect();
    if !missing.is_empty() {
        problems.push(format!(
            "{}의 담임이 지정되지 않았습니다. 담임 수업 시간을 계산할 수 없어 보결 조회가 정확하지 않습니다.",
            missing.join(", ")
        ));
    }

    let inactive_hr: Vec<String> = l
        .classes
        .iter()
        .filter(|c| c.homeroom_active == Some(false))
        .map(|c| c.label.clone())
        .collect();
    if !inactive_hr.is_empty() {
        problems.push(format!(
            "{}의 담임이 비활성 상태입니다. 새 담임을 지정해 주세요.",
            inactive_hr.join(", ")
        ));
    }

    if l.counts.substitutable == 0 {
        problems.push(
            "보결 배정 대상인 교사가 없습니다. 최소 한 명은 보결 대상으로 지정해 주세요.".to_string(),
        );
    }

    let no_subject: Vec<String> = l
        .teachers
        .iter()
        .filter(|t| t.active && t.role_code == ROLE_SPECIAL && t.subjects.is_empty())
        .map(|t| format!("{} 선생님", t.name))
        .collect();
    if !no_subject.is_empty() {
        problems.push(format!(
            "{}의 담당 과목이 비어 있습니다. 전담 시간표를 입력할 때 필요합니다.",
            no_subject.join(", ")
        ));
    }

    Ok(problems)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::memory_conn;
    use crate::repo::school::{ClassCount, ClassNaming, SchoolInput};

    fn setup(c: &Connection) -> i64 {
        school::save(
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
        .unwrap()
    }

    fn input(name: &str, role: &str) -> TeacherInput {
        TeacherInput {
            id: None,
            name: name.into(),
            role_code: role.into(),
            is_substitutable: true,
            memo: None,
            homeroom_class_ids: vec![],
            subject_ids: vec![],
            replace_existing_homeroom: false,
        }
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

    fn find<'a>(l: &'a TeacherList, name: &str) -> &'a TeacherView {
        l.teachers.iter().find(|t| t.name == name).unwrap()
    }

    // ---------- 등록 ----------

    #[test]
    fn 교사를_등록하고_다시_읽을_수_있다() {
        let c = memory_conn();
        setup(&c);

        upsert(&c, &input("김민수", ROLE_HOMEROOM)).unwrap();
        let l = list(&c).unwrap();

        assert_eq!(l.counts.total, 1);
        assert_eq!(l.counts.homeroom, 1);
        let t = find(&l, "김민수");
        assert_eq!(t.role_label, "담임");
        assert!(t.is_substitutable);
        assert!(t.active);
        assert!(!t.has_records, "새 교사는 아직 기록이 없으니 삭제할 수 있다");
    }

    #[test]
    fn 이름_앞뒤_공백은_정리하고_빈_이름은_막는다() {
        let c = memory_conn();
        setup(&c);

        upsert(&c, &input("  박지훈  ", ROLE_SPECIAL)).unwrap();
        assert_eq!(find(&list(&c).unwrap(), "박지훈").name, "박지훈");

        let e = upsert(&c, &input("   ", ROLE_OTHER)).unwrap_err();
        assert!(e.user_message.contains("이름"), "{}", e.user_message);
    }

    #[test]
    fn 알_수_없는_구분은_막는다() {
        let c = memory_conn();
        setup(&c);
        let e = upsert(&c, &input("김민수", "PRINCIPAL")).unwrap_err();
        assert!(e.user_message.contains("구분"), "{}", e.user_message);
    }

    #[test]
    fn 담임에게_학급을_연결한다() {
        let c = memory_conn();
        setup(&c);
        let cid = class_id(&c, 5, 1);

        let mut i = input("김민수", ROLE_HOMEROOM);
        i.homeroom_class_ids = vec![cid];
        upsert(&c, &i).unwrap();

        let l = list(&c).unwrap();
        let t = find(&l, "김민수");
        assert_eq!(t.homerooms.len(), 1);
        assert_eq!(t.homerooms[0].label, "5-1");

        let slot = l.classes.iter().find(|s| s.class_id == cid).unwrap();
        assert_eq!(slot.homeroom_name.as_deref(), Some("김민수"));
    }

    #[test]
    fn 담임이_아니면_학급을_지정할_수_없다() {
        let c = memory_conn();
        setup(&c);
        let mut i = input("박지훈", ROLE_SPECIAL);
        i.homeroom_class_ids = vec![class_id(&c, 5, 1)];

        let e = upsert(&c, &i).unwrap_err();
        assert!(e.user_message.contains("담당 학급"), "{}", e.user_message);
    }

    #[test]
    fn 한_학급에_담임이_겹치면_확인을_요구한다() {
        let c = memory_conn();
        setup(&c);
        let cid = class_id(&c, 5, 1);

        let mut a = input("김민수", ROLE_HOMEROOM);
        a.homeroom_class_ids = vec![cid];
        upsert(&c, &a).unwrap();

        // 같은 학급을 다른 교사에게 주려 하면 막고 이유를 알려준다
        let mut b = input("이영희", ROLE_HOMEROOM);
        b.homeroom_class_ids = vec![cid];
        let e = upsert(&c, &b).unwrap_err();
        assert_eq!(e.code, "HOMEROOM_TAKEN");
        assert!(e.user_message.contains("김민수"), "{}", e.user_message);
        assert!(e.user_message.contains("5학년 1반"), "{}", e.user_message);

        // 확인을 받은 뒤에는 바꿔 준다
        b.replace_existing_homeroom = true;
        upsert(&c, &b).unwrap();

        let l = list(&c).unwrap();
        assert_eq!(
            l.classes.iter().find(|s| s.class_id == cid).unwrap().homeroom_name.as_deref(),
            Some("이영희")
        );
        assert!(find(&l, "김민수").homerooms.is_empty(), "먼저 있던 담임은 학급에서 빠진다");
        assert!(find(&l, "김민수").active, "담임에서 빠져도 교사는 그대로 남는다");
    }

    #[test]
    fn 같은_교사를_다시_저장할_때는_확인을_묻지_않는다() {
        let c = memory_conn();
        setup(&c);
        let cid = class_id(&c, 3, 2);

        let mut i = input("김민수", ROLE_HOMEROOM);
        i.homeroom_class_ids = vec![cid];
        let id = upsert(&c, &i).unwrap();

        i.id = Some(id);
        i.memo = Some("3학년 부장".into());
        upsert(&c, &i).unwrap();

        let t = find(&list(&c).unwrap(), "김민수").clone();
        assert_eq!(t.memo.as_deref(), Some("3학년 부장"));
        assert_eq!(t.homerooms[0].label, "3-2");
    }

    #[test]
    fn 복식학급처럼_두_학급도_맡을_수_있다() {
        let c = memory_conn();
        setup(&c);
        let mut i = input("김민수", ROLE_HOMEROOM);
        i.homeroom_class_ids = vec![class_id(&c, 1, 1), class_id(&c, 2, 1)];
        upsert(&c, &i).unwrap();

        assert_eq!(find(&list(&c).unwrap(), "김민수").homerooms.len(), 2);
    }

    #[test]
    fn 전담교사에게_과목을_여러_개_지정할_수_있다() {
        let c = memory_conn();
        setup(&c);

        let mut i = input("박지훈", ROLE_SPECIAL);
        i.subject_ids = vec![subject_id(&c, "체육"), subject_id(&c, "음악")];
        upsert(&c, &i).unwrap();

        let names: Vec<String> = find(&list(&c).unwrap(), "박지훈")
            .subjects
            .iter()
            .map(|s| s.name.clone())
            .collect();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"체육".to_string()));
        assert!(names.contains(&"음악".to_string()));
    }

    #[test]
    fn 구분을_담임에서_전담으로_바꾸면_학급_연결이_풀린다() {
        let c = memory_conn();
        setup(&c);
        let cid = class_id(&c, 4, 1);

        let mut i = input("김민수", ROLE_HOMEROOM);
        i.homeroom_class_ids = vec![cid];
        let id = upsert(&c, &i).unwrap();

        let mut j = input("김민수", ROLE_SPECIAL);
        j.id = Some(id);
        upsert(&c, &j).unwrap();

        let l = list(&c).unwrap();
        assert!(find(&l, "김민수").homerooms.is_empty());
        assert!(
            l.classes.iter().find(|s| s.class_id == cid).unwrap().homeroom_teacher_id.is_none(),
            "학급은 담임 미지정으로 돌아간다"
        );
    }

    #[test]
    fn 기타_교사는_담당_업무를_적을_수_있다() {
        let c = memory_conn();
        setup(&c);
        let mut i = input("이영희", ROLE_OTHER);
        i.memo = Some("보건교사".into());
        i.is_substitutable = false;
        upsert(&c, &i).unwrap();

        let l = list(&c).unwrap();
        let t = find(&l, "이영희");
        assert_eq!(t.memo.as_deref(), Some("보건교사"));
        assert!(!t.is_substitutable);
        assert_eq!(l.counts.substitutable, 0);
        assert_eq!(l.counts.other, 1);
    }

    // ---------- 비활성 / 삭제 ----------

    #[test]
    fn 비활성으로_바꾸면_담임_학급이_비고_어디가_비었는지_알려준다() {
        let c = memory_conn();
        setup(&c);
        let cid = class_id(&c, 6, 1);

        let mut i = input("김민수", ROLE_HOMEROOM);
        i.homeroom_class_ids = vec![cid];
        let id = upsert(&c, &i).unwrap();

        let freed = set_active(&c, id, false).unwrap();
        assert_eq!(freed, vec!["6-1".to_string()]);

        let l = list(&c).unwrap();
        assert_eq!(l.counts.total, 0, "활성 교사 수에서 빠진다");
        assert_eq!(l.counts.inactive, 1);
        assert!(!find(&l, "김민수").active);
        assert!(l.classes.iter().find(|s| s.class_id == cid).unwrap().homeroom_teacher_id.is_none());
    }

    #[test]
    fn 비활성_교사도_이름으로_계속_조회된다() {
        let c = memory_conn();
        setup(&c);
        let id = upsert(&c, &input("김민수", ROLE_HOMEROOM)).unwrap();
        set_active(&c, id, false).unwrap();

        let name: String = c
            .query_row("SELECT name FROM teachers WHERE id = ?1", [id], |r| r.get(0))
            .unwrap();
        assert_eq!(name, "김민수", "과거 기록에서 당시 이름을 찾을 수 있어야 한다");

        // 다시 활성으로 되돌릴 수 있다
        set_active(&c, id, true).unwrap();
        assert!(find(&list(&c).unwrap(), "김민수").active);
    }

    #[test]
    fn 기록이_없는_교사는_완전히_지울_수_있다() {
        let c = memory_conn();
        setup(&c);
        let id = upsert(&c, &input("김민수", ROLE_OTHER)).unwrap();

        delete(&c, id).unwrap();
        assert_eq!(list(&c).unwrap().teachers.len(), 0);
    }

    #[test]
    fn 보결_기록이_있는_교사는_지울_수_없고_비활성을_안내한다() {
        let c = memory_conn();
        setup(&c);
        let absent = upsert(&c, &input("김민수", ROLE_HOMEROOM)).unwrap();
        let sub = upsert(&c, &input("박지훈", ROLE_SPECIAL)).unwrap();

        c.execute(
            "INSERT INTO substitutions
               (date, day_of_week, grade, class_no, slot_type, slot_label,
                start_min, end_min, absent_teacher_id, sub_teacher_id)
             VALUES ('2026-09-09', 3, 5, 2, 'PERIOD', '6교시', 830, 870, ?1, ?2)",
            params![absent, sub],
        )
        .unwrap();

        for id in [absent, sub] {
            let e = delete(&c, id).unwrap_err();
            assert_eq!(e.code, "IN_USE");
            assert!(e.user_message.contains("비활성"), "{}", e.user_message);
        }

        let l = list(&c).unwrap();
        assert!(find(&l, "김민수").has_records);
        assert_eq!(find(&l, "박지훈").sub_count_total, 1);

        // 비활성은 된다. 그리고 기록은 그대로 남는다.
        set_active(&c, sub, false).unwrap();
        let still: i64 = c
            .query_row("SELECT COUNT(*) FROM substitutions", [], |r| r.get(0))
            .unwrap();
        assert_eq!(still, 1, "교사를 비활성으로 바꿔도 보결 기록은 남는다");
    }

    // ---------- 일괄 작업 ----------

    #[test]
    fn 여러_교사의_보결_대상_여부를_한_번에_바꾼다() {
        let c = memory_conn();
        setup(&c);
        let a = upsert(&c, &input("김민수", ROLE_HOMEROOM)).unwrap();
        let b = upsert(&c, &input("박지훈", ROLE_SPECIAL)).unwrap();
        let d = upsert(&c, &input("이영희", ROLE_OTHER)).unwrap();

        set_substitutable_bulk(&c, &[a, b], false).unwrap();

        let l = list(&c).unwrap();
        assert!(!find(&l, "김민수").is_substitutable);
        assert!(!find(&l, "박지훈").is_substitutable);
        assert!(find(&l, "이영희").is_substitutable, "고르지 않은 교사는 그대로");
        assert_eq!(l.counts.substitutable, 1);

        set_substitutable_bulk(&c, &[a, b, d], true).unwrap();
        assert_eq!(list(&c).unwrap().counts.substitutable, 3);
    }

    #[test]
    fn 담임을_빠르게_한꺼번에_등록한다() {
        let c = memory_conn();
        setup(&c);

        let entries: Vec<QuickHomeroomEntry> = vec![
            QuickHomeroomEntry { class_id: class_id(&c, 1, 1), name: "김민수".into() },
            QuickHomeroomEntry { class_id: class_id(&c, 1, 2), name: "이영희".into() },
            QuickHomeroomEntry { class_id: class_id(&c, 2, 1), name: "  ".into() }, // 빈 칸은 건너뜀
        ];
        let (created, problems) = quick_add_homerooms(&c, &entries).unwrap();

        assert_eq!(created, 2);
        assert!(problems.is_empty());

        let l = list(&c).unwrap();
        assert_eq!(l.counts.homeroom, 2);
        assert_eq!(
            l.classes.iter().find(|s| s.label == "1-1").unwrap().homeroom_name.as_deref(),
            Some("김민수")
        );
        assert!(l.classes.iter().find(|s| s.label == "2-1").unwrap().homeroom_teacher_id.is_none());
    }

    #[test]
    fn 이미_담임이_있는_학급은_빠른_등록에서_건너뛴다() {
        let c = memory_conn();
        setup(&c);
        let cid = class_id(&c, 1, 1);

        let mut i = input("김민수", ROLE_HOMEROOM);
        i.homeroom_class_ids = vec![cid];
        upsert(&c, &i).unwrap();

        let (created, problems) = quick_add_homerooms(
            &c,
            &[QuickHomeroomEntry { class_id: cid, name: "이영희".into() }],
        )
        .unwrap();

        assert_eq!(created, 0);
        assert!(problems[0].contains("1학년 1반"), "{problems:?}");
        assert_eq!(
            list(&c).unwrap().classes.iter().find(|s| s.class_id == cid).unwrap().homeroom_name.as_deref(),
            Some("김민수")
        );
    }

    // ---------- 명단 한 번에 등록 ----------

    fn row(name: &str) -> BulkTeacherRow {
        BulkTeacherRow {
            name: name.into(),
            role_code: None,
            memo: None,
            grade: None,
            class_no: None,
            class_name: None,
            subject_names: vec![],
        }
    }

    #[test]
    fn 명단을_한_번에_등록한다() {
        let c = memory_conn();
        setup(&c);

        let rows = vec![
            BulkTeacherRow { grade: Some(1), class_no: Some(1), ..row("김민수") },
            BulkTeacherRow { grade: Some(1), class_no: Some(2), ..row("이영희") },
            BulkTeacherRow { subject_names: vec!["체육".into()], ..row("박지훈") },
            BulkTeacherRow { memo: Some("보건교사".into()), ..row("최수정") },
            row("   "), // 빈 줄
        ];
        let (created, skipped, problems) = bulk_create(&c, &rows).unwrap();

        assert_eq!(created, 4);
        assert_eq!(skipped, 1);
        assert!(problems.is_empty(), "{problems:?}");

        let l = list(&c).unwrap();
        assert_eq!(find(&l, "김민수").role_code, ROLE_HOMEROOM, "학급이 있으면 담임으로 본다");
        assert_eq!(find(&l, "김민수").homerooms[0].label, "1-1");
        assert_eq!(find(&l, "박지훈").role_code, ROLE_SPECIAL, "과목이 있으면 전담으로 본다");
        assert_eq!(find(&l, "박지훈").subjects[0].name, "체육");
        assert_eq!(find(&l, "최수정").role_code, ROLE_OTHER);
    }

    #[test]
    fn 없는_과목은_만들어_주고_없는_학급은_알려준다() {
        let c = memory_conn();
        setup(&c);

        let rows = vec![
            BulkTeacherRow { subject_names: vec!["로봇과학".into()], ..row("박지훈") },
            BulkTeacherRow { grade: Some(9), class_no: Some(1), ..row("김민수") },
        ];
        let (created, _, problems) = bulk_create(&c, &rows).unwrap();

        assert_eq!(created, 2);
        let l = list(&c).unwrap();
        assert!(l.subjects.iter().any(|s| s.name == "로봇과학"), "새 과목이 만들어진다");
        assert!(problems.iter().any(|m| m.contains("9학년 1반")), "{problems:?}");
        assert!(find(&l, "김민수").homerooms.is_empty(), "없는 학급은 연결하지 않고 교사만 등록");
    }

    // ---------- 다음 단계 확인 ----------

    #[test]
    fn 교사가_없으면_다음_단계로_못_넘어간다() {
        let c = memory_conn();
        setup(&c);
        let p = readiness(&c).unwrap();
        assert_eq!(p.len(), 1);
        assert!(p[0].contains("한 명 이상"), "{}", p[0]);
    }

    #[test]
    fn 담임_미지정_학급이_있으면_알려준다() {
        let c = memory_conn();
        setup(&c);
        let mut i = input("김민수", ROLE_HOMEROOM);
        i.homeroom_class_ids = vec![class_id(&c, 1, 1)];
        upsert(&c, &i).unwrap();

        let p = readiness(&c).unwrap();
        assert!(p.iter().any(|m| m.contains("1-2")), "{p:?}");
    }

    #[test]
    fn 전담교사_과목이_비면_알려준다() {
        let c = memory_conn();
        setup(&c);
        // 모든 학급에 담임 배정
        let entries: Vec<QuickHomeroomEntry> = list(&c)
            .unwrap()
            .classes
            .iter()
            .enumerate()
            .map(|(i, s)| QuickHomeroomEntry {
                class_id: s.class_id,
                name: format!("담임{i}"),
            })
            .collect();
        quick_add_homerooms(&c, &entries).unwrap();
        upsert(&c, &input("박지훈", ROLE_SPECIAL)).unwrap();

        let p = readiness(&c).unwrap();
        assert!(p.iter().any(|m| m.contains("박지훈") && m.contains("담당 과목")), "{p:?}");
    }

    #[test]
    fn 보결_대상이_한_명도_없으면_알려준다() {
        let c = memory_conn();
        setup(&c);
        let mut i = input("김민수", ROLE_OTHER);
        i.is_substitutable = false;
        upsert(&c, &i).unwrap();

        let p = readiness(&c).unwrap();
        assert!(p.iter().any(|m| m.contains("보결 배정 대상")), "{p:?}");
    }

    #[test]
    fn 학교_설정_전에는_안내_오류가_난다() {
        let c = memory_conn();
        let e = list(&c).unwrap_err();
        assert_eq!(e.code, "SETUP_REQUIRED");
    }
}
