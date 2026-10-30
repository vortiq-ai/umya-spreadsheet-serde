// a:spcPct
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use super::super::super::Int32Value;
use crate::{
    XlsxError,
    writer::driver::write_start_tag,
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct SpacingPercent {
    val: Int32Value,
}
impl SpacingPercent {
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
        empty: bool,
    ) -> Result<(), XlsxError> {
        let raw = crate::reader::driver::require_attribute(e, b"val")?;
        let value = crate::xml::parse_i32(&raw).map_err(crate::xml::xml_error)?;
        if !(0..=13_200_000).contains(&value) {
            return Err(crate::xml::xml_error(
                "DrawingML spacing percent is outside its schema range",
            ));
        }
        if !empty {
            crate::reader::driver::consume_empty_element(reader, e)?;
        }
        self.set_val(value);
        Ok(())
    }

    #[inline]
    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        // a:spcPct
        write_start_tag(
            writer,
            "a:spcPct",
            vec![("val", &self.val.value_string()).into()],
            true,
        );
    }
}
