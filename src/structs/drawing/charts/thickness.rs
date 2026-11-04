// c:thickness
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use super::super::super::ByteValue;
use crate::writer::driver::write_start_tag;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct Thickness {
    val: ByteValue,
}
impl Thickness {
    #[must_use]
    pub fn val(&self) -> u8 {
        self.val.value()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use val()")]
    pub fn get_val(&self) -> u8 {
        self.val()
    }

    pub fn set_val(&mut self, value: u8) -> &mut Thickness {
        self.val.set_value(value);
        self
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        _reader: &mut Reader<R>,
        e: &BytesStart,
    ) -> Result<(), crate::XlsxError> {
        self.val
            .set_value_string(crate::reader::driver::require_attribute(e, b"val")?);
        Ok(())
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        // c:thickness
        write_start_tag(
            writer,
            "c:thickness",
            vec![("val", &self.val.value_string()).into()],
            true,
        );
    }
}
