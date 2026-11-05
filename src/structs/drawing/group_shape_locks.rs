// a:grpSpLocks
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
    reader::driver::read_attribute,
    structs::BooleanValue,
    writer::driver::write_start_tag,
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct GroupShapeLocks {
    #[cfg_attr(feature = "serde", serde(default))]
    extensions:       Option<Box<crate::drawing::ExtensionList>>,
    no_change_aspect: BooleanValue,
    no_grouping:      BooleanValue,
    no_move:          BooleanValue,
    no_resize:        BooleanValue,
    no_rotation:      BooleanValue,
    no_selection:     BooleanValue,
    no_ungrouping:    BooleanValue,
}

impl GroupShapeLocks {
    #[inline]
    #[must_use]
    pub fn no_change_aspect(&self) -> bool {
        self.no_change_aspect.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use no_change_aspect()")]
    pub fn get_no_change_aspect(&self) -> bool {
        self.no_change_aspect()
    }

    #[inline]
    pub fn set_no_change_aspect(&mut self, value: bool) {
        self.no_change_aspect.set_value(value);
    }

    #[inline]
    #[must_use]
    pub fn no_grouping(&self) -> bool {
        self.no_grouping.value()
    }

    #[inline]
    pub fn set_no_grouping(&mut self, value: bool) {
        self.no_grouping.set_value(value);
    }

    #[inline]
    #[must_use]
    pub fn no_move(&self) -> bool {
        self.no_move.value()
    }

    #[inline]
    pub fn set_no_move(&mut self, value: bool) {
        self.no_move.set_value(value);
    }

    #[inline]
    #[must_use]
    pub fn no_resize(&self) -> bool {
        self.no_resize.value()
    }

    #[inline]
    pub fn set_no_resize(&mut self, value: bool) {
        self.no_resize.set_value(value);
    }

    #[inline]
    #[must_use]
    pub fn no_rotation(&self) -> bool {
        self.no_rotation.value()
    }

    #[inline]
    pub fn set_no_rotation(&mut self, value: bool) {
        self.no_rotation.set_value(value);
    }

    #[inline]
    #[must_use]
    pub fn no_selection(&self) -> bool {
        self.no_selection.value()
    }

    #[inline]
    pub fn set_no_selection(&mut self, value: bool) {
        self.no_selection.set_value(value);
    }

    #[inline]
    #[must_use]
    pub fn no_ungrouping(&self) -> bool {
        self.no_ungrouping.value()
    }

    #[inline]
    pub fn set_no_ungrouping(&mut self, value: bool) {
        self.no_ungrouping.set_value(value);
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        _reader: &mut Reader<R>,
        e: &BytesStart,
    ) -> Result<(), crate::XlsxError> {
        for (name, field) in [
            (b"noChangeAspect".as_slice(), &mut self.no_change_aspect),
            (b"noGrp".as_slice(), &mut self.no_grouping),
            (b"noMove".as_slice(), &mut self.no_move),
            (b"noResize".as_slice(), &mut self.no_resize),
            (b"noRot".as_slice(), &mut self.no_rotation),
            (b"noSelect".as_slice(), &mut self.no_selection),
            (b"noUngrp".as_slice(), &mut self.no_ungrouping),
        ] {
            if let Some(value) = read_attribute(e, name)? {
                field.set_value(crate::xml::parse_bool(&value).map_err(crate::xml::xml_error)?);
            }
        }
        Ok(())
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

    pub(crate) fn read_with_session<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        owner: &BytesStart<'_>,
        empty: bool,
        session: Option<&crate::reader::xlsx::SourceCacheSession>,
    ) -> Result<(), crate::XlsxError> {
        let inherited = session
            .and_then(crate::reader::xlsx::SourceCacheSession::current_namespace_scope)
            .unwrap_or_default();
        self.read_with_scope(reader, owner, empty, &inherited)
    }

    pub(crate) fn read_with_scope<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        owner: &BytesStart<'_>,
        empty: bool,
        inherited: &crate::drawing::charts::NamespaceScope,
    ) -> Result<(), crate::XlsxError> {
        let scope = inherited.child(owner).map_err(crate::xml::xml_error)?;
        let name = scope
            .resolve(owner.name().as_ref(), false)
            .map_err(crate::xml::xml_error)?;
        if !name.is(crate::helper::const_str::DRAWINGML_MAIN_NS, "grpSpLocks") {
            return Err(crate::xml::xml_error("invalid DrawingML lock owner"));
        }
        self.set_attributes(reader, owner)?;
        if empty {
            return Ok(());
        }
        let end = owner.name().as_ref().to_vec();
        let mut buffer = Vec::new();
        loop {
            match reader.read_event_into(&mut buffer)? {
                ref event @ (Event::Start(ref child) | Event::Empty(ref child)) => {
                    let name = scope
                        .child(child)
                        .and_then(|scope| scope.resolve(child.name().as_ref(), false))
                        .map_err(crate::xml::xml_error)?;
                    if !name.is(crate::helper::const_str::DRAWINGML_MAIN_NS, "extLst")
                        || self.extensions.is_some()
                    {
                        return Err(crate::xml::xml_error(
                            "unsupported or duplicate DrawingML lock child",
                        ));
                    }
                    self.extensions = Some(Box::new(
                        crate::drawing::ExtensionList::read(
                            reader,
                            child,
                            matches!(event, Event::Empty(_)),
                            &scope,
                        )
                        .map_err(crate::xml::xml_error)?,
                    ));
                }
                Event::End(child) if child.name().as_ref() == end.as_slice() => return Ok(()),
                Event::Text(text)
                    if text
                        .iter()
                        .all(|byte| matches!(byte, b' ' | b'\t' | b'\r' | b'\n')) => {}
                Event::Comment(_) | Event::PI(_) => (),
                Event::Eof => return Err(crate::xml::xml_error("unterminated DrawingML lock")),
                _ => return Err(crate::xml::xml_error("unexpected DrawingML lock content")),
            }
            buffer.clear();
        }
    }

    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
    ) -> Result<(), crate::XlsxError> {
        self.write_with_relationships(writer, &[])
    }

    pub(crate) fn write_with_relationships(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        relationships: &[(String, String)],
    ) -> Result<(), crate::XlsxError> {
        // a:grpSpLocks
        let mut attributes: crate::structs::AttrCollection = Vec::new();

        let no_change_aspect_str = self.no_change_aspect.value_string();
        if self.no_change_aspect.has_value() {
            attributes.push(("noChangeAspect", no_change_aspect_str).into());
        }

        let no_grouping_str = self.no_grouping.value_string();
        if self.no_grouping.has_value() {
            attributes.push(("noGrp", no_grouping_str).into());
        }

        let no_move_str = self.no_move.value_string();
        if self.no_move.has_value() {
            attributes.push(("noMove", no_move_str).into());
        }

        let no_resize_str = self.no_resize.value_string();
        if self.no_resize.has_value() {
            attributes.push(("noResize", no_resize_str).into());
        }

        let no_rotation_str = self.no_rotation.value_string();
        if self.no_rotation.has_value() {
            attributes.push(("noRot", no_rotation_str).into());
        }

        let no_selection_str = self.no_selection.value_string();
        if self.no_selection.has_value() {
            attributes.push(("noSelect", no_selection_str).into());
        }

        let no_ungrouping_str = self.no_ungrouping.value_string();
        if self.no_ungrouping.has_value() {
            attributes.push(("noUngrp", no_ungrouping_str).into());
        }

        write_start_tag(
            writer,
            "a:grpSpLocks",
            attributes,
            self.extensions.is_none(),
        );
        if let Some(extensions) = &self.extensions {
            extensions.write_with_relationships(writer, relationships)?;
            crate::writer::driver::write_end_tag(writer, "a:grpSpLocks");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn every_modeled_group_lock_is_checked_and_preserves_presence() {
        for name in [
            "noChangeAspect",
            "noGrp",
            "noMove",
            "noResize",
            "noRot",
            "noSelect",
            "noUngrp",
        ] {
            for lexical in [" &#x31; ", "false"] {
                let xml = format!(r#"<a:grpSpLocks {name}="{lexical}"/>"#);
                let mut reader = Reader::from_reader(xml.as_bytes());
                let Event::Empty(start) = reader.read_event().unwrap() else {
                    panic!("empty lock required");
                };
                let mut value = GroupShapeLocks::default();
                value.set_attributes(&mut reader, &start).unwrap();
                let mut writer = Writer::new(Cursor::new(Vec::new()));
                value.write_to(&mut writer).unwrap();
                let output = String::from_utf8(writer.into_inner().into_inner()).unwrap();
                assert!(
                    output.contains(&format!(
                        "{name}=\"{}\"",
                        if lexical == "false" { "0" } else { "1" }
                    )),
                    "{output}"
                );
                assert_eq!(
                    output.matches('=').count(),
                    1,
                    "absent locks materialized: {output}"
                );
            }
            let xml = format!(r#"<a:grpSpLocks {name}="bad"/>"#);
            let mut reader = Reader::from_reader(xml.as_bytes());
            let Event::Empty(start) = reader.read_event().unwrap() else {
                panic!("empty lock required");
            };
            assert!(
                GroupShapeLocks::default()
                    .set_attributes(&mut reader, &start)
                    .is_err(),
                "{name}"
            );
        }
    }
}

#[cfg(test)]
mod extension_tests {
    use super::*;
    use crate::drawing::charts::NamespaceScope;

    fn parsed(content: &str) -> Result<GroupShapeLocks, crate::XlsxError> {
        let source = format!(
            r#"<root xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:f="urn:future" xmlns:q="urn:qualified" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><a:grpSpLocks>{content}</a:grpSpLocks></root>"#
        );
        let mut reader = Reader::from_reader(source.as_bytes());
        let Event::Start(root) = reader.read_event().unwrap() else {
            panic!("root required");
        };
        let scope = NamespaceScope::default().child(&root).unwrap();
        let Event::Start(owner) = reader.read_event().unwrap() else {
            panic!("lock required");
        };
        let mut value = GroupShapeLocks::default();
        value.read_with_scope(&mut reader, &owner, false, &scope)?;
        Ok(value)
    }

    #[test]
    fn lock_extensions_keep_qualified_values_and_borrowed_relationships() {
        let mut value = parsed(r#"<a:extLst><a:ext uri="urn:test"><f:payload type="q:Kind" r:id="rId9"/></a:ext></a:extLst>"#).unwrap();
        assert!(value.extensions().is_some());
        let mut ids = Vec::new();
        value.visit_relationships(&mut |_, id| ids.push(id.to_owned()));
        assert_eq!(ids, ["rId9"]);
        value.visit_relationships_mut(&mut |_, id| *id = "rId10".into());
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        value.write_to(&mut writer).unwrap();
        let output = String::from_utf8(writer.into_inner().into_inner()).unwrap();
        assert!(output.contains("q:Kind"), "{output}");
        assert!(output.contains("urn:qualified"), "{output}");
        assert!(output.contains("rId10"), "{output}");
    }

    #[test]
    fn lock_extension_children_are_checked() {
        for content in [
            "<a:extLst/><a:extLst/>",
            "<f:extLst/>",
            "<a:unknown/>",
            "<a:extLst><a:ext/></a:extLst>",
            "text",
        ] {
            assert!(parsed(content).is_err(), "{content}");
        }
    }
}
