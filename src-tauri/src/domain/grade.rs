//! 학년에 관한 학교 규칙. **DB와 Tauri를 모르는 순수 계산 계층.**
//!
//! ## 학년군
//!
//! 초등학교에서는 이웃한 두 학년을 한 묶음으로 본다. 교육과정이 두 학년
//! 단위로 묶여 있어서, 같은 학년 선생님 다음으로 사정을 잘 아는 것이
//! 짝 학년 선생님이다.
//!
//! ```text
//! 1·2학년   3·4학년   5·6학년
//! ```
//!
//! ## 숫자를 여기 한 곳에만 둔다
//!
//! '저학년 / 고학년' 같은 묶음을 여기저기 적어 두면 학교마다 다른 운영을
//! 만났을 때 고칠 곳을 놓친다. 그래서 학년 묶음을 다루는 곳은 이 함수
//! 하나뿐이다.
//!
//! 이 프로그램은 초등학교 전용이므로 묶음을 사용자가 정하게 하지 않는다.
//! 나중에 학교가 정하게 할 일이 생기면 이 함수의 속만 바꾸면 된다.

/// 이 학년과 한 학년군으로 묶이는 **짝 학년**.
///
/// 자기 자신은 절대 돌려주지 않는다 — '같은 학년'과 '같은 학년군'은
/// 서로 다른 것이고, 한 사람이 둘 다일 수는 없다.
///
/// 1~6학년 밖의 값(0, 7, 음수 등)에는 짝이 없으므로 `None` 이다.
/// 학년 자료가 덜 들어갔거나 다른 학교급 자료가 섞여 들어와도 여기서
/// 조용히 멈춘다.
pub fn paired_grade(grade: i32) -> Option<i32> {
    match grade {
        1 | 3 | 5 => Some(grade + 1),
        2 | 4 | 6 => Some(grade - 1),
        _ => None,
    }
}

/// 두 학년이 한 학년군인가. (같은 학년은 학년군이 **아니다**)
pub fn same_band(a: i32, b: i32) -> bool {
    paired_grade(a) == Some(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 짝_학년은_여섯_학년_모두_짝이_있다() {
        assert_eq!(paired_grade(1), Some(2));
        assert_eq!(paired_grade(2), Some(1));
        assert_eq!(paired_grade(3), Some(4));
        assert_eq!(paired_grade(4), Some(3));
        assert_eq!(paired_grade(5), Some(6));
        assert_eq!(paired_grade(6), Some(5));
    }

    #[test]
    fn 자기_자신은_짝이_아니다() {
        for g in 1..=6 {
            assert_ne!(paired_grade(g), Some(g), "{g}학년");
            assert!(!same_band(g, g), "{g}학년");
        }
    }

    #[test]
    fn 짝의_짝은_자기_자신이다() {
        for g in 1..=6 {
            let p = paired_grade(g).unwrap();
            assert_eq!(paired_grade(p), Some(g), "{g}학년의 짝 {p}학년");
        }
    }

    #[test]
    fn 학년군은_양쪽_모두에서_성립한다() {
        for (a, b) in [(1, 2), (3, 4), (5, 6)] {
            assert!(same_band(a, b), "{a}·{b}");
            assert!(same_band(b, a), "{b}·{a}");
        }
    }

    #[test]
    fn 학년군이_아닌_짝은_걸러진다() {
        // 2·3 은 이웃이지만 한 학년군이 아니다
        assert!(!same_band(2, 3));
        assert!(!same_band(4, 5));
        // 멀리 떨어진 학년
        assert!(!same_band(1, 6));
        assert!(!same_band(3, 6));
    }

    #[test]
    fn 범위_밖의_학년은_짝이_없다() {
        for g in [-1, 0, 7, 8, 99, i32::MIN, i32::MAX] {
            assert_eq!(paired_grade(g), None, "{g}학년");
            assert!(!same_band(g, 1), "{g}학년");
            assert!(!same_band(1, g), "1학년과 {g}학년");
        }
    }
}
