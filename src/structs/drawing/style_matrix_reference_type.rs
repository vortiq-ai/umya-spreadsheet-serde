// a:lnRef
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
use crate::writer::driver::{
    write_end_tag,
    write_start_tag,
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct StyleMatrixReferenceType {
    index:        Box<str>,
    scheme_color: Option<Box<SchemeColor>>,
}

impl StyleMatrixReferenceType {
    #[inline]
    #[must_use]
    pub fn index(&self) -> &str {
        &self.index
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use index()")]
    pub fn get_index(&self) -> &str {
        self.index()
    }

    #[inline]
    pub fn set_index<S: Into<String>>(&mut self, value: S) {
        self.index = value.into().into_boxed_str();
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
    pub fn set_scheme_color(&mut self, value: SchemeColor) {
        self.scheme_color = Some(Box::new(value));
    }

    pub(crate) fn read_with_session<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        owner: &BytesStart,
        empty: bool,
        session: Option<&crate::reader::xlsx::SourceCacheSession>,
    ) -> Result<(), crate::XlsxError> {
        let _guard = session
            .map(|session| session.enter_child_scope(owner))
            .transpose()?
            .flatten();
        let scope = crate::drawing::charts::NamespaceScope::default()
            .child(owner)
            .map_err(crate::xml::xml_error)?;
        let index = crate::reader::driver::require_attribute(owner, b"idx")?;
        if crate::reader::driver::local_name(owner.name().as_ref()) == b"fontRef" {
            if !matches!(index.as_str(), "major" | "minor" | "none") {
                return Err(crate::xml::xml_error(
                    "invalid drawing font reference index",
                ));
            }
        } else {
            crate::xml::parse_u32(&index).map_err(crate::xml::xml_error)?;
        }
        self.set_index(index);
        if empty {
            return Ok(());
        }
        let end = owner.name().as_ref().to_vec();
        let mut buffer = Vec::new();
        loop {
            match reader.read_event_into(&mut buffer)? {
                ref event @ (Event::Start(ref child) | Event::Empty(ref child)) => {
                    let empty = matches!(event, Event::Empty(_));
                    let name = scope
                        .child(child)
                        .and_then(|scope| scope.resolve(child.name().as_ref(), false))
                        .map_err(crate::xml::xml_error)?;
                    if !name.is(crate::helper::const_str::DRAWINGML_MAIN_NS, "schemeClr")
                        || self.scheme_color.is_some()
                    {
                        return Err(crate::xml::xml_error(
                            "unsupported or duplicate drawing style color",
                        ));
                    }
                    let mut value = SchemeColor::default();
                    value.read_with_session(reader, child, empty, session)?;
                    self.set_scheme_color(value);
                }
                Event::End(child) if child.name().as_ref() == end.as_slice() => return Ok(()),
                Event::Eof => {
                    return Err(crate::xml::xml_error(
                        "unterminated drawing style reference",
                    ));
                }
                _ => (),
            }
            buffer.clear();
        }
    }

    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        tag_name: &str,
    ) -> Result<(), crate::XlsxError> {
        if let Some(color) = &self.scheme_color {
            write_start_tag(writer, tag_name, vec![("idx", &self.index).into()], false);
            // a:schemeClr
            color.write_to(writer)?;
            write_end_tag(writer, tag_name);
        } else {
            write_start_tag(writer, tag_name, vec![("idx", &self.index).into()], true);
        }
        Ok(())
    }
}
