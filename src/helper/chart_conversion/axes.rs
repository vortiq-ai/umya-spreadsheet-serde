//! Section 6's axes: one neutral value per axis, axis-type conversion, axis
//! creation from the generated templates, and the id topology.
use super::{
    AxisRole,
    ChartSubtype,
    model,
    plan::{
        Replay,
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
        GroupRule,
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
        TemplateIds,
        instantiate,
    },
};
use crate::{
    reader::xlsx::SourceCacheSession,
    structs::drawing::charts::{
        AxisId,
        CategoryAxis,
        DateAxis,
        PlotArea,
        SeriesAxis,
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
    /// Created from a generated template.
    Created(&'static AxisTemplate),
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
            Origin::Created(template) => instantiate(
                &template.root,
                TemplateIds {
                    axis:  self.id,
                    cross: cross.unwrap_or_default(),
                },
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
    let categories = source.categories(xy);
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
        // one rule in every category cell.
        let (data, spacing) = if matches!(rule.key.role, AxisRole::Category) {
            categories
        } else {
            (CategoryDataClass::Absent, DateSpacing::Absent)
        };
        let origin = match *cell(rule.outcome.get(class))? {
            AxisOutcome::Absent => continue,
            AxisOutcome::Present(spec) => {
                let mut element = before.ok_or(Unresolved::Rejected)?.clone();
                edit(&mut element, spec, pair, class, (data, spacing), replay)?;
                Origin::Kept(element)
            }
            AxisOutcome::Created => {
                // A created axis of any role is selected by the source's
                // style and category data. Monthly `c:cat` categories select
                // the monthly tree when Excel created a date axis for them;
                // for a category axis they are filed with the ordinary trees.
                let by_creation = cell(pair.created.get(presence(source.styled)))?;
                let ordinary = || cell(by_creation.get(CreationClass::Ordinary).get(categories.0));
                let template = if source.category.1 == DateSpacing::Monthly {
                    cell(by_creation.get(CreationClass::Monthly).get(categories.0))
                        .or_else(|_| ordinary())?
                } else {
                    ordinary()?
                };
                Origin::Created(template)
            }
        };
        let id = match &origin {
            Origin::Kept(element) => element
                .val("axId")
                .and_then(|id| id.parse().ok())
                .ok_or(Unresolved::Rejected)?,
            Origin::Created(_) => {
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
    write::carried(
        by_categories(&spec.base_time_unit, data, spacing)?,
        axis,
        "baseTimeUnit",
    );
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
        (super::rule::Carried::Keep, _) => (),
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

/// The axis elements in the order the writer groups them.
const AXIS_NAMES: [&str; 4] = ["catAx", "dateAx", "valAx", "serAx"];

fn axis_names(area: &Element) -> Vec<&str> {
    area.children
        .iter()
        .map(Element::local)
        .filter(|name| AXIS_NAMES.contains(name))
        .collect()
}

/// Whether umya's writer puts the planned axes in the planned order. A plot
/// area writes its axes in the order its part was read with (none when that
/// is the grouped order), then every other axis grouped by kind; the source
/// as written shows that order.
pub(super) fn written_in_order(source_area: &Element, planned_area: &Element) -> bool {
    let rank = |name: &&str| AXIS_NAMES.iter().position(|axis| axis == name);
    let mut recorded = axis_names(source_area);
    if recorded.is_sorted_by_key(rank) {
        recorded.clear();
    }
    let planned = axis_names(planned_area);
    let mut left = planned.clone();
    let mut written = Vec::new();
    for kind in recorded {
        if let Some(at) = left.iter().position(|name| *name == kind) {
            written.push(left.remove(at));
        }
    }
    left.sort_by_key(rank);
    written.extend(left);
    written == planned
}

/// Whether every series axis of `planned_area` crosses as umya writes it:
/// umya's series axis always writes `c:crosses` unless it holds `c:crossesAt`.
pub(super) fn crossings_writable(planned_area: &Element) -> bool {
    planned_area
        .children_named("serAx")
        .all(|axis| axis.child("crosses").is_some() || axis.child("crossesAt").is_some())
}

/// The axes of the result as umya's own axis structs.
#[derive(Debug, Default)]
pub(super) struct PlannedAxes {
    pub(super) category: Vec<CategoryAxis>,
    pub(super) date:     Vec<DateAxis>,
    pub(super) value:    Vec<ValueAxis>,
    pub(super) series:   Vec<SeriesAxis>,
}

/// Reads the result's axes through umya's axis readers; a kept axis keeps
/// the extension list of its live axis in `live`.
pub(super) fn read_axes(area: &Element, live: &PlotArea) -> Result<PlannedAxes, Unresolved> {
    let session = SourceCacheSession::deferred();
    let mut axes = PlannedAxes::default();
    for element in &area.children {
        if !AXIS_NAMES.contains(&element.local()) {
            continue;
        }
        // umya reads an extension list only within the part it came from: an
        // axis that keeps one takes it from the live axis with its id.
        let mut skeleton = element.clone();
        let kept = if skeleton.child("extLst").is_some() {
            skeleton.remove("extLst");
            let id = element.val("axId").and_then(|id| id.parse::<u32>().ok());
            let same = |axis: &AxisId| Some(axis.val()) == id;
            let kept = match element.local() {
                "catAx" => live
                    .category_axis()
                    .iter()
                    .find(|axis| same(axis.axis_id()))
                    .and_then(CategoryAxis::extensions),
                "dateAx" => live
                    .date_axis()
                    .iter()
                    .find(|axis| same(axis.axis_id()))
                    .and_then(DateAxis::extensions),
                "valAx" => live
                    .value_axis()
                    .iter()
                    .find(|axis| same(axis.axis_id()))
                    .and_then(ValueAxis::extensions),
                "serAx" => live
                    .series_axis()
                    .iter()
                    .find(|axis| same(axis.axis_id()))
                    .and_then(SeriesAxis::extensions),
                _ => None,
            };
            Some(kept.ok_or(Unresolved::Rejected)?.clone())
        } else {
            None
        };
        let read = match element.local() {
            "catAx" => {
                let mut axis = CategoryAxis::default();
                let read = xml::read(&skeleton, |reader, start, _| {
                    axis.set_attributes(reader, start, &session)
                });
                axis.set_extensions(kept);
                axes.category.push(axis);
                read
            }
            "dateAx" => {
                let mut axis = DateAxis::default();
                let read = xml::read(&skeleton, |reader, start, _| {
                    axis.set_attributes(reader, start, &session)
                });
                axis.set_extensions(kept);
                axes.date.push(axis);
                read
            }
            "valAx" => {
                let mut axis = ValueAxis::default();
                let read = xml::read(&skeleton, |reader, start, _| {
                    axis.set_attributes(reader, start, &session)
                });
                axis.set_extensions(kept);
                axes.value.push(axis);
                read
            }
            "serAx" => {
                let mut axis = SeriesAxis::default();
                let read = xml::read(&skeleton, |reader, start, _| {
                    axis.set_attributes(reader, start, &session)
                });
                axis.set_extensions(kept);
                axes.series.push(axis);
                read
            }
            _ => Some(()),
        };
        read.ok_or(Unresolved::Rejected)?;
    }
    Ok(axes)
}
