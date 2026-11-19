//! Section 5.1: the only step that refuses.
//!
//! `plan` classifies the chart, applies the refusal order, and replays the
//! rule table on the chart as umya writes it: the result is the chart Excel
//! writes, as a tree. Everything the later steps need is read from that tree
//! here, through umya's own readers, so nothing after `plan` can fail and the
//! chart is untouched on every refusal.
use super::{
    ChartConversionRefusal,
    ChartSource,
    ChartSubtype,
    ComboLayout,
    ConversionChange,
    SeriesPaint,
    SeriesRequirement,
    axes::{
        self,
        PlannedAxes,
    },
    build::{
        self,
        Owner,
    },
    chart_at,
    commit,
    group_builder,
    model::{
        self,
        Model,
    },
    rule::{
        Conversion,
        GroupRule,
        GroupValuesRule,
        KindRule,
        PairRule,
        RotationY,
        SeriesSelection,
        SeriesStyle,
        SliceAngle,
    },
    rules::{
        KIND_RULES,
        PAIR_RULES,
    },
    series,
    series_count,
    slots::{
        Presence,
        RuleCell,
    },
    source::{
        self,
        Source,
    },
    subtype,
    write::{
        self,
        XmlText,
    },
    xml::{
        self,
        Element,
        TemplateIds,
    },
};
use crate::{
    Workbook,
    structs::drawing::charts::{
        AreaChartSeries,
        BackWall,
        Chart,
        ChartGroup,
        Floor,
        SeriesPartKind,
        SideWall,
        View3D,
    },
};

/// A step of the conversion that is not built yet, or a class Excel has not
/// been measured on yet. Each one refuses before the chart is touched, as
/// `Unsupported`; none is a refusal Excel makes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Step {
    /// Removing the title, legend or another chart-level child.
    ChartLevel,
    /// Writing the plot area's own shape properties.
    PlotAreaShape,
    /// Band formats of surface targets.
    Bands,
    /// Completing or rewriting data points.
    Points,
    /// Setting the direction of error bars.
    ErrorBarDirection,
    /// A 3-D shape derived from another value.
    Shape3d,
    /// A series child the table does not govern changes.
    UngovernedSeriesPart,
    /// x values of a class no Excel round has converted for this pair yet.
    UnmeasuredXClass,
    /// Categories of a class no Excel round has converted for this pair yet,
    /// or of mixed classes.
    UnmeasuredCategoryClass,
    /// A surface or of-pie source whose series and value counts no Excel
    /// round has converted yet.
    UnmeasuredShape,
    /// Series whose `c:order` differs from their order in the plot area.
    SeriesOrder,
    /// A reference to a whole column or row: umya's cell reader projects
    /// ranges between two complete cells only.
    WholeLineReference,
    /// The result as umya writes it differs from the planned chart.
    StagedDifference,
    /// A retained owner holds stored state its restorers cannot carry into
    /// the owner the plan reads.
    HiddenState,
}

/// Why a rule cell gives no result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Unresolved {
    /// Excel's result for this class was not measured.
    Rejected,
    /// The step is not built yet.
    Pending(Step),
    /// The input is one umya cannot write or project: a zero column or row, a
    /// reversed range, or a sheet that cannot be read. This says nothing
    /// about Excel.
    Invalid,
}

/// A measured cell's action.
pub(super) const fn cell<T>(cell: &RuleCell<T>) -> Result<&T, Unresolved> {
    match cell {
        RuleCell::Measured(value) => Ok(value),
        RuleCell::Rejected => Err(Unresolved::Rejected),
    }
}

pub(super) const fn presence(present: bool) -> Presence {
    if present {
        Presence::Present
    } else {
        Presence::Absent
    }
}

/// One plot group of the result, without its series: how it is built, the
/// ids of its axes, and the source positions of the series it takes, in
/// order.
#[derive(Debug)]
pub(super) struct PlannedGroup {
    pub(super) owner:  Owner<ChartGroup>,
    pub(super) axes:   Vec<u32>,
    pub(super) series: Vec<usize>,
}

/// The 3-D scene of the result, when it differs from the source's.
#[derive(Debug, Default)]
pub(super) struct PlannedScene {
    pub(super) view:      Option<View3D>,
    pub(super) floor:     Option<Owner<Floor>>,
    pub(super) side_wall: Option<Owner<SideWall>>,
    pub(super) back_wall: Option<Owner<BackWall>>,
}

/// The parts of the result `build` assembles.
#[derive(Debug)]
pub(super) struct Planned {
    pub(super) groups: Vec<PlannedGroup>,
    pub(super) axes:   PlannedAxes,
    pub(super) scene:  Option<PlannedScene>,
    pub(super) series: Vec<series::Patch>,
}

/// Everything the conversion does, resolved: the converted chart, written and
/// compared already, and its report. Borrows nothing from the chart.
#[derive(Debug)]
pub(super) struct Plan {
    pub(super) from:    ChartSource,
    pub(super) to:      ChartSubtype,
    pub(super) chart:   Chart,
    pub(super) changes: Vec<ConversionChange>,
    pub(super) paint:   Vec<SeriesPaint>,
}

/// Plans converting chart `chart` of the sheet at `sheet`, which reads its
/// cells from `workbook`, to `target`; `None` when it already is `target`.
/// The checks that need no cells come first; then the sheets the chart reads
/// are read, and nothing else in the workbook changes.
pub(super) fn plan(
    workbook: &mut Workbook,
    sheet: usize,
    chart: usize,
    target: ChartSubtype,
) -> Result<Option<Plan>, ChartConversionRefusal> {
    let found = chart_at(workbook, sheet, chart).ok_or(ChartConversionRefusal::ChartNotFound)?;
    let plot_area = found.plot_area();
    let Some(from) = subtype::classify(plot_area) else {
        return Err(if plot_area.chart_groups().nth(1).is_some() {
            ChartConversionRefusal::MultipleGroups
        } else {
            ChartConversionRefusal::Unsupported {
                from: None,
                to:   target,
            }
        });
    };
    if let Some(refusal) = target.series_requirement().refusal(series_count(plot_area)) {
        return Err(refusal);
    }
    if from == ChartSource::Subtype(target) {
        return Ok(None);
    }
    source::referenced_sheets(found, workbook)
        .and_then(|sheets| read_sheets(workbook, &sheets))
        .and_then(|()| {
            let found = chart_at(workbook, sheet, chart).ok_or(Unresolved::Invalid)?;
            replay(found, workbook, from, target)
        })
        .map(Some)
        .map_err(|_| ChartConversionRefusal::Unsupported {
            from: Some(from),
            to:   target,
        })
}

/// Reads the sheets at `sheets` that are not read yet.
fn read_sheets(workbook: &mut Workbook, sheets: &[usize]) -> Result<(), Unresolved> {
    for &index in sheets {
        workbook
            .try_read_sheet(index)
            .map_err(|_| Unresolved::Invalid)?;
    }
    Ok(())
}

fn subtype_index(subtype: ChartSubtype) -> Option<usize> {
    ChartSubtype::ALL.iter().position(|other| *other == subtype)
}

fn source_index(source: ChartSource) -> Option<usize> {
    match source {
        ChartSource::Subtype(subtype) => subtype_index(subtype),
        ChartSource::Combo(layout) => Some(
            ChartSubtype::COUNT
                + match layout {
                    ComboLayout::StackedAreaColumn => 0,
                    ComboLayout::ColumnLine => 1,
                    ComboLayout::ColumnLineSecondary => 2,
                    ComboLayout::ColumnColumnSecondary => 3,
                },
        ),
    }
}

/// The measured conversion for the source's shape.
fn conversion(source: &Source, target: ChartSubtype) -> Result<&'static Conversion, Unresolved> {
    let rule = source_index(source.from)
        .and_then(|row| PAIR_RULES.get(row))
        .and_then(|row| row.get(subtype_index(target)?))
        .ok_or(Unresolved::Rejected)?;
    match rule {
        PairRule::Same | PairRule::Unsupported => Err(Unresolved::Rejected),
        PairRule::Convert(conversion) => Ok(conversion),
        PairRule::ConvertByShape(shapes) => {
            let unmeasured = Unresolved::Pending(Step::UnmeasuredShape);
            let values = uniform_values(source).ok_or(unmeasured)?;
            let series = u8::try_from(source.series.len()).map_err(|_| unmeasured)?;
            let values = u8::try_from(values).map_err(|_| unmeasured)?;
            shapes
                .cells
                .iter()
                .find(|(shape, _)| shape.series == series && shape.values == values)
                .map(|(_, conversion)| *conversion)
                .ok_or(unmeasured)
        }
    }
}

/// The value count every series has, if they agree.
fn uniform_values(source: &Source) -> Option<usize> {
    let mut counts = source.series.iter().map(|entry| entry.values);
    let values = counts.next().flatten()?;
    counts.all(|count| count == Some(values)).then_some(values)
}

/// Section 4.2's measured shapes of the six restricted sources (series by
/// values, by style). Another shape was not measured yet.
fn restricted_shape_measured(source: &Source, target: ChartSubtype) -> bool {
    let ChartSource::Subtype(from) = source.from else {
        return true;
    };
    let extended = matches!(
        from,
        ChartSubtype::Surface3D | ChartSubtype::Contour | ChartSubtype::PieOfPie
    );
    let restricted = extended
        || matches!(
            from,
            ChartSubtype::WireframeSurface3D
                | ChartSubtype::WireframeContour
                | ChartSubtype::BarOfPie
        );
    if !restricted {
        return true;
    }
    let series = source.series.len();
    let values = uniform_values(source);
    if let SeriesRequirement::Exactly(required) = target.series_requirement() {
        return series == usize::from(required) && values == Some(6);
    }
    if values == Some(6) {
        return if extended || source.styled {
            (1..=8).contains(&series)
        } else {
            matches!(series, 3 | 4 | 5 | 7)
        };
    }
    if series != 1 || !matches!(values, Some(8 | 12 | 20)) {
        return false;
    }
    let pie_family = matches!(
        target,
        ChartSubtype::Pie
            | ChartSubtype::ExplodedPie
            | ChartSubtype::Pie3D
            | ChartSubtype::ExplodedPie3D
            | ChartSubtype::Doughnut
            | ChartSubtype::ExplodedDoughnut
            | ChartSubtype::PieOfPie
            | ChartSubtype::BarOfPie
    );
    source.styled || (extended && pie_family)
}

/// Section 5.1's step 5 and the source checks of step 6, after identity and
/// before anything is replayed: one validated series mapping, and the
/// measured source shapes.
fn admit(
    live: &[&AreaChartSeries],
    source: &Source,
    target: ChartSubtype,
) -> Result<(), Unresolved> {
    // The writer leaves out categories holding both numeric sources, and the
    // reader refuses them: no rule can read such a series.
    if live.iter().any(|series| {
        series.category_axis_data().is_some_and(|categories| {
            categories.number_reference().is_some() && categories.number_literal().is_some()
        })
    }) {
        return Err(Unresolved::Invalid);
    }
    if source.series.is_empty() {
        return Err(Unresolved::Rejected);
    }
    let surface = matches!(
        source.from,
        ChartSource::Subtype(
            ChartSubtype::Surface3D
                | ChartSubtype::WireframeSurface3D
                | ChartSubtype::Contour
                | ChartSubtype::WireframeContour
        )
    );
    if surface
        && source
            .group(0)
            .and_then(|group| group.child("bandFmts"))
            .is_none()
    {
        return Err(Unresolved::Rejected);
    }
    // Every slot is keyed by `c:idx`: each series has its own, umya holds the
    // series in the order its writer emits them, and that order is `c:order`.
    let indexes: Vec<u32> = source.series.iter().map(|entry| entry.index).collect();
    if indexes
        .iter()
        .enumerate()
        .any(|(at, index)| indexes.iter().take(at).any(|earlier| earlier == index))
    {
        return Err(Unresolved::Rejected);
    }
    if !live
        .iter()
        .map(|series| series.index().val())
        .eq(indexes.iter().copied())
    {
        return Err(Unresolved::Rejected);
    }
    let orders: Option<Vec<u32>> = source
        .series
        .iter()
        .map(|entry| entry.element.val("order")?.parse().ok())
        .collect();
    if !orders
        .ok_or(Unresolved::Rejected)?
        .windows(2)
        .all(|pair| pair[0] < pair[1])
    {
        return Err(Unresolved::Pending(Step::SeriesOrder));
    }
    if !restricted_shape_measured(source, target) {
        return Err(Unresolved::Pending(Step::UnmeasuredShape));
    }
    Ok(())
}

/// Section 12's x classes and the categories' data class, as the pair's
/// measurements admit them: x values or categories of a class no round
/// converted wait for one, and so do categories of mixed classes.
fn admit_data_classes(source: &Source, conversion: &Conversion) -> Result<(), Unresolved> {
    if source.has_x {
        let (data, spacing) = source.x;
        let measured = cell(conversion.x_values.get(data))
            .and_then(|spacings| cell(spacings.get(spacing)))
            .is_ok();
        if !measured {
            return Err(Unresolved::Pending(Step::UnmeasuredXClass));
        }
        return Ok(());
    }
    let measured = source
        .joined
        .is_some_and(|class| cell(conversion.category.get(class)).is_ok());
    if !measured {
        return Err(Unresolved::Pending(Step::UnmeasuredCategoryClass));
    }
    Ok(())
}

fn replay(
    chart: &Chart,
    workbook: &Workbook,
    from: ChartSource,
    target: ChartSubtype,
) -> Result<Plan, Unresolved> {
    let kind = subtype_index(target)
        .and_then(|index| KIND_RULES.get(index))
        .and_then(Option::as_ref)
        .ok_or(Unresolved::Rejected)?;
    let source = Source::read(chart, workbook, from).ok_or(Unresolved::Rejected)?;
    let live: Vec<&AreaChartSeries> = chart
        .plot_area()
        .chart_groups()
        .flat_map(|group| group.series().area_chart_series())
        .collect();
    admit(&live, &source, target)?;
    let conversion = conversion(&source, target)?;
    admit_data_classes(&source, conversion)?;
    let replay = Replay {
        source: &source,
        kind,
        conversion,
        target,
    };
    let mut after = replay.chart()?;
    let area = after.child("plotArea").ok_or(Unresolved::Rejected)?;
    let source_area = source.chart.child("plotArea").ok_or(Unresolved::Rejected)?;
    let position = |index: u32| {
        source
            .series
            .iter()
            .position(|entry| entry.index == index)
            .ok_or(Unresolved::Rejected)
    };
    let mut groups = Vec::new();
    for (owner, axes, indexes) in group_builder::read_groups(area, source_area)? {
        let series = indexes
            .into_iter()
            .map(position)
            .collect::<Result<Vec<_>, _>>()?;
        groups.push(PlannedGroup {
            owner,
            axes,
            series,
        });
    }
    let mut members: Vec<usize> = groups
        .iter()
        .flat_map(|group| group.series.iter().copied())
        .collect();
    members.sort_unstable();
    if !members.iter().copied().eq(0..source.series.len()) {
        return Err(Unresolved::Rejected);
    }
    let planned = Planned {
        groups,
        axes: axes::read_axes(area, source_area, chart.plot_area())?,
        scene: read_scene(&source.chart, &after)?,
        series: series::patches(&source, &kind.data, area, &live)?,
    };
    let staged = stage(chart, workbook, &source, planned, &mut after)?;
    let written = Model::of(&after);
    let changes = model::changes(&Model::of(&source.chart), &written, &staged.removed);
    let paint = model::paint(&written, kind.paint).ok_or(Unresolved::Rejected)?;
    Ok(Plan {
        from,
        to: target,
        chart: staged.chart,
        changes,
        paint,
    })
}

/// The result on its staging copy.
struct Staged {
    chart:   Chart,
    /// The parts the target families' cleanup removed, by source position.
    removed: Vec<(usize, SeriesPartKind)>,
}

/// Section 5's commit, on a copy of `chart`: detaches, builds and attaches
/// the result, writes it with `workbook` and compares it with `after`, the
/// planned `c:chart`, less what the target families' cleanup removed. On a
/// match `after` is the `c:chart` umya writes for the result.
fn stage(
    chart: &Chart,
    workbook: &Workbook,
    source: &Source,
    planned: Planned,
    after: &mut Element,
) -> Result<Staged, Unresolved> {
    let mut staged = chart.clone();
    let detached = commit::detach(&mut staged);
    let (built, cleanups) = build::build(planned, detached, workbook)?;
    if !commit::attach(&mut staged, built) {
        return Err(Unresolved::Rejected);
    }
    let written = xml::written_bytes(|writer| staged.write_to(writer, workbook))
        .ok_or(Unresolved::Pending(Step::StagedDifference))?;
    let area = after.child_mut("plotArea").ok_or(Unresolved::Rejected)?;
    let mut removed = Vec::new();
    for cleanup in &cleanups {
        let entry = source
            .series
            .get(cleanup.position)
            .ok_or(Unresolved::Rejected)?;
        let index = entry.index.to_string();
        let series = area
            .children
            .iter_mut()
            .filter(|group| group.local().ends_with("Chart"))
            .flat_map(|group| group.children.iter_mut())
            .find(|series| series.local() == "ser" && series.val("idx") == Some(index.as_str()))
            .ok_or(Unresolved::Rejected)?;
        series::remove_cleaned(series, cleanup);
        removed.extend(
            cleanup
                .removed
                .iter()
                .map(|(kind, _)| (cleanup.position, *kind)),
        );
    }
    if !xml::matches_written(after, &written) {
        return Err(Unresolved::Pending(Step::StagedDifference));
    }
    Ok(Staged {
        chart: staged,
        removed,
    })
}

/// The rule table applied to one source chart for one target.
pub(super) struct Replay<'a> {
    pub(super) source:     &'a Source,
    pub(super) kind:       &'static KindRule,
    pub(super) conversion: &'static Conversion,
    pub(super) target:     ChartSubtype,
}

impl Replay<'_> {
    /// The `c:chart` Excel writes.
    fn chart(&self) -> Result<Element, Unresolved> {
        let mut chart = self.source.chart.clone();
        self.chart_rules(&chart)?;
        self.scene(&mut chart)?;
        let axes = axes::plan_axes(self)?;
        let mut groups = Vec::new();
        for (index, rule) in self.kind.groups.iter().enumerate() {
            let values = self
                .conversion
                .groups
                .get(index)
                .ok_or(Unresolved::Rejected)?;
            groups.push(self.group(index, rule, values, &axes)?);
        }
        let area = chart.child_mut("plotArea").ok_or(Unresolved::Rejected)?;
        let kept: Vec<Element> = area
            .children
            .drain(..)
            .filter(|child| {
                !child.local().ends_with("Chart")
                    && !matches!(child.local(), "catAx" | "valAx" | "dateAx" | "serAx")
            })
            .collect();
        let (layout, rest): (Vec<Element>, Vec<Element>) = kept
            .into_iter()
            .partition(|child| child.local() == "layout");
        area.children = layout
            .into_iter()
            .chain(groups)
            .chain(axes.into_elements())
            .chain(rest)
            .collect();
        Ok(chart)
    }

    /// Chart-level children are carried as they are; removing one is not
    /// built yet.
    fn chart_rules(&self, chart: &Element) -> Result<(), Unresolved> {
        let rules = &self.kind.chart;
        let checks = [
            (&rules.title, "title"),
            (&rules.legend, "legend"),
            (&rules.auto_title_deleted, "autoTitleDeleted"),
            (&rules.plot_visible_only, "plotVisOnly"),
            (&rules.display_blanks_as, "dispBlanksAs"),
        ];
        for (rule, name) in checks {
            match cell(rule.get(presence(chart.child(name).is_some())))? {
                super::rule::Retain::Keep => (),
                super::rule::Retain::Remove => return Err(Unresolved::Pending(Step::ChartLevel)),
            }
        }
        Ok(())
    }

    fn scene(&self, chart: &mut Element) -> Result<(), Unresolved> {
        let rules = self.conversion.scene.get(self.source.scene);
        let ids = TemplateIds { axis: 0, cross: 0 };
        let view = chart.child("view3D").cloned();
        let present =
            |name: &str| presence(view.as_ref().and_then(|view| view.child(name)).is_some());
        let mut view3d = view.clone().unwrap_or_else(|| Element::new("c:view3D"));
        write::carried(
            *cell(rules.rotation_x.get(present("rotX")))?,
            &mut view3d,
            "rotX",
        );
        match *cell(rules.rotation_y.get(present("rotY")))? {
            RotationY::Keep => (),
            RotationY::Remove => view3d.remove("rotY"),
            RotationY::Set(angle) => view3d.put(Element::value("rotY", &angle.xml())),
            RotationY::FromFirstSliceAngle => {
                let angle = self
                    .source
                    .group(0)
                    .and_then(|group| group.val("firstSliceAng"))
                    .ok_or(Unresolved::Rejected)?;
                view3d.put(Element::value("rotY", angle));
            }
        }
        write::carried(
            *cell(rules.right_angle_axes.get(present("rAngAx")))?,
            &mut view3d,
            "rAngAx",
        );
        write::carried(
            *cell(rules.perspective.get(present("perspective")))?,
            &mut view3d,
            "perspective",
        );
        chart.remove("view3D");
        match *cell(rules.view.get(presence(view.is_some())))? {
            super::rule::Carried::Remove => (),
            super::rule::Carried::Keep if view.is_none() => (),
            super::rule::Carried::Keep | super::rule::Carried::Set(()) => {
                chart.children.insert(0, view3d);
            }
        }
        let walls = [
            (&rules.floor, "floor"),
            (&rules.side_wall, "sideWall"),
            (&rules.back_wall, "backWall"),
        ];
        for (rule, name) in walls {
            let rule = *cell(rule.get(presence(chart.child(name).is_some())))?;
            write::carried_node(rule, chart, name, ids);
        }
        if let Some(area) = chart.child("plotArea") {
            let rule = *cell(rules.plot_area.get(presence(area.child("spPr").is_some())))?;
            if !matches!(rule, super::rule::Carried::Keep) {
                return Err(Unresolved::Pending(Step::PlotAreaShape));
            }
        }
        Ok(())
    }

    /// Target plot group `index` with its series and axis references.
    fn group(
        &self,
        index: usize,
        rule: &GroupRule,
        values: &GroupValuesRule,
        axes: &axes::AxisLayout,
    ) -> Result<Element, Unresolved> {
        let before = self.source.group(index);
        let tag = group_builder::tag(rule.chart_type);
        let mut group = Element::new(format!("c:{tag}"));
        if let Some(before) = before {
            // A group of the same type keeps every child it holds, extensions
            // included; a retyped group starts from the governed children.
            let same_type = before.local() == tag;
            group.children = before
                .children
                .iter()
                .filter(|child| match child.local() {
                    "ser" | "axId" => false,
                    local => same_type || group_builder::GOVERNED.contains(&local),
                })
                .cloned()
                .collect();
        }
        let has = |name: &str| presence(before.and_then(|group| group.child(name)).is_some());
        macro_rules! carried {
            ($($field:ident => $name:literal),* $(,)?) => {$(
                write::carried(*cell(rule.$field.get(has($name)))?, &mut group, $name);
            )*};
        }
        carried! {
            bar_direction => "barDir", grouping => "grouping", vary_colors => "varyColors",
            overlap => "overlap", hole_size => "holeSize", bubble_scale => "bubbleScale",
            show_negative_bubbles => "showNegBubbles", scatter_style => "scatterStyle",
            radar_style => "radarStyle", wireframe => "wireframe", shape => "shape",
            show_marker => "marker", smooth => "smooth", of_pie_type => "ofPieType",
            second_pie_size => "secondPieSize",
        }
        let retained = [
            (&rule.size_represents, "sizeRepresents"),
            (&rule.bubble_3d, "bubble3D"),
            (&rule.gap_depth, "gapDepth"),
            (&rule.drop_lines, "dropLines"),
        ];
        for (field, name) in retained {
            write::retained(*cell(field.get(has(name)))?, &mut group, name);
        }
        write::carried_presence(
            *cell(rule.band_formats.get(has("bandFmts")))?,
            &mut group,
            "bandFmts",
        );
        write::carried(
            *cell(values.gap_width.get(has("gapWidth")))?,
            &mut group,
            "gapWidth",
        );
        match *cell(values.first_slice_angle.get(has("firstSliceAng")))? {
            SliceAngle::Keep => (),
            SliceAngle::Remove => group.remove("firstSliceAng"),
            SliceAngle::Set(angle) => group.put(Element::value("firstSliceAng", &angle.xml())),
            SliceAngle::FromSceneRotationY => {
                let angle = self
                    .source
                    .chart
                    .child("view3D")
                    .and_then(|view| view.val("rotY"))
                    .ok_or(Unresolved::Rejected)?;
                group.put(Element::value("firstSliceAng", angle));
            }
        }
        // Bands follow the series' own style and the bands the group holds
        // (the group rule above reads the wrapper); the group's trees follow
        // the style the scene keeps.
        let holds_bands = presence(
            before
                .and_then(|group| group.child("bandFmts"))
                .is_some_and(|bands| bands.child("bandFmt").is_some()),
        );
        let bands = *cell(values.bands.get(self.source.style()).get(holds_bands))?;
        let style = self.source.scene;
        if !matches!(bands, super::rule::BandRule::Keep) {
            return Err(Unresolved::Pending(Step::Bands));
        }
        let ids = TemplateIds { axis: 0, cross: 0 };
        let trees = [
            (&values.high_low_lines, "hiLowLines"),
            (&values.up_down_bars, "upDownBars"),
            (&values.series_lines, "serLines"),
            (&values.data_labels, "dLbls"),
        ];
        for (field, name) in trees {
            write::carried_node(
                *cell(field.get(style).get(has(name)))?,
                &mut group,
                name,
                ids,
            );
        }
        for series in self.selected(rule.series) {
            group.children.push(self.series(series)?);
        }
        group.children.extend(axes.group_references(rule)?);
        Ok(group)
    }

    fn selected(&self, selection: SeriesSelection) -> impl Iterator<Item = &source::SourceSeries> {
        let skip = usize::from(matches!(selection, SeriesSelection::AfterFirst));
        let take = if matches!(selection, SeriesSelection::First) {
            1
        } else {
            usize::MAX
        };
        self.source.series.iter().skip(skip).take(take)
    }

    /// The style rules of the series at `position`.
    pub(super) fn style(
        &self,
        position: usize,
        styled: super::slots::SeriesStyleClass,
    ) -> Result<&'static SeriesStyle, Unresolved> {
        let styles = self.conversion.series;
        let style = match styles {
            [only] => only,
            _ => styles.get(position).ok_or(Unresolved::Rejected)?,
        };
        Ok(*style.get(styled))
    }

    fn series(&self, entry: &source::SourceSeries) -> Result<Element, Unresolved> {
        let position = self
            .source
            .series
            .iter()
            .position(|other| other.index == entry.index)
            .ok_or(Unresolved::Rejected)?;
        series::after(self, entry, position)
    }
}

fn read_scene(before: &Element, after: &Element) -> Result<Option<PlannedScene>, Unresolved> {
    let names = ["view3D", "floor", "sideWall", "backWall"];
    if names
        .iter()
        .all(|name| before.child(name) == after.child(name))
    {
        return Ok(None);
    }
    let mut scene = PlannedScene::default();
    if let Some(element) = after.child("view3D") {
        let mut view = View3D::default();
        xml::read(element, |reader, start, _| {
            view.set_attributes(reader, start)
        })
        .ok_or(Unresolved::Rejected)?;
        scene.view = Some(view);
    }
    // A wall unchanged as written is the live wall; another is read without
    // its extension lists and takes the live wall's stored state.
    macro_rules! wall {
        ($field:ident, $name:literal, $kind:ident) => {
            if let Some(element) = after.child($name) {
                let read = |element: &Element| {
                    let mut wall = $kind::default();
                    xml::read(&xml::without_extensions(element), |reader, start, _| {
                        wall.set_attributes(reader, start)
                    })
                    .map(|()| wall)
                    .ok_or(Unresolved::Rejected)
                };
                scene.$field = Some(match before.child($name) {
                    Some(written) if written == element => Owner::Kept,
                    written => Owner::Rebuilt {
                        read:  read(element)?,
                        probe: written.map(read).transpose()?,
                    },
                });
            }
        };
    }
    wall!(floor, "floor", Floor);
    wall!(side_wall, "sideWall", SideWall);
    wall!(back_wall, "backWall", BackWall);
    Ok(Some(scene))
}
