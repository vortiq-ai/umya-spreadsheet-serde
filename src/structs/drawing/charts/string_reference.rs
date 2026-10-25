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
    reader::{
        driver::local_name,
        xlsx::{
            SourceCacheSession,
            finalize_parsed_string_cache,
            register_parsed_string_cache,
        },
    },
    structs::Workbook,
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct StringReference {
    formula:      Formula,
    string_cache: StringCache,
    #[cfg_attr(feature = "serde", serde(default))]
    cached_value: Option<String>,
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
        session: &SourceCacheSession,
    ) {
        let mut buffer = Vec::new();
        loop {
            buffer.clear();
            let start = reader.buffer_position();
            match reader.read_event_into(&mut buffer).unwrap() {
                Event::Start(e) => match local_name(e.name().into_inner()) {
                    b"f" => self.formula.set_attributes(reader, &e),
                    b"strCache" if session.is_deferred() => {
                        let summary;
                        (self.cached_value, self.string_cache, summary) =
                            StringCache::set_deferred_attributes(
                                reader,
                                self.formula.address_str(),
                            );
                        register_parsed_string_cache(
                            session,
                            &mut self.string_cache,
                            Some((start, reader.buffer_position())),
                            summary,
                        );
                    }
                    b"strCache" => {
                        (self.cached_value, self.string_cache) =
                            StringCache::set_attributes(reader, &e, self.formula.address_str());
                        register_parsed_string_cache(session, &mut self.string_cache, None, None);
                    }
                    _ => (),
                },
                Event::Empty(e) if local_name(e.name().into_inner()) == b"strCache" => {
                    self.string_cache = StringCache::empty_source(self.formula.address_str());
                    register_parsed_string_cache(
                        session,
                        &mut self.string_cache,
                        session
                            .is_deferred()
                            .then_some((start, reader.buffer_position())),
                        None,
                    );
                }
                Event::End(e) if local_name(e.name().into_inner()) == b"strRef" => {
                    finalize_parsed_string_cache(
                        session,
                        &mut self.string_cache,
                        self.formula.address_str(),
                    );
                    return;
                }
                Event::Eof => panic!("Error: Could not find {} end element", "c:strRef"),
                _ => (),
            }
        }
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>, wb: &Workbook) {
        // c:strRef
        write_start_tag(writer, "c:strRef", vec![], false);

        // c:f
        self.formula.write_to(writer);

        // c:strCache
        self.string_cache.write_to(writer, self.formula(), wb);

        write_end_tag(writer, "c:strRef");
    }
}
