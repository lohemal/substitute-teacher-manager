//! 보결 불필요 — 요청서의 Case A~L.
//!
//! ## 무엇을 확인하는가
//!
//! 종일 결근을 등록하면 그 선생님이 그 날 맡은 시간에서 보결 필요 교시가
//! 나온다. 그런데 등록한 뒤에 일정이 바뀌는 일이 있다 — 가장 흔한 것이
//! 전담시간 변경이다. 그런 칸을 **기록을 남기면서** 미배정에서 빼는 것이
//! 이 기능이다.
//!
//! 지우는 것이 아니다. 결근 기록도, 배정 기록도 그대로 두고, '이 칸은
//! 보결하지 않기로 했다'는 사실만 따로 적는다.

use rusqlite::params;

use super::*;
use crate::db::memory_conn;
use crate::domain::period;
use crate::repo::assign::{self as ra, AbsenceInput, AssignInput};
use crate::repo::stats;

const MON: &str = "2026-09-07";

fn hm(h: i32, m: i32) -> i32 {
    h * 60 + m
}

/// 5학년 가람·나리 두 반. 5교시까지이고 점심 구간은 두지 않는다 —
/// 종일 결근 하나에서 정확히 **5칸**이 나오게 해서 숫자를 또렷하게 본다.
///
/// 교사
///   김결근 — 5-가람 담임 (이 사람이 종일 결근한다)
///   박전담 · 최전담 — 그 날 수업이 없어 늘 후보가 된다
struct School {
    conn: Connection,
    absent: i64,
    sub_a: i64,
    sub_b: i64,
    class: i64,
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
            "INSERT INTO terms(id, school_year, semester, name, is_current)
             VALUES (1, 2026, 2, '2026학년도 2학기', 1)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO bell_schedules(id, term_id, name) VALUES (1, 1, '기본')",
            [],
        )
        .unwrap();
        let mut t = hm(9, 0);
        for p in 1..=5 {
            conn.execute(
                "INSERT INTO bell_slots(bell_schedule_id, day_of_week, slot_type, period_no, label, start_min, end_min)
                 VALUES (1, 1, 'PERIOD', ?1, ?2, ?3, ?4)",
                params![p, format!("{p}교시"), t, t + 40],
            )
            .unwrap();
            t += 45;
        }
        conn.execute(
            "INSERT INTO grade_bell_map(term_id, grade, bell_schedule_id) VALUES (1, 5, 1)",
            [],
        )
        .unwrap();

        let tid = |name: &str, role: &str| {
            conn.execute(
                "INSERT INTO teachers(name, role_code) VALUES (?1, ?2)",
                params![name, role],
            )
            .unwrap();
            conn.last_insert_rowid()
        };
        let absent = tid("김결근", "HOMEROOM");
        let sub_a = tid("박전담", "SPECIAL");
        let sub_b = tid("최전담", "SPECIAL");

        conn.execute(
            "INSERT INTO classes(id, term_id, grade, class_no, name, homeroom_teacher_id)
             VALUES (1, 1, 5, 1, '가람', ?1)",
            params![absent],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO classes(id, term_id, grade, class_no, name)
             VALUES (2, 1, 5, 2, '나리')",
            [],
        )
        .unwrap();

        Self {
            conn,
            absent,
            sub_a,
            sub_b,
            class: 1,
        }
    }

    /// 김결근을 종일 결근으로 등록한다.
    fn absent_all_day(&self) -> i64 {
        ra::create_absence(
            &self.conn,
            &AbsenceInput {
                teacher_id: self.absent,
                date: MON.into(),
                is_all_day: true,
                start_min: None,
                end_min: None,
                reason_code: Some("SICK".into()),
                reason_text: None,
            },
        )
        .unwrap()
    }

    fn plan(&self) -> ra::DayPlan {
        ra::day_plan(&self.conn, MON, self.absent).unwrap()
    }

    fn stats(&self) -> stats::StatsView {
        stats::view(
            &self.conn,
            &stats::StatsQuery {
                preset: Some(period::CUSTOM.into()),
                from: Some(MON.into()),
                to: Some(MON.into()),
            },
        )
        .unwrap()
    }

    fn waive(&self, period: i32, reason: &str, note: Option<&str>) -> AppResult<i64> {
        crate::repo::waiver::waive(
            &self.conn,
            &crate::repo::waiver::WaiverInput {
                date: MON.into(),
                class_id: self.class,
                slot_type: "PERIOD".into(),
                period_no: Some(period),
                absent_teacher_id: self.absent,
                reason_code: reason.into(),
                note: note.map(|s| s.to_string()),
            },
        )
    }

    fn assign(&self, period: i32, sub: i64) -> AppResult<ra::AssignSaved> {
        ra::assign_one(
            &self.conn,
            &AssignInput {
                date: MON.into(),
                class_id: self.class,
                slot_type: "PERIOD".into(),
                period_no: Some(period),
                absent_teacher_id: Some(self.absent),
                sub_teacher_id: sub,
                absence_id: None,
                reason_code: None,
                reason_text: None,
            },
        )
    }

    /// 그 교시의 계획 칸
    fn slot(&self, plan: &ra::DayPlan, period: i32) -> ra::PlanSlot {
        plan.slots
            .iter()
            .find(|s| s.period_no == Some(period))
            .unwrap_or_else(|| panic!("{period}교시 칸이 있어야 한다"))
            .clone()
    }
}

// ============================================================
//  Case A — 손대지 않으면 전부 미배정
// ============================================================

#[test]
fn case_a_종일_결근이면_다섯_칸이_미배정이다() {
    let s = School::new();
    s.absent_all_day();

    let plan = s.plan();
    assert_eq!(plan.slots.len(), 5, "1~5교시");
    assert!(plan.slots.iter().all(|x| x.waiver.is_none()));

    let v = s.stats();
    assert_eq!(v.summary.required, 5);
    assert_eq!(v.summary.covered, 0);
    assert_eq!(v.summary.unassigned, 5);
    assert_eq!(v.summary.not_required, 0);
    assert_eq!(v.open_slots.len(), 5);
}

// ============================================================
//  Case B — 한 칸을 보결 불필요로
// ============================================================

#[test]
fn case_b_한_칸을_보결_불필요로_하면_미배정에서_빠진다() {
    let s = School::new();
    s.absent_all_day();
    s.waive(3, crate::repo::waiver::REASON_SPECIAL_CHANGED, None).unwrap();

    let v = s.stats();
    assert_eq!(v.summary.required, 5, "필요했던 칸 수는 그대로다");
    assert_eq!(v.summary.unassigned, 4, "미배정은 4여야 한다");
    assert_eq!(v.summary.not_required, 1);
    assert_eq!(v.summary.covered, 0);

    // 미배정 목록에 3교시가 없다
    assert_eq!(v.open_slots.len(), 4);
    assert!(
        !v.open_slots.iter().any(|o| o.slot_label == "3교시"),
        "3교시가 미배정 목록에 남아 있다"
    );

    // 사유가 남는다
    let plan = s.plan();
    let w = s.slot(&plan, 3).waiver.expect("보결 불필요 기록이 있어야 한다");
    assert_eq!(w.reason_code, crate::repo::waiver::REASON_SPECIAL_CHANGED);
    assert_eq!(w.reason_label, "전담시간 변경");
    assert!(w.note.is_none());

    // 날짜별·결근별 숫자도 같이 맞는다
    assert_eq!(v.days[0].unassigned, 4);
    assert_eq!(v.days[0].not_required, 1);
    assert_eq!(v.absences[0].unassigned, 4);
    assert_eq!(v.absences[0].not_required, 1);
}

#[test]
fn 기타_사유에는_메모를_남길_수_있다() {
    let s = School::new();
    s.absent_all_day();
    s.waive(2, crate::repo::waiver::REASON_OTHER, Some("  체험학습으로 수업 없음  "))
        .unwrap();

    let plan = s.plan();
    let w = s.slot(&plan, 2).waiver.unwrap();
    assert_eq!(w.reason_label, "기타");
    assert_eq!(w.note.as_deref(), Some("체험학습으로 수업 없음"));
}

#[test]
fn 모르는_사유는_막는다() {
    let s = School::new();
    s.absent_all_day();
    let e = s.waive(2, "WHATEVER", None).unwrap_err();
    assert!(e.user_message.contains("사유"), "{}", e.user_message);
}

#[test]
fn 보결이_필요하지_않은_시간은_처리할_수_없다() {
    let s = School::new();
    s.absent_all_day();
    // 6교시는 시정표에 없다
    let e = s.waive(6, crate::repo::waiver::REASON_SPECIAL_CHANGED, None).unwrap_err();
    assert!(e.user_message.contains("보결이 필요한 시간이 아닙니다"), "{}", e.user_message);
}

#[test]
fn 같은_칸을_두_번_처리하지_않는다() {
    let s = School::new();
    s.absent_all_day();
    s.waive(3, crate::repo::waiver::REASON_SPECIAL_CHANGED, None).unwrap();
    let e = s.waive(3, crate::repo::waiver::REASON_OTHER, None).unwrap_err();
    assert!(e.user_message.contains("이미"), "{}", e.user_message);
}

// ============================================================
//  Case C — 일부 배정 + 일부 불필요
// ============================================================

#[test]
fn case_c_배정_둘_불필요_하나_미배정_둘() {
    let s = School::new();
    s.absent_all_day();
    s.assign(1, s.sub_a).unwrap();
    s.assign(2, s.sub_b).unwrap();
    s.waive(3, crate::repo::waiver::REASON_SPECIAL_CHANGED, None).unwrap();

    let v = s.stats();
    assert_eq!(v.summary.required, 5);
    assert_eq!(v.summary.covered, 2, "배정 2");
    assert_eq!(v.summary.not_required, 1, "보결 불필요 1");
    assert_eq!(v.summary.unassigned, 2, "미배정 2");
    assert_eq!(v.open_slots.len(), 2);
    assert_eq!(
        v.open_slots.iter().map(|o| o.slot_label.as_str()).collect::<Vec<_>>(),
        ["4교시", "5교시"]
    );
}

// ============================================================
//  Case D — 되돌리기
// ============================================================

#[test]
fn case_d_되돌리면_다시_미배정이_된다() {
    let s = School::new();
    s.absent_all_day();
    s.assign(1, s.sub_a).unwrap();
    s.assign(2, s.sub_b).unwrap();
    let id = s.waive(3, crate::repo::waiver::REASON_SPECIAL_CHANGED, None).unwrap();

    crate::repo::waiver::revoke(&s.conn, id).unwrap();

    let v = s.stats();
    assert_eq!(v.summary.covered, 2);
    assert_eq!(v.summary.unassigned, 3, "3교시가 다시 미배정이 된다");
    assert_eq!(v.summary.not_required, 0);
    assert!(v.open_slots.iter().any(|o| o.slot_label == "3교시"));

    // 화면에서도 보결 불필요 표시가 사라지고 후보가 보인다
    let plan = s.plan();
    let slot = s.slot(&plan, 3);
    assert!(slot.waiver.is_none());
    assert!(!slot.candidates.is_empty(), "다시 후보를 고를 수 있어야 한다");

    // 기록은 지우지 않고 남겨 둔다
    let (status, revoked): (String, Option<String>) = s
        .conn
        .query_row(
            "SELECT status, revoked_at FROM substitution_waivers WHERE id = ?1",
            params![id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(status, crate::repo::waiver::STATUS_REVOKED);
    assert!(revoked.is_some(), "되돌린 시각이 남아야 한다");
}

#[test]
fn 두_번_되돌리지_않는다() {
    let s = School::new();
    s.absent_all_day();
    let id = s.waive(3, crate::repo::waiver::REASON_SPECIAL_CHANGED, None).unwrap();
    crate::repo::waiver::revoke(&s.conn, id).unwrap();
    let e = crate::repo::waiver::revoke(&s.conn, id).unwrap_err();
    assert!(e.user_message.contains("되돌렸"), "{}", e.user_message);
}

#[test]
fn 되돌린_뒤_같은_칸을_다시_처리할_수_있다() {
    let s = School::new();
    s.absent_all_day();
    let first = s.waive(3, crate::repo::waiver::REASON_SPECIAL_CHANGED, None).unwrap();
    crate::repo::waiver::revoke(&s.conn, first).unwrap();
    let second = s.waive(3, crate::repo::waiver::REASON_OTHER, Some("다시 바뀜")).unwrap();
    assert_ne!(first, second);
    assert_eq!(s.stats().summary.not_required, 1);
}

// ============================================================
//  Case E · F — 횟수와 수당에 섞이지 않는다
// ============================================================

#[test]
fn case_e_보결_불필요는_어느_교사의_횟수에도_들어가지_않는다() {
    let s = School::new();
    s.absent_all_day();
    s.assign(1, s.sub_a).unwrap();
    s.waive(3, crate::repo::waiver::REASON_SPECIAL_CHANGED, None).unwrap();

    let counts = repo_find_counts(&s.conn);
    assert_eq!(counts.get(&s.sub_a).map(|c| c.today), Some(1));
    assert_eq!(counts.get(&s.sub_a).map(|c| c.total), Some(1));
    assert!(
        counts.get(&s.sub_b).is_none(),
        "아무것도 하지 않은 선생님은 0회"
    );

    let v = s.stats();
    let total: i32 = v.teachers.iter().map(|t| t.period).sum();
    assert_eq!(total, 1, "실제 배정 1건뿐이다");
    assert_eq!(v.summary.assigned, 1);
}

#[test]
fn case_f_보결_불필요는_수당에_들어가지_않는다() {
    let s = School::new();
    s.absent_all_day();
    s.assign(1, s.sub_a).unwrap();
    s.waive(3, crate::repo::waiver::REASON_SPECIAL_CHANGED, None).unwrap();
    crate::repo::settings::put_int(&s.conn, crate::repo::settings::SUB_PAY_PER_CASE, 15_000).unwrap();

    let v = crate::repo::pay::view(
        &s.conn,
        &crate::repo::pay::PayQuery {
            mode: Some("CUSTOM".into()),
            month: None,
            from: Some(MON.into()),
            to: Some(MON.into()),
        },
    )
    .unwrap();

    assert_eq!(v.summary.total_substituted, 1, "보결 1회");
    assert_eq!(v.summary.total_payable, 1, "인정 1회");
    assert_eq!(v.summary.total_amount, 15_000);
}

// ============================================================
//  Case G — 결근 기록은 그대로
// ============================================================

#[test]
fn case_g_보결_불필요로_해도_결근_기록은_그대로다() {
    let s = School::new();
    let absence_id = s.absent_all_day();
    for p in 1..=5 {
        s.waive(p, crate::repo::waiver::REASON_SPECIAL_CHANGED, None).unwrap();
    }

    let (n, status, all_day): (i64, String, i64) = s
        .conn
        .query_row(
            "SELECT COUNT(*), MAX(status), MAX(is_all_day) FROM absences WHERE id = ?1",
            params![absence_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(n, 1, "결근 기록이 그대로 있어야 한다");
    assert_eq!(status, "ACTIVE");
    assert_eq!(all_day, 1, "종일 결근 그대로");

    let v = s.stats();
    assert_eq!(v.summary.absent_teachers, 1, "결근한 사실은 남는다");
    assert_eq!(v.summary.absence_count, 1);
    assert_eq!(v.absences[0].count, 1);
    assert_eq!(v.absences[0].required, 5, "필요했던 칸 수도 남는다");
    assert_eq!(v.absences[0].not_required, 5);
    assert_eq!(v.summary.unassigned, 0, "처리할 일은 남지 않는다");
}

// ============================================================
//  Case H — 이미 배정된 칸
// ============================================================

#[test]
fn case_h_이미_배정된_칸은_먼저_취소해야_한다() {
    let s = School::new();
    s.absent_all_day();
    let saved = s.assign(3, s.sub_a).unwrap();

    let e = s.waive(3, crate::repo::waiver::REASON_SPECIAL_CHANGED, None).unwrap_err();
    assert!(
        e.user_message.contains("먼저 배정을 취소"),
        "{}",
        e.user_message
    );
    assert!(e.user_message.contains("박전담"), "{}", e.user_message);

    // 배정 기록을 몰래 지우지 않는다
    let still: i64 = s
        .conn
        .query_row(
            "SELECT COUNT(*) FROM substitutions WHERE id = ?1 AND status = 'ASSIGNED'",
            params![saved.id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(still, 1);
    assert_eq!(s.stats().summary.not_required, 0);

    // 취소한 뒤에는 된다
    ra::cancel(&s.conn, saved.id, Some("전담시간 변경")).unwrap();
    s.waive(3, crate::repo::waiver::REASON_SPECIAL_CHANGED, None).unwrap();

    let v = s.stats();
    assert_eq!(v.summary.not_required, 1);
    assert_eq!(v.summary.cancelled, 1, "취소 기록도 그대로 남는다");
    assert_eq!(v.summary.covered, 0);
}

// ============================================================
//  Case I · J — 오래된 화면에서 저장하려 할 때
// ============================================================

#[test]
fn case_i_보결_불필요인_칸에는_저장이_막힌다() {
    let s = School::new();
    s.absent_all_day();

    // 화면 A: 3교시 후보를 미리 조회해 둔다
    let before = s.plan();
    let picked = s.slot(&before, 3).candidates[0].teacher_id;

    // 다른 곳에서 3교시를 보결 불필요로 처리
    s.waive(3, crate::repo::waiver::REASON_SPECIAL_CHANGED, None).unwrap();

    // 화면 A 에서 그대로 저장
    let e = s.assign(3, picked).unwrap_err();
    assert_eq!(e.code, "NOT_REQUIRED", "{}", e.user_message);
    assert!(
        e.user_message.contains("보결이 필요하지 않은 시간"),
        "{}",
        e.user_message
    );
    assert!(e.user_message.contains("되돌리기"), "{}", e.user_message);

    // 다른 교시는 그대로 저장된다
    assert!(s.assign(4, picked).is_ok());
}

#[test]
fn case_j_되돌린_뒤에는_다시_저장된다() {
    let s = School::new();
    s.absent_all_day();
    let id = s.waive(3, crate::repo::waiver::REASON_SPECIAL_CHANGED, None).unwrap();
    assert!(s.assign(3, s.sub_a).is_err());

    crate::repo::waiver::revoke(&s.conn, id).unwrap();

    let plan = s.plan();
    let slot = s.slot(&plan, 3);
    assert!(!slot.candidates.is_empty());
    let saved = s.assign(3, slot.candidates[0].teacher_id).unwrap();
    assert_eq!(saved.slot_label, "3교시");

    let v = s.stats();
    assert_eq!(v.summary.covered, 1);
    assert_eq!(v.summary.unassigned, 4);
    assert_eq!(v.summary.not_required, 0);
}

/// 보결 조회 화면에서도 같은 사실을 본다.
#[test]
fn case_j_단일_조회에서도_보결_불필요가_보인다() {
    use crate::domain::find::{find_candidates, FindRequest};

    let s = School::new();
    s.absent_all_day();
    s.waive(3, crate::repo::waiver::REASON_SPECIAL_CHANGED, None).unwrap();

    let snap = crate::repo::find::snapshot(&s.conn, MON).unwrap();
    let counts = crate::repo::find::counts(&s.conn, MON).unwrap();
    let r = find_candidates(
        &snap,
        &FindRequest {
            class_id: s.class,
            slot_type: "PERIOD".into(),
            period_no: Some(3),
            absent_teacher_id: Some(s.absent),
        },
        &counts,
    )
    .unwrap();

    let n = r.notice.expect("알림이 있어야 한다");
    assert_eq!(n.kind, "WAIVED");
    assert!(n.title.contains("보결 불필요"), "{}", n.title);

    // 4교시는 평소대로다
    let r4 = find_candidates(
        &snap,
        &FindRequest {
            class_id: s.class,
            slot_type: "PERIOD".into(),
            period_no: Some(4),
            absent_teacher_id: Some(s.absent),
        },
        &counts,
    )
    .unwrap();
    assert!(r4.notice.is_none() || r4.notice.as_ref().unwrap().kind != "WAIVED");
}

// ============================================================
//  Case K — 엑셀
// ============================================================

#[test]
fn case_k_엑셀_숫자가_늘어나지_않는다() {
    let s = School::new();
    s.absent_all_day();
    s.assign(1, s.sub_a).unwrap();
    s.waive(3, crate::repo::waiver::REASON_SPECIAL_CHANGED, None).unwrap();

    // 배정 내역 — 실제 배정 1건만
    let hist = ra::history(
        &s.conn,
        &ra::HistoryFilter {
            from: Some(MON.into()),
            to: Some(MON.into()),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(hist.rows.len(), 1, "보결 불필요는 배정 내역이 아니다");

    // 현황 — 미배정 3, 보결 불필요 1
    let v = s.stats();
    assert_eq!(v.summary.assigned, 1);
    assert_eq!(v.summary.unassigned, 3);
    assert_eq!(v.summary.not_required, 1);
    assert_eq!(v.open_slots.len(), 3);
}

// ============================================================
//  Case L — 기존 전담 자동 제외는 그대로
// ============================================================

#[test]
fn case_l_전담이_들어오는_교시는_여전히_자동으로_빠진다() {
    let s = School::new();
    // 5-가람 3교시에 전담 수업이 들어 있다 (결근 등록 전부터)
    s.conn
        .execute(
            "INSERT INTO lessons(term_id, class_id, teacher_id, day_of_week, period_no, lesson_type, replaces_homeroom)
             VALUES (1, 1, ?1, 1, 3, 'SPECIAL', 1)",
            params![s.sub_a],
        )
        .unwrap();
    s.absent_all_day();

    let plan = s.plan();
    assert_eq!(plan.slots.len(), 4, "3교시는 처음부터 보결 대상이 아니다");
    assert!(plan.slots.iter().all(|x| x.period_no != Some(3)));

    let v = s.stats();
    assert_eq!(v.summary.required, 4);
    assert_eq!(v.summary.unassigned, 4);
    assert_eq!(v.summary.not_required, 0, "자동 제외는 보결 불필요가 아니다");

    // 그 칸은 보결 불필요로 처리할 수도 없다 (애초에 필요하지 않다)
    assert!(s.waive(3, crate::repo::waiver::REASON_SPECIAL_CHANGED, None).is_err());
}

// ============================================================
//  도움 함수
// ============================================================

fn repo_find_counts(
    conn: &Connection,
) -> std::collections::HashMap<i64, crate::domain::find::SubCounts> {
    crate::repo::find::counts(conn, MON).unwrap()
}
