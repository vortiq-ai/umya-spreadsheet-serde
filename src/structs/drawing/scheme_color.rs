// a:schemeClr: one ordered native transform list (including repetitions).
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
    SchemeColorValues,
};
use crate::{
    structs::EnumValue,
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Clone, Default, Debug)]
pub struct SchemeColor {
    val:        EnumValue<SchemeColorValues>,
    transforms: Vec<ColorTransform>,
}
impl SchemeColor {
    #[inline]
    #[must_use]
    pub fn val(&self) -> &SchemeColorValues {
        self.val.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use val()")]
    pub fn get_val(&self) -> &SchemeColorValues {
        self.val()
    }

    #[inline]
    pub fn set_val(&mut self, value: SchemeColorValues) -> &mut Self {
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
        self.val.set_value(
            value
                .parse::<SchemeColorValues>()
                .map_err(|()| crate::xml::xml_error("invalid DrawingML enumeration"))?,
        );
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
        let attributes: crate::structs::AttrCollection = vec![("val", value).into()];
        write_start_tag(
            writer,
            "a:schemeClr",
            attributes,
            self.transforms.is_empty(),
        );
        if !self.transforms.is_empty() {
            for transform in &self.transforms {
                transform.write_to(writer)?;
            }
            write_end_tag(writer, "a:schemeClr");
        }
        Ok(())
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for SchemeColor {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        const FIELDS: &[&str] = &[
            "val",
            "transforms",
            "luminance",
            "luminance_modulation",
            "luminance_offset",
            "saturation",
            "saturation_modulation",
            "shade",
            "alpha",
            "tint",
        ];
        struct ColourVisitor;
        impl<'de> serde::de::Visitor<'de> for ColourVisitor {
            type Value = SchemeColor;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a DrawingML colour")
            }

            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> Result<Self::Value, A::Error> {
                let val = seq
                    .next_element()?
                    .ok_or_else(|| serde::de::Error::invalid_length(0, &self))?;
                let transforms = seq
                    .next_element()?
                    .ok_or_else(|| serde::de::Error::invalid_length(1, &self))?;
                if seq.next_element::<serde::de::IgnoredAny>()?.is_some() {
                    return Err(serde::de::Error::invalid_length(3, &self));
                }
                Ok(SchemeColor { val, transforms })
            }

            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> Result<Self::Value, A::Error> {
                let mut val: Option<EnumValue<SchemeColorValues>> = None;
                let mut transforms: Option<Vec<ColorTransform>> = None;
                let mut luminance: Option<Option<LegacyPercentage>> = None;
                let mut luminance_modulation: Option<Option<LegacyPercentage>> = None;
                let mut luminance_offset: Option<Option<LegacyPercentage>> = None;
                let mut saturation: Option<Option<LegacyPercentage>> = None;
                let mut saturation_modulation: Option<Option<LegacyPercentage>> = None;
                let mut shade: Option<Option<LegacyPercentage>> = None;
                let mut alpha: Option<Option<LegacyPercentage>> = None;
                let mut tint: Option<Option<LegacyPercentage>> = None;
                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "val" => {
                            if val.is_some() {
                                return Err(serde::de::Error::duplicate_field("val"));
                            }
                            val = Some(map.next_value()?);
                        }
                        "transforms" => {
                            if transforms.is_some() {
                                return Err(serde::de::Error::duplicate_field("transforms"));
                            }
                            transforms = Some(map.next_value()?);
                        }
                        "luminance" => {
                            if luminance.is_some() {
                                return Err(serde::de::Error::duplicate_field("luminance"));
                            }
                            luminance = Some(map.next_value()?);
                        }
                        "luminance_modulation" => {
                            if luminance_modulation.is_some() {
                                return Err(serde::de::Error::duplicate_field(
                                    "luminance_modulation",
                                ));
                            }
                            luminance_modulation = Some(map.next_value()?);
                        }
                        "luminance_offset" => {
                            if luminance_offset.is_some() {
                                return Err(serde::de::Error::duplicate_field("luminance_offset"));
                            }
                            luminance_offset = Some(map.next_value()?);
                        }
                        "saturation" => {
                            if saturation.is_some() {
                                return Err(serde::de::Error::duplicate_field("saturation"));
                            }
                            saturation = Some(map.next_value()?);
                        }
                        "saturation_modulation" => {
                            if saturation_modulation.is_some() {
                                return Err(serde::de::Error::duplicate_field(
                                    "saturation_modulation",
                                ));
                            }
                            saturation_modulation = Some(map.next_value()?);
                        }
                        "shade" => {
                            if shade.is_some() {
                                return Err(serde::de::Error::duplicate_field("shade"));
                            }
                            shade = Some(map.next_value()?);
                        }
                        "alpha" => {
                            if alpha.is_some() {
                                return Err(serde::de::Error::duplicate_field("alpha"));
                            }
                            alpha = Some(map.next_value()?);
                        }
                        "tint" => {
                            if tint.is_some() {
                                return Err(serde::de::Error::duplicate_field("tint"));
                            }
                            tint = Some(map.next_value()?);
                        }
                        _ => return Err(serde::de::Error::unknown_field(&key, FIELDS)),
                    }
                }
                let val = val.ok_or_else(|| serde::de::Error::missing_field("val"))?;
                let legacy = luminance.is_some()
                    || luminance_modulation.is_some()
                    || luminance_offset.is_some()
                    || saturation.is_some()
                    || saturation_modulation.is_some()
                    || shade.is_some()
                    || alpha.is_some()
                    || tint.is_some();
                if transforms.is_some() && legacy {
                    return Err(serde::de::Error::custom(
                        "conflicting current and legacy colour transforms",
                    ));
                }
                if let Some(transforms) = transforms {
                    return Ok(SchemeColor { val, transforms });
                }
                // Preserve the emission order of the legacy OOXML writer.
                let mut transforms = Vec::new();
                if let Some(value) = luminance.flatten().filter(|value| value.val.has_value()) {
                    transforms.push(ColorTransform::Luminance(value.val.value().to_string()));
                }
                if let Some(value) = luminance_modulation
                    .flatten()
                    .filter(|value| value.val.has_value())
                {
                    transforms.push(ColorTransform::LuminanceModulation(
                        value.val.value().to_string(),
                    ));
                }
                if let Some(value) = luminance_offset
                    .flatten()
                    .filter(|value| value.val.has_value())
                {
                    transforms.push(ColorTransform::LuminanceOffset(
                        value.val.value().to_string(),
                    ));
                }
                if let Some(value) = saturation.flatten().filter(|value| value.val.has_value()) {
                    transforms.push(ColorTransform::Saturation(value.val.value().to_string()));
                }
                if let Some(value) = saturation_modulation
                    .flatten()
                    .filter(|value| value.val.has_value())
                {
                    transforms.push(ColorTransform::SaturationModulation(
                        value.val.value().to_string(),
                    ));
                }
                if let Some(value) = shade.flatten().filter(|value| value.val.has_value()) {
                    transforms.push(ColorTransform::Shade(value.val.value().to_string()));
                }
                if let Some(value) = alpha.flatten().filter(|value| value.val.has_value()) {
                    let mut alpha = Alpha::default();
                    alpha.set_val(value.val.value().to_string());
                    transforms.push(ColorTransform::Alpha(alpha));
                }
                if let Some(value) = tint.flatten().filter(|value| value.val.has_value()) {
                    transforms.push(ColorTransform::Tint(value.val.value().to_string()));
                }
                for transform in &transforms {
                    transform.validate().map_err(serde::de::Error::custom)?;
                }
                Ok(SchemeColor { val, transforms })
            }
        }
        deserializer.deserialize_struct("SchemeColor", &["val", "transforms"], ColourVisitor)
    }
}

#[cfg(feature = "serde")]
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LegacyPercentage {
    pub(super) val: crate::Int32Value,
}
