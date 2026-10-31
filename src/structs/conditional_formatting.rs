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
    BooleanValue,
    ConditionalFormattingRule,
    DifferentialFormats,
    SequenceOfReferences,
    package_extension::PreservedExtension,
    preserved_ranges::{
        LocalEdit,
        remove_lines,
    },
};
use crate::{
    XlsxError,
    drawing::charts::{
        NamespaceScope,
        OpaqueElement,
    },
    reader::driver::{
        get_attribute,
        set_string_from_xml,
        try_xml_read_loop,
    },
    traits::AdjustmentCoordinate,
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Default, Debug, Clone)]
pub struct ConditionalFormatting {
    sequence_of_references: SequenceOfReferences,
    conditional_collection: Vec<ConditionalFormattingRule>,
    pivot:                  BooleanValue,
    extensions:             Vec<PreservedExtension>,
}

impl ConditionalFormatting {
    #[inline]
    #[must_use]
    pub fn sequence_of_references(&self) -> &SequenceOfReferences {
        &self.sequence_of_references
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use sequence_of_references()")]
    pub fn get_sequence_of_references(&self) -> &SequenceOfReferences {
        self.sequence_of_references()
    }

    #[inline]
    pub fn sequence_of_references_mut(&mut self) -> &mut SequenceOfReferences {
        &mut self.sequence_of_references
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use sequence_of_references_mut()")]
    pub fn get_sequence_of_references_mut(&mut self) -> &mut SequenceOfReferences {
        self.sequence_of_references_mut()
    }

    #[inline]
    pub fn set_sequence_of_references(&mut self, value: SequenceOfReferences) -> &mut Self {
        self.sequence_of_references = value;
        self
    }

    #[inline]
    #[must_use]
    pub fn conditional_collection(&self) -> &[ConditionalFormattingRule] {
        &self.conditional_collection
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use conditional_collection()")]
    pub fn get_conditional_collection(&self) -> &[ConditionalFormattingRule] {
        self.conditional_collection()
    }

    #[inline]
    pub fn conditional_collection_mut(&mut self) -> &mut Vec<ConditionalFormattingRule> {
        &mut self.conditional_collection
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use conditional_collection_mut()")]
    pub fn get_conditional_collection_mut(&mut self) -> &mut Vec<ConditionalFormattingRule> {
        self.conditional_collection_mut()
    }

    #[inline]
    pub fn set_conditional_collection(
        &mut self,
        value: impl Into<Vec<ConditionalFormattingRule>>,
    ) -> &mut Self {
        self.conditional_collection = value.into();
        self
    }

    #[inline]
    pub fn add_conditional_collection(&mut self, value: ConditionalFormattingRule) -> &mut Self {
        self.conditional_collection.push(value);
        self
    }

    /// Whether the formatting applies to a pivot table (`pivot` attribute).
    #[inline]
    #[must_use]
    pub fn pivot(&self) -> bool {
        self.pivot.value()
    }

    #[inline]
    pub fn set_pivot(&mut self, value: bool) -> &mut Self {
        self.pivot.set_value(value);
        self
    }

    /// The `ext` entries of the `extLst`, kept as read.
    pub fn extensions(&self) -> impl Iterator<Item = &OpaqueElement> {
        self.extensions.iter().map(|entry| &entry.element)
    }

    /// The preserved extensions of the rules, then of the format itself, in
    /// the order they are written.
    pub(crate) fn extension_entries(&self) -> impl Iterator<Item = &PreservedExtension> {
        self.conditional_collection
            .iter()
            .flat_map(ConditionalFormattingRule::extension_entries)
            .chain(&self.extensions)
    }

    pub(crate) fn extension_entries_mut(
        &mut self,
    ) -> impl Iterator<Item = &mut PreservedExtension> {
        self.conditional_collection
            .iter_mut()
            .flat_map(ConditionalFormattingRule::extension_entries_mut)
            .chain(&mut self.extensions)
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
        differential_formats: &DifferentialFormats,
    ) -> Result<(), XlsxError> {
        if let Some(v) = get_attribute(e, b"sqref") {
            self.sequence_of_references.set_sqref(v);
        }
        set_string_from_xml!(self, e, pivot, "pivot");

        try_xml_read_loop!(
            reader,
            ref n @ (Event::Empty(ref e) | Event::Start(ref e))
                if crate::reader::spreadsheet_xml::is_native_start(e, b"ext") => {
                let is_empty = matches!(n, Event::Empty(_));
                self.extensions.push(PreservedExtension::new(
                    OpaqueElement::read(reader, e, is_empty, &NamespaceScope::default())
                        .map_err(crate::xml::xml_error)?,
                ));
            },
            ref n @ (Event::Empty(ref e) | Event::Start(ref e)) => {
                let is_empty = matches!(n, Event::Empty(_));
                if e.name().into_inner() == b"cfRule" {
                    let mut obj = ConditionalFormattingRule::default();
                    obj.set_attributes(reader, e, differential_formats, is_empty)?;
                    self.conditional_collection.push(obj);
                }
            },
            Event::End(ref e) => {
                if e.name().into_inner() == b"conditionalFormatting" {
                    return Ok(())
                }
            },
            Event::Eof => return Err(crate::xml::xml_error("conditionalFormatting has no end element"))
        );
    }

    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        differential_formats: &mut DifferentialFormats,
        relationship_ids: &mut dyn Iterator<Item = Vec<String>>,
    ) -> Result<(), XlsxError> {
        let is_inner = !self.conditional_collection.is_empty() || !self.extensions.is_empty();

        // conditionalFormatting
        let mut attributes: crate::structs::AttrCollection = Vec::new();

        if self.pivot.has_value() {
            attributes.push(("pivot", self.pivot.value_string()).into());
        }
        let sequence_of_references = &self.sequence_of_references.get_sqref();
        attributes.push(("sqref", sequence_of_references).into());

        write_start_tag(writer, "conditionalFormatting", attributes, !is_inner);

        if is_inner {
            // cfRule
            for v in &self.conditional_collection {
                v.write_to(writer, differential_formats, relationship_ids)?;
            }

            // extLst
            if !self.extensions.is_empty() {
                write_start_tag(writer, "extLst", vec![], false);
                for extension in &self.extensions {
                    extension.write_rebound(writer, relationship_ids)?;
                }
                write_end_tag(writer, "extLst");
            }

            write_end_tag(writer, "conditionalFormatting");
        }
        Ok(())
    }
}
impl AdjustmentCoordinate for ConditionalFormatting {
    #[inline]
    fn adjustment_insert_coordinate(
        &mut self,
        root_col_num: u32,
        offset_col_num: u32,
        root_row_num: u32,
        offset_row_num: u32,
    ) {
        self.sequence_of_references.adjustment_insert_coordinate(
            root_col_num,
            offset_col_num,
            root_row_num,
            offset_row_num,
        );
    }

    #[inline]
    fn adjustment_remove_coordinate(
        &mut self,
        root_col_num: u32,
        offset_col_num: u32,
        root_row_num: u32,
        offset_row_num: u32,
    ) {
        remove_lines(
            &mut self.sequence_of_references,
            &LocalEdit {
                insert:    false,
                root_col:  root_col_num,
                col_count: offset_col_num,
                root_row:  root_row_num,
                row_count: offset_row_num,
            },
        );
    }

    #[inline]
    fn is_remove_coordinate(
        &self,
        root_col_num: u32,
        offset_col_num: u32,
        root_row_num: u32,
        offset_row_num: u32,
    ) -> bool {
        let mut remaining = self.sequence_of_references.clone();
        remove_lines(
            &mut remaining,
            &LocalEdit {
                insert:    false,
                root_col:  root_col_num,
                col_count: offset_col_num,
                root_row:  root_row_num,
                row_count: offset_row_num,
            },
        )
    }
}
