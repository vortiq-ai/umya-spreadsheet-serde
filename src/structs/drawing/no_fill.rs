// a:noFill
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use crate::writer::driver::write_start_tag;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct NoFill {}
impl NoFill {
    #[inline]
    pub(crate) fn set_attributes<R: std::io::BufRead>(
        reader: &mut Reader<R>,
        e: &BytesStart,
        empty_flag: bool,
    ) -> Result<(), crate::XlsxError> {
        if !empty_flag {
            crate::structs::drawing::color_transform::consume_paint_leaf(reader, e, None)?;
        }
        Ok(())
    }

    #[inline]
    pub(crate) fn write_to(writer: &mut Writer<Cursor<Vec<u8>>>) {
        // a:noFill
        write_start_tag(writer, "a:noFill", vec![], true);
    }
}
