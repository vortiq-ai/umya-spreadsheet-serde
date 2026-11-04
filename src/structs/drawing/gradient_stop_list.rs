// a:gsLst
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::{
        BytesStart,
        Event,
    },
};

use super::GradientStop;
use crate::writer::driver::{
    write_end_tag,
    write_start_tag,
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct GradientStopList {
    gradient_stop: Vec<GradientStop>,
}

impl GradientStopList {
    #[inline]
    #[must_use]
    pub fn gradient_stop(&self) -> &[GradientStop] {
        &self.gradient_stop
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use gradient_stop()")]
    pub fn get_gradient_stop(&self) -> &[GradientStop] {
        self.gradient_stop()
    }

    #[inline]
    pub fn gradient_stop_mut(&mut self) -> &mut Vec<GradientStop> {
        &mut self.gradient_stop
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use gradient_stop_mut()")]
    pub fn get_gradient_stop_mut(&mut self) -> &mut Vec<GradientStop> {
        self.gradient_stop_mut()
    }

    #[inline]
    pub fn set_gradient_stop(
        &mut self,
        value: impl Into<Vec<GradientStop>>,
    ) -> &mut GradientStopList {
        self.gradient_stop = value.into();
        self
    }

    #[inline]
    pub fn add_gradient_stop(&mut self, value: GradientStop) -> &mut GradientStopList {
        self.gradient_stop.push(value);
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

        if empty {
            return Err(crate::xml::xml_error(
                "gradient stop list requires at least two stops",
            ));
        }
        let initial_len = self.gradient_stop.len();
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
                        b"gs" => {
                            let mut value = GradientStop::default();
                            value.read_with_session(reader, child, child_empty, session)?;
                            self.add_gradient_stop(value);
                        }
                        _ => {
                            if !child_empty {
                                crate::reader::driver::skip_element(reader, child)?;
                            }
                        }
                    }
                }
                Event::End(ref end) if end.name().as_ref() == end_name.as_slice() => {
                    return if self.gradient_stop.len() - initial_len >= 2 {
                        Ok(())
                    } else {
                        Err(crate::xml::xml_error(
                            "gradient stop list requires at least two stops",
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
        if self.gradient_stop.is_empty() {
            return Ok(());
        }
        if self.gradient_stop.len() < 2 {
            return Err(crate::xml::xml_error(
                "gradient stop list requires at least two stops",
            ));
        }
        // a:gsLst
        write_start_tag(writer, "a:gsLst", vec![], false);

        // a:gs
        for v in &self.gradient_stop {
            v.write_to(writer)?;
        }

        write_end_tag(writer, "a:gsLst");
        Ok(())
    }
}
