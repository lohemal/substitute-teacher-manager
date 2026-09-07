//! 마이그레이션 러너.
//!
//! `PRAGMA user_version`을 스키마 버전으로 쓰고, 아래 목록을 번호순으로 적용한다.
//! 적용 전에 기존 DB 파일을 `backups/`에 자동 복사한다.
//!
//! 새 마이그레이션 추가 방법
//!   1. `migrations/00N_설명.sql` 파일 생성
//!   2. 아래 MIGRATIONS 배열에 한 줄 추가
//! 기존 파일은 절대 수정하지 않는다(이미 적용된 사용자가 있으므로).

use std::path::Path;

use rusqlite::Connection;

use crate::error::{AppError, AppResult};

struct Migration {
    version: i32,
    name: &'static str,
    sql: &'static str,
}

const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "001_init",
        sql: include_str!("../../migrations/001_init.sql"),
    },
    Migration {
        version: 2,
        name: "002_class_name",
        sql: include_str!("../../migrations/002_class_name.sql"),
    },
    Migration {
        version: 3,
        name: "003_lesson_time_unique",
        sql: include_str!("../../migrations/003_lesson_time_unique.sql"),
    },
    Migration {
        version: 4,
        name: "004_sub_pay",
        sql: include_str!("../../migrations/004_sub_pay.sql"),
    },
];

pub fn latest_version() -> i32 {
    MIGRATIONS.iter().map(|m| m.version).max().unwrap_or(0)
}

pub fn current_version(conn: &Connection) -> rusqlite::Result<i32> {
    conn.query_row("PRAGMA user_version", [], |r| r.get(0))
}

pub fn run(conn: &mut Connection, db_path: &Path) -> AppResult<()> {
    let from = current_version(conn)?;
    let to = latest_version();

    if from == to {
        return Ok(());
    }
    if from > to {
        return Err(AppError::new(
            "SCHEMA_TOO_NEW",
            "더 최신 버전의 프로그램에서 만든 자료입니다. 프로그램을 최신 버전으로 업데이트해 주세요.",
        )
        .detail(format!("db user_version={from}, app supports={to}")));
    }

    // 기존 자료가 있는 경우에만 백업 (최초 생성 시에는 백업할 것이 없음)
    if from > 0 {
        backup(db_path, from)?;
    }

    for m in MIGRATIONS.iter().filter(|m| m.version > from) {
        let tx = conn.transaction()?;
        tx.execute_batch(m.sql).map_err(|e| {
            AppError::new(
                "MIGRATION_FAILED",
                "자료 구조를 업데이트하지 못했습니다. 프로그램을 다시 시작해 주세요.",
            )
            .detail(format!("{} :: {e}", m.name))
        })?;
        tx.pragma_update(None, "user_version", m.version)?;
        tx.commit()?;
        log::info!("migration applied: {} (v{})", m.name, m.version);
    }

    Ok(())
}

fn backup(db_path: &Path, version: i32) -> AppResult<()> {
    if !db_path.exists() {
        return Ok(());
    }
    let dir = db_path
        .parent()
        .ok_or_else(|| AppError::internal("DB 경로에 상위 폴더가 없습니다."))?
        .join("backups");
    std::fs::create_dir_all(&dir)?;

    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let name = format!("bogyeol-v{version}-{stamp}.db");
    std::fs::copy(db_path, dir.join(name))?;
    Ok(())
}
