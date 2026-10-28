//! Non-materializing owner traversal fragment. The joint cache owner must call
//! these functions only after all pre-existing and phase6/7 roles.
use super::{Chart, DataLabel, DisplayLabelChild, DisplayUnits, DisplayUnitsLabel, DrawingChartParts, PivotFormat, StringReference, ValueAxis};
use std::collections::HashMap;

pub(crate) struct OwnerCounts {
    pivots: HashMap<u32, usize>,
    axes: HashMap<u32, usize>,
    pivot_container_valid: bool,
}
impl OwnerCounts {
    pub(crate) fn new(chart: &Chart) -> Self {
        let mut pivots = HashMap::new();
        if let Some(owners) = chart.pivot_formats() {
            for owner in owners { if let Some(id) = owner.index().explicit_value() { *pivots.entry(id).or_insert(0) += 1; } }
        }
        let mut axes = HashMap::new();
        for owner in chart.plot_area().value_axis() {
            if let Some(id) = owner.axis_id().explicit_value() { *axes.entry(id).or_insert(0) += 1; }
        }
        Self { pivots, axes, pivot_container_valid: !chart.pivot_container_has_conflict() }
    }
    pub(crate) fn pivot(&self, owner: &PivotFormat) -> Option<u32> {
        owner.index().explicit_value().filter(|id| self.pivot_container_valid && owner.index_is_present() && !owner.has_conflict() && self.pivots.get(id) == Some(&1))
    }
    pub(crate) fn axis(&self, owner: &ValueAxis) -> Option<u32> {
        owner.axis_id().explicit_value().filter(|id| !owner.display_units_has_conflict() && self.axes.get(id) == Some(&1))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Phase8ReferenceIdentity {
    PivotFormatLabel(u32),
    DisplayUnitsLabel(u32),
}

fn pivot_mut(pivot: &mut PivotFormat, identity: Option<Phase8ReferenceIdentity>, visit: &mut dyn FnMut(Option<Phase8ReferenceIdentity>, &mut StringReference)) {
    if let Some(r) = pivot.label_mut().and_then(DataLabel::text_mut).and_then(|t| t.string_reference_mut()) { visit(identity, r); }
    for label in pivot.extra_labels_mut() {
        if let Some(r) = label.text_mut().and_then(|t| t.string_reference_mut()) { visit(None, r); }
    }
}
fn container_mut(container: &mut DrawingChartParts, visit: &mut dyn FnMut(Option<Phase8ReferenceIdentity>, &mut StringReference)) {
    if let Some(pivots) = container.pivot_formats_mut() { for p in pivots { pivot_mut(p, None, visit); } }
    for child in container.duplicate_containers_mut() { container_mut(child, visit); }
}
fn units_mut(units: &mut DisplayUnits, identity: Option<Phase8ReferenceIdentity>, visit: &mut dyn FnMut(Option<Phase8ReferenceIdentity>, &mut StringReference)) {
    if let Some(label) = units.label_mut() { display_label_mut(label, identity, visit); }
    for label in units.extra_labels_mut() { display_label_mut(label, None, visit); }
}
fn display_label_mut(label: &mut DisplayUnitsLabel, identity: Option<Phase8ReferenceIdentity>, visit: &mut dyn FnMut(Option<Phase8ReferenceIdentity>, &mut StringReference)) {
    if let Some(r) = label.text_mut().and_then(|t| t.string_reference_mut()) { visit(identity, r); }
    for item in label.extra_children_mut() { if let DisplayLabelChild::Text(text) = item.child_mut() {
        if let Some(r) = text.string_reference_mut() { visit(None, r); }
    }}
}
fn pivot_ref<'a>(pivot: &'a PivotFormat, identity: Option<Phase8ReferenceIdentity>, visit: &mut dyn FnMut(Option<Phase8ReferenceIdentity>, &'a StringReference)) {
    if let Some(r) = pivot.label().and_then(DataLabel::text).and_then(|t| t.string_reference()) { visit(identity, r); }
    for label in pivot.extra_labels() { if let Some(r) = label.text().and_then(|t| t.string_reference()) { visit(None, r); } }
}
fn container_ref<'a>(container: &'a DrawingChartParts, visit: &mut dyn FnMut(Option<Phase8ReferenceIdentity>, &'a StringReference)) {
    if let Some(pivots) = container.pivot_formats() { for p in pivots { pivot_ref(p, None, visit); } }
    for child in container.duplicate_containers() { container_ref(child, visit); }
}
fn units_ref<'a>(units: &'a DisplayUnits, identity: Option<Phase8ReferenceIdentity>, visit: &mut dyn FnMut(Option<Phase8ReferenceIdentity>, &'a StringReference)) {
    if let Some(label) = units.label() { display_label_ref(label, identity, visit); }
    for label in units.extra_labels() { display_label_ref(label, None, visit); }
}
fn display_label_ref<'a>(label: &'a DisplayUnitsLabel, identity: Option<Phase8ReferenceIdentity>, visit: &mut dyn FnMut(Option<Phase8ReferenceIdentity>, &'a StringReference)) {
    if let Some(r) = label.text().and_then(|t| t.string_reference()) { visit(identity, r); }
    for item in label.extra_children() { if let DisplayLabelChild::Text(text) = item.child() {
        if let Some(r) = text.string_reference() { visit(None, r); }
    }}
}

pub(crate) fn visit_phase8_references_mut(
    chart: &mut Chart,
    visit: &mut dyn FnMut(Option<Phase8ReferenceIdentity>, &mut StringReference),
) {
    let counts = OwnerCounts::new(chart);
    if let Some(pivots) = chart.pivot_formats_mut() {
        for pivot in pivots {
            let identity = counts.pivot(pivot).map(Phase8ReferenceIdentity::PivotFormatLabel);
            pivot_mut(pivot, identity, visit);
        }
    }
    if let Some(containers) = chart.duplicate_pivot_containers_mut() { for c in containers { container_mut(c, visit); } }
    for axis in chart.plot_area_mut().value_axis_mut() {
        let identity = counts.axis(axis).map(Phase8ReferenceIdentity::DisplayUnitsLabel);
        if let Some(units) = axis.display_units_mut() { units_mut(units, identity, visit); }
        if let Some(extra) = axis.extra_display_units_mut() { for units in extra { units_mut(units, None, visit); } }
    }
}

pub(crate) fn visit_phase8_references<'a>(
    chart: &'a Chart,
    visit: &mut dyn FnMut(Option<Phase8ReferenceIdentity>, &'a StringReference),
) {
    let counts = OwnerCounts::new(chart);
    if let Some(pivots) = chart.pivot_formats() {
        for pivot in pivots {
            let identity = counts.pivot(pivot).map(Phase8ReferenceIdentity::PivotFormatLabel);
            pivot_ref(pivot, identity, visit);
        }
    }
    for c in chart.duplicate_pivot_containers() { container_ref(c, visit); }
    for axis in chart.plot_area().value_axis() {
        let identity = counts.axis(axis).map(Phase8ReferenceIdentity::DisplayUnitsLabel);
        if let Some(units) = axis.display_units() { units_ref(units, identity, visit); }
        for units in axis.extra_display_units() { units_ref(units, None, visit); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::structs::drawing::charts::{
        ChartText, DisplayUnit, Formula, Index,
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
        let mut label_index = Index::default(); label_index.set_val(3); data_label.set_index(label_index);
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
        let mut label_index = Index::default(); label_index.set_val(3); label.set_index(label_index);
        label.set_text(Some(reference("Sheet1!$A$1")));
        let mut index = Index::default(); index.set_val(7);
        let mut pivot = PivotFormat::default(); pivot.set_index(index).set_label(label);
        let mut json = serde_json::to_value(&pivot).unwrap();
        json["index_present"] = false.into();
        json["invalid_index"] = true.into();
        json["duplicate_indices"] = serde_json::json!(["8"]);
        let pivot: PivotFormat = serde_json::from_value(json).unwrap();
        let mut chart = Chart::default(); chart.set_pivot_formats(vec![pivot]);
        let mut borrowed = Vec::new();
        visit_phase8_references(&chart, &mut |id, reference| borrowed.push((id, reference.formula().address_str().to_owned())));
        assert_eq!(borrowed, vec![(None, "Sheet1!$A$1".into())]);
        let mut mutable = Vec::new();
        visit_phase8_references_mut(&mut chart, &mut |id, reference| { mutable.push(id); reference.formula_mut().set_address_str("Sheet1!$A$2"); });
        assert_eq!(mutable, vec![None]);
        assert_eq!(chart.pivot_formats().unwrap()[0].label().unwrap().text().unwrap().string_reference().unwrap().formula().address_str(), "Sheet1!$A$2");
    }

    #[cfg(feature = "serde")]
    #[test]
    fn duplicate_container_disqualifies_primary_and_duplicate_in_both_traversals() {
        let mut label = DataLabel::default(); let mut label_index = Index::default(); label_index.set_val(3); label.set_index(label_index); label.set_text(Some(reference("Missing!$A$1")));
        let mut index = Index::default(); index.set_val(7);
        let mut pivot = PivotFormat::default(); pivot.set_index(index).set_label(label);
        let mut chart = Chart::default(); chart.set_pivot_formats(vec![pivot]);
        let mut value = serde_json::to_value(&chart).unwrap();
        let duplicate = value["parts"].clone();
        value["parts"]["duplicate_containers"] = serde_json::json!([duplicate]);
        value["parts"]["invalid"] = false.into();
        let mut chart: Chart = serde_json::from_value(value).unwrap();
        let mut borrowed = Vec::new();
        visit_phase8_references(&chart, &mut |id, reference| borrowed.push((id, reference.formula().address_str().to_owned())));
        let mut mutable = Vec::new();
        visit_phase8_references_mut(&mut chart, &mut |id, reference| mutable.push((id, reference.formula().address_str().to_owned())));
        assert_eq!(borrowed, vec![(None, "Missing!$A$1".into()), (None, "Missing!$A$1".into())]);
        assert_eq!(mutable, borrowed);
    }

    #[test]
    fn absent_and_explicit_zero_index_have_identical_borrowed_mutable_roles() {
        let mut label = DataLabel::default(); let mut label_index = Index::default(); label_index.set_val(3); label.set_index(label_index); label.set_text(Some(reference("Missing!$A$1")));
        let mut pivot = PivotFormat::default(); pivot.set_index(Index::default()).set_label(label);
        let mut chart = Chart::default(); chart.set_pivot_formats(vec![pivot]);
        for expected in [None, Some(Phase8ReferenceIdentity::PivotFormatLabel(0))] {
            if expected.is_some() {
                let mut index = Index::default(); index.set_val(0);
                chart.pivot_formats_mut().unwrap()[0].set_index(index);
            }
            let mut borrowed = Vec::new(); visit_phase8_references(&chart, &mut |id, _| borrowed.push(id));
            let mut mutable = Vec::new(); visit_phase8_references_mut(&mut chart, &mut |id, _| mutable.push(id));
            assert_eq!(borrowed, vec![expected]); assert_eq!(mutable, borrowed);
        }
    }
}
