// c:crosses
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use super::{
    super::super::EnumValue,
    CrossesValues,
};
use crate::writer::driver::write_start_tag;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct Crosses {
    val: EnumValue<CrossesValues>,
}
impl Crosses {
    #[must_use]
    pub fn val(&self) -> &CrossesValues {
        self.val.value()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use val()")]
    pub fn get_val(&self) -> &CrossesValues {
        self.val()
    }

    pub fn set_val(&mut self, value: CrossesValues) -> &mut Crosses {
        self.val.set_value(value);
        self
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        _reader: &mut Reader<R>,
        e: &BytesStart,
    ) -> Result<(), crate::XlsxError> {
        self.val
            .set_value(crate::reader::driver::require_enum_attribute::<
                CrossesValues,
            >(e, b"val")?);
        Ok(())
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        // c:crosses
        write_start_tag(
            writer,
            "c:crosses",
            vec![("val", self.val.value_string()).into()],
            true,
        );
    }
}
