// c:tickLblPos
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use super::{
    super::super::EnumValue,
    TickLabelPositionValues,
};
use crate::writer::driver::write_start_tag;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct TickLabelPosition {
    val: EnumValue<TickLabelPositionValues>,
}
impl TickLabelPosition {
    #[must_use]
    pub fn val(&self) -> &TickLabelPositionValues {
        self.val.value()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use val()")]
    pub fn get_val(&self) -> &TickLabelPositionValues {
        self.val()
    }

    pub fn set_val(&mut self, value: TickLabelPositionValues) -> &mut TickLabelPosition {
        self.val.set_value(value);
        self
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        _reader: &mut Reader<R>,
        e: &BytesStart,
    ) -> Result<(), crate::XlsxError> {
        let value =
            crate::reader::driver::read_enum_attribute::<TickLabelPositionValues>(e, b"val")?
                .unwrap_or(TickLabelPositionValues::NextTo);
        self.val.set_value(value);
        Ok(())
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        // c:tickLblPos
        write_start_tag(
            writer,
            "c:tickLblPos",
            vec![("val", self.val.value_string()).into()],
            true,
        );
    }
}
