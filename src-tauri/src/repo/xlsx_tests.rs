//! 엑셀 파일 만들기 시험.
//!
//! XLSX는 zip 안에 XML이 들어 있는 형식이다. 그래서 만든 파일을 **다시 풀어
//! XML을 들여다보며** 확인한다. 눈으로 여는 것과 같은 것을 보는 셈이다.
//!   - 숫자가 글자로 저장되지 않았는가 (`<v>15000</v>` vs `t="s"`)
//!   - 한글이 그대로 들어 있는가
//!   - 머리글 고정·자동필터가 걸렸는가

use std::io::Read;

use super::*;

fn tmp_dir(tag: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!(
        "bogyeol-xlsx-test-{tag}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// zip 안의 파일 하나를 글자로 꺼낸다.
fn part(path: &std::path::Path, inner: &str) -> String {
    let f = std::fs::File::open(path).expect("만든 파일을 열 수 있어야 한다");
    let mut zip = zip::ZipArchive::new(f).expect("XLSX는 zip 형식이다");
    let mut e = zip
        .by_name(inner)
        .unwrap_or_else(|_| panic!("{inner} 가 들어 있어야 한다"));
    let mut s = String::new();
    e.read_to_string(&mut s).unwrap();
    s
}

fn names(path: &std::path::Path) -> Vec<String> {
    let f = std::fs::File::open(path).unwrap();
    let zip = zip::ZipArchive::new(f).unwrap();
    zip.file_names().map(|s| s.to_string()).collect()
}

/// 시트 XML + 공유 문자열을 합쳐 돌려준다 (글자는 공유 문자열 쪽에 들어간다).
fn sheet_and_strings(path: &std::path::Path, n: usize) -> (String, String) {
    let sheet = part(path, &format!("xl/worksheets/sheet{n}.xml"));
    let strings = if names(path).iter().any(|x| x == "xl/sharedStrings.xml") {
        part(path, "xl/sharedStrings.xml")
    } else {
        String::new()
    };
    (sheet, strings)
}

fn sample() -> Sheet {
    let mut s = Sheet::new(
        "교사별 계산",
        vec![
            col("교사명", 14.0),
            col("보결 횟수", 12.0),
            col("지급액", 16.0),
            col("날짜", 13.0),
            col("시작 시각", 11.0),
        ],
    );
    s.push(vec![
        Cell::text("김민수"),
        Cell::Count(7),
        Cell::Money(105_000),
        Cell::Date("2026-09-07".into()),
        Cell::Time(9 * 60 + 40),
    ]);
    s.push(vec![
        Cell::text("박지현"),
        Cell::Count(4),
        Cell::Money(60_000),
        Cell::Date("2026-09-08".into()),
        Cell::Time(13 * 60),
    ]);
    s.with_total(vec![
        Cell::text("합계"),
        Cell::Count(11),
        Cell::Money(165_000),
        Cell::Blank,
        Cell::Blank,
    ])
}

// ------------------------------------------------------------
//  진짜 XLSX 인가
// ------------------------------------------------------------

#[test]
fn 진짜_xlsx_형식으로_만든다() {
    let dir = tmp_dir("format");
    let path = write_book(&dir, "시험.xlsx", &[sample()]).unwrap();

    assert!(path.exists());
    let bytes = std::fs::read(&path).unwrap();
    assert_eq!(&bytes[0..2], b"PK", "zip 으로 시작해야 한다 (확장자만 바꾼 것이 아니다)");
    assert!(!bytes.starts_with(&[0xEF, 0xBB, 0xBF]), "CSV 시절의 BOM 이 없다");

    let inside = names(&path);
    for want in [
        "[Content_Types].xml",
        "xl/workbook.xml",
        "xl/worksheets/sheet1.xml",
        "xl/styles.xml",
    ] {
        assert!(inside.iter().any(|x| x == want), "{want} 가 있어야 한다: {inside:?}");
    }
    std::fs::remove_dir_all(&dir).ok();
}

// ------------------------------------------------------------
//  숫자는 숫자로
// ------------------------------------------------------------

#[test]
fn 횟수와_금액은_글자가_아니라_숫자로_저장된다() {
    let dir = tmp_dir("number");
    let path = write_book(&dir, "시험.xlsx", &[sample()]).unwrap();
    let (sheet, strings) = sheet_and_strings(&path, 1);

    // 값이 그대로 숫자로 들어 있다
    assert!(sheet.contains("<v>7</v>"), "보결 횟수 7");
    assert!(sheet.contains("<v>105000</v>"), "지급액 105000");
    assert!(sheet.contains("<v>165000</v>"), "합계 165000");

    // 쉼표가 붙은 글자로 저장되지 않았다
    assert!(!sheet.contains("105,000"), "숫자를 글자로 적으면 Excel 이 합계를 못 낸다");
    assert!(!strings.contains("105,000"));
    assert!(!strings.contains("105000"), "공유 문자열에 금액이 있으면 글자로 저장된 것이다");

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn 금액에_천_단위_쉼표_서식이_붙는다() {
    let dir = tmp_dir("fmt");
    let path = write_book(&dir, "시험.xlsx", &[sample()]).unwrap();
    let styles = part(&path, "xl/styles.xml");

    assert!(
        styles.contains("#,##0&quot;원&quot;") || styles.contains("#,##0\"원\""),
        "금액 서식이 있어야 한다: {styles}"
    );
    assert!(styles.contains("#,##0"), "횟수 서식");
    assert!(styles.contains("yyyy\\-mm\\-dd") || styles.contains("yyyy-mm-dd"), "날짜 서식");
    assert!(styles.contains("hh:mm"), "시각 서식");

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn 날짜와_시각은_실제_값으로_저장된다() {
    let dir = tmp_dir("date");
    let path = write_book(&dir, "시험.xlsx", &[sample()]).unwrap();
    let (sheet, strings) = sheet_and_strings(&path, 1);

    // 2026-09-07 의 Excel 일련번호 = 1900 체계로 46272
    assert!(sheet.contains("<v>46272</v>"), "2026-09-07 이 실제 날짜여야 한다");
    assert!(!strings.contains("2026-09-07"), "날짜가 글자로 저장되면 정렬이 안 된다");

    // 09:40 = 580분 / 1440 = 0.402777...
    assert!(
        sheet.contains("<v>0.40277777777777779</v>") || sheet.contains("<v>0.402777"),
        "09:40 이 실제 시각이어야 한다: {sheet}"
    );

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn 스물네시는_글자로_남긴다() {
    // 24:00 을 실제 시각으로 담으면 00:00 이 되어 하루가 사라진다
    let dir = tmp_dir("t24");
    let mut s = Sheet::new("t", vec![col("끝", 10.0)]);
    s.push(vec![Cell::Time(1440)]);
    let path = write_book(&dir, "시험.xlsx", &[s]).unwrap();
    let (_, strings) = sheet_and_strings(&path, 1);
    assert!(strings.contains("24:00"), "{strings}");
    std::fs::remove_dir_all(&dir).ok();
}

// ------------------------------------------------------------
//  한글
// ------------------------------------------------------------

#[test]
fn 한글이_그대로_들어간다() {
    let dir = tmp_dir("hangul");
    let path = write_book(&dir, "시험.xlsx", &[sample()]).unwrap();
    let (_, strings) = sheet_and_strings(&path, 1);

    for want in ["교사명", "보결 횟수", "지급액", "김민수", "박지현", "합계"] {
        assert!(strings.contains(want), "'{want}' 가 있어야 한다");
    }
    // 시트 이름도 한글
    let book = part(&path, "xl/workbook.xml");
    assert!(book.contains("교사별 계산"), "{book}");

    std::fs::remove_dir_all(&dir).ok();
}

// ------------------------------------------------------------
//  머리글 · 고정 · 필터 · 너비
// ------------------------------------------------------------

#[test]
fn 머리글_고정과_자동필터가_걸린다() {
    let dir = tmp_dir("pane");
    let path = write_book(&dir, "시험.xlsx", &[sample()]).unwrap();
    let (sheet, _) = sheet_and_strings(&path, 1);

    assert!(sheet.contains("<pane"), "머리글 고정: {sheet}");
    assert!(sheet.contains(r#"ySplit="1""#), "1행을 고정한다");
    assert!(sheet.contains("frozen"), "고정 방식");
    assert!(sheet.contains("<autoFilter"), "자동필터");

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn 자동필터는_합계_행을_넣지_않는다() {
    // 걸러도 합계가 늘 보여야 한다
    let dir = tmp_dir("filter");
    let path = write_book(&dir, "시험.xlsx", &[sample()]).unwrap();
    let (sheet, _) = sheet_and_strings(&path, 1);

    // 자료 2줄 + 머리글 → 필터 범위는 A1:E3, 합계는 4행
    assert!(sheet.contains(r#"<autoFilter ref="A1:E3""#), "{sheet}");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn 열_너비가_들어간다() {
    let dir = tmp_dir("width");
    let path = write_book(&dir, "시험.xlsx", &[sample()]).unwrap();
    let (sheet, _) = sheet_and_strings(&path, 1);
    assert!(sheet.contains("<cols>"), "열 너비: {sheet}");
    assert!(sheet.contains("customWidth"), "직접 정한 너비");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn 머리글은_굵게_나온다() {
    let dir = tmp_dir("bold");
    let path = write_book(&dir, "시험.xlsx", &[sample()]).unwrap();
    let styles = part(&path, "xl/styles.xml");
    assert!(styles.contains("<b/>"), "굵은 글씨 서식이 있어야 한다");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn 필터를_끈_시트에는_걸리지_않는다() {
    let dir = tmp_dir("nofilter");
    let mut s = Sheet::new("조회 조건", vec![col("항목", 18.0), col("값", 30.0)]).no_filter();
    s.push(vec![Cell::text("기간"), Cell::text("2026-09-01 ~ 2026-09-30")]);
    let path = write_book(&dir, "시험.xlsx", &[s]).unwrap();
    let (sheet, _) = sheet_and_strings(&path, 1);

    assert!(!sheet.contains("<autoFilter"), "표가 아닌 시트에는 필터를 걸지 않는다");
    assert!(sheet.contains(r#"ySplit="1""#), "머리글 고정은 그대로 한다");
    std::fs::remove_dir_all(&dir).ok();
}

// ------------------------------------------------------------
//  시트 여러 개
// ------------------------------------------------------------

#[test]
fn 표가_여러_개면_시트로_나눈다() {
    let dir = tmp_dir("sheets");
    let a = Sheet::new("요약", vec![col("항목", 20.0), col("값", 12.0)]).no_filter();
    let b = sample();
    let mut c = Sheet::new("날짜별", vec![col("날짜", 13.0)]);
    c.push(vec![Cell::Date("2026-09-07".into())]);

    let path = write_book(&dir, "시험.xlsx", &[a, b, c]).unwrap();
    let book = part(&path, "xl/workbook.xml");
    for want in ["요약", "교사별 계산", "날짜별"] {
        assert!(book.contains(want), "시트 '{want}': {book}");
    }
    let inside = names(&path);
    for n in 1..=3 {
        assert!(
            inside.iter().any(|x| x == &format!("xl/worksheets/sheet{n}.xml")),
            "시트 {n}"
        );
    }
    std::fs::remove_dir_all(&dir).ok();
}

// ------------------------------------------------------------
//  이름과 자잘한 것
// ------------------------------------------------------------

#[test]
fn 파일_이름은_확장자만_xlsx_다() {
    let n = safe_name("보결현황");
    assert!(n.starts_with("보결현황-"), "{n}");
    assert!(n.ends_with(".xlsx"), "{n}");
    assert!(!n.contains(".csv"));
    // 보결현황-YYYYMMDD-HHMMSS.xlsx
    assert_eq!(n.chars().count(), 4 + 1 + 15 + 5, "{n}");
}

#[test]
fn 시트_이름에_쓸_수_없는_글자를_바꾼다() {
    let dir = tmp_dir("name");
    // Excel 은 : \ / ? * [ ] 를 막고 31자를 넘길 수 없다
    let s = Sheet::new("현황: 9/1~9/30 [원본]", vec![col("가", 10.0)]).no_filter();
    let long = Sheet::new(&"가".repeat(50), vec![col("나", 10.0)]).no_filter();
    let path = write_book(&dir, "시험.xlsx", &[s, long]).unwrap();

    let book = part(&path, "xl/workbook.xml");
    assert!(!book.contains("현황:"), "쓸 수 없는 글자가 남아 있다: {book}");
    assert!(book.contains("현황  9 1~9 30  원본"), "{book}");
    assert!(book.contains(&"가".repeat(31)));
    assert!(!book.contains(&"가".repeat(32)), "31자로 자른다");

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn 자료가_없어도_머리글은_남는다() {
    let dir = tmp_dir("empty");
    let s = Sheet::new("교사별", vec![col("교사명", 14.0), col("지급액", 16.0)]);
    let path = write_book(&dir, "시험.xlsx", &[s]).unwrap();
    let (sheet, strings) = sheet_and_strings(&path, 1);

    assert!(strings.contains("교사명") && strings.contains("지급액"));
    assert!(sheet.contains(r#"ySplit="1""#));
    // 자료가 없으면 필터 범위는 머리글 한 줄
    assert!(sheet.contains(r#"<autoFilter ref="A1:B1""#), "{sheet}");

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn 없는_값은_빈_칸으로_둔다() {
    assert_eq!(Cell::opt_text(None::<String>), Cell::Blank);
    assert_eq!(Cell::opt_text(Some("가")), Cell::Text("가".into()));
    assert_eq!(Cell::opt_count(None), Cell::Blank);
    assert_eq!(Cell::opt_count(Some(3)), Cell::Count(3));
}

#[test]
fn 이상한_날짜는_글자로_남기고_실패하지_않는다() {
    let dir = tmp_dir("baddate");
    let mut s = Sheet::new("t", vec![col("날짜", 13.0)]);
    s.push(vec![Cell::Date("2026-13-99".into())]);
    s.push(vec![Cell::Date("".into())]);
    let path = write_book(&dir, "시험.xlsx", &[s]).unwrap();
    let (_, strings) = sheet_and_strings(&path, 1);
    assert!(strings.contains("2026-13-99"), "내보내기가 자료 때문에 실패하면 안 된다");
    std::fs::remove_dir_all(&dir).ok();
}
