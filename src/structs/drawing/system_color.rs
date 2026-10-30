// a:sysClr: one ordered native transform list (including repetitions).
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use super::{
    ColorTransform,
    PercentageType,
    SystemColorValues,
};
use crate::{
    StringValue,
    structs::EnumValue,
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct SystemColor {
    val:        EnumValue<SystemColorValues>,
    last_color: StringValue,
    #[cfg_attr(feature = "serde", serde(default))]
    transforms: Vec<ColorTransform>,
}
impl SystemColor {
    #[inline]
    #[must_use]
    pub fn val(&self) -> &SystemColorValues {
        self.val.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use val()")]
    pub fn get_val(&self) -> &SystemColorValues {
        self.val()
    }

    #[inline]
    pub fn set_val(&mut self, value: SystemColorValues) -> &mut Self {
        self.val.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn last_color(&self) -> &str {
        self.last_color.value_str()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use last_color()")]
    pub fn get_last_color(&self) -> &str {
        self.last_color()
    }

    #[inline]
    pub fn set_last_color<S: Into<String>>(&mut self, value: S) {
        self.last_color.set_value(value.into());
    }

    /// Last matching transform, without discarding its original lexical value.
    #[must_use]
    pub fn luminance_modulation(&self) -> Option<&str> {
        self.transforms
            .iter()
            .rev()
            .find_map(|transform| match transform {
                ColorTransform::LuminanceModulation(value) => Some(value.as_str()),
                _ => None,
            })
    }

    #[must_use]
    pub fn get_luminance_modulation(&self) -> Option<&str> {
        self.luminance_modulation()
    }

    pub fn luminance_modulation_mut(&mut self) -> Option<&mut String> {
        self.transforms
            .iter_mut()
            .rev()
            .find_map(|transform| match transform {
                ColorTransform::LuminanceModulation(value) => Some(value),
                _ => None,
            })
    }

    pub fn get_luminance_modulation_mut(&mut self) -> Option<&mut String> {
        self.luminance_modulation_mut()
    }

    pub fn set_luminance_modulation(&mut self, value: PercentageType) {
        let transform = value.into_value().to_string();
        if let Some(current) = self.luminance_modulation_mut() {
            *current = transform;
        } else {
            self.transforms
                .push(ColorTransform::LuminanceModulation(transform));
        }
    }

    /// Last matching transform, without discarding its original lexical value.
    #[must_use]
    pub fn luminance_offset(&self) -> Option<&str> {
        self.transforms
            .iter()
            .rev()
            .find_map(|transform| match transform {
                ColorTransform::LuminanceOffset(value) => Some(value.as_str()),
                _ => None,
            })
    }

    #[must_use]
    pub fn get_luminance_offset(&self) -> Option<&str> {
        self.luminance_offset()
    }

    pub fn luminance_offset_mut(&mut self) -> Option<&mut String> {
        self.transforms
            .iter_mut()
            .rev()
            .find_map(|transform| match transform {
                ColorTransform::LuminanceOffset(value) => Some(value),
                _ => None,
            })
    }

    pub fn get_luminance_offset_mut(&mut self) -> Option<&mut String> {
        self.luminance_offset_mut()
    }

    pub fn set_luminance_offset(&mut self, value: PercentageType) {
        let transform = value.into_value().to_string();
        if let Some(current) = self.luminance_offset_mut() {
            *current = transform;
        } else {
            self.transforms
                .push(ColorTransform::LuminanceOffset(transform));
        }
    }

    #[must_use]
    pub fn transforms(&self) -> &[ColorTransform] {
        &self.transforms
    }

    pub fn transforms_mut(&mut self) -> &mut Vec<ColorTransform> {
        &mut self.transforms
    }

    pub fn set_transforms(&mut self, values: Vec<ColorTransform>) -> &mut Self {
        self.transforms = values;
        self
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        owner: &BytesStart,
        empty: bool,
    ) -> Result<(), crate::XlsxError> {
        self.read_with_session(reader, owner, empty, None)
    }

    pub(crate) fn set_attributes_with_session<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        owner: &BytesStart,
        empty: bool,
        session: &crate::reader::xlsx::SourceCacheSession,
    ) -> Result<(), crate::XlsxError> {
        self.read_with_session(reader, owner, empty, Some(session))
    }

    pub(crate) fn read_with_session<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        owner: &BytesStart,
        empty: bool,
        session: Option<&crate::reader::xlsx::SourceCacheSession>,
    ) -> Result<(), crate::XlsxError> {
        let _guard = session
            .map(|s| s.enter_child_scope(owner))
            .transpose()?
            .flatten();
        let value = crate::reader::driver::require_attribute(owner, b"val")?;
        self.val.set_value(
            value
                .parse::<SystemColorValues>()
                .map_err(|()| crate::xml::xml_error("invalid DrawingML enumeration"))?,
        );
        if let Some(value) = crate::reader::driver::read_attribute(owner, b"lastClr")? {
            if value.len() != 6 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                return Err(crate::xml::xml_error("invalid DrawingML last color"));
            }
            self.last_color.set_value_string(value);
        }
        if !empty {
            self.transforms = super::color_transform::read_transforms(reader, owner, session)?;
        }
        Ok(())
    }

    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
    ) -> Result<(), crate::XlsxError> {
        let value = self.val.value_string();
        let mut attributes: crate::structs::AttrCollection = vec![("val", value).into()];
        if self.last_color.has_value() {
            attributes.push(("lastClr", self.last_color.value_str()).into());
        }
        write_start_tag(writer, "a:sysClr", attributes, self.transforms.is_empty());
        if !self.transforms.is_empty() {
            for transform in &self.transforms {
                transform.write_to(writer)?;
            }
            write_end_tag(writer, "a:sysClr");
        }
        Ok(())
    }
}
