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
    Color,
    ConditionalFormatValueObject,
    StringValue,
};
use crate::{
    reader::driver::{
        get_attribute,
        set_string_from_xml,
        xml_read_loop,
    },
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct IconSet {
    cfvo_collection:  Vec<ConditionalFormatValueObject>,
    color_collection: Vec<Color>,
    icon_set:         StringValue,
    show_value:       BooleanValue,
    percent:          BooleanValue,
    reverse:          BooleanValue,
}

impl IconSet {
    #[inline]
    #[must_use]
    pub fn cfvo_collection(&self) -> &[ConditionalFormatValueObject] {
        &self.cfvo_collection
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use cfvo_collection()")]
    pub fn get_cfvo_collection(&self) -> &[ConditionalFormatValueObject] {
        self.cfvo_collection()
    }

    #[inline]
    pub fn set_cfvo_collection(
        &mut self,
        value: impl Into<Vec<ConditionalFormatValueObject>>,
    ) -> &mut Self {
        self.cfvo_collection = value.into();
        self
    }

    #[inline]
    pub fn add_cfvo_collection(&mut self, value: ConditionalFormatValueObject) -> &mut Self {
        self.cfvo_collection.push(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn color_collection(&self) -> &[Color] {
        &self.color_collection
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use color_collection()")]
    pub fn get_color_collection(&self) -> &[Color] {
        self.color_collection()
    }

    #[inline]
    pub fn set_color_collection(&mut self, value: impl Into<Vec<Color>>) -> &mut Self {
        self.color_collection = value.into();
        self
    }

    #[inline]
    pub fn add_color_collection(&mut self, value: Color) -> &mut Self {
        self.color_collection.push(value);
        self
    }

    /// The `iconSet` attribute: the name of the icon set.
    #[inline]
    #[must_use]
    pub fn icon_set(&self) -> Option<&str> {
        self.icon_set.has_value().then(|| self.icon_set.value_str())
    }

    #[inline]
    pub fn set_icon_set<S: Into<String>>(&mut self, value: S) -> &mut Self {
        self.icon_set.set_value(value);
        self
    }

    /// The `showValue` attribute.
    #[inline]
    #[must_use]
    pub fn show_value(&self) -> Option<bool> {
        self.show_value.has_value().then(|| self.show_value.value())
    }

    #[inline]
    pub fn set_show_value(&mut self, value: bool) -> &mut Self {
        self.show_value.set_value(value);
        self
    }

    /// The `percent` attribute.
    #[inline]
    #[must_use]
    pub fn percent(&self) -> Option<bool> {
        self.percent.has_value().then(|| self.percent.value())
    }

    #[inline]
    pub fn set_percent(&mut self, value: bool) -> &mut Self {
        self.percent.set_value(value);
        self
    }

    /// The `reverse` attribute.
    #[inline]
    #[must_use]
    pub fn reverse(&self) -> Option<bool> {
        self.reverse.has_value().then(|| self.reverse.value())
    }

    #[inline]
    pub fn set_reverse(&mut self, value: bool) -> &mut Self {
        self.reverse.set_value(value);
        self
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
    ) {
        set_string_from_xml!(self, e, icon_set, "iconSet");
        set_string_from_xml!(self, e, show_value, "showValue");
        set_string_from_xml!(self, e, percent, "percent");
        set_string_from_xml!(self, e, reverse, "reverse");
        xml_read_loop!(
            reader,
                ref n @ (Event::Empty(ref e) | Event::Start(ref e)) => {
                    let is_empty = matches!(n, Event::Empty(_));
                    match e.name().into_inner() {
                        b"cfvo" => {
                            let mut obj = ConditionalFormatValueObject::default();
                            obj.set_attributes(reader, e, is_empty);
                            self.cfvo_collection.push(obj);
                        }
                        b"color" => {
                            let mut obj = Color::default();
                            obj.set_attributes(reader, e, is_empty);
                            self.color_collection.push(obj);
                        }
                        _ => (),
                    }
                },
                Event::End(ref e) => {
                    if e.name().into_inner() == b"iconSet" {
                        return
                    }
                },
                Event::Eof => return
        );
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        // iconSet
        let mut attributes: crate::structs::AttrCollection = Vec::new();
        if self.icon_set.has_value() {
            attributes.push(("iconSet", self.icon_set.value_str()).into());
        }
        if self.show_value.has_value() {
            attributes.push(("showValue", self.show_value.value_string()).into());
        }
        if self.percent.has_value() {
            attributes.push(("percent", self.percent.value_string()).into());
        }
        if self.reverse.has_value() {
            attributes.push(("reverse", self.reverse.value_string()).into());
        }
        write_start_tag(writer, "iconSet", attributes, false);

        // cfvo
        for v in &self.cfvo_collection {
            v.write_to(writer);
        }

        // color
        for v in &self.color_collection {
            v.write_to_color(writer);
        }

        write_end_tag(writer, "iconSet");
    }
}
