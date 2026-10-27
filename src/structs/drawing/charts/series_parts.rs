use super::{
    DataPoint,
    Trendline,
};
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
/// Ordered point overrides and trendlines, with their native child owners.
pub struct SeriesParts {
    data_points: Vec<DataPoint>,
    #[cfg_attr(feature = "serde", serde(default))]
    trendlines:  Vec<Trendline>,
}
impl SeriesParts {
    #[must_use]
    pub fn trendlines(&self) -> &[Trendline] {
        &self.trendlines
    }

    pub fn trendlines_mut(&mut self) -> &mut Vec<Trendline> {
        &mut self.trendlines
    }

    #[must_use]
    pub fn data_points(&self) -> &[DataPoint] {
        &self.data_points
    }

    pub fn data_points_mut(&mut self) -> &mut Vec<DataPoint> {
        &mut self.data_points
    }
}
