// xdr:graphicFrame
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
    super::{
        super::StringValue,
        Graphic,
    },
    NonVisualGraphicFrameProperties,
    Transform,
};
use crate::{
    structs::raw::RawRelationships,
    traits::AdjustmentCoordinateWithSheet,
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct GraphicFrame {
    r#macro:                             StringValue,
    non_visual_graphic_frame_properties: NonVisualGraphicFrameProperties,
    transform:                           Transform,
    graphic:                             Graphic,
    #[cfg_attr(feature = "serde", serde(default))]
    source_parts:                        Option<Box<[String]>>,
}

impl GraphicFrame {
    #[must_use]
    pub fn source_parts(&self) -> &[String] {
        self.source_parts.as_deref().unwrap_or_default()
    }

    pub(crate) fn set_source_parts(&mut self, value: Option<Vec<String>>) {
        self.source_parts = value.map(Vec::into_boxed_slice);
    }

    pub(crate) fn restore_source_parts(&mut self, previous: &Self) {
        self.source_parts.clone_from(&previous.source_parts);
    }

    #[inline]
    #[must_use]
    pub fn r#macro(&self) -> &str {
        self.r#macro.value_str()
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use macro()")]
    pub fn get_macro(&self) -> &str {
        self.r#macro()
    }

    #[inline]
    pub fn set_macro<S: Into<String>>(&mut self, value: S) -> &mut GraphicFrame {
        self.r#macro.set_value(value);
        self
    }

    #[inline]
    #[must_use]
    pub fn non_visual_graphic_frame_properties(&self) -> &NonVisualGraphicFrameProperties {
        &self.non_visual_graphic_frame_properties
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use non_visual_graphic_frame_properties()")]
    pub fn get_non_visual_graphic_frame_properties(&self) -> &NonVisualGraphicFrameProperties {
        self.non_visual_graphic_frame_properties()
    }

    #[inline]
    pub fn non_visual_graphic_frame_properties_mut(
        &mut self,
    ) -> &mut NonVisualGraphicFrameProperties {
        &mut self.non_visual_graphic_frame_properties
    }

    #[inline]
    #[deprecated(
        since = "3.0.0",
        note = "Use non_visual_graphic_frame_properties_mut()"
    )]
    pub fn get_non_visual_graphic_frame_properties_mut(
        &mut self,
    ) -> &mut NonVisualGraphicFrameProperties {
        self.non_visual_graphic_frame_properties_mut()
    }

    #[inline]
    pub fn set_non_visual_graphic_frame_properties(
        &mut self,
        value: NonVisualGraphicFrameProperties,
    ) -> &mut Self {
        self.non_visual_graphic_frame_properties = value;
        self
    }

    #[inline]
    #[must_use]
    pub fn transform(&self) -> &Transform {
        &self.transform
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use transform()")]
    pub fn get_transform(&self) -> &Transform {
        self.transform()
    }

    #[inline]
    pub fn transform_mut(&mut self) -> &mut Transform {
        &mut self.transform
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use transform_mut()")]
    pub fn get_transform_mut(&mut self) -> &mut Transform {
        self.transform_mut()
    }

    #[inline]
    pub fn set_transform(&mut self, value: Transform) -> &mut Self {
        self.transform = value;
        self
    }

    #[inline]
    #[must_use]
    pub fn graphic(&self) -> &Graphic {
        &self.graphic
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use graphic()")]
    pub fn get_graphic(&self) -> &Graphic {
        self.graphic()
    }

    #[inline]
    pub fn graphic_mut(&mut self) -> &mut Graphic {
        &mut self.graphic
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use graphic_mut()")]
    pub fn get_graphic_mut(&mut self) -> &mut Graphic {
        self.graphic_mut()
    }

    #[inline]
    pub fn set_graphic(&mut self, value: Graphic) -> &mut Self {
        self.graphic = value;
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
        if let Some(value) = crate::reader::driver::read_attribute(owner, b"macro")? {
            self.r#macro.set_value(value);
        }
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
                    if !seen.insert((name.namespace_uri.clone(), name.local_name.clone())) {
                        return Err(crate::xml::xml_error("duplicate graphic frame child"));
                    }
                    if name.is(crate::helper::const_str::DRAWINGML_MAIN_NS, "graphic") {
                        if empty {
                            return Err(crate::xml::xml_error("empty drawing graphic"));
                        }
                        let parts = self.graphic.set_attributes(
                            reader,
                            child,
                            drawing_relationships,
                            session,
                            provider,
                        )?;
                        self.set_source_parts((!parts.is_empty()).then_some(parts));
                    } else if name.is(
                        crate::helper::const_str::SHEET_DRAWING_NS,
                        "nvGraphicFramePr",
                    ) {
                        if empty {
                            return Err(crate::xml::xml_error(
                                "empty nonvisual graphic frame properties",
                            ));
                        }
                        self.non_visual_graphic_frame_properties
                            .set_attributes(reader, child, session)?;
                    } else if name.is(crate::helper::const_str::SHEET_DRAWING_NS, "xfrm") {
                        self.transform
                            .read_with_session(reader, child, empty, Some(session))?;
                    } else {
                        return Err(crate::xml::xml_error("unsupported graphic frame child"));
                    }
                }
                Event::End(child) if child.name().as_ref() == end.as_slice() => {
                    if seen.len() != 3 {
                        return Err(crate::xml::xml_error(
                            "missing required graphic frame child",
                        ));
                    }
                    return Ok(());
                }
                Event::Eof => return Err(crate::xml::xml_error("unterminated graphic frame")),
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
        // xdr:graphicFrame
        write_start_tag(
            writer,
            "xdr:graphicFrame",
            vec![("macro", self.r#macro.value_str()).into()],
            false,
        );

        // xdr:nvGraphicFramePr
        self.non_visual_graphic_frame_properties
            .write_to(writer, rel_list, ole_id)?;

        // xdr:xfrm
        self.transform.write_to(writer);

        // a:graphic
        Graphic::write_to(writer, rel_list);

        write_end_tag(writer, "xdr:graphicFrame");
        Ok(())
    }
}
impl AdjustmentCoordinateWithSheet for GraphicFrame {
    #[inline]
    fn adjustment_insert_coordinate_with_sheet(
        &mut self,
        sheet_name: &str,
        root_col_num: u32,
        offset_col_num: u32,
        root_row_num: u32,
        offset_row_num: u32,
    ) {
        self.graphic.adjustment_insert_coordinate_with_sheet(
            sheet_name,
            root_col_num,
            offset_col_num,
            root_row_num,
            offset_row_num,
        );
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
        self.graphic.adjustment_remove_coordinate_with_sheet(
            sheet_name,
            root_col_num,
            offset_col_num,
            root_row_num,
            offset_row_num,
        );
    }
}
