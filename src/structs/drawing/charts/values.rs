use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

// c:val
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
pub struct Values {
    number_reference: NumberReference,
    #[cfg_attr(feature = "serde", serde(default))]
    number_literal:   Option<Box<super::NumericData>>,
}

impl Values {
    pub(crate) fn conflict(&self) -> Option<super::SeriesPartConflictReason> {
        self.number_literal().and_then(super::NumericData::conflict)
    }

    #[must_use]
    pub fn number_literal(&self) -> Option<&super::NumericData> {
        self.number_literal.as_deref()
    }

    pub fn number_literal_mut(&mut self) -> Option<&mut super::NumericData> {
        self.number_literal.as_deref_mut()
    }

    pub fn remove_number_literal(&mut self) -> &mut Self {
        self.number_literal = None;
        self
    }

    pub fn set_number_literal(&mut self, value: super::NumericData) -> &mut Self {
        self.number_reference = NumberReference::default();
        self.number_literal = Some(Box::new(value));
        self
    }

    pub(crate) fn reference_role_parts(
        &self,
    ) -> (Option<&NumberReference>, Option<&super::NumericData>) {
        match self.number_literal() {
            Some(value) => (None, Some(value)),
            None => (Some(&self.number_reference), None),
        }
    }

    pub(crate) fn projection_parts_mut(
        &mut self,
    ) -> (
        Option<&mut NumberReference>,
        Option<&mut super::NumericData>,
    ) {
        if let Some(literal) = self.number_literal.as_deref_mut() {
            literal.projection_extensions_mut();
        }
        self.reference_role_parts_mut()
    }

    pub(crate) fn reference_role_parts_mut(
        &mut self,
    ) -> (
        Option<&mut NumberReference>,
        Option<&mut super::NumericData>,
    ) {
        match self.number_literal.as_deref_mut() {
            Some(value) => {
                self.number_reference = NumberReference::default();
                (None, Some(value))
            }
            None => (Some(&mut self.number_reference), None),
        }
    }

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
        self.number_literal = None;
        &mut self.number_reference
    }

    #[deprecated(since = "3.0.0", note = "Use number_reference_mut()")]
    pub fn get_number_reference_mut(&mut self) -> &mut NumberReference {
        self.number_reference_mut()
    }

    pub fn set_number_reference(&mut self, value: NumberReference) -> &mut Values {
        self.number_reference = value;
        self.number_literal = None;
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
        let mut has_source = false;
        let complete = super::numeric_data::children(
            reader,
            e,
            false,
            |reader, child, empty| -> Result<bool, crate::XlsxError> {
                Ok({
                    let native = match session {
                        Some(session) => session
                            .accepts_child_namespace(child, super::extension_registry::NS_C)?,
                        None => true,
                    };
                    let child_name = child.name();
                    let name = crate::reader::driver::local_name(child_name.as_ref());
                    if !native || !matches!(name, b"numLit" | b"numRef") {
                        return Ok(false);
                    }
                    if has_source {
                        return Err(crate::xml::xml_error(
                            "chart values have multiple numeric data sources",
                        ));
                    }
                    has_source = true;
                    if name == b"numLit" {
                        self.set_number_literal(match session {
                            Some(session) => super::NumericData::read_with_session(
                                reader, child, empty, session,
                            )?,
                            None => super::NumericData::read(reader, child, empty)?,
                        });
                        return Ok(true);
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
        if !complete {
            return Err(crate::xml::xml_error("Could not read numeric data source"));
        }
        Ok(())
    }

    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        wb: &Workbook,
    ) -> Result<(), crate::XlsxError> {
        if self.conflict().is_some() {
            return Err(crate::xml::xml_error(
                "chart values contain an invalid numeric literal",
            ));
        }
        // c:val
        write_start_tag(writer, "c:val", vec![], false);

        // c:numRef
        if let Some(value) = self.number_literal() {
            value.write_to(writer)?;
        } else {
            self.number_reference.write_to(writer, wb)?;
        }

        write_end_tag(writer, "c:val");

        Ok(())
    }
}
