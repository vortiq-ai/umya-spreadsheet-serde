// c:strLit
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use super::{
    SeriesPartConflictReason,
    StringPoint,
    XmlUnsignedInt,
    numeric_data::children,
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
pub struct StringLiteral {
    string_point_list: Vec<StringPoint>,
    #[cfg_attr(feature = "serde", serde(default))]
    parts:             Option<Box<StringLiteralParts>>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
struct StringLiteralParts {
    point_count: Option<XmlUnsignedInt>,
    invalid:     bool,
    extensions:  Option<Box<super::ExtensionList>>,
}
impl StringLiteral {
    pub(crate) fn set_attributes_with_session<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
        session: &crate::reader::xlsx::SourceCacheSession,
    ) {
        *self = Self::read_with_session(reader, e, false, session);
    }

    #[must_use]
    pub fn extensions(&self) -> Option<&super::ExtensionList> {
        self.parts.as_deref().and_then(|p| p.extensions.as_deref())
    }

    pub fn extensions_mut(&mut self) -> Option<&mut super::ExtensionList> {
        self.parts
            .as_deref_mut()
            .and_then(|p| p.extensions.as_deref_mut())
    }

    pub fn set_extensions(&mut self, value: Option<super::ExtensionList>) -> &mut Self {
        self.parts.get_or_insert_with(Default::default).extensions = value.map(Box::new);
        self
    }

    #[must_use]
    pub fn point_count(&self) -> Option<&XmlUnsignedInt> {
        self.parts.as_deref().and_then(|p| p.point_count.as_ref())
    }

    pub fn set_point_count(&mut self, value: Option<XmlUnsignedInt>) -> &mut Self {
        self.parts.get_or_insert_with(Default::default).point_count = value;
        self
    }

    #[must_use]
    pub fn conflict(&self) -> Option<SeriesPartConflictReason> {
        if self
            .parts
            .as_ref()
            .is_some_and(|p| p.invalid || p.point_count.as_ref().is_some_and(|v| v.val().is_none()))
        {
            Some(SeriesPartConflictReason::InvalidValue)
        } else {
            self.string_point_list
                .iter()
                .find_map(StringPoint::conflict)
        }
    }

    #[must_use]
    pub fn string_point_list(&self) -> &[StringPoint] {
        &self.string_point_list
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use string_point_list()")]
    pub fn get_string_point_list(&self) -> &[StringPoint] {
        self.string_point_list()
    }

    pub fn string_point_list_mut(&mut self) -> &mut Vec<StringPoint> {
        &mut self.string_point_list
    }

    #[deprecated(since = "3.0.0", note = "Use string_point_list_mut()")]
    pub fn get_string_point_list_mut(&mut self) -> &mut Vec<StringPoint> {
        self.string_point_list_mut()
    }

    pub fn add_string_point_list(&mut self, value: StringPoint) -> &mut Self {
        self.string_point_list.push(value);
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
        let mut parts = StringLiteralParts::default();
        let mut counts = 0;
        parts.invalid = !children(reader, e, empty, |reader, child, empty| {
            match if session.is_none_or(|session| {
                session.accepts_child_namespace(child, super::extension_registry::NS_C)
            }) {
                local_name(child.name().into_inner())
            } else {
                b""
            } {
                b"extLst" if session.is_some() => {
                    parts.extensions = Some(Box::new(super::ExtensionList::read(
                        reader,
                        child,
                        empty,
                        session.expect("checked session"),
                    )));
                }
                b"pt" => result.string_point_list.push(match session {
                    Some(session) => StringPoint::read_with_session(reader, child, empty, session),
                    None => StringPoint::read(reader, child, empty),
                }),
                b"ptCount" => {
                    counts += 1;
                    parts.point_count = Some(XmlUnsignedInt::read(child));
                    if !empty {
                        crate::reader::driver::skip_element(reader, child);
                    }
                }
                _ => return false,
            }
            true
        });
        parts.invalid |= counts > 1;
        result.parts = Some(Box::new(parts));
        result
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        if self.conflict().is_some() {
            return;
        }
        // c:strLit
        write_start_tag(writer, "c:strLit", vec![], false);

        // c:ptCount
        if let Some(parts) = &self.parts {
            if let Some(count) = &parts.point_count {
                count.write_to_as(writer, "c:ptCount");
            }
        } else {
            let count = self.string_point_list.len().to_string();
            write_start_tag(writer, "c:ptCount", vec![("val", &count).into()], true);
        }

        // c:pt
        for (index, obj) in self.string_point_list.iter().enumerate() {
            obj.write_to(writer, index.try_into().unwrap());
        }

        if let Some(ext) = self.extensions() {
            ext.write_for_owner(writer, None, "strLit", None);
        }
        write_end_tag(writer, "c:strLit");
    }
}
