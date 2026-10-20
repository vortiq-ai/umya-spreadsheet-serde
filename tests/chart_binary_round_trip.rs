#![cfg(feature = "serde")]

use serde::{
    Serialize,
    de::DeserializeOwned,
};
use umya_spreadsheet::drawing::charts::{
    CategoryAxisData,
    Formula,
    GroupingValues,
    MultiLevelStringCache,
    MultiLevelStringPoint,
    MultiLevelStringReference,
    StringReference,
};

// Charts cross the binary RPC wire as bincode; every field must come back.
fn binary_round_trip<T: Serialize + DeserializeOwned>(value: &T) -> T {
    bincode::deserialize(&bincode::serialize(value).unwrap()).unwrap()
}

fn formula(address: &str) -> Formula {
    let mut formula = Formula::default();
    formula.set_address_str(address);
    formula
}

#[test]
fn multi_level_categories_survive_the_binary_wire() {
    let mut point = MultiLevelStringPoint::default();
    point.set_index(2).set_value("2027");
    let mut cache = MultiLevelStringCache::default();
    cache.set_point_count(4).levels_mut().push(vec![point]);
    let mut reference = MultiLevelStringReference::default();
    reference
        .set_formula(formula("Sheet1!$A$2:$B$5"))
        .set_cache(cache);
    let mut categories = CategoryAxisData::default();
    categories.set_multi_level_string_reference(reference);

    let restored = binary_round_trip(&categories);
    let reference = restored.multi_level_string_reference().unwrap();
    assert_eq!(reference.formula().address_str(), "Sheet1!$A$2:$B$5");
    assert_eq!(reference.cache().unwrap().levels()[0][0].value(), "2027");
}

#[test]
fn cached_series_labels_survive_the_binary_wire() {
    let mut reference = StringReference::default();
    reference.set_formula(formula("'Income & Sales'!$A$1"));
    let mut json = serde_json::to_value(&reference).unwrap();
    json["cached_value"] = serde_json::json!("Revenue & Sales");
    let reference: StringReference = serde_json::from_value(json).unwrap();

    let restored = binary_round_trip(&reference);
    assert_eq!(restored.cached_value(), Some("Revenue & Sales"));
    assert_eq!(restored.formula().address_str(), "'Income & Sales'!$A$1");
}

#[test]
fn clustered_grouping_survives_the_binary_wire() {
    assert!(matches!(
        binary_round_trip(&GroupingValues::Clustered),
        GroupingValues::Clustered
    ));
}
