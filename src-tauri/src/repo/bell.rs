//! 학년별 시정표(교시 시각)와 점심시간.
//!
//! 저장 구조는 **프로필 + 학년 매핑**이다.
//!   - `bell_schedules`  : 시정표 유형 (예: 저학년 시정표)
//!   - `bell_slots`      : 유형 × 요일 × 교시/점심 의 실제 시각
//!   - `grade_bell_map`  : 학년 -> 유형
//!
//! 학년 6개 × 요일 5개 × 교시 8개를 일일이 입력하면 240줄이 되므로,
//! 실제 학교가 쓰는 2~3가지 유형만 만들어 학년에 붙이는 방식으로 입력량을 줄인다.
//!
//! 시각은 모두 '자정 기준 분' 정수다. 보결 가능 여부는 교시 번호가 아니라
//! 이 시각 구간으로 판단한다.

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use super::school;
use crate::error::{AppError, AppResult};

const DAY_NAMES: [&str; 8] = ["", "월", "화", "수", "목", "금", "토", "일"];

pub fn day_name(d: i32) -> &'static str {
    DAY_NAMES.get(d as usize).copied().unwrap_or("?")
}

pub fn fmt_min(m: i32) -> String {
    format!("{:02}:{:02}", m / 60, m % 60)
}

pub const SLOT_PERIOD: &str = "PERIOD";
pub const SLOT_LUNCH: &str = "LUNCH";
/// 중간놀이·아침활동처럼 학교마다 다른 시간 구간. 이름은 사용자가 정한다.
pub const SLOT_OTHER: &str = "OTHER";
pub const RECESS_LABEL: &str = "중간놀이";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Slot {
    pub day_of_week: i32,
    pub slot_type: String,
    pub period_no: Option<i32>,
    pub label: String,
    pub start_min: i32,
    pub end_min: i32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BellProfile {
    pub id: i64,
    pub name: String,
    pub grades: Vec<i32>,
    pub slots: Vec<Slot>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BellOverview {
    pub profiles: Vec<BellProfile>,
    /// 아직 어떤 시정표도 지정되지 않은 학년
    pub unassigned_grades: Vec<i32>,
    pub min_grade: i32,
    pub max_grade: i32,
    pub school_days: Vec<i32>,
}

// ============================================================
//  검증 — 화면과 저장이 똑같은 규칙을 쓰도록 여기 한 곳에만 둔다.
// ============================================================

/// 잘못된 점을 모두 모아 사용자용 한국어 문장으로 돌려준다.
pub fn collect_problems(slots: &[Slot]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();

    for s in slots {
        let where_ = format!("{}요일 {}", day_name(s.day_of_week), s.label);

        if !(1..=7).contains(&s.day_of_week) {
            out.push("요일이 올바르지 않습니다.".into());
            continue;
        }
        if !matches!(s.slot_type.as_str(), SLOT_PERIOD | SLOT_LUNCH | SLOT_OTHER) {
            out.push(format!("{where_}: 교시 구분이 올바르지 않습니다."));
            continue;
        }
        if s.slot_type == SLOT_PERIOD && s.period_no.is_none() {
            out.push(format!("{where_}: 교시 번호가 없습니다."));
        }
        if s.slot_type != SLOT_PERIOD && s.period_no.is_some() {
            out.push(format!("{where_}: 수업 시간이 아닌 구간에는 교시 번호를 붙일 수 없습니다."));
        }
        if s.slot_type != SLOT_PERIOD && s.label.trim().is_empty() {
            out.push(format!(
                "{}요일: 이름이 없는 시간 구간이 있습니다. 이름을 입력해 주세요.",
                day_name(s.day_of_week)
            ));
        }
        if !(0..=1440).contains(&s.start_min) || !(0..=1440).contains(&s.end_min) {
            out.push(format!("{where_}: 시각은 00:00부터 24:00 사이여야 합니다."));
            continue;
        }
        if s.end_min <= s.start_min {
            out.push(format!(
                "{where_}: 끝나는 시각({})이 시작 시각({})보다 빠르거나 같습니다.",
                fmt_min(s.end_min),
                fmt_min(s.start_min)
            ));
        }
    }

    for day in 1..=7 {
        let mut v: Vec<&Slot> = slots.iter().filter(|s| s.day_of_week == day).collect();
        if v.is_empty() {
            continue;
        }
        let dn = day_name(day);

        if v.iter().filter(|s| s.slot_type == SLOT_LUNCH).count() > 1 {
            out.push(format!("{dn}요일에 점심시간이 두 번 있습니다. 하나만 남겨 주세요."));
        }

        let mut nums: Vec<i32> = v.iter().filter_map(|s| s.period_no).collect();
        nums.sort_unstable();
        for w in nums.windows(2) {
            if w[0] == w[1] {
                out.push(format!("{dn}요일에 {}교시가 두 번 있습니다.", w[0]));
            }
        }

        // 시각이 유효한 것만 가지고 겹침을 본다
        v.retain(|s| s.end_min > s.start_min);
        v.sort_by_key(|s| (s.start_min, s.end_min));

        for w in v.windows(2) {
            let (a, b) = (w[0], w[1]);
            if a.end_min > b.start_min {
                out.push(format!(
                    "{dn}요일 {}({}~{})와 {}({}~{})의 시간이 겹칩니다.",
                    a.label,
                    fmt_min(a.start_min),
                    fmt_min(a.end_min),
                    b.label,
                    fmt_min(b.start_min),
                    fmt_min(b.end_min)
                ));
            }
        }

        // 시간 순서와 교시 순서가 어긋나면 입력 실수일 가능성이 높다
        let periods: Vec<i32> = v.iter().filter_map(|s| s.period_no).collect();
        for w in periods.windows(2) {
            if w[0] > w[1] {
                out.push(format!(
                    "{dn}요일: {}교시가 {}교시보다 먼저 시작합니다. 교시 순서를 확인해 주세요.",
                    w[1], w[0]
                ));
                break;
            }
        }
    }

    out
}

pub fn validate_slots(slots: &[Slot]) -> AppResult<()> {
    let problems = collect_problems(slots);
    match problems.first() {
        None => Ok(()),
        Some(first) => Err(AppError::invalid(first.clone()).detail(problems.join(" / "))),
    }
}

// ============================================================
//  자동 생성 — 사용자가 시각을 하나하나 입력하지 않도록
// ============================================================

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenParams {
    /// 1교시 시작 시각(분)
    pub first_start_min: i32,
    /// 한 교시 수업 시간(분)
    pub lesson_minutes: i32,
    /// 쉬는 시간(분)
    pub break_minutes: i32,
    /// 몇 교시 뒤에 점심인가. 0이면 점심 없음
    pub lunch_after_period: i32,
    pub lunch_minutes: i32,
    /// 몇 교시 뒤에 중간놀이인가. 0이면 없음
    #[serde(default)]
    pub recess_after_period: i32,
    #[serde(default)]
    pub recess_minutes: i32,
    /// 중간놀이 구간의 이름. 비어 있으면 '중간놀이'
    #[serde(default)]
    pub recess_label: String,
    /// 요일별 교시 수 [(요일, 교시수)]
    pub periods_by_day: Vec<(i32, i32)>,
}

impl GenParams {
    fn validate(&self) -> AppResult<()> {
        if !(0..=1439).contains(&self.first_start_min) {
            return Err(AppError::invalid("1교시 시작 시각이 올바르지 않습니다."));
        }
        if !(5..=180).contains(&self.lesson_minutes) {
            return Err(AppError::invalid("한 교시 수업 시간은 5분에서 180분 사이로 입력해 주세요."));
        }
        if !(0..=120).contains(&self.break_minutes) {
            return Err(AppError::invalid("쉬는 시간은 0분에서 120분 사이로 입력해 주세요."));
        }
        if self.lunch_after_period > 0 && !(10..=180).contains(&self.lunch_minutes) {
            return Err(AppError::invalid("점심시간은 10분에서 180분 사이로 입력해 주세요."));
        }
        if self.recess_after_period > 0 && !(5..=120).contains(&self.recess_minutes) {
            return Err(AppError::invalid("중간놀이 시간은 5분에서 120분 사이로 입력해 주세요."));
        }
        if self.recess_after_period > 0
            && self.lunch_after_period > 0
            && self.recess_after_period == self.lunch_after_period
        {
            return Err(AppError::invalid(
                "중간놀이와 점심을 같은 교시 뒤에 둘 수는 없습니다. 서로 다른 교시로 정해 주세요.",
            ));
        }
        if self.periods_by_day.iter().any(|(_, n)| !(0..=15).contains(n)) {
            return Err(AppError::invalid("하루 교시 수는 15교시 이내로 입력해 주세요."));
        }
        Ok(())
    }
}

/// 규칙에 따라 요일별 교시/점심 구간을 만들어 낸다.
///
/// 예) 09:00 시작 / 수업 40분 / 쉬는시간 10분 / 4교시 뒤 점심 50분
///     1교시 09:00~09:40, 2교시 09:50~10:30, 3교시 10:40~11:20,
///     4교시 11:30~12:10 … 이 아니라 아래처럼 점심 앞에서는 쉬는시간을 넣지 않는다.
///     4교시 11:10~11:50, 점심 11:50~12:40, 5교시 12:40~13:20
pub fn generate(p: &GenParams) -> AppResult<Vec<Slot>> {
    p.validate()?;

    let mut out = Vec::new();

    for &(day, count) in &p.periods_by_day {
        if !(1..=7).contains(&day) || count <= 0 {
            continue;
        }
        let mut t = p.first_start_min;

        for period in 1..=count {
            out.push(Slot {
                day_of_week: day,
                slot_type: SLOT_PERIOD.into(),
                period_no: Some(period),
                label: format!("{period}교시"),
                start_min: t,
                end_min: t + p.lesson_minutes,
            });
            t += p.lesson_minutes;

            let mut abutted = false;

            // 중간놀이는 보통 쉬는 시간을 대신하므로 바로 이어 붙인다
            if p.recess_after_period > 0 && period == p.recess_after_period && period < count {
                let label = if p.recess_label.trim().is_empty() {
                    RECESS_LABEL.to_string()
                } else {
                    p.recess_label.trim().to_string()
                };
                out.push(Slot {
                    day_of_week: day,
                    slot_type: SLOT_OTHER.into(),
                    period_no: None,
                    label,
                    start_min: t,
                    end_min: t + p.recess_minutes,
                });
                t += p.recess_minutes;
                abutted = true;
            }

            if p.lunch_after_period > 0 && period == p.lunch_after_period {
                out.push(Slot {
                    day_of_week: day,
                    slot_type: SLOT_LUNCH.into(),
                    period_no: None,
                    label: "점심".into(),
                    start_min: t,
                    end_min: t + p.lunch_minutes,
                });
                t += p.lunch_minutes;
                abutted = true;
            }

            if !abutted && period < count {
                t += p.break_minutes;
            }
        }

        // 교시 수가 점심 예정 교시보다 적으면 마지막 교시 뒤에 점심을 붙인다
        // (저학년이 일찍 끝나도 점심은 먹는 경우)
        if p.lunch_after_period > count && count > 0 {
            out.push(Slot {
                day_of_week: day,
                slot_type: SLOT_LUNCH.into(),
                period_no: None,
                label: "점심".into(),
                start_min: t,
                end_min: t + p.lunch_minutes,
            });
        }
    }

    validate_slots(&out)?;
    Ok(out)
}

// ============================================================
//  시간 일괄 조정 — 짜 놓은 구조는 그대로 두고 길이만 다시 계산
// ============================================================

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReflowParams {
    pub first_start_min: i32,
    pub lesson_minutes: i32,
    pub break_minutes: i32,
    pub lunch_minutes: i32,
    /// 중간놀이 등 그 밖의 시간 구간 길이
    pub other_minutes: i32,
}

impl ReflowParams {
    fn validate(&self) -> AppResult<()> {
        if !(0..=1439).contains(&self.first_start_min) {
            return Err(AppError::invalid("1교시 시작 시각이 올바르지 않습니다."));
        }
        if !(5..=180).contains(&self.lesson_minutes) {
            return Err(AppError::invalid("한 교시 수업 시간은 5분에서 180분 사이로 입력해 주세요."));
        }
        if !(0..=120).contains(&self.break_minutes) {
            return Err(AppError::invalid("쉬는 시간은 0분에서 120분 사이로 입력해 주세요."));
        }
        if !(10..=180).contains(&self.lunch_minutes) {
            return Err(AppError::invalid("점심시간은 10분에서 180분 사이로 입력해 주세요."));
        }
        if !(5..=120).contains(&self.other_minutes) {
            return Err(AppError::invalid("중간놀이 시간은 5분에서 120분 사이로 입력해 주세요."));
        }
        Ok(())
    }
}

/// 교시 수·점심 위치·중간놀이 위치는 건드리지 않고,
/// 입력한 길이대로 시각만 앞에서부터 다시 계산한다.
///
/// 수업과 수업 사이에만 쉬는 시간을 넣는다. 점심·중간놀이는 앞뒤로 바로 이어 붙인다
/// (중간놀이가 그 자리의 쉬는 시간을 대신하기 때문).
pub fn reflow(slots: &[Slot], p: &ReflowParams) -> AppResult<Vec<Slot>> {
    p.validate()?;

    let mut out: Vec<Slot> = Vec::with_capacity(slots.len());

    for day in 1..=7 {
        let mut rows: Vec<Slot> = slots
            .iter()
            .filter(|s| s.day_of_week == day)
            .cloned()
            .collect();
        if rows.is_empty() {
            continue;
        }
        rows.sort_by_key(|s| (s.start_min, s.end_min));

        let mut t = p.first_start_min;
        for i in 0..rows.len() {
            let minutes = match rows[i].slot_type.as_str() {
                SLOT_LUNCH => p.lunch_minutes,
                SLOT_PERIOD => p.lesson_minutes,
                _ => p.other_minutes,
            };
            rows[i].start_min = t;
            rows[i].end_min = t + minutes;
            t += minutes;

            let this_is_lesson = rows[i].slot_type == SLOT_PERIOD;
            let next_is_lesson = rows
                .get(i + 1)
                .map(|n| n.slot_type == SLOT_PERIOD)
                .unwrap_or(false);
            if this_is_lesson && next_is_lesson {
                t += p.break_minutes;
            }
        }

        out.extend(rows);
    }

    validate_slots(&out)?;
    Ok(out)
}

/// 여러 시정표 유형에 한꺼번에 적용한다.
pub fn reflow_profiles(
    conn: &Connection,
    term_id: i64,
    profile_ids: &[i64],
    p: &ReflowParams,
) -> AppResult<()> {
    for id in profile_ids {
        ensure_profile(conn, term_id, *id)?;
        let slots = slots_of(conn, *id)?;
        if slots.is_empty() {
            continue;
        }
        let next = reflow(&slots, p)?;
        save_slots(conn, term_id, *id, &next)?;
    }
    Ok(())
}

// ============================================================
//  조회
// ============================================================

pub fn overview(conn: &Connection) -> AppResult<BellOverview> {
    let sc = school::get(conn)?.ok_or_else(|| {
        AppError::setup_required("학교 기본 설정을 먼저 완료해 주세요. 학년과 학급이 있어야 시정표를 만들 수 있습니다.")
    })?;
    let term_id = school::current_term_id(conn)?;

    let mut stmt =
        conn.prepare("SELECT id, name FROM bell_schedules WHERE term_id = ?1 ORDER BY id")?;
    let rows: Vec<(i64, String)> = stmt
        .query_map([term_id], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;

    let mut profiles = Vec::with_capacity(rows.len());
    for (id, name) in rows {
        profiles.push(BellProfile {
            id,
            name,
            grades: grades_of(conn, term_id, id)?,
            slots: slots_of(conn, id)?,
        });
    }

    let unassigned_grades: Vec<i32> = (sc.min_grade..=sc.max_grade)
        .filter(|g| !profiles.iter().any(|p| p.grades.contains(g)))
        .collect();

    Ok(BellOverview {
        profiles,
        unassigned_grades,
        min_grade: sc.min_grade,
        max_grade: sc.max_grade,
        school_days: sc.school_days,
    })
}

fn grades_of(conn: &Connection, term_id: i64, profile_id: i64) -> AppResult<Vec<i32>> {
    let mut stmt = conn.prepare(
        "SELECT grade FROM grade_bell_map
          WHERE term_id = ?1 AND bell_schedule_id = ?2 ORDER BY grade",
    )?;
    let rows: Vec<i32> = stmt
        .query_map(params![term_id, profile_id], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

fn slots_of(conn: &Connection, profile_id: i64) -> AppResult<Vec<Slot>> {
    let mut stmt = conn.prepare(
        "SELECT day_of_week, slot_type, period_no, label, start_min, end_min
           FROM bell_slots WHERE bell_schedule_id = ?1
          ORDER BY day_of_week, start_min",
    )?;
    let rows: Vec<Slot> = stmt
        .query_map([profile_id], |r| {
            Ok(Slot {
                day_of_week: r.get(0)?,
                slot_type: r.get(1)?,
                period_no: r.get(2)?,
                label: r.get(3)?,
                start_min: r.get(4)?,
                end_min: r.get(5)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

// ============================================================
//  변경
// ============================================================

fn ensure_profile(conn: &Connection, term_id: i64, profile_id: i64) -> AppResult<()> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM bell_schedules WHERE id = ?1 AND term_id = ?2",
        params![profile_id, term_id],
        |r| r.get(0),
    )?;
    if n == 0 {
        return Err(AppError::not_found("시정표 유형을 찾을 수 없습니다."));
    }
    Ok(())
}

fn clean_name(name: &str) -> AppResult<String> {
    let n = name.trim();
    if n.is_empty() {
        return Err(AppError::invalid("시정표 유형 이름을 입력해 주세요."));
    }
    if n.chars().count() > 30 {
        return Err(AppError::invalid("시정표 유형 이름은 30자 이내로 입력해 주세요."));
    }
    Ok(n.to_string())
}

pub fn create_profile(conn: &Connection, term_id: i64, name: &str) -> AppResult<i64> {
    let name = clean_name(name)?;
    conn.execute(
        "INSERT INTO bell_schedules(term_id, name) VALUES (?1, ?2)",
        params![term_id, name],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn rename_profile(conn: &Connection, term_id: i64, id: i64, name: &str) -> AppResult<()> {
    ensure_profile(conn, term_id, id)?;
    let name = clean_name(name)?;
    conn.execute(
        "UPDATE bell_schedules SET name = ?2 WHERE id = ?1",
        params![id, name],
    )?;
    Ok(())
}

pub fn delete_profile(conn: &Connection, term_id: i64, id: i64) -> AppResult<()> {
    ensure_profile(conn, term_id, id)?;
    let grades = grades_of(conn, term_id, id)?;
    if !grades.is_empty() {
        let list = grades
            .iter()
            .map(|g| format!("{g}학년"))
            .collect::<Vec<_>>()
            .join(", ");
        return Err(AppError::new(
            "IN_USE",
            format!("{list}이(가) 이 시정표를 쓰고 있어 지울 수 없습니다. 먼저 다른 시정표로 옮겨 주세요."),
        ));
    }
    conn.execute("DELETE FROM bell_schedules WHERE id = ?1", [id])?;
    Ok(())
}

/// 이 유형의 시각을 통째로 바꾼다.
///
/// 지난 보결 기록은 `substitutions`에 실제 시각이 복사되어 있으므로
/// 시정표를 고쳐도 과거 기록은 달라지지 않는다.
pub fn save_slots(conn: &Connection, term_id: i64, profile_id: i64, slots: &[Slot]) -> AppResult<()> {
    ensure_profile(conn, term_id, profile_id)?;
    validate_slots(slots)?;

    conn.execute(
        "DELETE FROM bell_slots WHERE bell_schedule_id = ?1",
        [profile_id],
    )?;

    let mut ins = conn.prepare(
        "INSERT INTO bell_slots
           (bell_schedule_id, day_of_week, slot_type, period_no, label, start_min, end_min)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    )?;
    for s in slots {
        ins.execute(params![
            profile_id,
            s.day_of_week,
            s.slot_type,
            s.period_no,
            s.label,
            s.start_min,
            s.end_min
        ])?;
    }
    Ok(())
}

/// 한 학년은 하나의 시정표만 쓴다. 다른 유형에 있던 학년은 자동으로 옮겨진다.
pub fn assign_grades(
    conn: &Connection,
    term_id: i64,
    profile_id: i64,
    grades: &[i32],
) -> AppResult<()> {
    ensure_profile(conn, term_id, profile_id)?;

    conn.execute(
        "DELETE FROM grade_bell_map WHERE term_id = ?1 AND bell_schedule_id = ?2",
        params![term_id, profile_id],
    )?;
    for g in grades {
        conn.execute(
            "INSERT INTO grade_bell_map(term_id, grade, bell_schedule_id)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(term_id, grade) DO UPDATE SET bell_schedule_id = excluded.bell_schedule_id",
            params![term_id, g, profile_id],
        )?;
    }
    Ok(())
}

// ============================================================
//  처음 시작용 템플릿
// ============================================================

/// 학년을 몇 덩어리로 나눌지에 따른 시작 템플릿.
/// SAME(전 학년 같음) / TWO(저·고학년) / THREE(저·중·고학년)
pub fn apply_template(conn: &Connection, term_id: i64, key: &str, days: &[i32]) -> AppResult<()> {
    let sc = school::get(conn)?
        .ok_or_else(|| AppError::setup_required("학교 기본 설정을 먼저 완료해 주세요."))?;

    let existing: i64 = conn.query_row(
        "SELECT COUNT(*) FROM bell_schedules WHERE term_id = ?1",
        [term_id],
        |r| r.get(0),
    )?;
    if existing > 0 {
        return Err(AppError::invalid(
            "이미 시정표 유형이 있습니다. 새로 시작하려면 기존 유형을 먼저 지워 주세요.",
        ));
    }

    let (lo, hi) = (sc.min_grade, sc.max_grade);
    let all: Vec<i32> = (lo..=hi).collect();

    // (이름, 학년들, 하루 교시 수, 몇 교시 뒤 점심)
    let groups: Vec<(String, Vec<i32>, i32, i32)> = match key {
        "SAME" => vec![("전체 시정표".to_string(), all, 6, 4)],
        "TWO" => {
            let split = (lo + 1).min(hi);
            vec![
                (
                    format!("저학년 시정표 ({lo}~{split}학년)"),
                    (lo..=split).collect(),
                    5,
                    4,
                ),
                (
                    format!("고학년 시정표 ({}~{hi}학년)", split + 1),
                    ((split + 1)..=hi).collect(),
                    6,
                    5,
                ),
            ]
        }
        "THREE" => {
            let a = (lo + 1).min(hi);
            let b = (lo + 3).min(hi);
            vec![
                (
                    format!("저학년 시정표 ({lo}~{a}학년)"),
                    (lo..=a).collect(),
                    5,
                    4,
                ),
                (
                    format!("중학년 시정표 ({}~{b}학년)", a + 1),
                    ((a + 1)..=b).collect(),
                    6,
                    4,
                ),
                (
                    format!("고학년 시정표 ({}~{hi}학년)", b + 1),
                    ((b + 1)..=hi).collect(),
                    6,
                    5,
                ),
            ]
        }
        _ => return Err(AppError::invalid("알 수 없는 시정표 템플릿입니다.")),
    };

    for (name, grades, periods, lunch_after) in groups {
        if grades.is_empty() {
            continue;
        }
        let id = create_profile(conn, term_id, &name)?;
        let params = GenParams {
            first_start_min: 9 * 60,
            lesson_minutes: 40,
            break_minutes: 10,
            lunch_after_period: lunch_after,
            lunch_minutes: 50,
            // 중간놀이는 학교마다 있고 없고가 다르므로 기본 템플릿에는 넣지 않는다.
            // 화면의 [+ 중간놀이 추가] 또는 '처음부터 다시 만들기'에서 넣을 수 있다.
            recess_after_period: 0,
            recess_minutes: 0,
            recess_label: String::new(),
            periods_by_day: days.iter().map(|d| (*d, periods)).collect(),
        };
        let slots = generate(&params)?;
        save_slots(conn, term_id, id, &slots)?;
        assign_grades(conn, term_id, id, &grades)?;
    }

    Ok(())
}

/// 모든 학년에 시정표가 지정되고 각 유형에 시각이 들어 있는지 확인한다.
pub fn readiness(conn: &Connection) -> AppResult<Vec<String>> {
    let ov = overview(conn)?;
    let mut problems = Vec::new();

    if ov.profiles.is_empty() {
        problems.push("시정표 유형이 아직 하나도 없습니다.".into());
    }
    if !ov.unassigned_grades.is_empty() {
        let list = ov
            .unassigned_grades
            .iter()
            .map(|g| format!("{g}학년"))
            .collect::<Vec<_>>()
            .join(", ");
        problems.push(format!("{list}에 사용할 시정표를 지정해 주세요."));
    }
    for p in &ov.profiles {
        if p.slots.is_empty() && !p.grades.is_empty() {
            problems.push(format!("'{}'에 교시 시각이 아직 없습니다.", p.name));
        }
        for d in &ov.school_days {
            if !p.grades.is_empty() && !p.slots.iter().any(|s| s.day_of_week == *d) {
                problems.push(format!(
                    "'{}'의 {}요일에 수업 교시가 없습니다. 수업이 없는 요일이라면 학교 기본 설정에서 그 요일을 빼 주세요.",
                    p.name,
                    day_name(*d)
                ));
            }
        }
    }
    Ok(problems)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::memory_conn;

    fn p(day: i32, no: i32, start: i32, end: i32) -> Slot {
        Slot {
            day_of_week: day,
            slot_type: SLOT_PERIOD.into(),
            period_no: Some(no),
            label: format!("{no}교시"),
            start_min: start,
            end_min: end,
        }
    }

    fn lunch(day: i32, start: i32, end: i32) -> Slot {
        Slot {
            day_of_week: day,
            slot_type: SLOT_LUNCH.into(),
            period_no: None,
            label: "점심".into(),
            start_min: start,
            end_min: end,
        }
    }

    fn hm(h: i32, m: i32) -> i32 {
        h * 60 + m
    }

    // ---------- 자동 생성 ----------

    #[test]
    fn 저학년_기본값은_설계_예시와_같은_시각을_만든다() {
        let g = generate(&GenParams {
            first_start_min: hm(9, 0),
            lesson_minutes: 40,
            break_minutes: 10,
            lunch_after_period: 4,
            lunch_minutes: 50,
            recess_after_period: 0,
            recess_minutes: 0,
            recess_label: String::new(),
            periods_by_day: vec![(1, 5)],
        })
        .unwrap();

        // 09:00 + (수업 40 + 쉬는시간 10) 반복. 점심 앞에서는 쉬는시간을 넣지 않는다.
        let find = |label: &str| g.iter().find(|s| s.label == label).unwrap().clone();
        assert_eq!((find("1교시").start_min, find("1교시").end_min), (hm(9, 0), hm(9, 40)));
        assert_eq!((find("2교시").start_min, find("2교시").end_min), (hm(9, 50), hm(10, 30)));
        assert_eq!((find("3교시").start_min, find("3교시").end_min), (hm(10, 40), hm(11, 20)));
        assert_eq!((find("4교시").start_min, find("4교시").end_min), (hm(11, 30), hm(12, 10)));
        assert_eq!((find("점심").start_min, find("점심").end_min), (hm(12, 10), hm(13, 0)));
        assert_eq!((find("5교시").start_min, find("5교시").end_min), (hm(13, 0), hm(13, 40)));
    }

    #[test]
    fn 고학년_기본값도_설계_예시와_같다() {
        let g = generate(&GenParams {
            first_start_min: hm(9, 0),
            lesson_minutes: 40,
            break_minutes: 10,
            lunch_after_period: 5,
            lunch_minutes: 50,
            recess_after_period: 0,
            recess_minutes: 0,
            recess_label: String::new(),
            periods_by_day: vec![(1, 6)],
        })
        .unwrap();

        let find = |label: &str| g.iter().find(|s| s.label == label).unwrap().clone();
        assert_eq!((find("5교시").start_min, find("5교시").end_min), (hm(12, 20), hm(13, 0)));
        assert_eq!((find("점심").start_min, find("점심").end_min), (hm(13, 0), hm(13, 50)));
        assert_eq!((find("6교시").start_min, find("6교시").end_min), (hm(13, 50), hm(14, 30)));
    }

    #[test]
    fn 요일마다_교시_수를_다르게_만들_수_있다() {
        let g = generate(&GenParams {
            first_start_min: hm(9, 0),
            lesson_minutes: 40,
            break_minutes: 10,
            lunch_after_period: 4,
            lunch_minutes: 50,
            recess_after_period: 0,
            recess_minutes: 0,
            recess_label: String::new(),
            periods_by_day: vec![(1, 6), (3, 4)],
        })
        .unwrap();

        let mon: Vec<i32> = g
            .iter()
            .filter(|s| s.day_of_week == 1)
            .filter_map(|s| s.period_no)
            .collect();
        let wed: Vec<i32> = g
            .iter()
            .filter(|s| s.day_of_week == 3)
            .filter_map(|s| s.period_no)
            .collect();
        assert_eq!(mon, vec![1, 2, 3, 4, 5, 6]);
        assert_eq!(wed, vec![1, 2, 3, 4], "수요일은 4교시까지");
        assert!(
            g.iter().any(|s| s.day_of_week == 3 && s.slot_type == SLOT_LUNCH),
            "짧은 날에도 점심은 있다"
        );
    }

    #[test]
    fn 생성한_시각은_서로_겹치지_않는다() {
        let g = generate(&GenParams {
            first_start_min: hm(8, 40),
            lesson_minutes: 45,
            break_minutes: 5,
            lunch_after_period: 4,
            lunch_minutes: 60,
            recess_after_period: 0,
            recess_minutes: 0,
            recess_label: String::new(),
            periods_by_day: (1..=5).map(|d| (d, 7)).collect(),
        })
        .unwrap();
        assert!(collect_problems(&g).is_empty());
    }

    // ---------- 검증 ----------

    #[test]
    fn 올바른_시정표는_문제가_없다() {
        let slots = vec![p(1, 1, 540, 580), p(1, 2, 590, 630), lunch(1, 710, 760)];
        assert!(collect_problems(&slots).is_empty());
        assert!(validate_slots(&slots).is_ok());
    }

    #[test]
    fn 붙어_있는_교시는_겹친_것이_아니다() {
        let slots = vec![p(1, 1, 540, 580), p(1, 2, 580, 620)];
        assert!(collect_problems(&slots).is_empty());
    }

    #[test]
    fn 종료가_시작보다_빠르면_막는다() {
        let slots = vec![p(1, 1, 600, 540)];
        let e = validate_slots(&slots).unwrap_err();
        assert!(e.user_message.contains("빠르거나 같습니다"), "{}", e.user_message);
        assert!(e.user_message.contains("10:00"), "{}", e.user_message);
    }

    #[test]
    fn 시간이_1분이라도_겹치면_막고_어디가_겹치는지_알려준다() {
        let slots = vec![p(1, 4, hm(11, 10), hm(11, 51)), lunch(1, hm(11, 50), hm(12, 40))];
        let problems = collect_problems(&slots);
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(problems[0].contains("월요일"), "{}", problems[0]);
        assert!(problems[0].contains("겹칩니다"), "{}", problems[0]);
        assert!(problems[0].contains("11:50"), "{}", problems[0]);
    }

    #[test]
    fn 다른_요일끼리는_겹쳐도_괜찮다() {
        let slots = vec![p(1, 1, 540, 580), p(2, 1, 540, 580)];
        assert!(collect_problems(&slots).is_empty());
    }

    #[test]
    fn 같은_교시가_두_번이면_막는다() {
        let slots = vec![p(1, 2, 540, 580), p(1, 2, 600, 640)];
        let problems = collect_problems(&slots);
        assert!(problems.iter().any(|m| m.contains("2교시가 두 번")), "{problems:?}");
    }

    #[test]
    fn 점심이_두_번이면_막는다() {
        let slots = vec![lunch(1, 700, 740), lunch(1, 760, 800)];
        let problems = collect_problems(&slots);
        assert!(problems.iter().any(|m| m.contains("점심시간이 두 번")), "{problems:?}");
    }

    #[test]
    fn 교시_순서와_시각_순서가_어긋나면_알려준다() {
        let slots = vec![p(1, 3, 540, 580), p(1, 2, 600, 640)];
        let problems = collect_problems(&slots);
        assert!(problems.iter().any(|m| m.contains("교시 순서")), "{problems:?}");
    }

    #[test]
    fn 문제가_여러_개면_모두_모아서_보여준다() {
        let slots = vec![p(1, 1, 600, 540), p(2, 1, 700, 700)];
        assert_eq!(collect_problems(&slots).len(), 2);
    }

    // ---------- 저장 / 프로필 ----------

    fn setup_school(c: &Connection) -> i64 {
        use super::super::school::{ClassCount, ClassNaming, SchoolInput};
        school::save(
            c,
            &SchoolInput {
                name: "한빛초".into(),
                school_type: "ELEMENTARY".into(),
                min_grade: 1,
                max_grade: 6,
                school_days: vec![1, 2, 3, 4, 5],
                school_year: 2026,
                semester: 2,
                class_counts: (1..=6).map(|g| ClassCount { grade: g, count: 2 }).collect(),
                naming: ClassNaming::default(),
            },
        )
        .unwrap()
    }

    fn full_week(periods: i32, lunch_after: i32) -> Vec<Slot> {
        generate(&GenParams {
            first_start_min: 540,
            lesson_minutes: 40,
            break_minutes: 10,
            lunch_after_period: lunch_after,
            lunch_minutes: 50,
            recess_after_period: 0,
            recess_minutes: 0,
            recess_label: String::new(),
            periods_by_day: (1..=5).map(|d| (d, periods)).collect(),
        })
        .unwrap()
    }

    #[test]
    fn 두_유형_템플릿은_학년을_빠짐없이_덮는다() {
        let c = memory_conn();
        let term = setup_school(&c);
        apply_template(&c, term, "TWO", &[1, 2, 3, 4, 5]).unwrap();

        let ov = overview(&c).unwrap();
        assert_eq!(ov.profiles.len(), 2);
        assert!(ov.unassigned_grades.is_empty(), "모든 학년에 시정표가 붙어야 한다");
        assert_eq!(ov.profiles[0].grades, vec![1, 2]);
        assert_eq!(ov.profiles[1].grades, vec![3, 4, 5, 6]);
        assert!(readiness(&c).unwrap().is_empty());
    }

    #[test]
    fn 저학년과_고학년의_점심시간이_다르게_만들어진다() {
        let c = memory_conn();
        let term = setup_school(&c);
        apply_template(&c, term, "TWO", &[1, 2, 3, 4, 5]).unwrap();

        let ov = overview(&c).unwrap();
        let lunch_of = |prof: &BellProfile| {
            let s = prof
                .slots
                .iter()
                .find(|s| s.day_of_week == 1 && s.slot_type == SLOT_LUNCH)
                .unwrap();
            (s.start_min, s.end_min)
        };
        let low = lunch_of(&ov.profiles[0]);
        let high = lunch_of(&ov.profiles[1]);
        assert_ne!(low, high, "저학년과 고학년 점심시간은 달라야 한다");
        assert_eq!(low, (hm(12, 10), hm(13, 0)), "저학년은 4교시 뒤 점심");
        assert_eq!(high, (hm(13, 0), hm(13, 50)), "고학년은 5교시 뒤 점심");
        assert!(low.1 <= high.0, "저학년이 먼저 먹고 고학년이 나중에 먹는다");
    }

    #[test]
    fn 한_학년은_하나의_시정표만_쓴다() {
        let c = memory_conn();
        let term = setup_school(&c);
        apply_template(&c, term, "TWO", &[1, 2, 3, 4, 5]).unwrap();
        let ov = overview(&c).unwrap();
        let (low, high) = (ov.profiles[0].id, ov.profiles[1].id);

        assign_grades(&c, term, low, &[1, 2, 3]).unwrap();

        let ov = overview(&c).unwrap();
        assert_eq!(ov.profiles.iter().find(|p| p.id == low).unwrap().grades, vec![1, 2, 3]);
        assert_eq!(
            ov.profiles.iter().find(|p| p.id == high).unwrap().grades,
            vec![4, 5, 6],
            "옮겨간 학년은 이전 유형에서 자동으로 빠져야 한다"
        );
    }

    #[test]
    fn 사용_중인_유형은_지울_수_없고_이유를_알려준다() {
        let c = memory_conn();
        let term = setup_school(&c);
        apply_template(&c, term, "TWO", &[1, 2, 3, 4, 5]).unwrap();
        let id = overview(&c).unwrap().profiles[0].id;

        let e = delete_profile(&c, term, id).unwrap_err();
        assert_eq!(e.code, "IN_USE");
        assert!(e.user_message.contains("1학년"), "{}", e.user_message);

        assign_grades(&c, term, id, &[]).unwrap();
        assert!(delete_profile(&c, term, id).is_ok());
    }

    #[test]
    fn 저장하면_이전_시각을_완전히_대체한다() {
        let c = memory_conn();
        let term = setup_school(&c);
        let id = create_profile(&c, term, "전체 시정표").unwrap();

        save_slots(&c, term, id, &[p(1, 1, 540, 580), p(1, 2, 590, 630)]).unwrap();
        save_slots(&c, term, id, &[p(1, 1, 550, 590)]).unwrap();

        let ov = overview(&c).unwrap();
        assert_eq!(ov.profiles[0].slots.len(), 1);
        assert_eq!(ov.profiles[0].slots[0].start_min, 550);
    }

    #[test]
    fn 겹치는_시각은_저장되지_않는다() {
        let c = memory_conn();
        let term = setup_school(&c);
        let id = create_profile(&c, term, "전체 시정표").unwrap();
        save_slots(&c, term, id, &[p(1, 1, 540, 580)]).unwrap();

        let bad = vec![p(1, 1, 540, 600), p(1, 2, 590, 630)];
        assert!(save_slots(&c, term, id, &bad).is_err());
    }

    #[test]
    fn 학교_설정_전에는_안내_오류가_난다() {
        let c = memory_conn();
        let e = overview(&c).unwrap_err();
        assert_eq!(e.code, "SETUP_REQUIRED");
    }

    #[test]
    fn 지정되지_않은_학년이_있으면_다음_단계로_못_넘어간다() {
        let c = memory_conn();
        let term = setup_school(&c);
        let id = create_profile(&c, term, "전체 시정표").unwrap();
        save_slots(&c, term, id, &full_week(5, 4)).unwrap();
        assign_grades(&c, term, id, &[1, 2, 3]).unwrap();

        let problems = readiness(&c).unwrap();
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(problems[0].contains("4학년, 5학년, 6학년"), "{}", problems[0]);
    }

    #[test]
    fn 수업_요일에_교시가_없으면_알려준다() {
        let c = memory_conn();
        let term = setup_school(&c);
        let id = create_profile(&c, term, "전체 시정표").unwrap();
        let mon_to_thu = generate(&GenParams {
            first_start_min: 540,
            lesson_minutes: 40,
            break_minutes: 10,
            lunch_after_period: 4,
            lunch_minutes: 50,
            recess_after_period: 0,
            recess_minutes: 0,
            recess_label: String::new(),
            periods_by_day: (1..=4).map(|d| (d, 5)).collect(),
        })
        .unwrap();
        save_slots(&c, term, id, &mon_to_thu).unwrap();
        assign_grades(&c, term, id, &[1, 2, 3, 4, 5, 6]).unwrap();

        let problems = readiness(&c).unwrap();
        assert!(problems.iter().any(|m| m.contains("금요일")), "{problems:?}");
    }
}

#[cfg(test)]
mod tests_recess_reflow {
    use super::*;
    use crate::db::memory_conn;

    fn hm(h: i32, m: i32) -> i32 {
        h * 60 + m
    }

    fn gp(count: i32, lunch_after: i32, recess_after: i32, recess_min: i32) -> GenParams {
        GenParams {
            first_start_min: hm(9, 0),
            lesson_minutes: 40,
            break_minutes: 10,
            lunch_after_period: lunch_after,
            lunch_minutes: 50,
            recess_after_period: recess_after,
            recess_minutes: recess_min,
            recess_label: String::new(),
            periods_by_day: vec![(1, count)],
        }
    }

    // ---------- 중간놀이 ----------

    #[test]
    fn 중간놀이는_쉬는시간을_대신해_바로_이어_붙는다() {
        // 2교시 뒤 중간놀이 30분
        let g = generate(&gp(5, 4, 2, 30)).unwrap();
        let find = |label: &str| g.iter().find(|s| s.label == label).unwrap().clone();

        assert_eq!((find("2교시").start_min, find("2교시").end_min), (hm(9, 50), hm(10, 30)));
        // 2교시가 끝나자마자 중간놀이가 시작한다 (쉬는 시간 10분이 따로 붙지 않는다)
        assert_eq!(
            (find("중간놀이").start_min, find("중간놀이").end_min),
            (hm(10, 30), hm(11, 0))
        );
        assert_eq!((find("3교시").start_min, find("3교시").end_min), (hm(11, 0), hm(11, 40)));
        assert_eq!(find("중간놀이").slot_type, SLOT_OTHER);
        assert!(find("중간놀이").period_no.is_none());
    }

    #[test]
    fn 중간놀이를_넣어도_전체가_겹치지_않는다() {
        let mut p = gp(6, 5, 2, 30);
        p.periods_by_day = (1..=5).map(|d| (d, 6)).collect();
        let g = generate(&p).unwrap();
        assert!(collect_problems(&g).is_empty(), "{:?}", collect_problems(&g));
        // 요일마다 교시6 + 점심1 + 중간놀이1 = 8개
        assert_eq!(g.len(), 8 * 5);
    }

    #[test]
    fn 중간놀이_이름을_바꿀_수_있다() {
        let mut p = gp(5, 4, 2, 20);
        p.recess_label = "아침 독서".into();
        let g = generate(&p).unwrap();
        assert!(g.iter().any(|s| s.label == "아침 독서" && s.slot_type == SLOT_OTHER));
    }

    #[test]
    fn 중간놀이와_점심을_같은_교시_뒤에_둘_수는_없다() {
        let e = generate(&gp(5, 3, 3, 30)).unwrap_err();
        assert!(e.user_message.contains("같은 교시 뒤"), "{}", e.user_message);
    }

    #[test]
    fn 마지막_교시_뒤에는_중간놀이를_넣지_않는다() {
        // 4교시까지인 날에 4교시 뒤 중간놀이를 요청해도 넣지 않는다
        let g = generate(&gp(4, 0, 4, 30)).unwrap();
        assert!(!g.iter().any(|s| s.slot_type == SLOT_OTHER));
    }

    #[test]
    fn 이름_없는_시간_구간은_막는다() {
        let bad = vec![Slot {
            day_of_week: 1,
            slot_type: SLOT_OTHER.into(),
            period_no: None,
            label: "   ".into(),
            start_min: 600,
            end_min: 630,
        }];
        let problems = collect_problems(&bad);
        assert!(problems.iter().any(|m| m.contains("이름")), "{problems:?}");
    }

    #[test]
    fn 수업이_아닌_구간에_교시_번호를_붙이면_막는다() {
        let bad = vec![Slot {
            day_of_week: 1,
            slot_type: SLOT_OTHER.into(),
            period_no: Some(2),
            label: "중간놀이".into(),
            start_min: 600,
            end_min: 630,
        }];
        let problems = collect_problems(&bad);
        assert!(problems.iter().any(|m| m.contains("교시 번호")), "{problems:?}");
    }

    // ---------- 시간 일괄 조정 ----------

    fn rp(lesson: i32, brk: i32, lunch: i32, other: i32) -> ReflowParams {
        ReflowParams {
            first_start_min: hm(9, 0),
            lesson_minutes: lesson,
            break_minutes: brk,
            lunch_minutes: lunch,
            other_minutes: other,
        }
    }

    #[test]
    fn 쉬는시간만_바꿔도_뒤_시각이_전부_밀린다() {
        let before = generate(&gp(5, 4, 0, 0)).unwrap();
        let after = reflow(&before, &rp(40, 5, 50, 30)).unwrap();

        let find = |g: &Vec<Slot>, l: &str| g.iter().find(|s| s.label == l).unwrap().clone();
        assert_eq!(find(&after, "1교시").start_min, hm(9, 0), "1교시는 그대로");
        assert_eq!(find(&after, "2교시").start_min, hm(9, 45), "쉬는시간 5분이면 09:45");
        assert_eq!(find(&after, "4교시").end_min, hm(11, 55));
        assert_eq!(find(&after, "점심").start_min, hm(11, 55));
    }

    #[test]
    fn 점심_길이만_바꿔도_구조는_그대로_유지된다() {
        let before = generate(&gp(5, 4, 2, 30)).unwrap();
        let after = reflow(&before, &rp(40, 10, 60, 30)).unwrap();

        assert_eq!(before.len(), after.len(), "칸 개수가 달라지면 안 된다");

        let kinds_before: Vec<&str> = {
            let mut v: Vec<&Slot> = before.iter().collect();
            v.sort_by_key(|s| s.start_min);
            v.iter().map(|s| s.slot_type.as_str()).collect()
        };
        let kinds_after: Vec<&str> = {
            let mut v: Vec<&Slot> = after.iter().collect();
            v.sort_by_key(|s| s.start_min);
            v.iter().map(|s| s.slot_type.as_str()).collect()
        };
        assert_eq!(kinds_before, kinds_after, "교시·점심·중간놀이 순서가 유지되어야 한다");

        let lunch = after.iter().find(|s| s.slot_type == SLOT_LUNCH).unwrap();
        assert_eq!(lunch.end_min - lunch.start_min, 60);
    }

    #[test]
    fn 중간놀이_길이도_일괄_반영된다() {
        let before = generate(&gp(5, 4, 2, 30)).unwrap();
        let after = reflow(&before, &rp(40, 10, 50, 20)).unwrap();

        let recess = after.iter().find(|s| s.slot_type == SLOT_OTHER).unwrap();
        assert_eq!(recess.end_min - recess.start_min, 20);
        // 중간놀이가 10분 줄었으니 3교시도 10분 당겨진다
        let p3 = after.iter().find(|s| s.label == "3교시").unwrap();
        assert_eq!(p3.start_min, hm(10, 50));
    }

    #[test]
    fn 일괄_조정_결과도_겹치지_않는다() {
        let mut p = gp(6, 5, 2, 30);
        p.periods_by_day = (1..=5).map(|d| (d, 6)).collect();
        let before = generate(&p).unwrap();
        let after = reflow(&before, &rp(45, 5, 60, 25)).unwrap();
        assert!(collect_problems(&after).is_empty(), "{:?}", collect_problems(&after));
    }

    #[test]
    fn 요일마다_교시_수가_달라도_각각_다시_계산한다() {
        let mut p = gp(6, 4, 0, 0);
        p.periods_by_day = vec![(1, 6), (3, 4)];
        let before = generate(&p).unwrap();
        let after = reflow(&before, &rp(40, 10, 50, 30)).unwrap();

        let mon: Vec<i32> = after.iter().filter(|s| s.day_of_week == 1).filter_map(|s| s.period_no).collect();
        let wed: Vec<i32> = after.iter().filter(|s| s.day_of_week == 3).filter_map(|s| s.period_no).collect();
        assert_eq!(mon.len(), 6);
        assert_eq!(wed.len(), 4, "수요일 교시 수는 그대로 4교시");

        // 두 요일 모두 1교시는 09:00에 시작
        for d in [1, 3] {
            let first = after.iter().find(|s| s.day_of_week == d && s.period_no == Some(1)).unwrap();
            assert_eq!(first.start_min, hm(9, 0));
        }
    }

    #[test]
    fn 여러_유형에_한꺼번에_반영할_수_있다() {
        use super::super::school::{ClassCount, ClassNaming, SchoolInput};
        let c = memory_conn();
        let term = school::save(
            &c,
            &SchoolInput {
                name: "한빛초".into(),
                school_type: "ELEMENTARY".into(),
                min_grade: 1,
                max_grade: 6,
                school_days: vec![1, 2, 3, 4, 5],
                school_year: 2026,
                semester: 2,
                class_counts: (1..=6).map(|g| ClassCount { grade: g, count: 2 }).collect(),
                naming: ClassNaming::default(),
            },
        )
        .unwrap();
        apply_template(&c, term, "TWO", &[1, 2, 3, 4, 5]).unwrap();

        let ids: Vec<i64> = overview(&c).unwrap().profiles.iter().map(|p| p.id).collect();
        reflow_profiles(&c, term, &ids, &rp(45, 5, 60, 30)).unwrap();

        let ov = overview(&c).unwrap();
        for prof in &ov.profiles {
            let first = prof
                .slots
                .iter()
                .find(|s| s.day_of_week == 1 && s.period_no == Some(1))
                .unwrap();
            assert_eq!(first.end_min - first.start_min, 45, "'{}' 수업 45분", prof.name);

            let lunch = prof
                .slots
                .iter()
                .find(|s| s.day_of_week == 1 && s.slot_type == SLOT_LUNCH)
                .unwrap();
            assert_eq!(lunch.end_min - lunch.start_min, 60, "'{}' 점심 60분", prof.name);
        }

        // 점심 위치(저학년 4교시 뒤 / 고학년 5교시 뒤)는 그대로 유지된다
        let low_lunch = ov.profiles[0]
            .slots
            .iter()
            .find(|s| s.day_of_week == 1 && s.slot_type == SLOT_LUNCH)
            .unwrap();
        let high_lunch = ov.profiles[1]
            .slots
            .iter()
            .find(|s| s.day_of_week == 1 && s.slot_type == SLOT_LUNCH)
            .unwrap();
        assert!(low_lunch.start_min < high_lunch.start_min, "저학년이 먼저 먹는 순서는 유지");
    }

    #[test]
    fn 말이_안_되는_길이는_막는다() {
        let before = generate(&gp(5, 4, 0, 0)).unwrap();
        assert!(reflow(&before, &rp(0, 10, 50, 30)).is_err());
        assert!(reflow(&before, &rp(40, 10, 0, 30)).is_err());
        assert!(reflow(&before, &rp(40, 10, 50, 0)).is_err());
    }
}
