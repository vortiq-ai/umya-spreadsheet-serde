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
