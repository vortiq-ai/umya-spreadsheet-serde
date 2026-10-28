use std::io::Cursor;

use quick_xml::Writer;

use super::{
    Area3DChart,
    AreaChart,
    AreaChartSeriesList,
    AxisId,
    Bar3DChart,
    BarChart,
    BubbleChart,
    DoughnutChart,
    Line3DChart,
    LineChart,
    OfPieChart,
    Pie3DChart,
    PieChart,
    RadarChart,
    ScatterChart,
    SeriesFamily,
};
use crate::{
    ChartType,
    Workbook,
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub enum ChartGroup {
    LineChart(LineChart),
    Line3DChart(Line3DChart),
    PieChart(PieChart),
    Pie3DChart(Pie3DChart),
    DoughnutChart(DoughnutChart),
    ScatterChart(ScatterChart),
    BarChart(BarChart),
    Bar3DChart(Bar3DChart),
    RadarChart(RadarChart),
    BubbleChart(BubbleChart),
    AreaChart(AreaChart),
    Area3DChart(Area3DChart),
    OfPieChart(OfPieChart),
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
// A position marker is smaller than a boxed additional container.
#[allow(variant_size_differences)]
pub enum ChartGroupEntry {
    First(ChartType),
    Additional(Box<ChartGroup>),
}

#[derive(Clone, Copy, Debug)]
pub enum ChartGroupRef<'a> {
    LineChart(&'a LineChart),
    Line3DChart(&'a Line3DChart),
    PieChart(&'a PieChart),
    Pie3DChart(&'a Pie3DChart),
    DoughnutChart(&'a DoughnutChart),
    ScatterChart(&'a ScatterChart),
    BarChart(&'a BarChart),
    Bar3DChart(&'a Bar3DChart),
    RadarChart(&'a RadarChart),
    BubbleChart(&'a BubbleChart),
    AreaChart(&'a AreaChart),
    Area3DChart(&'a Area3DChart),
    OfPieChart(&'a OfPieChart),
}

#[derive(Debug)]
pub enum ChartGroupMut<'a> {
    LineChart(&'a mut LineChart),
    Line3DChart(&'a mut Line3DChart),
    PieChart(&'a mut PieChart),
    Pie3DChart(&'a mut Pie3DChart),
    DoughnutChart(&'a mut DoughnutChart),
    ScatterChart(&'a mut ScatterChart),
    BarChart(&'a mut BarChart),
    Bar3DChart(&'a mut Bar3DChart),
    RadarChart(&'a mut RadarChart),
    BubbleChart(&'a mut BubbleChart),
    AreaChart(&'a mut AreaChart),
    Area3DChart(&'a mut Area3DChart),
    OfPieChart(&'a mut OfPieChart),
}

impl ChartGroup {
    #[must_use]
    pub fn kind(&self) -> ChartType {
        self.as_group_ref().kind()
    }

    pub(crate) fn as_group_ref(&self) -> ChartGroupRef<'_> {
        match self {
            Self::LineChart(chart) => ChartGroupRef::LineChart(chart),
            Self::Line3DChart(chart) => ChartGroupRef::Line3DChart(chart),
            Self::PieChart(chart) => ChartGroupRef::PieChart(chart),
            Self::Pie3DChart(chart) => ChartGroupRef::Pie3DChart(chart),
            Self::DoughnutChart(chart) => ChartGroupRef::DoughnutChart(chart),
            Self::ScatterChart(chart) => ChartGroupRef::ScatterChart(chart),
            Self::BarChart(chart) => ChartGroupRef::BarChart(chart),
            Self::Bar3DChart(chart) => ChartGroupRef::Bar3DChart(chart),
            Self::RadarChart(chart) => ChartGroupRef::RadarChart(chart),
            Self::BubbleChart(chart) => ChartGroupRef::BubbleChart(chart),
            Self::AreaChart(chart) => ChartGroupRef::AreaChart(chart),
            Self::Area3DChart(chart) => ChartGroupRef::Area3DChart(chart),
            Self::OfPieChart(chart) => ChartGroupRef::OfPieChart(chart),
        }
    }

    pub(crate) fn as_group_mut(&mut self) -> ChartGroupMut<'_> {
        match self {
            Self::LineChart(chart) => ChartGroupMut::LineChart(chart),
            Self::Line3DChart(chart) => ChartGroupMut::Line3DChart(chart),
            Self::PieChart(chart) => ChartGroupMut::PieChart(chart),
            Self::Pie3DChart(chart) => ChartGroupMut::Pie3DChart(chart),
            Self::DoughnutChart(chart) => ChartGroupMut::DoughnutChart(chart),
            Self::ScatterChart(chart) => ChartGroupMut::ScatterChart(chart),
            Self::BarChart(chart) => ChartGroupMut::BarChart(chart),
            Self::Bar3DChart(chart) => ChartGroupMut::Bar3DChart(chart),
            Self::RadarChart(chart) => ChartGroupMut::RadarChart(chart),
            Self::BubbleChart(chart) => ChartGroupMut::BubbleChart(chart),
            Self::AreaChart(chart) => ChartGroupMut::AreaChart(chart),
            Self::Area3DChart(chart) => ChartGroupMut::Area3DChart(chart),
            Self::OfPieChart(chart) => ChartGroupMut::OfPieChart(chart),
        }
    }
}

impl<'a> ChartGroupRef<'a> {
    #[must_use]
    pub fn kind(self) -> ChartType {
        match self {
            Self::LineChart(_) => ChartType::LineChart,
            Self::Line3DChart(_) => ChartType::Line3DChart,
            Self::PieChart(_) => ChartType::PieChart,
            Self::Pie3DChart(_) => ChartType::Pie3DChart,
            Self::DoughnutChart(_) => ChartType::DoughnutChart,
            Self::ScatterChart(_) => ChartType::ScatterChart,
            Self::BarChart(_) => ChartType::BarChart,
            Self::Bar3DChart(_) => ChartType::Bar3DChart,
            Self::RadarChart(_) => ChartType::RadarChart,
            Self::BubbleChart(_) => ChartType::BubbleChart,
            Self::AreaChart(_) => ChartType::AreaChart,
            Self::Area3DChart(_) => ChartType::Area3DChart,
            Self::OfPieChart(_) => ChartType::OfPieChart,
        }
    }

    /// The series type this group writes its series as.
    #[must_use]
    pub fn series_family(self) -> SeriesFamily {
        SeriesFamily::from(self.kind())
    }

    #[must_use]
    pub fn series(self) -> &'a AreaChartSeriesList {
        match self {
            Self::LineChart(chart) => chart.area_chart_series_list(),
            Self::Line3DChart(chart) => chart.area_chart_series_list(),
            Self::PieChart(chart) => chart.area_chart_series_list(),
            Self::Pie3DChart(chart) => chart.area_chart_series_list(),
            Self::DoughnutChart(chart) => chart.area_chart_series_list(),
            Self::ScatterChart(chart) => chart.area_chart_series_list(),
            Self::BarChart(chart) => chart.area_chart_series_list(),
            Self::Bar3DChart(chart) => chart.area_chart_series_list(),
            Self::RadarChart(chart) => chart.area_chart_series_list(),
            Self::BubbleChart(chart) => chart.area_chart_series_list(),
            Self::AreaChart(chart) => chart.area_chart_series_list(),
            Self::Area3DChart(chart) => chart.area_chart_series_list(),
            Self::OfPieChart(chart) => chart.area_chart_series_list(),
        }
    }

    #[must_use]
    pub fn axis_ids(self) -> Option<&'a [AxisId]> {
        match self {
            Self::LineChart(chart) => Some(chart.axis_id()),
            Self::Line3DChart(chart) => Some(chart.axis_id()),
            Self::ScatterChart(chart) => Some(chart.axis_id()),
            Self::BarChart(chart) => Some(chart.axis_id()),
            Self::Bar3DChart(chart) => Some(chart.axis_id()),
            Self::RadarChart(chart) => Some(chart.axis_id()),
            Self::BubbleChart(chart) => Some(chart.axis_id()),
            Self::AreaChart(chart) => Some(chart.axis_id()),
            Self::Area3DChart(chart) => Some(chart.axis_id()),
            Self::PieChart(_)
            | Self::Pie3DChart(_)
            | Self::DoughnutChart(_)
            | Self::OfPieChart(_) => None,
        }
    }

    pub(crate) fn write_to(self, writer: &mut Writer<Cursor<Vec<u8>>>, wb: &Workbook) {
        match self {
            Self::LineChart(chart) => chart.write_to(writer, wb),
            Self::Line3DChart(chart) => chart.write_to(writer, wb),
            Self::PieChart(chart) => chart.write_to(writer, wb),
            Self::Pie3DChart(chart) => chart.write_to(writer, wb),
            Self::DoughnutChart(chart) => chart.write_to(writer, wb),
            Self::ScatterChart(chart) => chart.write_to(writer, wb),
            Self::BarChart(chart) => chart.write_to(writer, wb),
            Self::Bar3DChart(chart) => chart.write_to(writer, wb),
            Self::RadarChart(chart) => chart.write_to(writer, wb),
            Self::BubbleChart(chart) => chart.write_to(writer, wb),
            Self::AreaChart(chart) => chart.write_to(writer, wb),
            Self::Area3DChart(chart) => chart.write_to(writer, wb),
            Self::OfPieChart(chart) => chart.write_to(writer, wb),
        }
    }
}

impl<'a> ChartGroupMut<'a> {
    pub(crate) fn formulas_mut(self) -> Vec<&'a mut super::Formula> {
        match self {
            Self::LineChart(chart) => chart.formulas_mut(),
            Self::Line3DChart(chart) => chart.formulas_mut(),
            Self::PieChart(chart) => chart.formulas_mut(),
            Self::Pie3DChart(chart) => chart.formulas_mut(),
            Self::DoughnutChart(chart) => chart.formulas_mut(),
            Self::ScatterChart(chart) => chart.formulas_mut(),
            Self::BarChart(chart) => chart.formulas_mut(),
            Self::Bar3DChart(chart) => chart.formulas_mut(),
            Self::RadarChart(chart) => chart.formulas_mut(),
            Self::BubbleChart(chart) => chart.formulas_mut(),
            Self::AreaChart(chart) => chart.formulas_mut(),
            Self::Area3DChart(chart) => chart.formulas_mut(),
            Self::OfPieChart(chart) => chart.formulas_mut(),
        }
    }

    #[must_use]
    pub fn kind(&self) -> ChartType {
        match self {
            Self::LineChart(_) => ChartType::LineChart,
            Self::Line3DChart(_) => ChartType::Line3DChart,
            Self::PieChart(_) => ChartType::PieChart,
            Self::Pie3DChart(_) => ChartType::Pie3DChart,
            Self::DoughnutChart(_) => ChartType::DoughnutChart,
            Self::ScatterChart(_) => ChartType::ScatterChart,
            Self::BarChart(_) => ChartType::BarChart,
            Self::Bar3DChart(_) => ChartType::Bar3DChart,
            Self::RadarChart(_) => ChartType::RadarChart,
            Self::BubbleChart(_) => ChartType::BubbleChart,
            Self::AreaChart(_) => ChartType::AreaChart,
            Self::Area3DChart(_) => ChartType::Area3DChart,
            Self::OfPieChart(_) => ChartType::OfPieChart,
        }
    }

    /// Consumes the view so the series borrow can outlive the view.
    #[must_use]
    pub fn series_mut(self) -> &'a mut AreaChartSeriesList {
        match self {
            Self::LineChart(chart) => chart.area_chart_series_list_mut(),
            Self::Line3DChart(chart) => chart.area_chart_series_list_mut(),
            Self::PieChart(chart) => chart.area_chart_series_list_mut(),
            Self::Pie3DChart(chart) => chart.area_chart_series_list_mut(),
            Self::DoughnutChart(chart) => chart.area_chart_series_list_mut(),
            Self::ScatterChart(chart) => chart.area_chart_series_list_mut(),
            Self::BarChart(chart) => chart.area_chart_series_list_mut(),
            Self::Bar3DChart(chart) => chart.area_chart_series_list_mut(),
            Self::RadarChart(chart) => chart.area_chart_series_list_mut(),
            Self::BubbleChart(chart) => chart.area_chart_series_list_mut(),
            Self::AreaChart(chart) => chart.area_chart_series_list_mut(),
            Self::Area3DChart(chart) => chart.area_chart_series_list_mut(),
            Self::OfPieChart(chart) => chart.area_chart_series_list_mut(),
        }
    }

    /// Consumes the view so the axis borrow can outlive the view.
    #[must_use]
    pub fn axis_ids_mut(self) -> Option<&'a mut Vec<AxisId>> {
        match self {
            Self::LineChart(chart) => Some(chart.axis_id_mut()),
            Self::Line3DChart(chart) => Some(chart.axis_id_mut()),
            Self::ScatterChart(chart) => Some(chart.axis_id_mut()),
            Self::BarChart(chart) => Some(chart.axis_id_mut()),
            Self::Bar3DChart(chart) => Some(chart.axis_id_mut()),
            Self::RadarChart(chart) => Some(chart.axis_id_mut()),
            Self::BubbleChart(chart) => Some(chart.axis_id_mut()),
            Self::AreaChart(chart) => Some(chart.axis_id_mut()),
            Self::Area3DChart(chart) => Some(chart.axis_id_mut()),
            Self::PieChart(_)
            | Self::Pie3DChart(_)
            | Self::DoughnutChart(_)
            | Self::OfPieChart(_) => None,
        }
    }
}
