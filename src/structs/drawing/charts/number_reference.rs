use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

// c:numRef
use super::Formula;
use super::numeric_data::children;

#[derive(Clone, Copy)]
pub(crate) enum NumericSourceCachePolicy {
    Read,
    SkipSourceCache,
}
use super::NumberingCache;
use crate::{
    reader::driver::local_name,
    structs::Workbook,
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct NumberReference {
    formula:         Formula,
    numbering_cache: NumberingCache,
}

impl NumberReference {
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

    pub fn set_formula(&mut self, value: Formula) -> &mut NumberReference {
        self.formula = value;
        self
    }

    #[must_use]
    pub fn numbering_cache(&self) -> &NumberingCache {
        &self.numbering_cache
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use numbering_cache()")]
    pub fn get_numbering_cache(&self) -> &NumberingCache {
        self.numbering_cache()
    }

    pub fn numbering_cache_mut(&mut self) -> &mut NumberingCache {
        &mut self.numbering_cache
    }

    #[deprecated(since = "3.0.0", note = "Use numbering_cache_mut()")]
    pub fn get_numbering_cache_mut(&mut self) -> &mut NumberingCache {
        self.numbering_cache_mut()
    }

    pub fn set_numbering_cache(&mut self, value: NumberingCache) -> &mut NumberReference {
        self.numbering_cache = value;
        self
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
    ) {
        // Existing callers keep source format-code handling. The policy is
        // explicit only for new roles; no new numeric cache storage is added.
        let (parsed, _) = self.read_status(reader, e, false, NumericSourceCachePolicy::Read);
        assert!(parsed, "Could not read numeric reference");
    }

    pub(crate) fn read_with_policy<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
        empty: bool,
        policy: NumericSourceCachePolicy,
    ) -> bool {
        let (parsed, valid) = self.read_status(reader, e, empty, policy);
        parsed && valid
    }

    // Parsing completion is distinct from new-role schema validity. Historical
    // readers panic on malformed XML, but keep their complete missing/duplicate
    // formula behavior rather than acquiring a new validation policy.
    fn read_status<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
        empty: bool,
        policy: NumericSourceCachePolicy,
    ) -> (bool, bool) {
        let mut formulas = 0usize;
        let mut valid = true;
        let complete = children(reader, e, empty, |reader, child, empty| {
            match local_name(child.name().into_inner()) {
                b"f" => {
                    formulas += 1;
                    if empty {
                        self.formula.set_address_str("");
                    } else {
                        valid &= self.formula.try_set_attributes(reader, child);
                    }
                    true
                }
                b"numCache" if matches!(policy, NumericSourceCachePolicy::Read) => {
                    if !empty {
                        self.numbering_cache.set_attributes(reader, child);
                    }
                    true
                }
                // SkipSourceCache consumes the entire source cache through the
                // shared direct-child loop. Descendants cannot become formulas.
                _ => false,
            }
        });
        (complete && valid, formulas == 1)
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>, wb: &Workbook) {
        // c:numRef
        write_start_tag(writer, "c:numRef", vec![], false);

        // c:f
        self.formula.write_to(writer);

        // c:numCache
        self.numbering_cache
            .write_to(writer, self.formula().address(), wb);

        write_end_tag(writer, "c:numRef");
    }
}

#[cfg(test)]
#[path = "number_reference_tests.rs"]
mod tests;
