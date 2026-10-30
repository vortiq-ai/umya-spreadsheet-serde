// CT_LineEndProperties
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use crate::{
    structs::StringValue,
    writer::driver::write_start_tag,
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct LineEndProperties {
    t_type: StringValue,
    width:  StringValue,
    length: StringValue,
}

impl LineEndProperties {
    #[inline]
    #[must_use]
    pub fn get_type(&self) -> &str {
        self.t_type.value_str()
    }

    #[inline]
    pub fn set_type<S: Into<String>>(&mut self, value: S) -> &mut Self {
        self.t_type.set_value(value.into());
        self
    }

    #[inline]
    #[must_use]
    pub fn width(&self) -> &str {
        self.width.value_str()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use width()")]
    pub fn get_width(&self) -> &str {
        self.width()
    }

    #[inline]
    pub fn set_width<S: Into<String>>(&mut self, value: S) -> &mut Self {
        self.width.set_value(value.into());
        self
    }

    #[inline]
    #[must_use]
    pub fn length(&self) -> &str {
        self.length.value_str()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use length()")]
    pub fn get_length(&self) -> &str {
        self.length()
    }

    #[inline]
    pub fn set_length<S: Into<String>>(&mut self, value: S) -> &mut Self {
        self.length.set_value(value.into());
        self
    }

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
        if let Some(value) = crate::reader::driver::read_attribute(owner, b"type")? {
            if !matches!(
                crate::xml::trim_xml_whitespace(&value),
                "none" | "triangle" | "stealth" | "diamond" | "oval" | "arrow"
            ) {
                return Err(crate::xml::xml_error("invalid DrawingML line end type"));
            }
            self.set_type(crate::xml::trim_xml_whitespace(&value));
        }
        if let Some(value) = crate::reader::driver::read_attribute(owner, b"w")? {
            if !matches!(crate::xml::trim_xml_whitespace(&value), "sm" | "med" | "lg") {
                return Err(crate::xml::xml_error("invalid DrawingML line end w"));
            }
            self.set_width(crate::xml::trim_xml_whitespace(&value));
        }
        if let Some(value) = crate::reader::driver::read_attribute(owner, b"len")? {
            if !matches!(crate::xml::trim_xml_whitespace(&value), "sm" | "med" | "lg") {
                return Err(crate::xml::xml_error("invalid DrawingML line end len"));
            }
            self.set_length(crate::xml::trim_xml_whitespace(&value));
        }
        if !empty {
            crate::structs::drawing::color_transform::consume_paint_leaf(reader, owner, session)?;
        }
        Ok(())
    }

    pub(crate) fn write_to_as(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        name: &str,
    ) -> Result<(), crate::XlsxError> {
        // CT_LineEndProperties
        if self.t_type.has_value()
            && !matches!(
                self.get_type(),
                "none" | "triangle" | "stealth" | "diamond" | "oval" | "arrow"
            )
        {
            return Err(crate::xml::xml_error("invalid DrawingML line end type"));
        }
        if self.width.has_value() && !matches!(self.width(), "sm" | "med" | "lg") {
            return Err(crate::xml::xml_error("invalid DrawingML line end width"));
        }
        if self.length.has_value() && !matches!(self.length(), "sm" | "med" | "lg") {
            return Err(crate::xml::xml_error("invalid DrawingML line end length"));
        }
        let mut attributes: crate::structs::AttrCollection = Vec::new();
        if self.t_type.has_value() {
            attributes.push(("type", (self.t_type.value_str())).into());
        }
        if self.width.has_value() {
            attributes.push(("w", (self.width.value_str())).into());
        }
        if self.length.has_value() {
            attributes.push(("len", (self.length.value_str())).into());
        }
        write_start_tag(writer, name, attributes, true);
        Ok(())
    }
}
