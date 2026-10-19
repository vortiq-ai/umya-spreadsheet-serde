use std::io::{
    Cursor,
    Write,
};

use umya_spreadsheet::{
    Cell,
    Workbook,
    reader::xlsx,
};

fn package(prefix: &str, cells: &str) -> Vec<u8> {
    let ns = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
    let rel = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
    let declaration = if prefix.is_empty() {
        format!("xmlns=\"{ns}\"")
    } else {
        format!("xmlns:{}=\"{ns}\"", prefix.trim_end_matches(':'))
    };
    let cells = cells
        .replace("<c", &format!("<{prefix}c"))
        .replace("</c", &format!("</{prefix}c"))
        .replace("<v", &format!("<{prefix}v"))
        .replace("</v", &format!("</{prefix}v"))
        .replace("<f", &format!("<{prefix}f"))
        .replace("</f", &format!("</{prefix}f"));
    let parts = [
        (
            "[Content_Types].xml",
            r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"/>"#
                .to_owned(),
        ),
        (
            "xl/workbook.xml",
            format!(
                r#"<workbook xmlns="{ns}" xmlns:r="{rel}"><sheets><sheet name="Data" sheetId="1" r:id="s1"/></sheets></workbook>"#
            ),
        ),
        (
            "xl/_rels/workbook.xml.rels",
            format!(
                r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="s1" Type="{rel}/worksheet" Target="worksheets/sheet1.xml"/></Relationships>"#
            ),
        ),
        (
            "xl/styles.xml",
            format!(
                r#"<styleSheet xmlns="{ns}"><fonts count="1"><font/></fonts><fills count="1"><fill><patternFill patternType="none"/></fill></fills><borders count="1"><border/></borders><cellStyleXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellStyleXfs><cellXfs count="2"><xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0"/><xf numFmtId="14" fontId="0" fillId="0" borderId="0" xfId="0"/></cellXfs></styleSheet>"#
            ),
        ),
        (
            "xl/worksheets/sheet1.xml",
            format!(
                r#"<{prefix}worksheet {declaration}><{prefix}sheetData><{prefix}row r="1">{cells}</{prefix}row></{prefix}sheetData></{prefix}worksheet>"#
            ),
        ),
    ];
    let mut out = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, xml) in parts {
        out.start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        out.write_all(xml.as_bytes()).unwrap();
    }
    out.finish().unwrap().into_inner()
}

fn dates() -> &'static str {
    r#"<c r="A1" t="d" s="1"><v>2025-01-01T00:00:00.000Z</v></c>
    <c r="B1" t="d"><v>2024-02-29</v></c>
    <c r="C1" t="d"><v>2025-01-01T12:30:15.250</v></c>
    <c r="D1" t="d"><v>2025-01-02T00:30:00+02:00</v></c>
    <c r="E1" t="d"><v>06:00:00.500</v></c>
    <c r="F1" t="d"><v>1900-02-28</v></c>
    <c r="G1" t="d"><v>1900-03-01</v></c>
    <c r="H1" t="d"><f>DATE(2025,1,1)</f><v>2025-01-01</v></c>
    <c r="I1" t="str"><v>2025-01-01</v></c>
    <c r="J1" t="n"><v>45658</v></c>
    <c r="K1" t="b"><v>1</v></c>
    <c r="L1" t="d"><v>2025-02-29</v></c>
    <c r="M1" t="d"><v>not a date</v></c>
    <c r="N1" t="d"><v/></c>"#
}

fn verify_cell(cell: &Cell) {
    let address = cell.coordinate().get_coordinate();
    let expected = match address.as_str() {
        "A1" | "H1" => Some(45658.0),
        "B1" => Some(45351.0),
        "C1" => Some(45658.0 + 45015.25 / 86400.0),
        "D1" => Some(45658.0 + 22.5 / 24.0),
        "E1" => Some(0.25 + 0.5 / 86400.0),
        "F1" => Some(59.0),
        "G1" => Some(61.0),
        _ => None,
    };
    if let Some(expected) = expected {
        let actual = cell
            .value_number()
            .unwrap_or_else(|| panic!("{address}: date became {:?}", cell.raw_value()));
        assert!(
            (actual - expected).abs() < 1e-9,
            "{address}: {actual} != {expected}"
        );
        assert!(
            cell.style().numbering_format().is_some(),
            "{address}: date lost its format"
        );
    }
    match address.as_str() {
        "A1" => assert_eq!(
            cell.style().numbering_format().unwrap().format_code(),
            "m/d/yyyy",
            "an authored date format must not be replaced"
        ),
        "B1" | "F1" | "G1" | "H1" => assert_eq!(
            cell.style().numbering_format().unwrap().format_code(),
            "yyyy-mm-dd"
        ),
        "C1" | "D1" => assert_eq!(
            cell.style().numbering_format().unwrap().format_code(),
            "yyyy-mm-dd hh:mm:ss.000"
        ),
        "E1" => assert_eq!(
            cell.style().numbering_format().unwrap().format_code(),
            "hh:mm:ss.000"
        ),
        _ => (),
    }
    match address.as_str() {
        "H1" => assert_eq!(cell.formula(), "DATE(2025,1,1)"),
        "I1" => {
            assert_eq!(cell.data_type(), "s");
            assert_eq!(cell.value(), "2025-01-01");
        }
        "J1" => {
            assert_eq!(cell.value_number(), Some(45658.0));
            assert!(
                cell.style()
                    .numbering_format()
                    .is_none_or(|f| f.format_code() == "General")
            );
        }
        "K1" => assert_eq!(cell.data_type(), "b"),
        "L1" => assert_eq!(cell.value(), "2025-02-29"),
        "M1" => assert_eq!(cell.value(), "not a date"),
        "N1" => assert!(cell.value().is_empty()),
        _ => (),
    }
}

fn verify_book(book: &Workbook, last_column: u32) {
    for cell in book.sheet(0).unwrap().cells() {
        verify_cell(cell);
    }
    assert_eq!(book.sheet(0).unwrap().highest_column(), last_column);
    for column in 'A'..='M' {
        assert!(book.sheet(0).unwrap().cell(format!("{column}1")).is_some());
    }
}

#[test]
fn explicit_dates_preserved_in_eager_and_lazy_readers() {
    for prefix in ["", "x:", "alternate:"] {
        let bytes = package(prefix, dates());
        verify_book(&xlsx::read_reader(Cursor::new(&bytes), true).unwrap(), 14);
        let mut lazy = xlsx::read_reader(Cursor::new(&bytes), false).unwrap();
        let cells = lazy.lazy_read_sheet_cells(0).unwrap();
        for cell in cells.collection() {
            verify_cell(cell);
        }
        lazy.read_sheet_collection();
        verify_book(&lazy, 14);
    }
}

#[test]
fn explicit_dates_preserved_by_streaming_reader() {
    for prefix in ["", "x:"] {
        let path = std::env::temp_dir().join(format!(
            "iso-date-stream-{}-{}.xlsx",
            std::process::id(),
            prefix.len()
        ));
        std::fs::write(&path, package(prefix, dates())).unwrap();
        let mut count = 0;
        let result = xlsx::read_sheet_by_name_stream(&path, "Data", |cell| {
            count += 1;
            verify_cell(cell);
        });
        std::fs::remove_file(path).unwrap();
        result.unwrap();
        assert_eq!(count, 14);
    }
}

#[test]
fn converted_dates_survive_writer_roundtrip_without_changing_formula_or_text() {
    let mut book = xlsx::read_reader(Cursor::new(package("x:", dates())), true).unwrap();
    let mut out = Cursor::new(Vec::new());
    umya_spreadsheet::writer::xlsx::write_writer(&mut book, &mut out).unwrap();
    // The writer omits the trailing unstyled empty N1, as for any empty cell.
    verify_book(
        &xlsx::read_reader(Cursor::new(out.into_inner()), true).unwrap(),
        13,
    );
}
