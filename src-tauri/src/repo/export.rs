//! Phase 10 — CSV 내보내기.
//!
//! ## Excel에서 한글이 깨지지 않게
//!
//! Windows Excel은 CSV를 열 때 UTF-8임을 스스로 알아내지 못한다. 파일 앞에
//! **BOM(`EF BB BF`)** 을 붙여 두면 그때부터 UTF-8로 읽는다. 이 세 바이트가
//! 없으면 한글이 전부 깨지므로 반드시 붙인다.
//!
//! 줄바꿈도 `\r\n`을 쓴다. 메모장에서 열었을 때 한 줄로 붙어 보이지 않는다.

use std::path::PathBuf;

use rusqlite::Connection;

use super::assign::{self, HistoryFilter};
use super::stats::{self, StatsQuery};
use crate::domain::time::fmt_min;
use crate::error::AppResult;

pub const BOM: &[u8] = &[0xEF, 0xBB, 0xBF];

const DAY: [&str; 8] = ["", "월", "화", "수", "목", "금", "토", "일"];

fn day_name(d: i32) -> &'static str {
    DAY.get(d as usize).copied().unwrap_or("")
}

/// CSV 한 칸. 쉼표·따옴표·줄바꿈이 들어 있으면 따옴표로 감싼다.
fn cell(v: &str) -> String {
    if v.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", v.replace('"', "\"\""))
    } else {
        v.to_string()
    }
}

fn row(cells: &[String]) -> String {
    let line: Vec<String> = cells.iter().map(|c| cell(c)).collect();
    format!("{}\r\n", line.join(","))
}

fn n(v: i32) -> String {
    v.to_string()
}

// ============================================================
//  배정 내역
// ============================================================

pub fn history_csv(conn: &Connection, f: &HistoryFilter) -> AppResult<String> {
    let view = assign::history(conn, f)?;
    let mut out = String::new();

    out.push_str(&row(&[
        "날짜".into(),
        "요일".into(),
        "결근 교사".into(),
        "보결 교사".into(),
        "학년".into(),
        "반".into(),
        "교시/점심".into(),
        "시작 시각".into(),
        "종료 시각".into(),
        "과목".into(),
        "결근 사유".into(),
        "상태".into(),
        "추천 순위".into(),
        "추천 근거".into(),
        "배정 시각".into(),
        "취소 시각".into(),
        "취소 사유".into(),
    ]));

    for r in &view.rows {
        // '5-가람' 에서 반 부분만 떼어 낸다 (기록 당시 표기 그대로)
        let class_only = r
            .class_label
            .split_once('-')
            .map(|(_, b)| b.to_string())
            .unwrap_or_else(|| r.class_label.clone());

        out.push_str(&row(&[
            r.date.clone(),
            day_name(r.day_of_week).to_string(),
            r.absent_teacher_name.clone().unwrap_or_default(),
            r.sub_teacher_name.clone(),
            n(r.grade),
            class_only,
            r.slot_label.clone(),
            fmt_min(r.start_min),
            fmt_min(r.end_min),
            r.subject_name.clone().unwrap_or_default(),
            r.reason_label.clone().unwrap_or_default(),
            if r.status == "CANCELLED" { "취소됨" } else { "배정" }.to_string(),
            r.recommend_rank.map(|x| x.to_string()).unwrap_or_default(),
            r.recommend_reason.clone().unwrap_or_default(),
            r.created_at.clone(),
            r.cancelled_at.clone().unwrap_or_default(),
            r.cancel_reason.clone().unwrap_or_default(),
        ]));
    }

    Ok(out)
}

// ============================================================
//  현황
// ============================================================

pub fn stats_csv(conn: &Connection, q: &StatsQuery) -> AppResult<String> {
    let v = stats::view(conn, q)?;
    let mut out = String::new();

    out.push_str(&row(&[
        "기간".into(),
        format!("{} ~ {}", v.from, v.to),
    ]));
    out.push_str("\r\n");

    // ---------- 요약 ----------
    out.push_str(&row(&["[요약]".into()]));
    out.push_str(&row(&["항목".into(), "값".into()]));
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
        out.push_str(&row(&[k.into(), n(val)]));
    }
    out.push_str("\r\n");

    // ---------- 교사별 ----------
    out.push_str(&row(&["[교사별 보결 현황]".into()]));
    out.push_str(&row(&[
        "교사".into(),
        "구분".into(),
        "담당".into(),
        "기간".into(),
        "오늘".into(),
        "이번 주".into(),
        "이번 달".into(),
        "이번 학기".into(),
        "누적".into(),
        "참고".into(),
        "상태".into(),
    ]));
    for t in &v.teachers {
        out.push_str(&row(&[
            t.name.clone(),
            t.role_label.clone(),
            t.duty.clone(),
            n(t.period),
            n(t.today),
            n(t.week),
            n(t.month),
            n(t.term),
            n(t.total),
            if t.band == "NONE" { String::new() } else { t.band_label.clone() },
            if !t.active {
                "비활성".into()
            } else if !t.is_substitutable {
                "보결 제외".into()
            } else {
                String::new()
            },
        ]));
    }
    out.push_str("\r\n");

    // ---------- 결근 ----------
    out.push_str(&row(&["[결근 현황]".into()]));
    out.push_str(&row(&[
        "교사".into(),
        "구분".into(),
        "결근".into(),
        "종일".into(),
        "일부".into(),
        "사유".into(),
        "보결 필요".into(),
        "배정".into(),
        "미배정".into(),
    ]));
    for a in &v.absences {
        out.push_str(&row(&[
            a.name.clone(),
            a.role_label.clone(),
            n(a.count),
            n(a.all_day),
            n(a.partial),
            a.reasons.clone(),
            n(a.required),
            n(a.assigned),
            n(a.unassigned),
        ]));
    }
    out.push_str("\r\n");

    // ---------- 날짜별 ----------
    out.push_str(&row(&["[날짜별 현황]".into()]));
    out.push_str(&row(&[
        "날짜".into(),
        "요일".into(),
        "결근".into(),
        "결근 교사".into(),
        "보결 필요".into(),
        "배정 완료".into(),
        "미배정".into(),
        "취소".into(),
    ]));
    for d in &v.days {
        // 아무 일도 없던 날은 넣지 않는다 — 학기 전체를 뽑아도 읽을 만하게
        if d.absent_teachers == 0 && d.assigned == 0 && d.cancelled == 0 && d.required == 0 {
            continue;
        }
        out.push_str(&row(&[
            d.date.clone(),
            day_name(d.day_of_week).to_string(),
            n(d.absent_teachers),
            d.absent_names.clone(),
            n(d.required),
            n(d.assigned),
            n(d.unassigned),
            n(d.cancelled),
        ]));
    }

    Ok(out)
}

// ============================================================
//  파일로 저장
// ============================================================

pub fn safe_name(base: &str) -> String {
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    format!("{base}-{stamp}.csv")
}

/// UTF-8 BOM을 붙여 저장한다. Excel에서 바로 열어도 한글이 깨지지 않는다.
pub fn write_csv(dir: &std::path::Path, name: &str, body: &str) -> AppResult<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(name);
    let mut bytes = Vec::with_capacity(BOM.len() + body.len());
    bytes.extend_from_slice(BOM);
    bytes.extend_from_slice(body.as_bytes());
    std::fs::write(&path, bytes)?;
    Ok(path)
}

#[cfg(test)]
#[path = "export_tests.rs"]
mod tests;
