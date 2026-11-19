//! The columns of the conversion table: the classes of a before value that
//! select a rule cell, and the slots a conversion can change.
//!
//! A cell is selected by the class of the value the chart holds before the
//! conversion. Every selector is exhaustive, so an unmeasured class is a
//! [`RuleCell::Rejected`] cell, never a default.

use super::{
    AxisRole,
    ConvertedPart,
};
use crate::structs::drawing::charts::SeriesPartKind;

/// A rule measured for one class of before value, or no measurement for it.
#[derive(Clone, Copy, Debug)]
pub(super) enum RuleCell<T> {
    Measured(T),
    /// Excel was never seen converting this class, or its observations
    /// disagree.
    Rejected,
}

/// Whether the before value is there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Presence {
    Absent,
    Present,
}

/// Whether the series has shape properties of its own (`c:ser/c:spPr`), even
/// empty ones.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SeriesStyleClass {
    Automatic,
    Styled,
}

/// The colour of a fill or an outline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ColorClass {
    Absent,
    NoFill,
    /// Exactly the palette colour Excel generates at the series' or point's
    /// index.
    SeriesAccent,
    Explicit,
}

/// A series' marker symbol.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MarkerClass {
    Absent,
    /// The symbol `none`.
    Hidden,
    Shown,
}

/// The type of the axis that holds a role before the conversion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum AxisClass {
    Absent,
    Category,
    Date,
    Value,
    Series,
}

/// What a series is in its source chart.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SourceSeriesRole {
    Ordinary,
    /// The first series of a surface chart, which owns the bands.
    SurfaceFirst,
    SurfaceRemaining,
    /// The column series of a volume stock chart.
    Volume,
    /// A price series of a volume stock chart.
    Stock,
}

/// The plot group that holds a series in its source chart.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SourceGroupClass {
    /// A chart with one plot group, or a volume stock chart.
    Single,
    ComboArea,
    ComboBar,
    ComboLine,
}

/// A series' own `c:smooth` value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SmoothFlag {
    Absent,
    Off,
    On,
}

/// How many values a series has, after its data roles are remapped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ValueCountClass {
    Six,
    Seven,
    Twelve,
    Sixteen,
    TwentyFour,
    Fifty,
    Other,
}

/// What the categories hold, after the data roles are remapped: moved x values
/// are categories too.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum CategoryDataClass {
    Absent,
    TextReference,
    /// Numbers formatted `yyyy\-mm\-dd`.
    DateSerialReference,
    /// Numbers formatted `General`.
    GeneralReference,
    Other,
}

/// The spacing of numeric categories.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DateSpacing {
    /// Text or absent categories.
    Absent,
    /// Consecutive serials; `General` numbers only as exactly 1 to N.
    Daily,
    /// The first day of consecutive months.
    Monthly,
    /// Any other distinct whole days whose smallest gap is under 28 days:
    /// Excel gives such dates a base unit of days.
    Other,
    /// Any other whole days whose smallest gap is 28 days or more, which
    /// Excel gives months or years, or that repeat a day, which no
    /// measurement covers.
    Wide,
}

/// What a series' or a point's 3-D shape (`a:sp3d`) holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Shape3dClass {
    Absent,
    /// No attributes and no children.
    Empty,
    Explicit,
}

/// Whether a created date axis serves monthly categories.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CreationClass {
    Ordinary,
    Monthly,
}

#[derive(Debug)]
pub(super) struct ByPresence<T> {
    pub(super) absent:  RuleCell<T>,
    pub(super) present: RuleCell<T>,
}

impl<T> ByPresence<T> {
    pub(super) const fn get(&self, class: Presence) -> &RuleCell<T> {
        match class {
            Presence::Absent => &self.absent,
            Presence::Present => &self.present,
        }
    }
}

#[derive(Debug)]
pub(super) struct ByShape3d<T> {
    pub(super) absent:   RuleCell<T>,
    pub(super) empty:    RuleCell<T>,
    pub(super) explicit: RuleCell<T>,
}

impl<T> ByShape3d<T> {
    pub(super) const fn get(&self, class: Shape3dClass) -> &RuleCell<T> {
        match class {
            Shape3dClass::Absent => &self.absent,
            Shape3dClass::Empty => &self.empty,
            Shape3dClass::Explicit => &self.explicit,
        }
    }
}

#[derive(Debug)]
pub(super) struct BySeriesStyle<T> {
    pub(super) automatic: T,
    pub(super) styled:    T,
}

impl<T> BySeriesStyle<T> {
    pub(super) const fn get(&self, class: SeriesStyleClass) -> &T {
        match class {
            SeriesStyleClass::Automatic => &self.automatic,
            SeriesStyleClass::Styled => &self.styled,
        }
    }
}

#[derive(Debug)]
pub(super) struct ByColor<T> {
    pub(super) absent:        RuleCell<T>,
    pub(super) no_fill:       RuleCell<T>,
    pub(super) series_accent: RuleCell<T>,
    pub(super) explicit:      RuleCell<T>,
}

impl<T> ByColor<T> {
    pub(super) const fn get(&self, class: ColorClass) -> &RuleCell<T> {
        match class {
            ColorClass::Absent => &self.absent,
            ColorClass::NoFill => &self.no_fill,
            ColorClass::SeriesAccent => &self.series_accent,
            ColorClass::Explicit => &self.explicit,
        }
    }
}

#[derive(Debug)]
pub(super) struct ByMarker<T> {
    pub(super) absent: RuleCell<T>,
    pub(super) hidden: RuleCell<T>,
    pub(super) shown:  RuleCell<T>,
}

impl<T> ByMarker<T> {
    pub(super) const fn get(&self, class: MarkerClass) -> &RuleCell<T> {
        match class {
            MarkerClass::Absent => &self.absent,
            MarkerClass::Hidden => &self.hidden,
            MarkerClass::Shown => &self.shown,
        }
    }
}

#[derive(Debug)]
pub(super) struct ByAxis<T> {
    pub(super) absent:   RuleCell<T>,
    pub(super) category: RuleCell<T>,
    pub(super) date:     RuleCell<T>,
    pub(super) value:    RuleCell<T>,
    pub(super) series:   RuleCell<T>,
}

impl<T> ByAxis<T> {
    pub(super) const fn get(&self, class: AxisClass) -> &RuleCell<T> {
        match class {
            AxisClass::Absent => &self.absent,
            AxisClass::Category => &self.category,
            AxisClass::Date => &self.date,
            AxisClass::Value => &self.value,
            AxisClass::Series => &self.series,
        }
    }
}

#[derive(Debug)]
pub(super) struct BySourceRole<T> {
    pub(super) ordinary:          T,
    pub(super) surface_first:     T,
    pub(super) surface_remaining: T,
    pub(super) volume:            T,
    pub(super) stock:             T,
}

impl<T> BySourceRole<T> {
    pub(super) const fn get(&self, class: SourceSeriesRole) -> &T {
        match class {
            SourceSeriesRole::Ordinary => &self.ordinary,
            SourceSeriesRole::SurfaceFirst => &self.surface_first,
            SourceSeriesRole::SurfaceRemaining => &self.surface_remaining,
            SourceSeriesRole::Volume => &self.volume,
            SourceSeriesRole::Stock => &self.stock,
        }
    }
}

#[derive(Debug)]
pub(super) struct BySmoothFlag<T> {
    pub(super) absent: RuleCell<T>,
    pub(super) off:    RuleCell<T>,
    pub(super) on:     RuleCell<T>,
}

impl<T> BySmoothFlag<T> {
    pub(super) const fn get(&self, class: SmoothFlag) -> &RuleCell<T> {
        match class {
            SmoothFlag::Absent => &self.absent,
            SmoothFlag::Off => &self.off,
            SmoothFlag::On => &self.on,
        }
    }
}

/// A combination series is selected by its source group and its own flag; any
/// other series only by whether it has the flag.
#[derive(Debug)]
pub(super) struct BySourceGroup<T> {
    pub(super) single:     ByPresence<T>,
    pub(super) combo_area: BySmoothFlag<T>,
    pub(super) combo_bar:  BySmoothFlag<T>,
    pub(super) combo_line: BySmoothFlag<T>,
}

impl<T> BySourceGroup<T> {
    pub(super) const fn get(&self, group: SourceGroupClass, flag: SmoothFlag) -> &RuleCell<T> {
        match group {
            SourceGroupClass::Single => self.single.get(match flag {
                SmoothFlag::Absent => Presence::Absent,
                SmoothFlag::Off | SmoothFlag::On => Presence::Present,
            }),
            SourceGroupClass::ComboArea => self.combo_area.get(flag),
            SourceGroupClass::ComboBar => self.combo_bar.get(flag),
            SourceGroupClass::ComboLine => self.combo_line.get(flag),
        }
    }
}

#[derive(Debug)]
pub(super) struct ByValueCount<T> {
    pub(super) six:         T,
    pub(super) seven:       T,
    pub(super) twelve:      T,
    pub(super) sixteen:     T,
    pub(super) twenty_four: T,
    pub(super) fifty:       T,
    pub(super) other:       T,
}

impl<T> ByValueCount<T> {
    pub(super) const fn get(&self, class: ValueCountClass) -> &T {
        match class {
            ValueCountClass::Six => &self.six,
            ValueCountClass::Seven => &self.seven,
            ValueCountClass::Twelve => &self.twelve,
            ValueCountClass::Sixteen => &self.sixteen,
            ValueCountClass::TwentyFour => &self.twenty_four,
            ValueCountClass::Fifty => &self.fifty,
            ValueCountClass::Other => &self.other,
        }
    }
}

#[derive(Debug)]
pub(super) struct ByDateSpacing<T> {
    pub(super) absent:  RuleCell<T>,
    pub(super) daily:   RuleCell<T>,
    pub(super) monthly: RuleCell<T>,
    pub(super) other:   RuleCell<T>,
    pub(super) wide:    RuleCell<T>,
}

impl<T> ByDateSpacing<T> {
    pub(super) const fn get(&self, class: DateSpacing) -> &RuleCell<T> {
        match class {
            DateSpacing::Absent => &self.absent,
            DateSpacing::Daily => &self.daily,
            DateSpacing::Monthly => &self.monthly,
            DateSpacing::Other => &self.other,
            DateSpacing::Wide => &self.wide,
        }
    }
}

#[derive(Debug)]
pub(super) struct ByCategoryData<T> {
    pub(super) absent:      RuleCell<T>,
    pub(super) text:        RuleCell<T>,
    pub(super) date_serial: RuleCell<T>,
    pub(super) general:     RuleCell<T>,
    pub(super) other:       RuleCell<T>,
}

impl<T> ByCategoryData<T> {
    pub(super) const fn get(&self, class: CategoryDataClass) -> &RuleCell<T> {
        match class {
            CategoryDataClass::Absent => &self.absent,
            CategoryDataClass::TextReference => &self.text,
            CategoryDataClass::DateSerialReference => &self.date_serial,
            CategoryDataClass::GeneralReference => &self.general,
            CategoryDataClass::Other => &self.other,
        }
    }
}

/// The monthly cell applies only when the created axis is a date axis.
#[derive(Debug)]
pub(super) struct ByMonthlyCreation<T> {
    pub(super) ordinary: T,
    pub(super) monthly:  T,
}

impl<T> ByMonthlyCreation<T> {
    pub(super) const fn get(&self, class: CreationClass) -> &T {
        match class {
            CreationClass::Ordinary => &self.ordinary,
            CreationClass::Monthly => &self.monthly,
        }
    }
}

/// One comparable value of a chart: a column of the table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Slot {
    Chart(ChartSlot),
    Scene(SceneSlot),
    Group(GroupSlot),
    Axis {
        role:      AxisRole,
        secondary: bool,
        slot:      AxisSlot,
    },
    Series(SeriesSlot),
    Point(PointSlot),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ChartSlot {
    Title,
    Legend,
    AutoTitleDeleted,
    PlotVisibleOnly,
    DisplayBlanksAs,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SceneSlot {
    View,
    RotationX,
    RotationY,
    RightAngleAxes,
    Perspective,
    Floor,
    SideWall,
    BackWall,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum GroupSlot {
    Type,
    BarDirection,
    Grouping,
    VaryColors,
    GapWidth,
    Overlap,
    FirstSliceAngle,
    HoleSize,
    BubbleScale,
    ShowNegativeBubbles,
    SizeRepresents,
    ScatterStyle,
    RadarStyle,
    Wireframe,
    Bubble3D,
    Shape,
    GapDepth,
    ShowMarker,
    Smooth,
    HighLowLines,
    UpDownBars,
    DropLines,
    SeriesLines,
    BandFormats,
    DataLabels,
    OfPieType,
    SecondPieSize,
    /// The bands' own shape properties (`c:bandFmt`).
    Bands,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum AxisSlot {
    Type,
    Orientation,
    Minimum,
    Maximum,
    LogBase,
    Deleted,
    Position,
    NumberFormat,
    MajorUnit,
    MinorUnit,
    Crosses,
    CrossesAt,
    CrossBetween,
    TickLabels,
    MajorGrid,
    Title,
    Auto,
    LabelAlign,
    LabelOffset,
    NoMultiLevel,
    BaseTimeUnit,
    MajorTicks,
    MinorTicks,
    Style,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SeriesSlot {
    Order,
    Text,
    Categories,
    Values,
    XValues,
    YValues,
    BubbleSize,
    Fill,
    Line,
    LineWidth,
    LineDash,
    LineCap,
    LineJoin,
    Marker,
    MarkerSymbol,
    MarkerSize,
    MarkerFill,
    MarkerLine,
    MarkerWidth,
    MarkerDash,
    Smooth,
    DataLabels,
    DataLabelFormat,
    DataLabelPosition,
    Trendlines,
    ErrorBars,
    InvertIfNegative,
    Explosion,
    Bubble3D,
    Shape,
    DataPoints,
}

/// A data point's own formatting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PointSlot {
    Fill,
    Line,
    LineWidth,
    LineDash,
    Bubble3D,
}

impl Slot {
    /// The part the report names for a change to this slot. `None` for the
    /// chart title, legend and chart flags and for a series' order and
    /// name, which every measured conversion keeps.
    pub(super) const fn part(self) -> Option<ConvertedPart> {
        let kind = match self {
            Self::Chart(_) | Self::Series(SeriesSlot::Order | SeriesSlot::Text) => return None,
            Self::Scene(_) => return Some(ConvertedPart::Scene),
            Self::Group(_) => return Some(ConvertedPart::Group),
            Self::Axis {
                role, secondary, ..
            } => return Some(ConvertedPart::Axis { role, secondary }),
            Self::Series(SeriesSlot::Fill) => return Some(ConvertedPart::SeriesFill),
            Self::Series(
                SeriesSlot::Line
                | SeriesSlot::LineWidth
                | SeriesSlot::LineDash
                | SeriesSlot::LineCap
                | SeriesSlot::LineJoin,
            ) => return Some(ConvertedPart::SeriesLine),
            Self::Series(SeriesSlot::Categories) => SeriesPartKind::Categories,
            Self::Series(SeriesSlot::Values) => SeriesPartKind::Values,
            Self::Series(SeriesSlot::XValues) => SeriesPartKind::XValues,
            Self::Series(SeriesSlot::YValues) => SeriesPartKind::YValues,
            Self::Series(SeriesSlot::BubbleSize) => SeriesPartKind::BubbleSize,
            Self::Series(
                SeriesSlot::Marker
                | SeriesSlot::MarkerSymbol
                | SeriesSlot::MarkerSize
                | SeriesSlot::MarkerFill
                | SeriesSlot::MarkerLine
                | SeriesSlot::MarkerWidth
                | SeriesSlot::MarkerDash,
            ) => SeriesPartKind::Marker,
            Self::Series(SeriesSlot::Smooth) => SeriesPartKind::Smooth,
            Self::Series(
                SeriesSlot::DataLabels
                | SeriesSlot::DataLabelFormat
                | SeriesSlot::DataLabelPosition,
            ) => SeriesPartKind::DataLabels,
            Self::Series(SeriesSlot::Trendlines) => SeriesPartKind::Trendlines,
            Self::Series(SeriesSlot::ErrorBars) => SeriesPartKind::ErrorBars,
            Self::Series(SeriesSlot::InvertIfNegative) => SeriesPartKind::InvertIfNegative,
            Self::Series(SeriesSlot::Explosion) => SeriesPartKind::Explosion,
            Self::Series(SeriesSlot::Bubble3D) => SeriesPartKind::Bubble3D,
            Self::Series(SeriesSlot::Shape) => SeriesPartKind::Shape,
            Self::Series(SeriesSlot::DataPoints) | Self::Point(_) => SeriesPartKind::DataPoints,
        };
        Some(ConvertedPart::SeriesPart(kind))
    }
}
