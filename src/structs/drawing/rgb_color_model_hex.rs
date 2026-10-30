// a:srgbClr: one ordered native transform list (including repetitions).
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use super::{
    Alpha,
    ColorTransform,
    PercentageType,
    PositiveFixedPercentageType,
};
use crate::{
    StringValue,
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct RgbColorModelHex {
    val:        StringValue,
    #[cfg_attr(feature = "serde", serde(default))]
    transforms: Vec<ColorTransform>,
}
impl RgbColorModelHex {
    #[inline]
    #[must_use]
    pub fn val(&self) -> &str {
        self.val.value_str()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use val()")]
    pub fn get_val(&self) -> &str {
        self.val()
    }

    #[inline]
    pub fn set_val<S: Into<String>>(&mut self, value: S) -> &mut Self {
        self.val.set_value(value);
        self
    }

    /// Last matching transform, without discarding its original lexical value.
    #[must_use]
    pub fn luminance(&self) -> Option<&str> {
        self.transforms
            .iter()
            .rev()
            .find_map(|transform| match transform {
                ColorTransform::Luminance(value) => Some(value.as_str()),
                _ => None,
            })
    }

    #[must_use]
    pub fn get_luminance(&self) -> Option<&str> {
        self.luminance()
    }

    pub fn luminance_mut(&mut self) -> Option<&mut String> {
        self.transforms
            .iter_mut()
            .rev()
            .find_map(|transform| match transform {
                ColorTransform::Luminance(value) => Some(value),
                _ => None,
            })
    }

    pub fn get_luminance_mut(&mut self) -> Option<&mut String> {
        self.luminance_mut()
    }

    pub fn set_luminance(&mut self, value: PercentageType) {
        let transform = value.into_value().to_string();
        if let Some(current) = self.luminance_mut() {
            *current = transform;
        } else {
            self.transforms.push(ColorTransform::Luminance(transform));
        }
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

    /// Last matching transform, without discarding its original lexical value.
    #[must_use]
    pub fn saturation(&self) -> Option<&str> {
        self.transforms
            .iter()
            .rev()
            .find_map(|transform| match transform {
                ColorTransform::Saturation(value) => Some(value.as_str()),
                _ => None,
            })
    }

    #[must_use]
    pub fn get_saturation(&self) -> Option<&str> {
        self.saturation()
    }

    pub fn saturation_mut(&mut self) -> Option<&mut String> {
        self.transforms
            .iter_mut()
            .rev()
            .find_map(|transform| match transform {
                ColorTransform::Saturation(value) => Some(value),
                _ => None,
            })
    }

    pub fn get_saturation_mut(&mut self) -> Option<&mut String> {
        self.saturation_mut()
    }

    pub fn set_saturation(&mut self, value: PercentageType) {
        let transform = value.into_value().to_string();
        if let Some(current) = self.saturation_mut() {
            *current = transform;
        } else {
            self.transforms.push(ColorTransform::Saturation(transform));
        }
    }

    /// Last matching transform, without discarding its original lexical value.
    #[must_use]
    pub fn saturation_modulation(&self) -> Option<&str> {
        self.transforms
            .iter()
            .rev()
            .find_map(|transform| match transform {
                ColorTransform::SaturationModulation(value) => Some(value.as_str()),
                _ => None,
            })
    }

    #[must_use]
    pub fn get_saturation_modulation(&self) -> Option<&str> {
        self.saturation_modulation()
    }

    pub fn saturation_modulation_mut(&mut self) -> Option<&mut String> {
        self.transforms
            .iter_mut()
            .rev()
            .find_map(|transform| match transform {
                ColorTransform::SaturationModulation(value) => Some(value),
                _ => None,
            })
    }

    pub fn get_saturation_modulation_mut(&mut self) -> Option<&mut String> {
        self.saturation_modulation_mut()
    }

    pub fn set_saturation_modulation(&mut self, value: PercentageType) {
        let transform = value.into_value().to_string();
        if let Some(current) = self.saturation_modulation_mut() {
            *current = transform;
        } else {
            self.transforms
                .push(ColorTransform::SaturationModulation(transform));
        }
    }

    /// Last matching transform, without discarding its original lexical value.
    #[must_use]
    pub fn shade(&self) -> Option<&str> {
        self.transforms
            .iter()
            .rev()
            .find_map(|transform| match transform {
                ColorTransform::Shade(value) => Some(value.as_str()),
                _ => None,
            })
    }

    #[must_use]
    pub fn get_shade(&self) -> Option<&str> {
        self.shade()
    }

    pub fn shade_mut(&mut self) -> Option<&mut String> {
        self.transforms
            .iter_mut()
            .rev()
            .find_map(|transform| match transform {
                ColorTransform::Shade(value) => Some(value),
                _ => None,
            })
    }

    pub fn get_shade_mut(&mut self) -> Option<&mut String> {
        self.shade_mut()
    }

    pub fn set_shade(&mut self, value: PositiveFixedPercentageType) {
        let transform = value.into_value().to_string();
        if let Some(current) = self.shade_mut() {
            *current = transform;
        } else {
            self.transforms.push(ColorTransform::Shade(transform));
        }
    }

    /// Last matching transform, without discarding its original lexical value.
    #[must_use]
    pub fn alpha(&self) -> Option<&Alpha> {
        self.transforms
            .iter()
            .rev()
            .find_map(|transform| match transform {
                ColorTransform::Alpha(value) => Some(value),
                _ => None,
            })
    }

    #[must_use]
    pub fn get_alpha(&self) -> Option<&Alpha> {
        self.alpha()
    }

    pub fn alpha_mut(&mut self) -> Option<&mut Alpha> {
        self.transforms
            .iter_mut()
            .rev()
            .find_map(|transform| match transform {
                ColorTransform::Alpha(value) => Some(value),
                _ => None,
            })
    }

    pub fn get_alpha_mut(&mut self) -> Option<&mut Alpha> {
        self.alpha_mut()
    }

    pub fn set_alpha(&mut self, value: PositiveFixedPercentageType) {
        let mut transform = Alpha::default();
        transform.set_val(value.into_value().to_string());
        if let Some(current) = self.alpha_mut() {
            *current = transform;
        } else {
            self.transforms.push(ColorTransform::Alpha(transform));
        }
    }

    /// Last matching transform, without discarding its original lexical value.
    #[must_use]
    pub fn tint(&self) -> Option<&str> {
        self.transforms
            .iter()
            .rev()
            .find_map(|transform| match transform {
                ColorTransform::Tint(value) => Some(value.as_str()),
                _ => None,
            })
    }

    #[must_use]
    pub fn get_tint(&self) -> Option<&str> {
        self.tint()
    }

    pub fn tint_mut(&mut self) -> Option<&mut String> {
        self.transforms
            .iter_mut()
            .rev()
            .find_map(|transform| match transform {
                ColorTransform::Tint(value) => Some(value),
                _ => None,
            })
    }

    pub fn get_tint_mut(&mut self) -> Option<&mut String> {
        self.tint_mut()
    }

    pub fn set_tint(&mut self, value: PositiveFixedPercentageType) {
        let transform = value.into_value().to_string();
        if let Some(current) = self.tint_mut() {
            *current = transform;
        } else {
            self.transforms.push(ColorTransform::Tint(transform));
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

    pub(crate) fn with_inner_params(&self) -> bool {
        !self.transforms.is_empty()
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
        if value.len() != 6 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(crate::xml::xml_error("invalid DrawingML RGB color"));
        }
        self.val.set_value_string(value);
        if !empty {
            self.transforms = super::color_transform::read_transforms(reader, owner, session)?;
        }
        Ok(())
    }

    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
    ) -> Result<(), crate::XlsxError> {
        let value = self.val.value_str();
        let attributes: crate::structs::AttrCollection = vec![("val", value).into()];
        write_start_tag(writer, "a:srgbClr", attributes, self.transforms.is_empty());
        if !self.transforms.is_empty() {
            for transform in &self.transforms {
                transform.write_to(writer)?;
            }
            write_end_tag(writer, "a:srgbClr");
        }
        Ok(())
    }
}
