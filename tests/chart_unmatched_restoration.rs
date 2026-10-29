#![cfg(feature = "serde")]

use std::io::Cursor;

use umya_spreadsheet::{
    Chart,
    drawing::charts::ChartGroupRef,
};

#[path = "support/chart_xml_fixture.rs"]
pub mod fixture;

const EXT: &str = "<c:extLst><c:ext uri=\"urn:untrusted\"><u:payload/></c:ext></c:extLst>";
const TEXT: &str = "<c:txPr><a:bodyPr/><a:lstStyle/><a:p><a:pPr/><a:r><a:rPr u=\"dbl\" \
                    kern=\"1200\"/><a:t>editable</a:t></a:r><a:endParaRPr/></a:p><a:p><a:r><a:\
                    t>omitted paragraph properties</a:t></a:r></a:p></c:txPr>";

fn imported(series: &str, group: &str, axis: &str, pivots: &str) -> Chart {
    fixture::imported_chart(&format!("<c:chartSpace xmlns:c=\"http://schemas.openxmlformats.org/drawingml/2006/chart\" xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" xmlns:u=\"urn:future\"><c:chart>{pivots}<c:plotArea><c:lineChart><c:grouping val=\"standard\"/>{series}{group}<c:axId val=\"10\"/><c:axId val=\"20\"/></c:lineChart>{axis}</c:plotArea></c:chart></c:chartSpace>"))
}

fn series(index: Option<u32>, formula: &str, hidden: bool) -> String {
    let index = index
        .map(|n| format!("<c:idx val=\"{n}\"/>"))
        .unwrap_or_default();
    let parts = if hidden {
        format!(
            "<c:dPt><c:idx val=\"9\"/></c:dPt><c:dLbls><c:dLbl><c:idx \
             val=\"3\"/><c:tx><c:strRef><c:f>Injected!$G$1</c:f></c:strRef></c:tx></c:dLbl><c:\
             showVal/><c:showPercent val=\"false\"/>{TEXT}</c:dLbls><c:trendline><c:trendlineType \
             val=\"linear\"/></c:trendline><c:errBars><c:errDir val=\"y\"/><c:errBarType \
             val=\"both\"/><c:errValType \
             val=\"cust\"/><c:plus><c:numRef><c:f>Injected!$E$2:$E$5</c:f></c:numRef></c:plus></c:\
             errBars>"
        )
    } else {
        String::new()
    };
    format!(
        "<c:ser>{index}<c:order \
         val=\"0\"/>{parts}<c:val><c:numRef><c:f>{formula}</c:f></c:numRef></c:val></c:ser>"
    )
}

fn exported_and_reloaded(chart: Chart) -> (String, Chart) {
    let mut book = umya_spreadsheet::new_file();
    book.new_sheet("Injected").unwrap();
    book.sheet_mut(0).unwrap().add_chart(chart);
    let xml = fixture::chart_xml(&book);
    assert!(
        !xml.contains("Injected!"),
        "untrusted reference exported: {xml}"
    );
    let mut reloaded = umya_spreadsheet::reader::xlsx::read_reader(
        Cursor::new(fixture::write_workbook(&book)),
        true,
    )
    .unwrap();
    reloaded.set_sheet_name(1, "RenamedInjection").unwrap();
    let after = fixture::chart_xml(&reloaded);
    assert!(
        !after.contains("Injected!") && !after.contains("RenamedInjection!"),
        "hidden role joined later reference traversal: {after}"
    );
    (
        xml,
        reloaded.sheet(0).unwrap().chart_collection()[0].clone(),
    )
}

fn assert_series_clean(chart: &Chart, count: usize) {
    let group = chart
        .chart_space()
        .chart()
        .plot_area()
        .chart_groups()
        .next()
        .unwrap();
    assert_eq!(group.series().area_chart_series().len(), count);
    for owner in group.series().area_chart_series() {
        assert!(owner.parts().is_none());
        assert!(owner.data_points().is_empty());
        assert!(owner.trendlines().is_empty());
        assert!(owner.error_bars().is_empty());
        assert!(owner.data_labels().unwrap().labels().is_empty());
        assert_eq!(
            owner
                .values()
                .unwrap()
                .number_reference()
                .formula()
                .address_str(),
            "Data!$H$8:$H$11"
        );
    }
}

#[test]
fn unmatched_populated_series_drops_each_hidden_occurrence_but_keeps_editable_state() {
    let previous = imported(&series(Some(1), "Data!$B$2:$B$5", false), "", "", "");
    let before = bincode::serialize(&previous).unwrap();
    let mut current = imported(&series(Some(44), "Data!$H$8:$H$11", true), "", "", "");
    let report = current.restore_export_parts(&previous);
    assert_eq!(report.unmatched_series, 1);
    assert_eq!(report.ambiguous_series, 0);
    assert_eq!(
        report.removed_incompatible_parts, 4,
        "count occurrence parents, not nested error formula: {report:?}"
    );
    for kind in ["point", "trend", "error", "label"] {
        assert!(
            report
                .diagnostics
                .iter()
                .any(|d| d.to_ascii_lowercase().contains(kind) && d.contains("group:0/series:0")),
            "{report:?}"
        );
    }
    assert_series_clean(&current, 1);
    let (xml, reloaded) = exported_and_reloaded(current);
    for tag in ["<c:dPt>", "<c:trendline>", "<c:errBars>", "<c:dLbl>"] {
        assert!(!xml.contains(tag), "{xml}");
    }
    for editable in [
        "<c:showVal/>",
        "<c:showPercent val=\"false\"/>",
        "u=\"dbl\"",
        "kern=\"1200\"",
        "<a:pPr/>",
        "<a:endParaRPr/>",
        "<a:p><a:r><a:t>omitted paragraph properties</a:t>",
        "editable",
        "Data!$H$8:$H$11",
    ] {
        assert!(xml.contains(editable), "lost {editable}: {xml}");
    }
    assert_series_clean(&reloaded, 1);
    assert_eq!(bincode::serialize(&previous).unwrap(), before);
}

#[test]
fn ambiguous_populated_pair_is_cleared_without_positional_matching() {
    let previous = imported(
        &(series(None, "Data!$H$8:$H$11", false) + &series(None, "Data!$H$8:$H$11", false)),
        "",
        "",
        "",
    );
    let before = bincode::serialize(&previous).unwrap();
    let mut current = imported(
        &(series(None, "Data!$H$8:$H$11", true) + &series(None, "Data!$H$8:$H$11", true)),
        "",
        "",
        "",
    );
    let report = current.restore_export_parts(&previous);
    assert_eq!(report.ambiguous_series, 2);
    assert_eq!(report.matched_series, 0);
    assert_eq!(report.removed_incompatible_parts, 8, "{report:?}");
    assert_series_clean(&current, 2);
    let (_, reloaded) = exported_and_reloaded(current);
    assert_series_clean(&reloaded, 2);
    assert_eq!(bincode::serialize(&previous).unwrap(), before);
}

#[test]
fn unmatched_group_drops_extensions_and_point_labels_not_label_flag_presence() {
    let previous = imported(&series(Some(1), "Data!$B$2:$B$5", false), "", "", "");
    let before = bincode::serialize(&previous).unwrap();
    let labels = format!(
        "<c:dLbls><c:dLbl><c:idx \
         val=\"3\"/><c:tx><c:strRef><c:f>Injected!$G$1</c:f></c:strRef></c:tx></c:dLbl><c:showVal/\
         ><c:showPercent val=\"false\"/>{TEXT}</c:dLbls>{EXT}"
    );
    let mut current = imported(&series(Some(44), "Data!$H$8:$H$11", false), &labels, "", "");
    let report = current.restore_export_parts(&previous);
    assert_eq!(report.removed_incompatible_parts, 2, "{report:?}");
    match current
        .chart_space()
        .chart()
        .plot_area()
        .chart_groups()
        .next()
        .unwrap()
    {
        ChartGroupRef::LineChart(group) => {
            assert!(group.extensions().is_none());
            assert!(group.data_labels().labels().is_empty());
        }
        _ => panic!("wrong group"),
    }
    let (xml, _) = exported_and_reloaded(current);
    for editable in [
        "<c:showVal/>",
        "<c:showPercent val=\"false\"/>",
        "u=\"dbl\"",
        "kern=\"1200\"",
        "<a:pPr/>",
        "<a:endParaRPr/>",
        "<a:p><a:r><a:t>omitted paragraph properties</a:t>",
    ] {
        assert!(xml.contains(editable), "lost {editable}: {xml}");
    }
    assert!(!xml.contains("urn:untrusted") && !xml.contains("<c:dLbl>"));
    assert_eq!(bincode::serialize(&previous).unwrap(), before);
}

#[test]
fn unmatched_value_axis_drops_units_and_duplicates_not_editable_text() {
    let previous = imported(
        &series(Some(1), "Data!$B$2:$B$5", false),
        "",
        "<c:valAx><c:axId val=\"20\"/></c:valAx>",
        "",
    );
    let before = bincode::serialize(&previous).unwrap();
    let units = "<c:dispUnits><c:builtInUnit \
                 val=\"thousands\"/><c:dispUnitsLbl><c:tx><c:strRef><c:f>Injected!$C$1</c:f></c:\
                 strRef></c:tx></c:dispUnitsLbl></c:dispUnits>";
    let axis = format!("<c:valAx><c:axId val=\"77\"/>{TEXT}{units}{units}</c:valAx>");
    let mut current = imported(&series(Some(1), "Data!$B$2:$B$5", false), "", &axis, "");
    let report = current.restore_export_parts(&previous);
    assert_eq!(report.removed_incompatible_parts, 2, "{report:?}");
    let axis = &current.chart_space().chart().plot_area().value_axis()[0];
    assert!(axis.display_units().is_none());
    assert!(axis.extra_display_units().is_empty());
    assert_eq!(axis.axis_id().val(), 77);
    let (xml, _) = exported_and_reloaded(current);
    assert!(!xml.contains("<c:dispUnits>"));
    for editable in [
        "u=\"dbl\"",
        "kern=\"1200\"",
        "<a:pPr/>",
        "<a:endParaRPr/>",
        "<a:p><a:r><a:t>omitted paragraph properties</a:t>",
    ] {
        assert!(xml.contains(editable), "lost {editable}: {xml}");
    }
    assert_eq!(bincode::serialize(&previous).unwrap(), before);
}

fn pivot(index: Option<u32>, formula: &str) -> String {
    let index = index
        .map(|n| format!("<c:idx val=\"{n}\"/>"))
        .unwrap_or_default();
    format!(
        "<c:pivotFmt>{index}<c:spPr/><c:marker><c:symbol \
         val=\"circle\"/></c:marker><c:dLbl><c:idx \
         val=\"9\"/><c:tx><c:strRef><c:f>{formula}</c:f></c:strRef></c:tx></c:dLbl>{EXT}</c:\
         pivotFmt>"
    )
}

#[test]
fn basis_without_pivots_rejects_supplied_collections_and_duplicate_containers() {
    for old_pivots in ["", "<c:pivotFmts/>"] {
        let previous = imported(
            &series(Some(1), "Data!$B$2:$B$5", false),
            "",
            "",
            old_pivots,
        );
        let before = bincode::serialize(&previous).unwrap();
        let pivots = format!(
            "<c:pivotFmts>{}</c:pivotFmts><c:pivotFmts>{}</c:pivotFmts>",
            pivot(Some(7), "Injected!$D$1"),
            pivot(Some(8), "Injected!$E$1")
        );
        let mut current = imported(&series(Some(1), "Data!$B$2:$B$5", false), "", "", &pivots);
        let report = current.restore_export_parts(&previous);
        assert_eq!(report.removed_incompatible_parts, 2, "{report:?}");
        assert!(
            current
                .chart_space()
                .chart()
                .pivot_formats()
                .is_none_or(|p| p.is_empty())
        );
        assert!(
            current
                .chart_space()
                .chart()
                .duplicate_pivot_containers()
                .is_empty()
        );
        let (xml, _) = exported_and_reloaded(current);
        assert!(!xml.contains("<c:pivotFmt>"));
        assert_eq!(bincode::serialize(&previous).unwrap(), before);
    }
}

#[test]
fn ambiguous_and_missing_pivot_placeholders_survive_but_their_hidden_owners_do_not() {
    let previous = imported(
        "",
        "",
        "",
        &format!(
            "<c:pivotFmts>{}{}</c:pivotFmts>",
            pivot(Some(7), "Data!$D$1"),
            pivot(Some(8), "Data!$E$1")
        ),
    );
    let before = bincode::serialize(&previous).unwrap();
    for supplied in [vec![Some(7), Some(7)], vec![None]] {
        let pivots = supplied
            .iter()
            .map(|index| pivot(*index, "Injected!$G$1"))
            .collect::<String>();
        let mut current = imported("", "", "", &format!("<c:pivotFmts>{pivots}</c:pivotFmts>"));
        let report = current.restore_export_parts(&previous);
        assert!(report.incompatible_owners > 0);
        let owners = current.chart_space().chart().pivot_formats().unwrap();
        assert!(owners.len() >= supplied.len());
        for (owner, index) in owners.iter().zip(&supplied) {
            assert_eq!(owner.index_is_present(), index.is_some());
            if let Some(index) = index {
                assert_eq!(owner.index().val(), *index);
            }
            assert!(owner.label().is_none());
            assert!(owner.shape_properties().is_none());
            assert!(owner.marker().is_none());
            assert!(owner.extensions().is_none());
        }
        let recreated = &owners[supplied.len()..];
        assert_eq!(
            recreated
                .iter()
                .map(|p| p.index().val())
                .collect::<Vec<_>>(),
            if supplied.len() == 1 {
                vec![7, 8]
            } else {
                vec![8]
            }
        );
        assert!(recreated.iter().all(|p| p.label().is_some()));
        assert!(
            !serde_json::to_string(&current)
                .unwrap()
                .contains("Injected!")
        );
        if supplied.iter().all(Option::is_some) {
            // Repeated explicit indices are ambiguous for restoration, not
            // invalid XML: each pivot still has its required idx child.
            let (xml, reloaded) = exported_and_reloaded(current.clone());
            assert_eq!(xml.matches("<c:pivotFmt>").count(), 3);
            let reloaded_owners = reloaded.chart_space().chart().pivot_formats().unwrap();
            assert_eq!(
                reloaded_owners
                    .iter()
                    .map(|owner| owner.index().val())
                    .collect::<Vec<_>>(),
                [7, 7, 8]
            );
            assert!(reloaded_owners.iter().all(|owner| owner.index_is_present()));
            for owner in &reloaded_owners[..supplied.len()] {
                assert!(owner.label().is_none());
                assert!(owner.shape_properties().is_none());
                assert!(owner.marker().is_none());
                assert!(owner.extensions().is_none());
            }
            assert!(reloaded_owners[2].label().is_some());
        } else {
            // Missing required idx remains invalid; removing hidden children
            // must not legalize the supplied placeholder's identity.
            let mut invalid_book = umya_spreadsheet::new_file();
            invalid_book
                .sheet_mut(0)
                .unwrap()
                .add_chart(current.clone());
            let error = umya_spreadsheet::writer::xlsx::write_writer(
                &invalid_book,
                &mut Cursor::new(Vec::new()),
            )
            .unwrap_err();
            assert!(error.to_string().contains("chart/pivotFmts"), "{error}");
            let mut exportable = current.clone();
            exportable
                .chart_space_mut()
                .chart_mut()
                .set_pivot_formats(recreated.to_vec());
            exported_and_reloaded(exportable);
        }
        assert_eq!(bincode::serialize(&previous).unwrap(), before);
    }
}

#[test]
fn matched_value_axis_without_authoritative_units_rejects_incoming_units() {
    let previous = imported(
        &series(Some(1), "Data!$B$2:$B$5", false),
        "",
        "<c:valAx><c:axId val=\"20\"/></c:valAx>",
        "",
    );
    let before = bincode::serialize(&previous).unwrap();
    let axis = format!(
        "<c:valAx><c:axId val=\"20\"/><c:numFmt formatCode=\"0.00\" \
         sourceLinked=\"false\"/>{TEXT}<c:dispUnits><c:builtInUnit \
         val=\"thousands\"/><c:dispUnitsLbl><c:tx><c:strRef><c:f>Injected!$C$1</c:f></c:strRef></\
         c:tx></c:dispUnitsLbl></c:dispUnits></c:valAx>"
    );
    let mut current = imported(&series(Some(1), "Data!$B$2:$B$5", false), "", &axis, "");
    let report = current.restore_export_parts(&previous);
    assert_eq!(report.removed_incompatible_parts, 1, "{report:?}");
    assert!(
        current.chart_space().chart().plot_area().value_axis()[0]
            .display_units()
            .is_none()
    );
    let (xml, _) = exported_and_reloaded(current);
    for editable in [
        "formatCode=\"0.00\"",
        "sourceLinked=\"false\"",
        "u=\"dbl\"",
        "kern=\"1200\"",
        "<a:pPr/>",
        "<a:endParaRPr/>",
        "<a:p><a:r><a:t>omitted paragraph properties</a:t>",
    ] {
        assert!(xml.contains(editable), "lost {editable}: {xml}");
    }
    assert_eq!(bincode::serialize(&previous).unwrap(), before);
}

#[test]
fn empty_authoritative_pivot_owner_counts_only_actual_recreation() {
    let plain = imported(&series(Some(1), "Data!$B$2:$B$5", false), "", "", "");
    let mut baseline = plain.clone();
    let ordinary_owners = baseline.restore_export_parts(&plain).restored_owners;
    let empty = imported(
        &series(Some(1), "Data!$B$2:$B$5", false),
        "",
        "",
        "<c:pivotFmts/>",
    );
    let before = bincode::serialize(&empty).unwrap();
    for (basis, projected, extra_owner) in
        [(&empty, true, 1), (&empty, false, 0), (&plain, false, 0)]
    {
        let mut current = basis.clone();
        if projected {
            current.strip_export_parts();
        }
        let report = current.restore_export_parts(basis);
        assert_eq!(
            report.restored_owners,
            ordinary_owners + extra_owner,
            "{report:?}"
        );
        assert_eq!(report.removed_incompatible_parts, 0);
        assert_eq!(
            current.chart_space().chart().pivot_formats().is_some(),
            basis.chart_space().chart().pivot_formats().is_some()
        );
    }
    assert_eq!(bincode::serialize(&empty).unwrap(), before);
}

#[test]
fn matched_value_axis_with_authoritative_empty_parts_rejects_incoming_units() {
    let mut previous = imported(
        &series(Some(1), "Data!$B$2:$B$5", false),
        "",
        "<c:valAx><c:axId val=\"20\"/></c:valAx>",
        "",
    );
    // An allocated empty native owner is valid and equivalent to absence for
    // hidden-content authority, but its persistence representation is retained.
    let empty_parts = serde_json::json!({"display_units": null, "extra_display_units": []});
    let mut axis_json =
        serde_json::to_value(&previous.plot_area_mut().value_axis_mut()[0]).unwrap();
    axis_json["parts"] = empty_parts.clone();
    previous.plot_area_mut().value_axis_mut()[0] = serde_json::from_value(axis_json).unwrap();
    let encoded = bincode::serialize(&previous).unwrap();
    let bases = [
        previous.clone(),
        serde_json::from_slice(&serde_json::to_vec(&previous).unwrap()).unwrap(),
        bincode::deserialize(&encoded).unwrap(),
    ];
    let incoming_axis = format!(
        "<c:valAx><c:axId val=\"20\"/><c:numFmt formatCode=\"0.00\" \
         sourceLinked=\"false\"/>{TEXT}<c:dispUnits><c:builtInUnit \
         val=\"thousands\"/><c:dispUnitsLbl><c:tx><c:strRef><c:f>Injected!$C$1</c:f></c:strRef></\
         c:tx></c:dispUnitsLbl></c:dispUnits></c:valAx>"
    );
    for basis in bases {
        let before = bincode::serialize(&basis).unwrap();
        let old_axis = &basis.chart_space().chart().plot_area().value_axis()[0];
        assert_eq!(
            serde_json::to_value(old_axis).unwrap()["parts"],
            empty_parts
        );
        assert!(!old_axis.display_units_has_conflict());
        let mut present = basis.clone();
        let present_report = present.restore_export_parts(&basis);
        let mut omitted = basis.clone();
        omitted.strip_export_parts();
        let omitted_report = omitted.restore_export_parts(&basis);
        assert_eq!(
            omitted_report.restored_owners,
            present_report.restored_owners + 1
        );
        let mut current = imported(
            &series(Some(1), "Data!$B$2:$B$5", false),
            "",
            &incoming_axis,
            "",
        );
        let report = current.restore_export_parts(&basis);
        assert_eq!(report.removed_incompatible_parts, 1, "{report:?}");
        assert!(report.diagnostics.iter().any(|diagnostic| {
            diagnostic.contains("display units") && diagnostic.contains("axis id:20")
        }));
        let axis = &current.chart_space().chart().plot_area().value_axis()[0];
        assert!(axis.display_units().is_none());
        assert!(axis.extra_display_units().is_empty());
        assert_eq!(serde_json::to_value(axis).unwrap()["parts"], empty_parts);
        let persisted: Chart =
            bincode::deserialize(&bincode::serialize(&current).unwrap()).unwrap();
        assert_eq!(
            serde_json::to_value(&persisted.chart_space().chart().plot_area().value_axis()[0])
                .unwrap()["parts"],
            empty_parts
        );
        let (xml, reloaded) = exported_and_reloaded(current);
        assert!(!xml.contains("<c:dispUnits>"));
        assert!(
            reloaded.chart_space().chart().plot_area().value_axis()[0]
                .display_units()
                .is_none()
        );
        for editable in [
            "formatCode=\"0.00\"",
            "sourceLinked=\"false\"",
            "u=\"dbl\"",
            "kern=\"1200\"",
            "<a:pPr/>",
            "<a:endParaRPr/>",
            "<a:p><a:r><a:t>omitted paragraph properties</a:t>",
        ] {
            assert!(xml.contains(editable), "lost {editable}: {xml}");
        }
        assert_eq!(bincode::serialize(&basis).unwrap(), before);
    }
    assert_eq!(bincode::serialize(&previous).unwrap(), encoded);
}

fn authority_variants(chart: &Chart) -> [Chart; 3] {
    [
        chart.clone(),
        serde_json::from_slice(&serde_json::to_vec(chart).unwrap()).unwrap(),
        bincode::deserialize(&bincode::serialize(chart).unwrap()).unwrap(),
    ]
}

fn promoted_extension(reference: &str, uri: &str) -> String {
    format!(
        "<c:extLst><c:ext uri=\"{uri}\"><u:w \
         xmlns:c15=\"http://schemas.microsoft.com/office/drawing/2012/chart\"><c15:fullRef><c15:\
         sqref>{reference}</c15:sqref></c15:fullRef></u:w></c:ext></c:extLst>"
    )
}

#[test]
fn matched_group_discards_supplied_extensions_without_an_authoritative_extension() {
    let previous = imported(&series(Some(1), "Data!$B$2:$B$5", false), "", "", "");
    let labels =
        format!("<c:dLbls><c:showVal/><c:showPercent val=\"false\"/>{TEXT}</c:dLbls>{EXT}");
    for basis in authority_variants(&previous) {
        let before = bincode::serialize(&basis).unwrap();
        let mut current = imported(&series(Some(1), "Data!$B$2:$B$5", false), &labels, "", "");
        let report = current.restore_export_parts(&basis);
        assert_eq!(report.matched_series, 1);
        assert_eq!(report.removed_incompatible_parts, 1, "{report:?}");
        assert!(report.diagnostics.iter().any(|diagnostic| {
            diagnostic.to_ascii_lowercase().contains("extension")
                && (diagnostic.contains("group") || diagnostic.contains("lineChart"))
        }));
        match current
            .chart_space()
            .chart()
            .plot_area()
            .chart_groups()
            .next()
            .unwrap()
        {
            ChartGroupRef::LineChart(group) => {
                assert!(group.extensions().is_none());
                assert!(group.data_labels().show_value().val());
                assert!(!group.data_labels().show_percent().val());
                let labels = serde_json::to_value(group.data_labels()).unwrap();
                assert_eq!(
                    labels["parts"]["present_flags"].as_u64().unwrap() & 0x12,
                    0x12
                );
            }
            _ => panic!("wrong group"),
        }
        let (xml, _) = exported_and_reloaded(current);
        assert!(!xml.contains("urn:untrusted"));
        for editable in [
            "<c:showVal val=\"1\"/>",
            "<c:showPercent val=\"0\"/>",
            "u=\"dbl\"",
            "kern=\"1200\"",
            "<a:pPr/>",
            "<a:p><a:r><a:t>omitted paragraph properties</a:t>",
        ] {
            assert!(xml.contains(editable), "lost {editable}: {xml}");
        }
        assert_eq!(bincode::serialize(&basis).unwrap(), before);
    }
}

#[test]
fn nested_numeric_extensions_are_untrusted_for_matched_unmatched_and_ambiguous_series() {
    let injection = promoted_extension("Injected!$A$1", "urn:untrusted");
    for (previous_index, current_index, formula, population) in [
        (Some(1), Some(1), "Data!$B$2:$B$5", 1),
        (Some(1), Some(44), "Data!$H$8:$H$11", 1),
        (None, None, "Data!$H$8:$H$11", 2),
    ] {
        let old_formula = if population == 2 {
            formula
        } else {
            "Data!$B$2:$B$5"
        };
        let previous_series = series(previous_index, old_formula, false).repeat(population);
        let supplied = series(current_index, formula, false)
            .replace("</c:numRef>", &format!("{injection}</c:numRef>"))
            .replace(
                "<c:order val=\"0\"/>",
                &format!("<c:order val=\"0\"/><c:dLbls><c:showVal/>{TEXT}</c:dLbls>"),
            )
            .repeat(population);
        let previous = imported(&previous_series, "", "", "");
        let incoming = imported(&supplied, "", "", "");
        // Prove the nested opaque terminal is a live promoted reference, not
        // merely a string that happens to disappear from writer output.
        let mut promotion_control = incoming.clone();
        let promotion_report = promotion_control.adjust_references(
            "Sheet1",
            &umya_spreadsheet::drawing::charts::ChartReferenceEdit::RenameSheet {
                old: "Injected".into(),
                new: "RenamedInjection".into(),
            },
        );
        assert_eq!(promotion_report.changed, population);
        for basis in authority_variants(&previous) {
            let before = bincode::serialize(&basis).unwrap();
            let mut current = incoming.clone();
            let report = current.restore_export_parts(&basis);
            assert_eq!(report.matched_series, usize::from(current_index == Some(1)));
            assert_eq!(
                report.unmatched_series,
                usize::from(current_index == Some(44))
            );
            assert_eq!(report.ambiguous_series, if population == 2 { 2 } else { 0 });
            assert_eq!(report.removed_incompatible_parts, population, "{report:?}");
            assert_eq!(
                report
                    .diagnostics
                    .iter()
                    .filter(|diagnostic| {
                        diagnostic.to_ascii_lowercase().contains("extension")
                            && diagnostic.contains("numRef")
                    })
                    .count(),
                population,
                "{report:?}"
            );
            let owners = current
                .chart_space()
                .chart()
                .plot_area()
                .chart_groups()
                .next()
                .unwrap();
            for owner in owners.series().area_chart_series() {
                let reference = owner.values().unwrap().number_reference();
                assert!(reference.extensions().is_none());
                assert_eq!(reference.formula().address_str(), formula);
            }
            let (xml, reloaded) = exported_and_reloaded(current);
            assert!(!xml.contains("urn:untrusted"));
            for editable in [
                "u=\"dbl\"",
                "kern=\"1200\"",
                "<a:pPr/>",
                "<a:p><a:r><a:t>omitted paragraph properties</a:t>",
            ] {
                assert!(xml.contains(editable), "lost {editable}: {xml}");
            }
            let reloaded_group = reloaded
                .chart_space()
                .chart()
                .plot_area()
                .chart_groups()
                .next()
                .unwrap();
            assert!(
                reloaded_group
                    .series()
                    .area_chart_series()
                    .iter()
                    .all(|owner| {
                        owner
                            .values()
                            .unwrap()
                            .number_reference()
                            .extensions()
                            .is_none()
                    })
            );
            assert_eq!(bincode::serialize(&basis).unwrap(), before);
        }
    }
}

#[test]
fn matched_shape_extension_is_dropped_without_losing_current_shape_or_text_styles() {
    let old_series = series(Some(1), "Data!$B$2:$B$5", false)
        .replace("<c:order val=\"0\"/>", "<c:order val=\"0\"/><c:spPr/>");
    let previous = imported(&old_series, "", "", "");
    let shape_extension = EXT.replace("c:ext", "a:ext");
    let supplied = series(Some(1), "Data!$B$2:$B$5", false).replace(
        "<c:order val=\"0\"/>",
        &format!(
            "<c:order val=\"0\"/><c:spPr><a:solidFill><a:srgbClr \
             val=\"123456\"/></a:solidFill>{shape_extension}</c:spPr><c:dLbls><c:showVal/>{TEXT}</\
             c:dLbls>"
        ),
    );
    for basis in authority_variants(&previous) {
        let before = bincode::serialize(&basis).unwrap();
        let mut current = imported(&supplied, "", "", "");
        let report = current.restore_export_parts(&basis);
        assert_eq!(report.matched_series, 1);
        assert_eq!(report.removed_incompatible_parts, 1, "{report:?}");
        assert!(report.diagnostics.iter().any(|diagnostic| {
            diagnostic.to_ascii_lowercase().contains("extension") && diagnostic.contains("spPr")
        }));
        let group = current
            .chart_space()
            .chart()
            .plot_area()
            .chart_groups()
            .next()
            .unwrap();
        assert!(
            group.series().area_chart_series()[0]
                .shape_properties()
                .unwrap()
                .extensions()
                .is_none()
        );
        let (xml, _) = exported_and_reloaded(current);
        assert!(!xml.contains("urn:untrusted"));
        for editable in [
            "val=\"123456\"",
            "u=\"dbl\"",
            "kern=\"1200\"",
            "<a:pPr/>",
            "<a:p><a:r><a:t>omitted paragraph properties</a:t>",
        ] {
            assert!(xml.contains(editable), "lost {editable}: {xml}");
        }
        assert_eq!(bincode::serialize(&basis).unwrap(), before);
    }
}

#[test]
fn authoritative_extensions_replace_different_supplied_group_shape_and_numeric_extensions() {
    let trusted = promoted_extension("Data!$Z$1", "urn:trusted");
    let untrusted = promoted_extension("Injected!$A$1", "urn:untrusted");
    let old_series = series(Some(1), "Data!$B$2:$B$5", false)
        .replace("</c:numRef>", &format!("{trusted}</c:numRef>"))
        .replace(
            "<c:order val=\"0\"/>",
            &format!(
                "<c:order val=\"0\"/><c:spPr>{}</c:spPr>",
                EXT.replace("urn:untrusted", "urn:trusted")
                    .replace("c:ext", "a:ext")
            ),
        );
    let previous = imported(
        &old_series,
        &EXT.replace("urn:untrusted", "urn:trusted"),
        "",
        "",
    );
    let supplied = series(Some(1), "Data!$B$2:$B$5", false)
        .replace("</c:numRef>", &format!("{untrusted}</c:numRef>"))
        .replace(
            "<c:order val=\"0\"/>",
            &format!(
                "<c:order val=\"0\"/><c:spPr><a:solidFill><a:srgbClr \
                 val=\"123456\"/></a:solidFill>{}</c:spPr>",
                EXT.replace("c:ext", "a:ext")
            ),
        );
    for basis in authority_variants(&previous) {
        let before = bincode::serialize(&basis).unwrap();
        let mut current = imported(&supplied, EXT, "", "");
        let report = current.restore_export_parts(&basis);
        assert_eq!(report.matched_series, 1);
        assert_eq!(report.removed_incompatible_parts, 3, "{report:?}");
        let old_group = basis
            .chart_space()
            .chart()
            .plot_area()
            .chart_groups()
            .next()
            .unwrap();
        let group = current
            .chart_space()
            .chart()
            .plot_area()
            .chart_groups()
            .next()
            .unwrap();
        let old_owner = &old_group.series().area_chart_series()[0];
        let owner = &group.series().area_chart_series()[0];
        assert_eq!(
            serde_json::to_value(owner.values().unwrap().number_reference().extensions()).unwrap(),
            serde_json::to_value(old_owner.values().unwrap().number_reference().extensions())
                .unwrap()
        );
        assert_eq!(
            serde_json::to_value(owner.shape_properties().unwrap().extensions()).unwrap(),
            serde_json::to_value(old_owner.shape_properties().unwrap().extensions()).unwrap()
        );
        match (group, old_group) {
            (ChartGroupRef::LineChart(group), ChartGroupRef::LineChart(old_group)) => {
                assert_eq!(
                    serde_json::to_value(group.extensions()).unwrap(),
                    serde_json::to_value(old_group.extensions()).unwrap()
                );
            }
            _ => panic!("wrong group"),
        }
        let (xml, _) = exported_and_reloaded(current);
        assert!(!xml.contains("urn:untrusted"));
        assert_eq!(xml.matches("urn:trusted").count(), 3);
        assert!(xml.contains("Data!$Z$1"));
        assert!(xml.contains("val=\"123456\""));
        assert_eq!(bincode::serialize(&basis).unwrap(), before);
    }
}

#[test]
fn changed_numeric_and_string_formulas_discard_incoming_and_previous_extensions() {
    let trusted = promoted_extension("Data!$Z$1", "urn:trusted");
    let untrusted = promoted_extension("Injected!$A$1", "urn:untrusted");
    let old_series = series(Some(1), "Data!$B$2:$B$5", false)
        .replace("</c:numRef>", &format!("{trusted}</c:numRef>"))
        .replace(
            "<c:order val=\"0\"/>",
            &format!(
                "<c:order val=\"0\"/><c:tx><c:strRef><c:f>Data!$I$1</c:f>{trusted}</c:strRef></c:\
                 tx>"
            ),
        );
    let previous = imported(&old_series, "", "", "");
    let supplied = series(Some(1), "Data!$H$8:$H$11", false)
        .replace("</c:numRef>", &format!("{untrusted}</c:numRef>"))
        .replace(
            "<c:order val=\"0\"/>",
            &format!(
                "<c:order val=\"0\"/><c:tx><c:strRef><c:f>Data!$J$1</c:f>{untrusted}</c:strRef></\
                 c:tx>"
            ),
        );
    for basis in authority_variants(&previous) {
        let before = bincode::serialize(&basis).unwrap();
        let mut current = imported(&supplied, "", "", "");
        let report = current.restore_export_parts(&basis);
        assert_eq!(report.matched_series, 1);
        assert_eq!(report.removed_incompatible_parts, 2, "{report:?}");
        for kind in ["numRef", "strRef"] {
            assert!(
                report.diagnostics.iter().any(|diagnostic| {
                    diagnostic.to_ascii_lowercase().contains("extension")
                        && diagnostic.contains(kind)
                }),
                "{report:?}"
            );
        }
        let group = current
            .chart_space()
            .chart()
            .plot_area()
            .chart_groups()
            .next()
            .unwrap();
        let owner = &group.series().area_chart_series()[0];
        assert!(
            owner
                .values()
                .unwrap()
                .number_reference()
                .extensions()
                .is_none()
        );
        let text = owner.chart_text().unwrap().string_reference().unwrap();
        assert!(text.extensions().is_none());
        assert_eq!(text.formula().address_str(), "Data!$J$1");
        let (xml, _) = exported_and_reloaded(current);
        assert!(!xml.contains("urn:untrusted") && !xml.contains("urn:trusted"));
        assert!(!xml.contains("Data!$Z$1"));
        assert!(xml.contains("Data!$H$8:$H$11") && xml.contains("Data!$J$1"));
        assert_eq!(bincode::serialize(&basis).unwrap(), before);
    }
}

#[test]
fn added_or_default_group_label_owners_discard_hidden_parents_once() {
    let point_extension = EXT.replace("urn:untrusted", "urn:nested-point");
    let labels_extension = promoted_extension("Injected!$Q$1", "urn:untrusted");
    let labels = format!(
        "<c:dLbls><c:dLbl><c:idx \
         val=\"3\"/><c:tx><c:strRef><c:f>Injected!$G$1</c:f></c:strRef></c:tx>{point_extension}</\
         c:dLbl><c:showVal val=\"1\"/><c:showPercent val=\"0\"/>{TEXT}{labels_extension}</c:dLbls>"
    );
    for optional_owner in [true, false] {
        let mut previous = if optional_owner {
            fixture::imported_chart(&format!(
                "<c:chartSpace xmlns:c=\"http://schemas.openxmlformats.org/drawingml/2006/chart\" \
                 xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" \
                 xmlns:u=\"urn:future\"><c:chart><c:plotArea><c:area3DChart><c:grouping \
                 val=\"standard\"/>{}<c:axId val=\"10\"/><c:axId \
                 val=\"20\"/></c:area3DChart></c:plotArea></c:chart></c:chartSpace>",
                series(Some(1), "Data!$B$2:$B$5", false)
            ))
        } else {
            imported(&series(Some(1), "Data!$B$2:$B$5", false), "", "", "")
        };
        if !optional_owner {
            match previous.plot_area_mut().chart_groups_mut().next().unwrap() {
                umya_spreadsheet::drawing::charts::ChartGroupMut::LineChart(group) => {
                    group.set_data_labels(umya_spreadsheet::drawing::charts::DataLabels::default());
                }
                _ => panic!("wrong group"),
            }
        }
        let incoming = if optional_owner {
            fixture::imported_chart(&format!(
                "<c:chartSpace xmlns:c=\"http://schemas.openxmlformats.org/drawingml/2006/chart\" \
                 xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" \
                 xmlns:u=\"urn:future\"><c:chart><c:plotArea><c:area3DChart><c:grouping \
                 val=\"standard\"/>{}{labels}<c:axId val=\"10\"/><c:axId \
                 val=\"20\"/></c:area3DChart></c:plotArea></c:chart></c:chartSpace>",
                series(Some(1), "Data!$B$2:$B$5", false)
            ))
        } else {
            imported(&series(Some(1), "Data!$B$2:$B$5", false), &labels, "", "")
        };
        for basis in authority_variants(&previous) {
            let before = bincode::serialize(&basis).unwrap();
            let old_group = basis
                .chart_space()
                .chart()
                .plot_area()
                .chart_groups()
                .next()
                .unwrap();
            match old_group {
                ChartGroupRef::Area3DChart(group) => assert!(group.data_labels().is_none()),
                ChartGroupRef::LineChart(group) => {
                    assert!(serde_json::to_value(group.data_labels()).unwrap()["parts"].is_null());
                }
                _ => panic!("wrong group"),
            }
            let mut current = incoming.clone();
            let report = current.restore_export_parts(&basis);
            assert_eq!(report.matched_series, 1);
            assert_eq!(report.removed_incompatible_parts, 2, "{report:?}");
            assert!(
                report
                    .diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.contains("point label")),
                "{report:?}"
            );
            assert!(
                report
                    .diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.to_ascii_lowercase().contains("extension")),
                "{report:?}"
            );
            let group = current
                .chart_space()
                .chart()
                .plot_area()
                .chart_groups()
                .next()
                .unwrap();
            let current_labels = match group {
                ChartGroupRef::Area3DChart(group) => group.data_labels().unwrap(),
                ChartGroupRef::LineChart(group) => group.data_labels(),
                _ => panic!("wrong group"),
            };
            assert!(current_labels.labels().is_empty());
            assert!(current_labels.extensions().is_none());
            assert!(current_labels.show_value().val());
            assert!(!current_labels.show_percent().val());
            let encoded_labels = serde_json::to_value(current_labels).unwrap();
            assert_eq!(
                encoded_labels["parts"]["present_flags"].as_u64().unwrap() & 0x12,
                0x12
            );
            let (xml, reloaded) = exported_and_reloaded(current);
            assert!(!xml.contains("<c:dLbl>"));
            assert!(!xml.contains("urn:untrusted") && !xml.contains("urn:nested-point"));
            assert!(
                !serde_json::to_string(&reloaded)
                    .unwrap()
                    .contains("Injected!")
            );
            for editable in [
                "<c:showVal val=\"1\"/>",
                "<c:showPercent val=\"0\"/>",
                "u=\"dbl\"",
                "kern=\"1200\"",
                "<a:pPr/>",
                "<a:p><a:r><a:t>omitted paragraph properties</a:t>",
            ] {
                assert!(xml.contains(editable), "lost {editable}: {xml}");
            }
            assert_eq!(bincode::serialize(&basis).unwrap(), before);
        }
    }
}
