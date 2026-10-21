use std::{
    fmt::Write,
    io::{
        Cursor,
        Read,
    },
};

use quick_xml::escape::{
    escape,
    partial_escape,
};
use umya_spreadsheet::{
    Chart,
    ChartType,
    RichText,
    Workbook,
    drawing::{
        Outline,
        PresetDash,
        PresetLineDashValues,
        charts::{
            BarChart,
            CategoryAxisData,
            ChartText,
            Formula,
            GroupingValues,
            ShapeProperties,
            StringReference,
        },
        spreadsheet::MarkerType,
    },
};

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

fn chart_part(bytes: &[u8]) -> String {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut xml = String::new();
    archive
        .by_name("xl/charts/chart1.xml")
        .unwrap()
        .read_to_string(&mut xml)
        .unwrap();
    xml
}

fn chart_parts_after_round_trip(book: &Workbook) -> [String; 2] {
    let bytes = write(book);
    let restored = umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(&bytes), true).unwrap();
    [chart_part(&bytes), chart_part(&write(&restored))]
}

fn between<'a>(xml: &'a str, start: &str, end: &str) -> &'a str {
    xml.split_once(start)
        .expect(start)
        .1
        .split_once(end)
        .expect(end)
        .0
}

fn assert_string_reference(xml: &str, element: &str, formula: &str, labels: &[&str]) {
    let reference = between(xml, &format!("<c:{element}>"), &format!("</c:{element}>"));
    let mut expected = format!(
        "<c:strRef><c:f>{}</c:f><c:strCache><c:ptCount val=\"{}\"/>",
        partial_escape(formula),
        labels.len()
    );
    for (index, label) in labels.iter().enumerate() {
        write!(
            expected,
            "<c:pt idx=\"{index}\"><c:v>{}</c:v></c:pt>",
            escape(*label)
        )
        .unwrap();
    }
    expected.push_str("</c:strCache></c:strRef>");
    assert_eq!(reference, expected);
}

fn category_workbook(reference: &str) -> Workbook {
    let mut book = umya_spreadsheet::new_file();
    let sheet = book.sheet_mut(0).unwrap();
    for row in 1..=12 {
        sheet.cell_mut((2, row)).set_value_number(row);
    }

    let mut chart = Chart::default();
    chart.new_chart(
        &ChartType::LineChart,
        marker("E1"),
        marker("M18"),
        vec!["Sheet1!$B$1:$B$12"],
    );
    let mut categories = CategoryAxisData::default();
    let mut string_reference = StringReference::default();
    string_reference.formula_mut().set_address_str(reference);
    categories.set_string_reference(string_reference);
    chart.area_chart_series_list_mut().area_chart_series_mut()[0]
        .set_category_axis_data(categories);
    sheet.add_chart(chart);
    book
}

fn chart_mut(book: &mut Workbook) -> &mut Chart {
    &mut book.sheet_mut(0).unwrap().chart_collection_mut()[0]
}

fn quarter_workbook() -> Workbook {
    let mut book = category_workbook("Sheet1!$A$1:$A$12");
    let sheet = book.sheet_mut(0).unwrap();
    for row in 1..=12 {
        let cell = sheet.cell_mut((1, row));
        cell.set_value_number(row);
        cell.style_mut()
            .number_format_mut()
            .set_format_code("\"Q \"0");
    }
    book
}

#[test]
fn literal_series_names_decode_only_wrapping_quotes_in_the_cache() {
    for (literal, expected) in [
        ("\"Cumulative Soft Costs\"", "Cumulative Soft Costs"),
        ("\"Revenue \"\"Net\"\"\"", "Revenue \"Net\""),
        ("\"\"\"Quoted\"\"\"", "\"Quoted\""),
        ("Unwrapped \"\"name\"\"", "Unwrapped \"\"name\"\""),
        ("\"Open label", "\"Open label"),
        ("Close label\"", "Close label\""),
        ("\"  padded  \"", "  padded  "),
        ("\"\"", ""),
        ("Revenue & Sales", "Revenue & Sales"),
        ("001", "001"),
    ] {
        let mut book = category_workbook("Sheet1!$A$1");
        let mut reference = StringReference::default();
        reference.formula_mut().set_string_value(literal);
        let mut text = ChartText::default();
        text.set_string_reference(reference);
        chart_mut(&mut book)
            .area_chart_series_list_mut()
            .area_chart_series_mut()[0]
            .set_chart_text(text);

        for xml in chart_parts_after_round_trip(&book) {
            assert_string_reference(&xml, "tx", literal, &[expected]);
        }
    }
}

#[test]
fn header_references_keep_the_cell_text_and_escaped_sheet_name() {
    let mut book = category_workbook("Sheet1!$A$1");
    book.new_sheet("Bob's").unwrap();
    book.sheet_by_name_mut("Bob's")
        .unwrap()
        .cell_mut("A1")
        .set_value_string("Quarterly Sales");
    let mut reference = StringReference::default();
    reference.formula_mut().set_address_str("'Bob''s'!$A$1");
    let mut text = ChartText::default();
    text.set_string_reference(reference);
    chart_mut(&mut book)
        .area_chart_series_list_mut()
        .area_chart_series_mut()[0]
        .set_chart_text(text);

    for xml in chart_parts_after_round_trip(&book) {
        assert_string_reference(&xml, "tx", "'Bob''s'!$A$1", &["Quarterly Sales"]);
    }
}

#[test]
fn category_labels_use_each_cells_number_format() {
    let book = quarter_workbook();
    let labels: Vec<String> = (1..=12).map(|quarter| format!("Q {quarter}")).collect();
    let labels: Vec<&str> = labels.iter().map(String::as_str).collect();
    for xml in chart_parts_after_round_trip(&book) {
        assert_string_reference(&xml, "cat", "Sheet1!$A$1:$A$12", &labels);
    }
}

#[test]
fn text_percent_and_general_categories_keep_their_display_text() {
    let mut book = category_workbook("Sheet1!$A$1:$A$4");
    let sheet = book.sheet_mut(0).unwrap();
    sheet.cell_mut("A1").set_value_string("Plain & unchanged");
    sheet
        .cell_mut("A1")
        .style_mut()
        .number_format_mut()
        .set_format_code("\"Q \"0");
    let mut rich_text = RichText::default();
    rich_text.set_text("Rich text");
    sheet.cell_mut("A2").set_rich_text(rich_text);
    sheet
        .cell_mut("A2")
        .style_mut()
        .number_format_mut()
        .set_format_code("0%");
    sheet.cell_mut("A3").set_value_number(0.25);
    sheet
        .cell_mut("A3")
        .style_mut()
        .number_format_mut()
        .set_format_code("0%");
    sheet.cell_mut("A4").set_value_number(12.5);
    assert!(sheet.cell("A4").unwrap().style().number_format().is_none());

    for xml in chart_parts_after_round_trip(&book) {
        assert_string_reference(
            &xml,
            "cat",
            "Sheet1!$A$1:$A$4",
            &["Plain & unchanged", "Rich text", "25%", "12.5"],
        );
    }
}

#[test]
fn rectangular_category_ranges_keep_empty_points_and_row_order() {
    let mut book = category_workbook("Sheet1!$C$1:$D$3");
    let sheet = book.sheet_mut(0).unwrap();
    sheet.cell_mut("D3").set_value_string("Last");
    sheet.cell_mut("C3").set_value_string("");
    sheet.cell_mut("C2").set_value_number(2);
    sheet
        .cell_mut("C2")
        .style_mut()
        .number_format_mut()
        .set_format_code("\"Q \"0");
    sheet.cell_mut("D1").set_value_string("First");

    for xml in chart_parts_after_round_trip(&book) {
        assert_string_reference(
            &xml,
            "cat",
            "Sheet1!$C$1:$D$3",
            &["", "First", "Q 2", "", "", "Last"],
        );
    }
}

#[test]
fn missing_category_sheets_keep_an_empty_cache() {
    let book = category_workbook("Missing!$A$1:$A$3");
    for xml in chart_parts_after_round_trip(&book) {
        assert_string_reference(&xml, "cat", "Missing!$A$1:$A$3", &[]);
    }
}

#[test]
fn edited_categories_use_the_loaded_cells_current_display_text() {
    let book = quarter_workbook();
    let bytes = write(&book);
    let mut labels: Vec<String> = (1..=12).map(|quarter| format!("Q {quarter}")).collect();
    let original: Vec<&str> = labels.iter().map(String::as_str).collect();
    assert_string_reference(&chart_part(&bytes), "cat", "Sheet1!$A$1:$A$12", &original);

    let mut restored =
        umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap();
    restored
        .sheet_mut(0)
        .unwrap()
        .cell_mut("A6")
        .set_value_number(42);
    labels[5] = "Q 42".to_string();
    let labels: Vec<&str> = labels.iter().map(String::as_str).collect();
    for xml in chart_parts_after_round_trip(&restored) {
        assert_string_reference(&xml, "cat", "Sheet1!$A$1:$A$12", &labels);
    }
}

#[test]
fn numeric_caches_keep_raw_values_and_their_format_code() {
    let mut book = category_workbook("Sheet1!$A$1");
    let sheet = book.sheet_mut(0).unwrap();
    sheet.cell_mut("A1").set_value_number(0.25);
    sheet
        .cell_mut("A1")
        .style_mut()
        .number_format_mut()
        .set_format_code("0%");
    let reference = chart_mut(&mut book)
        .area_chart_series_list_mut()
        .area_chart_series_mut()[0]
        .values_mut()
        .unwrap()
        .number_reference_mut();
    let mut formula = Formula::default();
    formula.set_address_str("Sheet1!$A$1");
    reference.set_formula(formula);
    assert_eq!(reference.formula().address_str(), "Sheet1!$A$1");
    reference
        .numbering_cache_mut()
        .format_code_mut()
        .set_text("0%");

    for xml in chart_parts_after_round_trip(&book) {
        assert_string_reference(&xml, "cat", "Sheet1!$A$1", &["25%"]);
        assert_eq!(
            between(&xml, "<c:numRef>", "</c:numRef>"),
            concat!(
                "<c:f>Sheet1!$A$1</c:f><c:numCache>",
                "<c:formatCode>0%</c:formatCode><c:ptCount val=\"1\"/>",
                "<c:pt idx=\"0\"><c:v>0.25</c:v></c:pt></c:numCache>"
            )
        );
    }
}

#[test]
fn clustered_containers_and_dashed_lines_keep_their_xml() {
    let mut book = category_workbook("Sheet1!$A$1:$A$12");
    let chart = chart_mut(&mut book);
    let mut series = chart.area_chart_series_list_mut().area_chart_series()[0].clone();
    series.index_mut().set_val(1);
    series.order_mut().set_val(1);
    let mut bar = BarChart::default();
    bar.grouping_mut().set_val(GroupingValues::Clustered);
    bar.area_chart_series_list_mut()
        .add_area_chart_series(series);
    chart.plot_area_mut().set_bar_chart(bar);

    let mut dash = PresetDash::default();
    dash.set_val(PresetLineDashValues::Dash);
    let mut outline = Outline::default();
    outline.set_preset_dash(dash);
    let mut shape = ShapeProperties::default();
    shape.set_outline(outline);
    chart.area_chart_series_list_mut().area_chart_series_mut()[0].set_shape_properties(shape);

    for xml in chart_parts_after_round_trip(&book) {
        let bar = between(&xml, "<c:barChart>", "</c:barChart>");
        assert!(bar.contains("<c:grouping val=\"clustered\"/>"), "{bar}");
        let line = between(&xml, "<c:lineChart>", "</c:lineChart>");
        assert!(line.contains("<c:grouping val=\"stacked\"/>"), "{line}");
        assert!(line.contains("<a:prstDash val=\"dash\"/>"), "{line}");
        for container in [bar, line] {
            assert!(container.contains("<c:f>Sheet1!$B$1:$B$12</c:f>"));
            assert!(container.contains("<c:f>Sheet1!$A$1:$A$12</c:f>"));
        }
    }
}
