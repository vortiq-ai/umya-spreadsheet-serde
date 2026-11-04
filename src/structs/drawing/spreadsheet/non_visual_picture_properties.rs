// xdr:nvPicPr
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
    NonVisualDrawingProperties,
    NonVisualPictureDrawingProperties,
};
use crate::writer::driver::{
    write_end_tag,
    write_start_tag,
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct NonVisualPictureProperties {
    non_visual_drawing_properties:         NonVisualDrawingProperties,
    non_visual_picture_drawing_properties: NonVisualPictureDrawingProperties,
}

impl NonVisualPictureProperties {
    #[inline]
    #[must_use]
    pub fn non_visual_drawing_properties(&self) -> &NonVisualDrawingProperties {
        &self.non_visual_drawing_properties
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use non_visual_drawing_properties()")]
    pub fn get_non_visual_drawing_properties(&self) -> &NonVisualDrawingProperties {
        self.non_visual_drawing_properties()
    }

    #[inline]
    pub fn non_visual_drawing_properties_mut(&mut self) -> &mut NonVisualDrawingProperties {
        &mut self.non_visual_drawing_properties
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use non_visual_drawing_properties_mut()")]
    pub fn get_non_visual_drawing_properties_mut(&mut self) -> &mut NonVisualDrawingProperties {
        self.non_visual_drawing_properties_mut()
    }

    #[inline]
    pub fn set_non_visual_drawing_properties(&mut self, value: NonVisualDrawingProperties) {
        self.non_visual_drawing_properties = value;
    }

    #[inline]
    #[must_use]
    pub fn non_visual_picture_drawing_properties(&self) -> &NonVisualPictureDrawingProperties {
        &self.non_visual_picture_drawing_properties
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use non_visual_picture_drawing_properties()")]
    pub fn get_non_visual_picture_drawing_properties(&self) -> &NonVisualPictureDrawingProperties {
        self.non_visual_picture_drawing_properties()
    }

    #[inline]
    pub fn non_visual_picture_drawing_properties_mut(
        &mut self,
    ) -> &mut NonVisualPictureDrawingProperties {
        &mut self.non_visual_picture_drawing_properties
    }

    #[inline]
    #[deprecated(
        since = "3.0.0",
        note = "Use non_visual_picture_drawing_properties_mut()"
    )]
    pub fn get_non_visual_picture_drawing_properties_mut(
        &mut self,
    ) -> &mut NonVisualPictureDrawingProperties {
        self.non_visual_picture_drawing_properties_mut()
    }

    #[inline]
    pub fn set_non_visual_picture_drawing_properties(
        &mut self,
        value: NonVisualPictureDrawingProperties,
    ) {
        self.non_visual_picture_drawing_properties = value;
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        owner: &BytesStart,
        session: &crate::reader::xlsx::SourceCacheSession,
    ) -> Result<(), crate::XlsxError> {
        let parent = session.current_namespace_scope().unwrap_or_default();
        let scope = parent
            .child(owner)
            .map_err(super::extended_chart::drawing_error)?;
        let _guard = session.enter_child_scope(owner)?;
        let end = owner.name().as_ref().to_vec();
        let mut buffer = Vec::new();
        let mut seen = std::collections::HashSet::new();
        loop {
            match reader.read_event_into(&mut buffer)? {
                ref event @ (Event::Start(ref child) | Event::Empty(ref child)) => {
                    let empty = matches!(event, Event::Empty(_));
                    let child_scope = scope
                        .child(child)
                        .map_err(super::extended_chart::drawing_error)?;
                    let name = child_scope
                        .resolve(child.name().as_ref(), false)
                        .map_err(super::extended_chart::drawing_error)?;
                    if name.namespace_uri != crate::helper::const_str::SHEET_DRAWING_NS
                        || !seen.insert(name.local_name.clone())
                    {
                        return Err(super::extended_chart::drawing_error(
                            "foreign or duplicate nonvisual drawing child",
                        ));
                    }
                    match name.local_name.as_str() {
                        "cNvPr" => {
                            self.non_visual_drawing_properties =
                                NonVisualDrawingProperties::read_with_scope(
                                    reader, child, empty, &scope,
                                )
                                .map_err(super::extended_chart::drawing_error)?;
                        }
                        "cNvPicPr" => {
                            self.non_visual_picture_drawing_properties
                                .read_with_scope(reader, child, empty, &scope)?;
                        }
                        _ => {
                            return Err(super::extended_chart::drawing_error(
                                "unsupported nonvisual drawing child",
                            ));
                        }
                    }
                }
                Event::End(child) if child.name().as_ref() == end.as_slice() => {
                    if !seen.contains("cNvPr") || !seen.contains("cNvPicPr") {
                        return Err(super::extended_chart::drawing_error(
                            "missing required nonvisual drawing child",
                        ));
                    }
                    return Ok(());
                }
                Event::Eof => {
                    return Err(super::extended_chart::drawing_error(
                        "unterminated nonvisual drawing properties",
                    ));
                }
                ref event @ (Event::Text(_) | Event::CData(_) | Event::GeneralRef(_)) => {
                    if !crate::reader::driver::decode_xml_character_event(event)?
                        .is_some_and(|text| crate::xml::trim_xml_whitespace(&text).is_empty())
                    {
                        return Err(super::extended_chart::drawing_error(
                            "non-whitespace nonvisual drawing content",
                        ));
                    }
                }
                Event::Comment(_) | Event::PI(_) => (),
                _ => {
                    return Err(super::extended_chart::drawing_error(
                        "invalid nonvisual drawing event",
                    ));
                }
            }
            buffer.clear();
        }
    }

    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        relationships: &[(String, String)],
        ole_id: usize,
    ) -> Result<(), crate::XlsxError> {
        // xdr:nvPicPr
        write_start_tag(writer, "xdr:nvPicPr", vec![], false);

        // xdr:cNvPr
        self.non_visual_drawing_properties
            .write_to(writer, ole_id, relationships)?;

        // xdr:cNvPicPr
        self.non_visual_picture_drawing_properties
            .write_to(writer, relationships)?;

        write_end_tag(writer, "xdr:nvPicPr");
        Ok(())
    }

    pub(crate) fn visit_relationships(
        &self,
        visit: &mut dyn FnMut(super::DrawingRelationshipKind, &str),
    ) {
        self.non_visual_drawing_properties
            .visit_relationships(visit);
        self.non_visual_picture_drawing_properties
            .visit_relationships(visit);
    }

    pub(crate) fn visit_relationships_mut(
        &mut self,
        visit: &mut dyn FnMut(super::DrawingRelationshipKind, &mut String),
    ) {
        self.non_visual_drawing_properties
            .visit_relationships_mut(visit);
        self.non_visual_picture_drawing_properties
            .visit_relationships_mut(visit);
    }
}
