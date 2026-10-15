// sheetFormatPr
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use super::{
    BooleanValue,
    ByteValue,
    DoubleValue,
    UInt32Value,
};
use crate::{
    reader::driver::{
        get_attribute,
        set_string_from_xml,
    },
    writer::driver::write_start_tag,
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct SheetFormatProperties {
    base_column_width:    UInt32Value,
    custom_height:        BooleanValue,
    default_column_width: DoubleValue,
    default_row_height:   DoubleValue,
    dy_descent:           DoubleValue,
    outline_level_column: ByteValue,
    outline_level_row:    ByteValue,
    thick_bottom:         BooleanValue,
    thick_top:            BooleanValue,
}

impl SheetFormatProperties {
    #[inline]
    #[must_use]
    pub fn base_column_width(&self) -> u32 {
        self.base_column_width.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use base_column_width()")]
    pub fn get_base_column_width(&self) -> u32 {
        self.base_column_width()
    }

    #[inline]
    pub fn set_base_column_width(&mut self, value: u32) -> &mut Self {
        self.base_column_width.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn custom_height(&self) -> bool {
        self.custom_height.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use custom_height()")]
    pub fn get_custom_height(&self) -> bool {
        self.custom_height()
    }

    #[inline]
    pub fn set_custom_height(&mut self, value: bool) -> &mut Self {
        self.custom_height.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn default_column_width(&self) -> f64 {
        self.default_column_width.value()
    }

    /// `defaultColWidth` as the sheet states it, or `None` when the sheet omits
    /// it.
    ///
    /// [`Self::default_column_width`] returns `0.0` for an absent attribute,
    /// which a caller cannot tell from a sheet that really does declare
    /// `defaultColWidth="0"`. The difference matters to anyone deciding
    /// whether a column inherits a sheet default or an application
    /// one: reading the `0` as a width collapses every column to nothing.
    #[inline]
    #[must_use]
    pub fn default_column_width_opt(&self) -> Option<f64> {
        self.default_column_width
            .has_value()
            .then(|| self.default_column_width.value())
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use default_column_width()")]
    pub fn get_default_column_width(&self) -> f64 {
        self.default_column_width()
    }

    #[inline]
    pub fn set_default_column_width(&mut self, value: f64) -> &mut Self {
        self.default_column_width.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn default_row_height(&self) -> f64 {
        self.default_row_height.value()
    }

    /// The declared default, or `None` when the worksheet omits it.
    #[inline]
    #[must_use]
    pub fn default_row_height_opt(&self) -> Option<f64> {
        self.default_row_height
            .has_value()
            .then(|| self.default_row_height.value())
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use default_row_height()")]
    pub fn get_default_row_height(&self) -> f64 {
        self.default_row_height()
    }

    #[inline]
    pub fn set_default_row_height(&mut self, value: f64) -> &mut Self {
        self.default_row_height.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn dy_descent(&self) -> f64 {
        self.dy_descent.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use dy_descent()")]
    pub fn get_dy_descent(&self) -> f64 {
        self.dy_descent()
    }

    #[inline]
    pub fn set_dy_descent(&mut self, value: f64) -> &mut Self {
        self.dy_descent.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn outline_level_column(&self) -> u8 {
        self.outline_level_column.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use outline_level_column()")]
    pub fn get_outline_level_column(&self) -> u8 {
        self.outline_level_column()
    }

    #[inline]
    pub fn set_outline_level_column(&mut self, value: u8) -> &mut Self {
        self.outline_level_column.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn outline_level_row(&self) -> u8 {
        self.outline_level_row.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use outline_level_row()")]
    pub fn get_outline_level_row(&self) -> u8 {
        self.outline_level_row()
    }

    #[inline]
    pub fn set_outline_level_row(&mut self, value: u8) -> &mut Self {
        self.outline_level_row.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn thick_bottom(&self) -> bool {
        self.thick_bottom.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use thick_bottom()")]
    pub fn get_thick_bottom(&self) -> bool {
        self.thick_bottom()
    }

    #[inline]
    pub fn set_thick_bottom(&mut self, value: bool) -> &mut Self {
        self.thick_bottom.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn thick_top(&self) -> bool {
        self.thick_top.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use thick_top()")]
    pub fn get_thick_top(&self) -> bool {
        self.thick_top()
    }

    #[inline]
    pub fn set_thick_top(&mut self, value: bool) -> &mut Self {
        self.thick_top.set_value(value);
        self
    }

    #[inline]
    pub(crate) fn set_defalut_value(&mut self) -> &mut Self {
        self.default_row_height.set_value(13.5);
        self.dy_descent.set_value(0.15);
        self
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        _reader: &mut Reader<R>,
        e: &BytesStart,
    ) {
        set_string_from_xml!(self, e, base_column_width, "baseColWidth");
        set_string_from_xml!(self, e, custom_height, "customHeight");
        set_string_from_xml!(self, e, default_column_width, "defaultColWidth");
        set_string_from_xml!(self, e, default_row_height, "defaultRowHeight");
        set_string_from_xml!(self, e, dy_descent, "dyDescent");
        set_string_from_xml!(self, e, outline_level_column, "outlineLevelCol");
        set_string_from_xml!(self, e, outline_level_row, "outlineLevelRow");
        set_string_from_xml!(self, e, thick_bottom, "thickBottom");
        set_string_from_xml!(self, e, thick_top, "thickTop");
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        // sheetFormatPr
        let mut attributes: crate::structs::AttrCollection = Vec::new();
        let str_base_column_width = self.base_column_width.value_string();
        if self.base_column_width.has_value() {
            attributes.push(("baseColWidth", &str_base_column_width).into());
        }

        let str_custom_height = self.custom_height.value_string();
        if self.custom_height.has_value() {
            attributes.push(("customHeight", str_custom_height).into());
        }

        let str_default_column_width = self.default_column_width.value_string();
        if self.default_column_width.has_value() {
            attributes.push(("defaultColWidth", &str_default_column_width).into());
        }

        // Always emit defaultRowHeight — Excel requires it.
        // Use 14.25 (Excel default) if the template didn't supply one.
        let str_default_row_height = if self.default_row_height.has_value() {
            self.default_row_height.value_string()
        } else {
            "14.25".to_string()
        };
        attributes.push(("defaultRowHeight", &str_default_row_height).into());

        // Always emit x14ac:dyDescent — Excel requires it.
        // Use 0.45 (Excel default) if the template didn't supply one.
        let str_dy_descent = if self.dy_descent.has_value() {
            self.dy_descent.value_string()
        } else {
            "0.45".to_string()
        };
        attributes.push(("x14ac:dyDescent", &str_dy_descent).into());

        let str_outline_level_column = self.outline_level_column.value_string();
        if self.outline_level_column.has_value() {
            attributes.push(("outlineLevelCol", &str_outline_level_column).into());
        }

        let str_outline_level_row = self.outline_level_row.value_string();
        if self.outline_level_row.has_value() {
            attributes.push(("outlineLevelRow", &str_outline_level_row).into());
        }

        let str_thick_bottom = self.thick_bottom.value_string();
        if self.thick_bottom.has_value() {
            attributes.push(("thickBottom", str_thick_bottom).into());
        }

        let str_thick_top = self.thick_top.value_string();
        if self.thick_top.has_value() {
            attributes.push(("thickTop", str_thick_top).into());
        }

        write_start_tag(writer, "sheetFormatPr", attributes, true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read_props(attributes: &str) -> SheetFormatProperties {
        let content = format!("sheetFormatPr {attributes}");
        let element = BytesStart::from_content(content, 14);
        let mut reader = Reader::from_str("");
        let mut props = SheetFormatProperties::default();
        props.set_attributes(&mut reader, &element);
        props
    }

    /// An absent `defaultColWidth` is not a width of zero.
    ///
    /// `default_column_width` folds both to `0.0`, so a caller deciding whether
    /// a column inherits a sheet default or an application one cannot tell
    /// them apart -- and reading the absent case as a width collapses every
    /// inheriting column to nothing.
    #[test]
    fn default_column_width_opt_separates_absent_from_zero() {
        assert_eq!(
            read_props(r#"defaultRowHeight="15" defaultColWidth="10.83""#)
                .default_column_width_opt(),
            Some(10.83),
            "a declared width is reported as declared"
        );
        assert_eq!(
            read_props(r#"defaultRowHeight="15""#).default_column_width_opt(),
            None,
            "an absent attribute is None, not Some(0.0)"
        );
        assert_eq!(
            read_props(r#"defaultRowHeight="15" defaultColWidth="0""#).default_column_width_opt(),
            Some(0.0),
            "a declared zero is still a declaration"
        );
        assert_eq!(
            read_props(r#"defaultRowHeight="15""#).default_column_width(),
            0.0,
            "the existing accessor keeps folding absent to zero"
        );
    }
}
