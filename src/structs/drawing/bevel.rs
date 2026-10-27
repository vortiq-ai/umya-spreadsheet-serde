// a:bevel
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::{
        BytesStart,
        Event,
    },
};

use crate::{
    reader::driver::xml_read_loop,
    writer::driver::write_start_tag,
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct Bevel {}
impl Bevel {
    #[inline]
    pub(crate) fn set_attributes<R: std::io::BufRead>(
        reader: &mut Reader<R>,
        e: &BytesStart,
        empty_flag: bool,
    ) {
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
            Event::Eof => panic!("Could not find drawing leaf end element"),
        );
    }

    #[inline]
    pub(crate) fn write_to(writer: &mut Writer<Cursor<Vec<u8>>>) {
        // a:bevel
        write_start_tag(writer, "a:bevel", vec![], true);
    }
}
