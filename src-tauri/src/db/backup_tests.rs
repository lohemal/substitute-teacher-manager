//! Phase 10 검증 — 진짜 파일로 백업하고 되돌리기.
//!
//! 메모리 DB로는 확인할 수 없는 부분이다. 임시 폴더에 실제 파일을 만들어
//! **백업 → 자료 변경 → 복원 → 되돌아왔는지**를 본다.

use super::*;

fn tmp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "bogyeol-db-test-{tag}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn open_at(dir: &Path) -> Db {
    Db::open(&dir.join("bogyeol.db")).unwrap()
}

fn add_teacher(db: &Db, name: &str) {
    db.write(|c| {
        c.execute(
            "INSERT INTO teachers(name, role_code) VALUES (?1, 'HOMEROOM')",
            [name],
        )?;
        Ok(())
    })
    .unwrap();
}

fn names(db: &Db) -> Vec<String> {
    db.read(|c| {
        let v: Vec<String> = c
            .prepare("SELECT name FROM teachers ORDER BY id")?
            .query_map([], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        Ok(v)
    })
    .unwrap()
}

#[test]
fn 백업을_만들고_그대로_되돌린다() {
    let dir = tmp_dir("roundtrip");
    let db = open_at(&dir);
    add_teacher(&db, "김철수");
    add_teacher(&db, "이영희");
    assert_eq!(names(&db), vec!["김철수", "이영희"]);

    // 백업
    let backup = dir.join("backups").join("보결자료-시험.db");
    db.backup_to(&backup).unwrap();
    assert!(backup.exists());

    // 자료를 바꾼다
    add_teacher(&db, "박민수");
    db.write(|c| {
        c.execute("DELETE FROM teachers WHERE name = '김철수'", [])?;
        Ok(())
    })
    .unwrap();
    assert_eq!(names(&db), vec!["이영희", "박민수"]);

    // 되돌린다
    let safety = dir.join("backups").join("before-restore-시험.db");
    db.restore_from(&backup, &safety).unwrap();

    assert_eq!(names(&db), vec!["김철수", "이영희"], "백업 시점으로 돌아온다");
    assert!(safety.exists(), "복원 직전 자료도 백업해 둔다");
}

#[test]
fn 복원_직전_백업으로_다시_되돌릴_수_있다() {
    let dir = tmp_dir("undo");
    let db = open_at(&dir);
    add_teacher(&db, "처음");

    let old = dir.join("backups").join("old.db");
    db.backup_to(&old).unwrap();

    add_teacher(&db, "나중");
    assert_eq!(names(&db), vec!["처음", "나중"]);

    // 되돌렸다가
    let safety = dir.join("backups").join("safety.db");
    db.restore_from(&old, &safety).unwrap();
    assert_eq!(names(&db), vec!["처음"]);

    // 마음이 바뀌어 다시 원래대로
    let safety2 = dir.join("backups").join("safety2.db");
    db.restore_from(&safety, &safety2).unwrap();
    assert_eq!(names(&db), vec!["처음", "나중"], "안전 백업으로 되살릴 수 있다");
}

#[test]
fn 복원_뒤에도_계속_쓸_수_있다() {
    let dir = tmp_dir("after");
    let db = open_at(&dir);
    add_teacher(&db, "가");

    let backup = dir.join("backups").join("b.db");
    db.backup_to(&backup).unwrap();
    add_teacher(&db, "나");

    db.restore_from(&backup, &dir.join("backups").join("s.db"))
        .unwrap();

    // 읽기·쓰기 모두 정상이어야 한다
    add_teacher(&db, "다");
    assert_eq!(names(&db), vec!["가", "다"]);
    assert_eq!(db.schema_version().unwrap(), migrate::latest_version());
}

#[test]
fn 자료_파일을_새로_열어도_복원_결과가_남아_있다() {
    let dir = tmp_dir("reopen");
    {
        let db = open_at(&dir);
        add_teacher(&db, "지워질사람");
        let backup = dir.join("backups").join("b.db");
        db.backup_to(&backup).unwrap();

        add_teacher(&db, "임시");
        db.restore_from(&backup, &dir.join("backups").join("s.db"))
            .unwrap();
        assert_eq!(names(&db), vec!["지워질사람"]);
    }

    // 앱을 다시 켠 것과 같은 상황
    let db2 = open_at(&dir);
    assert_eq!(names(&db2), vec!["지워질사람"], "복원 결과가 파일에 남는다");
}

#[test]
fn 백업은_WAL에_남은_내용까지_담는다() {
    let dir = tmp_dir("wal");
    let db = open_at(&dir);
    // 방금 쓴 내용은 아직 WAL에만 있을 수 있다. 파일 복사가 아니라
    // SQLite 백업 API를 쓰므로 그것까지 들어가야 한다.
    add_teacher(&db, "방금쓴사람");

    let backup = dir.join("backups").join("b.db");
    db.backup_to(&backup).unwrap();

    let c = rusqlite::Connection::open_with_flags(
        &backup,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .unwrap();
    let n: i64 = c
        .query_row(
            "SELECT COUNT(*) FROM teachers WHERE name = '방금쓴사람'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(n, 1);
}

#[test]
fn 백업은_파일_하나로_만들어진다() {
    // USB로 옮길 때 한 파일만 복사해 가도 자료가 온전해야 한다
    let dir = tmp_dir("single");
    let db = open_at(&dir);
    add_teacher(&db, "김철수");

    let backup = dir.join("backups").join("하나.db");
    db.backup_to(&backup).unwrap();

    assert!(backup.exists());
    for side in ["하나.db-wal", "하나.db-shm"] {
        assert!(
            !dir.join("backups").join(side).exists(),
            "{side} 이 남으면 안 된다"
        );
    }

    // 그 한 파일만으로 읽을 수 있다
    let c = rusqlite::Connection::open_with_flags(
        &backup,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .unwrap();
    let n: i64 = c
        .query_row("SELECT COUNT(*) FROM teachers", [], |r| r.get(0))
        .unwrap();
    assert_eq!(n, 1);
}

#[test]
fn 잘못된_파일로_복원하면_원래_자료가_그대로다() {
    let dir = tmp_dir("bad");
    let db = open_at(&dir);
    add_teacher(&db, "그대로");

    let junk = dir.join("가짜.db");
    std::fs::write(&junk, "이건 데이터베이스가 아닙니다".as_bytes()).unwrap();

    let err = db
        .restore_from(&junk, &dir.join("backups").join("s.db"))
        .unwrap_err();
    assert_eq!(err.code, "RESTORE_FAILED");
    assert!(err.user_message.contains("원래 자료는 그대로"), "{}", err.user_message);

    // 쓰기도 여전히 된다
    add_teacher(&db, "추가");
    assert_eq!(names(&db), vec!["그대로", "추가"]);
}

#[test]
fn 예전_버전_백업은_열면서_자료_구조를_올린다() {
    let dir = tmp_dir("upgrade");
    let db = open_at(&dir);
    add_teacher(&db, "지금사람");

    // 001만 적용된 예전 자료 파일을 그대로 만든다 (버전만 낮추면 진짜가 아니다)
    let old = dir.join("v1.db");
    {
        let c = rusqlite::Connection::open(&old).unwrap();
        c.execute_batch(include_str!("../../migrations/001_init.sql"))
            .unwrap();
        c.pragma_update(None, "user_version", 1).unwrap();
        c.execute(
            "INSERT INTO teachers(name, role_code) VALUES ('옛날사람', 'HOMEROOM')",
            [],
        )
        .unwrap();
    }

    db.restore_from(&old, &dir.join("backups").join("s.db"))
        .unwrap();

    assert_eq!(
        db.schema_version().unwrap(),
        migrate::latest_version(),
        "복원하면서 최신 구조로 올린다"
    );
    assert_eq!(names(&db), vec!["옛날사람"]);

    // 새 구조의 열이 실제로 생겼는지 (002가 더한 것)
    db.read(|c| {
        c.query_row("SELECT COUNT(name) FROM classes", [], |r| r.get::<_, i64>(0))?;
        Ok(())
    })
    .unwrap();
}
