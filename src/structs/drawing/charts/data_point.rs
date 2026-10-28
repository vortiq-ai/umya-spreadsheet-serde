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
    #[cfg_attr(feature = "serde", serde(default))]
    extensions:         Option<Box<super::ExtensionList>>,
}
impl DataPoint {
    // Projection only: retain diagnostics and absent children exactly.
    pub(crate) fn reference_role_parts(&self) -> (
        Option<&Marker>,
        Option<&ShapeProperties>,
        Option<&super::ExtensionList>,
    ) {
        (
            self.marker.as_ref(),
            self.shape_properties.as_ref(),
            self.extensions.as_deref(),
        )
    }

    pub(crate) fn reference_role_parts_mut(&mut self) -> (
        Option<&mut Marker>,
        Option<&mut ShapeProperties>,
        Option<&mut super::ExtensionList>,
    ) {
        (
            self.marker.as_mut(),
            self.shape_properties.as_mut(),
            self.extensions.as_deref_mut(),
        )
    }


    #[must_use]
    pub fn extensions(&self) -> Option<&super::ExtensionList> {
        self.extensions.as_deref()
    }

    pub fn extensions_mut(&mut self) -> Option<&mut super::ExtensionList> {
        self.extensions.as_deref_mut()
    }

    pub fn set_extensions(&mut self, value: Option<super::ExtensionList>) -> &mut Self {
        self.extensions = value.map(Box::new);
        self
    }

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
        Self::read_common(reader, empty, None, None)
    }

    pub(crate) fn read_with_session<R: std::io::BufRead>(
        reader: &mut Reader<R>,
        e: &quick_xml::events::BytesStart,
        empty: bool,
        session: &crate::reader::xlsx::SourceCacheSession,
    ) -> Self {
        Self::read_common(reader, empty, Some(session), Some(e))
    }

    fn read_common<R: std::io::BufRead>(
        reader: &mut Reader<R>,
        empty: bool,
        session: Option<&crate::reader::xlsx::SourceCacheSession>,
        owner: Option<&quick_xml::events::BytesStart>,
    ) -> Self {
        let _scope_guard = session
            .zip(owner)
            .and_then(|(session, e)| session.enter_child_scope(e));
        let end = owner.map(|e| e.name().into_inner().to_vec());
        let mut result = Self::default();
        if empty {
            return result;
        }
        xml_read_loop!(reader,
            ref event @ (Event::Empty(ref e) | Event::Start(ref e)) => {
                let empty = matches!(event, Event::Empty(_));
                let mut consumed = false;
                match if session.is_none_or(|session|session.accepts_child_namespace(e,super::extension_registry::NS_C)){local_name(e.name().into_inner())}else{b""} {
                    b"extLst" if session.is_some()=>{result.set_extensions(Some(super::ExtensionList::read(reader,e,empty,session.expect("checked session"))));consumed=true;},
                    b"idx" => { result.index=XmlUnsignedInt::read(e); },
                    b"invertIfNegative" => { result.invert_if_negative=Some(XmlBoolean::read(e)); },
                    b"bubble3D" => { result.bubble_3d=Some(XmlBoolean::read(e)); },
                    b"explosion" => { let mut value=Explosion::default(); value.set_attributes(reader,e); result.explosion=Some(value); },
                    b"marker" => { let mut value=Marker::default(); match session{Some(session)=>value.set_attributes_with_session(reader,e,empty,session),None=>value.set_attributes(reader,e,empty)} result.marker=Some(value); consumed=true; },
                    b"spPr" => { let mut value=ShapeProperties::default(); if !empty { match session{Some(session)=>value.set_attributes_with_session(reader,e,session),None=>value.set_attributes(reader,e)} } result.shape_properties=Some(value); consumed=true; },
                    b"pictureOptions" => { result.picture_options=Some(PictureOptions::read(reader,empty)); consumed=true; },
                    _ => (),
                }
                if !empty && !consumed { skip_element(reader,e); }
            },
            Event::End(ref e) => { if end.as_ref().map_or_else(||local_name(e.name().into_inner())==b"dPt",|name|e.name().into_inner()==name.as_slice()) { return result; } },
            Event::Eof => panic!("Could not find dPt end element"),
        );
    }

    pub(crate) fn is_valid(&self) -> bool {
        self.structural_is_valid()
            && self.marker.as_ref().is_none_or(Marker::is_valid)
            && self
                .extensions()
                .is_none_or(|ext| ext.conflict_for_owner("dPt", None).is_none())
    }

    fn structural_is_valid(&self) -> bool {
        self.has_index()
            && self
                .invert_if_negative
                .as_ref()
                .is_none_or(XmlBoolean::is_valid)
            && self.bubble_3d.as_ref().is_none_or(XmlBoolean::is_valid)
            && self.explosion.as_ref().is_none_or(Explosion::is_valid)
            && self.marker.as_ref().is_none_or(Marker::structural_is_valid)
            && self
                .picture_options
                .as_ref()
                .is_none_or(PictureOptions::is_valid)
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        if !self.structural_is_valid() {
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
        if let Some(ext) = self.extensions() {
            ext.write_for_owner(writer, None, "dPt", None);
        }
        write_end_tag(writer, "c:dPt");
    }
}
