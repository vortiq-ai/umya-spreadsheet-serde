// c:doughnutChart
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
    DataLabels,
    FirstSliceAngle,
    HoleSize,
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
pub struct DoughnutChart {
    vary_colors:            VaryColors,
    area_chart_series_list: AreaChartSeriesList,
    data_labels:            DataLabels,
    first_slice_angle:      FirstSliceAngle,
    hole_size:              HoleSize,
    #[cfg_attr(feature = "serde", serde(default))]
    extensions:             Option<Box<super::ExtensionList>>,
}

impl DoughnutChart {
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

    pub fn set_vary_colors(&mut self, value: VaryColors) -> &mut DoughnutChart {
        self.vary_colors = value;
        self
    }

    #[must_use]
    pub fn area_chart_series_list(&self) -> &AreaChartSeriesList {
        &self.area_chart_series_list
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use vaarea_chart_series_listl()")]
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

    pub fn set_data_labels(&mut self, value: DataLabels) -> &mut DoughnutChart {
        self.data_labels = value;
        self.data_labels.mark_owner_present();
        self
    }

    #[must_use]
    pub fn first_slice_angle(&self) -> &FirstSliceAngle {
        &self.first_slice_angle
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use first_slice_angle()")]
    pub fn get_first_slice_angle(&self) -> &FirstSliceAngle {
        self.first_slice_angle()
    }

    pub fn first_slice_angle_mut(&mut self) -> &mut FirstSliceAngle {
        &mut self.first_slice_angle
    }

    #[deprecated(since = "3.0.0", note = "Use first_slice_angle_mut()")]
    pub fn get_first_slice_angle_mut(&mut self) -> &mut FirstSliceAngle {
        self.first_slice_angle_mut()
    }

    pub fn set_first_slice_angle(&mut self, value: FirstSliceAngle) -> &mut DoughnutChart {
        self.first_slice_angle = value;
        self
    }

    #[must_use]
    pub fn hole_size(&self) -> &HoleSize {
        &self.hole_size
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use hole_size()")]
    pub fn get_hole_size(&self) -> &HoleSize {
        self.hole_size()
    }

    pub fn hole_size_mut(&mut self) -> &mut HoleSize {
        &mut self.hole_size
    }

    #[deprecated(since = "3.0.0", note = "Use hole_size_mut()")]
    pub fn get_hole_size_mut(&mut self) -> &mut HoleSize {
        self.hole_size_mut()
    }

    pub fn set_hole_size(&mut self, value: HoleSize) -> &mut DoughnutChart {
        self.hole_size = value;
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
                b"varyColors" => {
                    self.vary_colors.set_attributes(reader, e);
                }
                b"firstSliceAng" => {
                    self.first_slice_angle.set_attributes(reader, e);
                }
                b"holeSize" => {
                    self.hole_size.set_attributes(reader, e);
                }
                _ => (),
            },
            Event::End(ref e) => {
                if local_name(e.name().into_inner()) == b"doughnutChart" {
                    return;
                }
            },
            Event::Eof => panic!("Error: Could not find {} end element", "c:doughnutChart")
        );
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>, wb: &Workbook) {
        // c:doughnutChart
        write_start_tag(writer, "c:doughnutChart", vec![], false);

        // c:varyColors
        self.vary_colors.write_to(writer);

        // c:ser
        for v in self.area_chart_series_list.area_chart_series() {
            v.write_to(writer, wb, SeriesFamily::Pie);
        }

        // c:dLbls
        self.data_labels.write_to(writer, wb);

        // c:firstSliceAng
        self.first_slice_angle.write_to(writer);

        // c:holeSize
        self.hole_size.write_to(writer);

        if let Some(ext) = self.extensions() {
            ext.write_for_owner(
                writer,
                Some(wb),
                "doughnutChart",
                Some(SeriesFamily::Pie),
            );
        }
        write_end_tag(writer, "c:doughnutChart");
    }
}
