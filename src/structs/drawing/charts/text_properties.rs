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
    reader::driver::{
        local_name,
        xml_read_loop,
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
        if let Some(parts) = &mut self.parts {
            parts.list_style_present = true;
        }
        &mut self.list_style
    }

    #[deprecated(since = "3.0.0", note = "Use list_style_mut()")]
    pub fn get_list_style_mut(&mut self) -> &mut ListStyle {
        self.list_style_mut()
    }

    pub fn set_list_style(&mut self, value: ListStyle) -> &mut TextProperties {
        if let Some(parts) = &mut self.parts {
            parts.list_style_present = true;
        }
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
        // c:txPr
        write_start_tag(writer, "c:txPr", vec![], false);

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

        // a:p
        for content in &self.paragraph {
            content.write_to(writer);
        }

        write_end_tag(writer, "c:txPr");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn roundtrip(xml: &str) -> String {
        let mut reader = Reader::from_reader(xml.as_bytes());
        let mut buf = Vec::new();
        let start = match reader.read_event_into(&mut buf).unwrap() {
            Event::Start(e) => e.into_owned(),
            _ => panic!("missing text properties"),
        };
        let mut value = TextProperties::default();
        value.set_attributes(&mut reader, &start);
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        value.write_to(&mut writer);
        String::from_utf8(writer.into_inner().into_inner()).unwrap()
    }
    #[test]
    fn list_style_presence_is_preserved() {
        assert!(!roundtrip("<c:txPr><a:bodyPr/><a:p/></c:txPr>").contains("lstStyle"));
        assert!(
            roundtrip("<c:txPr><a:bodyPr/><a:lstStyle/><a:p/></c:txPr>").contains("<a:lstStyle/>")
        );
    }
}
