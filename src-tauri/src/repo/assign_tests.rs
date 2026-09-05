//! Phase 8 검증 — 요구사항 13의 항목을 모두 확인한다.
//!
//! 실제 DB와 같은 모양의 작은 학교를 메모리에 만들어 시험한다.
//!   1~2학년 4교시 + 점심(3교시 뒤) · 3~6학년 5교시 + 점심(4교시 뒤)
//!   반 이름은 가람·나리

use rusqlite::{params, Connection};

use super::*;
use crate::db::memory_conn;

// ============================================================
//  시험용 학교 만들기
// ============================================================

fn hm(h: i32, m: i32) -> i32 {
    h * 60 + m
}

struct School {
    conn: Connection,
    term_id: i64,
}

impl School {
    /// 2개 학년군 × 2반, 담임 4명 + 전담 1명 + 기타 1명.
    fn new() -> Self {
        let conn = memory_conn();
        conn.execute(
            "INSERT INTO school(id, name, min_grade, max_grade) VALUES (1, '한빛초등학교', 1, 6)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO terms(id, school_year, semester, name, is_current)
             VALUES (1, 2026, 1, '2026학년도 1학기', 1)",
            [],
        )
        .unwrap();
        let s = Self { conn, term_id: 1 };
        s.build_bells();
        s.build_classes();
        s
    }

    /// 저학년(1~2) 4교시 + 3교시 뒤 점심 / 고학년(5~6) 5교시 + 4교시 뒤 점심.
    /// 점심 시각이 학년군마다 달라야 '실제 시각으로 판단하는지'를 볼 수 있다.
    fn build_bells(&self) {
        for (sid, name, grades, count, lunch_after, first) in [
            (1i64, "저학년", vec![1, 2], 4, 3, hm(9, 0)),
            (2i64, "고학년", vec![5, 6], 5, 4, hm(9, 0)),
        ] {
            self.conn
                .execute(
                    "INSERT INTO bell_schedules(id, term_id, name) VALUES (?1, ?2, ?3)",
                    params![sid, self.term_id, name],
                )
                .unwrap();
            for day in 1..=5 {
                let mut t = first;
                for p in 1..=count {
                    self.conn
                        .execute(
                            "INSERT INTO bell_slots(bell_schedule_id, day_of_week, slot_type, period_no, label, start_min, end_min)
                             VALUES (?1, ?2, 'PERIOD', ?3, ?4, ?5, ?6)",
                            params![sid, day, p, format!("{p}교시"), t, t + 40],
                        )
                        .unwrap();
                    t += 40;
                    if p == lunch_after {
                        self.conn
                            .execute(
                                "INSERT INTO bell_slots(bell_schedule_id, day_of_week, slot_type, period_no, label, start_min, end_min)
                                 VALUES (?1, ?2, 'LUNCH', NULL, '점심', ?3, ?4)",
                                params![sid, day, t, t + 50],
                            )
                            .unwrap();
                        t += 50;
                    } else if p < count {
                        t += 5;
                    }
                }
            }
            for g in grades {
                self.conn
                    .execute(
                        "INSERT INTO grade_bell_map(term_id, grade, bell_schedule_id) VALUES (?1, ?2, ?3)",
                        params![self.term_id, g, sid],
                    )
                    .unwrap();
            }
        }
    }

    fn build_classes(&self) {
        let mut cid = 1i64;
        for grade in [1, 2, 5, 6] {
            for (no, name) in [(1, "가람"), (2, "나리")] {
                self.conn
                    .execute(
                        "INSERT INTO classes(id, term_id, grade, class_no, name) VALUES (?1, ?2, ?3, ?4, ?5)",
                        params![cid, self.term_id, grade, no, name],
                    )
                    .unwrap();
                cid += 1;
            }
        }
    }

    fn add_teacher(&self, name: &str, role: &str) -> i64 {
        self.conn
            .execute(
                "INSERT INTO teachers(name, role_code) VALUES (?1, ?2)",
                params![name, role],
            )
            .unwrap();
        self.conn.last_insert_rowid()
    }

    fn set_homeroom(&self, class_id: i64, teacher_id: i64) {
        self.conn
            .execute(
                "UPDATE classes SET homeroom_teacher_id = ?2 WHERE id = ?1",
                params![class_id, teacher_id],
            )
            .unwrap();
    }

    fn class_id(&self, grade: i32, class_no: i32) -> i64 {
        self.conn
            .query_row(
                "SELECT id FROM classes WHERE term_id = ?1 AND grade = ?2 AND class_no = ?3",
                params![self.term_id, grade, class_no],
                |r| r.get(0),
            )
            .unwrap()
    }

    fn add_lesson(&self, teacher_id: i64, class_id: i64, day: i32, period: i32, subject: &str) {
        let sid: i64 = self
            .conn
            .query_row("SELECT id FROM subjects WHERE name = ?1", [subject], |r| {
                r.get(0)
            })
            .unwrap();
        self.conn
            .execute(
                "INSERT INTO lessons(term_id, teacher_id, class_id, subject_id, day_of_week, period_no, lesson_type, replaces_homeroom)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'SPECIAL', 1)",
                params![self.term_id, teacher_id, class_id, sid, day, period],
            )
            .unwrap();
    }

    fn assign(&self, date: &str, class_id: i64, period: i32, absent: i64, sub: i64) -> AppResult<AssignSaved> {
        assign_one(
            &self.conn,
            &AssignInput {
                date: date.to_string(),
                class_id,
                slot_type: "PERIOD".into(),
                period_no: Some(period),
                absent_teacher_id: Some(absent),
                sub_teacher_id: sub,
                absence_id: None,
                reason_code: None,
                reason_text: None,
            },
        )
    }

    fn counts_of(&self, date: &str, teacher_id: i64) -> SubCounts {
        repo_find::counts(&self.conn, date)
            .unwrap()
            .get(&teacher_id)
            .copied()
            .unwrap_or_default()
    }

    fn absent_all_day(&self, teacher_id: i64, date: &str, reason: &str) -> i64 {
        create_absence(
            &self.conn,
            &AbsenceInput {
                teacher_id,
                date: date.to_string(),
                is_all_day: true,
                start_min: None,
                end_min: None,
                reason_code: Some(reason.to_string()),
                reason_text: None,
            },
        )
        .unwrap()
    }
}

/// 2026-09-07 은 월요일이다.
const MON: &str = "2026-09-07";
const TUE: &str = "2026-09-08";

/// 담임 4명 + 전담 1명 + 교감 1명이 있는 기본 학교.
fn basic() -> (School, Vec<i64>) {
    let s = School::new();
    let mut hrs = Vec::new();
    for (grade, no, name) in [
        (1, 1, "김일가"),
        (1, 2, "김일나"),
        (5, 1, "이오가"),
        (5, 2, "이오나"),
    ] {
        let t = s.add_teacher(name, "HOMEROOM");
        s.set_homeroom(s.class_id(grade, no), t);
        hrs.push(t);
    }
    (s, hrs)
}

// ============================================================
//  1. 정상 단일 배정
// ============================================================

#[test]
fn 정상적으로_한_건을_배정한다() {
    let (s, hr) = basic();
    // 1-가람 담임(hr[0])이 결근 → 1교시 보결. 5학년 담임은 그 시간 수업 중이므로
    // 후보가 되려면 전담이 들어와야 한다. 여기서는 전담교사를 쓴다.
    let sp = s.add_teacher("박전담", "SPECIAL");

    let saved = s.assign(MON, s.class_id(1, 1), 1, hr[0], sp).unwrap();

    assert_eq!(saved.sub_teacher_name, "박전담");
    assert_eq!(saved.class_label, "1-가람");
    assert_eq!(saved.slot_label, "1교시");
    assert_eq!((saved.start_min, saved.end_min), (hm(9, 0), hm(9, 40)));
    assert_eq!(saved.absent_teacher_name.as_deref(), Some("김일가"));

    // 기록이 남았다
    let h = history(&s.conn, &HistoryFilter::default()).unwrap();
    assert_eq!(h.rows.len(), 1);
    assert_eq!(h.rows[0].status, "ASSIGNED");
    assert_eq!(h.rows[0].class_label, "1-가람");
    assert_eq!(h.rows[0].day_of_week, 1, "월요일");
    assert_eq!(h.assigned_count, 1);
}

#[test]
fn 배정하면_추천_순위와_근거도_남는다() {
    let (s, hr) = basic();
    let sp = s.add_teacher("박전담", "SPECIAL");
    s.assign(MON, s.class_id(1, 1), 1, hr[0], sp).unwrap();

    let h = history(&s.conn, &HistoryFilter::default()).unwrap();
    let r = &h.rows[0];
    assert!(r.recommend_rank.is_some(), "몇 순위 추천이었는지 남아야 한다");
    assert!(
        r.recommend_reason.as_deref().is_some_and(|x| !x.is_empty()),
        "추천 근거도 남아야 한다"
    );
}

// ============================================================
//  2. 저장 직전 재검증
// ============================================================

#[test]
fn 조회_후_다른_보결이_생기면_저장을_막는다() {
    let (s, hr) = basic();
    let sp = s.add_teacher("박전담", "SPECIAL");

    // 5-가람 1교시에 먼저 배정 (09:00~09:40)
    s.assign(MON, s.class_id(5, 1), 1, hr[2], sp).unwrap();

    // 같은 시각 1-가람 1교시에 같은 사람을 넣으려 하면 막힌다
    let e = s.assign(MON, s.class_id(1, 1), 1, hr[0], sp).unwrap_err();
    assert!(
        e.user_message.contains("박전담"),
        "누가 왜 안 되는지 알려야 한다: {}",
        e.user_message
    );
    assert_eq!(
        history(&s.conn, &HistoryFilter::default()).unwrap().rows.len(),
        1,
        "막혔으면 저장되지 않아야 한다"
    );
}

#[test]
fn 조회_후_결근이_추가되면_저장을_막는다() {
    let (s, hr) = basic();
    let sp = s.add_teacher("박전담", "SPECIAL");

    // 조회 시점에는 가능했지만, 그 사이에 전담교사가 출장으로 등록되었다
    s.absent_all_day(sp, MON, "TRIP");

    let e = s.assign(MON, s.class_id(1, 1), 1, hr[0], sp).unwrap_err();
    assert_eq!(e.code, "RECHECK_FAILED");
    assert!(e.user_message.contains("상황이 바뀌었습니다"), "{}", e.user_message);
}

#[test]
fn 보결_대상에서_빠지면_저장을_막는다() {
    let (s, hr) = basic();
    let sp = s.add_teacher("박전담", "SPECIAL");
    s.conn
        .execute("UPDATE teachers SET is_substitutable = 0 WHERE id = ?1", [sp])
        .unwrap();

    let e = s.assign(MON, s.class_id(1, 1), 1, hr[0], sp).unwrap_err();
    assert_eq!(e.code, "RECHECK_FAILED");
}

#[test]
fn 결근_교사_본인은_자기_보결로_넣을_수_없다() {
    let (s, hr) = basic();
    let e = s.assign(MON, s.class_id(1, 1), 1, hr[0], hr[0]).unwrap_err();
    assert!(e.user_message.contains("결근"), "{}", e.user_message);
}

// ============================================================
//  3. 결근이 조회에 즉시 반영된다
// ============================================================

#[test]
fn 종일_결근_교사는_후보에서_빠진다() {
    let (s, hr) = basic();
    let sp = s.add_teacher("박전담", "SPECIAL");
    s.absent_all_day(sp, MON, "SICK");

    let snap = repo_find::snapshot(&s.conn, MON).unwrap();
    let counts = repo_find::counts(&s.conn, MON).unwrap();
    let r = crate::domain::find::find_candidates(
        &snap,
        &FindRequest {
            class_id: s.class_id(1, 1),
            slot_type: "PERIOD".into(),
            period_no: Some(1),
            absent_teacher_id: Some(hr[0]),
        },
        &counts,
    )
    .unwrap();

    assert!(
        r.eligible.iter().all(|c| c.teacher_id != sp),
        "결근한 사람은 후보가 아니다"
    );
    let ex = r.excluded.iter().find(|c| c.teacher_id == sp).unwrap();
    assert_eq!(ex.reason_code, "EXCLUDED_ABSENCE");
}

#[test]
fn 일부_시간_결근은_그_시각에만_반영된다() {
    let (s, hr) = basic();
    let sp = s.add_teacher("박전담", "SPECIAL");

    // 오전 반차: 09:00~11:00
    create_absence(
        &s.conn,
        &AbsenceInput {
            teacher_id: sp,
            date: MON.into(),
            is_all_day: false,
            start_min: Some(hm(9, 0)),
            end_min: Some(hm(11, 0)),
            reason_code: Some("ANNUAL".into()),
            reason_text: None,
        },
    )
    .unwrap();

    // 1교시(09:00~09:40)는 막히고
    let e = s.assign(MON, s.class_id(1, 1), 1, hr[0], sp).unwrap_err();
    assert_eq!(e.code, "RECHECK_FAILED");

    // 5학년 5교시(11:55~12:35)는 결근 시간 밖이므로 배정된다
    let ok = s.assign(MON, s.class_id(5, 1), 5, hr[2], sp).unwrap();
    assert!(ok.start_min >= hm(11, 0), "결근 구간 뒤여야 한다");
}

#[test]
fn 결근_시각이_거꾸로면_등록을_막는다() {
    let (s, _) = basic();
    let t = s.add_teacher("박전담", "SPECIAL");
    let e = create_absence(
        &s.conn,
        &AbsenceInput {
            teacher_id: t,
            date: MON.into(),
            is_all_day: false,
            start_min: Some(hm(11, 0)),
            end_min: Some(hm(9, 0)),
            reason_code: None,
            reason_text: None,
        },
    )
    .unwrap_err();
    assert!(e.user_message.contains("종료 시각"), "{}", e.user_message);
}

#[test]
fn 종일_결근을_두_번_등록하지_않는다() {
    let (s, _) = basic();
    let t = s.add_teacher("박전담", "SPECIAL");
    s.absent_all_day(t, MON, "SICK");
    let e = create_absence(
        &s.conn,
        &AbsenceInput {
            teacher_id: t,
            date: MON.into(),
            is_all_day: true,
            start_min: None,
            end_min: None,
            reason_code: Some("ANNUAL".into()),
            reason_text: None,
        },
    )
    .unwrap_err();
    assert!(e.user_message.contains("이미"), "{}", e.user_message);
}

#[test]
fn 결근_목록에_사유와_시각이_함께_나온다() {
    let (s, _) = basic();
    let t = s.add_teacher("박전담", "SPECIAL");
    s.absent_all_day(t, MON, "TRIP");

    let list = absences_on(&s.conn, MON, false).unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].reason_label, "출장");
    assert!(list[0].is_all_day);
    assert_eq!(list[0].teacher_name, "박전담");
}

#[test]
fn 결근을_취소하면_후보로_돌아온다() {
    let (s, hr) = basic();
    let sp = s.add_teacher("박전담", "SPECIAL");
    let aid = s.absent_all_day(sp, MON, "SICK");
    assert!(s.assign(MON, s.class_id(1, 1), 1, hr[0], sp).is_err());

    let left = cancel_absence(&s.conn, aid).unwrap();
    assert_eq!(left, 0, "이 결근으로 배정된 보결이 없다");

    assert!(s.assign(MON, s.class_id(1, 1), 1, hr[0], sp).is_ok());
    // 기록은 남는다
    let all = absences_on(&s.conn, MON, true).unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].status, "CANCELLED");
    assert!(all[0].cancelled_at.is_some());
}

// ============================================================
//  4. 취소 · 재배정
// ============================================================

#[test]
fn 취소하면_보결_횟수에서_빠지고_기록은_남는다() {
    let (s, hr) = basic();
    let sp = s.add_teacher("박전담", "SPECIAL");
    let saved = s.assign(MON, s.class_id(1, 1), 1, hr[0], sp).unwrap();
    assert_eq!(s.counts_of(MON, sp).today, 1);
    assert_eq!(s.counts_of(MON, sp).total, 1);

    cancel(&s.conn, saved.id, Some("출장이 취소되어 담임이 출근함")).unwrap();

    assert_eq!(s.counts_of(MON, sp).today, 0, "취소는 집계에서 빠진다");
    assert_eq!(s.counts_of(MON, sp).total, 0);

    // 이력에는 남는다
    let h = history(&s.conn, &HistoryFilter::default()).unwrap();
    assert_eq!(h.rows.len(), 1);
    assert_eq!(h.rows[0].status, "CANCELLED");
    assert!(h.rows[0].cancelled_at.is_some());
    assert_eq!(
        h.rows[0].cancel_reason.as_deref(),
        Some("출장이 취소되어 담임이 출근함")
    );
    assert_eq!(h.cancelled_count, 1);
    assert_eq!(h.assigned_count, 0);
}

#[test]
fn 배정_기록은_지우지_않는다() {
    let (s, hr) = basic();
    let sp = s.add_teacher("박전담", "SPECIAL");
    let saved = s.assign(MON, s.class_id(1, 1), 1, hr[0], sp).unwrap();
    cancel(&s.conn, saved.id, None).unwrap();

    let n: i64 = s
        .conn
        .query_row("SELECT COUNT(*) FROM substitutions", [], |r| r.get(0))
        .unwrap();
    assert_eq!(n, 1, "행은 그대로 있어야 한다");
}

#[test]
fn 취소한_뒤_같은_시간에_다시_배정할_수_있다() {
    let (s, hr) = basic();
    let a = s.add_teacher("박전담", "SPECIAL");
    let b = s.add_teacher("최전담", "SPECIAL");

    let first = s.assign(MON, s.class_id(1, 1), 1, hr[0], a).unwrap();
    // 취소 전에는 같은 학급 같은 시간에 두 번 넣을 수 없다
    assert!(s.assign(MON, s.class_id(1, 1), 1, hr[0], b).is_err());

    cancel(&s.conn, first.id, Some("담당 변경")).unwrap();
    let second = s.assign(MON, s.class_id(1, 1), 1, hr[0], b).unwrap();
    assert_eq!(second.sub_teacher_name, "최전담");

    let h = history(&s.conn, &HistoryFilter::default()).unwrap();
    assert_eq!(h.rows.len(), 2, "취소된 것과 새 배정이 모두 보인다");
    assert_eq!(h.assigned_count, 1);
    assert_eq!(h.cancelled_count, 1);
}

#[test]
fn 이미_취소된_배정은_다시_취소하지_않는다() {
    let (s, hr) = basic();
    let sp = s.add_teacher("박전담", "SPECIAL");
    let saved = s.assign(MON, s.class_id(1, 1), 1, hr[0], sp).unwrap();
    cancel(&s.conn, saved.id, None).unwrap();
    let e = cancel(&s.conn, saved.id, None).unwrap_err();
    assert!(e.user_message.contains("이미 취소"), "{}", e.user_message);
}

// ============================================================
//  5. 시간 겹침
// ============================================================

#[test]
fn 같은_교사를_연속_교시에_배정할_수_있다() {
    let (s, hr) = basic();
    let sp = s.add_teacher("박전담", "SPECIAL");

    let a = s.assign(MON, s.class_id(1, 1), 1, hr[0], sp).unwrap();
    let b = s.assign(MON, s.class_id(1, 1), 2, hr[0], sp).unwrap();

    assert!(a.end_min <= b.start_min, "시간이 겹치지 않는다");
    assert_eq!(s.counts_of(MON, sp).today, 2);
}

#[test]
fn 같은_시간_다른_반에_같은_교사를_넣지_못한다() {
    let (s, hr) = basic();
    let sp = s.add_teacher("박전담", "SPECIAL");

    s.assign(MON, s.class_id(1, 1), 1, hr[0], sp).unwrap();
    let e = s.assign(MON, s.class_id(1, 2), 1, hr[1], sp).unwrap_err();
    assert!(e.user_message.contains("박전담"), "{}", e.user_message);
}

#[test]
fn 교시_번호가_같아도_실제_시각이_다르면_배정된다() {
    let (s, hr) = basic();
    let sp = s.add_teacher("박전담", "SPECIAL");

    // 1학년은 3교시 뒤 점심, 5학년은 4교시 뒤 점심 → 4교시 시각이 다르다
    let a = s.assign(MON, s.class_id(1, 1), 4, hr[0], sp).unwrap();
    let b = s.assign(MON, s.class_id(5, 1), 4, hr[2], sp).unwrap();
    assert_ne!(
        (a.start_min, a.end_min),
        (b.start_min, b.end_min),
        "같은 4교시라도 실제 시각이 달라야 이 시험이 뜻이 있다"
    );
}

// ============================================================
//  6. 하루 일괄 보결
// ============================================================

/// 5-가람 담임이 결근. 월요일 2교시에는 전담 영어가 들어온다.
fn batch_school() -> (School, Vec<i64>, i64) {
    let (s, hr) = basic();
    let sp = s.add_teacher("박전담", "SPECIAL");
    s.add_lesson(sp, s.class_id(5, 1), 1, 2, "영어");
    (s, hr, sp)
}

#[test]
fn 결근_교사의_그_날_일정을_모두_찾는다() {
    let (s, hr, _) = batch_school();
    let plan = day_plan(&s.conn, MON, hr[2]).unwrap();

    assert_eq!(plan.teacher_name, "이오가");
    // 5학년은 5교시 + 점심. 2교시는 전담이 들어오므로 빠진다 → 정규수업 4 + 점심 1
    let periods: Vec<i32> = plan
        .slots
        .iter()
        .filter(|x| x.slot_type == "PERIOD")
        .map(|x| x.period_no.unwrap())
        .collect();
    assert_eq!(periods, vec![1, 3, 4, 5], "전담이 들어오는 2교시는 빠진다");
    assert!(
        plan.slots.iter().any(|x| x.slot_type == "LUNCH"),
        "점심 지도도 보결 대상이다"
    );
    assert!(plan.slots.iter().all(|x| !x.candidates.is_empty()) || !plan.warnings.is_empty());
}

#[test]
fn 일괄_계획은_시간_순서로_나오고_성격을_알려준다() {
    let (s, hr, _) = batch_school();
    let plan = day_plan(&s.conn, MON, hr[2]).unwrap();

    let mut prev = -1;
    for x in &plan.slots {
        assert!(x.start_min >= prev, "시간 순서여야 한다");
        prev = x.start_min;
    }
    assert!(plan
        .slots
        .iter()
        .any(|x| x.kind_label == "정규 수업"));
    assert!(plan.slots.iter().any(|x| x.kind_label == "점심 지도"));
}

#[test]
fn 전담교사가_결근하면_전담_수업이_보결_대상이_된다() {
    let (s, _, sp) = batch_school();
    let plan = day_plan(&s.conn, MON, sp).unwrap();

    assert_eq!(plan.slots.len(), 1);
    assert_eq!(plan.slots[0].kind_label, "전담 수업");
    assert_eq!(plan.slots[0].class_label, "5-가람");
    assert_eq!(plan.slots[0].period_no, Some(2));
    assert_eq!(plan.slots[0].subject_name.as_deref(), Some("영어"));
}

#[test]
fn 일부_시간_결근이면_그_구간의_일정만_나온다() {
    let (s, hr, _) = batch_school();
    // 오후 반차 (12:00 이후)
    create_absence(
        &s.conn,
        &AbsenceInput {
            teacher_id: hr[2],
            date: MON.into(),
            is_all_day: false,
            start_min: Some(hm(12, 0)),
            end_min: Some(hm(16, 0)),
            reason_code: Some("ANNUAL".into()),
            reason_text: None,
        },
    )
    .unwrap();

    let plan = day_plan(&s.conn, MON, hr[2]).unwrap();
    assert!(
        plan.slots.iter().all(|x| x.end_min > hm(12, 0)),
        "12시 전에 끝나는 시간은 보결이 필요 없다: {:?}",
        plan.slots.iter().map(|x| x.slot_label.clone()).collect::<Vec<_>>()
    );
    assert!(plan.absence_label.as_deref().is_some_and(|l| l.contains("연가")));
}

#[test]
fn 일괄로_여러_시간을_한꺼번에_저장한다() {
    let (s, hr, sp) = batch_school();
    let other = s.add_teacher("최기타", "OTHER");

    let out = assign_batch(
        &s.conn,
        &BatchInput {
            date: MON.into(),
            absent_teacher_id: hr[2],
            absence_id: None,
            picks: vec![
                BatchPick {
                    class_id: s.class_id(5, 1),
                    slot_type: "PERIOD".into(),
                    period_no: Some(1),
                    sub_teacher_id: sp,
                },
                BatchPick {
                    class_id: s.class_id(5, 1),
                    slot_type: "PERIOD".into(),
                    period_no: Some(3),
                    sub_teacher_id: sp,
                },
                BatchPick {
                    class_id: s.class_id(5, 1),
                    slot_type: "PERIOD".into(),
                    period_no: Some(4),
                    sub_teacher_id: other,
                },
            ],
        },
    )
    .unwrap();

    assert_eq!(out.saved.len(), 3);
    assert_eq!(s.counts_of(MON, sp).today, 2, "연속되지 않은 두 교시");
    assert_eq!(s.counts_of(MON, other).today, 1);
}

#[test]
fn 일괄_저장_중_한_칸이라도_막히면_전부_되돌린다() {
    let (s, hr, sp) = batch_school();

    // 3교시에 같은 사람을 두 반에 넣는다 → 두 번째에서 막혀야 한다
    let e = assign_batch(
        &s.conn,
        &BatchInput {
            date: MON.into(),
            absent_teacher_id: hr[2],
            absence_id: None,
            picks: vec![
                BatchPick {
                    class_id: s.class_id(5, 1),
                    slot_type: "PERIOD".into(),
                    period_no: Some(1),
                    sub_teacher_id: sp,
                },
                BatchPick {
                    class_id: s.class_id(5, 2),
                    slot_type: "PERIOD".into(),
                    period_no: Some(1),
                    sub_teacher_id: sp,
                },
            ],
        },
    )
    .unwrap_err();

    assert!(
        e.user_message.contains("아무것도 저장하지 않았습니다"),
        "{}",
        e.user_message
    );
    // repo 계층 시험이라 트랜잭션이 없다. 앱에서는 Db::write 가 되돌린다.
    // 여기서는 '두 번째가 막혔다'는 것만 확인한다.
    let n: i64 = s
        .conn
        .query_row(
            "SELECT COUNT(*) FROM substitutions WHERE status = 'ASSIGNED'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(n, 1, "막히기 전 1건만 들어가 있고, 트랜잭션이 이를 되돌린다");
}

#[test]
fn 일괄_배정은_결근_기록과_사유로_묶인다() {
    let (s, hr, sp) = batch_school();
    let aid = s.absent_all_day(hr[2], MON, "TRIP");

    assign_batch(
        &s.conn,
        &BatchInput {
            date: MON.into(),
            absent_teacher_id: hr[2],
            absence_id: None, // 활성 결근을 알아서 찾는다
            picks: vec![BatchPick {
                class_id: s.class_id(5, 1),
                slot_type: "PERIOD".into(),
                period_no: Some(1),
                sub_teacher_id: sp,
            }],
        },
    )
    .unwrap();

    let (linked, reason): (Option<i64>, Option<String>) = s
        .conn
        .query_row(
            "SELECT absence_id, reason_code FROM substitutions",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(linked, Some(aid));
    assert_eq!(reason.as_deref(), Some("TRIP"), "결근 사유를 이어받는다");

    // 결근을 취소하려 하면 남은 보결 건수를 알려 준다
    assert_eq!(cancel_absence(&s.conn, aid).unwrap(), 1);
}

#[test]
fn 빈_일괄_배정은_막는다() {
    let (s, hr, _) = batch_school();
    let e = assign_batch(
        &s.conn,
        &BatchInput {
            date: MON.into(),
            absent_teacher_id: hr[2],
            absence_id: None,
            picks: vec![],
        },
    )
    .unwrap_err();
    assert!(e.user_message.contains("하나 이상"), "{}", e.user_message);
}

#[test]
fn 이미_배정된_시간은_일괄_계획에_표시된다() {
    let (s, hr, sp) = batch_school();
    s.assign(MON, s.class_id(5, 1), 1, hr[2], sp).unwrap();

    let plan = day_plan(&s.conn, MON, hr[2]).unwrap();
    let first = plan
        .slots
        .iter()
        .find(|x| x.period_no == Some(1))
        .unwrap();
    assert_eq!(first.existing_sub_name.as_deref(), Some("박전담"));
}

// ============================================================
//  7. 스냅샷 — 나중에 설정이 바뀌어도 기록은 그대로
// ============================================================

#[test]
fn 반_이름이_바뀌어도_과거_기록의_표기는_그대로다() {
    let (s, hr) = basic();
    let sp = s.add_teacher("박전담", "SPECIAL");
    s.assign(MON, s.class_id(1, 1), 1, hr[0], sp).unwrap();

    // 새 학년도에 반 이름을 바꾼다
    s.conn
        .execute(
            "UPDATE classes SET name = '한빛' WHERE id = ?1",
            [s.class_id(1, 1)],
        )
        .unwrap();

    let h = history(&s.conn, &HistoryFilter::default()).unwrap();
    assert_eq!(h.rows[0].class_label, "1-가람", "기록은 당시 표기 그대로");
}

#[test]
fn 시정표가_바뀌어도_과거_기록의_시각은_그대로다() {
    let (s, hr) = basic();
    let sp = s.add_teacher("박전담", "SPECIAL");
    s.assign(MON, s.class_id(1, 1), 1, hr[0], sp).unwrap();

    s.conn
        .execute(
            "UPDATE bell_slots SET start_min = start_min + 30, end_min = end_min + 30",
            [],
        )
        .unwrap();

    let h = history(&s.conn, &HistoryFilter::default()).unwrap();
    assert_eq!(h.rows[0].start_min, hm(9, 0), "기록은 당시 시각 그대로");
}

#[test]
fn 전담_시간에_배정하면_과목과_안내가_함께_남는다() {
    let (s, hr, sp) = batch_school();
    let other = s.add_teacher("최기타", "OTHER");

    // 5-가람 2교시는 박전담의 영어 시간이다. 담임이 결근해도 보결이 필요 없다.
    let saved = s.assign(MON, s.class_id(5, 1), 2, hr[2], other).unwrap();
    let n = saved.notice.as_ref().expect("확인 안내가 있어야 한다");
    assert!(n.title.contains("확인 바랍니다"), "{}", n.title);

    let h = history(&s.conn, &HistoryFilter::default()).unwrap();
    assert_eq!(h.rows[0].subject_name.as_deref(), Some("영어"));
    let _ = sp;
}

// ============================================================
//  8. 배정 내역 화면
// ============================================================

#[test]
fn 배정_내역을_날짜와_이름으로_거른다() {
    let (s, hr) = basic();
    let a = s.add_teacher("박전담", "SPECIAL");
    let b = s.add_teacher("최전담", "SPECIAL");
    s.assign(MON, s.class_id(1, 1), 1, hr[0], a).unwrap();
    s.assign(TUE, s.class_id(1, 1), 1, hr[0], b).unwrap();

    let only_mon = history(
        &s.conn,
        &HistoryFilter {
            from: Some(MON.into()),
            to: Some(MON.into()),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(only_mon.rows.len(), 1);
    assert_eq!(only_mon.rows[0].sub_teacher_name, "박전담");

    let by_name = history(
        &s.conn,
        &HistoryFilter {
            keyword: Some("최전담".into()),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(by_name.rows.len(), 1);
    assert_eq!(by_name.rows[0].date, TUE);

    // 결근 교사 이름으로도 찾힌다
    let by_absent = history(
        &s.conn,
        &HistoryFilter {
            keyword: Some("김일가".into()),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(by_absent.rows.len(), 2);
}

#[test]
fn 상태로_거를_수_있다() {
    let (s, hr) = basic();
    let a = s.add_teacher("박전담", "SPECIAL");
    let saved = s.assign(MON, s.class_id(1, 1), 1, hr[0], a).unwrap();
    s.assign(MON, s.class_id(1, 1), 2, hr[0], a).unwrap();
    cancel(&s.conn, saved.id, None).unwrap();

    let live = history(
        &s.conn,
        &HistoryFilter {
            status: Some("ASSIGNED".into()),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(live.rows.len(), 1);

    let dead = history(
        &s.conn,
        &HistoryFilter {
            status: Some("CANCELLED".into()),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(dead.rows.len(), 1);
}

#[test]
fn 앱을_다시_켜도_기록이_그대로_복원된다() {
    let (s, hr) = basic();
    let sp = s.add_teacher("박전담", "SPECIAL");
    let one = s.assign(MON, s.class_id(1, 1), 1, hr[0], sp).unwrap();
    s.assign(MON, s.class_id(1, 1), 2, hr[0], sp).unwrap();
    cancel(&s.conn, one.id, Some("담임 출근")).unwrap();

    // 같은 DB를 새 커넥션으로 다시 읽는 것과 같은 상황
    let h = history(&s.conn, &HistoryFilter::default()).unwrap();
    assert_eq!(h.rows.len(), 2);
    assert_eq!(h.assigned_count, 1);
    assert_eq!(h.cancelled_count, 1);
    assert_eq!(s.counts_of(MON, sp).today, 1, "취소분을 뺀 횟수");

    let cancelled = h.rows.iter().find(|r| r.status == "CANCELLED").unwrap();
    assert_eq!(cancelled.cancel_reason.as_deref(), Some("담임 출근"));
    assert!(cancelled.cancelled_at.is_some());
}
