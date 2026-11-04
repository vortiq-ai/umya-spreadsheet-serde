use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::{
        BytesStart,
        Event,
    },
};

use super::run_properties::RunProperties;
use crate::{
    XlsxError,
    helper::const_str::DRAWINGML_MAIN_NS as NS_A,
    reader::driver::try_xml_read_loop,
    writer::driver::{
        write_end_tag,
        write_start_tag,
        write_text_node,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct Run {
    text:           Box<str>,
    run_properties: RunProperties,
    #[cfg_attr(feature = "serde", serde(default))]
    parts:          Option<Box<RunParts>>,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct RunParts {
    properties_present: bool,
}

impl Run {
    // Used only while assembling a fresh native chart, before import metadata.
    pub(crate) fn set_native_chart_run_properties(&mut self, value: RunProperties) {
        self.run_properties = value;
    }

    pub(crate) fn strip_export_parts(&mut self) {
        self.discard_incoming_export_parts();
        self.run_properties.strip_export_parts();
    }

    pub(crate) fn discard_incoming_export_parts(&mut self) {
        self.parts = None;
    }

    pub(crate) fn restore_export_parts(&mut self, previous: &Self) -> Result<usize, XlsxError> {
        if self.text != previous.text {
            return Ok(0);
        }
        self.restore_edited_export_parts(previous)
    }

    /// Restores the run's own hidden parts whatever text it holds now: Excel
    /// keeps a run's formatting when its characters are retyped.
    pub(crate) fn restore_edited_export_parts(
        &mut self,
        previous: &Self,
    ) -> Result<usize, XlsxError> {
        let mut restored = 0;
        if self.parts.is_none() {
            if let Some(parts) = &previous.parts {
                let mut parts = parts.clone();
                // Omitted imported rPr cannot hide a current formatting edit.
                if self.run_properties.export_parts_supplied()
                    || !self
                        .run_properties
                        .projected_values_match(&previous.run_properties)?
                {
                    parts.properties_present = true;
                }
                self.parts = Some(parts);
                restored += 1;
            }
        }
        Ok(restored
            + self
                .run_properties
                .restore_export_parts(&previous.run_properties))
    }

    #[inline]
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use text()")]
    pub fn get_text(&self) -> &str {
        self.text()
    }

    #[inline]
    pub fn set_text<S: Into<String>>(&mut self, value: S) {
        self.text = value.into().into_boxed_str();
    }

    #[inline]
    #[must_use]
    pub fn run_properties(&self) -> &RunProperties {
        &self.run_properties
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use run_properties()")]
    pub fn get_run_properties(&self) -> &RunProperties {
        self.run_properties()
    }

    #[inline]
    pub fn run_properties_mut(&mut self) -> &mut RunProperties {
        self.parts
            .get_or_insert_with(Default::default)
            .properties_present = true;
        &mut self.run_properties
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use run_properties_mut()")]
    pub fn get_run_properties_mut(&mut self) -> &mut RunProperties {
        self.run_properties_mut()
    }

    #[inline]
    pub fn set_run_properties(&mut self, value: RunProperties) {
        self.parts
            .get_or_insert_with(Default::default)
            .properties_present = true;
        self.run_properties = value;
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
        self.parts = Some(Box::new(RunParts::default()));
        let mut properties_present = false;
        let mut text_present = false;
        let end = e.name().as_ref().to_vec();
        try_xml_read_loop!(reader,
            ref event @ (Event::Empty(ref e) | Event::Start(ref e)) => {
                let empty = matches!(event, Event::Empty(_));
                let native = match session {
                    Some(session) => session.accepts_child_namespace(e, NS_A)?,
                    None => true,
                };
                if !native {
                    if !empty { crate::reader::driver::skip_element(reader, e)?; }
                    continue;
                }
                match crate::reader::driver::local_name(e.name().as_ref()) {
                    b"rPr" if !properties_present && !text_present => {
                        properties_present = true;
                        self.parts.get_or_insert_with(Default::default).properties_present = true;
                        self.run_properties.read_with_session(reader, e, empty, session)?;
                    }
                    b"t" if !text_present => {
                        crate::reader::driver::validate_attributes(e)?;
                        text_present = true;
                        self.set_text(crate::reader::driver::read_simple_text(reader, e, empty)?
                            .ok_or_else(|| crate::xml::xml_error("invalid DrawingML run text"))?);
                    }
                    b"rPr" | b"t" => return Err(crate::xml::xml_error("duplicate or misplaced DrawingML run child")),
                    _ => if !empty { crate::reader::driver::skip_element(reader, e)?; },
                }
            },
            Event::End(ref e) if e.name().as_ref() == end.as_slice() => {
                return if text_present { Ok(()) } else { Err(crate::xml::xml_error("DrawingML run requires a text element")) };
            },
            Event::Eof => return Err(crate::xml::xml_error("unterminated DrawingML run")),
        );
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) -> Result<(), XlsxError> {
        // a:r
        write_start_tag(writer, "a:r", vec![], false);

        // a:rPr
        if self
            .parts
            .as_ref()
            .is_none_or(|parts| parts.properties_present)
        {
            self.run_properties.write_to_rpr(writer)?;
        }

        // a:t
        write_start_tag(writer, "a:t", vec![], false);
        write_text_node(writer, &*self.text);
        write_end_tag(writer, "a:t");

        write_end_tag(writer, "a:r");
        Ok(())
    }
}

#[cfg(test)]
mod parity_tests {
    use super::*;
    fn parse(xml: &str) -> Run {
        let mut reader = Reader::from_reader(xml.as_bytes());
        let mut buf = Vec::new();
        let e = match reader.read_event_into(&mut buf).unwrap() {
            Event::Start(e) => e.into_owned(),
            _ => panic!("expected run"),
        };
        let mut run = Run::default();
        run.read_with_session(&mut reader, &e, None).unwrap();
        run
    }
    fn xml(run: &Run) -> String {
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        run.write_to(&mut writer).unwrap();
        String::from_utf8(writer.into_inner().into_inner()).unwrap()
    }
    #[test]
    fn imported_property_presence_and_mutation_are_preserved() {
        let mut absent = parse("<a:r><a:t>value</a:t></a:r>");
        assert!(!xml(&absent).contains("<a:rPr"));
        absent.set_text("edited");
        assert!(!xml(&absent).contains("<a:rPr"));
        absent.run_properties_mut();
        assert!(xml(&absent).contains("<a:rPr"));
        let mut alias = parse("<a:r><a:t>value</a:t></a:r>");
        // This assertion intentionally checks the deprecated mutable alias.
        #[allow(deprecated)]
        alias.get_run_properties_mut();
        assert!(xml(&alias).contains("<a:rPr"));
        let mut setter = parse("<a:r><a:t>value</a:t></a:r>");
        setter.set_run_properties(RunProperties::default());
        assert!(xml(&setter).contains("<a:rPr"));
        assert!(xml(&parse("<a:r><a:rPr/><a:t>value</a:t></a:r>")).contains("<a:rPr"));
        assert!(xml(&Run::default()).contains("<a:rPr"));
    }
}
