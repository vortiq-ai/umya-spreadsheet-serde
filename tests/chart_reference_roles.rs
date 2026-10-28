#![cfg(feature = "serde")]
//! Public compatibility checks for the native mutable formula projection.
use umya_spreadsheet::drawing::charts::{
    Chart,
    ChartText,
    Formula,
    StringReference,
    Title,
};

#[test]
fn title_formula_projection_is_live_without_materializing_absent_owners() {
    let mut formula = Formula::default();
    formula.set_address_str("Sheet1!$A$1");
    let mut reference = StringReference::default();
    reference.set_formula(formula);
    let mut text = ChartText::default();
    text.set_string_reference(reference);
    let mut title = Title::default();
    title.set_chart_text(text);
    let mut chart = Chart::default();
    chart.set_title(title);
    let before = serde_json::to_value(&chart).unwrap();
    {
        let references = chart.formula_mut();
        assert_eq!(references.len(), 1);
        assert_eq!(references[0].address_str(), "Sheet1!$A$1");
    }
    assert_eq!(serde_json::to_value(&chart).unwrap(), before);
    chart
        .formula_mut()
        .remove(0)
        .set_string_value("exact decided formula text");
    assert_eq!(
        chart
            .title()
            .unwrap()
            .chart_text()
            .unwrap()
            .string_reference()
            .unwrap()
            .formula()
            .address_str(),
        "exact decided formula text",
    );
    assert!(chart.pivot_formats().is_none());
}
