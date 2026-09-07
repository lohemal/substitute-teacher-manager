//! 보결 수당 계산 시험. DB 없이 계산만 확인한다.

use super::pay::*;

fn cases(n: usize) -> Vec<Case> {
    (0..n)
        .map(|i| Case::period("2026-09-03", 9 * 60 + i as i32 * 50))
        .collect()
}

fn tally(got: usize, own: usize) -> Tally {
    Tally {
        cases: cases(got),
        own_caused: cases(own),
    }
}

fn cfg(policy: &str, per_case: i64) -> PayConfig {
    PayConfig {
        policy: policy.to_string(),
        per_case,
    }
}

// ------------------------------------------------------------
//  ALL_ASSIGNED — 전체 지급
// ------------------------------------------------------------

#[test]
fn 전체_지급이면_보결한_횟수를_모두_인정한다() {
    let s = settle(&cfg(ALL_ASSIGNED, 15_000), &tally(7, 0));
    assert_eq!(s.substituted, 7);
    assert_eq!(s.payable, 7);
    assert_eq!(s.amount, 105_000);
}

#[test]
fn 전체_지급은_본인_발생_보결을_차감하지_않는다() {
    let s = settle(&cfg(ALL_ASSIGNED, 15_000), &tally(7, 2));
    assert_eq!(s.own_caused, 2, "참고정보로는 그대로 보여 준다");
    assert_eq!(s.payable, 7, "지급 인정 횟수에서는 빼지 않는다");
    assert_eq!(s.amount, 105_000);
}

// ------------------------------------------------------------
//  DEDUCT_OWN_CAUSED — 본인 발생분 차감
// ------------------------------------------------------------

#[test]
fn 차감_정책은_본인_발생_보결을_뺀다() {
    // 요청서의 예: 보결 7회, 본인 발생 2회 → 인정 5회 × 15,000 = 75,000원
    let s = settle(&cfg(DEDUCT_OWN_CAUSED, 15_000), &tally(7, 2));
    assert_eq!(s.substituted, 7);
    assert_eq!(s.own_caused, 2);
    assert_eq!(s.payable, 5);
    assert_eq!(s.amount, 75_000);
}

#[test]
fn 차감_결과가_음수이면_0회로_본다() {
    let s = settle(&cfg(DEDUCT_OWN_CAUSED, 15_000), &tally(2, 5));
    assert_eq!(s.payable, 0, "수당을 되돌려 받는 일은 없다");
    assert_eq!(s.amount, 0);
    assert_eq!(s.substituted, 2, "원래 횟수는 그대로 보여 준다");
    assert_eq!(s.own_caused, 5);
}

#[test]
fn 차감해서_정확히_0이_되는_경우() {
    let s = settle(&cfg(DEDUCT_OWN_CAUSED, 15_000), &tally(3, 3));
    assert_eq!(s.payable, 0);
    assert_eq!(s.amount, 0);
}

#[test]
fn 보결이_없으면_지급액도_없다() {
    for policy in [ALL_ASSIGNED, DEDUCT_OWN_CAUSED] {
        let s = settle(&cfg(policy, 15_000), &tally(0, 0));
        assert_eq!(s.payable, 0, "{policy}");
        assert_eq!(s.amount, 0, "{policy}");
    }
}

// ------------------------------------------------------------
//  1회 수당
// ------------------------------------------------------------

#[test]
fn 수당_15000원_곱_5회는_75000원() {
    let s = settle(&cfg(ALL_ASSIGNED, 15_000), &tally(5, 0));
    assert_eq!(s.amount, 75_000);
}

#[test]
fn 수당이_0원이면_지급액도_0원이다() {
    let s = settle(&cfg(ALL_ASSIGNED, 0), &tally(9, 0));
    assert_eq!(s.payable, 9, "횟수는 그대로 센다");
    assert_eq!(s.amount, 0, "아직 금액을 정하지 않은 학교");
}

#[test]
fn 음수_수당은_0원으로_본다() {
    // 화면에서 막지만, 계산 계층도 스스로를 지킨다
    let s = settle(&cfg(ALL_ASSIGNED, -5_000), &tally(3, 0));
    assert_eq!(s.per_case, 0);
    assert_eq!(s.amount, 0);
}

// ------------------------------------------------------------
//  정책 바꾸기 — 기록은 그대로, 결과만 다시 나온다
// ------------------------------------------------------------

#[test]
fn 같은_기록에_정책만_바꾸면_결과가_다시_계산된다() {
    let t = tally(7, 2);

    let all = settle(&cfg(ALL_ASSIGNED, 15_000), &t);
    let deduct = settle(&cfg(DEDUCT_OWN_CAUSED, 15_000), &t);

    assert_eq!(all.amount, 105_000);
    assert_eq!(deduct.amount, 75_000);
    assert_eq!(
        (all.substituted, all.own_caused),
        (deduct.substituted, deduct.own_caused),
        "정책을 바꿔도 집계된 기록 자체는 같다"
    );
}

#[test]
fn 같은_기록에_수당만_바꾸면_금액만_달라진다() {
    let t = tally(5, 0);
    let a = settle(&cfg(ALL_ASSIGNED, 15_000), &t);
    let b = settle(&cfg(ALL_ASSIGNED, 20_000), &t);
    assert_eq!(a.payable, b.payable);
    assert_eq!(a.amount, 75_000);
    assert_eq!(b.amount, 100_000);
}

// ------------------------------------------------------------
//  정책 등록부
// ------------------------------------------------------------

#[test]
fn 두_정책이_등록되어_있다() {
    let codes: Vec<&str> = all_policies().iter().map(|r| r.code()).collect();
    assert_eq!(codes, vec![ALL_ASSIGNED, DEDUCT_OWN_CAUSED]);
}

#[test]
fn 모든_정책은_이름과_설명을_가진다() {
    for r in all_policies() {
        assert!(!r.label().is_empty(), "{}", r.code());
        assert!(!r.hint().is_empty(), "{}", r.code());
    }
}

#[test]
fn 알_수_없는_정책은_전체_지급으로_되돌린다() {
    assert_eq!(rule_of("NOPE").code(), ALL_ASSIGNED);
    assert_eq!(rule_of("").code(), ALL_ASSIGNED);
    assert!(!is_known_policy("MONTHLY_CAP"), "아직 만들지 않은 정책");
    assert!(is_known_policy(ALL_ASSIGNED));
    assert!(is_known_policy(DEDUCT_OWN_CAUSED));
}

#[test]
fn 어떤_정책도_음수를_돌려주지_않는다() {
    let t = tally(1, 9);
    for r in all_policies() {
        assert!(r.payable(&t) >= 0, "{}", r.code());
    }
}

// ------------------------------------------------------------
//  점심·여러 교시
// ------------------------------------------------------------

#[test]
fn 점심_보결도_한_건으로_센다() {
    let t = Tally {
        cases: vec![
            Case::period("2026-09-03", 9 * 60),
            Case::lunch("2026-09-03", 12 * 60),
            Case::period("2026-09-03", 13 * 60),
        ],
        own_caused: vec![],
    };
    let s = settle(&cfg(ALL_ASSIGNED, 15_000), &t);
    assert_eq!(s.substituted, 3, "점심도 정식 배정이면 1회다");
    assert_eq!(s.amount, 45_000);
}

#[test]
fn 같은_날_여러_교시는_각각_한_건이다() {
    let t = Tally {
        cases: vec![
            Case::period("2026-09-07", 9 * 60),
            Case::period("2026-09-07", 9 * 60 + 50),
            Case::period("2026-09-07", 10 * 60 + 40),
        ],
        own_caused: vec![],
    };
    assert_eq!(settle(&cfg(ALL_ASSIGNED, 10_000), &t).amount, 30_000);
}

// ------------------------------------------------------------
//  계산 설명
// ------------------------------------------------------------

#[test]
fn 전체_지급_설명() {
    let lines = explain(&cfg(ALL_ASSIGNED, 15_000), &tally(7, 2));
    assert_eq!(lines[0], "보결 7회 전체 인정");
    assert_eq!(lines[1], "7회 × 15,000원 = 105,000원");
}

#[test]
fn 차감_정책_설명() {
    let lines = explain(&cfg(DEDUCT_OWN_CAUSED, 15_000), &tally(7, 2));
    assert_eq!(lines[0], "7회 - 2회 = 지급 인정 5회");
    assert_eq!(lines[1], "5회 × 15,000원 = 75,000원");
}

#[test]
fn 차감이_음수일_때는_0회로_잡은_이유를_함께_적는다() {
    let lines = explain(&cfg(DEDUCT_OWN_CAUSED, 15_000), &tally(2, 5));
    assert_eq!(lines[0], "2회 - 5회 = 지급 인정 0회");
    assert!(lines[1].contains("0회보다 작아지지 않습니다"), "{:?}", lines);
    assert_eq!(lines[2], "0회 × 15,000원 = 0원");
}

// ------------------------------------------------------------
//  금액 표기
// ------------------------------------------------------------

#[test]
fn 천_단위_쉼표를_넣는다() {
    assert_eq!(won(0), "0");
    assert_eq!(won(15), "15");
    assert_eq!(won(150), "150");
    assert_eq!(won(1_500), "1,500");
    assert_eq!(won(15_000), "15,000");
    assert_eq!(won(135_000), "135,000");
    assert_eq!(won(1_350_000), "1,350,000");
    assert_eq!(won(12_345_678), "12,345,678");
}

#[test]
fn 음수도_모양이_깨지지_않는다() {
    assert_eq!(won(-15_000), "-15,000");
}
