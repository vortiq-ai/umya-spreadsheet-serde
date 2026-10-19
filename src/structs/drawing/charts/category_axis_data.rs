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
    StringLiteral,
    StringReference,
};
use crate::{
    drawing::charts::NumberReference,
    reader::driver::{
        local_name,
        xml_read_loop,
    },
    structs::Workbook,
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

#[derive(Clone, Default, Debug)]
pub struct CategoryAxisData {
    string_reference:             Option<StringReference>,
    string_literal:               Option<StringLiteral>,
    number_reference:             Option<NumberReference>,
    multi_level_string_reference: Option<MultiLevelStringReference>,
}

#[cfg(feature = "serde")]
mod serde_impl {
    use serde::{
        Deserialize,
        Deserializer,
        Serialize,
        Serializer,
        ser::SerializeStruct,
    };

    use super::{
        CategoryAxisData,
        MultiLevelStringReference,
        NumberReference,
        StringLiteral,
        StringReference,
    };

    // The browser's binary chart model predates multi-level references. Keep
    // its exact field count and order while retaining native data in JSON.
    #[derive(Deserialize)]
    #[serde(rename = "CategoryAxisData")]
    struct Legacy {
        string_reference: Option<StringReference>,
        string_literal:   Option<StringLiteral>,
        number_reference: Option<NumberReference>,
    }

    #[derive(Deserialize)]
    #[serde(rename = "CategoryAxisData")]
    struct HumanReadable {
        #[serde(flatten)]
        legacy:                       Legacy,
        #[serde(default)]
        multi_level_string_reference: Option<MultiLevelStringReference>,
    }

    impl Serialize for CategoryAxisData {
        fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            let human_readable = serializer.is_human_readable();
            let mut state = serializer
                .serialize_struct("CategoryAxisData", if human_readable { 4 } else { 3 })?;
            state.serialize_field("string_reference", &self.string_reference)?;
            state.serialize_field("string_literal", &self.string_literal)?;
            state.serialize_field("number_reference", &self.number_reference)?;
            if human_readable {
                state.serialize_field(
                    "multi_level_string_reference",
                    &self.multi_level_string_reference,
                )?;
            }
            state.end()
        }
    }

    impl<'de> Deserialize<'de> for CategoryAxisData {
        fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            let (legacy, multi_level_string_reference) = if deserializer.is_human_readable() {
                let value = HumanReadable::deserialize(deserializer)?;
                (value.legacy, value.multi_level_string_reference)
            } else {
                (Legacy::deserialize(deserializer)?, None)
            };
            Ok(Self {
                string_reference: legacy.string_reference,
                string_literal: legacy.string_literal,
                number_reference: legacy.number_reference,
                multi_level_string_reference,
            })
        }
    }
}

impl CategoryAxisData {
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
        _e: &BytesStart,
    ) {
        xml_read_loop!(
            reader,
            Event::Start(ref e) => {
                match local_name(e.name().into_inner()) {
                    b"multiLvlStrRef" => {
                        let mut obj = MultiLevelStringReference::default();
                        obj.set_attributes(reader, e);
                        self.set_multi_level_string_reference(obj);
                    }
                    b"strRef" => {
                        let mut obj = StringReference::default();
                        obj.set_attributes(reader, e);
                        self.set_string_reference(obj);
                    }
                    b"strLit" => {
                        let mut obj = StringLiteral::default();
                        obj.set_attributes(reader, e);
                        self.set_string_literal(obj);
                    }
                    b"numRef" => {
                        let mut obj = NumberReference::default();
                        obj.set_attributes(reader, e);
                        self.set_number_reference(obj);
                    }
                    _ => (),
                }
            },
            Event::End(ref e) => {
                if local_name(e.name().into_inner()) == b"cat" {
                    return;
                }
            },
            Event::Eof => panic!("Error: Could not find {} end element", "c:cat")
        );
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>, wb: &Workbook) {
        // c:cat
        write_start_tag(writer, "c:cat", vec![], false);

        // Keep hierarchy and sparse point indices in their native
        // representation.
        if let Some(v) = &self.multi_level_string_reference {
            v.write_to(writer);
        }

        // c:strRef
        if let Some(v) = &self.string_reference {
            v.write_to(writer, wb);
        }

        // c:strLit
        if let Some(v) = &self.string_literal {
            v.write_to(writer);
        }

        // c:numRef
        if let Some(v) = &self.number_reference {
            v.write_to(writer, wb);
        }

        write_end_tag(writer, "c:cat");
    }
}
