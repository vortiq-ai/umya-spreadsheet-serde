// c:upDownBars
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use super::{
    ChartLines,
    ExtensionList,
    GapWidth,
    ShapeProperties,
    group_lines::{
        read_chart_lines,
        store_once,
    },
    numeric_data::children,
};
use crate::{
    reader::{
        driver::local_name,
        xlsx::SourceCacheSession,
    },
    structs::Workbook,
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

/// `c:upBars` and `c:downBars` (`CT_UpDownBar`) hold exactly what a chart
/// line holds: an optional shape-properties child.
pub type UpDownBar = ChartLines;

/// `CT_UpDownBars`: the bars between the first and last series of a line or
/// stock group.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct UpDownBars {
    gap_width:  Option<GapWidth>,
    up_bars:    Option<UpDownBar>,
    down_bars:  Option<UpDownBar>,
    extensions: Option<Box<ExtensionList>>,
}

impl UpDownBars {
    #[must_use]
    pub fn gap_width(&self) -> Option<&GapWidth> {
        self.gap_width.as_ref()
    }

    pub fn gap_width_mut(&mut self) -> Option<&mut GapWidth> {
        self.gap_width.as_mut()
    }

    pub fn set_gap_width(&mut self, value: Option<GapWidth>) -> &mut Self {
        self.gap_width = value;
        self
    }

    #[must_use]
    pub fn up_bars(&self) -> Option<&UpDownBar> {
        self.up_bars.as_ref()
    }

    pub fn up_bars_mut(&mut self) -> Option<&mut UpDownBar> {
        self.up_bars.as_mut()
    }

    pub fn set_up_bars(&mut self, value: Option<UpDownBar>) -> &mut Self {
        self.up_bars = value;
        self
    }

    #[must_use]
    pub fn down_bars(&self) -> Option<&UpDownBar> {
        self.down_bars.as_ref()
    }

    pub fn down_bars_mut(&mut self) -> Option<&mut UpDownBar> {
        self.down_bars.as_mut()
    }

    pub fn set_down_bars(&mut self, value: Option<UpDownBar>) -> &mut Self {
        self.down_bars = value;
        self
    }

    #[must_use]
    pub fn extensions(&self) -> Option<&ExtensionList> {
        self.extensions.as_deref()
    }

    pub fn extensions_mut(&mut self) -> Option<&mut ExtensionList> {
        self.extensions.as_deref_mut()
    }

    pub fn set_extensions(&mut self, value: Option<ExtensionList>) -> &mut Self {
        self.extensions = value.map(Box::new);
        self
    }

    pub(crate) fn restore_export_parts(&mut self, previous: &Self) -> usize {
        super::chart_metadata_restoration::replace_hidden_parts(
            &mut self.extensions,
            previous.extensions.as_ref(),
        )
    }

    pub(crate) fn shapes(&self) -> impl Iterator<Item = &ShapeProperties> {
        [&self.up_bars, &self.down_bars]
            .into_iter()
            .flatten()
            .filter_map(ChartLines::shape_properties)
    }

    pub(crate) fn parts_mut(&mut self) -> (Vec<&mut ShapeProperties>, Option<&mut ExtensionList>) {
        let shapes = [&mut self.up_bars, &mut self.down_bars]
            .into_iter()
            .flatten()
            .filter_map(ChartLines::shape_properties_mut)
            .collect();
        (shapes, self.extensions.as_deref_mut())
    }

    pub(crate) fn read<R: std::io::BufRead>(
        reader: &mut Reader<R>,
        e: &BytesStart,
        empty: bool,
        session: &SourceCacheSession,
    ) -> Result<Self, crate::XlsxError> {
        let _scope_guard = session.enter_child_scope(e)?;
        let mut result = Self::default();
        let mut extensions = None;
        children(
            reader,
            e,
            empty,
            |reader, e, empty| -> Result<bool, crate::XlsxError> {
                if !session.accepts_child_namespace(e, super::extension_registry::NS_C)? {
                    return Ok(false);
                }
                match local_name(e.name().into_inner()) {
                    b"gapWidth" => {
                        let mut value = GapWidth::default();
                        value.set_attributes(reader, e)?;
                        if !empty {
                            crate::reader::driver::skip_element(reader, e)?;
                        }
                        store_once(&mut result.gap_width, value, "c:gapWidth")?;
                    }
                    b"upBars" => {
                        let value = read_chart_lines(reader, e, empty, session, "c:upBars")?;
                        store_once(&mut result.up_bars, value, "c:upBars")?;
                    }
                    b"downBars" => {
                        let value = read_chart_lines(reader, e, empty, session, "c:downBars")?;
                        store_once(&mut result.down_bars, value, "c:downBars")?;
                    }
                    b"extLst" => {
                        let value = ExtensionList::read(reader, e, empty, session)?;
                        store_once(&mut extensions, Box::new(value), "c:extLst")?;
                    }
                    _ => return Ok(false),
                }
                Ok(true)
            },
        )?;
        result.extensions = extensions;
        Ok(result)
    }

    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        wb: &Workbook,
    ) -> Result<(), crate::XlsxError> {
        write_start_tag(writer, "c:upDownBars", vec![], false);
        if let Some(value) = &self.gap_width {
            value.write_to(writer);
        }
        if let Some(value) = &self.up_bars {
            value.write_to_as(writer, "c:upBars")?;
        }
        if let Some(value) = &self.down_bars {
            value.write_to_as(writer, "c:downBars")?;
        }
        if let Some(ext) = &self.extensions {
            ext.write_for_owner(writer, Some(wb), "upDownBars", None)?;
        }
        write_end_tag(writer, "c:upDownBars");
        Ok(())
    }
}
