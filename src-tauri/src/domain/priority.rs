//! STEP 2 — 학교가 정한 기준으로 추천 순서를 매긴다.
//!
//! ## 구조
//!
//! 기준 하나가 `PriorityRule` 구현체 하나다. 각 기준은
//!   - 두 후보를 비교하는 함수 `compare`
//!   - 그 후보에게 붙일 근거 문구 `tag`
//! 만 알면 된다. 정렬은 **켜져 있는 기준을 순서대로 이어 붙여** 수행한다.
//!
//! 새 기준을 추가하려면 `PriorityRule` 구현체 하나를 만들어 `all_rules()`에
//! 등록하기만 하면 된다. **DB 구조도, 정렬 코드도 손대지 않는다.**
//! (DB에는 어떤 기준을 쓰는지와 순서만 저장한다)
//!
//! ## 마지막 동점 처리
//!
//! 켜 둔 기준으로 끝까지 같으면 아래 순서로 정한다. 같은 자료라면 언제
//! 조회해도 **항상 같은 순서**가 나온다.
//!   1) 누적 보결 적은 순  2) 당일 보결 적은 순
//!   3) 담임 학년·반 오름차순  4) 이름 가나다순  5) 교사 번호

use std::cmp::Ordering;

use serde::{Deserialize, Serialize};

use super::find::{Candidate, ELIGIBLE_AFTER_SCHOOL_END};
use super::schedule::ROLE_SPECIAL;

// ============================================================
//  기준 하나가 알아야 하는 것
// ============================================================

/// 정렬할 때 참고하는 상황.
pub struct PriorityCtx {
    /// 보결이 필요한 학급의 학년
    pub target_grade: i32,
    /// 후보들 중 가장 적은 보결 횟수 (근거 문구에 쓴다)
    pub min_today: i32,
    pub min_month: i32,
    pub min_total: i32,
}

pub trait PriorityRule: Send + Sync {
    fn key(&self) -> &'static str;
    /// 설정 화면에 보일 이름
    fn label(&self) -> &'static str;
    /// 한 줄 설명
    fn hint(&self) -> &'static str;

    /// `Less` 면 a가 더 앞순위.
    fn compare(&self, a: &Candidate, b: &Candidate, ctx: &PriorityCtx) -> Ordering;

    /// 이 후보에게 붙일 근거 문구. 해당 없으면 None.
    fn tag(&self, c: &Candidate, ctx: &PriorityCtx) -> Option<String>;
}

/// 화면에 보여 줄 기준 정보 + 사용 여부.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleSetting {
    pub rule_key: String,
    pub enabled: bool,
    pub sort_order: i32,
}

fn is_homeroom(c: &Candidate) -> bool {
    !c.homeroom_grades.is_empty()
}

/// `true`가 앞으로 오도록.
fn prefer_true(a: bool, b: bool) -> Ordering {
    b.cmp(&a)
}

// ============================================================
//  기본 제공 기준
// ============================================================

struct SameGrade;
impl PriorityRule for SameGrade {
    fn key(&self) -> &'static str {
        "SAME_GRADE"
    }
    fn label(&self) -> &'static str {
        "동학년 교사 우선"
    }
    fn hint(&self) -> &'static str {
        "보결이 필요한 학급과 같은 학년을 맡은 담임을 먼저 추천합니다."
    }
    fn compare(&self, a: &Candidate, b: &Candidate, ctx: &PriorityCtx) -> Ordering {
        prefer_true(
            a.homeroom_grades.contains(&ctx.target_grade),
            b.homeroom_grades.contains(&ctx.target_grade),
        )
    }
    fn tag(&self, c: &Candidate, ctx: &PriorityCtx) -> Option<String> {
        c.homeroom_grades
            .contains(&ctx.target_grade)
            .then(|| "동학년".to_string())
    }
}

struct FewestToday;
impl PriorityRule for FewestToday {
    fn key(&self) -> &'static str {
        "FEWEST_TODAY"
    }
    fn label(&self) -> &'static str {
        "당일 보결 횟수 적은 순"
    }
    fn hint(&self) -> &'static str {
        "오늘 이미 보결을 많이 한 선생님을 뒤로 보냅니다."
    }
    fn compare(&self, a: &Candidate, b: &Candidate, _: &PriorityCtx) -> Ordering {
        a.counts.today.cmp(&b.counts.today)
    }
    fn tag(&self, c: &Candidate, ctx: &PriorityCtx) -> Option<String> {
        (c.counts.today == ctx.min_today).then(|| format!("오늘 {}회", c.counts.today))
    }
}

struct FewestMonth;
impl PriorityRule for FewestMonth {
    fn key(&self) -> &'static str {
        "FEWEST_MONTH"
    }
    fn label(&self) -> &'static str {
        "이번 달 보결 횟수 적은 순"
    }
    fn hint(&self) -> &'static str {
        "한 달 안에서 보결이 고르게 돌아가도록 합니다."
    }
    fn compare(&self, a: &Candidate, b: &Candidate, _: &PriorityCtx) -> Ordering {
        a.counts.month.cmp(&b.counts.month)
    }
    fn tag(&self, c: &Candidate, ctx: &PriorityCtx) -> Option<String> {
        (c.counts.month == ctx.min_month).then(|| format!("이번 달 {}회", c.counts.month))
    }
}

struct FewestTotal;
impl PriorityRule for FewestTotal {
    fn key(&self) -> &'static str {
        "FEWEST_TOTAL"
    }
    fn label(&self) -> &'static str {
        "누적 보결 횟수 적은 순"
    }
    fn hint(&self) -> &'static str {
        "지금까지 보결을 적게 한 선생님을 먼저 추천합니다."
    }
    fn compare(&self, a: &Candidate, b: &Candidate, _: &PriorityCtx) -> Ordering {
        a.counts.total.cmp(&b.counts.total)
    }
    fn tag(&self, c: &Candidate, ctx: &PriorityCtx) -> Option<String> {
        (c.counts.total == ctx.min_total).then(|| "누적 적음".to_string())
    }
}

struct PreferSpecial;
impl PriorityRule for PreferSpecial {
    fn key(&self) -> &'static str {
        "PREFER_SPECIAL"
    }
    fn label(&self) -> &'static str {
        "전담교사 우선"
    }
    fn hint(&self) -> &'static str {
        "공강이 있는 전담교사를 담임보다 먼저 추천합니다."
    }
    fn compare(&self, a: &Candidate, b: &Candidate, _: &PriorityCtx) -> Ordering {
        prefer_true(a.role_code == ROLE_SPECIAL, b.role_code == ROLE_SPECIAL)
    }
    fn tag(&self, c: &Candidate, _: &PriorityCtx) -> Option<String> {
        (c.role_code == ROLE_SPECIAL).then(|| "전담".to_string())
    }
}

struct PreferFinished;
impl PriorityRule for PreferFinished {
    fn key(&self) -> &'static str {
        "PREFER_FINISHED"
    }
    fn label(&self) -> &'static str {
        "수업이 종료된 담임교사 우선"
    }
    fn hint(&self) -> &'static str {
        "그날 수업이 이미 끝난 선생님을 먼저 추천합니다. 담임을 가장 앞에 둡니다."
    }
    fn compare(&self, a: &Candidate, b: &Candidate, _: &PriorityCtx) -> Ordering {
        rank(a).cmp(&rank(b))
    }
    fn tag(&self, c: &Candidate, _: &PriorityCtx) -> Option<String> {
        (c.reason_code == ELIGIBLE_AFTER_SCHOOL_END).then(|| "수업 종료".to_string())
    }
}

/// 수업 종료 담임(0) → 수업 종료(1) → 그 밖(2)
fn rank(c: &Candidate) -> u8 {
    let finished = c.reason_code == ELIGIBLE_AFTER_SCHOOL_END;
    match (finished, is_homeroom(c)) {
        (true, true) => 0,
        (true, false) => 1,
        _ => 2,
    }
}

struct PreferLowerGrade;
impl PriorityRule for PreferLowerGrade {
    fn key(&self) -> &'static str {
        "PREFER_LOWER_GRADE"
    }
    fn label(&self) -> &'static str {
        "저학년 담임교사 우선"
    }
    fn hint(&self) -> &'static str {
        "낮은 학년을 맡은 담임을 먼저 추천합니다. 담임이 아닌 선생님은 뒤로 갑니다."
    }
    fn compare(&self, a: &Candidate, b: &Candidate, _: &PriorityCtx) -> Ordering {
        lowest(a).cmp(&lowest(b))
    }
    fn tag(&self, c: &Candidate, _: &PriorityCtx) -> Option<String> {
        c.homeroom_grades
            .iter()
            .min()
            .map(|g| format!("{g}학년 담임"))
    }
}

/// 담임이 아니면 아주 큰 값 (뒤로 간다)
fn lowest(c: &Candidate) -> i32 {
    c.homeroom_grades.iter().min().copied().unwrap_or(99)
}

/// 등록된 모든 기준. **새 기준은 여기에만 추가하면 된다.**
pub fn all_rules() -> Vec<Box<dyn PriorityRule>> {
    vec![
        Box::new(SameGrade),
        Box::new(FewestToday),
        Box::new(FewestMonth),
        Box::new(FewestTotal),
        Box::new(PreferSpecial),
        Box::new(PreferFinished),
        Box::new(PreferLowerGrade),
    ]
}

/// 처음 설치했을 때의 기준 (켜진 것과 순서).
pub const DEFAULT_ORDER: &[(&str, bool)] = &[
    ("SAME_GRADE", true),
    ("FEWEST_TODAY", true),
    ("FEWEST_TOTAL", true),
    ("FEWEST_MONTH", false),
    ("PREFER_SPECIAL", false),
    ("PREFER_FINISHED", false),
    ("PREFER_LOWER_GRADE", false),
];

pub fn is_known_key(key: &str) -> bool {
    all_rules().iter().any(|r| r.key() == key)
}

// ============================================================
//  순서 매기기
// ============================================================

/// 켜져 있는 기준을 순서대로 적용해 후보를 정렬하고, 순위와 근거를 채운다.
pub fn rank_candidates(
    candidates: &mut Vec<Candidate>,
    settings: &[RuleSetting],
    target_grade: i32,
) {
    if candidates.is_empty() {
        return;
    }

    let ctx = PriorityCtx {
        target_grade,
        min_today: candidates.iter().map(|c| c.counts.today).min().unwrap_or(0),
        min_month: candidates.iter().map(|c| c.counts.month).min().unwrap_or(0),
        min_total: candidates.iter().map(|c| c.counts.total).min().unwrap_or(0),
    };

    // 설정된 순서대로, 켜져 있고 우리가 아는 기준만 모은다
    let registry = all_rules();
    let mut order: Vec<&RuleSetting> = settings.iter().filter(|s| s.enabled).collect();
    order.sort_by_key(|s| s.sort_order);

    let active: Vec<&Box<dyn PriorityRule>> = order
        .iter()
        .filter_map(|s| registry.iter().find(|r| r.key() == s.rule_key))
        .collect();

    candidates.sort_by(|a, b| {
        for rule in &active {
            match rule.compare(a, b, &ctx) {
                Ordering::Equal => continue,
                other => return other,
            }
        }
        tie_break(a, b)
    });

    // 순위 · 근거 · 순서를 가른 기준
    let n = candidates.len();
    for i in 0..n {
        let tags: Vec<String> = active
            .iter()
            .filter_map(|r| r.tag(&candidates[i], &ctx))
            .collect();

        // 바로 다음 후보와 순서를 가른 첫 번째 기준
        let decided_by = if i + 1 < n {
            active
                .iter()
                .find(|r| r.compare(&candidates[i], &candidates[i + 1], &ctx) != Ordering::Equal)
                .map(|r| r.label().to_string())
        } else {
            None
        };

        let c = &mut candidates[i];
        c.rank = (i + 1) as i32;
        c.reason = if tags.is_empty() {
            c.status_label.clone()
        } else {
            tags.join(" · ")
        };
        c.decided_by = decided_by;
    }
}

/// 켜 둔 기준으로 끝까지 같을 때. 항상 같은 결과가 나오도록 마지막에 교사 번호까지 본다.
fn tie_break(a: &Candidate, b: &Candidate) -> Ordering {
    a.counts
        .total
        .cmp(&b.counts.total)
        .then(a.counts.today.cmp(&b.counts.today))
        .then(lowest(a).cmp(&lowest(b)))
        .then(
            a.homeroom_grades
                .len()
                .cmp(&b.homeroom_grades.len())
                .reverse(),
        )
        .then(a.name.cmp(&b.name))
        .then(a.teacher_id.cmp(&b.teacher_id))
}

/// 마지막 동점 처리 기준을 사람 말로 (설정 화면에 보여 준다).
pub const TIE_BREAK_TEXT: &str =
    "누적 보결이 적은 순 → 당일 보결이 적은 순 → 담임 학년이 낮은 순 → 이름 순";
