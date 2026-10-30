// xdr:grpSpPr
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::{
        BytesStart,
        Event,
    },
};

use super::super::Transform2D;
use crate::writer::driver::{
    write_end_tag,
    write_start_tag,
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct GroupShapeProperties {
    transform2d: Option<Transform2D>,
}

impl GroupShapeProperties {
    #[inline]
    #[must_use]
    pub fn transform2d(&self) -> Option<&Transform2D> {
        self.transform2d.as_ref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use transform2d()")]
    pub fn get_transform2d(&self) -> Option<&Transform2D> {
        self.transform2d()
    }

    #[inline]
    pub fn transform2d_mut(&mut self) -> Option<&mut Transform2D> {
        self.transform2d.as_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use transform2d_mut()")]
    pub fn get_transform2d_mut(&mut self) -> Option<&mut Transform2D> {
        self.transform2d_mut()
    }

    #[inline]
    pub fn set_transform2d(&mut self, value: Transform2D) -> &mut Self {
        self.transform2d = Some(value);
        self
    }

    pub(crate) fn read_with_session<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        owner: &BytesStart,
        empty: bool,
        session: &crate::reader::xlsx::SourceCacheSession,
    ) -> Result<(), crate::XlsxError> {
        let _guard = session.enter_child_scope(owner)?;
        let scope = crate::drawing::charts::NamespaceScope::default()
            .child(owner)
            .map_err(crate::xml::xml_error)?;
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
                    if !name.is(crate::helper::const_str::DRAWINGML_MAIN_NS, "xfrm")
                        || self.transform2d.is_some()
                    {
                        return Err(crate::xml::xml_error(
                            "unsupported or duplicate group shape property",
                        ));
                    }
                    let mut value = Transform2D::default();
                    value.read_with_session(reader, child, empty, Some(session))?;
                    self.set_transform2d(value);
                }
                Event::End(child) if child.name().as_ref() == end.as_slice() => return Ok(()),
                Event::Eof => {
                    return Err(crate::xml::xml_error("unterminated group shape properties"));
                }
                _ => (),
            }
            buffer.clear();
        }
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        // xdr:grpSpPr
        write_start_tag(writer, "xdr:grpSpPr", vec![], false);

        // a:xfrm
        if let Some(v) = &self.transform2d {
            v.write_to(writer);
        }

        write_end_tag(writer, "xdr:grpSpPr");
    }
}
