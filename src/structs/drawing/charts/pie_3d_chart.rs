// c:pie3DChart
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
pub struct Pie3DChart {
    vary_colors:            VaryColors,
    area_chart_series_list: AreaChartSeriesList,
    data_labels:            DataLabels,
    #[cfg_attr(feature = "serde", serde(default))]
    extensions:             Option<Box<super::ExtensionList>>,
}

impl Pie3DChart {
    pub(crate) fn labels_for_roles(&self) -> impl Iterator<Item = &DataLabels> {
        std::iter::once(&self.data_labels)
    }

    pub(crate) fn labels_for_roles_mut(&mut self) -> impl Iterator<Item = &mut DataLabels> {
        std::iter::once(&mut self.data_labels)
    }

    pub(crate) fn formulas_mut(&mut self) -> Vec<&mut super::Formula> {
        let mut result = Vec::new();
        for series in self.area_chart_series_list.area_chart_series_mut() {
            result.extend(series.formula_mut());
        }
        result.extend(self.data_labels.formulas_mut());
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

    pub fn set_vary_colors(&mut self, value: VaryColors) -> &mut Pie3DChart {
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

    pub fn set_data_labels(&mut self, value: DataLabels) -> &mut Pie3DChart {
        self.data_labels = value;
        self.data_labels.mark_owner_present();
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
            Event::Start(ref e) => {
                match if session.accepts_child_namespace(e,super::extension_registry::NS_C){local_name(e.name().0)}else{b""} {
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
                }
            },
            Event::Empty(ref e) => {
                if local_name(e.name().into_inner())==b"extLst"{self.set_extensions(Some(super::ExtensionList::read(reader,e,true,session)));}

                if local_name(e.name().0) == b"dLbls" {
                    self.data_labels.mark_empty_import();
                } else if local_name(e.name().0) == b"varyColors" {
                    self.vary_colors.set_attributes(reader, e);
                }
            },
            Event::End(ref e) => {
                if local_name(e.name().0) == b"pie3DChart" {
                    return;
                }
            },
            Event::Eof => panic!("Error: Could not find {} end element", "c:pie3DChart")
        );
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>, wb: &Workbook) {
        // c:pie3DChart
        write_start_tag(writer, "c:pie3DChart", vec![], false);

        // c:varyColors
        self.vary_colors.write_to(writer);

        // c:ser
        for v in self.area_chart_series_list.area_chart_series() {
            v.write_to(writer, wb, SeriesFamily::Pie);
        }

        // c:dLbls
        self.data_labels.write_to(writer, wb);

        if let Some(ext) = self.extensions() {
            ext.write_for_owner(
                writer,
                Some(wb),
                "pie3DChart",
                Some(SeriesFamily::Pie),
            );
        }
        write_end_tag(writer, "c:pie3DChart");
    }
}
