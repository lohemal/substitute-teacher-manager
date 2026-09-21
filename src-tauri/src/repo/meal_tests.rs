//! 전담교사 식사시간 — 설정 저장과 조회.
//!
//! 우리 학교 시정표를 그대로 쓴다.
//!
//! ```text
//!          저학년(1·2·3)      고학년(4·5·6)
//!  점심    12:10~13:10       13:10~14:00
//!  5교시   13:10~13:50       12:20~13:00
//! ```

use rusqlite::params;

use super::*;
use crate::db::memory_conn;

fn hm(h: i32, m: i32) -> i32 {
    h * 60 + m
}

const LOW_BELL: i64 = 1;
const HIGH_BELL: i64 = 2;
const SPECIAL: i64 = 100;

struct School {
    conn: Connection,
}

impl School {
    fn new() -> Self {
        let conn = memory_conn();
        conn.execute(
            "INSERT INTO school(id, name, min_grade, max_grade, school_days)
             VALUES (1, '한빛초등학교', 1, 6, '1,2,3,4,5')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO terms(id, school_year, semester, name, is_current)
             VALUES (1, 2026, 2, '2026학년도 2학기', 1)",
            [],
        )
        .unwrap();

        for (id, name) in [(LOW_BELL, "저학년 시정표"), (HIGH_BELL, "고학년 시정표")] {
            conn.execute(
                "INSERT INTO bell_schedules(id, term_id, name) VALUES (?1, 1, ?2)",
                params![id, name],
            )
            .unwrap();
        }

        for day in 1..=5 {
            // 공통 1~4교시
            for (bell, _) in [(LOW_BELL, ()), (HIGH_BELL, ())] {
                let mut t = hm(9, 0);
                for p in 1..=4 {
                    conn.execute(
                        "INSERT INTO bell_slots(bell_schedule_id, day_of_week, slot_type, period_no, label, start_min, end_min)
                         VALUES (?1, ?2, 'PERIOD', ?3, ?4, ?5, ?6)",
                        params![bell, day, p, format!("{p}교시"), t, t + 40],
                    )
                    .unwrap();
                    t += 50;
                }
            }
            // 저학년: 점심 12:10~13:10 → 5교시 13:10~13:50
            conn.execute(
                "INSERT INTO bell_slots(bell_schedule_id, day_of_week, slot_type, period_no, label, start_min, end_min)
                 VALUES (?1, ?2, 'LUNCH', NULL, '점심', ?3, ?4)",
                params![LOW_BELL, day, hm(12, 10), hm(13, 10)],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO bell_slots(bell_schedule_id, day_of_week, slot_type, period_no, label, start_min, end_min)
                 VALUES (?1, ?2, 'PERIOD', 5, '5교시', ?3, ?4)",
                params![LOW_BELL, day, hm(13, 10), hm(13, 50)],
            )
            .unwrap();
            // 고학년: 5교시 12:20~13:00 → 점심 13:10~14:00
            conn.execute(
                "INSERT INTO bell_slots(bell_schedule_id, day_of_week, slot_type, period_no, label, start_min, end_min)
                 VALUES (?1, ?2, 'PERIOD', 5, '5교시', ?3, ?4)",
                params![HIGH_BELL, day, hm(12, 20), hm(13, 0)],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO bell_slots(bell_schedule_id, day_of_week, slot_type, period_no, label, start_min, end_min)
                 VALUES (?1, ?2, 'LUNCH', NULL, '점심', ?3, ?4)",
                params![HIGH_BELL, day, hm(13, 10), hm(14, 0)],
            )
            .unwrap();
        }

        for g in 1..=6 {
            let bell = if g <= 3 { LOW_BELL } else { HIGH_BELL };
            conn.execute(
                "INSERT INTO grade_bell_map(term_id, grade, bell_schedule_id) VALUES (1, ?1, ?2)",
                params![g, bell],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO classes(id, term_id, grade, class_no, name) VALUES (?1, 1, ?1, 1, '가람')",
                params![g],
            )
            .unwrap();
        }

        conn.execute(
            "INSERT INTO teachers(id, name, role_code) VALUES (?1, '김전담', 'SPECIAL')",
            params![SPECIAL],
        )
        .unwrap();

        Self { conn }
    }

    /// 김전담에게 그 학년 1반 그 교시 수업을 준다.
    fn teach(&self, day: i32, grade: i64, period: i32) {
        self.conn
            .execute(
                "INSERT INTO lessons(term_id, teacher_id, day_of_week, period_no, class_id)
                 VALUES (1, ?1, ?2, ?3, ?4)",
                params![SPECIAL, day, period, grade],
            )
            .unwrap();
    }

    fn day(&self, v: &MealView, day: i32) -> MealDay {
        v.days.iter().find(|d| d.day_of_week == day).unwrap().clone()
    }
}

// ============================================================
//  점심 패턴 뽑기
// ============================================================

#[test]
fn 시정표에서_점심_패턴을_뽑아_학년을_묶는다() {
    let s = School::new();
    let p = patterns(&s.conn).unwrap();

    assert_eq!(p.len(), 2, "저학년·고학년 두 가지");
    assert_eq!(p[0].grade_label, "1·2·3학년", "이른 점심부터");
    assert_eq!(p[0].time_label, "12:10~13:10");
    assert_eq!(p[0].bell_schedule_id, LOW_BELL);
    assert_eq!(p[1].grade_label, "4·5·6학년");
    assert_eq!(p[1].time_label, "13:10~14:00");
    assert_eq!(p[0].by_day.len(), 5, "요일마다 한 줄");
}

#[test]
fn 저학년_고학년이라는_말을_코드에_박아_두지_않는다() {
    // 학년 배정을 바꾸면 묶음도 따라 바뀐다
    let s = School::new();
    s.conn
        .execute(
            "UPDATE grade_bell_map SET bell_schedule_id = ?1 WHERE grade = 4",
            params![LOW_BELL],
        )
        .unwrap();
    let p = patterns(&s.conn).unwrap();
    assert_eq!(p[0].grade_label, "1·2·3·4학년");
    assert_eq!(p[1].grade_label, "5·6학년");
}

#[test]
fn 점심_패턴이_셋_이상인_학교도_다룬다() {
    let s = School::new();
    s.conn
        .execute(
            "INSERT INTO bell_schedules(id, term_id, name) VALUES (3, 1, '6학년 시정표')",
            [],
        )
        .unwrap();
    for day in 1..=5 {
        s.conn
            .execute(
                "INSERT INTO bell_slots(bell_schedule_id, day_of_week, slot_type, period_no, label, start_min, end_min)
                 VALUES (3, ?1, 'LUNCH', NULL, '점심', ?2, ?3)",
                params![day, hm(11, 50), hm(12, 40)],
            )
            .unwrap();
    }
    s.conn
        .execute("UPDATE grade_bell_map SET bell_schedule_id = 3 WHERE grade = 6", [])
        .unwrap();

    let p = patterns(&s.conn).unwrap();
    assert_eq!(p.len(), 3);
    assert_eq!(p[0].time_label, "11:50~12:40", "가장 이른 것부터");
    assert_eq!(p[0].grade_label, "6학년");
}

#[test]
fn 요일마다_점심시간이_다르면_그렇게_알려_준다() {
    let s = School::new();
    s.conn
        .execute(
            "UPDATE bell_slots SET start_min = ?1, end_min = ?2
              WHERE bell_schedule_id = ?3 AND day_of_week = 5 AND slot_type = 'LUNCH'",
            params![hm(12, 0), hm(13, 0), LOW_BELL],
        )
        .unwrap();
    let p = patterns(&s.conn).unwrap();
    assert_eq!(p[0].time_label, "요일마다 다름");
    let fri = p[0].by_day.iter().find(|d| d.day_of_week == 5).unwrap();
    assert_eq!(fri.start_min, hm(12, 0));
}

// ============================================================
//  학교 기본 식사시간
// ============================================================

#[test]
fn 기본_식사시간은_시각이_아니라_시정표를_가리킨다() {
    let s = School::new();
    assert_eq!(default_bell_id(&s.conn).unwrap(), None, "처음에는 정하지 않았다");

    set_default_bell(&s.conn, Some(LOW_BELL)).unwrap();
    assert_eq!(default_bell_id(&s.conn).unwrap(), Some(LOW_BELL));
    assert_eq!(
        default_window(&s.conn, 1).unwrap(),
        Some(Interval::new(hm(12, 10), hm(13, 10)))
    );
}

#[test]
fn 시정표의_점심시간을_고치면_기본_식사시간도_따라_바뀐다() {
    // 계산해 둔 값을 들고 있지 않다는 것이 핵심이다
    let s = School::new();
    set_default_bell(&s.conn, Some(LOW_BELL)).unwrap();
    assert_eq!(default_window(&s.conn, 1).unwrap().unwrap().start, hm(12, 10));

    s.conn
        .execute(
            "UPDATE bell_slots SET start_min = ?1, end_min = ?2
              WHERE bell_schedule_id = ?3 AND slot_type = 'LUNCH'",
            params![hm(12, 0), hm(13, 0), LOW_BELL],
        )
        .unwrap();

    assert_eq!(
        default_window(&s.conn, 1).unwrap(),
        Some(Interval::new(hm(12, 0), hm(13, 0))),
        "옛 값이 남아 있으면 안 된다"
    );
}

#[test]
fn 요일마다_다른_점심시간도_그_요일_값을_쓴다() {
    let s = School::new();
    s.conn
        .execute(
            "UPDATE bell_slots SET start_min = ?1, end_min = ?2
              WHERE bell_schedule_id = ?3 AND day_of_week = 5 AND slot_type = 'LUNCH'",
            params![hm(11, 40), hm(12, 30), LOW_BELL],
        )
        .unwrap();
    set_default_bell(&s.conn, Some(LOW_BELL)).unwrap();

    assert_eq!(default_window(&s.conn, 1).unwrap().unwrap().start, hm(12, 10));
    assert_eq!(default_window(&s.conn, 5).unwrap().unwrap().start, hm(11, 40));
}

#[test]
fn 없는_시정표를_기본값으로_지정할_수_없다() {
    let s = School::new();
    assert!(set_default_bell(&s.conn, Some(999)).is_err());
}

#[test]
fn 기본값을_없앨_수_있다() {
    let s = School::new();
    set_default_bell(&s.conn, Some(LOW_BELL)).unwrap();
    set_default_bell(&s.conn, None).unwrap();
    assert_eq!(default_bell_id(&s.conn).unwrap(), None);
    assert_eq!(default_window(&s.conn, 1).unwrap(), None);
}

#[test]
fn 시정표가_지워지면_기본값도_없는_것으로_본다() {
    let s = School::new();
    set_default_bell(&s.conn, Some(LOW_BELL)).unwrap();
    s.conn
        .execute("DELETE FROM bell_schedules WHERE id = ?1", params![LOW_BELL])
        .unwrap();
    assert_eq!(
        default_bell_id(&s.conn).unwrap(),
        None,
        "가리키던 것이 없으면 추측하지 않는다"
    );
}

// ============================================================
//  Case G — 수동 지정의 안전성
// ============================================================

#[test]
fn case_g_실제_수업과_겹치는_식사시간은_저장을_막는다() {
    let s = School::new();
    s.teach(1, 4, 5); // 월요일 4학년 5교시 12:20~13:00

    let e = set_override(
        &s.conn,
        &MealOverrideInput {
            teacher_id: SPECIAL,
            day_of_week: 1,
            start_min: Some(hm(12, 10)),
            end_min: Some(hm(13, 10)),
        },
    )
    .unwrap_err();

    assert!(
        e.user_message.contains("실제 수업") && e.user_message.contains("12:20~13:00"),
        "{}",
        e.user_message
    );

    // 저장되지 않았다
    assert!(overrides_for_day(&s.conn, 1).unwrap().is_empty());
}

#[test]
fn 겹치지_않는_식사시간은_저장된다() {
    let s = School::new();
    s.teach(1, 4, 5);

    set_override(
        &s.conn,
        &MealOverrideInput {
            teacher_id: SPECIAL,
            day_of_week: 1,
            start_min: Some(hm(13, 10)),
            end_min: Some(hm(14, 0)),
        },
    )
    .unwrap();

    assert_eq!(
        overrides_for_day(&s.conn, 1).unwrap().get(&SPECIAL),
        Some(&Interval::new(hm(13, 10), hm(14, 0)))
    );
    assert!(
        overrides_for_day(&s.conn, 2).unwrap().is_empty(),
        "요일마다 따로다"
    );
}

#[test]
fn 맞닿는_시간은_겹침이_아니라서_저장된다() {
    let s = School::new();
    s.teach(1, 4, 5); // 12:20~13:00
    set_override(
        &s.conn,
        &MealOverrideInput {
            teacher_id: SPECIAL,
            day_of_week: 1,
            start_min: Some(hm(13, 0)),
            end_min: Some(hm(14, 0)),
        },
    )
    .expect("13:00 시작은 13:00 종료 수업과 겹치지 않는다");
}

#[test]
fn 시각을_비우면_자동_판정으로_되돌린다() {
    let s = School::new();
    set_override(
        &s.conn,
        &MealOverrideInput {
            teacher_id: SPECIAL,
            day_of_week: 1,
            start_min: Some(hm(13, 10)),
            end_min: Some(hm(14, 0)),
        },
    )
    .unwrap();
    set_override(
        &s.conn,
        &MealOverrideInput {
            teacher_id: SPECIAL,
            day_of_week: 1,
            start_min: None,
            end_min: None,
        },
    )
    .unwrap();
    assert!(overrides_for_day(&s.conn, 1).unwrap().is_empty());
}

#[test]
fn 이상한_시각은_막는다() {
    let s = School::new();
    for (a, b) in [(hm(13, 0), hm(13, 0)), (hm(14, 0), hm(13, 0)), (-1, 100)] {
        assert!(
            set_override(
                &s.conn,
                &MealOverrideInput {
                    teacher_id: SPECIAL,
                    day_of_week: 1,
                    start_min: Some(a),
                    end_min: Some(b),
                },
            )
            .is_err(),
            "{a}~{b}"
        );
    }
    assert!(set_override(
        &s.conn,
        &MealOverrideInput {
            teacher_id: SPECIAL,
            day_of_week: 9,
            start_min: Some(hm(12, 0)),
            end_min: Some(hm(13, 0)),
        },
    )
    .is_err());
}

// ============================================================
//  화면에 넘길 값
// ============================================================

#[test]
fn 화면은_요일마다_어떻게_정해졌는지_알려_준다() {
    let s = School::new();
    s.teach(1, 4, 5); // 월: 고학년 5교시 → 늦은 점심 자동
    s.teach(2, 1, 5); // 화: 저학년 5교시 → 이른 점심 자동

    let v = view(&s.conn, Some(SPECIAL)).unwrap();
    assert_eq!(v.teacher_name.as_deref(), Some("김전담"));
    assert!(v.is_special);
    assert_eq!(v.days.len(), 5);

    let mon = s.day(&v, 1);
    assert_eq!(mon.source, "AUTO");
    assert_eq!(mon.source_label, "자동");
    assert_eq!(mon.time_label, "13:10~14:00");
    assert_eq!(mon.lessons.len(), 1, "그 날 수업도 함께 보여 준다");

    let tue = s.day(&v, 2);
    assert_eq!(tue.time_label, "12:10~13:10");

    // 수업이 없는 요일은 기본값이 없으면 확인 필요
    let wed = s.day(&v, 3);
    assert_eq!(wed.source, "UNKNOWN");
    assert_eq!(wed.source_label, "확인 필요");
    assert!(wed.note.is_some());
    assert!(v.needs_default, "기본값을 정해 달라고 알려 준다");
}

#[test]
fn 기본값을_정하면_확인_필요가_사라진다() {
    let s = School::new();
    set_default_bell(&s.conn, Some(HIGH_BELL)).unwrap();

    let v = view(&s.conn, Some(SPECIAL)).unwrap();
    assert!(!v.needs_default);
    let mon = s.day(&v, 1);
    assert_eq!(mon.source, "DEFAULT");
    assert_eq!(mon.source_label, "학교 기본");
    assert_eq!(mon.time_label, "13:10~14:00");
}

#[test]
fn 수동_지정은_직접_지정으로_보인다() {
    let s = School::new();
    set_default_bell(&s.conn, Some(LOW_BELL)).unwrap();
    set_override(
        &s.conn,
        &MealOverrideInput {
            teacher_id: SPECIAL,
            day_of_week: 3,
            start_min: Some(hm(13, 10)),
            end_min: Some(hm(14, 0)),
        },
    )
    .unwrap();

    let v = view(&s.conn, Some(SPECIAL)).unwrap();
    assert_eq!(s.day(&v, 3).source, "MANUAL");
    assert_eq!(s.day(&v, 3).source_label, "직접 지정");
    assert_eq!(s.day(&v, 3).time_label, "13:10~14:00");
    assert_eq!(s.day(&v, 1).source, "DEFAULT", "다른 요일은 그대로 기본값");
}

#[test]
fn 교사를_고르지_않으면_패턴만_돌려준다() {
    let s = School::new();
    let v = view(&s.conn, None).unwrap();
    assert_eq!(v.patterns.len(), 2);
    assert!(v.days.is_empty());
    assert!(v.teacher_id.is_none());
}

#[test]
fn 점심_패턴이_하나뿐이면_기본값을_묻지_않는다() {
    let s = School::new();
    s.conn
        .execute("UPDATE grade_bell_map SET bell_schedule_id = ?1", params![LOW_BELL])
        .unwrap();
    s.conn
        .execute("DELETE FROM bell_schedules WHERE id = ?1", params![HIGH_BELL])
        .unwrap();

    let v = view(&s.conn, Some(SPECIAL)).unwrap();
    assert!(!v.needs_default, "고를 것이 하나뿐이면 물을 이유가 없다");
    assert_eq!(s.day(&v, 1).source, "AUTO");
    assert_eq!(s.day(&v, 1).time_label, "12:10~13:10");
}

// ============================================================
//  기존 자료에 영향이 없다
// ============================================================

#[test]
fn 처음_업데이트한_상태에서는_아무_것도_막지_않는다() {
    // v0.1.7 에서 올라온 직후 — 기본값도 수동 지정도 없다
    let s = School::new();
    assert_eq!(default_bell_id(&s.conn).unwrap(), None);
    assert!(overrides_for_day(&s.conn, 1).unwrap().is_empty());

    // 패턴은 시정표에서 바로 뽑힌다
    assert_eq!(patterns(&s.conn).unwrap().len(), 2);
}
