// c:pageMargins
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use super::super::super::DoubleValue;
use crate::writer::driver::write_start_tag;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct PageMargins {
    bottom: DoubleValue,
    left:   DoubleValue,
    right:  DoubleValue,
    top:    DoubleValue,
    header: DoubleValue,
    footer: DoubleValue,
}
impl PageMargins {
    #[must_use]
    pub fn bottom(&self) -> f64 {
        self.bottom.value()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use bottom()")]
    pub fn get_bottom(&self) -> f64 {
        self.bottom()
    }

    pub fn set_bottom(&mut self, value: f64) -> &mut Self {
        self.bottom.set_value(value);
        self
    }

    #[must_use]
    pub fn left(&self) -> f64 {
        self.left.value()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use left()")]
    pub fn get_left(&self) -> f64 {
        self.left()
    }

    pub fn set_left(&mut self, value: f64) -> &mut Self {
        self.left.set_value(value);
        self
    }

    #[must_use]
    pub fn right(&self) -> f64 {
        self.right.value()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use right()")]
    pub fn get_right(&self) -> f64 {
        self.right()
    }

    pub fn set_right(&mut self, value: f64) -> &mut Self {
        self.right.set_value(value);
        self
    }

    #[must_use]
    pub fn top(&self) -> f64 {
        self.top.value()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use top()")]
    pub fn get_top(&self) -> f64 {
        self.top()
    }

    pub fn set_top(&mut self, value: f64) -> &mut Self {
        self.top.set_value(value);
        self
    }

    #[must_use]
    pub fn header(&self) -> f64 {
        self.header.value()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use header()")]
    pub fn get_header(&self) -> f64 {
        self.header()
    }

    pub fn set_header(&mut self, value: f64) -> &mut Self {
        self.header.set_value(value);
        self
    }

    #[must_use]
    pub fn footer(&self) -> f64 {
        self.footer.value()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use footer()")]
    pub fn get_footer(&self) -> f64 {
        self.footer()
    }

    pub fn set_footer(&mut self, value: f64) -> &mut Self {
        self.footer.set_value(value);
        self
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        _reader: &mut Reader<R>,
        e: &BytesStart,
    ) -> Result<(), crate::XlsxError> {
        self.bottom.set_value(
            crate::xml::parse_double(&crate::reader::driver::require_attribute(e, b"b")?)
                .map_err(crate::xml::xml_error)?,
        );
        self.left.set_value(
            crate::xml::parse_double(&crate::reader::driver::require_attribute(e, b"l")?)
                .map_err(crate::xml::xml_error)?,
        );
        self.right.set_value(
            crate::xml::parse_double(&crate::reader::driver::require_attribute(e, b"r")?)
                .map_err(crate::xml::xml_error)?,
        );
        self.top.set_value(
            crate::xml::parse_double(&crate::reader::driver::require_attribute(e, b"t")?)
                .map_err(crate::xml::xml_error)?,
        );
        self.header.set_value(
            crate::xml::parse_double(&crate::reader::driver::require_attribute(e, b"header")?)
                .map_err(crate::xml::xml_error)?,
        );
        self.footer.set_value(
            crate::xml::parse_double(&crate::reader::driver::require_attribute(e, b"footer")?)
                .map_err(crate::xml::xml_error)?,
        );
        Ok(())
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        // c:pageMargins
        write_start_tag(
            writer,
            "c:pageMargins",
            vec![
                ("b", crate::xml::double_lexical(self.bottom.value())).into(),
                ("l", crate::xml::double_lexical(self.left.value())).into(),
                ("r", crate::xml::double_lexical(self.right.value())).into(),
                ("t", crate::xml::double_lexical(self.top.value())).into(),
                ("header", crate::xml::double_lexical(self.header.value())).into(),
                ("footer", crate::xml::double_lexical(self.footer.value())).into(),
            ],
            true,
        );
    }
}
