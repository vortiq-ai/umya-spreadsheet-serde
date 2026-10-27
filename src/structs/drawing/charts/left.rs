// c:x
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use super::super::super::DoubleValue;
use crate::writer::driver::write_start_tag;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct Left {
    val:   DoubleValue,
    #[cfg_attr(feature = "serde", serde(default))]
    parts: Option<Box<super::ScalarLexeme>>,
}
impl Left {
    #[must_use]
    pub fn val(&self) -> f64 {
        self.parsed_val().unwrap_or(0.0)
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use val()")]
    pub fn get_val(&self) -> f64 {
        self.val()
    }

    pub fn set_val(&mut self, value: f64) -> &mut Left {
        self.val = DoubleValue::default();
        self.parts = None;
        if value.is_finite() {
            self.val.set_value(value);
        } else {
            self.parts = Some(Box::new(super::ScalarLexeme {
                attribute: if value.is_nan() {
                    "NaN".into()
                } else if value.is_sign_positive() {
                    "INF".into()
                } else {
                    "-INF".into()
                },
            }));
        }
        self
    }

    #[must_use]
    pub fn parsed_val(&self) -> Option<f64> {
        if let Some(parts) = &self.parts {
            if self.val.has_value() {
                return None;
            }
            super::xml_scalar::parse_xml_double(&parts.attribute)
        } else if self.val.has_value() {
            Some(self.val.value())
        } else {
            None
        }
    }

    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.parsed_val().is_some()
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
        self.val = DoubleValue::default();
        self.parts = None;
        if let Some(value) = crate::drawing::get_unescaped_attribute(e, b"val") {
            self.parts = Some(Box::new(super::ScalarLexeme { attribute: value }));
        }
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        if !self.is_valid() {
            return;
        }
        let generated = self.val.value_string();
        let value = self.lexical_val().unwrap_or(&generated);
        write_start_tag(writer, "c:x", vec![("val", value).into()], true);
    }
}
