// a:blipFill
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
    super::super::BooleanValue,
    Blip,
    SourceRectangle,
    Stretch,
};
use crate::{
    structs::raw::RawRelationships,
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct BlipFill {
    rotate_with_shape: BooleanValue,
    blip:              Blip,
    source_rectangle:  Option<Box<SourceRectangle>>,
    stretch:           Stretch,
}

impl BlipFill {
    #[inline]
    #[must_use]
    pub fn rotate_with_shape(&self) -> bool {
        self.rotate_with_shape.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use rotate_with_shape()")]
    pub fn get_rotate_with_shape(&self) -> bool {
        self.rotate_with_shape()
    }

    #[inline]
    pub fn set_rotate_with_shape(&mut self, value: bool) -> &mut Self {
        self.rotate_with_shape.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn source_rectangle(&self) -> Option<&SourceRectangle> {
        self.source_rectangle.as_deref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use source_rectangle()")]
    pub fn get_source_rectangle(&self) -> Option<&SourceRectangle> {
        self.source_rectangle()
    }

    #[inline]
    pub fn source_rectangle_mut(&mut self) -> Option<&mut SourceRectangle> {
        self.source_rectangle.as_deref_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use source_rectangle_mut()")]
    pub fn get_source_rectangle_mut(&mut self) -> Option<&mut SourceRectangle> {
        self.source_rectangle_mut()
    }

    #[inline]
    pub fn set_source_rectangle(&mut self, value: SourceRectangle) -> &mut Self {
        self.source_rectangle = Some(Box::new(value));
        self
    }

    #[inline]
    #[must_use]
    pub fn blip(&self) -> &Blip {
        &self.blip
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use blip()")]
    pub fn get_blip(&self) -> &Blip {
        self.blip()
    }

    #[inline]
    pub fn blip_mut(&mut self) -> &mut Blip {
        &mut self.blip
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use blip_mut()")]
    pub fn get_blip_mut(&mut self) -> &mut Blip {
        self.blip_mut()
    }

    #[inline]
    pub fn set_blip(&mut self, value: Blip) -> &mut Self {
        self.blip = value;
        self
    }

    #[inline]
    #[must_use]
    pub fn stretch(&self) -> &Stretch {
        &self.stretch
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use stretch()")]
    pub fn get_stretch(&self) -> &Stretch {
        self.stretch()
    }

    #[inline]
    pub fn stretch_mut(&mut self) -> &mut Stretch {
        &mut self.stretch
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use stretch_mut()")]
    pub fn get_stretch_mut(&mut self) -> &mut Stretch {
        self.stretch_mut()
    }

    #[inline]
    pub fn set_stretch(&mut self, value: Stretch) -> &mut Self {
        self.stretch = value;
        self
    }

    pub(crate) fn read_with_session<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        owner: &BytesStart,
        empty: bool,
        drawing_relationships: Option<&RawRelationships>,
        session: &crate::reader::xlsx::SourceCacheSession,
    ) -> Result<(), crate::XlsxError> {
        let _guard = session.enter_child_scope(owner)?;
        let scope = crate::drawing::charts::NamespaceScope::default()
            .child(owner)
            .map_err(crate::xml::xml_error)?;
        if let Some(value) = crate::reader::driver::read_attribute(owner, b"rotWithShape")? {
            self.set_rotate_with_shape(
                crate::xml::parse_bool(&value).map_err(crate::xml::xml_error)?,
            );
        }
        if empty {
            return Ok(());
        }
        let end = owner.name().as_ref().to_vec();
        let mut buffer = Vec::new();
        let mut seen = std::collections::HashSet::new();
        loop {
            match reader.read_event_into(&mut buffer)? {
                ref event @ (Event::Start(ref child) | Event::Empty(ref child)) => {
                    let empty = matches!(event, Event::Empty(_));
                    let name = scope
                        .child(child)
                        .and_then(|scope| scope.resolve(child.name().as_ref(), false))
                        .map_err(crate::xml::xml_error)?;
                    if name.namespace_uri != crate::helper::const_str::DRAWINGML_MAIN_NS
                        || !seen.insert(name.local_name.clone())
                    {
                        return Err(crate::xml::xml_error(
                            "foreign or duplicate drawing blip-fill child",
                        ));
                    }
                    match name.local_name.as_str() {
                        "blip" => {
                            let relationships = drawing_relationships.ok_or_else(|| {
                                crate::xml::xml_error("drawing image relationships are missing")
                            })?;
                            self.blip.read_with_session(
                                reader,
                                child,
                                empty,
                                relationships,
                                Some(session),
                            )?;
                        }
                        "srcRect" => {
                            let mut value = SourceRectangle::default();
                            value.read_with_session(reader, child, empty, Some(session))?;
                            self.set_source_rectangle(value);
                        }
                        "stretch" => {
                            self.stretch
                                .read_with_session(reader, child, empty, Some(session))?;
                        }
                        _ => {
                            return Err(crate::xml::xml_error(
                                "unsupported drawing blip-fill child",
                            ));
                        }
                    }
                }
                Event::End(child) if child.name().as_ref() == end.as_slice() => return Ok(()),
                Event::Eof => return Err(crate::xml::xml_error("unterminated drawing blip fill")),
                _ => (),
            }
            buffer.clear();
        }
    }

    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        rel_list: &mut Vec<(String, String)>,
    ) -> Result<(), crate::XlsxError> {
        self.write_to_as(writer, rel_list, "a:blipFill")
    }

    pub(crate) fn write_to_as(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        rel_list: &mut Vec<(String, String)>,
        name: &str,
    ) -> Result<(), crate::XlsxError> {
        let mut attributes: crate::structs::AttrCollection = Vec::new();
        if self.rotate_with_shape.has_value() {
            attributes.push(("rotWithShape", self.rotate_with_shape.value_string()).into());
        }
        write_start_tag(writer, name, attributes, false);

        // a:blip
        self.blip.write_to(writer, rel_list)?;

        // a:srcRect
        if let Some(v) = &self.source_rectangle {
            v.write_to(writer)?;
        }

        // a:stretch
        self.stretch.write_to(writer)?;

        write_end_tag(writer, name);
        Ok(())
    }
}
