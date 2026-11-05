// a:fgClr
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
    reader::driver::try_xml_read_loop,
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct ForegroundColor {
    scheme_color: SchemeColor,
}

impl ForegroundColor {
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
    pub fn set_scheme_color(&mut self, value: SchemeColor) -> &mut ForegroundColor {
        self.scheme_color = value;
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
                    (true, b"schemeClr") => {
                        self.scheme_color
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
        // a:fgClr
        write_start_tag(writer, "a:fgClr", vec![], false);

        // a:schemeClr
        self.scheme_color.write_to(writer)?;

        write_end_tag(writer, "a:fgClr");

        Ok(())
    }
}
