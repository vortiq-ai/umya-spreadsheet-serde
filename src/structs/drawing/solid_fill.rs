// a:solidFill
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
    rgb_color_model_hex::RgbColorModelHex,
    scheme_color::SchemeColor,
};
use crate::{
    drawing::SystemColor,
    reader::driver::xml_read_loop,
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct SolidFill {
    scheme_color:        Option<Box<SchemeColor>>,
    rgb_color_model_hex: Option<Box<RgbColorModelHex>>,
    system_color:        Option<Box<SystemColor>>,
}

impl SolidFill {
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
    pub fn set_scheme_color(&mut self, value: SchemeColor) {
        self.scheme_color = Some(Box::new(value));
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
    pub fn set_rgb_color_model_hex(&mut self, value: RgbColorModelHex) {
        self.rgb_color_model_hex = Some(Box::new(value));
    }

    #[inline]
    #[must_use]
    pub fn system_color(&self) -> Option<&SystemColor> {
        self.system_color.as_deref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use system_color()")]
    pub fn get_system_color(&self) -> Option<&SystemColor> {
        self.system_color()
    }

    #[inline]
    pub fn system_color_mut(&mut self) -> Option<&mut SystemColor> {
        self.system_color.as_deref_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use system_color_mut()")]
    pub fn get_system_color_mut(&mut self) -> Option<&mut SystemColor> {
        self.system_color_mut()
    }

    #[inline]
    pub fn set_system_color(&mut self, value: SystemColor) {
        self.system_color = Some(Box::new(value));
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
    ) {
        self.read_attributes(reader, e, false);
    }

    pub(crate) fn read_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
        empty: bool,
    ) {
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
        b"sysClr" => { let mut value=SystemColor::default(); value.set_attributes(reader,e,empty); self.set_system_color(value); },
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
        // a:solidFill
        write_start_tag(writer, "a:solidFill", vec![], false);
        if let Some(color) = &self.scheme_color {
            color.write_to(writer);
        }

        // a:srgbClr
        if let Some(hex) = &self.rgb_color_model_hex {
            hex.write_to(writer);
        }

        // a:sysClr
        if let Some(v) = &self.system_color {
            v.write_to(writer);
        }

        write_end_tag(writer, "a:solidFill");
    }
}
