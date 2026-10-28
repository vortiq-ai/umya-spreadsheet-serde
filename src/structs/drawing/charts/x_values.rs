// c:xVal
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use super::NumberReference;
use crate::{
    reader::driver::local_name,
    structs::Workbook,
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct XValues {
    number_reference: NumberReference,
}

impl XValues {

    #[must_use]
    pub fn number_reference(&self) -> &NumberReference {
        &self.number_reference
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use number_reference()")]
    pub fn get_number_reference(&self) -> &NumberReference {
        self.number_reference()
    }

    pub fn number_reference_mut(&mut self) -> &mut NumberReference {
        &mut self.number_reference
    }

    #[deprecated(since = "3.0.0", note = "Use number_reference_mut()")]
    pub fn get_number_reference_mut(&mut self) -> &mut NumberReference {
        self.number_reference_mut()
    }

    pub fn set_number_reference(&mut self, value: NumberReference) -> &mut XValues {
        self.number_reference = value;
        self
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        owner: &BytesStart,
    ) {
        self.read_common(reader, owner, None);
    }

    pub(crate) fn set_attributes_with_session<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
        session: &crate::reader::xlsx::SourceCacheSession,
    ) {
        self.read_common(reader, e, Some(session));
    }

    fn read_common<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
        session: Option<&crate::reader::xlsx::SourceCacheSession>,
    ) {
        let _scope_guard = session.and_then(|session| session.enter_child_scope(e));
        let complete = super::numeric_data::children(reader, e, false, |reader, child, empty| {
            if local_name(child.name().into_inner()) != b"numRef" {
                return false;
            }
            if !empty {
                match session {
                    Some(session) => self
                        .number_reference
                        .set_attributes_with_session(reader, child, session),
                    None => self.number_reference.set_attributes(reader, child),
                }
            }
            true
        });
        assert!(complete, "Could not read numeric data source");
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>, wb: &Workbook) {
        // c:xVal
        write_start_tag(writer, "c:xVal", vec![], false);

        // c:numRef
        self.number_reference.write_to(writer, wb);

        write_end_tag(writer, "c:xVal");
    }
}
