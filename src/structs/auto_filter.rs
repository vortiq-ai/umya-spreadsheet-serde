// autoFilter
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use super::Range;
use crate::{
    FilterColumn,
    traits::AdjustmentCoordinate,
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct AutoFilter {
    range:          Range,
    #[cfg_attr(feature = "serde", serde(default))]
    revision_uid:   Option<String>,
    #[cfg_attr(feature = "serde", serde(default))]
    filter_columns: Vec<FilterColumn>,
    #[cfg_attr(feature = "serde", serde(default))]
    sort_state:     Option<crate::structs::filter_rule::FilterSortState>,
    #[cfg_attr(feature = "serde", serde(default))]
    extensions:     Option<crate::drawing::charts::OpaqueElement>,
}

impl AutoFilter {
    #[must_use]
    pub fn sort_state(&self) -> Option<&crate::structs::filter_rule::FilterSortState> {
        self.sort_state.as_ref()
    }

    pub fn sort_state_mut(&mut self) -> &mut Option<crate::structs::filter_rule::FilterSortState> {
        &mut self.sort_state
    }

    #[inline]
    #[must_use]
    pub fn range(&self) -> &Range {
        &self.range
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use range()")]
    pub fn get_range(&self) -> &Range {
        self.range()
    }

    #[inline]
    pub fn range_mut(&mut self) -> &mut Range {
        &mut self.range
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use range_mut()")]
    pub fn get_range_mut(&mut self) -> &mut Range {
        self.range_mut()
    }

    #[inline]
    pub fn set_range<S: Into<String>>(&mut self, value: S) {
        let mut range = Range::default();
        range.set_range(value.into());
        self.range = range;
    }

    #[inline]
    #[must_use]
    pub fn filter_columns(&self) -> &[FilterColumn] {
        &self.filter_columns
    }

    #[inline]
    pub fn filter_columns_mut(&mut self) -> &mut Vec<FilterColumn> {
        &mut self.filter_columns
    }

    #[inline]
    pub fn add_filter_column(&mut self, value: FilterColumn) -> &mut Self {
        self.filter_columns.push(value);
        self
    }

    fn range_is_removed(
        range: &Range,
        root_col_num: u32,
        offset_col_num: u32,
        root_row_num: u32,
        offset_row_num: u32,
    ) -> bool {
        let covered = |start: Option<u32>, end: Option<u32>, root: u32, count: u32| {
            start.is_some_and(|start| {
                root != 0
                    && count != 0
                    && root <= start
                    && u64::from(end.unwrap_or(start)) < u64::from(root) + u64::from(count)
            })
        };
        covered(
            range
                .coordinate_start_col()
                .map(super::ColumnReference::num),
            range.coordinate_end_col().map(super::ColumnReference::num),
            root_col_num,
            offset_col_num,
        ) || covered(
            range.coordinate_start_row().map(super::RowReference::num),
            range.coordinate_end_row().map(super::RowReference::num),
            root_row_num,
            offset_row_num,
        )
    }

    fn adjust_removed_range(
        range: &mut Range,
        root_col_num: u32,
        offset_col_num: u32,
        root_row_num: u32,
        offset_row_num: u32,
    ) {
        // A partly deleted filter/sort reference keeps its surviving cells.
        // Removed endpoints clip to the deletion boundary instead of moving
        // backwards by the entire deleted count.
        let adjusted_boundary = |value: u32, root: u32, count: u32, trailing: bool| {
            if count == 0 || value < root {
                value
            } else if u64::from(value) >= u64::from(root) + u64::from(count) {
                value - count
            } else if trailing {
                root.saturating_sub(1)
            } else {
                root
            }
        };
        if let Some(column) = range.coordinate_start_col_mut() {
            column.set_num(adjusted_boundary(
                column.num(),
                root_col_num,
                offset_col_num,
                false,
            ));
        }
        if let Some(column) = range.coordinate_end_col_mut() {
            column.set_num(adjusted_boundary(
                column.num(),
                root_col_num,
                offset_col_num,
                true,
            ));
        }
        if let Some(row) = range.coordinate_start_row_mut() {
            row.set_num(adjusted_boundary(
                row.num(),
                root_row_num,
                offset_row_num,
                false,
            ));
        }
        if let Some(row) = range.coordinate_end_row_mut() {
            row.set_num(adjusted_boundary(
                row.num(),
                root_row_num,
                offset_row_num,
                true,
            ));
        }
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        element: &BytesStart,
        empty: bool,
    ) -> Result<(), String> {
        self.set_attributes_scoped(
            reader,
            element,
            empty,
            &crate::drawing::charts::NamespaceScope::with_bindings([(
                None,
                "http://schemas.openxmlformats.org/spreadsheetml/2006/main".into(),
            )]),
        )
    }

    pub(crate) fn set_attributes_scoped<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        element: &BytesStart,
        empty: bool,
        parent: &crate::drawing::charts::NamespaceScope,
    ) -> Result<(), String> {
        let node = crate::drawing::charts::OpaqueElement::read(reader, element, empty, parent)?;
        self.read_element(&node)
    }

    fn read_element(&mut self, node: &crate::drawing::charts::OpaqueElement) -> Result<(), String> {
        use crate::xml::{
            attributes,
            direct_children,
            expect,
            required,
        };
        const NS: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
        expect(node, NS, "autoFilter")?;
        attributes(
            node,
            &["ref"],
            &[
                (crate::helper::const_str::MC_NS, "Ignorable"),
                (
                    "http://schemas.microsoft.com/office/spreadsheetml/2014/revision",
                    "uid",
                ),
            ],
        )?;
        let mut result = Self::default();
        result.set_range(required(node, "ref")?);
        result.revision_uid = crate::xml::attribute(
            node,
            "http://schemas.microsoft.com/office/spreadsheetml/2014/revision",
            "uid",
        )?
        .map(str::to_owned);
        for child in direct_children(node)? {
            if child.name.is(NS, "filterColumn") {
                result
                    .filter_columns
                    .push(FilterColumn::from_element(&child)?);
            } else if child.name.is(NS, "sortState") {
                if result.sort_state.is_some() {
                    return Err("duplicate auto-filter sort state".into());
                }
                result.sort_state =
                    Some(crate::structs::filter_rule::FilterSortState::read(&child)?);
            } else if child.name.is(NS, "extLst") {
                if result.extensions.is_some() {
                    return Err("duplicate auto-filter extensions".into());
                }
                result.extensions = Some(child);
            } else {
                return Err("unknown native auto-filter child".into());
            }
        }
        *self = result;
        Ok(())
    }

    pub fn from_xml(bytes: &[u8]) -> Result<Self, String> {
        let mut result = Self::default();
        result.read_element(&crate::xml::root(bytes)?)?;
        Ok(result)
    }

    pub fn to_xml(&self) -> Result<Vec<u8>, String> {
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        self.write_fragment(&mut writer, true)
            .map_err(|error| error.to_string())?;
        Ok(writer.into_inner().into_inner())
    }

    #[inline]
    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
    ) -> Result<(), crate::XlsxError> {
        self.write_fragment(writer, false)
    }

    fn write_fragment(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        standalone: bool,
    ) -> Result<(), crate::XlsxError> {
        let empty_flg = self.filter_columns.is_empty()
            && self.sort_state.is_none()
            && self.extensions.is_none();

        // autoFilter
        let mut attributes: crate::structs::AttrCollection =
            vec![("ref", self.range.range()).into()];
        if let Some(uid) = &self.revision_uid {
            attributes.push(
                (
                    "xmlns:xr",
                    "http://schemas.microsoft.com/office/spreadsheetml/2014/revision",
                )
                    .into(),
            );
            attributes.push(("xr:uid", uid.as_str()).into());
        }
        if standalone {
            attributes.push(
                (
                    "xmlns",
                    "http://schemas.openxmlformats.org/spreadsheetml/2006/main",
                )
                    .into(),
            );
        }
        write_start_tag(writer, "autoFilter", attributes, empty_flg);
        if empty_flg {
            return Ok(());
        }

        for obj in &self.filter_columns {
            obj.write_to(writer)?;
        }

        if let Some(sort) = &self.sort_state {
            sort.write_to(writer).map_err(crate::xml::xml_error)?;
        }
        if let Some(extension) = &self.extensions {
            extension
                .write_to(
                    writer,
                    &crate::drawing::charts::NamespaceScope::with_bindings([(
                        None,
                        "http://schemas.openxmlformats.org/spreadsheetml/2006/main".into(),
                    )]),
                )
                .map_err(crate::xml::xml_error)?;
        }
        write_end_tag(writer, "autoFilter");
        Ok(())
    }
}
impl AdjustmentCoordinate for AutoFilter {
    #[inline]
    fn adjustment_insert_coordinate(
        &mut self,
        root_col_num: u32,
        offset_col_num: u32,
        root_row_num: u32,
        offset_row_num: u32,
    ) {
        if offset_col_num != 0 {
            if let Some(start) = self.range.coordinate_start_col() {
                let start = start.num();
                if root_col_num > start {
                    for column in &mut self.filter_columns {
                        if start + column.column_id() >= root_col_num {
                            column.set_column_id(column.column_id() + offset_col_num);
                        }
                    }
                }
            }
        }
        self.range.adjustment_insert_coordinate(
            root_col_num,
            offset_col_num,
            root_row_num,
            offset_row_num,
        );
        if let Some(sort) = &mut self.sort_state {
            sort.reference.adjustment_insert_coordinate(
                root_col_num,
                offset_col_num,
                root_row_num,
                offset_row_num,
            );
            for condition in &mut sort.conditions {
                condition.reference.adjustment_insert_coordinate(
                    root_col_num,
                    offset_col_num,
                    root_row_num,
                    offset_row_num,
                );
            }
        }
    }

    #[inline]
    fn adjustment_remove_coordinate(
        &mut self,
        root_col_num: u32,
        offset_col_num: u32,
        root_row_num: u32,
        offset_row_num: u32,
    ) {
        let start = self
            .range
            .coordinate_start_col()
            .map(super::ColumnReference::num);
        Self::adjust_removed_range(
            &mut self.range,
            root_col_num,
            offset_col_num,
            root_row_num,
            offset_row_num,
        );
        if offset_col_num != 0 {
            if let (Some(start), Some(new_start)) = (
                start,
                self.range
                    .coordinate_start_col()
                    .map(super::ColumnReference::num),
            ) {
                let end = root_col_num + offset_col_num;
                self.filter_columns.retain_mut(|column| {
                    let absolute = start + column.column_id();
                    if (root_col_num..end).contains(&absolute) {
                        return false;
                    }
                    let adjusted = if absolute >= end {
                        absolute - offset_col_num
                    } else {
                        absolute
                    };
                    column.set_column_id(adjusted - new_start);
                    true
                });
            }
        }
        if self.sort_state.as_ref().is_some_and(|sort| {
            Self::range_is_removed(
                &sort.reference,
                root_col_num,
                offset_col_num,
                root_row_num,
                offset_row_num,
            )
        }) {
            self.sort_state = None;
        } else if let Some(sort) = &mut self.sort_state {
            Self::adjust_removed_range(
                &mut sort.reference,
                root_col_num,
                offset_col_num,
                root_row_num,
                offset_row_num,
            );
            sort.conditions.retain_mut(|condition| {
                if Self::range_is_removed(
                    &condition.reference,
                    root_col_num,
                    offset_col_num,
                    root_row_num,
                    offset_row_num,
                ) {
                    return false;
                }
                Self::adjust_removed_range(
                    &mut condition.reference,
                    root_col_num,
                    offset_col_num,
                    root_row_num,
                    offset_row_num,
                );
                true
            });
        }
    }

    #[inline]
    fn is_remove_coordinate(
        &self,
        root_col_num: u32,
        offset_col_num: u32,
        root_row_num: u32,
        offset_row_num: u32,
    ) -> bool {
        Self::range_is_removed(
            &self.range,
            root_col_num,
            offset_col_num,
            root_row_num,
            offset_row_num,
        )
    }
}
