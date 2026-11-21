//! How each generated value is written into the after tree. Nothing here
//! holds a value of its own: every value comes from the rule table or from
//! the source chart.
use super::{
    model::palette,
    plan::Unresolved,
    rule::{
        AxisPosition,
        BarDirection,
        BarShape,
        Carried,
        ColorAction,
        ColorSlot,
        CrossBetween,
        DashAction,
        DashSlot,
        Grouping,
        LabelAlignment,
        LineCap,
        LineJoin,
        MarkerSymbol,
        Node,
        NumberFormatRule,
        OfPieKind,
        Orientation,
        Paint,
        PaintTransform,
        RadarStyle,
        Retain,
        ScatterStyle,
        TickLabelPosition,
        TickMark,
        TimeUnit,
        WidthRule,
    },
    xml::{
        Element,
        TemplateIds,
        instantiate,
    },
};
use crate::structs::EnumTrait;

/// A value the table names, as the text of its XML attribute.
pub(super) trait XmlText: Copy {
    fn xml(self) -> String;
}

impl XmlText for bool {
    fn xml(self) -> String {
        let text = if self { "1" } else { "0" };
        text.to_owned()
    }
}

macro_rules! numbers {
    ($($number:ty),*) => {$(
        impl XmlText for $number {
            fn xml(self) -> String {
                self.to_string()
            }
        }
    )*};
}
numbers!(i8, u8, u16, i32, u32);

macro_rules! names {
    ($($kind:ty { $($variant:ident => $text:literal),* $(,)? })*) => {$(
        impl XmlText for $kind {
            fn xml(self) -> String {
                let text = match self { $(Self::$variant => $text),* };
                text.to_owned()
            }
        }
    )*};
}
names! {
    BarDirection { Bar => "bar", Column => "col" }
    Grouping {
        Clustered => "clustered", PercentStacked => "percentStacked", Stacked => "stacked",
        Standard => "standard",
    }
    BarShape {
        Box => "box", Cone => "cone", ConeToMax => "coneToMax", Cylinder => "cylinder",
        Pyramid => "pyramid", PyramidToMax => "pyramidToMax",
    }
    OfPieKind { Bar => "bar", Pie => "pie" }
    ScatterStyle {
        Line => "line", LineMarker => "lineMarker", Marker => "marker", None => "none",
        Smooth => "smooth", SmoothMarker => "smoothMarker",
    }
    RadarStyle { Filled => "filled", Marker => "marker", Standard => "standard" }
    Orientation { MaxMin => "maxMin", MinMax => "minMax" }
    AxisPosition { Bottom => "b", Left => "l", Right => "r", Top => "t" }
    TickLabelPosition { High => "high", Low => "low", NextTo => "nextTo", None => "none" }
    TickMark { Cross => "cross", Inside => "in", None => "none", Outside => "out" }
    CrossBetween { Between => "between", MidpointCategory => "midCat" }
    LabelAlignment { Center => "ctr", Left => "l", Right => "r" }
    TimeUnit { Days => "days", Months => "months", Years => "years" }
    MarkerSymbol {
        Auto => "auto", Circle => "circle", Dash => "dash", Diamond => "diamond", Dot => "dot",
        None => "none", Picture => "picture", Plus => "plus", Square => "square",
        Star => "star", Triangle => "triangle", X => "x",
    }
    LineCap { Flat => "flat", Round => "rnd", Square => "sq" }
    LineJoin { Bevel => "bevel", Miter => "miter", Round => "round" }
}

/// The child `name` of `owner` after a carried value: kept, removed or set
/// to `<c:name val=".."/>`.
pub(super) fn carried<T: XmlText>(rule: Carried<T>, owner: &mut Element, name: &str) {
    match rule {
        Carried::Keep => (),
        Carried::Remove => owner.remove(name),
        Carried::Set(value) => owner.put(Element::value(name, &value.xml())),
    }
}

/// The child `name` of `owner` after a retained value.
pub(super) fn retained(rule: Retain, owner: &mut Element, name: &str) {
    match rule {
        Retain::Keep => (),
        Retain::Remove => owner.remove(name),
    }
}

/// A subtree the table carries as a measured template.
pub(super) fn carried_node(
    rule: Carried<&'static Node>,
    owner: &mut Element,
    name: &str,
    ids: TemplateIds,
) {
    match rule {
        Carried::Keep => (),
        Carried::Remove => owner.remove(name),
        Carried::Set(node) => owner.put(instantiate(node, ids, None)),
    }
}

/// An attribute after a carried value.
pub(super) fn carried_attribute<T: XmlText>(rule: Carried<T>, owner: &mut Element, name: &str) {
    match rule {
        Carried::Keep => (),
        Carried::Remove => owner.attributes.retain(|(key, _)| key != name),
        Carried::Set(value) => {
            owner.attributes.retain(|(key, _)| key != name);
            owner.attributes.push((name.to_owned(), value.xml()));
        }
    }
}

/// A presence the table carries: kept, removed, or set as an empty element
/// that the other rules fill.
pub(super) fn carried_presence(rule: Carried<()>, owner: &mut Element, name: &str) {
    match rule {
        Carried::Keep => (),
        Carried::Remove => owner.remove(name),
        Carried::Set(()) => {
            if owner.child(name).is_none() {
                owner.children.push(Element::new(format!("c:{name}")));
            }
        }
    }
}

fn color_element(paint: &Paint) -> Element {
    let (name, value, transforms) = match paint {
        Paint::NoFill => return Element::new("a:noFill"),
        Paint::Scheme { value, transforms } => {
            ("a:schemeClr", value.value_string().to_owned(), *transforms)
        }
        Paint::Rgb { value, transforms } => ("a:srgbClr", (*value).to_owned(), *transforms),
    };
    let mut color = Element::new(name);
    color.attributes.push(("val".to_owned(), value));
    for transform in transforms {
        let (name, value) = match transform {
            PaintTransform::LuminanceMod(value) => ("a:lumMod", value),
            PaintTransform::LuminanceOff(value) => ("a:lumOff", value),
            PaintTransform::Alpha(value) => ("a:alpha", value),
        };
        let mut child = Element::new(name);
        child.attributes.push(("val".to_owned(), value.to_string()));
        color.children.push(child);
    }
    let mut fill = Element::new("a:solidFill");
    fill.children.push(color);
    fill
}

/// The palette fill Excel generates at `index`, keeping `alpha`: a chart is
/// never refused for holding more series or points than were measured.
fn accent(index: u32, alpha: Option<&str>) -> Result<Element, Unresolved> {
    let position = usize::try_from(index).map_err(|_| Unresolved::Rejected)?;
    let (scheme, transforms) = palette(position, alpha);
    let mut color = Element::new("a:schemeClr");
    color.attributes.push(("val".to_owned(), scheme));
    for (name, value) in transforms {
        let mut child = Element::new(format!("a:{name}"));
        child.attributes.push(("val".to_owned(), value));
        color.children.push(child);
    }
    let mut fill = Element::new("a:solidFill");
    fill.children.push(color);
    Ok(fill)
}

/// The alpha a fill's colour ends with: a series' transparency, which a
/// generated accent keeps.
fn alpha(fill: Option<&Element>) -> Option<String> {
    let last = fill?.children.first()?.children.last()?;
    if last.local() == "alpha" {
        last.attribute("val").map(str::to_owned)
    } else {
        None
    }
}

const FILLS: [&str; 6] = [
    "noFill",
    "solidFill",
    "gradFill",
    "pattFill",
    "blipFill",
    "grpFill",
];

/// The fill of a shape or outline.
pub(super) fn fill(owner: Option<&Element>) -> Option<&Element> {
    owner?
        .children
        .iter()
        .find(|child| FILLS.contains(&child.local()))
}

fn set_fill(owner: &mut Element, fill: Option<Element>) {
    let position = owner
        .children
        .iter()
        .position(|child| FILLS.contains(&child.local()));
    owner
        .children
        .retain(|child| !FILLS.contains(&child.local()));
    if let Some(fill) = fill {
        let at = position.unwrap_or(0).min(owner.children.len());
        owner.children.insert(at, fill);
    }
}

/// The before fills a colour action can copy from.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Colors<'a> {
    pub(super) fill:        Option<&'a Element>,
    pub(super) line:        Option<&'a Element>,
    pub(super) marker_fill: Option<&'a Element>,
    pub(super) marker_line: Option<&'a Element>,
}

/// Where a colour or width action writes, and what it reads.
#[derive(Clone, Copy, Debug)]
pub(super) struct Place<'a> {
    /// The series' `c:idx`.
    pub(super) series:       u32,
    /// The point's index, when the action writes a point.
    pub(super) point:        Option<u32>,
    /// The before fills a copy reads.
    pub(super) copy:         Colors<'a>,
    /// For a point: the series' own fill of the same part, and its line
    /// width, which `FromSeries` takes.
    pub(super) series_fill:  Option<&'a Element>,
    pub(super) series_width: Option<&'a str>,
}

impl<'a> Place<'a> {
    /// A series' own parts.
    pub(super) const fn series(index: u32, copy: Colors<'a>) -> Self {
        Self {
            series: index,
            point: None,
            copy,
            series_fill: None,
            series_width: None,
        }
    }
}

/// The fill of `owner` after a colour action at `place`: the one colour codec
/// series, markers and points share.
pub(super) fn color(
    action: ColorAction,
    owner: &mut Element,
    place: &Place<'_>,
) -> Result<(), Unresolved> {
    let replaced = alpha(fill(Some(owner)));
    let fill = match action {
        ColorAction::Keep => return Ok(()),
        ColorAction::Remove => None,
        ColorAction::SeriesAccent => Some(accent(place.series, replaced.as_deref())?),
        ColorAction::SeriesAccentWithAlpha(alpha) => {
            Some(accent(place.series, Some(&alpha.to_string()))?)
        }
        ColorAction::PointAccent => Some(accent(
            place.point.ok_or(Unresolved::Rejected)?,
            replaced.as_deref(),
        )?),
        ColorAction::Set(paint) => Some(color_element(paint)),
        ColorAction::FromSeries => {
            place.point.ok_or(Unresolved::Rejected)?;
            place.series_fill.cloned()
        }
        ColorAction::CopyFrom(slot) => {
            let copied = match slot {
                ColorSlot::Fill => place.copy.fill,
                ColorSlot::Line => place.copy.line,
                ColorSlot::MarkerFill => place.copy.marker_fill,
                ColorSlot::MarkerLine => place.copy.marker_line,
            };
            match copied {
                Some(fill) if fill.local() != "noFill" => Some(fill.clone()),
                // Nothing to copy: the accent of the point, or of the series.
                _ => Some(accent(place.point.unwrap_or(place.series), None)?),
            }
        }
    };
    set_fill(owner, fill);
    Ok(())
}

/// An outline's width after a width rule at `place`.
pub(super) fn width(
    rule: WidthRule,
    line: &mut Element,
    place: &Place<'_>,
) -> Result<(), Unresolved> {
    let set = |line: &mut Element, width: &str| {
        line.attributes.retain(|(key, _)| key != "w");
        line.attributes.push(("w".to_owned(), width.to_owned()));
    };
    match rule {
        WidthRule::Keep => (),
        WidthRule::Remove => line.attributes.retain(|(key, _)| key != "w"),
        WidthRule::Set(width) => set(line, &width.to_string()),
        WidthRule::KeepOrSet(width) => {
            if line.attribute("w").is_none() {
                set(line, &width.to_string());
            }
        }
        WidthRule::FromSeries => {
            place.point.ok_or(Unresolved::Rejected)?;
            match place.series_width {
                Some(width) => set(line, width),
                None => line.attributes.retain(|(key, _)| key != "w"),
            }
        }
    }
    Ok(())
}

/// An outline's dash after a dash action.
pub(super) fn dash(
    action: DashAction,
    line: &mut Element,
    series: Option<&Element>,
    marker: Option<&Element>,
) {
    match action {
        DashAction::Keep => (),
        DashAction::Remove => line.remove("prstDash"),
        DashAction::CopyFrom(slot) => {
            let source = match slot {
                DashSlot::Line => series,
                DashSlot::MarkerLine => marker,
            };
            match source.and_then(|line| line.child("prstDash")) {
                Some(dash) => line.put(dash.clone()),
                None => line.remove("prstDash"),
            }
        }
    }
}

/// An outline's join after a carried join.
pub(super) fn join(rule: Carried<LineJoin>, line: &mut Element) {
    let joins = ["round", "bevel", "miter"];
    match rule {
        Carried::Keep => (),
        Carried::Remove => line
            .children
            .retain(|child| !joins.contains(&child.local())),
        Carried::Set(join) => {
            line.children
                .retain(|child| !joins.contains(&child.local()));
            line.children
                .push(Element::new(format!("a:{}", join.xml())));
        }
    }
}

/// A `c:numFmt` after a number-format rule.
pub(super) fn number_format(rule: NumberFormatRule, owner: &mut Element) -> Result<(), Unresolved> {
    let format = |code: &str, linked: &str| {
        let mut format = Element::new("c:numFmt");
        format
            .attributes
            .push(("formatCode".to_owned(), code.to_owned()));
        format
            .attributes
            .push(("sourceLinked".to_owned(), linked.to_owned()));
        format
    };
    match rule {
        NumberFormatRule::Keep => (),
        NumberFormatRule::Remove => owner.remove("numFmt"),
        NumberFormatRule::KeepOrGeneralLinked => {
            if owner.child("numFmt").is_none() {
                owner.put(format("General", "1"));
            }
        }
        NumberFormatRule::SetCodeKeepLink(code) => {
            let linked = owner
                .child("numFmt")
                .and_then(|format| format.attribute("sourceLinked"))
                .ok_or(Unresolved::Rejected)?
                .to_owned();
            owner.put(format(code, &linked));
        }
        NumberFormatRule::Set {
            code,
            source_linked,
        } => owner.put(format(code, &source_linked.xml())),
    }
    Ok(())
}

/// Edits the child `name` of `owner`, created empty when missing; the child
/// keeps its position.
pub(super) fn edit_child<R>(
    owner: &mut Element,
    name: &str,
    edit: impl FnOnce(&mut Element) -> R,
) -> R {
    let local = name.split_once(':').map_or(name, |(_, local)| local);
    let mut child = owner
        .child(local)
        .cloned()
        .unwrap_or_else(|| Element::new(name));
    let result = edit(&mut child);
    owner.put(child);
    result
}
