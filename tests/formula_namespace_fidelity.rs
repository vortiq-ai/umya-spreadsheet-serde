//! Formula namespaces are serialized text, not requests to alias or calculate
//! functions.
use std::io::{
    Cursor,
    Read,
    Write,
};

use umya_spreadsheet::{
    Workbook,
    reader,
    writer,
};
use zip::{
    ZipArchive,
    ZipWriter,
    write::SimpleFileOptions,
};

fn xml_text(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn write_book(book: &Workbook) -> Vec<u8> {
    let mut bytes = Vec::new();
    writer::xlsx::write_writer(book, &mut bytes).expect("write workbook");
    bytes
}

fn read_book(bytes: Vec<u8>) -> Workbook {
    reader::xlsx::read_reader(Cursor::new(bytes), true).expect("read workbook")
}

fn worksheet_xml(bytes: &[u8]) -> String {
    let mut archive = ZipArchive::new(Cursor::new(bytes)).expect("open workbook archive");
    let mut xml = String::new();
    archive
        .by_name("xl/worksheets/sheet1.xml")
        .expect("first worksheet part")
        .read_to_string(&mut xml)
        .expect("read worksheet XML");
    xml
}

// The skeleton supplies package metadata only. Formula input is authored
// directly in OOXML so a writer-side normalization cannot conceal a
// corresponding reader defect.
fn raw_formula_fixture(formulas_and_errors: &[(&str, &str)]) -> Vec<u8> {
    let mut skeleton = umya_spreadsheet::new_file();
    for name in [
        "_xludf.rri",
        "_xlfn.RRI",
        "_xlfn.RRI and O'Brien",
        "Rates 2026",
    ] {
        skeleton.new_sheet(name).expect("add referenced sheet");
    }

    let mut sheet_xml = String::from(concat!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
        r#"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">"#,
        "<sheetData>",
    ));
    for (index, (formula, error)) in formulas_and_errors.iter().enumerate() {
        let row = index + 1;
        sheet_xml.push_str(&format!(
            r#"<row r="{row}"><c r="A{row}" t="e"><f>{}</f><v>{error}</v></c></row>"#,
            xml_text(formula),
        ));
    }
    sheet_xml.push_str("</sheetData></worksheet>");

    let mut source =
        ZipArchive::new(Cursor::new(write_book(&skeleton))).expect("open skeleton archive");
    let mut output = ZipWriter::new(Cursor::new(Vec::new()));
    for index in 0..source.len() {
        let mut part = source.by_index(index).expect("skeleton part");
        let mut bytes = Vec::new();
        part.read_to_end(&mut bytes).expect("read skeleton part");
        if part.name() == "xl/worksheets/sheet1.xml" {
            bytes = sheet_xml.as_bytes().to_vec();
        }
        output
            .start_file(part.name(), SimpleFileOptions::default())
            .expect("start fixture part");
        output.write_all(&bytes).expect("write fixture part");
    }
    output
        .finish()
        .expect("finish fixture archive")
        .into_inner()
}

fn assert_formulas_and_errors(book: &Workbook, cases: &[(&str, &str)]) {
    let sheet = book.sheet(0).expect("first worksheet");
    for (index, (formula, error)) in cases.iter().enumerate() {
        let address = format!("A{}", index + 1);
        let cell = sheet.cell(address.as_str()).expect("formula cell");
        assert!(cell.is_formula(), "{address}");
        assert_eq!(cell.formula(), *formula, "{address}: formula text");
        assert_eq!(cell.data_type(), "e", "{address}: cached error type");
        assert!(
            cell.cell_value().is_error(),
            "{address}: cached error value"
        );
        assert_eq!(cell.value(), *error, "{address}: cached error text");
        assert_eq!(cell.value_number(), None, "{address}: no evaluation");
    }
}

fn assert_raw_formulas_and_errors(bytes: &[u8], cases: &[(&str, &str)]) {
    let xml = worksheet_xml(bytes);
    for (index, (formula, error)) in cases.iter().enumerate() {
        let address = format!("A{}", index + 1);
        let expected = format!(
            r#"<c r="{address}" t="e"><f>{}</f><v>{error}</v></c>"#,
            xml_text(formula),
        );
        assert!(
            xml.contains(&expected),
            "{address}: expected {expected}\n{xml}"
        );
    }
}

#[test]
fn raw_formula_namespaces_and_literal_prefixes_survive_read_write_read() {
    let cases = [
        ("_xludf.rri(3,100,200)", "#NAME?"),
        ("_xlfn.RRI(3,100,200)", "#NUM!"),
        ("RRI(3,100,200)", "#VALUE!"),
        ("_XlUdF.rRi($A$1,A$2,$A3)", "#DIV/0!"),
        ("_XlFn.RrI(Sheet1!$B$2,Sheet1!B$3,Sheet1!$B4)", "#REF!"),
        ("_xludf.SUM(A1:A3)", "#NAME?"),
        ("_xludf.IF(A1,A2,A3)", "#NAME?"),
        ("_xludf.vendor.rri(3,100,200)", "#NAME?"),
        (
            "SUM(_xludf.rri(3,100,200),_xlfn.RRI(4,200,300),RRI(5,300,500))",
            "#N/A",
        ),
        (
            "_xludf.rri('Rates 2026'!D$7,'_xludf.rri'!$B$2,'_xlfn.RRI'!$C3)",
            "#NULL!",
        ),
        (
            "_xlfn.RRI(3,'_xlfn.RRI and O''Brien'!$B$2,Sheet1!$C$3)",
            "#NAME?",
        ),
        (
            r#"IF(A1="_xludf.rri","_xlfn.RRI & <RRI>","_xludf.SUM(""quoted"")")"#,
            "#NAME?",
        ),
        (
            r#"IF(A1="_xludf.rri",_xludf.rri(A1,B1,C1),_xlfn.RRI(C1,B1,A1))"#,
            "#NAME?",
        ),
    ];
    let input = raw_formula_fixture(&cases);
    assert_raw_formulas_and_errors(&input, &cases);

    let book = read_book(input);
    assert_formulas_and_errors(&book, &cases);

    let output = write_book(&book);
    assert_raw_formulas_and_errors(&output, &cases);
    assert_formulas_and_errors(&read_book(output), &cases);
}

#[test]
fn explicit_rri_replacement_changes_only_selected_formula_without_calculation() {
    let cases = [
        ("_xludf.rri(3,100,200)", "#NAME?"),
        ("_xludf.rri(4,200,300)", "#NAME?"),
        ("_xlfn.RRI(3,100,200)", "#NUM!"),
    ];
    let mut book = read_book(raw_formula_fixture(&cases));
    assert_formulas_and_errors(&book, &cases);

    book.sheet_mut(0)
        .expect("first worksheet")
        .cell_mut("A1")
        .set_formula("RRI(3,100,200)");
    let replaced = [("RRI(3,100,200)", "#NAME?"), cases[1], cases[2]];
    assert_formulas_and_errors(&book, &replaced);
    let output = write_book(&book);
    assert_raw_formulas_and_errors(&output, &replaced);
    let mut book = read_book(output);
    assert_formulas_and_errors(&book, &replaced);

    // This is deliberately caller-supplied, not the evaluated result of RRI.
    book.sheet_mut(0)
        .expect("first worksheet")
        .cell_mut("A1")
        .set_formula_result_number(0.25);
    let output = write_book(&book);
    let xml = worksheet_xml(&output);
    assert!(
        xml.contains(r#"<c r="A1"><f>RRI(3,100,200)</f><v>0.25</v></c>"#),
        "{xml}",
    );
    let book = read_book(output);
    let sheet = book.sheet(0).expect("first worksheet");
    let repaired = sheet.cell("A1").expect("replaced cell");
    assert_eq!(repaired.formula(), "RRI(3,100,200)");
    assert_eq!(repaired.data_type(), "n");
    assert_eq!(repaired.value_number(), Some(0.25));
    for (address, (formula, error)) in [("A2", cases[1]), ("A3", cases[2])] {
        let untouched = sheet.cell(address).expect("untouched cell");
        assert_eq!(untouched.formula(), formula, "{address}");
        assert_eq!(untouched.data_type(), "e", "{address}");
        assert_eq!(untouched.value(), error, "{address}");
    }
}
