//! Section 6's group builder: a plot group of the result, read without its
//! series through umya's reader for its type, so the group holds exactly the
//! children Excel writes, absent ones included.
use super::{
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
            DoughnutChart,
            ExtensionList,
            Line3DChart,
            LineChart,
            OfPieChart,
            Pie3DChart,
            PieChart,
            PlotArea,
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
    ($element:expr, $session:expr, $extensions:expr, $($name:literal => $kind:ident),* $(,)?) => {
        match $element.local() {
            $($name => {
                let mut group = $kind::default();
                let read = xml::read($element, |reader, start, _| {
                    group.set_attributes(reader, start, $session)
                });
                group.set_extensions($extensions);
                read.map(|()| ChartGroup::$kind(group))
            })*
            _ => None,
        }
    };
}

/// One group element without its series, read by its type's reader, with
/// `extensions` as its extension list.
fn read_group(
    element: &Element,
    session: &SourceCacheSession,
    extensions: Option<ExtensionList>,
) -> Option<ChartGroup> {
    read_as!(element, session, extensions,
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

/// The result's plot groups, each without its series, and the `c:idx` of the
/// series each takes, in order. umya reads an extension list only within the
/// part it came from, so a group that keeps one takes it from `live`'s group
/// at its position, which must be of its type.
pub(super) fn read_groups(
    area: &Element,
    live: &PlotArea,
) -> Result<Vec<(ChartGroup, Vec<u32>)>, Unresolved> {
    let session = SourceCacheSession::deferred();
    let mut live = live.chart_groups();
    area.children
        .iter()
        .filter(|child| child.local().ends_with("Chart"))
        .map(|element| {
            let kept = live.next();
            let mut skeleton = element.clone();
            skeleton.remove("ser");
            let extensions = if skeleton.child("extLst").is_some() {
                skeleton.remove("extLst");
                let kept = kept
                    .filter(|group| tag(group.kind()) == element.local())
                    .and_then(|group| group.into_reference_role_parts().extensions)
                    .ok_or(Unresolved::Rejected)?;
                Some(kept.clone())
            } else {
                None
            };
            let series = element
                .children_named("ser")
                .map(|series| series.val("idx").and_then(|idx| idx.parse().ok()))
                .collect::<Option<Vec<u32>>>()
                .ok_or(Unresolved::Rejected)?;
            let group = read_group(&skeleton, &session, extensions).ok_or(Unresolved::Rejected)?;
            Ok((group, series))
        })
        .collect()
}
