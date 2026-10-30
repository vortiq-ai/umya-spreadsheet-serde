// xdr:xfrm
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::{
        BytesStart,
        Event,
    },
};

use super::super::{
    Extents,
    Offset,
};
use crate::{
    BooleanValue,
    Int32Value,
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct Transform {
    offset:          Offset,
    extents:         Extents,
    rotation:        Int32Value,
    vertical_flip:   BooleanValue,
    horizontal_flip: BooleanValue,
}

impl Transform {
    #[inline]
    #[must_use]
    pub fn offset(&self) -> &Offset {
        &self.offset
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use offset()")]
    pub fn get_offset(&self) -> &Offset {
        self.offset()
    }

    #[inline]
    pub fn offset_mut(&mut self) -> &mut Offset {
        &mut self.offset
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use offset_mut()")]
    pub fn get_offset_mut(&mut self) -> &mut Offset {
        self.offset_mut()
    }

    #[inline]
    pub fn set_offset(&mut self, value: Offset) -> &mut Transform {
        self.offset = value;
        self
    }

    #[inline]
    #[must_use]
    pub fn extents(&self) -> &Extents {
        &self.extents
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use extents()")]
    pub fn get_extents(&self) -> &Extents {
        self.extents()
    }

    #[inline]
    pub fn extents_mut(&mut self) -> &mut Extents {
        &mut self.extents
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use extents_mut()")]
    pub fn get_extents_mut(&mut self) -> &mut Extents {
        self.extents_mut()
    }

    #[inline]
    pub fn set_extents(&mut self, value: Extents) -> &mut Transform {
        self.extents = value;
        self
    }

    #[inline]
    #[must_use]
    pub fn rotation(&self) -> i32 {
        self.rotation.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use rotation()")]
    pub fn get_rotation(&self) -> i32 {
        self.rotation()
    }

    #[inline]
    pub fn set_rotation(&mut self, value: i32) {
        self.rotation.set_value(value);
    }

    #[inline]
    #[must_use]
    pub fn vertical_flip(&self) -> bool {
        self.vertical_flip.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use vertical_flip()")]
    pub fn get_vertical_flip(&self) -> bool {
        self.vertical_flip()
    }

    #[inline]
    pub fn set_vertical_flip(&mut self, value: bool) {
        self.vertical_flip.set_value(value);
    }

    #[inline]
    #[must_use]
    pub fn horizontal_flip(&self) -> bool {
        self.horizontal_flip.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use horizontal_flip()")]
    pub fn get_horizontal_flip(&self) -> bool {
        self.horizontal_flip()
    }

    #[inline]
    pub fn set_horizontal_flip(&mut self, value: bool) {
        self.horizontal_flip.set_value(value);
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        owner: &BytesStart,
    ) -> Result<(), crate::XlsxError> {
        self.read_with_session(reader, owner, false, None)
    }

    pub(crate) fn read_with_session<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        owner: &BytesStart,
        empty: bool,
        session: Option<&crate::reader::xlsx::SourceCacheSession>,
    ) -> Result<(), crate::XlsxError> {
        let _guard = session
            .map(|session| session.enter_child_scope(owner))
            .transpose()?
            .flatten();
        let scope = crate::drawing::charts::NamespaceScope::default()
            .child(owner)
            .map_err(crate::xml::xml_error)?;
        if let Some(value) = crate::reader::driver::read_attribute(owner, b"rot")? {
            self.set_rotation(crate::xml::parse_i32(&value).map_err(crate::xml::xml_error)?);
        }
        if let Some(value) = crate::reader::driver::read_attribute(owner, b"flipH")? {
            self.set_horizontal_flip(
                crate::xml::parse_bool(&value).map_err(crate::xml::xml_error)?,
            );
        }
        if let Some(value) = crate::reader::driver::read_attribute(owner, b"flipV")? {
            self.set_vertical_flip(crate::xml::parse_bool(&value).map_err(crate::xml::xml_error)?);
        }
        if empty {
            return Ok(());
        }
        let end = owner.name().as_ref().to_vec();
        let mut buffer = Vec::new();
        loop {
            match reader.read_event_into(&mut buffer)? {
                ref event @ (Event::Start(ref child) | Event::Empty(ref child)) => {
                    let empty = matches!(event, Event::Empty(_));
                    let name = scope
                        .child(child)
                        .and_then(|scope| scope.resolve(child.name().as_ref(), false))
                        .map_err(crate::xml::xml_error)?;
                    if name.namespace_uri == crate::helper::const_str::DRAWINGML_MAIN_NS {
                        match name.local_name.as_str() {
                            "off" => self
                                .offset
                                .read_with_session(reader, child, empty, session)?,
                            "ext" => self
                                .extents
                                .read_with_session(reader, child, empty, session)?,
                            _ => {
                                return Err(crate::xml::xml_error(
                                    "unsupported drawing transform child",
                                ));
                            }
                        }
                    } else if !empty {
                        crate::reader::driver::skip_element(reader, child)?;
                    }
                }
                Event::End(child) if child.name().as_ref() == end.as_slice() => return Ok(()),
                Event::Eof => return Err(crate::xml::xml_error("unterminated drawing transform")),
                _ => (),
            }
            buffer.clear();
        }
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        // xdr:xfrm
        let mut attributes: crate::structs::AttrCollection = Vec::new();
        let rot = self.rotation.value_string();
        if self.rotation.has_value() {
            attributes.push(("rot", &rot).into());
        }
        if self.horizontal_flip.has_value() {
            attributes.push(("flipH", self.horizontal_flip.value_string()).into());
        }
        if self.vertical_flip.has_value() {
            attributes.push(("flipV", self.vertical_flip.value_string()).into());
        }
        write_start_tag(writer, "xdr:xfrm", attributes, false);

        // a:off
        self.offset.write_to(writer);

        // a:ext
        self.extents.write_to(writer);

        write_end_tag(writer, "xdr:xfrm");
    }
}
