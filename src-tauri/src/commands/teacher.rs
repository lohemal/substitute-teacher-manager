//! 교사 관리 명령.

use tauri::State;

use crate::error::AppResult;
use crate::repo::setup;
use crate::repo::teacher::{
    self, BulkResult, BulkTeacherRow, QuickHomeroomEntry, SetActiveResult, TeacherInput,
    TeacherList,
};
use crate::AppState;

const STEP: &str = "TEACHER";

#[tauri::command]
pub fn teacher_list(state: State<'_, AppState>) -> AppResult<TeacherList> {
    state.db.read(teacher::list)
}

#[tauri::command]
pub fn teacher_upsert(state: State<'_, AppState>, input: TeacherInput) -> AppResult<TeacherList> {
    state.db.write(|c| {
        teacher::upsert(c, &input)?;
        setup::touch_in_progress(c, STEP)?;
        teacher::list(c)
    })
}

#[tauri::command]
pub fn teacher_set_active(
    state: State<'_, AppState>,
    teacher_id: i64,
    active: bool,
) -> AppResult<SetActiveResult> {
    state.db.write(|c| {
        let unassigned_classes = teacher::set_active(c, teacher_id, active)?;
        Ok(SetActiveResult {
            list: teacher::list(c)?,
            unassigned_classes,
        })
    })
}

#[tauri::command]
pub fn teacher_delete(state: State<'_, AppState>, teacher_id: i64) -> AppResult<TeacherList> {
    state.db.write(|c| {
        teacher::delete(c, teacher_id)?;
        teacher::list(c)
    })
}

/// 선택한 교사들의 보결 배정 대상 여부를 한 번에 바꾼다.
#[tauri::command]
pub fn teacher_set_substitutable_bulk(
    state: State<'_, AppState>,
    teacher_ids: Vec<i64>,
    value: bool,
) -> AppResult<TeacherList> {
    state.db.write(|c| {
        teacher::set_substitutable_bulk(c, &teacher_ids, value)?;
        setup::touch_in_progress(c, STEP)?;
        teacher::list(c)
    })
}

/// 선택한 교사들을 한 번에 활성/비활성으로 바꾼다.
#[tauri::command]
pub fn teacher_set_active_bulk(
    state: State<'_, AppState>,
    teacher_ids: Vec<i64>,
    active: bool,
) -> AppResult<SetActiveResult> {
    state.db.write(|c| {
        let unassigned_classes = teacher::set_active_bulk(c, &teacher_ids, active)?;
        Ok(SetActiveResult {
            list: teacher::list(c)?,
            unassigned_classes,
        })
    })
}

/// 담임이 비어 있는 학급에 이름만 넣어 교사를 함께 만든다.
#[tauri::command]
pub fn teacher_quick_add_homerooms(
    state: State<'_, AppState>,
    entries: Vec<QuickHomeroomEntry>,
) -> AppResult<BulkResult> {
    state.db.write(|c| {
        let (created, problems) = teacher::quick_add_homerooms(c, &entries)?;
        setup::touch_in_progress(c, STEP)?;
        Ok(BulkResult {
            created,
            skipped: 0,
            problems,
            list: teacher::list(c)?,
        })
    })
}

/// 명단을 한 번에 등록한다. (붙여넣기 / 나중에 CSV·Excel 가져오기가 이 명령을 쓴다)
#[tauri::command]
pub fn teacher_bulk_create(
    state: State<'_, AppState>,
    rows: Vec<BulkTeacherRow>,
) -> AppResult<BulkResult> {
    state.db.write(|c| {
        let (created, skipped, problems) = teacher::bulk_create(c, &rows)?;
        setup::touch_in_progress(c, STEP)?;
        Ok(BulkResult {
            created,
            skipped,
            problems,
            list: teacher::list(c)?,
        })
    })
}

#[tauri::command]
pub fn subject_upsert(state: State<'_, AppState>, name: String) -> AppResult<TeacherList> {
    state.db.write(|c| {
        teacher::upsert_subject(c, &name)?;
        teacher::list(c)
    })
}

#[tauri::command]
pub fn teacher_readiness(state: State<'_, AppState>) -> AppResult<Vec<String>> {
    state.db.read(teacher::readiness)
}

/// 설정 마법사 3단계 완료 처리. 빠진 곳이 있으면 안내 문구를 돌려준다.
#[tauri::command]
pub fn teacher_finish_step(state: State<'_, AppState>) -> AppResult<Vec<String>> {
    state.db.write(|c| {
        let problems = teacher::readiness(c)?;
        if problems.is_empty() {
            setup::set_status(c, STEP, setup::STATUS_DONE)?;
        }
        Ok(problems)
    })
}
