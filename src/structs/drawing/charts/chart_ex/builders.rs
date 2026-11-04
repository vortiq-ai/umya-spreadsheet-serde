//! Construction helpers for native `ChartEx` layouts.
//!
//! These helpers use the MS-ODRAWXML layout/data model and leave optional
//! formatting to the application theme. The package writer adds the chart style
//! and colour parts Excel inserts with the same kind of chart, and
//! `ExtendedChart::new` wraps the frame in the compatibility namespace Excel
//! writes for it.
use std::fmt::Write as _;

use super::{
    ChartExAutomaticNumber,
    ChartExAxis,
    ChartExAxisUnit,
    ChartExAxisUnits,
    ChartExBinning,
    ChartExCategoryAxisScaling,
    ChartExChart,
    ChartExChartTitle,
    ChartExData,
    ChartExDataCollection,
    ChartExDimension,
    ChartExDimensionSource,
    ChartExFormula,
    ChartExGapWidth,
    ChartExGeography,
    ChartExGridlines,
    ChartExNumericDimension,
    ChartExNumericDimensionType,
    ChartExNumericReference,
    ChartExParentLabelLayout,
    ChartExParentLabelProperties,
    ChartExPlotArea,
    ChartExPlotAreaRegion,
    ChartExPosAlign,
    ChartExQuartileMethod,
    ChartExSeries,
    ChartExSeriesElementVisibilities,
    ChartExSeriesLayout,
    ChartExSidePos,
    ChartExSpace,
    ChartExStatistics,
    ChartExStringDimension,
    ChartExStringDimensionType,
    ChartExStringReference,
    ChartExText,
    ChartExTextData,
    ChartExTickLabels,
    ChartExValueAxisScaling,
};
use crate::{
    DefinedName,
    Workbook,
    drawing::charts::XmlDouble,
};

/// Creates a native extended chart with references to workbook data.
///
/// A category formula can cover several columns (hierarchy levels), or rows
/// when its direction is `ChartExFormulaDirection::Row`. Values are left as
/// source data; Excel performs binning and statistical calculations.
#[derive(Clone, Debug)]
pub struct ChartExBuilder {
    space: ChartExSpace,
    error: Option<String>,
}

impl ChartExBuilder {
    #[must_use]
    pub fn sunburst(categories: ChartExFormula, values: ChartExFormula) -> Self {
        Self::categorical(
            ChartExSeriesLayout::Sunburst,
            categories,
            values,
            ChartExNumericDimensionType::Size,
        )
    }

    #[must_use]
    pub fn treemap(
        categories: ChartExFormula,
        values: ChartExFormula,
        parent_labels: ChartExParentLabelLayout,
    ) -> Self {
        let mut builder = Self::categorical(
            ChartExSeriesLayout::Treemap,
            categories,
            values,
            ChartExNumericDimensionType::Size,
        );
        builder.series().layout_properties_mut().parent_label_layout =
            Some(ChartExParentLabelProperties {
                value: parent_labels,
            });
        builder
    }

    #[must_use]
    pub fn waterfall(
        categories: ChartExFormula,
        values: ChartExFormula,
        subtotals: impl IntoIterator<Item = u32>,
        connector_lines: bool,
    ) -> Self {
        let mut builder = Self::categorical(
            ChartExSeriesLayout::Waterfall,
            categories,
            values,
            ChartExNumericDimensionType::Value,
        );
        let properties = builder.series().layout_properties_mut();
        properties.set_subtotal_indices(subtotals);
        properties.visibility = Some(ChartExSeriesElementVisibilities {
            connector_lines: Some(connector_lines),
            ..Default::default()
        });
        builder
    }

    #[must_use]
    pub fn histogram(values: ChartExFormula, binning: ChartExBinning) -> Self {
        let mut builder = Self::numeric(ChartExSeriesLayout::ClusteredColumn, values);
        builder.series().layout_properties_mut().binning = Some(binning);
        builder
    }

    /// Creates histogram columns and their cumulative Pareto line.
    #[must_use]
    pub fn pareto(values: ChartExFormula, binning: ChartExBinning) -> Self {
        let mut builder = Self::histogram(values, binning);
        builder.series().axis_ids = vec![1];
        let mut line = ChartExSeries {
            layout_id: ChartExSeriesLayout::ParetoLine,
            ..Default::default()
        };
        line.owner_idx = Some(0);
        line.axis_ids = vec![2];
        let chart = builder.space.chart_mut().expect("constructed chart");
        chart.series_mut().push(line);
        let mut axis = value_axis(2);
        axis.major_gridlines = None;
        axis.value_scaling = Some(ChartExValueAxisScaling {
            min: Some(number(0.0)),
            max: Some(number(1.0)),
            ..Default::default()
        });
        axis.units = Some(ChartExAxisUnits {
            unit: Some(ChartExAxisUnit::Percentage),
            ..Default::default()
        });
        chart.axes_mut().push(axis);
        builder
    }

    #[must_use]
    /// Each formula identifies the observations for one box-and-whisker group.
    /// An empty group list is rejected by `build`.
    pub fn box_whisker(
        values: impl IntoIterator<Item = ChartExFormula>,
        quartile_method: ChartExQuartileMethod,
        visibility: &ChartExSeriesElementVisibilities,
    ) -> Self {
        let mut builder = Self::with_dimensions(ChartExSeriesLayout::BoxWhisker, Vec::new());
        let collection = builder.space.data_mut().expect("constructed data");
        collection.data.clear();
        let mut series = Vec::new();
        for formula in values {
            let Ok(id) = u32::try_from(collection.data.len()) else {
                builder.error = Some("ChartEx has too many box-and-whisker groups".into());
                return builder;
            };
            collection.data.push(ChartExData {
                id,
                dimensions: vec![ChartExDimension::Numeric(ChartExNumericDimension {
                    kind:   ChartExNumericDimensionType::Value,
                    source: ChartExDimensionSource::Reference(ChartExNumericReference::new(
                        formula,
                    )),
                })],
                ..Default::default()
            });
            let mut group = ChartExSeries::new(ChartExSeriesLayout::BoxWhisker, id);
            let properties = group.layout_properties_mut();
            properties.statistics = Some(ChartExStatistics {
                quartile_method: Some(quartile_method),
            });
            properties.visibility = Some(visibility.clone());
            series.push(group);
        }
        *builder
            .space
            .chart_mut()
            .expect("constructed chart")
            .series_mut() = series;
        builder
    }

    #[must_use]
    pub fn funnel(categories: ChartExFormula, values: ChartExFormula) -> Self {
        Self::categorical(
            ChartExSeriesLayout::Funnel,
            categories,
            values,
            ChartExNumericDimensionType::Value,
        )
    }

    /// Creates a region (filled) map. Excel looks the regions up with its
    /// geography provider and stores the result in the chart on save, so no
    /// geography cache is written here. Culture names are required (for
    /// example `"en-US"` and `"US"`).
    #[must_use]
    pub fn region_map(
        categories: ChartExFormula,
        values: ChartExFormula,
        culture_language: impl Into<String>,
        culture_region: impl Into<String>,
    ) -> Self {
        let mut builder = Self::categorical(
            ChartExSeriesLayout::RegionMap,
            categories,
            values,
            ChartExNumericDimensionType::ColorVal,
        );
        let (language, region) = (culture_language.into(), culture_region.into());
        if [&language, &region]
            .iter()
            .any(|name| name.is_empty() || name.trim() != name.as_str())
        {
            builder.error = Some(
                "ChartEx region map needs a culture language and region without surrounding                  whitespace"
                    .into(),
            );
        }
        builder.series().layout_properties_mut().geography = Some(ChartExGeography {
            culture_language: language,
            culture_region: region,
            attribution: "Powered by Bing".into(),
            ..Default::default()
        });
        builder
    }

    #[must_use]
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.space.chart_mut().expect("constructed chart").title = Some(ChartExChartTitle {
            position: Some(ChartExSidePos::Top),
            align: Some(ChartExPosAlign::Center),
            overlay: Some(false),
            text: Some(literal_text(title)),
            ..Default::default()
        });
        self
    }

    /// Names the chart's data series. A chart with several data groups (box
    /// and whisker) has no single series to name; `build` reports that.
    #[must_use]
    pub fn series_name(mut self, name: impl Into<String>) -> Self {
        let chart = self.space.chart_mut().expect("constructed chart");
        let mut data_series = chart
            .series_mut()
            .iter_mut()
            .filter(|series| series.data_id.is_some());
        match (data_series.next(), data_series.next()) {
            (Some(series), None) => series.text = Some(literal_text(name)),
            (Some(_), Some(_)) => {
                self.error.get_or_insert_with(|| {
                    "ChartEx series_name needs a chart with one data series".into()
                });
            }
            _ => (),
        }
        self
    }

    /// Produces a detached chart using the supplied formulas directly.
    pub fn build(mut self) -> Result<ChartExSpace, String> {
        if let Some(error) = self.error {
            return Err(error);
        }
        self.space.validate()?;
        for series in self.space.chart().expect("constructed chart").series() {
            if let Some(binning) = series
                .layout_properties
                .as_ref()
                .and_then(|properties| properties.binning.as_ref())
            {
                if binning.bin_count.is_some() && binning.bin_size.is_some() {
                    return Err("ChartEx binning needs either bin count or bin size".into());
                }
                if binning.bin_count == Some(0) {
                    return Err("ChartEx bin count must be positive".into());
                }
                if binning.bin_size.as_ref().is_some_and(|size| {
                    size.value()
                        .is_none_or(|value| !value.is_finite() || value <= 0.0)
                }) {
                    return Err("ChartEx bin size must be finite and positive".into());
                }
            }
        }
        // Excel writes a unique id for every series it inserts.
        for (index, series) in self
            .space
            .chart_mut()
            .expect("constructed chart")
            .series_mut()
            .iter_mut()
            .enumerate()
        {
            if series.unique_id.is_none() {
                let mut random = [0_u8; 12];
                rand::fill(&mut random);
                let hex = random.iter().fold(String::new(), |mut hex, byte| {
                    let _ = write!(hex, "{byte:02X}");
                    hex
                });
                series.unique_id = Some(format!(
                    "{{{:08X}-{}-{}-{}-{}}}",
                    index + 1,
                    &hex[0..4],
                    &hex[4..8],
                    &hex[8..12],
                    &hex[12..24]
                ));
            }
        }
        let mut empty_formula = false;
        self.space.visit_formulas(&mut |formula| {
            empty_formula |= formula.address_str().trim().is_empty();
        });
        if empty_formula {
            return Err("ChartEx source formula cannot be empty".into());
        }
        Ok(self.space)
    }

    /// Registers the supplied formulas as hidden workbook names, following
    /// Excel's `_xlchart.vN.i` convention. Existing names are never replaced:
    /// the index continues after the highest `_xlchart` index in any scope.
    /// Sources must be sheet-qualified absolute cell/range references; relative
    /// formulas and names remain available through `build` instead.
    /// Validation happens before any workbook mutation.
    pub fn build_with_defined_names(self, workbook: &mut Workbook) -> Result<ChartExSpace, String> {
        let mut space = self.build()?;
        let mut qualified = true;
        space.visit_formulas(&mut |formula| {
            let text = formula.address_str();
            let (sheet, range) = crate::helper::address::split_reference_address(
                text.trim().trim_start_matches('='),
            );
            qualified &= !sheet.is_empty()
                && !sheet.contains(['[', ']'])
                && super::super::reference_edit_engine::is_native_coordinate_operand(range)
                && range.split(':').all(|cell| {
                    let (_, _, column_lock, row_lock) =
                        crate::helper::coordinate::index_from_coordinate(cell.to_ascii_uppercase());
                    column_lock != Some(false) && row_lock != Some(false)
                });
        });
        if !qualified {
            return Err(
                "ChartEx named creation requires sheet-qualified absolute cell/range sources"
                    .into(),
            );
        }
        // Excel numbers these names with one workbook-wide counter and puts
        // the feature level of the chart kind in the version (v1 for the
        // original chart kinds, v2 for funnel, v5 for region maps).
        let version = match space.insertion_namespace() {
            Some(("cx2", _)) => 2_u32,
            Some(("cx4", _)) => 5,
            _ => 1,
        };
        let mut next = 0_u32;
        for name in workbook.defined_names().iter().chain(
            workbook
                .sheet_collection_no_check()
                .iter()
                .flat_map(crate::Worksheet::defined_names),
        ) {
            let lower = name.name().to_ascii_lowercase();
            if let Some(index) = lower
                .strip_prefix("_xlchart.v")
                .and_then(|rest| rest.split_once('.'))
                .and_then(|(_, index)| index.parse::<u32>().ok())
            {
                next = next.max(
                    index
                        .checked_add(1)
                        .ok_or("ChartEx defined name namespace exhausted")?,
                );
            }
        }
        let mut definitions = Vec::new();
        let mut failure = None;
        space.visit_formulas_mut(&mut |formula| {
            let Some(index) = u32::try_from(definitions.len())
                .ok()
                .and_then(|count| next.checked_add(count))
            else {
                failure = Some("ChartEx defined name namespace exhausted");
                return;
            };
            let name = format!("_xlchart.v{version}.{index}");
            let mut definition = DefinedName::default();
            definition
                .set_name(&name)
                .set_address(formula.address_str());
            definition.set_hidden(true);
            formula.set_string_value(name);
            definitions.push(definition);
        });
        if let Some(message) = failure {
            return Err(message.into());
        }
        for definition in definitions {
            workbook.add_defined_names(definition);
        }
        Ok(space)
    }

    fn series(&mut self) -> &mut ChartExSeries {
        &mut self
            .space
            .chart_mut()
            .expect("constructed chart")
            .series_mut()[0]
    }

    fn categorical(
        layout: ChartExSeriesLayout,
        categories: ChartExFormula,
        values: ChartExFormula,
        value_kind: ChartExNumericDimensionType,
    ) -> Self {
        Self::with_dimensions(
            layout,
            vec![
                ChartExDimension::String(ChartExStringDimension {
                    kind:   ChartExStringDimensionType::Category,
                    source: ChartExDimensionSource::Reference(ChartExStringReference::new(
                        categories,
                    )),
                }),
                ChartExDimension::Numeric(ChartExNumericDimension {
                    kind:   value_kind,
                    source: ChartExDimensionSource::Reference(ChartExNumericReference::new(values)),
                }),
            ],
        )
    }

    fn numeric(layout: ChartExSeriesLayout, values: ChartExFormula) -> Self {
        Self::with_dimensions(
            layout,
            vec![ChartExDimension::Numeric(ChartExNumericDimension {
                kind:   ChartExNumericDimensionType::Value,
                source: ChartExDimensionSource::Reference(ChartExNumericReference::new(values)),
            })],
        )
    }

    fn with_dimensions(layout: ChartExSeriesLayout, dimensions: Vec<ChartExDimension>) -> Self {
        let series = ChartExSeries::new(layout, 0);
        let axis = if matches!(
            layout,
            ChartExSeriesLayout::Waterfall
                | ChartExSeriesLayout::ClusteredColumn
                | ChartExSeriesLayout::BoxWhisker
        ) {
            vec![
                ChartExAxis {
                    id: 0,
                    category_scaling: Some(ChartExCategoryAxisScaling {
                        gap_width: Some(ChartExGapWidth(number(match layout {
                            ChartExSeriesLayout::ClusteredColumn => 0.0,
                            ChartExSeriesLayout::BoxWhisker => 1.0,
                            _ => 0.5,
                        }))),
                    }),
                    tick_labels: Some(ChartExTickLabels::default()),
                    ..Default::default()
                },
                value_axis(1),
            ]
        } else if layout == ChartExSeriesLayout::Funnel {
            vec![ChartExAxis {
                id: 0,
                category_scaling: Some(ChartExCategoryAxisScaling::default()),
                tick_labels: Some(ChartExTickLabels::default()),
                ..Default::default()
            }]
        } else {
            Vec::new()
        };
        Self {
            error: None,
            space: ChartExSpace::new(
                ChartExDataCollection {
                    data: vec![ChartExData {
                        id: 0,
                        dimensions,
                        ..Default::default()
                    }],
                    ..Default::default()
                },
                ChartExChart {
                    plot_area: Some(ChartExPlotArea {
                        plot_area_region: Some(ChartExPlotAreaRegion {
                            series: vec![series],
                            ..Default::default()
                        }),
                        axis,
                        ..Default::default()
                    }),
                    ..Default::default()
                },
            ),
        }
    }
}

fn literal_text(text: impl Into<String>) -> ChartExText {
    ChartExText {
        tx_data: Some(ChartExTextData::literal(text)),
        rich:    None,
    }
}

fn value_axis(id: u32) -> ChartExAxis {
    ChartExAxis {
        id,
        value_scaling: Some(ChartExValueAxisScaling::default()),
        major_gridlines: Some(ChartExGridlines::default()),
        tick_labels: Some(ChartExTickLabels::default()),
        ..Default::default()
    }
}

fn number(value: f64) -> ChartExAutomaticNumber {
    let mut number = XmlDouble::default();
    number.set_value(value);
    ChartExAutomaticNumber::Number(number)
}
