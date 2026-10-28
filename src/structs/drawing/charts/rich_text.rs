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
    drawing::Run,
    reader::driver::local_name,
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
    xml_read_loop,
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
        if let Some(parts) = &mut self.parts {
            parts.list_style_present = true;
        }
        &mut self.list_style
    }

    #[deprecated(since = "3.0.0", note = "Use list_style_mut()")]
    pub fn get_list_style_mut(&mut self) -> &mut ListStyle {
        self.list_style_mut()
    }

    pub fn set_list_style(&mut self, value: ListStyle) {
        if let Some(parts) = &mut self.parts {
            parts.list_style_present = true;
        }
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

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
    ) {
        self.parts = Some(Box::new(TextBodyParts::default()));
        let end = e.name().into_inner().to_vec();
        xml_read_loop!(
            reader,
            ref event @ (Event::Start(ref e) | Event::Empty(ref e)) => {
                let empty = matches!(event, Event::Empty(_));
                match local_name(e.name().into_inner()) {
                    b"p" => {
                        let mut v = Paragraph::default();
                        if empty {
                            v.mark_empty_import();
                        } else {
                            v.set_attributes(reader, e);
                        }
                        self.add_paragraph(v);
                    }
                    b"lstStyle" => {
                        if let Some(parts) = &mut self.parts { parts.list_style_present = true; }
                        if !empty { self.list_style.set_attributes(reader, e); }
                    }
                    b"bodyPr" => {
                        let mut v = BodyProperties::default();
                        v.set_attributes(reader, e, empty);
                        self.set_body_properties(v);
                    }
                    _ => {
                        if !empty {
                            crate::reader::driver::skip_element(reader, e);
                        }
                    }
                }
            },
            Event::End(ref e) => {
                if e.name().into_inner() == end.as_slice() {
                    return;
                }
            },
            Event::Eof => panic!("Could not find text body end element"),
        );
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
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
            self.list_style.write_to(writer);
        }

        for content in &self.paragraph {
            content.write_to(writer);
        }

        write_end_tag(writer, "c:rich");
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
        value.set_attributes(&mut reader, &e);
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        value.write_to(&mut writer);
        assert!(
            !String::from_utf8(writer.into_inner().into_inner())
                .unwrap()
                .contains("lstStyle")
        );
        value.list_style_mut();
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        value.write_to(&mut writer);
        assert!(
            String::from_utf8(writer.into_inner().into_inner())
                .unwrap()
                .contains("<a:lstStyle/>")
        );
    }
}
