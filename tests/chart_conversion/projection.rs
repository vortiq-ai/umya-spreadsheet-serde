//! A chart part as the measurement saw it: comparable slots and flattened
//! element paths. `slots_of` ports the measurement driver's `features()` and
//! adds the slots that read the XML directly; `paths_of` and `diff_paths` port
//! its `flatten()` and `diff_paths()`.
use std::collections::BTreeMap;

use super::xml::{
    CHART,
    DRAWING,
    Element,
    parse,
};

/// A slot value; the shapes the measurement stores.
#[derive(Clone, Debug, PartialEq)]
pub enum Json {
    Null,
    Count(u64),
    Text(String),
    List(Vec<Json>),
    Map(BTreeMap<String, Json>),
}

pub type Slots = BTreeMap<String, Json>;
pub type Paths = BTreeMap<String, String>;

/// Elements the path projection leaves out.
const SKIPPED: [&str; 10] = [
    "numCache",
    "strCache",
    "multiLvlStrCache",
    "extLst",
    "txPr",
    "printSettings",
    "pageMargins",
    "layout",
    "numLit",
    "strLit",
];

const LABEL_FLAGS: [&str; 7] = [
    "delete",
    "showLegendKey",
    "showVal",
    "showCatName",
    "showSerName",
    "showPercent",
    "showBubbleSize",
];

const GROUP_VALUES: [&str; 21] = [
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
];

const GROUP_PARTS: [&str; 5] = [
    "hiLowLines",
    "upDownBars",
    "dropLines",
    "serLines",
    "bandFmts",
];

const SERIES_VALUES: [&str; 4] = ["invertIfNegative", "explosion", "bubble3D", "shape"];

const AXIS_VALUES: [&str; 13] = [
    "axPos",
    "delete",
    "majorUnit",
    "minorUnit",
    "crosses",
    "crossesAt",
    "crossBetween",
    "tickLblPos",
    "auto",
    "lblAlgn",
    "lblOffset",
    "noMultiLvlLbl",
    "baseTimeUnit",
];

const AXES: [&str; 4] = ["catAx", "valAx", "dateAx", "serAx"];

pub fn slots_of(xml: &str) -> Slots {
    let space = parse(xml);
    let chart = space.c("chart").expect("a chart part has c:chart");
    let mut slots = Slots::new();
    feature_slots(chart, &mut slots);
    xml_slots(chart, &mut slots);
    slots
}

pub fn paths_of(xml: &str) -> Paths {
    let root = parse(xml);
    let mut paths = Paths::new();
    flatten(&root, "", &root.name, &mut paths);
    paths
}

pub struct PathDiff {
    pub dropped: Vec<String>,
    pub added:   Vec<String>,
    pub changed: BTreeMap<String, (String, String)>,
    pub carried: usize,
}

pub fn diff_paths(before: &Paths, after: &Paths) -> PathDiff {
    PathDiff {
        dropped: before
            .keys()
            .filter(|path| !after.contains_key(*path))
            .cloned()
            .collect(),
        added:   after
            .keys()
            .filter(|path| !before.contains_key(*path))
            .cloned()
            .collect(),
        changed: before
            .iter()
            .filter_map(|(path, was)| {
                after
                    .get(path)
                    .filter(|now| *now != was)
                    .map(|now| (path.clone(), (was.clone(), now.clone())))
            })
            .collect(),
        carried: before
            .iter()
            .filter(|(path, was)| after.get(*path) == Some(*was))
            .count(),
    }
}

// ---- paths ---------------------------------------------------------------

fn local(key: &str) -> &str {
    key.rsplit_once('}').map_or(key, |(_, name)| name)
}

/// Python renders a missing value as `None` inside formatted text.
fn shown(value: Option<&str>) -> &str {
    value.unwrap_or("None")
}

fn flatten(element: &Element, path: &str, key: &str, paths: &mut Paths) {
    let is_group = element.name.to_lowercase().ends_with("chart") && element.name != "chart";
    let here = format!("{path}/{}", if is_group { "GROUP" } else { key });
    let mut attributes: Vec<&(String, String)> = element.attributes.iter().collect();
    attributes.sort();
    let attributes = attributes
        .iter()
        .map(|(key, value)| format!("{}={value}", local(key)))
        .collect::<Vec<_>>()
        .join(";");
    let text = element.text.trim();
    if element.children.is_empty() || !attributes.is_empty() || !text.is_empty() {
        paths.insert(
            here.clone(),
            format!("{attributes}|{text}").trim_matches('|').to_owned(),
        );
    }
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for child in &element.children {
        *counts.entry(child.name.as_str()).or_default() += 1;
    }
    let mut seen: BTreeMap<&str, usize> = BTreeMap::new();
    for child in &element.children {
        let name = child.name.as_str();
        if SKIPPED.contains(&name) {
            continue;
        }
        let key = if name == "ser" {
            let index = child
                .c("idx")
                .map_or("?", |idx| shown(idx.attribute("val")));
            format!("ser[{index}]")
        } else if counts[name] > 1 {
            let occurrence = seen.entry(name).or_default();
            *occurrence += 1;
            format!("{name}[{}]", *occurrence - 1)
        } else {
            name.to_owned()
        };
        flatten(child, &here, &key, paths);
    }
}

// ---- slots ---------------------------------------------------------------

fn text(value: impl Into<String>) -> Json {
    Json::Text(value.into())
}

fn optional(value: Option<String>) -> Json {
    value.map_or(Json::Null, Json::Text)
}

fn list(items: impl IntoIterator<Item = Json>) -> Json {
    Json::List(items.into_iter().collect())
}

fn map<const N: usize>(entries: [(&str, Json); N]) -> Json {
    Json::Map(
        entries
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value))
            .collect(),
    )
}

/// A slot that is absent when it has no value.
fn put(slots: &mut Slots, key: impl Into<String>, value: Option<Json>) {
    if let Some(value) = value {
        slots.insert(key.into(), value);
    }
}

/// The `val` of the first child `c:<name>`; `""` when it has none.
fn val(parent: &Element, name: &str) -> Option<String> {
    parent
        .c(name)
        .map(|child| child.attribute("val").unwrap_or_default().to_owned())
}

fn text_of(element: &Element) -> String {
    element
        .all_named(DRAWING, "t")
        .iter()
        .map(|run| run.text.as_str())
        .collect()
}

/// A title's text, or `(auto)` when it carries none.
fn label(title: Option<&Element>) -> Option<Json> {
    title.map(|title| match title.c("tx") {
        Some(tx) => text(text_of(tx)),
        None => text("(auto)"),
    })
}

fn format_pair(owner: &Element) -> String {
    format!(
        "{}|{}",
        shown(owner.attribute("formatCode")),
        shown(owner.attribute("sourceLinked"))
    )
}

fn label_flags(labels: Option<&Element>) -> Option<Json> {
    let labels = labels?;
    Some(Json::Map(
        LABEL_FLAGS
            .iter()
            .filter_map(|flag| val(labels, flag).map(|value| ((*flag).to_owned(), text(value))))
            .collect(),
    ))
}

/// The fill, outline colour, dash and width of a shape.
fn color_of(shape: Option<&Element>) -> Option<Json> {
    let shape = shape?;
    let mut colors = BTreeMap::new();
    let paint = |fill: &Element| {
        let color = fill.children.first()?;
        Some(format!(
            "{}:{}",
            color.name,
            color.attribute("val").unwrap_or_default()
        ))
    };
    if let Some(color) = shape.a("solidFill").and_then(paint) {
        colors.insert("fill", text(color));
    } else if shape.a("noFill").is_some() {
        colors.insert("fill", text("noFill"));
    }
    if let Some(line) = shape.a("ln") {
        if let Some(color) = line.a("solidFill").and_then(paint) {
            colors.insert("line", text(color));
        } else if line.a("noFill").is_some() {
            colors.insert("line", text("noFill"));
        }
        if let Some(dash) = line.a("prstDash") {
            colors.insert("dash", optional(dash.attribute("val").map(str::to_owned)));
        }
        if let Some(width) = line.attribute("w").filter(|width| !width.is_empty()) {
            colors.insert("lnW", text(width));
        }
    }
    (!colors.is_empty()).then(|| {
        Json::Map(
            colors
                .into_iter()
                .map(|(key, value)| (key.to_owned(), value))
                .collect(),
        )
    })
}

/// The cap and join of a shape's outline, which `color_of` leaves out.
fn line_extras(shape: Option<&Element>) -> BTreeMap<&'static str, String> {
    let mut extras = BTreeMap::new();
    if let Some(line) = shape.and_then(|shape| shape.a("ln")) {
        if let Some(cap) = line.attribute("cap").filter(|cap| !cap.is_empty()) {
            extras.insert("cap", cap.to_owned());
        }
        if let Some(join) = ["round", "bevel", "miter"]
            .into_iter()
            .find(|join| line.a(join).is_some())
        {
            extras.insert("join", join.to_owned());
        }
    }
    extras
}

fn group_elements(chart: &Element) -> impl Iterator<Item = &Element> {
    chart
        .c("plotArea")
        .into_iter()
        .flat_map(|area| area.children.iter())
        .filter(|group| group.name.ends_with("Chart"))
}

fn axis_elements(chart: &Element) -> impl Iterator<Item = (&Element, String)> {
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    chart
        .c("plotArea")
        .into_iter()
        .flat_map(|area| area.children.iter())
        .filter(|axis| AXES.contains(&axis.name.as_str()))
        .map(move |axis| {
            let ordinal = counts.entry(axis.name.as_str()).or_default();
            let prefix = format!("ax.{}{}.", axis.name, *ordinal);
            *ordinal += 1;
            (axis, prefix)
        })
}

fn feature_slots(chart: &Element, slots: &mut Slots) {
    put(slots, "part", Some(text("chart")));
    put(
        slots,
        "autoTitleDeleted",
        val(chart, "autoTitleDeleted").map(Json::Text),
    );
    put(slots, "title", label(chart.c("title")));
    put(
        slots,
        "legend",
        chart.c("legend").map(|legend| {
            let position = val(legend, "legendPos").filter(|position| !position.is_empty());
            text(position.unwrap_or_else(|| "(no legendPos)".to_owned()))
        }),
    );
    put(
        slots,
        "plotVisOnly",
        val(chart, "plotVisOnly").map(Json::Text),
    );
    put(
        slots,
        "dispBlanksAs",
        val(chart, "dispBlanksAs").map(Json::Text),
    );
    put(slots, "view3D", chart.c("view3D").map(|_| text("yes")));
    let area = chart.c("plotArea").expect("a chart has c:plotArea");
    put(
        slots,
        "plotAreaChildren",
        Some(list(
            area.children
                .iter()
                .filter(|child| !["layout", "spPr", "extLst"].contains(&child.name.as_str()))
                .map(|child| text(child.name.as_str())),
        )),
    );

    for (index, group) in group_elements(chart).enumerate() {
        let prefix = format!("g{index}.");
        put(
            slots,
            format!("{prefix}type"),
            Some(text(group.name.as_str())),
        );
        for name in GROUP_VALUES {
            put(
                slots,
                format!("{prefix}{name}"),
                val(group, name).map(Json::Text),
            );
        }
        for name in GROUP_PARTS {
            put(
                slots,
                format!("{prefix}{name}"),
                group.c(name).map(|_| text("present")),
            );
        }
        put(
            slots,
            format!("{prefix}dLbls"),
            label_flags(group.c("dLbls")),
        );
        put(
            slots,
            format!("{prefix}axIds"),
            Some(list(group.all_c("axId").map(|axis| {
                optional(axis.attribute("val").map(str::to_owned))
            }))),
        );
        for series in group.all_c("ser") {
            series_slots(
                series,
                &format!("{prefix}ser{}.", shown(val(series, "idx").as_deref())),
                slots,
            );
        }
    }

    for (axis, prefix) in axis_elements(chart) {
        axis_slots(axis, &prefix, slots);
    }
}

fn series_slots(series: &Element, prefix: &str, slots: &mut Slots) {
    put(
        slots,
        format!("{prefix}order"),
        val(series, "order").map(Json::Text),
    );
    put(
        slots,
        format!("{prefix}tx"),
        series.c("tx").map(|tx| match tx.descendant_c("f") {
            Some(reference) => text(format!("ref:{}", reference.text)),
            None => {
                let rich = text_of(tx);
                let literal = if rich.is_empty() {
                    tx.all_named(CHART, "v")
                        .iter()
                        .map(|value| value.text.as_str())
                        .collect()
                } else {
                    rich
                };
                text(format!("lit:{literal}"))
            }
        }),
    );
    for name in ["cat", "val", "xVal", "yVal", "bubbleSize"] {
        if let Some(data) = series.c(name) {
            put(
                slots,
                format!("{prefix}{name}"),
                match data.descendant_c("f") {
                    Some(reference) => {
                        (!reference.text.is_empty()).then(|| text(reference.text.as_str()))
                    }
                    None => Some(text("(literal)")),
                },
            );
        }
    }
    put(slots, format!("{prefix}style"), color_of(series.c("spPr")));
    if let Some(marker) = series.c("marker") {
        put(
            slots,
            format!("{prefix}marker"),
            Some(map([
                ("symbol", optional(val(marker, "symbol"))),
                ("size", optional(val(marker, "size"))),
                ("sp", color_of(marker.c("spPr")).unwrap_or(Json::Null)),
            ])),
        );
    }
    put(
        slots,
        format!("{prefix}smooth"),
        val(series, "smooth").map(Json::Text),
    );
    if let Some(labels) = series.c("dLbls") {
        put(slots, format!("{prefix}dLbls"), label_flags(Some(labels)));
        put(
            slots,
            format!("{prefix}dLbls.numFmt"),
            labels.c("numFmt").map(|format| text(format_pair(format))),
        );
        put(
            slots,
            format!("{prefix}dLbls.pos"),
            val(labels, "dLblPos").map(Json::Text),
        );
    }
    let trendlines: Vec<_> = series.all_c("trendline").collect();
    if !trendlines.is_empty() {
        put(
            slots,
            format!("{prefix}trendline"),
            Some(list(
                trendlines
                    .iter()
                    .map(|line| optional(val(line, "trendlineType"))),
            )),
        );
    }
    let error_bars: Vec<_> = series.all_c("errBars").collect();
    if !error_bars.is_empty() {
        put(
            slots,
            format!("{prefix}errBars"),
            Some(list(error_bars.iter().map(|bars| {
                list(
                    ["errDir", "errBarType", "errValType"]
                        .iter()
                        .map(|name| optional(val(bars, name))),
                )
            }))),
        );
    }
    for name in SERIES_VALUES {
        put(
            slots,
            format!("{prefix}{name}"),
            val(series, name).map(Json::Text),
        );
    }
    let points = series.all_c("dPt").count();
    if points > 0 {
        put(
            slots,
            format!("{prefix}dPt"),
            Some(Json::Count(points as u64)),
        );
    }
}

fn axis_slots(axis: &Element, prefix: &str, slots: &mut Slots) {
    let scaling = axis.c("scaling");
    put(
        slots,
        format!("{prefix}id"),
        val(axis, "axId").map(Json::Text),
    );
    put(
        slots,
        format!("{prefix}crossAx"),
        val(axis, "crossAx").map(Json::Text),
    );
    for name in ["orientation", "min", "max", "logBase"] {
        put(
            slots,
            format!("{prefix}{name}"),
            scaling
                .and_then(|scaling| val(scaling, name))
                .map(Json::Text),
        );
    }
    for name in AXIS_VALUES {
        put(
            slots,
            format!("{prefix}{name}"),
            val(axis, name).map(Json::Text),
        );
    }
    put(
        slots,
        format!("{prefix}numFmt"),
        axis.c("numFmt").map(|format| text(format_pair(format))),
    );
    put(
        slots,
        format!("{prefix}majorGridlines"),
        axis.c("majorGridlines").map(|_| text("yes")),
    );
    put(slots, format!("{prefix}title"), label(axis.c("title")));
}

/// The slots `feature_slots` cannot see: the 3-D scene, of-pie settings,
/// band formats, line caps and joins, per-point formats, literal bubble sizes
/// and axis tick marks and styles.
fn xml_slots(chart: &Element, slots: &mut Slots) {
    if let Some(view) = chart.c("view3D") {
        for name in ["rotX", "rotY", "rAngAx", "perspective"] {
            put(
                slots,
                format!("scene.{name}"),
                val(view, name).map(Json::Text),
            );
        }
    }
    for name in ["floor", "sideWall", "backWall"] {
        if let Some(wall) = chart.c(name) {
            put(
                slots,
                format!("scene.{name}"),
                Some(map([
                    ("thickness", optional(val(wall, "thickness"))),
                    ("sp", color_of(wall.c("spPr")).unwrap_or(Json::Null)),
                ])),
            );
        }
    }
    for (index, group) in group_elements(chart).enumerate() {
        let prefix = format!("g{index}.");
        put(
            slots,
            format!("{prefix}ofPieType"),
            val(group, "ofPieType").map(Json::Text),
        );
        put(
            slots,
            format!("{prefix}secondPieSize"),
            val(group, "secondPieSize").map(Json::Text),
        );
        let bands: Vec<_> = group
            .c("bandFmts")
            .into_iter()
            .flat_map(|bands| bands.all_c("bandFmt"))
            .collect();
        if !bands.is_empty() {
            put(
                slots,
                format!("{prefix}bandFmt"),
                Some(list(bands.iter().map(|band| {
                    let extras = line_extras(band.c("spPr"));
                    map([
                        ("idx", optional(val(band, "idx"))),
                        ("sp", color_of(band.c("spPr")).unwrap_or(Json::Null)),
                        (
                            "line",
                            if extras.is_empty() {
                                Json::Null
                            } else {
                                Json::Map(
                                    extras
                                        .into_iter()
                                        .map(|(key, value)| (key.to_owned(), text(value)))
                                        .collect(),
                                )
                            },
                        ),
                    ])
                }))),
            );
        }
        for series in group.all_c("ser") {
            let prefix = format!("{prefix}ser{}.", shown(val(series, "idx").as_deref()));
            series_xml_slots(series, &prefix, slots);
        }
    }
    for (axis, prefix) in axis_elements(chart) {
        put(
            slots,
            format!("{prefix}majorTickMark"),
            val(axis, "majorTickMark").map(Json::Text),
        );
        put(
            slots,
            format!("{prefix}minorTickMark"),
            val(axis, "minorTickMark").map(Json::Text),
        );
        put(slots, format!("{prefix}style"), color_of(axis.c("spPr")));
    }
}

fn series_xml_slots(series: &Element, prefix: &str, slots: &mut Slots) {
    let extras = line_extras(series.c("spPr"));
    put(
        slots,
        format!("{prefix}st.cap"),
        extras.get("cap").cloned().map(Json::Text),
    );
    put(
        slots,
        format!("{prefix}st.join"),
        extras.get("join").cloned().map(Json::Text),
    );
    let points: Vec<_> = series.all_c("dPt").collect();
    if !points.is_empty() {
        let shapes: Vec<Option<Json>> = points
            .iter()
            .map(|point| color_of(point.c("spPr")))
            .collect();
        put(
            slots,
            format!("{prefix}dPt.idx"),
            Some(list(points.iter().map(|point| optional(val(point, "idx"))))),
        );
        for color in ["fill", "line", "lnW", "dash"] {
            put(
                slots,
                format!("{prefix}dPt.{color}"),
                Some(list(shapes.iter().map(|shape| match shape {
                    Some(Json::Map(colors)) => colors.get(color).cloned().unwrap_or(Json::Null),
                    _ => Json::Null,
                }))),
            );
        }
        put(
            slots,
            format!("{prefix}dPt.bubble3D"),
            Some(list(
                points.iter().map(|point| optional(val(point, "bubble3D"))),
            )),
        );
    }
    if let Some(literal) = series.c("bubbleSize").and_then(|sizes| sizes.c("numLit")) {
        put(
            slots,
            format!("{prefix}bubbleSize.literal"),
            Some(map([
                (
                    "formatCode",
                    literal
                        .c("formatCode")
                        .filter(|code| !code.text.is_empty())
                        .map_or(Json::Null, |code| text(code.text.as_str())),
                ),
                ("ptCount", optional(val(literal, "ptCount"))),
                (
                    "pts",
                    list(literal.all_c("pt").map(|point| {
                        list([
                            optional(point.attribute("idx").map(str::to_owned)),
                            point
                                .c("v")
                                .filter(|value| !value.text.is_empty())
                                .map_or(Json::Null, |value| text(value.text.as_str())),
                        ])
                    })),
                ),
            ])),
        );
    }
}
