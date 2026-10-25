// c:plotArea
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
    Area3DChart,
    AreaChart,
    AreaChartSeriesList,
    Bar3DChart,
    BarChart,
    BubbleChart,
    CategoryAxis,
    ChartGroup,
    ChartGroupEntry,
    ChartGroupMut,
    ChartGroupRef,
    DoughnutChart,
    Formula,
    GroupingValues,
    Layout,
    Line3DChart,
    LineChart,
    OfPieChart,
    Pie3DChart,
    PieChart,
    RadarChart,
    ScatterChart,
    SeriesAxis,
    ShapeProperties,
    ValueAxis,
};
use crate::{
    drawing::charts::DateAxis,
    reader::driver::{
        local_name,
        skip_element,
    },
    structs::{
        ChartType,
        Workbook,
    },
    traits::AdjustmentCoordinateWithSheet,
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
    xml_read_loop,
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct PlotArea {
    layout:            Layout,
    line_chart:        Option<LineChart>,
    line_3d_chart:     Option<Line3DChart>,
    pie_chart:         Option<PieChart>,
    pie_3d_chart:      Option<Pie3DChart>,
    doughnut_chart:    Option<DoughnutChart>,
    scatter_chart:     Option<ScatterChart>,
    bar_chart:         Option<BarChart>,
    bar_3d_chart:      Option<Bar3DChart>,
    radar_chart:       Option<RadarChart>,
    bubble_chart:      Option<BubbleChart>,
    area_chart:        Option<AreaChart>,
    area_3d_chart:     Option<Area3DChart>,
    of_pie_chart:      Option<OfPieChart>,
    category_axis:     Vec<CategoryAxis>,
    date_axis:         Vec<DateAxis>,
    value_axis:        Vec<ValueAxis>,
    series_axis:       Vec<SeriesAxis>,
    shape_properties:  Option<ShapeProperties>,
    #[cfg_attr(feature = "serde", serde(default))]
    chart_group_order: Vec<ChartGroupEntry>,
}

// Resolve malformed markers identically for borrowed views and owned repair.
fn order_chart_groups<T>(
    mut first: [Option<T>; 13],
    entries: impl Iterator<Item = (ChartType, Option<T>)>,
) -> Vec<T> {
    let entries: Vec<_> = entries.collect();
    let mut marked = [false; 13];
    for (kind, group) in &entries {
        if group.is_none() {
            marked[*kind as usize] = true;
        }
    }
    let mut result = Vec::new();
    for (kind, group) in entries {
        let index = kind as usize;
        if let Some(group) = group {
            if !marked[index] {
                result.extend(first[index].take());
            }
            result.push(group);
        } else {
            result.extend(first[index].take());
        }
    }
    result.extend(first.into_iter().flatten());
    result
}

impl PlotArea {
    /// Every container once, in document order, or legacy order without
    /// markers.
    #[must_use = "the iterator must be consumed to visit chart containers"]
    pub fn chart_groups(&self) -> impl Iterator<Item = ChartGroupRef<'_>> {
        let first = [
            self.line_chart.as_ref().map(ChartGroupRef::LineChart),
            self.line_3d_chart.as_ref().map(ChartGroupRef::Line3DChart),
            self.pie_chart.as_ref().map(ChartGroupRef::PieChart),
            self.pie_3d_chart.as_ref().map(ChartGroupRef::Pie3DChart),
            self.doughnut_chart
                .as_ref()
                .map(ChartGroupRef::DoughnutChart),
            self.scatter_chart.as_ref().map(ChartGroupRef::ScatterChart),
            self.bar_chart.as_ref().map(ChartGroupRef::BarChart),
            self.bar_3d_chart.as_ref().map(ChartGroupRef::Bar3DChart),
            self.radar_chart.as_ref().map(ChartGroupRef::RadarChart),
            self.bubble_chart.as_ref().map(ChartGroupRef::BubbleChart),
            self.area_chart.as_ref().map(ChartGroupRef::AreaChart),
            self.area_3d_chart.as_ref().map(ChartGroupRef::Area3DChart),
            self.of_pie_chart.as_ref().map(ChartGroupRef::OfPieChart),
        ];
        let entries = self.chart_group_order.iter().map(|entry| match entry {
            ChartGroupEntry::First(kind) => (*kind, None),
            ChartGroupEntry::Additional(group) => (group.kind(), Some(group.as_group_ref())),
        });
        order_chart_groups(first, entries).into_iter()
    }

    #[must_use = "the iterator must be consumed to visit chart containers"]
    pub fn chart_groups_mut(&mut self) -> impl Iterator<Item = ChartGroupMut<'_>> {
        let first = [
            self.line_chart.as_mut().map(ChartGroupMut::LineChart),
            self.line_3d_chart.as_mut().map(ChartGroupMut::Line3DChart),
            self.pie_chart.as_mut().map(ChartGroupMut::PieChart),
            self.pie_3d_chart.as_mut().map(ChartGroupMut::Pie3DChart),
            self.doughnut_chart
                .as_mut()
                .map(ChartGroupMut::DoughnutChart),
            self.scatter_chart.as_mut().map(ChartGroupMut::ScatterChart),
            self.bar_chart.as_mut().map(ChartGroupMut::BarChart),
            self.bar_3d_chart.as_mut().map(ChartGroupMut::Bar3DChart),
            self.radar_chart.as_mut().map(ChartGroupMut::RadarChart),
            self.bubble_chart.as_mut().map(ChartGroupMut::BubbleChart),
            self.area_chart.as_mut().map(ChartGroupMut::AreaChart),
            self.area_3d_chart.as_mut().map(ChartGroupMut::Area3DChart),
            self.of_pie_chart.as_mut().map(ChartGroupMut::OfPieChart),
        ];
        let entries = self.chart_group_order.iter_mut().map(|entry| match entry {
            ChartGroupEntry::First(kind) => (*kind, None),
            ChartGroupEntry::Additional(group) => (group.kind(), Some(group.as_group_mut())),
        });
        order_chart_groups(first, entries).into_iter()
    }

    fn replace_first_chart_group(&mut self, group: ChartGroup) -> Option<ChartGroup> {
        match group {
            ChartGroup::LineChart(chart) => {
                self.line_chart.replace(chart).map(ChartGroup::LineChart)
            }
            ChartGroup::Line3DChart(chart) => self
                .line_3d_chart
                .replace(chart)
                .map(ChartGroup::Line3DChart),
            ChartGroup::PieChart(chart) => self.pie_chart.replace(chart).map(ChartGroup::PieChart),
            ChartGroup::Pie3DChart(chart) => {
                self.pie_3d_chart.replace(chart).map(ChartGroup::Pie3DChart)
            }
            ChartGroup::DoughnutChart(chart) => self
                .doughnut_chart
                .replace(chart)
                .map(ChartGroup::DoughnutChart),
            ChartGroup::ScatterChart(chart) => self
                .scatter_chart
                .replace(chart)
                .map(ChartGroup::ScatterChart),
            ChartGroup::BarChart(chart) => self.bar_chart.replace(chart).map(ChartGroup::BarChart),
            ChartGroup::Bar3DChart(chart) => {
                self.bar_3d_chart.replace(chart).map(ChartGroup::Bar3DChart)
            }
            ChartGroup::RadarChart(chart) => {
                self.radar_chart.replace(chart).map(ChartGroup::RadarChart)
            }
            ChartGroup::BubbleChart(chart) => self
                .bubble_chart
                .replace(chart)
                .map(ChartGroup::BubbleChart),
            ChartGroup::AreaChart(chart) => {
                self.area_chart.replace(chart).map(ChartGroup::AreaChart)
            }
            ChartGroup::Area3DChart(chart) => self
                .area_3d_chart
                .replace(chart)
                .map(ChartGroup::Area3DChart),
            ChartGroup::OfPieChart(chart) => {
                self.of_pie_chart.replace(chart).map(ChartGroup::OfPieChart)
            }
        }
    }

    fn set_chart_group(&mut self, group: ChartGroup) -> &mut Self {
        self.normalize_chart_groups();
        let kind = group.kind();
        let previous = self.replace_first_chart_group(group);
        if previous.is_none() && !self.chart_group_order.is_empty() {
            self.chart_group_order.push(ChartGroupEntry::First(kind));
        }
        self.normalize_chart_groups();
        self
    }

    fn push_chart_group(&mut self, group: ChartGroup) {
        if self.chart_group_order.is_empty() {
            self.chart_group_order = self
                .chart_groups()
                .map(|group| ChartGroupEntry::First(group.kind()))
                .collect();
        }
        let kind = group.kind();
        let present = match kind {
            ChartType::LineChart => self.line_chart.is_some(),
            ChartType::Line3DChart => self.line_3d_chart.is_some(),
            ChartType::PieChart => self.pie_chart.is_some(),
            ChartType::Pie3DChart => self.pie_3d_chart.is_some(),
            ChartType::DoughnutChart => self.doughnut_chart.is_some(),
            ChartType::ScatterChart => self.scatter_chart.is_some(),
            ChartType::BarChart => self.bar_chart.is_some(),
            ChartType::Bar3DChart => self.bar_3d_chart.is_some(),
            ChartType::RadarChart => self.radar_chart.is_some(),
            ChartType::BubbleChart => self.bubble_chart.is_some(),
            ChartType::AreaChart => self.area_chart.is_some(),
            ChartType::Area3DChart => self.area_3d_chart.is_some(),
            ChartType::OfPieChart => self.of_pie_chart.is_some(),
        };
        if present {
            self.chart_group_order
                .push(ChartGroupEntry::Additional(Box::new(group)));
        } else {
            drop(self.replace_first_chart_group(group));
            self.chart_group_order.push(ChartGroupEntry::First(kind));
        }
    }

    /// Appends a container while keeping the first of each family in its field.
    pub fn add_chart_group(&mut self, group: ChartGroup) -> &mut Self {
        self.normalize_chart_groups();
        self.push_chart_group(group);
        self.normalize_chart_groups();
        self
    }

    /// Repairs markers and promotes first containers without changing
    /// traversal.
    pub fn normalize_chart_groups(&mut self) {
        let first = [
            self.line_chart.take().map(ChartGroup::LineChart),
            self.line_3d_chart.take().map(ChartGroup::Line3DChart),
            self.pie_chart.take().map(ChartGroup::PieChart),
            self.pie_3d_chart.take().map(ChartGroup::Pie3DChart),
            self.doughnut_chart.take().map(ChartGroup::DoughnutChart),
            self.scatter_chart.take().map(ChartGroup::ScatterChart),
            self.bar_chart.take().map(ChartGroup::BarChart),
            self.bar_3d_chart.take().map(ChartGroup::Bar3DChart),
            self.radar_chart.take().map(ChartGroup::RadarChart),
            self.bubble_chart.take().map(ChartGroup::BubbleChart),
            self.area_chart.take().map(ChartGroup::AreaChart),
            self.area_3d_chart.take().map(ChartGroup::Area3DChart),
            self.of_pie_chart.take().map(ChartGroup::OfPieChart),
        ];
        let entries = std::mem::take(&mut self.chart_group_order)
            .into_iter()
            .map(|entry| match entry {
                ChartGroupEntry::First(kind) => (kind, None),
                ChartGroupEntry::Additional(group) => (group.kind(), Some(*group)),
            });
        let mut seen = [false; 13];
        let mut previous = None;
        let mut legacy_order = true;
        for group in order_chart_groups(first, entries) {
            let kind = group.kind();
            let index = kind as usize;
            if seen[index] {
                legacy_order = false;
                self.chart_group_order
                    .push(ChartGroupEntry::Additional(Box::new(group)));
            } else {
                legacy_order &= previous.is_none_or(|previous| previous < index);
                previous = Some(index);
                seen[index] = true;
                drop(self.replace_first_chart_group(group));
                self.chart_group_order.push(ChartGroupEntry::First(kind));
            }
        }
        if legacy_order {
            self.chart_group_order.clear();
        }
    }

    #[must_use]
    pub fn layout(&self) -> &Layout {
        &self.layout
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use layout()")]
    pub fn get_layout(&self) -> &Layout {
        self.layout()
    }

    pub fn layout_mut(&mut self) -> &mut Layout {
        &mut self.layout
    }

    #[deprecated(since = "3.0.0", note = "Use layout_mut()")]
    pub fn get_layout_mut(&mut self) -> &mut Layout {
        self.layout_mut()
    }

    pub fn set_layout(&mut self, value: Layout) -> &mut Self {
        self.layout = value;
        self
    }

    #[must_use]
    pub fn line_chart(&self) -> Option<&LineChart> {
        self.line_chart.as_ref()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use line_chart()")]
    pub fn get_line_chart(&self) -> Option<&LineChart> {
        self.line_chart()
    }

    pub fn line_chart_mut(&mut self) -> Option<&mut LineChart> {
        self.line_chart.as_mut()
    }

    #[deprecated(since = "3.0.0", note = "Use line_chart_mut()")]
    pub fn get_line_chart_mut(&mut self) -> Option<&mut LineChart> {
        self.line_chart_mut()
    }

    pub fn set_line_chart(&mut self, value: LineChart) -> &mut Self {
        self.set_chart_group(ChartGroup::LineChart(value))
    }

    #[must_use]
    pub fn line_3d_chart(&self) -> Option<&Line3DChart> {
        self.line_3d_chart.as_ref()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use line_3d_chart()")]
    pub fn get_line_3d_chart(&self) -> Option<&Line3DChart> {
        self.line_3d_chart()
    }

    pub fn line_3d_chart_mut(&mut self) -> Option<&mut Line3DChart> {
        self.line_3d_chart.as_mut()
    }

    #[deprecated(since = "3.0.0", note = "Use line_3d_chart_mut()")]
    pub fn get_line_3d_chart_mut(&mut self) -> Option<&mut Line3DChart> {
        self.line_3d_chart_mut()
    }

    pub fn set_line_3d_chart(&mut self, value: Line3DChart) -> &mut Self {
        self.set_chart_group(ChartGroup::Line3DChart(value))
    }

    #[must_use]
    pub fn pie_chart(&self) -> Option<&PieChart> {
        self.pie_chart.as_ref()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use pie_chart()")]
    pub fn get_pie_chart(&self) -> Option<&PieChart> {
        self.pie_chart()
    }

    pub fn pie_chart_mut(&mut self) -> Option<&mut PieChart> {
        self.pie_chart.as_mut()
    }

    #[deprecated(since = "3.0.0", note = "Use pie_chart_mut()")]
    pub fn get_pie_chart_mut(&mut self) -> Option<&mut PieChart> {
        self.pie_chart_mut()
    }

    pub fn set_pie_chart(&mut self, value: PieChart) -> &mut Self {
        self.set_chart_group(ChartGroup::PieChart(value))
    }

    #[must_use]
    pub fn pie_3d_chart(&self) -> Option<&Pie3DChart> {
        self.pie_3d_chart.as_ref()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use pie_3d_chart()")]
    pub fn get_pie_3d_chart(&self) -> Option<&Pie3DChart> {
        self.pie_3d_chart()
    }

    pub fn pie_3d_chart_mut(&mut self) -> Option<&mut Pie3DChart> {
        self.pie_3d_chart.as_mut()
    }

    #[deprecated(since = "3.0.0", note = "Use pie_3d_chart_mut()")]
    pub fn get_pie_3d_chart_mut(&mut self) -> Option<&mut Pie3DChart> {
        self.pie_3d_chart_mut()
    }

    pub fn set_pie_3d_chart(&mut self, value: Pie3DChart) -> &mut Self {
        self.set_chart_group(ChartGroup::Pie3DChart(value))
    }

    #[must_use]
    pub fn doughnut_chart(&self) -> Option<&DoughnutChart> {
        self.doughnut_chart.as_ref()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use doughnut_chart()")]
    pub fn get_doughnut_chart(&self) -> Option<&DoughnutChart> {
        self.doughnut_chart()
    }

    pub fn doughnut_chart_mut(&mut self) -> Option<&mut DoughnutChart> {
        self.doughnut_chart.as_mut()
    }

    #[deprecated(since = "3.0.0", note = "Use doughnut_chart_mut()")]
    pub fn get_doughnut_chart_mut(&mut self) -> Option<&mut DoughnutChart> {
        self.doughnut_chart_mut()
    }

    pub fn set_doughnut_chart(&mut self, value: DoughnutChart) -> &mut Self {
        self.set_chart_group(ChartGroup::DoughnutChart(value))
    }

    #[must_use]
    pub fn scatter_chart(&self) -> Option<&ScatterChart> {
        self.scatter_chart.as_ref()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use scatter_chart()")]
    pub fn get_scatter_chart(&self) -> Option<&ScatterChart> {
        self.scatter_chart()
    }

    pub fn scatter_chart_mut(&mut self) -> Option<&mut ScatterChart> {
        self.scatter_chart.as_mut()
    }

    #[deprecated(since = "3.0.0", note = "Use scatter_chart_mut()")]
    pub fn get_scatter_chart_mut(&mut self) -> Option<&mut ScatterChart> {
        self.scatter_chart_mut()
    }

    pub fn set_scatter_chart(&mut self, value: ScatterChart) -> &mut Self {
        self.set_chart_group(ChartGroup::ScatterChart(value))
    }

    #[must_use]
    pub fn bar_chart(&self) -> Option<&BarChart> {
        self.bar_chart.as_ref()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use bar_chart()")]
    pub fn get_bar_chart(&self) -> Option<&BarChart> {
        self.bar_chart()
    }

    pub fn bar_chart_mut(&mut self) -> Option<&mut BarChart> {
        self.bar_chart.as_mut()
    }

    #[deprecated(since = "3.0.0", note = "Use bar_chart_mut()")]
    pub fn get_bar_chart_mut(&mut self) -> Option<&mut BarChart> {
        self.bar_chart_mut()
    }

    pub fn set_bar_chart(&mut self, value: BarChart) -> &mut Self {
        self.set_chart_group(ChartGroup::BarChart(value))
    }

    #[must_use]
    pub fn bar_3d_chart(&self) -> Option<&Bar3DChart> {
        self.bar_3d_chart.as_ref()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use bar_3d_chart()")]
    pub fn get_bar_3d_chart(&self) -> Option<&Bar3DChart> {
        self.bar_3d_chart()
    }

    pub fn bar_3d_chart_mut(&mut self) -> Option<&mut Bar3DChart> {
        self.bar_3d_chart.as_mut()
    }

    #[deprecated(since = "3.0.0", note = "Use bar_3d_chart_mut()")]
    pub fn get_bar_3d_chart_mut(&mut self) -> Option<&mut Bar3DChart> {
        self.bar_3d_chart_mut()
    }

    pub fn set_bar_3d_chart(&mut self, value: Bar3DChart) -> &mut Self {
        self.set_chart_group(ChartGroup::Bar3DChart(value))
    }

    #[must_use]
    pub fn radar_chart(&self) -> Option<&RadarChart> {
        self.radar_chart.as_ref()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use radar_chart()")]
    pub fn get_radar_chart(&self) -> Option<&RadarChart> {
        self.radar_chart()
    }

    pub fn radar_chart_mut(&mut self) -> Option<&mut RadarChart> {
        self.radar_chart.as_mut()
    }

    #[deprecated(since = "3.0.0", note = "Use radar_chart_mut()")]
    pub fn get_radar_chart_mut(&mut self) -> Option<&mut RadarChart> {
        self.radar_chart_mut()
    }

    pub fn set_radar_chart(&mut self, value: RadarChart) -> &mut Self {
        self.set_chart_group(ChartGroup::RadarChart(value))
    }

    #[must_use]
    pub fn bubble_chart(&self) -> Option<&BubbleChart> {
        self.bubble_chart.as_ref()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use bubble_chart()")]
    pub fn get_bubble_chart(&self) -> Option<&BubbleChart> {
        self.bubble_chart()
    }

    pub fn bubble_chart_mut(&mut self) -> Option<&mut BubbleChart> {
        self.bubble_chart.as_mut()
    }

    #[deprecated(since = "3.0.0", note = "Use bubble_chart_mut()")]
    pub fn get_bubble_chart_mut(&mut self) -> Option<&mut BubbleChart> {
        self.bubble_chart_mut()
    }

    pub fn set_bubble_chart(&mut self, value: BubbleChart) -> &mut Self {
        self.set_chart_group(ChartGroup::BubbleChart(value))
    }

    #[must_use]
    pub fn area_chart(&self) -> Option<&AreaChart> {
        self.area_chart.as_ref()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use area_chart()")]
    pub fn get_area_chart(&self) -> Option<&AreaChart> {
        self.area_chart()
    }

    pub fn area_chart_mut(&mut self) -> Option<&mut AreaChart> {
        self.area_chart.as_mut()
    }

    #[deprecated(since = "3.0.0", note = "Use area_chart_mut()")]
    pub fn get_area_chart_mut(&mut self) -> Option<&mut AreaChart> {
        self.area_chart_mut()
    }

    pub fn set_area_chart(&mut self, value: AreaChart) -> &mut Self {
        self.set_chart_group(ChartGroup::AreaChart(value))
    }

    #[must_use]
    pub fn area_3d_chart(&self) -> Option<&Area3DChart> {
        self.area_3d_chart.as_ref()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use area_3d_chart()")]
    pub fn get_area_3d_chart(&self) -> Option<&Area3DChart> {
        self.area_3d_chart()
    }

    pub fn area_3d_chart_mut(&mut self) -> Option<&mut Area3DChart> {
        self.area_3d_chart.as_mut()
    }

    #[deprecated(since = "3.0.0", note = "Use area_3d_chart_mut()")]
    pub fn get_area_3d_chart_mut(&mut self) -> Option<&mut Area3DChart> {
        self.area_3d_chart_mut()
    }

    pub fn set_area_3d_chart(&mut self, value: Area3DChart) -> &mut Self {
        self.set_chart_group(ChartGroup::Area3DChart(value))
    }

    #[must_use]
    pub fn of_pie_chart(&self) -> Option<&OfPieChart> {
        self.of_pie_chart.as_ref()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use of_pie_chart()")]
    pub fn get_of_pie_chart(&self) -> Option<&OfPieChart> {
        self.of_pie_chart()
    }

    pub fn of_pie_chart_mut(&mut self) -> Option<&mut OfPieChart> {
        self.of_pie_chart.as_mut()
    }

    #[deprecated(since = "3.0.0", note = "Use of_pie_chart_mut()")]
    pub fn get_of_pie_chart_mut(&mut self) -> Option<&mut OfPieChart> {
        self.of_pie_chart_mut()
    }

    pub fn set_of_pie_chart(&mut self, value: OfPieChart) -> &mut Self {
        self.set_chart_group(ChartGroup::OfPieChart(value))
    }

    #[must_use]
    pub fn category_axis(&self) -> &[CategoryAxis] {
        &self.category_axis
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use category_axis()")]
    pub fn get_category_axis(&self) -> &[CategoryAxis] {
        self.category_axis()
    }

    pub fn category_axis_mut(&mut self) -> &mut Vec<CategoryAxis> {
        &mut self.category_axis
    }

    #[deprecated(since = "3.0.0", note = "Use category_axis_mut()")]
    pub fn get_category_axis_mut(&mut self) -> &mut Vec<CategoryAxis> {
        self.category_axis_mut()
    }

    pub fn set_category_axis(&mut self, value: impl Into<Vec<CategoryAxis>>) -> &mut Self {
        self.category_axis = value.into();
        self
    }

    pub fn add_category_axis(&mut self, value: CategoryAxis) -> &mut Self {
        self.category_axis.push(value);
        self
    }

    #[must_use]
    pub fn date_axis(&self) -> &[DateAxis] {
        &self.date_axis
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use date_axis()")]
    pub fn get_date_axis(&self) -> &[DateAxis] {
        self.date_axis()
    }

    pub fn date_axis_mut(&mut self) -> &mut Vec<DateAxis> {
        &mut self.date_axis
    }

    #[deprecated(since = "3.0.0", note = "Use date_axis_mut()")]
    pub fn get_date_axis_mut(&mut self) -> &mut Vec<DateAxis> {
        self.date_axis_mut()
    }

    pub fn set_date_axis(&mut self, value: impl Into<Vec<DateAxis>>) -> &mut Self {
        self.date_axis = value.into();
        self
    }

    pub fn add_date_axis(&mut self, value: DateAxis) -> &mut Self {
        self.date_axis.push(value);
        self
    }

    #[must_use]
    pub fn value_axis(&self) -> &[ValueAxis] {
        &self.value_axis
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use value_axis()")]
    pub fn get_value_axis(&self) -> &[ValueAxis] {
        self.value_axis()
    }

    pub fn value_axis_mut(&mut self) -> &mut Vec<ValueAxis> {
        &mut self.value_axis
    }

    #[deprecated(since = "3.0.0", note = "Use value_axis_mut()")]
    pub fn get_value_axis_mut(&mut self) -> &mut Vec<ValueAxis> {
        self.value_axis_mut()
    }

    pub fn set_value_axis(&mut self, value: impl Into<Vec<ValueAxis>>) -> &mut Self {
        self.value_axis = value.into();
        self
    }

    pub fn add_value_axis(&mut self, value: ValueAxis) -> &mut Self {
        self.value_axis.push(value);
        self
    }

    #[must_use]
    pub fn series_axis(&self) -> &[SeriesAxis] {
        &self.series_axis
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use series_axis()")]
    pub fn get_series_axis(&self) -> &[SeriesAxis] {
        self.series_axis()
    }

    pub fn series_axis_mut(&mut self) -> &mut Vec<SeriesAxis> {
        &mut self.series_axis
    }

    #[deprecated(since = "3.0.0", note = "Use series_axis_mut()")]
    pub fn get_series_axis_mut(&mut self) -> &mut Vec<SeriesAxis> {
        self.series_axis_mut()
    }

    pub fn set_series_axis(&mut self, value: impl Into<Vec<SeriesAxis>>) -> &mut Self {
        self.series_axis = value.into();
        self
    }

    pub fn add_series_axis(&mut self, value: SeriesAxis) -> &mut Self {
        self.series_axis.push(value);
        self
    }

    #[must_use]
    pub fn shape_properties(&self) -> Option<&ShapeProperties> {
        self.shape_properties.as_ref()
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use shape_properties()")]
    pub fn get_shape_properties(&self) -> Option<&ShapeProperties> {
        self.shape_properties()
    }

    pub fn shape_properties_mut(&mut self) -> Option<&mut ShapeProperties> {
        self.shape_properties.as_mut()
    }

    #[deprecated(since = "3.0.0", note = "Use shape_properties_mut()")]
    pub fn get_shape_properties_mut(&mut self) -> Option<&mut ShapeProperties> {
        self.shape_properties_mut()
    }

    pub fn set_shape_properties(&mut self, value: ShapeProperties) -> &mut Self {
        self.shape_properties = Some(value);
        self
    }

    pub fn set_grouping(&mut self, value: GroupingValues) -> &mut Self {
        if let Some(chart) = &mut self.line_chart {
            chart.grouping_mut().set_val(value);
            return self;
        }
        if let Some(chart) = &mut self.line_3d_chart {
            chart.grouping_mut().set_val(value);
            return self;
        }
        if let Some(chart) = &mut self.bar_chart {
            chart.grouping_mut().set_val(value);
            return self;
        }
        if let Some(chart) = &mut self.bar_3d_chart {
            chart.grouping_mut().set_val(value);
            return self;
        }
        if let Some(chart) = &mut self.area_chart {
            chart.grouping_mut().set_val(value);
            return self;
        }
        if let Some(chart) = &mut self.area_3d_chart {
            chart.grouping_mut().set_val(value);
            return self;
        }
        panic! {"Non-Grouping."};
    }

    pub fn area_chart_series_list_mut(&mut self) -> &mut AreaChartSeriesList {
        if let Some(chart) = &mut self.line_chart {
            return chart.area_chart_series_list_mut();
        }
        if let Some(chart) = &mut self.line_3d_chart {
            return chart.area_chart_series_list_mut();
        }
        if let Some(chart) = &mut self.pie_chart {
            return chart.area_chart_series_list_mut();
        }
        if let Some(chart) = &mut self.pie_3d_chart {
            return chart.area_chart_series_list_mut();
        }
        if let Some(chart) = &mut self.doughnut_chart {
            return chart.area_chart_series_list_mut();
        }
        if let Some(chart) = &mut self.scatter_chart {
            return chart.area_chart_series_list_mut();
        }
        if let Some(chart) = &mut self.bar_chart {
            return chart.area_chart_series_list_mut();
        }
        if let Some(chart) = &mut self.bar_3d_chart {
            return chart.area_chart_series_list_mut();
        }
        if let Some(chart) = &mut self.radar_chart {
            return chart.area_chart_series_list_mut();
        }
        if let Some(chart) = &mut self.bubble_chart {
            return chart.area_chart_series_list_mut();
        }
        if let Some(chart) = &mut self.area_chart {
            return chart.area_chart_series_list_mut();
        }
        if let Some(chart) = &mut self.area_3d_chart {
            return chart.area_chart_series_list_mut();
        }
        if let Some(chart) = &mut self.of_pie_chart {
            return chart.area_chart_series_list_mut();
        }
        panic! {"Non-ChartSeriesList."};
    }

    #[deprecated(since = "3.0.0", note = "Use area_chart_series_list_mut()")]
    pub fn get_area_chart_series_list_mut(&mut self) -> &mut AreaChartSeriesList {
        self.area_chart_series_list_mut()
    }

    pub fn formula_mut(&mut self) -> Vec<&mut Formula> {
        let mut result = Vec::new();
        for group in self.chart_groups_mut() {
            for series in group.series_mut().area_chart_series_mut() {
                result.extend(series.formula_mut());
            }
        }
        result
    }

    #[deprecated(since = "3.0.0", note = "Use formula_mut()")]
    pub fn get_formula_mut(&mut self) -> Vec<&mut Formula> {
        self.formula_mut()
    }

    pub(crate) fn is_support(&self) -> bool {
        self.chart_groups().next().is_some()
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        _e: &BytesStart,
        session: &crate::reader::xlsx::SourceCacheSession,
    ) {
        xml_read_loop!(
            reader,
            Event::Start(ref e) => match local_name(e.name().0) {
                b"layout" => {
                    self.layout.set_attributes(reader, e, false);
                }
                b"lineChart" => {
                    let mut obj = LineChart::default();
                    obj.set_attributes(reader, e, session);
                    self.push_chart_group(ChartGroup::LineChart(obj));
                }
                b"line3DChart" => {
                    let mut obj = Line3DChart::default();
                    obj.set_attributes(reader, e, session);
                    self.push_chart_group(ChartGroup::Line3DChart(obj));
                }
                b"pieChart" => {
                    let mut obj = PieChart::default();
                    obj.set_attributes(reader, e, session);
                    self.push_chart_group(ChartGroup::PieChart(obj));
                }
                b"pie3DChart" => {
                    let mut obj = Pie3DChart::default();
                    obj.set_attributes(reader, e, session);
                    self.push_chart_group(ChartGroup::Pie3DChart(obj));
                }
                b"doughnutChart" => {
                    let mut obj = DoughnutChart::default();
                    obj.set_attributes(reader, e, session);
                    self.push_chart_group(ChartGroup::DoughnutChart(obj));
                }
                b"scatterChart" => {
                    let mut obj = ScatterChart::default();
                    obj.set_attributes(reader, e, session);
                    self.push_chart_group(ChartGroup::ScatterChart(obj));
                }
                b"barChart" => {
                    let mut obj = BarChart::default();
                    obj.set_attributes(reader, e, session);
                    self.push_chart_group(ChartGroup::BarChart(obj));
                }
                b"bar3DChart" => {
                    let mut obj = Bar3DChart::default();
                    obj.set_attributes(reader, e, session);
                    self.push_chart_group(ChartGroup::Bar3DChart(obj));
                }
                b"radarChart" => {
                    let mut obj = RadarChart::default();
                    obj.set_attributes(reader, e, session);
                    self.push_chart_group(ChartGroup::RadarChart(obj));
                }
                b"bubbleChart" => {
                    let mut obj = BubbleChart::default();
                    obj.set_attributes(reader, e, session);
                    self.push_chart_group(ChartGroup::BubbleChart(obj));
                }
                b"areaChart" => {
                    let mut obj = AreaChart::default();
                    obj.set_attributes(reader, e, session);
                    self.push_chart_group(ChartGroup::AreaChart(obj));
                }
                b"area3DChart" => {
                    let mut obj = Area3DChart::default();
                    obj.set_attributes(reader, e, session);
                    self.push_chart_group(ChartGroup::Area3DChart(obj));
                }
                b"ofPieChart" => {
                    let mut obj = OfPieChart::default();
                    obj.set_attributes(reader, e, session);
                    self.push_chart_group(ChartGroup::OfPieChart(obj));
                }
                b"catAx" => {
                    let mut obj = CategoryAxis::default();
                    obj.set_attributes(reader, e, session);
                    self.add_category_axis(obj);
                }
                b"dateAx" => {
                    let mut obj = DateAxis::default();
                    obj.set_attributes(reader, e, session);
                    self.add_date_axis(obj);
                }
                b"valAx" => {
                    let mut obj = ValueAxis::default();
                    obj.set_attributes(reader, e, session);
                    self.add_value_axis(obj);
                }
                b"serAx" => {
                    let mut obj = SeriesAxis::default();
                    obj.set_attributes(reader, e, session);
                    self.add_series_axis(obj);
                }
                b"spPr" => {
                    let mut obj = ShapeProperties::default();
                    obj.set_attributes(reader, e);
                    self.set_shape_properties(obj);
                }
                _ => skip_element(reader, e),
            },
            Event::End(ref e) => {
                if local_name(e.name().0) == b"plotArea" {
                    self.normalize_chart_groups();
                    return;
                }
            },
            Event::Eof => panic!("Error: Could not find {} end element", "c:plotArea"),
        );
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>, wb: &Workbook) {
        // c:plotArea
        write_start_tag(writer, "c:plotArea", vec![], false);

        // c:layout
        self.layout.write_to(writer);

        for group in self.chart_groups() {
            group.write_to(writer, wb);
        }

        // c:catAx
        for v in &self.category_axis {
            v.write_to(writer, wb);
        }

        // c:dateAx
        for v in &self.date_axis {
            v.write_to(writer, wb);
        }

        // c:valAx
        for v in &self.value_axis {
            v.write_to(writer, wb);
        }

        // c:serAx
        for v in &self.series_axis {
            v.write_to(writer, wb);
        }

        // c:spPr
        if let Some(v) = &self.shape_properties {
            v.write_to(writer);
        }

        write_end_tag(writer, "c:plotArea");
    }
}
impl AdjustmentCoordinateWithSheet for PlotArea {
    fn adjustment_insert_coordinate_with_sheet(
        &mut self,
        sheet_name: &str,
        root_col_num: u32,
        offset_col_num: u32,
        root_row_num: u32,
        offset_row_num: u32,
    ) {
        for formula in self.formula_mut() {
            formula.adjustment_insert_coordinate_with_sheet(
                sheet_name,
                root_col_num,
                offset_col_num,
                root_row_num,
                offset_row_num,
            );
        }
    }

    fn adjustment_remove_coordinate_with_sheet(
        &mut self,
        sheet_name: &str,
        root_col_num: u32,
        offset_col_num: u32,
        root_row_num: u32,
        offset_row_num: u32,
    ) {
        for formula in self.formula_mut() {
            formula.adjustment_remove_coordinate_with_sheet(
                sheet_name,
                root_col_num,
                offset_col_num,
                root_row_num,
                offset_row_num,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drawing::charts::{
        AreaChartSeries,
        Values,
    };

    #[test]
    fn removal_adjusts_formulas_in_every_container() {
        let mut plot = PlotArea::default();
        for group in [
            ChartGroup::LineChart(LineChart::default()),
            ChartGroup::Line3DChart(Line3DChart::default()),
            ChartGroup::PieChart(PieChart::default()),
            ChartGroup::Pie3DChart(Pie3DChart::default()),
            ChartGroup::DoughnutChart(DoughnutChart::default()),
            ChartGroup::ScatterChart(ScatterChart::default()),
            ChartGroup::BarChart(BarChart::default()),
            ChartGroup::Bar3DChart(Bar3DChart::default()),
            ChartGroup::RadarChart(RadarChart::default()),
            ChartGroup::BubbleChart(BubbleChart::default()),
            ChartGroup::AreaChart(AreaChart::default()),
            ChartGroup::Area3DChart(Area3DChart::default()),
            ChartGroup::OfPieChart(OfPieChart::default()),
            ChartGroup::LineChart(LineChart::default()),
        ] {
            plot.add_chart_group(group);
        }
        for group in plot.chart_groups_mut() {
            let mut values = Values::default();
            values
                .number_reference_mut()
                .formula_mut()
                .set_address_str("Sheet1!$B$3:$B$4");
            let mut series = AreaChartSeries::default();
            series.set_values(values);
            group.series_mut().add_area_chart_series(series);
        }
        plot.adjustment_remove_coordinate_with_sheet("Sheet1", 0, 0, 2, 1);
        let formulas = plot.formula_mut();
        assert_eq!(formulas.len(), 14);
        for formula in formulas {
            assert_eq!(formula.address_str(), "Sheet1!$B$2:$B$3");
        }
    }
}
