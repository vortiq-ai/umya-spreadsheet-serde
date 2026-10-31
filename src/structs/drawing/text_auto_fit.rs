use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use crate::{
    XlsxError,
    writer::driver::write_start_tag,
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub struct ShapeAutoFit {}
impl ShapeAutoFit {
    #[inline]
    pub(crate) fn set_attributes<R: std::io::BufRead>(
        reader: &mut Reader<R>,
        e: &BytesStart,
        empty: bool,
    ) -> Result<(), XlsxError> {
        crate::reader::driver::validate_attributes(e)?;
        if !empty {
            crate::reader::driver::consume_empty_element(reader, e)?;
        }
        Ok(())
    }

    #[inline]
    pub(crate) fn write_to(writer: &mut Writer<Cursor<Vec<u8>>>) {
        write_start_tag(writer, "a:spAutoFit", vec![], true);
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TextAutoFit {
    Disabled,
    Normal(NormalAutoFit),
    Shape(ShapeAutoFit),
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub struct NormalAutoFit {
    font_scale:             Option<String>,
    line_spacing_reduction: Option<String>,
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for NormalAutoFit {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = NormalAutoFit;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("NormalAutoFit")
            }

            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> Result<Self::Value, A::Error> {
                let font_scale: Option<String> = seq
                    .next_element()?
                    .ok_or_else(|| serde::de::Error::invalid_length(0, &self))?;
                let line_spacing_reduction: Option<String> = seq
                    .next_element()?
                    .ok_or_else(|| serde::de::Error::invalid_length(1, &self))?;
                let mut result = NormalAutoFit::default();
                result
                    .set_font_scale(font_scale.as_deref())
                    .map_err(serde::de::Error::custom)?;
                result
                    .set_line_spacing_reduction(line_spacing_reduction.as_deref())
                    .map_err(serde::de::Error::custom)?;
                Ok(result)
            }

            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> Result<Self::Value, A::Error> {
                let mut font_scale = None;
                let mut line_spacing_reduction = None;
                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "font_scale" => {
                            if font_scale.is_some() {
                                return Err(serde::de::Error::duplicate_field("font_scale"));
                            }
                            font_scale = Some(map.next_value()?);
                        }
                        "line_spacing_reduction" => {
                            if line_spacing_reduction.is_some() {
                                return Err(serde::de::Error::duplicate_field(
                                    "line_spacing_reduction",
                                ));
                            }
                            line_spacing_reduction = Some(map.next_value()?);
                        }
                        _ => {
                            return Err(serde::de::Error::unknown_field(
                                &key,
                                &["font_scale", "line_spacing_reduction"],
                            ));
                        }
                    }
                }
                let font_scale: Option<String> = font_scale.unwrap_or_default();
                let line_spacing_reduction: Option<String> =
                    line_spacing_reduction.unwrap_or_default();
                let mut result = NormalAutoFit::default();
                result
                    .set_font_scale(font_scale.as_deref())
                    .map_err(serde::de::Error::custom)?;
                result
                    .set_line_spacing_reduction(line_spacing_reduction.as_deref())
                    .map_err(serde::de::Error::custom)?;
                Ok(result)
            }
        }
        deserializer.deserialize_struct(
            "NormalAutoFit",
            &["font_scale", "line_spacing_reduction"],
            Visitor,
        )
    }
}

impl NormalAutoFit {
    #[must_use]
    pub fn font_scale(&self) -> Option<&str> {
        self.font_scale.as_deref()
    }

    pub fn set_font_scale(&mut self, value: Option<&str>) -> Result<&mut Self, XlsxError> {
        self.font_scale = value
            .map(|value| Self::percentage(value, 1000, 100_000))
            .transpose()?;
        Ok(self)
    }

    #[must_use]
    pub fn line_spacing_reduction(&self) -> Option<&str> {
        self.line_spacing_reduction.as_deref()
    }

    pub fn set_line_spacing_reduction(
        &mut self,
        value: Option<&str>,
    ) -> Result<&mut Self, XlsxError> {
        self.line_spacing_reduction = value
            .map(|value| Self::percentage(value, 0, 13_200_000))
            .transpose()?;
        Ok(self)
    }

    fn percentage(value: &str, minimum: i32, maximum: i32) -> Result<String, XlsxError> {
        let value = crate::xml::trim_xml_whitespace(value);
        if value.ends_with('%') {
            // The percentage-string branch is not restricted by the integer
            // branch's bounds.
            super::SourceRectangle::validate_percentage(value)?;
        } else {
            let number = crate::xml::parse_i32(value).map_err(crate::xml::xml_error)?;
            if !(minimum..=maximum).contains(&number) {
                return Err(crate::xml::xml_error(
                    "text autofit percentage is out of range",
                ));
            }
        }
        Ok(value.to_owned())
    }

    fn read<R: std::io::BufRead>(
        reader: &mut Reader<R>,
        owner: &BytesStart<'_>,
        empty: bool,
    ) -> Result<Self, XlsxError> {
        let mut value = Self::default();
        let font_scale = crate::reader::driver::read_attribute(owner, b"fontScale")?;
        let reduction = crate::reader::driver::read_attribute(owner, b"lnSpcReduction")?;
        value.set_font_scale(font_scale.as_deref())?;
        value.set_line_spacing_reduction(reduction.as_deref())?;
        if !empty {
            crate::reader::driver::consume_empty_element(reader, owner)?;
        }
        Ok(value)
    }

    fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        let mut attributes: crate::structs::AttrCollection = Vec::new();
        if let Some(value) = self.font_scale() {
            attributes.push(("fontScale", value).into());
        }
        if let Some(value) = self.line_spacing_reduction() {
            attributes.push(("lnSpcReduction", value).into());
        }
        write_start_tag(writer, "a:normAutofit", attributes, true);
    }
}

impl TextAutoFit {
    pub(crate) fn read<R: std::io::BufRead>(
        reader: &mut Reader<R>,
        owner: &BytesStart<'_>,
        empty: bool,
    ) -> Result<Self, XlsxError> {
        match crate::reader::driver::local_name(owner.name().as_ref()) {
            b"noAutofit" => {
                ShapeAutoFit::set_attributes(reader, owner, empty)?;
                Ok(Self::Disabled)
            }
            b"spAutoFit" => {
                ShapeAutoFit::set_attributes(reader, owner, empty)?;
                Ok(Self::Shape(ShapeAutoFit::default()))
            }
            b"normAutofit" => Ok(Self::Normal(NormalAutoFit::read(reader, owner, empty)?)),
            _ => Err(crate::xml::xml_error("unsupported DrawingML text autofit")),
        }
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        match self {
            Self::Disabled => write_start_tag(writer, "a:noAutofit", vec![], true),
            Self::Normal(value) => value.write_to(writer),
            Self::Shape(_) => ShapeAutoFit::write_to(writer),
        }
    }
}
