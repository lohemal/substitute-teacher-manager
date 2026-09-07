//! 보결 수당 계산. **DB와 Tauri를 모르는 순수 계산 계층.**
//!
//! ## 기본 계산식
//!
//! ```text
//! 지급 인정 보결 횟수 × 1회 보결 수당 = 지급액
//! ```
//!
//! ## 계산 근거는 배정 기록뿐이다
//!
//! `repo`가 기간 안의 실제 배정 기록(`substitutions`, `status='ASSIGNED'`)만
//! 골라 `Tally`로 넘겨 준다. 수당 때문에 배정 기록을 고치거나, 수당용 횟수를
//! 따로 저장하는 일은 없다. 그래서 배정을 취소하거나 1회 수당을 바꾸면 다음
//! 조회에서 곧바로 반영된다.
//!
//! ## 정책은 코드로 늘린다
//!
//! 학교마다 지급 규정이 다르므로 `PayRule` 구현체 하나가 정책 하나다.
//! 새 정책을 추가하려면 구현체를 만들어 `all_policies()`에 등록하면 된다.
//! **DB 구조도, 화면도 손대지 않는다** (DB에는 정책 코드 문자열만 저장한다).
//!
//! 앞으로 '월 최대 지급 횟수', '1일 최대 지급 횟수', '특정 보결 유형 제외'
//! 같은 규정이 필요할 수 있다. 그래서 정책에게 횟수만 주지 않고 **사례 목록
//! 전체**(`Tally::cases`)를 넘긴다. 그런 규정은 날짜와 유형을 봐야 계산할 수
//! 있기 때문이다. 다만 지금 요청하지 않은 규칙은 만들어 두지 않는다.

use serde::Serialize;

pub const ALL_ASSIGNED: &str = "ALL_ASSIGNED";
pub const DEDUCT_OWN_CAUSED: &str = "DEDUCT_OWN_CAUSED";

/// 정책이 정해지지 않았을 때 쓰는 값. 아무것도 깎지 않는 쪽이 안전하다.
pub const DEFAULT_POLICY: &str = ALL_ASSIGNED;
pub const DEFAULT_PER_CASE: i64 = 0;

// ============================================================
//  정책이 볼 수 있는 것
// ============================================================

/// 보결 한 건. 앞으로 날짜·유형을 보는 정책이 생길 수 있어 함께 담는다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Case {
    /// YYYY-MM-DD
    pub date: String,
    /// PERIOD | LUNCH — 점심 보결도 정식 배정이면 한 건이다
    pub slot_type: String,
    pub start_min: i32,
}

#[cfg(test)]
impl Case {
    /// 시험에서 쓰기 좋은 짧은 생성자. 실제 자료는 `repo::pay` 가 SQL 에서 만든다.
    pub fn period(date: &str, start_min: i32) -> Self {
        Self {
            date: date.to_string(),
            slot_type: "PERIOD".into(),
            start_min,
        }
    }
    pub fn lunch(date: &str, start_min: i32) -> Self {
        Self {
            date: date.to_string(),
            slot_type: "LUNCH".into(),
            start_min,
        }
    }
}

/// 한 교사의 기간 안 집계.
#[derive(Debug, Clone, Default)]
pub struct Tally {
    /// 이 선생님이 **다른 사람을 대신해** 실제로 들어간 건
    pub cases: Vec<Case>,
    /// 이 선생님의 결근 때문에 **다른 사람이 실제로 들어간** 건.
    /// 결근했지만 아무도 배정되지 않은 시간은 여기에 들어오지 않는다.
    pub own_caused: Vec<Case>,
}

impl Tally {
    pub fn substituted(&self) -> i32 {
        self.cases.len() as i32
    }
    pub fn own_caused_count(&self) -> i32 {
        self.own_caused.len() as i32
    }
}

// ============================================================
//  정책 하나가 알아야 하는 것
// ============================================================

pub trait PayRule: Send + Sync {
    fn code(&self) -> &'static str;
    /// 설정 화면에 보일 이름
    fn label(&self) -> &'static str;
    /// 한 줄 설명
    fn hint(&self) -> &'static str;

    /// 지급 인정 횟수. **음수를 돌려주지 않는다.**
    fn payable(&self, t: &Tally) -> i32;

    /// 상세 화면에 보여 줄 계산 설명. 금액 줄은 부르는 쪽에서 붙인다.
    fn steps(&self, t: &Tally) -> Vec<String>;
}

/// 실제 보결 횟수 전체 지급.
struct AllAssigned;
impl PayRule for AllAssigned {
    fn code(&self) -> &'static str {
        ALL_ASSIGNED
    }
    fn label(&self) -> &'static str {
        "실제 보결 횟수 전체 지급"
    }
    fn hint(&self) -> &'static str {
        "기간 안에 다른 선생님을 대신해 들어간 횟수를 모두 지급 인정합니다."
    }
    fn payable(&self, t: &Tally) -> i32 {
        t.substituted()
    }
    fn steps(&self, t: &Tally) -> Vec<String> {
        vec![format!("보결 {}회 전체 인정", t.substituted())]
    }
}

/// 본인으로 인해 발생한 보결 횟수 차감.
struct DeductOwnCaused;
impl PayRule for DeductOwnCaused {
    fn code(&self) -> &'static str {
        DEDUCT_OWN_CAUSED
    }
    fn label(&self) -> &'static str {
        "본인으로 인해 발생한 보결 횟수 차감"
    }
    fn hint(&self) -> &'static str {
        "보결한 횟수에서, 본인이 결근해 다른 선생님이 대신 들어간 횟수를 뺍니다. \
         결근했지만 아무도 배정되지 않은 시간은 차감하지 않습니다."
    }
    fn payable(&self, t: &Tally) -> i32 {
        // 음수가 될 수 없다 — 수당을 되돌려 받는 일은 없다
        (t.substituted() - t.own_caused_count()).max(0)
    }
    fn steps(&self, t: &Tally) -> Vec<String> {
        let got = t.substituted();
        let own = t.own_caused_count();
        let raw = got - own;
        let mut out = vec![format!(
            "{got}회 - {own}회 = 지급 인정 {}회",
            raw.max(0)
        )];
        if raw < 0 {
            out.push(format!(
                "차감 결과가 {raw}회이지만 지급 인정 횟수는 0회보다 작아지지 않습니다."
            ));
        }
        out
    }
}

/// 쓸 수 있는 정책 전체. **새 정책은 여기에만 추가한다.**
pub fn all_policies() -> Vec<Box<dyn PayRule>> {
    vec![Box::new(AllAssigned), Box::new(DeductOwnCaused)]
}

/// 저장된 코드에 해당하는 정책. 알 수 없는 코드면 기본 정책으로 되돌린다 —
/// 화면이 빈 채로 멈추거나 금액이 0으로 보이는 것보다 낫다.
pub fn rule_of(code: &str) -> Box<dyn PayRule> {
    all_policies()
        .into_iter()
        .find(|r| r.code() == code)
        .unwrap_or_else(|| Box::new(AllAssigned))
}

pub fn is_known_policy(code: &str) -> bool {
    all_policies().iter().any(|r| r.code() == code)
}

// ============================================================
//  설정과 결과
// ============================================================

#[derive(Debug, Clone)]
pub struct PayConfig {
    pub policy: String,
    /// 1회 보결 수당(원). 0 이상 정수.
    pub per_case: i64,
}

impl Default for PayConfig {
    fn default() -> Self {
        Self {
            policy: DEFAULT_POLICY.to_string(),
            per_case: DEFAULT_PER_CASE,
        }
    }
}

/// 한 사람의 계산 결과.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Settlement {
    /// 다른 사람을 대신해 들어간 횟수
    pub substituted: i32,
    /// 본인 결근으로 다른 사람이 들어간 횟수 (참고정보로도 보여 준다)
    pub own_caused: i32,
    /// 지급 인정 횟수
    pub payable: i32,
    pub per_case: i64,
    pub amount: i64,
}

/// 정책과 1회 수당으로 한 사람의 지급액을 낸다.
pub fn settle(cfg: &PayConfig, t: &Tally) -> Settlement {
    let rule = rule_of(&cfg.policy);
    let per_case = cfg.per_case.max(0);
    let payable = rule.payable(t).max(0);
    Settlement {
        substituted: t.substituted(),
        own_caused: t.own_caused_count(),
        payable,
        per_case,
        amount: payable as i64 * per_case,
    }
}

/// 상세 화면에 그대로 쓰는 계산 설명.
pub fn explain(cfg: &PayConfig, t: &Tally) -> Vec<String> {
    let rule = rule_of(&cfg.policy);
    let s = settle(cfg, t);
    let mut out = rule.steps(t);
    out.push(format!(
        "{}회 × {}원 = {}원",
        s.payable,
        won(s.per_case),
        won(s.amount)
    ));
    out
}

/// 천 단위 쉼표. 화면과 계산 설명이 같은 모양을 쓰도록 여기 한 곳에 둔다.
pub fn won(v: i64) -> String {
    let neg = v < 0;
    let digits = v.abs().to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3 + 1);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    if neg {
        format!("-{out}")
    } else {
        out
    }
}
