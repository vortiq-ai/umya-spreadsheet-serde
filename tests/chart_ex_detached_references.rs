use std::io::Cursor;

use umya_spreadsheet::{
    DefinedName,
    drawing::{
        charts::*,
        spreadsheet::ExtendedChart,
    },
    reader,
    writer,
};

fn chart(formula: &str) -> ExtendedChart {
    let xml = format!(
        r#"<cx:chartSpace xmlns:cx="http://schemas.microsoft.com/office/drawing/2014/chartex"><cx:chartData><cx:data id="0"><cx:numDim type="val"><cx:f>{formula}</cx:f></cx:numDim></cx:data></cx:chartData><cx:chart><cx:plotArea><cx:plotAreaRegion><cx:series layoutId="waterfall"><cx:dataId val="0"/></cx:series></cx:plotAreaRegion></cx:plotArea></cx:chart></cx:chartSpace>"#
    );
    ExtendedChart::new(
        Default::default(),
        ChartExSpace::from_xml(xml.as_bytes()).unwrap(),
    )
}

fn formulas(chart: &ExtendedChart) -> Vec<String> {
    let mut values = Vec::new();
    chart
        .chart_space()
        .visit_formulas(&mut |formula| values.push(formula.address_str()));
    values
}

fn edits() -> Vec<(ChartReferenceEdit, &'static str)> {
    vec![
        (
            ChartReferenceEdit::RenameSheet {
                old: "Data".into(),
                new: "New Data".into(),
            },
            "'New Data'!$B$2:$B$4",
        ),
        (
            ChartReferenceEdit::DeleteSheet {
                sheet:       "Data".into(),
                sheet_order: vec!["Host".into(), "Data".into(), "Other".into()],
            },
            "#REF!",
        ),
        (
            ChartReferenceEdit::InsertRows {
                sheet: "Data".into(),
                at:    3,
                count: 2,
            },
            "Data!$B$2:$B$6",
        ),
        (
            ChartReferenceEdit::InsertColumns {
                sheet: "Data".into(),
                at:    2,
                count: 2,
            },
            "Data!$D$2:$D$4",
        ),
        (
            ChartReferenceEdit::DeleteRows {
                sheet: "Data".into(),
                rows:  vec![3],
            },
            "Data!$B$2:$B$3",
        ),
        (
            ChartReferenceEdit::DeleteColumns {
                sheet:   "Data".into(),
                columns: vec![1],
            },
            "Data!$A$2:$A$4",
        ),
        (
            ChartReferenceEdit::DeleteColumns {
                sheet:   "Data".into(),
                columns: vec![2],
            },
            "Data!#REF!",
        ),
        (
            ChartReferenceEdit::MoveRange {
                source_sheet:      "Data".into(),
                source:            ReferenceRect {
                    first: ReferenceCell {
                        column: 2,
                        row:    2,
                    },
                    last:  ReferenceCell {
                        column: 2,
                        row:    4,
                    },
                },
                destination_sheet: "Other".into(),
                destination:       ReferenceCell {
                    column: 4,
                    row:    7,
                },
            },
            "Other!$D$7:$D$9",
        ),
    ]
}

#[test]
fn detached_direct_references_follow_structural_edits_and_roundtrip() {
    for (edit, expected) in edits() {
        let mut source = umya_spreadsheet::new_file();
        source.set_sheet_name(0, "Host").unwrap();
        source.new_sheet("Data").unwrap();
        source.new_sheet("Other").unwrap();
        source.new_sheet("New Data").unwrap();
        source
            .sheet_mut(0)
            .unwrap()
            .worksheet_drawing_mut()
            .add_extended_chart(chart("Data!$B$2:$B$4"));
        let mut bytes = Vec::new();
        writer::xlsx::write_writer(&source, &mut bytes).unwrap();
        for eager in [false, true] {
            let mut book = reader::xlsx::read_reader(Cursor::new(&bytes), eager).unwrap();
            let host = book.sheet_mut(0).unwrap();
            let mut detached = host.worksheet_drawing_mut().extended_charts_mut().remove(0);
            let report = detached.adjust_references("Host", &edit).unwrap();
            assert_eq!(report.adjustments.visited, 1);
            assert_eq!(report.adjustments.changed, 1);
            assert_eq!(
                report.adjustments.invalidated,
                usize::from(expected.contains("#REF!"))
            );
            assert!(report.defined_names.is_empty());
            assert_eq!(formulas(&detached), [expected], "{edit:?}");
            host.worksheet_drawing_mut().add_extended_chart(detached);
            let mut output = Vec::new();
            writer::xlsx::write_writer(&book, &mut output).unwrap();
            let reread = reader::xlsx::read_reader(Cursor::new(output), true).unwrap();
            assert_eq!(
                formulas(
                    &reread
                        .sheet(0)
                        .unwrap()
                        .worksheet_drawing()
                        .extended_charts()[0]
                ),
                [expected]
            );
        }
    }
}

#[test]
#[cfg(feature = "serde")]
fn detached_named_references_are_reported_once_and_kept_for_the_defined_name_owner() {
    for (edit, _) in edits() {
        for formula in ["_xlchart.v1.0", "Data!_xlchart.v1.0", "[0]!_xlchart.v1.0"] {
            let mut detached = chart(formula);
            let encoded = serde_json::to_vec(&detached).unwrap();
            detached = serde_json::from_slice(&encoded).unwrap();
            let report = detached.adjust_references("Host", &edit).unwrap();
            let expected = match (&edit, formula.starts_with("Data!")) {
                (ChartReferenceEdit::RenameSheet { .. }, true) => "'New Data'!_xlchart.v1.0",
                (ChartReferenceEdit::DeleteSheet { .. }, true) => "[0]!_xlchart.v1.0",
                _ => formula,
            };
            assert_eq!(formulas(&detached), [expected]);
            assert_eq!(report.adjustments.changed, usize::from(expected != formula));
            assert_eq!(report.defined_names.len(), 1);
            assert_eq!(report.defined_names[0].name, "_xlchart.v1.0");
            assert_eq!(
                report.defined_names[0].sheet.as_deref(),
                if formula.starts_with("Data!") {
                    Some("Data")
                } else {
                    None
                }
            );
            assert_eq!(
                report.defined_names[0].workbook_scoped,
                formula.starts_with("[0]!")
            );
        }
    }
}

#[test]
fn detached_reference_report_deduplicates_names_without_touching_literals_or_external_names() {
    let mut detached = chart(
        "SUM(_xlchart.v1.0,_XLCHART.V1.0,Data!$B$2:$B$4,\"Data!B2\",'[book.xlsx]Data'!_xlchart.v1.\
         0)",
    );
    let report = detached
        .adjust_references(
            "Host",
            &ChartReferenceEdit::InsertRows {
                sheet: "Data".into(),
                at:    3,
                count: 2,
            },
        )
        .unwrap();
    assert_eq!(report.defined_names.len(), 1);
    assert_eq!(
        formulas(&detached),
        [
            "SUM(_xlchart.v1.0,_XLCHART.V1.0,Data!$B$2:$B$6,\"Data!B2\",'[book.xlsx]Data'!\
             _xlchart.v1.0)"
        ]
    );
}

#[test]
fn invalid_detached_edit_is_an_explicit_error_and_does_not_modify_the_chart() {
    let mut detached = chart("Data!$B$2:$B$4");
    assert!(
        detached
            .adjust_references(
                "Host",
                &ChartReferenceEdit::InsertRows {
                    sheet: "Data".into(),
                    at:    0,
                    count: 1,
                }
            )
            .is_err()
    );
    assert_eq!(formulas(&detached), ["Data!$B$2:$B$4"]);
}

#[test]
fn detached_unqualified_references_use_the_supplied_host_sheet() {
    let edit = ChartReferenceEdit::DeleteRows {
        sheet: "Data".into(),
        rows:  vec![2, 3, 4],
    };
    let mut local = chart("$B$2:$B$4");
    let report = local.adjust_references("Data", &edit).unwrap();
    assert_eq!(formulas(&local), ["#REF!"]);
    assert_eq!(report.adjustments.invalidated, 1);
    let mut unrelated = chart("$B$2:$B$4");
    let report = unrelated.adjust_references("Host", &edit).unwrap();
    assert_eq!(formulas(&unrelated), ["$B$2:$B$4"]);
    assert_eq!(report.adjustments.changed, 0);
}

#[test]
#[cfg(feature = "serde")]
fn detached_reference_edits_reach_native_and_standard_compatibility_frames() {
    use umya_spreadsheet::drawing::spreadsheet::{
        DrawingChoice,
        DrawingObject,
        ExtendedChartCompatibility,
    };
    let mut standard = umya_spreadsheet::Chart::default();
    standard.new_chart(
        &umya_spreadsheet::ChartType::LineChart,
        Default::default(),
        Default::default(),
        vec!["Data!$B$2:$B$4"],
    );
    let frame = standard.two_cell_anchor().graphic_frame().unwrap().clone();
    let mut detached = chart("Data!$B$2:$B$4");
    detached.set_compatibility(Some(ExtendedChartCompatibility {
        required_namespaces: vec![(
            "cx".into(),
            "http://schemas.microsoft.com/office/drawing/2014/chartex".into(),
        )],
        preceding_choices: vec![DrawingChoice {
            required_namespaces: vec![("next".into(), "urn:next".into())],
            content:             DrawingObject::ExtendedFrame(Box::new(
                chart("Data!$B$2:$B$4").frame().clone(),
            )),
        }],
        fallback: Some(Box::new(DrawingObject::GraphicFrame(Box::new(frame)))),
        fallback_present: true,
        ..Default::default()
    }));
    let report = detached
        .adjust_references(
            "Host",
            &ChartReferenceEdit::RenameSheet {
                old: "Data".into(),
                new: "New Data".into(),
            },
        )
        .unwrap();
    assert_eq!(report.adjustments.changed, 3);
    let encoded = serde_json::to_string(&detached).unwrap();
    let mut restored: ExtendedChart = serde_json::from_str(&encoded).unwrap();
    assert_eq!(formulas(&restored), ["'New Data'!$B$2:$B$4"]);
    let compatibility = restored.compatibility_mut().unwrap();
    let DrawingObject::ExtendedFrame(native) = &compatibility.preceding_choices[0].content else {
        panic!("native choice")
    };
    let mut native_formulas = Vec::new();
    native
        .chart_space()
        .visit_formulas(&mut |formula| native_formulas.push(formula.address_str()));
    assert_eq!(native_formulas, ["'New Data'!$B$2:$B$4"]);
    let DrawingObject::GraphicFrame(standard) = compatibility.fallback.as_deref_mut().unwrap()
    else {
        panic!("standard fallback")
    };
    let standard_formulas: Vec<_> = standard
        .graphic_mut()
        .graphic_data_mut()
        .chart_space_mut()
        .chart_mut()
        .plot_area_mut()
        .formula_mut()
        .iter()
        .map(|formula| formula.address_str())
        .collect();
    assert_eq!(standard_formulas, ["'New Data'!$B$2:$B$4"]);
}

#[test]
#[cfg(feature = "serde")]
fn detached_chart_preserves_and_reports_opaque_extension_payloads() {
    let mut detached = chart("Data!$B$2:$B$4");
    let xml = r#"<cx:chartSpace xmlns:cx="http://schemas.microsoft.com/office/drawing/2014/chartex"><cx:chartData><cx:data id="0"><cx:numDim type="val"><cx:f>Data!$B$2:$B$4</cx:f></cx:numDim></cx:data></cx:chartData><cx:chart><cx:plotArea><cx:plotAreaRegion><cx:series layoutId="waterfall"><cx:dataId val="0"/></cx:series></cx:plotAreaRegion></cx:plotArea></cx:chart><cx:extLst><cx:ext uri="urn:keep"><v:payload xmlns:v="urn:vendor" text="Data!B2"/></cx:ext></cx:extLst></cx:chartSpace>"#;
    detached
        .frame_mut()
        .set_chart_space(ChartExSpace::from_xml(xml.as_bytes()).unwrap());
    let payload = serde_json::to_value(&detached.chart_space().extensions).unwrap();
    let report = detached
        .adjust_references(
            "Host",
            &ChartReferenceEdit::RenameSheet {
                old: "Data".into(),
                new: "New Data".into(),
            },
        )
        .unwrap();
    assert_eq!(report.adjustments.preserved_opaque_extensions, 1);
    assert_eq!(
        detached.chart_space().extensions.as_ref().unwrap().entries[0]
            .uri
            .as_deref(),
        Some("urn:keep")
    );
    assert_eq!(
        serde_json::to_value(&detached.chart_space().extensions).unwrap(),
        payload
    );
}

#[test]
#[cfg(feature = "serde")]
fn detached_reports_preserved_data_and_color_mapping_extensions() {
    let xml = r#"<cx:chartSpace xmlns:cx="http://schemas.microsoft.com/office/drawing/2014/chartex" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><cx:chartData><cx:data id="0"><cx:numDim type="val"><cx:f>Data!B2</cx:f></cx:numDim><cx:extLst><cx:ext uri="urn:data"><v:payload xmlns:v="urn:vendor"/></cx:ext></cx:extLst></cx:data></cx:chartData><cx:chart><cx:plotArea><cx:plotAreaRegion><cx:series layoutId="waterfall"><cx:dataId val="0"/></cx:series></cx:plotAreaRegion></cx:plotArea></cx:chart><cx:clrMapOvr bg1="lt1" tx1="dk1" bg2="lt2" tx2="dk2" accent1="accent1" accent2="accent2" accent3="accent3" accent4="accent4" accent5="accent5" accent6="accent6" hlink="hlink" folHlink="folHlink"><a:extLst><a:ext uri="urn:one"/><a:ext uri="urn:two"/></a:extLst></cx:clrMapOvr></cx:chartSpace>"#;
    let mut space: ChartExSpace = serde_json::from_value(
        serde_json::to_value(ChartExSpace::from_xml(xml.as_bytes()).unwrap()).unwrap(),
    )
    .unwrap();
    let data_extensions = serde_json::to_value(&space.data().unwrap().data[0].extensions).unwrap();
    let colors = serde_json::to_value(&space.color_mapping).unwrap();
    let report = space
        .adjust_references(
            "Host",
            &ChartReferenceEdit::RenameSheet {
                old: "Data".into(),
                new: "Renamed".into(),
            },
        )
        .unwrap();
    assert_eq!(report.adjustments.preserved_opaque_extensions, 3);
    assert_eq!(
        serde_json::to_value(&space.data().unwrap().data[0].extensions).unwrap(),
        data_extensions
    );
    assert_eq!(serde_json::to_value(&space.color_mapping).unwrap(), colors);
}

#[test]
fn caller_can_rewrite_a_reported_defined_name_once_and_export_current_values() {
    let mut book = umya_spreadsheet::new_file();
    book.set_sheet_name(0, "Host").unwrap();
    book.new_sheet("Data").unwrap();
    book.sheet_mut(1)
        .unwrap()
        .cell_mut("B3")
        .set_value_number(17);
    book.sheet_mut(1)
        .unwrap()
        .cell_mut("B4")
        .set_value_number(23);
    let mut name = DefinedName::default();
    name.set_name("_xlchart.v1.0").set_address("Data!$B$2:$B$3");
    book.add_defined_names(name);
    let mut detached = chart("_xlchart.v1.0");
    let report = detached
        .adjust_references(
            "Host",
            &ChartReferenceEdit::InsertRows {
                sheet: "Data".into(),
                at:    2,
                count: 1,
            },
        )
        .unwrap();
    assert_eq!(report.defined_names.len(), 1);
    book.defined_names_mut()[0].set_address("Data!$B$3:$B$4");
    book.sheet_mut(0)
        .unwrap()
        .worksheet_drawing_mut()
        .add_extended_chart(detached);
    let mut bytes = Vec::new();
    writer::xlsx::write_writer(&book, &mut bytes).unwrap();
    let reread = reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap();
    let chart = &reread
        .sheet(0)
        .unwrap()
        .worksheet_drawing()
        .extended_charts()[0];
    assert_eq!(formulas(chart), ["_xlchart.v1.0"]);
    let mut resolver = WorkbookChartExResolver::new(&reread);
    let xml =
        String::from_utf8(chart.chart_space().to_xml("Host", &mut resolver).unwrap()).unwrap();
    assert!(xml.contains("<cx:pt idx=\"0\">17</cx:pt>"));
    assert!(xml.contains("<cx:pt idx=\"1\">23</cx:pt>"));
}

fn chart_part(bytes: &[u8]) -> String {
    use std::io::Read;
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut xml = String::new();
    archive
        .by_name("xl/charts/chartEx1.xml")
        .unwrap()
        .read_to_string(&mut xml)
        .unwrap();
    xml
}

#[test]
fn detached_edits_export_the_same_chart_part_as_the_workbook_structural_edit() {
    type WorkbookEdit = fn(&mut umya_spreadsheet::Workbook);
    let cell = |column, row| ReferenceCell { column, row };
    let cases: Vec<(ChartReferenceEdit, WorkbookEdit)> = vec![
        (
            ChartReferenceEdit::RenameSheet {
                old: "Data".into(),
                new: "Renamed Data".into(),
            },
            |book| book.set_sheet_name(1, "Renamed Data").unwrap(),
        ),
        (
            ChartReferenceEdit::DeleteSheet {
                sheet:       "Data".into(),
                sheet_order: vec!["Host".into(), "Data".into(), "Other".into()],
            },
            |book| book.remove_sheet_by_name("Data").unwrap(),
        ),
        (
            ChartReferenceEdit::InsertRows {
                sheet: "Data".into(),
                at:    3,
                count: 2,
            },
            |book| book.insert_new_row("Data", 3, 2),
        ),
        (
            ChartReferenceEdit::InsertColumns {
                sheet: "Data".into(),
                at:    2,
                count: 2,
            },
            |book| book.insert_new_column_by_index("Data", 2, 2),
        ),
        (
            ChartReferenceEdit::DeleteRows {
                sheet: "Data".into(),
                rows:  vec![3],
            },
            |book| book.remove_row("Data", 3, 1),
        ),
        (
            ChartReferenceEdit::DeleteColumns {
                sheet:   "Data".into(),
                columns: vec![1],
            },
            |book| book.remove_column_by_index("Data", 1, 1),
        ),
        (
            ChartReferenceEdit::DeleteColumns {
                sheet:   "Data".into(),
                columns: vec![2],
            },
            |book| book.remove_column_by_index("Data", 2, 1),
        ),
        (
            ChartReferenceEdit::MoveRange {
                source_sheet:      "Data".into(),
                source:            ReferenceRect {
                    first: cell(2, 2),
                    last:  cell(2, 4),
                },
                destination_sheet: "Other".into(),
                destination:       cell(4, 7),
            },
            |book| {
                book.move_range(
                    "Data",
                    ReferenceRect {
                        first: ReferenceCell {
                            column: 2,
                            row:    2,
                        },
                        last:  ReferenceCell {
                            column: 2,
                            row:    4,
                        },
                    },
                    "Other",
                    ReferenceCell {
                        column: 4,
                        row:    7,
                    },
                )
                .unwrap();
            },
        ),
    ];
    let mut source = umya_spreadsheet::new_file();
    source.set_sheet_name(0, "Host").unwrap();
    source.new_sheet("Data").unwrap();
    source.new_sheet("Other").unwrap();
    for row in 2..=4 {
        source
            .sheet_mut(1)
            .unwrap()
            .cell_mut((2, row))
            .set_value_number(row * 11);
    }
    source
        .sheet_mut(0)
        .unwrap()
        .worksheet_drawing_mut()
        .add_extended_chart(chart("Data!$B$2:$B$4"));
    let mut bytes = Vec::new();
    writer::xlsx::write_writer(&source, &mut bytes).unwrap();
    for (edit, workbook_edit) in cases {
        let mut in_workbook = reader::xlsx::read_reader(Cursor::new(&bytes), true).unwrap();
        workbook_edit(&mut in_workbook);
        let mut expected = Vec::new();
        writer::xlsx::write_writer(&in_workbook, &mut expected).unwrap();

        // Only the chart formulas are under test: the workbook operation also
        // moves or deletes the data itself, so it is applied to the detached
        // book too, after the chart has been taken out of it.
        let mut data_only = reader::xlsx::read_reader(Cursor::new(&bytes), true).unwrap();
        let mut detached = data_only
            .sheet_mut(0)
            .unwrap()
            .worksheet_drawing_mut()
            .extended_charts_mut()
            .remove(0);
        detached.adjust_references("Host", &edit).unwrap();
        workbook_edit(&mut data_only);
        data_only
            .sheet_mut(0)
            .unwrap()
            .worksheet_drawing_mut()
            .add_extended_chart(detached);
        let mut actual = Vec::new();
        writer::xlsx::write_writer(&data_only, &mut actual).unwrap();
        assert_eq!(chart_part(&actual), chart_part(&expected), "{edit:?}");
    }
}

fn rich_chart() -> ExtendedChart {
    use umya_spreadsheet::drawing::spreadsheet::{
        DrawingObject,
        ExtendedChartCompatibility,
    };
    let xml = r#"<cx:chartSpace xmlns:cx="http://schemas.microsoft.com/office/drawing/2014/chartex"><cx:chartData><cx:data id="0"><cx:strDim type="cat"><cx:f>Data!$A$2:$A$4</cx:f></cx:strDim><cx:numDim type="val"><cx:f>Data!$B$2:$B$4</cx:f></cx:numDim></cx:data><cx:data id="1"><cx:numDim type="val"><cx:f>Data!$2:$4</cx:f></cx:numDim></cx:data><cx:data id="2"><cx:numDim type="val"><cx:f>Data!$B:$B</cx:f></cx:numDim></cx:data><cx:data id="3"><cx:numDim type="val"><cx:f>$C$2:$C$4</cx:f></cx:numDim></cx:data><cx:data id="4"><cx:numDim type="val"><cx:f>_xlchart.v1.0</cx:f></cx:numDim></cx:data></cx:chartData><cx:chart><cx:plotArea><cx:plotAreaRegion><cx:series layoutId="waterfall"><cx:tx><cx:txData><cx:f>Data!$B$1</cx:f><cx:v>Sales</cx:v></cx:txData></cx:tx><cx:dataId val="0"/></cx:series><cx:series layoutId="waterfall"><cx:dataId val="1"/></cx:series><cx:series layoutId="waterfall"><cx:dataId val="2"/></cx:series><cx:series layoutId="waterfall"><cx:dataId val="3"/></cx:series><cx:series layoutId="waterfall"><cx:dataId val="4"/></cx:series></cx:plotAreaRegion></cx:plotArea></cx:chart></cx:chartSpace>"#;
    let mut standard = umya_spreadsheet::Chart::default();
    standard.new_chart(
        &umya_spreadsheet::ChartType::LineChart,
        Default::default(),
        Default::default(),
        vec!["Data!$B$2:$B$4"],
    );
    let frame = standard.two_cell_anchor().graphic_frame().unwrap().clone();
    let mut chart = ExtendedChart::new(
        Default::default(),
        ChartExSpace::from_xml(xml.as_bytes()).unwrap(),
    );
    chart.set_compatibility(Some(ExtendedChartCompatibility {
        required_namespaces: vec![(
            "cx1".into(),
            "http://schemas.microsoft.com/office/drawing/2015/9/8/chartex".into(),
        )],
        fallback: Some(Box::new(DrawingObject::GraphicFrame(Box::new(frame)))),
        fallback_present: true,
        ..Default::default()
    }));
    chart
}

fn chart_parts(bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
    use std::io::Read;
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut parts = Vec::new();
    for index in 0..archive.len() {
        let mut file = archive.by_index(index).unwrap();
        let name = file.name().unwrap().into_owned();
        if name.starts_with("xl/charts/") || name.starts_with("xl/drawings/drawing") {
            let mut data = Vec::new();
            file.read_to_end(&mut data).unwrap();
            parts.push((name, data));
        }
    }
    parts.sort();
    parts
}

#[test]
fn detached_edits_agree_with_the_workbook_for_every_formula_form_and_frame_branch() {
    type WorkbookEdit = fn(&mut umya_spreadsheet::Workbook);
    let cases: Vec<(ChartReferenceEdit, WorkbookEdit)> = vec![
        (
            ChartReferenceEdit::RenameSheet {
                old: "Data".into(),
                new: "Renamed Data".into(),
            },
            |book| book.set_sheet_name(1, "Renamed Data").unwrap(),
        ),
        (
            ChartReferenceEdit::DeleteSheet {
                sheet:       "Data".into(),
                sheet_order: vec!["Host".into(), "Data".into(), "Other".into()],
            },
            |book| book.remove_sheet_by_name("Data").unwrap(),
        ),
        (
            ChartReferenceEdit::InsertRows {
                sheet: "Data".into(),
                at:    3,
                count: 2,
            },
            |book| book.insert_new_row("Data", 3, 2),
        ),
        (
            ChartReferenceEdit::InsertRows {
                sheet: "Host".into(),
                at:    3,
                count: 1,
            },
            |book| book.insert_new_row("Host", 3, 1),
        ),
        (
            ChartReferenceEdit::InsertColumns {
                sheet: "Data".into(),
                at:    1,
                count: 1,
            },
            |book| book.insert_new_column_by_index("Data", 1, 1),
        ),
        (
            ChartReferenceEdit::DeleteRows {
                sheet: "Data".into(),
                rows:  vec![3],
            },
            |book| book.remove_row("Data", 3, 1),
        ),
        (
            ChartReferenceEdit::DeleteRows {
                sheet: "Host".into(),
                rows:  vec![2],
            },
            |book| book.remove_row("Host", 2, 1),
        ),
        (
            ChartReferenceEdit::DeleteColumns {
                sheet:   "Data".into(),
                columns: vec![2],
            },
            |book| book.remove_column_by_index("Data", 2, 1),
        ),
    ];
    let mut source = umya_spreadsheet::new_file();
    source.set_sheet_name(0, "Host").unwrap();
    source.new_sheet("Data").unwrap();
    source.new_sheet("Other").unwrap();
    for row in 1..=4 {
        for (sheet, column, value) in [(0, 3, 7), (1, 1, 3), (1, 2, 11)] {
            source
                .sheet_mut(sheet)
                .unwrap()
                .cell_mut((column, row))
                .set_value_number(row * value);
        }
        source
            .sheet_mut(2)
            .unwrap()
            .cell_mut((1, row))
            .set_value_number(row * 5);
    }
    let mut name = DefinedName::default();
    name.set_name("_xlchart.v1.0")
        .set_address("Other!$A$1:$A$3");
    source.add_defined_names(name);
    source
        .sheet_mut(0)
        .unwrap()
        .worksheet_drawing_mut()
        .add_extended_chart(rich_chart());
    let mut bytes = Vec::new();
    writer::xlsx::write_writer(&source, &mut bytes).unwrap();
    for (edit, workbook_edit) in cases {
        let mut in_workbook = reader::xlsx::read_reader(Cursor::new(&bytes), true).unwrap();
        workbook_edit(&mut in_workbook);
        let mut expected = Vec::new();
        writer::xlsx::write_writer(&in_workbook, &mut expected).unwrap();

        let mut detached_book = reader::xlsx::read_reader(Cursor::new(&bytes), true).unwrap();
        let mut detached = detached_book
            .sheet_mut(0)
            .unwrap()
            .worksheet_drawing_mut()
            .extended_charts_mut()
            .remove(0);
        detached.adjust_references("Host", &edit).unwrap();
        workbook_edit(&mut detached_book);
        detached_book
            .sheet_mut(0)
            .unwrap()
            .worksheet_drawing_mut()
            .add_extended_chart(detached);
        let mut actual = Vec::new();
        writer::xlsx::write_writer(&detached_book, &mut actual).unwrap();
        let (actual, expected) = (chart_parts(&actual), chart_parts(&expected));
        assert_eq!(
            actual.iter().map(|part| &part.0).collect::<Vec<_>>(),
            expected.iter().map(|part| &part.0).collect::<Vec<_>>(),
            "{edit:?}"
        );
        for (actual, expected) in actual.iter().zip(&expected) {
            assert_eq!(
                String::from_utf8_lossy(&actual.1),
                String::from_utf8_lossy(&expected.1),
                "{edit:?}: {}",
                actual.0
            );
        }
    }
}
