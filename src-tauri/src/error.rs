//! 앱 전역 오류 타입.
//!
//! 화면에는 `user_message`(사용자가 해결할 수 있는 한국어 설명)만 보여주고,
//! `detail`(개발자용 원문)은 [자세히] 접기 안에 둔다.

use serde::Serialize;

pub type AppResult<T> = std::result::Result<T, AppError>;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    /// 프로그램이 분기 처리할 때 쓰는 코드. 화면에 그대로 노출하지 않는다.
    pub code: String,
    /// 사용자에게 보여줄 한국어 문장.
    pub user_message: String,
    /// 개발자용 원문. 접기 안에 표시.
    pub detail: Option<String>,
}

impl AppError {
    pub fn new(code: &str, user_message: impl Into<String>) -> Self {
        Self {
            code: code.to_string(),
            user_message: user_message.into(),
            detail: None,
        }
    }

    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// 입력값이 잘못된 경우
    pub fn invalid(user_message: impl Into<String>) -> Self {
        Self::new("INVALID_INPUT", user_message)
    }

    /// 찾는 대상이 없는 경우
    pub fn not_found(user_message: impl Into<String>) -> Self {
        Self::new("NOT_FOUND", user_message)
    }

    /// 설정이 아직 되어 있지 않아 진행할 수 없는 경우
    pub fn setup_required(user_message: impl Into<String>) -> Self {
        Self::new("SETUP_REQUIRED", user_message)
    }

    /// 예상하지 못한 내부 오류
    pub fn internal(detail: impl Into<String>) -> Self {
        Self::new(
            "INTERNAL",
            "예상하지 못한 문제가 발생했습니다. 프로그램을 다시 시작해 주세요.",
        )
        .detail(detail)
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.code, self.user_message)?;
        if let Some(d) = &self.detail {
            write!(f, " ({d})")?;
        }
        Ok(())
    }
}

impl std::error::Error for AppError {}

impl From<rusqlite::Error> for AppError {
    fn from(e: rusqlite::Error) -> Self {
        use rusqlite::Error as E;
        match &e {
            E::QueryReturnedNoRows => AppError::not_found("요청하신 자료를 찾을 수 없습니다."),
            E::SqliteFailure(err, msg) => {
                let text = msg.clone().unwrap_or_default();
                if text.contains("UNIQUE constraint failed") {
                    AppError::new("DUPLICATE", "이미 같은 내용이 등록되어 있습니다.")
                        .detail(format!("{err:?} {text}"))
                } else if text.contains("FOREIGN KEY constraint failed") {
                    AppError::new(
                        "IN_USE",
                        "다른 자료에서 사용 중이라 처리할 수 없습니다. 연결된 자료를 먼저 정리해 주세요.",
                    )
                    .detail(format!("{err:?} {text}"))
                } else if text.contains("CHECK constraint failed") {
                    AppError::invalid("입력한 값이 올바르지 않습니다. 다시 확인해 주세요.")
                        .detail(format!("{err:?} {text}"))
                } else {
                    AppError::new("DB_ERROR", "자료를 저장하거나 불러오지 못했습니다.")
                        .detail(format!("{err:?} {text}"))
                }
            }
            other => AppError::new("DB_ERROR", "자료를 저장하거나 불러오지 못했습니다.")
                .detail(other.to_string()),
        }
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        AppError::internal(format!("JSON 처리 실패: {e}"))
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::new("IO_ERROR", "파일을 읽거나 쓰지 못했습니다.").detail(e.to_string())
    }
}

impl From<tauri::Error> for AppError {
    fn from(e: tauri::Error) -> Self {
        AppError::internal(format!("앱 내부 오류: {e}"))
    }
}
