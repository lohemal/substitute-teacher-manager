//! Phase 9 — 보결 현황 명령. 모두 읽기 전용이다.

use tauri::State;

use crate::error::AppResult;
use crate::repo::stats as repo;
use crate::AppState;

/// 고른 기간의 현황을 한 번에 돌려준다.
///
/// 통계값을 따로 저장하지 않고 그때그때 다시 세므로, 배정을 취소하거나
/// 고치면 다음 조회에서 곧바로 반영된다.
#[tauri::command]
pub fn stats_view(
    state: State<'_, AppState>,
    query: Option<repo::StatsQuery>,
) -> AppResult<repo::StatsView> {
    let q = query.unwrap_or_default();
    state.db.read(|c| repo::view(c, &q))
}
