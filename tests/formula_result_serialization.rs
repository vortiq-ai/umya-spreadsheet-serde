use std::io::Read as _;

use umya_spreadsheet::{
    CellErrorType,
    new_file,
    writer,
};

#[test]
fn formula_cached_results_are_written_with_their_explicit_types() {
    let mut book = new_file();
    let sheet = book.sheet_by_name_mut("Sheet1").expect("Sheet1");

    sheet
        .cell_mut("A1")
        .set_formula("1+1")
        .set_formula_result_number(2);
    sheet
        .cell_mut("B1")
        .set_formula("1")
        .set_formula_result_string("01");
    sheet
        .cell_mut("C1")
        .set_formula("1=1")
        .set_formula_result_bool(true);
    sheet
        .cell_mut("D1")
        .set_formula("1/0")
        .set_formula_result_error(CellErrorType::Div0);
    sheet
        .cell_mut("E1")
        .set_formula("1=0")
        .set_formula_result_bool(false);

    let mut bytes = Vec::new();
    writer::xlsx::write_writer(&book, &mut bytes).expect("write workbook");
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).expect("open xlsx");
    let mut worksheet_xml = String::new();
    archive
        .by_name("xl/worksheets/sheet1.xml")
        .expect("sheet1.xml")
        .read_to_string(&mut worksheet_xml)
        .expect("read sheet1.xml");

    let sheet_data = worksheet_xml
        .split_once("<sheetData>")
        .and_then(|(_, xml)| xml.split_once("</sheetData>"))
        .map(|(xml, _)| xml)
        .expect("sheetData");
    assert_eq!(
        sheet_data,
        concat!(
            r#"<row r="1" spans="1:5">"#,
            r#"<c r="A1"><f>1+1</f><v>2</v></c>"#,
            r#"<c r="B1" t="str"><f>1</f><v>01</v></c>"#,
            r#"<c r="C1" t="b"><f>1=1</f><v>1</v></c>"#,
            r#"<c r="D1" t="e"><f>1/0</f><v>#DIV/0!</v></c>"#,
            r#"<c r="E1" t="b"><f>1=0</f><v>0</v></c>"#,
            "</row>",
        )
    );
}

#[test]
fn formula_result_default_remains_type_guessing_and_preserves_the_formula() {
    let mut book = new_file();
    let cell = book
        .sheet_by_name_mut("Sheet1")
        .expect("Sheet1")
        .cell_mut("A1");
    cell.set_formula("20+22").set_formula_result_default("42");

    assert!(cell.is_formula());
    assert_eq!(cell.formula(), "20+22");
    assert_eq!(cell.value_number(), Some(42.0));
}
