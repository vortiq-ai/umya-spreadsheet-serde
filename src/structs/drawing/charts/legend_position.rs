// c:legendPos
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use super::{
    super::super::EnumValue,
    LegendPositionValues,
};
use crate::writer::driver::write_start_tag;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct LegendPosition {
    val: EnumValue<LegendPositionValues>,
}
impl LegendPosition {
    #[must_use]
    pub fn val(&self) -> &LegendPositionValues {
        self.val.value()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use val()")]
    pub fn get_val(&self) -> &LegendPositionValues {
        self.val()
    }

    pub fn set_val(&mut self, value: LegendPositionValues) -> &mut LegendPosition {
        self.val.set_value(value);
        self
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        _reader: &mut Reader<R>,
        e: &BytesStart,
    ) -> Result<(), crate::XlsxError> {
        let value = crate::reader::driver::read_enum_attribute::<LegendPositionValues>(e, b"val")?
            .unwrap_or(LegendPositionValues::Right);
        self.val.set_value(value);
        Ok(())
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        // c:legendPos
        write_start_tag(
            writer,
            "c:legendPos",
            vec![("val", self.val.value_string()).into()],
            true,
        );
    }
}
