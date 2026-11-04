use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::{
        BytesStart,
        Event,
    },
};

use super::Outline;
use crate::{
    XlsxError,
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct LineStyleList {
    outline_collection: Vec<Outline>,
}

impl LineStyleList {
    #[inline]
    #[must_use]
    pub fn outline_collection(&self) -> &[Outline] {
        &self.outline_collection
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use outline_collection()")]
    pub fn get_outline_collection(&self) -> &[Outline] {
        self.outline_collection()
    }

    #[inline]
    pub fn outline_collection_mut(&mut self) -> &mut Vec<Outline> {
        &mut self.outline_collection
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use outline_collection_mut()")]
    pub fn get_outline_collection_mut(&mut self) -> &mut Vec<Outline> {
        self.outline_collection_mut()
    }

    #[inline]
    pub fn set_outline_collection(&mut self, value: impl Into<Vec<Outline>>) -> &mut Self {
        self.outline_collection = value.into();
        self
    }

    #[inline]
    pub fn add_outline_collection(&mut self, value: Outline) -> &mut Self {
        self.outline_collection.push(value);
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

        if empty {
            return Ok(());
        }
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
                        b"ln" => {
                            let mut value = Outline::default();
                            value.read_with_session(reader, child, child_empty, session)?;
                            self.outline_collection.push(value);
                        }
                        _ => {
                            if !child_empty {
                                crate::reader::driver::skip_element(reader, child)?;
                            }
                        }
                    }
                }
                Event::End(ref child) if child.name().as_ref() == end.as_slice() => {
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

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) -> Result<(), XlsxError> {
        // a:lnStyleLst
        write_start_tag(writer, "a:lnStyleLst", vec![], false);

        // a:ln
        for v in &self.outline_collection {
            v.write_to(writer)?;
        }

        write_end_tag(writer, "a:lnStyleLst");
        Ok(())
    }
}
