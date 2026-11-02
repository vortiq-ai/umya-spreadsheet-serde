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
    #[cfg_attr(feature = "serde", serde(default))]
    extensions:      Option<Box<super::ExtensionList>>,
    #[cfg_attr(feature = "serde", serde(default))]
    pivot_cache:     Option<Box<PivotNumericCache>>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
struct PivotNumericCache {
    formula: String,
    data:    super::NumericData,
}

impl NumberReference {
    pub(crate) fn retarget_pivot_cache(&mut self, formula: &str) {
        let before = self.formula.address_str();
        if let Some(cache) = self
            .pivot_cache
            .as_mut()
            .filter(|cache| cache.formula == before)
        {
            formula.clone_into(&mut cache.formula);
        }
    }

    pub(crate) fn clear_pivot_cache(&mut self) {
        self.pivot_cache = None;
    }

    pub(crate) fn pivot_cache(&self) -> Option<&super::NumericData> {
        self.pivot_cache
            .as_deref()
            .filter(|cache| cache.formula == self.formula.address_str())
            .map(|cache| &cache.data)
    }

    pub(crate) fn restore_export_parts(&mut self, previous: &Self) -> usize {
        let cache = previous
            .pivot_cache
            .as_ref()
            .filter(|cache| cache.formula == self.formula.address_str());
        let restored =
            super::chart_metadata_restoration::replace_hidden_parts(&mut self.pivot_cache, cache);
        let previous = if self.formula.address_str() == previous.formula.address_str() {
            previous.extensions.as_ref()
        } else {
            None
        };
        restored
            + super::chart_metadata_restoration::replace_hidden_parts(
                &mut self.extensions,
                previous,
            )
    }

    // A retaining Formula projection must consume these disjoint fields once.
    pub(crate) fn split_formula_extensions_mut(
        &mut self,
    ) -> (&mut Formula, Option<&mut super::ExtensionList>) {
        (&mut self.formula, self.extensions.as_deref_mut())
    }

    #[must_use]
    pub fn extensions(&self) -> Option<&super::ExtensionList> {
        self.extensions.as_deref()
    }

    pub(crate) fn projection_extensions_mut(&mut self) -> Option<&mut super::ExtensionList> {
        self.extensions = None;
        self.clear_pivot_cache();
        None
    }

    pub fn extensions_mut(&mut self) -> Option<&mut super::ExtensionList> {
        self.extensions.as_deref_mut()
    }

    pub fn set_extensions(&mut self, value: Option<super::ExtensionList>) -> &mut Self {
        self.extensions = value.map(Box::new);
        self
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
    ) -> Result<(), crate::XlsxError> {
        // Existing callers keep source format-code handling. The policy is
        // explicit only for new roles; no new numeric cache storage is added.
        let (parsed, _) =
            self.read_status(reader, e, false, NumericSourceCachePolicy::Read, None)?;
        if !(parsed) {
            return Err(crate::xml::xml_error("Could not read numeric reference"));
        }
        Ok(())
    }

    pub(crate) fn read_with_policy<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
        empty: bool,
        policy: NumericSourceCachePolicy,
    ) -> Result<bool, crate::XlsxError> {
        let (parsed, valid) = self.read_status(reader, e, empty, policy, None)?;
        Ok(parsed && valid)
    }

    pub(crate) fn set_attributes_with_session<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
        session: &crate::reader::xlsx::SourceCacheSession,
    ) -> Result<(), crate::XlsxError> {
        let (parsed, _) = self.read_status(
            reader,
            e,
            false,
            NumericSourceCachePolicy::Read,
            Some(session),
        )?;
        if !(parsed) {
            return Err(crate::xml::xml_error("Could not read numeric reference"));
        }
        Ok(())
    }

    pub(crate) fn read_with_policy_session<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
        empty: bool,
        policy: NumericSourceCachePolicy,
        session: &crate::reader::xlsx::SourceCacheSession,
    ) -> Result<bool, crate::XlsxError> {
        let (parsed, valid) = self.read_status(reader, e, empty, policy, Some(session))?;
        Ok(parsed && valid)
    }

    // Parsing completion is distinct from formula-count validity. The default
    // reader preserves complete missing/duplicate formula behavior; policy
    // readers also report whether exactly one formula was present.
    fn read_status<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
        empty: bool,
        policy: NumericSourceCachePolicy,
        session: Option<&crate::reader::xlsx::SourceCacheSession>,
    ) -> Result<(bool, bool), crate::XlsxError> {
        let _scope_guard = session
            .map(|session| session.enter_child_scope(e))
            .transpose()?
            .flatten();
        let mut formulas = 0usize;
        let mut valid = true;
        let mut pivot_data = None;
        let complete = children(
            reader,
            e,
            empty,
            |reader, child, empty| -> Result<bool, crate::XlsxError> {
                Ok({
                    match if match session {
                        Some(session) => session
                            .accepts_child_namespace(child, super::extension_registry::NS_C)?,
                        None => true,
                    } {
                        local_name(child.name().into_inner())
                    } else {
                        b""
                    } {
                        b"extLst" if session.is_some() => {
                            self.set_extensions(Some(super::ExtensionList::read(
                                reader,
                                child,
                                empty,
                                session.ok_or_else(|| {
                                    crate::xml::xml_error("missing chart session")
                                })?,
                            )?));
                            true
                        }
                        b"f" => {
                            formulas += 1;
                            if empty {
                                self.formula.set_address_str("");
                            } else {
                                valid &= self.formula.try_set_attributes(reader, child)?;
                            }
                            true
                        }
                        b"numCache" if matches!(policy, NumericSourceCachePolicy::Read) => {
                            if let Some(session) =
                                session.filter(|session| session.is_pivot_chart())
                            {
                                if pivot_data.is_some() {
                                    return Err(crate::xml::xml_error(
                                        "pivot numeric reference has repeated caches",
                                    ));
                                }
                                let data = super::NumericData::read_with_session(
                                    reader, child, empty, session,
                                )?;
                                if !data.is_valid() {
                                    return Err(crate::xml::xml_error(
                                        "pivot numeric reference has an invalid cache",
                                    ));
                                }
                                if let Some(format) = data.format_code() {
                                    self.numbering_cache.format_code_mut().set_text(format);
                                }
                                pivot_data = Some(data);
                            } else if !empty {
                                self.numbering_cache.set_attributes(reader, child)?;
                            }
                            true
                        }
                        // SkipSourceCache consumes the entire source cache through the
                        // shared direct-child loop. Descendants cannot become formulas.
                        _ => false,
                    }
                })
            },
        )?;
        self.pivot_cache = pivot_data.map(|data| {
            Box::new(PivotNumericCache {
                formula: self.formula.address_str(),
                data,
            })
        });
        Ok((complete && valid, formulas == 1))
    }

    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        wb: &Workbook,
    ) -> Result<(), crate::XlsxError> {
        // c:numRef
        write_start_tag(writer, "c:numRef", vec![], false);

        // c:f
        self.formula.write_to(writer);

        // c:numCache
        if let Some(cache) = self.pivot_cache() {
            cache.write_to_as(writer, "c:numCache")?;
        } else {
            self.numbering_cache
                .write_to(writer, self.formula().address(), wb);
        }

        if let Some(ext) = self.extensions() {
            ext.write_for_owner(writer, Some(wb), "numRef", None)?;
        }
        write_end_tag(writer, "c:numRef");

        Ok(())
    }
}

#[cfg(test)]
#[path = "number_reference_tests.rs"]
mod tests;
