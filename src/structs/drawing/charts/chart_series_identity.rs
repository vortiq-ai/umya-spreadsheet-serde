//! Match chart series by identity, never collection position.
//!
//! Positions locate an owner after matching; they are never identity keys.
use std::{
    collections::HashMap,
    hash::Hash,
};

use super::{
    AreaChartSeries,
    PlotArea,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct SeriesPosition {
    pub(super) group:  usize,
    pub(super) series: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SeriesMatch {
    ExplicitIndex(SeriesPosition),
    ValuesFormula(SeriesPosition),
    Ambiguous,
    Unmatched,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct SeriesCorrespondence {
    pub(super) current:            Vec<(SeriesPosition, SeriesMatch)>,
    pub(super) unmatched_previous: Vec<SeriesPosition>,
}

pub(super) fn match_series(current: &PlotArea, previous: &PlotArea) -> SeriesCorrespondence {
    let current = identities(current);
    let previous = identities(previous);
    // Counts belong to the complete original population, never the remaining
    // unmatched owners. An index match cannot make a duplicate formula unique.
    let current_counts = IdentityCounts::new(&current);
    let previous_counts = IdentityCounts::new(&previous);
    let mut matches = vec![SeriesMatch::Unmatched; current.len()];
    let mut used_previous = vec![false; previous.len()];

    // Reserve every higher-priority explicit-index match before allowing any
    // formula fallback, independently of incoming collection order.
    for (current_index, owner) in current.iter().enumerate() {
        let Some(index) = owner.index else {
            continue;
        };
        if let Some(previous_index) =
            unique_match(&current_counts.indices, &previous_counts.indices, &index)
        {
            matches[current_index] = SeriesMatch::ExplicitIndex(previous[previous_index].position);
            used_previous[previous_index] = true;
        }
    }

    for (current_index, owner) in current.iter().enumerate() {
        if matches[current_index] != SeriesMatch::Unmatched {
            continue;
        }
        if let Some(formula) = &owner.formula {
            if let Some(previous_index) =
                unique_match(&current_counts.formulas, &previous_counts.formulas, formula)
            {
                if used_previous[previous_index] {
                    matches[current_index] = SeriesMatch::Ambiguous;
                } else {
                    matches[current_index] =
                        SeriesMatch::ValuesFormula(previous[previous_index].position);
                    used_previous[previous_index] = true;
                }
                continue;
            }
        }
        if owner.index.as_ref().is_some_and(|index| {
            ambiguous(&current_counts.indices, &previous_counts.indices, index)
        }) || owner.formula.as_ref().is_some_and(|formula| {
            ambiguous(&current_counts.formulas, &previous_counts.formulas, formula)
        }) {
            matches[current_index] = SeriesMatch::Ambiguous;
        }
    }

    SeriesCorrespondence {
        current:            current
            .iter()
            .zip(matches)
            .map(|(owner, result)| (owner.position, result))
            .collect(),
        unmatched_previous: previous
            .iter()
            .zip(used_previous)
            .filter_map(|(owner, used)| (!used).then_some(owner.position))
            .collect(),
    }
}

struct SeriesIdentity {
    position: SeriesPosition,
    index:    Option<u32>,
    formula:  Option<String>,
}

fn identities(plot: &PlotArea) -> Vec<SeriesIdentity> {
    let mut result = Vec::new();
    for (group, owner) in plot.chart_groups().enumerate() {
        // Reuse the shared group-owner projection. Hidden filtered-series
        // collections remain omitted metadata, not replacement series.
        for (series, owner) in owner.into_reference_role_parts().series.iter().enumerate() {
            result.push(SeriesIdentity {
                position: SeriesPosition { group, series },
                index:    owner.index().explicit_value(),
                formula:  values_formula(owner),
            });
        }
    }
    result
}

fn values_formula(series: &AreaChartSeries) -> Option<String> {
    let formula = series
        .values()
        .map(|values| values.number_reference().formula())
        .or_else(|| {
            series
                .y_values()
                .map(|values| values.number_reference().formula())
        })?;
    // The native representation already canonicalizes parsed addresses and
    // sheet quoting. Preserve free formula/literal text, workbook identity,
    // sheet spelling and absolute markers; do not reparse or rewrite owners.
    let formula = formula.address_str();
    (!formula.is_empty()).then_some(formula)
}

#[derive(Default)]
struct IdentityCounts {
    indices:  HashMap<u32, Vec<usize>>,
    formulas: HashMap<String, Vec<usize>>,
}

impl IdentityCounts {
    fn new(owners: &[SeriesIdentity]) -> Self {
        let mut counts = Self::default();
        for (position, owner) in owners.iter().enumerate() {
            if let Some(index) = owner.index {
                counts.indices.entry(index).or_default().push(position);
            }
            if let Some(formula) = &owner.formula {
                counts
                    .formulas
                    .entry(formula.clone())
                    .or_default()
                    .push(position);
            }
        }
        counts
    }
}

fn unique_match<K: Eq + Hash>(
    current: &HashMap<K, Vec<usize>>,
    previous: &HashMap<K, Vec<usize>>,
    key: &K,
) -> Option<usize> {
    if current.get(key)?.len() != 1 {
        return None;
    }
    match previous.get(key)?.as_slice() {
        [position] => Some(*position),
        _ => None,
    }
}

fn ambiguous<K: Eq + Hash>(
    current: &HashMap<K, Vec<usize>>,
    previous: &HashMap<K, Vec<usize>>,
    key: &K,
) -> bool {
    current.get(key).is_some_and(|owners| owners.len() > 1)
        || previous.get(key).is_some_and(|owners| owners.len() > 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drawing::charts::{
        BarChart,
        ChartGroup,
        LineChart,
        Values,
        YValues,
    };

    fn series(index: Option<u32>, formula: Option<&str>) -> AreaChartSeries {
        let mut result = AreaChartSeries::default();
        if let Some(index) = index {
            result.index_mut().set_val(index);
        }
        if let Some(formula) = formula {
            let mut values = Values::default();
            values
                .number_reference_mut()
                .formula_mut()
                .set_address_str(formula);
            result.set_values(values);
        }
        result
    }

    fn plot(series: Vec<AreaChartSeries>) -> PlotArea {
        let mut line = LineChart::default();
        line.area_chart_series_list_mut()
            .set_area_chart_series(series);
        let mut result = PlotArea::default();
        result.set_line_chart(line);
        result
    }

    fn position(group: usize, series: usize) -> SeriesPosition {
        SeriesPosition { group, series }
    }

    fn outcomes(result: &SeriesCorrespondence) -> Vec<SeriesMatch> {
        result.current.iter().map(|(_, result)| *result).collect()
    }

    #[test]
    fn explicit_zero_is_identity_but_default_zero_is_not() {
        let previous = plot(vec![series(Some(0), None)]);
        let explicit = match_series(&plot(vec![series(Some(0), None)]), &previous);
        assert_eq!(
            outcomes(&explicit),
            vec![SeriesMatch::ExplicitIndex(position(0, 0))]
        );
        assert!(explicit.unmatched_previous.is_empty());
        let absent = match_series(&plot(vec![series(None, None)]), &previous);
        assert_eq!(outcomes(&absent), vec![SeriesMatch::Unmatched]);
        assert_eq!(absent.unmatched_previous, vec![position(0, 0)]);
    }

    #[test]
    fn series_without_identity_never_match_by_position() {
        let previous = plot(vec![series(None, None), series(None, None)]);
        let current = plot(vec![series(None, None), series(None, None)]);
        let result = match_series(&current, &previous);
        assert_eq!(
            outcomes(&result),
            vec![SeriesMatch::Unmatched, SeriesMatch::Unmatched]
        );
        assert_eq!(
            result.unmatched_previous,
            vec![position(0, 0), position(0, 1)]
        );
    }

    #[test]
    fn unique_indices_follow_reordered_removed_and_new_series() {
        let previous = plot(vec![
            series(Some(11), None),
            series(Some(12), None),
            series(Some(13), None),
        ]);
        let current = plot(vec![
            series(Some(13), None),
            series(Some(11), None),
            series(Some(99), None),
        ]);
        let result = match_series(&current, &previous);
        assert_eq!(
            result.current,
            vec![
                (position(0, 0), SeriesMatch::ExplicitIndex(position(0, 2))),
                (position(0, 1), SeriesMatch::ExplicitIndex(position(0, 0))),
                (position(0, 2), SeriesMatch::Unmatched),
            ]
        );
        assert_eq!(result.unmatched_previous, vec![position(0, 1)]);
    }

    #[test]
    fn unique_formula_fallback_handles_missing_and_changed_indices() {
        let previous = plot(vec![
            series(Some(1), Some("Data!$A$1:$A$4")),
            series(Some(2), Some("Data!$B$1:$B$4")),
        ]);
        let current = plot(vec![
            series(None, Some("Data!$B$1:$B$4")),
            series(Some(77), Some("Data!$A$1:$A$4")),
        ]);
        let result = match_series(&current, &previous);
        assert_eq!(
            outcomes(&result),
            vec![
                SeriesMatch::ValuesFormula(position(0, 1)),
                SeriesMatch::ValuesFormula(position(0, 0)),
            ]
        );
        assert!(result.unmatched_previous.is_empty());
    }

    #[test]
    fn duplicate_indices_can_only_be_disambiguated_by_unique_formulas() {
        let previous = plot(vec![
            series(Some(3), Some("Data!$A$1")),
            series(Some(3), Some("Data!$B$1")),
            series(Some(4), None),
        ]);
        let current = plot(vec![
            series(Some(3), Some("Data!$B$1")),
            series(Some(3), Some("Data!$A$1")),
            series(Some(4), None),
            series(Some(4), None),
        ]);
        let result = match_series(&current, &previous);
        assert_eq!(
            outcomes(&result),
            vec![
                SeriesMatch::ValuesFormula(position(0, 1)),
                SeriesMatch::ValuesFormula(position(0, 0)),
                SeriesMatch::Ambiguous,
                SeriesMatch::Ambiguous,
            ]
        );
        assert_eq!(result.unmatched_previous, vec![position(0, 2)]);
    }

    #[test]
    fn duplicate_previous_indices_without_formulas_are_ambiguous() {
        let previous = plot(vec![series(Some(5), None), series(Some(5), None)]);
        let result = match_series(&plot(vec![series(Some(5), None)]), &previous);
        assert_eq!(outcomes(&result), vec![SeriesMatch::Ambiguous]);
        assert_eq!(result.unmatched_previous.len(), 2);
    }

    #[test]
    fn duplicate_formula_candidates_do_not_become_unique_after_index_matches() {
        let previous = plot(vec![
            series(Some(1), Some("Data!$A$1")),
            series(Some(2), Some("Data!$A$1")),
        ]);
        let current = plot(vec![
            series(Some(1), Some("Data!$A$1")),
            series(None, Some("Data!$A$1")),
        ]);
        let result = match_series(&current, &previous);
        assert_eq!(
            outcomes(&result),
            vec![
                SeriesMatch::ExplicitIndex(position(0, 0)),
                SeriesMatch::Ambiguous,
            ]
        );
        assert_eq!(result.unmatched_previous, vec![position(0, 1)]);
    }

    #[test]
    fn unique_index_wins_over_a_conflicting_formula_and_reserves_its_owner() {
        let previous = plot(vec![
            series(Some(1), Some("Data!$A$1")),
            series(Some(2), Some("Data!$B$1")),
        ]);
        let current = plot(vec![
            series(None, Some("Data!$A$1")),
            series(Some(1), Some("Data!$B$1")),
        ]);
        let result = match_series(&current, &previous);
        assert_eq!(
            outcomes(&result),
            vec![
                SeriesMatch::Ambiguous,
                SeriesMatch::ExplicitIndex(position(0, 0)),
            ]
        );
        assert_eq!(result.unmatched_previous, vec![position(0, 1)]);
    }

    #[test]
    fn formula_identity_preserves_sheet_workbook_absolute_and_literal_distinctions() {
        for different in [
            "Other!$A$1",
            "[Other.xlsx]Data!$A$1",
            "Data!A1",
            "\"Data!$A$1\"",
            "data!$A$1",
        ] {
            let result = match_series(
                &plot(vec![series(None, Some(different))]),
                &plot(vec![series(None, Some("Data!$A$1"))]),
            );
            assert_eq!(
                outcomes(&result),
                vec![SeriesMatch::Unmatched],
                "{different}"
            );
        }
    }

    #[test]
    fn native_address_canonicalization_matches_quoted_sheet_names() {
        let result = match_series(
            &plot(vec![series(None, Some("'Data'!$A$1"))]),
            &plot(vec![series(None, Some("Data!$A$1"))]),
        );
        assert_eq!(
            outcomes(&result),
            vec![SeriesMatch::ValuesFormula(position(0, 0))]
        );
        let mut raw = series(None, Some("Data!$A$1"));
        raw.values_mut()
            .unwrap()
            .number_reference_mut()
            .formula_mut()
            .address_mut()
            .set_sheet_name("Bob's Data");
        let escaped = series(None, Some("'Bob''s Data'!$A$1"));
        assert_eq!(
            outcomes(&match_series(&plot(vec![raw]), &plot(vec![escaped]))),
            vec![SeriesMatch::ValuesFormula(position(0, 0))]
        );
    }

    #[test]
    fn values_formula_precedes_y_values_and_empty_formulas_are_not_identity() {
        let mut both = series(None, Some("Data!$A$1"));
        let mut y = YValues::default();
        y.number_reference_mut()
            .formula_mut()
            .set_address_str("Data!$B$1");
        both.set_y_values(y.clone());
        let mut y_only = series(None, None);
        y_only.set_y_values(y);
        assert_eq!(
            outcomes(&match_series(
                &plot(vec![y_only.clone()]),
                &plot(vec![both.clone()])
            )),
            vec![SeriesMatch::Unmatched]
        );
        assert_eq!(
            outcomes(&match_series(
                &plot(vec![y_only.clone()]),
                &plot(vec![y_only])
            )),
            vec![SeriesMatch::ValuesFormula(position(0, 0))]
        );
        assert_eq!(
            outcomes(&match_series(
                &plot(vec![series(None, Some(""))]),
                &plot(vec![series(None, Some(""))])
            )),
            vec![SeriesMatch::Unmatched]
        );
    }

    #[test]
    fn identity_counts_include_no_parts_series_in_every_group() {
        let mut previous = plot(vec![series(Some(7), Some("Data!$A$1"))]);
        let mut bar = BarChart::default();
        bar.area_chart_series_list_mut()
            .set_area_chart_series(vec![series(Some(7), Some("Data!$A$1"))]);
        previous.add_chart_group(ChartGroup::BarChart(bar));
        let result = match_series(&plot(vec![series(Some(7), Some("Data!$A$1"))]), &previous);
        assert_eq!(outcomes(&result), vec![SeriesMatch::Ambiguous]);
        assert_eq!(
            result.unmatched_previous,
            vec![position(0, 0), position(1, 0)]
        );
    }

    #[test]
    fn global_series_identity_follows_group_reorder_and_family_change() {
        let mut previous = plot(vec![series(Some(1), None)]);
        let mut bar = BarChart::default();
        bar.area_chart_series_list_mut()
            .set_area_chart_series(vec![series(Some(2), None)]);
        previous.add_chart_group(ChartGroup::BarChart(bar));
        let current = plot(vec![series(Some(2), None), series(Some(1), None)]);
        let result = match_series(&current, &previous);
        assert_eq!(
            outcomes(&result),
            vec![
                SeriesMatch::ExplicitIndex(position(1, 0)),
                SeriesMatch::ExplicitIndex(position(0, 0)),
            ]
        );
        assert!(result.unmatched_previous.is_empty());
    }

    #[cfg(feature = "serde")]
    #[test]
    fn correspondence_does_not_mutate_either_serialized_chart() {
        let current = plot(vec![series(None, Some("Data!$A$1"))]);
        let previous = plot(vec![series(Some(1), Some("Data!$A$1"))]);
        let current_before = bincode::serialize(&current).unwrap();
        let previous_before = serde_json::to_vec(&previous).unwrap();
        drop(match_series(&current, &previous));
        assert_eq!(bincode::serialize(&current).unwrap(), current_before);
        assert_eq!(serde_json::to_vec(&previous).unwrap(), previous_before);
    }
}
