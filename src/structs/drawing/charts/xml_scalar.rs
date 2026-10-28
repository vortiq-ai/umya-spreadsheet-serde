use std::io::Cursor;

use quick_xml::{
    Writer,
    events::BytesStart,
};

use crate::{
    drawing::get_unescaped_attribute,
    writer::driver::write_start_tag,
};
/// The four XML boolean spellings, retained independently of their semantic
/// value.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BooleanLexeme {
    Zero,
    One,
    False,
    True,
}
impl BooleanLexeme {
    #[must_use]
    pub fn value(self) -> bool {
        matches!(self, Self::One | Self::True)
    }

    pub(crate) fn lexical(self) -> &'static str {
        match self {
            Self::Zero => "0",
            Self::One => "1",
            Self::False => "false",
            Self::True => "true",
        }
    }
}
/// Trim only XML Schema whitespace, not unrelated Unicode spacing characters.
pub(crate) fn trim_xml_whitespace(value: &str) -> &str {
    value.trim_matches(|c| matches!(c, ' ' | '\t' | '\r' | '\n'))
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct XmlBoolean {
    attribute: Option<String>,
}
impl XmlBoolean {
    #[must_use]
    pub fn attribute(&self) -> Option<&str> {
        self.attribute.as_deref()
    }

    #[must_use]
    pub fn lexeme(&self) -> Option<BooleanLexeme> {
        self.attribute
            .as_deref()
            .and_then(|s| match trim_xml_whitespace(s) {
                "0" => Some(BooleanLexeme::Zero),
                "1" => Some(BooleanLexeme::One),
                "false" => Some(BooleanLexeme::False),
                "true" => Some(BooleanLexeme::True),
                _ => None,
            })
    }

    /// Absence has `CT_Boolean`'s true default; an invalid-present value has no
    /// value.
    pub fn value(&self) -> Option<bool> {
        if self.attribute.is_none() {
            Some(true)
        } else {
            self.lexeme().map(BooleanLexeme::value)
        }
    }

    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.value().is_some()
    }

    pub fn set_value(&mut self, value: bool) -> &mut Self {
        self.set_attribute(Some(if value {
            BooleanLexeme::One
        } else {
            BooleanLexeme::Zero
        }))
    }

    pub fn set_attribute(&mut self, value: Option<BooleanLexeme>) -> &mut Self {
        self.attribute = value.map(|v| v.lexical().to_owned());
        self
    }

    /// The authoritative source spelling, including an invalid-present
    /// attribute.
    pub fn set_lexical_attribute(&mut self, value: Option<String>) -> &mut Self {
        self.attribute = value;
        self
    }

    pub(crate) fn read(e: &BytesStart) -> Self {
        Self {
            attribute: get_unescaped_attribute(e, b"val"),
        }
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>, name: &str) {
        write_start_tag(
            writer,
            name,
            self.attribute
                .as_deref()
                .map(|v| vec![("val", v).into()])
                .unwrap_or_default(),
            true,
        );
    }
}

/// Source spelling is authoritative on repaired legacy scalar leaves. Import
/// clears the old typed branch; typed setters clear this branch, so they cannot
/// diverge.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ScalarLexeme {
    pub(crate) attribute: String,
}

pub(crate) fn parse_xml_unsigned(value: &str) -> Option<u32> {
    let digits = trim_xml_whitespace(value);
    if digits.is_empty() || !digits.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let digits = digits.trim_start_matches('0');
    if digits.is_empty() {
        return Some(0);
    }
    digits.parse().ok()
}

/// XML Schema double lexical grammar is narrower than Rust's float parser.
pub(crate) fn parse_xml_double(value: &str) -> Option<f64> {
    let value = trim_xml_whitespace(value);
    match value {
        "INF" => return Some(f64::INFINITY),
        "-INF" => return Some(f64::NEG_INFINITY),
        "NaN" => return Some(f64::NAN),
        _ => (),
    }
    let unsigned = value
        .strip_prefix('+')
        .or_else(|| value.strip_prefix('-'))
        .unwrap_or(value);
    let mut exponent_split = unsigned.split(['e', 'E']);
    let mantissa = exponent_split.next()?;
    if let Some(exponent) = exponent_split.next() {
        let digits = exponent
            .strip_prefix('+')
            .or_else(|| exponent.strip_prefix('-'))
            .unwrap_or(exponent);
        if digits.is_empty() || !digits.bytes().all(|c| c.is_ascii_digit()) {
            return None;
        }
    }
    if exponent_split.next().is_some() {
        return None;
    }
    let mut dot_split = mantissa.split('.');
    let whole = dot_split.next()?;
    let fraction = dot_split.next().unwrap_or("");
    if dot_split.next().is_some()
        || (whole.is_empty() && fraction.is_empty())
        || !whole
            .bytes()
            .chain(fraction.bytes())
            .all(|c| c.is_ascii_digit())
    {
        return None;
    }
    value.parse().ok()
}

/// A newly modeled point index retains its one authoritative XML attribute.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct XmlUnsignedInt {
    attribute: Option<String>,
}
impl XmlUnsignedInt {
    #[must_use]
    pub fn attribute(&self) -> Option<&str> {
        self.attribute.as_deref()
    }

    pub fn val(&self) -> Option<u32> {
        self.attribute.as_deref().and_then(parse_xml_unsigned)
    }

    pub fn set_val(&mut self, value: u32) -> &mut Self {
        self.attribute = Some(value.to_string());
        self
    }

    pub fn set_lexical_attribute(&mut self, value: Option<String>) -> &mut Self {
        self.attribute = value;
        self
    }

    pub(crate) fn read(e: &BytesStart) -> Self {
        Self {
            attribute: get_unescaped_attribute(e, b"val"),
        }
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        self.write_to_as(writer, "c:idx");
    }

    pub(crate) fn write_to_as(&self, writer: &mut Writer<Cursor<Vec<u8>>>, name: &str) {
        write_start_tag(
            writer,
            name,
            self.attribute
                .as_deref()
                .map(|v| vec![("val", v).into()])
                .unwrap_or_default(),
            true,
        );
    }
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct XmlDouble {
    attribute: Option<String>,
}
impl XmlDouble {
    #[must_use]
    pub fn attribute(&self) -> Option<&str> {
        self.attribute.as_deref()
    }

    pub fn value(&self) -> Option<f64> {
        self.attribute.as_deref().and_then(parse_xml_double)
    }

    pub fn set_value(&mut self, value: f64) -> &mut Self {
        self.attribute = Some(if value.is_nan() {
            "NaN".into()
        } else if value == f64::INFINITY {
            "INF".into()
        } else if value == f64::NEG_INFINITY {
            "-INF".into()
        } else {
            value.to_string()
        });
        self
    }

    pub fn set_attribute(&mut self, value: Option<String>) -> &mut Self {
        self.attribute = value;
        self
    }

    pub(crate) fn read(e: &BytesStart) -> Self {
        Self {
            attribute: get_unescaped_attribute(e, b"val"),
        }
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>, name: &str) {
        write_start_tag(
            writer,
            name,
            self.attribute
                .as_ref()
                .map(|s| vec![("val", s.as_str()).into()])
                .unwrap_or_default(),
            true,
        );
    }
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ScalarElement<T> {
    attribute: Option<T>,
}
impl<T> ScalarElement<T> {
    pub fn attribute(&self) -> Option<&T> {
        self.attribute.as_ref()
    }

    pub fn set_attribute(&mut self, value: Option<T>) -> &mut Self {
        self.attribute = value;
        self
    }
}
/// Point picture-format spellings from `CT_PictureFormat`.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub enum PictureFormat {
    #[default]
    Stretch,
    Stack,
    StackScale,
}
impl PictureFormat {
    pub(crate) fn lexical(self) -> &'static str {
        match self {
            Self::Stretch => "stretch",
            Self::Stack => "stack",
            Self::StackScale => "stackScale",
        }
    }
}
