//! Phase 10 — 자료 백업 · 복원.
//!
//! ## 되돌릴 수 없는 일은 하지 않는다
//!
//! 복원은 지금 자료를 덮어쓰는 일이다. 그래서 **복원 직전에 지금 자료를 먼저
//! 백업**하고(`before-restore-…`), 백업 파일이 정말 이 프로그램의 자료인지
//! 확인한 뒤에야 진행한다. 잘못된 파일이면 아예 시작하지 않는다.
//!
//! ## 자동 백업
//!
//! 하루 1회 또는 앱 종료 시 자동으로 남긴다. 자동 백업만 개수를 제한해
//! 지우고, **사람이 직접 만든 백업은 지우지 않는다** — 지워서 곤란해지는
//! 쪽이 파일이 조금 쌓이는 쪽보다 훨씬 나쁘다.

use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags};
use serde::Serialize;

use crate::db::migrate;
use crate::error::{AppError, AppResult};

/// 사람이 직접 만든 백업
pub const KIND_MANUAL: &str = "MANUAL";
/// 자동 백업 (개수 제한 대상)
pub const KIND_AUTO: &str = "AUTO";
/// 복원 직전에 남긴 안전 백업
pub const KIND_SAFETY: &str = "SAFETY";
/// 자료 구조 업데이트 전 백업 (migrate 가 만든다)
pub const KIND_UPGRADE: &str = "UPGRADE";

fn kind_of(name: &str) -> &'static str {
    if name.starts_with("auto-") {
        KIND_AUTO
    } else if name.starts_with("before-restore-") || name.starts_with("before-reset-") {
        KIND_SAFETY
    } else if name.starts_with("bogyeol-v") {
        KIND_UPGRADE
    } else {
        KIND_MANUAL
    }
}

pub fn kind_label(kind: &str) -> &'static str {
    match kind {
        KIND_AUTO => "자동",
        KIND_SAFETY => "복원 전 안전 백업",
        KIND_UPGRADE => "자료 구조 업데이트 전",
        _ => "직접 만듦",
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupFile {
    pub name: String,
    pub path: String,
    /// MANUAL | AUTO | SAFETY | UPGRADE
    pub kind: String,
    pub kind_label: String,
    /// 'YYYY-MM-DD HH:MM'
    pub made_at: String,
    pub size_kb: i64,
    /// 이 파일의 자료 구조 버전 (읽을 수 없으면 None)
    pub schema_version: Option<i32>,
    /// 지금 프로그램에서 복원할 수 있는가
    pub restorable: bool,
    /// 복원할 수 없다면 그 이유
    pub problem: Option<String>,
}

fn stamp() -> String {
    chrono::Local::now().format("%Y%m%d-%H%M%S").to_string()
}

pub fn manual_name() -> String {
    format!("보결자료-{}.db", stamp())
}

pub fn auto_name() -> String {
    format!("auto-{}.db", stamp())
}

pub fn safety_name(reason: &str) -> String {
    format!("before-{reason}-{}.db", stamp())
}

// ============================================================
//  확인
// ============================================================

/// 이 파일이 정말 우리 프로그램의 자료인지 확인한다.
///
/// 확장자만 보고 믿지 않는다. 실제로 열어서 스키마 버전과 꼭 있어야 할
/// 표가 들어 있는지 본다.
pub fn inspect(path: &Path) -> (Option<i32>, Option<String>) {
    if !path.exists() {
        return (None, Some("파일을 찾을 수 없습니다.".into()));
    }
    let conn = match Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY) {
        Ok(c) => c,
        Err(_) => {
            return (
                None,
                Some("이 프로그램의 자료 파일이 아닙니다. (열 수 없음)".into()),
            )
        }
    };

    let version: Option<i32> = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .ok();

    // 꼭 있어야 할 표들
    const MUST: &[&str] = &["school", "terms", "teachers", "classes", "substitutions"];
    for t in MUST {
        let found: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name = ?1",
                [t],
                |r| r.get(0),
            )
            .unwrap_or(0);
        if found == 0 {
            return (
                version,
                Some(format!(
                    "이 프로그램의 자료 파일이 아닙니다. ('{t}' 자료가 없습니다)"
                )),
            );
        }
    }

    match version {
        None => (
            None,
            Some("자료 구조 버전을 읽을 수 없습니다.".into()),
        ),
        Some(v) if v <= 0 => (
            version,
            Some("아직 자료가 만들어지지 않은 빈 파일입니다.".into()),
        ),
        Some(v) if v > migrate::latest_version() => (
            version,
            Some(format!(
                "더 최신 버전의 프로그램에서 만든 자료입니다 (v{v}). \
                 프로그램을 업데이트한 뒤 복원해 주세요."
            )),
        ),
        _ => (version, None),
    }
}

// ============================================================
//  목록 · 정리
// ============================================================

pub fn list(dir: &Path) -> AppResult<Vec<BackupFile>> {
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut out: Vec<BackupFile> = Vec::new();

    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|x| x.to_str()) != Some("db") {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        let meta = entry.metadata()?;
        let made = meta
            .modified()
            .ok()
            .map(chrono::DateTime::<chrono::Local>::from)
            .map(|t| t.format("%Y-%m-%d %H:%M").to_string())
            .unwrap_or_default();

        let (version, problem) = inspect(&path);
        tidy_sidecars(&path);
        let kind = kind_of(&name);
        out.push(BackupFile {
            name,
            path: path.display().to_string(),
            kind: kind.to_string(),
            kind_label: kind_label(kind).to_string(),
            made_at: made,
            size_kb: (meta.len() as i64 + 1023) / 1024,
            schema_version: version,
            restorable: problem.is_none(),
            problem,
        });
    }

    // 새것이 위로
    out.sort_by(|a, b| b.made_at.cmp(&a.made_at).then(b.name.cmp(&a.name)));
    Ok(out)
}

/// 백업을 읽고 나면 SQLite가 `-wal`·`-shm` 곁파일을 만들어 둘 때가 있다.
///
/// 예전에 만든 WAL 모드 백업을 열면 생기는데, 내용이 없는 빈 파일이다.
/// 그대로 두면 백업 폴더가 지저분해지고 USB로 옮길 때 헷갈리므로,
/// **비어 있는 것만** 지운다. (내용이 있는 `-wal`은 자료의 일부이므로 둔다)
fn tidy_sidecars(db_path: &Path) {
    // -shm 은 공유 메모리 인덱스일 뿐 자료가 아니다. 언제 지워도 다시 만들어진다.
    let shm = std::path::PathBuf::from(format!("{}-shm", db_path.display()));
    let _ = std::fs::remove_file(&shm);

    // -wal 에는 아직 본 파일에 반영되지 않은 내용이 들어 있을 수 있다.
    // **비어 있을 때만** 지운다.
    let wal = std::path::PathBuf::from(format!("{}-wal", db_path.display()));
    if std::fs::metadata(&wal).map(|m| m.len() == 0).unwrap_or(false) {
        let _ = std::fs::remove_file(&wal);
    }
}

/// 자동 백업을 최근 `keep`개만 남기고 지운다. **직접 만든 백업은 건드리지 않는다.**
pub fn prune_auto(dir: &Path, keep: usize) -> AppResult<usize> {
    let autos: Vec<BackupFile> = list(dir)?
        .into_iter()
        .filter(|b| b.kind == KIND_AUTO)
        .collect();
    let mut removed = 0;
    for b in autos.into_iter().skip(keep) {
        if std::fs::remove_file(&b.path).is_ok() {
            removed += 1;
            for ext in ["-wal", "-shm"] {
                let _ = std::fs::remove_file(format!("{}{ext}", b.path));
            }
        }
    }
    Ok(removed)
}

/// 오늘 만든 자동 백업이 있는가 (하루 1회 설정에 쓴다)
pub fn has_auto_today(dir: &Path) -> bool {
    let today = chrono::Local::now().format("%Y%m%d").to_string();
    list(dir)
        .map(|v| {
            v.iter()
                .any(|b| b.kind == KIND_AUTO && b.name.contains(&today))
        })
        .unwrap_or(false)
}

/// 목록에서 고른 파일 이름을 실제 경로로 바꾼다. **폴더 밖으로 나가지 못하게 한다.**
pub fn resolve_in_dir(dir: &Path, name: &str) -> AppResult<PathBuf> {
    let name = name.trim();
    if name.is_empty() || name.contains('/') || name.contains('\\') || name.contains("..") {
        return Err(AppError::invalid("백업 파일 이름이 올바르지 않습니다."));
    }
    let path = dir.join(name);
    if !path.exists() {
        return Err(AppError::not_found(
            "백업 파일을 찾을 수 없습니다. 목록을 새로 고쳐 주세요.",
        ));
    }
    Ok(path)
}

pub fn delete(dir: &Path, name: &str) -> AppResult<()> {
    let path = resolve_in_dir(dir, name)?;
    std::fs::remove_file(&path).map_err(|e| {
        AppError::new("DELETE_FAILED", "백업 파일을 지우지 못했습니다.").detail(e.to_string())
    })?;
    for ext in ["-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{ext}", path.display()));
    }
    Ok(())
}

/// 탐색기로 폴더를 연다. (Windows 전용)
pub fn open_folder(dir: &Path) -> AppResult<()> {
    std::fs::create_dir_all(dir)?;
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(dir.as_os_str())
            .spawn()
            .map_err(|e| {
                AppError::new("OPEN_FAILED", "폴더를 열지 못했습니다.").detail(e.to_string())
            })?;
        return Ok(());
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = dir;
        Err(AppError::new(
            "OPEN_FAILED",
            "이 환경에서는 폴더 열기를 지원하지 않습니다.",
        ))
    }
}

#[cfg(test)]
#[path = "backup_tests.rs"]
mod tests;
