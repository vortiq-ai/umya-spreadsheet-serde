// c:pageSetup
use std::io::Cursor;

use quick_xml::Writer;

use crate::writer::driver::write_start_tag;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct PageSetup {}
impl PageSetup {
    pub(crate) fn write_to(writer: &mut Writer<Cursor<Vec<u8>>>) {
        // c:pageSetup
        write_start_tag(writer, "c:pageSetup", vec![], true);
    }
}
