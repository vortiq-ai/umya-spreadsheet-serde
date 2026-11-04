// a:picLocks
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
    writer::driver::write_start_tag,
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct PictureLocks {    no_change_aspect:     bool,

    #[cfg_attr(feature = "serde", serde(default))]
    extensions:           Option<Box<crate::drawing::ExtensionList>>,
    #[cfg_attr(feature = "serde", serde(default))]
    no_grouping:          crate::structs::BooleanValue,
    #[cfg_attr(feature = "serde", serde(default))]
    no_selection:         crate::structs::BooleanValue,
    #[cfg_attr(feature = "serde", serde(default))]
    no_rotation:          crate::structs::BooleanValue,
    #[cfg_attr(feature = "serde", serde(default))]
    no_move:              crate::structs::BooleanValue,
    #[cfg_attr(feature = "serde", serde(default))]
    no_resize:            crate::structs::BooleanValue,
    #[cfg_attr(feature = "serde", serde(default))]
    no_edit_points:       crate::structs::BooleanValue,
    #[cfg_attr(feature = "serde", serde(default))]
    no_adjust_handles:    crate::structs::BooleanValue,
    #[cfg_attr(feature = "serde", serde(default))]
    no_change_arrowheads: crate::structs::BooleanValue,
    #[cfg_attr(feature = "serde", serde(default))]
    no_change_shape_type: crate::structs::BooleanValue,
    #[cfg_attr(feature = "serde", serde(default))]
    no_crop:              crate::structs::BooleanValue,
}

impl PictureLocks {
    #[inline]
    #[must_use]
    pub fn no_change_aspect(&self) -> bool {
        self.no_change_aspect
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use no_change_aspect()")]
    pub fn get_no_change_aspect(&self) -> bool {
        self.no_change_aspect()
    }

    #[inline]
    pub fn set_no_change_aspect(&mut self, value: bool) -> &mut Self {
        self.no_change_aspect = value;
        self
    }

    #[must_use]
    pub fn no_grouping(&self) -> bool {
        self.no_grouping.value()
    }

    pub fn set_no_grouping(&mut self, value: bool) -> &mut Self {
        self.no_grouping.set_value(value);
        self
    }

    #[must_use]
    pub fn no_selection(&self) -> bool {
        self.no_selection.value()
    }

    pub fn set_no_selection(&mut self, value: bool) -> &mut Self {
        self.no_selection.set_value(value);
        self
    }

    #[must_use]
    pub fn no_rotation(&self) -> bool {
        self.no_rotation.value()
    }

    pub fn set_no_rotation(&mut self, value: bool) -> &mut Self {
        self.no_rotation.set_value(value);
        self
    }

    #[must_use]
    pub fn no_move(&self) -> bool {
        self.no_move.value()
    }

    pub fn set_no_move(&mut self, value: bool) -> &mut Self {
        self.no_move.set_value(value);
        self
    }

    #[must_use]
    pub fn no_resize(&self) -> bool {
        self.no_resize.value()
    }

    pub fn set_no_resize(&mut self, value: bool) -> &mut Self {
        self.no_resize.set_value(value);
        self
    }

    #[must_use]
    pub fn no_edit_points(&self) -> bool {
        self.no_edit_points.value()
    }

    pub fn set_no_edit_points(&mut self, value: bool) -> &mut Self {
        self.no_edit_points.set_value(value);
        self
    }

    #[must_use]
    pub fn no_adjust_handles(&self) -> bool {
        self.no_adjust_handles.value()
    }

    pub fn set_no_adjust_handles(&mut self, value: bool) -> &mut Self {
        self.no_adjust_handles.set_value(value);
        self
    }

    #[must_use]
    pub fn no_change_arrowheads(&self) -> bool {
        self.no_change_arrowheads.value()
    }

    pub fn set_no_change_arrowheads(&mut self, value: bool) -> &mut Self {
        self.no_change_arrowheads.set_value(value);
        self
    }

    #[must_use]
    pub fn no_change_shape_type(&self) -> bool {
        self.no_change_shape_type.value()
    }

    pub fn set_no_change_shape_type(&mut self, value: bool) -> &mut Self {
        self.no_change_shape_type.set_value(value);
        self
    }

    #[must_use]
    pub fn no_crop(&self) -> bool {
        self.no_crop.value()
    }

    pub fn set_no_crop(&mut self, value: bool) -> &mut Self {
        self.no_crop.set_value(value);
        self
    }

    #[inline]
    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        _reader: &mut Reader<R>,
        e: &BytesStart,
    ) -> Result<(), crate::XlsxError> {
        if let Some(value) = read_attribute(e, b"noChangeAspect")? {
            self.set_no_change_aspect(
                crate::xml::parse_bool(&value).map_err(crate::xml::xml_error)?,
            );
        }
        for (name, field) in [
            (b"noGrp".as_slice(), &mut self.no_grouping),
            (b"noSelect".as_slice(), &mut self.no_selection),
            (b"noRot".as_slice(), &mut self.no_rotation),
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
            (b"noCrop".as_slice(), &mut self.no_crop),
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
        if !name.is(crate::helper::const_str::DRAWINGML_MAIN_NS, "picLocks") {
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

    #[inline]
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
        // a:picLocks
        let no_change_aspect = if self.no_change_aspect { "1" } else { "0" };
        let mut attributes: crate::structs::AttrCollection =
            vec![("noChangeAspect", no_change_aspect).into()];
        if self.no_grouping.has_value() {
            attributes.push(("noGrp", self.no_grouping.value_string()).into());
        }
        if self.no_selection.has_value() {
            attributes.push(("noSelect", self.no_selection.value_string()).into());
        }
        if self.no_rotation.has_value() {
            attributes.push(("noRot", self.no_rotation.value_string()).into());
        }
        if self.no_move.has_value() {
            attributes.push(("noMove", self.no_move.value_string()).into());
        }
        if self.no_resize.has_value() {
            attributes.push(("noResize", self.no_resize.value_string()).into());
        }
        if self.no_edit_points.has_value() {
            attributes.push(("noEditPoints", self.no_edit_points.value_string()).into());
        }
        if self.no_adjust_handles.has_value() {
            attributes.push(("noAdjustHandles", self.no_adjust_handles.value_string()).into());
        }
        if self.no_change_arrowheads.has_value() {
            attributes.push(
                (
                    "noChangeArrowheads",
                    self.no_change_arrowheads.value_string(),
                )
                    .into(),
            );
        }
        if self.no_change_shape_type.has_value() {
            attributes.push(
                (
                    "noChangeShapeType",
                    self.no_change_shape_type.value_string(),
                )
                    .into(),
            );
        }
        if self.no_crop.has_value() {
            attributes.push(("noCrop", self.no_crop.value_string()).into());
        }
        write_start_tag(writer, "a:picLocks", attributes, self.extensions.is_none());
        if let Some(extensions) = &self.extensions {
            extensions.write_with_relationships(writer, relationships)?;
            crate::writer::driver::write_end_tag(writer, "a:picLocks");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn picture_lock_booleans_are_checked_and_false_writes_zero() {
        for (lexical, expected) in [
            ("true", true),
            (" &#x31; ", true),
            ("false", false),
            ("0", false),
        ] {
            let xml = format!(r#"<a:picLocks noChangeAspect="{lexical}"/>"#);
            let mut reader = Reader::from_reader(xml.as_bytes());
            let Event::Empty(start) = reader.read_event().unwrap() else {
                panic!("empty lock required");
            };
            let mut value = PictureLocks::default();
            value.set_attributes(&mut reader, &start).unwrap();
            assert_eq!(value.no_change_aspect(), expected);
            let mut writer = Writer::new(Cursor::new(Vec::new()));
            value.write_to(&mut writer).unwrap();
            let output = String::from_utf8(writer.into_inner().into_inner()).unwrap();
            assert!(
                output.contains(if expected {
                    r#"noChangeAspect="1""#
                } else {
                    r#"noChangeAspect="0""#
                }),
                "{output}"
            );
        }
        for attrs in [
            r#"noChangeAspect="2""#,
            r#"noChangeAspect="&missing;""#,
            r#"noChangeAspect="1" noChangeAspect="0""#,
        ] {
            let xml = format!("<a:picLocks {attrs}/>");
            let mut reader = Reader::from_reader(xml.as_bytes());
            let Event::Empty(start) = reader.read_event().unwrap() else {
                panic!("empty lock required");
            };
            assert!(
                PictureLocks::default()
                    .set_attributes(&mut reader, &start)
                    .is_err()
            );
        }
    }
}

#[cfg(test)]
mod extension_tests {
    use super::*;
    use crate::drawing::charts::NamespaceScope;

    fn parsed(content: &str) -> Result<PictureLocks, crate::XlsxError> {
        let source = format!(
            r#"<root xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:f="urn:future" xmlns:q="urn:qualified" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><a:picLocks>{content}</a:picLocks></root>"#
        );
        let mut reader = Reader::from_reader(source.as_bytes());
        let Event::Start(root) = reader.read_event().unwrap() else {
            panic!("root required");
        };
        let scope = NamespaceScope::default().child(&root).unwrap();
        let Event::Start(owner) = reader.read_event().unwrap() else {
            panic!("lock required");
        };
        let mut value = PictureLocks::default();
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

#[cfg(test)]
mod flag_tests {
    use super::*;

    #[test]
    fn all_picture_lock_flags_preserve_explicit_false_and_reject_invalid_booleans() {
        for name in [
            "noGrp",
            "noSelect",
            "noRot",
            "noMove",
            "noResize",
            "noEditPoints",
            "noAdjustHandles",
            "noChangeArrowheads",
            "noChangeShapeType",
            "noCrop",
        ] {
            let source = format!(r#"<a:picLocks {name}="false"/>"#);
            let mut reader = Reader::from_reader(source.as_bytes());
            let Event::Empty(owner) = reader.read_event().unwrap() else {
                panic!("empty lock required");
            };
            let mut value = PictureLocks::default();
            value.set_attributes(&mut reader, &owner).unwrap();
            let mut writer = Writer::new(Cursor::new(Vec::new()));
            value.write_to(&mut writer).unwrap();
            let output = String::from_utf8(writer.into_inner().into_inner()).unwrap();
            assert!(output.contains(&format!(r#"{name}="0""#)), "{output}");
            assert_eq!(
                output.matches('=').count(),
                2,
                "absent flag materialized: {output}"
            );
            let source = format!(r#"<a:picLocks {name}="bad"/>"#);
            let mut reader = Reader::from_reader(source.as_bytes());
            let Event::Empty(owner) = reader.read_event().unwrap() else {
                panic!("empty lock required");
            };
            assert!(
                PictureLocks::default()
                    .set_attributes(&mut reader, &owner)
                    .is_err(),
                "{name}"
            );
        }
    }
}
