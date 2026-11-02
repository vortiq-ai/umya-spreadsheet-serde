// c:xVal
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use super::{
    CategoryAxisData,
    NumberReference,
    NumericData,
    SeriesPartConflictReason,
    numeric_data::write_numeric_source,
};
use crate::structs::Workbook;

/// `c:xVal`, a `CT_AxDataSource`: a numeric reference, an array constant such
/// as `={1,2,3}`, or text (a string reference, string literal or multi-level
/// reference, as Excel writes for a scatter chart with text X values). The text
/// source, then the numeric literal, take precedence over the reference while
/// they are present.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct XValues {
    number_reference: NumberReference,
    #[cfg_attr(feature = "serde", serde(default))]
    number_literal:   Option<NumericData>,
    #[cfg_attr(feature = "serde", serde(default))]
    text_source:      Option<CategoryAxisData>,
}

impl XValues {
    super::numeric_data::numeric_literal_api!(text_source);

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

    /// Makes `value` the source, replacing any literal or text source.
    pub fn set_number_reference(&mut self, value: NumberReference) -> &mut XValues {
        self.number_reference = value;
        self.number_literal = None;
        self.text_source = None;
        self
    }

    /// The text source (`c:strRef`, `c:strLit` or `c:multiLvlStrRef`), when
    /// the series holds one instead of numbers.
    #[must_use]
    pub fn text_source(&self) -> Option<&CategoryAxisData> {
        self.text_source.as_ref()
    }

    pub fn text_source_mut(&mut self) -> Option<&mut CategoryAxisData> {
        self.text_source.as_mut()
    }

    /// Makes the text source the one written; the numeric sources are not
    /// written while it is present.
    pub fn set_text_source(&mut self, value: CategoryAxisData) -> &mut XValues {
        self.text_source = Some(value);
        self
    }

    pub(crate) fn conflict(&self) -> Option<SeriesPartConflictReason> {
        match &self.text_source {
            Some(source) => source.conflict(),
            None => self.number_literal.as_ref().and_then(NumericData::conflict),
        }
    }

    pub(crate) fn has_text_source(&self) -> bool {
        self.text_source.is_some()
    }

    /// Whether the numeric reference is not the source here.
    pub(crate) fn is_literal(&self) -> bool {
        self.number_literal.is_some() || self.text_source.is_some()
    }

    /// The text source as role traversal sees it, whether or not it is the
    /// written source.
    pub(crate) fn text_source_for_roles(&self) -> Option<&CategoryAxisData> {
        self.text_source.as_ref()
    }

    pub(crate) fn text_source_for_roles_mut(&mut self) -> Option<&mut CategoryAxisData> {
        self.text_source.as_mut()
    }

    pub(crate) fn set_attributes_with_session<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
        session: &crate::reader::xlsx::SourceCacheSession,
    ) -> Result<(), crate::XlsxError> {
        // The element is a `CT_AxDataSource`: read it as one, then keep a
        // numeric choice in the numeric fields and a text choice whole.
        let mut source = CategoryAxisData::default();
        source.set_attributes(reader, e, session)?;
        if source.holds_text() {
            self.text_source = Some(source);
        } else {
            match source.take_numeric() {
                (Some(reference), None) => self.number_reference = reference,
                (None, Some(literal)) => self.number_literal = Some(literal),
                (None, None) => (),
                (Some(_), Some(_)) => {
                    return Err(crate::xml::xml_error("Could not read numeric data source"));
                }
            }
        }
        Ok(())
    }

    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        wb: &Workbook,
    ) -> Result<(), crate::XlsxError> {
        match &self.text_source {
            Some(source) => source.write_to_as(writer, wb, "c:xVal"),
            None => write_numeric_source(
                writer,
                wb,
                "c:xVal",
                "x values",
                &self.number_reference,
                self.number_literal.as_ref(),
            ),
        }
    }
}
