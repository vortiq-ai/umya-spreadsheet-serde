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
    ExtensionList,
    charts::NamespaceScope,
    spreadsheet::DrawingRelationshipKind,
};
use crate::{
    BooleanValue,
    XlsxError,
    helper::const_str::DRAWINGML_MAIN_NS,
    reader::xlsx::SourceCacheSession,
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ConnectorLocks {
    no_grouping:          BooleanValue,
    no_selection:         BooleanValue,
    no_rotation:          BooleanValue,
    no_change_aspect:     BooleanValue,
    no_move:              BooleanValue,
    no_resize:            BooleanValue,
    no_edit_points:       BooleanValue,
    no_adjust_handles:    BooleanValue,
    no_change_arrowheads: BooleanValue,
    no_change_shape_type: BooleanValue,
    extensions:           Option<Box<ExtensionList>>,
}

impl ConnectorLocks {
    #[must_use]
    pub fn no_grouping(&self) -> bool {
        self.no_grouping.value()
    }

    #[must_use]
    pub fn no_grouping_is_set(&self) -> bool {
        self.no_grouping.has_value()
    }

    pub fn set_no_grouping(&mut self, value: bool) -> &mut Self {
        self.no_grouping.set_value(value);
        self
    }

    pub fn clear_no_grouping(&mut self) -> &mut Self {
        self.no_grouping = BooleanValue::default();
        self
    }

    #[must_use]
    pub fn no_selection(&self) -> bool {
        self.no_selection.value()
    }

    #[must_use]
    pub fn no_selection_is_set(&self) -> bool {
        self.no_selection.has_value()
    }

    pub fn set_no_selection(&mut self, value: bool) -> &mut Self {
        self.no_selection.set_value(value);
        self
    }

    pub fn clear_no_selection(&mut self) -> &mut Self {
        self.no_selection = BooleanValue::default();
        self
    }

    #[must_use]
    pub fn no_rotation(&self) -> bool {
        self.no_rotation.value()
    }

    #[must_use]
    pub fn no_rotation_is_set(&self) -> bool {
        self.no_rotation.has_value()
    }

    pub fn set_no_rotation(&mut self, value: bool) -> &mut Self {
        self.no_rotation.set_value(value);
        self
    }

    pub fn clear_no_rotation(&mut self) -> &mut Self {
        self.no_rotation = BooleanValue::default();
        self
    }

    #[must_use]
    pub fn no_change_aspect(&self) -> bool {
        self.no_change_aspect.value()
    }

    #[must_use]
    pub fn no_change_aspect_is_set(&self) -> bool {
        self.no_change_aspect.has_value()
    }

    pub fn set_no_change_aspect(&mut self, value: bool) -> &mut Self {
        self.no_change_aspect.set_value(value);
        self
    }

    pub fn clear_no_change_aspect(&mut self) -> &mut Self {
        self.no_change_aspect = BooleanValue::default();
        self
    }

    #[must_use]
    pub fn no_move(&self) -> bool {
        self.no_move.value()
    }

    #[must_use]
    pub fn no_move_is_set(&self) -> bool {
        self.no_move.has_value()
    }

    pub fn set_no_move(&mut self, value: bool) -> &mut Self {
        self.no_move.set_value(value);
        self
    }

    pub fn clear_no_move(&mut self) -> &mut Self {
        self.no_move = BooleanValue::default();
        self
    }

    #[must_use]
    pub fn no_resize(&self) -> bool {
        self.no_resize.value()
    }

    #[must_use]
    pub fn no_resize_is_set(&self) -> bool {
        self.no_resize.has_value()
    }

    pub fn set_no_resize(&mut self, value: bool) -> &mut Self {
        self.no_resize.set_value(value);
        self
    }

    pub fn clear_no_resize(&mut self) -> &mut Self {
        self.no_resize = BooleanValue::default();
        self
    }

    #[must_use]
    pub fn no_edit_points(&self) -> bool {
        self.no_edit_points.value()
    }

    #[must_use]
    pub fn no_edit_points_is_set(&self) -> bool {
        self.no_edit_points.has_value()
    }

    pub fn set_no_edit_points(&mut self, value: bool) -> &mut Self {
        self.no_edit_points.set_value(value);
        self
    }

    pub fn clear_no_edit_points(&mut self) -> &mut Self {
        self.no_edit_points = BooleanValue::default();
        self
    }

    #[must_use]
    pub fn no_adjust_handles(&self) -> bool {
        self.no_adjust_handles.value()
    }

    #[must_use]
    pub fn no_adjust_handles_is_set(&self) -> bool {
        self.no_adjust_handles.has_value()
    }

    pub fn set_no_adjust_handles(&mut self, value: bool) -> &mut Self {
        self.no_adjust_handles.set_value(value);
        self
    }

    pub fn clear_no_adjust_handles(&mut self) -> &mut Self {
        self.no_adjust_handles = BooleanValue::default();
        self
    }

    #[must_use]
    pub fn no_change_arrowheads(&self) -> bool {
        self.no_change_arrowheads.value()
    }

    #[must_use]
    pub fn no_change_arrowheads_is_set(&self) -> bool {
        self.no_change_arrowheads.has_value()
    }

    pub fn set_no_change_arrowheads(&mut self, value: bool) -> &mut Self {
        self.no_change_arrowheads.set_value(value);
        self
    }

    pub fn clear_no_change_arrowheads(&mut self) -> &mut Self {
        self.no_change_arrowheads = BooleanValue::default();
        self
    }

    #[must_use]
    pub fn no_change_shape_type(&self) -> bool {
        self.no_change_shape_type.value()
    }

    #[must_use]
    pub fn no_change_shape_type_is_set(&self) -> bool {
        self.no_change_shape_type.has_value()
    }

    pub fn set_no_change_shape_type(&mut self, value: bool) -> &mut Self {
        self.no_change_shape_type.set_value(value);
        self
    }

    pub fn clear_no_change_shape_type(&mut self) -> &mut Self {
        self.no_change_shape_type = BooleanValue::default();
        self
    }

    #[must_use]
    pub fn extensions(&self) -> Option<&ExtensionList> {
        self.extensions.as_deref()
    }

    pub fn extensions_mut(&mut self) -> Option<&mut ExtensionList> {
        self.extensions.as_deref_mut()
    }

    pub fn set_extensions(&mut self, value: Option<ExtensionList>) -> &mut Self {
        self.extensions = value.map(Box::new);
        self
    }

    pub(crate) fn visit_relationships(&self, visit: &mut dyn FnMut(DrawingRelationshipKind, &str)) {
        if let Some(element) = self.extensions().and_then(ExtensionList::element) {
            super::spreadsheet::extension_relationships(element, visit);
        }
    }

    pub(crate) fn visit_relationships_mut(
        &mut self,
        visit: &mut dyn FnMut(DrawingRelationshipKind, &mut String),
    ) {
        if let Some(element) = self.extensions_mut().and_then(ExtensionList::element_mut) {
            super::spreadsheet::extension_relationships_mut(element, visit);
        }
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        owner: &BytesStart<'_>,
    ) -> Result<(), XlsxError> {
        self.read_with_session(reader, owner, false, None)
    }

    pub(crate) fn read_with_session<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        owner: &BytesStart<'_>,
        empty: bool,
        session: Option<&SourceCacheSession>,
    ) -> Result<(), XlsxError> {
        let inherited = session
            .and_then(SourceCacheSession::current_namespace_scope)
            .unwrap_or_default();
        let _guard = session
            .map(|s| s.enter_child_scope(owner))
            .transpose()?
            .flatten();
        self.read_with_scope(reader, owner, empty, &inherited)
    }

    pub(crate) fn read_with_scope<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        owner: &BytesStart<'_>,
        empty: bool,
        inherited: &NamespaceScope,
    ) -> Result<(), XlsxError> {
        crate::reader::driver::validate_attributes(owner)?;
        let scope = inherited.child(owner).map_err(crate::xml::xml_error)?;
        let name = scope
            .resolve(owner.name().as_ref(), false)
            .map_err(crate::xml::xml_error)?;
        if !name.is(DRAWINGML_MAIN_NS, "cxnSpLocks") {
            return Err(crate::xml::xml_error("invalid lock element expanded name"));
        }
        *self = Self::default();
        for attribute in owner.attributes() {
            let attribute = attribute.map_err(crate::xml::xml_error)?;
            let key = attribute.key.as_ref();
            if key == b"xmlns" || key.starts_with(b"xmlns:") {
                continue;
            }
            if !matches!(
                key,
                b"noGrp"
                    | b"noSelect"
                    | b"noRot"
                    | b"noChangeAspect"
                    | b"noMove"
                    | b"noResize"
                    | b"noEditPoints"
                    | b"noAdjustHandles"
                    | b"noChangeArrowheads"
                    | b"noChangeShapeType"
            ) {
                return Err(crate::xml::xml_error("unsupported lock attribute"));
            }
        }
        for (name, target) in [
            (b"noGrp".as_slice(), &mut self.no_grouping),
            (b"noSelect".as_slice(), &mut self.no_selection),
            (b"noRot".as_slice(), &mut self.no_rotation),
            (b"noChangeAspect".as_slice(), &mut self.no_change_aspect),
            (b"noMove".as_slice(), &mut self.no_move),
            (b"noResize".as_slice(), &mut self.no_resize),
            (b"noEditPoints".as_slice(), &mut self.no_edit_points),
            (b"noAdjustHandles".as_slice(), &mut self.no_adjust_handles),
            (
                b"noChangeArrowheads".as_slice(),
                &mut self.no_change_arrowheads,
            ),
            (
                b"noChangeShapeType".as_slice(),
                &mut self.no_change_shape_type,
            ),
        ] {
            if let Some(value) = crate::reader::driver::read_attribute(owner, name)? {
                target.set_value(crate::xml::parse_bool(&value).map_err(crate::xml::xml_error)?);
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
                    let child_name = scope
                        .child(child)
                        .and_then(|scope| scope.resolve(child.name().as_ref(), false))
                        .map_err(crate::xml::xml_error)?;
                    if !child_name.is(DRAWINGML_MAIN_NS, "extLst") || self.extensions.is_some() {
                        return Err(crate::xml::xml_error("unexpected or duplicate lock child"));
                    }
                    self.extensions = Some(Box::new(
                        ExtensionList::read(
                            reader,
                            child,
                            matches!(event, Event::Empty(_)),
                            &scope,
                        )
                        .map_err(crate::xml::xml_error)?,
                    ));
                }
                Event::End(child) if child.name().as_ref() == end => return Ok(()),
                ref event @ (Event::Text(_) | Event::CData(_) | Event::GeneralRef(_)) => {
                    if !crate::reader::driver::decode_xml_character_event(event)?.is_some_and(
                        |text| {
                            text.bytes()
                                .all(|byte| matches!(byte, b' ' | b'\t' | b'\r' | b'\n'))
                        },
                    ) {
                        return Err(crate::xml::xml_error("non-whitespace lock content"));
                    }
                }
                Event::Comment(_) | Event::PI(_) => (),
                _ => {
                    return Err(crate::xml::xml_error(
                        "invalid or unterminated lock element",
                    ));
                }
            }
            buffer.clear();
        }
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) -> Result<(), XlsxError> {
        self.write_with_relationships(writer, &[])
    }

    pub(crate) fn write_with_relationships(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        relationships: &[(String, String)],
    ) -> Result<(), XlsxError> {
        let mut attributes: crate::structs::AttrCollection = Vec::new();
        for (name, value) in [
            ("noGrp", &self.no_grouping),
            ("noSelect", &self.no_selection),
            ("noRot", &self.no_rotation),
            ("noChangeAspect", &self.no_change_aspect),
            ("noMove", &self.no_move),
            ("noResize", &self.no_resize),
            ("noEditPoints", &self.no_edit_points),
            ("noAdjustHandles", &self.no_adjust_handles),
            ("noChangeArrowheads", &self.no_change_arrowheads),
            ("noChangeShapeType", &self.no_change_shape_type),
        ] {
            if value.has_value() {
                attributes.push((name, value.value_string()).into());
            }
        }
        write_start_tag(
            writer,
            "a:cxnSpLocks",
            attributes,
            self.extensions.is_none(),
        );
        if let Some(extensions) = &self.extensions {
            extensions.write_with_relationships(writer, relationships)?;
            write_end_tag(writer, "a:cxnSpLocks");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const NS: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
    fn scope() -> NamespaceScope {
        NamespaceScope::with_bindings([(Some("a".into()), NS.into())])
    }
    fn parse(xml: &str) -> Result<ConnectorLocks, XlsxError> {
        let mut reader = Reader::from_str(xml);
        let mut buffer = Vec::new();
        let event = reader.read_event_into(&mut buffer)?;
        let empty = matches!(event, Event::Empty(_));
        let owner = match event {
            Event::Start(owner) | Event::Empty(owner) => owner,
            _ => return Err(crate::xml::xml_error("missing locks owner")),
        };
        let mut value = ConnectorLocks::default();
        value.read_with_scope(&mut reader, &owner, empty, &scope())?;
        Ok(value)
    }
    fn render(value: &ConnectorLocks) -> Result<String, XlsxError> {
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        value.write_to(&mut writer)?;
        String::from_utf8(writer.into_inner().into_inner()).map_err(Into::into)
    }
    #[test]
    fn native_flags_preserve_presence_and_accept_both_element_forms() {
        for xml in [
            format!(r#"<q:cxnSpLocks xmlns:q="{NS}" noGrp="false" noMove=" &#x9;true&#xA; "/>"#),
            format!(r#"<cxnSpLocks xmlns="{NS}" noGrp="0" noMove="1"></cxnSpLocks>"#),
        ] {
            let value = parse(&xml).unwrap();
            assert!(!value.no_grouping());
            assert!(value.no_grouping_is_set());
            assert!(value.no_move());
            assert!(!value.no_resize_is_set());
            let output = render(&value).unwrap();
            assert!(output.contains(r#"noGrp="0""#));
            assert!(output.contains(r#"noMove="1""#));
            assert!(!output.contains("noResize"));
            let mut changed = value.clone();
            changed.set_no_resize(false);
            changed.clear_no_grouping();
            let output = render(&changed).unwrap();
            assert!(output.contains(r#"noResize="0""#));
            assert!(!output.contains("noGrp"));
        }
    }
    #[test]
    fn malformed_flags_and_children_return_errors_without_panics() {
        for body in [
            r#"noGrp="2"/>"#,
            r#"noGrp="true" noGrp="false"/>"#,
            r#"noUnknown="true"/>"#,
            r#"><a:extLst/><a:extLst/></a:cxnSpLocks>"#,
            r#"><a:unexpected/></a:cxnSpLocks>"#,
            r#"><x:extLst xmlns:x="urn:wrong"/></a:cxnSpLocks>"#,
            r#"><a:extLst><a:ext/></a:extLst></a:cxnSpLocks>"#,
            r#">"#,
        ] {
            let xml = format!("<a:cxnSpLocks {body}");
            let result = std::panic::catch_unwind(|| parse(&xml));
            assert_eq!(
                result
                    .expect("lock parser must not panic")
                    .unwrap_err()
                    .kind(),
                crate::XlsxErrorKind::Xml
            );
        }
    }
    #[test]
    fn extension_namespaces_and_aux_relationships_are_retained() {
        let xml = format!(
            r#"<q:cxnSpLocks xmlns:q="{NS}" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><q:extLst><q:ext uri="urn:future"><u:payload xmlns:u="urn:future" r:embed="rId7"/></q:ext></q:extLst></q:cxnSpLocks>"#
        );
        let mut value = parse(&xml).unwrap();
        assert!(value.extensions().unwrap().element().is_some());
        let mut ids = Vec::new();
        value.visit_relationships(&mut |_, id| ids.push(id.to_string()));
        assert_eq!(ids, ["rId7"]);
        value.visit_relationships_mut(&mut |_, id| *id = "rId9".into());
        let output = render(&value).unwrap();
        assert!(output.contains("rId9"));
        let restored = parse(&output).unwrap();
        let mut ids = Vec::new();
        restored.visit_relationships(&mut |_, id| ids.push(id.to_string()));
        assert_eq!(ids, ["rId9"]);
    }
    #[test]
    fn session_adapter_uses_genuine_inherited_namespace_bindings() {
        let xml = format!(
            r#"<drawing xmlns:q="{NS}" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><q:cxnSpLocks><q:extLst><q:ext uri="urn:future" r:id="rId3"/></q:extLst></q:cxnSpLocks></drawing>"#
        );
        let mut reader = Reader::from_str(&xml);
        let Event::Start(parent) = reader.read_event().unwrap() else {
            panic!("drawing root")
        };
        let inherited = NamespaceScope::default().child(&parent).unwrap();
        let session = SourceCacheSession::immediate();
        let _guard = session.push_extension_scope(inherited);
        let Event::Start(owner) = reader.read_event().unwrap() else {
            panic!("locks owner")
        };
        let mut value = ConnectorLocks::default();
        value
            .read_with_session(&mut reader, &owner, false, Some(&session))
            .unwrap();
        let mut ids = Vec::new();
        value.visit_relationships(&mut |_, id| ids.push(id.to_string()));
        assert_eq!(ids, ["rId3"]);
        assert!(matches!(reader.read_event().unwrap(), Event::End(_)));
    }

    #[cfg(feature = "serde")]
    #[test]
    fn native_locks_have_equal_json_bincode_and_xml_roundtrips() {
        let value = parse(r#"<a:cxnSpLocks noGrp="0"><a:extLst><a:ext uri="urn:future"/></a:extLst></a:cxnSpLocks>"#).unwrap();
        let json = serde_json::to_value(&value).unwrap();
        let binary = bincode::serialize(&value).unwrap();
        for restored in [
            serde_json::from_value::<ConnectorLocks>(json.clone()).unwrap(),
            bincode::deserialize::<ConnectorLocks>(&binary).unwrap(),
        ] {
            assert_eq!(serde_json::to_value(&restored).unwrap(), json);
            assert_eq!(bincode::serialize(&restored).unwrap(), binary);
            assert_eq!(render(&restored).unwrap(), render(&value).unwrap());
        }
    }
}
