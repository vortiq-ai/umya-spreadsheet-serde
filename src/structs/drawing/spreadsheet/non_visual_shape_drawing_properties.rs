use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::{
        BytesStart,
        Event,
    },
};

use crate::writer::driver::{
    write_end_tag,
    write_start_tag,
};
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct NonVisualShapeDrawingProperties {
    extensions:  Option<crate::drawing::ExtensionList>,
    shape_locks: Option<Box<crate::drawing::ShapeLocks>>,
    text_box:    crate::BooleanValue,
}
impl NonVisualShapeDrawingProperties {
    #[must_use]
    pub fn shape_locks(&self) -> Option<&crate::drawing::ShapeLocks> {
        self.shape_locks.as_deref()
    }

    pub fn shape_locks_mut(&mut self) -> Option<&mut crate::drawing::ShapeLocks> {
        self.shape_locks.as_deref_mut()
    }

    pub fn set_shape_locks(&mut self, value: crate::drawing::ShapeLocks) {
        self.shape_locks = Some(Box::new(value));
    }

    #[must_use]
    pub fn text_box(&self) -> bool {
        self.text_box.value()
    }

    pub fn set_text_box(&mut self, value: bool) {
        self.text_box.set_value(value);
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
        if let Some(value) = crate::reader::driver::read_attribute(owner, b"txBox")? {
            self.text_box.set_value(
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
                        "spLocks" => {
                            let mut value = crate::drawing::ShapeLocks::default();
                            value.read_with_scope(reader, child, empty, &scope)?;
                            self.set_shape_locks(value);
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
        if self.text_box.has_value() {
            attributes.push(("txBox", self.text_box.value_string()).into());
        }
        let empty = self.shape_locks.is_none() && self.extensions.is_none();
        write_start_tag(writer, "xdr:cNvSpPr", attributes, empty);
        if let Some(value) = &self.shape_locks {
            value.write_with_relationships(writer, relationships)?;
        }
        if let Some(value) = &self.extensions {
            value.write_with_relationships(writer, relationships)?;
        }
        if !empty {
            write_end_tag(writer, "xdr:cNvSpPr");
        }
        Ok(())
    }

    pub(crate) fn visit_relationships(
        &self,
        visit: &mut dyn FnMut(super::DrawingRelationshipKind, &str),
    ) {
        if let Some(value) = &self.shape_locks {
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
        if let Some(value) = &mut self.shape_locks {
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
