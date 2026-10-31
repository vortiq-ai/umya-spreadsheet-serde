#![cfg(feature = "serde")]

use serde::{
    Deserialize,
    Serialize,
};
use umya_spreadsheet::{
    EnumValue,
    StringValue,
    drawing::{
        Alpha,
        ColorTransform,
        PercentageType,
        PositiveFixedPercentageType,
        PresetColor,
        RgbColorModelHex,
        SchemeColor,
        SchemeColorValues,
    },
};

#[derive(Serialize)]
struct LegacyColour {
    val:                   serde_json::Value,
    luminance:             Option<PercentageType>,
    luminance_modulation:  Option<PercentageType>,
    luminance_offset:      Option<PercentageType>,
    saturation:            Option<PercentageType>,
    saturation_modulation: Option<PercentageType>,
    shade:                 Option<PositiveFixedPercentageType>,
    alpha:                 Option<PositiveFixedPercentageType>,
    tint:                  Option<PositiveFixedPercentageType>,
}

fn legacy_colour(val: serde_json::Value) -> LegacyColour {
    fn percentage(value: i32) -> Option<PercentageType> {
        let mut result = PercentageType::default();
        result.set_val(value);
        Some(result)
    }
    fn fixed(value: i32) -> Option<PositiveFixedPercentageType> {
        let mut result = PositiveFixedPercentageType::default();
        result.set_val(value);
        Some(result)
    }
    LegacyColour {
        val,
        luminance: percentage(11000),
        luminance_modulation: percentage(22000),
        luminance_offset: percentage(33000),
        saturation: percentage(44000),
        saturation_modulation: percentage(55000),
        shade: fixed(66000),
        alpha: fixed(77000),
        tint: fixed(88000),
    }
}

fn assert_transforms(transforms: &[ColorTransform]) {
    assert_eq!(transforms.len(), 8);
    assert!(matches!(&transforms[0], ColorTransform::Luminance(v) if v == "11000"));
    assert!(matches!(&transforms[1], ColorTransform::LuminanceModulation(v) if v == "22000"));
    assert!(matches!(&transforms[2], ColorTransform::LuminanceOffset(v) if v == "33000"));
    assert!(matches!(&transforms[3], ColorTransform::Saturation(v) if v == "44000"));
    assert!(matches!(&transforms[4], ColorTransform::SaturationModulation(v) if v == "55000"));
    assert!(matches!(&transforms[5], ColorTransform::Shade(v) if v == "66000"));
    assert!(matches!(&transforms[6], ColorTransform::Alpha(v) if v.val() == "77000"));
    assert!(matches!(&transforms[7], ColorTransform::Tint(v) if v == "88000"));
}

#[test]
fn legacy_colours_preserve_all_transforms_in_old_writer_order() {
    let mut scheme = SchemeColor::default();
    scheme.set_val(SchemeColorValues::Text1);
    let val = serde_json::to_value(scheme).unwrap()["val"].clone();
    let source = serde_json::to_vec(&legacy_colour(val)).unwrap();
    let colour: SchemeColor = serde_json::from_slice(&source).unwrap();
    assert!(matches!(colour.val(), SchemeColorValues::Text1));
    assert_transforms(colour.transforms());

    let mut rgb = RgbColorModelHex::default();
    rgb.set_val("2468AC");
    let val = serde_json::to_value(rgb).unwrap()["val"].clone();
    let source = serde_json::to_vec(&legacy_colour(val)).unwrap();
    let colour: RgbColorModelHex = serde_json::from_slice(&source).unwrap();
    assert_eq!(colour.val(), "2468AC");
    assert_transforms(colour.transforms());
}

#[test]
fn legacy_preset_colour_preserves_alpha() {
    #[derive(Serialize)]
    struct LegacyPreset<'a> {
        val:   &'a str,
        alpha: Option<Alpha>,
    }
    let mut alpha = Alpha::default();
    alpha.set_val("45000");
    let source = serde_json::to_vec(&LegacyPreset {
        val:   "red",
        alpha: Some(alpha),
    })
    .unwrap();
    let colour: PresetColor = serde_json::from_slice(&source).unwrap();
    assert_eq!(colour.val(), "red");
    assert!(matches!(colour.transforms(), [ColorTransform::Alpha(v)] if v.val() == "45000"));
}

#[test]
fn colour_maps_reject_unknown_and_mixed_legacy_fields() {
    fn assert_invalid<T: serde::de::DeserializeOwned + Serialize>(value: T, legacy_key: &str) {
        let mut source = serde_json::to_value(value).unwrap();
        source
            .as_object_mut()
            .unwrap()
            .insert("unknown".into(), serde_json::Value::Null);
        assert!(serde_json::from_value::<T>(source.clone()).is_err());
        source.as_object_mut().unwrap().remove("unknown");
        source
            .as_object_mut()
            .unwrap()
            .insert(legacy_key.into(), serde_json::Value::Null);
        assert!(serde_json::from_value::<T>(source).is_err());
    }
    assert_invalid(SchemeColor::default(), "luminance_offset");
    assert_invalid(RgbColorModelHex::default(), "alpha");
    assert_invalid(PresetColor::default(), "alpha");
}

#[test]
fn current_colours_keep_the_derived_bincode_field_sequence() {
    #[derive(Deserialize, Serialize)]
    struct DerivedColour<T> {
        val:        T,
        transforms: Vec<ColorTransform>,
    }
    fn assert_round_trip<T, V>(value: &T)
    where
        T: serde::de::DeserializeOwned + Serialize,
        V: serde::de::DeserializeOwned + Serialize,
    {
        let encoded = bincode::serialize(value).unwrap();
        let derived: DerivedColour<V> =
            serde_json::from_value(serde_json::to_value(value).unwrap()).unwrap();
        assert_eq!(encoded, bincode::serialize(&derived).unwrap());
        let decoded: T = bincode::deserialize(&encoded).unwrap();
        assert_eq!(bincode::serialize(&decoded).unwrap(), encoded);
    }
    let mut scheme = SchemeColor::default();
    scheme.set_val(SchemeColorValues::Accent2);
    scheme
        .transforms_mut()
        .push(ColorTransform::Tint("12345".into()));
    assert_round_trip::<_, EnumValue<SchemeColorValues>>(&scheme);
    let mut rgb = RgbColorModelHex::default();
    rgb.set_val("ABCDEF");
    rgb.transforms_mut()
        .push(ColorTransform::Shade("34567".into()));
    assert_round_trip::<_, StringValue>(&rgb);
    let mut preset = PresetColor::default();
    preset.set_val("blue");
    preset
        .transforms_mut()
        .push(ColorTransform::Luminance("25000".into()));
    assert_round_trip::<_, Box<str>>(&preset);
}

#[test]
fn colour_sequences_reject_legacy_fields_and_extra_current_fields() {
    fn assert_sequences<T: Serialize + serde::de::DeserializeOwned>(colour: T) {
        let value = serde_json::to_value(colour).unwrap();
        let mut current = vec![value["val"].clone(), value["transforms"].clone()];
        assert!(serde_json::from_value::<T>(serde_json::Value::Array(current.clone())).is_ok());
        current.push(serde_json::Value::Null);
        assert!(serde_json::from_value::<T>(serde_json::Value::Array(current)).is_err());
        let legacy = legacy_colour(value["val"].clone());
        let old = serde_json::to_value(legacy).unwrap();
        let fields = [
            "val",
            "luminance",
            "luminance_modulation",
            "luminance_offset",
            "saturation",
            "saturation_modulation",
            "shade",
            "alpha",
            "tint",
        ];
        let sequence = fields.iter().map(|field| old[*field].clone()).collect();
        assert!(serde_json::from_value::<T>(serde_json::Value::Array(sequence)).is_err());
    }
    assert_sequences(SchemeColor::default());
    assert_sequences(RgbColorModelHex::default());
    let mut alpha = Alpha::default();
    alpha.set_val("50000");
    let sequence = vec![
        serde_json::to_value("red").unwrap(),
        serde_json::to_value(Some(alpha)).unwrap(),
    ];
    assert!(serde_json::from_value::<PresetColor>(serde_json::Value::Array(sequence)).is_err());
}

#[test]
fn legacy_unset_percentage_values_do_not_create_transforms() {
    fn empty_legacy(val: serde_json::Value) -> LegacyColour {
        LegacyColour {
            val,
            luminance: Some(PercentageType::default()),
            luminance_modulation: Some(PercentageType::default()),
            luminance_offset: Some(PercentageType::default()),
            saturation: Some(PercentageType::default()),
            saturation_modulation: Some(PercentageType::default()),
            shade: Some(PositiveFixedPercentageType::default()),
            alpha: Some(PositiveFixedPercentageType::default()),
            tint: Some(PositiveFixedPercentageType::default()),
        }
    }
    let scheme = SchemeColor::default();
    let val = serde_json::to_value(scheme).unwrap()["val"].clone();
    let source = serde_json::to_vec(&empty_legacy(val)).unwrap();
    let colour: SchemeColor = serde_json::from_slice(&source).unwrap();
    assert!(colour.transforms().is_empty());
    let rgb = RgbColorModelHex::default();
    let val = serde_json::to_value(rgb).unwrap()["val"].clone();
    let source = serde_json::to_vec(&empty_legacy(val)).unwrap();
    let colour: RgbColorModelHex = serde_json::from_slice(&source).unwrap();
    assert!(colour.transforms().is_empty());
}
