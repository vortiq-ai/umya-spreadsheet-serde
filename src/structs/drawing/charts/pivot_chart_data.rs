//! Snapshot a `PivotChart` before its pivot table disappears.
use super::{
    AreaChartSeries,
    CategoryAxisData,
    ChartSpace,
    ChartText,
    NumericData,
    NumericPoint,
    StringLiteral,
    StringPoint,
    Values,
    XmlUnsignedInt,
};
use crate::{
    Workbook,
    XlsxError,
};

pub(crate) struct FrozenPivotChart(Vec<Vec<FrozenSeries>>);
struct FrozenSeries {
    text:       Option<ChartText>,
    categories: Option<CategoryAxisData>,
    values:     Option<Values>,
}

fn index(value: u32) -> XmlUnsignedInt {
    let mut result = XmlUnsignedInt::default();
    result.set_val(value);
    result
}

impl FrozenPivotChart {
    pub(crate) fn new(space: &ChartSpace, book: &Workbook) -> Result<Self, XlsxError> {
        Ok(Self(
            space
                .chart()
                .plot_area()
                .chart_groups()
                .map(|group| {
                    group
                        .series()
                        .area_chart_series()
                        .iter()
                        .map(|series| FrozenSeries::new(series, book))
                        .collect::<Result<Vec<_>, XlsxError>>()
                })
                .collect::<Result<Vec<_>, XlsxError>>()?,
        ))
    }

    pub(crate) fn apply(self, space: &mut ChartSpace) {
        for (group, frozen) in space
            .chart_mut()
            .plot_area_mut()
            .chart_groups_mut()
            .zip(self.0)
        {
            for (series, frozen) in group
                .series_mut()
                .area_chart_series_mut()
                .iter_mut()
                .zip(frozen)
            {
                if let Some(text) = frozen.text {
                    series.set_chart_text(text);
                }
                if let Some(categories) = frozen.categories {
                    series.set_category_axis_data(categories);
                }
                if let Some(values) = frozen.values {
                    series.set_values(values);
                }
            }
        }
        space.remove_pivot_source();
    }
}

impl FrozenSeries {
    fn new(series: &AreaChartSeries, book: &Workbook) -> Result<Self, XlsxError> {
        let text = series
            .chart_text()
            .map(|text| -> Result<_, XlsxError> {
                let Some(reference) = text.string_reference() else {
                    return Ok(text.clone());
                };
                let value = if let Some(value) = reference.cached_value() {
                    let mut literal = super::NumericValue::default();
                    literal.set_text(value);
                    literal
                } else {
                    let literal =
                        reference
                            .string_cache()
                            .freeze(reference.formula(), book, None)?;
                    literal
                        .string_point_list()
                        .first()
                        .ok_or_else(|| {
                            crate::xml::xml_error("PivotChart series name has no cached point")
                        })?
                        .numeric_value()
                        .clone()
                };
                let mut result = ChartText::default();
                result.set_numeric_value(value);
                Ok(result)
            })
            .transpose()?;
        // Pivot ranges can include totals that are absent from plotted data.
        let point_limit = series
            .values()
            .and_then(|values| values.number_reference().pivot_cache())
            .and_then(|cache| cache.point_count())
            .and_then(XmlUnsignedInt::val);
        let categories = series
            .category_axis_data()
            .map(|categories| -> Result<_, XlsxError> {
                let literal = if let Some(reference) = categories.string_reference() {
                    reference
                        .string_cache()
                        .freeze(reference.formula(), book, point_limit)?
                } else if let Some(reference) = categories.multi_level_string_reference() {
                    let cache = reference.cache().ok_or_else(|| {
                        crate::xml::xml_error("PivotChart category hierarchy has no cache")
                    })?;
                    let count = cache.point_count().ok_or_else(|| {
                        crate::xml::xml_error("PivotChart category hierarchy has no point count")
                    })?;
                    let mut literal = StringLiteral::default();
                    literal.set_point_count(Some(index(count)));
                    let levels: Vec<_> = cache
                        .levels()
                        .iter()
                        .rev()
                        .map(|level| {
                            let mut points: Vec<_> = level.iter().collect();
                            points.sort_by_key(|point| point.index());
                            points
                        })
                        .collect();
                    let mut positions = vec![0; levels.len()];
                    let mut labels = vec![None; levels.len()];
                    for at in 0..count {
                        for ((level, position), label) in
                            levels.iter().zip(&mut positions).zip(&mut labels)
                        {
                            while let Some(point) =
                                level.get(*position).filter(|point| point.index() <= at)
                            {
                                *label = Some(point.value());
                                *position += 1;
                            }
                        }
                        let label = labels
                            .iter()
                            .copied()
                            .collect::<Option<Vec<_>>>()
                            .ok_or_else(|| {
                                crate::xml::xml_error(
                                    "PivotChart category hierarchy has an unlabelled point",
                                )
                            })?
                            .join(" ");
                        let mut point = StringPoint::default();
                        point.set_index(index(at));
                        point.numeric_value_mut().set_text(label);
                        literal.add_string_point_list(point);
                    }
                    literal
                } else if let Some(reference) = categories.number_reference() {
                    super::StringCache::default().freeze(reference.formula(), book, point_limit)?
                } else {
                    return Ok(categories.clone());
                };
                let mut result = CategoryAxisData::default();
                result.set_string_literal(literal);
                Ok(result)
            })
            .transpose()?;
        let values = series
            .values()
            .map(|values| -> Result<_, XlsxError> {
                if values.number_literal().is_some() {
                    return Ok(values.clone());
                }
                let reference = values.number_reference();
                let literal = if let Some(cache) = reference.pivot_cache() {
                    cache.clone()
                } else {
                    let address = reference.formula().address();
                    let sheet = book.sheet_by_name(
                        &crate::helper::address::unescape_sheet_name(address.sheet_name()),
                    )?;
                    let cells = sheet.cell_value_by_range(&address.range().range());
                    let mut literal = NumericData::default();
                    literal.set_format_code(Some(
                        reference.numbering_cache().format_code().text().to_owned(),
                    ));
                    let count = u32::try_from(cells.len()).map_err(|_| {
                        crate::xml::xml_error("PivotChart has too many numeric points")
                    })?;
                    literal.set_point_count(Some(index(count)));
                    for (at, value) in (0..count).zip(cells) {
                        let mut point = NumericPoint::default();
                        point.index_mut().set_val(at);
                        point.set_value(Some(value.value().into_owned()));
                        literal.points_mut().push(point);
                    }
                    literal
                };
                let mut result = Values::default();
                result.set_number_literal(literal);
                Ok(result)
            })
            .transpose()?;
        Ok(Self {
            text,
            categories,
            values,
        })
    }
}
