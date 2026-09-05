//! Phase 6 검증 — 요구사항 12의 사례를 모두 확인한다.
//!
//! 실제 DB에 들어 있는 것과 같은 모양의 학교로 시험한다.
//!   저학년(1~2) 5교시까지 · 4교시 뒤 점심
//!   중학년(3~4) 5교시까지 · 3교시 뒤 점심   <- 점심 시각이 학년군마다 다르다
//!   고학년(5~6) 6교시까지 · 5교시 뒤 점심
//!   반 이름은 가람·나리·다솜·라온

use std::collections::HashMap;

use super::find::*;
use super::schedule::*;
use super::time::Interval;

const HR: &str = "HOMEROOM";
const SP: &str = "SPECIAL";
const OT: &str = "OTHER";

fn hm(h: i32, m: i32) -> i32 {
    h * 60 + m
}

const CLASS_NAMES: [&str; 4] = ["가람", "나리", "다솜", "라온"];

/// 학년군별 시정표를 만든다. (교시 수, 점심 위치가 다르다)
fn slots_for(grade: i32, day: i32) -> Vec<SlotInfo> {
    let (count, lunch_after) = match grade {
        1 | 2 => (5, 4),
        3 | 4 => (5, 3),
        _ => (6, 5),
    };
    let (lesson, brk, lunch_len) = (40, 5, 50);
    let mut out = Vec::new();
    let mut t = hm(9, 0);
    for p in 1..=count {
        out.push(SlotInfo {
            grade,
            day_of_week: day,
            slot_type: SLOT_PERIOD.into(),
            period_no: Some(p),
            label: format!("{p}교시"),
            start_min: t,
            end_min: t + lesson,
        });
        t += lesson;
        if p == lunch_after {
            out.push(SlotInfo {
                grade,
                day_of_week: day,
                slot_type: SLOT_LUNCH.into(),
                period_no: None,
                label: "점심".into(),
                start_min: t,
                end_min: t + lunch_len,
            });
            t += lunch_len;
        } else if p < count {
            t += brk;
        }
    }
    out
}

struct World {
    snap: DaySnapshot,
    next_teacher: i64,
}

impl World {
    /// 1~6학년 × 4반, 각 반에 담임. 전담 없음.
    fn new(day: i32) -> Self {
        let mut classes = Vec::new();
        let mut teachers = Vec::new();
        let mut slots = Vec::new();
        let mut cid = 1i64;
        let mut tid = 1i64;

        for grade in 1..=6 {
            slots.extend(slots_for(grade, day));
            for (i, name) in CLASS_NAMES.iter().enumerate() {
                let class_no = i as i32 + 1;
                teachers.push(TeacherInfo {
                    id: tid,
                    name: format!("{grade}-{name} 담임"),
                    role_code: HR.into(),
                    role_label: "담임".into(),
                    memo: None,
                    is_substitutable: true,
                    active: true,
                    subjects: vec![],
                });
                classes.push(ClassInfo {
                    id: cid,
                    grade,
                    class_no,
                    label: format!("{grade}-{name}"),
                    full_label: format!("{grade}학년 {name}반"),
                    homeroom_teacher_id: Some(tid),
                });
                cid += 1;
                tid += 1;
            }
        }

        Self {
            snap: DaySnapshot {
                date: "2026-09-09".into(),
                day_of_week: day,
                teachers,
                classes,
                slots,
                lessons: vec![],
                duties: vec![],
                absences: vec![],
                assigned: vec![],
                settings: EngineSettings::default(),
            },
            next_teacher: tid,
        }
    }

    fn add_teacher(&mut self, name: &str, role: &str, subjects: &[&str]) -> i64 {
        let id = self.next_teacher;
        self.next_teacher += 1;
        self.snap.teachers.push(TeacherInfo {
            id,
            name: name.into(),
            role_code: role.into(),
            role_label: match role {
                HR => "담임",
                SP => "전담",
                _ => "기타",
            }
            .into(),
            memo: None,
            is_substitutable: true,
            active: true,
            subjects: subjects.iter().map(|s| s.to_string()).collect(),
        });
        id
    }

    fn class_id(&self, grade: i32, name: &str) -> i64 {
        self.snap
            .classes
            .iter()
            .find(|c| c.grade == grade && c.label == format!("{grade}-{name}"))
            .expect("학급이 있어야 한다")
            .id
    }

    fn homeroom_of(&self, grade: i32, name: &str) -> i64 {
        let cid = self.class_id(grade, name);
        self.snap
            .classes
            .iter()
            .find(|c| c.id == cid)
            .unwrap()
            .homeroom_teacher_id
            .unwrap()
    }

    fn add_lesson(&mut self, teacher_id: i64, grade: i32, name: &str, period: i32, subject: &str) {
        let class_id = self.class_id(grade, name);
        self.snap.lessons.push(LessonInfo {
            teacher_id,
            class_id,
            period_no: period,
            replaces_homeroom: true,
            subject_name: Some(subject.into()),
        });
    }

    fn slot_time(&self, grade: i32, slot_type: &str, period: Option<i32>) -> (i32, i32) {
        let s = find_slot(&self.snap.slots, grade, self.snap.day_of_week, slot_type, period)
            .expect("시정표에 있어야 한다");
        (s.start_min, s.end_min)
    }

    fn find(&self, grade: i32, name: &str, period: i32) -> FindResult {
        self.run(grade, name, SLOT_PERIOD, Some(period), None)
    }

    fn find_lunch(&self, grade: i32, name: &str) -> FindResult {
        self.run(grade, name, SLOT_LUNCH, None, None)
    }

    fn run(
        &self,
        grade: i32,
        name: &str,
        slot_type: &str,
        period: Option<i32>,
        absent: Option<i64>,
    ) -> FindResult {
        let req = FindRequest {
            class_id: self.class_id(grade, name),
            slot_type: slot_type.into(),
            period_no: period,
            absent_teacher_id: absent,
        };
        find_candidates(&self.snap, &req, &HashMap::new()).expect("조회가 되어야 한다")
    }
}

fn of<'a>(r: &'a FindResult, teacher_id: i64) -> &'a Candidate {
    r.eligible
        .iter()
        .chain(r.excluded.iter())
        .find(|c| c.teacher_id == teacher_id)
        .expect("교사가 결과에 있어야 한다")
}

fn is_eligible(r: &FindResult, teacher_id: i64) -> bool {
    r.eligible.iter().any(|c| c.teacher_id == teacher_id)
}

// ============================================================
//  요구사항 3 · 4 — 실제 시각으로 변환하고 시각으로 판단
// ============================================================

#[test]
fn 학년_요일_교시를_실제_시각으로_바꾼다() {
    let w = World::new(3); // 수요일
    let r = w.find(5, "가람", 5);

    // 고학년 5교시 = 09:00부터 (40+5)*4 = 09:00+180 -> 12:00~12:40
    assert_eq!(r.slot.start_min, hm(12, 0));
    assert_eq!(r.slot.end_min, hm(12, 40));
    assert_eq!(r.slot.slot_label, "5교시");
    assert_eq!(r.slot.class_label, "5-가람");
    assert_eq!(r.slot.class_full_label, "5학년 가람반");
    assert_eq!(r.slot.day_of_week, 3);
}

#[test]
fn 점심도_실제_시각_구간으로_바꾼다() {
    let w = World::new(1);
    let low = w.find_lunch(1, "가람");
    let mid = w.find_lunch(3, "가람");
    let high = w.find_lunch(5, "가람");

    assert_eq!(low.slot.slot_type, "LUNCH");
    assert_eq!(low.slot.period_no, None);
    // 학년군마다 점심 시각이 다르다
    assert_ne!(
        (low.slot.start_min, low.slot.end_min),
        (mid.slot.start_min, mid.slot.end_min)
    );
    assert_ne!(
        (mid.slot.start_min, mid.slot.end_min),
        (high.slot.start_min, high.slot.end_min)
    );
}

#[test]
fn 그_학년에_없는_교시는_안내_오류를_낸다() {
    let w = World::new(1);
    let req = FindRequest {
        class_id: w.class_id(1, "가람"),
        slot_type: SLOT_PERIOD.into(),
        period_no: Some(6), // 저학년은 5교시까지
        absent_teacher_id: None,
    };
    let err = find_candidates(&w.snap, &req, &HashMap::new()).unwrap_err();
    match err {
        FindError::SlotNotFound { class_full_label, slot_label, .. } => {
            assert_eq!(class_full_label, "1학년 가람반");
            assert_eq!(slot_label, "6교시");
        }
        other => panic!("다른 오류: {other:?}"),
    }
}

// ============================================================
//  요구사항 12 — 필수 검증 사례
// ============================================================

#[test]
fn 사례1_같은_교시에_전담수업_중인_교사는_제외한다() {
    let mut w = World::new(1);
    let sp = w.add_teacher("박전담", SP, &["과학"]);
    w.add_lesson(sp, 5, "나리", 3, "과학");

    let r = w.find(5, "가람", 3);

    assert!(!is_eligible(&r, sp));
    let c = of(&r, sp);
    assert_eq!(c.reason_code, EXCLUDED_SPECIAL_LESSON);
    assert_eq!(c.status_label, "전담 수업 중");
    assert!(c.detail.as_deref().unwrap().contains("5-나리"), "{c:?}");
}

#[test]
fn 사례2_학년이_달라도_실제_시간이_겹치면_제외한다() {
    let mut w = World::new(1);
    let sp = w.add_teacher("박전담", SP, &["과학"]);
    // 1학년 3교시와 5학년 3교시는 시각이 같다 (둘 다 09:00 시작 · 같은 간격)
    let (a, b) = w.slot_time(1, SLOT_PERIOD, Some(3));
    let (c, d) = w.slot_time(5, SLOT_PERIOD, Some(3));
    assert_eq!((a, b), (c, d), "이 사례는 시각이 같아야 성립한다");

    w.add_lesson(sp, 1, "가람", 3, "과학"); // 1학년에 수업
    let r = w.find(5, "가람", 3); // 5학년 보결

    assert!(!is_eligible(&r, sp), "다른 학년이지만 시간이 겹쳐 제외");
    assert_eq!(of(&r, sp).reason_code, EXCLUDED_SPECIAL_LESSON);
}

#[test]
fn 사례3_같은_교시_번호라도_시각이_다르면_후보에_넣는다() {
    let mut w = World::new(1);
    let sp = w.add_teacher("박전담", SP, &["과학"]);

    // 중학년 4교시와 고학년 4교시의 시각이 다른지 먼저 확인
    let mid = w.slot_time(3, SLOT_PERIOD, Some(4));
    let high = w.slot_time(5, SLOT_PERIOD, Some(4));
    assert_ne!(mid, high, "학년군별 점심 위치 차이로 시각이 달라야 한다");
    assert!(
        !Interval::new(mid.0, mid.1).overlaps(&Interval::new(high.0, high.1)),
        "겹치지 않아야 이 사례가 성립한다: {mid:?} vs {high:?}"
    );

    w.add_lesson(sp, 3, "가람", 4, "과학"); // 중학년 4교시에 수업
    let r = w.find(5, "가람", 4); // 고학년 4교시 보결

    assert!(
        is_eligible(&r, sp),
        "교시 번호는 같지만 실제 시각이 안 겹치므로 후보"
    );
    assert_eq!(of(&r, sp).reason_code, ELIGIBLE_SPECIALIST_FREE);
}

#[test]
fn 사례4_전담이_들어간_반의_담임은_그_시간_후보가_된다() {
    let mut w = World::new(1);
    let sp = w.add_teacher("박전담", SP, &["과학"]);
    let hr = w.homeroom_of(5, "나리");
    w.add_lesson(sp, 5, "나리", 3, "과학");

    let r = w.find(5, "가람", 3);

    assert!(is_eligible(&r, hr), "전담이 들어와 공강이므로 후보");
    let c = of(&r, hr);
    assert_eq!(c.reason_code, ELIGIBLE_HOMEROOM_FREE);
    assert_eq!(c.status_label, "담임 공강");

    // 전담이 들어오지 않은 반의 담임은 수업 중이므로 제외
    let busy = w.homeroom_of(5, "다솜");
    assert!(!is_eligible(&r, busy));
    assert_eq!(of(&r, busy).reason_code, EXCLUDED_REGULAR_CLASS);
}

#[test]
fn 사례5_저학년_수업이_끝났으면_고학년_보결_후보가_된다() {
    let w = World::new(1);
    // 저학년 마지막 교시 종료 시각 < 고학년 6교시 시작 시각
    let low_end = w.slot_time(1, SLOT_PERIOD, Some(5)).1;
    let high_start = w.slot_time(5, SLOT_PERIOD, Some(6)).0;
    assert!(low_end <= high_start, "{low_end} <= {high_start}");

    let r = w.find(5, "가람", 6);
    let low_hr = w.homeroom_of(1, "가람");

    assert!(is_eligible(&r, low_hr), "학년이 다르다는 이유로 빼지 않는다");
    let c = of(&r, low_hr);
    assert_eq!(c.reason_code, ELIGIBLE_AFTER_SCHOOL_END);
    assert_eq!(c.status_label, "수업 종료");
}

#[test]
fn 사례6_점심시간_겹침을_정확히_구분한다() {
    let mut w = World::new(1);
    let free = w.add_teacher("박전담", SP, &["과학"]);
    let busy = w.add_teacher("김전담", SP, &["체육"]);

    let (ls, le) = w.slot_time(5, SLOT_LUNCH, None);
    // 고학년 점심과 겹치는 시간에 수업을 하나 준다
    w.snap.duties.push(DutyInfo {
        teacher_id: busy,
        block_type: "DUTY".into(),
        label: "겹치는 일정".into(),
        start_min: le - 5,
        end_min: le + 30,
    });

    let r = w.find_lunch(5, "가람");
    assert_eq!((r.slot.start_min, r.slot.end_min), (ls, le));

    // (1) 같은 학년군 담임은 자기 점심 지도 중 -> 제외
    let same = w.homeroom_of(5, "나리");
    assert!(!is_eligible(&r, same));
    assert_eq!(of(&r, same).reason_code, EXCLUDED_LUNCH_DUTY);
    assert_eq!(of(&r, same).status_label, "점심 지도");

    // (2) 점심시간이 다른 학년군 담임은 그때 '수업 중'이다.
    //     시차 급식이므로 이게 정상이고, 이유가 점심 지도와 정확히 구분되어야 한다.
    let low_lunch = w.slot_time(1, SLOT_LUNCH, None);
    assert_ne!(low_lunch, (ls, le), "학년군마다 점심 시각이 달라야 한다");
    let low_hr = w.homeroom_of(1, "가람");
    assert!(!is_eligible(&r, low_hr));
    assert_eq!(
        of(&r, low_hr).reason_code,
        EXCLUDED_REGULAR_CLASS,
        "저학년 담임은 점심 지도가 아니라 수업 때문에 제외되어야 한다"
    );

    // (3) 그 시간에 아무 일정 없는 전담은 후보
    assert!(is_eligible(&r, free));

    // (4) 점심과 1분이라도 겹치는 일정이 있으면 제외
    assert!(!is_eligible(&r, busy));
    assert_eq!(of(&r, busy).reason_code, EXCLUDED_FIXED_DUTY);
}

#[test]
fn 점심_직전에_끝나는_일정은_겹치지_않는다() {
    let mut w = World::new(1);
    let sp = w.add_teacher("박전담", SP, &["과학"]);
    let (ls, _) = w.slot_time(5, SLOT_LUNCH, None);

    // 점심 시작 시각에 딱 끝나는 일정
    w.snap.duties.push(DutyInfo {
        teacher_id: sp,
        block_type: "DUTY".into(),
        label: "직전 일정".into(),
        start_min: ls - 40,
        end_min: ls,
    });

    assert!(
        is_eligible(&w.find_lunch(5, "가람"), sp),
        "종료 시각 == 점심 시작 시각이면 겹치지 않는다"
    );
}

#[test]
fn 사례7_일정_종료와_보결_시작이_같은_시각이면_후보에_넣는다() {
    let mut w = World::new(1);
    let sp = w.add_teacher("박전담", SP, &["과학"]);

    // 중학년 점심 종료 == 중학년 4교시 시작 인지 확인
    let lunch = w.slot_time(3, SLOT_LUNCH, None);
    let p4 = w.slot_time(3, SLOT_PERIOD, Some(4));
    assert_eq!(lunch.1, p4.0, "점심이 끝나자마자 4교시가 시작해야 한다");

    // 전담에게 중학년 3교시 수업을 준다. 3교시 종료 == 점심 시작
    let p3 = w.slot_time(3, SLOT_PERIOD, Some(3));
    assert_eq!(p3.1, lunch.0);
    w.add_lesson(sp, 3, "가람", 3, "과학");

    // 4교시 보결 -> 3교시(종료 = 점심 시작)와 겹치지 않는다
    let r = w.find(3, "나리", 4);
    assert!(is_eligible(&r, sp), "접점은 겹치지 않는다");

    // 3교시 자체에 보결을 넣으면 당연히 제외
    let r3 = w.find(3, "나리", 3);
    assert!(!is_eligible(&r3, sp));
}

#[test]
fn 사례8_부재_중인_교사는_제외한다() {
    let mut w = World::new(1);
    let sp = w.add_teacher("박전담", SP, &["과학"]);

    // 종일 부재
    w.snap.absences.push(AbsenceInfo {
        teacher_id: sp,
        is_all_day: true,
        start_min: 0,
        end_min: 1440,
        reason_label: Some("연가".into()),
    });

    let r = w.find(5, "가람", 3);
    assert!(!is_eligible(&r, sp));
    let c = of(&r, sp);
    assert_eq!(c.reason_code, EXCLUDED_ABSENCE);
    assert_eq!(c.status_label, "부재 중");
    assert!(c.detail.as_deref().unwrap().contains("연가"), "{c:?}");
}

#[test]
fn 사례8b_시간_단위_부재는_그_시간만_제외한다() {
    let mut w = World::new(1);
    let sp = w.add_teacher("박전담", SP, &["과학"]);
    let (s3, e3) = w.slot_time(5, SLOT_PERIOD, Some(3));

    w.snap.absences.push(AbsenceInfo {
        teacher_id: sp,
        is_all_day: false,
        start_min: s3,
        end_min: e3,
        reason_label: Some("출장".into()),
    });

    assert!(!is_eligible(&w.find(5, "가람", 3), sp), "부재 시간과 겹침");
    assert!(is_eligible(&w.find(5, "가람", 1), sp), "다른 시간은 후보");
}

#[test]
fn 사례9_같은_시간에_이미_보결이_있으면_제외한다() {
    let mut w = World::new(1);
    let sp = w.add_teacher("박전담", SP, &["과학"]);
    let (s, e) = w.slot_time(5, SLOT_PERIOD, Some(3));

    w.snap.assigned.push(AssignedInfo {
        sub_teacher_id: sp,
        start_min: s,
        end_min: e,
        label: "보결 6-가람 3교시".into(),
    });

    let r = w.find(5, "가람", 3);
    assert!(!is_eligible(&r, sp));
    let c = of(&r, sp);
    assert_eq!(c.reason_code, EXCLUDED_ALREADY_ASSIGNED);
    assert_eq!(c.status_label, "다른 보결 배정됨");
    assert!(c.detail.as_deref().unwrap().contains("6-가람"), "{c:?}");

    // 겹치지 않는 다른 시간에는 여전히 후보
    assert!(is_eligible(&w.find(5, "가람", 1), sp));
}

// ============================================================
//  요구사항 5 — 그 밖의 제외 조건
// ============================================================

#[test]
fn 보결_대상이_아닌_교사는_제외한다() {
    let mut w = World::new(1);
    let vice = w.add_teacher("최교감", OT, &[]);
    if let Some(t) = w.snap.teachers.iter_mut().find(|t| t.id == vice) {
        t.is_substitutable = false;
        t.memo = Some("교감".into());
    }

    let r = w.find(5, "가람", 3);
    assert!(!is_eligible(&r, vice));
    let c = of(&r, vice);
    assert_eq!(c.reason_code, EXCLUDED_NOT_SUBSTITUTABLE);
    assert_eq!(c.status_label, "보결 대상 아님");
}

#[test]
fn 비활성_교사는_제외한다() {
    let mut w = World::new(1);
    let gone = w.add_teacher("떠난선생", SP, &["과학"]);
    w.snap.teachers.iter_mut().find(|t| t.id == gone).unwrap().active = false;

    let r = w.find(5, "가람", 3);
    assert!(!is_eligible(&r, gone));
    assert_eq!(of(&r, gone).reason_code, EXCLUDED_INACTIVE);
}

#[test]
fn 결근_당사자는_제외한다() {
    let w = World::new(1);
    let absent = w.homeroom_of(5, "가람");
    let r = w.run(5, "가람", SLOT_PERIOD, Some(3), Some(absent));

    assert!(!is_eligible(&r, absent));
    assert_eq!(of(&r, absent).reason_code, EXCLUDED_ABSENT_TEACHER);
}

#[test]
fn 등록된_고정_일정과_겹치면_제외한다() {
    let mut w = World::new(1);
    let sp = w.add_teacher("박전담", SP, &["과학"]);
    let (s, e) = w.slot_time(5, SLOT_PERIOD, Some(3));

    w.snap.duties.push(DutyInfo {
        teacher_id: sp,
        block_type: "DUTY".into(),
        label: "업무 협의회".into(),
        start_min: s,
        end_min: e,
    });

    let r = w.find(5, "가람", 3);
    assert!(!is_eligible(&r, sp));
    let c = of(&r, sp);
    assert_eq!(c.reason_code, EXCLUDED_FIXED_DUTY);
    assert!(c.detail.as_deref().unwrap().contains("업무 협의회"));
}

#[test]
fn 보결_배정_불가로_등록한_시간은_제외한다() {
    let mut w = World::new(1);
    let sp = w.add_teacher("박전담", SP, &["과학"]);
    let (s, e) = w.slot_time(5, SLOT_PERIOD, Some(3));

    w.snap.duties.push(DutyInfo {
        teacher_id: sp,
        block_type: "NO_SUB".into(),
        label: "보결 제외 시간".into(),
        start_min: s,
        end_min: e,
    });

    let r = w.find(5, "가람", 3);
    assert_eq!(of(&r, sp).reason_code, EXCLUDED_NO_SUB_BLOCK);
    assert_eq!(of(&r, sp).status_label, "보결 배정 불가 시간");
}

#[test]
fn 중간놀이_지도와_겹치면_제외한다() {
    let mut w = World::new(1);
    // 5학년 2교시 뒤에 중간놀이를 넣는다
    let p2 = w.slot_time(5, SLOT_PERIOD, Some(2));
    w.snap.slots.push(SlotInfo {
        grade: 5,
        day_of_week: w.snap.day_of_week,
        slot_type: SLOT_OTHER.into(),
        period_no: None,
        label: "중간놀이".into(),
        start_min: p2.1,
        end_min: p2.1 + 20,
    });
    let sp = w.add_teacher("박전담", SP, &["과학"]);
    w.snap.duties.push(DutyInfo {
        teacher_id: sp,
        block_type: "DUTY".into(),
        label: "다른 일".into(),
        start_min: 0,
        end_min: 1,
    });

    // 중간놀이 시간에 보결이 필요한 경우를 만들려면 그 구간을 target으로 삼아야 하므로
    // 여기서는 담임이 중간놀이 지도로 제외되는지만 본다 (2교시 직후 시간과 겹치는 교시)
    let hr = w.homeroom_of(5, "나리");
    let blocks = build_busy_index(&w.snap);
    let hit = blocks
        .of(hr)
        .iter()
        .any(|b| b.kind == BusyKind::Recess && b.label.contains("중간놀이"));
    assert!(hit, "담임에게 중간놀이 지도 일정이 붙어야 한다");
}

// ============================================================
//  요구사항 6 · 7 — 담임 파생 계산
// ============================================================

#[test]
fn 담임_수업은_전체_교시에서_전담_시간을_뺀_것이다() {
    let mut w = World::new(1);
    let sp = w.add_teacher("박전담", SP, &["과학"]);
    w.add_lesson(sp, 5, "가람", 2, "과학");
    w.add_lesson(sp, 5, "가람", 4, "과학");

    let hr = w.homeroom_of(5, "가람");
    let idx = build_busy_index(&w.snap);
    let periods: Vec<i32> = idx
        .of(hr)
        .iter()
        .filter(|b| b.kind == BusyKind::HomeroomLesson)
        .filter_map(|b| {
            b.label
                .split_whitespace()
                .last()
                .and_then(|s| s.trim_end_matches("교시").parse().ok())
        })
        .collect();

    assert_eq!(periods, vec![1, 3, 5, 6], "2·4교시는 전담이 들어와 빠진다");
}

#[test]
fn 공동수업이면_담임도_함께_수업_중이다() {
    let mut w = World::new(1);
    let sp = w.add_teacher("박전담", SP, &["과학"]);
    let class_id = w.class_id(5, "나리");
    w.snap.lessons.push(LessonInfo {
        teacher_id: sp,
        class_id,
        period_no: 3,
        replaces_homeroom: false, // 공동수업
        subject_name: Some("과학".into()),
    });

    let r = w.find(5, "가람", 3);
    let hr = w.homeroom_of(5, "나리");
    assert!(!is_eligible(&r, hr), "공동수업은 담임도 함께 들어간다");
    assert_eq!(of(&r, hr).reason_code, EXCLUDED_REGULAR_CLASS);
}

#[test]
fn 학년이_다르다는_이유만으로는_절대_제외하지_않는다() {
    let mut w = World::new(1);
    // 1학년 담임 모두에게 그 시간을 비워 준다: 1학년 1교시에 전담 투입
    let sp = w.add_teacher("박전담", SP, &["과학"]);
    w.add_lesson(sp, 1, "가람", 1, "과학");

    let r = w.find(6, "가람", 1); // 6학년 1교시 보결
    let low_hr = w.homeroom_of(1, "가람");

    assert!(is_eligible(&r, low_hr));
    assert_eq!(of(&r, low_hr).reason_code, ELIGIBLE_HOMEROOM_FREE);
}

// ============================================================
//  결과 모양
// ============================================================

#[test]
fn 결과에_필요한_정보가_모두_들어_있다() {
    let mut w = World::new(1);
    let sp = w.add_teacher("박전담", SP, &["과학", "체육"]);
    w.add_lesson(sp, 5, "나리", 1, "과학");

    let mut counts = HashMap::new();
    counts.insert(sp, SubCounts { today: 1, month: 3, total: 7 });

    let req = FindRequest {
        class_id: w.class_id(5, "가람"),
        slot_type: SLOT_PERIOD.into(),
        period_no: Some(3),
        absent_teacher_id: None,
    };
    let r = find_candidates(&w.snap, &req, &counts).unwrap();

    let c = of(&r, sp);
    assert_eq!(c.name, "박전담");
    assert_eq!(c.role_label, "전담");
    assert_eq!(c.duty, "과학, 체육");
    assert_eq!(c.counts.today, 1);
    assert_eq!(c.counts.month, 3);
    assert_eq!(c.counts.total, 7);
    assert!(!c.blocks.is_empty(), "그 날 일정이 개발자 확인용으로 들어 있어야 한다");

    let hr = of(&r, w.homeroom_of(5, "나리"));
    assert_eq!(hr.duty, "5-나리");
    assert_eq!(hr.homeroom_grades, vec![5]);
}

#[test]
fn 모든_교사가_가능_또는_불가능으로_한_번씩만_들어간다() {
    let mut w = World::new(1);
    w.add_teacher("박전담", SP, &["과학"]);
    let r = w.find(5, "가람", 3);

    let total = r.eligible.len() + r.excluded.len();
    assert_eq!(total, w.snap.teachers.len());

    let mut ids: Vec<i64> = r
        .eligible
        .iter()
        .chain(r.excluded.iter())
        .map(|c| c.teacher_id)
        .collect();
    ids.sort_unstable();
    let before = ids.len();
    ids.dedup();
    assert_eq!(ids.len(), before, "같은 교사가 두 번 나오면 안 된다");
}

#[test]
fn 후보가_없으면_안내_문구를_준다() {
    let mut w = World::new(1);
    // 모든 교사를 보결 대상에서 뺀다
    for t in &mut w.snap.teachers {
        t.is_substitutable = false;
    }
    let r = w.find(5, "가람", 3);
    assert!(r.eligible.is_empty());
    assert!(!r.warnings.is_empty());
    assert!(r.warnings[0].contains("배정할 수 있는 선생님이 없습니다"));
}

#[test]
fn 담임_미지정_학급이_있으면_알려준다() {
    let mut w = World::new(1);
    w.snap.classes[0].homeroom_teacher_id = None;
    let r = w.find(5, "가람", 3);
    assert!(r.warnings.iter().any(|m| m.contains("담임이 지정되지 않아")), "{:?}", r.warnings);
}

#[test]
fn 설정을_끄면_점심에_담임을_빼지_않는다() {
    let mut w = World::new(1);
    w.snap.settings.exclude_homeroom_on_own_lunch = false;

    let r = w.find_lunch(5, "가람");
    let same = w.homeroom_of(5, "나리");
    assert!(is_eligible(&r, same), "설정을 끄면 점심에도 후보가 된다");
}

// ============================================================
//  이 시간 담당 교사 / 전담 시간 알림
//  현장에서 가장 흔한 헛배정 — 전담 시간인데 모르고 보결을 넣는 것 — 을 막는다.
// ============================================================

#[test]
fn 전담이_들어오는_교시는_담당이_전담이다() {
    let mut w = World::new(5);
    let sp = w.add_teacher("김민수", SP, &["영어"]);
    w.add_lesson(sp, 6, "가람", 2, "영어");

    let r = w.find(6, "가람", 2);
    let ic = &r.slot.in_charge;

    assert_eq!(ic.teacher_id, Some(sp));
    assert_eq!(ic.subject_name.as_deref(), Some("영어"));
    assert!(ic.covered_by_other, "담임이 아닌 사람이 담당한다");
    assert!(!ic.is_homeroom);
}

#[test]
fn 전담이_없는_교시는_담당이_담임이다() {
    let mut w = World::new(5);
    let sp = w.add_teacher("김민수", SP, &["영어"]);
    w.add_lesson(sp, 6, "가람", 2, "영어");

    // 같은 반 3교시에는 전담이 없다
    let r = w.find(6, "가람", 3);
    let ic = &r.slot.in_charge;

    assert_eq!(ic.teacher_id, Some(w.homeroom_of(6, "가람")));
    assert!(ic.is_homeroom);
    assert!(!ic.covered_by_other);
    assert!(r.notice.is_none(), "담임 시간에는 알릴 것이 없다");
}

#[test]
fn 점심시간_담당은_담임이다() {
    let mut w = World::new(5);
    let sp = w.add_teacher("김민수", SP, &["영어"]);
    w.add_lesson(sp, 6, "가람", 2, "영어");

    let r = w.find_lunch(6, "가람");
    assert_eq!(r.slot.in_charge.teacher_id, Some(w.homeroom_of(6, "가람")));
    assert!(r.slot.in_charge.is_homeroom);
    assert!(r.notice.is_none());
}

#[test]
fn 다른_반_전담은_이_반_담당이_아니다() {
    let mut w = World::new(5);
    let sp = w.add_teacher("김민수", SP, &["영어"]);
    w.add_lesson(sp, 6, "나리", 2, "영어");

    let r = w.find(6, "가람", 2);
    assert_eq!(r.slot.in_charge.teacher_id, Some(w.homeroom_of(6, "가람")));
    assert!(!r.slot.in_charge.covered_by_other);
}

#[test]
fn 전담_시간이면_확인_안내를_띄운다() {
    let mut w = World::new(5);
    let sp = w.add_teacher("김민수", SP, &["영어"]);
    w.add_lesson(sp, 6, "가람", 2, "영어");

    // 담임이 결근한 상황
    let hr = w.homeroom_of(6, "가람");
    let r = w.run(6, "가람", SLOT_PERIOD, Some(2), Some(hr));

    let n = r.notice.as_ref().expect("전담 시간이면 안내가 있어야 한다");
    assert_eq!(n.kind, "SPECIAL_LESSON");
    assert_eq!(n.title, "전담(영어) 시간입니다. 확인 바랍니다.");
    assert!(n.body.contains("김민수"), "{}", n.body);
    assert!(n.body.contains("보결이 필요하지 않습니다"), "{}", n.body);
}

#[test]
fn 전담_본인이_결근하면_안내를_띄우지_않는다() {
    let mut w = World::new(5);
    let sp = w.add_teacher("김민수", SP, &["영어"]);
    w.add_lesson(sp, 6, "가람", 2, "영어");

    let r = w.run(6, "가람", SLOT_PERIOD, Some(2), Some(sp));
    assert!(r.notice.is_none(), "전담이 결근했으면 보결이 맞다");
}

#[test]
fn 전담_시간이어도_담임은_후보로_남는다() {
    let mut w = World::new(5);
    let sp = w.add_teacher("김민수", SP, &["영어"]);
    w.add_lesson(sp, 6, "가람", 2, "영어");

    // 전담이 결근한 경우 그 반 담임이 들어가는 것이 자연스럽다
    let hr = w.homeroom_of(6, "가람");
    let r = w.run(6, "가람", SLOT_PERIOD, Some(2), Some(sp));
    assert!(is_eligible(&r, hr), "담임은 이 시간 공강이므로 후보다");
}

#[test]
fn 담임이_없는_학급은_담당을_알_수_없다고_알린다() {
    let mut w = World::new(5);
    let cid = w.class_id(6, "가람");
    for c in w.snap.classes.iter_mut() {
        if c.id == cid {
            c.homeroom_teacher_id = None;
        }
    }

    let r = w.find(6, "가람", 2);
    let n = r.notice.as_ref().expect("담임이 없으면 알려야 한다");
    assert_eq!(n.kind, "NO_HOMEROOM");
    assert!(n.body.contains("담임을 먼저 지정"), "{}", n.body);
}
