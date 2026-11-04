// a:themeElements
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
    ColorScheme,
    FontScheme,
    FormatScheme,
};
use crate::{
    XlsxError,
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ThemeElements {
    color_scheme:  ColorScheme,
    font_scheme:   FontScheme,
    format_scheme: FormatScheme,
}

impl ThemeElements {
    #[inline]
    #[must_use]
    pub fn color_scheme(&self) -> &ColorScheme {
        &self.color_scheme
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use color_scheme()")]
    pub fn get_color_scheme(&self) -> &ColorScheme {
        self.color_scheme()
    }

    #[inline]
    pub fn color_scheme_mut(&mut self) -> &mut ColorScheme {
        &mut self.color_scheme
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use color_scheme_mut()")]
    pub fn get_color_scheme_mut(&mut self) -> &mut ColorScheme {
        self.color_scheme_mut()
    }

    #[inline]
    pub fn set_color_scheme(&mut self, value: ColorScheme) -> &mut Self {
        self.color_scheme = value;
        self
    }

    #[inline]
    #[must_use]
    pub fn font_scheme(&self) -> &FontScheme {
        &self.font_scheme
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use font_scheme()")]
    pub fn get_font_scheme(&self) -> &FontScheme {
        self.font_scheme()
    }

    #[inline]
    pub fn font_scheme_mut(&mut self) -> &mut FontScheme {
        &mut self.font_scheme
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use font_scheme_mut()")]
    pub fn get_font_scheme_mut(&mut self) -> &mut FontScheme {
        self.font_scheme_mut()
    }

    #[inline]
    pub fn set_font_scheme(&mut self, value: FontScheme) -> &mut Self {
        self.font_scheme = value;
        self
    }

    #[inline]
    #[must_use]
    pub fn format_scheme(&self) -> &FormatScheme {
        &self.format_scheme
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use format_scheme()")]
    pub fn get_format_scheme(&self) -> &FormatScheme {
        self.format_scheme()
    }

    #[inline]
    pub fn format_scheme_mut(&mut self) -> &mut FormatScheme {
        &mut self.format_scheme
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use format_scheme_mut()")]
    pub fn get_format_scheme_mut(&mut self) -> &mut FormatScheme {
        self.format_scheme_mut()
    }

    #[inline]
    pub fn set_format_scheme(&mut self, value: FormatScheme) -> &mut Self {
        self.format_scheme = value;
        self
    }

    pub(crate) fn read_with_session<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        owner: &BytesStart<'_>,
        empty: bool,
        session: Option<&crate::reader::xlsx::SourceCacheSession>,
    ) -> Result<(), XlsxError> {
        let _scope_guard = session
            .map(|session| session.enter_child_scope(owner))
            .transpose()?
            .flatten();
        crate::reader::driver::validate_attributes(owner)?;

        if empty {
            return Err(crate::xml::xml_error("Theme owner requires child content"));
        }
        let mut seen = 0u32;
        let end = owner.name().as_ref().to_vec();
        let mut buffer = Vec::new();
        loop {
            match reader.read_event_into(&mut buffer)? {
                ref event @ (Event::Start(ref child) | Event::Empty(ref child)) => {
                    let child_empty = matches!(event, Event::Empty(_));
                    let native = match session {
                        Some(session) => session.accepts_child_namespace(
                            child,
                            crate::helper::const_str::DRAWINGML_MAIN_NS,
                        )?,
                        None => true,
                    };
                    if !native {
                        return Err(crate::xml::xml_error("foreign Theme child"));
                    }
                    crate::reader::driver::validate_attributes(child)?;
                    match crate::reader::driver::local_name(child.name().as_ref()) {
                        b"clrScheme" => {
                            if seen & 1 != 0 {
                                return Err(crate::xml::xml_error("duplicate Theme child"));
                            }
                            seen |= 1;
                            self.color_scheme.read_with_session(
                                reader,
                                child,
                                child_empty,
                                session,
                            )?;
                        }
                        b"fontScheme" => {
                            if seen & (1 << 1) != 0 {
                                return Err(crate::xml::xml_error("duplicate Theme child"));
                            }
                            seen |= 1 << 1;
                            self.font_scheme.read_with_session(
                                reader,
                                child,
                                child_empty,
                                session,
                            )?;
                        }
                        b"fmtScheme" => {
                            if seen & (1 << 2) != 0 {
                                return Err(crate::xml::xml_error("duplicate Theme child"));
                            }
                            seen |= 1 << 2;
                            self.format_scheme.read_with_session(
                                reader,
                                child,
                                child_empty,
                                session,
                            )?;
                        }
                        _ => {
                            if !child_empty {
                                crate::reader::driver::skip_element(reader, child)?;
                            }
                        }
                    }
                }
                Event::End(ref child) if child.name().as_ref() == end.as_slice() => {
                    if seen & 7 != 7 {
                        return Err(crate::xml::xml_error(
                            "Theme owner is missing required children",
                        ));
                    }
                    return Ok(());
                }
                Event::Text(ref text)
                    if !text
                        .iter()
                        .all(|byte| matches!(byte, b' ' | b'\t' | b'\r' | b'\n')) =>
                {
                    return Err(crate::xml::xml_error("unexpected text in Theme container"));
                }
                Event::Eof => return Err(crate::xml::xml_error("unterminated Theme owner")),
                _ => (),
            }
            buffer.clear();
        }
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) -> Result<(), XlsxError> {
        // a:themeElements
        write_start_tag(writer, "a:themeElements", vec![], false);

        // a:clrScheme
        self.color_scheme.write_to(writer)?;

        // a:fontScheme
        self.font_scheme.write_to(writer);

        // a:fmtScheme
        self.format_scheme.write_to(writer)?;

        write_end_tag(writer, "a:themeElements");
        Ok(())
    }
}
