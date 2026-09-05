use super::*;

const HR: &str = "HOMEROOM";
const SP: &str = "SPECIAL";

fn share(name: &str, role: &str, count: i32) -> Share {
    Share {
        teacher_id: name.len() as i64 * 100 + count as i64,
        name: name.to_string(),
        role_code: role.to_string(),
        role_label: if role == HR { "담임" } else { "전담" }.to_string(),
        count,
    }
}

fn band(shares: &[Share], name: &str) -> String {
    let sp = spreads(shares);
    let s = shares.iter().find(|s| s.name == name).unwrap();
    band_of(s, &sp)
}

#[test]
fn 구분끼리만_견준다() {
    // 전담은 공강이 많아 자연히 횟수가 크다. 담임과 견주면 안 된다.
    let v = vec![
        share("담임가", HR, 1),
        share("담임나", HR, 2),
        share("담임다", HR, 3),
        share("전담가", SP, 20),
        share("전담나", SP, 21),
        share("전담다", SP, 22),
    ];
    assert_eq!(band(&v, "전담나"), BAND_TYPICAL, "전담 안에서는 평균 수준이다");
    assert_eq!(band(&v, "담임나"), BAND_TYPICAL, "담임 안에서도 평균 수준이다");

    let sp = spreads(&v);
    assert_eq!(sp.len(), 2, "구분마다 따로 나온다");
    let hr = sp.iter().find(|s| s.role_code == HR).unwrap();
    assert_eq!(hr.avg, 2.0);
    assert_eq!((hr.max, hr.min, hr.spread), (3, 1, 2));
    let spd = sp.iter().find(|s| s.role_code == SP).unwrap();
    assert_eq!(spd.avg, 21.0);
}

#[test]
fn 평균에서_크게_벗어나면_표시한다() {
    let v = vec![
        share("가", HR, 0),
        share("나", HR, 1),
        share("다", HR, 1),
        share("라", HR, 10),
    ];
    // 평균 3.0, 허용폭 max(2, 0.75) = 2.0 -> 1.0~5.0 이 '평균 수준'
    assert_eq!(band(&v, "라"), BAND_MORE);
    assert_eq!(band(&v, "가"), BAND_LESS, "0회는 경계 1.0보다 아래다");
    assert_eq!(band(&v, "나"), BAND_TYPICAL, "1회는 경계 안이다");
}

#[test]
fn 한두_회_차이는_평균_수준으로_본다() {
    let v = vec![
        share("가", HR, 2),
        share("나", HR, 4),
        share("다", HR, 6),
    ];
    // 평균 4.0, 허용폭 max(2, 1.0) = 2.0 -> 2.0~6.0
    assert_eq!(band(&v, "가"), BAND_TYPICAL);
    assert_eq!(band(&v, "나"), BAND_TYPICAL);
    assert_eq!(band(&v, "다"), BAND_TYPICAL);
}

#[test]
fn 전체_횟수가_적으면_표시하지_않는다() {
    // 학기 초처럼 아직 몇 건 없을 때 1~2회 차이로 표시가 붙으면 소음이 된다
    let v = vec![
        share("가", HR, 0),
        share("나", HR, 0),
        share("다", HR, 1),
        share("라", HR, 2),
    ];
    // 평균 0.75, 허용폭 2.0 -> 2.75 를 넘어야 '많음'
    assert_eq!(band(&v, "라"), BAND_TYPICAL, "2회로는 표시하지 않는다");
    assert_eq!(band(&v, "가"), BAND_TYPICAL);
}

#[test]
fn 횟수가_크면_허용폭도_넓어진다() {
    let v = vec![
        share("가", SP, 16),
        share("나", SP, 20),
        share("다", SP, 24),
    ];
    // 평균 20, 허용폭 max(2, 5) = 5 -> 15~25
    assert_eq!(band(&v, "가"), BAND_TYPICAL, "20회 중 4회 차이는 평균 수준");
    assert_eq!(band(&v, "다"), BAND_TYPICAL);
}

#[test]
fn 사람이_적으면_표시하지_않는다() {
    let v = vec![share("가", HR, 0), share("나", HR, 9)];
    assert_eq!(band(&v, "나"), BAND_NONE, "둘뿐이면 한쪽은 반드시 많음이 되어 뜻이 없다");
    assert!(!spreads(&v)[0].comparable);
}

#[test]
fn 모두_같으면_표시하지_않는다() {
    let v = vec![
        share("가", HR, 2),
        share("나", HR, 2),
        share("다", HR, 2),
        share("라", HR, 2),
    ];
    assert_eq!(band(&v, "가"), BAND_NONE);
    let sp = &spreads(&v)[0];
    assert_eq!(sp.spread, 0);
    assert!(!sp.comparable);
}

#[test]
fn 아무도_보결을_맡지_않아도_계산이_된다() {
    let v = vec![
        share("가", HR, 0),
        share("나", HR, 0),
        share("다", HR, 0),
    ];
    let sp = &spreads(&v)[0];
    assert_eq!(sp.total, 0);
    assert_eq!(sp.avg, 0.0);
    assert!(!sp.comparable, "차이가 없으면 표시하지 않는다");
    assert_eq!(band(&v, "가"), BAND_NONE);
}

#[test]
fn 최다_최소_이름을_알려준다() {
    let v = vec![
        share("김가", HR, 1),
        share("박나", HR, 7),
        share("이다", HR, 3),
    ];
    let sp = &spreads(&v)[0];
    assert_eq!(sp.max_name.as_deref(), Some("박나"));
    assert_eq!(sp.min_name.as_deref(), Some("김가"));
}

#[test]
fn 표시_문구는_판정하지_않는다() {
    for b in [BAND_MORE, BAND_TYPICAL, BAND_LESS, BAND_NONE] {
        let t = band_label(b);
        assert!(!t.contains("과다"), "{t}");
        assert!(!t.contains("문제"), "{t}");
        assert!(!t.contains("부족"), "{t}");
    }
    assert!(HOW_TEXT.contains("참고용"));
}

#[test]
fn 빈_목록도_안전하다() {
    assert!(spreads(&[]).is_empty());
}
