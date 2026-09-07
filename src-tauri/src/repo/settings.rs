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

use crate::domain::pay;
use crate::error::{AppError, AppResult};

// ---------- 보결 판단 ----------
pub const DERIVE_HOMEROOM: &str = "derive_homeroom_schedule";
pub const EXCLUDE_LUNCH: &str = "exclude_homeroom_on_own_lunch";
pub const EXCLUDE_RECESS: &str = "exclude_homeroom_on_recess";
pub const INCLUDE_SPECIAL: &str = "include_special_teachers";
pub const INCLUDE_AFTER_END: &str = "include_after_school_end";
pub const INCLUDE_OTHER_GRADE: &str = "include_other_grade_homeroom";

// ---------- 보결 수당 ----------
//
// 계산 결과는 저장하지 않는다. 여기 있는 것은 **학교가 정한 규정**뿐이고,
// 금액은 조회할 때마다 배정 기록에서 다시 계산한다.
pub const SUB_PAY_PER_CASE: &str = "sub_pay_per_case";
pub const SUB_PAY_POLICY: &str = "sub_pay_policy";

/// 1회 보결 수당의 상한. 오타로 0을 더 붙였을 때를 막는 정도의 값이다.
pub const SUB_PAY_MAX: i64 = 1_000_000;

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

/// 지급 기준 하나. 정의는 `domain::pay` 한 곳에만 있다.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyRow {
    pub code: String,
    pub label: String,
    pub hint: String,
}

pub fn policy_rows() -> Vec<PolicyRow> {
    pay::all_policies()
        .iter()
        .map(|r| PolicyRow {
            code: r.code().to_string(),
            label: r.label().to_string(),
            hint: r.hint().to_string(),
        })
        .collect()
}

/// 지금 저장된 수당 설정. 수당 화면과 설정 화면이 같은 값을 본다.
pub fn pay_config(conn: &Connection) -> AppResult<pay::PayConfig> {
    let per_case = get_int(conn, SUB_PAY_PER_CASE, pay::DEFAULT_PER_CASE)?.clamp(0, SUB_PAY_MAX);
    let policy = get_text(conn, SUB_PAY_POLICY, pay::DEFAULT_POLICY)?;
    let policy = if pay::is_known_policy(&policy) {
        policy
    } else {
        pay::DEFAULT_POLICY.to_string()
    };
    Ok(pay::PayConfig { policy, per_case })
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsView {
    pub engine: Vec<OptionRow>,
    /// OFF | DAILY | ON_EXIT
    pub auto_backup_mode: String,
    pub auto_backup_keep: i64,
    /// 1회 보결 수당(원)
    pub sub_pay_per_case: i64,
    /// ALL_ASSIGNED | DEDUCT_OWN_CAUSED
    pub sub_pay_policy: String,
    /// 고를 수 있는 지급 기준 — 화면은 이 목록만 보고 그린다
    pub sub_pay_policies: Vec<PolicyRow>,
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

    // 수당 설정은 '기본값과 다른가' 표시에 넣지 않는다. 금액을 정하는 것은
    // 동작 방식을 바꾼 것이 아니라 학교 규정을 적어 둔 것이다.
    let cfg = pay_config(conn)?;

    Ok(SettingsView {
        engine,
        auto_backup_mode: mode,
        auto_backup_keep: keep,
        sub_pay_per_case: cfg.per_case,
        sub_pay_policy: cfg.policy,
        sub_pay_policies: policy_rows(),
        changed,
        db_path: db_path.to_string(),
        backup_dir: backup_dir.to_string(),
        export_dir: export_dir.to_string(),
        fixed_note: FIXED_NOTE.to_string(),
    })
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsInput {
    /// 바꿀 옵션만 담아도 된다
    #[serde(default)]
    pub engine: Vec<EngineInput>,
    #[serde(default)]
    pub auto_backup_mode: Option<String>,
    #[serde(default)]
    pub auto_backup_keep: Option<i64>,
    #[serde(default)]
    pub sub_pay_per_case: Option<i64>,
    #[serde(default)]
    pub sub_pay_policy: Option<String>,
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

    if let Some(v) = input.sub_pay_per_case {
        if v < 0 {
            return Err(AppError::invalid("1회 보결 수당은 0원 이상이어야 합니다."));
        }
        if v > SUB_PAY_MAX {
            return Err(AppError::invalid(format!(
                "1회 보결 수당이 너무 큽니다. {}원 이하로 입력해 주세요.",
                pay::won(SUB_PAY_MAX)
            )));
        }
        put(conn, SUB_PAY_PER_CASE, &v.to_string())?;
    }
    if let Some(p) = &input.sub_pay_policy {
        if !pay::is_known_policy(p) {
            return Err(AppError::invalid("알 수 없는 보결 수당 지급 기준입니다."));
        }
        put(conn, SUB_PAY_POLICY, &format!("\"{p}\""))?;
    }
    Ok(())
}

/// **동작 옵션만** 기본값으로 되돌린다. 자료는 건드리지 않는다.
///
/// 보결 수당(1회 금액·지급 기준)은 여기서 되돌리지 않는다. 학교가 정해 적어
/// 둔 규정이라서, 판단 옵션을 되돌리려던 사람이 금액까지 0원으로 잃는 것은
/// 놀라운 일이다. 수당은 수당 설정에서 직접 고친다.
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
