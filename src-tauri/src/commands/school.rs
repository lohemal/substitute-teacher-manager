//! 학교 기본 설정 (설정 마법사 1단계).

use tauri::State;

use crate::error::AppResult;
use crate::repo::school::{self, SchoolInput, SchoolView};
use crate::repo::setup;
use crate::AppState;

const STEP: &str = "SCHOOL";

#[tauri::command]
pub fn school_get(state: State<'_, AppState>) -> AppResult<Option<SchoolView>> {
    state.db.read(school::get)
}

/// 학교 정보 + 학년도/학기 + 학급을 저장하고 1단계를 완료 처리한다.
#[tauri::command]
pub fn school_save(state: State<'_, AppState>, input: SchoolInput) -> AppResult<SchoolView> {
    state.db.write(|c| {
        school::save(c, &input)?;
        setup::set_status(c, STEP, setup::STATUS_DONE)?;
        setup::clear_draft(c, STEP)?;
        school::get(c)?.ok_or_else(|| {
            crate::error::AppError::internal("학교 정보를 저장한 뒤 다시 읽지 못했습니다.")
        })
    })
}
