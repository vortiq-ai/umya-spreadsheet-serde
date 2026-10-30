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

use super::{
    ShapeProperties,
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
    ) -> Result<(), crate::XlsxError> {
        self.read_with_session(reader, owner, empty_flg, None)?;
        Ok(())
    }

    pub(crate) fn set_attributes_with_session<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
        empty_flg: bool,
        session: &crate::reader::xlsx::SourceCacheSession,
    ) -> Result<(), crate::XlsxError> {
        self.read_with_session(reader, e, empty_flg, Some(session))?;
        Ok(())
    }

    fn read_with_session<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        owner: &BytesStart,
        empty_flg: bool,
        session: Option<&crate::reader::xlsx::SourceCacheSession>,
    ) -> Result<(), crate::XlsxError> {
        let _scope_guard = session
            .map(|session| session.enter_child_scope(owner))
            .transpose()?
            .flatten();
        let end = owner.name().into_inner().to_vec();

        if empty_flg {
            return Ok(());
        }

        try_xml_read_loop!(
            reader,
            ref event @ (Event::Start(ref e) | Event::Empty(ref e)) => {
                let empty = matches!(event, Event::Empty(_));
                let native = match session {
                    Some(session) => session.accepts_child_namespace(e, NS_C)?,
                    None => true,
                };
                if native && local_name(e.name().into_inner()) == b"spPr" {
                    let mut obj = ShapeProperties::default();
                    if !empty {
                        match session {
                            Some(session) => obj.set_attributes_with_session(reader, e, session)?,
                            None => obj.set_attributes(reader, e)?,
                        }
                    }
                    self.set_shape_properties(obj);
                } else if !empty {
                    skip_element(reader, e)?;
                }
            },
            Event::End(ref e) => {
                if e.name().into_inner() == end.as_slice() {
                    return Ok(())
                }
            },
            Event::Eof => return Err(crate::xml::xml_error(format!("Error: Could not find {} end element", "c:majorGridlines")))
        );
    }

    fn with_include(&self) -> bool {
        self.shape_properties.is_some()
    }

    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
    ) -> Result<(), crate::XlsxError> {
        if !self.with_include() {
            // c:majorGridlines
            write_start_tag(writer, "c:majorGridlines", vec![], true);
            return Ok(());
        }
        // c:majorGridlines
        write_start_tag(writer, "c:majorGridlines", vec![], false);

        // c:spPr
        if let Some(v) = &self.shape_properties {
            v.write_to(writer)?;
        }

        write_end_tag(writer, "c:majorGridlines");

        Ok(())
    }
}
