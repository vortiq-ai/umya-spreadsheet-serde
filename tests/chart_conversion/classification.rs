//! What the operation reads each measured chart as, against what Excel
//! reopened it as.
use std::collections::{
    BTreeMap,
    BTreeSet,
};

use umya_spreadsheet::{
    ChartType,
    drawing::charts::{
        AreaChart,
        Chart,
        ChartGroup,
    },
    helper::chart_conversion::{
        ChartConversionRefusal,
        ChartSubtype,
    },
};

use super::{
    conversion::{
        Reading,
        assert_unchanged,
        chart,
        chart_part,
        convert,
        find_part,
        inserted_after,
        observations_by_part,
        open,
        read,
        replaced,
        replaced_all,
        replaced_last,
        subtype,
        without_between,
    },
    measured::{
        measured,
        read_part,
    },
    workbook::workbook_with,
    xml::{
        CHART,
        parse,
    },
};

/// The chart types no measured subtype is held by. The measurement reached all
/// sixteen.
const UNMEASURED: [ChartType; 0] = [];

const CHART_TYPES: [ChartType; 16] = [
    ChartType::LineChart,
    ChartType::Line3DChart,
    ChartType::PieChart,
    ChartType::Pie3DChart,
    ChartType::DoughnutChart,
    ChartType::ScatterChart,
    ChartType::BarChart,
    ChartType::Bar3DChart,
    ChartType::RadarChart,
    ChartType::BubbleChart,
    ChartType::AreaChart,
    ChartType::Area3DChart,
    ChartType::OfPieChart,
    ChartType::StockChart,
    ChartType::SurfaceChart,
    ChartType::Surface3DChart,
];

/// The element a plot group of this type is written as.
fn plot_group_element(kind: ChartType) -> &'static str {
    match kind {
        ChartType::LineChart => "lineChart",
        ChartType::Line3DChart => "line3DChart",
        ChartType::PieChart => "pieChart",
        ChartType::Pie3DChart => "pie3DChart",
        ChartType::DoughnutChart => "doughnutChart",
        ChartType::ScatterChart => "scatterChart",
        ChartType::BarChart => "barChart",
        ChartType::Bar3DChart => "bar3DChart",
        ChartType::RadarChart => "radarChart",
        ChartType::BubbleChart => "bubbleChart",
        ChartType::AreaChart => "areaChart",
        ChartType::Area3DChart => "area3DChart",
        ChartType::OfPieChart => "ofPieChart",
        ChartType::StockChart => "stockChart",
        ChartType::SurfaceChart => "surfaceChart",
        ChartType::Surface3DChart => "surface3DChart",
    }
}

/// The plot groups of a part, in document order.
fn plot_groups(part: &str) -> Vec<String> {
    let space = parse(part);
    let plot_area = space
        .c("chart")
        .and_then(|chart| chart.c("plotArea"))
        .expect("a part has a plot area");
    plot_area
        .children
        .iter()
        .filter(|child| child.namespace == CHART && child.name.ends_with("Chart"))
        .map(|child| child.name.clone())
        .collect()
}

/// 763 native parts, 10 parts of the four combination layouts, 10 combination
/// probes and 3 scatter parts the measurement flagged invalid.
const CLASSES: [(&str, usize); 4] = [
    ("native", 763),
    ("combo", 10),
    ("probe", 10),
    ("invalid_scatter", 3),
];

#[test]
fn every_before_part_classifies_as_its_subtype() {
    let mut classes: BTreeMap<&str, usize> = BTreeMap::new();
    let mut native: BTreeSet<&str> = BTreeSet::new();
    let mut wrong = Vec::new();
    for part in &measured().parts {
        *classes.entry(part.class.as_str()).or_default() += 1;
        let expected = match (part.class.as_str(), &part.subtype, &part.layout) {
            ("native", Some(subtype), None) => {
                native.insert(subtype);
                Reading::Subtype(subtype.clone())
            }
            ("invalid_scatter", Some(subtype), None) => Reading::Subtype(subtype.clone()),
            ("combo", None, Some(layout)) => Reading::Combo(layout.clone()),
            ("probe", None, None) => Reading::MultipleGroups,
            other => panic!("{}: unexpected measured record {other:?}", part.fixture),
        };
        let found = read(&mut open(part));
        if found != expected {
            wrong.push(format!(
                "{}: Excel reopened {expected:?}, read as {found:?}",
                part.fixture
            ));
        }
    }
    assert!(
        wrong.is_empty(),
        "{} of {} parts are read differently from Excel; the first {}:\n{}",
        wrong.len(),
        measured().parts.len(),
        wrong.len().min(30),
        wrong[..wrong.len().min(30)].join("\n")
    );
    assert_eq!(classes, BTreeMap::from(CLASSES));
    let every: BTreeSet<String> = ChartSubtype::ALL
        .iter()
        .map(|subtype| format!("{subtype:?}"))
        .collect();
    assert_eq!(
        native
            .iter()
            .map(|name| (*name).to_owned())
            .collect::<BTreeSet<_>>(),
        every,
        "the native parts hold every subtype and no other"
    );
}

#[test]
fn every_chart_type_is_measured_or_listed() {
    let names: BTreeSet<String> = ChartSubtype::ALL
        .iter()
        .map(|subtype| format!("{subtype:?}"))
        .collect();
    assert_eq!(
        names.len(),
        ChartSubtype::COUNT,
        "ALL names a subtype twice"
    );

    for kind in CHART_TYPES {
        let reached = ChartSubtype::ALL
            .iter()
            .any(|subtype| subtype.chart_type() == kind);
        let listed = UNMEASURED.contains(&kind);
        assert!(
            reached != listed,
            "{kind:?} is reached by a subtype: {reached}, on the unmeasured list: {listed}"
        );
    }

    // The group a subtype maps to is the group the measured parts write: the
    // only one, or the last of a volume stock chart's two.
    let mut wrong = Vec::new();
    for part in &measured().parts {
        let Some(name) = &part.subtype else {
            continue;
        };
        let groups = plot_groups(&read_part(part));
        let written = groups.last().expect("a part has a plot group");
        let kind = CHART_TYPES
            .iter()
            .find(|kind| plot_group_element(**kind) == written)
            .unwrap_or_else(|| panic!("{} writes an unknown group {written}", part.fixture));
        if subtype(name).chart_type() != *kind {
            wrong.push(format!(
                "{}: {name} maps to {:?}, the part writes {kind:?}",
                part.fixture,
                subtype(name).chart_type()
            ));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn mixed_or_unknown_source_is_refused() {
    let line = read_part(find_part("LineWithMarkers", 3));
    let column = read_part(find_part("ClusteredColumn", 3));
    let column_3d = read_part(find_part("ClusteredColumn3D", 3));
    let stock = read_part(find_part("HighLowClose", 3));
    let pie = read_part(find_part("ExplodedPie", 3));
    let bubble = read_part(find_part("Bubble", 2));
    let scatter = read_part(find_part("ScatterLinesWithMarkers", 2));
    let straight_scatter = read_part(find_part("ScatterLines", 2));
    let radar = read_part(find_part("Radar", 3));
    let cases = [
        (
            "a line chart whose last series hides its marker",
            replaced_last(
                &line,
                "<c:symbol val=\"circle\"/>",
                "<c:symbol val=\"none\"/>",
            ),
        ),
        (
            "a column chart with a cylinder series",
            inserted_after(&column, "</c:val>", "<c:shape val=\"cylinder\"/>"),
        ),
        (
            "a 3-D column chart with cylinder columns",
            replaced(
                &column_3d,
                "<c:shape val=\"box\"/>",
                "<c:shape val=\"cylinder\"/>",
            ),
        ),
        (
            "a stock chart without high-low lines",
            without_between(&stock, "<c:hiLowLines>", "</c:hiLowLines>"),
        ),
        (
            "a pie whose series explode by neither 0 nor 25",
            replaced_all(
                &pie,
                "<c:explosion val=\"25\"/>",
                "<c:explosion val=\"40\"/>",
            ),
        ),
        (
            "a bubble chart whose last series differs in its 3-D effect",
            replaced_last(
                &bubble,
                "<c:bubble3D val=\"0\"/>",
                "<c:bubble3D val=\"1\"/>",
            ),
        ),
        (
            "a scatter chart in the marker style",
            replaced(
                &scatter,
                "<c:scatterStyle val=\"lineMarker\"/>",
                "<c:scatterStyle val=\"marker\"/>",
            ),
        ),
        (
            "a straight scatter chart whose last line is hidden",
            replaced_last(
                &straight_scatter,
                "</c:tx>",
                "</c:tx><c:spPr><a:ln><a:noFill/></a:ln></c:spPr>",
            ),
        ),
        (
            "a radar chart in the standard style",
            replaced(
                &radar,
                "<c:radarStyle val=\"marker\"/>",
                "<c:radarStyle val=\"standard\"/>",
            ),
        ),
    ];
    for (what, part) in cases {
        let mut book = workbook_with(&part);
        let before = chart_part(&book);
        for target in [
            ChartSubtype::ClusteredColumn,
            ChartSubtype::Surface3D,
            ChartSubtype::HighLowClose,
        ] {
            assert_eq!(
                convert(&mut book, target).err(),
                Some(ChartConversionRefusal::Unsupported {
                    from: None,
                    to:   target,
                }),
                "{what} to {target:?}"
            );
        }
        assert_unchanged(&book, &before, what);
    }

    let mut empty = Chart::default();
    assert_eq!(
        empty.convert_chart_type(ChartSubtype::Pie).err(),
        Some(ChartConversionRefusal::Unsupported {
            from: None,
            to:   ChartSubtype::Pie,
        }),
        "a chart without a plot group"
    );
}

#[test]
fn multi_group_charts_outside_the_measured_layouts_are_refused() {
    let by_part = observations_by_part();
    let mut checked = 0;
    for (index, part) in measured().parts.iter().enumerate() {
        if part.class != "probe" {
            continue;
        }
        let mut book = open(part);
        let before = chart_part(&book);
        for observation in &by_part[index] {
            assert!(observation.ok, "Excel made every probe conversion");
            assert_eq!(
                convert(&mut book, subtype(&observation.target)).err(),
                Some(ChartConversionRefusal::MultipleGroups),
                "{} to {}",
                part.fixture,
                observation.target
            );
            checked += 1;
        }
        assert_unchanged(&book, &before, &part.fixture);
    }
    assert_eq!(checked, 70, "the probes' Excel-successful conversions");

    // A measured column and line chart with an area group added. The chart is
    // the part as measured, not one the operation has already converted.
    let layout = measured()
        .parts
        .iter()
        .find(|part| part.layout.as_deref() == Some("ColumnLine"))
        .expect("the measurement holds a column and line chart");
    let mut book = open(layout);
    chart(&mut book)
        .plot_area_mut()
        .add_chart_group(ChartGroup::AreaChart(AreaChart::default()));
    assert_eq!(
        chart(&mut book).plot_area().chart_groups().count(),
        3,
        "a column, line and area chart has three plot groups"
    );
    let before = chart_part(&book);
    assert_eq!(
        convert(&mut book, ChartSubtype::LineWithMarkers).err(),
        Some(ChartConversionRefusal::MultipleGroups),
        "a column, line and area chart"
    );
    assert_unchanged(&book, &before, "a column, line and area chart");
}
