use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use super::charts::{
    NamespaceScope,
    OpaqueElement,
};
use crate::{
    helper::const_str::DRAWINGML_MAIN_NS,
    writer::driver::write_start_tag,
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ExtensionList {
    #[cfg_attr(feature = "serde", serde(default))]
    element: Option<Box<OpaqueElement>>,
}
impl ExtensionList {
    #[must_use]
    pub fn element(&self) -> Option<&OpaqueElement> {
        self.element.as_deref()
    }

    pub fn element_mut(&mut self) -> Option<&mut OpaqueElement> {
        self.element.as_deref_mut()
    }

    pub fn new(element: OpaqueElement) -> Result<Self, String> {
        Self::validate(&element)?;
        Ok(Self {
            element: Some(Box::new(element)),
        })
    }

    pub(crate) fn validate(element: &OpaqueElement) -> Result<(), String> {
        if !element.name.is(DRAWINGML_MAIN_NS, "extLst") {
            return Err("invalid DrawingML extension-list root".into());
        }
        for child in crate::xml::direct_children(element)? {
            if !child.name.is(DRAWINGML_MAIN_NS, "ext") {
                return Err("invalid DrawingML extension child".into());
            }
            crate::xml::required_attribute(&child, "", "uri")?;
        }
        Ok(())
    }

    pub(crate) fn read<R: std::io::BufRead>(
        reader: &mut Reader<R>,
        element: &BytesStart<'_>,
        empty: bool,
        scope: &NamespaceScope,
    ) -> Result<Self, String> {
        let mut value = OpaqueElement::read(reader, element, empty, scope)?;
        value.namespaces = NamespaceScope::with_bindings(
            scope
                .effective_bindings()
                .into_iter()
                .chain(value.namespaces),
        )
        .effective_bindings();
        Self::new(value)
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) -> Result<(), String> {
        if let Some(element) = &self.element {
            Self::validate(element)?;
            element.write_to(
                writer,
                &NamespaceScope::with_bindings([(Some("a".into()), DRAWINGML_MAIN_NS.into())]),
            )
        } else {
            write_start_tag(writer, "a:extLst", vec![], true);
            Ok(())
        }
    }

    pub(crate) fn write_with_relationships(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        relationships: &[(String, String)],
    ) -> Result<(), crate::XlsxError> {
        if !relationships.iter().any(|(kind, _)| kind == "AUX") {
            return self.write_to(writer).map_err(crate::xml::xml_error);
        }
        let mut metadata = self.clone();
        if let Some(element) = metadata.element_mut() {
            crate::drawing::spreadsheet::extension_relationships_mut(element, &mut |_, id| {
                crate::drawing::spreadsheet::rebind_relationship_id(id, relationships);
            });
        }
        metadata.write_to(writer).map_err(crate::xml::xml_error)
    }
}
