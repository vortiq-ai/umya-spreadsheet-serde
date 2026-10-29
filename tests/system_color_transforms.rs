//! System-color luminance survives the ordinary chart import/export path.
#![cfg(feature = "serde")]

use std::io::Cursor;

use umya_spreadsheet::{
    Chart,
    Workbook,
    drawing::{
        PercentageType,
        SystemColor,
    },
};

#[path = "support/chart_xml_fixture.rs"]
pub mod fixture;

const NS_A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const COLOR: &str = "<a:sysClr val=\"windowText\" lastClr=\"000000\"><a:lumMod \
                     val=\"25000\"/><a:lumOff val=\"75000\"/></a:sysClr>";

fn imported(color: &str, eager: bool) -> Workbook {
    let xml = format!(
        "<c:chartSpace xmlns:c=\"http://schemas.openxmlformats.org/drawingml/2006/chart\" \
         xmlns:a=\"{NS_A}\" xmlns:d=\"{NS_A}\" \
         xmlns:x=\"urn:foreign-color\"><c:chart><c:plotArea><c:scatterChart><c:scatterStyle \
         val=\"lineMarker\"/><c:ser><c:idx val=\"1\"/><c:order \
         val=\"1\"/><c:dLbls><c:spPr><a:ln><a:solidFill>{color}</a:solidFill></a:ln></c:spPr><c:\
         showVal val=\"1\"/></c:dLbls></c:ser><c:axId val=\"1\"/><c:axId \
         val=\"2\"/></c:scatterChart></c:plotArea></c:chart></c:chartSpace>"
    );
    let mut book = umya_spreadsheet::new_file();
    book.sheet_mut(0).unwrap().add_chart(fixture::base_chart());
    let bytes = fixture::replace_chart_part(&fixture::write_workbook(&book), &xml);
    let mut book = umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(bytes), eager).unwrap();
    book.read_sheet(0);
    book
}

fn exported(chart: Chart) -> String {
    let mut book = umya_spreadsheet::new_file();
    book.sheet_mut(0).unwrap().add_chart(chart);
    fixture::chart_xml(&book)
}

#[test]
fn label_luminance_survives_eager_and_lazy_import_and_persistence() {
    for eager in [true, false] {
        let book = imported(COLOR, eager);
        assert!(fixture::chart_xml(&book).contains(COLOR), "eager={eager}");
        let chart = &book.sheet(0).unwrap().chart_collection()[0];
        let json = serde_json::to_vec(chart).unwrap();
        let binary = bincode::serialize(chart).unwrap();
        for restored in [
            serde_json::from_slice(&json).unwrap(),
            bincode::deserialize(&binary).unwrap(),
        ] {
            assert!(exported(restored).contains(COLOR), "eager={eager}");
        }
    }
}

#[test]
fn paired_transform_tags_and_native_namespace_aliases_survive() {
    for color in [
        COLOR.replace(
            "<a:lumMod val=\"25000\"/>",
            "<a:lumMod val=\"25000\"></a:lumMod>",
        ),
        COLOR.replace("a:", "d:"),
        format!(
            "<sysClr xmlns=\"{NS_A}\" val=\"windowText\" lastClr=\"000000\"><lumMod \
             val=\"25000\"/><lumOff val=\"75000\"/></sysClr>"
        ),
    ] {
        for eager in [true, false] {
            assert!(
                fixture::chart_xml(&imported(&color, eager)).contains(COLOR),
                "{color}"
            );
        }
    }
}

#[test]
fn foreign_and_rebound_transforms_do_not_replace_native_values() {
    for foreign in [
        "<x:lumMod val=\"90000\"/>",
        "<x:lumOff val=\"90000\"><a:lumMod val=\"90000\"/></x:lumOff>",
        "<a:lumMod xmlns:a=\"urn:foreign-color\" val=\"90000\"/>",
        "<a:lumOff xmlns:a=\"urn:foreign-color\" val=\"90000\"><d:lumOff \
         val=\"90000\"/></a:lumOff>",
    ] {
        let color = COLOR.replace("</a:sysClr>", &format!("{foreign}</a:sysClr>"));
        for eager in [true, false] {
            assert!(
                fixture::chart_xml(&imported(&color, eager)).contains(COLOR),
                "{foreign}"
            );
        }
    }
}

#[test]
fn plain_system_colors_keep_their_attributes_and_empty_form() {
    for color in [
        "<a:sysClr val=\"windowText\" lastClr=\"000000\"/>",
        "<a:sysClr val=\"windowText\"/>",
    ] {
        for eager in [true, false] {
            assert!(fixture::chart_xml(&imported(color, eager)).contains(color));
        }
    }
}

#[test]
fn luminance_transform_order_and_repetitions_survive() {
    for transforms in [
        "<a:lumOff val=\"75000\"/><a:lumMod val=\"25000\"/>",
        "<a:lumMod val=\"50000\"/><a:lumOff val=\"75000\"/><a:lumMod val=\"25000\"/>",
    ] {
        let color =
            format!("<a:sysClr val=\"windowText\" lastClr=\"000000\">{transforms}</a:sysClr>");
        for eager in [true, false] {
            let book = imported(&color, eager);
            assert!(fixture::chart_xml(&book).contains(&color), "{color}");
            let chart = &book.sheet(0).unwrap().chart_collection()[0];
            let json = serde_json::to_vec(chart).unwrap();
            let binary = bincode::serialize(chart).unwrap();
            for restored in [
                serde_json::from_slice(&json).unwrap(),
                bincode::deserialize(&binary).unwrap(),
            ] {
                assert!(exported(restored).contains(&color), "{color}");
            }
        }
    }
}

#[test]
fn luminance_accessors_edit_only_the_last_matching_transform() {
    fn percentage(value: i32) -> PercentageType {
        let mut result = PercentageType::default();
        result.set_val(value);
        result
    }
    let mut old = serde_json::to_value(SystemColor::default()).unwrap();
    old.as_object_mut().unwrap().remove("luminance_transforms");
    let mut color: SystemColor = serde_json::from_value(old).unwrap();
    assert!(color.luminance_modulation().is_none());
    assert!(color.luminance_offset().is_none());
    color.set_luminance_offset(percentage(75000));
    color.set_luminance_modulation(percentage(50000));
    let mut saved = serde_json::to_value(&color).unwrap();
    saved["luminance_transforms"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({"Modulation": percentage(25000)}));
    let mut color: SystemColor = serde_json::from_value(saved.clone()).unwrap();
    assert_eq!(color.luminance_modulation().unwrap().val(), 25000);
    color.luminance_modulation_mut().unwrap().set_val(20000);
    color.set_luminance_modulation(percentage(30000));
    color.luminance_offset_mut().unwrap().set_val(60000);
    color.set_luminance_offset(percentage(65000));
    saved["luminance_transforms"][0] = serde_json::json!({"Offset": percentage(65000)});
    saved["luminance_transforms"][2] = serde_json::json!({"Modulation": percentage(30000)});
    assert_eq!(serde_json::to_value(color).unwrap(), saved);
}
