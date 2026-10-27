// c:explosion
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use super::{
    super::super::UInt32Value,
    xml_scalar::{
        ScalarLexeme,
        parse_xml_unsigned,
    },
};
use crate::{
    drawing::get_unescaped_attribute,
    writer::driver::write_start_tag,
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct Explosion {
    val:   UInt32Value,
    #[cfg_attr(feature = "serde", serde(default))]
    parts: Option<Box<ScalarLexeme>>,
}
impl Explosion {
    #[must_use]
    pub fn has_val(&self) -> bool {
        self.parts.is_some() || self.val.has_value()
    }

    /// Parsed semantic value; invalid-present attributes are never schema
    /// defaults.
    #[must_use]
    pub fn parsed_val(&self) -> Option<u32> {
        if !self.is_coherent() {
            return None;
        }
        self.parts.as_ref().map_or_else(
            || self.val.has_value().then(|| self.val.value()),
            |parts| parse_xml_unsigned(&parts.attribute),
        )
    }

    /// A deserialized payload must not activate both legacy and lexical
    /// branches.
    #[must_use]
    pub fn is_coherent(&self) -> bool {
        !(self.val.has_value() && self.parts.is_some())
    }

    #[must_use]
    pub fn lexical_val(&self) -> Option<&str> {
        self.parts.as_ref().map(|p| p.attribute.as_str())
    }

    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.parsed_val().is_some()
    }

    #[must_use]
    pub fn val(&self) -> u32 {
        self.parsed_val().unwrap_or(0)
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use val()")]
    pub fn get_val(&self) -> u32 {
        self.val()
    }

    pub fn set_val(&mut self, value: u32) -> &mut Explosion {
        self.parts = None;
        self.val.set_value(value);
        self
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        _reader: &mut Reader<R>,
        e: &BytesStart,
    ) {
        self.val = UInt32Value::default();
        self.parts = get_unescaped_attribute(e, b"val")
            .map(|attribute| Box::new(ScalarLexeme { attribute }));
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        if !self.is_valid() {
            return;
        }
        let value = self
            .parts
            .as_ref()
            .map(|p| p.attribute.clone())
            .or_else(|| self.val.has_value().then(|| self.val.value_string()));
        write_start_tag(
            writer,
            "c:explosion",
            value
                .as_deref()
                .map(|v| vec![("val", v).into()])
                .unwrap_or_default(),
            true,
        );
    }
}
