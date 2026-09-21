//! 전담교사 식사시간 명령.
//!
//! 저장하는 것은 **사람이 정한 두 가지**뿐이다 — 학교 기본 식사시간과
//! 교사별·요일별 수동 지정. 자동 판정 결과는 저장하지 않고 조회할 때마다
//! 지금 시정표·전담 시간표로 다시 계산한다.

use tauri::State;

use crate::error::AppResult;
use crate::repo::meal as repo;
use crate::AppState;

/// 점심 패턴 · 기본 식사시간 · (교사를 주면) 그 교사의 요일별 식사시간.
#[tauri::command]
pub fn meal_view(
    state: State<'_, AppState>,
    teacher_id: Option<i64>,
) -> AppResult<repo::MealView> {
    state.db.read(|c| repo::view(c, teacher_id))
}

/// 학교 기본 식사시간을 정한다. `null` 이면 정하지 않은 상태로 되돌린다.
#[tauri::command]
pub fn meal_set_default(
    state: State<'_, AppState>,
    bell_schedule_id: Option<i64>,
    teacher_id: Option<i64>,
) -> AppResult<repo::MealView> {
    state.db.write(|c| {
        repo::set_default_bell(c, bell_schedule_id)?;
        repo::view(c, teacher_id)
    })
}

/// 교사별·요일별 식사시간을 직접 지정한다.
///
/// 시각을 비우면 수동 지정을 없애고 자동 판정으로 되돌린다.
/// 그 날 실제 수업과 겹치면 저장하지 않고 이유를 알려 준다.
#[tauri::command]
pub fn meal_set_override(
    state: State<'_, AppState>,
    input: repo::MealOverrideInput,
) -> AppResult<repo::MealView> {
    let teacher_id = input.teacher_id;
    state.db.write(|c| {
        repo::set_override(c, &input)?;
        repo::view(c, Some(teacher_id))
    })
}
