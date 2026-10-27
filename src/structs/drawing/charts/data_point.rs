use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::Event,
};

use super::{
    Explosion,
    Marker,
    PictureOptions,
    ShapeProperties,
    XmlBoolean,
    XmlUnsignedInt,
};
use crate::{
    reader::driver::{
        local_name,
        skip_element,
    },
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
    xml_read_loop,
};
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct DataPoint {
    index:              XmlUnsignedInt,
    invert_if_negative: Option<XmlBoolean>,
    marker:             Option<Marker>,
    bubble_3d:          Option<XmlBoolean>,
    explosion:          Option<Explosion>,
    shape_properties:   Option<ShapeProperties>,
    picture_options:    Option<PictureOptions>,
}
impl DataPoint {
    #[must_use]
    pub fn index(&self) -> &XmlUnsignedInt {
        &self.index
    }

    pub fn index_mut(&mut self) -> &mut XmlUnsignedInt {
        &mut self.index
    }

    /// Missing or malformed required idx/@val stays invalid in storage.
    #[must_use]
    pub fn has_index(&self) -> bool {
        self.index.val().is_some()
    }

    #[must_use]
    pub fn invert_if_negative(&self) -> Option<&XmlBoolean> {
        self.invert_if_negative.as_ref()
    }

    pub fn invert_if_negative_mut(&mut self) -> Option<&mut XmlBoolean> {
        self.invert_if_negative.as_mut()
    }

    pub fn set_invert_if_negative(&mut self, value: XmlBoolean) -> &mut Self {
        self.invert_if_negative = Some(value);
        self
    }

    #[must_use]
    pub fn marker(&self) -> Option<&Marker> {
        self.marker.as_ref()
    }

    pub fn marker_mut(&mut self) -> Option<&mut Marker> {
        self.marker.as_mut()
    }

    pub fn set_marker(&mut self, value: Marker) -> &mut Self {
        self.marker = Some(value);
        self
    }

    #[must_use]
    pub fn bubble_3d(&self) -> Option<&XmlBoolean> {
        self.bubble_3d.as_ref()
    }

    pub fn bubble_3d_mut(&mut self) -> Option<&mut XmlBoolean> {
        self.bubble_3d.as_mut()
    }

    pub fn set_bubble_3d(&mut self, value: XmlBoolean) -> &mut Self {
        self.bubble_3d = Some(value);
        self
    }

    #[must_use]
    pub fn explosion(&self) -> Option<&Explosion> {
        self.explosion.as_ref()
    }

    pub fn explosion_mut(&mut self) -> Option<&mut Explosion> {
        self.explosion.as_mut()
    }

    pub fn set_explosion(&mut self, value: Explosion) -> &mut Self {
        self.explosion = Some(value);
        self
    }

    #[must_use]
    pub fn shape_properties(&self) -> Option<&ShapeProperties> {
        self.shape_properties.as_ref()
    }

    pub fn shape_properties_mut(&mut self) -> Option<&mut ShapeProperties> {
        self.shape_properties.as_mut()
    }

    pub fn set_shape_properties(&mut self, value: ShapeProperties) -> &mut Self {
        self.shape_properties = Some(value);
        self
    }

    #[must_use]
    pub fn picture_options(&self) -> Option<&PictureOptions> {
        self.picture_options.as_ref()
    }

    pub fn picture_options_mut(&mut self) -> Option<&mut PictureOptions> {
        self.picture_options.as_mut()
    }

    pub fn set_picture_options(&mut self, value: PictureOptions) -> &mut Self {
        self.picture_options = Some(value);
        self
    }

    pub(crate) fn read<R: std::io::BufRead>(reader: &mut Reader<R>, empty: bool) -> Self {
        let mut result = Self::default();
        if empty {
            return result;
        }
        xml_read_loop!(reader,
            ref event @ (Event::Empty(ref e) | Event::Start(ref e)) => {
                let empty = matches!(event, Event::Empty(_));
                let mut consumed = false;
                match local_name(e.name().into_inner()) {
                    b"idx" => { result.index=XmlUnsignedInt::read(e); },
                    b"invertIfNegative" => { result.invert_if_negative=Some(XmlBoolean::read(e)); },
                    b"bubble3D" => { result.bubble_3d=Some(XmlBoolean::read(e)); },
                    b"explosion" => { let mut value=Explosion::default(); value.set_attributes(reader,e); result.explosion=Some(value); },
                    b"marker" => { let mut value=Marker::default(); value.set_attributes(reader,e,empty); result.marker=Some(value); consumed=true; },
                    b"spPr" => { let mut value=ShapeProperties::default(); if !empty { value.set_attributes(reader,e); } result.shape_properties=Some(value); consumed=true; },
                    b"pictureOptions" => { result.picture_options=Some(PictureOptions::read(reader,empty)); consumed=true; },
                    _ => (),
                }
                if !empty && !consumed { skip_element(reader,e); }
            },
            Event::End(ref e) => { if local_name(e.name().into_inner()) == b"dPt" { return result; } },
            Event::Eof => panic!("Could not find dPt end element"),
        );
    }

    pub(crate) fn is_valid(&self) -> bool {
        self.has_index()
            && self
                .invert_if_negative
                .as_ref()
                .is_none_or(XmlBoolean::is_valid)
            && self.bubble_3d.as_ref().is_none_or(XmlBoolean::is_valid)
            && self.explosion.as_ref().is_none_or(Explosion::is_valid)
            && self.marker.as_ref().is_none_or(Marker::is_valid)
            && self
                .picture_options
                .as_ref()
                .is_none_or(PictureOptions::is_valid)
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        if !self.is_valid() {
            return;
        }
        write_start_tag(writer, "c:dPt", vec![], false);
        self.index.write_to(writer);
        if let Some(v) = &self.invert_if_negative {
            v.write_to(writer, "c:invertIfNegative");
        }
        if let Some(v) = &self.marker {
            v.write_to(writer);
        }
        if let Some(v) = &self.bubble_3d {
            v.write_to(writer, "c:bubble3D");
        }
        if let Some(v) = &self.explosion {
            v.write_to(writer);
        }
        if let Some(v) = &self.shape_properties {
            v.write_to(writer);
        }
        if let Some(v) = &self.picture_options {
            v.write_to(writer);
        }
        write_end_tag(writer, "c:dPt");
    }
}
