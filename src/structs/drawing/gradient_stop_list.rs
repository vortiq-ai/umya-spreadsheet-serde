// a:gsLst
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::{
        BytesStart,
        Event,
    },
};

use super::GradientStop;
use crate::{
    reader::driver::xml_read_loop,
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct GradientStopList {
    gradient_stop: Vec<GradientStop>,
}

impl GradientStopList {
    #[inline]
    #[must_use]
    pub fn gradient_stop(&self) -> &[GradientStop] {
        &self.gradient_stop
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use gradient_stop()")]
    pub fn get_gradient_stop(&self) -> &[GradientStop] {
        self.gradient_stop()
    }

    #[inline]
    pub fn gradient_stop_mut(&mut self) -> &mut Vec<GradientStop> {
        &mut self.gradient_stop
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use gradient_stop_mut()")]
    pub fn get_gradient_stop_mut(&mut self) -> &mut Vec<GradientStop> {
        self.gradient_stop_mut()
    }

    #[inline]
    pub fn set_gradient_stop(
        &mut self,
        value: impl Into<Vec<GradientStop>>,
    ) -> &mut GradientStopList {
        self.gradient_stop = value.into();
        self
    }

    #[inline]
    pub fn add_gradient_stop(&mut self, value: GradientStop) -> &mut GradientStopList {
        self.gradient_stop.push(value);
        self
    }

    pub(crate) fn read_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
        empty: bool,
    ) {
        if empty {
            return;
        }
        let end_name = e.name().as_ref().to_vec();
        xml_read_loop!(reader,
                    ref event @ (Event::Start(ref e) | Event::Empty(ref e)) => {
                        let empty=matches!(event,Event::Empty(_));
                        match crate::reader::driver::local_name(e.name().into_inner()) {
        b"gs" => { let mut value=GradientStop::default(); value.read_attributes(reader,e,empty); self.add_gradient_stop(value); },
                            _=> { if !empty { crate::reader::driver::skip_element(reader,e); } },
                        }
                    },
                    Event::End(ref end) => {
                    if end.name().as_ref() == end_name.as_slice() {
                        return;
                    }
                },
                    Event::Eof => panic!("Could not find drawing owner end element"),
                );
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        if self.gradient_stop.is_empty() {
            return;
        }
        // a:gsLst
        write_start_tag(writer, "a:gsLst", vec![], false);

        // a:gs
        for v in &self.gradient_stop {
            v.write_to(writer);
        }

        write_end_tag(writer, "a:gsLst");
    }
}
