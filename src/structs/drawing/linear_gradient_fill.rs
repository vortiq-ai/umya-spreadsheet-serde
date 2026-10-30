// a:lin
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use super::super::super::{
    BooleanValue,
    Int32Value,
};
use crate::writer::driver::write_start_tag;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct LinearGradientFill {
    angle:  Int32Value,
    scaled: BooleanValue,
}

impl LinearGradientFill {
    #[inline]
    #[must_use]
    pub fn angle(&self) -> i32 {
        self.angle.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use angle()")]
    pub fn get_angle(&self) -> i32 {
        self.angle()
    }

    #[inline]
    pub fn set_angle(&mut self, value: i32) -> &mut LinearGradientFill {
        self.angle.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn scaled(&self) -> bool {
        self.scaled.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use scaled()")]
    pub fn get_scaled(&self) -> bool {
        self.scaled()
    }

    #[inline]
    pub fn set_scaled(&mut self, value: bool) -> &mut LinearGradientFill {
        self.scaled.set_value(value);
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
        if let Some(value) = crate::reader::driver::read_attribute(owner, b"ang")? {
            self.angle
                .set_value(crate::xml::parse_i32(&value).map_err(crate::xml::xml_error)?);
        }
        if let Some(value) = crate::reader::driver::read_attribute(owner, b"scaled")? {
            self.scaled
                .set_value(crate::xml::parse_bool(&value).map_err(crate::xml::xml_error)?);
        }
        if !empty {
            crate::structs::drawing::color_transform::consume_paint_leaf(reader, owner, session)?;
        }
        Ok(())
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        // a:lin
        let mut attributes: crate::structs::AttrCollection = Vec::new();
        let ang = self.angle.value_string();
        if self.angle.has_value() {
            attributes.push(("ang", &ang).into());
        }
        if self.scaled.has_value() {
            attributes.push(("scaled", self.scaled.value_string()).into());
        }
        write_start_tag(writer, "a:lin", attributes, true);
    }
}
