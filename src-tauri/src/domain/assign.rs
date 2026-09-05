//! Phase 8 — 보결 배정 확정과 하루 일괄 보결. **DB를 모르는 순수 계산 계층.**
//!
//! ## 왜 별도의 모듈인가
//!
//! 조회(`find`)와 배정은 판단 기준이 같아야 한다. 조회에서 가능하다고 한 교사가
//! 저장할 때 막히거나, 그 반대가 되면 안 된다. 그래서 배정도 **같은 엔진을 다시
//! 돌려서** 확정한다. 여기에는 그 재검증과, 하루 일괄 보결에 필요한
//! '결근 교사가 그 날 맡은 시간 찾기'만 들어 있다.
//!
//! ## 일괄 배정의 시간 겹침
//!
//! 같은 교사를 2교시와 3교시에 넣는 것은 정상이다. 막아야 하는 것은 **실제 시각이
//! 겹치는** 경우뿐이다. 그래서 한 칸을 확정할 때마다 그 결과를 `DaySnapshot`에
//! 넣어(`occupy`) 다음 칸을 검증한다. 겹침 판단은 조회와 똑같은 코드가 한다.

use std::collections::{HashMap, HashSet};

use super::find::{find_candidates, in_charge_of, Candidate, FindError, FindRequest, SlotNotice};
use super::priority::{rank_candidates, RuleSetting};
use super::schedule::{find_slot, AssignedInfo, DaySnapshot, SLOT_LUNCH, SLOT_PERIOD};
use super::time::Interval;

// ============================================================
//  결근 교사가 그 날 맡은 시간
// ============================================================

/// 보결이 필요한 한 칸이 어떤 성격인지.
pub const DUTY_LESSON: &str = "LESSON";
pub const DUTY_HOMEROOM: &str = "HOMEROOM_LESSON";
pub const DUTY_LUNCH: &str = "LUNCH";

pub fn duty_kind_label(kind: &str) -> &'static str {
    match kind {
        DUTY_LESSON => "전담 수업",
        DUTY_HOMEROOM => "정규 수업",
        DUTY_LUNCH => "점심 지도",
        _ => "기타",
    }
}

#[derive(Debug, Clone)]
pub struct DutySlot {
    pub class_id: i64,
    pub class_label: String,
    pub slot_type: String,
    pub period_no: Option<i32>,
    pub slot_label: String,
    pub start_min: i32,
    pub end_min: i32,
    /// LESSON | HOMEROOM_LESSON | LUNCH
    pub kind: String,
    pub subject_name: Option<String>,
}

impl DutySlot {
    pub fn interval(&self) -> Interval {
        Interval::new(self.start_min, self.end_min)
    }
}

/// 그 교사가 이 날 맡고 있어 **보결이 필요한 시간**을 모두 찾는다.
///
/// `window`를 주면 (일부 시간 결근) 그 구간과 겹치는 것만 남긴다.
pub fn duty_slots(snap: &DaySnapshot, teacher_id: i64, window: Option<Interval>) -> Vec<DutySlot> {
    let mut out: Vec<DutySlot> = Vec::new();
    let class_by_id: HashMap<i64, &_> = snap.classes.iter().map(|c| (c.id, c)).collect();

    // ---------- 1. 직접 입력된 수업 (전담·교차·공동) ----------
    for l in snap.lessons.iter().filter(|l| l.teacher_id == teacher_id) {
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
            continue; // 시정표에 없는 교시 — 시간표 화면이 이미 문제로 알려 준다
        };
        out.push(DutySlot {
            class_id: cls.id,
            class_label: cls.label.clone(),
            slot_type: SLOT_PERIOD.to_string(),
            period_no: slot.period_no,
            slot_label: slot.label.clone(),
            start_min: slot.start_min,
            end_min: slot.end_min,
            kind: DUTY_LESSON.to_string(),
            subject_name: l.subject_name.clone(),
        });
    }

    // ---------- 2. 담임 파생 수업 ----------
    // 자기 반 전체 교시 − 전담이 들어오는 교시. 조회 엔진과 같은 규칙이다.
    if snap.settings.derive_homeroom_schedule {
        let covered: HashSet<(i64, i32)> = snap
            .lessons
            .iter()
            .filter(|l| l.replaces_homeroom)
            .map(|l| (l.class_id, l.period_no))
            .collect();

        for cls in snap
            .classes
            .iter()
            .filter(|c| c.homeroom_teacher_id == Some(teacher_id))
        {
            for slot in snap.slots.iter().filter(|s| {
                s.grade == cls.grade
                    && s.day_of_week == snap.day_of_week
                    && s.slot_type == SLOT_PERIOD
            }) {
                if covered.contains(&(cls.id, slot.period_no.unwrap_or(0))) {
                    continue;
                }
                out.push(DutySlot {
                    class_id: cls.id,
                    class_label: cls.label.clone(),
                    slot_type: SLOT_PERIOD.to_string(),
                    period_no: slot.period_no,
                    slot_label: slot.label.clone(),
                    start_min: slot.start_min,
                    end_min: slot.end_min,
                    kind: DUTY_HOMEROOM.to_string(),
                    subject_name: None,
                });
            }
        }
    }

    // ---------- 3. 점심 지도 ----------
    //
    // 중간놀이는 보결을 배정하지 않는다. 현장에서 그 시간까지 사람을 넣지는
    // 않기 때문이다. (다만 후보 판정에서는 여전히 담임의 '바쁜 시간'이다 —
    // 중간놀이 중인 선생님을 다른 반 보결로 부르지는 않는다.)
    for cls in snap
        .classes
        .iter()
        .filter(|c| c.homeroom_teacher_id == Some(teacher_id))
    {
        if !snap.settings.exclude_homeroom_on_own_lunch {
            continue;
        }
        for slot in snap.slots.iter().filter(|s| {
            s.grade == cls.grade && s.day_of_week == snap.day_of_week && s.slot_type == SLOT_LUNCH
        }) {
            out.push(DutySlot {
                class_id: cls.id,
                class_label: cls.label.clone(),
                slot_type: slot.slot_type.clone(),
                period_no: slot.period_no,
                slot_label: slot.label.clone(),
                start_min: slot.start_min,
                end_min: slot.end_min,
                kind: DUTY_LUNCH.to_string(),
                subject_name: None,
            });
        }
    }

    // 일부 시간 결근이면 그 구간에 걸치는 것만 남긴다
    if let Some(w) = window {
        out.retain(|d| d.interval().overlaps(&w));
    }

    out.sort_by_key(|d| (d.start_min, d.end_min, d.class_id));
    out
}

// ============================================================
//  배정 확정 (저장 직전 재검증)
// ============================================================

#[derive(Debug, Clone)]
pub struct AssignPlan {
    pub class_id: i64,
    /// PERIOD | LUNCH
    pub slot_type: String,
    pub period_no: Option<i32>,
    pub sub_teacher_id: i64,
}

/// 저장할 스냅샷 값. 나중에 설정이 바뀌어도 기록은 이 값 그대로 남는다.
#[derive(Debug, Clone)]
pub struct AssignRow {
    pub day_of_week: i32,
    pub class_id: i64,
    pub grade: i32,
    pub class_no: i32,
    /// '5-가람' — 반 이름이 바뀌어도 이 표기는 그대로 남는다
    pub class_label: String,
    pub class_full_label: String,
    pub slot_type: String,
    pub period_no: Option<i32>,
    pub slot_label: String,
    pub start_min: i32,
    pub end_min: i32,
    pub absent_teacher_id: Option<i64>,
    pub absent_teacher_name: Option<String>,
    pub sub_teacher_id: i64,
    pub sub_teacher_name: String,
    /// 그 시간의 과목 (전담 시간이면 그 과목)
    pub subject_name: Option<String>,
    pub recommend_rank: Option<i32>,
    pub recommend_reason: Option<String>,
    /// 배정 직전에 한 번 더 보여 줄 안내 (전담 시간 등). 막지는 않는다.
    pub notice: Option<SlotNotice>,
}

impl AssignRow {
    pub fn interval(&self) -> Interval {
        Interval::new(self.start_min, self.end_min)
    }
}

#[derive(Debug, Clone)]
pub enum AssignError {
    /// 그 학년·요일에 그 시간이 없다
    Slot(FindError),
    /// 교사를 찾을 수 없다 (지워졌거나 잘못된 요청)
    UnknownTeacher,
    /// 결근 교사를 자기 보결로 넣으려 한다
    SameAsAbsent { name: String },
    /// 조회 이후 상황이 바뀌어 더 이상 배정할 수 없다
    NotEligible {
        name: String,
        /// '수업 중' 같은 한국어 상태
        status: String,
        detail: Option<String>,
    },
}

/// 저장 직전 재검증. **조회와 똑같은 엔진을 다시 돌린다.**
///
/// 조회한 뒤에 다른 보결이 먼저 배정되었거나, 결근이 추가되었거나, 시간표가
/// 바뀌었거나, 보결 대상에서 빠졌다면 여기서 걸러진다.
pub fn verify(
    snap: &DaySnapshot,
    counts: &HashMap<i64, super::find::SubCounts>,
    settings: &[RuleSetting],
    absent_teacher_id: Option<i64>,
    plan: &AssignPlan,
) -> Result<AssignRow, AssignError> {
    if absent_teacher_id == Some(plan.sub_teacher_id) {
        let name = teacher_name(snap, plan.sub_teacher_id).unwrap_or_else(|| "그 선생님".into());
        return Err(AssignError::SameAsAbsent { name });
    }

    let req = FindRequest {
        class_id: plan.class_id,
        slot_type: plan.slot_type.clone(),
        period_no: plan.period_no,
        absent_teacher_id,
    };
    let mut result = find_candidates(snap, &req, counts).map_err(AssignError::Slot)?;
    rank_candidates(&mut result.eligible, settings, result.slot.grade);

    let picked: &Candidate = match result
        .eligible
        .iter()
        .find(|c| c.teacher_id == plan.sub_teacher_id)
    {
        Some(c) => c,
        None => {
            return Err(match result
                .excluded
                .iter()
                .find(|c| c.teacher_id == plan.sub_teacher_id)
            {
                Some(c) => AssignError::NotEligible {
                    name: c.name.clone(),
                    status: c.status_label.to_string(),
                    detail: c.detail.clone(),
                },
                None => AssignError::UnknownTeacher,
            });
        }
    };

    let slot = &result.slot;
    // 그 시간의 과목 — 전담이 들어오는 시간이면 그 과목을 기록해 둔다
    let in_charge = in_charge_of(snap, slot.class_id, &slot.slot_type, slot.period_no);

    Ok(AssignRow {
        day_of_week: slot.day_of_week,
        class_id: slot.class_id,
        grade: slot.grade,
        class_no: slot.class_no,
        class_label: slot.class_label.clone(),
        class_full_label: slot.class_full_label.clone(),
        slot_type: slot.slot_type.clone(),
        period_no: slot.period_no,
        slot_label: slot.slot_label.clone(),
        start_min: slot.start_min,
        end_min: slot.end_min,
        absent_teacher_id,
        absent_teacher_name: absent_teacher_id.and_then(|id| teacher_name(snap, id)),
        sub_teacher_id: picked.teacher_id,
        sub_teacher_name: picked.name.clone(),
        subject_name: in_charge.subject_name.clone(),
        recommend_rank: (picked.rank > 0).then_some(picked.rank),
        recommend_reason: (!picked.reason.is_empty()).then(|| picked.reason.clone()),
        notice: result.notice.clone(),
    })
}

/// 확정한 배정을 하루치 자료에 반영한다.
///
/// 일괄 배정에서 **앞 칸의 결과가 뒤 칸 검증에 보이도록** 하기 위한 것이다.
/// 이렇게 하면 같은 시간 다른 반에 같은 교사를 넣는 일이 자동으로 걸린다.
pub fn occupy(snap: &mut DaySnapshot, row: &AssignRow) {
    snap.assigned.push(AssignedInfo {
        sub_teacher_id: row.sub_teacher_id,
        start_min: row.start_min,
        end_min: row.end_min,
        label: format!("보결 {} {}", row.class_label, row.slot_label),
    });
}

fn teacher_name(snap: &DaySnapshot, id: i64) -> Option<String> {
    snap.teachers.iter().find(|t| t.id == id).map(|t| t.name.clone())
}

/// 사용자에게 보여 줄 한국어 문장.
pub fn error_message(e: &AssignError) -> String {
    match e {
        AssignError::Slot(FindError::ClassNotFound) => {
            "학급을 찾을 수 없습니다. 화면을 새로 고친 뒤 다시 시도해 주세요.".to_string()
        }
        AssignError::Slot(FindError::SlotNotFound {
            class_full_label,
            slot_label,
            ..
        }) => format!(
            "{class_full_label}의 {slot_label}가 시정표에 없습니다. 시정표가 바뀐 것 같습니다. \
             화면을 새로 고친 뒤 다시 확인해 주세요."
        ),
        AssignError::UnknownTeacher => {
            "선택한 선생님을 찾을 수 없습니다. 화면을 새로 고친 뒤 다시 선택해 주세요.".to_string()
        }
        AssignError::SameAsAbsent { name } => {
            format!("{name} 선생님은 이 시간에 결근하시므로 보결로 배정할 수 없습니다.")
        }
        AssignError::NotEligible {
            name,
            status,
            detail,
        } => {
            let head = format!(
                "조회한 뒤 상황이 바뀌었습니다. {name} 선생님은 지금 이 시간에 배정할 수 없습니다 ({status})."
            );
            match detail {
                Some(d) => format!("{head}\n겹치는 일정: {d}"),
                None => head,
            }
        }
    }
}
