//! Excel's insertion defaults for new `ChartEx` charts: the chart style and
//! colour parts Excel requires beside every `ChartEx` part, and the
//! markup-compatibility namespace its drawing frame requires.
//!
//! Excel refuses a native extended chart whose part has no `chartStyle` and
//! `chartColorStyle` dependencies. The payloads below are the unmodified
//! parts Excel itself writes when it inserts each chart kind with its default
//! style and the default colourful palette (style ids 366/381/395/406/410/
//! 419/494, colour id 10). They are attached only to charts the package
//! writer creates without an imported owner; imported dependencies are
//! preserved as they are.
use super::{
    ChartExSeriesLayout,
    ChartExSpace,
};

/// Style and colour payloads for one chart kind.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ChartExStyleParts {
    pub(crate) style:  &'static [u8],
    pub(crate) colors: &'static [u8],
}

const COLORS: &[u8] = include_bytes!("style_parts/colors-10.xml");

impl ChartExStyleParts {
    fn for_layout(layout: ChartExSeriesLayout) -> Self {
        let style: &'static [u8] = match layout {
            // Histogram and Pareto share Excel's clustered-column default.
            ChartExSeriesLayout::ClusteredColumn | ChartExSeriesLayout::ParetoLine => {
                include_bytes!("style_parts/style-366.xml")
            }
            ChartExSeriesLayout::Sunburst => include_bytes!("style_parts/style-381.xml"),
            ChartExSeriesLayout::Waterfall => include_bytes!("style_parts/style-395.xml"),
            ChartExSeriesLayout::BoxWhisker => include_bytes!("style_parts/style-406.xml"),
            ChartExSeriesLayout::Treemap => include_bytes!("style_parts/style-410.xml"),
            ChartExSeriesLayout::Funnel => include_bytes!("style_parts/style-419.xml"),
            ChartExSeriesLayout::RegionMap => include_bytes!("style_parts/style-494.xml"),
        };
        Self {
            style,
            colors: COLORS,
        }
    }
}

const CX1: (&str, &str) = (
    "cx1",
    "http://schemas.microsoft.com/office/drawing/2015/9/8/chartex",
);
const CX2: (&str, &str) = (
    "cx2",
    "http://schemas.microsoft.com/office/drawing/2015/10/21/chartex",
);
const CX4: (&str, &str) = (
    "cx4",
    "http://schemas.microsoft.com/office/drawing/2016/5/10/chartex",
);

impl ChartExSpace {
    /// The `mc:Choice` `Requires` namespace Excel writes around a newly
    /// inserted chart of this kind (observed in Excel-authored drawings).
    pub(crate) fn insertion_namespace(&self) -> Option<(&'static str, &'static str)> {
        let layout = self.chart()?.series().first()?.layout_id;
        Some(match layout {
            ChartExSeriesLayout::Funnel => CX2,
            ChartExSeriesLayout::RegionMap => CX4,
            _ => CX1,
        })
    }

    /// The parts Excel writes for a newly inserted chart of this kind. A
    /// space without any series (or a dashboard) has no Excel default.
    pub(crate) fn default_style_parts(&self) -> Option<ChartExStyleParts> {
        self.chart()?
            .series()
            .first()
            .map(|series| ChartExStyleParts::for_layout(series.layout_id))
    }
}
