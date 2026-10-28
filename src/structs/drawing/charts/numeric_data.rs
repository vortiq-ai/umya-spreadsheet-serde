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
    NumberReference,
    SeriesPartConflictReason,
    XmlUnsignedInt,
};
use crate::{
    reader::driver::local_name,
    structs::Workbook,
    writer::driver::{
        write_end_tag,
        write_start_tag,
        write_text_node,
    },
};

/// An inline numeric data point. The required index remains diagnosable if
/// absent.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct NumericPoint {
    index:       XmlUnsignedInt,
    value:       Option<String>,
    format_code: Option<String>,
    invalid:     bool,
}
impl NumericPoint {
    #[must_use]
    pub fn index(&self) -> &XmlUnsignedInt {
        &self.index
    }

    pub fn index_mut(&mut self) -> &mut XmlUnsignedInt {
        &mut self.index
    }

    #[must_use]
    pub fn value(&self) -> Option<&str> {
        self.value.as_deref()
    }

    pub fn set_value(&mut self, value: Option<String>) -> &mut Self {
        self.value = value;
        self
    }

    #[must_use]
    pub fn format_code(&self) -> Option<&str> {
        self.format_code.as_deref()
    }

    pub fn set_format_code(&mut self, value: Option<String>) -> &mut Self {
        self.format_code = value;
        self
    }

    fn read<R: std::io::BufRead>(reader: &mut Reader<R>, e: &BytesStart, empty: bool) -> Self {
        Self::read_common(reader, e, empty, None)
    }

    fn read_with_session<R: std::io::BufRead>(
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

        let mut point = Self::default();
        point
            .index
            .set_lexical_attribute(crate::drawing::get_unescaped_attribute(e, b"idx"));
        point.format_code = crate::drawing::get_unescaped_attribute(e, b"formatCode");
        let complete = children(reader, e, empty, |reader, child, empty| {
            if session.is_none_or(|session| {
                session.accepts_child_namespace(child, super::extension_registry::NS_C)
            }) && local_name(child.name().into_inner()) == b"v"
            {
                point.invalid |= point.value.is_some();
                point.value = read_text(reader, child, empty);
                point.invalid |= point.value.is_none();
                true
            } else {
                false
            }
        });
        point.invalid |= !complete;
        point
    }

    fn is_valid(&self) -> bool {
        !self.invalid && self.index.val().is_some() && self.value.is_some()
    }

    fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        let Some(index) = self.index.attribute() else {
            return;
        };
        let Some(value) = &self.value else {
            return;
        };
        let mut attrs = vec![("idx", index).into()];
        if let Some(format) = &self.format_code {
            attrs.push(("formatCode", format.as_str()).into());
        }
        write_start_tag(writer, "c:pt", attrs, false);
        write_start_tag(writer, "c:v", vec![], false);
        write_text_node(writer, value);
        write_end_tag(writer, "c:v");
        write_end_tag(writer, "c:pt");
    }
}

/// Literal data is stored, unlike a numeric reference's regenerated cache.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct NumericData {
    format_code: Option<String>,
    point_count: Option<XmlUnsignedInt>,
    points:      Vec<NumericPoint>,
    invalid:     bool,
    #[cfg_attr(feature = "serde", serde(default))]
    extensions:  Option<Box<super::ExtensionList>>,
}
impl NumericData {
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

    #[must_use]
    pub fn format_code(&self) -> Option<&str> {
        self.format_code.as_deref()
    }

    pub fn set_format_code(&mut self, value: Option<String>) -> &mut Self {
        self.format_code = value;
        self
    }

    #[must_use]
    pub fn point_count(&self) -> Option<&XmlUnsignedInt> {
        self.point_count.as_ref()
    }

    pub fn set_point_count(&mut self, value: Option<XmlUnsignedInt>) -> &mut Self {
        self.point_count = value;
        self
    }

    #[must_use]
    pub fn points(&self) -> &[NumericPoint] {
        &self.points
    }

    pub fn points_mut(&mut self) -> &mut Vec<NumericPoint> {
        &mut self.points
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
        let mut data = Self::default();
        let complete = children(reader, e, empty, |reader, child, empty| {
            match if session.is_none_or(|session| {
                session.accepts_child_namespace(child, super::extension_registry::NS_C)
            }) {
                local_name(child.name().into_inner())
            } else {
                b""
            } {
                b"extLst" if session.is_some() => {
                    data.set_extensions(Some(super::ExtensionList::read(
                        reader,
                        child,
                        empty,
                        session.expect("checked session"),
                    )));
                    true
                }
                b"formatCode" => {
                    data.invalid |= data.format_code.is_some();
                    data.format_code = read_text(reader, child, empty);
                    data.invalid |= data.format_code.is_none();
                    true
                }
                b"ptCount" => {
                    data.invalid |= data.point_count.is_some();
                    data.point_count = Some(XmlUnsignedInt::read(child));
                    false
                }
                b"pt" => {
                    data.points.push(match session {
                        Some(session) => {
                            NumericPoint::read_with_session(reader, child, empty, session)
                        }
                        None => NumericPoint::read(reader, child, empty),
                    });
                    true
                }
                _ => false,
            }
        });
        data.invalid |= !complete;
        data
    }

    #[must_use]
    pub fn conflict(&self) -> Option<SeriesPartConflictReason> {
        (!self.is_valid()).then_some(SeriesPartConflictReason::InvalidValue)
    }

    pub(crate) fn is_valid(&self) -> bool {
        !self.invalid
            && self.point_count.as_ref().is_none_or(|v| v.val().is_some())
            && self.points.iter().all(NumericPoint::is_valid)
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        if !self.is_valid() {
            return;
        }
        write_start_tag(writer, "c:numLit", vec![], false);
        if let Some(value) = &self.format_code {
            write_start_tag(writer, "c:formatCode", vec![], false);
            write_text_node(writer, value);
            write_end_tag(writer, "c:formatCode");
        }
        if let Some(count) = &self.point_count {
            write_start_tag(
                writer,
                "c:ptCount",
                count
                    .attribute()
                    .map(|v| vec![("val", v).into()])
                    .unwrap_or_default(),
                true,
            );
        }
        for point in &self.points {
            point.write_to(writer);
        }
        if let Some(ext) = self.extensions() {
            ext.write_for_owner(writer, None, "numLit", None);
        }
        write_end_tag(writer, "c:numLit");
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub enum NumericDataSource {
    Reference(Box<NumberReference>),
    Literal(Box<NumericData>),
}
impl NumericDataSource {
    #[must_use]
    pub fn literal(&self) -> Option<&NumericData> {
        match self {
            Self::Literal(value) => Some(value),
            Self::Reference(_) => None,
        }
    }

    #[must_use]
    pub fn reference(&self) -> Option<&NumberReference> {
        match self {
            Self::Reference(value) => Some(value),
            Self::Literal(_) => None,
        }
    }

    pub fn reference_mut(&mut self) -> Option<&mut NumberReference> {
        match self {
            Self::Reference(value) => Some(value),
            Self::Literal(_) => None,
        }
    }

    pub fn literal_mut(&mut self) -> Option<&mut NumericData> {
        match self {
            Self::Literal(value) => Some(value),
            Self::Reference(_) => None,
        }
    }

    pub(crate) fn is_valid(&self) -> bool {
        match self {
            Self::Reference(_) => true,
            Self::Literal(value) => value.is_valid(),
        }
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>, workbook: &Workbook) {
        match self {
            Self::Reference(value) => value.write_to(writer, workbook),
            Self::Literal(value) => value.write_to(writer),
        }
    }
}

/// Visit only direct children and stop at the exact owning `QName`. Unknown
/// start elements are consumed as whole subtrees, never exposing grandchildren.
/// A callback returns true when it consumed the child's end itself.
pub(crate) fn children<R: std::io::BufRead>(
    reader: &mut Reader<R>,
    owner: &BytesStart,
    empty: bool,
    mut visit: impl FnMut(&mut Reader<R>, &BytesStart, bool) -> bool,
) -> bool {
    if empty {
        return true;
    }
    let end = owner.name().into_inner().to_vec();
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(ref child)) => {
                visit(reader, child, true);
            }
            Ok(Event::Start(ref child)) => {
                if !visit(reader, child, false) && !skip(reader) {
                    return false;
                }
            }
            Ok(Event::End(ref child)) if child.name().into_inner() == end => return true,
            Ok(Event::Eof) | Err(_) => return false,
            _ => (),
        }
        buf.clear();
    }
}
fn skip<R: std::io::BufRead>(reader: &mut Reader<R>) -> bool {
    let mut depth = 1usize;
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(_)) => depth += 1,
            Ok(Event::End(_)) => {
                depth -= 1;
                if depth == 0 {
                    return true;
                }
            }
            Ok(Event::Eof) | Err(_) => return false,
            _ => (),
        }
        buf.clear();
    }
}
pub(crate) use crate::reader::driver::read_simple_text as read_text;
