//! 초기 설정 마법사 관련 명령.

use tauri::State;

use crate::error::{AppError, AppResult};
use crate::repo::setup::{self, SetupState};
use crate::AppState;

const STEP_KEYS: [&str; 6] = ["SCHOOL", "BELL", "TEACHER", "LESSON", "PRIORITY", "DONE"];

fn check_step(step_key: &str) -> AppResult<()> {
    if STEP_KEYS.contains(&step_key) {
        Ok(())
    } else {
        Err(AppError::invalid("알 수 없는 설정 단계입니다."))
    }
}

#[tauri::command]
pub fn setup_get_state(state: State<'_, AppState>) -> AppResult<SetupState> {
    state.db.read(setup::get_state)
}

/// 입력 도중 자동 저장. 화면에서 잠깐 멈출 때마다 호출된다.
#[tauri::command]
pub fn setup_save_draft(
    state: State<'_, AppState>,
    step_key: String,
    value_json: String,
) -> AppResult<()> {
    check_step(&step_key)?;
    state.db.write(|c| {
        setup::save_draft(c, &step_key, &value_json)?;
        setup::touch_in_progress(c, &step_key)?;
        Ok(())
    })
}

#[tauri::command]
pub fn setup_get_draft(state: State<'_, AppState>, step_key: String) -> AppResult<Option<String>> {
    check_step(&step_key)?;
    state.db.read(|c| setup::get_draft(c, &step_key))
}

/// 아직 준비되지 않았거나 우리 학교에 필요 없는 단계를 건너뛴다.
#[tauri::command]
pub fn setup_skip_step(state: State<'_, AppState>, step_key: String) -> AppResult<SetupState> {
    check_step(&step_key)?;
    state.db.write(|c| {
        setup::set_status(c, &step_key, setup::STATUS_SKIPPED)?;
        setup::get_state(c)
    })
}

/// 초기 설정을 마친다. 이후 프로그램 시작 화면은 '보결 조회'가 된다.
#[tauri::command]
pub fn setup_complete(state: State<'_, AppState>) -> AppResult<SetupState> {
    state.db.write(|c| {
        setup::set_status(c, "DONE", setup::STATUS_DONE)?;
        setup::mark_completed(c)?;
        setup::get_state(c)
    })
}

/// 처음 단계부터 다시 확인한다. **입력한 자료는 지우지 않는다.**
#[tauri::command]
pub fn setup_restart(state: State<'_, AppState>) -> AppResult<SetupState> {
    state.db.write(|c| {
        setup::restart(c)?;
        setup::get_state(c)
    })
}
