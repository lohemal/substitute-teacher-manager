//! 시간 구간. **프로그램 안의 모든 시간 겹침 판단은 여기 한 곳을 쓴다.**

use serde::Serialize;

/// 자정 기준 분. 09:00 → 540
pub type Min = i32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Interval {
    pub start: Min,
    pub end: Min,
}

impl Interval {
    pub fn new(start: Min, end: Min) -> Self {
        Self { start, end }
    }

    /// 1분이라도 겹치면 true.
    ///
    /// 판단식은 `existing_start < target_end && target_start < existing_end`.
    /// 그래서 **한쪽이 끝나는 시각에 다른 쪽이 시작하면 겹치지 않는다.**
    /// (예: 11:25 종료 ↔ 12:15 시작, 12:15 종료 ↔ 12:15 시작 모두 겹치지 않음)
    pub fn overlaps(&self, other: &Interval) -> bool {
        self.start < other.end && other.start < self.end
    }

    /// (Phase 8 배정 저장 시 검증에서 사용)
    #[allow(dead_code)]
    pub fn is_valid(&self) -> bool {
        self.end > self.start
    }

    /// (Phase 9 통계에서 사용)
    #[allow(dead_code)]
    pub fn minutes(&self) -> Min {
        self.end - self.start
    }
}

/// 화면·기록에 쓰는 `09:40` 표기.
pub fn fmt_min(m: Min) -> String {
    format!("{:02}:{:02}", m / 60, m % 60)
}

pub fn fmt_range(i: &Interval) -> String {
    format!("{}~{}", fmt_min(i.start), fmt_min(i.end))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn iv(a: Min, b: Min) -> Interval {
        Interval::new(a, b)
    }
    fn hm(h: Min, m: Min) -> Min {
        h * 60 + m
    }

    #[test]
    fn 완전히_같은_구간은_겹친다() {
        assert!(iv(600, 640).overlaps(&iv(600, 640)));
    }

    #[test]
    fn 일부만_겹쳐도_겹친다() {
        assert!(iv(600, 640).overlaps(&iv(630, 700)));
        assert!(iv(630, 700).overlaps(&iv(600, 640)));
    }

    #[test]
    fn 한_구간이_다른_구간에_들어가도_겹친다() {
        assert!(iv(600, 700).overlaps(&iv(620, 640)));
        assert!(iv(620, 640).overlaps(&iv(600, 700)));
    }

    #[test]
    fn 단_1분만_겹쳐도_겹친다() {
        assert!(iv(600, 641).overlaps(&iv(640, 700)));
    }

    #[test]
    fn 끝나는_시각과_시작하는_시각이_같으면_겹치지_않는다() {
        // 요구사항 8: 11:25~12:15 기존 일정, 12:15~12:55 보결 -> 겹치지 않음
        let existing = iv(hm(11, 25), hm(12, 15));
        let target = iv(hm(12, 15), hm(12, 55));
        assert!(!existing.overlaps(&target));
        assert!(!target.overlaps(&existing));
    }

    #[test]
    fn 완전히_떨어진_구간은_겹치지_않는다() {
        assert!(!iv(600, 640).overlaps(&iv(700, 740)));
        assert!(!iv(700, 740).overlaps(&iv(600, 640)));
    }

    #[test]
    fn 겹침_판단은_양쪽_방향이_같다() {
        let cases = [
            (iv(540, 580), iv(580, 620)),
            (iv(540, 581), iv(580, 620)),
            (iv(540, 700), iv(600, 640)),
            (iv(540, 580), iv(700, 740)),
        ];
        for (a, b) in cases {
            assert_eq!(a.overlaps(&b), b.overlaps(&a), "{a:?} vs {b:?}");
        }
    }

    #[test]
    fn 표기() {
        assert_eq!(fmt_min(hm(9, 40)), "09:40");
        assert_eq!(fmt_range(&iv(hm(12, 15), hm(12, 55))), "12:15~12:55");
    }
}
