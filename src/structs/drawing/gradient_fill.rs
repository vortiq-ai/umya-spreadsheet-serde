// a:gradFill
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
    super::{
        super::EnumValue,
        BooleanValue,
    },
    GradientStopList,
    LinearGradientFill,
    PathGradientFill,
    TileFlipValues,
    TileRectangle,
};
use crate::writer::driver::{
    write_end_tag,
    write_start_tag,
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct GradientFill {
    flip:                 EnumValue<TileFlipValues>,
    rotate_with_shape:    BooleanValue,
    gradient_stop_list:   GradientStopList,
    linear_gradient_fill: Option<Box<LinearGradientFill>>,
    tile_rectangle:       Option<Box<TileRectangle>>,
    #[cfg_attr(feature = "serde", serde(default))]
    path_gradient_fill:   Option<Box<PathGradientFill>>,
}

impl GradientFill {
    #[inline]
    #[must_use]
    pub fn flip(&self) -> &TileFlipValues {
        self.flip.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use flip()")]
    pub fn get_flip(&self) -> &TileFlipValues {
        self.flip()
    }

    #[inline]
    pub fn set_flip(&mut self, value: TileFlipValues) -> &mut GradientFill {
        self.flip.set_value(value);
        self
    }

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
    pub fn set_rotate_with_shape(&mut self, value: bool) -> &mut GradientFill {
        self.rotate_with_shape.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn gradient_stop_list(&self) -> &GradientStopList {
        &self.gradient_stop_list
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use gradient_stop_list()")]
    pub fn get_gradient_stop_list(&self) -> &GradientStopList {
        self.gradient_stop_list()
    }

    #[inline]
    pub fn gradient_stop_list_mut(&mut self) -> &mut GradientStopList {
        &mut self.gradient_stop_list
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use gradient_stop_list_mut()")]
    pub fn get_gradient_stop_list_mut(&mut self) -> &mut GradientStopList {
        self.gradient_stop_list_mut()
    }

    #[inline]
    pub fn set_gradient_stop_list(&mut self, value: GradientStopList) -> &mut GradientFill {
        self.gradient_stop_list = value;
        self
    }

    #[inline]
    #[must_use]
    pub fn linear_gradient_fill(&self) -> Option<&LinearGradientFill> {
        self.linear_gradient_fill.as_deref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use linear_gradient_fill()")]
    pub fn get_linear_gradient_fill(&self) -> Option<&LinearGradientFill> {
        self.linear_gradient_fill()
    }

    #[inline]
    pub fn linear_gradient_fill_mut(&mut self) -> Option<&mut LinearGradientFill> {
        self.linear_gradient_fill.as_deref_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use linear_gradient_fill_mut()")]
    pub fn get_linear_gradient_fill_mut(&mut self) -> Option<&mut LinearGradientFill> {
        self.linear_gradient_fill_mut()
    }

    #[inline]
    pub fn set_linear_gradient_fill(&mut self, value: LinearGradientFill) -> &mut GradientFill {
        self.path_gradient_fill = None;
        self.linear_gradient_fill = Some(Box::new(value));
        self
    }

    #[must_use]
    pub fn path_gradient_fill(&self) -> Option<&PathGradientFill> {
        self.path_gradient_fill.as_deref()
    }

    pub fn path_gradient_fill_mut(&mut self) -> Option<&mut PathGradientFill> {
        self.path_gradient_fill.as_deref_mut()
    }

    pub fn set_path_gradient_fill(&mut self, value: PathGradientFill) -> &mut Self {
        self.linear_gradient_fill = None;
        self.path_gradient_fill = Some(Box::new(value));
        self
    }

    pub fn clear_shading(&mut self) -> &mut Self {
        self.linear_gradient_fill = None;
        self.path_gradient_fill = None;
        self
    }

    #[inline]
    #[must_use]
    pub fn tile_rectangle(&self) -> Option<&TileRectangle> {
        self.tile_rectangle.as_deref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use tile_rectangle()")]
    pub fn get_tile_rectangle(&self) -> Option<&TileRectangle> {
        self.tile_rectangle()
    }

    #[inline]
    pub fn tile_rectangle_mut(&mut self) -> Option<&mut TileRectangle> {
        self.tile_rectangle.as_deref_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use tile_rectangle_mut()")]
    pub fn get_tile_rectangle_mut(&mut self) -> Option<&mut TileRectangle> {
        self.tile_rectangle_mut()
    }

    #[inline]
    pub fn set_tile_rectangle(&mut self, value: TileRectangle) -> &mut GradientFill {
        self.tile_rectangle = Some(Box::new(value));
        self
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
    ) -> Result<(), crate::XlsxError> {
        self.read_with_session(reader, e, false, None)
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
        if let Some(value) = crate::reader::driver::read_attribute(owner, b"flip")? {
            self.flip.set_value(
                value
                    .parse::<TileFlipValues>()
                    .map_err(|()| crate::xml::xml_error("invalid DrawingML enumeration"))?,
            );
        }
        if let Some(value) = crate::reader::driver::read_attribute(owner, b"rotWithShape")? {
            self.rotate_with_shape
                .set_value(crate::xml::parse_bool(&value).map_err(crate::xml::xml_error)?);
        }
        if empty {
            return Ok(());
        }
        let end_name = owner.name().as_ref().to_vec();
        let mut buffer = Vec::new();
        let mut stops_seen = false;
        let mut shading_seen = false;
        let mut tile_seen = false;
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
                        b"lin" => {
                            if shading_seen {
                                return Err(crate::xml::xml_error(
                                    "duplicate or conflicting gradient shading",
                                ));
                            }
                            shading_seen = true;
                            let mut value = LinearGradientFill::default();
                            value.read_with_session(reader, child, child_empty, session)?;
                            self.set_linear_gradient_fill(value);
                        }
                        b"path" => {
                            if shading_seen {
                                return Err(crate::xml::xml_error(
                                    "duplicate or conflicting gradient shading",
                                ));
                            }
                            shading_seen = true;
                            let mut value = PathGradientFill::default();
                            value.read_with_session(reader, child, child_empty, session)?;
                            self.set_path_gradient_fill(value);
                        }
                        b"tileRect" => {
                            if tile_seen {
                                return Err(crate::xml::xml_error(
                                    "duplicate gradient tile rectangle",
                                ));
                            }
                            tile_seen = true;
                            let mut value = TileRectangle::default();
                            value.read_with_session(reader, child, child_empty, session)?;
                            self.set_tile_rectangle(value);
                        }
                        b"gsLst" => {
                            if stops_seen {
                                return Err(crate::xml::xml_error("duplicate gradient stop list"));
                            }
                            stops_seen = true;
                            self.gradient_stop_list.read_with_session(
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
        if self.linear_gradient_fill.is_some() && self.path_gradient_fill.is_some() {
            return Err(crate::xml::xml_error("conflicting gradient shading"));
        }
        // a:gradFill
        let mut attributes: crate::structs::AttrCollection = Vec::new();
        if self.flip.has_value() {
            attributes.push(("flip", self.flip.value_string()).into());
        }
        if self.rotate_with_shape.has_value() {
            attributes.push(("rotWithShape", self.rotate_with_shape.value_string()).into());
        }
        write_start_tag(writer, "a:gradFill", attributes, false);

        // a:gsLst
        self.gradient_stop_list.write_to(writer)?;

        // a:lin
        if let Some(v) = &self.linear_gradient_fill {
            v.write_to(writer);
        }

        // a:tileRect
        if let Some(path) = &self.path_gradient_fill {
            path.write_to(writer)?;
        }
        if let Some(rectangle) = &self.tile_rectangle {
            rectangle.write_to(writer)?;
        }

        write_end_tag(writer, "a:gradFill");
        Ok(())
    }
}
