// a:p
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
    ParagraphProperties,
    Run,
    RunProperties,
};
use crate::{
    reader::driver::xml_read_loop,
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

fn run_position(index: usize) -> u32 {
    u32::try_from(index).expect("paragraph run position exceeds the u32 field index range")
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct Paragraph {
    paragraph_properties:    ParagraphProperties,
    run:                     Vec<Run>,
    end_para_run_properties: Option<Box<RunProperties>>,
    #[cfg_attr(feature = "serde", serde(default))]
    parts:                   Option<Box<ParagraphParts>>,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ParagraphParts {
    properties_present: bool,
    fields:             Vec<PositionedTextField>,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct PositionedTextField {
    pub before_run: u32,
    pub field:      TextField,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct TextField {
    pub id:                   String,
    pub kind:                 Option<String>,
    pub run_properties:       Option<RunProperties>,
    pub paragraph_properties: Option<ParagraphProperties>,
    pub text:                 Option<String>,
}
impl TextField {
    pub fn is_valid(&self) -> bool {
        let id = self.id.as_bytes();
        id.len() == 38
            && id[0] == b'{'
            && id[37] == b'}'
            && [9, 14, 19, 24].into_iter().all(|i| id[i] == b'-')
            && id.iter().enumerate().all(|(i, c)| {
                i == 0
                    || i == 37
                    || [9, 14, 19, 24].contains(&i)
                    || c.is_ascii_digit()
                    || (b'A'..=b'F').contains(c)
            })
            && self
                .run_properties
                .as_ref()
                .is_none_or(RunProperties::supplemental_attributes_valid)
            && self
                .paragraph_properties
                .as_ref()
                .is_none_or(ParagraphProperties::supplemental_attributes_valid)
    }

    fn read<R: std::io::BufRead>(reader: &mut Reader<R>, e: &BytesStart, empty: bool) -> Self {
        let mut result = Self {
            id: crate::drawing::get_unescaped_attribute(e, b"id").unwrap_or_default(),
            kind: crate::drawing::get_unescaped_attribute(e, b"type"),
            ..Self::default()
        };
        if empty {
            return result;
        }
        let end = e.name().into_inner().to_vec();
        xml_read_loop!(reader,
            ref event @ (Event::Start(ref e) | Event::Empty(ref e)) => {
                let empty=matches!(event,Event::Empty(_));
                match crate::reader::driver::local_name(e.name().into_inner()) {
                    b"rPr" => { let mut v=RunProperties::default(); v.set_attributes(reader,e,empty); result.run_properties=Some(v); },
                    b"pPr" => { let mut v=ParagraphProperties::default(); v.set_attributes(reader,e,empty); result.paragraph_properties=Some(v); },
                    b"t" => { result.text=Some(crate::reader::driver::read_simple_text(reader,e,empty)
                        .expect("invalid DrawingML field text")); },
                    _ => { if !empty { crate::reader::driver::skip_element(reader,e); } },
                }
            },
            Event::End(ref e) => { if e.name().into_inner()==end.as_slice() { return result; } },
            Event::Eof => panic!("Could not find DrawingML text field end element"),
        );
    }

    fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        let mut attrs: crate::structs::AttrCollection = vec![("id", self.id.as_str()).into()];
        if let Some(v) = &self.kind {
            attrs.push(("type", v.as_str()).into());
        }
        write_start_tag(writer, "a:fld", attrs, false);
        if let Some(v) = &self.run_properties {
            v.write_to_rpr(writer);
        }
        if let Some(v) = &self.paragraph_properties {
            v.write_to(writer);
        }
        if let Some(v) = &self.text {
            write_start_tag(writer, "a:t", vec![], false);
            crate::writer::driver::write_text_node(writer, v);
            write_end_tag(writer, "a:t");
        }
        write_end_tag(writer, "a:fld");
    }
}

impl Paragraph {
    #[must_use]
    pub fn supplemental_content_valid(&self) -> bool {
        self.paragraph_properties.supplemental_attributes_valid()
            && self
                .run
                .iter()
                .all(|v| v.run_properties().supplemental_attributes_valid())
            && self
                .end_para_run_properties
                .as_ref()
                .is_none_or(|v| v.supplemental_attributes_valid())
            && self.parts.as_ref().is_none_or(|p| {
                p.fields
                    .windows(2)
                    .all(|pair| pair[0].before_run <= pair[1].before_run)
                    && p.fields
                        .iter()
                        .all(|f| f.before_run as usize <= self.run.len() && f.field.is_valid())
            })
    }

    pub(crate) fn mark_empty_import(&mut self) {
        self.parts = Some(Box::new(ParagraphParts::default()));
    }

    #[must_use]
    pub fn fields(&self) -> &[PositionedTextField] {
        self.parts.as_ref().map_or(&[], |p| &p.fields)
    }

    pub fn add_field(&mut self, before_run: u32, field: TextField) -> bool {
        if before_run as usize > self.run.len() {
            return false;
        }
        let p = self.parts.get_or_insert_with(|| {
            Box::new(ParagraphParts {
                properties_present: true,
                ..ParagraphParts::default()
            })
        });
        let pos = p.fields.partition_point(|v| v.before_run <= before_run);
        p.fields
            .insert(pos, PositionedTextField { before_run, field });
        true
    }

    pub fn insert_run(&mut self, index: usize, value: Run) -> bool {
        if index > self.run.len() {
            return false;
        }
        let insertion_position = run_position(index);
        if let Some(p) = &mut self.parts {
            for field in &mut p.fields {
                if field.before_run >= insertion_position {
                    field.before_run = field
                        .before_run
                        .checked_add(1)
                        .expect("paragraph field position overflow during run insertion");
                }
            }
        }
        self.run.insert(index, value);
        true
    }

    pub fn remove_run(&mut self, index: usize) -> Option<Run> {
        if index >= self.run.len() {
            return None;
        }
        let removed_position = run_position(index);
        if let Some(p) = &mut self.parts {
            for field in &mut p.fields {
                if field.before_run > removed_position {
                    field.before_run -= 1;
                }
            }
        }
        Some(self.run.remove(index))
    }

    #[inline]
    #[must_use]
    pub fn paragraph_properties(&self) -> &ParagraphProperties {
        &self.paragraph_properties
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use paragraph_properties()")]
    pub fn get_paragraph_properties(&self) -> &ParagraphProperties {
        self.paragraph_properties()
    }

    #[inline]
    pub fn paragraph_properties_mut(&mut self) -> &mut ParagraphProperties {
        if let Some(p) = &mut self.parts {
            p.properties_present = true;
        }
        &mut self.paragraph_properties
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use paragraph_properties_mut()")]
    pub fn get_paragraph_properties_mut(&mut self) -> &mut ParagraphProperties {
        self.paragraph_properties_mut()
    }

    #[inline]
    pub fn set_paragraph_properties(&mut self, value: ParagraphProperties) -> &mut Self {
        if let Some(p) = &mut self.parts {
            p.properties_present = true;
        }
        self.paragraph_properties = value;
        self
    }

    #[inline]
    #[must_use]
    pub fn run(&self) -> &[Run] {
        &self.run
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use run()")]
    pub fn get_run(&self) -> &[Run] {
        self.run()
    }

    #[inline]
    pub fn add_run(&mut self, value: Run) {
        let _ = self.insert_run(self.run.len(), value);
    }

    #[inline]
    #[must_use]
    pub fn end_para_run_properties(&self) -> Option<&RunProperties> {
        self.end_para_run_properties.as_deref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use end_para_run_properties()")]
    pub fn get_end_para_run_properties(&self) -> Option<&RunProperties> {
        self.end_para_run_properties()
    }

    #[inline]
    pub fn end_para_run_properties_mut(&mut self) -> Option<&mut RunProperties> {
        self.end_para_run_properties.as_deref_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use end_para_run_properties_mut()")]
    pub fn get_end_para_run_properties_mut(&mut self) -> Option<&mut RunProperties> {
        self.end_para_run_properties_mut()
    }

    #[inline]
    pub fn set_end_para_run_properties(&mut self, value: RunProperties) -> &mut Self {
        self.end_para_run_properties = Some(Box::new(value));
        self
    }

    #[inline]
    pub fn remove_end_para_run_properties(&mut self) {
        self.end_para_run_properties = None;
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
    ) {
        self.parts = Some(Box::new(ParagraphParts::default()));
        let end = e.name().into_inner().to_vec();
        xml_read_loop!(
            reader,
            ref n @ (Event::Empty(ref e) | Event::Start(ref e)) => {
                let is_empty = matches!(n, Event::Empty(_));
                match crate::reader::driver::local_name(e.name().into_inner()) {
                    b"pPr" => {
                        self.parts.as_mut().unwrap().properties_present=true;
                        self.paragraph_properties
                            .set_attributes(reader, e, is_empty);
                    }
                    b"fld" => {
                        let field = TextField::read(reader, e, is_empty);
                        self.parts.as_mut().unwrap().fields.push(PositionedTextField {
                            before_run: run_position(self.run.len()),
                            field,
                        });
                    }
                    b"r" => {
                        let mut run = Run::default();
                        if !is_empty {
                            run.set_attributes(reader, e);
                        }
                        self.run.push(run);
                    }
                    b"endParaRPr" => {
                        let mut run_properties = RunProperties::default();
                        run_properties.set_attributes(reader, e, is_empty);
                        self.set_end_para_run_properties(run_properties);
                    }
                    _ => {
                        if !is_empty {
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
            Event::Eof => panic!("Error: Could not find {} end element", "a:p"),
        );
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        // a:p
        write_start_tag(writer, "a:p", vec![], false);

        // a:pPr
        if self.parts.as_ref().is_none_or(|p| p.properties_present) {
            self.paragraph_properties.write_to(writer);
        }

        // a:r
        for (i, run) in self.run.iter().enumerate() {
            if let Some(p) = &self.parts {
                let run_index = run_position(i);
                for field in p.fields.iter().filter(|v| v.before_run == run_index) {
                    field.field.write_to(writer);
                }
            }
            run.write_to(writer);
        }
        if let Some(p) = &self.parts {
            for field in p
                .fields
                .iter()
                .filter(|v| v.before_run == run_position(self.run.len()))
            {
                field.field.write_to(writer);
            }
        }

        // a:endParaRPr
        if let Some(v) = &self.end_para_run_properties {
            v.write_to_end_para_rpr(writer);
        }

        write_end_tag(writer, "a:p");
    }
}

#[cfg(test)]
mod parity_tests {
    use super::*;

    #[test]
    fn run_position_preserves_the_u32_boundary() {
        assert_eq!(run_position(usize::try_from(u32::MAX).unwrap()), u32::MAX);
    }

    #[cfg(target_pointer_width = "64")]
    #[test]
    #[should_panic(expected = "paragraph run position exceeds the u32 field index range")]
    fn run_position_reports_overflow_instead_of_truncating() {
        let beyond_u32 = usize::try_from(u64::from(u32::MAX) + 1).unwrap();
        let _ = run_position(beyond_u32);
    }

    #[test]
    fn field_order_and_property_absence_survive_run_edits() {
        let xml = "<a:p><a:r><a:t>A</a:t></a:r><a:fld \
                   id=\"{01234567-89AB-CDEF-0123-456789ABCDEF}\" \
                   type=\"slidenum\"><a:t>4</a:t></a:fld><a:r><a:t>B</a:t></a:r></a:p>";
        let mut reader = Reader::from_reader(xml.as_bytes());
        let mut buf = Vec::new();
        let e = match reader.read_event_into(&mut buf).unwrap() {
            Event::Start(e) => e.into_owned(),
            _ => panic!("expected paragraph"),
        };
        let mut p = Paragraph::default();
        p.set_attributes(&mut reader, &e);
        assert_eq!(p.fields()[0].before_run, 1);
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        p.write_to(&mut writer);
        let out = String::from_utf8(writer.into_inner().into_inner()).unwrap();
        assert!(!out.contains("<a:pPr"));
        assert!(out.find("<a:fld").unwrap() > out.find(">A</a:t>").unwrap());
        assert!(out.find("<a:fld").unwrap() < out.find(">B</a:t>").unwrap());
        p.remove_run(0).unwrap();
        assert_eq!(p.fields()[0].before_run, 0);
        assert!(p.insert_run(0, Run::default()));
        assert_eq!(p.fields()[0].before_run, 1);
    }
}
