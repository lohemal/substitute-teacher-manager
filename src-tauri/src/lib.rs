mod commands;
mod db;
mod domain;
pub mod error;
mod label;
mod repo;

use std::sync::Arc;

use tauri::Manager;

use crate::db::Db;

pub struct AppState {
    pub db: Arc<Db>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            // %APPDATA%\kr.school.bogyeol\bogyeol.db
            let dir = app.path().app_data_dir()?;
            let db_path = dir.join("bogyeol.db");

            let db = Db::open(&db_path)?;
            log::info!(
                "DB ready: {} (schema v{})",
                db.path().display(),
                db.schema_version()?
            );

            app.manage(AppState { db: Arc::new(db) });

            // 설정이 '하루 1회'면 오늘 첫 실행에 자동 백업을 남긴다
            if let Some(name) = commands::auto_backup(&app.state::<AppState>(), false)
                .unwrap_or_else(|e| {
                    log::warn!("자동 백업 실패: {e}");
                    None
                })
            {
                log::info!("자동 백업: {name}");
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // 설정이 '앱 종료 시'면 창을 닫을 때 남긴다
            if let tauri::WindowEvent::Destroyed = event {
                let state = window.state::<AppState>();
                match commands::auto_backup(&state, true) {
                    Ok(Some(name)) => log::info!("종료 시 자동 백업: {name}"),
                    Ok(None) => {}
                    Err(e) => log::warn!("종료 시 자동 백업 실패: {e}"),
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            // 설정 마법사
            commands::setup_get_state,
            commands::setup_save_draft,
            commands::setup_get_draft,
            commands::setup_skip_step,
            commands::setup_complete,
            commands::setup_restart,
            // 학교 기본 설정
            commands::school_get,
            commands::school_save,
            // 시정표 · 점심시간
            commands::bell_overview,
            commands::bell_apply_template,
            commands::bell_generate,
            commands::bell_reflow,
            commands::bell_check_slots,
            commands::bell_create_profile,
            commands::bell_rename_profile,
            commands::bell_delete_profile,
            commands::bell_save_profile,
            commands::bell_finish_step,
            commands::bell_readiness,
            // 교사 관리
            commands::teacher_list,
            commands::teacher_upsert,
            commands::teacher_set_active,
            commands::teacher_delete,
            commands::teacher_set_substitutable_bulk,
            commands::teacher_set_active_bulk,
            commands::teacher_quick_add_homerooms,
            commands::teacher_bulk_create,
            commands::subject_upsert,
            commands::teacher_readiness,
            commands::teacher_finish_step,
            // 전담교사 시간표
            commands::lesson_overview,
            commands::lesson_get,
            commands::lesson_upsert,
            commands::lesson_delete,
            commands::lesson_check,
            commands::lesson_bulk_replace,
            commands::lesson_homeroom_free,
            commands::lesson_readiness,
            commands::lesson_warnings,
            commands::lesson_finish_step,
            // 보결 가능 교사 조회
            commands::find_options,
            commands::find_weekday,
            commands::find_in_charge,
            commands::find_candidates,
            // 추천 우선순위
            commands::priority_list,
            commands::priority_save,
            commands::priority_reset,
            commands::priority_finish_step,
            // 보결 배정 · 취소 · 기록
            commands::absence_create,
            commands::absence_cancel,
            commands::absence_list,
            commands::assign_create,
            commands::assign_day_plan,
            commands::assign_batch,
            commands::assign_cancel,
            commands::assign_history,
            // 보결 현황
            commands::stats_view,
            // 전담교사 식사시간 — 사람이 정한 값만 저장한다
            commands::meal_view,
            commands::meal_set_default,
            commands::meal_set_override,
            // 보결 수당 — 기록을 집계만 한다 (읽기 전용)
            commands::pay_view,
            commands::pay_detail,
            // 백업 · 복원
            commands::backup_create,
            commands::backup_list,
            commands::backup_delete,
            commands::backup_inspect,
            commands::backup_restore,
            commands::backup_auto_now,
            commands::backup_open_folder,
            commands::export_open_folder,
            commands::data_open_folder,
            // 동작 옵션
            commands::settings_view,
            commands::settings_save,
            commands::settings_reset,
            // 학기
            commands::term_list,
            commands::term_save_dates,
            commands::term_set_current,
            commands::term_start_new,
            // 내보내기 · 초기화
            commands::export_history_xlsx,
            commands::export_stats_xlsx,
            commands::export_pay_xlsx,
            commands::data_reset,
        ])
        .run(tauri::generate_context!())
        .expect("보결 배정 시스템을 시작하지 못했습니다.");
}
