use umya_spreadsheet::drawing::charts::*;

const CX: &str = "http://schemas.microsoft.com/office/drawing/2014/chartex";

#[derive(Default)]
struct RecordingResolver {
    calls: Vec<String>,
    fail:  bool,
    value: String,
}
impl ChartExDataResolver for RecordingResolver {
    fn text_value(
        &mut self,
        host: &str,
        formula: &ChartExFormula,
    ) -> Result<Option<String>, String> {
        self.calls.push(format!("{host}:{}", formula.text()));
        if self.fail {
            return Err("unresolved source".into());
        }
        Ok(Some(self.value.clone()))
    }

    fn string_levels(
        &mut self,
        host: &str,
        reference: &ChartExStringReference,
    ) -> Result<Vec<ChartExStringLevel>, String> {
        self.calls
            .push(format!("{host}:{}", reference.formula().text()));
        if self.fail {
            return Err("unresolved source".into());
        }
        Ok(vec![ChartExStringLevel {
            point_count: 1,
            points: vec![ChartExStringPoint {
                index: 0,
                value: self.value.clone(),
            }],
            ..Default::default()
        }])
    }

    fn numeric_levels(
        &mut self,
        host: &str,
        reference: &ChartExNumericReference,
    ) -> Result<Vec<ChartExNumericLevel>, String> {
        self.calls
            .push(format!("{host}:{}", reference.formula().text()));
        if self.fail {
            return Err("unresolved source".into());
        }
        Ok(vec![])
    }
}

fn document(dimensions: &str, series: &str) -> String {
    format!(
        "<cx:chartSpace xmlns:cx=\"{CX}\"><cx:chartData><cx:data \
         id=\"7\">{dimensions}</cx:data></cx:chartData><cx:chart><cx:plotArea><cx:\
         plotAreaRegion>{series}</cx:plotAreaRegion></cx:plotArea></cx:chart></cx:chartSpace>"
    )
}
fn literal_document(layout: &str) -> String {
    document(
        "<cx:strDim type=\"cat\"><cx:lvl ptCount=\"4\" name=\"\"><cx:pt \
         idx=\"0\">same</cx:pt><cx:pt idx=\"3\">same</cx:pt></cx:lvl><cx:lvl ptCount=\"4\"><cx:pt \
         idx=\"0\">parent</cx:pt></cx:lvl></cx:strDim><cx:numDim type=\"val\"><cx:lvl \
         ptCount=\"4\" formatCode=\"0.00\"><cx:pt idx=\"3\">12.5</cx:pt></cx:lvl></cx:numDim>",
        &format!("<cx:series layoutId=\"{layout}\"><cx:dataId val=\"7\"/></cx:series>"),
    )
}
fn export(chart: &ChartExSpace) -> String {
    String::from_utf8(
        chart
            .to_xml("Data", &mut RecordingResolver::default())
            .unwrap(),
    )
    .unwrap()
}

#[test]
fn all_layouts_have_editable_series() {
    for layout in [
        ChartExSeriesLayout::BoxWhisker,
        ChartExSeriesLayout::ClusteredColumn,
        ChartExSeriesLayout::Funnel,
        ChartExSeriesLayout::ParetoLine,
        ChartExSeriesLayout::RegionMap,
        ChartExSeriesLayout::Sunburst,
        ChartExSeriesLayout::Treemap,
        ChartExSeriesLayout::Waterfall,
    ] {
        let mut chart =
            ChartExSpace::from_xml(literal_document(layout.as_str()).as_bytes()).unwrap();
        let series = &mut chart.chart_mut().unwrap().series_mut()[0];
        assert_eq!(series.layout(), layout);
        series.set_hidden(Some(true));
        series.layout_properties_mut().set_subtotal_indices([0, 3]);
        let reread = ChartExSpace::from_xml(export(&chart).as_bytes()).unwrap();
        let series = &reread.chart().unwrap().series()[0];
        assert_eq!(series.hidden, Some(true));
        assert_eq!(series.layout(), layout);
        let indices = &series
            .layout_properties
            .as_ref()
            .unwrap()
            .subtotals
            .as_ref()
            .unwrap()
            .indices;
        assert_eq!(
            indices.iter().map(|value| value.value).collect::<Vec<_>>(),
            [0, 3]
        );
    }
}

#[test]
fn literal_hierarchy_sparse_indexes_and_mixed_dimension_order_survive_cache_strip() {
    let mut chart = ChartExSpace::from_xml(literal_document("sunburst").as_bytes()).unwrap();
    chart.strip_imported_caches();
    let dimensions = &mut chart.data_mut().unwrap().data[0].dimensions;
    let ChartExDimension::String(dimension) = &mut dimensions[0] else {
        panic!("string first")
    };
    let ChartExDimensionSource::Literal(levels) = &mut dimension.source else {
        panic!("literal")
    };
    assert_eq!(levels.len(), 2);
    assert_eq!(levels[0].name.as_deref(), Some(""));
    assert_eq!(levels[1].name, None);
    levels[1].points[0].value = "edited parent".into();
    let reread = ChartExSpace::from_xml(export(&chart).as_bytes()).unwrap();
    let dimensions = &reread.data().unwrap().data[0].dimensions;
    let ChartExDimension::String(dimension) = &dimensions[0] else {
        panic!("string first")
    };
    let ChartExDimensionSource::Literal(levels) = &dimension.source else {
        panic!("literal")
    };
    assert_eq!(levels[0].point_count, 4);
    assert_eq!(levels[0].points[1].index, 3);
    assert_eq!(levels[1].points[0].value, "edited parent");
    assert!(matches!(dimensions[1], ChartExDimension::Numeric(_)));
}

#[test]
fn reference_exports_resolve_current_values_with_the_host_and_fail_explicitly() {
    let xml = document(
        "<cx:strDim type=\"cat\"><cx:f dir=\"row\">_xlchart.v1.0</cx:f><cx:lvl \
         ptCount=\"1\"><cx:pt idx=\"0\">stale</cx:pt></cx:lvl></cx:strDim>",
        "<cx:series layoutId=\"waterfall\"><cx:dataId val=\"7\"/></cx:series>",
    );
    let mut chart = ChartExSpace::from_xml(xml.as_bytes()).unwrap();
    let mut resolver = RecordingResolver {
        value: "fresh".into(),
        ..Default::default()
    };
    let output = String::from_utf8(chart.to_xml("Local sheet", &mut resolver).unwrap()).unwrap();
    assert!(output.contains(">fresh</cx:pt>"));
    assert!(!output.contains("stale"));
    assert_eq!(resolver.calls, ["Local sheet:_xlchart.v1.0"]);
    chart.visit_formulas_mut(&mut |formula| {
        formula.set_string_value("Data!A1:A4");
    });
    let output = String::from_utf8(chart.to_xml("Local sheet", &mut resolver).unwrap()).unwrap();
    assert!(output.contains("Data!A1:A4"));
    assert_eq!(resolver.calls.last().unwrap(), "Local sheet:Data!A1:A4");
    resolver.fail = true;
    assert!(
        chart
            .to_xml("Local sheet", &mut resolver)
            .unwrap_err()
            .contains("unresolved source")
    );
}

#[test]
fn namespaces_resolve_without_accepting_foreign_children() {
    let xml = literal_document("sunburst");
    let aliased = xml.replace("cx:", "q:").replace("xmlns:cx", "xmlns:q");
    assert!(ChartExSpace::from_xml(aliased.as_bytes()).is_ok());
    let default_ns = xml.replace("xmlns:cx", "xmlns").replace("cx:", "");
    assert!(ChartExSpace::from_xml(default_ns.as_bytes()).is_ok());
    assert!(ChartExSpace::from_xml(xml.replace(CX, "urn:foreign").as_bytes()).is_err());
    let foreign = xml.replace(
        "<cx:plotAreaRegion>",
        "<cx:plotAreaRegion><f:series xmlns:f=\"urn:foreign\" layoutId=\"waterfall\"><cx:series \
         layoutId=\"funnel\"/></f:series>",
    );
    assert!(ChartExSpace::from_xml(foreign.as_bytes()).is_err());
}

#[test]
fn malformed_known_content_is_not_silently_discarded() {
    let xml = literal_document("waterfall");
    for invalid in [
        format!("{xml}{xml}"),
        format!("{xml}unexpected"),
        xml.replace("id=\"7\"", ""),
        xml.replace("layoutId=\"waterfall\"", "layoutId=\"unknown\""),
        xml.replace("<cx:dataId val=\"7\"/>", "<cx:dataId val=\"99\"/>"),
        xml.replace(
            "<cx:dataId val=\"7\"/>",
            "<cx:dataId val=\"7\"/><cx:dataId val=\"7\"/>",
        ),
        xml.replace("</cx:series>", "<cx:unknown/></cx:series>"),
        xml.replace(
            "</cx:chartData>",
            "<cx:data id=\"7\"><cx:numDim type=\"val\"><cx:lvl \
             ptCount=\"0\"/></cx:numDim></cx:data></cx:chartData>",
        ),
    ] {
        assert!(
            ChartExSpace::from_xml(invalid.as_bytes()).is_err(),
            "{invalid}"
        );
    }
}

#[cfg(feature = "serde")]
#[test]
fn edited_charts_roundtrip_in_json_and_bincode() {
    let mut chart = ChartExSpace::from_xml(literal_document("waterfall").as_bytes()).unwrap();
    chart.chart_mut().unwrap().series_mut()[0]
        .layout_properties_mut()
        .set_subtotal_indices([0, 3]);
    let value = serde_json::to_value(&chart).unwrap();
    let json: ChartExSpace = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(serde_json::to_value(&json).unwrap(), value);
    let bytes = bincode::serialize(&chart).unwrap();
    let binary: ChartExSpace = bincode::deserialize(&bytes).unwrap();
    assert_eq!(serde_json::to_value(&binary).unwrap(), value);
    assert_eq!(export(&chart), export(&json));
    assert_eq!(export(&chart), export(&binary));
}

#[test]
fn reference_titles_are_refreshed_instead_of_reusing_imported_text() {
    let xml = literal_document("waterfall").replace(
        "<cx:chart>",
        "<cx:chart><cx:title><cx:tx><cx:txData><cx:f>Data!A1</cx:f><cx:v>stale \
         title</cx:v></cx:txData></cx:tx></cx:title>",
    );
    let chart = ChartExSpace::from_xml(xml.as_bytes()).unwrap();
    let mut resolver = RecordingResolver {
        value: "updated title".into(),
        ..Default::default()
    };
    let output = String::from_utf8(chart.to_xml("Data", &mut resolver).unwrap()).unwrap();
    assert!(output.contains("updated title"));
    assert!(!output.contains("stale title"));
    assert_eq!(resolver.calls, ["Data:Data!A1"]);
}

#[test]
fn layout_properties_are_native_and_conflicting_choices_fail() {
    let xml = literal_document("boxWhisker").replace(
        "</cx:series>",
        "<cx:layoutPr><cx:visibility meanLine=\"0\" outliers=\"1\"/><cx:binning \
         intervalClosed=\"l\" underflow=\"auto\" \
         overflow=\"99\"><cx:binCount>4</cx:binCount></cx:binning><cx:statistics \
         quartileMethod=\"inclusive\"/></cx:layoutPr></cx:series>",
    );
    let mut chart = ChartExSpace::from_xml(xml.as_bytes()).unwrap();
    let properties = chart.chart_mut().unwrap().series_mut()[0].layout_properties_mut();
    assert_eq!(
        properties.visibility.as_ref().unwrap().mean_line,
        Some(false)
    );
    properties.binning.as_mut().unwrap().bin_count = Some(8);
    let output = export(&chart);
    assert!(output.contains("<cx:binCount val=\"8\"/>"));
    assert!(output.contains("quartileMethod=\"inclusive\""));
    let invalid = xml.replace(
        "<cx:binCount>4</cx:binCount>",
        "<cx:binSize>2.5</cx:binSize><cx:binCount>4</cx:binCount>",
    );
    assert!(ChartExSpace::from_xml(invalid.as_bytes()).is_err());
}

#[test]
fn repeated_geo_cache_variants_keep_order_and_are_editable() {
    let xml = literal_document("regionMap").replace(
        "</cx:series>",
        "<cx:layoutPr><cx:geography cultureLanguage=\"en-US\" cultureRegion=\"US\" \
         attribution=\"source\"><cx:geoCache \
         provider=\"test\"><cx:binary>AQID</cx:binary><cx:clear/><cx:binary>BAU=</cx:binary></cx:\
         geoCache></cx:geography></cx:layoutPr></cx:series>",
    );
    let mut chart = ChartExSpace::from_xml(xml.as_bytes()).unwrap();
    let cache = chart.chart_mut().unwrap().series_mut()[0]
        .layout_properties_mut()
        .geography
        .as_mut()
        .unwrap()
        .geo_cache
        .as_mut()
        .unwrap();
    assert_eq!(cache.entries.len(), 3);
    let ChartExGeoCacheEntry::Binary(binary) = &mut cache.entries[0] else {
        panic!("binary")
    };
    binary.0 = vec![9, 8];
    let reread = ChartExSpace::from_xml(export(&chart).as_bytes()).unwrap();
    let cache = reread.chart().unwrap().series()[0]
        .layout_properties
        .as_ref()
        .unwrap()
        .geography
        .as_ref()
        .unwrap()
        .geo_cache
        .as_ref()
        .unwrap();
    let ChartExGeoCacheEntry::Binary(binary) = &cache.entries[0] else {
        panic!("binary")
    };
    assert_eq!(binary.0, [9, 8]);
    assert!(matches!(cache.entries[1], ChartExGeoCacheEntry::Clear(_)));
    assert!(matches!(cache.entries[2], ChartExGeoCacheEntry::Binary(_)));
}

#[test]
fn foreign_direct_children_are_rejected_instead_of_silently_lost() {
    let source = literal_document("sunburst");
    for child in [
        "<future:item xmlns:future=\"urn:future\"/>",
        "<future:item xmlns:future=\"urn:future\"><cx:series layoutId=\"funnel\"/></future:item>",
    ] {
        let xml = source.replace(
            "<cx:plotAreaRegion>",
            &format!("<cx:plotAreaRegion>{child}"),
        );
        assert!(
            ChartExSpace::from_xml(xml.as_bytes()).is_err(),
            "unsupported child was discarded: {child}"
        );
    }
}

#[test]
fn malformed_foreign_subtrees_return_errors_without_panicking() {
    let xml = format!("<cx:chartSpace xmlns:cx=\"{CX}\"><future:item xmlns:future=\"urn:future\">");
    let result = std::panic::catch_unwind(|| ChartExSpace::from_xml(xml.as_bytes()));
    assert!(result.is_ok(), "the fallible XML reader panicked");
    assert!(result.unwrap().is_err());
}

#[test]
fn future_extension_content_is_preserved_at_its_declared_boundary() {
    let source = literal_document("sunburst").replace(
        "</cx:chartSpace>",
        "<cx:extLst><cx:ext uri=\"urn:future-extension\"><future:item xmlns:future=\"urn:future\" \
         xmlns:q=\"urn:types\" \
         Type=\"q:Kind\">kept</future:item></cx:ext></cx:extLst></cx:chartSpace>",
    );
    let chart = ChartExSpace::from_xml(source.as_bytes()).unwrap();
    let output = export(&chart);
    assert!(output.contains("urn:future-extension"));
    assert!(output.contains("Type=\"q:Kind\""));
    assert!(output.contains("xmlns:q=\"urn:types\""));
    let reread = ChartExSpace::from_xml(output.as_bytes()).unwrap();
    assert_eq!(export(&reread), output);
}

#[test]
fn malformed_drawingml_owners_return_errors_without_panicking() {
    for owner in ["spPr", "txPr"] {
        let xml = format!("<cx:chartSpace xmlns:cx=\"{CX}\"><cx:{owner}>");
        let result = std::panic::catch_unwind(|| ChartExSpace::from_xml(xml.as_bytes()));
        assert!(result.is_ok(), "truncated {owner} panicked");
        assert!(result.unwrap().is_err(), "truncated {owner} was accepted");
    }
}

#[test]
fn invalid_drawingml_line_width_returns_an_error_without_panicking() {
    let xml = literal_document("waterfall").replace(
        "</cx:chartSpace>",
        "<cx:spPr xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\"><a:ln \
         w=\"bad\"></a:ln></cx:spPr></cx:chartSpace>",
    );
    let result = std::panic::catch_unwind(|| ChartExSpace::from_xml(xml.as_bytes()));
    assert!(result.is_ok(), "invalid DrawingML width panicked");
    assert!(
        result.unwrap().is_err(),
        "invalid DrawingML width was accepted"
    );
}

#[test]
fn missing_drawingml_geometry_attributes_return_errors_without_panicking() {
    let xml = literal_document("waterfall").replace(
        "</cx:chartSpace>",
        "<cx:spPr xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\"><a:prstGeom><a:avLst/></a:prstGeom></cx:spPr></cx:chartSpace>",
    );
    let result = std::panic::catch_unwind(|| ChartExSpace::from_xml(xml.as_bytes()));
    assert!(result.is_ok(), "missing DrawingML geometry preset panicked");
    assert!(
        result.unwrap().is_err(),
        "missing DrawingML geometry preset was accepted"
    );
}

#[test]
fn missing_drawingml_color_attributes_return_errors_without_panicking() {
    for color in ["schemeClr", "srgbClr"] {
        let xml = literal_document("waterfall").replace(
            "</cx:chartSpace>",
            &format!("<cx:spPr xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\"><a:solidFill><a:{color}/></a:solidFill></cx:spPr></cx:chartSpace>"),
        );
        let result = std::panic::catch_unwind(|| ChartExSpace::from_xml(xml.as_bytes()));
        assert!(result.is_ok(), "missing DrawingML {color} value panicked");
        assert!(
            result.unwrap().is_err(),
            "missing DrawingML {color} value was accepted"
        );
    }
}

#[test]
fn drawingml_geometry_accepts_namespace_aliases() {
    let xml = literal_document("waterfall").replace(
        "</cx:chartSpace>",
        "<cx:spPr xmlns:d=\"http://schemas.openxmlformats.org/drawingml/2006/main\"><d:prstGeom \
         prst=\"rect\"><d:avLst/></d:prstGeom></cx:spPr></cx:chartSpace>",
    );
    let result = std::panic::catch_unwind(|| ChartExSpace::from_xml(xml.as_bytes()));
    assert!(result.is_ok(), "valid DrawingML namespace alias panicked");
    assert!(
        result.unwrap().is_ok(),
        "valid DrawingML namespace alias was rejected"
    );
}

#[test]
fn drawingml_empty_and_expanded_properties_have_the_same_values() {
    for property in [
        r#"<a:prstGeom prst="ellipse"/>"#,
        r#"<a:ln w="34"/>"#,
        r#"<a:xfrm rot="120"><a:off x="12" y="-4"/><a:ext cx="200" cy="300"/></a:xfrm>"#,
    ] {
        let expanded = property.replace("/>", "></a:prstGeom>");
        let expanded = if property.starts_with("<a:ln ") {
            property.replace("/>", "></a:ln>")
        } else if property.starts_with("<a:xfrm ") {
            property
                .replace(
                    r#"<a:off x="12" y="-4"/>"#,
                    r#"<a:off x="12" y="-4"></a:off>"#,
                )
                .replace(
                    r#"<a:ext cx="200" cy="300"/>"#,
                    r#"<a:ext cx="200" cy="300"></a:ext>"#,
                )
        } else {
            expanded
        };
        let parse = |content: &str| {
            let xml = literal_document("waterfall").replace("</cx:chartSpace>", &format!(r#"<cx:spPr xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">{content}</cx:spPr></cx:chartSpace>"#));
            ChartExSpace::from_xml(xml.as_bytes()).unwrap()
        };
        let empty = export(&parse(property));
        let expanded = export(&parse(&expanded));
        assert_eq!(empty, expanded, "{property}");
        if property.starts_with("<a:prstGeom ") {
            assert!(empty.contains(r#"prst="ellipse""#));
        }
        if property.starts_with("<a:ln ") {
            assert!(empty.contains(r#"w="34""#));
        }
        if property.starts_with("<a:xfrm ") {
            assert!(empty.contains(r#"x="12""#));
        }
    }
}

#[test]
fn malformed_drawingml_transforms_and_gradients_return_errors() {
    for property in [
        r#"<a:xfrm><a:off x="bad" y="2"/><a:ext cx="3" cy="4"/></a:xfrm>"#,
        r#"<a:solidFill><a:srgbClr val="112233"><a:lumMod val="bad"/></a:srgbClr></a:solidFill>"#,
        r#"<a:ln><a:gradFill><a:gsLst><a:gs pos="bad"><a:srgbClr val="112233"/></a:gs></a:gsLst></a:gradFill></a:ln>"#,
    ] {
        let xml = literal_document("waterfall").replace("</cx:chartSpace>", &format!(r#"<cx:spPr xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">{property}</cx:spPr></cx:chartSpace>"#));
        let result = std::panic::catch_unwind(|| ChartExSpace::from_xml(xml.as_bytes()));
        assert!(result.is_ok(), "malformed DrawingML panicked: {property}");
        assert!(
            result.unwrap().is_err(),
            "malformed DrawingML was accepted: {property}"
        );
    }
}

#[test]
fn drawingml_gradients_and_color_transforms_keep_native_values() {
    let content = r#"<a:ln><a:gradFill><a:gsLst><a:gs pos="30000"><a:srgbClr val="112233"><a:lumMod val="65000"/></a:srgbClr></a:gs><a:gs pos="100000"><a:srgbClr val="445566"/></a:gs></a:gsLst><a:lin ang="5400000" scaled="true"/></a:gradFill></a:ln>"#;
    let xml = literal_document("waterfall").replace("</cx:chartSpace>", &format!(r#"<cx:spPr xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">{content}</cx:spPr></cx:chartSpace>"#));
    let chart = ChartExSpace::from_xml(xml.as_bytes()).unwrap();
    let output = export(&chart);
    for expected in [
        r#"pos="30000""#,
        r#"val="112233""#,
        r#"val="65000""#,
        r#"ang="5400000""#,
    ] {
        assert!(output.contains(expected), "missing {expected}: {output}");
    }
    assert!(ChartExSpace::from_xml(output.as_bytes()).is_ok());
}

#[test]
fn nested_shape_extensions_keep_their_original_namespace_context() {
    let xml = literal_document("waterfall").replace(
        "</cx:chartSpace>",
        r#"<cx:spPr xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:f="urn:shape-future"><a:extLst><a:ext uri="{shape-future}"><f:payload future="f:Value" xml:space="preserve"> A &amp; B </f:payload></a:ext></a:extLst></cx:spPr></cx:chartSpace>"#,
    );
    let chart = ChartExSpace::from_xml(xml.as_bytes()).unwrap();
    let output = export(&chart);
    assert!(output.contains("urn:shape-future"), "{output}");
    assert!(output.contains("f:Value"), "{output}");
    assert!(output.contains(" A &amp; B "), "{output}");
    let reread = ChartExSpace::from_xml(output.as_bytes()).unwrap();
    assert_eq!(export(&reread), output);
}

#[test]
fn chart_shape_gradients_are_native_editable_and_roundtrip() {
    let xml = literal_document("waterfall").replace(
        "</cx:chartSpace>",
        r#"<cx:spPr xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><a:gradFill rotWithShape="1"><a:gsLst><a:gs pos="0"><a:srgbClr val="112233"/></a:gs><a:gs pos="100000"><a:srgbClr val="445566"/></a:gs></a:gsLst><a:lin ang="0" scaled="1"/></a:gradFill></cx:spPr></cx:chartSpace>"#,
    );
    let mut chart = ChartExSpace::from_xml(xml.as_bytes()).unwrap();
    let shape = chart.shape_properties.as_mut().unwrap();
    let gradient = shape.gradient_fill_mut().unwrap();
    assert_eq!(gradient.gradient_stop_list().gradient_stop().len(), 2);
    gradient.gradient_stop_list_mut().gradient_stop_mut()[0]
        .set_position("1000")
        .unwrap();
    let output = export(&chart);
    assert!(output.contains("<a:gradFill"), "{output}");
    assert!(output.contains("pos=\"1000\""), "{output}");
    assert_eq!(
        export(&ChartExSpace::from_xml(output.as_bytes()).unwrap()),
        output
    );
    #[cfg(feature = "serde")]
    {
        let json: ChartExSpace =
            serde_json::from_value(serde_json::to_value(&chart).unwrap()).unwrap();
        let binary: ChartExSpace =
            bincode::deserialize(&bincode::serialize(&chart).unwrap()).unwrap();
        assert_eq!(export(&json), output);
        assert_eq!(export(&binary), output);
    }
}

#[test]
fn duplicate_and_conflicting_shape_properties_are_not_silently_replaced() {
    for children in [
        "<a:solidFill/><a:solidFill/>",
        "<a:noFill/><a:solidFill/>",
        "<a:xfrm/><a:xfrm/>",
        "<a:ln/><a:ln/>",
        r#"<a:solidFill><a:srgbClr val="112233"/><a:srgbClr val="445566"/></a:solidFill>"#,
        r#"<a:solidFill><a:srgbClr val="112233"/><a:schemeClr val="accent1"/></a:solidFill>"#,
        r#"<a:gradFill><a:gsLst><a:gs pos="0"><a:srgbClr val="112233"/><a:schemeClr val="accent1"/></a:gs><a:gs pos="100000"><a:srgbClr val="445566"/></a:gs></a:gsLst></a:gradFill>"#,
    ] {
        let xml = literal_document("waterfall").replace(
            "</cx:chartSpace>",
            &format!(r#"<cx:spPr xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">{children}</cx:spPr></cx:chartSpace>"#),
        );
        assert!(
            ChartExSpace::from_xml(xml.as_bytes()).is_err(),
            "{children}"
        );
    }
}

#[test]
fn setting_a_chart_fill_replaces_the_previous_choice() {
    let mut shape = ShapeProperties::default();
    shape.set_gradient_fill(umya_spreadsheet::drawing::GradientFill::default());
    shape.set_solid_fill(umya_spreadsheet::drawing::SolidFill::default());
    assert!(shape.gradient_fill().is_none());
    assert!(shape.solid_fill().is_some());
    shape.set_no_fill(umya_spreadsheet::drawing::NoFill::default());
    assert!(shape.solid_fill().is_none());
    assert!(shape.no_fill().is_some());
}

#[cfg(feature = "serde")]
#[test]
fn serde_cannot_export_conflicting_shape_fills() {
    use umya_spreadsheet::drawing::{
        GradientFill,
        NoFill,
        PatternFill,
        SolidFill,
    };
    for (field, fill) in [
        (
            "solid_fill",
            serde_json::to_value(SolidFill::default()).unwrap(),
        ),
        ("no_fill", serde_json::to_value(NoFill::default()).unwrap()),
        (
            "pattern_fill",
            serde_json::to_value(PatternFill::default()).unwrap(),
        ),
    ] {
        let mut value = serde_json::to_value(ShapeProperties::default()).unwrap();
        value["gradient_fill"] = serde_json::to_value(GradientFill::default()).unwrap();
        value[field] = fill;
        let mut chart = ChartExSpace::from_xml(literal_document("waterfall").as_bytes()).unwrap();
        chart.shape_properties = Some(serde_json::from_value(value).unwrap());
        let book = umya_spreadsheet::new_file();
        let mut resolver = WorkbookChartExResolver::new(&book);
        let error = chart.to_xml("Sheet1", &mut resolver).unwrap_err();
        assert!(
            error.contains("conflicting shape fills"),
            "{field}: {error}"
        );
    }
}
