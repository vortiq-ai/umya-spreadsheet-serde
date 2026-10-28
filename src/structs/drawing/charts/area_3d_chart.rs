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
    DataLabels,
    Grouping,
    SeriesFamily,
    VaryColors,
};
use crate::{
    reader::driver::{
        local_name,
        skip_element,
        xml_read_loop,
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
    axis_id:                Vec<AxisId>,
    #[cfg_attr(feature = "serde", serde(default))]
    extensions:             Option<Box<super::ExtensionList>>,
}

impl Area3DChart {
    pub(crate) fn labels_for_roles(&self) -> impl Iterator<Item = &DataLabels> {
        self.data_labels.as_deref().into_iter()
    }

    pub(crate) fn labels_for_roles_mut(&mut self) -> impl Iterator<Item = &mut DataLabels> {
        self.data_labels.as_deref_mut().into_iter()
    }

    pub(crate) fn formulas_mut(&mut self) -> Vec<&mut super::Formula> {
        let mut result = Vec::new();
        for series in self.area_chart_series_list.area_chart_series_mut() {
            result.extend(series.formula_mut());
        }
        if let Some(v) = &mut self.data_labels {
            result.extend(v.formulas_mut());
        }
        if let Some(v) = &mut self.extensions {
            result.extend(v.formulas_mut());
        }
        result
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
    ) {
        let _scope_guard = session.enter_child_scope(owner);
        xml_read_loop!(
            reader,
            ref n @ (Event::Empty(ref e) | Event::Start(ref e)) => {
                let is_empty = matches!(n, Event::Empty(_));
                match if session.accepts_child_namespace(e,super::extension_registry::NS_C){local_name(e.name().into_inner())}else{b""} {
                    b"extLst"=>{self.set_extensions(Some(super::ExtensionList::read(reader,e,is_empty,session)));},
                    b"ser" => {
                        let mut obj = AreaChartSeries::default();
                        obj.set_attributes(reader, e, session);
                        self.area_chart_series_list_mut()
                            .add_area_chart_series(obj);
                        }
                    b"dLbls" => {
                        let mut obj = DataLabels::default();
                        if is_empty {
                            obj.mark_empty_import();
                        } else {
                            obj.set_attributes(reader, e, session);
                        }
                        self.set_data_labels(obj);
                    }
                    b"grouping" => {
                        self.grouping.set_attributes(reader, e);
                    }
                    b"varyColors" => {
                        self.vary_colors.set_attributes(reader, e);
                    }
                    b"axId" => {
                        let mut obj = AxisId::default();
                        obj.set_attributes(reader, e);
                        self.add_axis_id(obj);
                    }
                    _ => {
                        if !is_empty {
                            skip_element(reader, e);
                        }
                    }
                }
            },
            Event::End(ref e) => {
                if local_name(e.name().into_inner()) == b"area3DChart" {
                    return;
                }
            },
            Event::Eof => panic!("Error: Could not find {} end element", "c:area3DChart")
        );
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>, wb: &Workbook) {
        // c:area3DChart
        write_start_tag(writer, "c:area3DChart", vec![], false);

        // c:grouping
        self.grouping.write_to(writer);

        // c:varyColors
        self.vary_colors.write_to(writer);

        // c:ser
        for v in self.area_chart_series_list.area_chart_series() {
            v.write_to(writer, wb, SeriesFamily::Area);
        }

        // c:dLbls
        if let Some(v) = &self.data_labels {
            v.write_to_present(writer, wb);
        }

        // c:axId
        for v in &self.axis_id {
            v.write_to(writer);
        }

        if let Some(ext) = self.extensions() {
            ext.write_for_owner(writer, Some(wb), "area3DChart", Some(SeriesFamily::Area));
        }
        write_end_tag(writer, "c:area3DChart");
    }
}
