#![cfg(feature = "serde")]

use umya_spreadsheet::{
    Chart,
    drawing::spreadsheet::{
        DrawingObject,
        GroupShape,
        TwoCellAnchor,
    },
};

#[path = "support/chart_xml_fixture.rs"]
pub mod fixture;

fn document() -> String {
    r#"<c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:u="urn:test"><c:chart><c:title><c:tx><c:rich><a:bodyPr/><a:lstStyle><a:lvl1pPr><a:defRPr><a:solidFill><a:prstClr val="red"/></a:solidFill></a:defRPr></a:lvl1pPr></a:lstStyle><a:p><a:r><a:t>Title</a:t></a:r></a:p></c:rich></c:tx></c:title><c:plotArea><c:lineChart><c:ser><c:idx val="1"/><c:order val="0"/><c:dPt><c:idx val="9"/></c:dPt><c:cat><c:strRef><c:f>Sheet1!$A$1</c:f><c:strCache><c:ptCount val="1"/><c:pt idx="0"><c:v>TRUSTED</c:v></c:pt></c:strCache></c:strRef></c:cat><c:val><c:numRef><c:f>Sheet1!$B$1</c:f></c:numRef></c:val></c:ser></c:lineChart></c:plotArea></c:chart><c:extLst><c:ext uri="urn:trusted"><u:payload>TRUSTED</u:payload></c:ext></c:extLst></c:chartSpace>"#.into()
}

fn grouped() -> Chart {
    let imported = fixture::imported_chart(&document());
    let mut group = GroupShape::default();
    group
        .non_visual_group_shape_properties_mut()
        .non_visual_drawing_properties_mut()
        .set_id(100)
        .set_name("Group");
    group.add_object(DrawingObject::GraphicFrame(Box::new(
        imported.two_cell_anchor().graphic_frame().unwrap().clone(),
    )));
    let mut anchor = TwoCellAnchor::default();
    anchor.set_group_shape(group);
    let mut chart = Chart::default();
    chart.set_two_cell_anchor(anchor);
    let mut book = umya_spreadsheet::new_file();
    book.sheet_mut(0).unwrap().add_chart(chart);
    let imported = umya_spreadsheet::reader::xlsx::read_reader(
        std::io::Cursor::new(fixture::write_workbook(&book)),
        true,
    )
    .unwrap();
    let mut chart = Chart::default();
    chart.set_two_cell_anchor(
        imported
            .sheet(0)
            .unwrap()
            .worksheet_drawing()
            .image_collection()[0]
            .two_cell_anchor()
            .unwrap()
            .clone(),
    );
    chart
}

fn roundtrip(chart: Chart) -> String {
    let mut book = umya_spreadsheet::new_file();
    book.sheet_mut(0).unwrap().add_chart(chart);
    let bytes = fixture::write_workbook(&book);
    let reread =
        umya_spreadsheet::reader::xlsx::read_reader(std::io::Cursor::new(bytes), true).unwrap();
    fixture::chart_xml(&reread)
}

#[test]
fn grouped_frames_strip_hidden_parts_and_restore_trusted_caches() {
    let previous = grouped();
    let mut current = previous.clone();
    current.strip_export_parts();
    fn assert_hidden_slots_empty(value: &serde_json::Value) {
        match value {
            serde_json::Value::Object(fields) => {
                for (key, value) in fields {
                    if matches!(
                        key.as_str(),
                        "parts" | "extensions" | "source" | "source_parts"
                    ) {
                        assert!(
                            value.is_null(),
                            "grouped export slot {key} survived: {value}"
                        );
                    }
                    assert_hidden_slots_empty(value);
                }
            }
            serde_json::Value::Array(values) => {
                for value in values {
                    assert_hidden_slots_empty(value);
                }
            }
            _ => (),
        }
    }
    assert_hidden_slots_empty(&serde_json::to_value(&current).unwrap());
    // Forged cache counts are discarded before the writer can expand them.
    let mut incoming = serde_json::to_value(&previous).unwrap();
    fn forge(value: &mut serde_json::Value) {
        match value {
            serde_json::Value::Object(fields) => {
                for (key, value) in fields {
                    if key == "point_count" {
                        *value = serde_json::json!(u32::MAX);
                    } else {
                        forge(value);
                    }
                }
            }
            serde_json::Value::Array(values) => {
                for value in values {
                    forge(value);
                }
            }
            serde_json::Value::String(value) => *value = value.replace("TRUSTED", "FORGED"),
            _ => (),
        }
    }
    forge(&mut incoming);
    current = serde_json::from_value(incoming).unwrap();
    current.restore_export_parts(&previous).unwrap();
    assert!(
        !serde_json::to_string(&current)
            .unwrap()
            .contains("4294967295")
    );
    let xml = roundtrip(current);
    assert!(xml.contains("TRUSTED"));
    assert!(!xml.contains("FORGED"));
    assert!(xml.contains("<c:ptCount val=\"1\""));
}

#[test]
fn rejected_frame_owners_discard_all_incoming_hidden_parts() {
    let supplied = fixture::imported_chart(
        &document()
            .replace("TRUSTED", "FORGED")
            .replace("val=\"9\"", "val=\"77\""),
    );
    let mut previous = fixture::imported_chart(&document());
    previous.chart_space_mut().set_extensions(None);
    previous
        .area_chart_series_list_mut()
        .area_chart_series_mut()[0]
        .data_points_mut()
        .clear();
    for missing_previous in [false, true] {
        let mut current: Chart =
            serde_json::from_value(serde_json::to_value(&supplied).unwrap()).unwrap();
        current
            .two_cell_anchor_mut()
            .graphic_frame_mut()
            .unwrap()
            .non_visual_graphic_frame_properties_mut()
            .non_visual_drawing_properties_mut()
            .set_id(999);
        let basis = if missing_previous {
            Chart::default()
        } else {
            previous.clone()
        };
        let report = current.restore_export_parts(&basis).unwrap();
        assert_eq!(report.incompatible_owners, 1);
        assert!(
            current.chart_space().extensions().is_none(),
            "rejected root extension survived"
        );
        assert!(
            current.area_chart_series_list_mut().area_chart_series_mut()[0]
                .data_points()
                .is_empty(),
            "rejected series supplement survived"
        );
        let xml = roundtrip(current);
        assert!(!xml.contains("urn:trusted"));
        assert!(!xml.contains("FORGED"));
        assert!(!xml.contains("<c:dPt>"));
    }
}

#[test]
fn restoration_errors_leave_receiver_and_authority_unchanged() {
    let previous = fixture::imported_chart(&document());
    let mut projection = previous.clone();
    projection.strip_export_parts();
    let mut json = serde_json::to_value(&projection).unwrap();
    let rich = &mut json["two_cell_anchor"]["graphic_frame"]["graphic"]["graphic_data"]
        ["chart_space"]["chart"]["title"]["chart_text"]["rich_text"];
    assert!(rich.is_object(), "rich title path");
    rich["list_style"]["text_paragraph_properties_type"]["lv1"]["default_run_properties"]
        ["solid_fill"]["preset_color"]["val"] = serde_json::json!("not-a-preset");
    let mut current: Chart = serde_json::from_value(json).unwrap();
    let before = serde_json::to_vec(&current).unwrap();
    let authority = serde_json::to_vec(&previous).unwrap();
    assert!(current.restore_export_parts(&previous).is_err());
    assert_eq!(serde_json::to_vec(&current).unwrap(), before);
    assert_eq!(serde_json::to_vec(&previous).unwrap(), authority);
}
