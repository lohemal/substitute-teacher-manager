//! 전담교사 식사시간이 **후보 판정**에 어떻게 걸리는지 — 요청서의 Case A~H.
//!
//! 우리 학교 시정표를 그대로 옮겼다.
//!
//! ```text
//!          저학년(1·2·3)                고학년(4·5·6)
//!  4교시   11:30~12:10                 11:30~12:10
//!  점심    12:10~13:10                 13:10~14:00
//!  5교시   13:10~13:50                 12:20~13:00
//! ```
//!
//! 고학년은 5교시를 하고 나서 늦게 먹고, 저학년은 먼저 먹고 5교시를 한다.
//! 그래서 전담교사가 그 날 어느 학년에 들어갔느냐로 식사시간이 갈린다.

use std::collections::HashMap;

use super::find::*;
use super::meal::{Meal, MealSource};
use super::schedule::*;
use super::time::Interval;

fn hm(h: i32, m: i32) -> i32 {
    h * 60 + m
}

const HIGH: [i32; 3] = [4, 5, 6];

/// 저학년 점심 12:10~13:10
fn lunch_low() -> Interval {
    Interval::new(hm(12, 10), hm(13, 10))
}
/// 고학년 점심 13:10~14:00
fn lunch_high() -> Interval {
    Interval::new(hm(13, 10), hm(14, 0))
}

fn slot(grade: i32, day: i32, ty: &str, p: Option<i32>, label: &str, a: i32, b: i32) -> SlotInfo {
    SlotInfo {
        grade,
        day_of_week: day,
        slot_type: ty.into(),
        period_no: p,
        label: label.into(),
        start_min: a,
        end_min: b,
    }
}

/// 한 학년치 시정표. `late_lunch` 면 5교시를 먼저 하고 나중에 먹는다.
fn slots_for(grade: i32, day: i32, late_lunch: bool, lunch: Interval) -> Vec<SlotInfo> {
    let mut v = vec![
        slot(grade, day, SLOT_PERIOD, Some(1), "1교시", hm(9, 0), hm(9, 40)),
        slot(grade, day, SLOT_PERIOD, Some(2), "2교시", hm(9, 50), hm(10, 30)),
        slot(grade, day, SLOT_PERIOD, Some(3), "3교시", hm(10, 40), hm(11, 20)),
        slot(grade, day, SLOT_PERIOD, Some(4), "4교시", hm(11, 30), hm(12, 10)),
    ];
    if late_lunch {
        // 고학년: 5교시 12:20~13:00 → 점심 13:10~14:00
        v.push(slot(grade, day, SLOT_PERIOD, Some(5), "5교시", hm(12, 20), hm(13, 0)));
        v.push(slot(grade, day, SLOT_LUNCH, None, "점심", lunch.start, lunch.end));
    } else {
        // 저학년: 점심 12:10~13:10 → 5교시 13:10~13:50
        v.push(slot(grade, day, SLOT_LUNCH, None, "점심", lunch.start, lunch.end));
        v.push(slot(grade, day, SLOT_PERIOD, Some(5), "5교시", hm(13, 10), hm(13, 50)));
    }
    v
}

const DAY: i32 = 1; // 월요일
const SPECIAL_ID: i64 = 100;

struct World {
    snap: DaySnapshot,
}

impl World {
    /// 1~6학년 × 1반 + 담임 6명 + 전담 1명(김전담).
    fn new() -> Self {
        Self::with_high_lunch(lunch_high())
    }

    /// Case H 를 위해 고학년 점심 시각만 바꿀 수 있게 열어 둔다.
    fn with_high_lunch(high: Interval) -> Self {
        let mut slots = Vec::new();
        let mut classes = Vec::new();
        let mut teachers = Vec::new();

        for g in 1..=6 {
            let late = HIGH.contains(&g);
            slots.extend(slots_for(g, DAY, late, if late { high } else { lunch_low() }));
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

    /// 김전담에게 그 학년 1반의 그 교시 수업을 준다.
    fn teach(mut self, grade: i32, period: i32) -> Self {
        self.snap.lessons.push(LessonInfo {
            teacher_id: SPECIAL_ID,
            class_id: grade as i64,
            period_no: period,
            replaces_homeroom: true,
            subject_name: Some("체육".into()),
        });
        self
    }

    fn school_default(mut self, iv: Interval) -> Self {
        self.snap.meal_default = Some(iv);
        self
    }

    fn manual(mut self, iv: Interval) -> Self {
        self.snap.meal_overrides.insert(SPECIAL_ID, iv);
        self
    }

    fn meal(&self) -> Meal {
        resolve_meal(&self.snap, SPECIAL_ID)
    }

    /// 그 학년 1반의 그 교시(또는 점심) 보결 후보를 찾는다.
    fn find(&self, grade: i32, slot_type: &str, period: Option<i32>) -> FindResult {
        find_candidates(
            &self.snap,
            &FindRequest {
                class_id: grade as i64,
                slot_type: slot_type.into(),
                period_no: period,
                absent_teacher_id: Some(grade as i64), // 그 반 담임이 결근
            },
            &HashMap::new(),
        )
        .expect("칸을 찾을 수 있어야 한다")
    }

    /// 김전담이 후보인가 / 빠졌다면 그 이유
    fn verdict(&self, r: &FindResult) -> (bool, String, Option<String>) {
        if let Some(c) = r.eligible.iter().find(|c| c.teacher_id == SPECIAL_ID) {
            return (true, c.reason_code.clone(), c.detail.clone());
        }
        let c = r
            .excluded
            .iter()
            .find(|c| c.teacher_id == SPECIAL_ID)
            .expect("김전담이 어느 쪽에든 있어야 한다");
        (false, c.reason_code.clone(), c.detail.clone())
    }
}

// ============================================================
//  Case A — 고학년 5교시를 하면 늦게 먹는다
// ============================================================

#[test]
fn case_a_고학년_5교시_수업이면_늦은_식사시간이_되고_저학년_5교시_보결에서_빠진다() {
    // 김전담: 4학년 5교시 12:20~13:00
    let w = World::new().teach(4, 5);

    assert_eq!(
        w.meal(),
        Meal::Known {
            interval: lunch_high(),
            source: MealSource::Auto
        },
        "12:10~13:10 은 수업과 겹쳐 쓸 수 없다"
    );

    // 저학년 5교시 13:10~13:50 보결
    let r = w.find(1, SLOT_PERIOD, Some(5));
    assert_eq!(r.slot.start_min, hm(13, 10));
    assert_eq!(r.slot.end_min, hm(13, 50));

    let (ok, code, detail) = w.verdict(&r);
    assert!(!ok, "식사시간이라 후보가 되면 안 된다");
    assert_eq!(code, EXCLUDED_SPECIAL_MEAL);
    assert_eq!(detail.as_deref(), Some("식사시간 13:10~14:00"));
    assert_eq!(status_label(&code), "식사시간");
}

// ============================================================
//  Case B — 저학년 5교시를 하면 먼저 먹는다
// ============================================================

#[test]
fn case_b_저학년_5교시_수업이면_이른_식사시간이_되고_고학년_5교시_보결에서_빠진다() {
    // 김전담: 1학년 5교시 13:10~13:50
    let w = World::new().teach(1, 5);

    assert_eq!(
        w.meal(),
        Meal::Known {
            interval: lunch_low(),
            source: MealSource::Auto
        }
    );

    // 고학년 5교시 12:20~13:00 보결
    let r = w.find(4, SLOT_PERIOD, Some(5));
    assert_eq!(r.slot.start_min, hm(12, 20));

    let (ok, code, detail) = w.verdict(&r);
    assert!(!ok);
    assert_eq!(code, EXCLUDED_SPECIAL_MEAL);
    assert_eq!(detail.as_deref(), Some("식사시간 12:10~13:10"));
}

// ============================================================
//  Case C — 둘 다 가능하면 학교 기본 식사시간
// ============================================================

#[test]
fn case_c_4교시까지만_하면_학교_기본_식사시간을_쓴다() {
    let w = World::new().teach(4, 4).school_default(lunch_low());

    assert_eq!(
        w.meal(),
        Meal::Known {
            interval: lunch_low(),
            source: MealSource::SchoolDefault
        }
    );

    // 기본이 이른 점심이므로 고학년 5교시(12:20~13:00)는 막힌다
    let (ok, code, _) = w.verdict(&w.find(4, SLOT_PERIOD, Some(5)));
    assert!(!ok);
    assert_eq!(code, EXCLUDED_SPECIAL_MEAL);

    // 저학년 5교시(13:10~13:50)는 식사시간이 아니므로 후보가 된다
    let (ok, code, _) = w.verdict(&w.find(1, SLOT_PERIOD, Some(5)));
    assert!(ok, "{code}");
}

// ============================================================
//  Case D — 수동 지정이 기본값을 이긴다
// ============================================================

#[test]
fn case_d_수동_지정이_학교_기본_식사시간보다_우선한다() {
    let w = World::new()
        .teach(4, 4)
        .school_default(lunch_low())
        .manual(lunch_high());

    assert_eq!(
        w.meal(),
        Meal::Known {
            interval: lunch_high(),
            source: MealSource::Manual
        }
    );

    // 이제 막히는 쪽이 뒤바뀐다
    let (ok, _, _) = w.verdict(&w.find(4, SLOT_PERIOD, Some(5)));
    assert!(ok, "12:20~13:00 은 이제 식사시간이 아니다");

    let (ok, code, detail) = w.verdict(&w.find(1, SLOT_PERIOD, Some(5)));
    assert!(!ok);
    assert_eq!(code, EXCLUDED_SPECIAL_MEAL);
    assert_eq!(detail.as_deref(), Some("식사시간 13:10~14:00"));
}

// ============================================================
//  Case E — 식사시간과 겹치는 일반 보결은 빠진다
// ============================================================

#[test]
fn case_e_식사시간과_겹치는_일반_수업_보결에서_빠진다() {
    let w = World::new().manual(lunch_high());
    let (ok, code, detail) = w.verdict(&w.find(1, SLOT_PERIOD, Some(5))); // 13:10~13:50
    assert!(!ok);
    assert_eq!(code, EXCLUDED_SPECIAL_MEAL);
    assert_eq!(detail.as_deref(), Some("식사시간 13:10~14:00"));
}

// ============================================================
//  Case F — 점심 보결에는 식사시간을 적용하지 않는다 (가장 중요)
// ============================================================

#[test]
fn case_f_식사시간과_겹쳐도_점심_보결_후보가_된다() {
    // 김전담 식사시간 13:10~14:00, 대상은 고학년 점심 보결 13:10~14:00
    let w = World::new().manual(lunch_high());

    let r = w.find(4, SLOT_LUNCH, None);
    assert_eq!(r.slot.start_min, hm(13, 10));
    assert_eq!(r.slot.end_min, hm(14, 0));

    let (ok, code, _) = w.verdict(&r);
    assert!(
        ok,
        "전담교사는 자기 식사시간에도 점심 보결을 맡을 수 있다 (지금: {code})"
    );
    assert_ne!(code, EXCLUDED_SPECIAL_MEAL);
}

#[test]
fn case_f_점심_보결이어도_다른_이유로는_그대로_빠진다() {
    // 같은 시간에 수업이 있으면 점심 보결도 못 맡는다 — 기존 규칙 그대로
    let w = World::with_high_lunch(Interval::new(hm(12, 20), hm(13, 0)))
        .teach(5, 5) // 5학년 5교시 12:20~13:00
        .manual(lunch_low());

    let r = w.find(4, SLOT_LUNCH, None); // 고학년 점심 12:20~13:00
    let (ok, code, _) = w.verdict(&r);
    assert!(!ok);
    assert_eq!(code, EXCLUDED_SPECIAL_LESSON, "수업 중이라 빠진다");
}

// ============================================================
//  Case H — 경계는 겹침이 아니다
// ============================================================

#[test]
fn case_h_수업_종료와_식사_시작이_맞닿으면_겹치지_않는다() {
    // 고학년 점심을 13:00~14:00 으로 두면 5교시 12:20~13:00 과 맞닿는다
    let w = World::with_high_lunch(Interval::new(hm(13, 0), hm(14, 0))).teach(4, 5);

    assert_eq!(
        w.meal(),
        Meal::Known {
            interval: Interval::new(hm(13, 0), hm(14, 0)),
            source: MealSource::Auto
        },
        "맞닿기만 하면 그 시간에 먹을 수 있다"
    );

    // 그리고 12:20~13:00 자체는 식사시간이 아니므로,
    // 수업이 없는 다른 전담이라면 그 시간 보결 후보가 된다
    let w2 = World::with_high_lunch(Interval::new(hm(13, 0), hm(14, 0))).manual(Interval::new(hm(13, 0), hm(14, 0)));
    let (ok, code, _) = w2.verdict(&w2.find(4, SLOT_PERIOD, Some(5))); // 12:20~13:00
    assert!(ok, "13:00 시작 식사시간은 13:00 종료 수업과 겹치지 않는다 ({code})");
}

// ============================================================
//  알 수 없을 때는 빼지 않는다
// ============================================================

#[test]
fn 식사시간을_정하지_못하면_후보에서_빼지_않고_알려_준다() {
    // 4교시까지만 수업 + 학교 기본 식사시간 미설정 → 둘 다 가능해서 판단 불가
    let w = World::new().teach(4, 4);
    assert_eq!(w.meal(), Meal::Unknown(super::meal::MealUnknown::NeedsDefault));

    let r = w.find(1, SLOT_PERIOD, Some(5));
    let (ok, code, _) = w.verdict(&r);
    assert!(ok, "모르면서 빼지 않는다 ({code})");

    assert!(
        r.warnings.iter().any(|w| w.contains("김전담") && w.contains("식사시간")),
        "대신 왜 그런지 알려 준다: {:?}",
        r.warnings
    );
}

#[test]
fn 식사시간을_정한_뒤에는_안내가_사라진다() {
    let w = World::new().teach(4, 4).school_default(lunch_high());
    let r = w.find(1, SLOT_PERIOD, Some(5));
    assert!(
        !r.warnings.iter().any(|w| w.contains("식사시간")),
        "{:?}",
        r.warnings
    );
}

#[test]
fn 점심_보결에서는_식사시간_안내를_띄우지_않는다() {
    // 점심 보결에는 이 제약 자체가 없으므로 알릴 것도 없다
    let w = World::new().teach(4, 4);
    let r = w.find(4, SLOT_LUNCH, None);
    assert!(!r.warnings.iter().any(|w| w.contains("식사시간")), "{:?}", r.warnings);
}

// ============================================================
//  담임에게는 걸리지 않는다 (기존 담임 점심 규칙과 분리)
// ============================================================

#[test]
fn 담임에게는_전담_식사시간_규칙이_걸리지_않는다() {
    // 2학년 담임은 저학년 점심(12:10~13:10)에 자기 반 급식 지도로 이미 빠진다.
    // 그것은 기존 담임 규칙이고, 전담 식사시간 코드가 아니어야 한다.
    let w = World::new().school_default(lunch_low());
    let r = w.find(1, SLOT_LUNCH, None); // 1학년 점심 12:10~13:10
    let two = r
        .excluded
        .iter()
        .find(|c| c.teacher_id == 2)
        .expect("2학년 담임은 자기 점심 지도로 빠진다");
    assert_eq!(two.reason_code, EXCLUDED_LUNCH_DUTY);
    assert_ne!(two.reason_code, EXCLUDED_SPECIAL_MEAL);
}

#[test]
fn 전담이_아닌_교사는_식사시간으로_빠지지_않는다() {
    // 1학년 담임은 저학년 점심에 급식 지도가 있지만, 5교시에는 전담 식사시간
    // 규칙과 무관하다. (자기 반 수업이면 수업 중으로 빠진다)
    let w = World::new().school_default(lunch_high());
    let r = w.find(4, SLOT_PERIOD, Some(5)); // 12:20~13:00
    for c in r.excluded.iter().filter(|c| c.teacher_id != SPECIAL_ID) {
        assert_ne!(
            c.reason_code, EXCLUDED_SPECIAL_MEAL,
            "{} 는 전담이 아니다",
            c.name
        );
    }
}

// ============================================================
//  회귀 — 식사시간 말고 다른 것은 그대로다
// ============================================================

#[test]
fn 식사시간_기능이_꺼진_상태의_판정은_예전과_같다() {
    // 기본값도 수동 지정도 없고 수업도 없으면 판단 불가 → 아무도 빼지 않는다
    let w = World::new();
    let r = w.find(1, SLOT_PERIOD, Some(5));
    assert!(
        !r.excluded.iter().any(|c| c.reason_code == EXCLUDED_SPECIAL_MEAL),
        "정해지지 않았는데 빼는 사람이 있으면 안 된다"
    );
}

#[test]
fn 식사시간과_겹치지_않는_시간에는_그대로_후보다() {
    let w = World::new().manual(lunch_high());
    // 1교시 09:00~09:40
    let (ok, code, _) = w.verdict(&w.find(1, SLOT_PERIOD, Some(1)));
    assert!(ok, "{code}");
    assert!(code.starts_with("ELIGIBLE"), "{code}");
}
