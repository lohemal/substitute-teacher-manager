//! Phase 10 검증 — 동작 옵션.

use super::*;
use crate::db::memory_conn;

fn v(conn: &Connection) -> SettingsView {
    view(conn, "db", "backups", "exports").unwrap()
}

fn on(view: &SettingsView, key: &str) -> bool {
    view.engine.iter().find(|o| o.key == key).unwrap().value
}

#[test]
fn 처음에는_모두_기본값이다() {
    let c = memory_conn();
    let s = v(&c);
    assert!(!s.changed, "설치 직후에는 바뀐 것이 없다");
    assert_eq!(s.engine.len(), ENGINE_OPTIONS.len());
    assert!(s.engine.iter().all(|o| o.is_default));
    assert_eq!(s.auto_backup_mode, BACKUP_DAILY, "기본은 하루 1회");
    assert_eq!(s.auto_backup_keep, 10);
    for o in &s.engine {
        assert!(!o.label.is_empty());
        assert!(!o.hint.is_empty(), "{}", o.key);
    }
}

#[test]
fn 옵션을_끄고_다시_읽으면_그대로다() {
    let c = memory_conn();
    save(
        &c,
        &SettingsInput {
            engine: vec![EngineInput { key: INCLUDE_SPECIAL.into(), value: false }],
            auto_backup_mode: None,
            auto_backup_keep: None,
        },
    )
    .unwrap();

    let s = v(&c);
    assert!(!on(&s, INCLUDE_SPECIAL));
    assert!(s.changed);
    assert!(on(&s, INCLUDE_OTHER_GRADE), "건드리지 않은 것은 그대로");
}

#[test]
fn 모르는_옵션은_막는다() {
    let c = memory_conn();
    let e = save(
        &c,
        &SettingsInput {
            engine: vec![EngineInput { key: "없는옵션".into(), value: true }],
            auto_backup_mode: None,
            auto_backup_keep: None,
        },
    )
    .unwrap_err();
    assert!(e.user_message.contains("알 수 없는"), "{}", e.user_message);
}

#[test]
fn 자동_백업_설정을_바꾼다() {
    let c = memory_conn();
    save(
        &c,
        &SettingsInput {
            engine: vec![],
            auto_backup_mode: Some(BACKUP_ON_EXIT.into()),
            auto_backup_keep: Some(5),
        },
    )
    .unwrap();
    let s = v(&c);
    assert_eq!(s.auto_backup_mode, BACKUP_ON_EXIT);
    assert_eq!(s.auto_backup_keep, 5);
}

#[test]
fn 보관_개수는_너무_작거나_크지_않게_맞춘다() {
    let c = memory_conn();
    for (put, want) in [(0, 3), (1, 3), (200, 60), (12, 12)] {
        save(
            &c,
            &SettingsInput { engine: vec![], auto_backup_mode: None, auto_backup_keep: Some(put) },
        )
        .unwrap();
        assert_eq!(v(&c).auto_backup_keep, want, "{put} -> {want}");
    }
}

#[test]
fn 자동_백업_방식이_이상하면_막는다() {
    let c = memory_conn();
    let e = save(
        &c,
        &SettingsInput { engine: vec![], auto_backup_mode: Some("아무거나".into()), auto_backup_keep: None },
    )
    .unwrap_err();
    assert!(e.user_message.contains("올바르지 않습니다"));
}

#[test]
fn 기본값으로_되돌려도_자료는_남는다() {
    let c = memory_conn();
    // 자료를 하나 넣어 둔다
    c.execute("INSERT INTO teachers(name, role_code) VALUES ('김철수','HOMEROOM')", [])
        .unwrap();

    save(
        &c,
        &SettingsInput {
            engine: vec![
                EngineInput { key: INCLUDE_SPECIAL.into(), value: false },
                EngineInput { key: EXCLUDE_LUNCH.into(), value: false },
            ],
            auto_backup_mode: Some(BACKUP_OFF.into()),
            auto_backup_keep: Some(3),
        },
    )
    .unwrap();
    assert!(v(&c).changed);

    reset(&c).unwrap();

    let s = v(&c);
    assert!(!s.changed, "모두 기본값으로 돌아온다");
    assert!(on(&s, INCLUDE_SPECIAL));
    assert_eq!(s.auto_backup_mode, BACKUP_DAILY);

    let n: i64 = c
        .query_row("SELECT COUNT(*) FROM teachers", [], |r| r.get(0))
        .unwrap();
    assert_eq!(n, 1, "설정 초기화는 자료를 지우지 않는다");
}

#[test]
fn 시간_충돌은_옵션으로_끌_수_없다() {
    // 옵션 목록에 '겹쳐도 후보에 넣기' 같은 것이 없어야 한다
    for d in ENGINE_OPTIONS {
        assert!(!d.key.contains("overlap"), "{}", d.key);
        assert!(!d.key.contains("conflict"), "{}", d.key);
        assert!(!d.label.contains("겹"), "{}", d.label);
    }
    assert!(FIXED_NOTE.contains("겹치는"));
}
