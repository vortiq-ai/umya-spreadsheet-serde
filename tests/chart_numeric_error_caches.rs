use std::io::{
    Cursor,
    Read,
};

use zip::ZipArchive;

const EXCEL_ERROR_CACHE: &[u8] = include_bytes!("fixtures/excel_numeric_cache_errors.xlsx");
const ERROR_CACHE_FORMULA: &str = "'Error Cache'!$B$2:$B$10";
const PINNED_CHART_FORMULA: &str = "'Chart A'!$D$3:$D$17";
const EXCEL_CACHE_POINTS: [(usize, &str); 9] = [
    (0, "#N/A"),
    (1, "0"),
    (2, "0"),
    (3, "0"),
    (4, "0"),
    (5, "0"),
    (6, "0"),
    (7, "0"),
    (8, "0"),
];

fn read(bytes: &[u8]) -> umya_spreadsheet::Workbook {
    umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap()
}

fn write(book: &umya_spreadsheet::Workbook) -> Vec<u8> {
    let mut bytes = Vec::new();
    umya_spreadsheet::writer::xlsx::write_writer(book, Cursor::new(&mut bytes)).unwrap();
    bytes
}

fn cache_points(bytes: &[u8], formula: &str) -> Vec<(usize, String)> {
    let mut archive = ZipArchive::new(Cursor::new(bytes)).unwrap();
    let formula_tag = format!("<c:f>{formula}</c:f>");
    let mut matches = Vec::new();

    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).unwrap();
        let name = entry.name().unwrap().to_owned();
        if !name.starts_with("xl/charts/chart") || !name.ends_with(".xml") {
            continue;
        }

        let mut xml = String::new();
        entry.read_to_string(&mut xml).unwrap();
        let Some(formula_start) = xml.find(&formula_tag) else {
            continue;
        };
        let formula_end = formula_start + formula_tag.len();
        let cache_start = formula_end + xml[formula_end..].find("<c:numCache>").unwrap();
        let cache_end = cache_start + xml[cache_start..].find("</c:numCache>").unwrap();
        let cache = &xml[cache_start..cache_end];
        let mut points = Vec::new();
        let mut remaining = cache;

        while let Some(point_start) = remaining.find("<c:pt idx=\"") {
            let index_start = point_start + "<c:pt idx=\"".len();
            let index_end = index_start + remaining[index_start..].find('\"').unwrap();
            let index = remaining[index_start..index_end].parse().unwrap();
            let value_start = index_end + remaining[index_end..].find("<c:v>").unwrap();
            let value_start = value_start + "<c:v>".len();
            let value_end = value_start + remaining[value_start..].find("</c:v>").unwrap();
            points.push((index, remaining[value_start..value_end].to_owned()));
            remaining = &remaining[value_end + "</c:v>".len()..];
        }

        matches.push(points);
    }

    assert_eq!(matches.len(), 1, "expected one numeric cache for {formula}");
    matches.pop().unwrap()
}

fn assert_worksheet_errors(book: &umya_spreadsheet::Workbook) {
    let sheet = book.sheet_by_name("Error Cache").unwrap();
    for (address, expected) in [
        ("B2", "#N/A"),
        ("B3", "#VALUE!"),
        ("B4", "#DIV/0!"),
        ("B5", "#REF!"),
        ("B6", "#NAME?"),
        ("B7", "#NUM!"),
        ("B8", "#NULL!"),
        // Excel's worksheet XML caches these dynamic formula results as #VALUE!.
        ("B9", "#VALUE!"),
        ("B10", "#VALUE!"),
    ] {
        assert_eq!(sheet.cell(address).unwrap().value(), expected, "{address}");
    }

    assert_eq!(
        book.sheet_by_name("Chart A")
            .unwrap()
            .cell("D3")
            .unwrap()
            .value(),
        "#N/A",
        "the pinned Chart A first point stays #N/A"
    );
}

#[test]
fn excel_numeric_chart_error_caches_round_trip_exactly() {
    assert_eq!(
        cache_points(EXCEL_ERROR_CACHE, ERROR_CACHE_FORMULA),
        EXCEL_CACHE_POINTS.map(|(index, value)| (index, value.to_owned()))
    );
    assert_eq!(
        cache_points(EXCEL_ERROR_CACHE, PINNED_CHART_FORMULA)[0],
        (0, "#N/A".to_owned())
    );

    let source = read(EXCEL_ERROR_CACHE);
    assert_worksheet_errors(&source);

    let written = write(&source);
    assert_eq!(
        cache_points(&written, ERROR_CACHE_FORMULA),
        EXCEL_CACHE_POINTS.map(|(index, value)| (index, value.to_owned()))
    );
    assert_eq!(
        cache_points(&written, PINNED_CHART_FORMULA)[0],
        (0, "#N/A".to_owned())
    );

    let reread = read(&written);
    assert_worksheet_errors(&reread);
    let rewritten = write(&reread);
    assert_eq!(
        cache_points(&rewritten, ERROR_CACHE_FORMULA),
        EXCEL_CACHE_POINTS.map(|(index, value)| (index, value.to_owned()))
    );
    assert_eq!(
        cache_points(&rewritten, PINNED_CHART_FORMULA)[0],
        (0, "#N/A".to_owned())
    );
}

#[test]
fn engine_only_error_texts_stay_text() {
    let mut book = read(EXCEL_ERROR_CACHE);
    let sheet = book.sheet_by_name_mut("Error Cache").unwrap();
    sheet.cell_mut("B9").set_value("#SPILL!");
    sheet.cell_mut("B10").set_value("#CALC!");

    for book in [&book, &read(&write(&book))] {
        let sheet = book.sheet_by_name("Error Cache").unwrap();
        for (address, text) in [("B9", "#SPILL!"), ("B10", "#CALC!")] {
            let cell = sheet.cell(address).unwrap();
            assert_eq!(cell.value(), text, "{address}");
            assert!(
                matches!(
                    cell.cell_value().raw_value(),
                    umya_spreadsheet::CellRawValue::String(_)
                ),
                "{address} stays text"
            );
        }
    }
}
