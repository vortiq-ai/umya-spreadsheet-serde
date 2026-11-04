use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::{
        BytesStart,
        Event,
    },
};

use super::super::{
    BodyProperties,
    ListStyle,
    Paragraph,
};
use crate::writer::driver::{
    write_end_tag,
    write_start_tag,
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct TextBody {
    body_properties: BodyProperties,
    list_style:      ListStyle,
    paragraph:       Vec<Paragraph>,
}

impl TextBody {
    #[inline]
    #[must_use]
    pub fn body_properties(&self) -> &BodyProperties {
        &self.body_properties
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use body_properties()")]
    pub fn get_body_properties(&self) -> &BodyProperties {
        self.body_properties()
    }

    #[inline]
    pub fn body_properties_mut(&mut self) -> &mut BodyProperties {
        &mut self.body_properties
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use body_properties_mut()")]
    pub fn get_body_properties_mut(&mut self) -> &mut BodyProperties {
        self.body_properties_mut()
    }

    #[inline]
    pub fn set_body_properties(&mut self, value: BodyProperties) {
        self.body_properties = value;
    }

    #[inline]
    #[must_use]
    pub fn list_style(&self) -> &ListStyle {
        &self.list_style
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use list_style()")]
    pub fn get_list_style(&self) -> &ListStyle {
        self.list_style()
    }

    #[inline]
    pub fn list_style_mut(&mut self) -> &mut ListStyle {
        &mut self.list_style
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use list_style_mut()")]
    pub fn get_list_style_mut(&mut self) -> &mut ListStyle {
        self.list_style_mut()
    }

    #[inline]
    pub fn set_list_style(&mut self, value: ListStyle) {
        self.list_style = value;
    }

    #[inline]
    #[must_use]
    pub fn paragraph(&self) -> &[Paragraph] {
        &self.paragraph
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use paragraph()")]
    pub fn get_paragraph(&self) -> &[Paragraph] {
        self.paragraph()
    }

    #[inline]
    pub fn paragraph_mut(&mut self) -> &mut Vec<Paragraph> {
        &mut self.paragraph
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use paragraph_mut()")]
    pub fn get_paragraph_mut(&mut self) -> &mut Vec<Paragraph> {
        self.paragraph_mut()
    }

    #[inline]
    pub fn add_paragraph(&mut self, value: Paragraph) {
        self.paragraph.push(value);
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
                    if name.namespace_uri != crate::helper::const_str::DRAWINGML_MAIN_NS {
                        return Err(crate::xml::xml_error("foreign drawing text child"));
                    }
                    match name.local_name.as_str() {
                        "p" => {
                            let mut value = Paragraph::default();
                            if empty {
                                value.mark_empty_import();
                            } else {
                                value.read_with_session(reader, child, Some(session))?;
                            }
                            self.add_paragraph(value);
                        }
                        "bodyPr" => {
                            let mut value = BodyProperties::default();
                            value.read_with_session(reader, child, empty, Some(session))?;
                            self.set_body_properties(value);
                        }
                        "lstStyle" => {
                            let mut value = ListStyle::default();
                            if !empty {
                                value.read_with_session(reader, child, Some(session))?;
                            }
                            self.set_list_style(value);
                        }
                        _ => return Err(crate::xml::xml_error("unsupported drawing text child")),
                    }
                }
                Event::End(child) if child.name().as_ref() == end.as_slice() => return Ok(()),
                Event::Eof => return Err(crate::xml::xml_error("unterminated drawing text body")),
                _ => (),
            }
            buffer.clear();
        }
    }

    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
    ) -> Result<(), crate::XlsxError> {
        // xdr:txBody
        write_start_tag(writer, "xdr:txBody", vec![], false);

        // a:bodyPr
        self.body_properties.write_to(writer);

        // a:lstStyle
        self.list_style.write_to(writer)?;

        for content in &self.paragraph {
            content.write_to(writer)?;
        }

        write_end_tag(writer, "xdr:txBody");
        Ok(())
    }
}
