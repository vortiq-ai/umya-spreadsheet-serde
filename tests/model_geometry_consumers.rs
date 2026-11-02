#![cfg(feature = "serde")]

use std::{
    io::{
        Cursor,
        Read,
    },
    path::PathBuf,
};

use serde_json::Value;
use umya_spreadsheet::{
    DefinedName,
    EnumTrait,
    SlicerCacheExtension,
    Table,
    TableColumn,
    Workbook,
    reader,
    writer,
};

fn read(bytes: &[u8], eager: bool) -> Workbook {
    let mut book = reader::xlsx::read_reader(Cursor::new(bytes), eager).unwrap();
    book.try_read_sheet_collection().unwrap();
    book
}

fn known_answers(family: &str) -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
        "tests/fixtures/model_operations/{family}-known-answers.json"
    ));
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

fn table_geometry(table: &Table) -> Value {
    serde_json::json!([
        format!("{}:{}", table.area().0, table.area().1),
        table.auto_filter().range().range(),
        table.totals_row_count(),
        table.totals_row_shown(),
        table
            .columns()
            .iter()
            .map(|column| serde_json::json!([
                column.id(),
                column.name(),
                column.totals_row_label(),
                column.totals_row_function().value_string(),
            ]))
            .collect::<Vec<_>>()
    ])
}

#[test]
fn retaining_a_blank_data_row_refuses_a_cut_through_another_table() {
    let expected = &known_answers("table")["sideBySide"];
    assert_eq!(expected["refused"], true);
    for eager in [false, true] {
        for worksheet in [false, true] {
            let mut book = read(
                include_bytes!("fixtures/model_operations/geometry-table.xlsx"),
                eager,
            );
            let sheet = book.sheet_by_name_mut("Data").unwrap();
            let mut table = Table::new("Tall", ("H5", "J15"));
            for (column, name) in [(8, "ExtraA"), (9, "ExtraB"), (10, "ExtraC")] {
                let mut model = TableColumn::new(name);
                model.set_id(column - 7);
                table.add_column(model);
                sheet.cell_mut((column, 5)).set_value(name);
                for row in 6..=15 {
                    sheet
                        .cell_mut((column, row))
                        .set_value_number(row * 10 + column);
                }
            }
            sheet.add_table(table);
            let before = format!("{book:?}");
            if worksheet {
                book.sheet_by_name_mut("Data").unwrap().remove_row(6, 4);
            } else {
                book.remove_row("Data", 6, 4);
            }
            assert_eq!(format!("{book:?}"), before);
            let check = |book: &Workbook, written: bool| {
                let sheet = book.sheet_by_name("Data").unwrap();
                let mut states = expected["states"].clone();
                if written {
                    for table in states.as_array_mut().unwrap() {
                        for column in table[4].as_array_mut().unwrap() {
                            if column[2].is_null() {
                                column[2] = Value::String(String::new());
                            }
                        }
                    }
                }
                assert_eq!(
                    serde_json::json!(
                        sheet
                            .tables()
                            .iter()
                            .map(table_geometry)
                            .collect::<Vec<_>>()
                    ),
                    states
                );
                for cell in expected["cells"].as_array().unwrap() {
                    let actual =
                        sheet
                            .cell(cell[0].as_str().unwrap())
                            .map_or_else(String::new, |model| {
                                if model.is_formula() {
                                    model.formula().to_owned()
                                } else {
                                    model.value().to_string()
                                }
                            });
                    assert_eq!(actual, cell[1].as_str().unwrap());
                }
            };
            check(&book, false);
            for light in [false, true] {
                let output = write(&book, light);
                check(&read(&output, true), true);
                keep_output(
                    &format!("side-by-side-refused-{worksheet}-{eager}-{light}"),
                    &output,
                );
            }
        }
    }
}

#[test]
fn retained_pivot_cache_keeps_the_adjusted_source_after_whole_pivot_removal() {
    let expected = &known_answers("pivot")["sourceResult"];
    assert!(expected["refused"].is_null());
    for eager in [false, true] {
        let mut book = read(
            include_bytes!("fixtures/slicers/excel-authored/pivot-slicers.xlsx"),
            eager,
        );
        let mut pivots =
            std::mem::take(book.sheet_by_name_mut("Report").unwrap().pivot_tables_mut());
        for pivot in &mut pivots {
            let reference = if pivot.pivot_table_definition().name() == "PivotTable1" {
                "Q3:R5"
            } else {
                "H3:I6"
            };
            pivot
                .pivot_table_definition_mut()
                .location_mut()
                .set_reference(reference);
        }
        book.sheet_by_name_mut("Data")
            .unwrap()
            .pivot_tables_mut()
            .extend(pivots);
        for cache in book.slicer_caches_mut() {
            if let Some(links) = &mut cache.pivot_tables {
                for link in links {
                    link.sheet_id = 1;
                }
            }
            for extension in &mut cache.extensions {
                if let SlicerCacheExtension::PivotTables { tables, .. } = extension {
                    for link in tables {
                        link.sheet_id = 1;
                    }
                }
            }
        }
        book.remove_row("Data", 3, 4);
        let check = |book: &Workbook| {
            assert_eq!(
                book.sheet_by_name("Data").unwrap().pivot_tables().len() as u64,
                expected["remainingPivots"].as_u64().unwrap()
            );
            let retained: Vec<_> = book.retained_pivot_caches().collect();
            assert_eq!(retained.len(), 1);
            let source = retained[0].1.cache_source().worksheet_source().unwrap();
            assert_eq!(
                source.address().sheet_name(),
                expected["sourceAfter"]["sheet"].as_str().unwrap()
            );
            assert_eq!(
                source.address().range().range(),
                expected["sourceAfter"]["reference"].as_str().unwrap()
            );
        };
        check(&book);
        for light in [false, true] {
            let output = write(&book, light);
            check(&read(&output, true));
            keep_output(&format!("retained-source-{eager}-{light}"), &output);
        }
    }
}

fn chart_state(xml: &str) -> Value {
    use quick_xml::events::Event;
    assert!(!xml.contains("pivotSource"));
    assert!(!xml.contains("numCache"));
    assert!(!xml.contains("strCache"));
    let mut reader = quick_xml::Reader::from_str(xml);
    let mut stack: Vec<Vec<u8>> = Vec::new();
    let mut series = Vec::new();
    loop {
        match reader.read_event().unwrap() {
            Event::Start(element) if element.local_name().as_ref() == b"v" => {
                let value = reader
                    .read_text(element.name())
                    .unwrap()
                    .decode()
                    .unwrap()
                    .into_owned();
                if let Some(current) = series.last_mut() {
                    let current: &mut Value = current;
                    if stack.iter().any(|name| name == b"tx") {
                        current["name"] = value.into();
                    } else if stack.iter().any(|name| name == b"cat") {
                        current["categories"]
                            .as_array_mut()
                            .unwrap()
                            .push(value.into());
                    } else if stack.iter().any(|name| name == b"val") {
                        current["values"]
                            .as_array_mut()
                            .unwrap()
                            .push(serde_json::from_str(&value).unwrap());
                    }
                }
            }
            Event::Start(element) => {
                assert_ne!(element.local_name().as_ref(), b"f");
                if element.local_name().as_ref() == b"ser" {
                    series.push(serde_json::json!({"name":"", "values":[], "categories":[]}));
                }
                stack.push(element.local_name().as_ref().to_vec());
            }
            Event::End(_) => {
                stack.pop();
            }
            Event::Eof => break,
            _ => (),
        }
    }
    serde_json::json!({"pivotLinked":false, "series":series})
}

#[test]
fn whole_pivot_row_and_column_removal_freezes_its_chart_consumers_as_excel_does() {
    for expected in known_answers("pivot")["chartConsumers"].as_array().unwrap() {
        for eager in [false, true] {
            let mut book = read(
                include_bytes!("fixtures/chart_space_parts/pivotchart.xlsx"),
                eager,
            );
            let chart = book.sheet_by_name("Pivot").unwrap().chart_collection()[0].clone();
            book.new_sheet("Charts").unwrap().add_chart(chart);
            if expected["operation"] == "rows" {
                book.remove_row("Pivot", 3, 6);
            } else {
                book.remove_column("Pivot", "A", 4);
            }
            assert_eq!(
                book.sheet_by_name("Pivot").unwrap().pivot_tables().len() as u64,
                expected["remainingPivots"].as_u64().unwrap()
            );
            for light in [false, true] {
                let output = write(&book, light);
                for bytes in [&output, &write(&read(&output, true), light)] {
                    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
                    let mut states = Vec::new();
                    for index in 0..zip.len() {
                        let mut part = zip.by_index(index).unwrap();
                        let name = part.name().unwrap();
                        if name.starts_with("xl/charts/chart") && name.ends_with(".xml") {
                            let mut xml = String::new();
                            part.read_to_string(&mut xml).unwrap();
                            states.push(chart_state(&xml));
                        }
                    }
                    let mut wanted = vec![expected["original"].clone(), expected["copied"].clone()];
                    states.sort_by_key(Value::to_string);
                    wanted.sort_by_key(Value::to_string);
                    assert_eq!(states, wanted);
                }
                keep_output(
                    &format!(
                        "pivot-chart-{}-{eager}-{light}",
                        expected["operation"].as_str().unwrap()
                    ),
                    &output,
                );
            }
        }
    }
}

#[test]
fn moved_and_combo_pivot_charts_preserve_excel_series_names_when_the_pivot_is_removed() {
    for expected in known_answers("pivot")["chartNaming"].as_array().unwrap() {
        assert!(expected["comboError"].is_null());
        let scenario = expected["scenario"].as_str().unwrap();
        let input = match scenario {
            "combo" => include_bytes!("fixtures/model_operations/pivot-combo-base.xlsx").as_slice(),
            "moved" => include_bytes!("fixtures/model_operations/pivot-moved-base.xlsx").as_slice(),
            _ => panic!("unmeasured chart scenario {scenario}"),
        };
        let wanted: Vec<_> = expected["after"].as_array().unwrap().iter().map(|chart| {
            let series: Vec<_> = chart["series"].as_array().unwrap().iter().map(|series| {
                serde_json::json!({"name":series["name"],"values":series["values"],"categories":series["categories"]})
            }).collect();
            serde_json::json!({"pivotLinked":chart["pivotLinked"],"series":series})
        }).collect();
        for eager in [false, true] {
            let mut book = read(input, eager);
            let host = if scenario == "combo" {
                "Pivot"
            } else {
                "Charts"
            };
            assert_eq!(
                book.sheet_by_name(host).unwrap().chart_collection()[0]
                    .chart_space()
                    .chart()
                    .plot_area()
                    .chart_groups()
                    .count() as u64,
                expected["before"][0]["groups"].as_u64().unwrap()
            );
            if expected["operation"] == "rows" {
                book.remove_row("Pivot", 3, 6);
            } else {
                book.remove_column("Pivot", "A", 4);
            }
            assert!(
                book.sheet_by_name("Pivot")
                    .unwrap()
                    .pivot_tables()
                    .is_empty()
            );
            for light in [false, true] {
                let output = write(&book, light);
                for bytes in [&output, &write(&read(&output, true), light)] {
                    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
                    let mut states = Vec::new();
                    for index in 0..zip.len() {
                        let mut part = zip.by_index(index).unwrap();
                        let name = part.name().unwrap();
                        if name.starts_with("xl/charts/chart") && name.ends_with(".xml") {
                            let mut xml = String::new();
                            part.read_to_string(&mut xml).unwrap();
                            states.push(chart_state(&xml));
                        }
                    }
                    assert_eq!(states, wanted, "{scenario}: {}", expected["operation"]);
                }
                keep_output(
                    &format!(
                        "pivot-chart-naming-{scenario}-{}-{eager}-{light}",
                        expected["operation"].as_str().unwrap()
                    ),
                    &output,
                );
            }
        }
    }
}

#[test]
fn pivot_charts_are_frozen_before_deleting_a_preceding_chart_anchor() {
    let expected = known_answers("pivot")["chartConsumers"][0]["original"].clone();
    let mut shorter = expected.clone();
    shorter["series"].as_array_mut().unwrap().truncate(1);
    for eager in [false, true] {
        let mut book = read(
            include_bytes!("fixtures/chart_space_parts/pivotchart.xlsx"),
            eager,
        );
        let sheet = book.sheet_by_name_mut("Pivot").unwrap();
        let mut pivot_chart = sheet.chart_collection_mut().remove(0);
        pivot_chart
            .two_cell_anchor_mut()
            .from_marker_mut()
            .set_coordinate("F3");
        pivot_chart
            .two_cell_anchor_mut()
            .to_marker_mut()
            .set_coordinate("K12");
        let mut ordinary = pivot_chart.clone();
        ordinary.chart_space_mut().remove_pivot_source();
        ordinary
            .two_cell_anchor_mut()
            .from_marker_mut()
            .set_coordinate("A13");
        ordinary
            .two_cell_anchor_mut()
            .to_marker_mut()
            .set_coordinate("B15");
        let mut second = pivot_chart.clone();
        let first_series = second.area_chart_series_list_mut().area_chart_series()[0].clone();
        second
            .area_chart_series_list_mut()
            .set_area_chart_series(vec![first_series]);
        second
            .two_cell_anchor_mut()
            .from_marker_mut()
            .set_coordinate("N3");
        second
            .two_cell_anchor_mut()
            .to_marker_mut()
            .set_coordinate("S12");
        sheet.add_chart(ordinary);
        sheet.add_chart(pivot_chart);
        sheet.add_chart(second);
        book.remove_column("Pivot", "A", 5);
        assert!(
            book.sheet_by_name("Pivot")
                .unwrap()
                .pivot_tables()
                .is_empty()
        );
        assert_eq!(
            book.sheet_by_name("Pivot")
                .unwrap()
                .chart_collection()
                .len(),
            2
        );
        for light in [false, true] {
            let output = write(&book, light);
            for bytes in [&output, &write(&read(&output, true), light)] {
                let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
                let mut states = Vec::new();
                for index in 0..zip.len() {
                    let mut part = zip.by_index(index).unwrap();
                    let name = part.name().unwrap();
                    if name.starts_with("xl/charts/chart") && name.ends_with(".xml") {
                        let mut xml = String::new();
                        part.read_to_string(&mut xml).unwrap();
                        states.push(chart_state(&xml));
                    }
                }
                let mut wanted = vec![expected.clone(), shorter.clone()];
                states.sort_by_key(Value::to_string);
                wanted.sort_by_key(Value::to_string);
                assert_eq!(states, wanted);
            }
            keep_output(
                &format!("pivot-charts-preceding-anchor-{eager}-{light}"),
                &output,
            );
        }
    }
}

fn write(book: &Workbook, light: bool) -> Vec<u8> {
    let mut output = Vec::new();
    if light {
        writer::xlsx::write_writer_light(book, &mut output).unwrap();
    } else {
        writer::xlsx::write_writer(book, &mut output).unwrap();
    }
    output
}

fn keep_output(name: &str, output: &[u8]) {
    let directory =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/model-operations/consumers");
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join(format!("{name}.xlsx")), output).unwrap();
}

fn names(book: &Workbook) -> Vec<String> {
    let mut names: Vec<_> = book
        .defined_names()
        .iter()
        .chain(
            book.sheet_collection_no_check()
                .iter()
                .flat_map(|sheet| sheet.defined_names()),
        )
        .map(|name| name.name().to_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn whole_table_row_and_column_removal_removes_its_slicer_consumers() {
    let input = include_bytes!("fixtures/slicers/excel-authored/table-slicer.xlsx");
    for eager in [false, true] {
        for rows in [false, true] {
            let mut book = read(input, eager);
            assert_eq!(
                book.sheet_by_name("Data").unwrap().tables()[0].name(),
                "Scores"
            );
            assert_eq!(book.slicer_caches().len(), 1);
            assert!(names(&book).iter().any(|name| name == "Slicer_Team"));
            if rows {
                book.remove_row("Data", 1, 7);
            } else {
                book.remove_column("Data", "A", 4);
            }
            let check = |book: &Workbook| {
                let sheet = book.sheet_by_name("Data").unwrap();
                assert!(sheet.tables().is_empty());
                assert!(sheet.slicers().is_empty());
                assert!(sheet.worksheet_drawing().slicer_frames().is_empty());
                assert!(book.slicer_caches().is_empty());
                assert!(names(book).iter().all(|name| name != "Slicer_Team"));
            };
            check(&book);
            for light in [false, true] {
                let output = write(&book, light);
                for reread_eager in [false, true] {
                    check(&read(&output, reread_eager));
                }
                keep_output(&format!("table-removed-{rows}-{eager}-{light}"), &output);
            }
        }
    }
}

#[test]
fn worksheet_whole_model_removal_with_consumers_is_refused_atomically() {
    for (fixture, host, input, row_count, columns) in [
        (
            "table",
            "Data",
            include_bytes!("fixtures/slicers/excel-authored/table-slicer.xlsx").as_slice(),
            7,
            4,
        ),
        (
            "slicer",
            "Report",
            include_bytes!("fixtures/slicers/excel-authored/pivot-slicers.xlsx").as_slice(),
            6,
            9,
        ),
        (
            "timeline",
            "Report",
            include_bytes!("fixtures/slicers/excel-authored/timeline.xlsx").as_slice(),
            6,
            9,
        ),
        (
            "chart",
            "Pivot",
            include_bytes!("fixtures/chart_space_parts/pivotchart.xlsx").as_slice(),
            8,
            4,
        ),
    ] {
        for eager in [false, true] {
            for rows in [false, true] {
                let mut book = read(input, eager);
                let before = format!("{book:?}");
                let sheet = book.sheet_by_name_mut(host).unwrap();
                if rows {
                    sheet.remove_row(1, row_count);
                } else {
                    sheet.remove_column_by_index(1, columns);
                }
                assert_eq!(format!("{book:?}"), before);
                for light in [false, true] {
                    let output = write(&book, light);
                    let again = read(&output, true);
                    assert_eq!(again.slicer_caches().len(), book.slicer_caches().len());
                    assert_eq!(again.timeline_caches().len(), book.timeline_caches().len());
                    assert_eq!(
                        again.sheet_by_name(host).unwrap().tables().len(),
                        book.sheet_by_name(host).unwrap().tables().len()
                    );
                    assert_eq!(
                        again.sheet_by_name(host).unwrap().pivot_tables().len(),
                        book.sheet_by_name(host).unwrap().pivot_tables().len()
                    );
                    assert_eq!(
                        again.sheet_by_name(host).unwrap().chart_collection().len(),
                        book.sheet_by_name(host).unwrap().chart_collection().len()
                    );
                    keep_output(
                        &format!(
                            "worksheet-model-removal-refused-{fixture}-{rows}-{eager}-{light}"
                        ),
                        &output,
                    );
                }
            }
        }
    }
}

fn slicer_sources(book: &Workbook) -> Vec<(String, Value)> {
    book.slicer_caches()
        .iter()
        .map(|cache| {
            (
                cache.name.clone(),
                serde_json::to_value(&cache.source).unwrap(),
            )
        })
        .collect()
}

fn timeline_states(book: &Workbook) -> Vec<(String, Value)> {
    book.timeline_caches()
        .iter()
        .map(|cache| {
            (
                cache.name.clone(),
                serde_json::to_value(&cache.state).unwrap(),
            )
        })
        .collect()
}

fn views(book: &Workbook) -> Vec<(String, Value, Value)> {
    book.sheet_collection_no_check()
        .iter()
        .map(|sheet| {
            (
                sheet.name().to_owned(),
                serde_json::to_value(sheet.slicers()).unwrap(),
                serde_json::to_value(sheet.timelines()).unwrap(),
            )
        })
        .collect()
}

#[test]
fn whole_pivot_row_removal_disconnects_and_retains_its_slicer_and_timeline_consumers() {
    check_whole_pivot_removal_consumers(true);
}

#[test]
fn whole_pivot_column_removal_disconnects_and_retains_its_slicer_and_timeline_consumers() {
    check_whole_pivot_removal_consumers(false);
}

fn check_whole_pivot_removal_consumers(rows: bool) {
    // Excel keeps these consumer caches, selections and views after the last
    // pivot table is deleted. The corresponding Excel-authored orphan-cache
    // fixtures also pin their retained cache definitions and records.
    for (fixture, input, columns) in [
        (
            "slicer",
            include_bytes!("fixtures/slicers/excel-authored/pivot-slicers.xlsx").as_slice(),
            9,
        ),
        (
            "timeline",
            include_bytes!("fixtures/slicers/excel-authored/timeline.xlsx").as_slice(),
            9,
        ),
    ] {
        let answers = known_answers("pivot");
        let sample = if fixture == "slicer" {
            "pivot-slicers"
        } else {
            fixture
        };
        let operation = if rows { "rows" } else { "columns" };
        let expected = answers["consumerOperations"]
            .as_array()
            .unwrap()
            .iter()
            .find(|fact| fact["sample"] == sample && fact["operation"] == operation)
            .unwrap();
        assert!(expected["refused"].is_null());
        let mut expected_caches = expected["before"].clone();
        for cache in expected_caches.as_array_mut().unwrap() {
            cache["pivots"] = serde_json::json!([]);
        }
        assert_eq!(expected["after"], expected_caches);
        for eager in [false, true] {
            let mut book = read(input, eager);
            let report = book.sheet_by_name("Report").unwrap();
            assert!(!report.pivot_tables().is_empty());
            let records = report.pivot_tables()[0]
                .pivot_cache_definition()
                .raw_cache_records()
                .to_vec();
            let before_slicers = slicer_sources(&book);
            let before_timelines = timeline_states(&book);
            let before_views = views(&book);
            let before_names = names(&book);
            if rows {
                book.remove_row("Report", 3, 4);
            } else {
                book.remove_column_by_index("Report", 1, columns);
            }
            let check = |book: &Workbook| {
                assert_eq!(
                    book.sheet_by_name("Report").unwrap().pivot_tables().len() as u64,
                    expected["remainingPivots"].as_u64().unwrap()
                );
                assert!(
                    book.sheet_by_name("Report")
                        .unwrap()
                        .pivot_tables()
                        .is_empty()
                );
                assert_eq!(slicer_sources(book), before_slicers);
                assert_eq!(timeline_states(book), before_timelines);
                assert_eq!(views(book), before_views);
                assert_eq!(names(book), before_names);
                for cache in book.slicer_caches() {
                    assert!(cache.linked_pivots().is_empty());
                }
                for cache in book.timeline_caches() {
                    assert!(cache.linked_pivots().is_empty());
                }
                let retained: Vec<_> = book.retained_pivot_caches().collect();
                assert_eq!(retained.len(), 1);
                assert_eq!(retained[0].1.raw_cache_records(), records);
            };
            check(&book);
            for light in [false, true] {
                let output = write(&book, light);
                for reread_eager in [false, true] {
                    check(&read(&output, reread_eager));
                }
                keep_output(
                    &format!("{fixture}-pivot-removed-{rows}-{eager}-{light}"),
                    &output,
                );
            }
        }
    }
}

fn add_references(book: &mut Workbook, host: &str) {
    book.sheet_by_name_mut(host)
        .unwrap()
        .cell_mut("P30")
        .set_formula("SUM(A8:A20)");
    book.new_sheet("References")
        .unwrap()
        .cell_mut("A1")
        .set_formula(format!("SUM('{host}'!A8:A20)"));
    let mut name = DefinedName::default();
    name.set_name("KeptRange")
        .set_address(format!("'{host}'!$A$8:$A$20"));
    book.defined_names_mut().push(name);
}

#[test]
fn a_later_table_refusal_leaves_all_models_cells_and_references_unchanged() {
    let input = include_bytes!("fixtures/model_operations/geometry-table.xlsx");
    for eager in [false, true] {
        for worksheet in [false, true] {
            let mut book = read(input, eager);
            let mut table = Table::new("LaterTable", ("H8", "J12"));
            for name in ["One", "Two", "Three"] {
                table.add_column(TableColumn::new(name));
            }
            let sheet = book.sheet_by_name_mut("Data").unwrap();
            sheet.add_table(table);
            for (column, name) in [(8, "One"), (9, "Two"), (10, "Three")] {
                sheet.cell_mut((column, 8)).set_value(name);
            }
            add_references(&mut book, "Data");
            let before = format!("{book:?}");
            // Sales permits this interior data-row deletion, but LaterTable
            // refuses deletion of its header. Nothing may be applied first.
            if worksheet {
                book.sheet_by_name_mut("Data").unwrap().remove_row(8, 1);
            } else {
                book.remove_row("Data", 8, 1);
            }
            assert_eq!(format!("{book:?}"), before);
            for light in [false, true] {
                let output = write(&book, light);
                let again = read(&output, true);
                assert_eq!(again.sheet_by_name("Data").unwrap().tables().len(), 2);
                assert_eq!(
                    again
                        .sheet_by_name("References")
                        .unwrap()
                        .cell("A1")
                        .unwrap()
                        .formula(),
                    "SUM('Data'!A8:A20)"
                );
                keep_output(
                    &format!("table-refused-{worksheet}-{eager}-{light}"),
                    &output,
                );
            }
        }
    }
}

#[test]
fn a_later_pivot_refusal_rolls_back_other_pivot_and_table_plans() {
    let input = include_bytes!("fixtures/model_operations/geometry-pivot.xlsx");
    for eager in [false, true] {
        for worksheet in [false, true] {
            for insert in [false, true] {
                let mut book = read(input, eager);
                let sheet = book.sheet_by_name_mut("Pivot").unwrap();
                let mut translated = sheet.pivot_tables()[0].clone();
                translated
                    .pivot_table_definition_mut()
                    .set_name("LaterPivot");
                translated
                    .pivot_table_definition_mut()
                    .location_mut()
                    .set_reference("H20:I23");
                sheet.pivot_tables_mut().insert(0, translated);
                let mut table = Table::new("TranslatedTable", ("A30", "B34"));
                for (column, name) in [(1, "One"), (2, "Two")] {
                    table.add_column(TableColumn::new(name));
                    sheet.cell_mut((column, 30)).set_value(name);
                }
                sheet.add_table(table);
                add_references(&mut book, "Pivot");
                let before = format!("{book:?}");
                // The first pivot would translate; the second refuses a cut
                // through its footprint, including its page fields.
                match (worksheet, insert) {
                    (false, false) => book.remove_row("Pivot", 6, 1),
                    (false, true) => book.insert_new_row("Pivot", 6, 1),
                    (true, false) => book.sheet_by_name_mut("Pivot").unwrap().remove_row(6, 1),
                    (true, true) => book
                        .sheet_by_name_mut("Pivot")
                        .unwrap()
                        .insert_new_row(6, 1),
                }
                assert_eq!(format!("{book:?}"), before);
            }
        }
    }
}
