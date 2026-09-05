//! 보결이 한쪽으로 쏠렸는지 **참고로** 살펴보는 계산. 순수 계산 계층.
//!
//! ## 판단하지 않는다
//!
//! 횟수가 많다고 잘못된 것이 아니다. 담임은 자기 반 수업 때문에 들어갈 수 있는
//! 시간이 적고, 전담은 공강이 많아 자연히 횟수가 늘어난다. 그래서
//!
//! - 비교는 **같은 구분(담임·전담·기타)끼리만** 한다.
//! - 결과는 `평균보다 많음 / 평균 수준 / 평균보다 적음` 세 가지 참고 표시로만 낸다.
//! - '과다', '문제' 같은 판정 문구는 쓰지 않는다.

use serde::Serialize;

pub const BAND_MORE: &str = "MORE";
pub const BAND_TYPICAL: &str = "TYPICAL";
pub const BAND_LESS: &str = "LESS";
/// 견줄 사람이 너무 적어 말할 수 없는 경우
pub const BAND_NONE: &str = "NONE";

pub fn band_label(band: &str) -> &'static str {
    match band {
        BAND_MORE => "평균보다 많음",
        BAND_TYPICAL => "평균 수준",
        BAND_LESS => "평균보다 적음",
        _ => "—",
    }
}

/// 이 구분(담임·전담·기타) 안에서의 분포. 서로 다른 구분끼리는 견주지 않는다.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoleSpread {
    pub role_code: String,
    pub role_label: String,
    /// 보결 대상인 사람 수
    pub people: i32,
    pub total: i32,
    pub avg: f64,
    pub max: i32,
    pub min: i32,
    /// 최다 − 최소
    pub spread: i32,
    /// 가장 많이 맡은 사람 (동률이면 이름 순 첫 사람)
    pub max_name: Option<String>,
    pub min_name: Option<String>,
    /// 참고 표시를 낼 만큼 사람이 있는가 (3명 이상)
    pub comparable: bool,
}

/// 한 사람의 몫.
#[derive(Debug, Clone)]
pub struct Share {
    pub teacher_id: i64,
    pub name: String,
    pub role_code: String,
    pub role_label: String,
    pub count: i32,
}

/// 견줄 만한 최소 인원. 두 명뿐이면 한 명은 반드시 '많음'이 되어 뜻이 없다.
const MIN_PEOPLE: usize = 3;

/// 평균에서 이만큼 벗어나야 '많음/적음'으로 본다.
///
/// 한두 번 차이는 그날그날의 사정일 뿐이다. 학기 초처럼 전체 횟수가 적을 때
/// 1회 차이로 표시가 붙으면 뜻 없는 소음이 되므로 최소 2회는 봐준다.
/// 횟수가 쌓이면 2회의 무게가 작아지므로 평균의 25%도 함께 본다.
fn tolerance(avg: f64) -> f64 {
    (avg * 0.25).max(2.0)
}

/// 구분별 분포를 낸다. 구분 안에 사람이 적으면 `comparable = false`가 된다.
pub fn spreads(shares: &[Share]) -> Vec<RoleSpread> {
    let mut order: Vec<(String, String)> = Vec::new();
    for s in shares {
        if !order.iter().any(|(c, _)| c == &s.role_code) {
            order.push((s.role_code.clone(), s.role_label.clone()));
        }
    }

    order
        .into_iter()
        .map(|(code, label)| {
            let mut group: Vec<&Share> = shares.iter().filter(|s| s.role_code == code).collect();
            group.sort_by(|a, b| a.name.cmp(&b.name));

            let people = group.len();
            let total: i32 = group.iter().map(|s| s.count).sum();
            let avg = if people == 0 {
                0.0
            } else {
                total as f64 / people as f64
            };
            let max = group.iter().map(|s| s.count).max().unwrap_or(0);
            let min = group.iter().map(|s| s.count).min().unwrap_or(0);

            RoleSpread {
                role_code: code,
                role_label: label,
                people: people as i32,
                total,
                avg,
                max,
                min,
                spread: max - min,
                max_name: group
                    .iter()
                    .find(|s| s.count == max)
                    .map(|s| s.name.clone()),
                min_name: group
                    .iter()
                    .find(|s| s.count == min)
                    .map(|s| s.name.clone()),
                comparable: people >= MIN_PEOPLE && max > min,
            }
        })
        .collect()
}

/// 한 사람이 자기 구분 안에서 어디쯤인지. 참고 표시일 뿐이다.
pub fn band_of(share: &Share, spreads: &[RoleSpread]) -> String {
    let Some(sp) = spreads.iter().find(|s| s.role_code == share.role_code) else {
        return BAND_NONE.to_string();
    };
    if !sp.comparable {
        return BAND_NONE.to_string();
    }
    let t = tolerance(sp.avg);
    let x = share.count as f64;
    if x > sp.avg + t {
        BAND_MORE.to_string()
    } else if x < sp.avg - t {
        BAND_LESS.to_string()
    } else {
        BAND_TYPICAL.to_string()
    }
}

/// 화면에 적어 둘 계산 방법 설명. 사용자가 숫자를 믿을 수 있어야 한다.
pub const HOW_TEXT: &str =
    "같은 구분(담임·전담·기타)끼리만 견줍니다. 평균에서 2회 또는 평균의 25% 가운데 \
     큰 폭보다 벗어나면 '평균보다 많음/적음'으로 표시합니다. 구분 안에 3명 미만이거나 \
     모두 같은 횟수면 표시하지 않습니다. 역할에 따라 들어갈 수 있는 시간이 다르므로 \
     참고용으로만 보아 주세요.";

#[cfg(test)]
#[path = "fairness_tests.rs"]
mod tests;
