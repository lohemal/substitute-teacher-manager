//! 동학년군 교사 우선 — 요청서의 Case A~L.
//!
//! ## 확인하려는 것
//!
//! 초등학교에서는 1·2 / 3·4 / 5·6 학년이 한 학년군이다. 같은 학년 선생님
//! 다음으로 사정을 잘 아는 것이 짝 학년 선생님이라, 그 다음 순서로 추천할
//! 수 있어야 한다.
//!
//! **후보 자격과는 상관이 없다.** 이것은 이미 배정 가능하다고 판정된
//! 사람들 사이의 순서만 정한다. 짝 학년이라고 후보로 만들지 않고, 짝
//! 학년이 아니라고 후보에서 빼지도 않는다 (Case J).
//!
//! ## 동학년과 겹치지 않는다
//!
//! 6학년 보결에서 6학년 담임은 **동학년**이지 동학년군이 아니다. 둘 다
//! 해당된다고 하면 학교가 두 기준의 순서를 따로 정하는 일이 뜻을 잃는다.

use super::find::*;
use super::grade::paired_grade;
use super::priority::*;

fn cand(
    id: i64,
    name: &str,
    role: &str,
    homeroom_grades: Vec<i32>,
    today: i32,
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
        counts: SubCounts {
            today,
            month: 0,
            total: 0,
        },
        homeroom_grades,
        blocks: vec![],
    }
}

/// 담임 한 사람. 이름은 바로 알아볼 수 있게 붙인다.
fn hr(id: i64, name: &str, grade: i32) -> Candidate {
    cand(id, name, "HOMEROOM", vec![grade], 0, ELIGIBLE_HOMEROOM_FREE)
}

fn hr_today(id: i64, name: &str, grade: i32, today: i32) -> Candidate {
    cand(
        id,
        name,
        "HOMEROOM",
        vec![grade],
        today,
        ELIGIBLE_HOMEROOM_FREE,
    )
}

/// 기준 목록. 적은 것만 켜진다.
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

/// 동학년 → 동학년군 순서로 정렬한 결과
fn ranked(mut list: Vec<Candidate>, target_grade: i32) -> Vec<Candidate> {
    rank_candidates(
        &mut list,
        &order(&["SAME_GRADE", "SAME_GRADE_BAND"]),
        target_grade,
    );
    list
}

/// 그 후보에게 붙은 근거 문구들
fn tags_of(list: &[Candidate], name: &str) -> Vec<String> {
    list.iter()
        .find(|c| c.name == name)
        .unwrap_or_else(|| panic!("{name} 가 목록에 있어야 한다"))
        .reason
        .split(" · ")
        .map(|s| s.to_string())
        .collect()
}

// ============================================================
//  Case A~E — 여섯 학년 모두 짝이 제자리에 온다
// ============================================================

#[test]
fn case_a_6학년_보결은_6학년_다음_5학년() {
    let r = ranked(
        vec![hr(3, "C 4학년", 4), hr(2, "B 5학년", 5), hr(1, "A 6학년", 6)],
        6,
    );
    assert_eq!(names(&r), ["A 6학년", "B 5학년", "C 4학년"]);
}

#[test]
fn case_b_5학년_보결은_5학년_다음_6학년() {
    let r = ranked(
        vec![hr(3, "C 4학년", 4), hr(2, "B 6학년", 6), hr(1, "A 5학년", 5)],
        5,
    );
    assert_eq!(names(&r), ["A 5학년", "B 6학년", "C 4학년"]);
}

#[test]
fn case_c_1학년_보결은_1학년_다음_2학년() {
    let r = ranked(
        vec![hr(3, "C 3학년", 3), hr(2, "B 2학년", 2), hr(1, "A 1학년", 1)],
        1,
    );
    assert_eq!(names(&r), ["A 1학년", "B 2학년", "C 3학년"]);
}

#[test]
fn case_d_2학년_보결은_2학년_다음_1학년() {
    let r = ranked(
        vec![hr(3, "C 3학년", 3), hr(2, "B 1학년", 1), hr(1, "A 2학년", 2)],
        2,
    );
    assert_eq!(names(&r), ["A 2학년", "B 1학년", "C 3학년"]);
}

#[test]
fn case_e_3학년과_4학년은_양방향으로_짝이다() {
    // 3학년 보결 → 3학년, 그다음 4학년
    let r = ranked(
        vec![hr(3, "C 5학년", 5), hr(2, "B 4학년", 4), hr(1, "A 3학년", 3)],
        3,
    );
    assert_eq!(names(&r), ["A 3학년", "B 4학년", "C 5학년"]);

    // 4학년 보결 → 4학년, 그다음 3학년
    let r = ranked(
        vec![hr(3, "C 5학년", 5), hr(2, "B 3학년", 3), hr(1, "A 4학년", 4)],
        4,
    );
    assert_eq!(names(&r), ["A 4학년", "B 3학년", "C 5학년"]);
}

/// 여섯 학년 전부를 한 번에. 대상 학년 → 짝 학년 → 나머지.
#[test]
fn 여섯_학년_모두_같은_방식으로_동작한다() {
    for target in 1..=6 {
        let pair = paired_grade(target).unwrap();
        let list: Vec<Candidate> = (1..=6)
            .map(|g| hr(g as i64, &format!("{g}학년 담임"), g))
            .collect();
        let r = ranked(list, target);

        assert_eq!(
            r[0].name,
            format!("{target}학년 담임"),
            "{target}학년 보결의 1순위는 동학년"
        );
        assert_eq!(
            r[1].name,
            format!("{pair}학년 담임"),
            "{target}학년 보결의 2순위는 짝 학년 {pair}"
        );
    }
}

// ============================================================
//  Case F — 동학년군이 동학년까지 끌어안으면 안 된다
// ============================================================

#[test]
fn case_f_동학년을_끄고_동학년군만_켜도_같은_학년은_혜택이_없다() {
    let mut list = vec![hr(1, "A 6학년", 6), hr(2, "B 5학년", 5), hr(3, "C 4학년", 4)];
    rank_candidates(&mut list, &order(&["SAME_GRADE_BAND"]), 6);

    assert_eq!(list[0].name, "B 5학년", "짝 학년만 앞으로 온다");

    // 6학년 담임은 동학년군으로 판정되지 않는다
    assert!(
        !list
            .iter()
            .find(|c| c.name == "A 6학년")
            .unwrap()
            .reason
            .contains("동학년군"),
        "동학년군 기준이 같은 학년까지 포괄하면 안 된다"
    );
    // 4학년 담임도 마찬가지
    assert!(!tags_of(&list, "C 4학년").iter().any(|t| t == "동학년군"));
}

// ============================================================
//  Case G — 순서는 학교가 정한다
// ============================================================

#[test]
fn case_g_기준_순서를_바꾸면_결과가_바뀐다() {
    let make = || {
        vec![
            hr_today(1, "A 6학년", 6, 3), // 동학년이지만 오늘 3회
            hr_today(2, "B 5학년", 5, 0), // 짝 학년, 오늘 0회
        ]
    };

    // 학년 관계를 먼저 보면 A 가 앞
    let mut list = make();
    rank_candidates(
        &mut list,
        &order(&["SAME_GRADE", "SAME_GRADE_BAND", "FEWEST_TODAY"]),
        6,
    );
    assert_eq!(names(&list), ["A 6학년", "B 5학년"]);

    // 횟수를 먼저 보면 B 가 앞
    let mut list = make();
    rank_candidates(
        &mut list,
        &order(&["FEWEST_TODAY", "SAME_GRADE", "SAME_GRADE_BAND"]),
        6,
    );
    assert_eq!(names(&list), ["B 5학년", "A 6학년"]);
}

/// 새 기준이 목록 어디에 있어도 제 일을 한다.
#[test]
fn 다른_기준들_사이_어디에_놓아도_동작한다() {
    let all = [
        "SAME_GRADE",
        "FEWEST_TODAY",
        "FEWEST_MONTH",
        "FEWEST_TOTAL",
        "PREFER_SPECIAL",
        "PREFER_FINISHED",
        "PREFER_LOWER_GRADE",
    ];

    // 다른 조건은 모두 같고 학년만 다른 두 담임
    for pos in 0..=all.len() {
        let mut keys: Vec<&str> = all.to_vec();
        keys.insert(pos, "SAME_GRADE_BAND");

        let mut list = vec![hr(3, "C 4학년", 4), hr(2, "B 5학년", 5)];
        rank_candidates(&mut list, &order(&keys), 6);

        // PREFER_LOWER_GRADE 가 SAME_GRADE_BAND 보다 앞이면 4학년이 이긴다
        let band_first = pos <= all.iter().position(|k| *k == "PREFER_LOWER_GRADE").unwrap();
        let want = if band_first { "B 5학년" } else { "C 4학년" };
        assert_eq!(
            list[0].name, want,
            "SAME_GRADE_BAND 를 {pos}번째에 넣었을 때"
        );
    }
}

// ============================================================
//  Case H · I — 담임 학년이 없는 선생님
// ============================================================

#[test]
fn case_h_전담교사는_동학년도_동학년군도_아니다() {
    let sp = cand(9, "김전담", "SPECIAL", vec![], 0, ELIGIBLE_SPECIALIST_FREE);
    let mut list = vec![sp, hr(2, "B 5학년", 5), hr(1, "A 6학년", 6)];
    rank_candidates(&mut list, &order(&["SAME_GRADE", "SAME_GRADE_BAND"]), 6);

    assert_eq!(names(&list), ["A 6학년", "B 5학년", "김전담"]);
    let t = tags_of(&list, "김전담");
    assert!(!t.iter().any(|x| x == "동학년"), "{t:?}");
    assert!(!t.iter().any(|x| x == "동학년군"), "{t:?}");
}

#[test]
fn case_i_기타_교사도_동학년군이_아니다() {
    let ot = cand(9, "박기타", "OTHER", vec![], 0, ELIGIBLE_FREE);
    let mut list = vec![ot, hr(2, "B 5학년", 5)];
    rank_candidates(&mut list, &order(&["SAME_GRADE_BAND"]), 6);

    assert_eq!(list[0].name, "B 5학년");
    assert!(!tags_of(&list, "박기타").iter().any(|x| x == "동학년군"));
}

/// 역할 코드가 아니라 **담임 학년이 있는가**로 가른다.
///
/// 지금 자료 모델에서는 전담·기타에게도 담임 학급을 줄 수 있다. 그런
/// 사람은 실제로 그 학년 담임이므로 학년군 판정을 받아야 한다.
#[test]
fn 담임_학급이_있으면_역할이_전담이어도_학년으로_본다() {
    let sp_with_class = cand(
        9,
        "김전담",
        "SPECIAL",
        vec![5],
        0,
        ELIGIBLE_SPECIALIST_FREE,
    );
    let mut list = vec![hr(3, "C 4학년", 4), sp_with_class];
    rank_candidates(&mut list, &order(&["SAME_GRADE_BAND"]), 6);

    assert_eq!(list[0].name, "김전담", "5학년을 맡았으므로 6학년의 짝이다");
    assert!(tags_of(&list, "김전담").iter().any(|x| x == "동학년군"));
}

// ============================================================
//  Case K — 근거 문구는 하나만
// ============================================================

#[test]
fn case_k_동학년과_동학년군은_한_사람에게_함께_붙지_않는다() {
    let r = ranked(vec![hr(1, "A 6학년", 6), hr(2, "B 5학년", 5)], 6);

    assert_eq!(tags_of(&r, "A 6학년"), ["동학년"]);
    assert_eq!(tags_of(&r, "B 5학년"), ["동학년군"]);
}

/// 두 학년을 함께 맡은 담임은 **가까운 쪽**으로 간다.
#[test]
fn 오륙학년을_함께_맡은_담임은_동학년으로만_본다() {
    let both = cand(
        1,
        "두학년 담임",
        "HOMEROOM",
        vec![5, 6],
        0,
        ELIGIBLE_HOMEROOM_FREE,
    );
    let r = ranked(vec![both, hr(2, "B 5학년", 5)], 6);

    assert_eq!(tags_of(&r, "두학년 담임"), ["동학년"]);
    assert_eq!(r[0].name, "두학년 담임");
}

// ============================================================
//  Case L — 짝 학년 표
// ============================================================

#[test]
fn case_l_짝_학년_표가_요청서와_같다() {
    for (g, want) in [(1, 2), (2, 1), (3, 4), (4, 3), (5, 6), (6, 5)] {
        assert_eq!(paired_grade(g), Some(want), "{g}학년");

        // 정렬에서도 그 짝만 혜택을 받는다
        let list: Vec<Candidate> = (1..=6)
            .filter(|x| *x != g)
            .map(|x| hr(x as i64, &format!("{x}학년"), x))
            .collect();
        let mut list = list;
        rank_candidates(&mut list, &order(&["SAME_GRADE_BAND"]), g);
        assert_eq!(list[0].name, format!("{want}학년"), "{g}학년 보결");
        for c in &list[1..] {
            assert!(
                !c.reason.contains("동학년군"),
                "{g}학년 보결에서 {} 가 학년군으로 잡혔다",
                c.name
            );
        }
    }
}

#[test]
fn 학년_범위를_벗어나도_안전하다() {
    for target in [0, 7, 99, -3] {
        let mut list = vec![hr(1, "A 1학년", 1), hr(2, "B 6학년", 6)];
        rank_candidates(&mut list, &order(&["SAME_GRADE_BAND"]), target);
        // 짝이 없으므로 아무도 혜택을 받지 않는다 (동점 처리로만 갈린다)
        for c in &list {
            assert!(!c.reason.contains("동학년군"), "{target}학년 보결");
        }
    }
}

// ============================================================
//  설정 등록 상태
// ============================================================

#[test]
fn 새_기준이_등록되어_있고_기본_자리는_동학년_다음이다() {
    assert!(is_known_key("SAME_GRADE_BAND"));

    let keys: Vec<&str> = DEFAULT_ORDER.iter().map(|(k, _)| *k).collect();
    let i = keys.iter().position(|k| *k == "SAME_GRADE").unwrap();
    assert_eq!(keys[i + 1], "SAME_GRADE_BAND", "동학년 바로 다음 자리");

    // 기본은 꺼짐 — 학교가 켜야 순서에 끼어든다
    let on = DEFAULT_ORDER
        .iter()
        .find(|(k, _)| *k == "SAME_GRADE_BAND")
        .unwrap()
        .1;
    assert!(!on, "기본값은 꺼짐이어야 한다");

    // 기존 기준의 뜻은 그대로다
    assert!(is_known_key("SAME_GRADE"));
}
