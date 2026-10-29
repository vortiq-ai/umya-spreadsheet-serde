//! Restore omitted chart metadata from an authoritative chart.

use std::collections::HashMap;

use super::{
    AreaChartSeries,
    ChartGroupRef,
    ChartSpace,
    ChartText,
    DataLabels,
    Layout,
    Marker,
    PlotArea,
    ShapeProperties,
    Title,
    chart_reference_roles::{
        AxisParts,
        AxisPartsMut,
        ReferenceTargetRef,
    },
    chart_series_identity::{
        SeriesCorrespondence,
        SeriesMatch,
        SeriesPosition,
        match_series,
    },
};

/// Restoration counts and diagnostics. Ambiguous identities are never matched
/// by collection position.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RestoreReport {
    pub matched_series:             usize,
    pub index_matched_series:       usize,
    pub formula_matched_series:     usize,
    pub unmatched_series:           usize,
    pub ambiguous_series:           usize,
    pub unmatched_previous_series:  usize,
    pub restored_points:            usize,
    pub restored_labels:            usize,
    pub restored_caches:            usize,
    pub recreated_owners:           usize,
    pub restored_owners:            usize,
    pub added_owners:               usize,
    pub removed_owners:             usize,
    pub incompatible_owners:        usize,
    pub removed_incompatible_parts: usize,
    pub diagnostics:                Vec<String>,
}

pub(super) fn replace_hidden_parts<T: Clone>(
    current: &mut Option<T>,
    previous: Option<&T>,
) -> usize {
    let restored = usize::from(current.is_none() && previous.is_some());
    match (current, previous) {
        (Some(current), Some(previous)) => current.clone_from(previous),
        (current, previous) => *current = previous.cloned(),
    }
    restored
}

pub(super) fn record_supplied_extension(present: bool, path: &str, report: &mut RestoreReport) {
    if present {
        report.removed_incompatible_parts += 1;
        report.diagnostics.push(format!(
            "discarded incoming extension at {path}; authoritative extension required"
        ));
    }
}

pub(crate) fn restore_space(current: &mut ChartSpace, previous: &ChartSpace) -> RestoreReport {
    // Incoming caches never become authoritative, even if their guard matches.
    super::clear_source_caches(current);
    let mut report = RestoreReport::default();
    let (chart, shape) = current.reference_role_parts_mut();
    let (old_chart, old_shape) = previous.reference_role_parts();
    let restored = chart.restore_export_parts(old_chart, &mut report);
    report.restored_owners += restored;
    let fields = chart.reference_role_parts_mut();
    let old = old_chart.reference_role_parts();
    paired(
        fields.title,
        old.title,
        restore_title,
        discard_title_extensions,
        &mut report,
    );
    restore_plot(fields.plot, old.plot, &mut report);
    report.restored_owners += fields.legend.restore_export_parts(old.legend);
    let (layout, legend_shape) = fields.legend.reference_role_parts_mut();
    let (old_layout, old_legend_shape) = old.legend.reference_role_parts();
    paired(
        layout,
        old_layout,
        restore_layout,
        discard_layout_extensions,
        &mut report,
    );
    paired(
        legend_shape,
        old_legend_shape,
        restore_shape,
        discard_shape_extensions,
        &mut report,
    );
    paired(
        shape,
        old_shape,
        restore_shape,
        discard_shape_extensions,
        &mut report,
    );
    paired(
        fields.floor.and_then(|wall| wall.shape_properties_mut()),
        old.floor.and_then(|wall| wall.shape_properties()),
        restore_shape,
        discard_shape_extensions,
        &mut report,
    );
    paired(
        fields
            .side_wall
            .and_then(|wall| wall.shape_properties_mut()),
        old.side_wall.and_then(|wall| wall.shape_properties()),
        restore_shape,
        discard_shape_extensions,
        &mut report,
    );
    paired(
        fields
            .back_wall
            .and_then(|wall| wall.shape_properties_mut()),
        old.back_wall.and_then(|wall| wall.shape_properties()),
        restore_shape,
        discard_shape_extensions,
        &mut report,
    );
    super::chart_reference_roles::visit_ref(current, &mut |_, target| {
        if let ReferenceTargetRef::String(reference) = target {
            report.restored_caches += usize::from(reference.string_cache().source().is_some());
        }
    });
    report
}

fn paired<T>(
    current: Option<&mut T>,
    previous: Option<&T>,
    restore: impl FnOnce(&mut T, &T, &mut RestoreReport),
    discard: fn(&mut T, &mut RestoreReport),
    report: &mut RestoreReport,
) {
    match (current, previous) {
        (Some(current), Some(previous)) => restore(current, previous, report),
        (None, None) => {}
        (Some(current), None) => {
            report.added_owners += 1;
            discard(current, report);
        }
        (None, Some(_)) => report.removed_owners += 1,
    }
}

fn restore_title(current: &mut Title, previous: &Title, report: &mut RestoreReport) {
    let (text, shape, layout) = current.reference_role_parts_mut();
    let (old_text, old_shape, old_layout) = previous.reference_role_parts();
    paired(
        text,
        old_text,
        restore_text,
        discard_text_extensions,
        report,
    );
    paired(
        shape,
        old_shape,
        restore_shape,
        discard_shape_extensions,
        report,
    );
    paired(
        layout,
        old_layout,
        restore_layout,
        discard_layout_extensions,
        report,
    );
}

pub(super) fn restore_text(
    current: &mut ChartText,
    previous: &ChartText,
    report: &mut RestoreReport,
) {
    let branch = |text: &ChartText| {
        (
            text.rich_text().is_some(),
            text.string_reference().is_some(),
            text.numeric_value().is_some(),
        )
    };
    if branch(current) != branch(previous) {
        discard_text_extensions(current, report);
        report.incompatible_owners += 1;
        report
            .diagnostics
            .push("incompatible chart text branch replacement".into());
        return;
    }
    if let (Some(text), Some(old)) = (current.rich_text_mut(), previous.rich_text()) {
        if text.projected_structure_matches(old) {
            report.restored_owners += text.restore_export_parts(old);
        } else {
            report.incompatible_owners += 1;
            report
                .diagnostics
                .push("replaced chart text retains no previous text supplements".into());
        }
    }
    if let (Some(reference), Some(old)) =
        (current.string_reference_mut(), previous.string_reference())
    {
        restore_string_reference(reference, old, report);
    }
}

fn restore_string_reference(
    current: &mut super::StringReference,
    previous: &super::StringReference,
    report: &mut RestoreReport,
) {
    record_supplied_extension(current.extensions().is_some(), "strRef", report);
    report.restored_owners += current.restore_export_parts(previous);
    if current.formula().address_str() != previous.formula().address_str() {
        current.string_cache_mut().set_source(None);
        return;
    }
    let formula = current.formula().address_str();
    let source = previous
        .string_cache()
        .source()
        .filter(|source| source.formula() == formula)
        .cloned();
    current.string_cache_mut().set_source(source);
}

fn restore_number_reference(
    current: &mut super::NumberReference,
    previous: &super::NumberReference,
    report: &mut RestoreReport,
) {
    record_supplied_extension(current.extensions().is_some(), "numRef", report);
    report.restored_owners += current.restore_export_parts(previous);
}

fn restore_multi_reference(
    current: &mut super::MultiLevelStringReference,
    previous: &super::MultiLevelStringReference,
    report: &mut RestoreReport,
) {
    record_supplied_extension(current.extensions().is_some(), "multiLvlStrRef", report);
    report.restored_owners += current.restore_export_parts(previous);
}

pub(super) fn restore_shape(
    current: &mut ShapeProperties,
    previous: &ShapeProperties,
    report: &mut RestoreReport,
) {
    record_supplied_extension(current.extensions().is_some(), "spPr", report);
    report.restored_owners += current.restore_export_parts(previous);
}

pub(super) fn restore_marker(current: &mut Marker, previous: &Marker, report: &mut RestoreReport) {
    record_supplied_extension(current.reference_role_parts().1.is_some(), "marker", report);
    report.restored_owners += current.restore_export_parts(previous);
    let (shape, _) = current.reference_role_parts_mut();
    let (old_shape, _) = previous.reference_role_parts();
    paired(
        shape,
        old_shape,
        restore_shape,
        discard_shape_extensions,
        report,
    );
}

pub(super) fn restore_layout(current: &mut Layout, previous: &Layout, report: &mut RestoreReport) {
    record_supplied_extension(current.extensions().is_some(), "layout", report);
    report.restored_owners += current.restore_export_parts(previous);
    if let Some(manual) = current.manual_layout_mut() {
        record_supplied_extension(manual.extensions().is_some(), "manualLayout", report);
        if let Some(old) = previous.manual_layout() {
            report.restored_owners += manual.restore_export_parts(old);
        } else {
            manual.projection_extensions_mut();
        }
    }
}

fn restore_labels(current: &mut DataLabels, previous: &DataLabels, report: &mut RestoreReport) {
    if previous.parts_for_restoration_present() {
        record_supplied_extension(current.reference_role_parts().3.is_some(), "dLbls", report);
    } else {
        discard_unmatched_labels(current, "dLbls", report);
    }
    if current
        .text_properties()
        .zip(previous.text_properties())
        .is_some_and(|(text, old)| !text.projected_structure_matches(old))
    {
        report.incompatible_owners += 1;
    }
    let restored = current.restore_export_parts(previous);
    report.restored_owners += restored;
    if restored != 0 {
        report.restored_labels += previous.labels().len();
    }
}

fn restore_series(
    current: &mut AreaChartSeries,
    previous: &AreaChartSeries,
    report: &mut RestoreReport,
) {
    let restored = current.restore_export_parts(previous);
    report.restored_owners += restored;
    if restored != 0 {
        report.restored_points += previous.data_points().len();
    }
    if previous.labels_for_roles().is_none() {
        if let Some(labels) = current.labels_for_roles_mut() {
            let removed = labels.discard_untrusted_point_labels();
            report.removed_incompatible_parts += removed;
            if removed != 0 {
                report
                    .diagnostics
                    .push("ignored incoming point labels without an authoritative owner".into());
            }
        }
    }
    if current.data_labels().is_none() {
        if let Some(labels) = previous.data_labels() {
            current.set_data_labels(labels.clone());
            report.recreated_owners += 1;
            report.restored_owners += 1;
            report.restored_labels += labels.labels().len();
        }
    } else {
        // labels_for_roles_mut is presence-neutral, unlike the editable getter.
        paired(
            current.labels_for_roles_mut(),
            previous.labels_for_roles(),
            restore_labels,
            discard_label_owner,
            report,
        );
    }
    let fields = current.reference_role_parts_mut();
    let old = previous.reference_role_parts();
    paired(
        fields.text,
        old.text,
        restore_text,
        discard_text_extensions,
        report,
    );
    paired(
        fields.shape,
        old.shape,
        restore_shape,
        discard_shape_extensions,
        report,
    );
    paired(
        fields.marker,
        old.marker,
        restore_marker,
        discard_marker_extensions,
        report,
    );
    if let Some(category) = fields.category {
        if let Some(old) = old.category {
            let (string, number, multi, literal) = category.reference_role_parts_mut();
            let (old_string, old_number, old_multi, old_literal) = old.reference_role_parts();
            paired(
                string,
                old_string,
                restore_string_reference,
                discard_string_reference_extensions,
                report,
            );
            if let Some(reference) = number {
                if let Some(old) = old_number {
                    restore_number_reference(reference, old, report);
                } else {
                    discard_number_reference_extensions(reference, report);
                }
            }
            if let Some(reference) = multi {
                if let Some(old) = old_multi {
                    restore_multi_reference(reference, old, report);
                } else {
                    discard_multi_reference_extensions(reference, report);
                }
            }
            if let Some(literal) = literal {
                record_supplied_extension(literal.extensions().is_some(), "strLit", report);
                report.restored_owners += literal.restore_extensions(old_literal);
            }
        } else {
            discard_category_extensions(category, report);
        }
    }

    macro_rules! numeric_reference {
        ($current:expr, $previous:expr) => {
            if let Some(value) = $current {
                if let Some(old) = $previous {
                    restore_number_reference(
                        value.number_reference_mut(),
                        old.number_reference(),
                        report,
                    );
                } else {
                    discard_number_reference_extensions(value.number_reference_mut(), report);
                }
            }
        };
    }
    numeric_reference!(fields.values, old.values);
    numeric_reference!(fields.xvalues, old.xvalues);
    numeric_reference!(fields.yvalues, old.yvalues);
    numeric_reference!(fields.bubble, old.bubble);
}

// These are occurrence parents: nested references leave with their owner and
// must not inflate the removal count.
fn discard_unmatched_series(
    current: &mut AreaChartSeries,
    group: usize,
    series: usize,
    report: &mut RestoreReport,
) {
    let location = format!("group:{group}/series:{series}");
    for (kind, count) in [
        ("point", current.data_points().len()),
        ("trendline", current.trendlines().len()),
        ("error bars", current.error_bars().len()),
        ("extension", usize::from(current.extensions().is_some())),
    ] {
        for occurrence in 0..count {
            report.removed_incompatible_parts += 1;
            report.diagnostics.push(format!(
                "removed incoming {kind} occurrence:{occurrence} from {location} without an \
                 authoritative series"
            ));
        }
    }
    current.projection_parts_mut();
    if let Some(labels) = current.labels_for_roles_mut() {
        discard_unmatched_labels(labels, &location, report);
    }
    discard_series_extensions(current, report);
}

fn discard_unmatched_labels(labels: &mut DataLabels, location: &str, report: &mut RestoreReport) {
    let removed = labels.discard_untrusted_point_labels();
    report.removed_incompatible_parts += removed;
    for occurrence in 0..removed {
        report.diagnostics.push(format!(
            "removed incoming point label occurrence:{occurrence} from {location} without an \
             authoritative owner"
        ));
    }
    discard_label_extensions(labels, report);
}

pub(super) fn discard_shape_extensions(current: &mut ShapeProperties, report: &mut RestoreReport) {
    record_supplied_extension(current.extensions().is_some(), "spPr", report);
    current.projection_extensions_mut();
}

pub(super) fn discard_layout_extensions(current: &mut Layout, report: &mut RestoreReport) {
    record_supplied_extension(current.extensions().is_some(), "layout", report);
    let (manual, _) = current.projection_parts_mut();
    if let Some(manual) = manual {
        record_supplied_extension(manual.extensions().is_some(), "manualLayout", report);
        manual.projection_extensions_mut();
    }
}

pub(super) fn discard_marker_extensions(current: &mut Marker, report: &mut RestoreReport) {
    record_supplied_extension(current.reference_role_parts().1.is_some(), "marker", report);
    let (shape, _) = current.projection_parts_mut();
    if let Some(shape) = shape {
        discard_shape_extensions(shape, report);
    }
}

pub(super) fn discard_text_extensions(current: &mut ChartText, report: &mut RestoreReport) {
    if let Some(reference) = current.string_reference_mut() {
        discard_string_reference_extensions(reference, report);
    }
}

fn discard_title_extensions(current: &mut Title, report: &mut RestoreReport) {
    let (text, shape, layout) = current.reference_role_parts_mut();
    if let Some(text) = text {
        discard_text_extensions(text, report);
    }
    if let Some(shape) = shape {
        discard_shape_extensions(shape, report);
    }
    if let Some(layout) = layout {
        discard_layout_extensions(layout, report);
    }
}

fn discard_string_reference_extensions(
    current: &mut super::StringReference,
    report: &mut RestoreReport,
) {
    record_supplied_extension(current.extensions().is_some(), "strRef", report);
    current.projection_extensions_mut();
}

fn discard_number_reference_extensions(
    current: &mut super::NumberReference,
    report: &mut RestoreReport,
) {
    record_supplied_extension(current.extensions().is_some(), "numRef", report);
    current.projection_extensions_mut();
}

fn discard_multi_reference_extensions(
    current: &mut super::MultiLevelStringReference,
    report: &mut RestoreReport,
) {
    record_supplied_extension(current.extensions().is_some(), "multiLvlStrRef", report);
    current.projection_extensions_mut();
}

fn discard_category_extensions(current: &mut super::CategoryAxisData, report: &mut RestoreReport) {
    let (string, number, multi, literal) = current.reference_role_parts_mut();
    if let Some(value) = string {
        discard_string_reference_extensions(value, report);
    }
    if let Some(value) = number {
        discard_number_reference_extensions(value, report);
    }
    if let Some(value) = multi {
        discard_multi_reference_extensions(value, report);
    }
    if let Some(value) = literal {
        record_supplied_extension(value.extensions().is_some(), "strLit", report);
        value.projection_extensions_mut();
    }
}

fn discard_numeric_series_extensions(
    values: Option<&mut super::Values>,
    x: Option<&mut super::XValues>,
    y: Option<&mut super::YValues>,
    bubble: Option<&mut super::BubbleSize>,
    report: &mut RestoreReport,
) {
    if let Some(value) = values {
        discard_number_reference_extensions(value.number_reference_mut(), report);
    }
    if let Some(value) = x {
        discard_number_reference_extensions(value.number_reference_mut(), report);
    }
    if let Some(value) = y {
        discard_number_reference_extensions(value.number_reference_mut(), report);
    }
    if let Some(value) = bubble {
        discard_number_reference_extensions(value.number_reference_mut(), report);
    }
}

fn discard_series_extensions(current: &mut AreaChartSeries, report: &mut RestoreReport) {
    let fields = current.reference_role_parts_mut();
    if let Some(value) = fields.text {
        discard_text_extensions(value, report);
    }
    if let Some(value) = fields.shape {
        discard_shape_extensions(value, report);
    }
    if let Some(value) = fields.marker {
        discard_marker_extensions(value, report);
    }
    if let Some(value) = fields.category {
        discard_category_extensions(value, report);
    }
    discard_numeric_series_extensions(
        fields.values,
        fields.xvalues,
        fields.yvalues,
        fields.bubble,
        report,
    );
}

fn discard_label_owner(current: &mut DataLabels, report: &mut RestoreReport) {
    discard_unmatched_labels(current, "dLbls", report);
}

fn discard_label_extensions(current: &mut DataLabels, report: &mut RestoreReport) {
    current.discard_extensions(report);
}

fn discard_axis_extensions(current: AxisPartsMut<'_>, report: &mut RestoreReport) {
    if let Some(value) = current.title {
        discard_title_extensions(value, report);
    }
    if let Some(value) = current.shape {
        discard_shape_extensions(value, report);
    }
    if let Some(value) = current
        .gridlines
        .and_then(|lines| lines.shape_properties_mut())
    {
        discard_shape_extensions(value, report);
    }
}

fn restore_plot(current: &mut PlotArea, previous: &PlotArea, report: &mut RestoreReport) {
    let correspondence = match_series(current, previous);
    report.unmatched_previous_series += correspondence.unmatched_previous.len();
    let old_groups: Vec<_> = previous.chart_groups().collect();
    let families: Vec<_> = current
        .chart_groups()
        .map(ChartGroupRef::series_family)
        .collect();
    let groups = match_groups(current, &old_groups, &correspondence, report);
    let group_extensions: Vec<_> = current
        .chart_groups()
        .map(|group| group.into_reference_role_parts().extensions.is_some())
        .collect();
    let by_position: HashMap<_, _> = correspondence
        .current
        .iter()
        .map(|(position, outcome)| ((position.group, position.series), *outcome))
        .collect();

    // Resolve axis correspondence before borrowing field-disjoint plot views.
    macro_rules! axes {
        ($read:ident, $write:ident, $kind:literal $(, $restore:ident)? $(; $discard:ident)?) => {{
            let old = previous.$read();
            let positions = unique_axis_matches(
                current
                    .$read()
                    .iter()
                    .map(|axis| axis.axis_id().explicit_value()),
                old.iter().map(|axis| axis.axis_id().explicit_value()),
            );
            for (position, axis) in current.$write().iter_mut().enumerate() {
                if let Some(old_position) = positions[position] {
                    $(let restored = axis.$restore(&old[old_position], report);
                    report.restored_owners += restored;)?
                    restore_axis(
                        axis.reference_role_parts_mut(),
                        &old[old_position].reference_role_parts(),
                        report,
                    );
                } else {
                    $(axis.$discard(report);)?
                    discard_axis_extensions(axis.reference_role_parts_mut(), report);
                    report.incompatible_owners += 1;
                    report
                        .diagnostics
                        .push(format!("unmatched or ambiguous {} axis:{position}", $kind));
                }
            }
        }};
    }
    axes!(
        category_axis,
        category_axis_mut,
        "category",
        restore_export_parts
    );
    axes!(date_axis, date_axis_mut, "date", restore_export_parts);
    axes!(value_axis, value_axis_mut, "value", restore_export_parts; discard_untrusted_display_units);
    axes!(series_axis, series_axis_mut, "series");

    let fields = current.reference_role_parts_mut();
    let old = previous.reference_role_parts();
    restore_layout(fields.layout, old.layout, report);
    paired(
        fields.shape,
        old.shape,
        restore_shape,
        discard_shape_extensions,
        report,
    );
    for (group_position, mut group) in fields.groups.into_iter().enumerate() {
        if let Some(old_position) = groups[group_position] {
            record_supplied_extension(
                group_extensions[group_position],
                &format!("group:{group_position}"),
                report,
            );
            report.restored_owners += group.restore_export_parts(old_groups[old_position]);
        }
        let fields = if groups[group_position].is_some() {
            group.into_reference_role_parts()
        } else {
            if group_extensions[group_position] {
                report.removed_incompatible_parts += 1;
                report.diagnostics.push(format!(
                    "removed incoming extension from group:{group_position} without an \
                     authoritative group"
                ));
            }
            // This detaches only the group's extension, not editable label
            // flags, text styles or their presence metadata.
            group.into_projection_parts()
        };
        if let Some(old_position) = groups[group_position] {
            paired(
                fields.labels,
                old_groups[old_position].into_reference_role_parts().labels,
                restore_labels,
                discard_label_owner,
                report,
            );
        } else if let Some(labels) = fields.labels {
            discard_unmatched_labels(labels, &format!("group:{group_position}"), report);
        }
        for (series_position, series) in fields.series.iter_mut().enumerate() {
            let position = (group_position, series_position);
            match by_position[&position] {
                SeriesMatch::ExplicitIndex(old_position)
                | SeriesMatch::ValuesFormula(old_position) => {
                    report.matched_series += 1;
                    if matches!(by_position[&position], SeriesMatch::ExplicitIndex(_)) {
                        report.index_matched_series += 1;
                    } else {
                        report.formula_matched_series += 1;
                    }
                    let old = &old_groups[old_position.group]
                        .into_reference_role_parts()
                        .series[old_position.series];
                    restore_series(series, old, report);
                    if families[group_position] != old_groups[old_position.group].series_family() {
                        let removed = series.remove_parts_not_allowed_in(families[group_position]);
                        report.removed_incompatible_parts += removed.len();
                        for (kind, occurrence) in removed {
                            report.diagnostics.push(format!(
                                "removed incompatible {kind:?} occurrence:{occurrence} from \
                                 group:{group_position}/series:{series_position}"
                            ));
                        }
                    }
                }
                SeriesMatch::Ambiguous => {
                    report.ambiguous_series += 1;
                    discard_unmatched_series(series, group_position, series_position, report);
                    report.diagnostics.push(format!(
                        "ambiguous series at group:{group_position}/series:{series_position}"
                    ));
                }
                SeriesMatch::Unmatched => {
                    report.unmatched_series += 1;
                    discard_unmatched_series(series, group_position, series_position, report);
                    report.diagnostics.push(format!(
                        "unmatched series at group:{group_position}/series:{series_position}"
                    ));
                }
            }
        }
    }
}

fn unique_axis_matches(
    current: impl Iterator<Item = Option<u32>>,
    previous: impl Iterator<Item = Option<u32>>,
) -> Vec<Option<usize>> {
    let current: Vec<_> = current.collect();
    let mut current_counts = HashMap::new();
    for id in current.iter().flatten() {
        *current_counts.entry(*id).or_insert(0) += 1;
    }
    let mut old = HashMap::<u32, Vec<usize>>::new();
    for (position, id) in previous.enumerate() {
        if let Some(id) = id {
            old.entry(id).or_default().push(position);
        }
    }
    current
        .into_iter()
        .map(|id| {
            let id = id?;
            if current_counts[&id] != 1 {
                return None;
            }
            match old.get(&id)?.as_slice() {
                [position] => Some(*position),
                _ => None,
            }
        })
        .collect()
}

fn restore_axis(current: AxisPartsMut<'_>, previous: &AxisParts<'_>, report: &mut RestoreReport) {
    paired(
        current.title,
        previous.title,
        restore_title,
        discard_title_extensions,
        report,
    );
    paired(
        current.shape,
        previous.shape,
        restore_shape,
        discard_shape_extensions,
        report,
    );
    paired(
        current
            .gridlines
            .and_then(|lines| lines.shape_properties_mut()),
        previous
            .gridlines
            .and_then(|lines| lines.shape_properties()),
        restore_shape,
        discard_shape_extensions,
        report,
    );
}

fn match_groups(
    current: &PlotArea,
    previous: &[ChartGroupRef<'_>],
    series: &SeriesCorrespondence,
    report: &mut RestoreReport,
) -> Vec<Option<usize>> {
    let mut candidates = Vec::new();
    for (position, group) in current.chart_groups().enumerate() {
        let members: Vec<SeriesPosition> = series
            .current
            .iter()
            .filter_map(|(owner, outcome)| {
                if owner.group != position {
                    return None;
                }
                match outcome {
                    SeriesMatch::ExplicitIndex(old) | SeriesMatch::ValuesFormula(old) => Some(*old),
                    _ => None,
                }
            })
            .collect();
        // A new/ambiguous group has no stored membership to identify it.
        let matches: Vec<_> = previous
            .iter()
            .enumerate()
            .filter_map(|(old_position, old)| {
                let membership = if members.is_empty() {
                    group.series().area_chart_series().is_empty()
                        && old.series().area_chart_series().is_empty()
                } else {
                    members.iter().all(|member| member.group == old_position)
                };
                (group.kind() == old.kind() && same_axes(group, *old) && membership)
                    .then_some(old_position)
            })
            .collect();
        candidates.push(matches);
    }
    let mut uses = HashMap::new();
    for matches in &candidates {
        if let [position] = matches.as_slice() {
            *uses.entry(*position).or_insert(0) += 1;
        }
    }
    candidates
        .into_iter()
        .enumerate()
        .map(|(position, matches)| {
            if let [old] = matches.as_slice() {
                if uses[old] == 1 {
                    return Some(*old);
                }
            }
            report.incompatible_owners += 1;
            report
                .diagnostics
                .push(format!("unmatched or ambiguous plot group:{position}"));
            None
        })
        .collect()
}

fn same_axes(current: ChartGroupRef<'_>, previous: ChartGroupRef<'_>) -> bool {
    current.axis_ids().map(|ids| {
        ids.iter()
            .map(super::AxisId::explicit_value)
            .collect::<Vec<_>>()
    }) == previous.axis_ids().map(|ids| {
        ids.iter()
            .map(super::AxisId::explicit_value)
            .collect::<Vec<_>>()
    })
}
