use std::{
    collections::HashMap,
    io::Cursor,
};

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use crate::{
    helper::{
        coordinate::index_from_coordinate,
        formula::{
            FormulaToken,
            adjustment_formula_coordinate,
            parse_to_tokens,
            render,
        },
    },
    reader::driver::{
        get_attribute,
        set_string_from_xml,
    },
    structs::{
        BooleanValue,
        CellFormulaValues,
        ColumnReference,
        EnumValue,
        Range,
        RowReference,
        StringValue,
        UInt32Value,
    },
    traits::{
        AdjustmentCoordinate,
        AdjustmentCoordinateWith2Sheet,
    },
    writer::driver::{
        write_end_tag,
        write_start_tag,
        write_text_node_conversion,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug, PartialEq, PartialOrd)]
pub struct CellFormula {
    bx:             BooleanValue,
    data_table_2d:  BooleanValue,
    data_table_row: BooleanValue,
    formula_type:   EnumValue<CellFormulaValues>,
    input_1deleted: BooleanValue,
    input_2deleted: BooleanValue,
    r1:             StringValue,
    r2:             StringValue,
    reference:      StringValue,
    shared_index:   UInt32Value,
    text:           StringValue,
    text_view:      StringValue,
}
impl CellFormula {
    #[inline]
    #[must_use]
    pub fn bx(&self) -> bool {
        self.bx.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use bx()")]
    pub fn get_bx(&self) -> bool {
        self.bx()
    }

    #[inline]
    pub fn set_bx(&mut self, value: bool) -> &mut Self {
        self.bx.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn data_table_2d(&self) -> bool {
        self.data_table_2d.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use bx()")]
    pub fn get_data_table_2d(&self) -> bool {
        self.data_table_2d()
    }

    #[inline]
    pub fn set_data_table_2d(&mut self, value: bool) -> &mut Self {
        self.data_table_2d.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn data_table_row(&self) -> bool {
        self.data_table_row.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use data_table_row()")]
    pub fn get_data_table_row(&self) -> bool {
        self.data_table_row()
    }

    #[inline]
    pub fn set_data_table_row(&mut self, value: bool) -> &mut Self {
        self.data_table_row.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn formula_type(&self) -> &CellFormulaValues {
        self.formula_type.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use formula_type()")]
    pub fn get_formula_type(&self) -> &CellFormulaValues {
        self.formula_type()
    }

    #[inline]
    pub fn set_formula_type(&mut self, value: CellFormulaValues) {
        self.formula_type.set_value(value);
    }

    #[inline]
    #[must_use]
    pub fn input_1deleted(&self) -> bool {
        self.input_1deleted.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use input_1deleted()")]
    pub fn get_input_1deleted(&self) -> bool {
        self.input_1deleted()
    }

    #[inline]
    pub fn set_input_1deleted(&mut self, value: bool) -> &mut Self {
        self.input_1deleted.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn input_2deleted(&self) -> bool {
        self.input_2deleted.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use input_2deleted()")]
    pub fn get_input_2deleted(&self) -> bool {
        self.input_2deleted()
    }

    #[inline]
    pub fn set_input_2deleted(&mut self, value: bool) -> &mut Self {
        self.input_2deleted.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn r1(&self) -> &str {
        self.r1.value_str()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use r1()")]
    pub fn get_r1(&self) -> &str {
        self.r1()
    }

    #[inline]
    pub fn set_r1<S: Into<String>>(&mut self, value: S) -> &mut Self {
        self.r1.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn r2(&self) -> &str {
        self.r2.value_str()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use r2()")]
    pub fn get_r2(&self) -> &str {
        self.r2()
    }
    
    #[inline]
    pub fn set_r2<S: Into<String>>(&mut self, value: S) -> &mut Self {
        self.r2.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn reference(&self) -> &str {
        self.reference.value_str()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use reference()")]
    pub fn get_reference(&self) -> &str {
        self.reference()
    }

    #[inline]
    pub fn set_reference<S: Into<String>>(&mut self, value: S) -> &mut Self {
        self.reference.set_value(value);
        self
    }

    fn array_reference_range(&self) -> Option<Range> {
        if self.formula_type() != &CellFormulaValues::Array {
            return None;
        }
        let reference = self.reference.value()?;
        let (start, end) = reference.split_once(':').unwrap_or((reference, reference));
        let (Some(start_col), Some(start_row), ..) = index_from_coordinate(start) else {
            return None;
        };
        let (Some(end_col), Some(end_row), ..) = index_from_coordinate(end) else {
            return None;
        };
        if start_col == 0 || start_row == 0 || start_col > end_col || start_row > end_row {
            return None;
        }
        let mut range = Range::default();
        range.set_range(reference);
        Some(range)
    }

    fn array_reference_after_removal(
        &self,
        root_col_num: u32,
        offset_col_num: u32,
        root_row_num: u32,
        offset_row_num: u32,
    ) -> Option<String> {
        // Cells removes a deleted top-left anchor before formula adjustments.
        // Do not fabricate a replacement owner if the start of either
        // axis is deleted.
        fn remove_axis(start: u32, end: u32, root: u32, count: u32) -> Option<(u32, u32)> {
            if root == 0 || count == 0 || root > end {
                return Some((start, end));
            }
            if root <= start {
                if count > start - root {
                    return None;
                }
                return Some((start - count, end - count));
            }
            // Clip the deleted tail to this range, including oversized
            // removals.
            Some((start, end - count.min(end - root + 1)))
        }

        let mut range = self.array_reference_range()?;
        let start_col = range.coordinate_start_col()?.num();
        let start_row = range.coordinate_start_row()?.num();
        let end_col = range
            .coordinate_end_col()
            .map_or(start_col, ColumnReference::num);
        let end_row = range
            .coordinate_end_row()
            .map_or(start_row, RowReference::num);
        let (start_col, end_col) = remove_axis(start_col, end_col, root_col_num, offset_col_num)?;
        let (start_row, end_row) = remove_axis(start_row, end_row, root_row_num, offset_row_num)?;
        range.coordinate_start_col_mut()?.set_num(start_col);
        range.coordinate_start_row_mut()?.set_num(start_row);
        if let Some(value) = range.coordinate_end_col_mut() {
            value.set_num(end_col);
        }
        if let Some(value) = range.coordinate_end_row_mut() {
            value.set_num(end_row);
        }
        Some(range.range())
    }

    #[inline]
    #[must_use]
    pub fn shared_index(&self) -> u32 {
        self.shared_index.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use shared_index()")]
    pub fn get_shared_index(&self) -> u32 {
        self.shared_index()
    }

    #[inline]
    pub fn set_shared_index(&mut self, value: u32) -> &mut Self {
        self.shared_index.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn text(&self) -> &str {
        if self.text_view.has_value() {
            return self.text_view.value_str();
        }
        self.text.value_str()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use text()")]
    pub fn get_text(&self) -> &str {
        self.text()
    }

    #[inline]
    pub fn set_text<S: Into<String>>(&mut self, value: S) -> &mut Self {
        self.text.set_value(value);
        self
    }

    #[inline]
    pub fn set_text_view<S: Into<String>>(&mut self, value: S) -> &mut Self {
        self.text_view.set_value(value);
        self
    }

    #[inline]
    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
        is_empty: bool,
        cell_reference_str: &str,
        formula_shared_list: &mut HashMap<u32, (String, Vec<FormulaToken>)>,
    ) {
        set_string_from_xml!(self, e, bx, "bx");
        set_string_from_xml!(self, e, data_table_2d, "dt2D");
        set_string_from_xml!(self, e, data_table_row, "dtr");
        set_string_from_xml!(self, e, formula_type, "t");
        set_string_from_xml!(self, e, input_1deleted, "del1");
        set_string_from_xml!(self, e, input_2deleted, "del2");
        set_string_from_xml!(self, e, r1, "r1");
        set_string_from_xml!(self, e, r2, "r2");
        set_string_from_xml!(self, e, reference, "ref");
        set_string_from_xml!(self, e, shared_index, "si");

        if !is_empty {
            let mut buf = Vec::new();
            let text = reader
                    .read_text_into(e.name(), &mut buf).unwrap();
            self.text.set_value(crate::helper::utils::unescape_xml_text(&text));
        }

        // Shared
        if self.formula_type.value() == &CellFormulaValues::Shared {
            match formula_shared_list.get(&self.shared_index.value()) {
                Some((parent_cell_reference_str, token)) => {
                    let parent_cell = index_from_coordinate(parent_cell_reference_str);
                    let self_cell = index_from_coordinate(cell_reference_str);
                    let (Some(parent_col_num), Some(parent_row_num), ..) = parent_cell else {
                        return;
                    };
                    let (Some(self_col_num), Some(self_row_num), ..) = self_cell else {
                        return;
                    };

                    // Shared-formula sibling rebasing: translate every
                    // RELATIVE reference in the master's formula by
                    // (self - master) so each sibling references the cells at
                    // its own offset; absolute refs ($A$1) stay put.
                    //
                    // `adjustment_insert_formula_coordinate` has insert-shift
                    // semantics (shift refs at >= root by N) — the wrong
                    // primitive here: it left every sibling pointing at the
                    // master's relative cells (e.g. C1 rendered as `A1+1`
                    // instead of `B1+1`). `adjustment_formula_coordinate` is
                    // the translate-to-new-origin primitive.
                    let offset_col_num: i32 = num_traits::cast::<_, i32>(self_col_num).unwrap()
                        - num_traits::cast::<_, i32>(parent_col_num).unwrap();
                    let offset_row_num: i32 = num_traits::cast::<_, i32>(self_row_num).unwrap()
                        - num_traits::cast::<_, i32>(parent_row_num).unwrap();

                    let mut token_new = token.clone();
                    adjustment_formula_coordinate(&mut token_new, offset_col_num, offset_row_num);
                    let value = render(token_new.as_ref());
                    self.text_view.set_value(value);
                }
                None => {
                    formula_shared_list.insert(
                        self.shared_index.value(),
                        (
                            cell_reference_str.to_string(),
                            parse_to_tokens(format!("={}", self.text.value_str())),
                        ),
                    );
                }
            }
        }
    }

    #[inline]
    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        coordinate: &str,
        formula_shared_list: &HashMap<u32, (String, Option<String>)>,
    ) {
        // f
        let mut attributes: crate::structs::AttrCollection = Vec::new();
        let bx_str = self.bx.value_string();
        if self.bx.has_value() {
            attributes.push(("bx", bx_str).into());
        }

        let data_table_2d_str = self.data_table_2d.value_string();
        if self.data_table_2d.has_value() {
            attributes.push(("dt2D", data_table_2d_str).into());
        }

        let data_table_row_str = self.data_table_row.value_string();
        if self.data_table_row.has_value() {
            attributes.push(("dtr", data_table_row_str).into());
        }

        let formula_type_str = self.formula_type.value_string();
        if self.formula_type.has_value() {
            // `t="array"` used to be suppressed here ("Not SUPPORT Array"),
            // which silently downgraded every Ctrl-Shift-Enter array to a plain
            // formula on write. That is not harmless: the members of the array
            // `ref` rectangle are ordinary cells in the file, so Excel 365 reads
            // the bare `<f>` as a SPILLING formula, finds them occupied and
            // returns `#SPILL!`. The `ref` itself already reaches the output
            // through the `self.reference` branch below.
            attributes.push(("t", formula_type_str).into());
        }

        let input_1deleted_str = self.input_1deleted.value_string();
        if self.input_1deleted.has_value() {
            attributes.push(("del1", input_1deleted_str).into());
        }

        let input_2deleted_str = self.input_2deleted.value_string();
        if self.input_2deleted.has_value() {
            attributes.push(("del2", input_2deleted_str).into());
        }

        if self.r1.has_value() {
            attributes.push(("r1", self.r1.value_str()).into());
        }

        if self.r2.has_value() {
            attributes.push(("r2", self.r2.value_str()).into());
        }

        #[allow(unused_assignments)]
        let mut reference_str = String::new();
        let mut wrote_ref = false;
        if let Some((start_col, end_col)) = formula_shared_list.get(&self.shared_index.value())
        {
            if coordinate == start_col {
                reference_str = match end_col {
                    Some(v) => {
                        format!("{start_col}:{v}")
                    }
                    None => start_col.clone(),
                };
                attributes.push(("ref", &reference_str).into());
                wrote_ref = true;
            }
        }

        // A What-If Data Table anchor carries its own `ref` — the block it drives,
        // e.g. `<f t="dataTable" ref="AG18:AK22" dt2D="1" dtr="1" r1="C20" r2="C31"/>`.
        // The reader parses it into `self.reference` (the `b"ref"` arm above) but this
        // writer only ever emitted `ref` for SHARED formulas, from the lookup above, so
        // the attribute was dropped and the table reached Excel as a dead block of
        // static values.
        //
        // Guarded on `wrote_ref`: a shared master has BOTH a parsed `self.reference`
        // and a shared-list-derived one. Emitting both yields a duplicate XML attribute,
        // which makes Excel repair every workbook that contains shared formulas.
        if !wrote_ref && self.reference.has_value() {
            attributes.push(("ref", self.reference.value_str()).into());
        }

        let shared_index_str = self.shared_index.value_string();
        if self.shared_index.has_value() {
            attributes.push(("si", &shared_index_str).into());
        }

        write_start_tag(writer, "f", attributes, false);
        write_text_node_conversion(writer, self.text.value_str());
        write_end_tag(writer, "f");
    }
}
impl CellFormula {
    pub(crate) fn adjust_array_geometry(&mut self, same_sheet: bool, root_col: u32, col_count: u32, root_row: u32, row_count: u32, remove: bool) {
        if !same_sheet { return; }
        if remove {
            if let Some(reference) = self.array_reference_after_removal(root_col, col_count, root_row, row_count) {
                self.reference.set_value(reference);
            }
        } else if let Some(mut range) = self.array_reference_range() {
            range.adjustment_insert_coordinate(root_col, col_count, root_row, row_count);
            self.reference.set_value(range.range());
        }
    }

    pub(crate) fn adjust_reference_texts_prepared(
        &mut self,
        host: &str,
        prepared: &crate::drawing::charts::reference_edit_engine::PreparedReferenceEdit<'_>,
        moved: bool,
    ) -> bool {
        use crate::drawing::charts::reference_edit_engine::{edit_prepared_formula_text, edit_prepared_formula_text_for_moved_cell};
        let mut changed = false;
        for text in [&mut self.text, &mut self.text_view] {
            if let Some(before) = text.value() {
                let result = if moved {
                    edit_prepared_formula_text_for_moved_cell(before, host, prepared, false)
                } else {
                    edit_prepared_formula_text(before, host, prepared, false)
                };
                if result.text != before { text.set_value(result.text); changed = true; }
            }
        }
        changed
    }

    fn adjust_reference_texts(&mut self, host: &str, edits: &[crate::drawing::charts::ChartReferenceEdit]) {
        use crate::drawing::charts::reference_edit_engine::PreparedReferenceEdit;
        for edit in edits {
            let Ok(prepared) = PreparedReferenceEdit::new(edit) else { return; };
            self.adjust_reference_texts_prepared(host, &prepared, false);
        }
    }
}
impl AdjustmentCoordinateWith2Sheet for CellFormula {
    #[inline]
    fn adjustment_insert_coordinate_with_2sheet(
        &mut self,
        self_sheet_name: &str,
        sheet_name: &str,
        root_col_num: u32,
        offset_col_num: u32,
        root_row_num: u32,
        offset_row_num: u32,
    ) {
        use crate::drawing::charts::ChartReferenceEdit;
        // An array's ref describes owned cells on this sheet, not references in
        // the formula text. Its geometry moves even when its coordinates use $.
        if self_sheet_name == sheet_name {
            if let Some(mut range) = self.array_reference_range() {
                range.adjustment_insert_coordinate(
                    root_col_num,
                    offset_col_num,
                    root_row_num,
                    offset_row_num,
                );
                self.reference.set_value(range.range());
            }
        }
        let mut edits = Vec::new();
        if offset_col_num != 0 { edits.push(ChartReferenceEdit::InsertColumns { sheet: sheet_name.into(), at: root_col_num, count: offset_col_num }); }
        if offset_row_num != 0 { edits.push(ChartReferenceEdit::InsertRows { sheet: sheet_name.into(), at: root_row_num, count: offset_row_num }); }
        self.adjust_reference_texts(self_sheet_name, &edits);
    }

    #[inline]
    fn adjustment_remove_coordinate_with_2sheet(
        &mut self,
        self_sheet_name: &str,
        sheet_name: &str,
        root_col_num: u32,
        offset_col_num: u32,
        root_row_num: u32,
        offset_row_num: u32,
    ) {
        use crate::drawing::charts::ChartReferenceEdit;
        if self_sheet_name == sheet_name {
            if let Some(reference) = self.array_reference_after_removal(
                root_col_num,
                offset_col_num,
                root_row_num,
                offset_row_num,
            ) {
                self.reference.set_value(reference);
            }
        }
        let mut edits = Vec::new();
        if offset_col_num != 0 { edits.push(ChartReferenceEdit::DeleteColumns {
            sheet: sheet_name.into(), columns: (root_col_num..root_col_num.saturating_add(offset_col_num).min(16_385)).collect(),
        }); }
        if offset_row_num != 0 { edits.push(ChartReferenceEdit::DeleteRows {
            sheet: sheet_name.into(), rows: (root_row_num..root_row_num.saturating_add(offset_row_num).min(1_048_577)).collect(),
        }); }
        self.adjust_reference_texts(self_sheet_name, &edits);
    }
}

#[cfg(test)]
mod array_reference_adjustment_tests {
    use super::*;

    fn array_formula(reference: &str) -> CellFormula {
        let mut formula = CellFormula::default();
        formula.set_formula_type(CellFormulaValues::Array);
        formula.set_reference(reference);
        formula.set_text("_xludf.rri(3,100,200)");
        formula
    }

    #[test]
    fn array_geometry_moves_locked_and_single_cell_references() {
        for (reference, expected) in [
            ("$C$4:$E$7", "$D$6:$F$9"),
            ("C4", "D6"),
            ("C$4:$E7", "D$6:$F9"),
        ] {
            let mut formula = array_formula(reference);
            formula.adjustment_insert_coordinate_with_2sheet("Main", "Main", 2, 1, 2, 2);
            assert_eq!(formula.reference(), expected);
            assert_eq!(formula.text(), "_xludf.rri(3,100,200)");
            formula.adjustment_remove_coordinate_with_2sheet("Main", "Main", 2, 1, 2, 2);
            assert_eq!(formula.reference(), reference);
        }
    }

    #[test]
    fn array_removal_clips_deleted_members_without_zero_coordinates() {
        let formula = array_formula("$C$4:$E$7");
        for (root_col, col_count, root_row, row_count, expected) in [
            (2, 1, 2, 1, "$B$3:$D$6"),
            (4, 1, 5, 1, "$C$4:$D$6"),
            (5, 100, 7, 100, "$C$4:$D$6"),
            (4, u32::MAX, 5, u32::MAX, "$C$4:$C$4"),
            (6, 100, 8, 100, "$C$4:$E$7"),
            (0, 0, 0, 0, "$C$4:$E$7"),
        ] {
            assert_eq!(
                formula.array_reference_after_removal(root_col, col_count, root_row, row_count),
                Some(expected.to_string()),
            );
        }
        assert_eq!(formula.array_reference_after_removal(3, 1, 0, 0), None);
        assert_eq!(formula.array_reference_after_removal(0, 0, 4, 1), None);
    }

    #[test]
    fn malformed_array_references_are_preserved_without_panicking() {
        for reference in [
            "",
            "A0:B2",
            "B2:A3",
            "A2:B1",
            "Sheet1!A1:B2",
            "A1:B2:C3",
            "A:B",
            "1:3",
        ] {
            let mut formula = array_formula(reference);
            formula.adjustment_insert_coordinate_with_2sheet("Main", "Main", 1, 1, 1, 1);
            assert_eq!(formula.reference(), reference);
            formula.adjustment_remove_coordinate_with_2sheet("Main", "Main", 1, 1, 1, 1);
            assert_eq!(formula.reference(), reference);
        }
    }

    #[test]
    fn only_same_sheet_array_metadata_is_adjusted() {
        for formula_type in [
            CellFormulaValues::Shared,
            CellFormulaValues::DataTable,
            CellFormulaValues::Normal,
        ] {
            let mut formula = array_formula("C4:E7");
            formula.set_formula_type(formula_type);
            formula.adjustment_insert_coordinate_with_2sheet("Main", "Main", 2, 1, 2, 1);
            assert_eq!(formula.reference(), "C4:E7");
            formula.adjustment_remove_coordinate_with_2sheet("Main", "Main", 2, 1, 2, 1);
            assert_eq!(formula.reference(), "C4:E7");
        }

        let mut formula = array_formula("C4:E7");
        formula.set_text("_xludf.rri(Other!A1,100,200)");
        formula.adjustment_insert_coordinate_with_2sheet("Main", "Other", 0, 0, 1, 1);
        assert_eq!(formula.reference(), "C4:E7");
        assert_eq!(formula.text(), "_xludf.rri(Other!A2,100,200)");
        formula.adjustment_remove_coordinate_with_2sheet("Main", "Other", 0, 0, 1, 1);
        assert_eq!(formula.reference(), "C4:E7");
        assert_eq!(formula.text(), "_xludf.rri(Other!A1,100,200)");
    }
}
