// xdr:cNvPr
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use super::super::super::{
    BooleanValue,
    StringValue,
    UInt32Value,
};
use crate::writer::driver::{
    write_end_tag,
    write_start_tag,
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct NonVisualDrawingProperties {
    id:              UInt32Value,
    name:            StringValue,
    hidden:          BooleanValue,
    #[cfg_attr(feature = "serde", serde(default))]
    description:     Option<String>,
    #[cfg_attr(feature = "serde", serde(default))]
    title:           Option<String>,
    #[cfg_attr(feature = "serde", serde(default))]
    extensions:      Option<Box<crate::drawing::charts::OpaqueElement>>,
    #[cfg_attr(feature = "serde", serde(default))]
    hyperlink_click: Option<Box<DrawingHyperlink>>,
    #[cfg_attr(feature = "serde", serde(default))]
    hyperlink_hover: Option<Box<DrawingHyperlink>>,
}

impl NonVisualDrawingProperties {
    pub(crate) fn same_restore_identity(&self, previous: &Self) -> bool {
        self.id.has_value() == previous.id.has_value()
            && self.id.value() == previous.id.value()
            && self.name.value() == previous.name.value()
    }

    #[inline]
    #[must_use]
    pub fn id(&self) -> u32 {
        self.id.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use id()")]
    pub fn get_id(&self) -> u32 {
        self.id()
    }

    #[inline]
    pub fn set_id(&mut self, value: u32) -> &mut Self {
        self.id.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn name(&self) -> &str {
        self.name.value_str()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use name()")]
    pub fn get_name(&self) -> &str {
        self.name()
    }

    #[inline]
    pub fn set_name<S: Into<String>>(&mut self, value: S) -> &mut Self {
        self.name.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn hidden(&self) -> bool {
        self.hidden.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use hidden()")]
    pub fn get_hidden(&self) -> bool {
        self.hidden()
    }

    #[inline]
    pub fn set_hidden(&mut self, value: bool) -> &mut Self {
        self.hidden.set_value(value);
        self
    }

    #[must_use]
    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    pub fn set_description(&mut self, value: Option<String>) {
        self.description = value;
    }

    #[must_use]
    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    pub fn set_title(&mut self, value: Option<String>) {
        self.title = value;
    }

    #[must_use]
    pub fn extensions(&self) -> Option<&crate::drawing::charts::OpaqueElement> {
        self.extensions.as_deref()
    }

    pub fn extensions_mut(&mut self) -> Option<&mut crate::drawing::charts::OpaqueElement> {
        self.extensions.as_deref_mut()
    }

    pub fn set_extensions(&mut self, value: Option<crate::drawing::charts::OpaqueElement>) {
        self.extensions = value.map(Box::new);
    }

    pub(crate) fn legacy_shape_id(&self) -> Result<Option<u32>, String> {
        Ok(self.legacy_shape_identity()?.map(|(_, id)| id))
    }

    fn legacy_shape_identity(&self) -> Result<Option<(usize, u32)>, String> {
        use crate::drawing::charts::OpaqueToken;
        let Some(extensions) = self.extensions() else {
            return Ok(None);
        };
        crate::drawing::ExtensionList::validate(extensions)?;
        let mut depth = 0_usize;
        let mut owned = false;
        let mut found = None;
        for (index, token) in extensions.children.iter().enumerate() {
            match token {
                OpaqueToken::Start(start) => {
                    if depth == 0 {
                        owned = start
                            .name
                            .is(crate::helper::const_str::DRAWINGML_MAIN_NS, "ext")
                            && start.attributes.iter().any(|(key, value)| {
                                key.is("", "uri")
                                    && value == "{63B3BB69-23CF-44E3-9099-C40C66FF867C}"
                            });
                    } else if depth == 1
                        && owned
                        && start
                            .name
                            .is(crate::helper::const_str::DRAWING_MAIN_NS, "compatExt")
                    {
                        let mut ids = start
                            .attributes
                            .iter()
                            .filter(|(key, _)| key.is("", "spid"));
                        let (_, text) = ids.next().ok_or("missing legacy OLE shape identifier")?;
                        if ids.next().is_some() || found.is_some() {
                            return Err("ambiguous legacy OLE shape identifier".into());
                        }
                        found = Some((index, crate::structs::parse_legacy_shape_id(text)?));
                    }
                    depth += 1;
                }
                OpaqueToken::End => {
                    depth = depth
                        .checked_sub(1)
                        .ok_or("invalid drawing extension depth")?;
                }
                _ => (),
            }
        }
        Ok(found)
    }

    pub(crate) fn has_id(&self) -> bool {
        self.id.has_value()
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        element: &BytesStart,
        empty: bool,
    ) -> Result<(), crate::XlsxError> {
        use crate::drawing::charts::NamespaceScope;
        let scope = NamespaceScope::default();
        *self = Self::read_with_scope(reader, element, empty, &scope)
            .map_err(super::extended_chart::drawing_error)?;
        Ok(())
    }

    pub(crate) fn read_with_scope<R: std::io::BufRead>(
        reader: &mut Reader<R>,
        element: &BytesStart,
        empty: bool,
        scope: &crate::drawing::charts::NamespaceScope,
    ) -> Result<Self, String> {
        use crate::{
            drawing::charts::OpaqueElement,
            xml::{
                attribute,
                direct_children,
                required_attribute,
            },
        };
        let element = OpaqueElement::read(reader, element, empty, scope)?;
        let mut value = Self::default();
        value.id.set_value(crate::xml::parse_u32(required_attribute(
            &element, "", "id",
        )?)?);
        value
            .name
            .set_value(required_attribute(&element, "", "name")?);
        if let Some(hidden) = attribute(&element, "", "hidden")? {
            value.hidden.set_value(crate::xml::parse_bool(hidden)?);
        }
        value.description = attribute(&element, "", "descr")?.map(str::to_owned);
        value.title = attribute(&element, "", "title")?.map(str::to_owned);
        for child in direct_children(&element)? {
            if child
                .name
                .is(crate::helper::const_str::DRAWINGML_MAIN_NS, "hlinkClick")
                && value.hyperlink_click.is_none()
            {
                value.hyperlink_click = Some(Box::new(DrawingHyperlink::read(&child)?));
            } else if child
                .name
                .is(crate::helper::const_str::DRAWINGML_MAIN_NS, "hlinkHover")
                && value.hyperlink_hover.is_none()
            {
                value.hyperlink_hover = Some(Box::new(DrawingHyperlink::read(&child)?));
            } else if child
                .name
                .is(crate::helper::const_str::DRAWINGML_MAIN_NS, "extLst")
                && value.extensions.is_none()
            {
                crate::drawing::ExtensionList::validate(&child)?;
                value.extensions = Some(Box::new(child));
            } else {
                return Err("unsupported or duplicate nonvisual drawing child".into());
            }
        }
        Ok(value)
    }

    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        ole_id: usize,
        relationships: &[(String, String)],
    ) -> Result<(), crate::XlsxError> {
        if relationships.iter().any(|(kind, _)| kind == "AUX") {
            let mut metadata = self.clone();
            metadata
                .visit_relationships_mut(&mut |_, id| rebind_relationship_id(id, relationships));
            metadata.write_content(writer, ole_id)?;
        } else {
            self.write_content(writer, ole_id)?;
        }
        Ok(())
    }

    fn write_content(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        ole_id: usize,
    ) -> Result<(), crate::XlsxError> {
        use crate::drawing::charts::{
            NamespaceScope,
            OpaqueElement,
            OpaqueStart,
            OpaqueToken,
            QName,
        };
        let mut extensions = self.extensions.as_deref().cloned();
        let legacy_identity = self
            .legacy_shape_identity()
            .map_err(super::extended_chart::drawing_error)?;
        if ole_id > 0 {
            crate::structs::parse_legacy_shape_id(&format!("_x0000_s{ole_id}"))
                .map_err(super::extended_chart::drawing_error)?;
            if let Some((index, _)) = legacy_identity {
                let Some(OpaqueToken::Start(start)) = extensions
                    .as_mut()
                    .and_then(|value| value.children.get_mut(index))
                else {
                    return Err(super::extended_chart::drawing_error(
                        "missing OLE compatibility owner",
                    ));
                };
                let Some((_, text)) = start
                    .attributes
                    .iter_mut()
                    .find(|(key, _)| key.is("", "spid"))
                else {
                    return Err(super::extended_chart::drawing_error(
                        "missing legacy OLE shape identifier",
                    ));
                };
                *text = format!("_x0000_s{ole_id}");
            } else {
                let name = |uri: &str, local: &str, prefix: &str| QName {
                    namespace_uri:    uri.into(),
                    local_name:       local.into(),
                    preferred_prefix: Some(prefix.into()),
                };
                let plain = |local: &str| QName {
                    namespace_uri:    String::new(),
                    local_name:       local.into(),
                    preferred_prefix: None,
                };
                let extensions = extensions.get_or_insert_with(|| OpaqueElement {
                    name:       name(crate::helper::const_str::DRAWINGML_MAIN_NS, "extLst", "a"),
                    namespaces: Vec::new(),
                    attributes: Vec::new(),
                    children:   Vec::new(),
                });
                extensions.children.extend([
                    OpaqueToken::Start(Box::new(OpaqueStart {
                        name:       name(crate::helper::const_str::DRAWINGML_MAIN_NS, "ext", "a"),
                        namespaces: Vec::new(),
                        attributes: vec![(
                            plain("uri"),
                            "{63B3BB69-23CF-44E3-9099-C40C66FF867C}".into(),
                        )],
                    })),
                    OpaqueToken::Start(Box::new(OpaqueStart {
                        name:       name(
                            "http://schemas.microsoft.com/office/drawing/2010/main",
                            "compatExt",
                            "a14",
                        ),
                        namespaces: vec![(
                            Some("a14".into()),
                            "http://schemas.microsoft.com/office/drawing/2010/main".into(),
                        )],
                        attributes: vec![(plain("spid"), format!("_x0000_s{ole_id}"))],
                    })),
                    OpaqueToken::End,
                    OpaqueToken::End,
                ]);
            }
        }
        let mut attributes: crate::structs::AttrCollection = Vec::new();
        let id = self.id.value_string();
        attributes.push(("id", &id).into());
        attributes.push(("name", self.name.value_str()).into());
        if self.hidden.has_value() {
            attributes.push(("hidden", self.hidden.value_string()).into());
        }
        if let Some(value) = &self.description {
            attributes.push(("descr", value).into());
        }
        if let Some(value) = &self.title {
            attributes.push(("title", value).into());
        }
        let inner = extensions.is_some()
            || self.hyperlink_click.is_some()
            || self.hyperlink_hover.is_some();
        write_start_tag(writer, "xdr:cNvPr", attributes, !inner);
        if let Some(value) = &self.hyperlink_click {
            value.write_to(writer, "a:hlinkClick")?;
        }
        if let Some(value) = &self.hyperlink_hover {
            value.write_to(writer, "a:hlinkHover")?;
        }
        if let Some(extensions) = &extensions {
            extensions
                .write_to(
                    writer,
                    &NamespaceScope::with_bindings([(
                        Some("a".into()),
                        crate::helper::const_str::DRAWINGML_MAIN_NS.into(),
                    )]),
                )
                .map_err(super::extended_chart::drawing_error)?;
        }
        if inner {
            write_end_tag(writer, "xdr:cNvPr");
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DrawingRelationshipKind {
    Hyperlink,
    Sound,
    Extension,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct DrawingSound {
    pub relationship_id: String,
    pub name:            Option<String>,
    pub built_in:        Option<bool>,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct DrawingHyperlink {
    pub relationship_id: Option<String>,
    pub invalid_url:     Option<String>,
    pub action:          Option<String>,
    pub target_frame:    Option<String>,
    pub tooltip:         Option<String>,
    pub history:         Option<bool>,
    pub highlight_click: Option<bool>,
    pub end_sound:       Option<bool>,
    pub sound:           Option<DrawingSound>,
    pub extensions:      Option<Box<crate::drawing::charts::OpaqueElement>>,
}
fn optional_bool(
    element: &crate::drawing::charts::OpaqueElement,
    name: &str,
) -> Result<Option<bool>, String> {
    crate::xml::attribute(element, "", name)?
        .map(crate::xml::parse_bool)
        .transpose()
}
impl DrawingHyperlink {
    fn read(element: &crate::drawing::charts::OpaqueElement) -> Result<Self, String> {
        use crate::xml::{
            attribute,
            direct_children,
            required_attribute,
        };
        let text = |namespace, name| {
            attribute(element, namespace, name)
                .map(|value| value.map(str::to_owned))
                .map_err(|error| error.clone())
        };
        let mut value = Self {
            relationship_id: text(crate::helper::const_str::REL_OFC_NS, "id")?,
            invalid_url: text("", "invalidUrl")?,
            action: text("", "action")?,
            target_frame: text("", "tgtFrame")?,
            tooltip: text("", "tooltip")?,
            history: optional_bool(element, "history")?,
            highlight_click: optional_bool(element, "highlightClick")?,
            end_sound: optional_bool(element, "endSnd")?,
            ..Self::default()
        };
        for child in direct_children(element)? {
            if child
                .name
                .is(crate::helper::const_str::DRAWINGML_MAIN_NS, "snd")
                && value.sound.is_none()
                && value.extensions.is_none()
            {
                if !direct_children(&child)?.is_empty() {
                    return Err("drawing sound must be empty".into());
                }
                value.sound = Some(DrawingSound {
                    relationship_id: required_attribute(
                        &child,
                        crate::helper::const_str::REL_OFC_NS,
                        "embed",
                    )?
                    .to_owned(),
                    name:            attribute(&child, "", "name")?.map(str::to_owned),
                    built_in:        optional_bool(&child, "builtIn")?,
                });
            } else if child
                .name
                .is(crate::helper::const_str::DRAWINGML_MAIN_NS, "extLst")
                && value.extensions.is_none()
            {
                crate::drawing::ExtensionList::validate(&child)?;
                value.extensions = Some(Box::new(child));
            } else {
                return Err(
                    "unsupported, duplicate or out-of-order drawing hyperlink child".into(),
                );
            }
        }
        Ok(value)
    }

    fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        name: &str,
    ) -> Result<(), crate::XlsxError> {
        let mut attrs: crate::structs::AttrCollection =
            vec![("xmlns:r", crate::helper::const_str::REL_OFC_NS).into()];
        for (name, value) in [
            ("r:id", &self.relationship_id),
            ("invalidUrl", &self.invalid_url),
            ("action", &self.action),
            ("tgtFrame", &self.target_frame),
            ("tooltip", &self.tooltip),
        ] {
            if let Some(value) = value {
                attrs.push((name, value).into());
            }
        }
        for (name, value) in [
            ("history", self.history),
            ("highlightClick", self.highlight_click),
            ("endSnd", self.end_sound),
        ] {
            if let Some(value) = value {
                attrs.push((name, if value { "1" } else { "0" }).into());
            }
        }
        let inner = self.sound.is_some() || self.extensions.is_some();
        write_start_tag(writer, name, attrs, !inner);
        if let Some(sound) = &self.sound {
            let mut attrs: crate::structs::AttrCollection =
                vec![("r:embed", &sound.relationship_id).into()];
            if let Some(value) = &sound.name {
                attrs.push(("name", value).into());
            }
            if let Some(value) = sound.built_in {
                attrs.push(("builtIn", if value { "1" } else { "0" }).into());
            }
            write_start_tag(writer, "a:snd", attrs, true);
        }
        if let Some(value) = &self.extensions {
            value
                .write_to(
                    writer,
                    &crate::drawing::charts::NamespaceScope::with_bindings([
                        (
                            Some("a".into()),
                            crate::helper::const_str::DRAWINGML_MAIN_NS.into(),
                        ),
                        (
                            Some("r".into()),
                            crate::helper::const_str::REL_OFC_NS.into(),
                        ),
                    ]),
                )
                .map_err(super::extended_chart::drawing_error)?;
        }
        if inner {
            write_end_tag(writer, name);
        }
        Ok(())
    }

    fn visit_relationships(&self, visit: &mut dyn FnMut(DrawingRelationshipKind, &str)) {
        if let Some(value) = &self.relationship_id {
            visit(DrawingRelationshipKind::Hyperlink, value);
        }
        if let Some(sound) = &self.sound {
            visit(DrawingRelationshipKind::Sound, &sound.relationship_id);
        }
        if let Some(value) = &self.extensions {
            extension_relationships(value, visit);
        }
    }

    fn visit_relationships_mut(
        &mut self,
        visit: &mut dyn FnMut(DrawingRelationshipKind, &mut String),
    ) {
        if let Some(value) = &mut self.relationship_id {
            visit(DrawingRelationshipKind::Hyperlink, value);
        }
        if let Some(sound) = &mut self.sound {
            visit(DrawingRelationshipKind::Sound, &mut sound.relationship_id);
        }
        if let Some(value) = &mut self.extensions {
            extension_relationships_mut(value, visit);
        }
    }
}
pub(crate) fn extension_relationships(
    value: &crate::drawing::charts::OpaqueElement,
    visit: &mut dyn FnMut(DrawingRelationshipKind, &str),
) {
    let mut attributes = |attrs: &[(crate::drawing::charts::QName, String)]| {
        for (name, value) in attrs {
            if name.namespace_uri == crate::helper::const_str::REL_OFC_NS
                && matches!(name.local_name.as_str(), "id" | "embed" | "link")
            {
                visit(DrawingRelationshipKind::Extension, value);
            }
        }
    };
    attributes(&value.attributes);
    for token in &value.children {
        if let crate::drawing::charts::OpaqueToken::Start(value) = token {
            attributes(&value.attributes);
        }
    }
}
pub(crate) fn extension_relationships_mut(
    value: &mut crate::drawing::charts::OpaqueElement,
    visit: &mut dyn FnMut(DrawingRelationshipKind, &mut String),
) {
    let mut attributes = |attrs: &mut [(crate::drawing::charts::QName, String)]| {
        for (name, value) in attrs {
            if name.namespace_uri == crate::helper::const_str::REL_OFC_NS
                && matches!(name.local_name.as_str(), "id" | "embed" | "link")
            {
                visit(DrawingRelationshipKind::Extension, value);
            }
        }
    };
    attributes(&mut value.attributes);
    for token in &mut value.children {
        if let crate::drawing::charts::OpaqueToken::Start(value) = token {
            attributes(&mut value.attributes);
        }
    }
}
impl NonVisualDrawingProperties {
    #[must_use]
    pub fn hyperlink_click(&self) -> Option<&DrawingHyperlink> {
        self.hyperlink_click.as_deref()
    }

    pub fn hyperlink_click_mut(&mut self) -> Option<&mut DrawingHyperlink> {
        self.hyperlink_click.as_deref_mut()
    }

    pub fn set_hyperlink_click(&mut self, value: Option<DrawingHyperlink>) {
        self.hyperlink_click = value.map(Box::new);
    }

    #[must_use]
    pub fn hyperlink_hover(&self) -> Option<&DrawingHyperlink> {
        self.hyperlink_hover.as_deref()
    }

    pub fn hyperlink_hover_mut(&mut self) -> Option<&mut DrawingHyperlink> {
        self.hyperlink_hover.as_deref_mut()
    }

    pub fn set_hyperlink_hover(&mut self, value: Option<DrawingHyperlink>) {
        self.hyperlink_hover = value.map(Box::new);
    }

    pub fn visit_relationships(&self, visit: &mut dyn FnMut(DrawingRelationshipKind, &str)) {
        if let Some(value) = &self.hyperlink_click {
            value.visit_relationships(visit);
        }
        if let Some(value) = &self.hyperlink_hover {
            value.visit_relationships(visit);
        }
        if let Some(value) = &self.extensions {
            extension_relationships(value, visit);
        }
    }

    pub fn visit_relationships_mut(
        &mut self,
        visit: &mut dyn FnMut(DrawingRelationshipKind, &mut String),
    ) {
        if let Some(value) = &mut self.hyperlink_click {
            value.visit_relationships_mut(visit);
        }
        if let Some(value) = &mut self.hyperlink_hover {
            value.visit_relationships_mut(visit);
        }
        if let Some(value) = &mut self.extensions {
            extension_relationships_mut(value, visit);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drawing::charts::{
        NamespaceScope,
        OpaqueElement,
    };

    fn read(xml: &str) -> NonVisualDrawingProperties {
        let mut reader = Reader::from_reader(xml.as_bytes());
        let quick_xml::events::Event::Start(root) = reader.read_event().unwrap() else {
            panic!("root")
        };
        NonVisualDrawingProperties::read_with_scope(
            &mut reader,
            &root,
            false,
            &NamespaceScope::default(),
        )
        .unwrap()
    }
    fn write(value: &NonVisualDrawingProperties) -> String {
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        write_start_tag(
            &mut writer,
            "xdr:wsDr",
            vec![
                ("xmlns:xdr", crate::helper::const_str::SHEET_DRAWING_NS).into(),
                ("xmlns:a", crate::helper::const_str::DRAWINGML_MAIN_NS).into(),
            ],
            false,
        );
        value.write_to(&mut writer, 0, &[]).unwrap();
        write_end_tag(&mut writer, "xdr:wsDr");
        let bytes = writer.into_inner().into_inner();
        let mut reader = Reader::from_reader(bytes.as_slice());
        let quick_xml::events::Event::Start(root) = reader.read_event().unwrap() else {
            panic!("root")
        };
        let root =
            OpaqueElement::read(&mut reader, &root, false, &NamespaceScope::default()).unwrap();
        let children = crate::xml::direct_children(&root).unwrap();
        String::from_utf8(super::super::extended_chart::canonical_xml(&children[0]).unwrap())
            .unwrap()
    }
    fn source_properties() -> NonVisualDrawingProperties {
        read(concat!(
            "<xdr:cNvPr xmlns:xdr=\"http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing\" ",
            "xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" ",
            "xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" id=\"4\" name=\"Chart 2\" descr=\"\" title=\"Waterfall\">",
            "<a:extLst><a:ext uri=\"{FF2B5EF4-FFF2-40B4-BE49-F238E27FC236}\">",
            "<a16:creationId xmlns:a16=\"http://schemas.microsoft.com/office/drawing/2014/main\" ",
            "id=\"{393D7C90-AF84-3958-641C-0FEC03FE8894}\"/></a:ext>",
            "<a:ext uri=\"urn:future\"><u:payload xmlns:u=\"urn:future\" r:link=\"rId11\"/></a:ext></a:extLst></xdr:cNvPr>"
        ))
    }
    #[test]
    fn native_frame_metadata_and_future_creation_id_survive_typed_edits() {
        let mut value = source_properties();
        assert_eq!(value.description(), Some(""));
        assert_eq!(value.title(), Some("Waterfall"));
        value.set_description(Some("edited description".into()));
        value.set_hyperlink_click(Some(DrawingHyperlink {
            relationship_id: Some("rId9".into()),
            action: Some("ppaction://hlinkshowjump".into()),
            history: Some(false),
            sound: Some(DrawingSound {
                relationship_id: "rId10".into(),
                name:            Some(String::new()),
                built_in:        Some(false),
            }),
            ..DrawingHyperlink::default()
        }));
        let mut relationships = Vec::new();
        value.visit_relationships(&mut |kind, id| relationships.push((kind, id.to_owned())));
        assert_eq!(
            relationships,
            [
                (DrawingRelationshipKind::Hyperlink, "rId9".into()),
                (DrawingRelationshipKind::Sound, "rId10".into()),
                (DrawingRelationshipKind::Extension, "rId11".into())
            ]
        );
        value.visit_relationships_mut(&mut |_, id| *id = format!("output-{id}"));
        let xml = write(&value);
        assert!(xml.contains("{393D7C90-AF84-3958-641C-0FEC03FE8894}"));
        let reread = read(&xml);
        assert_eq!(reread.id(), 4);
        assert_eq!(reread.description(), Some("edited description"));
        let link = reread.hyperlink_click().unwrap();
        assert_eq!(link.relationship_id.as_deref(), Some("output-rId9"));
        assert_eq!(link.history, Some(false));
        assert_eq!(link.sound.as_ref().unwrap().relationship_id, "output-rId10");
        assert_eq!(link.sound.as_ref().unwrap().name.as_deref(), Some(""));
        assert!(xml.contains("output-rId11"));
    }
    #[cfg(feature = "serde")]
    #[test]
    fn nonvisual_metadata_roundtrips_in_json_and_bincode() {
        let value = source_properties();
        let expected = serde_json::to_value(&value).unwrap();
        let json: NonVisualDrawingProperties = serde_json::from_value(expected.clone()).unwrap();
        let binary: NonVisualDrawingProperties =
            bincode::deserialize(&bincode::serialize(&value).unwrap()).unwrap();
        assert_eq!(serde_json::to_value(json).unwrap(), expected);
        assert_eq!(serde_json::to_value(binary).unwrap(), expected);
    }
}

pub(crate) fn rebind_relationship_id(id: &mut String, relationships: &[(String, String)]) {
    if let Some(index) = relationships
        .iter()
        .position(|(kind, source)| kind == "AUX" && source == id)
    {
        *id = format!("rId{}", index + 1);
    }
}
