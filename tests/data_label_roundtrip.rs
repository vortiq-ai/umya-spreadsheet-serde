use std::io::{
    Cursor,
    Read,
    Write,
};

use umya_spreadsheet::{
    Chart,
    ChartType,
    Workbook,
    drawing::spreadsheet::MarkerType,
};

fn marker(coordinate: &str) -> MarkerType {
    let mut marker = MarkerType::default();
    marker.set_coordinate(coordinate);
    marker
}

fn base_book(chart_type: &ChartType) -> Workbook {
    let mut book = umya_spreadsheet::new_file();
    let sheet = book.sheet_mut(0).unwrap();
    for row in 1..=3 {
        sheet
            .cell_mut((1, row))
            .set_value_string(format!("Row {row}"));
        sheet.cell_mut((2, row)).set_value_number(row * 10);
    }
    let mut chart = Chart::default();
    chart.new_chart(
        chart_type,
        marker("E1"),
        marker("M18"),
        vec!["Sheet1!$B$1:$B$3"],
    );
    sheet.add_chart(chart);
    book
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

/// The same package with the chart part replaced by `edit(chart part)`.
fn with_chart_part(bytes: &[u8], edit: impl Fn(&str) -> String) -> Vec<u8> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut out = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for index in 0..archive.len() {
        let mut file = archive.by_index(index).unwrap();
        let name = file.name().to_string();
        let mut data = Vec::new();
        file.read_to_end(&mut data).unwrap();
        if name == "xl/charts/chart1.xml" {
            data = edit(&String::from_utf8(data).unwrap()).into_bytes();
        }
        out.start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        out.write_all(&data).unwrap();
    }
    out.finish().unwrap().into_inner()
}

fn replace_between(xml: &str, start: &str, end: &str, with: &str) -> String {
    let from = xml.find(start).expect(start);
    let to = from + xml[from..].find(end).expect(end) + end.len();
    format!("{}{with}{}", &xml[..from], &xml[to..])
}

fn between<'a>(xml: &'a str, start: &str, end: &str) -> &'a str {
    let from = xml.find(start).expect(start);
    let rest = &xml[from..];
    &rest[..rest.find(end).expect(end) + end.len()]
}

fn read(bytes: &[u8]) -> Workbook {
    umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap()
}

/// Replaces the first series of the written chart with `series` and reads it
/// back.
fn read_with_series(chart_type: &ChartType, series: &str) -> Workbook {
    read(&with_chart_part(&write(&base_book(chart_type)), |xml| {
        replace_between(
            xml,
            "<c:ser>",
            "</c:ser>",
            &format!("<c:ser>{series}</c:ser>"),
        )
    }))
}

#[test]
fn xlsx_point_labels_and_flags_roundtrip_and_mutate() {
    let series = concat!(
        "<c:idx val=\"0\"/><c:order val=\"0\"/>",
        "<c:dLbls><c:dLbl><c:idx val=\"9\"/><c:dLblPos val=\"outEnd\"/>",
        "<c:showVal/><c:separator>A&amp;B</c:separator></c:dLbl>",
        "<c:showVal val=\"true\"/><c:showCatName/><c:showLeaderLines \
         val=\"1\"/><c:leaderLines/></c:dLbls>"
    );
    let mut book = read_with_series(&ChartType::LineChart, series);
    let xml = chart_part(&write(&book));
    assert!(xml.contains(
        "<c:dLbl><c:idx val=\"9\"/><c:dLblPos \
         val=\"outEnd\"/><c:showVal/><c:separator>A&amp;B</c:separator></c:dLbl>"
    ));
    assert!(xml.contains("<c:showVal val=\"true\"/><c:showCatName/>"));
    assert!(xml.contains("<c:showLeaderLines val=\"1\"/><c:leaderLines></c:leaderLines>"));
    let plot = book.sheet_mut(0).unwrap().chart_collection_mut()[0]
        .chart_space_mut()
        .chart_mut()
        .plot_area_mut();
    let series = &mut plot
        .line_chart_mut()
        .unwrap()
        .area_chart_series_list_mut()
        .area_chart_series_mut()[0];
    series
        .data_labels_mut()
        .unwrap()
        .show_value_mut()
        .set_val(false);
    let xml = chart_part(&write(&book));
    assert!(xml.contains("<c:showVal val=\"0\"/><c:showCatName/>"));
}

#[test]
fn xlsx_label_text_preserves_body_and_interleaved_field() {
    let series = concat!(
        "<c:idx val=\"0\"/><c:order val=\"0\"/><c:dLbls>",
        "<c:txPr><a:bodyPr vert=\"vert270\" anchorCtr=\"true\"/><a:p>",
        "<a:r><a:t>before</a:t></a:r><a:fld id=\"{01234567-89AB-CDEF-0123-456789ABCDEF}\" \
         type=\"slidenum\"><a:t>4</a:t></a:fld>",
        "<a:r><a:t>after</a:t></a:r></a:p></c:txPr><c:showVal val=\"1\"/></c:dLbls>"
    );
    let book = read_with_series(&ChartType::LineChart, series);
    let xml = chart_part(&write(&book));
    assert!(xml.contains("vert=\"vert270\""));
    assert!(xml.contains("anchorCtr=\"true\""));
    let tx = between(&xml, "<c:txPr>", "</c:txPr>");
    assert!(!tx.contains("<a:lstStyle"));
    assert!(!tx.contains("<a:rPr"));
    assert!(tx.find(">before</a:t>").unwrap() < tx.find("<a:fld ").unwrap());
    assert!(tx.find("<a:fld ").unwrap() < tx.find(">after</a:t>").unwrap());
}

#[test]
fn populated_label_extension_keeps_its_group_and_order() {
    let series=concat!("<c:idx val=\"0\"/><c:order val=\"0\"/><c:dLbls>",
        "<c:extLst><c:ext uri=\"{CE6537A1-D6FC-4f65-9D91-7224C49458BB}\" xmlns:c15=\"http://schemas.microsoft.com/office/drawing/2012/chart\">",
        "<c15:showDataLabelsRange val=\"1\"/><c15:showLeaderLines val=\"1\"/><c15:leaderLines/>",
        "</c:ext></c:extLst></c:dLbls>");
    let book = read_with_series(&ChartType::LineChart, series);
    let output = chart_part(&write(&book));
    let series = between(&output, "<c:ser>", "</c:ser>");
    let labels = between(series, "<c:dLbls>", "</c:dLbls>");
    let ext = between(labels, "<c:extLst", "</c:extLst>");
    assert_eq!(
        ext.matches("{CE6537A1-D6FC-4f65-9D91-7224C49458BB}")
            .count(),
        1
    );
    assert_eq!(ext.matches("<c15:showDataLabelsRange").count(), 1);
    assert_eq!(ext.matches("<c15:showLeaderLines").count(), 1);
    assert_eq!(ext.matches("<c15:leaderLines").count(), 1);
    assert!(
        ext.find("<c15:showDataLabelsRange").unwrap() < ext.find("<c15:showLeaderLines").unwrap()
    );
    assert!(ext.find("<c15:showLeaderLines").unwrap() < ext.find("<c15:leaderLines").unwrap());
}

#[test]
fn xlsx_point_label_reference_uses_the_ordinary_chart_cache_path() {
    let series = concat!(
        "<c:idx val=\"0\"/><c:order val=\"0\"/><c:dLbls><c:dLbl><c:idx val=\"2\"/>",
        "<c:tx><c:strRef><c:f>Sheet1!$A$1</c:f></c:strRef></c:tx></c:dLbl></c:dLbls>"
    );
    let book = read_with_series(&ChartType::LineChart, series);
    let xml = chart_part(&write(&book));
    let label = between(&xml, "<c:dLbl>", "</c:dLbl>");
    assert!(label.contains("<c:f>Sheet1!$A$1</c:f>"));
    assert!(label.contains("Row 1"));
}

#[test]
fn xlsx_point_label_layout_and_manual_layout_extensions_survive() {
    let ext = concat!(
        "<c:extLst><c:ext uri=\"{01234567-89AB-CDEF-0123-456789ABCDEF}\" ",
        "xmlns:z=\"urn:test-label-layout\"><z:payload val=\"kept\"/></c:ext></c:extLst>"
    );
    let series = format!(
        "<c:idx val=\"0\"/><c:order val=\"0\"/><c:dLbls><c:dLbl><c:idx \
         val=\"2\"/><c:layout><c:manualLayout>{ext}</c:manualLayout>{ext}</c:layout></c:dLbl></c:\
         dLbls>"
    );
    let book = read_with_series(&ChartType::LineChart, &series);
    let xml = chart_part(&write(&book));
    let label = between(&xml, "<c:dLbl>", "</c:dLbl>");
    assert_eq!(
        label
            .matches("{01234567-89AB-CDEF-0123-456789ABCDEF}")
            .count(),
        2
    );
    assert_eq!(label.matches("val=\"kept\"").count(), 2);
}

#[cfg(feature = "serde")]
#[test]
fn imported_labels_survive_json_and_bincode_then_mutation() {
    let series = concat!(
        "<c:idx val=\"0\"/><c:order val=\"0\"/><c:dLbls><c:dLbl>",
        "<c:idx val=\"2\"/><c:showVal/></c:dLbl><c:showVal/><c:showCatName val=\"true\"/>",
        "</c:dLbls>"
    );
    let mut book = read_with_series(&ChartType::LineChart, series);
    let series = &mut book.sheet_mut(0).unwrap().chart_collection_mut()[0]
        .chart_space_mut()
        .chart_mut()
        .plot_area_mut()
        .line_chart_mut()
        .unwrap()
        .area_chart_series_list_mut()
        .area_chart_series_mut()[0];
    let original = series.data_labels().unwrap();
    let json = serde_json::to_string(original).unwrap();
    let json_labels: umya_spreadsheet::drawing::charts::DataLabels =
        serde_json::from_str(&json).unwrap();
    let binary = bincode::serialize(&json_labels).unwrap();
    let restored: umya_spreadsheet::drawing::charts::DataLabels =
        bincode::deserialize(&binary).unwrap();
    series.set_data_labels(restored);
    series
        .data_labels_mut()
        .unwrap()
        .show_value_mut()
        .set_val(false);
    let xml = chart_part(&write(&book));
    assert!(xml.contains("<c:dLbl><c:idx val=\"2\"/><c:showVal/></c:dLbl>"));
    assert!(xml.contains("<c:showVal val=\"0\"/><c:showCatName val=\"true\"/>"));
}

#[test]
fn point_and_group_label_formatting_remain_distinct() {
    let series = concat!(
        "<c:idx val=\"0\"/><c:order val=\"0\"/><c:dLbls><c:dLbl>",
        "<c:idx val=\"3\"/><c:numFmt formatCode=\"0.0\" sourceLinked=\"false\"/>",
        "<c:spPr><a:solidFill><a:srgbClr val=\"FF0000\"/></a:solidFill></c:spPr>",
        "<c:dLblPos val=\"t\"/><c:showVal val=\"1\"/></c:dLbl>",
        "<c:numFmt formatCode=\"0%\" sourceLinked=\"1\"/>",
        "<c:spPr><a:solidFill><a:srgbClr val=\"0000FF\"/></a:solidFill></c:spPr>",
        "<c:dLblPos val=\"b\"/><c:showVal val=\"0\"/></c:dLbls>"
    );
    let book = read_with_series(&ChartType::LineChart, series);
    let xml = chart_part(&write(&book));
    let group = between(&xml, "<c:dLbls>", "</c:dLbls>");
    let point = between(group, "<c:dLbl>", "</c:dLbl>");
    assert!(point.contains("FF0000"));
    assert!(!point.contains("0000FF"));
    assert!(point.contains("formatCode=\"0.0\""));
    assert!(point.contains("sourceLinked=\"false\""));
    assert!(group.contains("0000FF"));
    assert!(group.contains("formatCode=\"0%\""));
}

#[test]
fn empty_imported_label_owner_does_not_invent_legacy_flags() {
    let book = read_with_series(
        &ChartType::LineChart,
        "<c:idx val=\"0\"/><c:order val=\"0\"/><c:dLbls/>",
    );
    let xml = chart_part(&write(&book));
    let series = between(&xml, "<c:ser>", "</c:ser>");
    let labels = between(series, "<c:dLbls>", "</c:dLbls>");
    assert!(!labels.contains("<c:show"));
}

#[test]
fn duplicate_point_label_indices_and_order_survive_xlsx_roundtrip() {
    let series = concat!(
        "<c:idx val=\"0\"/><c:order val=\"0\"/><c:dLbls>",
        "<c:dLbl><c:idx val=\"9\"/><c:separator>first</c:separator></c:dLbl>",
        "<c:dLbl><c:idx val=\"9\"/><c:separator>second</c:separator></c:dLbl>",
        "</c:dLbls>"
    );
    let book = read_with_series(&ChartType::LineChart, series);
    let xml = chart_part(&write(&book));
    let labels = between(&xml, "<c:dLbls>", "</c:dLbls>");
    assert_eq!(labels.matches("<c:idx val=\"9\"/>").count(), 2);
    assert!(
        labels.find(">first</c:separator>").unwrap()
            < labels.find(">second</c:separator>").unwrap()
    );
}

#[test]
fn group_label_owner_absence_survives_import_until_explicit_mutation() {
    let input = write(&base_book(&ChartType::LineChart));
    let input = with_chart_part(&input, |xml| {
        let after_series = xml.find("</c:ser>").unwrap() + "</c:ser>".len();
        let relative = xml[after_series..].find("<c:dLbls>").unwrap();
        let begin = after_series + relative;
        let end = begin + xml[begin..].find("</c:dLbls>").unwrap() + "</c:dLbls>".len();
        format!("{}{}", &xml[..begin], &xml[end..])
    });
    let mut book = read(&input);
    let xml = chart_part(&write(&book));
    let line = between(&xml, "<c:lineChart>", "</c:lineChart>");
    let after_series = &line[line.find("</c:ser>").unwrap() + "</c:ser>".len()..];
    assert!(!after_series.contains("<c:dLbls"));
    let group = book.sheet_mut(0).unwrap().chart_collection_mut()[0]
        .chart_space_mut()
        .chart_mut()
        .plot_area_mut()
        .line_chart_mut()
        .unwrap();
    group.data_labels_mut().show_value_mut().set_val(true);
    let xml = chart_part(&write(&book));
    let line = between(&xml, "<c:lineChart>", "</c:lineChart>");
    let after_series = &line[line.find("</c:ser>").unwrap() + "</c:ser>".len()..];
    assert!(after_series.contains("<c:dLbls><c:showVal val=\"1\"/></c:dLbls>"));
}

#[cfg(feature = "serde")]
#[test]
fn imported_absent_group_owner_encoded_cost_is_exact() {
    let input = with_chart_part(&write(&base_book(&ChartType::LineChart)), |xml| {
        let after = xml.find("</c:ser>").unwrap() + "</c:ser>".len();
        let begin = after + xml[after..].find("<c:dLbls>").unwrap();
        let end = begin + xml[begin..].find("</c:dLbls>").unwrap() + "</c:dLbls>".len();
        format!("{}{}", &xml[..begin], &xml[end..])
    });
    let book = read(&input);
    let group = book.sheet(0).unwrap().chart_collection()[0]
        .chart_space()
        .chart()
        .plot_area()
        .line_chart()
        .unwrap();
    let imported = bincode::serialize(group.data_labels()).unwrap();
    let legacy =
        bincode::serialize(&umya_spreadsheet::drawing::charts::DataLabels::default()).unwrap();
    assert_eq!(imported.len(), legacy.len() + 31);
}

#[test]
fn imported_empty_group_owner_is_not_treated_as_absent() {
    let input = with_chart_part(&write(&base_book(&ChartType::LineChart)), |xml| {
        let after = xml.find("</c:ser>").unwrap() + "</c:ser>".len();
        let begin = after + xml[after..].find("<c:dLbls>").unwrap();
        let end = begin + xml[begin..].find("</c:dLbls>").unwrap() + "</c:dLbls>".len();
        format!("{}<c:dLbls/>{}", &xml[..begin], &xml[end..])
    });
    let book = read(&input);
    let xml = chart_part(&write(&book));
    let line = between(&xml, "<c:lineChart>", "</c:lineChart>");
    let after = &line[line.find("</c:ser>").unwrap() + "</c:ser>".len()..];
    assert!(after.contains("<c:dLbls></c:dLbls>"));
}

#[test]
fn label_number_format_preserves_missing_source_linked_attribute() {
    let series = concat!(
        "<c:idx val=\"0\"/><c:order val=\"0\"/><c:dLbls>",
        "<c:dLbl><c:idx val=\"1\"/><c:numFmt formatCode=\"0.00\"/></c:dLbl>",
        "</c:dLbls>"
    );
    let book = read_with_series(&ChartType::LineChart, series);
    let xml = chart_part(&write(&book));
    let label = between(&xml, "<c:dLbl>", "</c:dLbl>");
    assert!(label.contains("<c:numFmt formatCode=\"0.00\"/>"));
    assert!(!label.contains("sourceLinked"));
}
