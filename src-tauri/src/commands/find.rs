//! 보결 가능 교사 조회 명령.

use serde::Deserialize;
use tauri::State;

use crate::domain::find::{self, FindError, FindRequest, FindResult};
use crate::domain::priority;
use crate::error::{AppError, AppResult};
use crate::repo::bell::day_name;
use crate::repo::find as repo_find;
use crate::repo::priority as repo_priority;
use crate::AppState;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FindQuery {
    /// YYYY-MM-DD
    pub date: String,
    pub class_id: i64,
    /// PERIOD | LUNCH
    pub slot_type: String,
    #[serde(default)]
    pub period_no: Option<i32>,
    #[serde(default)]
    pub absent_teacher_id: Option<i64>,
}

/// 조회 화면이 쓰는 선택 항목 (학급·교시·교사·사유)
#[tauri::command]
pub fn find_options(state: State<'_, AppState>) -> AppResult<repo_find::FindOptions> {
    state.db.read(repo_find::options)
}

/// 날짜만 주면 요일을 돌려준다. (화면 표시용)
#[tauri::command]
pub fn find_weekday(date: String) -> AppResult<i32> {
    Ok(repo_find::parse_date(&date)?.1)
}

/// 조회 전에 보여 주는 '이 시간 담당' 안내.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InChargeView {
    pub slot: find::TargetSlot,
    pub notice: Option<find::SlotNotice>,
}

/// 그 시간을 원래 누가 담당하는지 알려 준다.
///
/// 조회 버튼을 누르기 전에 "이 시간은 전담 시간입니다"를 먼저 보여 주기 위한 것이다.
/// 시정표에 없는 시간이면 오류 대신 `None`을 돌려준다 (조회할 때 안내한다).
#[tauri::command]
pub fn find_in_charge(
    state: State<'_, AppState>,
    query: FindQuery,
) -> AppResult<Option<InChargeView>> {
    if !matches!(query.slot_type.as_str(), "PERIOD" | "LUNCH") {
        return Ok(None);
    }
    if query.slot_type == "PERIOD" && query.period_no.is_none() {
        return Ok(None);
    }

    state.db.read(|c| {
        let snap = repo_find::snapshot(c, &query.date)?;
        let req = FindRequest {
            class_id: query.class_id,
            slot_type: query.slot_type.clone(),
            period_no: query.period_no,
            absent_teacher_id: query.absent_teacher_id,
        };

        let Ok(slot) = find::resolve_slot(&snap, &req) else {
            return Ok(None);
        };
        let notice = find::build_notice(
            &slot.in_charge,
            &slot.class_label,
            &slot.slot_label,
            query.absent_teacher_id,
        );
        Ok(Some(InChargeView { slot, notice }))
    })
}

/// 보결 가능 교사를 찾는다.
#[tauri::command]
pub fn find_candidates(state: State<'_, AppState>, query: FindQuery) -> AppResult<FindResult> {
    if !matches!(query.slot_type.as_str(), "PERIOD" | "LUNCH") {
        return Err(AppError::invalid("보결 시간을 선택해 주세요."));
    }
    if query.slot_type == "PERIOD" && query.period_no.is_none() {
        return Err(AppError::invalid("몇 교시인지 선택해 주세요."));
    }

    state.db.read(|c| {
        let snap = repo_find::snapshot(c, &query.date)?;
        let counts = repo_find::counts(c, &query.date)?;

        let req = FindRequest {
            class_id: query.class_id,
            slot_type: query.slot_type.clone(),
            period_no: query.period_no,
            absent_teacher_id: query.absent_teacher_id,
        };

        // STEP 1 — 가능/불가능 판정
        let mut result = find::find_candidates(&snap, &req, &counts).map_err(|e| match e {
            FindError::ClassNotFound => {
                AppError::not_found("학급을 찾을 수 없습니다. 다시 선택해 주세요.")
            }
            FindError::SlotNotFound {
                class_full_label,
                day_of_week,
                slot_label,
            } => AppError::new(
                "SLOT_NOT_FOUND",
                format!(
                    "{}은 {}요일에 {}가 없습니다. 시정표를 확인하거나 다른 시간을 선택해 주세요.",
                    class_full_label,
                    day_name(day_of_week),
                    slot_label
                ),
            ),
        })?;

        // STEP 2 — 학교가 정한 기준으로 추천 순서를 매긴다
        let settings = repo_priority::settings(c)?;
        priority::rank_candidates(&mut result.eligible, &settings, result.slot.grade);

        Ok(result)
    })
}
