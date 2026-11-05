// a:outerShdw
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
pub struct OuterShadow {
    blur_radius:                StringValue,
    alignment:                  StringValue,
    horizontal_ratio:           StringValue,
    vertical_ratio:             StringValue,
    direction:                  StringValue,
    distance:                   StringValue,
    rotate_with_shape:          StringValue,
    preset_color:               Option<Box<PresetColor>>,
    scheme_color:               Option<Box<SchemeColor>>,
    rgb_color_model_hex:        Option<Box<RgbColorModelHex>>,
    #[cfg_attr(feature = "serde", serde(default))]
    hsl_color:                  Option<Box<HslColor>>,
    #[cfg_attr(feature = "serde", serde(default))]
    rgb_color_model_percentage: Option<Box<RgbColorModelPercentage>>,
}

impl OuterShadow {
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
        self.hsl_color = None;
        self.rgb_color_model_percentage = Some(Box::new(value));
        self
    }

    #[inline]
    #[must_use]
    pub fn blur_radius(&self) -> Option<&str> {
        self.blur_radius.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use blur_radius()")]
    pub fn get_blur_radius(&self) -> Option<&str> {
        self.blur_radius()
    }

    #[inline]
    pub fn set_blur_radius<S: Into<String>>(&mut self, value: S) -> &mut Self {
        self.blur_radius.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn horizontal_ratio(&self) -> Option<&str> {
        self.horizontal_ratio.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use horizontal_ratio()")]
    pub fn get_horizontal_ratio(&self) -> Option<&str> {
        self.horizontal_ratio()
    }

    #[inline]
    pub fn set_horizontal_ratio<S: Into<String>>(&mut self, value: S) -> &mut Self {
        self.horizontal_ratio.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn vertical_ratio(&self) -> Option<&str> {
        self.vertical_ratio.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use vertical_ratio()")]
    pub fn get_vertical_ratio(&self) -> Option<&str> {
        self.vertical_ratio()
    }

    #[inline]
    pub fn set_vertical_ratio<S: Into<String>>(&mut self, value: S) -> &mut Self {
        self.vertical_ratio.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn alignment(&self) -> Option<&str> {
        self.alignment.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use alignment()")]
    pub fn get_alignment(&self) -> Option<&str> {
        self.alignment()
    }

    #[inline]
    pub fn set_alignment<S: Into<String>>(&mut self, value: S) -> &mut Self {
        self.alignment.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn direction(&self) -> Option<&str> {
        self.direction.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use direction()")]
    pub fn get_direction(&self) -> Option<&str> {
        self.direction()
    }

    #[inline]
    pub fn set_direction<S: Into<String>>(&mut self, value: S) -> &mut Self {
        self.direction.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn distance(&self) -> Option<&str> {
        self.distance.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use distance()")]
    pub fn get_distance(&self) -> Option<&str> {
        self.distance()
    }

    #[inline]
    pub fn set_distance<S: Into<String>>(&mut self, value: S) -> &mut Self {
        self.distance.set_value(value);
        self
    }

    #[must_use]
    pub fn rotate_with_shape(&self) -> Option<&str> {
        self.rotate_with_shape.value()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use rotate_with_shape()")]
    pub fn get_rotate_with_shape(&self) -> Option<&str> {
        self.rotate_with_shape()
    }

    #[inline]
    pub fn set_rotate_with_shape<S: Into<String>>(&mut self, value: S) -> &mut Self {
        self.rotate_with_shape.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn preset_color(&self) -> Option<&PresetColor> {
        self.preset_color.as_deref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use preset_color()")]
    pub fn get_preset_color(&self) -> Option<&PresetColor> {
        self.preset_color()
    }

    #[inline]
    pub fn preset_color_mut(&mut self) -> Option<&mut PresetColor> {
        self.preset_color.as_deref_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use preset_color_mut()")]
    pub fn get_preset_color_mut(&mut self) -> Option<&mut PresetColor> {
        self.preset_color_mut()
    }

    #[inline]
    pub fn set_preset_color(&mut self, value: PresetColor) -> &mut Self {
        self.hsl_color = None;
        self.rgb_color_model_percentage = None;
        self.preset_color = Some(Box::new(value));
        self
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
    pub fn set_scheme_color(&mut self, value: SchemeColor) -> &mut Self {
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
    pub fn set_rgb_color_model_hex(&mut self, value: RgbColorModelHex) -> &mut Self {
        self.hsl_color = None;
        self.rgb_color_model_percentage = None;
        self.rgb_color_model_hex = Some(Box::new(value));
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
        if let Some(value) = crate::reader::driver::read_attribute(owner, b"blurRad")? {
            let parsed = crate::xml::trim_xml_whitespace(&value)
                .parse::<i64>()
                .map_err(crate::xml::xml_error)?;
            if parsed < 0 {
                return Err(crate::xml::xml_error("negative DrawingML blurRad"));
            }
            self.set_blur_radius(value);
        }
        if let Some(value) = crate::reader::driver::read_attribute(owner, b"dist")? {
            let parsed = crate::xml::trim_xml_whitespace(&value)
                .parse::<i64>()
                .map_err(crate::xml::xml_error)?;
            if parsed < 0 {
                return Err(crate::xml::xml_error("negative DrawingML dist"));
            }
            self.set_distance(value);
        }
        if let Some(value) = crate::reader::driver::read_attribute(owner, b"dir")? {
            let _validated = crate::xml::parse_i32(&value).map_err(crate::xml::xml_error)?;
            self.set_direction(value);
        }
        if let Some(value) = crate::reader::driver::read_attribute(owner, b"sx")? {
            let _validated = crate::xml::parse_i32(&value).map_err(crate::xml::xml_error)?;
            self.set_horizontal_ratio(value);
        }
        if let Some(value) = crate::reader::driver::read_attribute(owner, b"sy")? {
            let _validated = crate::xml::parse_i32(&value).map_err(crate::xml::xml_error)?;
            self.set_vertical_ratio(value);
        }
        if let Some(value) = crate::reader::driver::read_attribute(owner, b"algn")? {
            self.set_alignment(value);
        }
        if let Some(value) = crate::reader::driver::read_attribute(owner, b"rotWithShape")? {
            let _validated = crate::xml::parse_bool(&value).map_err(crate::xml::xml_error)?;
            self.set_rotate_with_shape(value);
        }
        if empty {
            return Ok(());
        }
        let end_name = owner.name().as_ref().to_vec();
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
                    match crate::reader::driver::local_name(child.name().into_inner()) {
                        b"prstClr" => {
                            let mut value = PresetColor::default();
                            value.read_with_session(reader, child, child_empty, session)?;
                            self.set_preset_color(value);
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
                        b"schemeClr" => {
                            let mut value = SchemeColor::default();
                            value.read_with_session(reader, child, child_empty, session)?;
                            self.set_scheme_color(value);
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
        // a:outerShdw
        let mut attributes: crate::structs::AttrCollection = Vec::new();
        if let Some(v) = self.blur_radius.value() {
            attributes.push(("blurRad", v).into());
        }
        if let Some(v) = self.distance.value() {
            attributes.push(("dist", v).into());
        }
        if let Some(v) = self.direction.value() {
            attributes.push(("dir", v).into());
        }
        if let Some(v) = self.horizontal_ratio.value() {
            attributes.push(("sx", v).into());
        }
        if let Some(v) = self.vertical_ratio.value() {
            attributes.push(("sy", v).into());
        }
        if let Some(v) = self.alignment.value() {
            attributes.push(("algn", v).into());
        }
        if let Some(v) = self.rotate_with_shape.value() {
            attributes.push(("rotWithShape", v).into());
        }
        write_start_tag(writer, "a:outerShdw", attributes, false);

        // a:prstClr
        if let Some(v) = &self.preset_color {
            v.write_to(writer)?;
        }

        if let Some(color) = &self.hsl_color {
            color.write_to(writer)?;
        }
        if let Some(color) = &self.rgb_color_model_percentage {
            color.write_to(writer)?;
        }
        // a:schemeClr
        if let Some(v) = &self.scheme_color {
            v.write_to(writer)?;
        }

        // a:srgbClr
        if let Some(v) = &self.rgb_color_model_hex {
            v.write_to(writer)?;
        }

        write_end_tag(writer, "a:outerShdw");
        Ok(())
    }
}
