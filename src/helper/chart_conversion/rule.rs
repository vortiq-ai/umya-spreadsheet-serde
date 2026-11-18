//! The schema of the conversion table, which the generated `rules.rs` fills
//! from Excel's measured conversions.
//!
//! A KIND rule belongs to a target subtype and a PAIR rule to an ordered pair
//! of source and target. Each cell holds the action Excel was measured to take
//! for one class of before value. Where the generator could not measure a class
//! it writes [`RuleCell::Rejected`], and the conversion refuses before it
//! touches the chart.

use super::{
    AxisRole,
    ChartSource,
    ChartSubtype,
    PaintedPart,
    slots::{
        ByAxis,
        ByCategoryData,
        ByColor,
        ByDateSpacing,
        ByMarker,
        ByMonthlyCreation,
        ByPresence,
        BySeriesStyle,
        ByShape3d,
        BySourceGroup,
        BySourceRole,
        ByValueCount,
    },
};
use crate::structs::{
    ChartType,
    drawing::{
        SchemeColorValues,
        charts::ErrorBarDirection,
    },
};

impl ChartSource {
    /// The rows of the pair table: every subtype, then the four combination
    /// layouts.
    pub(super) const COUNT: usize = ChartSubtype::COUNT + 4;
}

/// A value the conversion keeps, removes or sets.
#[derive(Clone, Copy, Debug)]
pub(super) enum Carried<T: Copy> {
    Keep,
    Remove,
    Set(T),
}

/// A value the conversion only keeps or removes.
#[derive(Clone, Copy, Debug)]
pub(super) enum Retain {
    Keep,
    Remove,
}

/// A colour: a fill, an outline or a marker's.
#[derive(Clone, Copy, Debug)]
pub(super) enum ColorAction {
    Keep,
    Remove,
    /// The palette colour at the series' index, keeping the alpha of the colour
    /// it replaces.
    SeriesAccent,
    /// The palette colour at the series' index, with this alpha.
    SeriesAccentWithAlpha(u32),
    /// The palette colour at the point's own index, keeping the alpha of the
    /// colour it replaces.
    PointAccent,
    Set(&'static Paint),
    /// Another colour of the same series or point. An absent or unfilled one
    /// gives the palette colour instead.
    CopyFrom(ColorSlot),
    /// A point's colour becomes its series' colour of the same part.
    FromSeries,
}

/// An exact colour.
#[derive(Clone, Debug)]
pub(super) enum Paint {
    NoFill,
    Scheme {
        value:      SchemeColorValues,
        transforms: &'static [PaintTransform],
    },
    Rgb {
        value:      &'static str,
        transforms: &'static [PaintTransform],
    },
}

/// A colour transform, in the order it is written.
#[derive(Clone, Copy, Debug)]
pub(super) enum PaintTransform {
    LuminanceMod(u32),
    LuminanceOff(u32),
    Alpha(u32),
}

#[derive(Clone, Copy, Debug)]
pub(super) enum ColorSlot {
    Fill,
    Line,
    MarkerFill,
    MarkerLine,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum DashAction {
    Keep,
    Remove,
    CopyFrom(DashSlot),
}

#[derive(Clone, Copy, Debug)]
pub(super) enum DashSlot {
    Line,
    MarkerLine,
}

/// An outline width in EMU.
#[derive(Clone, Copy, Debug)]
pub(super) enum WidthRule {
    Keep,
    Remove,
    Set(u32),
    /// Keeps a width that is there, and writes this one where there is none.
    KeepOrSet(u32),
    /// A point's width becomes its series' width.
    FromSeries,
}

/// A data role: categories, values, x or y values, bubble sizes.
#[derive(Clone, Copy, Debug)]
pub(super) enum RoleAction {
    Keep,
    Remove,
    /// The role takes over the content of another role, which is gone
    /// afterwards.
    MoveFrom(DataRole),
    /// A new bubble-size literal with a `1` for every value.
    OnesPerPoint,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum DataRole {
    Categories,
    Values,
    XValues,
    YValues,
    BubbleSize,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum SliceAngle {
    Keep,
    Remove,
    Set(u16),
    FromSceneRotationY,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum RotationY {
    Keep,
    Remove,
    Set(u16),
    FromFirstSliceAngle,
}

/// A number format, `formatCode` with its `sourceLinked` flag.
#[derive(Clone, Copy, Debug)]
pub(super) enum NumberFormatRule {
    Keep,
    Remove,
    /// Keeps a format that is there, and writes `General`, source linked, where
    /// there is none.
    KeepOrGeneralLinked,
    /// Writes this code and keeps the format's own `sourceLinked`.
    SetCodeKeepLink(&'static str),
    Set {
        code:          &'static str,
        source_linked: bool,
    },
}

#[derive(Clone, Copy, Debug)]
pub(super) enum ErrorBarRule {
    Keep,
    Remove,
    /// Keeps every error bar and gives it this direction, or none.
    SetDirection(Option<ErrorBarDirection>),
}

/// The bands of a surface plot group.
#[derive(Clone, Copy, Debug)]
pub(super) enum BandRule {
    Keep,
    Remove,
    /// One band for each point override of a varied source's first series.
    FromPoints,
    /// The converted first series' style repeated over `bands` bands, each
    /// with its point's colour. A wireframe target puts that colour in the
    /// outline. Where the style holds the generated accent of the first
    /// series, band `k` takes the accent of index `k` instead.
    FromFirstSeries {
        bands: usize,
    },
    Set(&'static [BandFormat]),
}

/// A series' data points.
#[derive(Clone, Copy, Debug)]
pub(super) enum DataPointsRule {
    Keep,
    Remove,
    /// Keeps the points and adds one for every other value.
    CompletePerValue(&'static PointFormat),
    /// As `CompletePerValue`, with one more point for the of-pie aggregate.
    CompleteWithAggregate(&'static PointFormat),
    /// One point for every band of the surface source.
    FromSurfaceBands(&'static BandPointFormat),
}

#[derive(Clone, Copy, Debug)]
pub(super) enum AxisOutcome {
    Absent,
    /// A new axis from the pair's measured template.
    Created,
    Present(&'static AxisSpec),
}

#[derive(Clone, Copy, Debug)]
pub(super) enum SeriesCount {
    Any,
    AtLeast(u8),
    Exactly(u8),
}

/// The series a plot group holds.
#[derive(Clone, Copy, Debug)]
pub(super) enum SeriesSelection {
    All,
    First,
    AfterFirst,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum AxisSet {
    Unattached,
    Primary,
    Secondary,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum PairRule {
    Same,
    Unsupported,
    Convert(&'static Conversion),
    /// A restricted source converts by its exact series and value count.
    ConvertByShape(&'static ShapeRules),
}

#[derive(Clone, Copy, Debug)]
pub(super) enum AxisType {
    Category,
    Date,
    Value,
    Series,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum Orientation {
    MaxMin,
    MinMax,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum AxisPosition {
    Bottom,
    Left,
    Right,
    Top,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum TickLabelPosition {
    High,
    Low,
    NextTo,
    None,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum TickMark {
    Cross,
    Inside,
    None,
    Outside,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum CrossBetween {
    Between,
    MidpointCategory,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum LabelAlignment {
    Center,
    Left,
    Right,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum TimeUnit {
    Days,
    Months,
    Years,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum BarDirection {
    Bar,
    Column,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum Grouping {
    Clustered,
    PercentStacked,
    Stacked,
    Standard,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum BarShape {
    Box,
    Cone,
    ConeToMax,
    Cylinder,
    Pyramid,
    PyramidToMax,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum OfPieKind {
    Bar,
    Pie,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum ScatterStyle {
    Line,
    LineMarker,
    Marker,
    None,
    Smooth,
    SmoothMarker,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum RadarStyle {
    Filled,
    Marker,
    Standard,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum MarkerSymbol {
    Auto,
    Circle,
    Dash,
    Diamond,
    Dot,
    None,
    Picture,
    Plus,
    Square,
    Star,
    Triangle,
    X,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum LineCap {
    Flat,
    Round,
    Square,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum LineJoin {
    Bevel,
    Miter,
    Round,
}

/// A series' or a point's 3-D shape (`a:sp3d`).
#[derive(Clone, Copy, Debug)]
pub(super) enum Shape3dRule {
    Keep,
    Remove,
    /// A contour from the owner's outline: its width and colour, each written
    /// only when the outline is a solid colour.
    ContourFromLine,
    /// A point's shape becomes its converted series' shape.
    FromSeries,
}

/// A point's tree or setting.
#[derive(Clone, Copy, Debug)]
pub(super) enum PointValue<T: Copy> {
    Keep,
    Remove,
    Set(T),
    /// The converted series' value.
    FromSeries,
}

/// What every conversion into one target subtype does.
#[derive(Debug)]
pub(super) struct KindRule {
    pub(super) chart:        ChartRules,
    /// One group; two for volume stock.
    pub(super) groups:       &'static [GroupRule],
    /// Keyed by role and axis set.
    pub(super) axes:         &'static [AxisRule],
    pub(super) series_count: SeriesCount,
    pub(super) data:         DataRules,
    pub(super) parts:        SeriesPartRules,
    /// One entry for every series, or one for each stock position.
    pub(super) paint:        &'static [PaintedPart],
}

#[derive(Debug)]
pub(super) struct ChartRules {
    pub(super) title:              ByPresence<Retain>,
    pub(super) legend:             ByPresence<Retain>,
    pub(super) auto_title_deleted: ByPresence<Retain>,
    pub(super) plot_visible_only:  ByPresence<Retain>,
    pub(super) display_blanks_as:  ByPresence<Retain>,
}

/// A target plot group. Its cells are selected by whether the source group at
/// the same position holds the value.
#[derive(Debug)]
pub(super) struct GroupRule {
    pub(super) chart_type:            ChartType,
    pub(super) series:                SeriesSelection,
    pub(super) axes:                  AxisSet,
    /// Excel writes one more axis id, `0`, after the group's real ones.
    pub(super) axis_id_placeholder:   bool,
    pub(super) bar_direction:         ByPresence<Carried<BarDirection>>,
    pub(super) grouping:              ByPresence<Carried<Grouping>>,
    pub(super) vary_colors:           ByPresence<Carried<bool>>,
    pub(super) overlap:               ByPresence<Carried<i8>>,
    pub(super) hole_size:             ByPresence<Carried<u8>>,
    pub(super) bubble_scale:          ByPresence<Carried<u16>>,
    pub(super) show_negative_bubbles: ByPresence<Carried<bool>>,
    pub(super) size_represents:       ByPresence<Retain>,
    pub(super) scatter_style:         ByPresence<Carried<ScatterStyle>>,
    pub(super) radar_style:           ByPresence<Carried<RadarStyle>>,
    pub(super) wireframe:             ByPresence<Carried<bool>>,
    pub(super) bubble_3d:             ByPresence<Retain>,
    pub(super) shape:                 ByPresence<Carried<BarShape>>,
    pub(super) gap_depth:             ByPresence<Retain>,
    pub(super) show_marker:           ByPresence<Carried<bool>>,
    pub(super) smooth:                ByPresence<Carried<bool>>,
    pub(super) drop_lines:            ByPresence<Retain>,
    pub(super) band_formats:          ByPresence<Carried<()>>,
    pub(super) of_pie_type:           ByPresence<Carried<OfPieKind>>,
    pub(super) second_pie_size:       ByPresence<Carried<u16>>,
}

#[derive(Debug)]
pub(super) struct AxisRule {
    pub(super) key:     AxisKey,
    /// By the type of the axis that held the role before.
    pub(super) outcome: ByAxis<AxisOutcome>,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct AxisKey {
    pub(super) role: AxisRole,
    pub(super) set:  AxisSet,
}

/// An existing axis. Category data and spacing select the category role's type,
/// alignment, multi-level labels and base time unit; the other roles hold one
/// rule in every one of those cells.
#[derive(Debug)]
pub(super) struct AxisSpec {
    pub(super) axis_type:      ByCategoryData<ByDateSpacing<AxisType>>,
    pub(super) orientation:    Carried<Orientation>,
    pub(super) minimum:        Retain,
    pub(super) maximum:        Retain,
    pub(super) log_base:       Retain,
    pub(super) deleted:        Carried<bool>,
    pub(super) position:       Carried<AxisPosition>,
    pub(super) major_unit:     Retain,
    pub(super) minor_unit:     Retain,
    pub(super) crosses:        Retain,
    pub(super) crosses_at:     Retain,
    pub(super) cross_between:  Carried<CrossBetween>,
    /// `None` for the value role, whose tick labels the pair governs.
    pub(super) tick_labels:    Option<Carried<TickLabelPosition>>,
    pub(super) title:          Retain,
    pub(super) auto:           Carried<bool>,
    pub(super) label_align:    ByCategoryData<ByDateSpacing<Carried<LabelAlignment>>>,
    pub(super) label_offset:   Carried<u16>,
    pub(super) no_multi_level: ByCategoryData<ByDateSpacing<Carried<bool>>>,
    pub(super) base_time_unit: ByCategoryData<ByDateSpacing<Carried<TimeUnit>>>,
    pub(super) major_ticks:    Carried<TickMark>,
    pub(super) minor_ticks:    Carried<TickMark>,
    pub(super) style:          Retain,
}

#[derive(Debug)]
pub(super) struct DataRules {
    pub(super) categories:  ByPresence<RoleAction>,
    pub(super) values:      ByPresence<RoleAction>,
    pub(super) x_values:    ByPresence<RoleAction>,
    pub(super) y_values:    ByPresence<RoleAction>,
    pub(super) bubble_size: ByPresence<RoleAction>,
}

#[derive(Debug)]
pub(super) struct SeriesPartRules {
    pub(super) order:               ByPresence<Retain>,
    pub(super) text:                ByPresence<Retain>,
    pub(super) data_labels:         ByPresence<Retain>,
    pub(super) data_label_position: ByPresence<Retain>,
    pub(super) trendlines:          ByPresence<Retain>,
    pub(super) error_bars:          ByPresence<ErrorBarRule>,
    pub(super) explosion:           ByPresence<Carried<u16>>,
    pub(super) bubble_3d:           ByPresence<Carried<bool>>,
    pub(super) shape:               ByPresence<Retain>,
    /// A series outline left with no colour, width, dash, cap or join, by
    /// whether it held one before: `Keep` leaves an empty `a:ln`. A point's or
    /// a marker's outline that the conversion empties always goes.
    pub(super) empty_line:          ByPresence<Retain>,
}

/// What a conversion from one source into one target does beyond its KIND rule.
#[derive(Debug)]
pub(super) struct Conversion {
    /// By the chart's style: a varied source counts as styled only as one
    /// series with points.
    pub(super) scene:    BySeriesStyle<&'static SceneRules>,
    /// By the class of the source's x values: measured only for the classes
    /// Excel converted; a source without x values selects nothing.
    pub(super) x_values: ByCategoryData<ByDateSpacing<()>>,
    /// One entry for each target group.
    pub(super) groups:   &'static [GroupValuesRule],
    /// Keyed like the KIND rule's axes.
    pub(super) axes:     &'static [AxisPairRule],
    /// One entry for every series, or one for each stock position.
    pub(super) series:   &'static [BySeriesStyle<&'static SeriesStyle>],
}

#[derive(Debug)]
pub(super) struct SceneRules {
    pub(super) view:             ByPresence<Carried<()>>,
    pub(super) rotation_x:       ByPresence<Carried<i8>>,
    pub(super) rotation_y:       ByPresence<RotationY>,
    pub(super) right_angle_axes: ByPresence<Carried<bool>>,
    pub(super) perspective:      ByPresence<Carried<u8>>,
    pub(super) floor:            ByPresence<Carried<&'static Node>>,
    pub(super) side_wall:        ByPresence<Carried<&'static Node>>,
    pub(super) back_wall:        ByPresence<Carried<&'static Node>>,
    /// The plot area's shape properties.
    pub(super) plot_area:        ByPresence<Carried<&'static Node>>,
}

#[derive(Debug)]
pub(super) struct GroupValuesRule {
    pub(super) gap_width:         ByPresence<Carried<u16>>,
    pub(super) first_slice_angle: ByPresence<SliceAngle>,
    /// By the source's style: any series with shape properties.
    pub(super) bands:             BySeriesStyle<ByPresence<BandRule>>,
    /// These four are selected by the chart's style, as the scene is.
    pub(super) high_low_lines:    BySeriesStyle<ByPresence<Carried<&'static Node>>>,
    pub(super) up_down_bars:      BySeriesStyle<ByPresence<Carried<&'static Node>>>,
    pub(super) series_lines:      BySeriesStyle<ByPresence<Carried<&'static Node>>>,
    pub(super) data_labels:       BySeriesStyle<ByPresence<Carried<&'static Node>>>,
}

#[derive(Debug)]
pub(super) struct BandFormat {
    pub(super) index: u32,
    /// The band's whole shape properties, as Excel wrote them.
    pub(super) shape: Option<&'static Node>,
}

pub(super) type NumberFormatRules = ByAxis<ByCategoryData<ByDateSpacing<NumberFormatRule>>>;
pub(super) type CreatedAxisRules =
    ByPresence<ByMonthlyCreation<ByCategoryData<&'static AxisTemplate>>>;

#[derive(Debug)]
pub(super) struct TitleRotationRule {
    pub(super) action: Carried<i32>,
    /// Reject source styles unobserved for this pair, axis and rotation
    /// presence.
    pub(super) styles: BySeriesStyle<bool>,
}

#[derive(Debug)]
pub(super) struct AxisPairRule {
    pub(super) key:            AxisKey,
    pub(super) number_format:  &'static NumberFormatRules,
    /// `Some` for the value role only.
    pub(super) tick_labels:    Option<ByAxis<Carried<TickLabelPosition>>>,
    /// By the source's style, the created axis' type and the category data.
    pub(super) created:        &'static CreatedAxisRules,
    /// An existing axis' gridlines, by the source's style.
    pub(super) major_grid:     BySeriesStyle<ByPresence<Carried<&'static Node>>>,
    /// An existing axis' title rotation, by prior rotation presence.
    pub(super) title_rotation: ByPresence<TitleRotationRule>,
}

pub(super) type FillRules = ByColor<ColorAction>;
pub(super) type LineRules = BySourceRole<ByColor<LineStyle>>;
pub(super) type MarkerRules = ByMarker<MarkerStyle>;
pub(super) type SmoothRules = BySourceGroup<Carried<bool>>;
pub(super) type PointRules = BySourceRole<ByValueCount<ByPresence<DataPointsRule>>>;
pub(super) type KeptPointRules = BySourceRole<ByValueCount<&'static PointFormat>>;

/// A series' formatting. Each cell is selected by the class of the series' own
/// value.
#[derive(Debug)]
pub(super) struct SeriesStyle {
    pub(super) fill:               &'static FillRules,
    pub(super) line:               &'static LineRules,
    pub(super) marker:             &'static MarkerRules,
    pub(super) smooth:             &'static SmoothRules,
    pub(super) invert_if_negative: ByPresence<Carried<bool>>,
    /// By the series' points, which the source's role and value count select.
    pub(super) points:             &'static PointRules,
    /// The formatting of each point the series keeps.
    pub(super) kept_points:        &'static KeptPointRules,
    pub(super) data_label_format:  ByPresence<NumberFormatRule>,
    pub(super) effects:            ByPresence<Carried<&'static Node>>,
    pub(super) shape_3d:           ByShape3d<Shape3dRule>,
    pub(super) marker_effects:     ByPresence<Carried<&'static Node>>,
}

#[derive(Debug)]
pub(super) struct LineStyle {
    pub(super) color: ColorAction,
    pub(super) width: WidthRule,
    pub(super) dash:  DashAction,
    pub(super) cap:   Carried<LineCap>,
    pub(super) join:  Carried<LineJoin>,
}

#[derive(Debug)]
pub(super) struct MarkerStyle {
    pub(super) presence: Carried<()>,
    pub(super) symbol:   Carried<MarkerSymbol>,
    pub(super) size:     Carried<u8>,
    pub(super) fill:     ColorAction,
    pub(super) line:     ColorAction,
    pub(super) width:    Carried<u32>,
    pub(super) dash:     DashAction,
}

/// A data point's formatting. A kept point is selected by its own values. A
/// point the conversion adds starts from its series' values, is selected by
/// them, and has no `bubble3D` of its own.
#[derive(Debug)]
pub(super) struct PointFormat {
    pub(super) fill:               ByColor<ColorAction>,
    /// By the outline colour.
    pub(super) line:               ByColor<PointLine>,
    pub(super) bubble_3d:          ByPresence<Carried<bool>>,
    pub(super) effects:            ByPresence<PointValue<&'static Node>>,
    pub(super) shape_3d:           ByShape3d<Shape3dRule>,
    pub(super) invert_if_negative: ByPresence<PointValue<bool>>,
    pub(super) marker:             ByPresence<PointValue<&'static Node>>,
    pub(super) cap:                ByPresence<PointValue<LineCap>>,
    pub(super) join:               ByPresence<PointValue<LineJoin>>,
}

#[derive(Debug)]
pub(super) struct PointLine {
    pub(super) color: ColorAction,
    pub(super) width: WidthRule,
    pub(super) dash:  DashAction,
}

/// The points made from a surface source's bands. A band's shape properties are
/// copied unless they equal the series' own.
#[derive(Debug)]
pub(super) struct BandPointFormat {
    /// Leaves the outline out.
    pub(super) remove_line:        bool,
    /// Replaces the outline with an unfilled one of this width.
    pub(super) hidden_line_width:  Option<u32>,
    pub(super) invert_if_negative: Option<bool>,
    pub(super) bubble_3d:          bool,
}

#[derive(Debug)]
pub(super) struct ShapeRules {
    pub(super) cells: &'static [(SourceShape, &'static Conversion)],
}

/// The actual series count and the uniform value count of a restricted source.
#[derive(Clone, Copy, Debug)]
pub(super) struct SourceShape {
    pub(super) series: u8,
    pub(super) values: u8,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum Namespace {
    Chart,
    Drawing,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum NodeValue {
    Literal(&'static str),
    /// The id of the axis being created.
    AxisId,
    /// The id of the axis it crosses.
    CrossAxisId,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct NodeAttribute {
    pub(super) name:  &'static str,
    pub(super) value: NodeValue,
}

/// One element of a sub-tree the conversion writes whole, as Excel wrote it.
#[derive(Clone, Copy, Debug)]
pub(super) struct Node {
    pub(super) namespace:  Namespace,
    pub(super) name:       &'static str,
    pub(super) attributes: &'static [NodeAttribute],
    pub(super) children:   &'static [Node],
}

/// A complete created axis; only its two ids come from the chart.
#[derive(Clone, Copy, Debug)]
pub(super) struct AxisTemplate {
    pub(super) root: Node,
}
