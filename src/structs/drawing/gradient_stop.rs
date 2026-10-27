// a:gs
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
    RgbColorModelHex,
    SchemeColor,
};
use crate::{
    reader::driver::{
        get_attribute,
        xml_read_loop,
    },
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct GradientStop {
    position:            i32,
    scheme_color:        Option<Box<SchemeColor>>,
    rgb_color_model_hex: Option<Box<RgbColorModelHex>>,
}

impl GradientStop {
    #[inline]
    #[must_use]
    pub fn position(&self) -> i32 {
        self.position
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use position()")]
    pub fn get_position(&self) -> i32 {
        self.position()
    }

    #[inline]
    pub fn set_position(&mut self, value: i32) -> &mut GradientStop {
        self.position = value;
        self
    }

    #[inline]
    #[must_use]
    pub fn scheme_color(&self) -> Option<&SchemeColor> {
        self.scheme_color.as_deref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use scheme_color()")]
    pub fn get_scheme_color(&self) -> Option<&SchemeColor> {
        self.scheme_color()
    }

    #[inline]
    pub fn scheme_color_mut(&mut self) -> Option<&mut SchemeColor> {
        self.scheme_color.as_deref_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use scheme_color_mut()")]
    pub fn get_scheme_color_mut(&mut self) -> Option<&mut SchemeColor> {
        self.scheme_color_mut()
    }

    #[inline]
    pub fn set_scheme_color(&mut self, value: SchemeColor) -> &mut GradientStop {
        self.scheme_color = Some(Box::new(value));
        self
    }

    #[inline]
    #[must_use]
    pub fn rgb_color_model_hex(&self) -> Option<&RgbColorModelHex> {
        self.rgb_color_model_hex.as_deref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use rgb_color_model_hex()")]
    pub fn get_rgb_color_model_hex(&self) -> Option<&RgbColorModelHex> {
        self.rgb_color_model_hex()
    }

    #[inline]
    pub fn rgb_color_model_hex_mut(&mut self) -> Option<&mut RgbColorModelHex> {
        self.rgb_color_model_hex.as_deref_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use rgb_color_model_hex_mut()")]
    pub fn get_rgb_color_model_hex_mut(&mut self) -> Option<&mut RgbColorModelHex> {
        self.rgb_color_model_hex_mut()
    }

    #[inline]
    pub fn set_rgb_color_model_hex(&mut self, value: RgbColorModelHex) -> &mut GradientStop {
        self.rgb_color_model_hex = Some(Box::new(value));
        self
    }

    pub(crate) fn read_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
        empty: bool,
    ) {
        if let Some(v) = get_attribute(e, b"pos") {
            self.set_position(v.parse::<i32>().unwrap());
        }

        if empty {
            return;
        }
        let end_name = e.name().as_ref().to_vec();
        xml_read_loop!(reader,
                    ref event @ (Event::Start(ref e) | Event::Empty(ref e)) => {
                        let empty=matches!(event,Event::Empty(_));
                        match crate::reader::driver::local_name(e.name().into_inner()) {
        b"schemeClr" => { let mut value=SchemeColor::default(); value.set_attributes(reader,e,empty); self.set_scheme_color(value); },
        b"srgbClr" => { let mut value=RgbColorModelHex::default(); value.set_attributes(reader,e,empty); self.set_rgb_color_model_hex(value); },
                            _=> { if !empty { crate::reader::driver::skip_element(reader,e); } },
                        }
                    },
                    Event::End(ref end) => {
                    if end.name().as_ref() == end_name.as_slice() {
                        return;
                    }
                },
                    Event::Eof => panic!("Could not find drawing owner end element"),
                );
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        // a:gs
        write_start_tag(
            writer,
            "a:gs",
            vec![("pos", &self.position.to_string()).into()],
            false,
        );

        // a:schemeClr
        if let Some(v) = &self.scheme_color {
            v.write_to(writer);
        }

        // a:srgbClr
        if let Some(v) = &self.rgb_color_model_hex {
            v.write_to(writer);
        }

        write_end_tag(writer, "a:gs");
    }
}
