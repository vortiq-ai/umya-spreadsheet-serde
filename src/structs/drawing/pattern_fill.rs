// a:pattFill
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
    BackgroundColor,
    ForegroundColor,
};
use crate::{
    drawing::get_unescaped_attribute,
    reader::driver::xml_read_loop,
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct PatternFill {
    preset:           Box<str>,
    foreground_color: ForegroundColor,
    background_color: BackgroundColor,
}

impl Default for PatternFill {
    #[inline]
    fn default() -> Self {
        Self {
            preset:           "pct5".into(),
            foreground_color: ForegroundColor::default(),
            background_color: BackgroundColor::default(),
        }
    }
}

impl PatternFill {
    #[inline]
    #[must_use]
    pub fn preset(&self) -> &str {
        &self.preset
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use preset()")]
    pub fn get_preset(&self) -> &str {
        self.preset()
    }

    #[inline]
    pub fn set_preset(&mut self, value: String) -> &mut Self {
        self.preset = value.into_boxed_str();
        self
    }

    #[inline]
    #[must_use]
    pub fn foreground_color(&self) -> &ForegroundColor {
        &self.foreground_color
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use foreground_color()")]
    pub fn get_foreground_color(&self) -> &ForegroundColor {
        self.foreground_color()
    }

    #[inline]
    pub fn foreground_color_mut(&mut self) -> &mut ForegroundColor {
        &mut self.foreground_color
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use foreground_color_mut()")]
    pub fn get_foreground_color_mut(&mut self) -> &mut ForegroundColor {
        self.foreground_color_mut()
    }

    #[inline]
    pub fn set_foreground_color(&mut self, value: ForegroundColor) -> &mut Self {
        self.foreground_color = value;
        self
    }

    #[inline]
    #[must_use]
    pub fn background_color(&self) -> &BackgroundColor {
        &self.background_color
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use background_color()")]
    pub fn get_background_color(&self) -> &BackgroundColor {
        self.background_color()
    }

    #[inline]
    pub fn background_color_mut(&mut self) -> &mut BackgroundColor {
        &mut self.background_color
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use background_color_mut()")]
    pub fn get_background_color_mut(&mut self) -> &mut BackgroundColor {
        self.background_color_mut()
    }

    #[inline]
    pub fn set_background_color(&mut self, value: BackgroundColor) -> &mut Self {
        self.background_color = value;
        self
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
    ) {
        self.set_attributes_impl(reader, e, None);
    }

    pub(crate) fn set_attributes_with_session<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
        session: &crate::reader::xlsx::SourceCacheSession,
    ) {
        self.set_attributes_impl(reader, e, Some(session));
    }

    fn set_attributes_impl<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        e: &BytesStart,
        session: Option<&crate::reader::xlsx::SourceCacheSession>,
    ) {
        let _scope_guard = session.and_then(|session| session.enter_child_scope(e));
        let end_name = e.name().as_ref().to_vec();
        if let Some(v) = get_unescaped_attribute(e, b"prst") {
            self.set_preset(v);
        }

        xml_read_loop!(
            reader,
            Event::Start(ref e) => {
                let child = if let Some(session) = session {
                    if session.accepts_child_namespace(
                        e,
                        crate::helper::const_str::DRAWINGML_MAIN_NS,
                    ) {
                        crate::reader::driver::local_name(e.name().into_inner())
                    } else {
                        b""
                    }
                } else {
                    match e.name().into_inner() {
                        b"a:fgClr" => b"fgClr".as_slice(),
                        b"a:bgClr" => b"bgClr".as_slice(),
                        _ => b"".as_slice(),
                    }
                };
                match child {
                    b"fgClr" => {
                        if let Some(session) = session {
                            self.foreground_color.set_attributes_with_session(
                                reader, e, session,
                            );
                        } else {
                            self.foreground_color.set_attributes(reader, e);
                        }
                    },
                    b"bgClr" => {
                        if let Some(session) = session {
                            self.background_color.set_attributes_with_session(
                                reader, e, session,
                            );
                        } else {
                            self.background_color.set_attributes(reader, e);
                        }
                    },
                    _ => {
                        if session.is_some() {
                            crate::reader::driver::skip_element(reader, e);
                        }
                    },
                }
            },
            Event::End(ref e) => {
                if (session.is_some() && e.name().as_ref() == end_name.as_slice())
                    || (session.is_none() && e.name().into_inner() == b"a:pattFill") {
                    return
                }
            },
            Event::Eof => panic!("Error: Could not find {} end element", "a:pattFill")
        );
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        // a:pattFill
        write_start_tag(
            writer,
            "a:pattFill",
            vec![("prst", &self.preset).into()],
            false,
        );

        // a:fgClr
        self.foreground_color.write_to(writer);

        // a:bgClr
        self.background_color.write_to(writer);

        write_end_tag(writer, "a:pattFill");
    }
}
