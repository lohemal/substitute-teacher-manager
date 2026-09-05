//! Phase 10 — 백업·복원, 동작 옵션, 학기, 내보내기.

use serde::Serialize;
use tauri::State;

use crate::error::{AppError, AppResult};
use crate::repo::{
    assign::HistoryFilter, backup as repo_backup, export as repo_export, settings as repo_settings,
    stats::StatsQuery, term as repo_term,
};
use crate::AppState;

// ============================================================
//  백업 · 복원
// ============================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupResult {
    pub name: String,
    pub path: String,
    pub folder: String,
}

/// 지금 자료를 백업 파일 하나로 저장한다.
#[tauri::command]
pub fn backup_create(state: State<'_, AppState>) -> AppResult<BackupResult> {
    let dir = state.db.backup_dir();
    let name = repo_backup::manual_name();
    let path = dir.join(&name);
    state.db.backup_to(&path)?;
    Ok(BackupResult {
        name,
        path: path.display().to_string(),
        folder: dir.display().to_string(),
    })
}

#[tauri::command]
pub fn backup_list(state: State<'_, AppState>) -> AppResult<Vec<repo_backup::BackupFile>> {
    repo_backup::list(&state.db.backup_dir())
}

#[tauri::command]
pub fn backup_delete(state: State<'_, AppState>, name: String) -> AppResult<()> {
    repo_backup::delete(&state.db.backup_dir(), &name)
}

#[tauri::command]
pub fn backup_open_folder(state: State<'_, AppState>) -> AppResult<()> {
    repo_backup::open_folder(&state.db.backup_dir())
}

#[tauri::command]
pub fn export_open_folder(state: State<'_, AppState>) -> AppResult<()> {
    repo_backup::open_folder(&state.db.export_dir())
}

/// 자료 폴더(= DB 파일이 있는 곳)를 연다.
#[tauri::command]
pub fn data_open_folder(state: State<'_, AppState>) -> AppResult<()> {
    let dir = state
        .db
        .path()
        .parent()
        .ok_or_else(|| AppError::internal("자료 폴더를 찾을 수 없습니다."))?
        .to_path_buf();
    repo_backup::open_folder(&dir)
}

/// 다른 곳에 있는 파일이 복원 가능한지 미리 확인한다.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InspectResult {
    pub ok: bool,
    pub schema_version: Option<i32>,
    pub problem: Option<String>,
}

#[tauri::command]
pub fn backup_inspect(path: String) -> AppResult<InspectResult> {
    let (version, problem) = repo_backup::inspect(std::path::Path::new(path.trim()));
    Ok(InspectResult {
        ok: problem.is_none(),
        schema_version: version,
        problem,
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreResult {
    /// 복원 직전에 남긴 안전 백업
    pub safety_backup: String,
    pub schema_version: i32,
}

/// 백업으로 되돌린다.
///
/// `name`은 백업 폴더 안의 파일, `path`는 다른 곳의 파일(USB 등)이다.
/// 되돌리기 전에 지금 자료를 자동으로 백업한다.
#[tauri::command]
pub fn backup_restore(
    state: State<'_, AppState>,
    name: Option<String>,
    path: Option<String>,
) -> AppResult<RestoreResult> {
    let dir = state.db.backup_dir();

    let src = match (name.as_deref(), path.as_deref()) {
        (Some(n), _) if !n.trim().is_empty() => repo_backup::resolve_in_dir(&dir, n)?,
        (_, Some(p)) if !p.trim().is_empty() => std::path::PathBuf::from(p.trim()),
        _ => return Err(AppError::invalid("되돌릴 백업 파일을 골라 주세요.")),
    };

    // 확인부터 — 잘못된 파일이면 아무것도 건드리지 않는다
    let (version, problem) = repo_backup::inspect(&src);
    if let Some(p) = problem {
        return Err(AppError::new("BAD_BACKUP", p));
    }

    let safety = dir.join(repo_backup::safety_name("restore"));
    state.db.restore_from(&src, &safety)?;

    Ok(RestoreResult {
        safety_backup: safety
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string(),
        schema_version: version.unwrap_or(0),
    })
}

/// 설정에 따라 자동 백업을 남긴다. 앱을 켤 때와 끌 때 호출한다.
///
/// `on_exit = true`면 '앱 종료 시' 설정일 때만 남긴다.
pub fn auto_backup(state: &AppState, on_exit: bool) -> AppResult<Option<String>> {
    let (mode, keep) = state.db.read(|c| {
        Ok((
            repo_settings::get_text(c, repo_settings::AUTO_BACKUP_MODE, repo_settings::BACKUP_DAILY)?,
            repo_settings::get_int(c, repo_settings::AUTO_BACKUP_KEEP, 10)?,
        ))
    })?;

    let dir = state.db.backup_dir();
    let should = match mode.as_str() {
        repo_settings::BACKUP_DAILY => !on_exit && !repo_backup::has_auto_today(&dir),
        repo_settings::BACKUP_ON_EXIT => on_exit,
        _ => false,
    };
    if !should {
        return Ok(None);
    }

    let name = repo_backup::auto_name();
    state.db.backup_to(&dir.join(&name))?;
    repo_backup::prune_auto(&dir, keep.max(1) as usize)?;
    Ok(Some(name))
}

/// 화면에서 직접 자동 백업을 돌려 볼 때 (설정 확인용)
#[tauri::command]
pub fn backup_auto_now(state: State<'_, AppState>) -> AppResult<Option<String>> {
    auto_backup(&state, false)
}

// ============================================================
//  동작 옵션
// ============================================================

#[tauri::command]
pub fn settings_view(state: State<'_, AppState>) -> AppResult<repo_settings::SettingsView> {
    let db_path = state.db.path().display().to_string();
    let backup_dir = state.db.backup_dir().display().to_string();
    let export_dir = state.db.export_dir().display().to_string();
    state
        .db
        .read(|c| repo_settings::view(c, &db_path, &backup_dir, &export_dir))
}

#[tauri::command]
pub fn settings_save(
    state: State<'_, AppState>,
    input: repo_settings::SettingsInput,
) -> AppResult<()> {
    state.db.write(|c| repo_settings::save(c, &input))
}

/// **동작 옵션만** 기본값으로. 자료는 그대로 둔다.
#[tauri::command]
pub fn settings_reset(state: State<'_, AppState>) -> AppResult<()> {
    state.db.write(repo_settings::reset)
}

// ============================================================
//  학기
// ============================================================

#[tauri::command]
pub fn term_list(state: State<'_, AppState>) -> AppResult<Vec<repo_term::TermRow>> {
    state.db.read(repo_term::list)
}

#[tauri::command]
pub fn term_save_dates(
    state: State<'_, AppState>,
    input: repo_term::TermDates,
) -> AppResult<()> {
    state.db.write(|c| repo_term::save_dates(c, &input))
}

#[tauri::command]
pub fn term_set_current(state: State<'_, AppState>, id: i64) -> AppResult<()> {
    state.db.write(|c| repo_term::set_current(c, id))
}

#[tauri::command]
pub fn term_start_new(
    state: State<'_, AppState>,
    input: repo_term::NewTermInput,
) -> AppResult<repo_term::NewTermResult> {
    // 새 학기는 되돌리기 어려우므로 먼저 백업해 둔다
    let dir = state.db.backup_dir();
    let safety = repo_backup::safety_name("newterm");
    let _ = state.db.backup_to(&dir.join(&safety));

    state.db.write(|c| repo_term::start_new(c, &input))
}

// ============================================================
//  CSV 내보내기
// ============================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    pub name: String,
    pub path: String,
    pub folder: String,
    pub rows: i32,
}

#[tauri::command]
pub fn export_history_csv(
    state: State<'_, AppState>,
    filter: Option<HistoryFilter>,
) -> AppResult<ExportResult> {
    let f = filter.unwrap_or_default();
    let body = state.db.read(|c| repo_export::history_csv(c, &f))?;
    let rows = body.lines().count().saturating_sub(1) as i32;

    let dir = state.db.export_dir();
    let name = repo_export::safe_name("보결배정내역");
    let path = repo_export::write_csv(&dir, &name, &body)?;

    Ok(ExportResult {
        name,
        path: path.display().to_string(),
        folder: dir.display().to_string(),
        rows,
    })
}

#[tauri::command]
pub fn export_stats_csv(
    state: State<'_, AppState>,
    query: Option<StatsQuery>,
) -> AppResult<ExportResult> {
    let q = query.unwrap_or_default();
    let body = state.db.read(|c| repo_export::stats_csv(c, &q))?;

    let dir = state.db.export_dir();
    let name = repo_export::safe_name("보결현황");
    let path = repo_export::write_csv(&dir, &name, &body)?;

    Ok(ExportResult {
        name,
        path: path.display().to_string(),
        folder: dir.display().to_string(),
        rows: body.lines().count() as i32,
    })
}

// ============================================================
//  자료 전체 초기화
// ============================================================

/// 사용자가 그대로 입력해야 하는 확인 문구.
pub const RESET_PHRASE: &str = "전체 초기화";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DataResetResult {
    pub backup: String,
    pub folder: String,
}

/// **모든 자료를 지운다.** 되돌릴 수 없으므로 확인 문구를 그대로 받아야 하고,
/// 지우기 직전에 반드시 백업을 남긴다.
#[tauri::command]
pub fn data_reset(state: State<'_, AppState>, confirm: String) -> AppResult<DataResetResult> {
    if confirm.trim() != RESET_PHRASE {
        return Err(AppError::invalid(format!(
            "확인 문구가 다릅니다. '{RESET_PHRASE}'을 그대로 입력해 주세요."
        )));
    }

    // 1. 반드시 백업부터
    let dir = state.db.backup_dir();
    let name = repo_backup::safety_name("reset");
    state.db.backup_to(&dir.join(&name))?;

    // 2. 사람이 넣은 자료만 지운다. 기준 자료(구분·사유·과목)는 남긴다.
    state.db.write(|c| {
        for sql in [
            "DELETE FROM substitutions",
            "DELETE FROM absences",
            "DELETE FROM lessons",
            "DELETE FROM teacher_busy_blocks",
            "DELETE FROM teacher_subjects",
            "DELETE FROM teachers",
            "DELETE FROM classes",
            "DELETE FROM grade_bell_map",
            "DELETE FROM bell_slots",
            "DELETE FROM bell_schedules",
            "DELETE FROM terms",
            "DELETE FROM school",
            "DELETE FROM app_meta WHERE key = 'setup_completed_at'",
            "UPDATE setup_steps SET status = 'PENDING'",
            "DELETE FROM app_meta WHERE key LIKE 'setup_draft:%'",
        ] {
            c.execute(sql, [])?;
        }
        Ok(())
    })?;

    Ok(DataResetResult {
        backup: name,
        folder: dir.display().to_string(),
    })
}
