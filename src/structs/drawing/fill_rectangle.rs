use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use super::SourceRectangle;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct FillRectangle {
    rectangle: SourceRectangle,
}

impl FillRectangle {
    #[must_use]
    pub fn bottom(&self) -> Option<&str> {
        self.rectangle.b()
    }

    #[must_use]
    pub fn left(&self) -> Option<&str> {
        self.rectangle.l()
    }

    #[must_use]
    pub fn right(&self) -> Option<&str> {
        self.rectangle.r()
    }

    #[must_use]
    pub fn top(&self) -> Option<&str> {
        self.rectangle.t()
    }

    pub fn set_bottom<S: Into<String>>(&mut self, value: S) {
        self.rectangle.set_b(value);
    }

    pub fn set_left<S: Into<String>>(&mut self, value: S) {
        self.rectangle.set_l(value);
    }

    pub fn set_right<S: Into<String>>(&mut self, value: S) {
        self.rectangle.set_r(value);
    }

    pub fn set_top<S: Into<String>>(&mut self, value: S) {
        self.rectangle.set_t(value);
    }

    pub(crate) fn read_with_session<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        owner: &BytesStart,
        empty: bool,
        session: Option<&crate::reader::xlsx::SourceCacheSession>,
    ) -> Result<(), crate::XlsxError> {
        self.rectangle
            .read_with_session(reader, owner, empty, session)
    }

    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
    ) -> Result<(), crate::XlsxError> {
        self.rectangle.write_to_as(writer, "a:fillRect")
    }
}
