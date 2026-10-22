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

fn without_part(bytes: &[u8], name: &str) -> Vec<u8> {
    let mut source = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let mut removed = false;
    for index in 0..source.len() {
        let mut entry = source.by_index(index).unwrap();
        if entry.name() == name {
            removed = true;
            continue;
        }
        let mut data = Vec::new();
        entry.read_to_end(&mut data).unwrap();
        output
            .start_file(entry.name(), zip::write::SimpleFileOptions::default())
            .unwrap();
        std::io::Write::write_all(&mut output, &data).unwrap();
    }
    assert!(removed, "{name} was not present");
    output.finish().unwrap().into_inner()
}

/// Replaces the category `c:strCache` of the written chart part with `cache`.
fn with_category_cache(bytes: &[u8], cache: &str) -> Vec<u8> {
    rewrite_part(bytes, "xl/charts/chart1.xml", |xml| {
        let category = xml.find("<c:cat>").unwrap();
        let start = category + xml[category..].find("<c:strCache>").unwrap();
        let end = start + xml[start..].find("</c:strCache>").unwrap() + "</c:strCache>".len();
        format!("{}{cache}{}", &xml[..start], &xml[end..])
    })
}

fn dense_cache(labels: &[&str]) -> String {
    let mut cache = format!("<c:strCache><c:ptCount val=\"{}\"/>", labels.len());
    for (index, label) in labels.iter().enumerate() {
        write!(
            cache,
            "<c:pt idx=\"{index}\"><c:v>{}</c:v></c:pt>",
            escape(*label)
        )
        .unwrap();
    }
    cache.push_str("</c:strCache>");
    cache
}

fn quarter_labels() -> Vec<String> {
    (1..=12).map(|quarter| format!("Q {quarter}")).collect()
}

fn as_strs(labels: &[String]) -> Vec<&str> {
    labels.iter().map(String::as_str).collect()
}

fn read_book(bytes: &[u8]) -> Workbook {
    umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap()
}

/// Two export cycles of the chart part: loaded and written, then reloaded and
/// written again.
fn export_cycles(bytes: &[u8]) -> [String; 2] {
    let once = write(&read_book(bytes));
    let twice = write(&read_book(&once));
    [chart_part(&once), chart_part(&twice)]
}

/// Plain numeric categories 1..12 with a source cache that labels them
/// `Q 1..Q 12`, which the cell values alone cannot reproduce.
fn plain_quarter_source(reference: &str, data_sheet: Option<&str>) -> Vec<u8> {
    let mut book = category_workbook(reference);
    if let Some(name) = data_sheet {
        book.new_sheet(name).unwrap();
    }
    let sheet = match data_sheet {
        Some(name) => book.sheet_by_name_mut(name).unwrap(),
        None => book.sheet_mut(0).unwrap(),
    };
    for row in 1..=12 {
        sheet.cell_mut((1, row)).set_value_number(row);
    }
    let labels = quarter_labels();
    with_category_cache(&write(&book), &dense_cache(&as_strs(&labels)))
}

#[test]
fn untouched_categories_keep_every_source_label_without_styles() {
    let bytes = without_part(
        &plain_quarter_source("Sheet1!$A$1:$A$12", None),
        "xl/styles.xml",
    );
    let labels = quarter_labels();
    for xml in export_cycles(&bytes) {
        assert_string_reference(&xml, "cat", "Sheet1!$A$1:$A$12", &as_strs(&labels));
    }
}

#[test]
fn edited_categories_replace_only_the_changed_points_display_text() {
    let bytes = without_part(
        &plain_quarter_source("Sheet1!$A$1:$A$12", None),
        "xl/styles.xml",
    );
    let mut book = read_book(&bytes);
    let cell = book.sheet_mut(0).unwrap().cell_mut("A6");
    cell.set_value_number(42);
    cell.style_mut()
        .number_format_mut()
        .set_format_code("\"Q \"0");
    let mut labels = quarter_labels();
    labels[5] = "Q 42".to_string();
    for xml in chart_parts_after_round_trip(&book) {
        assert_string_reference(&xml, "cat", "Sheet1!$A$1:$A$12", &as_strs(&labels));
    }
}

#[test]
fn source_string_caches_preserve_counts_sparse_indices_and_empty_values() {
    let reference = "Sheet1!$A$1:$A$9";
    let mut book = category_workbook(reference);
    for row in 1..=9 {
        book.sheet_mut(0)
            .unwrap()
            .cell_mut((1, row))
            .set_value_number(100 + row);
    }
    let bytes = write(&book);
    for (source, expected) in [
        (
            concat!(
                "<c:strCache><c:ptCount val=\"9\"/>",
                "<c:pt idx=\"4\"><c:v>Four</c:v></c:pt>",
                "<c:pt idx=\"1\"><c:v/></c:pt>",
                "<c:pt idx=\"7\"/>",
                "<c:pt idx=\"0\"><c:v>A &amp; B</c:v></c:pt>",
                "</c:strCache>"
            ),
            concat!(
                "<c:strCache><c:ptCount val=\"9\"/>",
                "<c:pt idx=\"4\"><c:v>Four</c:v></c:pt>",
                "<c:pt idx=\"1\"><c:v></c:v></c:pt>",
                "<c:pt idx=\"7\"></c:pt>",
                "<c:pt idx=\"0\"><c:v>A &amp; B</c:v></c:pt>",
                "</c:strCache>"
            ),
        ),
        (
            "<c:strCache><c:pt idx=\"2\"><c:v> Two </c:v></c:pt></c:strCache>",
            "<c:strCache><c:pt idx=\"2\"><c:v> Two </c:v></c:pt></c:strCache>",
        ),
        (
            concat!(
                "<c:strCache><c:ptCount val=\"40\"/>",
                "<c:pt idx=\"30\"><c:v>Far</c:v></c:pt></c:strCache>"
            ),
            concat!(
                "<c:strCache><c:ptCount val=\"40\"/>",
                "<c:pt idx=\"30\"><c:v>Far</c:v></c:pt></c:strCache>"
            ),
        ),
        (
            concat!(
                "<c:strCache><c:ptCount val=\"4\"/>",
                "<c:pt idx=\"3\"><c:v>First</c:v></c:pt>",
                "<c:pt idx=\"3\"><c:v>Second</c:v></c:pt></c:strCache>"
            ),
            concat!(
                "<c:strCache><c:ptCount val=\"4\"/>",
                "<c:pt idx=\"3\"><c:v>First</c:v></c:pt>",
                "<c:pt idx=\"3\"><c:v>Second</c:v></c:pt></c:strCache>"
            ),
        ),
        (
            "<c:strCache><c:ptCount val=\"0\"/></c:strCache>",
            "<c:strCache><c:ptCount val=\"0\"/></c:strCache>",
        ),
    ] {
        let source_bytes = with_category_cache(&bytes, source);
        for xml in export_cycles(&source_bytes) {
            assert_eq!(
                between(&xml, "<c:cat>", "</c:cat>"),
                format!("<c:strRef><c:f>{reference}</c:f>{expected}</c:strRef>"),
                "source cache {source}"
            );
        }
    }
}

#[test]
fn empty_source_caches_stay_empty_in_both_spellings() {
    let mut book = category_workbook("Sheet1!$A$1:$A$3");
    for row in 1..=3 {
        book.sheet_mut(0)
            .unwrap()
            .cell_mut((1, row))
            .set_value_number(row);
    }
    let bytes = write(&book);
    for (source, expected) in [
        ("<c:strCache/>", "<c:strCache></c:strCache>"),
        ("<c:strCache></c:strCache>", "<c:strCache></c:strCache>"),
        (
            "<c:strCache><c:ptCount val=\"0\"/></c:strCache>",
            "<c:strCache><c:ptCount val=\"0\"/></c:strCache>",
        ),
    ] {
        let source_bytes = rewrite_part(&bytes, "xl/charts/chart1.xml", |xml| {
            let category = xml.find("<c:cat>").unwrap();
            let start = category + xml[category..].find("<c:strCache>").unwrap();
            let end = start + xml[start..].find("</c:strCache>").unwrap() + "</c:strCache>".len();
            format!("{}{source}{}", &xml[..start], &xml[end..])
        });
        for xml in export_cycles(&source_bytes) {
            assert_eq!(
                between(&xml, "<c:cat>", "</c:cat>"),
                format!("<c:strRef><c:f>Sheet1!$A$1:$A$3</c:f>{expected}</c:strRef>"),
                "source cache {source}"
            );
        }
    }
}

#[cfg(feature = "serde")]
fn source_caches(value: &serde_json::Value, found: &mut Vec<serde_json::Value>) {
    match value {
        serde_json::Value::Object(map) => {
            if let Some(source) = map.get("source").filter(|source| source.is_object()) {
                found.push(source.clone());
            }
            for child in map.values() {
                source_caches(child, found);
            }
        }
        serde_json::Value::Array(items) => {
            for child in items {
                source_caches(child, found);
            }
        }
        _ => {}
    }
}

#[cfg(feature = "serde")]
fn without_source_caches(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            if map.get("source").is_some_and(serde_json::Value::is_object) {
                map.remove("source");
            }
            for child in map.values_mut() {
                without_source_caches(child);
            }
        }
        serde_json::Value::Array(items) => {
            for child in items {
                without_source_caches(child);
            }
        }
        _ => {}
    }
}

#[cfg(feature = "serde")]
fn plain_number_workbook(chart: Chart) -> Workbook {
    let mut book = umya_spreadsheet::new_file();
    let sheet = book.sheet_mut(0).unwrap();
    for row in 1..=12 {
        sheet.cell_mut((1, row)).set_value_number(row);
    }
    sheet.add_chart(chart);
    book
}

#[cfg(feature = "serde")]
#[test]
fn chart_serde_round_trip_keeps_source_points_and_load_baselines() {
    let book = read_book(&plain_quarter_source("Sheet1!$A$1:$A$12", None));
    let json = serde_json::to_value(&book.sheet(0).unwrap().chart_collection()[0]).unwrap();

    let mut caches = Vec::new();
    source_caches(&json, &mut caches);
    assert_eq!(caches.len(), 1, "{json}");
    let cache = &caches[0];
    assert_eq!(cache["point_count"], 12);
    assert_eq!(cache["loaded_formula"], "Sheet1!$A$1:$A$12");
    let points = cache["points"].as_array().unwrap();
    assert_eq!(points.len(), 12);
    for (index, point) in points.iter().enumerate() {
        assert_eq!(point["index"], index);
        assert_eq!(point["value"], format!("Q {}", index + 1));
        assert_eq!(point["loaded_display"], (index + 1).to_string());
    }

    let restored: Chart = serde_json::from_value(json.clone()).unwrap();
    let untouched = plain_number_workbook(restored.clone());
    let labels = quarter_labels();
    for xml in chart_parts_after_round_trip(&untouched) {
        assert_string_reference(&xml, "cat", "Sheet1!$A$1:$A$12", &as_strs(&labels));
    }

    let mut edited = plain_number_workbook(restored);
    edited
        .sheet_mut(0)
        .unwrap()
        .cell_mut("A6")
        .set_value_number(42);
    let mut labels = quarter_labels();
    labels[5] = "42".to_string();
    for xml in chart_parts_after_round_trip(&edited) {
        assert_string_reference(&xml, "cat", "Sheet1!$A$1:$A$12", &as_strs(&labels));
    }

    let mut legacy = json;
    without_source_caches(&mut legacy);
    let mut caches = Vec::new();
    source_caches(&legacy, &mut caches);
    assert!(caches.is_empty());
    let restored: Chart = serde_json::from_value(legacy).unwrap();
    let numbers: Vec<String> = (1..=12).map(|number| number.to_string()).collect();
    for xml in chart_parts_after_round_trip(&plain_number_workbook(restored)) {
        assert_string_reference(&xml, "cat", "Sheet1!$A$1:$A$12", &as_strs(&numbers));
    }
}

#[test]
fn category_format_changes_refresh_only_visible_changes() {
    let mut book = read_book(&plain_quarter_source("Sheet1!$A$1:$A$12", None));
    let sheet = book.sheet_mut(0).unwrap();
    sheet
        .cell_mut("A3")
        .style_mut()
        .number_format_mut()
        .set_format_code("0.00");
    sheet
        .cell_mut("A4")
        .style_mut()
        .number_format_mut()
        .set_format_code("0");
    let mut labels = quarter_labels();
    labels[2] = "3.00".to_string();
    for xml in chart_parts_after_round_trip(&book) {
        assert_string_reference(&xml, "cat", "Sheet1!$A$1:$A$12", &as_strs(&labels));
    }
}

#[test]
fn retargeted_category_references_generate_the_new_ranges_display_text() {
    let mut book = read_book(&plain_quarter_source("Sheet1!$A$1:$A$12", None));
    let sheet = book.sheet_mut(0).unwrap();
    for (row, value) in [(1, 7), (2, 8), (3, 9)] {
        sheet.cell_mut((3, row)).set_value_number(value);
    }
    chart_mut(&mut book)
        .area_chart_series_list_mut()
        .area_chart_series_mut()[0]
        .category_axis_data_mut()
        .unwrap()
        .string_reference_mut()
        .unwrap()
        .formula_mut()
        .set_address_str("Sheet1!$C$1:$C$3");
    for xml in chart_parts_after_round_trip(&book) {
        assert_string_reference(&xml, "cat", "Sheet1!$C$1:$C$3", &["7", "8", "9"]);
    }
}

#[test]
fn later_sheet_category_references_capture_the_loaded_display() {
    let bytes = plain_quarter_source("Data!$A$1:$A$12", Some("Data"));
    let labels = quarter_labels();
    for xml in export_cycles(&bytes) {
        assert_string_reference(&xml, "cat", "Data!$A$1:$A$12", &as_strs(&labels));
    }

    let mut book = read_book(&bytes);
    book.sheet_by_name_mut("Data")
        .unwrap()
        .cell_mut("A6")
        .set_value_number(42);
    let mut labels = quarter_labels();
    labels[5] = "42".to_string();
    for xml in chart_parts_after_round_trip(&book) {
        assert_string_reference(&xml, "cat", "Data!$A$1:$A$12", &as_strs(&labels));
    }
}

#[test]
fn lazy_category_references_capture_before_mutable_access_returns() {
    let bytes = plain_quarter_source("Data!$A$1:$A$12", Some("Data"));
    let mut book = umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(&bytes), false).unwrap();
    book.sheet_mut(0).unwrap();
    // Exporting while the data sheet is still raw keeps every untouched label.
    let untouched = quarter_labels();
    assert_string_reference(
        &chart_part(&write(&book)),
        "cat",
        "Data!$A$1:$A$12",
        &as_strs(&untouched),
    );
    book.sheet_by_name_mut("Data")
        .unwrap()
        .cell_mut("A6")
        .set_value_number(42);
    let mut labels = quarter_labels();
    labels[5] = "42".to_string();
    for xml in chart_parts_after_round_trip(&book) {
        assert_string_reference(&xml, "cat", "Data!$A$1:$A$12", &as_strs(&labels));
    }
}

/// Moves the chart of the first one-cell frame into the chart collection, the
/// only place the writer exports charts from, and returns its written part.
fn exported_one_cell_chart(book: &mut Workbook) -> String {
    let sheet = book.sheet_mut(0).unwrap();
    let chart_space = sheet.worksheet_drawing().one_cell_anchor_collection()[0]
        .graphic_frame()
        .unwrap()
        .graphic()
        .graphic_data()
        .chart_space()
        .clone();
    let mut chart = Chart::default();
    chart.new_chart(
        &ChartType::LineChart,
        marker("E1"),
        marker("M18"),
        vec!["Sheet1!$B$1:$B$12"],
    );
    *chart.chart_space_mut() = chart_space;
    sheet.add_chart(chart);
    chart_part(&write(book))
}

#[test]
fn one_cell_chart_frames_capture_source_category_baselines() {
    let bytes = rewrite_part(
        &plain_quarter_source("Sheet1!$A$1:$A$12", None),
        "xl/drawings/drawing1.xml",
        |xml| {
            let start = xml.find("<xdr:to>").unwrap();
            let end = xml.find("</xdr:to>").unwrap() + "</xdr:to>".len();
            let xml = format!(
                "{}<xdr:ext cx=\"4572000\" cy=\"2743200\"/>{}",
                &xml[..start],
                &xml[end..]
            );
            xml.replace("xdr:twoCellAnchor", "xdr:oneCellAnchor")
        },
    );

    let mut book = read_book(&bytes);
    assert!(book.sheet(0).unwrap().chart_collection().is_empty());
    let labels = quarter_labels();
    assert_string_reference(
        &exported_one_cell_chart(&mut book),
        "cat",
        "Sheet1!$A$1:$A$12",
        &as_strs(&labels),
    );

    let mut book = read_book(&bytes);
    book.sheet_mut(0)
        .unwrap()
        .cell_mut("A6")
        .set_value_number(42);
    let mut labels = quarter_labels();
    labels[5] = "42".to_string();
    assert_string_reference(
        &exported_one_cell_chart(&mut book),
        "cat",
        "Sheet1!$A$1:$A$12",
        &as_strs(&labels),
    );
}
