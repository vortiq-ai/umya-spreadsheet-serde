//! A worksheet part that declares a non-UTF-8 encoding is read in that
//! encoding. The package is authored here; the expected text is the Unicode
//! text the authored bytes stand for.
use std::io::{
    Cursor,
    Write,
};

use zip::{
    ZipWriter,
    write::SimpleFileOptions,
};

fn package(sheet: &[u8]) -> Vec<u8> {
    let parts: [(&str, &[u8]); 4] = [
        (
            "[Content_Types].xml",
            br#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            br#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#,
        ),
        (
            "xl/workbook.xml",
            br#"<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="Plan1" sheetId="1" r:id="rId1"/></sheets></workbook>"#,
        ),
        (
            "xl/_rels/workbook.xml.rels",
            br#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="sheet1.xml"/></Relationships>"#,
        ),
    ];
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, data) in parts {
        zip.start_file(name, SimpleFileOptions::default()).unwrap();
        zip.write_all(data).unwrap();
    }
    zip.start_file("xl/sheet1.xml", SimpleFileOptions::default())
        .unwrap();
    zip.write_all(sheet).unwrap();
    zip.finish().unwrap().into_inner()
}

#[test]
fn latin1_worksheet_text_is_read_as_unicode() {
    // 0xE9 is "é" and 0xBA is "º" in ISO-8859-1.
    let mut sheet = br#"<?xml version="1.0" encoding="ISO-8859-1" standalone="yes"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData><row r="1"><c r="A1" t="inlineStr"><is><t><![CDATA[S"#.to_vec();
    sheet.push(0xE9);
    sheet.extend_from_slice(b"rie]]></t></is></c><c r=\"B1\" t=\"inlineStr\"><is><t>N");
    sheet.push(0xBA);
    sheet.extend_from_slice(b" Nota</t></is></c></row></sheetData></worksheet>");
    let mut book =
        umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(package(&sheet)), true).unwrap();
    book.try_read_sheet_collection().unwrap();
    let sheet = book.sheet_by_name("Plan1").unwrap();
    assert_eq!(sheet.cell("A1").unwrap().value(), "S\u{e9}rie");
    assert_eq!(sheet.cell("B1").unwrap().value(), "N\u{ba} Nota");
}

#[test]
fn worksheet_declaring_an_encoding_the_parsers_cannot_read_is_an_error() {
    let sheet = br#"<?xml version="1.0" encoding="no-such-encoding"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData/></worksheet>"#;
    let result = umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(package(sheet)), true);
    let message = result.err().expect("must fail").to_string();
    assert!(message.contains("no-such-encoding"), "{message}");
}
