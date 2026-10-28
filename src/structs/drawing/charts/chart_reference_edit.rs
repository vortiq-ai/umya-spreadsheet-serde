//! Structural chart-reference edit types. Coordinates are one-based.

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChartReferenceEdit {
    RenameSheet { old: String, new: String },
    DeleteSheet { sheet: String, sheet_order: Vec<String> },
    InsertRows { sheet: String, at: u32, count: u32 },
    InsertColumns { sheet: String, at: u32, count: u32 },
    DeleteRows { sheet: String, rows: Vec<u32> },
    DeleteColumns { sheet: String, columns: Vec<u32> },
    MoveRange {
        source_sheet: String,
        source: ReferenceRect,
        destination_sheet: String,
        destination: ReferenceCell,
    },
}

/// Coordinates are one-based; no change to existing Worksheet API coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReferenceCell {
    pub column: u32,
    pub row: u32,
}

/// Both endpoints are inclusive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReferenceRect {
    pub first: ReferenceCell,
    pub last: ReferenceCell,
}

#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub struct ReferenceAdjustmentReport {
    pub visited: usize,
    pub changed: usize,
    pub invalidated: usize,
    pub preserved_opaque_extensions: usize,
}

/// Translate the legacy coordinate tuple once per structural operation.
/// None means out-of-grid input; Some(empty) is a no-op.
pub(crate) fn coordinate_edits(
    sheet: &str, root_col: u32, col_count: u32, root_row: u32, row_count: u32, remove: bool,
) -> Option<Vec<ChartReferenceEdit>> {
    if (col_count != 0 && (!(1..=16_384).contains(&root_col) || col_count > 16_384))
        || (row_count != 0 && (!(1..=1_048_576).contains(&root_row) || row_count > 1_048_576)) {
        return None;
    }
    let mut edits = Vec::with_capacity(2);
    if col_count != 0 {
        edits.push(if remove {
            ChartReferenceEdit::DeleteColumns { sheet: sheet.into(), columns: (root_col..root_col.saturating_add(col_count).min(16_385)).collect() }
        } else {
            ChartReferenceEdit::InsertColumns { sheet: sheet.into(), at: root_col, count: col_count }
        });
    }
    if row_count != 0 {
        edits.push(if remove {
            ChartReferenceEdit::DeleteRows { sheet: sheet.into(), rows: (root_row..root_row.saturating_add(row_count).min(1_048_577)).collect() }
        } else {
            ChartReferenceEdit::InsertRows { sheet: sheet.into(), at: root_row, count: row_count }
        });
    }
    Some(edits)
}

/// Internal context for one already-validated structural coordinate operation.
pub(crate) struct CoordinateChange<'a> {
    pub(crate) sheet: &'a str,
    pub(crate) root_col: u32,
    pub(crate) col_count: u32,
    pub(crate) root_row: u32,
    pub(crate) row_count: u32,
    pub(crate) remove: bool,
}
