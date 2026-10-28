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
    pub(crate) fn formulas_mut(&mut self) -> Vec<&mut super::Formula> {
        let mut result = Vec::new();
        for v in &mut self.data_points {
            result.extend(v.formulas_mut());
        }
        for v in &mut self.trendlines {
            result.extend(v.formulas_mut());
        }
        for v in &mut self.error_bars {
            result.extend(v.formulas_mut());
        }
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
