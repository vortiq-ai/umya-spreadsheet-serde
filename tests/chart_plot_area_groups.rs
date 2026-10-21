#![cfg(feature = "serde")]

use std::io::{
    Cursor,
    Read,
    Write,
};

use serde::{
    Deserialize,
    Serialize,
};
use serde_json::{
    Value,
    json,
};
use umya_spreadsheet::{
    Chart,
    ChartType,
    Workbook,
    drawing::{
        self,
        charts::{
            self,
            ChartGroup,
            ChartGroupEntry,
            ChartGroupRef,
            PlotArea,
        },
    },
};

const FAMILIES: [(ChartType, &str); 13] = [
    (ChartType::LineChart, "line_chart"),
    (ChartType::Line3DChart, "line_3d_chart"),
    (ChartType::PieChart, "pie_chart"),
    (ChartType::Pie3DChart, "pie_3d_chart"),
    (ChartType::DoughnutChart, "doughnut_chart"),
    (ChartType::ScatterChart, "scatter_chart"),
    (ChartType::BarChart, "bar_chart"),
    (ChartType::Bar3DChart, "bar_3d_chart"),
    (ChartType::RadarChart, "radar_chart"),
    (ChartType::BubbleChart, "bubble_chart"),
    (ChartType::AreaChart, "area_chart"),
    (ChartType::Area3DChart, "area_3d_chart"),
    (ChartType::OfPieChart, "of_pie_chart"),
];

// The complete previous JSON model ignores the new order key.
#[derive(Serialize, Deserialize)]
struct PreviousPlotArea {
    layout:           charts::Layout,
    line_chart:       Option<charts::LineChart>,
    line_3d_chart:    Option<charts::Line3DChart>,
    pie_chart:        Option<charts::PieChart>,
    pie_3d_chart:     Option<charts::Pie3DChart>,
    doughnut_chart:   Option<charts::DoughnutChart>,
    scatter_chart:    Option<charts::ScatterChart>,
    bar_chart:        Option<charts::BarChart>,
    bar_3d_chart:     Option<charts::Bar3DChart>,
    radar_chart:      Option<charts::RadarChart>,
    bubble_chart:     Option<charts::BubbleChart>,
    area_chart:       Option<charts::AreaChart>,
    area_3d_chart:    Option<charts::Area3DChart>,
    of_pie_chart:     Option<charts::OfPieChart>,
    category_axis:    Vec<charts::CategoryAxis>,
    date_axis:        Vec<charts::DateAxis>,
    value_axis:       Vec<charts::ValueAxis>,
    series_axis:      Vec<charts::SeriesAxis>,
    shape_properties: Option<charts::ShapeProperties>,
}

fn group(kind: ChartType, index: u32) -> ChartGroup {
    let mut series = charts::AreaChartSeries::default();
    series.index_mut().set_val(index);
    series.order_mut().set_val(index + 100);
    let mut values = charts::Values::default();
    values
        .number_reference_mut()
        .formula_mut()
        .set_address_str(format!("Sheet1!$B${}:$B${}", index + 2, index + 3));
    series.set_values(values);
    macro_rules! container {
        ($family:ident) => {{
            let mut chart = charts::$family::default();
            chart
                .area_chart_series_list_mut()
                .add_area_chart_series(series);
            ChartGroup::$family(chart)
        }};
    }
    match kind {
        ChartType::LineChart => container!(LineChart),
        ChartType::Line3DChart => container!(Line3DChart),
        ChartType::PieChart => container!(PieChart),
        ChartType::Pie3DChart => container!(Pie3DChart),
        ChartType::DoughnutChart => container!(DoughnutChart),
        ChartType::ScatterChart => container!(ScatterChart),
        ChartType::BarChart => container!(BarChart),
        ChartType::Bar3DChart => container!(Bar3DChart),
        ChartType::RadarChart => container!(RadarChart),
        ChartType::BubbleChart => container!(BubbleChart),
        ChartType::AreaChart => container!(AreaChart),
        ChartType::Area3DChart => container!(Area3DChart),
        ChartType::OfPieChart => container!(OfPieChart),
    }
}

fn set_group(plot: &mut PlotArea, group: ChartGroup) {
    match group {
        ChartGroup::LineChart(chart) => {
            plot.set_line_chart(chart);
        }
        ChartGroup::Line3DChart(chart) => {
            plot.set_line_3d_chart(chart);
        }
        ChartGroup::PieChart(chart) => {
            plot.set_pie_chart(chart);
        }
        ChartGroup::Pie3DChart(chart) => {
            plot.set_pie_3d_chart(chart);
        }
        ChartGroup::DoughnutChart(chart) => {
            plot.set_doughnut_chart(chart);
        }
        ChartGroup::ScatterChart(chart) => {
            plot.set_scatter_chart(chart);
        }
        ChartGroup::BarChart(chart) => {
            plot.set_bar_chart(chart);
        }
        ChartGroup::Bar3DChart(chart) => {
            plot.set_bar_3d_chart(chart);
        }
        ChartGroup::RadarChart(chart) => {
            plot.set_radar_chart(chart);
        }
        ChartGroup::BubbleChart(chart) => {
            plot.set_bubble_chart(chart);
        }
        ChartGroup::AreaChart(chart) => {
            plot.set_area_chart(chart);
        }
        ChartGroup::Area3DChart(chart) => {
            plot.set_area_3d_chart(chart);
        }
        ChartGroup::OfPieChart(chart) => {
            plot.set_of_pie_chart(chart);
        }
    }
}

fn legacy_group(plot: &PlotArea, kind: ChartType) -> Value {
    match kind {
        ChartType::LineChart => serde_json::to_value(plot.line_chart()).unwrap(),
        ChartType::Line3DChart => serde_json::to_value(plot.line_3d_chart()).unwrap(),
        ChartType::PieChart => serde_json::to_value(plot.pie_chart()).unwrap(),
        ChartType::Pie3DChart => serde_json::to_value(plot.pie_3d_chart()).unwrap(),
        ChartType::DoughnutChart => serde_json::to_value(plot.doughnut_chart()).unwrap(),
        ChartType::ScatterChart => serde_json::to_value(plot.scatter_chart()).unwrap(),
        ChartType::BarChart => serde_json::to_value(plot.bar_chart()).unwrap(),
        ChartType::Bar3DChart => serde_json::to_value(plot.bar_3d_chart()).unwrap(),
        ChartType::RadarChart => serde_json::to_value(plot.radar_chart()).unwrap(),
        ChartType::BubbleChart => serde_json::to_value(plot.bubble_chart()).unwrap(),
        ChartType::AreaChart => serde_json::to_value(plot.area_chart()).unwrap(),
        ChartType::Area3DChart => serde_json::to_value(plot.area_3d_chart()).unwrap(),
        ChartType::OfPieChart => serde_json::to_value(plot.of_pie_chart()).unwrap(),
    }
}

fn signature(plot: &PlotArea) -> Vec<(ChartType, Value)> {
    plot.chart_groups()
        .map(|group| {
            let value = match group {
                ChartGroupRef::LineChart(chart) => serde_json::to_value(chart).unwrap(),
                ChartGroupRef::Line3DChart(chart) => serde_json::to_value(chart).unwrap(),
                ChartGroupRef::PieChart(chart) => serde_json::to_value(chart).unwrap(),
                ChartGroupRef::Pie3DChart(chart) => serde_json::to_value(chart).unwrap(),
                ChartGroupRef::DoughnutChart(chart) => serde_json::to_value(chart).unwrap(),
                ChartGroupRef::ScatterChart(chart) => serde_json::to_value(chart).unwrap(),
                ChartGroupRef::BarChart(chart) => serde_json::to_value(chart).unwrap(),
                ChartGroupRef::Bar3DChart(chart) => serde_json::to_value(chart).unwrap(),
                ChartGroupRef::RadarChart(chart) => serde_json::to_value(chart).unwrap(),
                ChartGroupRef::BubbleChart(chart) => serde_json::to_value(chart).unwrap(),
                ChartGroupRef::AreaChart(chart) => serde_json::to_value(chart).unwrap(),
                ChartGroupRef::Area3DChart(chart) => serde_json::to_value(chart).unwrap(),
                ChartGroupRef::OfPieChart(chart) => serde_json::to_value(chart).unwrap(),
            };
            (group.kind(), value)
        })
        .collect()
}

fn indices(plot: &PlotArea) -> Vec<u32> {
    plot.chart_groups()
        .map(|group| group.series().area_chart_series()[0].index().val())
        .collect()
}

fn workbook_with_plot(plot: &PlotArea) -> Workbook {
    let mut book = umya_spreadsheet::new_file();
    let mut chart = Chart::default();
    let mut from = drawing::spreadsheet::MarkerType::default();
    let mut to = drawing::spreadsheet::MarkerType::default();
    from.set_coordinate("D1");
    to.set_coordinate("L15");
    chart.new_chart(&ChartType::LineChart, from, to, vec!["Sheet1!$B$2:$B$3"]);
    *chart.plot_area_mut() = plot.clone();
    book.sheet_by_name_mut("Sheet1").unwrap().add_chart(chart);
    book
}

fn chart_bytes(plot: &PlotArea) -> Vec<u8> {
    let mut output = Cursor::new(Vec::new());
    umya_spreadsheet::writer::xlsx::write_writer(&workbook_with_plot(plot), &mut output).unwrap();
    output.into_inner()
}

fn workbook_plot(book: &Workbook) -> PlotArea {
    book.sheet_by_name("Sheet1").unwrap().chart_collection()[0]
        .chart_space()
        .chart()
        .plot_area()
        .clone()
}

fn insert_row(plot: &PlotArea) -> PlotArea {
    let mut book = workbook_with_plot(plot);
    book.insert_new_row("Sheet1", 2, 1);
    workbook_plot(&book)
}

fn chart_xml(bytes: &[u8]) -> String {
    let mut archive = ::zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut xml = String::new();
    archive
        .by_name("xl/charts/chart1.xml")
        .unwrap()
        .read_to_string(&mut xml)
        .unwrap();
    xml
}

fn read_plot(bytes: &[u8]) -> PlotArea {
    let book = umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap();
    workbook_plot(&book)
}

fn native_signature(plot: &PlotArea) -> Vec<Value> {
    plot.chart_groups()
        .map(|group| {
            let series: Vec<_> = group
                .series()
                .area_chart_series()
                .iter()
                .map(|series| {
                    let formulas: Vec<_> = series
                        .clone()
                        .formula_mut()
                        .into_iter()
                        .map(|formula| formula.address_str())
                        .collect();
                    json!([series.index().val(), series.order().val(), formulas])
                })
                .collect();
            let axes = group
                .axis_ids()
                .map(|axes| axes.iter().map(charts::AxisId::val).collect::<Vec<_>>());
            json!([group.kind(), series, axes])
        })
        .collect()
}

fn assert_xml_round_trip(plot: &PlotArea) -> PlotArea {
    let bytes = chart_bytes(plot);
    let restored = read_plot(&bytes);
    assert_canonical(&restored);
    assert_eq!(native_signature(&restored), native_signature(plot));
    assert_eq!(chart_xml(&chart_bytes(&restored)), chart_xml(&bytes));
    restored
}

fn assert_canonical(plot: &PlotArea) {
    let json = serde_json::to_value(plot).unwrap();
    let entries: Vec<ChartGroupEntry> =
        serde_json::from_value(json["chart_group_order"].clone()).unwrap();
    let mut first = [false; 13];
    for entry in &entries {
        match entry {
            ChartGroupEntry::First(kind) => {
                assert!(!first[*kind as usize]);
                assert!(!legacy_group(plot, *kind).is_null());
                first[*kind as usize] = true;
            }
            ChartGroupEntry::Additional(group) => assert!(first[group.kind() as usize]),
        }
    }
    let mut seen = [false; 13];
    for (kind, value) in signature(plot) {
        if !seen[kind as usize] {
            assert_eq!(legacy_group(plot, kind), value);
            if !entries.is_empty() {
                assert!(first[kind as usize]);
            }
            seen[kind as usize] = true;
        }
    }
    let kinds: Vec<_> = plot.chart_groups().map(ChartGroupRef::kind).collect();
    if kinds
        .windows(2)
        .all(|pair| (pair[0] as usize) < pair[1] as usize)
    {
        assert!(entries.is_empty());
    }
    let mut normalized = plot.clone();
    normalized.normalize_chart_groups();
    assert_eq!(serde_json::to_value(normalized).unwrap(), json);
}

fn assert_repair(plot: &mut PlotArea) {
    let before = signature(plot);
    let mutable_indices: Vec<_> = plot
        .chart_groups_mut()
        .map(|group| group.series_mut().area_chart_series()[0].index().val())
        .collect();
    assert_eq!(mutable_indices, indices(plot));
    plot.normalize_chart_groups();
    assert_eq!(signature(plot), before);
    assert_canonical(plot);
}

fn container_count(value: &Value) -> usize {
    match value {
        Value::Object(fields) => {
            usize::from(fields.contains_key("area_chart_series_list"))
                + fields.values().map(container_count).sum::<usize>()
        }
        Value::Array(items) => items.iter().map(container_count).sum(),
        _ => 0,
    }
}

#[test]
fn repeated_line_containers_survive_read_and_write() {
    let mut plot = PlotArea::default();
    plot.add_chart_group(group(ChartType::LineChart, 0));
    plot.add_chart_group(group(ChartType::LineChart, 1));
    for (group, ids) in plot
        .chart_groups_mut()
        .zip([[44_831_849, 55_979_570], [42_695_901, 89_363_512]])
    {
        let axes = group.axis_ids_mut().unwrap();
        for id in ids {
            let mut axis = charts::AxisId::default();
            axis.set_val(id);
            axes.push(axis);
        }
    }
    let mut axis = charts::ValueAxis::default();
    axis.axis_id_mut().set_val(89_363_512);
    axis.axis_position_mut()
        .set_val(charts::AxisPositionValues::Right);
    plot.add_value_axis(axis);
    let restored = assert_xml_round_trip(&plot);
    assert_eq!(indices(&restored), vec![0, 1]);
    assert_eq!(
        restored
            .chart_groups()
            .nth(1)
            .unwrap()
            .axis_ids()
            .unwrap()
            .iter()
            .map(charts::AxisId::val)
            .collect::<Vec<_>>(),
        vec![42_695_901, 89_363_512]
    );
    let axis = &restored.value_axis()[0];
    assert_eq!(axis.axis_id().val(), 89_363_512);
    assert!(matches!(
        axis.axis_position().val(),
        charts::AxisPositionValues::Right
    ));
}

#[test]
fn every_family_round_trips_in_a_combined_plot_area() {
    let mut plot = PlotArea::default();
    let mut expected = Vec::new();
    for (index, (kind, _)) in FAMILIES.into_iter().rev().enumerate() {
        plot.add_chart_group(group(kind, u32::try_from(index).unwrap()));
        expected.push(kind);
        if kind == ChartType::BarChart {
            plot.add_chart_group(group(ChartType::LineChart, 30));
            expected.push(ChartType::LineChart);
        }
    }
    let restored = assert_xml_round_trip(&plot);
    assert_eq!(
        restored
            .chart_groups()
            .map(ChartGroupRef::kind)
            .collect::<Vec<_>>(),
        expected
    );
    for group in restored.chart_groups() {
        assert_eq!(
            group.axis_ids().is_none(),
            matches!(
                group.kind(),
                ChartType::PieChart
                    | ChartType::Pie3DChart
                    | ChartType::DoughnutChart
                    | ChartType::OfPieChart
            )
        );
        let series = &group.series().area_chart_series()[0];
        assert_eq!(series.order().val(), series.index().val() + 100);
    }
    // An unsupported plot is filtered by the XLSX reader.
    let bytes = chart_bytes(&PlotArea::default());
    let book = umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap();
    assert!(
        book.sheet_by_name("Sheet1")
            .unwrap()
            .chart_collection()
            .is_empty()
    );
}

#[test]
fn supported_families_are_imported_independently() {
    for (kind, _) in FAMILIES {
        let mut plot = PlotArea::default();
        plot.add_chart_group(group(kind, 0));
        let restored = assert_xml_round_trip(&plot);
        assert_eq!(
            restored
                .chart_groups()
                .map(ChartGroupRef::kind)
                .collect::<Vec<_>>(),
            vec![kind]
        );
    }
}

#[test]
fn unsupported_only_containers_are_filtered_during_import() {
    let bytes = chart_bytes(&PlotArea::default());
    let xml = chart_xml(&bytes).replace(
        "<c:plotArea>",
        concat!(
            "<c:plotArea><c:surfaceChart>",
            r#"<c:wireframe val="0"/><c:ser><c:idx val="0"/><c:order val="0"/>"#,
            "<c:val><c:numRef><c:f>Sheet1!$B$2:$B$3</c:f></c:numRef></c:val></c:ser>",
            r#"<c:axId val="101"/><c:axId val="102"/><c:axId val="103"/>"#,
            "</c:surfaceChart>",
        ),
    );
    assert!(xml.contains("<c:surfaceChart>"));
    let mut archive = ::zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut output = ::zip::ZipWriter::new(Cursor::new(Vec::new()));
    for index in 0..archive.len() {
        let mut part = archive.by_index(index).unwrap();
        output
            .start_file(part.name(), ::zip::write::SimpleFileOptions::default())
            .unwrap();
        if part.name() == "xl/charts/chart1.xml" {
            output.write_all(xml.as_bytes()).unwrap();
        } else {
            std::io::copy(&mut part, &mut output).unwrap();
        }
    }
    let bytes = output.finish().unwrap().into_inner();
    let book = umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap();
    assert!(
        book.sheet_by_name("Sheet1")
            .unwrap()
            .chart_collection()
            .is_empty()
    );
}

#[test]
fn structural_adjustment_moves_formulas_in_every_container() {
    let mut plot = PlotArea::default();
    for (kind, _) in FAMILIES {
        plot.add_chart_group(group(kind, 0));
    }
    plot.add_chart_group(group(ChartType::LineChart, 0));
    assert_eq!(plot.formula_mut().len(), 14);
    plot = insert_row(&plot);
    for formula in plot.formula_mut() {
        assert_eq!(formula.address_str(), "Sheet1!$B$3:$B$4");
    }
    assert_xml_round_trip(&plot);
}

#[test]
fn legacy_getters_return_the_first_container() {
    let mut plot = PlotArea::default();
    for (kind, _) in FAMILIES.into_iter().rev() {
        plot.add_chart_group(group(kind, 10));
        plot.add_chart_group(group(kind, 20));
    }
    for (kind, _) in FAMILIES {
        let expected = serde_json::to_value(group(kind, 10)).unwrap();
        assert_eq!(
            legacy_group(&plot, kind),
            *expected.as_object().unwrap().values().next().unwrap()
        );
    }
    assert_canonical(&plot);
}

#[test]
fn setter_replaces_only_the_first_container() {
    for (kind, _) in FAMILIES {
        let mut plot = PlotArea::default();
        plot.add_chart_group(group(ChartType::BubbleChart, 9));
        plot.add_chart_group(group(kind, 10));
        plot.add_chart_group(group(kind, 20));
        let mut expected = signature(&plot);
        let replacement = serde_json::to_value(group(kind, 40)).unwrap();
        expected
            .iter_mut()
            .find(|(family, _)| *family == kind)
            .unwrap()
            .1 = replacement
            .as_object()
            .unwrap()
            .values()
            .next()
            .unwrap()
            .clone();
        set_group(&mut plot, group(kind, 40));
        assert_eq!(signature(&plot), expected);
        assert_canonical(&plot);
    }
}

#[test]
fn legacy_first_found_helpers_use_legacy_precedence() {
    let mut plot = PlotArea::default();
    plot.add_chart_group(group(ChartType::AreaChart, 2));
    plot.add_chart_group(group(ChartType::BarChart, 1));
    plot.add_chart_group(group(ChartType::LineChart, 0));
    plot.add_chart_group(group(ChartType::LineChart, 3));
    plot.set_grouping(charts::GroupingValues::Stacked);
    assert!(matches!(
        plot.line_chart().unwrap().grouping().val(),
        charts::GroupingValues::Stacked
    ));
    assert!(!matches!(
        plot.bar_chart().unwrap().grouping().val(),
        charts::GroupingValues::Stacked
    ));
    plot.area_chart_series_list_mut().area_chart_series_mut()[0]
        .index_mut()
        .set_val(50);
    assert_eq!(indices(&plot), vec![2, 1, 50, 3]);
}

#[test]
fn json_round_trip_keeps_container_order_and_first_containers_in_legacy_fields() {
    let mut plot = PlotArea::default();
    for (kind, _) in FAMILIES.into_iter().rev() {
        plot.add_chart_group(group(kind, 0));
    }
    plot.add_chart_group(group(ChartType::LineChart, 1));
    let mut json = serde_json::to_value(&plot).unwrap();
    assert_eq!(container_count(&json), 14);
    let restored: PlotArea = serde_json::from_value(json.clone()).unwrap();
    assert_eq!(serde_json::to_value(&restored).unwrap(), json);
    let previous: PreviousPlotArea = serde_json::from_value(json.clone()).unwrap();
    json.as_object_mut().unwrap().remove("chart_group_order");
    assert_eq!(serde_json::to_value(previous).unwrap(), json);
    let mut legacy: PlotArea = serde_json::from_value(json.clone()).unwrap();
    assert!(
        serde_json::to_value(&legacy).unwrap()["chart_group_order"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        legacy
            .chart_groups()
            .map(ChartGroupRef::kind)
            .collect::<Vec<_>>(),
        FAMILIES.map(|(kind, _)| kind)
    );
    legacy.normalize_chart_groups();
    assert_canonical(&legacy);

    let previous: PreviousPlotArea = serde_json::from_value(json).unwrap();
    let old_binary = bincode::serialize(&previous).unwrap();
    let new_binary = bincode::serialize(&legacy).unwrap();
    assert_eq!(new_binary.len() - old_binary.len(), 8);
    assert!(
        serde_json::to_vec(&legacy).unwrap().len() - serde_json::to_vec(&previous).unwrap().len()
            <= 24
    );
}

#[test]
fn container_order_has_one_canonical_form() {
    for (kind, field) in FAMILIES {
        for count in 0..=3 {
            for repair in 0..8 {
                let mut value = serde_json::to_value(PlotArea::default()).unwrap();
                if count > 0 && repair != 0 {
                    let first = serde_json::to_value(group(kind, 0)).unwrap();
                    value[field] = first.as_object().unwrap().values().next().unwrap().clone();
                }
                let mut entries = Vec::new();
                let start = if repair == 0 || repair == 7 { 0 } else { 1 };
                for index in start..count {
                    entries.push(ChartGroupEntry::Additional(Box::new(group(kind, index))));
                }
                match repair {
                    // Orphans, including stale markers ahead of their payload.
                    0 => entries.insert(0, ChartGroupEntry::First(kind)),
                    // Missing marker before an additional container.
                    1 => {}
                    // Correct marker, a duplicate, and an absent family's marker.
                    2 => {
                        entries.insert(0, ChartGroupEntry::First(kind));
                        entries.push(ChartGroupEntry::First(kind));
                        let absent = FAMILIES[(kind as usize + 1) % 13].0;
                        entries.insert(0, ChartGroupEntry::First(absent));
                    }
                    // A late marker requires swapping the stored first container.
                    3 => entries.push(ChartGroupEntry::First(kind)),
                    // No list means the historical family order.
                    4 => entries.clear(),
                    // A marker-only list must collapse to the empty list.
                    5 => entries = vec![ChartGroupEntry::First(kind)],
                    // Several early additional containers and duplicate late markers.
                    6 => {
                        entries.push(ChartGroupEntry::First(kind));
                        entries.push(ChartGroupEntry::First(kind));
                    }
                    // An entirely orphaned list with no markers.
                    _ => {
                        value[field] = Value::Null;
                    }
                }
                value["chart_group_order"] = serde_json::to_value(entries).unwrap();
                let mut plot: PlotArea = serde_json::from_value(value).unwrap();
                assert_repair(&mut plot);
            }
        }
    }
    // Generated mixed-family orders exercise every population from zero to
    // three.
    for seed in 0..16 {
        let mut plot = PlotArea::default();
        for offset in 0..13 {
            let (kind, _) = FAMILIES[(offset * 5 + seed) % 13];
            for index in 0..(offset + seed) % 4 {
                plot.add_chart_group(group(kind, u32::try_from(index).unwrap()));
            }
        }
        let mut value = serde_json::to_value(&plot).unwrap();
        let entries = value["chart_group_order"].as_array_mut().unwrap();
        entries.reverse();
        if let Some(entry) = entries.first().cloned() {
            entries.push(entry);
        }
        let mut malformed: PlotArea = serde_json::from_value(value).unwrap();
        assert_repair(&mut malformed);
    }
}

#[test]
fn marker_repairs_preserve_the_declared_sequence() {
    let mut value = serde_json::to_value(PlotArea::default()).unwrap();
    let first = serde_json::to_value(group(ChartType::LineChart, 10)).unwrap();
    value["line_chart"] = first["LineChart"].clone();
    value["chart_group_order"] = serde_json::to_value(vec![
        ChartGroupEntry::Additional(Box::new(group(ChartType::LineChart, 20))),
        ChartGroupEntry::Additional(Box::new(group(ChartType::LineChart, 30))),
        ChartGroupEntry::First(ChartType::LineChart),
        ChartGroupEntry::First(ChartType::LineChart),
    ])
    .unwrap();
    let mut plot: PlotArea = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(indices(&plot), vec![20, 30, 10]);
    assert_repair(&mut plot);
    assert_eq!(
        plot.line_chart()
            .unwrap()
            .area_chart_series_list()
            .area_chart_series()[0]
            .index()
            .val(),
        20
    );

    // A missing marker inserts the stored container immediately before its
    // extras.
    value["chart_group_order"]
        .as_array_mut()
        .unwrap()
        .truncate(2);
    let mut plot: PlotArea = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(indices(&plot), vec![10, 20, 30]);
    assert_repair(&mut plot);

    // A stale marker must not pull an orphan ahead of another family.
    value["line_chart"] = Value::Null;
    value["chart_group_order"] = serde_json::to_value(vec![
        ChartGroupEntry::First(ChartType::LineChart),
        ChartGroupEntry::Additional(Box::new(group(ChartType::BarChart, 40))),
        ChartGroupEntry::Additional(Box::new(group(ChartType::LineChart, 20))),
    ])
    .unwrap();
    let mut plot: PlotArea = serde_json::from_value(value).unwrap();
    assert_eq!(indices(&plot), vec![40, 20]);
    assert_repair(&mut plot);
    assert_eq!(indices(&plot), vec![40, 20]);

    // Unmarked first containers without extras follow the declared sequence
    // in legacy order.
    let mut value = serde_json::to_value(PlotArea::default()).unwrap();
    let first = serde_json::to_value(group(ChartType::LineChart, 1)).unwrap();
    value["line_chart"] = first["LineChart"].clone();
    let first = serde_json::to_value(group(ChartType::BarChart, 2)).unwrap();
    value["bar_chart"] = first["BarChart"].clone();
    let first = serde_json::to_value(group(ChartType::AreaChart, 3)).unwrap();
    value["area_chart"] = first["AreaChart"].clone();
    value["chart_group_order"] = serde_json::to_value(vec![
        ChartGroupEntry::First(ChartType::AreaChart),
        ChartGroupEntry::Additional(Box::new(group(ChartType::AreaChart, 4))),
    ])
    .unwrap();
    let mut plot: PlotArea = serde_json::from_value(value).unwrap();
    assert_eq!(indices(&plot), vec![3, 4, 1, 2]);
    assert_repair(&mut plot);
    assert_eq!(indices(&plot), vec![3, 4, 1, 2]);
}

#[test]
fn setters_keep_container_order_canonical() {
    let mut plot = PlotArea::default();
    for (kind, _) in FAMILIES.into_iter().rev() {
        set_group(&mut plot, group(kind, 0));
        assert_canonical(&plot);
        plot.add_chart_group(group(kind, 1));
        assert_canonical(&plot);
        set_group(&mut plot, group(kind, 2));
        assert_canonical(&plot);
    }
    // Newly added fields keep an explicit native-order list's existing
    // positions.
    let mut plot = PlotArea::default();
    plot.add_chart_group(group(ChartType::BarChart, 0));
    plot.add_chart_group(group(ChartType::LineChart, 1));
    plot.set_pie_chart(charts::PieChart::default());
    assert_eq!(
        plot.chart_groups()
            .map(ChartGroupRef::kind)
            .collect::<Vec<_>>(),
        vec![
            ChartType::BarChart,
            ChartType::LineChart,
            ChartType::PieChart
        ]
    );
    let mut repaired = plot.clone();
    repaired.normalize_chart_groups();
    assert_eq!(
        serde_json::to_value(repaired).unwrap(),
        serde_json::to_value(plot).unwrap()
    );

    // Setters on a fresh plot keep legacy order regardless of call order.
    for line_first in [true, false] {
        let mut plot = PlotArea::default();
        if line_first {
            plot.set_line_chart(charts::LineChart::default());
            plot.set_bar_chart(charts::BarChart::default());
        } else {
            plot.set_bar_chart(charts::BarChart::default());
            plot.set_line_chart(charts::LineChart::default());
        }
        assert_eq!(
            plot.chart_groups()
                .map(ChartGroupRef::kind)
                .collect::<Vec<_>>(),
            vec![ChartType::LineChart, ChartType::BarChart]
        );
        assert!(
            serde_json::to_value(&plot).unwrap()["chart_group_order"]
                .as_array()
                .unwrap()
                .is_empty()
        );
    }
}

#[test]
fn legacy_getters_traversal_and_adjustments_share_one_copy() {
    let mut plot = PlotArea::default();
    plot.add_chart_group(group(ChartType::BarChart, 0));
    plot.add_chart_group(group(ChartType::LineChart, 1));
    plot.add_chart_group(group(ChartType::LineChart, 2));
    let before = serde_json::to_value(&plot).unwrap();
    for group in plot.chart_groups_mut() {
        for series in group.series_mut().area_chart_series_mut() {
            let mut label = charts::NumericValue::default();
            label.set_text(format!("Updated series {}", series.index().val()));
            let mut text = charts::ChartText::default();
            text.set_numeric_value(label);
            series.set_chart_text(text);
        }
    }
    plot.bar_chart_mut()
        .unwrap()
        .grouping_mut()
        .set_val(charts::GroupingValues::Stacked);
    let mut color = drawing::RgbColorModelHex::default();
    color.set_val("12AB34");
    let mut fill = drawing::SolidFill::default();
    fill.set_rgb_color_model_hex(color);
    let mut shape = charts::ShapeProperties::default();
    shape.set_solid_fill(fill);
    plot.line_chart_mut()
        .unwrap()
        .area_chart_series_list_mut()
        .area_chart_series_mut()[0]
        .set_shape_properties(shape);
    plot.chart_groups_mut()
        .nth(2)
        .unwrap()
        .series_mut()
        .area_chart_series_mut()[0]
        .values_mut()
        .unwrap()
        .number_reference_mut()
        .formula_mut()
        .set_address_str("Sheet1!$C$2:$C$3");
    plot = insert_row(&plot);
    assert_ne!(serde_json::to_value(&plot).unwrap(), before);
    let json = serde_json::to_value(&plot).unwrap();
    assert_eq!(container_count(&json), 3);
    let restored = assert_xml_round_trip(&plot);
    assert!(matches!(
        restored.bar_chart().unwrap().grouping().val(),
        charts::GroupingValues::Stacked
    ));
    assert_eq!(
        restored
            .line_chart()
            .unwrap()
            .area_chart_series_list()
            .area_chart_series()[0]
            .shape_properties()
            .unwrap()
            .solid_fill()
            .unwrap()
            .rgb_color_model_hex()
            .unwrap()
            .val(),
        "12AB34"
    );
    let formula = restored
        .chart_groups()
        .nth(2)
        .unwrap()
        .series()
        .area_chart_series()[0]
        .values()
        .unwrap()
        .number_reference()
        .formula()
        .address_str();
    assert_eq!(formula, "Sheet1!$C$3:$C$4");
    let xml = chart_xml(&chart_bytes(&plot));
    assert_eq!(xml.matches("12AB34").count(), 1);
    assert_eq!(xml.matches("Updated series ").count(), 3);
    assert!(xml.contains("Sheet1!$C$3:$C$4"));
    assert!(!xml.contains("Sheet1!$C$2:$C$3"));
    let json_restored: PlotArea = serde_json::from_value(json).unwrap();
    assert_eq!(signature(&json_restored), signature(&plot));
    assert_canonical(&plot);
}
