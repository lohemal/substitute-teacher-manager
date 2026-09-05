//! 보결 판단 로직. **DB와 Tauri를 모르는 순수 계산 계층.**
//!
//! `repo`가 필요한 자료를 모두 읽어 구조체로 넘겨 주고, 여기서는 그것만 보고
//! 결과를 낸다. 그래서 DB 없이 시험할 수 있고, 같은 입력이면 항상 같은 결과가 나온다.
//!
//! - `time`     : 시간 구간과 겹침 판단 (프로그램 전체에서 이 한 곳만 쓴다)
//! - `schedule` : 하루치 자료 -> 교사별 바쁜 시간 구간
//! - `find`     : STEP 1 후보 걸러내기 (가능/불가능 판정)
//! - `priority` : STEP 2 학교별 기준으로 추천 순서 매기기
//! - `assign`   : STEP 3 배정 직전 재검증 · 하루 일괄 보결 대상 찾기
//! - `period`   : 오늘/주/월/학기를 실제 날짜 범위로
//! - `fairness` : 보결이 쏠렸는지 참고로 살펴보기 (판정하지 않는다)

pub mod assign;
pub mod fairness;
pub mod find;
pub mod period;
pub mod priority;
pub mod schedule;
pub mod time;

#[cfg(test)]
#[path = "assign_tests.rs"]
mod assign_tests;

#[cfg(test)]
#[path = "find_tests.rs"]
mod find_tests;

#[cfg(test)]
#[path = "priority_tests.rs"]
mod priority_tests;
