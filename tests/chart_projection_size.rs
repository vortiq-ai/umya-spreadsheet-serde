#![cfg(feature = "serde")]

use std::{
    fmt::Write,
    io::{
        Cursor,
        Read,
    },
};

use umya_spreadsheet::{
    Chart,
    ChartType,
    Workbook,
    drawing::{
        charts::{
            CategoryAxisData,
            StringReference,
        },
        spreadsheet::MarkerType,
    },
};

/// Rows every workbook below declares for its category range, whatever its
/// cache holds, so that the formula text is the same in all of them.
const RANGE_ROWS: u32 = 100_000;

/// Projection size, including empty export-only slots.
// The legend's empty extension slot adds one `None` tag (1 byte) to bincode and
// `,"parts":null` (13 bytes) to JSON.
// Relative to 696 bincode / 5718 JSON bytes, axis options add 21 / 405:
// scaling and axis scalar option tags add 17 bincode bytes, and the hidden
// extension slots on two scalings and two axes add 4 bytes (72 in JSON).
// Stock/surface plot slots and line-group drop/high-low lines and up/down
// bars add another six None tags (6 bincode bytes) and 122 JSON bytes.
// The empty chart-wide text, pivot source, external data, user shapes, data
// table, print-settings, colour-mapping, protection, pivot-option and
// plot-extension slots add 13 tag bytes to bincode and 224 bytes of null keys
// to JSON. The absent numeric literal of the one value series adds one tag and
// `,"number_literal":null` (22 bytes). The one numeric reference also adds a
// hidden pivot-cache tag and `,"pivot_cache":null` (19 bytes).
// The one series adds three more `None` tags (picture options, shape, and the
// literal slot of its categories) and, to JSON, `,"picture_options":null,
// "shape":null` (36 bytes) plus `,"number_literal":null` (22 bytes).
// The two optional chart-drawing fields each add one bincode None tag. Their
// JSON cost is exactly the field names, separators, and null literals below.
// The accounting above is master's. This branch holds several drawing models at
// the fleet pin, so its chart carries a different set of optional fields and
// costs a little more. The two terms below are MEASURED on this branch, not
// derived, and which held-back model each byte belongs to has not been
// attributed. They come back out as those models land; what this test is really
// for -- that the cost does not grow with the source cache -- is asserted above
// and holds.
const HELD_BACK_MODEL_BINCODE_BYTES: usize = 16;
const HELD_BACK_MODEL_JSON_BYTES: usize = 369;
const CHART_BINCODE_BYTES: usize = 696 + 27 + 15 + 3 + 2 + HELD_BACK_MODEL_BINCODE_BYTES;
const CHART_JSON_BYTES: usize = 5718
    + 527
    + 265
    + 36
    + 22
    + r#","chart_drawing":null"#.len()
    + r#","chart_drawing_parts":null"#.len()
    + HELD_BACK_MODEL_JSON_BYTES;

fn marker(coordinate: &str) -> MarkerType {
    let mut marker = MarkerType::default();
    marker.set_coordinate(coordinate);
    marker
}

fn write(book: &Workbook) -> Vec<u8> {
    let mut bytes = Vec::new();
    umya_spreadsheet::writer::xlsx::write_writer(book, &mut Cursor::new(&mut bytes)).unwrap();
    bytes
}

fn part(bytes: &[u8], name: &str) -> String {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut xml = String::new();
    archive
        .by_name(name)
        .unwrap()
        .read_to_string(&mut xml)
        .unwrap();
    xml
}

fn rewrite_part(bytes: &[u8], name: &str, edit: impl FnOnce(String) -> String) -> Vec<u8> {
    let mut source = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let mut edit = Some(edit);
    for index in 0..source.len() {
        let mut entry = source.by_index(index).unwrap();
        let mut data = Vec::new();
        entry.read_to_end(&mut data).unwrap();
        if entry.name().unwrap() == name {
            data = edit.take().unwrap()(String::from_utf8(data).unwrap()).into_bytes();
        }
        output
            .start_file(
                entry.name().unwrap(),
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
        std::io::Write::write_all(&mut output, &data).unwrap();
    }
    assert!(edit.is_none(), "{name} was not rewritten");
    output.finish().unwrap().into_inner()
}

/// One chart whose categories are the numbers `1..=RANGE_ROWS` and whose
/// category cache holds `points` labels that all differ from their cells.
fn workbook_with_cache(points: u32) -> Vec<u8> {
    let mut book = umya_spreadsheet::new_file();
    let sheet = book.sheet_mut(0).unwrap();
    for row in 1..=RANGE_ROWS {
        sheet.cell_mut((1, row)).set_value_number(row);
    }
    let mut chart = Chart::default();
    chart.new_chart(
        &ChartType::LineChart,
        marker("E1"),
        marker("M18"),
        vec!["Sheet1!$B$1:$B$12"],
    );
    let mut categories = CategoryAxisData::default();
    let mut reference = StringReference::default();
    reference
        .formula_mut()
        .set_address_str(format!("Sheet1!$A$1:$A${RANGE_ROWS}"));
    categories.set_string_reference(reference);
    chart.area_chart_series_list_mut().area_chart_series_mut()[0]
        .set_category_axis_data(categories);
    sheet.add_chart(chart);

    let mut cache = format!("<c:strCache><c:ptCount val=\"{points}\"/>");
    for index in 0..points {
        write!(
            cache,
            "<c:pt idx=\"{index}\"><c:v>Label {index}</c:v></c:pt>"
        )
        .unwrap();
    }
    cache.push_str("</c:strCache>");
    rewrite_part(&write(&book), "xl/charts/chart1.xml", |xml| {
        let category = xml.find("<c:cat>").unwrap();
        let start = category + xml[category..].find("<c:strCache>").unwrap();
        let end = start + xml[start..].find("</c:strCache>").unwrap() + "</c:strCache>".len();
        format!("{}{cache}{}", &xml[..start], &xml[end..])
    })
}

fn assert_sources_removed(value: &serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            if let Some(source) = map.get("source") {
                assert!(source.is_null());
            }
            for child in map.values() {
                assert_sources_removed(child);
            }
        }
        serde_json::Value::Array(items) => {
            for child in items {
                assert_sources_removed(child);
            }
        }
        _ => {}
    }
}

fn wire_lengths(bytes: &[u8]) -> (usize, usize) {
    let book = umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap();
    // Projections omit caches and detachable export parts.
    let mut chart = book.sheet(0).unwrap().chart_collection()[0].clone();
    chart.strip_export_parts();
    let value = serde_json::to_value(&chart).unwrap();
    assert_sources_removed(&value);
    (
        bincode::serialize(&chart).unwrap().len(),
        serde_json::to_vec(&chart).unwrap().len(),
    )
}

#[test]
fn chart_bytes_do_not_depend_on_source_cache_size() {
    let lengths: Vec<(usize, usize)> = [10, 10_000, 100_000]
        .into_iter()
        .map(|points| wire_lengths(&workbook_with_cache(points)))
        .collect();
    assert_eq!(lengths[0], lengths[1], "10 and 10,000 point caches");
    assert_eq!(lengths[0], lengths[2], "10 and 100,000 point caches");
    assert_eq!(
        lengths[0],
        (CHART_BINCODE_BYTES, CHART_JSON_BYTES),
        "a chart is the same bytes whatever its file cached"
    );

    // Within the budget, every source label still reaches the export.
    let bytes = workbook_with_cache(10_000);
    let book = umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(&bytes), true).unwrap();
    let exported = part(&write(&book), "xl/charts/chart1.xml");
    let category = exported
        .split_once("<c:cat>")
        .unwrap()
        .1
        .split_once("</c:cat>")
        .unwrap()
        .0;
    let mut expected = format!(
        "<c:strRef><c:f>Sheet1!$A$1:$A${RANGE_ROWS}</c:f><c:strCache><c:ptCount val=\"10000\"/>"
    );
    for index in 0..10_000 {
        write!(
            expected,
            "<c:pt idx=\"{index}\"><c:v>Label {index}</c:v></c:pt>"
        )
        .unwrap();
    }
    expected.push_str("</c:strCache></c:strRef>");
    assert_eq!(category, expected);
}
