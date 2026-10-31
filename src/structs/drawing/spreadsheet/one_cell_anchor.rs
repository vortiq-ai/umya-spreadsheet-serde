// xdr:oneCellAnchor
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
    ConnectionShape,
    Extent,
    GraphicFrame,
    GroupShape,
    MarkerType,
    Picture,
    Shape,
};
use crate::{
    structs::raw::RawRelationships,
    traits::AdjustmentCoordinate,
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct OneCellAnchor {
    from_marker:      MarkerType,
    extent:           Extent,
    group_shape:      Option<Box<GroupShape>>,
    graphic_frame:    Option<Box<GraphicFrame>>,
    shape:            Option<Box<Shape>>,
    picture:          Option<Box<Picture>>,
    #[cfg_attr(feature = "serde", serde(default))]
    client_data:      Option<Box<super::DrawingClientData>>,
    #[cfg_attr(feature = "serde", serde(default))]
    connection_shape: Option<Box<ConnectionShape>>,
}

impl OneCellAnchor {
    #[inline]
    #[must_use]
    pub fn from_marker(&self) -> &MarkerType {
        &self.from_marker
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use from_marker()")]
    pub fn get_from_marker(&self) -> &MarkerType {
        self.from_marker()
    }

    #[inline]
    pub fn from_marker_mut(&mut self) -> &mut MarkerType {
        &mut self.from_marker
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use from_marker_mut()")]
    pub fn get_from_marker_mut(&mut self) -> &mut MarkerType {
        self.from_marker_mut()
    }

    #[inline]
    pub fn set_from_marker(&mut self, value: MarkerType) -> &mut OneCellAnchor {
        self.from_marker = value;
        self
    }

    #[inline]
    #[must_use]
    pub fn extent(&self) -> &Extent {
        &self.extent
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use extent()")]
    pub fn get_extent(&self) -> &Extent {
        self.extent()
    }

    #[inline]
    pub fn extent_mut(&mut self) -> &mut Extent {
        &mut self.extent
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use extent_mut()")]
    pub fn get_extent_mut(&mut self) -> &mut Extent {
        self.extent_mut()
    }

    #[inline]
    pub fn set_extent(&mut self, value: Extent) -> &mut OneCellAnchor {
        self.extent = value;
        self
    }

    #[inline]
    #[must_use]
    pub fn group_shape(&self) -> Option<&GroupShape> {
        self.group_shape.as_deref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use group_shape()")]
    pub fn get_group_shape(&self) -> Option<&GroupShape> {
        self.group_shape()
    }

    #[inline]
    pub fn group_shape_mut(&mut self) -> Option<&mut GroupShape> {
        self.group_shape.as_deref_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use group_shape_mut()")]
    pub fn get_group_shape_mut(&mut self) -> Option<&mut GroupShape> {
        self.group_shape_mut()
    }

    #[inline]
    pub fn set_group_shape(&mut self, value: GroupShape) -> &mut Self {
        self.group_shape = Some(Box::new(value));
        self
    }

    #[inline]
    #[must_use]
    pub fn shape(&self) -> Option<&Shape> {
        self.shape.as_deref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use shape()")]
    pub fn get_shape(&self) -> Option<&Shape> {
        self.shape()
    }

    #[inline]
    pub fn shape_mut(&mut self) -> Option<&mut Shape> {
        self.shape.as_deref_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use shape_mut()")]
    pub fn get_shape_mut(&mut self) -> Option<&mut Shape> {
        self.shape_mut()
    }

    #[inline]
    pub fn set_shape(&mut self, value: Shape) -> &mut OneCellAnchor {
        self.shape = Some(Box::new(value));
        self
    }

    #[inline]
    #[must_use]
    pub fn connection_shape(&self) -> Option<&ConnectionShape> {
        self.connection_shape.as_deref()
    }

    #[inline]
    pub fn connection_shape_mut(&mut self) -> Option<&mut ConnectionShape> {
        self.connection_shape.as_deref_mut()
    }

    #[inline]
    pub fn set_connection_shape(&mut self, value: ConnectionShape) -> &mut Self {
        self.connection_shape = Some(Box::new(value));
        self
    }

    #[inline]
    #[must_use]
    pub fn picture(&self) -> Option<&Picture> {
        self.picture.as_deref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use picture()")]
    pub fn get_picture(&self) -> Option<&Picture> {
        self.picture()
    }

    #[inline]
    pub fn picture_mut(&mut self) -> Option<&mut Picture> {
        self.picture.as_deref_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use picture_mut()")]
    pub fn get_picture_mut(&mut self) -> Option<&mut Picture> {
        self.picture_mut()
    }

    #[inline]
    pub fn set_picture(&mut self, value: Picture) -> &mut Self {
        self.picture = Some(Box::new(value));
        self
    }

    #[inline]
    #[must_use]
    pub fn graphic_frame(&self) -> Option<&GraphicFrame> {
        self.graphic_frame.as_deref()
    }

    #[inline]
    pub fn graphic_frame_mut(&mut self) -> Option<&mut GraphicFrame> {
        self.graphic_frame.as_deref_mut()
    }

    #[inline]
    pub fn set_graphic_frame(&mut self, value: GraphicFrame) -> &mut Self {
        self.graphic_frame = Some(Box::new(value));
        self
    }

    #[inline]
    pub(crate) fn is_image(&self) -> bool {
        let missing_embedded_image = self
            .picture()
            .is_some_and(|picture| picture.blip_fill().blip().image().image_name().is_empty())
            || self.group_shape().is_some_and(|group| {
                group
                    .content()
                    .iter()
                    .flat_map(|child| child.media_objects())
                    .any(|image| image.image_name().is_empty())
            });
        (self.picture.is_some() || self.group_shape.is_some()) && !missing_embedded_image
    }

    #[must_use]
    pub fn client_data(&self) -> Option<&super::DrawingClientData> {
        self.client_data.as_deref()
    }

    pub fn client_data_mut(&mut self) -> Option<&mut super::DrawingClientData> {
        self.client_data.as_deref_mut()
    }

    pub fn set_client_data(&mut self, value: super::DrawingClientData) -> &mut Self {
        self.client_data = Some(Box::new(value));
        self
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
        let mut objects = 0_usize;
        loop {
            match reader.read_event_into(&mut buffer)? {
                ref event @ (Event::Start(ref child) | Event::Empty(ref child)) => {
                    let empty = matches!(event, Event::Empty(_));
                    let name = scope
                        .child(child)
                        .and_then(|scope| scope.resolve(child.name().as_ref(), false))
                        .map_err(crate::xml::xml_error)?;
                    if name.namespace_uri != crate::helper::const_str::SHEET_DRAWING_NS
                        || !seen.insert(name.local_name.clone())
                    {
                        return Err(crate::xml::xml_error(
                            "foreign or duplicate drawing anchor child",
                        ));
                    }
                    if matches!(
                        name.local_name.as_str(),
                        "sp" | "pic" | "grpSp" | "cxnSp" | "graphicFrame"
                    ) {
                        objects += 1;
                        if objects > 1 {
                            return Err(crate::xml::xml_error(
                                "drawing anchor has multiple objects",
                            ));
                        }
                    }
                    match name.local_name.as_str() {
                        "from" => {
                            if empty {
                                return Err(crate::xml::xml_error("empty drawing marker"));
                            }
                            self.from_marker.set_attributes(reader, child)?;
                        }
                        "grpSp" => {
                            if empty {
                                return Err(crate::xml::xml_error("empty drawing group"));
                            }
                            let mut value = GroupShape::default();
                            value.set_attributes(
                                reader,
                                child,
                                drawing_relationships,
                                session,
                                provider,
                            )?;
                            self.set_group_shape(value);
                        }
                        "graphicFrame" => {
                            if empty {
                                return Err(crate::xml::xml_error("empty graphic frame"));
                            }
                            let mut value = GraphicFrame::default();
                            value.set_attributes(
                                reader,
                                child,
                                drawing_relationships,
                                session,
                                provider,
                            )?;
                            self.set_graphic_frame(value);
                        }
                        "sp" => {
                            if empty {
                                return Err(crate::xml::xml_error("empty drawing shape"));
                            }
                            let mut value = Shape::default();
                            value.set_attributes(reader, child, drawing_relationships, session)?;
                            self.set_shape(value);
                        }
                        "pic" => {
                            if empty {
                                return Err(crate::xml::xml_error("empty drawing picture"));
                            }
                            let mut value = Picture::default();
                            value.set_attributes(reader, child, drawing_relationships, session)?;
                            self.set_picture(value);
                        }
                        "cxnSp" => {
                            if empty {
                                return Err(crate::xml::xml_error("empty drawing connector"));
                            }
                            let mut value = ConnectionShape::default();
                            value.set_attributes(reader, child, drawing_relationships, session)?;
                            self.set_connection_shape(value);
                        }
                        "clientData" => {
                            let element = crate::drawing::charts::OpaqueElement::read(
                                reader, child, empty, &scope,
                            )
                            .map_err(crate::xml::xml_error)?;
                            self.client_data =
                                Some(Box::new(super::DrawingClientData::read(&element)?));
                        }
                        "ext" => {
                            self.extent.set_attributes(reader, child)?;
                            if !empty {
                                crate::reader::driver::consume_empty_element(reader, child)?;
                            }
                        }
                        _ => return Err(crate::xml::xml_error("unsupported drawing anchor child")),
                    }
                }
                Event::End(child) if child.name().as_ref() == end.as_slice() => {
                    if objects != 1
                        || !(seen.contains("from")
                            && seen.contains("ext")
                            && seen.contains("clientData"))
                    {
                        return Err(crate::xml::xml_error(
                            "missing required drawing anchor child",
                        ));
                    }
                    return Ok(());
                }
                Event::Eof => return Err(crate::xml::xml_error("unterminated drawing anchor")),
                _ => (),
            }
            buffer.clear();
        }
    }

    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        rel_list: &mut Vec<(String, String)>,
    ) -> Result<(), crate::XlsxError> {
        // xdr:oneCellAnchor
        write_start_tag(writer, "xdr:oneCellAnchor", vec![], false);

        // xdr:from
        self.from_marker.write_to_from(writer);

        // xdr:ext
        self.extent.write_to(writer);

        // xdr:grpSp
        if let Some(v) = &self.group_shape {
            v.write_to(writer, rel_list, 0)?;
        }

        // xdr:graphicFrame
        if let Some(v) = &self.graphic_frame {
            v.write_to(writer, rel_list, 0)?;
        }

        // xdr:sp
        if let Some(v) = &self.shape {
            v.write_to(writer, rel_list, 0)?;
        }

        // xdr:cxnSp
        if let Some(value) = &self.connection_shape {
            value.write_to(writer, rel_list, 0)?;
        }

        // xdr:pic
        if let Some(v) = &self.picture {
            v.write_to(writer, rel_list, 0)?;
        }

        // xdr:clientData
        if let Some(value) = &self.client_data {
            value.write_to(writer);
        } else {
            write_start_tag(writer, "xdr:clientData", vec![], true);
        }

        write_end_tag(writer, "xdr:oneCellAnchor");
        Ok(())
    }
}
impl AdjustmentCoordinate for OneCellAnchor {
    #[inline]
    fn adjustment_insert_coordinate(
        &mut self,
        root_col_num: u32,
        offset_col_num: u32,
        root_row_num: u32,
        offset_row_num: u32,
    ) {
        self.from_marker.adjustment_insert_coordinate(
            root_col_num,
            offset_col_num,
            root_row_num,
            offset_row_num,
        );
    }

    #[inline]
    fn adjustment_remove_coordinate(
        &mut self,
        root_col_num: u32,
        offset_col_num: u32,
        root_row_num: u32,
        offset_row_num: u32,
    ) {
        self.from_marker.adjustment_remove_coordinate(
            root_col_num,
            offset_col_num,
            root_row_num,
            offset_row_num,
        );
    }

    #[inline]
    fn is_remove_coordinate(
        &self,
        root_col_num: u32,
        offset_col_num: u32,
        root_row_num: u32,
        offset_row_num: u32,
    ) -> bool {
        self.from_marker.is_remove_coordinate(
            root_col_num,
            offset_col_num,
            root_row_num,
            offset_row_num,
        )
    }
}

impl OneCellAnchor {
    pub(crate) fn visit_picture_blips(&self, visit: &mut dyn FnMut(&crate::drawing::Blip)) {
        if let Some(picture) = self.picture() {
            visit(picture.blip_fill().blip());
        }
        if let Some(group) = self.group_shape() {
            for child in group.content() {
                child.visit_picture_blips(visit);
            }
        }
    }

    pub(crate) fn visit_picture_blips_mut(
        &mut self,
        visit: &mut dyn FnMut(&mut crate::drawing::Blip),
    ) {
        if let Some(picture) = self.picture_mut() {
            visit(picture.blip_fill_mut().blip_mut());
        }
        if let Some(group) = self.group_shape_mut() {
            for child in group.content_mut() {
                child.visit_picture_blips_mut(visit);
            }
        }
    }
}

impl OneCellAnchor {
    pub fn visit_standard_frames(
        &self,
        visit: &mut dyn FnMut(super::DrawingFrameKey, &GraphicFrame),
    ) {
        if let Some(frame) = self.graphic_frame() {
            let id = frame
                .non_visual_graphic_frame_properties()
                .non_visual_drawing_properties()
                .id();
            visit(super::DrawingFrameKey::primary(id), frame);
        }
        if let Some(group) = self.group_shape() {
            for child in group.content() {
                child.visit_standard_frames(&super::DrawingFrameKey::primary(child.id()), visit);
            }
        }
    }

    pub fn visit_extended_frames(
        &self,
        visit: &mut dyn FnMut(super::DrawingFrameKey, &super::ExtendedChartFrame),
    ) {
        if let Some(group) = self.group_shape() {
            for child in group.content() {
                child.visit_extended_frames(&super::DrawingFrameKey::primary(child.id()), visit);
            }
        }
    }

    pub fn visit_standard_frames_mut(
        &mut self,
        visit: &mut dyn FnMut(super::DrawingFrameKey, &mut GraphicFrame),
    ) {
        if let Some(frame) = self.graphic_frame_mut() {
            let id = frame
                .non_visual_graphic_frame_properties()
                .non_visual_drawing_properties()
                .id();
            visit(super::DrawingFrameKey::primary(id), frame);
        }
        if let Some(group) = self.group_shape_mut() {
            for child in group.content_mut() {
                child
                    .visit_standard_frames_mut(&super::DrawingFrameKey::primary(child.id()), visit);
            }
        }
    }

    pub fn visit_extended_frames_mut(
        &mut self,
        visit: &mut dyn FnMut(super::DrawingFrameKey, &mut super::ExtendedChartFrame),
    ) {
        if let Some(group) = self.group_shape_mut() {
            for child in group.content_mut() {
                child
                    .visit_extended_frames_mut(&super::DrawingFrameKey::primary(child.id()), visit);
            }
        }
    }
}

impl OneCellAnchor {
    pub(crate) fn visit_non_visual_relationships(
        &self,
        visit: &mut dyn FnMut(super::DrawingRelationshipKind, &str),
    ) {
        if let Some(value) = self.graphic_frame() {
            value
                .non_visual_graphic_frame_properties()
                .visit_relationships(visit);
        }
        if let Some(value) = self.shape() {
            value
                .non_visual_shape_properties()
                .visit_relationships(visit);
        }
        if let Some(value) = self.connection_shape() {
            value
                .non_visual_connection_shape_properties()
                .visit_relationships(visit);
        }
        if let Some(value) = self.picture() {
            value
                .non_visual_picture_properties()
                .visit_relationships(visit);
        }
        if let Some(value) = self.group_shape() {
            value
                .non_visual_group_shape_properties()
                .visit_relationships(visit);
            for child in value.content() {
                child.visit_non_visual_relationships(visit);
            }
        }
    }

    pub(crate) fn visit_non_visual_relationships_mut(
        &mut self,
        visit: &mut dyn FnMut(super::DrawingRelationshipKind, &mut String),
    ) {
        if let Some(value) = self.graphic_frame_mut() {
            value
                .non_visual_graphic_frame_properties_mut()
                .visit_relationships_mut(visit);
        }
        if let Some(value) = self.shape_mut() {
            value
                .non_visual_shape_properties_mut()
                .visit_relationships_mut(visit);
        }
        if let Some(value) = self.connection_shape_mut() {
            value
                .non_visual_connection_shape_properties_mut()
                .visit_relationships_mut(visit);
        }
        if let Some(value) = self.picture_mut() {
            value
                .non_visual_picture_properties_mut()
                .visit_relationships_mut(visit);
        }
        if let Some(value) = self.group_shape_mut() {
            value
                .non_visual_group_shape_properties_mut()
                .visit_relationships_mut(visit);
            for child in value.content_mut() {
                child.visit_non_visual_relationships_mut(visit);
            }
        }
    }
}
