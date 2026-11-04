// a:avLst
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::{
        BytesStart,
        Event,
    },
};

use super::shape_guide::ShapeGuide;
use crate::writer::driver::{
    write_end_tag,
    write_start_tag,
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct AdjustValueList {
    shape_guide_collection: Vec<ShapeGuide>,
}

impl AdjustValueList {
    #[inline]
    #[must_use]
    pub fn shape_guide_collection(&self) -> &[ShapeGuide] {
        &self.shape_guide_collection
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use shape_guide_collection()")]
    pub fn get_shape_guide_collection(&self) -> &[ShapeGuide] {
        self.shape_guide_collection()
    }

    #[inline]
    pub fn shape_guide_collection_mut(&mut self) -> &mut Vec<ShapeGuide> {
        &mut self.shape_guide_collection
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use shape_guide_collection_mut()")]
    pub fn get_shape_guide_collection_mut(&mut self) -> &mut Vec<ShapeGuide> {
        self.shape_guide_collection_mut()
    }

    #[inline]
    pub fn set_shape_guide_collection(&mut self, value: impl Into<Vec<ShapeGuide>>) {
        self.shape_guide_collection = value.into();
    }

    #[inline]
    pub fn add_shape_guide_collection(&mut self, value: ShapeGuide) {
        self.shape_guide_collection.push(value);
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
    ) -> Result<(), crate::XlsxError> {
        self.read_with_session(reader, e, false, None)
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
                        b"gd" => {
                            let mut value = ShapeGuide::default();
                            value.set_name(crate::reader::driver::require_attribute(
                                child, b"name",
                            )?);
                            value.set_fmla(crate::reader::driver::require_attribute(
                                child, b"fmla",
                            )?);
                            if !child_empty {
                                crate::structs::drawing::color_transform::consume_paint_leaf(
                                    reader, child, session,
                                )?;
                            }
                            self.add_shape_guide_collection(value);
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

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        // a:avLst
        if self.shape_guide_collection.is_empty() {
            write_start_tag(writer, "a:avLst", vec![], true);
        } else {
            write_start_tag(writer, "a:avLst", vec![], false);
            for gd in &self.shape_guide_collection {
                gd.write_to(writer);
            }
            write_end_tag(writer, "a:avLst");
        }
    }
}
