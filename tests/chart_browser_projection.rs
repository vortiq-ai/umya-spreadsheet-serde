#![cfg(feature = "serde")]

#[path = "support/browser_chart_fixture.rs"]
pub mod fixture;
use fixture::imported_chart;
use umya_spreadsheet::{
    Chart,
    ChartType,
    drawing::{
        charts::{
            CategoryAxisData,
            DataLabels,
            DataPoint,
            StringLiteral,
            StringPoint,
            XmlUnsignedInt,
        },
        spreadsheet::MarkerType,
    },
};

fn chart() -> Chart {
    let mut chart = Chart::default();
    chart.new_chart(
        &ChartType::LineChart,
        MarkerType::default(),
        MarkerType::default(),
        vec!["Data!$B$2:$B$5"],
    );
    chart
}

fn bytes(chart: &Chart) -> (Vec<u8>, Vec<u8>) {
    (
        bincode::serialize(chart).unwrap(),
        serde_json::to_vec(chart).unwrap(),
    )
}

#[test]
fn browser_projection_removes_hidden_collections_without_changing_editable_series() {
    let expected = chart();
    let mut actual = expected.clone();
    let series = &mut actual.area_chart_series_list_mut().area_chart_series_mut()[0];
    let mut point = DataPoint::default();
    point.index_mut().set_val(9);
    series.data_points_mut().extend([point.clone(), point]);
    let mut labels = DataLabels::default();
    labels.labels_mut().resize(2, Default::default());
    series.set_data_labels(labels);
    let mut expected = expected;
    expected
        .area_chart_series_list_mut()
        .area_chart_series_mut()[0]
        .set_data_labels(DataLabels::default());
    actual.strip_browser_projection();
    assert_eq!(bytes(&actual), bytes(&expected));
    let once = bytes(&actual);
    actual.strip_browser_projection();
    assert_eq!(bytes(&actual), once);
}

#[test]
fn browser_bytes_do_not_depend_on_hidden_point_and_label_counts() {
    let mut expected = chart();
    expected
        .area_chart_series_list_mut()
        .area_chart_series_mut()[0]
        .set_data_labels(DataLabels::default());
    let expected = bytes(&expected);
    for count in [10, 10_000, 100_000] {
        let mut actual = chart();
        let series = &mut actual.area_chart_series_list_mut().area_chart_series_mut()[0];
        series.data_points_mut().resize(count, Default::default());
        let mut labels = DataLabels::default();
        labels.labels_mut().resize(count, Default::default());
        series.set_data_labels(labels);
        actual.strip_browser_projection();
        assert_eq!(bytes(&actual), expected, "hidden count {count}");
    }
}

#[test]
fn browser_projection_keeps_inline_sparse_repeated_string_data_and_count_presence() {
    let mut actual = chart();
    let mut literal = StringLiteral::default();
    let mut count = XmlUnsignedInt::default();
    count.set_val(20);
    literal.set_point_count(Some(count));
    for (index, text) in [(9, "first"), (9, "second"), (1, "last")] {
        let mut point = StringPoint::default();
        let mut idx = XmlUnsignedInt::default();
        idx.set_val(index);
        point.set_index(idx);
        point.numeric_value_mut().set_text(text);
        literal.add_string_point_list(point);
    }
    let mut category = CategoryAxisData::default();
    category.set_string_literal(literal);
    actual.area_chart_series_list_mut().area_chart_series_mut()[0].set_category_axis_data(category);
    let before = bytes(&actual);
    actual.strip_browser_projection();
    assert_eq!(bytes(&actual), before);
}

#[test]
fn browser_projection_of_chart_without_physical_frame_is_a_no_op() {
    let mut actual = Chart::default();
    let before = bytes(&actual);
    actual.strip_browser_projection();
    assert_eq!(bytes(&actual), before);
}

#[test]
fn server_parts_restore_checks_physical_identity_without_positional_fallback() {
    let mut previous = chart();
    previous
        .area_chart_series_list_mut()
        .area_chart_series_mut()[0]
        .data_points_mut()
        .push(DataPoint::default());
    let mut current = previous.clone();
    current.strip_browser_projection();
    // The constructor's absent ID is not an explicitly supplied zero.
    current
        .two_cell_anchor_mut()
        .graphic_frame_mut()
        .unwrap()
        .non_visual_graphic_frame_properties_mut()
        .non_visual_drawing_properties_mut()
        .set_id(0);
    let report = current.restore_server_parts(&previous);
    assert_eq!(report.incompatible_owners, 1);
    assert_eq!(report.matched_series, 0);
    assert!(!report.diagnostics.is_empty());
    assert!(
        current.area_chart_series_list_mut().area_chart_series_mut()[0]
            .data_points()
            .is_empty()
    );
}

#[test]
fn server_parts_restore_allows_current_anchor_geometry_changes() {
    let mut previous = chart();
    previous
        .area_chart_series_list_mut()
        .area_chart_series_mut()[0]
        .data_points_mut()
        .push(DataPoint::default());
    let mut current = previous.clone();
    current.strip_browser_projection();
    current
        .two_cell_anchor_mut()
        .from_marker_mut()
        .set_coordinate("Z9");
    let marker = bincode::serialize(current.two_cell_anchor().from_marker()).unwrap();
    let report = current.restore_server_parts(&previous);
    assert_eq!(report.matched_series, 1);
    assert_eq!(report.restored_points, 1);
    assert_eq!(
        bincode::serialize(current.two_cell_anchor().from_marker()).unwrap(),
        marker
    );
}

fn ordered_server_series_chart() -> Chart {
    imported_chart(concat!(
        "<c:chartSpace xmlns:c=\"http://schemas.openxmlformats.org/drawingml/2006/chart\" \
         xmlns:u=\"urn:future\"><c:chart><c:plotArea><c:lineChart>",
        "<c:grouping val=\"standard\"/><c:ser><c:idx val=\"0\"/><c:order val=\"0\"/>",
        "<c:dPt><c:idx val=\"9\"/><c:marker><c:symbol val=\"circle\"/></c:marker></c:dPt>",
        "<c:dPt><c:idx val=\"9\"/><c:marker><c:symbol val=\"square\"/></c:marker></c:dPt>",
        "<c:dPt><c:idx val=\"2\"/><c:marker><c:symbol val=\"triangle\"/></c:marker></c:dPt>",
        "<c:trendline><c:name>first</c:name><c:trendlineType val=\"linear\"/></c:trendline>",
        "<c:trendline><c:name>second</c:name><c:trendlineType val=\"poly\"/><c:order \
         val=\"2\"/></c:trendline>",
        "<c:errBars><c:errBarType val=\"both\"/><c:errValType val=\"cust\"/>",
        "<c:plus><c:numRef><c:f>Errors!$A$1:$A$3</c:f></c:numRef></c:plus>",
        "<c:minus><c:numRef><c:f>Errors!$B$1:$B$3</c:f></c:numRef></c:minus></c:errBars>",
        "<c:val><c:numRef><c:f>Data!$B$1:$B$3</c:f></c:numRef></c:val>",
        "<c:extLst><c:ext uri=\"urn:future\"><u:payload>stored</u:payload></c:ext></c:extLst>",
        "</c:ser></c:lineChart></c:plotArea></c:chart></c:chartSpace>"
    ))
}

fn server_series_parts_bytes(chart: &Chart) -> (Vec<u8>, Vec<u8>) {
    let series = &chart
        .chart_space()
        .chart()
        .plot_area()
        .line_chart()
        .unwrap()
        .area_chart_series_list()
        .area_chart_series()[0];
    (
        bincode::serialize(&series.parts()).unwrap(),
        serde_json::to_vec(&series.parts()).unwrap(),
    )
}

#[test]
fn matched_empty_incoming_parts_cannot_suppress_stored_ordered_series_collections() {
    let previous = ordered_server_series_chart();
    let expected = server_series_parts_bytes(&previous);
    let mut current = previous.clone();
    current.strip_browser_projection();
    let series = &mut current.area_chart_series_list_mut().area_chart_series_mut()[0];
    series.parts_mut();
    series
        .values_mut()
        .unwrap()
        .number_reference_mut()
        .formula_mut()
        .set_address_str("Changed!$C$4:$C$6");
    let report = current.restore_server_parts(&previous);
    assert_eq!(server_series_parts_bytes(&current), expected);
    assert_eq!(report.matched_series, 1);
    assert_eq!(report.restored_points, 3);
    assert_eq!(
        current.area_chart_series_list_mut().area_chart_series_mut()[0]
            .values()
            .unwrap()
            .number_reference()
            .formula()
            .address_str(),
        "Changed!$C$4:$C$6"
    );
}

#[test]
fn populated_untrusted_incoming_parts_cannot_replace_authoritative_series_collections() {
    let previous = ordered_server_series_chart();
    let expected = server_series_parts_bytes(&previous);
    let mut current = previous.clone();
    current.strip_browser_projection();
    let series = &mut current.area_chart_series_list_mut().area_chart_series_mut()[0];
    series.data_points_mut().resize(7, Default::default());
    series.trendlines_mut().push(Default::default());
    series.error_bars_mut().push(Default::default());
    let mut extensions = previous
        .chart_space()
        .chart()
        .plot_area()
        .line_chart()
        .unwrap()
        .area_chart_series_list()
        .area_chart_series()[0]
        .extensions()
        .unwrap()
        .clone();
    extensions.entries[0].uri = Some("urn:untrusted".into());
    series.set_extensions(Some(extensions));
    let report = current.restore_server_parts(&previous);
    assert_eq!(server_series_parts_bytes(&current), expected);
    assert_eq!(report.matched_series, 1);
    assert_eq!(report.restored_points, 3);
}

#[test]
fn supplied_untrusted_parts_cannot_resurrect_collections_removed_from_authoritative_basis() {
    let history_before = ordered_server_series_chart();
    let mut authoritative = history_before.clone();
    let series = &mut authoritative
        .area_chart_series_list_mut()
        .area_chart_series_mut()[0];
    series.data_points_mut().clear();
    series.trendlines_mut().clear();
    series.error_bars_mut().clear();
    series.set_extensions(None);
    let expected = server_series_parts_bytes(&authoritative);
    let mut current = history_before.clone();
    let report = current.restore_server_parts(&authoritative);
    assert_eq!(server_series_parts_bytes(&current), expected);
    assert_eq!(report.matched_series, 1);
    assert_eq!(report.restored_points, 0);
    assert_ne!(server_series_parts_bytes(&history_before), expected);
}

#[test]
fn absent_authoritative_series_parts_do_not_admit_new_incoming_hidden_content() {
    let previous = chart();
    let expected = server_series_parts_bytes(&previous);
    let mut current = previous.clone();
    current.area_chart_series_list_mut().area_chart_series_mut()[0]
        .data_points_mut()
        .resize(3, Default::default());
    let report = current.restore_server_parts(&previous);
    assert_eq!(server_series_parts_bytes(&current), expected);
    assert_eq!(report.matched_series, 1);
    assert_eq!(report.restored_points, 0);
}

#[test]
fn compatible_display_unit_replacement_recovers_the_omitted_server_label_and_extensions() {
    let xml = fixture::all_owner_xml();
    let previous = fixture::imported_chart(&xml);
    let label =
        "<c:dispUnitsLbl><c:tx><c:strRef><c:f>Data!$C$1</c:f></c:strRef></c:tx></c:dispUnitsLbl>";
    let supplied = fixture::imported_chart(&xml.replace(label, ""));
    let units = supplied.chart_space().chart().plot_area().value_axis()[0]
        .display_units()
        .unwrap()
        .clone();
    let expected = bincode::serialize(
        previous.chart_space().chart().plot_area().value_axis()[0]
            .display_units()
            .unwrap(),
    )
    .unwrap();
    let mut current = previous.clone();
    current.strip_browser_projection();
    current.plot_area_mut().value_axis_mut()[0].set_display_units(units);
    current.restore_server_parts(&previous);
    let units = current.chart_space().chart().plot_area().value_axis()[0]
        .display_units()
        .unwrap();
    assert!(units.label().is_some());
    assert_eq!(
        units.effective_builtin(),
        Some(umya_spreadsheet::drawing::charts::BuiltInUnit::Thousands)
    );
    assert_eq!(bincode::serialize(units).unwrap(), expected);
}

#[test]
fn compatible_display_unit_restore_respects_label_removal_in_the_authoritative_basis() {
    let xml = fixture::all_owner_xml();
    let history_before = fixture::imported_chart(&xml);
    let label =
        "<c:dispUnitsLbl><c:tx><c:strRef><c:f>Data!$C$1</c:f></c:strRef></c:tx></c:dispUnitsLbl>";
    let authoritative = fixture::imported_chart(&xml.replace(label, ""));
    let units = authoritative.chart_space().chart().plot_area().value_axis()[0]
        .display_units()
        .unwrap()
        .clone();
    let mut current = history_before.clone();
    current.strip_browser_projection();
    current.plot_area_mut().value_axis_mut()[0].set_display_units(units);
    current.restore_server_parts(&authoritative);
    let units = current.chart_space().chart().plot_area().value_axis()[0]
        .display_units()
        .unwrap();
    assert!(units.label().is_none());
    assert!(
        history_before
            .chart_space()
            .chart()
            .plot_area()
            .value_axis()[0]
            .display_units()
            .unwrap()
            .label()
            .is_some()
    );
}

#[test]
fn compatible_custom_display_unit_restore_preserves_current_numeric_spelling() {
    let xml = fixture::all_owner_xml().replace(
        "<c:builtInUnit val=\"thousands\"/>",
        "<c:custUnit val=\"1.0\"/>",
    );
    let previous = fixture::imported_chart(&xml);
    let label =
        "<c:dispUnitsLbl><c:tx><c:strRef><c:f>Data!$C$1</c:f></c:strRef></c:tx></c:dispUnitsLbl>";
    let supplied = fixture::imported_chart(
        &xml.replace("<c:custUnit val=\"1.0\"/>", "<c:custUnit val=\"1\"/>")
            .replace(label, ""),
    );
    let units = supplied.chart_space().chart().plot_area().value_axis()[0]
        .display_units()
        .unwrap()
        .clone();
    let mut current = previous.clone();
    current.strip_browser_projection();
    current.plot_area_mut().value_axis_mut()[0].set_display_units(units);
    current.restore_server_parts(&previous);
    let units = current.chart_space().chart().plot_area().value_axis()[0]
        .display_units()
        .unwrap();
    assert!(units.label().is_some());
    match units.unit() {
        umya_spreadsheet::drawing::charts::DisplayUnit::Custom(value) => {
            assert_eq!(value.attribute(), Some("1"));
        }
        other => panic!("custom unit branch replaced: {other:?}"),
    }
}

#[test]
fn compatible_default_display_unit_restores_label_without_fabricating_unit_attribute() {
    let xml = fixture::all_owner_xml();
    let previous = fixture::imported_chart(&xml);
    let label =
        "<c:dispUnitsLbl><c:tx><c:strRef><c:f>Data!$C$1</c:f></c:strRef></c:tx></c:dispUnitsLbl>";
    let supplied = fixture::imported_chart(
        &xml.replace("<c:builtInUnit val=\"thousands\"/>", "<c:builtInUnit/>")
            .replace(label, ""),
    );
    let units = supplied.chart_space().chart().plot_area().value_axis()[0]
        .display_units()
        .unwrap()
        .clone();
    let mut current = previous.clone();
    current.strip_browser_projection();
    current.plot_area_mut().value_axis_mut()[0].set_display_units(units);
    current.restore_server_parts(&previous);
    let units = current.chart_space().chart().plot_area().value_axis()[0]
        .display_units()
        .unwrap();
    assert!(units.label().is_some());
    match units.unit() {
        umya_spreadsheet::drawing::charts::DisplayUnit::BuiltIn(value) => {
            assert!(value.attribute().is_none());
        }
        other => panic!("built-in unit branch replaced: {other:?}"),
    }
}

fn assert_export_slots_empty(value: &serde_json::Value, path: &str) {
    match value {
        serde_json::Value::Object(values) => {
            for (key, value) in values {
                let path = format!("{path}/{key}");
                if key == "parts" || key == "extensions" || key == "source" {
                    assert!(value.is_null(), "export slot survived at {path}: {value}");
                }
                assert_export_slots_empty(value, &path);
            }
        }
        serde_json::Value::Array(values) => {
            for (index, value) in values.iter().enumerate() {
                assert_export_slots_empty(value, &format!("{path}/{index}"));
            }
        }
        _ => {}
    }
}

#[test]
fn browser_projection_reaches_titles_axis_legend_and_all_detachable_owner_slots() {
    // Hand-authored XML is independent of the candidate's projection. This
    // fixture deliberately uses no scalar/literal supplements: every parts
    // slot present here is detachable export metadata.
    let xml = fixture::all_owner_xml();
    let previous = imported_chart(&xml);
    let plot = previous.chart_space().chart().plot_area();
    assert_eq!(plot.chart_groups().count(), 2);
    assert!(previous.chart_space().chart().pivot_formats().is_some());
    assert!(plot.value_axis()[0].display_units().is_some());
    let mut current = previous.clone();
    current.strip_browser_projection();
    assert_export_slots_empty(&serde_json::to_value(&current).unwrap(), "chart");
    let plot = current.chart_space().chart().plot_area();
    assert_eq!(plot.chart_groups().count(), 2);
    assert_eq!(
        plot.value_axis()[0]
            .numbering_format()
            .source_linked_value(),
        Some(true)
    );
    let once = bytes(&current);
    current.strip_browser_projection();
    assert_eq!(bytes(&current), once);
}

#[test]
fn browser_projection_detaches_supplements_for_each_supported_group_kind() {
    for group in [
        "areaChart",
        "area3DChart",
        "barChart",
        "bar3DChart",
        "bubbleChart",
        "doughnutChart",
        "lineChart",
        "line3DChart",
        "ofPieChart",
        "pieChart",
        "pie3DChart",
        "radarChart",
        "scatterChart",
    ] {
        let ext = "<c:extLst><c:ext uri=\"urn:future\"><u:payload/></c:ext></c:extLst>";
        let xml = format!(
            "<c:chartSpace xmlns:c=\"http://schemas.openxmlformats.org/drawingml/2006/chart\" \
             xmlns:u=\"urn:future\"><c:chart><c:plotArea><c:{group}><c:ser><c:idx \
             val=\"0\"/><c:order val=\"0\"/><c:dPt><c:idx \
             val=\"9\"/></c:dPt>{ext}</c:ser>{ext}</c:{group}></c:plotArea></c:chart></c:\
             chartSpace>"
        );
        let mut current = imported_chart(&xml);
        let plot = current.chart_space().chart().plot_area();
        let groups: Vec<_> = plot.chart_groups().collect();
        assert_eq!(groups.len(), 1, "{group}");
        assert_eq!(
            groups[0].series().area_chart_series()[0]
                .data_points()
                .len(),
            1,
            "{group}"
        );
        current.strip_browser_projection();
        assert_export_slots_empty(&serde_json::to_value(&current).unwrap(), group);
        assert_eq!(
            current
                .chart_space()
                .chart()
                .plot_area()
                .chart_groups()
                .count(),
            1
        );
    }
}

fn null_export_slots(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(values) => {
            for (key, value) in values {
                if matches!(key.as_str(), "parts" | "extensions" | "source") {
                    *value = serde_json::Value::Null;
                } else {
                    null_export_slots(value);
                }
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                null_export_slots(value);
            }
        }
        _ => {}
    }
}

#[test]
fn browser_projection_preserves_full_content_outside_selected_export_slots() {
    let complete = imported_chart(&fixture::all_owner_xml());
    let mut browser = complete.clone();
    browser.strip_browser_projection();
    let mut expected = serde_json::to_value(&complete).unwrap();
    let actual = serde_json::to_value(&browser).unwrap();
    null_export_slots(&mut expected);
    for path in [
        concat!(
            "/two_cell_anchor/graphic_frame/graphic/graphic_data/chart_space/chart/plot_area/",
            "date_axis/0/numbering_format/source_linked/value"
        ),
        concat!(
            "/two_cell_anchor/graphic_frame/graphic/graphic_data/chart_space/chart/plot_area/",
            "value_axis/0/numbering_format/source_linked/value"
        ),
    ] {
        assert_eq!(
            expected.pointer(path),
            Some(&serde_json::Value::Null),
            "{path}"
        );
        assert_eq!(
            actual.pointer(path),
            Some(&serde_json::json!(true)),
            "{path}"
        );
        *expected.pointer_mut(path).unwrap() = serde_json::json!(true);
    }
    assert_eq!(actual, expected);
}

#[test]
fn full_content_comparison_detects_same_length_unrelated_mutation() {
    let mut browser = imported_chart(&fixture::all_owner_xml());
    browser.strip_browser_projection();
    let expected = serde_json::to_value(&browser).unwrap();
    let mut changed = expected.clone();
    assert_eq!(
        changed.pointer("/default_language"),
        Some(&serde_json::json!("en-GB"))
    );
    *changed.pointer_mut("/default_language").unwrap() = serde_json::json!("fr-FR");
    assert_eq!(
        serde_json::to_vec(&changed).unwrap().len(),
        serde_json::to_vec(&expected).unwrap().len()
    );
    assert_ne!(changed, expected);
}
