//! Phase 9 검증 — 요구사항 11의 항목을 모두 확인한다.
//!
//! 통계는 원본을 그때그때 다시 세는 것이므로, **자료를 바꾼 뒤 다시 조회했을 때
//! 숫자가 따라오는지**가 핵심이다.

use rusqlite::params;

use super::*;
use crate::db::memory_conn;
use crate::domain::period;
use crate::repo::assign::{
    self as ra, AbsenceInput, AssignInput, BatchInput, BatchPick,
};

// ============================================================
//  시험용 학교
// ============================================================

fn hm(h: i32, m: i32) -> i32 {
    h * 60 + m
}

struct School {
    conn: Connection,
}

impl School {
    /// 1·5학년 × 2반. 저학년 4교시(3교시 뒤 점심) / 고학년 5교시(4교시 뒤 점심).
    fn new() -> Self {
        let conn = memory_conn();
        conn.execute(
            "INSERT INTO school(id, name, min_grade, max_grade) VALUES (1, '한빛초등학교', 1, 6)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO terms(id, school_year, semester, name, is_current)
             VALUES (1, 2026, 2, '2026학년도 2학기', 1)",
            [],
        )
        .unwrap();

        for (sid, name, grades, count, lunch_after) in [
            (1i64, "저학년", vec![1, 2], 4, 3),
            (2i64, "고학년", vec![5, 6], 5, 4),
        ] {
            conn.execute(
                "INSERT INTO bell_schedules(id, term_id, name) VALUES (?1, 1, ?2)",
                params![sid, name],
            )
            .unwrap();
            for day in 1..=5 {
                let mut t = hm(9, 0);
                for p in 1..=count {
                    conn.execute(
                        "INSERT INTO bell_slots(bell_schedule_id, day_of_week, slot_type, period_no, label, start_min, end_min)
                         VALUES (?1, ?2, 'PERIOD', ?3, ?4, ?5, ?6)",
                        params![sid, day, p, format!("{p}교시"), t, t + 40],
                    )
                    .unwrap();
                    t += 40;
                    if p == lunch_after {
                        conn.execute(
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
                conn.execute(
                    "INSERT INTO grade_bell_map(term_id, grade, bell_schedule_id) VALUES (1, ?1, ?2)",
                    params![g, sid],
                )
                .unwrap();
            }
        }

        let mut cid = 1i64;
        for grade in [1, 5] {
            for (no, name) in [(1, "가람"), (2, "나리")] {
                conn.execute(
                    "INSERT INTO classes(id, term_id, grade, class_no, name) VALUES (?1, 1, ?2, ?3, ?4)",
                    params![cid, grade, no, name],
                )
                .unwrap();
                cid += 1;
            }
        }
        Self { conn }
    }

    fn teacher(&self, name: &str, role: &str) -> i64 {
        self.conn
            .execute(
                "INSERT INTO teachers(name, role_code) VALUES (?1, ?2)",
                params![name, role],
            )
            .unwrap();
        self.conn.last_insert_rowid()
    }

    fn class_id(&self, grade: i32, no: i32) -> i64 {
        self.conn
            .query_row(
                "SELECT id FROM classes WHERE grade = ?1 AND class_no = ?2",
                params![grade, no],
                |r| r.get(0),
            )
            .unwrap()
    }

    fn set_homeroom(&self, class_id: i64, teacher_id: i64) {
        self.conn
            .execute(
                "UPDATE classes SET homeroom_teacher_id = ?2 WHERE id = ?1",
                params![class_id, teacher_id],
            )
            .unwrap();
    }

    fn assign(&self, date: &str, grade: i32, no: i32, period: i32, absent: i64, sub: i64) -> i64 {
        ra::assign_one(
            &self.conn,
            &AssignInput {
                date: date.into(),
                class_id: self.class_id(grade, no),
                slot_type: "PERIOD".into(),
                period_no: Some(period),
                absent_teacher_id: Some(absent),
                sub_teacher_id: sub,
                absence_id: None,
                reason_code: None,
                reason_text: None,
            },
        )
        .unwrap_or_else(|e| panic!("{date} {grade}-{no} {period}교시 배정 실패: {}", e.user_message))
        .id
    }

    fn absent(&self, teacher: i64, date: &str, reason: &str) -> i64 {
        ra::create_absence(
            &self.conn,
            &AbsenceInput {
                teacher_id: teacher,
                date: date.into(),
                is_all_day: true,
                start_min: None,
                end_min: None,
                reason_code: Some(reason.into()),
                reason_text: None,
            },
        )
        .unwrap()
    }

    fn absent_part(&self, teacher: i64, date: &str, from: i32, to: i32) -> i64 {
        ra::create_absence(
            &self.conn,
            &AbsenceInput {
                teacher_id: teacher,
                date: date.into(),
                is_all_day: false,
                start_min: Some(from),
                end_min: Some(to),
                reason_code: Some("ANNUAL".into()),
                reason_text: None,
            },
        )
        .unwrap()
    }

    fn on(&self, from: &str, to: &str) -> StatsView {
        view(
            &self.conn,
            &StatsQuery {
                preset: Some(period::CUSTOM.into()),
                from: Some(from.into()),
                to: Some(to.into()),
            },
        )
        .unwrap()
    }

    fn day(&self, date: &str) -> StatsView {
        self.on(date, date)
    }

    fn of<'a>(&self, v: &'a StatsView, name: &str) -> &'a TeacherStat {
        v.teachers.iter().find(|t| t.name == name).unwrap()
    }
}

/// 2026-09-07(월) ~ 09-11(금)
const MON: &str = "2026-09-07";
const TUE: &str = "2026-09-08";
const WED: &str = "2026-09-09";

/// 담임 4명 + 전담 2명 + 기타 1명
fn basic() -> (School, Vec<i64>, i64, i64) {
    let s = School::new();
    let mut hr = Vec::new();
    for (grade, no, name) in [
        (1, 1, "김일가"),
        (1, 2, "김일나"),
        (5, 1, "이오가"),
        (5, 2, "이오나"),
    ] {
        let t = s.teacher(name, "HOMEROOM");
        s.set_homeroom(s.class_id(grade, no), t);
        hr.push(t);
    }
    let sp1 = s.teacher("박전담", "SPECIAL");
    let sp2 = s.teacher("최전담", "SPECIAL");
    (s, hr, sp1, sp2)
}

// ============================================================
//  1. 정상 배정은 통계에 들어간다
// ============================================================

#[test]
fn 배정한_건수가_요약과_교사별에_들어간다() {
    let (s, hr, sp1, sp2) = basic();
    s.assign(MON, 1, 1, 1, hr[0], sp1);
    s.assign(MON, 1, 1, 2, hr[0], sp2);

    let v = s.day(MON);
    assert_eq!(v.summary.assigned, 2);
    assert_eq!(v.summary.cancelled, 0);
    assert_eq!(v.summary.sub_teachers, 2, "보결을 맡은 선생님 2명");
    assert_eq!(s.of(&v, "박전담").period, 1);
    assert_eq!(s.of(&v, "최전담").period, 1);
    assert_eq!(s.of(&v, "김일가").period, 0);
}

#[test]
fn 기간_밖의_배정은_세지_않는다() {
    let (s, hr, sp1, _) = basic();
    s.assign(MON, 1, 1, 1, hr[0], sp1);
    s.assign(WED, 1, 1, 1, hr[0], sp1);

    assert_eq!(s.day(MON).summary.assigned, 1);
    assert_eq!(s.day(TUE).summary.assigned, 0);
    assert_eq!(s.on(MON, WED).summary.assigned, 2);
}

// ============================================================
//  2. 취소는 빠지고 3. 재배정만 잡힌다
// ============================================================

#[test]
fn 취소한_배정은_통계에서_빠진다() {
    let (s, hr, sp1, _) = basic();
    let id = s.assign(MON, 1, 1, 1, hr[0], sp1);
    assert_eq!(s.day(MON).summary.assigned, 1);

    ra::cancel(&s.conn, id, Some("담임 출근")).unwrap();

    let v = s.day(MON);
    assert_eq!(v.summary.assigned, 0, "취소는 배정 수에서 빠진다");
    assert_eq!(v.summary.cancelled, 1, "취소 건수로는 남는다");
    assert_eq!(v.summary.sub_teachers, 0);
    assert_eq!(s.of(&v, "박전담").period, 0);
    assert_eq!(s.of(&v, "박전담").total, 0, "누적에서도 빠진다");
}

#[test]
fn 취소_후_재배정하면_새_배정만_센다() {
    let (s, hr, sp1, sp2) = basic();
    let first = s.assign(MON, 1, 1, 1, hr[0], sp1);
    ra::cancel(&s.conn, first, Some("담당 변경")).unwrap();
    s.assign(MON, 1, 1, 1, hr[0], sp2);

    let v = s.day(MON);
    assert_eq!(v.summary.assigned, 1);
    assert_eq!(v.summary.cancelled, 1);
    assert_eq!(s.of(&v, "박전담").period, 0, "취소된 사람은 0");
    assert_eq!(s.of(&v, "최전담").period, 1, "새로 맡은 사람만 1");
}

// ============================================================
//  4. 기간 계산
// ============================================================

#[test]
fn 오늘_이번주_이번달_학기_범위를_계산한다() {
    use chrono::NaiveDate;
    let wed = NaiveDate::from_ymd_opt(2026, 9, 9).unwrap(); // 수요일

    let t = period::resolve(period::TODAY, wed, None, (None, None));
    assert_eq!((t.from.as_str(), t.to.as_str()), ("2026-09-09", "2026-09-09"));

    let w = period::resolve(period::WEEK, wed, None, (None, None));
    assert_eq!(
        (w.from.as_str(), w.to.as_str()),
        ("2026-09-07", "2026-09-13"),
        "월요일부터 일요일까지"
    );

    let m = period::resolve(period::MONTH, wed, None, (None, None));
    assert_eq!((m.from.as_str(), m.to.as_str()), ("2026-09-01", "2026-09-30"));

    // 학기 날짜를 넣지 않은 학교는 학사 일정으로 계산한다
    let term = period::TermInfo {
        school_year: 2026,
        semester: 2,
        name: "2026학년도 2학기".into(),
        start: None,
        end: None,
    };
    let q = period::resolve(period::TERM, wed, Some(&term), (None, None));
    assert_eq!((q.from.as_str(), q.to.as_str()), ("2026-09-01", "2027-02-28"));

    let term1 = period::TermInfo { semester: 1, ..term.clone() };
    let q1 = period::resolve(period::TERM, wed, Some(&term1), (None, None));
    assert_eq!((q1.from.as_str(), q1.to.as_str()), ("2026-03-01", "2026-08-31"));
}

#[test]
fn 학교가_넣어_둔_학기_날짜를_먼저_쓴다() {
    use chrono::NaiveDate;
    let term = period::TermInfo {
        school_year: 2026,
        semester: 2,
        name: "2026학년도 2학기".into(),
        start: Some("2026-08-20".into()),
        end: Some("2027-01-10".into()),
    };
    let r = period::resolve(
        period::TERM,
        NaiveDate::from_ymd_opt(2026, 9, 9).unwrap(),
        Some(&term),
        (None, None),
    );
    assert_eq!((r.from.as_str(), r.to.as_str()), ("2026-08-20", "2027-01-10"));
}

#[test]
fn 사용자_지정_기간이_거꾸로면_바로잡는다() {
    use chrono::NaiveDate;
    let r = period::resolve(
        period::CUSTOM,
        NaiveDate::from_ymd_opt(2026, 9, 9).unwrap(),
        None,
        (Some("2026-09-20"), Some("2026-09-10")),
    );
    assert_eq!((r.from.as_str(), r.to.as_str()), ("2026-09-10", "2026-09-20"));
}

#[test]
fn 기간을_바꾸면_숫자도_바뀐다() {
    let (s, hr, sp1, _) = basic();
    s.assign(MON, 1, 1, 1, hr[0], sp1);
    s.assign(TUE, 1, 1, 1, hr[0], sp1);
    s.assign(WED, 1, 1, 1, hr[0], sp1);

    assert_eq!(s.day(MON).summary.assigned, 1);
    assert_eq!(s.on(MON, TUE).summary.assigned, 2);
    assert_eq!(s.on(MON, WED).summary.assigned, 3);
    assert_eq!(s.of(&s.on(MON, TUE), "박전담").period, 2);
    assert_eq!(s.of(&s.on(MON, WED), "박전담").period, 3);
    assert_eq!(
        s.of(&s.day(MON), "박전담").total,
        3,
        "누적은 기간과 상관없이 전체다"
    );
}

// ============================================================
//  5. 결근 집계 (종일 · 일부 시간)
// ============================================================

#[test]
fn 종일_결근과_일부_시간_결근을_나누어_센다() {
    let (s, hr, _, _) = basic();
    s.absent(hr[0], MON, "SICK");
    s.absent_part(hr[1], MON, hm(13, 0), hm(16, 0));

    let v = s.day(MON);
    assert_eq!(v.summary.absent_teachers, 2);
    assert_eq!(v.summary.absence_count, 2);

    let a = v.absences.iter().find(|a| a.name == "김일가").unwrap();
    assert_eq!((a.all_day, a.partial), (1, 0));
    assert_eq!(a.reasons, "병가");

    let b = v.absences.iter().find(|a| a.name == "김일나").unwrap();
    assert_eq!((b.all_day, b.partial), (0, 1));
    assert_eq!(b.reasons, "연가");
}

#[test]
fn 종일_결근은_그_날_모든_시간이_보결_대상이다() {
    let (s, hr, _, _) = basic();
    s.absent(hr[0], MON, "TRIP");

    let v = s.day(MON);
    // 1학년은 4교시 + 점심 = 5칸
    assert_eq!(v.summary.required, 5);
    assert_eq!(v.summary.unassigned, 5, "아직 아무도 배정하지 않았다");
    assert_eq!(v.open_slots.len(), 5);
    assert!(v.open_slots.iter().all(|o| o.absent_teacher_name == "김일가"));
}

#[test]
fn 일부_시간_결근은_그_구간만_보결_대상이다() {
    let (s, hr, _, _) = basic();
    // 1학년 3교시는 10:30~11:10, 점심 11:10~12:00, 4교시 12:00~12:40
    s.absent_part(hr[0], MON, hm(11, 30), hm(16, 0));

    let v = s.day(MON);
    assert!(v.summary.required < 5, "종일보다 적어야 한다");
    assert!(
        v.open_slots.iter().all(|o| o.end_after(hm(11, 30))),
        "11:30 전에 끝나는 시간은 빠진다: {:?}",
        v.open_slots.iter().map(|o| o.slot_label.clone()).collect::<Vec<_>>()
    );

    let a = &v.absences[0];
    assert_eq!(a.required, v.summary.required);
    assert_eq!(a.partial, 1);
}

#[test]
fn 취소한_결근은_보결_대상에서_빠진다() {
    let (s, hr, _, _) = basic();
    let id = s.absent(hr[0], MON, "SICK");
    assert_eq!(s.day(MON).summary.required, 5);

    ra::cancel_absence(&s.conn, id).unwrap();

    let v = s.day(MON);
    assert_eq!(v.summary.required, 0);
    assert_eq!(v.summary.absent_teachers, 0);
    assert!(v.absences.is_empty());
}

#[test]
fn 배정하면_미배정이_줄어든다() {
    let (s, hr, sp1, sp2) = basic();
    s.absent(hr[0], MON, "TRIP");
    assert_eq!(s.day(MON).summary.unassigned, 5);

    s.assign(MON, 1, 1, 1, hr[0], sp1);
    s.assign(MON, 1, 1, 2, hr[0], sp2);

    let v = s.day(MON);
    assert_eq!(v.summary.required, 5);
    assert_eq!(v.summary.assigned, 2);
    assert_eq!(v.summary.unassigned, 3);
    assert_eq!(v.open_slots.len(), 3);

    let a = &v.absences[0];
    assert_eq!((a.required, a.assigned, a.unassigned), (5, 2, 3));
}

// ============================================================
//  6. 다건 배정도 한 칸에 1건
// ============================================================

#[test]
fn 다건_배정은_칸마다_한_건으로_센다() {
    let (s, hr, sp1, sp2) = basic();
    s.absent(hr[2], MON, "TRIP"); // 5-가람 담임

    ra::assign_batch(
        &s.conn,
        &BatchInput {
            date: MON.into(),
            absent_teacher_id: hr[2],
            absence_id: None,
            picks: vec![
                BatchPick { class_id: s.class_id(5, 1), slot_type: "PERIOD".into(), period_no: Some(1), sub_teacher_id: sp1 },
                BatchPick { class_id: s.class_id(5, 1), slot_type: "PERIOD".into(), period_no: Some(2), sub_teacher_id: sp1 },
                BatchPick { class_id: s.class_id(5, 1), slot_type: "PERIOD".into(), period_no: Some(3), sub_teacher_id: sp2 },
            ],
        },
    )
    .unwrap();

    let v = s.day(MON);
    assert_eq!(v.summary.assigned, 3, "3칸이면 3건");
    assert_eq!(v.summary.sub_teachers, 2, "사람 수는 2명");
    assert_eq!(s.of(&v, "박전담").period, 2, "같은 사람이 두 칸이면 2회");
    assert_eq!(s.of(&v, "최전담").period, 1);
    // 5학년은 5교시 + 점심 = 6칸
    assert_eq!(v.summary.required, 6);
    assert_eq!(v.summary.unassigned, 3);
}

// ============================================================
//  7. 날짜별 현황
// ============================================================

#[test]
fn 날짜별로_결근과_배정을_보여준다() {
    let (s, hr, sp1, _) = basic();
    s.absent(hr[0], MON, "SICK");
    s.assign(MON, 1, 1, 1, hr[0], sp1);
    s.assign(TUE, 5, 1, 1, hr[2], sp1);

    let v = s.on(MON, WED);
    assert_eq!(v.days.len(), 3, "월·화·수");

    let mon = v.days.iter().find(|d| d.date == MON).unwrap();
    assert_eq!(mon.day_of_week, 1);
    assert_eq!(mon.absent_teachers, 1);
    assert_eq!(mon.absent_names, "김일가");
    assert_eq!((mon.required, mon.assigned, mon.unassigned), (5, 1, 4));

    let tue = v.days.iter().find(|d| d.date == TUE).unwrap();
    assert_eq!(tue.absent_teachers, 0);
    assert_eq!(tue.assigned, 1);
    assert_eq!(tue.required, 0, "결근 등록이 없으면 필요 건수는 0이다");

    let wed = v.days.iter().find(|d| d.date == WED).unwrap();
    assert_eq!((wed.required, wed.assigned, wed.unassigned), (0, 0, 0));
}

#[test]
fn 미배정이_남은_날을_찾을_수_있다() {
    let (s, hr, sp1, sp2) = basic();
    s.absent(hr[0], MON, "SICK");
    s.absent(hr[2], TUE, "TRIP");
    // 화요일만 모두 배정한다 (5학년 5교시 + 점심 = 6칸).
    // 같은 반 담임(이오나)은 그 시간 자기 반 수업 중이므로 전담 두 명이 나눠 맡는다.
    for p in 1..=5 {
        s.assign(TUE, 5, 1, p, hr[2], if p % 2 == 0 { sp2 } else { sp1 });
    }
    ra::assign_one(
        &s.conn,
        &AssignInput {
            date: TUE.into(),
            class_id: s.class_id(5, 1),
            slot_type: "LUNCH".into(),
            period_no: None,
            absent_teacher_id: Some(hr[2]),
            sub_teacher_id: sp1,
            absence_id: None,
            reason_code: None,
            reason_text: None,
        },
    )
    .unwrap();

    let v = s.on(MON, WED);
    let open: Vec<&DayStat> = v.days.iter().filter(|d| d.unassigned > 0).collect();
    assert_eq!(open.len(), 1);
    assert_eq!(open[0].date, MON);
    assert!(v.open_slots.iter().all(|o| o.date == MON));
}

// ============================================================
//  8. 과거 기록은 설정이 바뀌어도 그대로
// ============================================================

#[test]
fn 반_이름을_바꿔도_과거_통계는_그대로다() {
    let (s, hr, sp1, _) = basic();
    s.assign(MON, 1, 1, 1, hr[0], sp1);
    let before = s.day(MON);

    s.conn
        .execute(
            "UPDATE classes SET name = '한빛' WHERE id = ?1",
            [s.class_id(1, 1)],
        )
        .unwrap();

    let after = s.day(MON);
    assert_eq!(before.summary.assigned, after.summary.assigned);
    assert_eq!(s.of(&after, "박전담").period, 1);

    let h = ra::history(&s.conn, &ra::HistoryFilter::default()).unwrap();
    assert_eq!(h.rows[0].class_label, "1-가람", "배정 내역은 당시 표기 그대로");
}

#[test]
fn 교사를_비활성으로_바꿔도_과거_횟수는_남는다() {
    let (s, hr, sp1, _) = basic();
    s.assign(MON, 1, 1, 1, hr[0], sp1);
    s.conn
        .execute("UPDATE teachers SET active = 0 WHERE id = ?1", [sp1])
        .unwrap();

    let v = s.day(MON);
    let t = s.of(&v, "박전담");
    assert_eq!(t.period, 1, "지난 보결은 그대로 남는다");
    assert!(!t.active, "비활성으로 표시된다");
    assert_eq!(v.summary.assigned, 1);
}

// ============================================================
//  9. 다시 조회해도 같은 결과 (앱 재실행과 같은 상황)
// ============================================================

#[test]
fn 몇_번을_조회해도_같은_결과가_나온다() {
    let (s, hr, sp1, sp2) = basic();
    s.absent(hr[0], MON, "SICK");
    s.assign(MON, 1, 1, 1, hr[0], sp1);
    let id = s.assign(MON, 1, 1, 2, hr[0], sp2);
    ra::cancel(&s.conn, id, None).unwrap();

    let a = s.day(MON);
    let b = s.day(MON);
    assert_eq!(a.summary.assigned, b.summary.assigned);
    assert_eq!(a.summary.unassigned, b.summary.unassigned);
    assert_eq!(a.summary.cancelled, b.summary.cancelled);
    assert_eq!(a.teachers.len(), b.teachers.len());
    for (x, y) in a.teachers.iter().zip(b.teachers.iter()) {
        assert_eq!((x.teacher_id, x.period, x.total), (y.teacher_id, y.period, y.total));
    }

    // 통계를 따로 저장하지 않는지 확인한다
    let tables: Vec<String> = s
        .conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table'")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    assert!(
        !tables.iter().any(|t| t.contains("stat")),
        "집계표를 따로 두지 않는다: {tables:?}"
    );
}

// ============================================================
//  10. 참고 표시 (공정성)
// ============================================================

#[test]
fn 같은_구분끼리만_참고_표시를_한다() {
    let (s, hr, sp1, sp2) = basic();
    let sp3 = s.teacher("한전담", "SPECIAL");

    // 전담 3명: 6 / 1 / 1 회
    for p in 1..=4 {
        s.assign(MON, 1, 1, p, hr[0], sp1);
    }
    s.assign(TUE, 1, 1, 1, hr[0], sp1);
    s.assign(WED, 1, 1, 1, hr[0], sp1);
    s.assign(MON, 5, 1, 1, hr[2], sp2);
    s.assign(MON, 5, 2, 1, hr[3], sp3);

    let v = s.on(MON, WED);
    assert_eq!(s.of(&v, "박전담").band, "MORE");
    assert_eq!(s.of(&v, "박전담").band_label, "평균보다 많음");
    // 전담 평균 2.7, 허용폭 2 -> 0.7~4.7 이 평균 수준이므로 1회는 아직 '적음'이 아니다
    assert_eq!(s.of(&v, "최전담").band, "TYPICAL");

    // 담임은 담임끼리 견준다 (모두 0회이므로 표시하지 않는다)
    assert_eq!(s.of(&v, "김일가").band, "NONE");

    let sp = v.spreads.iter().find(|x| x.role_code == "SPECIAL").unwrap();
    assert_eq!(sp.people, 3);
    assert_eq!(sp.total, 8);
    assert_eq!((sp.max, sp.min, sp.spread), (6, 1, 5));
    assert_eq!(sp.max_name.as_deref(), Some("박전담"));
    assert!(sp.comparable);

    assert!(v.fairness_note.contains("참고용"));
}

#[test]
fn 보결_대상이_아닌_사람은_평균을_흔들지_않는다() {
    let (s, hr, sp1, sp2) = basic();
    let boss = s.teacher("최교장", "SPECIAL");
    s.conn
        .execute("UPDATE teachers SET is_substitutable = 0 WHERE id = ?1", [boss])
        .unwrap();

    s.assign(MON, 1, 1, 1, hr[0], sp1);
    s.assign(MON, 1, 1, 2, hr[0], sp2);

    let v = s.day(MON);
    let sp = v.spreads.iter().find(|x| x.role_code == "SPECIAL").unwrap();
    assert_eq!(sp.people, 2, "보결 대상 2명만 견준다");
    assert_eq!(s.of(&v, "최교장").band, "NONE");
    assert_eq!(s.of(&v, "최교장").period, 0);
}

// ============================================================
//  11. 담당 표시
// ============================================================

#[test]
fn 담임은_학급_전담은_과목이_담당으로_나온다() {
    let (s, hr, sp1, _) = basic();
    let sid: i64 = s
        .conn
        .query_row("SELECT id FROM subjects WHERE name = '영어'", [], |r| r.get(0))
        .unwrap();
    s.conn
        .execute(
            "INSERT INTO teacher_subjects(teacher_id, subject_id, is_primary) VALUES (?1, ?2, 1)",
            params![sp1, sid],
        )
        .unwrap();

    let v = s.day(MON);
    assert_eq!(s.of(&v, "김일가").duty, "1-가람");
    assert_eq!(s.of(&v, "박전담").duty, "영어");
    let _ = hr;
}

/// 시험 안에서만 쓰는 작은 도우미
impl OpenSlot {
    fn end_after(&self, m: i32) -> bool {
        self.end_min > m
    }
}
