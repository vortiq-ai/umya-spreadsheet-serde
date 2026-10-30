// c:rich
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
    super::{
        BodyProperties,
        ListStyle,
        Paragraph,
    },
    text_properties::TextBodyParts,
};
use crate::{
    XlsxError,
    drawing::Run,
    helper::const_str::DRAWINGML_MAIN_NS as NS_A,
    reader::driver::local_name,
    try_xml_read_loop,
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct RichText {
    body_properties: BodyProperties,
    list_style:      ListStyle,
    paragraph:       Vec<Paragraph>,
    #[cfg_attr(feature = "serde", serde(default))]
    parts:           Option<Box<TextBodyParts>>,
}

impl RichText {
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

    #[inline]
    pub fn set_text<S: Into<String>>(&mut self, value: S) -> &mut Self {
        self.paragraph.clear();
        let mut paragraph = Paragraph::default();
        let mut run = Run::default();
        run.set_text(value);
        paragraph.add_run(run);
        self.add_paragraph(paragraph);
        self
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

    pub fn set_body_properties(&mut self, value: BodyProperties) {
        self.body_properties = value;
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

    pub fn set_list_style(&mut self, value: ListStyle) {
        self.parts
            .get_or_insert_with(Default::default)
            .list_style_present = true;
        self.list_style = value;
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

    pub fn add_paragraph(&mut self, value: Paragraph) {
        self.paragraph.push(value);
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
        // c:rich
        write_start_tag(writer, "c:rich", vec![], false);

        // a:bodyPr
        self.body_properties.write_to(writer);

        // a:lstStyle
        if self
            .parts
            .as_ref()
            .is_none_or(|parts| parts.list_style_present)
        {
            self.list_style.write_to(writer)?;
        }

        for content in &self.paragraph {
            content.write_to(writer)?;
        }

        write_end_tag(writer, "c:rich");
        Ok(())
    }
}

#[cfg(test)]
mod parity_tests {
    use super::*;
    #[test]
    fn absent_list_style_is_preserved_and_mutable_access_marks_presence() {
        let mut reader = Reader::from_reader(b"<c:rich><a:bodyPr/><a:p/></c:rich>".as_slice());
        let mut buf = Vec::new();
        let e = match reader.read_event_into(&mut buf).unwrap() {
            Event::Start(e) => e.into_owned(),
            _ => panic!("expected rich text"),
        };
        let mut value = RichText::default();
        value.read_with_session(&mut reader, &e, None).unwrap();
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        value.write_to(&mut writer).unwrap();
        assert!(
            !String::from_utf8(writer.into_inner().into_inner())
                .unwrap()
                .contains("lstStyle")
        );
        value.list_style_mut();
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        value.write_to(&mut writer).unwrap();
        assert!(
            String::from_utf8(writer.into_inner().into_inner())
                .unwrap()
                .contains("<a:lstStyle/>")
        );
    }
}
