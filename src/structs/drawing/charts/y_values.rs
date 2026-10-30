// c:yVal
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use super::NumberReference;
use crate::{
    structs::Workbook,
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct YValues {
    number_reference: NumberReference,
}

impl YValues {
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

    pub fn set_number_reference(&mut self, value: NumberReference) -> &mut YValues {
        self.number_reference = value;
        self
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        owner: &BytesStart,
    ) -> Result<(), crate::XlsxError> {
        self.read_common(reader, owner, None)?;
        Ok(())
    }

    pub(crate) fn set_attributes_with_session<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
        session: &crate::reader::xlsx::SourceCacheSession,
    ) -> Result<(), crate::XlsxError> {
        self.read_common(reader, e, Some(session))?;
        Ok(())
    }

    fn read_common<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
        session: Option<&crate::reader::xlsx::SourceCacheSession>,
    ) -> Result<(), crate::XlsxError> {
        let _scope_guard = session
            .map(|session| session.enter_child_scope(e))
            .transpose()?
            .flatten();
        let complete = super::numeric_data::children(
            reader,
            e,
            false,
            |reader, child, empty| -> Result<bool, crate::XlsxError> {
                Ok({
                    if !super::numeric_data::is_native_numeric_reference(child, session)? {
                        return Ok(false);
                    }
                    if !empty {
                        match session {
                            Some(session) => self
                                .number_reference
                                .set_attributes_with_session(reader, child, session)?,
                            None => self.number_reference.set_attributes(reader, child)?,
                        }
                    }
                    true
                })
            },
        )?;
        if !(complete) {
            return Err(crate::xml::xml_error("Could not read numeric data source"));
        }
        Ok(())
    }

    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        wb: &Workbook,
    ) -> Result<(), crate::XlsxError> {
        // c:yVal
        write_start_tag(writer, "c:yVal", vec![], false);

        // c:numRef
        self.number_reference.write_to(writer, wb)?;

        write_end_tag(writer, "c:yVal");

        Ok(())
    }
}
