#![cfg(feature = "serde")]

use umya_spreadsheet::{
    Chart,
    ChartType,
    drawing::spreadsheet::MarkerType,
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

#[test]
fn adding_a_title_is_not_an_incompatible_owner() {
    let previous = chart();
    let mut current = previous.clone();
    current.set_title("New title");
    let report = current.restore_server_parts(&previous);
    assert_eq!(report.incompatible_owners, 0, "{report:?}");
    assert_eq!(report.added_owners, 1);
    assert_eq!(report.removed_owners, 0);
    assert!(report.diagnostics.is_empty(), "{report:?}");
    assert!(current.chart_space().chart().title().is_some());
    assert!(previous.chart_space().chart().title().is_none());
}

#[test]
fn removing_a_title_is_not_an_incompatible_owner() {
    let current = chart();
    let mut previous = current.clone();
    previous.set_title("Old title");
    let mut replacement = current;
    let report = replacement.restore_server_parts(&previous);
    assert_eq!(report.incompatible_owners, 0, "{report:?}");
    assert_eq!(report.added_owners, 0);
    assert_eq!(report.removed_owners, 1);
    assert!(report.diagnostics.is_empty(), "{report:?}");
    assert!(replacement.chart_space().chart().title().is_none());
    assert!(previous.chart_space().chart().title().is_some());
}
