// a:solidFill
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
    RgbColorModelPercentage,
    rgb_color_model_hex::RgbColorModelHex,
    scheme_color::SchemeColor,
};
use crate::{
    drawing::SystemColor,
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct SolidFill {
    #[cfg_attr(feature = "serde", serde(default))]
    hsl_color:                  Option<Box<HslColor>>,
    #[cfg_attr(feature = "serde", serde(default))]
    rgb_color_model_percentage: Option<Box<RgbColorModelPercentage>>,
    scheme_color:               Option<Box<SchemeColor>>,
    rgb_color_model_hex:        Option<Box<RgbColorModelHex>>,
    #[cfg_attr(feature = "serde", serde(default))]
    preset_color:               Option<Box<PresetColor>>,
    system_color:               Option<Box<SystemColor>>,
}

impl SolidFill {
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

    pub(crate) fn has_color(&self) -> bool {
        self.scheme_color.is_some()
            || self.rgb_color_model_hex.is_some()
            || self.preset_color.is_some()
            || self.system_color.is_some()
            || self.hsl_color.is_some()
            || self.rgb_color_model_percentage.is_some()
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
    pub fn set_scheme_color(&mut self, value: SchemeColor) {
        self.preset_color = None;
        self.rgb_color_model_hex = None;
        self.system_color = None;
        self.hsl_color = None;
        self.rgb_color_model_percentage = None;
        self.scheme_color = Some(Box::new(value));
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
    pub fn set_rgb_color_model_hex(&mut self, value: RgbColorModelHex) {
        self.preset_color = None;
        self.scheme_color = None;
        self.system_color = None;
        self.hsl_color = None;
        self.rgb_color_model_percentage = None;
        self.rgb_color_model_hex = Some(Box::new(value));
    }

    #[inline]
    #[must_use]
    pub fn system_color(&self) -> Option<&SystemColor> {
        self.system_color.as_deref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use system_color()")]
    pub fn get_system_color(&self) -> Option<&SystemColor> {
        self.system_color()
    }

    #[inline]
    pub fn system_color_mut(&mut self) -> Option<&mut SystemColor> {
        self.system_color.as_deref_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use system_color_mut()")]
    pub fn get_system_color_mut(&mut self) -> Option<&mut SystemColor> {
        self.system_color_mut()
    }

    #[inline]
    pub fn set_system_color(&mut self, value: SystemColor) {
        self.preset_color = None;
        self.scheme_color = None;
        self.rgb_color_model_hex = None;
        self.hsl_color = None;
        self.rgb_color_model_percentage = None;
        self.system_color = Some(Box::new(value));
    }

    #[must_use]
    pub fn preset_color(&self) -> Option<&PresetColor> {
        self.preset_color.as_deref()
    }

    pub fn preset_color_mut(&mut self) -> Option<&mut PresetColor> {
        self.preset_color.as_deref_mut()
    }

    pub fn set_preset_color(&mut self, value: PresetColor) -> &mut Self {
        self.scheme_color = None;
        self.rgb_color_model_hex = None;
        self.system_color = None;
        self.hsl_color = None;
        self.rgb_color_model_percentage = None;
        self.preset_color = Some(Box::new(value));
        self
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
    ) -> Result<(), crate::XlsxError> {
        self.read_with_session(reader, e, false, None)
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

        if empty {
            return Ok(());
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
                        return Err(crate::xml::xml_error("duplicate or conflicting fill color"));
                    }
                    color_seen = true;
                    match crate::reader::driver::local_name(child.name().into_inner()) {
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
                        b"sysClr" => {
                            let mut value = SystemColor::default();
                            value.read_with_session(reader, child, child_empty, session)?;
                            self.set_system_color(value);
                        }
                        _ => {
                            if !child_empty {
                                crate::reader::driver::skip_element(reader, child)?;
                            }
                        }
                    }
                }
                Event::End(ref end) if end.name().as_ref() == end_name.as_slice() => return Ok(()),
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
        self.write_to_as(writer, "a:solidFill")
    }

    pub(crate) fn write_to_as(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        name: &str,
    ) -> Result<(), crate::XlsxError> {
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
            > 1
        {
            return Err(crate::xml::xml_error("conflicting native paint colors"));
        }

        // a:solidFill
        write_start_tag(writer, name, vec![], false);
        if let Some(color) = &self.scheme_color {
            color.write_to(writer)?;
        }

        if let Some(color) = &self.hsl_color {
            color.write_to(writer)?;
        }
        if let Some(color) = &self.rgb_color_model_percentage {
            color.write_to(writer)?;
        }
        // a:srgbClr
        if let Some(hex) = &self.rgb_color_model_hex {
            hex.write_to(writer)?;
        }

        // a:sysClr
        if let Some(v) = &self.system_color {
            v.write_to(writer)?;
        }

        if let Some(color) = &self.preset_color {
            color.write_to(writer)?;
        }
        write_end_tag(writer, name);
        Ok(())
    }
}
