//! A chart's caches are written point by point, from the cells a reference
//! reads or from the labels a file stored. Each cache kind keeps its own
//! escaping: number and string caches escape quotes, multi-level levels leave
//! them as they are. A point without a value is a start and an end tag.
use std::io::Cursor;

use umya_spreadsheet::{
    RichText,
    TextElement,
    Workbook,
    Worksheet,
    reader,
};

use super::{
    conversion::{
        chart_part,
        replaced_between,
    },
    fixture::{
        base_chart,
        replace_chart_part,
        write_workbook,
    },
    measured::part_xml,
};

/// Three clustered-column series of six points, whose series are replaced.
const COLUMNS: &str = "before/conversions2/bare/colClustered.chart.xml";

/// A series of `COLUMNS`'s bar group with the given category and value
/// references.
fn series(index: usize, categories: &str, values: &str) -> String {
    format!(
        concat!(
            r#"<c:ser><c:idx val="{index}"/><c:order val="{index}"/>"#,
            r#"<c:invertIfNegative val="0"/><c:cat>{categories}</c:cat>"#,
            "<c:val>{values}</c:val></c:ser>"
        ),
        index = index,
        categories = categories,
        values = values
    )
}

/// `COLUMNS` with `series` in place of its own.
fn columns_with(series: &str) -> String {
    replaced_between(
        &part_xml(COLUMNS),
        "<c:ser>",
        "<c:dLbls>",
        &format!("{series}<c:dLbls>"),
    )
}

/// A number reference to `formula`; its cache keeps only its format code.
fn number_reference(formula: &str) -> String {
    let cache = "<c:numCache><c:formatCode>General</c:formatCode></c:numCache>";
    format!("<c:numRef><c:f>{formula}</c:f>{cache}</c:numRef>")
}

/// `COLUMNS` with three series: the first reads text cells through a string
/// reference without a cache and every number form through a number
/// reference; the second has stored multi-level labels; the third has a
/// stored string cache whose first two labels differ from its cells, the
/// second one stored without a value.
fn part() -> String {
    let multi_level = concat!(
        "<c:multiLvlStrRef><c:f>Data!$C$2:$D$4</c:f><c:multiLvlStrCache>",
        r#"<c:ptCount val="3"/><c:lvl><c:pt idx="0"><c:v>&lt;&amp;&gt;&quot;&apos;</c:v></c:pt>"#,
        r#"<c:pt idx="1"><c:v>Zürich ✓</c:v></c:pt><c:pt idx="2"><c:v></c:v></c:pt></c:lvl>"#,
        r#"<c:lvl><c:pt idx="0"><c:v>G&quot;1&apos;</c:v></c:pt></c:lvl>"#,
        "</c:multiLvlStrCache></c:multiLvlStrRef>"
    );
    let stored = concat!(
        r#"<c:strRef><c:f>Data!$E$2:$E$4</c:f><c:strCache><c:ptCount val="3"/>"#,
        r#"<c:pt idx="0"><c:v>Stored &lt;0&gt;</c:v></c:pt><c:pt idx="1"/>"#,
        r#"<c:pt idx="2"><c:v>a&quot;b&apos;c</c:v></c:pt></c:strCache></c:strRef>"#
    );
    let all = [
        series(
            0,
            "<c:strRef><c:f>Data!$A$2:$A$5</c:f></c:strRef>",
            &number_reference("Data!$B$2:$B$16"),
        ),
        series(1, multi_level, &number_reference("Data!$B$2:$B$4")),
        series(2, stored, &number_reference("Data!$B$2:$B$4")),
    ]
    .concat();
    columns_with(&all)
}

/// The workbook read from a file whose chart is `part` and whose `Data` sheet
/// holds, at load, the cells `fill` sets.
fn read_with(part: &str, fill: impl FnOnce(&mut Worksheet)) -> Workbook {
    let mut book = umya_spreadsheet::new_file();
    let sheet = book.sheet_mut(0).unwrap();
    sheet.set_name("Data");
    fill(sheet);
    sheet.add_chart(base_chart());
    let file = replace_chart_part(&write_workbook(&book), part);
    reader::xlsx::read_reader(Cursor::new(file), true).unwrap()
}

/// The workbook read from a file whose chart is `part()`, its `Data` sheet
/// holding, at load, the cells of the stored string cache: the first two
/// differ from the stored labels, the third equals its label.
fn workbook() -> Workbook {
    read_with(&part(), |sheet| {
        sheet.cell_mut("E2").set_value_string("Cell 0");
        sheet.cell_mut("E3").set_value_string(r#"Shown "1""#);
        sheet.cell_mut("E4").set_value_string(r#"a"b'c"#);
    })
}

/// The cache element `tag` that follows the reference `<c:f>formula</c:f>`
/// in the written chart part.
fn cache<'a>(xml: &'a str, formula: &str, tag: &str) -> &'a str {
    let reference = xml
        .find(&format!("<c:f>{formula}</c:f>"))
        .unwrap_or_else(|| panic!("{formula} is not in the part"));
    let start = reference + xml[reference..].find(&format!("<{tag}>")).unwrap();
    let close = format!("</{tag}>");
    let end = start + xml[start..].find(&close).unwrap() + close.len();
    &xml[start..end]
}

/// Cache points `0..` holding `values`, a point without a value as a start
/// and an end tag.
fn points(values: &[Option<&str>]) -> String {
    values
        .iter()
        .enumerate()
        .map(|(index, value)| match value {
            Some(value) => format!(r#"<c:pt idx="{index}"><c:v>{value}</c:v></c:pt>"#),
            None => format!(r#"<c:pt idx="{index}"></c:pt>"#),
        })
        .collect()
}

/// Every cache kind writes its points as before: number caches write a
/// worksheet error as `0` and any other value as its text, string caches write
/// the cells' text or the stored labels, all escaped with quotes as entities,
/// and multi-level levels write their stored labels with quotes left as they
/// are. A blank cell and an empty string are an empty value; a stored point
/// without a value is a start and an end tag, never self-closed.
#[test]
fn cache_points_write_as_before() {
    let mut book = workbook();
    let sheet = book.sheet_mut(0).unwrap();
    sheet.cell_mut("A2").set_value_string(r#"<&>"'"#);
    sheet.cell_mut("A3").set_value_string("Zürich ✓");
    sheet.cell_mut("A4").set_value_string("");
    // A5 stays blank.
    sheet.cell_mut("B2").set_value_number(3.0);
    sheet.cell_mut("B3").set_value_number(0.25);
    sheet.cell_mut("B4").set_value_number(1e308);
    // A cell takes a number that is not finite as `#NUM!`.
    sheet.cell_mut("B5").set_value_number(f64::NAN);
    sheet.cell_mut("B6").set_value_number(f64::INFINITY);
    sheet.cell_mut("B7").set_value_number(f64::NEG_INFINITY);
    let errors = [
        "#DIV/0!", "#NAME?", "#NULL!", "#NUM!", "#REF!", "#VALUE!", "#N/A",
    ];
    for (row, error) in (8_u32..).zip(errors) {
        sheet.cell_mut((2, row)).set_error(error);
    }
    // B15 stays blank.
    sheet.cell_mut("B16").set_value_string(r#"q"'<"#);
    let written = chart_part(&book);

    assert_eq!(
        cache(&written, "Data!$A$2:$A$5", "c:strCache"),
        concat!(
            r#"<c:strCache><c:ptCount val="4"/>"#,
            r#"<c:pt idx="0"><c:v>&lt;&amp;&gt;&quot;&apos;</c:v></c:pt>"#,
            r#"<c:pt idx="1"><c:v>Zürich ✓</c:v></c:pt>"#,
            r#"<c:pt idx="2"><c:v></c:v></c:pt><c:pt idx="3"><c:v></c:v></c:pt>"#,
            "</c:strCache>"
        )
    );
    assert_eq!(
        cache(&written, "Data!$B$2:$B$16", "c:numCache"),
        concat!(
            r#"<c:numCache><c:formatCode>General</c:formatCode><c:ptCount val="15"/>"#,
            r#"<c:pt idx="0"><c:v>3</c:v></c:pt><c:pt idx="1"><c:v>0.25</c:v></c:pt>"#,
            r#"<c:pt idx="2"><c:v>1e308</c:v></c:pt><c:pt idx="3"><c:v>0</c:v></c:pt>"#,
            r#"<c:pt idx="4"><c:v>0</c:v></c:pt><c:pt idx="5"><c:v>0</c:v></c:pt>"#,
            r#"<c:pt idx="6"><c:v>0</c:v></c:pt><c:pt idx="7"><c:v>0</c:v></c:pt>"#,
            r#"<c:pt idx="8"><c:v>0</c:v></c:pt><c:pt idx="9"><c:v>0</c:v></c:pt>"#,
            r#"<c:pt idx="10"><c:v>0</c:v></c:pt><c:pt idx="11"><c:v>0</c:v></c:pt>"#,
            r#"<c:pt idx="12"><c:v>#N/A</c:v></c:pt><c:pt idx="13"><c:v></c:v></c:pt>"#,
            r#"<c:pt idx="14"><c:v>q&quot;&apos;&lt;</c:v></c:pt></c:numCache>"#
        )
    );
    assert_eq!(
        cache(&written, "Data!$C$2:$D$4", "c:multiLvlStrCache"),
        concat!(
            r#"<c:multiLvlStrCache><c:ptCount val="3"/><c:lvl>"#,
            r#"<c:pt idx="0"><c:v>&lt;&amp;&gt;"'</c:v></c:pt>"#,
            r#"<c:pt idx="1"><c:v>Zürich ✓</c:v></c:pt><c:pt idx="2"><c:v></c:v></c:pt>"#,
            r#"</c:lvl><c:lvl><c:pt idx="0"><c:v>G"1'</c:v></c:pt></c:lvl>"#,
            "</c:multiLvlStrCache>"
        )
    );
    assert_eq!(
        cache(&written, "Data!$E$2:$E$4", "c:strCache"),
        concat!(
            r#"<c:strCache><c:ptCount val="3"/>"#,
            r#"<c:pt idx="0"><c:v>Stored &lt;0&gt;</c:v></c:pt><c:pt idx="1"></c:pt>"#,
            r#"<c:pt idx="2"><c:v>a&quot;b&apos;c</c:v></c:pt></c:strCache>"#
        )
    );
}

/// A number is written as its shortest decimal text: never with an exponent
/// below 1e308, and with one from 1e308 on, where Excel refuses the 309-digit
/// decimal. A boolean keeps its text, and an error Excel saves as zero is `0`.
#[test]
fn numeric_cache_points_write_every_number_form() {
    let numbers = [
        0.0,
        -0.0,
        3.0,
        0.1 + 0.2,
        1e21,
        1e-7,
        5e-324,
        1.5e307,
        1e308,
        -1e308,
        f64::MAX,
        // A cell takes a number that is not finite as `#NUM!`.
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
    ];
    let part = columns_with(&series(
        0,
        "<c:strRef><c:f>Data!$A$2</c:f></c:strRef>",
        &number_reference("Data!$B$2:$B$17"),
    ));
    let mut book = read_with(&part, |_| {});
    let sheet = book.sheet_mut(0).unwrap();
    for (row, number) in (2_u32..).zip(numbers) {
        sheet.cell_mut((2, row)).set_value_number(number);
    }
    sheet.cell_mut("B16").set_value_bool(true);
    sheet.cell_mut("B17").set_error("#DIV/0!");

    let zeros = |count| "0".repeat(count);
    let texts = [
        "0".to_owned(),
        "-0".to_owned(),
        "3".to_owned(),
        "0.30000000000000004".to_owned(),
        format!("1{}", zeros(21)),
        "0.0000001".to_owned(),
        format!("0.{}5", zeros(323)),
        format!("15{}", zeros(306)),
        "1e308".to_owned(),
        "-1e308".to_owned(),
        "1.7976931348623157e308".to_owned(),
        "0".to_owned(),
        "0".to_owned(),
        "0".to_owned(),
        "TRUE".to_owned(),
        "0".to_owned(),
    ];
    let values: Vec<_> = texts.iter().map(|text| Some(text.as_str())).collect();
    assert_eq!(
        cache(&chart_part(&book), "Data!$B$2:$B$17", "c:numCache"),
        format!(
            r#"<c:numCache><c:formatCode>General</c:formatCode><c:ptCount val="16"/>{}</c:numCache>"#,
            points(&values)
        )
    );
}

/// The cells of `Data!$A$2:$A$14`: text with every markup character,
/// non-ASCII text, an empty string, rich text of two runs, a boolean, General
/// numbers, numbers formatted `@`, `0.00` and as a date, a blank cell, and a
/// number whose format shows only literal text with quotes.
fn fill_text_cells(sheet: &mut Worksheet) {
    sheet.cell_mut("A2").set_value_string(r#"a<b>&"c'"#);
    sheet.cell_mut("A3").set_value_string("Zürich ✓");
    sheet.cell_mut("A4").set_value_string("");
    let mut rich = RichText::default();
    for text in [r#""x<"#, "&y'"] {
        let mut run = TextElement::default();
        run.set_text(text);
        rich.add_rich_text_elements(run);
    }
    sheet.cell_mut("A5").set_rich_text(rich);
    sheet.cell_mut("A6").set_value_bool(true);
    sheet.cell_mut("A7").set_value_number(3.0);
    sheet.cell_mut("A8").set_value_number(-0.0);
    sheet.cell_mut("A9").set_value_number(0.1 + 0.2);
    for (cell, number, code) in [
        ("A10", 3.0, "@"),
        ("A11", 3.0, "0.00"),
        ("A12", 45000.0, "yyyy-mm-dd"),
        // No digit placeholder: an escaped quote, the quoted text `Q1'` and an
        // escaped quote, whatever the number.
        ("A14", 3.0, r#"\""Q1'"\""#),
    ] {
        sheet.cell_mut(cell).set_value_number(number);
        sheet
            .cell_mut(cell)
            .style_mut()
            .number_format_mut()
            .set_format_code(code);
    }
    // A13 stays blank.
}

/// Each text point is its cell's display text, escaped with quotes as
/// entities, also where a number format writes quotes; rich text is its runs
/// together, in order. A cache umya generates
/// and a stored cache write the same points, except where the stored cache
/// keeps a label the cells cannot reproduce: a label for the rich text, and a
/// point stored without a value, written as a start and an end tag.
#[test]
fn text_cache_points_write_each_cell_display() {
    let displays = [
        Some("a&lt;b&gt;&amp;&quot;c&apos;"),
        Some("Zürich ✓"),
        Some(""),
        Some("&quot;x&lt;&amp;y&apos;"),
        Some("TRUE"),
        Some("3"),
        Some("-0"),
        Some("0.30000000000000004"),
        Some("3"),
        Some("3.00"),
        Some("2023-03-15"),
        Some(""),
        Some("&quot;Q1&apos;&quot;"),
    ];
    let mut labels = displays;
    labels[1] = None;
    labels[3] = Some("Stored &quot;rich&apos; &lt;&gt;");
    let formula = "<c:f>Data!$A$2:$A$14</c:f>";
    let stored = format!(
        r#"<c:strRef>{formula}<c:strCache><c:ptCount val="13"/>{}</c:strCache></c:strRef>"#,
        points(&labels).replace(r#"<c:pt idx="1"></c:pt>"#, r#"<c:pt idx="1"/>"#)
    );
    for (reference, expected) in [
        (format!("<c:strRef>{formula}</c:strRef>"), points(&displays)),
        (stored, points(&labels)),
    ] {
        let part = columns_with(&series(0, &reference, &number_reference("Data!$B$2")));
        let mut book = read_with(&part, fill_text_cells);
        // Set again after the read, so that each cell holds what is set here.
        fill_text_cells(book.sheet_mut(0).unwrap());
        assert_eq!(
            cache(&chart_part(&book), "Data!$A$2:$A$14", "c:strCache"),
            format!(r#"<c:strCache><c:ptCount val="13"/>{expected}</c:strCache>"#),
            "{reference}"
        );
    }
}
