use std::io::{
    Cursor,
    Read,
};

fn xml(bytes: &[u8]) -> String {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut text = String::new();
    zip.by_name("xl/worksheets/sheet1.xml")
        .unwrap()
        .read_to_string(&mut text)
        .unwrap();
    text
}

#[test]
fn explicit_zero_height_and_false_custom_flag_are_written() {
    let mut book = umya_spreadsheet::new_file();
    book.sheet_mut(0)
        .unwrap()
        .row_dimension_mut(12)
        .set_height(0.0)
        .set_custom_height(false);
    let mut bytes = Vec::new();
    umya_spreadsheet::writer::xlsx::write_writer(&book, &mut bytes).unwrap();
    let xml = xml(&bytes);
    assert!(xml.contains("ht=\"0\""), "{xml}");
    assert!(xml.contains("customHeight=\"0\""), "{xml}");
}

#[test]
fn height_and_flag_presence_survive_read_write_independently() {
    let mut book = umya_spreadsheet::new_file();
    let states = [
        (Some(17.123_456_789_012_3), Some(true)),
        (Some(0.0), Some(false)),
        (Some(15.0), None),
        (None, Some(false)),
        (None, Some(true)),
        (None, None),
    ];
    let sheet = book.sheet_mut(0).unwrap();
    sheet
        .sheet_format_properties_mut()
        .set_default_row_height(18.125);
    for (i, &(height, flag)) in states.iter().enumerate() {
        sheet
            .row_dimension_mut(u32::try_from(i + 1).unwrap())
            .set_height_attributes(height, flag);
    }
    let mut bytes = Vec::new();
    umya_spreadsheet::writer::xlsx::write_writer(&book, &mut bytes).unwrap();
    let raw = xml(&bytes);
    assert!(raw.contains("defaultRowHeight=\"18.125\""));
    assert!(raw.contains("<row r=\"3\" ht=\"15\"/>"), "{raw}");
    assert!(raw.contains("<row r=\"4\" customHeight=\"0\"/>"), "{raw}");
    let back = umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap();
    let sheet = back.sheet(0).unwrap();
    assert_eq!(
        sheet.sheet_format_properties().default_row_height_opt(),
        Some(18.125)
    );
    for (i, expected) in states.iter().enumerate() {
        let row = sheet.row_dimension(u32::try_from(i + 1).unwrap()).unwrap();
        assert_eq!((row.height_opt(), row.custom_height_opt()), *expected);
    }
}

#[test]
fn restoring_absence_clears_previous_authored_height() {
    let mut row = umya_spreadsheet::Row::default();
    row.set_height(22.5);
    assert_eq!(row.custom_height_opt(), Some(true));
    row.set_height_attributes(None, None);
    assert_eq!(row.height_opt(), None);
    assert_eq!(row.custom_height_opt(), None);
    assert_eq!(
        umya_spreadsheet::SheetFormatProperties::default().default_row_height_opt(),
        None
    );
}
