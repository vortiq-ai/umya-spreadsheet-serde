// a:endCxn
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use super::super::super::UInt32Value;
use crate::{
    reader::driver::require_attribute,
    writer::driver::write_start_tag,
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct EndConnection {
    id:    UInt32Value,
    index: UInt32Value,
}
impl EndConnection {
    #[inline]
    #[must_use]
    pub fn id(&self) -> u32 {
        self.id.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use id()")]
    pub fn get_id(&self) -> u32 {
        self.id()
    }

    #[inline]
    pub fn set_id(&mut self, value: u32) {
        self.id.set_value(value);
    }

    #[inline]
    #[must_use]
    pub fn index(&self) -> u32 {
        self.index.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use index()")]
    pub fn get_index(&self) -> u32 {
        self.index()
    }

    #[inline]
    pub fn set_index(&mut self, value: u32) {
        self.index.set_value(value);
    }

    #[inline]
    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        _reader: &mut Reader<R>,
        e: &BytesStart,
    ) -> Result<(), crate::XlsxError> {
        let id = require_attribute(e, b"id")?;
        let index = require_attribute(e, b"idx")?;
        let id = crate::xml::parse_u32(&id).map_err(crate::xml::xml_error)?;
        let index = crate::xml::parse_u32(&index).map_err(crate::xml::xml_error)?;
        self.set_id(id);
        self.set_index(index);
        Ok(())
    }

    #[inline]
    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        write_start_tag(
            writer,
            "a:endCxn",
            vec![
                ("id", self.id.value_string()).into(),
                ("idx", self.index.value_string()).into(),
            ],
            true,
        );
    }
}

#[cfg(test)]
mod tests {
    use quick_xml::events::Event;

    use super::*;

    fn parsed(xml: &str) -> Result<EndConnection, crate::XlsxError> {
        let mut reader = Reader::from_reader(xml.as_bytes());
        let Event::Empty(start) = reader.read_event().unwrap() else {
            panic!("expected empty connection");
        };
        let mut value = EndConnection::default();
        value.set_attributes(&mut reader, &start)?;
        Ok(value)
    }

    #[test]
    fn connection_attributes_are_required_normalized_and_checked() {
        let value = parsed(r#"<a:endCxn id=" &#x32; " idx="0"/>"#).unwrap();
        assert_eq!(value.id(), 2);
        assert_eq!(value.index(), 0);
        for attrs in [
            r#"id="1""#,
            r#"idx="0""#,
            r#"id="bad" idx="0""#,
            r#"id="1" idx="-1""#,
            r#"id="4294967296" idx="0""#,
            r#"id="1" id="2" idx="0""#,
            r#"id="1" idx="&missing;""#,
        ] {
            assert!(parsed(&format!("<a:endCxn {attrs}/>")).is_err(), "{attrs}");
        }
    }
}
