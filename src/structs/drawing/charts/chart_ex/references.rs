//! Detached edits share the workbook's reference grammar and geometry.
use std::collections::HashSet;

use super::{
    ChartExDimension,
    ChartExDimensionSource,
    ChartExFormula,
    ChartExFormulaDirection,
    ChartExSpace,
    ChartExText,
    ChartExTextData,
    xml::NativeElement,
};
use crate::{
    drawing::{
        charts::{
            ChartReferenceEdit,
            ReferenceAdjustmentReport,
            chart_reference_roles::{
                ReferenceGrammar,
                visit_mut,
            },
            reference_edit_engine::{
                FormulaEdit,
                PreparedReferenceEdit,
                edit_prepared_formula_text,
                is_named_operand,
                is_native_coordinate_operand,
            },
        },
        spreadsheet::ExtendedChart,
    },
    helper::{
        address::{
            split_reference_address,
            unescape_sheet_name,
        },
        coordinate::index_from_coordinate,
        formula::chart_formula_operands,
    },
};

/// A name operand whose definition belongs to the caller's name table.
/// Qualifiers describe the original formula, before the edit. An unqualified
/// name uses the supplied chart host's ordinary local/workbook lookup rules.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChartExDefinedNameReference {
    pub name:            String,
    pub sheet:           Option<String>,
    /// Explicit `[0]!` qualification selects a workbook-scoped name.
    pub workbook_scoped: bool,
}

/// A caller-resolved name operand whose definition points at a sheet being
/// deleted. The address is used to retain the dimension's number of levels.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChartExDefinedNameSource {
    /// Exact operand identity, usually copied from `defined_names` in the
    /// preceding adjustment report.
    pub reference: ChartExDefinedNameReference,
    /// Original defined-name formula, used to retain the dimension's levels.
    pub address:   String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ChartExReferenceAdjustmentReport {
    pub adjustments:          ReferenceAdjustmentReport,
    /// Each name/scope identity occurs once, in formula traversal order.
    /// Definitions are neither resolved nor rewritten by detached chart edits.
    pub defined_names:        Vec<ChartExDefinedNameReference>,
    /// Names of `deleted_names` that no formula of the chart refers to any
    /// more after a sheet deletion. The caller owns removal of their
    /// definitions.
    pub unused_defined_names: Vec<ChartExDefinedNameReference>,
}

type NameIdentity = (String, Option<String>, bool);

#[derive(Default)]
struct EditReport {
    report:          ChartExReferenceAdjustmentReport,
    names:           HashSet<NameIdentity>,
    remaining_names: HashSet<NameIdentity>,
}

/// What a sheet deletion removes: the operands of `names` and every operand
/// that reads a cell of the deleted sheet.
struct DeletedSheet<'a> {
    host:  &'a str,
    edit:  &'a PreparedReferenceEdit<'a>,
    names: &'a [ChartExDefinedNameSource],
}

struct DeletedSource {
    address:     String,
    invalidated: usize,
}

impl DeletedSheet<'_> {
    fn source(&self, formula: &ChartExFormula) -> Option<DeletedSource> {
        for operand in chart_formula_operands(&formula.text()) {
            let decision = edit_prepared_formula_text(operand.text, self.host, self.edit, false);
            if decision.invalidated > 0 {
                return Some(DeletedSource {
                    address:     operand.text.into(),
                    invalidated: decision.invalidated,
                });
            }
            let named = EditReport::name_references(operand.text)
                .into_iter()
                .next()
                .and_then(|reference| {
                    self.names.iter().find(|source| {
                        name_identity(&source.reference) == name_identity(&reference)
                    })
                });
            if let Some(source) = named {
                return Some(DeletedSource {
                    address:     source.address.clone(),
                    invalidated: 0,
                });
            }
        }
        None
    }
}

impl EditReport {
    fn name_references(text: &str) -> Vec<ChartExDefinedNameReference> {
        chart_formula_operands(text)
            .into_iter()
            .filter_map(|operand| {
                let (qualifier, name) = split_reference_address(operand.text);
                if !is_named_operand(name) || qualifier.contains(':') {
                    return None;
                }
                let workbook_scoped = qualifier == "[0]";
                if !workbook_scoped && operand.text.contains(['[', ']']) {
                    return None;
                }
                let sheet = (!qualifier.is_empty() && !workbook_scoped)
                    .then(|| unescape_sheet_name(qualifier).into_owned());
                Some(ChartExDefinedNameReference {
                    name: name.into(),
                    sheet,
                    workbook_scoped,
                })
            })
            .collect()
    }

    fn observe(&mut self, text: &str) {
        self.report.adjustments.visited += 1;
        for reference in Self::name_references(text) {
            if self.names.insert(name_identity(&reference)) {
                self.report.defined_names.push(reference);
            }
        }
    }

    fn rewrite(
        &mut self,
        text: &str,
        host: &str,
        edit: &PreparedReferenceEdit<'_>,
        bare_error: bool,
    ) -> FormulaEdit {
        let decision = edit_prepared_formula_text(text, host, edit, bare_error);
        for reference in Self::name_references(&decision.text) {
            self.remaining_names.insert(name_identity(&reference));
        }
        self.report.adjustments.invalidated += decision.invalidated;
        decision
    }

    fn decision(
        &mut self,
        text: &str,
        host: &str,
        edit: &PreparedReferenceEdit<'_>,
        bare_error: bool,
    ) -> FormulaEdit {
        self.observe(text);
        self.rewrite(text, host, edit, bare_error)
    }

    fn space(
        &mut self,
        space: &mut ChartExSpace,
        host: &str,
        edit: &PreparedReferenceEdit<'_>,
        deleted_names: &[ChartExDefinedNameSource],
    ) -> Result<(), String> {
        self.report.adjustments.preserved_opaque_extensions += space.opaque_extension_count();
        space.visit_formulas(&mut |formula| self.observe(&formula.address_str()));
        if matches!(edit.edit(), ChartReferenceEdit::DeleteSheet { .. }) {
            let deleted = DeletedSheet {
                host,
                edit,
                names: deleted_names,
            };
            self.remove_deleted_sources(space, &deleted)?;
        }
        space.visit_formulas_mut(&mut |formula| {
            let decision = self.rewrite(&formula.address_str(), host, edit, false);
            if formula.replace_reference_text_preserving_address_identity(&decision.text) {
                self.report.adjustments.changed += 1;
            }
        });
        Ok(())
    }

    /// Excel's result for a deleted source sheet: a dimension becomes empty
    /// literal levels, and a name formula or text that reads the sheet is
    /// removed. Other formulas follow the ordinary edit.
    fn remove_deleted_sources(
        &mut self,
        space: &mut ChartExSpace,
        deleted: &DeletedSheet<'_>,
    ) -> Result<(), String> {
        if let Some(data) = &mut space.data_collection {
            for dimension in data.data.iter_mut().flat_map(|data| &mut data.dimensions) {
                match dimension {
                    ChartExDimension::String(value) => {
                        self.remove_dimension_source(&mut value.source, deleted)?;
                    }
                    ChartExDimension::Numeric(value) => {
                        self.remove_dimension_source(&mut value.source, deleted)?;
                    }
                }
            }
        }
        if let Some(chart) = &mut space.chart {
            if let Some(title) = &mut chart.title {
                self.remove_text_source(&mut title.text, deleted);
            }
            if let Some(plot_area) = &mut chart.plot_area {
                for axis in &mut plot_area.axis {
                    if let Some(title) = &mut axis.title {
                        self.remove_text_source(&mut title.text, deleted);
                    }
                    if let Some(label) = axis
                        .units
                        .as_mut()
                        .and_then(|units| units.units_label.as_mut())
                    {
                        self.remove_text_source(&mut label.text, deleted);
                    }
                }
                for series in plot_area
                    .plot_area_region
                    .iter_mut()
                    .flat_map(|region| &mut region.series)
                {
                    self.remove_text_source(&mut series.text, deleted);
                }
            }
        }
        Ok(())
    }

    fn remove_dimension_source<L: Default>(
        &mut self,
        source: &mut ChartExDimensionSource<L>,
        deleted: &DeletedSheet<'_>,
    ) -> Result<(), String> {
        let ChartExDimensionSource::Reference(reference) = source else {
            return Ok(());
        };
        if let Some(removed) = deleted.source(reference.formula()) {
            let count = source_level_count(&removed.address, reference.formula().direction())?;
            *source = ChartExDimensionSource::Literal((0..count).map(|_| L::default()).collect());
            self.removed(&removed);
        } else if let Some(removed) = reference
            .name_formula()
            .and_then(|formula| deleted.source(formula))
        {
            reference.set_name_formula(None);
            self.removed(&removed);
        }
        Ok(())
    }

    fn remove_text_source(&mut self, text: &mut Option<ChartExText>, deleted: &DeletedSheet<'_>) {
        let removed = text
            .as_ref()
            .and_then(|text| text.tx_data.as_ref())
            .and_then(ChartExTextData::formula)
            .and_then(|formula| deleted.source(formula));
        if let Some(removed) = removed {
            *text = None;
            self.removed(&removed);
        }
    }

    fn removed(&mut self, source: &DeletedSource) {
        self.report.adjustments.changed += 1;
        self.report.adjustments.invalidated += source.invalidated;
    }

    fn finish_unused_names(&mut self, sources: &[ChartExDefinedNameSource]) {
        for reference in &self.report.defined_names {
            let identity = name_identity(reference);
            if sources
                .iter()
                .any(|source| name_identity(&source.reference) == identity)
                && !self.remaining_names.contains(&identity)
            {
                self.report.unused_defined_names.push(reference.clone());
            }
        }
    }
}

fn name_identity(reference: &ChartExDefinedNameReference) -> NameIdentity {
    (
        reference.name.to_lowercase(),
        reference.sheet.as_ref().map(|value| value.to_lowercase()),
        reference.workbook_scoped,
    )
}

/// The number of levels the deleted source supplied, in the formula's
/// direction.
fn source_level_count(address: &str, direction: ChartExFormulaDirection) -> Result<usize, String> {
    for operand in chart_formula_operands(address) {
        let (_, range) = split_reference_address(operand.text);
        if !is_native_coordinate_operand(range) {
            continue;
        }
        let (first, last) = range.split_once(':').unwrap_or((range, range));
        let (first_column, first_row, ..) = index_from_coordinate(first.to_ascii_uppercase());
        let (last_column, last_row, ..) = index_from_coordinate(last.to_ascii_uppercase());
        let extent = match direction {
            ChartExFormulaDirection::Column => coordinate_extent(first_column, last_column, 16_384),
            ChartExFormulaDirection::Row => coordinate_extent(first_row, last_row, 1_048_576),
        };
        if let Some((start, end)) = extent {
            if end >= start {
                return usize::try_from(end - start + 1)
                    .map_err(|_| format!("ChartEx level count exceeds usize for {address}"));
            }
        }
    }
    Err(format!(
        "cannot preserve ChartEx levels from deleted source {address}"
    ))
}

/// A whole-row or whole-column operand spans the worksheet's extent.
fn coordinate_extent(first: Option<u32>, last: Option<u32>, limit: u32) -> Option<(u32, u32)> {
    match (first, last) {
        (Some(first), Some(last)) => Some((first, last)),
        (None, None) => Some((1, limit)),
        _ => None,
    }
}

impl ChartExSpace {
    /// Adjust direct formula operands with the workbook's edit engine and
    /// report defined-name dependencies for the caller to update once.
    /// Sheet qualifiers on name operands follow workbook rename/delete rules;
    /// the referenced definitions themselves are never changed here.
    /// Invalid edit coordinates/order return an error before any mutation.
    ///
    /// `DeleteSheet` gives Excel's result for `ChartEx`. A dimension that reads
    /// the deleted sheet becomes empty literal levels (`ptCount="0"`, one per
    /// source row or column, whatever cached points it held), the series and
    /// chart, axis and units-label texts that read it are removed, and a
    /// dimension name formula that reads it is dropped. Only a direct operand
    /// is recognised here; for a name operand use
    /// `adjust_references_with_deleted_names`. A deleted source whose level
    /// count cannot be read from a plain cell range fails with "cannot
    /// preserve `ChartEx` levels" before anything changes.
    pub fn adjust_references(
        &mut self,
        host_sheet: &str,
        edit: &ChartReferenceEdit,
    ) -> Result<ChartExReferenceAdjustmentReport, String> {
        self.adjust_references_with_deleted_names(host_sheet, edit, &[])
    }

    /// Adjust this `ChartEx` space. For sheet deletion, `deleted_names`
    /// supplies the exact name operands the caller resolved to that sheet,
    /// with their original definition formulas. Converted dimensions become
    /// literal empty levels and the report identifies names now unused.
    pub fn adjust_references_with_deleted_names(
        &mut self,
        host_sheet: &str,
        edit: &ChartReferenceEdit,
        deleted_names: &[ChartExDefinedNameSource],
    ) -> Result<ChartExReferenceAdjustmentReport, String> {
        let prepared = PreparedReferenceEdit::new(edit)
            .map_err(|error| format!("invalid ChartEx reference edit: {error:?}"))?;
        let mut report = EditReport::default();
        let mut updated = self.clone();
        report.space(&mut updated, host_sheet, &prepared, deleted_names)?;
        report.finish_unused_names(deleted_names);
        *self = updated;
        Ok(report.report)
    }
}

impl ExtendedChart {
    /// Adjust this detached chart, including native/standard frames in every
    /// compatibility branch. Uses one-based coordinates and the complete
    /// pre-delete sheet order, just like workbook structural operations.
    /// Defined-name dependencies are reported once; their definitions remain
    /// owned by the caller. Invalid edits fail before mutating any frame.
    /// Unlike `Chart::adjust_references`, which leaves the chart unchanged on
    /// an invalid edit, the error is returned so the caller can react.
    ///
    /// `DeleteSheet` gives Excel's result for `ChartEx`. A dimension that reads
    /// the deleted sheet becomes empty literal levels (`ptCount="0"`, one per
    /// source row or column, whatever cached points it held), the series and
    /// chart, axis and units-label texts that read it are removed, and a
    /// dimension name formula that reads it is dropped. Only a direct operand
    /// is recognised here; for a name operand use
    /// `adjust_references_with_deleted_names`. A deleted source whose level
    /// count cannot be read from a plain cell range fails with "cannot
    /// preserve `ChartEx` levels" before anything changes.
    pub fn adjust_references(
        &mut self,
        host_sheet: &str,
        edit: &ChartReferenceEdit,
    ) -> Result<ChartExReferenceAdjustmentReport, String> {
        self.adjust_references_with_deleted_names(host_sheet, edit, &[])
    }

    /// Adjust this detached chart. `deleted_names` is the set of exact name
    /// operands whose definitions the caller resolved to the deleted sheet,
    /// together with their original formulas. The report's
    /// `unused_defined_names` identifies the operands the caller can now
    /// remove from its name table.
    pub fn adjust_references_with_deleted_names(
        &mut self,
        host_sheet: &str,
        edit: &ChartReferenceEdit,
        deleted_names: &[ChartExDefinedNameSource],
    ) -> Result<ChartExReferenceAdjustmentReport, String> {
        let prepared = PreparedReferenceEdit::new(edit)
            .map_err(|error| format!("invalid ChartEx reference edit: {error:?}"))?;
        let mut updated = self.clone();
        let mut report = EditReport::default();
        let mut error = None;
        updated.visit_extended_frames_mut(&mut |_, frame| {
            if error.is_none() {
                error = report
                    .space(
                        frame.chart_space_mut(),
                        host_sheet,
                        &prepared,
                        deleted_names,
                    )
                    .err();
            }
        });
        if let Some(error) = error {
            return Err(error);
        }
        updated.visit_standard_frames_mut(&mut |_, frame| {
            let preserved = visit_mut(
                frame.graphic_mut().graphic_data_mut().chart_space_mut(),
                &mut |metadata, mut target| {
                    let decision = report.decision(
                        &target.formula().address_str(),
                        host_sheet,
                        &prepared,
                        metadata.grammar == ReferenceGrammar::LabelRangeFormula,
                    );
                    if target.replace_for_edit(edit, &decision.text) {
                        report.report.adjustments.changed += 1;
                    }
                },
            );
            report.report.adjustments.preserved_opaque_extensions += preserved;
        });
        report.finish_unused_names(deleted_names);
        *self = updated;
        Ok(report.report)
    }
}
