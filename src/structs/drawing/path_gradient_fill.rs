//! Native path-gradient geometry.
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::{
        BytesStart,
        Event,
    },
};

use super::SourceRectangle;
use crate::{
    XlsxError,
    reader::xlsx::SourceCacheSession,
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathShadeType {
    Shape,
    Circle,
    Rectangle,
}

impl PathShadeType {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Shape => "shape",
            Self::Circle => "circle",
            Self::Rectangle => "rect",
        }
    }
}
impl std::str::FromStr for PathShadeType {
    type Err = XlsxError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "shape" => Ok(Self::Shape),
            "circle" => Ok(Self::Circle),
            "rect" => Ok(Self::Rectangle),
            _ => Err(crate::xml::xml_error("invalid gradient path shape")),
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct PathGradientFill {
    path:              Option<PathShadeType>,
    fill_to_rectangle: Option<SourceRectangle>,
}
impl PathGradientFill {
    #[must_use]
    pub fn path(&self) -> Option<PathShadeType> {
        self.path
    }

    pub fn set_path(&mut self, value: PathShadeType) -> &mut Self {
        self.path = Some(value);
        self
    }

    pub fn clear_path(&mut self) -> &mut Self {
        self.path = None;
        self
    }

    #[must_use]
    pub fn fill_to_rectangle(&self) -> Option<&SourceRectangle> {
        self.fill_to_rectangle.as_ref()
    }

    pub fn fill_to_rectangle_mut(&mut self) -> Option<&mut SourceRectangle> {
        self.fill_to_rectangle.as_mut()
    }

    pub fn set_fill_to_rectangle(&mut self, value: SourceRectangle) -> &mut Self {
        self.fill_to_rectangle = Some(value);
        self
    }

    pub fn clear_fill_to_rectangle(&mut self) -> &mut Self {
        self.fill_to_rectangle = None;
        self
    }

    pub(crate) fn read_with_session<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        owner: &BytesStart<'_>,
        empty: bool,
        session: Option<&SourceCacheSession>,
    ) -> Result<(), XlsxError> {
        let _guard = session
            .map(|s| s.enter_child_scope(owner))
            .transpose()?
            .flatten();
        self.path = crate::reader::driver::read_attribute(owner, b"path")?
            .map(|s| crate::xml::trim_xml_whitespace(&s).parse())
            .transpose()?;
        if empty {
            return Ok(());
        }
        let end = owner.name().as_ref().to_vec();
        let mut buffer = Vec::new();
        loop {
            match reader.read_event_into(&mut buffer)? {
                ref event @ (Event::Start(ref child) | Event::Empty(ref child)) => {
                    if let Some(session) = session {
                        if !session.accepts_child_namespace(
                            child,
                            crate::helper::const_str::DRAWINGML_MAIN_NS,
                        )? {
                            if matches!(event, Event::Start(_)) {
                                crate::reader::driver::skip_element(reader, child)?;
                            }
                            continue;
                        }
                    }
                    if crate::reader::driver::local_name(child.name().as_ref()) != b"fillToRect" {
                        if matches!(event, Event::Start(_)) {
                            crate::reader::driver::skip_element(reader, child)?;
                        }
                        continue;
                    }
                    if self.fill_to_rectangle.is_some() {
                        return Err(crate::xml::xml_error(
                            "unexpected or duplicate gradient fill rectangle",
                        ));
                    }
                    let mut rectangle = SourceRectangle::default();
                    rectangle.read_with_session(
                        reader,
                        child,
                        matches!(event, Event::Empty(_)),
                        session,
                    )?;
                    self.fill_to_rectangle = Some(rectangle);
                }
                Event::End(child) if child.name().as_ref() == end => return Ok(()),
                Event::Text(text)
                    if text
                        .iter()
                        .all(|b| matches!(b, b' ' | b'\t' | b'\r' | b'\n')) => {}
                Event::Comment(_) | Event::PI(_) => (),
                _ => {
                    return Err(crate::xml::xml_error(
                        "invalid or unterminated gradient path",
                    ));
                }
            }
            buffer.clear();
        }
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) -> Result<(), XlsxError> {
        let mut attrs: crate::structs::AttrCollection = Vec::new();
        if let Some(path) = self.path {
            attrs.push(("path", path.as_str()).into());
        }
        write_start_tag(writer, "a:path", attrs, self.fill_to_rectangle.is_none());
        if let Some(rectangle) = &self.fill_to_rectangle {
            rectangle.write_to_as(writer, "a:fillToRect")?;
            write_end_tag(writer, "a:path");
        }
        Ok(())
    }
}
