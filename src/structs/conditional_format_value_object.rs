use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::{
        BytesStart,
        Event,
    },
};

use super::{
    BooleanValue,
    ConditionalFormatValueObjectValues,
    EnumValue,
    StringValue,
};
use crate::{
    reader::driver::{
        get_attribute,
        set_string_from_xml,
        xml_read_loop,
    },
    writer::driver::write_start_tag,
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ConditionalFormatValueObject {
    r#type: EnumValue<ConditionalFormatValueObjectValues>,
    val:    StringValue,
    gte:    BooleanValue,
}

impl ConditionalFormatValueObject {
    #[inline]
    #[must_use]
    pub fn get_type(&self) -> &ConditionalFormatValueObjectValues {
        self.r#type.value()
    }

    #[inline]
    pub fn set_type(&mut self, value: ConditionalFormatValueObjectValues) -> &mut Self {
        self.r#type.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn val(&self) -> &str {
        self.val.value_str()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use val()")]
    pub fn get_val(&self) -> &str {
        self.val()
    }

    #[inline]
    pub fn set_val<S: Into<String>>(&mut self, value: S) -> &mut Self {
        self.val.set_value(value.into());
        self
    }

    /// The `gte` attribute: whether the threshold comparison is "greater than
    /// or equal".
    #[inline]
    #[must_use]
    pub fn gte(&self) -> Option<bool> {
        self.gte.has_value().then(|| self.gte.value())
    }

    #[inline]
    pub fn set_gte(&mut self, value: Option<bool>) -> &mut Self {
        match value {
            Some(value) => {
                self.gte.set_value(value);
            }
            None => self.gte = BooleanValue::default(),
        }
        self
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
        empty_flg: bool,
    ) {
        set_string_from_xml!(self, e, r#type, "type");
        set_string_from_xml!(self, e, val, "val");
        set_string_from_xml!(self, e, gte, "gte");

        if empty_flg {
            return;
        }

        xml_read_loop!(
            reader,
            Event::End(ref e) => {
                if e.name().into_inner() == b"cfvo" {
                    return
                }
            },
            Event::Eof => panic!("Error: Could not find {} end element", "cfvo")
        );
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        // cfvo
        let mut attributes: crate::structs::AttrCollection = Vec::new();
        let ctype = self.r#type.value_string();
        if self.r#type.has_value() {
            attributes.push(("type", ctype).into());
        }
        let val = self.val.value_str();
        if self.val.has_value() {
            attributes.push(("val", val).into());
        }
        if self.gte.has_value() {
            attributes.push(("gte", self.gte.value_string()).into());
        }

        write_start_tag(writer, "cfvo", attributes, true);
    }
}
