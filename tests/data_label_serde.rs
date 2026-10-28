#![cfg(feature = "serde")]
use umya_spreadsheet::drawing::charts::{
    DataLabel,
    DataLabelPosition,
    DataLabels,
    XmlBoolean,
};

#[test]
fn label_payload_survives_json_and_binary_serialization() {
    let mut label = DataLabel::default();
    label.index_mut().set_val(9);
    label.set_position(Some(DataLabelPosition::OutsideEnd));
    label.set_separator(Some("A&B".to_owned()));
    label.flags_mut().show_value = Some(XmlBoolean::default());
    let mut labels = DataLabels::default();
    labels.labels_mut().push(label);
    let json = serde_json::to_string(&labels).unwrap();
    let from_json: DataLabels = serde_json::from_str(&json).unwrap();
    let bytes = bincode::serialize(&labels).unwrap();
    let from_binary: DataLabels = bincode::deserialize(&bytes).unwrap();
    for value in [&from_json, &from_binary] {
        let label = &value.labels()[0];
        assert_eq!(label.index().val(), 9);
        assert_eq!(label.position(), Some(DataLabelPosition::OutsideEnd));
        assert_eq!(label.separator(), Some("A&B"));
        assert_eq!(label.flags().show_value.as_ref().unwrap().attribute(), None);
    }
}

#[test]
fn historical_data_labels_json_accepts_only_the_appended_default() {
    let mut original = DataLabels::default();
    original.show_value_mut().set_val(true);
    let mut value = serde_json::to_value(&original).unwrap();
    value.as_object_mut().unwrap().remove("parts");
    let restored: DataLabels = serde_json::from_value(value).unwrap();
    assert!(restored.show_value().val());
    assert!(restored.labels().is_empty());
    assert!(restored.extensions().is_none());
}

#[test]
fn appended_default_does_not_change_the_legacy_field_prefix() {
    use umya_spreadsheet::drawing::charts::{
        ShowBubbleSize,
        ShowCategoryName,
        ShowLeaderLines,
        ShowLegendKey,
        ShowPercent,
        ShowSeriesName,
        ShowValue,
        TextProperties,
    };
    #[derive(serde::Serialize)]
    struct Legacy<'a> {
        show_legend_key:    &'a ShowLegendKey,
        show_value:         &'a ShowValue,
        show_category_name: &'a ShowCategoryName,
        show_series_name:   &'a ShowSeriesName,
        show_percent:       &'a ShowPercent,
        show_bubble_size:   &'a ShowBubbleSize,
        show_leader_lines:  Option<&'a ShowLeaderLines>,
        text_properties:    Option<&'a TextProperties>,
    }
    let mut value = DataLabels::default();
    value.show_value_mut().set_val(true);
    let legacy = Legacy {
        show_legend_key:    value.show_legend_key(),
        show_value:         value.show_value(),
        show_category_name: value.show_category_name(),
        show_series_name:   value.show_series_name(),
        show_percent:       value.show_percent(),
        show_bubble_size:   value.show_bubble_size(),
        show_leader_lines:  value.show_leader_lines(),
        text_properties:    value.text_properties(),
    };
    let old = bincode::serialize(&legacy).unwrap();
    let current = bincode::serialize(&value).unwrap();
    assert_eq!(&current[..old.len()], old.as_slice());
    assert_eq!(&current[old.len()..], &[0]);
    let mut json = serde_json::to_value(&value).unwrap();
    assert_eq!(
        json.as_object_mut().unwrap().remove("parts"),
        Some(serde_json::Value::Null)
    );
    assert_eq!(json, serde_json::to_value(&legacy).unwrap());
}

#[test]
fn run_presence_supplement_is_appended_after_original_wire_fields() {
    use umya_spreadsheet::drawing::{
        Run,
        RunProperties,
    };
    #[derive(serde::Serialize)]
    struct LegacyRun<'a> {
        text:           &'a str,
        run_properties: &'a RunProperties,
    }
    let mut run = Run::default();
    run.set_text("text");
    let old = LegacyRun {
        text:           run.text(),
        run_properties: run.run_properties(),
    };
    let old_bytes = bincode::serialize(&old).unwrap();
    let new_bytes = bincode::serialize(&run).unwrap();
    assert_eq!(&new_bytes[..old_bytes.len()], old_bytes.as_slice());
    assert_eq!(&new_bytes[old_bytes.len()..], &[0]);
    let mut json = serde_json::to_value(&run).unwrap();
    assert_eq!(
        json.as_object_mut().unwrap().remove("parts"),
        Some(serde_json::Value::Null)
    );
    assert_eq!(json, serde_json::to_value(&old).unwrap());
    let restored: Run = serde_json::from_value(json).unwrap();
    assert_eq!(restored.text(), "text");
}

#[test]
fn imported_absent_run_properties_add_exactly_one_encoded_byte() {
    use umya_spreadsheet::drawing::Run;
    let original = Run::default();
    let mut value = serde_json::to_value(&original).unwrap();
    value["parts"] = serde_json::json!({"properties_present": false});
    let imported: Run = serde_json::from_value(value).unwrap();
    assert_eq!(
        bincode::serialize(&imported).unwrap().len(),
        bincode::serialize(&original).unwrap().len() + 1
    );
}

#[test]
fn inconsistent_absent_owner_json_remains_diagnosable_without_losing_children() {
    use umya_spreadsheet::drawing::charts::SeriesPartConflictReason;
    let mut label = DataLabel::default();
    label.index_mut().set_val(4);
    let mut labels = DataLabels::default();
    labels.labels_mut().push(label);
    let mut json = serde_json::to_value(&labels).unwrap();
    json["parts"]["owner_present"] = serde_json::json!(false);
    let restored: DataLabels = serde_json::from_value(json).unwrap();
    assert_eq!(
        restored.conflict(),
        Some(SeriesPartConflictReason::ConflictingChoice)
    );
    assert_eq!(restored.labels()[0].index().val(), 4);
}
