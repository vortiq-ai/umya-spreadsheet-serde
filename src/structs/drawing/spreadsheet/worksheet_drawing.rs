// xdr:wsDr
use std::{
    collections::HashSet,
    io::Cursor,
};

#[cfg(test)]
use quick_xml::events::Event;
use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use super::{
    AbsoluteAnchor,
    ConnectionShape,
    ExtendedChart,
    GraphicFrame,
    NonVisualDrawingProperties,
    OneCellAnchor,
    Picture,
    Shape,
    TwoCellAnchor,
};
use crate::{
    drawing::charts::{
        NamespaceScope,
        OpaqueElement,
    },
    helper::const_str::{
        DRAWINGML_MAIN_NS,
        SHEET_DRAWING_NS,
    },
    structs::{
        Chart,
        Image,
        OleObjects,
        XlsxError,
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
    xml::direct_children,
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct WorksheetDrawing {
    image_collection:           Vec<Image>,
    chart_collection:           Vec<Chart>,
    one_cell_anchor_collection: Vec<OneCellAnchor>,
    two_cell_anchor_collection: Vec<TwoCellAnchor>,
    #[cfg_attr(feature = "serde", serde(default))]
    extended_charts:            Vec<ExtendedChart>,
    #[cfg_attr(feature = "serde", serde(default))]
    absolute_anchors:           Vec<AbsoluteAnchor>,
    #[cfg_attr(feature = "serde", serde(default))]
    object_order:               Vec<DrawingObjectKey>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum DrawingObjectKey {
    Chart(u32),
    Image(u32),
    OneCell(u32),
    TwoCell(u32),
    Absolute(u32),
    ExtendedChart(u32),
    Ole(u32),
}
impl DrawingObjectKey {
    fn id(&self) -> u32 {
        match self {
            Self::Chart(id)
            | Self::Image(id)
            | Self::OneCell(id)
            | Self::TwoCell(id)
            | Self::Absolute(id)
            | Self::ExtendedChart(id)
            | Self::Ole(id) => *id,
        }
    }
}

impl WorksheetDrawing {
    #[inline]
    #[must_use]
    pub fn image_collection(&self) -> &[Image] {
        &self.image_collection
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use image_collection()")]
    pub fn get_image_collection(&self) -> &[Image] {
        self.image_collection()
    }

    #[inline]
    pub fn image_collection_mut(&mut self) -> &mut Vec<Image> {
        &mut self.image_collection
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use image_collection_mut()")]
    pub fn get_image_collection_mut(&mut self) -> &mut Vec<Image> {
        self.image_collection_mut()
    }

    #[inline]
    pub fn add_image(&mut self, mut value: Image) -> &mut Self {
        let id = self.available_object_id();
        if let Some(anchor) = value.two_cell_anchor_mut() {
            assign_two_identity(anchor, id);
        }
        if let Some(anchor) = value.one_cell_anchor_mut() {
            assign_one_identity(anchor, id);
        }
        self.image_collection.push(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn image(&self, col: u32, row: u32) -> Option<&Image> {
        self.image_collection
            .iter()
            .find(|&image| image.col() == col - 1 && image.row() == row - 1)
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use image()")]
    pub fn get_image(&self, col: &u32, row: &u32) -> Option<&Image> {
        self.image(*col, *row)
    }

    #[inline]
    pub fn image_mut(&mut self, col: u32, row: u32) -> Option<&mut Image> {
        self.image_collection
            .iter_mut()
            .find(|image| image.col() == col - 1 && image.row() == row - 1)
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use image_mut()")]
    pub fn get_image_mut(&mut self, col: &u32, row: &u32) -> Option<&mut Image> {
        self.image_mut(*col, *row)
    }

    #[inline]
    #[must_use]
    pub fn images(&self, col: u32, row: u32) -> Vec<&Image> {
        self.image_collection
            .iter()
            .filter(|image| image.col() == col - 1 && image.row() == row - 1)
            .collect()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use images()")]
    pub fn get_images(&self, col: &u32, row: &u32) -> Vec<&Image> {
        self.images(*col, *row)
    }

    #[inline]
    pub fn images_mut(&mut self, col: u32, row: u32) -> Vec<&mut Image> {
        self.image_collection
            .iter_mut()
            .filter(|image| image.col() == col - 1 && image.row() == row - 1)
            .collect()
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use images_mut()")]
    pub fn get_images_mut(&mut self, col: &u32, row: &u32) -> Vec<&mut Image> {
        self.images_mut(*col, *row)
    }

    #[inline]
    #[must_use]
    pub fn chart_collection(&self) -> &[Chart] {
        &self.chart_collection
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use chart_collection()")]
    pub fn get_chart_collection(&self) -> &[Chart] {
        self.chart_collection()
    }

    #[inline]
    pub fn chart_collection_mut(&mut self) -> &mut Vec<Chart> {
        &mut self.chart_collection
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use chart_collection_mut()")]
    pub fn get_chart_collection_mut(&mut self) -> &mut Vec<Chart> {
        self.chart_collection_mut()
    }

    #[inline]
    pub fn add_chart_collection(&mut self, mut value: Chart) -> &mut Self {
        if let Some(frame) = value.two_cell_anchor_mut().graphic_frame_mut() {
            let properties = frame
                .non_visual_graphic_frame_properties_mut()
                .non_visual_drawing_properties_mut();
            if !properties.has_id() {
                properties.set_id(self.available_object_id());
            }
        }
        self.chart_collection.push(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn chart(&self, col: u32, row: u32) -> Option<&Chart> {
        self.chart_collection
            .iter()
            .find(|&chart| chart.col() == col - 1 && chart.row() == row - 1)
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use chart()")]
    pub fn get_chart(&self, col: &u32, row: &u32) -> Option<&Chart> {
        self.chart(*col, *row)
    }

    #[inline]
    pub fn chart_mut(&mut self, col: u32, row: u32) -> Option<&mut Chart> {
        self.chart_collection
            .iter_mut()
            .find(|chart| chart.col() == col - 1 && chart.row() == row - 1)
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use chart_mut()")]
    pub fn get_chart_mut(&mut self, col: &u32, row: &u32) -> Option<&mut Chart> {
        self.chart_mut(*col, *row)
    }

    #[must_use]
    pub fn charts(&self, col: u32, row: u32) -> Vec<&Chart> {
        self.chart_collection
            .iter()
            .filter(|chart| chart.col() == col - 1 && chart.row() == row - 1)
            .collect()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use charts()")]
    pub fn get_charts(&self, col: &u32, row: &u32) -> Vec<&Chart> {
        self.charts(*col, *row)
    }

    pub fn charts_mut(&mut self, col: u32, row: u32) -> Vec<&mut Chart> {
        self.chart_collection
            .iter_mut()
            .filter(|chart| chart.col() == col - 1 && chart.row() == row - 1)
            .collect()
    }

    #[deprecated(since = "3.0.0", note = "Use charts_mut()")]
    pub fn get_charts_mut(&mut self, col: &u32, row: &u32) -> Vec<&mut Chart> {
        self.charts_mut(*col, *row)
    }

    #[inline]
    #[must_use]
    pub fn one_cell_anchor_collection(&self) -> &[OneCellAnchor] {
        &self.one_cell_anchor_collection
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use one_cell_anchor_collection()")]
    pub fn get_one_cell_anchor_collection(&self) -> &[OneCellAnchor] {
        self.one_cell_anchor_collection()
    }

    #[inline]
    pub fn one_cell_anchor_collection_mut(&mut self) -> &mut Vec<OneCellAnchor> {
        &mut self.one_cell_anchor_collection
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use one_cell_anchor_collection_mut()")]
    pub fn get_one_cell_anchor_collection_mut(&mut self) -> &mut Vec<OneCellAnchor> {
        self.one_cell_anchor_collection_mut()
    }

    #[inline]
    pub fn add_one_cell_anchor_collection(&mut self, mut value: OneCellAnchor) -> &mut Self {
        assign_one_identity(&mut value, self.available_object_id());
        self.one_cell_anchor_collection.push(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn two_cell_anchor_collection(&self) -> &[TwoCellAnchor] {
        &self.two_cell_anchor_collection
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use two_cell_anchor_collection()")]
    pub fn get_two_cell_anchor_collection(&self) -> &[TwoCellAnchor] {
        self.two_cell_anchor_collection()
    }

    #[inline]
    pub fn two_cell_anchor_collection_mut(&mut self) -> &mut Vec<TwoCellAnchor> {
        &mut self.two_cell_anchor_collection
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use two_cell_anchor_collection_mut()")]
    pub fn get_two_cell_anchor_collection_mut(&mut self) -> &mut Vec<TwoCellAnchor> {
        self.two_cell_anchor_collection_mut()
    }

    #[inline]
    pub fn add_two_cell_anchor_collection(&mut self, mut value: TwoCellAnchor) -> &mut Self {
        assign_two_identity(&mut value, self.available_object_id());
        self.two_cell_anchor_collection.push(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn has_drawing_object(&self) -> bool {
        !self.chart_collection.is_empty()
            || !self.image_collection.is_empty()
            || !self.one_cell_anchor_collection.is_empty()
            || !self.two_cell_anchor_collection.is_empty()
            || !self.extended_charts.is_empty()
            || !self.absolute_anchors.is_empty()
    }

    #[must_use]
    pub fn graphic_frame_collection(&self) -> Vec<&GraphicFrame> {
        self.two_cell_anchor_collection
            .iter()
            .filter_map(|two_cell_anchor| two_cell_anchor.graphic_frame())
            .collect()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use graphic_frame_collection()")]
    pub fn get_graphic_frame_collection(&self) -> Vec<&GraphicFrame> {
        self.graphic_frame_collection()
    }

    pub fn graphic_frame_collection_mut(&mut self) -> Vec<&mut GraphicFrame> {
        self.two_cell_anchor_collection
            .iter_mut()
            .filter_map(|two_cell_anchor| two_cell_anchor.graphic_frame_mut())
            .collect()
    }

    #[deprecated(since = "3.0.0", note = "Use graphic_frame_collection_mut()")]
    pub fn get_graphic_frame_collection_mut(&mut self) -> Vec<&mut GraphicFrame> {
        self.graphic_frame_collection_mut()
    }

    #[must_use]
    pub fn shape_collection(&self) -> Vec<&Shape> {
        self.two_cell_anchor_collection
            .iter()
            .filter_map(|two_cell_anchor| two_cell_anchor.shape())
            .collect()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use shape_collection()")]
    pub fn get_shape_collection(&self) -> Vec<&Shape> {
        self.shape_collection()
    }

    pub fn shape_collection_mut(&mut self) -> Vec<&mut Shape> {
        self.two_cell_anchor_collection
            .iter_mut()
            .filter_map(|two_cell_anchor| two_cell_anchor.shape_mut())
            .collect()
    }

    #[deprecated(since = "3.0.0", note = "Use shape_collection_mut()")]
    pub fn get_shape_collection_mut(&mut self) -> Vec<&mut Shape> {
        self.shape_collection_mut()
    }

    #[must_use]
    pub fn connection_shape_collection(&self) -> Vec<&ConnectionShape> {
        self.two_cell_anchor_collection
            .iter()
            .filter_map(|two_cell_anchor| two_cell_anchor.connection_shape())
            .collect()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use connection_shape_collection()")]
    pub fn get_connection_shape_collection(&self) -> Vec<&ConnectionShape> {
        self.connection_shape_collection()
    }

    pub fn connection_shape_collection_mut(&mut self) -> Vec<&mut ConnectionShape> {
        self.two_cell_anchor_collection
            .iter_mut()
            .filter_map(|two_cell_anchor| two_cell_anchor.connection_shape_mut())
            .collect()
    }

    #[deprecated(since = "3.0.0", note = "Use connection_shape_collection_mut()")]
    pub fn get_connection_shape_collection_mut(&mut self) -> Vec<&mut ConnectionShape> {
        self.connection_shape_collection_mut()
    }

    #[must_use]
    pub fn picture_collection(&self) -> Vec<&Picture> {
        self.two_cell_anchor_collection
            .iter()
            .filter_map(|two_cell_anchor| two_cell_anchor.picture())
            .collect()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use picture_collection()")]
    pub fn get_picture_collection(&self) -> Vec<&Picture> {
        self.picture_collection()
    }

    pub fn one_cell_anchor_all_list(&mut self) -> Vec<&mut OneCellAnchor> {
        self.one_cell_anchor_collection
            .iter_mut()
            .chain(
                self.image_collection
                    .iter_mut()
                    .filter_map(|image| image.one_cell_anchor_mut()),
            )
            .collect()
    }

    #[deprecated(since = "3.0.0", note = "Use one_cell_anchor_all_list()")]
    pub fn get_one_cell_anchor_all_list(&mut self) -> Vec<&mut OneCellAnchor> {
        self.one_cell_anchor_all_list()
    }

    pub fn two_cell_anchor_all_list(&mut self) -> Vec<&mut TwoCellAnchor> {
        self.two_cell_anchor_collection
            .iter_mut()
            .chain(
                self.chart_collection
                    .iter_mut()
                    .map(Chart::two_cell_anchor_mut),
            )
            .chain(
                self.image_collection
                    .iter_mut()
                    .filter_map(|image| image.two_cell_anchor_mut()),
            )
            .collect()
    }

    #[deprecated(since = "3.0.0", note = "Use two_cell_anchor_all_list()")]
    pub fn get_two_cell_anchor_all_list(&mut self) -> Vec<&mut TwoCellAnchor> {
        self.two_cell_anchor_all_list()
    }

    pub fn picture_collection_mut(&mut self) -> Vec<&mut Picture> {
        self.two_cell_anchor_collection
            .iter_mut()
            .filter_map(|two_cell_anchor| two_cell_anchor.picture_mut())
            .collect()
    }

    #[deprecated(since = "3.0.0", note = "Use picture_collection_mut()")]
    pub fn get_picture_collection_mut(&mut self) -> Vec<&mut Picture> {
        self.picture_collection_mut()
    }

    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        ole_objects: &OleObjects,
        rel_list: &mut Vec<(String, String)>,
    ) -> Result<(), XlsxError> {
        self.validate_object_order()?;
        write_start_tag(
            writer,
            "xdr:wsDr",
            vec![
                ("xmlns:xdr", SHEET_DRAWING_NS).into(),
                ("xmlns:a", DRAWINGML_MAIN_NS).into(),
            ],
            false,
        );
        let mut emitted = HashSet::new();
        let shape_ids = ole_objects.resolved_shape_ids()?;
        for key in &self.object_order {
            if self.write_ordered(key, writer, ole_objects, &shape_ids, rel_list)? {
                emitted.insert(key.clone());
            }
        }
        for chart in &self.chart_collection {
            let key = two_identity(chart.two_cell_anchor()).map(DrawingObjectKey::Chart);
            if !key.as_ref().is_some_and(|key| emitted.contains(key)) {
                chart.two_cell_anchor().write_to(writer, rel_list, 0)?;
            }
        }
        for image in &self.image_collection {
            let key = image_identity(image).map(DrawingObjectKey::Image);
            if !key.as_ref().is_some_and(|key| emitted.contains(key)) {
                image.write_to(writer, rel_list)?;
            }
        }
        for value in &self.two_cell_anchor_collection {
            let key = two_identity(value).map(DrawingObjectKey::TwoCell);
            if !key.as_ref().is_some_and(|key| emitted.contains(key)) {
                value.write_to(writer, rel_list, 0)?;
            }
        }
        for value in &self.one_cell_anchor_collection {
            let key = one_identity(value).map(DrawingObjectKey::OneCell);
            if !key.as_ref().is_some_and(|key| emitted.contains(key)) {
                value.write_to(writer, rel_list)?;
            }
        }
        for value in &self.absolute_anchors {
            let key = value
                .content()
                .map(|value| DrawingObjectKey::Absolute(value.id()));
            if !key.as_ref().is_some_and(|key| emitted.contains(key)) {
                value.write_to(writer, rel_list)?;
            }
        }
        for value in &self.extended_charts {
            if !emitted.contains(&DrawingObjectKey::ExtendedChart(value.frame_id())) {
                value.write_to(writer, rel_list)?;
            }
        }
        for (ole_object, shape_id) in ole_objects.ole_object().iter().zip(shape_ids) {
            let key = two_identity(ole_object.two_cell_anchor()).map(DrawingObjectKey::Ole);
            if !key.as_ref().is_some_and(|key| emitted.contains(key)) {
                ole_object
                    .two_cell_anchor()
                    .write_to(writer, rel_list, shape_id)?;
            }
        }
        write_end_tag(writer, "xdr:wsDr");
        Ok(())
    }
}
impl AdjustmentCoordinate for WorksheetDrawing {
    fn adjustment_insert_coordinate(
        &mut self,
        root_col_num: u32,
        offset_col_num: u32,
        root_row_num: u32,
        offset_row_num: u32,
    ) {
        for chart in &mut self.extended_charts {
            chart.adjustment_insert_coordinate(
                root_col_num,
                offset_col_num,
                root_row_num,
                offset_row_num,
            );
        }
        for anchor in &mut self.one_cell_anchor_collection {
            anchor.adjustment_insert_coordinate(
                root_col_num,
                offset_col_num,
                root_row_num,
                offset_row_num,
            );
        }
        for anchor in &mut self.two_cell_anchor_collection {
            anchor.adjustment_insert_coordinate(
                root_col_num,
                offset_col_num,
                root_row_num,
                offset_row_num,
            );
        }
        for chart in &mut self.chart_collection {
            chart.adjustment_insert_coordinate(
                root_col_num,
                offset_col_num,
                root_row_num,
                offset_row_num,
            );
        }
        for image in &mut self.image_collection {
            image.adjustment_insert_coordinate(
                root_col_num,
                offset_col_num,
                root_row_num,
                offset_row_num,
            );
        }
    }

    fn adjustment_remove_coordinate(
        &mut self,
        root_col_num: u32,
        offset_col_num: u32,
        root_row_num: u32,
        offset_row_num: u32,
    ) {
        for chart in &mut self.extended_charts {
            chart.adjustment_remove_coordinate(
                root_col_num,
                offset_col_num,
                root_row_num,
                offset_row_num,
            );
        }
        self.one_cell_anchor_collection.retain(|k| {
            !(k.is_remove_coordinate(root_col_num, offset_col_num, root_row_num, offset_row_num))
        });
        for anchor in &mut self.one_cell_anchor_collection {
            anchor.adjustment_remove_coordinate(
                root_col_num,
                offset_col_num,
                root_row_num,
                offset_row_num,
            );
        }
        self.two_cell_anchor_collection.retain(|k| {
            !(k.is_remove_coordinate(root_col_num, offset_col_num, root_row_num, offset_row_num))
        });
        for anchor in &mut self.two_cell_anchor_collection {
            anchor.adjustment_remove_coordinate(
                root_col_num,
                offset_col_num,
                root_row_num,
                offset_row_num,
            );
        }
        self.chart_collection.retain(|k| {
            !(k.is_remove_coordinate(root_col_num, offset_col_num, root_row_num, offset_row_num))
        });
        for chart in &mut self.chart_collection {
            chart.adjustment_remove_coordinate(
                root_col_num,
                offset_col_num,
                root_row_num,
                offset_row_num,
            );
        }
        self.image_collection.retain(|k| {
            !(k.is_remove_coordinate(root_col_num, offset_col_num, root_row_num, offset_row_num))
        });
        for image in &mut self.image_collection {
            image.adjustment_remove_coordinate(
                root_col_num,
                offset_col_num,
                root_row_num,
                offset_row_num,
            );
        }
    }
}
impl AdjustmentCoordinateWithSheet for WorksheetDrawing {
    fn adjustment_insert_coordinate_with_sheet(
        &mut self,
        sheet_name: &str,
        root_col_num: u32,
        offset_col_num: u32,
        root_row_num: u32,
        offset_row_num: u32,
    ) {
        // chart
        for chart in &mut self.chart_collection {
            chart.adjustment_insert_coordinate_with_sheet(
                sheet_name,
                root_col_num,
                offset_col_num,
                root_row_num,
                offset_row_num,
            );
        }
    }

    fn adjustment_remove_coordinate_with_sheet(
        &mut self,
        sheet_name: &str,
        root_col_num: u32,
        offset_col_num: u32,
        root_row_num: u32,
        offset_row_num: u32,
    ) {
        // chart
        for chart in &mut self.chart_collection {
            chart.adjustment_remove_coordinate_with_sheet(
                sheet_name,
                root_col_num,
                offset_col_num,
                root_row_num,
                offset_row_num,
            );
        }
    }
}

fn frame_identity(value: &GraphicFrame) -> u32 {
    value
        .non_visual_graphic_frame_properties()
        .non_visual_drawing_properties()
        .id()
}
fn one_identity(value: &OneCellAnchor) -> Option<u32> {
    value
        .graphic_frame()
        .map(frame_identity)
        .or_else(|| {
            value.group_shape().map(|value| {
                value
                    .non_visual_group_shape_properties()
                    .non_visual_drawing_properties()
                    .id()
            })
        })
        .or_else(|| {
            value.shape().map(|value| {
                value
                    .non_visual_shape_properties()
                    .non_visual_drawing_properties()
                    .id()
            })
        })
        .or_else(|| {
            value.picture().map(|value| {
                value
                    .non_visual_picture_properties()
                    .non_visual_drawing_properties()
                    .id()
            })
        })
}
fn two_identity(value: &TwoCellAnchor) -> Option<u32> {
    value
        .graphic_frame()
        .map(frame_identity)
        .or_else(|| {
            value.group_shape().map(|value| {
                value
                    .non_visual_group_shape_properties()
                    .non_visual_drawing_properties()
                    .id()
            })
        })
        .or_else(|| {
            value.shape().map(|value| {
                value
                    .non_visual_shape_properties()
                    .non_visual_drawing_properties()
                    .id()
            })
        })
        .or_else(|| {
            value.connection_shape().map(|value| {
                value
                    .non_visual_connection_shape_properties()
                    .non_visual_drawing_properties()
                    .id()
            })
        })
        .or_else(|| {
            value.picture().map(|value| {
                value
                    .non_visual_picture_properties()
                    .non_visual_drawing_properties()
                    .id()
            })
        })
}
fn image_identity(value: &Image) -> Option<u32> {
    value
        .two_cell_anchor()
        .and_then(two_identity)
        .or_else(|| value.one_cell_anchor().and_then(one_identity))
}
fn unique<T>(mut values: impl Iterator<Item = T>) -> Result<Option<T>, XlsxError> {
    let first = values.next();
    if values.next().is_some() {
        return Err(super::extended_chart::drawing_error(
            "ambiguous drawing owner id",
        ));
    }
    Ok(first)
}
impl WorksheetDrawing {
    fn available_object_id(&self) -> u32 {
        let mut ids = HashSet::new();
        ids.extend(self.object_order.iter().map(DrawingObjectKey::id));
        self.visit_drawing_properties(&mut |properties| {
            ids.insert(properties.id());
        });
        let mut next = 1u32;
        while ids.contains(&next) {
            next = next.checked_add(1).expect("drawing IDs exhausted");
        }
        next
    }

    #[must_use]
    pub fn extended_charts(&self) -> &[ExtendedChart] {
        &self.extended_charts
    }

    pub fn extended_charts_mut(&mut self) -> &mut Vec<ExtendedChart> {
        &mut self.extended_charts
    }

    pub fn add_extended_chart(&mut self, mut value: ExtendedChart) -> &mut Self {
        if !value
            .frame_properties()
            .non_visual_drawing_properties()
            .has_id()
        {
            value
                .frame_properties_mut()
                .non_visual_drawing_properties_mut()
                .set_id(self.available_object_id());
        }
        self.extended_charts.push(value);
        self
    }

    #[must_use]
    pub fn absolute_anchors(&self) -> &[AbsoluteAnchor] {
        &self.absolute_anchors
    }

    pub fn absolute_anchors_mut(&mut self) -> &mut Vec<AbsoluteAnchor> {
        &mut self.absolute_anchors
    }

    pub fn add_absolute_anchor(&mut self, value: AbsoluteAnchor) -> &mut Self {
        self.absolute_anchors.push(value);
        self
    }

    #[must_use]
    pub fn object_order(&self) -> &[DrawingObjectKey] {
        &self.object_order
    }

    pub fn set_object_order(&mut self, value: Vec<DrawingObjectKey>) {
        self.object_order = value;
    }

    pub fn validate_object_order(&self) -> Result<(), XlsxError> {
        let mut ids = HashSet::new();
        for value in &self.object_order {
            if !ids.insert(value.id()) {
                return Err(super::extended_chart::drawing_error(
                    "duplicate active drawing owner id",
                ));
            }
        }
        let mut frame_keys = HashSet::new();
        let mut duplicate = false;
        self.visit_standard_frames(&mut |key, _| {
            duplicate |= !frame_keys.insert(key.relationship_key());
        });
        self.visit_extended_frames(&mut |key, _| {
            duplicate |= !frame_keys.insert(key.relationship_key());
        });
        if duplicate {
            return Err(super::extended_chart::drawing_error(
                "ambiguous scoped drawing frame identity",
            ));
        }
        for value in &self.absolute_anchors {
            if value.content().is_none() || value.extent().cx() < 0 || value.extent().cy() < 0 {
                return Err(super::extended_chart::drawing_error(
                    "invalid absolute anchor",
                ));
            }
        }
        for value in &self.extended_charts {
            value
                .chart_space()
                .validate()
                .map_err(super::extended_chart::drawing_error)?;
        }
        Ok(())
    }

    fn write_ordered(
        &self,
        key: &DrawingObjectKey,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        ole_objects: &OleObjects,
        shape_ids: &[usize],
        rels: &mut Vec<(String, String)>,
    ) -> Result<bool, XlsxError> {
        let begin = rels.len();
        let found =
            match key {
                DrawingObjectKey::Ole(id) => {
                    let Some((index, value)) =
                        unique(ole_objects.ole_object().iter().enumerate().filter(
                            |(_, value)| two_identity(value.two_cell_anchor()) == Some(*id),
                        ))?
                    else {
                        return Ok(false);
                    };
                    value
                        .two_cell_anchor()
                        .write_to(writer, rels, shape_ids[index])?;
                    true
                }
                DrawingObjectKey::Chart(id) => {
                    if let Some(value) = unique(
                        self.chart_collection
                            .iter()
                            .filter(|value| two_identity(value.two_cell_anchor()) == Some(*id)),
                    )? {
                        value.two_cell_anchor().write_to(writer, rels, 0)?;
                        true
                    } else {
                        false
                    }
                }
                DrawingObjectKey::Image(id) => {
                    if let Some(value) = unique(
                        self.image_collection
                            .iter()
                            .filter(|value| image_identity(value) == Some(*id)),
                    )? {
                        value.write_to(writer, rels)?;
                        true
                    } else {
                        false
                    }
                }
                DrawingObjectKey::OneCell(id) => {
                    if let Some(value) = unique(
                        self.one_cell_anchor_collection
                            .iter()
                            .filter(|value| one_identity(value) == Some(*id)),
                    )? {
                        value.write_to(writer, rels)?;
                        true
                    } else {
                        false
                    }
                }
                DrawingObjectKey::TwoCell(id) => {
                    if let Some(value) = unique(
                        self.two_cell_anchor_collection
                            .iter()
                            .filter(|value| two_identity(value) == Some(*id)),
                    )? {
                        value.write_to(writer, rels, 0)?;
                        true
                    } else {
                        false
                    }
                }
                DrawingObjectKey::Absolute(id) => {
                    if let Some(value) =
                        unique(self.absolute_anchors.iter().filter(|value| {
                            value.content().is_some_and(|value| value.id() == *id)
                        }))?
                    {
                        value.write_to(writer, rels)?;
                        true
                    } else {
                        false
                    }
                }
                DrawingObjectKey::ExtendedChart(id) => {
                    if let Some(value) = unique(
                        self.extended_charts
                            .iter()
                            .filter(|value| value.frame_id() == *id),
                    )? {
                        value.write_to(writer, rels)?;
                        true
                    } else {
                        false
                    }
                }
            };
        if matches!(
            key,
            DrawingObjectKey::Chart(_)
                | DrawingObjectKey::Image(_)
                | DrawingObjectKey::OneCell(_)
                | DrawingObjectKey::TwoCell(_)
                | DrawingObjectKey::Ole(_)
        ) {
            for (kind, identity) in &mut rels[begin..] {
                if kind == "CHART" && identity.is_empty() {
                    *identity = key.id().to_string();
                }
            }
        }
        Ok(found)
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        root: &BytesStart,
        relationships: Option<&RawRelationships>,
        ole_objects: &mut OleObjects,
        session: &crate::reader::xlsx::SourceCacheSession,
        provider: &mut dyn crate::reader::xlsx::ChartPartProvider,
    ) -> Result<(), XlsxError> {
        use super::extended_chart::{
            COMPATIBILITY_NS,
            drawing_error,
            with_fragment,
        };
        let root = OpaqueElement::read(reader, root, false, &NamespaceScope::default())
            .map_err(drawing_error)?;
        if !root.name.is(SHEET_DRAWING_NS, "wsDr") {
            return Err(drawing_error("invalid worksheet drawing namespace"));
        }
        let children = direct_children(&root).map_err(drawing_error)?;
        let mut ole_identities = HashSet::new();
        for child in children {
            if child.name.is(COMPATIBILITY_NS, "AlternateContent") {
                let choices = direct_children(&child).map_err(drawing_error)?;
                let valid_fallback = choices.len() == 2
                    && choices[1].name.is(COMPATIBILITY_NS, "Fallback")
                    && direct_children(&choices[1])
                        .map_err(drawing_error)?
                        .is_empty();
                if (choices.len() != 1 && !valid_fallback)
                    || !choices[0].name.is(COMPATIBILITY_NS, "Choice")
                {
                    return Err(drawing_error("unsupported OLE compatibility branches"));
                }
                let choice = &choices[0];
                let requirements = crate::xml::required_attribute(choice, "", "Requires")
                    .map_err(drawing_error)?;
                let scope = NamespaceScope::with_bindings(choice.namespaces.clone());
                let prefixes: Vec<_> = requirements.split_ascii_whitespace().collect();
                if prefixes.is_empty() {
                    return Err(drawing_error("OLE compatibility requirements are missing"));
                }
                for prefix in prefixes {
                    let name = scope
                        .resolve(format!("{prefix}:required").as_bytes(), false)
                        .map_err(drawing_error)?;
                    if name.namespace_uri != crate::helper::const_str::DRAWING_MAIN_NS {
                        return Err(drawing_error("unsupported OLE compatibility requirement"));
                    }
                }
                let anchors = direct_children(choice).map_err(drawing_error)?;
                if anchors.len() != 1 || !anchors[0].name.is(SHEET_DRAWING_NS, "twoCellAnchor") {
                    return Err(drawing_error("unsupported OLE compatibility owner"));
                }
                let mut anchor = TwoCellAnchor::default();
                with_fragment(&anchors[0], |reader, start, _| {
                    let _scope = session.push_extension_scope(
                        NamespaceScope::default()
                            .child(start)
                            .map_err(drawing_error)?,
                    );
                    let immediate = session.immediate_subsession();
                    anchor.set_attributes(reader, start, relationships, &immediate, provider)
                })?;
                anchor.set_is_alternate_content(true);
                let id = two_identity(&anchor).ok_or_else(|| {
                    drawing_error("compatibility anchor requires a typed drawing object")
                })?;
                if let Some((index, identity)) = find_ole_owner(&anchor, ole_objects)? {
                    if !ole_identities.insert(identity) {
                        return Err(drawing_error("duplicate drawing OLE owner"));
                    }
                    ole_objects.ole_object_mut()[index].set_two_cell_anchor(anchor);
                    self.object_order.push(DrawingObjectKey::Ole(id));
                } else {
                    // VML controls use this envelope too, but their actual
                    // native shape identity does not make them OLE objects.
                    let control = anchor.shape().is_some()
                        && crate::structs::OleObject::anchor_properties(&anchor)
                            .map(NonVisualDrawingProperties::legacy_shape_id)
                            .transpose()
                            .map_err(drawing_error)?
                            .flatten()
                            .is_some();
                    if !control {
                        return Err(drawing_error("drawing OLE owner is missing"));
                    }
                    self.object_order.push(DrawingObjectKey::TwoCell(id));
                    self.add_two_cell_anchor_collection(anchor);
                }
                continue;
            }
            if child.name.namespace_uri != SHEET_DRAWING_NS {
                return Err(drawing_error(
                    "unsupported worksheet drawing compatibility content",
                ));
            }
            if !matches!(
                child.name.local_name.as_str(),
                "oneCellAnchor" | "twoCellAnchor" | "absoluteAnchor"
            ) {
                return Err(drawing_error("unsupported worksheet drawing child"));
            }
            if let Some(chart) =
                ExtendedChart::from_anchor(&child, relationships, session, provider)?
            {
                self.object_order
                    .push(DrawingObjectKey::ExtendedChart(chart.frame_id()));
                self.add_extended_chart(chart);
                continue;
            }
            if child.name.local_name == "absoluteAnchor" {
                let value = AbsoluteAnchor::from_element(&child, relationships, session, provider)?;
                self.object_order.push(DrawingObjectKey::Absolute(
                    value.content().expect("validated object").id(),
                ));
                self.add_absolute_anchor(value);
            } else if child.name.local_name == "oneCellAnchor" {
                let value = with_fragment(&child, |reader, start, _| {
                    let _scope = session.push_extension_scope(
                        NamespaceScope::default()
                            .child(start)
                            .map_err(drawing_error)?,
                    );
                    let mut value = OneCellAnchor::default();
                    value.set_attributes(reader, start, relationships, session, provider)?;
                    Ok(value)
                })?;
                let id = one_identity(&value)
                    .ok_or_else(|| drawing_error("one-cell anchor requires a typed object"))?;
                if value.is_image() {
                    let mut image = Image::default();
                    image.set_one_cell_anchor(value);
                    self.object_order.push(DrawingObjectKey::Image(id));
                    self.add_image(image);
                } else {
                    self.object_order.push(DrawingObjectKey::OneCell(id));
                    self.add_one_cell_anchor_collection(value);
                }
            } else {
                let mut value = with_fragment(&child, |reader, start, _| {
                    let _scope = session.push_extension_scope(
                        NamespaceScope::default()
                            .child(start)
                            .map_err(drawing_error)?,
                    );
                    let mut value = TwoCellAnchor::default();
                    value.set_attributes(reader, start, relationships, session, provider)?;
                    Ok(value)
                })?;
                let id = two_identity(&value)
                    .ok_or_else(|| drawing_error("two-cell anchor requires a typed object"))?;
                if let Some((index, identity)) = find_ole_owner(&value, ole_objects)? {
                    if !ole_identities.insert(identity) {
                        return Err(drawing_error("duplicate drawing OLE owner"));
                    }
                    session.hydrate_ole_anchor(&mut value, provider)?;
                    self.object_order.push(DrawingObjectKey::Ole(id));
                    ole_objects.ole_object_mut()[index].set_two_cell_anchor(value);
                    continue;
                }
                if !value.is_support() {
                    continue;
                }
                if value.is_chart() {
                    let mut chart = Chart::default();
                    chart.set_two_cell_anchor(value);
                    self.object_order.push(DrawingObjectKey::Chart(id));
                    self.add_chart_collection(chart);
                } else if value.is_image() {
                    let mut image = Image::default();
                    image.set_two_cell_anchor(value);
                    self.object_order.push(DrawingObjectKey::Image(id));
                    self.add_image(image);
                } else {
                    self.object_order.push(DrawingObjectKey::TwoCell(id));
                    self.add_two_cell_anchor_collection(value);
                }
            }
        }
        if ole_objects.ole_object().iter().any(|object| {
            !object.requires().is_empty()
                && object
                    .shape_id()
                    .is_some_and(|id| !ole_identities.contains(&id))
        }) {
            return Err(drawing_error("drawing OLE anchor is missing"));
        }
        self.validate_object_order()
    }
}

fn find_ole_owner(
    anchor: &TwoCellAnchor,
    objects: &OleObjects,
) -> Result<Option<(usize, u32)>, XlsxError> {
    let Some(properties) = crate::structs::OleObject::anchor_properties(anchor) else {
        return Ok(None);
    };
    // cNvPr/@id belongs to the native DrawingML object ID domain. The
    // worksheet OLE shapeId belongs to the VML domain and must be linked by
    // actual compatibility identity, never guessed from the native ID.
    let Some(identity) = properties
        .legacy_shape_id()
        .map_err(super::extended_chart::drawing_error)?
    else {
        return Ok(None);
    };
    let mut matches = objects
        .ole_object()
        .iter()
        .enumerate()
        .filter(|(_, object)| object.shape_id() == Some(identity));
    let Some((index, _)) = matches.next() else {
        // compatExt also identifies VML controls (buttons/list boxes), not
        // only OLE objects. Only a matching worksheet oleObject owns this
        // anchor; never infer an OLE owner from the extension alone.
        return Ok(None);
    };
    if matches.next().is_some() {
        return Err(super::extended_chart::drawing_error(
            "ambiguous drawing OLE shape identity",
        ));
    }
    Ok(Some((index, identity)))
}

fn assign_one_identity(value: &mut OneCellAnchor, id: u32) {
    let properties = if value.graphic_frame().is_some() {
        value
            .graphic_frame_mut()
            .unwrap()
            .non_visual_graphic_frame_properties_mut()
            .non_visual_drawing_properties_mut()
    } else if value.group_shape().is_some() {
        value
            .group_shape_mut()
            .unwrap()
            .non_visual_group_shape_properties_mut()
            .non_visual_drawing_properties_mut()
    } else if value.shape().is_some() {
        value
            .shape_mut()
            .unwrap()
            .non_visual_shape_properties_mut()
            .non_visual_drawing_properties_mut()
    } else if value.picture().is_some() {
        value
            .picture_mut()
            .unwrap()
            .non_visual_picture_properties_mut()
            .non_visual_drawing_properties_mut()
    } else {
        return;
    };
    if !properties.has_id() {
        properties.set_id(id);
    }
}
fn assign_two_identity(value: &mut TwoCellAnchor, id: u32) {
    let properties = if value.graphic_frame().is_some() {
        value
            .graphic_frame_mut()
            .unwrap()
            .non_visual_graphic_frame_properties_mut()
            .non_visual_drawing_properties_mut()
    } else if value.group_shape().is_some() {
        value
            .group_shape_mut()
            .unwrap()
            .non_visual_group_shape_properties_mut()
            .non_visual_drawing_properties_mut()
    } else if value.shape().is_some() {
        value
            .shape_mut()
            .unwrap()
            .non_visual_shape_properties_mut()
            .non_visual_drawing_properties_mut()
    } else if value.connection_shape().is_some() {
        value
            .connection_shape_mut()
            .unwrap()
            .non_visual_connection_shape_properties_mut()
            .non_visual_drawing_properties_mut()
    } else if value.picture().is_some() {
        value
            .picture_mut()
            .unwrap()
            .non_visual_picture_properties_mut()
            .non_visual_drawing_properties_mut()
    } else {
        return;
    };
    if !properties.has_id() {
        properties.set_id(id);
    }
}

#[cfg(test)]
mod drawing_xml_tests {
    use super::*;

    #[test]
    fn malformed_drawing_attributes_are_rejected_before_dispatch() {
        let xml = br#"<xdr:wsDr xmlns:xdr="http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing"><xdr:twoCellAnchor><xdr:sp><xdr:nvSpPr><xdr:cNvPr id="1" \ name="Group 1"/></xdr:nvSpPr></xdr:sp></xdr:twoCellAnchor></xdr:wsDr>"#;
        let mut reader = Reader::from_reader(&xml[..]);
        let Event::Start(root) = reader.read_event().unwrap() else {
            panic!("drawing root")
        };
        let mut drawing = WorksheetDrawing::default();
        let mut provider =
            crate::reader::xlsx::LazyChartPartProvider::new(std::collections::HashMap::new());
        let error = drawing
            .set_attributes(
                &mut reader,
                &root,
                None,
                &mut OleObjects::default(),
                &crate::reader::xlsx::SourceCacheSession::deferred(),
                &mut provider,
            )
            .unwrap_err();
        assert!(error.to_string().contains("attribute"), "{error}");
    }
}

impl WorksheetDrawing {
    pub fn visit_standard_frames(
        &self,
        visit: &mut dyn FnMut(super::DrawingFrameKey, &GraphicFrame),
    ) {
        for value in &self.chart_collection {
            value.two_cell_anchor().visit_standard_frames(visit);
        }
        for value in &self.image_collection {
            if let Some(anchor) = value.one_cell_anchor() {
                anchor.visit_standard_frames(visit);
            }
            if let Some(anchor) = value.two_cell_anchor() {
                anchor.visit_standard_frames(visit);
            }
        }
        for anchor in &self.two_cell_anchor_collection {
            anchor.visit_standard_frames(visit);
        }
        for anchor in &self.one_cell_anchor_collection {
            anchor.visit_standard_frames(visit);
        }
        for value in &self.absolute_anchors {
            if let Some(content) = value.content() {
                content
                    .visit_standard_frames(&super::DrawingFrameKey::primary(content.id()), visit);
            }
        }
        for value in &self.extended_charts {
            value.visit_standard_frames(visit);
        }
    }

    pub fn visit_standard_frames_mut(
        &mut self,
        visit: &mut dyn FnMut(super::DrawingFrameKey, &mut GraphicFrame),
    ) {
        for value in &mut self.chart_collection {
            value.two_cell_anchor_mut().visit_standard_frames_mut(visit);
        }
        for value in &mut self.image_collection {
            if let Some(anchor) = value.one_cell_anchor_mut() {
                anchor.visit_standard_frames_mut(visit);
            }
            if let Some(anchor) = value.two_cell_anchor_mut() {
                anchor.visit_standard_frames_mut(visit);
            }
        }
        for anchor in &mut self.two_cell_anchor_collection {
            anchor.visit_standard_frames_mut(visit);
        }
        for anchor in &mut self.one_cell_anchor_collection {
            anchor.visit_standard_frames_mut(visit);
        }
        for value in &mut self.absolute_anchors {
            if let Some(content) = value.content_mut() {
                content.visit_standard_frames_mut(
                    &super::DrawingFrameKey::primary(content.id()),
                    visit,
                );
            }
        }
        for value in &mut self.extended_charts {
            value.visit_standard_frames_mut(visit);
        }
    }

    pub fn visit_extended_frames(
        &self,
        visit: &mut dyn FnMut(super::DrawingFrameKey, &super::ExtendedChartFrame),
    ) {
        for value in &self.chart_collection {
            value.two_cell_anchor().visit_extended_frames(visit);
        }
        for value in &self.image_collection {
            if let Some(anchor) = value.one_cell_anchor() {
                anchor.visit_extended_frames(visit);
            }
            if let Some(anchor) = value.two_cell_anchor() {
                anchor.visit_extended_frames(visit);
            }
        }
        for anchor in &self.two_cell_anchor_collection {
            anchor.visit_extended_frames(visit);
        }
        for anchor in &self.one_cell_anchor_collection {
            anchor.visit_extended_frames(visit);
        }
        for value in &self.absolute_anchors {
            if let Some(content) = value.content() {
                content
                    .visit_extended_frames(&super::DrawingFrameKey::primary(content.id()), visit);
            }
        }
        for value in &self.extended_charts {
            value.visit_extended_frames(visit);
        }
    }

    pub fn visit_extended_frames_mut(
        &mut self,
        visit: &mut dyn FnMut(super::DrawingFrameKey, &mut super::ExtendedChartFrame),
    ) {
        for value in &mut self.chart_collection {
            value.two_cell_anchor_mut().visit_extended_frames_mut(visit);
        }
        for value in &mut self.image_collection {
            if let Some(anchor) = value.one_cell_anchor_mut() {
                anchor.visit_extended_frames_mut(visit);
            }
            if let Some(anchor) = value.two_cell_anchor_mut() {
                anchor.visit_extended_frames_mut(visit);
            }
        }
        for anchor in &mut self.two_cell_anchor_collection {
            anchor.visit_extended_frames_mut(visit);
        }
        for anchor in &mut self.one_cell_anchor_collection {
            anchor.visit_extended_frames_mut(visit);
        }
        for value in &mut self.absolute_anchors {
            if let Some(content) = value.content_mut() {
                content.visit_extended_frames_mut(
                    &super::DrawingFrameKey::primary(content.id()),
                    visit,
                );
            }
        }
        for value in &mut self.extended_charts {
            value.visit_extended_frames_mut(visit);
        }
    }

    pub fn visit_extended_spaces(
        &self,
        visit: &mut dyn FnMut(&crate::drawing::charts::ChartExSpace),
    ) {
        self.visit_extended_frames(&mut |_, frame| visit(frame.chart_space()));
    }

    pub fn visit_extended_spaces_mut(
        &mut self,
        visit: &mut dyn FnMut(&mut crate::drawing::charts::ChartExSpace),
    ) {
        self.visit_extended_frames_mut(&mut |_, frame| visit(frame.chart_space_mut()));
    }
}

fn one_properties(value: &OneCellAnchor, visit: &mut dyn FnMut(&NonVisualDrawingProperties)) {
    if let Some(value) = value.graphic_frame() {
        visit(
            value
                .non_visual_graphic_frame_properties()
                .non_visual_drawing_properties(),
        );
    }
    if let Some(value) = value.group_shape() {
        super::absolute_anchor::group_properties(value, visit);
    }
    if let Some(value) = value.shape() {
        visit(
            value
                .non_visual_shape_properties()
                .non_visual_drawing_properties(),
        );
    }
    if let Some(value) = value.picture() {
        visit(
            value
                .non_visual_picture_properties()
                .non_visual_drawing_properties(),
        );
    }
}
fn one_properties_mut(
    value: &mut OneCellAnchor,
    visit: &mut dyn FnMut(&mut NonVisualDrawingProperties),
) {
    if let Some(value) = value.graphic_frame_mut() {
        visit(
            value
                .non_visual_graphic_frame_properties_mut()
                .non_visual_drawing_properties_mut(),
        );
    }
    if let Some(value) = value.group_shape_mut() {
        super::absolute_anchor::group_properties_mut(value, visit);
    }
    if let Some(value) = value.shape_mut() {
        visit(
            value
                .non_visual_shape_properties_mut()
                .non_visual_drawing_properties_mut(),
        );
    }
    if let Some(value) = value.picture_mut() {
        visit(
            value
                .non_visual_picture_properties_mut()
                .non_visual_drawing_properties_mut(),
        );
    }
}
fn two_properties(value: &TwoCellAnchor, visit: &mut dyn FnMut(&NonVisualDrawingProperties)) {
    if let Some(value) = value.graphic_frame() {
        visit(
            value
                .non_visual_graphic_frame_properties()
                .non_visual_drawing_properties(),
        );
    }
    if let Some(value) = value.group_shape() {
        super::absolute_anchor::group_properties(value, visit);
    }
    if let Some(value) = value.shape() {
        visit(
            value
                .non_visual_shape_properties()
                .non_visual_drawing_properties(),
        );
    }
    if let Some(value) = value.connection_shape() {
        visit(
            value
                .non_visual_connection_shape_properties()
                .non_visual_drawing_properties(),
        );
    }
    if let Some(value) = value.picture() {
        visit(
            value
                .non_visual_picture_properties()
                .non_visual_drawing_properties(),
        );
    }
}
fn two_properties_mut(
    value: &mut TwoCellAnchor,
    visit: &mut dyn FnMut(&mut NonVisualDrawingProperties),
) {
    if let Some(value) = value.graphic_frame_mut() {
        visit(
            value
                .non_visual_graphic_frame_properties_mut()
                .non_visual_drawing_properties_mut(),
        );
    }
    if let Some(value) = value.group_shape_mut() {
        super::absolute_anchor::group_properties_mut(value, visit);
    }
    if let Some(value) = value.shape_mut() {
        visit(
            value
                .non_visual_shape_properties_mut()
                .non_visual_drawing_properties_mut(),
        );
    }
    if let Some(value) = value.connection_shape_mut() {
        visit(
            value
                .non_visual_connection_shape_properties_mut()
                .non_visual_drawing_properties_mut(),
        );
    }
    if let Some(value) = value.picture_mut() {
        visit(
            value
                .non_visual_picture_properties_mut()
                .non_visual_drawing_properties_mut(),
        );
    }
}
impl WorksheetDrawing {
    pub fn visit_drawing_properties(&self, visit: &mut dyn FnMut(&NonVisualDrawingProperties)) {
        for value in &self.chart_collection {
            two_properties(value.two_cell_anchor(), visit);
        }
        for value in &self.image_collection {
            if let Some(value) = value.one_cell_anchor() {
                one_properties(value, visit);
            }
            if let Some(value) = value.two_cell_anchor() {
                two_properties(value, visit);
            }
        }
        for value in &self.one_cell_anchor_collection {
            one_properties(value, visit);
        }
        for value in &self.two_cell_anchor_collection {
            two_properties(value, visit);
        }
        for value in &self.extended_charts {
            value.visit_properties(visit);
        }
        for value in &self.absolute_anchors {
            if let Some(value) = value.content() {
                value.visit_properties(visit);
            }
        }
    }

    pub fn visit_drawing_properties_mut(
        &mut self,
        visit: &mut dyn FnMut(&mut NonVisualDrawingProperties),
    ) {
        for value in &mut self.chart_collection {
            two_properties_mut(value.two_cell_anchor_mut(), visit);
        }
        for value in &mut self.image_collection {
            if let Some(value) = value.one_cell_anchor_mut() {
                one_properties_mut(value, visit);
            }
            if let Some(value) = value.two_cell_anchor_mut() {
                two_properties_mut(value, visit);
            }
        }
        for value in &mut self.one_cell_anchor_collection {
            one_properties_mut(value, visit);
        }
        for value in &mut self.two_cell_anchor_collection {
            two_properties_mut(value, visit);
        }
        for value in &mut self.extended_charts {
            value.visit_properties_mut(visit);
        }
        for value in &mut self.absolute_anchors {
            if let Some(value) = value.content_mut() {
                value.visit_properties_mut(visit);
            }
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

    pub(crate) fn additional_media_objects(&self) -> Vec<&crate::structs::MediaObject> {
        let mut values = Vec::new();
        for value in &self.absolute_anchors {
            if let Some(value) = value.content() {
                values.extend(value.media_objects());
            }
        }
        for value in &self.extended_charts {
            values.extend(value.media_objects());
        }
        for value in &self.one_cell_anchor_collection {
            if let Some(value) = value.picture() {
                values.push(value.blip_fill().blip().image());
            }
            if let Some(value) = value
                .shape()
                .and_then(|value| value.shape_properties().blip_fill())
            {
                values.push(value.blip().image());
            }
            if let Some(value) = value.group_shape() {
                for child in value.content() {
                    values.extend(child.media_objects());
                }
            }
        }
        for value in &self.two_cell_anchor_collection {
            if let Some(value) = value.picture() {
                values.push(value.blip_fill().blip().image());
            }
            if let Some(value) = value
                .shape()
                .and_then(|value| value.shape_properties().blip_fill())
            {
                values.push(value.blip().image());
            }
            if let Some(value) = value
                .connection_shape()
                .and_then(|value| value.shape_properties().blip_fill())
            {
                values.push(value.blip().image());
            }
            if let Some(value) = value.group_shape() {
                for child in value.content() {
                    values.extend(child.media_objects());
                }
            }
        }
        values
    }
}

fn one_shape_properties(value: &OneCellAnchor, visit: &mut dyn FnMut(&super::ShapeProperties)) {
    if let Some(value) = value.shape() {
        visit(value.shape_properties());
    }
    if let Some(value) = value.picture() {
        visit(value.shape_properties());
    }
    if let Some(value) = value.group_shape() {
        for child in value.content() {
            child.visit_shape_properties(visit);
        }
    }
}
fn one_shape_properties_mut(
    value: &mut OneCellAnchor,
    visit: &mut dyn FnMut(&mut super::ShapeProperties),
) {
    if let Some(value) = value.shape_mut() {
        visit(value.shape_properties_mut());
    }
    if let Some(value) = value.picture_mut() {
        visit(value.shape_properties_mut());
    }
    if let Some(value) = value.group_shape_mut() {
        for child in value.content_mut() {
            child.visit_shape_properties_mut(visit);
        }
    }
}
fn two_shape_properties(value: &TwoCellAnchor, visit: &mut dyn FnMut(&super::ShapeProperties)) {
    if let Some(value) = value.shape() {
        visit(value.shape_properties());
    }
    if let Some(value) = value.connection_shape() {
        visit(value.shape_properties());
    }
    if let Some(value) = value.picture() {
        visit(value.shape_properties());
    }
    if let Some(value) = value.group_shape() {
        for child in value.content() {
            child.visit_shape_properties(visit);
        }
    }
}
fn two_shape_properties_mut(
    value: &mut TwoCellAnchor,
    visit: &mut dyn FnMut(&mut super::ShapeProperties),
) {
    if let Some(value) = value.shape_mut() {
        visit(value.shape_properties_mut());
    }
    if let Some(value) = value.connection_shape_mut() {
        visit(value.shape_properties_mut());
    }
    if let Some(value) = value.picture_mut() {
        visit(value.shape_properties_mut());
    }
    if let Some(value) = value.group_shape_mut() {
        for child in value.content_mut() {
            child.visit_shape_properties_mut(visit);
        }
    }
}
impl WorksheetDrawing {
    fn visit_shape_properties(&self, visit: &mut dyn FnMut(&super::ShapeProperties)) {
        for value in &self.chart_collection {
            two_shape_properties(value.two_cell_anchor(), visit);
        }
        for value in &self.image_collection {
            if let Some(value) = value.one_cell_anchor() {
                one_shape_properties(value, visit);
            }
            if let Some(value) = value.two_cell_anchor() {
                two_shape_properties(value, visit);
            }
        }
        for value in &self.one_cell_anchor_collection {
            one_shape_properties(value, visit);
        }
        for value in &self.two_cell_anchor_collection {
            two_shape_properties(value, visit);
        }
        for value in &self.absolute_anchors {
            if let Some(value) = value.content() {
                value.visit_shape_properties(visit);
            }
        }
        for value in &self.extended_charts {
            value.visit_shape_properties(visit);
        }
    }

    fn visit_shape_properties_mut(&mut self, visit: &mut dyn FnMut(&mut super::ShapeProperties)) {
        for value in &mut self.chart_collection {
            two_shape_properties_mut(value.two_cell_anchor_mut(), visit);
        }
        for value in &mut self.image_collection {
            if let Some(value) = value.one_cell_anchor_mut() {
                one_shape_properties_mut(value, visit);
            }
            if let Some(value) = value.two_cell_anchor_mut() {
                two_shape_properties_mut(value, visit);
            }
        }
        for value in &mut self.one_cell_anchor_collection {
            one_shape_properties_mut(value, visit);
        }
        for value in &mut self.two_cell_anchor_collection {
            two_shape_properties_mut(value, visit);
        }
        for value in &mut self.absolute_anchors {
            if let Some(value) = value.content_mut() {
                value.visit_shape_properties_mut(visit);
            }
        }
        for value in &mut self.extended_charts {
            value.visit_shape_properties_mut(visit);
        }
    }
}

impl TwoCellAnchor {
    pub(crate) fn visit_relationships(
        &self,
        visit: &mut dyn FnMut(super::DrawingRelationshipKind, &str),
    ) {
        self.visit_non_visual_relationships(visit);
        two_shape_properties(self, &mut |properties| {
            properties.visit_relationships(visit);
        });
        self.visit_picture_blips(&mut |value| value.visit_relationships(visit));
    }
}

#[cfg(test)]
mod identity_tests {
    use super::*;

    #[test]
    fn ole_binding_requires_actual_legacy_identity_not_native_id_or_ordinal() {
        let mut shape = Shape::default();
        shape
            .non_visual_shape_properties_mut()
            .non_visual_drawing_properties_mut()
            .set_id(7);
        let mut anchor = TwoCellAnchor::default();
        anchor.set_shape(shape);
        let mut objects = OleObjects::default();
        let mut first = crate::OleObject::default();
        first.set_shape_id(7);
        objects.set_ole_object(first);
        let mut second = crate::OleObject::default();
        second.set_shape_id(4501);
        objects.set_ole_object(second);
        assert_eq!(find_ole_owner(&anchor, &objects).unwrap(), None);
        let extension = crate::xml::root(br#"<a:extLst xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:a14="http://schemas.microsoft.com/office/drawing/2010/main"><a:ext uri="{63B3BB69-23CF-44E3-9099-C40C66FF867C}"><a14:compatExt spid="_x0000_s4501"/></a:ext></a:extLst>"#).unwrap();
        anchor
            .shape_mut()
            .unwrap()
            .non_visual_shape_properties_mut()
            .non_visual_drawing_properties_mut()
            .set_extensions(Some(extension));
        assert_eq!(find_ole_owner(&anchor, &objects).unwrap(), Some((1, 4501)));
        assert_eq!(
            anchor
                .shape()
                .unwrap()
                .non_visual_shape_properties()
                .non_visual_drawing_properties()
                .id(),
            7
        );
    }

    #[test]
    fn new_object_ids_reserve_group_children_and_imported_ole_owners() {
        let mut group = super::super::GroupShape::default();
        group
            .non_visual_group_shape_properties_mut()
            .non_visual_drawing_properties_mut()
            .set_id(4);
        for id in [1, 2] {
            let mut shape = Shape::default();
            shape
                .non_visual_shape_properties_mut()
                .non_visual_drawing_properties_mut()
                .set_id(id);
            group.add_shape_collection(shape);
        }
        let mut anchor = TwoCellAnchor::default();
        anchor.set_group_shape(group);
        let mut drawing = WorksheetDrawing::default();
        drawing.add_two_cell_anchor_collection(anchor);
        drawing.object_order.push(DrawingObjectKey::Ole(3));
        drawing.add_extended_chart(ExtendedChart::default());
        drawing.add_extended_chart(ExtendedChart::default());
        assert_eq!(drawing.extended_charts()[0].frame_id(), 5);
        assert_eq!(drawing.extended_charts()[1].frame_id(), 6);
    }
}

impl WorksheetDrawing {
    fn visit_picture_blips(&self, visit: &mut dyn FnMut(&crate::drawing::Blip)) {
        for value in &self.chart_collection {
            value.two_cell_anchor().visit_picture_blips(visit);
        }
        for value in &self.one_cell_anchor_collection {
            value.visit_picture_blips(visit);
        }
        for value in &self.two_cell_anchor_collection {
            value.visit_picture_blips(visit);
        }
        for value in &self.image_collection {
            if let Some(anchor) = value.one_cell_anchor() {
                anchor.visit_picture_blips(visit);
            }
            if let Some(anchor) = value.two_cell_anchor() {
                anchor.visit_picture_blips(visit);
            }
        }
        for value in &self.absolute_anchors {
            if let Some(content) = value.content() {
                content.visit_picture_blips(visit);
            }
        }
        for value in &self.extended_charts {
            value.visit_picture_blips(visit);
        }
    }
}

impl WorksheetDrawing {
    fn visit_picture_blips_mut(&mut self, visit: &mut dyn FnMut(&mut crate::drawing::Blip)) {
        for value in &mut self.chart_collection {
            value.two_cell_anchor_mut().visit_picture_blips_mut(visit);
        }
        for value in &mut self.one_cell_anchor_collection {
            value.visit_picture_blips_mut(visit);
        }
        for value in &mut self.two_cell_anchor_collection {
            value.visit_picture_blips_mut(visit);
        }
        for value in &mut self.image_collection {
            if let Some(anchor) = value.one_cell_anchor_mut() {
                anchor.visit_picture_blips_mut(visit);
            }
            if let Some(anchor) = value.two_cell_anchor_mut() {
                anchor.visit_picture_blips_mut(visit);
            }
        }
        for value in &mut self.absolute_anchors {
            if let Some(content) = value.content_mut() {
                content.visit_picture_blips_mut(visit);
            }
        }
        for value in &mut self.extended_charts {
            value.visit_picture_blips_mut(visit);
        }
    }
}

impl WorksheetDrawing {
    fn visit_non_visual_relationships(
        &self,
        visit: &mut dyn FnMut(super::DrawingRelationshipKind, &str),
    ) {
        for value in &self.chart_collection {
            value
                .two_cell_anchor()
                .visit_non_visual_relationships(visit);
        }
        for value in &self.one_cell_anchor_collection {
            value.visit_non_visual_relationships(visit);
        }
        for value in &self.two_cell_anchor_collection {
            value.visit_non_visual_relationships(visit);
        }
        for value in &self.image_collection {
            if let Some(anchor) = value.one_cell_anchor() {
                anchor.visit_non_visual_relationships(visit);
            }
            if let Some(anchor) = value.two_cell_anchor() {
                anchor.visit_non_visual_relationships(visit);
            }
        }
        for value in &self.absolute_anchors {
            if let Some(content) = value.content() {
                content.visit_non_visual_relationships(visit);
            }
        }
        for value in &self.extended_charts {
            value.visit_non_visual_relationships(visit);
        }
    }

    fn visit_non_visual_relationships_mut(
        &mut self,
        visit: &mut dyn FnMut(super::DrawingRelationshipKind, &mut String),
    ) {
        for value in &mut self.chart_collection {
            value
                .two_cell_anchor_mut()
                .visit_non_visual_relationships_mut(visit);
        }
        for value in &mut self.one_cell_anchor_collection {
            value.visit_non_visual_relationships_mut(visit);
        }
        for value in &mut self.two_cell_anchor_collection {
            value.visit_non_visual_relationships_mut(visit);
        }
        for value in &mut self.image_collection {
            if let Some(anchor) = value.one_cell_anchor_mut() {
                anchor.visit_non_visual_relationships_mut(visit);
            }
            if let Some(anchor) = value.two_cell_anchor_mut() {
                anchor.visit_non_visual_relationships_mut(visit);
            }
        }
        for value in &mut self.absolute_anchors {
            if let Some(content) = value.content_mut() {
                content.visit_non_visual_relationships_mut(visit);
            }
        }
        for value in &mut self.extended_charts {
            value.visit_non_visual_relationships_mut(visit);
        }
    }
}
