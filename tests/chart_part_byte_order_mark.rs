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
