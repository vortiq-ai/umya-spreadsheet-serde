#![cfg(feature = "serde")]

use umya_spreadsheet::{
    AutoFilter,
    CellFormula,
    PageSetup,
    Table,
};

#[test]
fn legacy_formula_json_defaults_calculate_cell_without_changing_existing_fields() {
    let mut formula = CellFormula::default();
    formula.set_text("SUM(A1:A3)").set_bx(true);
    let original = serde_json::to_value(&formula).unwrap();
    let mut legacy = original.clone();
    assert!(
        legacy
            .as_object_mut()
            .unwrap()
            .remove("calculate_cell")
            .is_some()
    );
    let restored: CellFormula = serde_json::from_value(legacy).unwrap();
    assert!(!restored.calculate_cell());
    assert_eq!(serde_json::to_value(restored).unwrap(), original);
}

#[test]
fn calculate_cell_presence_survives_json_and_binary_roundtrips() {
    for value in [None, Some(false), Some(true)] {
        let mut formula = CellFormula::default();
        formula.set_text("NOW()");
        if let Some(value) = value {
            formula.set_calculate_cell(value);
        }
        let original = serde_json::to_value(&formula).unwrap();
        assert_eq!(
            original["calculate_cell"]["value"],
            serde_json::json!(value)
        );
        let json: CellFormula = serde_json::from_value(original.clone()).unwrap();
        let binary: CellFormula =
            bincode::deserialize(&bincode::serialize(&formula).unwrap()).unwrap();
        assert_eq!(serde_json::to_value(json).unwrap(), original);
        assert_eq!(serde_json::to_value(binary).unwrap(), original);
    }
}

#[test]
fn legacy_page_setup_json_defaults_fit_to_page_without_changing_existing_fields() {
    let mut setup = PageSetup::default();
    setup.set_fit_to_width(2).set_fit_to_height(3);
    let original = serde_json::to_value(&setup).unwrap();
    let mut legacy = original.clone();
    assert!(
        legacy
            .as_object_mut()
            .unwrap()
            .remove("fit_to_page")
            .is_some()
    );
    let restored: PageSetup = serde_json::from_value(legacy).unwrap();
    assert!(!restored.fit_to_page());
    assert_eq!(serde_json::to_value(restored).unwrap(), original);
}

#[test]
fn legacy_auto_filter_json_defaults_new_column_collection() {
    let mut filter = AutoFilter::default();
    filter.set_range("A1:C4");
    let original = serde_json::to_value(&filter).unwrap();
    let mut legacy = original.clone();
    assert!(
        legacy
            .as_object_mut()
            .unwrap()
            .remove("filter_columns")
            .is_some()
    );
    let restored: AutoFilter = serde_json::from_value(legacy).unwrap();
    assert!(restored.filter_columns().is_empty());
    assert_eq!(serde_json::to_value(restored).unwrap(), original);
}

#[test]
fn legacy_table_json_defaults_new_auto_filter_without_losing_table_fields() {
    let mut original = serde_json::to_value(Table::new("Example", ("A1", "C4"))).unwrap();
    let mut legacy = original.clone();
    assert!(
        legacy
            .as_object_mut()
            .unwrap()
            .remove("auto_filter")
            .is_some()
    );
    let restored: Table = serde_json::from_value(legacy).unwrap();
    assert!(restored.auto_filter().range().range().is_empty());
    let mut restored = serde_json::to_value(restored).unwrap();
    original.as_object_mut().unwrap().remove("auto_filter");
    restored.as_object_mut().unwrap().remove("auto_filter");
    assert_eq!(restored, original);
}
