// c:ofPieType
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use super::{
    super::super::EnumValue,
    OfPieValues,
};
use crate::writer::driver::write_start_tag;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct OfPieType {
    val: EnumValue<OfPieValues>,
}
impl OfPieType {
    #[must_use]
    pub fn val(&self) -> &OfPieValues {
        self.val.value()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use val()")]
    pub fn get_val(&self) -> &OfPieValues {
        self.val()
    }

    pub fn set_val(&mut self, value: OfPieValues) -> &mut OfPieType {
        self.val.set_value(value);
        self
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        _reader: &mut Reader<R>,
        e: &BytesStart,
    ) -> Result<(), crate::XlsxError> {
        let value = crate::reader::driver::read_enum_attribute::<OfPieValues>(e, b"val")?
            .unwrap_or(OfPieValues::Pie);
        self.val.set_value(value);
        Ok(())
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        // c:ofPieType
        write_start_tag(
            writer,
            "c:ofPieType",
            vec![("val", self.val.value_string()).into()],
            true,
        );
    }
}
