//! One series through the conversion: the `c:ser` Excel writes for it, and
//! the series umya holds afterwards.
//!
//! A series is never rebuilt from the tree. A data role that moves keeps its
//! typed reference (`move_role`); the parts that change are cleared with
//! `remove_parts` and their new content is read onto the series by umya's own
//! series reader, so references and every unchanged part stay as they were.
use super::{
    build::Cleanup,
    plan::{
        Replay,
        Step,
        Unresolved,
        cell,
        presence,
    },
    rule::{
        Carried,
        DataRole,
        DataRules,
        ErrorBarRule,
        LineStyle,
        MarkerStyle,
        RoleAction,
        SeriesStyle,
        Shape3dRule,
    },
    slots::{
        Shape3dClass,
        SmoothFlag,
    },
    source::{
        Source,
        SourceSeries,
    },
    write::{
        self,
        Colors,
        Place,
    },
    xml::{
        self,
        Element,
        TemplateIds,
    },
};
use crate::{
    reader::xlsx::SourceCacheSession,
    structs::drawing::charts::{
        AreaChartSeries,
        CategoryAxisData,
        NumberReference,
        NumericData,
        SeriesPartKind,
        Values,
        XValues,
        YValues,
    },
};

/// The series children each part kind owns.
const PARTS: [(&str, SeriesPartKind); 17] = [
    ("explosion", SeriesPartKind::Explosion),
    ("invertIfNegative", SeriesPartKind::InvertIfNegative),
    ("marker", SeriesPartKind::Marker),
    ("cat", SeriesPartKind::Categories),
    ("val", SeriesPartKind::Values),
    ("xVal", SeriesPartKind::XValues),
    ("yVal", SeriesPartKind::YValues),
    ("bubbleSize", SeriesPartKind::BubbleSize),
    ("bubble3D", SeriesPartKind::Bubble3D),
    ("smooth", SeriesPartKind::Smooth),
    ("shape", SeriesPartKind::Shape),
    ("pictureOptions", SeriesPartKind::PictureOptions),
    ("dPt", SeriesPartKind::DataPoints),
    ("trendline", SeriesPartKind::Trendlines),
    ("dLbls", SeriesPartKind::DataLabels),
    ("extLst", SeriesPartKind::Extensions),
    ("errBars", SeriesPartKind::ErrorBars),
];

/// The data roles: their references are moved, never re-read.
const ROLES: [&str; 5] = ["cat", "val", "xVal", "yVal", "bubbleSize"];

const fn role_name(role: DataRole) -> &'static str {
    match role {
        DataRole::Categories => "cat",
        DataRole::Values => "val",
        DataRole::XValues => "xVal",
        DataRole::YValues => "yVal",
        DataRole::BubbleSize => "bubbleSize",
    }
}

/// A data role whose content becomes another role, as categories become x
/// values. The prediction renames the element; the commit moves the typed
/// reference. Both read it from `role_moves`.
#[derive(Clone, Copy, Debug)]
pub(super) struct RoleMove {
    pub(super) from: DataRole,
    pub(super) to:   DataRole,
}

/// The role moves the KIND rule makes on a series, read from its roles'
/// presence before the conversion.
pub(super) fn role_moves(data: &DataRules, before: &Element) -> Result<Vec<RoleMove>, Unresolved> {
    let roles = [
        (&data.categories, DataRole::Categories),
        (&data.values, DataRole::Values),
        (&data.x_values, DataRole::XValues),
        (&data.y_values, DataRole::YValues),
        (&data.bubble_size, DataRole::BubbleSize),
    ];
    let mut moves = Vec::new();
    for (rule, to) in roles {
        let present = before.child(role_name(to)).is_some();
        if let RoleAction::MoveFrom(from) = *cell(rule.get(presence(present)))? {
            if before.child(role_name(from)).is_none() {
                return Err(Unresolved::Rejected);
            }
            moves.push(RoleMove { from, to });
        }
    }
    Ok(moves)
}

/// A data role's content in the form its writer reads it.
enum Content {
    Reference(NumberReference),
    Literal(NumericData),
    Text(Box<CategoryAxisData>),
}

/// What `role` of `series` writes, by its writer's precedence: moved out, or
/// copied when the result keeps the role.
fn take(series: &mut AreaChartSeries, role: DataRole, keep: bool) -> Result<Content, Unresolved> {
    fn owned<T: Clone + Default>(value: Option<&mut T>, keep: bool) -> Result<T, Unresolved> {
        let value = value.ok_or(Unresolved::Rejected)?;
        Ok(if keep {
            value.clone()
        } else {
            std::mem::take(value)
        })
    }
    Ok(match role {
        // The text source is written first, and it keeps an older literal.
        DataRole::XValues => {
            let mut x = owned(series.x_values_mut(), keep)?;
            if x.text_source().is_some() {
                Content::Text(Box::new(
                    x.text_source_mut().map(std::mem::take).unwrap_or_default(),
                ))
            } else if let Some(literal) = x.number_literal_mut() {
                Content::Literal(std::mem::take(literal))
            } else {
                Content::Reference(std::mem::take(x.number_reference_mut()))
            }
        }
        DataRole::YValues => {
            let mut y = owned(series.y_values_mut(), keep)?;
            match y.number_literal_mut() {
                Some(literal) => Content::Literal(std::mem::take(literal)),
                None => Content::Reference(std::mem::take(y.number_reference_mut())),
            }
        }
        DataRole::Values => {
            let mut values = owned(series.values_mut(), keep)?;
            match values.reference_role_parts_mut() {
                (_, Some(literal)) => Content::Literal(std::mem::take(literal)),
                (Some(reference), None) => Content::Reference(std::mem::take(reference)),
                (None, None) => return Err(Unresolved::Rejected),
            }
        }
        DataRole::Categories => {
            let mut categories = owned(series.category_axis_data_mut(), keep)?;
            if categories.holds_text() {
                Content::Text(Box::new(categories))
            } else {
                match categories.take_numeric() {
                    (Some(reference), None) => Content::Reference(reference),
                    (None, Some(literal)) => Content::Literal(literal),
                    _ => return Err(Unresolved::Rejected),
                }
            }
        }
        DataRole::BubbleSize => return Err(Unresolved::Rejected),
    })
}

/// Installs `content` as `role` of `series`, the way the role's reader
/// installs it.
fn put(series: &mut AreaChartSeries, role: DataRole, content: Content) -> Result<(), Unresolved> {
    match (role, content) {
        (DataRole::XValues, content) => {
            let mut x = XValues::default();
            match content {
                Content::Text(text) => x.set_text_source(*text),
                Content::Literal(literal) => x.set_number_literal(literal),
                Content::Reference(reference) => x.set_number_reference(reference),
            };
            series.set_x_values(x);
        }
        (DataRole::Categories, content) => {
            let categories = match content {
                Content::Text(text) => *text,
                Content::Literal(literal) => {
                    let mut categories = CategoryAxisData::default();
                    categories.set_number_literal(literal);
                    categories
                }
                Content::Reference(reference) => {
                    let mut categories = CategoryAxisData::default();
                    categories.set_number_reference(reference);
                    categories
                }
            };
            series.set_category_axis_data(categories);
        }
        (DataRole::YValues, Content::Literal(literal)) => {
            let mut y = YValues::default();
            y.set_number_literal(literal);
            series.set_y_values(y);
        }
        (DataRole::YValues, Content::Reference(reference)) => {
            let mut y = YValues::default();
            y.set_number_reference(reference);
            series.set_y_values(y);
        }
        (DataRole::Values, Content::Literal(literal)) => {
            let mut values = Values::default();
            values.set_number_literal(literal);
            series.set_values(values);
        }
        (DataRole::Values, Content::Reference(reference)) => {
            let mut values = Values::default();
            values.set_number_reference(reference);
            series.set_values(values);
        }
        _ => return Err(Unresolved::Rejected),
    }
    Ok(())
}

/// Moves one role's typed content into another on `series`; the source role
/// stays when the result keeps it.
fn move_role(
    series: &mut AreaChartSeries,
    role_move: RoleMove,
    keep_source: bool,
) -> Result<(), Unresolved> {
    let content = take(series, role_move.from, keep_source)?;
    put(series, role_move.to, content)?;
    if !keep_source {
        let kind = part_kind(role_name(role_move.from)).ok_or(Unresolved::Rejected)?;
        drop(series.remove_parts(&[kind]));
    }
    Ok(())
}

/// The part kind of the series child `name`.
fn part_kind(name: &str) -> Option<SeriesPartKind> {
    PARTS
        .iter()
        .find(|(part, _)| *part == name)
        .map(|(_, kind)| *kind)
}

/// Shape properties that held something before and nothing now go, as Excel
/// drops an owner the conversion empties.
fn drop_if_emptied(owner: &mut Element, before: Option<&Element>) {
    let emptied = owner
        .child("spPr")
        .is_some_and(|shape| shape.children.is_empty());
    if emptied && before.is_some_and(|shape| !shape.children.is_empty()) {
        owner.remove("spPr");
    }
}

/// The literal Excel writes for a series without bubble sizes: one `1` per
/// value.
fn ones(values: usize) -> Element {
    let mut literal = Element::new("c:numLit");
    let mut code = Element::new("c:formatCode");
    "General".clone_into(&mut code.text);
    literal.children.push(code);
    literal
        .children
        .push(Element::value("ptCount", &values.to_string()));
    for index in 0..values {
        let mut point = Element::new("c:pt");
        point.attributes.push(("idx".to_owned(), index.to_string()));
        let mut value = Element::new("c:v");
        "1".clone_into(&mut value.text);
        point.children.push(value);
        literal.children.push(point);
    }
    let mut size = Element::new("c:bubbleSize");
    size.children.push(literal);
    size
}

fn is_empty_line(line: &Element) -> bool {
    write::fill(Some(line)).is_none()
        && line.attribute("w").is_none()
        && line.attribute("cap").is_none()
        && !line
            .children
            .iter()
            .any(|child| matches!(child.local(), "prstDash" | "round" | "bevel" | "miter"))
}

/// The `c:ser` Excel writes for `entry`, the series at `position`.
pub(super) fn after(
    replay: &Replay<'_>,
    entry: &SourceSeries,
    position: usize,
) -> Result<Element, Unresolved> {
    let before = &entry.element;
    let mut series = before.clone();
    let has = |name: &str| presence(before.child(name).is_some());
    let data = &replay.kind.data;
    let roles = [
        (&data.categories, "cat"),
        (&data.values, "val"),
        (&data.x_values, "xVal"),
        (&data.y_values, "yVal"),
        (&data.bubble_size, "bubbleSize"),
    ];
    for (rule, name) in roles {
        match *cell(rule.get(has(name)))? {
            RoleAction::Keep => (),
            RoleAction::Remove => series.remove(name),
            RoleAction::MoveFrom(from) => {
                let mut moved = before
                    .child(role_name(from))
                    .ok_or(Unresolved::Rejected)?
                    .clone();
                format!("c:{name}").clone_into(&mut moved.name);
                series.put(moved);
            }
            RoleAction::OnesPerPoint => series.put(ones(entry.values.ok_or(Unresolved::Rejected)?)),
        }
    }
    let parts = &replay.kind.parts;
    write::retained(*cell(parts.order.get(has("order")))?, &mut series, "order");
    write::retained(*cell(parts.text.get(has("tx")))?, &mut series, "tx");
    write::retained(
        *cell(parts.data_labels.get(has("dLbls")))?,
        &mut series,
        "dLbls",
    );
    let position_present = before
        .child("dLbls")
        .is_some_and(|labels| labels.child("dLblPos").is_some());
    if let (super::rule::Retain::Remove, Some(labels)) = (
        *cell(parts.data_label_position.get(presence(position_present)))?,
        series.child_mut("dLbls"),
    ) {
        labels.remove("dLblPos");
    }
    write::retained(
        *cell(parts.trendlines.get(has("trendline")))?,
        &mut series,
        "trendline",
    );
    match *cell(parts.error_bars.get(has("errBars")))? {
        ErrorBarRule::Keep => (),
        ErrorBarRule::Remove => series.remove("errBars"),
        ErrorBarRule::SetDirection(_) => return Err(Unresolved::Pending(Step::ErrorBarDirection)),
    }
    write::carried(
        *cell(parts.explosion.get(has("explosion")))?,
        &mut series,
        "explosion",
    );
    write::carried(
        *cell(parts.bubble_3d.get(has("bubble3D")))?,
        &mut series,
        "bubble3D",
    );
    write::retained(*cell(parts.shape.get(has("shape")))?, &mut series, "shape");
    let style = replay.style(position, entry.style)?;
    apply_style(entry, style, &mut series)?;
    let line_before = before
        .child("spPr")
        .and_then(|shape| shape.child("ln"))
        .is_some_and(|line| !is_empty_line(line));
    let emptied = series
        .child("spPr")
        .and_then(|shape| shape.child("ln"))
        .is_some_and(is_empty_line)
        && before
            .child("spPr")
            .is_some_and(|shape| shape.child("ln").is_some());
    if emptied {
        let rule = *cell(parts.empty_line.get(presence(line_before)))?;
        if let (super::rule::Retain::Remove, Some(shape)) = (rule, series.child_mut("spPr")) {
            shape.remove("ln");
        }
    }
    drop_if_emptied(&mut series, before.child("spPr"));
    Ok(series)
}

fn apply_style(
    entry: &SourceSeries,
    style: &SeriesStyle,
    series: &mut Element,
) -> Result<(), Unresolved> {
    let before = &entry.element;
    let shape = before.child("spPr");
    let marker_shape = before
        .child("marker")
        .and_then(|marker| marker.child("spPr"));
    let colors = Colors {
        fill:        write::fill(shape),
        line:        write::fill(shape.and_then(|shape| shape.child("ln"))),
        marker_fill: write::fill(marker_shape),
        marker_line: write::fill(marker_shape.and_then(|shape| shape.child("ln"))),
    };
    let place = Place::series(entry.index, colors);
    let fill = *cell(style.fill.get(entry.fill))?;
    if !matches!(fill, super::rule::ColorAction::Keep) {
        write::edit_child(series, "c:spPr", |shape| write::color(fill, shape, &place))?;
    }
    let line: &LineStyle = cell(style.line.get(entry.role).get(entry.line))?;
    if !is_kept_line(line) {
        let series_line = shape.and_then(|shape| shape.child("ln"));
        let marker_line = marker_shape.and_then(|shape| shape.child("ln"));
        write::edit_child(series, "c:spPr", |shape| {
            write::edit_child(shape, "a:ln", |outline| {
                write::color(line.color, outline, &place)?;
                write::width(line.width, outline, &place)?;
                write::dash(line.dash, outline, series_line, marker_line);
                write::carried_attribute(line.cap, outline, "cap");
                write::join(line.join, outline);
                Ok::<(), Unresolved>(())
            })
        })?;
    }
    let marker: &MarkerStyle = cell(style.marker.get(entry.marker))?;
    apply_marker(marker, series, &place, marker_shape)?;
    let flag = match before.val("smooth") {
        None => SmoothFlag::Absent,
        Some("1" | "true") => SmoothFlag::On,
        Some(_) => SmoothFlag::Off,
    };
    write::carried(
        *cell(style.smooth.get(entry.group_of, flag))?,
        series,
        "smooth",
    );
    write::carried(
        *cell(
            style
                .invert_if_negative
                .get(presence(before.child("invertIfNegative").is_some())),
        )?,
        series,
        "invertIfNegative",
    );
    let points = cell(
        style
            .points
            .get(entry.role)
            .get(entry.value_count())
            .get(presence(before.child("dPt").is_some())),
    )?;
    match (points, before.child("dPt").is_some()) {
        (super::rule::DataPointsRule::Keep, false) => (),
        (super::rule::DataPointsRule::Remove, _) => series.remove("dPt"),
        _ => return Err(Unresolved::Pending(Step::Points)),
    }
    if let Some(labels) = series.child_mut("dLbls") {
        let format = *cell(
            style
                .data_label_format
                .get(presence(labels.child("numFmt").is_some())),
        )?;
        write::number_format(format, labels)?;
    }
    let ids = TemplateIds { axis: 0, cross: 0 };
    let effects = *cell(style.effects.get(presence(
        shape.and_then(|shape| shape.child("effectLst")).is_some(),
    )))?;
    if !matches!(effects, Carried::Keep) {
        write::edit_child(series, "c:spPr", |shape| {
            write::carried_node(effects, shape, "effectLst", ids);
        });
    }
    let shape_3d = match shape.and_then(|shape| shape.child("sp3d")) {
        None => Shape3dClass::Absent,
        Some(sp3d) if sp3d.children.is_empty() && sp3d.attributes.is_empty() => Shape3dClass::Empty,
        Some(_) => Shape3dClass::Explicit,
    };
    match *cell(style.shape_3d.get(shape_3d))? {
        Shape3dRule::Keep => (),
        Shape3dRule::Remove => {
            if let Some(shape) = series.child_mut("spPr") {
                shape.remove("sp3d");
            }
        }
        _ => return Err(Unresolved::Pending(Step::Shape3d)),
    }
    let marker_effects_rule = *cell(
        style.marker_effects.get(presence(
            marker_shape
                .and_then(|shape| shape.child("effectLst"))
                .is_some(),
        )),
    )?;
    if !matches!(marker_effects_rule, Carried::Keep) {
        if let Some(marker) = series.child_mut("marker") {
            write::edit_child(marker, "c:spPr", |shape| {
                write::carried_node(marker_effects_rule, shape, "effectLst", ids);
            });
        }
    }
    if let Some(marker) = series.child_mut("marker") {
        drop_if_emptied(marker, marker_shape);
    }
    Ok(())
}

const fn is_kept_line(line: &LineStyle) -> bool {
    matches!(line.color, super::rule::ColorAction::Keep)
        && matches!(line.width, super::rule::WidthRule::Keep)
        && matches!(line.dash, super::rule::DashAction::Keep)
        && matches!(line.cap, Carried::Keep)
        && matches!(line.join, Carried::Keep)
}

fn apply_marker(
    style: &MarkerStyle,
    series: &mut Element,
    place: &Place<'_>,
    before_shape: Option<&Element>,
) -> Result<(), Unresolved> {
    write::carried_presence(style.presence, series, "marker");
    let Some(marker) = series.child_mut("marker") else {
        return Ok(());
    };
    write::carried(style.symbol, marker, "symbol");
    write::carried(style.size, marker, "size");
    let styled = !matches!(style.fill, super::rule::ColorAction::Keep)
        || !matches!(style.line, super::rule::ColorAction::Keep)
        || !matches!(style.width, Carried::Keep)
        || !matches!(style.dash, super::rule::DashAction::Keep);
    if !styled {
        return Ok(());
    }
    let marker_line = before_shape.and_then(|shape| shape.child("ln"));
    write::edit_child(marker, "c:spPr", |shape| {
        write::color(style.fill, shape, place)?;
        write::edit_child(shape, "a:ln", |line| {
            write::color(style.line, line, place)?;
            write::carried_attribute(style.width, line, "w");
            write::dash(style.dash, line, None, marker_line);
            Ok::<(), Unresolved>(())
        })?;
        // A marker outline the conversion empties goes (section 3.3).
        if shape.child("ln").is_some_and(is_empty_line) {
            shape.remove("ln");
        }
        Ok(())
    })
}

/// The series Excel writes for one source series.
#[derive(Debug)]
pub(super) struct Patch {
    /// The series' position in the source, which is its position in the
    /// detached groups.
    pub(super) position: usize,
    /// The series as the result holds it; `None` when it does not change.
    pub(super) resolved: Option<AreaChartSeries>,
}

/// Takes out of `series`, the `c:ser` the source wrote as the plan edited it,
/// the children of the parts the target family's cleanup removed from the
/// stored series. A stored error bar or extension is a written child only
/// when the source family's writer emitted it.
pub(super) fn remove_cleaned(series: &mut Element, cleanup: &Cleanup) {
    let mut error_bars = Vec::new();
    let mut extensions = Vec::new();
    for &(kind, occurrence) in &cleanup.removed {
        match kind {
            SeriesPartKind::ErrorBars => {
                error_bars.extend(cleanup.error_bars.iter().position(|at| *at == occurrence));
            }
            SeriesPartKind::Extensions => {
                extensions.extend(cleanup.extensions.iter().position(|at| *at == occurrence));
            }
            // Every other kind, repeated ones included, goes whole.
            kind => {
                if let Some((name, _)) = PARTS.iter().find(|(_, part)| *part == kind) {
                    series.remove(name);
                }
            }
        }
    }
    remove_written(series, "errBars", &error_bars);
    if cleanup
        .removed
        .iter()
        .any(|(kind, _)| *kind == SeriesPartKind::Extensions)
    {
        if let Some(list) = series.child_mut("extLst") {
            remove_written(list, "ext", &extensions);
        }
        if cleanup.extensions_left == 0 {
            // The cleanup emptied the stored list, which is written as it is
            // now whether or not the source wrote it.
            match &cleanup.emptied {
                Some(list) => series.put(list.clone()),
                None => series.remove("extLst"),
            }
        } else if series
            .child("extLst")
            .is_some_and(|list| list.child("ext").is_none())
        {
            // The writer leaves out a stored list none of whose entries it
            // emits.
            series.remove("extLst");
        }
    }
}

/// Removes the children `local` of `owner` at `positions` among them.
fn remove_written(owner: &mut Element, local: &str, positions: &[usize]) {
    let mut at = 0;
    owner.children.retain(|child| {
        if child.local() != local {
            return true;
        }
        at += 1;
        !positions.contains(&(at - 1))
    });
}

fn children<'a>(series: &'a Element, name: &str) -> Vec<&'a Element> {
    series
        .children
        .iter()
        .filter(|child| child.local() == name)
        .collect()
}

/// The result of every source series, resolved against its `c:ser` in
/// `area`. `live` holds umya's series in source order.
pub(super) fn patches(
    source: &Source,
    data: &DataRules,
    area: &Element,
    live: &[&AreaChartSeries],
) -> Result<Vec<Patch>, Unresolved> {
    let after_series: Vec<&Element> = area
        .children
        .iter()
        .filter(|group| group.local().ends_with("Chart"))
        .flat_map(|group| group.children_named("ser"))
        .collect();
    let session = SourceCacheSession::deferred();
    let mut out = Vec::new();
    for (position, entry) in source.series.iter().enumerate() {
        let index = entry.index.to_string();
        let after = after_series
            .iter()
            .find(|series| series.val("idx") == Some(index.as_str()))
            .ok_or(Unresolved::Rejected)?;
        let before = &entry.element;
        for name in ["idx", "order", "tx"] {
            if children(before, name) != children(after, name) {
                return Err(Unresolved::Pending(Step::UngovernedSeriesPart));
            }
        }
        let moves = role_moves(data, before)?;
        let moved = |name: &str| {
            moves
                .iter()
                .any(|role| role_name(role.to) == name || role_name(role.from) == name)
        };
        let mut removed = Vec::new();
        let mut patch = Element::new("c:ser");
        for (name, kind) in PARTS {
            let (was, now) = (children(before, name), children(after, name));
            if was == now || moved(name) {
                continue;
            }
            if name == "extLst" {
                return Err(Unresolved::Pending(Step::UngovernedSeriesPart));
            }
            let literal_sizes =
                name == "bubbleSize" && now.iter().all(|size| size.child("numLit").is_some());
            if ROLES.contains(&name) && !now.is_empty() && !literal_sizes {
                return Err(Unresolved::Rejected);
            }
            if !was.is_empty() {
                removed.push(kind);
            }
            patch.children.extend(now.into_iter().cloned());
        }
        let remove_shape = match (before.child("spPr"), after.child("spPr")) {
            (was, now) if was == now => false,
            (_, None) => true,
            (_, Some(shape)) => {
                patch.children.push(shape.clone());
                false
            }
        };
        let known = |child: &Element| {
            PARTS.iter().any(|(name, _)| *name == child.local())
                || matches!(child.local(), "idx" | "order" | "tx" | "spPr")
        };
        if after.children.iter().any(|child| !known(child)) {
            return Err(Unresolved::Pending(Step::UngovernedSeriesPart));
        }
        let unchanged =
            moves.is_empty() && removed.is_empty() && patch.children.is_empty() && !remove_shape;
        let resolved = if unchanged {
            None
        } else {
            let mut series = (*live.get(position).ok_or(Unresolved::Rejected)?).clone();
            for role in &moves {
                let keep_source = after.child(role_name(role.from)).is_some();
                move_role(&mut series, *role, keep_source)?;
            }
            drop(series.remove_parts(&removed));
            if remove_shape {
                series.remove_shape_properties();
            }
            if !patch.children.is_empty() {
                xml::read(&patch, |reader, start, _| {
                    series.set_attributes(reader, start, &session)
                })
                .ok_or(Unresolved::Rejected)?;
            }
            Some(series)
        };
        out.push(Patch { position, resolved });
    }
    Ok(out)
}
