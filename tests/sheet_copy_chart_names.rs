#![cfg(feature = "serde")]

use std::io::Cursor;

use umya_spreadsheet::{
    XlsxErrorKind,
    reader,
    writer,
};

#[test]
fn copying_a_sheet_refuses_to_drop_a_generated_name_used_by_a_cell() {
    let input = include_bytes!("fixtures/extended_charts/excel-authored/histogram.xlsx");
    for eager in [false, true] {
        let mut book = reader::xlsx::read_reader(Cursor::new(input), eager).unwrap();
        book.sheet_mut(0)
            .unwrap()
            .cell_mut("A20")
            .set_formula("_xlchart.v1.0");
        let before = format!("{book:?}");
        let mut copy = book.sheet(0).unwrap().clone();
        copy.set_name("Copy");
        let error = book.add_sheet(copy).unwrap_err();
        assert_eq!(error.kind(), XlsxErrorKind::InvalidArgument);
        assert_eq!(
            format!("{book:?}"),
            before,
            "copy refusal must leave every owner unchanged"
        );
        let mut output = Vec::new();
        writer::xlsx::write_writer(&book, &mut output).unwrap();
        let reread = reader::xlsx::read_reader(Cursor::new(output), true).unwrap();
        assert_eq!(
            reread.sheet(0).unwrap().cell("A20").unwrap().formula(),
            "_xlchart.v1.0"
        );
        assert!(
            reread
                .defined_names()
                .iter()
                .chain(
                    reread
                        .sheet_collection()
                        .iter()
                        .flat_map(|sheet| sheet.defined_names())
                )
                .any(|name| name.name() == "_xlchart.v1.0" && name.address() == "Sheet1!$A$2:$B$5")
        );
    }
}
