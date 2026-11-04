#![cfg(feature = "serde")]
//! Stock, surface and series-axis charts, high-low lines, up/down bars and
//! drop lines through the ordinary route: Excel-authored package -> reader ->
//! public API -> writer -> XML. The fixtures in `fixtures/stock_surface` were
//! authored with Excel COM by `stock_surface_excel_author.ps1` (Excel 16.0
//! build 20527); every expected value below comes from those files or from
//! `dml-chart.xsd` (ECMA-376 5th edition Part 4, transitional), never from
//! the library's own output.
use std::{
    io::Cursor,
    path::PathBuf,
};

use umya_spreadsheet::{
    Chart,
    ChartType,
    Workbook,
    drawing::{
        charts::ChartGroupRef,
        spreadsheet::MarkerType,
    },
    reader,
    writer,
};

#[path = "support/xml_skeleton.rs"]
mod skeleton;
use skeleton::{
    chart_part,
    normalized_plot,
};

fn fixture(name: &str) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/stock_surface")
        .join(format!("{name}.xlsx"));
    std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn read(bytes: &[u8]) -> Workbook {
    reader::xlsx::read_reader(Cursor::new(bytes.to_vec()), true).unwrap()
}

fn write(book: &Workbook) -> Vec<u8> {
    let mut out = Cursor::new(Vec::new());
    writer::xlsx::write_writer(book, &mut out).unwrap();
    out.into_inner()
}

fn chart(book: &Workbook) -> &Chart {
    &book.sheet(0).unwrap().chart_collection()[0]
}

fn chart_mut(book: &mut Workbook) -> &mut Chart {
    &mut book.sheet_mut(0).unwrap().chart_collection_mut()[0]
}

fn kinds(book: &Workbook) -> Vec<ChartType> {
    chart(book)
        .chart_space()
        .chart()
        .plot_area()
        .chart_groups()
        .map(ChartGroupRef::kind)
        .collect()
}

const EXCEL_FILES: [&str; 13] = [
    "stock-hlc",
    "stock-ohlc",
    "stock-vhlc",
    "stock-vohlc",
    "surface",
    "surface-wireframe",
    "surface-topview",
    "surface-topview-wireframe",
    "line-lines",
    "line3d",
    "column3d-series-axis",
    "area-droplines",
    "area3d-droplines",
];

#[test]
fn every_excel_chart_round_trips_element_by_element() {
    for name in EXCEL_FILES {
        let bytes = fixture(name);
        let output = write(&read(&bytes));
        let expected = normalized_plot(&chart_part(&bytes));
        let actual = normalized_plot(&chart_part(&output));
        if expected != actual {
            let first = expected
                .iter()
                .zip(&actual)
                .position(|(a, b)| a != b)
                .unwrap_or(expected.len().min(actual.len()));
            panic!(
                "{name}: plot area differs at line {first}\nexcel:   {:?}\nlibrary: {:?}",
                expected.get(first),
                actual.get(first)
            );
        }
        // The same chart rewritten again is stable.
        assert_eq!(
            normalized_plot(&chart_part(&write(&read(&output)))),
            actual,
            "{name} is not stable on the second pass"
        );
    }
}

#[test]
fn excel_chart_types_are_recognised_as_groups() {
    use ChartType::*;
    for (name, expected) in [
        ("stock-hlc", vec![StockChart]),
        ("stock-ohlc", vec![StockChart]),
        ("stock-vhlc", vec![BarChart, StockChart]),
        ("stock-vohlc", vec![BarChart, StockChart]),
        ("surface", vec![Surface3DChart]),
        ("surface-wireframe", vec![Surface3DChart]),
        ("surface-topview", vec![SurfaceChart]),
        ("surface-topview-wireframe", vec![SurfaceChart]),
        ("line-lines", vec![LineChart]),
        ("line3d", vec![Line3DChart]),
        ("column3d-series-axis", vec![Bar3DChart]),
        ("area-droplines", vec![AreaChart]),
        ("area3d-droplines", vec![Area3DChart]),
    ] {
        assert_eq!(kinds(&read(&fixture(name))), expected, "{name}");
    }
}

#[test]
fn stock_chart_reads_series_lines_and_bars_as_excel_wrote_them() {
    let book = read(&fixture("stock-ohlc"));
    let plot = chart(&book).chart_space().chart().plot_area();
    let stock = plot.stock_chart().unwrap();
    assert_eq!(stock.area_chart_series_list().area_chart_series().len(), 4);
    assert!(stock.hi_low_lines().is_some());
    assert!(stock.drop_lines().is_none());
    let bars = stock.up_down_bars().unwrap();
    assert_eq!(bars.gap_width().unwrap().val(), 150);
    assert!(bars.up_bars().unwrap().shape_properties().is_some());
    assert!(bars.down_bars().unwrap().shape_properties().is_some());
    assert_eq!(stock.axis_id().len(), 2);
    let hlc = read(&fixture("stock-hlc"));
    let stock = chart(&hlc)
        .chart_space()
        .chart()
        .plot_area()
        .stock_chart()
        .unwrap();
    assert_eq!(stock.area_chart_series_list().area_chart_series().len(), 3);
    assert!(stock.hi_low_lines().is_some());
    assert!(stock.up_down_bars().is_none());
}

#[test]
fn volume_stock_chart_is_one_chart_with_a_bar_and_a_stock_group() {
    let bytes = fixture("stock-vohlc");
    let book = read(&bytes);
    assert_eq!(book.sheet(0).unwrap().chart_collection().len(), 1);
    let plot = chart(&book).chart_space().chart().plot_area();
    assert_eq!(plot.bar_chart().unwrap().axis_id().len(), 2);
    assert_eq!(plot.stock_chart().unwrap().axis_id().len(), 2);
    // Two category and two value axes: primary and secondary.
    assert_eq!(plot.category_axis().len(), 2);
    assert_eq!(plot.value_axis().len(), 2);
    let xml = chart_part(&write(&book));
    assert_eq!(xml.matches("<c:barChart>").count(), 1);
    assert_eq!(xml.matches("<c:stockChart>").count(), 1);
    assert!(xml.find("<c:barChart>").unwrap() < xml.find("<c:stockChart>").unwrap());
}

#[test]
fn surface_charts_keep_wireframe_bands_and_series_axis() {
    for (name, wireframe, three_d) in [
        ("surface", false, true),
        ("surface-wireframe", true, true),
        ("surface-topview", false, false),
        ("surface-topview-wireframe", true, false),
    ] {
        let book = read(&fixture(name));
        let plot = chart(&book).chart_space().chart().plot_area();
        let (series, bands, axes, got) = if three_d {
            let group = plot.surface_3d_chart().unwrap();
            (
                group.area_chart_series_list().area_chart_series().len(),
                group.band_formats().map(<[_]>::len),
                group.axis_id().len(),
                group.wireframe().val(),
            )
        } else {
            let group = plot.surface_chart().unwrap();
            (
                group.area_chart_series_list().area_chart_series().len(),
                group.band_formats().map(<[_]>::len),
                group.axis_id().len(),
                group.wireframe().val(),
            )
        };
        assert_eq!(series, 4, "{name}");
        assert_eq!(got, wireframe, "{name}");
        assert_eq!(axes, 3, "{name}");
        assert!(bands.unwrap() >= 1, "{name}");
        assert_eq!(plot.series_axis().len(), 1, "{name}");
        // The band indexes are consecutive from zero, as Excel numbers them.
        let xml = chart_part(&write(&book));
        let band_count = bands.unwrap();
        assert_eq!(xml.matches("<c:bandFmt>").count(), band_count, "{name}");
        assert!(
            xml.contains(&format!("<c:idx val=\"{}\"/>", band_count - 1)),
            "{name}"
        );
    }
}

#[test]
fn series_axis_formatting_survives_the_round_trip() {
    for name in ["column3d-series-axis", "line3d", "surface"] {
        let bytes = fixture(name);
        let book = read(&bytes);
        let axis = &chart(&book).chart_space().chart().plot_area().series_axis()[0];
        assert!(axis.shape_properties().is_some(), "{name}");
        assert!(axis.text_properties().is_some(), "{name}");
        let excel = skeleton::plot_children(&chart_part(&bytes));
        let library = skeleton::plot_children(&chart_part(&write(&book)));
        let find = |all: &[(String, Vec<String>)]| {
            all.iter()
                .find(|(tag, _)| tag == "c:serAx")
                .map(|(_, lines)| lines.clone())
                .unwrap()
        };
        assert_eq!(find(&excel), find(&library), "{name}");
    }
}

#[test]
fn line_group_lines_follow_the_schema_order() {
    let bytes = fixture("line-lines");
    let book = read(&bytes);
    let line = chart(&book)
        .chart_space()
        .chart()
        .plot_area()
        .line_chart()
        .unwrap();
    assert!(line.drop_lines().is_some());
    assert!(line.hi_low_lines().is_some());
    assert!(line.up_down_bars().is_some());
    let xml = chart_part(&write(&book));
    let group = skeleton::element(&xml, "c:lineChart");
    let order = [
        "<c:dLbls>",
        "<c:dropLines>",
        "<c:hiLowLines>",
        "<c:upDownBars>",
        "<c:marker val",
        "<c:smooth val",
        "<c:axId ",
    ];
    // Each child follows the previous one; the series' own `c:smooth` comes
    // before the group's children, so search from the last dLbls on.
    let mut at = group.rfind("<c:dLbls>").unwrap();
    for tag in order {
        let found = group[at..]
            .find(tag)
            .unwrap_or_else(|| panic!("{tag} out of order in {group}"));
        at += found + tag.len();
    }
}

#[test]
fn drop_lines_are_kept_on_area_and_area_3d_groups() {
    for name in ["area-droplines", "area3d-droplines"] {
        let book = read(&fixture(name));
        let xml = chart_part(&write(&book));
        assert_eq!(xml.matches("<c:dropLines>").count(), 1, "{name}");
        let labels = xml.find("</c:dLbls>").unwrap();
        let drop = xml.find("<c:dropLines>").unwrap();
        let ax = xml.find("<c:axId ").unwrap();
        assert!(labels < drop && drop < ax, "{name}");
    }
}

#[test]
fn stock_series_count_is_kept_as_excel_keeps_it() {
    // Excel itself saves 2 series after a series is deleted from a
    // high-low-close chart, and opens a stock chart with 5 series.
    for (name, series) in [("count-hlc-delete1", 2), ("file-ohlc-five-series", 5)] {
        let book = read(&fixture(name));
        let stock = chart(&book)
            .chart_space()
            .chart()
            .plot_area()
            .stock_chart()
            .unwrap();
        assert_eq!(
            stock.area_chart_series_list().area_chart_series().len(),
            series
        );
        let xml = chart_part(&write(&book));
        assert_eq!(
            skeleton::element(&xml, "c:stockChart")
                .matches("<c:ser>")
                .count(),
            series
        );
    }
}

fn with_chart_part(bytes: &[u8], edit: impl FnOnce(String) -> String) -> Vec<u8> {
    use std::io::{
        Read,
        Write,
    };
    let mut input = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let mut edit = Some(edit);
    for index in 0..input.len() {
        let mut entry = input.by_index(index).unwrap();
        let mut data = Vec::new();
        entry.read_to_end(&mut data).unwrap();
        if entry.name().unwrap() == "xl/charts/chart1.xml" {
            let text = String::from_utf8(data).unwrap();
            data = edit.take().unwrap()(text).into_bytes();
        }
        output
            .start_file(
                entry.name().unwrap(),
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
        output.write_all(&data).unwrap();
    }
    output.finish().unwrap().into_inner()
}

const HIDDEN_NS: &str = "xmlns:h=\"urn:hidden\"";

fn hidden(marker: &str) -> String {
    format!(
        "<c:extLst><c:ext uri=\"urn:hidden\" {HIDDEN_NS}><h:p>{marker}</h:p></c:ext></c:extLst>"
    )
}

fn hidden_shape(marker: &str) -> String {
    format!(
        "<c:spPr><a:ln w=\"9525\"><a:solidFill><a:srgbClr \
         val=\"112233\"/></a:solidFill></a:ln><a:extLst><a:ext uri=\"urn:hidden\" \
         {HIDDEN_NS}><h:p>{marker}</h:p></a:ext></a:extLst></c:spPr>"
    )
}

fn replace_once(text: &str, from: &str, to: &str) -> String {
    assert!(text.contains(from), "{from} not found");
    text.replacen(from, to, 1)
}

/// Excel's stock chart with a hidden part in every stock, surface and
/// line-group owner.
fn stock_with_hidden_parts() -> Vec<u8> {
    with_chart_part(&fixture("stock-ohlc"), |xml| {
        let mut xml = xml;
        // group extension list, before the group's closing tag
        xml = replace_once(
            &xml,
            "</c:stockChart>",
            &format!("{}</c:stockChart>", hidden("GROUPEXT")),
        );
        // hi-low lines: hidden extension inside its shape properties
        let hi = skeleton::element(&xml, "c:hiLowLines").to_owned();
        xml = replace_once(
            &xml,
            &hi,
            &format!("<c:hiLowLines>{}</c:hiLowLines>", hidden_shape("HILOWEXT")),
        );
        // up/down bars: own extension list and a hidden part in each bar
        xml = replace_once(
            &xml,
            "</c:upDownBars>",
            &format!("{}</c:upDownBars>", hidden("BARSEXT")),
        );
        let up = skeleton::element(&xml, "c:upBars").to_owned();
        xml = replace_once(
            &xml,
            &up,
            &format!("<c:upBars>{}</c:upBars>", hidden_shape("UPBAREXT")),
        );
        xml
    })
}

fn xml_of(book: &Workbook) -> String {
    chart_part(&write(book))
}

#[test]
fn hidden_parts_of_stock_owners_are_kept_stripped_and_restored() {
    let book = read(&stock_with_hidden_parts());
    let full = xml_of(&book);
    for marker in ["GROUPEXT", "HILOWEXT", "BARSEXT", "UPBAREXT"] {
        assert!(full.contains(marker), "{marker} lost by the reader/writer");
    }

    let mut projected = chart(&book).clone();
    projected.strip_export_parts();
    let mut stripped_book = book.clone();
    *chart_mut(&mut stripped_book) = projected.clone();
    let stripped = xml_of(&stripped_book);
    for marker in ["GROUPEXT", "HILOWEXT", "BARSEXT", "UPBAREXT"] {
        assert!(!stripped.contains(marker), "{marker} survived the strip");
    }
    // The visible owners stay.
    assert!(stripped.contains("<c:hiLowLines>"));
    assert!(stripped.contains("<c:upDownBars>"));

    let mut restored = projected;
    let report = restored.restore_export_parts(chart(&book)).unwrap();
    assert!(report.restored_owners >= 4, "{report:?}");
    let mut restored_book = book.clone();
    *chart_mut(&mut restored_book) = restored;
    let again = xml_of(&restored_book);
    for marker in ["GROUPEXT", "HILOWEXT", "BARSEXT", "UPBAREXT"] {
        assert!(again.contains(marker), "{marker} not restored");
    }
}

#[test]
fn an_explicit_removal_wins_over_restoration() {
    let book = read(&stock_with_hidden_parts());
    let mut edited = chart(&book).clone();
    edited.strip_export_parts();
    {
        let plot = edited.chart_space_mut().chart_mut().plot_area_mut();
        let mut groups = plot.chart_groups_mut();
        let group = groups.next().unwrap();
        assert!(group.remove_hi_low_lines());
    }
    {
        let plot = edited.chart_space_mut().chart_mut().plot_area_mut();
        let group = plot.chart_groups_mut().next().unwrap();
        assert!(group.remove_up_down_bars());
    }
    // The authoritative copy for restoration holds the removals too.
    let mut basis = chart(&book).clone();
    for remove in [true, false] {
        let plot = basis.chart_space_mut().chart_mut().plot_area_mut();
        let group = plot.chart_groups_mut().next().unwrap();
        if remove {
            assert!(group.remove_hi_low_lines());
        } else {
            assert!(group.remove_up_down_bars());
        }
    }
    edited.restore_export_parts(&basis).unwrap();
    let mut out = book.clone();
    *chart_mut(&mut out) = edited;
    let xml = xml_of(&out);
    assert!(!xml.contains("<c:hiLowLines>"));
    assert!(!xml.contains("<c:upDownBars>"));
    assert!(!xml.contains("HILOWEXT"));
    assert!(!xml.contains("BARSEXT"));
    assert!(!xml.contains("UPBAREXT"));
    // The group's own hidden part is not part of the removal.
    assert!(xml.contains("GROUPEXT"));
}

#[test]
fn removal_reports_whether_the_group_type_has_the_element() {
    let mut book = read(&fixture("surface"));
    let plot = chart_mut(&mut book)
        .chart_space_mut()
        .chart_mut()
        .plot_area_mut();
    assert!(
        !plot
            .chart_groups_mut()
            .next()
            .unwrap()
            .remove_hi_low_lines()
    );
    let plot = chart_mut(&mut book)
        .chart_space_mut()
        .chart_mut()
        .plot_area_mut();
    assert!(!plot.chart_groups_mut().next().unwrap().remove_drop_lines());
    let mut book = read(&fixture("area-droplines"));
    let plot = chart_mut(&mut book)
        .chart_space_mut()
        .chart_mut()
        .plot_area_mut();
    assert!(plot.chart_groups_mut().next().unwrap().remove_drop_lines());
    assert!(!xml_of(&book).contains("<c:dropLines>"));
}

#[test]
fn surface_hidden_parts_are_kept_stripped_and_restored() {
    let bytes = with_chart_part(&fixture("surface"), |xml| {
        let mut xml = replace_once(
            &xml,
            "</c:surface3DChart>",
            &format!("{}</c:surface3DChart>", hidden("SURFACEEXT")),
        );
        let band = skeleton::element(&xml, "c:bandFmt").to_owned();
        xml = replace_once(
            &xml,
            &band,
            &format!(
                "<c:bandFmt><c:idx val=\"0\"/>{}</c:bandFmt>",
                hidden_shape("BANDEXT")
            ),
        );
        xml
    });
    let book = read(&bytes);
    let full = xml_of(&book);
    assert!(full.contains("SURFACEEXT"));
    assert!(full.contains("BANDEXT"));
    let mut projected = chart(&book).clone();
    projected.strip_export_parts();
    let mut stripped = book.clone();
    *chart_mut(&mut stripped) = projected.clone();
    let stripped_xml = xml_of(&stripped);
    assert!(!stripped_xml.contains("SURFACEEXT"));
    assert!(!stripped_xml.contains("BANDEXT"));
    assert_eq!(stripped_xml.matches("<c:bandFmt>").count(), 15);
    projected.restore_export_parts(chart(&book)).unwrap();
    let mut restored = book.clone();
    *chart_mut(&mut restored) = projected;
    let restored_xml = xml_of(&restored);
    assert!(restored_xml.contains("SURFACEEXT"));
    assert!(restored_xml.contains("BANDEXT"));
}

fn row_insert_formulas(name: &str) -> (Vec<String>, Vec<String>) {
    let mut book = read(&fixture(name));
    let before = formulas(&xml_of(&book));
    book.sheet_mut(0).unwrap().insert_new_row(3, 2);
    (before, formulas(&xml_of(&book)))
}

fn formulas(xml: &str) -> Vec<String> {
    xml.split("<c:f>")
        .skip(1)
        .map(|rest| rest.split("</c:f>").next().unwrap().to_owned())
        .collect()
}

#[test]
fn row_insert_moves_stock_and_surface_series_formulas() {
    // Data rows 2..6 in every fixture; inserting two rows at row 3 moves the
    // end of each range from 6 to 8 and leaves the header row alone.
    for name in ["stock-ohlc", "surface", "surface-topview"] {
        let (before, after) = row_insert_formulas(name);
        assert_eq!(before.len(), after.len(), "{name}");
        assert!(before.iter().any(|f| f.ends_with("$6")), "{name}");
        for (old, new) in before.iter().zip(&after) {
            let expected = if old.contains("$2:$") {
                old.replace("$6", "$8")
            } else {
                old.clone()
            };
            assert_eq!(new, &expected, "{name}: {old}");
        }
    }
}

#[test]
fn created_stock_and_surface_charts_write_schema_ordered_parts() {
    for (kind, series, group, tags) in [
        (
            ChartType::StockChart,
            vec!["Sheet1!$B$2:$B$6", "Sheet1!$C$2:$C$6", "Sheet1!$D$2:$D$6"],
            "c:stockChart",
            vec![
                "<c:ser>",
                "<c:dLbls>",
                "<c:hiLowLines>",
                "<c:axId ",
                "<c:axId ",
            ],
        ),
        (
            ChartType::StockChart,
            vec![
                "Sheet1!$B$2:$B$6",
                "Sheet1!$C$2:$C$6",
                "Sheet1!$D$2:$D$6",
                "Sheet1!$E$2:$E$6",
            ],
            "c:stockChart",
            vec![
                "<c:ser>",
                "<c:dLbls>",
                "<c:hiLowLines>",
                "<c:upDownBars>",
                "<c:axId ",
            ],
        ),
        (
            ChartType::Surface3DChart,
            vec!["Sheet1!$B$2:$B$6", "Sheet1!$C$2:$C$6"],
            "c:surface3DChart",
            vec![
                "<c:wireframe ",
                "<c:ser>",
                "<c:axId ",
                "<c:axId ",
                "<c:axId ",
            ],
        ),
        (
            ChartType::SurfaceChart,
            vec!["Sheet1!$B$2:$B$6", "Sheet1!$C$2:$C$6"],
            "c:surfaceChart",
            vec![
                "<c:wireframe ",
                "<c:ser>",
                "<c:axId ",
                "<c:axId ",
                "<c:axId ",
            ],
        ),
    ] {
        let mut book = umya_spreadsheet::new_file();
        let mut created = Chart::default();
        created.new_chart(
            &kind,
            MarkerType::default(),
            MarkerType::default(),
            series.clone(),
        );
        book.sheet_mut(0).unwrap().add_chart(created);
        let bytes = write(&book);
        let xml = chart_part(&bytes);
        let body = skeleton::element(&xml, group);
        let mut at = 0;
        for tag in &tags {
            let found = body[at..]
                .find(tag)
                .unwrap_or_else(|| panic!("{kind:?}: {tag} out of order in {body}"));
            at += found + tag.len();
        }
        // A reread keeps the series and the kind.
        let again = read(&bytes);
        assert_eq!(kinds(&again), vec![kind.clone()]);
        let plot = chart(&again).chart_space().chart().plot_area();
        let count = plot
            .chart_groups()
            .next()
            .unwrap()
            .series()
            .area_chart_series()
            .len();
        assert_eq!(count, series.len());
        // Surface series write only c:cat and c:val after the shared children.
        if group.starts_with("c:surface") {
            assert!(!body.contains("<c:marker") && !body.contains("<c:smooth"));
            assert_eq!(plot.series_axis().len(), 1);
        }
        emit(
            &format!(
                "created-{}-{}",
                group.trim_start_matches("c:"),
                series.len()
            ),
            &bytes,
        );
    }
}

#[test]
fn surface_series_cannot_hold_points_or_labels() {
    let bytes = with_chart_part(&fixture("surface"), |xml| {
        replace_once(
            &xml,
            "<c:cat>",
            "<c:dPt><c:idx val=\"1\"/></c:dPt><c:dLbls><c:showVal val=\"1\"/></c:dLbls><c:cat>",
        )
    });
    let book = read(&bytes);
    let xml = xml_of(&book);
    let group = skeleton::element(&xml, "c:surface3DChart");
    assert!(!group.contains("<c:dPt>"));
    assert!(!group.contains("<c:dLbls>"));
    let conflicts = chart(&book)
        .chart_space()
        .chart()
        .plot_area()
        .series_part_conflicts();
    assert!(
        conflicts
            .iter()
            .any(|c| format!("{:?}", c.part) == "DataPoints")
    );
    assert!(
        conflicts
            .iter()
            .any(|c| format!("{:?}", c.part) == "DataLabels")
    );
}

#[test]
fn duplicate_group_lines_are_an_error() {
    let bytes = with_chart_part(&fixture("stock-hlc"), |xml| {
        let lines = skeleton::element(&xml, "c:hiLowLines").to_owned();
        replace_once(&xml, &lines, &format!("{lines}{lines}"))
    });
    let result = reader::xlsx::read_reader(Cursor::new(bytes), true);
    let message = format!(
        "{:?}",
        result.err().expect("duplicate c:hiLowLines must fail")
    );
    assert!(message.contains("hiLowLines"), "{message}");
}

#[test]
fn persisted_charts_from_before_the_new_groups_still_load() {
    // A chart saved by the previous revision has none of the keys this
    // revision added.
    for name in [
        "line-lines",
        "stock-ohlc",
        "surface",
        "column3d-series-axis",
    ] {
        let book = read(&fixture(name));
        let mut json = serde_json::to_value(chart(&book)).unwrap();
        let removed = strip_keys(
            &mut json,
            &[
                "drop_lines",
                "hi_low_lines",
                "up_down_bars",
                "stock_chart",
                "surface_chart",
                "surface_3d_chart",
            ],
        );
        assert!(removed > 0, "{name}");
        let old: Chart = serde_json::from_value(json).unwrap();
        // Old JSON of a chart without the new groups is a line, bar, ...
        // chart without lines: it loads and writes.
        let mut reloaded = read(&fixture(name));
        *chart_mut(&mut reloaded) = old;
        let _ = write(&reloaded);
    }
    // An old line chart keeps its group and series.
    let book = read(&fixture("line3d"));
    let json = serde_json::to_string(chart(&book)).unwrap();
    let reloaded: Chart = serde_json::from_str(&json).unwrap();
    assert_eq!(
        reloaded
            .chart_space()
            .chart()
            .plot_area()
            .chart_groups()
            .count(),
        1
    );
}

fn strip_keys(value: &mut serde_json::Value, keys: &[&str]) -> usize {
    match value {
        serde_json::Value::Object(map) => {
            let mut removed = 0;
            for key in keys {
                if map.remove(*key).is_some() {
                    removed += 1;
                }
            }
            removed + map.values_mut().map(|v| strip_keys(v, keys)).sum::<usize>()
        }
        serde_json::Value::Array(items) => items.iter_mut().map(|v| strip_keys(v, keys)).sum(),
        _ => 0,
    }
}

#[test]
fn persistence_round_trips_every_new_owner() {
    for name in EXCEL_FILES {
        let book = read(&fixture(name));
        let json: Chart =
            serde_json::from_str(&serde_json::to_string(chart(&book)).unwrap()).unwrap();
        let binary: Chart =
            bincode::deserialize(&bincode::serialize(chart(&book)).unwrap()).unwrap();
        for copy in [json, binary] {
            let mut out = book.clone();
            *chart_mut(&mut out) = copy;
            assert_eq!(
                normalized_plot(&xml_of(&out)),
                normalized_plot(&xml_of(&book)),
                "{name}"
            );
        }
    }
}

/// Writes `bytes` where `stock_surface_excel_open.ps1` finds it.
fn emit(name: &str, bytes: &[u8]) {
    if let Some(target) = std::env::var_os("CARGO_TARGET_DIR") {
        let directory = PathBuf::from(target).join("stock-surface-validation");
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join(format!("{name}.xlsx")), bytes).unwrap();
    }
}

fn first_group_of(book: &mut Workbook) -> umya_spreadsheet::drawing::charts::ChartGroupMut<'_> {
    chart_mut(book)
        .chart_space_mut()
        .chart_mut()
        .plot_area_mut()
        .chart_groups_mut()
        .next()
        .unwrap()
}

#[test]
fn edited_charts_write_what_the_edit_asked_for() {
    // Round trip of every Excel chart, unedited.
    for name in EXCEL_FILES {
        emit(&format!("roundtrip-{name}"), &write(&read(&fixture(name))));
    }
    // A series range change on a stock and a surface chart.
    for (name, group) in [
        ("stock-ohlc", "c:stockChart"),
        ("surface", "c:surface3DChart"),
    ] {
        let mut book = read(&fixture(name));
        {
            let plot = chart_mut(&mut book)
                .chart_space_mut()
                .chart_mut()
                .plot_area_mut();
            let mut formulas = plot.formula_mut();
            let values = formulas
                .iter_mut()
                .find(|f| f.address_str() == "Sheet1!$B$2:$B$6")
                .expect("series 1 values");
            values.set_address_str("Sheet1!$B$2:$B$5");
        }
        let bytes = write(&book);
        let xml = chart_part(&bytes);
        let body = skeleton::element(&xml, group);
        assert!(body.contains("<c:f>Sheet1!$B$2:$B$5</c:f>"), "{name}");
        assert!(!body.contains("<c:f>Sheet1!$B$2:$B$6</c:f>"), "{name}");
        emit(&format!("edit-range-{name}"), &bytes);
    }
    // Element removals, as Excel's Lines and Up/Down Bars menus do.
    let mut book = read(&fixture("stock-ohlc"));
    assert!(first_group_of(&mut book).remove_up_down_bars());
    assert!(first_group_of(&mut book).remove_hi_low_lines());
    let bytes = write(&book);
    let xml = chart_part(&bytes);
    assert!(!xml.contains("<c:upDownBars>") && !xml.contains("<c:hiLowLines>"));
    emit("edit-removed-stock-lines-and-bars", &bytes);

    let mut book = read(&fixture("line-lines"));
    assert!(first_group_of(&mut book).remove_drop_lines());
    assert!(first_group_of(&mut book).remove_hi_low_lines());
    assert!(first_group_of(&mut book).remove_up_down_bars());
    let bytes = write(&book);
    let xml = chart_part(&bytes);
    for tag in ["<c:dropLines>", "<c:hiLowLines>", "<c:upDownBars>"] {
        assert!(!xml.contains(tag), "{tag}");
    }
    emit("edit-removed-line-lines", &bytes);
}

/// `<c:x val="1"/>` -> `<c:x val="1"></c:x>`: the same document with every
/// empty element written as a start and an end tag.
fn expand_empty_elements(xml: &str) -> String {
    let mut out = String::with_capacity(xml.len() * 2);
    let mut rest = xml;
    while let Some(open) = rest.find('<') {
        out.push_str(&rest[..open]);
        let close = rest[open..].find('>').unwrap() + open;
        let tag = &rest[open..=close];
        if tag.ends_with("/>") && !tag.starts_with("<?") && !tag.starts_with("<!") {
            let inner = &tag[1..tag.len() - 2];
            let name = inner.split_whitespace().next().unwrap();
            out.push_str(&format!("<{inner}></{name}>"));
        } else {
            out.push_str(tag);
        }
        rest = &rest[close + 1..];
    }
    out.push_str(rest);
    out
}

/// The plot-area parts the stock and surface readers own: every chart group and
/// the series axis text and shape, and the line and bar elements of the other
/// groups. The leaves of the older group and axis readers are not part of this
/// comparison.
fn owned_plot_parts(xml: &str) -> Vec<String> {
    const OWN_GROUPS: [&str; 3] = ["c:stockChart", "c:surfaceChart", "c:surface3DChart"];
    const OWN_CHILDREN: [&str; 3] = ["  c:dropLines", "  c:hiLowLines", "  c:upDownBars"];
    let mut lines = Vec::new();
    for (name, body) in skeleton::plot_children(xml) {
        if name == "c:serAx" {
            let from = body
                .iter()
                .position(|l| l == "  c:spPr")
                .unwrap_or(body.len());
            let to = body
                .iter()
                .position(|l| l.starts_with("  c:crossAx"))
                .unwrap_or(from);
            lines.push(name);
            lines.extend(body[from..to.max(from)].iter().cloned());
        } else if OWN_GROUPS.contains(&name.as_str()) {
            lines.push(name);
            lines.extend(body);
        } else if name.ends_with("Chart") {
            // Other groups: only the line and bar elements added to them.
            lines.push(name);
            let mut inside = false;
            for line in body {
                if line.starts_with("  ") && !line.starts_with("   ") {
                    inside = OWN_CHILDREN.iter().any(|own| line.starts_with(own));
                }
                if inside {
                    lines.push(line);
                }
            }
        }
    }
    lines
}

#[test]
fn start_and_end_tag_leaves_import_like_empty_elements() {
    use umya_spreadsheet::writer::xlsx::write_writer_light;
    for name in EXCEL_FILES {
        let bytes = fixture(name);
        let expected = owned_plot_parts(&chart_part(&write(&read(&bytes))));
        let expanded = with_chart_part(&bytes, |xml| {
            let text = expand_empty_elements(&xml);
            assert_ne!(text, xml, "{name} has no empty element to expand");
            text
        });
        for eager in [true, false] {
            let mut book = reader::xlsx::read_reader(Cursor::new(expanded.clone()), eager)
                .unwrap_or_else(|e| panic!("{name} eager={eager}: {e}"));
            book.try_read_sheet(0).unwrap();
            let mut light = Cursor::new(Vec::new());
            write_writer_light(&book, &mut light).unwrap();
            for (mode, output) in [("full", write(&book)), ("light", light.into_inner())] {
                assert_eq!(
                    owned_plot_parts(&chart_part(&output)),
                    expected,
                    "{name} eager={eager} {mode}"
                );
                emit(&format!("paired-{name}-{mode}"), &output);
            }
        }
    }
}

#[test]
fn comments_inside_stock_and_surface_leaves_preserve_values() {
    for name in ["stock-ohlc", "surface"] {
        let bytes = fixture(name);
        let expected = owned_plot_parts(&chart_part(&write(&read(&bytes))));
        let expanded = with_chart_part(&bytes, |xml| {
            let xml = expand_empty_elements(&xml)
                .replace("></c:idx>", "> <!--leaf--> </c:idx>")
                .replace("></c:gapWidth>", "> <!--leaf--> </c:gapWidth>");
            assert!(xml.contains(" <!--leaf--> </c:idx>"), "{name}");
            if name == "stock-ohlc" {
                assert!(xml.contains(" <!--leaf--> </c:gapWidth>"), "{name}");
            }
            xml
        });
        assert_eq!(
            owned_plot_parts(&chart_part(&write(&read(&expanded)))),
            expected,
            "{name}"
        );
    }
}

fn find_mut<'a>(value: &'a mut serde_json::Value, key: &str) -> Option<&'a mut serde_json::Value> {
    match value {
        serde_json::Value::Object(map) => {
            if map.contains_key(key) {
                return map.get_mut(key);
            }
            map.values_mut().find_map(|child| find_mut(child, key))
        }
        serde_json::Value::Array(items) => items.iter_mut().find_map(|child| find_mut(child, key)),
        _ => None,
    }
}

/// What a client can send: the projection of `source` with a positioned text
/// field planted in the series axis text, optionally behind a second
/// paragraph that makes the text body differ from the stored one.
fn with_planted_field(source: &Chart, second_paragraph: bool) -> Chart {
    let mut json = serde_json::to_value(source).unwrap();
    let axis = &mut find_mut(&mut json, "series_axis").unwrap()[0];
    let paragraphs = axis["text_properties"]["paragraph"].as_array_mut().unwrap();
    if second_paragraph {
        let copy = paragraphs[0].clone();
        paragraphs.push(copy);
    }
    paragraphs[0]["parts"] = serde_json::json!({
        "properties_present": false,
        "fields": [{
            "before_run": 0,
            "field": {
                "id": "{01234567-89AB-CDEF-0123-456789ABCDEF}",
                "kind": null,
                "run_properties": null,
                "paragraph_properties": null,
                "text": "PLANTED"
            }
        }]
    });
    serde_json::from_value(json).unwrap()
}

fn basis_with(book: &Workbook, edit: impl FnOnce(&mut serde_json::Value)) -> Chart {
    let mut json = serde_json::to_value(chart(book)).unwrap();
    edit(&mut json);
    serde_json::from_value(json).unwrap()
}

#[test]
fn series_axis_text_never_keeps_a_field_the_client_planted() {
    let book = read(&fixture("column3d-series-axis"));
    let mut projection = chart(&book).clone();
    projection.strip_export_parts();
    let no_text = |json: &mut serde_json::Value| {
        find_mut(json, "series_axis").unwrap()[0]["text_properties"] = serde_json::Value::Null;
    };
    let no_axis = |json: &mut serde_json::Value| {
        *find_mut(json, "series_axis").unwrap() = serde_json::json!([]);
    };
    type Edit = fn(&mut serde_json::Value);
    let cases: [(&str, bool, Option<Edit>); 4] = [
        ("same structure", false, None),
        ("incompatible structure", true, None),
        ("no old text", false, Some(no_text)),
        ("unmatched axis", false, Some(no_axis)),
    ];
    for (label, second_paragraph, basis_edit) in cases {
        let basis = match basis_edit {
            Some(edit) => basis_with(&book, edit),
            None => chart(&book).clone(),
        };
        let mut incoming = with_planted_field(&projection, second_paragraph);
        incoming.restore_export_parts(&basis).unwrap();
        let mut sent = book.clone();
        *chart_mut(&mut sent) = incoming;
        let xml = xml_of(&sent);
        assert!(
            !xml.contains("<a:fld"),
            "{label}: a planted field was exported"
        );
        assert!(
            !xml.contains("PLANTED"),
            "{label}: planted text was exported"
        );
        assert!(
            xml.contains("<c:serAx>"),
            "{label}: the axis itself is kept"
        );
    }
}
