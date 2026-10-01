//! 보결 불필요 명령.
//!
//! 결근으로 생긴 보결 필요 교시 가운데, 일정이 바뀌어 사람을 넣지 않아도
//! 되는 칸을 기록하고 되돌린다. 결근 기록과 배정 기록은 건드리지 않는다.

use tauri::State;

use crate::error::AppResult;
use crate::repo::{assign as repo_assign, waiver as repo};
use crate::AppState;

/// 보결 불필요로 처리하고, 바뀐 하루치 계획을 돌려준다.
#[tauri::command]
pub fn waiver_set(
    state: State<'_, AppState>,
    input: repo::WaiverInput,
) -> AppResult<repo_assign::DayPlan> {
    let (date, teacher_id) = (input.date.clone(), input.absent_teacher_id);
    state.db.write(|c| {
        repo::waive(c, &input)?;
        repo_assign::day_plan(c, &date, teacher_id)
    })
}

/// 다시 보결이 필요한 칸으로 되돌린다. 기록은 남는다.
#[tauri::command]
pub fn waiver_revoke(
    state: State<'_, AppState>,
    id: i64,
    date: String,
    teacher_id: i64,
) -> AppResult<repo_assign::DayPlan> {
    state.db.write(|c| {
        repo::revoke(c, id)?;
        repo_assign::day_plan(c, &date, teacher_id)
    })
}
