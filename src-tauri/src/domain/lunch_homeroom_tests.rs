//! 점심 보결에 담임을 부를 것인가 — 요청서의 Case A~L.
//!
//! ## 무엇을 확인하는가
//!
//! 학교마다 점심 보결 운영이 다르다. 담임을 아예 부르지 않는 학교가 있고,
//! 점심시간이 다른 학년끼리 서로 맡아 주는 학교가 있다. 그래서 설정으로
//! 두되, **켜도 무조건 허용하지는 않는다** — 자기 학년 점심시간과 실제로
//! 겹치면 그 사람은 그때 급식 지도 중이다.
//!
//! ## 학년 묶음을 쓰지 않는다
//!
//! '저학년 / 고학년' 두 묶음을 가정하면 틀린다. 실제 우리 학교 자료에는
//! 점심 패턴이 **셋**이었고 서로 조금씩 겹쳤다.
//!
//! ```text
//!  3·4학년   11:25~12:15
//!  1·2학년   12:10~13:00      <- 3·4학년과 5분 겹친다
//!  5·6학년   12:55~13:45      <- 1·2학년과 5분 겹친다
//! ```
//!
//! '다른 점심 그룹'이라는 이유만으로 허용하면 급식 지도 중인 선생님을 다른
//! 반에 보내게 된다. 그래서 시각으로만 판단한다 (Case L).

use std::collections::HashMap;

use super::find::*;
use super::schedule::*;
use super::time::Interval;

fn hm(h: i32, m: i32) -> i32 {
    h * 60 + m
}

const DAY: i32 = 1; // 월요일
const SPECIAL_ID: i64 = 100;
const OTHER_ID: i64 = 200;
const ROLE_OTHER: &str = "OTHER";

/// Case A~F 의 시정표. 1·2·3학년은 먼저, 4·5·6학년은 나중에 먹는다.
fn two_patterns(grade: i32) -> Interval {
    if grade <= 3 {
        Interval::new(hm(12, 10), hm(13, 10))
    } else {
        Interval::new(hm(13, 10), hm(14, 0))
    }
}

/// Case L 의 시정표. 실제 학교 자료 그대로 **세 가지**이고 서로 겹친다.
fn three_patterns(grade: i32) -> Interval {
    match grade {
        3 | 4 => Interval::new(hm(11, 25), hm(12, 15)),
        1 | 2 => Interval::new(hm(12, 10), hm(13, 0)),
        _ => Interval::new(hm(12, 55), hm(13, 45)),
    }
}

fn slot(grade: i32, ty: &str, p: Option<i32>, label: &str, a: i32, b: i32) -> SlotInfo {
    SlotInfo {
        grade,
        day_of_week: DAY,
        slot_type: ty.into(),
        period_no: p,
        label: label.into(),
        start_min: a,
        end_min: b,
    }
}

struct World {
    snap: DaySnapshot,
}

impl World {
    /// 1~6학년 × 1반. 담임 6명(교사 번호 = 학년) + 전담 1명 + 기타 1명.
    ///
    /// 교시는 **오전만** 둔다(1~4교시 9:00~11:20). 어느 학년 점심과도 겹치지
    /// 않으므로, 담임이 빠진다면 그 이유는 이번에 넣은 규칙뿐이다.
    fn new(lunch_of: fn(i32) -> Interval) -> Self {
        let mut slots = Vec::new();
        let mut classes = Vec::new();
        let mut teachers = Vec::new();

        for g in 1..=6 {
            slots.push(slot(g, SLOT_PERIOD, Some(1), "1교시", hm(9, 0), hm(9, 40)));
            slots.push(slot(g, SLOT_PERIOD, Some(2), "2교시", hm(9, 50), hm(10, 30)));
            slots.push(slot(g, SLOT_PERIOD, Some(3), "3교시", hm(10, 40), hm(11, 20)));
            let l = lunch_of(g);
            slots.push(slot(g, SLOT_LUNCH, None, "점심", l.start, l.end));

            teachers.push(TeacherInfo {
                id: g as i64,
                name: format!("{g}-1 담임"),
                role_code: ROLE_HOMEROOM.into(),
                role_label: "담임".into(),
                memo: None,
                is_substitutable: true,
                active: true,
                subjects: vec![],
            });
            classes.push(ClassInfo {
                id: g as i64,
                grade: g,
                class_no: 1,
                label: format!("{g}-1"),
                full_label: format!("{g}학년 1반"),
                homeroom_teacher_id: Some(g as i64),
            });
        }

        teachers.push(TeacherInfo {
            id: SPECIAL_ID,
            name: "김전담".into(),
            role_code: ROLE_SPECIAL.into(),
            role_label: "전담".into(),
            memo: None,
            is_substitutable: true,
            active: true,
            subjects: vec!["체육".into()],
        });
        teachers.push(TeacherInfo {
            id: OTHER_ID,
            name: "박기타".into(),
            role_code: ROLE_OTHER.into(),
            role_label: "기타".into(),
            memo: Some("교무".into()),
            is_substitutable: true,
            active: true,
            subjects: vec![],
        });

        Self {
            snap: DaySnapshot {
                date: "2026-09-14".into(),
                day_of_week: DAY,
                teachers,
                classes,
                slots,
                lessons: vec![],
                duties: vec![],
                absences: vec![],
                assigned: vec![],
                settings: EngineSettings::default(),
                meal_default: None,
                meal_overrides: HashMap::new(),
            },
        }
    }

    /// 기본 시정표(두 가지 점심)로 만든다.
    fn two() -> Self {
        Self::new(two_patterns)
    }

    /// 점심 보결에 담임을 넣는 설정을 켠다.
    fn cross_on(mut self) -> Self {
        self.snap.settings.include_cross_lunch_homeroom = true;
        self
    }

    /// 자기 학년 점심 급식 지도 설정을 끈다.
    ///
    /// 켜 두면 담임의 자기 점심시간이 이미 '바쁜 시간'이라 그쪽에서 먼저
    /// 걸린다. 이번에 넣은 겹침 판정 자체를 보려면 꺼야 한다.
    fn no_lunch_duty(mut self) -> Self {
        self.snap.settings.exclude_homeroom_on_own_lunch = false;
        self
    }

    /// 교시 하나를 더 놓는다 (전담 수업 시각을 만들 때).
    fn period(mut self, grade: i32, no: i32, a: i32, b: i32) -> Self {
        self.snap
            .slots
            .push(slot(grade, SLOT_PERIOD, Some(no), &format!("{no}교시"), a, b));
        self
    }

    /// 그 학년 1반의 그 교시를 이 교사가 담임 대신 맡는다.
    fn taught_by(mut self, teacher_id: i64, grade: i32, period: i32) -> Self {
        self.snap.lessons.push(LessonInfo {
            teacher_id,
            class_id: grade as i64,
            period_no: period,
            replaces_homeroom: true,
            subject_name: Some("체육".into()),
        });
        self
    }

    /// 전담의 식사시간을 직접 지정한다.
    fn meal(mut self, iv: Interval) -> Self {
        self.snap.meal_overrides.insert(SPECIAL_ID, iv);
        self
    }

    /// 그 학년 1반의 점심 보결 후보를 찾는다. (그 반 담임이 결근한 상황)
    fn lunch(&self, grade: i32) -> FindResult {
        self.find(grade, SLOT_LUNCH, None)
    }

    fn find(&self, grade: i32, slot_type: &str, period: Option<i32>) -> FindResult {
        find_candidates(
            &self.snap,
            &FindRequest {
                class_id: grade as i64,
                slot_type: slot_type.into(),
                period_no: period,
                absent_teacher_id: Some(grade as i64),
            },
            &HashMap::new(),
        )
        .expect("칸을 찾을 수 있어야 한다")
    }
}

/// 그 교사가 후보인가 / 빠졌다면 그 이유와 설명.
fn verdict(r: &FindResult, teacher_id: i64) -> (bool, String, Option<String>) {
    if let Some(c) = r.eligible.iter().find(|c| c.teacher_id == teacher_id) {
        return (true, c.reason_code.clone(), c.detail.clone());
    }
    let c = r
        .excluded
        .iter()
        .find(|c| c.teacher_id == teacher_id)
        .expect("어느 쪽에든 있어야 한다");
    (false, c.reason_code.clone(), c.detail.clone())
}

// ============================================================
//  Case A — 꺼져 있으면 자기 학년 점심 보결에 못 들어간다
// ============================================================

#[test]
fn case_a_설정이_꺼져_있으면_자기_학년_점심_보결에서_빠진다() {
    // 1학년 담임, 대상은 1학년 점심 12:10~13:10
    let w = World::two();
    let r = w.lunch(1);
    assert_eq!(r.slot.start_min, hm(12, 10));
    assert_eq!(r.slot.end_min, hm(13, 10));

    // 2학년 담임도 같은 점심시간이다 (1학년 담임 본인은 결근 당사자라 제외)
    let (ok, code, _) = verdict(&r, 2);
    assert!(!ok, "꺼져 있으면 담임은 점심 보결 후보가 아니다");
    // 급식 지도 설정이 켜져 있으면 그쪽이 더 구체적인 이유이므로 먼저 걸린다
    assert_eq!(code, EXCLUDED_LUNCH_DUTY);

    // 급식 지도 설정을 꺼도 결과는 같다 — 이번 규칙이 받아 낸다
    let w = World::two().no_lunch_duty();
    let (ok, code, detail) = verdict(&w.lunch(1), 2);
    assert!(!ok);
    assert_eq!(code, EXCLUDED_HOMEROOM_LUNCH_POLICY);
    assert_eq!(
        status_label(&code),
        "담임교사는 점심 보결 대상에서 제외됨"
    );
    assert!(detail.unwrap().contains("설정"));
}

// ============================================================
//  Case B — 꺼져 있으면 점심시간이 달라도 빠진다
// ============================================================

#[test]
fn case_b_설정이_꺼져_있으면_점심시간이_달라도_빠진다() {
    let w = World::two();
    // 4학년 담임의 점심은 13:10~14:00 — 대상 12:10~13:10 과 겹치지 않는다
    assert_eq!(
        homeroom_lunch_intervals(&w.snap, 4),
        vec![Interval::new(hm(13, 10), hm(14, 0))]
    );

    let (ok, code, _) = verdict(&w.lunch(1), 4);
    assert!(!ok, "꺼져 있으면 시간이 비어도 부르지 않는다");
    assert_eq!(code, EXCLUDED_HOMEROOM_LUNCH_POLICY);
}

// ============================================================
//  Case C — 켜도 자기 점심과 같은 시간이면 빠진다
// ============================================================

#[test]
fn case_c_켜도_자기_점심과_같은_시간이면_빠진다() {
    // 급식 지도 설정까지 꺼서 이번 규칙만 남긴다
    let w = World::two().cross_on().no_lunch_duty();

    // 2학년 담임의 점심은 1학년과 같은 12:10~13:10
    let (ok, code, detail) = verdict(&w.lunch(1), 2);
    assert!(!ok, "자기가 급식 지도 중인 시간이다");
    assert_eq!(code, EXCLUDED_HOMEROOM_OWN_LUNCH);
    assert_eq!(status_label(&code), "자기 학년 점심시간과 겹침");
    assert_eq!(detail.as_deref(), Some("자기 학년 점심 12:10~13:10"));
}

// ============================================================
//  Case D — 켜고 경계만 맞닿으면 후보가 된다
// ============================================================

#[test]
fn case_d_켜고_경계만_맞닿으면_후보가_된다() {
    let w = World::two().cross_on();

    // 대상 12:10~13:10, 4학년 담임 자기 점심 13:10~14:00
    let r = w.lunch(1);
    let (ok, code, _) = verdict(&r, 4);
    assert!(
        ok,
        "13:10 == 13:10 은 겹침이 아니다 (지금 {code})"
    );

    // 5·6학년 담임도 같은 점심이므로 함께 후보가 된다
    for g in [5, 6] {
        assert!(verdict(&r, g).0, "{g}학년 담임도 후보여야 한다");
    }
    // 같은 점심시간인 2·3학년 담임은 여전히 빠진다
    for g in [2, 3] {
        assert!(!verdict(&r, g).0, "{g}학년 담임은 급식 지도 중이다");
    }
}

// ============================================================
//  Case E — 5분만 겹쳐도 빠진다
// ============================================================

#[test]
fn case_e_오분만_겹쳐도_빠진다() {
    // 세 가지 점심 패턴. 대상 1학년 12:10~13:00
    let w = World::new(three_patterns).cross_on().no_lunch_duty();
    let r = w.lunch(1);
    assert_eq!(r.slot.start_min, hm(12, 10));
    assert_eq!(r.slot.end_min, hm(13, 0));

    // 5학년 담임 자기 점심 12:55~13:45 — 12:55~13:00 다섯 분이 겹친다
    let (ok, code, detail) = verdict(&r, 5);
    assert!(!ok, "5분이라도 겹치면 후보가 아니다");
    assert_eq!(code, EXCLUDED_HOMEROOM_OWN_LUNCH);
    assert_eq!(detail.as_deref(), Some("자기 학년 점심 12:55~13:45"));
}

// ============================================================
//  Case F — 반대 방향 교차도 똑같이 된다
// ============================================================

#[test]
fn case_f_반대_방향_교차도_후보가_된다() {
    let w = World::two().cross_on();

    // 대상 4학년 점심 13:10~14:00, 1·2·3학년 담임 자기 점심 12:10~13:10
    let r = w.lunch(4);
    assert_eq!(r.slot.start_min, hm(13, 10));
    assert_eq!(r.slot.end_min, hm(14, 0));

    for g in [1, 2, 3] {
        let (ok, code, _) = verdict(&r, g);
        assert!(ok, "{g}학년 담임이 후보여야 한다 (지금 {code})");
    }
    // 같은 점심시간인 5·6학년 담임은 빠진다
    for g in [5, 6] {
        assert!(!verdict(&r, g).0, "{g}학년 담임은 급식 지도 중이다");
    }
}

// ============================================================
//  Case G — 자기 학년 점심을 모르면 부르지 않는다
// ============================================================

#[test]
fn case_g_자기_학년_점심을_모르면_부르지_않는다() {
    let mut w = World::two().cross_on();
    // 4학년 시정표에서 점심을 지운다 (자료가 덜 들어간 상태)
    w.snap
        .slots
        .retain(|s| !(s.grade == 4 && s.slot_type == SLOT_LUNCH));
    assert!(homeroom_lunch_intervals(&w.snap, 4).is_empty());

    let (ok, code, detail) = verdict(&w.lunch(1), 4);
    assert!(!ok, "모르면서 후보에 넣으면 안 된다");
    assert_eq!(code, EXCLUDED_HOMEROOM_LUNCH_UNKNOWN);
    assert_eq!(
        status_label(&code),
        "자기 학년 점심시간을 확인할 수 없음"
    );
    assert!(detail.unwrap().contains("알 수 없"));

    // 5·6학년 담임은 점심시간을 알 수 있으므로 그대로 후보다
    assert!(verdict(&w.lunch(1), 5).0);
}

// ============================================================
//  Case H — 전담교사는 식사시간에도 점심 보결을 맡는다 (v0.1.8 유지)
// ============================================================

#[test]
fn case_h_전담은_식사시간과_같아도_점심_보결_후보다() {
    // 김전담의 식사시간을 13:10~14:00 으로 직접 지정하고, 같은 시간의
    // 4학년 점심 보결을 찾는다.
    let w = World::two().meal(Interval::new(hm(13, 10), hm(14, 0)));

    let r = w.lunch(4);
    assert_eq!(r.slot.start_min, hm(13, 10));
    assert_eq!(r.slot.end_min, hm(14, 0));

    let (ok, code, _) = verdict(&r, SPECIAL_ID);
    assert!(ok, "점심 보결은 식사시간 때문에 빠지지 않는다 (지금 {code})");

    // 설정을 켜도 전담에게는 아무 영향이 없다
    let w = World::two()
        .cross_on()
        .meal(Interval::new(hm(13, 10), hm(14, 0)));
    assert!(verdict(&w.lunch(4), SPECIAL_ID).0);
}

// ============================================================
//  Case I — 전담이라도 실제 수업 중이면 빠진다
// ============================================================

#[test]
fn case_i_전담이_실제_수업_중이면_빠진다() {
    // 4학년 5교시 12:20~13:00 을 김전담이 맡는다. 대상은 1학년 점심 12:10~13:10.
    let w = World::two()
        .period(4, 5, hm(12, 20), hm(13, 0))
        .taught_by(SPECIAL_ID, 4, 5)
        .meal(Interval::new(hm(13, 10), hm(14, 0)));

    let (ok, code, detail) = verdict(&w.lunch(1), SPECIAL_ID);
    assert!(!ok, "수업 중이므로 빠져야 한다");
    assert_eq!(
        code, EXCLUDED_SPECIAL_LESSON,
        "식사시간이 아니라 실제 수업 때문에 빠져야 한다"
    );
    assert!(detail.unwrap().contains("12:20~13:00"));
}

// ============================================================
//  Case J — 기타 교사는 예전 그대로
// ============================================================

#[test]
fn case_j_기타_교사는_설정과_무관하다() {
    for on in [false, true] {
        let w = if on { World::two().cross_on() } else { World::two() };
        let (ok, code, _) = verdict(&w.lunch(1), OTHER_ID);
        assert!(ok, "기타 선생님은 점심 보결 후보다 (설정 {on}, 지금 {code})");
    }
}

// ============================================================
//  Case K — 일반 수업 보결은 달라지지 않는다
// ============================================================

#[test]
fn case_k_일반_수업_보결은_설정에_영향받지_않는다() {
    for on in [false, true] {
        // 4학년 1교시에 전담이 들어와 4학년 담임이 비는 상황.
        // 대상은 1학년 1교시 9:00~9:40.
        let w = World::two().taught_by(SPECIAL_ID, 4, 1);
        let w = if on { w.cross_on() } else { w };

        let r = w.find(1, SLOT_PERIOD, Some(1));
        assert_eq!(r.slot.start_min, hm(9, 0));

        let (ok, code, _) = verdict(&r, 4);
        assert!(
            ok,
            "일반 보결의 담임 판정은 그대로여야 한다 (설정 {on}, 지금 {code})"
        );
        assert_eq!(code, ELIGIBLE_HOMEROOM_FREE);
    }
}

// ============================================================
//  Case L — 실제 학교의 점심 패턴 세 가지
// ============================================================

#[test]
fn case_l_세_가지_점심_패턴은_시각으로만_갈린다() {
    let w = World::new(three_patterns).cross_on().no_lunch_duty();

    // 1·2학년 점심 12:10~13:00 에 대한 각 학년 담임의 판정
    let r = w.lunch(1);
    assert_eq!((r.slot.start_min, r.slot.end_min), (hm(12, 10), hm(13, 0)));

    // 3·4학년 11:25~12:15 → 12:10~12:15 겹침 → 제외
    // 5·6학년 12:55~13:45 → 12:55~13:00 겹침 → 제외
    // 2학년      12:10~13:00 → 같은 시간 → 제외
    // 결국 이 점심 보결에는 어느 학년 담임도 들어갈 수 없다.
    for g in 2..=6 {
        let (ok, code, _) = verdict(&r, g);
        assert!(
            !ok,
            "{g}학년 담임은 5분이라도 겹치므로 빠져야 한다 (지금 {code})"
        );
        assert_eq!(code, EXCLUDED_HOMEROOM_OWN_LUNCH, "{g}학년");
    }

    // 5·6학년 점심 12:55~13:45 라면 3·4학년 담임은 겹치지 않는다
    let r = w.lunch(5);
    assert_eq!((r.slot.start_min, r.slot.end_min), (hm(12, 55), hm(13, 45)));
    for g in [3, 4] {
        let (ok, code, _) = verdict(&r, g);
        assert!(ok, "{g}학년 담임(11:25~12:15)은 겹치지 않는다 (지금 {code})");
    }
    for g in [1, 2, 6] {
        let (ok, _, _) = verdict(&r, g);
        assert!(!ok, "{g}학년 담임은 겹친다");
    }

    // 3·4학년 점심 11:25~12:15 라면 5·6학년 담임만 들어갈 수 있다
    let r = w.lunch(3);
    assert_eq!((r.slot.start_min, r.slot.end_min), (hm(11, 25), hm(12, 15)));
    for g in [5, 6] {
        assert!(verdict(&r, g).0, "{g}학년 담임(12:55~13:45)은 겹치지 않는다");
    }
    for g in [1, 2, 4] {
        assert!(!verdict(&r, g).0, "{g}학년 담임은 겹친다");
    }
}

// ============================================================
//  설정을 바꾸면 곧바로 반영된다
// ============================================================

#[test]
fn 설정을_바꾸면_다음_조회부터_곧바로_반영된다() {
    let mut w = World::two();

    // 꺼짐 → 4학년 담임 제외
    assert!(!verdict(&w.lunch(1), 4).0);

    // 켜짐 → 겹치지 않으므로 후보
    w.snap.settings.include_cross_lunch_homeroom = true;
    assert!(verdict(&w.lunch(1), 4).0, "켠 즉시 후보가 되어야 한다");

    // 다시 꺼짐 → 즉시 제외
    w.snap.settings.include_cross_lunch_homeroom = false;
    let (ok, code, _) = verdict(&w.lunch(1), 4);
    assert!(!ok, "끈 즉시 다시 빠져야 한다");
    assert_eq!(code, EXCLUDED_HOMEROOM_LUNCH_POLICY);
}

// ============================================================
//  저장할 때 다시 검증한다
// ============================================================

#[test]
fn 화면이_오래되었어도_저장할_때_다시_막는다() {
    use super::assign::{verify, AssignError, AssignPlan};

    let mut w = World::two().cross_on();
    let plan = AssignPlan {
        class_id: 1,
        slot_type: SLOT_LUNCH.into(),
        period_no: None,
        sub_teacher_id: 4, // 4학년 담임
    };

    // 켜져 있을 때는 저장된다
    let row = verify(&w.snap, &HashMap::new(), &[], Some(1), &plan)
        .expect("켜져 있으면 배정할 수 있어야 한다");
    assert_eq!(row.sub_teacher_id, 4);

    // 화면은 그대로 둔 채 설정만 끄면, 저장 단계가 막는다
    w.snap.settings.include_cross_lunch_homeroom = false;
    match verify(&w.snap, &HashMap::new(), &[], Some(1), &plan) {
        Err(AssignError::NotEligible { name, status, .. }) => {
            assert_eq!(name, "4-1 담임");
            assert_eq!(status, "담임교사는 점심 보결 대상에서 제외됨");
        }
        other => panic!("막아야 한다: {other:?}"),
    }

    // 반대로 자기 점심과 겹치는 담임은 켜 두어도 저장되지 않는다
    let w = World::two().cross_on().no_lunch_duty();
    let plan = AssignPlan {
        sub_teacher_id: 2, // 1학년과 같은 점심시간
        ..plan
    };
    match verify(&w.snap, &HashMap::new(), &[], Some(1), &plan) {
        Err(AssignError::NotEligible { status, .. }) => {
            assert_eq!(status, "자기 학년 점심시간과 겹침");
        }
        other => panic!("막아야 한다: {other:?}"),
    }
}

// ============================================================
//  자기 학년 점심시간 뽑기
// ============================================================

#[test]
fn 담임이_맡은_학급의_점심시간을_시정표에서_뽑는다() {
    let w = World::new(three_patterns);

    assert_eq!(
        homeroom_lunch_intervals(&w.snap, 3),
        vec![Interval::new(hm(11, 25), hm(12, 15))]
    );
    assert_eq!(
        homeroom_lunch_intervals(&w.snap, 6),
        vec![Interval::new(hm(12, 55), hm(13, 45))]
    );
    // 담임이 아닌 사람에게는 없다
    assert!(homeroom_lunch_intervals(&w.snap, SPECIAL_ID).is_empty());
}

#[test]
fn 두_학년을_맡은_담임은_점심시간이_둘_다_나온다() {
    let mut w = World::new(three_patterns);
    // 3학년 1반 담임이 6학년 1반도 맡는 상황
    if let Some(c) = w.snap.classes.iter_mut().find(|c| c.grade == 6) {
        c.homeroom_teacher_id = Some(3);
    }

    assert_eq!(
        homeroom_lunch_intervals(&w.snap, 3),
        vec![
            Interval::new(hm(11, 25), hm(12, 15)),
            Interval::new(hm(12, 55), hm(13, 45)),
        ]
    );

    // 1학년 점심 12:10~13:00 은 두 시간 모두와 겹치므로 빠진다
    w.snap.settings.include_cross_lunch_homeroom = true;
    w.snap.settings.exclude_homeroom_on_own_lunch = false;
    let (ok, code, _) = verdict(&w.lunch(1), 3);
    assert!(!ok);
    assert_eq!(code, EXCLUDED_HOMEROOM_OWN_LUNCH);
}

#[test]
fn 같은_시각을_맡은_학급이_여럿이면_한_번만_나온다() {
    let mut w = World::two();
    // 1학년 1반 담임이 2학년 1반도 맡는다 (둘 다 12:10~13:10)
    if let Some(c) = w.snap.classes.iter_mut().find(|c| c.grade == 2) {
        c.homeroom_teacher_id = Some(1);
    }
    assert_eq!(
        homeroom_lunch_intervals(&w.snap, 1),
        vec![Interval::new(hm(12, 10), hm(13, 10))]
    );
}
