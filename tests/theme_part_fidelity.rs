//! A workbook's theme part survives a read and write unchanged.
use std::io::{
    Cursor,
    Read,
    Write,
};

use umya_spreadsheet::{
    self as umya,
    structs::drawing::Theme,
};
use zip::write::SimpleFileOptions;

const FIXTURE: &str = "./tests/test_files/aaa_theme.xlsx";
const THEME_PART: &str = "xl/theme/theme1.xml";

fn part(bytes: &[u8], name: &str) -> String {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut text = String::new();
    archive
        .by_name(name)
        .unwrap()
        .read_to_string(&mut text)
        .unwrap();
    text
}

fn save(book: &umya::Workbook) -> Vec<u8> {
    let mut bytes = vec![];
    umya::writer::xlsx::write_writer(book, &mut bytes).unwrap();
    bytes
}

#[test]
fn an_unchanged_theme_is_written_back_as_read() {
    let source = std::fs::read(FIXTURE).unwrap();
    let theme = part(&source, THEME_PART);
    // The model does not hold this, so a regenerated part would lose it.
    assert!(theme.contains("<a:extLst>"));

    let book = umya::reader::xlsx::read(FIXTURE).unwrap();
    assert_eq!(book.theme().source_xml(), Some(theme.as_str()));
    assert_eq!(part(&save(&book), THEME_PART), theme);
}

#[test]
fn a_changed_theme_is_written_from_the_model() {
    let mut book = umya::reader::xlsx::read(FIXTURE).unwrap();
    book.theme_mut().set_name("Renamed");
    assert_eq!(book.theme().source_xml(), None);

    let written = part(&save(&book), THEME_PART);
    assert!(written.contains(r#"name="Renamed""#), "{written}");

    let mut book = umya::reader::xlsx::read(FIXTURE).unwrap();
    book.theme_mut()
        .theme_elements_mut()
        .color_scheme_mut()
        .set_name("Changed");
    assert_eq!(book.theme().source_xml(), None);
    assert!(part(&save(&book), THEME_PART).contains(r#"name="Changed""#));
}

#[test]
fn a_theme_parsed_from_text_keeps_the_text_and_its_colors() {
    let source = std::fs::read(FIXTURE).unwrap();
    let xml = part(&source, THEME_PART);
    let theme = Theme::from_xml(&xml);
    assert_eq!(theme.source_xml(), Some(xml.as_str()));
    assert_eq!(
        theme.theme_elements().color_scheme().accent6().val(),
        "70AD47"
    );

    let mut book = umya::new_file();
    book.set_theme(theme);
    assert_eq!(part(&save(&book), THEME_PART), xml);
}

#[test]
fn a_theme_with_relationships_of_its_own_is_written_from_the_model() {
    // An image fill refers to its image through the theme's relationships part,
    // which is not written back.
    let source = std::fs::read(FIXTURE).unwrap();
    let mut archive = zip::ZipArchive::new(Cursor::new(source)).unwrap();
    let mut with_rels = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for i in 0..archive.len() {
        with_rels
            .raw_copy_file(archive.by_index_raw(i).unwrap())
            .unwrap();
    }
    with_rels
        .start_file(
            "xl/theme/_rels/theme1.xml.rels",
            SimpleFileOptions::default(),
        )
        .unwrap();
    with_rels
        .write_all(
            br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="../media/image1.png"/></Relationships>"#,
        )
        .unwrap();
    let bytes = with_rels.finish().unwrap().into_inner();

    let book = umya::reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap();
    assert_eq!(book.theme().source_xml(), None);
    assert_eq!(
        book.theme().theme_elements().color_scheme().accent6().val(),
        "70AD47"
    );
}
