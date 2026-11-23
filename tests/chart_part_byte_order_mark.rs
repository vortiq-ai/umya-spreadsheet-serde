//! A chart part saved with a UTF-8 byte-order mark reads like the same part
//! without one.
//!
//! quick-xml skips the mark without counting it in its buffer positions. The
//! lazy chart caches record byte ranges while parsing and replay them against
//! the part later, so with a mark every range was three bytes off and the read
//! failed the WHOLE workbook ("tag not closed"). Some writers do save XML parts
//! with a mark, and the reader before the lazy caches opened them.

use std::io::{
    Cursor,
    Read,
    Write,
};

const BOM: &[u8] = b"\xEF\xBB\xBF";

fn with_marked_chart_parts(bytes: &[u8]) -> (Vec<u8>, usize) {
    let mut source = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    let mut marked = 0;
    for index in 0..source.len() {
        let mut entry = source.by_index(index).unwrap();
        let name = entry.name().unwrap().to_string();
        let mut data = Vec::new();
        entry.read_to_end(&mut data).unwrap();
        if name.starts_with("xl/charts/chart") && name.ends_with(".xml") && !data.starts_with(BOM) {
            let mut with_mark = BOM.to_vec();
            with_mark.extend_from_slice(&data);
            data = with_mark;
            marked += 1;
        }
        output.start_file(name, options).unwrap();
        output.write_all(&data).unwrap();
    }
    (output.finish().unwrap().into_inner(), marked)
}

fn chart_counts(book: &umya_spreadsheet::Workbook) -> Vec<usize> {
    book.sheet_collection()
        .iter()
        .map(|sheet| sheet.chart_collection().len())
        .collect()
}

#[test]
fn a_chart_part_with_a_byte_order_mark_reads_like_one_without() {
    for fixture in ["issue_281.xlsx", "issue_298.xlsx", "aaa.xlsx"] {
        let path = format!("{}/tests/test_files/{fixture}", env!("CARGO_MANIFEST_DIR"));
        let bytes = std::fs::read(&path).unwrap();
        let plain = umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(bytes.clone()), true)
            .unwrap_or_else(|error| panic!("{fixture}: the unmarked file reads: {error}"));
        let (marked_bytes, marked) = with_marked_chart_parts(&bytes);
        assert!(marked > 0, "{fixture}: at least one chart part was marked");
        let marked_book =
            umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(marked_bytes), true)
                .unwrap_or_else(|error| panic!("{fixture}: a marked chart part reads: {error}"));
        assert_eq!(
            chart_counts(&marked_book),
            chart_counts(&plain),
            "{fixture}"
        );
        for (marked_sheet, plain_sheet) in marked_book
            .sheet_collection()
            .iter()
            .zip(plain.sheet_collection())
        {
            for (marked_chart, plain_chart) in marked_sheet
                .chart_collection()
                .iter()
                .zip(plain_sheet.chart_collection())
            {
                assert_eq!(
                    serde_json::to_value(marked_chart).unwrap(),
                    serde_json::to_value(plain_chart).unwrap(),
                    "{fixture}: the marked chart reads to the same model"
                );
            }
        }
    }
}

fn chart_models(book: &umya_spreadsheet::Workbook) -> Vec<serde_json::Value> {
    book.sheet_collection()
        .iter()
        .flat_map(|sheet| sheet.chart_collection())
        .map(|chart| serde_json::to_value(chart).unwrap())
        .collect()
}

/// The same marked parts read lazily, one sheet at a time (`read_reader(..,
/// false)` then `try_read_sheet`). That path keeps each chart part for its
/// cache replay, and kept it WITH the mark while the ranges were recorded
/// without it: every sheet with a chart failed to read.
#[test]
fn a_chart_part_with_a_byte_order_mark_reads_lazily_like_one_without() {
    for fixture in ["issue_281.xlsx", "issue_298.xlsx", "aaa.xlsx"] {
        let path = format!("{}/tests/test_files/{fixture}", env!("CARGO_MANIFEST_DIR"));
        let bytes = std::fs::read(&path).unwrap();
        let (marked_bytes, marked) = with_marked_chart_parts(&bytes);
        assert!(marked > 0, "{fixture}: at least one chart part was marked");
        let read_one_at_a_time = |bytes: Vec<u8>| {
            let mut book = umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(bytes), false)
                .unwrap_or_else(|error| panic!("{fixture}: the lazy read: {error}"));
            for index in 0..book.sheet_collection_no_check().len() {
                book.try_read_sheet(index)
                    .unwrap_or_else(|error| panic!("{fixture}: sheet {index} reads: {error}"));
            }
            book
        };
        let plain = read_one_at_a_time(bytes);
        let marked_book = read_one_at_a_time(marked_bytes);
        assert_eq!(
            chart_counts(&marked_book),
            chart_counts(&plain),
            "{fixture}"
        );
        assert_eq!(
            chart_models(&marked_book),
            chart_models(&plain),
            "{fixture}: the marked charts read to the same models"
        );
    }
}

/// The failing shape itself: a chart on the first sheet naming a later sheet's
/// cells, read lazily with only the chart's sheet built. The named sheet is
/// still raw, so the chart's caches are replayed against the chart part the
/// lazy provider kept, and with a byte-order mark every replayed range was
/// three bytes early ("tag not closed").
#[test]
fn a_marked_chart_naming_a_raw_sheet_reads_lazily() {
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
    let (marked_bytes, marked) = with_marked_chart_parts(&bytes);
    assert!(marked > 0, "the chart part was marked");

    let chart_sheet_alone = |bytes: Vec<u8>| -> Result<serde_json::Value, String> {
        let mut lazy = umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(bytes), false)
            .map_err(|error| error.to_string())?;
        lazy.try_read_sheet(0).map_err(|error| error.to_string())?;
        Ok(serde_json::to_value(lazy.sheet_collection_no_check()[0].chart_collection()).unwrap())
    };
    let plain = chart_sheet_alone(bytes).expect("the unmarked chart reads lazily");
    let marked =
        chart_sheet_alone(marked_bytes).expect("a marked chart naming a raw sheet reads lazily");
    assert_eq!(marked, plain);
}
