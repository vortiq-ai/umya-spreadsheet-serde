// a:alpha
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use crate::writer::driver::write_start_tag;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct Alpha {
    val: Box<str>,
}
impl Alpha {
    #[inline]
    #[must_use]
    pub fn val(&self) -> &str {
        &self.val
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use val()")]
    pub fn get_val(&self) -> &str {
        self.val()
    }

    #[inline]
    pub fn set_val<S: Into<String>>(&mut self, value: S) {
        self.val = value.into().into_boxed_str();
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
        let value = crate::reader::driver::require_attribute(owner, b"val")?;
        let parsed = crate::xml::parse_i32(&value).map_err(crate::xml::xml_error)?;
        if !(0..=100_000).contains(&parsed) {
            return Err(crate::xml::xml_error("alpha outside 0..100000"));
        }
        self.set_val(value);
        if !empty {
            crate::structs::drawing::color_transform::consume_paint_leaf(reader, owner, session)?;
        }
        Ok(())
    }

    #[inline]
    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        // a:alpha
        write_start_tag(writer, "a:alpha", vec![("val", &self.val).into()], true);
    }
}
