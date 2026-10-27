// a:glow
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
    super::super::Int64Value,
    SchemeColor,
};
use crate::{
    reader::driver::{
        get_attribute,
        xml_read_loop,
    },
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct Glow {
    radius:       Int64Value,
    scheme_color: Option<Box<SchemeColor>>,
}

impl Glow {
    #[inline]
    #[must_use]
    pub fn radius(&self) -> i64 {
        self.radius.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use radius()")]
    pub fn get_radius(&self) -> i64 {
        self.radius()
    }

    #[inline]
    pub fn set_radius(&mut self, value: i64) -> &mut Glow {
        self.radius.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn scheme_color(&self) -> Option<&SchemeColor> {
        self.scheme_color.as_deref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use scheme_color()")]
    pub fn get_scheme_color(&self) -> Option<&SchemeColor> {
        self.scheme_color()
    }

    #[inline]
    pub fn set_scheme_color(&mut self, value: SchemeColor) {
        self.scheme_color = Some(Box::new(value));
    }

    pub(crate) fn read_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
        empty: bool,
    ) {
        if let Some(radius) = get_attribute(e, b"rad") {
            self.radius.set_value_string(radius);
        }

        if empty {
            return;
        }
        let end_name = e.name().as_ref().to_vec();
        xml_read_loop!(reader,
                    ref event @ (Event::Start(ref e) | Event::Empty(ref e)) => {
                        let empty=matches!(event,Event::Empty(_));
                        match crate::reader::driver::local_name(e.name().into_inner()) {
        b"schemeClr" => { let mut value=SchemeColor::default(); value.set_attributes(reader,e,empty); self.set_scheme_color(value); },
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
        // a:glow
        write_start_tag(
            writer,
            "a:glow",
            if self.radius.has_value() { vec![("rad",self.radius.value_string()).into()] } else { vec![] },
            false,
        );

        // a:schemeClr
        if let Some(v) = &self.scheme_color {
            v.write_to(writer);
        }

        write_end_tag(writer, "a:glow");
    }
}
