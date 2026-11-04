// c:shape
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use super::{
    super::super::EnumValue,
    ShapeValues,
};
use crate::writer::driver::write_start_tag;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct Shape {
    val: EnumValue<ShapeValues>,
}
impl Shape {
    #[must_use]
    pub fn val(&self) -> &ShapeValues {
        self.val.value()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use val()")]
    pub fn get_val(&self) -> &ShapeValues {
        self.val()
    }

    pub fn set_val(&mut self, value: ShapeValues) -> &mut Shape {
        self.val.set_value(value);
        self
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        _reader: &mut Reader<R>,
        e: &BytesStart,
    ) -> Result<(), crate::XlsxError> {
        let value = crate::reader::driver::read_enum_attribute::<ShapeValues>(e, b"val")?
            .unwrap_or(ShapeValues::Box);
        self.val.set_value(value);
        Ok(())
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        // c:shape
        write_start_tag(
            writer,
            "c:shape",
            vec![("val", self.val.value_string()).into()],
            true,
        );
    }
}
