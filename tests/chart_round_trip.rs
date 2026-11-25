//! Charts Excel saved, read and written back, keep what Excel wrote: one error
//! bar on a pie, radar or surface series, the contour and extrusion colours of
//! a 3-D shape, line and bar group settings Excel leaves out, the crossing
//! Excel leaves out of a secondary axis, and Excel's axis order. The same holds
//! after the chart is stored as JSON or binary and after a second xlsx write.
//!
//! Each part in `fixtures/chart_round_trip` is a chart part Excel saved, and
//! every expected value of the read-and-write tests is taken from it. Charts
//! built here, and charts stored before the library kept these parts, still
//! write the group settings, crossings and axis order they wrote before, and
//! keep one error bar on a pie, radar or surface series, as Excel does.
use std::io::Cursor;

use umya_spreadsheet::{
    Chart,
    ChartType,
    Workbook,
    drawing::{
        charts::{
            AreaChartSeriesList,
            CrossesValues,
            ErrorBars,
            Overlap,
            PlotArea,
            SeriesFamily,
            SeriesPartConflictReason,
            SeriesPartKind,
            ShowMarker,
            Smooth,
        },
        spreadsheet::MarkerType,
    },
    reader,
};

#[path = "support/chart_xml_fixture.rs"]
pub mod fixture;
#[path = "support/xml_skeleton.rs"]
mod skeleton;

const ERROR_BAR_PARTS: [&str; 3] = [
    "pie_3d_error_bars_and_contours",
    "radar_error_bars",
    "surface_3d_error_bars",
];

const AXES: [&str; 4] = ["c:catAx", "c:dateAx", "c:valAx", "c:serAx"];

fn part(name: &str) -> String {
    let path = format!(
        "{}/tests/fixtures/chart_round_trip/{name}.xml",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{path}: {error}"))
}

/// The workbook read back from a file whose chart is `xml`, on a sheet named
/// like the one the part refers to.
fn excel_book(xml: &str) -> Workbook {
    let mut book = umya_spreadsheet::new_file();
    let sheet = book.sheet_mut(0).unwrap();
    sheet.set_name("Data");
    sheet.add_chart(fixture::base_chart());
    let file = fixture::replace_chart_part(&fixture::write_workbook(&book), xml);
    reader::xlsx::read_reader(Cursor::new(file), true).unwrap()
}

fn chart_mut(book: &mut Workbook) -> &mut Chart {
    &mut book.sheet_mut(0).unwrap().chart_collection_mut()[0]
}

/// The workbook as read, and with its chart restored from JSON, from binary
/// and from a second xlsx write.
fn stored(book: &Workbook) -> Vec<(&'static str, Workbook)> {
    let chart = &book.sheet(0).unwrap().chart_collection()[0];
    let json: Chart = serde_json::from_slice(&serde_json::to_vec(chart).unwrap()).unwrap();
    let binary: Chart = bincode::deserialize(&bincode::serialize(chart).unwrap()).unwrap();
    let mut books = vec![("read", book.clone())];
    for (route, chart) in [("json", json), ("binary", binary)] {
        let mut copy = book.clone();
        *chart_mut(&mut copy) = chart;
        books.push((route, copy));
    }
    let rewritten = fixture::write_workbook(book);
    books.push((
        "xlsx",
        reader::xlsx::read_reader(Cursor::new(rewritten), true).unwrap(),
    ));
    books
}

fn depth(line: &str) -> usize {
    (line.len() - line.trim_start().len()) / 2
}

fn name(line: &str) -> &str {
    line.trim_start().split(' ').next().unwrap()
}

/// Each `wanted` element of `xml` in document order: its subtree (one line per
/// element, caches left out), its direct children, and the name of the element
/// that follows it under the same parent.
fn elements(xml: &str, wanted: &str) -> Vec<(Vec<String>, Vec<String>, Option<String>)> {
    let lines = skeleton::skeleton(xml);
    let mut found = Vec::new();
    for (at, line) in lines.iter().enumerate() {
        if name(line) != wanted {
            continue;
        }
        let level = depth(line);
        let end = lines[at + 1..]
            .iter()
            .position(|line| depth(line) <= level)
            .map_or(lines.len(), |offset| at + 1 + offset);
        let subtree = lines[at..end]
            .iter()
            .map(|line| line[level * 2..].to_owned())
            .collect();
        let children = lines[at + 1..end]
            .iter()
            .filter(|line| depth(line) == level + 1)
            .map(|line| line.trim_start().to_owned())
            .collect();
        let next = lines
            .get(end)
            .filter(|line| depth(line) == level)
            .map(|line| name(line).to_owned());
        found.push((subtree, children, next));
    }
    found
}

/// The direct children of every `group` element.
fn group_children(xml: &str, group: &str) -> Vec<String> {
    elements(xml, group)
        .into_iter()
        .flat_map(|(_, children, _)| children)
        .collect()
}

fn plot_children(xml: &str) -> Vec<String> {
    skeleton::plot_children(xml)
        .into_iter()
        .map(|(name, _)| name)
        .collect()
}

/// Whether one of `lines` is an `element`.
fn holds(lines: &[String], element: &str) -> bool {
    lines.iter().any(|line| name(line) == element)
}

/// `view` of Excel's part equals `view` of the library's write of it, after
/// every storage route.
fn assert_kept(part_name: &str, view: impl Fn(&str) -> Vec<String>) -> Vec<String> {
    let excel = part(part_name);
    let expected = view(&excel);
    assert!(!expected.is_empty(), "{part_name}: nothing to compare");
    for (route, book) in stored(&excel_book(&excel)) {
        assert_eq!(
            view(&fixture::chart_xml(&book)),
            expected,
            "{part_name} through {route}"
        );
    }
    expected
}

fn error_bars_view(xml: &str) -> Vec<String> {
    elements(xml, "c:errBars")
        .into_iter()
        .map(|(subtree, _, next)| format!("{}\nthen {next:?}", subtree.join("\n")))
        .collect()
}

#[test]
fn pie_radar_and_surface_series_keep_the_error_bar_excel_saved() {
    for (part_name, group) in [
        ("pie_3d_error_bars_and_contours", "c:pie3DChart"),
        ("radar_error_bars", "c:radarChart"),
        ("surface_3d_error_bars", "c:surface3DChart"),
    ] {
        assert!(plot_children(&part(part_name)).contains(&group.to_owned()));
        let bars = assert_kept(part_name, error_bars_view);
        // Excel writes it just before the categories.
        assert!(bars.iter().all(|bar| bar.ends_with("then Some(\"c:cat\")")));
    }
}

fn shapes_3d(xml: &str) -> Vec<String> {
    elements(xml, "a:sp3d")
        .into_iter()
        .map(|(subtree, ..)| subtree.join("\n"))
        .collect()
}

#[test]
fn shape_contours_keep_their_width_and_colour() {
    let with_width = assert_kept("pie_3d_error_bars_and_contours", shapes_3d);
    assert!(with_width.iter().any(|shape| {
        shape.starts_with("a:sp3d contourW=25400\n  a:contourClr\n    a:schemeClr val=lt1")
    }));
    let colour_only = assert_kept("line_3d_contour_colour_only", shapes_3d);
    assert!(colour_only.contains(&"a:sp3d\n  a:contourClr\n    a:srgbClr val=FF0000".to_owned()));
}

#[test]
fn shape_extrusion_colours_keep_their_place_before_the_contour() {
    let alone = assert_kept("series_extrusion_colour_only", shapes_3d);
    assert!(alone.contains(&"a:sp3d\n  a:extrusionClr\n    a:srgbClr val=000000".to_owned()));
    let all = assert_kept("series_bevels_extrusion_and_contour", shapes_3d);
    let expected = [
        "a:sp3d prstMaterial=legacyMetal",
        "  a:bevelT h=50800 w=101600",
        "  a:bevelB h=50800 w=101600",
        "  a:extrusionClr",
        "    a:srgbClr val=000000",
        "  a:contourClr",
        "    a:srgbClr val=000000",
    ];
    assert!(all.contains(&expected.join("\n")), "{all:?}");
}

#[test]
fn a_line_group_without_a_marker_setting_writes_none() {
    let children = assert_kept("line_group_without_marker", |xml| {
        group_children(xml, "c:lineChart")
    });
    assert!(!holds(&children, "c:marker"));
}

#[test]
fn a_bar_group_without_an_overlap_writes_none() {
    for part_name in [
        "combo_without_overlap_or_smooth",
        "stock_secondary_date_axes",
    ] {
        let children = assert_kept(part_name, |xml| group_children(xml, "c:barChart"));
        assert!(!holds(&children, "c:overlap"));
    }
}

#[test]
fn a_secondary_axis_without_a_crossing_writes_none() {
    let view = |xml: &str| -> Vec<String> {
        skeleton::plot_children(xml)
            .into_iter()
            .filter(|(name, _)| AXES.contains(&name.as_str()))
            .map(|(name, lines)| {
                let crossing: Vec<_> = lines
                    .iter()
                    .filter(|line| line.starts_with("  c:crosses"))
                    .map(|line| line.trim_start().to_owned())
                    .collect();
                format!("{name} {crossing:?}")
            })
            .collect()
    };
    for part_name in [
        "secondary_category_axes",
        "stock_secondary_date_axes",
        "scatter_secondary_axis_without_crossing",
    ] {
        let axes = assert_kept(part_name, view);
        assert!(axes.last().unwrap().ends_with(" []"), "{axes:?}");
    }
}

#[test]
fn axes_keep_the_order_excel_wrote() {
    for (part_name, kind) in [
        ("secondary_category_axes", "c:catAx"),
        ("stock_secondary_date_axes", "c:dateAx"),
    ] {
        let children = assert_kept(part_name, plot_children);
        let axes: Vec<_> = children
            .iter()
            .filter(|name| AXES.contains(&name.as_str()))
            .collect();
        assert_eq!(axes, [kind, "c:valAx", "c:valAx", kind]);
    }
}

#[test]
fn a_line_group_without_a_smooth_setting_writes_none() {
    let children = assert_kept("combo_without_overlap_or_smooth", |xml| {
        group_children(xml, "c:lineChart")
    });
    assert!(!holds(&children, "c:smooth"));
    assert!(holds(&children, "c:marker"));
}

#[test]
fn group_settings_written_as_start_and_end_tags_are_kept() {
    let mut paired = part("secondary_category_axes");
    // The blank keeps each pair from being read as one empty element. The
    // smooth value is flipped, so that the default cannot write it.
    for (empty, pair) in [
        (
            r#"<c:marker val="1"/>"#,
            "<c:marker val=\"1\">\n</c:marker>",
        ),
        (
            r#"<c:smooth val="0"/><c:axId"#,
            "<c:smooth val=\"1\">\n</c:smooth><c:axId",
        ),
        (
            r#"<c:overlap val="-27"/>"#,
            "<c:overlap val=\"-27\">\n</c:overlap>",
        ),
    ] {
        assert_eq!(paired.matches(empty).count(), 1, "{empty}");
        paired = paired.replace(empty, pair);
    }
    let xml = fixture::chart_xml(&excel_book(&paired));
    for group in ["c:lineChart", "c:barChart"] {
        assert_eq!(
            group_children(&xml, group),
            group_children(&paired, group),
            "{group}"
        );
    }
}

#[test]
fn a_type_change_into_a_family_without_error_bars_drops_them() {
    let mut book = excel_book(&part("line_3d_contour_colour_only"));
    let line = chart_mut(&mut book)
        .chart_space()
        .chart()
        .plot_area()
        .chart_groups()
        .next()
        .unwrap()
        .series()
        .clone();
    let bars = |list: &AreaChartSeriesList| -> usize {
        list.area_chart_series()
            .iter()
            .map(|series| series.error_bars().len())
            .sum()
    };
    assert_eq!(bars(&line), 1);
    for kind in [
        ChartType::PieChart,
        ChartType::Pie3DChart,
        ChartType::OfPieChart,
        ChartType::DoughnutChart,
        ChartType::RadarChart,
        ChartType::SurfaceChart,
        ChartType::Surface3DChart,
    ] {
        let mut list = line.clone();
        let removed = list.remove_parts_not_allowed_in(SeriesFamily::from(&kind));
        let removed: Vec<_> = removed
            .iter()
            .filter(|removal| removal.part == SeriesPartKind::ErrorBars)
            .map(|removal| removal.occurrence)
            .collect();
        assert_eq!(removed, [0], "{kind:?}");
        assert_eq!(bars(&list), 0, "{kind:?}");
    }

    // The same cleanup on a pie series Excel saved with an error bar leaves
    // nothing for the writer.
    let mut book = excel_book(&part("pie_3d_error_bars_and_contours"));
    chart_mut(&mut book)
        .chart_space_mut()
        .chart_mut()
        .plot_area_mut()
        .chart_groups_mut()
        .next()
        .unwrap()
        .series_mut()
        .remove_parts_not_allowed_in(SeriesFamily::Pie);
    for (route, book) in stored(&book) {
        assert!(
            error_bars_view(&fixture::chart_xml(&book)).is_empty(),
            "{route}"
        );
    }
}

#[test]
fn a_group_that_changes_type_inside_a_family_drops_its_error_bars() {
    // Excel drops the error bar when it converts the 3-D pie part to a doughnut
    // chart and the 3-D surface part to a flat surface chart.
    for (part_name, from, to) in [
        (
            "pie_3d_error_bars_and_contours",
            "c:pie3DChart",
            "c:doughnutChart",
        ),
        (
            "surface_3d_error_bars",
            "c:surface3DChart",
            "c:surfaceChart",
        ),
    ] {
        let excel = part(part_name);
        let basis = excel_book(&excel).sheet(0).unwrap().chart_collection()[0].clone();
        let mut book = excel_book(&excel.replace(from, to));
        let bars = |book: &Workbook| error_bars_view(&fixture::chart_xml(book)).len();
        assert_eq!(bars(&book), 1, "{part_name} before the restoration");
        let projection = chart_mut(&mut book);
        projection.strip_export_parts();
        let report = projection.restore_export_parts(&basis).unwrap();
        // The bar sits on the second series of the only group.
        let removal = "removed incompatible ErrorBars occurrence:0 from group:0/series:1";
        assert!(
            report.diagnostics.contains(&removal.to_owned()),
            "{part_name}: {:?}",
            report.diagnostics
        );
        assert_eq!(bars(&book), 0, "{part_name} after the restoration");
    }
}

#[test]
fn a_second_error_bar_on_those_families_is_reported_and_not_written() {
    for part_name in ERROR_BAR_PARTS {
        let excel = part(part_name);
        let mut book = excel_book(&excel);
        let plot = chart_mut(&mut book).chart_space().chart().plot_area();
        let before = plot.series_part_conflicts();
        assert!(
            before.iter().all(|c| c.part != SeriesPartKind::ErrorBars),
            "{part_name}: {before:?}"
        );
        let list = chart_mut(&mut book)
            .chart_space_mut()
            .chart_mut()
            .plot_area_mut()
            .chart_groups_mut()
            .next()
            .unwrap()
            .series_mut()
            .area_chart_series_mut();
        let index = list
            .iter()
            .position(|series| !series.error_bars().is_empty())
            .unwrap();
        let second: ErrorBars = list[index].error_bars()[0].clone();
        list[index].error_bars_mut().push(second);
        for (route, book) in stored(&book) {
            let conflicts = book.sheet(0).unwrap().chart_collection()[0]
                .chart_space()
                .chart()
                .plot_area()
                .series_part_conflicts();
            let added: Vec<_> = conflicts.iter().filter(|c| !before.contains(c)).collect();
            assert_eq!(
                added.len(),
                usize::from(route != "xlsx"),
                "{part_name} {route}"
            );
            if let Some(conflict) = added.first() {
                assert_eq!(
                    (conflict.series_index, conflict.part, conflict.occurrence),
                    (index, SeriesPartKind::ErrorBars, 1),
                );
                assert_eq!(conflict.reason, SeriesPartConflictReason::TooMany);
            }
            assert_eq!(
                error_bars_view(&fixture::chart_xml(&book)),
                error_bars_view(&excel),
                "{part_name} {route}"
            );
        }
    }
}

#[test]
fn an_edit_writes_what_the_part_left_out() {
    let mut book = excel_book(&part("line_group_without_marker"));
    chart_mut(&mut book)
        .chart_space_mut()
        .chart_mut()
        .plot_area_mut()
        .line_chart_mut()
        .unwrap()
        .show_marker_mut()
        .set_val(true);
    let children = group_children(&fixture::chart_xml(&book), "c:lineChart");
    assert!(children.contains(&"c:marker val=1".to_owned()));

    let mut book = excel_book(&part("combo_without_overlap_or_smooth"));
    let plot = chart_mut(&mut book)
        .chart_space_mut()
        .chart_mut()
        .plot_area_mut();
    plot.line_chart_mut().unwrap().smooth_mut().set_val(true);
    plot.bar_chart_mut().unwrap().overlap_mut().set_val(-20);
    let xml = fixture::chart_xml(&book);
    assert!(group_children(&xml, "c:lineChart").contains(&"c:smooth val=1".to_owned()));
    assert!(group_children(&xml, "c:barChart").contains(&"c:overlap val=-20".to_owned()));

    let mut book = excel_book(&part("secondary_category_axes"));
    chart_mut(&mut book)
        .chart_space_mut()
        .chart_mut()
        .plot_area_mut()
        .category_axis_mut()[1]
        .crosses_mut()
        .set_val(CrossesValues::Maximum);
    let secondary = elements(&fixture::chart_xml(&book), "c:catAx").remove(1).1;
    assert!(secondary.contains(&"c:crosses val=max".to_owned()));

    let mut book = excel_book(&part("scatter_secondary_axis_without_crossing"));
    chart_mut(&mut book)
        .chart_space_mut()
        .chart_mut()
        .plot_area_mut()
        .value_axis_mut()[3]
        .crosses_mut()
        .set_val(CrossesValues::Maximum);
    let deleted = elements(&fixture::chart_xml(&book), "c:valAx").remove(3).1;
    assert!(deleted.contains(&"c:crosses val=max".to_owned()));
}

#[test]
fn every_setter_writes_the_setting_the_part_left_out() {
    // Setter, part, edit, the element that holds the setting, its position
    // among those elements, and the line it writes. The `set_crosses` rows
    // pass the `Crosses` of the axis without a crossing, which carries the
    // absence.
    let rows: [(&str, &str, fn(&mut PlotArea), &str, usize, &str); 11] = [
        (
            "set_show_marker",
            "line_group_without_marker",
            |plot| {
                let mut marker = ShowMarker::default();
                marker.set_val(true);
                plot.line_chart_mut().unwrap().set_show_marker(marker);
            },
            "c:lineChart",
            0,
            "c:marker val=1",
        ),
        (
            "set_smooth",
            "combo_without_overlap_or_smooth",
            |plot| {
                let mut smooth = Smooth::default();
                smooth.set_val(true);
                plot.line_chart_mut().unwrap().set_smooth(smooth);
            },
            "c:lineChart",
            0,
            "c:smooth val=1",
        ),
        (
            "set_overlap",
            "stock_secondary_date_axes",
            |plot| {
                let mut overlap = Overlap::default();
                overlap.set_val(-20);
                plot.bar_chart_mut().unwrap().set_overlap(overlap);
            },
            "c:barChart",
            0,
            "c:overlap val=-20",
        ),
        (
            "DateAxis::set_crosses",
            "stock_secondary_date_axes",
            |plot| {
                let crossing = plot.date_axis()[1].crosses().clone();
                plot.date_axis_mut()[1].set_crosses(crossing);
            },
            "c:dateAx",
            1,
            "c:crosses val=autoZero",
        ),
        (
            "DateAxis::set_crosses_at",
            "stock_secondary_date_axes",
            |plot| {
                plot.date_axis_mut()[1].set_crosses_at(2.5);
            },
            "c:dateAx",
            1,
            "c:crossesAt val=2.5",
        ),
        (
            "DateAxis::remove_crosses_at",
            "stock_secondary_date_axes",
            |plot| {
                plot.date_axis_mut()[1].remove_crosses_at();
            },
            "c:dateAx",
            1,
            "c:crosses val=autoZero",
        ),
        (
            "Crosses::set_val, DateAxis::set_crosses",
            "stock_secondary_date_axes",
            |plot| {
                let mut crossing = plot.date_axis()[1].crosses().clone();
                crossing.set_val(CrossesValues::Minimum);
                plot.date_axis_mut()[1].set_crosses(crossing);
            },
            "c:dateAx",
            1,
            "c:crosses val=min",
        ),
        (
            "ValueAxis::set_crosses",
            "scatter_secondary_axis_without_crossing",
            |plot| {
                let crossing = plot.value_axis()[3].crosses().clone();
                plot.value_axis_mut()[3].set_crosses(crossing);
            },
            "c:valAx",
            3,
            "c:crosses val=autoZero",
        ),
        (
            "ValueAxis::set_crosses_at",
            "scatter_secondary_axis_without_crossing",
            |plot| {
                plot.value_axis_mut()[3].set_crosses_at(2.5);
            },
            "c:valAx",
            3,
            "c:crossesAt val=2.5",
        ),
        (
            "ValueAxis::remove_crosses_at",
            "scatter_secondary_axis_without_crossing",
            |plot| {
                plot.value_axis_mut()[3].remove_crosses_at();
            },
            "c:valAx",
            3,
            "c:crosses val=autoZero",
        ),
        (
            "Crosses::set_val, ValueAxis::set_crosses",
            "scatter_secondary_axis_without_crossing",
            |plot| {
                let mut crossing = plot.value_axis()[3].crosses().clone();
                crossing.set_val(CrossesValues::Minimum);
                plot.value_axis_mut()[3].set_crosses(crossing);
            },
            "c:valAx",
            3,
            "c:crosses val=min",
        ),
    ];
    for (setter, part_name, edit, element, at, line) in rows {
        let mut book = excel_book(&part(part_name));
        edit(
            chart_mut(&mut book)
                .chart_space_mut()
                .chart_mut()
                .plot_area_mut(),
        );
        let (_, children, _) = elements(&fixture::chart_xml(&book), element).remove(at);
        assert!(
            children.contains(&line.to_owned()),
            "{setter} on {part_name}: {children:?}"
        );
    }
}

#[test]
fn added_and_removed_axes_follow_the_order_excel_wrote() {
    let excel = part("secondary_category_axes");
    let mut book = excel_book(&excel);
    let plot = chart_mut(&mut book)
        .chart_space_mut()
        .chart_mut()
        .plot_area_mut();
    plot.category_axis_mut().pop();
    plot.value_axis_mut().pop();
    let mut expected = plot_children(&excel);
    for axis in ["c:catAx", "c:valAx"] {
        let last = expected.iter().rposition(|name| name == axis).unwrap();
        expected.remove(last);
    }
    assert_eq!(plot_children(&fixture::chart_xml(&book)), expected);

    let mut book = excel_book(&excel);
    let plot = chart_mut(&mut book)
        .chart_space_mut()
        .chart_mut()
        .plot_area_mut();
    let extra = plot.value_axis()[0].clone();
    plot.add_value_axis(extra);
    let mut expected = plot_children(&excel);
    let after_axes = expected
        .iter()
        .rposition(|name| AXES.contains(&name.as_str()))
        .unwrap();
    expected.insert(after_axes + 1, "c:valAx".to_owned());
    assert_eq!(plot_children(&fixture::chart_xml(&book)), expected);
}

#[test]
fn charts_built_here_write_what_they_wrote_before() {
    for (kind, group, settings) in [
        (
            ChartType::LineChart,
            "c:lineChart",
            &["c:marker", "c:smooth"][..],
        ),
        (ChartType::BarChart, "c:barChart", &["c:overlap"][..]),
    ] {
        let mut chart = Chart::default();
        chart.new_chart(
            &kind,
            MarkerType::default(),
            MarkerType::default(),
            vec!["Data!$B$2:$B$5"],
        );
        let mut book = umya_spreadsheet::new_file();
        book.sheet_mut(0).unwrap().add_chart(chart);
        let xml = fixture::chart_xml(&book);
        let children = group_children(&xml, group);
        for setting in settings {
            assert!(holds(&children, setting), "{kind:?} {setting}");
        }
        let axes: Vec<_> = plot_children(&xml)
            .into_iter()
            .filter(|name| AXES.contains(&name.as_str()))
            .collect();
        assert_eq!(axes, ["c:catAx", "c:valAx"], "{kind:?}");
        for axis in ["c:catAx", "c:valAx"] {
            for (_, children, _) in elements(&xml, axis) {
                assert!(holds(&children, "c:crosses"), "{kind:?} {axis}");
            }
        }
    }

    // A pie series built here keeps one error bar too; a second is reported
    // and not written.
    let mut chart = Chart::default();
    chart.new_chart(
        &ChartType::PieChart,
        MarkerType::default(),
        MarkerType::default(),
        vec!["Data!$B$2:$B$5"],
    );
    let radar = part("radar_error_bars");
    let bar = excel_book(&radar).sheet(0).unwrap().chart_collection()[0]
        .chart_space()
        .chart()
        .plot_area()
        .chart_groups()
        .flat_map(|group| group.series().area_chart_series())
        .find_map(|series| series.error_bars().first().cloned())
        .unwrap();
    let bars = chart
        .chart_space_mut()
        .chart_mut()
        .plot_area_mut()
        .chart_groups_mut()
        .next()
        .unwrap()
        .series_mut()
        .area_chart_series_mut()[0]
        .error_bars_mut();
    bars.push(bar.clone());
    bars.push(bar);
    let conflicts = chart
        .chart_space()
        .chart()
        .plot_area()
        .series_part_conflicts();
    assert_eq!(conflicts.len(), 1);
    assert_eq!(
        (conflicts[0].part, conflicts[0].occurrence),
        (SeriesPartKind::ErrorBars, 1)
    );
    assert_eq!(conflicts[0].reason, SeriesPartConflictReason::TooMany);
    let mut book = umya_spreadsheet::new_file();
    book.sheet_mut(0).unwrap().add_chart(chart);
    let subtrees = |xml: &str| -> Vec<Vec<String>> {
        elements(xml, "c:errBars")
            .into_iter()
            .map(|(subtree, ..)| subtree)
            .collect()
    };
    assert_eq!(subtrees(&fixture::chart_xml(&book)), subtrees(&radar));
}

/// Removes every `key` from the JSON tree and counts the removals. A key
/// written `owner/name` is removed only from the object held under the field
/// `owner`.
fn remove_keys(value: &mut serde_json::Value, keys: &[&str], removed: &mut [usize]) {
    match value {
        serde_json::Value::Object(fields) => {
            for (key, count) in keys.iter().zip(removed.iter_mut()) {
                let field = match key.split_once('/') {
                    Some((owner, name)) => fields
                        .get_mut(owner)
                        .and_then(serde_json::Value::as_object_mut)
                        .and_then(|object| object.remove(name)),
                    None => fields.remove(*key),
                };
                *count += usize::from(field.is_some());
            }
            for field in fields.values_mut() {
                remove_keys(field, keys, removed);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                remove_keys(item, keys, removed);
            }
        }
        _ => (),
    }
}

#[test]
fn charts_stored_before_these_parts_were_kept_write_what_they_wrote_before() {
    const KEYS: [&str; 8] = [
        "contour_width",
        "contour_color",
        "extrusion_color",
        "marker_absent",
        "smooth_absent",
        "overlap_absent",
        // Only the crossing's flag is new: `SeriesLines` had an `absent` already.
        "crosses/absent",
        "axis_order",
    ];
    let mut removed = [0; KEYS.len()];
    for part_name in [
        "pie_3d_error_bars_and_contours",
        "radar_error_bars",
        "surface_3d_error_bars",
        "line_3d_contour_colour_only",
        "line_group_without_marker",
        "combo_without_overlap_or_smooth",
        "secondary_category_axes",
        "stock_secondary_date_axes",
        "series_extrusion_colour_only",
        "series_bevels_extrusion_and_contour",
        "scatter_secondary_axis_without_crossing",
    ] {
        let mut book = excel_book(&part(part_name));
        let mut json = serde_json::to_value(&book.sheet(0).unwrap().chart_collection()[0]).unwrap();
        remove_keys(&mut json, &KEYS, &mut removed);
        *chart_mut(&mut book) = serde_json::from_value(json).unwrap();
        let xml = fixture::chart_xml(&book);

        // The error bar was stored before as well; it is written, as Excel
        // keeps it.
        if ERROR_BAR_PARTS.contains(&part_name) {
            assert_eq!(
                error_bars_view(&xml),
                error_bars_view(&part(part_name)),
                "{part_name}"
            );
        }
        for (group, setting) in [
            ("c:lineChart", "c:marker"),
            ("c:lineChart", "c:smooth"),
            ("c:barChart", "c:overlap"),
        ] {
            for (_, children, _) in elements(&xml, group) {
                assert!(holds(&children, setting), "{part_name} {group} {setting}");
            }
        }
        let plot = plot_children(&xml);
        let axes: Vec<_> = plot
            .iter()
            .filter(|name| AXES.contains(&name.as_str()))
            .collect();
        let mut grouped = axes.clone();
        grouped.sort_by_key(|name| AXES.iter().position(|axis| *axis == name.as_str()));
        assert_eq!(axes, grouped, "{part_name}");
        for axis in ["c:catAx", "c:dateAx", "c:valAx"] {
            for (_, children, _) in elements(&xml, axis) {
                assert!(
                    holds(&children, "c:crosses") || holds(&children, "c:crossesAt"),
                    "{part_name} {axis}"
                );
            }
        }
        assert!(
            !xml.contains("contourW")
                && !xml.contains("contourClr")
                && !xml.contains("extrusionClr"),
            "{part_name}"
        );

        // Every setting is in the part now, so reading it and writing it
        // again changes nothing.
        let rewritten = fixture::write_workbook(&book);
        let again = reader::xlsx::read_reader(Cursor::new(rewritten), true).unwrap();
        assert_eq!(fixture::chart_xml(&again), xml, "{part_name} written twice");
    }
    assert!(removed.iter().all(|count| *count > 0), "{removed:?}");
}

#[test]
fn a_projection_restored_from_the_read_chart_keeps_what_the_part_left_out() {
    let view = |xml: &str| -> Vec<String> {
        let mut view = plot_children(xml);
        for group in ["c:lineChart", "c:barChart"] {
            view.extend(
                group_children(xml, group)
                    .iter()
                    .map(|line| name(line).to_owned()),
            );
        }
        for axis in AXES {
            for (_, children, _) in elements(xml, axis) {
                view.extend(
                    children
                        .into_iter()
                        .filter(|line| line.starts_with("c:crosses")),
                );
            }
        }
        view
    };
    for part_name in [
        "line_group_without_marker",
        "combo_without_overlap_or_smooth",
        "secondary_category_axes",
        "stock_secondary_date_axes",
        "scatter_secondary_axis_without_crossing",
    ] {
        let excel = part(part_name);
        let mut book = excel_book(&excel);
        let basis = chart_mut(&mut book).clone();
        let projection = chart_mut(&mut book);
        projection.strip_export_parts();
        projection.restore_export_parts(&basis).unwrap();
        assert_eq!(
            view(&fixture::chart_xml(&book)),
            view(&excel),
            "{part_name}"
        );
    }
}
