//! Phase 8 — 배정 재검증과 하루 일괄 대상 찾기 (DB 없이).
//!
//! `find_tests.rs`와 같은 모양의 학교를 쓴다.
//!   저학년(1~2) 5교시 · 4교시 뒤 점심 / 고학년(5~6) 6교시 · 5교시 뒤 점심

use std::collections::HashMap;

use super::assign::*;
use super::find::{FindError, SubCounts};
use super::priority::{RuleSetting, DEFAULT_ORDER};
use super::schedule::*;
use super::time::Interval;

const HR: &str = "HOMEROOM";
const SP: &str = "SPECIAL";
const OT: &str = "OTHER";

fn hm(h: i32, m: i32) -> i32 {
    h * 60 + m
}

fn default_settings() -> Vec<RuleSetting> {
    DEFAULT_ORDER
        .iter()
        .enumerate()
        .map(|(i, (k, on))| RuleSetting {
            rule_key: k.to_string(),
            enabled: *on,
            sort_order: i as i32 + 1,
        })
        .collect()
}

fn slots_for(grade: i32, day: i32) -> Vec<SlotInfo> {
    let (count, lunch_after) = match grade {
        1 | 2 => (5, 4),
        3 | 4 => (5, 3),
        _ => (6, 5),
    };
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
            end_min: t + 40,
        });
        t += 40;
        if p == lunch_after {
            out.push(SlotInfo {
                grade,
                day_of_week: day,
                slot_type: SLOT_LUNCH.into(),
                period_no: None,
                label: "점심".into(),
                start_min: t,
                end_min: t + 50,
            });
            t += 50;
        } else if p < count {
            t += 5;
        }
    }
    out
}

const CLASS_NAMES: [&str; 2] = ["가람", "나리"];

struct World {
    snap: DaySnapshot,
    next_teacher: i64,
}

impl World {
    fn new(day: i32) -> Self {
        let mut classes = Vec::new();
        let mut teachers = Vec::new();
        let mut slots = Vec::new();
        let (mut cid, mut tid) = (1i64, 1i64);

        for grade in [1, 2, 5, 6] {
            slots.extend(slots_for(grade, day));
            for (i, name) in CLASS_NAMES.iter().enumerate() {
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
                    class_no: i as i32 + 1,
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
                date: "2026-09-07".into(),
                day_of_week: day,
                teachers,
                classes,
                slots,
                lessons: vec![],
                duties: vec![],
                absences: vec![],
                assigned: vec![],
                settings: EngineSettings::default(),
                meal_default: None,
                meal_overrides: HashMap::new(),
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

    fn verify(&self, grade: i32, name: &str, period: i32, absent: i64, sub: i64) -> Result<AssignRow, AssignError> {
        verify(
            &self.snap,
            &HashMap::<i64, SubCounts>::new(),
            &default_settings(),
            Some(absent),
            &AssignPlan {
                class_id: self.class_id(grade, name),
                slot_type: SLOT_PERIOD.into(),
                period_no: Some(period),
                sub_teacher_id: sub,
            },
        )
    }
}

// ============================================================
//  결근 교사가 그 날 맡은 시간
// ============================================================

#[test]
fn 담임의_하루는_자기_반_전체_교시와_점심이다() {
    let w = World::new(1);
    let hr = w.homeroom_of(5, "가람");
    let d = duty_slots(&w.snap, hr, None);

    // 고학년 6교시 + 점심 1
    assert_eq!(d.len(), 7);
    assert_eq!(
        d.iter().filter(|x| x.kind == DUTY_HOMEROOM).count(),
        6,
        "정규 수업 6교시"
    );
    assert_eq!(d.iter().filter(|x| x.kind == DUTY_LUNCH).count(), 1);
    assert!(d.iter().all(|x| x.class_label == "5-가람"));
}

#[test]
fn 전담이_들어오는_교시는_담임의_보결_대상이_아니다() {
    let mut w = World::new(1);
    let sp = w.add_teacher("김민수", SP, &["영어"]);
    w.add_lesson(sp, 5, "가람", 3, "영어");

    let hr = w.homeroom_of(5, "가람");
    let d = duty_slots(&w.snap, hr, None);

    assert!(
        !d.iter().any(|x| x.period_no == Some(3)),
        "3교시는 전담이 수업하므로 보결이 필요 없다"
    );
    assert_eq!(d.iter().filter(|x| x.kind == DUTY_HOMEROOM).count(), 5);
}

#[test]
fn 중간놀이는_보결_대상이_아니다() {
    let mut w = World::new(1);
    // 2교시 뒤에 중간놀이를 넣는다
    let after2 = w
        .snap
        .slots
        .iter()
        .find(|s| s.grade == 5 && s.slot_type == SLOT_PERIOD && s.period_no == Some(2))
        .map(|s| s.end_min)
        .unwrap();
    w.snap.slots.push(SlotInfo {
        grade: 5,
        day_of_week: 1,
        slot_type: SLOT_OTHER.into(),
        period_no: None,
        label: "중간놀이".into(),
        start_min: after2,
        end_min: after2 + 20,
    });

    let hr = w.homeroom_of(5, "가람");
    let d = duty_slots(&w.snap, hr, None);
    assert!(
        d.iter().all(|x| x.slot_label != "중간놀이"),
        "중간놀이에는 보결을 배정하지 않는다: {:?}",
        d.iter().map(|x| x.slot_label.clone()).collect::<Vec<_>>()
    );

    // 다만 그 시간에 다른 반 보결로 부르지는 않는다 (여전히 바쁜 시간이다)
    let idx = build_busy_index(&w.snap);
    assert!(
        idx.conflicts(hr, &Interval::new(after2, after2 + 20))
            .iter()
            .any(|b| b.label.contains("중간놀이")),
        "중간놀이는 담임의 바쁜 시간으로 남아야 한다"
    );
}

#[test]
fn 전담교사의_하루는_자기_수업뿐이다() {
    let mut w = World::new(1);
    let sp = w.add_teacher("김민수", SP, &["영어"]);
    w.add_lesson(sp, 5, "가람", 3, "영어");
    w.add_lesson(sp, 6, "나리", 4, "영어");

    let d = duty_slots(&w.snap, sp, None);
    assert_eq!(d.len(), 2);
    assert!(d.iter().all(|x| x.kind == DUTY_LESSON));
    assert_eq!(d[0].subject_name.as_deref(), Some("영어"));
    assert!(d[0].start_min <= d[1].start_min, "시간 순서");
}

#[test]
fn 일부_시간_결근이면_그_구간에_걸치는_시간만_남는다() {
    let w = World::new(1);
    let hr = w.homeroom_of(5, "가람");

    let all = duty_slots(&w.snap, hr, None);
    let pm = duty_slots(&w.snap, hr, Some(Interval::new(hm(12, 0), hm(16, 0))));

    assert!(pm.len() < all.len());
    assert!(pm.iter().all(|x| x.end_min > hm(12, 0)));
}

#[test]
fn 구간에_걸치기만_해도_보결_대상이다() {
    let w = World::new(1);
    let hr = w.homeroom_of(5, "가람");

    // 1교시(09:00~09:40) 도중인 09:30부터 조퇴
    let d = duty_slots(&w.snap, hr, Some(Interval::new(hm(9, 30), hm(16, 0))));
    assert!(
        d.iter().any(|x| x.period_no == Some(1)),
        "도중에 나가면 그 교시도 보결이 필요하다"
    );
}

#[test]
fn 담임이_아니고_수업도_없으면_보결_대상이_없다() {
    let mut w = World::new(1);
    let etc = w.add_teacher("최교감", OT, &[]);
    assert!(duty_slots(&w.snap, etc, None).is_empty());
}

// ============================================================
//  저장 직전 재검증
// ============================================================

#[test]
fn 후보였던_교사는_그대로_확정된다() {
    let mut w = World::new(1);
    let sp = w.add_teacher("김민수", SP, &["영어"]);
    let hr = w.homeroom_of(5, "가람");

    let row = w.verify(5, "가람", 1, hr, sp).expect("배정할 수 있어야 한다");
    assert_eq!(row.sub_teacher_id, sp);
    assert_eq!(row.sub_teacher_name, "김민수");
    assert_eq!(row.class_label, "5-가람");
    assert_eq!(row.slot_label, "1교시");
    assert_eq!((row.start_min, row.end_min), (hm(9, 0), hm(9, 40)));
    assert_eq!(row.absent_teacher_name.as_deref(), Some("5-가람 담임"));
    assert!(row.recommend_rank.is_some());
}

#[test]
fn 수업_중인_교사는_확정되지_않는다() {
    let w = World::new(1);
    let hr = w.homeroom_of(5, "가람");
    let busy = w.homeroom_of(6, "가람"); // 자기 반 1교시 수업 중

    let e = w.verify(5, "가람", 1, hr, busy).unwrap_err();
    match &e {
        AssignError::NotEligible { name, status, .. } => {
            assert_eq!(name, "6-가람 담임");
            assert_eq!(status, "수업 중");
        }
        other => panic!("수업 중으로 막혀야 한다: {other:?}"),
    }
    assert!(error_message(&e).contains("상황이 바뀌었습니다"));
}

#[test]
fn 없는_교시는_시정표_문제로_알려준다() {
    let mut w = World::new(1);
    let sp = w.add_teacher("김민수", SP, &["영어"]);
    let hr = w.homeroom_of(1, "가람");

    // 저학년은 5교시까지다
    let e = w.verify(1, "가람", 6, hr, sp).unwrap_err();
    assert!(matches!(e, AssignError::Slot(FindError::SlotNotFound { .. })));
    assert!(error_message(&e).contains("시정표"), "{}", error_message(&e));
}

#[test]
fn 모르는_교사는_거부한다() {
    let w = World::new(1);
    let hr = w.homeroom_of(5, "가람");
    let e = w.verify(5, "가람", 1, hr, 9999).unwrap_err();
    assert!(matches!(e, AssignError::UnknownTeacher));
}

#[test]
fn 결근_교사_본인은_거부한다() {
    let w = World::new(1);
    let hr = w.homeroom_of(5, "가람");
    let e = w.verify(5, "가람", 1, hr, hr).unwrap_err();
    assert!(matches!(e, AssignError::SameAsAbsent { .. }));
}

#[test]
fn 전담_시간에_배정하면_확인_안내가_함께_나온다() {
    let mut w = World::new(1);
    let sp = w.add_teacher("김민수", SP, &["영어"]);
    w.add_lesson(sp, 5, "가람", 3, "영어");
    let etc = w.add_teacher("최교감", OT, &[]);
    let hr = w.homeroom_of(5, "가람");

    let row = w.verify(5, "가람", 3, hr, etc).unwrap();
    let n = row.notice.as_ref().expect("배정 직전에도 알려야 한다");
    assert!(n.title.contains("확인 바랍니다"), "{}", n.title);
    assert_eq!(row.subject_name.as_deref(), Some("영어"), "과목도 기록에 남는다");
}

// ============================================================
//  일괄 배정의 시간 겹침 (occupy)
// ============================================================

#[test]
fn 앞에서_배정한_시간이_뒤_검증에_반영된다() {
    let mut w = World::new(1);
    let sp = w.add_teacher("김민수", SP, &["영어"]);
    let hr5 = w.homeroom_of(5, "가람");
    let hr6 = w.homeroom_of(6, "가람");

    // 5-가람 1교시에 확정
    let first = w.verify(5, "가람", 1, hr5, sp).unwrap();
    occupy(&mut w.snap, &first);

    // 같은 시각 6-가람 1교시에 같은 사람 → 막힌다
    let e = w.verify(6, "가람", 1, hr6, sp).unwrap_err();
    match e {
        AssignError::NotEligible { status, .. } => {
            assert_eq!(status, "다른 보결 배정됨");
        }
        other => panic!("이미 배정됨으로 막혀야 한다: {other:?}"),
    }
}

#[test]
fn 겹치지_않는_다음_교시는_계속_배정할_수_있다() {
    let mut w = World::new(1);
    let sp = w.add_teacher("김민수", SP, &["영어"]);
    let hr = w.homeroom_of(5, "가람");

    let a = w.verify(5, "가람", 1, hr, sp).unwrap();
    occupy(&mut w.snap, &a);
    let b = w.verify(5, "가람", 2, hr, sp).unwrap();

    assert!(a.end_min <= b.start_min);
    assert_eq!(b.sub_teacher_id, sp, "연속 교시는 같은 사람도 된다");
}

#[test]
fn 학년군이_다르면_같은_교시라도_시각이_다르다() {
    let mut w = World::new(1);
    let sp = w.add_teacher("김민수", SP, &["영어"]);
    let hr1 = w.homeroom_of(1, "가람");
    let hr5 = w.homeroom_of(5, "가람");

    // 저학년은 4교시 뒤 점심, 고학년은 5교시 뒤 점심 → 5교시 시각이 다르다
    let a = w.verify(1, "가람", 5, hr1, sp).unwrap();
    occupy(&mut w.snap, &a);
    let b = w.verify(5, "가람", 5, hr5, sp).unwrap();

    assert_ne!((a.start_min, a.end_min), (b.start_min, b.end_min));
    assert!(!a.interval().overlaps(&b.interval()), "겹치지 않으므로 둘 다 된다");
}
