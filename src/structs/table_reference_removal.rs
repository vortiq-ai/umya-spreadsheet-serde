use crate::{
    Table,
    Workbook,
    Worksheet,
    drawing::charts::chart_reference_roles::{
        visit_mut,
        visit_ref,
    },
    structs::defined_name::remove_table_references,
};

pub(super) struct TableReferenceRemoval {
    removed: Vec<(String, Option<String>)>,
}

impl TableReferenceRemoval {
    pub(super) fn prepare(
        book: &mut Workbook,
        sheet: &str,
        planned: &[Table],
    ) -> Result<Self, String> {
        let plan = Self::prepare_sheet(
            book.sheet_by_name(sheet)
                .map_err(|error| error.to_string())?,
            planned,
        )?;
        if plan.removed.is_empty() {
            return Ok(plan);
        }
        book.try_read_sheet_collection()
            .map_err(|error| error.to_string())?;
        for name in book.defined_names() {
            plan.rewrite(&name.address())?;
        }
        for sheet in book.sheet_collection() {
            plan.validate_sheet(sheet)?;
        }
        Ok(plan)
    }

    pub(super) fn prepare_sheet(sheet: &Worksheet, planned: &[Table]) -> Result<Self, String> {
        let mut removed = Vec::new();
        for table in sheet.tables() {
            if let Some(next) = planned
                .iter()
                .find(|next| next.name().eq_ignore_ascii_case(table.name()))
            {
                for column in table.columns() {
                    if !next
                        .columns()
                        .iter()
                        .any(|next| next.name().eq_ignore_ascii_case(column.name()))
                    {
                        removed.push((table.name().to_owned(), Some(column.name().to_owned())));
                    }
                }
            } else {
                removed.push((table.name().to_owned(), None));
            }
        }
        let plan = Self { removed };
        if plan.removed.is_empty() {
            return Ok(plan);
        }
        plan.validate_sheet(sheet)?;
        Ok(plan)
    }

    fn validate_sheet(&self, sheet: &Worksheet) -> Result<(), String> {
        for name in sheet.defined_names() {
            self.rewrite(&name.address())?;
        }
        for cell in sheet.cells() {
            self.rewrite(cell.formula())?;
        }
        let mut result = Ok(());
        sheet.visit_chart_spaces(&mut |space| {
            visit_ref(space, &mut |_, target| {
                if let Err(error) = self.rewrite(&target.formula().address_str()) {
                    result = Err(error);
                }
            });
        });
        sheet
            .worksheet_drawing()
            .visit_extended_spaces(&mut |space| {
                space.visit_formulas(&mut |formula| {
                    if let Err(error) = self.rewrite(&formula.address_str()) {
                        result = Err(error);
                    }
                });
            });
        result
    }

    fn rewrite(&self, formula: &str) -> Result<Option<String>, String> {
        remove_table_references(formula, &self.removed)
    }

    pub(super) fn apply(self, book: &mut Workbook) {
        if self.removed.is_empty() {
            return;
        }
        for name in book.defined_names_mut() {
            if let Some(text) = self
                .rewrite(&name.address())
                .expect("preflighted table reference")
            {
                name.set_address(text);
            }
        }
        for sheet in book.sheet_collection_mut() {
            self.apply_sheet(sheet);
        }
    }

    pub(super) fn apply_sheet(&self, sheet: &mut Worksheet) {
        for name in sheet.defined_names_mut() {
            if let Some(text) = self
                .rewrite(&name.address())
                .expect("preflighted table reference")
            {
                name.set_address(text);
            }
        }
        for cell in sheet.cells_mut() {
            if let Some(text) = self
                .rewrite(cell.formula())
                .expect("preflighted table reference")
            {
                let mut formula = cell.formula_obj().expect("rewritten formula cell").clone();
                formula.set_text(text);
                cell.cell_value_mut().set_formula_obj(formula);
                cell.cell_value_mut().set_formula_result_blank();
            }
        }
        sheet.visit_chart_spaces_mut(&mut |space| {
            visit_mut(space, &mut |_, mut target| {
                if let Some(text) = self
                    .rewrite(&target.formula_mut().address_str())
                    .expect("preflighted table reference")
                {
                    target.replace_decided_text(&text);
                }
            });
        });
        sheet
            .worksheet_drawing_mut()
            .visit_extended_spaces_mut(&mut |space| {
                space.visit_formulas_mut(&mut |formula| {
                    if let Some(text) = self
                        .rewrite(&formula.address_str())
                        .expect("preflighted table reference")
                    {
                        formula.replace_reference_text_preserving_address_identity(&text);
                    }
                });
            });
    }
}
