// c:bandFmt
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use super::{
    Index,
    ShapeProperties,
    group_lines::store_once,
    numeric_data::children,
};
use crate::{
    reader::{
        driver::local_name,
        xlsx::SourceCacheSession,
    },
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

/// `CT_BandFmt`: the formatting of one value band of a surface group.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct BandFormat {
    index:            Index,
    shape_properties: Option<ShapeProperties>,
}

impl BandFormat {
    #[must_use]
    pub fn index(&self) -> &Index {
        &self.index
    }

    pub fn index_mut(&mut self) -> &mut Index {
        &mut self.index
    }

    #[must_use]
    pub fn shape_properties(&self) -> Option<&ShapeProperties> {
        self.shape_properties.as_ref()
    }

    pub fn shape_properties_mut(&mut self) -> Option<&mut ShapeProperties> {
        self.shape_properties.as_mut()
    }

    pub fn set_shape_properties(&mut self, value: Option<ShapeProperties>) -> &mut Self {
        self.shape_properties = value;
        self
    }

    pub(crate) fn read<R: std::io::BufRead>(
        reader: &mut Reader<R>,
        e: &BytesStart,
        empty: bool,
        session: &SourceCacheSession,
    ) -> Result<Self, crate::XlsxError> {
        let _scope_guard = session.enter_child_scope(e)?;
        let mut result = Self::default();
        let mut index = None;
        children(
            reader,
            e,
            empty,
            |reader, e, empty| -> Result<bool, crate::XlsxError> {
                if !session.accepts_child_namespace(e, super::extension_registry::NS_C)? {
                    return Ok(false);
                }
                match local_name(e.name().into_inner()) {
                    b"idx" => {
                        let mut value = Index::default();
                        value.set_attributes(reader, e)?;
                        if !empty {
                            crate::reader::driver::skip_element(reader, e)?;
                        }
                        store_once(&mut index, value, "c:idx")?;
                    }
                    b"spPr" => {
                        let mut shape = ShapeProperties::default();
                        if !empty {
                            shape.set_attributes_with_session(reader, e, session)?;
                        }
                        store_once(&mut result.shape_properties, shape, "c:spPr")?;
                    }
                    _ => return Ok(false),
                }
                Ok(true)
            },
        )?;
        result.index =
            index.ok_or_else(|| crate::xml::xml_error("c:bandFmt requires a c:idx child"))?;
        Ok(result)
    }

    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
    ) -> Result<(), crate::XlsxError> {
        write_start_tag(writer, "c:bandFmt", vec![], false);
        self.index.write_to(writer);
        if let Some(shape) = &self.shape_properties {
            shape.write_to(writer)?;
        }
        write_end_tag(writer, "c:bandFmt");
        Ok(())
    }
}
