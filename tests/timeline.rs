#![cfg(feature = "serde")]
//! Timelines (date filters on pivot tables) through the public entry points:
//! the readers, the editing API and the writer. Every expected value comes
//! from workbooks Excel authored (`tests/fixtures/timelines/excel-authored`
//! and `tests/fixtures/slicers/excel-authored/timeline.xlsx`, each checked
//! against its SHA-256 manifest) or from the schema, never from library
//! output. `timeline_excel_open.ps1` opens the outputs of these tests in Excel.
#[path = "support/slicer_package_fixture.rs"]
mod fixture;

use std::io::{
    Cursor,
    Read,
    Write,
};

use chrono::NaiveDate;
use fixture::*;
use serde_json::Value;
use sha2::{
    Digest,
    Sha256,
};
use umya_spreadsheet::{
    NewTimeline,
    PivotTable,
    SlicerStyleDefinition,
    SlicerStyleElementType,
    SlicerStyleItemDefinition,
    Style,
    TableStyleDefinition,
    TableStyleElementDefinition,
    TableStyleElementType,
    TimelineLevel,
    TimelineSelection,
    Workbook,
    drawing::spreadsheet::{
        DrawingAnchor,
        Extent,
        SlicerFrameKind,
    },
    pivot_date_filter,
};
use zip::{
    ZipArchive,
    ZipWriter,
    write::SimpleFileOptions,
};

const EXCEL_DIRECTORY: &str = "tests/fixtures/timelines/excel-authored";
const SLICER_DIRECTORY: &str = "tests/fixtures/slicers/excel-authored";

fn checked(directory: &str, name: &str, manifest_key: &str) -> Vec<u8> {
    let bytes = std::fs::read(format!("{directory}/{name}.xlsx")).unwrap();
    let hash: String = Sha256::digest(&bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let manifest: Value =
        serde_json::from_slice(&std::fs::read(format!("{directory}/manifest.json")).unwrap())
            .unwrap();
    assert_eq!(
        manifest[manifest_key]["workbook_sha256"].as_str(),
        Some(hash.as_str()),
        "{name} does not match the manifest"
    );
    bytes
}

/// The workbook Excel authored with one timeline over `PivotTable1`, filtered
/// to 2024-01-01 ..= 2024-02-29 (data of 2024 only).
fn one_timeline() -> Vec<u8> {
    checked(SLICER_DIRECTORY, "timeline", "timeline")
}

/// Two pivot tables on one cache, two timelines (`Day`, `Ship`), the `Ship`
/// timeline linking both pivot tables and using a custom style.
fn two_pivots() -> Vec<u8> {
    checked(
        EXCEL_DIRECTORY,
        "two-pivots-custom-style",
        "two-pivots-custom-style",
    )
}

/// A pivot table with a date field and no timeline.
fn without_timeline() -> Vec<u8> {
    checked(
        EXCEL_DIRECTORY,
        "pivot-without-timeline",
        "pivot-without-timeline",
    )
}

/// A pivot table whose date field still holds the filter of a deleted
/// timeline.
fn filtered_without_timeline() -> Vec<u8> {
    checked(EXCEL_DIRECTORY, "no-timeline", "no-timeline")
}

/// Two timeline views on one timeline cache that links no pivot table.
fn disconnected() -> Vec<u8> {
    checked(EXCEL_DIRECTORY, "disconnected-pivot", "disconnected-pivot")
}

fn date(year: i32, month: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, day).unwrap()
}

fn range(start: (i32, u32, u32), end: (i32, u32, u32)) -> TimelineSelection {
    TimelineSelection::Range {
        start: date(start.0, start.1, start.2),
        end:   date(end.0, end.1, end.2),
    }
}

fn saved_parts(book: &Workbook) -> Parts {
    unpack(&save(book))
}

/// The package with `edit` applied to every text part.
fn repacked(bytes: &[u8], edit: impl Fn(&str, &mut String)) -> Vec<u8> {
    let mut archive = ZipArchive::new(Cursor::new(bytes.to_vec())).unwrap();
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    for index in 0..archive.len() {
        let mut part = archive.by_index(index).unwrap();
        let name = part.name().unwrap().into_owned();
        let mut content = Vec::new();
        part.read_to_end(&mut content).unwrap();
        zip.start_file(name.clone(), SimpleFileOptions::default())
            .unwrap();
        if let Ok(mut text) = String::from_utf8(content.clone()) {
            edit(&name, &mut text);
            zip.write_all(text.as_bytes()).unwrap();
        } else {
            zip.write_all(&content).unwrap();
        }
    }
    zip.finish().unwrap().into_inner()
}

fn replace_once(text: &mut String, from: &str, to: &str) {
    assert_eq!(text.matches(from).count(), 1, "{from} in {text}");
    *text = text.replace(from, to);
}

fn pivot<'a>(book: &'a Workbook, sheet: &str, name: &str) -> &'a PivotTable {
    book.sheet_by_name(sheet)
        .unwrap()
        .pivot_tables()
        .iter()
        .find(|table| table.pivot_table_definition().name() == name)
        .unwrap_or_else(|| panic!("pivot table {name} on {sheet}"))
}

fn filter_of(book: &Workbook, name: &str, field: u32) -> Option<(String, Vec<String>)> {
    pivot_date_filter(pivot(book, "Report", name), field)
}

fn between(low: u32, high: u32) -> Option<(String, Vec<String>)> {
    Some((
        "dateBetween".to_owned(),
        vec![low.to_string(), high.to_string()],
    ))
}

fn timeline_cache_part(parts: &Parts, index: usize) -> &str {
    &parts[&format!("xl/timelineCaches/timelineCache{index}.xml")]
}

fn pivot_part<'a>(parts: &'a Parts, name: &str) -> &'a str {
    parts
        .iter()
        .find(|(path, xml)| {
            path.starts_with("xl/pivotTables/pivotTable")
                && attribute(tags(xml, "<pivotTableDefinition")[0], "name").as_deref() == Some(name)
        })
        .map(|(_, xml)| xml.as_str())
        .unwrap_or_else(|| panic!("pivot part of {name}"))
}

/// The text of the `<filter>` of `field`, up to and including its autoFilter.
fn filter_text(pivot_xml: &str, field: u32) -> String {
    let start = pivot_xml
        .find(&format!("<filter fld=\"{field}\""))
        .unwrap_or_else(|| panic!("filter of field {field} in {pivot_xml}"));
    let end = pivot_xml[start..].find("</autoFilter>").unwrap() + start + "</autoFilter>".len();
    collapse_empty_elements(&pivot_xml[start..end])
}

/// The filter Excel writes for a date range (`customFilters`), derived from
/// the Excel-authored `sel*` workbooks.
fn excel_filter(field: u32, id: u32, name: &str, low: u32, high: Option<u32>) -> String {
    match high {
        Some(high) => format!(
            r#"<filter fld="{field}" type="dateBetween" evalOrder="-1" id="{id}" name="{name}"><autoFilter ref="A1"><filterColumn colId="0"><customFilters and="1"><customFilter operator="greaterThanOrEqual" val="{low}"/><customFilter operator="lessThanOrEqual" val="{high}"/></customFilters></filterColumn></autoFilter>"#
        ),
        None => format!(
            r#"<filter fld="{field}" type="dateEqual" evalOrder="-1" id="{id}" name="{name}"><autoFilter ref="A1"><filterColumn colId="0"><customFilters><customFilter val="{low}"/></customFilters></filterColumn></autoFilter>"#
        ),
    }
}

fn state_tag(cache_xml: &str) -> &str {
    let start = cache_xml.find("<state ").unwrap();
    let end = cache_xml.find("</state>").unwrap() + "</state>".len();
    &cache_xml[start..end]
}

fn timeline_tag<'a>(timelines_xml: &'a str, name: &str) -> &'a str {
    tags(timelines_xml, "<timeline")
        .into_iter()
        .find(|tag| attribute(tag, "name").as_deref() == Some(name))
        .unwrap_or_else(|| panic!("timeline {name} in {timelines_xml}"))
}

fn timelines_part(parts: &Parts) -> &str {
    let paths: Vec<_> = parts
        .keys()
        .filter(|path| path.starts_with("xl/timelines/timeline"))
        .collect();
    assert_eq!(paths.len(), 1, "{paths:?}");
    &parts[paths[0]]
}

/// The parts that differ between two packages, with their text. The shared
/// string table is left out: its `count` grows with every save of the same
/// workbook, whatever the save contains.
fn assert_same_parts(actual: &Parts, expected: &Parts, label: &str) {
    let mut differing = Vec::new();
    for path in actual.keys().chain(expected.keys()) {
        if path != "xl/sharedStrings.xml"
            && actual.get(path) != expected.get(path)
            && !differing.contains(path)
        {
            differing.push(path.clone());
        }
    }
    assert!(
        differing.is_empty(),
        "{label}: parts differ: {:?}
{:#?}",
        differing,
        differing
            .iter()
            .map(|path| (actual.get(path), expected.get(path)))
            .collect::<Vec<_>>()
    );
}

/// `<a b="1"></a>` written as `<a b="1"/>`: the same element either way.
fn collapse_empty_elements(xml: &str) -> String {
    let mut result = xml.to_owned();
    let mut from = 0;
    while let Some(found) = result[from..].find("></") {
        let at = from + found;
        let close_end = result[at + 3..].find('>').map(|end| at + 3 + end);
        let open_start = result[..at].rfind('<');
        if let (Some(close_end), Some(open_start)) = (close_end, open_start) {
            let name = &result[at + 3..close_end];
            let open = &result[open_start + 1..at];
            let open_name = open.split([' ', '/']).next().unwrap_or("");
            if open_name == name && !open.starts_with('/') && !open.ends_with('/') {
                result.replace_range(at..=close_end, "/>");
                from = open_start;
                continue;
            }
        }
        from = at + 1;
    }
    result
}

fn without_namespace_declarations(xml: &str) -> String {
    let mut result = String::new();
    let mut rest = xml;
    while let Some(start) = rest.find(" xmlns") {
        result.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let equals = after.find('=').unwrap();
        let quote = after[equals + 1..].chars().next().unwrap();
        let close = after[equals + 2..].find(quote).unwrap();
        rest = &after[equals + 2 + close + 1..];
    }
    result.push_str(rest);
    collapse_empty_elements(&result)
}

fn element_text<'a>(xml: &'a str, name: &str) -> &'a str {
    let start = xml.find(&format!("<{name}")).unwrap();
    let close = format!("</{name}>");
    let end = xml[start..].find(&close).unwrap() + start + close.len();
    &xml[start..end]
}

// ---- reading and writing what Excel wrote -------------------------------

#[test]
fn excel_timeline_is_modelled_and_written_back_as_excel_wrote_it() {
    let original = unpack(&one_timeline());
    for (label, read) in readers() {
        let mut book = read(&one_timeline()).unwrap();
        book.try_read_sheet_collection().unwrap();
        let caches = book.timeline_caches();
        assert_eq!(caches.len(), 1, "{label}");
        let cache = &caches[0];
        assert_eq!(cache.name, "NativeTimeline_Day", "{label}");
        assert_eq!(cache.source_name, "Day", "{label}");
        assert_eq!(
            cache
                .pivot_tables
                .as_ref()
                .unwrap()
                .iter()
                .map(|link| (link.sheet_id, link.name.as_str()))
                .collect::<Vec<_>>(),
            vec![(2, "PivotTable1")],
            "{label}"
        );
        assert_eq!(cache.state.pivot_cache_id, 1_968_286_413, "{label}");
        assert_eq!(cache.state.filter_type, "dateBetween", "{label}");
        let selection = cache.state.selection.as_ref().unwrap();
        assert_eq!(selection.start_date, "2024-01-01T00:00:00", "{label}");
        assert_eq!(selection.end_date, "2024-02-29T00:00:00", "{label}");
        assert_eq!(
            cache.state.bounds.end_date, "2025-01-01T00:00:00",
            "{label}"
        );
        assert_eq!(
            cache.selection().unwrap(),
            range((2024, 1, 1), (2024, 2, 29)),
            "{label}"
        );

        let sheet = book.sheet_by_name("Report").unwrap();
        let [timeline] = sheet.timelines() else {
            panic!("{label}: one timeline expected");
        };
        assert_eq!(timeline.name, "Day", "{label}");
        assert_eq!(timeline.cache, "NativeTimeline_Day", "{label}");
        assert_eq!(timeline.level, TimelineLevel::Months, "{label}");
        assert_eq!(timeline.selection_level, TimelineLevel::Months, "{label}");
        assert_eq!(
            timeline.scroll_position.as_deref(),
            Some("2024-01-01T00:00:00"),
            "{label}"
        );
        let frames = sheet.worksheet_drawing().slicer_frames();
        assert_eq!(frames.len(), 1, "{label}");
        assert_eq!(frames[0].kind(), SlicerFrameKind::Timeline, "{label}");
        assert_eq!(frames[0].slicer_name(), "Day", "{label}");

        keep_output(&format!("timeline-round-trip-{label}"), &save(&book));
        let written = saved_parts(&book);
        assert_eq!(
            timeline_cache_part(&written, 1),
            original["xl/timelineCaches/timelineCache1.xml"],
            "{label}: the timeline cache part is Excel's"
        );
        assert_eq!(
            without_namespace_declarations(timelines_part(&written)),
            without_namespace_declarations(&original["xl/timelines/timeline1.xml"]),
            "{label}: the timelines part is Excel's"
        );
        assert_eq!(
            content_type(&written, "xl/timelineCaches/timelineCache1.xml").as_deref(),
            Some("application/vnd.ms-excel.timelineCache+xml"),
            "{label}"
        );
        assert_eq!(
            filter_text(pivot_part(&written, "PivotTable1"), 3),
            excel_filter(3, 2, "Day", 45292, Some(45351)),
            "{label}: the pivot filter is Excel's"
        );
    }
}

// ---- selection --------------------------------------------------------------

#[test]
fn selection_writes_the_filter_and_levels_excel_measured() {
    // (start, end, serial of the first and last day, selectionLevel): the
    // Excel-authored `sel0..7` and `t1-*` workbooks (TimelineState.
    // SetFilterDateRange, day serials from the pivot filters Excel wrote).
    let cases = [
        (
            (2024, 1, 1),
            (2024, 6, 30),
            45292,
            Some(45473),
            TimelineLevel::Quarters,
        ),
        (
            (2024, 1, 1),
            (2024, 12, 31),
            45292,
            Some(45657),
            TimelineLevel::Years,
        ),
        (
            (2024, 4, 1),
            (2024, 12, 31),
            45383,
            Some(45657),
            TimelineLevel::Quarters,
        ),
        (
            (2024, 3, 1),
            (2024, 3, 31),
            45352,
            Some(45382),
            TimelineLevel::Months,
        ),
        (
            (2024, 2, 10),
            (2024, 2, 10),
            45332,
            None,
            TimelineLevel::Days,
        ),
        (
            (2024, 1, 1),
            (2024, 3, 31),
            45292,
            Some(45382),
            TimelineLevel::Quarters,
        ),
        (
            (2024, 7, 1),
            (2024, 9, 30),
            45474,
            Some(45565),
            TimelineLevel::Quarters,
        ),
        (
            (2024, 1, 10),
            (2024, 2, 20),
            45301,
            Some(45342),
            TimelineLevel::Days,
        ),
        (
            (2024, 1, 1),
            (2024, 5, 31),
            45292,
            Some(45443),
            TimelineLevel::Months,
        ),
    ];
    for (start, end, low, high, level) in cases {
        let mut book = read_eager(&one_timeline()).unwrap();
        book.set_timeline_selection("NativeTimeline_Day", range(start, end))
            .unwrap();
        let expected_filter = match high {
            Some(high) => between(low, high),
            None => Some(("dateEqual".to_owned(), vec![low.to_string()])),
        };
        assert_eq!(
            filter_of(&book, "PivotTable1", 3),
            expected_filter,
            "{start:?}..{end:?}"
        );
        if start == (2024, 3, 1) {
            keep_output("timeline-selection-march", &save(&book));
        }
        let written = saved_parts(&book);
        assert_eq!(
            filter_text(pivot_part(&written, "PivotTable1"), 3),
            excel_filter(3, 2, "Day", low, high),
            "{start:?}..{end:?}"
        );
        assert!(
            collapse_empty_elements(pivot_part(&written, "PivotTable1"))
                .contains(r#"<x15:pivotFilter useWholeDay="1"/>"#),
            "{start:?}..{end:?}: the end day is included"
        );
        let state = state_tag(timeline_cache_part(&written, 1));
        assert!(
            state.contains(if high.is_some() {
                r#"filterType="dateBetween""#
            } else {
                r#"filterType="dateEqual""#
            }),
            "{state}"
        );
        assert!(
            state.contains(&format!(
                r#"<selection startDate="{:04}-{:02}-{:02}T00:00:00" endDate="{:04}-{:02}-{:02}T00:00:00"/>"#,
                start.0, start.1, start.2, end.0, end.1, end.2
            )),
            "{state}"
        );
        // The selection never changes the level the view shows.
        let tag = timeline_tag(timelines_part(&written), "Day").to_owned();
        assert_eq!(attribute(&tag, "level").as_deref(), Some("2"), "{tag}");
        assert_eq!(
            attribute(&tag, "selectionLevel"),
            Some(level.index().to_string()),
            "{start:?}..{end:?}: {tag}"
        );
        for (label, read) in readers() {
            let again = read(&save(&book)).unwrap();
            assert_eq!(
                again.timeline_caches()[0].selection().unwrap(),
                range(start, end),
                "{label}"
            );
        }
    }
}

#[test]
fn clearing_the_selection_removes_the_filter_as_excel_does() {
    let mut book = read_eager(&one_timeline()).unwrap();
    book.set_timeline_selection("NativeTimeline_Day", TimelineSelection::All)
        .unwrap();
    assert_eq!(filter_of(&book, "PivotTable1", 3), None);
    keep_output("timeline-selection-cleared", &save(&book));
    let written = saved_parts(&book);
    // Excel's `t1-then-cleared`: no selection, `unknown`, no pivot filter.
    assert_eq!(
        state_tag(timeline_cache_part(&written, 1)),
        concat!(
            r#"<state minimalRefreshVersion="6" lastRefreshVersion="6" pivotCacheId="1968286413" "#,
            r#"filterType="unknown"><bounds startDate="2024-01-01T00:00:00" "#,
            r#"endDate="2025-01-01T00:00:00"/></state>"#
        )
    );
    assert!(!pivot_part(&written, "PivotTable1").contains("<filters"));
    for (label, read) in readers() {
        let mut again = read(&save(&book)).unwrap();
        again.try_read_sheet_collection().unwrap();
        assert_eq!(
            again.timeline_caches()[0].selection().unwrap(),
            TimelineSelection::All,
            "{label}"
        );
    }
}

#[test]
fn selection_outside_the_data_is_taken_as_excel_takes_it_and_reversed_changes_nothing() {
    // Excel (`b1`, `b3`, `b5`): a range beyond the dates of the data is
    // selected as given, the bounds stay as they were.
    for (label, start, end, low, high, level) in [
        (
            "starts before the data",
            (2023, 3, 1),
            (2024, 2, 29),
            44986,
            45351,
            TimelineLevel::Months,
        ),
        (
            "entirely after the data",
            (2028, 1, 1),
            (2028, 6, 30),
            46753,
            46934,
            TimelineLevel::Quarters,
        ),
        (
            "wider than the data",
            (2022, 1, 1),
            (2030, 12, 31),
            44562,
            47848,
            TimelineLevel::Years,
        ),
    ] {
        let mut book = read_eager(&one_timeline()).unwrap();
        book.set_timeline_selection("NativeTimeline_Day", range(start, end))
            .unwrap();
        keep_output(
            &format!("timeline-selection-outside-{}", start.0),
            &save(&book),
        );
        let written = saved_parts(&book);
        let state = state_tag(timeline_cache_part(&written, 1));
        assert_eq!(
            attribute(state, "filterType").as_deref(),
            Some("dateBetween"),
            "{label}"
        );
        let cache = timeline_cache_part(&written, 1);
        assert!(
            cache.contains(
                r#"<bounds startDate="2024-01-01T00:00:00" endDate="2025-01-01T00:00:00"/>"#
            ),
            "{label}: {cache}"
        );
        assert_eq!(
            filter_of(&book, "PivotTable1", 3),
            between(low, high),
            "{label}"
        );
        assert_eq!(
            book.sheet_by_name("Report").unwrap().timelines()[0].selection_level,
            level,
            "{label}"
        );
    }
    let mut book = read_eager(&one_timeline()).unwrap();
    let before = saved_parts(&book);
    for (label, selection) in [("reversed", range((2024, 3, 1), (2024, 2, 1)))] {
        let message = book
            .set_timeline_selection("NativeTimeline_Day", selection)
            .unwrap_err()
            .to_string();
        assert!(message.contains("NativeTimeline_Day"), "{label}: {message}");
        assert_same_parts(
            &saved_parts(&book),
            &before,
            &format!("{label}: nothing changed"),
        );
    }
    assert!(
        book.set_timeline_selection("Missing", TimelineSelection::All)
            .unwrap_err()
            .to_string()
            .contains("Missing")
    );
}

#[test]
fn selection_updates_every_linked_pivot_table() {
    let mut book = read_eager(&two_pivots()).unwrap();
    // `Ship` (field 3) links both pivot tables; `PivotTable1` also keeps a
    // `Day` filter (field 2) that the selection does not touch.
    assert_eq!(filter_of(&book, "PivotTable1", 3), between(45323, 45565));
    assert_eq!(filter_of(&book, "PivotTable2", 3), between(45323, 45565));
    book.set_timeline_selection("NativeTimeline_Ship", range((2024, 3, 1), (2024, 3, 31)))
        .unwrap();
    for name in ["PivotTable1", "PivotTable2"] {
        assert_eq!(filter_of(&book, name, 3), between(45352, 45382), "{name}");
    }
    assert_eq!(filter_of(&book, "PivotTable1", 2), between(45292, 45443));
    keep_output("timeline-two-pivots-march", &save(&book));
    let written = saved_parts(&book);
    let first = pivot_part(&written, "PivotTable1");
    // A replaced filter keeps its id and its place.
    assert_eq!(
        filter_text(first, 3),
        excel_filter(3, 5, "Ship", 45352, Some(45382))
    );
    assert!(first.find("fld=\"3\"").unwrap() < first.find("fld=\"2\"").unwrap());
    assert_eq!(
        filter_text(pivot_part(&written, "PivotTable2"), 3),
        excel_filter(3, 3, "Ship", 45352, Some(45382))
    );

    book.set_timeline_selection("NativeTimeline_Ship", TimelineSelection::All)
        .unwrap();
    keep_output("timeline-two-pivots-cleared", &save(&book));
    let cleared = saved_parts(&book);
    assert!(!pivot_part(&cleared, "PivotTable2").contains("<filters"));
    let first = pivot_part(&cleared, "PivotTable1");
    assert!(first.contains(r#"<filters count="1">"#), "{first}");
    assert_eq!(
        filter_text(first, 2),
        excel_filter(2, 2, "Day", 45292, Some(45443))
    );
}

#[test]
fn selection_is_all_or_nothing_across_linked_pivot_tables() {
    // The second pivot table linked by `Ship` holds a caption filter on the
    // Ship field, which a date selection cannot replace.
    let bytes = repacked(&two_pivots(), |path, text| {
        if path.starts_with("xl/pivotTables/pivotTable")
            && text.contains(r#" name="PivotTable2" cacheId"#)
        {
            replace_once(
                text,
                r#"type="dateBetween" evalOrder="-1" id="3" name="Ship""#,
                r#"type="captionEqual" evalOrder="-1" id="3" name="Ship""#,
            );
        }
    });
    let mut book = read_eager(&bytes).unwrap();
    let before = saved_parts(&book);
    let first_filter = filter_of(&book, "PivotTable1", 3);
    let message = book
        .set_timeline_selection("NativeTimeline_Ship", range((2024, 3, 1), (2024, 3, 31)))
        .unwrap_err()
        .to_string();
    assert!(
        message.contains("NativeTimeline_Ship") && message.contains("PivotTable2"),
        "{message}"
    );
    // The first linked pivot table accepted the filter on its own; the whole
    // selection was refused, so nothing changed anywhere.
    assert_eq!(filter_of(&book, "PivotTable1", 3), first_filter);
    assert_same_parts(&saved_parts(&book), &before, "unchanged");
    let message = book
        .set_timeline_selection("NativeTimeline_Ship", TimelineSelection::All)
        .unwrap_err()
        .to_string();
    assert!(message.contains("NativeTimeline_Ship"), "{message}");
    assert_same_parts(&saved_parts(&book), &before, "unchanged");
}

#[test]
fn unresolved_pivot_binding_names_the_timeline_cache() {
    let bytes = repacked(&one_timeline(), |path, text| {
        if path == "xl/timelineCaches/timelineCache1.xml" {
            replace_once(text, r#"name="PivotTable1"/>"#, r#"name="PivotTable9"/>"#);
        }
    });
    for (label, read) in readers() {
        let failure = read(&bytes)
            .and_then(|book| try_save(&book))
            .expect_err(label)
            .to_string();
        assert!(
            failure.contains("NativeTimeline_Day") && failure.contains("PivotTable9"),
            "{label}: {failure}"
        );
    }
}

// ---- creating, editing and deleting ----------------------------------------

fn new_timeline(name: &str) -> NewTimeline<'_> {
    let mut extent = Extent::default();
    extent.set_cx(3_810_000).set_cy(1_397_000);
    NewTimeline {
        sheet: "Report",
        name,
        pivot_sheet: "Report",
        pivot_table: "PivotTable1",
        field: "Day",
        anchor: DrawingAnchor::OneCell {
            from: Default::default(),
            extent,
        },
    }
}

#[test]
fn created_timeline_is_written_with_the_parts_excel_writes() {
    let mut book = read_eager(&without_timeline()).unwrap();
    book.add_timeline(&new_timeline("Day")).unwrap();
    keep_output("timeline-created-unfiltered", &save(&book));
    let parts = saved_parts(&book);
    let workbook = &parts["xl/workbook.xml"];
    assert!(
        workbook.contains(r#"<definedName name="NativeTimeline_Day">#N/A</definedName>"#),
        "{workbook}"
    );
    assert!(workbook.contains("{D0CA8CA8-9F24-4464-BF8E-62219DCF47F9}"));
    let reference = tags(workbook, "<x15:timelineCacheRef")
        .into_iter()
        .next()
        .unwrap();
    let (kind, part) = resolve(
        &parts,
        "xl/workbook.xml",
        &attribute(reference, "r:id").unwrap(),
    );
    assert_eq!(
        kind,
        "http://schemas.microsoft.com/office/2011/relationships/timelineCache"
    );
    assert_eq!(part, "xl/timelineCaches/timelineCache1.xml");
    assert_eq!(
        content_type(&parts, &part).as_deref(),
        Some("application/vnd.ms-excel.timelineCache+xml")
    );
    // The x14 pivot cache id the timeline cache names is the one the pivot
    // cache now carries.
    let pivot_cache = &parts["xl/pivotCache/pivotCacheDefinition1.xml"];
    let id = attribute(
        tags(pivot_cache, "<x14:pivotCacheDefinition")[0],
        "pivotCacheId",
    )
    .unwrap();
    let cache = timeline_cache_part(&parts, 1);
    let root = tags(cache, "<timelineCacheDefinition")[0];
    assert_eq!(
        attribute(root, "name").as_deref(),
        Some("NativeTimeline_Day")
    );
    assert_eq!(attribute(root, "sourceName").as_deref(), Some("Day"));
    assert!(
        cache.contains(r#"<pivotTables><pivotTable tabId="2" name="PivotTable1"/></pivotTables>"#)
    );
    // Excel's unfiltered timeline (`t1-unfiltered`); the data runs from
    // 2024-01-05 to 2025-03-30, so the bounds are 2024-01-01 ..= 2026-01-01.
    assert_eq!(
        state_tag(cache),
        format!(
            concat!(
                r#"<state minimalRefreshVersion="6" lastRefreshVersion="6" pivotCacheId="{}" "#,
                r#"filterType="unknown"><bounds startDate="2024-01-01T00:00:00" "#,
                r#"endDate="2026-01-01T00:00:00"/></state>"#
            ),
            id
        )
    );
    let timelines = timelines_part(&parts);
    let tag = timeline_tag(timelines, "Day");
    assert_eq!(
        attribute(tag, "cache").as_deref(),
        Some("NativeTimeline_Day")
    );
    assert_eq!(attribute(tag, "caption").as_deref(), Some("Day"));
    assert_eq!(attribute(tag, "level").as_deref(), Some("2"));
    assert_eq!(
        content_type(&parts, "xl/timelines/timeline1.xml").as_deref(),
        Some("application/vnd.ms-excel.timeline+xml")
    );
    let sheet_part = sheet_part_of(&parts, "Report");
    let sheet = &parts[&sheet_part];
    assert!(sheet.contains("{7E03D99C-DC04-49d9-9315-930204A7B6E9}"));
    let reference = tags(sheet, "<x15:timelineRef")[0];
    let (kind, part) = resolve(&parts, &sheet_part, &attribute(reference, "r:id").unwrap());
    assert_eq!(
        kind,
        "http://schemas.microsoft.com/office/2011/relationships/timeline"
    );
    assert_eq!(part, "xl/timelines/timeline1.xml");
    let drawing = parts
        .iter()
        .find(|(path, _)| path.starts_with("xl/drawings/drawing"))
        .map(|(_, xml)| xml)
        .unwrap();
    assert!(drawing.contains(r#"Requires="tsle""#), "{drawing}");
    assert!(drawing.contains("http://schemas.microsoft.com/office/drawing/2012/timeslicer"));
    assert!(drawing.contains(r#"<tsle:timeslicer"#) && drawing.contains(r#"name="Day""#));

    for (label, read) in readers() {
        let mut again = read(&save(&book)).unwrap();
        again.try_read_sheet_collection().unwrap();
        assert_eq!(again.timeline_caches().len(), 1, "{label}");
        let cache = &again.timeline_caches()[0];
        assert_eq!(
            cache.selection().unwrap(),
            TimelineSelection::All,
            "{label}"
        );
        assert_eq!(cache.state.pivot_cache_id.to_string(), id, "{label}");
        let sheet = again.sheet_by_name("Report").unwrap();
        assert_eq!(sheet.timelines()[0].name, "Day", "{label}");
        assert_eq!(
            sheet.worksheet_drawing().slicer_frames()[0].kind(),
            SlicerFrameKind::Timeline,
            "{label}"
        );
    }

    // The new timeline filters the pivot table it was created for.
    book.set_timeline_selection("NativeTimeline_Day", range((2024, 1, 1), (2024, 2, 29)))
        .unwrap();
    book.set_timeline_level("Day", TimelineLevel::Quarters)
        .unwrap();
    book.timeline_mut("Day").unwrap().caption = Some("Sales days".to_owned());
    assert_eq!(filter_of(&book, "PivotTable1", 2), between(45292, 45351));
    keep_output("timeline-created", &save(&book));
    for (label, read) in readers() {
        let mut again = read(&save(&book)).unwrap();
        again.try_read_sheet_collection().unwrap();
        let timeline = &again.sheet_by_name("Report").unwrap().timelines()[0];
        assert_eq!(timeline.level, TimelineLevel::Quarters, "{label}");
        assert_eq!(timeline.selection_level, TimelineLevel::Months, "{label}");
        assert_eq!(timeline.caption.as_deref(), Some("Sales days"), "{label}");
        assert_eq!(
            filter_of(&again, "PivotTable1", 2),
            between(45292, 45351),
            "{label}"
        );
    }
}

#[test]
fn creating_a_timeline_rejects_what_excel_cannot_filter() {
    let mut book = read_eager(&without_timeline()).unwrap();
    let before = saved_parts(&book);
    let mut cases = Vec::new();
    let mut not_a_date = new_timeline("Team");
    not_a_date.field = "Team";
    cases.push(("not a date field", not_a_date, "does not hold dates"));
    let mut missing_field = new_timeline("Gone");
    missing_field.field = "Gone";
    cases.push(("missing field", missing_field, "Gone"));
    let mut missing_pivot = new_timeline("Nowhere");
    missing_pivot.pivot_table = "PivotTable9";
    cases.push(("missing pivot", missing_pivot, "PivotTable9"));
    let mut missing_sheet = new_timeline("Nowhere");
    missing_sheet.sheet = "Missing";
    cases.push(("missing sheet", missing_sheet, "Missing"));
    cases.push(("unnamed", new_timeline(""), "needs a name"));
    for (label, request, expected) in cases {
        let message = book.add_timeline(&request).unwrap_err().to_string();
        assert!(message.contains(expected), "{label}: {message}");
        assert_same_parts(
            &saved_parts(&book),
            &before,
            &format!("{label}: nothing changed"),
        );
    }

    // Names are unique among slicers and timelines, ignoring case.
    book.add_timeline(&new_timeline("Day")).unwrap();
    let message = book
        .add_timeline(&new_timeline("DAY"))
        .unwrap_err()
        .to_string();
    assert!(message.contains("already exists"), "{message}");
}

#[test]
fn a_new_timeline_takes_over_the_date_range_the_field_already_filters_by() {
    // Excel (`p6-create-over-filter`): adding a timeline over a date field of
    // a pivot table that filters it by 2024-01-01 ..= 2024-02-29 selects that
    // range in the timeline and drops the pivot table's own filter.
    for (label, read) in readers() {
        let mut book = read(&filtered_without_timeline()).unwrap();
        book.try_read_sheet_collection().unwrap();
        assert_eq!(
            filter_of(&book, "PivotTable1", 2),
            between(45292, 45351),
            "{label}"
        );
        book.add_timeline(&new_timeline("Day")).unwrap();
        if label == "eager" {
            keep_output("timeline-adopted-range", &save(&book));
        }
        let written = saved_parts(&book);
        let cache = timeline_cache_part(&written, 1);
        assert_eq!(
            attribute(state_tag(cache), "filterType").as_deref(),
            Some("dateBetween"),
            "{label}"
        );
        assert!(
            cache.contains(
                r#"<selection startDate="2024-01-01T00:00:00" endDate="2024-02-29T00:00:00"/>"#
            ),
            "{label}: {cache}"
        );
        assert!(
            !pivot_part(&written, "PivotTable1").contains("<filters"),
            "{label}"
        );
        let mut again = read(&save(&book)).unwrap();
        again.try_read_sheet_collection().unwrap();
        assert_eq!(filter_of(&again, "PivotTable1", 2), None, "{label}");
        assert_eq!(
            again.timeline_caches()[0].selection().unwrap(),
            range((2024, 1, 1), (2024, 2, 29)),
            "{label}"
        );
        // Months, as Excel writes for a whole-month selection.
        assert_eq!(
            again.sheet_by_name("Report").unwrap().timelines()[0].selection_level,
            TimelineLevel::Months,
            "{label}"
        );
    }
}

#[test]
fn view_properties_level_and_rename_persist() {
    let mut book = read_eager(&one_timeline()).unwrap();
    {
        let timeline = book.timeline_mut("Day").unwrap();
        timeline.caption = Some("My Days".to_owned());
        timeline.show_header = Some(false);
        timeline.show_selection_label = Some(false);
        timeline.show_time_level = Some(false);
        timeline.show_horizontal_scrollbar = Some(false);
        timeline.scroll_position = Some("2024-02-01T00:00:00".to_owned());
        timeline.style = Some("TimeSlicerStyleDark2".to_owned());
    }
    book.set_timeline_level("Day", TimelineLevel::Quarters)
        .unwrap();
    book.rename_timeline("Day", "Sales Days").unwrap();
    keep_output("timeline-view-edited", &save(&book));
    let parts = saved_parts(&book);
    // Excel's attribute order (`t1-quarters-range`).
    assert_eq!(
        timeline_tag(timelines_part(&parts), "Sales Days"),
        concat!(
            r#"<timeline name="Sales Days" xr10:uid="{84ED1F61-60F8-4FE3-9D19-2AA0C92D8A48}" "#,
            r#"cache="NativeTimeline_Day" caption="My Days" showHeader="0" "#,
            r#"showSelectionLabel="0" showTimeLevel="0" showHorizontalScrollbar="0" "#,
            r#"level="1" selectionLevel="2" scrollPosition="2024-02-01T00:00:00" "#,
            r#"style="TimeSlicerStyleDark2"/>"#
        )
    );
    let drawing = parts
        .iter()
        .find(|(path, _)| path.starts_with("xl/drawings/drawing"))
        .map(|(_, xml)| xml)
        .unwrap();
    assert_eq!(
        drawing.matches(r#"name="Sales Days""#).count(),
        2,
        "{drawing}"
    );
    assert!(!drawing.contains(r#"name="Day""#), "{drawing}");
    for (label, read) in readers() {
        let mut again = read(&save(&book)).unwrap();
        again.try_read_sheet_collection().unwrap();
        let sheet = again.sheet_by_name("Report").unwrap();
        let timeline = &sheet.timelines()[0];
        assert_eq!(timeline.name, "Sales Days", "{label}");
        assert_eq!(timeline.level, TimelineLevel::Quarters, "{label}");
        assert_eq!(timeline.show_header, Some(false), "{label}");
        assert_eq!(
            timeline.style.as_deref(),
            Some("TimeSlicerStyleDark2"),
            "{label}"
        );
        assert_eq!(
            sheet.worksheet_drawing().slicer_frames()[0].slicer_name(),
            "Sales Days",
            "{label}"
        );
    }
}

#[test]
fn renaming_to_a_taken_name_or_styling_unknown_is_an_error() {
    let mut book = read_eager(&disconnected()).unwrap();
    let before = saved_parts(&book);
    let message = book.rename_timeline("T", "t2").unwrap_err().to_string();
    assert!(message.contains("already exists"), "{message}");
    assert!(book.rename_timeline("Nope", "X").is_err());
    assert_same_parts(&saved_parts(&book), &before, "unchanged");

    book.timeline_mut("T").unwrap().style = Some("NoSuchTimelineStyle".to_owned());
    let message = try_save(&book).unwrap_err().to_string();
    assert!(
        message.contains("NoSuchTimelineStyle") && message.contains("'T'"),
        "{message}"
    );
}

#[test]
fn custom_timeline_style_round_trips_with_its_differential_formats() {
    let original = unpack(&two_pivots());
    let styles = &original["xl/styles.xml"];
    for (label, read) in readers() {
        let book = read(&two_pivots()).unwrap();
        let model = book.timeline_styles().unwrap();
        assert_eq!(model.default_style, "TimeSlicerStyleLight1", "{label}");
        assert_eq!(model.styles.len(), 1, "{label}");
        assert_eq!(model.styles[0].name, "MyTimelineStyle", "{label}");
        assert_eq!(model.styles[0].elements.len(), 7, "{label}");
        if label == "eager" {
            keep_output("timeline-custom-style", &save(&book));
        }
        let written = saved_parts(&book);
        let saved = &written["xl/styles.xml"];
        for element in ["x15:timelineStyles", "x15:dxfs"] {
            assert_eq!(
                without_namespace_declarations(element_text(saved, element)),
                without_namespace_declarations(element_text(styles, element)),
                "{label}: {element}"
            );
        }
        // The table style entry Excel writes next to the timeline style: its
        // count adds the 7 timeline style elements to its own 3. Its xr9:uid
        // is not kept.
        let entries = element_text(styles, "tableStyles");
        let uid = attribute(tags(entries, "<tableStyle")[0], "xr9:uid").unwrap();
        assert_eq!(
            element_text(saved, "tableStyles"),
            entries.replace(&format!(r#" xr9:uid="{uid}""#), ""),
            "{label}"
        );
        assert_eq!(ext_uris(saved), ext_uris(styles), "{label}");
        let timelines = timelines_part(&written);
        assert_eq!(
            attribute(timeline_tag(timelines, "ShipTl"), "style").as_deref(),
            Some("MyTimelineStyle"),
            "{label}"
        );
        assert_eq!(
            attribute(timeline_tag(timelines, "DayTl"), "style"),
            None,
            "{label}"
        );
    }
}

#[test]
fn custom_timeline_slicer_and_table_styles_are_written_together_as_excel_writes_them() {
    // Excel's own save of this package (Excel 16, SaveCopyAs) orders the
    // stylesheet extensions x14:dxfs, x14:slicerStyles, x15:dxfs,
    // x15:timelineStyles, and counts the elements of a slicer or timeline
    // style in the count of its table style entry.
    let mut book = read_eager(&two_pivots()).unwrap();
    let fill = |argb: &str| {
        let mut style = Style::default();
        style.set_background_color(argb);
        style
    };
    book.set_custom_table_style(&TableStyleDefinition {
        name:              "Banded".into(),
        applies_to_tables: true,
        applies_to_pivots: false,
        elements:          vec![TableStyleElementDefinition {
            element_type: TableStyleElementType::HeaderRow,
            stripe_size:  1,
            format:       Some(fill("FF00B050")),
        }],
    })
    .unwrap();
    book.set_custom_slicer_style(&SlicerStyleDefinition {
        name:         "Chips".into(),
        whole_slicer: Some(fill("FFEEEEEE")),
        header:       None,
        items:        vec![SlicerStyleItemDefinition {
            element_type: SlicerStyleElementType::SelectedItemWithData,
            format:       Some(fill("FF0070C0")),
        }],
    })
    .unwrap();
    let output = save(&book);
    keep_output("timeline-slicer-table-styles", &output);
    let styles = &unpack(&output)["xl/styles.xml"];
    assert_eq!(
        ext_uris(styles),
        [
            "{46F421CA-312F-682f-3DD2-61675219B42D}",
            "{EB79DEF2-80B8-43e5-95BD-54CBDDF9020C}",
            "{A0A4C193-F2C1-4fcb-8827-314CF55A85BB}",
            "{9260A510-F301-46a8-8635-F512D64BE5F5}",
        ]
    );
    let counts: Vec<_> = tags(styles, "<tableStyle")
        .into_iter()
        .map(|tag| (attribute(tag, "name"), attribute(tag, "count")))
        .collect();
    let entry = |name: &str, count: &str| (Some(name.to_owned()), Some(count.to_owned()));
    assert_eq!(
        counts,
        [
            entry("MyTimelineStyle", "10"),
            entry("Banded", "1"),
            entry("Chips", "2"),
        ]
    );
    for (label, read) in readers() {
        let mut back = read(&output).unwrap();
        let timeline_styles = back.timeline_styles().unwrap();
        assert_eq!(timeline_styles.styles[0].name, "MyTimelineStyle", "{label}");
        assert_eq!(timeline_styles.styles[0].elements.len(), 7, "{label}");
        assert_eq!(
            back.custom_table_styles(),
            book.custom_table_styles(),
            "{label}"
        );
        assert_eq!(
            back.custom_slicer_styles(),
            book.custom_slicer_styles(),
            "{label}"
        );
        assert_eq!(
            back.timeline_mut("ShipTl").unwrap().style.as_deref(),
            Some("MyTimelineStyle"),
            "{label}"
        );
    }
}

#[test]
fn a_timeline_style_dxf_id_outside_its_list_is_an_error() {
    let broken = repacked(&two_pivots(), |path, text| {
        if path == "xl/styles.xml" {
            replace_once(
                text,
                r#"<x15:timelineStyleElement type="selectionLabel" dxfId="6"/>"#,
                r#"<x15:timelineStyleElement type="selectionLabel" dxfId="7"/>"#,
            );
        }
    });
    let error = read_eager(&broken).unwrap_err().to_string();
    assert!(
        error.contains("timeline style MyTimelineStyle element selectionLabel references dxfId 7"),
        "{error}"
    );
}

#[test]
fn connecting_and_disconnecting_a_pivot_table_follows_excel() {
    let mut book = read_eager(&disconnected()).unwrap();
    assert!(book.timeline_caches()[0].linked_pivots().is_empty());
    // Not connected: the selection is kept by the timeline only.
    book.set_timeline_selection("NativeTimeline_Day", range((2024, 3, 1), (2024, 3, 31)))
        .unwrap();
    assert_eq!(filter_of(&book, "PivotTable1", 2), between(45292, 45351));
    book.connect_timeline_pivot("NativeTimeline_Day", "Report", "PivotTable1")
        .unwrap();
    // Connecting applies the timeline's selection to the pivot table.
    assert_eq!(filter_of(&book, "PivotTable1", 2), between(45352, 45382));
    keep_output("timeline-connected", &save(&book));
    let parts = saved_parts(&book);
    assert!(
        timeline_cache_part(&parts, 1)
            .contains(r#"<pivotTables><pivotTable tabId="2" name="PivotTable1"/></pivotTables>"#)
    );
    let message = book
        .connect_timeline_pivot("NativeTimeline_Day", "Report", "PivotTable1")
        .unwrap_err()
        .to_string();
    assert!(message.contains("already"), "{message}");
    assert_same_parts(&saved_parts(&book), &parts, "unchanged");

    // Disconnecting the only pivot table keeps the cache, its selection and
    // the pivot table's filter.
    book.disconnect_timeline_pivot("NativeTimeline_Day", "Report", "PivotTable1")
        .unwrap();
    keep_output("timeline-disconnected", &save(&book));
    let parts = saved_parts(&book);
    let cache = timeline_cache_part(&parts, 1);
    assert!(!cache.contains("<pivotTables>"), "{cache}");
    assert!(
        cache.contains(
            r#"<selection startDate="2024-03-01T00:00:00" endDate="2024-03-31T00:00:00"/>"#
        )
    );
    assert_eq!(
        filter_text(pivot_part(&parts, "PivotTable1"), 2),
        excel_filter(2, 2, "Day", 45352, Some(45382))
    );
    assert!(
        book.disconnect_timeline_pivot("NativeTimeline_Day", "Report", "PivotTable1")
            .is_err()
    );
    for (label, read) in readers() {
        let mut again = read(&save(&book)).unwrap();
        again.try_read_sheet_collection().unwrap();
        assert!(
            again.timeline_caches()[0].linked_pivots().is_empty(),
            "{label}"
        );
    }
}

#[test]
fn removing_timeline_views_follows_excel() {
    let mut book = read_eager(&disconnected()).unwrap();
    book.remove_timeline("T").unwrap();
    {
        let sheet = book.sheet_by_name("Report").unwrap();
        assert_eq!(
            sheet
                .timelines()
                .iter()
                .map(|t| t.name.as_str())
                .collect::<Vec<_>>(),
            ["T2"]
        );
        assert_eq!(
            sheet
                .worksheet_drawing()
                .slicer_frames()
                .iter()
                .map(|frame| frame.slicer_name())
                .collect::<Vec<_>>(),
            ["T2"]
        );
    }
    keep_output("timeline-one-view-left", &save(&book));
    // The cache stays while a view shows it.
    assert_eq!(book.timeline_caches().len(), 1);
    assert!(
        book.defined_names()
            .iter()
            .any(|name| name.name() == "NativeTimeline_Day")
    );
    assert!(book.remove_timeline("T").is_err());

    // Deleting the last view removes the cache and its defined name; the
    // pivot filter stays (Excel's `p5-delete-last-view`).
    book.remove_timeline("T2").unwrap();
    assert!(book.timeline_caches().is_empty());
    assert!(
        !book
            .defined_names()
            .iter()
            .any(|name| name.name().starts_with("NativeTimeline_"))
    );
    assert_eq!(filter_of(&book, "PivotTable1", 2), between(45292, 45351));
    keep_output("timeline-views-removed", &save(&book));
    let parts = saved_parts(&book);
    assert!(
        parts.keys().all(|path| !path.contains("imeline")),
        "{:?}",
        parts.keys().collect::<Vec<_>>()
    );
    assert!(!parts["xl/workbook.xml"].contains("timelineCache"));
    assert!(!parts[&sheet_part_of(&parts, "Report")].contains("timelineRef"));
    for (label, read) in readers() {
        let mut again = read(&save(&book)).unwrap();
        again.try_read_sheet_collection().unwrap();
        assert!(again.timeline_caches().is_empty(), "{label}");
    }
}

#[test]
fn removing_the_pivot_sheet_removes_its_timelines_like_excel() {
    // Excel's `s-delete-report`: the timeline, its cache and the defined
    // name go with the sheet that holds the pivot table.
    for (label, read) in readers() {
        let mut book = read(&one_timeline()).unwrap();
        book.remove_sheet_by_name("Report").unwrap();
        assert!(book.timeline_caches().is_empty(), "{label}");
        assert!(
            !book
                .defined_names()
                .iter()
                .any(|name| name.name().starts_with("NativeTimeline_")),
            "{label}"
        );
        if label == "eager" {
            keep_output("timeline-pivot-sheet-removed", &save(&book));
        }
        let parts = saved_parts(&book);
        assert!(
            parts.keys().all(|path| !path.contains("imeline")),
            "{label}: {:?}",
            parts.keys().collect::<Vec<_>>()
        );
        let again = read(&save(&book)).unwrap();
        assert!(again.timeline_caches().is_empty(), "{label}");
    }
}

#[test]
fn empty_timeline_extension_left_by_excel_is_kept_without_timelines() {
    // Workbooks Excel saved after the last timeline was deleted still hold the
    // empty `{D0CA8CA8-9F24-4464-BF8E-62219DCF47F9}` extension.
    let bytes = std::fs::read("tests/test_files/issue_286.xlsx").unwrap();
    assert!(unpack(&bytes)["xl/workbook.xml"].contains("{D0CA8CA8-9F24-4464-BF8E-62219DCF47F9}"));
    for (label, read) in readers() {
        let mut book = read(&bytes).unwrap();
        book.try_read_sheet_collection().unwrap();
        assert!(book.timeline_caches().is_empty(), "{label}");
        let written = saved_parts(&book);
        assert!(
            written["xl/workbook.xml"].contains("{D0CA8CA8-9F24-4464-BF8E-62219DCF47F9}"),
            "{label}"
        );
        assert!(
            written.keys().all(|path| !path.starts_with("xl/timeline")),
            "{label}"
        );
    }
}

#[test]
fn slicer_frames_persisted_before_timelines_still_load() {
    // A drawing serialized by a revision without timelines has no `kind` on
    // its slicer frames.
    let book = read_eager(&checked(SLICER_DIRECTORY, "pivot-slicers", "pivot-slicers")).unwrap();
    let drawing = book.sheet_by_name("Report").unwrap().worksheet_drawing();
    let mut persisted = serde_json::to_value(drawing).unwrap();
    let frames = persisted["slicer_frames"].as_array_mut().unwrap();
    assert!(!frames.is_empty());
    for frame in frames {
        assert!(frame.as_object_mut().unwrap().remove("kind").is_some());
    }
    let loaded: umya_spreadsheet::drawing::spreadsheet::WorksheetDrawing =
        serde_json::from_value(persisted).unwrap();
    let frames = loaded.slicer_frames();
    assert!(!frames.is_empty());
    assert!(
        frames
            .iter()
            .all(|frame| frame.kind() == SlicerFrameKind::Slicer)
    );
}

/// `xml` with every element of the main namespace written with the prefix `s`.
fn with_prefixed_main_namespace(xml: &str) -> String {
    let mut output = String::new();
    let mut rest = xml;
    while let Some(at) = rest.find('<') {
        output.push_str(&rest[..=at]);
        rest = &rest[at + 1..];
        let (closing, name_start) = match rest.strip_prefix('/') {
            Some(after) => ("/", after),
            None => ("", rest),
        };
        let name_end = name_start
            .find(|c: char| !c.is_ascii_alphanumeric() && c != ':')
            .unwrap();
        let name = &name_start[..name_end];
        if name.is_empty() || name.contains(':') {
            continue;
        }
        output.push_str(closing);
        output.push_str("s:");
        rest = name_start;
    }
    output.push_str(rest);
    output.replacen(r#"xmlns=""#, r#"xmlns:s=""#, 1)
}

#[test]
fn custom_timeline_style_survives_a_prefixed_stylesheet() {
    let prefixed = repacked(&two_pivots(), |path, text| {
        if path == "xl/styles.xml" {
            *text = with_prefixed_main_namespace(text);
            assert!(
                text.contains("<s:extLst>") && text.contains("<s:dxf>"),
                "{text}"
            );
        }
    });
    let original = unpack(&two_pivots());
    for (label, read) in readers() {
        let book = read(&prefixed).unwrap();
        let model = book.timeline_styles().unwrap();
        assert_eq!(model.styles.len(), 1, "{label}");
        assert_eq!(model.styles[0].name, "MyTimelineStyle", "{label}");
        let written = saved_parts(&book);
        for element in ["x15:timelineStyles", "x15:dxfs"] {
            // The differential formats keep the prefix of the source.
            let saved =
                without_namespace_declarations(element_text(&written["xl/styles.xml"], element))
                    .replace("<s:", "<")
                    .replace("</s:", "</");
            assert_eq!(
                saved,
                without_namespace_declarations(element_text(&original["xl/styles.xml"], element)),
                "{label}: {element}"
            );
        }
        let again = read(&save(&book)).unwrap();
        assert_eq!(
            again.timeline_styles().unwrap().styles[0].name,
            "MyTimelineStyle",
            "{label}"
        );
    }
}

#[test]
fn selection_works_for_a_date_field_named_with_an_ampersand() {
    let renamed = repacked(&one_timeline(), |path, text| match path {
        "xl/pivotCache/pivotCacheDefinition1.xml" => {
            replace_once(
                text,
                r#"<cacheField name="Day""#,
                r#"<cacheField name="Order &amp; Ship""#,
            );
        }
        "xl/timelineCaches/timelineCache1.xml" => {
            replace_once(
                text,
                r#"sourceName="Day""#,
                r#"sourceName="Order &amp; Ship""#,
            );
        }
        _ => (),
    });
    for (label, read) in readers() {
        let mut book = read(&renamed).unwrap();
        book.set_timeline_selection("NativeTimeline_Day", range((2024, 2, 1), (2024, 3, 31)))
            .unwrap();
        let written = saved_parts(&book);
        let pivot_xml = pivot_part(&written, "PivotTable1");
        assert!(
            pivot_xml.contains(r#"name="Order &amp; Ship""#),
            "{label}: {pivot_xml}"
        );
        // Excel's serials of 2024-02-01 and 2024-03-31.
        assert!(
            pivot_xml.contains(r#"val="45323""#) && pivot_xml.contains(r#"val="45382""#),
            "{label}"
        );
        let again = read(&save(&book)).unwrap();
        assert_eq!(
            again.timeline_caches()[0].selection().unwrap(),
            range((2024, 2, 1), (2024, 3, 31)),
            "{label}"
        );
    }
}

#[test]
fn created_timeline_frame_has_the_fallback_shape_excel_writes() {
    // Excel (`f1-fallback-geometry`): a timeline at column C + 104775 EMU, row
    // 8 + 31750 EMU, 4229100 x 1104900 EMU, on a sheet with these column
    // widths and row heights, has a fallback box at the absolute position
    // 1905000, 1651000 of the same size.
    let setup = |book: &mut Workbook| {
        let sheet = book.sheet_by_name_mut("Report").unwrap();
        for (column, width) in [
            ("A", 13.42578125),
            ("B", 13.5703125),
            ("D", 31.28515625),
            ("F", 3.7109375),
        ] {
            sheet.column_dimension_mut(column).set_width(width);
        }
        for (row, height) in [(1, 30.0), (4, 22.5), (9, 39.950000000000003)] {
            sheet.row_dimension_mut(row).set_height(height);
        }
    };
    let mut from = umya_spreadsheet::drawing::spreadsheet::MarkerType::default();
    from.set_col(2)
        .set_col_off(104775)
        .set_row(7)
        .set_row_off(31750);
    let mut to = umya_spreadsheet::drawing::spreadsheet::MarkerType::default();
    to.set_col(7)
        .set_col_off(171450)
        .set_row(11)
        .set_row_off(60325);
    let mut extent = Extent::default();
    extent.set_cx(4_229_100).set_cy(1_104_900);
    for (label, anchor) in [
        (
            "one cell",
            DrawingAnchor::OneCell {
                from: from.clone(),
                extent,
            },
        ),
        (
            "two cell",
            DrawingAnchor::TwoCell {
                from,
                to,
                edit_as: None,
            },
        ),
    ] {
        let mut book = read_eager(&without_timeline()).unwrap();
        book.try_read_sheet_collection().unwrap();
        setup(&mut book);
        let mut request = new_timeline("Day");
        request.anchor = anchor;
        book.add_timeline(&request).unwrap();
        keep_output(
            &format!("timeline-created-fallback-{}", label.replace(' ', "-")),
            &save(&book),
        );
        let drawing = saved_parts(&book)["xl/drawings/drawing1.xml"].clone();
        assert!(
            drawing.contains(
                r#"<a:off x="1905000" y="1651000"></a:off><a:ext cx="4229100" cy="1104900"></a:ext>"#
            ),
            "{label}: {drawing}"
        );
        assert!(
            drawing.contains("<mc:Fallback>")
                && drawing
                    .contains("Timeline: Works in Excel 2013 or higher. Do not move or resize."),
            "{label}: {drawing}"
        );
        let again = read_eager(&save(&book)).unwrap();
        assert_eq!(
            again
                .sheet_by_name("Report")
                .unwrap()
                .worksheet_drawing()
                .slicer_frames()[0]
                .kind(),
            SlicerFrameKind::Timeline,
            "{label}"
        );
    }
}

#[test]
fn removing_a_sheet_unlinks_only_its_pivot_tables_from_caches_linked_on_several_sheets() {
    // Excel (`s-delete-report`, `p5-disconnect-only`): a timeline cache loses
    // the pivot tables of a deleted sheet and goes away with its last one;
    // its views and defined name go with it.
    let mut book = read_eager(&two_pivots()).unwrap();
    book.try_read_sheet_collection().unwrap();
    book.new_sheet("SheetA").unwrap();
    book.new_sheet("SheetB").unwrap();
    let id_of = |book: &Workbook, name: &str| -> u32 {
        book.sheet_by_name(name)
            .unwrap()
            .sheet_id()
            .parse()
            .unwrap()
    };
    let (id_a, id_b) = (id_of(&book, "SheetA"), id_of(&book, "SheetB"));
    let report = book.sheet_by_name_mut("Report").unwrap();
    let second = report.pivot_tables_mut().remove(1);
    let first = report.pivot_tables_mut().remove(0);
    assert_eq!(first.pivot_table_definition().name(), "PivotTable1");
    assert_eq!(second.pivot_table_definition().name(), "PivotTable2");
    book.sheet_by_name_mut("SheetA")
        .unwrap()
        .add_pivot_table(first);
    book.sheet_by_name_mut("SheetB")
        .unwrap()
        .add_pivot_table(second);
    for cache in book.timeline_caches_mut() {
        for link in cache.pivot_tables.iter_mut().flatten() {
            link.sheet_id = if link.name == "PivotTable1" {
                id_a
            } else {
                id_b
            };
        }
    }
    let linked = |book: &Workbook| -> Vec<(String, Vec<String>)> {
        book.timeline_caches()
            .iter()
            .map(|cache| {
                (
                    cache.name.clone(),
                    cache
                        .linked_pivots()
                        .iter()
                        .map(|link| link.name.clone())
                        .collect(),
                )
            })
            .collect()
    };
    assert_eq!(
        linked(&book),
        vec![
            (
                "NativeTimeline_Day".to_owned(),
                vec!["PivotTable1".to_owned()]
            ),
            (
                "NativeTimeline_Ship".to_owned(),
                vec!["PivotTable1".to_owned(), "PivotTable2".to_owned()]
            ),
        ]
    );

    book.remove_sheet_by_name("SheetA").unwrap();
    // `Day` linked only the removed pivot table and goes with its view;
    // `Ship` keeps the pivot table of the other sheet.
    assert_eq!(
        linked(&book),
        vec![(
            "NativeTimeline_Ship".to_owned(),
            vec!["PivotTable2".to_owned()]
        )]
    );
    let names: Vec<_> = book
        .sheet_by_name("Report")
        .unwrap()
        .timelines()
        .iter()
        .map(|timeline| timeline.name.clone())
        .collect();
    assert_eq!(names, ["ShipTl"]);
    assert!(
        book.defined_names()
            .iter()
            .all(|name| name.name() != "NativeTimeline_Day")
    );
    assert!(
        book.defined_names()
            .iter()
            .any(|name| name.name() == "NativeTimeline_Ship")
    );
    let parts = saved_parts(&book);
    assert_eq!(
        parts
            .keys()
            .filter(|path| path.starts_with("xl/timelineCaches/"))
            .count(),
        1
    );

    book.remove_sheet_by_name("SheetB").unwrap();
    assert!(book.timeline_caches().is_empty());
    assert!(book.sheet_by_name("Report").unwrap().timelines().is_empty());
    assert!(
        saved_parts(&book)
            .keys()
            .all(|path| !path.contains("imeline"))
    );
}

#[test]
fn a_selection_and_a_pivot_filter_that_disagree_are_kept_as_excel_keeps_them() {
    // Excel (`m1`, `m2`) opens such a workbook without repair and neither
    // changes the selection nor the pivot filter, so neither does the library.
    let unfiltered = repacked(&one_timeline(), |path, text| {
        if path == "xl/pivotTables/pivotTable1.xml" {
            let start = text.find("<filters").unwrap();
            let end = text.find("</filters>").unwrap() + "</filters>".len();
            text.replace_range(start..end, "");
        }
    });
    let differing = repacked(&one_timeline(), |path, text| {
        if path == "xl/pivotTables/pivotTable1.xml" {
            replace_once(text, r#"val="45351""#, r#"val="45300""#);
        }
    });
    for (label, bytes, filter) in [
        ("pivot unfiltered", unfiltered, None),
        ("pivot filter differs", differing, between(45292, 45300)),
    ] {
        for (reader, read) in readers() {
            let mut book = read(&bytes).unwrap();
            book.try_read_sheet_collection().unwrap();
            assert_eq!(
                book.timeline_caches()[0].selection().unwrap(),
                range((2024, 1, 1), (2024, 2, 29)),
                "{label} {reader}"
            );
            assert_eq!(
                filter_of(&book, "PivotTable1", 3),
                filter,
                "{label} {reader}"
            );
            let mut again = read(&save(&book)).unwrap();
            again.try_read_sheet_collection().unwrap();
            assert_eq!(
                again.timeline_caches()[0].selection().unwrap(),
                range((2024, 1, 1), (2024, 2, 29)),
                "{label} {reader}"
            );
            assert_eq!(
                filter_of(&again, "PivotTable1", 3),
                filter,
                "{label} {reader}"
            );
        }
    }
}

#[test]
fn a_timeline_pivot_filter_needs_its_auto_filter_as_excel_requires() {
    // Excel (`m3`..`m6`) cannot open a cache whose `timelinePivotFilter` has no
    // `autoFilter`; with one it opens and ignores the element.
    let with_filter = |inner: &'static str| {
        repacked(&one_timeline(), move |path, text| {
            if path == "xl/timelineCaches/timelineCache1.xml" {
                replace_once(
                    text,
                    "</state>",
                    &format!(
                        r#"</state><timelinePivotFilter fld="3" id="2" useWholeDay="1">{inner}</timelinePivotFilter>"#
                    ),
                );
            }
        })
    };
    let auto = r#"<autoFilter ref="A1"><filterColumn colId="0"><customFilters and="1"><customFilter operator="greaterThanOrEqual" val="45292"/><customFilter operator="lessThanOrEqual" val="45351"/></customFilters></filterColumn></autoFilter>"#;
    for (label, read) in readers() {
        let mut book = read(&with_filter(auto)).unwrap();
        book.try_read_sheet_collection().unwrap();
        let filter = book.timeline_caches()[0].pivot_filter.as_ref().unwrap();
        assert_eq!((filter.field, filter.id), (3, 2), "{label}");
        let cache = timeline_cache_part(&saved_parts(&book), 1).to_owned();
        assert!(cache.contains("<timelinePivotFilter"), "{label}: {cache}");

        let message = match read(&with_filter("")) {
            Ok(mut book) => book.try_read_sheet_collection().unwrap_err().to_string(),
            Err(error) => error.to_string(),
        };
        assert!(
            message.contains("NativeTimeline_Day") && message.contains("autoFilter"),
            "{label}: {message}"
        );
    }
}

fn linked_names(book: &Workbook) -> Vec<(String, Vec<String>)> {
    book.timeline_caches()
        .iter()
        .map(|cache| {
            (
                cache.name.clone(),
                cache
                    .linked_pivots()
                    .iter()
                    .map(|link| link.name.clone())
                    .collect(),
            )
        })
        .collect()
}

#[test]
fn removing_a_pivot_table_unlinks_it_from_timeline_caches_as_excel_does() {
    // Excel (`d1`, `d3`): deleting a pivot table drops it from the timeline
    // cache that listed it; the cache keeps its views and its selection.
    for (label, read) in readers() {
        let mut book = read(&two_pivots()).unwrap();
        book.try_read_sheet_collection().unwrap();
        assert_eq!(
            linked_names(&book),
            vec![
                (
                    "NativeTimeline_Day".to_owned(),
                    vec!["PivotTable1".to_owned()]
                ),
                (
                    "NativeTimeline_Ship".to_owned(),
                    vec!["PivotTable1".to_owned(), "PivotTable2".to_owned()]
                ),
            ],
            "{label}"
        );
        let selected_before = book.timeline_caches()[0].selection().unwrap();
        book.remove_pivot_table("Report", "PivotTable1").unwrap();
        if label == "eager" {
            keep_output("timeline-pivot-removed", &save(&book));
        }
        assert_eq!(
            linked_names(&book),
            vec![
                ("NativeTimeline_Day".to_owned(), vec![]),
                (
                    "NativeTimeline_Ship".to_owned(),
                    vec!["PivotTable2".to_owned()]
                ),
            ],
            "{label}"
        );
        assert_eq!(
            book.timeline_caches()[0].selection().unwrap(),
            selected_before,
            "{label}"
        );
        let written = saved_parts(&book);
        let day_cache = timeline_cache_part(&written, 1);
        assert!(day_cache.contains("NativeTimeline_Day"), "{label}");
        assert!(!day_cache.contains("<pivotTables>"), "{label}: {day_cache}");
        assert_eq!(
            timelines_part(&written).matches("<timeline ").count(),
            2,
            "{label}"
        );
        assert_eq!(
            written
                .keys()
                .filter(|path| path.starts_with("xl/pivotTables/pivotTable"))
                .count(),
            1,
            "{label}"
        );
        let mut again = read(&save(&book)).unwrap();
        again.try_read_sheet_collection().unwrap();
        assert_eq!(linked_names(&again), linked_names(&book), "{label}");

        // Excel (`d2`) retains the cache definition and records when the last
        // pivot table goes, declaring it in timelineCachePivotCaches instead.
        book.remove_pivot_table("Report", "PivotTable2").unwrap();
        assert!(
            book.sheet_by_name("Report")
                .unwrap()
                .pivot_tables()
                .is_empty()
        );
        assert_eq!(
            book.retained_pivot_caches()
                .map(|(id, definition)| (id, definition.slicer_cache_id()))
                .collect::<Vec<_>>(),
            [(0, Some(2060443253))]
        );
        assert_eq!(
            linked_names(&book),
            [
                ("NativeTimeline_Day".into(), vec![]),
                ("NativeTimeline_Ship".into(), vec![]),
            ]
        );
        let selected = [
            range((2024, 1, 1), (2024, 5, 31)),
            range((2024, 2, 1), (2024, 9, 30)),
        ];
        assert_eq!(
            book.timeline_caches()
                .iter()
                .map(|cache| cache.selection().unwrap())
                .collect::<Vec<_>>(),
            selected
        );
        let normal = save(&book);
        let mut stream = umya_spreadsheet::writer::streaming_writer::StreamingWriter::new(
            zip::ZipWriter::new(Cursor::new(Vec::new())),
            book.clone(),
        );
        for name in ["Data", "Report"] {
            let sheet = stream.take_sheet(name).unwrap();
            stream.flush_sheet(sheet).unwrap();
        }
        let streamed = stream.finish().unwrap().into_inner();
        for (writer, output) in [("normal", normal), ("streaming", streamed)] {
            let parts = unpack(&output);
            assert!(!parts["xl/workbook.xml"].contains("<pivotCaches>"));
            assert!(parts["xl/workbook.xml"].contains(
                r#"<x15:timelineCachePivotCaches><pivotCache cacheId="0" r:id="rId3"/></x15:timelineCachePivotCaches>"#));
            assert!(
                parts
                    .keys()
                    .all(|path| !path.starts_with("xl/pivotTables/"))
            );
            assert_eq!(
                parts["xl/pivotCache/pivotCacheRecords1.xml"],
                unpack(&two_pivots())["xl/pivotCache/pivotCacheRecords1.xml"]
            );
            for (rereader, reread) in readers() {
                let mut again = reread(&output).unwrap();
                again.try_read_sheet_collection().unwrap();
                assert_eq!(
                    again.retained_pivot_caches().count(),
                    1,
                    "{label} {writer} {rereader}"
                );
                assert_eq!(
                    again
                        .timeline_caches()
                        .iter()
                        .map(|cache| cache.selection().unwrap())
                        .collect::<Vec<_>>(),
                    selected
                );
                assert_eq!(again.sheet_by_name("Report").unwrap().timelines().len(), 2);
            }
            keep_output(
                &format!("timeline-all-pivots-removed-{label}-{writer}"),
                &output,
            );
        }
        assert!(
            book.remove_pivot_table("Report", "Missing")
                .unwrap_err()
                .to_string()
                .contains("Missing"),
            "{label}"
        );
    }
}

#[test]
fn renaming_a_pivot_table_keeps_its_timelines_linked_as_excel_does() {
    // Excel (measurement `timelines-rename-first`, PivotTable1 renamed to
    // "Renamed Pivot"): both timeline caches list the new name.
    let expected = [
        r#"<pivotTables><pivotTable tabId="2" name="Renamed Pivot"/></pivotTables>"#,
        r#"<pivotTables><pivotTable tabId="2" name="Renamed Pivot"/><pivotTable tabId="2" name="PivotTable2"/></pivotTables>"#,
    ];
    for (label, read) in readers() {
        let mut book = read(&two_pivots()).unwrap();
        book.rename_pivot_table("Report", "PivotTable1", "Renamed Pivot")
            .unwrap();
        let linked = vec![
            (
                "NativeTimeline_Day".to_owned(),
                vec!["Renamed Pivot".to_owned()],
            ),
            (
                "NativeTimeline_Ship".to_owned(),
                vec!["Renamed Pivot".to_owned(), "PivotTable2".to_owned()],
            ),
        ];
        assert_eq!(linked_names(&book), linked, "{label}");
        let mut stream = umya_spreadsheet::writer::streaming_writer::StreamingWriter::new(
            zip::ZipWriter::new(Cursor::new(Vec::new())),
            book.clone(),
        );
        for name in ["Data", "Report"] {
            let sheet = stream.take_sheet(name).unwrap();
            stream.flush_sheet(sheet).unwrap();
        }
        let streamed = stream.finish().unwrap().into_inner();
        for (writer, output) in [("normal", save(&book)), ("streaming", streamed)] {
            let parts = unpack(&output);
            for (index, pivots) in expected.iter().enumerate() {
                assert!(
                    timeline_cache_part(&parts, index + 1).contains(pivots),
                    "{label} {writer}"
                );
            }
            for (rereader, reread) in readers() {
                let mut again = reread(&output).unwrap();
                again.try_read_sheet_collection().unwrap();
                assert_eq!(linked_names(&again), linked, "{label} {writer} {rereader}");
            }
            keep_output(&format!("timeline-pivot-renamed-{label}-{writer}"), &output);
        }
    }
}

#[test]
fn removing_a_pivot_table_keeps_a_pivot_slicer_cache_left_without_pivot_tables() {
    // Excel (`d4`; measurement `slicers-clear-second`, which clears
    // PivotTable2 of this workbook): a pivot slicer cache left without pivot
    // tables stays with its slicer and its defined name; its part no longer
    // lists pivot tables. The pivot cache stays with PivotTable1.
    let bytes = checked(SLICER_DIRECTORY, "pivot-slicers", "pivot-slicers");
    for (label, read) in readers() {
        let mut book = read(&bytes).unwrap();
        book.try_read_sheet_collection().unwrap();
        let links = |book: &Workbook| -> Vec<(String, Vec<String>)> {
            book.slicer_caches()
                .iter()
                .map(|cache| {
                    (
                        cache.name.clone(),
                        cache
                            .linked_pivots()
                            .iter()
                            .map(|link| link.name.clone())
                            .collect(),
                    )
                })
                .collect()
        };
        let expected = vec![
            ("Slicer_Team".to_owned(), vec!["PivotTable1".to_owned()]),
            ("Slicer_Player".to_owned(), vec![]),
        ];
        book.remove_pivot_table("Report", "PivotTable2").unwrap();
        assert_eq!(links(&book), expected, "{label}");
        let mut caches_of_slicers: Vec<_> = book
            .sheet_by_name("Report")
            .unwrap()
            .slicers()
            .iter()
            .map(|slicer| slicer.cache.clone())
            .collect();
        caches_of_slicers.sort();
        assert_eq!(
            caches_of_slicers,
            ["Slicer_Player", "Slicer_Team"],
            "{label}"
        );
        assert!(
            book.defined_names()
                .iter()
                .any(|name| name.name() == "Slicer_Player"),
            "{label}"
        );
        assert_eq!(book.retained_pivot_caches().count(), 0, "{label}");
        let saved = save(&book);
        let parts = unpack(&saved);
        let player = parts
            .values()
            .find(|xml| {
                xml.contains("<slicerCacheDefinition") && xml.contains("name=\"Slicer_Player\"")
            })
            .unwrap();
        assert!(!player.contains("pivotTables"), "{label}: {player}");
        assert!(
            parts["xl/workbook.xml"].contains("<pivotCaches>"),
            "{label}"
        );
        let mut again = read(&saved).unwrap();
        again.try_read_sheet_collection().unwrap();
        assert_eq!(links(&again), expected, "{label}");
    }
}
