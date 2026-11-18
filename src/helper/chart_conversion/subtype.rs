//! Reads a chart's source type from the plot groups it holds, by what the
//! measured Excel charts hold. A chart that matches none of them is not
//! recognised.
use super::{
    ChartSource,
    ChartSubtype,
    ComboLayout,
};
use crate::structs::{
    ChartType,
    drawing::charts::{
        AreaChartSeries,
        AxisId,
        Bar3DChart,
        BarChart,
        BarDirectionValues,
        Bubble3D,
        ChartGroupRef,
        Grouping,
        GroupingValues,
        LineChart,
        MarkerStyleValues,
        OfPieValues,
        PlotArea,
        RadarChart,
        RadarStyleValues,
        ScatterChart,
        ScatterStyleValues,
        ShapeProperties,
        ShapeValues,
        StockChart,
    },
};

/// The explosion Excel writes on every series of an exploded pie or doughnut.
const EXPLOSION: u32 = 25;

impl ChartSubtype {
    /// The plot group that holds this subtype. A volume stock subtype is held
    /// by a stock group, which a column group precedes.
    #[must_use]
    pub const fn chart_type(self) -> ChartType {
        match self {
            Self::ClusteredColumn
            | Self::StackedColumn
            | Self::PercentStackedColumn
            | Self::ClusteredBar
            | Self::StackedBar
            | Self::PercentStackedBar => ChartType::BarChart,
            Self::ClusteredColumn3D
            | Self::StackedColumn3D
            | Self::PercentStackedColumn3D
            | Self::Column3D
            | Self::ClusteredBar3D
            | Self::StackedBar3D
            | Self::PercentStackedBar3D => ChartType::Bar3DChart,
            Self::Line
            | Self::StackedLine
            | Self::PercentStackedLine
            | Self::LineWithMarkers
            | Self::StackedLineWithMarkers
            | Self::PercentStackedLineWithMarkers => ChartType::LineChart,
            Self::Line3D => ChartType::Line3DChart,
            Self::Area | Self::StackedArea | Self::PercentStackedArea => ChartType::AreaChart,
            Self::Area3D | Self::StackedArea3D | Self::PercentStackedArea3D => {
                ChartType::Area3DChart
            }
            Self::Pie | Self::ExplodedPie => ChartType::PieChart,
            Self::Pie3D | Self::ExplodedPie3D => ChartType::Pie3DChart,
            Self::PieOfPie | Self::BarOfPie => ChartType::OfPieChart,
            Self::Doughnut | Self::ExplodedDoughnut => ChartType::DoughnutChart,
            Self::Scatter
            | Self::ScatterSmoothWithMarkers
            | Self::ScatterSmooth
            | Self::ScatterLinesWithMarkers
            | Self::ScatterLines => ChartType::ScatterChart,
            Self::Bubble | Self::Bubble3DEffect => ChartType::BubbleChart,
            Self::Radar | Self::RadarWithMarkers | Self::FilledRadar => ChartType::RadarChart,
            Self::Surface3D | Self::WireframeSurface3D => ChartType::Surface3DChart,
            Self::Contour | Self::WireframeContour => ChartType::SurfaceChart,
            Self::HighLowClose
            | Self::OpenHighLowClose
            | Self::VolumeHighLowClose
            | Self::VolumeOpenHighLowClose => ChartType::StockChart,
        }
    }
}

/// What the plot groups say the chart is: one group holds a subtype, two groups
/// hold a volume stock subtype or a combination layout, and anything else is
/// not recognised.
pub(super) fn classify(plot_area: &PlotArea) -> Option<ChartSource> {
    let groups: Vec<ChartGroupRef<'_>> = plot_area.chart_groups().collect();
    match groups.as_slice() {
        [group] => single_group(*group).map(ChartSource::Subtype),
        [first, second] => volume_stock(*first, *second)
            .map(ChartSource::Subtype)
            .or_else(|| combo_layout(*first, *second).map(ChartSource::Combo)),
        _ => None,
    }
}

fn single_group(group: ChartGroupRef<'_>) -> Option<ChartSubtype> {
    let series = group.series().area_chart_series();
    match group {
        ChartGroupRef::BarChart(bar) => bar_subtype(bar, series),
        ChartGroupRef::Bar3DChart(bar) => bar_3d_subtype(bar, series),
        ChartGroupRef::LineChart(line) => line_subtype(line, series),
        ChartGroupRef::Line3DChart(line) => {
            matches!(line.grouping().val(), GroupingValues::Standard)
                .then_some(ChartSubtype::Line3D)
        }
        ChartGroupRef::AreaChart(area) => area_subtype(
            area.grouping(),
            [
                ChartSubtype::Area,
                ChartSubtype::StackedArea,
                ChartSubtype::PercentStackedArea,
            ],
        ),
        ChartGroupRef::Area3DChart(area) => area_subtype(
            area.grouping(),
            [
                ChartSubtype::Area3D,
                ChartSubtype::StackedArea3D,
                ChartSubtype::PercentStackedArea3D,
            ],
        ),
        ChartGroupRef::PieChart(_) => {
            exploded_subtype(series, ChartSubtype::Pie, ChartSubtype::ExplodedPie)
        }
        ChartGroupRef::Pie3DChart(_) => {
            exploded_subtype(series, ChartSubtype::Pie3D, ChartSubtype::ExplodedPie3D)
        }
        ChartGroupRef::DoughnutChart(_) => exploded_subtype(
            series,
            ChartSubtype::Doughnut,
            ChartSubtype::ExplodedDoughnut,
        ),
        ChartGroupRef::OfPieChart(pie) => Some(match pie.of_pie_type().val() {
            OfPieValues::Pie => ChartSubtype::PieOfPie,
            OfPieValues::Bar => ChartSubtype::BarOfPie,
        }),
        ChartGroupRef::ScatterChart(scatter) => scatter_subtype(scatter, series),
        ChartGroupRef::RadarChart(radar) => radar_subtype(radar, series),
        ChartGroupRef::BubbleChart(_) => bubble_subtype(series),
        ChartGroupRef::StockChart(stock) => stock_subtype(
            stock,
            ChartSubtype::HighLowClose,
            ChartSubtype::OpenHighLowClose,
        ),
        ChartGroupRef::SurfaceChart(surface) => Some(if surface.wireframe().val() {
            ChartSubtype::WireframeContour
        } else {
            ChartSubtype::Contour
        }),
        ChartGroupRef::Surface3DChart(surface) => Some(if surface.wireframe().val() {
            ChartSubtype::WireframeSurface3D
        } else {
            ChartSubtype::Surface3D
        }),
    }
}

/// Whether every series is a plain box, which a series without a shape is.
fn has_box_shapes(series: &[AreaChartSeries]) -> bool {
    series.iter().all(|each| {
        each.shape()
            .is_none_or(|shape| matches!(shape.val(), ShapeValues::Box))
    })
}

fn bar_subtype(bar: &BarChart, series: &[AreaChartSeries]) -> Option<ChartSubtype> {
    if !has_box_shapes(series) {
        return None;
    }
    match (bar.bar_direction().val(), bar.grouping().val()) {
        (BarDirectionValues::Column, GroupingValues::Clustered) => {
            Some(ChartSubtype::ClusteredColumn)
        }
        (BarDirectionValues::Column, GroupingValues::Stacked) => Some(ChartSubtype::StackedColumn),
        (BarDirectionValues::Column, GroupingValues::PercentStacked) => {
            Some(ChartSubtype::PercentStackedColumn)
        }
        (BarDirectionValues::Bar, GroupingValues::Clustered) => Some(ChartSubtype::ClusteredBar),
        (BarDirectionValues::Bar, GroupingValues::Stacked) => Some(ChartSubtype::StackedBar),
        (BarDirectionValues::Bar, GroupingValues::PercentStacked) => {
            Some(ChartSubtype::PercentStackedBar)
        }
        (BarDirectionValues::Column | BarDirectionValues::Bar, GroupingValues::Standard) => None,
    }
}

fn bar_3d_subtype(bar: &Bar3DChart, series: &[AreaChartSeries]) -> Option<ChartSubtype> {
    if !matches!(bar.shape().val(), ShapeValues::Box) || !has_box_shapes(series) {
        return None;
    }
    match (bar.bar_direction().val(), bar.grouping().val()) {
        (BarDirectionValues::Column, GroupingValues::Clustered) => {
            Some(ChartSubtype::ClusteredColumn3D)
        }
        (BarDirectionValues::Column, GroupingValues::Stacked) => {
            Some(ChartSubtype::StackedColumn3D)
        }
        (BarDirectionValues::Column, GroupingValues::PercentStacked) => {
            Some(ChartSubtype::PercentStackedColumn3D)
        }
        (BarDirectionValues::Column, GroupingValues::Standard) => Some(ChartSubtype::Column3D),
        (BarDirectionValues::Bar, GroupingValues::Clustered) => Some(ChartSubtype::ClusteredBar3D),
        (BarDirectionValues::Bar, GroupingValues::Stacked) => Some(ChartSubtype::StackedBar3D),
        (BarDirectionValues::Bar, GroupingValues::PercentStacked) => {
            Some(ChartSubtype::PercentStackedBar3D)
        }
        (BarDirectionValues::Bar, GroupingValues::Standard) => None,
    }
}

fn line_subtype(line: &LineChart, series: &[AreaChartSeries]) -> Option<ChartSubtype> {
    let with_markers = markers_shown(series)?;
    let (plain, marked) = match line.grouping().val() {
        GroupingValues::Standard => (ChartSubtype::Line, ChartSubtype::LineWithMarkers),
        GroupingValues::Stacked => (
            ChartSubtype::StackedLine,
            ChartSubtype::StackedLineWithMarkers,
        ),
        GroupingValues::PercentStacked => (
            ChartSubtype::PercentStackedLine,
            ChartSubtype::PercentStackedLineWithMarkers,
        ),
        GroupingValues::Clustered => return None,
    };
    Some(if with_markers { marked } else { plain })
}

/// The area subtype of a grouping, from the standard, stacked and percent
/// stacked subtypes in that order.
fn area_subtype(
    grouping: &Grouping,
    [standard, stacked, percent]: [ChartSubtype; 3],
) -> Option<ChartSubtype> {
    match grouping.val() {
        GroupingValues::Standard => Some(standard),
        GroupingValues::Stacked => Some(stacked),
        GroupingValues::PercentStacked => Some(percent),
        GroupingValues::Clustered => None,
    }
}

/// `exploded` when every series is exploded and `whole` when none is.
fn exploded_subtype(
    series: &[AreaChartSeries],
    whole: ChartSubtype,
    exploded: ChartSubtype,
) -> Option<ChartSubtype> {
    let all_exploded = uniform(series, is_exploded)?;
    Some(if all_exploded { exploded } else { whole })
}

fn scatter_subtype(scatter: &ScatterChart, series: &[AreaChartSeries]) -> Option<ChartSubtype> {
    let with_markers = markers_shown(series)?;
    let first_hidden = series.first().is_some_and(has_hidden_line);
    let any_hidden = series.iter().any(has_hidden_line);
    match scatter.scatter_style().val() {
        ScatterStyleValues::LineMarker => match (with_markers, first_hidden, any_hidden) {
            (false, _, false) => Some(ChartSubtype::ScatterLines),
            (true, true, _) => Some(ChartSubtype::Scatter),
            (true, false, false) => Some(ChartSubtype::ScatterLinesWithMarkers),
            (false, _, true) | (true, false, true) => None,
        },
        ScatterStyleValues::SmoothMarker if !any_hidden => Some(if with_markers {
            ChartSubtype::ScatterSmoothWithMarkers
        } else {
            ChartSubtype::ScatterSmooth
        }),
        ScatterStyleValues::SmoothMarker
        | ScatterStyleValues::Line
        | ScatterStyleValues::Marker
        | ScatterStyleValues::Smooth => None,
    }
}

fn radar_subtype(radar: &RadarChart, series: &[AreaChartSeries]) -> Option<ChartSubtype> {
    match radar.radar_style().val() {
        RadarStyleValues::Filled => Some(ChartSubtype::FilledRadar),
        RadarStyleValues::Marker => markers_shown(series).map(|with_markers| {
            if with_markers {
                ChartSubtype::RadarWithMarkers
            } else {
                ChartSubtype::Radar
            }
        }),
        RadarStyleValues::Standard => None,
    }
}

fn bubble_subtype(series: &[AreaChartSeries]) -> Option<ChartSubtype> {
    let three_d = uniform(series, |each| each.bubble_3d().map(Bubble3D::val))?;
    Some(if three_d {
        ChartSubtype::Bubble3DEffect
    } else {
        ChartSubtype::Bubble
    })
}

/// `with_bars` when the group has up and down bars and `plain` when it has not;
/// a stock group without high-low lines is not recognised.
fn stock_subtype(
    stock: &StockChart,
    plain: ChartSubtype,
    with_bars: ChartSubtype,
) -> Option<ChartSubtype> {
    stock.hi_low_lines()?;
    Some(if stock.up_down_bars().is_some() {
        with_bars
    } else {
        plain
    })
}

/// A column group of one series, then a stock group on axes of its own.
fn volume_stock(first: ChartGroupRef<'_>, second: ChartGroupRef<'_>) -> Option<ChartSubtype> {
    let (ChartGroupRef::BarChart(volume), ChartGroupRef::StockChart(stock)) = (first, second)
    else {
        return None;
    };
    let columns = volume.area_chart_series_list().area_chart_series();
    if columns.len() != 1
        || bar_subtype(volume, columns) != Some(ChartSubtype::ClusteredColumn)
        || !separate_axes(first, second)
    {
        return None;
    }
    stock_subtype(
        stock,
        ChartSubtype::VolumeHighLowClose,
        ChartSubtype::VolumeOpenHighLowClose,
    )
}

/// Two groups that hold the first two series and the third.
fn combo_layout(first: ChartGroupRef<'_>, second: ChartGroupRef<'_>) -> Option<ComboLayout> {
    if series_indices(first) != [0, 1] || series_indices(second) != [2] {
        return None;
    }
    match (first, second) {
        (ChartGroupRef::AreaChart(area), ChartGroupRef::BarChart(bar)) => {
            let stacked = matches!(area.grouping().val(), GroupingValues::Stacked);
            (stacked && is_clustered_column(bar) && shares_axes(first, second))
                .then_some(ComboLayout::StackedAreaColumn)
        }
        (ChartGroupRef::BarChart(bar), ChartGroupRef::LineChart(line)) => {
            let standard = matches!(line.grouping().val(), GroupingValues::Standard);
            if !standard || !is_clustered_column(bar) {
                return None;
            }
            if shares_axes(first, second) {
                Some(ComboLayout::ColumnLine)
            } else if separate_axes(first, second) {
                Some(ComboLayout::ColumnLineSecondary)
            } else {
                None
            }
        }
        (ChartGroupRef::BarChart(primary), ChartGroupRef::BarChart(secondary)) => {
            (is_clustered_column(primary)
                && is_clustered_column(secondary)
                && separate_axes(first, second))
            .then_some(ComboLayout::ColumnColumnSecondary)
        }
        _ => None,
    }
}

fn is_clustered_column(bar: &BarChart) -> bool {
    bar_subtype(bar, bar.area_chart_series_list().area_chart_series())
        == Some(ChartSubtype::ClusteredColumn)
}

/// The `c:idx` of each series of the group, in ascending order.
fn series_indices(group: ChartGroupRef<'_>) -> Vec<u32> {
    let mut indices: Vec<u32> = group
        .series()
        .area_chart_series()
        .iter()
        .map(|each| each.index().val())
        .collect();
    indices.sort_unstable();
    indices
}

fn axis_ids(group: ChartGroupRef<'_>) -> Vec<u32> {
    group
        .axis_ids()
        .unwrap_or_default()
        .iter()
        .map(AxisId::val)
        .collect()
}

/// Whether both groups draw on the same axes.
fn shares_axes(first: ChartGroupRef<'_>, second: ChartGroupRef<'_>) -> bool {
    let ids = axis_ids(first);
    !ids.is_empty() && ids == axis_ids(second)
}

/// Whether the groups draw on axes with no axis in common.
fn separate_axes(first: ChartGroupRef<'_>, second: ChartGroupRef<'_>) -> bool {
    let (first, second) = (axis_ids(first), axis_ids(second));
    !first.is_empty() && !second.is_empty() && first.iter().all(|id| !second.contains(id))
}

/// The value every series agrees on. `None` when there is no series, a series
/// has no value or the series differ.
fn uniform(
    series: &[AreaChartSeries],
    value: impl Fn(&AreaChartSeries) -> Option<bool>,
) -> Option<bool> {
    let first = value(series.first()?)?;
    series
        .iter()
        .all(|each| value(each) == Some(first))
        .then_some(first)
}

/// Whether every series shows its marker. `None` when the series disagree or
/// one leaves the marker to the application.
fn markers_shown(series: &[AreaChartSeries]) -> Option<bool> {
    uniform(series, |each| {
        let symbol = each.marker()?.symbol()?;
        if !symbol.has_val() {
            return None;
        }
        match symbol.val() {
            MarkerStyleValues::Auto => None,
            MarkerStyleValues::None => Some(false),
            _ => Some(true),
        }
    })
}

/// Whether the series is exploded. `None` for an explosion Excel does not write
/// on its pie and doughnut types.
fn is_exploded(series: &AreaChartSeries) -> Option<bool> {
    let Some(explosion) = series.explosion() else {
        return Some(false);
    };
    match explosion.parsed_val()? {
        0 => Some(false),
        EXPLOSION => Some(true),
        _ => None,
    }
}

/// Whether the series draws no line, as a markers-only scatter series does.
fn has_hidden_line(series: &AreaChartSeries) -> bool {
    series
        .shape_properties()
        .and_then(ShapeProperties::outline)
        .is_some_and(|outline| outline.no_fill().is_some())
}
