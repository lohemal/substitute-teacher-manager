//! SQLite 연결 관리.
//!
//! 1인 사용 데스크톱 앱이므로 커넥션 풀 대신 `Mutex<Connection>` 하나로 충분하다.
//! 모든 DB 접근은 `Db::read` / `Db::write`를 통해서만 이루어진다.

pub mod migrate;

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use rusqlite::Connection;

use crate::error::{AppError, AppResult};

pub struct Db {
    conn: Mutex<Connection>,
    path: PathBuf,
}

impl Db {
    /// DB 파일을 열고 필요한 마이그레이션을 적용한다.
    pub fn open(path: &Path) -> AppResult<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }

        let mut conn = Connection::open(path).map_err(|e| {
            AppError::new(
                "DB_OPEN_FAILED",
                "자료 파일을 열지 못했습니다. 프로그램을 다시 시작해 주세요.",
            )
            .detail(format!("{} :: {e}", path.display()))
        })?;

        setup_conn(&conn)?;
        migrate::run(&mut conn, path)?;

        Ok(Self {
            conn: Mutex::new(conn),
            path: path.to_path_buf(),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// 읽기 전용 작업.
    pub fn read<T>(&self, f: impl FnOnce(&Connection) -> AppResult<T>) -> AppResult<T> {
        let guard = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        f(&guard)
    }

    /// 쓰기 작업. 클로저가 Err를 반환하면 전체가 롤백된다.
    pub fn write<T>(&self, f: impl FnOnce(&Connection) -> AppResult<T>) -> AppResult<T> {
        let mut guard = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let tx = guard.transaction()?;
        let out = f(&tx)?;
        tx.commit()?;
        Ok(out)
    }

    /// 현재 스키마 버전.
    pub fn schema_version(&self) -> AppResult<i32> {
        self.read(|c| Ok(migrate::current_version(c)?))
    }

    /// 백업 등이 놓이는 폴더 (`<자료 폴더>/backups`).
    pub fn backup_dir(&self) -> PathBuf {
        self.path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("backups")
    }

    /// 내보내기 파일이 놓이는 폴더.
    pub fn export_dir(&self) -> PathBuf {
        self.path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("exports")
    }

    /// 지금 자료를 통째로 `dest`에 복사한다.
    ///
    /// 파일을 그대로 복사하지 않고 SQLite 백업 API를 쓴다. WAL에만 남아 있는
    /// 내용까지 포함되고, 쓰는 도중에도 안전하기 때문이다.
    pub fn backup_to(&self, dest: &Path) -> AppResult<()> {
        if let Some(dir) = dest.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let guard = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        copy_db(&guard, dest)
    }

    /// 백업 파일로 되돌린다.
    ///
    /// 1. 지금 자료를 먼저 백업한다 (되돌리기 전 마지막 안전장치)
    /// 2. 열려 있던 연결을 닫는다
    /// 3. 백업 내용을 자료 파일에 통째로 옮긴다
    /// 4. 다시 열고, 필요하면 마이그레이션을 적용한다
    ///
    /// 어느 단계에서 실패해도 1에서 만든 백업으로 되돌릴 수 있다.
    pub fn restore_from(&self, src: &Path, safety_copy: &Path) -> AppResult<()> {
        let mut guard = self.conn.lock().unwrap_or_else(|e| e.into_inner());

        // 1. 지금 자료 백업
        if let Some(dir) = safety_copy.parent() {
            std::fs::create_dir_all(dir)?;
        }
        copy_db(&guard, safety_copy)?;

        // 2. 연결을 닫는다 (자리를 비워 두기 위해 메모리 DB로 바꾼다)
        *guard = Connection::open_in_memory()?;

        // 3. 백업 -> 자료 파일
        let restore = (|| -> AppResult<Connection> {
            let src_conn = Connection::open_with_flags(
                src,
                rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
            )?;
            let mut dest = Connection::open(&self.path)?;
            {
                let b = rusqlite::backup::Backup::new(&src_conn, &mut dest)?;
                b.run_to_completion(500, std::time::Duration::ZERO, None)?;
            }
            Ok(dest)
        })();

        // 4. 다시 연다
        let mut conn = match restore {
            Ok(c) => c,
            Err(e) => {
                // 되돌리기가 실패했다 — 방금 만든 안전 백업으로 원래대로
                let recovered = (|| -> AppResult<Connection> {
                    let src_conn = Connection::open_with_flags(
                        safety_copy,
                        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
                    )?;
                    let mut dest = Connection::open(&self.path)?;
                    {
                        let b = rusqlite::backup::Backup::new(&src_conn, &mut dest)?;
                        b.run_to_completion(500, std::time::Duration::ZERO, None)?;
                    }
                    Ok(dest)
                })();
                match recovered {
                    Ok(c) => {
                        setup_conn(&c)?;
                        *guard = c;
                        return Err(AppError::new(
                            "RESTORE_FAILED",
                            "복원하지 못했습니다. 원래 자료는 그대로 있습니다.",
                        )
                        .detail(e.to_string()));
                    }
                    Err(e2) => {
                        return Err(AppError::new(
                            "RESTORE_FAILED",
                            format!(
                                "복원에 실패했고 원래 자료도 되돌리지 못했습니다.                                  프로그램을 닫고 백업 폴더의 '{}' 파일을 자료 파일로                                  직접 바꿔 주세요.",
                                safety_copy.file_name().unwrap_or_default().to_string_lossy()
                            ),
                        )
                        .detail(format!("{e} / {e2}")));
                    }
                }
            }
        };

        setup_conn(&conn)?;
        migrate::run(&mut conn, &self.path)?;
        *guard = conn;
        Ok(())
    }
}

fn setup_conn(conn: &Connection) -> AppResult<()> {
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    Ok(())
}

fn copy_db(from: &Connection, dest: &Path) -> AppResult<()> {
    let mut out = Connection::open(dest)?;
    {
        let b = rusqlite::backup::Backup::new(from, &mut out)?;
        b.run_to_completion(500, std::time::Duration::ZERO, None)?;
    }
    // 백업본은 WAL을 쓰지 않게 한다.
    //
    // 원본이 WAL 모드라 복사본도 WAL로 열리는데, 그러면 백업 하나에
    // `.db` `.db-wal` `.db-shm` 세 파일이 생긴다. USB로 옮길 때 하나만
    // 복사해 가면 자료가 깨지므로, 한 파일로 합쳐 둔다.
    let _: String = out.pragma_update_and_check(None, "journal_mode", "DELETE", |r| r.get(0))?;
    Ok(())
}

#[cfg(test)]
#[path = "backup_tests.rs"]
mod backup_tests;

#[cfg(test)]
#[path = "migrate_real_db_tests.rs"]
mod migrate_real_db_tests;

/// 테스트용: 실제 마이그레이션을 적용한 메모리 DB.
#[cfg(test)]
pub fn memory_conn() -> Connection {
    let mut conn = Connection::open_in_memory().expect("메모리 DB를 열지 못했습니다");
    conn.pragma_update(None, "foreign_keys", "ON").unwrap();
    migrate::run(&mut conn, Path::new(":memory:")).expect("마이그레이션 실패");
    conn
}
