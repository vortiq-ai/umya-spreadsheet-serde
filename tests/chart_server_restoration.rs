#![cfg(feature = "serde")]

use umya_spreadsheet::{
    Chart,
    ChartType,
    drawing::{
        charts::{
            DataLabels,
            DataPoint,
            Values,
        },
        spreadsheet::MarkerType,
    },
};

#[path = "support/browser_chart_fixture.rs"]
pub mod fixture;

fn chart(formulas: Vec<&str>) -> Chart {
    let mut result = Chart::default();
    result.new_chart(
        &ChartType::LineChart,
        MarkerType::default(),
        MarkerType::default(),
        formulas,
    );
    result
}

fn install_points(chart: &mut Chart, series: usize, indices: &[u32]) {
    let owner = &mut chart.area_chart_series_list_mut().area_chart_series_mut()[series];
    for index in indices {
        let mut point = DataPoint::default();
        point.index_mut().set_val(*index);
        owner.data_points_mut().push(point);
    }
}

fn point_indices(chart: &mut Chart, series: usize) -> Vec<u32> {
    chart.area_chart_series_list_mut().area_chart_series_mut()[series]
        .data_points()
        .iter()
        .map(|point| point.index().val().unwrap())
        .collect()
}

#[test]
fn restored_series_follow_identity_through_reordering_removal_and_creation() {
    let mut previous = chart(vec!["Data!$B$2:$B$5", "Data!$C$2:$C$5", "Data!$D$2:$D$5"]);
    install_points(&mut previous, 0, &[9, 9, 1]);
    install_points(&mut previous, 1, &[7]);
    install_points(&mut previous, 2, &[3]);
    let before = bincode::serialize(&previous).unwrap();
    let mut current = previous.clone();
    current.strip_browser_projection();
    let mut series = current
        .area_chart_series_list_mut()
        .area_chart_series()
        .to_vec();
    series.swap(0, 2);
    series.remove(1);
    let mut new = umya_spreadsheet::drawing::charts::AreaChartSeries::default();
    new.index_mut().set_val(44);
    let mut values = Values::default();
    values
        .number_reference_mut()
        .formula_mut()
        .set_address_str("Data!$E$2:$E$5");
    new.set_values(values);
    series.push(new);
    current
        .area_chart_series_list_mut()
        .set_area_chart_series(series);
    let report = current.restore_server_parts(&previous);
    assert_eq!(report.matched_series, 2);
    assert_eq!(report.unmatched_series, 1);
    assert_eq!(report.unmatched_previous_series, 1);
    assert_eq!(point_indices(&mut current, 0), [3]);
    assert_eq!(point_indices(&mut current, 1), [9, 9, 1]);
    assert!(point_indices(&mut current, 2).is_empty());
    assert_eq!(report.restored_points, 4);
    assert_eq!(
        bincode::serialize(&previous).unwrap(),
        before,
        "history before-state"
    );
}

#[test]
fn agent_rebuilt_all_zero_indices_use_unique_values_formula_without_position() {
    let mut previous = chart(vec!["Data!$B$2:$B$5", "Data!$C$2:$C$5"]);
    install_points(&mut previous, 0, &[9, 9]);
    install_points(&mut previous, 1, &[2]);
    // A native authoritative basis can contain gRPC's all-zero indices.
    for series in previous
        .area_chart_series_list_mut()
        .area_chart_series_mut()
    {
        series.index_mut().set_val(0);
    }
    let mut current = chart(vec!["Data!$C$2:$C$5", "Data!$B$2:$B$5"]);
    for series in current.area_chart_series_list_mut().area_chart_series_mut() {
        series.index_mut().set_val(0);
    }
    let report = current.restore_server_parts(&previous);
    assert_eq!(report.formula_matched_series, 2);
    assert_eq!(report.index_matched_series, 0);
    assert_eq!(point_indices(&mut current, 0), [2]);
    assert_eq!(point_indices(&mut current, 1), [9, 9]);
}

#[test]
fn ambiguous_owners_are_reported_without_copying_a_neighbor_parts_collection() {
    let mut previous = chart(vec!["Data!$B$2:$B$5", "Data!$B$2:$B$5"]);
    for series in previous
        .area_chart_series_list_mut()
        .area_chart_series_mut()
    {
        series.index_mut().set_val(0);
    }
    install_points(&mut previous, 0, &[9]);
    install_points(&mut previous, 1, &[2]);
    let mut current = previous.clone();
    current.strip_browser_projection();
    let report = current.restore_server_parts(&previous);
    assert_eq!(report.ambiguous_series, 2);
    assert_eq!(report.matched_series, 0);
    assert!(!report.diagnostics.is_empty());
    assert!(point_indices(&mut current, 0).is_empty());
    assert!(point_indices(&mut current, 1).is_empty());
}

#[test]
fn hidden_label_parent_is_recreated_without_losing_duplicate_point_labels() {
    let mut previous = chart(vec!["Data!$B$2:$B$5"]);
    let mut labels = DataLabels::default();
    for index in [9, 9, 1] {
        let mut label = umya_spreadsheet::drawing::charts::DataLabel::default();
        label.index_mut().set_val(index);
        labels.labels_mut().push(label);
    }
    previous
        .area_chart_series_list_mut()
        .area_chart_series_mut()[0]
        .set_data_labels(labels);
    let mut current = chart(vec!["Data!$B$2:$B$5"]);
    assert!(
        current.area_chart_series_list_mut().area_chart_series_mut()[0]
            .data_labels()
            .is_none()
    );
    let report = current.restore_server_parts(&previous);
    let labels = current.area_chart_series_list_mut().area_chart_series_mut()[0]
        .data_labels()
        .unwrap()
        .labels();
    assert_eq!(
        labels
            .iter()
            .map(|label| label.index().val())
            .collect::<Vec<_>>(),
        [9, 9, 1]
    );
    assert_eq!(report.restored_labels, 3);
    assert!(report.recreated_owners >= 1);
}

#[test]
fn explicit_removal_uses_changed_authoritative_basis_not_history_before_state() {
    let mut history_before = chart(vec!["Data!$B$2:$B$5"]);
    install_points(&mut history_before, 0, &[9]);
    let before = bincode::serialize(&history_before).unwrap();
    let mut basis = history_before.clone();
    basis.area_chart_series_list_mut().area_chart_series_mut()[0]
        .data_points_mut()
        .clear();
    let mut current = basis.clone();
    current.strip_browser_projection();
    current.restore_server_parts(&basis);
    assert!(point_indices(&mut current, 0).is_empty());
    assert_eq!(bincode::serialize(&history_before).unwrap(), before);
}

#[test]
fn every_imported_owner_roundtrips_projection_and_persistence_without_state_changes() {
    let previous = fixture::imported_chart(&fixture::all_owner_xml());
    let before_json = serde_json::to_value(&previous).unwrap();
    let before_binary = bincode::serialize(&previous).unwrap();
    let mut current = previous.clone();
    current.strip_browser_projection();
    // Cross the real JSON replacement boundary, not just a native clone.
    current = serde_json::from_slice(&serde_json::to_vec(&current).unwrap()).unwrap();
    let report = current.restore_server_parts(&previous);
    assert_eq!(report.matched_series, 2);
    assert_eq!(report.ambiguous_series, 0);
    assert!(report.restored_owners > 20, "{report:?}");
    assert_eq!(serde_json::to_value(&current).unwrap(), before_json);
    assert_eq!(bincode::serialize(&current).unwrap(), before_binary);
    let persisted: Chart = bincode::deserialize(&bincode::serialize(&current).unwrap()).unwrap();
    assert_eq!(serde_json::to_value(persisted).unwrap(), before_json);
    assert_eq!(bincode::serialize(&previous).unwrap(), before_binary);
}

#[test]
fn changed_title_text_does_not_recreate_fields_from_the_previous_title() {
    let previous = fixture::imported_chart(&fixture::all_owner_xml());
    let mut current = previous.clone();
    current.strip_browser_projection();
    current.set_title("a new title");
    let report = current.restore_server_parts(&previous);
    assert!(report.incompatible_owners >= 1);
    let mut book = umya_spreadsheet::new_file();
    book.sheet_mut(0).unwrap().add_chart(current);
    let xml = fixture::chart_xml(&book);
    let title = xml
        .split_once("<c:title>")
        .unwrap()
        .1
        .split_once("</c:title>")
        .unwrap()
        .0;
    assert!(title.contains("a new title"));
    assert!(!title.contains("a:fld"), "{title}");
    assert!(!title.contains("kept text"));
}

#[test]
fn axis_reordering_restores_only_the_matching_explicit_axis_kind_and_id() {
    let xml = fixture::all_owner_xml();
    let extra = "<c:valAx><c:axId val=\"21\"/><c:dispUnits><c:builtInUnit \
                 val=\"millions\"/></c:dispUnits></c:valAx>";
    let previous =
        fixture::imported_chart(&xml.replace("</c:plotArea>", &format!("{extra}</c:plotArea>")));
    let mut current = previous.clone();
    current.strip_browser_projection();
    current.plot_area_mut().value_axis_mut().swap(0, 1);
    current.restore_server_parts(&previous);
    let axes = current.chart_space().chart().plot_area().value_axis();
    assert_eq!(axes[0].axis_id().val(), 21);
    assert_eq!(axes[1].axis_id().val(), 20);
    // Independently authored unit and label ownership: only axis20 has a
    // reference label.
    assert!(axes[0].display_units().unwrap().label().is_none());
    assert!(axes[1].display_units().unwrap().label().is_some());
}

#[test]
fn current_marker_fill_and_manual_layout_values_win_over_old_supplements() {
    let xml = fixture::all_owner_xml();
    let previous = fixture::imported_chart(&xml);
    let edited = xml
        .replace("123456", "ABCDEF")
        .replace("val=\"circle\"", "val=\"square\"")
        .replace("<c:manualLayout>", "<c:manualLayout><c:x val=\"0.75\"/>");
    let mut current = fixture::imported_chart(&edited);
    current.strip_browser_projection();
    current.restore_server_parts(&previous);
    let mut book = umya_spreadsheet::new_file();
    book.sheet_mut(0).unwrap().add_chart(current);
    let exported = fixture::chart_xml(&book);
    let editable_series = exported
        .split_once("<c:ser>")
        .unwrap()
        .1
        .split_once("<c:dPt>")
        .unwrap()
        .0;
    assert!(editable_series.contains("val=\"ABCDEF\""));
    assert!(!editable_series.contains("val=\"123456\""));
    // Point formatting is hidden server-owned content, unlike series/marker
    // formatting: importing another browser replacement cannot change it.
    let point = exported
        .split_once("<c:dPt>")
        .unwrap()
        .1
        .split_once("</c:dPt>")
        .unwrap()
        .0;
    assert!(point.contains("val=\"123456\""));
    assert!(exported.contains("<c:symbol val=\"square\"/>"));
    assert!(exported.contains("<c:x val=\"0.75\"/>"));
    assert!(
        exported.contains("urn:future"),
        "compatible extension owners survive style edits"
    );
}

#[test]
fn explicit_display_unit_replacement_does_not_recreate_the_previous_label() {
    let xml = fixture::all_owner_xml();
    let previous = fixture::imported_chart(&xml);
    let label =
        "<c:dispUnitsLbl><c:tx><c:strRef><c:f>Data!$C$1</c:f></c:strRef></c:tx></c:dispUnitsLbl>";
    let replacement =
        fixture::imported_chart(&xml.replace("thousands", "millions").replace(label, ""));
    let units = replacement.chart_space().chart().plot_area().value_axis()[0]
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
    assert!(units.label().is_none());
    assert_eq!(
        units.effective_builtin(),
        Some(umya_spreadsheet::drawing::charts::BuiltInUnit::Millions)
    );
}

#[test]
fn reordered_same_family_groups_restore_extensions_by_membership_not_position() {
    let xml = fixture::all_owner_xml();
    let first_start = xml.find("<c:lineChart>").unwrap();
    let first_end =
        first_start + xml[first_start..].find("</c:lineChart>").unwrap() + "</c:lineChart>".len();
    let second_start = first_end + xml[first_end..].find("<c:lineChart>").unwrap();
    let second_end =
        second_start + xml[second_start..].find("</c:lineChart>").unwrap() + "</c:lineChart>".len();
    let alpha = xml[first_start..first_end].replace("hidden", "owner-alpha");
    let beta = xml[second_start..second_end].replace("hidden", "owner-beta");
    let original = format!("{}{alpha}{beta}{}", &xml[..first_start], &xml[second_end..]);
    let reordered = format!("{}{beta}{alpha}{}", &xml[..first_start], &xml[second_end..]);
    let previous = fixture::imported_chart(&original);
    let mut current = fixture::imported_chart(&reordered);
    current.strip_browser_projection();
    let report = current.restore_server_parts(&previous);
    assert_eq!(report.matched_series, 2);
    let mut book = umya_spreadsheet::new_file();
    book.sheet_mut(0).unwrap().add_chart(current);
    let exported = fixture::chart_xml(&book);
    let groups: Vec<_> = exported
        .split("<c:lineChart>")
        .skip(1)
        .map(|group| group.split_once("</c:lineChart>").unwrap().0)
        .collect();
    assert!(groups[0].contains("owner-beta"));
    assert!(!groups[0].contains("owner-alpha"));
    assert!(groups[1].contains("owner-alpha"));
    assert!(!groups[1].contains("owner-beta"));
}

fn empty_group_chart(group_count: usize) -> Chart {
    let group = "<c:lineChart><c:grouping val=\"standard\"/><c:dLbls><c:numFmt \
                 formatCode=\"0.00\"/><c:showVal/></c:dLbls><c:extLst><c:ext \
                 uri=\"urn:empty-owner\"><u:payload>retained empty \
                 group</u:payload></c:ext></c:extLst></c:lineChart>";
    fixture::imported_chart(&format!(
        "<c:chartSpace xmlns:c=\"http://schemas.openxmlformats.org/drawingml/2006/chart\" \
         xmlns:u=\"urn:future\"><c:chart><c:plotArea>{}</c:plotArea></c:chart></c:chartSpace>",
        group.repeat(group_count)
    ))
}

#[test]
fn unique_empty_group_preserves_its_extension_and_label_owner_without_position_matching() {
    let previous = empty_group_chart(1);
    let before = bincode::serialize(&previous).unwrap();
    let mut current = previous.clone();
    current.strip_browser_projection();
    let report = current.restore_server_parts(&previous);
    assert_eq!(report.incompatible_owners, 0, "{report:?}");
    assert_eq!(bincode::serialize(&current).unwrap(), before);
}

#[test]
fn indistinguishable_empty_groups_are_reported_and_never_paired_by_position() {
    let previous = empty_group_chart(2);
    let mut current = previous.clone();
    current.strip_browser_projection();
    let report = current.restore_server_parts(&previous);
    assert!(report.incompatible_owners >= 2);
    let json = serde_json::to_string(&current).unwrap();
    assert!(!json.contains("retained empty group"));
}

#[test]
fn replacing_the_title_text_branch_reports_incompatible_owner_without_restoring_fields() {
    let xml = fixture::all_owner_xml();
    let start = xml.find("<c:title>").unwrap();
    let end = start + xml[start..].find("</c:title>").unwrap() + "</c:title>".len();
    let edited = format!(
        "{}<c:title><c:tx><c:v>literal replacement</c:v></c:tx></c:title>{}",
        &xml[..start],
        &xml[end..]
    );
    let previous = fixture::imported_chart(&xml);
    let mut current = fixture::imported_chart(&edited);
    current.strip_browser_projection();
    let report = current.restore_server_parts(&previous);
    assert!(report.incompatible_owners >= 1);
    assert!(
        report
            .diagnostics
            .iter()
            .any(|message| message.contains("text branch"))
    );
    let text = current
        .chart_space()
        .chart()
        .title()
        .unwrap()
        .chart_text()
        .unwrap();
    assert!(text.rich_text().is_none());
    assert_eq!(text.numeric_value().unwrap().text(), "literal replacement");
}

fn indexed_pivot(index: Option<u32>) -> umya_spreadsheet::drawing::charts::PivotFormat {
    let mut pivot = umya_spreadsheet::drawing::charts::PivotFormat::default();
    if let Some(value) = index {
        let mut index = umya_spreadsheet::drawing::charts::Index::default();
        index.set_val(value);
        pivot.set_index(index);
    }
    pivot
}

fn two_pivot_chart() -> Chart {
    let second = "<c:pivotFmt><c:idx val=\"8\"/><c:dLbl><c:idx \
                  val=\"2\"/><c:tx><c:strRef><c:f>Data!$E$1</c:f></c:strRef></c:tx></c:dLbl></c:\
                  pivotFmt>";
    fixture::imported_chart(
        &fixture::all_owner_xml().replace("</c:pivotFmts>", &format!("{second}</c:pivotFmts>")),
    )
}

#[test]
fn supplied_pivot_collections_restore_hidden_owners_by_unique_explicit_index() {
    let previous = two_pivot_chart();
    let mut current = previous.clone();
    current.strip_browser_projection();
    current
        .chart_space_mut()
        .chart_mut()
        .set_pivot_formats(vec![indexed_pivot(Some(8)), indexed_pivot(Some(7))]);
    current.restore_server_parts(&previous);
    let pivots = current.chart_space().chart().pivot_formats().unwrap();
    assert_eq!(
        pivots[0]
            .label()
            .unwrap()
            .text()
            .unwrap()
            .string_reference()
            .unwrap()
            .formula()
            .address_str(),
        "Data!$E$1"
    );
    assert_eq!(
        pivots[1]
            .label()
            .unwrap()
            .text()
            .unwrap()
            .string_reference()
            .unwrap()
            .formula()
            .address_str(),
        "Data!$D$1"
    );
}

#[test]
fn duplicate_or_missing_pivot_indices_do_not_receive_another_pivots_label() {
    let previous = two_pivot_chart();
    for supplied in [
        vec![indexed_pivot(Some(7)), indexed_pivot(Some(7))],
        vec![indexed_pivot(None)],
    ] {
        let supplied_count = supplied.len();
        let mut current = previous.clone();
        current.strip_browser_projection();
        current
            .chart_space_mut()
            .chart_mut()
            .set_pivot_formats(supplied);
        let report = current.restore_server_parts(&previous);
        assert!(report.incompatible_owners >= 1, "{report:?}");
        let pivots = current.chart_space().chart().pivot_formats().unwrap();
        assert!(
            pivots[..supplied_count]
                .iter()
                .all(|pivot| pivot.label().is_none())
        );
        // Missing unique stored owners are separately recreated, never used
        // as a positional source for one of the supplied ambiguous owners.
        for pivot in &pivots[supplied_count..] {
            let formula = pivot
                .label()
                .unwrap()
                .text()
                .unwrap()
                .string_reference()
                .unwrap()
                .formula()
                .address_str();
            match pivot.index().val() {
                7 => assert_eq!(formula, "Data!$D$1"),
                8 => assert_eq!(formula, "Data!$E$1"),
                other => panic!("unexpected recreated pivot {other}"),
            }
        }
    }
}

#[test]
fn absent_authoritative_point_labels_cannot_admit_hidden_client_labels() {
    let previous = chart(vec!["Data!$B$2:$B$5"]);
    let mut current = previous.clone();
    let mut labels = DataLabels::default();
    labels.show_value_mut().set_val(true);
    let mut label = umya_spreadsheet::drawing::charts::DataLabel::default();
    label.index_mut().set_val(9);
    labels.labels_mut().push(label);
    current.area_chart_series_list_mut().area_chart_series_mut()[0].set_data_labels(labels);
    current.restore_server_parts(&previous);
    let labels = current.area_chart_series_list_mut().area_chart_series_mut()[0]
        .data_labels()
        .unwrap();
    assert!(labels.labels().is_empty());
    assert!(
        labels.show_value().val(),
        "current editable flag must survive"
    );
}

#[test]
fn omitted_pivots_in_supplied_empty_or_partial_collections_are_recreated_from_basis() {
    let previous = two_pivot_chart();
    for supplied in [Vec::new(), vec![indexed_pivot(Some(8))]] {
        let mut current = previous.clone();
        current.strip_browser_projection();
        current
            .chart_space_mut()
            .chart_mut()
            .set_pivot_formats(supplied);
        let report = current.restore_server_parts(&previous);
        let pivots = current.chart_space().chart().pivot_formats().unwrap();
        let restored: std::collections::BTreeMap<_, _> = pivots
            .iter()
            .map(|pivot| {
                (
                    pivot.index().val(),
                    pivot
                        .label()
                        .unwrap()
                        .text()
                        .unwrap()
                        .string_reference()
                        .unwrap()
                        .formula()
                        .address_str(),
                )
            })
            .collect();
        assert_eq!(
            restored,
            std::collections::BTreeMap::from([
                (7, "Data!$D$1".to_owned()),
                (8, "Data!$E$1".to_owned())
            ])
        );
        assert!(report.recreated_owners >= 1);
    }
}

#[test]
fn preedited_pivot_removal_is_not_undone_by_recreating_missing_collection_owners() {
    let history_before = two_pivot_chart();
    let before = bincode::serialize(&history_before).unwrap();
    let mut basis = history_before.clone();
    let retained = basis.chart_space().chart().pivot_formats().unwrap()[1].clone();
    basis
        .chart_space_mut()
        .chart_mut()
        .set_pivot_formats(vec![retained]);
    let mut current = basis.clone();
    current.strip_browser_projection();
    current
        .chart_space_mut()
        .chart_mut()
        .set_pivot_formats(Vec::new());
    current.restore_server_parts(&basis);
    let pivots = current.chart_space().chart().pivot_formats().unwrap();
    assert_eq!(pivots.len(), 1);
    assert_eq!(pivots[0].index().val(), 8);
    assert_eq!(bincode::serialize(&history_before).unwrap(), before);
}

#[test]
fn empty_supplied_pivot_collection_preserves_ambiguous_and_invalid_stored_owners_exactly() {
    let second = "<c:pivotFmt><c:idx val=\"7\"/><c:dLbl><c:idx \
                  val=\"2\"/><c:tx><c:strRef><c:f>Data!$E$1</c:f></c:strRef></c:tx></c:dLbl></c:\
                  pivotFmt>";
    let duplicate =
        fixture::all_owner_xml().replace("</c:pivotFmts>", &format!("{second}</c:pivotFmts>"));
    let missing = duplicate.replacen("<c:pivotFmt><c:idx val=\"7\"/>", "<c:pivotFmt>", 1);
    for xml in [duplicate, missing] {
        let previous = fixture::imported_chart(&xml);
        let before = serde_json::to_value(previous.chart_space().chart()).unwrap()["parts"].clone();
        let binary =
            bincode::serialize(previous.chart_space().chart().pivot_formats().unwrap()).unwrap();
        let mut current = previous.clone();
        current.strip_browser_projection();
        current
            .chart_space_mut()
            .chart_mut()
            .set_pivot_formats(Vec::new());
        let report = current.restore_server_parts(&previous);
        assert_eq!(
            serde_json::to_value(current.chart_space().chart()).unwrap()["parts"],
            before
        );
        assert_eq!(
            bincode::serialize(current.chart_space().chart().pivot_formats().unwrap()).unwrap(),
            binary
        );
        assert_eq!(
            report.restored_labels, 4,
            "two ordinary point labels and two primary pivot labels"
        );
    }
}
