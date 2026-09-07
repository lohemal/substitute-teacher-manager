//! Phase 10 검증 — 동작 옵션이 후보 조회에 곧바로 반영되는가.
//!
//! 설정은 조회할 때마다 다시 읽으므로, 값을 바꾸면 **다음 조회부터** 달라져야 한다.
//! 그리고 **시간이 겹치는 사람은 어떤 설정으로도 후보가 되지 않아야** 한다.

use rusqlite::{params, Connection};

use crate::repo::find as repo_find;
use crate::repo::settings as st;
use crate::db::memory_conn;
use crate::domain::find::{find_candidates, FindRequest};

const MON: &str = "2026-09-07";

fn hm(h: i32, m: i32) -> i32 {
    h * 60 + m
}

/// 저학년(1) 4교시 · 고학년(5) 6교시. 두 학년의 하교 시각이 다르다.
fn school() -> Connection {
    let c = memory_conn();
    c.execute(
        "INSERT INTO school(id, name, min_grade, max_grade) VALUES (1, '한빛초등학교', 1, 6)",
        [],
    )
    .unwrap();
    c.execute(
        "INSERT INTO terms(id, school_year, semester, name, is_current)
         VALUES (1, 2026, 2, '2026학년도 2학기', 1)",
        [],
    )
    .unwrap();

    for (sid, name, grade, count) in [(1i64, "저학년", 1i32, 4i32), (2, "고학년", 5, 6)] {
        c.execute(
            "INSERT INTO bell_schedules(id, term_id, name) VALUES (?1, 1, ?2)",
            params![sid, name],
        )
        .unwrap();
        let mut t = hm(9, 0);
        for p in 1..=count {
            c.execute(
                "INSERT INTO bell_slots(bell_schedule_id, day_of_week, slot_type, period_no, label, start_min, end_min)
                 VALUES (?1, 1, 'PERIOD', ?2, ?3, ?4, ?5)",
                params![sid, p, format!("{p}교시"), t, t + 40],
            )
            .unwrap();
            t += 45;
            if p == count - 2 {
                c.execute(
                    "INSERT INTO bell_slots(bell_schedule_id, day_of_week, slot_type, period_no, label, start_min, end_min)
                     VALUES (?1, 1, 'LUNCH', NULL, '점심', ?2, ?3)",
                    params![sid, t, t + 50],
                )
                .unwrap();
                t += 50;
            }
        }
        c.execute(
            "INSERT INTO grade_bell_map(term_id, grade, bell_schedule_id) VALUES (1, ?1, ?2)",
            params![grade, sid],
        )
        .unwrap();
    }

    for (id, name, role) in [
        (1i64, "김일가", "HOMEROOM"),  // 1학년 담임 — 4교시에 하교
        (2, "이오가", "HOMEROOM"),     // 5학년 담임
        (3, "박전담", "SPECIAL"),      // 전담 — 수업 없음
    ] {
        c.execute(
            "INSERT INTO teachers(id, name, role_code) VALUES (?1, ?2, ?3)",
            params![id, name, role],
        )
        .unwrap();
    }
    c.execute(
        "INSERT INTO classes(id, term_id, grade, class_no, name, homeroom_teacher_id)
         VALUES (1, 1, 1, 1, '가람', 1)",
        [],
    )
    .unwrap();
    c.execute(
        "INSERT INTO classes(id, term_id, grade, class_no, name, homeroom_teacher_id)
         VALUES (2, 1, 5, 1, '가람', 2)",
        [],
    )
    .unwrap();
    c
}

fn set(conn: &Connection, key: &str, value: bool) {
    st::save(
        conn,
        &st::SettingsInput {
            engine: vec![st::EngineInput {
                key: key.to_string(),
                value,
            }],
            auto_backup_mode: None,
            auto_backup_keep: None,
            ..Default::default()
        },
    )
    .unwrap();
}

/// 5학년 6교시(1학년은 이미 하교한 시간)의 후보 이름
fn candidates(conn: &Connection, period: i32) -> Vec<String> {
    let snap = repo_find::snapshot(conn, MON).unwrap();
    let counts = repo_find::counts(conn, MON).unwrap();
    let r = find_candidates(
        &snap,
        &FindRequest {
            class_id: 2,
            slot_type: "PERIOD".into(),
            period_no: Some(period),
            absent_teacher_id: None,
        },
        &counts,
    )
    .unwrap();
    let mut names: Vec<String> = r.eligible.iter().map(|c| c.name.clone()).collect();
    names.sort();
    names
}

fn why(conn: &Connection, period: i32, name: &str) -> String {
    let snap = repo_find::snapshot(conn, MON).unwrap();
    let counts = repo_find::counts(conn, MON).unwrap();
    let r = find_candidates(
        &snap,
        &FindRequest {
            class_id: 2,
            slot_type: "PERIOD".into(),
            period_no: Some(period),
            absent_teacher_id: None,
        },
        &counts,
    )
    .unwrap();
    r.excluded
        .iter()
        .find(|c| c.name == name)
        .map(|c| c.reason_code.clone())
        .unwrap_or_else(|| "ELIGIBLE".into())
}

// ============================================================
//  옵션을 바꾸면 곧바로 반영된다
// ============================================================

#[test]
fn 전담_제외를_켜면_전담이_빠진다() {
    let c = school();
    assert!(candidates(&c, 6).contains(&"박전담".to_string()));

    set(&c, st::INCLUDE_SPECIAL, false);

    assert!(
        !candidates(&c, 6).contains(&"박전담".to_string()),
        "설정을 끄면 곧바로 빠진다"
    );
    assert_eq!(why(&c, 6, "박전담"), "EXCLUDED_BY_OPTION_SPECIAL");

    set(&c, st::INCLUDE_SPECIAL, true);
    assert!(candidates(&c, 6).contains(&"박전담".to_string()), "다시 켜면 돌아온다");
}

#[test]
fn 수업_끝난_담임_제외를_켜면_빠진다() {
    let c = school();
    // 1학년은 4교시(11:15)에 끝난다. 5학년 6교시(11:15 이후)에는 후보다.
    assert!(candidates(&c, 6).contains(&"김일가".to_string()));

    set(&c, st::INCLUDE_AFTER_END, false);

    assert!(!candidates(&c, 6).contains(&"김일가".to_string()));
    assert_eq!(why(&c, 6, "김일가"), "EXCLUDED_BY_OPTION_AFTER_END");
}

#[test]
fn 다른_학년_담임_제외를_켜면_빠진다() {
    let c = school();
    assert!(candidates(&c, 6).contains(&"김일가".to_string()));

    set(&c, st::INCLUDE_OTHER_GRADE, false);

    assert!(!candidates(&c, 6).contains(&"김일가".to_string()));
    assert_eq!(why(&c, 6, "김일가"), "EXCLUDED_BY_OPTION_OTHER_GRADE");
    // 전담은 담임이 아니므로 이 설정과 상관없다
    assert!(candidates(&c, 6).contains(&"박전담".to_string()));
}

#[test]
fn 여러_옵션을_함께_꺼도_각각_이유를_알려준다() {
    let c = school();
    set(&c, st::INCLUDE_SPECIAL, false);
    set(&c, st::INCLUDE_OTHER_GRADE, false);

    assert!(candidates(&c, 6).is_empty(), "둘 다 빠지면 후보가 없다");
    assert_eq!(why(&c, 6, "박전담"), "EXCLUDED_BY_OPTION_SPECIAL");
    assert_eq!(why(&c, 6, "김일가"), "EXCLUDED_BY_OPTION_OTHER_GRADE");
}

#[test]
fn 점심_담임_제외를_끄면_점심에도_담임이_나온다() {
    let c = school();
    let lunch = |conn: &Connection| {
        let snap = repo_find::snapshot(conn, MON).unwrap();
        let counts = repo_find::counts(conn, MON).unwrap();
        let r = find_candidates(
            &snap,
            &FindRequest {
                class_id: 2,
                slot_type: "LUNCH".into(),
                period_no: None,
                absent_teacher_id: None,
            },
            &counts,
        )
        .unwrap();
        r.eligible.iter().any(|x| x.name == "이오가")
    };

    assert!(!lunch(&c), "기본은 자기 반 점심에 담임을 빼는 것");
    set(&c, st::EXCLUDE_LUNCH, false);
    assert!(lunch(&c), "끄면 담임도 후보가 된다");
}

// ============================================================
//  시간 충돌은 어떤 설정으로도 뚫리지 않는다
// ============================================================

#[test]
fn 수업_중인_교사는_모든_설정을_켜도_후보가_아니다() {
    let c = school();
    // 모든 '넣기' 옵션을 켜고 '빼기' 옵션을 끈다 — 가장 느슨한 설정
    for (k, v) in [
        (st::INCLUDE_SPECIAL, true),
        (st::INCLUDE_AFTER_END, true),
        (st::INCLUDE_OTHER_GRADE, true),
        (st::EXCLUDE_LUNCH, false),
        (st::EXCLUDE_RECESS, false),
    ] {
        set(&c, k, v);
    }

    // 1학년 1교시는 09:00~09:40. 같은 시각 5학년 1교시에 1학년 담임을 부를 수 없다.
    let names = candidates(&c, 1);
    assert!(
        !names.contains(&"김일가".to_string()),
        "자기 반 수업 중인 담임은 어떤 설정으로도 후보가 아니다: {names:?}"
    );
    assert_eq!(why(&c, 1, "김일가"), "EXCLUDED_REGULAR_CLASS");
}

#[test]
fn 담임_자동_계산을_끄면_담임이_비어_보인다() {
    let c = school();
    assert!(!candidates(&c, 1).contains(&"김일가".to_string()));

    // 이 옵션은 '수업이 있다고 볼지'를 정하는 것이므로 결과가 달라진다.
    // 그래서 화면에서 켜 두기를 권한다.
    set(&c, st::DERIVE_HOMEROOM, false);
    assert!(
        candidates(&c, 1).contains(&"김일가".to_string()),
        "담임 수업을 계산하지 않으면 늘 비어 있는 것으로 본다"
    );
}
