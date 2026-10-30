// a:miter
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use crate::{
    Int32Value,
    writer::driver::write_start_tag,
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct Miter {
    limit: Int32Value,
}

impl Miter {
    #[inline]
    #[must_use]
    pub fn limit(&self) -> i32 {
        self.limit.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use limit()")]
    pub fn get_limit(&self) -> i32 {
        self.limit()
    }

    #[inline]
    pub fn set_limit(&mut self, value: i32) -> &mut Self {
        self.limit.set_value(value);
        self
    }

    #[inline]
    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
        empty_flag: bool,
    ) -> Result<(), crate::XlsxError> {
        self.read_with_session(reader, e, empty_flag, None)
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
        if let Some(value) = crate::reader::driver::read_attribute(owner, b"lim")? {
            let parsed = crate::xml::parse_i32(&value).map_err(crate::xml::xml_error)?;
            if parsed < 0 {
                return Err(crate::xml::xml_error("negative DrawingML lim"));
            }
            self.limit.set_value(parsed);
        }
        if !empty {
            crate::structs::drawing::color_transform::consume_paint_leaf(reader, owner, session)?;
        }
        Ok(())
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        // a:miter
        let mut attributes: crate::structs::AttrCollection = Vec::new();
        let lim = self.limit.value_string();
        if self.limit.has_value() {
            attributes.push(("lim", &lim).into());
        }
        write_start_tag(writer, "a:miter", attributes, true);
    }
}
