// c:numFmt
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use super::super::super::{
    BooleanValue,
    StringValue,
};
use crate::{
    drawing::get_unescaped_attribute,
    writer::driver::write_start_tag,
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct NumberingFormat {
    format_code:   StringValue,
    source_linked: BooleanValue,
    #[cfg_attr(feature = "serde", serde(default))]
    parts:         Option<Box<NumberingFormatParts>>,
}
/// Source lexical state is authoritative only while the legacy typed branch is
/// absent.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct NumberingFormatParts {
    source_linked: Option<super::XmlBoolean>,
}
impl NumberingFormat {
    #[must_use]
    pub fn format_code(&self) -> &str {
        self.format_code.value_str()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use format_code()")]
    pub fn get_format_code(&self) -> &str {
        self.format_code()
    }

    pub fn set_format_code<S: Into<String>>(&mut self, value: S) -> &mut NumberingFormat {
        self.format_code.set_value(value);
        self
    }

    #[must_use]
    pub fn source_linked(&self) -> bool {
        self.source_linked_value().unwrap_or(false)
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use source_linked()")]
    pub fn get_source_linked(&self) -> bool {
        self.source_linked()
    }

    pub fn set_source_linked(&mut self, value: bool) -> &mut NumberingFormat {
        self.parts = None;
        self.source_linked.set_value(value);
        self
    }

    #[must_use]
    pub fn has_format_code(&self) -> bool {
        self.format_code.has_value()
    }

    #[must_use]
    pub fn source_linked_value(&self) -> Option<bool> {
        if let Some(parts) = &self.parts {
            if self.source_linked.has_value() {
                return None;
            }
            parts
                .source_linked
                .as_ref()
                .and_then(super::XmlBoolean::value)
        } else if self.source_linked.has_value() {
            Some(self.source_linked.value())
        } else {
            None
        }
    }

    #[must_use]
    pub fn source_linked_attribute(&self) -> Option<&str> {
        if let Some(parts) = &self.parts {
            parts
                .source_linked
                .as_ref()
                .and_then(super::XmlBoolean::attribute)
        } else if self.source_linked.has_value() {
            Some(self.source_linked.value_string())
        } else {
            None
        }
    }

    pub fn set_source_linked_attribute(&mut self, value: Option<String>) -> &mut Self {
        self.source_linked = BooleanValue::default();
        self.parts = Some(Box::new(NumberingFormatParts {
            source_linked: value.map(|s| {
                let mut v = super::XmlBoolean::default();
                v.set_lexical_attribute(Some(s));
                v
            }),
        }));
        self
    }

    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.has_format_code()
            && self.parts.as_ref().is_none_or(|p| {
                !self.source_linked.has_value()
                    && p.source_linked
                        .as_ref()
                        .is_none_or(super::XmlBoolean::is_valid)
            })
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        _reader: &mut Reader<R>,
        e: &BytesStart,
    ) {
        if let Some(value) = get_unescaped_attribute(e, b"formatCode") {
            self.format_code.set_value(value);
        }
        self.source_linked = BooleanValue::default();
        let source_linked = get_unescaped_attribute(e, b"sourceLinked").map(|value| {
            let mut v = super::XmlBoolean::default();
            v.set_lexical_attribute(Some(value));
            v
        });

        self.parts = None;
        match source_linked {
            Some(value) if matches!(value.attribute(), Some("0" | "1")) => {
                self.source_linked
                    .set_value(value.value().expect("canonical boolean"));
            }
            Some(value) => {
                self.parts = Some(Box::new(NumberingFormatParts {
                    source_linked: Some(value),
                }));
            }
            None => (),
        }
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        if !self.is_valid() {
            return;
        }
        let mut attributes: crate::structs::AttrCollection =
            vec![("formatCode", self.format_code.value_str()).into()];
        if let Some(parts) = &self.parts {
            if let Some(value) = parts
                .source_linked
                .as_ref()
                .and_then(super::XmlBoolean::attribute)
            {
                attributes.push(("sourceLinked", value).into());
            }
        } else if self.source_linked.has_value() {
            attributes.push(("sourceLinked", self.source_linked.value_string()).into());
        }
        write_start_tag(writer, "c:numFmt", attributes, true);
    }
}
