// c:baseTimeUnit
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use super::{
    super::super::EnumValue,
    TimeUnitValues,
};
use crate::writer::driver::write_start_tag;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct BaseTimeUnit {
    val: EnumValue<TimeUnitValues>,
}
impl BaseTimeUnit {
    #[inline]
    #[must_use]
    pub fn val(&self) -> &TimeUnitValues {
        self.val.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use val()")]
    pub fn get_val(&self) -> &TimeUnitValues {
        self.val()
    }

    #[inline]
    pub fn set_val(&mut self, value: TimeUnitValues) -> &mut Self {
        self.val.set_value(value);
        self
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        _reader: &mut Reader<R>,
        e: &BytesStart,
    ) -> Result<(), crate::XlsxError> {
        let value = crate::reader::driver::read_enum_attribute::<TimeUnitValues>(e, b"val")?
            .unwrap_or(TimeUnitValues::Days);
        self.val.set_value(value);
        Ok(())
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        // c:baseTimeUnit
        write_start_tag(
            writer,
            "c:baseTimeUnit",
            vec![("val", self.val.value_string()).into()],
            true,
        );
    }
}
