//! A chart series reference must keep the quotes Excel requires around a sheet
//! name. `'P&L'!$D$34:$M$34` used to read back as `P&L!$D$34:$M$34` and was
//! written that way, which Excel rejects.
use std::io::{
    Cursor,
    Read,
};

use umya_spreadsheet::{
    Address,
    Chart,
    ChartType,
    drawing::{
        self,
        charts::{
            CategoryAxisData,
            Formula,
            StringReference,
        },
    },
    structs::Workbook,
};

fn ref_round_trip(reference: &str) -> String {
    let mut formula = Formula::default();
    formula.set_address_str(reference);
    formula.address_str()
}

#[test]
fn sheet_names_that_need_quotes_keep_them() {
    for reference in [
        "'P&L'!$D$34:$M$34",
        "'Cashflow (Monthly)'!$J$6:$BG$6",
        "'Bob''s'!$A$1",
        "'Bob''s Plan'!$A$1",
        "'FY2024'!$A$1",
        "'a1'!$A$1",
        "'R1C1'!$A$1",
        "'1Q'!$A$1",
        "'a!b'!$A$1",
        "'Données'!$A$1",
    ] {
        assert_eq!(ref_round_trip(reference), reference);
    }
}

#[test]
fn plain_sheet_names_and_bare_ranges_stay_unquoted() {
    assert_eq!(ref_round_trip("Sheet1!$A$1:$A$3"), "Sheet1!$A$1:$A$3");
    assert_eq!(ref_round_trip("$A$1"), "$A$1");
}

#[test]
fn a_literal_string_value_is_unchanged() {
    assert_eq!(ref_round_trip(r#""Net Profit""#), r#""Net Profit""#);
    let mut formula = Formula::default();
    formula.set_string_value("P&L");
    assert_eq!(formula.address_str(), "P&L");
}

#[cfg(feature = "serde")]
#[test]
fn a_legacy_payload_with_a_doubled_apostrophe_is_not_escaped_again() {
    let mut formula = Formula::default();
    formula.set_address_str("'Bob''s Plan'!$A$1");
    let json = serde_json::to_string(&formula).unwrap();
    // Older versions stored the name exactly as parsed, doubled apostrophe
    // kept.
    assert!(json.contains(r#""sheet_name":"Bob''s Plan""#), "{json}");
    let legacy: Formula = serde_json::from_str(&json).unwrap();
    assert_eq!(legacy.address_str(), "'Bob''s Plan'!$A$1");
}

fn chart_part(bytes: &[u8]) -> String {
    let mut archive = ::zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut xml = String::new();
    archive
        .by_name("xl/charts/chart1.xml")
        .unwrap()
        .read_to_string(&mut xml)
        .unwrap();
    xml
}

fn write(book: &Workbook) -> Vec<u8> {
    let mut bytes = Vec::new();
    umya_spreadsheet::writer::xlsx::write_writer(book, &mut Cursor::new(&mut bytes)).unwrap();
    bytes
}

fn marker(coordinate: &str) -> drawing::spreadsheet::MarkerType {
    let mut marker = drawing::spreadsheet::MarkerType::default();
    marker.set_coordinate(coordinate);
    marker
}

#[test]
fn write_to_quotes_and_escapes_the_reference() {
    let mut book = umya_spreadsheet::new_file();
    book.new_sheet("P&L").unwrap();
    let mut chart = Chart::default();
    chart.new_chart(
        &ChartType::LineChart,
        marker("C1"),
        marker("D5"),
        vec!["'P&L'!$D$34:$M$34"],
    );
    book.sheet_by_name_mut("P&L").unwrap().add_chart(chart);

    let xml = chart_part(&write(&book));
    assert!(xml.contains("<c:f>'P&amp;L'!$D$34:$M$34</c:f>"), "{xml}");
}

fn between<'a>(xml: &'a str, start: &str, end: &str) -> &'a str {
    let rest = xml.split_once(start).expect(start).1;
    rest.split_once(end).expect(end).0
}

/// Writes a chart on `sheet_name`, reads the file back and writes it again,
/// checking the raw `<c:f>` text, the caches and the API value each time.
fn assert_chart_round_trip(sheet_name: &str, written: &str, escaped: &str) {
    let values = format!("{written}!$B$1:$B$3");
    let categories = format!("{written}!$A$1:$A$3");

    let mut book = umya_spreadsheet::new_file();
    book.new_sheet(sheet_name).unwrap();
    let sheet = book.sheet_by_name_mut(sheet_name).unwrap();
    for (row, value) in [(1, 5), (2, 9), (3, 12)] {
        sheet.cell_mut((1, row)).set_value_number(row);
        sheet.cell_mut((2, row)).set_value_number(value);
    }
    let mut chart = Chart::default();
    chart.new_chart(
        &ChartType::LineChart,
        marker("D1"),
        marker("H10"),
        vec![values.as_str()],
    );
    let mut string_reference = StringReference::default();
    string_reference.formula_mut().set_address_str(&categories);
    let mut category_axis_data = CategoryAxisData::default();
    category_axis_data.set_string_reference(string_reference);
    chart.area_chart_series_list_mut().area_chart_series_mut()[0]
        .set_category_axis_data(category_axis_data);
    book.sheet_by_name_mut(sheet_name).unwrap().add_chart(chart);

    let mut bytes = write(&book);
    for pass in ["first write", "second write"] {
        let xml = chart_part(&bytes);
        assert!(
            xml.contains(&format!("<c:f>{escaped}!$B$1:$B$3</c:f>")),
            "{pass}, values: {xml}"
        );
        assert!(
            xml.contains(&format!("<c:f>{escaped}!$A$1:$A$3</c:f>")),
            "{pass}, categories: {xml}"
        );
        let numbers = between(&xml, "<c:numCache>", "</c:numCache>");
        for cached in ["<c:v>5</c:v>", "<c:v>9</c:v>", "<c:v>12</c:v>"] {
            assert!(numbers.contains(cached), "{pass}, numCache {cached}: {xml}");
        }
        let strings = between(&xml, "<c:strCache>", "</c:strCache>");
        for cached in ["<c:v>1</c:v>", "<c:v>2</c:v>", "<c:v>3</c:v>"] {
            assert!(strings.contains(cached), "{pass}, strCache {cached}: {xml}");
        }

        let read = umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(&bytes), true).unwrap();
        let mut chart = read.sheet_by_name(sheet_name).unwrap().chart_collection()[0].clone();
        let series = &chart.area_chart_series_list_mut().area_chart_series()[0];
        assert_eq!(
            series
                .values()
                .unwrap()
                .number_reference()
                .formula()
                .address_str(),
            values
        );
        assert_eq!(
            series
                .category_axis_data()
                .unwrap()
                .string_reference()
                .unwrap()
                .formula()
                .address_str(),
            categories
        );
        bytes = write(&read);
    }
}

#[test]
fn a_chart_on_an_ampersand_sheet_round_trips_through_xlsx() {
    assert_chart_round_trip("P&L", "'P&L'", "'P&amp;L'");
}

#[test]
fn a_chart_on_an_apostrophe_sheet_round_trips_through_xlsx() {
    assert_chart_round_trip("Bob's Plan", "'Bob''s Plan'", "'Bob''s Plan'");
}

#[test]
fn raw_sheet_names_set_through_the_setters_are_escaped() {
    for raw in ["Bob's", "Bob''s"] {
        let mut formula = Formula::default();
        formula.set_address_str("Sheet1!$A$1");
        formula.address_mut().set_sheet_name(raw);
        assert_eq!(formula.address_str(), "'Bob''s'!$A$1", "{raw}");

        let mut address = Address::default();
        address.set_address("Sheet1!$A$1");
        address.set_sheet_name(raw);
        let mut formula = Formula::default();
        formula.set_address(address);
        assert_eq!(formula.address_str(), "'Bob''s'!$A$1", "{raw}");
    }
}

#[test]
fn sheets_that_differ_only_in_apostrophes_keep_their_own_cache() {
    let mut book = umya_spreadsheet::new_file();
    for (name, value) in [("Bob's", 5), ("Bob''s", 99)] {
        book.new_sheet(name).unwrap();
        book.sheet_by_name_mut(name)
            .unwrap()
            .cell_mut("A1")
            .set_value_number(value);
    }
    let mut chart = Chart::default();
    chart.new_chart(
        &ChartType::LineChart,
        marker("D1"),
        marker("H10"),
        vec!["'Bob''s'!$A$1", "'Bob''''s'!$A$1"],
    );
    book.sheet_by_name_mut("Sheet1").unwrap().add_chart(chart);

    let xml = chart_part(&write(&book));
    let series: Vec<&str> = xml.split("<c:numRef>").skip(1).collect();
    assert_eq!(series.len(), 2, "{xml}");
    assert!(series[0].starts_with("<c:f>'Bob''s'!$A$1</c:f>"), "{xml}");
    assert!(series[0].contains("<c:v>5</c:v>"), "{xml}");
    assert!(series[1].starts_with("<c:f>'Bob''''s'!$A$1</c:f>"), "{xml}");
    assert!(series[1].contains("<c:v>99</c:v>"), "{xml}");
}
