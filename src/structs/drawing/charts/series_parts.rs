use super::DataPoint;
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
/// Ordered point overrides. Other series supplements are added by their own
/// owners.
pub struct SeriesParts {
    data_points: Vec<DataPoint>,
}
impl SeriesParts {
    #[must_use]
    pub fn data_points(&self) -> &[DataPoint] {
        &self.data_points
    }

    pub fn data_points_mut(&mut self) -> &mut Vec<DataPoint> {
        &mut self.data_points
    }
}
