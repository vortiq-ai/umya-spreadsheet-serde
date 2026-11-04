#![cfg(feature = "serde")]

use std::io::{
    Cursor,
    Read,
    Write,
};

use umya_spreadsheet::{
    Chart,
    Workbook,
    drawing::{
        charts::ChartExSpace,
        spreadsheet::{
            DrawingObject,
            ExtendedChartFrame,
            GroupShape,
            TwoCellAnchor,
        },
    },
};

#[path = "support/chart_xml_fixture.rs"]
pub mod fixture;

fn document() -> String {
    let shape = "<cx:spPr><a:solidFill><a:srgbClr val=\"123456\"/></a:solidFill><a:extLst><a:ext \
                 uri=\"urn:trusted-shape\"><u:payload xmlns:u=\"urn:test\">TRUSTED \
                 SHAPE</u:payload></a:ext></a:extLst></cx:spPr>";
    let extension = "<cx:extLst><cx:ext uri=\"urn:trusted\"><u:payload \
                     xmlns:u=\"urn:test\">TRUSTED</u:payload></cx:ext></cx:extLst>";
    format!(
        r#"<cx:chartSpace xmlns:cx="http://schemas.microsoft.com/office/drawing/2014/chartex" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:u="urn:test"><cx:chartData><cx:data id="7"><cx:strDim type="cat"><cx:f>Sheet1!$A$1</cx:f><cx:lvl ptCount="1"><cx:pt idx="0">TRUSTED LABEL</cx:pt></cx:lvl></cx:strDim>{extension}</cx:data>{extension}</cx:chartData><cx:chart><cx:title><cx:tx><cx:rich><a:bodyPr/><a:lstStyle/><a:p><a:r><a:t>North</a:t></a:r><a:br><a:rPr b="1"/></a:br><a:r><a:t>South</a:t></a:r><a:fld id="{{01234567-0123-0123-0123-0123456789AB}}"><a:t>TRUSTED FIELD</a:t></a:fld></a:p></cx:rich></cx:tx>{shape}{extension}</cx:title><cx:plotArea><cx:plotAreaRegion><cx:series layoutId="waterfall" uniqueId="series-one"><cx:dataId val="7"/>{extension}</cx:series>{extension}</cx:plotAreaRegion>{extension}</cx:plotArea>{extension}</cx:chart>{extension}</cx:chartSpace>"#
    )
}

fn book_with(chart: Chart) -> Workbook {
    let mut book = umya_spreadsheet::new_file();
    book.sheet_mut(0).unwrap().cell_mut("A1").set_value("live");
    book.sheet_mut(0).unwrap().add_chart(chart);
    book
}

fn imported() -> Chart {
    imported_document(&document())
}

fn imported_document(xml: &str) -> Chart {
    let mut extended = ExtendedChartFrame::default();
    extended
        .frame_properties_mut()
        .non_visual_drawing_properties_mut()
        .set_id(17)
        .set_name("Extended");
    extended.set_chart_space(ChartExSpace::from_xml(xml.as_bytes()).unwrap());
    let standard = fixture::base_chart()
        .two_cell_anchor()
        .graphic_frame()
        .unwrap()
        .clone();
    let mut group = GroupShape::default();
    group
        .non_visual_group_shape_properties_mut()
        .non_visual_drawing_properties_mut()
        .set_id(100)
        .set_name("Group");
    group.add_object(DrawingObject::GraphicFrame(Box::new(standard)));
    group.add_object(DrawingObject::ExtendedFrame(Box::new(extended)));
    let mut anchor = TwoCellAnchor::default();
    anchor.set_group_shape(group);
    let mut chart = Chart::default();
    chart.set_two_cell_anchor(anchor);
    let mut archive =
        zip::ZipArchive::new(Cursor::new(fixture::write_workbook(&book_with(chart)))).unwrap();
    let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let mut replaced = false;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).unwrap();
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).unwrap();
        if entry.name().unwrap().starts_with("xl/charts/chartEx")
            && entry.name().unwrap().ends_with(".xml")
        {
            bytes = xml.as_bytes().to_vec();
            replaced = true;
        }
        output
            .start_file(
                entry.name().unwrap(),
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
        output.write_all(&bytes).unwrap();
    }
    assert!(replaced);
    let book = umya_spreadsheet::reader::xlsx::read_reader(
        Cursor::new(output.finish().unwrap().into_inner()),
        true,
    )
    .unwrap();
    let mut chart = Chart::default();
    chart.set_two_cell_anchor(
        book.sheet(0)
            .unwrap()
            .worksheet_drawing()
            .image_collection()[0]
            .two_cell_anchor()
            .unwrap()
            .clone(),
    );
    chart
}

fn extended_json(chart: &Chart) -> serde_json::Value {
    let mut value = None;
    chart
        .two_cell_anchor()
        .visit_extended_frames(&mut |_, frame| {
            value = Some(serde_json::to_value(frame.chart_space()).unwrap())
        });
    value.unwrap()
}

#[test]
fn grouped_extended_frames_strip_every_hidden_owner() {
    let mut chart = imported();
    chart.strip_export_parts();
    let json = extended_json(&chart).to_string();
    assert!(
        !json.contains("TRUSTED"),
        "ChartEx metadata leaked into projection"
    );
    assert!(
        !json.contains("formula_guard"),
        "ChartEx source cache leaked into projection"
    );
}

#[test]
fn grouped_extended_frames_restore_authority_and_editable_line_breaks() {
    let previous = imported();
    let trusted = extended_json(&previous);
    let mut projection = previous.clone();
    projection.strip_export_parts();
    let mut incoming = serde_json::to_value(&projection).unwrap();
    fn forge(value: &mut serde_json::Value, trusted: &serde_json::Value) {
        match (value, trusted) {
            (serde_json::Value::Object(current), serde_json::Value::Object(old)) => {
                for (key, old) in old {
                    if matches!(key.as_str(), "extensions" | "parts" | "source_cache")
                        && !old.is_null()
                    {
                        let forged = serde_json::to_string(old)
                            .unwrap()
                            .replace("TRUSTED", "FORGED");
                        current.insert(key.clone(), serde_json::from_str(&forged).unwrap());
                    } else if let Some(current) = current.get_mut(key) {
                        forge(current, old);
                    }
                }
            }
            (serde_json::Value::Array(current), serde_json::Value::Array(old)) => {
                for (current, old) in current.iter_mut().zip(old) {
                    forge(current, old);
                }
            }
            _ => (),
        }
    }
    forge(&mut incoming, &serde_json::to_value(&previous).unwrap());
    let mut current: Chart = serde_json::from_value(incoming).unwrap();
    current
        .two_cell_anchor_mut()
        .group_shape_mut()
        .unwrap()
        .content_mut()
        .reverse();
    current.restore_export_parts(&previous).unwrap();
    assert_eq!(extended_json(&current), trusted);
    let book = book_with(current);
    let bytes = fixture::write_workbook(&book);
    let reread =
        umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(bytes.clone()), true).unwrap();
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut xml = String::new();
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).unwrap();
        if entry.name().unwrap().starts_with("xl/charts/chartEx")
            && entry.name().unwrap().ends_with(".xml")
        {
            entry.read_to_string(&mut xml).unwrap();
        }
    }
    assert!(xml.contains("TRUSTED FIELD"));
    assert!(xml.contains("TRUSTED LABEL"));
    assert!(!xml.contains("FORGED"));
    let north = xml.find(">North</a:t>").unwrap();
    let line_break = xml.find("<a:br").expect("explicit title line break");
    let south = xml.find(">South</a:t>").unwrap();
    assert!(north < line_break && line_break < south);
    let mut count = 0;
    reread
        .sheet(0)
        .unwrap()
        .worksheet_drawing()
        .visit_extended_frames(&mut |_, _| count += 1);
    assert_eq!(count, 1);
}

#[test]
fn rejected_extended_frame_identity_discards_every_incoming_hidden_owner() {
    let previous = imported();
    let forged = serde_json::to_string(&previous)
        .unwrap()
        .replace("TRUSTED", "FORGED");
    let mut current: Chart = serde_json::from_str(&forged).unwrap();
    current
        .two_cell_anchor_mut()
        .visit_extended_frames_mut(&mut |_, frame| {
            frame
                .frame_properties_mut()
                .non_visual_drawing_properties_mut()
                .set_id(18);
        });
    let report = current.restore_export_parts(&previous).unwrap();
    assert!(report.incompatible_owners > 0);
    assert!(!extended_json(&current).to_string().contains("FORGED"));
    assert!(
        !extended_json(&current)
            .to_string()
            .contains("formula_guard")
    );
    let bytes = fixture::write_workbook(&book_with(current));
    let book = umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap();
    book.sheet(0)
        .unwrap()
        .worksheet_drawing()
        .visit_extended_frames(&mut |_, frame| {
            assert!(
                !serde_json::to_string(frame.chart_space())
                    .unwrap()
                    .contains("FORGED")
            );
        });
}

#[test]
fn dependent_series_owner_positions_do_not_establish_metadata_identity() {
    let xml = document();
    let start = xml.find("<cx:series ").unwrap();
    let end = xml[start..].find("</cx:series>").unwrap() + start + "</cx:series>".len();
    let owner = &xml[start..end];
    let dependent = |index: u32, marker: &str| {
        owner
            .replace("uniqueId=\"series-one\"", &format!("ownerIdx=\"{index}\""))
            .replace("TRUSTED", marker)
    };
    let series = format!(
        "{}{}{}{}",
        owner.replace("series-one", "owner-one"),
        owner.replace("series-one", "owner-two"),
        dependent(0, "FIRST"),
        dependent(1, "SECOND")
    );
    let previous = imported_document(&format!("{}{}{}", &xml[..start], series, &xml[end..]));
    let mut projection = previous.clone();
    projection.strip_export_parts();
    let mut current: Chart =
        serde_json::from_value(serde_json::to_value(projection).unwrap()).unwrap();
    current
        .two_cell_anchor_mut()
        .visit_extended_frames_mut(&mut |_, frame| {
            let series = &mut frame
                .chart_space_mut()
                .chart
                .as_mut()
                .unwrap()
                .plot_area
                .as_mut()
                .unwrap()
                .plot_area_region
                .as_mut()
                .unwrap()
                .series;
            series.swap(0, 1);
            series[2].owner_idx = Some(1);
            series[3].owner_idx = Some(0);
        });
    let report = current.restore_export_parts(&previous).unwrap();
    assert!(report.incompatible_owners >= 2);
    current
        .two_cell_anchor()
        .visit_extended_frames(&mut |_, frame| {
            let series = &frame
                .chart_space()
                .chart
                .as_ref()
                .unwrap()
                .plot_area
                .as_ref()
                .unwrap()
                .plot_area_region
                .as_ref()
                .unwrap()
                .series;
            assert!(series[2].extensions.is_none());
            assert!(series[3].extensions.is_none());
        });
    let bytes = fixture::write_workbook(&book_with(current));
    let book = umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap();
    book.sheet(0)
        .unwrap()
        .worksheet_drawing()
        .visit_extended_frames(&mut |_, frame| {
            let json = serde_json::to_string(frame.chart_space()).unwrap();
            assert!(!json.contains("FIRST"));
            assert!(!json.contains("SECOND"));
        });
}
