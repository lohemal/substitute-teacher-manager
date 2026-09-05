//! Phase 10 검증 — 백업 파일 확인과 정리.
//!
//! 실제 백업·복원(파일을 바꿔 끼우는 일)은 `db::tests`에서 확인한다.
//! 여기서는 **어떤 파일을 받아들이고 어떤 파일을 막는지**를 본다.

use super::*;

use rusqlite::Connection;

fn tmp_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "bogyeol-backup-test-{tag}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// 진짜 자료 파일 하나를 만든다.
fn make_db(path: &std::path::Path) {
    let mut conn = Connection::open(path).unwrap();
    crate::db::migrate::run(&mut conn, path).unwrap();
}

#[test]
fn 우리_자료_파일은_복원할_수_있다() {
    let dir = tmp_dir("ok");
    let f = dir.join("보결자료-20260905-090000.db");
    make_db(&f);

    let (version, problem) = inspect(&f);
    assert_eq!(problem, None, "문제가 없어야 한다");
    assert_eq!(version, Some(crate::db::migrate::latest_version()));

    let list = list(&dir).unwrap();
    assert_eq!(list.len(), 1);
    assert!(list[0].restorable);
    assert_eq!(list[0].kind, KIND_MANUAL);
    assert!(list[0].size_kb > 0);
}

#[test]
fn 우리_파일이_아니면_막는다() {
    let dir = tmp_dir("bad");

    // 그냥 텍스트 파일에 확장자만 붙인 것
    let junk = dir.join("가짜.db");
    std::fs::write(&junk, "이건 데이터베이스가 아닙니다".as_bytes()).unwrap();
    let (_, problem) = inspect(&junk);
    assert!(problem.is_some(), "열리지 않는 파일은 막아야 한다");

    // 열리기는 하지만 우리 표가 없는 SQLite 파일
    let other = dir.join("남의DB.db");
    {
        let c = Connection::open(&other).unwrap();
        c.execute("CREATE TABLE memo(id INTEGER)", []).unwrap();
    }
    let (_, problem) = inspect(&other);
    let msg = problem.expect("우리 자료가 아니라고 알려야 한다");
    assert!(msg.contains("이 프로그램의 자료 파일이 아닙니다"), "{msg}");

    // 목록에서도 복원 불가로 나온다
    let list = list(&dir).unwrap();
    assert_eq!(list.len(), 2);
    assert!(list.iter().all(|b| !b.restorable));
}

#[test]
fn 없는_파일은_막는다() {
    let dir = tmp_dir("missing");
    let (_, problem) = inspect(&dir.join("없어요.db"));
    assert!(problem.unwrap().contains("찾을 수 없습니다"));
}

#[test]
fn 더_최신_버전의_자료는_막는다() {
    let dir = tmp_dir("newer");
    let f = dir.join("미래.db");
    make_db(&f);
    {
        let c = Connection::open(&f).unwrap();
        c.pragma_update(None, "user_version", 999).unwrap();
    }
    let (v, problem) = inspect(&f);
    assert_eq!(v, Some(999));
    assert!(problem.unwrap().contains("더 최신 버전"));
}

#[test]
fn 폴더_밖의_파일은_이름으로_고를_수_없다() {
    let dir = tmp_dir("escape");
    for bad in ["../다른곳.db", "a/b.db", "..", ""] {
        assert!(
            resolve_in_dir(&dir, bad).is_err(),
            "막아야 한다: {bad:?}"
        );
    }
}

#[test]
fn 종류를_이름으로_알아본다() {
    let dir = tmp_dir("kind");
    for name in [
        "auto-20260905-090000.db",
        "before-restore-20260905-090000.db",
        "bogyeol-v3-20260905-090000.db",
        "보결자료-20260905-090000.db",
    ] {
        make_db(&dir.join(name));
    }
    let list = list(&dir).unwrap();
    let kind_of = |n: &str| {
        list.iter()
            .find(|b| b.name == n)
            .map(|b| b.kind.clone())
            .unwrap()
    };
    assert_eq!(kind_of("auto-20260905-090000.db"), KIND_AUTO);
    assert_eq!(kind_of("before-restore-20260905-090000.db"), KIND_SAFETY);
    assert_eq!(kind_of("bogyeol-v3-20260905-090000.db"), KIND_UPGRADE);
    assert_eq!(kind_of("보결자료-20260905-090000.db"), KIND_MANUAL);
}

#[test]
fn 자동_백업만_개수를_줄이고_직접_만든_것은_남긴다() {
    let dir = tmp_dir("prune");
    for i in 1..=5 {
        make_db(&dir.join(format!("auto-2026090{i}-090000.db")));
    }
    make_db(&dir.join("보결자료-20260901-090000.db"));
    make_db(&dir.join("before-restore-20260901-090000.db"));

    let removed = prune_auto(&dir, 3).unwrap();
    assert_eq!(removed, 2, "자동 백업 5개 중 2개를 지운다");

    let left = list(&dir).unwrap();
    assert_eq!(left.iter().filter(|b| b.kind == KIND_AUTO).count(), 3);
    assert_eq!(
        left.iter().filter(|b| b.kind == KIND_MANUAL).count(),
        1,
        "직접 만든 백업은 남는다"
    );
    assert_eq!(
        left.iter().filter(|b| b.kind == KIND_SAFETY).count(),
        1,
        "안전 백업도 남는다"
    );

    // 남은 자동 백업은 새것들이다
    let autos: Vec<String> = left
        .iter()
        .filter(|b| b.kind == KIND_AUTO)
        .map(|b| b.name.clone())
        .collect();
    assert!(autos.iter().any(|n| n.contains("20260905")));
    assert!(!autos.iter().any(|n| n.contains("20260901")));
}

#[test]
fn 오늘_자동_백업이_있는지_알아본다() {
    let dir = tmp_dir("today");
    assert!(!has_auto_today(&dir), "처음에는 없다");

    make_db(&dir.join(auto_name()));
    assert!(has_auto_today(&dir), "방금 만들었으니 있다");

    // 직접 만든 백업은 자동 백업으로 치지 않는다
    let dir2 = tmp_dir("today2");
    make_db(&dir2.join(manual_name()));
    assert!(!has_auto_today(&dir2));
}

#[test]
fn 백업_파일을_지운다() {
    let dir = tmp_dir("delete");
    let name = manual_name();
    make_db(&dir.join(&name));
    assert_eq!(list(&dir).unwrap().len(), 1);

    delete(&dir, &name).unwrap();
    assert!(list(&dir).unwrap().is_empty());
    assert!(delete(&dir, &name).is_err(), "없는 파일은 오류");
}

#[test]
fn 파일_이름에_날짜와_시각이_들어간다() {
    let today = chrono::Local::now().format("%Y%m%d").to_string();
    assert!(manual_name().contains(&today));
    assert!(auto_name().starts_with("auto-"));
    assert!(auto_name().contains(&today));
    assert!(safety_name("restore").starts_with("before-restore-"));
    // 같은 초에 두 번 불러도 확장자는 db
    assert!(manual_name().ends_with(".db"));
}

#[test]
fn 빈_폴더도_안전하다() {
    let dir = std::env::temp_dir().join("bogyeol-없는폴더-12345");
    assert!(list(&dir).unwrap().is_empty());
    assert_eq!(prune_auto(&dir, 3).unwrap(), 0);
}
