//! Native `CT_AxDataSource` choices for filtered category titles.
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use super::{
    MultiLevelStringReference,
    NumberReference,
    NumericData,
    SeriesPartConflictReason,
    StringLiteral,
    StringReference,
    number_reference::NumericSourceCachePolicy,
    numeric_data::children,
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
    },
};
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub enum AxisDataSourceChoice {
    MultiLevelStringReference(Box<MultiLevelStringReference>),
    NumberReference(Box<NumberReference>),
    NumericLiteral(Box<NumericData>),
    StringReference(Box<StringReference>),
    StringLiteral(Box<StringLiteral>),
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct AxisDataSource {
    choices: Vec<AxisDataSourceChoice>,
    invalid: bool,
}
impl AxisDataSource {
    #[must_use]
    pub fn choices(&self) -> &[AxisDataSourceChoice] {
        &self.choices
    }

    pub fn choices_mut(&mut self) -> &mut Vec<AxisDataSourceChoice> {
        &mut self.choices
    }

    #[must_use]
    pub fn conflict(&self) -> Option<SeriesPartConflictReason> {
        if self.choices.is_empty() {
            return Some(SeriesPartConflictReason::MissingRequiredChild);
        }
        if self.choices.len() > 1 {
            return Some(SeriesPartConflictReason::TooMany);
        }
        if self.invalid {
            return Some(SeriesPartConflictReason::InvalidValue);
        }
        match &self.choices[0] {
            AxisDataSourceChoice::NumericLiteral(v) => v.conflict(),
            AxisDataSourceChoice::StringLiteral(v) => v.conflict(),
            _ => None,
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
            let value =
                match if session.accepts_child_namespace(child, super::extension_registry::NS_C) {
                    local_name(child.name().into_inner())
                } else {
                    b""
                } {
                    b"multiLvlStrRef" => {
                        let mut v = MultiLevelStringReference::default();
                        if !empty {
                            v.set_attributes_with_session(reader, child, session);
                        }
                        AxisDataSourceChoice::MultiLevelStringReference(Box::new(v))
                    }
                    b"numRef" => {
                        let mut v = NumberReference::default();
                        result.invalid |= !v.read_with_policy_session(
                            reader,
                            child,
                            empty,
                            NumericSourceCachePolicy::SkipSourceCache,
                            session,
                        );
                        AxisDataSourceChoice::NumberReference(Box::new(v))
                    }
                    b"numLit" => AxisDataSourceChoice::NumericLiteral(Box::new(
                        NumericData::read_with_session(reader, child, empty, session),
                    )),
                    b"strRef" => {
                        let mut v = StringReference::default();
                        if !empty {
                            v.set_attributes(reader, child, session);
                        }
                        AxisDataSourceChoice::StringReference(Box::new(v))
                    }
                    b"strLit" => AxisDataSourceChoice::StringLiteral(Box::new(
                        StringLiteral::read_with_session(reader, child, empty, session),
                    )),
                    _ => return false,
                };
            result.choices.push(value);
            true
        }) || result.invalid;
        result
    }

    pub(crate) fn write_to_as(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        wb: &Workbook,
        name: &str,
    ) {
        if self.conflict().is_some() {
            return;
        }
        write_start_tag(writer, name, vec![], false);
        for value in &self.choices {
            match value {
                AxisDataSourceChoice::MultiLevelStringReference(v) => v.write_to(writer),
                AxisDataSourceChoice::NumberReference(v) => v.write_to(writer, wb),
                AxisDataSourceChoice::NumericLiteral(v) => v.write_to(writer),
                AxisDataSourceChoice::StringReference(v) => v.write_to(writer, wb),
                AxisDataSourceChoice::StringLiteral(v) => v.write_to(writer),
            }
        }
        write_end_tag(writer, name);
    }
}
