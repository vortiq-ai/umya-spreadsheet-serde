use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::{
        BytesStart,
        Event,
    },
};

// lineChart
use super::AreaChartSeries;
use super::{
    AreaChartSeriesList,
    AxisId,
    DataLabels,
    Grouping,
    SeriesFamily,
    ShowMarker,
    Smooth,
    VaryColors,
};
use crate::{
    reader::driver::{
        local_name,
        skip_element,
    },
    structs::Workbook,
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
    xml_read_loop,
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct LineChart {
    grouping:               Grouping,
    vary_colors:            VaryColors,
    area_chart_series_list: AreaChartSeriesList,
    data_labels:            DataLabels,
    show_marker:            ShowMarker,
    smooth:                 Smooth,
    axis_id:                Vec<AxisId>,
    #[cfg_attr(feature = "serde", serde(default))]
    extensions:             Option<Box<super::ExtensionList>>,
}

impl LineChart {
    pub(crate) fn reference_group_parts(&self) -> (
        &AreaChartSeriesList,
        Option<&DataLabels>,
        Option<&super::ExtensionList>,
    ) {
        (&self.area_chart_series_list, Some(&self.data_labels), self.extensions.as_deref())
    }

    pub(crate) fn reference_group_parts_mut(&mut self) -> (
        &mut AreaChartSeriesList,
        Option<&mut DataLabels>,
        Option<&mut super::ExtensionList>,
    ) {
        (&mut self.area_chart_series_list, Some(&mut self.data_labels), self.extensions.as_deref_mut())
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
    pub fn data_labels(&self) -> &DataLabels {
        &self.data_labels
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use data_labels()")]
    pub fn get_data_labels(&self) -> &DataLabels {
        self.data_labels()
    }

    pub fn data_labels_mut(&mut self) -> &mut DataLabels {
        self.data_labels.mark_owner_present();
        &mut self.data_labels
    }

    #[deprecated(since = "3.0.0", note = "Use data_labels_mut()")]
    pub fn get_data_labels_mut(&mut self) -> &mut DataLabels {
        self.data_labels_mut()
    }

    pub fn set_data_labels(&mut self, value: DataLabels) -> &mut Self {
        self.data_labels = value;
        self.data_labels.mark_owner_present();
        self
    }

    #[must_use]
    pub fn show_marker(&self) -> &ShowMarker {
        &self.show_marker
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use show_marker()")]
    pub fn get_show_marker(&self) -> &ShowMarker {
        self.show_marker()
    }

    pub fn show_marker_mut(&mut self) -> &mut ShowMarker {
        &mut self.show_marker
    }

    #[deprecated(since = "3.0.0", note = "Use show_marker_mut()")]
    pub fn get_show_marker_mut(&mut self) -> &mut ShowMarker {
        self.show_marker_mut()
    }

    pub fn set_show_marker(&mut self, value: ShowMarker) -> &mut Self {
        self.show_marker = value;
        self
    }

    #[must_use]
    pub fn smooth(&self) -> &Smooth {
        &self.smooth
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use smooth()")]
    pub fn get_smooth(&self) -> &Smooth {
        self.smooth()
    }

    pub fn smooth_mut(&mut self) -> &mut Smooth {
        &mut self.smooth
    }

    #[deprecated(since = "3.0.0", note = "Use smooth_mut()")]
    pub fn get_smooth_mut(&mut self) -> &mut Smooth {
        self.smooth_mut()
    }

    pub fn set_smooth(&mut self, value: Smooth) -> &mut Self {
        self.smooth = value;
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
        let _scope_guard=session.enter_child_scope(owner);
        self.data_labels.mark_group_absent();
        xml_read_loop!(
            reader,
            Event::Start(ref e) => match if session.accepts_child_namespace(e,super::extension_registry::NS_C){local_name(e.name().into_inner())}else{b""} {
                b"extLst"=>{self.set_extensions(Some(super::ExtensionList::read(reader,e,false,session)));},
                b"ser" => {
                    let mut obj = AreaChartSeries::default();
                    obj.set_attributes(reader, e, session);
                    self.area_chart_series_list_mut()
                        .add_area_chart_series(obj);
                }
                b"dLbls" => {
                    self.data_labels.set_attributes(reader, e, session);
                }
                _ => skip_element(reader, e),
            },
            Event::Empty(ref e) => match if session.accepts_child_namespace(e,super::extension_registry::NS_C){local_name(e.name().into_inner())}else{b""} {
                b"extLst"=>{self.set_extensions(Some(super::ExtensionList::read(reader,e,true,session)));},
                    b"dLbls" => self.data_labels.mark_empty_import(),
                b"grouping" => {
                    self.grouping.set_attributes(reader, e);
                }
                b"varyColors" => {
                    self.vary_colors.set_attributes(reader, e);
                }
                b"marker" => {
                    self.show_marker.set_attributes(reader, e);
                }
                b"smooth" => {
                    self.smooth.set_attributes(reader, e);
                }
                b"axId" => {
                    let mut obj = AxisId::default();
                    obj.set_attributes(reader, e);
                    self.add_axis_id(obj);
                }
                _ => (),
            },
            Event::End(ref e) => {
                if local_name(e.name().into_inner()) == b"lineChart" {
                    return;
                }
            },
            Event::Eof => panic!("Error: Could not find {} end element", "c:lineChart"),
        );
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>, wb: &Workbook) {
        // c:lineChart
        write_start_tag(writer, "c:lineChart", vec![], false);

        // c:grouping
        self.grouping.write_to(writer);

        // c:varyColors
        self.vary_colors.write_to(writer);

        // c:ser
        for v in self.area_chart_series_list.area_chart_series() {
            v.write_to(writer, wb, SeriesFamily::Line);
        }

        // c:dLbls
        self.data_labels.write_to(writer, wb);

        // c:marker
        self.show_marker.write_to(writer);

        // c:smooth
        self.smooth.write_to(writer);

        // c:axId
        for v in &self.axis_id {
            v.write_to(writer);
        }

        if let Some(ext) = self.extensions() {
            ext.write_for_owner(
                writer,
                Some(wb),
                "lineChart",
                Some(SeriesFamily::Line),
            );
        }
        write_end_tag(writer, "c:lineChart");
    }
}
