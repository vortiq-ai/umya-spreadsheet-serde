#![cfg(feature = "serde")]

use std::io::Cursor;

use umya_spreadsheet::{
    Workbook,
    reader,
    writer::streaming_writer::StreamingWriter,
};

const INPUT: &[u8] = include_bytes!("fixtures/extended_charts/sheet-deletion/named-source.xlsx");

fn input() -> Workbook {
    reader::xlsx::read_reader(Cursor::new(INPUT), true).unwrap()
}

fn streaming(book: Workbook) -> StreamingWriter<Cursor<Vec<u8>>> {
    StreamingWriter::new(zip::ZipWriter::new(Cursor::new(Vec::new())), book)
}

#[test]
fn a_late_unregistered_source_range_stays_an_explicit_streaming_error() {
    let mut book = input();
    book.sheet_by_name_mut("Sheet1")
        .unwrap()
        .cell_mut("Z100")
        .set_value_number(9876);
    let mut stream = streaming(book);
    let source = stream.take_sheet("Sheet1").unwrap();
    stream.flush_sheet(source).unwrap();
    let mut host = stream.take_sheet("ChartHost").unwrap();
    host.worksheet_drawing_mut()
        .visit_extended_spaces_mut(&mut |space| {
            space.visit_formulas_mut(&mut |formula| {
                if formula.address_str() == "_xlchart.v1.2" {
                    formula.set_address_str("Sheet1!$Z$100");
                }
            });
        });
    let error = stream.flush_sheet(host).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("ChartEx source worksheet is unavailable: Sheet1"),
        "{error}"
    );
    assert!(stream.finish().is_err());
}

#[test]
fn an_unedited_chart_host_cannot_read_a_flushed_source() {
    let mut stream = streaming(input());
    let source = stream.take_sheet("Sheet1").unwrap();
    stream.flush_sheet(source).unwrap();
    let host = stream.take_sheet("ChartHost").unwrap();
    let error = stream.flush_sheet(host).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("ChartEx source worksheet is unavailable: Sheet1")
    );
    assert!(stream.finish().is_err());
}
