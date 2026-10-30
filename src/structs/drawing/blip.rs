// a:blip
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
    helper::const_str::REL_OFC_NS,
    structs::{
        MediaObject,
        raw::RawRelationships,
    },
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct Blip {
    image:      MediaObject,
    cstate:     Box<str>,
    #[cfg_attr(feature = "serde", serde(default))]
    extensions: Option<Box<crate::drawing::ExtensionList>>,
}

impl Blip {
    #[inline]
    #[must_use]
    pub fn image(&self) -> &MediaObject {
        &self.image
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use image()")]
    pub fn get_image(&self) -> &MediaObject {
        self.image()
    }

    #[inline]
    pub fn image_mut(&mut self) -> &mut MediaObject {
        &mut self.image
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use image_mut()")]
    pub fn get_image_mut(&mut self) -> &mut MediaObject {
        self.image_mut()
    }

    #[inline]
    pub fn set_image(&mut self, value: MediaObject) -> &mut Self {
        self.image = value;
        self
    }

    #[inline]
    #[must_use]
    pub fn cstate(&self) -> &str {
        &self.cstate
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use cstate()")]
    pub fn get_cstate(&self) -> &str {
        self.cstate()
    }

    #[inline]
    pub fn set_cstate<S: Into<String>>(&mut self, value: S) -> &mut Self {
        self.cstate = value.into().into_boxed_str();
        self
    }

    #[must_use]
    pub fn extensions(&self) -> Option<&crate::drawing::ExtensionList> {
        self.extensions.as_deref()
    }

    pub fn extensions_mut(&mut self) -> Option<&mut crate::drawing::ExtensionList> {
        self.extensions.as_deref_mut()
    }

    pub fn set_extensions(&mut self, value: Option<crate::drawing::ExtensionList>) {
        self.extensions = value.map(Box::new);
    }

    pub(crate) fn visit_relationships(
        &self,
        visit: &mut dyn FnMut(crate::drawing::spreadsheet::DrawingRelationshipKind, &str),
    ) {
        if let Some(element) = self
            .extensions()
            .and_then(crate::drawing::ExtensionList::element)
        {
            crate::drawing::spreadsheet::extension_relationships(element, visit);
        }
    }

    pub(crate) fn visit_relationships_mut(
        &mut self,
        visit: &mut dyn FnMut(crate::drawing::spreadsheet::DrawingRelationshipKind, &mut String),
    ) {
        if let Some(element) = self
            .extensions_mut()
            .and_then(crate::drawing::ExtensionList::element_mut)
        {
            crate::drawing::spreadsheet::extension_relationships_mut(element, visit);
        }
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        owner: &BytesStart,
        drawing_relationships: &RawRelationships,
        empty: bool,
    ) -> Result<(), crate::XlsxError> {
        self.read_with_session(reader, owner, empty, drawing_relationships, None)
    }

    pub(crate) fn read_with_session<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        owner: &BytesStart,
        empty: bool,
        drawing_relationships: &RawRelationships,
        session: Option<&crate::reader::xlsx::SourceCacheSession>,
    ) -> Result<(), crate::XlsxError> {
        let _guard = session
            .map(|session| session.enter_child_scope(owner))
            .transpose()?
            .flatten();
        let scope = crate::drawing::charts::NamespaceScope::default()
            .child(owner)
            .map_err(crate::xml::xml_error)?;
        if let Some(value) = crate::reader::driver::read_attribute(owner, b"cstate")? {
            if !matches!(
                value.as_str(),
                "email" | "screen" | "print" | "hqprint" | "none"
            ) {
                return Err(crate::xml::xml_error("invalid blip compression state"));
            }
            self.set_cstate(value);
        }
        let mut embedded = None;
        for attribute in owner.attributes() {
            let attribute = attribute.map_err(crate::xml::xml_error)?;
            if attribute.key.as_ref() == b"xmlns" || attribute.key.as_ref().starts_with(b"xmlns:") {
                continue;
            }
            let name = scope
                .resolve(attribute.key.as_ref(), true)
                .map_err(crate::xml::xml_error)?;
            if name.is(REL_OFC_NS, "embed") {
                embedded = Some(
                    attribute
                        .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                        .map_err(crate::xml::xml_error)?
                        .into_owned(),
                );
            }
        }
        let id =
            embedded.ok_or_else(|| crate::xml::xml_error("missing embedded blip relationship"))?;
        let mut matches = drawing_relationships
            .relationship_list()
            .iter()
            .filter(|item| item.id() == id);
        let relationship = matches
            .next()
            .ok_or_else(|| crate::xml::xml_error("embedded blip relationship is missing"))?;
        if matches.next().is_some() {
            return Err(crate::xml::xml_error(
                "ambiguous embedded blip relationship",
            ));
        }
        if relationship.get_type() != crate::helper::const_str::IMAGE_NS {
            return Err(crate::xml::xml_error(
                "blip relationship does not target an image",
            ));
        }
        self.image_mut()
            .set_image_name(relationship.raw_file().file_name())
            .set_image_data(relationship.raw_file().file_data());
        if empty {
            return Ok(());
        }
        let end = owner.name().as_ref().to_vec();
        let mut buffer = Vec::new();
        loop {
            match reader.read_event_into(&mut buffer)? {
                ref event @ (Event::Start(ref child) | Event::Empty(ref child)) => {
                    let empty = matches!(event, Event::Empty(_));
                    let name = scope
                        .child(child)
                        .and_then(|scope| scope.resolve(child.name().as_ref(), false))
                        .map_err(crate::xml::xml_error)?;
                    if !name.is(crate::helper::const_str::DRAWINGML_MAIN_NS, "extLst")
                        || self.extensions.is_some()
                    {
                        return Err(crate::xml::xml_error("unsupported or duplicate blip child"));
                    }
                    self.extensions = Some(Box::new(
                        crate::drawing::ExtensionList::read(reader, child, empty, &scope)
                            .map_err(crate::xml::xml_error)?,
                    ));
                }
                Event::End(child) if child.name().as_ref() == end.as_slice() => return Ok(()),
                Event::Eof => return Err(crate::xml::xml_error("unterminated blip")),
                _ => (),
            }
            buffer.clear();
        }
    }

    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        rel_list: &mut Vec<(String, String)>,
    ) -> Result<(), crate::XlsxError> {
        // a:blip
        let r_id = self.image.rid(rel_list);
        let r_id_str = format!("rId{r_id}");
        let mut attributes: crate::structs::AttrCollection = Vec::new();
        attributes.push(("xmlns:r", REL_OFC_NS).into());
        attributes.push(("r:embed", &r_id_str).into());
        if !&self.cstate.is_empty() {
            attributes.push(("cstate", &self.cstate).into());
        }
        write_start_tag(writer, "a:blip", attributes, false);

        if let Some(extensions) = &self.extensions {
            extensions.write_with_relationships(writer, rel_list)?;
        }

        write_end_tag(writer, "a:blip");
        Ok(())
    }
}
