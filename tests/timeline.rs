#![cfg(feature = "serde")]
//! Timelines (date filters on pivot tables) through the public entry points:
//! the readers, direct model edits and the writer. Every expected value comes
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
    drawing::spreadsheet::SlicerFrameKind,
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

// ---- editing views and styles -----------------------------------------------

#[test]
fn view_properties_and_level_persist() {
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
        timeline.level = TimelineLevel::Quarters;
    }
    keep_output("timeline-view-edited", &save(&book));
    let parts = saved_parts(&book);
    // Excel's attribute order (`t1-quarters-range`).
    assert_eq!(
        timeline_tag(timelines_part(&parts), "Day"),
        concat!(
            r#"<timeline name="Day" xr10:uid="{84ED1F61-60F8-4FE3-9D19-2AA0C92D8A48}" "#,
            r#"cache="NativeTimeline_Day" caption="My Days" showHeader="0" "#,
            r#"showSelectionLabel="0" showTimeLevel="0" showHorizontalScrollbar="0" "#,
            r#"level="1" selectionLevel="2" scrollPosition="2024-02-01T00:00:00" "#,
            r#"style="TimeSlicerStyleDark2"/>"#
        )
    );
    for (label, read) in readers() {
        let mut again = read(&save(&book)).unwrap();
        again.try_read_sheet_collection().unwrap();
        let sheet = again.sheet_by_name("Report").unwrap();
        let timeline = &sheet.timelines()[0];
        assert_eq!(timeline.name, "Day", "{label}");
        assert_eq!(timeline.level, TimelineLevel::Quarters, "{label}");
        assert_eq!(timeline.show_header, Some(false), "{label}");
        assert_eq!(
            timeline.style.as_deref(),
            Some("TimeSlicerStyleDark2"),
            "{label}"
        );
    }
}

#[test]
fn an_unknown_timeline_style_is_an_error() {
    let mut book = read_eager(&disconnected()).unwrap();
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
