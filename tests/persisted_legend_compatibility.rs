#![cfg(feature = "serde")]

use std::io::Cursor;

use umya_spreadsheet::{
    drawing::charts::Chart,
    reader,
};

fn historical_chart() -> serde_json::Value {
    let charts: serde_json::Value = serde_json::from_slice(include_bytes!(
        "fixtures/persisted_charts/7fd0288662ac/standard.json"
    ))
    .unwrap();
    charts[0]["two_cell_anchor"]["graphic_frame"]["graphic"]["graphic_data"]["chart_space"]["chart"]
        .clone()
}

#[test]
fn absent_legend_presence_matches_the_old_writer_export() {
    let book = reader::xlsx::read_reader(
        Cursor::new(include_bytes!(
            "fixtures/persisted_charts/7fd0288662ac/standard.xlsx"
        )),
        true,
    )
    .unwrap();
    let oracle = book.sheet(0).unwrap().chart_collection()[0]
        .chart_space()
        .chart();
    assert!(oracle.legend_present());
    let restored: Chart = serde_json::from_value(historical_chart()).unwrap();
    assert_eq!(restored.legend_present(), oracle.legend_present());
}

#[test]
fn explicit_legend_absence_stays_absent() {
    let mut value = historical_chart();
    value["legend_present"] = false.into();
    let chart: Chart = serde_json::from_value(value).unwrap();
    assert!(!chart.legend_present());
}

#[test]
fn chart_compatibility_rejects_unknown_and_duplicate_fields() {
    let mut value = serde_json::to_value(Chart::default()).unwrap();
    value["unrecognized"] = true.into();
    assert!(serde_json::from_value::<Chart>(value).is_err());
    let value = serde_json::to_string(&Chart::default()).unwrap();
    let duplicate = format!("{},\"title\":null}}", value.trim_end_matches('}'));
    assert!(serde_json::from_str::<Chart>(&duplicate).is_err());
}

#[test]
fn current_reader_legend_binary_bytes_are_preserved() {
    let book = reader::xlsx::read_reader(
        Cursor::new(include_bytes!(
            "fixtures/persisted_charts/7fd0288662ac/standard.xlsx"
        )),
        true,
    )
    .unwrap();
    let mut chart = book.sheet(0).unwrap().chart_collection()[0]
        .chart_space()
        .chart()
        .clone();
    for present in [true, false] {
        chart.set_legend_present(present);
        let bytes = bincode::serialize(&chart).unwrap();
        let restored: Chart = bincode::deserialize(&bytes).unwrap();
        assert_eq!(restored.legend_present(), present);
        assert_eq!(bincode::serialize(&restored).unwrap(), bytes);
    }
}

#[test]
fn legacy_chart_field_sequence_is_rejected() {
    let chart = historical_chart();
    let legacy = serde_json::json!([
        chart["title"],
        chart["auto_title_deleted"],
        chart["view_3d"],
        chart["floor"],
        chart["side_wall"],
        chart["back_wall"],
        chart["plot_area"],
        chart["legend"],
        chart["plot_visible_only"],
        chart["display_blanks_as"],
        chart["show_data_labels_over_maximum"]
    ]);
    assert!(serde_json::from_value::<Chart>(legacy).is_err());
}
