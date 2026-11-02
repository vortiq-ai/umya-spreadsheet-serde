use super::{
    EditAsValues,
    Extent,
    MarkerType,
};
use crate::{
    drawing::Point2DType,
    structs::Worksheet,
};

/// The grid before and after a structural edit. Sparse sizes keep placement
/// independent of the number of empty rows preceding a drawing.
pub(crate) struct DrawingGrid {
    columns: GridAxis,
    rows:    GridAxis,
}

struct GridAxis {
    default: i64,
    sizes:   Vec<(u32, i64)>,
    /// `shifts[n]` is the summed difference from `default` of the first `n`
    /// sizes, so a start is one binary search away.
    shifts:  Vec<i64>,
    limit:   u32,
}

impl GridAxis {
    fn new(default: i64, mut sizes: Vec<(u32, i64)>, limit: u32) -> Self {
        sizes.sort_unstable_by_key(|(index, _)| *index);
        let mut shifts = Vec::with_capacity(sizes.len() + 1);
        shifts.push(0_i64);
        let mut total = 0_i64;
        for (_, size) in &sizes {
            total = total.saturating_add(size.saturating_sub(default));
            shifts.push(total);
        }
        Self {
            default,
            sizes,
            shifts,
            limit,
        }
    }

    fn start(&self, index: u32) -> i64 {
        let before = self.sizes.partition_point(|(at, _)| *at < index);
        i64::from(index)
            .saturating_mul(self.default)
            .saturating_add(self.shifts[before])
    }

    fn position(&self, marker: &MarkerType, column: bool) -> i64 {
        if column {
            self.start(marker.col())
                .saturating_add(i64::from(marker.col_off()))
        } else {
            self.start(marker.row())
                .saturating_add(i64::from(marker.row_off()))
        }
    }

    fn set_position(&self, marker: &mut MarkerType, column: bool, position: i64) {
        let position = position.max(0);
        let mut low = 0;
        let mut high = self.limit;
        while low + 1 < high {
            let middle = low + (high - low) / 2;
            if self.start(middle) <= position {
                low = middle;
            } else {
                high = middle;
            }
        }
        let offset = i32::try_from(position.saturating_sub(self.start(low))).unwrap_or(i32::MAX);
        if column {
            marker.set_col(low).set_col_off(offset);
        } else {
            marker.set_row(low).set_row_off(offset);
        }
    }
}

/// A worksheet size in EMUs, clamped to Excel's limits so a NaN, infinite or
/// huge stored value cannot overflow the placement arithmetic.
fn size_emu(value: f64, pixels_per_unit: f64, limit: f64) -> i64 {
    let value = if value.is_nan() {
        0.0
    } else {
        value.clamp(0.0, limit)
    };
    num_traits::cast::<_, i64>((value * pixels_per_unit).round())
        .unwrap_or(0)
        .saturating_mul(9525)
}

impl DrawingGrid {
    /// The physical `((x, width), (y, height))` of an anchored object.
    pub(crate) fn rectangle(&self, anchor: &DrawingAnchor) -> ((i64, i64), (i64, i64)) {
        let (x, y, width, height) = match anchor {
            DrawingAnchor::TwoCell { from, to, .. } => {
                let x = self.columns.position(from, true);
                let y = self.rows.position(from, false);
                (
                    x,
                    y,
                    self.columns.position(to, true).saturating_sub(x),
                    self.rows.position(to, false).saturating_sub(y),
                )
            }
            DrawingAnchor::OneCell { from, extent } => (
                self.columns.position(from, true),
                self.rows.position(from, false),
                extent.cx(),
                extent.cy(),
            ),
            DrawingAnchor::Absolute { position, extent } => {
                (position.x(), position.y(), extent.cx(), extent.cy())
            }
        };
        ((x, width), (y, height))
    }

    pub(crate) fn new(sheet: &Worksheet) -> Self {
        // Excel stores column widths in default-digit units (at most 255) and
        // row heights in points (at most 409.5). Round to pixels before
        // converting to EMUs.
        let column_size = |width: f64| size_emu(width, 7.0, 255.0);
        let row_size = |height: f64| size_emu(height, 4.0 / 3.0, 409.5);
        let default_column = sheet
            .sheet_format_properties()
            .default_column_width_opt()
            .map_or(64 * 9525, column_size);
        let default_row = row_size(
            sheet
                .sheet_format_properties()
                .effective_default_row_height(),
        );
        let columns = sheet
            .column_dimensions()
            .iter()
            .filter_map(|column| {
                let size = if column.hidden() {
                    0
                } else {
                    column_size(column.width())
                };
                (size != default_column).then_some((column.col_num().saturating_sub(1), size))
            })
            .collect();
        let rows = sheet
            .row_dimensions()
            .into_iter()
            .filter_map(|row| {
                let size = if row.hidden() {
                    0
                } else {
                    row.height_opt().map_or(default_row, row_size)
                };
                (size != default_row).then_some((row.row_num().saturating_sub(1), size))
            })
            .collect();
        Self {
            columns: GridAxis::new(default_column, columns, 16_384),
            rows:    GridAxis::new(default_row, rows, 1_048_576),
        }
    }
}

pub(crate) struct DrawingEdit<'a> {
    pub(crate) before:  &'a DrawingGrid,
    pub(crate) after:   &'a DrawingGrid,
    pub(crate) insert:  bool,
    pub(crate) column:  u32,
    pub(crate) columns: u32,
    pub(crate) row:     u32,
    pub(crate) rows:    u32,
}

impl DrawingEdit<'_> {
    fn axis(
        &self,
        from: &mut MarkerType,
        to: &mut MarkerType,
        mode: &EditAsValues,
        column: bool,
    ) -> bool {
        let (root, count, before, after) = if column {
            (
                self.column,
                self.columns,
                &self.before.columns,
                &self.after.columns,
            )
        } else {
            (self.row, self.rows, &self.before.rows, &self.after.rows)
        };
        if root == 0 || count == 0 {
            return true;
        }
        let start = before.position(from, column);
        let end = before.position(to, column);
        match mode {
            EditAsValues::Absolute => {
                after.set_position(from, column, start);
                after.set_position(to, column, end);
            }
            EditAsValues::OneCell => {
                from.adjust_axis(column, self.insert, root, count);
                let moved = after.position(from, column);
                after.set_position(to, column, moved.saturating_add(end.saturating_sub(start)));
            }
            EditAsValues::TwoCell => {
                if !self.insert
                    && start >= before.start(root - 1)
                    && end <= before.start((root - 1).saturating_add(count))
                {
                    return false;
                }
                from.adjust_axis(column, self.insert, root, count);
                if end > start {
                    to.adjust_end_axis(column, self.insert, root, count);
                } else {
                    to.adjust_axis(column, self.insert, root, count);
                }
            }
        }
        true
    }

    pub(crate) fn two_cell(
        &self,
        from: &mut MarkerType,
        to: &mut MarkerType,
        mode: &EditAsValues,
    ) -> bool {
        self.axis(from, to, mode, true) && self.axis(from, to, mode, false)
    }

    pub(crate) fn one_cell(&self, from: &mut MarkerType) {
        from.adjust_axis(true, self.insert, self.column, self.columns);
        from.adjust_axis(false, self.insert, self.row, self.rows);
    }

    pub(crate) fn anchor(&self, anchor: &mut DrawingAnchor) -> bool {
        match anchor {
            DrawingAnchor::TwoCell { from, to, edit_as } => {
                self.two_cell(from, to, edit_as.as_ref().unwrap_or(&EditAsValues::TwoCell))
            }
            DrawingAnchor::OneCell { from, .. } => {
                self.one_cell(from);
                true
            }
            DrawingAnchor::Absolute { .. } => true,
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub enum DrawingAnchor {
    TwoCell {
        from:    MarkerType,
        to:      MarkerType,
        edit_as: Option<EditAsValues>,
    },
    OneCell {
        from:   MarkerType,
        extent: Extent,
    },
    Absolute {
        position: Point2DType,
        extent:   Extent,
    },
}

impl Default for DrawingAnchor {
    fn default() -> Self {
        Self::TwoCell {
            from:    MarkerType::default(),
            to:      MarkerType::default(),
            edit_as: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn marker(column: u32, row: u32) -> MarkerType {
        let mut marker = MarkerType::default();
        marker
            .set_col(column)
            .set_row(row)
            .set_col_off(17)
            .set_row_off(29);
        marker
    }

    #[test]
    fn sizes_clamp_to_excels_limits_without_panicking() {
        assert_eq!(size_emu(f64::NAN, 7.0, 255.0), 0);
        assert_eq!(size_emu(-3.0, 7.0, 255.0), 0);
        assert_eq!(size_emu(f64::INFINITY, 7.0, 255.0), 1785 * 9525);
        assert_eq!(size_emu(f64::MAX, 4.0 / 3.0, 409.5), 546 * 9525);
        assert_eq!(size_emu(8.43, 7.0, 255.0), 59 * 9525);
    }

    #[test]
    fn starts_add_each_size_that_differs_from_the_default() {
        let axis = GridAxis::new(10, vec![(5, 4), (2, 0), (7, 30)], 100);
        assert_eq!(axis.start(2), 20);
        assert_eq!(axis.start(3), 30 - 10);
        assert_eq!(axis.start(6), 60 - 10 - 6);
        assert_eq!(axis.start(8), 80 - 10 - 6 + 20);
        let mut at = marker(0, 0);
        axis.set_position(&mut at, false, 62);
        assert_eq!((at.row(), at.row_off()), (7, 8));
    }

    #[cfg(feature = "serde")]
    #[test]
    fn every_geometry_kind_roundtrips_without_a_chart_payload() {
        for anchor in [
            DrawingAnchor::default(),
            DrawingAnchor::OneCell {
                from:   marker(2, 3),
                extent: Extent::default(),
            },
            DrawingAnchor::Absolute {
                position: Point2DType::default(),
                extent:   Extent::default(),
            },
        ] {
            let json = serde_json::to_value(&anchor).unwrap();
            let restored: DrawingAnchor = serde_json::from_value(json.clone()).unwrap();
            assert_eq!(serde_json::to_value(restored).unwrap(), json);
            let bytes = bincode::serialize(&anchor).unwrap();
            let restored: DrawingAnchor = bincode::deserialize(&bytes).unwrap();
            assert_eq!(serde_json::to_value(restored).unwrap(), json);
        }
    }
}
