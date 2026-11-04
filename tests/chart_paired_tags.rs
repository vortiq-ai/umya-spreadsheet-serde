#![cfg(feature = "serde")]
use std::{
    io::Cursor,
    path::{
        Path,
        PathBuf,
    },
};

use umya_spreadsheet::{
    ChartType,
    drawing::charts::{
        Chart,
        GroupingValues,
        OfPieChart,
        SizeRepresentsValues,
        SplitTypeValues,
        XmlDouble,
        XmlUnsignedInt,
    },
};

#[path = "support/chart_xml_fixture.rs"]
pub mod fixture;

fn is_chart_part(name: &str) -> bool {
    let Some(file) = name.strip_prefix("xl/charts/") else {
        return false;
    };
    let stem = file.strip_suffix(".xml").unwrap_or("");
    let digits = stem
        .strip_prefix("chartEx")
        .or_else(|| stem.strip_prefix("chart"))
        .unwrap_or("");
    !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())
}

fn fixtures(dir: &Path, found: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            fixtures(&path, found);
        } else if path.extension().is_some_and(|ext| ext == "xlsx") {
            found.push(path);
        }
    }
}

#[test]
#[ignore = "short manual read timing, run with --ignored --nocapture"]
fn largest_chart_fixture_read_timing() {
    let mut found = Vec::new();
    for root in ["tests/fixtures", "tests/test_files"] {
        fixtures(Path::new(root), &mut found);
    }
    let mut ranked = Vec::new();
    for path in found {
        let package = std::fs::read(&path).unwrap();
        let mut archive = zip::ZipArchive::new(Cursor::new(&package)).unwrap();
        let chart_bytes: u64 = (0..archive.len())
            .map(|index| {
                let entry = archive.by_index(index).unwrap();
                if is_chart_part(&entry.name().unwrap()) {
                    entry.size()
                } else {
                    0
                }
            })
            .sum();
        if chart_bytes > 0 {
            ranked.push((chart_bytes, path));
        }
    }
    ranked.sort_by(|a, b| b.cmp(a));
    for (chart_bytes, path) in ranked.into_iter().take(3) {
        let package = std::fs::read(&path).unwrap();
        let start = std::time::Instant::now();
        let mut charts = 0;
        for _ in 0..3 {
            let mut book =
                umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(package.clone()), true)
                    .unwrap();
            book.read_sheet_collection();
            charts += book
                .sheet_collection()
                .iter()
                .map(|sheet| sheet.chart_collection().len())
                .sum::<usize>();
        }
        assert!(charts > 0);
        eprintln!(
            "{}: chart_bytes={chart_bytes}, reads=3, elapsed_ms={:.3}",
            path.display(),
            start.elapsed().as_secs_f64() * 1000.0
        );
    }
}

const BAR_CHART: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><c:chartSpace \
     xmlns:c=\"http://schemas.openxmlformats.org/drawingml/2006/chart\"><c:chart><c:plotArea><c:\
     layout/><c:barChart><c:barDir val=\"col\"></c:barDir><c:grouping \
     val=\"percentStacked\"></c:grouping><c:varyColors val=\"1\"></c:varyColors><c:gapWidth \
     val=\"37\"></c:gapWidth><c:overlap val=\"100\"></c:overlap><c:axId \
     val=\"111\"></c:axId><c:axId val=\"222\"></c:axId></c:barChart><c:catAx><c:axId \
     val=\"111\"></c:axId><c:scaling><c:orientation \
     val=\"maxMin\"></c:orientation></c:scaling><c:delete val=\"1\"></c:delete><c:axPos \
     val=\"t\"></c:axPos><c:majorTickMark val=\"in\"></c:majorTickMark><c:crossAx \
     val=\"222\"></c:crossAx></c:catAx><c:valAx><c:axId \
     val=\"222\"></c:axId><c:scaling><c:orientation \
     val=\"minMax\"></c:orientation></c:scaling><c:delete val=\"0\"></c:delete><c:axPos \
     val=\"l\"></c:axPos><c:crossAx val=\"111\"></c:crossAx></c:valAx></c:plotArea></c:chart></c:\
     chartSpace>";

fn package_with_chart(xml: &str) -> Vec<u8> {
    let template = fixture::write_workbook(&fixture::imported_workbook(BAR_CHART));
    fixture::replace_chart_part(&template, xml)
}

fn read_package(package: &[u8]) -> umya_spreadsheet::Workbook {
    let mut book =
        umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(package.to_vec()), true).unwrap();
    book.try_read_sheet_collection().unwrap();
    book
}

fn write_package(book: &umya_spreadsheet::Workbook, light: bool) -> Vec<u8> {
    let mut output = Cursor::new(Vec::new());
    if light {
        umya_spreadsheet::writer::xlsx::write_writer_light(book, &mut output).unwrap();
    } else {
        umya_spreadsheet::writer::xlsx::write_writer(book, &mut output).unwrap();
    }
    output.into_inner()
}

fn assert_full_and_light_roundtrips<F>(book: &umya_spreadsheet::Workbook, check: F)
where
    F: Fn(&umya_spreadsheet::Workbook),
{
    check(book);
    for light in [false, true] {
        check(&read_package(&write_package(book, light)));
    }
}

#[test]
fn a_split_group_and_axis_leaf_keeps_its_value() {
    let mut book = fixture::imported_workbook(BAR_CHART);
    book.read_sheet_collection();
    let xml = fixture::chart_xml(&book);
    let chart = &book.sheet(0).unwrap().chart_collection()[0];
    let group = chart.chart_space().chart().plot_area().bar_chart().unwrap();
    assert!(matches!(
        group.grouping().val(),
        GroupingValues::PercentStacked
    ));
    for expected in [
        "<c:grouping val=\"percentStacked\"/>",
        "<c:gapWidth val=\"37\"/>",
        "<c:overlap val=\"100\"/>",
        "<c:orientation val=\"maxMin\"/>",
        "<c:delete val=\"1\"/>",
        "<c:axPos val=\"t\"/>",
        "<c:majorTickMark val=\"in\"/>",
    ] {
        assert!(xml.contains(expected), "missing {expected} in {xml}");
    }

    book.sheet_mut(0).unwrap().chart_collection_mut()[0]
        .chart_space_mut()
        .chart_mut()
        .plot_area_mut()
        .bar_chart_mut()
        .unwrap()
        .gap_width_mut()
        .set_val(42);
    let output = fixture::write_workbook(&book);
    let reread = umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(output), true).unwrap();
    let group = reread.sheet(0).unwrap().chart_collection()[0]
        .chart_space()
        .chart()
        .plot_area()
        .bar_chart()
        .unwrap();
    assert_eq!(group.gap_width().val(), 42);
    assert!(matches!(
        group.grouping().val(),
        GroupingValues::PercentStacked
    ));
}

#[test]
fn empty_optional_chart_owners_survive_eager_lazy_and_both_writers() {
    fn assert_owners(book: &umya_spreadsheet::Workbook) {
        let chart = book.sheet(0).unwrap().chart_collection()[0]
            .chart_space()
            .chart();
        assert!(chart.title().is_some(), "empty c:title was lost");
        assert!(chart.view_3d().is_some(), "empty c:view3D was lost");
        assert!(chart.floor().is_some(), "empty c:floor was lost");
        assert!(chart.side_wall().is_some(), "empty c:sideWall was lost");
        assert!(chart.back_wall().is_some(), "empty c:backWall was lost");
        assert!(chart.legend_present(), "empty c:legend was lost");
    }

    // Self-closing owners reach the readers as Empty events, paired ones as
    // Start events; both must keep the owner.
    for paired in [false, true] {
        let owner = |name: &str| {
            if paired {
                format!("<c:{name}></c:{name}>")
            } else {
                format!("<c:{name}/>")
            }
        };
        let before_plot_area: String = ["title", "view3D", "floor", "sideWall", "backWall"]
            .into_iter()
            .map(owner)
            .collect();
        let xml = BAR_CHART
            .replace(
                "<c:chart><c:plotArea>",
                &format!("<c:chart>{before_plot_area}<c:plotArea>"),
            )
            .replace(
                "</c:plotArea></c:chart>",
                &format!("</c:plotArea>{}</c:chart>", owner("legend")),
            );
        let package = package_with_chart(&xml);

        let eager = read_package(&package);
        assert_full_and_light_roundtrips(&eager, assert_owners);

        let path = std::env::temp_dir().join(format!(
            "umya-empty-chart-owners-{}-{paired}.xlsx",
            std::process::id()
        ));
        std::fs::write(&path, package).unwrap();
        let mut lazy = umya_spreadsheet::reader::xlsx::lazy_read(&path).unwrap();
        lazy.try_read_sheet_collection().unwrap();
        assert_full_and_light_roundtrips(&lazy, assert_owners);
        std::fs::remove_file(path).unwrap();
    }
}

#[test]
fn empty_chart_groups_keep_presence_kind_and_order() {
    // The three pie families permit no children in the schema. Also exercise
    // every other group accepted by PlotArea, so normalization cannot erase it.
    let kinds = [
        ("pieChart", ChartType::PieChart),
        ("pie3DChart", ChartType::Pie3DChart),
        ("doughnutChart", ChartType::DoughnutChart),
        ("lineChart", ChartType::LineChart),
        ("line3DChart", ChartType::Line3DChart),
        ("barChart", ChartType::BarChart),
        ("bar3DChart", ChartType::Bar3DChart),
        ("areaChart", ChartType::AreaChart),
        ("area3DChart", ChartType::Area3DChart),
        ("scatterChart", ChartType::ScatterChart),
        ("radarChart", ChartType::RadarChart),
        ("bubbleChart", ChartType::BubbleChart),
        ("ofPieChart", ChartType::OfPieChart),
        ("stockChart", ChartType::StockChart),
        ("surfaceChart", ChartType::SurfaceChart),
        ("surface3DChart", ChartType::Surface3DChart),
    ];
    for (name, kind) in kinds {
        for repeated in [false, true] {
            let groups = if repeated {
                format!(
                    "<c:{name}></c:{name}><c:pieChart><c:ser><c:idx val=\"7\"/><c:order \
                     val=\"0\"/></c:ser></c:pieChart><c:{name}></c:{name}>"
                )
            } else {
                format!("<c:{name}></c:{name}>")
            };
            let xml = format!(
                "<c:chartSpace xmlns:c=\"http://schemas.openxmlformats.org/drawingml/2006/chart\">\
                 <c:chart><c:plotArea>{groups}</c:plotArea></c:chart></c:chartSpace>"
            );
            let package = package_with_chart(&xml);
            for eager in [true, false] {
                let mut book = umya_spreadsheet::reader::xlsx::read_reader(
                    Cursor::new(package.clone()),
                    eager,
                )
                .unwrap();
                book.try_read_sheet_collection().unwrap();
                let expected_xml = fixture::chart_xml(&book);
                assert_full_and_light_roundtrips(&book, |book| {
                    let charts = book.sheet(0).unwrap().chart_collection();
                    assert_eq!(
                        charts.len(),
                        1,
                        "{name}, repeated={repeated}, eager={eager}"
                    );
                    let groups: Vec<_> = charts[0]
                        .chart_space()
                        .chart()
                        .plot_area()
                        .chart_groups()
                        .collect();
                    let expected = if repeated {
                        vec![kind, ChartType::PieChart, kind]
                    } else {
                        vec![kind]
                    };
                    assert_eq!(
                        groups.iter().map(|group| group.kind()).collect::<Vec<_>>(),
                        expected
                    );
                    assert!(groups[0].series().area_chart_series().is_empty());
                    if repeated {
                        let series = groups[1].series().area_chart_series();
                        assert_eq!(series.len(), 1);
                        assert_eq!(series[0].index().val(), 7);
                        assert!(groups[2].series().area_chart_series().is_empty());
                    }
                    assert_eq!(fixture::chart_xml(book), expected_xml);
                });
            }
        }
    }
}

fn chart_part(package: &[u8]) -> String {
    use std::io::Read;
    let mut archive = zip::ZipArchive::new(Cursor::new(package)).unwrap();
    let mut xml = String::new();
    archive
        .by_name("xl/charts/chart1.xml")
        .unwrap()
        .read_to_string(&mut xml)
        .unwrap();
    xml
}

fn paired_setting_leaves(xml: &str) -> String {
    use quick_xml::{
        Reader,
        Writer,
        events::Event,
    };
    let mut reader = Reader::from_str(xml);
    let mut writer = Writer::new(Vec::new());
    loop {
        match reader.read_event().unwrap() {
            Event::Eof => break,
            Event::Empty(element)
                if matches!(
                    element.local_name().as_ref(),
                    b"gapDepth"
                        | b"rotX"
                        | b"rotY"
                        | b"rAngAx"
                        | b"hPercent"
                        | b"depthPercent"
                        | b"splitType"
                        | b"splitPos"
                        | b"secondPieSize"
                        | b"secondPiePt"
                        | b"bubbleScale"
                        | b"showNegBubbles"
                        | b"sizeRepresents"
                ) =>
            {
                let end = element.to_end().into_owned();
                writer.write_event(Event::Start(element)).unwrap();
                writer.write_event(Event::End(end)).unwrap();
            }
            event => writer.write_event(event).unwrap(),
        }
    }
    String::from_utf8(writer.into_inner()).unwrap()
}

fn emit_settings_package(name: &str, bytes: &[u8]) {
    if let Some(target) = std::env::var_os("CARGO_TARGET_DIR") {
        let directory = PathBuf::from(target).join("chart-paired-settings");
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join(format!("{name}.xlsx")), bytes).unwrap();
    }
}

fn assert_setting_values(case: &str, book: &umya_spreadsheet::Workbook) {
    let charts = book.sheet(0).unwrap().chart_collection();
    assert_eq!(charts.len(), 1, "{case}: chart count");
    let chart = charts[0].chart_space().chart();
    let plot = chart.plot_area();
    assert_eq!(plot.chart_groups().count(), 1);
    match case {
        "bar3d" => assert_eq!(plot.bar_3d_chart().unwrap().gap_depth().unwrap().val(), 275),
        "line3d" => assert_eq!(
            plot.line_3d_chart().unwrap().gap_depth().unwrap().val(),
            200
        ),
        "view3d" => {
            let view = chart.view_3d().unwrap();
            assert_eq!(view.rotate_x().unwrap().val(), 15);
            assert_eq!(view.rotate_y().unwrap().val(), 20);
            assert!(!view.right_angle_axes().unwrap().val());
            assert_eq!(view.height_percent().unwrap().val(), 165);
            assert_eq!(view.depth_percent().unwrap().val(), 250);
        }
        "ofpie" => {
            let pie = plot.of_pie_chart().unwrap();
            assert_eq!(*pie.split_type().unwrap().val(), SplitTypeValues::Percent);
            assert_eq!(pie.split_position().unwrap().value(), Some(3.25));
            assert_eq!(pie.second_pie_size().val(), 62);
            assert!(pie.custom_split().is_none());
        }
        "custom-ofpie" => {
            let pie = plot.of_pie_chart().unwrap();
            assert_eq!(*pie.split_type().unwrap().val(), SplitTypeValues::Custom);
            assert_eq!(pie.second_pie_size().val(), 62);
            assert_eq!(second_pie_points(pie), [1, 3]);
        }
        "bubble" => {
            let bubble = plot.bubble_chart().unwrap();
            assert_eq!(bubble.bubble_scale().val(), 177);
            assert!(bubble.show_negative_bubbles().val());
            assert_eq!(
                *bubble.size_represents().unwrap().val(),
                SizeRepresentsValues::Width
            );
        }
        _ => panic!("unknown settings case {case}"),
    }
}

fn second_pie_points(pie: &OfPieChart) -> Vec<u32> {
    pie.custom_split()
        .unwrap()
        .iter()
        .map(|point| point.val().unwrap())
        .collect()
}

fn assert_setting_order(case: &str, xml: &str) {
    let names: &[&str] = match case {
        "bar3d" => &["gapWidth", "gapDepth", "axId"],
        "line3d" => &["gapDepth", "axId"],
        "view3d" => &["rotX", "hPercent", "rotY", "depthPercent", "rAngAx"],
        "ofpie" => &["splitType", "splitPos", "secondPieSize"],
        "custom-ofpie" => &["splitType", "custSplit", "secondPieSize"],
        "bubble" => &["bubbleScale", "showNegBubbles", "sizeRepresents", "axId"],
        _ => unreachable!(),
    };
    let positions: Vec<_> = names
        .iter()
        .map(|name| xml.find(&format!("<c:{name}")).unwrap())
        .collect();
    assert!(
        positions.windows(2).all(|pair| pair[0] < pair[1]),
        "{case}: schema order {names:?}"
    );
}

#[test]
fn paired_owner_settings_import_and_reread_non_default_values() {
    // Excel COM authored these files; expected constants come from their XML
    // and the documented author script, independently of the library writer.
    for case in [
        "bar3d",
        "line3d",
        "view3d",
        "ofpie",
        "custom-ofpie",
        "bubble",
    ] {
        let source =
            std::fs::read(format!("tests/fixtures/chart_paired_settings/{case}.xlsx")).unwrap();
        let original_xml = chart_part(&source);
        for paired in [false, true] {
            let form = if paired { "paired" } else { "empty" };
            let package = if paired {
                fixture::replace_chart_part(&source, &paired_setting_leaves(&original_xml))
            } else {
                source.clone()
            };
            emit_settings_package(&format!("{case}-{form}-input"), &package);
            let path = std::env::temp_dir().join(format!(
                "umya-settings-{}-{case}-{form}.xlsx",
                std::process::id()
            ));
            std::fs::write(&path, &package).unwrap();
            let books = [
                read_package(&package),
                umya_spreadsheet::reader::xlsx::lazy_read(&path).unwrap(),
            ];
            for (index, mut book) in books.into_iter().enumerate() {
                book.try_read_sheet_collection().unwrap();
                assert_setting_values(case, &book);
                let mode = if index == 0 { "eager" } else { "lazy" };
                for light in [false, true] {
                    let output = write_package(&book, light);
                    let writer = if light { "light" } else { "full" };
                    emit_settings_package(&format!("{case}-{form}-{mode}-{writer}"), &output);
                    assert_setting_order(case, &chart_part(&output));
                    let reread = read_package(&output);
                    assert_setting_values(case, &reread);
                }
            }
            std::fs::remove_file(path).unwrap();
        }
    }
}

fn settings_chart(book: &mut umya_spreadsheet::Workbook) -> &mut Chart {
    book.sheet_mut(0).unwrap().chart_collection_mut()[0]
        .chart_space_mut()
        .chart_mut()
}

fn edit_setting(case: &str, chart: &mut Chart) {
    match case {
        "bar3d" => {
            let group = chart.plot_area_mut().bar_3d_chart_mut().unwrap();
            group.gap_depth_mut().unwrap().set_val(125);
        }
        "line3d" => {
            let group = chart.plot_area_mut().line_3d_chart_mut().unwrap();
            group.gap_depth_mut().unwrap().set_val(225);
        }
        "view3d" => {
            let view = chart.view_3d_mut().unwrap();
            view.height_percent_mut().unwrap().set_val(120);
            view.depth_percent_mut().unwrap().set_val(300);
        }
        "ofpie" => {
            let pie = chart.plot_area_mut().of_pie_chart_mut().unwrap();
            pie.split_type_mut()
                .unwrap()
                .set_val(SplitTypeValues::Value);
            let mut position = XmlDouble::default();
            position.set_value(5.5);
            pie.set_split_position(position);
        }
        "custom-ofpie" => {
            let pie = chart.plot_area_mut().of_pie_chart_mut().unwrap();
            *pie.custom_split_mut().unwrap() = [0, 2]
                .map(|index| {
                    let mut point = XmlUnsignedInt::default();
                    point.set_val(index);
                    point
                })
                .to_vec();
        }
        "bubble" => {
            let group = chart.plot_area_mut().bubble_chart_mut().unwrap();
            group
                .size_represents_mut()
                .unwrap()
                .set_val(SizeRepresentsValues::Area);
        }
        _ => unreachable!(),
    }
}

/// The elements the edit writes, spelled as the schema serializes the values.
fn edited_elements(case: &str) -> &'static [&'static str] {
    match case {
        "bar3d" => &["<c:gapDepth val=\"125\"/>"],
        "line3d" => &["<c:gapDepth val=\"225\"/>"],
        "view3d" => &["<c:hPercent val=\"120\"/>", "<c:depthPercent val=\"300\"/>"],
        "ofpie" => &["<c:splitType val=\"val\"/>", "<c:splitPos val=\"5.5\"/>"],
        "custom-ofpie" => {
            &["<c:custSplit><c:secondPiePt val=\"0\"/><c:secondPiePt val=\"2\"/></c:custSplit>"]
        }
        "bubble" => &["<c:sizeRepresents val=\"area\"/>"],
        _ => unreachable!(),
    }
}

fn assert_edited_setting(case: &str, chart: &Chart) {
    let plot = chart.plot_area();
    match case {
        "bar3d" => assert_eq!(plot.bar_3d_chart().unwrap().gap_depth().unwrap().val(), 125),
        "line3d" => assert_eq!(
            plot.line_3d_chart().unwrap().gap_depth().unwrap().val(),
            225
        ),
        "view3d" => {
            let view = chart.view_3d().unwrap();
            assert_eq!(view.height_percent().unwrap().val(), 120);
            assert_eq!(view.depth_percent().unwrap().val(), 300);
        }
        "ofpie" => {
            let pie = plot.of_pie_chart().unwrap();
            assert_eq!(*pie.split_type().unwrap().val(), SplitTypeValues::Value);
            assert_eq!(pie.split_position().unwrap().value(), Some(5.5));
        }
        "custom-ofpie" => assert_eq!(second_pie_points(plot.of_pie_chart().unwrap()), [0, 2]),
        "bubble" => assert_eq!(
            *plot
                .bubble_chart()
                .unwrap()
                .size_represents()
                .unwrap()
                .val(),
            SizeRepresentsValues::Area
        ),
        _ => unreachable!(),
    }
}

/// Removes the edited settings and returns their element names.
fn remove_setting(case: &str, chart: &mut Chart) -> &'static [&'static str] {
    match case {
        "bar3d" => {
            let group = chart.plot_area_mut().bar_3d_chart_mut().unwrap();
            group.remove_gap_depth();
            &["gapDepth"]
        }
        "line3d" => {
            let group = chart.plot_area_mut().line_3d_chart_mut().unwrap();
            group.remove_gap_depth();
            &["gapDepth"]
        }
        "view3d" => {
            let view = chart.view_3d_mut().unwrap();
            view.remove_height_percent().remove_depth_percent();
            &["hPercent", "depthPercent"]
        }
        "ofpie" => {
            let pie = chart.plot_area_mut().of_pie_chart_mut().unwrap();
            pie.remove_split_type().remove_split_position();
            &["splitType", "splitPos"]
        }
        "custom-ofpie" => {
            let pie = chart.plot_area_mut().of_pie_chart_mut().unwrap();
            pie.remove_custom_split();
            &["custSplit"]
        }
        "bubble" => {
            let group = chart.plot_area_mut().bubble_chart_mut().unwrap();
            group.remove_size_represents();
            &["sizeRepresents"]
        }
        _ => unreachable!(),
    }
}

fn assert_removed_setting(case: &str, chart: &Chart) {
    let plot = chart.plot_area();
    match case {
        "bar3d" => assert!(plot.bar_3d_chart().unwrap().gap_depth().is_none()),
        "line3d" => assert!(plot.line_3d_chart().unwrap().gap_depth().is_none()),
        "view3d" => {
            let view = chart.view_3d().unwrap();
            assert!(view.height_percent().is_none());
            assert!(view.depth_percent().is_none());
        }
        "ofpie" => {
            let pie = plot.of_pie_chart().unwrap();
            assert!(pie.split_type().is_none());
            assert!(pie.split_position().is_none());
        }
        "custom-ofpie" => assert!(plot.of_pie_chart().unwrap().custom_split().is_none()),
        "bubble" => assert!(plot.bubble_chart().unwrap().size_represents().is_none()),
        _ => unreachable!(),
    }
}

#[test]
fn settings_read_from_paired_leaves_can_be_edited_and_removed() {
    // Starts from the paired form, so the edited model is the one the
    // normalized reader built. The outputs feed
    // chart_paired_settings_excel.ps1.
    for case in [
        "bar3d",
        "line3d",
        "view3d",
        "ofpie",
        "custom-ofpie",
        "bubble",
    ] {
        let source =
            std::fs::read(format!("tests/fixtures/chart_paired_settings/{case}.xlsx")).unwrap();
        let paired =
            fixture::replace_chart_part(&source, &paired_setting_leaves(&chart_part(&source)));
        let mut book = read_package(&paired);
        edit_setting(case, settings_chart(&mut book));
        for light in [false, true] {
            let writer = if light { "light" } else { "full" };
            let edited = write_package(&book, light);
            emit_settings_package(&format!("{case}-edited-{writer}"), &edited);
            let xml = chart_part(&edited);
            for element in edited_elements(case) {
                assert!(xml.contains(element), "{case}: missing {element} in {xml}");
            }
            let mut reread = read_package(&edited);
            assert_edited_setting(case, settings_chart(&mut reread));

            let names = remove_setting(case, settings_chart(&mut reread));
            let removed = write_package(&reread, light);
            emit_settings_package(&format!("{case}-removed-{writer}"), &removed);
            let xml = chart_part(&removed);
            for name in names {
                assert!(
                    !xml.contains(&format!("<c:{name}")),
                    "{case}: {name} still written"
                );
            }
            assert_removed_setting(case, settings_chart(&mut read_package(&removed)));
        }
    }
}

#[test]
fn second_pie_points_with_inner_content_keep_their_indexes() {
    // A comment (or blanks) inside a leaf keeps its tags apart, so the
    // normalization leaves it to the owner's Start arm.
    let source = std::fs::read("tests/fixtures/chart_paired_settings/custom-ofpie.xlsx").unwrap();
    let mut xml = chart_part(&source);
    for (from, to) in [
        (
            "<c:secondPiePt val=\"1\"/>",
            "<c:secondPiePt val=\"1\"><!--point--></c:secondPiePt>",
        ),
        (
            "<c:secondPiePt val=\"3\"/>",
            "<c:secondPiePt val=\"3\"> </c:secondPiePt>",
        ),
    ] {
        assert!(xml.contains(from), "fixture lacks {from}");
        xml = xml.replacen(from, to, 1);
    }
    let package = fixture::replace_chart_part(&source, &xml);
    let path = std::env::temp_dir().join(format!(
        "umya-second-pie-points-{}.xlsx",
        std::process::id()
    ));
    std::fs::write(&path, &package).unwrap();
    let mut lazy = umya_spreadsheet::reader::xlsx::lazy_read(&path).unwrap();
    lazy.try_read_sheet_collection().unwrap();
    for book in [read_package(&package), lazy] {
        assert_full_and_light_roundtrips(&book, |book| {
            let chart = &book.sheet(0).unwrap().chart_collection()[0];
            let pie = chart.chart_space().chart().plot_area().of_pie_chart();
            assert_eq!(second_pie_points(pie.unwrap()), [1, 3]);
        });
    }
    std::fs::remove_file(path).unwrap();
}

#[test]
fn empty_text_and_containers_import_like_their_paired_forms() {
    let empty = BAR_CHART
        .replace(
            "<c:chartSpace ",
            "<c:chartSpace xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" ",
        )
        .replace(
            "<c:chart>",
            concat!(
                "<c:chart><c:title><c:tx><c:rich><a:bodyPr/><a:lstStyle/>",
                "<a:p/><a:p><a:r><a:t/></a:r><a:r><a:t>sentinel</a:t></a:r></a:p>",
                "</c:rich></c:tx><c:layout/></c:title>"
            ),
        )
        .replace(
            "<c:gapWidth",
            concat!(
                "<c:ser><c:idx val=\"0\"/><c:order val=\"0\"/><c:tx><c:v/></c:tx>",
                "<c:cat><c:strRef><c:f/><c:strCache><c:ptCount \
                 val=\"0\"/></c:strCache></c:strRef></c:cat>",
                "<c:val><c:numLit><c:formatCode/><c:ptCount \
                 val=\"0\"/></c:numLit></c:val></c:ser><c:gapWidth"
            ),
        );
    let mut paired = empty.clone();
    for name in [
        "c:f",
        "c:v",
        "c:formatCode",
        "c:layout",
        "a:bodyPr",
        "a:lstStyle",
        "a:p",
        "a:t",
    ] {
        paired = paired.replace(&format!("<{name}/>"), &format!("<{name}></{name}>"));
    }
    let template = fixture::write_workbook(&fixture::imported_workbook(BAR_CHART));
    let input = fixture::replace_chart_part(&template, &empty);
    let split = fixture::replace_chart_part(&template, &paired);
    for eager in [true, false] {
        let mut original =
            umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(input.clone()), eager).unwrap();
        let mut split =
            umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(split.clone()), eager).unwrap();
        original.read_sheet_collection();
        split.read_sheet_collection();
        let expected = fixture::chart_xml(&original);
        assert!(expected.contains("sentinel"));
        assert_eq!(expected, fixture::chart_xml(&split));
        for book in [&original, &split] {
            let mut output = Cursor::new(Vec::new());
            umya_spreadsheet::writer::xlsx::write_writer_light(book, &mut output).unwrap();
            let reread =
                umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(output.into_inner()), true)
                    .unwrap();
            assert_eq!(expected, fixture::chart_xml(&reread));
        }
    }
}

#[test]
fn empty_cached_values_keep_a_present_empty_name_in_both_xml_forms() {
    fn assert_name(book: &umya_spreadsheet::Workbook) {
        let reference = book.sheet(0).unwrap().chart_collection()[0]
            .chart_space()
            .chart()
            .plot_area()
            .chart_groups()
            .next()
            .unwrap()
            .series()
            .area_chart_series()[0]
            .chart_text()
            .unwrap()
            .string_reference()
            .unwrap();
        assert_eq!(reference.cached_value(), Some(""));
    }
    let template = fixture::write_workbook(&fixture::imported_workbook(BAR_CHART));
    for (index, value) in ["<c:v/>", "<c:v></c:v>"].into_iter().enumerate() {
        let series = format!(
            concat!(
                "<c:ser><c:idx val=\"0\"/><c:order val=\"0\"/><c:tx><c:strRef>",
                "<c:f>Sheet1!$A$1</c:f><c:strCache><c:ptCount val=\"1\"/>",
                "<c:pt idx=\"0\">{value}</c:pt></c:strCache></c:strRef></c:tx></c:ser>"
            ),
            value = value
        );
        let xml = BAR_CHART.replace("<c:gapWidth", &format!("{series}<c:gapWidth"));
        let input = fixture::replace_chart_part(&template, &xml);
        let path = std::env::temp_dir().join(format!(
            "umya-empty-cache-{}-{index}.xlsx",
            std::process::id()
        ));
        std::fs::write(&path, &input).unwrap();
        let books = [
            umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(input.clone()), true).unwrap(),
            umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(input), false).unwrap(),
            umya_spreadsheet::reader::xlsx::lazy_read(&path).unwrap(),
        ];
        for mut book in books {
            book.try_read_sheet_collection().unwrap();
            assert_name(&book);
            let mut full = Cursor::new(Vec::new());
            umya_spreadsheet::writer::xlsx::write_writer(&book, &mut full).unwrap();
            let mut light = Cursor::new(Vec::new());
            umya_spreadsheet::writer::xlsx::write_writer_light(&book, &mut light).unwrap();
            for output in [full.into_inner(), light.into_inner()] {
                let reread =
                    umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(output), true).unwrap();
                assert_name(&reread);
            }
        }
        std::fs::remove_file(path).unwrap();
    }
}
