//! Serializer regressions: cached values are fixture data, not calculated
//! results.
use std::{
    collections::BTreeMap,
    io::{
        Cursor,
        Read,
        Write,
    },
};

use quick_xml::{
    Reader,
    XmlVersion,
    events::Event,
};
use umya_spreadsheet::{
    CellErrorType,
    CellFormulaValues,
    CellRawValue,
    Workbook,
    reader,
    writer,
};
use zip::{
    ZipArchive,
    ZipWriter,
    write::SimpleFileOptions,
};

// Author the input formula metadata directly, so a writer regression cannot
// silently remove the property this read/write/read test is meant to exercise.
fn fixture(sheet_data: &str) -> Vec<u8> {
    let worksheet = format!(
        concat!(
            r#"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">"#,
            "<sheetData>{sheet_data}</sheetData></worksheet>",
        ),
        sheet_data = sheet_data,
    );
    let parts = [
        (
            "[Content_Types].xml",
            concat!(
                r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">"#,
                r#"<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>"#,
                r#"<Default Extension="xml" ContentType="application/xml"/>"#,
                r#"<Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>"#,
                r#"<Override PartName="/xl/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"/>"#,
                r#"<Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>"#,
                r#"<Override PartName="/xl/worksheets/sheet2.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>"#,
                "</Types>",
            ),
        ),
        (
            "_rels/.rels",
            concat!(
                r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#,
                r#"<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/>"#,
                "</Relationships>",
            ),
        ),
        (
            "xl/workbook.xml",
            concat!(
                r#"<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">"#,
                r#"<sheets><sheet name="Sheet1" sheetId="1" r:id="rId1"/>"#,
                r#"<sheet name="Input sheet" sheetId="2" r:id="rId2"/></sheets></workbook>"#,
            ),
        ),
        (
            "xl/_rels/workbook.xml.rels",
            concat!(
                r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#,
                r#"<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/>"#,
                r#"<Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet2.xml"/>"#,
                r#"<Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>"#,
                "</Relationships>",
            ),
        ),
        (
            "xl/styles.xml",
            concat!(
                r#"<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">"#,
                r#"<fonts count="2"><font><sz val="11"/><name val="Calibri"/></font>"#,
                r#"<font><b/><sz val="11"/><name val="Calibri"/></font></fonts>"#,
                r#"<fills count="2"><fill><patternFill patternType="none"/></fill>"#,
                r#"<fill><patternFill patternType="gray125"/></fill></fills>"#,
                r#"<borders count="1"><border><left/><right/><top/><bottom/><diagonal/></border></borders>"#,
                r#"<cellStyleXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellStyleXfs>"#,
                r#"<cellXfs count="2"><xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0"/>"#,
                r#"<xf numFmtId="0" fontId="1" fillId="0" borderId="0" xfId="0" applyFont="1"/></cellXfs>"#,
                r#"<cellStyles count="1"><cellStyle name="Normal" xfId="0" builtinId="0"/></cellStyles>"#,
                "</styleSheet>",
            ),
        ),
        ("xl/worksheets/sheet1.xml", worksheet.as_str()),
        (
            "xl/worksheets/sheet2.xml",
            concat!(
                r#"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">"#,
                r#"<sheetData><row r="3"><c r="C3"><v>31</v></c></row></sheetData></worksheet>"#,
            ),
        ),
    ];
    let mut archive = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, contents) in parts {
        archive
            .start_file(name, SimpleFileOptions::default())
            .unwrap();
        archive.write_all(contents.as_bytes()).unwrap();
    }
    archive.finish().unwrap().into_inner()
}

fn write(book: &Workbook) -> Vec<u8> {
    let mut output = Vec::new();
    writer::xlsx::write_writer(book, &mut output).unwrap();
    output
}

fn read(bytes: &[u8]) -> Workbook {
    reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap()
}

#[derive(Debug, Default, PartialEq)]
struct XmlCell {
    kind:           Option<String>,
    metadata_index: Option<String>,
    formula:        Option<(BTreeMap<String, String>, String)>,
    value:          Option<String>,
}

fn xml_cell(bytes: &[u8], coordinate: &str) -> XmlCell {
    let mut archive = ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut xml = String::new();
    archive
        .by_name("xl/worksheets/sheet1.xml")
        .unwrap()
        .read_to_string(&mut xml)
        .unwrap();
    let mut reader = Reader::from_str(&xml);
    let mut found = false;
    let mut cell = XmlCell::default();
    loop {
        match reader.read_event().unwrap() {
            Event::Start(event) if event.name().as_ref() == b"c" => {
                let attributes: BTreeMap<_, _> = event
                    .attributes()
                    .map(|attribute| {
                        let attribute = attribute.unwrap();
                        (
                            String::from_utf8(attribute.key.as_ref().to_vec()).unwrap(),
                            attribute
                                .normalized_value(XmlVersion::Implicit1_0)
                                .unwrap()
                                .into_owned(),
                        )
                    })
                    .collect();
                found = attributes.get("r").map(String::as_str) == Some(coordinate);
                if found {
                    cell.kind = attributes.get("t").cloned();
                    cell.metadata_index = attributes.get("cm").cloned();
                }
            }
            Event::Start(event) if found && event.name().as_ref() == b"f" => {
                let attributes = event
                    .attributes()
                    .map(|attribute| {
                        let attribute = attribute.unwrap();
                        (
                            String::from_utf8(attribute.key.as_ref().to_vec()).unwrap(),
                            attribute
                                .normalized_value(XmlVersion::Implicit1_0)
                                .unwrap()
                                .into_owned(),
                        )
                    })
                    .collect();
                let text = reader.read_text(event.name()).unwrap();
                let decoded = text.decode().unwrap();
                cell.formula = Some((
                    attributes,
                    quick_xml::escape::unescape(&decoded).unwrap().into_owned(),
                ));
            }
            Event::Empty(event) if found && event.name().as_ref() == b"f" => {
                let attributes = event
                    .attributes()
                    .map(|attribute| {
                        let attribute = attribute.unwrap();
                        (
                            String::from_utf8(attribute.key.as_ref().to_vec()).unwrap(),
                            attribute
                                .normalized_value(XmlVersion::Implicit1_0)
                                .unwrap()
                                .into_owned(),
                        )
                    })
                    .collect();
                cell.formula = Some((attributes, String::new()));
            }
            Event::Start(event) if found && event.name().as_ref() == b"v" => {
                let text = reader.read_text(event.name()).unwrap();
                let decoded = text.decode().unwrap();
                cell.value = Some(quick_xml::escape::unescape(&decoded).unwrap().into_owned());
            }
            Event::End(event) if found && event.name().as_ref() == b"c" => return cell,
            Event::Eof => panic!("cell {coordinate} missing from worksheet: {xml}"),
            _ => {}
        }
    }
}

fn assert_xml_formula(bytes: &[u8], coordinate: &str, attributes: &[(&str, &str)], text: &str) {
    let expected = attributes
        .iter()
        .map(|&(key, value)| (key.to_string(), value.to_string()))
        .collect();
    assert_eq!(
        xml_cell(bytes, coordinate).formula,
        Some((expected, text.to_string())),
        "formula metadata at {coordinate}",
    );
}

#[test]
fn shared_namespaces_and_mixed_references_survive_read_write_read() {
    let master = concat!(
        "_xludf.rri(A1,$B1,C$2,$D$3,'Input sheet'!E4,'Input sheet'!$F$5,",
        r#""_xludf.rri A1")+_xlfn.RRI(2,3,4)"#,
    );
    let mut bytes = fixture(&format!(
        concat!(
            r#"<row r="4"><c r="D4" t="e"><f t="shared" si="7" ref="D4:E5">{master}</f><v>#NAME?</v></c>"#,
            r#"<c r="E4" t="e"><f t="shared" si="7"/><v>#NAME?</v></c></row>"#,
            r#"<row r="5"><c r="D5" t="e"><f t="shared" si="7"/><v>#NAME?</v></c>"#,
            r#"<c r="E5" t="e"><f t="shared" si="7"/><v>#NAME?</v></c></row>"#,
        ),
        master = quick_xml::escape::escape(master),
    ));
    let expected = [
        ("D4", master.to_string()),
        (
            "E4",
            concat!(
                "_xludf.rri(B1,$B1,D$2,$D$3,'Input sheet'!F4,'Input sheet'!$F$5,",
                r#""_xludf.rri A1")+_xlfn.RRI(2,3,4)"#,
            )
            .to_string(),
        ),
        (
            "D5",
            concat!(
                "_xludf.rri(A2,$B2,C$2,$D$3,'Input sheet'!E5,'Input sheet'!$F$5,",
                r#""_xludf.rri A1")+_xlfn.RRI(2,3,4)"#,
            )
            .to_string(),
        ),
        (
            "E5",
            concat!(
                "_xludf.rri(B2,$B2,D$2,$D$3,'Input sheet'!F5,'Input sheet'!$F$5,",
                r#""_xludf.rri A1")+_xlfn.RRI(2,3,4)"#,
            )
            .to_string(),
        ),
    ];

    for _ in 0..3 {
        let book = read(&bytes);
        let sheet = book.sheet(0).unwrap();
        for (coordinate, formula) in &expected {
            let cell = sheet.cell(*coordinate).unwrap();
            assert_eq!(cell.formula(), formula, "{coordinate}");
            assert_eq!(cell.formula_shared_index(), Some(7));
            assert_eq!(cell.raw_value(), &CellRawValue::Error(CellErrorType::Name));
        }
        bytes = write(&book);
        assert_xml_formula(
            &bytes,
            "D4",
            &[("t", "shared"), ("si", "7"), ("ref", "D4:E5")],
            master,
        );
        for coordinate in ["E4", "D5", "E5"] {
            assert_xml_formula(&bytes, coordinate, &[("t", "shared"), ("si", "7")], "");
        }
    }
}

#[test]
fn fixed_array_bounds_namespaces_cached_members_and_sentinels_survive_read_write_read() {
    let formula = "_xludf.rri(A2:A4,1,2)+_xlfn.RRI(A2:A4,3,4)";
    let mut bytes = fixture(&format!(
        concat!(
            r#"<row r="2"><c r="A2" t="inlineStr"><is><t>left sentinel</t></is></c>"#,
            r#"<c r="B2" t="e"><f t="array" ref="B2:C4">{formula}</f><v>#NAME?</v></c>"#,
            r#"<c r="C2"><v>17</v></c><c r="D2" s="1"><v>4242</v></c></row>"#,
            r#"<row r="3"><c r="B3"><v>-8.5</v></c><c r="C3" t="e"><v>#DIV/0!</v></c></row>"#,
            r#"<row r="4"><c r="B4" t="b"><v>1</v></c>"#,
            r#"<c r="C4" t="inlineStr"><is><t>0017</t></is></c></row>"#,
            r#"<row r="5"><c r="B5" t="inlineStr"><is><t>below sentinel</t></is></c></row>"#,
        ),
        formula = quick_xml::escape::escape(formula),
    ));

    for _ in 0..3 {
        let book = read(&bytes);
        let sheet = book.sheet(0).unwrap();
        let anchor = sheet.cell("B2").unwrap();
        assert_eq!(anchor.formula(), formula);
        assert_eq!(
            anchor.raw_value(),
            &CellRawValue::Error(CellErrorType::Name)
        );
        let owner = anchor.formula_obj().unwrap();
        assert_eq!(owner.formula_type(), &CellFormulaValues::Array);
        assert_eq!(owner.reference(), "B2:C4");
        assert_eq!(
            sheet.cell("C2").unwrap().raw_value(),
            &CellRawValue::Numeric(17.0)
        );
        assert_eq!(
            sheet.cell("B3").unwrap().raw_value(),
            &CellRawValue::Numeric(-8.5)
        );
        assert_eq!(
            sheet.cell("C3").unwrap().raw_value(),
            &CellRawValue::Error(CellErrorType::Div0),
        );
        assert_eq!(
            sheet.cell("B4").unwrap().raw_value(),
            &CellRawValue::Bool(true)
        );
        assert!(matches!(
            sheet.cell("C4").unwrap().raw_value(),
            CellRawValue::String(value) if value.as_ref() == "0017"
        ));
        for coordinate in ["C2", "B3", "C3", "B4", "C4"] {
            assert!(sheet.cell(coordinate).unwrap().formula_obj().is_none());
        }
        assert_eq!(sheet.value("A2"), "left sentinel");
        assert_eq!(sheet.value("D2"), "4242");
        assert!(sheet.style("D2").font().unwrap().bold());
        assert_eq!(sheet.value("B5"), "below sentinel");

        bytes = write(&book);
        assert_xml_formula(&bytes, "B2", &[("t", "array"), ("ref", "B2:C4")], formula);
        let anchor = xml_cell(&bytes, "B2");
        assert_eq!(anchor.kind.as_deref(), Some("e"));
        assert_eq!(anchor.value.as_deref(), Some("#NAME?"));
        let error_member = xml_cell(&bytes, "C3");
        assert_eq!(error_member.kind.as_deref(), Some("e"));
        assert_eq!(error_member.value.as_deref(), Some("#DIV/0!"));
        assert!(error_member.formula.is_none());
    }
}

#[test]
fn shared_namespaces_survive_public_row_and_column_insertion() {
    let formula = concat!(
        "_xludf.rri(A1,Sheet1!B2,'Input sheet'!C3)+_xlfn.RRI(2,3,4)",
        r#"+"_xludf.rri A1""#,
    );
    let bytes = fixture(&format!(
        concat!(
            r#"<row r="4"><c r="D4" t="e"><f t="shared" si="12" ref="D4:E4">{formula}</f><v>#NAME?</v></c>"#,
            r#"<c r="E4" t="e"><f t="shared" si="12"/><v>#NAME?</v></c></row>"#,
        ),
        formula = quick_xml::escape::escape(formula),
    ));
    let mut book = read(&bytes);
    book.insert_new_row("Sheet1", 1, 1);
    book.insert_new_column("Sheet1", "A", 1);
    let master = concat!(
        "_xludf.rri(B2,Sheet1!C3,'Input sheet'!C3)+_xlfn.RRI(2,3,4)",
        r#"+"_xludf.rri A1""#,
    );
    let sibling = concat!(
        "_xludf.rri(C2,Sheet1!D3,'Input sheet'!D3)+_xlfn.RRI(2,3,4)",
        r#"+"_xludf.rri A1""#,
    );

    for _ in 0..2 {
        let sheet = book.sheet(0).unwrap();
        assert_eq!(sheet.cell("E5").unwrap().formula(), master);
        assert_eq!(sheet.cell("F5").unwrap().formula(), sibling);
        assert_eq!(book.sheet_by_name("Input sheet").unwrap().value("C3"), "31");
        let bytes = write(&book);
        assert_xml_formula(
            &bytes,
            "E5",
            &[("t", "shared"), ("si", "12"), ("ref", "E5:F5")],
            master,
        );
        assert_xml_formula(&bytes, "F5", &[("t", "shared"), ("si", "12")], "");
        book = read(&bytes);
    }
}

#[test]
fn fixed_array_bounds_follow_public_row_and_column_insertion() {
    let formula = "_xludf.rri(A2:A4,1,2)+_xlfn.RRI(A2:A4,3,4)";
    let bytes = fixture(&format!(
        concat!(
            r#"<row r="2"><c r="B2" t="e"><f t="array" ref="B2:C4">{formula}</f><v>#NAME?</v></c>"#,
            r#"<c r="C2"><v>17</v></c><c r="D2" s="1"><v>4242</v></c></row>"#,
            r#"<row r="3"><c r="B3"><v>-8.5</v></c><c r="C3" t="e"><v>#DIV/0!</v></c></row>"#,
            r#"<row r="4"><c r="B4" t="b"><v>1</v></c>"#,
            r#"<c r="C4" t="inlineStr"><is><t>0017</t></is></c></row>"#,
        ),
        formula = quick_xml::escape::escape(formula),
    ));
    let mut book = read(&bytes);
    book.insert_new_row("Sheet1", 1, 2);
    book.insert_new_column("Sheet1", "A", 1);
    let moved_formula = "_xludf.rri(B4:B6,1,2)+_xlfn.RRI(B4:B6,3,4)";

    for _ in 0..2 {
        let sheet = book.sheet(0).unwrap();
        let anchor = sheet.cell("C4").unwrap();
        assert_eq!(anchor.formula(), moved_formula);
        assert_eq!(
            anchor.raw_value(),
            &CellRawValue::Error(CellErrorType::Name)
        );
        assert_eq!(sheet.value("D4"), "17");
        assert_eq!(sheet.value("C5"), "-8.5");
        assert_eq!(sheet.value("D5"), "#DIV/0!");
        assert_eq!(sheet.value("C6"), "TRUE");
        assert_eq!(sheet.value("D6"), "0017");
        assert_eq!(sheet.value("E4"), "4242");
        assert!(sheet.style("E4").font().unwrap().bold());

        let bytes = write(&book);
        assert_xml_formula(
            &bytes,
            "C4",
            &[("t", "array"), ("ref", "C4:D6")],
            moved_formula,
        );
        book = read(&bytes);
        assert_eq!(
            book.sheet(0)
                .unwrap()
                .cell("C4")
                .unwrap()
                .formula_obj()
                .unwrap()
                .reference(),
            "C4:D6",
        );
    }
}

const STRUCTURAL_ARRAY_FORMULA: &str = "_xludf.rri('Input sheet'!C3,1,2)+_xlfn.RRI(2,3,4)";

fn structural_array_fixture() -> Vec<u8> {
    fixture(&format!(
        concat!(
            r#"<row r="4"><c r="B4" t="inlineStr"><is><t>left sentinel</t></is></c>"#,
            r#"<c r="C4" t="e"><f t="array" ref="C4:E7">{formula}</f><v>#NAME?</v></c>"#,
            r#"<c r="D4"><v>42</v></c><c r="E4"><v>43</v></c>"#,
            r#"<c r="F4" s="1"><v>4400</v></c></row>"#,
            r#"<row r="5"><c r="C5"><v>51</v></c><c r="D5"><v>52</v></c><c r="E5"><v>53</v></c></row>"#,
            r#"<row r="6"><c r="C6"><v>61</v></c><c r="D6"><v>62</v></c><c r="E6"><v>63</v></c></row>"#,
            r#"<row r="7"><c r="C7"><v>71</v></c><c r="D7"><v>72</v></c><c r="E7"><v>73</v></c></row>"#,
            r#"<row r="8"><c r="C8"><v>8800</v></c></row>"#,
        ),
        formula = quick_xml::escape::escape(STRUCTURAL_ARRAY_FORMULA),
    ))
}

fn assert_array(book: &Workbook, coordinate: &str, reference: &str, formula: &str) {
    let anchor = book.sheet(0).unwrap().cell(coordinate).unwrap();
    assert_eq!(anchor.formula(), formula, "formula at {coordinate}");
    assert_eq!(
        anchor.raw_value(),
        &CellRawValue::Error(CellErrorType::Name)
    );
    let owner = anchor.formula_obj().unwrap();
    assert_eq!(owner.formula_type(), &CellFormulaValues::Array);
    assert_eq!(owner.reference(), reference, "array bounds at {coordinate}");
}

fn round_trip_array(book: &Workbook, coordinate: &str, reference: &str, formula: &str) -> Workbook {
    assert_array(book, coordinate, reference, formula);
    let bytes = write(book);
    assert_xml_formula(
        &bytes,
        coordinate,
        &[("t", "array"), ("ref", reference)],
        formula,
    );
    let back = read(&bytes);
    assert_array(&back, coordinate, reference, formula);
    back
}

#[test]
fn fixed_array_moves_when_rows_and_columns_before_it_are_removed() {
    let mut book = read(&structural_array_fixture());
    book.remove_row("Sheet1", 2, 1);
    book.remove_column("Sheet1", "B", 1);
    let back = round_trip_array(&book, "B3", "B3:D6", STRUCTURAL_ARRAY_FORMULA);

    for book in [&book, &back] {
        let sheet = book.sheet(0).unwrap();
        assert_eq!(sheet.value("C3"), "42");
        assert_eq!(sheet.value("D6"), "73");
        assert_eq!(sheet.value("E3"), "4400");
        assert!(sheet.style("E3").font().unwrap().bold());
        assert_eq!(sheet.value("B7"), "8800");
        assert_eq!(book.sheet_by_name("Input sheet").unwrap().value("C3"), "31");
    }
}

#[test]
fn fixed_array_shrinks_for_interior_and_clipped_trailing_deletions() {
    // Large trailing deletions intentionally extend beyond the array. The end
    // must clip to the last surviving member, never subtract into row/column 0.
    let cases = [
        (true, 5, 1, "C4:E6", "C5", "61"),
        (true, 7, 1, "C4:E6", "C6", "61"),
        (true, 6, 100, "C4:E5", "C5", "51"),
        (true, 5, 100, "C4:E4", "E4", "43"),
        (false, 4, 1, "C4:D7", "D4", "43"),
        (false, 5, 1, "C4:D7", "D4", "42"),
        (false, 5, 100, "C4:D7", "D4", "42"),
        (false, 4, 100, "C4:C7", "C7", "71"),
    ];
    for (rows, root, count, reference, survivor, value) in cases {
        let mut book = read(&structural_array_fixture());
        if rows {
            book.remove_row("Sheet1", root, count);
        } else {
            book.remove_column_by_index("Sheet1", root, count);
        }
        let back = round_trip_array(&book, "C4", reference, STRUCTURAL_ARRAY_FORMULA);
        for book in [&book, &back] {
            let sheet = book.sheet(0).unwrap();
            assert_eq!(
                sheet.value(survivor),
                value,
                "rows={rows}, root={root}, count={count}, survivor={survivor}",
            );
            assert!(sheet.cell(survivor).unwrap().formula_obj().is_none());
            assert_eq!(book.sheet_by_name("Input sheet").unwrap().value("C3"), "31");
        }
    }
}

#[test]
fn fixed_array_expands_for_interior_row_and_column_insertions() {
    let mut book = read(&structural_array_fixture());
    book.insert_new_row("Sheet1", 5, 2);
    book.insert_new_column("Sheet1", "D", 1);
    let back = round_trip_array(&book, "C4", "C4:F9", STRUCTURAL_ARRAY_FORMULA);

    for book in [&book, &back] {
        let sheet = book.sheet(0).unwrap();
        assert_eq!(sheet.value("E7"), "52");
        assert_eq!(sheet.value("F9"), "73");
        assert_eq!(sheet.value("G4"), "4400");
        assert!(sheet.style("G4").font().unwrap().bold());
        assert_eq!(sheet.value("C10"), "8800");
        // Inserting space does not ask this serializer to invent cached
        // results.
        assert_eq!(sheet.value("D5"), "");
        assert_eq!(book.sheet_by_name("Input sheet").unwrap().value("C3"), "31");
    }
}

#[test]
fn edits_after_a_fixed_array_leave_its_geometry_and_cached_cells_unchanged() {
    let mut book = read(&structural_array_fixture());
    book.insert_new_row("Sheet1", 20, 2);
    book.insert_new_column("Sheet1", "J", 2);
    book.remove_row("Sheet1", 20, 3);
    book.remove_column("Sheet1", "J", 3);
    let back = round_trip_array(&book, "C4", "C4:E7", STRUCTURAL_ARRAY_FORMULA);

    for book in [&book, &back] {
        let sheet = book.sheet(0).unwrap();
        assert_eq!(sheet.value("B4"), "left sentinel");
        assert_eq!(sheet.value("D4"), "42");
        assert_eq!(sheet.value("E7"), "73");
        assert_eq!(sheet.value("F4"), "4400");
        assert!(sheet.style("F4").font().unwrap().bold());
        assert_eq!(sheet.value("C8"), "8800");
        assert_eq!(book.sheet_by_name("Input sheet").unwrap().value("C3"), "31");
    }
}

#[test]
fn other_sheet_edits_update_formula_refs_without_moving_local_array_geometry() {
    let mut book = read(&structural_array_fixture());
    book.insert_new_row("Input sheet", 1, 2);
    book.insert_new_column("Input sheet", "A", 1);
    let inserted_formula = "_xludf.rri('Input sheet'!D5,1,2)+_xlfn.RRI(2,3,4)";
    book = round_trip_array(&book, "C4", "C4:E7", inserted_formula);
    assert_eq!(book.sheet_by_name("Input sheet").unwrap().value("D5"), "31");

    book.remove_row("Input sheet", 1, 1);
    book.remove_column("Input sheet", "A", 1);
    let removed_formula = "_xludf.rri('Input sheet'!C4,1,2)+_xlfn.RRI(2,3,4)";
    book = round_trip_array(&book, "C4", "C4:E7", removed_formula);
    assert_eq!(book.sheet_by_name("Input sheet").unwrap().value("C4"), "31");
    let sheet = book.sheet(0).unwrap();
    assert_eq!(sheet.value("B4"), "left sentinel");
    assert_eq!(sheet.value("D4"), "42");
    assert_eq!(sheet.value("E7"), "73");
    assert_eq!(sheet.value("F4"), "4400");
    assert!(sheet.style("F4").font().unwrap().bold());
    assert_eq!(sheet.value("C8"), "8800");
}

#[test]
fn deleting_a_fixed_array_anchor_does_not_invent_a_replacement_formula() {
    for rows in [true, false] {
        let mut book = read(&structural_array_fixture());
        if rows {
            book.remove_row("Sheet1", 4, 1);
        } else {
            book.remove_column("Sheet1", "C", 1);
        }
        let bytes = write(&book);
        assert!(xml_cell(&bytes, "C4").formula.is_none());
        let back = read(&bytes);
        for book in [&book, &back] {
            let sheet = book.sheet(0).unwrap();
            assert_eq!(sheet.value("C4"), if rows { "51" } else { "42" });
            assert!(
                sheet
                    .cells()
                    .iter()
                    .all(|cell| cell.formula_obj().is_none())
            );
            assert_eq!(book.sheet_by_name("Input sheet").unwrap().value("C3"), "31");
        }
    }
}

#[test]
fn dynamic_array_metadata_survives_local_geometry_and_other_sheet_edits() {
    let mut book = read(&structural_array_fixture());
    let metadata_index = book.ensure_dynamic_array_metadata();
    assert_ne!(metadata_index, 0);
    book.sheet_mut(0)
        .unwrap()
        .cell_mut("C4")
        .set_cell_meta_index(metadata_index);
    // Start the edit sequence from an actual metadata-bearing XLSX, not merely
    // an in-memory cm assignment that could conceal a metadata writer failure.
    book = read(&write(&book));

    for stage in 0..4 {
        let (coordinate, reference, formula, first_member, last_member, source) = match stage {
            0 => ("C4", "C4:E7", STRUCTURAL_ARRAY_FORMULA, "D4", "E7", "C3"),
            1 => {
                book.insert_new_row("Sheet1", 2, 1);
                book.insert_new_column("Sheet1", "B", 1);
                ("D5", "D5:F8", STRUCTURAL_ARRAY_FORMULA, "E5", "F8", "C3")
            }
            2 => {
                book.remove_row("Sheet1", 2, 1);
                book.remove_column("Sheet1", "B", 1);
                ("C4", "C4:E7", STRUCTURAL_ARRAY_FORMULA, "D4", "E7", "C3")
            }
            _ => {
                book.insert_new_row("Input sheet", 1, 1);
                book.insert_new_column("Input sheet", "A", 1);
                (
                    "C4",
                    "C4:E7",
                    "_xludf.rri('Input sheet'!D4,1,2)+_xlfn.RRI(2,3,4)",
                    "D4",
                    "E7",
                    "D4",
                )
            }
        };
        let bytes = write(&book);
        assert_xml_formula(
            &bytes,
            coordinate,
            &[("t", "array"), ("ref", reference)],
            formula,
        );
        assert_eq!(
            xml_cell(&bytes, coordinate).metadata_index,
            Some(metadata_index.to_string()),
            "raw dynamic anchor cm at stage {stage}",
        );
        let back = read(&bytes);
        for book in [&book, &back] {
            assert_array(book, coordinate, reference, formula);
            let sheet = book.sheet(0).unwrap();
            let anchor = sheet.cell(coordinate).unwrap();
            assert_eq!(anchor.cell_meta_index(), metadata_index);
            assert!(book.is_dynamic_array_metadata(anchor.cell_meta_index()));
            for (member, value) in [(first_member, 42.0), (last_member, 73.0)] {
                let cell = sheet.cell(member).unwrap();
                assert_eq!(cell.raw_value(), &CellRawValue::Numeric(value));
                assert!(cell.formula_obj().is_none());
                assert!(!book.is_dynamic_array_metadata(cell.cell_meta_index()));
            }
            assert_eq!(
                book.sheet_by_name("Input sheet").unwrap().value(source),
                "31"
            );
        }
        book = back;
    }
}
