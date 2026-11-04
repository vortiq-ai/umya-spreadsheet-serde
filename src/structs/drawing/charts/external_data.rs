// c:externalData
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::{
        BytesStart,
        Event,
    },
};

use crate::{
    reader::driver::{
        local_name,
        skip_element,
        try_xml_read_loop,
    },
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

/// The link of a chart to the workbook that holds its data (Excel's "Edit
/// Data"). `relationship_id` names the chart part's relationship to that
/// workbook.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ExternalData {
    relationship_id: String,
    auto_update:     Option<bool>,
}

impl ExternalData {
    #[must_use]
    pub fn relationship_id(&self) -> &str {
        &self.relationship_id
    }

    #[must_use]
    pub fn auto_update(&self) -> Option<bool> {
        self.auto_update
    }

    pub fn set_auto_update(&mut self, value: bool) -> &mut Self {
        self.auto_update = Some(value);
        self
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        owner: &BytesStart,
        empty: bool,
        session: &crate::reader::xlsx::SourceCacheSession,
    ) -> Result<(), crate::XlsxError> {
        self.relationship_id = session
            .relationship_id(owner)?
            .filter(|id| !id.is_empty())
            .ok_or_else(|| crate::xml::xml_error("c:externalData requires a non-empty r:id"))?;
        if empty {
            return Ok(());
        }
        let owner_end = owner.name().as_ref().to_vec();
        let _scope_guard = session.enter_child_scope(owner)?;
        try_xml_read_loop!(
            reader,
            Event::Start(ref e) => {
                if session.accepts_child_namespace(e, super::extension_registry::NS_C)?
                    && local_name(e.name().into_inner()) == b"autoUpdate" {
                    self.auto_update = Some(read_flag(e)?);
                    crate::reader::driver::consume_empty_element(reader, e)?;
                } else {
                    skip_element(reader, e)?;
                }
            },
            Event::Empty(ref e) => {
                if session.accepts_child_namespace(e, super::extension_registry::NS_C)?
                    && local_name(e.name().into_inner()) == b"autoUpdate" {
                    self.auto_update = Some(read_flag(e)?);
                }
            },
            Event::End(ref e) => {
                if e.name().as_ref() == owner_end {
                    return Ok(());
                }
            },
            Event::Eof => return Err(crate::xml::xml_error("Error: Could not find c:externalData end element")),
        );
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        // c:externalData
        let attributes = vec![("r:id", self.relationship_id.as_str()).into()];
        match self.auto_update {
            None => write_start_tag(writer, "c:externalData", attributes, true),
            Some(value) => {
                write_start_tag(writer, "c:externalData", attributes, false);
                write_start_tag(
                    writer,
                    "c:autoUpdate",
                    vec![("val", if value { "1" } else { "0" }).into()],
                    true,
                );
                write_end_tag(writer, "c:externalData");
            }
        }
    }
}

fn read_flag(element: &BytesStart) -> Result<bool, crate::XlsxError> {
    crate::reader::driver::read_attribute(element, b"val")?
        .map(|value| crate::xml::parse_bool(&value))
        .transpose()
        .map_err(crate::xml::xml_error)
        .map(|value| value.unwrap_or(true))
}
