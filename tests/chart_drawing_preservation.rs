#![cfg(feature = "serde")]
use std::io::{
    Cursor,
    Read,
    Write,
};

use umya_spreadsheet::{
    drawing::charts::ChartDrawingPoint,
    reader,
    writer,
};

const FIXTURE: &[u8] = include_bytes!("fixtures/chart_drawing_excel.xlsx");
const ROOT: &str = "<c:userShapes xmlns:c=\"http://schemas.openxmlformats.org/drawingml/2006/chart\" xmlns:cdr=\"http://schemas.openxmlformats.org/drawingml/2006/chartDrawing\" xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\">";
const FROM_TO: &str = "<cdr:from><cdr:x>0</cdr:x><cdr:y>0</cdr:y></cdr:from><cdr:to><cdr:x>1</cdr:\
                       x><cdr:y>1</cdr:y></cdr:to>";

fn package(object: &str) -> Vec<u8> {
    package_objects(&[object])
}

fn package_objects(objects: &[&str]) -> Vec<u8> {
    let anchors: String = objects
        .iter()
        .map(|object| format!("<cdr:relSizeAnchor>{FROM_TO}{object}</cdr:relSizeAnchor>"))
        .collect();
    let xml = format!("{ROOT}{anchors}</c:userShapes>");
    let mut input = zip::ZipArchive::new(Cursor::new(FIXTURE)).unwrap();
    let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for index in 0..input.len() {
        let mut file = input.by_index(index).unwrap();
        let name = file.name().unwrap().to_string();
        let mut data = Vec::new();
        file.read_to_end(&mut data).unwrap();
        if name.starts_with("xl/drawings/drawing")
            && String::from_utf8_lossy(&data).contains("userShapes")
        {
            data = xml.as_bytes().to_vec();
        }
        output
            .start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        output.write_all(&data).unwrap();
    }
    output.finish().unwrap().into_inner()
}

fn drawing_xml(bytes: &[u8]) -> String {
    let mut input = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    for index in 0..input.len() {
        let mut file = input.by_index(index).unwrap();
        if file.name().unwrap().starts_with("xl/drawings/drawing") {
            let mut data = String::new();
            file.read_to_string(&mut data).unwrap();
            if data.contains("userShapes") {
                return data;
            }
        }
    }
    panic!("chart drawing missing")
}

fn shape(id: u32) -> String {
    format!(
        "<cdr:sp><cdr:nvSpPr><cdr:cNvPr id=\"{id}\" name=\"Shape \
         {id}\"/><cdr:cNvSpPr/></cdr:nvSpPr><cdr:spPr><a:prstGeom \
         prst=\"rect\"><a:avLst/></a:prstGeom></cdr:spPr></cdr:sp>"
    )
}

#[test]
fn added_shapes_reserve_ids_of_nested_group_children() {
    let object = format!(
        "<cdr:grpSp><cdr:nvGrpSpPr><cdr:cNvPr id=\"2\" \
         name=\"Group\"/><cdr:cNvGrpSpPr/></cdr:nvGrpSpPr><cdr:grpSpPr/>{}<cdr:grpSp><cdr:\
         nvGrpSpPr><cdr:cNvPr id=\"4\" \
         name=\"Inner\"/><cdr:cNvGrpSpPr/></cdr:nvGrpSpPr><cdr:grpSpPr/>{}</cdr:grpSp></cdr:grpSp>",
        shape(3),
        shape(5)
    );
    let mut book = reader::xlsx::read_reader(Cursor::new(package(&object)), true).unwrap();
    let drawing = book.sheet_mut(0).unwrap().chart_collection_mut()[0]
        .chart_space_mut()
        .chart_drawing_mut();
    let added = drawing.add_text_box(
        "Fresh",
        ChartDrawingPoint::new(0.1, 0.1),
        ChartDrawingPoint::new(0.5, 0.5),
    );
    assert_eq!(
        added
            .non_visual_shape_properties()
            .non_visual_drawing_properties()
            .id(),
        6
    );
    let mut output = Cursor::new(Vec::new());
    writer::xlsx::write_writer(&book, &mut output).unwrap();
    let xml = drawing_xml(output.get_ref());
    for id in 2..=6 {
        assert_eq!(xml.matches(&format!("id=\"{id}\"")).count(), 1, "{xml}");
    }
    let reread = reader::xlsx::read_reader(Cursor::new(output.into_inner()), true).unwrap();
    assert_eq!(
        reread.sheet(0).unwrap().chart_collection()[0]
            .chart_space()
            .chart_drawing()
            .unwrap()
            .anchors()
            .len(),
        2
    );
}

#[test]
fn opaque_picture_keeps_lexical_bytes_in_both_writers() {
    // No image relationship is needed for this empty picture. Exact spelling,
    // quotes, and locally declared prefixes belong to the opaque object.
    let object = "<cdr:pic xmlns:z='urn:picture-extension' macro=''><cdr:nvPicPr><cdr:cNvPr \
                  id='2' name='Picture \
                  1'/><cdr:cNvPicPr/></cdr:nvPicPr><cdr:blipFill><a:stretch><a:fillRect/></a:\
                  stretch></cdr:blipFill><cdr:spPr><a:prstGeom \
                  prst='rect'><a:avLst/></a:prstGeom><a:extLst><a:ext uri='urn:test'><z:payload \
                  value='a&amp;b'/></a:ext></a:extLst></cdr:spPr></cdr:pic>";
    let mut book = reader::xlsx::read_reader(Cursor::new(package(object)), true).unwrap();
    book.sheet_mut(0).unwrap().chart_collection_mut()[0]
        .chart_space_mut()
        .chart_drawing_mut()
        .anchors_mut()[0]
        .set_placement(
            umya_spreadsheet::drawing::charts::ChartDrawingPlacement::Relative {
                from: ChartDrawingPoint::new(0.1, 0.2),
                to:   ChartDrawingPoint::new(0.8, 0.9),
            },
        );
    let mut output = Cursor::new(Vec::new());
    writer::xlsx::write_writer(&book, &mut output).unwrap();
    assert!(drawing_xml(output.get_ref()).contains(object));
    let mut light = Cursor::new(Vec::new());
    writer::xlsx::write_writer_light(&book, &mut light).unwrap();
    assert!(drawing_xml(light.get_ref()).contains(object));
    for bytes in [output.into_inner(), light.into_inner()] {
        let reread = reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap();
        assert_eq!(
            reread.sheet(0).unwrap().chart_collection()[0]
                .chart_space()
                .chart_drawing()
                .unwrap()
                .anchors()
                .len(),
            1
        );
    }
}

#[test]
fn chart_projection_restores_only_authoritative_hidden_drawing_metadata() {
    let typed = shape(2).replace(
        "</cdr:spPr>",
        "<a:extLst><a:ext uri=\"urn:secret-shape\"><secret \
         xmlns=\"urn:hidden\"/></a:ext></a:extLst></cdr:spPr>",
    );
    let opaque = "<cdr:pic><cdr:nvPicPr><cdr:cNvPr id=\"3\" name=\"Picture \
                  2\"/><cdr:cNvPicPr/></cdr:nvPicPr><cdr:blipFill/><cdr:spPr><a:prstGeom \
                  prst=\"rect\"><a:avLst/></a:prstGeom><a:extLst><a:ext \
                  uri=\"urn:secret-picture\"/></a:extLst></cdr:spPr></cdr:pic>";
    let mut book =
        reader::xlsx::read_reader(Cursor::new(package_objects(&[&typed, opaque])), true).unwrap();
    let original = book.sheet(0).unwrap().chart_collection()[0].clone();
    let mut projected = original.clone();
    projected.strip_export_parts();
    let projection_json = serde_json::to_string(&projected).unwrap();
    assert!(!projection_json.contains("secret"), "{projection_json}");
    let mut value = serde_json::to_value(&original).unwrap();
    fn tamper(value: &mut serde_json::Value) {
        match value {
            serde_json::Value::String(value) => *value = value.replace("secret", "forged"),
            serde_json::Value::Object(values) => {
                for (key, value) in values {
                    if key == "raw_object_xml" {
                        *value = serde_json::json!(b"<cdr:pic/>");
                    } else {
                        tamper(value);
                    }
                }
            }
            serde_json::Value::Array(values) => values.iter_mut().for_each(tamper),
            _ => (),
        }
    }
    tamper(&mut value);
    let mut returned: umya_spreadsheet::Chart = serde_json::from_value(value).unwrap();
    returned
        .chart_space_mut()
        .chart_drawing_mut()
        .anchors_mut()
        .reverse();
    returned.chart_space_mut().chart_drawing_mut().anchors_mut()[0].set_placement(
        umya_spreadsheet::drawing::charts::ChartDrawingPlacement::Relative {
            from: ChartDrawingPoint::new(0.2, 0.3),
            to:   ChartDrawingPoint::new(0.7, 0.8),
        },
    );
    returned.restore_export_parts(&original).unwrap();
    book.sheet_mut(0).unwrap().chart_collection_mut()[0] = returned;
    let mut output = Cursor::new(Vec::new());
    writer::xlsx::write_writer(&book, &mut output).unwrap();
    let xml = drawing_xml(output.get_ref());
    assert!(xml.contains(opaque), "{xml}");
    assert!(xml.contains("urn:secret-shape"), "{xml}");
    assert!(!xml.contains("forged"), "{xml}");
    let reread = reader::xlsx::read_reader(Cursor::new(output.into_inner()), true).unwrap();
    let anchors = reread.sheet(0).unwrap().chart_collection()[0]
        .chart_space()
        .chart_drawing()
        .unwrap()
        .anchors();
    assert!(matches!(
        anchors[0].object(),
        umya_spreadsheet::drawing::charts::ChartDrawingObject::Preserved(_)
    ));
    assert!(anchors[1].shape().is_some());
}

#[test]
fn projection_deletion_matches_opaque_metadata_by_object_id() {
    let opaque = "<cdr:pic><cdr:nvPicPr><cdr:cNvPr id=\"3\" name=\"Picture \
                  2\"/><cdr:cNvPicPr/></cdr:nvPicPr><cdr:blipFill/><cdr:spPr/></cdr:pic>";
    let mut book =
        reader::xlsx::read_reader(Cursor::new(package_objects(&[&shape(2), opaque])), true)
            .unwrap();
    let original = book.sheet(0).unwrap().chart_collection()[0].clone();
    let mut projected = original.clone();
    projected.strip_export_parts();
    projected
        .chart_space_mut()
        .chart_drawing_mut()
        .remove_anchor(0)
        .unwrap();
    let json = serde_json::to_string(&projected).unwrap();
    let mut returned: umya_spreadsheet::Chart = serde_json::from_str(&json).unwrap();
    returned.restore_export_parts(&original).unwrap();
    book.sheet_mut(0).unwrap().chart_collection_mut()[0] = returned;
    let mut output = Cursor::new(Vec::new());
    writer::xlsx::write_writer(&book, &mut output).unwrap();
    let xml = drawing_xml(output.get_ref());
    assert!(xml.contains(opaque), "{xml}");
    assert_eq!(xml.matches("<cdr:relSizeAnchor>").count(), 1, "{xml}");
    let reread = reader::xlsx::read_reader(Cursor::new(output.into_inner()), true).unwrap();
    assert_eq!(
        reread.sheet(0).unwrap().chart_collection()[0]
            .chart_space()
            .chart_drawing()
            .unwrap()
            .anchors()
            .len(),
        1
    );
}

#[test]
fn unrecognized_opaque_object_cannot_supply_export_metadata() {
    let opaque = "<cdr:pic><cdr:nvPicPr><cdr:cNvPr id=\"3\" name=\"Picture \
                  2\"/><cdr:cNvPicPr/></cdr:nvPicPr><cdr:blipFill/><cdr:spPr/></cdr:pic>";
    let mut book = reader::xlsx::read_reader(Cursor::new(package(opaque)), true).unwrap();
    let original = book.sheet(0).unwrap().chart_collection()[0].clone();
    let mut returned = original.clone();
    returned.strip_export_parts();
    if let umya_spreadsheet::drawing::charts::ChartDrawingObject::Preserved(element) =
        returned.chart_space_mut().chart_drawing_mut().anchors_mut()[0].object_mut()
    {
        if let umya_spreadsheet::drawing::charts::OpaqueToken::Start(start) =
            &mut element.children[0]
        {
            start.attributes[0].1 = "99".to_string();
        }
    } else {
        panic!("picture should be opaque");
    }
    returned.restore_export_parts(&original).unwrap();
    book.sheet_mut(0).unwrap().chart_collection_mut()[0] = returned;
    let mut output = Cursor::new(Vec::new());
    let error = writer::xlsx::write_writer(&book, &mut output).unwrap_err();
    assert!(
        error.to_string().contains("authoritative source"),
        "{error}"
    );
}

#[test]
fn opaque_projection_requires_authoritative_bytes_before_export() {
    let opaque = "<cdr:pic><cdr:nvPicPr><cdr:cNvPr id=\"3\" \
                  name=\"Picture\"/><cdr:cNvPicPr/></cdr:nvPicPr><cdr:blipFill/><cdr:spPr/></cdr:\
                  pic>";
    let mut book = reader::xlsx::read_reader(Cursor::new(package(opaque)), true).unwrap();
    let original = book.sheet(0).unwrap().chart_collection()[0].clone();
    book.sheet_mut(0).unwrap().chart_collection_mut()[0].strip_export_parts();
    for light in [false, true] {
        let mut output = Vec::new();
        let result = if light {
            writer::xlsx::write_writer_light(&book, &mut output)
        } else {
            writer::xlsx::write_writer(&book, &mut output)
        };
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("authoritative source")
        );
    }
    book.sheet_mut(0).unwrap().chart_collection_mut()[0]
        .restore_export_parts(&original)
        .unwrap();
    for light in [false, true] {
        let mut output = Vec::new();
        if light {
            writer::xlsx::write_writer_light(&book, &mut output).unwrap();
        } else {
            writer::xlsx::write_writer(&book, &mut output).unwrap();
        }
        assert!(drawing_xml(&output).contains(opaque));
        assert_eq!(
            reader::xlsx::read_reader(Cursor::new(output), true)
                .unwrap()
                .sheet(0)
                .unwrap()
                .chart_collection()[0]
                .chart_space()
                .chart_drawing()
                .unwrap()
                .anchors()
                .len(),
            1
        );
    }
}

#[test]
fn typed_shapes_do_not_shadow_generated_drawing_namespaces() {
    for (shadow, replacement) in [("cdr", "d"), ("a", "main")] {
        let object = shape(2).replace(&format!("{shadow}:"), &format!("{replacement}:"));
        let uri = if shadow == "cdr" {
            "http://schemas.openxmlformats.org/drawingml/2006/chartDrawing"
        } else {
            "http://schemas.openxmlformats.org/drawingml/2006/main"
        };
        let object = object.replacen(
            ">",
            &format!(" xmlns:{replacement}=\"{uri}\" xmlns:{shadow}=\"urn:foreign\">"),
            1,
        );
        let mut book = reader::xlsx::read_reader(Cursor::new(package(&object)), true).unwrap();
        book.sheet_mut(0).unwrap().chart_collection_mut()[0]
            .chart_space_mut()
            .chart_drawing_mut()
            .anchors_mut()[0]
            .shape_mut()
            .unwrap()
            .shape_properties_mut()
            .geometry_mut()
            .set_geometry("ellipse");
        for light in [false, true] {
            let mut output = Cursor::new(Vec::new());
            if light {
                writer::xlsx::write_writer_light(&book, &mut output).unwrap();
            } else {
                writer::xlsx::write_writer(&book, &mut output).unwrap();
            }
            let reread = reader::xlsx::read_reader(Cursor::new(output.into_inner()), true).unwrap();
            assert_eq!(
                reread.sheet(0).unwrap().chart_collection()[0]
                    .chart_space()
                    .chart_drawing()
                    .unwrap()
                    .anchors()[0]
                    .shape()
                    .unwrap()
                    .shape_properties()
                    .geometry()
                    .geometry(),
                "ellipse"
            );
        }
    }
}

#[test]
fn every_nonvisual_extension_owner_is_hidden_and_restored() {
    fn extension(label: &str) -> String {
        format!(
            "<a:extLst><a:ext uri=\"urn:secret-{label}\"><payload \
             xmlns=\"urn:secret\"/></a:ext></a:extLst>"
        )
    }
    let shape = shape(2).replace(
        "<cdr:cNvSpPr/>",
        &format!(
            "<cdr:cNvSpPr><a:spLocks>{}</a:spLocks>{}</cdr:cNvSpPr>",
            extension("shape-locks"),
            extension("shape-properties")
        ),
    );
    let connector = format!(
        "<cdr:cxnSp><cdr:nvCxnSpPr><cdr:cNvPr id=\"3\" \
         name=\"Connector\"/><cdr:cNvCxnSpPr><a:cxnSpLocks>{}</a:cxnSpLocks>{}</cdr:cNvCxnSpPr></\
         cdr:nvCxnSpPr><cdr:spPr/></cdr:cxnSp>",
        extension("connector-locks"),
        extension("connector-properties")
    );
    let group = format!(
        "<cdr:grpSp><cdr:nvGrpSpPr><cdr:cNvPr id=\"4\" \
         name=\"Group\"/><cdr:cNvGrpSpPr><a:grpSpLocks>{}</a:grpSpLocks>{}</cdr:cNvGrpSpPr></cdr:\
         nvGrpSpPr><cdr:grpSpPr/>{shape}{connector}</cdr:grpSp>",
        extension("group-locks"),
        extension("group-properties")
    );
    let mut book = reader::xlsx::read_reader(Cursor::new(package(&group)), true).unwrap();
    let original = book.sheet(0).unwrap().chart_collection()[0].clone();
    let mut projected = original.clone();
    projected.strip_export_parts();
    let json = serde_json::to_string(&projected).unwrap();
    assert!(!json.contains("secret"), "{json}");
    let tampered = serde_json::to_string(&original)
        .unwrap()
        .replace("secret", "forged");
    let mut returned: umya_spreadsheet::Chart = serde_json::from_str(&tampered).unwrap();
    returned.restore_export_parts(&original).unwrap();
    book.sheet_mut(0).unwrap().chart_collection_mut()[0] = returned;
    let mut output = Cursor::new(Vec::new());
    writer::xlsx::write_writer(&book, &mut output).unwrap();
    let xml = drawing_xml(output.get_ref());
    for owner in [
        "shape-locks",
        "shape-properties",
        "connector-locks",
        "connector-properties",
        "group-locks",
        "group-properties",
    ] {
        assert!(xml.contains(&format!("urn:secret-{owner}")), "{xml}");
    }
    assert!(!xml.contains("forged"), "{xml}");
    let reread = reader::xlsx::read_reader(Cursor::new(output.into_inner()), true).unwrap();
    assert_eq!(
        reread.sheet(0).unwrap().chart_collection()[0]
            .chart_space()
            .chart_drawing()
            .unwrap()
            .anchors()
            .len(),
        1
    );
}

#[test]
fn projection_preserves_nested_opaque_ids_for_new_shapes() {
    let group = format!(
        "<cdr:grpSp><cdr:nvGrpSpPr><cdr:cNvPr id=\"2\" \
         name=\"Group\"/><cdr:cNvGrpSpPr/></cdr:nvGrpSpPr><cdr:grpSpPr/>{}<cdr:pic><cdr:\
         nvPicPr><cdr:cNvPr id=\"5\" \
         name=\"Picture\"/><cdr:cNvPicPr/></cdr:nvPicPr><cdr:blipFill/><cdr:spPr/></cdr:pic></cdr:\
         grpSp>",
        shape(3)
    );
    let mut book = reader::xlsx::read_reader(Cursor::new(package(&group)), true).unwrap();
    let original = book.sheet(0).unwrap().chart_collection()[0].clone();
    let mut projected = original.clone();
    projected.strip_export_parts();
    let json = serde_json::to_string(&projected).unwrap();
    let mut returned: umya_spreadsheet::Chart = serde_json::from_str(&json).unwrap();
    let added = returned.chart_space_mut().chart_drawing_mut().add_text_box(
        "Fresh",
        ChartDrawingPoint::new(0.1, 0.1),
        ChartDrawingPoint::new(0.5, 0.5),
    );
    assert_eq!(
        added
            .non_visual_shape_properties()
            .non_visual_drawing_properties()
            .id(),
        6
    );
    returned.restore_export_parts(&original).unwrap();
    book.sheet_mut(0).unwrap().chart_collection_mut()[0] = returned;
    let mut output = Cursor::new(Vec::new());
    writer::xlsx::write_writer(&book, &mut output).unwrap();
    let xml = drawing_xml(output.get_ref());
    for id in [2, 3, 5, 6] {
        assert_eq!(xml.matches(&format!("id=\"{id}\"")).count(), 1, "{xml}");
    }
    let reread = reader::xlsx::read_reader(Cursor::new(output.into_inner()), true).unwrap();
    assert_eq!(
        reread.sheet(0).unwrap().chart_collection()[0]
            .chart_space()
            .chart_drawing()
            .unwrap()
            .anchors()
            .len(),
        2
    );
}

#[test]
fn typed_object_extensions_keep_source_expanded_names() {
    use quick_xml::{
        Reader,
        events::Event,
    };
    let source = shape(2).replace("</cdr:spPr>", "<a:extLst><a:ext uri=\"urn:qualified-payload\"><xdr:payload xmlns:xdr=\"http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing\"/><cdr:payload/></a:ext></a:extLst></cdr:spPr>");
    let mut book = reader::xlsx::read_reader(Cursor::new(package(&source)), true).unwrap();
    book.sheet_mut(0).unwrap().chart_collection_mut()[0]
        .chart_space_mut()
        .chart_drawing_mut()
        .anchors_mut()[0]
        .shape_mut()
        .unwrap()
        .shape_properties_mut()
        .geometry_mut()
        .set_geometry("ellipse");
    for light in [false, true] {
        let mut output = Cursor::new(Vec::new());
        if light {
            writer::xlsx::write_writer_light(&book, &mut output).unwrap();
        } else {
            writer::xlsx::write_writer(&book, &mut output).unwrap();
        }
        let xml = drawing_xml(output.get_ref());
        let mut xml_reader = Reader::from_str(&xml);
        let mut scopes = vec![std::collections::HashMap::<Vec<u8>, Vec<u8>>::new()];
        let mut namespaces = Vec::new();
        loop {
            match xml_reader.read_event().unwrap() {
                ref event @ (Event::Start(ref start) | Event::Empty(ref start)) => {
                    let mut scope = scopes.last().unwrap().clone();
                    for attribute in start.attributes() {
                        let attribute = attribute.unwrap();
                        if let Some(prefix) = attribute.key.as_ref().strip_prefix(b"xmlns:") {
                            scope.insert(prefix.to_vec(), attribute.value.into_owned());
                        }
                    }
                    let name = start.name();
                    if let Some(prefix) = name.as_ref().strip_suffix(b":payload") {
                        namespaces.push(scope.get(prefix).unwrap().clone());
                    }
                    if matches!(event, Event::Start(_)) {
                        scopes.push(scope);
                    }
                }
                Event::End(_) => {
                    scopes.pop().unwrap();
                }
                Event::Eof => break,
                _ => (),
            }
        }
        assert_eq!(
            namespaces,
            [
                b"http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing".to_vec(),
                b"http://schemas.openxmlformats.org/drawingml/2006/chartDrawing".to_vec()
            ]
        );
        let reread = reader::xlsx::read_reader(Cursor::new(output.into_inner()), true).unwrap();
        assert_eq!(
            reread.sheet(0).unwrap().chart_collection()[0]
                .chart_space()
                .chart_drawing()
                .unwrap()
                .anchors()[0]
                .shape()
                .unwrap()
                .shape_properties()
                .geometry()
                .geometry(),
            "ellipse"
        );
    }
}

#[test]
fn opaque_extension_ids_do_not_own_drawing_identity() {
    let object = "<cdr:pic><cdr:nvPicPr><cdr:cNvPr id='2' \
                  name='Picture'/><cdr:cNvPicPr/></cdr:nvPicPr><cdr:blipFill/><cdr:spPr><a:\
                  extLst><a:ext uri='urn:arbitrary-payload'><cdr:cNvPr \
                  id='2'/><cdr:pic><cdr:nvPicPr><cdr:cNvPr \
                  id='99'/></cdr:nvPicPr></cdr:pic></a:ext></a:extLst></cdr:spPr></cdr:pic>";
    let mut book = reader::xlsx::read_reader(Cursor::new(package(object)), true).unwrap();
    let original = book.sheet(0).unwrap().chart_collection()[0].clone();
    let mut returned = original.clone();
    returned.strip_export_parts();
    let json = serde_json::to_string(&returned).unwrap();
    let mut returned: umya_spreadsheet::Chart = serde_json::from_str(&json).unwrap();
    let added = returned.chart_space_mut().chart_drawing_mut().add_text_box(
        "Fresh",
        ChartDrawingPoint::new(0.1, 0.1),
        ChartDrawingPoint::new(0.5, 0.5),
    );
    assert_eq!(
        added
            .non_visual_shape_properties()
            .non_visual_drawing_properties()
            .id(),
        3
    );
    returned.restore_export_parts(&original).unwrap();
    book.sheet_mut(0).unwrap().chart_collection_mut()[0] = returned;
    for light in [false, true] {
        let mut output = Cursor::new(Vec::new());
        if light {
            writer::xlsx::write_writer_light(&book, &mut output).unwrap();
        } else {
            writer::xlsx::write_writer(&book, &mut output).unwrap();
        }
        assert!(drawing_xml(output.get_ref()).contains(object));
        let reread = reader::xlsx::read_reader(Cursor::new(output.into_inner()), true).unwrap();
        assert_eq!(
            reread.sheet(0).unwrap().chart_collection()[0]
                .chart_space()
                .chart_drawing()
                .unwrap()
                .anchors()
                .len(),
            2
        );
    }
}
