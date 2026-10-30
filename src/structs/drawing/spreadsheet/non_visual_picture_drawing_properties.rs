use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::{
        BytesStart,
        Event,
    },
};

use crate::{
    BooleanValue,
    drawing::PictureLocks,
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct NonVisualPictureDrawingProperties {
    prefer_relative_resize: BooleanValue,
    picture_locks:          Option<PictureLocks>,
    extensions:             Option<crate::drawing::ExtensionList>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_resize_defaults_to_true_without_fabricating_presence() {
        assert!(NonVisualPictureDrawingProperties::default().prefer_relative_resize());
        for (attribute, expected) in [
            (None, true),
            (Some("true"), true),
            (Some("1"), true),
            (Some("false"), false),
            (Some("0"), false),
        ] {
            for empty in [false, true] {
                let attribute_xml = attribute
                    .map(|value| format!(" preferRelativeResize=\"{value}\""))
                    .unwrap_or_default();
                let ending = if empty { "/>" } else { "></x:cNvPicPr>" };
                let xml = format!(
                    r#"<x:cNvPicPr xmlns:x="http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing"{attribute_xml}{ending}"#
                );
                let mut reader = Reader::from_reader(xml.as_bytes());
                let owner = match reader.read_event().unwrap() {
                    Event::Start(owner) | Event::Empty(owner) => owner,
                    _ => panic!("property owner"),
                };
                let mut value = NonVisualPictureDrawingProperties::default();
                value
                    .read_with_scope(
                        &mut reader,
                        &owner,
                        empty,
                        &crate::drawing::charts::NamespaceScope::default(),
                    )
                    .unwrap();
                assert_eq!(value.prefer_relative_resize(), expected);
                let mut writer = Writer::new(Cursor::new(Vec::new()));
                value.write_to(&mut writer, &[]).unwrap();
                let written = String::from_utf8(writer.into_inner().into_inner()).unwrap();
                assert_eq!(
                    written.contains("preferRelativeResize="),
                    attribute.is_some()
                );
                if attribute.is_some() {
                    assert!(written.contains(if expected {
                        "preferRelativeResize=\"1\""
                    } else {
                        "preferRelativeResize=\"0\""
                    }));
                }
            }
        }
    }
}

impl NonVisualPictureDrawingProperties {
    #[inline]
    #[must_use]
    pub fn prefer_relative_resize(&self) -> bool {
        !self.prefer_relative_resize.has_value() || self.prefer_relative_resize.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use prefer_relative_resize()")]
    pub fn get_prefer_relative_resize(&self) -> bool {
        self.prefer_relative_resize()
    }

    #[inline]
    pub fn set_prefer_relative_resize(&mut self, value: bool) {
        self.prefer_relative_resize.set_value(value);
    }

    #[inline]
    #[must_use]
    pub fn picture_locks(&self) -> Option<&PictureLocks> {
        self.picture_locks.as_ref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use picture_locks()")]
    pub fn get_picture_locks(&self) -> Option<&PictureLocks> {
        self.picture_locks()
    }

    #[inline]
    pub fn picture_locks_mut(&mut self) -> Option<&mut PictureLocks> {
        self.picture_locks.as_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use picture_locks_mut()")]
    pub fn get_picture_locks_mut(&mut self) -> Option<&mut PictureLocks> {
        self.picture_locks_mut()
    }

    #[inline]
    pub fn set_picture_locks(&mut self, value: PictureLocks) {
        self.picture_locks = Some(value);
    }

    #[must_use]
    pub fn extensions(&self) -> Option<&crate::drawing::ExtensionList> {
        self.extensions.as_ref()
    }

    pub fn extensions_mut(&mut self) -> Option<&mut crate::drawing::ExtensionList> {
        self.extensions.as_mut()
    }

    pub fn set_extensions(&mut self, value: Option<crate::drawing::ExtensionList>) {
        self.extensions = value;
    }

    pub(crate) fn read_with_scope<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        owner: &BytesStart,
        empty: bool,
        parent: &crate::drawing::charts::NamespaceScope,
    ) -> Result<(), crate::XlsxError> {
        let scope = parent
            .child(owner)
            .map_err(super::extended_chart::drawing_error)?;
        if let Some(value) = crate::reader::driver::read_attribute(owner, b"preferRelativeResize")?
        {
            self.prefer_relative_resize.set_value(
                crate::xml::parse_bool(&value).map_err(super::extended_chart::drawing_error)?,
            );
        }
        if empty {
            return Ok(());
        }
        let end = owner.name().as_ref().to_vec();
        let mut seen = std::collections::HashSet::new();
        let mut buffer = Vec::new();
        loop {
            match reader.read_event_into(&mut buffer)? {
                ref event @ (Event::Start(ref child) | Event::Empty(ref child)) => {
                    let empty = matches!(event, Event::Empty(_));
                    let child_scope = scope
                        .child(child)
                        .map_err(super::extended_chart::drawing_error)?;
                    let name = child_scope
                        .resolve(child.name().as_ref(), false)
                        .map_err(super::extended_chart::drawing_error)?;
                    if name.namespace_uri != crate::helper::const_str::DRAWINGML_MAIN_NS
                        || !seen.insert(name.local_name.clone())
                    {
                        return Err(super::extended_chart::drawing_error(
                            "foreign or duplicate nonvisual property",
                        ));
                    }
                    match name.local_name.as_str() {
                        "picLocks" => {
                            let mut value = PictureLocks::default();
                            value.read_with_scope(reader, child, empty, &scope)?;
                            self.set_picture_locks(value);
                        }
                        "extLst" => {
                            self.extensions = Some(
                                crate::drawing::ExtensionList::read(reader, child, empty, &scope)
                                    .map_err(super::extended_chart::drawing_error)?,
                            );
                        }
                        _ => {
                            return Err(super::extended_chart::drawing_error(
                                "unsupported nonvisual property",
                            ));
                        }
                    }
                }
                Event::End(child) if child.name().as_ref() == end.as_slice() => return Ok(()),
                Event::Eof => {
                    return Err(super::extended_chart::drawing_error(
                        "unterminated nonvisual drawing properties",
                    ));
                }
                ref event @ (Event::Text(_) | Event::CData(_) | Event::GeneralRef(_)) => {
                    if !crate::reader::driver::decode_xml_character_event(event)?
                        .is_some_and(|text| crate::xml::trim_xml_whitespace(&text).is_empty())
                    {
                        return Err(super::extended_chart::drawing_error(
                            "non-whitespace nonvisual drawing content",
                        ));
                    }
                }
                Event::Comment(_) | Event::PI(_) => (),
                _ => {
                    return Err(super::extended_chart::drawing_error(
                        "invalid nonvisual drawing event",
                    ));
                }
            }
            buffer.clear();
        }
    }

    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        relationships: &[(String, String)],
    ) -> Result<(), crate::XlsxError> {
        let mut attributes: crate::structs::AttrCollection = Vec::new();
        if self.prefer_relative_resize.has_value() {
            attributes.push(
                (
                    "preferRelativeResize",
                    self.prefer_relative_resize.value_string(),
                )
                    .into(),
            );
        }
        let empty = self.picture_locks.is_none() && self.extensions.is_none();
        write_start_tag(writer, "xdr:cNvPicPr", attributes, empty);
        if let Some(value) = &self.picture_locks {
            value.write_with_relationships(writer, relationships)?;
        }
        if let Some(value) = &self.extensions {
            value.write_with_relationships(writer, relationships)?;
        }
        if !empty {
            write_end_tag(writer, "xdr:cNvPicPr");
        }
        Ok(())
    }

    pub(crate) fn visit_relationships(
        &self,
        visit: &mut dyn FnMut(super::DrawingRelationshipKind, &str),
    ) {
        if let Some(value) = &self.picture_locks {
            value.visit_relationships(visit);
        }
        if let Some(element) = self
            .extensions
            .as_ref()
            .and_then(crate::drawing::ExtensionList::element)
        {
            super::extension_relationships(element, visit);
        }
    }

    pub(crate) fn visit_relationships_mut(
        &mut self,
        visit: &mut dyn FnMut(super::DrawingRelationshipKind, &mut String),
    ) {
        if let Some(value) = &mut self.picture_locks {
            value.visit_relationships_mut(visit);
        }
        if let Some(element) = self
            .extensions
            .as_mut()
            .and_then(crate::drawing::ExtensionList::element_mut)
        {
            super::extension_relationships_mut(element, visit);
        }
    }
}
