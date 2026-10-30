// xdr:ext
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use super::super::super::Int64Value;
use crate::writer::driver::write_start_tag;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct Extent {
    cx: Int64Value,
    cy: Int64Value,
}

impl Extent {
    #[inline]
    #[must_use]
    pub fn cx(&self) -> i64 {
        self.cx.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use cx()")]
    pub fn get_cx(&self) -> i64 {
        self.cx()
    }

    #[inline]
    pub fn set_cx(&mut self, value: i64) -> &mut Extent {
        self.cx.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn cy(&self) -> i64 {
        self.cy.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use cy()")]
    pub fn get_cy(&self) -> i64 {
        self.cy()
    }

    #[inline]
    pub fn set_cy(&mut self, value: i64) -> &mut Extent {
        self.cy.set_value(value);
        self
    }

    #[inline]
    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        _reader: &mut Reader<R>,
        owner: &BytesStart,
    ) -> Result<(), crate::XlsxError> {
        for (name, target) in [
            (b"cx".as_slice(), &mut self.cx),
            (b"cy".as_slice(), &mut self.cy),
        ] {
            let text = crate::reader::driver::require_attribute(owner, name)?;
            let value = crate::xml::trim_xml_whitespace(&text)
                .parse::<i64>()
                .map_err(crate::xml::xml_error)?;
            if value < 0 {
                return Err(crate::xml::xml_error("negative drawing extent"));
            }
            target.set_value(value);
        }
        Ok(())
    }

    #[inline]
    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        // xdr:ext
        write_start_tag(
            writer,
            "xdr:ext",
            vec![
                ("cx", self.cx.value_string()).into(),
                ("cy", self.cy.value_string()).into(),
            ],
            true,
        );
    }
}
