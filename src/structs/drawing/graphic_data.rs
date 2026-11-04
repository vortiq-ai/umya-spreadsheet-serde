// *:graphicData
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
    charts::{
        ChartSpace,
        NamespaceScope,
    },
    spreadsheet::drawing_error,
};
use crate::{
    helper::const_str::{
        DRAWINGML_CHART_NS,
        REL_OFC_NS,
    },
    reader::{
        driver::{
            skip_element,
            try_xml_read_loop,
        },
        xlsx::chart,
    },
    structs::raw::RawRelationships,
    traits::AdjustmentCoordinateWithSheet,
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct GraphicData {
    chart_space: ChartSpace,
}

impl GraphicData {
    #[inline]
    #[must_use]
    pub fn chart_space(&self) -> &ChartSpace {
        &self.chart_space
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use chart_space()")]
    pub fn get_chart_space(&self) -> &ChartSpace {
        self.chart_space()
    }

    #[inline]
    pub fn chart_space_mut(&mut self) -> &mut ChartSpace {
        &mut self.chart_space
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use chart_space_mut()")]
    pub fn get_chart_space_mut(&mut self) -> &mut ChartSpace {
        self.chart_space_mut()
    }

    #[inline]
    pub fn set_chart_space(&mut self, value: ChartSpace) -> &GraphicData {
        self.chart_space = value;
        self
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        owner: &BytesStart,
        drawing_relationships: Option<&RawRelationships>,
        session: &crate::reader::xlsx::SourceCacheSession,
        provider: &mut dyn crate::reader::xlsx::ChartPartProvider,
    ) -> Result<Vec<String>, crate::XlsxError> {
        let mut source_parts = Vec::new();
        let _guard = session.enter_child_scope(owner)?;
        let scope = NamespaceScope::default()
            .child(owner)
            .map_err(drawing_error)?;
        let end = owner.name().as_ref().to_vec();
        try_xml_read_loop!(
            reader,
            ref event @ (Event::Start(ref element) | Event::Empty(ref element)) => {
                let empty = matches!(event, Event::Empty(_));
                let child_scope = scope.child(element).map_err(drawing_error)?;
                let name = child_scope.resolve(element.name().as_ref(), false)
                    .map_err(drawing_error)?;
                if name.is(DRAWINGML_CHART_NS, "chart") {
                    let mut chart_id = None;
                    for attribute in element.attributes() {
                        let attribute = attribute
                            .map_err(|error| drawing_error(error.to_string()))?;
                        if attribute.key.as_ref() == b"xmlns" || attribute.key.as_ref().starts_with(b"xmlns:") { continue; }
                        let name = child_scope.resolve(attribute.key.as_ref(), true)
                            .map_err(drawing_error)?;
                        if name.is(REL_OFC_NS, "id") {
                            if chart_id.is_some() {
                                return Err(drawing_error("duplicate chart relationship id"));
                            }
                            chart_id = Some(attribute
                                .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                                .map_err(|error| drawing_error(error.to_string()))?.into_owned());
                        }
                    }
                    let chart_id = chart_id
                        .ok_or_else(|| drawing_error("chart relationship id required"))?;
                    let relationship = drawing_relationships
                        .and_then(|list| list.find_relationship_by_rid(&chart_id))
                        .ok_or_else(|| drawing_error("chart relationship not found"))?;
                    chart::read(relationship.raw_file(), &mut self.chart_space, session, provider)?;
                    source_parts.push(relationship.raw_file().file_target().to_owned());
                    if !empty { crate::reader::driver::consume_empty_element(reader, element)?; }
                } else if !empty { skip_element(reader, element)?; }
            },
            Event::End(ref element) => {
                if element.name().as_ref() == end { return Ok(source_parts); }
            },
            Event::Eof => return Err(drawing_error("unterminated graphicData"))
        );
    }

    pub(crate) fn write_to(
        writer: &mut Writer<Cursor<Vec<u8>>>,
        rel_list: &mut Vec<(String, String)>,
    ) {
        // a:graphicData
        write_start_tag(
            writer,
            "a:graphicData",
            vec![("uri", DRAWINGML_CHART_NS).into()],
            false,
        );

        // c:chart
        rel_list.push((String::from("CHART"), String::new()));
        write_start_tag(
            writer,
            "c:chart",
            vec![
                ("xmlns:c", DRAWINGML_CHART_NS).into(),
                ("xmlns:r", REL_OFC_NS).into(),
                ("r:id", format!("rId{}", rel_list.len()).as_str()).into(),
            ],
            true,
        );

        write_end_tag(writer, "a:graphicData");
    }
}
impl AdjustmentCoordinateWithSheet for GraphicData {
    #[inline]
    fn adjustment_insert_coordinate_with_sheet(
        &mut self,
        sheet_name: &str,
        root_col_num: u32,
        offset_col_num: u32,
        root_row_num: u32,
        offset_row_num: u32,
    ) {
        self.chart_space.adjustment_insert_coordinate_with_sheet(
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
        self.chart_space.adjustment_remove_coordinate_with_sheet(
            sheet_name,
            root_col_num,
            offset_col_num,
            root_row_num,
            offset_row_num,
        );
    }
}
