use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use super::numeric_data::{
    children,
    read_text,
};
// c:pt
use super::{
    NumericValue,
    SeriesPartConflictReason,
    XmlUnsignedInt,
};
use crate::{
    reader::driver::local_name,
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct StringPoint {
    numeric_value: NumericValue,
    #[cfg_attr(feature = "serde", serde(default))]
    parts:         Option<Box<StringPointParts>>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
struct StringPointParts {
    index:         XmlUnsignedInt,
    value_present: bool,
    invalid:       bool,
}
impl StringPoint {
    #[must_use]
    pub fn index(&self) -> Option<&XmlUnsignedInt> {
        self.parts.as_deref().map(|parts| &parts.index)
    }

    pub fn set_index(&mut self, value: XmlUnsignedInt) -> &mut Self {
        self.parts
            .get_or_insert_with(|| {
                Box::new(StringPointParts {
                    value_present: true,
                    ..Default::default()
                })
            })
            .index = value;
        self
    }

    #[must_use]
    pub fn value_present(&self) -> bool {
        self.parts.as_ref().is_none_or(|p| p.value_present)
    }

    #[must_use]
    pub fn conflict(&self) -> Option<SeriesPartConflictReason> {
        self.parts.as_ref().and_then(|parts| {
            if !parts.value_present || parts.index.attribute().is_none() {
                Some(SeriesPartConflictReason::MissingRequiredChild)
            } else if parts.invalid || parts.index.val().is_none() {
                Some(SeriesPartConflictReason::InvalidValue)
            } else {
                None
            }
        })
    }

    #[must_use]
    pub fn numeric_value(&self) -> &NumericValue {
        &self.numeric_value
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use numeric_value()")]
    pub fn get_numeric_value(&self) -> &NumericValue {
        self.numeric_value()
    }

    pub fn numeric_value_mut(&mut self) -> &mut NumericValue {
        if let Some(parts) = &mut self.parts {
            parts.value_present = true;
        }
        &mut self.numeric_value
    }

    #[deprecated(since = "3.0.0", note = "Use numeric_value_mut()")]
    pub fn get_numeric_value_mut(&mut self) -> &mut NumericValue {
        self.numeric_value_mut()
    }

    pub fn set_numeric_value(&mut self, value: NumericValue) -> &mut Self {
        if let Some(parts) = &mut self.parts {
            parts.value_present = true;
        }
        self.numeric_value = value;
        self
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        owner: &BytesStart,
    ) {
        *self = Self::read(reader, owner, false);
    }

    pub(crate) fn read<R: std::io::BufRead>(
        reader: &mut Reader<R>,
        e: &BytesStart,
        empty: bool,
    ) -> Self {
        Self::read_common(reader, e, empty, None)
    }

    pub(crate) fn read_with_session<R: std::io::BufRead>(
        reader: &mut Reader<R>,
        e: &BytesStart,
        empty: bool,
        session: &crate::reader::xlsx::SourceCacheSession,
    ) -> Self {
        Self::read_common(reader, e, empty, Some(session))
    }

    fn read_common<R: std::io::BufRead>(
        reader: &mut Reader<R>,
        e: &BytesStart,
        empty: bool,
        session: Option<&crate::reader::xlsx::SourceCacheSession>,
    ) -> Self {
        let _scope_guard = session.and_then(|session| session.enter_child_scope(e));

        let mut result = Self::default();
        let mut parts = StringPointParts::default();
        parts
            .index
            .set_lexical_attribute(crate::drawing::get_unescaped_attribute(e, b"idx"));
        let mut values = 0;
        parts.invalid = !children(reader, e, empty, |reader, child, empty| {
            if !session.is_none_or(|session| {
                session.accepts_child_namespace(child, super::extension_registry::NS_C)
            }) || local_name(child.name().into_inner()) != b"v"
            {
                return false;
            }
            values += 1;
            match read_text(reader, child, empty) {
                Some(value) => {
                    result.numeric_value.set_text(value);
                    parts.value_present = true;
                }
                None => parts.invalid = true,
            }
            true
        }) || parts.invalid;
        parts.invalid |= values > 1;
        result.parts = Some(Box::new(parts));
        result
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>, index: u32) {
        if self.conflict().is_some() {
            return;
        }
        // Imported identity belongs to this point even after vector edits.
        let index_str = self
            .parts
            .as_ref()
            .and_then(|p| p.index.attribute())
            .map_or_else(|| index.to_string(), str::to_owned);
        write_start_tag(writer, "c:pt", vec![("idx", &index_str).into()], false);

        // c:v
        self.numeric_value.write_to(writer);

        write_end_tag(writer, "c:pt");
    }
}
