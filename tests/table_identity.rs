use std::io::{
    Cursor,
    Read,
    Write,
};

use umya_spreadsheet::{
    Table,
    TableColumn,
    Workbook,
    reader,
    writer,
};

fn write(book: &Workbook) -> Vec<u8> {
    let mut bytes = Vec::new();
    writer::xlsx::write_writer(book, &mut bytes).unwrap();
    bytes
}

fn table(name: &str, area: (&str, &str)) -> Table {
    let mut table = Table::new(name, area);
    for name in ["First", "Second", "Third"] {
        table.add_column(TableColumn::new(name));
    }
    table
}

fn imported(table_id: u32) -> Workbook {
    let mut book = umya_spreadsheet::new_file();
    book.sheet_mut(0)
        .unwrap()
        .add_table(table("Sales", ("A1", "C3")));
    let bytes = write(&book);
    let xml = format!(
        "<table xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" \
         id=\"{table_id}\" name=\"Sales\" displayName=\"Sales\" ref=\"A1:C3\"><tableColumns \
         count=\"3\"><tableColumn id=\"0\" name=\"First\"/><tableColumn id=\"42\" \
         name=\"Second\"><calculatedColumnFormula>1+2</calculatedColumnFormula></\
         tableColumn><tableColumn id=\"7\" name=\"Third\"/></tableColumns></table>"
    );
    let mut source = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for index in 0..source.len() {
        let mut entry = source.by_index(index).unwrap();
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).unwrap();
        if entry.name().unwrap() == "xl/tables/table1.xml" {
            bytes = xml.as_bytes().to_vec();
        }
        output
            .start_file(
                entry.name().unwrap(),
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
        output.write_all(&bytes).unwrap();
    }
    let bytes = output.finish().unwrap().into_inner();
    reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap()
}

fn identities(table: &Table) -> Vec<(&str, Option<u32>)> {
    table
        .columns()
        .iter()
        .map(|column| (column.name(), column.id()))
        .collect()
}

#[test]
fn imported_table_and_sparse_column_identities_survive_reordering() {
    for table_id in [0, 99] {
        let mut book = imported(table_id);
        let table = &mut book.sheet_mut(0).unwrap().tables_mut()[0];
        assert_eq!(table.id(), Some(table_id));
        assert_eq!(
            identities(table),
            [("First", Some(0)), ("Second", Some(42)), ("Third", Some(7))]
        );
        assert_eq!(
            table.columns()[1]
                .calculated_column_formula()
                .map(String::as_str),
            Some("1+2")
        );
        table.columns_mut().swap(0, 2);
        let book = reader::xlsx::read_reader(Cursor::new(write(&book)), true).unwrap();
        let table = &book.sheet(0).unwrap().tables()[0];
        assert_eq!(table.id(), Some(table_id));
        assert_eq!(
            identities(table),
            [("Third", Some(7)), ("Second", Some(42)), ("First", Some(0))]
        );
        assert_eq!(
            table.columns()[1]
                .calculated_column_formula()
                .map(String::as_str),
            Some("1+2")
        );
    }
}

#[test]
fn missing_column_identities_are_allocated_around_all_explicit_identities() {
    let mut book = umya_spreadsheet::new_file();
    let mut table = table("Sales", ("A1", "D3"));
    table.columns_mut()[1].set_id(1);
    table.columns_mut()[2].set_id(3);
    table.add_column(TableColumn::new("Fourth"));
    book.sheet_mut(0).unwrap().add_table(table);
    let book = reader::xlsx::read_reader(Cursor::new(write(&book)), true).unwrap();
    let columns = book.sheet(0).unwrap().tables()[0].columns();
    assert_eq!(columns[1].id(), Some(1));
    assert_eq!(columns[2].id(), Some(3));
    let ids = columns
        .iter()
        .map(|column| column.id().unwrap())
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(ids.len(), columns.len());
}

#[test]
fn duplicate_column_identities_are_reported_instead_of_reassigned() {
    let mut book = umya_spreadsheet::new_file();
    let mut table = table("Sales", ("A1", "C3"));
    table.columns_mut()[0].set_id(9);
    table.columns_mut()[1].set_id(9);
    book.sheet_mut(0).unwrap().add_table(table);
    let error = writer::xlsx::write_writer(&book, &mut Vec::new()).unwrap_err();
    assert!(
        error.to_string().contains("duplicate column identity 9"),
        "{error}"
    );
}

#[test]
fn anonymous_tables_do_not_take_later_explicit_identities() {
    let mut book = umya_spreadsheet::new_file();
    book.sheet_mut(0)
        .unwrap()
        .add_table(table("New", ("A1", "C3")));
    let mut explicit = table("Existing", ("E1", "G3"));
    explicit.set_id(1);
    book.sheet_mut(0).unwrap().add_table(explicit);
    book.new_sheet("Other").unwrap();
    let mut zero = table("Zero", ("A1", "C3"));
    zero.set_id(0);
    book.sheet_mut(1).unwrap().add_table(zero);
    let book = reader::xlsx::read_reader(Cursor::new(write(&book)), true).unwrap();
    let first = book.sheet(0).unwrap().tables();
    assert_eq!(first[1].id(), Some(1));
    assert_ne!(first[0].id(), Some(1));
    assert_ne!(first[0].id(), Some(0));
    assert_eq!(book.sheet(1).unwrap().tables()[0].id(), Some(0));
}

#[cfg(feature = "serde")]
#[test]
fn identity_fields_roundtrip_in_json_and_bincode_and_default_when_absent() {
    let mut table = table("Sales", ("A1", "C3"));
    assert_eq!(table.id(), None);
    assert_eq!(table.columns()[0].id(), None);
    table.set_id(0);
    table.columns_mut()[0].set_id(42);
    table.columns_mut()[1].set_id(0);
    let value = serde_json::to_value(&table).unwrap();
    let json: Table = serde_json::from_value(value.clone()).unwrap();
    let binary: Table = bincode::deserialize(&bincode::serialize(&table).unwrap()).unwrap();
    for decoded in [json, binary] {
        assert_eq!(decoded.id(), Some(0));
        assert_eq!(
            identities(&decoded),
            [("First", Some(42)), ("Second", Some(0)), ("Third", None)]
        );
    }
    let mut legacy = value;
    legacy.as_object_mut().unwrap().remove("id");
    for column in legacy["columns"].as_array_mut().unwrap() {
        column.as_object_mut().unwrap().remove("id");
    }
    let decoded: Table = serde_json::from_value(legacy).unwrap();
    assert_eq!(decoded.id(), None);
    assert!(decoded.columns().iter().all(|column| column.id().is_none()));
}

#[test]
fn a_new_table_does_not_overwrite_a_later_lazy_table_part() {
    let mut source = umya_spreadsheet::new_file();
    source.new_sheet("Existing").unwrap();
    let mut existing = table("ExistingSales", ("A1", "C3"));
    existing.set_id(17);
    existing.columns_mut()[0].set_id(42);
    source.sheet_mut(1).unwrap().add_table(existing);
    let bytes = write(&source);
    let mut book = reader::xlsx::read_reader(Cursor::new(bytes), false).unwrap();
    book.sheet_mut(0)
        .unwrap()
        .add_table(table("NewSales", ("A1", "C3")));
    assert_eq!(
        book.sheet(1).unwrap_err().kind(),
        umya_spreadsheet::XlsxErrorKind::NotDeserialized
    );
    let bytes = write(&book);
    assert_eq!(
        book.sheet(1).unwrap_err().kind(),
        umya_spreadsheet::XlsxErrorKind::NotDeserialized
    );
    let mut archive = zip::ZipArchive::new(Cursor::new(&bytes)).unwrap();
    let mut tables = Vec::new();
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).unwrap();
        let path = entry.name().unwrap().to_string();
        if path.starts_with("xl/tables/") && path.ends_with(".xml") {
            let mut xml = String::new();
            entry.read_to_string(&mut xml).unwrap();
            tables.push((path, xml));
        }
    }
    assert_eq!(tables.len(), 2);
    let preserved = tables
        .iter()
        .find(|(path, _)| path == "xl/tables/table1.xml")
        .unwrap();
    assert!(
        preserved.1.contains("name=\"ExistingSales\""),
        "{}",
        preserved.1
    );
    let created = tables
        .iter()
        .find(|(path, _)| path == "xl/tables/table2.xml")
        .unwrap();
    assert!(created.1.contains("name=\"NewSales\""), "{}", created.1);
    let reread = reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap();
    let new = &reread.sheet(0).unwrap().tables()[0];
    let old = &reread.sheet(1).unwrap().tables()[0];
    assert_eq!(new.name(), "NewSales");
    assert_ne!(new.id(), Some(17));
    assert_eq!(old.name(), "ExistingSales");
    assert_eq!(old.id(), Some(17));
    assert_eq!(old.columns()[0].id(), Some(42));
}
