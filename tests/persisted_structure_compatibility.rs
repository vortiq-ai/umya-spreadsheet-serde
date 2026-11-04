#![cfg(feature = "serde")]

use serde::{
    Serialize,
    de::DeserializeOwned,
};
use serde_json::{
    Value,
    json,
};
use umya_spreadsheet::drawing::{
    BackgroundColor,
    BodyProperties,
    FillRectangle,
    ForegroundColor,
    NormalAutoFit,
    SchemeColor,
    ShapeAutoFit,
    TextAutoFit,
    TileRectangle,
    spreadsheet::{
        DrawingObject,
        GroupShape,
        NonVisualShapeProperties,
        Picture,
        Shape,
    },
};

fn current<T: Default + Serialize>() -> Value {
    serde_json::to_value(T::default()).unwrap()
}

#[test]
fn legacy_pattern_colors_keep_their_scheme_choice() {
    let legacy = json!({"scheme_color": current::<SchemeColor>()});
    assert!(
        serde_json::from_value::<BackgroundColor>(legacy.clone())
            .unwrap()
            .scheme_color()
            .is_some()
    );
    assert!(
        serde_json::from_value::<ForegroundColor>(legacy)
            .unwrap()
            .scheme_color()
            .is_some()
    );
}

#[test]
fn legacy_rectangles_match_the_old_writers_empty_elements() {
    let fill: FillRectangle =
        serde_json::from_value(json!({"left": 7, "top": 11, "right": 13, "bottom": 17})).unwrap();
    assert_eq!(
        (fill.left(), fill.top(), fill.right(), fill.bottom()),
        (None, None, None, None)
    );
    let tile: TileRectangle = serde_json::from_value(json!({})).unwrap();
    assert_eq!(
        (tile.left(), tile.top(), tile.right(), tile.bottom()),
        (None, None, None, None)
    );
}

#[test]
fn legacy_shape_autofit_is_preserved() {
    let mut legacy = current::<BodyProperties>();
    legacy.as_object_mut().unwrap().remove("auto_fit");
    legacy["shape_auto_fit"] = json!({});
    let body: BodyProperties = serde_json::from_value(legacy).unwrap();
    assert!(matches!(body.auto_fit(), Some(TextAutoFit::Shape(_))));
}

#[test]
fn legacy_groups_emit_pictures_before_shapes() {
    let mut legacy = current::<GroupShape>();
    legacy.as_object_mut().unwrap().remove("content");
    legacy["picture_collection"] = json!([current::<Picture>()]);
    legacy["shape_collection"] = json!([current::<Shape>()]);
    let group: GroupShape = serde_json::from_value(legacy).unwrap();
    assert!(matches!(
        group.content(),
        [DrawingObject::Picture(_), DrawingObject::Shape(_)]
    ));
}

#[test]
fn legacy_nonvisual_shape_properties_retain_empty_drawing_properties() {
    let mut legacy = current::<NonVisualShapeProperties>();
    legacy
        .as_object_mut()
        .unwrap()
        .remove("non_visual_shape_drawing_properties");
    let shape: NonVisualShapeProperties = serde_json::from_value(legacy).unwrap();
    assert!(!shape.non_visual_shape_drawing_properties().text_box());
    assert!(
        shape
            .non_visual_shape_drawing_properties()
            .shape_locks()
            .is_none()
    );
    assert!(
        shape
            .non_visual_shape_drawing_properties()
            .extensions()
            .is_none()
    );
}

fn rejects_unknown<T: Default + Serialize + DeserializeOwned>() {
    let mut value = current::<T>();
    value["unknown_field"] = json!(true);
    assert!(serde_json::from_value::<T>(value).is_err());
}

#[test]
fn compatibility_visitors_reject_unknown_keys() {
    rejects_unknown::<BackgroundColor>();
    rejects_unknown::<ForegroundColor>();
    rejects_unknown::<FillRectangle>();
    rejects_unknown::<TileRectangle>();
    rejects_unknown::<BodyProperties>();
    rejects_unknown::<GroupShape>();
    rejects_unknown::<NonVisualShapeProperties>();
    rejects_unknown::<NormalAutoFit>();
}

#[test]
fn compatibility_visitors_reject_mixed_representations() {
    let mut background = current::<BackgroundColor>();
    background["scheme_color"] = current::<SchemeColor>();
    assert!(serde_json::from_value::<BackgroundColor>(background).is_err());
    let mut foreground = current::<ForegroundColor>();
    foreground["scheme_color"] = current::<SchemeColor>();
    assert!(serde_json::from_value::<ForegroundColor>(foreground).is_err());
    let mut fill = current::<FillRectangle>();
    fill["left"] = json!(0);
    assert!(serde_json::from_value::<FillRectangle>(fill).is_err());
    let mut body = current::<BodyProperties>();
    body["shape_auto_fit"] = Value::Null;
    assert!(serde_json::from_value::<BodyProperties>(body).is_err());
    let mut group = current::<GroupShape>();
    group["picture_collection"] = json!([]);
    assert!(serde_json::from_value::<GroupShape>(group).is_err());
}

#[test]
fn normal_autofit_keeps_percentage_validation() {
    let normal: NormalAutoFit =
        serde_json::from_value(json!({"font_scale": "75000", "line_spacing_reduction": "1000"}))
            .unwrap();
    assert_eq!(normal.font_scale(), Some("75000"));
    assert_eq!(normal.line_spacing_reduction(), Some("1000"));
    for value in [
        json!({"font_scale": "999"}),
        json!({"line_spacing_reduction": "13200001"}),
    ] {
        assert!(serde_json::from_value::<NormalAutoFit>(value).is_err());
    }
}

#[test]
fn compatibility_visitors_reject_duplicate_null_keys() {
    let body = serde_json::to_string(&BodyProperties::default()).unwrap();
    let body = format!("{},\"auto_fit\":null}}", body.trim_end_matches('}'));
    assert!(serde_json::from_str::<BodyProperties>(&body).is_err());
    assert!(
        serde_json::from_str::<NormalAutoFit>("{\"font_scale\":null,\"font_scale\":null}").is_err()
    );
}

#[test]
fn compatibility_visitors_accept_only_current_sequence_lengths() {
    assert!(serde_json::from_value::<TileRectangle>(json!([])).is_err());
    let nonvisual = current::<NonVisualShapeProperties>();
    assert!(
        serde_json::from_value::<NonVisualShapeProperties>(json!([
            nonvisual["non_visual_drawing_properties"]
        ]))
        .is_err()
    );
    let group = current::<GroupShape>();
    assert!(
        serde_json::from_value::<GroupShape>(json!([
            group["non_visual_group_shape_properties"],
            group["group_shape_properties"],
            [],
            []
        ]))
        .is_err()
    );
    let background = current::<BackgroundColor>();
    assert!(
        serde_json::from_value::<BackgroundColor>(json!([background["color"], false])).is_err()
    );
    assert!(serde_json::from_value::<NormalAutoFit>(json!([null])).is_err());
    assert!(serde_json::from_value::<NormalAutoFit>(json!([null, null, null])).is_err());
}

fn round_trip<T: Serialize + DeserializeOwned>(value: T) {
    let before = bincode::serialize(&value).unwrap();
    let restored: T = bincode::deserialize(&before).unwrap();
    assert_eq!(bincode::serialize(&restored).unwrap(), before);
}

#[test]
fn current_binary_field_sequences_are_preserved() {
    round_trip(BackgroundColor::default());
    round_trip(ForegroundColor::default());
    round_trip(FillRectangle::default());
    round_trip(TileRectangle::default());
    let mut body = BodyProperties::default();
    body.set_shape_auto_fit(ShapeAutoFit::default());
    round_trip(body);
    round_trip(GroupShape::default());
    round_trip(NonVisualShapeProperties::default());
    round_trip(NormalAutoFit::default());
}
