// a:glow
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
    super::super::Int64Value,
    HslColor,
    RgbColorModelPercentage,
    SchemeColor,
};
use crate::writer::driver::{
    write_end_tag,
    write_start_tag,
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct Glow {    radius:                     Int64Value,
    scheme_color:               Option<Box<SchemeColor>>,

    #[cfg_attr(feature = "serde", serde(default))]
    hsl_color:                  Option<Box<HslColor>>,
    #[cfg_attr(feature = "serde", serde(default))]
    rgb_color_model_percentage: Option<Box<RgbColorModelPercentage>>,
}

impl Glow {
    #[must_use]
    pub fn hsl_color(&self) -> Option<&HslColor> {
        self.hsl_color.as_deref()
    }

    pub fn hsl_color_mut(&mut self) -> Option<&mut HslColor> {
        self.hsl_color.as_deref_mut()
    }

    pub fn set_hsl_color(&mut self, value: HslColor) -> &mut Self {
        self.scheme_color = None;
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
        self.hsl_color = None;
        self.rgb_color_model_percentage = Some(Box::new(value));
        self
    }

    #[inline]
    #[must_use]
    pub fn radius(&self) -> i64 {
        self.radius.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use radius()")]
    pub fn get_radius(&self) -> i64 {
        self.radius()
    }

    #[inline]
    pub fn set_radius(&mut self, value: i64) -> &mut Glow {
        self.radius.set_value(value);
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
    pub fn set_scheme_color(&mut self, value: SchemeColor) {
        self.hsl_color = None;
        self.rgb_color_model_percentage = None;
        self.scheme_color = Some(Box::new(value));
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
        if let Some(value) = crate::reader::driver::read_attribute(owner, b"rad")? {
            let parsed = crate::xml::trim_xml_whitespace(&value)
                .parse::<i64>()
                .map_err(crate::xml::xml_error)?;
            if !(0..=2_147_483_647).contains(&parsed) {
                return Err(crate::xml::xml_error("DrawingML rad out of range"));
            }
            self.radius.set_value(parsed);
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
        if self.radius.has_value() && !(0..=2_147_483_647).contains(&self.radius()) {
            return Err(crate::xml::xml_error("DrawingML rad out of range"));
        }
        // a:glow
        write_start_tag(
            writer,
            "a:glow",
            if self.radius.has_value() {
                vec![("rad", self.radius.value_string()).into()]
            } else {
                vec![]
            },
            false,
        );

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

        write_end_tag(writer, "a:glow");
        Ok(())
    }
}
