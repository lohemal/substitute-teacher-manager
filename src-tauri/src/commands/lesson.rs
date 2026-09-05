//! 전담교사 시간표 명령.

use tauri::State;

use crate::error::AppResult;
use crate::repo::lesson::{
    self, HomeroomFreeSlot, LessonInput, LessonOverview, LessonRow, TeacherLessons,
};
use crate::repo::setup;
use crate::AppState;

const STEP: &str = "LESSON";

#[tauri::command]
pub fn lesson_overview(state: State<'_, AppState>) -> AppResult<LessonOverview> {
    state.db.read(lesson::overview)
}

#[tauri::command]
pub fn lesson_get(state: State<'_, AppState>, teacher_id: i64) -> AppResult<TeacherLessons> {
    state.db.read(|c| lesson::get(c, teacher_id))
}

#[tauri::command]
pub fn lesson_upsert(state: State<'_, AppState>, input: LessonInput) -> AppResult<TeacherLessons> {
    state.db.write(|c| {
        lesson::upsert(c, &input)?;
        setup::touch_in_progress(c, STEP)?;
        lesson::get(c, input.teacher_id)
    })
}

#[tauri::command]
pub fn lesson_delete(state: State<'_, AppState>, lesson_id: i64) -> AppResult<TeacherLessons> {
    state.db.write(|c| {
        let teacher_id = lesson::delete(c, lesson_id)?;
        lesson::get(c, teacher_id)
    })
}

/// 붙여넣기 미리보기용 검증. 저장하지 않는다.
#[tauri::command]
pub fn lesson_check(state: State<'_, AppState>, rows: Vec<LessonRow>) -> AppResult<Vec<String>> {
    state.db.read(|c| lesson::check(c, &rows))
}

/// 한 교사의 시간표를 통째로 바꾼다. (붙여넣기 반영)
#[tauri::command]
pub fn lesson_bulk_replace(
    state: State<'_, AppState>,
    teacher_id: i64,
    rows: Vec<LessonRow>,
) -> AppResult<TeacherLessons> {
    state.db.write(|c| {
        lesson::bulk_replace(c, teacher_id, &rows)?;
        setup::touch_in_progress(c, STEP)?;
        lesson::get(c, teacher_id)
    })
}

/// 전담 수업 때문에 담임이 비게 되는 시간 (담임 공강 확인용)
#[tauri::command]
pub fn lesson_homeroom_free(state: State<'_, AppState>) -> AppResult<Vec<HomeroomFreeSlot>> {
    state.db.read(lesson::homeroom_free_slots)
}

#[tauri::command]
pub fn lesson_readiness(state: State<'_, AppState>) -> AppResult<Vec<String>> {
    state.db.read(lesson::readiness)
}

#[tauri::command]
pub fn lesson_warnings(state: State<'_, AppState>) -> AppResult<Vec<String>> {
    state.db.read(lesson::warnings)
}

/// 설정 마법사 4단계 완료 처리.
#[tauri::command]
pub fn lesson_finish_step(state: State<'_, AppState>) -> AppResult<Vec<String>> {
    state.db.write(|c| {
        let problems = lesson::readiness(c)?;
        if problems.is_empty() {
            setup::set_status(c, STEP, setup::STATUS_DONE)?;
        }
        Ok(problems)
    })
}
