#![cfg(feature = "serde")]
//! A chart group whose series were all replaced (change source data) keeps its
//! own hidden parts when a single unclaimed group of the same kind and axes
//! exists. Excel (measured with `SetSourceData` to a new range on a bar chart
//! whose `c:barChart` carries an `c:extLst` and whose group `c:dLbls` carry
//! an extension) keeps the group `c:extLst` and the group labels.
use umya_spreadsheet::{
    Chart,
    drawing::charts::RestoreReport,
};

#[path = "support/chart_xml_fixture.rs"]
pub mod fixture;

const FLAGS: &str =
    "<c:showLegendKey val=\"0\"/><c:showVal val=\"1\"/><c:showCatName                      \
     val=\"0\"/><c:showSerName val=\"0\"/><c:showPercent                      \
     val=\"0\"/><c:showBubbleSize val=\"0\"/>";

fn ext(marker: &str) -> String {
    format!(
        "<c:extLst><c:ext uri=\"urn:hidden\"><h:p          \
         xmlns:h=\"urn:hidden\">{marker}</h:p></c:ext></c:extLst>"
    )
}

/// A clustered bar group on the axes `axes` whose series (index, column) read
/// `Sheet1!$<column>$2:$<column>$5`, with a group extension and group labels
/// that carry an extension, both marked with `tag`.
fn bar(axes: (u32, u32), series: &[(u32, &str)], tag: &str) -> String {
    let series: String = series
        .iter()
        .map(|(index, column)| {
            format!(
                "<c:ser><c:idx val=\"{index}\"/><c:order \
                 val=\"{index}\"/><c:cat><c:strRef><c:f>Sheet1!$A$2:$A$5</c:f></c:strRef></c:\
                 cat><c:val><c:numRef><c:f>Sheet1!${column}$2:${column}$5</c:f></c:numRef></c:\
                 val></c:ser>"
            )
        })
        .collect();
    format!(
        "<c:barChart><c:barDir val=\"col\"/><c:grouping val=\"clustered\"/><c:varyColors          \
         val=\"0\"/>{series}<c:dLbls>{FLAGS}{}</c:dLbls><c:gapWidth val=\"150\"/><c:axId          \
         val=\"{}\"/><c:axId val=\"{}\"/>{}</c:barChart>",
        ext(&format!("{tag}LABELEXT")),
        axes.0,
        axes.1,
        ext(&format!("{tag}EXT")),
    )
}

fn axes(ids: &[(u32, u32)]) -> String {
    ids.iter()
        .map(|(category, value)| {
            format!(
                "<c:catAx><c:axId val=\"{category}\"/><c:scaling><c:orientation \
                 val=\"minMax\"/></c:scaling><c:delete val=\"0\"/><c:axPos val=\"b\"/><c:crossAx \
                 val=\"{value}\"/></c:catAx><c:valAx><c:axId \
                 val=\"{value}\"/><c:scaling><c:orientation val=\"minMax\"/></c:scaling><c:delete \
                 val=\"0\"/><c:axPos val=\"l\"/><c:crossAx val=\"{category}\"/></c:valAx>"
            )
        })
        .collect()
}

fn document(groups: &str, axis_ids: &[(u32, u32)]) -> String {
    format!(
        "<c:chartSpace xmlns:c=\"http://schemas.openxmlformats.org/drawingml/2006/chart\" xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\"><c:chart><c:plotArea><c:layout/>{groups}{}</c:plotArea></c:chart></c:chartSpace>",
        axes(axis_ids)
    )
}

/// The client's copy: what the typed edit sends back (hidden parts stripped).
fn client(xml: &str) -> Chart {
    let mut chart = fixture::imported_chart(xml);
    chart.strip_export_parts();
    chart
}

/// Restores `client` against `basis` and returns the report and exported XML.
fn restore(mut client: Chart, basis: &Chart) -> (RestoreReport, String) {
    let report = client.restore_export_parts(basis).unwrap();
    let mut book = umya_spreadsheet::new_file();
    book.sheet_mut(0).unwrap().add_chart(client);
    (report, fixture::chart_xml(&book))
}

#[test]
fn a_single_same_kind_group_keeps_its_parts_when_every_series_is_replaced() {
    let basis = fixture::imported_chart(&document(
        &bar((111, 222), &[(0, "B"), (1, "C")], "OLD"),
        &[(111, 222)],
    ));
    let replaced = document(
        &bar((111, 222), &[(7, "E"), (8, "F")], "NEW"),
        &[(111, 222)],
    );
    let (report, xml) = restore(client(&replaced), &basis);
    assert!(
        xml.contains("OLDEXT") && xml.contains("OLDLABELEXT"),
        "{xml}"
    );
    assert!(
        !xml.contains("NEWEXT") && !xml.contains("NEWLABELEXT"),
        "{xml}"
    );
    // The new source data is what is exported; the series have no counterpart.
    assert!(xml.contains("Sheet1!$E$2:$E$5") && xml.contains("Sheet1!$F$2:$F$5"));
    assert!(!xml.contains("Sheet1!$B$2:$B$5"));
    assert_eq!(report.matched_series, 0);
    assert_eq!(report.unmatched_series, 2);
    assert!(
        report
            .diagnostics
            .iter()
            .all(|line| !line.contains("plot group")),
        "{:?}",
        report.diagnostics
    );
}

#[test]
fn two_same_kind_groups_with_every_series_replaced_do_not_match() {
    let basis = fixture::imported_chart(&document(
        &(bar((111, 222), &[(0, "B")], "FIRST") + &bar((111, 222), &[(1, "C")], "SECOND")),
        &[(111, 222)],
    ));
    let replaced = document(
        &(bar((111, 222), &[(7, "E")], "N1") + &bar((111, 222), &[(8, "F")], "N2")),
        &[(111, 222)],
    );
    let (report, xml) = restore(client(&replaced), &basis);
    for marker in ["FIRST", "SECOND"] {
        assert!(!xml.contains(marker), "{marker} came back: {xml}");
    }
    for position in [0, 1] {
        assert!(
            report
                .diagnostics
                .contains(&format!("unmatched or ambiguous plot group:{position}")),
            "{:?}",
            report.diagnostics
        );
    }
}

#[test]
fn a_group_on_other_axes_does_not_match_even_when_every_series_is_replaced() {
    let basis = fixture::imported_chart(&document(
        &bar((111, 222), &[(0, "B")], "OLD"),
        &[(111, 222), (333, 444)],
    ));
    let replaced = document(
        &bar((333, 444), &[(7, "E")], "NEW"),
        &[(111, 222), (333, 444)],
    );
    let (report, xml) = restore(client(&replaced), &basis);
    assert!(
        !xml.contains("OLDEXT") && !xml.contains("OLDLABELEXT"),
        "{xml}"
    );
    assert!(
        report
            .diagnostics
            .contains(&"unmatched or ambiguous plot group:0".to_string()),
        "{:?}",
        report.diagnostics
    );
}

#[test]
fn an_explicit_group_label_removal_wins_over_the_matched_group() {
    let basis = fixture::imported_chart(&document(
        &bar((111, 222), &[(0, "B"), (1, "C")], "OLD"),
        &[(111, 222)],
    ));
    let replaced = document(
        &bar((111, 222), &[(7, "E"), (8, "F")], "NEW"),
        &[(111, 222)],
    );
    let mut edited = client(&replaced);
    assert!(
        edited
            .chart_space_mut()
            .chart_mut()
            .plot_area_mut()
            .chart_groups_mut()
            .next()
            .unwrap()
            .remove_data_labels()
    );
    let (_, xml) = restore(edited, &basis);
    assert!(xml.contains("OLDEXT"), "{xml}");
    assert!(!xml.contains("OLDLABELEXT"), "{xml}");
    assert!(xml.contains("</c:ser><c:gapWidth"), "{xml}");
}
