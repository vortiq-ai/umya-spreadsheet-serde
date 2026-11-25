//! The measurement's slot vocabulary, read from a chart tree, and the report
//! built from two of them.
//!
//! `Model::of` reads the slots the measurement driver reads (section 4.1), so
//! a slot Excel left alone compares equal here too. `changes` and `paint`
//! are the generator's `part_changes` and `series_paint`: every pair reports
//! through them, and no target adds report code of its own.
use std::fmt::Write as _;

use super::{
    ChangeKind,
    ConversionChange,
    ConvertedPart,
    PaintColor,
    PaintedPart,
    SeriesPaint,
    slots::{
        AxisSlot,
        ChartSlot,
        GroupSlot,
        PointSlot,
        SceneSlot,
        SeriesSlot,
        Slot,
    },
    xml::Element,
};
use crate::structs::drawing::charts::SeriesPartKind;

/// One slot occurrence: the slot, the plot group it belongs to (0 outside
/// groups) and which of the slot's measured fields it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Key {
    pub(super) slot:   Slot,
    pub(super) group:  usize,
    pub(super) detail: &'static str,
}

impl Key {
    const fn of(slot: Slot) -> Self {
        Self {
            slot,
            group: 0,
            detail: "",
        }
    }
}

type Values = Vec<(Key, String)>;

/// The slots of one chart: chart-level ones, then each series' by `c:idx`.
#[derive(Clone, Debug, Default)]
pub(super) struct Model {
    chart:  Values,
    series: Vec<(u32, Values)>,
}

const LABEL_FLAGS: [&str; 7] = [
    "delete",
    "showLegendKey",
    "showVal",
    "showCatName",
    "showSerName",
    "showPercent",
    "showBubbleSize",
];

const GROUP_VALUES: [(&str, GroupSlot); 22] = [
    ("barDir", GroupSlot::BarDirection),
    ("grouping", GroupSlot::Grouping),
    ("varyColors", GroupSlot::VaryColors),
    ("gapWidth", GroupSlot::GapWidth),
    ("overlap", GroupSlot::Overlap),
    ("firstSliceAng", GroupSlot::FirstSliceAngle),
    ("holeSize", GroupSlot::HoleSize),
    ("bubbleScale", GroupSlot::BubbleScale),
    ("showNegBubbles", GroupSlot::ShowNegativeBubbles),
    ("sizeRepresents", GroupSlot::SizeRepresents),
    ("scatterStyle", GroupSlot::ScatterStyle),
    ("radarStyle", GroupSlot::RadarStyle),
    ("wireframe", GroupSlot::Wireframe),
    ("bubble3D", GroupSlot::Bubble3D),
    ("shape", GroupSlot::Shape),
    ("gapDepth", GroupSlot::GapDepth),
    ("marker", GroupSlot::ShowMarker),
    ("smooth", GroupSlot::Smooth),
    ("ofPieType", GroupSlot::OfPieType),
    ("secondPieSize", GroupSlot::SecondPieSize),
    ("hiLowLines", GroupSlot::HighLowLines),
    ("upDownBars", GroupSlot::UpDownBars),
];

const GROUP_PARTS: [(&str, GroupSlot); 3] = [
    ("dropLines", GroupSlot::DropLines),
    ("serLines", GroupSlot::SeriesLines),
    ("bandFmts", GroupSlot::BandFormats),
];

const AXIS_VALUES: [(&str, AxisSlot); 15] = [
    ("axPos", AxisSlot::Position),
    ("delete", AxisSlot::Deleted),
    ("majorUnit", AxisSlot::MajorUnit),
    ("minorUnit", AxisSlot::MinorUnit),
    ("crosses", AxisSlot::Crosses),
    ("crossesAt", AxisSlot::CrossesAt),
    ("crossBetween", AxisSlot::CrossBetween),
    ("tickLblPos", AxisSlot::TickLabels),
    ("auto", AxisSlot::Auto),
    ("lblAlgn", AxisSlot::LabelAlign),
    ("lblOffset", AxisSlot::LabelOffset),
    ("noMultiLvlLbl", AxisSlot::NoMultiLevel),
    ("baseTimeUnit", AxisSlot::BaseTimeUnit),
    ("majorTickMark", AxisSlot::MajorTicks),
    ("minorTickMark", AxisSlot::MinorTicks),
];

const SCALING_VALUES: [(&str, AxisSlot); 4] = [
    ("orientation", AxisSlot::Orientation),
    ("min", AxisSlot::Minimum),
    ("max", AxisSlot::Maximum),
    ("logBase", AxisSlot::LogBase),
];

const SERIES_VALUES: [(&str, SeriesSlot); 5] = [
    ("smooth", SeriesSlot::Smooth),
    ("invertIfNegative", SeriesSlot::InvertIfNegative),
    ("explosion", SeriesSlot::Explosion),
    ("bubble3D", SeriesSlot::Bubble3D),
    ("shape", SeriesSlot::Shape),
];

const DATA_ROLES: [(&str, SeriesSlot); 5] = [
    ("cat", SeriesSlot::Categories),
    ("val", SeriesSlot::Values),
    ("xVal", SeriesSlot::XValues),
    ("yVal", SeriesSlot::YValues),
    ("bubbleSize", SeriesSlot::BubbleSize),
];

/// Data roles that move into one another: a move is not a removal.
const MOVES: [(SeriesSlot, SeriesSlot); 4] = [
    (SeriesSlot::Categories, SeriesSlot::XValues),
    (SeriesSlot::Values, SeriesSlot::YValues),
    (SeriesSlot::XValues, SeriesSlot::Categories),
    (SeriesSlot::YValues, SeriesSlot::Values),
];

/// The luminance tiers of generated accents, six accents per tier.
const TIERS: [&[(&str, &str)]; 9] = [
    &[],
    &[("lumMod", "60000")],
    &[("lumMod", "80000"), ("lumOff", "20000")],
    &[("lumMod", "80000")],
    &[("lumMod", "60000"), ("lumOff", "40000")],
    &[("lumMod", "50000")],
    &[("lumMod", "70000"), ("lumOff", "30000")],
    &[("lumMod", "70000")],
    &[("lumMod", "50000"), ("lumOff", "50000")],
];

const NONE: &str = "\u{0}";

fn join<'a>(items: impl IntoIterator<Item = Option<&'a str>>) -> String {
    items
        .into_iter()
        .map(|item| item.unwrap_or(NONE))
        .collect::<Vec<_>>()
        .join("\u{1f}")
}

fn descendants<'a>(element: &'a Element, local: &'a str, out: &mut Vec<&'a Element>) {
    for child in &element.children {
        if child.local() == local {
            out.push(child);
        }
        descendants(child, local, out);
    }
}

fn first_descendant<'a>(element: &'a Element, local: &'a str) -> Option<&'a Element> {
    let mut found = Vec::new();
    descendants(element, local, &mut found);
    found.first().copied()
}

fn text_of(element: &Element) -> String {
    let mut runs = Vec::new();
    descendants(element, "t", &mut runs);
    runs.iter()
        .filter(|run| run.name.starts_with("a:"))
        .map(|run| run.text.as_str())
        .collect()
}

/// A title's text, or `(auto)` when it carries none.
fn label(title: &Element) -> String {
    title
        .child("tx")
        .map_or_else(|| "(auto)".to_owned(), text_of)
}

fn format_pair(owner: &Element) -> String {
    join([
        owner.attribute("formatCode"),
        owner.attribute("sourceLinked"),
    ])
}

fn label_flags(labels: &Element) -> String {
    let flags: Vec<String> = LABEL_FLAGS
        .iter()
        .filter_map(|flag| labels.val(flag).map(|value| format!("{flag}={value}")))
        .collect();
    join(flags.iter().map(|flag| Some(flag.as_str())))
}

/// The fill child of a shape's properties.
pub(super) fn fill_of(shape: &Element) -> Option<&Element> {
    shape.children.iter().find(|child| {
        matches!(
            child.local(),
            "noFill" | "solidFill" | "gradFill" | "pattFill" | "blipFill" | "grpFill"
        )
    })
}

/// A fill as an exact colour: `noFill`, `schemeClr:accent1[lumMod=60000]`, or
/// the whole element.
pub(super) fn paint_of(fill: &Element) -> String {
    if fill.local() == "noFill" {
        return "noFill".to_owned();
    }
    let color = match fill.children.as_slice() {
        [color]
            if fill.local() == "solidFill" && matches!(color.local(), "schemeClr" | "srgbClr") =>
        {
            Some(color)
        }
        _ => None,
    };
    if let Some(color) = color {
        let mut paint = format!(
            "{}:{}",
            color.local(),
            color.attribute("val").unwrap_or_default()
        );
        for transform in &color.children {
            let _ = write!(
                paint,
                "[{}={}]",
                transform.local(),
                transform.attribute("val").unwrap_or_default()
            );
        }
        return paint;
    }
    fill.canonical()
}

/// The colour Excel generates for the series or point at `index`, keeping a
/// series' alpha: its scheme colour and the transforms that follow it in
/// order. Six accents share a luminance tier; past the last tier the tiers
/// repeat, so no index is too large for the palette.
pub(super) fn palette(index: usize, alpha: Option<&str>) -> (String, Vec<(&'static str, String)>) {
    let tier = TIERS[index / 6 % TIERS.len()];
    let transforms = tier
        .iter()
        .map(|(name, value)| (*name, (*value).to_owned()))
        .chain(alpha.map(|alpha| ("alpha", alpha.to_owned())))
        .collect();
    (format!("accent{}", index % 6 + 1), transforms)
}

/// The palette colour at `index` as `paint_of` spells a fill's colour.
pub(super) fn accent_paint(index: usize, alpha: Option<&str>) -> String {
    let (scheme, transforms) = palette(index, alpha);
    let mut paint = format!("schemeClr:{scheme}");
    for (name, value) in transforms {
        let _ = write!(paint, "[{name}={value}]");
    }
    paint
}

pub(super) fn alpha_of(paint: &str) -> Option<&str> {
    paint
        .strip_suffix(']')?
        .rsplit_once("[alpha=")
        .map(|(_, alpha)| alpha)
        .filter(|alpha| alpha.bytes().all(|byte| byte.is_ascii_digit()))
}

fn outline(shape: Option<&Element>) -> Option<&Element> {
    shape.and_then(|shape| shape.child("ln"))
}

fn line_cap(shape: Option<&Element>) -> Option<&str> {
    outline(shape)
        .and_then(|line| line.attribute("cap"))
        .filter(|cap| !cap.is_empty())
}

fn line_join(shape: Option<&Element>) -> Option<&'static str> {
    let line = outline(shape)?;
    ["round", "bevel", "miter"]
        .into_iter()
        .find(|join| line.child(join).is_some())
}

fn dash_of(shape: Option<&Element>) -> Option<&str> {
    outline(shape)
        .and_then(|line| line.child("prstDash"))
        .map(|dash| dash.attribute("val").unwrap_or(NONE))
}

fn width_of(shape: Option<&Element>) -> Option<&str> {
    outline(shape)
        .and_then(|line| line.attribute("w"))
        .filter(|width| !width.is_empty())
}

/// Fill, outline colour, width, dash, cap and join of a shape.
fn shape_of(shape: Option<&Element>) -> String {
    let fill = shape.and_then(fill_of).map(paint_of);
    let line = outline(shape).and_then(fill_of).map(paint_of);
    join([
        fill.as_deref(),
        line.as_deref(),
        width_of(shape),
        dash_of(shape),
        line_cap(shape),
        line_join(shape),
    ])
}

/// The axis roles of a chart's axes, the way the measurement names them: the
/// first group's axes are primary, a value axis on the bottom or top of an
/// xy chart is the category role.
fn axis_key(axis: &Element, primary: &[&str], xy: bool) -> Option<(super::AxisRole, bool)> {
    use super::AxisRole;
    let secondary = !axis.val("axId").is_some_and(|id| primary.contains(&id));
    let role = match axis.local() {
        "catAx" | "dateAx" => AxisRole::Category,
        "serAx" => AxisRole::Series,
        "valAx" if xy && matches!(axis.val("axPos"), Some("b" | "t")) => AxisRole::Category,
        "valAx" => AxisRole::Value,
        _ => return None,
    };
    Some((role, secondary))
}

/// A chart's axes with their role and whether they are on the secondary set.
pub(super) fn axes_of(chart: &Element) -> Vec<(super::AxisRole, bool, &Element)> {
    let Some(area) = chart.child("plotArea") else {
        return Vec::new();
    };
    let groups: Vec<&Element> = area
        .children
        .iter()
        .filter(|child| child.local().ends_with("Chart"))
        .collect();
    let primary: Vec<&str> = groups
        .first()
        .map(|group| {
            group
                .children_named("axId")
                .filter_map(|id| id.attribute("val"))
                .collect()
        })
        .unwrap_or_default();
    let xy = groups
        .iter()
        .any(|group| matches!(group.local(), "scatterChart" | "bubbleChart"));
    area.children
        .iter()
        .filter_map(|axis| {
            let (role, secondary) = axis_key(axis, &primary, xy)?;
            Some((role, secondary, axis))
        })
        .collect()
}

impl Model {
    /// The slots of a `c:chart` tree.
    pub(super) fn of(chart: &Element) -> Self {
        let mut model = Self::default();
        model.chart_slots(chart);
        model
    }

    fn put(&mut self, key: Key, value: Option<String>) {
        if let Some(value) = value {
            self.chart.push((key, value));
        }
    }

    fn chart_slots(&mut self, chart: &Element) {
        let chart_values = [
            ("autoTitleDeleted", ChartSlot::AutoTitleDeleted),
            ("plotVisOnly", ChartSlot::PlotVisibleOnly),
            ("dispBlanksAs", ChartSlot::DisplayBlanksAs),
        ];
        for (name, slot) in chart_values {
            self.put(
                Key::of(Slot::Chart(slot)),
                chart.val(name).map(str::to_owned),
            );
        }
        self.put(
            Key::of(Slot::Chart(ChartSlot::Title)),
            chart.child("title").map(label),
        );
        self.put(
            Key::of(Slot::Chart(ChartSlot::Legend)),
            chart.child("legend").map(|legend| {
                legend
                    .val("legendPos")
                    .filter(|position| !position.is_empty())
                    .unwrap_or("(no legendPos)")
                    .to_owned()
            }),
        );
        if let Some(view) = chart.child("view3D") {
            self.put(
                Key::of(Slot::Scene(SceneSlot::View)),
                Some("yes".to_owned()),
            );
            let scene = [
                ("rotX", SceneSlot::RotationX),
                ("rotY", SceneSlot::RotationY),
                ("rAngAx", SceneSlot::RightAngleAxes),
                ("perspective", SceneSlot::Perspective),
            ];
            for (name, slot) in scene {
                self.put(
                    Key::of(Slot::Scene(slot)),
                    view.val(name).map(str::to_owned),
                );
            }
        }
        let walls = [
            ("floor", SceneSlot::Floor),
            ("sideWall", SceneSlot::SideWall),
            ("backWall", SceneSlot::BackWall),
        ];
        for (name, slot) in walls {
            self.put(
                Key::of(Slot::Scene(slot)),
                chart.child(name).map(|wall| {
                    join([
                        wall.val("thickness"),
                        Some(shape_of(wall.child("spPr")).as_str()),
                    ])
                }),
            );
        }
        let Some(area) = chart.child("plotArea") else {
            return;
        };
        let groups: Vec<&Element> = area
            .children
            .iter()
            .filter(|child| child.local().ends_with("Chart"))
            .collect();
        for (index, group) in groups.iter().enumerate() {
            self.group_slots(index, group);
        }
        for (role, secondary, axis) in axes_of(chart) {
            self.axis_slots(axis, role, secondary);
        }
    }

    fn group_slots(&mut self, index: usize, group: &Element) {
        let key = |slot, detail| Key {
            slot: Slot::Group(slot),
            group: index,
            detail,
        };
        self.put(key(GroupSlot::Type, ""), Some(group.local().to_owned()));
        for (name, slot) in GROUP_VALUES {
            self.put(key(slot, ""), group.val(name).map(str::to_owned));
        }
        for (name, slot) in GROUP_PARTS {
            self.put(
                key(slot, ""),
                group.child(name).map(|_| "present".to_owned()),
            );
        }
        self.put(
            key(GroupSlot::DataLabels, ""),
            group.child("dLbls").map(label_flags),
        );
        let bands: Vec<&Element> = group
            .child("bandFmts")
            .map(|bands| bands.children_named("bandFmt").collect())
            .unwrap_or_default();
        if !bands.is_empty() {
            let shapes: Vec<String> = bands
                .iter()
                .map(|band| join([band.val("idx"), Some(shape_of(band.child("spPr")).as_str())]))
                .collect();
            self.put(
                key(GroupSlot::Bands, ""),
                Some(join(shapes.iter().map(|shape| Some(shape.as_str())))),
            );
        }
        for series in group.children_named("ser") {
            let index = series
                .val("idx")
                .and_then(|idx| idx.parse().ok())
                .unwrap_or(u32::MAX);
            self.series.push((index, series_slots(series)));
        }
    }

    fn axis_slots(&mut self, axis: &Element, role: super::AxisRole, secondary: bool) {
        let key = |slot| {
            Key::of(Slot::Axis {
                role,
                secondary,
                slot,
            })
        };
        self.put(key(AxisSlot::Type), Some(axis.local().to_owned()));
        if let Some(scaling) = axis.child("scaling") {
            for (name, slot) in SCALING_VALUES {
                self.put(key(slot), scaling.val(name).map(str::to_owned));
            }
        }
        for (name, slot) in AXIS_VALUES {
            self.put(key(slot), axis.val(name).map(str::to_owned));
        }
        self.put(
            key(AxisSlot::NumberFormat),
            axis.child("numFmt").map(format_pair),
        );
        self.put(
            key(AxisSlot::MajorGrid),
            axis.child("majorGridlines").map(|_| "yes".to_owned()),
        );
        self.put(key(AxisSlot::Title), axis.child("title").map(label));
        let style = axis.child("spPr").and_then(|shape| {
            let fill = color_name(shape);
            let line = outline(Some(shape)).and_then(color_name);
            let dash = dash_of(Some(shape));
            let width = width_of(Some(shape));
            (fill.is_some() || line.is_some() || dash.is_some() || width.is_some())
                .then(|| join([fill.as_deref(), line.as_deref(), dash, width]))
        });
        self.put(key(AxisSlot::Style), style);
    }

    /// Each series' slots in series order, with its `c:idx`.
    pub(super) fn series(&self) -> &[(u32, Values)] {
        &self.series
    }
}

/// The colour of a shape or outline without transforms, as the slot
/// vocabulary names it: a solid fill's first colour, or `noFill`.
fn color_name(owner: &Element) -> Option<String> {
    match owner
        .child("solidFill")
        .and_then(|fill| fill.children.first())
    {
        Some(color) => Some(format!(
            "{}:{}",
            color.local(),
            color.attribute("val").unwrap_or_default()
        )),
        None => owner.child("noFill").map(|_| "noFill".to_owned()),
    }
}

fn series_slots(series: &Element) -> Values {
    let mut values = Values::new();
    let mut put = |slot: Slot, detail: &'static str, value: Option<String>| {
        if let Some(value) = value {
            values.push((
                Key {
                    slot,
                    group: 0,
                    detail,
                },
                value,
            ));
        }
    };
    put(
        Slot::Series(SeriesSlot::Order),
        "",
        series.val("order").map(str::to_owned),
    );
    put(
        Slot::Series(SeriesSlot::Text),
        "",
        series
            .child("tx")
            .map(|tx| match first_descendant(tx, "f") {
                Some(reference) => format!("ref:{}", reference.text),
                None => format!("lit:{}", text_of(tx)),
            }),
    );
    for (name, slot) in DATA_ROLES {
        if let Some(data) = series.child(name) {
            let value = match first_descendant(data, "f") {
                Some(reference) => (!reference.text.is_empty()).then(|| reference.text.clone()),
                None => Some("(literal)".to_owned()),
            };
            put(Slot::Series(slot), "", value);
        }
    }
    if let Some(literal) = series
        .child("bubbleSize")
        .and_then(|size| size.child("numLit"))
    {
        let points: Vec<String> = literal
            .children_named("pt")
            .map(|point| {
                join([
                    point.attribute("idx"),
                    point.child("v").map(|value| value.text.as_str()),
                ])
            })
            .collect();
        put(
            Slot::Series(SeriesSlot::BubbleSize),
            "literal",
            Some(join(
                [
                    literal.child("formatCode").map(|code| code.text.as_str()),
                    literal.val("ptCount"),
                ]
                .into_iter()
                .chain(points.iter().map(|point| Some(point.as_str()))),
            )),
        );
    }
    let shape = series.child("spPr");
    let paints = [
        (SeriesSlot::Fill, shape.and_then(fill_of)),
        (SeriesSlot::Line, outline(shape).and_then(fill_of)),
    ];
    for (slot, fill) in paints {
        put(Slot::Series(slot), "", fill.map(paint_of));
    }
    put(
        Slot::Series(SeriesSlot::LineWidth),
        "",
        width_of(shape).map(str::to_owned),
    );
    put(
        Slot::Series(SeriesSlot::LineDash),
        "",
        dash_of(shape).map(str::to_owned),
    );
    put(
        Slot::Series(SeriesSlot::LineCap),
        "",
        line_cap(shape).map(str::to_owned),
    );
    put(
        Slot::Series(SeriesSlot::LineJoin),
        "",
        line_join(shape).map(str::to_owned),
    );
    if let Some(marker) = series.child("marker") {
        let marker_shape = marker.child("spPr");
        put(
            Slot::Series(SeriesSlot::Marker),
            "",
            Some("present".to_owned()),
        );
        put(
            Slot::Series(SeriesSlot::MarkerSymbol),
            "",
            marker.val("symbol").map(str::to_owned),
        );
        put(
            Slot::Series(SeriesSlot::MarkerSize),
            "",
            marker.val("size").map(str::to_owned),
        );
        put(
            Slot::Series(SeriesSlot::MarkerFill),
            "",
            marker_shape.and_then(fill_of).map(paint_of),
        );
        put(
            Slot::Series(SeriesSlot::MarkerLine),
            "",
            outline(marker_shape).and_then(fill_of).map(paint_of),
        );
        put(
            Slot::Series(SeriesSlot::MarkerWidth),
            "",
            width_of(marker_shape).map(str::to_owned),
        );
        put(
            Slot::Series(SeriesSlot::MarkerDash),
            "",
            dash_of(marker_shape).map(str::to_owned),
        );
    }
    for (name, slot) in SERIES_VALUES {
        put(Slot::Series(slot), "", series.val(name).map(str::to_owned));
    }
    if let Some(labels) = series.child("dLbls") {
        put(
            Slot::Series(SeriesSlot::DataLabels),
            "",
            Some(label_flags(labels)),
        );
        put(
            Slot::Series(SeriesSlot::DataLabelFormat),
            "",
            labels.child("numFmt").map(format_pair),
        );
        put(
            Slot::Series(SeriesSlot::DataLabelPosition),
            "",
            labels.val("dLblPos").map(str::to_owned),
        );
    }
    let trendlines: Vec<_> = series
        .children_named("trendline")
        .map(|line| line.val("trendlineType"))
        .collect();
    put(
        Slot::Series(SeriesSlot::Trendlines),
        "",
        (!trendlines.is_empty()).then(|| join(trendlines)),
    );
    let error_bars: Vec<String> = series
        .children_named("errBars")
        .map(|bars| join(["errDir", "errBarType", "errValType"].map(|name| bars.val(name))))
        .collect();
    put(
        Slot::Series(SeriesSlot::ErrorBars),
        "",
        (!error_bars.is_empty()).then(|| join(error_bars.iter().map(|bars| Some(bars.as_str())))),
    );
    let points: Vec<&Element> = series.children_named("dPt").collect();
    if !points.is_empty() {
        put(
            Slot::Series(SeriesSlot::DataPoints),
            "",
            Some(join(points.iter().map(|point| point.val("idx")))),
        );
        let shapes: Vec<Option<&Element>> =
            points.iter().map(|point| point.child("spPr")).collect();
        let fills: Vec<Option<String>> = shapes
            .iter()
            .map(|shape| shape.and_then(fill_of).map(paint_of))
            .collect();
        let lines: Vec<Option<String>> = shapes
            .iter()
            .map(|shape| outline(*shape).and_then(fill_of).map(paint_of))
            .collect();
        put(
            Slot::Point(PointSlot::Fill),
            "",
            Some(join(fills.iter().map(Option::as_deref))),
        );
        put(
            Slot::Point(PointSlot::Line),
            "",
            Some(join(lines.iter().map(Option::as_deref))),
        );
        put(
            Slot::Point(PointSlot::LineWidth),
            "",
            Some(join(shapes.iter().map(|shape| width_of(*shape)))),
        );
        put(
            Slot::Point(PointSlot::LineDash),
            "",
            Some(join(shapes.iter().map(|shape| dash_of(*shape)))),
        );
        put(
            Slot::Point(PointSlot::Bubble3D),
            "",
            Some(join(points.iter().map(|point| point.val("bubble3D")))),
        );
    }
    values
}

fn get(values: &Values, slot: Slot) -> Option<&str> {
    values
        .iter()
        .find(|(key, _)| key.slot == slot && key.detail.is_empty())
        .map(|(_, value)| value.as_str())
}

/// One change per changed part, sorted by part.
fn part_changes(before: &Values, after: &Values, moves: bool) -> Vec<(ConvertedPart, ChangeKind)> {
    let mut out = Vec::new();
    let mut skipped: Vec<Slot> = Vec::new();
    if moves {
        for (from, to) in MOVES {
            let (from, to) = (Slot::Series(from), Slot::Series(to));
            let moved = matches!(
                (get(before, from), get(after, from), get(before, to), get(after, to)),
                (Some(was), None, None, Some(now)) if was == now
            );
            if let (true, Some(ConvertedPart::SeriesPart(kind)), Some(part)) =
                (moved, to.part(), from.part())
            {
                out.push((part, ChangeKind::MovedTo(kind)));
                skipped.extend([from, to]);
            }
        }
    }
    let mut states: Vec<(ConvertedPart, bool, bool, bool)> = Vec::new();
    let keys = before.iter().chain(after.iter()).map(|(key, _)| *key);
    let mut seen: Vec<Key> = Vec::new();
    for key in keys {
        if seen.contains(&key) || skipped.contains(&key.slot) {
            continue;
        }
        seen.push(key);
        let Some(part) = key.slot.part() else {
            continue;
        };
        let was = before
            .iter()
            .find(|(other, _)| *other == key)
            .map(|(_, value)| value);
        let now = after
            .iter()
            .find(|(other, _)| *other == key)
            .map(|(_, value)| value);
        if !states.iter().any(|state| state.0 == part) {
            states.push((part, false, false, false));
        }
        if let Some(state) = states.iter_mut().find(|state| state.0 == part) {
            state.1 |= was.is_some();
            state.2 |= now.is_some();
            state.3 |= was != now;
        }
    }
    out.extend(
        states
            .into_iter()
            .filter(|state| state.3)
            .map(|(part, was, now, _)| {
                let change = match (was, now) {
                    (true, true) => ChangeKind::Rewritten,
                    (_, true) => ChangeKind::Added,
                    _ => ChangeKind::Removed,
                };
                (part, change)
            }),
    );
    out.sort_by_key(|(part, _)| rank(*part));
    out
}

/// A part's position in the report.
fn rank(part: ConvertedPart) -> usize {
    use super::AxisRole;
    match part {
        ConvertedPart::Scene => 0,
        ConvertedPart::Group => 1,
        ConvertedPart::Axis { role, secondary } => {
            2 + 2 * match role {
                AxisRole::Category => 0,
                AxisRole::Value => 1,
                AxisRole::Series => 2,
            } + usize::from(secondary)
        }
        ConvertedPart::SeriesPart(kind) => {
            8 + match kind {
                SeriesPartKind::Explosion => 0,
                SeriesPartKind::InvertIfNegative => 1,
                SeriesPartKind::Marker => 2,
                SeriesPartKind::Categories => 3,
                SeriesPartKind::Values => 4,
                SeriesPartKind::XValues => 5,
                SeriesPartKind::YValues => 6,
                SeriesPartKind::BubbleSize => 7,
                SeriesPartKind::Bubble3D => 8,
                SeriesPartKind::Smooth => 9,
                SeriesPartKind::Shape => 10,
                SeriesPartKind::PictureOptions => 11,
                SeriesPartKind::DataPoints => 12,
                SeriesPartKind::Trendlines => 13,
                SeriesPartKind::DataLabels => 14,
                SeriesPartKind::Extensions => 15,
                SeriesPartKind::ErrorBars => 16,
            }
        }
        ConvertedPart::SeriesFill => 25,
        ConvertedPart::SeriesLine => 26,
    }
}

/// Section 2.3's `changes()`: the chart's changed parts, then each series',
/// in series order. A series is matched by its `c:idx`.
/// `removed` holds the parts the target families' cleanup removed from the
/// stored series, by source position; each is reported once, ranked with the
/// slot changes. A removal decides its part over the slot diff, which sees
/// only what was written, except a part moved to another role.
pub(super) fn changes(
    before: &Model,
    after: &Model,
    removed: &[(usize, SeriesPartKind)],
) -> Vec<ConversionChange> {
    let mut out: Vec<ConversionChange> = part_changes(&before.chart, &after.chart, false)
        .into_iter()
        .map(|(part, change)| ConversionChange {
            series_index: None,
            part,
            change,
        })
        .collect();
    let empty = Values::new();
    for (position, (index, was)) in before.series.iter().enumerate() {
        let now = after
            .series
            .iter()
            .find(|(other, _)| other == index)
            .map_or(&empty, |(_, values)| values);
        let mut parts = part_changes(was, now, true);
        for (_, kind) in removed.iter().filter(|(at, _)| *at == position) {
            let part = ConvertedPart::SeriesPart(*kind);
            match parts.iter_mut().find(|(reported, _)| *reported == part) {
                Some((_, ChangeKind::MovedTo(_))) => (),
                Some((_, change)) => *change = ChangeKind::Removed,
                None => parts.push((part, ChangeKind::Removed)),
            }
        }
        parts.sort_by_key(|(part, _)| rank(*part));
        out.extend(parts.into_iter().map(|(part, change)| ConversionChange {
            series_index: Some(position),
            part,
            change,
        }));
    }
    out
}

/// Section 2.3's `series_paint()`: where each series' colour sits in `paint`
/// (one entry, or one per series) and where it comes from.
pub(super) fn paint(after: &Model, paint: &[PaintedPart]) -> Option<Vec<SeriesPaint>> {
    let series = after.series();
    if paint.len() != 1 && paint.len() != series.len() {
        return None;
    }
    series
        .iter()
        .enumerate()
        .map(|(position, (index, values))| {
            let part = *paint.get(position).or_else(|| paint.first())?;
            let color = match part {
                PaintedPart::Nothing => PaintColor::Unpainted,
                PaintedPart::Marker
                    if get(values, Slot::Series(SeriesSlot::MarkerSymbol)) == Some("none") =>
                {
                    PaintColor::Unpainted
                }
                PaintedPart::Fill | PaintedPart::Line | PaintedPart::Marker => {
                    let slot = match part {
                        PaintedPart::Fill => SeriesSlot::Fill,
                        PaintedPart::Line => SeriesSlot::Line,
                        PaintedPart::Marker | PaintedPart::Nothing => SeriesSlot::MarkerFill,
                    };
                    paint_color(get(values, Slot::Series(slot)), *index)
                }
            };
            Some(SeriesPaint { part, color })
        })
        .collect()
}

fn paint_color(paint: Option<&str>, index: u32) -> PaintColor {
    let Some(paint) = paint else {
        return PaintColor::SeriesAccent;
    };
    if paint == "noFill" {
        return PaintColor::Unpainted;
    }
    let accent = usize::try_from(index)
        .ok()
        .map(|index| accent_paint(index, alpha_of(paint)));
    if accent.as_deref() == Some(paint) {
        PaintColor::SeriesAccent
    } else {
        PaintColor::Explicit
    }
}
