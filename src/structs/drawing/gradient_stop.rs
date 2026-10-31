// a:gs
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::{
        BytesStart,
        Event,
    },
};

use super::{
    HslColor,
    PresetColor,
    RgbColorModelHex,
    RgbColorModelPercentage,
    SchemeColor,
    SystemColor,
};
use crate::writer::driver::{
    write_end_tag,
    write_start_tag,
};

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Clone, Debug)]
pub struct GradientStop {
    #[cfg_attr(feature = "serde", serde(default))]
    hsl_color:                  Option<Box<HslColor>>,
    #[cfg_attr(feature = "serde", serde(default))]
    rgb_color_model_percentage: Option<Box<RgbColorModelPercentage>>,
    pub(super) position:        Box<str>,
    scheme_color:               Option<Box<SchemeColor>>,
    rgb_color_model_hex:        Option<Box<RgbColorModelHex>>,
    #[cfg_attr(feature = "serde", serde(default))]
    preset_color:               Option<Box<PresetColor>>,
    #[cfg_attr(feature = "serde", serde(default))]
    system_color:               Option<Box<SystemColor>>,
}

impl Default for GradientStop {
    fn default() -> Self {
        Self {
            hsl_color:                  None,
            rgb_color_model_percentage: None,
            position:                   "0".into(),
            scheme_color:               None,
            rgb_color_model_hex:        None,
            preset_color:               None,
            system_color:               None,
        }
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for GradientStop {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        const FIELDS: &[&str] = &[
            "hsl_color",
            "rgb_color_model_percentage",
            "position",
            "scheme_color",
            "rgb_color_model_hex",
            "preset_color",
            "system_color",
        ];
        struct MapPosition;
        impl<'de> serde::de::DeserializeSeed<'de> for MapPosition {
            type Value = Box<str>;

            fn deserialize<D: serde::Deserializer<'de>>(
                self,
                deserializer: D,
            ) -> Result<Self::Value, D::Error> {
                struct PositionVisitor;
                impl serde::de::Visitor<'_> for PositionVisitor {
                    type Value = Box<str>;

                    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                        f.write_str("a gradient percentage string or legacy integer")
                    }

                    fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Self::Value, E> {
                        super::color_transform::validate_positive_fixed_percentage(value)
                            .map_err(E::custom)?;
                        Ok(value.into())
                    }

                    fn visit_i64<E: serde::de::Error>(self, value: i64) -> Result<Self::Value, E> {
                        self.visit_str(&value.to_string())
                    }

                    fn visit_u64<E: serde::de::Error>(self, value: u64) -> Result<Self::Value, E> {
                        self.visit_str(&value.to_string())
                    }
                }
                // Only map values use self-describing token dispatch. The
                // sequence visitor below reads the current string field.
                deserializer.deserialize_any(PositionVisitor)
            }
        }
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = GradientStop;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("GradientStop")
            }

            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> Result<Self::Value, A::Error> {
                let hsl_color = seq
                    .next_element()?
                    .ok_or_else(|| serde::de::Error::invalid_length(0, &self))?;
                let rgb_color_model_percentage = seq
                    .next_element()?
                    .ok_or_else(|| serde::de::Error::invalid_length(1, &self))?;
                let position: Box<str> = seq
                    .next_element()?
                    .ok_or_else(|| serde::de::Error::invalid_length(2, &self))?;
                super::color_transform::validate_positive_fixed_percentage(&position)
                    .map_err(serde::de::Error::custom)?;
                Ok(GradientStop {
                    hsl_color,
                    rgb_color_model_percentage,
                    position,
                    scheme_color: seq
                        .next_element()?
                        .ok_or_else(|| serde::de::Error::invalid_length(3, &self))?,
                    rgb_color_model_hex: seq
                        .next_element()?
                        .ok_or_else(|| serde::de::Error::invalid_length(4, &self))?,
                    preset_color: seq
                        .next_element()?
                        .ok_or_else(|| serde::de::Error::invalid_length(5, &self))?,
                    system_color: seq
                        .next_element()?
                        .ok_or_else(|| serde::de::Error::invalid_length(6, &self))?,
                })
            }

            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> Result<Self::Value, A::Error> {
                let mut hsl_color = None;
                let mut rgb_color_model_percentage = None;
                let mut position = None;
                let mut scheme_color = None;
                let mut rgb_color_model_hex = None;
                let mut preset_color = None;
                let mut system_color = None;
                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "hsl_color" => {
                            if hsl_color.is_some() {
                                return Err(serde::de::Error::duplicate_field("hsl_color"));
                            }
                            hsl_color = Some(map.next_value()?);
                        }
                        "rgb_color_model_percentage" => {
                            if rgb_color_model_percentage.is_some() {
                                return Err(serde::de::Error::duplicate_field(
                                    "rgb_color_model_percentage",
                                ));
                            }
                            rgb_color_model_percentage = Some(map.next_value()?);
                        }
                        "position" => {
                            if position.is_some() {
                                return Err(serde::de::Error::duplicate_field("position"));
                            }
                            position = Some(map.next_value_seed(MapPosition)?);
                        }
                        "scheme_color" => {
                            if scheme_color.is_some() {
                                return Err(serde::de::Error::duplicate_field("scheme_color"));
                            }
                            scheme_color = Some(map.next_value()?);
                        }
                        "rgb_color_model_hex" => {
                            if rgb_color_model_hex.is_some() {
                                return Err(serde::de::Error::duplicate_field(
                                    "rgb_color_model_hex",
                                ));
                            }
                            rgb_color_model_hex = Some(map.next_value()?);
                        }
                        "preset_color" => {
                            if preset_color.is_some() {
                                return Err(serde::de::Error::duplicate_field("preset_color"));
                            }
                            preset_color = Some(map.next_value()?);
                        }
                        "system_color" => {
                            if system_color.is_some() {
                                return Err(serde::de::Error::duplicate_field("system_color"));
                            }
                            system_color = Some(map.next_value()?);
                        }
                        _ => return Err(serde::de::Error::unknown_field(&key, FIELDS)),
                    }
                }
                Ok(GradientStop {
                    hsl_color:                  hsl_color.unwrap_or(None),
                    rgb_color_model_percentage: rgb_color_model_percentage.unwrap_or(None),
                    position:                   position
                        .ok_or_else(|| serde::de::Error::missing_field("position"))?,
                    scheme_color:               scheme_color.unwrap_or(None),
                    rgb_color_model_hex:        rgb_color_model_hex.unwrap_or(None),
                    preset_color:               preset_color.unwrap_or(None),
                    system_color:               system_color.unwrap_or(None),
                })
            }
        }
        deserializer.deserialize_struct("GradientStop", FIELDS, Visitor)
    }
}

impl GradientStop {
    #[must_use]
    pub fn hsl_color(&self) -> Option<&HslColor> {
        self.hsl_color.as_deref()
    }

    pub fn hsl_color_mut(&mut self) -> Option<&mut HslColor> {
        self.hsl_color.as_deref_mut()
    }

    pub fn set_hsl_color(&mut self, value: HslColor) -> &mut Self {
        self.scheme_color = None;
        self.rgb_color_model_hex = None;
        self.preset_color = None;
        self.system_color = None;
        self.rgb_color_model_percentage = None;
        self.hsl_color = Some(Box::new(value));
        self
    }

    #[must_use]
    pub fn rgb_color_model_percentage(&self) -> Option<&RgbColorModelPercentage> {
        self.rgb_color_model_percentage.as_deref()
    }

    pub fn rgb_color_model_percentage_mut(&mut self) -> Option<&mut RgbColorModelPercentage> {
        self.rgb_color_model_percentage.as_deref_mut()
    }

    pub fn set_rgb_color_model_percentage(&mut self, value: RgbColorModelPercentage) -> &mut Self {
        self.scheme_color = None;
        self.rgb_color_model_hex = None;
        self.preset_color = None;
        self.system_color = None;
        self.hsl_color = None;
        self.rgb_color_model_percentage = Some(Box::new(value));
        self
    }

    #[inline]
    #[must_use]
    pub fn position(&self) -> &str {
        &self.position
    }

    #[inline]
    pub fn set_position(&mut self, value: &str) -> Result<&mut Self, crate::XlsxError> {
        let value = crate::xml::trim_xml_whitespace(value);
        super::color_transform::validate_positive_fixed_percentage(value)?;
        self.position = value.into();
        Ok(self)
    }

    #[inline]
    #[must_use]
    pub fn scheme_color(&self) -> Option<&SchemeColor> {
        self.scheme_color.as_deref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use scheme_color()")]
    pub fn get_scheme_color(&self) -> Option<&SchemeColor> {
        self.scheme_color()
    }

    #[inline]
    pub fn scheme_color_mut(&mut self) -> Option<&mut SchemeColor> {
        self.scheme_color.as_deref_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use scheme_color_mut()")]
    pub fn get_scheme_color_mut(&mut self) -> Option<&mut SchemeColor> {
        self.scheme_color_mut()
    }

    #[inline]
    pub fn set_scheme_color(&mut self, value: SchemeColor) -> &mut GradientStop {
        self.system_color = None;
        self.preset_color = None;
        self.rgb_color_model_hex = None;
        self.hsl_color = None;
        self.rgb_color_model_percentage = None;
        self.scheme_color = Some(Box::new(value));
        self
    }

    #[inline]
    #[must_use]
    pub fn rgb_color_model_hex(&self) -> Option<&RgbColorModelHex> {
        self.rgb_color_model_hex.as_deref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use rgb_color_model_hex()")]
    pub fn get_rgb_color_model_hex(&self) -> Option<&RgbColorModelHex> {
        self.rgb_color_model_hex()
    }

    #[inline]
    pub fn rgb_color_model_hex_mut(&mut self) -> Option<&mut RgbColorModelHex> {
        self.rgb_color_model_hex.as_deref_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use rgb_color_model_hex_mut()")]
    pub fn get_rgb_color_model_hex_mut(&mut self) -> Option<&mut RgbColorModelHex> {
        self.rgb_color_model_hex_mut()
    }

    #[inline]
    pub fn set_rgb_color_model_hex(&mut self, value: RgbColorModelHex) -> &mut GradientStop {
        self.system_color = None;
        self.preset_color = None;
        self.scheme_color = None;
        self.hsl_color = None;
        self.rgb_color_model_percentage = None;
        self.rgb_color_model_hex = Some(Box::new(value));
        self
    }

    #[must_use]
    pub fn preset_color(&self) -> Option<&PresetColor> {
        self.preset_color.as_deref()
    }

    pub fn preset_color_mut(&mut self) -> Option<&mut PresetColor> {
        self.preset_color.as_deref_mut()
    }

    pub fn set_preset_color(&mut self, value: PresetColor) -> &mut Self {
        self.system_color = None;
        self.scheme_color = None;
        self.rgb_color_model_hex = None;
        self.hsl_color = None;
        self.rgb_color_model_percentage = None;
        self.preset_color = Some(Box::new(value));
        self
    }

    #[must_use]
    pub fn system_color(&self) -> Option<&SystemColor> {
        self.system_color.as_deref()
    }

    pub fn system_color_mut(&mut self) -> Option<&mut SystemColor> {
        self.system_color.as_deref_mut()
    }

    pub fn set_system_color(&mut self, value: SystemColor) -> &mut Self {
        self.scheme_color = None;
        self.rgb_color_model_hex = None;
        self.preset_color = None;
        self.hsl_color = None;
        self.rgb_color_model_percentage = None;
        self.system_color = Some(Box::new(value));
        self
    }

    pub(crate) fn read_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
        empty: bool,
    ) -> Result<(), crate::XlsxError> {
        self.read_with_session(reader, e, empty, None)
    }

    pub(crate) fn read_attributes_with_session<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
        empty: bool,
        session: &crate::reader::xlsx::SourceCacheSession,
    ) -> Result<(), crate::XlsxError> {
        self.read_with_session(reader, e, empty, Some(session))
    }

    pub(crate) fn read_with_session<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        owner: &BytesStart<'_>,
        empty: bool,
        session: Option<&crate::reader::xlsx::SourceCacheSession>,
    ) -> Result<(), crate::XlsxError> {
        let _scope_guard = session
            .map(|session| session.enter_child_scope(owner))
            .transpose()?
            .flatten();
        let value = crate::reader::driver::require_attribute(owner, b"pos")?;
        self.set_position(&value)?;
        if empty {
            return Err(crate::xml::xml_error(
                "gradient stop requires exactly one color",
            ));
        }
        let end_name = owner.name().as_ref().to_vec();
        let mut color_seen = false;
        let mut buffer = Vec::new();
        loop {
            let event = reader.read_event_into(&mut buffer)?;
            match event {
                ref event @ (Event::Start(ref child) | Event::Empty(ref child)) => {
                    let child_empty = matches!(event, Event::Empty(_));
                    let native = match session {
                        Some(session) => session.accepts_child_namespace(
                            child,
                            crate::helper::const_str::DRAWINGML_MAIN_NS,
                        )?,
                        None => true,
                    };
                    if !native {
                        if !child_empty {
                            crate::reader::driver::skip_element(reader, child)?;
                        }
                        continue;
                    }
                    if !matches!(
                        crate::reader::driver::local_name(child.name().into_inner()),
                        b"prstClr"
                            | b"schemeClr"
                            | b"srgbClr"
                            | b"sysClr"
                            | b"hslClr"
                            | b"scrgbClr"
                    ) {
                        if !child_empty {
                            crate::reader::driver::skip_element(reader, child)?;
                        }
                        continue;
                    }
                    if color_seen {
                        return Err(crate::xml::xml_error(
                            "duplicate or conflicting gradient color",
                        ));
                    }
                    color_seen = true;
                    match crate::reader::driver::local_name(child.name().into_inner()) {
                        b"sysClr" => {
                            let mut value = SystemColor::default();
                            value.read_with_session(reader, child, child_empty, session)?;
                            self.set_system_color(value);
                        }
                        b"prstClr" => {
                            let mut value = PresetColor::default();
                            value.read_with_session(reader, child, child_empty, session)?;
                            self.set_preset_color(value);
                        }
                        b"schemeClr" => {
                            let mut value = SchemeColor::default();
                            value.read_with_session(reader, child, child_empty, session)?;
                            self.set_scheme_color(value);
                        }
                        b"hslClr" => {
                            let mut value = HslColor::default();
                            value.read_with_session(reader, child, child_empty, session)?;
                            self.set_hsl_color(value);
                        }
                        b"scrgbClr" => {
                            let mut value = RgbColorModelPercentage::default();
                            value.read_with_session(reader, child, child_empty, session)?;
                            self.set_rgb_color_model_percentage(value);
                        }
                        b"srgbClr" => {
                            let mut value = RgbColorModelHex::default();
                            value.read_with_session(reader, child, child_empty, session)?;
                            self.set_rgb_color_model_hex(value);
                        }
                        _ => {
                            if !child_empty {
                                crate::reader::driver::skip_element(reader, child)?;
                            }
                        }
                    }
                }
                Event::End(ref end) if end.name().as_ref() == end_name.as_slice() => {
                    return if color_seen {
                        Ok(())
                    } else {
                        Err(crate::xml::xml_error(
                            "gradient stop requires exactly one color",
                        ))
                    };
                }
                Event::Text(ref text)
                    if text
                        .iter()
                        .all(|byte| matches!(byte, b' ' | b'\t' | b'\r' | b'\n')) => {}
                Event::Comment(_) | Event::PI(_) => (),
                Event::Eof => return Err(crate::xml::xml_error("unterminated DrawingML element")),
                _ => return Err(crate::xml::xml_error("unexpected DrawingML content")),
            }
            buffer.clear();
        }
    }

    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
    ) -> Result<(), crate::XlsxError> {
        super::color_transform::validate_positive_fixed_percentage(self.position())?;
        if [
            self.hsl_color.is_some(),
            self.rgb_color_model_percentage.is_some(),
            self.scheme_color.is_some(),
            self.rgb_color_model_hex.is_some(),
            self.preset_color.is_some(),
            self.system_color.is_some(),
        ]
        .into_iter()
        .filter(|present| *present)
        .count()
            != 1
        {
            return Err(crate::xml::xml_error(
                "gradient stop requires exactly one color",
            ));
        }

        // a:gs
        write_start_tag(writer, "a:gs", vec![("pos", self.position()).into()], false);

        // a:schemeClr
        if let Some(v) = &self.scheme_color {
            v.write_to(writer)?;
        }

        if let Some(color) = &self.hsl_color {
            color.write_to(writer)?;
        }
        if let Some(color) = &self.rgb_color_model_percentage {
            color.write_to(writer)?;
        }
        // a:srgbClr
        if let Some(v) = &self.rgb_color_model_hex {
            v.write_to(writer)?;
        }

        if let Some(color) = &self.system_color {
            color.write_to(writer)?;
        }
        if let Some(color) = &self.preset_color {
            color.write_to(writer)?;
        }
        write_end_tag(writer, "a:gs");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_native_positions_and_missing_colors_fail_before_output() {
        let mut stop = GradientStop::default();
        for invalid in ["-1", "100001", "bad"] {
            stop.position = invalid.into();
            let mut writer = Writer::new(Cursor::new(Vec::new()));
            assert!(stop.write_to(&mut writer).is_err());
            assert!(writer.into_inner().into_inner().is_empty());
        }
        stop.position = "0".into();
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        assert!(stop.write_to(&mut writer).is_err());
        assert!(writer.into_inner().into_inner().is_empty());
    }
}
