// a:fontScheme
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
    super::StringValue,
    FontCollectionType,
};
use crate::{
    XlsxError,
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct FontScheme {
    name:       StringValue,
    major_font: FontCollectionType,
    minor_font: FontCollectionType,
}

impl FontScheme {
    #[inline]
    #[must_use]
    pub fn name(&self) -> &str {
        self.name.value_str()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use name()")]
    pub fn get_name(&self) -> &str {
        self.name()
    }

    #[inline]
    pub fn set_name<S: Into<String>>(&mut self, value: S) -> &mut Self {
        self.name.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn major_font(&self) -> &FontCollectionType {
        &self.major_font
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use major_font()")]
    pub fn get_major_font(&self) -> &FontCollectionType {
        self.major_font()
    }

    #[inline]
    pub fn major_font_mut(&mut self) -> &mut FontCollectionType {
        &mut self.major_font
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use major_font_mut()")]
    pub fn get_major_font_mut(&mut self) -> &mut FontCollectionType {
        self.major_font_mut()
    }

    #[inline]
    pub fn set_major_font(&mut self, value: FontCollectionType) -> &mut Self {
        self.major_font = value;
        self
    }

    #[inline]
    #[must_use]
    pub fn minor_font(&self) -> &FontCollectionType {
        &self.minor_font
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use minor_font()")]
    pub fn get_minor_font(&self) -> &FontCollectionType {
        self.minor_font()
    }

    #[inline]
    pub fn minor_font_mut(&mut self) -> &mut FontCollectionType {
        &mut self.minor_font
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use minor_font_mut()")]
    pub fn get_minor_font_mut(&mut self) -> &mut FontCollectionType {
        self.minor_font_mut()
    }

    #[inline]
    pub fn set_minor_font(&mut self, value: FontCollectionType) -> &mut Self {
        self.minor_font = value;
        self
    }

    pub(crate) fn read_with_session<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        owner: &BytesStart<'_>,
        empty: bool,
        session: Option<&crate::reader::xlsx::SourceCacheSession>,
    ) -> Result<(), XlsxError> {
        let _scope_guard = session
            .map(|session| session.enter_child_scope(owner))
            .transpose()?
            .flatten();
        crate::reader::driver::validate_attributes(owner)?;
        if let Some(value) = crate::reader::driver::read_attribute(owner, b"name")? {
            self.name.set_value(value);
        }
        if empty {
            return Err(crate::xml::xml_error("Theme owner requires child content"));
        }
        let mut seen = 0u32;
        let end = owner.name().as_ref().to_vec();
        let mut buffer = Vec::new();
        loop {
            match reader.read_event_into(&mut buffer)? {
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
                        return Err(crate::xml::xml_error("foreign Theme child"));
                    }
                    crate::reader::driver::validate_attributes(child)?;
                    match crate::reader::driver::local_name(child.name().as_ref()) {
                        b"majorFont" => {
                            if seen & 1 != 0 {
                                return Err(crate::xml::xml_error("duplicate Theme child"));
                            }
                            seen |= 1;
                            self.major_font.read_with_session(
                                reader,
                                child,
                                child_empty,
                                session,
                            )?;
                        }
                        b"minorFont" => {
                            if seen & (1 << 1) != 0 {
                                return Err(crate::xml::xml_error("duplicate Theme child"));
                            }
                            seen |= 1 << 1;
                            self.minor_font.read_with_session(
                                reader,
                                child,
                                child_empty,
                                session,
                            )?;
                        }
                        _ => {
                            if !child_empty {
                                crate::reader::driver::skip_element(reader, child)?;
                            }
                        }
                    }
                }
                Event::End(ref child) if child.name().as_ref() == end.as_slice() => {
                    if seen & 3 != 3 {
                        return Err(crate::xml::xml_error(
                            "Theme owner is missing required children",
                        ));
                    }
                    return Ok(());
                }
                Event::Text(ref text)
                    if !text
                        .iter()
                        .all(|byte| matches!(byte, b' ' | b'\t' | b'\r' | b'\n')) =>
                {
                    return Err(crate::xml::xml_error("unexpected text in Theme container"));
                }
                Event::Eof => return Err(crate::xml::xml_error("unterminated Theme owner")),
                _ => (),
            }
            buffer.clear();
        }
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        // a:fontScheme
        let mut attributes: crate::structs::AttrCollection = Vec::new();
        if self.name.has_value() {
            attributes.push(("name", self.name.value_str()).into());
        }
        write_start_tag(writer, "a:fontScheme", attributes, false);

        // a:majorFont
        self.major_font.write_to_major_font(writer);

        // a:minorFont
        self.minor_font.write_to_minor_font(writer);

        write_end_tag(writer, "a:fontScheme");
    }
}
