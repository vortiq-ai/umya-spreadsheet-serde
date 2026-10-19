use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::{
        BytesStart,
        Event,
    },
};

// c:strRef
use super::Formula;
use super::StringCache;
use crate::{
    reader::driver::local_name,
    structs::Workbook,
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
    xml_read_loop,
};

#[derive(Clone, Default, Debug)]
pub struct StringReference {
    formula:      Formula,
    string_cache: StringCache,
    cached_value: Option<String>,
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
        Formula,
        StringCache,
        StringReference,
    };

    #[derive(Deserialize)]
    #[serde(rename = "StringReference")]
    struct Legacy {
        formula:      Formula,
        string_cache: StringCache,
    }

    #[derive(Deserialize)]
    #[serde(rename = "StringReference")]
    struct HumanReadable {
        #[serde(flatten)]
        legacy:       Legacy,
        #[serde(default)]
        cached_value: Option<String>,
    }

    impl Serialize for StringReference {
        fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            let human_readable = serializer.is_human_readable();
            let mut state = serializer
                .serialize_struct("StringReference", if human_readable { 3 } else { 2 })?;
            state.serialize_field("formula", &self.formula)?;
            state.serialize_field("string_cache", &self.string_cache)?;
            if human_readable {
                state.serialize_field("cached_value", &self.cached_value)?;
            }
            state.end()
        }
    }

    impl<'de> Deserialize<'de> for StringReference {
        fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            let (legacy, cached_value) = if deserializer.is_human_readable() {
                let value = HumanReadable::deserialize(deserializer)?;
                (value.legacy, value.cached_value)
            } else {
                (Legacy::deserialize(deserializer)?, None)
            };
            Ok(Self {
                formula: legacy.formula,
                string_cache: legacy.string_cache,
                cached_value,
            })
        }
    }
}

impl StringReference {
    /// The imported label at cache index zero, before workbook values are
    /// resolved.
    #[must_use]
    pub fn cached_value(&self) -> Option<&str> {
        self.cached_value.as_deref()
    }

    #[must_use]
    pub fn formula(&self) -> &Formula {
        &self.formula
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use formula()")]
    pub fn get_formula(&self) -> &Formula {
        self.formula()
    }

    pub fn formula_mut(&mut self) -> &mut Formula {
        &mut self.formula
    }

    #[deprecated(since = "3.0.0", note = "Use formula_mut()")]
    pub fn get_formula_mut(&mut self) -> &mut Formula {
        self.formula_mut()
    }

    pub fn set_formula(&mut self, value: Formula) -> &mut StringReference {
        self.formula = value;
        self
    }

    #[must_use]
    pub fn string_cache(&self) -> &StringCache {
        &self.string_cache
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use string_cache()")]
    pub fn get_string_cache(&self) -> &StringCache {
        self.string_cache()
    }

    pub fn string_cache_mut(&mut self) -> &mut StringCache {
        &mut self.string_cache
    }

    #[deprecated(since = "3.0.0", note = "Use string_cache_mut()")]
    pub fn get_string_cache_mut(&mut self) -> &mut StringCache {
        self.string_cache_mut()
    }

    pub fn set_string_cache(&mut self, value: StringCache) -> &mut StringReference {
        self.string_cache = value;
        self
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        _e: &BytesStart,
    ) {
        xml_read_loop!(
            reader,
            Event::Start(ref e) => match local_name(e.name().0) {
                b"f" => {
                    self.formula.set_attributes(reader, e);
                }
                b"strCache" => {
                    self.cached_value = StringCache::set_attributes(reader, e);
                }
                _ => (),
            },
            Event::End(ref e) => {
                if local_name(e.name().0) == b"strRef" {
                    return;
                }
            },
            Event::Eof => panic!("Error: Could not find {} end element", "c:strRef"),
        );
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>, wb: &Workbook) {
        // c:strRef
        write_start_tag(writer, "c:strRef", vec![], false);

        // c:f
        self.formula.write_to(writer);

        // c:strCache
        StringCache::write_to(writer, self.formula(), wb);

        write_end_tag(writer, "c:strRef");
    }
}
