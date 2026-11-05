// a:pattFill
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
    BackgroundColor,
    ForegroundColor,
};
use crate::writer::driver::{
    write_end_tag,
    write_start_tag,
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct PatternFill {
    preset:           Box<str>,
    foreground_color: Option<ForegroundColor>,
    background_color: Option<BackgroundColor>,
}

impl Default for PatternFill {
    #[inline]
    fn default() -> Self {
        Self {
            preset:           "pct5".into(),
            foreground_color: None,
            background_color: None,
        }
    }
}

impl PatternFill {
    #[inline]
    #[must_use]
    pub fn preset(&self) -> &str {
        &self.preset
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use preset()")]
    pub fn get_preset(&self) -> &str {
        self.preset()
    }

    #[inline]
    pub fn set_preset(&mut self, value: String) -> &mut Self {
        self.preset = value.into_boxed_str();
        self
    }

    #[inline]
    #[must_use]
    pub fn foreground_color(&self) -> Option<&ForegroundColor> {
        self.foreground_color.as_ref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use foreground_color()")]
    pub fn get_foreground_color(&self) -> Option<&ForegroundColor> {
        self.foreground_color()
    }

    #[inline]
    pub fn foreground_color_mut(&mut self) -> Option<&mut ForegroundColor> {
        self.foreground_color.as_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use foreground_color_mut()")]
    pub fn get_foreground_color_mut(&mut self) -> Option<&mut ForegroundColor> {
        self.foreground_color_mut()
    }

    #[inline]
    pub fn set_foreground_color(&mut self, value: ForegroundColor) -> &mut Self {
        self.foreground_color = Some(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn background_color(&self) -> Option<&BackgroundColor> {
        self.background_color.as_ref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use background_color()")]
    pub fn get_background_color(&self) -> Option<&BackgroundColor> {
        self.background_color()
    }

    #[inline]
    pub fn background_color_mut(&mut self) -> Option<&mut BackgroundColor> {
        self.background_color.as_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use background_color_mut()")]
    pub fn get_background_color_mut(&mut self) -> Option<&mut BackgroundColor> {
        self.background_color_mut()
    }

    #[inline]
    pub fn set_background_color(&mut self, value: BackgroundColor) -> &mut Self {
        self.background_color = Some(value);
        self
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
    ) -> Result<(), crate::XlsxError> {
        self.read_with_session(reader, e, false, None)
    }

    pub(crate) fn set_attributes_with_session<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
        session: &crate::reader::xlsx::SourceCacheSession,
    ) -> Result<(), crate::XlsxError> {
        self.read_with_session(reader, e, false, Some(session))
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
        self.set_preset(crate::reader::driver::read_attribute(owner, b"prst")?.unwrap_or_default());
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
                        b"fgClr" => {
                            if self.foreground_color.is_some() {
                                return Err(crate::xml::xml_error(
                                    "duplicate DrawingML pattern color",
                                ));
                            }
                            let mut value = ForegroundColor::default();
                            value.read_with_session(reader, child, child_empty, session)?;
                            self.set_foreground_color(value);
                        }
                        b"bgClr" => {
                            if self.background_color.is_some() {
                                return Err(crate::xml::xml_error(
                                    "duplicate DrawingML pattern color",
                                ));
                            }
                            let mut value = BackgroundColor::default();
                            value.read_with_session(reader, child, child_empty, session)?;
                            self.set_background_color(value);
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
        // a:pattFill
        write_start_tag(
            writer,
            "a:pattFill",
            if self.preset.is_empty() {
                vec![]
            } else {
                vec![("prst", &self.preset).into()]
            },
            false,
        );

        // a:fgClr
        if let Some(color) = &self.foreground_color {
            color.write_to(writer)?;
        }

        // a:bgClr
        if let Some(color) = &self.background_color {
            color.write_to(writer)?;
        }

        write_end_tag(writer, "a:pattFill");
        Ok(())
    }
}
