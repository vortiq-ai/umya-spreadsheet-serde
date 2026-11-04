// c:txPr
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::{
        BytesStart,
        Event,
    },
};

use super::super::{
    BodyProperties,
    ListStyle,
    Paragraph,
};
use crate::{
    XlsxError,
    helper::const_str::DRAWINGML_MAIN_NS as NS_A,
    reader::driver::{
        local_name,
        try_xml_read_loop,
    },
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct TextProperties {
    body_properties: BodyProperties,
    list_style:      ListStyle,
    paragraph:       Vec<Paragraph>,
    #[cfg_attr(feature = "serde", serde(default))]
    parts:           Option<Box<TextBodyParts>>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct TextBodyParts {
    pub(super) list_style_present: bool,
}

impl TextProperties {
    /// Incoming structural metadata cannot suppress editable formatting.
    pub(crate) fn discard_incoming_export_parts(&mut self) {
        self.parts = None;
        for paragraph in &mut self.paragraph {
            paragraph.discard_incoming_export_parts();
        }
    }

    pub(crate) fn discard_fields(&mut self) {
        for paragraph in &mut self.paragraph {
            paragraph.discard_fields();
        }
    }

    pub(crate) fn strip_export_parts(&mut self) {
        self.parts = None;
        self.body_properties.strip_export_parts();
        self.list_style.strip_export_parts();
        for paragraph in &mut self.paragraph {
            paragraph.strip_export_parts();
        }
    }

    pub(crate) fn projected_structure_matches(&self, previous: &Self) -> bool {
        self.paragraph.len() == previous.paragraph.len()
            && self
                .paragraph
                .iter()
                .zip(&previous.paragraph)
                .all(|(current, previous)| current.projected_structure_matches(previous))
    }

    pub(crate) fn restore_export_parts(&mut self, previous: &Self) -> Result<usize, XlsxError> {
        if !self.projected_structure_matches(previous) {
            return Ok(0);
        }
        let mut restored = 0;
        if self.parts.is_none() {
            if let Some(parts) = &previous.parts {
                let mut parts = parts.clone();
                if !self
                    .list_style
                    .projected_values_match(&previous.list_style)?
                {
                    parts.list_style_present = true;
                }
                self.parts = Some(parts);
                restored += 1;
            }
        }
        restored += self
            .body_properties
            .restore_export_parts(&previous.body_properties);
        restored += self.list_style.restore_export_parts(&previous.list_style);
        for (current, previous) in self.paragraph.iter_mut().zip(&previous.paragraph) {
            restored += current.restore_export_parts(previous)?;
        }
        Ok(restored)
    }

    #[must_use]
    pub fn body_properties(&self) -> &BodyProperties {
        &self.body_properties
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use body_properties()")]
    pub fn get_body_properties(&self) -> &BodyProperties {
        self.body_properties()
    }

    pub fn body_properties_mut(&mut self) -> &mut BodyProperties {
        &mut self.body_properties
    }

    #[deprecated(since = "3.0.0", note = "Use body_properties_mut()")]
    pub fn get_body_properties_mut(&mut self) -> &mut BodyProperties {
        self.body_properties_mut()
    }

    pub fn set_body_properties(&mut self, value: BodyProperties) -> &mut TextProperties {
        self.body_properties = value;
        self
    }

    #[must_use]
    pub fn list_style(&self) -> &ListStyle {
        &self.list_style
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use list_style()")]
    pub fn get_list_style(&self) -> &ListStyle {
        self.list_style()
    }

    pub fn list_style_mut(&mut self) -> &mut ListStyle {
        self.parts
            .get_or_insert_with(Default::default)
            .list_style_present = true;
        &mut self.list_style
    }

    #[deprecated(since = "3.0.0", note = "Use list_style_mut()")]
    pub fn get_list_style_mut(&mut self) -> &mut ListStyle {
        self.list_style_mut()
    }

    pub fn set_list_style(&mut self, value: ListStyle) -> &mut TextProperties {
        self.parts
            .get_or_insert_with(Default::default)
            .list_style_present = true;
        self.list_style = value;
        self
    }

    #[must_use]
    pub fn paragraph(&self) -> &[Paragraph] {
        &self.paragraph
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use paragraph()")]
    pub fn get_paragraph(&self) -> &[Paragraph] {
        self.paragraph()
    }

    pub fn paragraph_mut(&mut self) -> &mut Vec<Paragraph> {
        &mut self.paragraph
    }

    #[deprecated(since = "3.0.0", note = "Use paragraph_mut()")]
    pub fn get_paragraph_mut(&mut self) -> &mut Vec<Paragraph> {
        self.paragraph_mut()
    }

    pub fn add_paragraph(&mut self, value: Paragraph) -> &mut TextProperties {
        self.paragraph.push(value);
        self
    }

    pub(crate) fn read_with_session<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
        session: Option<&crate::reader::xlsx::SourceCacheSession>,
    ) -> Result<(), XlsxError> {
        let _scope_guard = session
            .map(|session| session.enter_child_scope(e))
            .transpose()?
            .flatten();
        crate::reader::driver::validate_attributes(e)?;
        self.parts = Some(Box::new(TextBodyParts::default()));
        let mut body_present = false;
        let mut list_present = false;
        let end = e.name().as_ref().to_vec();
        try_xml_read_loop!(reader,
            ref event @ (Event::Start(ref e) | Event::Empty(ref e)) => {
                let empty = matches!(event, Event::Empty(_));
                let native = match session {
                    Some(session) => session.accepts_child_namespace(e, NS_A)?,
                    None => true,
                };
                if !native {
                    if !empty { crate::reader::driver::skip_element(reader, e)?; }
                    continue;
                }
                match local_name(e.name().as_ref()) {
                    b"p" => {
                        if !body_present {
                            return Err(crate::xml::xml_error("DrawingML paragraph precedes body properties"));
                        }
                        let mut paragraph = Paragraph::default();
                        if empty {
                            crate::reader::driver::validate_attributes(e)?;
                            paragraph.mark_empty_import();
                        } else {
                            paragraph.read_with_session(reader, e, session)?;
                        }
                        self.add_paragraph(paragraph);
                    }
                    b"lstStyle" => {
                        if !body_present || list_present || !self.paragraph.is_empty() {
                            return Err(crate::xml::xml_error("duplicate or misplaced DrawingML list style"));
                        }
                        list_present = true;
                        if let Some(parts) = &mut self.parts { parts.list_style_present = true; }
                        if empty {
                            crate::reader::driver::validate_attributes(e)?;
                        } else {
                            self.list_style.read_with_session(reader, e, session)?;
                        }
                    }
                    b"bodyPr" => {
                        if body_present || !self.paragraph.is_empty() {
                            return Err(crate::xml::xml_error("duplicate or misplaced DrawingML body properties"));
                        }
                        body_present = true;
                        let mut properties = BodyProperties::default();
                        properties.read_with_session(reader, e, empty, session)?;
                        self.set_body_properties(properties);
                    }
                    _ => if !empty { crate::reader::driver::skip_element(reader, e)?; },
                }
            },
            Event::End(ref e) if e.name().as_ref() == end.as_slice() => {
                if !body_present || self.paragraph.is_empty() {
                    return Err(crate::xml::xml_error("DrawingML text body requires body properties and a paragraph"));
                }
                return Ok(());
            },
            Event::Text(ref text)
                if !text.iter().all(|byte| matches!(byte, b' ' | b'\t' | b'\r' | b'\n')) =>
            {
                return Err(crate::xml::xml_error("unexpected text in DrawingML text body"));
            },
            Event::Eof => return Err(crate::xml::xml_error("unterminated DrawingML text body")),
        );
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) -> Result<(), XlsxError> {
        // c:txPr
        write_start_tag(writer, "c:txPr", vec![], false);

        // a:bodyPr
        self.body_properties.write_to(writer)?;

        // a:lstStyle
        if self
            .parts
            .as_ref()
            .is_none_or(|parts| parts.list_style_present)
        {
            self.list_style.write_to(writer)?;
        }

        // a:p
        for content in &self.paragraph {
            content.write_to(writer)?;
        }

        write_end_tag(writer, "c:txPr");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    // Tests for master's `parts` preservation (BodyProperties vertical/anchorCtr and the
    // supplement merge) are absent: this fork keeps no preserved XML, so they cannot pass
    // here. They return with the deferred in-chart text fidelity work.
    use super::*;
    fn roundtrip(xml: &str) -> String {
        let mut reader = Reader::from_reader(xml.as_bytes());
        let mut buf = Vec::new();
        let start = match reader.read_event_into(&mut buf).unwrap() {
            Event::Start(e) => e.into_owned(),
            _ => panic!("missing text properties"),
        };
        let mut value = TextProperties::default();
        value.read_with_session(&mut reader, &start, None).unwrap();
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        value.write_to(&mut writer).unwrap();
        String::from_utf8(writer.into_inner().into_inner()).unwrap()
    }
}

#[cfg(all(test, feature = "serde"))]
mod projection_tests {
    use super::*;
    use crate::drawing::{
        ParagraphProperties,
        Run,
        RunProperties,
        TextField,
        TextUnderline,
    };

    // Derive expected metadata slots from the complete model, independently
    // of the projection implementation.
    fn clear_supplement_slots(value: &mut serde_json::Value) -> usize {
        match value {
            serde_json::Value::Object(fields) => fields
                .iter_mut()
                .map(|(name, value)| {
                    if name == "parts" {
                        let present = usize::from(!value.is_null());
                        *value = serde_json::Value::Null;
                        present
                    } else {
                        clear_supplement_slots(value)
                    }
                })
                .sum(),
            serde_json::Value::Array(values) => values.iter_mut().map(clear_supplement_slots).sum(),
            _ => 0,
        }
    }

    fn assert_projection<T>(mut value: T, strip: impl Fn(&mut T))
    where
        T: serde::Serialize + serde::de::DeserializeOwned,
    {
        let mut expected = serde_json::to_value(&value).unwrap();
        assert!(clear_supplement_slots(&mut expected) > 0);
        let expected_native: T = serde_json::from_value(expected.clone()).unwrap();
        let expected_bytes = bincode::serialize(&expected_native).unwrap();
        strip(&mut value);
        assert_eq!(serde_json::to_value(&value).unwrap(), expected);
        assert_eq!(bincode::serialize(&value).unwrap(), expected_bytes);
        strip(&mut value);
        assert_eq!(serde_json::to_value(&value).unwrap(), expected);
        assert_eq!(bincode::serialize(&value).unwrap(), expected_bytes);
    }

    fn run_properties() -> RunProperties {
        let mut properties = RunProperties::default();
        properties
            .set_language("en-US")
            .set_bold("1")
            .set_sz("1200");
        properties.set_underline(Some(TextUnderline::Double));
        properties.set_kerning(Some(1100));
        properties.set_baseline(Some(-200));
        properties
    }

    fn paragraph() -> Paragraph {
        let mut paragraph = Paragraph::default();
        paragraph.paragraph_properties_mut().set_level(Some(3));
        paragraph
            .paragraph_properties_mut()
            .set_right_to_left("0")
            .set_default_run_properties(run_properties());
        for text in ["first", "second"] {
            let mut run = Run::default();
            run.set_text(text);
            run.set_run_properties(run_properties());
            paragraph.add_run(run);
        }
        paragraph.set_end_para_run_properties(run_properties());
        assert!(paragraph.add_field(
            1,
            TextField {
                id: "{12345678-1234-1234-1234-123456789ABC}".into(),
                kind: Some("VALUE".into()),
                text: Some("hidden field".into()),
                ..TextField::default()
            },
        ));
        paragraph
    }
    #[test]
    fn run_property_projection_preserves_existing_formatting() {
        assert_projection(run_properties(), RunProperties::strip_export_parts);
    }

    #[test]
    fn paragraph_property_projection_strips_nested_default_run_parts() {
        let mut properties = ParagraphProperties::default();
        properties.set_right_to_left("1");
        assert!(properties.set_level(Some(5)));
        properties.set_default_run_properties(run_properties());
        assert_projection(properties, ParagraphProperties::strip_export_parts);
    }

    #[test]
    fn run_projection_removes_import_presence_without_changing_text() {
        let xml = "<a:r><a:rPr lang=\"en-US\" b=\"1\" u=\"dbl\"/><a:t>unchanged text</a:t></a:r>";
        let mut reader = Reader::from_reader(xml.as_bytes());
        let mut buf = Vec::new();
        let Event::Start(start) = reader.read_event_into(&mut buf).unwrap() else {
            panic!("missing run");
        };
        let mut run = Run::default();
        run.read_with_session(&mut reader, &start.into_owned(), None)
            .unwrap();
        assert_projection(run, Run::strip_export_parts);
    }

    #[test]
    fn paragraph_projection_removes_fields_and_keeps_run_order() {
        let value = paragraph();
        assert_eq!(value.fields().len(), 1);
        assert_eq!(value.run()[0].text(), "first");
        assert_eq!(value.run()[1].text(), "second");
        assert_projection(value, Paragraph::strip_export_parts);
    }
    fn imported_text_properties() -> TextProperties {
        let xml = concat!(
            "<c:txPr><a:bodyPr vert=\"vert270\" anchorCtr=\"false\" rot=\"60000\"/>",
            "<a:lstStyle><a:lvl1pPr><a:defRPr u=\"dbl\"/></a:lvl1pPr></a:lstStyle>",
            "<a:p><a:pPr lvl=\"2\"><a:defRPr u=\"dbl\" kern=\"1200\"/></a:pPr>",
            "<a:r><a:rPr b=\"1\" baseline=\"-200\"/><a:t>first</a:t></a:r>",
            "<a:fld id=\"{12345678-1234-1234-1234-123456789ABC}\"><a:t>field</a:t></a:fld>",
            "<a:r><a:t>second</a:t></a:r><a:endParaRPr u=\"sng\"/></a:p></c:txPr>",
        );
        let mut reader = Reader::from_reader(xml.as_bytes());
        let mut buf = Vec::new();
        let Event::Start(start) = reader.read_event_into(&mut buf).unwrap() else {
            panic!("missing text properties");
        };
        let mut value = TextProperties::default();
        value
            .read_with_session(&mut reader, &start.into_owned(), None)
            .unwrap();
        value
    }

    #[test]
    fn imported_text_projection_restores_exact_storage_bytes() {
        let original = imported_text_properties();
        let mut projected = original.clone();
        projected.strip_export_parts();
        assert!(projected.paragraph()[0].fields().is_empty());
        assert!(projected.restore_export_parts(&original).unwrap() > 0);
        assert_eq!(
            serde_json::to_value(&projected).unwrap(),
            serde_json::to_value(&original).unwrap()
        );
        assert_eq!(
            bincode::serialize(&projected).unwrap(),
            bincode::serialize(&original).unwrap()
        );
        assert_eq!(projected.restore_export_parts(&original).unwrap(), 0);
    }

    #[test]
    fn rich_text_projection_restores_exact_storage_bytes() {
        let original: super::super::RichText =
            serde_json::from_value(serde_json::to_value(imported_text_properties()).unwrap())
                .unwrap();
        let mut projected = original.clone();
        projected.strip_export_parts();
        assert!(projected.restore_export_parts(&original).unwrap() > 0);
        assert_eq!(
            bincode::serialize(&projected).unwrap(),
            bincode::serialize(&original).unwrap()
        );
        assert_eq!(projected.restore_export_parts(&original).unwrap(), 0);
    }

    #[test]
    fn replaced_run_text_cannot_regain_fields_or_run_supplements() {
        let original = imported_text_properties();
        let mut projected = original.clone();
        projected.strip_export_parts();
        let mut replacement = Run::default();
        replacement.set_text("replacement");
        assert!(projected.paragraph_mut()[0].remove_run(0).is_some());
        assert!(projected.paragraph_mut()[0].insert_run(0, replacement));
        let before = bincode::serialize(&projected).unwrap();
        assert!(!projected.projected_structure_matches(&original));
        assert_eq!(projected.restore_export_parts(&original).unwrap(), 0);
        assert_eq!(bincode::serialize(&projected).unwrap(), before);
        assert!(projected.paragraph()[0].fields().is_empty());
    }

    #[test]
    fn replacing_paragraph_collection_cannot_regain_text_metadata() {
        let original: super::super::RichText =
            serde_json::from_value(serde_json::to_value(imported_text_properties()).unwrap())
                .unwrap();
        let mut projected = original.clone();
        projected.strip_export_parts();
        projected.set_text("replacement title");
        let before = bincode::serialize(&projected).unwrap();
        assert_eq!(projected.restore_export_parts(&original).unwrap(), 0);
        assert_eq!(bincode::serialize(&projected).unwrap(), before);
    }

    #[test]
    fn editable_text_style_values_win_over_restored_metadata() {
        let original = imported_text_properties();
        let mut projected = original.clone();
        projected.strip_export_parts();
        projected.body_properties_mut().set_rotation(120000);
        let mut changed_run = projected.paragraph()[0].run()[0].clone();
        changed_run
            .run_properties_mut()
            .set_bold("0")
            .set_italic("1");
        assert!(projected.paragraph_mut()[0].remove_run(0).is_some());
        assert!(projected.paragraph_mut()[0].insert_run(0, changed_run));
        assert!(projected.restore_export_parts(&original).unwrap() > 0);
        assert_eq!(projected.body_properties().rotation(), 120000);
        assert_eq!(
            projected.paragraph()[0].run()[0].run_properties().bold(),
            "0"
        );
        assert_eq!(
            projected.paragraph()[0].run()[0].run_properties().italic(),
            "1"
        );
        assert_eq!(
            projected.paragraph()[0].run()[0]
                .run_properties()
                .baseline(),
            Some(-200)
        );
        assert_eq!(projected.paragraph()[0].fields().len(), 1);
    }

    #[test]
    fn omitted_imported_property_elements_cannot_hide_new_style() {
        let xml = "<c:txPr><a:bodyPr/><a:p><a:r><a:t>same</a:t></a:r></a:p></c:txPr>";
        let mut reader = Reader::from_reader(xml.as_bytes());
        let mut buf = Vec::new();
        let Event::Start(start) = reader.read_event_into(&mut buf).unwrap() else {
            panic!("missing text properties");
        };
        let mut original = TextProperties::default();
        original
            .read_with_session(&mut reader, &start.into_owned(), None)
            .unwrap();
        let mut projected = original.clone();
        projected.strip_export_parts();
        let paragraph = &mut projected.paragraph_mut()[0];
        paragraph.paragraph_properties_mut().set_right_to_left("1");
        let mut run = paragraph.run()[0].clone();
        run.run_properties_mut().set_bold("1");
        assert!(paragraph.remove_run(0).is_some());
        assert!(paragraph.insert_run(0, run));
        assert!(projected.restore_export_parts(&original).unwrap() > 0);
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        projected.write_to(&mut writer).unwrap();
        let xml = String::from_utf8(writer.into_inner().into_inner()).unwrap();
        assert!(xml.contains("<a:pPr rtl=\"1\""), "{xml}");
        assert!(xml.contains("<a:rPr b=\"1\""), "{xml}");
    }
    #[test]
    fn restoring_empty_text_owners_does_not_materialize_parts() {
        let original = TextProperties::default();
        let mut current = original.clone();
        let before = bincode::serialize(&current).unwrap();
        assert_eq!(current.restore_export_parts(&original).unwrap(), 0);
        assert_eq!(bincode::serialize(&current).unwrap(), before);
    }
    #[test]
    fn run_supplement_edit_keeps_unrelated_attributes() {
        let mut previous = run_properties();
        previous.set_underline_uses_text_fill(true);
        let mut current = previous.clone();
        current.strip_export_parts();
        current.set_underline(Some(TextUnderline::Single));
        assert_eq!(current.restore_export_parts(&previous), 1);
        assert_eq!(current.underline(), Some(TextUnderline::Single));
        assert_eq!(current.kerning(), Some(1100));
        assert_eq!(current.baseline(), Some(-200));
        assert!(current.underline_uses_text_fill());
        assert_eq!(current.restore_export_parts(&previous), 0);
    }
    #[test]
    fn supplement_only_edits_survive_omitted_property_restore() {
        let xml = "<c:txPr><a:bodyPr/><a:p><a:r><a:t>same</a:t></a:r></a:p></c:txPr>";
        let mut reader = Reader::from_reader(xml.as_bytes());
        let mut buf = Vec::new();
        let Event::Start(start) = reader.read_event_into(&mut buf).unwrap() else {
            panic!("missing text properties");
        };
        let mut original = TextProperties::default();
        original
            .read_with_session(&mut reader, &start.into_owned(), None)
            .unwrap();
        let mut current = original.clone();
        current.strip_export_parts();
        assert!(
            current.paragraph_mut()[0]
                .paragraph_properties_mut()
                .set_level(Some(0))
        );
        let mut run = current.paragraph()[0].run()[0].clone();
        run.run_properties_mut()
            .set_underline(Some(TextUnderline::Double));
        assert!(current.paragraph_mut()[0].remove_run(0).is_some());
        assert!(current.paragraph_mut()[0].insert_run(0, run));
        assert!(current.restore_export_parts(&original).unwrap() > 0);
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        current.write_to(&mut writer).unwrap();
        let xml = String::from_utf8(writer.into_inner().into_inner()).unwrap();
        assert!(xml.contains("<a:pPr lvl=\"0\""), "{xml}");
        assert!(xml.contains("<a:rPr u=\"dbl\""), "{xml}");
    }

    #[test]
    fn explicit_empty_property_edits_keep_element_presence() {
        let xml = "<c:txPr><a:bodyPr/><a:p><a:r><a:t>same</a:t></a:r></a:p></c:txPr>";
        let mut reader = Reader::from_reader(xml.as_bytes());
        let mut buf = Vec::new();
        let Event::Start(start) = reader.read_event_into(&mut buf).unwrap() else {
            panic!("missing text properties");
        };
        let mut original = TextProperties::default();
        original
            .read_with_session(&mut reader, &start.into_owned(), None)
            .unwrap();
        let mut current = original.clone();
        current.strip_export_parts();
        assert!(
            current.paragraph_mut()[0]
                .paragraph_properties_mut()
                .set_level(None)
        );
        let mut run = current.paragraph()[0].run()[0].clone();
        run.run_properties_mut().set_underline(None);
        assert!(current.paragraph_mut()[0].remove_run(0).is_some());
        assert!(current.paragraph_mut()[0].insert_run(0, run));
        assert!(current.restore_export_parts(&original).unwrap() > 0);
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        current.write_to(&mut writer).unwrap();
        let xml = String::from_utf8(writer.into_inner().into_inner()).unwrap();
        assert!(xml.contains("<a:pPr/>"), "{xml}");
        assert!(xml.contains("<a:rPr/>"), "{xml}");
    }

    #[test]
    fn explicit_empty_list_style_edits_keep_text_property_presence() {
        let xml = "<c:txPr><a:bodyPr/><a:p/></c:txPr>";
        let mut reader = Reader::from_reader(xml.as_bytes());
        let mut buf = Vec::new();
        let Event::Start(start) = reader.read_event_into(&mut buf).unwrap() else {
            panic!("missing text properties");
        };
        let mut original = TextProperties::default();
        original
            .read_with_session(&mut reader, &start.into_owned(), None)
            .unwrap();
        for setter in [false, true] {
            let mut current = original.clone();
            current.strip_export_parts();
            if setter {
                current.set_list_style(ListStyle::default());
            } else {
                current.list_style_mut();
            }
            current.restore_export_parts(&original).unwrap();
            let mut writer = Writer::new(Cursor::new(Vec::new()));
            current.write_to(&mut writer).unwrap();
            let xml = String::from_utf8(writer.into_inner().into_inner()).unwrap();
            assert!(xml.contains("<a:lstStyle/>"), "setter={setter}: {xml}");
        }
    }

    #[test]
    fn explicit_empty_list_style_edits_keep_rich_text_presence() {
        let xml = "<c:rich><a:bodyPr/><a:p/></c:rich>";
        let mut reader = Reader::from_reader(xml.as_bytes());
        let mut buf = Vec::new();
        let Event::Start(start) = reader.read_event_into(&mut buf).unwrap() else {
            panic!("missing rich text");
        };
        let mut original = super::super::RichText::default();
        original
            .read_with_session(&mut reader, &start.into_owned(), None)
            .unwrap();
        for setter in [false, true] {
            let mut current = original.clone();
            current.strip_export_parts();
            if setter {
                current.set_list_style(ListStyle::default());
            } else {
                current.list_style_mut();
            }
            current.restore_export_parts(&original).unwrap();
            let mut writer = Writer::new(Cursor::new(Vec::new()));
            current.write_to(&mut writer).unwrap();
            let xml = String::from_utf8(writer.into_inner().into_inner()).unwrap();
            assert!(xml.contains("<a:lstStyle/>"), "setter={setter}: {xml}");
        }
    }

    #[test]
    fn same_default_property_mutations_keep_explicit_element_presence() {
        let xml = "<c:txPr><a:bodyPr/><a:p><a:r><a:t>same</a:t></a:r></a:p></c:txPr>";
        let mut reader = Reader::from_reader(xml.as_bytes());
        let mut buf = Vec::new();
        let Event::Start(start) = reader.read_event_into(&mut buf).unwrap() else {
            panic!("missing text properties");
        };
        let mut original = TextProperties::default();
        original
            .read_with_session(&mut reader, &start.into_owned(), None)
            .unwrap();
        for setter in [false, true] {
            let mut current = original.clone();
            current.strip_export_parts();
            let mut run = current.paragraph()[0].run()[0].clone();
            if setter {
                current.paragraph_mut()[0].set_paragraph_properties(ParagraphProperties::default());
                run.set_run_properties(RunProperties::default());
            } else {
                current.paragraph_mut()[0].paragraph_properties_mut();
                run.run_properties_mut();
            }
            assert!(current.paragraph_mut()[0].remove_run(0).is_some());
            assert!(current.paragraph_mut()[0].insert_run(0, run));
            current.restore_export_parts(&original).unwrap();
            let mut writer = Writer::new(Cursor::new(Vec::new()));
            current.write_to(&mut writer).unwrap();
            let xml = String::from_utf8(writer.into_inner().into_inner()).unwrap();
            assert!(xml.contains("<a:pPr/>"), "setter={setter}: {xml}");
            assert!(xml.contains("<a:rPr/>"), "setter={setter}: {xml}");
        }
    }

    #[test]
    fn paragraph_property_access_does_not_drop_hidden_positioned_fields() {
        let original = imported_text_properties();
        let mut current = original.clone();
        current.strip_export_parts();
        current.paragraph_mut()[0].paragraph_properties_mut();
        assert!(current.restore_export_parts(&original).unwrap() > 0);
        assert_eq!(current.paragraph()[0].fields().len(), 1);
        assert_eq!(
            bincode::serialize(&current).unwrap(),
            bincode::serialize(&original).unwrap()
        );
    }
}

#[cfg(all(test, feature = "serde"))]
mod paint_error_tests {
    use super::*;
    use crate::drawing::{
        ParagraphProperties,
        PresetColor,
        Run,
        RunProperties,
        SolidFill,
        TextParagraphPropertiesType,
    };

    fn invalid_paint_properties() -> RunProperties {
        let mut color = PresetColor::default();
        color.set_val("not-a-preset");
        let mut fill = SolidFill::default();
        fill.set_preset_color(color);
        let mut properties = RunProperties::default();
        properties.set_solid_fill(fill);
        properties
    }

    #[test]
    fn invalid_paints_fail_projected_style_comparison() {
        let invalid = invalid_paint_properties();
        assert!(
            invalid
                .projected_values_match(&RunProperties::default())
                .is_err()
        );
        let mut paragraph = ParagraphProperties::default();
        paragraph.set_default_run_properties(invalid.clone());
        assert!(
            paragraph
                .projected_values_match(&ParagraphProperties::default())
                .is_err()
        );
        let mut level = TextParagraphPropertiesType::default();
        level.set_default_run_properties(invalid);
        let mut list = ListStyle::default();
        list.set_default_paragraph_properties(level);
        assert!(list.projected_values_match(&ListStyle::default()).is_err());
    }

    #[test]
    fn text_restore_reports_native_paint_errors_without_restoring_presence() {
        let mut previous = TextProperties::default();
        previous.parts = Some(Box::new(TextBodyParts {
            list_style_present: true,
        }));
        let mut current = previous.clone();
        current.strip_export_parts();
        let mut level = TextParagraphPropertiesType::default();
        level.set_default_run_properties(invalid_paint_properties());
        current.list_style.set_default_paragraph_properties(level);
        let before = bincode::serialize(&current).unwrap();
        assert!(current.restore_export_parts(&previous).is_err());
        assert_eq!(bincode::serialize(&current).unwrap(), before);
        let mut rich: super::super::RichText =
            serde_json::from_value(serde_json::to_value(&current).unwrap()).unwrap();
        let previous_rich: super::super::RichText =
            serde_json::from_value(serde_json::to_value(&previous).unwrap()).unwrap();
        let before = bincode::serialize(&rich).unwrap();
        assert!(rich.restore_export_parts(&previous_rich).is_err());
        assert_eq!(bincode::serialize(&rich).unwrap(), before);
    }

    #[test]
    fn run_and_paragraph_restore_propagate_native_paint_errors() {
        let mut previous = Run::default();
        previous.set_text("text");
        previous.run_properties_mut().set_language("en-US");
        let mut current = previous.clone();
        *current.run_properties_mut() = invalid_paint_properties();
        current.strip_export_parts();
        let before = bincode::serialize(&current).unwrap();
        assert!(current.restore_export_parts(&previous).is_err());
        assert_eq!(bincode::serialize(&current).unwrap(), before);
        let mut previous = Paragraph::default();
        previous
            .paragraph_properties_mut()
            .set_alignment(crate::drawing::TextAlignmentTypeValues::Center);
        let mut current = previous.clone();
        current
            .paragraph_properties_mut()
            .set_default_run_properties(invalid_paint_properties());
        current.strip_export_parts();
        let before = bincode::serialize(&current).unwrap();
        assert!(current.restore_export_parts(&previous).is_err());
        assert_eq!(bincode::serialize(&current).unwrap(), before);
    }
}
