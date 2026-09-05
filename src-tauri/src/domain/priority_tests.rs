//! Phase 7 검증 — 학교가 정한 기준대로 순서가 바뀌는지 확인한다.

use super::find::*;
use super::priority::*;

fn cand(
    id: i64,
    name: &str,
    role: &str,
    homeroom_grades: Vec<i32>,
    today: i32,
    month: i32,
    total: i32,
    reason_code: &str,
) -> Candidate {
    Candidate {
        rank: 0,
        reason: String::new(),
        decided_by: None,
        teacher_id: id,
        name: name.into(),
        role_code: role.into(),
        role_label: match role {
            "HOMEROOM" => "담임",
            "SPECIAL" => "전담",
            _ => "기타",
        }
        .into(),
        duty: String::new(),
        eligible: true,
        reason_code: reason_code.into(),
        status_label: status_label(reason_code).into(),
        detail: None,
        counts: SubCounts { today, month, total },
        homeroom_grades,
        blocks: vec![],
    }
}

fn hr(id: i64, name: &str, grade: i32, today: i32, month: i32, total: i32) -> Candidate {
    cand(
        id,
        name,
        "HOMEROOM",
        vec![grade],
        today,
        month,
        total,
        ELIGIBLE_HOMEROOM_FREE,
    )
}

fn sp(id: i64, name: &str, today: i32, month: i32, total: i32) -> Candidate {
    cand(
        id,
        name,
        "SPECIAL",
        vec![],
        today,
        month,
        total,
        ELIGIBLE_SPECIALIST_FREE,
    )
}

/// 기준 순서를 문자열 목록으로 간단히 만든다.
fn order(keys: &[&str]) -> Vec<RuleSetting> {
    keys.iter()
        .enumerate()
        .map(|(i, k)| RuleSetting {
            rule_key: k.to_string(),
            enabled: true,
            sort_order: i as i32 + 1,
        })
        .collect()
}

fn names(list: &[Candidate]) -> Vec<String> {
    list.iter().map(|c| c.name.clone()).collect()
}

// ============================================================
//  기준 순서가 실제로 결과를 바꾼다
// ============================================================

/// 학교 A: 동학년 → 당일 → 누적
/// 학교 B: 누적 → 당일 → 동학년
/// 같은 후보인데 순서가 달라야 한다.
fn three_candidates() -> Vec<Candidate> {
    vec![
        // 동학년(5학년)이지만 누적이 많다
        hr(1, "가동학년", 5, 0, 2, 10),
        // 다른 학년인데 누적이 가장 적다
        hr(2, "나누적적음", 1, 0, 1, 1),
        // 다른 학년, 누적 중간
        hr(3, "다중간", 2, 0, 1, 5),
    ]
}

#[test]
fn 학교a_동학년_우선이_다른_기준보다_먼저_적용된다() {
    let mut list = three_candidates();
    rank_candidates(
        &mut list,
        &order(&["SAME_GRADE", "FEWEST_TODAY", "FEWEST_TOTAL"]),
        5,
    );

    assert_eq!(names(&list), vec!["가동학년", "나누적적음", "다중간"]);
    assert_eq!(list[0].rank, 1);
    assert!(list[0].reason.contains("동학년"), "{}", list[0].reason);
}

#[test]
fn 학교b_누적_우선으로_바꾸면_순서가_달라진다() {
    let mut list = three_candidates();
    rank_candidates(
        &mut list,
        &order(&["FEWEST_TOTAL", "FEWEST_TODAY", "SAME_GRADE"]),
        5,
    );

    assert_eq!(
        names(&list),
        vec!["나누적적음", "다중간", "가동학년"],
        "누적이 적은 순으로 바뀌어야 한다"
    );
    assert_eq!(list[0].rank, 1);
}

#[test]
fn 같은_후보라도_기준_순서에_따라_1순위가_달라진다() {
    let mut a = three_candidates();
    let mut b = three_candidates();
    rank_candidates(&mut a, &order(&["SAME_GRADE", "FEWEST_TOTAL"]), 5);
    rank_candidates(&mut b, &order(&["FEWEST_TOTAL", "SAME_GRADE"]), 5);

    assert_eq!(a[0].name, "가동학년");
    assert_eq!(b[0].name, "나누적적음");
    assert_ne!(a[0].teacher_id, b[0].teacher_id);
}

#[test]
fn 당일_횟수가_누적보다_먼저면_당일이_먼저_적용된다() {
    let mut list = vec![
        hr(1, "가오늘많음", 3, 2, 2, 0), // 누적은 0인데 오늘 2회
        hr(2, "나오늘없음", 3, 0, 5, 20), // 누적은 20인데 오늘 0회
    ];
    rank_candidates(&mut list, &order(&["FEWEST_TODAY", "FEWEST_TOTAL"]), 5);
    assert_eq!(names(&list), vec!["나오늘없음", "가오늘많음"]);

    // 순서를 뒤집으면 결과도 뒤집힌다
    let mut list2 = vec![
        hr(1, "가오늘많음", 3, 2, 2, 0),
        hr(2, "나오늘없음", 3, 0, 5, 20),
    ];
    rank_candidates(&mut list2, &order(&["FEWEST_TOTAL", "FEWEST_TODAY"]), 5);
    assert_eq!(names(&list2), vec!["가오늘많음", "나오늘없음"]);
}

#[test]
fn 이번_달_기준도_따로_동작한다() {
    let mut list = vec![
        hr(1, "가이번달많음", 3, 0, 9, 9),
        hr(2, "나이번달적음", 3, 0, 1, 30),
    ];
    rank_candidates(&mut list, &order(&["FEWEST_MONTH"]), 5);
    assert_eq!(names(&list), vec!["나이번달적음", "가이번달많음"]);
}

// ============================================================
//  개별 기준
// ============================================================

#[test]
fn 전담교사_우선() {
    let mut list = vec![
        hr(1, "가담임", 5, 0, 0, 0),
        sp(2, "나전담", 0, 0, 0),
    ];
    rank_candidates(&mut list, &order(&["PREFER_SPECIAL"]), 5);
    assert_eq!(names(&list), vec!["나전담", "가담임"]);
    assert!(list[0].reason.contains("전담"), "{}", list[0].reason);

    // 이 기준을 끄면 동점 처리로 이름 순
    let mut off = vec![
        hr(1, "가담임", 5, 0, 0, 0),
        sp(2, "나전담", 0, 0, 0),
    ];
    rank_candidates(
        &mut off,
        &[RuleSetting {
            rule_key: "PREFER_SPECIAL".into(),
            enabled: false,
            sort_order: 1,
        }],
        5,
    );
    assert_eq!(names(&off), vec!["가담임", "나전담"]);
}

#[test]
fn 수업이_종료된_담임교사_우선() {
    let mut list = vec![
        // 공강 담임
        hr(1, "가공강담임", 5, 0, 0, 0),
        // 수업 종료 전담
        cand(2, "나종료전담", "SPECIAL", vec![], 0, 0, 0, ELIGIBLE_AFTER_SCHOOL_END),
        // 수업 종료 담임
        cand(3, "다종료담임", "HOMEROOM", vec![1], 0, 0, 0, ELIGIBLE_AFTER_SCHOOL_END),
    ];
    rank_candidates(&mut list, &order(&["PREFER_FINISHED"]), 5);

    assert_eq!(
        names(&list),
        vec!["다종료담임", "나종료전담", "가공강담임"],
        "수업 종료 담임 → 수업 종료 → 그 밖"
    );
    assert!(list[0].reason.contains("수업 종료"), "{}", list[0].reason);
}

#[test]
fn 저학년_담임교사_우선() {
    let mut list = vec![
        hr(1, "가6학년", 6, 0, 0, 0),
        hr(2, "나1학년", 1, 0, 0, 0),
        hr(3, "다3학년", 3, 0, 0, 0),
        sp(4, "라전담", 0, 0, 0),
    ];
    rank_candidates(&mut list, &order(&["PREFER_LOWER_GRADE"]), 5);

    assert_eq!(
        names(&list),
        vec!["나1학년", "다3학년", "가6학년", "라전담"],
        "담임이 아닌 교사는 뒤로"
    );
    assert!(list[0].reason.contains("1학년 담임"), "{}", list[0].reason);
}

#[test]
fn 동학년은_담임_학년이_여러_개여도_인정한다() {
    // 복식학급 담임 (1학년과 2학년을 함께 맡음)
    let mut list = vec![
        hr(1, "가5학년", 5, 0, 0, 5),
        cand(2, "나복식", "HOMEROOM", vec![1, 2], 0, 0, 0, ELIGIBLE_HOMEROOM_FREE),
    ];
    rank_candidates(&mut list, &order(&["SAME_GRADE"]), 2);
    assert_eq!(names(&list), vec!["나복식", "가5학년"]);
}

// ============================================================
//  동점 처리
// ============================================================

#[test]
fn 완전히_같은_조건이면_항상_같은_순서가_나온다() {
    let make = || {
        vec![
            hr(3, "다", 5, 0, 0, 0),
            hr(1, "가", 5, 0, 0, 0),
            hr(2, "나", 5, 0, 0, 0),
        ]
    };
    let rules = order(&["SAME_GRADE", "FEWEST_TODAY", "FEWEST_TOTAL"]);

    // 여러 번 돌려도 같아야 한다
    let mut first: Option<Vec<String>> = None;
    for _ in 0..5 {
        let mut list = make();
        rank_candidates(&mut list, &rules, 5);
        let got = names(&list);
        match &first {
            None => first = Some(got),
            Some(f) => assert_eq!(*f, got, "같은 자료면 항상 같은 순서여야 한다"),
        }
    }
    assert_eq!(first.unwrap(), vec!["가", "나", "다"], "이름 가나다순");
}

#[test]
fn 입력_순서가_달라도_결과는_같다() {
    let rules = order(&["SAME_GRADE"]);
    let mut a = vec![hr(1, "가", 5, 0, 0, 3), hr(2, "나", 5, 0, 0, 1)];
    let mut b = vec![hr(2, "나", 5, 0, 0, 1), hr(1, "가", 5, 0, 0, 3)];
    rank_candidates(&mut a, &rules, 5);
    rank_candidates(&mut b, &rules, 5);
    assert_eq!(names(&a), names(&b));
    assert_eq!(names(&a), vec!["나", "가"], "동점이면 누적 적은 순이 먼저");
}

#[test]
fn 동점_처리는_누적_당일_학년_이름_순서로_적용된다() {
    // 누적이 갈리는 경우
    let mut by_total = vec![hr(1, "가", 5, 0, 0, 5), hr(2, "나", 5, 0, 0, 1)];
    rank_candidates(&mut by_total, &order(&["SAME_GRADE"]), 5);
    assert_eq!(names(&by_total), vec!["나", "가"]);

    // 누적이 같고 당일이 갈리는 경우
    let mut by_today = vec![hr(1, "가", 5, 2, 0, 1), hr(2, "나", 5, 0, 0, 1)];
    rank_candidates(&mut by_today, &order(&["SAME_GRADE"]), 5);
    assert_eq!(names(&by_today), vec!["나", "가"]);

    // 누적·당일이 같고 학년이 갈리는 경우
    let mut by_grade = vec![hr(1, "가", 6, 0, 0, 1), hr(2, "나", 2, 0, 0, 1)];
    rank_candidates(&mut by_grade, &order(&["FEWEST_TODAY"]), 5);
    assert_eq!(names(&by_grade), vec!["나", "가"]);
}

// ============================================================
//  근거 문구
// ============================================================

#[test]
fn 추천_근거는_켜_둔_기준의_말로_만든다() {
    let mut list = vec![
        hr(1, "가동학년", 5, 0, 1, 2),
        sp(2, "나전담", 3, 3, 8),
    ];
    rank_candidates(
        &mut list,
        &order(&["SAME_GRADE", "FEWEST_TODAY", "PREFER_SPECIAL"]),
        5,
    );

    let top = &list[0];
    assert_eq!(top.name, "가동학년");
    assert!(top.reason.contains("동학년"), "{}", top.reason);
    assert!(top.reason.contains("오늘 0회"), "{}", top.reason);
    assert!(!top.reason.contains("1순위"), "순위 숫자만 쓰지 않는다");

    let second = &list[1];
    assert!(second.reason.contains("전담"), "{}", second.reason);
}

#[test]
fn 해당하는_기준이_없으면_현재_상태를_근거로_쓴다() {
    let mut list = vec![sp(1, "가전담", 5, 5, 5)];
    rank_candidates(&mut list, &order(&["SAME_GRADE"]), 5);
    assert_eq!(list[0].reason, "전담 공강");
}

#[test]
fn 순서를_가른_기준을_알려준다() {
    let mut list = vec![
        hr(1, "가동학년", 5, 0, 0, 9),
        hr(2, "나다른학년", 1, 0, 0, 0),
    ];
    rank_candidates(&mut list, &order(&["SAME_GRADE", "FEWEST_TOTAL"]), 5);

    assert_eq!(list[0].decided_by.as_deref(), Some("동학년 교사 우선"));
    assert_eq!(list[1].decided_by, None, "마지막 후보는 비교 대상이 없다");
}

// ============================================================
//  설정 자체
// ============================================================

#[test]
fn 꺼둔_기준은_정렬에_쓰이지_않는다() {
    let mut list = vec![
        hr(1, "가동학년", 5, 0, 0, 9),
        hr(2, "나다른학년", 1, 0, 0, 0),
    ];
    rank_candidates(
        &mut list,
        &[
            RuleSetting {
                rule_key: "SAME_GRADE".into(),
                enabled: false,
                sort_order: 1,
            },
            RuleSetting {
                rule_key: "FEWEST_TOTAL".into(),
                enabled: true,
                sort_order: 2,
            },
        ],
        5,
    );
    assert_eq!(names(&list), vec!["나다른학년", "가동학년"]);
    assert!(
        !list[0].reason.contains("동학년"),
        "꺼둔 기준은 근거에도 나오지 않는다"
    );
}

#[test]
fn 모르는_기준_이름은_그냥_무시한다() {
    let mut list = vec![hr(1, "가", 5, 0, 0, 5), hr(2, "나", 5, 0, 0, 1)];
    rank_candidates(
        &mut list,
        &[
            RuleSetting {
                rule_key: "옛날기준".into(),
                enabled: true,
                sort_order: 1,
            },
            RuleSetting {
                rule_key: "FEWEST_TOTAL".into(),
                enabled: true,
                sort_order: 2,
            },
        ],
        5,
    );
    assert_eq!(names(&list), vec!["나", "가"]);
}

#[test]
fn 후보가_없어도_문제가_없다() {
    let mut list: Vec<Candidate> = vec![];
    rank_candidates(&mut list, &order(&["SAME_GRADE"]), 5);
    assert!(list.is_empty());
}

#[test]
fn 후보가_한_명이면_1순위가_된다() {
    let mut list = vec![hr(1, "가", 5, 0, 0, 0)];
    rank_candidates(&mut list, &order(&["SAME_GRADE"]), 5);
    assert_eq!(list[0].rank, 1);
    assert_eq!(list[0].decided_by, None);
}

#[test]
fn 기본_기준_목록에_필요한_일곱_가지가_모두_있다() {
    let keys: Vec<&str> = all_rules().iter().map(|r| r.key()).collect();
    for want in [
        "SAME_GRADE",
        "FEWEST_TODAY",
        "FEWEST_MONTH",
        "FEWEST_TOTAL",
        "PREFER_SPECIAL",
        "PREFER_FINISHED",
        "PREFER_LOWER_GRADE",
    ] {
        assert!(keys.contains(&want), "{want} 기준이 있어야 한다");
        assert!(is_known_key(want));
    }
    assert_eq!(DEFAULT_ORDER.len(), keys.len(), "기본 순서에 모든 기준이 있어야 한다");
}

#[test]
fn 기준마다_이름과_설명이_있다() {
    for r in all_rules() {
        assert!(!r.label().is_empty(), "{}", r.key());
        assert!(!r.hint().is_empty(), "{}", r.key());
    }
}
