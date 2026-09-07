//! 보결 수당 집계 시험 — 요구사항 8·12의 항목을 확인한다.
//!
//! 핵심은 **배정 기록만으로 계산된다**는 것이다. 그래서 배정을 취소하거나
//! 설정을 바꾼 뒤 다시 조회했을 때 숫자가 따라오는지를 본다.

use rusqlite::params;

use super::*;
use crate::db::memory_conn;
use crate::domain::pay::{ALL_ASSIGNED, DEDUCT_OWN_CAUSED};
use crate::repo::assign::{self as ra, AbsenceInput, AssignInput};
use crate::repo::settings::{self, SettingsInput};

// ============================================================
//  시험용 학교 — 1·5학년 × 2반
// ============================================================

fn hm(h: i32, m: i32) -> i32 {
    h * 60 + m
}

struct School {
    conn: Connection,
}

impl School {
    fn new() -> Self {
        let conn = memory_conn();
        conn.execute(
            "INSERT INTO school(id, name, min_grade, max_grade) VALUES (1, '한빛초등학교', 1, 6)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO terms(id, school_year, semester, name, is_current, start_date, end_date)
             VALUES (1, 2026, 2, '2026학년도 2학기', 1, '2026-09-01', '2027-02-28')",
            [],
        )
        .unwrap();

        // 저학년 4교시(3교시 뒤 점심) / 고학년 5교시(4교시 뒤 점심)
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

    /// 교시 보결 한 건. 돌려주는 값은 배정 id (취소 시험에 쓴다).
    fn assign(&self, date: &str, grade: i32, no: i32, period: i32, absent: i64, sub: i64) -> i64 {
        self.assign_slot(date, grade, no, "PERIOD", Some(period), absent, sub)
    }

    /// 점심 보결 한 건.
    fn assign_lunch(&self, date: &str, grade: i32, no: i32, absent: i64, sub: i64) -> i64 {
        self.assign_slot(date, grade, no, "LUNCH", None, absent, sub)
    }

    fn assign_slot(
        &self,
        date: &str,
        grade: i32,
        no: i32,
        slot_type: &str,
        period: Option<i32>,
        absent: i64,
        sub: i64,
    ) -> i64 {
        ra::assign_one(
            &self.conn,
            &AssignInput {
                date: date.into(),
                class_id: self.class_id(grade, no),
                slot_type: slot_type.into(),
                period_no: period,
                absent_teacher_id: Some(absent),
                sub_teacher_id: sub,
                absence_id: None,
                reason_code: None,
                reason_text: None,
            },
        )
        .unwrap_or_else(|e| {
            panic!("{date} {grade}-{no} {slot_type} 배정 실패: {}", e.user_message)
        })
        .id
    }

    fn absent(&self, teacher: i64, date: &str) -> i64 {
        ra::create_absence(
            &self.conn,
            &AbsenceInput {
                teacher_id: teacher,
                date: date.into(),
                is_all_day: true,
                start_min: None,
                end_min: None,
                reason_code: Some("ANNUAL".into()),
                reason_text: None,
            },
        )
        .unwrap()
    }

    fn set_pay(&self, policy: &str, per_case: i64) {
        settings::save(
            &self.conn,
            &SettingsInput {
                sub_pay_policy: Some(policy.into()),
                sub_pay_per_case: Some(per_case),
                ..Default::default()
            },
        )
        .unwrap();
    }

    fn month(&self, ym: &str) -> PayView {
        view(
            &self.conn,
            &PayQuery {
                mode: Some(MODE_MONTH.into()),
                month: Some(ym.into()),
                ..Default::default()
            },
        )
        .unwrap()
    }

    fn range(&self, from: &str, to: &str) -> PayView {
        view(
            &self.conn,
            &PayQuery {
                mode: Some(MODE_CUSTOM.into()),
                from: Some(from.into()),
                to: Some(to.into()),
                ..Default::default()
            },
        )
        .unwrap()
    }

    fn detail_of(&self, teacher: i64, from: &str, to: &str) -> PayDetail {
        detail(
            &self.conn,
            teacher,
            &PayQuery {
                mode: Some(MODE_CUSTOM.into()),
                from: Some(from.into()),
                to: Some(to.into()),
                ..Default::default()
            },
        )
        .unwrap()
    }
}

fn row<'a>(v: &'a PayView, name: &str) -> &'a PayRow {
    v.rows
        .iter()
        .find(|r| r.name == name)
        .unwrap_or_else(|| panic!("{name} 이 표에 없다: {:?}", v.rows.iter().map(|r| &r.name).collect::<Vec<_>>()))
}

/// 2026-09-07(월) ~ 09-11(금)
const MON: &str = "2026-09-07";
const TUE: &str = "2026-09-08";
const WED: &str = "2026-09-09";
const SEP: &str = "2026-09";

/// 담임 4명(1-가람, 1-나리, 5-가람, 5-나리) + 전담 2명.
///
/// **담임을 대신 들어갈 사람으로 쓸 수 없는 곳이 많다.** 담임은 자기 반
/// 수업 시간에 이미 바쁘고, 배정 엔진은 그것을 정확히 막는다 (그 규칙은
/// 이번 기능 때문에 손대지 않는다). 그래서 시험에서도 실제로 비어 있는
/// 사람만 쓴다.
///   - 전담(sp, sp2): 수업을 넣지 않았으므로 언제나 비어 있다
///   - 1학년 담임: 저학년은 4교시(~12:40)에 끝나므로 고학년 5교시(12:45~)에 들어갈 수 있다
///
/// 돌려주는 순서: [1-가람, 1-나리, 5-가람, 5-나리], 전담1, 전담2
fn basic() -> (School, Vec<i64>, i64, i64) {
    let s = School::new();
    let mut hr = Vec::new();
    for (grade, no, name) in [
        (1, 1, "김일가"),
        (1, 2, "김일나"),
        (5, 1, "박오가"),
        (5, 2, "박오나"),
    ] {
        let id = s.teacher(name, "HOMEROOM");
        s.set_homeroom(s.class_id(grade, no), id);
        hr.push(id);
    }
    let sp = s.teacher("이전담", "SPECIAL");
    let sp2 = s.teacher("최전담", "SPECIAL");
    (s, hr, sp, sp2)
}

// ============================================================
//  1) ASSIGNED 만 센다 / 2) 취소는 빠진다
// ============================================================

#[test]
fn 배정_상태인_건만_보결_횟수에_들어간다() {
    let (s, hr, sp, _sp2) = basic();
    s.set_pay(ALL_ASSIGNED, 15_000);

    // 이전담이 1-가람 담임을 대신해 3건
    s.assign(MON, 1, 1, 1, hr[0], sp);
    s.assign(MON, 1, 1, 2, hr[0], sp);
    let third = s.assign(MON, 1, 1, 3, hr[0], sp);

    let v = s.month(SEP);
    assert_eq!(row(&v, "이전담").substituted, 3);
    assert_eq!(row(&v, "이전담").amount, 45_000);

    // 한 건 취소 → 곧바로 빠진다
    ra::cancel(&s.conn, third, Some("착오")).unwrap();
    let v = s.month(SEP);
    assert_eq!(row(&v, "이전담").substituted, 2, "취소분은 세지 않는다");
    assert_eq!(row(&v, "이전담").amount, 30_000);
}

#[test]
fn 취소된_배정은_본인_발생_보결에서도_빠진다() {
    let (s, hr, sp, _sp2) = basic();
    s.set_pay(DEDUCT_OWN_CAUSED, 15_000);

    let a = s.assign(MON, 1, 1, 1, hr[0], sp);
    s.assign(MON, 1, 1, 2, hr[0], sp);

    assert_eq!(row(&s.month(SEP), "김일가").own_caused, 2);

    ra::cancel(&s.conn, a, None).unwrap();
    assert_eq!(
        row(&s.month(SEP), "김일가").own_caused,
        1,
        "취소된 건은 차감 근거가 되지 않는다"
    );
}

#[test]
fn 취소한_뒤_다시_배정하면_유효한_배정만_센다() {
    let (s, hr, sp, _sp2) = basic();
    s.set_pay(ALL_ASSIGNED, 15_000);

    // 이전담에게 배정했다가 취소하고 최전담에게 다시 배정
    let first = s.assign(MON, 1, 1, 1, hr[0], sp);
    ra::cancel(&s.conn, first, Some("교체")).unwrap();
    s.assign(MON, 1, 1, 1, hr[0], _sp2);

    let v = s.month(SEP);
    assert!(
        v.rows.iter().all(|r| r.name != "이전담"),
        "취소만 남은 사람은 표에 오르지 않는다"
    );
    assert_eq!(row(&v, "최전담").substituted, 1);
    assert_eq!(v.summary.total_substituted, 1, "같은 시간을 두 번 세지 않는다");
    assert_eq!(v.summary.total_amount, 15_000);
}

// ============================================================
//  3·4) 대신 들어간 건 / 본인 결근으로 생긴 건
// ============================================================

#[test]
fn 대신_들어간_건과_본인_발생_건을_따로_센다() {
    let (s, hr, sp, _sp2) = basic();
    s.set_pay(DEDUCT_OWN_CAUSED, 15_000);

    // 김일가(1-가람 담임)가 결근 → 이전담 2건, 최전담 1건이 대신 들어간다
    s.assign(MON, 1, 1, 1, hr[0], sp);
    s.assign(MON, 1, 1, 2, hr[0], sp);
    s.assign(MON, 1, 1, 3, hr[0], _sp2);

    // 이전담이 결근 → 김일가가 5-가람 5교시에 1건 들어간다
    // (저학년은 12:40에 끝나므로 12:45 시작인 고학년 5교시에 갈 수 있다)
    s.assign(TUE, 5, 1, 5, sp, hr[0]);

    let v = s.month(SEP);

    let kim = row(&v, "김일가");
    assert_eq!(kim.substituted, 1, "대신 들어간 것은 1건");
    assert_eq!(kim.own_caused, 3, "본인 결근으로 생긴 것은 3건");
    assert_eq!(kim.payable, 0, "1 - 3 은 음수이므로 0");
    assert_eq!(kim.amount, 0);

    let lee = row(&v, "이전담");
    assert_eq!(lee.substituted, 2);
    assert_eq!(lee.own_caused, 1);
    assert_eq!(lee.payable, 1);
    assert_eq!(lee.amount, 15_000);
}

// ============================================================
//  5) 미배정 결근은 차감하지 않는다
// ============================================================

#[test]
fn 아무도_배정되지_않은_결근은_차감에_들어가지_않는다() {
    let (s, hr, sp, _sp2) = basic();
    s.set_pay(DEDUCT_OWN_CAUSED, 15_000);

    // 김일가가 수요일 하루 종일 결근으로 등록되어 있지만 배정은 하나도 하지 않았다
    s.absent(hr[0], WED);
    // 김일가는 다른 날 고학년 5교시에 2건을 대신 들어갔다
    s.assign(MON, 5, 1, 5, hr[2], hr[0]);
    s.assign(TUE, 5, 1, 5, hr[2], hr[0]);

    let v = s.month(SEP);
    let kim = row(&v, "김일가");
    assert_eq!(kim.own_caused, 0, "결근만으로는 차감하지 않는다");
    assert_eq!(kim.payable, 2);
    assert_eq!(kim.amount, 30_000);

    // 그 결근에 한 건이라도 배정하면 그때부터 차감된다
    s.assign(WED, 1, 1, 1, hr[0], sp);
    let v = s.month(SEP);
    assert_eq!(row(&v, "김일가").own_caused, 1);
    assert_eq!(row(&v, "김일가").payable, 1);
    assert_eq!(row(&v, "김일가").amount, 15_000);
}

// ============================================================
//  6) 차감 결과가 음수면 0
// ============================================================

#[test]
fn 차감_결과가_음수여도_지급액은_0원이다() {
    let (s, hr, sp, _sp2) = basic();
    s.set_pay(DEDUCT_OWN_CAUSED, 15_000);

    // 김일가: 대신 들어간 것 없음, 본인 결근으로 3건 발생
    s.assign(MON, 1, 1, 1, hr[0], sp);
    s.assign(MON, 1, 1, 2, hr[0], sp);
    s.assign(MON, 1, 1, 3, hr[0], _sp2);

    let v = s.month(SEP);
    let kim = row(&v, "김일가");
    assert_eq!(kim.substituted, 0);
    assert_eq!(kim.own_caused, 3);
    assert_eq!(kim.payable, 0);
    assert_eq!(kim.amount, 0);
}

// ============================================================
//  7) 기간 경계
// ============================================================

#[test]
fn 기간_밖의_기록은_세지_않는다() {
    let (s, hr, sp, _sp2) = basic();
    s.set_pay(ALL_ASSIGNED, 10_000);

    s.assign("2026-08-31", 1, 1, 1, hr[0], sp); // 8월
    s.assign("2026-09-01", 1, 1, 1, hr[0], sp); // 9월 첫날
    s.assign("2026-09-30", 1, 1, 1, hr[0], sp); // 9월 마지막날
    s.assign("2026-10-01", 1, 1, 1, hr[0], sp); // 10월

    let sep = s.month("2026-09");
    assert_eq!(sep.from, "2026-09-01");
    assert_eq!(sep.to, "2026-09-30");
    assert_eq!(
        row(&sep, "이전담").substituted,
        2,
        "9월 첫날과 마지막날은 들어가고, 8월·10월은 빠진다"
    );
    assert_eq!(sep.summary.total_amount, 20_000);

    assert_eq!(row(&s.month("2026-08"), "이전담").substituted, 1);
    assert_eq!(row(&s.month("2026-10"), "이전담").substituted, 1);
}

#[test]
fn 사용자_지정_기간의_양_끝은_모두_포함된다() {
    let (s, hr, sp, _sp2) = basic();
    s.set_pay(ALL_ASSIGNED, 10_000);

    s.assign(MON, 1, 1, 1, hr[0], sp);
    s.assign(TUE, 1, 1, 1, hr[0], sp);
    s.assign(WED, 1, 1, 1, hr[0], sp);

    assert_eq!(row(&s.range(MON, WED), "이전담").substituted, 3);
    assert_eq!(row(&s.range(MON, TUE), "이전담").substituted, 2);
    assert_eq!(row(&s.range(TUE, TUE), "이전담").substituted, 1);
    assert!(
        s.range("2026-09-01", "2026-09-06").rows.is_empty(),
        "기록이 없는 기간은 빈 표"
    );
}

// ============================================================
//  8·9) 여러 교시 · 점심
// ============================================================

#[test]
fn 같은_날_여러_교시는_각각_한_건이다() {
    let (s, hr, sp, _sp2) = basic();
    s.set_pay(ALL_ASSIGNED, 15_000);

    s.assign(MON, 1, 1, 1, hr[0], sp);
    s.assign(MON, 1, 1, 2, hr[0], sp);
    s.assign(MON, 1, 1, 3, hr[0], sp);
    s.assign(MON, 1, 1, 4, hr[0], sp);

    let v = s.month(SEP);
    assert_eq!(row(&v, "이전담").substituted, 4);
    assert_eq!(row(&v, "이전담").amount, 60_000);
}

#[test]
fn 점심_보결도_한_건으로_센다() {
    let (s, hr, sp, _sp2) = basic();
    s.set_pay(ALL_ASSIGNED, 15_000);

    s.assign(MON, 1, 1, 1, hr[0], sp);
    s.assign_lunch(MON, 1, 1, hr[0], sp);

    let v = s.month(SEP);
    assert_eq!(row(&v, "이전담").substituted, 2, "점심도 정식 배정이면 1회");
    assert_eq!(row(&v, "이전담").amount, 30_000);

    let d = s.detail_of(sp, MON, MON);
    assert!(
        d.substituted.iter().any(|c| c.slot_label == "점심"),
        "상세에도 점심이 그대로 나온다"
    );
}

// ============================================================
//  요약과 정책 전환
// ============================================================

#[test]
fn 요약은_표의_합계와_같다() {
    let (s, hr, sp, _sp2) = basic();
    s.set_pay(DEDUCT_OWN_CAUSED, 15_000);

    s.assign(MON, 1, 1, 1, hr[0], sp);
    s.assign(MON, 1, 1, 2, hr[0], sp);
    s.assign(TUE, 5, 1, 5, sp, hr[1]);
    s.assign(WED, 1, 2, 1, hr[1], _sp2);

    let v = s.month(SEP);
    let sum_sub: i32 = v.rows.iter().map(|r| r.substituted).sum();
    let sum_own: i32 = v.rows.iter().map(|r| r.own_caused).sum();
    let sum_pay: i32 = v.rows.iter().map(|r| r.payable).sum();
    let sum_amt: i64 = v.rows.iter().map(|r| r.amount).sum();

    assert_eq!(v.summary.total_substituted, sum_sub);
    assert_eq!(v.summary.total_own_caused, sum_own);
    assert_eq!(v.summary.total_payable, sum_pay);
    assert_eq!(v.summary.total_amount, sum_amt);
    assert_eq!(
        v.summary.paid_teachers,
        v.rows.iter().filter(|r| r.payable > 0).count() as i32
    );
    assert_eq!(v.summary.total_substituted, 4, "배정은 모두 4건");
}

#[test]
fn 정책만_바꾸면_같은_기록으로_금액이_다시_계산된다() {
    let (s, hr, sp, _sp2) = basic();

    s.assign(MON, 1, 1, 1, hr[0], sp);
    s.assign(MON, 1, 1, 2, hr[0], sp);
    s.assign(MON, 1, 1, 3, hr[0], sp);
    // 이전담이 결근한 날 김일가가 대신 들어감 → 이전담의 본인 발생분 1건
    s.assign(TUE, 5, 1, 5, sp, hr[0]);

    s.set_pay(ALL_ASSIGNED, 15_000);
    let all = s.month(SEP);
    assert_eq!(row(&all, "이전담").payable, 3);
    assert_eq!(row(&all, "이전담").amount, 45_000);
    assert_eq!(row(&all, "이전담").own_caused, 1, "전체 지급에서도 보여 준다");

    s.set_pay(DEDUCT_OWN_CAUSED, 15_000);
    let deduct = s.month(SEP);
    assert_eq!(row(&deduct, "이전담").payable, 2);
    assert_eq!(row(&deduct, "이전담").amount, 30_000);

    // 기록 자체는 그대로다
    let n: i32 = s
        .conn
        .query_row("SELECT COUNT(*) FROM substitutions", [], |r| r.get(0))
        .unwrap();
    assert_eq!(n, 4, "정책을 바꿔도 배정 기록 수는 그대로다");
}

#[test]
fn 수당만_바꾸면_금액만_달라진다() {
    let (s, hr, sp, _sp2) = basic();
    s.assign(MON, 1, 1, 1, hr[0], sp);
    s.assign(MON, 1, 1, 2, hr[0], sp);

    s.set_pay(ALL_ASSIGNED, 15_000);
    assert_eq!(s.month(SEP).summary.total_amount, 30_000);

    s.set_pay(ALL_ASSIGNED, 20_000);
    let v = s.month(SEP);
    assert_eq!(v.summary.total_payable, 2, "횟수는 그대로");
    assert_eq!(v.summary.total_amount, 40_000);
    assert_eq!(row(&v, "이전담").per_case, 20_000);
}

#[test]
fn 수당을_정하지_않았으면_알려_준다() {
    let (s, hr, sp, _sp2) = basic();
    s.assign(MON, 1, 1, 1, hr[0], sp);

    let v = s.month(SEP);
    assert_eq!(v.per_case, 0, "기본값은 0원");
    assert!(v.per_case_unset, "화면에서 안내해야 한다");
    assert_eq!(v.policy, ALL_ASSIGNED, "기본 정책");
    assert_eq!(row(&v, "이전담").substituted, 1, "횟수는 그래도 센다");
    assert_eq!(row(&v, "이전담").amount, 0);
}

// ============================================================
//  기간 이동
// ============================================================

#[test]
fn 이전_다음_달을_알려_준다() {
    let (s, ..) = basic();

    let v = s.month("2026-09");
    assert_eq!(v.month, "2026-09");
    assert_eq!(v.prev_month, "2026-08");
    assert_eq!(v.next_month, "2026-10");
    assert_eq!(v.range_label, "2026년 9월");

    // 해를 넘는 경우
    let v = s.month("2026-12");
    assert_eq!(v.prev_month, "2026-11");
    assert_eq!(v.next_month, "2027-01");
    let v = s.month("2027-01");
    assert_eq!(v.prev_month, "2026-12");
}

#[test]
fn 잘못된_달은_이번_달로_되돌린다() {
    let (s, ..) = basic();
    for bad in ["", "2026-13", "abcd", "2026", "2026-00"] {
        let v = view(
            &s.conn,
            &PayQuery {
                mode: Some(MODE_MONTH.into()),
                month: Some(bad.into()),
                ..Default::default()
            },
        )
        .unwrap();
        let now = crate::domain::period::ym_of(chrono::Local::now().date_naive());
        assert_eq!(v.month, now, "'{bad}' 은 이번 달로 되돌린다");
    }
}

#[test]
fn 학기를_벗어나면_알려_주지만_막지_않는다() {
    let (s, hr, sp, _sp2) = basic();
    s.set_pay(ALL_ASSIGNED, 15_000);
    // 학기는 2026-09-01 ~ 2027-02-28 로 잡아 두었다
    s.assign("2026-08-31", 1, 1, 1, hr[0], sp);

    let v = s.month("2026-08");
    assert!(v.term_note.is_some(), "안내는 뜬다");
    assert_eq!(
        row(&v, "이전담").substituted,
        1,
        "그래도 기록이 있으면 계산한다"
    );

    assert!(s.month("2026-09").term_note.is_none(), "학기 안이면 조용하다");
}

// ============================================================
//  상세 — 계산 근거
// ============================================================

#[test]
fn 상세는_보결한_내역과_본인_발생분을_모두_보여_준다() {
    let (s, hr, sp, _sp2) = basic();
    s.set_pay(DEDUCT_OWN_CAUSED, 15_000);

    // 이전담: 대신 들어간 것 3건
    s.assign(MON, 1, 1, 1, hr[0], sp);
    s.assign(MON, 1, 1, 2, hr[0], sp);
    s.assign(TUE, 1, 2, 1, hr[1], sp);
    // 이전담 결근 → 김일가·김일나가 고학년 5교시에 대신 (본인 발생 2건)
    s.assign(WED, 5, 1, 5, sp, hr[0]);
    s.assign(WED, 5, 2, 5, sp, hr[1]);

    let d = s.detail_of(sp, MON, WED);
    assert_eq!(d.name, "이전담");
    assert_eq!(d.substituted.len(), 3);
    assert_eq!(d.own_caused.len(), 2);
    assert_eq!(d.payable, 1);
    assert_eq!(d.amount, 15_000);
    assert!(d.deducts);

    // 계산 설명
    assert_eq!(d.steps[0], "3회 - 2회 = 지급 인정 1회");
    assert_eq!(d.steps[1], "1회 × 15,000원 = 15,000원");

    // 상대 이름이 들어 있다
    assert_eq!(d.substituted[0].counterpart, "김일가", "결근한 선생님");
    let others: Vec<&str> = d.own_caused.iter().map(|c| c.counterpart.as_str()).collect();
    assert!(others.contains(&"김일가") && others.contains(&"김일나"), "{others:?}");

    // 날짜순
    assert!(d.substituted.windows(2).all(|w| w[0].date <= w[1].date));
}

#[test]
fn 전체_지급_정책의_상세는_차감하지_않는다고_알려_준다() {
    let (s, hr, sp, _sp2) = basic();
    s.set_pay(ALL_ASSIGNED, 15_000);

    s.assign(MON, 1, 1, 1, hr[0], sp);
    s.assign(MON, 1, 1, 2, hr[0], sp);
    s.assign(WED, 5, 1, 5, sp, hr[0]);

    let d = s.detail_of(sp, MON, WED);
    assert!(!d.deducts);
    assert_eq!(d.own_caused.len(), 1, "참고정보로 목록은 보여 준다");
    assert_eq!(d.payable, 2, "차감하지 않는다");
    assert_eq!(d.steps[0], "보결 2회 전체 인정");
    assert_eq!(d.steps[1], "2회 × 15,000원 = 30,000원");
}

#[test]
fn 상세의_학급_표기는_기록_당시_이름을_쓴다() {
    let (s, hr, sp, _sp2) = basic();
    s.set_pay(ALL_ASSIGNED, 15_000);
    s.assign(MON, 1, 1, 1, hr[0], sp);

    let before = s.detail_of(sp, MON, MON).substituted[0].class_label.clone();
    assert_eq!(before, "1-가람");

    // 반 이름을 바꿔도 과거 기록의 표기는 그대로여야 한다
    s.conn
        .execute("UPDATE classes SET name = '한빛' WHERE grade = 1 AND class_no = 1", [])
        .unwrap();

    let after = s.detail_of(sp, MON, MON).substituted[0].class_label.clone();
    assert_eq!(after, "1-가람", "스냅샷을 쓰므로 바뀌지 않는다");
}

#[test]
fn 상세_목록의_길이와_요약_횟수가_어긋나지_않는다() {
    let (s, hr, sp, _sp2) = basic();
    s.set_pay(DEDUCT_OWN_CAUSED, 15_000);

    s.assign(MON, 1, 1, 1, hr[0], sp);
    s.assign(MON, 1, 1, 2, hr[0], sp);
    s.assign(WED, 5, 1, 5, sp, hr[0]);

    let v = s.month(SEP);
    for r in &v.rows {
        let d = s.detail_of(r.teacher_id, &v.from, &v.to);
        assert_eq!(d.substituted.len() as i32, r.substituted, "{}", r.name);
        assert_eq!(d.own_caused.len() as i32, r.own_caused, "{}", r.name);
        assert_eq!(d.payable, r.payable, "{}", r.name);
        assert_eq!(d.amount, r.amount, "{}", r.name);
    }
}

// ============================================================
//  통계에 영향을 주지 않는다
// ============================================================

#[test]
fn 수당을_조회해도_기록과_통계는_그대로다() {
    use crate::repo::stats::{self, StatsQuery};

    let (s, hr, sp, _sp2) = basic();
    s.set_pay(DEDUCT_OWN_CAUSED, 15_000);
    s.assign(MON, 1, 1, 1, hr[0], sp);
    s.assign(MON, 1, 1, 2, hr[0], sp);

    let q = StatsQuery {
        preset: Some("CUSTOM".into()),
        from: Some(MON.into()),
        to: Some(MON.into()),
    };
    let before = stats::view(&s.conn, &q).unwrap();

    // 수당을 여러 번 조회하고 설정도 바꿔 본다
    let _ = s.month(SEP);
    s.set_pay(ALL_ASSIGNED, 99_000);
    let _ = s.month(SEP);
    let _ = s.detail_of(sp, MON, MON);
    let _ = sheets(&s.conn, &PayQuery::default()).unwrap();

    let after = stats::view(&s.conn, &q).unwrap();
    assert_eq!(before.summary.assigned, after.summary.assigned);
    assert_eq!(before.summary.required, after.summary.required);
    assert_eq!(before.summary.cancelled, after.summary.cancelled);
    assert_eq!(before.teachers.len(), after.teachers.len());
    for (b, a) in before.teachers.iter().zip(after.teachers.iter()) {
        assert_eq!((b.name.as_str(), b.period, b.total), (a.name.as_str(), a.period, a.total));
    }
}

// ============================================================
//  엑셀 내보내기
// ============================================================

use crate::repo::xlsx::Cell;

fn book(s: &School, ym: &str) -> Vec<crate::repo::xlsx::Sheet> {
    sheets(
        &s.conn,
        &PayQuery {
            mode: Some(MODE_MONTH.into()),
            month: Some(ym.into()),
            ..Default::default()
        },
    )
    .unwrap()
}

fn col_of(sh: &crate::repo::xlsx::Sheet, title: &str) -> usize {
    sh.columns
        .iter()
        .position(|c| c.title == title)
        .unwrap_or_else(|| {
            panic!(
                "'{title}' 열이 없다: {:?}",
                sh.columns.iter().map(|c| &c.title).collect::<Vec<_>>()
            )
        })
}

#[test]
fn 엑셀에_요청한_여섯_열이_들어간다() {
    let (s, hr, sp, _sp2) = basic();
    s.set_pay(DEDUCT_OWN_CAUSED, 15_000);
    s.assign(MON, 1, 1, 1, hr[0], sp);
    s.assign(MON, 1, 1, 2, hr[0], sp);
    s.assign(WED, 5, 1, 5, sp, hr[0]);

    let bk = book(&s, SEP);
    let table = &bk[0];
    assert_eq!(table.name, "교사별 수당");

    let want = [
        "교사명",
        "보결 횟수",
        "본인 발생 보결 횟수",
        "지급 인정 횟수",
        "1회 보결 수당",
        "지급액",
    ];
    for t in want {
        col_of(table, t);
    }

    // 이전담: 보결 2 · 본인발생 1 · 인정 1 · 15,000원
    let row = table
        .rows
        .iter()
        .find(|r| r[0] == Cell::text("이전담"))
        .expect("이전담 줄");
    assert_eq!(row[col_of(table, "보결 횟수")], Cell::Count(2));
    assert_eq!(row[col_of(table, "본인 발생 보결 횟수")], Cell::Count(1));
    assert_eq!(row[col_of(table, "지급 인정 횟수")], Cell::Count(1));
    assert_eq!(row[col_of(table, "1회 보결 수당")], Cell::Money(15_000));
    assert_eq!(row[col_of(table, "지급액")], Cell::Money(15_000));
}

#[test]
fn 엑셀은_금액과_횟수를_숫자로_담는다() {
    // 엑셀에서 그대로 더할 수 있어야 한다 — '15,000원' 은 글자가 된다
    let (s, hr, sp, _sp2) = basic();
    s.set_pay(ALL_ASSIGNED, 15_000);
    s.assign(MON, 1, 1, 1, hr[0], sp);

    let bk = book(&s, SEP);
    let table = &bk[0];
    for (ri, row) in table.rows.iter().enumerate() {
        for t in ["보결 횟수", "본인 발생 보결 횟수", "지급 인정 횟수"] {
            assert!(
                matches!(row[col_of(table, t)], Cell::Count(_)),
                "{ri}번째 줄 '{t}' 가 숫자가 아니다"
            );
        }
        for t in ["1회 보결 수당", "지급액"] {
            assert!(
                matches!(row[col_of(table, t)], Cell::Money(_)),
                "{ri}번째 줄 '{t}' 가 금액이 아니다"
            );
        }
    }
}

#[test]
fn 엑셀에_합계_행이_붙는다() {
    let (s, hr, sp, _sp2) = basic();
    s.set_pay(ALL_ASSIGNED, 15_000);
    s.assign(MON, 1, 1, 1, hr[0], sp);
    s.assign(MON, 1, 1, 2, hr[0], sp);

    let bk = book(&s, SEP);
    let table = &bk[0];
    let total = table.total.as_ref().expect("합계 행이 있어야 한다");
    assert_eq!(total[0], Cell::text("합계"));
    assert_eq!(total[col_of(table, "보결 횟수")], Cell::Count(2));
    assert_eq!(total[col_of(table, "지급액")], Cell::Money(30_000));
}

#[test]
fn 조회_조건_시트에_지급_기준과_금액이_남는다() {
    let (s, hr, sp, _sp2) = basic();
    s.set_pay(DEDUCT_OWN_CAUSED, 15_000);
    s.assign(MON, 1, 1, 1, hr[0], sp);

    let bk = book(&s, SEP);
    let cond = bk.iter().find(|x| x.name == "조회 조건").expect("조건 시트");
    assert!(!cond.filter, "조건 시트에는 필터를 걸지 않는다");

    let flat: String = cond
        .rows
        .iter()
        .map(|r| {
            r.iter()
                .map(|c| match c {
                    Cell::Text(v) => v.clone(),
                    other => format!("{other:?}"),
                })
                .collect::<Vec<_>>()
                .join("=")
        })
        .collect::<Vec<_>>()
        .join(" | ");

    assert!(flat.contains("본인으로 인해 발생한 보결 횟수 차감"), "{flat}");
    assert!(flat.contains("15,000원"), "조건에는 사람이 읽는 금액을 적는다: {flat}");
    assert!(flat.contains("2026년 9월"), "{flat}");
    assert!(flat.contains("DEDUCT_OWN_CAUSED"), "{flat}");
}

#[test]
fn 기록이_없으면_빈_표를_만든다() {
    let (s, ..) = basic();
    let bk = book(&s, "2026-01");
    let table = &bk[0];
    assert!(table.rows.is_empty());
    assert!(!table.columns.is_empty(), "머리글은 남는다");
    assert_eq!(
        table.total.as_ref().unwrap()[col_of(table, "지급액")],
        Cell::Money(0)
    );
}

// ============================================================
//  마이그레이션 — 기존 자료 보존
// ============================================================

#[test]
fn 마이그레이션은_설정_두_줄만_넣는다() {
    let conn = memory_conn();
    let keys: Vec<String> = conn
        .prepare("SELECT key FROM settings ORDER BY key")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();

    assert!(keys.contains(&"sub_pay_per_case".to_string()));
    assert!(keys.contains(&"sub_pay_policy".to_string()));

    let cfg = settings::pay_config(&conn).unwrap();
    assert_eq!(cfg.per_case, 0, "금액은 학교가 정하기 전까지 0원");
    assert_eq!(cfg.policy, ALL_ASSIGNED, "아무것도 깎지 않는 쪽이 기본");
}

#[test]
fn 잘못된_수당_설정은_저장하지_않는다() {
    let (s, ..) = basic();

    assert!(
        settings::save(
            &s.conn,
            &SettingsInput {
                sub_pay_per_case: Some(-1),
                ..Default::default()
            }
        )
        .is_err(),
        "음수 금액"
    );
    assert!(
        settings::save(
            &s.conn,
            &SettingsInput {
                sub_pay_per_case: Some(9_999_999),
                ..Default::default()
            }
        )
        .is_err(),
        "터무니없이 큰 금액"
    );
    assert!(
        settings::save(
            &s.conn,
            &SettingsInput {
                sub_pay_policy: Some("MONTHLY_CAP".into()),
                ..Default::default()
            }
        )
        .is_err(),
        "아직 없는 정책"
    );

    // 하나도 저장되지 않았다
    let cfg = settings::pay_config(&s.conn).unwrap();
    assert_eq!(cfg.per_case, 0);
    assert_eq!(cfg.policy, ALL_ASSIGNED);
}

#[test]
fn 설정_초기화는_수당을_건드리지_않는다() {
    let (s, ..) = basic();
    s.set_pay(DEDUCT_OWN_CAUSED, 15_000);

    settings::reset(&s.conn).unwrap();

    let cfg = settings::pay_config(&s.conn).unwrap();
    assert_eq!(cfg.per_case, 15_000, "학교가 정해 적어 둔 금액이다");
    assert_eq!(cfg.policy, DEDUCT_OWN_CAUSED);
}
