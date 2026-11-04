// c:orientation
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use super::{
    super::super::EnumValue,
    OrientationValues,
};
use crate::writer::driver::write_start_tag;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct Orientation {
    val: EnumValue<OrientationValues>,
}
impl Orientation {
    #[must_use]
    pub fn val(&self) -> &OrientationValues {
        self.val.value()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use val()")]
    pub fn get_val(&self) -> &OrientationValues {
        self.val()
    }

    pub fn set_val(&mut self, value: OrientationValues) -> &mut Orientation {
        self.val.set_value(value);
        self
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        _reader: &mut Reader<R>,
        e: &BytesStart,
    ) -> Result<(), crate::XlsxError> {
        let value = crate::reader::driver::read_enum_attribute::<OrientationValues>(e, b"val")?
            .unwrap_or(OrientationValues::MinMax);
        self.val.set_value(value);
        Ok(())
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        // c:orientation
        write_start_tag(
            writer,
            "c:orientation",
            vec![("val", self.val.value_string()).into()],
            true,
        );
    }
}
