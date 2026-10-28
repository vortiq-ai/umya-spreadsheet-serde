// a:spcBef
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::{
        BytesStart,
        Event,
    },
};

use super::SpacingPercent;
use crate::{
    reader::driver::xml_read_loop,
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct SpaceBefore {
    spacing_percent: Option<SpacingPercent>,
}

impl SpaceBefore {
    #[inline]
    #[must_use]
    pub fn spacing_percent(&self) -> Option<&SpacingPercent> {
        self.spacing_percent.as_ref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use spacing_percent()")]
    pub fn get_spacing_percent(&self) -> Option<&SpacingPercent> {
        self.spacing_percent()
    }

    #[inline]
    pub fn spacing_percent_mut(&mut self) -> Option<&mut SpacingPercent> {
        self.spacing_percent.as_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use spacing_percent_mut()")]
    pub fn get_spacing_percent_mut(&mut self) -> Option<&mut SpacingPercent> {
        self.spacing_percent_mut()
    }

    #[inline]
    pub fn set_spacing_percent(&mut self, value: SpacingPercent) -> &mut Self {
        self.spacing_percent = Some(value);
        self
    }

    #[inline]
    pub fn remove_spacing_percent(&mut self) {
        self.spacing_percent = None;
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
        empty: bool,
    ) {
        if empty {
            return;
        }
        let end = e.name().into_inner().to_vec();
        xml_read_loop!(reader,
            ref event @ (Event::Start(ref child) | Event::Empty(ref child)) => {
                let empty = matches!(event, Event::Empty(_));
                if crate::reader::driver::local_name(child.name().into_inner()) == b"spcPct" {
                    let mut value = SpacingPercent::default();
                    value.set_attributes(reader, child);
                    self.set_spacing_percent(value);
                }
                if !empty { crate::reader::driver::skip_element(reader, child); }
            },
            Event::End(ref child) => {
                if child.name().into_inner() == end.as_slice() { return; }
            },
            Event::Eof => panic!("unterminated paragraph spacing")
        );
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        // a:spcBef
        write_start_tag(writer, "a:spcBef", vec![], false);

        // a:spcPct
        if let Some(v) = &self.spacing_percent {
            v.write_to(writer);
        }

        write_end_tag(writer, "a:spcBef");
    }
}
