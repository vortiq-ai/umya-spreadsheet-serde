#![cfg(feature = "serde")]

#[path = "support/chart_xml_fixture.rs"]
pub mod fixture;

use serde_json::Value;
use umya_spreadsheet::{
    Chart,
    Workbook,
};

const FIELD: &str = "<a:fld id=\"{01234567-89AB-CDEF-0123-456789ABCDEF}\" \
                     type=\"slidenum\"><a:t>TRUSTED</a:t></a:fld>";

fn document(content: &str) -> String {
    format!("<c:chartSpace xmlns:c=\"http://schemas.openxmlformats.org/drawingml/2006/chart\" xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\"><c:chart><c:title><c:tx><c:rich><a:bodyPr/><a:lstStyle/><a:p>{content}</a:p></c:rich></c:tx></c:title><c:plotArea><c:lineChart><c:grouping val=\"standard\"/></c:lineChart></c:plotArea></c:chart></c:chartSpace>")
}

fn paragraph(value: &mut Value) -> &mut Value {
    if value.get("run").is_some() && value.get("paragraph_properties").is_some() {
        return value;
    }
    match value {
        Value::Object(map) => {
            for child in map.values_mut() {
                if contains_paragraph(child) {
                    return paragraph(child);
                }
            }
        }
        Value::Array(values) => {
            for child in values {
                if contains_paragraph(child) {
                    return paragraph(child);
                }
            }
        }
        _ => {}
    }
    panic!("missing title paragraph")
}

fn contains_paragraph(value: &Value) -> bool {
    value.get("run").is_some() && value.get("paragraph_properties").is_some()
        || match value {
            Value::Object(map) => map.values().any(contains_paragraph),
            Value::Array(values) => values.iter().any(contains_paragraph),
            _ => false,
        }
}

fn save_chart(book: &mut Workbook, chart: Chart) -> String {
    book.sheet_mut(0).unwrap().chart_collection_mut()[0] = chart;
    let bytes = fixture::write_workbook(book);
    let reread =
        umya_spreadsheet::reader::xlsx::read_reader(std::io::Cursor::new(bytes), true).unwrap();
    fixture::chart_xml(&reread)
}

#[test]
fn returned_projection_uses_only_authoritative_title_fields() {
    for with_field in [true, false] {
        for edit_text in [false, true] {
            let content = format!(
                "<a:r><a:t>Title</a:t></a:r>{}",
                if with_field { FIELD } else { "" }
            );
            let mut book = fixture::imported_workbook(&document(&content));
            let previous = book.sheet(0).unwrap().chart_collection()[0].clone();
            let mut projection = previous.clone();
            projection.strip_export_parts();
            let mut json = serde_json::to_value(&projection).unwrap();
            let p = paragraph(&mut json);
            p["parts"] = serde_json::json!({"properties_present": false, "fields": [{"before_run": 1, "field": {"id": "{01234567-89AB-CDEF-0123-456789ABCDEF}", "kind": "slidenum", "run_properties": null, "paragraph_properties": null, "text": "FORGED"}}]});
            if edit_text {
                p["run"][0]["text"] = "Edited".into();
            }
            let mut current: Chart = serde_json::from_value(json).unwrap();
            current.restore_export_parts(&previous).unwrap();
            let xml = save_chart(&mut book, current);
            assert!(!xml.contains("FORGED"), "{xml}");
            assert_eq!(xml.contains("TRUSTED"), with_field, "{xml}");
            assert!(xml.contains(if edit_text {
                ">Edited</a:t>"
            } else {
                ">Title</a:t>"
            }));
        }
    }
}

#[test]
fn paragraph_breaks_preserve_properties_and_order_through_projection_and_persistence() {
    let content = format!(
        "<a:r><a:t>North</a:t></a:r><a:br><a:rPr b=\"1\" \
         u=\"dbl\"/></a:br>{FIELD}<a:br/><a:r><a:t>South</a:t></a:r><a:br/>"
    );
    let mut book = fixture::imported_workbook(&document(&content));
    let previous = book.sheet(0).unwrap().chart_collection()[0].clone();
    let mut projection = previous.clone();
    projection.strip_export_parts();
    let mut json = serde_json::to_value(projection).unwrap();
    paragraph(&mut json)["breaks"][0]["run_properties"]["parts"] = forged_break_parts();
    let mut current: Chart = serde_json::from_value(json).unwrap();
    current.restore_export_parts(&previous).unwrap();
    let xml = save_chart(&mut book, current);
    let north = xml.find(">North</a:t>").unwrap();
    let first_break = xml[north..]
        .find("<a:br")
        .map(|i| north + i)
        .expect("break after North");
    let field = xml.find("<a:fld").unwrap();
    let second_break = xml[field..]
        .find("<a:br")
        .map(|i| field + i)
        .expect("break after field");
    let south = xml.find(">South</a:t>").unwrap();
    assert!(
        north < first_break && first_break < field && field < second_break && second_break < south
    );
    assert_eq!(xml.matches("<a:br").count(), 3, "{xml}");
    assert!(xml[first_break..field].contains("b=\"1\""), "{xml}");
    assert!(xml[first_break..field].contains("u=\"dbl\""), "{xml}");
}

#[test]
fn paragraphs_from_older_json_without_breaks_remain_readable() {
    let mut book = fixture::imported_workbook(&document("<a:r><a:t>Title</a:t></a:r>"));
    let previous = book.sheet(0).unwrap().chart_collection()[0].clone();
    let mut json = serde_json::to_value(&previous).unwrap();
    paragraph(&mut json)
        .as_object_mut()
        .unwrap()
        .remove("breaks");
    let chart: Chart = serde_json::from_value(json).unwrap();
    let xml = save_chart(&mut book, chart);
    assert!(xml.contains(">Title</a:t>"));
    assert!(!xml.contains("<a:br"));
}

#[test]
fn break_edits_survive_authoritative_metadata_restoration() {
    let mut book = fixture::imported_workbook(&document(
        "<a:r><a:t>North</a:t></a:r><a:br><a:rPr b=\"1\" \
         u=\"dbl\"/></a:br><a:r><a:t>South</a:t></a:r><a:br/>",
    ));
    let previous = book.sheet(0).unwrap().chart_collection()[0].clone();
    let mut projection = previous.clone();
    projection.strip_export_parts();
    let mut json = serde_json::to_value(projection).unwrap();
    let p = paragraph(&mut json);
    let breaks = p["breaks"]
        .as_array_mut()
        .expect("editable break collection");
    breaks.pop();
    breaks[0]["run_properties"]["bold"]["value"] = "0".into();
    breaks[0]["run_properties"]["parts"] = forged_break_parts();
    let mut chart: Chart = serde_json::from_value(json).unwrap();
    chart.restore_export_parts(&previous).unwrap();
    let xml = save_chart(&mut book, chart);
    assert_eq!(xml.matches("<a:br").count(), 1, "{xml}");
    let start = xml.find("<a:br").unwrap();
    let end = xml[start..].find("</a:br>").map(|i| start + i).unwrap();
    assert!(xml[start..end].contains("b=\"0\""), "{xml}");
    assert!(
        !xml[start..end].contains("u="),
        "changed break topology cannot restore hidden formatting"
    );
}

fn forged_break_parts() -> Value {
    serde_json::json!({
        "underline": "Single",
        "underline_uses_text_fill": false,
        "kerning": null,
        "baseline": null,
        "invalid_attributes": 0,
    })
}

#[test]
fn invalid_break_children_and_misplaced_breaks_are_rejected() {
    let mut book = umya_spreadsheet::new_file();
    book.sheet_mut(0).unwrap().add_chart(fixture::base_chart());
    let bytes = fixture::write_workbook(&book);
    for content in [
        "<a:br><a:rPr/><a:rPr/></a:br>",
        "<a:br><a:t>invalid</a:t></a:br>",
        "<a:endParaRPr/><a:br/>",
        "<a:br/><a:pPr/>",
    ] {
        let input = fixture::replace_chart_part(&bytes, &document(content));
        assert!(
            umya_spreadsheet::reader::xlsx::read_reader(std::io::Cursor::new(input), true).is_err(),
            "{content}"
        );
    }
}

#[test]
fn invalid_serialized_break_positions_fail_export_explicitly() {
    let mut book = fixture::imported_workbook(&document("<a:r><a:t>Title</a:t></a:r><a:br/>"));
    let previous = book.sheet(0).unwrap().chart_collection()[0].clone();
    let mut json = serde_json::to_value(&previous).unwrap();
    paragraph(&mut json)["breaks"][0]["before_run"] = u32::MAX.into();
    book.sheet_mut(0).unwrap().chart_collection_mut()[0] = serde_json::from_value(json).unwrap();
    let result =
        umya_spreadsheet::writer::xlsx::write_writer(&book, std::io::Cursor::new(Vec::new()));
    assert!(result.is_err());
}

#[test]
fn invalid_serialized_field_positions_fail_export_explicitly() {
    for positions in [vec![1, 0], vec![2]] {
        let mut book =
            fixture::imported_workbook(&document(&format!("<a:r><a:t>Title</a:t></a:r>{FIELD}")));
        let previous = book.sheet(0).unwrap().chart_collection()[0].clone();
        let mut json = serde_json::to_value(&previous).unwrap();
        let p = paragraph(&mut json);
        let field = p["parts"]["fields"][0].clone();
        p["parts"]["fields"] = positions
            .iter()
            .map(|position| {
                let mut field = field.clone();
                field["before_run"] = (*position).into();
                field
            })
            .collect();
        book.sheet_mut(0).unwrap().chart_collection_mut()[0] =
            serde_json::from_value(json).unwrap();
        let result =
            umya_spreadsheet::writer::xlsx::write_writer(&book, std::io::Cursor::new(Vec::new()));
        assert!(
            result.is_err(),
            "positions {positions:?} must fail instead of dropping fields"
        );
    }
}

fn paragraphs(value: &mut Value) -> &mut Vec<Value> {
    if value
        .get("paragraph")
        .is_some_and(|p| p[0].get("run").is_some())
    {
        return value["paragraph"].as_array_mut().unwrap();
    }
    match value {
        Value::Object(map) => map
            .values_mut()
            .find(|child| contains_paragraph(child))
            .map(paragraphs)
            .expect("rich text paragraphs"),
        Value::Array(values) => values
            .iter_mut()
            .find(|child| contains_paragraph(child))
            .map(paragraphs)
            .expect("rich text paragraphs"),
        _ => panic!("rich text paragraphs"),
    }
}

#[test]
fn an_added_paragraph_never_carries_forged_hidden_parts() {
    let mut book =
        fixture::imported_workbook(&document(&format!("<a:r><a:t>Title</a:t></a:r>{FIELD}")));
    let previous = book.sheet(0).unwrap().chart_collection()[0].clone();
    let mut projection = previous.clone();
    projection.strip_export_parts();
    let mut json = serde_json::to_value(&projection).unwrap();
    let list = paragraphs(&mut json);
    let mut added = list[0].clone();
    added["parts"] = serde_json::json!({"properties_present": false, "fields": [{"before_run": 1, "field": {"id": "{01234567-89AB-CDEF-0123-456789ABCDEF}", "kind": "slidenum", "run_properties": null, "paragraph_properties": null, "text": "FORGED"}}]});
    added["run"][0]["text"] = "Added".into();
    list.push(added);
    let mut current: Chart = serde_json::from_value(json).unwrap();
    current.restore_export_parts(&previous).unwrap();
    let xml = save_chart(&mut book, current);
    assert!(!xml.contains("FORGED"), "{xml}");
    assert!(xml.contains(">Added</a:t>"), "{xml}");
    assert_eq!(xml.matches("TRUSTED").count(), 1, "{xml}");
}
