//! Native `CT_ChartLines`, shared by native and registered extension label
//! owners.
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use super::{
    SeriesPartConflictReason,
    ShapeProperties,
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
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartLines {
    shape_properties: Option<ShapeProperties>,
    duplicate_shape:  bool,
}
impl ChartLines {

    #[must_use]
    pub fn shape_properties(&self) -> Option<&ShapeProperties> {
        self.shape_properties.as_ref()
    }

    pub fn shape_properties_mut(&mut self) -> Option<&mut ShapeProperties> {
        self.shape_properties.as_mut()
    }

    pub fn set_shape_properties(&mut self, value: Option<ShapeProperties>) -> &mut Self {
        self.shape_properties = value;
        self.duplicate_shape = false;
        self
    }

    pub(crate) fn conflict(&self) -> Option<SeriesPartConflictReason> {
        self.duplicate_shape
            .then_some(SeriesPartConflictReason::TooMany)
    }

    pub(crate) fn read<R: std::io::BufRead>(
        reader: &mut Reader<R>,
        e: &BytesStart,
        empty: bool,
        session: &SourceCacheSession,
    ) -> Self {
        let _scope_guard = session.enter_child_scope(e);
        let mut result = Self::default();
        let complete = children(reader, e, empty, |reader, e, empty| {
            if local_name(e.name().into_inner()) != b"spPr" {
                return false;
            }
            result.duplicate_shape |= result.shape_properties.is_some();
            let mut shape = ShapeProperties::default();
            if !empty {
                shape.set_attributes_with_session(reader, e, session);
            }
            result.shape_properties = Some(shape);
            true
        });
        result.duplicate_shape |= !complete;
        result
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        self.write_to_as(writer, "c:leaderLines");
    }

    pub(crate) fn write_to_as(&self, writer: &mut Writer<Cursor<Vec<u8>>>, name: &str) {
        if self.conflict().is_some() {
            return;
        }
        write_start_tag(writer, name, vec![], false);
        if let Some(shape) = &self.shape_properties {
            shape.write_to(writer);
        }
        write_end_tag(writer, name);
    }
}
