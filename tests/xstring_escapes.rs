use std::io::{
    Cursor,
    Read,
    Write,
};

use umya_spreadsheet::{
    reader,
    writer,
};
use zip::ZipArchive;

// Strings in SpreadsheetML are ST_Xstring: characters that XML 1.0 forbids are
// written as _xHHHH_, and text that already reads as such an escape has its
// underscore written as _x005F_.
#[test]
fn shared_strings_escape_forbidden_characters_and_literal_escapes() {
    let values = [
        "a\u{1}b\u{b}c\u{1f}d",
        "_x0041_ stays literal",
        "_x005F_ and _x00",
        "nonchar \u{fffe}",
        "tab\tnew line\n",
    ];
    let mut book = umya_spreadsheet::new_file();
    let sheet = book.sheet_mut(0).unwrap();
    for (i, value) in values.iter().enumerate() {
        sheet.cell_mut((1, i as u32 + 1)).set_value_string(*value);
    }
    let mut bytes = Vec::new();
    writer::xlsx::write_writer(&book, &mut bytes).unwrap();

    let mut archive = ZipArchive::new(Cursor::new(bytes.clone())).unwrap();
    let mut shared_strings = String::new();
    archive
        .by_name("xl/sharedStrings.xml")
        .unwrap()
        .read_to_string(&mut shared_strings)
        .unwrap();
    assert!(
        shared_strings.contains("a_x0001_b_x000B_c_x001F_d"),
        "{shared_strings}"
    );
    assert!(
        shared_strings.contains("_x005F_x0041_ stays literal"),
        "{shared_strings}"
    );
    assert!(
        shared_strings.contains("_x005F_x005F_ and _x00"),
        "{shared_strings}"
    );
    assert!(
        shared_strings.contains("nonchar _xFFFE_"),
        "{shared_strings}"
    );
    assert!(
        shared_strings.contains("tab\tnew line\n"),
        "{shared_strings}"
    );

    let book = reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap();
    let sheet = book.sheet(0).unwrap();
    for (i, value) in values.iter().enumerate() {
        assert_eq!(sheet.value((1, i as u32 + 1)), *value);
    }
}

// Excel writes a control character in a string as _xHHHH_.
#[test]
fn shared_string_escapes_are_read_as_their_characters() {
    let mut book = umya_spreadsheet::new_file();
    book.sheet_mut(0)
        .unwrap()
        .cell_mut("A1")
        .set_value_string("PLACEHOLDER");
    let mut bytes = Vec::new();
    writer::xlsx::write_writer(&book, &mut bytes).unwrap();
    let bytes = replace_in_part(
        &bytes,
        "xl/sharedStrings.xml",
        "PLACEHOLDER",
        "line_x000B_break _x005F_x0041_",
    );

    let book = reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap();
    assert_eq!(book.sheet(0).unwrap().value("A1"), "line\u{b}break _x0041_");
}

const CELL_STRINGS: [(&str, &str); 5] = [
    ("_x0041_", "A"),
    ("literal _x005F_x0041_", "literal _x0041_"),
    ("line_x000B_break", "line\u{b}break"),
    ("amp &amp; _x0026_", "amp & &"),
    ("<![CDATA[_x0041_&amp;]]>", "A&amp;"),
];

fn part_text(bytes: &[u8], part: &str) -> String {
    let mut archive = ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut text = String::new();
    archive
        .by_name(part)
        .unwrap()
        .read_to_string(&mut text)
        .unwrap();
    text
}

fn escaped_cell_workbook() -> Vec<u8> {
    let mut book = umya_spreadsheet::new_file();
    book.sheet_mut(0)
        .unwrap()
        .cell_mut("A1")
        .set_value_string("seed");
    let mut bytes = Vec::new();
    writer::xlsx::write_writer(&book, &mut bytes).unwrap();
    let mut shared = String::from(
        "<sst xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" count=\"5\" \
         uniqueCount=\"5\">",
    );
    let mut rows = String::from("<sheetData>");
    for (index, (encoded, _)) in CELL_STRINGS.iter().enumerate() {
        let row = index + 1;
        shared.push_str(&format!("<si><t>{encoded}</t></si>"));
        rows.push_str(&format!(
            "<row r=\"{row}\"><c r=\"A{row}\" t=\"s\"><v>{index}</v></c><c r=\"B{row}\" \
             t=\"inlineStr\"><is><t>{encoded}</t></is></c><c r=\"C{row}\" \
             t=\"str\"><f>\"cached\"</f><v>{encoded}</v></c></row>"
        ));
    }
    shared.push_str("</sst>");
    rows.push_str("</sheetData>");
    bytes = replace_in_part(
        &bytes,
        "xl/sharedStrings.xml",
        &part_text(&bytes, "xl/sharedStrings.xml"),
        &shared,
    );
    let worksheet = part_text(&bytes, "xl/worksheets/sheet1.xml");
    let start = worksheet.find("<sheetData>").unwrap();
    let end = worksheet.find("</sheetData>").unwrap() + "</sheetData>".len();
    replace_in_part(
        &bytes,
        "xl/worksheets/sheet1.xml",
        &worksheet[start..end],
        &rows,
    )
}

fn expected_cell_values() -> std::collections::BTreeMap<String, String> {
    let mut expected = std::collections::BTreeMap::new();
    for (index, (_, value)) in CELL_STRINGS.iter().enumerate() {
        for column in ['A', 'B', 'C'] {
            expected.insert(format!("{column}{}", index + 1), (*value).to_owned());
        }
    }
    expected
}

fn assert_cell_strings(book: &umya_spreadsheet::Workbook) {
    let sheet = book.sheet(0).unwrap();
    for (coordinate, value) in expected_cell_values() {
        assert_eq!(sheet.value(coordinate.as_str()), value, "{coordinate}");
    }
    for row in 1..=CELL_STRINGS.len() as u32 {
        assert_eq!(sheet.cell((3, row)).unwrap().formula(), "\"cached\"");
    }
}

#[test]
fn shared_inline_and_cached_strings_decode_once_on_every_cell_read_route() {
    let bytes = escaped_cell_workbook();
    for eager in [false, true] {
        let mut book = reader::xlsx::read_reader(Cursor::new(&bytes), eager).unwrap();
        book.try_read_sheet(0).unwrap();
        assert_cell_strings(&book);
    }

    let path = std::env::temp_dir().join(format!(
        "umya-xstring-cells-{}-{}.xlsx",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
    ));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .unwrap();
    file.write_all(&bytes).unwrap();
    drop(file);
    let mut book = reader::xlsx::lazy_read(&path).unwrap();
    let values: std::collections::BTreeMap<_, _> = book
        .lazy_read_sheet_cells(0)
        .unwrap()
        .iter_collection()
        .map(|cell| (cell.coordinate().to_string(), cell.value().into_owned()))
        .collect();
    assert_eq!(values, expected_cell_values());
    book.try_read_sheet(0).unwrap();
    assert_cell_strings(&book);
    let mut streamed = std::collections::BTreeMap::new();
    reader::xlsx::read_sheet_by_name_stream(&path, "Sheet1", |cell| {
        streamed.insert(cell.coordinate().to_string(), cell.value().into_owned());
    })
    .unwrap();
    assert_eq!(streamed, expected_cell_values());
    std::fs::remove_file(path).unwrap();
}

#[test]
fn cached_formula_strings_escape_and_roundtrip_through_every_writer() {
    let values = [
        "_x0041_",
        "line\u{b}break",
        "a&<\u{1}>",
        "_x005F_x0041_",
        "nonchar \u{fffe}",
    ];
    for mode in ["normal", "light", "streaming"] {
        let mut book = umya_spreadsheet::new_file();
        for (index, value) in values.iter().enumerate() {
            book.sheet_mut(0)
                .unwrap()
                .cell_mut((1, index as u32 + 1))
                .set_formula("1")
                .set_formula_result_string(*value);
        }
        let bytes = match mode {
            "normal" => {
                let mut bytes = Vec::new();
                writer::xlsx::write_writer(&book, &mut bytes).unwrap();
                bytes
            }
            "light" => {
                let mut bytes = Vec::new();
                writer::xlsx::write_writer_light(&book, &mut bytes).unwrap();
                bytes
            }
            "streaming" => {
                let zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
                let mut writer = writer::streaming_writer::StreamingWriter::new(zip, book);
                let sheet = writer.take_sheet("Sheet1").unwrap();
                writer.flush_sheet(sheet).unwrap();
                writer.finish().unwrap().into_inner()
            }
            _ => unreachable!(),
        };
        let xml = part_text(&bytes, "xl/worksheets/sheet1.xml");
        assert_eq!(xml.matches("t=\"str\"").count(), values.len(), "{mode}");
        assert!(xml.contains("_x005F_x0041_"), "{mode}: {xml}");
        assert!(xml.contains("_x000B_"), "{mode}: {xml}");
        assert!(xml.contains("_x0001_"), "{mode}: {xml}");
        assert!(xml.contains("_xFFFE_"), "{mode}: {xml}");
        assert!(
            !xml.chars().any(|c| matches!(
                c,
                '\u{0}'..='\u{8}' | '\u{b}' | '\u{c}' | '\u{e}'..='\u{1f}' | '\u{fffe}' | '\u{ffff}'
            )),
            "{mode}"
        );
        let reread = reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap();
        for (index, value) in values.iter().enumerate() {
            let cell = reread
                .sheet(0)
                .unwrap()
                .cell((1, index as u32 + 1))
                .unwrap();
            assert_eq!(cell.value(), *value, "{mode}/{index}");
            assert_eq!(cell.formula(), "1", "{mode}/{index}");
        }
    }
}

#[test]
fn chart_cache_strings_do_not_decode_spreadsheet_cell_escapes() {
    use umya_spreadsheet::{
        Chart,
        ChartType,
        drawing::spreadsheet::MarkerType,
    };
    let mut book = umya_spreadsheet::new_file();
    let mut chart = Chart::default();
    chart.new_chart(
        &ChartType::LineChart,
        MarkerType::default(),
        MarkerType::default(),
        vec!["Missing!$A$1"],
    );
    book.sheet_mut(0).unwrap().add_chart(chart);
    let mut bytes = Vec::new();
    writer::xlsx::write_writer(&book, &mut bytes).unwrap();
    let xml = "<c:chartSpace xmlns:c=\"http://schemas.openxmlformats.org/drawingml/2006/chart\">\
               <c:chart><c:plotArea><c:lineChart><c:ser><c:idx val=\"1\"/><c:order val=\"1\"/>\
               <c:tx><c:strRef><c:f>Missing!$A$1</c:f><c:strCache><c:ptCount val=\"1\"/>\
               <c:pt idx=\"0\"><c:v>_x0041_</c:v></c:pt></c:strCache></c:strRef></c:tx>\
               </c:ser></c:lineChart></c:plotArea></c:chart></c:chartSpace>";
    bytes = replace_in_part(
        &bytes,
        "xl/charts/chart1.xml",
        &part_text(&bytes, "xl/charts/chart1.xml"),
        xml,
    );
    let book = reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap();
    let chart = &book.sheet(0).unwrap().chart_collection()[0];
    let group = chart
        .chart_space()
        .chart()
        .plot_area()
        .chart_groups()
        .next()
        .unwrap();
    let series = &group.series().area_chart_series()[0];
    assert_eq!(
        series
            .chart_text()
            .unwrap()
            .string_reference()
            .unwrap()
            .cached_value(),
        Some("_x0041_")
    );
}

fn replace_in_part(bytes: &[u8], part: &str, from: &str, to: &str) -> Vec<u8> {
    let mut source = ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for index in 0..source.len() {
        let mut file = source.by_index(index).unwrap();
        let mut content = Vec::new();
        file.read_to_end(&mut content).unwrap();
        let name = file.name().unwrap().to_string();
        if name == part {
            content = String::from_utf8(content)
                .unwrap()
                .replace(from, to)
                .into_bytes();
        }
        output
            .start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        output.write_all(&content).unwrap();
    }
    output.finish().unwrap().into_inner()
}
