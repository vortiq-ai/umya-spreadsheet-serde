// a:fgClr: native DrawingML color choice.
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use super::{
    HslColor,
    PresetColor,
    RgbColorModelHex,
    RgbColorModelPercentage,
    SchemeColor,
    SolidFill,
    SystemColor,
};
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Clone, Default, Debug)]
pub struct ForegroundColor {
    color: SolidFill,
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for ForegroundColor {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = ForegroundColor;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("ForegroundColor")
            }

            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> Result<Self::Value, A::Error> {
                Ok(ForegroundColor {
                    color: seq
                        .next_element()?
                        .ok_or_else(|| serde::de::Error::invalid_length(0, &self))?,
                })
            }

            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> Result<Self::Value, A::Error> {
                let mut color = None;
                let mut scheme_color = None;
                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "color" => {
                            if color.is_some() {
                                return Err(serde::de::Error::duplicate_field("color"));
                            }
                            color = Some(map.next_value()?);
                        }
                        "scheme_color" => {
                            if scheme_color.is_some() {
                                return Err(serde::de::Error::duplicate_field("scheme_color"));
                            }
                            scheme_color = Some(map.next_value()?);
                        }
                        _ => {
                            return Err(serde::de::Error::unknown_field(
                                &key,
                                &["color", "scheme_color"],
                            ));
                        }
                    }
                }
                if color.is_some() && scheme_color.is_some() {
                    return Err(serde::de::Error::custom(
                        "color and scheme_color are mutually exclusive",
                    ));
                }
                let color = match (color, scheme_color) {
                    (Some(value), None) => value,
                    (None, Some(value)) => {
                        let mut fill = SolidFill::default();
                        fill.set_scheme_color(value);
                        fill
                    }
                    _ => return Err(serde::de::Error::missing_field("color")),
                };
                Ok(ForegroundColor { color })
            }
        }
        deserializer.deserialize_struct("ForegroundColor", &["color"], Visitor)
    }
}

impl ForegroundColor {
    #[must_use]
    pub fn hsl_color(&self) -> Option<&HslColor> {
        self.color.hsl_color()
    }

    pub fn hsl_color_mut(&mut self) -> Option<&mut HslColor> {
        self.color.hsl_color_mut()
    }

    pub fn set_hsl_color(&mut self, value: HslColor) -> &mut Self {
        self.color.set_hsl_color(value);
        self
    }

    #[must_use]
    pub fn rgb_color_model_percentage(&self) -> Option<&RgbColorModelPercentage> {
        self.color.rgb_color_model_percentage()
    }

    pub fn rgb_color_model_percentage_mut(&mut self) -> Option<&mut RgbColorModelPercentage> {
        self.color.rgb_color_model_percentage_mut()
    }

    pub fn set_rgb_color_model_percentage(&mut self, value: RgbColorModelPercentage) -> &mut Self {
        self.color.set_rgb_color_model_percentage(value);
        self
    }

    #[must_use]
    pub fn scheme_color(&self) -> Option<&SchemeColor> {
        self.color.scheme_color()
    }

    #[must_use]
    pub fn get_scheme_color(&self) -> Option<&SchemeColor> {
        self.scheme_color()
    }

    pub fn scheme_color_mut(&mut self) -> Option<&mut SchemeColor> {
        self.color.scheme_color_mut()
    }

    pub fn get_scheme_color_mut(&mut self) -> Option<&mut SchemeColor> {
        self.scheme_color_mut()
    }

    pub fn set_scheme_color(&mut self, value: SchemeColor) -> &mut Self {
        self.color.set_scheme_color(value);
        self
    }

    #[must_use]
    pub fn rgb_color_model_hex(&self) -> Option<&RgbColorModelHex> {
        self.color.rgb_color_model_hex()
    }

    #[must_use]
    pub fn get_rgb_color_model_hex(&self) -> Option<&RgbColorModelHex> {
        self.rgb_color_model_hex()
    }

    pub fn rgb_color_model_hex_mut(&mut self) -> Option<&mut RgbColorModelHex> {
        self.color.rgb_color_model_hex_mut()
    }

    pub fn get_rgb_color_model_hex_mut(&mut self) -> Option<&mut RgbColorModelHex> {
        self.rgb_color_model_hex_mut()
    }

    pub fn set_rgb_color_model_hex(&mut self, value: RgbColorModelHex) -> &mut Self {
        self.color.set_rgb_color_model_hex(value);
        self
    }

    #[must_use]
    pub fn system_color(&self) -> Option<&SystemColor> {
        self.color.system_color()
    }

    #[must_use]
    pub fn get_system_color(&self) -> Option<&SystemColor> {
        self.system_color()
    }

    pub fn system_color_mut(&mut self) -> Option<&mut SystemColor> {
        self.color.system_color_mut()
    }

    pub fn get_system_color_mut(&mut self) -> Option<&mut SystemColor> {
        self.system_color_mut()
    }

    pub fn set_system_color(&mut self, value: SystemColor) -> &mut Self {
        self.color.set_system_color(value);
        self
    }

    #[must_use]
    pub fn preset_color(&self) -> Option<&PresetColor> {
        self.color.preset_color()
    }

    #[must_use]
    pub fn get_preset_color(&self) -> Option<&PresetColor> {
        self.preset_color()
    }

    pub fn preset_color_mut(&mut self) -> Option<&mut PresetColor> {
        self.color.preset_color_mut()
    }

    pub fn get_preset_color_mut(&mut self) -> Option<&mut PresetColor> {
        self.preset_color_mut()
    }

    pub fn set_preset_color(&mut self, value: PresetColor) -> &mut Self {
        self.color.set_preset_color(value);
        self
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        owner: &BytesStart,
    ) -> Result<(), crate::XlsxError> {
        self.read_with_session(reader, owner, false, None)
    }

    pub(crate) fn set_attributes_with_session<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        owner: &BytesStart,
        session: &crate::reader::xlsx::SourceCacheSession,
    ) -> Result<(), crate::XlsxError> {
        self.read_with_session(reader, owner, false, Some(session))
    }

    pub(crate) fn read_with_session<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        owner: &BytesStart,
        empty: bool,
        session: Option<&crate::reader::xlsx::SourceCacheSession>,
    ) -> Result<(), crate::XlsxError> {
        self.color
            .read_with_session(reader, owner, empty, session)?;
        if !self.color.has_color() {
            return Err(crate::xml::xml_error(
                "DrawingML color container requires exactly one color",
            ));
        }
        Ok(())
    }

    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
    ) -> Result<(), crate::XlsxError> {
        if !self.color.has_color() {
            return Err(crate::xml::xml_error(
                "DrawingML color container requires exactly one color",
            ));
        }
        self.color.write_to_as(writer, "a:fgClr")
    }
}
