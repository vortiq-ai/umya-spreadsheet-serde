use std::io::{
    Cursor,
    Read,
};

use umya_spreadsheet::{
    FilterColumn,
    Table,
    TableColumn,
    Workbook,
    reader,
    writer,
};
use zip::ZipArchive;

fn write(book: &Workbook) -> Vec<u8> {
    let mut out = Vec::new();
    writer::xlsx::write_writer(book, &mut out).unwrap();
    out
}

fn table_xml(bytes: &[u8]) -> String {
    let mut archive = ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut xml = String::new();
    archive
        .by_name("xl/tables/table1.xml")
        .unwrap()
        .read_to_string(&mut xml)
        .unwrap();
    xml
}

fn book_with_table(table: Table) -> Workbook {
    let mut book = umya_spreadsheet::new_file();
    book.sheet_mut(0).unwrap().add_table(table);
    book
}

#[test]
fn new_table_gets_autofilter_over_its_area() {
    let mut table = Table::new("Table1", ("A1", "B3"));
    table.add_column(TableColumn::new("Name"));
    table.add_column(TableColumn::new("Value"));

    let xml = table_xml(&write(&book_with_table(table)));
    assert!(xml.contains(r#"<autoFilter ref="A1:B3"/>"#), "{xml}");
}

#[test]
fn table_without_autofilter_is_written_without_one() {
    let mut table = Table::default();
    table.set_name("Table1");
    table.set_area(("A1", "B3"));
    table.add_column(TableColumn::new("Name"));
    table.add_column(TableColumn::new("Value"));

    let bytes = write(&book_with_table(table));
    assert!(!table_xml(&bytes).contains("<autoFilter"));

    // and it stays that way after reading the file back
    let reread = reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap();
    assert!(!table_xml(&write(&reread)).contains("<autoFilter"));
}

#[test]
fn table_autofilter_survives_roundtrip() {
    let mut table = Table::new("Table1", ("A1", "B3"));
    table.add_column(TableColumn::new("Name"));
    table.add_column(TableColumn::new("Value"));
    table.set_auto_filter("A1:B2");

    let bytes = write(&book_with_table(table));
    let reread = reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap();
    let xml = table_xml(&write(&reread));
    assert!(xml.contains(r#"<autoFilter ref="A1:B2"/>"#), "{xml}");
}

#[test]
fn resizing_a_table_updates_its_area_derived_filter_and_preserves_columns() {
    let mut table = Table::new("Table1", ("A1", "B3"));
    table.add_column(TableColumn::new("Name"));
    table.add_column(TableColumn::new("Value"));
    let mut column = FilterColumn::default();
    column.set_column_id(1).set_hidden_button(true);
    table.auto_filter_mut().add_filter_column(column);

    for end in ["B10", "B20", "B5"] {
        table.set_area(("A1", end));
        let expected = format!("A1:{end}");
        assert_eq!(table.auto_filter().range().range(), expected);
        assert_eq!(table.auto_filter().filter_columns().len(), 1);
        assert_eq!(table.auto_filter().filter_columns()[0].column_id(), 1);
        assert!(table.auto_filter().filter_columns()[0].hidden_button());

        let bytes = write(&book_with_table(table.clone()));
        let xml = table_xml(&bytes);
        assert!(
            xml.contains(&format!(r#"<autoFilter ref="{expected}">"#)),
            "{xml}"
        );
        assert!(
            xml.contains(r#"<filterColumn colId="1" hiddenButton="1"/>"#),
            "{xml}"
        );
        let reread = reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap();
        assert_eq!(table_xml(&write(&reread)), xml);
    }
}

#[test]
fn resizing_a_table_preserves_missing_and_custom_filter_ranges() {
    for custom in [false, true] {
        let mut table = Table::default();
        table.set_name("Table1");
        table.set_display_name("Table1");
        table.set_area(("A1", "B3"));
        table.add_column(TableColumn::new("Name"));
        table.add_column(TableColumn::new("Value"));
        if custom {
            table.set_auto_filter("A1:B2");
            let mut column = FilterColumn::default();
            column.set_column_id(1).set_hidden_button(true);
            table.auto_filter_mut().add_filter_column(column);
        }
        let before = format!("{:?}", table.auto_filter());
        for end in ["B10", "B20"] {
            table.set_area(("A1", end));
            assert_eq!(format!("{:?}", table.auto_filter()), before);
            let bytes = write(&book_with_table(table.clone()));
            let xml = table_xml(&bytes);
            if custom {
                assert!(xml.contains(r#"<autoFilter ref="A1:B2">"#), "{xml}");
                assert!(
                    xml.contains(r#"<filterColumn colId="1" hiddenButton="1"/>"#),
                    "{xml}"
                );
            } else {
                assert!(!xml.contains("<autoFilter"), "{xml}");
            }
            let reread = reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap();
            assert_eq!(table_xml(&write(&reread)), xml);
        }
    }
}
