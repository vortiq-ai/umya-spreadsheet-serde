// c:secondPieSize
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use super::super::super::UInt16Value;
use crate::writer::driver::write_start_tag;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct SecondPieSize {
    val: UInt16Value,
}
impl Default for SecondPieSize {
    fn default() -> Self {
        let mut val = UInt16Value::default();
        val.set_value(Self::DEFAULT);
        Self { val }
    }
}
impl SecondPieSize {
    /// `ST_SecondPieSize` (5 to 200 percent) defaults to 75.
    const DEFAULT: u16 = 75;

    #[must_use]
    pub fn val(&self) -> u16 {
        if self.val.has_value() {
            self.val.value()
        } else {
            Self::DEFAULT
        }
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use val()")]
    pub fn get_val(&self) -> u16 {
        self.val()
    }

    pub fn set_val(&mut self, value: u16) -> &mut SecondPieSize {
        self.val.set_value(value);
        self
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
    ) -> Result<(), crate::XlsxError> {
        let value = super::percent_element::read_percent(
            reader,
            e,
            "secondPieSize",
            (5, 200),
            Self::DEFAULT,
        )?;
        self.val.set_value(value);
        Ok(())
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        // c:secondPieSize
        let value = self.val().to_string();
        write_start_tag(
            writer,
            "c:secondPieSize",
            vec![("val", value.as_str()).into()],
            true,
        );
    }
}
