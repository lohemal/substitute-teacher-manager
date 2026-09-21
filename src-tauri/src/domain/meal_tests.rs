//! 전담교사 식사시간 판정 시험.
//!
//! 요청서의 우리 학교 사례를 그대로 쓴다.
//!
//! ```text
//!            저학년(1·2·3)      고학년(4·5·6)
//! 점심       12:10~13:10       13:10~14:00
//! 5교시      13:10~13:50       12:20~13:00
//! ```

use super::meal::*;
use super::time::Interval;

fn hm(h: i32, m: i32) -> i32 {
    h * 60 + m
}
fn iv(a: (i32, i32), b: (i32, i32)) -> Interval {
    Interval::new(hm(a.0, a.1), hm(b.0, b.1))
}

/// 저학년 점심 12:10~13:10
fn lunch_low() -> Interval {
    iv((12, 10), (13, 10))
}
/// 고학년 점심 13:10~14:00
fn lunch_high() -> Interval {
    iv((13, 10), (14, 0))
}
/// 저학년 5교시 13:10~13:50
fn p5_low() -> Interval {
    iv((13, 10), (13, 50))
}
/// 고학년 5교시 12:20~13:00
fn p5_high() -> Interval {
    iv((12, 20), (13, 0))
}

fn both() -> Vec<Interval> {
    vec![lunch_low(), lunch_high()]
}

fn input(lessons: Vec<Interval>) -> MealInput {
    MealInput {
        candidates: both(),
        school_default: None,
        manual: None,
        lessons,
    }
}

// ============================================================
//  Case A — 고학년 5교시를 하면 늦은 점심
// ============================================================

#[test]
fn case_a_고학년_5교시를_하면_늦은_식사시간이_자동으로_정해진다() {
    let m = resolve(&input(vec![p5_high()]));
    assert_eq!(
        m,
        Meal::Known {
            interval: lunch_high(),
            source: MealSource::Auto
        },
        "12:20~13:00 수업은 12:10~13:10 과 겹치므로 13:10~14:00 만 남는다"
    );
}

// ============================================================
//  Case B — 저학년 5교시를 하면 이른 점심
// ============================================================

#[test]
fn case_b_저학년_5교시를_하면_이른_식사시간이_자동으로_정해진다() {
    let m = resolve(&input(vec![p5_low()]));
    assert_eq!(
        m,
        Meal::Known {
            interval: lunch_low(),
            source: MealSource::Auto
        },
        "13:10~13:50 수업은 13:10~14:00 과 겹치므로 12:10~13:10 만 남는다"
    );
}

// ============================================================
//  Case C — 둘 다 가능하면 학교 기본 식사시간
// ============================================================

#[test]
fn case_c_둘_다_가능하면_학교_기본_식사시간을_쓴다() {
    let m = resolve(&MealInput {
        school_default: Some(lunch_low()),
        ..input(vec![iv((11, 30), (12, 10))]) // 4교시까지만
    });
    assert_eq!(
        m,
        Meal::Known {
            interval: lunch_low(),
            source: MealSource::SchoolDefault
        }
    );
}

#[test]
fn case_c_수업이_아예_없어도_기본_식사시간을_쓴다() {
    let m = resolve(&MealInput {
        school_default: Some(lunch_high()),
        ..input(vec![])
    });
    assert_eq!(
        m,
        Meal::Known {
            interval: lunch_high(),
            source: MealSource::SchoolDefault
        }
    );
}

// ============================================================
//  Case D — 수동 지정이 가장 세다
// ============================================================

#[test]
fn case_d_수동_지정은_기본값보다_우선한다() {
    let m = resolve(&MealInput {
        school_default: Some(lunch_low()),
        manual: Some(lunch_high()),
        ..input(vec![iv((11, 30), (12, 10))])
    });
    assert_eq!(
        m,
        Meal::Known {
            interval: lunch_high(),
            source: MealSource::Manual
        }
    );
}

#[test]
fn 수동_지정은_자동_판정보다도_우선한다() {
    // 자동이라면 13:10~14:00 이 나올 상황
    let m = resolve(&MealInput {
        manual: Some(lunch_low()),
        ..input(vec![p5_high()])
    });
    assert_eq!(
        m,
        Meal::Known {
            interval: lunch_low(),
            source: MealSource::Manual
        }
    );
}

#[test]
fn 수동_지정은_후보에_없는_시간이어도_그대로_쓴다() {
    // 학교 사정으로 시정표에 없는 시간에 먹는 전담이 있을 수 있다
    let odd = iv((11, 40), (12, 20));
    let m = resolve(&MealInput {
        manual: Some(odd),
        ..input(vec![])
    });
    assert_eq!(m.interval(), Some(odd));
}

// ============================================================
//  기본값이 그 날 수업과 겹칠 때
// ============================================================

#[test]
fn 기본_식사시간이_그날_수업과_겹치면_가능한_시간으로_바꾼다() {
    let m = resolve(&MealInput {
        school_default: Some(lunch_low()), // 이른 점심이 기본
        ..input(vec![p5_high()])           // 그런데 12:20~13:00 수업
    });
    assert_eq!(
        m,
        Meal::Known {
            interval: lunch_high(),
            source: MealSource::Auto
        },
        "기본값을 고집하지 않고 가능한 시간을 쓴다"
    );
}

// ============================================================
//  정할 수 없는 경우 — 추측하지 않는다
// ============================================================

#[test]
fn 기본값이_없고_여러_개가_가능하면_확인_필요다() {
    let m = resolve(&input(vec![]));
    assert_eq!(m, Meal::Unknown(MealUnknown::NeedsDefault));
    assert_eq!(m.interval(), None, "모르면 시간을 만들어 내지 않는다");
}

#[test]
fn 모든_점심시간이_수업과_겹치면_확인_필요다() {
    let m = resolve(&MealInput {
        school_default: Some(lunch_low()),
        ..input(vec![p5_low(), p5_high()])
    });
    assert_eq!(m, Meal::Unknown(MealUnknown::AllBusy));
}

#[test]
fn 시정표에_점심이_없으면_확인_필요다() {
    let m = resolve(&MealInput {
        candidates: vec![],
        school_default: None,
        manual: None,
        lessons: vec![],
    });
    assert_eq!(m, Meal::Unknown(MealUnknown::NoCandidates));
}

#[test]
fn 점심이_하나뿐인_학교는_기본값_없이도_정해진다() {
    // 전 학년이 같은 시간에 먹는 학교
    let m = resolve(&MealInput {
        candidates: vec![lunch_low()],
        school_default: None,
        manual: None,
        lessons: vec![],
    });
    assert_eq!(
        m,
        Meal::Known {
            interval: lunch_low(),
            source: MealSource::Auto
        },
        "고를 것이 하나뿐이면 기본값을 묻지 않는다"
    );
}

#[test]
fn 알_수_없는_이유마다_다른_안내를_준다() {
    for u in [
        MealUnknown::NoCandidates,
        MealUnknown::NeedsDefault,
        MealUnknown::AllBusy,
    ] {
        assert!(!u.message().is_empty());
    }
    assert_ne!(
        MealUnknown::NeedsDefault.message(),
        MealUnknown::AllBusy.message()
    );
}

// ============================================================
//  Case H — 경계는 겹침이 아니다
// ============================================================

#[test]
fn case_h_수업_종료와_식사_시작이_같으면_겹치지_않는다() {
    // 수업 12:20~13:00 / 식사 13:00~14:00
    let meal = iv((13, 0), (14, 0));
    let m = resolve(&MealInput {
        candidates: vec![meal],
        school_default: None,
        manual: None,
        lessons: vec![p5_high()],
    });
    assert_eq!(
        m,
        Meal::Known {
            interval: meal,
            source: MealSource::Auto
        },
        "맞닿기만 하면 식사할 수 있다"
    );
}

#[test]
fn 식사_종료와_수업_시작이_같아도_겹치지_않는다() {
    let meal = iv((12, 10), (13, 10));
    let m = resolve(&MealInput {
        candidates: vec![meal],
        school_default: None,
        manual: None,
        lessons: vec![p5_low()], // 13:10 시작
    });
    assert_eq!(m.interval(), Some(meal));
}

#[test]
fn 일_분만_겹쳐도_그_식사시간은_쓸_수_없다() {
    // 13:59~14:30 은 늦은 점심(13:10~14:00)과 딱 1분 겹친다.
    // 이른 점심(12:10~13:10)과는 전혀 겹치지 않는다.
    let m = resolve(&input(vec![iv((13, 59), (14, 30))]));
    assert_eq!(
        m,
        Meal::Known {
            interval: lunch_low(),
            source: MealSource::Auto
        },
        "1분이라도 겹치면 그 식사시간은 빠지고 이른 점심만 남는다"
    );
}

// ============================================================
//  Case G — 수동 지정의 안전성
// ============================================================

#[test]
fn case_g_수동_지정이_실제_수업과_겹치면_알려_준다() {
    let hit = manual_conflict(&lunch_low(), &[p5_high()]);
    assert_eq!(hit, Some(p5_high()), "12:10~13:10 은 12:20~13:00 수업과 겹친다");
}

#[test]
fn 겹치지_않는_수동_지정은_통과한다() {
    assert_eq!(manual_conflict(&lunch_high(), &[p5_high()]), None);
    assert_eq!(manual_conflict(&lunch_low(), &[]), None);
    // 맞닿는 경우
    assert_eq!(
        manual_conflict(&iv((13, 0), (14, 0)), &[p5_high()]),
        None,
        "13:00 시작은 13:00 종료 수업과 겹치지 않는다"
    );
}

// ============================================================
//  후보 합치기
// ============================================================

#[test]
fn 같은_시각의_점심은_하나로_합친다() {
    // 1·2·3학년이 모두 12:10~13:10 이면 후보는 하나다
    let v = dedup_candidates(vec![
        lunch_low(),
        lunch_low(),
        lunch_low(),
        lunch_high(),
        lunch_high(),
        lunch_high(),
    ]);
    assert_eq!(v, vec![lunch_low(), lunch_high()]);
}

#[test]
fn 후보는_시작_시각_순으로_돌려준다() {
    let v = dedup_candidates(vec![lunch_high(), lunch_low()]);
    assert_eq!(v[0], lunch_low());
}

#[test]
fn 점심_패턴이_셋_이상인_학교도_다룬다() {
    let a = iv((11, 50), (12, 40));
    let b = lunch_low();
    let c = lunch_high();
    let v = dedup_candidates(vec![c, a, b, a]);
    assert_eq!(v, vec![a, b, c]);

    // 그중 둘이 막히면 남은 하나로 자동 결정된다
    let m = resolve(&MealInput {
        candidates: v,
        school_default: None,
        manual: None,
        lessons: vec![iv((11, 50), (13, 10))],
    });
    assert_eq!(
        m,
        Meal::Known {
            interval: c,
            source: MealSource::Auto
        }
    );
}
