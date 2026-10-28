//! Implemented typed payload leaves only; no retained raw copy overrides them.
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use super::{
    SeriesPartConflictReason,
    ShapeProperties,
    extension_names::NamespaceScope,
    numeric_data::children,
};
use crate::writer::driver::{
    write_end_tag,
    write_start_tag,
};
pub const NS_C14: &str = "http://schemas.microsoft.com/office/drawing/2007/8/2/chart";
pub const NS_C16: &str = "http://schemas.microsoft.com/office/drawing/2014/chart";

/// `CT_ChartUniqueID` is a required `xsd:string` attribute, NOT a GUID restriction.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ChartUniqueId {
    value: Option<String>,
}
impl ChartUniqueId {
    #[must_use]
    pub fn value(&self) -> Option<&str> {
        self.value.as_deref()
    }

    pub fn set_value(&mut self, value: Option<String>) -> &mut Self {
        self.value = value;
        self
    }

    #[must_use]
    pub fn conflict(&self) -> Option<SeriesPartConflictReason> {
        self.value
            .is_none()
            .then_some(SeriesPartConflictReason::MissingRequiredChild)
    }

    pub(crate) fn read(e: &BytesStart) -> Self {
        Self {
            value: crate::drawing::get_unescaped_attribute(e, b"val"),
        }
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        if let Some(value) = &self.value {
            write_start_tag(
                writer,
                "c16:uniqueId",
                vec![("val", value.as_str()).into()],
                true,
            );
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct InvertSolidFill {
    shape_properties: Option<ShapeProperties>,
    extra_shapes:     Vec<ShapeProperties>,
    invalid:          bool,
}
impl InvertSolidFill {
    #[must_use]
    pub fn shape_properties(&self) -> Option<&ShapeProperties> {
        self.shape_properties.as_ref()
    }

    pub fn shape_properties_mut(&mut self) -> Option<&mut ShapeProperties> {
        self.shape_properties.as_mut()
    }

    pub fn set_shape_properties(&mut self, value: Option<ShapeProperties>) -> &mut Self {
        self.shape_properties = value;
        self.extra_shapes.clear();
        self
    }

    #[must_use]
    pub fn extra_shapes(&self) -> &[ShapeProperties] {
        &self.extra_shapes
    }

    #[must_use]
    pub fn conflict(&self) -> Option<SeriesPartConflictReason> {
        if self.shape_properties.is_none() {
            Some(SeriesPartConflictReason::MissingRequiredChild)
        } else if !self.extra_shapes.is_empty() {
            Some(SeriesPartConflictReason::TooMany)
        } else {
            self.invalid
                .then_some(SeriesPartConflictReason::InvalidValue)
        }
    }

    /// The outer chart parser supplies the real inherited namespace scope.
    pub(crate) fn read<R: std::io::BufRead>(
        reader: &mut Reader<R>,
        e: &BytesStart,
        empty: bool,
        inherited: &NamespaceScope,
    ) -> Self {
        Self::read_common(reader, e, empty, inherited, None)
    }

    pub(crate) fn read_with_session<R: std::io::BufRead>(
        reader: &mut Reader<R>,
        e: &BytesStart,
        empty: bool,
        inherited: &NamespaceScope,
        session: &crate::reader::xlsx::SourceCacheSession,
    ) -> Self {
        Self::read_common(reader, e, empty, inherited, Some(session))
    }

    fn read_common<R: std::io::BufRead>(
        reader: &mut Reader<R>,
        e: &BytesStart,
        empty: bool,
        inherited: &NamespaceScope,
        session: Option<&crate::reader::xlsx::SourceCacheSession>,
    ) -> Self {
        let mut result = Self::default();
        let scope = inherited.child(e).ok();
        result.invalid |= scope.is_none();
        let complete = children(reader, e, empty, |reader, child, empty| {
            let Some(scope) = &scope else {
                return false;
            };
            let Ok(child_scope) = scope.child(child) else {
                result.invalid = true;
                return false;
            };
            let Ok(name) = child_scope.resolve(child.name().into_inner(), false) else {
                result.invalid = true;
                return false;
            };
            if name.is(NS_C14, "spPr") {
                let mut shape = ShapeProperties::default();
                if !empty {
                    match session {
                        Some(session) => shape.set_attributes_with_session(reader, child, session),
                        None => shape.set_attributes(reader, child),
                    }
                }
                if result.shape_properties.is_none() {
                    result.shape_properties = Some(shape);
                } else {
                    result.extra_shapes.push(shape);
                }
                true
            } else {
                false
            }
        });
        result.invalid |= !complete;
        result
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        if self.conflict().is_some() {
            return;
        }
        if let Some(shape) = &self.shape_properties {
            write_start_tag(writer, "c14:invertSolidFillFmt", vec![], false);
            // Owner patch delegates the unchanged real ShapeProperties writer
            // with the c14 element name required by CT_InvertSolidFillFmt.
            shape.write_to_as(writer, "c14:spPr");
            write_end_tag(writer, "c14:invertSolidFillFmt");
        }
    }
}
