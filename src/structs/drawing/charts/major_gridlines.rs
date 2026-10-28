// c:majorGridlines
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::{
        BytesStart,
        Event,
    },
};

use super::ShapeProperties;
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
pub struct MajorGridlines {
    shape_properties: Option<ShapeProperties>,
}

impl MajorGridlines {

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
        empty_flg: bool,
    ) {
        self.read_with_session(reader, owner, empty_flg, None);
    }

    pub(crate) fn set_attributes_with_session<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
        empty_flg:bool,
        session: &crate::reader::xlsx::SourceCacheSession,
    ) {
        self.read_with_session(reader, e, empty_flg, Some(session));
    }

    fn read_with_session<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        owner: &BytesStart,
        empty_flg:bool,
        session: Option<&crate::reader::xlsx::SourceCacheSession>,
    ) {
        let _scope_guard = session.and_then(|session| session.enter_child_scope(owner));

        if empty_flg {
            return;
        }

        xml_read_loop!(
            reader,
            Event::Start(ref e) => {
                if local_name(e.name().into_inner()) == b"spPr" {
                    let mut obj = ShapeProperties::default();
                    match session{Some(session)=>obj.set_attributes_with_session(reader,e,session),None=>obj.set_attributes(reader,e)}
                    self.set_shape_properties(obj);
                }
            },
            Event::End(ref e) => {
                if local_name(e.name().into_inner()) == b"majorGridlines" {
                    return
                }
            },
            Event::Eof => panic!("Error: Could not find {} end element", "c:majorGridlines")
        );
    }

    fn with_include(&self) -> bool {
        self.shape_properties.is_some()
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        if !self.with_include() {
            // c:majorGridlines
            write_start_tag(writer, "c:majorGridlines", vec![], true);
            return;
        }
        // c:majorGridlines
        write_start_tag(writer, "c:majorGridlines", vec![], false);

        // c:spPr
        if let Some(v) = &self.shape_properties {
            v.write_to(writer);
        }

        write_end_tag(writer, "c:majorGridlines");
    }
}
