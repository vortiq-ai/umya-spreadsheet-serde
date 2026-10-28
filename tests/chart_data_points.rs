//! Point overrides are independent from series identity and formatting.
#![cfg(feature = "serde")]
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
        charts::{
            AreaChartSeries,
            DataPoint,
            Explosion,
            MarkerStyleValues,
            SeriesFamily,
            SeriesPartConflictReason,
            SeriesPartKind,
            Size,
            XmlDouble,
        },
        spreadsheet::MarkerType,
    },
};
fn marker(coordinate: &str) -> MarkerType {
    let mut marker = MarkerType::default();
    marker.set_coordinate(coordinate);
    marker
}

fn base_book(chart_type: &ChartType, series: usize) -> Workbook {
    let mut book = umya_spreadsheet::new_file();
    let sheet = book.sheet_mut(0).unwrap();
    for row in 1..=3 {
        sheet
            .cell_mut((1, row))
            .set_value_string(format!("Row {row}"));
        sheet.cell_mut((2, row)).set_value_number(row * 10);
        sheet.cell_mut((3, row)).set_value_number(row * 100);
    }
    let sources = ["Sheet1!$B$1:$B$3", "Sheet1!$C$1:$C$3"];
    // A scatter series takes two references and a bubble series three; a
    // doughnut series takes its categories, then its values.
    let sources = match chart_type {
        ChartType::DoughnutChart => vec!["Sheet1!$A$1:$A$3", sources[0]],
        ChartType::ScatterChart => vec![sources[0], sources[1]],
        ChartType::BubbleChart => vec![sources[0], sources[1], sources[1]],
        _ => sources[..series].to_vec(),
    };
    let mut chart = Chart::default();
    chart.new_chart(chart_type, marker("E1"), marker("M18"), sources);
    sheet.add_chart(chart);
    book
}

fn write(book: &Workbook) -> Vec<u8> {
    let mut bytes = Vec::new();
    umya_spreadsheet::writer::xlsx::write_writer(book, &mut Cursor::new(&mut bytes)).unwrap();
    bytes
}

fn read(bytes: &[u8]) -> Workbook {
    umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap()
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

fn replace_first_series(xml: &str, series: &str) -> String {
    let start = xml.find("<c:ser>").expect("a series in the chart part");
    let end = start + xml[start..].find("</c:ser>").expect("series end") + "</c:ser>".len();
    format!("{}<c:ser>{series}</c:ser>{}", &xml[..start], &xml[end..])
}

fn chart_of(book: &Workbook) -> &Chart {
    &book.sheet(0).unwrap().chart_collection()[0]
}

fn first_series(book: &Workbook) -> &AreaChartSeries {
    &chart_of(book)
        .chart_space()
        .chart()
        .plot_area()
        .chart_groups()
        .next()
        .expect("one chart group")
        .series()
        .area_chart_series()[0]
}
fn series_xml(book: &Workbook) -> String {
    let xml = chart_part(&write(book));
    let start = xml.find("<c:ser>").unwrap();
    let end = start + xml[start..].find("</c:ser>").unwrap() + 8;
    xml[start..end].to_string()
}
fn input(kind: &ChartType, children: &str) -> Workbook {
    let bytes = write(&base_book(kind, 1));
    read(&with_chart_part(&bytes, |xml| {
        replace_first_series(
            xml,
            &format!("<c:idx val=\"1\"/><c:order val=\"1\"/>{children}"),
        )
    }))
}
fn persisted(book: &Workbook) -> [Workbook; 2] {
    let chart = chart_of(book);
    let json: Chart = serde_json::from_str(&serde_json::to_string(chart).unwrap()).unwrap();
    let binary: Chart = bincode::deserialize(&bincode::serialize(chart).unwrap()).unwrap();
    let bytes = write(book);
    [json, binary].map(|chart| {
        let mut book = read(&bytes);
        book.sheet_mut(0).unwrap().chart_collection_mut()[0] = chart;
        book
    })
}
fn point_xml(series: &str) -> Vec<&str> {
    let mut tail = series;
    let mut result = vec![];
    while let Some(start) = tail.find("<c:dPt>") {
        tail = &tail[start..];
        let end = tail.find("</c:dPt>").unwrap() + 8;
        result.push(&tail[..end]);
        tail = &tail[end..];
    }
    result
}
const POINT: &str = concat!(
    "<c:dPt><c:idx val=\"9\"/><c:invertIfNegative val=\"false\"/>",
    "<c:marker><c:symbol val=\"circle\"/><c:size/><c:spPr><a:solidFill><a:srgbClr \
     val=\"ED7D31\"/></a:solidFill></c:spPr></c:marker>",
    "<c:bubble3D/><c:explosion val=\"25\"/><c:spPr><a:noFill/><a:ln \
     w=\"12700\"><a:solidFill><a:schemeClr val=\"accent1\"><a:lumMod val=\"50000\"/><a:lumOff \
     val=\"20000\"/></a:schemeClr></a:solidFill><a:prstDash \
     val=\"dash\"/><a:round/></a:ln><a:effectLst/></c:spPr>",
    "<c:pictureOptions><c:applyToFront/><c:applyToSides val=\"0\"/><c:applyToEnd \
     val=\"true\"/><c:pictureFormat val=\"stackScale\"/><c:pictureStackUnit \
     val=\"1.2500E+1\"/></c:pictureOptions></c:dPt>"
);

#[test]
fn point_children_keep_schema_order_duplicate_indices_and_scalar_spelling() {
    for kind in [
        ChartType::BarChart,
        ChartType::LineChart,
        ChartType::AreaChart,
        ChartType::PieChart,
        ChartType::ScatterChart,
        ChartType::BubbleChart,
        ChartType::RadarChart,
    ] {
        let second = "<c:dPt><c:idx val=\"9\"/><c:invertIfNegative val=\"1\"/></c:dPt>";
        let book = input(&kind, &format!("{POINT}{second}<c:dLbls/>"));
        let series = first_series(&book);
        assert_eq!(series.data_points().len(), 2);
        assert_eq!(series.index().val(), 1);
        assert_eq!(series.order().val(), 1);
        assert_eq!(series.data_points()[0].index().val(), Some(9));
        assert_eq!(series.data_points()[1].index().val(), Some(9));
        let point = &series.data_points()[0];
        assert_eq!(point.invert_if_negative().unwrap().value(), Some(false));
        assert_eq!(point.bubble_3d().unwrap().value(), Some(true));
        assert_eq!(
            point
                .picture_options()
                .unwrap()
                .stack_unit()
                .unwrap()
                .attribute(),
            Some("1.2500E+1")
        );
        assert_eq!(
            point
                .picture_options()
                .unwrap()
                .stack_unit()
                .unwrap()
                .value(),
            Some(12.5)
        );
        assert!(point.shape_properties().is_some());
        assert!(series.shape_properties().is_none());
        assert!(
            chart_of(&book)
                .chart_space()
                .chart()
                .plot_area()
                .series_part_conflicts()
                .is_empty()
        );
        if let Some(directory) = std::env::var_os("UMYA_POINT_EXPORT_DIR") {
            let directory = std::path::PathBuf::from(directory);
            std::fs::create_dir_all(&directory).unwrap();
            std::fs::write(directory.join(format!("{kind:?}.xlsx")), write(&book)).unwrap();
            for (index, restored) in persisted(&book).iter().enumerate() {
                std::fs::write(
                    directory.join(format!("{kind:?}-reload-{index}.xlsx")),
                    write(restored),
                )
                .unwrap();
            }
        }
        let expected = format!("{POINT}{second}");
        for restored in std::iter::once(book.clone()).chain(persisted(&book)) {
            let xml = series_xml(&restored);
            let points = point_xml(&xml).concat();
            assert_eq!(points, expected, "{kind:?}");
            assert!(xml.find("</c:dPt>").unwrap() < xml.find("<c:dLbls>").unwrap());
            let reloaded = read(&write(&restored));
            assert_eq!(point_xml(&series_xml(&reloaded)).concat(), expected);
        }
    }
}

#[test]
fn point_marker_is_distinct_from_line_identity_and_series_marker() {
    let point = "<c:dPt><c:idx val=\"9\"/><c:marker><c:symbol val=\"circle\"/><c:size \
                 val=\"5\"/><c:spPr><a:solidFill><a:srgbClr \
                 val=\"ED7D31\"/></a:solidFill></c:spPr></c:marker></c:dPt>";
    let series_marker = "<c:marker><c:symbol val=\"diamond\"/><c:size val=\"7\"/></c:marker>";
    let book = input(&ChartType::LineChart, &format!("{series_marker}{point}"));
    for restored in std::iter::once(book.clone()).chain(persisted(&book)) {
        let series = first_series(&restored);
        assert_eq!((series.index().val(), series.order().val()), (1, 1));
        assert!(matches!(
            series.marker().unwrap().symbol().unwrap().val(),
            MarkerStyleValues::Diamond
        ));
        assert!(matches!(
            series.data_points()[0]
                .marker()
                .unwrap()
                .symbol()
                .unwrap()
                .val(),
            MarkerStyleValues::Circle
        ));
        let xml = series_xml(&restored);
        assert!(xml.contains(series_marker));
        assert_eq!(point_xml(&xml), vec![point]);
    }
}

#[test]
fn chart_prefix_and_default_namespace_do_not_change_point_ownership() {
    let bytes = write(&base_book(&ChartType::LineChart, 1));
    let original = with_chart_part(&bytes, |xml| {
        replace_first_series(
            xml,
            &format!("<c:idx val=\"1\"/><c:order val=\"1\"/>{POINT}"),
        )
    });
    for default in [false, true] {
        let alternate = with_chart_part(&original, |xml| {
            if default {
                xml.replace("xmlns:c=", "xmlns=")
                    .replace("<c:", "<")
                    .replace("</c:", "</")
            } else {
                xml.replace("xmlns:c=", "xmlns:q=")
                    .replace("<c:", "<q:")
                    .replace("</c:", "</q:")
            }
        });
        let book = read(&alternate);
        assert_eq!(first_series(&book).index().val(), 1);
        assert_eq!(point_xml(&series_xml(&book)), vec![POINT]);
    }
}

#[test]
fn type_change_removes_only_incompatible_series_parts_not_points() {
    let mut book = input(
        &ChartType::LineChart,
        &format!("<c:marker><c:symbol val=\"diamond\"/></c:marker>{POINT}<c:smooth val=\"1\"/>"),
    );
    let line = book.sheet_mut(0).unwrap().chart_collection_mut()[0]
        .chart_space_mut()
        .chart_mut()
        .plot_area_mut()
        .line_chart_mut()
        .unwrap();
    let list = line.area_chart_series_list_mut();
    let points_before = serde_json::to_value(list.area_chart_series()[0].data_points()).unwrap();
    let removed = list.remove_parts_not_allowed_in(SeriesFamily::Pie);
    assert_eq!(
        removed.iter().map(|r| r.part).collect::<Vec<_>>(),
        vec![SeriesPartKind::Marker, SeriesPartKind::Smooth]
    );
    assert_eq!(
        serde_json::to_value(list.area_chart_series()[0].data_points()).unwrap(),
        points_before
    );
    assert!(
        list.remove_parts_not_allowed_in(SeriesFamily::Pie)
            .is_empty()
    );
}

#[test]
fn invalid_required_point_and_symbol_values_are_reported_without_fabrication() {
    let book = input(
        &ChartType::LineChart,
        concat!(
            "<c:marker><c:symbol/></c:marker>",
            "<c:dPt><c:marker><c:size/></c:marker></c:dPt>",
            "<c:dPt><c:idx/><c:bubble3D/></c:dPt>",
            "<c:dPt><c:idx val=\"3\"/><c:marker><c:symbol/></c:marker></c:dPt>",
            "<c:dPt><c:idx \
             val=\"4\"/><c:pictureOptions><c:pictureStackUnit/></c:pictureOptions></c:dPt>",
            "<c:dPt><c:idx val=\"5\"/><c:pictureOptions><c:pictureStackUnit \
             val=\"0\"/></c:pictureOptions></c:dPt>"
        ),
    );
    for restored in std::iter::once(book.clone()).chain(persisted(&book)) {
        let points = first_series(&restored).data_points();
        assert_eq!(points.len(), 5);
        assert!(!points[0].has_index());
        assert!(!points[1].has_index());
        assert!(!points[2].marker().unwrap().symbol().unwrap().has_val());
        let conflicts = chart_of(&restored)
            .chart_space()
            .chart()
            .plot_area()
            .series_part_conflicts();
        assert_eq!(conflicts.len(), 6);
        assert_eq!(conflicts[0].part, SeriesPartKind::Marker);
        assert_eq!(
            conflicts[1].reason,
            SeriesPartConflictReason::MissingRequiredChild
        );
        assert_eq!(
            conflicts[2].reason,
            SeriesPartConflictReason::MissingRequiredChild
        );
        assert!(
            conflicts[3..]
                .iter()
                .all(|c| c.reason == SeriesPartConflictReason::InvalidValue)
        );
        assert!(point_xml(&series_xml(&restored)).is_empty());
        assert!(!series_xml(&restored).contains("<c:symbol"));
    }
}

#[test]
#[allow(deprecated)]
fn marker_size_presence_survives_current_and_deprecated_edits_and_reload() {
    let mut book = input(
        &ChartType::LineChart,
        "<c:dPt><c:idx val=\"9\"/><c:marker><c:size/><c:spPr/></c:marker></c:dPt>",
    );
    assert_eq!(
        first_series(&book).data_points()[0]
            .marker()
            .unwrap()
            .size()
            .unwrap()
            .val(),
        5
    );
    assert!(series_xml(&book).contains("<c:size/>"));
    let series = &mut book.sheet_mut(0).unwrap().chart_collection_mut()[0]
        .chart_space_mut()
        .chart_mut()
        .plot_area_mut()
        .line_chart_mut()
        .unwrap()
        .area_chart_series_list_mut()
        .area_chart_series_mut()[0];
    series.data_points_mut()[0]
        .marker_mut()
        .unwrap()
        .get_size_mut()
        .unwrap()
        .set_val(5);
    let restored = persisted(&book);
    for mut book in restored {
        assert!(series_xml(&book).contains("<c:size val=\"5\"/>"));
        let series = &mut book.sheet_mut(0).unwrap().chart_collection_mut()[0]
            .chart_space_mut()
            .chart_mut()
            .plot_area_mut()
            .line_chart_mut()
            .unwrap()
            .area_chart_series_list_mut()
            .area_chart_series_mut()[0];
        series.data_points_mut()[0]
            .marker_mut()
            .unwrap()
            .size_mut()
            .unwrap()
            .set_val(8);
        assert!(series_xml(&read(&write(&book))).contains("<c:size val=\"8\"/>"));
    }
}

#[test]
fn absent_supplement_defaults_on_old_json_and_costs_one_binary_option_byte() {
    let series = AreaChartSeries::default();
    let mut json = serde_json::to_value(&series).unwrap();
    assert!(
        json.as_object_mut()
            .unwrap()
            .remove("parts")
            .unwrap()
            .is_null()
    );
    let old = serde_json::to_string(&json).unwrap();
    let restored: AreaChartSeries = serde_json::from_str(&old).unwrap();
    assert!(restored.parts().is_none());
    assert_eq!(
        serde_json::to_string(&series).unwrap().len(),
        old.len() + 13
    );
    let bytes = bincode::serialize(&series).unwrap();
    // Two original scalar-option leaves and thirteen original optional
    // children, plus parts.
    assert_eq!(bytes, vec![0; 16]);
    // Positional bincode is intentionally versioned by its callers, not
    // default-compatible.
    assert!(bincode::deserialize::<AreaChartSeries>(&bytes[..bytes.len() - 1]).is_err());
    let mut invalid = series;
    invalid.data_points_mut().push(DataPoint::default());
    let _: AreaChartSeries = bincode::deserialize(&bincode::serialize(&invalid).unwrap()).unwrap();
}

#[test]
fn imported_combo_preserves_point_marker_through_persistence_and_export() {
    let bytes = include_bytes!("fixtures/fm_12_05_combo.xlsx");
    let book = read(bytes);
    let charts: Vec<_> = book
        .sheet_collection()
        .iter()
        .flat_map(|s| s.chart_collection())
        .collect();
    assert_eq!(charts.len(), 1);
    let chart = charts[0];
    let series = chart
        .chart_space()
        .chart()
        .plot_area()
        .chart_groups()
        .into_iter()
        .flat_map(|g| g.series().area_chart_series())
        .find(|s| !s.data_points().is_empty())
        .unwrap();
    assert_eq!((series.index().val(), series.order().val()), (1, 1));
    assert_eq!(series.data_points().len(), 1);
    assert_eq!(series.data_points()[0].index().val(), Some(9));
    assert!(matches!(
        series.data_points()[0]
            .marker()
            .unwrap()
            .symbol()
            .unwrap()
            .val(),
        MarkerStyleValues::Circle
    ));
    let expected = "<c:dPt><c:idx val=\"9\"/><c:marker><c:symbol val=\"circle\"/><c:size \
                    val=\"5\"/><c:spPr><a:solidFill><a:srgbClr \
                    val=\"ed7d31\"/></a:solidFill></c:spPr></c:marker></c:dPt>";
    let json: Chart = serde_json::from_str(&serde_json::to_string(chart).unwrap()).unwrap();
    let binary: Chart = bincode::deserialize(&bincode::serialize(chart).unwrap()).unwrap();
    for chart in [chart.clone(), json, binary] {
        let mut copy = book.clone();
        let host = copy
            .sheet_collection_mut()
            .iter_mut()
            .find(|s| !s.chart_collection().is_empty())
            .unwrap();
        host.chart_collection_mut()[0] = chart;
        assert_eq!(point_xml(&chart_part(&write(&copy))), vec![expected]);
        assert_eq!(
            point_xml(&chart_part(&write(&read(&write(&copy))))),
            vec![expected]
        );
    }
}

#[test]
fn point_boolean_spelling_and_picture_format_presence_survive_serde() {
    let children = concat!(
        "<c:dPt><c:idx val=\"0\"/><c:invertIfNegative/><c:bubble3D \
         val=\"false\"/><c:pictureOptions><c:applyToFront val=\"0\"/><c:applyToSides \
         val=\"1\"/><c:applyToEnd val=\"true\"/><c:pictureFormat \
         val=\"stretch\"/></c:pictureOptions></c:dPt>",
        "<c:dPt><c:idx val=\"1\"/><c:pictureOptions/></c:dPt>"
    );
    let book = input(&ChartType::LineChart, children);
    for restored in std::iter::once(book.clone()).chain(persisted(&book)) {
        let points = first_series(&restored).data_points();
        assert!(
            points[0]
                .invert_if_negative()
                .unwrap()
                .attribute()
                .is_none()
        );
        assert_eq!(points[0].invert_if_negative().unwrap().value(), Some(true));
        let picture = points[0].picture_options().unwrap();
        assert_eq!(picture.apply_to_front().unwrap().value(), Some(false));
        assert_eq!(picture.apply_to_sides().unwrap().value(), Some(true));
        assert_eq!(picture.apply_to_end().unwrap().value(), Some(true));
        assert!(picture.format().unwrap().attribute().is_some());
        assert!(points[1].picture_options().unwrap().format().is_none());
        let expected = children.replace(
            "<c:pictureOptions/>",
            "<c:pictureOptions></c:pictureOptions>",
        );
        assert_eq!(point_xml(&series_xml(&restored)).concat(), expected);
    }
}

#[test]
fn unmodeled_point_extensions_cannot_overwrite_typed_point_or_parent_fields() {
    let book = input(
        &ChartType::LineChart,
        concat!(
            "<c:dPt><c:idx val=\"9\"/><c:extLst><c:ext uri=\"future\"><c:idx \
             val=\"77\"/><c:marker><c:symbol val=\"triangle\"/></c:marker></c:ext></c:extLst>",
            "<c:pictureOptions><c:future><c:applyToFront val=\"0\"/></c:future><c:applyToEnd \
             val=\"true\"/></c:pictureOptions></c:dPt>"
        ),
    );
    let series = first_series(&book);
    assert_eq!(series.index().val(), 1);
    assert_eq!(series.data_points()[0].index().val(), Some(9));
    assert!(series.marker().is_none());
    assert!(series.data_points()[0].marker().is_none());
    let picture = series.data_points()[0].picture_options().unwrap();
    assert!(picture.apply_to_front().is_none());
    assert_eq!(picture.apply_to_end().unwrap().value(), Some(true));
    // The actual extension owner now retains opaque future children; they
    // must not overwrite any independently asserted typed point/series field.
    let extension = series.data_points()[0].extensions().unwrap();
    assert_eq!(extension.entries.len(), 1);
    assert_eq!(extension.entries[0].uri.as_deref(), Some("future"));
    assert_eq!(extension.entries[0].nodes.len(), 2);
    let output = series_xml(&book);
    assert!(output.contains("uri=\"future\""));
    assert!(output.contains("val=\"77\""));
    assert!(output.contains("val=\"triangle\""));
}

#[test]
fn alternate_drawing_prefix_keeps_point_fill_line_and_color_transforms() {
    let point = POINT
        .replace(
            "<c:dPt>",
            "<c:dPt xmlns:d=\"http://schemas.openxmlformats.org/drawingml/2006/main\">",
        )
        .replace("<a:", "<d:")
        .replace("</a:", "</d:");
    let book = input(&ChartType::LineChart, &point);
    assert_eq!(point_xml(&series_xml(&book)), vec![POINT]);
}

#[test]
fn scalar_setters_canonicalize_only_the_changed_attribute() {
    let mut book = input(&ChartType::LineChart, POINT);
    let series = &mut book.sheet_mut(0).unwrap().chart_collection_mut()[0]
        .chart_space_mut()
        .chart_mut()
        .plot_area_mut()
        .line_chart_mut()
        .unwrap()
        .area_chart_series_list_mut()
        .area_chart_series_mut()[0];
    let point = &mut series.data_points_mut()[0];
    point.invert_if_negative_mut().unwrap().set_value(false);
    point
        .picture_options_mut()
        .unwrap()
        .stack_unit_mut()
        .unwrap()
        .set_value(25.0);
    let expected = POINT
        .replace(
            "invertIfNegative val=\"false\"",
            "invertIfNegative val=\"0\"",
        )
        .replace("1.2500E+1", "25");
    for restored in persisted(&book) {
        assert_eq!(point_xml(&series_xml(&restored)), vec![expected.as_str()]);
    }
}

#[test]
fn missing_required_picture_format_and_explosion_values_remain_invalid() {
    let book = input(
        &ChartType::LineChart,
        concat!(
            "<c:dPt><c:idx val=\"1\"/><c:explosion/></c:dPt>",
            "<c:dPt><c:idx \
             val=\"2\"/><c:pictureOptions><c:pictureFormat/></c:pictureOptions></c:dPt>"
        ),
    );
    for restored in std::iter::once(book.clone()).chain(persisted(&book)) {
        let series = first_series(&restored);
        assert!(!series.data_points()[0].explosion().unwrap().has_val());
        assert!(
            series.data_points()[1]
                .picture_options()
                .unwrap()
                .format()
                .unwrap()
                .attribute()
                .is_none()
        );
        assert_eq!(
            chart_of(&restored)
                .chart_space()
                .chart()
                .plot_area()
                .series_part_conflicts()
                .len(),
            2
        );
        assert!(point_xml(&series_xml(&restored)).is_empty());
    }
}

fn save_witness(name: &str, book: &Workbook) {
    if let Some(directory) = std::env::var_os("UMYA_POINT_EXPORT_DIR") {
        let directory = std::path::PathBuf::from(directory).join("witnesses");
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join(format!("{name}-original.xlsx")), write(book)).unwrap();
        for (index, restored) in persisted(book).iter().enumerate() {
            std::fs::write(
                directory.join(format!("{name}-reload-{index}.xlsx")),
                write(restored),
            )
            .unwrap();
        }
    }
}

#[test]
fn xml_boolean_whitespace_preserves_false_semantics_and_source_spelling() {
    let point = concat!(
        "<c:dPt><c:idx val=\"9\"/><c:invertIfNegative val=\" false \"/><c:bubble3D val=\" 0 \"/>",
        "<c:pictureOptions><c:applyToFront val=\" false \"/><c:applyToSides val=\" 0 \
         \"/><c:applyToEnd val=\" true \"/></c:pictureOptions></c:dPt>"
    );
    let book = input(&ChartType::LineChart, point);
    for restored in std::iter::once(book.clone()).chain(persisted(&book)) {
        let p = &first_series(&restored).data_points()[0];
        assert_eq!(p.invert_if_negative().unwrap().value(), Some(false));
        assert_eq!(p.invert_if_negative().unwrap().attribute(), Some(" false "));
        assert_eq!(p.bubble_3d().unwrap().value(), Some(false));
        assert_eq!(
            p.picture_options()
                .unwrap()
                .apply_to_front()
                .unwrap()
                .value(),
            Some(false)
        );
        assert_eq!(
            p.picture_options()
                .unwrap()
                .apply_to_sides()
                .unwrap()
                .value(),
            Some(false)
        );
        assert_eq!(
            p.picture_options().unwrap().apply_to_end().unwrap().value(),
            Some(true)
        );
        assert_eq!(point_xml(&series_xml(&restored)), vec![point]);
        assert!(
            chart_of(&restored)
                .chart_space()
                .chart()
                .plot_area()
                .series_part_conflicts()
                .is_empty()
        );
    }
    save_witness("whitespace-booleans", &book);
}

#[test]
fn invalid_present_booleans_are_stored_reported_and_never_changed_to_true() {
    for name in [
        "invertIfNegative",
        "bubble3D",
        "applyToFront",
        "applyToSides",
        "applyToEnd",
    ] {
        for lexical in ["yes", "", " FALSE ", "\u{a0}false\u{a0}"] {
            let leaf = format!("<c:{name} val=\"{lexical}\"/>");
            let children = if name.starts_with("apply") {
                format!("<c:pictureOptions>{leaf}</c:pictureOptions>")
            } else {
                leaf
            };
            let book = input(
                &ChartType::LineChart,
                &format!("<c:dPt><c:idx val=\"9\"/>{children}</c:dPt>"),
            );
            for restored in std::iter::once(book.clone()).chain(persisted(&book)) {
                let p = &first_series(&restored).data_points()[0];
                let value = match name {
                    "invertIfNegative" => p.invert_if_negative().unwrap(),
                    "bubble3D" => p.bubble_3d().unwrap(),
                    "applyToFront" => p.picture_options().unwrap().apply_to_front().unwrap(),
                    "applyToSides" => p.picture_options().unwrap().apply_to_sides().unwrap(),
                    _ => p.picture_options().unwrap().apply_to_end().unwrap(),
                };
                assert_eq!(value.attribute(), Some(lexical));
                assert_eq!(value.value(), None);
                assert!(!value.is_valid());
                let conflicts = chart_of(&restored)
                    .chart_space()
                    .chart()
                    .plot_area()
                    .series_part_conflicts();
                assert_eq!(conflicts.len(), 1);
                assert_eq!(
                    (
                        conflicts[0].group_index,
                        conflicts[0].series_index,
                        conflicts[0].part,
                        conflicts[0].occurrence,
                        conflicts[0].reason
                    ),
                    (
                        0,
                        0,
                        SeriesPartKind::DataPoints,
                        0,
                        SeriesPartConflictReason::InvalidValue
                    )
                );
                assert!(point_xml(&series_xml(&restored)).is_empty());
                assert_eq!(
                    value.attribute(),
                    Some(lexical),
                    "export must not modify stored invalid state"
                );
            }
        }
    }
}

#[test]
fn point_unsigned_values_keep_valid_whitespace_and_do_not_fabricate_invalid_numbers() {
    let mut children = String::new();
    for (lexical, expected) in [
        (" 9 ", 9),
        (" 0009 ", 9),
        ("0", 0),
        ("4294967295", u32::MAX),
    ] {
        children.push_str(&format!(
            "<c:dPt><c:idx val=\"{lexical}\"/><c:marker><c:size val=\" 7 \
             \"/></c:marker><c:explosion val=\"{lexical}\"/></c:dPt>"
        ));
        let book = input(&ChartType::LineChart, &children);
        let point = first_series(&book).data_points().last().unwrap();
        assert_eq!(point.index().val(), Some(expected));
        assert_eq!(point.explosion().unwrap().parsed_val(), Some(expected));
        assert_eq!(
            point.marker().unwrap().size().unwrap().parsed_val(),
            Some(7)
        );
    }
    let book = input(&ChartType::LineChart, &children);
    for restored in std::iter::once(book.clone()).chain(persisted(&book)) {
        assert_eq!(point_xml(&series_xml(&restored)).concat(), children);
        assert!(
            chart_of(&restored)
                .chart_space()
                .chart()
                .plot_area()
                .series_part_conflicts()
                .is_empty()
        );
    }
    save_witness("whitespace-unsigned", &book);
    for lexical in [
        "abc",
        "-1",
        "4294967296",
        "",
        "\u{a0}9\u{a0}",
        "++9",
        "+9",
        "-0",
    ] {
        for leaf in ["idx", "explosion"] {
            let children = if leaf == "idx" {
                format!("<c:idx val=\"{lexical}\"/>")
            } else {
                format!("<c:idx val=\"9\"/><c:explosion val=\"{lexical}\"/>")
            };
            let book = input(&ChartType::LineChart, &format!("<c:dPt>{children}</c:dPt>"));
            for restored in std::iter::once(book.clone()).chain(persisted(&book)) {
                let point = &first_series(&restored).data_points()[0];
                if leaf == "idx" {
                    assert_eq!(point.index().attribute(), Some(lexical));
                    assert_eq!(point.index().val(), None);
                } else {
                    assert_eq!(point.explosion().unwrap().lexical_val(), Some(lexical));
                    assert_eq!(point.explosion().unwrap().parsed_val(), None);
                }
                let conflicts = chart_of(&restored)
                    .chart_space()
                    .chart()
                    .plot_area()
                    .series_part_conflicts();
                assert_eq!(conflicts.len(), 1);
                assert_eq!(
                    (
                        conflicts[0].part,
                        conflicts[0].occurrence,
                        conflicts[0].reason
                    ),
                    (
                        SeriesPartKind::DataPoints,
                        0,
                        SeriesPartConflictReason::InvalidValue
                    )
                );
                assert!(point_xml(&series_xml(&restored)).is_empty());
            }
        }
    }
}

#[test]
#[allow(deprecated)]
fn invalid_marker_size_is_not_an_omitted_default_and_mutators_replace_lexical_state() {
    for lexical in ["abc", "", "256", "-1", "1", "73", "\u{a0}7\u{a0}"] {
        let book = input(
            &ChartType::LineChart,
            &format!(
                "<c:dPt><c:idx val=\"9\"/><c:marker><c:size val=\"{lexical}\"/></c:marker></c:dPt>"
            ),
        );
        for mut restored in std::iter::once(book.clone()).chain(persisted(&book)) {
            let size = first_series(&restored).data_points()[0]
                .marker()
                .unwrap()
                .size()
                .unwrap();
            assert_eq!(size.lexical_val(), Some(lexical));
            assert!(!size.is_valid());
            assert_ne!(size.parsed_val(), Some(5));
            let conflicts = chart_of(&restored)
                .chart_space()
                .chart()
                .plot_area()
                .series_part_conflicts();
            assert_eq!(conflicts.len(), 1);
            assert_eq!(conflicts[0].reason, SeriesPartConflictReason::InvalidValue);
            assert!(point_xml(&series_xml(&restored)).is_empty());
            let series = &mut restored.sheet_mut(0).unwrap().chart_collection_mut()[0]
                .chart_space_mut()
                .chart_mut()
                .plot_area_mut()
                .line_chart_mut()
                .unwrap()
                .area_chart_series_list_mut()
                .area_chart_series_mut()[0];
            let size = series.data_points_mut()[0]
                .marker_mut()
                .unwrap()
                .get_size_mut()
                .unwrap();
            size.set_val(7);
            assert_eq!(size.lexical_val(), None);
            assert_eq!(size.get_val(), 7);
            assert!(size.is_valid());
            assert!(series_xml(&restored).contains("<c:size val=\"7\"/>"));
        }
    }
}

#[test]
fn pie_explosion_missing_or_malformed_values_are_reported_and_omitted_without_storage_mutation() {
    for (leaf, reason) in [
        (
            "<c:explosion/>",
            SeriesPartConflictReason::MissingRequiredChild,
        ),
        (
            "<c:explosion val=\"abc\"/>",
            SeriesPartConflictReason::InvalidValue,
        ),
    ] {
        let book = input(&ChartType::PieChart, leaf);
        for restored in std::iter::once(book.clone()).chain(persisted(&book)) {
            let before =
                serde_json::to_value(first_series(&restored).explosion().unwrap()).unwrap();
            let conflicts = chart_of(&restored)
                .chart_space()
                .chart()
                .plot_area()
                .series_part_conflicts();
            assert_eq!(conflicts.len(), 1);
            assert_eq!(
                (
                    conflicts[0].group_index,
                    conflicts[0].series_index,
                    conflicts[0].part,
                    conflicts[0].occurrence,
                    conflicts[0].reason
                ),
                (0, 0, SeriesPartKind::Explosion, 0, reason)
            );
            assert!(!series_xml(&restored).contains("<c:explosion"));
            assert_eq!(
                serde_json::to_value(first_series(&restored).explosion().unwrap()).unwrap(),
                before
            );
        }
    }
    let mut book = input(&ChartType::PieChart, "");
    book.sheet_mut(0).unwrap().chart_collection_mut()[0]
        .chart_space_mut()
        .chart_mut()
        .plot_area_mut()
        .pie_chart_mut()
        .unwrap()
        .area_chart_series_list_mut()
        .area_chart_series_mut()[0]
        .set_explosion(Explosion::default());
    assert!(first_series(&book).explosion().is_some());
    let conflicts = chart_of(&book)
        .chart_space()
        .chart()
        .plot_area()
        .series_part_conflicts();
    assert_eq!(conflicts.len(), 1);
    assert_eq!(
        (conflicts[0].part, conflicts[0].reason),
        (
            SeriesPartKind::Explosion,
            SeriesPartConflictReason::MissingRequiredChild
        )
    );
    assert!(!series_xml(&book).contains("<c:explosion"));
    save_witness("invalid-pie-explosion", &book);
}

#[test]
fn expanded_alternate_drawing_leaves_match_empty_leaves_and_bound_unknown_children() {
    let empty = concat!(
        "<c:dPt><c:idx val=\"9\"/><c:marker><c:symbol \
         val=\"circle\"/><c:spPr><a:solidFill><a:srgbClr val=\"ED7D31\"><a:alpha \
         val=\"80000\"/><a:tint val=\"20000\"/></a:srgbClr></a:solidFill></c:spPr></c:marker>",
        "<c:spPr><a:noFill/><a:ln><a:solidFill><a:schemeClr val=\"accent1\"><a:lumMod \
         val=\"50000\"/><a:lumOff val=\"20000\"/></a:schemeClr></a:solidFill><a:prstDash \
         val=\"dash\"/><a:round/></a:ln><a:effectLst/></c:spPr></c:dPt>"
    );
    let mut expanded = empty.to_string();
    let mut legal_expanded = empty.to_string();
    for (name, attribute) in [
        ("alpha", " val=\"80000\""),
        ("tint", " val=\"20000\""),
        ("lumMod", " val=\"50000\""),
        ("lumOff", " val=\"20000\""),
        ("prstDash", " val=\"dash\""),
        ("round", ""),
        ("noFill", ""),
        ("effectLst", ""),
    ] {
        legal_expanded = legal_expanded.replace(
            &format!("<a:{name}{attribute}/>"),
            &format!("<a:{name}{attribute}></a:{name}>"),
        );
        expanded = expanded.replace(
            &format!("<a:{name}{attribute}/>"),
            &format!(
                "<a:{name}{attribute}><future xmlns=\"urn:future\"><a:{name}{attribute}/><a:alpha \
                 val=\"1234\"/></future></a:{name}>"
            ),
        );
    }
    // Effect-list extensions are not a scalar and would legitimately own
    // effects; only compare its empty boundary.
    expanded = expanded.replace(
        "<a:effectLst><future xmlns=\"urn:future\"><a:effectLst/><a:alpha \
         val=\"1234\"/></future></a:effectLst>",
        "<a:effectLst></a:effectLst>",
    );
    for (name, expanded) in [
        ("expanded-drawing-leaves", legal_expanded),
        ("contained-drawing-leaves", expanded),
    ] {
        let expanded = expanded
            .replace(
                "<c:dPt>",
                "<c:dPt xmlns:d=\"http://schemas.openxmlformats.org/drawingml/2006/main\">",
            )
            .replace("<a:", "<d:")
            .replace("</a:", "</d:");
        let book = input(&ChartType::LineChart, &expanded);
        for restored in std::iter::once(book.clone()).chain(persisted(&book)) {
            assert_eq!(point_xml(&series_xml(&restored)), vec![empty]);
            assert_eq!(first_series(&restored).index().val(), 1);
            assert_eq!(
                first_series(&restored).data_points()[0].index().val(),
                Some(9)
            );
        }
        save_witness(name, &book);
    }
}

#[test]
fn xml_double_accepts_only_schema_lexical_forms_and_keeps_valid_exponents() {
    for (lexical, expected) in [
        (" 1.2500E+1 ", 12.5),
        ("+.5", 0.5),
        ("1.", 1.0),
        ("1E309", f64::INFINITY),
        ("INF", f64::INFINITY),
    ] {
        let mut value = XmlDouble::default();
        value.set_attribute(Some(lexical.into()));
        assert_eq!(value.value(), Some(expected));
        assert_eq!(value.attribute(), Some(lexical));
    }
    for lexical in [
        "inf",
        "Infinity",
        "infinity",
        "+INF",
        "nan",
        "1_0",
        "1e",
        "\u{a0}1\u{a0}",
    ] {
        let mut value = XmlDouble::default();
        value.set_attribute(Some(lexical.into()));
        assert_eq!(value.value(), None, "{lexical}");
        assert_eq!(value.attribute(), Some(lexical));
    }
    let point = "<c:dPt><c:idx val=\"9\"/><c:pictureOptions><c:pictureStackUnit val=\" 1.2500E+1 \
                 \"/></c:pictureOptions></c:dPt>";
    let book = input(&ChartType::LineChart, point);
    for restored in std::iter::once(book.clone()).chain(persisted(&book)) {
        assert_eq!(
            first_series(&restored).data_points()[0]
                .picture_options()
                .unwrap()
                .stack_unit()
                .unwrap()
                .value(),
            Some(12.5)
        );
        assert_eq!(point_xml(&series_xml(&restored)), vec![point]);
    }
    save_witness("whitespace-double", &book);
    for lexical in ["inf", "Infinity", "+INF", "1e", "\u{a0}1\u{a0}"] {
        let book = input(
            &ChartType::LineChart,
            &format!(
                "<c:dPt><c:idx val=\"9\"/><c:pictureOptions><c:pictureStackUnit \
                 val=\"{lexical}\"/></c:pictureOptions></c:dPt>"
            ),
        );
        for restored in std::iter::once(book.clone()).chain(persisted(&book)) {
            let value = first_series(&restored).data_points()[0]
                .picture_options()
                .unwrap()
                .stack_unit()
                .unwrap();
            assert_eq!(value.attribute(), Some(lexical));
            assert_eq!(value.value(), None);
            let conflicts = chart_of(&restored)
                .chart_space()
                .chart()
                .plot_area()
                .series_part_conflicts();
            assert_eq!(conflicts.len(), 1);
            assert_eq!(conflicts[0].reason, SeriesPartConflictReason::InvalidValue);
            assert!(point_xml(&series_xml(&restored)).is_empty());
        }
    }
}

#[test]
#[allow(deprecated)]
fn legacy_scalar_json_and_public_setters_do_not_retain_stale_source_spelling() {
    let mut size: Size = serde_json::from_str("{\"val\":{\"value\":7}}").unwrap();
    assert_eq!(size.parsed_val(), Some(7));
    assert_eq!(size.lexical_val(), None);
    size.set_val(9);
    assert_eq!(size.get_val(), 9);
    let mut explosion: Explosion = serde_json::from_str("{\"val\":{\"value\":25}}").unwrap();
    assert_eq!(explosion.parsed_val(), Some(25));
    assert_eq!(explosion.lexical_val(), None);
    explosion.set_val(50);
    assert_eq!(explosion.get_val(), 50);
    assert_eq!(bincode::serialize(&Size::default()).unwrap(), vec![0, 0]);
    assert_eq!(
        bincode::serialize(&Explosion::default()).unwrap(),
        vec![0, 0]
    );
    assert_eq!(
        serde_json::to_string(&Size::default()).unwrap(),
        "{\"val\":{\"value\":null},\"parts\":null}"
    );
    assert_eq!(
        serde_json::to_string(&Explosion::default()).unwrap(),
        "{\"val\":{\"value\":null},\"parts\":null}"
    );
    // Old default scalar bytes had one Option tag. The appended None is exactly
    // one byte / thirteen JSON bytes for EACH present repaired leaf instance.
    assert_eq!(bincode::serialize(&size).unwrap(), vec![1, 9, 0]);
    assert_eq!(
        bincode::serialize(&explosion).unwrap(),
        vec![1, 50, 0, 0, 0, 0]
    );
    let mut book = input(
        &ChartType::LineChart,
        "<c:dPt><c:idx val=\" 9 \"/><c:explosion val=\" 9 \"/><c:marker><c:size val=\" 7 \
         \"/></c:marker></c:dPt>",
    );
    let point = &first_series(&book).data_points()[0];
    assert_eq!(
        bincode::serialize(point.marker().unwrap().size().unwrap())
            .unwrap()
            .len(),
        13
    );
    assert_eq!(
        bincode::serialize(point.explosion().unwrap())
            .unwrap()
            .len(),
        13
    );
    let series = &mut book.sheet_mut(0).unwrap().chart_collection_mut()[0]
        .chart_space_mut()
        .chart_mut()
        .plot_area_mut()
        .line_chart_mut()
        .unwrap()
        .area_chart_series_list_mut()
        .area_chart_series_mut()[0];
    let point = &mut series.data_points_mut()[0];
    point.index_mut().set_val(2);
    point.explosion_mut().unwrap().set_val(10);
    point.marker_mut().unwrap().size_mut().unwrap().set_val(8);
    let expected = "<c:dPt><c:idx val=\"2\"/><c:marker><c:size val=\"8\"/></c:marker><c:explosion \
                    val=\"10\"/></c:dPt>";
    for restored in persisted(&book) {
        let point = &first_series(&restored).data_points()[0];
        assert_eq!(point.index().attribute(), Some("2"));
        assert_eq!(point.explosion().unwrap().lexical_val(), None);
        assert_eq!(point.marker().unwrap().size().unwrap().lexical_val(), None);
        assert_eq!(point_xml(&series_xml(&restored)), vec![expected]);
    }
}

#[test]
fn invalid_boolean_can_be_repaired_through_its_existing_public_setter() {
    let mut book = input(
        &ChartType::LineChart,
        "<c:dPt><c:idx val=\"9\"/><c:invertIfNegative val=\"yes\"/></c:dPt>",
    );
    let series = &mut book.sheet_mut(0).unwrap().chart_collection_mut()[0]
        .chart_space_mut()
        .chart_mut()
        .plot_area_mut()
        .line_chart_mut()
        .unwrap()
        .area_chart_series_list_mut()
        .area_chart_series_mut()[0];
    series.data_points_mut()[0]
        .invert_if_negative_mut()
        .unwrap()
        .set_value(false);
    for restored in persisted(&book) {
        assert_eq!(
            first_series(&restored).data_points()[0]
                .invert_if_negative()
                .unwrap()
                .value(),
            Some(false)
        );
        assert_eq!(
            point_xml(&series_xml(&restored)),
            vec!["<c:dPt><c:idx val=\"9\"/><c:invertIfNegative val=\"0\"/></c:dPt>"]
        );
        assert!(
            chart_of(&restored)
                .chart_space()
                .chart()
                .plot_area()
                .series_part_conflicts()
                .is_empty()
        );
    }
}

#[test]
fn escaped_xml_whitespace_is_decoded_once_before_scalar_semantics() {
    let book = input(
        &ChartType::LineChart,
        "<c:dPt><c:idx val=\"&#32;9&#32;\"/><c:invertIfNegative \
         val=\"&#32;false&#32;\"/><c:marker><c:symbol val=\"cir&#99;le\"/><c:size \
         val=\"&#32;7&#32;\"/></c:marker></c:dPt>",
    );
    for restored in std::iter::once(book.clone()).chain(persisted(&book)) {
        let point = &first_series(&restored).data_points()[0];
        assert_eq!(point.index().val(), Some(9));
        assert_eq!(point.index().attribute(), Some(" 9 "));
        assert_eq!(point.invert_if_negative().unwrap().value(), Some(false));
        assert_eq!(
            point.marker().unwrap().size().unwrap().parsed_val(),
            Some(7)
        );
        assert!(matches!(
            point.marker().unwrap().symbol().unwrap().val(),
            MarkerStyleValues::Circle
        ));
        assert_eq!(
            point_xml(&series_xml(&restored)),
            vec![
                "<c:dPt><c:idx val=\" 9 \"/><c:invertIfNegative val=\" false \
                 \"/><c:marker><c:symbol val=\"circle\"/><c:size val=\" 7 \"/></c:marker></c:dPt>"
            ]
        );
    }
}

/// Expand test INPUT elements, optionally placing an unsupported same-name
/// subtree inside each owner. Expected output remains the independently
/// authored schema/model fixture, never captured from either product route.
fn expanded_drawing_input(xml: &str, contain_unknown: bool) -> String {
    use quick_xml::{
        Reader,
        Writer,
        events::{
            BytesEnd,
            BytesStart,
            Event,
        },
    };
    let mut reader = Reader::from_str(xml);
    let mut writer = Writer::new(Cursor::new(Vec::new()));
    loop {
        let event = reader.read_event().unwrap();
        let empty = matches!(event, Event::Empty(_));
        match event {
            Event::Start(start) | Event::Empty(start) => {
                let start = start.into_owned();
                let end_name = String::from_utf8(start.name().as_ref().to_vec()).unwrap();
                writer.write_event(Event::Start(start.clone())).unwrap();
                if contain_unknown {
                    let mut future = BytesStart::new("z:future");
                    future.push_attribute(("xmlns:z", "urn:future"));
                    writer.write_event(Event::Start(future)).unwrap();
                    writer.write_event(Event::Empty(start)).unwrap();
                    writer
                        .write_event(Event::Empty(BytesStart::from_content(
                            "a:alpha val=\"1234\"",
                            7,
                        )))
                        .unwrap();
                    writer
                        .write_event(Event::End(BytesEnd::new("z:future")))
                        .unwrap();
                }
                if empty {
                    writer
                        .write_event(Event::End(BytesEnd::new(end_name)))
                        .unwrap();
                }
            }
            Event::Eof => break,
            event => writer.write_event(event.into_owned()).unwrap(),
        }
    }
    String::from_utf8(writer.into_inner().into_inner()).unwrap()
}

#[test]
fn every_modeled_outline_and_effect_delegate_preserves_fields_across_expanded_aliases() {
    let cases: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("fixtures/point_drawing_delegates.json")).unwrap();
    assert_eq!(cases.len(), 13);
    for case in cases {
        let name = case["name"].as_str().unwrap();
        let source = case["input"].as_str().unwrap();
        let expected = format!(
            "<c:dPt><c:idx val=\"9\"/><c:spPr>{}</c:spPr></c:dPt>",
            case["expected"].as_str().unwrap()
        );
        let mut original_shape = None;
        for (form, fragment) in [
            ("empty", source.to_owned()),
            ("expanded", expanded_drawing_input(source, false)),
            ("contained", expanded_drawing_input(source, true)),
        ] {
            let fragment = if form == "empty" {
                fragment
            } else {
                fragment.replace("<a:", "<d:").replace("</a:", "</d:")
            };
            let point = format!(
                "<c:dPt xmlns:d=\"http://schemas.openxmlformats.org/drawingml/2006/main\"><c:idx \
                 val=\"9\"/><c:spPr>{fragment}</c:spPr></c:dPt>"
            );
            let book = input(&ChartType::LineChart, &point);
            let shape = serde_json::to_value(
                first_series(&book).data_points()[0]
                    .shape_properties()
                    .unwrap(),
            )
            .unwrap();
            if let Some(before) = &original_shape {
                assert_eq!(&shape, before, "{name}/{form}");
            } else {
                original_shape = Some(shape);
            }
            for restored in std::iter::once(book.clone()).chain(persisted(&book)) {
                assert_eq!(
                    point_xml(&series_xml(&restored)),
                    vec![expected.as_str()],
                    "{name}/{form}"
                );
                assert_eq!(first_series(&restored).index().val(), 1);
                assert_eq!(
                    first_series(&restored).data_points()[0].index().val(),
                    Some(9)
                );
                assert_eq!(
                    point_xml(&series_xml(&read(&write(&restored)))),
                    vec![expected.as_str()],
                    "{name}/{form}/reload"
                );
            }
            save_witness(&format!("delegate-{name}-{form}"), &book);
        }
    }
}

#[test]
#[allow(deprecated)]
fn deserialized_dual_scalar_branches_are_invalid_until_explicit_public_edit() {
    for (name, legacy, lexical) in [
        ("different", 7, "8"),
        ("invalid-valid", 73, "7"),
        ("both-invalid", 73, "abc"),
        ("equal", 7, "7"),
    ] {
        let both = serde_json::json!({"val":{"value":legacy},"parts":{"attribute":lexical}});
        let size: Size = serde_json::from_value(both.clone()).unwrap();
        assert!(!size.is_coherent());
        assert_eq!(size.parsed_val(), None);
        assert!(!size.is_valid());
        let size: Size = bincode::deserialize(&bincode::serialize(&size).unwrap()).unwrap();
        assert_eq!(serde_json::to_value(&size).unwrap(), both);
        let mut book = input(
            &ChartType::LineChart,
            "<c:dPt><c:idx val=\"9\"/><c:marker/></c:dPt>",
        );
        book.sheet_mut(0).unwrap().chart_collection_mut()[0]
            .chart_space_mut()
            .chart_mut()
            .plot_area_mut()
            .line_chart_mut()
            .unwrap()
            .area_chart_series_list_mut()
            .area_chart_series_mut()[0]
            .data_points_mut()[0]
            .marker_mut()
            .unwrap()
            .set_size(size);
        save_witness(&format!("dual-size-{name}"), &book);
        for (form, mut restored) in std::iter::once(book.clone())
            .chain(persisted(&book))
            .enumerate()
        {
            let conflicts = chart_of(&restored)
                .chart_space()
                .chart()
                .plot_area()
                .series_part_conflicts();
            assert_eq!(conflicts.len(), 1);
            assert_eq!(conflicts[0].reason, SeriesPartConflictReason::InvalidValue);
            assert!(point_xml(&series_xml(&restored)).is_empty());
            assert_eq!(
                serde_json::to_value(
                    first_series(&restored).data_points()[0]
                        .marker()
                        .unwrap()
                        .size()
                        .unwrap()
                )
                .unwrap(),
                both
            );
            let marker = restored.sheet_mut(0).unwrap().chart_collection_mut()[0]
                .chart_space_mut()
                .chart_mut()
                .plot_area_mut()
                .line_chart_mut()
                .unwrap()
                .area_chart_series_list_mut()
                .area_chart_series_mut()[0]
                .data_points_mut()[0]
                .marker_mut()
                .unwrap();
            let size = if form == 1 {
                marker.get_size_mut().unwrap()
            } else {
                marker.size_mut().unwrap()
            };
            size.set_val(9);
            assert!(size.is_coherent());
            assert_eq!(size.parsed_val(), Some(9));
            assert_eq!(size.lexical_val(), None);
            assert_eq!(size.get_val(), 9);
            assert!(series_xml(&restored).contains("<c:size val=\"9\"/>"));
        }
    }
    // Every typed u32 is in its primitive domain; the ambiguous state is still
    // invalid even for equal values, not resolved by arbitrary branch
    // precedence.
    for (name, legacy, lexical) in [
        ("different", 7, "8"),
        ("max-different", u32::MAX, "7"),
        ("invalid-lexeme", u32::MAX, "abc"),
        ("equal", 7, "7"),
    ] {
        let both = serde_json::json!({"val":{"value":legacy},"parts":{"attribute":lexical}});
        let explosion: Explosion = serde_json::from_value(both.clone()).unwrap();
        assert!(!explosion.is_coherent());
        assert_eq!(explosion.parsed_val(), None);
        assert!(!explosion.is_valid());
        let explosion: Explosion =
            bincode::deserialize(&bincode::serialize(&explosion).unwrap()).unwrap();
        assert_eq!(serde_json::to_value(&explosion).unwrap(), both);
        let mut book = input(&ChartType::PieChart, "");
        book.sheet_mut(0).unwrap().chart_collection_mut()[0]
            .chart_space_mut()
            .chart_mut()
            .plot_area_mut()
            .pie_chart_mut()
            .unwrap()
            .area_chart_series_list_mut()
            .area_chart_series_mut()[0]
            .set_explosion(explosion);
        save_witness(&format!("dual-explosion-{name}"), &book);
        for (form, mut restored) in std::iter::once(book.clone())
            .chain(persisted(&book))
            .enumerate()
        {
            let conflicts = chart_of(&restored)
                .chart_space()
                .chart()
                .plot_area()
                .series_part_conflicts();
            assert_eq!(conflicts.len(), 1);
            assert_eq!(
                (conflicts[0].part, conflicts[0].reason),
                (
                    SeriesPartKind::Explosion,
                    SeriesPartConflictReason::InvalidValue
                )
            );
            assert!(!series_xml(&restored).contains("<c:explosion"));
            assert_eq!(
                serde_json::to_value(first_series(&restored).explosion().unwrap()).unwrap(),
                both
            );
            let series = &mut restored.sheet_mut(0).unwrap().chart_collection_mut()[0]
                .chart_space_mut()
                .chart_mut()
                .plot_area_mut()
                .pie_chart_mut()
                .unwrap()
                .area_chart_series_list_mut()
                .area_chart_series_mut()[0];
            let explosion = if form == 1 {
                series.get_explosion_mut().unwrap()
            } else {
                series.explosion_mut().unwrap()
            };
            explosion.set_val(9);
            assert!(explosion.is_coherent());
            assert_eq!(explosion.get_val(), 9);
            assert_eq!(explosion.lexical_val(), None);
            assert!(series_xml(&restored).contains("<c:explosion val=\"9\"/>"));
        }
    }
}
