// a:prstClr
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::{
        BytesStart,
        Event,
    },
};

use super::alpha::Alpha;
use crate::{
    drawing::get_unescaped_attribute,
    reader::driver::xml_read_loop,
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct PresetColor {
    val:   Box<str>,
    alpha: Option<Alpha>,
}

impl PresetColor {
    #[inline]
    #[must_use]
    pub fn val(&self) -> &str {
        &self.val
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use val()")]
    pub fn get_val(&self) -> &str {
        self.val()
    }

    #[inline]
    pub fn set_val<S: Into<String>>(&mut self, value: S) {
        self.val = value.into().into_boxed_str();
    }

    #[inline]
    #[must_use]
    pub fn alpha(&self) -> Option<&Alpha> {
        self.alpha.as_ref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use alpha()")]
    pub fn get_alpha(&self) -> Option<&Alpha> {
        self.alpha()
    }

    #[inline]
    pub fn alpha_mut(&mut self) -> Option<&mut Alpha> {
        self.alpha.as_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use alpha_mut()")]
    pub fn get_alpha_mut(&mut self) -> Option<&mut Alpha> {
        self.alpha_mut()
    }

    #[inline]
    pub fn set_alpha(&mut self, value: Alpha) {
        self.alpha = Some(value);
    }

    pub(crate) fn read_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
        empty: bool,
    ) {
        self.set_val(get_unescaped_attribute(e, b"val").unwrap());

        if empty {
            return;
        }
        let end_name = e.name().as_ref().to_vec();
        xml_read_loop!(reader,
                    ref event @ (Event::Start(ref e) | Event::Empty(ref e)) => {
                        let empty=matches!(event,Event::Empty(_));
                        match crate::reader::driver::local_name(e.name().into_inner()) {
        b"alpha" => { let mut value=Alpha::default(); value.set_attributes(reader,e); self.set_alpha(value); if !empty { crate::reader::driver::skip_element(reader,e); } },
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
        // a:prstClr
        write_start_tag(writer, "a:prstClr", vec![("val", &self.val).into()], false);

        // a:alpha
        if let Some(v) = &self.alpha {
            v.write_to(writer);
        }

        write_end_tag(writer, "a:prstClr");
    }
}
