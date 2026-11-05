// c:printSettings
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::{
        BytesStart,
        Event,
    },
};

use super::{
    HeaderFooter,
    PageMargins,
    PageSetup,
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

/// The print settings of a chart. Each part is absent until the file or the
/// caller sets it; an absent part writes nothing.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct PrintSettings {
    header_footer:                Option<HeaderFooter>,
    page_margins:                 Option<PageMargins>,
    page_setup:                   Option<PageSetup>,
    #[cfg_attr(feature = "serde", serde(default))]
    legacy_drawing_header_footer: Option<String>,
}

impl PrintSettings {
    #[must_use]
    pub fn header_footer(&self) -> Option<&HeaderFooter> {
        self.header_footer.as_ref()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use header_footer()")]
    pub fn get_header_footer(&self) -> Option<&HeaderFooter> {
        self.header_footer()
    }

    pub fn header_footer_mut(&mut self) -> Option<&mut HeaderFooter> {
        self.header_footer.as_mut()
    }

    #[deprecated(since = "3.0.0", note = "Use header_footer_mut()")]
    pub fn get_header_footer_mut(&mut self) -> Option<&mut HeaderFooter> {
        self.header_footer_mut()
    }

    pub fn set_header_footer(&mut self, value: HeaderFooter) -> &mut Self {
        self.header_footer = Some(value);
        self
    }

    pub fn remove_header_footer(&mut self) -> &mut Self {
        self.header_footer = None;
        self
    }

    #[must_use]
    pub fn page_margins(&self) -> Option<&PageMargins> {
        self.page_margins.as_ref()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use page_margins()")]
    pub fn get_page_margins(&self) -> Option<&PageMargins> {
        self.page_margins()
    }

    pub fn page_margins_mut(&mut self) -> &mut PageMargins {
        self.page_margins.get_or_insert_with(PageMargins::default)
    }

    #[deprecated(since = "3.0.0", note = "Use page_margins_mut()")]
    pub fn get_page_margins_mut(&mut self) -> &mut PageMargins {
        self.page_margins_mut()
    }

    pub fn set_page_margins(&mut self, value: PageMargins) -> &mut Self {
        self.page_margins = Some(value);
        self
    }

    pub fn remove_page_margins(&mut self) -> &mut Self {
        self.page_margins = None;
        self
    }

    #[must_use]
    pub fn page_setup(&self) -> Option<&PageSetup> {
        self.page_setup.as_ref()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use page_setup()")]
    pub fn get_page_setup(&self) -> Option<&PageSetup> {
        self.page_setup()
    }

    pub fn page_setup_mut(&mut self) -> Option<&mut PageSetup> {
        self.page_setup.as_mut()
    }

    #[deprecated(since = "3.0.0", note = "Use page_setup_mut()")]
    pub fn get_page_setup_mut(&mut self) -> Option<&mut PageSetup> {
        self.page_setup_mut()
    }

    pub fn set_page_setup(&mut self, value: PageSetup) -> &mut Self {
        self.page_setup = Some(value);
        self
    }

    pub fn remove_page_setup(&mut self) -> &mut Self {
        self.page_setup = None;
        self
    }

    /// The chart part relationship id of the legacy header and footer
    /// drawing (`c:legacyDrawingHF`), which holds the header and footer
    /// pictures.
    #[must_use]
    pub fn legacy_drawing_header_footer(&self) -> Option<&str> {
        self.legacy_drawing_header_footer.as_deref()
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        source_owner: &BytesStart,
        session: &crate::reader::xlsx::SourceCacheSession,
    ) -> Result<(), crate::XlsxError> {
        let owner_end = source_owner.name().as_ref().to_vec();
        let _scope_guard = session.enter_child_scope(source_owner)?;
        try_xml_read_loop!(
            reader,
            Event::Start(ref e) => {
                match if session.accepts_child_namespace(e, super::extension_registry::NS_C)? {
                    local_name(e.name().into_inner())
                } else {
                    b""
                } {
                    b"headerFooter" => {
                        let mut obj = HeaderFooter::default();
                        obj.set_attributes(reader, e, false, session)?;
                        self.header_footer = Some(obj);
                    }
                    b"pageMargins" => {
                        let mut obj = PageMargins::default();
                        obj.set_attributes(reader, e)?;
                        self.page_margins = Some(obj);
                        crate::reader::driver::consume_empty_element(reader, e)?;
                    }
                    b"pageSetup" => {
                        let mut obj = PageSetup::default();
                        obj.set_attributes(e)?;
                        self.page_setup = Some(obj);
                        crate::reader::driver::consume_empty_element(reader, e)?;
                    }
                    b"legacyDrawingHF" => {
                        self.legacy_drawing_header_footer = Some(read_relationship_id(e, session)?);
                        crate::reader::driver::consume_empty_element(reader, e)?;
                    }
                    _ => skip_element(reader, e)?,
                }
            },
            Event::Empty(ref e) => {
                match if session.accepts_child_namespace(e, super::extension_registry::NS_C)? {
                    local_name(e.name().into_inner())
                } else {
                    b""
                } {
                    b"headerFooter" => {
                        let mut obj = HeaderFooter::default();
                        obj.set_attributes(reader, e, true, session)?;
                        self.header_footer = Some(obj);
                    }
                    b"pageMargins" => {
                        let mut obj = PageMargins::default();
                        obj.set_attributes(reader, e)?;
                        self.page_margins = Some(obj);
                    }
                    b"pageSetup" => {
                        let mut obj = PageSetup::default();
                        obj.set_attributes(e)?;
                        self.page_setup = Some(obj);
                    }
                    b"legacyDrawingHF" => {
                        self.legacy_drawing_header_footer = Some(read_relationship_id(e, session)?);
                    }
                    _ => (),
                }
            },
            Event::End(ref e) => {
                if e.name().as_ref() == owner_end {
                    return Ok(());
                }
            },
            Event::Eof => return Err(crate::xml::xml_error(format!("Error: Could not find {} end element", "c:printSettings")))
        );
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        // c:printSettings
        write_start_tag(writer, "c:printSettings", vec![], false);

        // c:headerFooter
        if let Some(v) = &self.header_footer {
            v.write_to(writer);
        }

        // c:pageMargins
        if let Some(v) = &self.page_margins {
            v.write_to(writer);
        }

        // c:pageSetup
        if let Some(v) = &self.page_setup {
            v.write_to(writer);
        }

        // c:legacyDrawingHF
        if let Some(id) = &self.legacy_drawing_header_footer {
            write_start_tag(
                writer,
                "c:legacyDrawingHF",
                vec![("r:id", id.as_str()).into()],
                true,
            );
        }

        write_end_tag(writer, "c:printSettings");
    }
}

fn read_relationship_id(
    element: &BytesStart,
    session: &crate::reader::xlsx::SourceCacheSession,
) -> Result<String, crate::XlsxError> {
    session
        .relationship_id(element)?
        .filter(|id| !id.is_empty())
        .ok_or_else(|| crate::xml::xml_error("c:legacyDrawingHF requires a non-empty r:id"))
}
