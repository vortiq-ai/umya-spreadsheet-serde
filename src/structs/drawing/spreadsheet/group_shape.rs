// xdr:grpSp
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
    GroupShapeProperties,
    NonVisualGroupShapeProperties,
    Picture,
    Shape,
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
pub struct GroupShape {
    non_visual_group_shape_properties: NonVisualGroupShapeProperties,
    group_shape_properties:            GroupShapeProperties,
    content:                           Vec<super::DrawingObject>,
}

impl GroupShape {
    #[inline]
    #[must_use]
    pub fn non_visual_group_shape_properties(&self) -> &NonVisualGroupShapeProperties {
        &self.non_visual_group_shape_properties
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use non_visual_group_shape_properties()")]
    pub fn get_non_visual_group_shape_properties(&self) -> &NonVisualGroupShapeProperties {
        self.non_visual_group_shape_properties()
    }

    #[inline]
    pub fn non_visual_group_shape_properties_mut(&mut self) -> &mut NonVisualGroupShapeProperties {
        &mut self.non_visual_group_shape_properties
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use non_visual_group_shape_properties_mut()")]
    pub fn get_non_visual_group_shape_properties_mut(
        &mut self,
    ) -> &mut NonVisualGroupShapeProperties {
        self.non_visual_group_shape_properties_mut()
    }

    #[inline]
    pub fn set_non_visual_group_shape_properties(&mut self, value: NonVisualGroupShapeProperties) {
        self.non_visual_group_shape_properties = value;
    }

    #[inline]
    #[must_use]
    pub fn group_shape_properties(&self) -> &GroupShapeProperties {
        &self.group_shape_properties
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use group_shape_properties()")]
    pub fn get_group_shape_properties(&self) -> &GroupShapeProperties {
        self.group_shape_properties()
    }

    #[inline]
    pub fn group_shape_properties_mut(&mut self) -> &mut GroupShapeProperties {
        &mut self.group_shape_properties
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use group_shape_properties_mut()")]
    pub fn get_group_shape_properties_mut(&mut self) -> &mut GroupShapeProperties {
        self.group_shape_properties_mut()
    }

    #[inline]
    pub fn set_group_shape_properties(&mut self, value: GroupShapeProperties) {
        self.group_shape_properties = value;
    }

    #[must_use]
    pub fn content(&self) -> &[super::DrawingObject] {
        &self.content
    }

    pub fn content_mut(&mut self) -> &mut Vec<super::DrawingObject> {
        &mut self.content
    }

    pub fn add_object(&mut self, value: super::DrawingObject) {
        self.content.push(value);
    }

    pub fn picture_collection(&self) -> impl Iterator<Item = &Picture> {
        self.content.iter().filter_map(|value| match value {
            super::DrawingObject::Picture(value) => Some(value.as_ref()),
            _ => None,
        })
    }

    pub fn picture_collection_mut(&mut self) -> impl Iterator<Item = &mut Picture> {
        self.content.iter_mut().filter_map(|value| match value {
            super::DrawingObject::Picture(value) => Some(value.as_mut()),
            _ => None,
        })
    }

    pub fn add_picture_collection(&mut self, value: Picture) {
        self.add_object(super::DrawingObject::Picture(Box::new(value)));
    }

    pub fn shape_collection(&self) -> impl Iterator<Item = &Shape> {
        self.content.iter().filter_map(|value| match value {
            super::DrawingObject::Shape(value) => Some(value.as_ref()),
            _ => None,
        })
    }

    pub fn shape_collection_mut(&mut self) -> impl Iterator<Item = &mut Shape> {
        self.content.iter_mut().filter_map(|value| match value {
            super::DrawingObject::Shape(value) => Some(value.as_mut()),
            _ => None,
        })
    }

    pub fn add_shape_collection(&mut self, value: Shape) {
        self.add_object(super::DrawingObject::Shape(Box::new(value)));
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        owner: &BytesStart,
        drawing_relationships: Option<&RawRelationships>,
        session: &crate::reader::xlsx::SourceCacheSession,
        provider: &mut dyn crate::reader::xlsx::ChartPartProvider,
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
                    if matches!(name.local_name.as_str(), "nvGrpSpPr" | "grpSpPr")
                        && !seen.insert(name.local_name.clone())
                    {
                        return Err(crate::xml::xml_error("duplicate spreadsheet drawing child"));
                    }
                    match name.local_name.as_str() {
                        "nvGrpSpPr" => {
                            if empty {
                                return Err(crate::xml::xml_error(
                                    "empty nonvisual group properties",
                                ));
                            }
                            self.non_visual_group_shape_properties
                                .set_attributes(reader, child, session)?;
                        }
                        "grpSpPr" => {
                            self.group_shape_properties
                                .read_with_session(reader, child, empty, session)?;
                        }
                        name @ ("pic" | "sp" | "grpSp" | "cxnSp" | "graphicFrame") => {
                            let value = if name == "graphicFrame" {
                                let element = crate::drawing::charts::OpaqueElement::read(
                                    reader, child, empty, &scope,
                                )
                                .map_err(crate::xml::xml_error)?;
                                super::DrawingObject::read(
                                    &element,
                                    drawing_relationships,
                                    session,
                                    provider,
                                )?
                            } else {
                                super::DrawingObject::read_ordinary(
                                    reader,
                                    child,
                                    empty,
                                    name,
                                    drawing_relationships,
                                    session,
                                    provider,
                                )?
                            };
                            self.add_object(value);
                        }
                        _ => {
                            return Err(crate::xml::xml_error(
                                "unsupported spreadsheet drawing child",
                            ));
                        }
                    }
                }
                Event::End(child) if child.name().as_ref() == end.as_slice() => {
                    if !(seen.contains("nvGrpSpPr") && seen.contains("grpSpPr")) {
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
        // xdr:grpSp
        write_start_tag(writer, "xdr:grpSp", vec![], false);

        // xdr:nvGrpSpPr
        self.non_visual_group_shape_properties
            .write_to(writer, rel_list, ole_id)?;

        // xdr:grpSpPr
        self.group_shape_properties.write_to(writer);

        for object in &self.content {
            object.write_to(writer, rel_list)?;
        }

        write_end_tag(writer, "xdr:grpSp");
        Ok(())
    }
}
