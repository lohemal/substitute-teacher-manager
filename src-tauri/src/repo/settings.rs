//! Phase 10 — 프로그램 동작 옵션.
//!
//! ## 무엇을 옵션으로 두고, 무엇을 두지 않는가
//!
//! 학교마다 다른 **운영 방침**만 옵션으로 둔다. 점심시간에 담임을 부를지,
//! 전담을 보결에 넣을지 같은 것이다.
//!
//! **실제 시간이 겹치는 교사를 후보에 넣는 설정은 만들지 않는다.** 그것은
//! 방침이 아니라 사실의 문제이고, 끄는 순간 한 사람이 같은 시각에 두 교실에
//! 있게 된다. Phase 6에서 정한 이 원칙은 옵션으로 흔들지 않는다.

use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

// ---------- 보결 판단 ----------
pub const DERIVE_HOMEROOM: &str = "derive_homeroom_schedule";
pub const EXCLUDE_LUNCH: &str = "exclude_homeroom_on_own_lunch";
pub const EXCLUDE_RECESS: &str = "exclude_homeroom_on_recess";
pub const INCLUDE_SPECIAL: &str = "include_special_teachers";
pub const INCLUDE_AFTER_END: &str = "include_after_school_end";
pub const INCLUDE_OTHER_GRADE: &str = "include_other_grade_homeroom";

// ---------- 자동 백업 ----------
pub const AUTO_BACKUP_MODE: &str = "auto_backup_mode";
pub const AUTO_BACKUP_KEEP: &str = "auto_backup_keep";

pub const BACKUP_OFF: &str = "OFF";
pub const BACKUP_DAILY: &str = "DAILY";
pub const BACKUP_ON_EXIT: &str = "ON_EXIT";

/// 옵션 하나의 정의. 화면은 이 목록만 보고 그린다 — 새 옵션은 여기에만 추가한다.
pub struct OptionDef {
    pub key: &'static str,
    pub label: &'static str,
    pub hint: &'static str,
    pub default: bool,
}

/// 보결 판단 옵션. 위에서부터 화면에 나오는 순서다.
pub const ENGINE_OPTIONS: &[OptionDef] = &[
    OptionDef {
        key: EXCLUDE_LUNCH,
        label: "점심시간에는 그 학년 담임을 부르지 않기",
        hint: "담임이 자기 반 급식 지도를 하는 학교에서 켜 둡니다. 끄면 점심시간에도 담임이 후보로 나옵니다.",
        default: true,
    },
    OptionDef {
        key: EXCLUDE_RECESS,
        label: "중간놀이 시간에도 그 학년 담임을 부르지 않기",
        hint: "중간놀이에 담임이 교실을 지키는 학교에서 켜 둡니다.",
        default: true,
    },
    OptionDef {
        key: INCLUDE_SPECIAL,
        label: "전담 선생님을 보결 후보에 넣기",
        hint: "전담은 공강이 많아 자주 후보가 됩니다. 전담에게 보결을 맡기지 않는 학교라면 끄세요.",
        default: true,
    },
    OptionDef {
        key: INCLUDE_OTHER_GRADE,
        label: "다른 학년 담임도 후보에 넣기",
        hint: "끄면 같은 학년 담임과 전담·기타 선생님만 후보가 됩니다.",
        default: true,
    },
    OptionDef {
        key: INCLUDE_AFTER_END,
        label: "그 날 수업이 끝난 담임도 후보에 넣기",
        hint: "저학년 담임은 고학년 6교시에 수업이 없습니다. 그때 부르지 않는 학교라면 끄세요.",
        default: true,
    },
    OptionDef {
        key: DERIVE_HOMEROOM,
        label: "담임 수업 시간을 자동으로 계산하기",
        hint: "자기 반 전체 교시에서 전담이 들어오는 교시를 뺀 것이 담임 수업입니다. 끄면 담임이 늘 비어 있는 것으로 보이므로 켜 두시길 권합니다.",
        default: true,
    },
];

pub fn get_bool(conn: &Connection, key: &str, default: bool) -> AppResult<bool> {
    let raw: Option<String> = conn
        .query_row("SELECT value_json FROM settings WHERE key = ?1", [key], |r| {
            r.get(0)
        })
        .optional()?;
    Ok(match raw.as_deref() {
        Some("true") => true,
        Some("false") => false,
        _ => default,
    })
}

pub fn get_text(conn: &Connection, key: &str, default: &str) -> AppResult<String> {
    let raw: Option<String> = conn
        .query_row("SELECT value_json FROM settings WHERE key = ?1", [key], |r| {
            r.get(0)
        })
        .optional()?;
    Ok(raw
        .map(|v| v.trim_matches('"').to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| default.to_string()))
}

pub fn get_int(conn: &Connection, key: &str, default: i64) -> AppResult<i64> {
    let raw: Option<String> = conn
        .query_row("SELECT value_json FROM settings WHERE key = ?1", [key], |r| {
            r.get(0)
        })
        .optional()?;
    Ok(raw.and_then(|v| v.trim().parse::<i64>().ok()).unwrap_or(default))
}

fn put(conn: &Connection, key: &str, value_json: &str) -> AppResult<()> {
    conn.execute(
        "INSERT INTO settings(key, value_json, updated_at)
         VALUES (?1, ?2, datetime('now','localtime'))
         ON CONFLICT(key) DO UPDATE SET value_json = ?2, updated_at = datetime('now','localtime')",
        rusqlite::params![key, value_json],
    )?;
    Ok(())
}

// ============================================================
//  화면에 넘길 모양
// ============================================================

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OptionRow {
    pub key: String,
    pub label: String,
    pub hint: String,
    pub value: bool,
    pub is_default: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsView {
    pub engine: Vec<OptionRow>,
    /// OFF | DAILY | ON_EXIT
    pub auto_backup_mode: String,
    pub auto_backup_keep: i64,
    /// 하나라도 기본값과 다른가
    pub changed: bool,
    pub db_path: String,
    pub backup_dir: String,
    pub export_dir: String,
    /// 옵션으로 끌 수 없는 것 — 화면에 못 박아 둔다
    pub fixed_note: String,
}

pub const FIXED_NOTE: &str =
    "실제 시간이 겹치는 선생님은 어떤 설정으로도 후보에 넣지 않습니다. \
     한 사람이 같은 시각에 두 교실에 있을 수는 없기 때문입니다.";

pub fn view(
    conn: &Connection,
    db_path: &str,
    backup_dir: &str,
    export_dir: &str,
) -> AppResult<SettingsView> {
    let mut engine = Vec::new();
    let mut changed = false;
    for d in ENGINE_OPTIONS {
        let value = get_bool(conn, d.key, d.default)?;
        if value != d.default {
            changed = true;
        }
        engine.push(OptionRow {
            key: d.key.to_string(),
            label: d.label.to_string(),
            hint: d.hint.to_string(),
            value,
            is_default: value == d.default,
        });
    }

    let mode = get_text(conn, AUTO_BACKUP_MODE, BACKUP_DAILY)?;
    let keep = get_int(conn, AUTO_BACKUP_KEEP, 10)?;
    if mode != BACKUP_DAILY || keep != 10 {
        changed = true;
    }

    Ok(SettingsView {
        engine,
        auto_backup_mode: mode,
        auto_backup_keep: keep,
        changed,
        db_path: db_path.to_string(),
        backup_dir: backup_dir.to_string(),
        export_dir: export_dir.to_string(),
        fixed_note: FIXED_NOTE.to_string(),
    })
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsInput {
    /// 바꿀 옵션만 담아도 된다
    #[serde(default)]
    pub engine: Vec<EngineInput>,
    #[serde(default)]
    pub auto_backup_mode: Option<String>,
    #[serde(default)]
    pub auto_backup_keep: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineInput {
    pub key: String,
    pub value: bool,
}

pub fn save(conn: &Connection, input: &SettingsInput) -> AppResult<()> {
    for e in &input.engine {
        if !ENGINE_OPTIONS.iter().any(|d| d.key == e.key) {
            return Err(AppError::invalid(format!(
                "알 수 없는 설정입니다: {}",
                e.key
            )));
        }
        put(conn, &e.key, if e.value { "true" } else { "false" })?;
    }

    if let Some(m) = &input.auto_backup_mode {
        if !matches!(m.as_str(), BACKUP_OFF | BACKUP_DAILY | BACKUP_ON_EXIT) {
            return Err(AppError::invalid("자동 백업 방식이 올바르지 않습니다."));
        }
        put(conn, AUTO_BACKUP_MODE, &format!("\"{m}\""))?;
    }
    if let Some(k) = input.auto_backup_keep {
        let k = k.clamp(3, 60);
        put(conn, AUTO_BACKUP_KEEP, &k.to_string())?;
    }
    Ok(())
}

/// **동작 옵션만** 기본값으로 되돌린다. 자료는 건드리지 않는다.
pub fn reset(conn: &Connection) -> AppResult<()> {
    for d in ENGINE_OPTIONS {
        put(conn, d.key, if d.default { "true" } else { "false" })?;
    }
    put(conn, AUTO_BACKUP_MODE, &format!("\"{BACKUP_DAILY}\""))?;
    put(conn, AUTO_BACKUP_KEEP, "10")?;
    Ok(())
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
