#![cfg(feature = "serde")]

#[path = "support/slicer_package_fixture.rs"]
mod fixture;

use std::io::{
    Cursor,
    Write,
};

use chrono::NaiveDate;
use fixture::*;
use umya_spreadsheet::{
    NewTimeline,
    TimelineLevel,
    TimelineSelection,
    drawing::spreadsheet::{
        DrawingAnchor,
        Extent,
    },
};
use zip::{
    ZipWriter,
    write::SimpleFileOptions,
};

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!(
        "tests/fixtures/timelines/excel-authored/{name}.xlsx"
    ))
    .unwrap()
}

fn repack(parts: Parts) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, xml) in parts {
        writer
            .start_file(name, SimpleFileOptions::default())
            .unwrap();
        writer.write_all(xml.as_bytes()).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

#[test]
fn timeline_style_names_are_literal_xml_strings() {
    // CT_TimelineStyle.name and CT_TimelineStyles.defaultTimelineStyle are
    // xsd:string, so the Xstring-looking sequence must remain literal.
    const NAME: &str = "_x0041_ & \"Timeline\"";
    const XML_NAME: &str = "_x0041_ &amp; &quot;Timeline&quot;";
    // Excel writes view references as Xstrings while the definitions and
    // default are literal XML strings (measure-style-reference.ps1).
    const XML_REFERENCE: &str = "_x005F_x0041_ &amp; &quot;Timeline&quot;";
    let mut original = unpack(&fixture("two-pivots-custom-style"));
    for (path, xml) in &mut original {
        let name = if path.starts_with("xl/timelines/") {
            XML_REFERENCE
        } else {
            XML_NAME
        };
        *xml = xml.replace("MyTimelineStyle", name).replace(
            "defaultTimelineStyle=\"TimeSlicerStyleLight1\"",
            &format!("defaultTimelineStyle=\"{XML_NAME}\""),
        );
    }
    let bytes = repack(original);
    for (label, read) in readers() {
        let mut book = read(&bytes).unwrap();
        let styles = book.timeline_styles().unwrap();
        assert_eq!(styles.default_style, NAME, "{label}");
        assert_eq!(styles.styles[0].name, NAME, "{label}");
        let view = book.timeline_mut("ShipTl").unwrap();
        assert_eq!(view.style.as_deref(), Some(NAME), "{label}");
        view.caption = Some("Shipping dates".into());
        let saved = save(&book);
        keep_output(&format!("timeline-literal-style-{label}"), &saved);
        let parts = unpack(&saved);
        let stylesheet = &parts["xl/styles.xml"];
        for element in ["<x15:timelineStyle", "<tableStyle"] {
            let named: Vec<_> = tags(stylesheet, element)
                .into_iter()
                .filter_map(|tag| attribute(tag, "name"))
                .collect();
            assert_eq!(named, vec![XML_NAME], "{label}: {element}");
        }
        assert_eq!(
            attribute(
                tags(stylesheet, "<x15:timelineStyles")[0],
                "defaultTimelineStyle"
            )
            .as_deref(),
            Some(XML_NAME),
            "{label}"
        );
        let view_xml = parts
            .values()
            .flat_map(|xml| tags(xml, "<timeline"))
            .find(|tag| attribute(tag, "name").as_deref() == Some("ShipTl"))
            .unwrap();
        assert_eq!(
            attribute(view_xml, "style").as_deref(),
            Some(XML_REFERENCE),
            "{label}"
        );
        let mut again = read(&saved).unwrap();
        assert_eq!(
            again.timeline_styles().unwrap().default_style,
            NAME,
            "{label}"
        );
        assert_eq!(
            again.timeline_styles().unwrap().styles[0].name,
            NAME,
            "{label}"
        );
        let view = again.timeline_mut("ShipTl").unwrap();
        assert_eq!(view.style.as_deref(), Some(NAME), "{label}");
        assert_eq!(view.caption.as_deref(), Some("Shipping dates"), "{label}");
    }
}

#[test]
fn new_timeline_views_use_the_existing_selection_level() {
    // Excel's level mapping: one day is 3 (Days), a whole year is 0 (Years).
    for (end_month, end_day, expected, xml_level) in [
        (1, 1, TimelineLevel::Days, "3"),
        (12, 31, TimelineLevel::Years, "0"),
    ] {
        for (label, read) in readers() {
            let mut book = read(&fixture("pivot-without-timeline")).unwrap();
            let mut extent = Extent::default();
            extent.set_cx(3_810_000).set_cy(1_397_000);
            let mut new = NewTimeline {
                sheet:       "Report",
                name:        "FirstTimeline",
                pivot_sheet: "Report",
                pivot_table: "PivotTable1",
                field:       "Day",
                anchor:      DrawingAnchor::OneCell {
                    from: Default::default(),
                    extent,
                },
            };
            book.add_timeline(&new).unwrap();
            book.set_timeline_selection(
                "NativeTimeline_Day",
                TimelineSelection::Range {
                    start: NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
                    end:   NaiveDate::from_ymd_opt(2024, end_month, end_day).unwrap(),
                },
            )
            .unwrap();
            new.name = "SecondTimeline";
            book.add_timeline(&new).unwrap();
            assert_eq!(
                book.timeline_mut("SecondTimeline").unwrap().selection_level,
                expected,
                "{label}"
            );
            let bytes = save(&book);
            keep_output(
                &format!("timeline-new-view-level-{xml_level}-{label}"),
                &bytes,
            );
            let parts = unpack(&bytes);
            let views: Vec<_> = parts
                .values()
                .flat_map(|xml| tags(xml, "<timeline"))
                .collect();
            assert_eq!(views.len(), 2, "{label}");
            for view in views {
                assert_eq!(
                    attribute(view, "selectionLevel").as_deref(),
                    Some(xml_level),
                    "{label}"
                );
            }
            let mut again = read(&bytes).unwrap();
            assert_eq!(
                again
                    .timeline_mut("SecondTimeline")
                    .unwrap()
                    .selection_level,
                expected,
                "{label}"
            );
            assert_eq!(again.timeline_caches().len(), 1, "{label}");
        }
    }
}
