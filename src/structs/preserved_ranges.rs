//! Structural edits (row and column insertion and removal) for the range
//! references inside preserved extension XML: the `xm:sqref` ranges of x14
//! conditional formats and sparklines, the `sqref` attribute of protected
//! ranges and ignored errors, and the `xm:f` formulas of the same extensions.
//!
//! Excel shifts these references with the cells and removes an entry whose
//! range is removed; the same rules apply here, using the ranges and the
//! formula editor the rest of the workbook uses.

use crate::{
    drawing::charts::{
        OpaqueElement,
        OpaqueToken,
        QName,
    },
    helper::{
        const_str::SHEET_MAIN_NS,
        coordinate::index_from_coordinate,
    },
    structs::{
        ColumnReference,
        Range,
        RowReference,
        SequenceOfReferences,
    },
    traits::AdjustmentCoordinate,
};

/// Namespace of the `xm:` elements (`xm:sqref`, `xm:f`) inside extensions.
pub(crate) const XM_NS: &str = "http://schemas.microsoft.com/office/excel/2006/main";
/// Namespace of the x14 extension elements.
const X14_NS: &str = "http://schemas.microsoft.com/office/spreadsheetml/2009/9/main";

/// A structural edit of the host sheet.
#[derive(Clone, Copy, Debug)]
pub(crate) struct LocalEdit {
    pub(crate) insert:    bool,
    pub(crate) root_col:  u32,
    pub(crate) col_count: u32,
    pub(crate) root_row:  u32,
    pub(crate) row_count: u32,
}

/// Parse a whitespace separated list of ranges (the XML list type: space,
/// tab, line feed and carriage return separate items) without ever reaching
/// the asserting range parser: every item must be one or two coordinates that
/// the range writes back unchanged.
fn parse_range_list(text: &str) -> Result<SequenceOfReferences, String> {
    let mut references = SequenceOfReferences::default();
    for item in text
        .split([' ', '\t', '\n', '\r'])
        .filter(|item| !item.is_empty())
    {
        let parts: Vec<&str> = item.split(':').collect();
        // Both ends of a range are cells, or both are columns, or both are
        // rows.
        let kinds: Vec<(bool, bool, bool)> = parts
            .iter()
            .map(|part| {
                let (col, row, ..) = index_from_coordinate(part);
                (col.is_some(), row.is_some(), row == Some(0))
            })
            .collect();
        let valid = matches!(parts.len(), 1 | 2)
            && kinds
                .iter()
                .all(|(has_col, has_row, zero)| (*has_col || *has_row) && !zero)
            && kinds
                .windows(2)
                .all(|pair| pair[0].0 == pair[1].0 && pair[0].1 == pair[1].1);
        if !valid {
            return Err(format!("'{item}' is not a cell range"));
        }
        let mut range = Range::default();
        range.set_range(item);
        if range.range() != item {
            return Err(format!("'{item}' is not a cell range"));
        }
        references.range_collection_mut().push(range);
    }
    if references.range_collection().is_empty() {
        return Err("the range list is empty".to_string());
    }
    Ok(references)
}

/// The reference text after the edit, or `None` when the edit removes it.
fn adjust_sqref(text: &str, edit: &LocalEdit) -> Result<Option<String>, String> {
    let mut references = parse_range_list(text)?;
    if edit.insert {
        references.adjustment_insert_coordinate(
            edit.root_col,
            edit.col_count,
            edit.root_row,
            edit.row_count,
        );
    } else if remove_lines(&mut references, edit) {
        return Ok(None);
    }
    Ok(Some(references.get_sqref()))
}

/// Follow the removal of rows or columns in a list of ranges as Excel does: a
/// range whose rows or columns are all removed goes with them, the others
/// shrink. Reports whether no range is left.
pub(crate) fn remove_lines(references: &mut SequenceOfReferences, edit: &LocalEdit) -> bool {
    references
        .range_collection_mut()
        .retain(|range| !is_wholly_removed(range, edit));
    for range in references.range_collection_mut() {
        shrink_range(range, edit);
        range.collapse_single_cell();
    }
    references.range_collection().is_empty()
}

/// The coordinate after removing `count` lines from `root`: lines after the
/// block move up; an edge inside the block moves to the nearest line that
/// is kept (the start to the line after the block, the end to the line
/// before it).
fn shrink_coordinate(num: u32, root: u32, count: u32, is_end: bool) -> u32 {
    if count == 0 || root == 0 || num < root {
        num
    } else if num >= root + count {
        num - count
    } else if is_end {
        root - 1
    } else {
        root
    }
}

fn shrink_range(range: &mut Range, edit: &LocalEdit) {
    if let Some(row) = range.coordinate_start_row_mut() {
        row.set_num(shrink_coordinate(
            row.num(),
            edit.root_row,
            edit.row_count,
            false,
        ));
    }
    if let Some(row) = range.coordinate_end_row_mut() {
        row.set_num(shrink_coordinate(
            row.num(),
            edit.root_row,
            edit.row_count,
            true,
        ));
    }
    if let Some(col) = range.coordinate_start_col_mut() {
        col.set_num(shrink_coordinate(
            col.num(),
            edit.root_col,
            edit.col_count,
            false,
        ));
    }
    if let Some(col) = range.coordinate_end_col_mut() {
        col.set_num(shrink_coordinate(
            col.num(),
            edit.root_col,
            edit.col_count,
            true,
        ));
    }
}

fn is_wholly_removed(range: &Range, edit: &LocalEdit) -> bool {
    let covered = |start: Option<u32>, end: Option<u32>, root: u32, count: u32| {
        count != 0
            && root != 0
            && match (start, end.or(start)) {
                (Some(start), Some(end)) => start >= root && end < root + count,
                _ => false,
            }
    };
    covered(
        range.coordinate_start_row().map(RowReference::num),
        range.coordinate_end_row().map(RowReference::num),
        edit.root_row,
        edit.row_count,
    ) || covered(
        range.coordinate_start_col().map(ColumnReference::num),
        range.coordinate_end_col().map(ColumnReference::num),
        edit.root_col,
        edit.col_count,
    )
}

fn is_named(name: &QName, namespace: &str, local: &str) -> bool {
    name.namespace_uri == namespace && name.local_name == local
}

/// Index of the matching end token of every start token.
fn matching_ends(tape: &[OpaqueToken]) -> Vec<Option<usize>> {
    let mut ends = vec![None; tape.len()];
    let mut stack = Vec::new();
    for (index, token) in tape.iter().enumerate() {
        match token {
            OpaqueToken::Start(_) => stack.push(index),
            OpaqueToken::End => {
                if let Some(start) = stack.pop() {
                    ends[start] = Some(index);
                }
            }
            _ => (),
        }
    }
    ends
}

/// Entries that carry their range in the `sqref` attribute (main namespace or
/// x14).
fn has_sqref_attribute(name: &QName) -> bool {
    [SHEET_MAIN_NS, X14_NS].iter().any(|namespace| {
        is_named(name, namespace, "protectedRange") || is_named(name, namespace, "ignoredError")
    })
}

/// Containers and the entry each needs to stay meaningful.
const REQUIRED_CHILD: [(&str, &str); 7] = [
    ("sparklines", "sparkline"),
    ("sparklineGroup", "sparklines"),
    ("sparklineGroups", "sparklineGroup"),
    ("conditionalFormattings", "conditionalFormatting"),
    ("protectedRanges", "protectedRange"),
    ("ignoredErrors", "ignoredError"),
    ("ext", ""),
];

fn required_child(name: &QName) -> Option<&'static str> {
    if name.namespace_uri != X14_NS && name.namespace_uri != SHEET_MAIN_NS {
        return None;
    }
    REQUIRED_CHILD
        .iter()
        .find(|(container, _)| *container == name.local_name)
        .map(|(_, child)| *child)
}

/// Direct child start tokens of the element opened at `start` (or of the root
/// when `start` is `None`), as `(token index, name)`.
fn direct_children<'a>(
    tape: &'a [OpaqueToken],
    ends: &[Option<usize>],
    start: Option<usize>,
) -> Vec<(usize, &'a QName)> {
    let (mut index, limit) = match start {
        Some(start) => (start + 1, ends[start].unwrap_or(tape.len())),
        None => (0, tape.len()),
    };
    let mut children = Vec::new();
    while index < limit {
        if let OpaqueToken::Start(child) = &tape[index] {
            children.push((index, &child.name));
            index = ends[index].map_or(limit, |end| end + 1);
        } else {
            index += 1;
        }
    }
    children
}

/// Remove containers that lost their entries, innermost first, until stable.
fn prune_empty_containers(element: &mut OpaqueElement) {
    loop {
        let ends = matching_ends(&element.children);
        let mut doomed = None;
        for (index, token) in element.children.iter().enumerate() {
            let OpaqueToken::Start(start) = token else {
                continue;
            };
            let Some(required) = required_child(&start.name) else {
                continue;
            };
            if required.is_empty() {
                continue;
            }
            let has_entry = direct_children(&element.children, &ends, Some(index))
                .iter()
                .any(|(_, name)| name.local_name == required);
            if !has_entry {
                doomed = ends[index].map(|end| (index, end));
                break;
            }
        }
        match doomed {
            Some((start, end)) => {
                element.children.drain(start..=end);
            }
            None => return,
        }
    }
}

/// Apply a structural edit of the host sheet to the ranges of `element` and
/// report whether nothing meaningful is left of it, so the owner drops it. A
/// range list that is not valid is left as it is and described in `conflicts`.
pub(crate) fn adjust_local_ranges(
    element: &mut OpaqueElement,
    edit: &LocalEdit,
    conflicts: &mut Vec<String>,
) -> bool {
    let ends = matching_ends(&element.children);
    let mut doomed: Vec<(usize, usize)> = Vec::new();
    let mut root_removed = false;
    let mut stack: Vec<usize> = Vec::new();
    for index in 0..element.children.len() {
        let in_sqref = stack
            .last()
            .is_some_and(|open| is_xm_start(&element.children, *open, "sqref"));
        match &mut element.children[index] {
            OpaqueToken::Start(start) => {
                if has_sqref_attribute(&start.name) {
                    if let Some((_, value)) = start.attributes.iter_mut().find(|(name, _)| {
                        name.namespace_uri.is_empty() && name.local_name == "sqref"
                    }) {
                        match adjust_sqref(value, edit) {
                            Ok(Some(text)) => *value = text,
                            Ok(None) => {
                                if let Some(end) = ends[index] {
                                    doomed.push((index, end));
                                }
                            }
                            Err(reason) => conflicts.push(cannot_follow(value, &reason)),
                        }
                    }
                }
                stack.push(index);
            }
            OpaqueToken::End => {
                stack.pop();
            }
            OpaqueToken::Text(text) => {
                if !in_sqref {
                    continue;
                }
                match adjust_sqref(text, edit) {
                    Err(reason) => conflicts.push(cannot_follow(text, &reason)),
                    Ok(Some(new)) => *text = new,
                    Ok(None) => {
                        // The entry owning the removed range goes with it.
                        if let Some(entry) = stack.len().checked_sub(2).map(|at| stack[at]) {
                            if let Some(end) = ends[entry] {
                                doomed.push((entry, end));
                            }
                        } else {
                            root_removed = true;
                        }
                    }
                }
            }
            _ => (),
        }
    }
    doomed.sort_unstable();
    doomed.dedup();
    for (start, end) in doomed.into_iter().rev() {
        if end < element.children.len() && start <= end {
            element.children.drain(start..=end);
        }
    }
    prune_empty_containers(element);
    let own_children = direct_children(&element.children, &matching_ends(&element.children), None);
    root_removed
        || match required_child(&element.name) {
            Some("") => own_children.is_empty(),
            Some(required) => !own_children
                .iter()
                .any(|(_, name)| name.local_name == required),
            None => false,
        }
}

fn is_xm_start(tape: &[OpaqueToken], index: usize, local: &str) -> bool {
    matches!(&tape[index], OpaqueToken::Start(start) if is_named(&start.name, XM_NS, local))
}

/// The text of every `xm:f` formula in `element`, in document order.
pub(crate) fn formula_texts(element: &OpaqueElement) -> Vec<String> {
    let mut texts = Vec::new();
    let mut open_formula = false;
    for token in &element.children {
        match token {
            OpaqueToken::Start(start) => open_formula = is_named(&start.name, XM_NS, "f"),
            OpaqueToken::Text(text) if open_formula => texts.push(text.clone()),
            OpaqueToken::End => open_formula = false,
            _ => (),
        }
    }
    texts
}

/// The conflict recorded for a range list a structural edit cannot follow.
fn cannot_follow(list: &str, reason: &str) -> String {
    format!("the preserved range list '{list}' cannot follow a row or column edit: {reason}")
}

/// Whether a formula holds a `#REF!` error value: the literal outside quoted
/// sheet names (`'#REF!'!C2` is a valid reference to a sheet named `#REF!`)
/// and outside string literals.
fn is_reference_error(formula: &str) -> bool {
    let mut quote: Option<char> = None;
    let mut rest = formula;
    while let Some(c) = rest.chars().next() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => (),
            None => {
                if c == '\'' || c == '"' {
                    quote = Some(c);
                } else if rest.starts_with("#REF!") {
                    return true;
                }
            }
        }
        rest = &rest[c.len_utf8()..];
    }
    false
}

/// Replace the text of the `xm:f` formulas of `element`; `texts` is parallel to
/// [`formula_texts`] and `None` keeps a formula. A sparkline whose source
/// became a reference error keeps its location and loses the source formula,
/// as Excel writes it.
pub(crate) fn set_formula_texts(element: &mut OpaqueElement, texts: &[Option<String>]) {
    let ends = matching_ends(&element.children);
    // Per formula, in order: the tokens of its element and whether a sparkline
    // owns it.
    let mut formulas: Vec<(usize, bool)> = Vec::new();
    let mut stack: Vec<usize> = Vec::new();
    for (index, token) in element.children.iter().enumerate() {
        match token {
            OpaqueToken::Start(start) => {
                if is_named(&start.name, XM_NS, "f") {
                    let owned_by_sparkline = stack.last().is_some_and(|parent| {
                        matches!(&element.children[*parent],
                            OpaqueToken::Start(owner) if is_named(&owner.name, X14_NS, "sparkline"))
                    });
                    formulas.push((index, owned_by_sparkline));
                }
                stack.push(index);
            }
            OpaqueToken::End => {
                stack.pop();
            }
            _ => (),
        }
    }
    let mut doomed = Vec::new();
    for (position, (start, owned_by_sparkline)) in formulas.into_iter().enumerate() {
        let Some(Some(new)) = texts.get(position) else {
            continue;
        };
        if let Some(OpaqueToken::Text(text)) = element.children.get_mut(start + 1) {
            text.clone_from(new);
        }
        if owned_by_sparkline && is_reference_error(new) {
            if let Some(end) = ends[start] {
                doomed.push((start, end));
            }
        }
    }
    for (start, end) in doomed.into_iter().rev() {
        element.children.drain(start..=end);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drawing::charts::NamespaceScope;

    fn read(xml: &str) -> OpaqueElement {
        let mut reader = quick_xml::Reader::from_str(xml);
        loop {
            match reader.read_event().unwrap() {
                quick_xml::events::Event::Start(start) => {
                    return OpaqueElement::read(
                        &mut reader,
                        &start,
                        false,
                        &NamespaceScope::default(),
                    )
                    .unwrap();
                }
                quick_xml::events::Event::Eof => panic!("no element"),
                _ => (),
            }
        }
    }

    fn written(element: &OpaqueElement) -> String {
        let mut writer = quick_xml::Writer::new(std::io::Cursor::new(Vec::new()));
        element
            .write_to(&mut writer, &NamespaceScope::default())
            .unwrap();
        String::from_utf8(writer.into_inner().into_inner()).unwrap()
    }

    const SPARKLINES: &str = r#"<ext xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:x14="http://schemas.microsoft.com/office/spreadsheetml/2009/9/main" xmlns:xm="http://schemas.microsoft.com/office/excel/2006/main" uri="{05C60535-1F16-4fd2-B633-F4F36F0B64E0}"><x14:sparklineGroups><x14:sparklineGroup><x14:sparklines><x14:sparkline><xm:f>Data!C2:C2</xm:f><xm:sqref>F2</xm:sqref></x14:sparkline><x14:sparkline><xm:f>Data!C3:C3</xm:f><xm:sqref>F3</xm:sqref></x14:sparkline></x14:sparklines></x14:sparklineGroup></x14:sparklineGroups></ext>"#;

    #[test]
    fn insert_shifts_sparkline_cells() {
        let mut element = read(SPARKLINES);
        let removed = adjust_local_ranges(
            &mut element,
            &LocalEdit {
                insert:    true,
                root_col:  0,
                col_count: 0,
                root_row:  3,
                row_count: 2,
            },
            &mut Vec::new(),
        );
        assert!(!removed);
        let xml = written(&element);
        assert!(xml.contains("<xm:sqref>F2</xm:sqref>"), "{xml}");
        assert!(xml.contains("<xm:sqref>F5</xm:sqref>"), "{xml}");
    }

    #[test]
    fn remove_drops_the_sparkline_of_a_removed_cell_and_empty_containers() {
        let mut element = read(SPARKLINES);
        let edit = LocalEdit {
            insert:    false,
            root_col:  0,
            col_count: 0,
            root_row:  2,
            row_count: 1,
        };
        assert!(!adjust_local_ranges(&mut element, &edit, &mut Vec::new()));
        let xml = written(&element);
        assert!(!xml.contains("Data!C2:C2"), "{xml}");
        assert!(xml.contains("<xm:sqref>F2</xm:sqref>"), "{xml}");
        let second = LocalEdit {
            root_row: 2,
            ..edit
        };
        assert!(adjust_local_ranges(&mut element, &second, &mut Vec::new()));
    }

    #[test]
    fn range_lists_split_on_every_xml_whitespace_and_reject_malformed_items() {
        let references = parse_range_list("A1:A2\tC1:C2\nE5 \r\nG7").unwrap();
        assert_eq!(references.get_sqref(), "A1:A2 C1:C2 E5 G7");
        for bad in [
            "A1:A2:A3",
            "a1",
            "A0",
            "A99999999999",
            "1A",
            "",
            "  ",
            "A1:",
        ] {
            assert!(parse_range_list(bad).is_err(), "{bad:?} must be rejected");
        }
    }

    #[test]
    fn reference_errors_are_found_outside_quoted_names_only() {
        assert!(is_reference_error("Data!#REF!"));
        assert!(is_reference_error("#REF!"));
        assert!(is_reference_error("SUM(#REF!:C3)"));
        assert!(!is_reference_error("'#REF!'!C2:C2"));
        assert!(!is_reference_error("'it''s #REF!'!A1"));
        assert!(!is_reference_error("\"#REF!\""));
        assert!(!is_reference_error("Data!C2:C2"));
    }

    #[test]
    fn formula_texts_are_listed_and_replaced_in_order() {
        let mut element = read(SPARKLINES);
        assert_eq!(formula_texts(&element), vec!["Data!C2:C2", "Data!C3:C3"]);
        set_formula_texts(&mut element, &[None, Some("Renamed!C3:C3".into())]);
        assert_eq!(formula_texts(&element), vec!["Data!C2:C2", "Renamed!C3:C3"]);
    }
}
