//! 시정표(교시 시각·점심시간) 설정 명령.

use tauri::State;

use crate::error::AppResult;
use crate::repo::bell::{self, BellOverview, GenParams, ReflowParams, Slot};
use crate::repo::school;
use crate::repo::setup;
use crate::AppState;

const STEP: &str = "BELL";

#[tauri::command]
pub fn bell_overview(state: State<'_, AppState>) -> AppResult<BellOverview> {
    state.db.read(bell::overview)
}

/// 시작 템플릿(전 학년 같음 / 저·고학년 / 저·중·고학년)으로 유형과 시각을 한 번에 만든다.
#[tauri::command]
pub fn bell_apply_template(
    state: State<'_, AppState>,
    template_key: String,
) -> AppResult<BellOverview> {
    state.db.write(|c| {
        let term_id = school::current_term_id(c)?;
        let sc = school::get(c)?.ok_or_else(|| {
            crate::error::AppError::setup_required("학교 기본 설정을 먼저 완료해 주세요.")
        })?;
        bell::apply_template(c, term_id, &template_key, &sc.school_days)?;
        setup::touch_in_progress(c, STEP)?;
        bell::overview(c)
    })
}

/// 규칙만 정해 주면 요일별 교시/점심 구간을 계산해서 돌려준다. 저장하지는 않는다.
#[tauri::command]
pub fn bell_generate(params: GenParams) -> AppResult<Vec<Slot>> {
    bell::generate(&params)
}

/// 교시 수와 점심·중간놀이 위치는 그대로 두고, 입력한 길이대로 시각만 다시 계산해 저장한다.
#[tauri::command]
pub fn bell_reflow(
    state: State<'_, AppState>,
    profile_ids: Vec<i64>,
    params: ReflowParams,
) -> AppResult<BellOverview> {
    state.db.write(|c| {
        let term_id = school::current_term_id(c)?;
        bell::reflow_profiles(c, term_id, &profile_ids, &params)?;
        setup::touch_in_progress(c, STEP)?;
        bell::overview(c)
    })
}

/// 입력 중인 시각의 문제점을 모두 돌려준다(저장 전 확인용).
#[tauri::command]
pub fn bell_check_slots(slots: Vec<Slot>) -> AppResult<Vec<String>> {
    Ok(bell::collect_problems(&slots))
}

#[tauri::command]
pub fn bell_create_profile(state: State<'_, AppState>, name: String) -> AppResult<BellOverview> {
    state.db.write(|c| {
        let term_id = school::current_term_id(c)?;
        bell::create_profile(c, term_id, &name)?;
        setup::touch_in_progress(c, STEP)?;
        bell::overview(c)
    })
}

#[tauri::command]
pub fn bell_rename_profile(
    state: State<'_, AppState>,
    profile_id: i64,
    name: String,
) -> AppResult<BellOverview> {
    state.db.write(|c| {
        let term_id = school::current_term_id(c)?;
        bell::rename_profile(c, term_id, profile_id, &name)?;
        bell::overview(c)
    })
}

#[tauri::command]
pub fn bell_delete_profile(
    state: State<'_, AppState>,
    profile_id: i64,
) -> AppResult<BellOverview> {
    state.db.write(|c| {
        let term_id = school::current_term_id(c)?;
        bell::delete_profile(c, term_id, profile_id)?;
        bell::overview(c)
    })
}

/// 유형의 시각과 적용 학년을 함께 저장한다.
#[tauri::command]
pub fn bell_save_profile(
    state: State<'_, AppState>,
    profile_id: i64,
    grades: Vec<i32>,
    slots: Vec<Slot>,
) -> AppResult<BellOverview> {
    state.db.write(|c| {
        let term_id = school::current_term_id(c)?;
        bell::save_slots(c, term_id, profile_id, &slots)?;
        bell::assign_grades(c, term_id, profile_id, &grades)?;
        setup::touch_in_progress(c, STEP)?;
        bell::overview(c)
    })
}

/// 설정 마법사 2단계 완료 처리. 빠진 곳이 있으면 안내 문구를 돌려준다.
#[tauri::command]
pub fn bell_finish_step(state: State<'_, AppState>) -> AppResult<Vec<String>> {
    state.db.write(|c| {
        let problems = bell::readiness(c)?;
        if problems.is_empty() {
            setup::set_status(c, STEP, setup::STATUS_DONE)?;
        }
        Ok(problems)
    })
}

/// 화면에서 '아직 남은 일'을 표시하기 위한 조회.
#[tauri::command]
pub fn bell_readiness(state: State<'_, AppState>) -> AppResult<Vec<String>> {
    state.db.read(bell::readiness)
}
