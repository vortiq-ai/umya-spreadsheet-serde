#![cfg(feature = "serde")]

#[path = "support/slicer_package_fixture.rs"]
mod fixture;

use std::io::{
    Cursor,
    Write,
};

use fixture::*;
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
