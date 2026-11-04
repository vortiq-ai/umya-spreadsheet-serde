use super::{
    BooleanValue,
    EnumValue,
    StringValue,
    TotalsRowFunctionValues,
    UInt32Value,
    coordinate::Coordinate,
};
use crate::{
    AutoFilter,
    helper::coordinate::CellCoordinates,
    traits::AdjustmentCoordinate,
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct Table {
    name:             Box<str>,
    area:             (Coordinate, Coordinate),
    display_name:     Box<str>,
    columns:          Vec<TableColumn>,
    style_info:       Option<Box<TableStyleInfo>>,
    totals_row_shown: BooleanValue,
    totals_row_count: UInt32Value,
    #[cfg_attr(feature = "serde", serde(default))]
    auto_filter:      AutoFilter,
    #[cfg_attr(feature = "serde", serde(default))]
    id:               Option<u32>,
}
impl Table {
    /// Plan on a clone before moving cells; commit only after every affected
    /// object accepts the edit. New column headers must also be written to
    /// the cells by the worksheet owner.
    pub(crate) fn insert_geometry(
        &mut self,
        root_col: u32,
        col_count: u32,
        root_row: u32,
        row_count: u32,
    ) -> Result<(), String> {
        if self.geometry_edit_is_after(root_col, col_count, root_row, row_count) {
            return Ok(());
        }
        self.check_geometry_edit(root_col, col_count, root_row, row_count)?;
        let mut next = self.clone();
        if col_count != 0 && root_col > self.area.0.col_num() && root_col <= self.area.1.col_num() {
            let index = (root_col - self.area.0.col_num()) as usize;
            // Preserve imported identities and allocate missing in-memory IDs
            // in the same order as table serialization before inserting.
            let mut reserved = std::collections::HashSet::new();
            for id in next.columns.iter().filter_map(TableColumn::id) {
                if !reserved.insert(id) {
                    return Err("duplicate table column ID".into());
                }
            }
            let mut candidate = 1_u32;
            for column in &mut next.columns {
                if column.id.is_none() {
                    while reserved.contains(&candidate) {
                        candidate = candidate.checked_add(1).ok_or("table column ID overflow")?;
                    }
                    column.set_id(candidate);
                    reserved.insert(candidate);
                }
            }
            for offset in 0..col_count {
                let mut id = 1_u32;
                while next.columns.iter().any(|column| column.id == Some(id)) {
                    id = id.checked_add(1).ok_or("table column ID overflow")?;
                }
                let mut suffix = 1_u32;
                while next
                    .columns
                    .iter()
                    .any(|column| column.name.eq_ignore_ascii_case(&format!("Column{suffix}")))
                {
                    suffix = suffix.checked_add(1).ok_or("table column name overflow")?;
                }
                let mut column = TableColumn::new(&format!("Column{suffix}"));
                column.set_id(id);
                next.columns.insert(index + offset as usize, column);
            }
        }
        next.area
            .0
            .adjustment_insert_coordinate(root_col, col_count, root_row, row_count);
        next.area
            .1
            .adjustment_insert_coordinate(root_col, col_count, root_row, row_count);
        next.auto_filter
            .adjustment_insert_coordinate(root_col, col_count, root_row, row_count);
        // Inserting immediately above totals grows the filter's data range
        // even though the insertion is beyond its old trailing endpoint.
        if self.filter_matches_data_area() {
            next.set_data_filter_range();
        }
        if next.area.1.col_num() > 16_384 || next.area.1.row_num() > 1_048_576 {
            return Err("table insertion exceeds the worksheet grid".into());
        }
        *self = next;
        Ok(())
    }

    /// Returns true when the table is wholly deleted. Refused edits and
    /// removals leave this value untouched, so the owner can preflight the
    /// whole workbook before changing cells or dependent consumers.
    pub(crate) fn remove_geometry(
        &mut self,
        root_col: u32,
        col_count: u32,
        root_row: u32,
        row_count: u32,
    ) -> Result<bool, String> {
        if self.geometry_edit_is_after(root_col, col_count, root_row, row_count) {
            return Ok(false);
        }
        self.check_geometry_edit(root_col, col_count, root_row, row_count)?;
        let covered = |start: u32, end: u32, root: u32, count: u32| {
            count != 0 && root <= start && u64::from(root) + u64::from(count) > u64::from(end)
        };
        if covered(
            self.area.0.col_num(),
            self.area.1.col_num(),
            root_col,
            col_count,
        ) || covered(
            self.area.0.row_num(),
            self.area.1.row_num(),
            root_row,
            row_count,
        ) {
            return Ok(true);
        }
        let contains = |value: u32, root: u32, count: u32| {
            count != 0 && value >= root && u64::from(value) < u64::from(root) + u64::from(count)
        };
        if contains(self.area.0.row_num(), root_row, row_count) {
            return Err("cannot delete a table header without deleting the whole table".into());
        }
        let boundary = |value: u32, root: u32, count: u32, trailing: bool| {
            if count == 0 || value < root {
                value
            } else if u64::from(value) >= u64::from(root) + u64::from(count) {
                value - count
            } else if trailing {
                root - 1
            } else {
                root
            }
        };
        let mut next = self.clone();
        next.columns = (self.area.0.col_num()..=self.area.1.col_num())
            .zip(&self.columns)
            .filter(|(column, _)| !contains(*column, root_col, col_count))
            .map(|(_, column)| column.clone())
            .collect();
        next.area
            .0
            .set_col_num(boundary(self.area.0.col_num(), root_col, col_count, false));
        next.area
            .1
            .set_col_num(boundary(self.area.1.col_num(), root_col, col_count, true));
        next.area
            .0
            .set_row_num(boundary(self.area.0.row_num(), root_row, row_count, false));
        next.area
            .1
            .set_row_num(boundary(self.area.1.row_num(), root_row, row_count, true));
        if self.totals_row_count() != 0 && contains(self.area.1.row_num(), root_row, row_count) {
            next.totals_row_count = UInt32Value::default();
            next.totals_row_shown.set_value(false);
            for column in &mut next.columns {
                column.totals_row_label.remove_value();
                column.totals_row_function = EnumValue::default();
            }
        }
        // Excel retains one blank data row when the last data row is
        // deleted. The owner must preserve/move the totals cells accordingly.
        next.area.1.set_row_num(
            next.area
                .1
                .row_num()
                .max(next.area.0.row_num() + 1 + next.totals_row_count()),
        );
        next.auto_filter
            .adjustment_remove_coordinate(root_col, col_count, root_row, row_count);
        if self.filter_matches_data_area() {
            next.set_data_filter_range();
        }
        *self = next;
        Ok(false)
    }

    fn geometry_edit_is_after(&self, col: u32, cols: u32, row: u32, rows: u32) -> bool {
        (cols == 0 || (col > 0 && col <= 16_384 && cols <= 16_385 - col))
            && (rows == 0 || (row > 0 && row <= 1_048_576 && rows <= 1_048_577 - row))
            && (cols == 0 || col > self.area.1.col_num())
            && (rows == 0 || row > self.area.1.row_num())
    }

    fn check_geometry_edit(
        &self,
        root_col: u32,
        col_count: u32,
        root_row: u32,
        row_count: u32,
    ) -> Result<(), String> {
        if !self.is_ok()
            || self.area.1.col_num() > 16_384
            || self.area.1.row_num() > 1_048_576
            || self.totals_row_count() > 1
            || self.totals_row_count() > self.area.1.row_num() - self.area.0.row_num()
            || self.columns.len() != (self.area.1.col_num() - self.area.0.col_num() + 1) as usize
        {
            return Err("invalid table geometry or column count".into());
        }
        for (root, count, limit) in [
            (root_col, col_count, 16_384_u32),
            (root_row, row_count, 1_048_576),
        ] {
            if count != 0
                && (root == 0 || u64::from(root) + u64::from(count) > u64::from(limit) + 1)
            {
                return Err("table edit is outside the worksheet grid".into());
            }
        }
        Ok(())
    }

    fn filter_matches_data_area(&self) -> bool {
        let mut end = self.area.1.clone();
        end.set_row_num(end.row_num() - self.totals_row_count());
        self.auto_filter.range().range() == format!("{}:{end}", self.area.0)
    }

    fn set_data_filter_range(&mut self) {
        let mut end = self.area.1.clone();
        end.set_row_num(end.row_num() - self.totals_row_count());
        self.auto_filter.set_range(format!("{}:{end}", self.area.0));
    }

    #[inline]
    #[must_use]
    pub fn id(&self) -> Option<u32> {
        self.id
    }

    #[inline]
    pub fn set_id(&mut self, id: u32) -> &mut Self {
        self.id = Some(id);
        self
    }

    #[inline]
    pub fn new<T>(name: &str, area: (T, T)) -> Self
    where
        T: Into<CellCoordinates>,
    {
        let coord_beg = Self::cell_coord_to_coord(area.0);
        let coord_end = Self::cell_coord_to_coord(area.1);
        let name: Box<str> = name.into();
        let mut auto_filter = AutoFilter::default();
        auto_filter.set_range(format!("{coord_beg}:{coord_end}"));
        Self {
            area: (coord_beg, coord_end),
            name: name.clone(),
            display_name: name,
            columns: Vec::<TableColumn>::default(),
            style_info: None,
            totals_row_shown: BooleanValue::default(),
            totals_row_count: UInt32Value::default(),
            auto_filter,
            id: None,
        }
    }

    #[inline]
    #[must_use]
    pub fn is_ok(&self) -> bool {
        !(self.name.is_empty()
            || self.display_name.is_empty()
            || self.area.0.col_num() == 0
            || self.area.0.row_num() == 0
            || self.area.1.col_num() == 0
            || self.area.1.row_num() == 0
            || self.area.0.col_num() > self.area.1.col_num()
            || self.area.0.row_num() > self.area.1.row_num())
    }

    #[inline]
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use name()")]
    pub fn get_name(&self) -> &str {
        self.name()
    }

    #[inline]
    pub fn set_name(&mut self, name: &str) {
        self.name = name.into();
        if self.display_name.is_empty() {
            self.display_name = name.into();
        }
    }

    #[inline]
    #[must_use]
    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use display_name()")]
    pub fn get_display_name(&self) -> &str {
        self.display_name()
    }

    #[inline]
    pub fn set_display_name(&mut self, display_name: &str) {
        self.display_name = display_name.into();
    }

    #[inline]
    #[must_use]
    pub fn area(&self) -> &(Coordinate, Coordinate) {
        &self.area
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use area()")]
    pub fn get_area(&self) -> &(Coordinate, Coordinate) {
        self.area()
    }

    #[inline]
    pub fn set_area<T>(&mut self, area: (T, T))
    where
        T: Into<CellCoordinates>,
    {
        let coord_beg = Self::cell_coord_to_coord(area.0);
        let coord_end = Self::cell_coord_to_coord(area.1);
        if self.auto_filter.range().range() == format!("{}:{}", self.area.0, self.area.1) {
            self.auto_filter
                .set_range(format!("{coord_beg}:{coord_end}"));
        }
        self.area = (coord_beg, coord_end);
    }

    #[inline]
    pub fn add_column(&mut self, col: TableColumn) {
        self.columns.push(col);
    }

    #[inline]
    #[must_use]
    pub fn columns(&self) -> &[TableColumn] {
        &self.columns
    }

    #[inline]
    #[must_use]
    pub fn columns_mut(&mut self) -> &mut [TableColumn] {
        &mut self.columns
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use columns()")]
    pub fn get_columns(&self) -> &[TableColumn] {
        self.columns()
    }

    #[inline]
    pub(crate) fn has_style_info(&self) -> bool {
        self.style_info.is_some()
    }

    #[inline]
    #[must_use]
    pub fn style_info(&self) -> Option<&TableStyleInfo> {
        self.style_info.as_deref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use style_info()")]
    pub fn get_style_info(&self) -> Option<&TableStyleInfo> {
        self.style_info()
    }

    #[inline]
    pub fn set_style_info(&mut self, style_info: Option<TableStyleInfo>) {
        self.style_info = style_info.map(Box::new);
    }

    #[inline]
    pub(crate) fn has_totals_row_shown(&self) -> bool {
        self.totals_row_shown.has_value()
    }

    #[inline]
    #[must_use]
    pub fn totals_row_shown(&self) -> bool {
        self.totals_row_shown.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use totals_row_shown()")]
    pub fn get_totals_row_shown(&self) -> bool {
        self.totals_row_shown()
    }

    #[inline]
    pub(crate) fn totals_row_shown_str(&self) -> &str {
        self.totals_row_shown.value_string()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use totals_row_shown_str()")]
    pub(crate) fn get_totals_row_shown_str(&self) -> &str {
        self.totals_row_shown_str()
    }

    #[inline]
    pub fn set_totals_row_shown(&mut self, value: bool) {
        self.totals_row_shown.set_value(value);
    }

    #[inline]
    pub(crate) fn set_totals_row_shown_str(&mut self, value: &str) {
        self.totals_row_shown.set_value_string(value);
    }

    #[inline]
    pub(crate) fn has_totals_row_count(&self) -> bool {
        self.totals_row_count.has_value()
    }

    #[inline]
    #[must_use]
    pub fn totals_row_count(&self) -> u32 {
        self.totals_row_count.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use totals_row_count()")]
    pub fn get_totals_row_count(&self) -> u32 {
        self.totals_row_count()
    }

    #[inline]
    pub(crate) fn totals_row_count_str(&self) -> String {
        self.totals_row_count.value_string()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use totals_row_count_str()")]
    pub(crate) fn get_totals_row_count_str(&self) -> String {
        self.totals_row_count_str()
    }

    #[inline]
    pub fn set_totals_row_count(&mut self, value: u32) {
        self.totals_row_count.set_value(value);
    }

    #[inline]
    pub(crate) fn set_totals_row_count_str(&mut self, value: &str) {
        self.totals_row_count.set_value_string(value);
    }

    #[inline]
    #[must_use]
    pub fn auto_filter(&self) -> &AutoFilter {
        &self.auto_filter
    }

    #[inline]
    pub fn auto_filter_mut(&mut self) -> &mut AutoFilter {
        &mut self.auto_filter
    }

    #[inline]
    pub fn set_auto_filter<S: Into<String>>(&mut self, range: S) {
        let mut auto_filter = AutoFilter::default();
        auto_filter.set_range(range);
        self.auto_filter = auto_filter;
    }

    #[inline]
    pub(crate) fn set_auto_filter_obj(&mut self, value: AutoFilter) {
        self.auto_filter = value;
    }

    #[inline]
    fn cell_coord_to_coord<T>(cc: T) -> Coordinate
    where
        T: Into<CellCoordinates>,
    {
        let cell_coord: CellCoordinates = cc.into();
        let mut coord: Coordinate = Coordinate::default();
        coord.set_col_num(cell_coord.col);
        coord.set_row_num(cell_coord.row);
        coord
    }
}

#[cfg(test)]
#[path = "../../tests/support/model_table_geometry.rs"]
mod model_geometry_tests;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct TableColumn {
    name:                      String,
    totals_row_label:          StringValue,
    totals_row_function:       EnumValue<TotalsRowFunctionValues>,
    calculated_column_formula: Option<String>,
    #[cfg_attr(feature = "serde", serde(default))]
    id:                        Option<u32>,
}
impl TableColumn {
    #[inline]
    #[must_use]
    pub fn id(&self) -> Option<u32> {
        self.id
    }

    #[inline]
    pub fn set_id(&mut self, id: u32) -> &mut Self {
        self.id = Some(id);
        self
    }

    #[inline]
    #[must_use]
    pub fn new(name: &str) -> Self {
        Self {
            name:                      name.to_string(),
            totals_row_label:          StringValue::default(),
            totals_row_function:       EnumValue::default(),
            calculated_column_formula: None,
            id:                        None,
        }
    }

    #[inline]
    #[must_use]
    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use name()")]
    pub fn get_name(&self) -> &str {
        self.name()
    }

    #[inline]
    pub fn set_name(&mut self, name: String) {
        self.name = name;
    }

    #[inline]
    #[allow(dead_code)]
    pub(crate) fn has_totals_row_label(&self) -> bool {
        self.totals_row_label.has_value()
    }

    #[inline]
    #[must_use]
    pub fn totals_row_label(&self) -> Option<&str> {
        self.totals_row_label.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use totals_row_label()")]
    pub fn get_totals_row_label(&self) -> Option<&str> {
        self.totals_row_label()
    }

    #[inline]
    pub(crate) fn totals_row_label_str(&self) -> &str {
        self.totals_row_label.value_str()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use totals_row_label_str()")]
    pub(crate) fn get_totals_row_label_str(&self) -> &str {
        self.totals_row_label_str()
    }

    #[inline]
    pub fn set_totals_row_label(&mut self, value: &str) {
        self.totals_row_label.set_value(value);
    }

    #[inline]
    pub(crate) fn set_totals_row_label_str(&mut self, value: &str) {
        self.totals_row_label.set_value_string(value);
    }

    #[inline]
    #[allow(dead_code)]
    pub(crate) fn has_totals_row_function(&self) -> bool {
        self.totals_row_function.has_value()
    }

    #[inline]
    #[must_use]
    pub fn totals_row_function(&self) -> &TotalsRowFunctionValues {
        self.totals_row_function.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use totals_row_function()")]
    pub fn get_totals_row_function(&self) -> &TotalsRowFunctionValues {
        self.totals_row_function()
    }

    #[inline]
    pub(crate) fn totals_row_function_str(&self) -> &str {
        self.totals_row_function.value_string()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use totals_row_function_str()")]
    pub(crate) fn get_totals_row_function_str(&self) -> &str {
        self.totals_row_function_str()
    }

    #[inline]
    pub fn set_totals_row_function(&mut self, value: TotalsRowFunctionValues) {
        self.totals_row_function.set_value(value);
    }

    #[inline]
    pub(crate) fn set_totals_row_function_str(&mut self, value: &str) {
        self.totals_row_function.set_value_string(value);
    }

    #[inline]
    #[must_use]
    pub fn calculated_column_formula(&self) -> Option<&String> {
        self.calculated_column_formula.as_ref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use calculated_column_formula()")]
    pub fn get_calculated_column_formula(&self) -> Option<&String> {
        self.calculated_column_formula()
    }

    #[inline]
    pub(crate) fn set_calculated_column_formula(&mut self, value: String) {
        self.calculated_column_formula = Some(value);
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct TableStyleInfo {
    name:             String,
    show_first_col:   ShowColumn,
    show_last_col:    ShowColumn,
    show_row_stripes: ShowStripes,
    show_col_stripes: ShowStripes,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy)]
pub enum ShowColumn {
    Show,
    Hide,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy)]
pub enum ShowStripes {
    Show,
    Hide,
}

impl Default for TableStyleInfo {
    fn default() -> Self {
        Self {
            name:             String::new(), // Default name can be an empty string
            show_first_col:   ShowColumn::Hide, // Default to Hide
            show_last_col:    ShowColumn::Hide, // Default to Hide
            show_row_stripes: ShowStripes::Hide, // Default to Hide
            show_col_stripes: ShowStripes::Hide, // Default to Hide
        }
    }
}

impl TableStyleInfo {
    #[inline]
    #[must_use]
    pub fn new(
        name: &str,
        show_first_col: ShowColumn,
        show_last_col: ShowColumn,
        show_row_stripes: ShowStripes,
        show_col_stripes: ShowStripes,
    ) -> Self {
        Self {
            name: name.to_string(),
            show_first_col,
            show_last_col,
            show_row_stripes,
            show_col_stripes,
        }
    }

    #[inline]
    #[must_use]
    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use name()")]
    pub fn get_name(&self) -> &str {
        self.name()
    }

    #[inline]
    pub fn set_name(&mut self, name: &str) -> &mut Self {
        self.name = name.to_string();
        self
    }

    #[inline]
    #[must_use]
    pub fn is_show_first_col(&self) -> bool {
        matches!(self.show_first_col, ShowColumn::Show)
    }

    #[inline]
    #[must_use]
    pub fn is_show_last_col(&self) -> bool {
        matches!(self.show_last_col, ShowColumn::Show)
    }

    #[inline]
    #[must_use]
    pub fn is_show_row_stripes(&self) -> bool {
        matches!(self.show_row_stripes, ShowStripes::Show)
    }

    #[inline]
    #[must_use]
    pub fn is_show_col_stripes(&self) -> bool {
        matches!(self.show_col_stripes, ShowStripes::Show)
    }
}
