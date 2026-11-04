// c:pivotSource
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::{
        BytesStart,
        Event,
    },
};

use super::ExtensionList;
use crate::{
    reader::driver::{
        local_name,
        skip_element,
        try_xml_read_loop,
    },
    structs::Workbook,
    writer::driver::{
        write_end_tag,
        write_start_tag,
        write_text_node,
    },
};

/// The link of a `PivotChart` to its pivot table: `name` is
/// `[workbook.xlsx]Sheet!PivotTableName`, `format_id` ties the chart to the
/// `chartFormat` entries of that table.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct PivotSource {
    name:       String,
    format_id:  u32,
    #[cfg_attr(feature = "serde", serde(default))]
    extensions: Vec<ExtensionList>,
}

impl PivotSource {
    #[must_use]
    pub fn new<S: Into<String>>(name: S, format_id: u32) -> Self {
        Self {
            name: name.into(),
            format_id,
            extensions: Vec::new(),
        }
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn set_name<S: Into<String>>(&mut self, value: S) -> &mut Self {
        self.name = value.into();
        self
    }

    #[must_use]
    pub fn format_id(&self) -> u32 {
        self.format_id
    }

    pub fn set_format_id(&mut self, value: u32) -> &mut Self {
        self.format_id = value;
        self
    }

    /// The pivot table name after the sheet part of `name`
    /// (`[book.xlsx]Sheet!PivotTable1` gives `PivotTable1`).
    #[must_use]
    pub fn table_name(&self) -> &str {
        split_name(&self.name).map_or("", |(_, _, table)| table)
    }

    /// The sheet that holds the pivot table.
    #[must_use]
    pub fn sheet_name(&self) -> Option<&str> {
        split_name(&self.name).map(|(_, sheet, _)| sheet)
    }

    /// Points the link at the sheet the pivot table moved to, keeping the
    /// workbook part and the table name.
    pub fn set_sheet_name(&mut self, sheet: &str) -> &mut Self {
        if let Some((book, _, table)) = split_name(&self.name) {
            self.name = format!("{book}{sheet}!{table}");
        }
        self
    }

    /// Follows a renamed sheet: when the pivot table sits on `old`, the link
    /// names `new` afterwards. Sheet names compare case-insensitively.
    pub(crate) fn rename_sheet(&mut self, old: &str, new: &str) {
        if self
            .sheet_name()
            .is_some_and(|sheet| sheet.eq_ignore_ascii_case(old))
        {
            self.set_sheet_name(new);
        }
    }

    /// The extension lists in source order (the schema allows several).
    #[must_use]
    pub fn extension_lists(&self) -> &[ExtensionList] {
        &self.extensions
    }

    pub fn extension_lists_mut(&mut self) -> &mut [ExtensionList] {
        &mut self.extensions
    }

    pub fn set_extension_lists(&mut self, value: Vec<ExtensionList>) -> &mut Self {
        self.extensions = value;
        self
    }

    pub(crate) fn strip_export_parts(&mut self) {
        self.extensions.clear();
    }

    pub(crate) fn restore_export_parts(
        &mut self,
        previous: &Self,
        report: &mut super::RestoreReport,
    ) -> usize {
        super::chart_metadata_restoration::record_supplied_extension(
            !self.extensions.is_empty(),
            "pivotSource",
            report,
        );
        let restored = usize::from(self.extensions.is_empty() && !previous.extensions.is_empty());
        self.extensions.clone_from(&previous.extensions);
        restored
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        owner: &BytesStart,
        session: &crate::reader::xlsx::SourceCacheSession,
    ) -> Result<(), crate::XlsxError> {
        let owner_end = owner.name().as_ref().to_vec();
        let _scope_guard = session.enter_child_scope(owner)?;
        let (mut has_name, mut has_format) = (false, false);
        try_xml_read_loop!(
            reader,
            Event::Start(ref e) => match if session.accepts_child_namespace(e,super::extension_registry::NS_C)?{local_name(e.name().into_inner())}else{b""} {
                b"name" => {
                    let mut buf = Vec::new();
                    let text = reader.read_text_into(e.name(), &mut buf)?;
                    self.name = crate::helper::utils::try_unescape_xml_text(&text)?;
                    has_name = true;
                }
                b"fmtId" => {
                    self.format_id = read_format_id(e)?;
                    has_format = true;
                    crate::reader::driver::consume_empty_element(reader, e)?;
                }
                b"extLst" => {
                    self.extensions.push(ExtensionList::read(reader, e, false, session)?);
                }
                _ => skip_element(reader, e)?,
            },
            Event::Empty(ref e) => match if session.accepts_child_namespace(e,super::extension_registry::NS_C)?{local_name(e.name().into_inner())}else{b""} {
                b"fmtId" => {
                    self.format_id = read_format_id(e)?;
                    has_format = true;
                }
                b"extLst" => {
                    self.extensions.push(ExtensionList::read(reader, e, true, session)?);
                }
                _ => (),
            },
            Event::End(ref e) => {
                if e.name().as_ref() == owner_end {
                    if !has_name || !has_format {
                        return Err(crate::xml::xml_error(
                            "c:pivotSource requires c:name and c:fmtId",
                        ));
                    }
                    return Ok(());
                }
            },
            Event::Eof => return Err(crate::xml::xml_error("Error: Could not find c:pivotSource end element")),
        );
    }

    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        wb: &Workbook,
    ) -> Result<(), crate::XlsxError> {
        // c:pivotSource
        write_start_tag(writer, "c:pivotSource", vec![], false);
        write_start_tag(writer, "c:name", vec![], false);
        write_text_node(writer, &self.name);
        write_end_tag(writer, "c:name");
        write_start_tag(
            writer,
            "c:fmtId",
            vec![("val", self.format_id.to_string()).into()],
            true,
        );
        for list in &self.extensions {
            list.write_for_owner(writer, Some(wb), "pivotSource", None)?;
        }
        write_end_tag(writer, "c:pivotSource");
        Ok(())
    }
}

fn read_format_id(element: &BytesStart) -> Result<u32, crate::XlsxError> {
    crate::xml::parse_u32(&crate::reader::driver::require_attribute(element, b"val")?)
        .map_err(crate::xml::xml_error)
}

/// Splits `[book]Sheet!Table` into the bracketed prefix (kept with its
/// brackets), the sheet name and the table name. Excel writes the sheet name
/// as it is, without quotes, even when it holds spaces (measured with Excel
/// 16.0 for the sheet "Sales 2026").
fn split_name(name: &str) -> Option<(&str, &str, &str)> {
    let book_end = if name.starts_with('[') {
        name.find(']')? + 1
    } else {
        0
    };
    let (book, rest) = name.split_at(book_end);
    let bang = rest.rfind('!')?;
    Some((book, &rest[..bang], &rest[bang + 1..]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_parts_follow_excels_unquoted_form() {
        let plain = PivotSource::new("[book.xlsx]Pivot!PivotTable1", 0);
        assert_eq!(plain.sheet_name(), Some("Pivot"));
        assert_eq!(plain.table_name(), "PivotTable1");
        let spaced = PivotSource::new("[book.xlsx]Sales 2026!Table 2", 3);
        assert_eq!(spaced.sheet_name(), Some("Sales 2026"));
        assert_eq!(spaced.table_name(), "Table 2");
        let bang = PivotSource::new("[book.xlsx]Q1!Q2!PivotTable1", 0);
        assert_eq!(bang.sheet_name(), Some("Q1!Q2"));
        let mut moved = plain;
        moved.set_sheet_name("Sales 2026");
        assert_eq!(moved.name(), "[book.xlsx]Sales 2026!PivotTable1");
        assert_eq!(PivotSource::new("opaque", 0).sheet_name(), None);
    }
}
