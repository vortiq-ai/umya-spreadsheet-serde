// a:graphic
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::{
        BytesStart,
        Event,
    },
};

use super::GraphicData;
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
pub struct Graphic {
    graphic_data: GraphicData,
}

impl Graphic {
    #[inline]
    #[must_use]
    pub fn graphic_data(&self) -> &GraphicData {
        &self.graphic_data
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use graphic_data()")]
    pub fn get_graphic_data(&self) -> &GraphicData {
        self.graphic_data()
    }

    #[inline]
    pub fn graphic_data_mut(&mut self) -> &mut GraphicData {
        &mut self.graphic_data
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use graphic_data_mut()")]
    pub fn get_graphic_data_mut(&mut self) -> &mut GraphicData {
        self.graphic_data_mut()
    }

    #[inline]
    pub fn set_graphic_data(&mut self, value: GraphicData) -> &mut Self {
        self.graphic_data = value;
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
        let parent = session.current_namespace_scope().unwrap_or_default();
        let scope = parent
            .child(owner)
            .map_err(super::spreadsheet::drawing_error)?;
        let _guard = session.enter_child_scope(owner)?;
        let end = owner.name().as_ref().to_vec();
        let mut source_parts = Vec::new();
        let mut seen = false;
        let mut buffer = Vec::new();
        loop {
            match reader.read_event_into(&mut buffer)? {
                ref event @ (Event::Start(ref child) | Event::Empty(ref child)) => {
                    let child_scope = scope
                        .child(child)
                        .map_err(super::spreadsheet::drawing_error)?;
                    let name = child_scope
                        .resolve(child.name().as_ref(), false)
                        .map_err(super::spreadsheet::drawing_error)?;
                    if !name.is(crate::helper::const_str::DRAWINGML_MAIN_NS, "graphicData") || seen
                    {
                        return Err(super::spreadsheet::drawing_error(
                            "foreign or duplicate graphic payload",
                        ));
                    }
                    seen = true;
                    if matches!(event, Event::Empty(_)) {
                        return Err(super::spreadsheet::drawing_error("empty graphic payload"));
                    }
                    source_parts.extend(self.graphic_data.set_attributes(
                        reader,
                        child,
                        drawing_relationships,
                        session,
                        provider,
                    )?);
                }
                Event::End(child) if child.name().as_ref() == end.as_slice() => {
                    if !seen {
                        return Err(super::spreadsheet::drawing_error("missing graphic payload"));
                    }
                    return Ok(source_parts);
                }
                Event::Eof => {
                    return Err(super::spreadsheet::drawing_error("unterminated graphic"));
                }
                ref event @ (Event::Text(_) | Event::CData(_) | Event::GeneralRef(_)) => {
                    if !crate::reader::driver::decode_xml_character_event(event)?
                        .is_some_and(|text| crate::xml::trim_xml_whitespace(&text).is_empty())
                    {
                        return Err(super::spreadsheet::drawing_error(
                            "non-whitespace graphic content",
                        ));
                    }
                }
                Event::Comment(_) | Event::PI(_) => (),
                _ => return Err(super::spreadsheet::drawing_error("invalid graphic event")),
            }
            buffer.clear();
        }
    }

    pub(crate) fn write_to(
        writer: &mut Writer<Cursor<Vec<u8>>>,
        rel_list: &mut Vec<(String, String)>,
    ) {
        // a:graphic
        write_start_tag(writer, "a:graphic", vec![], false);

        // a:graphicData
        GraphicData::write_to(writer, rel_list);

        write_end_tag(writer, "a:graphic");
    }
}
impl AdjustmentCoordinateWithSheet for Graphic {
    #[inline]
    fn adjustment_insert_coordinate_with_sheet(
        &mut self,
        sheet_name: &str,
        root_col_num: u32,
        offset_col_num: u32,
        root_row_num: u32,
        offset_row_num: u32,
    ) {
        self.graphic_data.adjustment_insert_coordinate_with_sheet(
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
        self.graphic_data.adjustment_remove_coordinate_with_sheet(
            sheet_name,
            root_col_num,
            offset_col_num,
            root_row_num,
            offset_row_num,
        );
    }
}
