//! 내보내기 — **엑셀 파일(XLSX)로 만든다.**
//!
//! 여기서는 무엇을 어느 열에 넣을지만 정하고, 파일을 만드는 일은
//! [`super::xlsx`] 한 곳이 맡는다. 그래서 세 화면의 파일이 모두 같은 모양이다
//! (굵은 머리글 · 첫 행 고정 · 자동필터 · 숫자는 숫자로).
//!
//! ## 표가 여러 개면 시트로 나눈다
//!
//! CSV 시절에는 '보결 현황' 네 가지 표를 빈 줄로 나눠 한 파일에 밀어 넣었다.
//! 엑셀에서는 시트가 있으니 그럴 필요가 없다 — 요약 · 교사별 · 결근 · 날짜별을
//! 각각 시트 하나로 만든다. 시트마다 머리글이 1행이므로 고정과 필터가 제대로
//! 걸린다.
//!
//! ## 조회 조건은 따로 적어 둔다
//!
//! 어느 기간·어느 조건으로 뽑았는지 파일만 보고 알 수 있어야 한다. 자료 시트
//! 위에 조건을 끼워 넣으면 머리글이 1행이 아니게 되므로, **'조회 조건' 시트**를
//! 따로 만든다.

use std::path::{Path, PathBuf};

use rusqlite::Connection;

use super::assign::{self, HistoryFilter};
use super::stats::{self, StatsQuery};
use super::xlsx::{col, Cell};
pub use super::xlsx::Sheet;
use crate::error::AppResult;

const DAY: [&str; 8] = ["", "월", "화", "수", "목", "금", "토", "일"];

fn day_name(d: i32) -> &'static str {
    DAY.get(d as usize).copied().unwrap_or("")
}

fn i(v: i32) -> Cell {
    Cell::Count(v as i64)
}

// ============================================================
//  배정 내역
// ============================================================

pub fn history_sheets(conn: &Connection, f: &HistoryFilter) -> AppResult<Vec<Sheet>> {
    let view = assign::history(conn, f)?;

    let mut sheet = Sheet::new(
        "배정 내역",
        vec![
            col("날짜", 12.0),
            col("요일", 6.0),
            col("결근 교사", 12.0),
            col("보결 교사", 12.0),
            col("학년", 7.0),
            col("반", 9.0),
            col("교시/점심", 11.0),
            col("시작 시각", 10.0),
            col("종료 시각", 10.0),
            col("과목", 10.0),
            col("결근 사유", 11.0),
            col("상태", 9.0),
            col("추천 순위", 10.0),
            col("추천 근거", 26.0),
            col("배정 시각", 20.0),
            col("취소 시각", 20.0),
            col("취소 사유", 20.0),
        ],
    );

    for r in &view.rows {
        // '5-가람' 에서 반 부분만 떼어 낸다 (기록 당시 표기 그대로)
        let class_only = r
            .class_label
            .split_once('-')
            .map(|(_, b)| b.to_string())
            .unwrap_or_else(|| r.class_label.clone());

        sheet.push(vec![
            Cell::Date(r.date.clone()),
            Cell::text(day_name(r.day_of_week)),
            Cell::opt_text(r.absent_teacher_name.clone()),
            Cell::text(r.sub_teacher_name.clone()),
            i(r.grade),
            Cell::text(class_only),
            Cell::text(r.slot_label.clone()),
            Cell::Time(r.start_min),
            Cell::Time(r.end_min),
            Cell::opt_text(r.subject_name.clone()),
            Cell::opt_text(r.reason_label.clone()),
            Cell::text(if r.status == "CANCELLED" { "취소됨" } else { "배정" }),
            Cell::opt_count(r.recommend_rank.map(|x| x as i64)),
            Cell::opt_text(r.recommend_reason.clone()),
            Cell::opt_datetime(Some(r.created_at.clone())),
            Cell::opt_datetime(r.cancelled_at.clone()),
            Cell::opt_text(r.cancel_reason.clone()),
        ]);
    }

    let cond = conditions(&[
        ("기간", range_text(f.from.as_deref(), f.to.as_deref())),
        ("배정 건수", view.assigned_count.to_string()),
        ("취소 건수", view.cancelled_count.to_string()),
        ("줄 수", view.rows.len().to_string()),
    ]);

    Ok(vec![sheet, cond])
}

// ============================================================
//  보결 현황 — 표 네 개를 시트 네 개로
// ============================================================

pub fn stats_sheets(conn: &Connection, q: &StatsQuery) -> AppResult<Vec<Sheet>> {
    let v = stats::view(conn, q)?;

    // ---------- 요약 ----------
    let mut summary = Sheet::new("요약", vec![col("항목", 22.0), col("값", 12.0)]).no_filter();
    for (k, val) in [
        ("결근 교사(명)", v.summary.absent_teachers),
        ("결근 기록(건)", v.summary.absence_count),
        ("보결 필요(건)", v.summary.required),
        ("필요분 중 배정(건)", v.summary.covered),
        ("미배정(건)", v.summary.unassigned),
        ("기간 배정(건)", v.summary.assigned),
        ("취소(건)", v.summary.cancelled),
        ("보결 맡은 교사(명)", v.summary.sub_teachers),
    ] {
        summary.push(vec![Cell::text(k), i(val)]);
    }

    // ---------- 교사별 ----------
    let mut teachers = Sheet::new(
        "교사별",
        vec![
            col("교사", 12.0),
            col("구분", 10.0),
            col("담당", 16.0),
            col("기간", 9.0),
            col("오늘", 8.0),
            col("이번 주", 9.0),
            col("이번 달", 9.0),
            col("이번 학기", 10.0),
            col("누적", 8.0),
            col("참고", 13.0),
            col("상태", 11.0),
        ],
    );
    for t in &v.teachers {
        teachers.push(vec![
            Cell::text(t.name.clone()),
            Cell::text(t.role_label.clone()),
            Cell::text(t.duty.clone()),
            i(t.period),
            i(t.today),
            i(t.week),
            i(t.month),
            i(t.term),
            i(t.total),
            if t.band == "NONE" {
                Cell::Blank
            } else {
                Cell::text(t.band_label.clone())
            },
            if !t.active {
                Cell::text("비활성")
            } else if !t.is_substitutable {
                Cell::text("보결 제외")
            } else {
                Cell::Blank
            },
        ]);
    }

    // ---------- 결근 ----------
    let mut absences = Sheet::new(
        "결근",
        vec![
            col("교사", 12.0),
            col("구분", 10.0),
            col("결근", 8.0),
            col("종일", 8.0),
            col("일부", 8.0),
            col("사유", 22.0),
            col("보결 필요", 10.0),
            col("배정", 8.0),
            col("미배정", 9.0),
        ],
    );
    for a in &v.absences {
        absences.push(vec![
            Cell::text(a.name.clone()),
            Cell::text(a.role_label.clone()),
            i(a.count),
            i(a.all_day),
            i(a.partial),
            Cell::text(a.reasons.clone()),
            i(a.required),
            i(a.assigned),
            i(a.unassigned),
        ]);
    }

    // ---------- 날짜별 ----------
    let mut days = Sheet::new(
        "날짜별",
        vec![
            col("날짜", 12.0),
            col("요일", 6.0),
            col("결근", 8.0),
            col("결근 교사", 26.0),
            col("보결 필요", 10.0),
            col("배정 완료", 10.0),
            col("미배정", 9.0),
            col("취소", 8.0),
        ],
    );
    for d in &v.days {
        // 아무 일도 없던 날은 넣지 않는다 — 학기 전체를 뽑아도 읽을 만하게
        if d.absent_teachers == 0 && d.assigned == 0 && d.cancelled == 0 && d.required == 0 {
            continue;
        }
        days.push(vec![
            Cell::Date(d.date.clone()),
            Cell::text(day_name(d.day_of_week)),
            i(d.absent_teachers),
            Cell::text(d.absent_names.clone()),
            i(d.required),
            i(d.assigned),
            i(d.unassigned),
            i(d.cancelled),
        ]);
    }

    // ---------- 미배정 ----------
    let mut open = Sheet::new(
        "미배정",
        vec![
            col("날짜", 12.0),
            col("요일", 6.0),
            col("학급", 10.0),
            col("교시/점심", 11.0),
            col("시작 시각", 10.0),
            col("종료 시각", 10.0),
            col("결근 교사", 12.0),
            col("구분", 12.0),
        ],
    );
    for o in &v.open_slots {
        open.push(vec![
            Cell::Date(o.date.clone()),
            Cell::text(day_name(o.day_of_week)),
            Cell::text(o.class_label.clone()),
            Cell::text(o.slot_label.clone()),
            Cell::Time(o.start_min),
            Cell::Time(o.end_min),
            Cell::text(o.absent_teacher_name.clone()),
            Cell::text(o.kind_label.clone()),
        ]);
    }

    let cond = conditions(&[
        ("기간", format!("{} ~ {}", v.from, v.to)),
        ("기간 이름", v.range_label.clone()),
        ("조회 방식", v.preset.clone()),
    ]);

    Ok(vec![summary, teachers, absences, days, open, cond])
}

// ============================================================
//  조회 조건 시트
// ============================================================

/// 어느 조건으로 뽑았는지 적어 두는 시트. 자료가 아니므로 필터는 걸지 않는다.
pub fn conditions(items: &[(&str, String)]) -> Sheet {
    let mut s = Sheet::new("조회 조건", vec![col("항목", 20.0), col("값", 40.0)]).no_filter();
    for (k, v) in items {
        s.push(vec![Cell::text(*k), Cell::text(v.clone())]);
    }
    s.push(vec![
        Cell::text("만든 시각"),
        Cell::text(chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()),
    ]);
    s
}

pub fn range_text(from: Option<&str>, to: Option<&str>) -> String {
    match (from, to) {
        (Some(a), Some(b)) => format!("{a} ~ {b}"),
        (Some(a), None) => format!("{a} 부터"),
        (None, Some(b)) => format!("{b} 까지"),
        (None, None) => "전체".to_string(),
    }
}

// ============================================================
//  파일로 저장
// ============================================================

pub fn safe_name(base: &str) -> String {
    super::xlsx::safe_name(base)
}

pub fn write_book(dir: &Path, name: &str, sheets: &[Sheet]) -> AppResult<PathBuf> {
    super::xlsx::write_book(dir, name, sheets)
}

#[cfg(test)]
#[path = "export_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "export_real_db_tests.rs"]
mod real_db_tests;
