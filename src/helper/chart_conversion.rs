//! Changes a chart to another chart type the way Excel's "Change Chart Type"
//! does.
//!
//! [`Workbook::convert_chart_type`] converts one chart of a sheet to a
//! [`ChartSubtype`] and returns a [`ChartConversion`] that lists what changed.
//! It reads the cells the chart's references name, as Excel does. When the
//! chart cannot be converted it returns a [`ChartConversionRefusal`] and
//! leaves the chart as it was.
//!
//! The chart's own type is read from what its plot groups and series hold,
//! never from how it was made. It is one of the [`ChartSubtype`]s or one of the
//! four [`ComboLayout`]s that Excel's ordinary combination charts produce; any
//! other chart is refused.
use std::{
    error::Error,
    fmt,
};

use crate::{
    Workbook,
    structs::drawing::charts::{
        Chart,
        PlotArea,
        SeriesPartKind,
    },
};

mod axes;
mod build;
mod commit;
mod group_builder;
mod model;
mod plan;
mod rule;
#[rustfmt::skip]
mod rules;
mod series;
mod slots;
mod source;
mod subtype;
mod write;
mod xml;

/// A chart type Excel's "Change Chart Type" produces, named the way Excel
/// names it.
///
/// [`ChartType`](crate::structs::ChartType) names only the plot group; a
/// subtype also fixes the grouping, the bar direction, the markers and the
/// explosion of the chart.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ChartSubtype {
    ClusteredColumn,
    StackedColumn,
    PercentStackedColumn,
    ClusteredBar,
    StackedBar,
    PercentStackedBar,
    Line,
    StackedLine,
    PercentStackedLine,
    LineWithMarkers,
    StackedLineWithMarkers,
    PercentStackedLineWithMarkers,
    Area,
    StackedArea,
    PercentStackedArea,
    Pie,
    ExplodedPie,
    PieOfPie,
    BarOfPie,
    Doughnut,
    ExplodedDoughnut,
    /// Markers only, without lines.
    Scatter,
    ScatterSmoothWithMarkers,
    ScatterSmooth,
    ScatterLinesWithMarkers,
    ScatterLines,
    Bubble,
    /// Bubbles drawn with a 3-D effect.
    Bubble3DEffect,
    Radar,
    RadarWithMarkers,
    FilledRadar,
    ClusteredColumn3D,
    StackedColumn3D,
    PercentStackedColumn3D,
    /// A 3-D column chart with a series axis.
    Column3D,
    ClusteredBar3D,
    StackedBar3D,
    PercentStackedBar3D,
    Line3D,
    Area3D,
    StackedArea3D,
    PercentStackedArea3D,
    Pie3D,
    ExplodedPie3D,
    Surface3D,
    WireframeSurface3D,
    /// A surface seen from above.
    Contour,
    /// A wireframe surface seen from above.
    WireframeContour,
    /// High, low and close lines.
    HighLowClose,
    /// Open, high, low and close lines with up and down bars.
    OpenHighLowClose,
    /// A volume column group, then high, low and close lines on secondary
    /// axes.
    VolumeHighLowClose,
    /// A volume column group, then open, high, low and close lines with up and
    /// down bars on secondary axes.
    VolumeOpenHighLowClose,
}

impl ChartSubtype {
    /// Every subtype, in declaration order.
    pub const ALL: [Self; Self::COUNT] = [
        Self::ClusteredColumn,
        Self::StackedColumn,
        Self::PercentStackedColumn,
        Self::ClusteredBar,
        Self::StackedBar,
        Self::PercentStackedBar,
        Self::Line,
        Self::StackedLine,
        Self::PercentStackedLine,
        Self::LineWithMarkers,
        Self::StackedLineWithMarkers,
        Self::PercentStackedLineWithMarkers,
        Self::Area,
        Self::StackedArea,
        Self::PercentStackedArea,
        Self::Pie,
        Self::ExplodedPie,
        Self::PieOfPie,
        Self::BarOfPie,
        Self::Doughnut,
        Self::ExplodedDoughnut,
        Self::Scatter,
        Self::ScatterSmoothWithMarkers,
        Self::ScatterSmooth,
        Self::ScatterLinesWithMarkers,
        Self::ScatterLines,
        Self::Bubble,
        Self::Bubble3DEffect,
        Self::Radar,
        Self::RadarWithMarkers,
        Self::FilledRadar,
        Self::ClusteredColumn3D,
        Self::StackedColumn3D,
        Self::PercentStackedColumn3D,
        Self::Column3D,
        Self::ClusteredBar3D,
        Self::StackedBar3D,
        Self::PercentStackedBar3D,
        Self::Line3D,
        Self::Area3D,
        Self::StackedArea3D,
        Self::PercentStackedArea3D,
        Self::Pie3D,
        Self::ExplodedPie3D,
        Self::Surface3D,
        Self::WireframeSurface3D,
        Self::Contour,
        Self::WireframeContour,
        Self::HighLowClose,
        Self::OpenHighLowClose,
        Self::VolumeHighLowClose,
        Self::VolumeOpenHighLowClose,
    ];
    /// The number of subtypes.
    pub const COUNT: usize = 52;

    /// The series count Excel accepts for this subtype.
    const fn series_requirement(self) -> SeriesRequirement {
        match self {
            Self::Surface3D | Self::WireframeSurface3D | Self::Contour | Self::WireframeContour => {
                SeriesRequirement::AtLeastTwo
            }
            Self::HighLowClose => SeriesRequirement::Exactly(3),
            Self::OpenHighLowClose | Self::VolumeHighLowClose => SeriesRequirement::Exactly(4),
            Self::VolumeOpenHighLowClose => SeriesRequirement::Exactly(5),
            Self::ClusteredColumn
            | Self::StackedColumn
            | Self::PercentStackedColumn
            | Self::ClusteredBar
            | Self::StackedBar
            | Self::PercentStackedBar
            | Self::Line
            | Self::StackedLine
            | Self::PercentStackedLine
            | Self::LineWithMarkers
            | Self::StackedLineWithMarkers
            | Self::PercentStackedLineWithMarkers
            | Self::Area
            | Self::StackedArea
            | Self::PercentStackedArea
            | Self::Pie
            | Self::ExplodedPie
            | Self::PieOfPie
            | Self::BarOfPie
            | Self::Doughnut
            | Self::ExplodedDoughnut
            | Self::Scatter
            | Self::ScatterSmoothWithMarkers
            | Self::ScatterSmooth
            | Self::ScatterLinesWithMarkers
            | Self::ScatterLines
            | Self::Bubble
            | Self::Bubble3DEffect
            | Self::Radar
            | Self::RadarWithMarkers
            | Self::FilledRadar
            | Self::ClusteredColumn3D
            | Self::StackedColumn3D
            | Self::PercentStackedColumn3D
            | Self::Column3D
            | Self::ClusteredBar3D
            | Self::StackedBar3D
            | Self::PercentStackedBar3D
            | Self::Line3D
            | Self::Area3D
            | Self::StackedArea3D
            | Self::PercentStackedArea3D
            | Self::Pie3D
            | Self::ExplodedPie3D => SeriesRequirement::Any,
        }
    }
}

/// The two-group layouts Excel's ordinary combination charts produce. They are
/// read as sources and are never a target.
///
/// In each layout the first group holds the first two series and the second
/// group holds the third.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ComboLayout {
    /// A stacked area group, then a clustered column group, on one set of
    /// axes.
    StackedAreaColumn,
    /// A clustered column group, then a line group, on one set of axes.
    ColumnLine,
    /// A clustered column group on the primary axes, then a line group on the
    /// secondary axes.
    ColumnLineSecondary,
    /// A clustered column group on the primary axes, then a clustered column
    /// group on the secondary axes.
    ColumnColumnSecondary,
}

/// What a chart is before a conversion.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ChartSource {
    /// A chart of one subtype.
    Subtype(ChartSubtype),
    /// A chart of one combination layout.
    Combo(ComboLayout),
}

/// What a conversion did to the chart.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChartConversion {
    from:         ChartSource,
    to:           ChartSubtype,
    series_paint: Vec<SeriesPaint>,
    changes:      Vec<ConversionChange>,
}

impl ChartConversion {
    /// What the chart was before the conversion.
    #[must_use]
    pub const fn from(&self) -> ChartSource {
        self.from
    }

    /// The subtype the chart is now.
    #[must_use]
    pub const fn to(&self) -> ChartSubtype {
        self.to
    }

    /// How each series is painted afterwards, one entry per series in series
    /// order. Empty when the chart already was the target.
    #[must_use]
    pub fn series_paint(&self) -> &[SeriesPaint] {
        &self.series_paint
    }

    /// Every part the conversion added, removed, moved or rewrote, each once.
    /// Sorted: chart level first, then by series, then by part. Empty when the
    /// chart already was the target.
    #[must_use]
    pub fn changes(&self) -> &[ConversionChange] {
        &self.changes
    }

    /// The report of a planned conversion.
    fn new(plan: &plan::Plan) -> Self {
        Self {
            from:         plan.from,
            to:           plan.to,
            series_paint: plan.paint.clone(),
            changes:      plan.changes.clone(),
        }
    }

    /// The report of a chart that already was `target` and stays as it is.
    fn unchanged(target: ChartSubtype) -> Self {
        Self {
            from:         ChartSource::Subtype(target),
            to:           target,
            series_paint: Vec::new(),
            changes:      Vec::new(),
        }
    }
}

/// How one series is painted after a conversion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SeriesPaint {
    pub part:  PaintedPart,
    pub color: PaintColor,
}

/// The part of a series that carries its colour, which follows the geometry of
/// the target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaintedPart {
    /// The area, column, bar, slice or bubble, and filled radar.
    Fill,
    /// The line of a line, scatter, radar or stock series.
    Line,
    /// The marker of a markers-only scatter series.
    Marker,
    /// A position that is actually unpainted. A series without explicit shape
    /// properties is not unpainted.
    Nothing,
}

/// Where the colour of a painted part comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaintColor {
    /// A colour the series sets itself.
    Explicit,
    /// The automatic accent colour of the series' position in the theme.
    SeriesAccent,
    /// No colour, because the part is unpainted.
    Unpainted,
}

/// One part a conversion added, removed, moved or rewrote.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConversionChange {
    /// The series the change belongs to, counted from zero in series order.
    /// `None` is a change to the chart itself.
    pub series_index: Option<usize>,
    pub part:         ConvertedPart,
    pub change:       ChangeKind,
}

/// A part of the chart a conversion can change.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ConvertedPart {
    /// The 3-D view, the floor and the walls.
    Scene,
    /// The settings of the plot group itself.
    Group,
    Axis {
        role:      AxisRole,
        secondary: bool,
    },
    /// A series child other than its shape properties: marker, data roles,
    /// points, trendlines and so on.
    SeriesPart(SeriesPartKind),
    /// The fill inside the series' shape properties.
    SeriesFill,
    /// The outline inside the series' shape properties.
    SeriesLine,
}

/// What an axis measures.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AxisRole {
    Category,
    Value,
    Series,
}

/// What a conversion did to a part.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChangeKind {
    Added,
    Removed,
    Rewritten,
    /// The part's content now has this role, as categories become x values.
    MovedTo(SeriesPartKind),
}

/// Why a conversion was refused. The chart is unchanged.
///
/// The checks run in a fixed order and the first that applies decides.
/// First the chart is read: a chart that is no recognised subtype or layout is
/// [`MultipleGroups`](Self::MultipleGroups) when it has several plot groups
/// and [`Unsupported`](Self::Unsupported) otherwise. Then a surface target
/// needs two series and a stock target an exact number of them. Only after
/// those do the same type and an unmeasured pair differ.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChartConversionRefusal {
    /// The chart is not a recognised subtype or layout (`from` is `None`), or
    /// Excel's result for this pair has not been measured.
    Unsupported {
        from: Option<ChartSource>,
        to:   ChartSubtype,
    },
    /// The target needs an exact number of series. Excel allows at most 255,
    /// so a larger count is reported as 255.
    SeriesCountMismatch { required: u8, actual: u8 },
    /// A surface target needs at least two series.
    NotValidForKind,
    /// The sheet has no chart at that position: no anchor, or an anchor
    /// without a graphic frame, or its sheet cannot be read.
    ChartNotFound,
    /// The chart has more than one plot group and is neither a volume stock
    /// chart nor one of the measured combination layouts.
    MultipleGroups,
}

impl fmt::Display for ChartConversionRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported {
                from: Some(from),
                to,
            } => write!(f, "converting {from:?} to {to:?} is not supported"),
            Self::Unsupported { from: None, to } => write!(
                f,
                "the chart is not a recognised type and cannot be converted to {to:?}"
            ),
            Self::SeriesCountMismatch {
                required,
                actual: u8::MAX,
            } => write!(
                f,
                "the target needs exactly {required} series and the chart has {} or more",
                u8::MAX
            ),
            Self::SeriesCountMismatch { required, actual } => write!(
                f,
                "the target needs exactly {required} series and the chart has {actual}"
            ),
            Self::NotValidForKind => f.write_str("a surface chart needs at least two series"),
            Self::ChartNotFound => f.write_str("the sheet has no chart at that position"),
            Self::MultipleGroups => f.write_str(
                "the chart has several plot groups and is neither a volume stock chart nor a \
                 measured combination layout",
            ),
        }
    }
}

impl Error for ChartConversionRefusal {}

/// The number of series a target needs before Excel accepts it.
#[derive(Clone, Copy)]
enum SeriesRequirement {
    Any,
    /// Every surface type.
    AtLeastTwo,
    /// Every stock type.
    Exactly(u8),
}

impl SeriesRequirement {
    fn refusal(self, series: usize) -> Option<ChartConversionRefusal> {
        match self {
            Self::AtLeastTwo if series < 2 => Some(ChartConversionRefusal::NotValidForKind),
            Self::Exactly(required) if series != usize::from(required) => {
                Some(ChartConversionRefusal::SeriesCountMismatch {
                    required,
                    actual: u8::try_from(series).unwrap_or(u8::MAX),
                })
            }
            Self::Any | Self::AtLeastTwo | Self::Exactly(_) => None,
        }
    }
}

/// Chart `chart` of the sheet at `sheet`, once the sheet is read. An anchor
/// without a graphic frame holds no chart.
fn chart_at(workbook: &Workbook, sheet: usize, chart: usize) -> Option<&Chart> {
    let frame = workbook
        .sheet(sheet)
        .ok()?
        .chart_collection()
        .get(chart)?
        .two_cell_anchor()
        .graphic_frame()?;
    Some(frame.graphic().graphic_data().chart_space().chart())
}

/// The series of every plot group together.
fn series_count(plot_area: &PlotArea) -> usize {
    plot_area
        .chart_groups()
        .map(|group| group.series().area_chart_series().len())
        .sum()
}

impl Workbook {
    /// Converts chart `chart` of the sheet at `sheet` (both counted from zero)
    /// to `target` the way Excel's "Change Chart Type" does and reports what
    /// changed. On `Err` the chart is unchanged.
    ///
    /// The conversion reads the cells the chart's references name, as Excel
    /// reads them; see [`ChartConversionRefusal`] for the order in which the
    /// checks apply. A sheet of a lazily read workbook is read when the chart
    /// is on it or reads its cells. A chart that already is `target` is
    /// returned unchanged with an empty report. A pair whose result Excel was
    /// not measured to give, or that this version does not build yet, is
    /// refused as [`ChartConversionRefusal::Unsupported`]; so is a chart whose
    /// references umya cannot read: a zero or reversed range, or a sheet that
    /// cannot be read.
    pub fn convert_chart_type(
        &mut self,
        sheet: usize,
        chart: usize,
        target: ChartSubtype,
    ) -> Result<ChartConversion, ChartConversionRefusal> {
        self.try_read_sheet(sheet)
            .map_err(|_| ChartConversionRefusal::ChartNotFound)?;
        let Some(plan) = plan::plan(self, sheet, chart, target)? else {
            return Ok(ChartConversion::unchanged(target));
        };
        let conversion = ChartConversion::new(&plan);
        // The plan holds the result, written and compared already.
        *self
            .sheet_mut(sheet)
            .ok()
            .and_then(|found| found.chart_collection_mut().get_mut(chart))
            .and_then(|found| found.two_cell_anchor_mut().graphic_frame_mut())
            .ok_or(ChartConversionRefusal::ChartNotFound)?
            .graphic_mut()
            .graphic_data_mut()
            .chart_space_mut()
            .chart_mut() = plan.chart;
        Ok(conversion)
    }
}
