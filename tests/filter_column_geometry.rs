use std::io::Cursor;

use umya_spreadsheet::{
    AutoFilter,
    Workbook,
    reader,
    writer,
};

fn roundtrip(book: &Workbook) -> Workbook {
    let mut bytes = Vec::new();
    writer::xlsx::write_writer(book, &mut bytes).unwrap();
    reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap()
}

fn filtered_book(range: &str, column_id: u32, value_cell: &str) -> Workbook {
    let mut book = umya_spreadsheet::new_file();
    let sheet = book.sheet_mut(0).unwrap();
    sheet.cell_mut(value_cell).set_value("A");
    sheet.set_auto_filter(range);
    *sheet.auto_filter_mut().unwrap() = AutoFilter::from_xml(
        format!(
            r#"<autoFilter xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" ref="{range}"><filterColumn colId="{column_id}"><filters><filter val="A"/></filters></filterColumn></autoFilter>"#
        ).as_bytes(),
    ).unwrap();
    roundtrip(&book)
}

fn assert_filter(book: &Workbook, range: &str, column_ids: &[u32]) {
    let book = roundtrip(book);
    let filter = book.sheet(0).unwrap().auto_filter().unwrap();
    assert_eq!(filter.range().range(), range);
    assert_eq!(
        filter
            .filter_columns()
            .iter()
            .map(|column| column.column_id())
            .collect::<Vec<_>>(),
        column_ids
    );
    for column in filter.filter_columns() {
        assert!(
            String::from_utf8(column.to_xml().unwrap())
                .unwrap()
                .contains(r#"val="A""#)
        );
    }
}

#[test]
fn inserting_inside_filter_moves_rules_with_their_columns() {
    let mut book = filtered_book("A1:C3", 1, "B2");
    book.insert_new_column("Sheet1", "B", 1);
    assert_eq!(roundtrip(&book).sheet(0).unwrap().value("C2"), "A");
    assert_filter(&book, "A1:D3", &[2]);
}

#[test]
fn inserting_before_filter_keeps_rule_offsets() {
    for column in ["A", "C"] {
        let mut book = filtered_book("C1:E3", 1, "D2");
        book.insert_new_column("Sheet1", column, 2);
        assert_filter(&book, "E1:G3", &[1]);
    }
}

#[test]
fn deleting_before_rule_shifts_its_filter_offset() {
    let mut book = filtered_book("A1:C3", 2, "C2");
    book.remove_column("Sheet1", "A", 1);
    assert_eq!(roundtrip(&book).sheet(0).unwrap().value("B2"), "A");
    assert_filter(&book, "A1:B3", &[1]);
}

#[test]
fn deleting_filtered_column_removes_its_rule() {
    let mut book = filtered_book("A1:C3", 1, "B2");
    book.remove_column("Sheet1", "B", 1);
    assert_filter(&book, "A1:B3", &[]);
}

#[test]
fn deleting_before_filter_keeps_rule_offsets() {
    let mut book = filtered_book("C1:E3", 1, "D2");
    book.remove_column("Sheet1", "A", 1);
    assert_filter(&book, "B1:D3", &[1]);
}

#[test]
fn deleting_across_filter_start_preserves_surviving_rule_identity() {
    let mut book = filtered_book("C1:F3", 3, "F2");
    book.remove_column("Sheet1", "B", 3);
    assert_filter(&book, "B1:C3", &[1]);
}

#[test]
fn row_edits_preserve_filter_column_offsets() {
    let mut book = filtered_book("A1:C3", 1, "B2");
    book.insert_new_row("Sheet1", 2, 2);
    assert_filter(&book, "A1:C5", &[1]);
    book.remove_row("Sheet1", 2, 2);
    assert_filter(&book, "A1:C3", &[1]);
}
