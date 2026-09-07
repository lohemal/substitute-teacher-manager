//! 보결 수당 명령. **모두 읽기 전용이다.**
//!
//! 수당은 남아 있는 배정 기록을 집계한 결과일 뿐이므로, 이 명령들은 어떤
//! 기록도 만들거나 고치지 않는다. 금액과 지급 기준은 설정 명령
//! (`settings_save`)으로 바꾸고, 여기서는 그 설정을 읽어 계산만 한다.

use tauri::State;

use crate::error::AppResult;
use crate::repo::pay as repo;
use crate::AppState;

use super::admin::ExportResult;

/// 고른 기간의 교사별 계산표.
#[tauri::command]
pub fn pay_view(
    state: State<'_, AppState>,
    query: Option<repo::PayQuery>,
) -> AppResult<repo::PayView> {
    let q = query.unwrap_or_default();
    state.db.read(|c| repo::view(c, &q))
}

/// 한 사람의 계산 근거. 어떤 보결이 세어졌는지 그대로 보여 준다.
#[tauri::command]
pub fn pay_detail(
    state: State<'_, AppState>,
    teacher_id: i64,
    query: Option<repo::PayQuery>,
) -> AppResult<repo::PayDetail> {
    let q = query.unwrap_or_default();
    state.db.read(|c| repo::detail(c, teacher_id, &q))
}

/// 지금 조회 결과를 엑셀 파일로 저장한다.
///
/// 금액과 횟수는 숫자로 담기므로 엑셀에서 그대로 합계를 낼 수 있다.
#[tauri::command]
pub fn export_pay_xlsx(
    state: State<'_, AppState>,
    query: Option<repo::PayQuery>,
) -> AppResult<ExportResult> {
    let q = query.unwrap_or_default();
    let sheets = state.db.read(|c| repo::sheets(c, &q))?;
    super::admin::save_book(&state, "보결수당", &sheets)
}
