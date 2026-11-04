#![cfg(feature = "serde")]
//! Display-unit and pivot serialization. Companion roundtrip tests exercise
//! XLSX I/O.
use serde::{
    Serialize,
    de::DeserializeOwned,
};
use umya_spreadsheet::drawing::charts::{
    BuiltInUnit,
    ChartText,
    DataLabel,
    DisplayUnit,
    DisplayUnits,
    DisplayUnitsLabel,
    Formula,
    Index,
    PivotFormat,
    StringReference,
    XmlDouble,
};
fn bin<T: Serialize + DeserializeOwned>(v: &T) -> T {
    bincode::deserialize(&bincode::serialize(v).unwrap()).unwrap()
}
fn text_ref(formula_text: &str) -> ChartText {
    let mut formula = Formula::default();
    formula.set_address_str(formula_text);
    let mut reference = StringReference::default();
    reference.set_formula(formula);
    let mut text = ChartText::default();
    text.set_string_reference(reference);
    text
}
#[test]
fn all_nine_units_are_exact_and_default_presence_is_distinct() {
    let names = [
        "hundreds",
        "thousands",
        "tenThousands",
        "hundredThousands",
        "millions",
        "tenMillions",
        "hundredMillions",
        "billions",
        "trillions",
    ];
    let units = [
        BuiltInUnit::Hundreds,
        BuiltInUnit::Thousands,
        BuiltInUnit::TenThousands,
        BuiltInUnit::HundredThousands,
        BuiltInUnit::Millions,
        BuiltInUnit::TenMillions,
        BuiltInUnit::HundredMillions,
        BuiltInUnit::Billions,
        BuiltInUnit::Trillions,
    ];
    for (name, unit) in names.into_iter().zip(units) {
        assert_eq!(unit.lexical(), name);
    }
    let absent = DisplayUnit::BuiltIn(Default::default());
    let restored = bin(&absent);
    let json = serde_json::to_value(&restored).unwrap();
    assert!(json["BuiltIn"]["attribute"].is_null());
    let DisplayUnit::BuiltIn(scalar) = restored else {
        panic!("choice changed");
    };
    assert_eq!(
        scalar.attribute().copied().unwrap_or_default(),
        BuiltInUnit::Thousands
    ); // XSD default, not serialized.
}
#[test]
fn custom_unit_keeps_original_lexical_and_numeric_value() {
    let mut value = XmlDouble::default();
    value.set_attribute(Some("1.00E+3".into()));
    let DisplayUnit::Custom(restored) = bin(&DisplayUnit::Custom(value)) else {
        panic!("choice changed");
    };
    assert_eq!(restored.attribute(), Some("1.00E+3"));
    assert_eq!(restored.value(), Some(1000.0));
}
#[test]
fn label_and_pivot_label_keep_string_reference_ownership() {
    let mut display_label = DisplayUnitsLabel::default();
    display_label.set_text(text_ref("Sheet1!$A$1"));
    let mut units = DisplayUnits::default();
    units
        .set_unit(DisplayUnit::BuiltIn(Default::default()))
        .set_label(display_label);
    let restored = bin(&units);
    assert_eq!(
        restored
            .label()
            .unwrap()
            .text()
            .unwrap()
            .string_reference()
            .unwrap()
            .formula()
            .address_str(),
        "Sheet1!$A$1"
    );

    let mut point_label = DataLabel::default();
    point_label.set_text(Some(text_ref("Sheet1!$B$1")));
    let mut index = Index::default();
    index.set_val(7);
    let mut format = PivotFormat::default();
    format.set_index(index).set_label(point_label);
    let format = bin(&format);
    assert!(format.index_is_present());
    assert_eq!(format.index().val(), 7);
    assert_eq!(
        format
            .label()
            .unwrap()
            .text()
            .unwrap()
            .string_reference()
            .unwrap()
            .formula()
            .address_str(),
        "Sheet1!$B$1"
    );
}
#[test]
fn absent_required_display_choice_and_pivot_index_stay_invalid() {
    let units = DisplayUnits::default();
    assert!(!units.has_valid_required_choice());
    let format = PivotFormat::default();
    assert!(!format.index_is_present());
}

#[test]
fn owner_json_without_parts_defaults_to_absent() {
    let mut chart_json =
        serde_json::to_value(umya_spreadsheet::drawing::charts::Chart::default()).unwrap();
    chart_json.as_object_mut().unwrap().remove("parts");
    let chart: umya_spreadsheet::drawing::charts::Chart =
        serde_json::from_value(chart_json).unwrap();
    assert!(chart.pivot_formats().is_none());

    let mut axis_json =
        serde_json::to_value(umya_spreadsheet::drawing::charts::ValueAxis::default()).unwrap();
    axis_json.as_object_mut().unwrap().remove("parts");
    let axis: umya_spreadsheet::drawing::charts::ValueAxis =
        serde_json::from_value(axis_json).unwrap();
    assert!(axis.display_units().is_none());
}

#[test]
#[ignore = "bodyPr parts preservation is held at the fleet pin"]
fn chart_json_without_parts_preserves_existing_fields() {
    // A populated chart without parts must retain every existing field.
    let old: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/chart_without_parts.json")).unwrap();
    assert!(old.get("parts").is_none());
    let chart: umya_spreadsheet::drawing::charts::Chart =
        serde_json::from_value(old.clone()).unwrap();
    assert!(chart.title().is_some());
    assert!(chart.plot_area().bar_chart().is_some());
    assert!(chart.pivot_formats().is_none());
    fn assert_existing_subset(old: &serde_json::Value, current: &serde_json::Value) {
        match old {
            serde_json::Value::Object(fields) => {
                let object = current.as_object().unwrap();
                for (key, value) in fields {
                    if key == "shape_auto_fit" {
                        assert!(
                            value.is_null(),
                            "fixture only contains absent legacy autofit"
                        );
                        assert_eq!(object.get("auto_fit"), Some(&serde_json::Value::Null));
                        assert!(!object.contains_key("shape_auto_fit"));
                    } else if key == "rgb_color_model_hex" && value.is_object() {
                        // This fixture has no legacy color transforms. Their
                        // canonical native representation is one ordered list.
                        let old_color = value.as_object().unwrap();
                        let current_color = object[key].as_object().unwrap();
                        let legacy_transforms = [
                            "alpha",
                            "luminance",
                            "luminance_modulation",
                            "luminance_offset",
                            "saturation",
                            "saturation_modulation",
                            "shade",
                            "tint",
                        ];
                        for transform in legacy_transforms {
                            assert_eq!(old_color.get(transform), Some(&serde_json::Value::Null));
                            assert!(!current_color.contains_key(transform));
                        }
                        assert_eq!(
                            current_color.get("transforms"),
                            Some(&serde_json::json!([]))
                        );
                        for (color_key, color_value) in old_color {
                            if !legacy_transforms.contains(&color_key.as_str()) {
                                assert_existing_subset(color_value, &current_color[color_key]);
                            }
                        }
                    } else {
                        assert_existing_subset(value, &object[key]);
                    }
                }
            }
            serde_json::Value::Array(values) => {
                let array = current.as_array().unwrap();
                assert_eq!(values.len(), array.len());
                for (left, right) in values.iter().zip(array) {
                    assert_existing_subset(left, right);
                }
            }
            _ => assert_eq!(old, current),
        }
    }
    assert_existing_subset(&old, &serde_json::to_value(&chart).unwrap());
    let restored: umya_spreadsheet::drawing::charts::Chart = bin(&chart);
    assert_existing_subset(&old, &serde_json::to_value(&restored).unwrap());
}

#[test]
fn value_axis_json_without_parts_defaults_only_missing_owner() {
    let old = r#"{
      "axis_id":{"val":{"value":42}},
      "scaling":{"orientation":{"val":{"value":null,"value_default":"MaxMin"}}},
      "delete":{"val":{"value":false}},
      "axis_position":{"val":{"value":null,"value_default":"Bottom"}},
      "major_gridlines":null,"title":null,
      "numbering_format":{"format_code":{"value":"0.00"},"source_linked":{"value":true}},
      "major_tick_mark":{"val":{"value":null,"value_default":"Cross"}},
      "minor_tick_mark":{"val":{"value":null,"value_default":"Cross"}},
      "tick_label_position":{"val":{"value":null,"value_default":"High"}},
      "crossing_axis":{"val":{"value":7}},
      "crosses":{"val":{"value":null,"value_default":"AutoZero"}},
      "cross_between":{"val":{"value":null,"value_default":"Between"}},
      "shape_properties":null,"text_properties":null
    }"#;
    let axis: umya_spreadsheet::drawing::charts::ValueAxis = serde_json::from_str(old).unwrap();
    assert_eq!(axis.axis_id().val(), 42);
    assert_eq!(axis.tick_crossing_axis().val(), 7);
    assert_eq!(axis.numbering_format().format_code(), "0.00");
    assert!(axis.display_units().is_none());
    let roundtrip: umya_spreadsheet::drawing::charts::ValueAxis = bin(&axis);
    assert_eq!(roundtrip.axis_id().val(), 42);
    assert!(roundtrip.display_units().is_none());
}
