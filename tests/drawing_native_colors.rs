use std::io::Cursor;

use umya_spreadsheet::{
    drawing::{
        ColorTransform,
        HslColor,
        RgbColorModelPercentage,
    },
    reader,
};

#[path = "support/chart_xml_fixture.rs"]
pub mod fixture;

const NS_A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
fn xml(fill: &str) -> String {
    format!(
        r#"<c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart" xmlns:a="{NS_A}" xmlns:d="{NS_A}" xmlns:x="urn:foreign"><c:chart><c:plotArea><c:lineChart><c:grouping val="standard"/><c:ser><c:idx val="0"/><c:order val="0"/><c:val><c:numRef><c:f>Data!$B$2:$B$5</c:f></c:numRef></c:val></c:ser></c:lineChart></c:plotArea></c:chart><c:spPr>{fill}</c:spPr></c:chartSpace>"#
    )
}
fn output(chart: umya_spreadsheet::Chart) -> String {
    let mut book = umya_spreadsheet::new_file();
    book.sheet_mut(0).unwrap().add_chart(chart);
    fixture::chart_xml(&book)
}
fn package(source: &str) -> Vec<u8> {
    let mut book = umya_spreadsheet::new_file();
    book.sheet_mut(0).unwrap().add_chart(fixture::base_chart());
    fixture::replace_chart_part(&fixture::write_workbook(&book), source)
}

#[cfg(feature = "serde")]
#[test]
fn hsl_and_scrgb_native_edits_preserve_exact_scalars_and_ordered_transforms() {
    for color in [
        r#"<a:hslClr hue="5400000" sat="50.25%" lum="25000"><a:hueOff val="-60000"/><a:lumMod val="125%"/><a:hueOff val="120000"/></a:hslClr>"#,
        r#"<a:scrgbClr r="-5000" g="50.25%" b="150000"><a:hueOff val="-60000"/><a:lumMod val="125%"/><a:hueOff val="120000"/></a:scrgbClr>"#,
    ] {
        let mut chart =
            fixture::imported_chart(&xml(&format!("<a:solidFill>{color}</a:solidFill>")));
        assert!(output(chart.clone()).contains(color));
        let fill = chart
            .chart_space_mut()
            .shape_properties_mut()
            .unwrap()
            .solid_fill_mut()
            .unwrap();
        let values = if color.contains("hslClr") {
            let value = fill.hsl_color_mut().unwrap();
            assert_eq!(value.saturation(), "50.25%");
            value
                .set_hue("60000")
                .unwrap()
                .set_saturation("75.5%")
                .unwrap()
                .set_luminance("30000")
                .unwrap();
            value.transforms_mut()
        } else {
            let value = fill.rgb_color_model_percentage_mut().unwrap();
            assert_eq!(value.green(), "50.25%");
            value
                .set_red("25%")
                .unwrap()
                .set_green("75000")
                .unwrap()
                .set_blue("-5%")
                .unwrap();
            value.transforms_mut()
        };
        assert_eq!(values.len(), 3);
        values[2] = ColorTransform::HueOffset("180000".into());
        let expected = output(chart.clone());
        assert!(
            expected.contains(
                r#"<a:hueOff val="-60000"/><a:lumMod val="125%"/><a:hueOff val="180000"/>"#
            )
        );
        for restored in [
            serde_json::from_slice(&serde_json::to_vec(&chart).unwrap()).unwrap(),
            bincode::deserialize(&bincode::serialize(&chart).unwrap()).unwrap(),
        ] {
            assert_eq!(output(restored), expected);
        }
        assert_eq!(
            fixture::chart_xml(&fixture::imported_workbook(&expected)),
            expected
        );
    }
    let mut hsl = HslColor::default();
    assert!(hsl.set_hue("21600000").is_err());
    assert!(hsl.set_hue("-1").is_err());
    assert!(hsl.set_saturation("bad").is_err());
    assert_eq!(hsl.hue(), "0");
    let mut rgb = RgbColorModelPercentage::default();
    assert!(rgb.set_blue("bad").is_err());
    assert_eq!(rgb.blue(), "0");
}

#[test]
#[ignore = "the DrawingML colour model is held at the fleet pin"]
fn native_color_choices_work_in_gradients_pattern_colors_and_namespace_aliases() {
    let hsl = r#"<a:hslClr hue="0" sat="100000" lum="50%"/>"#;
    let rgb = r#"<a:scrgbClr r="100000" g="0" b="0"/>"#;
    for fill in [
        format!("<a:solidFill>{hsl}</a:solidFill>"),
        format!("<a:solidFill>{rgb}</a:solidFill>"),
        format!("<a:effectLst><a:glow>{hsl}</a:glow></a:effectLst>"),
        format!("<a:effectLst><a:outerShdw>{rgb}</a:outerShdw></a:effectLst>"),
        format!(
            r#"<a:gradFill><a:gsLst><a:gs pos="0">{hsl}</a:gs><a:gs pos="100000">{rgb}</a:gs></a:gsLst></a:gradFill>"#
        ),
        format!(
            r#"<a:pattFill prst="pct5"><a:fgClr>{hsl}</a:fgClr><a:bgClr>{rgb}</a:bgClr></a:pattFill>"#
        ),
    ] {
        let expected = output(fixture::imported_chart(&xml(&fill)));
        for name in ["hslClr", "scrgbClr"] {
            if fill.contains(name) {
                assert!(expected.contains(name), "{expected}");
            }
        }
        for source in [
            xml(&fill.replace("a:", "d:")),
            xml(&fill.replace("<a:", "<").replace("</a:", "</"))
                .replace("<c:spPr>", &format!(r#"<c:spPr xmlns="{NS_A}">"#)),
            xml(&fill.replace(
                "</a:solidFill>",
                &format!(
                    "<x:hslClr><a:scrgbClr r=\"1\" g=\"1\" b=\"1\"/></x:hslClr></a:solidFill>"
                ),
            )),
        ] {
            assert_eq!(output(fixture::imported_chart(&source)), expected);
        }
    }
}

#[test]
fn malformed_required_native_color_attributes_and_multiple_choices_return_errors() {
    for fill in [
        r#"<a:solidFill><a:hslClr hue="0" sat="0"/></a:solidFill>"#,
        r#"<a:solidFill><a:hslClr hue="21600000" sat="0" lum="0"/></a:solidFill>"#,
        r#"<a:solidFill><a:scrgbClr r="0" g="0"/></a:solidFill>"#,
        r#"<a:solidFill><a:scrgbClr r="bad" g="0" b="0"/></a:solidFill>"#,
        r#"<a:solidFill><a:hslClr hue="0" sat="0" lum="0"/><a:scrgbClr r="0" g="0" b="0"/></a:solidFill>"#,
    ] {
        for eager in [true, false] {
            let result = reader::xlsx::read_reader(Cursor::new(package(&xml(fill))), eager);
            if eager {
                assert!(result.is_err(), "{fill}");
            } else {
                let mut book = result.unwrap();
                assert!(book.try_read_sheet(0).is_err(), "{fill}");
            }
        }
    }
}
