//! Table and pivot geometry owned by row/column operations.
use crate::{
    Location,
    PivotTable,
    Table,
    Workbook,
    Worksheet,
    traits::AdjustmentCoordinateWithSheet,
};

pub(crate) struct ModelGeometryPlan {
    tables:              Vec<Table>,
    pivot_locations:     Vec<Option<Location>>,
    removed_pivot_names: Vec<String>,
    removed_pivots:      Vec<PivotTable>,
    removed_tables:      Vec<Table>,
    sheet_id:            String,
    headers:             Vec<(u32, u32, String)>,
    cleared_columns:     Vec<(u32, u32)>,
    retained_row:        Option<u32>,
    frozen_charts:       super::FrozenPivotCharts,
    reference_removal:   Option<super::table_reference_removal::TableReferenceRemoval>,
}

impl Worksheet {
    pub(crate) fn adjust_model_coordinates(
        &mut self,
        host: &str,
        coordinates: (u32, u32, u32, u32),
        remove: bool,
    ) {
        let (col, cols, row, rows) = coordinates;
        if host != self.name() {
            if remove {
                self.adjustment_remove_coordinate_with_sheet(host, col, cols, row, rows);
            } else {
                self.adjustment_insert_coordinate_with_sheet(host, col, cols, row, rows);
            }
            return;
        }
        let Ok(mut plan) = self.prepare_model_geometry(col, cols, row, rows, remove) else {
            return;
        };
        // A Worksheet cannot reconcile workbook-owned slicers, caches or
        // pivot charts. Refuse model removal until the Workbook owns the edit.
        if !plan.removed_tables.is_empty() || !plan.removed_pivot_names.is_empty() {
            return;
        }
        let Ok(reference_removal) =
            super::table_reference_removal::TableReferenceRemoval::prepare_sheet(
                self,
                plan.tables(),
            )
        else {
            return;
        };
        reference_removal.apply_sheet(self);
        plan.adjust_geometry(self, coordinates, remove);
        plan.adjust_sheet_references(self, host, coordinates, remove);
    }

    pub(crate) fn prepare_model_geometry(
        &self,
        col: u32,
        cols: u32,
        row: u32,
        rows: u32,
        remove: bool,
    ) -> Result<ModelGeometryPlan, String> {
        let mut plan = ModelGeometryPlan {
            tables:              Vec::new(),
            pivot_locations:     Vec::new(),
            removed_pivot_names: Vec::new(),
            removed_pivots:      Vec::new(),
            removed_tables:      Vec::new(),
            sheet_id:            self.sheet_id().into(),
            headers:             Vec::new(),
            cleared_columns:     Vec::new(),
            retained_row:        None,
            frozen_charts:       Vec::new(),
            reference_removal:   None,
        };
        let mut retained_tables = Vec::new();
        for table in self.tables() {
            let mut next = table.clone();
            if remove && next.remove_geometry(col, cols, row, rows)? {
                plan.removed_tables.push(table.clone());
                continue;
            }
            if !remove {
                next.insert_geometry(col, cols, row, rows)?;
                if cols != 0 && next.columns().len() > table.columns().len() {
                    for column in col..col + cols {
                        plan.headers.push((
                            column,
                            next.area().0.row_num(),
                            next.columns()[(column - next.area().0.col_num()) as usize]
                                .name()
                                .into(),
                        ));
                    }
                }
            } else if rows != 0
                && table
                    .area()
                    .0
                    .row_num()
                    .checked_add(1)
                    .zip(
                        table
                            .area()
                            .1
                            .row_num()
                            .checked_sub(table.totals_row_count()),
                    )
                    .is_some_and(|(first, last)| {
                        first <= last
                            && row <= first
                            && u64::from(row) + u64::from(rows) > u64::from(last)
                    })
            {
                plan.cleared_columns
                    .push((table.area().0.col_num(), table.area().1.col_num()));
                retained_tables.push(plan.tables.len());
                plan.retained_row = Some(table.area().0.row_num() + 1);
            }
            plan.tables.push(next);
        }
        for pivot in self.pivot_tables() {
            let mut location = pivot.pivot_table_definition().location().clone();
            if remove && location.remove_geometry(col, cols, row, rows)? {
                plan.removed_pivot_names
                    .push(pivot.pivot_table_definition().name().into());
                plan.pivot_locations.push(None);
                continue;
            }
            if !remove {
                location.insert_geometry(col, cols, row, rows)?;
            }
            plan.pivot_locations.push(Some(location));
        }
        // A contiguous deletion can retain only its first table data row.
        if let Some(blank) = plan.retained_row {
            for (table_index, table) in plan.tables.iter_mut().enumerate() {
                if !retained_tables.contains(&table_index) || table.area().0.row_num() + 1 != blank
                {
                    if table.area().0.row_num() < blank && blank <= table.area().1.row_num() {
                        return Err("retaining a table data row would split another table".into());
                    }
                    table.insert_geometry(0, 0, blank, 1)?;
                }
            }
            for location in plan.pivot_locations.iter_mut().flatten() {
                location.insert_geometry(0, 0, blank, 1)?;
            }
        }
        Ok(plan)
    }
}

impl ModelGeometryPlan {
    pub(crate) fn adjust_reference_edits(
        &self,
        edits: &mut [crate::drawing::charts::ChartReferenceEdit],
    ) {
        for edit in edits {
            if let crate::drawing::charts::ChartReferenceEdit::DeleteRows { rows, .. } = edit {
                rows.retain(|row| self.retained_row != Some(*row));
            }
        }
    }

    pub(crate) fn tables(&self) -> &[Table] {
        &self.tables
    }

    pub(crate) fn apply_table_reference_removal(&mut self, book: &mut Workbook) {
        if let Some(removal) = self.reference_removal.take() {
            removal.apply(book);
        }
    }

    pub(crate) fn apply_frozen_charts(&mut self, book: &mut Workbook) {
        book.apply_frozen_pivot_charts(
            std::mem::take(&mut self.frozen_charts),
            Some(&self.sheet_id),
        );
    }

    pub(crate) fn adjust_sheet_references(
        &self,
        sheet: &mut Worksheet,
        host: &str,
        coordinates: (u32, u32, u32, u32),
        remove: bool,
    ) {
        use crate::drawing::charts::ChartReferenceEdit;
        let (root_col, col_count, root_row, row_count) = coordinates;
        let Some(mut edits) = crate::drawing::charts::coordinate_edits(
            host, root_col, col_count, root_row, row_count, remove,
        ) else {
            return;
        };
        self.adjust_reference_edits(&mut edits);
        for edit in edits {
            match edit {
                ChartReferenceEdit::DeleteRows { rows, .. } => {
                    // The retained first data rows are cleared, not deleted.
                    // Dispatch the remaining contiguous runs in native order.
                    let mut index = 0;
                    while index < rows.len() {
                        let first = index;
                        while index + 1 < rows.len() && rows[index + 1] == rows[index] + 1 {
                            index += 1;
                        }
                        sheet.adjustment_remove_coordinate_with_sheet(
                            host,
                            0,
                            0,
                            rows[first]
                                - u32::try_from(first)
                                    .expect("deleted rows fit the worksheet grid"),
                            u32::try_from(index - first + 1)
                                .expect("deleted rows fit the worksheet grid"),
                        );
                        index += 1;
                    }
                }
                ChartReferenceEdit::DeleteColumns { .. } => {
                    sheet.adjustment_remove_coordinate_with_sheet(host, root_col, col_count, 0, 0);
                }
                ChartReferenceEdit::InsertRows { .. } => {
                    sheet.adjustment_insert_coordinate_with_sheet(host, 0, 0, root_row, row_count);
                }
                ChartReferenceEdit::InsertColumns { .. } => {
                    sheet.adjustment_insert_coordinate_with_sheet(host, root_col, col_count, 0, 0);
                }
                _ => unreachable!("coordinate_edits returns only row/column changes"),
            }
        }
    }

    pub(crate) fn adjust_geometry(
        &mut self,
        sheet: &mut Worksheet,
        coordinates: (u32, u32, u32, u32),
        remove: bool,
    ) -> crate::drawing::spreadsheet::DrawingRemovals {
        let (col, cols, row, rows) = coordinates;
        let mut removed = crate::drawing::spreadsheet::DrawingRemovals::default();
        if remove {
            if let Some(retained) = self.retained_row {
                // Excel clears the table data, retaining the sheet row and
                // its unrelated cells, styles and height.
                for &(left, right) in &self.cleared_columns {
                    for column in left..=right {
                        if sheet.cell((column, retained)).is_some() {
                            sheet.cell_mut((column, retained)).set_blank();
                        }
                    }
                }
                removed = sheet.adjustment_remove_geometry(col, cols, row + 1, rows - 1);
            } else {
                removed = sheet.adjustment_remove_geometry(col, cols, row, rows);
            }
        } else {
            sheet.adjustment_insert_geometry(col, cols, row, rows);
        }
        *sheet.tables_mut() = std::mem::take(&mut self.tables);
        let pivots = std::mem::take(sheet.pivot_tables_mut());
        for (mut pivot, location) in pivots.into_iter().zip(&mut self.pivot_locations) {
            if let Some(location) = location.take() {
                pivot.pivot_table_definition_mut().set_location(location);
                sheet.pivot_tables_mut().push(pivot);
            } else {
                self.removed_pivots.push(pivot);
            }
        }
        for (col, row, name) in std::mem::take(&mut self.headers) {
            sheet.cell_mut((col, row)).set_value(name);
        }
        removed
    }
}

impl Workbook {
    pub(crate) fn prepare_workbook_model_geometry(
        &mut self,
        sheet: &str,
        col: u32,
        cols: u32,
        row: u32,
        rows: u32,
        remove: bool,
    ) -> Option<ModelGeometryPlan> {
        let mut plan = self
            .sheet_by_name_mut(sheet)
            .ok()?
            .prepare_model_geometry(col, cols, row, rows, remove)
            .ok()?;
        if (!plan.removed_tables.is_empty() || !plan.removed_pivot_names.is_empty())
            && self.prepare_slicer_reconciliation().is_err()
        {
            return None;
        }
        plan.reference_removal = Some(
            super::table_reference_removal::TableReferenceRemoval::prepare(
                self,
                sheet,
                plan.tables(),
            )
            .ok()?,
        );
        if !plan.removed_pivot_names.is_empty() {
            self.try_read_sheet_collection().ok()?;
            let hosts = (0..self.work_sheet_collection.len()).collect::<Vec<_>>();
            plan.frozen_charts = self
                .prepare_frozen_pivot_charts(&hosts, sheet, |space| {
                    space.pivot_source().is_some_and(|source| {
                        plan.removed_pivot_names
                            .iter()
                            .any(|name| name == source.table_name())
                    })
                })
                .ok()?;
        }
        Some(plan)
    }

    pub(crate) fn finish_workbook_model_geometry(&mut self, plan: &ModelGeometryPlan) {
        self.reconcile_consumers_after_model_removal(
            &plan.sheet_id,
            &plan.removed_tables,
            &plan.removed_pivots,
        );
    }
}
