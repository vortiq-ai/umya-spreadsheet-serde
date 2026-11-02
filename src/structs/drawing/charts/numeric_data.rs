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

    fn read<R: std::io::BufRead>(
        reader: &mut Reader<R>,
        e: &BytesStart,
        empty: bool,
    ) -> Result<Self, crate::XlsxError> {
        Self::read_common(reader, e, empty, None)
    }

    fn read_with_session<R: std::io::BufRead>(
        reader: &mut Reader<R>,
        e: &BytesStart,
        empty: bool,
        session: &crate::reader::xlsx::SourceCacheSession,
    ) -> Result<Self, crate::XlsxError> {
        Self::read_common(reader, e, empty, Some(session))
    }

    fn read_common<R: std::io::BufRead>(
        reader: &mut Reader<R>,
        e: &BytesStart,
        empty: bool,
        session: Option<&crate::reader::xlsx::SourceCacheSession>,
    ) -> Result<Self, crate::XlsxError> {
        let _scope_guard = session
            .map(|session| session.enter_child_scope(e))
            .transpose()?
            .flatten();
        let mut point = Self::default();
        point
            .index
            .set_lexical_attribute(crate::reader::driver::read_attribute(e, b"idx")?);
        point.format_code = crate::reader::driver::read_attribute(e, b"formatCode")?;
        children(reader, e, empty, |reader, child, empty| {
            let native = match session {
                Some(session) => {
                    session.accepts_child_namespace(child, super::extension_registry::NS_C)?
                }
                None => true,
            };
            if native && local_name(child.name().as_ref()) == b"v" {
                point.invalid |= point.value.is_some();
                point.value = read_text(reader, child, empty)?;
                point.invalid |= point.value.is_none();
                Ok(true)
            } else {
                Ok(false)
            }
        })?;
        Ok(point)
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

    pub(crate) fn projection_extensions_mut(&mut self) -> Option<&mut super::ExtensionList> {
        self.extensions = None;
        None
    }

    pub(crate) fn restore_extensions(&mut self, previous: Option<&Self>) -> usize {
        super::chart_metadata_restoration::replace_hidden_parts(
            &mut self.extensions,
            previous.and_then(|value| value.extensions.as_ref()),
        )
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
    ) -> Result<Self, crate::XlsxError> {
        Self::read_common(reader, e, empty, None)
    }

    pub(crate) fn read_with_session<R: std::io::BufRead>(
        reader: &mut Reader<R>,
        e: &BytesStart,
        empty: bool,
        session: &crate::reader::xlsx::SourceCacheSession,
    ) -> Result<Self, crate::XlsxError> {
        Self::read_common(reader, e, empty, Some(session))
    }

    fn read_common<R: std::io::BufRead>(
        reader: &mut Reader<R>,
        e: &BytesStart,
        empty: bool,
        session: Option<&crate::reader::xlsx::SourceCacheSession>,
    ) -> Result<Self, crate::XlsxError> {
        let _scope_guard = session
            .map(|session| session.enter_child_scope(e))
            .transpose()?
            .flatten();
        let mut data = Self::default();
        children(reader, e, empty, |reader, child, empty| {
            let native = match session {
                Some(session) => {
                    session.accepts_child_namespace(child, super::extension_registry::NS_C)?
                }
                None => true,
            };
            if !native {
                return Ok(false);
            }
            match local_name(child.name().as_ref()) {
                b"extLst" if session.is_some() => {
                    let session = session
                        .ok_or_else(|| crate::xml::xml_error("missing extension session"))?;
                    data.set_extensions(Some(super::ExtensionList::read(
                        reader, child, empty, session,
                    )?));
                    Ok(true)
                }
                b"formatCode" => {
                    data.invalid |= data.format_code.is_some();
                    data.format_code = read_text(reader, child, empty)?;
                    data.invalid |= data.format_code.is_none();
                    Ok(true)
                }
                b"ptCount" => {
                    data.invalid |= data.point_count.is_some();
                    data.point_count = Some(XmlUnsignedInt::read(child));
                    Ok(false)
                }
                b"pt" => {
                    data.points.push(match session {
                        Some(session) => {
                            NumericPoint::read_with_session(reader, child, empty, session)?
                        }
                        None => NumericPoint::read(reader, child, empty)?,
                    });
                    Ok(true)
                }
                _ => Ok(false),
            }
        })?;
        Ok(data)
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

    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
    ) -> Result<(), crate::XlsxError> {
        self.write_to_as(writer, "c:numLit")
    }

    pub(crate) fn write_to_as(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        tag: &str,
    ) -> Result<(), crate::XlsxError> {
        if !self.is_valid() {
            return Ok(());
        }
        write_start_tag(writer, tag, vec![], false);
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
            ext.write_for_owner(writer, None, tag.trim_start_matches("c:"), None)?;
        }
        write_end_tag(writer, tag);

        Ok(())
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

    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        workbook: &Workbook,
    ) -> Result<(), crate::XlsxError> {
        match self {
            Self::Reference(value) => value.write_to(writer, workbook)?,
            Self::Literal(value) => value.write_to(writer)?,
        }

        Ok(())
    }
}

/// Reads the `numRef` / `numLit` choice of a `CT_NumDataSource` owner into
/// the owner's reference and literal. Holding more than one choice is an
/// error, never a silent pick.
pub(crate) fn read_numeric_source<R: std::io::BufRead>(
    reader: &mut Reader<R>,
    owner: &BytesStart,
    session: &crate::reader::xlsx::SourceCacheSession,
    reference: &mut NumberReference,
    literal: &mut Option<NumericData>,
) -> Result<(), crate::XlsxError> {
    let _scope_guard = session.enter_child_scope(owner)?;
    let mut choices = 0_usize;
    let complete = children(
        reader,
        owner,
        false,
        |reader, child, empty| -> Result<bool, crate::XlsxError> {
            if !session.accepts_child_namespace(child, super::extension_registry::NS_C)? {
                return Ok(false);
            }
            match local_name(child.name().into_inner()) {
                b"numRef" => {
                    choices += 1;
                    if !empty {
                        reference.set_attributes_with_session(reader, child, session)?;
                    }
                    Ok(true)
                }
                b"numLit" => {
                    choices += 1;
                    *literal = Some(NumericData::read_with_session(
                        reader, child, empty, session,
                    )?);
                    Ok(true)
                }
                _ => Ok(false),
            }
        },
    )?;
    if !complete || choices > 1 {
        return Err(crate::xml::xml_error("Could not read numeric data source"));
    }
    Ok(())
}

/// The array-constant API and hidden-extension plumbing of a numeric data
/// source that stores `number_literal: Option<NumericData>`. The literal is
/// the source while it is present. There is no remover: a constant is replaced
/// by another source, never by an empty reference.
macro_rules! numeric_literal_api {
    ($($clear_field:ident)?) => {
        /// The array-constant source, when the series holds one instead of a
        /// reference.
        #[must_use]
        pub fn number_literal(&self) -> Option<&super::NumericData> {
            self.number_literal.as_ref()
        }

        pub fn number_literal_mut(&mut self) -> Option<&mut super::NumericData> {
            self.number_literal.as_mut()
        }

        /// Makes the literal the source; the reference is not written while a
        /// literal is present.
        pub fn set_number_literal(&mut self, value: super::NumericData) -> &mut Self {
            $(self.$clear_field = None;)?
            self.number_literal = Some(value);
            self
        }

        #[must_use]
        pub fn extensions(&self) -> Option<&super::ExtensionList> {
            self.number_literal
                .as_ref()
                .and_then(super::NumericData::extensions)
        }

        pub fn extensions_mut(&mut self) -> Option<&mut super::ExtensionList> {
            self.number_literal
                .as_mut()
                .and_then(super::NumericData::extensions_mut)
        }

        pub(crate) fn projection_extensions_mut(&mut self) -> Option<&mut super::ExtensionList> {
            self.number_literal
                .as_mut()
                .and_then(super::NumericData::projection_extensions_mut)
        }

        pub(crate) fn restore_literal_extensions(&mut self, previous: &Self) -> usize {
            match self.number_literal.as_mut() {
                Some(literal) => literal.restore_extensions(previous.number_literal.as_ref()),
                None => 0,
            }
        }
    };
}
pub(crate) use numeric_literal_api;

/// A `CT_NumDataSource` owner (`c:val`, `c:yVal`, `c:bubbleSize`): a numeric
/// reference or an array constant such as `={1,2,3}`.
macro_rules! numeric_source {
    ($(#[$doc:meta])* $name:ident, $tag:literal, $label:literal) => {
        $(#[$doc])*
        #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
        #[derive(Clone, Default, Debug)]
        pub struct $name {
            number_reference: super::NumberReference,
            #[cfg_attr(feature = "serde", serde(default))]
            number_literal:   Option<super::NumericData>,
        }

        impl $name {
            #[must_use]
            pub fn number_reference(&self) -> &super::NumberReference {
                &self.number_reference
            }

            #[must_use]
            #[deprecated(since = "3.0.0", note = "Use number_reference()")]
            pub fn get_number_reference(&self) -> &super::NumberReference {
                self.number_reference()
            }

            pub fn number_reference_mut(&mut self) -> &mut super::NumberReference {
                &mut self.number_reference
            }

            #[deprecated(since = "3.0.0", note = "Use number_reference_mut()")]
            pub fn get_number_reference_mut(&mut self) -> &mut super::NumberReference {
                self.number_reference_mut()
            }

            /// Makes `value` the source, replacing any literal.
            pub fn set_number_reference(&mut self, value: super::NumberReference) -> &mut Self {
                self.number_reference = value;
                self.number_literal = None;
                self
            }

            super::numeric_data::numeric_literal_api!();

            pub(crate) fn conflict(&self) -> Option<super::SeriesPartConflictReason> {
                self.number_literal
                    .as_ref()
                    .and_then(super::NumericData::conflict)
            }

            pub(crate) fn is_literal(&self) -> bool {
                self.number_literal.is_some()
            }

            pub(crate) fn set_attributes_with_session<R: std::io::BufRead>(
                &mut self,
                reader: &mut quick_xml::Reader<R>,
                e: &quick_xml::events::BytesStart,
                session: &crate::reader::xlsx::SourceCacheSession,
            ) -> Result<(), crate::XlsxError> {
                super::numeric_data::read_numeric_source(
                    reader,
                    e,
                    session,
                    &mut self.number_reference,
                    &mut self.number_literal,
                )
            }

            pub(crate) fn write_to(
                &self,
                writer: &mut quick_xml::Writer<std::io::Cursor<Vec<u8>>>,
                wb: &crate::structs::Workbook,
            ) -> Result<(), crate::XlsxError> {
                super::numeric_data::write_numeric_source(
                    writer,
                    wb,
                    concat!("c:", $tag),
                    $label,
                    &self.number_reference,
                    self.number_literal.as_ref(),
                )
            }
        }
    };
}
pub(crate) use numeric_source;

/// Writes a `CT_NumDataSource` owner: the literal when the owner holds one,
/// otherwise the reference. An invalid literal is an export error naming
/// `label`.
pub(crate) fn write_numeric_source(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    workbook: &Workbook,
    name: &str,
    label: &str,
    reference: &NumberReference,
    literal: Option<&NumericData>,
) -> Result<(), crate::XlsxError> {
    if literal.is_some_and(|value| !value.is_valid()) {
        return Err(crate::xml::xml_error(format!(
            "chart {label} contain an invalid numeric literal"
        )));
    }
    write_start_tag(writer, name, vec![], false);
    match literal {
        Some(value) => value.write_to(writer)?,
        None => reference.write_to(writer, workbook)?,
    }
    write_end_tag(writer, name);
    Ok(())
}

/// Visit direct children; the callback reports whether it consumed the owner
/// end.
pub(crate) fn children<R: std::io::BufRead>(
    reader: &mut Reader<R>,
    owner: &BytesStart,
    empty: bool,
    mut visit: impl FnMut(&mut Reader<R>, &BytesStart, bool) -> Result<bool, crate::XlsxError>,
) -> Result<bool, crate::XlsxError> {
    if empty {
        return Ok(true);
    }
    let end = owner.name().as_ref().to_vec();
    let mut buffer = Vec::new();
    loop {
        match reader.read_event_into(&mut buffer)? {
            Event::Empty(child) => {
                visit(reader, &child, true)?;
            }
            Event::Start(child) => {
                if !visit(reader, &child, false)? {
                    crate::reader::driver::skip_element(reader, &child)?;
                }
            }
            Event::End(child) if child.name().as_ref() == end => return Ok(true),
            Event::Eof | Event::End(_) => {
                return Err(crate::xml::xml_error("unterminated chart element"));
            }
            _ => (),
        }
        buffer.clear();
    }
}

/// Element-only owners retain declined-child diagnostics, not XML parse errors.
pub(crate) fn element_children<R: std::io::BufRead>(
    reader: &mut Reader<R>,
    owner: &BytesStart,
    empty: bool,
    mut visit: impl FnMut(&mut Reader<R>, &BytesStart, bool) -> Result<bool, crate::XlsxError>,
) -> Result<bool, crate::XlsxError> {
    if empty {
        return Ok(true);
    }
    let end = owner.name().as_ref().to_vec();
    let mut valid = true;
    let mut buffer = Vec::new();
    loop {
        match reader.read_event_into(&mut buffer)? {
            Event::Empty(child) => valid &= visit(reader, &child, true)?,
            Event::Start(child) => {
                if !visit(reader, &child, false)? {
                    valid = false;
                    crate::reader::driver::skip_element(reader, &child)?;
                }
            }
            Event::End(child) if child.name().as_ref() == end => return Ok(valid),
            ref event @ (Event::Text(_) | Event::CData(_) | Event::GeneralRef(_)) => {
                let text = crate::reader::driver::decode_xml_character_event(event)?
                    .ok_or_else(|| crate::xml::xml_error("invalid chart XML text"))?;
                valid &= text
                    .bytes()
                    .all(|byte| matches!(byte, b' ' | b'\t' | b'\r' | b'\n'));
            }
            Event::Comment(_) | Event::PI(_) => (),
            Event::Eof | Event::End(_) => {
                return Err(crate::xml::xml_error("unterminated chart element"));
            }
            _ => return Ok(false),
        }
        buffer.clear();
    }
}
pub(crate) use crate::reader::driver::read_simple_text as read_text;
