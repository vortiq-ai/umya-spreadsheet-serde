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
    ChartLines,
    DataLabels,
    DoughnutChart,
    Line3DChart,
    LineChart,
    OfPieChart,
    Pie3DChart,
    PieChart,
    RadarChart,
    ScatterChart,
    SeriesFamily,
    StockChart,
    Surface3DChart,
    SurfaceChart,
    group_lines::{
        GroupLines,
        GroupLinesMut,
    },
};
use crate::{
    ChartType,
    Workbook,
};

fn without_series_lines<'a, S, L, E, G>(
    (series, labels, extensions, lines): (S, L, E, G),
) -> (S, L, E, G, Option<&'a mut [ChartLines]>) {
    (series, labels, extensions, lines, None)
}

fn without_any_lines<'a, S, L, E>(
    (series, labels, extensions): (S, L, E),
) -> (S, L, E, GroupLinesMut<'a>, Option<&'a mut [ChartLines]>) {
    (series, labels, extensions, GroupLinesMut::default(), None)
}

fn with_series_lines<S, L, E>(
    (series, labels, extensions, series_lines): (S, L, E, Option<&mut [ChartLines]>),
) -> (S, L, E, GroupLinesMut<'_>, Option<&mut [ChartLines]>) {
    (
        series,
        labels,
        extensions,
        GroupLinesMut::default(),
        series_lines,
    )
}

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
    StockChart(StockChart),
    SurfaceChart(SurfaceChart),
    Surface3DChart(Surface3DChart),
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
    StockChart(&'a StockChart),
    SurfaceChart(&'a SurfaceChart),
    Surface3DChart(&'a Surface3DChart),
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
    StockChart(&'a mut StockChart),
    SurfaceChart(&'a mut SurfaceChart),
    Surface3DChart(&'a mut Surface3DChart),
}

fn without_lines<'a, S, L, E>(parts: (S, L, E)) -> (S, L, E, GroupLines<'a>) {
    (parts.0, parts.1, parts.2, GroupLines::default())
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
            Self::StockChart(chart) => ChartGroupRef::StockChart(chart),
            Self::SurfaceChart(chart) => ChartGroupRef::SurfaceChart(chart),
            Self::Surface3DChart(chart) => ChartGroupRef::Surface3DChart(chart),
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
            Self::StockChart(chart) => ChartGroupMut::StockChart(chart),
            Self::SurfaceChart(chart) => ChartGroupMut::SurfaceChart(chart),
            Self::Surface3DChart(chart) => ChartGroupMut::Surface3DChart(chart),
        }
    }
}

impl<'a> ChartGroupRef<'a> {
    pub(crate) fn into_reference_role_parts(self) -> super::chart_reference_roles::GroupParts<'a> {
        let (series, labels, extensions, lines) = match self {
            Self::LineChart(owner) => owner.reference_group_parts(),
            Self::Line3DChart(owner) => owner.reference_group_parts(),
            Self::PieChart(owner) => without_lines(owner.reference_group_parts()),
            Self::Pie3DChart(owner) => without_lines(owner.reference_group_parts()),
            Self::DoughnutChart(owner) => without_lines(owner.reference_group_parts()),
            Self::ScatterChart(owner) => without_lines(owner.reference_group_parts()),
            Self::BarChart(owner) => without_lines(owner.reference_group_parts()),
            Self::Bar3DChart(owner) => without_lines(owner.reference_group_parts()),
            Self::RadarChart(owner) => without_lines(owner.reference_group_parts()),
            Self::BubbleChart(owner) => without_lines(owner.reference_group_parts()),
            Self::AreaChart(owner) => owner.reference_group_parts(),
            Self::Area3DChart(owner) => owner.reference_group_parts(),
            Self::OfPieChart(owner) => without_lines(owner.reference_group_parts()),
            Self::StockChart(owner) => owner.reference_group_parts(),
            Self::SurfaceChart(owner) => owner.reference_group_parts(),
            Self::Surface3DChart(owner) => owner.reference_group_parts(),
        };
        let series_lines = match self {
            Self::BarChart(owner) => Some(owner.series_lines()),
            Self::OfPieChart(owner) => Some(owner.series_lines().lines()),
            _ => None,
        };
        let (shapes, lines_extensions) = (lines.shapes(), lines.extensions());
        super::chart_reference_roles::GroupParts {
            series: series.area_chart_series(),
            labels,
            extensions,
            series_lines,
            shapes,
            lines_extensions,
        }
    }

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
            Self::StockChart(_) => ChartType::StockChart,
            Self::SurfaceChart(_) => ChartType::SurfaceChart,
            Self::Surface3DChart(_) => ChartType::Surface3DChart,
        }
    }

    pub(crate) fn lines(self) -> GroupLines<'a> {
        match self {
            Self::LineChart(chart) => chart.lines_for_roles(),
            Self::Line3DChart(chart) => chart.lines_for_roles(),
            Self::AreaChart(chart) => chart.lines_for_roles(),
            Self::Area3DChart(chart) => chart.lines_for_roles(),
            Self::StockChart(chart) => chart.lines_for_roles(),
            Self::SurfaceChart(chart) => chart.lines_for_roles(),
            Self::Surface3DChart(chart) => chart.lines_for_roles(),
            _ => GroupLines::default(),
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
            Self::StockChart(chart) => chart.area_chart_series_list(),
            Self::SurfaceChart(chart) => chart.area_chart_series_list(),
            Self::Surface3DChart(chart) => chart.area_chart_series_list(),
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
            Self::StockChart(chart) => Some(chart.axis_id()),
            Self::SurfaceChart(chart) => Some(chart.axis_id()),
            Self::Surface3DChart(chart) => Some(chart.axis_id()),
            Self::PieChart(_)
            | Self::Pie3DChart(_)
            | Self::DoughnutChart(_)
            | Self::OfPieChart(_) => None,
        }
    }

    pub(crate) fn write_to(
        self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        wb: &Workbook,
    ) -> Result<(), crate::XlsxError> {
        match self {
            Self::LineChart(chart) => chart.write_to(writer, wb)?,
            Self::Line3DChart(chart) => chart.write_to(writer, wb)?,
            Self::PieChart(chart) => chart.write_to(writer, wb)?,
            Self::Pie3DChart(chart) => chart.write_to(writer, wb)?,
            Self::DoughnutChart(chart) => chart.write_to(writer, wb)?,
            Self::ScatterChart(chart) => chart.write_to(writer, wb)?,
            Self::BarChart(chart) => chart.write_to(writer, wb)?,
            Self::Bar3DChart(chart) => chart.write_to(writer, wb)?,
            Self::RadarChart(chart) => chart.write_to(writer, wb)?,
            Self::BubbleChart(chart) => chart.write_to(writer, wb)?,
            Self::AreaChart(chart) => chart.write_to(writer, wb)?,
            Self::Area3DChart(chart) => chart.write_to(writer, wb)?,
            Self::OfPieChart(chart) => chart.write_to(writer, wb)?,
            Self::StockChart(chart) => chart.write_to(writer, wb)?,
            Self::SurfaceChart(chart) => chart.write_to(writer, wb)?,
            Self::Surface3DChart(chart) => chart.write_to(writer, wb)?,
        }

        Ok(())
    }
}

impl<'a> ChartGroupMut<'a> {
    pub(crate) fn restore_export_parts(&mut self, previous: ChartGroupRef<'_>) -> usize {
        match (self, previous) {
            (Self::LineChart(current), ChartGroupRef::LineChart(previous)) => {
                current.restore_export_parts(previous)
            }
            (Self::Line3DChart(current), ChartGroupRef::Line3DChart(previous)) => {
                current.restore_export_parts(previous)
            }
            (Self::PieChart(current), ChartGroupRef::PieChart(previous)) => {
                current.restore_export_parts(previous)
            }
            (Self::Pie3DChart(current), ChartGroupRef::Pie3DChart(previous)) => {
                current.restore_export_parts(previous)
            }
            (Self::DoughnutChart(current), ChartGroupRef::DoughnutChart(previous)) => {
                current.restore_export_parts(previous)
            }
            (Self::ScatterChart(current), ChartGroupRef::ScatterChart(previous)) => {
                current.restore_export_parts(previous)
            }
            (Self::BarChart(current), ChartGroupRef::BarChart(previous)) => {
                current.restore_export_parts(previous)
            }
            (Self::Bar3DChart(current), ChartGroupRef::Bar3DChart(previous)) => {
                current.restore_export_parts(previous)
            }
            (Self::RadarChart(current), ChartGroupRef::RadarChart(previous)) => {
                current.restore_export_parts(previous)
            }
            (Self::BubbleChart(current), ChartGroupRef::BubbleChart(previous)) => {
                current.restore_export_parts(previous)
            }
            (Self::AreaChart(current), ChartGroupRef::AreaChart(previous)) => {
                current.restore_export_parts(previous)
            }
            (Self::Area3DChart(current), ChartGroupRef::Area3DChart(previous)) => {
                current.restore_export_parts(previous)
            }
            (Self::OfPieChart(current), ChartGroupRef::OfPieChart(previous)) => {
                current.restore_export_parts(previous)
            }
            (Self::StockChart(current), ChartGroupRef::StockChart(previous)) => {
                current.restore_export_parts(previous)
            }
            (Self::SurfaceChart(current), ChartGroupRef::SurfaceChart(previous)) => {
                current.restore_export_parts(previous)
            }
            (Self::Surface3DChart(current), ChartGroupRef::Surface3DChart(previous)) => {
                current.restore_export_parts(previous)
            }
            _ => 0,
        }
    }

    pub(crate) fn lines_mut(&mut self) -> GroupLinesMut<'_> {
        match self {
            Self::LineChart(chart) => chart.reference_group_parts_mut().3,
            Self::Line3DChart(chart) => chart.reference_group_parts_mut().3,
            Self::AreaChart(chart) => chart.reference_group_parts_mut().3,
            Self::Area3DChart(chart) => chart.reference_group_parts_mut().3,
            Self::StockChart(chart) => chart.reference_group_parts_mut().3,
            Self::SurfaceChart(chart) => chart.reference_group_parts_mut().3,
            Self::Surface3DChart(chart) => chart.reference_group_parts_mut().3,
            _ => GroupLinesMut::default(),
        }
    }

    /// Removes the group's `c:dropLines`, as Excel's "Lines" menu does; the
    /// group then writes none. Returns `false` when the group type has no
    /// drop lines (line, 3-D line, area, 3-D area and stock groups have them).
    #[must_use]
    pub fn remove_drop_lines(self) -> bool {
        match self {
            Self::LineChart(chart) => drop(chart.set_drop_lines(None)),
            Self::Line3DChart(chart) => drop(chart.set_drop_lines(None)),
            Self::AreaChart(chart) => drop(chart.set_drop_lines(None)),
            Self::Area3DChart(chart) => drop(chart.set_drop_lines(None)),
            Self::StockChart(chart) => drop(chart.set_drop_lines(None)),
            _ => return false,
        }
        true
    }

    /// Removes the group's `c:hiLowLines`. Returns `false` when the group
    /// type has none (line and stock groups have them).
    #[must_use]
    pub fn remove_hi_low_lines(self) -> bool {
        match self {
            Self::LineChart(chart) => drop(chart.set_hi_low_lines(None)),
            Self::StockChart(chart) => drop(chart.set_hi_low_lines(None)),
            _ => return false,
        }
        true
    }

    /// Removes the group's `c:upDownBars` with their formatting and hidden
    /// parts, as Excel's "Up/Down Bars" menu does. Returns `false` when the
    /// group type has none (line and stock groups have them).
    #[must_use]
    pub fn remove_up_down_bars(self) -> bool {
        match self {
            Self::LineChart(chart) => drop(chart.set_up_down_bars(None)),
            Self::StockChart(chart) => drop(chart.set_up_down_bars(None)),
            _ => return false,
        }
        true
    }

    pub(crate) fn into_projection_parts(self) -> super::chart_reference_roles::GroupPartsMut<'a> {
        let (series, labels, extensions, lines, series_lines) = match self {
            Self::LineChart(owner) => without_series_lines(owner.projection_group_parts_mut()),
            Self::Line3DChart(owner) => without_series_lines(owner.projection_group_parts_mut()),
            Self::PieChart(owner) => without_any_lines(owner.projection_group_parts_mut()),
            Self::Pie3DChart(owner) => without_any_lines(owner.projection_group_parts_mut()),
            Self::DoughnutChart(owner) => without_any_lines(owner.projection_group_parts_mut()),
            Self::ScatterChart(owner) => without_any_lines(owner.projection_group_parts_mut()),
            Self::BarChart(owner) => with_series_lines(owner.projection_group_parts_mut()),
            Self::Bar3DChart(owner) => without_any_lines(owner.projection_group_parts_mut()),
            Self::RadarChart(owner) => without_any_lines(owner.projection_group_parts_mut()),
            Self::BubbleChart(owner) => without_any_lines(owner.projection_group_parts_mut()),
            Self::AreaChart(owner) => without_series_lines(owner.projection_group_parts_mut()),
            Self::Area3DChart(owner) => without_series_lines(owner.projection_group_parts_mut()),
            Self::OfPieChart(owner) => with_series_lines(owner.projection_group_parts_mut()),
            Self::StockChart(owner) => without_series_lines(owner.projection_group_parts_mut()),
            Self::SurfaceChart(owner) => without_series_lines(owner.projection_group_parts_mut()),
            Self::Surface3DChart(owner) => without_series_lines(owner.projection_group_parts_mut()),
        };
        let (shapes, lines_extensions) = lines.into_parts();
        super::chart_reference_roles::GroupPartsMut {
            series: series.area_chart_series_mut(),
            labels,
            extensions,
            series_lines,
            shapes,
            lines_extensions,
        }
    }

    pub(crate) fn into_reference_role_parts(
        self,
    ) -> super::chart_reference_roles::GroupPartsMut<'a> {
        let (series, labels, extensions, lines, series_lines) = match self {
            Self::LineChart(owner) => without_series_lines(owner.reference_group_parts_mut()),
            Self::Line3DChart(owner) => without_series_lines(owner.reference_group_parts_mut()),
            Self::PieChart(owner) => without_any_lines(owner.reference_group_parts_mut()),
            Self::Pie3DChart(owner) => without_any_lines(owner.reference_group_parts_mut()),
            Self::DoughnutChart(owner) => without_any_lines(owner.reference_group_parts_mut()),
            Self::ScatterChart(owner) => without_any_lines(owner.reference_group_parts_mut()),
            Self::BarChart(owner) => with_series_lines(owner.reference_group_parts_mut()),
            Self::Bar3DChart(owner) => without_any_lines(owner.reference_group_parts_mut()),
            Self::RadarChart(owner) => without_any_lines(owner.reference_group_parts_mut()),
            Self::BubbleChart(owner) => without_any_lines(owner.reference_group_parts_mut()),
            Self::AreaChart(owner) => without_series_lines(owner.reference_group_parts_mut()),
            Self::Area3DChart(owner) => without_series_lines(owner.reference_group_parts_mut()),
            Self::OfPieChart(owner) => with_series_lines(owner.reference_group_parts_mut()),
            Self::StockChart(owner) => without_series_lines(owner.reference_group_parts_mut()),
            Self::SurfaceChart(owner) => without_series_lines(owner.reference_group_parts_mut()),
            Self::Surface3DChart(owner) => without_series_lines(owner.reference_group_parts_mut()),
        };
        let (shapes, lines_extensions) = lines.into_parts();
        super::chart_reference_roles::GroupPartsMut {
            series: series.area_chart_series_mut(),
            labels,
            extensions,
            series_lines,
            shapes,
            lines_extensions,
        }
    }

    /// Removes the group's own `c:dLbls` (the labels it applies to every series
    /// without labels of its own), with its point labels, extension list and
    /// hidden source parts. The group then writes no `c:dLbls`. Series-level
    /// labels, point overrides (`c:dPt`) and all other group content are
    /// untouched. Returns `false` when the group type has no label owner.
    #[must_use]
    pub fn remove_data_labels(self) -> bool {
        match self.into_reference_role_parts().labels {
            Some(labels) => {
                labels.mark_group_absent();
                true
            }
            None => false,
        }
    }

    /// Turns off the data labels of one series (`series_index` counts in the
    /// group's series order) the way Excel does: a group that shows labels
    /// needs the series to carry an explicit deleted owner, otherwise the
    /// series simply writes no `c:dLbls`. Point overrides (`c:dPt`) are
    /// untouched. Returns `false` when the group has no such series.
    #[must_use]
    pub fn remove_series_data_labels(self, series_index: usize) -> bool {
        let parts = self.into_reference_role_parts();
        let group_shows_labels = parts
            .labels
            .as_deref()
            .is_some_and(DataLabels::shows_labels);
        let Some(series) = parts.series.get_mut(series_index) else {
            return false;
        };
        if group_shows_labels {
            series.delete_data_labels();
        } else {
            series.remove_data_labels();
        }
        true
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
            Self::StockChart(_) => ChartType::StockChart,
            Self::SurfaceChart(_) => ChartType::SurfaceChart,
            Self::Surface3DChart(_) => ChartType::Surface3DChart,
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
            Self::StockChart(chart) => chart.area_chart_series_list_mut(),
            Self::SurfaceChart(chart) => chart.area_chart_series_list_mut(),
            Self::Surface3DChart(chart) => chart.area_chart_series_list_mut(),
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
            Self::StockChart(chart) => Some(chart.axis_id_mut()),
            Self::SurfaceChart(chart) => Some(chart.axis_id_mut()),
            Self::Surface3DChart(chart) => Some(chart.axis_id_mut()),
            Self::PieChart(_)
            | Self::Pie3DChart(_)
            | Self::DoughnutChart(_)
            | Self::OfPieChart(_) => None,
        }
    }
}
