//! Lossless chart-reference editing. Caller edit validation belongs to
//! preparation; prepared leaf decisions are total and preserve malformed text.
use super::{ChartReferenceEdit, ReferenceCell, ReferenceRect};
use crate::helper::{
    address::{is_address, quote_sheet_name, split_address, unescape_sheet_name},
    coordinate::{index_from_coordinate, string_from_column_index},
    formula::{chart_formula_operands, ChartFormulaOperand},
};

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct FormulaEdit {
    pub text: String,
    pub invalidated: usize,
}
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum UnresolvedEdit {
    InvalidCoordinate,
}

fn sheet_matches(left: &str, right: &str) -> bool {
    left.eq_ignore_ascii_case(right) || left.to_lowercase() == right.to_lowercase()
}

/// A single unlocked A1-looking token outside Excel's grid can be a defined
/// name (XFE1, A1048577, A0), not a malformed cell. Keep that lexical identity
/// for the workbook's named-reference owner. `$` and `:` are not name chars.
fn is_out_of_grid_name_operand(range: &str) -> bool {
    if range.contains(['$', ':']) || !is_address(range.to_ascii_uppercase()) { return false; }
    let (column, row, _, _) = index_from_coordinate(range.to_ascii_uppercase());
    column.is_none_or(|value| value == 0 || value > 16_384)
        || row.is_none_or(|value| value == 0 || value > 1_048_576)
}

pub(crate) struct PreparedReferenceEdit<'a> {
    edit: &'a ChartReferenceEdit,
    deleted: Vec<u32>,
    deleted_count: u32,
}
impl<'a> PreparedReferenceEdit<'a> {
    pub(crate) fn new(edit: &'a ChartReferenceEdit) -> Result<Self, UnresolvedEdit> {
        let mut deleted = match edit {
            ChartReferenceEdit::DeleteRows { rows, .. } => rows.clone(),
            ChartReferenceEdit::DeleteColumns { columns, .. } => columns.clone(),
            ChartReferenceEdit::RenameSheet { .. } | ChartReferenceEdit::DeleteSheet { .. }
            | ChartReferenceEdit::InsertRows { .. } | ChartReferenceEdit::InsertColumns { .. }
            | ChartReferenceEdit::MoveRange { .. } => Vec::new(),
        };
        if deleted.contains(&0) { return Err(UnresolvedEdit::InvalidCoordinate); }
        deleted.sort_unstable(); deleted.dedup();
        match edit {
            ChartReferenceEdit::InsertRows { at, count, .. } | ChartReferenceEdit::InsertColumns { at, count, .. } if *at == 0 && *count != 0 => return Err(UnresolvedEdit::InvalidCoordinate),
            ChartReferenceEdit::InsertRows { at, count, .. } if *count != 0 && (*at > 1_048_576 || *count > 1_048_576 || *count > 1_048_576 - *at + 1) => return Err(UnresolvedEdit::InvalidCoordinate),
            ChartReferenceEdit::InsertColumns { at, count, .. } if *count != 0 && (*at > 16_384 || *count > 16_384 || *count > 16_384 - *at + 1) => return Err(UnresolvedEdit::InvalidCoordinate),
            ChartReferenceEdit::DeleteRows { .. } if deleted.iter().any(|v| *v > 1_048_576) => return Err(UnresolvedEdit::InvalidCoordinate),
            ChartReferenceEdit::DeleteColumns { .. } if deleted.iter().any(|v| *v > 16_384) => return Err(UnresolvedEdit::InvalidCoordinate),
            ChartReferenceEdit::MoveRange { source, destination, .. } => {
                if source.first.column == 0 || source.first.row == 0 || source.first.column > source.last.column || source.first.row > source.last.row
                    || source.last.column > 16_384 || source.last.row > 1_048_576 || destination.column == 0 || destination.row == 0
                    || destination.column.checked_add(source.last.column - source.first.column).is_none_or(|v| v > 16_384)
                    || destination.row.checked_add(source.last.row - source.first.row).is_none_or(|v| v > 1_048_576) {
                    return Err(UnresolvedEdit::InvalidCoordinate);
                }
            }
            ChartReferenceEdit::DeleteSheet { sheet, sheet_order } => {
                let mut names = std::collections::HashSet::new();
                if !sheet_order.iter().any(|name| sheet_matches(name, sheet))
                    || sheet_order.iter().any(|name| name.is_empty() || !names.insert(name.to_lowercase())) {
                    return Err(UnresolvedEdit::InvalidCoordinate);
                }
            }
            ChartReferenceEdit::RenameSheet { .. } | ChartReferenceEdit::InsertRows { .. }
            | ChartReferenceEdit::InsertColumns { .. } | ChartReferenceEdit::DeleteRows { .. }
            | ChartReferenceEdit::DeleteColumns { .. } => (),
        }
        // The normalized unique native indices fit the selected grid axis.
        let deleted_count = deleted.iter().fold(0_u32, |count, _| count + 1);
        Ok(Self { edit, deleted, deleted_count })
    }
}

/// Only caller edit preparation can fail. Formula text, including malformed
/// imported operands, does not add a chart- or workbook-wide refusal channel.
pub(crate) fn edit_formula_text(input: &str, host: &str, edit: &ChartReferenceEdit, bare_error: bool) -> Result<FormulaEdit, UnresolvedEdit> {
    Ok(edit_prepared_formula_text(input, host, &PreparedReferenceEdit::new(edit)?, bare_error))
}

pub(crate) fn edit_prepared_formula_text(input: &str, host: &str, prepared: &PreparedReferenceEdit<'_>, bare_error: bool) -> FormulaEdit {
    edit_prepared_formula_text_with_context(input, host, prepared, bare_error, false)
}

/// Use only for a cell transported by the prepared `MoveRange`. Out-of-domain
/// invocation (not `MoveRange` or not its source host) has the same explicit
/// no-change policy as an invalid direct Chart edit; it is not a leaf fallback.
pub(crate) fn edit_prepared_formula_text_for_moved_cell(input: &str, original_host: &str, prepared: &PreparedReferenceEdit<'_>, bare_error: bool) -> FormulaEdit {
    let valid_context = matches!(prepared.edit, ChartReferenceEdit::MoveRange { source_sheet, .. } if sheet_matches(original_host, source_sheet));
    if !valid_context { return FormulaEdit { text: input.to_owned(), invalidated: 0 }; }
    edit_prepared_formula_text_with_context(input, original_host, prepared, bare_error, true)
}

/// Classify with the existing coordinate grammar, then enforce native bounds.
/// Names and malformed anchored/compound fragments stay lexical. In particular
/// a numeric suffix too large for u32 is not an omitted whole-axis dimension.
fn is_native_coordinate_operand(range: &str) -> bool {
    if is_out_of_grid_name_operand(range) { return false; }
    let upper = range.to_ascii_uppercase();
    let parts: Vec<_> = upper.split(':').collect();
    let cells = is_address(&upper);
    let columns = parts.len() == 2 && parts.iter().all(|part| {
        let letters = part.trim_start_matches('$');
        !letters.is_empty() && letters.bytes().all(|byte| byte.is_ascii_alphabetic())
    });
    let rows = parts.len() == 2 && parts.iter().all(|part| {
        let digits = part.trim_start_matches('$');
        !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit())
    });
    if !cells && !columns && !rows { return false; }
    parts.into_iter().all(|part| {
        let (column, row, _, _) = index_from_coordinate(part);
        let valid_column = column.is_some_and(|value| value != 0 && value <= 16_384);
        let valid_row = row.is_some_and(|value| value != 0 && value <= 1_048_576);
        if cells { valid_column && valid_row }
        else if columns { valid_column && row.is_none() }
        else { column.is_none() && valid_row }
    })
}

fn edit_prepared_formula_text_with_context(input: &str, host: &str, prepared: &PreparedReferenceEdit<'_>, bare_error: bool, moved_host: bool) -> FormulaEdit {
    let edit = prepared.edit;
    let mut patches = Vec::new();
    let mut invalidated = 0;
    // Mandatory source-backed spans come from the shared lexer's raw emission
    // boundary, not a filter over optional spans or a second formula parser.
    for ChartFormulaOperand { span, text: value } in chart_formula_operands(input) {
        if value.contains('[') || value.contains(']') { continue; }
        let (sheet, range) = if let Some(qualifier) = value.strip_suffix("!#REF!") {
            (qualifier.trim_matches(&['\'', '"'][..]), "#REF!")
        } else { split_address(value) };
        if range == "#REF!" {
            let replacement = if sheet.contains(':') {
                edit_3d_sheet(sheet, range, prepared).0
            } else {
                let effective_sheet = unescape_sheet_name(sheet);
                match edit {
                    ChartReferenceEdit::RenameSheet { old, new } if sheet_matches(&effective_sheet, old) =>
                        Some(format!("{}!#REF!", quote_sheet_name(new))),
                    ChartReferenceEdit::DeleteSheet { sheet: deleted, .. } if sheet_matches(&effective_sheet, deleted) =>
                        Some("#REF!".to_owned()),
                    ChartReferenceEdit::RenameSheet { .. } | ChartReferenceEdit::DeleteSheet { .. }
                    | ChartReferenceEdit::InsertRows { .. } | ChartReferenceEdit::InsertColumns { .. }
                    | ChartReferenceEdit::DeleteRows { .. } | ChartReferenceEdit::DeleteColumns { .. }
                    | ChartReferenceEdit::MoveRange { .. } => None,
                }
            };
            if let Some(text) = replacement { patches.push((span, if bare_error { "#REF!".to_owned() } else { text })); }
            continue;
        }
        if !is_native_coordinate_operand(range) { continue; }
        if sheet.contains(':') {
            let (replacement, invalid) = edit_3d_sheet(sheet, range, prepared);
            if let Some(text) = replacement { patches.push((span, text)); }
            invalidated += usize::from(invalid);
            continue;
        }
        let effective_sheet = if sheet.is_empty() { host.to_owned() } else { unescape_sheet_name(sheet).into_owned() };
        let target = match edit {
            ChartReferenceEdit::RenameSheet { old, .. } => old,
            ChartReferenceEdit::DeleteSheet { sheet, .. } | ChartReferenceEdit::InsertRows { sheet, .. }
            | ChartReferenceEdit::InsertColumns { sheet, .. } | ChartReferenceEdit::DeleteRows { sheet, .. }
            | ChartReferenceEdit::DeleteColumns { sheet, .. } => sheet,
            ChartReferenceEdit::MoveRange { source_sheet, .. } => source_sheet,
        };
        let matches_target = sheet_matches(&effective_sheet, target);
        let (replacement, invalid) = match edit {
            ChartReferenceEdit::RenameSheet { new, .. } if !sheet.is_empty() && matches_target =>
                (Some(format!("{}!{range}", quote_sheet_name(new))), false),
            ChartReferenceEdit::DeleteSheet { .. } if matches_target => (Some("#REF!".to_owned()), true),
            ChartReferenceEdit::InsertRows { at, count, .. } if matches_target =>
                insert_in_range(value, range, *at, *count, false),
            ChartReferenceEdit::InsertColumns { at, count, .. } if matches_target =>
                insert_in_range(value, range, *at, *count, true),
            ChartReferenceEdit::DeleteRows { .. } if matches_target =>
                delete_in_range(value, range, &prepared.deleted, prepared.deleted_count, false),
            ChartReferenceEdit::DeleteColumns { .. } if matches_target =>
                delete_in_range(value, range, &prepared.deleted, prepared.deleted_count, true),
            ChartReferenceEdit::MoveRange { source_sheet, source, destination_sheet, destination } =>
                move_in_range(value, range, &effective_sheet, &MoveContext { source_sheet, source, destination_sheet, destination, moved_host, unqualified: sheet.is_empty() }),
            ChartReferenceEdit::RenameSheet { .. } | ChartReferenceEdit::DeleteSheet { .. }
            | ChartReferenceEdit::InsertRows { .. } | ChartReferenceEdit::InsertColumns { .. }
            | ChartReferenceEdit::DeleteRows { .. } | ChartReferenceEdit::DeleteColumns { .. } => (None, false),
        };
        invalidated += usize::from(invalid);
        if let Some(text) = replacement { patches.push((span, if invalid && bare_error { "#REF!".to_owned() } else { text })); }
    }
    if bare_error && invalidated > 0 { return FormulaEdit { text: "#REF!".to_owned(), invalidated }; }
    let mut output = String::new();
    let mut cursor = 0;
    // The producer's typed projection guarantees ordered, nonoverlapping UTF8
    // spans, including malformed-source sibling cases tested at that owner.
    for (span, replacement) in patches {
        output.push_str(&input[cursor..span.start]);
        output.push_str(&replacement);
        cursor = span.end;
    }
    output.push_str(&input[cursor..]);
    FormulaEdit { text: output, invalidated }
}

// Extrapolated from independently audited Excel CELL/NAME captures, not a
// claim of an Excel chart-3D witness. A single-sheet structural edit retains
// a multi-sheet coordinate area; native endpoint/interior row captures confirm
// that this is not equivalent to changing the area on every included sheet.
fn edit_3d_sheet(sheet: &str, range: &str, prepared: &PreparedReferenceEdit<'_>) -> (Option<String>, bool) {
    let edit = prepared.edit;
    let Some((first, last)) = sheet.split_once(':') else { return (None, false); };
    let mut first=unescape_sheet_name(first).into_owned();
    let mut last=unescape_sheet_name(last).into_owned();
    let mut changed=false;
    if sheet_matches(&first, &last) && !matches!(edit, ChartReferenceEdit::RenameSheet { .. } | ChartReferenceEdit::DeleteSheet { .. }) {
        let single = format!("{}!{range}", quote_sheet_name(&first));
        let result = edit_prepared_formula_text(&single, "", prepared, false);
        return ((result.text != single).then_some(result.text), result.invalidated != 0);
    }
    match edit {
        ChartReferenceEdit::RenameSheet { old,new } => {
            if sheet_matches(&first,old) { first.clone_from(new); changed=true; }
            if sheet_matches(&last,old) { last.clone_from(new); changed=true; }
        }
        ChartReferenceEdit::DeleteSheet { sheet: deleted, sheet_order } => {
            let first_deleted=sheet_matches(&first,deleted);
            let last_deleted=sheet_matches(&last,deleted);
            if first_deleted && last_deleted { return (Some("#REF!".into()),true); }
            if first_deleted || last_deleted {
                // Preparation validates real pre-delete order. An imported
                // dangling endpoint absent from that order remains lexical.
                let Some(fi) = sheet_order.iter().position(|name| sheet_matches(name, &first)) else { return (None, false); };
                let Some(li) = sheet_order.iter().position(|name| sheet_matches(name, &last)) else { return (None, false); };
                if fi==li { return (Some("#REF!".into()),true); }
                if first_deleted { first.clone_from(&sheet_order[if fi<li {fi+1} else {fi-1}]); }
                if last_deleted { last.clone_from(&sheet_order[if fi<li {li-1} else {li+1}]); }
                if sheet_matches(&first, &last) { return (Some(format!("{}!{range}", quote_sheet_name(&first))), false); }
                changed=true;
            }
        }
        ChartReferenceEdit::InsertRows { .. } | ChartReferenceEdit::InsertColumns { .. }
        | ChartReferenceEdit::DeleteRows { .. } | ChartReferenceEdit::DeleteColumns { .. }
        | ChartReferenceEdit::MoveRange { .. } => return (None, false),
    }
    if !changed { return (None,false); }
    let first_quoted=quote_sheet_name(&first); let last_quoted=quote_sheet_name(&last);
    let formatted=if first_quoted.starts_with('\'') || last_quoted.starts_with('\'') {
        format!("'{}:{}'",first.replace('\'',"''"),last.replace('\'',"''"))
    } else { format!("{first}:{last}") };
    (Some(format!("{formatted}!{range}")),false)
}

/// Inclusive geometric bounds, independent of operand spelling and `$` locks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct MoveBounds {
    left: u32,
    top: u32,
    right: u32,
    bottom: u32,
}

#[derive(Clone, Copy)]
enum EdgeAxis { Columns, Rows }

impl MoveBounds {
    fn contains(self, other: Self) -> bool {
        self.left <= other.left && self.top <= other.top
            && self.right >= other.right && self.bottom >= other.bottom
    }

    fn intersects(self, other: Self) -> bool {
        self.left <= other.right && self.right >= other.left
            && self.top <= other.bottom && self.bottom >= other.top
    }

    fn union_bounds(self, other: Self) -> Self {
        Self { left: self.left.min(other.left), top: self.top.min(other.top),
            right: self.right.max(other.right), bottom: self.bottom.max(other.bottom) }
    }

    /// Keep a rectangular remainder when an affected block covers a complete
    /// edge strip. Interior holes and corner notches do not change the range.
    /// A destination wholly within a reference overwrites data, not its
    /// identity: only a destination crossing an edge clips that edge.
    fn edge_remainder(self, block: Self, destination: bool) -> (Option<Self>, Option<EdgeAxis>) {
        if !self.intersects(block) { return (Some(self), None); }
        if block.contains(self) { return (None, None); }
        if block.left <= self.left && block.right >= self.right {
            if block.top <= self.top && (!destination || block.top < self.top) {
                return (Some(Self { top: block.bottom + 1, ..self }), Some(EdgeAxis::Rows));
            }
            if block.bottom >= self.bottom && (!destination || block.bottom > self.bottom) {
                return (Some(Self { bottom: block.top - 1, ..self }), Some(EdgeAxis::Rows));
            }
        }
        if block.top <= self.top && block.bottom >= self.bottom {
            if block.left <= self.left && (!destination || block.left < self.left) {
                return (Some(Self { left: block.right + 1, ..self }), Some(EdgeAxis::Columns));
            }
            if block.right >= self.right && (!destination || block.right > self.right) {
                return (Some(Self { right: block.left - 1, ..self }), Some(EdgeAxis::Columns));
            }
        }
        (Some(self), None)
    }
}

#[derive(Clone, Copy)]
struct MoveContext<'a> {
    source_sheet: &'a str,
    source: &'a ReferenceRect,
    destination_sheet: &'a str,
    destination: &'a ReferenceCell,
    moved_host: bool,
    unqualified: bool,
}

fn move_in_range(value: &str, range: &str, sheet: &str, context: &MoveContext<'_>) -> (Option<String>, bool) {
    let MoveContext { source_sheet, source, destination_sheet, destination, moved_host, unqualified } = *context;
    let source_bounds = MoveBounds { left: source.first.column, top: source.first.row,
        right: source.last.column, bottom: source.last.row };
    let destination_bounds = MoveBounds { left: destination.column, top: destination.row,
        right: destination.column + source.last.column - source.first.column,
        bottom: destination.row + source.last.row - source.first.row };
    let mut cells = Vec::new();
    for part in range.split(':') {
        let (col, row, cl, rl) = index_from_coordinate(part.to_ascii_uppercase());
        cells.push((col, row, cl.unwrap_or(false), rl.unwrap_or(false)));
    }
    let (first_col, first_row, _, _) = cells[0];
    let (last_col, last_row, _, _) = cells[cells.len() - 1];
    let (c1, c2) = (first_col.unwrap_or(1), last_col.unwrap_or(16_384));
    let (r1, r2) = (first_row.unwrap_or(1), last_row.unwrap_or(1_048_576));
    let bounds = MoveBounds { left: c1.min(c2), top: r1.min(r2), right: c1.max(c2), bottom: r1.max(r2) };
    let on_source = sheet_matches(sheet, source_sheet);
    let on_destination = sheet_matches(sheet, destination_sheet);
    let same_sheet = sheet_matches(source_sheet, destination_sheet);
    let prefix = value.len() - range.len();
    let mut sheet_prefix = value[..prefix].to_owned();
    // Only validated source-contained coordinates are translated; prepared
    // destination bounds keep addition/subtraction within the native grid.
    let translated = |coordinate: u32, source_start: u32, destination_start: u32| {
        if destination_start >= source_start { coordinate + (destination_start - source_start) }
        else { coordinate - (source_start - destination_start) }
    };
    let whole_axis = first_col.is_none() || first_row.is_none();
    let spans_omitted_axis = |block: MoveBounds| {
        (first_row.is_none() && block.top == 1 && block.bottom == 1_048_576)
            || (first_col.is_none() && block.left == 1 && block.right == 16_384)
    };
    let finite_axis_cut = whole_axis && !(on_source && spans_omitted_axis(source_bounds))
        && !(on_destination && spans_omitted_axis(destination_bounds));
    let new_bounds = if finite_axis_cut && !(on_source && source_bounds.contains(bounds)) {
        // A whole-axis operand keeps its axis identity under a finite cell
        // cut, including a cut at row/column one. Do not materialize a finite
        // A1 range merely because an edge strip of cells left the sheet.
        if on_destination && destination_bounds.contains(bounds) { None }
        else {
            if moved_host && unqualified && !same_sheet { sheet_prefix = format!("{}!", quote_sheet_name(source_sheet)); }
            Some(bounds)
        }
    } else if moved_host && unqualified && on_source && !same_sheet {
        // An implicit reference in a transported formula now belongs to the
        // destination host. Follow its source intersection; when there is no
        // moved precedent, keep the original sheet explicit instead.
        if source_bounds.intersects(bounds) {
            Some(MoveBounds {
                left: translated(bounds.left.max(source_bounds.left), source_bounds.left, destination_bounds.left),
                top: translated(bounds.top.max(source_bounds.top), source_bounds.top, destination_bounds.top),
                right: translated(bounds.right.min(source_bounds.right), source_bounds.left, destination_bounds.left),
                bottom: translated(bounds.bottom.min(source_bounds.bottom), source_bounds.top, destination_bounds.top),
            })
        } else {
            sheet_prefix = format!("{}!", quote_sheet_name(source_sheet));
            Some(bounds)
        }
    } else if on_source && source_bounds.contains(bounds) {
        // Follow a wholly moved reference even when source and destination
        // overlap. Structural movement is independent of absolute markers.
        if !same_sheet { sheet_prefix = format!("{}!", quote_sheet_name(destination_sheet)); }
        Some(MoveBounds {
            left: translated(bounds.left, source_bounds.left, destination_bounds.left),
            top: translated(bounds.top, source_bounds.top, destination_bounds.top),
            right: translated(bounds.right, source_bounds.left, destination_bounds.left),
            bottom: translated(bounds.bottom, source_bounds.top, destination_bounds.top),
        })
    } else if on_source {
        let (remainder, edge) = bounds.edge_remainder(source_bounds, false);
        if !same_sheet {
            // A partial cross-sheet source retains its source identity. Only
            // complete edge strips disappear; this is not a copy offset.
            remainder
        } else if let Some(axis) = edge.filter(|axis| {
            if bounds.intersects(destination_bounds) { return true; }
            let full_axis_source = !whole_axis && match axis {
                EdgeAxis::Rows => source_bounds.left == 1 && source_bounds.right == 16_384,
                EdgeAxis::Columns => source_bounds.top == 1 && source_bounds.bottom == 1_048_576,
            };
            if !moved_host && !full_axis_source { return false; }
            let (low, high, offset) = match axis {
                EdgeAxis::Rows => (bounds.left, bounds.right, i64::from(destination_bounds.left) - i64::from(source_bounds.left)),
                EdgeAxis::Columns => (bounds.top, bounds.bottom, i64::from(destination_bounds.top) - i64::from(source_bounds.top)),
            };
            i64::from(low) + offset <= i64::from(high) && i64::from(high) + offset >= i64::from(low)
        }) {
            // For a source edge moved into the reference, retain the unmoved
            // remainder and the destination span along that edge's axis. The
            // perpendicular projection still belongs to the reference, not
            // to a wider/taller source block (e.g. A2:C3 versus B2:B5).
            let moved_edge = match axis {
                EdgeAxis::Rows => MoveBounds {
                    left: translated(bounds.left, source_bounds.left, destination_bounds.left),
                    right: translated(bounds.right, source_bounds.left, destination_bounds.left),
                    ..destination_bounds
                },
                EdgeAxis::Columns => MoveBounds {
                    top: translated(bounds.top, source_bounds.top, destination_bounds.top),
                    bottom: translated(bounds.bottom, source_bounds.top, destination_bounds.top),
                    ..destination_bounds
                },
            };
            let full_axis_source = match axis {
                EdgeAxis::Rows => source_bounds.left == 1 && source_bounds.right == 16_384,
                EdgeAxis::Columns => source_bounds.top == 1 && source_bounds.bottom == 1_048_576,
            };
            let moved_edge = if moved_host || full_axis_source || source_bounds.intersects(destination_bounds) {
                // In an overlapping cut, only the reference's actual source
                // intersection survives the overwrite, not the whole block.
                MoveBounds {
                    left: translated(bounds.left.max(source_bounds.left), source_bounds.left, destination_bounds.left),
                    top: translated(bounds.top.max(source_bounds.top), source_bounds.top, destination_bounds.top),
                    right: translated(bounds.right.min(source_bounds.right), source_bounds.left, destination_bounds.left),
                    bottom: translated(bounds.bottom.min(source_bounds.bottom), source_bounds.top, destination_bounds.top),
                }
            } else { moved_edge };
            remainder.and_then(|remaining| remaining.edge_remainder(destination_bounds, false).0)
                .map_or(Some(moved_edge), |remaining| Some(remaining.union_bounds(moved_edge)))
        } else {
            bounds.edge_remainder(destination_bounds, true).0
        }
    } else if on_destination {
        bounds.edge_remainder(destination_bounds, true).0
    } else {
        Some(bounds)
    };
    let Some(new_bounds) = new_bounds else {
        return (Some(format!("{}#REF!", &value[..prefix])), true);
    };
    if new_bounds == bounds && sheet_prefix == value[..prefix] { return (None, false); }
    let last_index = cells.len() - 1;
    let omit_column = first_col.is_none() && new_bounds.left == 1 && new_bounds.right == 16_384;
    let omit_row = first_row.is_none() && new_bounds.top == 1 && new_bounds.bottom == 1_048_576;
    let output: Vec<_> = cells.into_iter().zip(range.split(':')).enumerate().map(|(i, ((_, _, cl, rl), original))| {
        let col = if (i == 0 && c1 <= c2) || (i == last_index && c1 > c2) { new_bounds.left } else { new_bounds.right };
        let row = if (i == 0 && r1 <= r2) || (i == last_index && r1 > r2) { new_bounds.top } else { new_bounds.bottom };
        render_coordinate((!omit_column).then_some(col), (!omit_row).then_some(row), cl, rl, original)
    }).collect();
    (Some(format!("{sheet_prefix}{}", output.join(":"))), false)
}

/// Binary search in normalized native indices using the cardinality already
/// bounded during preparation; no per-leaf narrowing conversion can fail.
fn deleted_partition(deleted: &[u32], count: u32, predicate: impl Fn(u32) -> bool) -> u32 {
    let (mut low, mut high) = (0, count);
    while low < high {
        let middle = low + (high - low) / 2;
        if predicate(deleted[middle as usize]) { low = middle + 1; }
        else { high = middle; }
    }
    low
}

fn delete_in_range(value: &str, range: &str, deleted: &[u32], deleted_count: u32, column: bool) -> (Option<String>, bool) {
    if deleted.is_empty() { return (None, false); }
    let mut cells = Vec::new();
    for part in range.split(':') {
        let (col, row, col_lock, row_lock) = index_from_coordinate(part.to_ascii_uppercase());
        cells.push((col, row, col_lock.unwrap_or(false), row_lock.unwrap_or(false)));
    }
    let selected = |cell: &(Option<u32>,Option<u32>,bool,bool)| if column { cell.0 } else { cell.1 };
    let Some(first) = selected(&cells[0]) else { return (None, false); };
    let Some(last) = selected(&cells[cells.len() - 1]) else { return (None, false); };
    let lo = first.min(last); let hi = first.max(last);
    let before = deleted_partition(deleted, deleted_count, |value| value < lo);
    let through = deleted_partition(deleted, deleted_count, |value| value <= hi);
    let inside = through - before;
    if inside == hi - lo + 1 {
        let prefix = value.len() - range.len();
        return (Some(format!("{}#REF!", &value[..prefix])), true);
    }
    if before == 0 && inside == 0 { return (None, false); }
    let new_lo = lo - before;
    let new_hi = hi - through;
    let reversed = first > last;
    let len = cells.len();
    for (index, cell) in cells.iter_mut().enumerate() {
        let new_value = if len == 1 || ((index == 0) != reversed) { new_lo } else { new_hi };
        if column { cell.0 = Some(new_value); } else { cell.1 = Some(new_value); }
    }
    let output: Vec<_> = cells.into_iter().zip(range.split(':')).map(|((col,row,cl,rl), original)| render_coordinate(col,row,cl,rl,original)).collect();
    let prefix = value.len() - range.len();
    (Some(format!("{}{}", &value[..prefix], output.join(":"))), false)
}

fn insert_in_range(value: &str, range: &str, at: u32, count: u32, column: bool) -> (Option<String>, bool) {
    if count == 0 { return (None, false); }
    let limit: u32 = if column { 16_384 } else { 1_048_576 };
    let mut changed = false;
    let mut output = Vec::new();
    let mut survives = false;
    let mut has_selected_dimension = false;
    for component in range.split(':') {
        let (col, row, col_lock, row_lock) = index_from_coordinate(component.to_ascii_uppercase());
        let (mut col, mut row) = (col, row);
        let selected = if column { &mut col } else { &mut row };
        if let Some(selected) = selected {
            has_selected_dimension = true;
            // Use a wider intermediate: a valid range can leave the grid
            // without making the edit invalid. Preserve the surviving edge,
            // or invalidate when every referenced coordinate has left it.
            let shifted = u64::from(*selected) + if *selected >= at { u64::from(count) } else { 0 };
            survives |= shifted <= u64::from(limit);
            let clipped = if shifted > u64::from(limit) { limit }
                else if *selected >= at { *selected + count } else { *selected };
            changed |= clipped != *selected;
            *selected = clipped;
        }
        output.push(render_coordinate(col, row, col_lock.unwrap_or(false), row_lock.unwrap_or(false), component));
    }
    let prefix = value.len() - range.len();
    if has_selected_dimension && !survives {
        return (Some(format!("{}#REF!", &value[..prefix])), true);
    }
    if !changed { return (None, false); }
    (Some(format!("{}{}", &value[..prefix], output.join(":"))), false)
}

fn render_coordinate(col: Option<u32>, row: Option<u32>, col_lock: bool, row_lock: bool, original: &str) -> String {
    let mut result = String::new();
    let (old_col,old_row,_,_)=index_from_coordinate(original.to_ascii_uppercase());
    let mut split=0;
    if old_col.is_some() {
        if original.as_bytes().first()==Some(&b'$') { split+=1; }
        while original.as_bytes().get(split).is_some_and(u8::is_ascii_alphabetic) { split+=1; }
    }
    if let Some(col) = col {
        if Some(col)==old_col { result.push_str(&original[..split]); }
        else {
            if col_lock { result.push('$'); }
            let letters=string_from_column_index(col);
            let old_letters=original[..split].trim_start_matches('$');
            if !old_letters.is_empty() && old_letters.bytes().all(|b| b.is_ascii_lowercase()) { result.push_str(&letters.to_ascii_lowercase()); }
            else { result.push_str(&letters); }
        }
    }
    if let Some(row) = row {
        if Some(row)==old_row { result.push_str(&original[split..]); }
        else { if row_lock { result.push('$'); } result.push_str(&row.to_string()); }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn insert_moves_locked_coordinates_without_touching_literal_or_external_operands() {
        let edit = ChartReferenceEdit::InsertRows { sheet: "Data".into(), at: 4, count: 1 };
        let input = "SUM(Data!$B$2:$B$5,\"Data!$B$2:$B$5\",'[Book.xlsx]Data'!$B$2:$B$5)";
        assert_eq!(edit_formula_text(input, "Host", &edit, false).unwrap().text, "SUM(Data!$B$2:$B$6,\"Data!$B$2:$B$5\",'[Book.xlsx]Data'!$B$2:$B$5)");
        assert_eq!(edit_formula_text("Data!$b$002:$b$5", "Host", &edit, false).unwrap().text, "Data!$b$002:$b$6");
    }
    #[test]
    fn rename_requotes_escaped_names_without_rewriting_untouched_slices() {
        let edit = ChartReferenceEdit::RenameSheet { old: "Data".into(), new: "Bob's Data".into() };
        assert_eq!(edit_formula_text("=Data!$B$2:$B$5 + 1", "Host", &edit, false).unwrap().text, "='Bob''s Data'!$B$2:$B$5 + 1");
    }
    #[test]
    fn unqualified_references_use_host_sheet_and_names_remain_names() {
        let edit = ChartReferenceEdit::InsertRows { sheet: "Host".into(), at: 1, count: 1 };
        assert_eq!(edit_formula_text("SUM($A$1,MyNamedRange,Other!$A$1)", "Host", &edit, false).unwrap().text,
                   "SUM($A$2,MyNamedRange,Other!$A$1)");
        assert_eq!(edit_formula_text("SUM($A$1,MyNamedRange)", "Other", &edit, false).unwrap().text,
                   "SUM($A$1,MyNamedRange)");
    }
    #[test]
    fn native_unicode_sheet_case_equivalence_adjusts_without_rewriting_qualifier_spelling() {
        let edit = ChartReferenceEdit::InsertRows { sheet: "Étage".into(), at: 1, count: 1 };
        assert_eq!(edit_formula_text("'étage'!$A$1", "Host", &edit, false).unwrap().text, "'étage'!$A$2");
    }
    #[test]
    fn deletion_is_normalized_and_shrinks_or_invalidates_references() {
        let edit = ChartReferenceEdit::DeleteRows { sheet: "Data".into(), rows: vec![3,3] };
        assert_eq!(edit_formula_text("Data!$B$2:$B$5", "Host", &edit, false).unwrap().text, "Data!$B$2:$B$4");
        let prepared = PreparedReferenceEdit::new(&edit).unwrap();
        assert_eq!(edit_prepared_formula_text("Data!$C$2:$C$5", "Host", &prepared, false).text, "Data!$C$2:$C$4");
        assert_eq!(edit_prepared_formula_text("Data!$D$1", "Host", &prepared, false).text, "Data!$D$1");
        let edit = ChartReferenceEdit::DeleteRows { sheet: "Data".into(), rows: vec![5,2,4,3] };
        let result = edit_formula_text("Data!$B$2:$B$5", "Host", &edit, false).unwrap();
        assert_eq!(result.text, "Data!#REF!"); assert_eq!(result.invalidated, 1);
        assert_eq!(edit_formula_text("Data!$B$2:$B$5", "Host", &edit, true).unwrap().text, "#REF!");
    }
    #[test]
    fn native_excel_field_table_deletions_keep_the_observed_role_specific_error_spelling() {
        // Excel COM type6, field-preparation-r2/run-r1; independently audited
        // c15:dlblFTEntry/c15:f only, not a label-range or chart-3D witness.
        let operations = [
            (ChartReferenceEdit::DeleteRows { sheet: "Data".into(), rows: vec![2,3,4,5] }, "Data!#REF!"),
            (ChartReferenceEdit::DeleteColumns { sheet: "Data".into(), columns: vec![3] }, "Data!#REF!"),
            (ChartReferenceEdit::DeleteSheet { sheet: "Data".into(), sheet_order: vec!["Data".into(),"Host".into()] }, "#REF!"),
        ];
        for (edit, expected) in operations {
            let result = edit_formula_text("Data!$C$2", "Host", &edit, false).unwrap();
            assert_eq!(result.text, expected);
            assert_eq!(result.invalidated, 1);
        }
    }
    #[test]
    fn witnessed_ordinary_deletion_formula_values_do_not_claim_owner_retention() {
        // Excel capture-r3 ordinary-complete-delete, surviving c:f values.
        // Other formula nodes disappear in Excel and are deliberately not
        // accounted for by this per-leaf test.
        let rows = ChartReferenceEdit::DeleteRows { sheet: "Data".into(), rows: vec![2,3,4,5] };
        assert_eq!(edit_formula_text("Data!$B$2", "Host", &rows, false).unwrap().text, "Data!#REF!");
        assert_eq!(edit_formula_text("Data!$B$1", "Host", &rows, false).unwrap().text, "Data!$B$1");
        let columns = ChartReferenceEdit::DeleteColumns { sheet: "Data".into(), columns: vec![2] };
        assert_eq!(edit_formula_text("Data!$B$2", "Host", &columns, false).unwrap().text, "Data!#REF!");
        assert_eq!(edit_formula_text("Data!$A$2:$A$5", "Host", &columns, false).unwrap().text, "Data!$A$2:$A$5");
    }
    #[test]
    fn whole_axis_ranges_move_only_their_structural_dimension() {
        let columns = ChartReferenceEdit::InsertColumns { sheet: "Data".into(), at: 3, count: 1 };
        assert_eq!(edit_formula_text("SUM(Data!$B:$D)", "Host", &columns, false).unwrap().text, "SUM(Data!$B:$E)");
        let rows = ChartReferenceEdit::DeleteRows { sheet: "Data".into(), rows: vec![2] };
        assert_eq!(edit_formula_text("Data!$1:$3", "Host", &rows, false).unwrap().text, "Data!$1:$2");
        assert_eq!(edit_formula_text("Data!$B:$D", "Host", &rows, false).unwrap().text, "Data!$B:$D");
    }
    #[test]
    fn structural_edits_use_one_based_coordinates_and_preserve_zero_count() {
        let edit = ChartReferenceEdit::InsertColumns { sheet: "Data".into(), at: 1, count: 1 };
        assert_eq!(edit_formula_text("Data!$A$1:$B$2", "Host", &edit, false).unwrap().text, "Data!$B$1:$C$2");
        let noop = ChartReferenceEdit::InsertColumns { sheet: "Data".into(), at: 1, count: 0 };
        assert_eq!(edit_formula_text("Data!$A$1:$B$2", "Host", &noop, false).unwrap().text, "Data!$A$1:$B$2");
        assert_eq!(edit_formula_text("Data!$A$0", "Host", &edit, false).unwrap().text, "Data!$A$0");
        let invalid = ChartReferenceEdit::InsertRows { sheet: "Data".into(), at: 0, count: 1 };
        assert_eq!(edit_formula_text("Data!A1", "Host", &invalid, false), Err(UnresolvedEdit::InvalidCoordinate));
        let invalid = ChartReferenceEdit::DeleteRows { sheet: "Data".into(), rows: vec![0] };
        assert_eq!(edit_formula_text("Data!A1", "Host", &invalid, false), Err(UnresolvedEdit::InvalidCoordinate));
    }
    #[test]
    fn witnessed_contained_moves_and_overwrite_are_handled() {
        // Excel capture-r3/full-contained-move and crosssheet-partial.
        let edit = ChartReferenceEdit::MoveRange { source_sheet: "Data".into(), source: ReferenceRect { first: ReferenceCell { column: 2, row: 2 }, last: ReferenceCell { column: 2, row: 5 } }, destination_sheet: "Data".into(), destination: ReferenceCell { column: 5, row: 8 } };
        assert_eq!(edit_formula_text("Data!$B$2:$B$5", "Host", &edit, false).unwrap().text, "Data!$E$8:$E$11");
        let overwritten=edit_formula_text("Data!$E$8:$E$9", "Host", &edit, false).unwrap();
        assert_eq!(overwritten.text, "Data!#REF!"); assert_eq!(overwritten.invalidated, 1);
        assert_eq!(edit_formula_text("Data!$B$1:$B$3", "Host", &edit, false).unwrap().text, "Data!$B$1:$B$3");
        let partial = ChartReferenceEdit::MoveRange { source_sheet: "Data".into(), source: ReferenceRect { first: ReferenceCell { column: 2, row: 3 }, last: ReferenceCell { column: 2, row: 4 } }, destination_sheet: "Other".into(), destination: ReferenceCell { column: 5, row: 8 } };
        assert_eq!(edit_formula_text("Data!$B$3:$B$4", "Host", &partial, false).unwrap().text, "Other!$E$8:$E$9");
        assert_eq!(edit_formula_text("Data!$B$2:$B$5", "Host", &partial, false).unwrap().text, "Data!$B$2:$B$5");
    }
    #[test]
    fn witnessed_single_column_overlap_preserves_the_surviving_reference_shape() {
        let edit = ChartReferenceEdit::MoveRange { source_sheet: "Data".into(), source: ReferenceRect { first: ReferenceCell { column: 2, row: 2 }, last: ReferenceCell { column: 2, row: 3 } }, destination_sheet: "Data".into(), destination: ReferenceCell { column: 2, row: 3 } };
        assert_eq!(edit_formula_text("Data!$B$2:$B$5", "Host", &edit, false).unwrap().text, "Data!$B$3:$B$5");
        assert_eq!(edit_formula_text("Data!$B$3:$B$4", "Host", &edit, false).unwrap().text, "Data!$B$4:$B$4");
    }
    #[test]
    fn witnessed_single_column_partial_cut_preserves_parent_and_invalidates_destination() {
        // Excel capture-r3/partial-one-column, actual chart formula nodes.
        let edit = ChartReferenceEdit::MoveRange { source_sheet: "Data".into(), source: ReferenceRect { first: ReferenceCell { column: 2, row: 3 }, last: ReferenceCell { column: 2, row: 4 } }, destination_sheet: "Data".into(), destination: ReferenceCell { column: 5, row: 8 } };
        for (before,after,invalidated) in [
            ("Data!$B$2:$B$5", "Data!$B$2:$B$5", 0),
            ("Data!$B$3:$B$4", "Data!$E$8:$E$9", 0),
            ("Data!$E$8:$E$9", "Data!#REF!", 1),
        ] {
            let result = edit_formula_text(before, "Host", &edit, false).unwrap();
            assert_eq!(result.text, after);
            assert_eq!(result.invalidated, invalidated);
        }
    }
    #[test]
    fn witnessed_partial_rectangle_move_retains_intersecting_series_reference() {
        // Excel capture-r3/partial-rectangle-names: genuine chart and name
        // references remain unchanged after Data!B2:C3 moves to Data!F8.
        let edit = ChartReferenceEdit::MoveRange { source_sheet: "Data".into(), source: ReferenceRect { first: ReferenceCell { column: 2, row: 2 }, last: ReferenceCell { column: 3, row: 3 } }, destination_sheet: "Data".into(), destination: ReferenceCell { column: 6, row: 8 } };
        assert_eq!(edit_formula_text("Data!$B$2:$B$5", "Host", &edit, false).unwrap().text, "Data!$B$2:$B$5");
    }
    #[test]
    fn three_dimensional_endpoint_rules_are_only_extrapolated_from_cell_name_captures() {
        let edit = ChartReferenceEdit::RenameSheet { old: "First".into(), new: "Renamed".into() };
        assert_eq!(edit_formula_text("First:Last!$B$2", "Host", &edit, false).unwrap().text, "Renamed:Last!$B$2");
        let edit = ChartReferenceEdit::RenameSheet { old: "First".into(), new: "First Bob's Data".into() };
        assert_eq!(edit_formula_text("First:Last!$B$2:$B$5", "Host", &edit, false).unwrap().text, "'First Bob''s Data:Last'!$B$2:$B$5");
        let edit = ChartReferenceEdit::DeleteSheet { sheet: "First".into(), sheet_order: vec!["First".into(), "Mid".into(), "Last".into(), "Other".into()] };
        assert_eq!(edit_formula_text("First:Last!$B$2:$B$5", "Host", &edit, false).unwrap().text, "Mid:Last!$B$2:$B$5");
        let structural = ChartReferenceEdit::InsertRows { sheet: "Mid".into(), at: 2, count: 1 };
        assert_eq!(edit_formula_text("First:Last!$B$2", "Host", &structural, false).unwrap().text, "First:Last!$B$2");
    }
    #[test]
    fn rectangle_edge_clipping_distinguishes_corner_holes_and_crossing_overwrites() {
        // Geometry unit expectations, not additional native Excel witnesses.
        let reference = MoveBounds { left: 2, top: 2, right: 4, bottom: 5 };
        let corner = MoveBounds { left: 1, top: 1, right: 2, bottom: 3 };
        assert_eq!(reference.edge_remainder(corner, false).0, Some(reference));
        let interior = MoveBounds { left: 3, top: 3, right: 3, bottom: 4 };
        assert_eq!(reference.edge_remainder(interior, false).0, Some(reference));
        let contained_edge = MoveBounds { left: 2, top: 2, right: 4, bottom: 3 };
        assert_eq!(reference.edge_remainder(contained_edge, false).0,
                   Some(MoveBounds { top: 4, ..reference }));
        assert_eq!(reference.edge_remainder(contained_edge, true).0, Some(reference));
        let crossing_edge = MoveBounds { top: 1, ..contained_edge };
        assert_eq!(reference.edge_remainder(crossing_edge, true).0,
                   Some(MoveBounds { top: 4, ..reference }));
        assert_eq!(reference.edge_remainder(reference, true).0, None);
    }
    #[test]
    fn contained_move_preserves_mixed_locks_quoted_unicode_literals_and_external_operands() {
        let edit = ChartReferenceEdit::MoveRange {
            source_sheet: "Données d'été".into(),
            source: ReferenceRect { first: ReferenceCell { column: 2, row: 2 }, last: ReferenceCell { column: 3, row: 3 } },
            destination_sheet: "Autre été".into(), destination: ReferenceCell { column: 6, row: 8 },
        };
        let input = "= SUM('Données d''été'!$b$002:C$3, NamedRange, \"Data!B2\", '[Book.xlsx]Données d''été'!B2)";
        assert_eq!(edit_formula_text(input, "Host", &edit, false).unwrap().text,
                   "= SUM('Autre été'!$f$8:G$9, NamedRange, \"Data!B2\", '[Book.xlsx]Données d''été'!B2)");
    }
    #[test]
    fn source_identity_move_preserves_lexical_bytes_and_reversed_endpoints() {
        let mut edit = ChartReferenceEdit::MoveRange {
            source_sheet: "Data".into(),
            source: ReferenceRect { first: ReferenceCell { column: 2, row: 2 }, last: ReferenceCell { column: 2, row: 5 } },
            destination_sheet: "Data".into(), destination: ReferenceCell { column: 2, row: 2 },
        };
        assert_eq!(edit_formula_text("Data!$b$005:$b$002", "Host", &edit, false).unwrap().text,
                   "Data!$b$005:$b$002");
        if let ChartReferenceEdit::MoveRange { destination, .. } = &mut edit { destination.column = 5; destination.row = 8; }
        assert_eq!(edit_formula_text("Data!$B$5:$B$2", "Host", &edit, false).unwrap().text,
                   "Data!$E$11:$E$8");
    }
    #[test]
    fn native_cut_matrix_series_formulas_match_all_twenty_captured_pairs() {
        // Excel 16.0 build 20527, independently audited CUT-MATRIX-AUDIT-r1;
        // observations are raw c:f series values, not inferred semantics.
        let cases = [
            ("cut-2x2-b2c3-to-f8", (2, 2), (3, 3), "Data", (6, 8), [("Data!$B$2:$B$5", "Data!$B$2:$B$5"), ("Data!$B$3:$B$4", "Data!$B$3:$B$4"), ("Data!$E$8:$E$9", "Data!$E$8:$E$9"), ("Data!$H$10:$H$12", "Data!$H$10:$H$12")]),
            ("cut-2x2-b3c4-to-f8", (2, 3), (3, 4), "Data", (6, 8), [("Data!$B$2:$B$5", "Data!$B$2:$B$5"), ("Data!$B$3:$B$4", "Data!$F$8:$F$9"), ("Data!$E$8:$E$9", "Data!$E$8:$E$9"), ("Data!$H$10:$H$12", "Data!$H$10:$H$12")]),
            ("cut-3x3-a1c3-to-f8", (1, 1), (3, 3), "Data", (6, 8), [("Data!$B$2:$B$5", "Data!$B$2:$B$5"), ("Data!$B$3:$B$4", "Data!$B$3:$B$4"), ("Data!$E$8:$E$9", "Data!$E$8:$E$9"), ("Data!$H$10:$H$12", "Data!$H$11:$H$12")]),
            ("cut-3x3-a4c6-to-f8", (1, 4), (3, 6), "Data", (6, 8), [("Data!$B$2:$B$5", "Data!$B$2:$B$5"), ("Data!$B$3:$B$4", "Data!$B$3:$B$4"), ("Data!$E$8:$E$9", "Data!$E$8:$E$9"), ("Data!$H$10:$H$12", "Data!$H$11:$H$12")]),
            ("cut-b2b3-to-b4", (2, 2), (2, 3), "Data", (2, 4), [("Data!$B$2:$B$5", "Data!$B$4:$B$5"), ("Data!$B$3:$B$4", "Data!$B$4:$B$5"), ("Data!$E$8:$E$9", "Data!$E$8:$E$9"), ("Data!$H$10:$H$12", "Data!$H$10:$H$12")]),
            ("cut-b3b4-to-b2", (2, 3), (2, 4), "Data", (2, 2), [("Data!$B$2:$B$5", "Data!$B$2:$B$5"), ("Data!$B$3:$B$4", "Data!$B$2:$B$3"), ("Data!$E$8:$E$9", "Data!$E$8:$E$9"), ("Data!$H$10:$H$12", "Data!$H$10:$H$12")]),
            ("cut-b3b4-to-b5", (2, 3), (2, 4), "Data", (2, 5), [("Data!$B$2:$B$5", "Data!$B$2:$B$4"), ("Data!$B$3:$B$4", "Data!$B$5:$B$6"), ("Data!$E$8:$E$9", "Data!$E$8:$E$9"), ("Data!$H$10:$H$12", "Data!$H$10:$H$12")]),
            ("cut-b1b3-to-b5", (2, 1), (2, 3), "Data", (2, 5), [("Data!$B$2:$B$5", "Data!$B$4:$B$7"), ("Data!$B$3:$B$4", "Data!$B$3:$B$4"), ("Data!$E$8:$E$9", "Data!$E$8:$E$9"), ("Data!$H$10:$H$12", "Data!$H$10:$H$12")]),
            ("cut-b4b6-to-b1", (2, 4), (2, 6), "Data", (2, 1), [("Data!$B$2:$B$5", "Data!$B$1:$B$3"), ("Data!$B$3:$B$4", "Data!$B$1:$B$3"), ("Data!$E$8:$E$9", "Data!$E$8:$E$9"), ("Data!$H$10:$H$12", "Data!$H$10:$H$12")]),
            ("cut-a2c3-to-a4", (1, 2), (3, 3), "Data", (1, 4), [("Data!$B$2:$B$5", "Data!$B$4:$B$5"), ("Data!$B$3:$B$4", "Data!$B$4:$B$5"), ("Data!$E$8:$E$9", "Data!$E$8:$E$9"), ("Data!$H$10:$H$12", "Data!$H$10:$H$12")]),
            ("cut-a3c4-to-a2", (1, 3), (3, 4), "Data", (1, 2), [("Data!$B$2:$B$5", "Data!$B$2:$B$5"), ("Data!$B$3:$B$4", "Data!$B$2:$B$3"), ("Data!$E$8:$E$9", "Data!$E$8:$E$9"), ("Data!$H$10:$H$12", "Data!$H$10:$H$12")]),
            ("cut-a2c5-to-f8", (1, 2), (3, 5), "Data", (6, 8), [("Data!$B$2:$B$5", "Data!$G$8:$G$11"), ("Data!$B$3:$B$4", "Data!$G$9:$G$10"), ("Data!$E$8:$E$9", "Data!$E$8:$E$9"), ("Data!$H$10:$H$12", "Data!$H$12:$H$12")]),
            ("cut-b2b5-to-b3", (2, 2), (2, 5), "Data", (2, 3), [("Data!$B$2:$B$5", "Data!$B$3:$B$6"), ("Data!$B$3:$B$4", "Data!$B$4:$B$5"), ("Data!$E$8:$E$9", "Data!$E$8:$E$9"), ("Data!$H$10:$H$12", "Data!$H$10:$H$12")]),
            ("cut-b2b5-to-b1", (2, 2), (2, 5), "Data", (2, 1), [("Data!$B$2:$B$5", "Data!$B$1:$B$4"), ("Data!$B$3:$B$4", "Data!$B$2:$B$3"), ("Data!$E$8:$E$9", "Data!$E$8:$E$9"), ("Data!$H$10:$H$12", "Data!$H$10:$H$12")]),
            ("cut-e8e9-to-b2", (5, 8), (5, 9), "Data", (2, 2), [("Data!$B$2:$B$5", "Data!$B$2:$B$5"), ("Data!$B$3:$B$4", "Data!$B$4:$B$4"), ("Data!$E$8:$E$9", "Data!$B$2:$B$3"), ("Data!$H$10:$H$12", "Data!$H$10:$H$12")]),
            ("cut-e8e9-to-b3", (5, 8), (5, 9), "Data", (2, 3), [("Data!$B$2:$B$5", "Data!$B$2:$B$5"), ("Data!$B$3:$B$4", "Data!#REF!"), ("Data!$E$8:$E$9", "Data!$B$3:$B$4"), ("Data!$H$10:$H$12", "Data!$H$10:$H$12")]),
            ("cut-crosssheet-b2c3-to-other-f8", (2, 2), (3, 3), "Other", (6, 8), [("Data!$B$2:$B$5", "Data!$B$4:$B$5"), ("Data!$B$3:$B$4", "Data!$B$4:$B$4"), ("Data!$E$8:$E$9", "Data!$E$8:$E$9"), ("Data!$H$10:$H$12", "Data!$H$10:$H$12")]),
            ("cut-crosssheet-b2b5-to-other-e8", (2, 2), (2, 5), "Other", (5, 8), [("Data!$B$2:$B$5", "Other!$E$8:$E$11"), ("Data!$B$3:$B$4", "Other!$E$9:$E$10"), ("Data!$E$8:$E$9", "Data!$E$8:$E$9"), ("Data!$H$10:$H$12", "Data!$H$10:$H$12")]),
            ("cut-b2c3-to-b4", (2, 2), (3, 3), "Data", (2, 4), [("Data!$B$2:$B$5", "Data!$B$4:$B$5"), ("Data!$B$3:$B$4", "Data!$B$4:$B$5"), ("Data!$E$8:$E$9", "Data!$E$8:$E$9"), ("Data!$H$10:$H$12", "Data!$H$10:$H$12")]),
            ("cut-crosssheet-a1c6-to-other-f8", (1, 1), (3, 6), "Other", (6, 8), [("Data!$B$2:$B$5", "Other!$G$9:$G$12"), ("Data!$B$3:$B$4", "Other!$G$10:$G$11"), ("Data!$E$8:$E$9", "Data!$E$8:$E$9"), ("Data!$H$10:$H$12", "Data!$H$10:$H$12")]),
        ];
        let mut differences = Vec::new();
        for (name, first, last, destination_sheet, destination, pairs) in cases {
            let edit = ChartReferenceEdit::MoveRange {
                source_sheet: "Data".into(),
                source: ReferenceRect { first: ReferenceCell { column: first.0, row: first.1 }, last: ReferenceCell { column: last.0, row: last.1 } },
                destination_sheet: destination_sheet.into(),
                destination: ReferenceCell { column: destination.0, row: destination.1 },
            };
            for (before, expected) in pairs {
                match edit_formula_text(before, "Host", &edit, false) {
                    Ok(actual) if actual.text == expected => {},
                    Ok(actual) => differences.push(format!("{name}: {before} expected {expected}, got Ok({actual:?})")),
                    Err(error) => differences.push(format!("{name}: {before} expected {expected}, got Err({error:?})")),
                }
            }
        }
        assert!(differences.is_empty(), "{}", differences.join("\n"));
    }

    #[test]
    fn native_partial_move_boundary_probes_match_all_twenty_eight_additional_pairs() {
        // Independent native cut-matrix-preparation-r2/AUDIT-r1: seven cuts,
        // four actual c:ser/c:val/c:numRef/c:f formulas each. Not generated
        // from the geometry implementation or an implementation snapshot.
        let before = ["Data!$B$2:$B$5", "Data!$B$3:$B$4", "Data!$E$8:$E$9", "Data!$H$10:$H$12"];
        let cases = [
            ("B2:B3 to B5", (2, 2), (2, 3), "Data", (2, 5), ["Data!$B$4:$B$6", "Data!$B$3:$B$4", "Data!$E$8:$E$9", "Data!$H$10:$H$12"]),
            ("B2:B3 to B6", (2, 2), (2, 3), "Data", (2, 6), ["Data!$B$2:$B$5", "Data!$B$3:$B$4", "Data!$E$8:$E$9", "Data!$H$10:$H$12"]),
            ("B3:B4 to B4", (2, 3), (2, 4), "Data", (2, 4), ["Data!$B$2:$B$5", "Data!$B$4:$B$5", "Data!$E$8:$E$9", "Data!$H$10:$H$12"]),
            ("B3:B4 to B6", (2, 3), (2, 4), "Data", (2, 6), ["Data!$B$2:$B$5", "Data!$B$6:$B$7", "Data!$E$8:$E$9", "Data!$H$10:$H$12"]),
            ("B1:B2 to B4", (2, 1), (2, 2), "Data", (2, 4), ["Data!$B$3:$B$5", "Data!$B$3:$B$3", "Data!$E$8:$E$9", "Data!$H$10:$H$12"]),
            ("B1:B4 to B5", (2, 1), (2, 4), "Data", (2, 5), ["Data!$B$5:$B$8", "Data!$B$7:$B$8", "Data!$E$8:$E$9", "Data!$H$10:$H$12"]),
            ("A2:C3 to Other F8", (1, 2), (3, 3), "Other", (6, 8), ["Data!$B$4:$B$5", "Data!$B$4:$B$4", "Data!$E$8:$E$9", "Data!$H$10:$H$12"]),
        ];
        let mut differences = Vec::new();
        for (name, first, last, destination_sheet, destination, expected) in cases {
            let edit = ChartReferenceEdit::MoveRange {
                source_sheet: "Data".into(),
                source: ReferenceRect { first: ReferenceCell { column: first.0, row: first.1 }, last: ReferenceCell { column: last.0, row: last.1 } },
                destination_sheet: destination_sheet.into(),
                destination: ReferenceCell { column: destination.0, row: destination.1 },
            };
            for (input, expected) in before.into_iter().zip(expected) {
                match edit_formula_text(input, "Host", &edit, false) {
                    Ok(actual) if actual.text == expected => {},
                    Ok(actual) => differences.push(format!("{name}: {input} expected {expected}, got Ok({actual:?})")),
                    Err(error) => differences.push(format!("{name}: {input} expected {expected}, got Err({error:?})")),
                }
            }
        }
        assert!(differences.is_empty(), "{}", differences.join("\n"));
    }

    #[test]
    fn moved_formula_hosts_match_native_captured_implicit_operand_values() {
        // Raw C3 before/after formula XML from cut matrix r1/r2, independent
        // of static chart-container adjustment and ordinary copy semantics.
        let cases = [
            ((2, 2), (3, 3), "Data", (6, 8), "SUM(A1:B2)"),
            ((2, 3), (3, 4), "Data", (6, 8), "SUM(A1:B2)"),
            ((1, 1), (3, 3), "Data", (6, 8), "SUM(F8:G9)"),
            ((1, 2), (3, 3), "Data", (1, 4), "SUM(A1:B4)"),
            ((1, 3), (3, 4), "Data", (1, 2), "SUM(A1:B1)"),
            ((1, 2), (3, 5), "Data", (6, 8), "SUM(A1:B2)"),
            ((2, 2), (3, 3), "Other", (6, 8), "SUM(F8:F8)"),
            ((1, 1), (3, 6), "Other", (6, 8), "SUM(F8:G9)"),
            ((1, 2), (3, 3), "Other", (6, 8), "SUM(F8:G8)"),
        ];
        let mut differences = Vec::new();
        for (first, last, destination_sheet, destination, expected) in cases {
            let edit = ChartReferenceEdit::MoveRange {
                source_sheet: "Data".into(),
                source: ReferenceRect { first: ReferenceCell { column: first.0, row: first.1 }, last: ReferenceCell { column: last.0, row: last.1 } },
                destination_sheet: destination_sheet.into(),
                destination: ReferenceCell { column: destination.0, row: destination.1 },
            };
            let prepared = PreparedReferenceEdit::new(&edit).unwrap();
            let actual = edit_prepared_formula_text_for_moved_cell("SUM(A1:B2)", "Data", &prepared, false);
            if actual.text != expected {
                differences.push(format!("{first:?}:{last:?} to {destination_sheet} {destination:?}: expected {expected}, got {actual:?}"));
            }
        }
        assert!(differences.is_empty(), "{}", differences.join("\n"));
    }
    #[test]
    fn whole_axis_moves_use_finite_sheet_bounds_without_rejecting_unaffected_operands() {
        // Coordinate/grammar unit expectations; not extra native fixtures.
        let mut edit = ChartReferenceEdit::MoveRange {
            source_sheet: "Data".into(),
            source: ReferenceRect { first: ReferenceCell { column: 2, row: 2 }, last: ReferenceCell { column: 3, row: 5 } },
            destination_sheet: "Data".into(), destination: ReferenceCell { column: 6, row: 8 },
        };
        for input in ["Data!$B:$B", "Data!$1:$5", "Other!$B:$C", "SUM(Data!$A:$XFD)"] {
            assert_eq!(edit_formula_text(input, "Host", &edit, false).unwrap().text, input);
        }
        if let ChartReferenceEdit::MoveRange { source, destination, .. } = &mut edit {
            source.first.row = 1; source.last.row = 1_048_576; source.last.column = 2;
            destination.column = 5; destination.row = 1;
        }
        assert_eq!(edit_formula_text("Data!$B:$B", "Host", &edit, false).unwrap().text, "Data!$E:$E");
    }

    #[test]
    fn native_whole_axis_identity_survives_cross_sheet_cuts_at_the_first_row() {
        // semantic-context-preparation-r2: exact cell/name formulas from
        // Excel saves. A finite edge cut must not turn B:B into B3:B1048576.
        for last_column in [2, 3] {
            let edit = ChartReferenceEdit::MoveRange {
                source_sheet: "Data".into(),
                source: ReferenceRect { first: ReferenceCell { column: if last_column == 2 { 2 } else { 1 }, row: 1 }, last: ReferenceCell { column: last_column, row: 2 } },
                destination_sheet: "Other".into(), destination: ReferenceCell { column: 6, row: 8 },
            };
            for input in ["Data!$B:$B", "Data!$1:$5", "SUM(Data!$B:$B)", "SUM(Data!$1:$5)"] {
                assert_eq!(edit_formula_text(input, "Other", &edit, false).unwrap().text, input);
            }
        }
    }
    #[test]
    fn native_three_dimensional_cell_and_name_areas_survive_single_sheet_row_edits() {
        // Native CELL/NAME evidence only: Excel refused 3D chart admission.
        for sheet in ["First", "Mid", "Last"] {
            for edit in [ChartReferenceEdit::InsertRows { sheet: sheet.into(), at: 3, count: 1 },
                         ChartReferenceEdit::DeleteRows { sheet: sheet.into(), rows: vec![2, 3, 4, 5] }] {
                for input in ["First:Last!$B$2:$B$5", "SUM(First:Last!$B$2:$B$5)"] {
                    assert_eq!(edit_formula_text(input, "Other", &edit, false).unwrap().text, input);
                }
            }
        }
    }
    #[test]
    fn native_moved_cell_context_distinguishes_explicit_and_implicit_sheet_qualifiers() {
        // semantic-context-preparation-r2 raw formula values. In particular
        // an explicit cross-sheet qualifier is NOT an implicit intersection.
        let cases = [
            ((2, 3), (3, 4), "Other", (6, 8), "SUM(A1:B2)", "SUM(Data!A1:B2)"),
            ((1, 2), (3, 3), "Data", (1, 4), "SUM(Data!A1:B2)", "SUM(Data!A1:B4)"),
            ((2, 2), (3, 3), "Other", (6, 8), "SUM(Data!A1:B2)", "SUM(Data!A1:B2)"),
        ];
        for (first, last, destination_sheet, destination, input, expected) in cases {
            let edit = ChartReferenceEdit::MoveRange {
                source_sheet: "Data".into(),
                source: ReferenceRect { first: ReferenceCell { column: first.0, row: first.1 }, last: ReferenceCell { column: last.0, row: last.1 } },
                destination_sheet: destination_sheet.into(), destination: ReferenceCell { column: destination.0, row: destination.1 },
            };
            let prepared = PreparedReferenceEdit::new(&edit).unwrap();
            assert_eq!(edit_prepared_formula_text_for_moved_cell(input, "Data", &prepared, false).text, expected);
        }
    }
    #[test]
    fn native_unicode_sheet_lookup_uses_case_equivalence_and_preserves_untouched_utf8() {
        // Native lookup of U+00E9tage matched U+00C9tage. Excel serialized
        // new U+00C9tU+00E9 bare; our existing shared quoting policy emits
        // the semantically equivalent quoted qualifier. No raw-byte native
        // equality is claimed for that newly generated sheet qualifier.
        let insert = ChartReferenceEdit::InsertRows { sheet: "\u{00e9}tage".into(), at: 3, count: 1 };
        assert_eq!(edit_formula_text("SUM(\u{00c9}tage!$B$2:$B$5)", "Other", &insert, false).unwrap().text,
                   "SUM(\u{00c9}tage!$B$2:$B$6)");
        let rename = ChartReferenceEdit::RenameSheet { old: "\u{00e9}tage".into(), new: "\u{00c9}t\u{00e9}".into() };
        assert_eq!(edit_formula_text("SUM(\u{00c9}tage!$B$2:$B$5)", "Other", &rename, false).unwrap().text,
                   "SUM('\u{00c9}t\u{00e9}'!$B$2:$B$5)");
        let input = "SUM('\u{00c9}tage'!$B$2:$B$5,Other!A1,\"\u{00c9}tage!B2\")";
        assert_eq!(edit_formula_text(input, "Other", &rename, false).unwrap().text,
                   "SUM('\u{00c9}t\u{00e9}'!$B$2:$B$5,Other!A1,\"\u{00c9}tage!B2\")");
    }

    #[test]
    fn native_row_insertion_clips_surviving_grid_edge_or_invalidates_the_singleton() {
        // Native semantic-context-preparation-r3 row-max: actual cell/name
        // formula XML, not a candidate-generated boundary expectation.
        let edit = ChartReferenceEdit::InsertRows { sheet: "Data".into(), at: 1, count: 1 };
        for (input, expected, invalidated) in [
            ("Data!$A$1048576", "Data!#REF!", 1),
            ("Data!$A$1048575:$A$1048576", "Data!$A$1048576:$A$1048576", 0),
            ("SUM(Data!$A$1048576)", "SUM(Data!#REF!)", 1),
            ("SUM(Data!$A$1048575:$A$1048576)", "SUM(Data!$A$1048576:$A$1048576)", 0),
        ] {
            let actual = edit_formula_text(input, "Other", &edit, false).unwrap();
            assert_eq!(actual.text, expected); assert_eq!(actual.invalidated, invalidated);
        }
        assert_eq!(edit_formula_text("Data!$A$1048576", "Other", &edit, true).unwrap().text, "#REF!");
        assert_eq!(edit_formula_text("SUM(Data!$A$1048576,Other!A1)", "Other", &edit, true).unwrap().text, "#REF!");
    }
    #[test]
    fn insertion_grid_clipping_preserves_orientation_locks_and_unrelated_operand_spans() {
        // Geometry/lexical invariants, not additional native Excel captures.
        let edit = ChartReferenceEdit::InsertRows { sheet: "Data".into(), at: 1, count: 2 };
        assert_eq!(edit_formula_text("Data!$a$1048576:a1048574", "Other", &edit, false).unwrap().text,
                   "Data!$a$1048576:a1048576");
        let input = "=SUM(Data!A1048576,Other!A1048576,\"Data!A1048576\",'[Book.xlsx]Data'!A1048576,NamedRange)";
        assert_eq!(edit_formula_text(input, "Other", &edit, false).unwrap().text,
                   "=SUM(Data!#REF!,Other!A1048576,\"Data!A1048576\",'[Book.xlsx]Data'!A1048576,NamedRange)");
        assert_eq!(edit_formula_text("SUM(Data!$A:$A)", "Other", &edit, false).unwrap().text, "SUM(Data!$A:$A)");
    }

    #[test]
    fn native_column_insertion_clips_surviving_grid_edge_or_invalidates_the_singleton() {
        // Native semantic-context-preparation-r3/run-r2 column-max case.
        let edit = ChartReferenceEdit::InsertColumns { sheet: "Data".into(), at: 1, count: 1 };
        for (input, expected, invalidated) in [
            ("Data!$XFD$1", "Data!#REF!", 1),
            ("Data!$XFC$1:$XFD$1", "Data!$XFD$1:$XFD$1", 0),
            ("SUM(Data!$XFD$1)", "SUM(Data!#REF!)", 1),
            ("SUM(Data!$XFC$1:$XFD$1)", "SUM(Data!$XFD$1:$XFD$1)", 0),
        ] {
            let actual = edit_formula_text(input, "Other", &edit, false).unwrap();
            assert_eq!(actual.text, expected); assert_eq!(actual.invalidated, invalidated);
        }
        assert_eq!(edit_formula_text("Data!$XFD$1", "Other", &edit, true).unwrap().text, "#REF!");
    }
    #[test]
    fn native_three_dimensional_cell_name_columns_moves_and_endpoint_collapse_match_saved_xml() {
        // Native CELL/NAME observations only. Every chart3D admission was
        // refused; this regression does not invent chart-role evidence.
        let mut edits: Vec<_> = ["First", "Mid", "Last"].into_iter().map(|sheet|
            ChartReferenceEdit::InsertColumns { sheet: sheet.into(), at: 2, count: 1 }).collect();
        for destination_sheet in ["First", "Other"] {
            edits.push(ChartReferenceEdit::MoveRange {
                source_sheet: "First".into(),
                source: ReferenceRect { first: ReferenceCell { column: 2, row: 2 }, last: ReferenceCell { column: 2, row: 5 } },
                destination_sheet: destination_sheet.into(), destination: ReferenceCell { column: 6, row: 8 },
            });
        }
        for edit in edits {
            for input in ["First:Last!$B$2:$B$5", "SUM(First:Last!$B$2:$B$5)"] {
                assert_eq!(edit_formula_text(input, "Other", &edit, false).unwrap().text, input);
            }
        }
        let collapse = ChartReferenceEdit::DeleteSheet { sheet: "First".into(), sheet_order: vec!["First".into(), "Last".into(), "Other".into()] };
        assert_eq!(edit_formula_text("First:Last!$B$2:$B$5", "Other", &collapse, false).unwrap().text, "Last!$B$2:$B$5");
        assert_eq!(edit_formula_text("SUM(First:Last!$B$2:$B$5)", "Other", &collapse, false).unwrap().text, "SUM(Last!$B$2:$B$5)");
    }

    #[test]
    fn qualified_reference_errors_retain_sheet_identity_until_renamed_or_deleted() {
        // Requires the existing shared lexer to return a single lossless
        // operand for Data!#REF!, not an Unknown prefix + bare Error token.
        let rename = ChartReferenceEdit::RenameSheet { old: "Data".into(), new: "Bob's Data".into() };
        let input = "=SUM(Data!#REF!,Other!#REF!,#REF!,\"Data!#REF!\",'[Book.xlsx]Data'!#REF!)";
        let actual = edit_formula_text(input, "Host", &rename, false).unwrap();
        assert_eq!(actual.text,
                   "=SUM('Bob''s Data'!#REF!,Other!#REF!,#REF!,\"Data!#REF!\",'[Book.xlsx]Data'!#REF!)");
        assert_eq!(actual.invalidated, 0);
        let delete = ChartReferenceEdit::DeleteSheet { sheet: "Data".into(), sheet_order: vec!["Data".into(), "Host".into()] };
        let actual = edit_formula_text("SUM(Data!#REF!,Other!#REF!)", "Host", &delete, false).unwrap();
        assert_eq!(actual.text, "SUM(#REF!,Other!#REF!)"); assert_eq!(actual.invalidated, 0);
        let insert = ChartReferenceEdit::InsertRows { sheet: "Data".into(), at: 1, count: 1 };
        assert_eq!(edit_formula_text(input, "Host", &insert, false).unwrap().text, input);
    }
    #[test]
    fn native_full_axis_cuts_distinguish_finite_references_from_whole_axis_identity() {
        // whole-axis-preparation-r1 independent raw XML: exact name/cell/
        // series references. Finite cuts and full-axis cuts are not aliases.
        let cases = [
            ((2, 1), (2, 1_048_576), "Other", (6, 1), "Data!$C:$C", "Data!$1:$5", "SUM(A1:A2)", "Other!$F$2:$F$5", "Other!$F$3:$F$4"),
            ((2, 1), (2, 1_048_576), "Data", (6, 1), "Data!$B:$C", "Data!$1:$5", "SUM(A1:F2)", "Data!$F$2:$F$5", "Data!$F$3:$F$4"),
            ((1, 1), (16_384, 2), "Other", (1, 8), "Data!$B:$C", "Data!$3:$5", "SUM(Other!A8:B9)", "Data!$B$3:$B$5", "Data!$B$3:$B$4"),
        ];
        for (first, last, destination_sheet, destination, columns, rows, cell_formula, first_series, second_series) in cases {
            let edit = ChartReferenceEdit::MoveRange {
                source_sheet: "Data".into(),
                source: ReferenceRect { first: ReferenceCell { column: first.0, row: first.1 }, last: ReferenceCell { column: last.0, row: last.1 } },
                destination_sheet: destination_sheet.into(), destination: ReferenceCell { column: destination.0, row: destination.1 },
            };
            for (input, expected) in [
                ("Data!$B:$C", columns.to_owned()),
                ("Data!$1:$5", rows.to_owned()),
                ("SUM(Data!$B:$C)", format!("SUM({columns})")),
                ("SUM(Data!$1:$5)", format!("SUM({rows})")),
                ("SUM(A1:B2)", cell_formula.to_owned()),
                ("Data!$B$2:$B$5", first_series.to_owned()),
                ("Data!$B$3:$B$4", second_series.to_owned()),
                ("Data!$E$8:$E$9", "Data!$E$8:$E$9".to_owned()),
                ("Data!$H$10:$H$12", "Data!$H$10:$H$12".to_owned()),
            ] {
                assert_eq!(edit_formula_text(input, "Data", &edit, false).unwrap().text, expected,
                           "{first:?}:{last:?} to {destination_sheet} {destination:?}: {input}");
            }
        }
    }

    #[test]
    fn native_out_of_grid_a1_looking_names_keep_identity_while_native_cells_adjust() {
        // named-operands-preparation-r1: native Excel admitted each as a
        // defined name. Its definition follows Data!A1 -> Data!A2 through the
        // workbook owner, but each formula keeps the name, not a cell address.
        let edit = ChartReferenceEdit::InsertRows { sheet: "Data".into(), at: 1, count: 1 };
        for name in ["XFE1", "A1048577", "A0"] {
            let input = format!("SUM({name},Data!A1)");
            assert_eq!(edit_formula_text(&input, "Other", &edit, false).unwrap().text,
                       format!("SUM({name},Data!A2)"));
            // Native save canonicalized the attempted Data!NAME to [0]!NAME
            // even before the edit; retain that actual captured baseline.
            let qualified = format!("SUM([0]!{name},Data!A1)");
            assert_eq!(edit_formula_text(&qualified, "Other", &edit, false).unwrap().text,
                       format!("SUM([0]!{name},Data!A2)"));
        }
    }
    #[test]
    fn name_classification_preserves_lexical_prefixes_case_and_large_numeric_suffixes() {
        // Classifier/lexical unit expectations, not extra native captures.
        let edit = ChartReferenceEdit::InsertRows { sheet: "Data".into(), at: 1, count: 1 };
        let input = "=SUM(xfe1,'Data'!A0,Data!A99999999999999999999,Other!A1048577,Data!A1)";
        assert_eq!(edit_formula_text(input, "Other", &edit, false).unwrap().text,
                   "=SUM(xfe1,'Data'!A0,Data!A99999999999999999999,Other!A1048577,Data!A2)");
        assert!(is_out_of_grid_name_operand("XFE1"));
        assert!(is_out_of_grid_name_operand("A99999999999999999999"));
        assert!(!is_out_of_grid_name_operand("XFD1048576"));
        assert!(!is_out_of_grid_name_operand("$XFE$1"));
        assert!(!is_out_of_grid_name_operand("XFE1:A1"));
    }

    #[test]
    fn malformed_coordinate_fragments_preserve_bytes_without_suppressing_native_siblings() {
        let edit = ChartReferenceEdit::InsertRows { sheet: "Data".into(), at: 1, count: 1 };
        let prepared = PreparedReferenceEdit::new(&edit).unwrap();
        for (input, expected) in [
            ("SUM(Data!$A$0,Data!A1)", "SUM(Data!$A$0,Data!A2)"),
            ("SUM(Data!$XFE$1,Data!A1)", "SUM(Data!$XFE$1,Data!A2)"),
            ("SUM(Data!$A$99999999999999999999,Data!A1)", "SUM(Data!$A$99999999999999999999,Data!A2)"),
            ("SUM(Data!A1:A99999999999999999999,Data!A1)", "SUM(Data!A1:A99999999999999999999,Data!A2)"),
            ("SUM(Data!A1,Data!A2;", "SUM(Data!A2,Data!A3;"),
        ] {
            let actual = edit_prepared_formula_text(input, "Host", &prepared, false);
            assert_eq!(actual.text, expected); assert_eq!(actual.invalidated, 0);
        }
    }
    #[test]
    fn invalid_caller_edit_context_rejects_only_during_preparation() {
        for order in [vec![], vec!["Other".into()], vec!["Data".into(), "data".into()]] {
            assert!(PreparedReferenceEdit::new(&ChartReferenceEdit::DeleteSheet { sheet: "Data".into(), sheet_order: order }).is_err());
        }
        for edit in [
            ChartReferenceEdit::InsertRows { sheet: "Data".into(), at: 1, count: u32::MAX },
            ChartReferenceEdit::InsertColumns { sheet: "Data".into(), at: 1, count: u32::MAX },
            ChartReferenceEdit::InsertRows { sheet: "Data".into(), at: 1_048_576, count: 2 },
        ] { assert!(PreparedReferenceEdit::new(&edit).is_err()); }
        let edit = ChartReferenceEdit::DeleteSheet { sheet: "Data".into(), sheet_order: vec!["Data".into(), "Host".into()] };
        let prepared = PreparedReferenceEdit::new(&edit).unwrap();
        let actual = edit_prepared_formula_text("SUM(Data:Missing!A1,Data!A1)", "Host", &prepared, false);
        assert_eq!(actual.text, "SUM(Data:Missing!A1,#REF!)");
        assert_eq!(actual.invalidated, 1);
    }

}
