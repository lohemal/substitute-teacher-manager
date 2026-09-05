//! 추천 기준 설정 명령.

use tauri::State;

use crate::error::AppResult;
use crate::repo::priority::{self, PriorityView, RuleInput};
use crate::repo::setup;
use crate::AppState;

const STEP: &str = "PRIORITY";

#[tauri::command]
pub fn priority_list(state: State<'_, AppState>) -> AppResult<PriorityView> {
    state.db.read(priority::view)
}

#[tauri::command]
pub fn priority_save(
    state: State<'_, AppState>,
    rules: Vec<RuleInput>,
) -> AppResult<PriorityView> {
    state.db.write(|c| {
        priority::save(c, &rules)?;
        setup::touch_in_progress(c, STEP)?;
        priority::view(c)
    })
}

#[tauri::command]
pub fn priority_reset(state: State<'_, AppState>) -> AppResult<PriorityView> {
    state.db.write(|c| {
        priority::reset(c)?;
        priority::view(c)
    })
}

/// 설정 마법사 5단계 완료 처리. 켜 둔 기준이 하나라도 있으면 통과한다.
#[tauri::command]
pub fn priority_finish_step(state: State<'_, AppState>) -> AppResult<Vec<String>> {
    state.db.write(|c| {
        let v = priority::view(c)?;
        let mut problems = Vec::new();
        if !v.rules.iter().any(|r| r.enabled) {
            problems.push(
                "추천 기준을 하나 이상 켜 주세요. 모두 끄면 추천 순서를 정할 수 없습니다."
                    .to_string(),
            );
        }
        if problems.is_empty() {
            setup::set_status(c, STEP, setup::STATUS_DONE)?;
        }
        Ok(problems)
    })
}
