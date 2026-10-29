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
    helper::const_str::DRAWINGML_MAIN_NS as NS_A,
    reader::driver::xml_read_loop,
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

    pub(crate) fn strip_browser_parts(&mut self) {
        self.parts = None;
        self.run_properties.strip_browser_parts();
    }

    pub(crate) fn restore_browser_parts(&mut self, previous: &Self) -> usize {
        if self.text != previous.text {
            return 0;
        }
        let mut restored = 0;
        if self.parts.is_none() {
            if let Some(parts) = &previous.parts {
                let mut parts = parts.clone();
                // Omitted imported rPr cannot hide a current formatting edit.
                if self.run_properties.browser_parts_supplied()
                    || !self
                        .run_properties
                        .browser_values_match(&previous.run_properties)
                {
                    parts.properties_present = true;
                }
                self.parts = Some(parts);
                restored += 1;
            }
        }
        restored
            + self
                .run_properties
                .restore_browser_parts(&previous.run_properties)
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
    ) {
        let _scope_guard = session.and_then(|session| session.enter_child_scope(e));
        self.parts = Some(Box::new(RunParts::default()));
        let end = e.name().into_inner().to_vec();
        xml_read_loop!(
            reader,
            ref n @ (Event::Empty(ref e) | Event::Start(ref e)) => {
                let is_empty = matches!(n, Event::Empty(_));
                let native = session.is_none_or(|session| session.accepts_child_namespace(e, NS_A));
                let name = if native {
                    crate::reader::driver::local_name(e.name().into_inner())
                } else { b"" };
                match name {
                    b"rPr" => {
                        self.parts.as_mut().unwrap().properties_present = true;
                        self.run_properties.read_with_session(reader, e, is_empty, session);
                    }
                    b"t" => {
                        self.set_text(crate::reader::driver::read_simple_text(reader,e,is_empty)
                            .expect("invalid DrawingML run text"));
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
            Event::Eof => panic!("Error: Could not find {} end element", "a:r"),
        );
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        // a:r
        write_start_tag(writer, "a:r", vec![], false);

        // a:rPr
        if self
            .parts
            .as_ref()
            .is_none_or(|parts| parts.properties_present)
        {
            self.run_properties.write_to_rpr(writer);
        }

        // a:t
        write_start_tag(writer, "a:t", vec![], false);
        write_text_node(writer, &*self.text);
        write_end_tag(writer, "a:t");

        write_end_tag(writer, "a:r");
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
        run.read_with_session(&mut reader, &e, None);
        run
    }
    fn xml(run: &Run) -> String {
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        run.write_to(&mut writer);
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
