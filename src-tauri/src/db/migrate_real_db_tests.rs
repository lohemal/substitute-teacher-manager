//! 실제 쓰고 있는 자료로 마이그레이션 보존을 확인한다.
//!
//! **원본은 건드리지 않는다.** 읽기 전용으로 열어 임시 폴더에 통째로 복사한
//! 뒤, 그 복사본에서만 마이그레이션을 돌린다.
//!
//! ```bash
//! cd src-tauri && cargo test --lib real_db -- --ignored --nocapture
//! ```
//!
//! 자료가 없는 컴퓨터(예: CI)에서는 조용히 지나간다.

use rusqlite::Connection;

use super::migrate;

fn live_db_path() -> Option<std::path::PathBuf> {
    let appdata = std::env::var("APPDATA").ok()?;
    let p = std::path::Path::new(&appdata)
        .join("kr.school.bogyeol")
        .join("bogyeol.db");
    p.exists().then_some(p)
}

/// 원본을 임시 파일로 복사해 연다. WAL에 남은 내용까지 가져오려고 백업 API를 쓴다.
fn open_copy() -> Option<(Connection, std::path::PathBuf)> {
    let src = live_db_path()?;
    let dst = std::env::temp_dir().join(format!(
        "bogyeol-migrate-test-{}.db",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let live = Connection::open_with_flags(
        &src,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI,
    )
    .ok()?;
    let mut copy = Connection::open(&dst).ok()?;
    {
        let backup = rusqlite::backup::Backup::new(&live, &mut copy).ok()?;
        backup.run_to_completion(200, std::time::Duration::ZERO, None).ok()?;
    }
    Some((copy, dst))
}

/// 사용자가 잃으면 안 되는 것들. 마이그레이션 전후로 하나도 달라지면 안 된다.
const TABLES: &[(&str, &str)] = &[
    ("학교", "school"),
    ("학기", "terms"),
    ("학급", "classes"),
    ("교사", "teachers"),
    ("과목", "subjects"),
    ("교사-과목", "teacher_subjects"),
    ("시정표", "bell_schedules"),
    ("시정표 시각", "bell_slots"),
    ("학년-시정표", "grade_bell_map"),
    ("전담 시간표", "lessons"),
    ("결근", "absences"),
    ("보결 배정", "substitutions"),
    ("추천 기준", "priority_rules"),
    ("설정 단계", "setup_steps"),
];

fn count(c: &Connection, table: &str) -> i64 {
    c.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
        .unwrap_or(-1)
}

/// 표 하나하나의 내용까지 같은지 보려고, 정렬한 전체 행을 한 문자열로 만든다.
fn fingerprint(c: &Connection, table: &str) -> String {
    let sql = format!(
        "SELECT group_concat(x, '|') FROM (SELECT quote(t.*) AS x FROM {table} t ORDER BY 1)"
    );
    c.query_row(&sql, [], |r| r.get::<_, Option<String>>(0))
        .unwrap_or(None)
        .unwrap_or_default()
}

#[test]
#[ignore = "실제 자료가 있는 컴퓨터에서만 의미가 있다"]
fn real_db_마이그레이션_후에도_자료가_그대로다() {
    let Some((mut conn, path)) = open_copy() else {
        println!("실제 자료가 없어 건너뜁니다.");
        return;
    };

    let was = migrate::current_version(&conn).unwrap();
    println!("복사본 스키마 버전 v{was} → 최신 v{}", migrate::latest_version());

    // 보결 수당 마이그레이션(v4) 직전 상태로 되돌려, 실제 업그레이드를 재현한다.
    // 이미 v4 이상이어도 004 는 INSERT OR IGNORE 이므로 몇 번 돌려도 같다.
    conn.pragma_update(None, "user_version", 3).unwrap();

    let before: Vec<(String, i64, String)> = TABLES
        .iter()
        .map(|(ko, t)| ((*ko).to_string(), count(&conn, t), fingerprint(&conn, t)))
        .collect();
    let settings_before = count(&conn, "settings");

    migrate::run(&mut conn, &path).expect("마이그레이션이 실패하면 안 된다");

    assert_eq!(
        migrate::current_version(&conn).unwrap(),
        migrate::latest_version(),
        "최신 버전으로 올라가야 한다"
    );

    println!("\n마이그레이션 전후 비교");
    for ((ko, n_before, fp_before), (_, t)) in before.iter().zip(TABLES.iter()) {
        let n_after = count(&conn, t);
        let fp_after = fingerprint(&conn, t);
        assert_eq!(
            *n_before, n_after,
            "{ko}({t}) 행 수가 달라졌다: {n_before} → {n_after}"
        );
        assert_eq!(
            *fp_before, fp_after,
            "{ko}({t}) 내용이 달라졌다"
        );
        println!("  ok   {ko:<12} {n_after}건 (내용 동일)");
    }

    // 설정은 두 줄만 늘어난다 (이미 있었다면 그대로)
    let settings_after = count(&conn, "settings");
    assert!(
        settings_after >= settings_before && settings_after <= settings_before + 2,
        "설정은 최대 두 줄만 늘어난다: {settings_before} → {settings_after}"
    );
    println!("  ok   설정         {settings_before}건 → {settings_after}건 (수당 설정 추가)");

    // 새 설정이 기본값으로 들어와 있다
    let cfg = crate::repo::settings::pay_config(&conn).unwrap();
    println!("  ok   수당 설정     1회 {}원 · {}", cfg.per_case, cfg.policy);
    assert!(cfg.per_case >= 0);
    assert!(crate::domain::pay::is_known_policy(&cfg.policy));

    // 그리고 그 자료로 수당을 계산해도 오류가 나지 않는다
    let v = crate::repo::pay::view(&conn, &crate::repo::pay::PayQuery::default()).unwrap();
    println!(
        "\n  ok   수당 조회     {} · 교사 {}명 · 보결 {}회 · 지급 인정 {}회 · {}원",
        v.range_label,
        v.summary.listed_teachers,
        v.summary.total_substituted,
        v.summary.total_payable,
        crate::domain::pay::won(v.summary.total_amount)
    );

    let _ = std::fs::remove_file(&path);
}

/// 실제 자료로 수당 계산이 배정 기록과 어긋나지 않는지 본다.
#[test]
#[ignore = "실제 자료가 있는 컴퓨터에서만 의미가 있다"]
fn real_db_수당은_배정_기록과_맞는다() {
    let Some((conn, path)) = open_copy() else {
        println!("실제 자료가 없어 건너뜁니다.");
        return;
    };

    // 기록이 있는 달을 찾는다
    let month: Option<String> = conn
        .query_row(
            "SELECT substr(date,1,7) FROM substitutions
              WHERE status = 'ASSIGNED' GROUP BY 1 ORDER BY COUNT(*) DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .ok();
    let Some(month) = month else {
        println!("배정 기록이 없어 건너뜁니다.");
        return;
    };

    let q = crate::repo::pay::PayQuery {
        mode: Some(crate::repo::pay::MODE_MONTH.into()),
        month: Some(month.clone()),
        ..Default::default()
    };
    let v = crate::repo::pay::view(&conn, &q).unwrap();

    // 그 달의 ASSIGNED 건수와 '총 보결 횟수'가 같아야 한다
    let assigned: i32 = conn
        .query_row(
            "SELECT COUNT(*) FROM substitutions
              WHERE status = 'ASSIGNED' AND date >= ?1 AND date <= ?2",
            rusqlite::params![v.from, v.to],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        v.summary.total_substituted, assigned,
        "총 보결 횟수는 그 기간의 ASSIGNED 건수와 같아야 한다"
    );

    // 취소분은 세지 않는다
    let cancelled: i32 = conn
        .query_row(
            "SELECT COUNT(*) FROM substitutions
              WHERE status = 'CANCELLED' AND date >= ?1 AND date <= ?2",
            rusqlite::params![v.from, v.to],
            |r| r.get(0),
        )
        .unwrap();

    println!(
        "  ok   {} · 배정 {assigned}건 / 취소 {cancelled}건 → 총 보결 {}회",
        v.range_label, v.summary.total_substituted
    );

    // 사람별 합계도 맞는지, 상세와 어긋나지 않는지
    let sum: i32 = v.rows.iter().map(|r| r.substituted).sum();
    assert_eq!(sum, assigned);

    for r in v.rows.iter().take(5) {
        let d = crate::repo::pay::detail(&conn, r.teacher_id, &q).unwrap();
        assert_eq!(d.substituted.len() as i32, r.substituted, "{}", r.name);
        assert_eq!(d.own_caused.len() as i32, r.own_caused, "{}", r.name);
        assert_eq!(d.amount, r.amount, "{}", r.name);
        println!(
            "  ok   {:<8} 보결 {}회 · 본인발생 {}회 · 인정 {}회 · {}원",
            r.name,
            r.substituted,
            r.own_caused,
            r.payable,
            crate::domain::pay::won(r.amount)
        );
    }

    let _ = std::fs::remove_file(&path);
}
