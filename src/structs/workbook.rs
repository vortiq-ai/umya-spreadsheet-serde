use std::sync::{
    Arc,
    RwLock,
};

#[path = "workbook_reference_hosts.rs"]
mod reference_hosts;

use crate::{
    StringValue,
    XlsxError,
    helper::{
        address::{
            split_address,
            unescape_sheet_name,
        },
        coordinate::column_index_from_string,
    },
    reader::xlsx::{
        LazyChartPartProvider,
        SourceCacheSession,
        capture_chart_source_caches,
        raw_to_deserialize_by_worksheet,
    },
    structs::{
        Address,
        CellValue,
        Cells,
        DefinedName,
        Properties,
        SharedStringTable,
        Stylesheet,
        WorkbookProtection,
        WorkbookView,
        Worksheet,
        drawing::{
            Theme,
            charts::ChartSourceCacheReport,
        },
    },
    traits::AdjustmentCoordinateWithSheet,
};

/// A Workbook Object.
/// The starting point of all struct.
#[derive(Clone, Default, Debug)]
pub struct Workbook {
    properties:               Properties,
    work_sheet_collection:    Vec<Worksheet>,
    chart_cache_report:       ChartSourceCacheReport,
    macros_code:              Option<Vec<u8>>,
    jsa_macros_code:          Option<Vec<u8>>,
    code_name:                StringValue,
    ribbon_xml_data:          StringValue,
    theme:                    Theme,
    stylesheet:               Stylesheet,
    shared_string_table:      Arc<RwLock<SharedStringTable>>,
    workbook_view:            WorkbookView,
    backup_context_types:     Vec<(Box<str>, Box<str>)>,
    pivot_caches:             Vec<(Box<str>, Box<str>, Box<str>)>,
    workbook_protection:      Option<Box<WorkbookProtection>>,
    defined_names:            Vec<DefinedName>,
    /// `<calcPr iterate=.. iterateCount=.. iterateDelta=..>` — Excel's
    /// iterative calculation settings. A model with a DELIBERATE circular
    /// reference (an LBO where debt drives interest drives cash flow drives
    /// debt) sets these, and Excel converges it. The writer previously
    /// hardcoded a bare `<calcPr calcId="122211"/>`, so a workbook read
    /// WITH these settings was written back WITHOUT them and Excel then
    /// refused to converge a file it had computed fine before the round
    /// trip.
    iterative_calc:           Option<(u32, f64)>,
    iterate_enabled:          bool,
    /// `<calcPr fullCalcOnLoad="1">` — tells Excel to recompute every formula
    /// when the file opens. Use it when a formula cache is missing, stale, or
    /// produced by a different calculation engine. Cached values still serve
    /// readers that do not recalculate.
    full_calc_on_load:        bool,
    pub(crate) cell_metadata: Option<super::cell_metadata::CellMetadata>,
}

// Precomputed decisions are kept separate from model mutation: an unresolved
// reference must not leave half a workbook adjusted.
struct WorkbookReferencePlan {
    global_names: Vec<Option<String>>,
    sheets:       Vec<SheetReferencePlan>,
}
struct SheetReferencePlan {
    index:  usize,
    names:  Vec<Option<String>>,
    cells:  Vec<((u32, u32), crate::structs::CellFormula)>,
    charts: Vec<Vec<crate::drawing::charts::reference_edit_engine::FormulaEdit>>,
}

#[cfg(test)]
mod stage9_move_geometry_tests {
    use crate::drawing::charts::{
        ReferenceCell,
        ReferenceRect,
    };

    fn cell(column: u32, row: u32) -> ReferenceCell {
        ReferenceCell { column, row }
    }
    fn rect(first: (u32, u32), last: (u32, u32)) -> ReferenceRect {
        ReferenceRect {
            first: cell(first.0, first.1),
            last:  cell(last.0, last.1),
        }
    }

    #[test]
    fn reference_plan_is_read_only_until_applied_and_preserves_cell_formula_metadata() {
        use crate::drawing::charts::ChartReferenceEdit;
        let mut book = crate::new_file();
        book.sheet_mut(0)
            .unwrap()
            .cell_mut((1, 1))
            .set_formula("Sheet1!B2")
            .set_formula_result_number(12);
        book.sheet_mut(0)
            .unwrap()
            .add_defined_name("TestName", "Sheet1!$B$2")
            .unwrap();
        let edit = ChartReferenceEdit::InsertRows {
            sheet: "Sheet1".into(),
            at:    2,
            count: 1,
        };
        let plan = book
            .prepare_workbook_reference_adjustment(&[0], &edit)
            .unwrap();
        assert_eq!(
            book.sheet(0).unwrap().cell((1, 1)).unwrap().formula(),
            "Sheet1!B2"
        );
        assert_eq!(
            book.sheet(0).unwrap().defined_names()[0].address(),
            "Sheet1!$B$2"
        );
        book.apply_workbook_reference_adjustment(plan);
        assert_eq!(
            book.sheet(0).unwrap().cell((1, 1)).unwrap().formula(),
            "Sheet1!B3"
        );
        assert_eq!(
            book.sheet(0).unwrap().defined_names()[0].address(),
            "Sheet1!$B$3"
        );
    }

    #[test]
    fn geometry_moves_cross_sheet_and_clears_old_destination_even_for_empty_source_cells() {
        let mut book = crate::new_file();
        book.new_sheet("Other").unwrap();
        book.sheet_mut(0)
            .unwrap()
            .cell_mut((2, 2))
            .set_value("source");
        book.sheet_mut(1)
            .unwrap()
            .cell_mut((5, 8))
            .set_value("overwritten");
        book.sheet_mut(1)
            .unwrap()
            .cell_mut((5, 9))
            .set_value("clear blank");
        book.relocate_cells_only("Sheet1", &rect((2, 2), (2, 3)), "Other", cell(5, 8))
            .unwrap();
        assert!(book.sheet(0).unwrap().cell((2, 2)).is_none());
        assert_eq!(
            book.sheet(1).unwrap().cell((5, 8)).unwrap().value(),
            "source"
        );
        assert!(book.sheet(1).unwrap().cell((5, 9)).is_none());
    }

    #[test]
    fn geometry_handles_overlapping_left_up_move_and_rejects_invalid_destination_before_changes() {
        let mut book = crate::new_file();
        for (row, value) in [(2, "a"), (3, "b"), (4, "c")] {
            book.sheet_mut(0)
                .unwrap()
                .cell_mut((3, row))
                .set_value(value);
        }
        book.relocate_cells_only("Sheet1", &rect((3, 3), (3, 4)), "Sheet1", cell(3, 2))
            .unwrap();
        assert_eq!(book.sheet(0).unwrap().cell((3, 2)).unwrap().value(), "b");
        assert_eq!(book.sheet(0).unwrap().cell((3, 3)).unwrap().value(), "c");
        assert!(book.sheet(0).unwrap().cell((3, 4)).is_none());
        assert!(
            book.relocate_cells_only("Sheet1", &rect((3, 2), (3, 3)), "Sheet1", cell(0, 1))
                .is_err()
        );
        assert_eq!(book.sheet(0).unwrap().cell((3, 2)).unwrap().value(), "b");
    }
}

impl Workbook {
    /// Preflight all required raw metadata, then capture their ordinary reader
    /// baselines before any semantic or geometry edit is performed.
    pub(crate) fn prepare_reference_hosts(
        &mut self,
        required: &[&str],
        edit: &crate::drawing::charts::ChartReferenceEdit,
    ) -> Result<Vec<usize>, XlsxError> {
        let hosts = reference_hosts::discover(self, required, edit)?;
        reference_hosts::materialize(self, &hosts);
        Ok(hosts)
    }

    /// Preflight every materialized formula owner before any reference or cell
    /// geometry is changed. Hosts must have been discovered and materialized.
    fn prepare_workbook_reference_adjustment(
        &self,
        hosts: &[usize],
        edit: &crate::drawing::charts::ChartReferenceEdit,
    ) -> Result<WorkbookReferencePlan, XlsxError> {
        use crate::drawing::charts::reference_edit_engine::{
            PreparedReferenceEdit,
            edit_prepared_formula_text,
        };
        let prepared = PreparedReferenceEdit::new(edit)
            .map_err(|e| XlsxError::CellError(format!("reference edit: {e:?}")))?;
        let decide = |text: &str, host: &str| -> Option<String> {
            let result = edit_prepared_formula_text(text, host, &prepared, false);
            (result.text != text).then_some(result.text)
        };
        let mut global_names = Vec::with_capacity(self.defined_names.len());
        for name in &self.defined_names {
            let host = if name.has_local_sheet_id() {
                self.work_sheet_collection
                    .get(name.local_sheet_id() as usize)
                    .map_or("", Worksheet::name)
            } else {
                ""
            };
            global_names.push(decide(&name.address(), host));
        }
        let mut sheets = Vec::with_capacity(hosts.len());
        for &index in hosts {
            let sheet = &self.work_sheet_collection[index];
            let host = sheet.name();
            let mut names = Vec::new();
            for name in sheet.defined_names() {
                names.push(decide(&name.address(), host));
            }
            let mut cells = Vec::new();
            for cell in sheet.cells() {
                if cell.is_formula() {
                    let c = cell.coordinate();
                    let moving = match edit {
                        crate::drawing::charts::ChartReferenceEdit::MoveRange {
                            source_sheet,
                            source,
                            ..
                        } => {
                            host.eq_ignore_ascii_case(source_sheet)
                                && (source.first.column..=source.last.column).contains(&c.col_num())
                                && (source.first.row..=source.last.row).contains(&c.row_num())
                        }
                        crate::drawing::charts::ChartReferenceEdit::RenameSheet { .. }
                        | crate::drawing::charts::ChartReferenceEdit::DeleteSheet { .. }
                        | crate::drawing::charts::ChartReferenceEdit::InsertRows { .. }
                        | crate::drawing::charts::ChartReferenceEdit::InsertColumns { .. }
                        | crate::drawing::charts::ChartReferenceEdit::DeleteRows { .. }
                        | crate::drawing::charts::ChartReferenceEdit::DeleteColumns { .. } => false,
                    };
                    if let Some(original) = cell.cell_value().formula_obj() {
                        let mut formula = original.clone();
                        if formula.adjust_reference_texts_prepared(host, &prepared, moving) {
                            cells.push(((c.col_num(), c.row_num()), formula));
                        }
                    }
                }
            }
            let charts = sheet.prepare_chart_space_edits(host, &prepared);
            sheets.push(SheetReferencePlan {
                index,
                names,
                cells,
                charts,
            });
        }
        Ok(WorkbookReferencePlan {
            global_names,
            sheets,
        })
    }

    fn apply_workbook_reference_adjustment(&mut self, plan: WorkbookReferencePlan) {
        for (name, decision) in self.defined_names.iter_mut().zip(plan.global_names) {
            if let Some(text) = decision {
                name.set_string_value(text);
            }
        }
        for sheet_plan in plan.sheets {
            let sheet = &mut self.work_sheet_collection[sheet_plan.index];
            for (name, decision) in sheet.defined_names_mut().iter_mut().zip(sheet_plan.names) {
                if let Some(text) = decision {
                    name.set_string_value(text);
                }
            }
            for (coordinate, formula) in sheet_plan.cells {
                let cell = sheet.cell_mut(coordinate);
                // The preflight clone preserves formula metadata and adjusts
                // both serialized text and any authoritative shared view.
                let preserve_error =
                    matches!(cell.raw_value(), crate::structs::CellRawValue::Error(_));
                cell.cell_value_mut().set_formula_obj(formula);
                if !preserve_error {
                    cell.cell_value_mut().set_formula_result_blank();
                }
            }
            sheet.apply_chart_space_edits(sheet_plan.charts);
        }
    }

    /// Move an inclusive source rectangle to a one-based destination cell.
    /// Reference decisions are preflighted before formula or geometry changes.
    pub fn move_range(
        &mut self,
        source_sheet: &str,
        source: crate::drawing::charts::ReferenceRect,
        destination_sheet: &str,
        destination: crate::drawing::charts::ReferenceCell,
    ) -> Result<(), XlsxError> {
        use crate::drawing::charts::{
            ChartReferenceEdit,
            reference_edit_engine::PreparedReferenceEdit,
        };
        let edit = ChartReferenceEdit::MoveRange {
            source_sheet: source_sheet.into(),
            source,
            destination_sheet: destination_sheet.into(),
            destination,
        };
        PreparedReferenceEdit::new(&edit)
            .map_err(|e| XlsxError::CellError(format!("reference edit: {e:?}")))?;
        self.find_sheet_index_by_name(source_sheet)?;
        self.find_sheet_index_by_name(destination_sheet)?;
        let hosts = self.prepare_reference_hosts(&[source_sheet, destination_sheet], &edit)?;
        let plan = self.prepare_workbook_reference_adjustment(&hosts, &edit)?;
        self.apply_workbook_reference_adjustment(plan);
        self.relocate_cells_only(source_sheet, &source, destination_sheet, destination)
    }

    // Legacy row/column entrypoints have no Result channel. Complete every
    // metadata scan before materialization, and retain their existing reader
    // panic-on-I/O boundary rather than silently dropping dependent hosts.
    fn structural_reference_hosts(
        &mut self,
        sheet: &str,
        edits: &[crate::drawing::charts::ChartReferenceEdit],
    ) -> Vec<usize> {
        // Invalid structural edit coordinates or a missing sheet are outside
        // this legacy void API's accepted domain; leave the workbook unchanged.
        if self.find_sheet_index_by_name(sheet).is_err()
            || edits.iter().any(|edit| {
                crate::drawing::charts::reference_edit_engine::PreparedReferenceEdit::new(edit)
                    .is_err()
            })
        {
            return Vec::new();
        }
        let mut hosts = Vec::new();
        for edit in edits {
            let found = reference_hosts::discover(self, &[sheet], edit).unwrap_or_else(|error| {
                panic!("cannot discover workbook reference hosts: {error}")
            });
            hosts.extend(found);
        }
        hosts.sort_unstable();
        hosts.dedup();
        reference_hosts::materialize(self, &hosts);
        hosts
    }

    fn adjust_global_defined_names(
        &mut self,
        edits: &[crate::drawing::charts::reference_edit_engine::PreparedReferenceEdit<'_>],
    ) {
        use crate::drawing::charts::reference_edit_engine::edit_prepared_formula_text;
        let sheet_names: Vec<_> = self
            .work_sheet_collection
            .iter()
            .map(|sheet| sheet.name().to_owned())
            .collect();
        for prepared in edits {
            for name in &mut self.defined_names {
                let host = if name.has_local_sheet_id() {
                    sheet_names
                        .get(name.local_sheet_id() as usize)
                        .map_or("", String::as_str)
                } else {
                    ""
                };
                let before = name.address();
                let decision = edit_prepared_formula_text(&before, host, prepared, false);
                if decision.text != before {
                    name.set_string_value(decision.text);
                }
            }
        }
    }

    /// Geometry-only building block for the coordinated workbook move. This
    /// does not adjust references; callers must preflight and dispatch the
    /// approved `ChartReferenceEdit` separately before exposing a public move.
    pub(crate) fn relocate_cells_only(
        &mut self,
        source_sheet: &str,
        source: &crate::drawing::charts::ReferenceRect,
        destination_sheet: &str,
        destination: crate::drawing::charts::ReferenceCell,
    ) -> Result<(), XlsxError> {
        let first = &source.first;
        let last = &source.last;
        if first.column == 0
            || first.row == 0
            || first.column > last.column
            || first.row > last.row
            || last.column > 16_384
            || last.row > 1_048_576
            || destination.column == 0
            || destination.row == 0
        {
            return Err(XlsxError::CellError("invalid move coordinates".into()));
        }
        let last_column = destination
            .column
            .checked_add(last.column - first.column)
            .filter(|v| *v <= 16_384)
            .ok_or_else(|| XlsxError::CellError("move exceeds column bounds".into()))?;
        let last_row = destination
            .row
            .checked_add(last.row - first.row)
            .filter(|v| *v <= 1_048_576)
            .ok_or_else(|| XlsxError::CellError("move exceeds row bounds".into()))?;
        let source_index = self.find_sheet_index_by_name(source_sheet)?;
        let destination_index = self.find_sheet_index_by_name(destination_sheet)?;
        let source_cells: Vec<_> = self
            .sheet_mut(source_index)?
            .cells()
            .into_iter()
            .filter(|cell| {
                let coordinate = cell.coordinate();
                (first.column..=last.column).contains(&coordinate.col_num())
                    && (first.row..=last.row).contains(&coordinate.row_num())
            })
            .cloned()
            .collect();
        let source_coordinates: Vec<_> = source_cells
            .iter()
            .map(|cell| (cell.coordinate().col_num(), cell.coordinate().row_num()))
            .collect();
        let target_coordinates: Vec<_> = self
            .sheet_mut(destination_index)?
            .cells()
            .into_iter()
            .filter_map(|cell| {
                let coordinate = cell.coordinate();
                ((destination.column..=last_column).contains(&coordinate.col_num())
                    && (destination.row..=last_row).contains(&coordinate.row_num()))
                .then_some((coordinate.col_num(), coordinate.row_num()))
            })
            .collect();
        for coordinate in source_coordinates {
            self.sheet_mut(source_index)?.remove_cell(coordinate);
        }
        for coordinate in target_coordinates {
            self.sheet_mut(destination_index)?.remove_cell(coordinate);
        }
        for mut cell in source_cells {
            let new_col = destination.column + cell.coordinate().col_num() - first.column;
            let new_row = destination.row + cell.coordinate().row_num() - first.row;
            cell.coordinate_mut()
                .set_col_num(new_col)
                .set_row_num(new_row);
            self.sheet_mut(destination_index)?.set_cell(cell);
        }
        Ok(())
    }

    /// Import outcomes, accumulated as sheets materialize. Eager reads finish
    /// the report before returning; lazy reads finish after all original
    /// sheets materialize. Repeated access never charges a cache again.
    ///
    /// These are historical import costs, not live accounting after chart
    /// edits, copies or stripping. Budget warnings use `over_cap_caches`.
    /// Lazy references into previously materialized sheets conservatively
    /// lose source labels even if those sheets were never edited. See
    /// [`ChartSourceCacheReport`] for the remaining lazy differences.
    #[must_use]
    pub fn chart_source_cache_report(&self) -> &ChartSourceCacheReport {
        &self.chart_cache_report
    }

    pub(crate) fn chart_source_cache_report_mut(&mut self) -> &mut ChartSourceCacheReport {
        &mut self.chart_cache_report
    }

    /// Returns whether a one-based cell `cm` index identifies a native dynamic
    /// array. Unrelated cell metadata must not be treated as dynamic-array
    /// identity.
    #[must_use]
    pub fn is_dynamic_array_metadata(&self, cell_meta_index: u32) -> bool {
        self.cell_metadata
            .as_ref()
            .is_some_and(|metadata| metadata.is_dynamic(cell_meta_index))
    }

    /// Ensure a standard XLDAPR dynamic-array record exists and return its
    /// one-based cell metadata index. Assign this to the array anchor's `cm`.
    /// Existing records and their indices are retained.
    pub fn ensure_dynamic_array_metadata(&mut self) -> u32 {
        self.cell_metadata
            .get_or_insert_with(Default::default)
            .ensure_dynamic()
    }

    /// Ensure a namespaced cell-metadata extension exists without replacing
    /// existing types or records. Properties must be a single XML element.
    pub fn ensure_cell_metadata_extension(
        &mut self,
        type_name: &str,
        extension_uri: &str,
        properties_xml: &str,
    ) -> Result<u32, XlsxError> {
        self.cell_metadata
            .get_or_insert_with(Default::default)
            .ensure_extension(type_name, extension_uri, properties_xml)
    }

    /// Resolve a cell cm index to a property of a named metadata extension.
    #[must_use]
    pub fn is_cell_metadata_extension(
        &self,
        cm: u32,
        type_name: &str,
        namespace: &str,
        local_name: &str,
    ) -> bool {
        self.cell_metadata
            .as_ref()
            .is_some_and(|metadata| metadata.is_extension(cm, type_name, namespace, local_name))
    }

    // ************************
    // update Coordinate
    // ************************
    /// Insert new rows.
    /// # Arguments
    /// * `sheet_name` - Specify the sheet name. ex) "Sheet1"
    /// * `row_index` - Specify point of insert. ex) 1
    /// * `num_rows` - Specify number to insert. ex) 2
    /// # Examples
    /// ```
    /// let mut book = umya_spreadsheet::new_file();
    /// book.insert_new_row("Sheet1", 2, 3);
    /// ```
    #[inline]
    pub fn insert_new_row(&mut self, sheet_name: &str, row_index: u32, num_rows: u32) {
        self.adjustment_insert_coordinate_with_sheet(sheet_name, 0, 0, row_index, num_rows);
    }

    /// Insert new columns.
    /// # Arguments
    /// * `sheet_name` - Specify the sheet name. ex) "Sheet1"
    /// * `column` - Specify point of insert. ex) "B"
    /// * `num_columns` - Specify number to insert. ex) 3
    /// # Examples
    /// ```
    /// let mut book = umya_spreadsheet::new_file();
    /// book.insert_new_column("Sheet1", "B", 3);
    /// ```
    #[inline]
    pub fn insert_new_column(&mut self, sheet_name: &str, column: &str, num_columns: u32) {
        self.insert_new_column_by_index(sheet_name, column_index_from_string(column), num_columns);
    }

    /// Insert new columns.
    /// # Arguments
    /// * `sheet_name` - Specify the sheet name. ex) "Sheet1"
    /// * `column_index` - Specify point of insert. ex) 2
    /// * `num_columns` - Specify number to insert. ex) 3
    /// # Examples
    /// ```rust
    /// let mut book = umya_spreadsheet::new_file();
    /// book.insert_new_column_by_index("Sheet1", 2, 3);
    /// ```
    #[inline]
    pub fn insert_new_column_by_index(
        &mut self,
        sheet_name: &str,
        column_index: u32,
        num_columns: u32,
    ) {
        self.adjustment_insert_coordinate_with_sheet(sheet_name, column_index, num_columns, 0, 0);
    }

    /// Remove rows.
    /// # Arguments
    /// * `sheet_name` - Specify the sheet name. ex) "Sheet1"
    /// * `row_index` - Specify point of remove. ex) 1
    /// * `num_rows` - Specify number to remove. ex) &2
    /// # Examples
    /// ```rust
    /// let mut book = umya_spreadsheet::new_file();
    /// book.remove_row("Sheet1", 2, 3);
    /// ```
    #[inline]
    pub fn remove_row(&mut self, sheet_name: &str, row_index: u32, num_rows: u32) {
        self.adjustment_remove_coordinate_with_sheet(sheet_name, 0, 0, row_index, num_rows);
    }

    /// Remove columns.
    /// # Arguments
    /// * `sheet_name` - Specify the sheet name. ex) "Sheet1"
    /// * `column` - Specify point of remove. ex) "B"
    /// * `num_columns` - Specify number to remove. ex) 3
    /// # Examples
    /// ```rust
    /// let mut book = umya_spreadsheet::new_file();
    /// book.remove_column("Sheet1", "B", 3);
    /// ```
    #[inline]
    pub fn remove_column(&mut self, sheet_name: &str, column: &str, num_columns: u32) {
        self.remove_column_by_index(sheet_name, column_index_from_string(column), num_columns);
    }

    /// Remove columns.
    /// # Arguments
    /// * `sheet_name` - Specify the sheet name. ex) "Sheet1"
    /// * `column_index` - Specify point of remove. ex) 2
    /// * `num_columns` - Specify number to remove. ex) 3
    /// # Examples
    /// ```rust
    /// let mut book = umya_spreadsheet::new_file();
    /// book.remove_column_by_index("Sheet1", 2, 3);
    /// ```
    #[inline]
    pub fn remove_column_by_index(
        &mut self,
        sheet_name: &str,
        column_index: u32,
        num_columns: u32,
    ) {
        self.adjustment_remove_coordinate_with_sheet(sheet_name, column_index, num_columns, 0, 0);
    }

    /// Gets the cell value by specifying an address.
    /// # Arguments
    /// * `address` - address. ex) "Sheet1!A1:C5"
    /// # Return value
    /// *`Vec<&CellValue>` - `CellValue` List.
    /// # Examples
    /// ```rust
    /// let mut book = umya_spreadsheet::new_file();
    /// let mut cell_value_List = book.cell_value_by_address("Sheet1!A1:C5");
    /// ```
    #[inline]
    #[must_use]
    pub fn cell_value_by_address(&self, address: &str) -> Vec<&CellValue> {
        let (sheet_name, range) = split_address(address);
        match self.sheet_by_name(&unescape_sheet_name(sheet_name)) {
            Ok(sheet) => sheet.cell_value_by_range(range),
            Err(_) => vec![],
        }
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use cell_value_by_address()")]
    pub fn get_cell_value_by_address(&self, address: &str) -> Vec<&CellValue> {
        self.cell_value_by_address(address)
    }

    /// (This method is crate only.)
    /// Gets the cell value by specifying an Address Object.
    /// # Arguments
    /// * `address` - Address Object
    /// # Return value
    /// *`Vec<&CellValue>` - `CellValue` List.
    #[inline]
    pub(crate) fn cell_value_by_address_crate(&self, address: &Address) -> Vec<&CellValue> {
        // A chart address holds its sheet name in formula-text form.
        match self.sheet_by_name(&unescape_sheet_name(address.sheet_name())) {
            Ok(sheet) => sheet.cell_value_by_range(&address.range().range()),
            Err(_) => vec![],
        }
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use cell_value_by_address_crate()")]
    pub(crate) fn get_cell_value_by_address_crate(&self, address: &Address) -> Vec<&CellValue> {
        self.cell_value_by_address_crate(address)
    }

    /// Get Theme.
    #[inline]
    #[must_use]
    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use theme()")]
    pub fn get_theme(&self) -> &Theme {
        self.theme()
    }

    /// Get Theme in mutable.
    #[inline]
    pub fn theme_mut(&mut self) -> &mut Theme {
        &mut self.theme
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use theme_mut()")]
    pub fn get_theme_mut(&mut self) -> &mut Theme {
        self.theme_mut()
    }

    /// Set Theme.
    /// # Arguments
    /// * `value` - Theme
    #[inline]
    pub fn set_theme(&mut self, value: Theme) -> &mut Self {
        self.theme = value;
        self
    }

    /// Get Properties.
    #[inline]
    #[must_use]
    pub fn properties(&self) -> &Properties {
        &self.properties
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use properties()")]
    pub fn get_properties(&self) -> &Properties {
        self.properties()
    }

    /// Get Properties in mutable.
    #[inline]
    pub fn properties_mut(&mut self) -> &mut Properties {
        &mut self.properties
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use properties_mut()")]
    pub fn get_properties_mut(&mut self) -> &mut Properties {
        self.properties_mut()
    }

    /// Set Properties.
    /// # Arguments
    /// * `value` - Properties
    #[inline]
    pub fn set_properties(&mut self, value: Properties) -> &mut Self {
        self.properties = value;
        self
    }

    /// Get Macros Code.
    /// # Return value
    /// * `Option<&Vec<u8>>` - Macros Code Raw Data.
    #[inline]
    #[must_use]
    pub fn macros_code(&self) -> Option<&[u8]> {
        self.macros_code.as_deref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use macros_code()")]
    pub fn get_macros_code(&self) -> Option<&[u8]> {
        self.macros_code()
    }

    /// Set Macros Code.
    /// # Arguments
    /// * `value` - Macros Code Raw Data.
    #[inline]
    pub fn set_macros_code(&mut self, value: impl Into<Vec<u8>>) -> &mut Self {
        self.macros_code = Some(value.into());
        self
    }

    /// Remove Macros Code
    #[inline]
    pub fn remove_macros_code(&mut self) -> &mut Self {
        self.macros_code = None;
        self
    }

    /// Has Macros Code
    #[inline]
    #[must_use]
    pub fn has_macros(&self) -> bool {
        self.macros_code.is_some()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use has_macros()")]
    pub fn get_has_macros(&self) -> bool {
        self.has_macros()
    }

    /// Get Macros Code.
    /// # Return value
    /// * `Option<&Vec<u8>>` - Macros Code Raw Data.
    #[inline]
    #[must_use]
    pub fn jsa_macros_code(&self) -> Option<&[u8]> {
        self.jsa_macros_code.as_deref()
    }

    /// Get Macros Code.
    /// # Return value
    /// * `Option<&Vec<u8>>` - Macros Code Raw Data.
    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use jsa_macros_code()")]
    pub fn get_jsa_macros_code(&self) -> Option<&[u8]> {
        self.jsa_macros_code()
    }

    /// Set Macros Code.
    /// # Arguments
    /// * `value` - Macros Code Raw Data.
    #[inline]
    pub fn set_jsa_macros_code(&mut self, value: impl Into<Vec<u8>>) -> &mut Self {
        self.jsa_macros_code = Some(value.into());
        self
    }

    /// Remove Macros Code
    #[inline]
    pub fn remove_jsa_macros_code(&mut self) -> &mut Self {
        self.jsa_macros_code = None;
        self
    }

    /// Has Macros Code
    #[inline]
    #[must_use]
    pub fn has_jsa_macros(&self) -> bool {
        self.jsa_macros_code.is_some()
    }

    /// Has Macros Code
    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use has_jsa_macros()")]
    pub fn get_has_jsa_macros(&self) -> bool {
        self.has_jsa_macros()
    }

    /// Set codeName property of workbook
    ///
    /// May be useful when importing VBA/macros code from another workbook
    /// and only used when writing book with macros code
    ///
    /// Default one is `ThisWorkbook`.
    ///
    /// Excel often uses `Workbook________` (8 underscores).
    #[inline]
    pub fn set_code_name<S: Into<String>>(&mut self, codename: S) -> &mut Self {
        self.code_name.set_value(codename);
        self
    }

    /// Get codeName property of workbook
    ///
    /// Must to be the same in workbook with VBA/macros code from this workbook
    /// for that code in Workbook object to work out of the box without
    /// adjustments
    #[inline]
    #[must_use]
    pub fn code_name(&self) -> Option<&str> {
        self.code_name.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use code_name()")]
    pub fn get_code_name(&self) -> Option<&str> {
        self.code_name()
    }

    /// (This method is crate only.)
    /// Get Stylesheet.
    #[inline]
    pub(crate) fn stylesheet(&self) -> &Stylesheet {
        &self.stylesheet
    }

    /// (This method is crate only.)
    /// Get Stylesheet.
    #[inline]
    pub(crate) fn stylesheet_mut(&mut self) -> &mut Stylesheet {
        &mut self.stylesheet
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use stylesheet()")]
    pub(crate) fn get_stylesheet(&self) -> &Stylesheet {
        self.stylesheet()
    }

    /// (This method is crate only.)
    /// Set Stylesheet.
    /// # Arguments
    /// * `value` - Stylesheet
    #[inline]
    pub(crate) fn set_stylesheet(&mut self, value: Stylesheet) -> &mut Self {
        self.stylesheet = value;
        self
    }

    /// (This method is crate only.)
    /// Set Default Value Stylesheet.
    #[inline]
    pub(crate) fn set_stylesheet_default_value(&mut self) -> &mut Self {
        self.stylesheet.set_defalut_value();
        self
    }

    /// (This method is crate only.)
    /// Get Shared String Table.
    #[inline]
    pub(crate) fn shared_string_table(&self) -> Arc<RwLock<SharedStringTable>> {
        self.shared_string_table.clone()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use shared_string_table()")]
    pub(crate) fn get_shared_string_table(&self) -> Arc<RwLock<SharedStringTable>> {
        self.shared_string_table()
    }

    /// (This method is crate only.)
    /// Set Shared String Table.
    /// # Arguments
    /// * `value` - Shared String Table
    #[inline]
    pub(crate) fn set_shared_string_table(&mut self, value: SharedStringTable) -> &mut Self {
        self.shared_string_table = Arc::new(RwLock::new(value));
        self
    }

    /// Get Work Sheet List.
    #[must_use]
    pub fn sheet_collection(&self) -> &[Worksheet] {
        for worksheet in &self.work_sheet_collection {
            assert!(
                worksheet.is_deserialized(),
                "This Worksheet is Not Deserialized. Please exec to read_sheet(&mut self, index: \
                 usize);"
            );
        }
        &self.work_sheet_collection
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use sheet_collection()")]
    pub fn get_sheet_collection(&self) -> &[Worksheet] {
        self.sheet_collection()
    }

    /// Get Work Sheet List.
    /// No check deserialized.
    #[inline]
    #[must_use]
    pub fn sheet_collection_no_check(&self) -> &[Worksheet] {
        &self.work_sheet_collection
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use sheet_collection_no_check()")]
    pub fn get_sheet_collection_no_check(&self) -> &[Worksheet] {
        self.sheet_collection_no_check()
    }

    /// Worksheets without materializing them.
    pub(crate) fn sheet_collection_unchecked_mut(&mut self) -> &mut [Worksheet] {
        &mut self.work_sheet_collection
    }

    /// Get Work Sheet List in mutable.
    #[inline]
    pub fn sheet_collection_mut(&mut self) -> &mut [Worksheet] {
        self.read_sheet_collection();
        &mut self.work_sheet_collection
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use sheet_collection_mut()")]
    pub fn get_sheet_collection_mut(&mut self) -> &mut [Worksheet] {
        self.sheet_collection_mut()
    }

    /// Get Work Sheet Count.
    /// # Return value
    /// * `usize` - Work Sheet Count.
    #[inline]
    #[must_use]
    pub fn sheet_count(&self) -> usize {
        self.work_sheet_collection.len()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use sheet_count()")]
    pub fn get_sheet_count(&self) -> usize {
        self.sheet_count()
    }

    /// deserialize by all worksheet.
    #[inline]
    pub fn read_sheet_collection(&mut self) -> &mut Self {
        if self
            .work_sheet_collection
            .iter()
            .all(Worksheet::is_deserialized)
        {
            return self;
        }
        let shared_string_table = self.shared_string_table();
        let stylesheet = self.stylesheet().clone();
        let mut session = SourceCacheSession::deferred();
        let mut provider = LazyChartPartProvider::new(std::collections::HashMap::default());
        let mut deserialized = Vec::new();
        for (index, worksheet) in self.work_sheet_collection.iter_mut().enumerate() {
            if !worksheet.is_deserialized() {
                raw_to_deserialize_by_worksheet(
                    worksheet,
                    &shared_string_table,
                    &stylesheet,
                    &mut session,
                    &mut provider,
                );
                deserialized.push(index);
            }
        }
        capture_chart_source_caches(self, &deserialized, &mut session, &mut provider);
        self
    }

    /// deserialize a worksheet.
    #[inline]
    pub fn read_sheet(&mut self, index: usize) -> &mut Self {
        let is_deserialized = self
            .work_sheet_collection
            .get(index)
            .unwrap()
            .is_deserialized();
        if is_deserialized {
            return self;
        }
        let shared_string_table = self.shared_string_table();
        let stylesheet = self.stylesheet().clone();
        let mut session = SourceCacheSession::deferred();
        let mut provider = LazyChartPartProvider::new(std::collections::HashMap::default());
        let worksheet = self.work_sheet_collection.get_mut(index).unwrap();
        raw_to_deserialize_by_worksheet(
            worksheet,
            &shared_string_table,
            &stylesheet,
            &mut session,
            &mut provider,
        );
        capture_chart_source_caches(self, &[index], &mut session, &mut provider);
        self
    }

    /// deserialize a worksheet.
    #[inline]
    pub fn read_sheet_by_name(&mut self, sheet_name: &str) -> &mut Self {
        let index = self.find_sheet_index_by_name(sheet_name).unwrap();
        self.read_sheet(index)
    }

    #[inline]
    pub(crate) fn find_sheet_index_by_name(&self, sheet_name: &str) -> Result<usize, XlsxError> {
        self.work_sheet_collection
            .iter()
            .position(|sheet| sheet.name() == sheet_name)
            .ok_or(XlsxError::NotFound())
    }

    /// Get Work Sheet.
    /// # Arguments
    /// * `index` - sheet index
    /// # Return value
    /// * `Result<&Worksheet, XlsxError>`.
    #[inline]
    pub fn sheet(&self, index: usize) -> Result<&Worksheet, XlsxError> {
        match self.work_sheet_collection.get(index) {
            Some(v) => {
                if v.is_deserialized() {
                    Ok(v)
                } else {
                    Err(XlsxError::NotDeserialized())
                }
            }
            None => Err(XlsxError::NotFound()),
        }
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use sheet()")]
    pub fn get_sheet(&self, index: &usize) -> Result<&Worksheet, XlsxError> {
        self.sheet(*index)
    }

    /// Get Work Sheet.
    /// # Arguments
    /// * `sheet_name` - sheet name
    /// # Return value
    /// * `Result<&Worksheet, XlsxError>`.
    #[inline]
    pub fn sheet_by_name(&self, sheet_name: &str) -> Result<&Worksheet, XlsxError> {
        self.find_sheet_index_by_name(sheet_name)
            .and_then(|index| self.sheet(index))
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use sheet_by_name()")]
    pub fn get_sheet_by_name(&self, sheet_name: &str) -> Result<&Worksheet, XlsxError> {
        self.sheet_by_name(sheet_name)
    }

    pub fn lazy_read_sheet_cells(&self, index: usize) -> Result<Cells, XlsxError> {
        let shared_string_table = self.shared_string_table();
        self.work_sheet_collection
            .get(index)
            .map(|v| v.cells_stream(&shared_string_table.read().unwrap(), self.stylesheet()))
            .ok_or(XlsxError::NotFound())
    }

    #[deprecated(since = "3.0.0", note = "Use lazy_read_sheet_cells()")]
    pub fn get_lazy_read_sheet_cells(&self, index: &usize) -> Result<Cells, XlsxError> {
        self.lazy_read_sheet_cells(*index)
    }

    /// Get Work Sheet in mutable.
    /// # Arguments
    /// * `index` - sheet index
    /// # Return value
    /// * `Result<&mut Worksheet, XlsxError>`.
    pub fn sheet_mut(&mut self, index: usize) -> Result<&mut Worksheet, XlsxError> {
        let is_deserialized = self
            .work_sheet_collection
            .get(index)
            .map(Worksheet::is_deserialized)
            .ok_or(XlsxError::NotFound())?;
        if is_deserialized {
            return Ok(self.work_sheet_collection.get_mut(index).unwrap());
        }
        let shared_string_table = self.shared_string_table();
        let stylesheet = self.stylesheet().clone();
        let mut session = SourceCacheSession::deferred();
        let mut provider = LazyChartPartProvider::new(std::collections::HashMap::default());
        let worksheet = self.work_sheet_collection.get_mut(index).unwrap();
        raw_to_deserialize_by_worksheet(
            worksheet,
            &shared_string_table,
            &stylesheet,
            &mut session,
            &mut provider,
        );
        capture_chart_source_caches(self, &[index], &mut session, &mut provider);
        Ok(self.work_sheet_collection.get_mut(index).unwrap())
    }

    #[deprecated(since = "3.0.0", note = "Use sheet_mut()")]
    pub fn get_sheet_mut(&mut self, index: &usize) -> Result<&mut Worksheet, XlsxError> {
        self.sheet_mut(*index)
    }

    /// Get Work Sheet in mutable.
    /// # Arguments
    /// * `sheet_name` - sheet name
    /// # Return value
    /// * `Result<&mut Worksheet, XlsxError>`.
    #[inline]
    pub fn sheet_by_name_mut(&mut self, sheet_name: &str) -> Result<&mut Worksheet, XlsxError> {
        self.find_sheet_index_by_name(sheet_name)
            .and_then(move |index| self.sheet_mut(index))
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use sheet_by_name_mut()")]
    pub fn get_sheet_by_name_mut(&mut self, sheet_name: &str) -> Result<&mut Worksheet, XlsxError> {
        self.sheet_by_name_mut(sheet_name)
    }

    #[inline]
    pub fn set_active_sheet(&mut self, index: u32) -> &mut Self {
        self.workbook_view_mut().set_active_tab(index);
        self
    }

    /// Get Active Work Sheet.
    /// # Return value
    /// * `&Worksheet` - Work sheet.
    #[inline]
    #[must_use]
    pub fn active_sheet(&self) -> &Worksheet {
        let index = self.workbook_view().active_tab();
        self.sheet(index as usize).unwrap()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use active_sheet()")]
    pub fn get_active_sheet(&self) -> &Worksheet {
        self.active_sheet()
    }

    /// Get Active Work Sheet in mutable.
    /// # Return value
    /// * `&mut Worksheet` - Work sheet.
    #[inline]
    pub fn active_sheet_mut(&mut self) -> &mut Worksheet {
        let index = self.workbook_view().active_tab();
        self.sheet_mut(index as usize).unwrap()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use active_sheet_mut()")]
    pub fn get_active_sheet_mut(&mut self) -> &mut Worksheet {
        self.active_sheet_mut()
    }

    /// Add Work Sheet.
    /// # Arguments
    /// * `value` - Work Sheet
    /// # Return value
    /// * `Result<&mut Worksheet, XlsxError>` - OK:added work sheet. Err:Error.
    #[inline]
    pub fn add_sheet(&mut self, value: Worksheet) -> Result<&mut Worksheet, XlsxError> {
        let title = value.name();
        Workbook::check_sheet_name(self, title)?;
        self.work_sheet_collection.push(value);
        Ok(self.work_sheet_collection.last_mut().unwrap())
    }

    /// Remove and return all worksheets, leaving the collection empty.
    /// Used by the streaming writer to take ownership of pre-built sheets while
    /// keeping the workbook-level configuration.
    #[inline]
    pub(crate) fn take_all_sheets(&mut self) -> Vec<Worksheet> {
        std::mem::take(&mut self.work_sheet_collection)
    }

    /// Remove Work Sheet.
    /// # Arguments
    /// * `index` - sheet index
    /// # Return value
    /// * `Result<(), XlsxError>` - OK:removed worksheet. Err:Error.
    #[inline]
    pub fn remove_sheet(&mut self, index: usize) -> Result<(), XlsxError> {
        let name = self
            .work_sheet_collection
            .get(index)
            .ok_or(XlsxError::NotFound())?
            .name()
            .to_owned();
        let sheet_order = self
            .work_sheet_collection
            .iter()
            .map(|sheet| sheet.name().to_owned())
            .collect();
        let edit = crate::drawing::charts::ChartReferenceEdit::DeleteSheet {
            sheet: name.clone(),
            sheet_order,
        };
        let hosts = self.prepare_reference_hosts(&[&name], &edit)?;
        let plan = self.prepare_workbook_reference_adjustment(&hosts, &edit)?;
        self.apply_workbook_reference_adjustment(plan);
        self.work_sheet_collection.remove(index);
        // localSheetId is a zero-based workbook index, not a stable sheet ID.
        // Deleted scopes disappear; later scopes shift with their owners.
        self.defined_names
            .retain(|name| !name.has_local_sheet_id() || name.local_sheet_id() as usize != index);
        for name in &mut self.defined_names {
            if name.has_local_sheet_id() && name.local_sheet_id() as usize > index {
                name.set_local_sheet_id(name.local_sheet_id() - 1);
            }
        }
        for sheet in &mut self.work_sheet_collection {
            sheet.defined_names_mut().retain(|name| {
                !name.has_local_sheet_id() || name.local_sheet_id() as usize != index
            });
            for name in sheet.defined_names_mut() {
                if name.has_local_sheet_id() && name.local_sheet_id() as usize > index {
                    name.set_local_sheet_id(name.local_sheet_id() - 1);
                }
            }
        }
        Ok(())
    }

    /// Remove Work Sheet.
    /// # Arguments
    /// * `sheet_name` - sheet name
    /// # Return value
    /// * `Result<(), XlsxError>` - OK:removed worksheet. Err:Error.
    pub fn remove_sheet_by_name(&mut self, sheet_name: &str) -> Result<(), XlsxError> {
        let index = self.find_sheet_index_by_name(sheet_name)?;
        self.remove_sheet(index)
    }

    /// Add New Work Sheet.
    /// # Arguments
    /// * `sheet_title` - sheet title
    /// # Return value
    /// * `Result<&mut Worksheet, XlsxError>` - OK:added work sheet. Err:Error.
    #[inline]
    pub fn new_sheet<S: Into<String>>(
        &mut self,
        sheet_title: S,
    ) -> Result<&mut Worksheet, XlsxError> {
        let v = sheet_title.into();
        Workbook::check_sheet_name(self, &v)?;
        let sheet_id = (self.work_sheet_collection.len() + 1).to_string();
        Ok(Workbook::add_new_sheet_crate(self, sheet_id, v.clone()))
    }

    /// (This method is crate only.)
    /// Add New Work Sheet.
    /// # Arguments
    /// * `index` - sheet index
    /// * `sheet_title` - sheet title
    /// # Return value
    /// * `&mut Worksheet` - added work sheet.
    pub(crate) fn add_new_sheet_crate<S: Into<String>>(
        &mut self,
        sheet_id: S,
        sheet_title: S,
    ) -> &mut Worksheet {
        let mut worksheet = Worksheet::default();
        worksheet.set_sheet_id(sheet_id);
        worksheet.set_name(sheet_title.into());
        worksheet.sheet_format_properties_mut().set_defalut_value();
        self.work_sheet_collection.push(worksheet);
        self.work_sheet_collection.last_mut().unwrap()
    }

    /// Set Sheet Name.
    /// # Arguments
    /// * `index` - target sheet index
    /// * `sheet_name` - sheet name
    /// # Return value
    /// * `Result<(), XlsxError>` - OK:Success  Err:Error.
    pub fn set_sheet_name<S: Into<String>>(
        &mut self,
        index: usize,
        sheet_name: S,
    ) -> Result<(), XlsxError> {
        let sheet_name_str = sheet_name.into();
        Workbook::check_sheet_name(self, sheet_name_str.as_ref())?;
        let old = self
            .work_sheet_collection
            .get(index)
            .ok_or(XlsxError::NotFound())?
            .name()
            .to_owned();
        let edit = crate::drawing::charts::ChartReferenceEdit::RenameSheet {
            old: old.clone(),
            new: sheet_name_str.clone(),
        };
        let hosts = self.prepare_reference_hosts(&[&old], &edit)?;
        let plan = self.prepare_workbook_reference_adjustment(&hosts, &edit)?;
        self.apply_workbook_reference_adjustment(plan);
        self.work_sheet_collection[index].set_name_after_reference_adjustment(sheet_name_str);
        Ok(())
    }

    /// (This method is crate only.)
    /// Check for duplicate sheet name.
    pub(crate) fn check_sheet_name(&self, value: &str) -> Result<(), XlsxError> {
        if self
            .work_sheet_collection
            .iter()
            .any(|work_sheet| value == work_sheet.name())
        {
            Err(XlsxError::NameDuplicate())
        } else {
            Ok(())
        }
    }

    /// (This method is crate only.)
    /// Has Ribbon XML Data.
    #[inline]
    pub(crate) fn has_ribbon(&self) -> bool {
        self.ribbon_xml_data.has_value()
    }

    /// Get Workbook View.
    #[inline]
    #[must_use]
    pub fn workbook_view(&self) -> &WorkbookView {
        &self.workbook_view
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use workbook_view()")]
    pub fn get_workbook_view(&self) -> &WorkbookView {
        self.workbook_view()
    }

    /// Get Workbook View in mutable.
    #[inline]
    pub fn workbook_view_mut(&mut self) -> &mut WorkbookView {
        &mut self.workbook_view
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use workbook_view_mut()")]
    pub fn get_workbook_view_mut(&mut self) -> &mut WorkbookView {
        self.workbook_view_mut()
    }

    /// Set Workbook View.
    /// # Arguments
    /// * `value` - `WorkbookView`
    #[inline]
    pub fn set_workbook_view(&mut self, value: WorkbookView) -> &mut Self {
        self.workbook_view = value;
        self
    }

    /// (This method is crate only.)
    /// Has Defined Names.
    #[inline]
    pub(crate) fn has_defined_names(&self) -> bool {
        if !self.defined_names.is_empty() {
            return true;
        }
        self.sheet_collection_no_check()
            .iter()
            .any(Worksheet::has_defined_names)
    }

    #[inline]
    pub(crate) fn backup_context_types(&self) -> &[(Box<str>, Box<str>)] {
        &self.backup_context_types
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use backup_context_types()")]
    pub(crate) fn get_backup_context_types(&self) -> &[(Box<str>, Box<str>)] {
        self.backup_context_types()
    }

    #[inline]
    pub(crate) fn set_backup_context_types(
        &mut self,
        value: impl Into<Vec<(String, String)>>,
    ) -> &mut Self {
        self.backup_context_types = value
            .into()
            .into_iter()
            .map(|(a, b)| (a.into_boxed_str(), b.into_boxed_str()))
            .collect();
        self
    }

    #[must_use]
    pub fn pivot_caches(&self) -> Vec<(String, String, String)> {
        let mut result: Vec<(String, String, String)> = Vec::new();
        for (val1, val2, val3) in &self.pivot_caches {
            let val3_up = format!("xl/{val3}");

            // Check if this cache is referenced by any worksheet's pivot cache
            // definitions (for existing files)
            let mut found_in_raw_data = false;
            for worksheet in self.sheet_collection_no_check() {
                for pivot_cache_definition in worksheet.pivot_cache_definition_collection() {
                    if val3_up == pivot_cache_definition
                        && !result.iter().any(|(_, _, r_val3)| r_val3 == &**val3)
                    {
                        result.push((val1.to_string(), val2.to_string(), val3.to_string()));
                        found_in_raw_data = true;
                        break;
                    }
                }
                if found_in_raw_data {
                    break;
                }
            }

            // If not found in raw data, check if any worksheet has pivot tables
            // (for newly created pivot tables)
            if !found_in_raw_data {
                for worksheet in self.sheet_collection_no_check() {
                    if !worksheet.pivot_tables().is_empty()
                        && !result.iter().any(|(_, _, r_val3)| r_val3 == &**val3)
                    {
                        result.push((val1.to_string(), val2.to_string(), val3.to_string()));
                        break;
                    }
                }
            }
        }
        result
    }

    #[deprecated(since = "3.0.0", note = "Use pivot_caches()")]
    pub(crate) fn get_pivot_caches(&self) -> Vec<(String, String, String)> {
        self.pivot_caches()
    }

    #[inline]
    pub fn add_pivot_caches(&mut self, value: (String, String, String)) -> &mut Self {
        self.pivot_caches.push((
            value.0.into_boxed_str(),
            value.1.into_boxed_str(),
            value.2.into_boxed_str(),
        ));
        self
    }

    #[inline]
    pub fn update_pivot_caches(&mut self, key: &str, value: &str) -> &mut Self {
        self.pivot_caches.iter_mut().for_each(|(val1, _, val3)| {
            if &**val1 == key {
                *val3 = value.to_owned().into_boxed_str();
            }
        });
        self
    }

    #[inline]
    #[must_use]
    pub fn workbook_protection(&self) -> Option<&WorkbookProtection> {
        self.workbook_protection.as_deref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use workbook_protection()")]
    pub fn get_workbook_protection(&self) -> Option<&WorkbookProtection> {
        self.workbook_protection()
    }

    #[inline]
    pub fn workbook_protection_mut(&mut self) -> &mut WorkbookProtection {
        self.workbook_protection
            .get_or_insert(Box::new(WorkbookProtection::default()))
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use workbook_protection_mut()")]
    pub fn get_workbook_protection_mut(&mut self) -> &mut WorkbookProtection {
        self.workbook_protection_mut()
    }

    #[inline]
    pub fn set_workbook_protection(&mut self, value: WorkbookProtection) -> &mut Self {
        self.workbook_protection = Some(Box::new(value));
        self
    }

    #[inline]
    pub fn remove_workbook_protection(&mut self) -> &mut Self {
        self.workbook_protection = None;
        self
    }

    /// Get Defined Name (Vec).
    #[inline]
    #[must_use]
    pub fn defined_names(&self) -> &[DefinedName] {
        &self.defined_names
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use defined_names()")]
    pub fn get_defined_names(&self) -> &[DefinedName] {
        self.defined_names()
    }

    /// Get Defined Name (Vec) in mutable.
    #[inline]
    pub fn defined_names_mut(&mut self) -> &mut Vec<DefinedName> {
        &mut self.defined_names
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use defined_names_mut()")]
    pub fn get_defined_names_mut(&mut self) -> &mut Vec<DefinedName> {
        self.defined_names_mut()
    }

    /// Set Defined Name (Vec).
    /// # Arguments
    /// * `value` - Vec<DefinedName>.
    #[inline]
    pub fn set_defined_names(&mut self, value: impl Into<Vec<DefinedName>>) {
        self.defined_names = value.into();
    }

    /// Add Defined Name.
    /// # Arguments
    /// * `value` - `DefinedName`.
    #[inline]
    pub fn add_defined_names(&mut self, value: DefinedName) {
        self.defined_names.push(value);
    }

    /// Whether iterative calculation is enabled (`<calcPr iterate="1">`).
    #[inline]
    #[must_use]
    pub fn iterate_enabled(&self) -> bool {
        self.iterate_enabled
    }

    /// `(iterateCount, iterateDelta)` when the workbook specifies them.
    #[inline]
    #[must_use]
    pub fn iterative_calc(&self) -> Option<(u32, f64)> {
        self.iterative_calc
    }

    /// Whether `<calcPr fullCalcOnLoad="1">` will be written.
    #[inline]
    #[must_use]
    pub fn full_calc_on_load(&self) -> bool {
        self.full_calc_on_load
    }

    /// Ask Excel to recalculate everything on open. This can fill missing
    /// caches and replace stale or engine-specific cached results with
    /// Excel's values.
    #[inline]
    pub fn set_full_calc_on_load(&mut self, enabled: bool) {
        self.full_calc_on_load = enabled;
    }

    /// Enable iterative calculation on write. `count`/`delta` are Excel's
    /// convergence limits; pass `None` to emit `iterate` alone and let Excel
    /// apply its own defaults.
    #[inline]
    pub fn set_iterative_calc(&mut self, enabled: bool, count_delta: Option<(u32, f64)>) {
        self.iterate_enabled = enabled;
        self.iterative_calc = count_delta;
    }

    /// Has `ThreadedComments`.
    #[inline]
    #[must_use]
    pub fn has_threaded_comments(&self) -> bool {
        for worksheet in &self.work_sheet_collection {
            if worksheet.has_threaded_comments() {
                return true;
            }
        }
        false
    }
}
impl AdjustmentCoordinateWithSheet for Workbook {
    fn adjustment_insert_coordinate_with_sheet(
        &mut self,
        sheet_name: &str,
        root_col_num: u32,
        offset_col_num: u32,
        root_row_num: u32,
        offset_row_num: u32,
    ) {
        use crate::drawing::charts::ChartReferenceEdit;
        if (offset_col_num != 0
            && (!(1..=16_384).contains(&root_col_num) || offset_col_num > 16_384))
            || (offset_row_num != 0
                && (!(1..=1_048_576).contains(&root_row_num) || offset_row_num > 1_048_576))
        {
            return;
        }
        let mut edits = Vec::new();
        if offset_col_num != 0 {
            edits.push(ChartReferenceEdit::InsertColumns {
                sheet: sheet_name.into(),
                at:    root_col_num,
                count: offset_col_num,
            });
        }
        if offset_row_num != 0 {
            edits.push(ChartReferenceEdit::InsertRows {
                sheet: sheet_name.into(),
                at:    root_row_num,
                count: offset_row_num,
            });
        }
        if edits.is_empty() {
            return;
        }
        let hosts = self.structural_reference_hosts(sheet_name, &edits);
        if hosts.is_empty() {
            return;
        }
        let prepared: Vec<_> = edits
            .iter()
            .map(|edit| {
                crate::drawing::charts::reference_edit_engine::PreparedReferenceEdit::new(edit)
                    .expect("validated structural edit")
            })
            .collect();
        self.adjust_global_defined_names(&prepared);
        for index in hosts {
            let worksheet = &mut self.work_sheet_collection[index];
            if worksheet.name() == sheet_name {
                worksheet.adjustment_insert_geometry(
                    root_col_num,
                    offset_col_num,
                    root_row_num,
                    offset_row_num,
                );
            }
            worksheet.adjust_reference_semantics(
                &crate::drawing::charts::CoordinateChange {
                    sheet:     sheet_name,
                    root_col:  root_col_num,
                    col_count: offset_col_num,
                    root_row:  root_row_num,
                    row_count: offset_row_num,
                    remove:    false,
                },
                &prepared,
            );
        }
    }

    fn adjustment_remove_coordinate_with_sheet(
        &mut self,
        sheet_name: &str,
        root_col_num: u32,
        offset_col_num: u32,
        root_row_num: u32,
        offset_row_num: u32,
    ) {
        use crate::drawing::charts::ChartReferenceEdit;
        if (offset_col_num != 0
            && (!(1..=16_384).contains(&root_col_num) || offset_col_num > 16_384))
            || (offset_row_num != 0
                && (!(1..=1_048_576).contains(&root_row_num) || offset_row_num > 1_048_576))
        {
            return;
        }
        let mut edits = Vec::new();
        if offset_col_num != 0 {
            edits.push(ChartReferenceEdit::DeleteColumns {
                sheet:   sheet_name.into(),
                columns: (root_col_num..root_col_num.saturating_add(offset_col_num).min(16_385))
                    .collect(),
            });
        }
        if offset_row_num != 0 {
            edits.push(ChartReferenceEdit::DeleteRows {
                sheet: sheet_name.into(),
                rows:  (root_row_num..root_row_num.saturating_add(offset_row_num).min(1_048_577))
                    .collect(),
            });
        }
        if edits.is_empty() {
            return;
        }
        let hosts = self.structural_reference_hosts(sheet_name, &edits);
        if hosts.is_empty() {
            return;
        }
        let prepared: Vec<_> = edits
            .iter()
            .map(|edit| {
                crate::drawing::charts::reference_edit_engine::PreparedReferenceEdit::new(edit)
                    .expect("validated structural edit")
            })
            .collect();
        self.adjust_global_defined_names(&prepared);
        for index in hosts {
            let worksheet = &mut self.work_sheet_collection[index];
            if worksheet.name() == sheet_name {
                worksheet.adjustment_remove_geometry(
                    root_col_num,
                    offset_col_num,
                    root_row_num,
                    offset_row_num,
                );
            }
            worksheet.adjust_reference_semantics(
                &crate::drawing::charts::CoordinateChange {
                    sheet:     sheet_name,
                    root_col:  root_col_num,
                    col_count: offset_col_num,
                    root_row:  root_row_num,
                    row_count: offset_row_num,
                    remove:    true,
                },
                &prepared,
            );
        }
    }
}

#[cfg(test)]
mod full_calc_on_load_tests {
    use crate::new_file;

    /// A missing formula cache is invisible to readers that do not recalculate;
    /// a present cache can still be stale or engine-specific. The flag tells
    /// Excel to replace either form with its own calculation on open.
    ///
    /// Task 129 (2026-08-28): 30 formulas exported as `<f>..</f><v/>` and the
    /// football-field chart drew nothing.
    #[test]
    fn emits_full_calc_on_load_only_when_asked() {
        for enabled in [false, true] {
            let mut book = new_file();
            book.set_full_calc_on_load(enabled);
            assert_eq!(book.full_calc_on_load(), enabled);

            let mut buf = Vec::new();
            crate::writer::xlsx::write_writer(&book, &mut buf).expect("write");

            let mut zip = zip::ZipArchive::new(std::io::Cursor::new(buf)).expect("zip");
            let mut xml = String::new();
            {
                use std::io::Read as _;
                zip.by_name("xl/workbook.xml")
                    .expect("workbook.xml")
                    .read_to_string(&mut xml)
                    .expect("read");
            }
            let has = xml.contains(r#"fullCalcOnLoad="1""#);
            assert_eq!(
                has,
                enabled,
                "fullCalcOnLoad present={has} but expected {enabled}; calcPr was: {}",
                xml.split("<calcPr")
                    .nth(1)
                    .unwrap_or("<none>")
                    .split('>')
                    .next()
                    .unwrap_or("")
            );
        }
    }
}
