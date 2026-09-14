//! 학교 기본 정보 / 학년도·학기 / 학급 편성.

use std::collections::HashSet;

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::label::normalize_class_name;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassCount {
    pub grade: i32,
    pub count: i32,
}

pub const NAMING_NUMBER: &str = "NUMBER";
pub const NAMING_NAME: &str = "NAME";

const MAX_CLASS_NAME_LEN: usize = 10;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GradeNames {
    pub grade: i32,
    pub names: Vec<String>,
}

/// 반을 어떻게 부르는지.
///
/// `NUMBER` 면 1반·2반, `NAME` 이면 가람반·나리반처럼 이름으로 부른다.
/// 대부분의 학교는 학년마다 같은 이름을 쓰므로 `shared_names` 하나만 채우면 되고,
/// 학년마다 다른 드문 경우에만 `per_grade`를 켜고 `grade_names`를 쓴다.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassNaming {
    pub mode: String,
    pub per_grade: bool,
    pub shared_names: Vec<String>,
    pub grade_names: Vec<GradeNames>,
}

impl Default for ClassNaming {
    fn default() -> Self {
        Self {
            mode: NAMING_NUMBER.to_string(),
            per_grade: false,
            shared_names: Vec::new(),
            grade_names: Vec::new(),
        }
    }
}

impl ClassNaming {
    /// 이 학년·반의 이름. 숫자 표기면 None.
    pub fn name_for(&self, grade: i32, class_no: i32) -> Option<String> {
        if self.mode != NAMING_NAME || class_no < 1 {
            return None;
        }
        let list = if self.per_grade {
            &self.grade_names.iter().find(|g| g.grade == grade)?.names
        } else {
            &self.shared_names
        };
        list.get((class_no - 1) as usize)
            .map(|s| normalize_class_name(s))
            .filter(|s| !s.is_empty())
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SchoolInput {
    pub name: String,
    pub school_type: String,
    pub min_grade: i32,
    pub max_grade: i32,
    /// 1=월 … 7=일
    pub school_days: Vec<i32>,
    pub school_year: i32,
    pub semester: i32,
    pub class_counts: Vec<ClassCount>,
    #[serde(default)]
    pub naming: ClassNaming,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SchoolView {
    pub name: String,
    pub school_type: String,
    pub min_grade: i32,
    pub max_grade: i32,
    pub school_days: Vec<i32>,
    pub school_year: i32,
    pub semester: i32,
    pub term_id: i64,
    pub term_name: String,
    pub class_counts: Vec<ClassCount>,
    pub naming: ClassNaming,
}

const MAX_CLASSES_PER_GRADE: i32 = 30;

impl SchoolInput {
    pub fn validate(&self) -> AppResult<()> {
        if self.name.trim().is_empty() {
            return Err(AppError::invalid("학교 이름을 입력해 주세요."));
        }
        if self.name.chars().count() > 40 {
            return Err(AppError::invalid("학교 이름이 너무 깁니다. 40자 이내로 입력해 주세요."));
        }
        if !matches!(self.school_type.as_str(), "ELEMENTARY" | "MIDDLE" | "HIGH") {
            return Err(AppError::invalid("학교 구분을 선택해 주세요."));
        }
        if self.min_grade < 1 || self.max_grade > 12 || self.min_grade > self.max_grade {
            return Err(AppError::invalid(
                "학년 범위가 올바르지 않습니다. 시작 학년이 마지막 학년보다 클 수 없습니다.",
            ));
        }
        if self.school_days.is_empty() {
            return Err(AppError::invalid("수업하는 요일을 하나 이상 선택해 주세요."));
        }
        if self.school_days.iter().any(|d| !(1..=7).contains(d)) {
            return Err(AppError::invalid("수업 요일이 올바르지 않습니다."));
        }
        if !(2000..=2100).contains(&self.school_year) {
            return Err(AppError::invalid("학년도를 올바르게 입력해 주세요."));
        }
        if !matches!(self.semester, 1 | 2) {
            return Err(AppError::invalid("학기는 1학기 또는 2학기로 선택해 주세요."));
        }

        for g in self.min_grade..=self.max_grade {
            let cc = self
                .class_counts
                .iter()
                .find(|c| c.grade == g)
                .ok_or_else(|| AppError::invalid(format!("{g}학년의 반 수를 입력해 주세요.")))?;
            if cc.count < 1 {
                return Err(AppError::invalid(format!(
                    "{g}학년의 반 수는 1개 이상이어야 합니다."
                )));
            }
            if cc.count > MAX_CLASSES_PER_GRADE {
                return Err(AppError::invalid(format!(
                    "{g}학년의 반 수가 너무 많습니다. {MAX_CLASSES_PER_GRADE}개 이내로 입력해 주세요."
                )));
            }
        }

        self.validate_naming()?;
        Ok(())
    }

    fn validate_naming(&self) -> AppResult<()> {
        if !matches!(self.naming.mode.as_str(), NAMING_NUMBER | NAMING_NAME) {
            return Err(AppError::invalid("반 이름 방식을 선택해 주세요."));
        }
        if self.naming.mode == NAMING_NUMBER {
            return Ok(());
        }

        for g in self.min_grade..=self.max_grade {
            let count = self
                .class_counts
                .iter()
                .find(|c| c.grade == g)
                .map(|c| c.count)
                .unwrap_or(0);

            let mut seen: HashSet<String> = HashSet::new();
            for n in 1..=count {
                let Some(name) = self.naming.name_for(g, n) else {
                    return Err(AppError::invalid(format!(
                        "{g}학년 {n}번째 반의 이름을 입력해 주세요."
                    )));
                };
                if name.chars().count() > MAX_CLASS_NAME_LEN {
                    return Err(AppError::invalid(format!(
                        "반 이름은 {MAX_CLASS_NAME_LEN}자 이내로 입력해 주세요. ('{name}')"
                    )));
                }
                if !seen.insert(name.clone()) {
                    return Err(AppError::invalid(format!(
                        "{g}학년에 '{name}'이라는 반이 두 개 있습니다. 반 이름은 학년 안에서 서로 달라야 합니다."
                    )));
                }
            }
        }
        Ok(())
    }
}

/// 학교 정보 + 학년도/학기 + 학급을 한 번에 저장한다. 반환값은 현재 학기 id.
pub fn save(conn: &Connection, input: &SchoolInput) -> AppResult<i64> {
    input.validate()?;

    let mut days = input.school_days.clone();
    days.sort_unstable();
    days.dedup();
    let days_csv = days
        .iter()
        .map(|d| d.to_string())
        .collect::<Vec<_>>()
        .join(",");

    conn.execute(
        "INSERT INTO school(id, name, school_type, min_grade, max_grade, school_days, updated_at)
         VALUES (1, ?1, ?2, ?3, ?4, ?5, datetime('now','localtime'))
         ON CONFLICT(id) DO UPDATE SET
            name        = excluded.name,
            school_type = excluded.school_type,
            min_grade   = excluded.min_grade,
            max_grade   = excluded.max_grade,
            school_days = excluded.school_days,
            updated_at  = excluded.updated_at",
        params![
            input.name.trim(),
            input.school_type,
            input.min_grade,
            input.max_grade,
            days_csv
        ],
    )?;

    let term_id = upsert_current_term(conn, input.school_year, input.semester)?;
    sync_classes(conn, term_id, input)?;

    Ok(term_id)
}

/// (학년도, 학기)에 해당하는 학기를 만들거나 찾아서 '현재 학기'로 지정한다.
fn upsert_current_term(conn: &Connection, year: i32, semester: i32) -> AppResult<i64> {
    let name = format!("{year}학년도 {semester}학기");

    // 부분 유니크 인덱스(is_current=1) 때문에 반드시 먼저 모두 해제해야 한다.
    conn.execute("UPDATE terms SET is_current = 0 WHERE is_current = 1", [])?;

    let existing: Option<i64> = conn
        .query_row(
            "SELECT id FROM terms WHERE school_year = ?1 AND semester = ?2",
            params![year, semester],
            |r| r.get(0),
        )
        .optional()?;

    let id = match existing {
        Some(id) => {
            conn.execute(
                "UPDATE terms SET name = ?2, is_current = 1 WHERE id = ?1",
                params![id, name],
            )?;
            id
        }
        None => {
            conn.execute(
                "INSERT INTO terms(school_year, semester, name, is_current)
                 VALUES (?1, ?2, ?3, 1)",
                params![year, semester, name],
            )?;
            conn.last_insert_rowid()
        }
    };
    Ok(id)
}

/// 학급을 입력값에 맞춘다. 줄어든 학급은 삭제하지 않고 비활성화한다(기록 보존).
fn sync_classes(conn: &Connection, term_id: i64, input: &SchoolInput) -> AppResult<()> {
    for g in input.min_grade..=input.max_grade {
        let count = input
            .class_counts
            .iter()
            .find(|c| c.grade == g)
            .map(|c| c.count)
            .unwrap_or(0);

        // 이름을 새로 붙이기 전에 이 학년의 기존 이름을 모두 비운다.
        // (예: 1반↔2반 이름을 서로 바꾸는 경우 유니크 인덱스와 부딪히지 않도록)
        conn.execute(
            "UPDATE classes SET name = NULL WHERE term_id = ?1 AND grade = ?2",
            params![term_id, g],
        )?;

        for n in 1..=count {
            let name = input.naming.name_for(g, n);
            conn.execute(
                "INSERT INTO classes(term_id, grade, class_no, name, active)
                 VALUES (?1, ?2, ?3, ?4, 1)
                 ON CONFLICT(term_id, grade, class_no) DO UPDATE SET
                    name = excluded.name,
                    active = 1",
                params![term_id, g, n, name],
            )?;
        }

        conn.execute(
            "UPDATE classes SET active = 0
              WHERE term_id = ?1 AND grade = ?2 AND class_no > ?3",
            params![term_id, g, count],
        )?;
    }

    // 학년 범위 밖 학급 비활성화
    conn.execute(
        "UPDATE classes SET active = 0
          WHERE term_id = ?1 AND (grade < ?2 OR grade > ?3)",
        params![term_id, input.min_grade, input.max_grade],
    )?;

    Ok(())
}

/// 저장된 학교 정보를 읽는다. 아직 저장 전이면 None.
pub fn get(conn: &Connection) -> AppResult<Option<SchoolView>> {
    let row: Option<(String, String, i32, i32, String)> = conn
        .query_row(
            "SELECT name, school_type, min_grade, max_grade, school_days FROM school WHERE id = 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .optional()?;

    let Some((name, school_type, min_grade, max_grade, days_csv)) = row else {
        return Ok(None);
    };

    let school_days: Vec<i32> = days_csv
        .split(',')
        .filter_map(|s| s.trim().parse::<i32>().ok())
        .collect();

    let term: Option<(i64, i32, i32, String)> = conn
        .query_row(
            "SELECT id, school_year, semester, name FROM terms WHERE is_current = 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()?;

    let Some((term_id, school_year, semester, term_name)) = term else {
        return Ok(None);
    };

    let mut stmt = conn.prepare(
        "SELECT grade, COUNT(*) FROM classes
          WHERE term_id = ?1 AND active = 1
          GROUP BY grade ORDER BY grade",
    )?;
    let class_counts: Vec<ClassCount> = stmt
        .query_map(params![term_id], |r| {
            Ok(ClassCount {
                grade: r.get(0)?,
                count: r.get(1)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;

    let naming = derive_naming(conn, term_id)?;

    Ok(Some(SchoolView {
        name,
        school_type,
        min_grade,
        max_grade,
        school_days,
        school_year,
        semester,
        term_id,
        term_name,
        class_counts,
        naming,
    }))
}

/// 저장된 학급 이름을 보고 어떤 방식을 쓰고 있는지 되짚는다.
///
/// 별도 설정값을 저장하지 않고 자료에서 판단하므로 둘이 어긋날 수 없다.
/// (시정표의 '요일 공통 / 요일별' 판단과 같은 방식)
fn derive_naming(conn: &Connection, term_id: i64) -> AppResult<ClassNaming> {
    let mut stmt = conn.prepare(
        "SELECT grade, class_no, name FROM classes
          WHERE term_id = ?1 AND active = 1
          ORDER BY grade, class_no",
    )?;
    let rows: Vec<(i32, i32, Option<String>)> = stmt
        .query_map(params![term_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<rusqlite::Result<_>>()?;

    if rows.is_empty() || rows.iter().all(|(_, _, n)| n.is_none()) {
        return Ok(ClassNaming::default());
    }

    // 학년별 이름 목록 (class_no 순서대로)
    let mut grade_names: Vec<GradeNames> = Vec::new();
    for (grade, _, name) in &rows {
        if grade_names.last().map(|g| g.grade) != Some(*grade) {
            grade_names.push(GradeNames {
                grade: *grade,
                names: Vec::new(),
            });
        }
        grade_names
            .last_mut()
            .expect("just pushed")
            .names
            .push(name.clone().unwrap_or_default());
    }

    // 모든 학년이 같은 이름 순서를 쓰는지 확인한다.
    // 반 수가 다른 학년은 짧은 쪽까지만 비교한다.
    let longest = grade_names
        .iter()
        .max_by_key(|g| g.names.len())
        .map(|g| g.names.clone())
        .unwrap_or_default();
    let shared = grade_names.iter().all(|g| {
        g.names
            .iter()
            .enumerate()
            .all(|(i, n)| longest.get(i) == Some(n))
    });

    Ok(ClassNaming {
        mode: NAMING_NAME.to_string(),
        per_grade: !shared,
        shared_names: if shared { longest } else { Vec::new() },
        grade_names,
    })
}

/// 현재 학기 id. 설정 전이면 오류.
/// (Phase 3 이후 시정표·시간표 조회에서 사용)
#[allow(dead_code)]
pub fn current_term_id(conn: &Connection) -> AppResult<i64> {
    conn.query_row("SELECT id FROM terms WHERE is_current = 1", [], |r| r.get(0))
        .optional()?
        .ok_or_else(|| {
            AppError::setup_required("학년도와 학기가 아직 설정되지 않았습니다. 학교 기본 설정을 먼저 완료해 주세요.")
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::memory_conn;

    fn input() -> SchoolInput {
        SchoolInput {
            name: "한빛초등학교".into(),
            school_type: "ELEMENTARY".into(),
            min_grade: 1,
            max_grade: 6,
            school_days: vec![1, 2, 3, 4, 5],
            school_year: 2026,
            semester: 2,
            class_counts: (1..=6).map(|g| ClassCount { grade: g, count: 2 }).collect(),
                naming: ClassNaming::default(),
        }
    }

    #[test]
    fn 저장하면_학교_학기_학급이_함께_만들어진다() {
        let c = memory_conn();
        let term_id = save(&c, &input()).unwrap();

        let v = get(&c).unwrap().expect("저장한 학교 정보를 읽을 수 있어야 한다");
        assert_eq!(v.name, "한빛초등학교");
        assert_eq!(v.term_id, term_id);
        assert_eq!(v.term_name, "2026학년도 2학기");
        assert_eq!(v.school_days, vec![1, 2, 3, 4, 5]);

        let total: i64 = c
            .query_row(
                "SELECT COUNT(*) FROM classes WHERE term_id=?1 AND active=1",
                [term_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(total, 12, "6학년 x 2반 = 12개 학급");
    }

    #[test]
    fn 학교_이름_앞뒤_공백은_정리한다() {
        let c = memory_conn();
        let mut i = input();
        i.name = "  한빛초  ".into();
        save(&c, &i).unwrap();
        assert_eq!(get(&c).unwrap().unwrap().name, "한빛초");
    }

    #[test]
    fn 반_수를_줄이면_삭제하지_않고_비활성화한다() {
        let c = memory_conn();
        let term_id = save(&c, &input()).unwrap();

        let mut i = input();
        i.class_counts = (1..=6).map(|g| ClassCount { grade: g, count: 1 }).collect();
        save(&c, &i).unwrap();

        let active: i64 = c
            .query_row(
                "SELECT COUNT(*) FROM classes WHERE term_id=?1 AND active=1",
                [term_id],
                |r| r.get(0),
            )
            .unwrap();
        let all: i64 = c
            .query_row("SELECT COUNT(*) FROM classes WHERE term_id=?1", [term_id], |r| r.get(0))
            .unwrap();

        assert_eq!(active, 6, "각 학년 1반씩만 사용 중");
        assert_eq!(all, 12, "기록 보존을 위해 행 자체는 남아 있어야 한다");
    }

    #[test]
    fn 학년_범위를_줄이면_범위_밖_학급은_비활성화된다() {
        let c = memory_conn();
        let term_id = save(&c, &input()).unwrap();

        let mut i = input();
        i.max_grade = 3;
        i.class_counts = (1..=3).map(|g| ClassCount { grade: g, count: 2 }).collect();
        save(&c, &i).unwrap();

        let active: i64 = c
            .query_row(
                "SELECT COUNT(*) FROM classes WHERE term_id=?1 AND active=1",
                [term_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(active, 6, "1~3학년 x 2반");
    }

    #[test]
    fn 다시_저장해도_학급이_중복_생성되지_않는다() {
        let c = memory_conn();
        let term_id = save(&c, &input()).unwrap();
        let again = save(&c, &input()).unwrap();
        assert_eq!(term_id, again, "같은 학년도·학기면 같은 학기를 재사용한다");

        let all: i64 = c
            .query_row("SELECT COUNT(*) FROM classes WHERE term_id=?1", [term_id], |r| r.get(0))
            .unwrap();
        assert_eq!(all, 12);
    }

    #[test]
    fn 학기를_바꾸면_현재_학기가_하나만_남는다() {
        let c = memory_conn();
        save(&c, &input()).unwrap();

        let mut next = input();
        next.school_year = 2027;
        next.semester = 1;
        let new_id = save(&c, &next).unwrap();

        let current: i64 = c
            .query_row("SELECT COUNT(*) FROM terms WHERE is_current=1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(current, 1, "현재 학기는 항상 하나뿐이어야 한다");
        assert_eq!(current_term_id(&c).unwrap(), new_id);

        let terms: i64 = c.query_row("SELECT COUNT(*) FROM terms", [], |r| r.get(0)).unwrap();
        assert_eq!(terms, 2, "이전 학기 기록은 남아 있어야 한다");
    }

    #[test]
    fn 설정_전에는_현재_학기를_묻는_것이_안내_오류가_된다() {
        let c = memory_conn();
        let e = current_term_id(&c).unwrap_err();
        assert_eq!(e.code, "SETUP_REQUIRED");
        assert!(e.user_message.contains("학년도"));
    }

    /// 화면에서는 학교급을 묻지 않지만 **저장 구조는 세 값을 그대로 받는다.**
    ///
    /// 이 프로그램은 초등학교용이라 화면에 학교급 선택 칸이 없고 새 설정은
    /// 늘 `ELEMENTARY` 로 저장된다. 그렇다고 중·고등학교 값을 거부하도록
    /// 좁히지는 않았다 — 예전 자료를 열지 못하게 되고, 나중에 지원할 때
    /// 다시 넓혀야 하기 때문이다.
    #[test]
    fn 학교급은_세_값을_그대로_받는다() {
        for t in ["ELEMENTARY", "MIDDLE", "HIGH"] {
            let c = memory_conn();
            save(
                &c,
                &SchoolInput {
                    school_type: t.into(),
                    ..input()
                },
            )
            .unwrap_or_else(|e| panic!("{t} 를 받아야 한다: {}", e.user_message));

            // 다시 읽어도 바뀌지 않는다 — 조용히 초등학교로 고치지 않는다
            assert_eq!(get(&c).unwrap().unwrap().school_type, t);
        }
    }

    #[test]
    fn 알_수_없는_학교급은_막는다() {
        let c = memory_conn();
        let e = save(
            &c,
            &SchoolInput {
                school_type: "UNIVERSITY".into(),
                ..input()
            },
        )
        .unwrap_err();
        assert!(e.user_message.contains("학교 구분"), "{}", e.user_message);
    }

    #[test]
    fn 잘못된_입력은_사용자가_이해할_수_있는_문구로_거부한다() {
        let empty_name = SchoolInput { name: "   ".into(), ..input() };
        assert!(empty_name.validate().unwrap_err().user_message.contains("학교 이름"));

        let bad_range = SchoolInput { min_grade: 5, max_grade: 2, ..input() };
        assert!(bad_range.validate().unwrap_err().user_message.contains("학년 범위"));

        let no_days = SchoolInput { school_days: vec![], ..input() };
        assert!(no_days.validate().unwrap_err().user_message.contains("요일"));

        let bad_semester = SchoolInput { semester: 3, ..input() };
        assert!(bad_semester.validate().unwrap_err().user_message.contains("학기"));

        let missing = SchoolInput {
            class_counts: vec![ClassCount { grade: 1, count: 2 }],
            ..input()
        };
        assert!(missing.validate().unwrap_err().user_message.contains("반 수"));

        let zero = SchoolInput {
            class_counts: (1..=6).map(|g| ClassCount { grade: g, count: 0 }).collect(),
            ..input()
        };
        assert!(zero.validate().unwrap_err().user_message.contains("1개 이상"));
    }
}

#[cfg(test)]
mod naming_tests {
    use super::*;
    use crate::db::memory_conn;

    fn base(counts: i32) -> SchoolInput {
        SchoolInput {
            name: "한빛초".into(),
            school_type: "ELEMENTARY".into(),
            min_grade: 1,
            max_grade: 6,
            school_days: vec![1, 2, 3, 4, 5],
            school_year: 2026,
            semester: 2,
            class_counts: (1..=6).map(|g| ClassCount { grade: g, count: counts }).collect(),
            naming: ClassNaming::default(),
        }
    }

    fn shared(names: &[&str]) -> ClassNaming {
        ClassNaming {
            mode: NAMING_NAME.into(),
            per_grade: false,
            shared_names: names.iter().map(|s| s.to_string()).collect(),
            grade_names: Vec::new(),
        }
    }

    fn names_of(c: &Connection, grade: i32) -> Vec<Option<String>> {
        let mut stmt = c
            .prepare("SELECT name FROM classes WHERE grade=?1 AND active=1 ORDER BY class_no")
            .unwrap();
        stmt.query_map([grade], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap()
    }

    #[test]
    fn 기본은_숫자_표기이고_이름을_저장하지_않는다() {
        let c = memory_conn();
        save(&c, &base(2)).unwrap();

        assert_eq!(names_of(&c, 1), vec![None, None]);
        let v = get(&c).unwrap().unwrap();
        assert_eq!(v.naming.mode, NAMING_NUMBER);
    }

    #[test]
    fn 모든_학년에_같은_이름을_붙인다() {
        let c = memory_conn();
        let mut i = base(3);
        i.naming = shared(&["가람", "나리", "다솜"]);
        save(&c, &i).unwrap();

        for g in 1..=6 {
            assert_eq!(
                names_of(&c, g),
                vec![
                    Some("가람".to_string()),
                    Some("나리".to_string()),
                    Some("다솜".to_string())
                ],
                "{g}학년"
            );
        }
    }

    #[test]
    fn 저장한_이름을_다시_읽으면_같은_방식으로_되짚는다() {
        let c = memory_conn();
        let mut i = base(3);
        i.naming = shared(&["가람", "나리", "다솜"]);
        save(&c, &i).unwrap();

        let v = get(&c).unwrap().unwrap();
        assert_eq!(v.naming.mode, NAMING_NAME);
        assert!(!v.naming.per_grade, "학년마다 같으면 공통으로 읽는다");
        assert_eq!(v.naming.shared_names, vec!["가람", "나리", "다솜"]);
    }

    #[test]
    fn 학년마다_다른_이름도_쓸_수_있다() {
        let c = memory_conn();
        let mut i = base(2);
        i.naming = ClassNaming {
            mode: NAMING_NAME.into(),
            per_grade: true,
            shared_names: Vec::new(),
            grade_names: (1..=6)
                .map(|g| GradeNames {
                    grade: g,
                    names: vec![format!("{g}가람"), format!("{g}나리")],
                })
                .collect(),
        };
        save(&c, &i).unwrap();

        assert_eq!(names_of(&c, 1), vec![Some("1가람".into()), Some("1나리".into())]);
        assert_eq!(names_of(&c, 5), vec![Some("5가람".into()), Some("5나리".into())]);

        let v = get(&c).unwrap().unwrap();
        assert!(v.naming.per_grade, "학년마다 다르면 학년별로 읽는다");
        assert_eq!(v.naming.grade_names.len(), 6);
    }

    #[test]
    fn 반_글자를_붙여_입력해도_한_번만_붙는다() {
        let c = memory_conn();
        let mut i = base(2);
        i.naming = shared(&["가람반", "나리반"]);
        save(&c, &i).unwrap();

        assert_eq!(names_of(&c, 1), vec![Some("가람".into()), Some("나리".into())]);
    }

    #[test]
    fn 이름이_비면_저장을_막고_어디가_빈지_알려준다() {
        let c = memory_conn();
        let mut i = base(3);
        i.naming = shared(&["가람", "나리"]); // 3번째가 없다
        let e = save(&c, &i).unwrap_err();
        assert!(e.user_message.contains("3번째"), "{}", e.user_message);
    }

    #[test]
    fn 한_학년에_같은_이름이_두_번이면_막는다() {
        let c = memory_conn();
        let mut i = base(2);
        i.naming = shared(&["가람", "가람"]);
        let e = save(&c, &i).unwrap_err();
        assert!(e.user_message.contains("두 개"), "{}", e.user_message);
    }

    #[test]
    fn 너무_긴_이름은_막는다() {
        let c = memory_conn();
        let mut i = base(1);
        i.naming = shared(&["가나다라마바사아자차카타"]);
        let e = save(&c, &i).unwrap_err();
        assert!(e.user_message.contains("10자"), "{}", e.user_message);
    }

    #[test]
    fn 이름을_서로_바꿔도_저장된다() {
        // 유니크 인덱스와 부딪히지 않는지 (1반↔2반 이름 교환)
        let c = memory_conn();
        let mut i = base(2);
        i.naming = shared(&["가람", "나리"]);
        save(&c, &i).unwrap();

        i.naming = shared(&["나리", "가람"]);
        save(&c, &i).unwrap();

        assert_eq!(names_of(&c, 1), vec![Some("나리".into()), Some("가람".into())]);
    }

    #[test]
    fn 이름_표기에서_숫자_표기로_되돌릴_수_있다() {
        let c = memory_conn();
        let mut i = base(2);
        i.naming = shared(&["가람", "나리"]);
        save(&c, &i).unwrap();

        i.naming = ClassNaming::default();
        save(&c, &i).unwrap();

        assert_eq!(names_of(&c, 1), vec![None, None]);
        assert_eq!(get(&c).unwrap().unwrap().naming.mode, NAMING_NUMBER);
    }

    #[test]
    fn 반_수를_늘리면_새_반에도_이름이_붙는다() {
        let c = memory_conn();
        let mut i = base(2);
        i.naming = shared(&["가람", "나리", "다솜"]);
        save(&c, &i).unwrap();
        assert_eq!(names_of(&c, 1).len(), 2);

        i.class_counts = (1..=6).map(|g| ClassCount { grade: g, count: 3 }).collect();
        save(&c, &i).unwrap();

        assert_eq!(
            names_of(&c, 1),
            vec![Some("가람".into()), Some("나리".into()), Some("다솜".into())]
        );
    }

    #[test]
    fn 학년마다_반_수가_달라도_공통_이름으로_읽는다() {
        let c = memory_conn();
        let mut i = base(3);
        i.class_counts = vec![
            ClassCount { grade: 1, count: 2 },
            ClassCount { grade: 2, count: 3 },
            ClassCount { grade: 3, count: 3 },
            ClassCount { grade: 4, count: 3 },
            ClassCount { grade: 5, count: 3 },
            ClassCount { grade: 6, count: 3 },
        ];
        i.naming = shared(&["가람", "나리", "다솜"]);
        save(&c, &i).unwrap();

        let v = get(&c).unwrap().unwrap();
        assert!(!v.naming.per_grade, "반 수만 다르면 여전히 공통 이름");
        assert_eq!(v.naming.shared_names, vec!["가람", "나리", "다솜"]);
        assert_eq!(names_of(&c, 1), vec![Some("가람".into()), Some("나리".into())]);
    }
}

