//! 실제 사용 중인 DB로 하는 검증. (요구사항 12)
//!
//! 원본을 건드리지 않도록 **DB 파일을 임시로 복사해서** 시험한다.
//! 자료가 있어야만 뜻이 있으므로 `#[ignore]`를 붙여 평소에는 돌리지 않는다.
//!
//!     cargo test --lib real_db -- --ignored --nocapture
//!
//! 사용자 PC에 자료가 없으면 그냥 넘어간다.

use std::collections::HashMap;

use rusqlite::Connection;

use crate::repo::find as repo_find;
use crate::domain::find::*;
use crate::domain::priority::rank_candidates;
use crate::repo::priority as repo_priority;
use crate::domain::schedule::{DaySnapshot, SLOT_LUNCH, SLOT_PERIOD};
use crate::domain::time::{fmt_range, Interval};

fn live_db_path() -> Option<std::path::PathBuf> {
    let appdata = std::env::var("APPDATA").ok()?;
    let p = std::path::Path::new(&appdata)
        .join("kr.school.bogyeol")
        .join("bogyeol.db");
    p.exists().then_some(p)
}

/// 원본을 복사해서 열고, 원하는 자료를 더 넣어 시험한다.
fn open_copy() -> Option<Connection> {
    let src = live_db_path()?;
    let dst = std::env::temp_dir().join(format!(
        "bogyeol-test-{}.db",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    // WAL에 남은 내용까지 가져오기 위해 백업 API를 쓴다
    let live = Connection::open_with_flags(
        &src,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI,
    )
    .ok()?;
    let mut copy = Connection::open(&dst).ok()?;
    {
        let backup = rusqlite::backup::Backup::new(&live, &mut copy).ok()?;
        backup.run_to_completion(200, std::time::Duration::ZERO, None).ok()?;
    }
    copy.pragma_update(None, "foreign_keys", "ON").ok()?;
    Some(copy)
}

/// 그 날짜의 요일에 맞는 날짜를 찾는다. (기준 날짜부터 앞으로 훑는다)
fn date_for_weekday(target: i32) -> String {
    use chrono::{Datelike, Duration, NaiveDate};
    let mut d = NaiveDate::from_ymd_opt(2026, 9, 7).unwrap();
    for _ in 0..7 {
        if d.weekday().number_from_monday() as i32 == target {
            return d.format("%Y-%m-%d").to_string();
        }
        d += Duration::days(1);
    }
    "2026-09-07".to_string()
}

struct Ctx {
    conn: Connection,
    date: String,
    snap: DaySnapshot,
    counts: HashMap<i64, SubCounts>,
}

impl Ctx {
    fn load(conn: Connection, date: &str) -> Self {
        let snap = repo_find::snapshot(&conn, date).expect("하루치 자료를 읽어야 한다");
        let counts = repo_find::counts(&conn, date).expect("횟수를 세야 한다");
        Self {
            conn,
            date: date.to_string(),
            snap,
            counts,
        }
    }

    fn reload(&mut self) {
        self.snap = repo_find::snapshot(&self.conn, &self.date).unwrap();
        self.counts = repo_find::counts(&self.conn, &self.date).unwrap();
    }

    fn class_of(&self, grade: i32, nth: usize) -> Option<i64> {
        self.snap
            .classes
            .iter()
            .filter(|c| c.grade == grade)
            .nth(nth)
            .map(|c| c.id)
    }

    fn label(&self, class_id: i64) -> String {
        self.snap
            .classes
            .iter()
            .find(|c| c.id == class_id)
            .map(|c| c.label.clone())
            .unwrap_or_default()
    }

    fn name_of(&self, teacher_id: i64) -> String {
        self.snap
            .teachers
            .iter()
            .find(|t| t.id == teacher_id)
            .map(|t| t.name.clone())
            .unwrap_or_default()
    }

    fn homeroom_of(&self, class_id: i64) -> Option<i64> {
        self.snap
            .classes
            .iter()
            .find(|c| c.id == class_id)?
            .homeroom_teacher_id
    }

    fn slot(&self, grade: i32, slot_type: &str, period: Option<i32>) -> Option<(i32, i32)> {
        crate::domain::schedule::find_slot(
            &self.snap.slots,
            grade,
            self.snap.day_of_week,
            slot_type,
            period,
        )
        .map(|s| (s.start_min, s.end_min))
    }

    /// 그 학급 학년의 시정표에 그 교시가 있는가
    fn slot_of(&self, class_id: i64, period: i32) -> Option<(i32, i32)> {
        let grade = self.snap.classes.iter().find(|c| c.id == class_id)?.grade;
        self.slot(grade, SLOT_PERIOD, Some(period))
    }

    fn find(&self, class_id: i64, slot_type: &str, period: Option<i32>) -> FindResult {
        self.find_absent(class_id, slot_type, period, None)
    }

    fn find_absent(
        &self,
        class_id: i64,
        slot_type: &str,
        period: Option<i32>,
        absent: Option<i64>,
    ) -> FindResult {
        let req = FindRequest {
            class_id,
            slot_type: slot_type.into(),
            period_no: period,
            absent_teacher_id: absent,
        };
        find_candidates(&self.snap, &req, &self.counts).expect("조회가 되어야 한다")
    }

    /// 저장된 추천 기준으로 순위까지 매긴 결과
    fn find_ranked(&self, class_id: i64, slot_type: &str, period: Option<i32>) -> FindResult {
        let mut r = self.find(class_id, slot_type, period);
        let settings = repo_priority::settings(&self.conn).unwrap();
        rank_candidates(&mut r.eligible, &settings, r.slot.grade);
        r
    }

    fn set_priority(&self, keys: &[&str]) {
        let all = [
            "SAME_GRADE",
            "FEWEST_TODAY",
            "FEWEST_MONTH",
            "FEWEST_TOTAL",
            "PREFER_SPECIAL",
            "PREFER_FINISHED",
            "PREFER_LOWER_GRADE",
        ];
        let mut rules: Vec<repo_priority::RuleInput> = keys
            .iter()
            .map(|k| repo_priority::RuleInput {
                rule_key: k.to_string(),
                enabled: true,
            })
            .collect();
        for k in all {
            if !keys.contains(&k) {
                rules.push(repo_priority::RuleInput {
                    rule_key: k.to_string(),
                    enabled: false,
                });
            }
        }
        repo_priority::save(&self.conn, &rules).unwrap();
    }

    fn verdict(&self, r: &FindResult, teacher_id: i64) -> (bool, String) {
        r.eligible
            .iter()
            .chain(r.excluded.iter())
            .find(|c| c.teacher_id == teacher_id)
            .map(|c| (c.eligible, c.reason_code.clone()))
            .unwrap_or((false, "NOT_FOUND".into()))
    }
}

fn pass(name: &str, detail: &str) {
    println!("  ok   {name}\n         {detail}");
}

#[test]
#[ignore = "실제 사용 중인 DB가 필요하다"]
fn real_db_시나리오_전체() {
    let Some(conn) = open_copy() else {
        println!("실제 DB가 없어 넘어간다");
        return;
    };

    // 전담 시간표가 들어 있는 요일을 고른다
    let mut ctx = Ctx::load(conn, &date_for_weekday(1)); // 월요일
    println!(
        "\n=== 실제 DB · {} (요일 {}) ===\n교사 {}명 · 학급 {}개 · 시정표 {}칸 · 수업 {}시간",
        ctx.date,
        ctx.snap.day_of_week,
        ctx.snap.teachers.len(),
        ctx.snap.classes.len(),
        ctx.snap.slots.len(),
        ctx.snap.lessons.len()
    );
    assert!(!ctx.snap.classes.is_empty(), "학급 자료가 있어야 한다");
    assert!(!ctx.snap.slots.is_empty(), "시정표가 있어야 한다");
    assert!(
        !ctx.snap.lessons.is_empty(),
        "전담 시간표가 있어야 이 검증이 뜻이 있다"
    );

    // ---------- 1. 같은 시간에 전담 수업 중인 교사 제외 ----------
    {
        let l = ctx.snap.lessons[0].clone();
        let cls = ctx
            .snap
            .classes
            .iter()
            .find(|c| c.id == l.class_id)
            .unwrap()
            .clone();
        // 같은 학년의 다른 반을 보결 대상으로 잡으면 시각이 같다
        let other = ctx
            .snap
            .classes
            .iter()
            .find(|c| c.grade == cls.grade && c.id != cls.id)
            .expect("같은 학년에 다른 반이 있어야 한다")
            .id;

        let r = ctx.find(other, SLOT_PERIOD, Some(l.period_no));
        let (ok, code) = ctx.verdict(&r, l.teacher_id);
        assert!(!ok, "{} 선생님은 수업 중이므로 제외", ctx.name_of(l.teacher_id));
        assert_eq!(code, EXCLUDED_SPECIAL_LESSON);
        pass(
            "같은 시간에 전담 수업 중인 교사 제외",
            &format!(
                "{} → {} {}교시 보결에서 제외 ({})",
                ctx.name_of(l.teacher_id),
                ctx.label(other),
                l.period_no,
                code
            ),
        );
    }

    // ---------- 2. 같은 교시 번호라도 시각이 다르면 후보 ----------
    {
        // 찾는 사례: 어떤 교사 T가 P교시에 수업이 있는데,
        //           보결 대상 학급의 P교시 시각과는 겹치지 않고,
        //           T의 그날 다른 일정과도 겹치지 않는 경우.
        // (다른 일정까지 확인해야 한다 — 학년군이 달라 P교시는 안 겹치지만
        //  T의 P+1교시가 겹칠 수 있다)
        let idx = crate::domain::schedule::build_busy_index(&ctx.snap);
        let mut found = None;

        'search: for cls in &ctx.snap.classes {
            for p in 1..=15 {
                let Some(target) = ctx.slot(cls.grade, SLOT_PERIOD, Some(p)) else {
                    continue;
                };
                let ti = Interval::new(target.0, target.1);

                for l in &ctx.snap.lessons {
                    if l.period_no != p || l.class_id == cls.id {
                        continue;
                    }
                    let lg = ctx.snap.classes.iter().find(|c| c.id == l.class_id).unwrap().grade;
                    let Some(own) = ctx.slot(lg, SLOT_PERIOD, Some(p)) else {
                        continue;
                    };
                    // 같은 교시 번호인데 시각이 다르고, 그 교사가 그 시간에 아무 일정도 없어야 한다
                    if Interval::new(own.0, own.1).overlaps(&ti) {
                        continue;
                    }
                    if !idx.conflicts(l.teacher_id, &ti).is_empty() {
                        continue;
                    }
                    found = Some((l.clone(), own, cls.id, p, target));
                    break 'search;
                }
            }
        }

        if let Some((l, own, target_class, p, target)) = found {
            let r = ctx.find(target_class, SLOT_PERIOD, Some(p));
            let (ok, code) = ctx.verdict(&r, l.teacher_id);
            assert!(
                ok,
                "{} 선생님은 {}교시지만 시각이 달라 후보여야 한다 ({} vs {})",
                ctx.name_of(l.teacher_id),
                p,
                fmt_range(&Interval::new(own.0, own.1)),
                fmt_range(&Interval::new(target.0, target.1))
            );
            pass(
                "같은 교시 번호라도 시각이 다르면 후보 포함",
                &format!(
                    "{} 선생님: {} {}교시 {} / 보결 {} {}교시 {} → 후보 ({})",
                    ctx.name_of(l.teacher_id),
                    ctx.label(l.class_id),
                    p,
                    fmt_range(&Interval::new(own.0, own.1)),
                    ctx.label(target_class),
                    p,
                    fmt_range(&Interval::new(target.0, target.1)),
                    code
                ),
            );
        } else {
            println!("  --   같은 교시 번호로 시각이 다른 사례가 이 요일에 없어 넘어감");
        }
    }

    // ---------- 2b. 다른 학년이지만 실제 시각이 겹치면 제외 ----------
    {
        // 학년이 다른데(= 학년군도 다를 수 있다) 시각이 겹쳐 제외되는 사례
        let idx = crate::domain::schedule::build_busy_index(&ctx.snap);
        let mut found = None;

        'search2: for cls in &ctx.snap.classes {
            for p in 1..=15 {
                let Some(target) = ctx.slot(cls.grade, SLOT_PERIOD, Some(p)) else {
                    continue;
                };
                let ti = Interval::new(target.0, target.1);

                for l in &ctx.snap.lessons {
                    let lg = ctx.snap.classes.iter().find(|c| c.id == l.class_id).unwrap().grade;
                    if lg == cls.grade {
                        continue; // 다른 학년이어야 한다
                    }
                    let Some(own) = ctx.slot(lg, SLOT_PERIOD, Some(l.period_no)) else {
                        continue;
                    };
                    if !Interval::new(own.0, own.1).overlaps(&ti) {
                        continue;
                    }
                    if idx.conflicts(l.teacher_id, &ti).is_empty() {
                        continue;
                    }
                    found = Some((l.clone(), own, cls.id, p, target));
                    break 'search2;
                }
            }
        }

        if let Some((l, own, target_class, p, target)) = found {
            let r = ctx.find(target_class, SLOT_PERIOD, Some(p));
            let (ok, code) = ctx.verdict(&r, l.teacher_id);
            assert!(!ok, "시각이 겹치므로 제외되어야 한다");
            pass(
                "다른 학년이지만 실제 시각이 겹치는 교사 제외",
                &format!(
                    "{} 선생님: {} {}교시 {} / 보결 {} {}교시 {} → 제외 ({})",
                    ctx.name_of(l.teacher_id),
                    ctx.label(l.class_id),
                    l.period_no,
                    fmt_range(&Interval::new(own.0, own.1)),
                    ctx.label(target_class),
                    p,
                    fmt_range(&Interval::new(target.0, target.1)),
                    code
                ),
            );
        } else {
            println!("  --   다른 학년끼리 시각이 겹치는 사례가 이 요일에 없어 넘어감");
        }
    }

    // ---------- 3. 전담이 들어간 반의 담임은 후보 ----------
    {
        let mut done = false;
        for l in &ctx.snap.lessons {
            if !l.replaces_homeroom {
                continue;
            }
            let Some(hr) = ctx.homeroom_of(l.class_id) else {
                continue;
            };
            let cls = ctx.snap.classes.iter().find(|c| c.id == l.class_id).unwrap();
            let Some(other) = ctx
                .snap
                .classes
                .iter()
                .find(|c| c.grade == cls.grade && c.id != cls.id)
                .map(|c| c.id)
            else {
                continue;
            };

            let r = ctx.find(other, SLOT_PERIOD, Some(l.period_no));
            let (ok, code) = ctx.verdict(&r, hr);
            // 실제 DB에는 이미 배정된 보결이 있을 수 있다. 그것은 정상적인 제외
            // 사유이므로 그런 칸은 건너뛰고 깨끗한 칸을 찾는다.
            if !ok && code == EXCLUDED_ALREADY_ASSIGNED {
                continue;
            }
            assert!(
                ok,
                "{} 선생님({})은 전담이 들어와 공강이므로 후보여야 한다 ({code})",
                ctx.name_of(hr),
                ctx.label(l.class_id)
            );
            assert_eq!(code, ELIGIBLE_HOMEROOM_FREE);
            pass(
                "전담이 들어간 반의 담임은 후보 포함",
                &format!(
                    "{} 선생님({}) → {} {}교시 보결 후보 ({})",
                    ctx.name_of(hr),
                    ctx.label(l.class_id),
                    ctx.label(other),
                    l.period_no,
                    code
                ),
            );
            done = true;
            break;
        }
        assert!(done, "전담 수업이 들어간 반이 하나는 있어야 한다");
    }

    // ---------- 4. 저학년 수업 종료 후 고학년 보결 후보 ----------
    {
        let low = ctx.snap.classes.iter().map(|c| c.grade).min().unwrap();
        let high = ctx.snap.classes.iter().map(|c| c.grade).max().unwrap();
        // 고학년의 마지막 교시
        let last = (1..=15)
            .rev()
            .find(|p| ctx.slot(high, SLOT_PERIOD, Some(*p)).is_some())
            .expect("고학년 교시가 있어야 한다");
        let target = ctx.slot(high, SLOT_PERIOD, Some(last)).unwrap();

        // 저학년의 마지막 일정 종료 시각
        let low_last = ctx
            .snap
            .slots
            .iter()
            .filter(|s| s.grade == low)
            .map(|s| s.end_min)
            .max()
            .unwrap();

        if low_last <= target.0 {
            let cid = ctx.class_of(high, 0).unwrap();
            let low_cid = ctx.class_of(low, 0).unwrap();
            let hr = ctx.homeroom_of(low_cid).expect("저학년 담임이 있어야 한다");
            let r = ctx.find(cid, SLOT_PERIOD, Some(last));
            let (ok, code) = ctx.verdict(&r, hr);
            assert!(ok, "{} 선생님은 이미 하교했으므로 후보", ctx.name_of(hr));
            assert_eq!(code, ELIGIBLE_AFTER_SCHOOL_END);
            pass(
                "저학년 수업 종료 후 고학년 보결 후보 포함",
                &format!(
                    "{}학년 마지막 일정 {} / {}학년 {}교시 {} → {} 선생님 후보 ({})",
                    low,
                    crate::domain::time::fmt_min(low_last),
                    high,
                    last,
                    fmt_range(&Interval::new(target.0, target.1)),
                    ctx.name_of(hr),
                    code
                ),
            );
        } else {
            println!("  --   저학년이 고학년 마지막 교시보다 늦게 끝나 이 사례는 넘어감");
        }
    }

    // ---------- 5. 점심시간 겹침 ----------
    {
        let grade = ctx.snap.classes[0].grade;
        if ctx.slot(grade, SLOT_LUNCH, None).is_some() {
            let a = ctx.class_of(grade, 0).unwrap();
            let b = ctx.class_of(grade, 1);
            let r = ctx.find(a, SLOT_LUNCH, None);
            assert_eq!(r.slot.slot_type, "LUNCH");

            if let Some(b) = b {
                let hr = ctx.homeroom_of(b).unwrap();
                let (ok, code) = ctx.verdict(&r, hr);
                assert!(!ok, "같은 학년 담임은 점심 지도 중");
                assert_eq!(code, EXCLUDED_LUNCH_DUTY);
                pass(
                    "점심시간 겹침 정확히 제외",
                    &format!(
                        "{} 점심 {} → {} 선생님({}) 제외 ({})",
                        ctx.label(a),
                        fmt_range(&Interval::new(r.slot.start_min, r.slot.end_min)),
                        ctx.name_of(hr),
                        ctx.label(b),
                        code
                    ),
                );
            }
        }
    }

    // ---------- 6. 접점(종료 == 시작)은 후보 ----------
    {
        // 어떤 교사의 일정이 보결 시작 시각에 딱 끝나는 사례를 찾는다
        let idx = crate::domain::schedule::build_busy_index(&ctx.snap);
        let mut done = false;
        'outer: for cls in &ctx.snap.classes {
            for p in 1..=15 {
                let Some((s, _)) = ctx.slot(cls.grade, SLOT_PERIOD, Some(p)) else {
                    continue;
                };
                for t in &ctx.snap.teachers {
                    if !t.active || !t.is_substitutable {
                        continue;
                    }
                    let blocks = idx.of(t.id);
                    if blocks.is_empty() {
                        continue;
                    }
                    let abuts = blocks.iter().any(|b| b.interval.end == s);
                    if !abuts {
                        continue;
                    }
                    // 그 시간에 다른 일정이 없어야 검증이 성립
                    if !idx.conflicts(t.id, &Interval::new(s, s + 1)).is_empty() {
                        continue;
                    }
                    let r = ctx.find(cls.id, SLOT_PERIOD, Some(p));
                    let (ok, code) = ctx.verdict(&r, t.id);
                    assert!(
                        ok,
                        "{} 선생님의 일정이 {}에 끝나고 보결이 같은 시각에 시작하므로 후보",
                        t.name,
                        crate::domain::time::fmt_min(s)
                    );
                    pass(
                        "일정 종료와 보결 시작이 같은 시각이면 후보 포함",
                        &format!(
                            "{} 선생님 일정 종료 {} == {} {}교시 시작 → 후보 ({})",
                            t.name,
                            crate::domain::time::fmt_min(s),
                            cls.label,
                            p,
                            code
                        ),
                    );
                    done = true;
                    break 'outer;
                }
            }
        }
        if !done {
            println!("  --   접점 사례를 이 자료에서 찾지 못해 넘어감");
        }
    }

    // ---------- 7. 부재 교사 제외 ----------
    {
        let target_class = ctx.snap.classes[0].id;
        let grade = ctx.snap.classes[0].grade;
        let period = (1..=15)
            .find(|p| ctx.slot(grade, SLOT_PERIOD, Some(*p)).is_some())
            .unwrap();

        // 그 시간에 원래 후보였던 교사를 하나 고른다
        let before = ctx.find(target_class, SLOT_PERIOD, Some(period));
        let victim = before
            .eligible
            .first()
            .map(|c| c.teacher_id)
            .expect("후보가 한 명은 있어야 한다");

        ctx.conn
            .execute(
                "INSERT INTO absences(teacher_id, date, is_all_day, reason_code)
                 VALUES (?1, ?2, 1, 'ANNUAL')",
                rusqlite::params![victim, ctx.date],
            )
            .unwrap();
        ctx.reload();

        let r = ctx.find(target_class, SLOT_PERIOD, Some(period));
        let (ok, code) = ctx.verdict(&r, victim);
        assert!(!ok, "부재 중인 교사는 제외");
        assert_eq!(code, EXCLUDED_ABSENCE);
        pass(
            "부재 교사 제외",
            &format!("{} 선생님 연가(종일) 등록 → 제외 ({})", ctx.name_of(victim), code),
        );

        // 되돌린다
        ctx.conn
            .execute("DELETE FROM absences WHERE teacher_id = ?1", [victim])
            .unwrap();
        ctx.reload();
        assert!(ctx.verdict(&ctx.find(target_class, SLOT_PERIOD, Some(period)), victim).0);
    }

    // ---------- 8. 이미 다른 보결이 배정된 교사 제외 ----------
    {
        let target_class = ctx.snap.classes[0].id;
        let grade = ctx.snap.classes[0].grade;
        let period = (1..=15)
            .find(|p| ctx.slot(grade, SLOT_PERIOD, Some(*p)).is_some())
            .unwrap();
        let (s, e) = ctx.slot(grade, SLOT_PERIOD, Some(period)).unwrap();

        let before = ctx.find(target_class, SLOT_PERIOD, Some(period));
        let victim = before.eligible.first().map(|c| c.teacher_id).unwrap();
        // 실제 DB에 이미 오늘 보결이 있을 수 있으므로 '늘어났는가'로 본다
        let today_before = ctx.counts.get(&victim).map(|c| c.today).unwrap_or(0);

        ctx.conn
            .execute(
                "INSERT INTO substitutions
                   (date, day_of_week, grade, class_no, class_label, slot_type, slot_label,
                    start_min, end_min, sub_teacher_id, status)
                 VALUES (?1, ?2, 9, 9, '9-확인', 'PERIOD', ?3, ?4, ?5, ?6, 'ASSIGNED')",
                rusqlite::params![
                    ctx.date,
                    ctx.snap.day_of_week,
                    format!("{period}교시"),
                    s,
                    e,
                    victim
                ],
            )
            .unwrap();
        ctx.reload();

        let r = ctx.find(target_class, SLOT_PERIOD, Some(period));
        let (ok, code) = ctx.verdict(&r, victim);
        assert!(!ok, "이미 보결이 배정된 교사는 제외");
        assert_eq!(code, EXCLUDED_ALREADY_ASSIGNED);
        pass(
            "같은 시간에 이미 다른 보결이 있으면 제외",
            &format!("{} 선생님 → 제외 ({})", ctx.name_of(victim), code),
        );

        // 오늘 보결 횟수에도 반영되는지
        assert_eq!(
            ctx.counts.get(&victim).map(|c| c.today).unwrap_or(0),
            today_before + 1,
            "오늘 보결 횟수가 하나 늘어야 한다"
        );
        pass("보결 횟수 집계", "오늘 횟수가 하나 늘었다");
    }

    // ---------- 9. 추천 기준 순서를 바꾸면 결과가 달라진다 ----------
    {
        // 기준이 실제로 갈리는 지점을 찾는다:
        // 동학년 담임 후보와 담임이 아닌 후보가 함께 있어야 한다
        let mut spot = None;
        'find_spot: for cls in &ctx.snap.classes {
            for p in 1..=15 {
                if ctx.slot(cls.grade, SLOT_PERIOD, Some(p)).is_none() {
                    continue;
                }
                let r = ctx.find(cls.id, SLOT_PERIOD, Some(p));
                let has_same_grade = r
                    .eligible
                    .iter()
                    .any(|c| c.homeroom_grades.contains(&cls.grade));
                let has_other = r
                    .eligible
                    .iter()
                    .any(|c| !c.homeroom_grades.contains(&cls.grade));
                if has_same_grade && has_other {
                    spot = Some((cls.id, cls.grade, p));
                    break 'find_spot;
                }
            }
        }

        if let Some((class_id, grade, period)) = spot {
            // 후보들에게 서로 다른 누적 횟수를 심어 순서 차이가 드러나게 한다
            let base = ctx.find(class_id, SLOT_PERIOD, Some(period));
            let ids: Vec<i64> = base.eligible.iter().map(|c| c.teacher_id).collect();
            // 동학년 담임에게만 누적을 많이 준다 (두 기준이 서로 다른 답을 내도록)
            for (i, id) in ids.iter().enumerate() {
                let same_grade = base
                    .eligible
                    .iter()
                    .find(|c| c.teacher_id == *id)
                    .map(|c| c.homeroom_grades.contains(&grade))
                    .unwrap_or(false);
                let n = if same_grade { 9 } else { 0 };
                for k in 0..n {
                    ctx.conn
                        .execute(
                            "INSERT INTO substitutions
                               (date, day_of_week, grade, class_no, class_label, slot_type, slot_label,
                                start_min, end_min, sub_teacher_id, status)
                             VALUES ('2020-01-01', 1, 9, ?1, '9-지난기록', 'PERIOD', ?2, ?3, ?4, ?5, 'ASSIGNED')",
                            rusqlite::params![
                                i as i32 + 1,
                                format!("{k}교시"),
                                600 + k * 10,
                                605 + k * 10,
                                id
                            ],
                        )
                        .unwrap();
                }
            }
            ctx.reload();

            // 학교 A: 동학년 우선
            ctx.set_priority(&["SAME_GRADE", "FEWEST_TODAY", "FEWEST_TOTAL"]);
            let a = ctx.find_ranked(class_id, SLOT_PERIOD, Some(period));
            let top_a = a.eligible[0].clone();

            // 학교 B: 누적 우선
            ctx.set_priority(&["FEWEST_TOTAL", "FEWEST_TODAY", "SAME_GRADE"]);
            let b = ctx.find_ranked(class_id, SLOT_PERIOD, Some(period));
            let top_b = b.eligible[0].clone();

            assert_eq!(a.eligible.len(), b.eligible.len(), "후보 수는 같아야 한다");
            assert_eq!(top_a.rank, 1);
            assert_eq!(top_b.rank, 1);
            assert!(!top_a.reason.is_empty() && !top_b.reason.is_empty());

            assert!(
                top_a.homeroom_grades.contains(&grade),
                "동학년 우선일 때는 동학년 담임이 1순위여야 한다 (지금 {})",
                top_a.name
            );
            assert!(
                !top_b.homeroom_grades.contains(&grade),
                "누적 우선일 때는 누적이 적은(동학년이 아닌) 교사가 1순위여야 한다 (지금 {})",
                top_b.name
            );
            assert_ne!(
                top_a.teacher_id, top_b.teacher_id,
                "기준 순서를 바꾸면 1순위가 실제로 달라져야 한다"
            );

            pass(
                "추천 기준 순서를 바꾸면 1순위가 달라진다",
                &format!(
                    "{} {}교시 · 후보 {}명
         동학년 우선 → 1순위 {} ({}) / 누적 우선 → 1순위 {} ({})",
                    ctx.label(class_id),
                    period,
                    a.eligible.len(),
                    top_a.name,
                    top_a.reason,
                    top_b.name,
                    top_b.reason
                ),
            );

            // 같은 기준으로 두 번 조회하면 순서가 같아야 한다
            let again = ctx.find_ranked(class_id, SLOT_PERIOD, Some(period));
            let names_1: Vec<&str> = b.eligible.iter().map(|c| c.name.as_str()).collect();
            let names_2: Vec<&str> = again.eligible.iter().map(|c| c.name.as_str()).collect();
            assert_eq!(names_1, names_2, "같은 기준·같은 자료면 순서가 같아야 한다");
            pass("같은 기준으로 재조회하면 순서가 같다", &names_2.join(" → "));

            // 기록을 지우고 기본 기준으로 되돌린다
            ctx.conn
                .execute("DELETE FROM substitutions WHERE date = '2020-01-01'", [])
                .unwrap();
            repo_priority::reset(&ctx.conn).unwrap();
            ctx.reload();
        } else {
            println!("  --   후보가 3명 이상인 시간을 찾지 못해 넘어감");
        }
    }

    // ---------- 요약 ----------
    {
        let grade = ctx.snap.classes[0].grade;
        let period = (1..=15)
            .find(|p| ctx.slot(grade, SLOT_PERIOD, Some(*p)).is_some())
            .unwrap();
        let r = ctx.find(ctx.snap.classes[0].id, SLOT_PERIOD, Some(period));
        println!(
            "\n{} {}교시 {} → 가능 {}명 / 불가 {}명",
            r.slot.class_full_label,
            period,
            fmt_range(&Interval::new(r.slot.start_min, r.slot.end_min)),
            r.eligible.len(),
            r.excluded.len()
        );
        let mut by_reason: HashMap<String, usize> = HashMap::new();
        for c in r.excluded.iter() {
            *by_reason.entry(c.reason_code.clone()).or_default() += 1;
        }
        let mut list: Vec<_> = by_reason.into_iter().collect();
        list.sort();
        for (code, n) in list {
            println!("   제외 {n:2}명  {code} ({})", status_label(&code));
        }
        for c in r.eligible.iter().take(6) {
            println!(
                "   가능      {} {} {} — {}",
                c.name, c.role_label, c.duty, c.status_label
            );
        }
    }
    println!();
}

// ============================================================
//  전담 시간 확인 안내 — 실제 DB
// ============================================================

/// 실제 시간표에서 '전담이 들어오는 교시'를 찾아 안내가 뜨는지 확인한다.
#[test]
#[ignore = "실제 사용 중인 DB가 필요하다"]
fn real_db_전담시간_확인안내() {
    let Some(conn) = open_copy() else {
        println!("실제 DB가 없어 넘어간다.");
        return;
    };

    let mut checked = 0usize;
    for day in 1..=5 {
        let date = date_for_weekday(day);
        let ctx = Ctx::load(open_copy().unwrap(), &date);

        // 이 요일에 전담(또는 교차)이 들어오는 (학급, 교시)를 모은다
        let covered: Vec<(i64, i32, i64)> = ctx
            .snap
            .lessons
            .iter()
            .filter(|l| l.replaces_homeroom)
            .map(|l| (l.class_id, l.period_no, l.teacher_id))
            .collect();

        for (class_id, period, sub_teacher) in covered.iter().take(3) {
            let Some(hr) = ctx.homeroom_of(*class_id) else { continue };
            if ctx.slot_of(*class_id, *period).is_none() {
                continue;
            }

            // (1) 담임이 결근한 경우 — 보결이 필요 없으니 안내가 떠야 한다
            let r = ctx.find_absent(*class_id, SLOT_PERIOD, Some(*period), Some(hr));
            let ic = &r.slot.in_charge;
            assert_eq!(
                ic.teacher_id,
                Some(*sub_teacher),
                "{} {}교시 담당은 전담이어야 한다",
                ctx.label(*class_id),
                period
            );
            assert!(ic.covered_by_other);
            let n = r.notice.as_ref().unwrap_or_else(|| {
                panic!(
                    "{} {}요일 {}교시에 안내가 떠야 한다",
                    ctx.label(*class_id),
                    day,
                    period
                )
            });
            assert!(n.title.contains("확인 바랍니다"), "{}", n.title);

            // (2) 그 전담이 결근한 경우 — 보결이 맞으니 안내가 없어야 한다
            let r2 = ctx.find_absent(*class_id, SLOT_PERIOD, Some(*period), Some(*sub_teacher));
            assert!(
                r2.notice.is_none(),
                "{}이 결근했으면 보결이 맞다",
                ctx.name_of(*sub_teacher)
            );
            // 그 반 담임은 이 시간 공강이므로 후보로 남아야 한다.
            // (실제 DB에는 이미 배정된 보결이 있을 수 있다 — 그것은 정상적인 제외 사유다)
            let (ok, code) = ctx.verdict(&r2, hr);
            assert!(
                ok || code == "EXCLUDED_ALREADY_ASSIGNED",
                "담임 {} 은 전담 시간에 공강이므로 후보여야 한다 ({code})",
                ctx.name_of(hr)
            );

            pass(
                &format!("{} {}요일 {}교시", ctx.label(*class_id), day, period),
                &format!(
                    "담당 {} ({}) · 안내 \"{}\" · 전담 결근 시에는 안내 없음",
                    ctx.name_of(*sub_teacher),
                    ic.subject_name.clone().unwrap_or_else(|| "?".into()),
                    n.title
                ),
            );
            checked += 1;
        }
    }

    // 담임 시간에는 안내가 없다
    let ctx = Ctx::load(conn, &date_for_weekday(1));
    let mut plain = 0usize;
    for c in ctx.snap.classes.iter() {
        for p in 1..=6 {
            if ctx.slot_of(c.id, p).is_none() {
                continue;
            }
            let taken = ctx
                .snap
                .lessons
                .iter()
                .any(|l| l.replaces_homeroom && l.class_id == c.id && l.period_no == p);
            if taken || c.homeroom_teacher_id.is_none() {
                continue;
            }
            let r = ctx.find(c.id, SLOT_PERIOD, Some(p));
            assert!(
                r.notice.is_none(),
                "{} {}교시는 담임 시간이므로 안내가 없어야 한다",
                c.label,
                p
            );
            assert_eq!(r.slot.in_charge.teacher_id, c.homeroom_teacher_id);
            plain += 1;
        }
    }

    if checked == 0 {
        println!("전담 시간표가 없어 확인할 것이 없다.");
    }
    pass(
        "담임 시간에는 안내가 없다",
        &format!("전담 시간 {checked}건 · 담임 시간 {plain}건 확인"),
    );
}

// ============================================================
//  Phase 8 — 배정 · 취소 · 재배정 (실제 DB 복사본)
// ============================================================

/// 실제 자료 위에서 배정 → 취소 → 재배정 → 기록 조회를 한 바퀴 돌린다.
/// 원본은 복사본이므로 사용자의 자료는 바뀌지 않는다.
#[test]
#[ignore = "실제 사용 중인 DB가 필요하다"]
fn real_db_배정_취소_재배정() {
    use crate::repo::assign as ra;

    let Some(conn) = open_copy() else {
        println!("실제 DB가 없어 넘어간다.");
        return;
    };

    // 후보가 넉넉한 (요일, 학급, 교시)를 찾는다
    let mut found: Option<(String, i64, i32, i64, Vec<i64>)> = None;
    'outer: for day in 1..=5 {
        let date = date_for_weekday(day);
        let ctx = Ctx::load(open_copy().unwrap(), &date);
        for c in ctx.snap.classes.iter() {
            for p in 1..=6 {
                let Some((start, _)) = ctx.slot_of(c.id, p) else {
                    continue;
                };
                let Some(hr) = ctx.homeroom_of(c.id) else { continue };
                // 이미 보결이 들어가 있는 칸은 건너뛴다 (사용자가 실제로 배정해 둔 것)
                let busy: i64 = ctx
                    .conn
                    .query_row(
                        "SELECT COUNT(*) FROM substitutions
                          WHERE date = ?1 AND status = 'ASSIGNED' AND class_id = ?2
                            AND start_min < ?3 AND ?4 < end_min",
                        rusqlite::params![date, c.id, start + 1, start],
                        |r| r.get(0),
                    )
                    .unwrap_or(0);
                if busy > 0 {
                    continue;
                }
                let r = ctx.find_absent(c.id, SLOT_PERIOD, Some(p), Some(hr));
                if r.eligible.len() >= 2 {
                    found = Some((
                        date.clone(),
                        c.id,
                        p,
                        hr,
                        r.eligible.iter().map(|x| x.teacher_id).collect(),
                    ));
                    break 'outer;
                }
            }
        }
    }

    let Some((date, class_id, period, absent, cands)) = found else {
        println!("후보가 2명 이상인 시간을 찾지 못해 넘어간다.");
        return;
    };
    let ctx = Ctx::load(conn, &date);
    let (first, second) = (cands[0], cands[1]);

    let input = |sub: i64| ra::AssignInput {
        date: date.clone(),
        class_id,
        slot_type: SLOT_PERIOD.into(),
        period_no: Some(period),
        absent_teacher_id: Some(absent),
        sub_teacher_id: sub,
        absence_id: None,
        reason_code: Some("TRIP".into()),
        reason_text: None,
    };

    // 1) 배정
    let saved = ra::assign_one(&ctx.conn, &input(first)).expect("배정되어야 한다");
    assert_eq!(saved.class_label, ctx.label(class_id));
    let before = ctx.conn
        .query_row(
            "SELECT COUNT(*) FROM substitutions WHERE status='ASSIGNED' AND sub_teacher_id=?1 AND date=?2",
            rusqlite::params![first, date],
            |r| r.get::<_, i64>(0),
        )
        .unwrap();
    assert_eq!(before, 1);

    // 2) 같은 학급 같은 시간에 다른 사람을 넣으려 하면 막힌다
    let blocked = ra::assign_one(&ctx.conn, &input(second)).unwrap_err();
    assert!(blocked.user_message.contains("이미"), "{}", blocked.user_message);

    // 3) 취소하면 집계에서 빠지고 기록은 남는다
    ra::cancel(&ctx.conn, saved.id, Some("담임 출근")).unwrap();
    let counts = repo_find::counts(&ctx.conn, &date).unwrap();
    let after = counts.get(&first).copied().unwrap_or_default().today;
    assert_eq!(after, 0, "취소분은 오늘 횟수에서 빠진다");

    // 4) 같은 시간에 다시 배정할 수 있다
    let again = ra::assign_one(&ctx.conn, &input(second)).expect("재배정되어야 한다");
    assert_ne!(again.id, saved.id);

    // 5) 기록에는 둘 다 남는다
    let h = ra::history(
        &ctx.conn,
        &ra::HistoryFilter {
            from: Some(date.clone()),
            to: Some(date.clone()),
            ..Default::default()
        },
    )
    .unwrap();
    let mine: Vec<_> = h
        .rows
        .iter()
        .filter(|r| r.id == saved.id || r.id == again.id)
        .collect();
    assert_eq!(mine.len(), 2);
    assert!(mine.iter().any(|r| r.status == "CANCELLED"));
    assert!(mine.iter().any(|r| r.status == "ASSIGNED"));

    pass(
        "배정 → 차단 → 취소 → 재배정",
        &format!(
            "{} {} {}교시 · {} → 취소 → {} (기록 2건 유지)",
            date,
            ctx.label(class_id),
            period,
            ctx.name_of(first),
            ctx.name_of(second)
        ),
    );

    // 6) 하루 일괄 보결 — 결근 교사의 그 날 일정을 뽑는다
    let plan = ra::day_plan(&ctx.conn, &date, absent).unwrap();
    assert_eq!(plan.teacher_id, absent);
    pass(
        "하루 일괄 보결 대상 추출",
        &format!(
            "{} 선생님 · {}칸 ({})",
            plan.teacher_name,
            plan.slots.len(),
            plan.slots
                .iter()
                .map(|x| format!("{} {}", x.class_label, x.slot_label))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    );
}

// ============================================================
//  Phase 9 — 현황 (실제 DB 복사본)
// ============================================================

/// 실제 자료로 현황 화면의 숫자를 뽑아 보고, 취소하면 숫자가 따라오는지 본다.
#[test]
#[ignore = "실제 사용 중인 DB가 필요하다"]
fn real_db_현황() {
    use crate::domain::period;
    use crate::repo::assign as ra;
    use crate::repo::stats as rs;
    use rusqlite::OptionalExtension;

    let Some(conn) = open_copy() else {
        println!("실제 DB가 없어 넘어간다.");
        return;
    };

    let ask = |preset: &str, from: Option<&str>, to: Option<&str>| {
        rs::view(
            &conn,
            &rs::StatsQuery {
                preset: Some(preset.to_string()),
                from: from.map(str::to_string),
                to: to.map(str::to_string),
            },
        )
        .expect("현황을 만들 수 있어야 한다")
    };

    // ---------- 기간별로 열어 본다 ----------
    for preset in [period::TODAY, period::WEEK, period::MONTH, period::TERM] {
        let v = ask(preset, None, None);
        assert!(v.from <= v.to, "{preset}: 범위가 뒤집히면 안 된다");
        // '보결 필요'는 결근 기록에서 나온 시간이고, '배정'은 기간 안의 배정 건수
        // 전체다. 결근을 등록하지 않고 바로 배정한 건도 있으므로 둘은 다를 수 있다.
        // 반드시 맞아야 하는 것은 필요 = 필요분 중 배정 + 미배정 이다.
        assert_eq!(
            v.summary.required,
            v.summary.covered + v.summary.unassigned,
            "{preset}: 필요 = 배정(필요분) + 미배정"
        );
        pass(
            &format!("{preset} 현황"),
            &format!(
                "{} · 결근 {}명 / 필요 {}건 (배정 {} · 미배정 {}) / 기간 배정 {}건 / 취소 {}건 / 맡은 교사 {}명",
                v.range_label,
                v.summary.absent_teachers,
                v.summary.required,
                v.summary.covered,
                v.summary.unassigned,
                v.summary.assigned,
                v.summary.cancelled,
                v.summary.sub_teachers
            ),
        );
    }

    // ---------- 교사별 표 ----------
    let term = ask(period::TERM, None, None);
    assert!(!term.teachers.is_empty(), "교사 목록이 있어야 한다");
    let mut top: Vec<&rs::TeacherStat> = term.teachers.iter().filter(|t| t.total > 0).collect();
    top.sort_by_key(|t| -t.total);
    pass(
        "교사별 누적",
        &if top.is_empty() {
            "아직 보결 기록이 없다".to_string()
        } else {
            top.iter()
                .take(5)
                .map(|t| format!("{}({}) {}회 {}", t.name, t.role_label, t.total, t.band_label))
                .collect::<Vec<_>>()
                .join(" · ")
        },
    );

    // 누적은 기간과 상관없이 같아야 한다
    let today = ask(period::TODAY, None, None);
    for t in &today.teachers {
        let same = term.teachers.iter().find(|x| x.teacher_id == t.teacher_id).unwrap();
        assert_eq!(t.total, same.total, "{}: 누적은 기간에 흔들리지 않는다", t.name);
    }

    // ---------- 구분별 분포 ----------
    for sp in &term.spreads {
        assert!(sp.max >= sp.min);
        assert_eq!(sp.spread, sp.max - sp.min);
        pass(
            &format!("{} 분포", sp.role_label),
            &format!(
                "{}명 · 합계 {} · 평균 {:.1} · 최다 {} ({}) · 최소 {} · 참고표시 {}",
                sp.people,
                sp.total,
                sp.avg,
                sp.max,
                sp.max_name.clone().unwrap_or_default(),
                sp.min,
                if sp.comparable { "함" } else { "안 함" }
            ),
        );
    }

    // ---------- 취소하면 숫자가 따라온다 ----------
    let live: Option<(i64, String, i64)> = conn
        .query_row(
            "SELECT s.id, s.date, s.sub_teacher_id FROM substitutions s
              WHERE s.status = 'ASSIGNED' ORDER BY s.date DESC LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()
        .unwrap();

    if let Some((id, date, tid)) = live {
        let before = ask(period::CUSTOM, Some(&date), Some(&date));
        let b = before.teachers.iter().find(|t| t.teacher_id == tid).unwrap();
        let (b_assigned, b_cancelled, b_count, b_total) =
            (before.summary.assigned, before.summary.cancelled, b.period, b.total);

        ra::cancel(&conn, id, Some("현황 검증")).unwrap();

        let after = ask(period::CUSTOM, Some(&date), Some(&date));
        let a = after.teachers.iter().find(|t| t.teacher_id == tid).unwrap();
        assert_eq!(after.summary.assigned, b_assigned - 1, "배정 수가 하나 줄어야 한다");
        assert_eq!(after.summary.cancelled, b_cancelled + 1, "취소 수가 하나 늘어야 한다");
        assert_eq!(a.period, b_count - 1, "그 교사의 기간 횟수가 줄어야 한다");
        assert_eq!(a.total, b_total - 1, "누적에서도 빠져야 한다");

        pass(
            "취소하면 현황이 곧바로 따라온다",
            &format!(
                "{} {} · 배정 {}→{} · 취소 {}→{} · 본인 누적 {}→{}",
                date, a.name, b_assigned, after.summary.assigned, b_cancelled,
                after.summary.cancelled, b_total, a.total
            ),
        );
    } else {
        println!("  --   배정 기록이 없어 취소 검증은 건너뛴다");
    }

    // ---------- 날짜별 ----------
    let month = ask(period::MONTH, None, None);
    let busy: Vec<&rs::DayStat> = month
        .days
        .iter()
        .filter(|d| d.absent_teachers > 0 || d.assigned > 0)
        .collect();
    pass(
        "날짜별 현황",
        &format!(
            "{}일 가운데 기록이 있는 날 {}일{}",
            month.days.len(),
            busy.len(),
            match month.days.iter().filter(|d| d.unassigned > 0).count() {
                0 => " · 미배정 없음".to_string(),
                n => format!(" · 미배정이 남은 날 {n}일"),
            }
        ),
    );
}
