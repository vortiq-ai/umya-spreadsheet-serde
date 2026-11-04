// Which series children each plot group's series type allows.
use crate::structs::ChartType;

/// The series type a plot group writes its `c:ser` elements as.
///
/// Doughnut and of-pie groups use the pie series type; every supported 3D
/// group uses the series type of its flat counterpart; stock groups use the
/// line series type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SeriesFamily {
    Bar,
    Line,
    Area,
    Pie,
    Scatter,
    Bubble,
    Radar,
    /// `CT_SurfaceSer`: only `c:cat` and `c:val` besides the shared children.
    Surface,
}

impl From<ChartType> for SeriesFamily {
    fn from(value: ChartType) -> Self {
        Self::from(&value)
    }
}

impl From<&ChartType> for SeriesFamily {
    fn from(value: &ChartType) -> Self {
        match value {
            ChartType::BarChart | ChartType::Bar3DChart => Self::Bar,
            ChartType::LineChart | ChartType::Line3DChart | ChartType::StockChart => Self::Line,
            ChartType::AreaChart | ChartType::Area3DChart => Self::Area,
            ChartType::PieChart
            | ChartType::Pie3DChart
            | ChartType::DoughnutChart
            | ChartType::OfPieChart => Self::Pie,
            ChartType::ScatterChart => Self::Scatter,
            ChartType::BubbleChart => Self::Bubble,
            ChartType::RadarChart => Self::Radar,
            ChartType::SurfaceChart | ChartType::Surface3DChart => Self::Surface,
        }
    }
}

/// A series child whose legality depends on the series type.
///
/// Children every series type allows (`c:idx`, `c:order`, `c:tx`, `c:spPr`,
/// `c:dLbls`) are not listed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SeriesPartKind {
    Explosion,
    InvertIfNegative,
    Marker,
    Categories,
    Values,
    XValues,
    YValues,
    BubbleSize,
    Bubble3D,
    Smooth,
    /// A bar series' column shape.
    Shape,
    /// Picture-fill stacking on bar and area series.
    PictureOptions,
    /// Ordered point overrides, legal in every family but surface.
    DataPoints,
    /// Ordered trendlines and their labels.
    Trendlines,
    /// Group and point data label content, legal in every family but surface.
    DataLabels,
    /// Ordered native extension occurrences.
    Extensions,
    /// Ordered error-bar occurrences with family-specific cardinality.
    ErrorBars,
}

impl SeriesPartKind {
    /// Family-dependent kinds, in the order the series types write them.
    pub(crate) const ALL: [Self; 12] = [
        Self::Explosion,
        Self::InvertIfNegative,
        Self::Marker,
        Self::Categories,
        Self::Values,
        Self::XValues,
        Self::YValues,
        Self::BubbleSize,
        Self::Bubble3D,
        Self::Smooth,
        Self::Shape,
        Self::PictureOptions,
    ];
}

impl SeriesFamily {
    /// Whether the family's series type has a child of this kind.
    #[must_use]
    pub fn allows(self, part: SeriesPartKind) -> bool {
        use SeriesPartKind::{
            Bubble3D,
            BubbleSize,
            Categories,
            Explosion,
            InvertIfNegative,
            Marker,
            PictureOptions,
            Shape,
            Smooth,
            Values,
            XValues,
            YValues,
        };
        if part == SeriesPartKind::ErrorBars {
            return super::error_bars::error_bar_limit(self) > 0;
        }
        if part == SeriesPartKind::Trendlines {
            return !matches!(self, Self::Pie | Self::Radar | Self::Surface);
        }
        if part == SeriesPartKind::Extensions {
            return true;
        }
        if matches!(
            part,
            SeriesPartKind::DataPoints | SeriesPartKind::DataLabels
        ) {
            return self != Self::Surface;
        }
        match self {
            Self::Bar => matches!(
                part,
                InvertIfNegative | Categories | Values | Shape | PictureOptions
            ),
            Self::Line => matches!(part, Marker | Categories | Values | Smooth),
            Self::Area => matches!(part, Categories | Values | PictureOptions),
            Self::Pie => matches!(part, Explosion | Categories | Values),
            Self::Scatter => matches!(part, Marker | XValues | YValues | Smooth),
            Self::Bubble => matches!(
                part,
                InvertIfNegative | XValues | YValues | BubbleSize | Bubble3D
            ),
            Self::Radar => matches!(part, Marker | Categories | Values),
            Self::Surface => matches!(part, Categories | Values),
        }
    }
}

/// Why a stored series part cannot be written.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SeriesPartConflictReason {
    /// The series type has no such child.
    NotAllowed,
    /// The series type allows fewer occurrences than are stored.
    TooMany,
    /// A child the series type requires is missing.
    MissingRequiredChild,
    /// Both branches of a choice are stored.
    ConflictingChoice,
    /// A stored value is outside what the schema allows.
    InvalidValue,
}

/// A stored series part the writer leaves out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SeriesPartConflict {
    /// Position of the plot group in `PlotArea::chart_groups()` order.
    pub group_index:  usize,
    /// Position of the series in its plot group.
    pub series_index: usize,
    pub part:         SeriesPartKind,
    /// Zero-based position among the occurrences of `part` in the series.
    pub occurrence:   usize,
    pub reason:       SeriesPartConflictReason,
}

/// A series part removed by an explicit change of series type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SeriesPartRemoval {
    /// Position of the series in its plot group.
    pub series_index: usize,
    pub part:         SeriesPartKind,
    /// Zero-based position among the occurrences of `part` in the series.
    pub occurrence:   usize,
}
