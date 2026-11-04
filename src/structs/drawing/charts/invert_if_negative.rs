// c:invertIfNegative
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use super::super::super::DoubleValue;
use crate::writer::driver::write_start_tag;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct InvertIfNegative {
    val: DoubleValue,
}
impl InvertIfNegative {
    #[must_use]
    pub fn val(&self) -> f64 {
        self.val.value()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use val()")]
    pub fn get_val(&self) -> f64 {
        self.val.value()
    }

    pub fn set_val(&mut self, value: f64) -> &mut InvertIfNegative {
        self.val.set_value(value);
        self
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        _reader: &mut Reader<R>,
        e: &BytesStart,
    ) -> Result<(), crate::XlsxError> {
        let value = crate::reader::driver::read_attribute(e, b"val")?
            .map(|value| crate::xml::parse_bool(&value))
            .transpose()
            .map_err(crate::xml::xml_error)?
            .unwrap_or(true);
        self.val.set_value(if value { 1.0 } else { 0.0 });
        Ok(())
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        // c:invertIfNegative
        write_start_tag(
            writer,
            "c:invertIfNegative",
            vec![("val", &self.val.value_string()).into()],
            true,
        );
    }
}
