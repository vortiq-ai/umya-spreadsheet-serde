#![cfg(feature = "serde")]

use std::io::{
    Cursor,
    Read,
};

use serde_json::{
    Value,
    json,
};
use umya_spreadsheet::{
    EnumValue,
    drawing::{
        BlackWhiteModeValues,
        charts::OpaqueElement,
        spreadsheet::{
            Shape,
            ShapeProperties,
            TwoCellAnchor,
        },
    },
    reader,
    writer,
};

fn rgb(current: bool, color: &str) -> Value {
    if current {
        json!({"val": {"value": color}, "transforms": []})
    } else {
        json!({
            "val": {"value": color},
            "luminance": null,
            "luminance_modulation": null,
            "luminance_offset": null,
            "saturation": null,
            "saturation_modulation": null,
            "shade": null,
            "alpha": null,
            "tint": null,
        })
    }
}

fn shape_json(current: bool) -> Value {
    let stops = [(0, "112233"), (100000, "ABCDEF")].map(|(position, color)| {
        if current {
            json!({
                "hsl_color": null,
                "rgb_color_model_percentage": null,
                "position": position.to_string(),
                "scheme_color": null,
                "rgb_color_model_hex": rgb(true, color),
                "preset_color": null,
                "system_color": null,
            })
        } else {
            json!({
                "position": position,
                "scheme_color": null,
                "rgb_color_model_hex": rgb(false, color),
            })
        }
    });
    let mut value = json!({
        "transform2d": {
            "offset": {"x": {"value": 11}, "y": {"value": 22}},
            "extents": {"cx": {"value": 300}, "cy": {"value": 400}},
            "child_offset": null,
            "child_extents": null,
            "rot": {"value": "60000"},
            "flip_v": {"value": "1"},
            "flip_h": {"value": "0"},
        },
        "preset_geometry": {
            "geometry": "rect",
            "adjust_value_list": {"shape_guide_collection": []},
        },
        "blip_fill": null,
        "solid_fill": null,
        "gradient_fill": {
            "flip": {"value": null, "value_default": "None"},
            "rotate_with_shape": {"value": true},
            "gradient_stop_list": {"gradient_stop": stops},
            "linear_gradient_fill": {"angle": {"value": 5400000}, "scaled": {"value": true}},
            "tile_rectangle": null,
        },
        "outline": null,
        "effect_list": null,
        "no_fill": null,
        "extension_list": null,
        "black_white_mode": {"value": "Gray", "value_default": "Auto"},
    });
    if current {
        value["gradient_fill"]["path_gradient_fill"] = Value::Null;
        value["preserved_properties"] = json!([{
            "name": {
                "namespace_uri": "http://schemas.openxmlformats.org/drawingml/2006/main",
                "local_name": "sp3d",
                "preferred_prefix": "a",
            },
            "namespaces": [["a", "http://schemas.openxmlformats.org/drawingml/2006/main"]],
            "attributes": [[{
                "namespace_uri": "",
                "local_name": "extrusionH",
                "preferred_prefix": null,
            }, "12700"]],
            "children": [],
        }]);
    }
    value
}

fn drawing_roundtrip(properties: ShapeProperties) -> (ShapeProperties, String) {
    let mut shape = Shape::default();
    shape
        .non_visual_shape_properties_mut()
        .non_visual_drawing_properties_mut()
        .set_id(1)
        .set_name("Gradient shape");
    shape.set_shape_properties(properties);
    let mut anchor = TwoCellAnchor::default();
    anchor.set_shape(shape);
    let mut book = umya_spreadsheet::new_file();
    book.sheet_mut(0)
        .unwrap()
        .worksheet_drawing_mut()
        .add_two_cell_anchor_collection(anchor);
    let mut bytes = Vec::new();
    writer::xlsx::write_writer(&book, &mut bytes).unwrap();
    let mut archive = zip::ZipArchive::new(Cursor::new(&bytes)).unwrap();
    let mut xml = String::new();
    archive
        .by_name("xl/drawings/drawing1.xml")
        .unwrap()
        .read_to_string(&mut xml)
        .unwrap();
    let reread = reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap();
    let properties = reread
        .sheet(0)
        .unwrap()
        .worksheet_drawing()
        .shape_collection()[0]
        .shape_properties()
        .clone();
    (properties, xml)
}

#[test]
fn historical_shape_maps_keep_the_old_writers_omitted_gradient() {
    let properties: ShapeProperties = serde_json::from_value(shape_json(false)).unwrap();
    assert!(properties.gradient_fill().is_none());
    let (properties, xml) = drawing_roundtrip(properties);
    assert!(properties.gradient_fill().is_none());
    assert!(!xml.contains("<a:gradFill"));
    assert!(xml.contains("bwMode=\"gray\""));
    assert!(xml.contains("x=\"11\""));
    assert!(xml.contains("y=\"22\""));
    assert!(xml.contains("prst=\"rect\""));
}

fn ordered_gradient_json(current: bool, position: i32, gradient_first: bool) -> String {
    let mut value = shape_json(current);
    value["gradient_fill"]["gradient_stop_list"]["gradient_stop"][0]["position"] = json!(position);
    let gradient = value
        .as_object_mut()
        .unwrap()
        .remove("gradient_fill")
        .unwrap();
    let rest = value.to_string();
    if gradient_first {
        format!(
            "{{\"gradient_fill\":{gradient},{}",
            rest.strip_prefix('{').unwrap()
        )
    } else {
        format!(
            "{},\"gradient_fill\":{gradient}}}",
            rest.strip_suffix('}').unwrap()
        )
    }
}

#[test]
fn historical_ignored_gradients_do_not_gain_current_position_validation() {
    // Historical positions were i32 values, but the old spreadsheet-shape
    // writer emitted no gradient at all. Its output remains authoritative.
    for position in [-1, 100001] {
        for gradient_first in [true, false] {
            let json = ordered_gradient_json(false, position, gradient_first);
            let properties: ShapeProperties = serde_json::from_str(&json).unwrap();
            assert!(properties.gradient_fill().is_none());
            let (properties, xml) = drawing_roundtrip(properties);
            assert!(properties.gradient_fill().is_none());
            assert!(!xml.contains("<a:gradFill"));
        }
    }
}

#[test]
fn current_shape_gradients_still_validate_positions_in_either_key_order() {
    for position in [-1, 100001] {
        for gradient_first in [true, false] {
            // Sorting the gradient before/after the other map keys also
            // places the current marker after/before it respectively.
            let json = ordered_gradient_json(true, position, gradient_first);
            assert!(serde_json::from_str::<ShapeProperties>(&json).is_err());
        }
    }
}

#[test]
fn discarded_historical_gradients_keep_their_outer_map_or_null_type_boundary() {
    for gradient in [json!(true), json!(17), json!("not a gradient"), json!([])] {
        let mut value = shape_json(false);
        value["gradient_fill"] = gradient;
        assert!(serde_json::from_value::<ShapeProperties>(value).is_err());
    }
    for current in [false, true] {
        let mut value = shape_json(current);
        value["gradient_fill"] = Value::Null;
        assert!(
            serde_json::from_value::<ShapeProperties>(value)
                .unwrap()
                .gradient_fill()
                .is_none()
        );
    }
}

#[test]
fn deferred_current_gradients_replay_nested_color_transform_enums() {
    let mut value = shape_json(true);
    value["gradient_fill"]["gradient_stop_list"]["gradient_stop"][0]["rgb_color_model_hex"]
        ["transforms"] = json!([
        "Inverse",
        {"Tint": "10000"},
        {"Alpha": {"val": "90000"}},
    ]);
    let properties: ShapeProperties = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(serde_json::to_value(&properties).unwrap(), value);
    let (_, xml) = drawing_roundtrip(properties);
    assert!(xml.contains("<a:inv"));
    assert!(xml.contains("<a:tint val=\"10000\""));
    assert!(xml.contains("<a:alpha val=\"90000\""));
}

#[test]
fn current_shape_maps_preserve_native_gradients_with_empty_or_populated_property_lists() {
    for preserved in [json!([]), shape_json(true)["preserved_properties"].clone()] {
        let mut expected = shape_json(true);
        expected["preserved_properties"] = preserved;
        let properties: ShapeProperties = serde_json::from_value(expected.clone()).unwrap();
        assert_eq!(serde_json::to_value(&properties).unwrap(), expected);
        let (properties, xml) = drawing_roundtrip(properties);
        assert!(properties.gradient_fill().is_some());
        assert!(xml.contains("<a:gradFill"));
        assert!(xml.contains("pos=\"0\""));
        assert!(xml.contains("pos=\"100000\""));
        assert!(xml.contains("val=\"112233\""));
        assert!(xml.contains("val=\"ABCDEF\""));
        assert_eq!(
            xml.contains("extrusionH=\"12700\""),
            !expected["preserved_properties"]
                .as_array()
                .unwrap()
                .is_empty(),
        );
    }
}

#[test]
fn shape_compatibility_maps_reject_unknown_duplicate_and_invalid_marker_fields() {
    for current in [false, true] {
        let mut value = shape_json(current);
        value["unknown_shape_property"] = Value::Null;
        assert!(serde_json::from_value::<ShapeProperties>(value).is_err());
        for field in ["gradient_fill", "solid_fill", "transform2d"] {
            let json = shape_json(current).to_string();
            let duplicate = format!("{},\"{field}\":null}}", json.strip_suffix('}').unwrap());
            assert!(serde_json::from_str::<ShapeProperties>(&duplicate).is_err());
        }
        for required in ["preset_geometry", "black_white_mode"] {
            let mut value = shape_json(current);
            value.as_object_mut().unwrap().remove(required);
            assert!(serde_json::from_value::<ShapeProperties>(value).is_err());
        }
    }
    for marker in [Value::Null, json!(true), json!({})] {
        let mut value = shape_json(true);
        value["preserved_properties"] = marker;
        assert!(serde_json::from_value::<ShapeProperties>(value).is_err());
    }
    let json = shape_json(true).to_string();
    let duplicate = format!(
        "{},\"preserved_properties\":[]}}",
        json.strip_suffix('}').unwrap()
    );
    assert!(serde_json::from_str::<ShapeProperties>(&duplicate).is_err());
}

#[test]
fn current_shape_binary_encoding_keeps_all_eleven_derived_fields() {
    let value = shape_json(true);
    let properties: ShapeProperties = serde_json::from_value(value.clone()).unwrap();
    let black_white_mode: EnumValue<BlackWhiteModeValues> =
        serde_json::from_value(value["black_white_mode"].clone()).unwrap();
    let preserved: Vec<OpaqueElement> =
        serde_json::from_value(value["preserved_properties"].clone()).unwrap();
    let expected = bincode::serialize(&(
        properties.transform2d(),
        properties.geometry(),
        properties.blip_fill(),
        properties.solid_fill(),
        properties.gradient_fill(),
        properties.outline(),
        properties.effect_list(),
        properties.no_fill(),
        properties.extension_list(),
        &black_white_mode,
        &preserved,
    ))
    .unwrap();
    assert_eq!(bincode::serialize(&properties).unwrap(), expected);
    let decoded: ShapeProperties = bincode::deserialize(&expected).unwrap();
    assert_eq!(serde_json::to_value(decoded).unwrap(), value);
    let legacy = bincode::serialize(&(
        properties.transform2d(),
        properties.geometry(),
        properties.blip_fill(),
        properties.solid_fill(),
        properties.gradient_fill(),
        properties.outline(),
        properties.effect_list(),
        properties.no_fill(),
        properties.extension_list(),
        &black_white_mode,
    ))
    .unwrap();
    assert!(bincode::deserialize::<ShapeProperties>(&legacy).is_err());
    let sequence = json!([
        value["transform2d"],
        value["preset_geometry"],
        value["blip_fill"],
        value["solid_fill"],
        value["gradient_fill"],
        value["outline"],
        value["effect_list"],
        value["no_fill"],
        value["extension_list"],
        value["black_white_mode"],
    ]);
    assert!(serde_json::from_value::<ShapeProperties>(sequence).is_err());
}
