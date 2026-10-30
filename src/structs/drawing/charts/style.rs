// c:style
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use super::super::super::ByteValue;
use crate::writer::driver::write_start_tag;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Default, Debug)]
pub struct Style {
    val: ByteValue,
}

impl Style {
    #[must_use]
    pub fn val(&self) -> u8 {
        self.val.value()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use val()")]
    pub fn get_val(&self) -> u8 {
        self.val()
    }

    pub fn set_val(&mut self, value: u8) -> &mut Style {
        self.val.set_value(value);
        self
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        _reader: &mut Reader<R>,
        e: &BytesStart,
    ) -> Result<(), crate::XlsxError> {
        let value = crate::xml::parse_u32(&crate::reader::driver::require_attribute(e, b"val")?)
            .map_err(crate::xml::xml_error)?;
        if !(1..=48).contains(&value) {
            return Err(crate::xml::xml_error("chart style value is out of range"));
        }
        self.val.set_value(
            u8::try_from(value).map_err(|error| crate::xml::xml_error(error.to_string()))?,
        );
        Ok(())
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        // c:style
        write_start_tag(
            writer,
            "c:style",
            vec![("val", &self.val.value_string()).into()],
            true,
        );
    }
}
