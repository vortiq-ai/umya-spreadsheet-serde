use std::collections::{
    HashMap,
    HashSet,
};

use super::Workbook;
use crate::{
    DefinedName,
    Worksheet,
    XlsxError,
    XlsxErrorKind,
    drawing::charts::{
        ChartReferenceEdit,
        chart_reference_roles::visit_ref,
        reference_edit_engine::{
            PreparedReferenceEdit,
            edit_prepared_formula_text,
        },
    },
    helper::{
        address::{
            name_eq_ignore_case,
            split_reference_address,
            unescape_sheet_name,
        },
        formula::chart_formula_operands,
    },
};

type NameScopes = Vec<(String, HashSet<String>)>;

fn chart_name(name: &str) -> Option<(u32, u32)> {
    let lower = name.to_ascii_lowercase();
    let (version, index) = lower.strip_prefix("_xlchart.v")?.split_once('.')?;
    Some((version.parse().ok()?, index.parse().ok()?))
}

fn is_global_operand(qualifier: &str, name: &str, host: &str, scopes: &NameScopes) -> bool {
    if qualifier == "[0]" {
        return true;
    }
    if qualifier.contains(['[', ']']) {
        return false;
    }
    let sheet = if qualifier.is_empty() {
        host.to_owned()
    } else {
        unescape_sheet_name(qualifier).into_owned()
    };
    !scopes.iter().any(|(scope, names)| {
        name_eq_ignore_case(scope, &sheet) && names.contains(&name.to_ascii_lowercase())
    })
}

fn generated_operands(text: &str, host: &str, scopes: &NameScopes) -> Vec<String> {
    chart_formula_operands(text)
        .into_iter()
        .filter_map(|operand| {
            let (qualifier, name) = split_reference_address(operand.text);
            if chart_name(name).is_some() && is_global_operand(qualifier, name, host, scopes) {
                Some(name.to_ascii_lowercase())
            } else {
                None
            }
        })
        .collect()
}

fn rewrite_names(
    text: &str,
    host: &str,
    names: &HashMap<String, String>,
    scopes: &NameScopes,
) -> String {
    let mut output = text.to_owned();
    let replacements = chart_formula_operands(text)
        .into_iter()
        .filter_map(|operand| {
            let (qualifier, name) = split_reference_address(operand.text);
            if !is_global_operand(qualifier, name, host, scopes) {
                return None;
            }
            names.get(&name.to_ascii_lowercase()).map(|replacement| {
                (
                    (operand.span.end - name.len())..operand.span.end,
                    replacement,
                )
            })
        })
        .collect::<Vec<_>>();
    for (span, replacement) in replacements.into_iter().rev() {
        output.replace_range(span, replacement);
    }
    output
}

fn rewrite_sheet_names(
    sheet: &mut Worksheet,
    names: &HashMap<String, String>,
    scopes: &NameScopes,
) {
    let host = sheet.name().to_owned();
    let rewrite = |text: &str| rewrite_names(text, &host, names, scopes);
    sheet
        .worksheet_drawing_mut()
        .visit_extended_spaces_mut(&mut |space| {
            space.visit_formulas_mut(&mut |formula| {
                formula.replace_reference_text_preserving_address_identity(&rewrite(
                    &formula.address_str(),
                ));
            });
        });
}

fn copy_error(message: &'static str) -> XlsxError {
    XlsxError::with_message(XlsxErrorKind::InvalidArgument, message)
}

fn has_external_generated_name_consumer(sheet: &Worksheet, scopes: &NameScopes) -> bool {
    let references = |text: &str| !generated_operands(text, sheet.name(), scopes).is_empty();
    for cell in sheet.cells() {
        if let Some(formula) = cell.cell_value().formula_obj() {
            let found = std::cell::Cell::new(false);
            formula.clone().rewrite_reference_texts(&|text| {
                found.set(found.get() || references(text));
                text.to_owned()
            });
            if found.get() {
                return true;
            }
        }
    }
    let mut chart_reference = false;
    sheet.visit_chart_spaces(&mut |space| {
        visit_ref(space, &mut |_, target| {
            chart_reference |= references(&target.formula().address_str());
        });
    });
    chart_reference
        || sheet
            .defined_names()
            .iter()
            .any(|name| references(&name.address()))
        || sheet.pivot_tables().iter().any(|pivot| {
            pivot
                .pivot_cache_definition()
                .cache_source()
                .worksheet_source()
                .is_some_and(|source| references(source.name()))
        })
}

impl Workbook {
    /// Excel rebuilds generated globals from live `ChartEx` owners on save.
    /// Dimension name formulas (`nf`) participate, including map headers.
    pub(super) fn prepare_sheet_copy_chart_names(
        &mut self,
        source: usize,
        copy: &mut Worksheet,
    ) -> Result<(), XlsxError> {
        let mut scopes = self
            .work_sheet_collection
            .iter()
            .map(|sheet| (sheet.name().to_owned(), HashSet::new()))
            .collect::<NameScopes>();
        for name in self.defined_names.iter().chain(
            self.work_sheet_collection
                .iter()
                .flat_map(Worksheet::defined_names),
        ) {
            if name.has_local_sheet_id() {
                if let Some((_, names)) = scopes.get_mut(name.local_sheet_id() as usize) {
                    names.insert(name.name().to_ascii_lowercase());
                }
            }
        }
        scopes.push((
            copy.name().to_owned(),
            copy.defined_names()
                .iter()
                .map(|name| name.name().to_ascii_lowercase())
                .collect(),
        ));
        let mut globals = HashMap::new();
        for name in self.defined_names.iter().chain(
            self.work_sheet_collection
                .iter()
                .flat_map(Worksheet::defined_names),
        ) {
            if !name.has_local_sheet_id()
                && chart_name(name.name()).is_some()
                && globals
                    .insert(name.name().to_ascii_lowercase(), name.clone())
                    .is_some()
            {
                return Err(copy_error("duplicate ChartEx generated global name"));
            }
        }
        if globals.is_empty() {
            return Ok(());
        }
        if self
            .work_sheet_collection
            .iter()
            .chain(std::iter::once(&*copy))
            .any(|sheet| has_external_generated_name_consumer(sheet, &scopes))
            || self
                .defined_names
                .iter()
                .any(|name| !generated_operands(&name.address(), "", &scopes).is_empty())
            || self.pivot_orphans.iter().any(|(_, cache)| {
                cache
                    .cache_source()
                    .worksheet_source()
                    .is_some_and(|source| {
                        !generated_operands(source.name(), source.address().sheet_name(), &scopes)
                            .is_empty()
                    })
            })
        {
            return Err(copy_error(
                "sheet copy cannot reconcile generated ChartEx names consumed outside ChartEx",
            ));
        }
        let mut ordered = Vec::new();
        let mut retained = HashSet::new();
        for sheet in &self.work_sheet_collection {
            sheet
                .worksheet_drawing()
                .visit_extended_spaces(&mut |space| {
                    let mut names = HashSet::new();
                    space.visit_formulas(&mut |formula| {
                        names.extend(generated_operands(
                            &formula.address_str(),
                            sheet.name(),
                            &scopes,
                        ));
                    });
                    let mut names = names.into_iter().collect::<Vec<_>>();
                    names.sort_by_key(|name| {
                        chart_name(name).map(|(version, number)| (number, version))
                    });
                    for name in names {
                        if retained.insert(name.clone()) {
                            ordered.push(name);
                        }
                    }
                });
        }
        let mut selected = HashSet::new();
        let mut copy_order = Vec::new();
        copy.worksheet_drawing()
            .visit_extended_spaces(&mut |space| {
                let mut names = HashSet::new();
                space.visit_formulas(&mut |formula| {
                    names.extend(generated_operands(
                        &formula.address_str(),
                        copy.name(),
                        &scopes,
                    ));
                });
                let mut names = names.into_iter().collect::<Vec<_>>();
                names.sort_by_key(|name| {
                    chart_name(name).map(|(version, number)| (number, version))
                });
                for name in names {
                    if selected.insert(name.clone()) {
                        copy_order.push(name);
                    }
                }
            });
        if ordered
            .iter()
            .chain(&copy_order)
            .any(|name| !globals.contains_key(name))
        {
            return Err(copy_error(
                "ChartEx references a missing generated global name",
            ));
        }
        let mut normalized = HashMap::new();
        for (index, name) in ordered.iter().enumerate() {
            let (version, _) = chart_name(name).expect("generated operand identity");
            let index = u32::try_from(index)
                .map_err(|_| copy_error("ChartEx generated name namespace exhausted"))?;
            normalized.insert(name.clone(), format!("_xlchart.v{version}.{index}"));
        }
        let old = self.work_sheet_collection[source].name().to_owned();
        let edit = ChartReferenceEdit::RenameSheet {
            old: old.clone(),
            new: copy.name().to_owned(),
        };
        let prepared = PreparedReferenceEdit::new(&edit)
            .map_err(|_| copy_error("invalid ChartEx sheet copy references"))?;
        let mut next = u32::try_from(ordered.len())
            .map_err(|_| copy_error("ChartEx generated name namespace exhausted"))?;
        let mut replacements = HashMap::new();
        for name in &copy_order {
            let (version, _) = chart_name(name).expect("generated operand identity");
            replacements.insert(name.clone(), format!("_xlchart.v{version}.{next}"));
            next = next
                .checked_add(1)
                .ok_or_else(|| copy_error("ChartEx generated name namespace exhausted"))?;
        }
        let copied = copy_order
            .iter()
            .map(|name| {
                let mut definition = globals[name].clone();
                definition.set_name(&replacements[name]);
                let address =
                    edit_prepared_formula_text(&definition.address(), &old, &prepared, false).text;
                definition.set_address(address);
                definition
            })
            .collect::<Vec<DefinedName>>();
        // Allocation and dependency validation finish before mutating owners.
        self.defined_names.retain(|name| {
            name.has_local_sheet_id()
                || chart_name(name.name()).is_none()
                || retained.contains(&name.name().to_ascii_lowercase())
        });
        for name in &mut self.defined_names {
            if !name.has_local_sheet_id() {
                if let Some(replacement) = normalized.get(&name.name().to_ascii_lowercase()) {
                    name.set_name(replacement);
                }
            }
        }
        for sheet in &mut self.work_sheet_collection {
            sheet.defined_names_mut().retain(|name| {
                name.has_local_sheet_id()
                    || chart_name(name.name()).is_none()
                    || retained.contains(&name.name().to_ascii_lowercase())
            });
            for name in sheet.defined_names_mut() {
                if !name.has_local_sheet_id() {
                    if let Some(replacement) = normalized.get(&name.name().to_ascii_lowercase()) {
                        name.set_name(replacement);
                    }
                }
            }
            rewrite_sheet_names(sheet, &normalized, &scopes);
        }
        rewrite_sheet_names(copy, &replacements, &scopes);
        self.defined_names.extend(copied);
        Ok(())
    }
}
