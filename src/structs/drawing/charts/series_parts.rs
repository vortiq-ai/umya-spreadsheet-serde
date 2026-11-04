use super::{
    DataPoint,
    ErrorBars,
    Trendline,
};
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
/// Ordered point overrides and trendlines, with their native child owners.
pub struct SeriesParts {
    data_points: Vec<DataPoint>,
    #[cfg_attr(feature = "serde", serde(default))]
    trendlines:  Vec<Trendline>,
    #[cfg_attr(feature = "serde", serde(default))]
    error_bars:  Vec<ErrorBars>,
    #[cfg_attr(feature = "serde", serde(default))]
    extensions:  Option<Box<super::ExtensionList>>,
}
impl SeriesParts {
    pub(crate) fn reference_role_parts(
        &self,
    ) -> (
        &[DataPoint],
        &[Trendline],
        &[ErrorBars],
        Option<&super::ExtensionList>,
    ) {
        (
            &self.data_points,
            &self.trendlines,
            &self.error_bars,
            self.extensions.as_deref(),
        )
    }

    pub(crate) fn reference_role_parts_mut(
        &mut self,
    ) -> (
        &mut [DataPoint],
        &mut [Trendline],
        &mut [ErrorBars],
        Option<&mut super::ExtensionList>,
    ) {
        (
            &mut self.data_points,
            &mut self.trendlines,
            &mut self.error_bars,
            self.extensions.as_deref_mut(),
        )
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
    pub fn error_bars(&self) -> &[ErrorBars] {
        &self.error_bars
    }

    pub fn error_bars_mut(&mut self) -> &mut Vec<ErrorBars> {
        &mut self.error_bars
    }

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
