//! Phase 10 검증 — CSV 내보내기.
//!
//! 가장 중요한 것은 **Excel에서 한글이 깨지지 않는 것**과
//! **화면에서 건 조건이 그대로 적용되는 것**이다.

use super::*;
use crate::db::memory_conn;
use crate::domain::period;
use crate::repo::assign::{self as ra, AbsenceInput, AssignInput};

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

// ============================================================
//  Excel 호환
// ============================================================

#[test]
fn 파일_앞에_BOM을_붙인다() {
    let dir = std::env::temp_dir().join(format!(
        "bogyeol-csv-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let path = write_csv(&dir, "시험.csv", "이름,값\r\n김철수,1\r\n").unwrap();

    let bytes = std::fs::read(&path).unwrap();
    assert_eq!(&bytes[0..3], BOM, "Excel이 UTF-8로 읽으려면 BOM이 필요하다");

    let text = String::from_utf8(bytes[3..].to_vec()).unwrap();
    assert!(text.starts_with("이름,값"));
    assert!(text.contains("김철수"), "한글이 그대로 들어 있다");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn 줄바꿈은_CRLF다() {
    let c = school();
    assign(&c, MON, 1, 1, 1, 3);
    let body = history_csv(&c, &ra::HistoryFilter::default()).unwrap();
    assert!(body.contains("\r\n"), "Windows 프로그램에서 한 줄로 붙지 않게");
    assert!(!body.contains("\n\n"));
}

#[test]
fn 쉼표와_따옴표가_든_값을_감싼다() {
    let c = school();
    let id = assign(&c, MON, 1, 1, 1, 3);
    ra::cancel(&c, id, Some("담임 출근, 보결 필요 없음")).unwrap();

    let body = history_csv(&c, &ra::HistoryFilter::default()).unwrap();
    assert!(
        body.contains("\"담임 출근, 보결 필요 없음\""),
        "쉼표가 든 값은 따옴표로 감싼다:\n{body}"
    );
}

// ============================================================
//  배정 내역
// ============================================================

#[test]
fn 요구한_항목이_모두_들어간다() {
    let c = school();
    assign(&c, MON, 1, 1, 1, 3);

    let body = history_csv(&c, &ra::HistoryFilter::default()).unwrap();
    let mut lines = body.lines();
    let head = lines.next().unwrap();
    for col in [
        "날짜",
        "요일",
        "결근 교사",
        "보결 교사",
        "학년",
        "반",
        "교시/점심",
        "시작 시각",
        "종료 시각",
        "결근 사유",
        "상태",
    ] {
        assert!(head.contains(col), "'{col}' 열이 있어야 한다: {head}");
    }

    let row = lines.next().unwrap();
    assert!(row.contains("2026-09-07"));
    assert!(row.contains("월"), "요일: {row}");
    assert!(row.contains("김일가"), "결근 교사: {row}");
    assert!(row.contains("박전담"), "보결 교사: {row}");
    assert!(row.contains("가람"), "반 이름: {row}");
    assert!(row.contains("09:00"), "시작 시각: {row}");
    assert!(row.contains("09:40"), "종료 시각: {row}");
    assert!(row.contains("출장"), "사유: {row}");
    assert!(row.contains("배정"), "상태: {row}");
}

#[test]
fn 취소된_기록은_취소됨으로_나온다() {
    let c = school();
    let id = assign(&c, MON, 1, 1, 1, 3);
    ra::cancel(&c, id, Some("담임 출근")).unwrap();

    let body = history_csv(&c, &ra::HistoryFilter::default()).unwrap();
    assert!(body.contains("취소됨"), "{body}");
    assert!(body.contains("담임 출근"));
}

#[test]
fn 화면에_건_조건이_그대로_적용된다() {
    let c = school();
    assign(&c, MON, 1, 1, 1, 3);
    assign(&c, TUE, 1, 1, 1, 4);

    let all = history_csv(&c, &ra::HistoryFilter::default()).unwrap();
    assert_eq!(all.lines().count(), 3, "머리글 + 2건");

    let only_mon = history_csv(
        &c,
        &ra::HistoryFilter {
            from: Some(MON.into()),
            to: Some(MON.into()),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(only_mon.lines().count(), 2, "머리글 + 1건");
    assert!(only_mon.contains("박전담"));
    assert!(!only_mon.contains("최전담"));

    let by_name = history_csv(
        &c,
        &ra::HistoryFilter {
            keyword: Some("최전담".into()),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(by_name.lines().count(), 2);
    assert!(by_name.contains(TUE));
}

#[test]
fn 기록이_없어도_머리글은_나온다() {
    let c = school();
    let body = history_csv(&c, &ra::HistoryFilter::default()).unwrap();
    assert_eq!(body.lines().count(), 1);
    assert!(body.starts_with("날짜,요일"));
}

// ============================================================
//  현황
// ============================================================

#[test]
fn 현황_CSV에_요약과_표가_모두_들어간다() {
    let c = school();
    ra::create_absence(
        &c,
        &AbsenceInput {
            teacher_id: 1,
            date: MON.into(),
            is_all_day: true,
            start_min: None,
            end_min: None,
            reason_code: Some("SICK".into()),
            reason_text: None,
        },
    )
    .unwrap();
    assign(&c, MON, 1, 1, 1, 3);

    let q = StatsQuery {
        preset: Some(period::CUSTOM.into()),
        from: Some(MON.into()),
        to: Some(MON.into()),
    };
    let body = stats_csv(&c, &q).unwrap();

    for section in ["[요약]", "[교사별 보결 현황]", "[결근 현황]", "[날짜별 현황]"] {
        assert!(body.contains(section), "'{section}' 이 있어야 한다");
    }
    assert!(body.contains("2026-09-07 ~ 2026-09-07"), "기간이 적혀 있다");
    assert!(body.contains("박전담"));
    assert!(body.contains("병가"));
    assert!(body.contains("미배정(건)"));
}

#[test]
fn 현황_CSV도_기간을_따른다() {
    let c = school();
    assign(&c, MON, 1, 1, 1, 3);
    assign(&c, TUE, 1, 2, 1, 4);

    let one = stats_csv(
        &c,
        &StatsQuery {
            preset: Some(period::CUSTOM.into()),
            from: Some(MON.into()),
            to: Some(MON.into()),
        },
    )
    .unwrap();
    let two = stats_csv(
        &c,
        &StatsQuery {
            preset: Some(period::CUSTOM.into()),
            from: Some(MON.into()),
            to: Some(TUE.into()),
        },
    )
    .unwrap();

    assert!(one.contains("기간 배정(건),1"), "{one}");
    assert!(two.contains("기간 배정(건),2"), "{two}");
}

#[test]
fn 파일_이름에_날짜와_시각이_들어간다() {
    let name = safe_name("보결배정내역");
    let today = chrono::Local::now().format("%Y%m%d").to_string();
    assert!(name.starts_with("보결배정내역-"));
    assert!(name.contains(&today));
    assert!(name.ends_with(".csv"));
}
