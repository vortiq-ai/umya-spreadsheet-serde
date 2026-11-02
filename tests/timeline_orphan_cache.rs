#![cfg(feature = "serde")]

#[path = "support/slicer_package_fixture.rs"]
mod fixture;

use std::io::Cursor;

use fixture::{
    attribute,
    keep_output,
    readers,
    save,
    tags,
    unpack,
};
use sha2::{
    Digest,
    Sha256,
};
use umya_spreadsheet::{
    DefinedName,
    PivotCacheSlicerProperties,
    SlicerPivotTable,
    Timeline,
    Workbook,
    drawing::spreadsheet::{
        DrawingAnchor,
        Extent,
        SlicerFrame,
    },
    writer::streaming_writer::StreamingWriter,
};

const FIXTURE: &[u8] = include_bytes!("fixtures/timelines/excel-authored/orphan-pivot-cache.xlsx");
const LIVE_FIXTURE: &[u8] =
    include_bytes!("fixtures/timelines/excel-authored/pivot-without-timeline.xlsx");
const RECORDS_PATH: &str = "xl/pivotCache/pivotCacheRecords1.xml";

fn assert_cache_source_xml(parts: &fixture::Parts, sheet: &str, reference: &str) {
    let cache = parts
        .iter()
        .find(|(path, _)| {
            path.starts_with("xl/pivotCache/pivotCacheDefinition") && path.ends_with(".xml")
        })
        .map(|(_, xml)| xml)
        .unwrap();
    let source = tags(cache, "<worksheetSource").into_iter().next().unwrap();
    assert_eq!(attribute(source, "sheet").as_deref(), Some(sheet));
    assert_eq!(attribute(source, "ref").as_deref(), Some(reference));
}

fn rename_data(book: &mut Workbook, name: &str) {
    book.try_read_sheet_collection().unwrap();
    let index = book
        .sheet_collection()
        .iter()
        .position(|sheet| sheet.name() == "Data")
        .unwrap();
    book.set_sheet_name(index, name).unwrap();
}

fn rename_data_without_materializing(book: &mut Workbook, name: &str) {
    let index = book
        .sheet_collection_no_check()
        .iter()
        .position(|sheet| sheet.name() == "Data")
        .unwrap();
    book.set_sheet_name(index, name).unwrap();
}

fn assert_excel_orphan_cache(book: &Workbook, records: &[u8]) {
    assert!(
        book.sheet_collection_no_check()
            .iter()
            .all(|sheet| sheet.pivot_tables().is_empty())
    );
    let caches: Vec<_> = book.retained_pivot_caches().collect();
    assert_eq!(caches.len(), 1);
    // Excel resaves d2-delete-all-linked-pivots's workbook cache id 11 as 0.
    // Its separate x14 identity, cache contents and timeline state stay intact.
    let (id, definition) = caches[0];
    assert_eq!(id, 0);
    assert_eq!(definition.slicer_cache_id(), Some(953_075_119));
    assert_eq!(definition.raw_cache_records(), records);
    let fields = definition.cache_fields().list();
    assert_eq!(
        fields.iter().map(|field| field.name()).collect::<Vec<_>>(),
        ["Team", "Points", "Day", "Ship"]
    );
    let day_items = fields[2].shared_items().items();
    assert_eq!(
        day_items
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        [
            "2024-01-05T00:00:00",
            "2024-01-20T00:00:00",
            "2024-02-10T00:00:00",
            "2024-05-25T00:00:00",
            "2024-08-14T00:00:00",
            "2025-03-03T00:00:00",
        ]
    );
    assert_eq!(book.timeline_caches().len(), 1);
    let timeline = &book.timeline_caches()[0];
    assert_eq!(timeline.name, "NativeTimeline_Day");
    assert_eq!(timeline.source_name, "Day");
    assert!(timeline.linked_pivots().is_empty());
    assert_eq!(timeline.state.pivot_cache_id, 953_075_119);
    assert_eq!(timeline.state.filter_type, "dateBetween");
    let selection = timeline.state.selection.as_ref().unwrap();
    assert_eq!(selection.start_date, "2024-01-01T00:00:00");
    assert_eq!(selection.end_date, "2024-02-29T00:00:00");
    assert_eq!(timeline.state.bounds.start_date, "2024-01-01T00:00:00");
    assert_eq!(timeline.state.bounds.end_date, "2026-01-01T00:00:00");
    let views = book.sheet_by_name("Report").unwrap().timelines();
    assert_eq!(views.len(), 1);
    assert_eq!(views[0].name, "DayTl");
    assert_eq!(views[0].cache, "NativeTimeline_Day");
}

#[test]
fn a_live_pivot_cache_and_a_retained_timeline_cache_are_written_apart() {
    for (reader, read) in readers() {
        let mut book = read(FIXTURE).unwrap();
        book.try_read_sheet_collection().unwrap();
        let mut source = read(include_bytes!(
            "fixtures/timelines/excel-authored/pivot-without-timeline.xlsx"
        ))
        .unwrap();
        source.try_read_sheet_collection().unwrap();
        let mut pivot = source.sheet_by_name("Report").unwrap().pivot_tables()[0].clone();
        pivot
            .pivot_table_definition_mut()
            .set_name("LivePivot")
            .set_cache_id(7);
        pivot
            .pivot_cache_definition_mut()
            .set_slicer_properties(PivotCacheSlicerProperties {
                // An x14 id distinct from the retained cache's 953075119,
                // set by hand.
                pivot_cache_id: Some(953_075_120),
                ..PivotCacheSlicerProperties::default()
            });
        book.sheet_by_name_mut("Report")
            .unwrap()
            .pivot_tables_mut()
            .push(pivot);
        let mut extent = Extent::default();
        extent.set_cx(3_810_000).set_cy(1_397_000);
        let mut address = DefinedName::default();
        address.set_name("NativeTimeline_Day1");
        address.set_address("#N/A");
        book.defined_names_mut().push(address);
        let mut cache = book.timeline_caches()[0].clone();
        cache.name = "NativeTimeline_Day1".to_owned();
        cache.uid = None;
        cache.pivot_tables = Some(vec![SlicerPivotTable {
            sheet_id: 2,
            name:     "LivePivot".to_owned(),
        }]);
        cache.state.pivot_cache_id = 953_075_120;
        cache.state.selection = None;
        "unknown".clone_into(&mut cache.state.filter_type);
        cache.pivot_filter = None;
        book.timeline_caches_mut().push(cache);
        let report = book.sheet_by_name_mut("Report").unwrap();
        report.timelines_mut().push(Timeline {
            name: "LiveTimeline".to_owned(),
            cache: "NativeTimeline_Day1".to_owned(),
            caption: Some("Day".to_owned()),
            ..Timeline::default()
        });
        report
            .worksheet_drawing_mut()
            .add_slicer_frame(SlicerFrame::new_timeline(
                DrawingAnchor::OneCell {
                    from: Default::default(),
                    extent,
                },
                "LiveTimeline",
            ));
        let normal = save(&book);
        let streamed = stream(book);
        for (writer, bytes) in [("normal", normal), ("streaming", streamed)] {
            let parts = unpack(&bytes);
            let workbook = &parts["xl/workbook.xml"];
            let main_start = workbook.find("<pivotCaches>").unwrap();
            let main_end = workbook.find("</pivotCaches>").unwrap();
            let main = tags(&workbook[main_start..main_end], "<pivotCache");
            assert_eq!(main.len(), 1, "{reader} {writer}");
            assert_eq!(attribute(main[0], "cacheId").as_deref(), Some("7"));
            let orphan_start = workbook.find("<x15:timelineCachePivotCaches>").unwrap();
            let orphan_end = workbook.find("</x15:timelineCachePivotCaches>").unwrap();
            let orphan = tags(&workbook[orphan_start..orphan_end], "<pivotCache");
            assert_eq!(orphan.len(), 1, "{reader} {writer}");
            assert_eq!(attribute(orphan[0], "cacheId").as_deref(), Some("0"));
            for (entry, expected_target) in [
                (main[0], "pivotCache/pivotCacheDefinition1.xml"),
                (orphan[0], "pivotCache/pivotCacheDefinition2.xml"),
            ] {
                let id = attribute(entry, "r:id").unwrap();
                let relationship = tags(&parts["xl/_rels/workbook.xml.rels"], "<Relationship")
                    .into_iter()
                    .find(|tag| attribute(tag, "Id").as_deref() == Some(&id))
                    .unwrap();
                assert_eq!(
                    attribute(relationship, "Target").as_deref(),
                    Some(expected_target),
                    "{reader} {writer}"
                );
            }
            assert_eq!(
                parts["xl/pivotCache/pivotCacheRecords2.xml"],
                unpack(FIXTURE)[RECORDS_PATH]
            );
            for (_, reread) in readers() {
                let mut again = reread(&bytes).unwrap();
                again.try_read_sheet_collection().unwrap();
                assert_eq!(again.retained_pivot_caches().count(), 1);
                let pivots = again.sheet_by_name("Report").unwrap().pivot_tables();
                assert_eq!(pivots.len(), 1);
                assert_eq!(pivots[0].pivot_table_definition().cache_id(), 7);
                assert_eq!(
                    pivots[0].pivot_cache_definition().slicer_cache_id(),
                    Some(953_075_120)
                );
                assert_eq!(again.timeline_caches()[0].state.pivot_cache_id, 953_075_119);
                assert_eq!(again.timeline_caches()[1].state.pivot_cache_id, 953_075_120);
            }
            keep_output(&format!("timeline-mixed-cache-{reader}-{writer}"), &bytes);
        }
    }
}

fn stream(book: Workbook) -> Vec<u8> {
    let names: Vec<_> = book
        .sheet_collection_no_check()
        .iter()
        .map(|sheet| sheet.name().to_owned())
        .collect();
    let mut writer = StreamingWriter::new(zip::ZipWriter::new(Cursor::new(Vec::new())), book);
    for name in names {
        let sheet = writer.take_sheet(&name).unwrap();
        writer.flush_sheet(sheet).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

#[test]
fn excels_timeline_without_a_pivot_table_keeps_its_definition_and_records() {
    let manifest: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/timelines/excel-authored/manifest.json"
    ))
    .unwrap();
    assert_eq!(
        Sha256::digest(FIXTURE)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>(),
        manifest["orphan-pivot-cache"]["workbook_sha256"]
            .as_str()
            .unwrap()
    );
    let fixture_parts = unpack(FIXTURE);
    let records = fixture_parts[RECORDS_PATH].as_bytes();
    assert_eq!(
        Sha256::digest(records)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>(),
        "adc3b0bd8eedd619595b45147fefdc675407b5b1ce2c76f200e4f5454a3e9399"
    );
    for (reader, read) in readers() {
        let mut book = read(FIXTURE).unwrap();
        book.try_read_sheet_collection().unwrap();
        assert_excel_orphan_cache(&book, records);
        let normal = save(&book);
        let streamed = stream(book);
        for (writer, bytes) in [("normal", normal), ("streaming", streamed)] {
            let parts = unpack(&bytes);
            assert_eq!(parts[RECORDS_PATH].as_bytes(), records, "{reader} {writer}");
            let workbook = &parts["xl/workbook.xml"];
            assert!(!workbook.contains("<pivotCaches>"));
            assert!(workbook.contains("<x15:timelineCachePivotCaches>"));
            let cache = tags(workbook, "<pivotCache");
            assert_eq!(cache.len(), 1);
            assert_eq!(attribute(cache[0], "cacheId").as_deref(), Some("0"));
            assert!(
                parts["xl/pivotCache/_rels/pivotCacheDefinition1.xml.rels"]
                    .contains("pivotCacheRecords1.xml")
            );
            assert!(parts["[Content_Types].xml"].contains("pivotCacheRecords+xml"));
            assert!(
                parts
                    .keys()
                    .all(|path| !path.starts_with("xl/pivotTables/"))
            );
            for (_, reread) in readers() {
                let mut again = reread(&bytes).unwrap();
                again.try_read_sheet_collection().unwrap();
                assert_excel_orphan_cache(&again, records);
            }
            keep_output(&format!("timeline-orphan-cache-{reader}-{writer}"), &bytes);
        }
    }
}

#[test]
fn retained_pivot_cache_sources_follow_sheet_renames_for_every_reader_and_writer() {
    for (reader, read) in readers() {
        let mut book = read(FIXTURE).unwrap();
        rename_data(&mut book, "Data Renamed");

        let normal = save(&book);
        let streamed = stream(book);
        for (writer, bytes) in [("normal", normal), ("streaming", streamed)] {
            let parts = unpack(&bytes);
            assert_cache_source_xml(&parts, "Data Renamed", "A1:D7");

            for (_, reread) in readers() {
                let mut again = reread(&bytes).unwrap();
                again.try_read_sheet_collection().unwrap();
                let (_, definition) = again.retained_pivot_caches().next().unwrap();
                let source = definition.cache_source().worksheet_source().unwrap();
                assert_eq!(source.address().sheet_name(), "Data Renamed");
                assert_eq!(source.address().range().range(), "A1:D7");
            }
            keep_output(&format!("orphan-cache-renamed-{reader}-{writer}"), &bytes);
        }
    }
}

#[test]
fn retained_pivot_cache_source_range_expands_when_rows_are_inserted() {
    for (reader, read) in readers() {
        let mut book = read(FIXTURE).unwrap();
        book.insert_new_row("Data", 4, 1);

        let normal = save(&book);
        let streamed = stream(book);
        for (writer, bytes) in [("normal", normal), ("streaming", streamed)] {
            let parts = unpack(&bytes);
            assert_cache_source_xml(&parts, "Data", "A1:D8");

            for (_, reread) in readers() {
                let mut again = reread(&bytes).unwrap();
                again.try_read_sheet_collection().unwrap();
                let (_, definition) = again.retained_pivot_caches().next().unwrap();
                let source = definition.cache_source().worksheet_source().unwrap();
                assert_eq!(source.address().sheet_name(), "Data");
                assert_eq!(source.address().range().range(), "A1:D8");
            }
            keep_output(
                &format!("orphan-cache-row-insert-{reader}-{writer}"),
                &bytes,
            );
        }
    }
}

#[test]
fn live_pivot_cache_sources_follow_the_same_reference_edits() {
    for (reader, read) in readers() {
        for (edit, expected_sheet, expected_ref) in [
            ("rename", "Data Renamed", "A1:C3"),
            ("row-insert", "Data", "A1:C4"),
        ] {
            let mut book = read(LIVE_FIXTURE).unwrap();
            match edit {
                "rename" => rename_data_without_materializing(&mut book, expected_sheet),
                "row-insert" => book.insert_new_row("Data", 3, 1),
                _ => unreachable!(),
            }
            let normal = save(&book);
            let streamed = stream(book);
            for (writer, bytes) in [("normal", normal), ("streaming", streamed)] {
                assert_cache_source_xml(&unpack(&bytes), expected_sheet, expected_ref);
                for (_, reread) in readers() {
                    let mut again = reread(&bytes).unwrap();
                    again.try_read_sheet_collection().unwrap();
                    let source = again.sheet_by_name("Report").unwrap().pivot_tables()[0]
                        .pivot_cache_definition()
                        .cache_source()
                        .worksheet_source()
                        .unwrap();
                    assert_eq!(source.address().sheet_name(), expected_sheet);
                    assert_eq!(source.address().range().range(), expected_ref);
                }
                keep_output(&format!("live-cache-{edit}-{reader}-{writer}"), &bytes);
            }
        }
    }
}

fn saved_source(bytes: &[u8]) -> (String, String) {
    let parts = unpack(bytes);
    let cache = parts
        .iter()
        .find(|(path, _)| {
            path.starts_with("xl/pivotCache/pivotCacheDefinition") && path.ends_with(".xml")
        })
        .map(|(_, xml)| xml)
        .unwrap();
    let source = tags(cache, "<worksheetSource").into_iter().next().unwrap();
    (
        attribute(source, "sheet").unwrap(),
        attribute(source, "ref").unwrap(),
    )
}

/// The source sheet and range of the cache of the pivot table on `host`, or
/// of the retained cache without one.
fn model_source(book: &Workbook, host: Option<&str>) -> (String, String) {
    let source = match host {
        Some(sheet) => book.sheet_by_name(sheet).unwrap().pivot_tables()[0]
            .pivot_cache_definition()
            .cache_source()
            .worksheet_source()
            .unwrap(),
        None => book
            .retained_pivot_caches()
            .next()
            .unwrap()
            .1
            .cache_source()
            .worksheet_source()
            .unwrap(),
    };
    (
        source.address().sheet_name().to_owned(),
        source.address().range().range(),
    )
}

/// Excel (deleting `Data`, rows 1:N or columns A:last of the source, then
/// saving) writes `<worksheetSource ref="A1:.." sheet="Data"/>` unchanged; a
/// deleted sheet only gains a link to the workbook's previous file.
#[test]
fn deleting_the_source_sheet_or_its_whole_range_leaves_the_cache_source_as_written() {
    for (fixture, host, cache_ref) in [
        (FIXTURE, None, "A1:D7"),
        (LIVE_FIXTURE, Some("Report"), "A1:C3"),
    ] {
        let rows = if host.is_some() { 3 } else { 7 };
        let columns = if host.is_some() { 3 } else { 4 };
        for (reader, read) in readers() {
            for edit in ["sheet", "rows", "columns"] {
                let mut book = read(fixture).unwrap();
                match edit {
                    "sheet" => book.remove_sheet_by_name("Data").unwrap(),
                    "rows" => book.remove_row("Data", 1, rows),
                    _ => book.remove_column("Data", "A", columns),
                }
                let normal = save(&book);
                let streamed = stream(book);
                for (writer, bytes) in [("normal", normal), ("streaming", streamed)] {
                    let label = format!("{reader} {writer} {edit}");
                    assert_eq!(
                        saved_source(&bytes),
                        ("Data".to_owned(), cache_ref.to_owned()),
                        "{label}"
                    );
                    if edit == "sheet" {
                        // Report was the second tab; Excel writes no activeTab
                        // (the first tab) once Data is gone.
                        let parts = unpack(&bytes);
                        let view = tags(&parts["xl/workbook.xml"], "<workbookView");
                        assert_eq!(
                            attribute(view[0], "activeTab").as_deref(),
                            Some("0"),
                            "{label}"
                        );
                    }
                    for (_, reread) in readers() {
                        let mut again = reread(&bytes).unwrap();
                        again.try_read_sheet_collection().unwrap();
                        assert_eq!(
                            again.sheet_by_name("Data").is_ok(),
                            edit != "sheet",
                            "{label}"
                        );
                        assert_eq!(
                            model_source(&again, host),
                            ("Data".to_owned(), cache_ref.to_owned()),
                            "{label}"
                        );
                    }
                    keep_output(
                        &format!(
                            "source-delete-{edit}-{}-{reader}-{writer}",
                            host.map_or("orphan", |_| "live")
                        ),
                        &bytes,
                    );
                }
            }
        }
    }
}

/// Row and column edits on a sheet that no longer exists change nothing, so the
/// cache sources that still name the deleted `Data` keep Excel's reference.
#[test]
fn edits_on_a_deleted_source_sheet_leave_the_cache_sources_unchanged() {
    for (fixture, host, cache_ref) in [
        (FIXTURE, None, "A1:D7"),
        (LIVE_FIXTURE, Some("Report"), "A1:C3"),
    ] {
        for (reader, read) in readers() {
            for edit in [
                "insert-rows",
                "insert-columns",
                "remove-rows",
                "remove-columns",
            ] {
                let mut book = read(fixture).unwrap();
                book.remove_sheet_by_name("Data").unwrap();
                let mut before = unpack(&save(&book));
                match edit {
                    "insert-rows" => book.insert_new_row("Data", 1, 1),
                    "insert-columns" => book.insert_new_column("Data", "A", 1),
                    "remove-rows" => book.remove_row("Data", 1, 1),
                    _ => book.remove_column("Data", "A", 1),
                }
                let normal = save(&book);
                let mut after = unpack(&normal);
                // The shared string count grows with every save.
                before.remove("xl/sharedStrings.xml");
                after.remove("xl/sharedStrings.xml");
                assert_eq!(before, after, "{reader} {edit}");
                let streamed = stream(book);
                for (writer, bytes) in [("normal", normal), ("streaming", streamed)] {
                    let label = format!("{reader} {writer} {edit}");
                    assert_eq!(
                        saved_source(&bytes),
                        ("Data".to_owned(), cache_ref.to_owned()),
                        "{label}"
                    );
                    for (_, reread) in readers() {
                        let mut again = reread(&bytes).unwrap();
                        again.try_read_sheet_collection().unwrap();
                        assert_eq!(
                            model_source(&again, host),
                            ("Data".to_owned(), cache_ref.to_owned()),
                            "{label}"
                        );
                    }
                }
            }
        }
    }
}

/// An insert that does not fit the grid leaves the workbook unchanged.
#[test]
fn inserting_past_the_grid_edge_changes_nothing() {
    for fixture in [FIXTURE, LIVE_FIXTURE] {
        for (reader, read) in readers() {
            for edit in ["row", "column"] {
                let mut book = read(fixture).unwrap();
                let mut before = unpack(&save(&book));
                match edit {
                    "row" => book.insert_new_row("Data", 1_048_576, 2),
                    _ => book.insert_new_column_by_index("Data", 16_384, 2),
                }
                let mut after = unpack(&save(&book));
                // The shared string count grows with every save.
                before.remove("xl/sharedStrings.xml");
                after.remove("xl/sharedStrings.xml");
                assert_eq!(before, after, "{reader} {edit}");
            }
        }
    }
}
