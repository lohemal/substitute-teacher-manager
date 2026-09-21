//! STEP 1 — 보결 후보 걸러내기.
//!
//! 여기서 하는 일은 **가능/불가능 판정**뿐이다. 추천 순서는 Phase 7에서
//! 학교별 기준으로 정렬한다.
//!
//! 판정은 교시 번호가 아니라 **실제 시각 구간**으로만 한다. 그래서
//!  - 학년마다 교시 시각이 달라도 정확하고,
//!  - 저학년 담임이 이미 하교했으면 고학년 보결 후보가 되고,
//!  - 학년이 다르다는 이유만으로는 절대 빼지 않는다.

use std::collections::HashMap;

use serde::Serialize;

use super::meal::Meal;
use super::schedule::{
    build_busy_index, find_slot, resolve_meal, BusyKind, ClassInfo, DaySnapshot, SlotInfo,
    TeacherInfo, ROLE_HOMEROOM, ROLE_SPECIAL, SLOT_LUNCH, SLOT_PERIOD,
};
use super::time::{fmt_range, Interval};

// ============================================================
//  판정 코드 — 개발자 확인용. 화면에는 한국어 상태만 보여 준다.
// ============================================================

pub const EXCLUDED_NOT_SUBSTITUTABLE: &str = "EXCLUDED_NOT_SUBSTITUTABLE";
pub const EXCLUDED_INACTIVE: &str = "EXCLUDED_INACTIVE";
pub const EXCLUDED_ABSENT_TEACHER: &str = "EXCLUDED_ABSENT_TEACHER";
pub const EXCLUDED_ABSENCE: &str = "EXCLUDED_ABSENCE";
pub const EXCLUDED_ALREADY_ASSIGNED: &str = "EXCLUDED_ALREADY_ASSIGNED";
pub const EXCLUDED_REGULAR_CLASS: &str = "EXCLUDED_REGULAR_CLASS";
pub const EXCLUDED_SPECIAL_LESSON: &str = "EXCLUDED_SPECIAL_LESSON";
pub const EXCLUDED_LUNCH_DUTY: &str = "EXCLUDED_LUNCH_DUTY";
pub const EXCLUDED_RECESS_DUTY: &str = "EXCLUDED_RECESS_DUTY";
pub const EXCLUDED_FIXED_DUTY: &str = "EXCLUDED_FIXED_DUTY";
pub const EXCLUDED_NO_SUB_BLOCK: &str = "EXCLUDED_NO_SUB_BLOCK";

/// 전담교사 식사시간과 겹친다. **일반 수업 보결에만 걸린다** —
/// 전담교사는 자기 식사시간에도 점심 보결은 맡을 수 있다.
pub const EXCLUDED_SPECIAL_MEAL: &str = "EXCLUDED_SPECIAL_MEAL";

// 학교가 설정으로 끈 경우 — 시간은 비어 있지만 방침상 부르지 않는다
pub const EXCLUDED_BY_OPTION_SPECIAL: &str = "EXCLUDED_BY_OPTION_SPECIAL";
pub const EXCLUDED_BY_OPTION_OTHER_GRADE: &str = "EXCLUDED_BY_OPTION_OTHER_GRADE";
pub const EXCLUDED_BY_OPTION_AFTER_END: &str = "EXCLUDED_BY_OPTION_AFTER_END";

pub const ELIGIBLE_HOMEROOM_FREE: &str = "ELIGIBLE_HOMEROOM_FREE";
pub const ELIGIBLE_SPECIALIST_FREE: &str = "ELIGIBLE_SPECIALIST_FREE";
pub const ELIGIBLE_AFTER_SCHOOL_END: &str = "ELIGIBLE_AFTER_SCHOOL_END";
pub const ELIGIBLE_BEFORE_SCHOOL_START: &str = "ELIGIBLE_BEFORE_SCHOOL_START";
pub const ELIGIBLE_NO_SCHEDULE: &str = "ELIGIBLE_NO_SCHEDULE";
pub const ELIGIBLE_FREE: &str = "ELIGIBLE_FREE";

/// 판정 코드를 학교에서 쓰는 말로 바꾼다.
pub fn status_label(code: &str) -> &'static str {
    match code {
        EXCLUDED_NOT_SUBSTITUTABLE => "보결 대상 아님",
        EXCLUDED_INACTIVE => "비활성",
        EXCLUDED_ABSENT_TEACHER => "결근 당사자",
        EXCLUDED_ABSENCE => "부재 중",
        EXCLUDED_ALREADY_ASSIGNED => "다른 보결 배정됨",
        EXCLUDED_REGULAR_CLASS => "수업 중",
        EXCLUDED_SPECIAL_LESSON => "전담 수업 중",
        EXCLUDED_LUNCH_DUTY => "점심 지도",
        EXCLUDED_RECESS_DUTY => "중간놀이 지도",
        EXCLUDED_FIXED_DUTY => "다른 일정 있음",
        EXCLUDED_NO_SUB_BLOCK => "보결 배정 불가 시간",
        EXCLUDED_SPECIAL_MEAL => "식사시간",
        EXCLUDED_BY_OPTION_SPECIAL => "설정: 전담 제외",
        EXCLUDED_BY_OPTION_OTHER_GRADE => "설정: 다른 학년 담임 제외",
        EXCLUDED_BY_OPTION_AFTER_END => "설정: 수업 끝난 담임 제외",

        ELIGIBLE_HOMEROOM_FREE => "담임 공강",
        ELIGIBLE_SPECIALIST_FREE => "전담 공강",
        ELIGIBLE_AFTER_SCHOOL_END => "수업 종료",
        ELIGIBLE_BEFORE_SCHOOL_START => "수업 시작 전",
        ELIGIBLE_NO_SCHEDULE => "수업 없음",
        ELIGIBLE_FREE => "공강",
        _ => "확인 필요",
    }
}

// ============================================================
//  요청 / 결과
// ============================================================

#[derive(Debug, Clone)]
pub struct FindRequest {
    pub class_id: i64,
    /// PERIOD | LUNCH
    pub slot_type: String,
    pub period_no: Option<i32>,
    pub absent_teacher_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetSlot {
    pub date: String,
    pub day_of_week: i32,
    pub class_id: i64,
    pub grade: i32,
    pub class_no: i32,
    /// '5-가람'
    pub class_label: String,
    /// '5학년 가람반'
    pub class_full_label: String,
    pub slot_type: String,
    pub period_no: Option<i32>,
    /// '5교시' 또는 '점심'
    pub slot_label: String,
    pub start_min: i32,
    pub end_min: i32,
    /// 원래 이 시간을 담당하는 교사
    pub in_charge: InCharge,
}

/// 이 시간을 원래 담당하는 교사.
///
/// 전담이 들어오는 시간이면 그 전담교사, 그 밖에는 그 반 담임이다.
/// **담임이 결근했더라도 전담 시간이면 보결이 필요하지 않으므로** 화면에서 이 정보를
/// 먼저 보여 주어 헛배정을 막는다.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InCharge {
    pub teacher_id: Option<i64>,
    pub name: Option<String>,
    pub role_code: Option<String>,
    pub role_label: Option<String>,
    pub subject_name: Option<String>,
    /// 담임 대신 다른 교사가 수업하는 시간인가
    pub covered_by_other: bool,
    /// 그 반 담임이 담당하는 시간인가
    pub is_homeroom: bool,
}

/// 조회 결과 맨 위에 크게 보여 줄 알림.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SlotNotice {
    /// SPECIAL_LESSON | NO_HOMEROOM
    pub kind: String,
    pub title: String,
    pub body: String,
}

/// 보결 횟수 (참고 표시용. Phase 7에서 정렬에 쓴다)
#[derive(Debug, Clone, Copy, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubCounts {
    pub today: i32,
    pub month: i32,
    pub total: i32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockView {
    pub kind: BusyKind,
    pub label: String,
    pub start_min: i32,
    pub end_min: i32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    /// 추천 순위 (1부터). 불가능한 후보는 0
    pub rank: i32,
    /// 추천 근거 — '동학년 · 오늘 0회' 처럼 사람이 읽는 문구
    pub reason: String,
    /// 바로 다음 후보와 순서를 가른 기준 (도움말용)
    pub decided_by: Option<String>,
    pub teacher_id: i64,
    pub name: String,
    pub role_code: String,
    pub role_label: String,
    /// 담임이면 담당 학급, 전담이면 과목, 기타면 담당 업무
    pub duty: String,
    pub eligible: bool,
    /// 개발자 확인용 코드
    pub reason_code: String,
    /// 화면에 보여 주는 한국어 상태
    pub status_label: String,
    /// 왜 그렇게 판정했는지 (겹친 일정 등)
    pub detail: Option<String>,
    pub counts: SubCounts,
    /// 담임인 경우 담당 학년 (Phase 7 동학년 판단에 쓴다)
    pub homeroom_grades: Vec<i32>,
    /// 그 날 이 교사의 모든 일정 — 개발자 확인용
    pub blocks: Vec<BlockView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FindResult {
    pub slot: TargetSlot,
    pub eligible: Vec<Candidate>,
    pub excluded: Vec<Candidate>,
    pub warnings: Vec<String>,
    /// 배정 전에 꼭 확인할 것 (전담 시간 등)
    pub notice: Option<SlotNotice>,
}

#[derive(Debug, Clone)]
pub enum FindError {
    ClassNotFound,
    /// 그 학년·요일에 그 교시(또는 점심)가 없다
    SlotNotFound {
        class_full_label: String,
        day_of_week: i32,
        slot_label: String,
    },
}

// ============================================================
//  판정
// ============================================================

fn duty_of(t: &TeacherInfo, classes: &[ClassInfo]) -> String {
    let homerooms: Vec<String> = classes
        .iter()
        .filter(|c| c.homeroom_teacher_id == Some(t.id))
        .map(|c| c.label.clone())
        .collect();
    if !homerooms.is_empty() {
        return homerooms.join(", ");
    }
    if !t.subjects.is_empty() {
        return t.subjects.join(", ");
    }
    t.memo.clone().unwrap_or_else(|| "—".to_string())
}

fn exclusion_code(kind: BusyKind) -> &'static str {
    match kind {
        BusyKind::Absence => EXCLUDED_ABSENCE,
        BusyKind::Substitution => EXCLUDED_ALREADY_ASSIGNED,
        BusyKind::NoSubBlock => EXCLUDED_NO_SUB_BLOCK,
        BusyKind::Lesson => EXCLUDED_SPECIAL_LESSON,
        BusyKind::HomeroomLesson => EXCLUDED_REGULAR_CLASS,
        BusyKind::Lunch => EXCLUDED_LUNCH_DUTY,
        BusyKind::Recess => EXCLUDED_RECESS_DUTY,
        BusyKind::FixedDuty => EXCLUDED_FIXED_DUTY,
    }
}

/// 보결 시간에 이 학급에 전담이 들어와 담임이 비는지.
fn homeroom_free_here(
    snap: &DaySnapshot,
    teacher_id: i64,
    target: &Interval,
) -> bool {
    for cls in snap
        .classes
        .iter()
        .filter(|c| c.homeroom_teacher_id == Some(teacher_id))
    {
        // 이 학급의 그 요일 교시 중 보결 시간과 겹치는 것
        for slot in snap.slots.iter().filter(|s| {
            s.grade == cls.grade && s.day_of_week == snap.day_of_week && s.slot_type == SLOT_PERIOD
        }) {
            if !Interval::new(slot.start_min, slot.end_min).overlaps(target) {
                continue;
            }
            let period = slot.period_no.unwrap_or(0);
            let covered = snap
                .lessons
                .iter()
                .any(|l| l.replaces_homeroom && l.class_id == cls.id && l.period_no == period);
            if covered {
                return true;
            }
        }
    }
    false
}

/// 후보 가능한 이유를 정한다.
fn eligible_code(snap: &DaySnapshot, teacher_id: i64, role_code: &str, target: &Interval) -> String {
    let idx = build_busy_index(snap);
    let blocks = idx.of(teacher_id);

    // 순서가 중요하다. **그 시간에 대해 더 구체적으로 설명해 주는 이유를 먼저** 고른다.
    // (예: 그날 수업이 늦게 시작하는 전담교사에게 '수업 시작 전'보다 '전담 공강'이 유용하다)
    if blocks.is_empty() {
        return ELIGIBLE_NO_SCHEDULE.to_string();
    }
    if homeroom_free_here(snap, teacher_id, target) {
        return ELIGIBLE_HOMEROOM_FREE.to_string();
    }
    if blocks.iter().all(|b| b.interval.end <= target.start) {
        return ELIGIBLE_AFTER_SCHOOL_END.to_string();
    }
    if role_code == ROLE_SPECIAL {
        return ELIGIBLE_SPECIALIST_FREE.to_string();
    }
    if blocks.iter().all(|b| b.interval.start >= target.end) {
        return ELIGIBLE_BEFORE_SCHOOL_START.to_string();
    }
    ELIGIBLE_FREE.to_string()
}

/// 이 시간을 원래 담당하는 교사를 찾는다.
///
/// 전담(또는 교차) 수업이 들어오는 교시면 그 교사, 그 밖에는 그 반 담임이다.
pub fn in_charge_of(
    snap: &DaySnapshot,
    class_id: i64,
    slot_type: &str,
    period_no: Option<i32>,
) -> InCharge {
    let empty = InCharge {
        teacher_id: None,
        name: None,
        role_code: None,
        role_label: None,
        subject_name: None,
        covered_by_other: false,
        is_homeroom: false,
    };

    let Some(cls) = snap.classes.iter().find(|c| c.id == class_id) else {
        return empty;
    };

    let describe = |teacher_id: i64, subject: Option<String>, covered: bool, homeroom: bool| {
        match snap.teachers.iter().find(|t| t.id == teacher_id) {
            Some(t) => InCharge {
                teacher_id: Some(t.id),
                name: Some(t.name.clone()),
                role_code: Some(t.role_code.clone()),
                role_label: Some(t.role_label.clone()),
                // 대신 들어오는 수업에 과목이 비어 있으면 그 교사의 담당 과목으로 채운다
                subject_name: subject.or_else(|| {
                    if covered {
                        t.subjects.first().cloned()
                    } else {
                        None
                    }
                }),
                covered_by_other: covered,
                is_homeroom: homeroom,
            },
            None => empty.clone(),
        }
    };

    // 교시라면 그 반에 들어오는 수업을 먼저 본다
    if slot_type == SLOT_PERIOD {
        if let Some(p) = period_no {
            if let Some(l) = snap
                .lessons
                .iter()
                .find(|l| l.replaces_homeroom && l.class_id == class_id && l.period_no == p)
            {
                return describe(l.teacher_id, l.subject_name.clone(), true, false);
            }
        }
    }

    // 그 밖에는 담임이 담당한다 (점심시간 포함)
    match cls.homeroom_teacher_id {
        Some(tid) => describe(tid, None, false, true),
        None => empty,
    }
}

/// 배정 전에 확인할 알림을 만든다.
///
/// 현장에서 가장 흔한 헛배정이 **전담 시간에 보결을 넣는 것**이다. 담임이 결근해도
/// 그 시간은 전담이 수업하므로 보결이 필요하지 않다. 그래서 담당 교사가 담임이 아니면
/// 결과 맨 위에 크게 알린다. 다만 그 전담 본인이 결근한 경우라면 보결이 맞으므로 알리지 않는다.
pub fn build_notice(
    in_charge: &InCharge,
    class_label: &str,
    slot_label: &str,
    absent_teacher_id: Option<i64>,
) -> Option<SlotNotice> {
    if in_charge.covered_by_other {
        // 담당 교사 본인이 결근한 경우라면 보결이 맞다
        if absent_teacher_id.is_some() && absent_teacher_id == in_charge.teacher_id {
            return None;
        }

        let who = in_charge.name.clone().unwrap_or_else(|| "다른 선생님".into());
        let role = in_charge.role_label.clone().unwrap_or_else(|| "담당".into());
        let subject = in_charge.subject_name.clone();
        let head = match (&subject, in_charge.role_code.as_deref()) {
            (Some(sub), Some(ROLE_SPECIAL)) => format!("전담({sub})"),
            (Some(sub), _) => format!("{who} 선생님({sub})"),
            (None, _) => format!("{who} 선생님"),
        };
        let detail = match &subject {
            Some(sub) => format!("{who} 선생님({role}·{sub})"),
            None => format!("{who} 선생님({role})"),
        };

        return Some(SlotNotice {
            kind: "SPECIAL_LESSON".to_string(),
            title: format!("{head} 시간입니다. 확인 바랍니다."),
            body: format!(
                "{class_label} {slot_label}는 담임이 아니라 {detail}이 수업하는 시간입니다. \
                 담임 선생님이 결근하셨다면 이 시간은 보결이 필요하지 않습니다. \
                 {who} 선생님이 결근한 경우에만 보결을 배정해 주세요."
            ),
        });
    }

    if in_charge.teacher_id.is_none() {
        return Some(SlotNotice {
            kind: "NO_HOMEROOM".to_string(),
            title: "담임이 지정되지 않은 학급입니다.".to_string(),
            body: format!(
                "{class_label}의 담임이 없어 이 시간을 누가 담당하는지 알 수 없습니다. \
                 교사 관리에서 담임을 먼저 지정해 주세요."
            ),
        });
    }

    None
}

/// 보결 시간을 정한다. 학년·요일·교시를 실제 시각으로 바꾼다.
pub fn resolve_slot(snap: &DaySnapshot, req: &FindRequest) -> Result<TargetSlot, FindError> {
    let cls = snap
        .classes
        .iter()
        .find(|c| c.id == req.class_id)
        .ok_or(FindError::ClassNotFound)?;

    let period_no = if req.slot_type == SLOT_LUNCH {
        None
    } else {
        req.period_no
    };

    let slot: Option<&SlotInfo> = find_slot(
        &snap.slots,
        cls.grade,
        snap.day_of_week,
        &req.slot_type,
        period_no,
    );

    let Some(slot) = slot else {
        return Err(FindError::SlotNotFound {
            class_full_label: cls.full_label.clone(),
            day_of_week: snap.day_of_week,
            slot_label: if req.slot_type == SLOT_LUNCH {
                "점심".to_string()
            } else {
                format!("{}교시", req.period_no.unwrap_or(0))
            },
        });
    };

    Ok(TargetSlot {
        date: snap.date.clone(),
        day_of_week: snap.day_of_week,
        class_id: cls.id,
        grade: cls.grade,
        class_no: cls.class_no,
        class_label: cls.label.clone(),
        class_full_label: cls.full_label.clone(),
        slot_type: slot.slot_type.clone(),
        period_no: slot.period_no,
        slot_label: slot.label.clone(),
        start_min: slot.start_min,
        end_min: slot.end_min,
        in_charge: in_charge_of(snap, cls.id, &slot.slot_type, slot.period_no),
    })
}

/// 학교 설정 때문에 빠지는 경우를 찾는다.
///
/// **시간이 겹치는지 본 뒤에만** 부른다. 여기서 걸리는 사람은 시간은 비어
/// 있지만 학교 방침으로 부르지 않는 사람이다.
fn excluded_by_option(
    snap: &DaySnapshot,
    t: &TeacherInfo,
    target: &Interval,
    target_grade: i32,
) -> Option<(&'static str, String)> {
    let st = &snap.settings;

    if !st.include_special_teachers && t.role_code == ROLE_SPECIAL {
        return Some((
            EXCLUDED_BY_OPTION_SPECIAL,
            "설정에서 전담 선생님을 보결 후보에 넣지 않도록 해 두었습니다".to_string(),
        ));
    }

    if t.role_code == ROLE_HOMEROOM {
        let grades: Vec<i32> = snap
            .classes
            .iter()
            .filter(|c| c.homeroom_teacher_id == Some(t.id))
            .map(|c| c.grade)
            .collect();

        if !st.include_other_grade_homeroom
            && !grades.is_empty()
            && !grades.contains(&target_grade)
        {
            return Some((
                EXCLUDED_BY_OPTION_OTHER_GRADE,
                format!(
                    "{}학년 담임입니다. 설정에서 다른 학년 담임을 넣지 않도록 해 두었습니다",
                    grades
                        .iter()
                        .map(|g| g.to_string())
                        .collect::<Vec<_>>()
                        .join("·")
                ),
            ));
        }

        if !st.include_after_school_end {
            let blocks = build_busy_index(snap);
            let mine = blocks.of(t.id);
            if !mine.is_empty() && mine.iter().all(|b| b.interval.end <= target.start) {
                let last = mine.last().map(|b| fmt_range(&b.interval)).unwrap_or_default();
                return Some((
                    EXCLUDED_BY_OPTION_AFTER_END,
                    format!("{last}에 그 날 일정이 끝났습니다. 설정에서 이런 경우를 넣지 않도록 해 두었습니다"),
                ));
            }
        }
    }

    None
}

/// 후보를 걸러낸다. 가능/불가능 모두 이유와 함께 돌려준다.
pub fn find_candidates(
    snap: &DaySnapshot,
    req: &FindRequest,
    counts: &HashMap<i64, SubCounts>,
) -> Result<FindResult, FindError> {
    let slot = resolve_slot(snap, req)?;
    let target = Interval::new(slot.start_min, slot.end_min);
    let idx = build_busy_index(snap);

    // 전담교사 식사시간은 **일반 수업 보결**에만 적용한다.
    let is_general_period = slot.slot_type != SLOT_LUNCH;
    let meals: HashMap<i64, Meal> = if is_general_period {
        snap.teachers
            .iter()
            .filter(|t| t.role_code == ROLE_SPECIAL)
            .map(|t| (t.id, resolve_meal(snap, t.id)))
            .collect()
    } else {
        HashMap::new()
    };

    let mut eligible: Vec<Candidate> = Vec::new();
    let mut excluded: Vec<Candidate> = Vec::new();

    for t in &snap.teachers {
        let duty = duty_of(t, &snap.classes);
        let homeroom_grades: Vec<i32> = snap
            .classes
            .iter()
            .filter(|c| c.homeroom_teacher_id == Some(t.id))
            .map(|c| c.grade)
            .collect();
        let blocks: Vec<BlockView> = idx
            .of(t.id)
            .iter()
            .map(|b| BlockView {
                kind: b.kind,
                label: b.label.clone(),
                start_min: b.interval.start,
                end_min: b.interval.end,
            })
            .collect();

        let make = |code: &str, detail: Option<String>| Candidate {
            rank: 0,
            reason: String::new(),
            decided_by: None,
            teacher_id: t.id,
            name: t.name.clone(),
            role_code: t.role_code.clone(),
            role_label: t.role_label.clone(),
            duty: duty.clone(),
            eligible: code.starts_with("ELIGIBLE"),
            reason_code: code.to_string(),
            status_label: status_label(code).to_string(),
            detail,
            counts: counts.get(&t.id).copied().unwrap_or_default(),
            homeroom_grades: homeroom_grades.clone(),
            blocks: blocks.clone(),
        };

        // ---- 사람 자체를 뺄 이유 ----
        if !t.active {
            excluded.push(make(EXCLUDED_INACTIVE, None));
            continue;
        }
        if !t.is_substitutable {
            excluded.push(make(
                EXCLUDED_NOT_SUBSTITUTABLE,
                t.memo.clone().map(|m| format!("{m} — 보결 대상에서 제외됨")),
            ));
            continue;
        }
        if req.absent_teacher_id == Some(t.id) {
            excluded.push(make(
                EXCLUDED_ABSENT_TEACHER,
                Some("이 시간에 결근하는 선생님입니다".to_string()),
            ));
            continue;
        }

        // ---- 시간이 겹치는 일정 ----
        let hits = idx.conflicts(t.id, &target);
        if let Some(first) = hits.first() {
            let detail = hits
                .iter()
                .map(|b| b.describe())
                .collect::<Vec<_>>()
                .join(", ");
            excluded.push(make(exclusion_code(first.kind), Some(detail)));
            continue;
        }

        // ---- 전담교사 식사시간 ----
        //
        // 전담교사도 밥을 먹어야 하므로 그 시간에는 **일반 수업 보결**을
        // 맡길 수 없다. 다만 **점심 보결은 맡을 수 있다** — 그래서 이것을
        // 바쁜 시간(BusyBlock)으로 만들지 않고 여기서 대상 종류를 보고
        // 판단한다.
        //
        // 설정으로 끄고 켜는 것이 아니라 사실의 문제이므로 hard filter 다.
        // 식사시간을 **알 수 없을 때는 빼지 않는다** — 모르면서 빼면 배정할
        // 사람이 없어지고, 왜 없는지도 알 수 없다. 대신 아래에서 알려 준다.
        if is_general_period && t.role_code == ROLE_SPECIAL {
            if let Some(m) = meals.get(&t.id).and_then(|m| m.interval()) {
                if m.overlaps(&target) {
                    excluded.push(make(
                        EXCLUDED_SPECIAL_MEAL,
                        Some(format!("식사시간 {}", fmt_range(&m))),
                    ));
                    continue;
                }
            }
        }

        // ---- 학교가 설정으로 끈 경우 ----
        // 시간은 비어 있지만 방침상 부르지 않는 사람이다. 시간 충돌과 달리
        // 학교가 정하는 것이므로, 왜 빠졌는지 설정 이름과 함께 알려 준다.
        if let Some((code, why)) = excluded_by_option(snap, t, &target, slot.grade) {
            excluded.push(make(code, Some(why)));
            continue;
        }

        // ---- 가능 ----
        let code = eligible_code(snap, t.id, &t.role_code, &target);
        let detail = match code.as_str() {
            ELIGIBLE_HOMEROOM_FREE => Some("이 시간에 학급에 전담 수업이 들어옵니다".to_string()),
            ELIGIBLE_AFTER_SCHOOL_END => idx
                .of(t.id)
                .last()
                .map(|b| format!("{}에 일정이 끝났습니다", fmt_range(&b.interval))),
            _ => None,
        };
        eligible.push(make(&code, detail));
    }

    // Phase 6에서는 정렬을 고정한다 (Phase 7에서 학교별 기준으로 바꾼다)
    let role_rank = |r: &str| match r {
        ROLE_HOMEROOM => 0,
        ROLE_SPECIAL => 1,
        _ => 2,
    };
    let sort_key = |c: &Candidate| {
        (
            role_rank(&c.role_code),
            c.homeroom_grades.first().copied().unwrap_or(99),
            c.name.clone(),
        )
    };
    eligible.sort_by_key(sort_key);
    excluded.sort_by_key(|c| (c.reason_code.clone(), c.name.clone()));

    let mut warnings = Vec::new();
    if eligible.is_empty() {
        warnings.push(
            "이 시간에 배정할 수 있는 선생님이 없습니다. 아래 '배정할 수 없는 선생님'에서 이유를 확인해 주세요."
                .to_string(),
        );
    }
    // 식사시간을 정하지 못한 전담이 후보에 남아 있으면 알려 준다.
    // 그 사람은 지금 **식사시간을 무시한 채** 후보에 들어와 있다.
    {
        let mut unsure: Vec<(&str, &str)> = Vec::new();
        for c in &eligible {
            if let Some(Meal::Unknown(why)) = meals.get(&c.teacher_id) {
                unsure.push((c.name.as_str(), why.message()));
            }
        }
        if !unsure.is_empty() {
            let names: Vec<&str> = unsure.iter().map(|(n, _)| *n).collect();
            let why = unsure[0].1;
            warnings.push(format!(
                "{} 선생님의 식사시간을 정하지 못해 식사시간을 빼고 판단했습니다. {}. (시간표 관리 → 전담교사 시간표)",
                names.join(", "),
                why
            ));
        }
    }

    if snap
        .classes
        .iter()
        .any(|c| c.homeroom_teacher_id.is_none())
    {
        let missing: Vec<String> = snap
            .classes
            .iter()
            .filter(|c| c.homeroom_teacher_id.is_none())
            .map(|c| c.label.clone())
            .collect();
        warnings.push(format!(
            "{}의 담임이 지정되지 않아 그 학급 수업 시간이 계산되지 않습니다.",
            missing.join(", ")
        ));
    }

    let notice = build_notice(
        &slot.in_charge,
        &slot.class_label,
        &slot.slot_label,
        req.absent_teacher_id,
    );

    Ok(FindResult {
        slot,
        eligible,
        excluded,
        warnings,
        notice,
    })
}
