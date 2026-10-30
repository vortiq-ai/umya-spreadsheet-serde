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
    drawing::{
        EndConnection,
        StartConnection,
    },
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct NonVisualConnectorShapeDrawingProperties {
    start_connection: Option<Box<StartConnection>>,
    end_connection:   Option<Box<EndConnection>>,
    extensions:       Option<crate::drawing::ExtensionList>,
    connector_locks:  Option<Box<crate::drawing::ConnectorLocks>>,
}

impl NonVisualConnectorShapeDrawingProperties {
    #[inline]
    #[must_use]
    pub fn start_connection(&self) -> Option<&StartConnection> {
        self.start_connection.as_deref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use start_connection()")]
    pub fn get_start_connection(&self) -> Option<&StartConnection> {
        self.start_connection()
    }

    #[inline]
    pub fn set_start_connection(&mut self, value: StartConnection) {
        self.start_connection = Some(Box::new(value));
    }

    #[inline]
    pub fn remove_start_connection(&mut self) {
        self.start_connection = None;
    }

    #[inline]
    #[must_use]
    pub fn end_connection(&self) -> Option<&EndConnection> {
        self.end_connection.as_deref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use end_connection()")]
    pub fn get_end_connection(&self) -> Option<&EndConnection> {
        self.end_connection()
    }

    #[inline]
    pub fn set_end_connection(&mut self, value: EndConnection) {
        self.end_connection = Some(Box::new(value));
    }

    #[inline]
    pub fn remove_end_connection(&mut self) {
        self.end_connection = None;
    }

    #[must_use]
    pub fn connector_locks(&self) -> Option<&crate::drawing::ConnectorLocks> {
        self.connector_locks.as_deref()
    }

    pub fn connector_locks_mut(&mut self) -> Option<&mut crate::drawing::ConnectorLocks> {
        self.connector_locks.as_deref_mut()
    }

    pub fn set_connector_locks(&mut self, value: crate::drawing::ConnectorLocks) {
        self.connector_locks = Some(Box::new(value));
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
                        "cxnSpLocks" => {
                            let mut value = crate::drawing::ConnectorLocks::default();
                            value.read_with_scope(reader, child, empty, &scope)?;
                            self.set_connector_locks(value);
                        }
                        "stCxn" => {
                            let mut value = StartConnection::default();
                            value.set_attributes(reader, child)?;
                            if !empty {
                                crate::reader::driver::consume_empty_element(reader, child)?;
                            }
                            self.set_start_connection(value);
                        }
                        "endCxn" => {
                            let mut value = EndConnection::default();
                            value.set_attributes(reader, child)?;
                            if !empty {
                                crate::reader::driver::consume_empty_element(reader, child)?;
                            }
                            self.set_end_connection(value);
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
        let attributes: crate::structs::AttrCollection = Vec::new();
        let empty = self.connector_locks.is_none()
            && self.extensions.is_none()
            && self.start_connection.is_none()
            && self.end_connection.is_none();
        write_start_tag(writer, "xdr:cNvCxnSpPr", attributes, empty);
        if let Some(value) = &self.connector_locks {
            value.write_with_relationships(writer, relationships)?;
        }
        if let Some(value) = &self.start_connection {
            value.write_to(writer);
        }
        if let Some(value) = &self.end_connection {
            value.write_to(writer);
        }
        if let Some(value) = &self.extensions {
            value.write_with_relationships(writer, relationships)?;
        }
        if !empty {
            write_end_tag(writer, "xdr:cNvCxnSpPr");
        }
        Ok(())
    }

    pub(crate) fn visit_relationships(
        &self,
        visit: &mut dyn FnMut(super::DrawingRelationshipKind, &str),
    ) {
        if let Some(value) = &self.connector_locks {
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
        if let Some(value) = &mut self.connector_locks {
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
