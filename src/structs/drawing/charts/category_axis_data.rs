// c:cat
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
    MultiLevelStringReference,
    NumericData,
    SeriesPartConflictReason,
    StringLiteral,
    StringReference,
};
use crate::{
    drawing::charts::NumberReference,
    reader::driver::{
        local_name,
        skip_element,
        try_xml_read_loop,
    },
    structs::Workbook,
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct CategoryAxisData {
    string_reference:             Option<StringReference>,
    string_literal:               Option<StringLiteral>,
    number_reference:             Option<NumberReference>,
    #[cfg_attr(feature = "serde", serde(default))]
    multi_level_string_reference: Option<MultiLevelStringReference>,
    #[cfg_attr(feature = "serde", serde(default))]
    number_literal:               Option<NumericData>,
}

/// The disjoint stored choices of a category source: string reference, number
/// reference, multi-level reference, string literal and number literal.
pub(crate) type CategoryParts<'a> = (
    Option<&'a StringReference>,
    Option<&'a NumberReference>,
    Option<&'a MultiLevelStringReference>,
    Option<&'a StringLiteral>,
    Option<&'a NumericData>,
);
pub(crate) type CategoryPartsMut<'a> = (
    Option<&'a mut StringReference>,
    Option<&'a mut NumberReference>,
    Option<&'a mut MultiLevelStringReference>,
    Option<&'a mut StringLiteral>,
    Option<&'a mut NumericData>,
);

#[cfg(test)]
mod category_writer_tests {
    use super::*;

    #[test]
    fn invalid_category_literals_omit_the_entire_category_wrapper() {
        for xml in [
            "<c:strLit><c:ptCount val=\"1\"/><c:pt idx=\"0\"/></c:strLit>",
            "<c:strLit><c:pt idx=\"x\"><c:v>label</c:v></c:pt></c:strLit>",
            "<c:strLit><c:ptCount val=\"1\"/><c:ptCount val=\"1\"/></c:strLit>",
        ] {
            let mut reader = Reader::from_str(xml);
            let Event::Start(start) = reader.read_event().unwrap() else {
                panic!("literal start");
            };
            let mut categories = CategoryAxisData::default();
            categories.set_string_literal(StringLiteral::read(&mut reader, &start, false).unwrap());
            let mut writer = Writer::new(Cursor::new(Vec::new()));
            categories
                .write_to(&mut writer, &Workbook::default())
                .unwrap();
            assert!(writer.into_inner().into_inner().is_empty());
        }
    }

    #[test]
    fn empty_category_choice_omits_the_entire_category_wrapper() {
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        CategoryAxisData::default()
            .write_to(&mut writer, &Workbook::default())
            .unwrap();
        assert!(writer.into_inner().into_inner().is_empty());
    }
}

impl CategoryAxisData {
    pub(crate) fn conflict(&self) -> Option<SeriesPartConflictReason> {
        let choices = [
            self.string_reference.is_some(),
            self.string_literal.is_some(),
            self.number_reference.is_some(),
            self.multi_level_string_reference.is_some(),
            self.number_literal.is_some(),
        ]
        .into_iter()
        .filter(|present| *present)
        .count();
        match choices {
            0 => Some(SeriesPartConflictReason::MissingRequiredChild),
            1 => self
                .string_literal
                .as_ref()
                .and_then(StringLiteral::conflict)
                .or_else(|| self.number_literal.as_ref().and_then(NumericData::conflict)),
            _ => Some(SeriesPartConflictReason::ConflictingChoice),
        }
    }

    // Disjoint stored children; preserves absent/invalid source states.
    pub(crate) fn reference_role_parts(&self) -> CategoryParts<'_> {
        (
            self.string_reference.as_ref(),
            self.number_reference.as_ref(),
            self.multi_level_string_reference.as_ref(),
            self.string_literal.as_ref(),
            self.number_literal.as_ref(),
        )
    }

    pub(crate) fn projection_parts_mut(&mut self) -> CategoryPartsMut<'_> {
        self.reference_role_parts_mut()
    }

    pub(crate) fn reference_role_parts_mut(&mut self) -> CategoryPartsMut<'_> {
        (
            self.string_reference.as_mut(),
            self.number_reference.as_mut(),
            self.multi_level_string_reference.as_mut(),
            self.string_literal.as_mut(),
            self.number_literal.as_mut(),
        )
    }

    #[must_use]
    pub fn multi_level_string_reference(&self) -> Option<&MultiLevelStringReference> {
        self.multi_level_string_reference.as_ref()
    }

    pub fn multi_level_string_reference_mut(&mut self) -> Option<&mut MultiLevelStringReference> {
        self.multi_level_string_reference.as_mut()
    }

    pub fn set_multi_level_string_reference(
        &mut self,
        value: MultiLevelStringReference,
    ) -> &mut Self {
        self.multi_level_string_reference = Some(value);
        self
    }

    pub fn remove_multi_level_string_reference(&mut self) -> &mut Self {
        self.multi_level_string_reference = None;
        self
    }

    #[must_use]
    pub fn string_reference(&self) -> Option<&StringReference> {
        self.string_reference.as_ref()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use string_reference()")]
    pub fn get_string_reference(&self) -> Option<&StringReference> {
        self.string_reference()
    }

    pub fn string_reference_mut(&mut self) -> Option<&mut StringReference> {
        self.string_reference.as_mut()
    }

    #[deprecated(since = "3.0.0", note = "Use get_string_reference_mut()")]
    pub fn get_string_reference_mut(&mut self) -> Option<&mut StringReference> {
        self.string_reference_mut()
    }

    pub fn set_string_reference(&mut self, value: StringReference) -> &mut Self {
        self.string_reference = Some(value);
        self
    }

    pub fn remove_string_reference(&mut self) -> &mut Self {
        self.string_reference = None;
        self
    }

    #[must_use]
    pub fn string_literal(&self) -> Option<&StringLiteral> {
        self.string_literal.as_ref()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use string_literal()")]
    pub fn get_string_literal(&self) -> Option<&StringLiteral> {
        self.string_literal()
    }

    pub fn string_literal_mut(&mut self) -> Option<&mut StringLiteral> {
        self.string_literal.as_mut()
    }

    #[deprecated(since = "3.0.0", note = "Use string_literal_mut()")]
    pub fn get_string_literal_mut(&mut self) -> Option<&mut StringLiteral> {
        self.string_literal_mut()
    }

    pub fn set_string_literal(&mut self, value: StringLiteral) -> &mut Self {
        self.string_literal = Some(value);
        self
    }

    pub fn remove_string_literal(&mut self) -> &mut Self {
        self.string_literal = None;
        self
    }

    /// The array-constant numeric categories, such as `={1,2,3}`.
    #[must_use]
    pub fn number_literal(&self) -> Option<&NumericData> {
        self.number_literal.as_ref()
    }

    pub fn number_literal_mut(&mut self) -> Option<&mut NumericData> {
        self.number_literal.as_mut()
    }

    pub fn set_number_literal(&mut self, value: NumericData) -> &mut Self {
        self.number_literal = Some(value);
        self
    }

    pub fn remove_number_literal(&mut self) -> &mut Self {
        self.number_literal = None;
        self
    }

    #[must_use]
    pub fn number_reference(&self) -> Option<&NumberReference> {
        self.number_reference.as_ref()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use number_reference()")]
    pub fn get_number_reference(&self) -> Option<&NumberReference> {
        self.number_reference()
    }

    pub fn number_reference_mut(&mut self) -> Option<&mut NumberReference> {
        self.number_reference.as_mut()
    }

    #[deprecated(since = "3.0.0", note = "Use number_reference_mut()")]
    pub fn get_number_reference_mut(&mut self) -> Option<&mut NumberReference> {
        self.number_reference_mut()
    }

    pub fn set_number_reference(&mut self, value: NumberReference) -> &mut Self {
        self.number_reference = Some(value);
        self
    }

    pub fn remove_number_reference(&mut self) -> &mut Self {
        self.number_reference = None;
        self
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        owner: &BytesStart,
        session: &crate::reader::xlsx::SourceCacheSession,
    ) -> Result<(), crate::XlsxError> {
        let _scope_guard = session.enter_child_scope(owner)?;
        let end = owner.name().into_inner().to_vec();
        try_xml_read_loop!(
            reader,
            Event::Start(ref e) => {
                match if session.accepts_child_namespace(e,super::extension_registry::NS_C)? {local_name(e.name().into_inner())} else {b""} {
                    b"multiLvlStrRef" => {
                        let mut obj = MultiLevelStringReference::default();
                        obj.set_attributes_with_session(reader, e, session)?;
                        self.set_multi_level_string_reference(obj);
                    }
                    b"strRef" => {
                        let mut obj = StringReference::default();
                        obj.set_attributes(reader, e, session)?;
                        self.set_string_reference(obj);
                    }
                    b"strLit" => {
                        let mut obj = StringLiteral::default();
                        obj.set_attributes_with_session(reader, e, session)?;
                        self.set_string_literal(obj);
                    }
                    b"numRef" => {
                        let mut obj = NumberReference::default();
                        obj.set_attributes_with_session(reader, e, session)?;
                        self.set_number_reference(obj);
                    }
                    b"numLit" => {
                        self.set_number_literal(NumericData::read_with_session(
                            reader, e, false, session,
                        )?);
                    }
                    _ => skip_element(reader, e)?,
                }
            },
            Event::Empty(ref e) => {
                if session.accepts_child_namespace(e, super::extension_registry::NS_C)? {
                    match local_name(e.name().into_inner()) {
                        b"strLit" => {
                            let literal =
                                StringLiteral::read_with_session(reader, e, true, session)?;
                            self.set_string_literal(literal);
                        }
                        b"numLit" => {
                            self.set_number_literal(NumericData::read_with_session(
                                reader, e, true, session,
                            )?);
                        }
                        _ => (),
                    }
                }
            },
            Event::End(ref e) => {
                if e.name().into_inner() == end.as_slice() {
                    return Ok(());
                }
            },
            Event::Eof => return Err(crate::xml::xml_error(format!("Error: Could not find {} end element", "c:cat")))
        );
    }

    /// Whether a text choice (string reference, string literal or multi-level
    /// reference) is stored.
    pub(crate) fn holds_text(&self) -> bool {
        self.string_reference.is_some()
            || self.string_literal.is_some()
            || self.multi_level_string_reference.is_some()
    }

    /// Moves the numeric reference and numeric literal out.
    pub(crate) fn take_numeric(&mut self) -> (Option<NumberReference>, Option<NumericData>) {
        (self.number_reference.take(), self.number_literal.take())
    }

    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        wb: &Workbook,
    ) -> Result<(), crate::XlsxError> {
        self.write_to_as(writer, wb, "c:cat")
    }

    pub(crate) fn write_to_as(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        wb: &Workbook,
        name: &str,
    ) -> Result<(), crate::XlsxError> {
        if self.conflict().is_some() {
            return Ok(());
        }
        // c:cat or c:xVal
        write_start_tag(writer, name, vec![], false);

        // Keep hierarchy and sparse point indices in their native
        // representation.
        if let Some(v) = &self.multi_level_string_reference {
            v.write_to(writer)?;
        }

        // c:strRef
        if let Some(v) = &self.string_reference {
            v.write_to(writer, wb)?;
        }

        // c:strLit
        if let Some(v) = &self.string_literal {
            v.write_to(writer)?;
        }

        // c:numRef
        if let Some(v) = &self.number_reference {
            v.write_to(writer, wb)?;
        }

        // c:numLit
        if let Some(v) = &self.number_literal {
            v.write_to(writer)?;
        }

        write_end_tag(writer, name);

        Ok(())
    }
}
