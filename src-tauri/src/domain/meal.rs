//! 전담교사 식사시간. **DB와 Tauri를 모르는 순수 계산 계층.**
//!
//! ## 왜 필요한가
//!
//! 전담교사도 밥을 먹는다. 그런데 그 시간이 학교 전체에 하나로 정해지지
//! 않는다 — 그 날 어느 학년 수업을 맡았느냐에 따라 달라진다.
//!
//! ```text
//!            저학년            고학년
//! 점심     12:10~13:10       13:10~14:00
//! 5교시    13:10~13:50       12:20~13:00
//! ```
//!
//! 고학년 5교시(12:20~13:00)를 하면 저학년 점심과 겹치므로 13:10~14:00 에
//! 먹고, 저학년 5교시(13:10~13:50)를 하면 반대가 된다. 그래서 **학년 그룹을
//! 코드에 박아 두지 않고** 그 날 실제 수업과 시정표의 점심 구간만 본다.
//!
//! ## '점심시간' 과 '점심 보결' 은 다른 말이다
//!
//! 이 모듈이 다루는 것은 **전담교사 식사시간**이다. 학급의 점심 구간
//! (`SLOT_LUNCH`)이나 업무 개념인 '점심 보결'과 헷갈리지 않도록 코드에서도
//! `meal` 이라는 말만 쓴다.
//!
//! ## 일반 수업 보결에만 적용한다
//!
//! 전담교사는 **자기 식사시간에도 점심 보결은 맡을 수 있다.** 그래서 이
//! 제약을 `BusyBlock` 으로 만들지 않는다 — 그렇게 하면 점심 보결까지 막힌다.
//! 판정하는 쪽(`find`)이 대상이 일반 수업인지 점심인지 보고 적용한다.
//!
//! ## 계산해 둔 값을 남기지 않는다
//!
//! 여기서 내는 답은 조회할 때마다 다시 만든다. 시정표나 전담 시간표가 바뀌면
//! 다음 조회에서 곧바로 따라온다.

use super::time::Interval;

/// 식사시간을 무엇으로 정했는가.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MealSource {
    /// 교사별·요일별로 직접 지정했다
    Manual,
    /// 그 날 수업과 겹치지 않는 시간이 하나뿐이라 자동으로 정해졌다
    Auto,
    /// 여러 개가 가능해서 학교 기본 식사시간을 썼다
    SchoolDefault,
}

/// 식사시간을 정하지 못한 까닭.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MealUnknown {
    /// 시정표에 점심 구간이 없다
    NoCandidates,
    /// 가능한 시간이 여럿인데 학교 기본 식사시간이 정해지지 않았다
    NeedsDefault,
    /// 그 날 수업이 모든 점심 구간과 겹친다
    AllBusy,
}

impl MealUnknown {
    pub fn message(&self) -> &'static str {
        match self {
            MealUnknown::NoCandidates => "시정표에 점심시간이 없어 식사시간을 알 수 없습니다",
            MealUnknown::NeedsDefault => {
                "식사할 수 있는 시간이 여러 개입니다. 학교 기본 식사시간을 정해 주세요"
            }
            MealUnknown::AllBusy => {
                "그 날 수업이 모든 점심시간과 겹칩니다. 식사시간을 직접 지정해 주세요"
            }
        }
    }
}

/// 한 교사·한 요일의 식사시간 판정.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Meal {
    Known { interval: Interval, source: MealSource },
    /// 알 수 없다. **이럴 때는 후보에서 빼지 않는다** — 모르면서 빼는 것이
    /// 모르면서 넣는 것보다 나쁘다. 대신 화면에서 알려 준다.
    Unknown(MealUnknown),
}

impl Meal {
    pub fn interval(&self) -> Option<Interval> {
        match self {
            Meal::Known { interval, .. } => Some(*interval),
            Meal::Unknown(_) => None,
        }
    }
}

/// 판정에 필요한 것 전부. 부르는 쪽이 그 날 자료로 채운다.
#[derive(Debug, Clone, Default)]
pub struct MealInput {
    /// 그 요일 학교의 점심 구간들 (같은 시각은 미리 합쳐 온다)
    pub candidates: Vec<Interval>,
    /// 학교 기본 식사시간 (그 요일 기준). 아직 정하지 않았으면 `None`
    pub school_default: Option<Interval>,
    /// 이 교사·이 요일에 직접 지정한 시간
    pub manual: Option<Interval>,
    /// 이 교사가 그 날 실제로 하는 수업 시간
    pub lessons: Vec<Interval>,
}

/// 식사시간을 정한다.
///
/// 순서는 이렇다. 위가 항상 이긴다.
///   1. 직접 지정
///   2. 그 날 수업과 겹치지 않는 시간이 하나뿐이면 그것
///   3. 여러 개면 학교 기본 식사시간 (그것이 수업과 겹치면 가능한 것 중 첫 번째)
///   4. 정할 수 없으면 `Unknown`
///
/// 겹침은 프로그램 전체가 쓰는 규칙 하나(`Interval::overlaps`)를 그대로 쓴다.
/// 그래서 수업 12:20~13:00 과 식사 13:00~14:00 은 겹치지 않는다.
pub fn resolve(input: &MealInput) -> Meal {
    // 1) 직접 지정이 가장 세다
    if let Some(m) = input.manual {
        return Meal::Known {
            interval: m,
            source: MealSource::Manual,
        };
    }

    if input.candidates.is_empty() {
        return Meal::Unknown(MealUnknown::NoCandidates);
    }

    let free: Vec<Interval> = input
        .candidates
        .iter()
        .copied()
        .filter(|c| !input.lessons.iter().any(|l| l.overlaps(c)))
        .collect();

    match free.len() {
        // 3-1) 그 날 수업이 모든 점심시간과 겹친다 — 함부로 고르지 않는다
        0 => Meal::Unknown(MealUnknown::AllBusy),

        // 2) 하나뿐이면 그것
        1 => Meal::Known {
            interval: free[0],
            source: MealSource::Auto,
        },

        // 3) 여러 개면 학교 기본값
        _ => match input.school_default {
            Some(d) if free.contains(&d) => Meal::Known {
                interval: d,
                source: MealSource::SchoolDefault,
            },
            // 기본값이 그 날 수업과 겹친다 — 가능한 것으로 자동 대체
            Some(_) => Meal::Known {
                interval: free[0],
                source: MealSource::Auto,
            },
            // 아직 정하지 않았다. 추측하지 않는다.
            None => Meal::Unknown(MealUnknown::NeedsDefault),
        },
    }
}

/// 직접 지정하려는 시간이 그 날 실제 수업과 겹치는가.
///
/// 겹치면 저장을 막는다. 수동 지정이라고 해서 있을 수 없는 시간까지
/// 받아 줄 이유는 없다.
pub fn manual_conflict(manual: &Interval, lessons: &[Interval]) -> Option<Interval> {
    lessons.iter().copied().find(|l| l.overlaps(manual))
}

/// 같은 시각의 점심 구간을 하나로 합친다. 학년이 달라도 시각이 같으면
/// 식사시간으로서는 같은 자리다.
pub fn dedup_candidates(mut v: Vec<Interval>) -> Vec<Interval> {
    v.sort_by_key(|i| (i.start, i.end));
    v.dedup();
    v
}
