#![cfg(feature = "serde")]

use std::{
    fs,
    io::Cursor,
    path::Path,
};

use serde_json::{
    Value,
    json,
};
use umya_spreadsheet::{
    drawing::{
        GradientStop,
        RgbColorModelHex,
        SchemeColor,
    },
    reader,
};

fn stops(value: &Value) -> Vec<Value> {
    match value {
        Value::Object(fields)
            if fields.contains_key("position") && fields.contains_key("scheme_color") =>
        {
            vec![value.clone()]
        }
        Value::Object(fields) => fields.values().flat_map(stops).collect(),
        Value::Array(values) => values.iter().flat_map(stops).collect(),
        _ => Vec::new(),
    }
}

#[test]
fn historical_integer_gradient_positions_match_the_authored_xml() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/persisted_charts");
    let mut checked = 0;
    for entry in fs::read_dir(root).unwrap() {
        let directory = entry.unwrap().path();
        let drawing = directory.join("gradient_stops.drawing.json");
        if !drawing.is_file() {
            continue;
        }
        let legacy: Value = serde_json::from_slice(&fs::read(drawing).unwrap()).unwrap();
        let book = reader::xlsx::read_reader(
            Cursor::new(fs::read(directory.join("../inputs/gradient_stops.xlsx")).unwrap()),
            true,
        )
        .unwrap();
        let oracle = serde_json::to_value(book.sheet(0).unwrap().worksheet_drawing()).unwrap();
        let expected = stops(&oracle);
        let legacy = stops(&legacy);
        assert_eq!(legacy.len(), 3);
        assert_eq!(expected.len(), 3);
        for ((legacy, expected), position) in legacy
            .into_iter()
            .zip(expected)
            .zip(["0", "37500", "100000"])
        {
            let restored: GradientStop = serde_json::from_value(legacy).unwrap();
            assert_eq!(restored.position(), position);
            assert_eq!(serde_json::to_value(restored).unwrap(), expected);
            checked += 1;
        }
    }
    assert_eq!(checked, 15);
}

#[test]
fn gradient_position_map_values_keep_percentage_validation() {
    for position in [
        json!(0),
        json!(50000),
        json!(100000),
        json!("0"),
        json!("50000"),
        json!("100000"),
        json!("37.5%"),
    ] {
        let mut value = serde_json::to_value(GradientStop::default()).unwrap();
        value["position"] = position.clone();
        let stop: GradientStop = serde_json::from_value(value).unwrap();
        let expected = position
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| position.to_string());
        assert_eq!(stop.position(), expected);
    }
    for position in [
        json!(-1),
        json!(100001),
        json!(2147483648_u64),
        json!(1.5),
        json!(true),
        json!(null),
        json!([]),
        json!({}),
        json!("100001"),
        json!("101%"),
        json!("invalid"),
    ] {
        let mut value = serde_json::to_value(GradientStop::default()).unwrap();
        value["position"] = position;
        assert!(serde_json::from_value::<GradientStop>(value).is_err());
    }
}

#[test]
fn gradient_compatibility_rejects_unknown_and_duplicate_keys() {
    let mut value = serde_json::to_value(GradientStop::default()).unwrap();
    value["unknown_field"] = json!(null);
    assert!(serde_json::from_value::<GradientStop>(value).is_err());
    let json = serde_json::to_string(&GradientStop::default()).unwrap();
    for duplicate in [
        "\"position\":0",
        "\"hsl_color\":null",
        "\"scheme_color\":null",
    ] {
        let json = format!("{},{duplicate}}}", json.trim_end_matches('}'));
        assert!(serde_json::from_str::<GradientStop>(&json).is_err());
    }
}

#[test]
fn gradient_binary_encoding_keeps_the_current_derived_field_order() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/persisted_charts/inputs/gradient_stops.xlsx");
    let book = reader::xlsx::read_reader(Cursor::new(fs::read(path).unwrap()), true).unwrap();
    let value = serde_json::to_value(book.sheet(0).unwrap().worksheet_drawing()).unwrap();
    let stops = stops(&value);
    assert_eq!(stops.len(), 3);
    for value in stops {
        let stop: GradientStop = serde_json::from_value(value).unwrap();
        let binary = bincode::serialize(&stop).unwrap();
        let derived = bincode::serialize(&(
            stop.hsl_color(),
            stop.rgb_color_model_percentage(),
            stop.position(),
            stop.scheme_color(),
            stop.rgb_color_model_hex(),
            stop.preset_color(),
            stop.system_color(),
        ))
        .unwrap();
        assert_eq!(binary, derived);
        let decoded: GradientStop = bincode::deserialize(&binary).unwrap();
        assert_eq!(binary, bincode::serialize(&decoded).unwrap());
    }
    let legacy =
        bincode::serialize(&(50000_i32, None::<SchemeColor>, None::<RgbColorModelHex>)).unwrap();
    assert!(bincode::deserialize::<GradientStop>(&legacy).is_err());
    assert!(
        serde_json::from_value::<GradientStop>(json!([null, null, 50000, null, null, null, null]))
            .is_err()
    );
    assert!(serde_json::from_value::<GradientStop>(json!(["50000", null, null])).is_err());
}
