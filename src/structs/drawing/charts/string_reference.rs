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
    #[cfg_attr(feature = "serde", serde(default))]
    extensions:   Option<Box<super::ExtensionList>>,
}

impl StringReference {
    pub(crate) fn formulas_mut(&mut self) -> Vec<&mut Formula> {
        let mut result = Vec::new();
        result.push(&mut self.formula);
        if let Some(value) = &mut self.extensions {
            result.extend(value.formulas_mut());
        }
        result
    }

    #[must_use]
    pub fn extensions(&self) -> Option<&super::ExtensionList> {
        self.extensions.as_deref()
    }

    pub fn extensions_mut(&mut self) -> Option<&mut super::ExtensionList> {
        self.extensions.as_deref_mut()
    }

    pub fn set_extensions(&mut self, value: Option<super::ExtensionList>) -> &mut Self {
        self.extensions = value.map(Box::new);
        self
    }

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
        e: &BytesStart,
        session: &SourceCacheSession,
    ) {
        self.read_named(reader, e, session, b"strCache");
    }

    /// Reads a real reference-shaped owner from the original chart reader.
    /// The cache wrapper differs in the registered label extension vocabulary,
    /// but admission, descriptors and deferred source spans have one owner.
    pub(crate) fn read_named<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
        session: &SourceCacheSession,
        cache_name: &[u8],
    ) {
        self.read_named_with(reader, e, session, cache_name, |_, _, _, _| false);
    }

    /// Returns direct formula/cache occurrence counts. Extension owners retain
    /// invalid cardinality themselves; this is the single source-cache parser.
    pub(crate) fn read_named_with<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
        session: &SourceCacheSession,
        cache_name: &[u8],
        mut extra: impl FnMut(&mut Reader<R>, &BytesStart, bool, (usize, usize)) -> bool,
    ) -> (usize, usize, bool) {
        let _scope_guard = session.enter_child_scope(e);
        let namespace = if cache_name == b"strCache" {
            super::extension_registry::NS_C
        } else {
            super::extension_registry::NS_C15
        };
        let end = e.name().into_inner().to_vec();
        let mut counts = (0, 0);
        let mut ordered = true;
        let mut buffer = Vec::new();
        loop {
            buffer.clear();
            let start = reader.buffer_position();
            match reader.read_event_into(&mut buffer).unwrap() {
                Event::Start(e) => match if session.accepts_child_namespace(&e, namespace) {
                    local_name(e.name().into_inner())
                } else {
                    b""
                } {
                    b"extLst" if cache_name == b"strCache" => {
                        self.set_extensions(Some(super::ExtensionList::read(
                            reader, &e, false, session,
                        )));
                    }
                    b"f" => {
                        ordered &= counts.1 == 0;
                        counts.0 += 1;
                        self.formula.set_attributes(reader, &e);
                    }
                    name if name == cache_name => {
                        ordered &= counts.0 > 0;
                        counts.1 += 1;
                        if session.is_deferred() {
                            let summary;
                            (self.cached_value, self.string_cache, summary) =
                                StringCache::read_deferred_named(
                                    reader,
                                    self.formula.address_str(),
                                    e.name().into_inner(),
                                    true,
                                );
                            register_parsed_string_cache(
                                session,
                                &mut self.string_cache,
                                Some((start, reader.buffer_position())),
                                summary,
                            );
                        } else {
                            (self.cached_value, self.string_cache) =
                                StringCache::set_attributes(reader, &e, self.formula.address_str());
                            register_parsed_string_cache(
                                session,
                                &mut self.string_cache,
                                None,
                                None,
                            );
                        }
                    }
                    _ => {
                        if !extra(reader, &e, false, counts) {
                            crate::reader::driver::skip_element(reader, &e);
                        }
                    }
                },
                Event::Empty(e) => match if session.accepts_child_namespace(&e, namespace) {
                    local_name(e.name().into_inner())
                } else {
                    b""
                } {
                    b"extLst" if cache_name == b"strCache" => {
                        self.set_extensions(Some(super::ExtensionList::read(
                            reader, &e, true, session,
                        )));
                    }
                    b"f" => {
                        ordered &= counts.1 == 0;
                        counts.0 += 1;
                        self.formula = Formula::default();
                    }
                    name if name == cache_name => {
                        ordered &= counts.0 > 0;
                        counts.1 += 1;
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
                    _ => {
                        extra(reader, &e, true, counts);
                    }
                },
                Event::End(e) if e.name().into_inner() == end.as_slice() => {
                    finalize_parsed_string_cache(
                        session,
                        &mut self.string_cache,
                        self.formula.address_str(),
                    );
                    return (counts.0, counts.1, ordered);
                }
                Event::Eof => panic!("unterminated string reference owner"),
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

        if let Some(ext) = self.extensions() {
            ext.write_for_owner(writer, Some(wb), "strRef", None);
        }
        write_end_tag(writer, "c:strRef");
    }
}
