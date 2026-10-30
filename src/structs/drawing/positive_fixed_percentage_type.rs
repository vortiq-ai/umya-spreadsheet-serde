use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use super::super::Int32Value;
use crate::writer::driver::write_start_tag;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct PositiveFixedPercentageType {
    val: Int32Value,
}

impl PositiveFixedPercentageType {
    pub(crate) fn into_value(self) -> i32 {
        self.val.value()
    }

    #[inline]
    #[must_use]
    pub fn val(&self) -> i32 {
        self.val.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use val()")]
    pub fn get_val(&self) -> i32 {
        self.val()
    }

    #[inline]
    pub fn set_val(&mut self, value: i32) -> &mut Self {
        self.val.set_value(value);
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
        let value = crate::reader::driver::require_attribute(owner, b"val")?;
        let value = crate::xml::parse_i32(&value).map_err(crate::xml::xml_error)?;
        if !(0..=100_000).contains(&value) {
            return Err(crate::xml::xml_error("percentage outside 0..100000"));
        }
        self.val.set_value(value);
        if !empty {
            crate::structs::drawing::color_transform::consume_paint_leaf(reader, owner, session)?;
        }
        Ok(())
    }

    #[inline]
    pub(crate) fn write_to_shade(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        self.write_to(writer, "a:shade");
    }

    #[inline]
    pub(crate) fn write_to_alpha(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        self.write_to(writer, "a:alpha");
    }

    #[inline]
    pub(crate) fn write_to_tint(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        self.write_to(writer, "a:tint");
    }

    fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>, tab_name: &str) {
        if self.val.has_value() {
            let mut attributes: crate::structs::AttrCollection = Vec::new();
            let val = self.val.value_string();
            attributes.push(("val", &val).into());
            write_start_tag(writer, tab_name, attributes, true);
        }
    }
}
