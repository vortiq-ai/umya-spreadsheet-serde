// xdr:pic
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
    BlipFill,
    NonVisualPictureProperties,
    ShapeProperties,
};
use crate::{
    structs::raw::RawRelationships,
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct Picture {
    non_visual_picture_properties: NonVisualPictureProperties,
    blip_fill:                     BlipFill,
    shape_properties:              ShapeProperties,
    #[cfg_attr(feature = "serde", serde(default))]
    shape_style:                   Option<Box<super::ShapeStyle>>,
}

impl Picture {
    #[must_use]
    pub fn shape_style(&self) -> Option<&super::ShapeStyle> {
        self.shape_style.as_deref()
    }

    pub fn shape_style_mut(&mut self) -> Option<&mut super::ShapeStyle> {
        self.shape_style.as_deref_mut()
    }

    pub fn set_shape_style(&mut self, value: super::ShapeStyle) {
        self.shape_style = Some(Box::new(value));
    }

    #[inline]
    #[must_use]
    pub fn non_visual_picture_properties(&self) -> &NonVisualPictureProperties {
        &self.non_visual_picture_properties
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use non_visual_picture_properties()")]
    pub fn get_non_visual_picture_properties(&self) -> &NonVisualPictureProperties {
        self.non_visual_picture_properties()
    }

    #[inline]
    pub fn non_visual_picture_properties_mut(&mut self) -> &mut NonVisualPictureProperties {
        &mut self.non_visual_picture_properties
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use non_visual_picture_properties_mut()")]
    pub fn get_non_visual_picture_properties_mut(&mut self) -> &mut NonVisualPictureProperties {
        self.non_visual_picture_properties_mut()
    }

    #[inline]
    pub fn set_non_visual_picture_properties(&mut self, value: NonVisualPictureProperties) {
        self.non_visual_picture_properties = value;
    }

    #[inline]
    #[must_use]
    pub fn blip_fill(&self) -> &BlipFill {
        &self.blip_fill
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use blip_fill()")]
    pub fn get_blip_fill(&self) -> &BlipFill {
        self.blip_fill()
    }

    #[inline]
    pub fn blip_fill_mut(&mut self) -> &mut BlipFill {
        &mut self.blip_fill
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use blip_fill_mut()")]
    pub fn get_blip_fill_mut(&mut self) -> &mut BlipFill {
        self.blip_fill_mut()
    }

    #[inline]
    pub fn set_blip_fill(&mut self, value: BlipFill) {
        self.blip_fill = value;
    }

    #[inline]
    #[must_use]
    pub fn shape_properties(&self) -> &ShapeProperties {
        &self.shape_properties
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use shape_properties()")]
    pub fn get_shape_properties(&self) -> &ShapeProperties {
        self.shape_properties()
    }

    #[inline]
    pub fn shape_properties_mut(&mut self) -> &mut ShapeProperties {
        &mut self.shape_properties
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use shape_properties_mut()")]
    pub fn get_shape_properties_mut(&mut self) -> &mut ShapeProperties {
        self.shape_properties_mut()
    }

    #[inline]
    pub fn set_shape_properties(&mut self, value: ShapeProperties) {
        self.shape_properties = value;
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        owner: &BytesStart,
        drawing_relationships: Option<&RawRelationships>,
        session: &crate::reader::xlsx::SourceCacheSession,
    ) -> Result<(), crate::XlsxError> {
        let _guard = session.enter_child_scope(owner)?;
        let scope = crate::drawing::charts::NamespaceScope::default()
            .child(owner)
            .map_err(crate::xml::xml_error)?;
        let end = owner.name().as_ref().to_vec();
        let mut buffer = Vec::new();
        let mut seen = std::collections::HashSet::new();
        loop {
            match reader.read_event_into(&mut buffer)? {
                ref event @ (Event::Start(ref child) | Event::Empty(ref child)) => {
                    let empty = matches!(event, Event::Empty(_));
                    let name = scope
                        .child(child)
                        .and_then(|scope| scope.resolve(child.name().as_ref(), false))
                        .map_err(crate::xml::xml_error)?;
                    if name.namespace_uri != crate::helper::const_str::SHEET_DRAWING_NS {
                        return Err(crate::xml::xml_error("foreign spreadsheet drawing child"));
                    }
                    if !seen.insert(name.local_name.clone()) {
                        return Err(crate::xml::xml_error("duplicate spreadsheet drawing child"));
                    }
                    match name.local_name.as_str() {
                        "nvPicPr" => {
                            if empty {
                                return Err(crate::xml::xml_error(
                                    "empty nonvisual picture properties",
                                ));
                            }
                            self.non_visual_picture_properties
                                .set_attributes(reader, child, session)?;
                        }
                        "blipFill" => {
                            self.blip_fill.read_with_session(
                                reader,
                                child,
                                empty,
                                drawing_relationships,
                                session,
                            )?;
                        }
                        "spPr" => {
                            self.shape_properties.read_with_session(
                                reader,
                                child,
                                empty,
                                drawing_relationships,
                                session,
                            )?;
                        }
                        "style" => {
                            let mut value = super::ShapeStyle::default();
                            value.read_with_session(reader, child, empty, session)?;
                            self.set_shape_style(value);
                        }
                        _ => {
                            return Err(crate::xml::xml_error(
                                "unsupported spreadsheet drawing child",
                            ));
                        }
                    }
                }
                Event::End(child) if child.name().as_ref() == end.as_slice() => {
                    if !(seen.contains("nvPicPr")
                        && seen.contains("blipFill")
                        && seen.contains("spPr"))
                    {
                        return Err(crate::xml::xml_error(
                            "missing required spreadsheet drawing child",
                        ));
                    }
                    return Ok(());
                }
                Event::Eof => {
                    return Err(crate::xml::xml_error(
                        "unterminated spreadsheet drawing object",
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
        rel_list: &mut Vec<(String, String)>,
        ole_id: usize,
    ) -> Result<(), crate::XlsxError> {
        // xdr:pic
        write_start_tag(writer, "xdr:pic", vec![], false);

        // xdr:nvPicPr
        self.non_visual_picture_properties
            .write_to(writer, rel_list, ole_id)?;

        // xdr:blipFill
        self.blip_fill
            .write_to_as(writer, rel_list, "xdr:blipFill")?;

        // xdr:spPr
        self.shape_properties.write_to(writer, rel_list)?;

        if let Some(style) = &self.shape_style {
            style.write_to(writer)?;
        }

        write_end_tag(writer, "xdr:pic");
        Ok(())
    }
}
