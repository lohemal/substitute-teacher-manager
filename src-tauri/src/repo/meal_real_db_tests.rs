//! 실제 쓰고 있는 자료로 전담교사 식사시간을 계산해 본다.
//!
//! **원본은 건드리지 않는다.** 읽기 전용으로 열어 임시 파일로 복사한 뒤,
//! 그 복사본에서만 설정을 바꿔 본다.
//!
//! ```bash
//! cd src-tauri && cargo test --lib real_db_식사시간 -- --ignored --nocapture
//! ```

use rusqlite::Connection;

use crate::domain::meal::{Meal, MealSource};
use crate::domain::schedule::resolve_meal;
use crate::domain::time::fmt_range;
use crate::repo::{find as repo_find, meal};

const DAY: [&str; 8] = ["", "월", "화", "수", "목", "금", "토", "일"];

fn live_db_path() -> Option<std::path::PathBuf> {
    let appdata = std::env::var("APPDATA").ok()?;
    let p = std::path::Path::new(&appdata)
        .join("kr.school.bogyeol")
        .join("bogyeol.db");
    p.exists().then_some(p)
}

fn open_copy() -> Option<Connection> {
    let src = live_db_path()?;
    let dst = std::env::temp_dir().join(format!(
        "bogyeol-meal-real-{}.db",
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
    copy.pragma_update(None, "foreign_keys", "ON").ok()?;
    // 앱이 열 때와 같게 최신 구조로 올린다 (복사본에만).
    crate::db::migrate::run(&mut copy, &dst).ok()?;
    Some(copy)
}

/// 그 요일에 해당하는 가까운 날짜.
fn date_for(day: i32) -> String {
    let today = chrono::Local::now().date_naive();
    let cur: i32 = today.format("%u").to_string().parse().unwrap_or(1);
    (today + chrono::Duration::days(((day - cur).rem_euclid(7)) as i64))
        .format("%Y-%m-%d")
        .to_string()
}

fn label(m: &Meal) -> String {
    match m {
        Meal::Known { interval, source } => format!(
            "{} ({})",
            fmt_range(interval),
            match source {
                MealSource::Manual => "직접 지정",
                MealSource::Auto => "자동",
                MealSource::SchoolDefault => "학교 기본",
            }
        ),
        Meal::Unknown(why) => format!("확인 필요 — {}", why.message()),
    }
}

#[test]
#[ignore = "실제 자료가 있는 컴퓨터에서만 의미가 있다"]
fn real_db_식사시간_판정() {
    let Some(conn) = open_copy() else {
        println!("실제 자료가 없어 건너뜁니다.");
        return;
    };

    let patterns = meal::patterns(&conn).unwrap();
    println!("\n=== 점심 패턴 (시정표에서 뽑음) ===");
    for p in &patterns {
        println!("  {} {} · {}", p.grade_label, p.time_label, p.bell_name);
    }
    assert!(!patterns.is_empty(), "시정표에 점심이 있어야 한다");

    let specials: Vec<(i64, String)> = conn
        .prepare("SELECT id, name FROM teachers WHERE role_code = 'SPECIAL' ORDER BY name")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    if specials.is_empty() {
        println!("전담 선생님이 없어 건너뜁니다.");
        return;
    }

    // ---------- 기본값 없이 ----------
    println!("\n=== 기본 식사시간을 정하지 않았을 때 ===");
    let mut unknown_before = 0;
    for (id, name) in &specials {
        let mut row = format!("  {name:<8}");
        for d in 1..=5 {
            let snap = repo_find::snapshot(&conn, &date_for(d)).unwrap();
            let m = resolve_meal(&snap, *id);
            if !matches!(m, Meal::Known { .. }) {
                unknown_before += 1;
            }
            row.push_str(&format!(" {}:{}", DAY[d as usize], short(&m)));
        }
        println!("{row}");
    }
    println!("  → 정하지 못한 칸 {unknown_before}개");

    // ---------- 기본값을 정한 뒤 (복사본에만) ----------
    let pick = patterns[0].bell_schedule_id;
    meal::set_default_bell(&conn, Some(pick)).unwrap();
    println!(
        "\n=== 학교 기본을 '{} {}' 로 정했을 때 ===",
        patterns[0].grade_label, patterns[0].time_label
    );

    let mut unknown_after = 0;
    for (id, name) in &specials {
        let mut row = format!("  {name:<8}");
        for d in 1..=5 {
            let snap = repo_find::snapshot(&conn, &date_for(d)).unwrap();
            let m = resolve_meal(&snap, *id);
            if !matches!(m, Meal::Known { .. }) {
                unknown_after += 1;
            }
            row.push_str(&format!(" {}:{}", DAY[d as usize], short(&m)));
        }
        println!("{row}");
    }
    println!("  → 정하지 못한 칸 {unknown_after}개");
    assert!(
        unknown_after <= unknown_before,
        "기본값을 정하면 정하지 못하는 칸이 늘어나면 안 된다"
    );

    // ---------- 한 사람 자세히 ----------
    let (id, name) = &specials[0];
    println!("\n=== {name} 선생님 (자세히) ===");
    for d in 1..=5 {
        let snap = repo_find::snapshot(&conn, &date_for(d)).unwrap();
        let lessons = crate::domain::schedule::teacher_lesson_intervals(&snap, *id);
        println!(
            "  {}요일  수업 {:<2}개 [{}]  → 식사 {}",
            DAY[d as usize],
            lessons.len(),
            lessons.iter().map(fmt_range).collect::<Vec<_>>().join(", "),
            label(&resolve_meal(&snap, *id))
        );
    }

    // ---------- 결정된 식사시간은 그 날 수업과 겹치지 않는다 ----------
    for (id, name) in &specials {
        for d in 1..=5 {
            let snap = repo_find::snapshot(&conn, &date_for(d)).unwrap();
            let lessons = crate::domain::schedule::teacher_lesson_intervals(&snap, *id);
            if let Meal::Known { interval, source } = resolve_meal(&snap, *id) {
                if source == MealSource::Manual {
                    continue; // 직접 지정은 저장할 때 이미 막는다
                }
                assert!(
                    !lessons.iter().any(|l| l.overlaps(&interval)),
                    "{name} {}요일: 식사 {} 가 수업과 겹친다",
                    DAY[d as usize],
                    fmt_range(&interval)
                );
            }
        }
    }
    println!("\n  ok   정해진 식사시간은 모두 그 날 수업과 겹치지 않는다");
}

fn short(m: &Meal) -> String {
    match m {
        Meal::Known { interval, .. } => fmt_range(interval),
        Meal::Unknown(_) => "확인필요".to_string(),
    }
}
