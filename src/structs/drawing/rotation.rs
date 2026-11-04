// a:rot
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
pub struct Rotation {
    latitude:   Int32Value,
    longitude:  Int32Value,
    revolution: Int32Value,
}

impl Rotation {
    #[inline]
    #[must_use]
    pub fn latitude(&self) -> i32 {
        self.latitude.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use latitude()")]
    pub fn get_latitude(&self) -> i32 {
        self.latitude()
    }

    #[inline]
    pub fn set_latitude(&mut self, value: i32) -> &mut Self {
        self.latitude.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn longitude(&self) -> i32 {
        self.longitude.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use longitude()")]
    pub fn get_longitude(&self) -> i32 {
        self.longitude()
    }

    #[inline]
    pub fn set_longitude(&mut self, value: i32) -> &mut Self {
        self.longitude.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn revolution(&self) -> i32 {
        self.revolution.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use revolution()")]
    pub fn get_revolution(&self) -> i32 {
        self.revolution()
    }

    #[inline]
    pub fn set_revolution(&mut self, value: i32) -> &mut Self {
        self.revolution.set_value(value);
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
        let value = crate::reader::driver::require_attribute(owner, b"lat")?;
        self.latitude
            .set_value(crate::xml::parse_i32(&value).map_err(crate::xml::xml_error)?);
        let value = crate::reader::driver::require_attribute(owner, b"lon")?;
        self.longitude
            .set_value(crate::xml::parse_i32(&value).map_err(crate::xml::xml_error)?);
        let value = crate::reader::driver::require_attribute(owner, b"rev")?;
        self.revolution
            .set_value(crate::xml::parse_i32(&value).map_err(crate::xml::xml_error)?);
        if !empty {
            crate::structs::drawing::color_transform::consume_paint_leaf(reader, owner, session)?;
        }
        Ok(())
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        let mut attributes: crate::structs::AttrCollection = Vec::new();
        let latitude = self.latitude.value_string();
        if self.latitude.has_value() {
            attributes.push(("lat", &latitude).into());
        }
        let longitude = self.longitude.value_string();
        if self.longitude.has_value() {
            attributes.push(("lon", &longitude).into());
        }
        let revolution = self.revolution.value_string();
        if self.latitude.has_value() {
            attributes.push(("rev", &revolution).into());
        }
        write_start_tag(writer, "a:rot", attributes, true);
    }
}
