// c:backWall
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
    ShapeProperties,
    Thickness,
};
use crate::{
    reader::driver::{
        local_name,
        xml_read_loop,
    },
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct BackWall {
    thickness:        Option<Thickness>,
    shape_properties: Option<ShapeProperties>,
}

impl BackWall {
    pub(crate) fn formulas_mut(&mut self) -> Vec<&mut super::Formula> {
        self.shape_properties
            .as_mut()
            .map_or_else(Vec::new, |v| v.formulas_mut())
    }

    #[must_use]
    pub fn thickness(&self) -> Option<&Thickness> {
        self.thickness.as_ref()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use thickness()")]
    pub fn get_thickness(&self) -> Option<&Thickness> {
        self.thickness()
    }

    pub fn thickness_mut(&mut self) -> Option<&mut Thickness> {
        self.thickness.as_mut()
    }

    #[deprecated(since = "3.0.0", note = "Use thickness_mut()")]
    pub fn get_thickness_mut(&mut self) -> Option<&mut Thickness> {
        self.thickness_mut()
    }

    pub fn set_thickness(&mut self, value: Thickness) -> &mut BackWall {
        self.thickness = Some(value);
        self
    }

    #[must_use]
    pub fn shape_properties(&self) -> Option<&ShapeProperties> {
        self.shape_properties.as_ref()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use shape_properties()")]
    pub fn get_shape_properties(&self) -> Option<&ShapeProperties> {
        self.shape_properties()
    }

    pub fn shape_properties_mut(&mut self) -> Option<&mut ShapeProperties> {
        self.shape_properties.as_mut()
    }

    #[deprecated(since = "3.0.0", note = "Use shape_properties_mut()")]
    pub fn get_shape_properties_mut(&mut self) -> Option<&mut ShapeProperties> {
        self.shape_properties_mut()
    }

    pub fn set_shape_properties(&mut self, value: ShapeProperties) -> &mut Self {
        self.shape_properties = Some(value);
        self
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        owner: &BytesStart,
    ) {
        self.read_with_session(reader, owner, None);
    }

    pub(crate) fn set_attributes_with_session<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
        session: &crate::reader::xlsx::SourceCacheSession,
    ) {
        self.read_with_session(reader, e, Some(session));
    }

    fn read_with_session<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        owner: &BytesStart,
        session: Option<&crate::reader::xlsx::SourceCacheSession>,
    ) {
        let _scope_guard = session.and_then(|session| session.enter_child_scope(owner));

        xml_read_loop!(
            reader,
            ref n @ (Event::Empty(ref e) | Event::Start(ref e)) => {
                let _is_empty = matches!(n, Event::Empty(_));
                if local_name(e.name().into_inner()) == b"thickness" {
                    let mut obj = Thickness::default();
                    obj.set_attributes(reader, e);
                    self.set_thickness(obj);
                }
                if local_name(e.name().into_inner()) == b"spPr" {
                    let mut obj = ShapeProperties::default();
                    match session{Some(session)=>obj.set_attributes_with_session(reader,e,session),None=>obj.set_attributes(reader,e)}
                    self.set_shape_properties(obj);
                }
            },
            Event::End(ref e) => {
                if local_name(e.name().into_inner()) == b"backWall" {
                    return;
                }
            },
            Event::Eof => panic!("Error: Could not find {} end element", "c:backWall")
        );
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        // c:backWall
        write_start_tag(writer, "c:backWall", vec![], false);

        // c:thickness
        if let Some(v) = &self.thickness {
            v.write_to(writer);
        }

        // c:spPr
        if let Some(v) = &self.shape_properties {
            v.write_to(writer);
        }

        write_end_tag(writer, "c:backWall");
    }
}
