use std::io::Cursor;

use umya_spreadsheet::{
    Workbook,
    reader,
};

#[path = "support/chart_xml_fixture.rs"]
pub mod fixture;

fn xml(fill: &str) -> String {
    format!(
        r#"<c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><c:chart><c:plotArea><c:lineChart><c:grouping val="standard"/><c:ser><c:idx val="0"/><c:order val="0"/><c:val><c:numRef><c:f>Data!$B$2:$B$5</c:f></c:numRef></c:val></c:ser></c:lineChart></c:plotArea></c:chart><c:spPr>{fill}</c:spPr></c:chartSpace>"#
    )
}

fn exported(fill: &str) -> String {
    fixture::chart_xml(&fixture::imported_workbook(&xml(fill)))
}

#[test]
fn path_gradients_keep_signed_fill_and_tile_rectangles() {
    let fill = r#"<a:gradFill><a:path path="circle"><a:fillToRect l="50000" t="-80000" r="50000" b="180000"/></a:path><a:tileRect l="-25.5%" t="2000" r="3000" b="125%"/></a:gradFill>"#;
    let output = exported(fill);
    for expected in [
        r#"<a:path path="circle">"#,
        r#"l="50000""#,
        r#"t="-80000""#,
        r#"b="180000""#,
        r#"l="-25.5%""#,
        r#"b="125%""#,
    ] {
        assert!(output.contains(expected), "{expected}: {output}");
    }
    assert_eq!(
        fixture::chart_xml(&fixture::imported_workbook(&output)),
        output
    );
}

#[test]
fn preset_colors_work_in_solid_fills_and_gradient_stops() {
    for fill in [
        r#"<a:solidFill><a:prstClr val="white"><a:tint val="12.5%"/><a:alpha val="75000"/><a:lumMod val="80000"/><a:lumMod val="90000"/></a:prstClr></a:solidFill>"#,
        r#"<a:gradFill><a:gsLst><a:gs pos="0"><a:prstClr val="green"/></a:gs><a:gs pos="100000"><a:prstClr val="white"/></a:gs></a:gsLst><a:lin ang="0"/></a:gradFill>"#,
    ] {
        let output = exported(fill);
        assert!(output.contains("a:prstClr"), "{output}");
        if fill.contains("12.5%") {
            assert!(output.contains(r#"<a:tint val="12.5%"/>"#));
            assert!(
                output.find(r#"lumMod val="80000""#).unwrap()
                    < output.find(r#"lumMod val="90000""#).unwrap()
            );
        }
        assert_eq!(
            fixture::chart_xml(&fixture::imported_workbook(&output)),
            output
        );
    }
}

fn package(xml: &str) -> Vec<u8> {
    let mut book = umya_spreadsheet::new_file();
    book.sheet_mut(0).unwrap().add_chart(fixture::base_chart());
    fixture::replace_chart_part(&fixture::write_workbook(&book), xml)
}

#[test]
fn malformed_paints_return_errors_without_committing_lazy_sheets() {
    for fill in [
        r#"<a:gradFill><a:lin/><a:path path="circle"/></a:gradFill>"#,
        r#"<a:gradFill><a:path/><a:path/></a:gradFill>"#,
        r#"<a:gradFill><a:tileRect/><a:tileRect/></a:gradFill>"#,
        r#"<a:gradFill><a:path path="triangle"/></a:gradFill>"#,
        r#"<a:gradFill><a:path><a:fillToRect l="bad"/></a:path></a:gradFill>"#,
        r#"<a:solidFill><a:prstClr val="not-a-color"/></a:solidFill>"#,
        r#"<a:solidFill><a:prstClr val="white"><a:tint val="100001"/></a:prstClr></a:solidFill>"#,
        r#"<a:solidFill><a:prstClr val="white"/><a:srgbClr val="FFFFFF"/></a:solidFill>"#,
    ] {
        let source_xml = xml(fill);
        let source = package(&source_xml);
        let eager = std::panic::catch_unwind(|| {
            reader::xlsx::read_reader(Cursor::new(source.clone()), true)
        });
        assert!(
            eager.expect("paint parser must not panic").is_err(),
            "{fill}"
        );
        let mut lazy: Workbook = reader::xlsx::read_reader(Cursor::new(source), false).unwrap();
        for _ in 0..2 {
            assert!(lazy.try_read_sheet(0).is_err(), "{fill}");
            assert!(lazy.try_sheet_collection().is_err());
            assert_eq!(fixture::chart_xml(&lazy), source_xml);
        }
    }
}

fn chart_output(chart: umya_spreadsheet::Chart) -> String {
    let mut book = umya_spreadsheet::new_file();
    book.sheet_mut(0).unwrap().add_chart(chart);
    fixture::chart_xml(&book)
}

#[test]
fn native_paint_edits_update_geometry_colors_and_choices() {
    use umya_spreadsheet::drawing::{
        ColorTransform,
        LinearGradientFill,
        PathGradientFill,
        PathShadeType,
        RgbColorModelHex,
    };
    let mut chart = fixture::imported_chart(&xml(
        r#"<a:gradFill><a:path path="circle"><a:fillToRect t="-80000"/></a:path><a:tileRect l="50000"/></a:gradFill>"#,
    ));
    let gradient = chart
        .chart_space_mut()
        .shape_properties_mut()
        .unwrap()
        .gradient_fill_mut()
        .unwrap();
    gradient
        .path_gradient_fill_mut()
        .unwrap()
        .set_path(PathShadeType::Rectangle)
        .fill_to_rectangle_mut()
        .unwrap()
        .set_t("-90.5%");
    gradient.tile_rectangle_mut().unwrap().set_left("-25%");
    let output = chart_output(chart.clone());
    assert!(output.contains(r#"<a:path path="rect">"#));
    assert!(output.contains(r#"t="-90.5%""#));
    assert!(output.contains(r#"l="-25%""#));
    let gradient = chart
        .chart_space_mut()
        .shape_properties_mut()
        .unwrap()
        .gradient_fill_mut()
        .unwrap();
    gradient.set_linear_gradient_fill(LinearGradientFill::default());
    assert!(gradient.path_gradient_fill().is_none());
    gradient.set_path_gradient_fill(PathGradientFill::default());
    assert!(gradient.linear_gradient_fill().is_none());

    let mut chart = fixture::imported_chart(&xml(
        r#"<a:solidFill><a:prstClr val="white"><a:alpha val="75000"/></a:prstClr></a:solidFill>"#,
    ));
    let solid = chart
        .chart_space_mut()
        .shape_properties_mut()
        .unwrap()
        .solid_fill_mut()
        .unwrap();
    let preset = solid.preset_color_mut().unwrap();
    preset.set_val("green");
    preset.alpha_mut().unwrap().set_val("50%");
    preset
        .transforms_mut()
        .push(ColorTransform::LuminanceModulation("120000".into()));
    let output = chart_output(chart.clone());
    assert!(output.contains(r#"<a:prstClr val="green">"#));
    assert!(output.contains(r#"<a:alpha val="50%"/>"#));
    assert!(output.contains(r#"<a:lumMod val="120000"/>"#));
    let solid = chart
        .chart_space_mut()
        .shape_properties_mut()
        .unwrap()
        .solid_fill_mut()
        .unwrap();
    let mut rgb = RgbColorModelHex::default();
    rgb.set_val("123456");
    solid.set_rgb_color_model_hex(rgb);
    assert!(solid.preset_color().is_none());
    let output = chart_output(chart);
    assert!(output.contains(r#"val="123456""#));
    assert!(!output.contains("prstClr"));
}

#[test]
fn all_color_transforms_retain_order_and_legal_lexical_values() {
    let transforms = r#"<a:tint val="12.50%"/><a:shade val="100000"/><a:comp/><a:inv/><a:gray/><a:alpha val="100.99%"/><a:alphaOff val="-25.25%"/><a:alphaMod val="250%"/><a:hue val="21599999"/><a:hueOff val="-21600000"/><a:hueMod val="150000"/><a:sat val="-25%"/><a:satOff val="-10"/><a:satMod val="250000"/><a:lum val="30%"/><a:lumOff val="-5000"/><a:lumMod val="150000"/><a:red val="20%"/><a:redOff val="-1000"/><a:redMod val="100000"/><a:green val="25%"/><a:greenOff val="-1000"/><a:greenMod val="100000"/><a:blue val="50%"/><a:blueOff val="-1000"/><a:blueMod val="100000"/><a:gamma/><a:invGamma/>"#;
    let fill =
        format!(r#"<a:solidFill><a:prstClr val="darkGrey">{transforms}</a:prstClr></a:solidFill>"#);
    let output = exported(&fill);
    assert!(output.contains(transforms), "{output}");
    for input in [
        xml(&fill)
            .replace("xmlns:a=", "xmlns:d=")
            .replace("a:", "d:"),
        xml(&fill)
            .replace("xmlns:a=", "xmlns:d=")
            .replace("a:", "d:")
            .replace(
                "<d:solidFill>",
                r#"<solidFill xmlns="http://schemas.openxmlformats.org/drawingml/2006/main">"#,
            )
            .replace("</d:solidFill>", "</solidFill>"),
    ] {
        assert_eq!(
            fixture::chart_xml(&fixture::imported_workbook(&input)),
            output
        );
    }
}

#[cfg(feature = "serde")]
#[test]
fn paint_models_have_equal_xml_json_and_bincode_roundtrips() {
    use umya_spreadsheet::drawing::charts::ShapeProperties;
    for fill in [
        r#"<a:gradFill><a:gsLst><a:gs pos="0"><a:prstClr val="green"><a:alpha val="25%"/></a:prstClr></a:gs><a:gs pos="100000"><a:sysClr val="window" lastClr="FFFFFF"/></a:gs></a:gsLst><a:path path="circle"><a:fillToRect l="-80.5%" t="180000"/></a:path><a:tileRect b="-3000"/></a:gradFill>"#,
        r#"<a:solidFill><a:prstClr val="white"><a:tint val="12.5%"/><a:lumMod val="80000"/><a:lumMod val="90000"/></a:prstClr></a:solidFill>"#,
    ] {
        let chart = fixture::imported_chart(&xml(fill));
        let shape = chart.chart_space().shape_properties().unwrap();
        let json = serde_json::to_value(shape).unwrap();
        let binary = bincode::serialize(shape).unwrap();
        let from_json: ShapeProperties = serde_json::from_value(json.clone()).unwrap();
        let from_binary: ShapeProperties = bincode::deserialize(&binary).unwrap();
        for restored in [from_json, from_binary] {
            assert_eq!(serde_json::to_value(&restored).unwrap(), json);
            assert_eq!(bincode::serialize(&restored).unwrap(), binary);
            let mut restored_chart = chart.clone();
            restored_chart
                .chart_space_mut()
                .set_shape_properties(restored);
            assert_eq!(chart_output(restored_chart), chart_output(chart.clone()));
        }
    }
}

#[test]
fn invalid_native_paint_edits_are_export_errors() {
    use umya_spreadsheet::drawing::ColorTransform;
    let mut chart = fixture::imported_chart(&xml(
        r#"<a:solidFill><a:prstClr val="white"/></a:solidFill>"#,
    ));
    chart
        .chart_space_mut()
        .shape_properties_mut()
        .unwrap()
        .solid_fill_mut()
        .unwrap()
        .preset_color_mut()
        .unwrap()
        .transforms_mut()
        .push(ColorTransform::Hue("21600000".into()));
    let mut book = umya_spreadsheet::new_file();
    book.sheet_mut(0).unwrap().add_chart(chart);
    assert!(
        umya_spreadsheet::writer::xlsx::write_writer(&book, &mut Cursor::new(Vec::new())).is_err()
    );
}

#[test]
fn gradient_stop_percentage_positions_roundtrip_without_numeric_conversion() {
    for position in ["0%", "50%", "50.25%", "100.99%", "00.5%"] {
        let fill = format!(
            r#"<a:gradFill><a:gsLst><a:gs pos="{position}"><a:prstClr val="white"/></a:gs><a:gs pos="100000"><a:prstClr val="green"/></a:gs></a:gsLst></a:gradFill>"#
        );
        let output = exported(&fill);
        assert!(output.contains(&format!(r#"pos="{position}""#)), "{output}");
        assert_eq!(
            fixture::chart_xml(&fixture::imported_workbook(&output)),
            output
        );
    }
}

#[test]
fn gradient_stops_require_a_color_and_two_explicit_list_entries() {
    for stops in [
        "",
        r#"<a:gs pos="0"><a:prstClr val="white"/></a:gs>"#,
        r#"<a:gs pos="0"/><a:gs pos="100000"><a:prstClr val="white"/></a:gs>"#,
        r#"<a:gs pos="0"></a:gs><a:gs pos="100000"><a:prstClr val="white"/></a:gs>"#,
        r#"<a:gs><a:prstClr val="green"/></a:gs><a:gs pos="100000"><a:prstClr val="white"/></a:gs>"#,
    ] {
        let source = package(&xml(&format!(
            "<a:gradFill><a:gsLst>{stops}</a:gsLst></a:gradFill>"
        )));
        assert!(
            reader::xlsx::read_reader(Cursor::new(source), true).is_err(),
            "{stops}"
        );
    }
    let source = package(&xml("<a:gradFill><a:gsLst/></a:gradFill>"));
    assert!(reader::xlsx::read_reader(Cursor::new(source), true).is_err());
}

#[cfg(feature = "serde")]
#[test]
fn gradient_positions_are_checked_editable_and_serde_stable() {
    use umya_spreadsheet::drawing::GradientStop;
    let mut chart = fixture::imported_chart(&xml(
        r#"<a:gradFill><a:gsLst><a:gs pos="0"><a:prstClr val="white"/></a:gs><a:gs pos="100000"><a:prstClr val="green"/></a:gs></a:gsLst></a:gradFill>"#,
    ));
    let stop = &mut chart
        .chart_space_mut()
        .shape_properties_mut()
        .unwrap()
        .gradient_fill_mut()
        .unwrap()
        .gradient_stop_list_mut()
        .gradient_stop_mut()[0];
    assert_eq!(stop.position(), "0");
    stop.set_position("50.25%").unwrap();
    for invalid in [
        "-1", "100001", "-0%", "101%", "100.999%", "+50%", "000%", "50.%",
    ] {
        assert!(stop.set_position(invalid).is_err(), "{invalid}");
        assert_eq!(stop.position(), "50.25%");
        let mut value = serde_json::to_value(&stop).unwrap();
        value["position"] = invalid.into();
        assert!(
            serde_json::from_value::<GradientStop>(value).is_err(),
            "{invalid}"
        );
    }
    let json: GradientStop = serde_json::from_value(serde_json::to_value(&stop).unwrap()).unwrap();
    let binary: GradientStop = bincode::deserialize(&bincode::serialize(&stop).unwrap()).unwrap();
    assert_eq!(json.position(), "50.25%");
    assert_eq!(binary.position(), "50.25%");
    *stop = binary;
    let output = chart_output(chart);
    assert!(output.contains(r#"pos="50.25%""#), "{output}");
    assert_eq!(
        fixture::chart_xml(&fixture::imported_workbook(&output)),
        output
    );
}

#[test]
fn native_gradients_reject_missing_colors_and_single_stop_lists() {
    use umya_spreadsheet::drawing::GradientStop;
    let mut chart = fixture::imported_chart(&xml(
        r#"<a:gradFill><a:gsLst><a:gs pos="0"><a:prstClr val="white"/></a:gs><a:gs pos="100000"><a:prstClr val="green"/></a:gs></a:gsLst></a:gradFill>"#,
    ));
    let mut no_color = chart.clone();
    no_color
        .chart_space_mut()
        .shape_properties_mut()
        .unwrap()
        .gradient_fill_mut()
        .unwrap()
        .gradient_stop_list_mut()
        .gradient_stop_mut()[0] = GradientStop::default();
    let mut book = umya_spreadsheet::new_file();
    book.sheet_mut(0).unwrap().add_chart(no_color);
    assert!(umya_spreadsheet::writer::xlsx::write_writer(&book, Vec::new()).is_err());
    chart
        .chart_space_mut()
        .shape_properties_mut()
        .unwrap()
        .gradient_fill_mut()
        .unwrap()
        .gradient_stop_list_mut()
        .gradient_stop_mut()
        .pop();
    let mut book = umya_spreadsheet::new_file();
    book.sheet_mut(0).unwrap().add_chart(chart.clone());
    assert!(umya_spreadsheet::writer::xlsx::write_writer(&book, Vec::new()).is_err());
    chart
        .chart_space_mut()
        .shape_properties_mut()
        .unwrap()
        .gradient_fill_mut()
        .unwrap()
        .gradient_stop_list_mut()
        .gradient_stop_mut()
        .clear();
    assert!(!chart_output(chart).contains("gsLst"));
}

#[cfg(feature = "serde")]
#[test]
fn every_native_color_owns_full_ordered_transforms_and_lexical_edits() {
    use umya_spreadsheet::drawing::ColorTransform;
    let transforms =
        r#"<a:hueOff val="-10800000"/><a:lumMod val="50.25%"/><a:gamma/><a:hueOff val="60000"/>"#;
    for (name, attributes) in [
        ("srgbClr", r#"val="123456""#),
        ("schemeClr", r#"val="accent1""#),
        ("sysClr", r#"val="window" lastClr="FFFFFF""#),
        ("prstClr", r#"val="white""#),
    ] {
        let fill =
            format!("<a:solidFill><a:{name} {attributes}>{transforms}</a:{name}></a:solidFill>");
        let mut chart = fixture::imported_chart(&xml(&fill));
        assert!(chart_output(chart.clone()).contains(&fill));
        let solid = chart
            .chart_space_mut()
            .shape_properties_mut()
            .unwrap()
            .solid_fill_mut()
            .unwrap();
        let values = match name {
            "srgbClr" => solid.rgb_color_model_hex_mut().unwrap().transforms_mut(),
            "schemeClr" => solid.scheme_color_mut().unwrap().transforms_mut(),
            "sysClr" => solid.system_color_mut().unwrap().transforms_mut(),
            _ => solid.preset_color_mut().unwrap().transforms_mut(),
        };
        assert_eq!(values.len(), 4);
        assert_eq!(values[1].value(), Some("50.25%"));
        values[3] = ColorTransform::HueOffset("120000".into());
        for restored in [
            serde_json::from_slice(&serde_json::to_vec(&chart).unwrap()).unwrap(),
            bincode::deserialize(&bincode::serialize(&chart).unwrap()).unwrap(),
        ] {
            let output = chart_output(restored);
            assert!(
                output.contains(&fill.replace("60000", "120000")),
                "{name}: {output}"
            );
            assert_eq!(
                fixture::chart_xml(&fixture::imported_workbook(&output)),
                output
            );
        }
    }
}

#[cfg(feature = "serde")]
#[test]
fn native_line_ends_preserve_both_ends_and_support_ordinary_edits() {
    use umya_spreadsheet::drawing::{
        LineEndProperties,
        TailEnd,
    };
    let line = r#"<a:ln><a:headEnd type="arrow" w="sm" len="lg"/><a:tailEnd type="triangle" w="lg" len="med"/></a:ln>"#;
    let mut chart = fixture::imported_chart(&xml(line));
    let outline = chart
        .chart_space_mut()
        .shape_properties_mut()
        .unwrap()
        .outline_mut()
        .unwrap();
    assert_eq!(outline.head_end().unwrap().get_type(), "arrow");
    assert_eq!(outline.tail_end().unwrap().width(), "lg");
    outline
        .head_end_mut()
        .unwrap()
        .set_type("diamond")
        .set_width("med")
        .set_length("sm");
    let mut tail = TailEnd::default();
    tail.set_type("oval").set_width("sm").set_length("lg");
    outline.set_tail_end(tail);
    let _: &LineEndProperties = outline.tail_end().unwrap();
    for restored in [
        serde_json::from_slice(&serde_json::to_vec(&chart).unwrap()).unwrap(),
        bincode::deserialize(&bincode::serialize(&chart).unwrap()).unwrap(),
    ] {
        let output = chart_output(restored);
        assert!(output.contains(r#"<a:headEnd type="diamond" w="med" len="sm"/><a:tailEnd type="oval" w="sm" len="lg"/>"#));
        assert_eq!(
            fixture::chart_xml(&fixture::imported_workbook(&output)),
            output
        );
    }
}

#[test]
fn invalid_native_line_end_enumerations_are_fallible() {
    for end in [
        r#"<a:headEnd type="bad"/>"#,
        r#"<a:tailEnd w="wide"/>"#,
        r#"<a:headEnd len="long"/>"#,
    ] {
        for eager in [true, false] {
            let source = package(&xml(&format!("<a:ln>{end}</a:ln>")));
            let result = reader::xlsx::read_reader(Cursor::new(source), eager);
            if eager {
                assert!(result.is_err(), "{end}");
            } else {
                let mut book = result.unwrap();
                assert!(book.try_read_sheet(0).is_err(), "{end}");
            }
        }
    }
}

#[cfg(feature = "serde")]
#[test]
fn native_pattern_colors_keep_rgb_values_and_color_choice_edits() {
    use umya_spreadsheet::drawing::SchemeColor;
    let fill = r#"<a:pattFill prst="pct5"><a:fgClr><a:srgbClr val="FF0000"><a:hueOff val="60000"/></a:srgbClr></a:fgClr><a:bgClr><a:srgbClr val="FFFFFF"/></a:bgClr></a:pattFill>"#;
    let mut chart = fixture::imported_chart(&xml(fill));
    assert!(chart_output(chart.clone()).contains(fill));
    let pattern = chart
        .chart_space_mut()
        .shape_properties_mut()
        .unwrap()
        .pattern_fill_mut()
        .unwrap();
    let foreground = pattern.foreground_color_mut().unwrap();
    assert_eq!(foreground.rgb_color_model_hex().unwrap().val(), "FF0000");
    assert!(foreground.scheme_color().is_none());
    foreground.set_scheme_color(SchemeColor::default());
    assert!(foreground.rgb_color_model_hex().is_none());
    for restored in [
        serde_json::from_slice(&serde_json::to_vec(&chart).unwrap()).unwrap(),
        bincode::deserialize(&bincode::serialize(&chart).unwrap()).unwrap(),
    ] {
        let output = chart_output(restored);
        assert!(output.contains("<a:fgClr><a:schemeClr"));
        assert!(output.contains(r#"<a:bgClr><a:srgbClr val="FFFFFF"/></a:bgClr>"#));
    }
}

#[test]
fn pattern_optional_containers_do_not_fabricate_colors_but_present_ones_require_a_choice() {
    let output = exported(r#"<a:pattFill prst="pct5"/>"#);
    assert!(!output.contains("<a:fgClr"));
    assert!(!output.contains("<a:bgClr"));
    for fill in [
        r#"<a:pattFill><a:fgClr/></a:pattFill>"#,
        r#"<a:pattFill><a:bgClr></a:bgClr></a:pattFill>"#,
        r#"<a:pattFill><a:fgClr><a:srgbClr val="000000"/><a:schemeClr val="accent1"/></a:fgClr></a:pattFill>"#,
        r#"<a:ln><a:headEnd/><a:headEnd/></a:ln>"#,
        r#"<a:ln><a:tailEnd/><a:tailEnd/></a:ln>"#,
        r#"<a:solidFill><a:srgbClr val="123456"><a:hueOff val="bad"/></a:srgbClr></a:solidFill>"#,
        r#"<a:solidFill><a:schemeClr val="accent1"><a:alphaMod val="-1"/></a:schemeClr></a:solidFill>"#,
        r#"<a:solidFill><a:sysClr val="window"><a:gamma>bad</a:gamma></a:sysClr></a:solidFill>"#,
    ] {
        let source = package(&xml(fill));
        assert!(
            reader::xlsx::read_reader(Cursor::new(source), true).is_err(),
            "{fill}"
        );
    }
}
