//! 내보내기 검증 — 엑셀 파일(XLSX).
//!
//! 확인할 것은 두 가지다.
//!   1. **화면에서 건 조건이 그대로 적용되는가** — 기간·상태 필터
//!   2. **값이 제 종류로 담기는가** — 횟수·금액은 숫자, 날짜는 날짜
//!
//! 파일 모양(굵은 머리글·고정·필터·서식)은 [`super::xlsx`] 쪽에서 실제 파일을
//! 풀어 확인한다. 여기서는 무엇이 어느 열에 들어가는지를 본다.

use super::*;
use crate::db::memory_conn;
use crate::domain::period;
use crate::repo::assign::{self as ra, AbsenceInput, AssignInput};
use crate::repo::xlsx::Cell;

fn hm(h: i32, m: i32) -> i32 {
    h * 60 + m
}

const MON: &str = "2026-09-07";
const TUE: &str = "2026-09-08";

/// 1학년 2반 · 4교시 + 점심. 담임 2명 + 전담 2명.
fn school() -> Connection {
    let c = memory_conn();
    c.execute(
        "INSERT INTO school(id, name, min_grade, max_grade) VALUES (1, '한빛초등학교', 1, 6)",
        [],
    )
    .unwrap();
    c.execute(
        "INSERT INTO terms(id, school_year, semester, name, is_current)
         VALUES (1, 2026, 2, '2026학년도 2학기', 1)",
        [],
    )
    .unwrap();
    c.execute(
        "INSERT INTO bell_schedules(id, term_id, name) VALUES (1, 1, '공통')",
        [],
    )
    .unwrap();
    for day in 1..=5 {
        let mut t = hm(9, 0);
        for p in 1..=4i32 {
            c.execute(
                "INSERT INTO bell_slots(bell_schedule_id, day_of_week, slot_type, period_no, label, start_min, end_min)
                 VALUES (1, ?1, 'PERIOD', ?2, ?3, ?4, ?5)",
                rusqlite::params![day, p, format!("{p}교시"), t, t + 40],
            )
            .unwrap();
            t += 45;
        }
        c.execute(
            "INSERT INTO bell_slots(bell_schedule_id, day_of_week, slot_type, period_no, label, start_min, end_min)
             VALUES (1, ?1, 'LUNCH', NULL, '점심', ?2, ?3)",
            rusqlite::params![day, t, t + 50],
        )
        .unwrap();
    }
    c.execute(
        "INSERT INTO grade_bell_map(term_id, grade, bell_schedule_id) VALUES (1, 1, 1)",
        [],
    )
    .unwrap();

    for (id, name, role) in [
        (1i64, "김일가", "HOMEROOM"),
        (2, "김일나", "HOMEROOM"),
        (3, "박전담", "SPECIAL"),
        (4, "최전담", "SPECIAL"),
    ] {
        c.execute(
            "INSERT INTO teachers(id, name, role_code) VALUES (?1, ?2, ?3)",
            rusqlite::params![id, name, role],
        )
        .unwrap();
    }
    for (id, no, cname, hr) in [(1i64, 1i32, "가람", 1i64), (2, 2, "나리", 2)] {
        c.execute(
            "INSERT INTO classes(id, term_id, grade, class_no, name, homeroom_teacher_id)
             VALUES (?1, 1, 1, ?2, ?3, ?4)",
            rusqlite::params![id, no, cname, hr],
        )
        .unwrap();
    }
    c
}

fn assign(c: &Connection, date: &str, class_id: i64, period: i32, absent: i64, sub: i64) -> i64 {
    ra::assign_one(
        c,
        &AssignInput {
            date: date.into(),
            class_id,
            slot_type: "PERIOD".into(),
            period_no: Some(period),
            absent_teacher_id: Some(absent),
            sub_teacher_id: sub,
            absence_id: None,
            reason_code: Some("TRIP".into()),
            reason_text: None,
        },
    )
    .unwrap()
    .id
}

// ---------- 시트를 뒤져 보는 도우미 ----------

fn sheet<'a>(sheets: &'a [Sheet], name: &str) -> &'a Sheet {
    sheets
        .iter()
        .find(|s| s.name == name)
        .unwrap_or_else(|| {
            panic!(
                "'{name}' 시트가 없다: {:?}",
                sheets.iter().map(|s| &s.name).collect::<Vec<_>>()
            )
        })
}

fn titles(s: &Sheet) -> Vec<String> {
    s.columns.iter().map(|c| c.title.clone()).collect()
}

fn at(s: &Sheet, name: &str, row: usize) -> Cell {
    let i = s
        .columns
        .iter()
        .position(|c| c.title == name)
        .unwrap_or_else(|| panic!("'{name}' 열이 없다: {:?}", titles(s)));
    s.rows[row][i].clone()
}

fn text_of(cell: &Cell) -> String {
    match cell {
        Cell::Text(v) => v.clone(),
        Cell::Date(v) | Cell::DateTime(v) => v.clone(),
        Cell::Count(n) | Cell::Money(n) => n.to_string(),
        Cell::Time(m) => format!("{:02}:{:02}", m / 60, m % 60),
        Cell::Blank => String::new(),
    }
}

/// 시트 전체를 한 글자열로 — '어딘가에 이 말이 있는가' 를 볼 때 쓴다.
fn flat(s: &Sheet) -> String {
    let mut out = titles(s).join(",");
    for r in &s.rows {
        out.push('\n');
        out.push_str(&r.iter().map(text_of).collect::<Vec<_>>().join(","));
    }
    out
}

// ============================================================
//  배정 내역
// ============================================================

#[test]
fn 요구한_항목이_모두_들어간다() {
    let c = school();
    assign(&c, MON, 1, 2, 1, 3);

    let sheets = history_sheets(&c, &HistoryFilter::default()).unwrap();
    let s = sheet(&sheets, "배정 내역");

    assert_eq!(
        titles(s),
        vec![
            "날짜", "요일", "결근 교사", "보결 교사", "학년", "반", "교시/점심",
            "시작 시각", "종료 시각", "과목", "결근 사유", "상태", "추천 순위",
            "추천 근거", "배정 시각", "취소 시각", "취소 사유",
        ]
    );

    assert_eq!(s.rows.len(), 1);
    assert_eq!(at(s, "날짜", 0), Cell::Date(MON.into()));
    assert_eq!(at(s, "요일", 0), Cell::text("월"));
    assert_eq!(at(s, "결근 교사", 0), Cell::text("김일가"));
    assert_eq!(at(s, "보결 교사", 0), Cell::text("박전담"));
    assert_eq!(at(s, "학년", 0), Cell::Count(1));
    assert_eq!(at(s, "반", 0), Cell::text("가람"), "기록 당시 반 이름");
    assert_eq!(at(s, "교시/점심", 0), Cell::text("2교시"));
    assert_eq!(at(s, "상태", 0), Cell::text("배정"));
    assert!(flat(s).contains("출장"), "결근 사유: {}", flat(s));
}

#[test]
fn 학년과_추천순위는_숫자로_담는다() {
    let c = school();
    assign(&c, MON, 1, 2, 1, 3);
    let sheets = history_sheets(&c, &HistoryFilter::default()).unwrap();
    let s = sheet(&sheets, "배정 내역");

    // 글자가 아니라 숫자여야 엑셀에서 정렬·합계가 된다
    assert!(matches!(at(s, "학년", 0), Cell::Count(_)));
    assert!(matches!(
        at(s, "추천 순위", 0),
        Cell::Count(_) | Cell::Blank
    ));
}

#[test]
fn 시각은_실제_시각으로_날짜는_실제_날짜로_담는다() {
    let c = school();
    assign(&c, MON, 1, 2, 1, 3);
    let sheets = history_sheets(&c, &HistoryFilter::default()).unwrap();
    let s = sheet(&sheets, "배정 내역");

    // 2교시 = 09:45~10:25
    assert_eq!(at(s, "시작 시각", 0), Cell::Time(hm(9, 45)));
    assert_eq!(at(s, "종료 시각", 0), Cell::Time(hm(10, 25)));
    assert!(matches!(at(s, "날짜", 0), Cell::Date(_)));
    assert!(
        matches!(at(s, "배정 시각", 0), Cell::DateTime(_)),
        "배정 시각은 날짜+시각이다"
    );
    assert_eq!(at(s, "취소 시각", 0), Cell::Blank, "취소하지 않았으면 빈 칸");
}

#[test]
fn 취소된_기록은_취소됨으로_나온다() {
    let c = school();
    let id = assign(&c, MON, 1, 2, 1, 3);
    ra::cancel(&c, id, Some("담임 복귀")).unwrap();

    let sheets = history_sheets(&c, &HistoryFilter::default()).unwrap();
    let s = sheet(&sheets, "배정 내역");
    assert_eq!(at(s, "상태", 0), Cell::text("취소됨"));
    assert_eq!(at(s, "취소 사유", 0), Cell::text("담임 복귀"));
    assert!(matches!(at(s, "취소 시각", 0), Cell::DateTime(_)));
}

#[test]
fn 화면에_건_조건이_그대로_적용된다() {
    let c = school();
    assign(&c, MON, 1, 2, 1, 3);
    assign(&c, TUE, 2, 3, 2, 4);

    // 기간
    let only_mon = history_sheets(
        &c,
        &HistoryFilter {
            from: Some(MON.into()),
            to: Some(MON.into()),
            ..Default::default()
        },
    )
    .unwrap();
    let s = sheet(&only_mon, "배정 내역");
    assert_eq!(s.rows.len(), 1);
    assert_eq!(at(s, "날짜", 0), Cell::Date(MON.into()));

    // 교사 이름
    let by_name = history_sheets(
        &c,
        &HistoryFilter {
            keyword: Some("최전담".into()),
            ..Default::default()
        },
    )
    .unwrap();
    let s = sheet(&by_name, "배정 내역");
    assert_eq!(s.rows.len(), 1);
    assert_eq!(at(s, "보결 교사", 0), Cell::text("최전담"));
}

#[test]
fn 조회_조건_시트에_기간이_남는다() {
    let c = school();
    assign(&c, MON, 1, 2, 1, 3);

    let sheets = history_sheets(
        &c,
        &HistoryFilter {
            from: Some(MON.into()),
            to: Some(TUE.into()),
            ..Default::default()
        },
    )
    .unwrap();
    let cond = sheet(&sheets, "조회 조건");
    let f = flat(cond);
    assert!(f.contains(&format!("{MON} ~ {TUE}")), "{f}");
    assert!(f.contains("만든 시각"), "{f}");
    assert!(!cond.filter, "조건 시트에는 필터를 걸지 않는다");
}

#[test]
fn 기록이_없어도_머리글은_나온다() {
    let c = school();
    let sheets = history_sheets(&c, &HistoryFilter::default()).unwrap();
    let s = sheet(&sheets, "배정 내역");
    assert!(s.rows.is_empty());
    assert_eq!(titles(s)[0], "날짜", "무엇이 비었는지 알 수 있어야 한다");
}

// ============================================================
//  보결 현황 — 시트로 나뉜다
// ============================================================

fn stats_of(c: &Connection, from: &str, to: &str) -> Vec<Sheet> {
    stats_sheets(
        c,
        &StatsQuery {
            preset: Some(period::CUSTOM.into()),
            from: Some(from.into()),
            to: Some(to.into()),
        },
    )
    .unwrap()
}

#[test]
fn 현황은_표마다_시트를_만든다() {
    let c = school();
    ra::create_absence(
        &c,
        &AbsenceInput {
            teacher_id: 1,
            date: MON.into(),
            is_all_day: true,
            start_min: None,
            end_min: None,
            reason_code: Some("ANNUAL".into()),
            reason_text: None,
        },
    )
    .unwrap();
    assign(&c, MON, 1, 2, 1, 3);

    let sheets = stats_of(&c, MON, MON);
    let names: Vec<&str> = sheets.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(
        names,
        vec!["요약", "교사별", "결근", "날짜별", "미배정", "조회 조건"],
        "CSV 시절 한 파일에 빈 줄로 밀어 넣었던 표들을 시트로 나눈다"
    );
}

#[test]
fn 현황의_숫자는_모두_숫자로_담긴다() {
    let c = school();
    assign(&c, MON, 1, 2, 1, 3);
    let sheets = stats_of(&c, MON, MON);

    // 요약
    let sum = sheet(&sheets, "요약");
    for (i, row) in sum.rows.iter().enumerate() {
        assert!(
            matches!(row[1], Cell::Count(_)),
            "요약 {i}번째 값이 숫자가 아니다: {:?}",
            row[1]
        );
    }
    assert!(flat(sum).contains("기간 배정(건)"));

    // 교사별
    let t = sheet(&sheets, "교사별");
    let park = t.rows.iter().position(|r| r[0] == Cell::text("박전담")).unwrap();
    assert_eq!(at(t, "기간", park), Cell::Count(1));
    assert_eq!(at(t, "누적", park), Cell::Count(1));
    assert!(matches!(at(t, "오늘", park), Cell::Count(_)));
}

#[test]
fn 현황도_기간을_따른다() {
    let c = school();
    assign(&c, MON, 1, 2, 1, 3);
    assign(&c, TUE, 2, 3, 2, 4);

    let mon = stats_of(&c, MON, MON);
    let t = sheet(&mon, "교사별");
    let park = t.rows.iter().position(|r| r[0] == Cell::text("박전담")).unwrap();
    let choi = t.rows.iter().position(|r| r[0] == Cell::text("최전담")).unwrap();
    assert_eq!(at(t, "기간", park), Cell::Count(1));
    assert_eq!(at(t, "기간", choi), Cell::Count(0), "월요일만 뽑았다");

    let both = stats_of(&c, MON, TUE);
    let t = sheet(&both, "교사별");
    let choi = t.rows.iter().position(|r| r[0] == Cell::text("최전담")).unwrap();
    assert_eq!(at(t, "기간", choi), Cell::Count(1));

    // 조회 조건에도 남는다
    assert!(flat(sheet(&both, "조회 조건")).contains(&format!("{MON} ~ {TUE}")));
}

#[test]
fn 날짜별_시트는_아무_일도_없던_날을_넣지_않는다() {
    let c = school();
    assign(&c, MON, 1, 2, 1, 3);

    // 한 주를 뽑아도 기록이 있는 날만 나온다
    let sheets = stats_of(&c, "2026-09-07", "2026-09-11");
    let d = sheet(&sheets, "날짜별");
    assert_eq!(d.rows.len(), 1, "{}", flat(d));
    assert_eq!(at(d, "날짜", 0), Cell::Date(MON.into()));
    assert!(matches!(at(d, "배정 완료", 0), Cell::Count(_)));
}

// ============================================================
//  파일 이름
// ============================================================

#[test]
fn 파일_이름은_기존_이름에_확장자만_xlsx다() {
    for base in ["보결배정내역", "보결현황", "보결수당"] {
        let n = safe_name(base);
        assert!(n.starts_with(&format!("{base}-")), "{n}");
        assert!(n.ends_with(".xlsx"), "{n}");
        assert!(!n.contains(".csv"), "{n}");
        // 날짜와 시각이 들어 있다
        let stamp: String = n
            .trim_start_matches(&format!("{base}-"))
            .trim_end_matches(".xlsx")
            .to_string();
        assert_eq!(stamp.len(), 15, "YYYYMMDD-HHMMSS 여야 한다: {stamp}");
        assert!(stamp.chars().all(|ch| ch.is_ascii_digit() || ch == '-'));
    }
}

#[test]
fn 실제로_파일이_만들어진다() {
    let c = school();
    assign(&c, MON, 1, 2, 1, 3);

    let dir = std::env::temp_dir().join(format!(
        "bogyeol-export-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let sheets = history_sheets(&c, &HistoryFilter::default()).unwrap();
    let name = safe_name("보결배정내역");
    let path = write_book(&dir, &name, &sheets).unwrap();

    assert!(path.exists());
    let bytes = std::fs::read(&path).unwrap();
    assert_eq!(&bytes[0..2], b"PK", "진짜 xlsx (zip) 여야 한다");
    assert!(bytes.len() > 1000, "빈 파일이 아니다");

    std::fs::remove_dir_all(&dir).ok();
}
