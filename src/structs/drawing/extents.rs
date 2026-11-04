// a:ext
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use super::super::super::Int64Value;
use crate::writer::driver::write_start_tag;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct Extents {
    cx: Int64Value,
    cy: Int64Value,
}
impl Extents {
    #[inline]
    #[must_use]
    pub fn cx(&self) -> i64 {
        self.cx.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use cx()")]
    pub fn get_cx(&self) -> i64 {
        self.cx()
    }

    #[inline]
    pub fn set_cx(&mut self, value: i64) -> &mut Extents {
        self.cx.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn cy(&self) -> i64 {
        self.cy.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use cy()")]
    pub fn get_cy(&self) -> i64 {
        self.cy()
    }

    #[inline]
    pub fn set_cy(&mut self, value: i64) -> &mut Extents {
        self.cy.set_value(value);
        self
    }

    #[inline]
    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
    ) -> Result<(), crate::XlsxError> {
        self.read_with_session(reader, e, false, None)
    }

    pub(crate) fn read_with_session<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        owner: &BytesStart<'_>,
        empty: bool,
        session: Option<&crate::reader::xlsx::SourceCacheSession>,
    ) -> Result<(), crate::XlsxError> {
        let _scope_guard = session
            .map(|session| session.enter_child_scope(owner))
            .transpose()?
            .flatten();
        let value = crate::reader::driver::require_attribute(owner, b"cx")?;
        let parsed = crate::xml::trim_xml_whitespace(&value)
            .parse::<i64>()
            .map_err(crate::xml::xml_error)?;
        if parsed < 0 {
            return Err(crate::xml::xml_error("negative DrawingML cx"));
        }
        self.cx.set_value(parsed);
        let value = crate::reader::driver::require_attribute(owner, b"cy")?;
        let parsed = crate::xml::trim_xml_whitespace(&value)
            .parse::<i64>()
            .map_err(crate::xml::xml_error)?;
        if parsed < 0 {
            return Err(crate::xml::xml_error("negative DrawingML cy"));
        }
        self.cy.set_value(parsed);
        if !empty {
            crate::structs::drawing::color_transform::consume_paint_leaf(reader, owner, session)?;
        }
        Ok(())
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        // a:ext
        write_start_tag(
            writer,
            "a:ext",
            vec![
                ("cx", self.cx.value_string()).into(),
                ("cy", self.cy.value_string()).into(),
            ],
            true,
        );
    }
}
