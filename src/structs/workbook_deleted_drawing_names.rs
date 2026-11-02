//! Excel keeps a hidden `_xlchart.*` name only while a surviving `ChartEx`
//! reads it. A formula cell, conditional format, defined name or data
//! validation that mentions the name does not keep it, so deleting a sheet or
//! the drawing that owned the last `ChartEx` consumer removes the name too.

use std::collections::HashSet;

use super::Workbook;
use crate::{
    drawing::charts::reference_edit_engine::is_named_operand,
    helper::{
        address::{
            name_eq_ignore_case,
            split_reference_address,
            unescape_sheet_name,
        },
        formula::{
            chart_formula_operands,
            mentions_generated_chart_name,
        },
    },
    structs::{
        DefinedName,
        Worksheet,
    },
};

pub(crate) type Identity = (String, Option<u32>);

fn identity(name: &DefinedName) -> Identity {
    (
        name.name().to_lowercase(),
        name.has_local_sheet_id().then(|| name.local_sheet_id()),
    )
}

fn is_generated_chart_name(name: &DefinedName) -> bool {
    name.hidden() && name.name().to_ascii_lowercase().starts_with("_xlchart.")
}

struct NameTable<'a> {
    names:  Vec<&'a DefinedName>,
    sheets: Vec<&'a str>,
}

impl<'a> NameTable<'a> {
    fn new(book: &'a Workbook) -> Self {
        Self {
            names:  book
                .defined_names
                .iter()
                .chain(
                    book.work_sheet_collection
                        .iter()
                        .flat_map(Worksheet::defined_names),
                )
                .collect(),
            sheets: book
                .work_sheet_collection
                .iter()
                .map(Worksheet::name)
                .collect(),
        }
    }

    /// The definitions the names in `text` select when read by a formula on
    /// the sheet `host`.
    fn references(&self, text: &str, host: &str) -> HashSet<Identity> {
        if !mentions_generated_chart_name(text) {
            return HashSet::new();
        }
        chart_formula_operands(text)
            .into_iter()
            .filter_map(|operand| {
                let (qualifier, name) = split_reference_address(operand.text);
                if !is_named_operand(name)
                    || qualifier.contains(':')
                    || (qualifier != "[0]" && operand.text.contains(['[', ']']))
                {
                    return None;
                }
                let scope = if qualifier.is_empty() {
                    host.into()
                } else if qualifier == "[0]" {
                    "".into()
                } else {
                    unescape_sheet_name(qualifier)
                };
                let local = self
                    .sheets
                    .iter()
                    .position(|sheet| name_eq_ignore_case(sheet, &scope))
                    .and_then(|index| u32::try_from(index).ok());
                self.names
                    .iter()
                    .find(|definition| {
                        definition.has_local_sheet_id()
                            && Some(definition.local_sheet_id()) == local
                            && name_eq_ignore_case(definition.name(), name)
                    })
                    .or_else(|| {
                        self.names.iter().find(|definition| {
                            !definition.has_local_sheet_id()
                                && name_eq_ignore_case(definition.name(), name)
                        })
                    })
                    .map(|definition| identity(definition))
            })
            .collect()
    }
}

impl Worksheet {
    /// The formulas of every `ChartEx` on this sheet.
    fn visit_chart_ex_name_formulas(&self, visit: &mut dyn FnMut(&str)) {
        self.worksheet_drawing()
            .visit_extended_spaces(&mut |space| {
                space.visit_formulas(&mut |formula| visit(&formula.address_str()));
            });
    }
}

impl Workbook {
    /// The part of `candidates` that no surviving `ChartEx` reads. `formulas`
    /// holds every formula of those `ChartEx` with the name of its host sheet.
    /// Sheet deletion and drawing deletion share this rule.
    pub(crate) fn generated_names_without_chart_ex_consumer(
        &self,
        mut candidates: HashSet<Identity>,
        formulas: &[(String, String)],
    ) -> HashSet<Identity> {
        let table = NameTable::new(self);
        for (host, text) in formulas {
            for used in table.references(text, host) {
                candidates.remove(&used);
            }
        }
        candidates
    }

    /// Drop the generated names of the deleted `ChartEx` whose formulas are
    /// `removed` (on the sheet `host`) that no surviving `ChartEx` reads. All
    /// sheets must be materialized before this pass.
    pub(crate) fn reconcile_removed_drawing_chart_names(&mut self, host: &str, removed: &[String]) {
        if removed.is_empty() {
            return;
        }
        let table = NameTable::new(self);
        let mut candidates = HashSet::new();
        for text in removed {
            candidates.extend(table.references(text, host));
        }
        candidates.retain(|candidate| {
            table
                .names
                .iter()
                .any(|name| identity(name) == *candidate && is_generated_chart_name(name))
        });
        let mut surviving = Vec::new();
        for sheet in &self.work_sheet_collection {
            sheet.visit_chart_ex_name_formulas(&mut |text| {
                surviving.push((sheet.name().to_owned(), text.to_owned()));
            });
        }
        let unused = self.generated_names_without_chart_ex_consumer(candidates, &surviving);
        self.defined_names
            .retain(|name| !unused.contains(&identity(name)));
        for sheet in &mut self.work_sheet_collection {
            sheet
                .defined_names_mut()
                .retain(|name| !unused.contains(&identity(name)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drawing::{
        charts::{
            ChartExBuilder,
            ChartExFormula,
        },
        spreadsheet::ExtendedChart,
    };

    fn name(text: &str, scope: Option<u32>, address: &str) -> DefinedName {
        let mut name = DefinedName::default();
        name.set_name(text).set_hidden(true);
        name.set_address(address);
        if let Some(scope) = scope {
            name.set_local_sheet_id(scope);
        }
        name
    }

    fn waterfall_reading(categories: &str, values: &str) -> ExtendedChart {
        let space = ChartExBuilder::waterfall(
            ChartExFormula::new(categories),
            ChartExFormula::new(values),
            [],
            false,
        )
        .build()
        .unwrap();
        ExtendedChart::new(Default::default(), space)
    }

    #[test]
    fn deletion_keeps_only_names_a_surviving_chart_ex_reads() {
        let mut book = crate::new_file();
        for definition in [
            name("_xlchart.kept", None, "Sheet1!$B$1"),
            name("_xlchart.removed", None, "Sheet1!$B$2"),
            name("_xlchart.shadow", None, "Sheet1!$B$3"),
            name("_xlchart.shadow", Some(0), "Sheet1!$B$4"),
            name("_xlchart.unrelated", None, "Sheet1!$B$5"),
        ] {
            book.defined_names_mut().push(definition);
        }
        book.new_sheet("Charts").unwrap();
        book.sheet_mut(1)
            .unwrap()
            .worksheet_drawing_mut()
            .add_extended_chart(waterfall_reading("_xlchart.kept", "_xlchart.kept"));
        book.reconcile_removed_drawing_chart_names(
            "Sheet1",
            &[
                "_xlchart.kept".into(),
                "_xlchart.removed".into(),
                "_xlchart.shadow".into(),
            ],
        );
        let names: HashSet<_> = book.defined_names().iter().map(identity).collect();
        assert_eq!(
            names,
            HashSet::from([
                ("_xlchart.kept".into(), None),
                ("_xlchart.shadow".into(), None),
                ("_xlchart.unrelated".into(), None),
            ])
        );
    }

    #[test]
    fn a_cell_or_name_that_mentions_a_generated_name_does_not_keep_it() {
        let mut book = crate::new_file();
        for definition in [
            name("_xlchart.first", None, "Sheet1!$B$1"),
            name("_xlchart.second", None, "Sheet1!$B$2"),
            name("Consumer", None, "_xlchart.first"),
        ] {
            book.defined_names_mut().push(definition);
        }
        let sheet = book.sheet_mut(0).unwrap();
        sheet.cell_mut("A1").set_formula("_xlchart.second");
        book.reconcile_removed_drawing_chart_names(
            "Sheet1",
            &["_xlchart.first".into(), "_xlchart.second".into()],
        );
        let names: Vec<_> = book.defined_names().iter().map(identity).collect();
        assert_eq!(names, vec![("consumer".to_owned(), None)]);
        assert_eq!(
            book.sheet(0).unwrap().cell("A1").unwrap().formula(),
            "_xlchart.second"
        );
    }

    #[test]
    fn only_text_naming_a_generated_name_is_worth_tokenizing() {
        assert!(mentions_generated_chart_name("SUM(_XLCHART.v1.0)"));
        assert!(!mentions_generated_chart_name("=SUM(A1:A3)"));
        assert!(!mentions_generated_chart_name("="));
    }
}
