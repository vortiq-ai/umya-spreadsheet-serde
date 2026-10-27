// c:wMode
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use super::{
    super::super::EnumValue,
    LayoutModeValues,
};
use crate::writer::driver::write_start_tag;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct WidthMode {
    val:   EnumValue<LayoutModeValues>,
    #[cfg_attr(feature = "serde", serde(default))]
    parts: Option<Box<super::ScalarLexeme>>,
}
impl WidthMode {
    #[must_use]
    pub fn val(&self) -> &LayoutModeValues {
        if !self.val.has_value() {
            return &LayoutModeValues::Factor;
        }
        self.val.value()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use val()")]
    pub fn get_val(&self) -> &LayoutModeValues {
        self.val()
    }

    pub fn set_val(&mut self, value: LayoutModeValues) -> &mut WidthMode {
        self.parts = None;
        self.val.set_value(value);
        self
    }

    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.parts.is_none()
    }

    #[must_use]
    pub fn lexical_val(&self) -> Option<&str> {
        self.parts.as_ref().map(|v| v.attribute.as_str())
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        _reader: &mut Reader<R>,
        e: &BytesStart,
    ) {
        self.val = EnumValue::default();
        self.parts = None;
        if let Some(value) = crate::drawing::get_unescaped_attribute(e, b"val") {
            if matches!(value.as_str(), "edge" | "factor") {
                self.val.set_value_string(value);
                self.parts = None;
            } else {
                self.parts = Some(Box::new(super::ScalarLexeme { attribute: value }));
            }
        }
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        if !self.is_valid() {
            return;
        }
        let attributes = if self.val.has_value() {
            vec![("val", self.val.value_string()).into()]
        } else {
            vec![]
        };
        write_start_tag(writer, "c:wMode", attributes, true);
    }
}
