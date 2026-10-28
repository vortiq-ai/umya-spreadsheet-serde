//! Non-materializing owner traversal fragment. The joint cache owner must call
//! these functions only after all pre-existing and phase6/7 roles.
use std::collections::HashMap;

use super::{
    Chart,
    PivotFormat,
    StringReference,
    ValueAxis,
    chart_source_caches::ReferenceIdentity,
};

pub(crate) struct OwnerCounts {
    pivots:                HashMap<u32, usize>,
    axes:                  HashMap<u32, usize>,
    pivot_container_valid: bool,
}
impl OwnerCounts {
    pub(crate) fn for_plot(plot: &super::PlotArea) -> Self {
        let mut axes = HashMap::new();
        for owner in plot.value_axis() {
            if let Some(id) = owner.axis_id().explicit_value() {
                *axes.entry(id).or_insert(0) += 1;
            }
        }
        Self {
            pivots: HashMap::new(),
            axes,
            pivot_container_valid: false,
        }
    }

    pub(crate) fn new(chart: &Chart) -> Self {
        let mut pivots = HashMap::new();
        if let Some(owners) = chart.pivot_formats() {
            for owner in owners {
                if let Some(id) = owner.index().explicit_value() {
                    *pivots.entry(id).or_insert(0) += 1;
                }
            }
        }
        let mut axes = HashMap::new();
        for owner in chart.plot_area().value_axis() {
            if let Some(id) = owner.axis_id().explicit_value() {
                *axes.entry(id).or_insert(0) += 1;
            }
        }
        Self {
            pivots,
            axes,
            pivot_container_valid: !chart.pivot_container_has_conflict(),
        }
    }

    pub(crate) fn pivot(&self, owner: &PivotFormat) -> Option<u32> {
        owner.index().explicit_value().filter(|id| {
            self.pivot_container_valid
                && owner.index_is_present()
                && !owner.has_conflict()
                && self.pivots.get(id) == Some(&1)
        })
    }

    pub(crate) fn axis(&self, owner: &ValueAxis) -> Option<u32> {
        owner
            .axis_id()
            .explicit_value()
            .filter(|id| !owner.display_units_has_conflict() && self.axes.get(id) == Some(&1))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Phase8ReferenceIdentity {
    PivotFormatLabel(u32),
    DisplayUnitsLabel(u32),
}

// Compatibility projection for the already-reviewed phase8 direct-role tests.
// The ownership definition now lives only in chart_reference_roles.
fn direct_role(role: &str) -> bool {
    // Reporting roles outside these four are not Phase8 direct cache owners.
    role == "pivot_label"
        || role == "pivot_label_extra"
        || role == "display_units_label"
        || role == "display_units_text_extra"
}

fn identity(value: Option<&ReferenceIdentity>) -> Option<Phase8ReferenceIdentity> {
    match value {
        Some(ReferenceIdentity::PivotFormatLabel(pivot_index)) => {
            Some(Phase8ReferenceIdentity::PivotFormatLabel(*pivot_index))
        }
        Some(ReferenceIdentity::DisplayUnitsLabel(axis_id)) => {
            Some(Phase8ReferenceIdentity::DisplayUnitsLabel(*axis_id))
        }
        Some(
            ReferenceIdentity::ChartTitle
            | ReferenceIdentity::Category(..)
            | ReferenceIdentity::SeriesText(..)
            | ReferenceIdentity::AxisTitle(..)
            | ReferenceIdentity::TrendlineLabel(..)
            | ReferenceIdentity::Extension(..),
        )
        | None => None,
    }
}

pub(crate) fn visit_phase8_references_mut(
    chart: &mut Chart,
    visit: &mut dyn FnMut(Option<Phase8ReferenceIdentity>, &mut StringReference),
) {
    super::chart_reference_roles::visit_chart_mut(chart, &mut |metadata, target| {
        if !direct_role(metadata.role) {
            return;
        }
        match target {
            super::chart_reference_roles::ReferenceTargetMut::String(reference) => {
                visit(identity(metadata.cache_identity.as_ref()), reference);
            }
            super::chart_reference_roles::ReferenceTargetMut::Number(..)
            | super::chart_reference_roles::ReferenceTargetMut::MultiLevel(..)
            | super::chart_reference_roles::ReferenceTargetMut::Formula(..) => (),
        }
    });
}

pub(crate) fn visit_phase8_references<'a>(
    chart: &'a Chart,
    visit: &mut dyn FnMut(Option<Phase8ReferenceIdentity>, &'a StringReference),
) {
    super::chart_reference_roles::visit_chart_ref(chart, &mut |metadata, target| {
        if !direct_role(metadata.role) {
            return;
        }
        match target {
            super::chart_reference_roles::ReferenceTargetRef::String(reference) => {
                visit(identity(metadata.cache_identity.as_ref()), reference);
            }
            super::chart_reference_roles::ReferenceTargetRef::Number(..)
            | super::chart_reference_roles::ReferenceTargetRef::MultiLevel(..)
            | super::chart_reference_roles::ReferenceTargetRef::Formula(..) => (),
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::structs::drawing::charts::{
        ChartText,
        DataLabel,
        DisplayUnit,
        DisplayUnits,
        DisplayUnitsLabel,
        Formula,
        Index,
    };

    fn reference(address: &str) -> ChartText {
        let mut formula = Formula::default();
        formula.set_address_str(address);
        let mut source = StringReference::default();
        source.set_formula(formula);
        let mut text = ChartText::default();
        text.set_string_reference(source);
        text
    }

    #[test]
    fn phase8_roles_are_exactly_once_pivot_then_axis_and_nonmaterializing() {
        let mut chart = Chart::default();
        let mut data_label = DataLabel::default();
        let mut label_index = Index::default();
        label_index.set_val(3);
        data_label.set_index(label_index);
        data_label.set_text(Some(reference("Sheet1!$A$1")));
        let mut index = Index::default();
        index.set_val(8);
        let mut pivot = PivotFormat::default();
        pivot.set_index(index).set_label(data_label);
        chart.set_pivot_formats(vec![pivot]);

        let mut units_label = DisplayUnitsLabel::default();
        units_label.set_text(reference("Sheet1!$B$1"));
        let mut units = DisplayUnits::default();
        units
            .set_unit(DisplayUnit::BuiltIn(Default::default()))
            .set_label(units_label);
        let mut axis = ValueAxis::default();
        axis.set_display_units(units);
        chart.plot_area_mut().add_value_axis(axis);

        let mut borrowed = Vec::new();
        visit_phase8_references(&chart, &mut |identity, reference| {
            borrowed.push((identity, reference.formula().address_str().to_owned()));
        });
        assert_eq!(
            borrowed,
            vec![
                (
                    Some(Phase8ReferenceIdentity::PivotFormatLabel(8)),
                    "Sheet1!$A$1".into()
                ),
                (None, "Sheet1!$B$1".into()),
            ]
        );
        let mut mutable = Vec::new();
        visit_phase8_references_mut(&mut chart, &mut |identity, reference| {
            mutable.push(identity);
            if reference.formula().address_str() == "Sheet1!$A$1" {
                reference.formula_mut().set_address_str("Sheet1!$A$2");
            }
        });
        assert_eq!(
            mutable,
            borrowed
                .iter()
                .map(|(identity, _)| *identity)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            chart.pivot_formats().unwrap()[0]
                .label()
                .unwrap()
                .text()
                .unwrap()
                .string_reference()
                .unwrap()
                .formula()
                .address_str(),
            "Sheet1!$A$2"
        );

        let mut empty = Chart::default();
        visit_phase8_references_mut(&mut empty, &mut |_, _| {
            panic!("empty access must not materialize roles")
        });
        assert!(empty.pivot_formats().is_none());
        assert!(empty.plot_area().value_axis().is_empty());
    }

    #[cfg(feature = "serde")]
    #[test]
    fn invalid_index_remains_traversable_but_never_supplies_cache_identity() {
        let mut label = DataLabel::default();
        let mut label_index = Index::default();
        label_index.set_val(3);
        label.set_index(label_index);
        label.set_text(Some(reference("Sheet1!$A$1")));
        let mut index = Index::default();
        index.set_val(7);
        let mut pivot = PivotFormat::default();
        pivot.set_index(index).set_label(label);
        let mut json = serde_json::to_value(&pivot).unwrap();
        json["index_present"] = false.into();
        json["invalid_index"] = true.into();
        json["duplicate_indices"] = serde_json::json!(["8"]);
        let pivot: PivotFormat = serde_json::from_value(json).unwrap();
        let mut chart = Chart::default();
        chart.set_pivot_formats(vec![pivot]);
        let mut borrowed = Vec::new();
        visit_phase8_references(&chart, &mut |id, reference| {
            borrowed.push((id, reference.formula().address_str().to_owned()))
        });
        assert_eq!(borrowed, vec![(None, "Sheet1!$A$1".into())]);
        let mut mutable = Vec::new();
        visit_phase8_references_mut(&mut chart, &mut |id, reference| {
            mutable.push(id);
            reference.formula_mut().set_address_str("Sheet1!$A$2");
        });
        assert_eq!(mutable, vec![None]);
        assert_eq!(
            chart.pivot_formats().unwrap()[0]
                .label()
                .unwrap()
                .text()
                .unwrap()
                .string_reference()
                .unwrap()
                .formula()
                .address_str(),
            "Sheet1!$A$2"
        );
    }

    #[cfg(feature = "serde")]
    #[test]
    fn duplicate_container_disqualifies_primary_and_duplicate_in_both_traversals() {
        let mut label = DataLabel::default();
        let mut label_index = Index::default();
        label_index.set_val(3);
        label.set_index(label_index);
        label.set_text(Some(reference("Missing!$A$1")));
        let mut index = Index::default();
        index.set_val(7);
        let mut pivot = PivotFormat::default();
        pivot.set_index(index).set_label(label);
        let mut chart = Chart::default();
        chart.set_pivot_formats(vec![pivot]);
        let mut value = serde_json::to_value(&chart).unwrap();
        let duplicate = value["parts"].clone();
        value["parts"]["duplicate_containers"] = serde_json::json!([duplicate]);
        value["parts"]["invalid"] = false.into();
        let mut chart: Chart = serde_json::from_value(value).unwrap();
        let mut borrowed = Vec::new();
        visit_phase8_references(&chart, &mut |id, reference| {
            borrowed.push((id, reference.formula().address_str().to_owned()))
        });
        let mut mutable = Vec::new();
        visit_phase8_references_mut(&mut chart, &mut |id, reference| {
            mutable.push((id, reference.formula().address_str().to_owned()))
        });
        assert_eq!(
            borrowed,
            vec![(None, "Missing!$A$1".into()), (None, "Missing!$A$1".into())]
        );
        assert_eq!(mutable, borrowed);
    }

    #[test]
    fn absent_and_explicit_zero_index_have_identical_borrowed_mutable_roles() {
        let mut label = DataLabel::default();
        let mut label_index = Index::default();
        label_index.set_val(3);
        label.set_index(label_index);
        label.set_text(Some(reference("Missing!$A$1")));
        let mut pivot = PivotFormat::default();
        pivot.set_index(Index::default()).set_label(label);
        let mut chart = Chart::default();
        chart.set_pivot_formats(vec![pivot]);
        for expected in [None, Some(Phase8ReferenceIdentity::PivotFormatLabel(0))] {
            if expected.is_some() {
                let mut index = Index::default();
                index.set_val(0);
                chart.pivot_formats_mut().unwrap()[0].set_index(index);
            }
            let mut borrowed = Vec::new();
            visit_phase8_references(&chart, &mut |id, _| borrowed.push(id));
            let mut mutable = Vec::new();
            visit_phase8_references_mut(&mut chart, &mut |id, _| mutable.push(id));
            assert_eq!(borrowed, vec![expected]);
            assert_eq!(mutable, borrowed);
        }
    }
}
