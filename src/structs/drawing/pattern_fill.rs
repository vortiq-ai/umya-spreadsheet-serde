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
    reader::driver::try_xml_read_loop,
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
    ) -> Result<(), crate::XlsxError> {
        self.read_with_session(reader, e, false, None)
    }

    /// Reads this element the way master does: by local name within the
    /// `DrawingML` namespace, so any prefix bound to it is accepted and a
    /// foreign lookalike is not, and ending on the name the caller actually
    /// opened rather than a literal `a:` one. The fields stay this fork's.
    pub(crate) fn read_with_session<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        owner: &BytesStart,
        empty: bool,
        session: Option<&crate::reader::xlsx::SourceCacheSession>,
    ) -> Result<(), crate::XlsxError> {
        let _guard = session
            .map(|value| value.enter_child_scope(owner))
            .transpose()?
            .flatten();
        if let Some(value) = crate::reader::driver::read_attribute(owner, b"prst")? {
            self.set_preset(value);
        }
        if empty {
            return Ok(());
        }
        let end_name = owner.name().as_ref().to_vec();
        try_xml_read_loop!(
            reader,
            ref event @ (Event::Start(ref child) | Event::Empty(ref child)) => {
                let child_empty = matches!(event, Event::Empty(_));
                let native = match session {
                    Some(session) => session.accepts_child_namespace(
                        child,
                        crate::helper::const_str::DRAWINGML_MAIN_NS,
                    )?,
                    None => true,
                };
                match (native, crate::reader::driver::local_name(child.name().into_inner())) {
                    (true, b"fgClr") => {
                        self.foreground_color
                            .read_with_session(reader, child, child_empty, session)?;
                    }
                    (true, b"bgClr") => {
                        self.background_color
                            .read_with_session(reader, child, child_empty, session)?;
                    }
                    _ => {
                        // Never look inside what this element does not model: a native-looking
                        // grandchild of a foreign wrapper is not a child of this element.
                        if !child_empty {
                            crate::reader::driver::skip_element(reader, child)?;
                        }
                    }
                }
            },
            Event::End(ref end) if end.name().as_ref() == end_name => return Ok(()),
            Event::Eof => return Err(crate::xml::xml_error("unterminated DrawingML element")),
        );
    }

    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
    ) -> Result<(), crate::XlsxError> {
        // a:pattFill
        write_start_tag(
            writer,
            "a:pattFill",
            vec![("prst", &self.preset).into()],
            false,
        );

        // a:fgClr
        self.foreground_color.write_to(writer)?;

        // a:bgClr
        self.background_color.write_to(writer)?;

        write_end_tag(writer, "a:pattFill");

        Ok(())
    }
}
