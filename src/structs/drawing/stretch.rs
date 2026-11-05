// a:stretch
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::{
        BytesStart,
        Event,
    },
};

use super::fill_rectangle::FillRectangle;
use crate::writer::driver::{
    write_end_tag,
    write_start_tag,
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct Stretch {
    fill_rectangle: Option<Box<FillRectangle>>,
}

impl Stretch {
    #[inline]
    #[must_use]
    pub fn fill_rectangle(&self) -> Option<&FillRectangle> {
        self.fill_rectangle.as_deref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use fill_rectangle()")]
    pub fn get_fill_rectangle(&self) -> Option<&FillRectangle> {
        self.fill_rectangle()
    }

    #[inline]
    pub fn fill_rectangle_mut(&mut self) -> Option<&mut FillRectangle> {
        self.fill_rectangle.as_deref_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use fill_rectangle_mut()")]
    pub fn get_fill_rectangle_mut(&mut self) -> Option<&mut FillRectangle> {
        self.fill_rectangle_mut()
    }

    #[inline]
    pub fn set_fill_rectangle(&mut self, value: FillRectangle) {
        self.fill_rectangle = Some(Box::new(value));
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
        let scope = match session
            .and_then(crate::reader::xlsx::SourceCacheSession::current_namespace_scope)
        {
            Some(scope) => scope,
            None => crate::drawing::charts::NamespaceScope::default()
                .child(owner)
                .map_err(crate::xml::xml_error)?,
        };
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
                    if !name.is(crate::helper::const_str::DRAWINGML_MAIN_NS, "fillRect")
                        || self.fill_rectangle.is_some()
                    {
                        return Err(crate::xml::xml_error(
                            "unsupported or duplicate stretch child",
                        ));
                    }
                    let mut value = FillRectangle::default();
                    value.read_with_session(reader, child, empty, session)?;
                    self.set_fill_rectangle(value);
                }
                Event::End(child) if child.name().as_ref() == end.as_slice() => return Ok(()),
                Event::Eof => return Err(crate::xml::xml_error("unterminated image stretch")),
                _ => (),
            }
            buffer.clear();
        }
    }

    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
    ) -> Result<(), crate::XlsxError> {
        // a:stretch
        match &self.fill_rectangle {
            Some(value) => {
                write_start_tag(writer, "a:stretch", vec![], false);
                value.write_to(writer)?;
                write_end_tag(writer, "a:stretch");
            }
            None => {
                write_start_tag(writer, "a:stretch", vec![], true);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    // Tests for master's SourceRectangle fill model are absent: this fork stores the four
    // insets directly and cannot express the percentage forms those tests assert. They
    // return with the deferred fill-rectangle work.
    use super::*;
    use crate::drawing::charts::NamespaceScope;
    fn read(xml: &str) -> Result<Stretch, crate::XlsxError> {
        let mut reader = Reader::from_reader(xml.as_bytes());
        let (owner, empty) = match reader.read_event()? {
            Event::Start(owner) => (owner, false),
            Event::Empty(owner) => (owner, true),
            _ => return Err(crate::xml::xml_error("missing stretch owner")),
        };
        let session = crate::reader::xlsx::SourceCacheSession::immediate();
        let _scope = session.push_extension_scope(
            NamespaceScope::default()
                .child(&owner)
                .map_err(crate::xml::xml_error)?,
        );
        let mut value = Stretch::default();
        value.read_with_session(&mut reader, &owner, empty, Some(&session))?;
        Ok(value)
    }
    fn write(value: &Stretch) -> String {
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        value.write_to(&mut writer).unwrap();
        String::from_utf8(writer.into_inner().into_inner()).unwrap()
    }
}
