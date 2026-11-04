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

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Clone, Default, Debug)]
pub struct Blip {
    image:      MediaObject,
    cstate:     Box<str>,
    #[cfg_attr(feature = "serde", serde(default))]
    effects:    Vec<super::BlipEffect>,
    #[cfg_attr(feature = "serde", serde(default))]
    extensions: Option<Box<crate::drawing::ExtensionList>>,
    #[cfg_attr(feature = "serde", serde(default))]
    link:       Option<String>,
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for Blip {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        const FIELDS: &[&str] = &["image", "cstate", "effects", "extensions", "link"];
        struct BlipVisitor;
        impl<'de> serde::de::Visitor<'de> for BlipVisitor {
            type Value = Blip;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a DrawingML blip")
            }

            fn visit_seq<A: serde::de::SeqAccess<'de>>(self, mut seq: A) -> Result<Blip, A::Error> {
                let value = Blip {
                    image:      seq
                        .next_element()?
                        .ok_or_else(|| serde::de::Error::invalid_length(0, &self))?,
                    cstate:     seq
                        .next_element()?
                        .ok_or_else(|| serde::de::Error::invalid_length(1, &self))?,
                    effects:    seq
                        .next_element()?
                        .ok_or_else(|| serde::de::Error::invalid_length(2, &self))?,
                    extensions: seq
                        .next_element()?
                        .ok_or_else(|| serde::de::Error::invalid_length(3, &self))?,
                    link:       seq
                        .next_element()?
                        .ok_or_else(|| serde::de::Error::invalid_length(4, &self))?,
                };
                if seq.next_element::<serde::de::IgnoredAny>()?.is_some() {
                    return Err(serde::de::Error::invalid_length(6, &self));
                }
                Ok(value)
            }

            fn visit_map<A: serde::de::MapAccess<'de>>(self, mut map: A) -> Result<Blip, A::Error> {
                let mut image = None;
                let mut cstate = None;
                let mut effects = None;
                let mut extensions = None;
                let mut link = None;
                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "image" => {
                            if image.is_some() {
                                return Err(serde::de::Error::duplicate_field("image"));
                            }
                            image = Some(map.next_value()?);
                        }
                        "cstate" => {
                            if cstate.is_some() {
                                return Err(serde::de::Error::duplicate_field("cstate"));
                            }
                            cstate = Some(map.next_value()?);
                        }
                        "effects" => {
                            if effects.is_some() {
                                return Err(serde::de::Error::duplicate_field("effects"));
                            }
                            effects = Some(map.next_value()?);
                        }
                        "extensions" => {
                            if extensions.is_some() {
                                return Err(serde::de::Error::duplicate_field("extensions"));
                            }
                            extensions = Some(map.next_value()?);
                        }
                        "link" => {
                            if link.is_some() {
                                return Err(serde::de::Error::duplicate_field("link"));
                            }
                            link = Some(map.next_value()?);
                        }
                        _ => return Err(serde::de::Error::unknown_field(&key, FIELDS)),
                    }
                }
                let legacy = effects.is_none() && extensions.is_none() && link.is_none();
                let extensions = match extensions {
                    Some(value) => value,
                    None if legacy => {
                        let xml = br#"<a:extLst xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><a:ext uri="{28A0092B-C50C-407E-A947-70E740481C1C}"><a14:useLocalDpi xmlns:a14="http://schemas.microsoft.com/office/drawing/2010/main" val="0"/></a:ext></a:extLst>"#;
                        let mut reader = Reader::from_reader(xml.as_slice());
                        let Event::Start(root) =
                            reader.read_event().map_err(serde::de::Error::custom)?
                        else {
                            return Err(serde::de::Error::custom(
                                "invalid legacy blip extension root",
                            ));
                        };
                        Some(Box::new(
                            crate::drawing::ExtensionList::read(
                                &mut reader,
                                &root,
                                false,
                                &crate::drawing::charts::NamespaceScope::default(),
                            )
                            .map_err(serde::de::Error::custom)?,
                        ))
                    }
                    None => None,
                };
                Ok(Blip {
                    image: image.ok_or_else(|| serde::de::Error::missing_field("image"))?,
                    cstate: cstate.ok_or_else(|| serde::de::Error::missing_field("cstate"))?,
                    effects: effects.unwrap_or_default(),
                    extensions,
                    link: link.unwrap_or_default(),
                })
            }
        }
        deserializer.deserialize_struct("Blip", FIELDS, BlipVisitor)
    }
}

impl Blip {
    #[must_use]
    pub fn link(&self) -> Option<&str> {
        self.link.as_deref()
    }

    pub fn set_link(&mut self, value: Option<String>) -> &mut Self {
        self.link = value;
        self
    }

    pub(crate) fn validate_chart_relationships(
        &self,
        relationships: &RawRelationships,
    ) -> Result<(), crate::XlsxError> {
        let mut failure = None;
        self.visit_relationships(&mut |_, id| {
            let mut matches = relationships
                .relationship_list()
                .iter()
                .filter(|edge| edge.id() == id);
            match (matches.next(), matches.next()) {
                (Some(edge), None)
                    if self.link.as_deref() != Some(id)
                        || edge.get_type() == crate::helper::const_str::IMAGE_NS => {}
                _ => {
                    failure = Some(crate::xml::xml_error(
                        "missing, ambiguous or wrongly typed chart image dependency",
                    ));
                }
            }
        });
        failure.map_or(Ok(()), Err)
    }

    #[must_use]
    pub fn effects(&self) -> &[super::BlipEffect] {
        &self.effects
    }

    pub fn effects_mut(&mut self) -> &mut Vec<super::BlipEffect> {
        &mut self.effects
    }

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
        if let Some(link) = &self.link {
            visit(
                crate::drawing::spreadsheet::DrawingRelationshipKind::Extension,
                link,
            );
        }
        for effect in &self.effects {
            crate::drawing::spreadsheet::extension_relationships(effect.element(), visit);
        }
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
        if let Some(link) = &mut self.link {
            visit(
                crate::drawing::spreadsheet::DrawingRelationshipKind::Extension,
                link,
            );
        }
        for effect in &mut self.effects {
            effect.visit_relationships_mut(visit);
        }
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
        self.read_with_session(reader, owner, empty, Some(drawing_relationships), None)
    }

    pub(crate) fn read_with_session<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        owner: &BytesStart,
        empty: bool,
        drawing_relationships: Option<&RawRelationships>,
        session: Option<&crate::reader::xlsx::SourceCacheSession>,
    ) -> Result<(), crate::XlsxError> {
        let _guard = session
            .map(|session| session.enter_child_scope(owner))
            .transpose()?
            .flatten();
        let scope = match session
            .and_then(crate::reader::xlsx::SourceCacheSession::current_namespace_scope)
        {
            Some(scope) => scope,
            None => crate::drawing::charts::NamespaceScope::default()
                .child(owner)
                .map_err(crate::xml::xml_error)?,
        };
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
            } else if name.is(REL_OFC_NS, "link") {
                self.link = Some(
                    attribute
                        .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                        .map_err(crate::xml::xml_error)?
                        .into_owned(),
                );
            }
        }
        for (id, embedded) in embedded
            .iter()
            .map(|id| (id, true))
            .chain(self.link.iter().map(|id| (id, false)))
        {
            let relationships = drawing_relationships
                .ok_or_else(|| crate::xml::xml_error("drawing image relationships are missing"))?;
            let mut matches = relationships
                .relationship_list()
                .iter()
                .filter(|item| item.id() == id);
            let relationship = matches
                .next()
                .ok_or_else(|| crate::xml::xml_error("blip relationship is missing"))?;
            if matches.next().is_some() {
                return Err(crate::xml::xml_error("ambiguous blip relationship"));
            }
            if relationship.get_type() != crate::helper::const_str::IMAGE_NS {
                return Err(crate::xml::xml_error(
                    "blip relationship does not target an image",
                ));
            }
            if embedded {
                if relationship.target_mode() == "External" {
                    return Err(crate::xml::xml_error(
                        "embedded blip relationship is external",
                    ));
                }
                self.image
                    .set_image_name(relationship.raw_file().file_name())
                    .set_image_data(relationship.raw_file().file_data());
            }
        }
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
                    if self.extensions.is_some() {
                        return Err(crate::xml::xml_error("blip child after extension list"));
                    }
                    if name.is(crate::helper::const_str::DRAWINGML_MAIN_NS, "extLst") {
                        self.extensions = Some(Box::new(
                            crate::drawing::ExtensionList::read(reader, child, empty, &scope)
                                .map_err(crate::xml::xml_error)?,
                        ));
                    } else {
                        self.effects.push(
                            super::BlipEffect::read(reader, child, empty, &scope)
                                .map_err(crate::xml::xml_error)?,
                        );
                    }
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
        let embed = (!self.image.image_name().is_empty())
            .then(|| format!("rId{}", self.image.rid(rel_list)));
        self.write_with_embed_id(writer, embed.as_deref(), rel_list)
    }

    pub(crate) fn write_with_embed_id(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        embed: Option<&str>,
        rel_list: &[(String, String)],
    ) -> Result<(), crate::XlsxError> {
        let mut attributes: crate::structs::AttrCollection = Vec::new();
        attributes.push(("xmlns:r", REL_OFC_NS).into());
        if let Some(embed) = embed {
            attributes.push(("r:embed", embed).into());
        }
        if let Some(link) = &self.link {
            let mut link = link.clone();
            crate::drawing::spreadsheet::rebind_relationship_id(&mut link, rel_list);
            attributes.push(("r:link", link).into());
        }
        if !&self.cstate.is_empty() {
            attributes.push(("cstate", &self.cstate).into());
        }
        write_start_tag(writer, "a:blip", attributes, false);

        for effect in &self.effects {
            effect
                .write_to(writer, rel_list)
                .map_err(crate::xml::xml_error)?;
        }

        if let Some(extensions) = &self.extensions {
            extensions.write_with_relationships(writer, rel_list)?;
        }

        write_end_tag(writer, "a:blip");
        Ok(())
    }
}
