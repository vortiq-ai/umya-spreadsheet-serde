use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::{
        BytesStart,
        Event,
    },
};

use super::super::Int32Value;
use crate::{
    reader::driver::{
        get_attribute,
        set_string_from_xml,
        xml_read_loop,
    },
    writer::driver::write_start_tag,
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct PercentageType {
    val: Int32Value,
}

impl PercentageType {
    #[inline]
    #[must_use]
    pub fn val(&self) -> i32 {
        self.val.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use val()")]
    pub fn get_val(&self) -> i32 {
        self.val()
    }

    #[inline]
    pub fn set_val(&mut self, value: i32) -> &mut Self {
        self.val.set_value(value);
        self
    }

    #[inline]
    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
        empty_flag: bool,
    ) {
        set_string_from_xml!(self, e, val, "val");

        if empty_flag {
            return;
        }

        let end_name = e.name().as_ref().to_vec();
        xml_read_loop!(reader,
            Event::Start(ref child) => { crate::reader::driver::skip_element(reader,child); },
            Event::End(ref end) => {
                    if end.name().as_ref() == end_name.as_slice() {
                        return;
                    }
                },
            Event::Eof => panic!("Could not find percentage element end"),
        );
    }

    #[inline]
    pub(crate) fn write_to_lum(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        self.write_to(writer, "a:lum");
    }

    #[inline]
    pub(crate) fn write_to_lum_mod(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        self.write_to(writer, "a:lumMod");
    }

    #[inline]
    pub(crate) fn write_to_lum_off(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        self.write_to(writer, "a:lumOff");
    }

    #[inline]
    pub(crate) fn write_to_sat(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        self.write_to(writer, "a:sat");
    }

    #[inline]
    pub(crate) fn write_to_sat_mod(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        self.write_to(writer, "a:satMod");
    }

    fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>, tab_name: &str) {
        if self.val.has_value() {
            let mut attributes: crate::structs::AttrCollection = Vec::new();
            let val = self.val.value_string();
            attributes.push(("val", &val).into());
            write_start_tag(writer, tab_name, attributes, true);
        }
    }
}
