//! Phase 10 검증 — 학기 날짜와 새 학기 시작.

use super::*;
use crate::db::memory_conn;

fn base() -> Connection {
    let c = memory_conn();
    c.execute(
        "INSERT INTO school(id, name, min_grade, max_grade) VALUES (1, '한빛초등학교', 1, 6)",
        [],
    )
    .unwrap();
    c.execute(
        "INSERT INTO terms(id, school_year, semester, name, is_current)
         VALUES (1, 2026, 1, '2026학년도 1학기', 1)",
        [],
    )
    .unwrap();

    // 시정표 1개 + 교시 4개 + 학년 연결
    c.execute(
        "INSERT INTO bell_schedules(id, term_id, name) VALUES (1, 1, '공통')",
        [],
    )
    .unwrap();
    for p in 1..=4i32 {
        c.execute(
            "INSERT INTO bell_slots(bell_schedule_id, day_of_week, slot_type, period_no, label, start_min, end_min)
             VALUES (1, 1, 'PERIOD', ?1, ?2, ?3, ?4)",
            params![p, format!("{p}교시"), 540 + (p - 1) * 45, 580 + (p - 1) * 45],
        )
        .unwrap();
    }
    c.execute(
        "INSERT INTO grade_bell_map(term_id, grade, bell_schedule_id) VALUES (1, 1, 1)",
        [],
    )
    .unwrap();

    // 교사 · 학급 · 전담 수업
    c.execute(
        "INSERT INTO teachers(id, name, role_code) VALUES (1, '김담임', 'HOMEROOM')",
        [],
    )
    .unwrap();
    c.execute(
        "INSERT INTO teachers(id, name, role_code) VALUES (2, '박전담', 'SPECIAL')",
        [],
    )
    .unwrap();
    c.execute(
        "INSERT INTO classes(id, term_id, grade, class_no, name, homeroom_teacher_id)
         VALUES (1, 1, 1, 1, '가람', 1)",
        [],
    )
    .unwrap();
    let sid: i64 = c
        .query_row("SELECT id FROM subjects WHERE name = '영어'", [], |r| {
            r.get(0)
        })
        .unwrap();
    c.execute(
        "INSERT INTO lessons(term_id, teacher_id, class_id, subject_id, day_of_week, period_no, lesson_type, replaces_homeroom)
         VALUES (1, 2, 1, ?1, 1, 2, 'SPECIAL', 1)",
        [sid],
    )
    .unwrap();

    // 지난 학기 보결 기록 하나
    c.execute(
        "INSERT INTO substitutions
            (term_id, date, day_of_week, class_id, grade, class_no, class_label,
             slot_type, period_no, slot_label, start_min, end_min, sub_teacher_id, status)
         VALUES (1, '2026-04-06', 1, 1, 1, 1, '1-가람', 'PERIOD', 1, '1교시', 540, 580, 2, 'ASSIGNED')",
        [],
    )
    .unwrap();
    c
}

fn new_input(copy_classes: bool, copy_bells: bool, copy_lessons: bool) -> NewTermInput {
    NewTermInput {
        school_year: 2026,
        semester: 2,
        name: None,
        start_date: None,
        end_date: None,
        copy_classes,
        copy_homerooms: copy_classes,
        copy_bells,
        copy_lessons,
        set_current: true,
    }
}

// ============================================================
//  학기 날짜
// ============================================================

#[test]
fn 날짜를_넣지_않으면_학사_일정으로_계산한다() {
    let c = base();
    let t = &list(&c).unwrap()[0];
    assert!(!t.explicit);
    assert_eq!(t.effective_from, "2026-03-01");
    assert_eq!(t.effective_to, "2026-08-31");
}

#[test]
fn 날짜를_직접_넣으면_그것을_쓴다() {
    let c = base();
    save_dates(
        &c,
        &TermDates {
            id: 1,
            name: None,
            start_date: Some("2026-03-02".into()),
            end_date: Some("2026-07-24".into()),
        },
    )
    .unwrap();

    let t = &list(&c).unwrap()[0];
    assert!(t.explicit);
    assert_eq!(t.effective_from, "2026-03-02");
    assert_eq!(t.effective_to, "2026-07-24");
}

#[test]
fn 날짜를_바꿔도_보결_기록은_그대로다() {
    let c = base();
    let before: i64 = c
        .query_row("SELECT COUNT(*) FROM substitutions", [], |r| r.get(0))
        .unwrap();

    save_dates(
        &c,
        &TermDates {
            id: 1,
            name: Some("2026학년도 1학기(수정)".into()),
            start_date: Some("2026-03-02".into()),
            end_date: Some("2026-07-24".into()),
        },
    )
    .unwrap();

    let after: i64 = c
        .query_row("SELECT COUNT(*) FROM substitutions", [], |r| r.get(0))
        .unwrap();
    assert_eq!(before, after);
    assert_eq!(list(&c).unwrap()[0].name, "2026학년도 1학기(수정)");
}

#[test]
fn 거꾸로_된_날짜는_막는다() {
    let c = base();
    let e = save_dates(
        &c,
        &TermDates {
            id: 1,
            name: None,
            start_date: Some("2026-08-31".into()),
            end_date: Some("2026-03-01".into()),
        },
    )
    .unwrap_err();
    assert!(e.user_message.contains("종료일이 시작일보다"));
}

#[test]
fn 시작일만_넣으면_막는다() {
    let c = base();
    let e = save_dates(
        &c,
        &TermDates {
            id: 1,
            name: None,
            start_date: Some("2026-03-01".into()),
            end_date: None,
        },
    )
    .unwrap_err();
    assert!(e.user_message.contains("함께 넣어"));
}

#[test]
fn 날짜를_비우면_다시_계산값으로_돌아간다() {
    let c = base();
    save_dates(
        &c,
        &TermDates {
            id: 1,
            name: None,
            start_date: Some("2026-03-02".into()),
            end_date: Some("2026-07-24".into()),
        },
    )
    .unwrap();
    save_dates(
        &c,
        &TermDates {
            id: 1,
            name: None,
            start_date: None,
            end_date: None,
        },
    )
    .unwrap();

    let t = &list(&c).unwrap()[0];
    assert!(!t.explicit);
    assert_eq!(t.effective_from, "2026-03-01");
}

// ============================================================
//  새 학기 시작
// ============================================================

#[test]
fn 새_학기를_만들면_지난_학기가_그대로_남는다() {
    let c = base();
    let out = start_new(&c, &new_input(true, true, true)).unwrap();

    assert_eq!(out.name, "2026학년도 2학기");
    let terms = list(&c).unwrap();
    assert_eq!(terms.len(), 2);

    let old = terms.iter().find(|t| t.semester == 1).unwrap();
    assert_eq!(old.class_count, 1, "지난 학기 학급 그대로");
    assert_eq!(old.lesson_count, 1, "지난 학기 수업 그대로");
    assert_eq!(old.sub_count, 1, "지난 학기 보결 기록 그대로");
    assert!(!old.is_current);

    let now = terms.iter().find(|t| t.semester == 2).unwrap();
    assert!(now.is_current);
    assert_eq!(now.sub_count, 0, "새 학기에는 아직 기록이 없다");
}

#[test]
fn 학급과_담임을_가져온다() {
    let c = base();
    let out = start_new(&c, &new_input(true, false, false)).unwrap();
    assert_eq!(out.copied_classes, 1);
    assert_eq!(out.copied_homerooms, 1);

    let (grade, name, hr): (i32, Option<String>, Option<i64>) = c
        .query_row(
            "SELECT grade, name, homeroom_teacher_id FROM classes WHERE term_id = ?1",
            [out.term_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(grade, 1);
    assert_eq!(name.as_deref(), Some("가람"));
    assert_eq!(hr, Some(1));
}

#[test]
fn 담임은_빼고_학급만_가져올_수_있다() {
    let c = base();
    let mut input = new_input(true, false, false);
    input.copy_homerooms = false;
    let out = start_new(&c, &input).unwrap();

    assert_eq!(out.copied_classes, 1);
    assert_eq!(out.copied_homerooms, 0);
    let hr: Option<i64> = c
        .query_row(
            "SELECT homeroom_teacher_id FROM classes WHERE term_id = ?1",
            [out.term_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(hr, None);
    assert!(out.next_steps.iter().any(|s| s.contains("담임")));
}

#[test]
fn 시정표를_가져온다() {
    let c = base();
    let out = start_new(&c, &new_input(false, true, false)).unwrap();
    assert_eq!(out.copied_bells, 1);

    let slots: i64 = c
        .query_row(
            "SELECT COUNT(*) FROM bell_slots s
               JOIN bell_schedules b ON b.id = s.bell_schedule_id
              WHERE b.term_id = ?1",
            [out.term_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(slots, 4, "교시 4개가 그대로 옮겨진다");

    let mapped: i64 = c
        .query_row(
            "SELECT COUNT(*) FROM grade_bell_map WHERE term_id = ?1",
            [out.term_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(mapped, 1, "학년 연결도 함께 옮겨진다");

    // 지난 학기 시정표는 그대로
    let old_slots: i64 = c
        .query_row(
            "SELECT COUNT(*) FROM bell_slots WHERE bell_schedule_id = 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(old_slots, 4);
}

#[test]
fn 전담_시간표는_고르면_가져오고_기본은_새로_입력한다() {
    let c = base();
    let with = start_new(&c, &new_input(true, true, true)).unwrap();
    assert_eq!(with.copied_lessons, 1);
    let n: i64 = c
        .query_row(
            "SELECT COUNT(*) FROM lessons WHERE term_id = ?1",
            [with.term_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(n, 1);

    // 복사하지 않으면 비어 있고, 무엇을 해야 하는지 알려 준다
    let c2 = base();
    let without = start_new(&c2, &new_input(true, true, false)).unwrap();
    assert_eq!(without.copied_lessons, 0);
    assert!(without.next_steps.iter().any(|s| s.contains("전담 시간표")));
}

#[test]
fn 교사와_과목은_복사하지_않고_그대로_이어진다() {
    let c = base();
    let before: i64 = c
        .query_row("SELECT COUNT(*) FROM teachers", [], |r| r.get(0))
        .unwrap();
    let out = start_new(&c, &new_input(true, true, true)).unwrap();
    let after: i64 = c
        .query_row("SELECT COUNT(*) FROM teachers", [], |r| r.get(0))
        .unwrap();

    assert_eq!(before, after, "교사를 복제하지 않는다");
    assert!(
        out.next_steps.iter().any(|s| s.contains("학교 전체 자료")),
        "{:?}",
        out.next_steps
    );
}

#[test]
fn 같은_학년도_학기는_두_번_만들지_않는다() {
    let c = base();
    let e = start_new(
        &c,
        &NewTermInput {
            semester: 1,
            ..new_input(false, false, false)
        },
    )
    .unwrap_err();
    assert!(e.user_message.contains("이미 있습니다"), "{}", e.user_message);
}

#[test]
fn 학년도와_학기_값을_확인한다() {
    let c = base();
    assert!(start_new(
        &c,
        &NewTermInput {
            school_year: 1800,
            ..new_input(false, false, false)
        }
    )
    .is_err());
    assert!(start_new(
        &c,
        &NewTermInput {
            semester: 3,
            ..new_input(false, false, false)
        }
    )
    .is_err());
}

#[test]
fn 현재_학기는_하나뿐이다() {
    let c = base();
    start_new(&c, &new_input(false, false, false)).unwrap();
    let n: i64 = c
        .query_row("SELECT COUNT(*) FROM terms WHERE is_current = 1", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(n, 1);

    set_current(&c, 1).unwrap();
    let cur: i64 = c
        .query_row("SELECT id FROM terms WHERE is_current = 1", [], |r| r.get(0))
        .unwrap();
    assert_eq!(cur, 1);
}

#[test]
fn 새_학기에서_반_이름을_바꿔도_과거_기록은_그대로다() {
    let c = base();
    let out = start_new(&c, &new_input(true, true, true)).unwrap();

    c.execute(
        "UPDATE classes SET name = '한빛' WHERE term_id = ?1",
        [out.term_id],
    )
    .unwrap();

    let label: String = c
        .query_row("SELECT class_label FROM substitutions", [], |r| r.get(0))
        .unwrap();
    assert_eq!(label, "1-가람", "지난 학기 기록은 당시 표기 그대로");
}
