// c:symbol
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use super::{
    super::super::EnumValue,
    MarkerStyleValues,
};
use crate::writer::driver::write_start_tag;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct Symbol {
    val: EnumValue<MarkerStyleValues>,
}
impl Symbol {
    #[must_use]
    pub fn val(&self) -> &MarkerStyleValues {
        self.val.value()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use val()")]
    pub fn get_val(&self) -> &MarkerStyleValues {
        self.val()
    }

    pub fn set_val(&mut self, value: MarkerStyleValues) -> &mut Symbol {
        self.val.set_value(value);
        self
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        _reader: &mut Reader<R>,
        e: &BytesStart,
    ) -> Result<(), crate::XlsxError> {
        self.val = EnumValue::default();
        if let Some(value) =
            crate::reader::driver::read_enum_attribute::<MarkerStyleValues>(e, b"val")?
        {
            self.val.set_value(value);
        }
        Ok(())
    }

    /// Whether the required symbol value was explicitly supplied.
    #[must_use]
    pub fn has_val(&self) -> bool {
        self.val.has_value()
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        // c:symbol
        write_start_tag(
            writer,
            "c:symbol",
            if self.val.has_value() {
                vec![("val", self.val.value_string()).into()]
            } else {
                vec![]
            },
            true,
        );
    }
}
