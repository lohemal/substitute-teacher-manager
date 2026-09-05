//! Phase 8 — 결근 등록 · 보결 배정 · 취소 · 배정 내역 명령.
//!
//! 쓰기는 모두 `state.db.write`(트랜잭션) 안에서 한다. 클로저가 오류를 돌려주면
//! 그때까지의 변경이 전부 되돌아가므로, 일괄 배정의 '전체 성공 / 전체 실패'가
//! 자연스럽게 지켜진다.

use tauri::State;

use crate::error::AppResult;
use crate::repo::assign as repo;
use crate::AppState;

// ---------- 결근 ----------

#[tauri::command]
pub fn absence_create(state: State<'_, AppState>, input: repo::AbsenceInput) -> AppResult<i64> {
    state.db.write(|c| repo::create_absence(c, &input))
}

/// 결근을 취소한다. 남아 있는 활성 보결 건수를 돌려준다.
#[tauri::command]
pub fn absence_cancel(state: State<'_, AppState>, id: i64) -> AppResult<i32> {
    state.db.write(|c| repo::cancel_absence(c, id))
}

#[tauri::command]
pub fn absence_list(
    state: State<'_, AppState>,
    date: String,
    all: Option<bool>,
) -> AppResult<Vec<repo::AbsenceRow>> {
    state
        .db
        .read(|c| repo::absences_on(c, &date, all.unwrap_or(false)))
}

// ---------- 배정 ----------

/// 하나 배정한다. 저장 직전에 후보 판정 엔진을 다시 돌린다.
#[tauri::command]
pub fn assign_create(
    state: State<'_, AppState>,
    input: repo::AssignInput,
) -> AppResult<repo::AssignSaved> {
    state.db.write(|c| repo::assign_one(c, &input))
}

/// 결근 교사의 그 날 일정과 시간대별 추천 후보.
#[tauri::command]
pub fn assign_day_plan(
    state: State<'_, AppState>,
    date: String,
    teacher_id: i64,
) -> AppResult<repo::DayPlan> {
    state.db.read(|c| repo::day_plan(c, &date, teacher_id))
}

/// 하루 일괄 보결. 전체 성공 또는 전체 실패.
#[tauri::command]
pub fn assign_batch(
    state: State<'_, AppState>,
    input: repo::BatchInput,
) -> AppResult<repo::BatchSaved> {
    state.db.write(|c| repo::assign_batch(c, &input))
}

/// 배정 취소. 기록은 남기고 상태만 바꾼다.
#[tauri::command]
pub fn assign_cancel(
    state: State<'_, AppState>,
    id: i64,
    reason: Option<String>,
) -> AppResult<()> {
    state.db.write(|c| repo::cancel(c, id, reason.as_deref()))
}

#[tauri::command]
pub fn assign_history(
    state: State<'_, AppState>,
    filter: Option<repo::HistoryFilter>,
) -> AppResult<repo::HistoryView> {
    let f = filter.unwrap_or_default();
    state.db.read(|c| repo::history(c, &f))
}
