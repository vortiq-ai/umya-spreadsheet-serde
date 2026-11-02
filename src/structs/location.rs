use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use super::{
    StringValue,
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
pub struct Location {
    reference:         StringValue,
    first_header_row:  UInt32Value,
    first_data_row:    UInt32Value,
    first_data_col:    UInt32Value,
    #[cfg_attr(feature = "serde", serde(default))]
    row_page_count:    Option<u32>,
    #[cfg_attr(feature = "serde", serde(default))]
    column_page_count: Option<u32>,
}
impl Location {
    fn bounds(&self) -> Result<(u32, u32, u32, u32), String> {
        let (start, end) = self
            .reference()
            .split_once(':')
            .unwrap_or((self.reference(), self.reference()));
        let (left, top, ..) = crate::helper::coordinate::index_from_coordinate(start);
        let (right, bottom, ..) = crate::helper::coordinate::index_from_coordinate(end);
        match (left, top, right, bottom) {
            (Some(left), Some(top), Some(right), Some(bottom))
                if left > 0
                    && top > 0
                    && left <= right
                    && top <= bottom
                    && right <= 16_384
                    && bottom <= 1_048_576 =>
            {
                Ok((left, top, right, bottom))
            }
            _ => Err("invalid pivot location reference".into()),
        }
    }

    fn footprint(&self) -> Result<(u32, u32, u32, u32), String> {
        let (left, top, right, bottom) = self.bounds()?;
        match (self.row_page_count, self.column_page_count) {
            (None, None) | (Some(0), Some(0)) => Ok((left, top, right, bottom)),
            (Some(rows), Some(columns)) if rows > 0 && columns > 0 => {
                // Excel's TableRange2 includes one blank row below the page
                // fields and one blank column between their two-cell pairs.
                let top = top
                    .checked_sub(rows)
                    .and_then(|value| value.checked_sub(1))
                    .filter(|value| *value > 0)
                    .ok_or("invalid pivot page-field rows")?;
                let page_right = columns
                    .checked_mul(3)
                    .and_then(|value| value.checked_sub(2))
                    .and_then(|value| left.checked_add(value))
                    .filter(|value| *value <= 16_384)
                    .ok_or("invalid pivot page-field columns")?;
                Ok((left, top, right.max(page_right), bottom))
            }
            _ => Err("incomplete pivot page-field geometry".into()),
        }
    }

    fn geometry_edit_is_after(&self, col: u32, cols: u32, row: u32, rows: u32) -> bool {
        if self.reference().is_empty() {
            return true;
        }
        let Ok((left, _, right, bottom)) = self.bounds() else {
            return false;
        };
        let page_right = self
            .column_page_count
            .unwrap_or(0)
            .saturating_mul(3)
            .saturating_sub(2)
            .saturating_add(left);
        (cols == 0 || col > right.max(page_right)) && (rows == 0 || row > bottom)
    }

    fn validate_edit(root: u32, count: u32, limit: u32) -> Result<(), String> {
        if count != 0 && (root == 0 || root > limit || count > limit - root + 1) {
            return Err("pivot structural edit is outside the worksheet grid".into());
        }
        Ok(())
    }

    /// Preflight before moving cells. A partial edit of the report, including
    /// its page fields, is refused by Excel. Offsets within the body stay
    /// fixed.
    pub(crate) fn insert_geometry(
        &mut self,
        root_col: u32,
        col_count: u32,
        root_row: u32,
        row_count: u32,
    ) -> Result<(), String> {
        Self::validate_edit(root_col, col_count, 16_384)?;
        Self::validate_edit(root_row, row_count, 1_048_576)?;
        if self.geometry_edit_is_after(root_col, col_count, root_row, row_count) {
            return Ok(());
        }
        let (left, top, right, bottom) = self.footprint()?;
        if (col_count != 0 && left < root_col && root_col <= right)
            || (row_count != 0 && top < root_row && root_row <= bottom)
        {
            return Err("cannot insert through part of a pivot table".into());
        }
        let col_offset = if col_count != 0 && root_col <= left {
            col_count
        } else {
            0
        };
        let row_offset = if row_count != 0 && root_row <= top {
            row_count
        } else {
            0
        };
        if right + col_offset > 16_384 || bottom + row_offset > 1_048_576 {
            return Err("pivot table would move outside the worksheet grid".into());
        }
        self.translate(i64::from(col_offset), i64::from(row_offset))
    }

    /// Returns true when the whole report must be removed by its owner.
    /// Removal and refusal leave this model unchanged.
    pub(crate) fn remove_geometry(
        &mut self,
        root_col: u32,
        col_count: u32,
        root_row: u32,
        row_count: u32,
    ) -> Result<bool, String> {
        Self::validate_edit(root_col, col_count, 16_384)?;
        Self::validate_edit(root_row, row_count, 1_048_576)?;
        if self.geometry_edit_is_after(root_col, col_count, root_row, row_count) {
            return Ok(false);
        }
        let (left, top, right, bottom) = self.footprint()?;
        let col_end = u64::from(root_col) + u64::from(col_count);
        let row_end = u64::from(root_row) + u64::from(row_count);
        if (col_count != 0 && root_col <= left && col_end > u64::from(right))
            || (row_count != 0 && root_row <= top && row_end > u64::from(bottom))
        {
            return Ok(true);
        }
        if (col_count != 0 && root_col <= right && col_end > u64::from(left))
            || (row_count != 0 && root_row <= bottom && row_end > u64::from(top))
        {
            return Err("cannot delete part of a pivot table".into());
        }
        let col_offset = if col_count != 0 && col_end <= u64::from(left) {
            -i64::from(col_count)
        } else {
            0
        };
        let row_offset = if row_count != 0 && row_end <= u64::from(top) {
            -i64::from(row_count)
        } else {
            0
        };
        self.translate(col_offset, row_offset)?;
        Ok(false)
    }

    fn translate(&mut self, col_offset: i64, row_offset: i64) -> Result<(), String> {
        if col_offset == 0 && row_offset == 0 {
            return Ok(());
        }
        let (left, top, right, bottom) = self.bounds()?;
        let coordinate = |column: u32, row: u32| -> Result<String, String> {
            let column = u32::try_from(i64::from(column) + col_offset)
                .map_err(|_| "pivot column translation is outside the worksheet grid")?;
            let row = u32::try_from(i64::from(row) + row_offset)
                .map_err(|_| "pivot row translation is outside the worksheet grid")?;
            Ok(crate::helper::coordinate::coordinate_from_index(
                column, row,
            ))
        };
        self.set_reference(format!(
            "{}:{}",
            coordinate(left, top)?,
            coordinate(right, bottom)?
        ));
        Ok(())
    }

    #[must_use]
    pub fn reference(&self) -> &str {
        self.reference.value_str()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use reference()")]
    pub fn get_reference(&self) -> &str {
        self.reference()
    }

    pub fn set_reference<S: Into<String>>(&mut self, value: S) -> &mut Self {
        self.reference.set_value(value);
        self
    }

    #[must_use]
    pub fn first_header_row(&self) -> u32 {
        self.first_header_row.value()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use first_header_row()")]
    pub fn get_first_header_row(&self) -> u32 {
        self.first_header_row()
    }

    pub fn set_first_header_row(&mut self, value: u32) -> &mut Self {
        self.first_header_row.set_value(value);
        self
    }

    #[must_use]
    pub fn first_data_row(&self) -> u32 {
        self.first_data_row.value()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use first_data_row()")]
    pub fn get_first_data_row(&self) -> u32 {
        self.first_data_row()
    }

    pub fn set_first_data_row(&mut self, value: u32) -> &mut Self {
        self.first_data_row.set_value(value);
        self
    }

    #[must_use]
    pub fn first_data_col(&self) -> u32 {
        self.first_data_col.value()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use first_data_col()")]
    pub fn get_first_data_col(&self) -> u32 {
        self.first_data_col()
    }

    pub fn set_first_data_col(&mut self, value: u32) -> &mut Self {
        self.first_data_col.set_value(value);
        self
    }

    /// Number of rows occupied by report-filter fields (`rowPageCount`).
    #[must_use]
    pub fn row_page_count(&self) -> Option<u32> {
        self.row_page_count
    }

    pub fn set_row_page_count(&mut self, value: u32) -> &mut Self {
        self.row_page_count = Some(value);
        self
    }

    pub fn remove_row_page_count(&mut self) -> &mut Self {
        self.row_page_count = None;
        self
    }

    /// Number of columns of report-filter fields (`colPageCount`). Each field
    /// occupies two worksheet columns, with a blank column between fields.
    #[must_use]
    pub fn column_page_count(&self) -> Option<u32> {
        self.column_page_count
    }

    pub fn set_column_page_count(&mut self, value: u32) -> &mut Self {
        self.column_page_count = Some(value);
        self
    }

    pub fn remove_column_page_count(&mut self) -> &mut Self {
        self.column_page_count = None;
        self
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        _reader: &mut Reader<R>,
        e: &BytesStart,
    ) {
        set_string_from_xml!(self, e, reference, "ref");
        set_string_from_xml!(self, e, first_header_row, "firstHeaderRow");
        set_string_from_xml!(self, e, first_data_row, "firstDataRow");
        set_string_from_xml!(self, e, first_data_col, "firstDataCol");
        self.row_page_count = get_attribute(e, b"rowPageCount").and_then(|v| v.trim().parse().ok());
        self.column_page_count =
            get_attribute(e, b"colPageCount").and_then(|v| v.trim().parse().ok());
    }

    #[allow(dead_code)]
    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        // location
        let mut attributes: crate::structs::AttrCollection = vec![
            ("ref", self.reference.value_str()).into(),
            ("firstHeaderRow", &self.first_header_row.value_string()).into(),
            ("firstDataRow", &self.first_data_row.value_string()).into(),
            ("firstDataCol", &self.first_data_col.value_string()).into(),
        ];
        if let Some(value) = self.row_page_count {
            attributes.push(("rowPageCount", value.to_string()).into());
        }
        if let Some(value) = self.column_page_count {
            attributes.push(("colPageCount", value.to_string()).into());
        }
        write_start_tag(writer, "location", attributes, true);
    }
}
