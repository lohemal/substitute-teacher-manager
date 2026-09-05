//! 학급 표시 이름을 만드는 곳. **여기 한 곳에서만 만든다.**
//!
//! 화면과 저장 스냅샷이 같은 문자열을 쓰도록 하기 위해 Rust에 두고,
//! 프론트엔드에는 `src/lib/classLabel.ts`에 같은 규칙을 둔다.

/// 사용자가 '가람반'이라고 입력해도 '가람'으로 보관한다.
/// (표시할 때 '반'을 붙이므로 '가람반반'이 되지 않도록)
pub fn normalize_class_name(raw: &str) -> String {
    let t = raw.trim();
    let stripped = t.strip_suffix('반').unwrap_or(t).trim();
    if stripped.is_empty() {
        t.to_string()
    } else {
        stripped.to_string()
    }
}

/// 표·칩처럼 좁은 곳에서 쓰는 짧은 표기. `5-가람` / `5-1`
pub fn class_short(grade: i32, class_no: i32, name: Option<&str>) -> String {
    match name.map(str::trim).filter(|s| !s.is_empty()) {
        Some(n) => format!("{grade}-{n}"),
        None => format!("{grade}-{class_no}"),
    }
}

/// 안내 문장에서 쓰는 긴 표기. `5학년 가람반` / `5학년 1반`
pub fn class_full(grade: i32, class_no: i32, name: Option<&str>) -> String {
    match name.map(str::trim).filter(|s| !s.is_empty()) {
        Some(n) => format!("{grade}학년 {n}반"),
        None => format!("{grade}학년 {class_no}반"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 이름이_없으면_숫자로_표기한다() {
        assert_eq!(class_short(5, 1, None), "5-1");
        assert_eq!(class_full(5, 1, None), "5학년 1반");
    }

    #[test]
    fn 이름이_있으면_이름으로_표기한다() {
        assert_eq!(class_short(5, 1, Some("가람")), "5-가람");
        assert_eq!(class_full(5, 1, Some("가람")), "5학년 가람반");
    }

    #[test]
    fn 빈_이름은_숫자로_되돌린다() {
        assert_eq!(class_short(3, 2, Some("   ")), "3-2");
        assert_eq!(class_full(3, 2, Some("")), "3학년 2반");
    }

    #[test]
    fn 반_글자를_붙여_입력해도_한_번만_붙는다() {
        assert_eq!(normalize_class_name("가람반"), "가람");
        assert_eq!(normalize_class_name("  나리반  "), "나리");
        assert_eq!(class_full(1, 1, Some(&normalize_class_name("가람반"))), "1학년 가람반");
    }

    #[test]
    fn 이름_자체가_반인_경우는_그대로_둔다() {
        assert_eq!(normalize_class_name("반"), "반");
    }

    #[test]
    fn 이름_안쪽의_반은_지우지_않는다() {
        assert_eq!(normalize_class_name("반딧불"), "반딧불");
    }
}
