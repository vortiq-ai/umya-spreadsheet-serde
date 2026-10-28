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

/// What a stripped `Chart` of this fixture serializes to: the bytes it had
/// before any source cache could be kept in it (672 bincode, 5309 JSON), plus
/// the string-reference cache Option tag and the one series-parts Option tag.
// Previous exact floor: 675 bincode / 5348 JSON. This fixture has three
// reference/group extension None tags (+3/+54), two Layout parts None tags
// (+2/+26), and the imported legacy group's six-present-flag owner supplement
// (+24/+310). No marker, shape, point or run owner is present in this fixture.
const CHART_BINCODE_BYTES: usize = 712;
const CHART_JSON_BYTES: usize = 5757;

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
        if entry.name() == name {
            data = edit.take().unwrap()(String::from_utf8(data).unwrap()).into_bytes();
        }
        output
            .start_file(entry.name(), zip::write::SimpleFileOptions::default())
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
    // What a browser is sent: the chart without its source caches.
    let mut chart = book.sheet(0).unwrap().chart_collection()[0].clone();
    chart.strip_source_caches();
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
