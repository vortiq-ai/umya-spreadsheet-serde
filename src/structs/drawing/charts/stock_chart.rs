// c:stockChart
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
    AreaChartSeries,
    AreaChartSeriesList,
    AxisId,
    ChartLines,
    DataLabels,
    SeriesFamily,
    UpDownBars,
    group_lines::{
        GroupLines,
        GroupLinesMut,
        optional_group_part,
        read_chart_lines,
        store_once,
    },
};
use crate::{
    reader::driver::{
        local_name,
        skip_element,
        try_xml_read_loop,
    },
    structs::Workbook,
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

/// `CT_StockChart`: high-low(-close) or open-high-low-close series drawn as
/// lines, with optional high-low lines and up/down bars.
///
/// Excel stores the volume stock types (`xlStockVHLC`, `xlStockVOHLC`) as a
/// `c:barChart` followed by a `c:stockChart` on secondary axes; both are
/// plot groups of one chart.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct StockChart {
    area_chart_series_list: AreaChartSeriesList,
    data_labels:            DataLabels,
    #[cfg_attr(feature = "serde", serde(default))]
    drop_lines:             Option<ChartLines>,
    #[cfg_attr(feature = "serde", serde(default))]
    hi_low_lines:           Option<ChartLines>,
    #[cfg_attr(feature = "serde", serde(default))]
    up_down_bars:           Option<UpDownBars>,
    axis_id:                Vec<AxisId>,
    #[cfg_attr(feature = "serde", serde(default))]
    extensions:             Option<Box<super::ExtensionList>>,
}

impl StockChart {
    optional_group_part!(drop_lines, ChartLines, drop_lines_mut, set_drop_lines);

    optional_group_part!(hi_low_lines, ChartLines, hi_low_lines_mut, set_hi_low_lines);

    optional_group_part!(up_down_bars, UpDownBars, up_down_bars_mut, set_up_down_bars);

    pub(crate) fn restore_export_parts(&mut self, previous: &Self) -> usize {
        super::chart_metadata_restoration::replace_hidden_parts(
            &mut self.extensions,
            previous.extensions.as_ref(),
        )
    }

    pub(crate) fn lines_for_roles(&self) -> GroupLines<'_> {
        GroupLines {
            drop_lines:   self.drop_lines.as_ref(),
            hi_low_lines: self.hi_low_lines.as_ref(),
            up_down_bars: self.up_down_bars.as_ref(),
            band_formats: &[],
        }
    }

    pub(crate) fn reference_group_parts(
        &self,
    ) -> (
        &AreaChartSeriesList,
        Option<&DataLabels>,
        Option<&super::ExtensionList>,
        GroupLines<'_>,
    ) {
        (
            &self.area_chart_series_list,
            Some(&self.data_labels),
            self.extensions.as_deref(),
            self.lines_for_roles(),
        )
    }

    pub(crate) fn projection_group_parts_mut(
        &mut self,
    ) -> (
        &mut AreaChartSeriesList,
        Option<&mut DataLabels>,
        Option<&mut super::ExtensionList>,
        GroupLinesMut<'_>,
    ) {
        self.extensions = None;
        if let Some(bars) = &mut self.up_down_bars {
            bars.set_extensions(None);
        }
        self.reference_group_parts_mut()
    }

    pub(crate) fn reference_group_parts_mut(
        &mut self,
    ) -> (
        &mut AreaChartSeriesList,
        Option<&mut DataLabels>,
        Option<&mut super::ExtensionList>,
        GroupLinesMut<'_>,
    ) {
        (
            &mut self.area_chart_series_list,
            Some(&mut self.data_labels),
            self.extensions.as_deref_mut(),
            GroupLinesMut {
                drop_lines:   self.drop_lines.as_mut(),
                hi_low_lines: self.hi_low_lines.as_mut(),
                up_down_bars: self.up_down_bars.as_mut(),
                band_formats: &mut [],
            },
        )
    }

    pub(crate) fn labels_for_roles(&self) -> impl Iterator<Item = &DataLabels> {
        std::iter::once(&self.data_labels)
    }

    pub(crate) fn labels_for_roles_mut(&mut self) -> impl Iterator<Item = &mut DataLabels> {
        std::iter::once(&mut self.data_labels)
    }

    #[must_use]
    pub fn extensions(&self) -> Option<&super::ExtensionList> {
        self.extensions.as_deref()
    }

    pub fn extensions_mut(&mut self) -> Option<&mut super::ExtensionList> {
        self.extensions.as_deref_mut()
    }

    pub fn set_extensions(&mut self, value: Option<super::ExtensionList>) -> &mut Self {
        self.extensions = value.map(Box::new);
        self
    }

    #[must_use]
    pub fn area_chart_series_list(&self) -> &AreaChartSeriesList {
        &self.area_chart_series_list
    }

    pub fn area_chart_series_list_mut(&mut self) -> &mut AreaChartSeriesList {
        &mut self.area_chart_series_list
    }

    pub fn set_area_chart_series_list(&mut self, value: AreaChartSeriesList) -> &mut Self {
        self.area_chart_series_list = value;
        self
    }

    #[must_use]
    pub fn data_labels(&self) -> &DataLabels {
        &self.data_labels
    }

    pub fn data_labels_mut(&mut self) -> &mut DataLabels {
        self.data_labels.mark_owner_present();
        &mut self.data_labels
    }

    pub fn set_data_labels(&mut self, value: DataLabels) -> &mut Self {
        self.data_labels = value;
        self.data_labels.mark_owner_present();
        self
    }

    #[must_use]
    pub fn axis_id(&self) -> &[AxisId] {
        &self.axis_id
    }

    pub fn axis_id_mut(&mut self) -> &mut Vec<AxisId> {
        &mut self.axis_id
    }

    pub fn set_axis_id(&mut self, value: impl Into<Vec<AxisId>>) -> &mut Self {
        self.axis_id = value.into();
        self
    }

    pub fn add_axis_id(&mut self, value: AxisId) -> &mut Self {
        self.axis_id.push(value);
        self
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        owner: &BytesStart,
        session: &crate::reader::xlsx::SourceCacheSession,
    ) -> Result<(), crate::XlsxError> {
        let owner_end = owner.name().as_ref().to_vec();
        let _scope_guard = session.enter_child_scope(owner)?;
        self.data_labels.mark_group_absent();
        try_xml_read_loop!(
            reader,
            ref n @ (Event::Empty(ref e) | Event::Start(ref e)) => {
                let is_empty = matches!(n, Event::Empty(_));
                match if session.accepts_child_namespace(e,super::extension_registry::NS_C)?{local_name(e.name().into_inner())}else{b""} {
                    b"extLst"=>{self.set_extensions(Some(super::ExtensionList::read(reader,e,is_empty,session)?));},
                    b"ser" => {
                        let mut obj = AreaChartSeries::default();
                        obj.set_attributes(reader, e, session)?;
                        self.area_chart_series_list_mut()
                            .add_area_chart_series(obj);
                    }
                    b"dLbls" => {
                        if is_empty {
                            self.data_labels.mark_empty_import();
                        } else {
                            self.data_labels.set_attributes(reader, e, session)?;
                        }
                    }
                    b"dropLines" => {
                        let value = read_chart_lines(reader, e, is_empty, session, "c:dropLines")?;
                        store_once(&mut self.drop_lines, value, "c:dropLines")?;
                    }
                    b"hiLowLines" => {
                        let value = read_chart_lines(reader, e, is_empty, session, "c:hiLowLines")?;
                        store_once(&mut self.hi_low_lines, value, "c:hiLowLines")?;
                    }
                    b"upDownBars" => {
                        let value = UpDownBars::read(reader, e, is_empty, session)?;
                        store_once(&mut self.up_down_bars, value, "c:upDownBars")?;
                    }
                    b"axId" => {
                        let mut obj = AxisId::default();
                        obj.set_attributes(reader, e)?;
                        self.add_axis_id(obj);
                    }
                    _ => {
                        if !is_empty {
                            skip_element(reader, e)?;
                        }
                    }
                }
            },
            Event::End(ref e) => {
                if e.name().as_ref() == owner_end {
                    return Ok(());
                }
            },
            Event::Eof => return Err(crate::xml::xml_error(format!("Error: Could not find {} end element", "c:stockChart")))
        );
    }

    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        wb: &Workbook,
    ) -> Result<(), crate::XlsxError> {
        // c:stockChart
        write_start_tag(writer, "c:stockChart", vec![], false);

        // c:ser
        for v in self.area_chart_series_list.area_chart_series() {
            v.write_to(writer, wb, SeriesFamily::Line)?;
        }

        // c:dLbls
        self.data_labels.write_to(writer, wb)?;

        // c:dropLines
        if let Some(v) = &self.drop_lines {
            v.write_to_as(writer, "c:dropLines")?;
        }

        // c:hiLowLines
        if let Some(v) = &self.hi_low_lines {
            v.write_to_as(writer, "c:hiLowLines")?;
        }

        // c:upDownBars
        if let Some(v) = &self.up_down_bars {
            v.write_to(writer, wb)?;
        }

        // c:axId
        for v in &self.axis_id {
            v.write_to(writer);
        }

        if let Some(ext) = self.extensions() {
            ext.write_for_owner(writer, Some(wb), "stockChart", Some(SeriesFamily::Line))?;
        }
        write_end_tag(writer, "c:stockChart");

        Ok(())
    }
}
