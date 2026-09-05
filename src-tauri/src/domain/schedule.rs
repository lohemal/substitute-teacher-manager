//! 하루치 일정을 모아 '교사별로 바쁜 시간 구간'을 만든다.
//!
//! **이 모듈은 DB를 모른다.** `repo::find`가 하루치 자료를 읽어 `DaySnapshot`으로
//! 넘겨 주고, 여기서는 그것만 보고 계산한다. 그래서 DB 없이 시험할 수 있다.
//!
//! ## 담임 수업 시간은 저장하지 않고 계산한다
//!
//!   담임 수업 = (자기 반의 그 요일 전체 교시) − (그 반에 전담이 들어오는 교시)
//!
//! 그래서 전담이 들어온 시간에는 담임이 자동으로 공강 후보가 된다.

use std::collections::{HashMap, HashSet};

use serde::Serialize;

use super::time::{fmt_range, Interval};

pub const SLOT_PERIOD: &str = "PERIOD";
pub const SLOT_LUNCH: &str = "LUNCH";
pub const SLOT_OTHER: &str = "OTHER";

pub const ROLE_HOMEROOM: &str = "HOMEROOM";
pub const ROLE_SPECIAL: &str = "SPECIAL";

// ============================================================
//  하루치 자료 (repo가 채워 준다)
// ============================================================

#[derive(Debug, Clone)]
pub struct TeacherInfo {
    pub id: i64,
    pub name: String,
    pub role_code: String,
    pub role_label: String,
    pub memo: Option<String>,
    pub is_substitutable: bool,
    pub active: bool,
    pub subjects: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ClassInfo {
    pub id: i64,
    pub grade: i32,
    pub class_no: i32,
    pub label: String,
    pub full_label: String,
    pub homeroom_teacher_id: Option<i64>,
}

/// 학년별 시정표의 한 칸.
#[derive(Debug, Clone)]
pub struct SlotInfo {
    pub grade: i32,
    pub day_of_week: i32,
    pub slot_type: String,
    pub period_no: Option<i32>,
    pub label: String,
    pub start_min: i32,
    pub end_min: i32,
}

/// 직접 입력된 수업 (전담·교차·공동).
#[derive(Debug, Clone)]
pub struct LessonInfo {
    pub teacher_id: i64,
    pub class_id: i64,
    pub period_no: i32,
    pub replaces_homeroom: bool,
    pub subject_name: Option<String>,
}

/// 수업 외 고정 일정. 이미 이 날짜/요일에 해당하는 것만 넘어온다.
#[derive(Debug, Clone)]
pub struct DutyInfo {
    pub teacher_id: i64,
    /// DUTY | NO_SUB | OTHER
    pub block_type: String,
    pub label: String,
    pub start_min: i32,
    pub end_min: i32,
}

#[derive(Debug, Clone)]
pub struct AbsenceInfo {
    pub teacher_id: i64,
    pub is_all_day: bool,
    pub start_min: i32,
    pub end_min: i32,
    pub reason_label: Option<String>,
}

/// 이 날 이미 배정된 보결.
#[derive(Debug, Clone)]
pub struct AssignedInfo {
    pub sub_teacher_id: i64,
    pub start_min: i32,
    pub end_min: i32,
    pub label: String,
}

/// 학교마다 다른 **운영 방침**. 시간이 겹치는지 여부는 여기서 다루지 않는다 —
/// 그것은 방침이 아니라 사실이므로 어떤 설정으로도 끌 수 없다.
#[derive(Debug, Clone)]
pub struct EngineSettings {
    /// 담임 수업 시간을 자동 계산할지
    pub derive_homeroom_schedule: bool,
    /// 자기 학년 점심시간에 담임을 후보에서 뺄지
    pub exclude_homeroom_on_own_lunch: bool,
    /// 중간놀이 등 그 밖의 시간 구간에도 담임을 뺄지
    pub exclude_homeroom_on_recess: bool,
    /// 전담 선생님을 후보에 넣을지
    pub include_special_teachers: bool,
    /// 그 날 수업이 끝난 담임을 후보에 넣을지
    pub include_after_school_end: bool,
    /// 다른 학년 담임을 후보에 넣을지
    pub include_other_grade_homeroom: bool,
}

impl Default for EngineSettings {
    fn default() -> Self {
        Self {
            derive_homeroom_schedule: true,
            exclude_homeroom_on_own_lunch: true,
            exclude_homeroom_on_recess: true,
            include_special_teachers: true,
            include_after_school_end: true,
            include_other_grade_homeroom: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DaySnapshot {
    pub date: String,
    pub day_of_week: i32,
    pub teachers: Vec<TeacherInfo>,
    pub classes: Vec<ClassInfo>,
    pub slots: Vec<SlotInfo>,
    pub lessons: Vec<LessonInfo>,
    pub duties: Vec<DutyInfo>,
    pub absences: Vec<AbsenceInfo>,
    pub assigned: Vec<AssignedInfo>,
    pub settings: EngineSettings,
}

// ============================================================
//  바쁜 시간 구간
// ============================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BusyKind {
    /// 직접 입력된 수업 (전담·교차·공동)
    Lesson,
    /// 담임 파생 수업
    HomeroomLesson,
    /// 자기 학년 점심 지도
    Lunch,
    /// 중간놀이 등 그 밖의 학급 시간
    Recess,
    /// 등록된 고정 업무
    FixedDuty,
    /// 보결 배정 불가로 등록된 시간
    NoSubBlock,
    /// 이미 배정된 보결
    Substitution,
    /// 부재 (연가·병가·출장 등)
    Absence,
}

impl BusyKind {
    /// 여러 일정이 함께 겹칠 때 어떤 이유를 먼저 알려 줄지.
    /// 숫자가 작을수록 먼저.
    pub fn priority(&self) -> u8 {
        match self {
            BusyKind::Absence => 0,
            BusyKind::Substitution => 1,
            BusyKind::NoSubBlock => 2,
            BusyKind::Lesson => 3,
            BusyKind::HomeroomLesson => 4,
            BusyKind::Lunch => 5,
            BusyKind::Recess => 6,
            BusyKind::FixedDuty => 7,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BusyBlock {
    pub teacher_id: i64,
    pub interval: Interval,
    pub kind: BusyKind,
    pub label: String,
}

impl BusyBlock {
    pub fn describe(&self) -> String {
        format!("{} ({})", self.label, fmt_range(&self.interval))
    }
}

/// 교사별로 그 날 바쁜 시간 구간을 모아 둔 것.
#[derive(Debug, Clone, Default)]
pub struct BusyIndex {
    by_teacher: HashMap<i64, Vec<BusyBlock>>,
}

impl BusyIndex {
    pub fn of(&self, teacher_id: i64) -> &[BusyBlock] {
        self.by_teacher
            .get(&teacher_id)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// 이 구간과 겹치는 일정들. 알려 줄 순서대로 정렬해서 돌려준다.
    pub fn conflicts(&self, teacher_id: i64, target: &Interval) -> Vec<&BusyBlock> {
        let mut hits: Vec<&BusyBlock> = self
            .of(teacher_id)
            .iter()
            .filter(|b| b.interval.overlaps(target))
            .collect();
        hits.sort_by_key(|b| (b.kind.priority(), b.interval.start));
        hits
    }

    fn push(&mut self, block: BusyBlock) {
        self.by_teacher.entry(block.teacher_id).or_default().push(block);
    }

    fn sort(&mut self) {
        for v in self.by_teacher.values_mut() {
            v.sort_by_key(|b| (b.interval.start, b.interval.end));
        }
    }
}

// ============================================================
//  만들기
// ============================================================

/// 이 학년·요일의 교시 칸을 찾는다.
pub fn find_slot<'a>(
    slots: &'a [SlotInfo],
    grade: i32,
    day: i32,
    slot_type: &str,
    period_no: Option<i32>,
) -> Option<&'a SlotInfo> {
    slots.iter().find(|s| {
        s.grade == grade
            && s.day_of_week == day
            && s.slot_type == slot_type
            && s.period_no == period_no
    })
}

/// 하루치 자료로 교사별 바쁜 시간을 만든다.
pub fn build_busy_index(snap: &DaySnapshot) -> BusyIndex {
    let mut idx = BusyIndex::default();

    let class_by_id: HashMap<i64, &ClassInfo> = snap.classes.iter().map(|c| (c.id, c)).collect();

    // ---------- 1. 직접 입력된 수업 ----------
    // 실제 시각은 '그 학급 학년'의 시정표에서 가져온다.
    for l in &snap.lessons {
        let Some(cls) = class_by_id.get(&l.class_id) else {
            continue;
        };
        let Some(slot) = find_slot(
            &snap.slots,
            cls.grade,
            snap.day_of_week,
            SLOT_PERIOD,
            Some(l.period_no),
        ) else {
            continue; // 시정표에 없는 교시 — 시간표 화면에서 이미 문제로 알려 준다
        };
        let subject = l.subject_name.clone().unwrap_or_else(|| "수업".to_string());
        idx.push(BusyBlock {
            teacher_id: l.teacher_id,
            interval: Interval::new(slot.start_min, slot.end_min),
            kind: BusyKind::Lesson,
            label: format!("{} {} {}", cls.label, subject, slot.label),
        });
    }

    // ---------- 2. 담임 파생 수업 ----------
    if snap.settings.derive_homeroom_schedule {
        // (학급, 교시) 에 담임을 대신하는 수업이 있는가
        let covered: HashSet<(i64, i32)> = snap
            .lessons
            .iter()
            .filter(|l| l.replaces_homeroom)
            .map(|l| (l.class_id, l.period_no))
            .collect();

        for cls in &snap.classes {
            let Some(tid) = cls.homeroom_teacher_id else {
                continue;
            };
            for slot in snap.slots.iter().filter(|s| {
                s.grade == cls.grade && s.day_of_week == snap.day_of_week && s.slot_type == SLOT_PERIOD
            }) {
                let period = slot.period_no.unwrap_or(0);
                if covered.contains(&(cls.id, period)) {
                    continue; // 전담이 들어오는 시간 -> 담임은 공강
                }
                idx.push(BusyBlock {
                    teacher_id: tid,
                    interval: Interval::new(slot.start_min, slot.end_min),
                    kind: BusyKind::HomeroomLesson,
                    label: format!("{} {}", cls.label, slot.label),
                });
            }
        }
    }

    // ---------- 3. 점심 지도 / 중간놀이 ----------
    for cls in &snap.classes {
        let Some(tid) = cls.homeroom_teacher_id else {
            continue;
        };
        for slot in snap
            .slots
            .iter()
            .filter(|s| s.grade == cls.grade && s.day_of_week == snap.day_of_week)
        {
            let (kind, on) = match slot.slot_type.as_str() {
                SLOT_LUNCH => (BusyKind::Lunch, snap.settings.exclude_homeroom_on_own_lunch),
                SLOT_OTHER => (BusyKind::Recess, snap.settings.exclude_homeroom_on_recess),
                _ => continue,
            };
            if !on {
                continue;
            }
            idx.push(BusyBlock {
                teacher_id: tid,
                interval: Interval::new(slot.start_min, slot.end_min),
                kind,
                label: format!("{} {}", cls.label, slot.label),
            });
        }
    }

    // ---------- 4. 등록된 고정 일정 ----------
    for d in &snap.duties {
        idx.push(BusyBlock {
            teacher_id: d.teacher_id,
            interval: Interval::new(d.start_min, d.end_min),
            kind: if d.block_type == "NO_SUB" {
                BusyKind::NoSubBlock
            } else {
                BusyKind::FixedDuty
            },
            label: d.label.clone(),
        });
    }

    // ---------- 5. 이미 배정된 보결 ----------
    for a in &snap.assigned {
        idx.push(BusyBlock {
            teacher_id: a.sub_teacher_id,
            interval: Interval::new(a.start_min, a.end_min),
            kind: BusyKind::Substitution,
            label: a.label.clone(),
        });
    }

    // ---------- 6. 부재 ----------
    for a in &snap.absences {
        let interval = if a.is_all_day {
            Interval::new(0, 1440)
        } else {
            Interval::new(a.start_min, a.end_min)
        };
        idx.push(BusyBlock {
            teacher_id: a.teacher_id,
            interval,
            kind: BusyKind::Absence,
            label: match (&a.reason_label, a.is_all_day) {
                (Some(r), true) => format!("{r} (종일)"),
                (Some(r), false) => r.clone(),
                (None, true) => "부재 (종일)".to_string(),
                (None, false) => "부재".to_string(),
            },
        });
    }

    idx.sort();
    idx
}
