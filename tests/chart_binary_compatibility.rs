#![cfg(feature = "serde")]

use serde::{
    Deserialize,
    Serialize,
};
use umya_spreadsheet::drawing::charts::{
    CategoryAxisData,
    Formula,
    GroupingValues,
    MultiLevelStringCache,
    MultiLevelStringPoint,
    MultiLevelStringReference,
    NumberReference,
    StringCache,
    StringLiteral,
    StringReference,
};

// These mirrors are the chart fields compiled into the existing browser codec.
#[derive(Clone, Serialize, Deserialize)]
struct LegacyStringReference {
    formula:      Formula,
    string_cache: StringCache,
}

#[derive(Clone, Serialize, Deserialize)]
struct LegacyCategoryAxisData {
    string_reference: Option<LegacyStringReference>,
    string_literal:   Option<StringLiteral>,
    number_reference: Option<NumberReference>,
}

#[derive(Serialize, Deserialize)]
enum LegacyGroupingValues {
    PercentStacked,
    Stacked,
    Standard,
}

fn formula(address: &str) -> Formula {
    let mut formula = Formula::default();
    formula.set_address_str(address);
    formula
}

fn string_reference() -> StringReference {
    let mut reference = StringReference::default();
    reference.set_formula(formula("'Income & Sales'!$A$1"));
    let mut json = serde_json::to_value(&reference).unwrap();
    json["cached_value"] = serde_json::json!("Revenue & Sales");
    serde_json::from_value(json).unwrap()
}

#[test]
fn string_reference_binary_bytes_match_legacy_and_leave_following_fields_aligned() {
    let reference = string_reference();
    let legacy = LegacyStringReference {
        formula:      reference.formula().clone(),
        string_cache: reference.string_cache().clone(),
    };
    let current_bytes =
        bincode::serialize(&(vec![reference.clone(), reference], 0x1234_u32)).unwrap();
    let legacy_bytes = bincode::serialize(&(vec![legacy.clone(), legacy], 0x1234_u32)).unwrap();
    assert_eq!(current_bytes, legacy_bytes);

    let (decoded, sentinel): (Vec<StringReference>, u32) =
        bincode::deserialize(&legacy_bytes).unwrap();
    assert_eq!(sentinel, 0x1234);
    assert_eq!(decoded.len(), 2);
    assert_eq!(decoded[0].formula().address_str(), "'Income & Sales'!$A$1");
    assert_eq!(decoded[0].cached_value(), None);
    let (old_browser, sentinel): (Vec<LegacyStringReference>, u32) =
        bincode::deserialize(&current_bytes).unwrap();
    assert_eq!(old_browser.len(), 2);
    assert_eq!(sentinel, 0x1234);
}

#[test]
fn category_binary_bytes_match_legacy_for_every_existing_source_kind() {
    let reference = string_reference();
    let mut categories = CategoryAxisData::default();
    categories.set_string_reference(reference.clone());
    let mut literal = CategoryAxisData::default();
    literal.set_string_literal(StringLiteral::default());
    let mut numeric_reference = NumberReference::default();
    numeric_reference.set_formula(formula("Sheet1!$B$2:$B$5"));
    let mut numeric = CategoryAxisData::default();
    numeric.set_number_reference(numeric_reference.clone());

    let old = vec![
        LegacyCategoryAxisData {
            string_reference: Some(LegacyStringReference {
                formula:      reference.formula().clone(),
                string_cache: reference.string_cache().clone(),
            }),
            string_literal:   None,
            number_reference: None,
        },
        LegacyCategoryAxisData {
            string_reference: None,
            string_literal:   Some(StringLiteral::default()),
            number_reference: None,
        },
        LegacyCategoryAxisData {
            string_reference: None,
            string_literal:   None,
            number_reference: Some(numeric_reference),
        },
    ];
    let current_bytes =
        bincode::serialize(&(vec![categories, literal, numeric], 9876_u32)).unwrap();
    let legacy_bytes = bincode::serialize(&(old, 9876_u32)).unwrap();
    assert_eq!(current_bytes, legacy_bytes);

    let (new_reader, sentinel): (Vec<CategoryAxisData>, u32) =
        bincode::deserialize(&legacy_bytes).unwrap();
    assert_eq!(sentinel, 9876);
    assert!(new_reader[0].string_reference().is_some());
    assert!(new_reader[1].string_literal().is_some());
    assert!(new_reader[2].number_reference().is_some());
    let (old_browser, sentinel): (Vec<LegacyCategoryAxisData>, u32) =
        bincode::deserialize(&current_bytes).unwrap();
    assert_eq!(old_browser.len(), 3);
    assert_eq!(sentinel, 9876);
}

#[test]
fn multi_level_categories_remain_in_json_while_binary_uses_legacy_shape() {
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

    let json = serde_json::to_string(&categories).unwrap();
    let restored: CategoryAxisData = serde_json::from_str(&json).unwrap();
    let reference = restored.multi_level_string_reference().unwrap();
    assert_eq!(reference.formula().address_str(), "Sheet1!$A$2:$B$5");
    assert_eq!(reference.cache().unwrap().levels()[0][0].index(), 2);
    assert_eq!(reference.cache().unwrap().levels()[0][0].value(), "2027");
    assert!(categories.multi_level_string_reference().is_some());

    let binary = bincode::serialize(&categories).unwrap();
    let legacy = LegacyCategoryAxisData {
        string_reference: None,
        string_literal:   None,
        number_reference: None,
    };
    assert_eq!(binary, bincode::serialize(&legacy).unwrap());
    let decoded: CategoryAxisData = bincode::deserialize(&binary).unwrap();
    assert!(decoded.multi_level_string_reference().is_none());
}

#[test]
fn grouping_binary_discriminants_remain_compatible_with_the_existing_browser() {
    for (value, legacy) in [
        (
            GroupingValues::PercentStacked,
            LegacyGroupingValues::PercentStacked,
        ),
        (GroupingValues::Stacked, LegacyGroupingValues::Stacked),
        (GroupingValues::Standard, LegacyGroupingValues::Standard),
        (GroupingValues::Clustered, LegacyGroupingValues::Standard),
    ] {
        let binary = bincode::serialize(&value).unwrap();
        assert_eq!(binary, bincode::serialize(&legacy).unwrap());
        let old_browser: LegacyGroupingValues = bincode::deserialize(&binary).unwrap();
        let restored: GroupingValues = bincode::deserialize(&binary).unwrap();
        match old_browser {
            LegacyGroupingValues::PercentStacked => {
                assert!(matches!(restored, GroupingValues::PercentStacked))
            }
            LegacyGroupingValues::Stacked => assert!(matches!(restored, GroupingValues::Stacked)),
            LegacyGroupingValues::Standard => assert!(matches!(restored, GroupingValues::Standard)),
        }
    }
    assert_eq!(
        serde_json::to_string(&GroupingValues::Clustered).unwrap(),
        "\"Clustered\""
    );
    let restored: GroupingValues = serde_json::from_str("\"Clustered\"").unwrap();
    assert!(matches!(restored, GroupingValues::Clustered));
}
