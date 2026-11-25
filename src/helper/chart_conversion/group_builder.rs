//! Section 6's group builder: a plot group of the result, read without its
//! series through umya's reader for its type, so the group holds exactly the
//! children Excel writes, absent ones included.
use super::{
    axes::restore_shape,
    build::Owner,
    plan::Unresolved,
    xml::{
        self,
        Element,
    },
};
use crate::{
    reader::xlsx::SourceCacheSession,
    structs::{
        ChartType,
        drawing::charts::{
            Area3DChart,
            AreaChart,
            Bar3DChart,
            BarChart,
            BubbleChart,
            ChartGroup,
            ChartLines,
            DoughnutChart,
            Line3DChart,
            LineChart,
            OfPieChart,
            Pie3DChart,
            PieChart,
            RadarChart,
            ScatterChart,
            StockChart,
            Surface3DChart,
            SurfaceChart,
        },
    },
};

/// The source group's children a target group starts from: those the rule
/// table governs, and the of-pie split type, which no rule names.
pub(super) const GOVERNED: [&str; 27] = [
    "barDir",
    "grouping",
    "varyColors",
    "gapWidth",
    "overlap",
    "firstSliceAng",
    "holeSize",
    "bubbleScale",
    "showNegBubbles",
    "sizeRepresents",
    "scatterStyle",
    "radarStyle",
    "wireframe",
    "bubble3D",
    "shape",
    "gapDepth",
    "marker",
    "smooth",
    "hiLowLines",
    "upDownBars",
    "dropLines",
    "serLines",
    "bandFmts",
    "dLbls",
    "ofPieType",
    "secondPieSize",
    "splitType",
];

/// A plot group's element name.
pub(super) const fn tag(chart_type: ChartType) -> &'static str {
    match chart_type {
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

macro_rules! read_as {
    ($element:expr, $session:expr, $($name:literal => $kind:ident),* $(,)?) => {
        match $element.local() {
            $($name => {
                let mut group = $kind::default();
                xml::read($element, |reader, start, _| {
                    group.set_attributes(reader, start, $session)
                })
                .map(|()| ChartGroup::$kind(group))
            })*
            _ => None,
        }
    };
}

/// One group element, read by its type's reader without its series and its
/// extension lists, which umya reads only within their part.
fn read_group(element: &Element, session: &SourceCacheSession) -> Option<ChartGroup> {
    let element = xml::without_extensions(&shallow(element, &["ser"]));
    read_as!(&element, session,
        "lineChart" => LineChart,
        "line3DChart" => Line3DChart,
        "pieChart" => PieChart,
        "pie3DChart" => Pie3DChart,
        "doughnutChart" => DoughnutChart,
        "scatterChart" => ScatterChart,
        "barChart" => BarChart,
        "bar3DChart" => Bar3DChart,
        "radarChart" => RadarChart,
        "bubbleChart" => BubbleChart,
        "areaChart" => AreaChart,
        "area3DChart" => Area3DChart,
        "ofPieChart" => OfPieChart,
        "stockChart" => StockChart,
        "surfaceChart" => SurfaceChart,
        "surface3DChart" => Surface3DChart,
    )
}

/// `element` without its children named in `drop`. The dropped children
/// (series, and axis ids for a skeleton) are never copied.
fn shallow(element: &Element, drop: &[&str]) -> Element {
    Element {
        name:       element.name.clone(),
        attributes: element.attributes.clone(),
        children:   element
            .children
            .iter()
            .filter(|child| !drop.contains(&child.local()))
            .cloned()
            .collect(),
        text:       element.text.clone(),
    }
}

/// One plot group of the result: how it is built, the ids of its axes, and
/// the `c:idx` of the series it takes, in order.
pub(super) type ReadGroup = (Owner<ChartGroup>, Vec<u32>, Vec<u32>);

/// The result's plot groups from their elements in `area`. A group at the
/// position of a source group retains it: unchanged as written apart from its
/// series and axis ids, it is the live group; otherwise it is read and takes
/// the live group's stored state when staged. `source_area` is the source as
/// written.
pub(super) fn read_groups(
    area: &Element,
    source_area: &Element,
) -> Result<Vec<ReadGroup>, Unresolved> {
    fn groups(area: &Element) -> Vec<&Element> {
        area.children
            .iter()
            .filter(|child| child.local().ends_with("Chart"))
            .collect()
    }
    let session = SourceCacheSession::deferred();
    let sources = groups(source_area);
    let skeleton = |group: &Element| shallow(group, &["ser", "axId"]);
    let read = |group: &Element| read_group(group, &session).ok_or(Unresolved::Rejected);
    groups(area)
        .into_iter()
        .enumerate()
        .map(|(at, element)| {
            let axes = element
                .children_named("axId")
                .map(|id| id.attribute("val").and_then(|id| id.parse().ok()))
                .collect::<Option<Vec<u32>>>()
                .ok_or(Unresolved::Rejected)?;
            let series = element
                .children_named("ser")
                .map(|series| series.val("idx").and_then(|idx| idx.parse().ok()))
                .collect::<Option<Vec<u32>>>()
                .ok_or(Unresolved::Rejected)?;
            let owner = match sources.get(at).copied() {
                None => Owner::Rebuilt {
                    read:  read(element)?,
                    probe: None,
                },
                Some(source) if skeleton(source) == skeleton(element) => Owner::Kept,
                Some(source) => Owner::Rebuilt {
                    read:  read(element)?,
                    probe: Some(read(source)?),
                },
            };
            Ok((owner, axes, series))
        })
        .collect()
}

/// Restores into `group`, read from a planned element, the stored state of
/// `previous`, the live group at its position, that the writer leaves out: a
/// group of the same type through its own restorer, and the labels and lines
/// a retyped group carries too.
pub(super) fn restore(group: &mut ChartGroup, previous: &ChartGroup) -> Option<()> {
    let same_type = group.kind() == previous.kind();
    let old = previous.as_group_ref();
    let mut current = group.as_group_mut();
    current.restore_export_parts(old);
    let lines = current.lines_mut();
    let old_lines = old.lines();
    for (line, old) in [
        (lines.drop_lines, old_lines.drop_lines),
        (lines.hi_low_lines, old_lines.hi_low_lines),
    ] {
        restore_shape(
            line.and_then(ChartLines::shape_properties_mut),
            old.and_then(ChartLines::shape_properties),
        );
    }
    if let (Some(bars), Some(old)) = (lines.up_down_bars, old_lines.up_down_bars) {
        bars.restore_export_parts(old);
        restore_shape(
            bars.up_bars_mut()
                .and_then(ChartLines::shape_properties_mut),
            old.up_bars().and_then(ChartLines::shape_properties),
        );
        restore_shape(
            bars.down_bars_mut()
                .and_then(ChartLines::shape_properties_mut),
            old.down_bars().and_then(ChartLines::shape_properties),
        );
    }
    // A band format takes the stored state of the one band with its index.
    for band in lines.band_formats.iter_mut() {
        let index = band.index().explicit_value();
        let mut same = old_lines
            .band_formats
            .iter()
            .filter(|old| old.index().explicit_value() == index);
        if let (Some(old), None) = (same.next(), same.next()) {
            restore_shape(band.shape_properties_mut(), old.shape_properties());
        }
    }
    let parts = current.into_reference_role_parts();
    let old_parts = old.into_reference_role_parts();
    if let (Some(labels), Some(old)) = (parts.labels, old_parts.labels) {
        labels.restore_export_parts(old).ok()?;
    }
    // The same type's restorer brings back its series lines already.
    if !same_type {
        if let (Some(lines), Some(old)) = (parts.series_lines, old_parts.series_lines) {
            for (line, old) in lines.iter_mut().zip(old) {
                restore_shape(line.shape_properties_mut(), old.shape_properties());
            }
        }
    }
    Some(())
}
