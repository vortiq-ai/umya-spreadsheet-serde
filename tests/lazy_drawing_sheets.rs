//! A lazy reader can tell which sheets have a drawing before reading any of
//! them (`Worksheet::has_drawing_part`), and by reading those sheets together,
//! after the others, gets the chart models an eager read gets.
//!
//! The eager read builds every sheet in one session, so each chart's caches are
//! reduced against freshly built cells. Read one sheet at a time, a chart
//! naming a sheet that is still raw keeps its cache's shape (that sheet may be
//! written back raw), and a chart naming a sheet read in an earlier call drops
//! its cache without reading that sheet. Reading the sheets without a drawing
//! first, then every sheet with one in a single `try_read_sheet_collection`,
//! puts each chart on the path the eager read takes wherever the cache matches
//! its cells, and lets a reader drop the first sheets' cells as it goes.

use std::io::Cursor;

fn charts(book: &umya_spreadsheet::Workbook) -> Vec<serde_json::Value> {
    book.sheet_collection()
        .iter()
        .flat_map(|sheet| sheet.chart_collection())
        .map(|chart| serde_json::to_value(chart).unwrap())
        .collect()
}

#[test]
fn sheets_with_a_drawing_are_known_before_they_are_read() {
    for fixture in ["issue_281.xlsx", "issue_298.xlsx", "aaa.xlsx"] {
        let path = format!("{}/tests/test_files/{fixture}", env!("CARGO_MANIFEST_DIR"));
        let bytes = std::fs::read(&path).unwrap();
        let mut lazy =
            umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(bytes), false).unwrap();
        let before: Vec<bool> = lazy
            .sheet_collection_no_check()
            .iter()
            .map(|sheet| sheet.has_drawing_part())
            .collect();
        assert!(
            before.iter().any(|&drawing| drawing),
            "{fixture}: a sheet has a chart"
        );
        lazy.try_read_sheet_collection().unwrap();
        let after: Vec<bool> = lazy
            .sheet_collection()
            .iter()
            .map(|sheet| sheet.has_drawing_object())
            .collect();
        assert_eq!(before, after, "{fixture}: raw answer == read answer");
    }
}

#[test]
fn sheets_with_a_drawing_read_together_last_give_the_eager_charts() {
    for fixture in ["issue_281.xlsx", "issue_298.xlsx", "aaa.xlsx"] {
        let path = format!("{}/tests/test_files/{fixture}", env!("CARGO_MANIFEST_DIR"));
        let bytes = std::fs::read(&path).unwrap();
        let eager =
            umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(bytes.clone()), true).unwrap();

        let mut lazy =
            umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(bytes), false).unwrap();
        let drawing: Vec<bool> = lazy
            .sheet_collection_no_check()
            .iter()
            .map(|sheet| sheet.has_drawing_part())
            .collect();
        for index in (0..drawing.len()).filter(|&index| !drawing[index]) {
            lazy.try_read_sheet(index).unwrap();
            // The reader is done with this sheet's cells: drop them, as an
            // importer does.
            let sheet = lazy.sheet_mut(index).unwrap();
            let cells: Vec<(u32, u32)> = sheet
                .cells()
                .iter()
                .map(|cell| (cell.coordinate().col_num(), cell.coordinate().row_num()))
                .collect();
            for cell in cells {
                sheet.remove_cell(cell);
            }
        }
        lazy.try_read_sheet_collection().unwrap();
        assert_eq!(charts(&lazy), charts(&eager), "{fixture}");
    }
}

/// A workbook whose chart, on the first sheet, names the cells of a second
/// sheet that has no drawing: the shape where reading one sheet at a time and
/// reading the whole book differ.
fn chart_naming_another_sheet() -> Vec<u8> {
    let mut book = umya_spreadsheet::new_file();
    book.new_sheet("Data").unwrap();
    let data = book.sheet_by_name_mut("Data").unwrap();
    for row in 1..=3u32 {
        data.cell_mut((1, row))
            .set_value_string(format!("Label {row}"));
        data.cell_mut((2, row)).set_value_number(row * 10);
    }
    let marker = |coordinate: &str| {
        let mut marker = umya_spreadsheet::drawing::spreadsheet::MarkerType::default();
        marker.set_coordinate(coordinate);
        marker
    };
    let mut chart = umya_spreadsheet::Chart::default();
    chart.new_chart(
        &umya_spreadsheet::ChartType::DoughnutChart,
        marker("E1"),
        marker("M18"),
        vec!["Data!$A$1:$A$3", "Data!$B$1:$B$3"],
    );
    book.sheet_mut(0).unwrap().add_chart(chart);
    let mut bytes = Vec::new();
    umya_spreadsheet::writer::xlsx::write_writer(&book, &mut Cursor::new(&mut bytes)).unwrap();
    bytes
}

fn with_chart_part(bytes: &[u8], edit: impl Fn(&str) -> String) -> Vec<u8> {
    use std::io::{
        Read,
        Write,
    };
    let mut source = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for index in 0..source.len() {
        let mut entry = source.by_index(index).unwrap();
        let name = entry.name().unwrap().to_string();
        let mut data = Vec::new();
        entry.read_to_end(&mut data).unwrap();
        if name == "xl/charts/chart1.xml" {
            data = edit(std::str::from_utf8(&data).unwrap()).into_bytes();
        }
        output.start_file(name, options).unwrap();
        output.write_all(&data).unwrap();
    }
    output.finish().unwrap().into_inner()
}

/// Sheets without a drawing one at a time, each one's cells dropped once read;
/// then every sheet with a drawing in one `try_read_sheet_collection`.
fn read_in_phases(bytes: &[u8]) -> umya_spreadsheet::Workbook {
    let mut book = umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(bytes), false).unwrap();
    let drawing: Vec<bool> = book
        .sheet_collection_no_check()
        .iter()
        .map(|sheet| sheet.has_drawing_part())
        .collect();
    for index in (0..drawing.len()).filter(|&index| !drawing[index]) {
        book.try_read_sheet(index).unwrap();
        let sheet = book.sheet_mut(index).unwrap();
        let cells: Vec<(u32, u32)> = sheet
            .cells()
            .iter()
            .map(|cell| (cell.coordinate().col_num(), cell.coordinate().row_num()))
            .collect();
        for cell in cells {
            sheet.remove_cell(cell);
        }
    }
    book.try_read_sheet_collection().unwrap();
    book
}

#[test]
fn a_chart_naming_a_sheet_without_a_drawing_gets_the_eager_charts_in_phases() {
    let bytes = chart_naming_another_sheet();
    let eager =
        umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(bytes.clone()), true).unwrap();

    // One sheet at a time in file order: the chart's sheet is read while "Data"
    // is still raw.
    let mut one_at_a_time =
        umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(bytes.clone()), false).unwrap();
    for index in 0..one_at_a_time.sheet_collection_no_check().len() {
        one_at_a_time.try_read_sheet(index).unwrap();
    }
    assert_ne!(
        charts(&one_at_a_time),
        charts(&eager),
        "reading one sheet at a time differs from the eager read here, or this test proves nothing"
    );

    assert_eq!(charts(&read_in_phases(&bytes)), charts(&eager));
}

/// The one difference left, stated: a chart whose cached label is not what its
/// cell shows. The eager read keeps that label as an exception; read in phases,
/// the chart names a sheet read in an earlier call, whose cache is dropped, so
/// the chart shows the cell's label.
#[test]
fn a_stale_cached_label_is_kept_by_the_eager_read_and_dropped_in_phases() {
    let stale = with_chart_part(&chart_naming_another_sheet(), |xml| {
        assert!(
            xml.contains("<c:v>Label 2</c:v>"),
            "the chart caches its labels"
        );
        xml.replace("<c:v>Label 2</c:v>", "<c:v>Stale label</c:v>")
    });
    let eager =
        umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(stale.clone()), true).unwrap();
    let eager = serde_json::to_string(&charts(&eager)).unwrap();
    let phases = serde_json::to_string(&charts(&read_in_phases(&stale))).unwrap();
    assert!(
        eager.contains("Stale label"),
        "the eager read keeps the stale label"
    );
    assert!(
        !phases.contains("Stale label"),
        "read in phases, the stale label is dropped"
    );
}

/// Every sheet with a drawing first, together (`try_read_sheets_first`), while
/// the others are still raw; then each other sheet alone, its cells dropped
/// once read.
fn read_drawing_sheets_first(bytes: &[u8]) -> umya_spreadsheet::Workbook {
    let mut book = umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(bytes), false).unwrap();
    let drawing: Vec<usize> = book
        .sheet_collection_no_check()
        .iter()
        .enumerate()
        .filter(|(_, sheet)| sheet.has_drawing_part())
        .map(|(index, _)| index)
        .collect();
    book.try_read_sheets_first(&drawing).unwrap();
    for index in 0..book.sheet_collection_no_check().len() {
        if drawing.contains(&index) {
            continue;
        }
        book.try_read_sheet(index).unwrap();
        let sheet = book.sheet_mut(index).unwrap();
        let cells: Vec<(u32, u32)> = sheet
            .cells()
            .iter()
            .map(|cell| (cell.coordinate().col_num(), cell.coordinate().row_num()))
            .collect();
        for cell in cells {
            sheet.remove_cell(cell);
        }
    }
    book
}

/// Read with the drawing sheets first, a chart naming a raw sheet is reduced as
/// the whole-book read reduces it: the same charts on a clean cache.
#[test]
fn drawing_sheets_read_first_give_the_eager_charts() {
    let bytes = chart_naming_another_sheet();
    let eager =
        umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(bytes.clone()), true).unwrap();
    assert_eq!(charts(&read_drawing_sheets_first(&bytes)), charts(&eager));
    for fixture in ["issue_281.xlsx", "issue_298.xlsx", "aaa.xlsx"] {
        let path = format!("{}/tests/test_files/{fixture}", env!("CARGO_MANIFEST_DIR"));
        let bytes = std::fs::read(&path).unwrap();
        let eager =
            umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(bytes.clone()), true).unwrap();
        assert_eq!(
            charts(&read_drawing_sheets_first(&bytes)),
            charts(&eager),
            "{fixture}"
        );
    }
}

/// And on a stale one: the cached label the cell no longer shows is kept as an
/// exception, as the whole-book read keeps it, where reading the drawing sheets
/// last drops it.
#[test]
fn drawing_sheets_read_first_keep_a_stale_cached_label_as_the_eager_read_does() {
    let stale = with_chart_part(&chart_naming_another_sheet(), |xml| {
        xml.replace("<c:v>Label 2</c:v>", "<c:v>Stale label</c:v>")
    });
    let eager =
        umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(stale.clone()), true).unwrap();
    let first = read_drawing_sheets_first(&stale);
    assert!(
        serde_json::to_string(&charts(&first))
            .unwrap()
            .contains("Stale label")
    );
    assert_eq!(charts(&first), charts(&eager));
}

#[test]
fn reading_sheets_first_refuses_an_index_past_the_last_sheet() {
    let bytes = chart_naming_another_sheet();
    let mut book = umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(bytes), false).unwrap();
    let count = book.sheet_collection_no_check().len();
    assert!(book.try_read_sheets_first(&[count]).is_err());
    assert!(!book.sheet_collection_no_check()[0].has_drawing_object());
}
