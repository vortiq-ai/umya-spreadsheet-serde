// xdr:spPr
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::{
        BytesStart,
        Event,
    },
};

use super::super::{
    BlipFill,
    EffectList,
    ExtensionList,
    NoFill,
    Outline,
    PatternFill,
    PresetGeometry,
    SolidFill,
    Transform2D,
};
use crate::{
    EnumValue,
    drawing::{
        BlackWhiteModeValues,
        GradientFill,
    },
    structs::raw::RawRelationships,
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Clone, Default, Debug)]
pub struct ShapeProperties {
    transform2d:          Option<Box<Transform2D>>,
    preset_geometry:      PresetGeometry,
    blip_fill:            Option<Box<BlipFill>>,
    solid_fill:           Option<Box<SolidFill>>,
    gradient_fill:        Option<Box<GradientFill>>,
    outline:              Option<Box<Outline>>,
    effect_list:          Option<Box<EffectList>>,
    no_fill:              Option<NoFill>,
    extension_list:       Option<ExtensionList>,
    black_white_mode:     EnumValue<BlackWhiteModeValues>,
    #[cfg_attr(feature = "serde", serde(default))]
    preserved_properties: Vec<crate::drawing::charts::OpaqueElement>,
    #[cfg_attr(feature = "serde", serde(default))]
    pattern_fill:         Option<Box<PatternFill>>,
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for ShapeProperties {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        const FIELDS: &[&str] = &[
            "transform2d",
            "preset_geometry",
            "blip_fill",
            "solid_fill",
            "gradient_fill",
            "outline",
            "effect_list",
            "no_fill",
            "extension_list",
            "black_white_mode",
            "preserved_properties",
            "pattern_fill",
        ];
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = ShapeProperties;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("spreadsheet shape properties")
            }

            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> Result<Self::Value, A::Error> {
                let value = ShapeProperties {
                    transform2d:          seq
                        .next_element()?
                        .ok_or_else(|| serde::de::Error::invalid_length(0, &self))?,
                    preset_geometry:      seq
                        .next_element()?
                        .ok_or_else(|| serde::de::Error::invalid_length(1, &self))?,
                    blip_fill:            seq
                        .next_element()?
                        .ok_or_else(|| serde::de::Error::invalid_length(2, &self))?,
                    solid_fill:           seq
                        .next_element()?
                        .ok_or_else(|| serde::de::Error::invalid_length(3, &self))?,
                    gradient_fill:        seq
                        .next_element()?
                        .ok_or_else(|| serde::de::Error::invalid_length(4, &self))?,
                    outline:              seq
                        .next_element()?
                        .ok_or_else(|| serde::de::Error::invalid_length(5, &self))?,
                    effect_list:          seq
                        .next_element()?
                        .ok_or_else(|| serde::de::Error::invalid_length(6, &self))?,
                    no_fill:              seq
                        .next_element()?
                        .ok_or_else(|| serde::de::Error::invalid_length(7, &self))?,
                    extension_list:       seq
                        .next_element()?
                        .ok_or_else(|| serde::de::Error::invalid_length(8, &self))?,
                    black_white_mode:     seq
                        .next_element()?
                        .ok_or_else(|| serde::de::Error::invalid_length(9, &self))?,
                    preserved_properties: seq
                        .next_element()?
                        .ok_or_else(|| serde::de::Error::invalid_length(10, &self))?,
                    pattern_fill:         seq
                        .next_element()?
                        .ok_or_else(|| serde::de::Error::invalid_length(11, &self))?,
                };
                if seq.next_element::<serde::de::IgnoredAny>()?.is_some() {
                    return Err(serde::de::Error::invalid_length(13, &self));
                }
                Ok(value)
            }

            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> Result<Self::Value, A::Error> {
                let mut transform2d = None;
                let mut preset_geometry = None;
                let mut blip_fill = None;
                let mut solid_fill = None;
                let mut gradient_fill = None;
                let mut outline = None;
                let mut effect_list = None;
                let mut no_fill = None;
                let mut extension_list = None;
                let mut black_white_mode = None;
                let mut preserved_properties = None;
                let mut pattern_fill = None;
                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "transform2d" => {
                            if transform2d.is_some() {
                                return Err(serde::de::Error::duplicate_field("transform2d"));
                            }
                            transform2d = Some(map.next_value()?);
                        }
                        "preset_geometry" => {
                            if preset_geometry.is_some() {
                                return Err(serde::de::Error::duplicate_field("preset_geometry"));
                            }
                            preset_geometry = Some(map.next_value()?);
                        }
                        "blip_fill" => {
                            if blip_fill.is_some() {
                                return Err(serde::de::Error::duplicate_field("blip_fill"));
                            }
                            blip_fill = Some(map.next_value()?);
                        }
                        "solid_fill" => {
                            if solid_fill.is_some() {
                                return Err(serde::de::Error::duplicate_field("solid_fill"));
                            }
                            solid_fill = Some(map.next_value()?);
                        }
                        "gradient_fill" => {
                            if gradient_fill.is_some() {
                                return Err(serde::de::Error::duplicate_field("gradient_fill"));
                            }
                            gradient_fill = Some(map.next_value::<DeferredGradientValue>()?);
                        }
                        "outline" => {
                            if outline.is_some() {
                                return Err(serde::de::Error::duplicate_field("outline"));
                            }
                            outline = Some(map.next_value()?);
                        }
                        "effect_list" => {
                            if effect_list.is_some() {
                                return Err(serde::de::Error::duplicate_field("effect_list"));
                            }
                            effect_list = Some(map.next_value()?);
                        }
                        "no_fill" => {
                            if no_fill.is_some() {
                                return Err(serde::de::Error::duplicate_field("no_fill"));
                            }
                            no_fill = Some(map.next_value()?);
                        }
                        "extension_list" => {
                            if extension_list.is_some() {
                                return Err(serde::de::Error::duplicate_field("extension_list"));
                            }
                            extension_list = Some(map.next_value()?);
                        }
                        "black_white_mode" => {
                            if black_white_mode.is_some() {
                                return Err(serde::de::Error::duplicate_field("black_white_mode"));
                            }
                            black_white_mode = Some(map.next_value()?);
                        }
                        "preserved_properties" => {
                            if preserved_properties.is_some() {
                                return Err(serde::de::Error::duplicate_field(
                                    "preserved_properties",
                                ));
                            }
                            preserved_properties = Some(map.next_value()?);
                        }
                        "pattern_fill" => {
                            if pattern_fill.is_some() {
                                return Err(serde::de::Error::duplicate_field("pattern_fill"));
                            }
                            pattern_fill = Some(map.next_value()?);
                        }
                        _ => return Err(serde::de::Error::unknown_field(&key, FIELDS)),
                    }
                }
                // Historical JSON retained gradients that its writer never
                // emitted. The new native property list
                // distinguishes maps whose gradients really are
                // part of the authoritative drawing.
                let gradient_fill = if preserved_properties.is_some() {
                    gradient_fill
                        .map(serde::Deserialize::deserialize)
                        .transpose()
                        .map_err(serde::de::Error::custom)?
                        .flatten()
                } else {
                    if gradient_fill.as_ref().is_some_and(|value| {
                        !matches!(
                            value,
                            DeferredGradientValue::Unit | DeferredGradientValue::Map(_)
                        )
                    }) {
                        return Err(serde::de::Error::custom(
                            "historical gradient field must be a map or null",
                        ));
                    }
                    // Unused legacy map contents are discarded without current
                    // nested validation, not revalidated against obsolete
                    // serde.
                    None
                };
                Ok(ShapeProperties {
                    transform2d: transform2d.unwrap_or(None),
                    preset_geometry: preset_geometry
                        .ok_or_else(|| serde::de::Error::missing_field("preset_geometry"))?,
                    blip_fill: blip_fill.unwrap_or(None),
                    solid_fill: solid_fill.unwrap_or(None),
                    gradient_fill,
                    outline: outline.unwrap_or(None),
                    effect_list: effect_list.unwrap_or(None),
                    no_fill: no_fill.unwrap_or(None),
                    extension_list: extension_list.unwrap_or(None),
                    black_white_mode: black_white_mode
                        .ok_or_else(|| serde::de::Error::missing_field("black_white_mode"))?,
                    preserved_properties: preserved_properties.unwrap_or_default(),
                    pattern_fill: pattern_fill.unwrap_or(None),
                })
            }
        }
        deserializer.deserialize_struct("ShapeProperties", FIELDS, Visitor)
    }
}

// Buffer only this self-describing map field until its format marker is known.
// Discarded historical gradients must not enter the current native validators;
// binary sequences above still deserialize GradientFill directly.
#[cfg(feature = "serde")]
enum DeferredGradientValue {
    Unit,
    Boolean(bool),
    Signed(i64),
    Unsigned(u64),
    Float(f64),
    String(String),
    Sequence(Vec<Self>),
    Map(Vec<(String, Self)>),
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for DeferredGradientValue {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = DeferredGradientValue;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a self-describing gradient field")
            }

            fn visit_unit<E: serde::de::Error>(self) -> Result<Self::Value, E> {
                Ok(DeferredGradientValue::Unit)
            }

            fn visit_none<E: serde::de::Error>(self) -> Result<Self::Value, E> {
                Ok(DeferredGradientValue::Unit)
            }

            fn visit_some<D: serde::Deserializer<'de>>(
                self,
                deserializer: D,
            ) -> Result<Self::Value, D::Error> {
                serde::Deserialize::deserialize(deserializer)
            }

            fn visit_bool<E: serde::de::Error>(self, value: bool) -> Result<Self::Value, E> {
                Ok(DeferredGradientValue::Boolean(value))
            }

            fn visit_i64<E: serde::de::Error>(self, value: i64) -> Result<Self::Value, E> {
                Ok(DeferredGradientValue::Signed(value))
            }

            fn visit_u64<E: serde::de::Error>(self, value: u64) -> Result<Self::Value, E> {
                Ok(DeferredGradientValue::Unsigned(value))
            }

            fn visit_f64<E: serde::de::Error>(self, value: f64) -> Result<Self::Value, E> {
                Ok(DeferredGradientValue::Float(value))
            }

            fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Self::Value, E> {
                Ok(DeferredGradientValue::String(value.to_owned()))
            }

            fn visit_string<E: serde::de::Error>(self, value: String) -> Result<Self::Value, E> {
                Ok(DeferredGradientValue::String(value))
            }

            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut sequence: A,
            ) -> Result<Self::Value, A::Error> {
                let mut values = Vec::new();
                while let Some(value) = sequence.next_element()? {
                    values.push(value);
                }
                Ok(DeferredGradientValue::Sequence(values))
            }

            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> Result<Self::Value, A::Error> {
                let mut values = Vec::new();
                while let Some(value) = map.next_entry()? {
                    values.push(value);
                }
                Ok(DeferredGradientValue::Map(values))
            }
        }
        deserializer.deserialize_any(Visitor)
    }
}

#[cfg(feature = "serde")]
impl serde::de::IntoDeserializer<'_, serde::de::value::Error> for DeferredGradientValue {
    type Deserializer = Self;

    fn into_deserializer(self) -> Self::Deserializer {
        self
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserializer<'de> for DeferredGradientValue {
    type Error = serde::de::value::Error;

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf unit unit_struct seq tuple tuple_struct map struct
        identifier ignored_any
    }

    fn deserialize_any<V: serde::de::Visitor<'de>>(
        self,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        match self {
            Self::Unit => visitor.visit_unit(),
            Self::Boolean(value) => visitor.visit_bool(value),
            Self::Signed(value) => visitor.visit_i64(value),
            Self::Unsigned(value) => visitor.visit_u64(value),
            Self::Float(value) => visitor.visit_f64(value),
            Self::String(value) => visitor.visit_string(value),
            Self::Sequence(values) => serde::Deserializer::deserialize_any(
                serde::de::value::SeqDeserializer::new(values.into_iter()),
                visitor,
            ),
            Self::Map(values) => serde::Deserializer::deserialize_any(
                serde::de::value::MapDeserializer::new(values.into_iter()),
                visitor,
            ),
        }
    }

    fn deserialize_option<V: serde::de::Visitor<'de>>(
        self,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        match self {
            Self::Unit => visitor.visit_none(),
            value => visitor.visit_some(value),
        }
    }

    fn deserialize_enum<V: serde::de::Visitor<'de>>(
        self,
        name: &'static str,
        variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        match self {
            Self::String(value) => serde::Deserializer::deserialize_enum(
                serde::de::value::StringDeserializer::new(value),
                name,
                variants,
                visitor,
            ),
            Self::Map(values) if values.len() == 1 => serde::Deserializer::deserialize_enum(
                serde::de::value::MapAccessDeserializer::new(
                    serde::de::value::MapDeserializer::new(values.into_iter()),
                ),
                name,
                variants,
                visitor,
            ),
            _ => Err(serde::de::Error::custom(
                "expected a string or single-entry gradient enum",
            )),
        }
    }

    fn deserialize_newtype_struct<V: serde::de::Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        visitor.visit_newtype_struct(self)
    }
}

impl ShapeProperties {
    pub(crate) fn strip_export_parts(&mut self) {
        self.extension_list = None;
        self.preserved_properties.clear();
        self.blip_fill = None;
    }

    pub(crate) fn restore_export_parts(&mut self, previous: &Self) {
        self.extension_list.clone_from(&previous.extension_list);
        self.preserved_properties
            .clone_from(&previous.preserved_properties);
        if !self.preset_geometry.geometry().is_empty() {
            self.preserved_properties
                .retain(|value| value.name.local_name != "custGeom");
        }
        if self.effect_list.is_some() {
            self.preserved_properties.retain(|value| {
                !matches!(value.name.local_name.as_str(), "effectLst" | "effectDag")
            });
        }
        if self.solid_fill.is_some()
            || self.gradient_fill.is_some()
            || self.pattern_fill.is_some()
            || self.no_fill.is_some()
        {
            self.blip_fill = None;
            self.clear_preserved_fills();
        } else {
            self.blip_fill.clone_from(&previous.blip_fill);
        }
    }

    fn clear_preserved_fills(&mut self) {
        self.preserved_properties
            .retain(|value| !matches!(value.name.local_name.as_str(), "pattFill" | "grpFill"));
    }

    fn clear_fills(&mut self) {
        self.clear_preserved_fills();
        self.blip_fill = None;
        self.solid_fill = None;
        self.gradient_fill = None;
        self.pattern_fill = None;
        self.no_fill = None;
    }

    #[must_use]
    pub fn pattern_fill(&self) -> Option<&PatternFill> {
        self.pattern_fill.as_deref()
    }

    pub fn pattern_fill_mut(&mut self) -> Option<&mut PatternFill> {
        self.pattern_fill.as_deref_mut()
    }

    pub fn set_pattern_fill(&mut self, value: PatternFill) -> &mut Self {
        self.clear_fills();
        self.pattern_fill = Some(Box::new(value));
        self
    }

    pub fn remove_pattern_fill(&mut self) -> &mut Self {
        self.pattern_fill = None;
        self
    }

    #[inline]
    #[must_use]
    pub fn transform2d(&self) -> Option<&Transform2D> {
        self.transform2d.as_deref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use transform2d()")]
    pub fn get_transform2d(&self) -> Option<&Transform2D> {
        self.transform2d()
    }

    #[inline]
    pub fn transform2d_mut(&mut self) -> Option<&mut Transform2D> {
        self.transform2d.as_deref_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use transform2d_mut()")]
    pub fn get_transform2d_mut(&mut self) -> Option<&mut Transform2D> {
        self.transform2d_mut()
    }

    #[inline]
    pub fn set_transform2d(&mut self, value: Transform2D) -> &mut Self {
        self.transform2d = Some(Box::new(value));
        self
    }

    #[inline]
    #[must_use]
    pub fn geometry(&self) -> &PresetGeometry {
        &self.preset_geometry
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use geometry()")]
    pub fn get_geometry(&self) -> &PresetGeometry {
        self.geometry()
    }

    #[inline]
    pub fn geometry_mut(&mut self) -> &mut PresetGeometry {
        self.preserved_properties
            .retain(|value| value.name.local_name != "custGeom");
        &mut self.preset_geometry
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use geometry_mut()")]
    pub fn get_geometry_mut(&mut self) -> &mut PresetGeometry {
        self.geometry_mut()
    }

    #[inline]
    pub fn set_geometry(&mut self, value: PresetGeometry) -> &mut Self {
        self.preserved_properties
            .retain(|value| value.name.local_name != "custGeom");
        self.preset_geometry = value;
        self
    }

    #[inline]
    #[must_use]
    pub fn blip_fill(&self) -> Option<&BlipFill> {
        self.blip_fill.as_deref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use blip_fill()")]
    pub fn get_blip_fill(&self) -> Option<&BlipFill> {
        self.blip_fill()
    }

    #[inline]
    pub fn blip_fill_mut(&mut self) -> Option<&mut BlipFill> {
        self.blip_fill.as_deref_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use blip_fill_mut()")]
    pub fn get_blip_fill_mut(&mut self) -> Option<&mut BlipFill> {
        self.blip_fill_mut()
    }

    #[inline]
    pub fn set_blip_fill(&mut self, value: BlipFill) -> &mut Self {
        self.clear_fills();
        self.blip_fill = Some(Box::new(value));
        self
    }

    #[inline]
    #[must_use]
    pub fn solid_fill(&self) -> Option<&SolidFill> {
        self.solid_fill.as_deref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use solid_fill()")]
    pub fn get_solid_fill(&self) -> Option<&SolidFill> {
        self.solid_fill()
    }

    #[inline]
    pub fn solid_fill_mut(&mut self) -> Option<&mut SolidFill> {
        self.solid_fill.as_deref_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use solid_fill_mut()")]
    pub fn get_solid_fill_mut(&mut self) -> Option<&mut SolidFill> {
        self.solid_fill_mut()
    }

    #[inline]
    pub fn set_solid_fill(&mut self, value: SolidFill) -> &mut Self {
        self.clear_fills();
        self.solid_fill = Some(Box::new(value));
        self
    }

    #[inline]
    #[must_use]
    pub fn gradient_fill(&self) -> Option<&GradientFill> {
        self.gradient_fill.as_deref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use gradient_fill()")]
    pub fn get_gradient_fill(&self) -> Option<&GradientFill> {
        self.gradient_fill()
    }

    #[inline]
    pub fn gradient_fill_mut(&mut self) -> Option<&mut GradientFill> {
        self.gradient_fill.as_deref_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use gradient_fill_mut()")]
    pub fn get_gradient_fill_mut(&mut self) -> Option<&mut GradientFill> {
        self.gradient_fill_mut()
    }

    #[inline]
    pub fn set_gradient_fill(&mut self, value: GradientFill) -> &mut Self {
        self.clear_fills();
        self.gradient_fill = Some(Box::new(value));
        self
    }

    #[inline]
    #[must_use]
    pub fn outline(&self) -> Option<&Outline> {
        self.outline.as_deref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use outline()")]
    pub fn get_outline(&self) -> Option<&Outline> {
        self.outline()
    }

    #[inline]
    pub fn outline_mut(&mut self) -> Option<&mut Outline> {
        self.outline.as_deref_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use outline_mut()")]
    pub fn get_outline_mut(&mut self) -> Option<&mut Outline> {
        self.outline_mut()
    }

    #[inline]
    pub fn set_outline(&mut self, value: Outline) -> &mut Self {
        self.outline = Some(Box::new(value));
        self
    }

    #[inline]
    #[must_use]
    pub fn effect_list(&self) -> Option<&EffectList> {
        self.effect_list.as_deref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use effect_list()")]
    pub fn get_effect_list(&self) -> Option<&EffectList> {
        self.effect_list()
    }

    #[inline]
    pub fn effect_list_mut(&mut self) -> Option<&mut EffectList> {
        self.effect_list.as_deref_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use effect_list_mut()")]
    pub fn get_effect_list_mut(&mut self) -> Option<&mut EffectList> {
        self.effect_list_mut()
    }

    #[inline]
    pub fn set_effect_list(&mut self, value: EffectList) -> &mut Self {
        self.preserved_properties
            .retain(|value| !matches!(value.name.local_name.as_str(), "effectLst" | "effectDag"));
        self.effect_list = Some(Box::new(value));
        self
    }

    #[inline]
    #[must_use]
    pub fn no_fill(&self) -> Option<&NoFill> {
        self.no_fill.as_ref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use no_fill()")]
    pub fn get_no_fill(&self) -> Option<&NoFill> {
        self.no_fill()
    }

    #[inline]
    pub fn no_fill_mut(&mut self) -> Option<&mut NoFill> {
        self.no_fill.as_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use no_fill_mut()")]
    pub fn get_no_fill_mut(&mut self) -> Option<&mut NoFill> {
        self.no_fill_mut()
    }

    #[inline]
    pub fn set_no_fill(&mut self, value: NoFill) -> &mut Self {
        self.clear_fills();
        self.no_fill = Some(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn extension_list(&self) -> Option<&ExtensionList> {
        self.extension_list.as_ref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use extension_list()")]
    pub fn get_extension_list(&self) -> Option<&ExtensionList> {
        self.extension_list()
    }

    #[inline]
    pub fn extension_list_mut(&mut self) -> Option<&mut ExtensionList> {
        self.extension_list.as_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use extension_list_mut()")]
    pub fn get_extension_list_mut(&mut self) -> Option<&mut ExtensionList> {
        self.extension_list_mut()
    }

    #[inline]
    pub fn set_extension_list(&mut self, value: ExtensionList) -> &mut Self {
        self.extension_list = Some(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn black_white_mode(&self) -> &BlackWhiteModeValues {
        self.black_white_mode.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use black_white_mode()")]
    pub fn get_black_white_mode(&self) -> &BlackWhiteModeValues {
        self.black_white_mode()
    }

    #[inline]
    pub fn set_black_white_mode(&mut self, value: BlackWhiteModeValues) -> &mut Self {
        self.black_white_mode.set_value(value);
        self
    }

    pub(crate) fn read_with_session<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        owner: &BytesStart,
        empty: bool,
        drawing_relationships: Option<&RawRelationships>,
        session: &crate::reader::xlsx::SourceCacheSession,
    ) -> Result<(), crate::XlsxError> {
        let _guard = session.enter_child_scope(owner)?;
        let scope = crate::drawing::charts::NamespaceScope::default()
            .child(owner)
            .map_err(crate::xml::xml_error)?;
        if let Some(value) = crate::reader::driver::read_attribute(owner, b"bwMode")? {
            self.set_black_white_mode(
                value
                    .parse()
                    .map_err(|()| crate::xml::xml_error("invalid drawing black-and-white mode"))?,
            );
        }
        if empty {
            return Ok(());
        }
        let end = owner.name().as_ref().to_vec();
        let mut buffer = Vec::new();
        let mut previous_rank = None;
        loop {
            match reader.read_event_into(&mut buffer)? {
                ref event @ (Event::Start(ref child) | Event::Empty(ref child)) => {
                    let empty = matches!(event, Event::Empty(_));
                    let name = scope
                        .child(child)
                        .and_then(|scope| scope.resolve(child.name().as_ref(), false))
                        .map_err(crate::xml::xml_error)?;
                    if name.namespace_uri != crate::helper::const_str::DRAWINGML_MAIN_NS {
                        if !empty {
                            crate::reader::driver::skip_element(reader, child)?;
                        }
                        continue;
                    }
                    let rank = match name.local_name.as_str() {
                        "xfrm" => 0,
                        "prstGeom" | "custGeom" => 1,
                        "noFill" | "solidFill" | "gradFill" | "blipFill" | "pattFill"
                        | "grpFill" => 2,
                        "ln" => 3,
                        "effectLst" | "effectDag" => 4,
                        "scene3d" => 5,
                        "sp3d" => 6,
                        "extLst" => 7,
                        _ => {
                            return Err(crate::xml::xml_error(
                                "unsupported drawing shape property",
                            ));
                        }
                    };
                    if previous_rank.is_some_and(|previous| rank <= previous) {
                        return Err(crate::xml::xml_error(
                            "duplicate or out-of-order drawing shape property",
                        ));
                    }
                    previous_rank = Some(rank);
                    match name.local_name.as_str() {
                        "custGeom" | "grpFill" | "effectDag" | "scene3d" | "sp3d" => {
                            self.preserved_properties.push(
                                crate::drawing::charts::OpaqueElement::read_detached(
                                    reader, child, empty, &scope,
                                )
                                .map_err(crate::xml::xml_error)?,
                            );
                        }
                        "xfrm" => {
                            let mut value = Transform2D::default();
                            value.read_with_session(reader, child, empty, Some(session))?;
                            self.set_transform2d(value);
                        }
                        "prstGeom" => self.preset_geometry.read_with_session(
                            reader,
                            child,
                            empty,
                            Some(session),
                        )?,
                        "blipFill" => {
                            let mut value = BlipFill::default();
                            value.read_with_session(
                                reader,
                                child,
                                empty,
                                drawing_relationships,
                                session,
                            )?;
                            self.set_blip_fill(value);
                        }
                        "gradFill" => {
                            let mut value = GradientFill::default();
                            value.read_with_session(reader, child, empty, Some(session))?;
                            self.set_gradient_fill(value);
                        }
                        "pattFill" => {
                            let mut value = PatternFill::default();
                            value.read_with_session(reader, child, empty, Some(session))?;
                            self.set_pattern_fill(value);
                        }
                        "ln" => {
                            let mut value = Outline::default();
                            value.read_with_session(reader, child, empty, Some(session))?;
                            self.set_outline(value);
                        }
                        "solidFill" => {
                            let mut value = SolidFill::default();
                            value.read_with_session(reader, child, empty, Some(session))?;
                            self.set_solid_fill(value);
                        }
                        "effectLst" => {
                            let element = crate::drawing::charts::OpaqueElement::read_detached(
                                reader, child, empty, &scope,
                            )
                            .map_err(crate::xml::xml_error)?;
                            let supported = crate::xml::direct_children(&element)
                                .map_err(crate::xml::xml_error)?
                                .iter()
                                .all(|child| {
                                    child.name.namespace_uri
                                        == crate::helper::const_str::DRAWINGML_MAIN_NS
                                        && child.name.local_name == "softEdge"
                                });
                            if supported {
                                let value =
                                    super::with_fragment(&element, |reader, child, empty| {
                                        let mut value = EffectList::default();
                                        value.read_with_session(
                                            reader,
                                            child,
                                            empty,
                                            Some(session),
                                        )?;
                                        Ok(value)
                                    })?;
                                self.set_effect_list(value);
                            } else {
                                // Color-bearing effects are not fully modelled.
                                // Keep their attributes, colors and transforms
                                // together.
                                self.preserved_properties.push(element);
                            }
                        }
                        "noFill" => {
                            if !empty {
                                crate::reader::driver::consume_empty_element(reader, child)?;
                            }
                            self.set_no_fill(NoFill::default());
                        }
                        "extLst" => {
                            self.set_extension_list(
                                ExtensionList::read(reader, child, empty, &scope)
                                    .map_err(crate::xml::xml_error)?,
                            );
                        }
                        _ => {
                            return Err(crate::xml::xml_error(
                                "unsupported drawing shape property",
                            ));
                        }
                    }
                }
                Event::End(child) if child.name().as_ref() == end.as_slice() => return Ok(()),
                Event::Eof => {
                    return Err(crate::xml::xml_error(
                        "unterminated drawing shape properties",
                    ));
                }
                _ => (),
            }
            buffer.clear();
        }
    }

    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        rel_list: &mut Vec<(String, String)>,
    ) -> Result<(), crate::XlsxError> {
        let fill_count = [
            self.blip_fill.is_some(),
            self.solid_fill.is_some(),
            self.gradient_fill.is_some(),
            self.pattern_fill.is_some(),
            self.no_fill.is_some(),
        ]
        .into_iter()
        .filter(|present| *present)
        .count()
            + self
                .preserved_properties
                .iter()
                .filter(|value| matches!(value.name.local_name.as_str(), "pattFill" | "grpFill"))
                .count();
        if fill_count > 1 {
            return Err(crate::xml::xml_error("conflicting shape fills"));
        }
        // xdr:spPr
        let mut attributes: crate::structs::AttrCollection = Vec::new();
        if self.black_white_mode.has_value() {
            attributes.push(("bwMode", self.black_white_mode.hash_string()).into());
        }
        write_start_tag(writer, "xdr:spPr", attributes, false);

        // a:xfrm
        if let Some(v) = &self.transform2d {
            v.write_to(writer);
        }

        // a:prstGeom
        if self
            .preserved_properties
            .iter()
            .any(|value| value.name.local_name == "custGeom")
        {
            self.write_preserved(writer, rel_list, &["custGeom"])?;
        } else if !self.preset_geometry.geometry().is_empty() {
            self.preset_geometry.write_to(writer);
        }

        // a:blipFill
        if let Some(v) = &self.blip_fill {
            v.write_to(writer, rel_list)?;
        }

        // a:solidFill
        if let Some(v) = &self.solid_fill {
            v.write_to(writer)?;
        }
        if let Some(v) = &self.gradient_fill {
            v.write_to(writer)?;
        }
        if let Some(v) = &self.pattern_fill {
            v.write_to(writer)?;
        }

        // a:noFill
        if self.no_fill.is_some() {
            NoFill::write_to(writer);
        }

        self.write_preserved(writer, rel_list, &["pattFill", "grpFill"])?;

        // a:ln
        if let Some(v) = &self.outline {
            v.write_to(writer)?;
        }

        // a:effectLst
        if let Some(v) = &self.effect_list {
            v.write_to(writer)?;
        }

        self.write_preserved(
            writer,
            rel_list,
            &["effectLst", "effectDag", "scene3d", "sp3d"],
        )?;

        if let Some(value) = &self.extension_list {
            let mut metadata = value.clone();
            if let Some(element) = metadata.element_mut() {
                super::non_visual_drawing_properties::extension_relationships_mut(
                    element,
                    &mut |_, id| {
                        super::non_visual_drawing_properties::rebind_relationship_id(id, rel_list);
                    },
                );
            }
            metadata
                .write_to(writer)
                .map_err(super::extended_chart::drawing_error)?;
        }
        write_end_tag(writer, "xdr:spPr");
        Ok(())
    }
}

impl ShapeProperties {
    fn write_preserved(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        rel_list: &[(String, String)],
        names: &[&str],
    ) -> Result<(), crate::XlsxError> {
        for value in &self.preserved_properties {
            if names.contains(&value.name.local_name.as_str()) {
                let mut value = value.clone();
                super::non_visual_drawing_properties::extension_relationships_mut(
                    &mut value,
                    &mut |_, id| {
                        super::non_visual_drawing_properties::rebind_relationship_id(id, rel_list);
                    },
                );
                value
                    .write_to(writer, &crate::drawing::charts::NamespaceScope::default())
                    .map_err(crate::xml::xml_error)?;
            }
        }
        Ok(())
    }

    pub(crate) fn visit_relationships(
        &self,
        visit: &mut dyn FnMut(super::DrawingRelationshipKind, &str),
    ) {
        if let Some(fill) = &self.blip_fill {
            fill.blip().visit_relationships(visit);
        }
        for value in &self.preserved_properties {
            super::non_visual_drawing_properties::extension_relationships(value, visit);
        }
        if let Some(value) = self
            .extension_list
            .as_ref()
            .and_then(ExtensionList::element)
        {
            super::non_visual_drawing_properties::extension_relationships(value, visit);
        }
    }

    pub(crate) fn visit_relationships_mut(
        &mut self,
        visit: &mut dyn FnMut(super::DrawingRelationshipKind, &mut String),
    ) {
        if let Some(fill) = &mut self.blip_fill {
            fill.blip_mut().visit_relationships_mut(visit);
        }
        for value in &mut self.preserved_properties {
            super::non_visual_drawing_properties::extension_relationships_mut(value, visit);
        }
        if let Some(value) = self
            .extension_list
            .as_mut()
            .and_then(ExtensionList::element_mut)
        {
            super::non_visual_drawing_properties::extension_relationships_mut(value, visit);
        }
    }
}

#[cfg(test)]
mod extension_tests {
    use super::*;

    #[test]
    fn gradient_fill_is_written_with_native_geometry_and_edits() {
        let mut value = read(
            r#"<x:spPr xmlns:x="http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><a:gradFill rotWithShape="1"><a:gsLst><a:gs pos="0"><a:srgbClr val="123456"/></a:gs><a:gs pos="100000"><a:srgbClr val="ABCDEF"/></a:gs></a:gsLst><a:lin ang="5400000" scaled="0"/></a:gradFill></x:spPr>"#,
        );
        let fill = value.gradient_fill_mut().unwrap();
        assert!(fill.rotate_with_shape());
        fill.set_rotate_with_shape(false);
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        value.write_to(&mut writer, &mut Vec::new()).unwrap();
        let output = String::from_utf8(writer.into_inner().into_inner()).unwrap();
        assert!(output.contains("<a:gradFill"));
        assert!(output.contains("rotWithShape=\"0\""));
        assert!(output.contains("123456"));
        assert!(output.contains("ABCDEF"));
        assert!(output.contains("ang=\"5400000\""));
    }

    fn read(xml: &str) -> ShapeProperties {
        let mut reader = Reader::from_reader(xml.as_bytes());
        let Event::Start(start) = reader.read_event().unwrap() else {
            panic!("root")
        };
        let mut value = ShapeProperties::default();
        let session = crate::reader::xlsx::SourceCacheSession::immediate();
        let _scope = session.push_extension_scope(
            crate::drawing::charts::NamespaceScope::default()
                .child(&start)
                .unwrap(),
        );
        value
            .read_with_session(&mut reader, &start, false, None, &session)
            .unwrap();
        value
    }
    #[test]
    fn source_hidden_fill_and_no_fill_are_preserved_without_capturing_foreign_lookalikes() {
        for no_fill in ["<d:noFill/>", "<d:noFill></d:noFill>"] {
            let xml = format!(
                "<x:spPr xmlns:x=\"http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing\" \
                 xmlns:d=\"http://schemas.openxmlformats.org/drawingml/2006/main\" xmlns:f=\"urn:foreign\" bwMode=\"auto\">\
                 <f:noFill/><f:extLst/>{no_fill}<d:extLst><d:ext uri=\"{{909E8E84-426E-40DD-AFC4-6F175D3DCCD1}}\">\
                 <a14:hiddenFill xmlns:a14=\"http://schemas.microsoft.com/office/drawing/2010/main\">\
                 <d:solidFill><d:srgbClr val=\"FFFFFF\"/></d:solidFill></a14:hiddenFill>\
                 </d:ext></d:extLst></x:spPr>"
            );
            let value = read(&xml);
            assert!(value.no_fill().is_some());
            let extension = value.extension_list().unwrap().element().unwrap();
            assert!(
                extension
                    .name
                    .is(crate::helper::const_str::DRAWINGML_MAIN_NS, "extLst")
            );
            let mut writer = Writer::new(Cursor::new(Vec::new()));
            write_start_tag(
                &mut writer,
                "xdr:wsDr",
                vec![
                    ("xmlns:xdr", crate::helper::const_str::SHEET_DRAWING_NS).into(),
                    ("xmlns:a", crate::helper::const_str::DRAWINGML_MAIN_NS).into(),
                ],
                false,
            );
            value.write_to(&mut writer, &mut Vec::new()).unwrap();
            write_end_tag(&mut writer, "xdr:wsDr");
            let bytes = writer.into_inner().into_inner();
            let output = std::str::from_utf8(&bytes).unwrap();
            assert!(output.contains("hiddenFill"));
            assert!(output.contains("FFFFFF"));
            assert!(!output.contains("<f:noFill"));
            assert!(!output.contains("<f:extLst"));
            let mut reader = Reader::from_reader(bytes.as_slice());
            let Event::Start(root) = reader.read_event().unwrap() else {
                panic!("root")
            };
            let root = crate::drawing::charts::OpaqueElement::read(
                &mut reader,
                &root,
                false,
                &crate::drawing::charts::NamespaceScope::default(),
            )
            .unwrap();
            let children = crate::xml::direct_children(&root).unwrap();
            let xml = super::super::extended_chart::canonical_xml(&children[0]).unwrap();
            let reread = read(std::str::from_utf8(&xml).unwrap());
            assert!(reread.no_fill().is_some());
            assert!(reread.extension_list().unwrap().element().is_some());
        }
    }
}
