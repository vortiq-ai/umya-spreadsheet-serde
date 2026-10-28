//! Label extension references share ordinary string-cache admission and replay.
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use super::{
    SeriesPartConflictReason,
    StringReference,
    numeric_data::{
        children,
        read_text,
    },
};
use crate::{
    reader::{
        driver::local_name,
        xlsx::SourceCacheSession,
    },
    structs::Workbook,
    writer::driver::{
        write_end_tag,
        write_start_tag,
        write_text_node,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub enum ReferenceValidation {
    #[default]
    Valid,
    Invalid,
    TooMany,
}
impl ReferenceValidation {
    fn conflict(&self) -> Option<SeriesPartConflictReason> {
        match self {
            Self::Valid => None,
            Self::Invalid => Some(SeriesPartConflictReason::InvalidValue),
            Self::TooMany => Some(SeriesPartConflictReason::TooMany),
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct SeriesDataLabelsRange {
    reference:       StringReference,
    cache_present:   bool,
    formula_present: bool,
    validation:      ReferenceValidation,
}
impl SeriesDataLabelsRange {
    pub(crate) fn reference_for_roles_mut(&mut self) -> &mut StringReference {
        &mut self.reference
    }

    pub(crate) fn formulas_mut(&mut self) -> Vec<&mut super::Formula> {
        let mut result = Vec::new();
        result.extend(self.reference.formulas_mut());
        result
    }

    #[must_use]
    pub fn reference(&self) -> &StringReference {
        &self.reference
    }

    pub fn reference_mut(&mut self) -> &mut StringReference {
        self.formula_present = true;
        &mut self.reference
    }

    pub fn set_reference(&mut self, reference: StringReference) -> &mut Self {
        self.reference = reference;
        self.formula_present = true;
        self
    }

    #[must_use]
    pub fn cache_present(&self) -> bool {
        self.cache_present
    }

    pub fn set_cache_present(&mut self, present: bool) -> &mut Self {
        self.cache_present = present;
        self
    }

    #[must_use]
    pub fn conflict(&self) -> Option<SeriesPartConflictReason> {
        if self.formula_present {
            self.validation.conflict()
        } else {
            Some(SeriesPartConflictReason::MissingRequiredChild)
        }
    }

    pub(crate) fn read<R: std::io::BufRead>(
        reader: &mut Reader<R>,
        e: &BytesStart,
        empty: bool,
        session: &SourceCacheSession,
    ) -> Self {
        let mut result = Self::default();
        if !empty {
            let (formula, cache, ordered) = result.reference.read_named_with(
                reader,
                e,
                session,
                b"dlblRangeCache",
                |_, _, _, _| {
                    result.validation = ReferenceValidation::Invalid;
                    false
                },
            );
            if !ordered {
                result.validation = ReferenceValidation::Invalid;
            }
            result.formula_present = formula > 0;
            result.cache_present = cache > 0;
            if formula > 1 || cache > 1 {
                result.validation = ReferenceValidation::TooMany;
            }
        }
        result
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>, wb: &Workbook) {
        if self.conflict().is_some() {
            return;
        }
        write_start_tag(writer, "c15:datalabelsRange", vec![], false);
        write_formula(writer, self.reference.formula().address_str());
        if self.cache_present {
            self.reference.string_cache().write_to_as(
                writer,
                self.reference.formula(),
                wb,
                "c15:dlblRangeCache",
            );
        }
        write_end_tag(writer, "c15:datalabelsRange");
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct DataLabelFieldEntry {
    field_guid:      String,
    reference:       StringReference,
    cache_present:   bool,
    guid_present:    bool,
    formula_present: bool,
    validation:      ReferenceValidation,
}
impl DataLabelFieldEntry {
    /// A present empty xsd:string is distinct from a missing field identity.
    #[must_use]
    pub fn explicit_field_guid(&self) -> Option<&str> {
        self.guid_present.then_some(self.field_guid.as_str())
    }

    pub(crate) fn reference_for_roles_mut(&mut self) -> &mut StringReference {
        &mut self.reference
    }

    pub(crate) fn formulas_mut(&mut self) -> Vec<&mut super::Formula> {
        let mut result = Vec::new();
        result.extend(self.reference.formulas_mut());
        result
    }

    #[must_use]
    pub fn field_guid(&self) -> &str {
        &self.field_guid
    }

    pub fn set_field_guid(&mut self, value: impl Into<String>) -> &mut Self {
        self.field_guid = value.into();
        self.guid_present = true;
        self
    }

    #[must_use]
    pub fn reference(&self) -> &StringReference {
        &self.reference
    }

    pub fn reference_mut(&mut self) -> &mut StringReference {
        self.formula_present = true;
        &mut self.reference
    }

    pub fn set_reference(&mut self, reference: StringReference) -> &mut Self {
        self.reference = reference;
        self.formula_present = true;
        self
    }

    #[must_use]
    pub fn cache_present(&self) -> bool {
        self.cache_present
    }

    pub fn set_cache_present(&mut self, present: bool) -> &mut Self {
        self.cache_present = present;
        self
    }

    #[must_use]
    pub fn conflict(&self) -> Option<SeriesPartConflictReason> {
        if !self.guid_present || !self.formula_present {
            Some(SeriesPartConflictReason::MissingRequiredChild)
        } else {
            self.validation.conflict()
        }
    }

    pub(crate) fn read<R: std::io::BufRead>(
        reader: &mut Reader<R>,
        e: &BytesStart,
        empty: bool,
        session: &SourceCacheSession,
    ) -> Self {
        let mut result = Self::default();
        if !empty {
            let mut guids = 0;
            let (formula, cache, ordered) = result.reference.read_named_with(
                reader,
                e,
                session,
                b"dlblFieldTableCache",
                |reader, child, empty, counts| {
                    if counts.0 > 0 || counts.1 > 0 {
                        result.validation = ReferenceValidation::Invalid;
                    }
                    if !session.accepts_child_namespace(child, super::extension_registry::NS_C15)
                        || local_name(child.name().into_inner()) != b"txfldGUID"
                    {
                        result.validation = ReferenceValidation::Invalid;
                        return false;
                    }
                    guids += 1;
                    match read_text(reader, child, empty) {
                        Some(value) => {
                            result.field_guid = value;
                            result.guid_present = true;
                        }
                        None => result.validation = ReferenceValidation::Invalid,
                    }
                    true
                },
            );
            if !ordered {
                result.validation = ReferenceValidation::Invalid;
            }
            result.formula_present = formula > 0;
            result.cache_present = cache > 0;
            if formula > 1 || cache > 1 || guids > 1 {
                result.validation = ReferenceValidation::Invalid;
            }
        }
        result
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>, wb: &Workbook) {
        if self.conflict().is_some() {
            return;
        }
        write_start_tag(writer, "c15:dlblFTEntry", vec![], false);
        write_start_tag(writer, "c15:txfldGUID", vec![], false);
        write_text_node(writer, &self.field_guid);
        write_end_tag(writer, "c15:txfldGUID");
        write_formula(writer, self.reference.formula().address_str());
        if self.cache_present {
            self.reference.string_cache().write_to_as(
                writer,
                self.reference.formula(),
                wb,
                "c15:dlblFieldTableCache",
            );
        }
        write_end_tag(writer, "c15:dlblFTEntry");
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct DataLabelFieldTable {
    entries: Vec<DataLabelFieldEntry>,
    invalid: bool,
}
impl DataLabelFieldTable {
    pub(crate) fn formulas_mut(&mut self) -> Vec<&mut super::Formula> {
        let mut result = Vec::new();
        for entry in &mut self.entries {
            result.extend(entry.formulas_mut());
        }
        result
    }

    #[must_use]
    pub fn entries(&self) -> &[DataLabelFieldEntry] {
        &self.entries
    }

    pub fn entries_mut(&mut self) -> &mut Vec<DataLabelFieldEntry> {
        &mut self.entries
    }

    #[must_use]
    pub fn conflict(&self) -> Option<SeriesPartConflictReason> {
        if self.invalid {
            Some(SeriesPartConflictReason::InvalidValue)
        } else {
            self.entries.iter().find_map(DataLabelFieldEntry::conflict)
        }
    }

    pub(crate) fn read<R: std::io::BufRead>(
        reader: &mut Reader<R>,
        e: &BytesStart,
        empty: bool,
        session: &SourceCacheSession,
    ) -> Self {
        let _scope_guard = session.enter_child_scope(e);
        let mut result = Self::default();
        result.invalid = !children(reader, e, empty, |reader, child, empty| {
            if !session.accepts_child_namespace(child, super::extension_registry::NS_C15)
                || local_name(child.name().into_inner()) != b"dlblFTEntry"
            {
                return false;
            }
            result
                .entries
                .push(DataLabelFieldEntry::read(reader, child, empty, session));
            true
        });
        result
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>, wb: &Workbook) {
        if self.conflict().is_some() {
            return;
        }
        write_start_tag(writer, "c15:dlblFieldTable", vec![], false);
        for entry in &self.entries {
            entry.write_to(writer, wb);
        }
        write_end_tag(writer, "c15:dlblFieldTable");
    }
}
fn write_formula(writer: &mut Writer<Cursor<Vec<u8>>>, value: String) {
    write_start_tag(writer, "c15:f", vec![], false);
    write_text_node(writer, value);
    write_end_tag(writer, "c15:f");
}
