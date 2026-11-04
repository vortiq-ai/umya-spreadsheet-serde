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

#[test]
fn chart_formulas_quote_sheet_names_that_need_quotes() {
    for (reference, expected) in [
        ("P&L!$A$1", "'P&L'!$A$1"),
        ("'Bob''s'!$A$1", "'Bob''s'!$A$1"),
        ("1Q!$A$1", "'1Q'!$A$1"),
        ("Q1!$A$1", "'Q1'!$A$1"),
        ("q1!$A$1", "'q1'!$A$1"),
        ("Ratio Analysis!$A$1", "'Ratio Analysis'!$A$1"),
        ("Data_2!$A$1", "Data_2!$A$1"),
        ("Sheet1!$A$1", "Sheet1!$A$1"),
        ("TRUE!$A$1", "'TRUE'!$A$1"),
        ("false!$A$1", "'false'!$A$1"),
        ("TrUe!$A$1", "'TrUe'!$A$1"),
        ("FaLsE!$A$1", "'FaLsE'!$A$1"),
        ("FY2024!$A$1", "'FY2024'!$A$1"),
        ("RC0!$A$1", "'RC0'!$A$1"),
        ("R1C1!$A$1", "'R1C1'!$A$1"),
        ("r1c1!$A$1", "'r1c1'!$A$1"),
        ("RC!$A$1", "'RC'!$A$1"),
        ("R1D!$A$1", "'R1D'!$A$1"),
        ("C3PO!$A$1", "'C3PO'!$A$1"),
        ("Plan2024!$A$1", "Plan2024!$A$1"),
        ("XFE1!$A$1", "XFE1!$A$1"),
        ("R0C0!$A$1", "R0C0!$A$1"),
    ] {
        assert_eq!(ref_round_trip(reference), expected, "{reference}");
    }
}

#[test]
fn sheet_names_with_a_dot_our_parser_cannot_read_are_quoted() {
    for (name, expected) in [
        ("Data.", "'Data.'"),
        ("a._b", "'a._b'"),
        ("a..b", "'a..b'"),
        ("Data.v2", "Data.v2"),
        ("Q1.x", "Q1.x"),
        ("_data.v2", "_data.v2"),
        ("T.DIST.2T", "T.DIST.2T"),
    ] {
        let reference = format!("{name}!$A$1");
        assert_eq!(ref_round_trip(&reference), format!("{expected}!$A$1"));
        assert_chart_round_trip(name, expected, expected);
    }
}

#[test]
fn chart_sheet_name_quoting_survives_xlsx_round_trips() {
    for (name, expected) in [
        ("1Q", "'1Q'"),
        ("Q1", "'Q1'"),
        ("q1", "'q1'"),
        ("Ratio Analysis", "'Ratio Analysis'"),
        ("R1C1", "'R1C1'"),
        ("Data_2", "Data_2"),
        ("TRUE", "'TRUE'"),
        ("FALSE", "'FALSE'"),
    ] {
        assert_chart_round_trip(name, expected, expected);
    }
}

#[test]
fn sheet_name_formatter_is_public() {
    assert_eq!(
        umya_spreadsheet::helper::address::quote_sheet_name("Bob's"),
        "'Bob''s'"
    );
}

#[test]
fn doubled_apostrophes_in_sheet_names_round_trip() {
    let mut address = Address::default();
    address.set_address("'Bob''s'!$A$1");
    assert_eq!(address.sheet_name(), "Bob''s");

    let mut formula = Formula::default();
    formula.set_address(address);
    assert_eq!(formula.address().sheet_name(), "Bob''s");
    assert_eq!(formula.address_str(), "'Bob''s'!$A$1");
}

#[test]
fn quoted_sheet_names_resolve_cell_values() {
    let mut book = umya_spreadsheet::new_file();
    for (name, coordinate, value, reference) in [
        ("Bob's", "A1", "Bob's Sales", "'Bob''s'!$A$1"),
        ("Bob''s", "A1", "Two apostrophes", "'Bob''''s'!$A$1"),
        ("P&L", "C33", "Profit", "'P&L'!$C$33"),
    ] {
        book.new_sheet(name).unwrap();
        book.sheet_by_name_mut(name)
            .unwrap()
            .cell_mut(coordinate)
            .set_value(value);
        let values = book.cell_value_by_address(reference);
        assert_eq!(values.len(), 1, "{reference}");
        assert_eq!(values[0].value(), value, "{reference}");
    }
}

fn workbook_with_apostrophe_sheet_references() -> Workbook {
    let mut book = umya_spreadsheet::new_file();
    book.new_sheet("Bob's").unwrap();
    book.sheet_by_name_mut("Sheet1")
        .unwrap()
        .cell_mut("A1")
        .set_formula("SUM('Bob''s'!A1:A3)");
    let sheet = book.sheet_by_name_mut("Bob's").unwrap();
    for row in 1..=3 {
        sheet.cell_mut((1, row)).set_value_number(row);
        sheet.cell_mut((2, row)).set_value_number(row);
    }
    sheet.cell_mut("C1").set_formula("'Bob''s'!A1*2");
    let mut chart = Chart::default();
    chart.new_chart(
        &ChartType::LineChart,
        marker("D1"),
        marker("H10"),
        vec!["'Bob''s'!$B$1:$B$3"],
    );
    book.sheet_by_name_mut("Sheet1").unwrap().add_chart(chart);
    book
}

#[test]
fn cell_formulas_on_an_apostrophe_sheet_follow_a_row_insert_and_escape_once() {
    let mut book = workbook_with_apostrophe_sheet_references();
    book.insert_new_row("Bob's", 1, 1);
    assert_eq!(
        book.sheet_by_name("Sheet1")
            .unwrap()
            .cell("A1")
            .unwrap()
            .formula(),
        "SUM('Bob''s'!A2:A4)"
    );
    assert_eq!(
        book.sheet_by_name("Bob's")
            .unwrap()
            .cell("C2")
            .unwrap()
            .formula(),
        "'Bob''s'!A2*2"
    );

    book.remove_row("Bob's", 1, 1);
    assert_eq!(
        book.sheet_by_name("Sheet1")
            .unwrap()
            .cell("A1")
            .unwrap()
            .formula(),
        "SUM('Bob''s'!A1:A3)"
    );
    assert_eq!(
        book.sheet_by_name("Bob's")
            .unwrap()
            .cell("C1")
            .unwrap()
            .formula(),
        "'Bob''s'!A1*2"
    );
}

#[test]
fn cell_formulas_distinguish_sheets_with_different_apostrophe_counts() {
    let mut book = umya_spreadsheet::new_file();
    for name in ["Bob's", "Bob''s"] {
        book.new_sheet(name).unwrap();
    }
    let sheet = book.sheet_by_name_mut("Sheet1").unwrap();
    sheet.cell_mut("A1").set_formula("'Bob''s'!A1");
    sheet.cell_mut("A2").set_formula("'Bob''''s'!A1");

    book.insert_new_row("Bob's", 1, 1);
    let sheet = book.sheet_by_name("Sheet1").unwrap();
    assert_eq!(sheet.cell("A1").unwrap().formula(), "'Bob''s'!A2");
    assert_eq!(sheet.cell("A2").unwrap().formula(), "'Bob''''s'!A1");

    book.remove_row("Bob's", 1, 1);
    let sheet = book.sheet_by_name("Sheet1").unwrap();
    assert_eq!(sheet.cell("A1").unwrap().formula(), "'Bob''s'!A1");
    assert_eq!(sheet.cell("A2").unwrap().formula(), "'Bob''''s'!A1");
}

fn apostrophe_chart_reference(book: &Workbook) -> String {
    let mut chart = book.sheet_by_name("Sheet1").unwrap().chart_collection()[0].clone();
    chart.area_chart_series_list_mut().area_chart_series()[0]
        .values()
        .unwrap()
        .number_reference()
        .formula()
        .address_str()
}

#[test]
fn chart_references_on_an_apostrophe_sheet_follow_a_row_insert() {
    let mut book = workbook_with_apostrophe_sheet_references();
    assert_eq!(apostrophe_chart_reference(&book), "'Bob''s'!$B$1:$B$3");

    book.insert_new_row("Bob's", 1, 1);
    assert_eq!(apostrophe_chart_reference(&book), "'Bob''s'!$B$2:$B$4");

    book.remove_row("Bob's", 1, 1);
    assert_eq!(apostrophe_chart_reference(&book), "'Bob''s'!$B$1:$B$3");
}

fn workbook_with_raw_and_parsed_chart_references(raw: &str, replace_address: bool) -> Workbook {
    let mut book = umya_spreadsheet::new_file();
    for name in ["Bob's", "Bob''s"] {
        book.new_sheet(name).unwrap();
        let sheet = book.sheet_by_name_mut(name).unwrap();
        for row in 1..=5 {
            sheet.cell_mut((2, row)).set_value_number(row);
        }
    }
    let mut chart = Chart::default();
    chart.new_chart(
        &ChartType::LineChart,
        marker("D1"),
        marker("H10"),
        vec![
            "'Bob''s'!$B$2:$B$4",
            "'Bob''''s'!$B$2:$B$4",
            "Sheet1!$B$2:$B$4",
        ],
    );
    let formula = chart.area_chart_series_list_mut().area_chart_series_mut()[2]
        .values_mut()
        .unwrap()
        .number_reference_mut()
        .formula_mut();
    if replace_address {
        let mut address = Address::default();
        address.set_address("Sheet1!$B$2:$B$4");
        address.set_sheet_name(raw);
        formula.set_address(address);
    } else {
        formula.address_mut().set_sheet_name(raw);
    }
    book.sheet_by_name_mut("Sheet1").unwrap().add_chart(chart);
    book
}

fn assert_raw_and_parsed_chart_references(
    book: &Workbook,
    raw: &str,
    single_apostrophe_range: &str,
    double_apostrophe_range: &str,
) {
    let mut chart = book.sheet_by_name("Sheet1").unwrap().chart_collection()[0].clone();
    let actual: Vec<(String, String)> = chart
        .area_chart_series_list_mut()
        .area_chart_series()
        .iter()
        .map(|series| {
            let formula = series.values().unwrap().number_reference().formula();
            (
                formula.address_str(),
                formula.address().sheet_name().to_string(),
            )
        })
        .collect();
    assert_eq!(
        actual,
        vec![
            (
                format!("'Bob''s'!{single_apostrophe_range}"),
                "Bob''s".to_string(),
            ),
            (
                format!("'Bob''''s'!{double_apostrophe_range}"),
                "Bob''''s".to_string(),
            ),
            (
                format!("'Bob''s'!{single_apostrophe_range}"),
                raw.to_string()
            ),
        ],
        "{raw}"
    );
}

fn assert_raw_chart_references_follow_row_changes(replace_address: bool) {
    for raw in ["Bob's", "Bob''s"] {
        let mut book = workbook_with_raw_and_parsed_chart_references(raw, replace_address);
        assert_raw_and_parsed_chart_references(&book, raw, "$B$2:$B$4", "$B$2:$B$4");

        book.insert_new_row("Bob's", 1, 1);
        assert_raw_and_parsed_chart_references(&book, raw, "$B$3:$B$5", "$B$2:$B$4");
        book.remove_row("Bob's", 1, 1);
        assert_raw_and_parsed_chart_references(&book, raw, "$B$2:$B$4", "$B$2:$B$4");

        book.remove_row("Bob's", 1, 1);
        assert_raw_and_parsed_chart_references(&book, raw, "$B$1:$B$3", "$B$2:$B$4");
        book.insert_new_row("Bob's", 1, 1);
        assert_raw_and_parsed_chart_references(&book, raw, "$B$2:$B$4", "$B$2:$B$4");

        book.insert_new_row("Bob''s", 1, 1);
        assert_raw_and_parsed_chart_references(&book, raw, "$B$2:$B$4", "$B$3:$B$5");
        book.remove_row("Bob''s", 1, 1);
        assert_raw_and_parsed_chart_references(&book, raw, "$B$2:$B$4", "$B$2:$B$4");

        book.remove_row("Bob''s", 1, 1);
        assert_raw_and_parsed_chart_references(&book, raw, "$B$2:$B$4", "$B$1:$B$3");
        book.insert_new_row("Bob''s", 1, 1);
        assert_raw_and_parsed_chart_references(&book, raw, "$B$2:$B$4", "$B$2:$B$4");
    }
}

#[test]
fn raw_sheet_names_set_through_address_mut_follow_row_changes() {
    assert_raw_chart_references_follow_row_changes(false);
}

#[test]
fn raw_sheet_names_set_through_set_address_follow_row_changes() {
    assert_raw_chart_references_follow_row_changes(true);
}
