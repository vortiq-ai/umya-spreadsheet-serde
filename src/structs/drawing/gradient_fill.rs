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
    TileFlipValues,
    TileRectangle,
};
use crate::{
    reader::driver::{
        get_attribute,
        set_string_from_xml,
        xml_read_loop,
    },
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct GradientFill {
    flip:                 EnumValue<TileFlipValues>,
    rotate_with_shape:    BooleanValue,
    gradient_stop_list:   GradientStopList,
    linear_gradient_fill: Option<Box<LinearGradientFill>>,
    tile_rectangle:       Option<Box<TileRectangle>>,
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
        self.linear_gradient_fill = Some(Box::new(value));
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
    ) {
        self.read_attributes(reader, e, false);
    }

    pub(crate) fn read_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
        empty: bool,
    ) {
        set_string_from_xml!(self, e, flip, "flip");
        set_string_from_xml!(self, e, rotate_with_shape, "rotWithShape");

        if empty {
            return;
        }
        let end_name = e.name().as_ref().to_vec();
        xml_read_loop!(reader,
                    ref event @ (Event::Start(ref e) | Event::Empty(ref e)) => {
                        let empty=matches!(event,Event::Empty(_));
                        match crate::reader::driver::local_name(e.name().into_inner()) {
        b"lin" => { let mut value=LinearGradientFill::default(); value.set_attributes(reader,e,empty); self.set_linear_gradient_fill(value); },
        b"tileRect" => { TileRectangle::set_attributes(reader,e,empty); self.set_tile_rectangle(TileRectangle::default()); },
        b"gsLst" => { self.gradient_stop_list.read_attributes(reader,e,empty); },
                            _=> { if !empty { crate::reader::driver::skip_element(reader,e); } },
                        }
                    },
                    Event::End(ref end) => {
                    if end.name().as_ref() == end_name.as_slice() {
                        return;
                    }
                },
                    Event::Eof => panic!("Could not find drawing owner end element"),
                );
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
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
        self.gradient_stop_list.write_to(writer);

        // a:lin
        if let Some(v) = &self.linear_gradient_fill {
            v.write_to(writer);
        }

        // a:tileRect
        if self.tile_rectangle.is_some() {
            TileRectangle::write_to(writer);
        }

        write_end_tag(writer, "a:gradFill");
    }
}
