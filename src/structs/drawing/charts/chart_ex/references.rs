//! Detached edits share the workbook's reference grammar and geometry.
use std::collections::HashSet;

use super::{
    ChartExSpace,
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
            },
        },
        spreadsheet::ExtendedChart,
    },
    helper::{
        address::{
            split_reference_address,
            unescape_sheet_name,
        },
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

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ChartExReferenceAdjustmentReport {
    pub adjustments:   ReferenceAdjustmentReport,
    /// Each name/scope identity occurs once, in formula traversal order.
    /// Definitions are neither resolved nor rewritten by detached chart edits.
    pub defined_names: Vec<ChartExDefinedNameReference>,
}

#[derive(Default)]
struct EditReport {
    report: ChartExReferenceAdjustmentReport,
    names:  HashSet<(String, Option<String>, bool)>,
}

impl EditReport {
    fn decision(
        &mut self,
        text: &str,
        host: &str,
        edit: &PreparedReferenceEdit<'_>,
        bare_error: bool,
    ) -> FormulaEdit {
        self.report.adjustments.visited += 1;
        for operand in chart_formula_operands(text) {
            let (qualifier, name) = split_reference_address(operand.text);
            if !is_named_operand(name) || qualifier.contains(':') {
                continue;
            }
            let workbook_scoped = qualifier == "[0]";
            if !workbook_scoped && operand.text.contains(['[', ']']) {
                continue;
            }
            let sheet = (!qualifier.is_empty() && !workbook_scoped)
                .then(|| unescape_sheet_name(qualifier).into_owned());
            let identity = (
                name.to_lowercase(),
                sheet.as_ref().map(|value| value.to_lowercase()),
                workbook_scoped,
            );
            if self.names.insert(identity) {
                self.report.defined_names.push(ChartExDefinedNameReference {
                    name: name.into(),
                    sheet,
                    workbook_scoped,
                });
            }
        }
        let decision = edit_prepared_formula_text(text, host, edit, bare_error);
        self.report.adjustments.invalidated += decision.invalidated;
        decision
    }

    fn space(&mut self, space: &mut ChartExSpace, host: &str, edit: &PreparedReferenceEdit<'_>) {
        self.report.adjustments.preserved_opaque_extensions += space.opaque_extension_count();
        space.visit_formulas_mut(&mut |formula| {
            let decision = self.decision(&formula.address_str(), host, edit, false);
            if formula.replace_reference_text_preserving_address_identity(&decision.text) {
                self.report.adjustments.changed += 1;
            }
        });
    }
}

impl ChartExSpace {
    /// Adjust direct formula operands with the workbook's edit engine and
    /// report defined-name dependencies for the caller to update once.
    /// Sheet qualifiers on name operands follow workbook rename/delete rules;
    /// the referenced definitions themselves are never changed here.
    /// Invalid edit coordinates/order return an error before any mutation.
    pub fn adjust_references(
        &mut self,
        host_sheet: &str,
        edit: &ChartReferenceEdit,
    ) -> Result<ChartExReferenceAdjustmentReport, String> {
        let prepared = PreparedReferenceEdit::new(edit)
            .map_err(|error| format!("invalid ChartEx reference edit: {error:?}"))?;
        let mut report = EditReport::default();
        report.space(self, host_sheet, &prepared);
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
    pub fn adjust_references(
        &mut self,
        host_sheet: &str,
        edit: &ChartReferenceEdit,
    ) -> Result<ChartExReferenceAdjustmentReport, String> {
        let prepared = PreparedReferenceEdit::new(edit)
            .map_err(|error| format!("invalid ChartEx reference edit: {error:?}"))?;
        let mut report = EditReport::default();
        self.visit_extended_frames_mut(&mut |_, frame| {
            report.space(frame.chart_space_mut(), host_sheet, &prepared);
        });
        self.visit_standard_frames_mut(&mut |_, frame| {
            let preserved = visit_mut(
                frame.graphic_mut().graphic_data_mut().chart_space_mut(),
                &mut |metadata, mut target| {
                    let decision = report.decision(
                        &target.formula().address_str(),
                        host_sheet,
                        &prepared,
                        metadata.grammar == ReferenceGrammar::LabelRangeFormula,
                    );
                    if target.replace_decided_text(&decision.text) {
                        report.report.adjustments.changed += 1;
                    }
                },
            );
            report.report.adjustments.preserved_opaque_extensions += preserved;
        });
        Ok(report.report)
    }
}
