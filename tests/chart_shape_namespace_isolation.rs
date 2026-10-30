//! Chart shape descendants resolve real namespaces without promoting
//! lookalikes.
#![cfg(feature = "serde")]

use umya_spreadsheet::drawing::charts::ShapeProperties;

#[path = "support/chart_xml_fixture.rs"]
pub mod fixture;

const NS_A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const GREEN: &str = "<a:solidFill><a:srgbClr val=\"00FF00\"/></a:solidFill>";

fn shape_in(children: &str, carrier: &str) -> ShapeProperties {
    let shape = format!("<c:spPr>{children}</c:spPr>");
    let parts = match carrier {
        "point" => format!("<c:dPt><c:idx val=\"9\"/>{shape}</c:dPt>"),
        "label" => format!("<c:dLbls><c:dLbl><c:idx val=\"9\"/>{shape}</c:dLbl></c:dLbls>"),
        "trendline" => {
            format!("<c:trendline>{shape}<c:trendlineType val=\"linear\"/></c:trendline>")
        }
        _ => panic!("unknown test carrier"),
    };
    let xml = format!(
        "<c:chartSpace xmlns:c=\"http://schemas.openxmlformats.org/drawingml/2006/chart\" \
         xmlns:a=\"{NS_A}\" xmlns:d=\"{NS_A}\" \
         xmlns:x=\"urn:future-shape\"><c:chart><c:plotArea><c:lineChart><c:grouping \
         val=\"standard\"/><c:ser><c:idx val=\"1\"/><c:order \
         val=\"1\"/>{parts}<c:val><c:numRef><c:f>Data!$B$2:$B$5</c:f></c:numRef></c:val></c:ser></\
         c:lineChart></c:plotArea></c:chart></c:chartSpace>"
    );
    let chart = fixture::imported_chart(&xml);
    let series = &chart
        .chart_space()
        .chart()
        .plot_area()
        .chart_groups()
        .next()
        .unwrap()
        .series()
        .area_chart_series()[0];
    match carrier {
        "point" => series.data_points()[0].shape_properties().unwrap(),
        "label" => series.data_labels().unwrap().labels()[0]
            .shape_properties()
            .unwrap(),
        "trendline" => series.trendlines()[0].shape_properties().unwrap(),
        _ => unreachable!(),
    }
    .clone()
}

fn shape(children: &str) -> ShapeProperties {
    shape_in(children, "point")
}

fn assert_same(native: &str, edited: &str) {
    assert_eq!(
        serde_json::to_value(shape(edited)).unwrap(),
        serde_json::to_value(shape(native)).unwrap(),
        "{edited}"
    );
}

#[test]
fn solid_fill_ignores_foreign_color_choices_in_every_chart_shape_carrier() {
    for carrier in ["point", "label", "trendline"] {
        let expected = shape_in(GREEN, carrier);
        assert_eq!(
            expected
                .solid_fill()
                .unwrap()
                .rgb_color_model_hex()
                .unwrap()
                .val(),
            "00FF00"
        );
        for foreign in [
            "<x:srgbClr val=\"FF0000\"/>",
            "<x:srgbClr val=\"FF0000\"><a:lumMod val=\"90000\"/></x:srgbClr>",
            "<x:schemeClr val=\"accent1\"/>",
            "<x:sysClr val=\"window\" lastClr=\"FF0000\"/>",
        ] {
            let edited = format!("<a:solidFill><a:srgbClr val=\"00FF00\"/>{foreign}</a:solidFill>");
            assert_eq!(
                serde_json::to_value(shape_in(&edited, carrier)).unwrap(),
                serde_json::to_value(&expected).unwrap(),
                "{carrier}: {foreign}"
            );
        }
    }
}

#[test]
fn rgb_transforms_ignore_foreign_lookalikes() {
    let native = "<a:solidFill><a:srgbClr val=\"00FF00\"><a:lumMod \
                  val=\"50000\"/></a:srgbClr></a:solidFill>";
    for name in [
        "lum", "lumMod", "lumOff", "sat", "satMod", "shade", "alpha", "tint",
    ] {
        for foreign in [
            format!("<x:{name} val=\"90000\"/>"),
            format!("<x:{name} val=\"90000\"><a:lumMod val=\"10000\"/></x:{name}>"),
        ] {
            let edited = native.replace("</a:srgbClr>", &format!("{foreign}</a:srgbClr>"));
            assert_same(native, &edited);
        }
    }
}

#[test]
fn scheme_transforms_ignore_foreign_lookalikes() {
    let native = "<a:solidFill><a:schemeClr val=\"accent1\"><a:lumMod \
                  val=\"50000\"/></a:schemeClr></a:solidFill>";
    for name in [
        "lum", "lumMod", "lumOff", "sat", "satMod", "shade", "alpha", "tint",
    ] {
        let foreign = format!("<x:{name} val=\"90000\"/>");
        assert_same(
            native,
            &native.replace("</a:schemeClr>", &format!("{foreign}</a:schemeClr>")),
        );
    }
}

#[test]
fn outline_ignores_foreign_dash_and_fill_choices() {
    let native = "<a:ln><a:solidFill><a:srgbClr val=\"00FF00\"/></a:solidFill><a:prstDash \
                  val=\"solid\"/></a:ln>";
    for foreign in [
        "<x:prstDash val=\"dash\"/>",
        "<x:prstDash val=\"dash\"><a:round/></x:prstDash>",
        "<x:noFill/>",
        "<x:solidFill><a:srgbClr val=\"FF0000\"/></x:solidFill>",
    ] {
        assert_same(
            native,
            &native.replace("</a:ln>", &format!("{foreign}</a:ln>")),
        );
    }
}

fn gradient(inner: &str) -> String {
    format!(
        "<a:ln><a:gradFill><a:gsLst><a:gs pos=\"0\"><a:srgbClr val=\"00FF00\"/></a:gs><a:gs \
         pos=\"100000\"><a:srgbClr val=\"0000FF\"/></a:gs></a:gsLst>{inner}</a:gradFill></a:ln>"
    )
}

#[test]
fn gradient_fill_ignores_foreign_container_choices() {
    let native = gradient("");
    for foreign in [
        "<x:lin ang=\"60000\" scaled=\"1\"/>",
        "<x:tileRect/>",
        "<x:gsLst><a:gs pos=\"99999\"><a:srgbClr val=\"FF0000\"/></a:gs></x:gsLst>",
    ] {
        assert_same(&native, &gradient(foreign));
    }
}

#[test]
fn gradient_stops_ignore_foreign_stops_and_color_choices() {
    let native = gradient("");
    let foreign_stop = "<x:gs pos=\"90000\"><a:srgbClr val=\"FF0000\"/></x:gs>";
    assert_same(
        &native,
        &native.replace("</a:gsLst>", &format!("{foreign_stop}</a:gsLst>")),
    );
    let foreign_color = "<x:srgbClr val=\"FF0000\"/>";
    assert_same(
        &native,
        &native.replace("</a:gs>", &format!("{foreign_color}</a:gs>")),
    );
}

#[test]
fn effects_ignore_foreign_effect_owners() {
    let native =
        "<a:effectLst><a:glow rad=\"10\"><a:schemeClr val=\"accent1\"/></a:glow></a:effectLst>";
    for foreign in [
        "<x:glow rad=\"999\"><a:schemeClr val=\"accent2\"/></x:glow>",
        "<x:outerShdw dist=\"999\"><a:srgbClr val=\"FF0000\"/></x:outerShdw>",
        "<x:softEdge rad=\"999\"/>",
    ] {
        assert_same(
            native,
            &native.replace("</a:effectLst>", &format!("{foreign}</a:effectLst>")),
        );
    }
}

#[test]
fn glow_and_shadow_ignore_foreign_color_choices() {
    let glow =
        "<a:effectLst><a:glow rad=\"10\"><a:schemeClr val=\"accent1\"/></a:glow></a:effectLst>";
    assert_same(
        glow,
        &glow.replace("</a:glow>", "<x:schemeClr val=\"accent2\"/></a:glow>"),
    );
    let shadow = "<a:effectLst><a:outerShdw dist=\"10\"><a:srgbClr \
                  val=\"00FF00\"/></a:outerShdw></a:effectLst>";
    for foreign in [
        "<x:srgbClr val=\"FF0000\"/>",
        "<x:schemeClr val=\"accent2\"/>",
        "<x:prstClr val=\"red\"/>",
    ] {
        assert_same(
            shadow,
            &shadow.replace("</a:outerShdw>", &format!("{foreign}</a:outerShdw>")),
        );
    }
}

#[test]
fn preset_color_ignores_foreign_alpha_transform() {
    let native = "<a:effectLst><a:outerShdw><a:prstClr val=\"red\"><a:alpha \
                  val=\"50000\"/></a:prstClr></a:outerShdw></a:effectLst>";
    assert_same(
        native,
        &native.replace("</a:prstClr>", "<x:alpha val=\"90000\"/></a:prstClr>"),
    );
}

#[test]
fn pattern_colors_skip_foreign_wrappers_without_promoting_native_grandchildren() {
    let native = "<a:pattFill prst=\"pct5\"><a:fgClr><a:schemeClr \
                  val=\"accent1\"/></a:fgClr><a:bgClr><a:schemeClr \
                  val=\"accent2\"/></a:bgClr></a:pattFill>";
    assert_same(
        native,
        &native.replace(
            "</a:pattFill>",
            "<x:wrapper><a:fgClr><a:schemeClr val=\"accent3\"/></a:fgClr></x:wrapper></a:pattFill>",
        ),
    );
    assert_same(
        native,
        &native.replace(
            "</a:fgClr>",
            "<x:wrapper><a:schemeClr val=\"accent3\"/></x:wrapper></a:fgClr>",
        ),
    );
}

#[test]
fn native_pattern_color_aliases_and_default_namespaces_remain_equivalent() {
    let native = "<a:pattFill prst=\"pct5\"><a:fgClr><a:schemeClr \
                  val=\"accent1\"/></a:fgClr><a:bgClr><a:schemeClr \
                  val=\"accent2\"/></a:bgClr></a:pattFill>";
    assert_same(native, &native.replace("a:", "d:"));
    let default = format!(
        "<pattFill xmlns=\"{NS_A}\" prst=\"pct5\"><fgClr><schemeClr \
         val=\"accent1\"/></fgClr><bgClr><schemeClr val=\"accent2\"/></bgClr></pattFill>"
    );
    assert_same(native, &default);
}

#[test]
fn native_drawing_color_aliases_and_default_namespace_remain_equivalent() {
    let native = "<a:solidFill><a:srgbClr val=\"00FF00\"><a:lumMod \
                  val=\"50000\"/></a:srgbClr></a:solidFill><a:ln><a:prstDash val=\"dash\"/></a:ln>";
    assert_same(native, &native.replace("a:", "d:"));
    let default = format!(
        "<solidFill xmlns=\"{NS_A}\"><srgbClr val=\"00FF00\"><lumMod \
         val=\"50000\"/></srgbClr></solidFill><ln xmlns=\"{NS_A}\"><prstDash val=\"dash\"/></ln>"
    );
    assert_same(native, &default);
}

#[test]
fn rebound_drawing_prefix_cannot_recolor_or_retransform_native_colors() {
    let native = "<a:solidFill><a:srgbClr val=\"00FF00\"><a:lumMod \
                  val=\"50000\"/></a:srgbClr></a:solidFill>";
    assert_same(
        native,
        &native.replace(
            "</a:solidFill>",
            "<a:srgbClr xmlns:a=\"urn:foreign-rebound\" val=\"FF0000\"/></a:solidFill>",
        ),
    );
    assert_same(
        native,
        &native.replace(
            "</a:srgbClr>",
            "<a:lumMod xmlns:a=\"urn:foreign-rebound\" val=\"90000\"/></a:srgbClr>",
        ),
    );
}
