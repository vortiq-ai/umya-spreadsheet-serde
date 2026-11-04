use std::io::Cursor;

use quick_xml::Writer;

use super::{
    ConnectionShape,
    DrawingAnchor,
    DrawingClientData,
    ExtendedChartFrame,
    Extent,
    GraphicFrame,
    GroupShape,
    Picture,
    Shape,
    extended_chart::{
        drawing_error,
        geometry,
        with_fragment,
        write_anchor_start,
    },
};
use crate::{
    drawing::{
        Point2DType,
        charts::OpaqueElement,
    },
    helper::const_str::SHEET_DRAWING_NS,
    structs::{
        XlsxError,
        raw::RawRelationships,
    },
    traits::AdjustmentCoordinateWithSheet,
    writer::driver::write_end_tag,
    xml::direct_children,
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub enum DrawingObject {
    GraphicFrame(Box<GraphicFrame>),
    ExtendedFrame(Box<ExtendedChartFrame>),
    Group(Box<GroupShape>),
    Shape(Box<Shape>),
    Connector(Box<ConnectionShape>),
    Picture(Box<Picture>),
}
impl DrawingObject {
    #[must_use]
    pub fn id(&self) -> u32 {
        match self {
            Self::GraphicFrame(value) => value
                .non_visual_graphic_frame_properties()
                .non_visual_drawing_properties()
                .id(),
            Self::ExtendedFrame(value) => value.frame_id(),
            Self::Group(value) => value
                .non_visual_group_shape_properties()
                .non_visual_drawing_properties()
                .id(),
            Self::Shape(value) => value
                .non_visual_shape_properties()
                .non_visual_drawing_properties()
                .id(),
            Self::Connector(value) => value
                .non_visual_connection_shape_properties()
                .non_visual_drawing_properties()
                .id(),
            Self::Picture(value) => value
                .non_visual_picture_properties()
                .non_visual_drawing_properties()
                .id(),
        }
    }

    pub(crate) fn read(
        element: &OpaqueElement,
        relationships: Option<&RawRelationships>,
        session: &crate::reader::xlsx::SourceCacheSession,
        provider: &mut dyn crate::reader::xlsx::ChartPartProvider,
    ) -> Result<Self, XlsxError> {
        if element.name.namespace_uri != SHEET_DRAWING_NS {
            return Err(drawing_error("foreign drawing object namespace"));
        }
        if element.name.local_name == "graphicFrame" {
            if let Some(frame) =
                ExtendedChartFrame::read(element, relationships, session, provider)?
            {
                return Ok(Self::ExtendedFrame(Box::new(frame)));
            }
        }
        with_fragment(element, |reader, start, empty| {
            Self::read_ordinary(
                reader,
                start,
                empty,
                &element.name.local_name,
                relationships,
                session,
                provider,
            )
        })
    }

    pub(crate) fn read_ordinary<R: std::io::BufRead>(
        reader: &mut quick_xml::Reader<R>,
        owner: &quick_xml::events::BytesStart<'_>,
        empty: bool,
        local_name: &str,
        relationships: Option<&RawRelationships>,
        session: &crate::reader::xlsx::SourceCacheSession,
        provider: &mut dyn crate::reader::xlsx::ChartPartProvider,
    ) -> Result<Self, XlsxError> {
        if empty {
            return Err(drawing_error("empty drawing object"));
        }
        Ok(match local_name {
            "graphicFrame" => {
                let mut value = GraphicFrame::default();
                value.set_attributes(reader, owner, relationships, session, provider)?;
                Self::GraphicFrame(Box::new(value))
            }
            "grpSp" => {
                let mut value = GroupShape::default();
                value.set_attributes(reader, owner, relationships, session, provider)?;
                Self::Group(Box::new(value))
            }
            "sp" => {
                let mut value = Shape::default();
                value.set_attributes(reader, owner, relationships, session)?;
                Self::Shape(Box::new(value))
            }
            "cxnSp" => {
                let mut value = ConnectionShape::default();
                value.set_attributes(reader, owner, relationships, session)?;
                Self::Connector(Box::new(value))
            }
            "pic" => {
                let mut value = Picture::default();
                value.set_attributes(reader, owner, relationships, session)?;
                Self::Picture(Box::new(value))
            }
            _ => return Err(drawing_error("unsupported drawing object")),
        })
    }

    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        rels: &mut Vec<(String, String)>,
    ) -> Result<(), XlsxError> {
        let begin = rels.len();
        match self {
            Self::GraphicFrame(value) => value.write_to(writer, rels, 0)?,
            Self::ExtendedFrame(value) => value.write_to(writer, rels)?,
            Self::Group(value) => value.write_to(writer, rels, 0)?,
            Self::Shape(value) => value.write_to(writer, rels, 0)?,
            Self::Connector(value) => value.write_to(writer, rels, 0)?,
            Self::Picture(value) => value.write_to(writer, rels, 0)?,
        }
        if matches!(self, Self::GraphicFrame(_)) {
            for (kind, identity) in &mut rels[begin..] {
                if kind == "CHART" {
                    *identity = self.id().to_string();
                }
            }
        }
        Ok(())
    }

    pub(crate) fn adjust_with_sheet(
        &mut self,
        name: &str,
        col: u32,
        col_offset: u32,
        row: u32,
        row_offset: u32,
        insert: bool,
    ) {
        if let Self::Group(group) = self {
            for child in group.content_mut() {
                child.adjust_with_sheet(name, col, col_offset, row, row_offset, insert);
            }
        }
        if let Self::GraphicFrame(frame) = self {
            if insert {
                frame.adjustment_insert_coordinate_with_sheet(
                    name, col, col_offset, row, row_offset,
                );
            } else {
                frame.adjustment_remove_coordinate_with_sheet(
                    name, col, col_offset, row, row_offset,
                );
            }
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct AbsoluteAnchor {
    position:    Point2DType,
    extent:      Extent,
    content:     Option<DrawingObject>,
    client_data: DrawingClientData,
}
impl AbsoluteAnchor {
    #[must_use]
    pub fn new(position: Point2DType, extent: Extent, content: DrawingObject) -> Self {
        Self {
            position,
            extent,
            content: Some(content),
            ..Self::default()
        }
    }

    #[must_use]
    pub fn position(&self) -> &Point2DType {
        &self.position
    }

    pub fn position_mut(&mut self) -> &mut Point2DType {
        &mut self.position
    }

    #[must_use]
    pub fn extent(&self) -> &Extent {
        &self.extent
    }

    pub fn extent_mut(&mut self) -> &mut Extent {
        &mut self.extent
    }

    #[must_use]
    pub fn content(&self) -> Option<&DrawingObject> {
        self.content.as_ref()
    }

    pub fn content_mut(&mut self) -> Option<&mut DrawingObject> {
        self.content.as_mut()
    }

    pub fn set_content(&mut self, content: DrawingObject) -> &mut Self {
        self.content = Some(content);
        self
    }

    #[must_use]
    pub fn client_data(&self) -> &DrawingClientData {
        &self.client_data
    }

    pub fn client_data_mut(&mut self) -> &mut DrawingClientData {
        &mut self.client_data
    }

    pub(crate) fn from_element(
        element: &OpaqueElement,
        relationships: Option<&RawRelationships>,
        session: &crate::reader::xlsx::SourceCacheSession,
        provider: &mut dyn crate::reader::xlsx::ChartPartProvider,
    ) -> Result<Self, XlsxError> {
        let (anchor, client_data) = geometry(element)?;
        let DrawingAnchor::Absolute { position, extent } = anchor else {
            return Err(drawing_error("absolute anchor required"));
        };
        let mut content = None;
        for child in direct_children(element).map_err(drawing_error)? {
            if child.name.is(SHEET_DRAWING_NS, "pos")
                || child.name.is(SHEET_DRAWING_NS, "ext")
                || child.name.is(SHEET_DRAWING_NS, "clientData")
            {
                continue;
            }
            if content.is_some() {
                return Err(drawing_error("multiple absolute-anchor objects"));
            }
            content = Some(DrawingObject::read(
                &child,
                relationships,
                session,
                provider,
            )?);
        }
        if content.is_none() {
            return Err(drawing_error("absolute anchor requires an object"));
        }
        Ok(Self {
            position,
            extent,
            content,
            client_data,
        })
    }

    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        rels: &mut Vec<(String, String)>,
    ) -> Result<(), XlsxError> {
        let anchor = DrawingAnchor::Absolute {
            position: self.position.clone(),
            extent:   self.extent.clone(),
        };
        let end = write_anchor_start(&anchor, writer);
        if let Some(content) = &self.content {
            content.write_to(writer, rels)?;
        }
        self.client_data.write_to(writer);
        write_end_tag(writer, end);
        Ok(())
    }
}

impl DrawingObject {
    pub(crate) fn visit_standard_frames(
        &self,
        key: &super::DrawingFrameKey,
        visit: &mut dyn FnMut(super::DrawingFrameKey, &GraphicFrame),
    ) {
        match self {
            Self::GraphicFrame(value) => visit(
                key.for_frame(
                    value
                        .non_visual_graphic_frame_properties()
                        .non_visual_drawing_properties()
                        .id(),
                ),
                value,
            ),
            Self::Group(value) => {
                for child in value.content() {
                    child.visit_standard_frames(&key.for_frame(child.id()), visit);
                }
            }
            _ => (),
        }
    }

    pub(crate) fn visit_standard_frames_mut(
        &mut self,
        key: &super::DrawingFrameKey,
        visit: &mut dyn FnMut(super::DrawingFrameKey, &mut GraphicFrame),
    ) {
        match self {
            Self::GraphicFrame(value) => visit(
                key.for_frame(
                    value
                        .non_visual_graphic_frame_properties()
                        .non_visual_drawing_properties()
                        .id(),
                ),
                value,
            ),
            Self::Group(value) => {
                for child in value.content_mut() {
                    child.visit_standard_frames_mut(&key.for_frame(child.id()), visit);
                }
            }
            _ => (),
        }
    }

    pub(crate) fn visit_extended_frames(
        &self,
        key: &super::DrawingFrameKey,
        visit: &mut dyn FnMut(super::DrawingFrameKey, &ExtendedChartFrame),
    ) {
        match self {
            Self::ExtendedFrame(value) => visit(key.for_frame(value.frame_id()), value),
            Self::Group(value) => {
                for child in value.content() {
                    child.visit_extended_frames(&key.for_frame(child.id()), visit);
                }
            }
            _ => (),
        }
    }

    pub(crate) fn visit_extended_frames_mut(
        &mut self,
        key: &super::DrawingFrameKey,
        visit: &mut dyn FnMut(super::DrawingFrameKey, &mut ExtendedChartFrame),
    ) {
        match self {
            Self::ExtendedFrame(value) => visit(key.for_frame(value.frame_id()), value),
            Self::Group(value) => {
                for child in value.content_mut() {
                    child.visit_extended_frames_mut(&key.for_frame(child.id()), visit);
                }
            }
            _ => (),
        }
    }
}

pub(crate) fn group_properties(
    value: &GroupShape,
    visit: &mut dyn FnMut(&super::NonVisualDrawingProperties),
) {
    visit(
        value
            .non_visual_group_shape_properties()
            .non_visual_drawing_properties(),
    );
    for child in value.content() {
        child.visit_properties(visit);
    }
}
pub(crate) fn group_properties_mut(
    value: &mut GroupShape,
    visit: &mut dyn FnMut(&mut super::NonVisualDrawingProperties),
) {
    visit(
        value
            .non_visual_group_shape_properties_mut()
            .non_visual_drawing_properties_mut(),
    );
    for child in value.content_mut() {
        child.visit_properties_mut(visit);
    }
}
impl DrawingObject {
    pub(crate) fn visit_properties(
        &self,
        visit: &mut dyn FnMut(&super::NonVisualDrawingProperties),
    ) {
        match self {
            Self::GraphicFrame(value) => visit(
                value
                    .non_visual_graphic_frame_properties()
                    .non_visual_drawing_properties(),
            ),
            Self::ExtendedFrame(value) => {
                visit(value.frame_properties().non_visual_drawing_properties());
            }
            Self::Group(value) => group_properties(value, visit),
            Self::Shape(value) => visit(
                value
                    .non_visual_shape_properties()
                    .non_visual_drawing_properties(),
            ),
            Self::Connector(value) => visit(
                value
                    .non_visual_connection_shape_properties()
                    .non_visual_drawing_properties(),
            ),
            Self::Picture(value) => visit(
                value
                    .non_visual_picture_properties()
                    .non_visual_drawing_properties(),
            ),
        }
    }

    pub(crate) fn visit_properties_mut(
        &mut self,
        visit: &mut dyn FnMut(&mut super::NonVisualDrawingProperties),
    ) {
        match self {
            Self::GraphicFrame(value) => visit(
                value
                    .non_visual_graphic_frame_properties_mut()
                    .non_visual_drawing_properties_mut(),
            ),
            Self::ExtendedFrame(value) => visit(
                value
                    .frame_properties_mut()
                    .non_visual_drawing_properties_mut(),
            ),
            Self::Group(value) => group_properties_mut(value, visit),
            Self::Shape(value) => visit(
                value
                    .non_visual_shape_properties_mut()
                    .non_visual_drawing_properties_mut(),
            ),
            Self::Connector(value) => visit(
                value
                    .non_visual_connection_shape_properties_mut()
                    .non_visual_drawing_properties_mut(),
            ),
            Self::Picture(value) => visit(
                value
                    .non_visual_picture_properties_mut()
                    .non_visual_drawing_properties_mut(),
            ),
        }
    }

    pub fn visit_relationships(&self, visit: &mut dyn FnMut(super::DrawingRelationshipKind, &str)) {
        self.visit_non_visual_relationships(visit);
        self.visit_shape_properties(&mut |value| value.visit_relationships(visit));
        self.visit_picture_blips(&mut |value| value.visit_relationships(visit));
    }

    pub fn visit_relationships_mut(
        &mut self,
        visit: &mut dyn FnMut(super::DrawingRelationshipKind, &mut String),
    ) {
        self.visit_non_visual_relationships_mut(visit);
        self.visit_shape_properties_mut(&mut |value| value.visit_relationships_mut(visit));
        self.visit_picture_blips_mut(&mut |value| value.visit_relationships_mut(visit));
    }

    pub(crate) fn media_objects(&self) -> Vec<&crate::structs::MediaObject> {
        let mut values = Vec::new();
        match self {
            Self::Picture(value) => values.push(value.blip_fill().blip().image()),
            Self::Shape(value) => {
                if let Some(fill) = value.shape_properties().blip_fill() {
                    values.push(fill.blip().image());
                }
            }
            Self::Connector(value) => {
                if let Some(fill) = value.shape_properties().blip_fill() {
                    values.push(fill.blip().image());
                }
            }
            Self::Group(value) => {
                for child in value.content() {
                    values.extend(child.media_objects());
                }
            }
            _ => (),
        }
        values
    }
}
impl AbsoluteAnchor {
    pub fn visit_relationships(&self, visit: &mut dyn FnMut(super::DrawingRelationshipKind, &str)) {
        if let Some(value) = &self.content {
            value.visit_relationships(visit);
        }
    }

    pub fn visit_relationships_mut(
        &mut self,
        visit: &mut dyn FnMut(super::DrawingRelationshipKind, &mut String),
    ) {
        if let Some(value) = &mut self.content {
            value.visit_relationships_mut(visit);
        }
    }
}

impl DrawingObject {
    pub(crate) fn visit_shape_properties(&self, visit: &mut dyn FnMut(&super::ShapeProperties)) {
        match self {
            Self::Shape(value) => visit(value.shape_properties()),
            Self::Connector(value) => visit(value.shape_properties()),
            Self::Picture(value) => visit(value.shape_properties()),
            Self::Group(value) => {
                for child in value.content() {
                    child.visit_shape_properties(visit);
                }
            }
            _ => (),
        }
    }

    pub(crate) fn visit_shape_properties_mut(
        &mut self,
        visit: &mut dyn FnMut(&mut super::ShapeProperties),
    ) {
        match self {
            Self::Shape(value) => visit(value.shape_properties_mut()),
            Self::Connector(value) => visit(value.shape_properties_mut()),
            Self::Picture(value) => visit(value.shape_properties_mut()),
            Self::Group(value) => {
                for child in value.content_mut() {
                    child.visit_shape_properties_mut(visit);
                }
            }
            _ => (),
        }
    }
}

impl DrawingObject {
    pub(crate) fn visit_picture_blips(&self, visit: &mut dyn FnMut(&crate::drawing::Blip)) {
        match self {
            Self::Picture(picture) => visit(picture.blip_fill().blip()),
            Self::Group(value) => {
                for child in value.content() {
                    child.visit_picture_blips(visit);
                }
            }
            _ => (),
        }
    }

    pub(crate) fn visit_picture_blips_mut(
        &mut self,
        visit: &mut dyn FnMut(&mut crate::drawing::Blip),
    ) {
        match self {
            Self::Picture(picture) => visit(picture.blip_fill_mut().blip_mut()),
            Self::Group(value) => {
                for child in value.content_mut() {
                    child.visit_picture_blips_mut(visit);
                }
            }
            _ => (),
        }
    }
}

impl DrawingObject {
    pub(crate) fn visit_non_visual_relationships(
        &self,
        visit: &mut dyn FnMut(super::DrawingRelationshipKind, &str),
    ) {
        match self {
            Self::GraphicFrame(value) => value
                .non_visual_graphic_frame_properties()
                .visit_relationships(visit),
            Self::ExtendedFrame(value) => value.frame_properties().visit_relationships(visit),
            Self::Shape(value) => value
                .non_visual_shape_properties()
                .visit_relationships(visit),
            Self::Connector(value) => value
                .non_visual_connection_shape_properties()
                .visit_relationships(visit),
            Self::Picture(value) => value
                .non_visual_picture_properties()
                .visit_relationships(visit),
            Self::Group(value) => {
                value
                    .non_visual_group_shape_properties()
                    .visit_relationships(visit);
                for child in value.content() {
                    child.visit_non_visual_relationships(visit);
                }
            }
        }
    }

    pub(crate) fn visit_non_visual_relationships_mut(
        &mut self,
        visit: &mut dyn FnMut(super::DrawingRelationshipKind, &mut String),
    ) {
        match self {
            Self::GraphicFrame(value) => value
                .non_visual_graphic_frame_properties_mut()
                .visit_relationships_mut(visit),
            Self::ExtendedFrame(value) => {
                value.frame_properties_mut().visit_relationships_mut(visit);
            }
            Self::Shape(value) => value
                .non_visual_shape_properties_mut()
                .visit_relationships_mut(visit),
            Self::Connector(value) => value
                .non_visual_connection_shape_properties_mut()
                .visit_relationships_mut(visit),
            Self::Picture(value) => value
                .non_visual_picture_properties_mut()
                .visit_relationships_mut(visit),
            Self::Group(value) => {
                value
                    .non_visual_group_shape_properties_mut()
                    .visit_relationships_mut(visit);
                for child in value.content_mut() {
                    child.visit_non_visual_relationships_mut(visit);
                }
            }
        }
    }
}
