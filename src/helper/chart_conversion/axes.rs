//! Section 6's axes: one neutral value per axis, axis-type conversion, axis
//! creation from the generated templates, and the id topology.
use super::{
    AxisRole,
    ChartSubtype,
    build::Owner,
    model,
    plan::{
        Replay,
        Step,
        Unresolved,
        cell,
        presence,
    },
    rule::{
        AxisKey,
        AxisOutcome,
        AxisPairRule,
        AxisSet,
        AxisSpec,
        AxisTemplate,
        AxisType,
        Carried,
        GroupRule,
        KindRule,
        SeriesSelection,
        TimeUnit,
        TimeUnitRule,
    },
    slots::{
        AxisClass,
        ByCategoryData,
        ByDateSpacing,
        CategoryDataClass,
        CreationClass,
        DateSpacing,
    },
    write,
    xml::{
        self,
        Element,
        SourceValues,
        TemplateIds,
        instantiate,
    },
};
use crate::{
    reader::xlsx::SourceCacheSession,
    structs::drawing::charts::{
        CategoryAxis,
        DateAxis,
        MajorGridlines,
        MinorGridlines,
        PlotArea,
        RestoreReport,
        SeriesAxis,
        ShapeProperties,
        Title,
        ValueAxis,
    },
};

/// Children every axis type has.
const COMMON: [&str; 17] = [
    "axId",
    "scaling",
    "delete",
    "axPos",
    "majorGridlines",
    "minorGridlines",
    "title",
    "numFmt",
    "majorTickMark",
    "minorTickMark",
    "tickLblPos",
    "spPr",
    "txPr",
    "crossAx",
    "crosses",
    "crossesAt",
    "extLst",
];

/// Children only some axis types have.
const fn own_children(axis_type: AxisType) -> &'static [&'static str] {
    match axis_type {
        AxisType::Category => &[
            "auto",
            "lblAlgn",
            "lblOffset",
            "tickLblSkip",
            "tickMarkSkip",
            "noMultiLvlLbl",
        ],
        AxisType::Value => &["crossBetween", "majorUnit", "minorUnit", "dispUnits"],
        AxisType::Date => &[
            "auto",
            "lblOffset",
            "baseTimeUnit",
            "majorUnit",
            "majorTimeUnit",
            "minorUnit",
            "minorTimeUnit",
        ],
        AxisType::Series => &["tickLblSkip", "tickMarkSkip"],
    }
}

const fn tag(axis_type: AxisType) -> &'static str {
    match axis_type {
        AxisType::Category => "c:catAx",
        AxisType::Value => "c:valAx",
        AxisType::Date => "c:dateAx",
        AxisType::Series => "c:serAx",
    }
}

fn class_of(axis: Option<&Element>) -> AxisClass {
    match axis.map(Element::local) {
        Some("catAx") => AxisClass::Category,
        Some("dateAx") => AxisClass::Date,
        Some("valAx") => AxisClass::Value,
        Some("serAx") => AxisClass::Series,
        _ => AxisClass::Absent,
    }
}

/// The position of the series deciding axis set `set` of the target: the
/// first series of the target group on that set.
fn decider(kind: &KindRule, set: AxisSet) -> usize {
    kind.groups
        .iter()
        .find(|group| secondary(group.axes) == secondary(set))
        .map_or(0, |group| {
            usize::from(matches!(group.series, SeriesSelection::AfterFirst))
        })
}

const fn secondary(set: AxisSet) -> bool {
    matches!(set, AxisSet::Secondary)
}

fn same_key(left: AxisKey, right: AxisKey) -> bool {
    left.role == right.role && secondary(left.set) == secondary(right.set)
}

/// The axis an axis of `role` crosses in its set.
const fn partner(role: AxisRole) -> AxisRole {
    match role {
        AxisRole::Category | AxisRole::Series => AxisRole::Value,
        AxisRole::Value => AxisRole::Category,
    }
}

/// One axis of the result, whatever its type: the neutral value section 6
/// converts through, so no axis type needs a function per other type.
#[derive(Clone, Debug)]
pub(super) struct AxisCore {
    pub(super) key: AxisKey,
    pub(super) id:  u32,
    origin:         Origin,
}

#[derive(Clone, Debug)]
enum Origin {
    /// The source's axis, edited.
    Kept(Element),
    /// Created from a generated template, with what it copies from the
    /// deciding categories.
    Created(&'static AxisTemplate, Option<SourceValues>),
}

impl AxisCore {
    /// The axis as `axis_type`: the element is renamed and keeps only the
    /// children that type has.
    fn retype(element: &mut Element, axis_type: AxisType) {
        let own = own_children(axis_type);
        tag(axis_type).clone_into(&mut element.name);
        element
            .children
            .retain(|child| COMMON.contains(&child.local()) || own.contains(&child.local()));
    }

    fn element(&self, cross: Option<u32>) -> Element {
        match &self.origin {
            Origin::Kept(element) => {
                let mut element = element.clone();
                element.put(Element::value("axId", &self.id.to_string()));
                if let Some(cross) = cross {
                    element.put(Element::value("crossAx", &cross.to_string()));
                }
                element
            }
            Origin::Created(template, source) => instantiate(
                &template.root,
                TemplateIds {
                    axis:  self.id,
                    cross: cross.unwrap_or_default(),
                },
                source.as_ref(),
            ),
        }
    }
}

/// The axes of the result with their ids allocated.
#[derive(Debug)]
pub(super) struct AxisLayout {
    cores: Vec<AxisCore>,
}

impl AxisLayout {
    fn id(&self, role: AxisRole, secondary_set: bool) -> Option<u32> {
        self.cores
            .iter()
            .find(|core| core.key.role == role && secondary(core.key.set) == secondary_set)
            .map(|core| core.id)
    }

    /// The axes as elements, each crossing its partner in its set.
    pub(super) fn into_elements(self) -> impl Iterator<Item = Element> {
        let crossings: Vec<Option<u32>> = self
            .cores
            .iter()
            .map(|core| self.id(partner(core.key.role), secondary(core.key.set)))
            .collect();
        self.cores
            .into_iter()
            .zip(crossings)
            .map(|(core, cross)| core.element(cross))
    }

    /// A group's `c:axId` children: category, value and series axis of its
    /// set, then the measured `0` sentinel where Excel writes one.
    pub(super) fn group_references(&self, rule: &GroupRule) -> Result<Vec<Element>, Unresolved> {
        let set = match rule.axes {
            AxisSet::Unattached => return Ok(Vec::new()),
            AxisSet::Primary => false,
            AxisSet::Secondary => true,
        };
        let category = self
            .id(AxisRole::Category, set)
            .ok_or(Unresolved::Rejected)?;
        let value = self.id(AxisRole::Value, set).ok_or(Unresolved::Rejected)?;
        let ids = [Some(category), Some(value), self.id(AxisRole::Series, set)]
            .into_iter()
            .flatten()
            .chain(rule.axis_id_placeholder.then_some(0));
        Ok(ids
            .map(|id| Element::value("axId", &id.to_string()))
            .collect())
    }
}

/// The axes Excel writes, each from the source's axis or from a template.
pub(super) fn plan_axes(replay: &Replay<'_>) -> Result<AxisLayout, Unresolved> {
    let source = replay.source;
    let area = source.chart.child("plotArea").ok_or(Unresolved::Rejected)?;
    let sources = model::axes_of(&source.chart);
    let xy = matches!(
        replay.target.chart_type(),
        crate::structs::ChartType::ScatterChart | crate::structs::ChartType::BubbleChart
    );
    let mut next = area
        .children
        .iter()
        .flat_map(|child| child.children_named("axId"))
        .filter_map(|id| id.attribute("val")?.parse::<u32>().ok())
        .max()
        .unwrap_or_default();
    let mut cores = Vec::new();
    for (index, rule) in replay.kind.axes.iter().enumerate() {
        let before = sources
            .iter()
            .find(|(role, set, _)| *role == rule.key.role && *set == secondary(rule.key.set))
            .map(|(_, _, axis)| *axis);
        let class = class_of(before);
        let pair = replay
            .conversion
            .axes
            .get(index)
            .filter(|pair| same_key(pair.key, rule.key))
            .ok_or(Unresolved::Rejected)?;
        // Category data selects only category-role fields; the others hold
        // one rule in every category cell. Each axis takes its class from the
        // series deciding its set in the target.
        let decider = decider(replay.kind, rule.key.set);
        let (data, spacing) = if matches!(rule.key.role, AxisRole::Category) {
            source.categories(xy, decider)
        } else {
            (CategoryDataClass::Absent, DateSpacing::Absent)
        };
        let origin = match *cell(rule.outcome.get(class))? {
            AxisOutcome::Absent => continue,
            AxisOutcome::Present(spec) => {
                let mut element = before.ok_or(Unresolved::Rejected)?.clone();
                let copied = source.copied(decider);
                edit(
                    &mut element,
                    spec,
                    pair,
                    class,
                    (data, spacing),
                    copied,
                    replay,
                )?;
                Origin::Kept(element)
            }
            AxisOutcome::Created => {
                // A created axis of any role is selected by the source's
                // style and its creation class. Monthly categories select the
                // monthly tree only when Excel created a date axis for them;
                // otherwise they are filed with the ordinary trees.
                let by_creation = cell(pair.created.get(presence(source.styled)))?;
                let created = source.creation_class(xy, decider);
                let monthly = (source.creation.spacing == DateSpacing::Monthly)
                    .then(|| cell(by_creation.get(CreationClass::Monthly).get(created)).ok())
                    .flatten()
                    .filter(|template| template.root.name == "dateAx");
                let template = match monthly {
                    Some(template) => template,
                    None => cell(by_creation.get(CreationClass::Ordinary).get(created))?,
                };
                let values = matches!(rule.key.role, AxisRole::Category)
                    .then(|| source.copied(decider))
                    .flatten()
                    .cloned();
                // A template that copies what no round measured of these
                // categories, such as the code Excel rebuilds for cells in a
                // built-in format with no measured code, waits for a round.
                if !SourceValues::fill(values.as_ref(), &template.root) {
                    return Err(Unresolved::Pending(Step::UnmeasuredCategoryClass));
                }
                Origin::Created(template, values)
            }
        };
        let id = match &origin {
            Origin::Kept(element) => element
                .val("axId")
                .and_then(|id| id.parse().ok())
                .ok_or(Unresolved::Rejected)?,
            Origin::Created(..) => {
                next = next.checked_add(1).ok_or(Unresolved::Rejected)?;
                next
            }
        };
        cores.push(AxisCore {
            key: rule.key,
            id,
            origin,
        });
    }
    // Excel's axis order, as every admitted pair measures it: set by set, the
    // category axis, the value axis, then the series axis. The secondary set,
    // and a bar chart whose axes are all created, write the value axis first.
    let created_bar = sources.is_empty()
        && matches!(
            replay.target,
            ChartSubtype::ClusteredBar
                | ChartSubtype::StackedBar
                | ChartSubtype::PercentStackedBar
                | ChartSubtype::ClusteredBar3D
                | ChartSubtype::StackedBar3D
                | ChartSubtype::PercentStackedBar3D
        );
    cores.sort_by_key(|core| {
        let value_first = secondary(core.key.set) || created_bar;
        let rank = match (core.key.role, value_first) {
            (AxisRole::Category, false) | (AxisRole::Value, true) => 0,
            (AxisRole::Value, false) | (AxisRole::Category, true) => 1,
            (AxisRole::Series, _) => 2,
        };
        (secondary(core.key.set), rank)
    });
    Ok(AxisLayout { cores })
}

/// The rule of a category-role field for the categories' data and spacing.
fn by_categories<T: Copy>(
    rule: &ByCategoryData<ByDateSpacing<T>>,
    data: CategoryDataClass,
    spacing: DateSpacing,
) -> Result<T, Unresolved> {
    cell(cell(rule.get(data))?.get(spacing)).copied()
}

/// An existing axis after its KIND spec and its PAIR rule.
fn edit(
    axis: &mut Element,
    spec: &AxisSpec,
    pair: &AxisPairRule,
    class: AxisClass,
    (data, spacing): (CategoryDataClass, DateSpacing),
    copied: Option<&SourceValues>,
    replay: &Replay<'_>,
) -> Result<(), Unresolved> {
    AxisCore::retype(axis, by_categories(&spec.axis_type, data, spacing)?);
    write::edit_child(axis, "c:scaling", |scaling| {
        write::carried(spec.orientation, scaling, "orientation");
        write::retained(spec.minimum, scaling, "min");
        write::retained(spec.maximum, scaling, "max");
        write::retained(spec.log_base, scaling, "logBase");
    });
    write::carried(spec.deleted, axis, "delete");
    write::carried(spec.position, axis, "axPos");
    write::retained(spec.major_unit, axis, "majorUnit");
    write::retained(spec.minor_unit, axis, "minorUnit");
    write::retained(spec.crosses, axis, "crosses");
    write::retained(spec.crosses_at, axis, "crossesAt");
    write::carried(spec.cross_between, axis, "crossBetween");
    match (spec.tick_labels, &pair.tick_labels) {
        (Some(rule), _) => write::carried(rule, axis, "tickLblPos"),
        (None, Some(rule)) => write::carried(*cell(rule.get(class))?, axis, "tickLblPos"),
        (None, None) => (),
    }
    write::retained(spec.title, axis, "title");
    write::carried(spec.auto, axis, "auto");
    write::carried(
        by_categories(&spec.label_align, data, spacing)?,
        axis,
        "lblAlgn",
    );
    write::carried(spec.label_offset, axis, "lblOffset");
    write::carried(
        by_categories(&spec.no_multi_level, data, spacing)?,
        axis,
        "noMultiLvlLbl",
    );
    let unit = match by_categories(&spec.base_time_unit, data, spacing)? {
        TimeUnitRule::Carried(rule) => rule,
        TimeUnitRule::FromSource => Carried::Set(
            match copied
                .and_then(|copied| copied.unit)
                .ok_or(Unresolved::Rejected)?
            {
                "years" => TimeUnit::Years,
                "months" => TimeUnit::Months,
                _ => TimeUnit::Days,
            },
        ),
    };
    write::carried(unit, axis, "baseTimeUnit");
    write::carried(spec.major_ticks, axis, "majorTickMark");
    write::carried(spec.minor_ticks, axis, "minorTickMark");
    write::retained(spec.style, axis, "spPr");
    let format = cell(cell(cell(pair.number_format.get(class))?.get(data))?.get(spacing))?;
    write::number_format(*format, axis)?;
    let style = replay.source.style();
    let grid = *cell(
        pair.major_grid
            .get(style)
            .get(presence(axis.child("majorGridlines").is_some())),
    )?;
    write::carried_node(
        grid,
        axis,
        "majorGridlines",
        TemplateIds { axis: 0, cross: 0 },
    );
    // The title's rotation, by its prior presence; the style must be one the
    // pair was measured with.
    let rotated = title_body(axis).is_some_and(|body| body.attribute("rot").is_some());
    let rotation = cell(pair.title_rotation.get(presence(rotated)))?;
    if !*rotation.styles.get(style) {
        return Err(Unresolved::Rejected);
    }
    match (rotation.action, title_body(axis)) {
        (Carried::Keep, _) => (),
        (action, Some(body)) => write::carried_attribute(action, body, "rot"),
        (_, None) => return Err(Unresolved::Rejected),
    }
    Ok(())
}

/// The text body of an axis title, where its rotation sits.
fn title_body(axis: &mut Element) -> Option<&mut Element> {
    axis.child_mut("title")?
        .child_mut("tx")?
        .child_mut("rich")?
        .child_mut("bodyPr")
}

/// The axis elements of a plot area.
const AXIS_NAMES: [&str; 4] = ["catAx", "dateAx", "valAx", "serAx"];

/// One axis of the result as umya's axis structs hold it.
#[derive(Debug)]
pub(super) enum AnyAxis {
    Category(CategoryAxis),
    Date(DateAxis),
    Value(ValueAxis),
    Series(SeriesAxis),
}

/// `$body` with `$axis` bound to the axis struct, whatever its type.
macro_rules! each {
    ($any:expr, $axis:ident => $body:expr) => {
        match $any {
            AnyAxis::Category($axis) => $body,
            AnyAxis::Date($axis) => $body,
            AnyAxis::Value($axis) => $body,
            AnyAxis::Series($axis) => $body,
        }
    };
}

impl AnyAxis {
    pub(super) fn id(&self) -> u32 {
        each!(self, axis => axis.axis_id().val())
    }

    /// The axis `element` names, read by its type's reader without the
    /// extension lists, which umya reads only within their part.
    fn read(element: &Element, session: &SourceCacheSession) -> Option<Self> {
        let element = xml::without_extensions(element);
        macro_rules! read_as {
            ($kind:ident, $variant:ident) => {{
                let mut axis = $kind::default();
                xml::read(&element, |reader, start, _| {
                    axis.set_attributes(reader, start, session)
                })?;
                Self::$variant(axis)
            }};
        }
        Some(match element.local() {
            "catAx" => read_as!(CategoryAxis, Category),
            "dateAx" => read_as!(DateAxis, Date),
            "valAx" => read_as!(ValueAxis, Value),
            "serAx" => read_as!(SeriesAxis, Series),
            _ => return None,
        })
    }

    /// Restores into this axis, read from a planned element, the stored
    /// state of `previous` that its writer leaves out, through the owners'
    /// own restorers.
    pub(super) fn restore(&mut self, previous: &Self) -> Option<()> {
        let mut report = RestoreReport::default();
        match (&mut *self, previous) {
            (Self::Category(axis), Self::Category(old)) => {
                axis.restore_export_parts(old, &mut report).ok()?;
            }
            (Self::Date(axis), Self::Date(old)) => {
                axis.restore_export_parts(old, &mut report).ok()?;
            }
            (Self::Value(axis), Self::Value(old)) => {
                axis.restore_export_parts(old, &mut report).ok()?;
            }
            (Self::Series(axis), Self::Series(old)) => {
                axis.restore_export_parts(old, &mut report).ok()?;
            }
            (any, old) => {
                // No restorer crosses axis types: the axis' own extension
                // list moves whole, and the parts both types have restore
                // through their own owners.
                let extensions = each!(old, axis => axis.extensions().cloned());
                each!(any, axis => {
                    axis.set_extensions(extensions);
                });
                let scaling = each!(old, axis => axis.scaling());
                each!(any, axis => axis.scaling_mut().restore_export_parts(scaling, &mut report));
                let format = each!(any, axis => axis.numbering_format_mut());
                format.discard_incoming_export_parts();
                format.restore_export_parts(each!(old, axis => axis.numbering_format()));
                let old_text = each!(old, axis => axis.text_properties());
                if let (Some(text), Some(old_text)) =
                    (each!(any, axis => axis.text_properties_mut()), old_text)
                {
                    text.discard_incoming_export_parts();
                    if text.projected_structure_matches(old_text) {
                        for (paragraph, old) in
                            text.paragraph_mut().iter_mut().zip(old_text.paragraph())
                        {
                            paragraph.restore_break_field_ranks(old);
                        }
                        text.restore_export_parts(old_text).ok()?;
                    }
                }
            }
        }
        // Title, shape and gridlines, which the axis restorers leave to the
        // chart's own traversal.
        let parts = each!(self, axis => axis.reference_role_parts_mut());
        let old = each!(previous, axis => axis.reference_role_parts());
        if let (Some(title), Some(old_title)) = (parts.title, old.title) {
            restore_title(title, old_title, &mut report)?;
        }
        restore_shape(parts.shape, old.shape);
        restore_shape(
            parts
                .gridlines
                .and_then(MajorGridlines::shape_properties_mut),
            old.gridlines.and_then(MajorGridlines::shape_properties),
        );
        restore_shape(
            parts
                .minor_gridlines
                .and_then(MinorGridlines::shape_properties_mut),
            old.minor_gridlines
                .and_then(MinorGridlines::shape_properties),
        );
        Some(())
    }
}

/// Restores the stored state of shape properties `previous` that the writer
/// leaves out, when both are there.
pub(super) fn restore_shape(
    current: Option<&mut ShapeProperties>,
    previous: Option<&ShapeProperties>,
) {
    if let (Some(current), Some(previous)) = (current, previous) {
        current.restore_export_parts(previous);
    }
}

/// An axis title's stored state: its extension list and text parts, its
/// text, shape and layout.
fn restore_title(title: &mut Title, previous: &Title, report: &mut RestoreReport) -> Option<()> {
    title.restore_export_parts(previous, report).ok()?;
    let (text, shape, layout, _) = title.reference_role_parts_mut();
    let (old_text, old_shape, old_layout, _) = previous.reference_role_parts();
    if let (Some(text), Some(old_text)) = (text, old_text) {
        if let (Some(rich), Some(old)) = (text.rich_text_mut(), old_text.rich_text()) {
            rich.restore_edited_export_parts(old, report).ok()?;
        }
        if let (Some(reference), Some(old)) =
            (text.string_reference_mut(), old_text.string_reference())
        {
            reference.restore_export_parts(old);
            let formula = reference.formula().address_str();
            let source = old
                .string_cache()
                .source()
                .filter(|source| source.formula() == formula)
                .cloned();
            reference.string_cache_mut().set_source(source);
        }
    }
    restore_shape(shape, old_shape);
    if let (Some(layout), Some(old)) = (layout, old_layout) {
        layout.restore_export_parts(old);
        if let (Some(manual), Some(old)) = (layout.manual_layout_mut(), old.manual_layout()) {
            manual.restore_export_parts(old);
        }
    }
    Some(())
}

/// The axes of the result in the order they are written: the element name
/// and id of each, and how the result gets it.
#[derive(Debug, Default)]
pub(super) struct PlannedAxes {
    pub(super) axes: Vec<(&'static str, u32, Owner<AnyAxis>)>,
}

/// Plans the result's axes from their elements in `area`. An axis whose id is
/// a live axis' replaces that axis, of whatever type: unchanged as written,
/// it is the live axis; otherwise it is read and takes the live axis' stored
/// state when staged. `source_area` is the source as written. Two live axes
/// with one id cannot be told apart.
pub(super) fn read_axes(
    area: &Element,
    source_area: &Element,
    live: &PlotArea,
) -> Result<PlannedAxes, Unresolved> {
    let session = SourceCacheSession::deferred();
    let live_ids: Vec<u32> = live
        .category_axis()
        .iter()
        .map(|axis| axis.axis_id().val())
        .chain(live.date_axis().iter().map(|axis| axis.axis_id().val()))
        .chain(live.value_axis().iter().map(|axis| axis.axis_id().val()))
        .chain(live.series_axis().iter().map(|axis| axis.axis_id().val()))
        .collect();
    let id_of = |element: &Element| element.val("axId").and_then(|id| id.parse::<u32>().ok());
    let read = |element: &Element| AnyAxis::read(element, &session).ok_or(Unresolved::Rejected);
    let mut axes = PlannedAxes::default();
    for element in &area.children {
        let Some(tag) = AXIS_NAMES.into_iter().find(|name| *name == element.local()) else {
            continue;
        };
        let id = id_of(element).ok_or(Unresolved::Rejected)?;
        let owner = match live_ids.iter().filter(|live| **live == id).count() {
            0 => Owner::Rebuilt {
                read:  read(element)?,
                probe: None,
            },
            1 => {
                let written = source_area
                    .children
                    .iter()
                    .find(|axis| AXIS_NAMES.contains(&axis.local()) && id_of(axis) == Some(id))
                    .ok_or(Unresolved::Rejected)?;
                if element == written {
                    Owner::Kept
                } else {
                    Owner::Rebuilt {
                        read:  read(element)?,
                        probe: Some(read(written)?),
                    }
                }
            }
            _ => return Err(Unresolved::Invalid),
        };
        axes.axes.push((tag, id, owner));
    }
    Ok(axes)
}
