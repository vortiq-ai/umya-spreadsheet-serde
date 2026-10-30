// a:srcRect
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use crate::{
    StringValue,
    writer::driver::write_start_tag,
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct SourceRectangle {
    t: StringValue,
    l: StringValue,
    r: StringValue,
    b: StringValue,
}
impl SourceRectangle {
    #[inline]
    pub fn set_t<S: Into<String>>(&mut self, value: S) {
        self.t.set_value(value);
    }

    #[inline]
    #[must_use]
    pub fn t(&self) -> Option<&str> {
        self.t.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use t()")]
    pub fn get_t(&self) -> Option<&str> {
        self.t()
    }

    #[inline]
    pub fn set_l<S: Into<String>>(&mut self, value: S) {
        self.l.set_value(value);
    }

    #[inline]
    #[must_use]
    pub fn l(&self) -> Option<&str> {
        self.l.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use l()")]
    pub fn get_l(&self) -> Option<&str> {
        self.l()
    }

    #[inline]
    pub fn set_r<S: Into<String>>(&mut self, value: S) {
        self.r.set_value(value);
    }

    #[inline]
    #[must_use]
    pub fn r(&self) -> Option<&str> {
        self.r.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use r()")]
    pub fn get_r(&self) -> Option<&str> {
        self.r()
    }

    #[inline]
    pub fn set_b<S: Into<String>>(&mut self, value: S) {
        self.b.set_value(value);
    }

    #[inline]
    #[must_use]
    pub fn b(&self) -> Option<&str> {
        self.b.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use b()")]
    pub fn get_b(&self) -> Option<&str> {
        self.b()
    }

    pub(crate) fn validate_percentage(value: &str) -> Result<(), crate::XlsxError> {
        if let Some(number) = value.strip_suffix('%') {
            let number = number.strip_prefix('-').unwrap_or(number);
            let mut pieces = number.split('.');
            let whole = pieces.next().unwrap_or("");
            let fraction = pieces.next();
            if whole.is_empty()
                || !whole.bytes().all(|byte| byte.is_ascii_digit())
                || fraction.is_some_and(|part| {
                    part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit())
                })
                || pieces.next().is_some()
            {
                return Err(crate::xml::xml_error("invalid drawing percentage"));
            }
        } else {
            crate::xml::parse_i32(value).map_err(crate::xml::xml_error)?;
        }
        Ok(())
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        owner: &BytesStart,
    ) -> Result<(), crate::XlsxError> {
        self.read_with_session(reader, owner, true, None)
    }

    pub(crate) fn read_with_session<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        owner: &BytesStart,
        empty: bool,
        session: Option<&crate::reader::xlsx::SourceCacheSession>,
    ) -> Result<(), crate::XlsxError> {
        let _guard = session
            .map(|session| session.enter_child_scope(owner))
            .transpose()?
            .flatten();
        for (name, target) in [
            (b"t".as_slice(), &mut self.t),
            (b"l".as_slice(), &mut self.l),
            (b"r".as_slice(), &mut self.r),
            (b"b".as_slice(), &mut self.b),
        ] {
            if let Some(value) = crate::reader::driver::read_attribute(owner, name)? {
                Self::validate_percentage(&value)?;
                target.set_value(value);
            }
        }
        if !empty {
            crate::structs::drawing::color_transform::consume_paint_leaf(reader, owner, session)?;
        }
        Ok(())
    }

    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
    ) -> Result<(), crate::XlsxError> {
        self.write_to_as(writer, "a:srcRect")
    }

    pub(crate) fn write_to_as(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        name: &str,
    ) -> Result<(), crate::XlsxError> {
        let mut attributes: crate::structs::AttrCollection = Vec::new();

        if let Some(v) = self.t.value() {
            Self::validate_percentage(v)?;
            attributes.push(("t", v).into());
        }
        if let Some(v) = self.l.value() {
            Self::validate_percentage(v)?;
            attributes.push(("l", v).into());
        }
        if let Some(v) = self.r.value() {
            Self::validate_percentage(v)?;
            attributes.push(("r", v).into());
        }
        if let Some(v) = self.b.value() {
            Self::validate_percentage(v)?;
            attributes.push(("b", v).into());
        }
        write_start_tag(writer, name, attributes, true);
        Ok(())
    }
}
