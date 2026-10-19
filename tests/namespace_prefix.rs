use std::io::{
    Cursor,
    Write,
};

use umya_spreadsheet::reader::xlsx;

// Original producer bytes: prefixed SpreadsheetML in every part, absolute
// worksheet relationship targets. Do not rewrite/repack this regression input.
const LOGISTICS: &[u8] = include_bytes!("test_files/logistics_two_lookups.xlsx");

#[test]
fn original_prefixed_logistics_imports_all_sheets_and_cells() {
    assert_eq!(LOGISTICS.len(), 7425);
    for eager in [true, false] {
        let mut book = xlsx::read_reader(Cursor::new(LOGISTICS), eager).unwrap();
        assert_eq!(book.sheet_collection_no_check().len(), 3);
        if !eager {
            let cells = book.lazy_read_sheet_cells(0).unwrap();
            assert_eq!(cells.get("A99").unwrap().value(), "S98");
            assert_eq!(cells.get("D2").unwrap().value_number(), Some(50.0));
        }
        book.read_sheet_collection();
        assert_eq!(book.sheet_collection().len(), 3);
        let sheets = book.sheet_collection();
        for (sheet, (name, rows, cols)) in sheets.iter().zip([
            ("Shipments", 99, 5),
            ("Lane Rates", 4, 3),
            ("Fuel Rates", 3, 2),
        ]) {
            assert_eq!(sheet.name(), name);
            assert_eq!(sheet.highest_row(), rows);
            assert_eq!(sheet.highest_column(), cols);
        }
        let shipments = &sheets[0];
        assert_eq!(shipments.cell("A1").unwrap().value(), "Shipment ID");
        assert_eq!(shipments.cell("A2").unwrap().value(), "S1");
        assert_eq!(shipments.cell("A99").unwrap().value(), "S98");
        assert_eq!(shipments.cell("D2").unwrap().value_number(), Some(50.0));
        assert!(shipments.cell("A1").unwrap().style().font().unwrap().bold());
    }
}

// Build independent small packages with equivalent namespace spellings. The
// real-world fixture above remains byte-for-byte untouched.
fn namespace_workbook(prefix: &str, absolute_target: bool) -> Vec<u8> {
    let ns = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
    let rel = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
    let declaration = if prefix.is_empty() {
        format!("xmlns=\"{ns}\"")
    } else {
        format!("xmlns:{}=\"{ns}\"", prefix.trim_end_matches(':'))
    };
    let rebound = if prefix.is_empty() {
        String::new()
    } else {
        format!(
            r#"<{prefix}sheet xmlns:{}="urn:unrelated"/>"#,
            prefix.trim_end_matches(':')
        )
    };
    let target = if absolute_target {
        "/xl/worksheets/sheet1.xml"
    } else {
        "worksheets/sheet1.xml"
    };
    let parts = [
        (
            "[Content_Types].xml",
            r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"/>"#
                .to_owned(),
        ),
        (
            "xl/workbook.xml",
            format!(
                r#"<{prefix}workbook {declaration} xmlns:link="{rel}"><{prefix}sheets>{rebound}<foreign:sheet xmlns:foreign="urn:unrelated"/><sheet xmlns="urn:unrelated"/><{prefix}sheet name="Data" sheetId="1" link:id="sheet1"></{prefix}sheet></{prefix}sheets></{prefix}workbook>"#
            ),
        ),
        (
            "xl/_rels/workbook.xml.rels",
            format!(
                r#"<pkg:Relationships xmlns:pkg="http://schemas.openxmlformats.org/package/2006/relationships"><pkg:Relationship Id="sheet1" Type="{rel}/worksheet" Target="{target}"/></pkg:Relationships>"#
            ),
        ),
        (
            "xl/sharedStrings.xml",
            format!(
                r#"<{prefix}sst {declaration} count="1" uniqueCount="1"><{prefix}si><{prefix}t xml:space="preserve"> x:literal &amp; text </{prefix}t></{prefix}si></{prefix}sst>"#
            ),
        ),
        (
            "xl/styles.xml",
            format!(
                r#"<{prefix}styleSheet {declaration}><{prefix}fonts count="1"><{prefix}font><{prefix}b/></{prefix}font></{prefix}fonts><{prefix}fills count="1"><{prefix}fill><{prefix}patternFill patternType="none"/></{prefix}fill></{prefix}fills><{prefix}borders count="1"><{prefix}border/></{prefix}borders><{prefix}cellStyleXfs count="1"><{prefix}xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></{prefix}cellStyleXfs><{prefix}cellXfs count="1"><{prefix}xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0"/></{prefix}cellXfs></{prefix}styleSheet>"#
            ),
        ),
        (
            "xl/worksheets/sheet1.xml",
            format!(
                r#"<{prefix}worksheet {declaration}><{prefix}sheetData><row xmlns="urn:unrelated" r="99"><c r="A99"><v>123</v></c></row><foreign:row xmlns:foreign="urn:unrelated" r="99"/><{prefix}row r="1"><{prefix}c r="A1" t="s" s="0"><{prefix}v>0</{prefix}v></{prefix}c><{prefix}c r="B1" t="b"><{prefix}v>1</{prefix}v></{prefix}c><{prefix}c r="C1"><{prefix}f>1+2</{prefix}f><{prefix}v>3</{prefix}v></{prefix}c><{prefix}c r="D1" t="inlineStr"><{prefix}is><{prefix}t>x:unchanged</{prefix}t></{prefix}is></{prefix}c></{prefix}row></{prefix}sheetData><x14:conditionalFormatting xmlns:x14="http://schemas.microsoft.com/office/spreadsheetml/2009/9/main"><x14:cfRule type="dataBar"/></x14:conditionalFormatting><{prefix}mergeCells count="1"><{prefix}mergeCell ref="A2:B2"/></{prefix}mergeCells></{prefix}worksheet>"#
            ),
        ),
    ];
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (path, xml) in parts {
        zip.start_file(path, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(xml.as_bytes()).unwrap();
    }
    zip.finish().unwrap().into_inner()
}

#[test]
fn equivalent_prefixes_preserve_shared_strings_styles_formulas_and_booleans() {
    for prefix in ["", "x:", "arbitrary:", "x14:"] {
        for absolute_target in [false, true] {
            let bytes = namespace_workbook(prefix, absolute_target);
            let book = xlsx::read_reader(Cursor::new(bytes), true).unwrap();
            assert_eq!(book.sheet_collection().len(), 1, "prefix={prefix}");
            let sheet = book.sheet(0).unwrap();
            assert_eq!(sheet.value("A1"), " x:literal & text ");
            assert_eq!(sheet.value("B1"), "TRUE");
            assert_eq!(sheet.value_number("C1"), Some(3.0));
            assert_eq!(sheet.cell("C1").unwrap().formula(), "1+2");
            assert_eq!(sheet.value("D1"), "x:unchanged");
            assert!(sheet.cell("A1").unwrap().style().font().unwrap().bold());
            assert_eq!(sheet.merge_cells().len(), 1);
            assert_eq!(sheet.highest_row(), 1, "foreign rows must be ignored");
        }
    }
}

fn rewrite_workbook(bytes: Vec<u8>, edit: impl FnOnce(String) -> String) -> Vec<u8> {
    use std::io::Read;
    let mut source = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let mut edit = Some(edit);
    for index in 0..source.len() {
        let mut part = source.by_index(index).unwrap();
        let mut data = Vec::new();
        part.read_to_end(&mut data).unwrap();
        if part.name() == "xl/workbook.xml" {
            data = edit.take().unwrap()(String::from_utf8(data).unwrap()).into_bytes();
        }
        output
            .start_file(part.name(), zip::write::SimpleFileOptions::default())
            .unwrap();
        output.write_all(&data).unwrap();
    }
    output.finish().unwrap().into_inner()
}

#[test]
fn relationship_attribute_uses_uri_not_prefix_or_attribute_order() {
    for prefix in ["", "x:"] {
        for before in [true, false] {
            for foreign_prefix in ["ext", "r"] {
                let foreign = format!(
                    r#"xmlns:{foreign_prefix}="urn:extension" {foreign_prefix}:id="unrelated" id="also-unrelated""#
                );
                let valid = r#"link:id="sheet1""#;
                let attrs = if before {
                    format!("{foreign} {valid}")
                } else {
                    format!("{valid} {foreign}")
                };
                let bytes = rewrite_workbook(namespace_workbook(prefix, false), |xml| {
                    xml.replace(valid, &attrs)
                });
                let book = xlsx::read_reader(Cursor::new(bytes), true).unwrap();
                assert_eq!(book.sheet_collection_no_check().len(), 1);
                assert_eq!(book.sheet(0).unwrap().value("A1"), " x:literal & text ");
            }
        }
    }
}

#[test]
fn invalid_workbook_namespaces_return_errors_without_panicking() {
    let rel = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
    let main = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
    for prefix in ["", "x:"] {
        for case in [
            "wrong_relationship_uri",
            "unqualified_id",
            "undeclared_attribute_prefix",
            "wrong_root_uri",
            "no_root_namespace",
            "reserved_xml_prefix",
            "nested_reserved_prefix",
            "duplicate_expanded_attribute",
        ] {
            let bytes = rewrite_workbook(namespace_workbook(prefix, false), |xml| match case {
                "wrong_relationship_uri" => xml.replace(rel, "urn:wrong"),
                "unqualified_id" => xml.replace("link:id=", "id="),
                "undeclared_attribute_prefix" => xml.replace("link:id=", "unknown:id="),
                "wrong_root_uri" => xml.replace(main, "urn:wrong"),
                "no_root_namespace" => xml.replace(main, ""),
                "reserved_xml_prefix" => xml.replace(
                    &format!("<{prefix}workbook "),
                    &format!("<{prefix}workbook xmlns:xml=\"urn:wrong\" "),
                ),
                "nested_reserved_prefix" => xml.replace(
                    &format!("<{prefix}sheets>"),
                    &format!("<{prefix}sheets xmlns:xml=\"urn:wrong\">"),
                ),
                "duplicate_expanded_attribute" => xml.replace(
                    "link:id=\"sheet1\"",
                    &format!("xmlns:other=\"{rel}\" other:id=\"other\" link:id=\"sheet1\""),
                ),
                _ => unreachable!(),
            });
            for eager in [true, false] {
                let result = std::panic::catch_unwind(|| {
                    xlsx::read_reader(Cursor::new(bytes.clone()), eager)
                });
                assert!(result.is_ok(), "panic: {prefix:?} {case} eager={eager}");
                assert!(
                    result.unwrap().is_err(),
                    "accepted: {prefix:?} {case} eager={eager}"
                );
            }
        }
    }
}
