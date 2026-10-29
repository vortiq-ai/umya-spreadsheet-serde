// a:bgClr
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::{
        BytesStart,
        Event,
    },
};

use super::SchemeColor;
use crate::{
    reader::driver::xml_read_loop,
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct BackgroundColor {
    scheme_color: SchemeColor,
}

impl BackgroundColor {
    #[inline]
    #[must_use]
    pub fn scheme_color(&self) -> &SchemeColor {
        &self.scheme_color
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use scheme_color()")]
    pub fn get_scheme_color(&self) -> &SchemeColor {
        self.scheme_color()
    }

    #[inline]
    pub fn scheme_color_mut(&mut self) -> &mut SchemeColor {
        &mut self.scheme_color
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use scheme_color_mut()")]
    pub fn get_scheme_color_mut(&mut self) -> &mut SchemeColor {
        self.scheme_color_mut()
    }

    #[inline]
    pub fn set_scheme_color(&mut self, value: SchemeColor) -> &mut BackgroundColor {
        self.scheme_color = value;
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
        owner: &BytesStart,
        session: Option<&crate::reader::xlsx::SourceCacheSession>,
    ) {
        let _scope_guard = session.and_then(|session| session.enter_child_scope(owner));
        let end_name = owner.name().as_ref().to_vec();
        xml_read_loop!(
            reader,
            ref n @ (Event::Empty(ref e) | Event::Start(ref e)) => {
                let is_empty = matches!(n, Event::Empty(_));
                let native = if let Some(session) = session {
                    session.accepts_child_namespace(
                        e,
                        crate::helper::const_str::DRAWINGML_MAIN_NS,
                    )
                        && crate::reader::driver::local_name(e.name().into_inner()) == b"schemeClr"
                } else {
                    e.name().into_inner() == b"a:schemeClr"
                };
                if native {
                    if let Some(session) = session {
                        self.scheme_color.set_attributes_with_session(
                            reader, e, is_empty, session,
                        );
                    } else {
                        self.scheme_color.set_attributes(reader, e, is_empty);
                    }
                } else if session.is_some() && !is_empty {
                    crate::reader::driver::skip_element(reader, e);
                }
            },
            Event::End(ref e) => {
                if (session.is_some() && e.name().as_ref() == end_name.as_slice())
                    || (session.is_none() && e.name().into_inner() == b"a:bgClr") {
                    return
                }
            },
            Event::Eof => panic!("Error: Could not find {} end element", "a:bgClr")
        );
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        // a:bgClr
        write_start_tag(writer, "a:bgClr", vec![], false);

        // a:schemeClr
        self.scheme_color.write_to(writer);

        write_end_tag(writer, "a:bgClr");
    }
}
