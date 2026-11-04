// a:prstDash
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use super::{
    super::EnumValue,
    PresetLineDashValues,
};
use crate::writer::driver::write_start_tag;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct PresetDash {
    val: EnumValue<PresetLineDashValues>,
}
impl PresetDash {
    #[inline]
    #[must_use]
    pub fn val(&self) -> &PresetLineDashValues {
        self.val.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use val()")]
    pub fn get_val(&self) -> &PresetLineDashValues {
        self.val()
    }

    #[inline]
    pub fn set_val(&mut self, value: PresetLineDashValues) -> &mut PresetDash {
        self.val.set_value(value);
        self
    }

    #[inline]
    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
        empty_flag: bool,
    ) -> Result<(), crate::XlsxError> {
        self.read_with_session(reader, e, empty_flag, None)
    }

    pub(crate) fn read_with_session<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        owner: &BytesStart<'_>,
        empty: bool,
        session: Option<&crate::reader::xlsx::SourceCacheSession>,
    ) -> Result<(), crate::XlsxError> {
        let _scope_guard = session
            .map(|session| session.enter_child_scope(owner))
            .transpose()?
            .flatten();
        let value = crate::reader::driver::require_attribute(owner, b"val")?;
        self.val.set_value(
            value
                .parse::<PresetLineDashValues>()
                .map_err(|()| crate::xml::xml_error("invalid DrawingML enumeration"))?,
        );
        if !empty {
            crate::structs::drawing::color_transform::consume_paint_leaf(reader, owner, session)?;
        }
        Ok(())
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        // a:prstDash
        write_start_tag(
            writer,
            "a:prstDash",
            vec![("val", self.val.value_string()).into()],
            true,
        );
    }
}
