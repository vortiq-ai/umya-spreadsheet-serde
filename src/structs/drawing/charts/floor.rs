// c:marker
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
    extension_registry::NS_C,
};
use crate::{
    reader::driver::{
        local_name,
        skip_element,
        try_xml_read_loop,
    },
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct Floor {
    thickness:        Option<Thickness>,
    shape_properties: Option<Box<ShapeProperties>>,
}

impl Floor {
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

    pub fn set_thickness(&mut self, value: Thickness) -> &mut Floor {
        self.thickness = Some(value);
        self
    }

    #[must_use]
    pub fn shape_properties(&self) -> Option<&ShapeProperties> {
        self.shape_properties.as_deref()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use shape_properties()")]
    pub fn get_shape_properties(&self) -> Option<&ShapeProperties> {
        self.shape_properties()
    }

    pub fn shape_properties_mut(&mut self) -> Option<&mut ShapeProperties> {
        self.shape_properties.as_deref_mut()
    }

    #[deprecated(since = "3.0.0", note = "Use shape_properties_mut()")]
    pub fn get_shape_properties_mut(&mut self) -> Option<&mut ShapeProperties> {
        self.shape_properties_mut()
    }

    pub fn set_shape_properties(&mut self, value: ShapeProperties) -> &mut Self {
        self.shape_properties = Some(Box::new(value));
        self
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        owner: &BytesStart,
    ) -> Result<(), crate::XlsxError> {
        self.read_with_session(reader, owner, None)?;
        Ok(())
    }

    pub(crate) fn set_attributes_with_session<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
        session: &crate::reader::xlsx::SourceCacheSession,
    ) -> Result<(), crate::XlsxError> {
        self.read_with_session(reader, e, Some(session))?;
        Ok(())
    }

    fn read_with_session<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        owner: &BytesStart,
        session: Option<&crate::reader::xlsx::SourceCacheSession>,
    ) -> Result<(), crate::XlsxError> {
        let _scope_guard = session
            .map(|session| session.enter_child_scope(owner))
            .transpose()?
            .flatten();
        let end = owner.name().into_inner().to_vec();

        try_xml_read_loop!(
            reader,
            ref event @ (Event::Empty(ref e) | Event::Start(ref e)) => {
                let empty = matches!(event, Event::Empty(_));
                let native = match session {
                    Some(session) => session.accepts_child_namespace(e, NS_C)?,
                    None => true,
                };
                match if native { local_name(e.name().into_inner()) } else { b"" } {
                    b"thickness" => {
                        let mut obj = Thickness::default();
                        obj.set_attributes(reader, e)?;
                        self.set_thickness(obj);
                        if !empty { skip_element(reader, e)?; }
                    },
                    b"spPr" => {
                        let mut obj = ShapeProperties::default();
                        if !empty {
                            match session {
                                Some(session) => {
                                    obj.set_attributes_with_session(reader, e, session)?;
                                }
                                None => obj.set_attributes(reader, e)?,
                            }
                        }
                        self.set_shape_properties(obj);
                    },
                    _ => { if !empty { skip_element(reader, e)?; } },
                }
            },
            Event::End(ref e) => {
                if e.name().into_inner() == end.as_slice() {
                    return Ok(());
                }
            },
            Event::Eof => return Err(crate::xml::xml_error(format!("Error: Could not find {} end element", "c:floor")))
        );
    }

    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
    ) -> Result<(), crate::XlsxError> {
        // c:floor
        write_start_tag(writer, "c:floor", vec![], false);

        // c:thickness
        if let Some(v) = &self.thickness {
            v.write_to(writer);
        }

        // c:spPr
        if let Some(v) = &self.shape_properties {
            v.write_to(writer)?;
        }

        write_end_tag(writer, "c:floor");

        Ok(())
    }
}
