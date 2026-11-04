// a:custDash and its ordered a:ds children.
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::{
        BytesStart,
        Event,
    },
};

use crate::{
    XlsxError,
    reader::xlsx::SourceCacheSession,
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct DashStop {
    dash_length:  String,
    space_length: String,
}

impl Default for DashStop {
    fn default() -> Self {
        Self {
            dash_length:  "0".into(),
            space_length: "0".into(),
        }
    }
}

impl DashStop {
    /// Dash length as thousandths of a percent, or percentage notation.
    #[must_use]
    pub fn dash_length(&self) -> &str {
        &self.dash_length
    }

    pub fn set_dash_length<S: Into<String>>(&mut self, value: S) -> &mut Self {
        self.dash_length = value.into();
        self
    }

    /// Space length as thousandths of a percent, or percentage notation.
    #[must_use]
    pub fn space_length(&self) -> &str {
        &self.space_length
    }

    pub fn set_space_length<S: Into<String>>(&mut self, value: S) -> &mut Self {
        self.space_length = value.into();
        self
    }

    fn validate(&self) -> Result<(), XlsxError> {
        super::color_transform::validate_positive_percentage(&self.dash_length)?;
        super::color_transform::validate_positive_percentage(&self.space_length)
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct CustomDash {
    #[cfg_attr(feature = "serde", serde(default))]
    stops: Vec<DashStop>,
}

impl CustomDash {
    #[must_use]
    pub fn stops(&self) -> &[DashStop] {
        &self.stops
    }

    pub fn stops_mut(&mut self) -> &mut Vec<DashStop> {
        &mut self.stops
    }

    pub fn set_stops(&mut self, value: Vec<DashStop>) -> &mut Self {
        self.stops = value;
        self
    }

    pub(crate) fn read_with_session<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        owner: &BytesStart<'_>,
        empty: bool,
        session: Option<&SourceCacheSession>,
    ) -> Result<(), XlsxError> {
        let _scope_guard = session
            .map(|session| session.enter_child_scope(owner))
            .transpose()?
            .flatten();
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
                    if !native
                        || crate::reader::driver::local_name(child.name().into_inner()) != b"ds"
                    {
                        return Err(crate::xml::xml_error(
                            "unexpected DrawingML custom dash child",
                        ));
                    }
                    let stop = DashStop {
                        dash_length:  crate::reader::driver::require_attribute(child, b"d")?,
                        space_length: crate::reader::driver::require_attribute(child, b"sp")?,
                    };
                    stop.validate()?;
                    if !child_empty {
                        super::color_transform::consume_paint_leaf(reader, child, session)?;
                    }
                    self.stops.push(stop);
                }
                Event::End(ref end) if end.name().as_ref() == end_name.as_slice() => return Ok(()),
                Event::Text(ref text)
                    if text
                        .iter()
                        .all(|byte| matches!(byte, b' ' | b'\t' | b'\r' | b'\n')) => {}
                Event::Comment(_) | Event::PI(_) => (),
                Event::Eof => {
                    return Err(crate::xml::xml_error("unterminated DrawingML custom dash"));
                }
                _ => {
                    return Err(crate::xml::xml_error(
                        "unexpected DrawingML custom dash content",
                    ));
                }
            }
            buffer.clear();
        }
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) -> Result<(), XlsxError> {
        for stop in &self.stops {
            stop.validate()?;
        }
        write_start_tag(writer, "a:custDash", vec![], false);
        for stop in &self.stops {
            write_start_tag(
                writer,
                "a:ds",
                vec![
                    ("d", stop.dash_length()).into(),
                    ("sp", stop.space_length()).into(),
                ],
                true,
            );
        }
        write_end_tag(writer, "a:custDash");
        Ok(())
    }
}
