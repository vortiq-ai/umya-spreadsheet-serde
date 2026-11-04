// c:rotX
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use super::super::super::SByteValue;
use crate::writer::driver::write_start_tag;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct RotateX {
    val: SByteValue,
}
impl RotateX {
    #[must_use]
    pub fn val(&self) -> i8 {
        self.val.value()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use val()")]
    pub fn get_val(&self) -> i8 {
        self.val()
    }

    pub fn set_val(&mut self, value: i8) -> &mut RotateX {
        self.val.set_value(value);
        self
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        _reader: &mut Reader<R>,
        e: &BytesStart,
    ) -> Result<(), crate::XlsxError> {
        let value = crate::reader::driver::read_attribute(e, b"val")?
            .map(|value| crate::xml::parse_i32(&value))
            .transpose()
            .map_err(crate::xml::xml_error)?
            .unwrap_or(0);
        if !(-90..=90).contains(&value) {
            return Err(crate::xml::xml_error(
                "chart rotate_x value is out of range",
            ));
        }
        self.val.set_value(
            i8::try_from(value).map_err(|error| crate::xml::xml_error(error.to_string()))?,
        );
        Ok(())
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        // c:rotX
        write_start_tag(
            writer,
            "c:rotX",
            vec![("val", &self.val.value_string()).into()],
            true,
        );
    }
}
