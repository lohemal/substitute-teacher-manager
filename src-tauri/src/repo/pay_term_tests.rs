//! 보결 수당의 '이번 학기' 조회 — 요청서의 Case A~M.
//!
//! ## 확인하려는 것 하나
//!
//! **'이번 학기'는 새로운 계산 방식이 아니다.** 지금 학기의 시작일과 종료일을
//! 기존 계산에 자동으로 넣어 주는 것뿐이다. 그래서 같은 날짜를 '기간 지정'
//! 으로 직접 넣으면 **모든 숫자가 한 글자도 다르지 않아야** 한다.
//!
//! 학기 날짜는 어디에도 베껴 두지 않는다. `terms` 의 현재 학기 한 곳만 본다.

use rusqlite::params;

use super::*;
use crate::db::memory_conn;
use crate::domain::pay::{ALL_ASSIGNED, DEDUCT_OWN_CAUSED};
use crate::repo::assign::{self as ra, AssignInput};
use crate::repo::settings::{self, SettingsInput};

/// 수업 시각은 이 시험에서 중요하지 않다. 날짜 범위만 본다.
const TERM_FROM: &str = "2026-09-01";
const TERM_TO: &str = "2027-02-28";

struct School {
    conn: Connection,
    kim: i64,
    lee: i64,
    park: i64,
}

impl School {
    /// 5학년 1~3반 · 요일마다 6교시까지 있는 시정표 하나.
    ///
    /// 세 선생님 모두 **전담으로 두고 담임을 두지 않는다.** 이 시험에서
    /// 보려는 것은 날짜 범위뿐이라, 누가 언제 수업 중인지가 끼어들면
    /// 시험의 뜻이 흐려진다. 그래서 늘 비어 있게 해 둔다.
    fn new() -> Self {
        Self::with_term(Some((TERM_FROM, TERM_TO)))
    }

    /// 학기 날짜를 바꾸거나, 현재 학기가 **없는** 상태를 만들 수 있다.
    ///
    /// 학기 행 자체는 늘 둔다 — 시정표·학급이 학기를 가리키기 때문이다.
    /// '현재 학기 없음' 은 `is_current = 0` 으로 만든다. 실제로 자료가
    /// 망가졌을 때의 모습도 이와 같다.
    fn with_term(term: Option<(&str, &str)>) -> Self {
        let conn = memory_conn();
        conn.execute(
            "INSERT INTO school(id, name, min_grade, max_grade) VALUES (1, '한빛초등학교', 1, 6)",
            [],
        )
        .unwrap();
        let (is_current, from, to) = match term {
            Some((a, b)) => (1, a, b),
            None => (0, TERM_FROM, TERM_TO),
        };
        conn.execute(
            "INSERT INTO terms(id, school_year, semester, name, is_current, start_date, end_date)
             VALUES (1, 2026, 2, '2026학년도 2학기', ?1, ?2, ?3)",
            params![is_current, from, to],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO bell_schedules(id, term_id, name) VALUES (1, 1, '기본')",
            [],
        )
        .unwrap();
        for day in 1..=5 {
            let mut t = 9 * 60;
            for p in 1..=6 {
                conn.execute(
                    "INSERT INTO bell_slots(bell_schedule_id, day_of_week, slot_type, period_no, label, start_min, end_min)
                     VALUES (1, ?1, 'PERIOD', ?2, ?3, ?4, ?5)",
                    params![day, p, format!("{p}교시"), t, t + 40],
                )
                .unwrap();
                t += 45;
            }
        }
        conn.execute(
            "INSERT INTO grade_bell_map(term_id, grade, bell_schedule_id) VALUES (1, 5, 1)",
            [],
        )
        .unwrap();

        let tid = |name: &str| {
            conn.execute(
                "INSERT INTO teachers(name, role_code) VALUES (?1, 'SPECIAL')",
                params![name],
            )
            .unwrap();
            conn.last_insert_rowid()
        };
        let kim = tid("김보결");
        let lee = tid("이보결");
        let park = tid("박결근");

        for (cid, no) in [(1i64, 1i32), (2, 2), (3, 3)] {
            conn.execute(
                "INSERT INTO classes(id, term_id, grade, class_no, name)
                 VALUES (?1, 1, 5, ?2, ?3)",
                params![cid, no, format!("{no}반")],
            )
            .unwrap();
        }

        let s = Self {
            conn,
            kim,
            lee,
            park,
        };
        s.set_pay(ALL_ASSIGNED, 15_000);
        s
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

    /// 박결근 선생님 대신 들어간 보결 한 건 (3반).
    fn assign(&self, date: &str, period: i32, sub: i64) -> i64 {
        self.assign_for(date, period, self.park, sub)
    }

    /// 결근한 선생님을 골라서. '본인 발생 보결' 을 만들 때 쓴다.
    fn assign_for(&self, date: &str, period: i32, absent: i64, sub: i64) -> i64 {
        ra::assign_one(
            &self.conn,
            &AssignInput {
                date: date.into(),
                class_id: 3,
                slot_type: "PERIOD".into(),
                period_no: Some(period),
                absent_teacher_id: Some(absent),
                sub_teacher_id: sub,
                absence_id: None,
                reason_code: None,
                reason_text: None,
            },
        )
        .unwrap_or_else(|e| panic!("{date} {period}교시 배정 실패: {}", e.user_message))
        .id
    }

    /// 조회 방식을 적지 않았을 때 — 메뉴에 처음 들어온 상태
    fn fresh(&self) -> PayView {
        view(&self.conn, &PayQuery::default()).unwrap()
    }

    fn term(&self) -> PayView {
        view(
            &self.conn,
            &PayQuery {
                mode: Some(MODE_TERM.into()),
                ..Default::default()
            },
        )
        .unwrap()
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
}

/// 두 조회 결과가 **계산된 숫자까지 모두** 같은지.
fn same_numbers(a: &PayView, b: &PayView, what: &str) {
    assert_eq!(a.from, b.from, "{what}: 시작일");
    assert_eq!(a.to, b.to, "{what}: 종료일");
    assert_eq!(
        a.summary.paid_teachers, b.summary.paid_teachers,
        "{what}: 지급 대상 교사 수"
    );
    assert_eq!(
        a.summary.listed_teachers, b.summary.listed_teachers,
        "{what}: 표에 오른 교사 수"
    );
    assert_eq!(
        a.summary.total_substituted, b.summary.total_substituted,
        "{what}: 보결 횟수"
    );
    assert_eq!(
        a.summary.total_own_caused, b.summary.total_own_caused,
        "{what}: 본인 발생 보결"
    );
    assert_eq!(
        a.summary.total_payable, b.summary.total_payable,
        "{what}: 인정 횟수"
    );
    assert_eq!(
        a.summary.total_amount, b.summary.total_amount,
        "{what}: 총 지급액"
    );
    assert_eq!(a.rows.len(), b.rows.len(), "{what}: 교사 수");
    for (x, y) in a.rows.iter().zip(b.rows.iter()) {
        assert_eq!(x.teacher_id, y.teacher_id, "{what}: 교사 순서");
        assert_eq!(x.substituted, y.substituted, "{what}: {} 보결 횟수", x.name);
        assert_eq!(x.own_caused, y.own_caused, "{what}: {} 본인 발생", x.name);
        assert_eq!(x.payable, y.payable, "{what}: {} 인정 횟수", x.name);
        assert_eq!(x.amount, y.amount, "{what}: {} 지급액", x.name);
    }
}

// ============================================================
//  Case A — 메뉴에 처음 들어왔을 때
// ============================================================

#[test]
fn case_a_기본_조회는_이번_학기다() {
    let s = School::new();
    let v = s.fresh();

    assert_eq!(v.mode, MODE_TERM, "조회 방식을 적지 않으면 이번 학기");
    assert_eq!(v.from, TERM_FROM);
    assert_eq!(v.to, TERM_TO);
    assert_eq!(v.term_label.as_deref(), Some("2026학년도 2학기"));
    assert!(!v.term_missing);

    // 이번 학기로 뽑은 것과 같다
    same_numbers(&v, &s.term(), "기본 진입 = 이번 학기");
}

// ============================================================
//  Case B — 학기 전체를 센다
// ============================================================

#[test]
fn case_b_학기_전체를_센다() {
    let s = School::new();
    // 2026-09-10(목) 2건 · 2026-10-05(월) 3건 · 2026-11-20(금) 1건
    s.assign("2026-09-10", 1, s.kim);
    s.assign("2026-09-10", 2, s.lee);
    s.assign("2026-10-05", 1, s.kim);
    s.assign("2026-10-05", 2, s.kim);
    s.assign("2026-10-05", 3, s.lee);
    s.assign("2026-11-20", 1, s.kim);

    let v = s.term();
    assert_eq!(v.summary.total_substituted, 6, "학기 전체 6건");
    assert_eq!(v.summary.total_payable, 6);
    assert_eq!(v.summary.total_amount, 6 * 15_000);

    // 한 달만 보면 그 달치만 — 이번 학기가 '이번 달'이 아님을 못 박는다
    assert_eq!(s.month("2026-10").summary.total_substituted, 3);
    assert_eq!(s.month("2026-09").summary.total_substituted, 2);
    assert_eq!(s.month("2026-11").summary.total_substituted, 1);
}

// ============================================================
//  Case C — 학기 밖은 세지 않는다 (양 끝은 포함)
// ============================================================

#[test]
fn case_c_학기_밖_기록은_빠지고_양_끝은_들어간다() {
    let s = School::new();
    s.assign("2026-08-31", 1, s.kim); // 학기 하루 전
    s.assign("2026-09-01", 1, s.kim); // 학기 첫날
    s.assign("2027-02-26", 1, s.kim); // 학기 마지막 주 (금)
    s.assign("2027-03-01", 1, s.kim); // 학기 하루 뒤

    let v = s.term();
    assert_eq!(v.summary.total_substituted, 2, "9/1 과 2/26 두 건만");

    // 종료일 당일도 포함되는지 따로 본다 (2027-02-28 은 일요일이라
    // 시정표에 없어 배정할 수 없으므로, 기록을 바로 넣어 확인한다)
    s.conn
        .execute(
            "INSERT INTO substitutions(term_id, date, day_of_week, class_id, grade, class_no,
                                       slot_type, period_no, slot_label, start_min, end_min,
                                       absent_teacher_id, sub_teacher_id, status)
             VALUES (1, '2027-02-28', 7, 3, 5, 3, 'PERIOD', 1, '1교시', 540, 580, ?1, ?2, 'ASSIGNED')",
            params![s.park, s.kim],
        )
        .unwrap();
    assert_eq!(s.term().summary.total_substituted, 3, "종료일 당일도 포함");
}

// ============================================================
//  Case D — 종료일이 미래여도 자르지 않는다
// ============================================================

#[test]
fn case_d_미래_종료일을_오늘로_자르지_않는다() {
    // 끝이 한참 뒤인 학기
    let s = School::with_term(Some(("2026-09-01", "2099-12-31")));
    let v = s.term();
    assert_eq!(v.to, "2099-12-31", "오늘로 잘라내면 안 된다");
    assert_eq!(v.from, "2026-09-01");

    // 미래 날짜의 기록도 그대로 세어진다 (실제로는 없겠지만, 자르지
    // 않는다는 것을 숫자로 보인다)
    s.conn
        .execute(
            "INSERT INTO substitutions(term_id, date, day_of_week, class_id, grade, class_no,
                                       slot_type, period_no, slot_label, start_min, end_min,
                                       absent_teacher_id, sub_teacher_id, status)
             VALUES (1, '2099-01-05', 1, 3, 5, 3, 'PERIOD', 1, '1교시', 540, 580, ?1, ?2, 'ASSIGNED')",
            params![s.park, s.kim],
        )
        .unwrap();
    assert_eq!(s.term().summary.total_substituted, 1);
}

// ============================================================
//  Case E · F — 월별 / 기간 지정은 그대로
// ============================================================

#[test]
fn case_e_월별은_예전_그대로다() {
    let s = School::new();
    s.assign("2026-09-10", 1, s.kim);
    s.assign("2026-10-05", 1, s.kim);
    s.assign("2026-10-06", 1, s.lee);

    let v = s.month("2026-10");
    assert_eq!(v.mode, MODE_MONTH);
    assert_eq!(v.from, "2026-10-01");
    assert_eq!(v.to, "2026-10-31");
    assert_eq!(v.range_label, "2026년 10월");
    assert_eq!(v.summary.total_substituted, 2);
    assert_eq!(v.summary.total_amount, 2 * 15_000);
    // 달 옮기기도 그대로
    assert_eq!(v.prev_month, "2026-09");
    assert_eq!(v.next_month, "2026-11");
}

#[test]
fn case_f_기간_지정은_예전_그대로다() {
    let s = School::new();
    s.assign("2026-09-10", 1, s.kim);
    s.assign("2026-09-30", 1, s.kim);
    s.assign("2026-10-20", 1, s.lee);

    let v = s.range("2026-09-15", "2026-10-15");
    assert_eq!(v.mode, MODE_CUSTOM);
    assert_eq!(v.from, "2026-09-15");
    assert_eq!(v.to, "2026-10-15");
    assert_eq!(v.summary.total_substituted, 1, "9/30 한 건만");
}

// ============================================================
//  Case G — 모드 전환
// ============================================================

#[test]
fn case_g_세_모드를_오가도_그때그때_맞는다() {
    let s = School::new();
    s.assign("2026-09-10", 1, s.kim);
    s.assign("2026-10-05", 1, s.kim);
    s.assign("2026-11-20", 1, s.lee);

    // 이번 학기 → 월별 → 기간 지정 → 이번 학기
    let a = s.term();
    assert_eq!((a.from.as_str(), a.to.as_str()), (TERM_FROM, TERM_TO));
    assert_eq!(a.summary.total_substituted, 3);

    let b = s.month("2026-10");
    assert_eq!((b.from.as_str(), b.to.as_str()), ("2026-10-01", "2026-10-31"));
    assert_eq!(b.summary.total_substituted, 1);

    let c = s.range("2026-09-01", "2026-10-31");
    assert_eq!(c.summary.total_substituted, 2);

    let d = s.term();
    same_numbers(&a, &d, "돌아왔을 때");
}

// ============================================================
//  Case H — 두 지급 기준 모두 그대로
// ============================================================

#[test]
fn case_h_지급_기준은_이번_학기에서도_그대로_적용된다() {
    let s = School::new();
    // 김보결: 대신 2건 / 본인 결근으로 1건 발생
    s.assign("2026-09-10", 1, s.kim);
    s.assign("2026-10-05", 1, s.kim);
    s.assign_for("2026-10-06", 1, s.kim, s.lee);

    // ALL_ASSIGNED — 대신 들어간 건을 모두 인정
    s.set_pay(ALL_ASSIGNED, 15_000);
    let v = s.term();
    let kim = v.rows.iter().find(|r| r.teacher_id == s.kim).unwrap();
    assert_eq!((kim.substituted, kim.own_caused), (2, 1));
    assert_eq!(kim.payable, 2);
    assert_eq!(kim.amount, 2 * 15_000);

    // DEDUCT_OWN_CAUSED — 본인 결근으로 생긴 건을 뺀다
    s.set_pay(DEDUCT_OWN_CAUSED, 15_000);
    let v = s.term();
    let kim = v.rows.iter().find(|r| r.teacher_id == s.kim).unwrap();
    assert_eq!((kim.substituted, kim.own_caused), (2, 1));
    assert_eq!(kim.payable, 1, "2 − 1");
    assert_eq!(kim.amount, 15_000);

    // 어느 기준이든 같은 날짜의 기간 지정과 똑같다
    same_numbers(&s.term(), &s.range(TERM_FROM, TERM_TO), "DEDUCT_OWN_CAUSED");
}

// ============================================================
//  Case I · J — 취소분 · 보결 불필요는 들어오지 않는다
// ============================================================

#[test]
fn case_i_취소한_배정은_이번_학기에서도_빠진다() {
    let s = School::new();
    let keep = s.assign("2026-09-10", 1, s.kim);
    let drop = s.assign("2026-10-05", 1, s.kim);
    assert_eq!(s.term().summary.total_substituted, 2);

    ra::cancel(&s.conn, drop, Some("결근 취소")).unwrap();

    let v = s.term();
    assert_eq!(v.summary.total_substituted, 1, "취소분은 빠진다");
    assert_eq!(v.summary.total_amount, 15_000);
    // 기록 자체는 남아 있다
    let n: i64 = s
        .conn
        .query_row(
            "SELECT COUNT(*) FROM substitutions WHERE status = 'CANCELLED'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(n, 1);
    assert!(keep > 0);
}

#[test]
fn case_j_보결_불필요는_이번_학기_수당에_들어오지_않는다() {
    let s = School::new();
    s.assign("2026-09-10", 1, s.kim);

    // 박결근 선생님이 3반 2교시를 맡고 있고, 그 날 종일 결근한다.
    // 그러면 그 칸이 '보결 필요' 로 잡히고, 그것을 보결 불필요로 돌린다.
    s.conn
        .execute(
            "INSERT INTO lessons(term_id, class_id, teacher_id, day_of_week, period_no, lesson_type, replaces_homeroom)
             VALUES (1, 3, ?1, 4, 2, 'SPECIAL', 1)",
            params![s.park],
        )
        .unwrap();
    ra::create_absence(
        &s.conn,
        &ra::AbsenceInput {
            teacher_id: s.park,
            date: "2026-09-10".into(), // 목요일
            is_all_day: true,
            start_min: None,
            end_min: None,
            reason_code: Some("SICK".into()),
            reason_text: None,
        },
    )
    .unwrap();
    crate::repo::waiver::waive(
        &s.conn,
        &crate::repo::waiver::WaiverInput {
            date: "2026-09-10".into(),
            class_id: 3,
            slot_type: "PERIOD".into(),
            period_no: Some(2),
            absent_teacher_id: s.park,
            reason_code: crate::repo::waiver::REASON_SPECIAL_CHANGED.into(),
            note: None,
        },
    )
    .unwrap();

    let v = s.term();
    assert_eq!(v.summary.total_substituted, 1, "실제 배정 1건뿐");
    assert_eq!(v.summary.total_payable, 1);
    assert_eq!(v.summary.total_amount, 15_000);

    let waivers: i64 = s
        .conn
        .query_row("SELECT COUNT(*) FROM substitution_waivers", [], |r| r.get(0))
        .unwrap();
    assert_eq!(waivers, 1, "기록은 남아 있지만 수당에는 들어오지 않는다");
}

// ============================================================
//  Case K — 현재 학기가 없을 때
// ============================================================

#[test]
fn case_k_현재_학기가_없으면_날짜를_넘겨짚지_않는다() {
    let s = School::with_term(None);

    let v = s.fresh();
    assert_eq!(v.mode, MODE_TERM);
    assert!(v.term_missing, "화면에서 알릴 수 있어야 한다");
    assert_eq!(v.term_label, None);
    assert_eq!(v.from, "", "날짜를 지어내지 않는다");
    assert_eq!(v.to, "");
    assert_eq!(v.summary.total_substituted, 0);
    assert_eq!(v.summary.total_amount, 0);
    assert!(v.rows.is_empty());
    // '학기를 벗어났다'는 안내는 뜻이 없으므로 띄우지 않는다
    assert_eq!(v.term_note, None);

    // 월별과 기간 지정은 그대로 쓸 수 있다
    s.conn
        .execute(
            "INSERT INTO substitutions(term_id, date, day_of_week, class_id, grade, class_no,
                                       slot_type, period_no, slot_label, start_min, end_min,
                                       absent_teacher_id, sub_teacher_id, status)
             VALUES (NULL, '2026-10-05', 1, 3, 5, 3, 'PERIOD', 1, '1교시', 540, 580, ?1, ?2, 'ASSIGNED')",
            params![s.park, s.kim],
        )
        .unwrap();

    let m = s.month("2026-10");
    assert!(!m.term_missing, "월별에는 학기가 필요 없다");
    assert_eq!(m.summary.total_substituted, 1);
    assert_eq!(m.summary.total_amount, 15_000);

    let r = s.range("2026-10-01", "2026-10-31");
    assert_eq!(r.summary.total_substituted, 1);
}

// ============================================================
//  Case L — 학기를 바꾸면 따라온다
// ============================================================

#[test]
fn case_l_학기를_바꾸면_이번_학기도_따라온다() {
    let s = School::new();
    assert_eq!(s.term().from, TERM_FROM);
    assert_eq!(s.term().to, TERM_TO);

    // 새 학기로 전환
    s.conn
        .execute("UPDATE terms SET is_current = 0 WHERE id = 1", [])
        .unwrap();
    s.conn
        .execute(
            "INSERT INTO terms(id, school_year, semester, name, is_current, start_date, end_date)
             VALUES (2, 2027, 1, '2027학년도 1학기', 1, '2027-03-01', '2027-08-31')",
            [],
        )
        .unwrap();

    let v = s.term();
    assert_eq!(v.from, "2027-03-01", "새 학기를 따라야 한다");
    assert_eq!(v.to, "2027-08-31");
    assert_eq!(v.term_label.as_deref(), Some("2027학년도 1학기"));
    assert!(!v.term_missing);
}

/// 학기에 날짜가 적혀 있지 않으면 학년도·학기로 계산한다 (기존 규칙).
#[test]
fn 학기에_날짜가_없으면_학사_일정으로_계산한다() {
    let s = School::new();
    s.conn
        .execute(
            "UPDATE terms SET start_date = NULL, end_date = NULL WHERE id = 1",
            [],
        )
        .unwrap();

    let v = s.term();
    assert_eq!(v.from, "2026-09-01", "2학기는 9/1 부터");
    assert_eq!(v.to, "2027-02-28", "다음 해 2월 말까지");
    assert!(!v.term_missing);
}

// ============================================================
//  Case M — 엑셀
// ============================================================

#[test]
fn case_m_엑셀이_화면과_같고_조회_방식이_적힌다() {
    let s = School::new();
    s.assign("2026-09-10", 1, s.kim);
    s.assign("2026-10-05", 1, s.kim);
    s.assign("2026-11-20", 1, s.lee);

    let q = PayQuery {
        mode: Some(MODE_TERM.into()),
        ..Default::default()
    };
    let v = view(&s.conn, &q).unwrap();
    let made = sheets(&s.conn, &q).unwrap();

    // 조회 조건 시트
    let cond = made.iter().find(|x| x.name == "조회 조건").expect("조회 조건 시트");
    let text = |key: &str| -> Option<String> {
        cond.rows.iter().find_map(|r| match (&r[0], &r[1]) {
            (Cell::Text(k), Cell::Text(val)) if k == key => Some(val.clone()),
            _ => None,
        })
    };
    assert_eq!(text("조회 방식").as_deref(), Some("이번 학기"));
    assert_eq!(text("학기").as_deref(), Some("2026학년도 2학기"));
    assert_eq!(
        text("조회 기간").as_deref(),
        Some(format!("{TERM_FROM} ~ {TERM_TO}").as_str())
    );

    // 교사별 시트의 합계가 화면 요약과 같다
    let table = made.iter().find(|x| x.name == "교사별 수당").expect("교사별 시트");
    let total = table.total.as_ref().expect("합계 행");
    assert!(
        matches!(total[3], Cell::Count(n) if n == v.summary.total_substituted as i64),
        "보결 횟수 합계"
    );
    assert!(
        matches!(total[5], Cell::Count(n) if n == v.summary.total_payable as i64),
        "인정 횟수 합계"
    );
    assert!(
        matches!(total[7], Cell::Money(n) if n == v.summary.total_amount),
        "지급액 합계"
    );
    assert_eq!(table.rows.len(), v.rows.len(), "교사 수");

    // 월별·기간 지정에도 조회 방식이 적힌다 (학기 줄은 없다)
    for (q, want) in [
        (
            PayQuery {
                mode: Some(MODE_MONTH.into()),
                month: Some("2026-10".into()),
                ..Default::default()
            },
            "월별",
        ),
        (
            PayQuery {
                mode: Some(MODE_CUSTOM.into()),
                from: Some("2026-09-15".into()),
                to: Some("2026-10-15".into()),
                ..Default::default()
            },
            "기간 지정",
        ),
    ] {
        let sh = sheets(&s.conn, &q).unwrap();
        let cond = sh.iter().find(|x| x.name == "조회 조건").unwrap();
        let has = |key: &str| {
            cond.rows
                .iter()
                .any(|r| matches!(&r[0], Cell::Text(k) if k == key))
        };
        let got = cond.rows.iter().find_map(|r| match (&r[0], &r[1]) {
            (Cell::Text(k), Cell::Text(v)) if k == "조회 방식" => Some(v.clone()),
            _ => None,
        });
        assert_eq!(got.as_deref(), Some(want));
        assert!(!has("학기"), "{want} 에는 학기 줄을 넣지 않는다");
        assert!(has("조회 기간"));
    }
}

// ============================================================
//  핵심 — 이번 학기는 계산 방식이 아니라 날짜 preset 이다
// ============================================================

#[test]
fn 이번_학기와_같은_날짜의_기간_지정은_완전히_같다() {
    let s = School::new();
    // 학기 안팎에 골고루 뿌린다
    for (date, sub) in [
        ("2026-08-28", s.kim),
        ("2026-09-01", s.kim),
        ("2026-09-10", s.lee),
        ("2026-10-05", s.kim),
        ("2026-12-11", s.lee),
        ("2027-02-26", s.kim),
        ("2027-03-02", s.lee),
    ] {
        s.assign(date, 1, sub);
    }
    // 본인 발생분도 섞는다
    s.assign_for("2026-11-10", 2, s.kim, s.lee);

    for policy in [ALL_ASSIGNED, DEDUCT_OWN_CAUSED] {
        s.set_pay(policy, 15_000);
        let t = s.term();
        let c = s.range(TERM_FROM, TERM_TO);
        same_numbers(&t, &c, policy);

        // 상세 내역도 같은 기간을 본다
        for tid in [s.kim, s.lee] {
            let dt = detail(
                &s.conn,
                tid,
                &PayQuery {
                    mode: Some(MODE_TERM.into()),
                    ..Default::default()
                },
            )
            .unwrap();
            let dc = detail(
                &s.conn,
                tid,
                &PayQuery {
                    mode: Some(MODE_CUSTOM.into()),
                    from: Some(TERM_FROM.into()),
                    to: Some(TERM_TO.into()),
                    ..Default::default()
                },
            )
            .unwrap();
            assert_eq!((dt.from.as_str(), dt.to.as_str()), (TERM_FROM, TERM_TO));
            assert_eq!(dt.substituted.len(), dc.substituted.len(), "{policy} 상세 건수");
            assert_eq!(dt.own_caused.len(), dc.own_caused.len());
            assert_eq!(dt.payable, dc.payable);
            assert_eq!(dt.amount, dc.amount);
            assert_eq!(dt.steps, dc.steps);

            // 요약의 그 사람 값과 상세가 어긋나지 않는다
            if let Some(r) = t.rows.iter().find(|r| r.teacher_id == tid) {
                assert_eq!(r.substituted as usize, dt.substituted.len());
                assert_eq!(r.payable, dt.payable);
                assert_eq!(r.amount, dt.amount);
            }
        }
    }
}

#[test]
fn 조회_방식_이름() {
    assert_eq!(mode_label(MODE_TERM), "이번 학기");
    assert_eq!(mode_label(MODE_MONTH), "월별");
    assert_eq!(mode_label(MODE_CUSTOM), "기간 지정");
}
