use std::io::{
    Cursor,
    Read,
    Write,
};

use umya_spreadsheet::{
    Chart,
    ChartType,
    Workbook,
    drawing::{
        charts::AreaChartSeries,
        spreadsheet::MarkerType,
    },
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

fn chart_of(book: &Workbook) -> &Chart {
    &book.sheet(0).unwrap().chart_collection()[0]
}

fn first_series(book: &Workbook, chart_type: &ChartType) -> AreaChartSeries {
    let plot = chart_of(book).chart_space().chart().plot_area();
    let list = match chart_type {
        ChartType::LineChart => plot.line_chart().unwrap().area_chart_series_list(),
        ChartType::BarChart => plot.bar_chart().unwrap().area_chart_series_list(),
        _ => unreachable!(),
    };
    list.area_chart_series()[0].clone()
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

fn read_with_chart_edit(edit: impl Fn(&str) -> String) -> Workbook {
    read(&with_chart_part(
        &write(&base_book(&ChartType::LineChart)),
        edit,
    ))
}

fn written_series(book: &Workbook) -> String {
    let xml = chart_part(&write(book));
    between(&xml, "<c:ser>", "</c:ser>").to_string()
}

fn line(color: &str) -> String {
    [
        "<c:spPr><a:ln w=\"28575\"><a:solidFill><a:srgbClr val=\"",
        color,
        "\"/></a:solidFill></a:ln></c:spPr>",
    ]
    .concat()
}

fn fill(color: &str) -> String {
    format!("<c:spPr><a:solidFill><a:srgbClr val=\"{color}\"/></a:solidFill></c:spPr>")
}

const REFS: &str = concat!(
    "<c:cat><c:strRef><c:f>Sheet1!$A$1:$A$3</c:f></c:strRef>",
    "</c:cat><c:val><c:numRef><c:f>Sheet1!$B$1:$B$3</c:f>",
    "</c:numRef></c:val>",
);

const SMALL_TEXT: &str = concat!(
    "<c:txPr><a:bodyPr/><a:lstStyle/><a:p><a:pPr>",
    "<a:defRPr sz=\"900\"/></a:pPr><a:endParaRPr lang=\"en-US\"/></a:p>",
    "</c:txPr>",
);

#[test]
fn point_formatting_does_not_replace_the_line_series_index_order_or_marker() {
    let point_marker = [
        "<c:dPt><c:idx val=\"9\"/><c:marker><c:symbol val=\"circle\"/>",
        "<c:size val=\"5\"/>",
        &fill("ed7d31"),
        "</c:marker></c:dPt>",
    ]
    .concat();
    let series = [
        "<c:idx val=\"1\"/><c:order val=\"1\"/>",
        &line("4472c4"),
        &point_marker,
        REFS,
        "<c:smooth val=\"0\"/>",
    ]
    .concat();
    let book = read_with_series(&ChartType::LineChart, &series);
    let read_series = first_series(&book, &ChartType::LineChart);
    assert_eq!(read_series.index().val(), 1);
    assert_eq!(read_series.order().val(), 1);
    assert!(read_series.marker().is_none());
    assert!(read_series.shape_properties().is_some());

    let ser = written_series(&book);
    assert!(
        ser.starts_with("<c:ser><c:idx val=\"1\"/><c:order val=\"1\"/>"),
        "{ser}"
    );
    assert!(ser.contains(&point_marker), "{ser}");
    assert!(ser.contains("circle"), "{ser}");
    assert!(ser.to_lowercase().contains("4472c4"), "{ser}");
    assert!(ser.to_lowercase().contains("ed7d31"), "{ser}");
}

#[test]
fn point_formatting_does_not_replace_the_bar_series_own_values() {
    let point = [
        "<c:dPt><c:idx val=\"3\"/><c:invertIfNegative val=\"1\"/>",
        "<c:bubble3D val=\"1\"/><c:explosion val=\"25\"/>",
        &fill("ff0000"),
        "</c:dPt>",
    ]
    .concat();
    let series = [
        "<c:idx val=\"0\"/><c:order val=\"0\"/>",
        "<c:invertIfNegative val=\"0\"/>",
        &fill("4472c4"),
        &point,
        REFS,
    ]
    .concat();
    let book = read_with_series(&ChartType::BarChart, &series);
    let read_series = first_series(&book, &ChartType::BarChart);
    assert_eq!(read_series.index().val(), 0);
    assert_eq!(read_series.order().val(), 0);
    assert!(read_series.explosion().is_none());
    assert!(read_series.bubble_3d().is_none());
    assert!(read_series.invert_if_negative().unwrap().val().abs() < f64::EPSILON);

    let ser = written_series(&book);
    assert!(ser.to_lowercase().contains("4472c4"), "{ser}");
    assert!(ser.to_lowercase().contains("ff0000"), "{ser}");
    assert!(ser.contains("<c:explosion val=\"25\"/>"), "{ser}");
    assert!(ser.contains("<c:bubble3D val=\"1\"/>"), "{ser}");
}

#[test]
fn trendlines_and_error_bars_do_not_replace_the_series_shape_properties() {
    let trendline = [
        "<c:trendline><c:name>Trend</c:name>",
        &line("00ff00"),
        "<c:trendlineType val=\"linear\"/><c:dispRSqr val=\"0\"/>",
        "<c:dispEq val=\"0\"/></c:trendline>",
    ]
    .concat();
    let error_bars = [
        "<c:errBars><c:errDir val=\"y\"/><c:errBarType val=\"both\"/>",
        "<c:errValType val=\"fixedVal\"/><c:noEndCap val=\"0\"/>",
        "<c:val val=\"10\"/>",
        &line("0000ff"),
        "</c:errBars>",
    ]
    .concat();
    let series = [
        "<c:idx val=\"2\"/><c:order val=\"2\"/>",
        &line("4472c4"),
        &trendline,
        &error_bars,
        REFS,
    ]
    .concat();
    let book = read_with_series(&ChartType::LineChart, &series);
    let read_series = first_series(&book, &ChartType::LineChart);
    assert_eq!(read_series.index().val(), 2);
    assert_eq!(read_series.order().val(), 2);
    assert!(read_series.values().is_some());

    let ser = written_series(&book).to_lowercase();
    assert!(ser.contains("4472c4"), "{ser}");
    assert!(ser.contains("00ff00"), "{ser}");
    assert_eq!(read_series.trendlines().len(), 1);
    assert_eq!(read_series.trendlines()[0].name(), Some("Trend"));
    assert!(ser.contains("0000ff"), "{ser}");
    assert_eq!(read_series.error_bars().len(), 1);
}

#[test]
fn series_without_unmodeled_parts_write_the_same_element_as_before() {
    let book = base_book(&ChartType::LineChart);
    let first = written_series(&book);
    let second = written_series(&read(&write(&book)));
    assert_eq!(first, second);
    assert_eq!(first, EXPECTED_PLAIN_SERIES);
}

const EXPECTED_PLAIN_SERIES: &str = concat!(
    "<c:ser><c:idx val=\"0\"/><c:order val=\"0\"/><c:val><c:numRef>",
    "<c:f>Sheet1!$B$1:$B$3</c:f><c:numCache>",
    "<c:formatCode>General</c:formatCode><c:ptCount val=\"3\"/>",
    "<c:pt idx=\"0\"><c:v>10</c:v></c:pt><c:pt idx=\"1\"><c:v>20</c:v>",
    "</c:pt><c:pt idx=\"2\"><c:v>30</c:v></c:pt></c:numCache>",
    "</c:numRef></c:val><c:smooth val=\"0\"/></c:ser>",
);

const EXPECTED_FULL_SERIES: &str = concat!(
    "<c:ser><c:idx val=\"4\"/><c:order val=\"5\"/><c:tx><c:strRef>",
    "<c:f>Sheet1!$B$1</c:f><c:strCache><c:ptCount val=\"1\"/>",
    "<c:pt idx=\"0\"><c:v>10</c:v></c:pt></c:strCache></c:strRef>",
    "</c:tx><c:spPr><a:ln w=\"28575\"><a:solidFill>",
    "<a:srgbClr val=\"4472c4\"/></a:solidFill></a:ln></c:spPr>",
    "<c:marker><c:symbol val=\"diamond\"/><c:size val=\"7\"/><c:spPr>",
    "<a:solidFill><a:srgbClr val=\"ed7d31\"/></a:solidFill></c:spPr>",
    "</c:marker><c:dLbls>",
    "<c:txPr><a:bodyPr/><a:lstStyle/><a:p><a:pPr>",
    "<a:defRPr sz=\"1000\"/></a:pPr><a:endParaRPr lang=\"en-US\"/></a:p>",
    "</c:txPr><c:showLegendKey val=\"0\"/><c:showVal val=\"1\"/>",
    "<c:showCatName val=\"0\"/><c:showSerName val=\"0\"/>",
    "<c:showPercent val=\"0\"/><c:showBubbleSize val=\"0\"/></c:dLbls>",
    "<c:cat><c:strRef><c:f>Sheet1!$A$1:$A$3</c:f><c:strCache>",
    "<c:ptCount val=\"3\"/><c:pt idx=\"0\"><c:v>Row 1</c:v></c:pt>",
    "<c:pt idx=\"1\"><c:v>Row 2</c:v></c:pt><c:pt idx=\"2\">",
    "<c:v>Row 3</c:v></c:pt></c:strCache></c:strRef></c:cat><c:val>",
    "<c:numRef><c:f>Sheet1!$B$1:$B$3</c:f><c:numCache><c:formatCode>",
    "</c:formatCode><c:ptCount val=\"3\"/><c:pt idx=\"0\"><c:v>10</c:v>",
    "</c:pt><c:pt idx=\"1\"><c:v>20</c:v></c:pt><c:pt idx=\"2\">",
    "<c:v>30</c:v></c:pt></c:numCache></c:numRef></c:val>",
    "<c:smooth val=\"1\"/></c:ser>",
);

#[test]
fn series_with_only_modeled_parts_write_the_same_element_as_before() {
    let labels = [
        "<c:dLbls><c:txPr><a:bodyPr/><a:lstStyle/><a:p><a:pPr>",
        "<a:defRPr sz=\"1000\"/></a:pPr><a:endParaRPr lang=\"en-US\"/></a:p>",
        "</c:txPr><c:showLegendKey val=\"0\"/><c:showVal val=\"1\"/>",
        "<c:showCatName val=\"0\"/><c:showSerName val=\"0\"/>",
        "<c:showPercent val=\"0\"/><c:showBubbleSize val=\"0\"/></c:dLbls>",
    ]
    .concat();
    let series = [
        "<c:idx val=\"4\"/><c:order val=\"5\"/>",
        "<c:tx><c:strRef><c:f>Sheet1!$B$1</c:f></c:strRef></c:tx>",
        "<c:explosion val=\"5\"/><c:invertIfNegative val=\"0\"/>",
        &line("4472c4"),
        "<c:marker><c:symbol val=\"diamond\"/><c:size val=\"7\"/>",
        &fill("ed7d31"),
        "</c:marker>",
        &labels,
        REFS,
        "<c:bubble3D val=\"0\"/><c:smooth val=\"1\"/>",
    ]
    .concat();
    let book = read_with_series(&ChartType::LineChart, &series);
    assert_eq!(written_series(&book), EXPECTED_FULL_SERIES);
}

#[test]
fn point_labels_do_not_replace_the_series_data_label_settings() {
    let series = [
        "<c:idx val=\"0\"/><c:order val=\"0\"/>",
        "<c:dLbls><c:dLbl><c:idx val=\"2\"/>",
        SMALL_TEXT,
        "<c:showLegendKey val=\"0\"/><c:showVal val=\"1\"/>",
        "<c:showCatName val=\"0\"/><c:showSerName val=\"0\"/>",
        "<c:showPercent val=\"0\"/><c:showBubbleSize val=\"0\"/>",
        "</c:dLbl><c:delete val=\"1\"/></c:dLbls>",
        REFS,
    ]
    .concat();
    let book = read_with_series(&ChartType::LineChart, &series);
    let read_series = first_series(&book, &ChartType::LineChart);
    let labels = read_series.data_labels().expect("series dLbls");
    assert!(labels.text_properties().is_none());
    assert!(!labels.show_value().val());
    assert_eq!(labels.labels().len(), 1);
    assert_eq!(labels.labels()[0].index().val(), 2);
    assert!(
        labels.labels()[0]
            .flags()
            .show_value
            .as_ref()
            .unwrap()
            .value()
            .unwrap()
    );
    let written = written_series(&book);
    assert!(written.contains("<c:dLbl><c:idx val=\"2\"/>"));
    assert!(written.contains("<c:delete val=\"1\"/>"));
}

#[test]
fn minor_gridlines_do_not_replace_the_axis_shape_properties() {
    let gridlines = format!("<c:minorGridlines>{}</c:minorGridlines>", line("ff0000"));
    for (close, is_value) in [("</c:valAx>", true), ("</c:catAx>", false)] {
        let book =
            read_with_chart_edit(|xml| xml.replacen(close, &format!("{gridlines}{close}"), 1));
        let plot = chart_of(&book).chart_space().chart().plot_area();
        let has_properties = if is_value {
            plot.value_axis()[0].shape_properties().is_some()
        } else {
            plot.category_axis()[0].shape_properties().is_some()
        };
        assert!(
            !has_properties,
            "axis (value: {is_value}) took the gridlines' spPr"
        );
    }
}

#[test]
fn display_units_label_does_not_replace_the_axis_own_formatting() {
    let units = [
        "<c:dispUnits><c:builtInUnit val=\"millions\"/><c:dispUnitsLbl>",
        &fill("ff0000"),
        SMALL_TEXT,
        "</c:dispUnitsLbl></c:dispUnits>",
    ]
    .concat();
    let own = line("4472c4");
    let book = read_with_chart_edit(|xml| {
        xml.replacen("</c:valAx>", &format!("{own}{units}</c:valAx>"), 1)
    });
    let rewritten = chart_part(&write(&book));
    let axis = between(&rewritten, "<c:valAx>", "</c:valAx>").to_lowercase();
    assert!(axis.contains("4472c4"), "{axis}");
    assert!(!axis.contains("ff0000"), "{axis}");
    assert!(!axis.contains("sz=\"900\""), "{axis}");
}

#[test]
fn minor_gridlines_do_not_replace_the_date_axis_shape_properties() {
    let gridlines = format!("<c:minorGridlines>{}</c:minorGridlines>", line("ff0000"));
    let book = read_with_chart_edit(|xml| {
        xml.replace("c:catAx>", "c:dateAx>").replacen(
            "</c:dateAx>",
            &format!("{gridlines}</c:dateAx>"),
            1,
        )
    });
    let plot = chart_of(&book).chart_space().chart().plot_area();
    assert_eq!(plot.date_axis().len(), 1);
    assert!(plot.date_axis()[0].shape_properties().is_none());
}

#[test]
fn data_table_does_not_replace_the_plot_area_shape_properties() {
    let table = [
        "<c:dTable><c:showHorzBorder val=\"1\"/><c:showVertBorder val=\"1\"/>",
        "<c:showOutline val=\"1\"/><c:showKeys val=\"0\"/>",
        &fill("ff0000"),
        "</c:dTable>",
    ]
    .concat();
    let book = read_with_chart_edit(|xml| {
        xml.replacen("</c:plotArea>", &format!("{table}</c:plotArea>"), 1)
    });
    let plot = chart_of(&book).chart_space().chart().plot_area();
    assert!(plot.shape_properties().is_none());
}

const FILTERING_URI: &str = "{02D57815-91ED-43cb-92C2-25804820EDAC}";
const C15_NS: &str = "http://schemas.microsoft.com/office/drawing/2012/chart";

fn visible_series(chart_type: &ChartType) -> String {
    let refs = match chart_type {
        ChartType::ScatterChart => concat!(
            "<c:xVal><c:numRef><c:f>Sheet1!$A$1:$A$3</c:f></c:numRef></c:xVal>",
            "<c:yVal><c:numRef><c:f>Sheet1!$B$1:$B$3</c:f></c:numRef></c:yVal>",
        ),
        ChartType::BubbleChart => concat!(
            "<c:xVal><c:numRef><c:f>Sheet1!$A$1:$A$3</c:f></c:numRef></c:xVal>",
            "<c:yVal><c:numRef><c:f>Sheet1!$B$1:$B$3</c:f></c:numRef></c:yVal>",
            "<c:bubbleSize><c:numRef><c:f>Sheet1!$B$1:$B$3</c:f></c:numRef>",
            "</c:bubbleSize>",
        ),
        _ => REFS,
    };
    format!("<c:ser><c:idx val=\"0\"/><c:order val=\"0\"/>{refs}</c:ser>")
}

fn filtered_series(chart_type: &ChartType) -> String {
    visible_series(chart_type)
        .replace("idx val=\"0\"", "idx val=\"7\"")
        .replace("order val=\"0\"", "order val=\"7\"")
        .replace("$B$1:$B$3", "$C$1:$C$3")
        .replace("c:ser>", "c15:ser>")
}

#[test]
fn filtered_series_extensions_do_not_become_ordinary_series() {
    let families = [
        (ChartType::BarChart, "barChart", "filteredBarSeries"),
        (ChartType::LineChart, "lineChart", "filteredLineSeries"),
        (ChartType::AreaChart, "areaChart", "filteredAreaSeries"),
        (ChartType::PieChart, "pieChart", "filteredPieSeries"),
        (
            ChartType::ScatterChart,
            "scatterChart",
            "filteredScatterSeries",
        ),
        (ChartType::RadarChart, "radarChart", "filteredRadarSeries"),
        (
            ChartType::BubbleChart,
            "bubbleChart",
            "filteredBubbleSeries",
        ),
    ];
    for (chart_type, group, filtered_tag) in families {
        let extension = format!(
            concat!(
                "<c:extLst><c:ext uri=\"{uri}\" xmlns:c15=\"{ns}\">",
                "<c15:{tag}>{series}</c15:{tag}></c:ext></c:extLst>",
            ),
            uri = FILTERING_URI,
            ns = C15_NS,
            tag = filtered_tag,
            series = filtered_series(&chart_type),
        );
        let close = format!("</c:{group}>");
        let book = read(&with_chart_part(&write(&base_book(&chart_type)), |xml| {
            let visible = visible_series(&chart_type);
            let one_visible = if xml.contains("<c:ser>") {
                replace_between(xml, "<c:ser>", "</c:ser>", &visible)
            } else {
                xml.replacen(&close, &format!("{visible}{close}"), 1)
            };
            assert_eq!(one_visible.matches("<c:ser>").count(), 1, "{group}");
            one_visible.replacen(&close, &format!("{extension}{close}"), 1)
        }));
        let xml = chart_part(&write(&book));
        assert_eq!(
            xml.matches("<c:ser>").count(),
            1,
            "{group}: the filtered series became an ordinary series: {xml}"
        );
        let ser = between(&xml, "<c:ser>", "</c:ser>");
        assert!(
            ser.starts_with("<c:ser><c:idx val=\"0\"/><c:order val=\"0\"/>"),
            "{group}: {ser}"
        );
        assert!(ser.contains("Sheet1!$B$1:$B$3"), "{group}: {ser}");
        // Registered filtered series have their own typed owner. Retain them
        // without promoting them into the ordinary series collection above.
        if matches!(chart_type, ChartType::PieChart) {
            // The pinned registration table has no filteredPieSeries row.
            // This is retained invalid known-URI content, not a positive row.
            let list = chart_of(&book)
                .chart_space()
                .chart()
                .plot_area()
                .pie_chart()
                .unwrap()
                .extensions()
                .unwrap();
            assert_eq!(list.entries.len(), 1);
            assert!(matches!(&list.entries[0].nodes[0],
                umya_spreadsheet::drawing::charts::ExtensionNode::Unknown(value)
                if value.name.local_name=="filteredPieSeries" && value.name.namespace_uri==C15_NS));
            let diagnostic = list.export_diagnostics_for_owner(
                Some(&book),
                "pieChart",
                Some(umya_spreadsheet::drawing::charts::SeriesFamily::Pie),
            );
            assert_eq!(diagnostic.len(), 1);
            assert_eq!(diagnostic[0].reason, "NotAllowed");
            assert!(!xml.contains("$C$1:$C$3"));
            continue;
        }
        assert!(xml.contains("$C$1:$C$3"), "{group}: {xml}");
        assert!(xml.contains("idx val=\"7\""), "{group}: {xml}");
        assert_eq!(xml.matches(&format!("<c15:{filtered_tag}>")).count(), 1);
        assert_eq!(xml.matches(FILTERING_URI).count(), 1);
    }
}

#[test]
fn skipped_series_extensions_do_not_admit_or_request_their_string_caches() {
    let cache = concat!(
        "<c:strCache><c:ptCount val=\"1\"/>",
        "<c:pt idx=\"0\"><c:v>A</c:v></c:pt></c:strCache>"
    );
    let modeled = [
        "<c:idx val=\"0\"/><c:order val=\"0\"/>",
        "<c:cat><c:strRef><c:f>Data!$A$1</c:f>",
        cache,
        "</c:strRef></c:cat><c:val><c:numRef><c:f>Sheet1!$B$1</c:f></c:numRef></c:val>",
    ]
    .concat();
    let skipped = [
        format!("<c:extLst><c:ext uri=\"{FILTERING_URI}\" xmlns:c15=\"{C15_NS}\">"),
        "<c15:filteredLineSeries><c15:ser><c:cat><c:strRef><c:f>Data!$B$1</c:f>".into(),
        cache.to_string(),
        "</c:strRef></c:cat></c15:ser></c15:filteredLineSeries></c:ext></c:extLst>".into(),
    ]
    .concat();
    let mut initial = base_book(&ChartType::LineChart);
    initial.sheet_mut(0).unwrap().set_name("Host");
    initial.new_sheet("Data").unwrap();
    initial
        .sheet_by_name_mut("Data")
        .unwrap()
        .cell_mut("A1")
        .set_value_number(1);
    let bytes = with_chart_part(&write(&initial), |xml| {
        let family = format!(
            "<c:extLst><c:ext \
             uri=\"family\"><c:cat><c:strRef><c:f>Data!$C$1</c:f>{cache}</c:strRef></c:cat></c:\
             ext></c:extLst>"
        );
        let chart = format!(
            "<c:extLst><c:ext \
             uri=\"chart\"><c:cat><c:strRef><c:f>Data!$D$1</c:f>{cache}</c:strRef></c:cat></c:\
             ext></c:extLst>"
        );
        replace_between(
            xml,
            "<c:ser>",
            "</c:ser>",
            &format!("<c:ser>{modeled}{skipped}</c:ser>"),
        )
        .replacen("</c:lineChart>", &format!("{family}</c:lineChart>"), 1)
        .replacen("</c:chart>", &format!("{chart}</c:chart>"), 1)
        .replacen(
            "<c:grouping val=\"stacked\"/>",
            "<c:grouping val=\"standard\"/>",
            1,
        )
    });
    let input_xml = chart_part(&bytes);
    assert_eq!(input_xml.matches("<c:ser>").count(), 1, "{input_xml}");
    assert!(
        input_xml.contains("<c:grouping val=\"standard\"/>"),
        "{input_xml}"
    );
    assert!(
        input_xml.contains("<c:varyColors val=\"0\"/>"),
        "{input_xml}"
    );
    let input_series = between(&input_xml, "<c:ser>", "</c:ser>");
    assert!(
        input_series.starts_with("<c:ser><c:idx val=\"0\"/><c:order val=\"0\"/>"),
        "{input_series}"
    );
    for formula in ["Data!$A$1", "Data!$B$1"] {
        assert!(input_series.contains(formula), "{input_series}");
    }
    for formula in ["Data!$C$1", "Data!$D$1"] {
        assert!(!input_series.contains(formula), "{input_series}");
        assert!(input_xml.contains(formula), "{input_xml}");
    }
    let mut book = umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(bytes), false).unwrap();
    book.sheet_mut(0).unwrap();
    let report = book.chart_source_cache_report();
    assert_eq!(
        (
            report.kept_caches,
            report.kept_points,
            report.kept_text_bytes,
            report.dropped_caches,
            report.reduced_caches,
            report.over_cap_caches,
            report.over_cap_points,
            report.over_cap_text_bytes,
        ),
        (1, 1, 2, 0, 0, 0, 0, 0)
    );
    let chart = chart_of(&book);
    let line = chart
        .chart_space()
        .chart()
        .plot_area()
        .line_chart()
        .unwrap();
    assert!(matches!(
        line.grouping().val(),
        umya_spreadsheet::drawing::charts::GroupingValues::Standard
    ));
    assert!(!line.vary_colors().val());
    assert_eq!(
        chart
            .chart_space()
            .chart()
            .plot_area()
            .line_chart()
            .unwrap()
            .area_chart_series_list()
            .area_chart_series()
            .len(),
        1
    );
    let category = chart
        .chart_space()
        .chart()
        .plot_area()
        .line_chart()
        .unwrap()
        .area_chart_series_list()
        .area_chart_series()[0]
        .category_axis_data()
        .unwrap()
        .string_reference()
        .unwrap();
    let source = category.string_cache().source().unwrap();
    let series = &line.area_chart_series_list().area_chart_series()[0];
    assert_eq!(series.index().val(), 0);
    assert_eq!(series.order().val(), 0);
    assert_eq!(
        series
            .values()
            .unwrap()
            .number_reference()
            .formula()
            .address_str(),
        "Sheet1!$B$1"
    );
    assert_eq!(source.formula(), "Data!$A$1");
    assert_eq!(source.point_count(), Some(1));
    assert!(source.index_runs().is_empty());
    assert_eq!(source.exceptions().len(), 1);
    assert_eq!(source.exceptions()[0].index(), 0);
    assert_eq!(source.exceptions()[0].value(), Some("A"));
    assert_eq!(source.exceptions()[0].loaded_display(), Some("1"));
    let written = chart_part(&write(&book));
    assert_eq!(
        between(&written, "<c:cat>", "</c:cat>"),
        "<c:cat><c:strRef><c:f>Data!$A$1</c:f><c:strCache><c:ptCount val=\"1\"/><c:pt \
         idx=\"0\"><c:v>A</c:v></c:pt></c:strCache></c:strRef></c:cat>"
    );
    // Invalid registered series (missing idx/order) remain diagnosed and
    // omitted; the unchanged report above proves no cache was admitted.
    assert!(!written.contains("Data!$B$1"), "{written}");
    assert_eq!(
        series.extensions().unwrap().conflict(),
        None,
        "opaque stored syntax does not invent a grammar refusal"
    );
    let diagnostic = series.extensions().unwrap().export_diagnostics_for_owner(
        Some(&book),
        "ser",
        Some(umya_spreadsheet::drawing::charts::SeriesFamily::Line),
    );
    assert_eq!(diagnostic.len(), 1);
    assert_eq!(diagnostic[0].reason, "NotAllowed");
    // An unknown group extension stays opaque, including its native-looking
    // reference/cache subtree; that language never joins chart cache roles.
    assert!(written.contains("Data!$C$1"), "{written}");
    assert_eq!(
        line.extensions().unwrap().entries[0].uri.as_deref(),
        Some("family")
    );
    // Unsupported chart-root extension ownership is unchanged at this stage.
    assert!(!written.contains("Data!$D$1"), "{written}");
}

#[test]
fn nested_leader_lines_extension_does_not_set_the_ordinary_leader_lines() {
    let labels = [
        "<c:dLbls><c:showLegendKey val=\"0\"/><c:showVal val=\"1\"/>",
        "<c:showCatName val=\"0\"/><c:showSerName val=\"0\"/>",
        "<c:showPercent val=\"0\"/><c:showBubbleSize val=\"0\"/>",
        "<c:extLst><c:ext uri=\"{CE6537A1-D6FC-4f65-9D91-7224C49458BB}\" ",
        "xmlns:c15=\"http://schemas.microsoft.com/office/drawing/2012/chart\">",
        "<c15:showLeaderLines val=\"1\"/></c:ext></c:extLst></c:dLbls>",
    ]
    .concat();
    let series = ["<c:idx val=\"0\"/><c:order val=\"0\"/>", &labels, REFS].concat();
    let book = read_with_series(&ChartType::LineChart, &series);
    let series_labels = first_series(&book, &ChartType::LineChart)
        .data_labels()
        .expect("series dLbls")
        .clone();
    assert!(series_labels.show_leader_lines().is_none());
    assert!(series_labels.show_value().val());

    let book = read_with_chart_edit(|xml| xml.replacen("<c:axId", &format!("{labels}<c:axId"), 1));
    let plot = chart_of(&book).chart_space().chart().plot_area();
    let group = plot.line_chart().unwrap();
    assert!(group.data_labels().show_leader_lines().is_none());
    assert!(chart_part(&write(&book)).contains("c15:showLeaderLines"));
}

#[test]
fn unmodeled_chart_and_chart_space_parts_do_not_change_their_parents_fields() {
    let plain = write(&base_book(&ChartType::LineChart));
    let expected = chart_part(&write(&read(&plain)));
    let chart_extension = concat!(
        "<c:extLst><c:ext uri=\"{56B9EC1D-385E-4148-901F-78D8002777C0}\">",
        "<c:autoTitleDeleted val=\"1\"/><c:plotVisOnly val=\"0\"/>",
        "<c:dispBlanksAs val=\"span\"/><c:showDLblsOverMax val=\"1\"/>",
        "</c:ext></c:extLst>",
    );
    let chart_space_extension = concat!(
        "<c:extLst><c:ext uri=\"{781A3756-C4B2-4CAC-9D66-4F8BD8637D16}\">",
        "<c:lang val=\"fr-FR\"/><c:roundedCorners val=\"1\"/>",
        "<c:date1904 val=\"1\"/>",
        "<c:spPr><a:solidFill><a:srgbClr val=\"ff0000\"/></a:solidFill></c:spPr>",
        "</c:ext></c:extLst>",
    );
    let patched = with_chart_part(&plain, |xml| {
        xml.replacen("</c:chart>", &format!("{chart_extension}</c:chart>"), 1)
            .replacen(
                "</c:chartSpace>",
                &format!("{chart_space_extension}</c:chartSpace>"),
                1,
            )
    });
    assert_eq!(chart_part(&write(&read(&patched))), expected);
}
