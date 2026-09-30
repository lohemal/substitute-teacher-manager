//! 동학년군 기준이 **실제 자료를 거쳐** 동작하는지 — Case J, 단일/다건 일치,
//! 그리고 기존 학교의 추천 기준 설정이 보존되는지.
//!
//! 여기서는 DB에 학교를 만들어 `repo` 를 통째로 지나가게 한다. 순수 계산
//! 부분은 `domain::grade_band_tests` 에서 이미 본다.

use rusqlite::{params, Connection};

use crate::db::memory_conn;
use crate::domain::find::{find_candidates, FindRequest};
use crate::domain::priority::rank_candidates;
use crate::repo::assign as repo_assign;
use crate::repo::find as repo_find;
use crate::repo::priority as repo_priority;

const MON: &str = "2026-09-07";

fn hm(h: i32, m: i32) -> i32 {
    h * 60 + m
}

/// 4·5·6학년 한 반씩. 시정표는 하나(모두 같은 시각)로 단순하게 둔다.
///
/// 교사
///   1 김사담 — 4학년 담임    (6학년의 짝이 **아니다**)
///   2 이오담 — 5학년 담임    (6학년의 짝)
///   3 박육담 — 6학년 담임    (이 사람이 결근한다)
///   4 최전담 — 전담, 그 날 수업 없음
///   5 나전담 — 4학년 전 교시를 맡는다  -> 김사담이 하루 종일 빈다
///   6 다전담 — 5학년 전 교시를 맡는다  -> 이오담이 하루 종일 빈다
///
/// 4·5학년 담임을 비워 두는 까닭은, 이 시험에서 순서를 가르는 것이
/// **학년 관계 하나뿐**이게 하기 위해서다. 시간이 겹쳐 빠지는 것은
/// Case J 에서 따로 본다.
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

    c.execute(
        "INSERT INTO bell_schedules(id, term_id, name) VALUES (1, 1, '기본')",
        [],
    )
    .unwrap();
    let mut t = hm(9, 0);
    for p in 1..=5 {
        c.execute(
            "INSERT INTO bell_slots(bell_schedule_id, day_of_week, slot_type, period_no, label, start_min, end_min)
             VALUES (1, 1, 'PERIOD', ?1, ?2, ?3, ?4)",
            params![p, format!("{p}교시"), t, t + 40],
        )
        .unwrap();
        t += 45;
    }
    for g in 4..=6 {
        c.execute(
            "INSERT INTO grade_bell_map(term_id, grade, bell_schedule_id) VALUES (1, ?1, 1)",
            params![g],
        )
        .unwrap();
    }

    for (id, name, role) in [
        (1i64, "김사담", "HOMEROOM"),
        (2, "이오담", "HOMEROOM"),
        (3, "박육담", "HOMEROOM"),
        (4, "최전담", "SPECIAL"),
        (5, "나전담", "SPECIAL"),
        (6, "다전담", "SPECIAL"),
    ] {
        c.execute(
            "INSERT INTO teachers(id, name, role_code) VALUES (?1, ?2, ?3)",
            params![id, name, role],
        )
        .unwrap();
    }
    for (cid, grade, tid) in [(1i64, 4i32, 1i64), (2, 5, 2), (3, 6, 3)] {
        c.execute(
            "INSERT INTO classes(id, term_id, grade, class_no, name, homeroom_teacher_id)
             VALUES (?1, 1, ?2, 1, '가람', ?3)",
            params![cid, grade, tid],
        )
        .unwrap();
    }

    // 나전담·다전담이 4·5학년 담임 대신 들어간다
    for (tid, cid) in [(5i64, 1i64), (6, 2)] {
        for p in 1..=5 {
            c.execute(
                "INSERT INTO lessons(term_id, class_id, teacher_id, day_of_week, period_no, lesson_type, replaces_homeroom)
                 VALUES (1, ?1, ?2, 1, ?3, 'SPECIAL', 1)",
                params![cid, tid, p],
            )
            .unwrap();
        }
    }
    c
}

/// 추천 기준을 이 순서로만 켠다.
fn set_rules(conn: &Connection, keys: &[&str]) {
    let all = [
        "SAME_GRADE",
        "SAME_GRADE_BAND",
        "FEWEST_TODAY",
        "FEWEST_MONTH",
        "FEWEST_TOTAL",
        "PREFER_SPECIAL",
        "PREFER_FINISHED",
        "PREFER_LOWER_GRADE",
    ];
    let mut rules: Vec<repo_priority::RuleInput> = keys
        .iter()
        .map(|k| repo_priority::RuleInput {
            rule_key: k.to_string(),
            enabled: true,
        })
        .collect();
    for k in all {
        if !keys.contains(&k) {
            rules.push(repo_priority::RuleInput {
                rule_key: k.to_string(),
                enabled: false,
            });
        }
    }
    repo_priority::save(conn, &rules).unwrap();
}

/// 6학년 가람반 그 교시의 후보를 추천 순서대로.
fn ranked_names(conn: &Connection, period: i32) -> Vec<String> {
    let snap = repo_find::snapshot(conn, MON).unwrap();
    let counts = repo_find::counts(conn, MON).unwrap();
    let mut r = find_candidates(
        &snap,
        &FindRequest {
            class_id: 3,
            slot_type: "PERIOD".into(),
            period_no: Some(period),
            absent_teacher_id: Some(3), // 박육담 결근
        },
        &counts,
    )
    .unwrap();
    rank_candidates(
        &mut r.eligible,
        &repo_priority::settings(conn).unwrap(),
        r.slot.grade,
    );
    r.eligible.iter().map(|c| c.name.clone()).collect()
}

// ============================================================
//  실제 자료를 거쳐도 짝 학년이 앞에 온다
// ============================================================

#[test]
fn 저장된_기준_순서대로_짝_학년이_앞에_온다() {
    let c = school();
    set_rules(&c, &["SAME_GRADE", "SAME_GRADE_BAND"]);

    // 6학년 담임(박육담)이 결근했으므로 4·5학년 담임과 전담이 후보다
    let got = ranked_names(&c, 1);
    assert_eq!(
        got,
        vec!["이오담", "김사담", "최전담"],
        "5학년 담임이 4학년 담임보다 앞이어야 한다"
    );

    // 학년군 기준을 끄면 옛 순서로 돌아간다 (동점 처리: 담임 학년 낮은 순)
    set_rules(&c, &["SAME_GRADE"]);
    assert_eq!(ranked_names(&c, 1), vec!["김사담", "이오담", "최전담"]);
}

// ============================================================
//  Case J — 후보 자격이 먼저다
// ============================================================

#[test]
fn case_j_짝_학년이어도_수업_중이면_후보가_아니다() {
    let c = school();
    set_rules(&c, &["SAME_GRADE", "SAME_GRADE_BAND"]);

    // 5학년 2교시에서 전담을 빼면, 그 시간은 이오담이 직접 수업한다
    c.execute(
        "DELETE FROM lessons WHERE class_id = 2 AND period_no = 2",
        [],
    )
    .unwrap();

    let snap = repo_find::snapshot(&c, MON).unwrap();
    let counts = repo_find::counts(&c, MON).unwrap();
    let r = find_candidates(
        &snap,
        &FindRequest {
            class_id: 3,
            slot_type: "PERIOD".into(),
            period_no: Some(2),
            absent_teacher_id: Some(3),
        },
        &counts,
    )
    .unwrap();

    assert!(
        !r.eligible.iter().any(|x| x.name == "이오담"),
        "자기 반 수업 중인 짝 학년 담임이 후보가 되면 안 된다"
    );
    let why = r
        .excluded
        .iter()
        .find(|x| x.name == "이오담")
        .expect("빠진 이유가 있어야 한다");
    assert_eq!(why.reason_code, "EXCLUDED_REGULAR_CLASS");

    // 추천 기준을 어떻게 놓아도 후보로 되살아나지 않는다
    for keys in [
        vec!["SAME_GRADE_BAND"],
        vec!["SAME_GRADE_BAND", "SAME_GRADE"],
        vec!["PREFER_LOWER_GRADE", "SAME_GRADE_BAND"],
    ] {
        set_rules(&c, &keys);
        assert!(
            !ranked_names(&c, 2).contains(&"이오담".to_string()),
            "{keys:?} 에서도 후보가 되면 안 된다"
        );
    }
}

#[test]
fn case_j_결근_중인_짝_학년_담임도_후보가_아니다() {
    let c = school();
    set_rules(&c, &["SAME_GRADE", "SAME_GRADE_BAND"]);
    c.execute(
        "INSERT INTO absences(term_id, teacher_id, date, is_all_day, status)
         VALUES (1, 2, ?1, 1, 'ACTIVE')",
        params![MON],
    )
    .unwrap();

    let got = ranked_names(&c, 1);
    assert!(!got.contains(&"이오담".to_string()), "{got:?}");
    assert_eq!(got, vec!["김사담", "최전담"]);
}

// ============================================================
//  단일 조회와 다건 배정이 같은 순서를 낸다
// ============================================================

#[test]
fn 단일_조회와_다건_배정의_추천이_같다() {
    let c = school();
    set_rules(&c, &["SAME_GRADE", "SAME_GRADE_BAND"]);
    c.execute(
        "INSERT INTO absences(term_id, teacher_id, date, is_all_day, status)
         VALUES (1, 3, ?1, 1, 'ACTIVE')",
        params![MON],
    )
    .unwrap();

    let plan = repo_assign::day_plan(&c, MON, 3).unwrap();
    assert!(!plan.slots.is_empty(), "6학년 담임의 수업이 잡혀야 한다");

    for slot in &plan.slots {
        let from_plan: Vec<String> = slot.candidates.iter().map(|x| x.name.clone()).collect();
        let from_find = ranked_names(&c, slot.period_no.unwrap());
        assert_eq!(
            from_plan, from_find,
            "{} 의 추천이 단일 조회와 달라졌다",
            slot.slot_label
        );
        assert_eq!(
            from_plan.first().map(|s| s.as_str()),
            Some("이오담"),
            "{} 의 1순위는 짝 학년 담임",
            slot.slot_label
        );
    }

    // 1순위 근거에 학년군이 적혀 있다
    let top = &plan.slots[0].candidates[0];
    assert!(top.reason.contains("동학년군"), "{}", top.reason);
}

// ============================================================
//  기존 학교의 설정이 그대로 남는다
// ============================================================

#[test]
fn 예전_자료에_새_기준_줄이_없어도_동학년_다음에_보인다() {
    let c = school();
    // v0.1.9 까지의 자료 모습 — 새 기준 줄이 아예 없다
    c.execute("DELETE FROM priority_rules WHERE rule_key = 'SAME_GRADE_BAND'", [])
        .unwrap();
    // 학교가 순서를 바꿔 두었다고 하자
    c.execute(
        "UPDATE priority_rules SET sort_order = 10, enabled = 1 WHERE rule_key = 'PREFER_SPECIAL'",
        [],
    )
    .unwrap();

    let before: Vec<(String, bool)> = repo_priority::settings(&c)
        .unwrap()
        .into_iter()
        .map(|s| (s.rule_key, s.enabled))
        .collect();

    let v = repo_priority::view(&c).unwrap();
    let keys: Vec<&str> = v.rules.iter().map(|r| r.rule_key.as_str()).collect();

    // 동학년 바로 다음 자리에 꺼진 채로 나타난다
    let i = keys.iter().position(|k| *k == "SAME_GRADE").unwrap();
    assert_eq!(keys[i + 1], "SAME_GRADE_BAND");
    assert!(
        !v.rules
            .iter()
            .find(|r| r.rule_key == "SAME_GRADE_BAND")
            .unwrap()
            .enabled,
        "예전 학교에서는 꺼져 있어야 한다"
    );

    // 원래 있던 기준들끼리의 순서와 켬/끔은 하나도 달라지지 않는다
    let after: Vec<(String, bool)> = v
        .rules
        .iter()
        .filter(|r| r.rule_key != "SAME_GRADE_BAND")
        .map(|r| (r.rule_key.clone(), r.enabled))
        .collect();
    assert_eq!(before, after, "기존 순서와 켬/끔이 보존되어야 한다");

    // 그리고 실제 추천에도 끼어들지 않는다 (저장된 줄이 없으므로)
    assert!(
        !repo_priority::settings(&c)
            .unwrap()
            .iter()
            .any(|s| s.rule_key == "SAME_GRADE_BAND"),
        "켜지 않았으므로 정렬에 쓰이지 않는다"
    );
}

#[test]
fn 화면을_여러_번_열어도_기준이_늘어나지_않는다() {
    let c = school();
    c.execute("DELETE FROM priority_rules WHERE rule_key = 'SAME_GRADE_BAND'", [])
        .unwrap();

    let n = repo_priority::view(&c).unwrap().rules.len();
    for _ in 0..5 {
        assert_eq!(repo_priority::view(&c).unwrap().rules.len(), n);
    }
    // 아직 DB 에는 줄이 생기지 않았다
    let rows: i64 = c
        .query_row(
            "SELECT COUNT(*) FROM priority_rules WHERE rule_key = 'SAME_GRADE_BAND'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(rows, 0, "화면을 여는 것만으로 자료를 쓰지 않는다");

    // 저장하면 그때 한 줄만 생긴다. 여러 번 저장해도 마찬가지다.
    for _ in 0..3 {
        let v = repo_priority::view(&c).unwrap();
        let input: Vec<repo_priority::RuleInput> = v
            .rules
            .iter()
            .map(|r| repo_priority::RuleInput {
                rule_key: r.rule_key.clone(),
                enabled: r.enabled || r.rule_key == "SAME_GRADE_BAND",
            })
            .collect();
        repo_priority::save(&c, &input).unwrap();
    }
    let rows: i64 = c
        .query_row(
            "SELECT COUNT(*) FROM priority_rules WHERE rule_key = 'SAME_GRADE_BAND'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(rows, 1, "몇 번을 저장해도 한 줄이다");
    assert_eq!(repo_priority::view(&c).unwrap().rules.len(), n);
}

#[test]
fn 새로_만든_자료에는_동학년_다음에_들어_있다() {
    let c = memory_conn();
    let keys: Vec<String> = repo_priority::view(&c)
        .unwrap()
        .rules
        .into_iter()
        .map(|r| r.rule_key)
        .collect();
    assert_eq!(keys[0], "SAME_GRADE");
    assert_eq!(keys[1], "SAME_GRADE_BAND");
    assert!(repo_priority::view(&c).unwrap().is_default, "설치 직후는 기본값");

    // 기본은 꺼짐이므로 정렬에 쓰이지 않는다
    let on: Vec<String> = repo_priority::settings(&c)
        .unwrap()
        .into_iter()
        .filter(|s| s.enabled)
        .map(|s| s.rule_key)
        .collect();
    assert_eq!(on, ["SAME_GRADE", "FEWEST_TODAY", "FEWEST_TOTAL"]);
}
