// c:area3DChart
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
    GapDepth,
    Grouping,
    SeriesFamily,
    VaryColors,
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

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct Area3DChart {
    grouping:               Grouping,
    vary_colors:            VaryColors,
    area_chart_series_list: AreaChartSeriesList,
    data_labels:            Option<Box<DataLabels>>,
    #[cfg_attr(feature = "serde", serde(default))]
    drop_lines:             Option<ChartLines>,
    axis_id:                Vec<AxisId>,
    #[cfg_attr(feature = "serde", serde(default))]
    gap_depth:              Option<GapDepth>,
    #[cfg_attr(feature = "serde", serde(default))]
    extensions:             Option<Box<super::ExtensionList>>,
}

impl Area3DChart {
    optional_group_part!(drop_lines, ChartLines, drop_lines_mut, set_drop_lines);

    pub(crate) fn restore_export_parts(&mut self, previous: &Self) -> usize {
        super::chart_metadata_restoration::replace_hidden_parts(
            &mut self.extensions,
            previous.extensions.as_ref(),
        )
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
            self.data_labels.as_deref(),
            self.extensions.as_deref(),
            self.lines_for_roles(),
        )
    }

    pub(crate) fn lines_for_roles(&self) -> GroupLines<'_> {
        GroupLines {
            drop_lines:   self.drop_lines.as_ref(),
            hi_low_lines: None,
            up_down_bars: None,
            band_formats: &[],
        }
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
            self.data_labels.as_deref_mut(),
            self.extensions.as_deref_mut(),
            GroupLinesMut {
                drop_lines:   self.drop_lines.as_mut(),
                hi_low_lines: None,
                up_down_bars: None,
                band_formats: &mut [],
            },
        )
    }

    pub(crate) fn labels_for_roles(&self) -> impl Iterator<Item = &DataLabels> {
        self.data_labels.as_deref().into_iter()
    }

    pub(crate) fn labels_for_roles_mut(&mut self) -> impl Iterator<Item = &mut DataLabels> {
        self.data_labels.as_deref_mut().into_iter()
    }

    /// The gap between series along the depth of a 3-D chart.
    #[must_use]
    pub fn gap_depth(&self) -> Option<&GapDepth> {
        self.gap_depth.as_ref()
    }

    pub fn gap_depth_mut(&mut self) -> Option<&mut GapDepth> {
        self.gap_depth.as_mut()
    }

    pub fn set_gap_depth(&mut self, value: GapDepth) -> &mut Self {
        self.gap_depth = Some(value);
        self
    }

    pub fn remove_gap_depth(&mut self) -> &mut Self {
        self.gap_depth = None;
        self
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
    pub fn grouping(&self) -> &Grouping {
        &self.grouping
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use grouping()")]
    pub fn get_grouping(&self) -> &Grouping {
        self.grouping()
    }

    pub fn grouping_mut(&mut self) -> &mut Grouping {
        &mut self.grouping
    }

    #[deprecated(since = "3.0.0", note = "Use grouping_mut()")]
    pub fn get_grouping_mut(&mut self) -> &mut Grouping {
        self.grouping_mut()
    }

    pub fn set_grouping(&mut self, value: Grouping) -> &mut Self {
        self.grouping = value;
        self
    }

    #[must_use]
    pub fn vary_colors(&self) -> &VaryColors {
        &self.vary_colors
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use vary_colors()")]
    pub fn get_vary_colors(&self) -> &VaryColors {
        self.vary_colors()
    }

    pub fn vary_colors_mut(&mut self) -> &mut VaryColors {
        &mut self.vary_colors
    }

    #[deprecated(since = "3.0.0", note = "Use vary_colors_mut()")]
    pub fn get_vary_colors_mut(&mut self) -> &mut VaryColors {
        self.vary_colors_mut()
    }

    pub fn set_vary_colors(&mut self, value: VaryColors) -> &mut Self {
        self.vary_colors = value;
        self
    }

    #[must_use]
    pub fn area_chart_series_list(&self) -> &AreaChartSeriesList {
        &self.area_chart_series_list
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use area_chart_series_list()")]
    pub fn get_area_chart_series_list(&self) -> &AreaChartSeriesList {
        self.area_chart_series_list()
    }

    pub fn area_chart_series_list_mut(&mut self) -> &mut AreaChartSeriesList {
        &mut self.area_chart_series_list
    }

    #[deprecated(since = "3.0.0", note = "Use area_chart_series_list_mut()")]
    pub fn get_area_chart_series_list_mut(&mut self) -> &mut AreaChartSeriesList {
        self.area_chart_series_list_mut()
    }

    pub fn set_area_chart_series_list(&mut self, value: AreaChartSeriesList) -> &mut Self {
        self.area_chart_series_list = value;
        self
    }

    #[must_use]
    pub fn data_labels(&self) -> Option<&DataLabels> {
        self.data_labels.as_deref()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use data_labels()")]
    pub fn get_data_labels(&self) -> Option<&DataLabels> {
        self.data_labels()
    }

    pub fn data_labels_mut(&mut self) -> Option<&mut DataLabels> {
        if let Some(labels) = &mut self.data_labels {
            labels.mark_owner_present();
        }
        self.data_labels.as_deref_mut()
    }

    #[deprecated(since = "3.0.0", note = "Use data_labels_mut()")]
    pub fn get_data_labels_mut(&mut self) -> Option<&mut DataLabels> {
        self.data_labels_mut()
    }

    pub fn set_data_labels(&mut self, value: DataLabels) -> &mut Self {
        self.data_labels = Some(Box::new(value));
        if let Some(labels) = &mut self.data_labels {
            labels.mark_owner_present();
        }
        self
    }

    #[must_use]
    pub fn axis_id(&self) -> &[AxisId] {
        &self.axis_id
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use axis_id()")]
    pub fn get_axis_id(&self) -> &[AxisId] {
        self.axis_id()
    }

    pub fn axis_id_mut(&mut self) -> &mut Vec<AxisId> {
        &mut self.axis_id
    }

    #[deprecated(since = "3.0.0", note = "Use axis_id_mut()")]
    pub fn get_axis_id_mut(&mut self) -> &mut Vec<AxisId> {
        self.axis_id_mut()
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
                        let mut obj = DataLabels::default();
                        if is_empty {
                            obj.mark_empty_import();
                        } else {
                            obj.set_attributes(reader, e, session)?;
                        }
                        self.set_data_labels(obj);
                    }
                    b"dropLines" => {
                        let value = read_chart_lines(reader, e, is_empty, session, "c:dropLines")?;
                        store_once(&mut self.drop_lines, value, "c:dropLines")?;
                    }
                    b"grouping" => {
                        self.grouping.set_attributes(reader, e)?;
                    }
                    b"varyColors" => {
                        self.vary_colors.set_attributes(reader, e)?;
                    }
                    b"gapDepth" => {
                        let mut obj = GapDepth::default();
                        obj.set_attributes(reader, e)?;
                        self.set_gap_depth(obj);
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
            Event::Eof => return Err(crate::xml::xml_error(format!("Error: Could not find {} end element", "c:area3DChart")))
        );
    }

    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        wb: &Workbook,
    ) -> Result<(), crate::XlsxError> {
        // c:area3DChart
        write_start_tag(writer, "c:area3DChart", vec![], false);

        // c:grouping
        self.grouping.write_to(writer);

        // c:varyColors
        self.vary_colors.write_to(writer);

        // c:ser
        for v in self.area_chart_series_list.area_chart_series() {
            v.write_to(writer, wb, SeriesFamily::Area)?;
        }

        // c:dLbls
        if let Some(v) = &self.data_labels {
            v.write_to_present(writer, wb)?;
        }

        // c:dropLines
        if let Some(v) = &self.drop_lines {
            v.write_to_as(writer, "c:dropLines")?;
        }

        // c:gapDepth
        if let Some(v) = &self.gap_depth {
            v.write_to(writer);
        }

        // c:axId
        for v in &self.axis_id {
            v.write_to(writer);
        }

        if let Some(ext) = self.extensions() {
            ext.write_for_owner(writer, Some(wb), "area3DChart", Some(SeriesFamily::Area))?;
        }
        write_end_tag(writer, "c:area3DChart");

        Ok(())
    }
}
