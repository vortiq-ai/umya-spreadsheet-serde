use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use super::super::StringValue;
use crate::{
    XlsxError,
    writer::driver::write_start_tag,
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct TextFontType {
    typeface:     StringValue,
    pitch_family: StringValue,
    charset:      StringValue,
    panose:       StringValue,
}

impl TextFontType {
    #[inline]
    #[must_use]
    pub fn typeface(&self) -> &str {
        self.typeface.value_str()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use typeface()")]
    pub fn get_typeface(&self) -> &str {
        self.typeface()
    }

    #[inline]
    pub fn set_typeface<S: Into<String>>(&mut self, value: S) -> &mut Self {
        self.typeface.set_value(value.into());
        self
    }

    #[inline]
    #[must_use]
    pub fn pitch_family(&self) -> &str {
        self.pitch_family.value_str()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use pitch_family()")]
    pub fn get_pitch_family(&self) -> &str {
        self.pitch_family()
    }

    #[inline]
    pub fn set_pitch_family<S: Into<String>>(&mut self, value: S) -> &mut Self {
        self.pitch_family.set_value(value.into());
        self
    }

    #[inline]
    #[must_use]
    pub fn charset(&self) -> &str {
        self.charset.value_str()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use charset()")]
    pub fn get_charset(&self) -> &str {
        self.charset()
    }

    #[inline]
    pub fn set_charset<S: Into<String>>(&mut self, value: S) -> &mut Self {
        self.charset.set_value(value.into());
        self
    }

    #[inline]
    #[must_use]
    pub fn panose(&self) -> &str {
        self.panose.value_str()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use panose()")]
    pub fn get_panose(&self) -> &str {
        self.panose()
    }

    #[inline]
    pub fn set_panose<S: Into<String>>(&mut self, value: S) -> &mut Self {
        self.panose.set_value(value.into());
        self
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
        empty_flag: bool,
    ) -> Result<(), XlsxError> {
        if let Some(value) = crate::reader::driver::read_attribute(e, b"typeface")? {
            self.set_typeface(value);
        }
        if let Some(value) = crate::reader::driver::read_attribute(e, b"pitchFamily")? {
            crate::xml::trim_xml_whitespace(&value)
                .parse::<i8>()
                .map_err(|_| crate::xml::xml_error("invalid DrawingML font pitch family"))?;
            self.set_pitch_family(value);
        }
        if let Some(value) = crate::reader::driver::read_attribute(e, b"charset")? {
            crate::xml::trim_xml_whitespace(&value)
                .parse::<i8>()
                .map_err(|_| crate::xml::xml_error("invalid DrawingML font character set"))?;
            self.set_charset(value);
        }
        if let Some(value) = crate::reader::driver::read_attribute(e, b"panose")? {
            if value.len() != 20 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                return Err(crate::xml::xml_error("invalid DrawingML font Panose value"));
            }
            self.set_panose(value);
        }
        if !empty_flag {
            crate::reader::driver::consume_empty_element(reader, e)?;
        }
        Ok(())
    }

    #[inline]
    pub(crate) fn write_to_latin(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        self.write_to(writer, "a:latin");
    }

    #[inline]
    pub(crate) fn write_to_cs(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        self.write_to(writer, "a:cs");
    }

    #[inline]
    pub(crate) fn write_to_ea(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        self.write_to(writer, "a:ea");
    }

    fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>, tab_name: &str) {
        let mut attributes: crate::structs::AttrCollection = Vec::new();
        if self.typeface.has_value() {
            attributes.push(("typeface", self.typeface.value_str()).into());
        }
        if self.pitch_family.has_value() {
            attributes.push(("pitchFamily", self.pitch_family.value_str()).into());
        }
        if self.charset.has_value() {
            attributes.push(("charset", self.charset.value_str()).into());
        }
        if self.panose.has_value() {
            attributes.push(("panose", self.panose.value_str()).into());
        }
        write_start_tag(writer, tab_name, attributes, true);
    }
}
