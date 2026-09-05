//! 앱 기본 정보 / 상태 조회.

use serde::Serialize;
use tauri::State;

use crate::db::migrate;
use crate::error::AppResult;
use crate::AppState;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    /// 앱 버전 (Cargo.toml)
    pub app_version: String,
    /// 현재 DB 스키마 버전
    pub schema_version: i32,
    /// 이 앱 빌드가 지원하는 최신 스키마 버전
    pub latest_schema_version: i32,
    /// 자료 파일 경로 (설정 화면의 '자료 위치 열기'에서 사용)
    pub db_path: String,
    /// 초기 설정 완료 여부
    pub setup_completed: bool,
}

#[tauri::command]
pub fn app_info(state: State<'_, AppState>) -> AppResult<AppInfo> {
    let db = &state.db;

    let (schema_version, setup_completed) = db.read(|c| {
        let v = migrate::current_version(c)?;
        let done: i64 = c.query_row(
            "SELECT COUNT(*) FROM app_meta WHERE key = 'setup_completed_at'",
            [],
            |r| r.get(0),
        )?;
        Ok((v, done > 0))
    })?;

    Ok(AppInfo {
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        schema_version,
        latest_schema_version: migrate::latest_version(),
        db_path: db.path().display().to_string(),
        setup_completed,
    })
}
