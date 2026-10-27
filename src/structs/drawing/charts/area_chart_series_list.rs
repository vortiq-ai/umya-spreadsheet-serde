use super::{
    AreaChartSeries,
    SeriesFamily,
    SeriesPartRemoval,
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct AreaChartSeriesList {
    area_chart_series: Vec<AreaChartSeries>,
}
impl AreaChartSeriesList {
    #[must_use]
    pub fn area_chart_series(&self) -> &[AreaChartSeries] {
        &self.area_chart_series
    }

    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use area_chart_series()")]
    pub fn get_area_chart_series(&self) -> &[AreaChartSeries] {
        self.area_chart_series()
    }

    pub fn area_chart_series_mut(&mut self) -> &mut [AreaChartSeries] {
        &mut self.area_chart_series
    }

    #[deprecated(since = "3.0.0", note = "Use area_chart_series_mut()")]
    pub fn get_area_chart_series_mut(&mut self) -> &mut [AreaChartSeries] {
        self.area_chart_series_mut()
    }

    pub fn set_area_chart_series(&mut self, value: impl Into<Vec<AreaChartSeries>>) -> &mut Self {
        self.area_chart_series = value.into();
        self
    }

    pub fn add_area_chart_series(&mut self, value: AreaChartSeries) -> &mut Self {
        self.area_chart_series.push(value);
        self
    }

    /// Removes from every series the parts `family`'s series type cannot
    /// hold, for an explicit change of the group's series type, and returns
    /// what was removed. Indices, order, names, formatting, data labels and
    /// data references stay.
    pub fn remove_parts_not_allowed_in(&mut self, family: SeriesFamily) -> Vec<SeriesPartRemoval> {
        let mut removed = Vec::new();
        for (series_index, series) in self.area_chart_series.iter_mut().enumerate() {
            removed.extend(series.remove_parts_not_allowed_in(family).into_iter().map(
                |(part, occurrence)| SeriesPartRemoval {
                    series_index,
                    part,
                    occurrence,
                },
            ));
        }
        removed
    }
}
