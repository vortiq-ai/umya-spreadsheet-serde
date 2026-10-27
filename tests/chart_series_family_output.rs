//! Each plot group writes its series as the series type of its family. The
//! expected child sequences are the ECMA-376 `CT_*Ser` sequences, written out
//! below and restricted to the children the model holds; nothing is captured
//! from the writer.
#![cfg(feature = "serde")]

use std::io::{
    Cursor,
    Read,
    Write,
};

use quick_xml::{
    Reader,
    events::Event,
};
use umya_spreadsheet::{
    Chart,
    ChartType,
    Workbook,
    drawing::{
        charts::{
            AreaChartSeries,
            AreaChartSeriesList,
            ChartGroup,
            Explosion,
            Marker,
            PieChart,
            SeriesFamily,
            SeriesPartConflict,
            SeriesPartConflictReason,
            SeriesPartKind,
            SeriesPartRemoval,
            Symbol,
        },
        spreadsheet::MarkerType,
    },
};

#[derive(Clone, Debug)]
struct Element {
    name:     String,
    attrs:    Vec<(String, String)>,
    text:     String,
    children: Vec<Element>,
}

impl Element {
    fn names(&self) -> Vec<&str> {
        self.children.iter().map(|c| c.name.as_str()).collect()
    }

    fn child(&self, name: &str) -> &Self {
        self.children
            .iter()
            .find(|c| c.name == name)
            .unwrap_or_else(|| panic!("no {name} in {}", self.name))
    }

    fn attr(&self, name: &str) -> &str {
        self.attrs
            .iter()
            .find(|(key, _)| key == name)
            .unwrap_or_else(|| panic!("no {name} on {}", self.name))
            .1
            .as_str()
    }
}

fn tree(xml: &str) -> Element {
    let mut reader = Reader::from_str(xml);
    let mut stack = vec![Element {
        name:     String::new(),
        attrs:    vec![],
        text:     String::new(),
        children: vec![],
    }];
    loop {
        let event = reader.read_event().unwrap();
        let empty = matches!(event, Event::Empty(_));
        match event {
            Event::Start(e) | Event::Empty(e) => {
                let attrs = e
                    .attributes()
                    .map(|a| {
                        let a = a.unwrap();
                        (
                            String::from_utf8(a.key.as_ref().to_vec()).unwrap(),
                            String::from_utf8(a.value.to_vec()).unwrap(),
                        )
                    })
                    .collect();
                let node = Element {
                    name: String::from_utf8(e.name().as_ref().to_vec()).unwrap(),
                    attrs,
                    text: String::new(),
                    children: vec![],
                };
                if empty {
                    stack.last_mut().unwrap().children.push(node);
                } else {
                    stack.push(node);
                }
            }
            Event::End(_) => {
                let node = stack.pop().unwrap();
                stack.last_mut().unwrap().children.push(node);
            }
            Event::Text(text) => {
                stack
                    .last_mut()
                    .unwrap()
                    .text
                    .push_str(&text.decode().unwrap());
            }
            Event::Eof => break,
            _ => (),
        }
    }
    assert_eq!(stack.len(), 1);
    stack.pop().unwrap().children.pop().unwrap()
}

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

fn replace_first(xml: &str, from: &str, to: &str) -> String {
    assert!(xml.contains(from), "{from} not in {xml}");
    xml.replacen(from, to, 1)
}

fn replace_first_series(xml: &str, series: &str) -> String {
    let start = xml.find("<c:ser>").expect("a series in the chart part");
    let end = start + xml[start..].find("</c:ser>").expect("series end") + "</c:ser>".len();
    format!("{}<c:ser>{series}</c:ser>{}", &xml[..start], &xml[end..])
}

fn chart_of(book: &Workbook) -> &Chart {
    &book.sheet(0).unwrap().chart_collection()[0]
}

/// The series elements of the written chart, in document order.
fn written_series(book: &Workbook) -> Vec<Element> {
    let xml = chart_part(&write(book));
    let root = tree(&xml);
    root.child("c:chart")
        .child("c:plotArea")
        .children
        .iter()
        .filter(|c| c.name.ends_with("Chart"))
        .flat_map(|group| group.children.iter())
        .filter(|c| c.name == "c:ser")
        .cloned()
        .collect()
}

fn written_series_xml(book: &Workbook) -> String {
    let xml = chart_part(&write(book));
    let start = xml.find("<c:ser>").unwrap();
    xml[start..start + xml[start..].find("</c:ser>").unwrap() + "</c:ser>".len()].to_string()
}

fn conflicts(book: &Workbook) -> Vec<SeriesPartConflict> {
    chart_of(book)
        .chart_space()
        .chart()
        .plot_area()
        .series_part_conflicts()
}

fn with_chart(bytes: &[u8], chart: Chart) -> Workbook {
    let mut book = read(bytes);
    book.sheet_mut(0).unwrap().chart_collection_mut()[0] = chart;
    book
}

fn through_json(chart: &Chart) -> Chart {
    serde_json::from_str(&serde_json::to_string(chart).unwrap()).unwrap()
}

fn through_bincode(chart: &Chart) -> Chart {
    bincode::deserialize(&bincode::serialize(chart).unwrap()).unwrap()
}

// A series holding every child the model has, in an order that matches no
// series type, so the written order can only come from the writer.
const FULL_SERIES: &str = concat!(
    "<c:smooth val=\"1\"/><c:bubble3D val=\"1\"/>",
    "<c:bubbleSize><c:numRef><c:f>Sheet1!$C$1:$C$3</c:f></c:numRef></c:bubbleSize>",
    "<c:yVal><c:numRef><c:f>Sheet1!$C$1:$C$3</c:f></c:numRef></c:yVal>",
    "<c:xVal><c:numRef><c:f>Sheet1!$B$1:$B$3</c:f></c:numRef></c:xVal>",
    "<c:val><c:numRef><c:f>Sheet1!$B$1:$B$3</c:f></c:numRef></c:val>",
    "<c:cat><c:strRef><c:f>Sheet1!$A$1:$A$3</c:f></c:strRef></c:cat>",
    "<c:dLbls><c:showLegendKey val=\"0\"/><c:showVal val=\"1\"/>",
    "<c:showCatName val=\"0\"/><c:showSerName val=\"0\"/>",
    "<c:showPercent val=\"0\"/><c:showBubbleSize val=\"0\"/></c:dLbls>",
    "<c:marker><c:symbol val=\"diamond\"/><c:size val=\"7\"/></c:marker>",
    "<c:invertIfNegative val=\"0\"/><c:explosion val=\"5\"/>",
    "<c:spPr><a:solidFill><a:srgbClr val=\"4472c4\"/></a:solidFill></c:spPr>",
    "<c:tx><c:strRef><c:f>Sheet1!$B$1</c:f></c:strRef></c:tx>",
    "<c:order val=\"5\"/><c:idx val=\"4\"/>",
);

const LABELS: &str = concat!(
    "<c:dLbls><c:showLegendKey val=\"0\"/><c:showVal val=\"1\"/>",
    "<c:showCatName val=\"0\"/><c:showSerName val=\"0\"/>",
    "<c:showPercent val=\"0\"/><c:showBubbleSize val=\"0\"/></c:dLbls>",
);
const CAT: &str = "<c:cat><c:strRef><c:f>Sheet1!$A$1:$A$3</c:f></c:strRef></c:cat>";
const VAL: &str = "<c:val><c:numRef><c:f>Sheet1!$B$1:$B$3</c:f></c:numRef></c:val>";

fn read_with_series(chart_type: &ChartType, series: &str) -> (Vec<u8>, Workbook) {
    let bytes = with_chart_part(&write(&base_book(chart_type, 1)), |xml| {
        replace_first_series(xml, series)
    });
    let book = read(&bytes);
    (bytes, book)
}

// The ECMA-376 series sequences after `idx, order, tx?, spPr?`, complete.
const BAR_SER: &[&str] = &[
    "invertIfNegative",
    "pictureOptions",
    "dPt",
    "dLbls",
    "trendline",
    "errBars",
    "cat",
    "val",
    "shape",
    "extLst",
];
const LINE_SER: &[&str] = &[
    "marker",
    "dPt",
    "dLbls",
    "trendline",
    "errBars",
    "cat",
    "val",
    "smooth",
    "extLst",
];
const AREA_SER: &[&str] = &[
    "pictureOptions",
    "dPt",
    "dLbls",
    "trendline",
    "errBars",
    "cat",
    "val",
    "extLst",
];
const PIE_SER: &[&str] = &["explosion", "dPt", "dLbls", "cat", "val", "extLst"];
const SCATTER_SER: &[&str] = &[
    "marker",
    "dPt",
    "dLbls",
    "trendline",
    "errBars",
    "xVal",
    "yVal",
    "smooth",
    "extLst",
];
const BUBBLE_SER: &[&str] = &[
    "invertIfNegative",
    "dPt",
    "dLbls",
    "trendline",
    "errBars",
    "xVal",
    "yVal",
    "bubbleSize",
    "bubble3D",
    "extLst",
];
const RADAR_SER: &[&str] = &["marker", "dPt", "dLbls", "cat", "val", "extLst"];

// The children the model holds, with the part kind of those that only some
// series types have, in the order the conflict list reports them.
const MODELED: [(&str, Option<SeriesPartKind>); 15] = [
    ("idx", None),
    ("order", None),
    ("tx", None),
    ("spPr", None),
    ("dLbls", None),
    ("explosion", Some(SeriesPartKind::Explosion)),
    ("invertIfNegative", Some(SeriesPartKind::InvertIfNegative)),
    ("marker", Some(SeriesPartKind::Marker)),
    ("cat", Some(SeriesPartKind::Categories)),
    ("val", Some(SeriesPartKind::Values)),
    ("xVal", Some(SeriesPartKind::XValues)),
    ("yVal", Some(SeriesPartKind::YValues)),
    ("bubbleSize", Some(SeriesPartKind::BubbleSize)),
    ("bubble3D", Some(SeriesPartKind::Bubble3D)),
    ("smooth", Some(SeriesPartKind::Smooth)),
];

/// The 13 supported plot groups: series type name, its ECMA sequence and the
/// family the group belongs to.
fn groups() -> [(
    ChartType,
    &'static str,
    &'static [&'static str],
    SeriesFamily,
); 13] {
    [
        (ChartType::BarChart, "CT_BarSer", BAR_SER, SeriesFamily::Bar),
        (
            ChartType::Bar3DChart,
            "CT_BarSer",
            BAR_SER,
            SeriesFamily::Bar,
        ),
        (
            ChartType::LineChart,
            "CT_LineSer",
            LINE_SER,
            SeriesFamily::Line,
        ),
        (
            ChartType::Line3DChart,
            "CT_LineSer",
            LINE_SER,
            SeriesFamily::Line,
        ),
        (
            ChartType::AreaChart,
            "CT_AreaSer",
            AREA_SER,
            SeriesFamily::Area,
        ),
        (
            ChartType::Area3DChart,
            "CT_AreaSer",
            AREA_SER,
            SeriesFamily::Area,
        ),
        (ChartType::PieChart, "CT_PieSer", PIE_SER, SeriesFamily::Pie),
        (
            ChartType::Pie3DChart,
            "CT_PieSer",
            PIE_SER,
            SeriesFamily::Pie,
        ),
        (
            ChartType::DoughnutChart,
            "CT_PieSer",
            PIE_SER,
            SeriesFamily::Pie,
        ),
        (
            ChartType::OfPieChart,
            "CT_PieSer",
            PIE_SER,
            SeriesFamily::Pie,
        ),
        (
            ChartType::ScatterChart,
            "CT_ScatterSer",
            SCATTER_SER,
            SeriesFamily::Scatter,
        ),
        (
            ChartType::BubbleChart,
            "CT_BubbleSer",
            BUBBLE_SER,
            SeriesFamily::Bubble,
        ),
        (
            ChartType::RadarChart,
            "CT_RadarSer",
            RADAR_SER,
            SeriesFamily::Radar,
        ),
    ]
}

/// `idx, order, tx?, spPr?` followed by the sequence, keeping only what the
/// model holds.
fn expected_names(sequence: &[&str]) -> Vec<String> {
    let mut names: Vec<String> = ["idx", "order", "tx", "spPr"]
        .iter()
        .map(ToString::to_string)
        .collect();
    names.extend(
        sequence
            .iter()
            .filter(|name| MODELED.iter().any(|(modeled, _)| modeled == *name))
            .map(ToString::to_string),
    );
    names
}

/// The family-dependent children a full series holds that the sequence lacks.
fn expected_conflicts(sequence: &[&str], group_index: usize) -> Vec<SeriesPartConflict> {
    MODELED
        .iter()
        .filter_map(|(name, kind)| kind.map(|kind| (name, kind)))
        .filter(|(name, _)| !sequence.contains(name))
        .map(|(_, part)| SeriesPartConflict {
            group_index,
            series_index: 0,
            part,
            occurrence: 0,
            reason: SeriesPartConflictReason::NotAllowed,
        })
        .collect()
}

#[test]
fn public_chart_builders_store_only_family_legal_series_parts() {
    for (chart_type, ..) in groups() {
        let book = base_book(&chart_type, 1);
        let plot = chart_of(&book).chart_space().chart().plot_area();
        assert_eq!(plot.series_part_conflicts(), vec![], "{chart_type:?}");
        match chart_type {
            ChartType::Line3DChart => {
                let chart = plot.line_3d_chart().unwrap();
                assert_eq!(
                    chart
                        .axis_id()
                        .iter()
                        .map(umya_spreadsheet::drawing::charts::AxisId::val)
                        .collect::<Vec<_>>(),
                    [213_468_160, 169_590_080, 110_000_000]
                );
                assert_eq!(
                    plot.category_axis()
                        .iter()
                        .map(|axis| axis.axis_id().val())
                        .collect::<Vec<_>>(),
                    [213_468_160]
                );
                assert_eq!(
                    plot.value_axis()
                        .iter()
                        .map(|axis| axis.axis_id().val())
                        .collect::<Vec<_>>(),
                    [169_590_080]
                );
                assert_eq!(
                    plot.series_axis()
                        .iter()
                        .map(|axis| (axis.axis_id().val(), axis.tick_crossing_axis().val()))
                        .collect::<Vec<_>>(),
                    [(110_000_000, 169_590_080)]
                );
            }
            ChartType::OfPieChart => {
                assert_eq!(plot.of_pie_chart().unwrap().second_pie_size().val(), 75);
            }
            _ => (),
        }
        let reread = read(&write(&book));
        assert_eq!(
            conflicts(&reread),
            vec![],
            "{chart_type:?} after read/write"
        );
    }
}

fn present(series: &AreaChartSeries) -> [bool; 10] {
    [
        series.explosion().is_some(),
        series.invert_if_negative().is_some(),
        series.marker().is_some(),
        series.category_axis_data().is_some(),
        series.values().is_some(),
        series.x_values().is_some(),
        series.y_values().is_some(),
        series.bubble_size().is_some(),
        series.bubble_3d().is_some(),
        series.smooth().is_some(),
    ]
}

#[test]
fn every_plot_group_writes_its_series_in_the_series_type_sequence() {
    for (chart_type, ecma_type, sequence, family) in groups() {
        let (bytes, book) = read_with_series(&chart_type, FULL_SERIES);
        let label = format!("{chart_type:?} as {ecma_type}");

        let group = chart_of(&book)
            .chart_space()
            .chart()
            .plot_area()
            .chart_groups()
            .next()
            .unwrap();
        assert_eq!(group.series_family(), family, "{label}");

        let series = written_series(&book);
        assert_eq!(series.len(), 1, "{label}");
        assert_eq!(
            series[0].names(),
            expected_names(sequence)
                .iter()
                .map(|name| format!("c:{name}"))
                .collect::<Vec<_>>(),
            "{label}"
        );
        // Children that stay keep what was read.
        for (name, expected) in [
            ("explosion", "5"),
            ("invertIfNegative", "0"),
            ("smooth", "1"),
            ("bubble3D", "1"),
            ("idx", "4"),
            ("order", "5"),
        ] {
            if expected_names(sequence).iter().any(|n| n == name) {
                assert_eq!(
                    series[0].child(&format!("c:{name}")).attr("val"),
                    expected,
                    "{label} {name}"
                );
            }
        }

        let expected = expected_conflicts(sequence, 0);
        assert_eq!(conflicts(&book), expected, "{label}");

        // The stored state, with its illegal parts, survives both wires and
        // writes the same series and reports the same conflicts.
        let first = written_series_xml(&book);
        for (wire, restored) in [
            ("json", through_json(chart_of(&book))),
            ("bincode", through_bincode(chart_of(&book))),
        ] {
            let again = with_chart(&bytes, restored);
            assert_eq!(written_series_xml(&again), first, "{label} via {wire}");
            assert_eq!(conflicts(&again), expected, "{label} via {wire}");
        }

        // What was written reads back as exactly itself, over both wires.
        let written = write(&book);
        let reread = read(&written);
        assert_eq!(conflicts(&reread), vec![], "{label} reread");
        assert_eq!(written_series_xml(&reread), first, "{label} reread");
        for (wire, restored) in [
            ("json", through_json(chart_of(&reread))),
            ("bincode", through_bincode(chart_of(&reread))),
        ] {
            let again = with_chart(&written, restored);
            assert_eq!(
                written_series_xml(&again),
                first,
                "{label} reread via {wire}"
            );
        }
    }
}

#[test]
fn pie_series_with_a_marker_and_smoothing_writes_only_pie_children() {
    let series = [
        "<c:idx val=\"2\"/><c:order val=\"3\"/>",
        "<c:marker><c:symbol val=\"circle\"/></c:marker>",
        "<c:smooth val=\"1\"/><c:explosion val=\"12\"/>",
        CAT,
        VAL,
    ]
    .concat();
    let (_, book) = read_with_series(&ChartType::PieChart, &series);
    let written = written_series(&book);
    assert_eq!(
        written[0].names(),
        ["c:idx", "c:order", "c:explosion", "c:cat", "c:val"]
    );
    assert_eq!(written[0].child("c:explosion").attr("val"), "12");
    assert_eq!(
        conflicts(&book),
        [SeriesPartKind::Marker, SeriesPartKind::Smooth].map(|part| SeriesPartConflict {
            group_index: 0,
            series_index: 0,
            part,
            occurrence: 0,
            reason: SeriesPartConflictReason::NotAllowed,
        })
    );
}

#[test]
fn scatter_series_with_categories_and_values_writes_neither() {
    let series = ["<c:idx val=\"0\"/><c:order val=\"0\"/>", CAT, VAL].concat();
    let (_, book) = read_with_series(&ChartType::ScatterChart, &series);
    assert_eq!(written_series(&book)[0].names(), ["c:idx", "c:order"]);
    assert_eq!(
        conflicts(&book),
        [SeriesPartKind::Categories, SeriesPartKind::Values].map(|part| SeriesPartConflict {
            group_index: 0,
            series_index: 0,
            part,
            occurrence: 0,
            reason: SeriesPartConflictReason::NotAllowed,
        })
    );
}

#[test]
fn conflicts_name_the_group_and_series_in_document_order() {
    let mut book = base_book(&ChartType::LineChart, 2);
    {
        let chart = &mut book.sheet_mut(0).unwrap().chart_collection_mut()[0];
        let plot = chart.plot_area_mut();
        let mut exploded = Explosion::default();
        exploded.set_val(9);
        plot.line_chart_mut()
            .unwrap()
            .area_chart_series_list_mut()
            .area_chart_series_mut()[1]
            .set_explosion(exploded);

        let mut circle = Marker::default();
        let mut symbol = Symbol::default();
        symbol.set_val(umya_spreadsheet::drawing::charts::MarkerStyleValues::Circle);
        circle.set_symbol(symbol);
        let mut pie_series = AreaChartSeries::default();
        pie_series.set_marker(circle);
        let mut list = AreaChartSeriesList::default();
        list.add_area_chart_series(pie_series);
        let mut pie = PieChart::default();
        pie.set_area_chart_series_list(list);
        plot.add_chart_group(ChartGroup::PieChart(pie));
    }

    assert_eq!(
        conflicts(&book),
        [
            SeriesPartConflict {
                group_index:  0,
                series_index: 1,
                part:         SeriesPartKind::Explosion,
                occurrence:   0,
                reason:       SeriesPartConflictReason::NotAllowed,
            },
            SeriesPartConflict {
                group_index:  1,
                series_index: 0,
                part:         SeriesPartKind::Marker,
                occurrence:   0,
                reason:       SeriesPartConflictReason::NotAllowed,
            },
        ]
    );
    let written = written_series(&book);
    assert_eq!(written.len(), 3);
    assert_eq!(
        written[0].names(),
        ["c:idx", "c:order", "c:val", "c:smooth"]
    );
    assert_eq!(
        written[1].names(),
        ["c:idx", "c:order", "c:val", "c:smooth"]
    );
    assert_eq!(written[2].names(), ["c:idx", "c:order"]);
}

#[test]
fn manual_layout_writes_target_modes_then_coordinates_in_schema_order() {
    let layout = concat!(
        "<c:layout><c:manualLayout><c:h val=\"0.5\"/><c:w val=\"0.75\"/>",
        "<c:y val=\"0.25\"/><c:x val=\"0.125\"/><c:hMode val=\"factor\"/>",
        "<c:wMode val=\"factor\"/><c:yMode val=\"edge\"/><c:xMode val=\"edge\"/>",
        "<c:layoutTarget val=\"inner\"/></c:manualLayout></c:layout>",
    );
    let bytes = with_chart_part(&write(&base_book(&ChartType::LineChart, 1)), |xml| {
        replace_first(
            xml,
            "<c:plotArea><c:layout/>",
            &format!("<c:plotArea>{layout}"),
        )
    });
    let xml = chart_part(&write(&read(&bytes)));
    let root = tree(&xml);
    let manual = &root
        .child("c:chart")
        .child("c:plotArea")
        .child("c:layout")
        .children[0];
    let written: Vec<(&str, &str)> = manual
        .children
        .iter()
        .map(|c| (c.name.as_str(), c.attr("val")))
        .collect();
    // CT_ManualLayout: layoutTarget, xMode, yMode, wMode, hMode, x, y, w, h.
    assert_eq!(
        written,
        [
            ("c:layoutTarget", "inner"),
            ("c:xMode", "edge"),
            ("c:yMode", "edge"),
            ("c:wMode", "factor"),
            ("c:hMode", "factor"),
            ("c:x", "0.125"),
            ("c:y", "0.25"),
            ("c:w", "0.75"),
            ("c:h", "0.5"),
        ]
    );
}

#[test]
fn paragraph_properties_write_line_spacing_before_the_default_run_properties() {
    let labels = concat!(
        "<c:dLbls><c:txPr><a:bodyPr/><a:lstStyle/><a:p><a:pPr algn=\"ctr\">",
        "<a:defRPr sz=\"1000\"/><a:lnSpc><a:spcPct val=\"90000\"/></a:lnSpc>",
        "</a:pPr><a:endParaRPr lang=\"en-US\"/></a:p></c:txPr>",
        "<c:showLegendKey val=\"0\"/><c:showVal val=\"1\"/><c:showCatName val=\"0\"/>",
        "<c:showSerName val=\"0\"/><c:showPercent val=\"0\"/>",
        "<c:showBubbleSize val=\"0\"/></c:dLbls>",
    );
    let series = ["<c:idx val=\"0\"/><c:order val=\"0\"/>", labels, CAT, VAL].concat();
    let (_, book) = read_with_series(&ChartType::LineChart, &series);
    let written = &written_series(&book)[0];
    let paragraph = &written
        .child("c:dLbls")
        .child("c:txPr")
        .child("a:p")
        .child("a:pPr");
    assert_eq!(paragraph.names(), ["a:lnSpc", "a:defRPr"]);
    assert_eq!(paragraph.attr("algn"), "ctr");
    assert_eq!(
        paragraph.child("a:lnSpc").child("a:spcPct").attr("val"),
        "90000"
    );
    assert_eq!(paragraph.child("a:defRPr").attr("sz"), "1000");
}

fn expected_removed(sequence: &[&str], present_names: &[&str]) -> Vec<SeriesPartKind> {
    MODELED
        .iter()
        .filter_map(|(name, kind)| kind.map(|kind| (name, kind)))
        .filter(|(name, _)| present_names.contains(name) && !sequence.contains(name))
        .map(|(_, kind)| kind)
        .collect()
}

#[test]
fn changing_the_series_type_removes_exactly_the_parts_the_target_lacks() {
    // Series 0 holds every part; series 1 is the builder's line series: values
    // and smoothing.
    let bytes = with_chart_part(&write(&base_book(&ChartType::LineChart, 2)), |xml| {
        replace_first_series(xml, FULL_SERIES)
    });
    let all = [
        "explosion",
        "invertIfNegative",
        "marker",
        "cat",
        "val",
        "xVal",
        "yVal",
        "bubbleSize",
        "bubble3D",
        "smooth",
    ];
    for (target_type, ecma_type, sequence, _) in groups() {
        let mut book = read(&bytes);
        let list = book.sheet_mut(0).unwrap().chart_collection_mut()[0]
            .plot_area_mut()
            .line_chart_mut()
            .unwrap()
            .area_chart_series_list_mut();
        // The fixture really holds every part before the change.
        assert_eq!(present(&list.area_chart_series()[0]), [true; 10]);
        let removed = list.remove_parts_not_allowed_in(SeriesFamily::from(&target_type));

        let mut expected: Vec<SeriesPartRemoval> = expected_removed(sequence, &all)
            .into_iter()
            .map(|part| SeriesPartRemoval {
                series_index: 0,
                part,
                occurrence: 0,
            })
            .collect();
        expected.extend(
            expected_removed(sequence, &["val", "smooth"])
                .into_iter()
                .map(|part| SeriesPartRemoval {
                    series_index: 1,
                    part,
                    occurrence: 0,
                }),
        );
        assert_eq!(removed, expected, "to {target_type:?} ({ecma_type})");

        // A second change finds nothing more, and what the target holds is
        // still there.
        let list = book.sheet_mut(0).unwrap().chart_collection_mut()[0]
            .plot_area_mut()
            .line_chart_mut()
            .unwrap()
            .area_chart_series_list_mut();
        assert_eq!(
            list.remove_parts_not_allowed_in(SeriesFamily::from(&target_type)),
            vec![],
            "to {target_type:?} again"
        );
        let kept_first = present(&list.area_chart_series()[0]);
        for (flag, name) in kept_first.iter().zip(all) {
            assert_eq!(*flag, sequence.contains(&name), "{target_type:?} {name}");
        }
        let kept_second = present(&list.area_chart_series()[1]);
        assert_eq!(
            kept_second,
            [
                false,
                false,
                false,
                false,
                sequence.contains(&"val"),
                false,
                false,
                false,
                false,
                sequence.contains(&"smooth"),
            ],
            "{target_type:?} second series"
        );
    }
}

#[test]
fn line_to_pie_keeps_points_labels_identity_and_data() {
    let series = [
        "<c:idx val=\"3\"/><c:order val=\"4\"/>",
        "<c:tx><c:strRef><c:f>Sheet1!$B$1</c:f></c:strRef></c:tx>",
        "<c:spPr><a:solidFill><a:srgbClr val=\"4472c4\"/></a:solidFill></c:spPr>",
        "<c:marker><c:symbol val=\"diamond\"/><c:size val=\"7\"/></c:marker>",
        LABELS,
        CAT,
        VAL,
        "<c:smooth val=\"0\"/>",
    ]
    .concat();
    let (_, mut book) = read_with_series(&ChartType::LineChart, &series);
    let list = book.sheet_mut(0).unwrap().chart_collection_mut()[0]
        .plot_area_mut()
        .line_chart_mut()
        .unwrap()
        .area_chart_series_list_mut();
    let removed = list.remove_parts_not_allowed_in(SeriesFamily::from(ChartType::PieChart));
    assert_eq!(
        removed,
        [SeriesPartKind::Marker, SeriesPartKind::Smooth].map(|part| SeriesPartRemoval {
            series_index: 0,
            part,
            occurrence: 0,
        })
    );

    // What the pie type keeps is untouched, so the line group still writes it.
    let written = &written_series(&book)[0];
    assert_eq!(
        written.names(),
        [
            "c:idx", "c:order", "c:tx", "c:spPr", "c:dLbls", "c:cat", "c:val"
        ]
    );
    assert_eq!(written.child("c:idx").attr("val"), "3");
    assert_eq!(written.child("c:order").attr("val"), "4");
    assert_eq!(written.child("c:dLbls").child("c:showVal").attr("val"), "1");
    assert_eq!(
        written.child("c:cat").child("c:strRef").child("c:f").text,
        "Sheet1!$A$1:$A$3"
    );
    assert_eq!(
        written.child("c:val").child("c:numRef").child("c:f").text,
        "Sheet1!$B$1:$B$3"
    );
    assert_eq!(conflicts(&book), vec![]);
}

#[test]
fn area_to_line_removes_only_what_area_stored_illegally() {
    let series = [
        "<c:idx val=\"1\"/><c:order val=\"1\"/><c:explosion val=\"4\"/>",
        "<c:invertIfNegative val=\"1\"/><c:smooth val=\"1\"/>",
        LABELS,
        CAT,
        VAL,
    ]
    .concat();
    let (_, mut book) = read_with_series(&ChartType::AreaChart, &series);
    assert_eq!(
        conflicts(&book),
        [
            SeriesPartKind::Explosion,
            SeriesPartKind::InvertIfNegative,
            SeriesPartKind::Smooth,
        ]
        .map(|part| SeriesPartConflict {
            group_index: 0,
            series_index: 0,
            part,
            occurrence: 0,
            reason: SeriesPartConflictReason::NotAllowed,
        })
    );
    let list = book.sheet_mut(0).unwrap().chart_collection_mut()[0]
        .plot_area_mut()
        .area_chart_mut()
        .unwrap()
        .area_chart_series_list_mut();
    let removed = list.remove_parts_not_allowed_in(SeriesFamily::from(ChartType::LineChart));
    assert_eq!(
        removed,
        [SeriesPartKind::Explosion, SeriesPartKind::InvertIfNegative].map(|part| {
            SeriesPartRemoval {
                series_index: 0,
                part,
                occurrence: 0,
            }
        })
    );
    // Smoothing is a line child, so it stays; the rest of the area series too.
    let kept = &list.area_chart_series()[0];
    assert_eq!(
        present(kept),
        [
            false, false, false, true, true, false, false, false, false, true
        ]
    );
    assert!(kept.smooth().unwrap().val());
    assert_eq!(kept.index().val(), 1);
    assert!(kept.data_labels().unwrap().show_value().val());
}
