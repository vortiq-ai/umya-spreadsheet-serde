// a:xfrm
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
    StringValue,
    structs::drawing::{
        Point2DType,
        PositiveSize2DType,
    },
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct Transform2D {
    offset:        Point2DType,
    extents:       PositiveSize2DType,
    child_offset:  Option<Box<Point2DType>>,
    child_extents: Option<Box<PositiveSize2DType>>,
    rot:           StringValue,
    flip_v:        StringValue,
    flip_h:        StringValue,
}

impl Transform2D {
    #[inline]
    #[must_use]
    pub fn offset(&self) -> &Point2DType {
        &self.offset
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use offset()")]
    pub fn get_offset(&self) -> &Point2DType {
        self.offset()
    }

    #[inline]
    pub fn offset_mut(&mut self) -> &mut Point2DType {
        &mut self.offset
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use offset_mut()")]
    pub fn get_offset_mut(&mut self) -> &mut Point2DType {
        self.offset_mut()
    }

    #[inline]
    pub fn set_offset(&mut self, value: Point2DType) {
        self.offset = value;
    }

    #[inline]
    #[must_use]
    pub fn extents(&self) -> &PositiveSize2DType {
        &self.extents
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use extents()")]
    pub fn get_extents(&self) -> &PositiveSize2DType {
        self.extents()
    }

    #[inline]
    pub fn extents_mut(&mut self) -> &mut PositiveSize2DType {
        &mut self.extents
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use extents_mut()")]
    pub fn get_extents_mut(&mut self) -> &mut PositiveSize2DType {
        self.extents_mut()
    }

    #[inline]
    pub fn set_extents(&mut self, value: PositiveSize2DType) {
        self.extents = value;
    }

    #[inline]
    #[must_use]
    pub fn child_offset(&self) -> Option<&Point2DType> {
        self.child_offset.as_deref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use child_offset()")]
    pub fn get_child_offset(&self) -> Option<&Point2DType> {
        self.child_offset()
    }

    #[inline]
    pub fn child_offset_mut(&mut self) -> Option<&mut Point2DType> {
        self.child_offset.as_deref_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use child_offset_mut()")]
    pub fn get_child_offset_mut(&mut self) -> Option<&mut Point2DType> {
        self.child_offset_mut()
    }

    #[inline]
    pub fn set_child_offset(&mut self, value: Point2DType) {
        self.child_offset = Some(Box::new(value));
    }

    #[inline]
    #[must_use]
    pub fn child_extents(&self) -> Option<&PositiveSize2DType> {
        self.child_extents.as_deref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use child_extents()")]
    pub fn get_child_extents(&self) -> Option<&PositiveSize2DType> {
        self.child_extents()
    }

    #[inline]
    pub fn child_extents_mut(&mut self) -> Option<&mut PositiveSize2DType> {
        self.child_extents.as_deref_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use child_extents_mut()")]
    pub fn get_child_extents_mut(&mut self) -> Option<&mut PositiveSize2DType> {
        self.child_extents_mut()
    }

    #[inline]
    pub fn set_child_extents(&mut self, value: PositiveSize2DType) {
        self.child_extents = Some(Box::new(value));
    }

    #[inline]
    #[must_use]
    pub fn rot(&self) -> Option<&str> {
        self.rot.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use rot()")]
    pub fn get_rot(&self) -> Option<&str> {
        self.rot()
    }

    #[inline]
    pub fn set_rot<S: Into<String>>(&mut self, value: S) {
        self.rot.set_value(value);
    }

    #[inline]
    #[must_use]
    pub fn flip_v(&self) -> Option<&str> {
        self.flip_v.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use flip_v()")]
    pub fn get_flip_v(&self) -> Option<&str> {
        self.flip_v()
    }

    #[inline]
    pub fn set_flip_v<S: Into<String>>(&mut self, value: S) {
        self.flip_v.set_value(value);
    }

    #[inline]
    #[must_use]
    pub fn flip_h(&self) -> Option<&str> {
        self.flip_h.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use flip_h()")]
    pub fn get_flip_h(&self) -> Option<&str> {
        self.flip_h()
    }

    #[inline]
    pub fn set_flip_h<S: Into<String>>(&mut self, value: S) {
        self.flip_h.set_value(value);
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
        if let Some(value) = crate::reader::driver::read_attribute(owner, b"rot")? {
            let _validated = crate::xml::parse_i32(&value).map_err(crate::xml::xml_error)?;
            self.set_rot(value);
        }
        if let Some(value) = crate::reader::driver::read_attribute(owner, b"flipH")? {
            let _validated = crate::xml::parse_bool(&value).map_err(crate::xml::xml_error)?;
            self.set_flip_h(value);
        }
        if let Some(value) = crate::reader::driver::read_attribute(owner, b"flipV")? {
            let _validated = crate::xml::parse_bool(&value).map_err(crate::xml::xml_error)?;
            self.set_flip_v(value);
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
                        b"off" => {
                            self.offset
                                .read_with_session(reader, child, child_empty, session)?;
                        }
                        b"ext" => {
                            self.extents
                                .read_with_session(reader, child, child_empty, session)?;
                        }
                        b"chOff" => {
                            let mut value = Point2DType::default();
                            value.read_with_session(reader, child, child_empty, session)?;
                            self.set_child_offset(value);
                        }
                        b"chExt" => {
                            let mut value = PositiveSize2DType::default();
                            value.read_with_session(reader, child, child_empty, session)?;
                            self.set_child_extents(value);
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
        // a:xfrm
        let mut attributes: crate::structs::AttrCollection = Vec::new();
        if let Some(v) = self.rot.value() {
            attributes.push(("rot", v).into());
        }
        if let Some(v) = self.flip_h.value() {
            attributes.push(("flipH", v).into());
        }
        if let Some(v) = self.flip_v.value() {
            attributes.push(("flipV", v).into());
        }
        write_start_tag(writer, "a:xfrm", attributes, false);

        // a:off
        self.offset.write_to_off(writer);

        // a:ext
        self.extents.write_to_ext(writer);

        // a:chOff
        if let Some(v) = &self.child_offset {
            v.write_to_ch_off(writer);
        }

        // a:chExt
        if let Some(v) = &self.child_extents {
            v.write_to_ch_ext(writer);
        }

        write_end_tag(writer, "a:xfrm");
    }
}
