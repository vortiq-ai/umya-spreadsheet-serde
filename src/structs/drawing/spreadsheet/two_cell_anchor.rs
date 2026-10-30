// xdr:twoCellAnchor
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
    super::super::EnumValue,
    ConnectionShape,
    EditAsValues,
    GraphicFrame,
    GroupShape,
    MarkerType,
    Picture,
    Shape,
};
use crate::{
    helper::const_str::{
        DRAWING_MAIN_NS,
        MC_NS,
    },
    structs::{
        BooleanValue,
        raw::RawRelationships,
    },
    traits::{
        AdjustmentCoordinate,
        AdjustmentCoordinateWithSheet,
    },
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct TwoCellAnchor {
    edit_as:              EnumValue<EditAsValues>,
    from_marker:          MarkerType,
    to_marker:            MarkerType,
    group_shape:          Option<Box<GroupShape>>,
    graphic_frame:        Option<Box<GraphicFrame>>,
    shape:                Option<Box<Shape>>,
    connection_shape:     Option<Box<ConnectionShape>>,
    picture:              Option<Box<Picture>>,
    is_alternate_content: BooleanValue,
    #[cfg_attr(feature = "serde", serde(default))]
    client_data:          Option<Box<super::DrawingClientData>>,
}

impl TwoCellAnchor {
    #[inline]
    #[must_use]
    pub fn edit_as(&self) -> &EditAsValues {
        self.edit_as.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use edit_as()")]
    pub fn get_edit_as(&self) -> &EditAsValues {
        self.edit_as()
    }

    #[inline]
    pub fn set_edit_as(&mut self, value: EditAsValues) -> &mut Self {
        self.edit_as.set_value(value);
        self
    }

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
    pub fn set_from_marker(&mut self, value: MarkerType) -> &mut Self {
        self.from_marker = value;
        self
    }

    #[inline]
    #[must_use]
    pub fn to_marker(&self) -> &MarkerType {
        &self.to_marker
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use to_marker()")]
    pub fn get_to_marker(&self) -> &MarkerType {
        self.to_marker()
    }

    #[inline]
    pub fn to_marker_mut(&mut self) -> &mut MarkerType {
        &mut self.to_marker
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use to_marker_mut()")]
    pub fn get_to_marker_mut(&mut self) -> &mut MarkerType {
        self.to_marker_mut()
    }

    #[inline]
    pub fn set_to_marker(&mut self, value: MarkerType) -> &mut Self {
        self.to_marker = value;
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
    pub fn graphic_frame(&self) -> Option<&GraphicFrame> {
        self.graphic_frame.as_deref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use graphic_frame()")]
    pub fn get_graphic_frame(&self) -> Option<&GraphicFrame> {
        self.graphic_frame()
    }

    #[inline]
    pub fn graphic_frame_mut(&mut self) -> Option<&mut GraphicFrame> {
        self.graphic_frame.as_deref_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use graphic_frame_mut()")]
    pub fn get_graphic_frame_mut(&mut self) -> Option<&mut GraphicFrame> {
        self.graphic_frame_mut()
    }

    #[inline]
    pub fn set_graphic_frame(&mut self, value: GraphicFrame) -> &mut Self {
        self.graphic_frame = Some(Box::new(value));
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
    pub fn set_shape(&mut self, value: Shape) -> &mut Self {
        self.shape = Some(Box::new(value));
        self
    }

    #[inline]
    #[must_use]
    pub fn connection_shape(&self) -> Option<&ConnectionShape> {
        self.connection_shape.as_deref()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use connection_shape()")]
    pub fn get_connection_shape(&self) -> Option<&ConnectionShape> {
        self.connection_shape()
    }

    #[inline]
    pub fn connection_shape_mut(&mut self) -> Option<&mut ConnectionShape> {
        self.connection_shape.as_deref_mut()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use connection_shape_mut()")]
    pub fn get_connection_shape_mut(&mut self) -> Option<&mut ConnectionShape> {
        self.connection_shape_mut()
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
    pub fn is_alternate_content(&self) -> bool {
        self.is_alternate_content.value()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use is_alternate_content()")]
    pub fn get_is_alternate_content(&self) -> bool {
        self.is_alternate_content()
    }

    #[inline]
    pub fn set_is_alternate_content(&mut self, value: bool) -> &mut Self {
        self.is_alternate_content.set_value(value);
        self
    }

    #[inline]
    pub(crate) fn is_support(&self) -> bool {
        match self.graphic_frame.as_ref() {
            Some(v) => v
                .graphic()
                .graphic_data()
                .chart_space()
                .chart()
                .plot_area()
                .is_support(),
            None => true,
        }
    }

    #[inline]
    pub(crate) fn is_chart(&self) -> bool {
        self.graphic_frame.is_some()
    }

    #[inline]
    pub(crate) fn is_image(&self) -> bool {
        self.picture.is_some() || self.group_shape.is_some()
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
        if let Some(value) = crate::reader::driver::read_enum_attribute(owner, b"editAs")? {
            self.edit_as.set_value(value);
        }

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
                        "clientData" => {
                            let element = crate::drawing::charts::OpaqueElement::read(
                                reader, child, empty, &scope,
                            )
                            .map_err(crate::xml::xml_error)?;
                            self.client_data =
                                Some(Box::new(super::DrawingClientData::read(&element)?));
                        }
                        "to" => {
                            if empty {
                                return Err(crate::xml::xml_error("empty drawing marker"));
                            }
                            self.to_marker.set_attributes(reader, child)?;
                        }
                        "cxnSp" => {
                            if empty {
                                return Err(crate::xml::xml_error("empty drawing connector"));
                            }
                            let mut value = ConnectionShape::default();
                            value.set_attributes(reader, child, drawing_relationships, session)?;
                            self.set_connection_shape(value);
                        }
                        _ => return Err(crate::xml::xml_error("unsupported drawing anchor child")),
                    }
                }
                Event::End(child) if child.name().as_ref() == end.as_slice() => {
                    if objects != 1
                        || !(seen.contains("from")
                            && seen.contains("to")
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
        ole_id: usize,
    ) -> Result<(), crate::XlsxError> {
        if self.is_alternate_content() {
            // mc:AlternateContent
            write_start_tag(
                writer,
                "mc:AlternateContent",
                vec![("xmlns:mc", MC_NS).into()],
                false,
            );

            // mc:Choice
            write_start_tag(
                writer,
                "mc:Choice",
                vec![
                    ("xmlns:a14", DRAWING_MAIN_NS).into(),
                    ("Requires", "a14").into(),
                ],
                false,
            );
        }

        // xdr:twoCellAnchor
        let mut attributes: crate::structs::AttrCollection = Vec::new();
        if self.edit_as.has_value() {
            attributes.push(("editAs", self.edit_as.value_string()).into());
        }
        write_start_tag(writer, "xdr:twoCellAnchor", attributes, false);

        // xdr:from
        self.from_marker.write_to_from(writer);

        // xdr:to
        self.to_marker.write_to_to(writer);

        // xdr:grpSp
        if let Some(v) = &self.group_shape {
            v.write_to(writer, rel_list, ole_id)?;
        }

        // xdr:graphicFrame
        if let Some(v) = &self.graphic_frame {
            v.write_to(writer, rel_list, ole_id)?;
        }

        // xdr:sp
        if let Some(v) = &self.shape {
            v.write_to(writer, rel_list, ole_id)?;
        }

        // xdr:cxnSp
        if let Some(v) = &self.connection_shape {
            v.write_to(writer, rel_list, ole_id)?;
        }

        // xdr:pic
        if let Some(v) = &self.picture {
            v.write_to(writer, rel_list, ole_id)?;
        }

        // xdr:clientData
        if let Some(value) = &self.client_data {
            value.write_to(writer);
        } else {
            write_start_tag(writer, "xdr:clientData", vec![], true);
        }

        write_end_tag(writer, "xdr:twoCellAnchor");

        if self.is_alternate_content() {
            write_end_tag(writer, "mc:Choice");

            // mc:Fallback
            write_start_tag(writer, "mc:Fallback", vec![], true);

            write_end_tag(writer, "mc:AlternateContent");
        }
        Ok(())
    }
}
impl AdjustmentCoordinate for TwoCellAnchor {
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
        self.to_marker.adjustment_insert_coordinate(
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
        self.to_marker.adjustment_remove_coordinate(
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
        ) || self.to_marker.is_remove_coordinate(
            root_col_num,
            offset_col_num,
            root_row_num,
            offset_row_num,
        )
    }
}
impl AdjustmentCoordinateWithSheet for TwoCellAnchor {
    #[inline]
    fn adjustment_insert_coordinate_with_sheet(
        &mut self,
        sheet_name: &str,
        root_col_num: u32,
        offset_col_num: u32,
        root_row_num: u32,
        offset_row_num: u32,
    ) {
        if let Some(v) = &mut self.graphic_frame {
            v.adjustment_insert_coordinate_with_sheet(
                sheet_name,
                root_col_num,
                offset_col_num,
                root_row_num,
                offset_row_num,
            );
        }
    }

    #[inline]
    fn adjustment_remove_coordinate_with_sheet(
        &mut self,
        sheet_name: &str,
        root_col_num: u32,
        offset_col_num: u32,
        root_row_num: u32,
        offset_row_num: u32,
    ) {
        if let Some(v) = &mut self.graphic_frame {
            v.adjustment_remove_coordinate_with_sheet(
                sheet_name,
                root_col_num,
                offset_col_num,
                root_row_num,
                offset_row_num,
            );
        }
    }
}

impl TwoCellAnchor {
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

impl TwoCellAnchor {
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

impl TwoCellAnchor {
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
        if let Some(value) = self.picture() {
            value
                .non_visual_picture_properties()
                .visit_relationships(visit);
        }
        if let Some(value) = self.connection_shape() {
            value
                .non_visual_connection_shape_properties()
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
        if let Some(value) = self.picture_mut() {
            value
                .non_visual_picture_properties_mut()
                .visit_relationships_mut(visit);
        }
        if let Some(value) = self.connection_shape_mut() {
            value
                .non_visual_connection_shape_properties_mut()
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
