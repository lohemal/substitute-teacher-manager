//! 실제 자료로 세 가지 엑셀 파일을 만들어 **바탕화면 임시 폴더에 남긴다.**
//!
//! 눈으로 열어 확인할 수 있도록 지우지 않는다. 실행하면 만들어진 경로와
//! 시트·열 구성을 찍어 준다.
//!
//! ```bash
//! cd src-tauri && cargo test --lib real_db_엑셀 -- --ignored --nocapture
//! ```
//!
//! **원본은 건드리지 않는다** — 읽기 전용으로 열어 임시 파일로 복사한 뒤,
//! 그 복사본에서만 조회한다.

use rusqlite::Connection;

use crate::repo::assign::HistoryFilter;
use crate::repo::pay::{self, PayQuery};
use crate::repo::stats::StatsQuery;
use crate::repo::xlsx::{Cell, Sheet};
use crate::repo::export;

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
        "bogyeol-xlsx-real-{}.db",
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
    Some(copy)
}

fn kind(c: &Cell) -> &'static str {
    match c {
        Cell::Text(_) => "글자",
        Cell::Count(_) => "숫자",
        Cell::Money(_) => "금액",
        Cell::Date(_) => "날짜",
        Cell::DateTime(_) => "날짜+시각",
        Cell::Time(_) => "시각",
        Cell::Blank => "빈칸",
    }
}

/// 시트 하나를 사람이 읽을 수 있게 찍는다 — 열 이름, 첫 줄의 값 종류, 줄 수.
fn describe(s: &Sheet) {
    println!(
        "  [{}]  {}줄{}{}",
        s.name,
        s.rows.len(),
        if s.filter { " · 자동필터" } else { "" },
        if s.total.is_some() { " · 합계행" } else { "" },
    );
    let head: Vec<String> = s
        .columns
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let k = s.rows.first().map(|r| kind(&r[i])).unwrap_or("-");
            format!("{}({k}, 너비 {})", c.title, c.width)
        })
        .collect();
    println!("     열: {}", head.join(" | "));
    if let Some(r) = s.rows.first() {
        let vals: Vec<String> = r
            .iter()
            .map(|c| match c {
                Cell::Text(v) => v.clone(),
                Cell::Count(n) => n.to_string(),
                Cell::Money(n) => format!("{}원", crate::domain::pay::won(*n)),
                Cell::Date(v) | Cell::DateTime(v) => v.clone(),
                Cell::Time(m) => format!("{:02}:{:02}", m / 60, m % 60),
                Cell::Blank => "-".into(),
            })
            .collect();
        println!("     첫 줄: {}", vals.join(" | "));
    }
}

/// 표 시트는 숫자여야 하는 열이 정말 숫자인지 본다.
fn assert_numeric(s: &Sheet, titles: &[&str]) {
    for t in titles {
        let Some(i) = s.columns.iter().position(|c| &c.title == t) else {
            continue;
        };
        for (ri, r) in s.rows.iter().enumerate() {
            assert!(
                matches!(r[i], Cell::Count(_) | Cell::Money(_) | Cell::Blank),
                "[{}] {ri}번째 줄 '{t}' 가 글자다: {:?}",
                s.name,
                r[i]
            );
        }
    }
}

#[test]
#[ignore = "실제 자료가 있는 컴퓨터에서만 의미가 있다"]
fn real_db_엑셀_파일_세_개를_만든다() {
    let Some(conn) = open_copy() else {
        println!("실제 자료가 없어 건너뜁니다.");
        return;
    };

    let dir = std::env::temp_dir().join("bogyeol-엑셀-확인");
    std::fs::create_dir_all(&dir).unwrap();
    println!("\n만들 곳: {}\n", dir.display());

    // ---------- 1) 배정 내역 ----------
    let f = HistoryFilter::default();
    let sheets = export::history_sheets(&conn, &f).unwrap();
    println!("=== 보결배정내역 ===");
    for s in &sheets {
        describe(s);
    }
    assert_numeric(&sheets[0], &["학년", "추천 순위"]);
    let p1 = export::write_book(&dir, &export::safe_name("보결배정내역"), &sheets).unwrap();
    println!("  → {}\n", p1.file_name().unwrap().to_string_lossy());

    // ---------- 2) 보결 현황 ----------
    // 기록이 있는 달을 고른다
    let month: Option<String> = conn
        .query_row(
            "SELECT substr(date,1,7) FROM substitutions
              WHERE status = 'ASSIGNED' GROUP BY 1 ORDER BY COUNT(*) DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .ok();
    let month = month.unwrap_or_else(|| "2026-09".to_string());
    // 달의 마지막 날은 domain::period 가 계산한다 (9월 31일 같은 날짜를 쓰면
    // CUSTOM 해석이 하루로 되돌아간다)
    let r = crate::domain::period::month_of(&month).expect("YYYY-MM");
    let q = StatsQuery {
        preset: Some("CUSTOM".into()),
        from: Some(r.from.clone()),
        to: Some(r.to.clone()),
    };
    let sheets = export::stats_sheets(&conn, &q).unwrap();
    println!("=== 보결현황 ({month}) ===");
    for s in &sheets {
        describe(s);
    }
    assert_numeric(
        sheets.iter().find(|s| s.name == "교사별").unwrap(),
        &["기간", "오늘", "이번 주", "이번 달", "이번 학기", "누적"],
    );
    let p2 = export::write_book(&dir, &export::safe_name("보결현황"), &sheets).unwrap();
    println!("  → {}\n", p2.file_name().unwrap().to_string_lossy());

    // ---------- 3) 보결 수당 ----------
    //
    // 1회 수당이 아직 0원이면 금액 서식을 눈으로 볼 수 없다. 그럴 때만
    // **복사본에** 15,000원을 넣어 본보기 파일을 만든다 (원본은 그대로다).
    let cfg = crate::repo::settings::pay_config(&conn).unwrap();
    if cfg.per_case == 0 {
        crate::repo::settings::save(
            &conn,
            &crate::repo::settings::SettingsInput {
                sub_pay_per_case: Some(15_000),
                sub_pay_policy: Some(crate::domain::pay::DEDUCT_OWN_CAUSED.into()),
                ..Default::default()
            },
        )
        .unwrap();
        println!("(1회 수당이 0원이라 복사본에만 15,000원을 넣어 본보기를 만듭니다)");
    }

    let pq = PayQuery {
        mode: Some(pay::MODE_MONTH.into()),
        month: Some(month.clone()),
        ..Default::default()
    };
    let sheets = pay::sheets(&conn, &pq).unwrap();
    println!("=== 보결수당 ({month}) ===");
    for s in &sheets {
        describe(s);
    }
    assert_numeric(
        &sheets[0],
        &[
            "보결 횟수",
            "본인 발생 보결 횟수",
            "지급 인정 횟수",
            "1회 보결 수당",
            "지급액",
        ],
    );
    let p3 = export::write_book(&dir, &export::safe_name("보결수당"), &sheets).unwrap();
    println!("  → {}\n", p3.file_name().unwrap().to_string_lossy());

    for p in [&p1, &p2, &p3] {
        let bytes = std::fs::read(p).unwrap();
        assert_eq!(&bytes[0..2], b"PK", "{}", p.display());
        println!(
            "  ok   {}  {:.1} KB",
            p.file_name().unwrap().to_string_lossy(),
            bytes.len() as f64 / 1024.0
        );
    }
    println!("\n이 폴더를 열어 직접 확인할 수 있습니다:\n  {}", dir.display());
}
