use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use super::{
    DataLabel,
    Marker,
    ReferenceSequence,
    SeriesPartConflictReason,
    ShapeProperties,
    XmlBoolean,
    XmlUnsignedInt,
    numeric_data::{
        children,
        read_text,
    },
};
use crate::{
    reader::{
        driver::local_name,
        xlsx::SourceCacheSession,
    },
    structs::Workbook,
    writer::driver::{
        write_end_tag,
        write_start_tag,
        write_text_node,
    },
};
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct CategoryFilterException {
    references:         ReferenceSequence,
    references_present: bool,
    shape_properties:   Option<ShapeProperties>,
    explosion:          Option<XmlUnsignedInt>,
    invert_if_negative: Option<XmlBoolean>,
    bubble_3d:          Option<XmlBoolean>,
    marker:             Option<Marker>,
    data_label:         Option<DataLabel>,
    invalid:            bool,
}
impl CategoryFilterException {
    // Structural field partition only: no allocation or presence changes.
    pub(crate) fn reference_role_parts(
        &self,
    ) -> (
        &ReferenceSequence,
        Option<&ShapeProperties>,
        Option<&Marker>,
        Option<&DataLabel>,
    ) {
        (
            &self.references,
            self.shape_properties.as_ref(),
            self.marker.as_ref(),
            self.data_label.as_ref(),
        )
    }

    pub(crate) fn reference_role_parts_mut(
        &mut self,
    ) -> (
        &mut ReferenceSequence,
        Option<&mut ShapeProperties>,
        Option<&mut Marker>,
        Option<&mut DataLabel>,
    ) {
        (
            &mut self.references,
            self.shape_properties.as_mut(),
            self.marker.as_mut(),
            self.data_label.as_mut(),
        )
    }

    #[must_use]
    pub fn references(&self) -> &ReferenceSequence {
        &self.references
    }

    pub fn references_mut(&mut self) -> &mut ReferenceSequence {
        self.references_present = true;
        &mut self.references
    }

    // Structural traversal must preserve a missing required sqref child.
    pub(crate) fn references_for_roles_mut(&mut self) -> &mut ReferenceSequence {
        &mut self.references
    }

    #[must_use]
    pub fn shape_properties(&self) -> Option<&ShapeProperties> {
        self.shape_properties.as_ref()
    }

    pub fn shape_properties_mut(&mut self) -> Option<&mut ShapeProperties> {
        self.shape_properties.as_mut()
    }

    pub fn set_shape_properties(&mut self, value: Option<ShapeProperties>) -> &mut Self {
        self.shape_properties = value;
        self
    }

    #[must_use]
    pub fn explosion(&self) -> Option<&XmlUnsignedInt> {
        self.explosion.as_ref()
    }

    pub fn explosion_mut(&mut self) -> Option<&mut XmlUnsignedInt> {
        self.explosion.as_mut()
    }

    pub fn set_explosion(&mut self, value: Option<XmlUnsignedInt>) -> &mut Self {
        self.explosion = value;
        self
    }

    #[must_use]
    pub fn invert_if_negative(&self) -> Option<&XmlBoolean> {
        self.invert_if_negative.as_ref()
    }

    pub fn invert_if_negative_mut(&mut self) -> Option<&mut XmlBoolean> {
        self.invert_if_negative.as_mut()
    }

    pub fn set_invert_if_negative(&mut self, value: Option<XmlBoolean>) -> &mut Self {
        self.invert_if_negative = value;
        self
    }

    #[must_use]
    pub fn bubble_3d(&self) -> Option<&XmlBoolean> {
        self.bubble_3d.as_ref()
    }

    pub fn bubble_3d_mut(&mut self) -> Option<&mut XmlBoolean> {
        self.bubble_3d.as_mut()
    }

    pub fn set_bubble_3d(&mut self, value: Option<XmlBoolean>) -> &mut Self {
        self.bubble_3d = value;
        self
    }

    #[must_use]
    pub fn marker(&self) -> Option<&Marker> {
        self.marker.as_ref()
    }

    pub fn marker_mut(&mut self) -> Option<&mut Marker> {
        self.marker.as_mut()
    }

    pub fn set_marker(&mut self, value: Option<Marker>) -> &mut Self {
        self.marker = value;
        self
    }

    #[must_use]
    pub fn data_label(&self) -> Option<&DataLabel> {
        self.data_label.as_ref()
    }

    pub fn data_label_mut(&mut self) -> Option<&mut DataLabel> {
        self.data_label.as_mut()
    }

    pub fn set_data_label(&mut self, value: Option<DataLabel>) -> &mut Self {
        self.data_label = value;
        self
    }

    #[must_use]
    pub fn conflict(&self) -> Option<SeriesPartConflictReason> {
        if !self.references_present {
            return Some(SeriesPartConflictReason::MissingRequiredChild);
        }
        if self.invalid
            || !self.references.is_valid()
            || self.explosion.as_ref().is_some_and(|v| v.val().is_none())
            || self
                .invert_if_negative
                .as_ref()
                .is_some_and(|v| !v.is_valid())
            || self.bubble_3d.as_ref().is_some_and(|v| !v.is_valid())
            || self.marker.as_ref().is_some_and(|v| !v.is_valid())
        {
            return Some(SeriesPartConflictReason::InvalidValue);
        }
        self.data_label.as_ref().and_then(DataLabel::conflict)
    }

    pub(crate) fn read<R: std::io::BufRead>(
        reader: &mut Reader<R>,
        e: &BytesStart,
        empty: bool,
        session: &SourceCacheSession,
    ) -> Self {
        let _scope_guard = session.enter_child_scope(e);
        let mut result = Self::default();
        let mut names = std::collections::HashSet::new();
        result.invalid = !children(reader, e, empty, |reader, child, empty| {
            let name = if session.accepts_child_namespace(child, super::extension_registry::NS_C15)
            {
                local_name(child.name().into_inner())
            } else {
                b""
            };
            if !matches!(
                name,
                b"sqref"
                    | b"spPr"
                    | b"explosion"
                    | b"invertIfNegative"
                    | b"bubble3D"
                    | b"marker"
                    | b"dLbl"
            ) {
                return false;
            }
            if !names.insert(name.to_vec()) {
                result.invalid = true;
            }
            match name {
                b"sqref" => {
                    result.references_present = true;
                    match read_text(reader, child, empty) {
                        Some(text) => {
                            result.references.set_text(&text);
                        }
                        None => result.invalid = true,
                    }
                }
                b"spPr" => {
                    let mut value = ShapeProperties::default();
                    if !empty {
                        value.set_attributes_with_session(reader, child, session);
                    }
                    result.shape_properties = Some(value);
                }
                b"explosion" => {
                    result.explosion = Some(XmlUnsignedInt::read(child));
                    if !empty {
                        crate::reader::driver::skip_element(reader, child);
                    }
                }
                b"invertIfNegative" => {
                    result.invert_if_negative = Some(XmlBoolean::read(child));
                    if !empty {
                        crate::reader::driver::skip_element(reader, child);
                    }
                }
                b"bubble3D" => {
                    result.bubble_3d = Some(XmlBoolean::read(child));
                    if !empty {
                        crate::reader::driver::skip_element(reader, child);
                    }
                }
                b"marker" => {
                    let mut value = Marker::default();
                    value.set_attributes_with_session(reader, child, empty, session);
                    result.marker = Some(value);
                }
                b"dLbl" => result.data_label = Some(DataLabel::read(reader, child, empty, session)),
                _ => unreachable!(),
            }
            true
        }) || result.invalid;
        result
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>, wb: &Workbook) {
        if self.conflict().is_some() {
            return;
        }
        write_start_tag(writer, "c15:categoryFilterException", vec![], false);
        write_start_tag(writer, "c15:sqref", vec![], false);
        write_text_node(
            writer,
            self.references
                .text()
                .expect("validated reference sequence"),
        );
        write_end_tag(writer, "c15:sqref");
        if let Some(v) = &self.shape_properties {
            v.write_to_as(writer, "c15:spPr");
        }
        if let Some(v) = &self.explosion {
            v.write_to_as(writer, "c15:explosion");
        }
        if let Some(v) = &self.invert_if_negative {
            v.write_to(writer, "c15:invertIfNegative");
        }
        if let Some(v) = &self.bubble_3d {
            v.write_to(writer, "c15:bubble3D");
        }
        if let Some(v) = &self.marker {
            v.write_to_as(writer, "c15:marker");
        }
        if let Some(v) = &self.data_label {
            v.write_to_as(writer, wb, "c15:dLbl");
        }
        write_end_tag(writer, "c15:categoryFilterException");
    }
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct CategoryFilterExceptions {
    entries: Vec<CategoryFilterException>,
    invalid: bool,
}
impl CategoryFilterExceptions {
    #[must_use]
    pub fn entries(&self) -> &[CategoryFilterException] {
        &self.entries
    }

    pub fn entries_mut(&mut self) -> &mut Vec<CategoryFilterException> {
        &mut self.entries
    }

    #[must_use]
    pub fn conflict(&self) -> Option<SeriesPartConflictReason> {
        if self.entries.is_empty() {
            Some(SeriesPartConflictReason::MissingRequiredChild)
        } else if self.invalid {
            Some(SeriesPartConflictReason::InvalidValue)
        } else {
            self.entries
                .iter()
                .find_map(CategoryFilterException::conflict)
        }
    }

    pub(crate) fn read<R: std::io::BufRead>(
        reader: &mut Reader<R>,
        e: &BytesStart,
        empty: bool,
        session: &SourceCacheSession,
    ) -> Self {
        let _scope_guard = session.enter_child_scope(e);
        let mut result = Self::default();
        result.invalid = !children(reader, e, empty, |reader, child, empty| {
            if !session.accepts_child_namespace(child, super::extension_registry::NS_C15)
                || local_name(child.name().into_inner()) != b"categoryFilterException"
            {
                return false;
            }
            result
                .entries
                .push(CategoryFilterException::read(reader, child, empty, session));
            true
        });
        result
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>, wb: &Workbook) {
        if self.conflict().is_some() {
            return;
        }
        write_start_tag(writer, "c15:categoryFilterExceptions", vec![], false);
        for entry in &self.entries {
            entry.write_to(writer, wb);
        }
        write_end_tag(writer, "c15:categoryFilterExceptions");
    }
}
