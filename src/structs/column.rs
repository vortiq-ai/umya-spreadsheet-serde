use quick_xml::{
    Reader,
    events::BytesStart,
};

use super::{
    BooleanValue,
    DoubleValue,
    Style,
    Stylesheet,
    UInt32Value,
};
use crate::{
    reader::driver::{
        get_attribute,
        set_string_from_xml,
    },
    structs::Cells,
    traits::AdjustmentValue,
};

/// # Examples
/// ## set auto width
/// ```rust
/// use umya_spreadsheet::*;
/// let mut book = new_file();
/// let mut worksheet = book.sheet_by_name_mut("Sheet1").unwrap();
/// worksheet.column_dimension_mut("A").set_auto_width(true);
/// ```
/// ## set manual width
/// ```rust
/// use umya_spreadsheet::*;
/// let mut book = new_file();
/// let mut worksheet = book.sheet_by_name_mut("Sheet1").unwrap();
/// worksheet.column_dimension_mut("A").set_width(60f64);
/// ```
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct Column {
    col_num:                 UInt32Value,
    pub(crate) width:        DoubleValue,
    pub(crate) hidden:       BooleanValue,
    pub(crate) best_fit:     BooleanValue,
    style:                   Box<Style>,
    auto_width:              BooleanValue,
    /// The `customWidth` attribute: whether `width` was set rather than
    /// inherited.
    pub(crate) custom_width: BooleanValue,
}

impl Default for Column {
    #[inline]
    fn default() -> Self {
        let mut width = DoubleValue::default();
        width.set_value(8.38f64);
        Self {
            col_num: UInt32Value::default(),
            width,
            hidden: BooleanValue::default(),
            best_fit: BooleanValue::default(),
            style: Box::new(Style::default()),
            auto_width: BooleanValue::default(),
            custom_width: BooleanValue::default(),
        }
    }
}

impl Column {
    #[inline]
    #[must_use]
    pub fn col_num(&self) -> u32 {
        self.col_num.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use col_num()")]
    pub fn get_col_num(&self) -> u32 {
        self.col_num()
    }

    #[inline]
    pub fn set_col_num(&mut self, value: u32) -> &mut Self {
        self.col_num.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn width(&self) -> f64 {
        self.width.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use width()")]
    pub fn get_width(&self) -> f64 {
        self.width()
    }

    #[inline]
    pub fn set_width(&mut self, value: f64) -> &mut Self {
        self.width.set_value(value);
        // Setting a width IS authoring it -- mirrors `Row::set_height`, which
        // has set `custom_height` all along. Without this, making the
        // writer conditional would regress every programmatic caller:
        // the width would be written but marked inherited, and Excel
        // would autofit over it.
        self.custom_width.set_value(true);
        self
    }

    #[inline]
    #[must_use]
    pub fn hidden(&self) -> bool {
        self.hidden.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use hidden()")]
    pub fn get_hidden(&self) -> bool {
        self.hidden()
    }

    #[inline]
    pub fn set_hidden(&mut self, value: bool) -> &mut Self {
        self.hidden.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn best_fit(&self) -> bool {
        self.best_fit.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use best_fit()")]
    pub fn get_best_fit(&self) -> bool {
        self.best_fit()
    }

    #[inline]
    pub fn set_best_fit(&mut self, value: bool) -> &mut Self {
        self.best_fit.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn style(&self) -> &Style {
        &self.style
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use style()")]
    pub fn get_style(&self) -> &Style {
        self.style()
    }

    #[inline]
    pub fn style_mut(&mut self) -> &mut Style {
        &mut self.style
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use style()")]
    pub fn get_style_mut(&mut self) -> &mut Style {
        self.style_mut()
    }

    #[inline]
    pub fn set_style(&mut self, value: Style) -> &mut Self {
        *self.style = value;
        self
    }

    /// Whether this column's width was authored rather than inherited from the
    /// sheet default.
    ///
    /// A `<cols>` entry covers a range, and a sheet commonly closes the run
    /// with one span reaching column 16384 whose only purpose is to carry
    /// `defaultColWidth`; that span sets `customWidth="false"`. Reading
    /// `width` alone cannot separate those columns from authored
    /// ones, because Excel writes `width` rounded to two decimals while
    /// `defaultColWidth` keeps full precision. Absent from the XML this
    /// reads `false`, matching the OOXML default.
    #[inline]
    #[must_use]
    pub fn custom_width(&self) -> bool {
        self.custom_width.value()
    }

    /// Mark this column's width as authored, or as inherited from the sheet
    /// default.
    ///
    /// `set_width` already marks it authored, which is what a caller almost
    /// always wants. This is the escape hatch for the other case: setting a
    /// width while leaving Excel free to autofit it. Mirrors
    /// `Row::set_custom_height`.
    #[inline]
    pub fn set_custom_width(&mut self, value: bool) -> &mut Self {
        self.custom_width.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn auto_width(&self) -> bool {
        self.auto_width.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use auto_width()")]
    pub fn get_auto_width(&self) -> bool {
        self.auto_width()
    }

    #[inline]
    pub fn set_auto_width(&mut self, value: bool) -> &mut Self {
        self.auto_width.set_value(value);
        self
    }

    pub(crate) fn calculation_auto_width(&mut self, cells: &Cells) -> &mut Self {
        if !self.auto_width() {
            return self;
        }

        let mut column_width_max = 0f64;

        // default font size len.
        let column_font_size = match self.style().font() {
            Some(font) => font.font_size().val(),
            None => 11f64,
        };

        for cell in cells.iter_cells_by_column(self.col_num()) {
            let column_width = cell.width_point(column_font_size);

            if column_width > column_width_max {
                column_width_max = column_width;
            }
        }

        // set default width if empty column.
        if column_width_max == 0f64 {
            column_width_max = 8.38f64;
        }

        self.set_width(column_width_max);
        self
    }

    #[inline]
    pub(crate) fn has_style(&self) -> bool {
        *self.style != Style::default()
    }

    #[inline]
    pub(crate) fn hash_code(&self) -> String {
        // 🚨 Every attribute `Columns::write_to` emits must appear here. This
        // hash is what decides whether adjacent columns coalesce into a
        // single `<col min= max=>` run, and a run is written from the
        // FIRST column of the group -- so an attribute that varies
        // within a group but is missing from the hash gets silently overwritten
        // with its neighbour's value. `custom_width` was safe to omit
        // only while it was hardcoded.
        crate::helper::utils::md5_hash(format!(
            "{}{}{}{}",
            self.width.value_string(),
            self.hidden.value_string(),
            self.best_fit.value_string(),
            self.custom_width.value_string(),
        ))
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use hash_code()")]
    pub(crate) fn get_hash_code(&self) -> String {
        self.hash_code()
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        _reader: &mut Reader<R>,
        e: &BytesStart,
        stylesheet: &Stylesheet,
    ) {
        set_string_from_xml!(self, e, width, "width");
        set_string_from_xml!(self, e, hidden, "hidden");
        set_string_from_xml!(self, e, best_fit, "bestFit");
        set_string_from_xml!(self, e, custom_width, "customWidth");

        // Excel ignores a column style it cannot read as an index.
        if let Some(index) = get_attribute(e, b"style").and_then(|v| v.parse::<usize>().ok()) {
            let style = stylesheet.style(index);
            self.set_style(style);
        }
    }
}
impl AdjustmentValue for Column {
    #[inline]
    fn adjustment_insert_value(&mut self, root_num: u32, offset_num: u32) {
        if self.col_num.value() >= root_num {
            self.col_num.set_value(self.col_num.value() + offset_num);
        }
    }

    #[inline]
    fn adjustment_remove_value(&mut self, root_num: u32, offset_num: u32) {
        if self.col_num.value() >= root_num {
            self.col_num.set_value(self.col_num.value() - offset_num);
        }
    }

    #[inline]
    fn is_remove_value(&self, root_num: u32, offset_num: u32) -> bool {
        self.col_num.value() >= root_num && self.col_num.value() < root_num + offset_num
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read_col(attributes: &str) -> Column {
        let content = format!("col {attributes}");
        let element = BytesStart::from_content(content, 3);
        let mut reader = Reader::from_str("");
        let mut column = Column::default();
        column.set_attributes(&mut reader, &element, &Stylesheet::default());
        column
    }

    /// `customWidth` distinguishes an authored width from an inherited one.
    ///
    /// A `<cols>` run is a set of ranges, and a sheet commonly closes it with
    /// one span reaching column 16384 that exists only to carry
    /// `defaultColWidth`. That span says `customWidth="false"`. Without the
    /// attribute a reader cannot tell those columns from real ones: `width`
    /// is written rounded to two decimals while `defaultColWidth` keeps full
    /// precision, so comparing the two does not separate them either.
    #[test]
    fn custom_width_reports_whether_the_width_was_authored() {
        assert!(
            read_col(r#"min="1" max="1" width="18.17" customWidth="true""#).custom_width(),
            "an authored width is marked custom"
        );
        assert!(
            !read_col(r#"min="1" max="16384" width="10.83" customWidth="false""#).custom_width(),
            "a span carrying the sheet default is not custom"
        );
        assert!(
            !read_col(r#"min="1" max="1" width="18.17""#).custom_width(),
            "absent from the XML it reads false, matching the OOXML default"
        );
    }

    /// The width itself is still read in every case; `custom_width` only
    /// qualifies it.
    #[test]
    fn reading_custom_width_leaves_the_other_attributes_alone() {
        let column = read_col(r#"min="1" max="1" width="18.17" customWidth="true" hidden="true""#);
        assert!((column.width() - 18.17).abs() < 1e-9);
        assert!(column.hidden());
        assert!(column.custom_width());
    }
}
