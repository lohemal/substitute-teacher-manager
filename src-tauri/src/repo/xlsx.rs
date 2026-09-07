//! 엑셀 파일(XLSX) 만들기. **내보내기 세 화면이 모두 이 한 곳을 쓴다.**
//!
//! ## 왜 CSV를 그만두었나
//!
//! CSV는 글자만 담을 수 있어서, 금액과 횟수도 결국 글자였다. Excel이 알아서
//! 숫자로 읽어 주기를 바라야 했고(`15,000` 은 글자가 된다), 한글이 깨지지
//! 않게 파일 앞에 BOM 세 바이트를 붙여야 했고, 표 네 개를 한 파일에 넣으려면
//! 빈 줄로 나눠 놓는 수밖에 없었다.
//!
//! XLSX는 그 세 가지가 형식 안에서 해결된다.
//!   - **숫자는 숫자로** 저장된다. 그대로 합계를 낼 수 있다
//!   - 글자는 XML(UTF-8)이라 **한글이 깨지지 않는다**. BOM 같은 장치가 없다
//!   - 표가 여러 개면 **시트로 나눈다**
//!
//! ## 모든 시트가 같은 모양이다
//!
//! 어느 파일을 열어도 1행이 굵은 머리글이고, 그 행이 **고정**되어 있어
//! 스크롤해도 열 이름이 보인다. 자료가 있는 시트에는 **자동필터**가 걸려
//! 있어 바로 걸러 볼 수 있다. 열 너비는 만드는 쪽에서 정한다.
//!
//! ## 서식은 여기 한 곳에만 있다
//!
//! 금액 서식(`#,##0"원"`)이나 날짜 서식(`yyyy-mm-dd`)을 화면마다 적어 두면
//! 반드시 어긋난다. 그래서 `Cell` 의 종류가 서식을 정하고, 만드는 쪽은
//! **무슨 값인지만** 말한다.

use std::path::{Path, PathBuf};

use rust_xlsxwriter::{
    Color, Format, FormatAlign, FormatBorder, Workbook, Worksheet, XlsxError,
};

use crate::error::{AppError, AppResult};

fn to_app_err(e: XlsxError) -> AppError {
    AppError::internal("엑셀 파일을 만들지 못했습니다.").detail(e.to_string())
}

// ============================================================
//  칸 하나
// ============================================================

/// 칸에 들어갈 값. **종류가 서식을 정한다.**
#[derive(Debug, Clone, PartialEq)]
pub enum Cell {
    /// 글자
    Text(String),
    /// 횟수·개수 — 숫자로 저장하고 천 단위 쉼표를 붙인다
    Count(i64),
    /// 금액(원) — 숫자로 저장하고 `#,##0"원"` 으로 보여 준다
    Money(i64),
    /// 날짜 `YYYY-MM-DD` — 실제 날짜로 저장한다 (해석 못 하면 글자로 남긴다)
    Date(String),
    /// `YYYY-MM-DD HH:MM:SS` — 실제 날짜+시각으로 저장한다 (`yyyy-mm-dd hh:mm`)
    DateTime(String),
    /// 자정 기준 분 — 실제 시각으로 저장한다 (`hh:mm`)
    Time(i32),
    /// 빈 칸
    Blank,
}

impl Cell {
    pub fn text(v: impl Into<String>) -> Self {
        Cell::Text(v.into())
    }
    /// 없으면 빈 칸으로.
    pub fn opt_text(v: Option<impl Into<String>>) -> Self {
        match v {
            Some(s) => Cell::Text(s.into()),
            None => Cell::Blank,
        }
    }
    pub fn opt_count(v: Option<i64>) -> Self {
        match v {
            Some(n) => Cell::Count(n),
            None => Cell::Blank,
        }
    }
    /// 비어 있으면 빈 칸, 아니면 실제 날짜+시각.
    pub fn opt_datetime(v: Option<String>) -> Self {
        match v.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            Some(s) => Cell::DateTime(s.to_string()),
            None => Cell::Blank,
        }
    }
}

/// 열 하나의 머리글과 너비.
#[derive(Debug, Clone)]
pub struct Column {
    pub title: String,
    /// Excel 열 너비(글자 수 기준). 한글은 두 칸을 차지한다고 보고 넉넉히 준다.
    pub width: f64,
}

pub fn col(title: &str, width: f64) -> Column {
    Column {
        title: title.to_string(),
        width,
    }
}

/// 시트 하나.
#[derive(Debug, Clone)]
pub struct Sheet {
    pub name: String,
    pub columns: Vec<Column>,
    pub rows: Vec<Vec<Cell>>,
    /// 자동필터를 걸까. 표가 아닌 시트(조회 조건 등)에는 걸지 않는다.
    pub filter: bool,
    /// 표 아래에 붙이는 합계 행. 굵게 나온다.
    pub total: Option<Vec<Cell>>,
}

impl Sheet {
    pub fn new(name: &str, columns: Vec<Column>) -> Self {
        Self {
            name: name.to_string(),
            columns,
            rows: Vec::new(),
            filter: true,
            total: None,
        }
    }
    pub fn no_filter(mut self) -> Self {
        self.filter = false;
        self
    }
    pub fn push(&mut self, row: Vec<Cell>) {
        self.rows.push(row);
    }
    pub fn with_total(mut self, row: Vec<Cell>) -> Self {
        self.total = Some(row);
        self
    }
}

// ============================================================
//  서식
// ============================================================

struct Styles {
    header: Format,
    text: Format,
    count: Format,
    money: Format,
    date: Format,
    datetime: Format,
    time: Format,
    total_text: Format,
    total_count: Format,
    total_money: Format,
}

/// 금액은 숫자로 저장하고 보여 줄 때만 쉼표와 '원'을 붙인다.
/// 그래서 Excel에서 그대로 합계를 낼 수 있다.
const MONEY_FMT: &str = "#,##0\"원\"";
const COUNT_FMT: &str = "#,##0";
const DATE_FMT: &str = "yyyy-mm-dd";
const DATETIME_FMT: &str = "yyyy-mm-dd hh:mm";
const TIME_FMT: &str = "hh:mm";

impl Styles {
    fn new() -> Self {
        let header = Format::new()
            .set_bold()
            .set_background_color(Color::RGB(0xF1F5F9))
            .set_border_bottom(FormatBorder::Thin)
            .set_border_bottom_color(Color::RGB(0xCBD5E1))
            .set_align(FormatAlign::Left)
            .set_align(FormatAlign::VerticalCenter);

        let total = || {
            Format::new()
                .set_bold()
                .set_background_color(Color::RGB(0xF8FAFC))
                .set_border_top(FormatBorder::Thin)
                .set_border_top_color(Color::RGB(0x94A3B8))
        };

        Self {
            header,
            text: Format::new(),
            count: Format::new().set_num_format(COUNT_FMT),
            money: Format::new().set_num_format(MONEY_FMT),
            date: Format::new().set_num_format(DATE_FMT),
            datetime: Format::new().set_num_format(DATETIME_FMT),
            time: Format::new().set_num_format(TIME_FMT),
            total_text: total(),
            total_count: total().set_num_format(COUNT_FMT),
            total_money: total().set_num_format(MONEY_FMT),
        }
    }
}

// ============================================================
//  쓰기
// ============================================================

/// 시트 이름에 쓸 수 없는 글자를 바꾼다. Excel은 `: \ / ? * [ ]` 를 막고
/// 31자를 넘길 수 없다.
fn safe_sheet_name(raw: &str) -> String {
    let cleaned: String = raw
        .chars()
        .map(|c| match c {
            ':' | '\\' | '/' | '?' | '*' | '[' | ']' => ' ',
            other => other,
        })
        .collect();
    let trimmed = cleaned.trim();
    let mut out: String = trimmed.chars().take(31).collect();
    if out.is_empty() {
        out.push_str("Sheet");
    }
    out
}

fn write_cell(
    ws: &mut Worksheet,
    r: u32,
    c: u16,
    cell: &Cell,
    st: &Styles,
    total_row: bool,
) -> Result<(), XlsxError> {
    match cell {
        Cell::Blank => {
            if total_row {
                ws.write_blank(r, c, &st.total_text)?;
            }
        }
        Cell::Text(v) => {
            let f = if total_row { &st.total_text } else { &st.text };
            ws.write_string_with_format(r, c, v, f)?;
        }
        Cell::Count(n) => {
            let f = if total_row { &st.total_count } else { &st.count };
            ws.write_number_with_format(r, c, *n as f64, f)?;
        }
        Cell::Money(n) => {
            let f = if total_row { &st.total_money } else { &st.money };
            ws.write_number_with_format(r, c, *n as f64, f)?;
        }
        Cell::Date(s) => {
            // 해석할 수 있으면 실제 날짜로, 아니면 글자로 남긴다 —
            // 내보내기가 자료 때문에 실패하면 안 된다.
            match chrono::NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d") {
                Ok(d) => ws.write_date_with_format(r, c, d, &st.date)?,
                Err(_) => ws.write_string_with_format(r, c, s, &st.text)?,
            };
        }
        Cell::DateTime(s) => {
            // `YYYY-MM-DD HH:MM:SS` 또는 `YYYY-MM-DDTHH:MM:SS`
            let t = s.trim();
            let parsed = chrono::NaiveDateTime::parse_from_str(t, "%Y-%m-%d %H:%M:%S")
                .or_else(|_| chrono::NaiveDateTime::parse_from_str(t, "%Y-%m-%dT%H:%M:%S"))
                .or_else(|_| chrono::NaiveDateTime::parse_from_str(t, "%Y-%m-%d %H:%M"));
            match parsed {
                Ok(dt) => ws.write_datetime_with_format(r, c, dt, &st.datetime)?,
                Err(_) => ws.write_string_with_format(r, c, s, &st.text)?,
            };
        }
        Cell::Time(min) => {
            let m = *min;
            // 24:00 은 실제 시각으로 담으면 00:00 이 되어 버린다. 그때만 글자로.
            if (0..1440).contains(&m) {
                let t = chrono::NaiveTime::from_hms_opt((m / 60) as u32, (m % 60) as u32, 0);
                match t {
                    Some(t) => ws.write_time_with_format(r, c, t, &st.time)?,
                    None => ws.write_string_with_format(r, c, fmt_hm(m), &st.text)?,
                };
            } else {
                ws.write_string_with_format(r, c, fmt_hm(m), &st.text)?;
            }
        }
    }
    Ok(())
}

fn fmt_hm(min: i32) -> String {
    format!("{:02}:{:02}", min / 60, min % 60)
}

fn fill_sheet(ws: &mut Worksheet, sheet: &Sheet, st: &Styles) -> Result<(), XlsxError> {
    ws.set_name(safe_sheet_name(&sheet.name))?;

    // 1행 = 머리글
    for (i, c) in sheet.columns.iter().enumerate() {
        let i = i as u16;
        ws.write_string_with_format(0, i, &c.title, &st.header)?;
        ws.set_column_width(i, c.width)?;
    }

    for (ri, row) in sheet.rows.iter().enumerate() {
        let r = ri as u32 + 1;
        for (ci, cell) in row.iter().enumerate() {
            write_cell(ws, r, ci as u16, cell, st, false)?;
        }
    }

    let mut last_row = sheet.rows.len() as u32;
    if let Some(total) = &sheet.total {
        let r = last_row + 1;
        for (ci, cell) in total.iter().enumerate() {
            write_cell(ws, r, ci as u16, cell, st, true)?;
        }
        last_row = r;
    }

    // 머리글 고정 — 스크롤해도 열 이름이 보인다
    ws.set_freeze_panes(1, 0)?;

    // 자동필터. 합계 행은 필터 범위에 넣지 않는다 (걸러도 늘 보이게).
    let last_col = sheet.columns.len().saturating_sub(1) as u16;
    if sheet.filter && !sheet.columns.is_empty() {
        let filter_last = if sheet.total.is_some() {
            last_row.saturating_sub(1)
        } else {
            last_row
        };
        ws.autofilter(0, 0, filter_last, last_col)?;
    }

    Ok(())
}

/// 시트들을 파일 하나로 쓴다. 자료가 없는 시트도 머리글은 남긴다 —
/// 빈 파일을 받은 사람이 무엇이 비었는지 알 수 있어야 한다.
pub fn write_book(dir: &Path, name: &str, sheets: &[Sheet]) -> AppResult<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(name);

    let st = Styles::new();
    let mut book = Workbook::new();

    if sheets.is_empty() {
        let ws = book.add_worksheet();
        ws.set_name("빈 자료").map_err(to_app_err)?;
    }
    for sheet in sheets {
        let ws = book.add_worksheet();
        fill_sheet(ws, sheet, &st).map_err(to_app_err)?;
    }

    book.save(&path).map_err(to_app_err)?;
    Ok(path)
}

/// `보결현황-20260907-193000.xlsx` 처럼 시각을 붙인 이름.
pub fn safe_name(base: &str) -> String {
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    format!("{base}-{stamp}.xlsx")
}

#[cfg(test)]
#[path = "xlsx_tests.rs"]
mod tests;
