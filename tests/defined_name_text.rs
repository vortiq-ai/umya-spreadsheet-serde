//! A defined name that holds a text constant keeps its quotes through a write
//! and a read: without them it is not a valid formula.
use std::io::{
    Cursor,
    Read,
};

use umya_spreadsheet::{
    self as umya,
    structs::DefinedName,
};

const FILE_TAG: &str = r#""LBO Analysis - Monro VF.xlsx""#;

fn workbook_xml(bytes: &[u8]) -> String {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut text = String::new();
    archive
        .by_name("xl/workbook.xml")
        .unwrap()
        .read_to_string(&mut text)
        .unwrap();
    text
}

#[test]
fn a_text_constant_name_keeps_its_quotes() {
    let mut book = umya::new_file();
    let mut name = DefinedName::default();
    name.set_name("FileTag");
    name.set_address(FILE_TAG);
    book.add_defined_names(name);

    let mut bytes = vec![];
    umya::writer::xlsx::write_writer(&book, &mut bytes).unwrap();
    let xml = workbook_xml(&bytes);
    assert!(
        xml.contains(&format!(
            r#"<definedName name="FileTag">{FILE_TAG}</definedName>"#
        )),
        "{xml}"
    );

    let book = umya::reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap();
    let name = book
        .defined_names()
        .iter()
        .find(|name| name.name() == "FileTag")
        .expect("FileTag");
    assert_eq!(name.address(), FILE_TAG);
}
