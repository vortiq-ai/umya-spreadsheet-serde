#![cfg(feature = "serde")]
use std::io::{
    Cursor,
    Read,
    Write,
};

use umya_spreadsheet::{
    Workbook,
    reader,
    writer,
};

const FIXTURE: &[u8] = include_bytes!("fixtures/chart_drawing_excel.xlsx");
const GROUP_PROPERTIES: &str =
    "<cdr:grpSpPr bwMode=\"gray\"><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"1000000\" \
     cy=\"500000\"/><a:chOff x=\"0\" y=\"0\"/><a:chExt cx=\"1000000\" \
     cy=\"500000\"/></a:xfrm><a:solidFill><a:srgbClr \
     val=\"123456\"/></a:solidFill><a:effectLst><a:glow rad=\"12700\"><a:srgbClr \
     val=\"ABCDEF\"/></a:glow></a:effectLst><a:scene3d><a:camera \
     prst=\"orthographicFront\"/><a:lightRig rig=\"threePt\" \
     dir=\"t\"/></a:scene3d><a:extLst><a:ext uri=\"urn:group\"><z:payload xmlns:z=\"urn:group\" \
     value=\"kept\"/></a:ext></a:extLst></cdr:grpSpPr>";

fn package(object: &str) -> Vec<u8> {
    package_in(object, false)
}

fn group(properties: &str) -> String {
    let mut object = format!(
        "<cdr:grpSp><cdr:nvGrpSpPr><cdr:cNvPr id=\"2\" \
         name=\"Group\"/><cdr:cNvGrpSpPr/></cdr:nvGrpSpPr>{properties}"
    );
    for (id, x) in [(3, 0), (4, 500000)] {
        object.push_str(&format!(
            "<cdr:sp><cdr:nvSpPr><cdr:cNvPr id=\"{id}\" name=\"Child \
             {id}\"/><cdr:cNvSpPr/></cdr:nvSpPr><cdr:spPr><a:xfrm><a:off x=\"{x}\" \
             y=\"0\"/><a:ext cx=\"500000\" cy=\"500000\"/></a:xfrm><a:prstGeom \
             prst=\"rect\"><a:avLst/></a:prstGeom></cdr:spPr></cdr:sp>"
        ));
    }
    object.push_str("</cdr:grpSp>");
    object
}
fn package_in(object: &str, worksheet: bool) -> Vec<u8> {
    let xml = format!("<c:userShapes xmlns:c=\"http://schemas.openxmlformats.org/drawingml/2006/chart\" xmlns:cdr=\"http://schemas.openxmlformats.org/drawingml/2006/chartDrawing\" xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\"><cdr:relSizeAnchor><cdr:from><cdr:x>0</cdr:x><cdr:y>0</cdr:y></cdr:from><cdr:to><cdr:x>1</cdr:x><cdr:y>1</cdr:y></cdr:to>{object}</cdr:relSizeAnchor></c:userShapes>");
    let mut input = zip::ZipArchive::new(Cursor::new(FIXTURE)).unwrap();
    let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for index in 0..input.len() {
        let mut file = input.by_index(index).unwrap();
        let name = file.name().unwrap().to_string();
        let mut data = Vec::new();
        file.read_to_end(&mut data).unwrap();
        if name.starts_with("xl/drawings/") {
            let original = String::from_utf8_lossy(&data);
            if worksheet && original.contains("<xdr:graphicFrame") {
                let start = original.find("<xdr:graphicFrame").unwrap();
                let end =
                    original.find("</xdr:graphicFrame>").unwrap() + "</xdr:graphicFrame>".len();
                let mut rewritten = original.into_owned();
                rewritten.replace_range(start..end, &object.replace("cdr:", "xdr:"));
                data = rewritten.into_bytes();
            } else if !worksheet && original.contains("userShapes") {
                data = xml.as_bytes().to_vec();
            }
        }
        output
            .start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        output.write_all(&data).unwrap();
    }
    output.finish().unwrap().into_inner()
}
fn open(bytes: &[u8], eager: bool) -> Workbook {
    let mut book = reader::xlsx::read_reader(Cursor::new(bytes.to_vec()), eager).unwrap();
    book.try_read_sheet(0).unwrap();
    book
}
fn save(book: &Workbook, light: bool) -> Vec<u8> {
    let mut output = Cursor::new(Vec::new());
    if light {
        writer::xlsx::write_writer_light(book, &mut output).unwrap();
    } else {
        writer::xlsx::write_writer(book, &mut output).unwrap();
    }
    output.into_inner()
}
fn drawing_xml(bytes: &[u8]) -> String {
    drawing_xml_in(bytes, false)
}

fn assert_preserved_object(object: &str, name: Option<&str>) {
    use umya_spreadsheet::drawing::charts::ChartDrawingObject;

    for eager in [false, true] {
        let book = open(&package(object), eager);
        let drawing_object = |book: &Workbook| {
            serde_json::to_value(
                book.sheet(0).unwrap().chart_collection()[0]
                    .chart_space()
                    .chart_drawing()
                    .unwrap()
                    .anchors()[0]
                    .object(),
            )
            .unwrap()
        };
        let drawing = book.sheet(0).unwrap().chart_collection()[0]
            .chart_space()
            .chart_drawing()
            .unwrap();
        assert!(matches!(
            drawing.anchors()[0].object(),
            ChartDrawingObject::Preserved(_)
        ));
        for light in [false, true] {
            let output = save(&book, light);
            assert!(drawing_xml(&output).contains(object), "{object}");
            let reread = open(&output, eager);
            assert_eq!(drawing_object(&reread), drawing_object(&book), "{object}");
            assert!(
                drawing_xml(&save(&reread, light)).contains(object),
                "{object}"
            );
            if let Some(name) = name {
                save_package(&output, &format!("preserved-{name}-{eager}-{light}"));
            }
        }
    }
}
fn drawing_xml_in(bytes: &[u8], worksheet: bool) -> String {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    for index in 0..archive.len() {
        let mut file = archive.by_index(index).unwrap();
        if file.name().unwrap().starts_with("xl/drawings/") {
            let mut data = String::new();
            file.read_to_string(&mut data).unwrap();
            if data.contains(if worksheet { "wsDr" } else { "userShapes" }) {
                return data;
            }
        }
    }
    panic!("chart drawing missing")
}
// Compare every tag, attribute and text after namespace and empty-tag
// normalization.
fn without_namespace_declarations(xml: &str) -> String {
    let mut reader = quick_xml::Reader::from_str(xml);
    let mut writer = quick_xml::Writer::new(Cursor::new(Vec::new()));
    loop {
        let event = reader.read_event().unwrap();
        match event {
            quick_xml::events::Event::Start(ref e) | quick_xml::events::Event::Empty(ref e) => {
                let mut start = quick_xml::events::BytesStart::new(
                    String::from_utf8(e.name().as_ref().to_vec()).unwrap(),
                );
                for attr in e.attributes() {
                    let attr = attr.unwrap();
                    if !attr.key.as_ref().starts_with(b"xmlns") {
                        start.push_attribute(attr);
                    }
                }
                let end = start.to_end().into_owned();
                writer
                    .write_event(quick_xml::events::Event::Start(start))
                    .unwrap();
                if matches!(event, quick_xml::events::Event::Empty(_)) {
                    writer
                        .write_event(quick_xml::events::Event::End(end))
                        .unwrap();
                }
            }
            quick_xml::events::Event::Eof => break,
            event => writer.write_event(event).unwrap(),
        }
    }
    String::from_utf8(writer.into_inner().into_inner()).unwrap()
}
#[test]
fn group_properties_preserve_fill_effect_scene_extensions_and_mode() {
    let object = group(GROUP_PROPERTIES);
    let bytes = package(&object);
    for eager in [true, false] {
        let book = open(&bytes, eager);
        for light in [false, true] {
            let output = save(&book, light);
            let xml = without_namespace_declarations(&drawing_xml(&output));
            assert!(
                xml.contains(&without_namespace_declarations(GROUP_PROPERTIES)),
                "{xml}"
            );
            let reread = open(&output, eager);
            assert!(
                without_namespace_declarations(&drawing_xml(&save(&reread, light)))
                    .contains(&without_namespace_declarations(GROUP_PROPERTIES))
            );
            save_package(&output, &format!("group-{eager}-{light}"));
        }
    }
}
fn save_package(output: &[u8], name: &str) {
    if let Some(target) = std::env::var_os("CARGO_TARGET_DIR") {
        let dir = std::path::PathBuf::from(target).join("chart-drawing-group-properties");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(format!("{name}.xlsx")), output).unwrap();
    }
}
#[test]
fn connector_preserves_published_attribute() {
    let object = "<cdr:cxnSp fPublished=\"true\"><cdr:nvCxnSpPr><cdr:cNvPr id=\"2\" \
                  name=\"Connector\"/><cdr:cNvCxnSpPr/></cdr:nvCxnSpPr><cdr:spPr><a:prstGeom \
                  prst=\"line\"><a:avLst/></a:prstGeom></cdr:spPr></cdr:cxnSp>";
    for eager in [true, false] {
        let book = open(&package(object), eager);
        for light in [false, true] {
            let output = save(&book, light);
            let xml = drawing_xml(&output);
            assert!(xml.contains("<cdr:cxnSp fPublished=\"1\">"), "{xml}");
            let reread = open(&output, eager);
            assert!(drawing_xml(&save(&reread, light)).contains("<cdr:cxnSp fPublished=\"1\">"));
            save_package(&output, &format!("connector-{eager}-{light}"));
        }
    }
}

#[test]
fn worksheet_groups_preserve_the_same_properties() {
    let object = group(GROUP_PROPERTIES);
    let bytes = package_in(&object, true);
    for eager in [true, false] {
        let book = open(&bytes, eager);
        for light in [false, true] {
            let output = save(&book, light);
            let xml = without_namespace_declarations(&drawing_xml_in(&output, true));
            assert!(
                xml.contains(&without_namespace_declarations(
                    &GROUP_PROPERTIES.replace("cdr:", "xdr:")
                )),
                "{xml}"
            );
            open(&output, eager);
        }
    }
}

#[test]
fn chart_group_preserves_shape_only_sp3d() {
    let object = "<cdr:grpSp><cdr:nvGrpSpPr><cdr:cNvPr id=\"2\" \
                  name=\"Group\"/><cdr:cNvGrpSpPr/></cdr:nvGrpSpPr><cdr:grpSpPr><a:sp3d/></cdr:\
                  grpSpPr></cdr:grpSp>";
    assert_preserved_object(object, None);
}

#[test]
fn group_properties_preserve_each_fill_and_effect_choice() {
    for fill in [
        "<a:noFill/>",
        "<a:gradFill><a:gsLst><a:gs pos=\"0\"><a:srgbClr val=\"112233\"/></a:gs><a:gs \
         pos=\"100000\"><a:srgbClr val=\"445566\"/></a:gs></a:gsLst><a:lin ang=\"0\" \
         scaled=\"1\"/></a:gradFill>",
        "<a:blipFill><a:stretch><a:fillRect/></a:stretch></a:blipFill>",
        "<a:pattFill prst=\"pct5\"><a:fgClr><a:srgbClr \
         val=\"112233\"/></a:fgClr><a:bgClr><a:srgbClr val=\"FFFFFF\"/></a:bgClr></a:pattFill>",
        "<a:grpFill/>",
    ] {
        let properties = format!("<cdr:grpSpPr>{fill}<a:effectDag/></cdr:grpSpPr>");
        let object = format!(
            "<cdr:grpSp><cdr:nvGrpSpPr><cdr:cNvPr id=\"2\" \
             name=\"Group\"/><cdr:cNvGrpSpPr/></cdr:nvGrpSpPr>{properties}</cdr:grpSp>"
        );
        for eager in [true, false] {
            let book = open(&package(&object), eager);
            for light in [false, true] {
                let output = save(&book, light);
                assert!(
                    without_namespace_declarations(&drawing_xml(&output))
                        .contains(&without_namespace_declarations(&properties))
                );
                open(&output, eager);
            }
        }
    }
}

#[test]
fn added_fields_default_when_older_json_omits_them() {
    use umya_spreadsheet::drawing::spreadsheet::{
        ConnectionShape,
        GroupShapeProperties,
    };
    let group: GroupShapeProperties = serde_json::from_str("{\"transform2d\":null}").unwrap();
    assert!(group.black_white_mode().is_none());
    assert!(group.preserved_properties().is_empty());
    let mut json = serde_json::to_value(ConnectionShape::default()).unwrap();
    json.as_object_mut().unwrap().remove("published");
    let connector: ConnectionShape = serde_json::from_value(json).unwrap();
    assert!(!connector.published());
}

#[test]
fn connector_publication_can_be_edited_and_removed() {
    use umya_spreadsheet::drawing::charts::ChartDrawingObject;
    let object = "<cdr:cxnSp fPublished=\"true\"><cdr:nvCxnSpPr><cdr:cNvPr id=\"2\" \
                  name=\"Connector\"/><cdr:cNvCxnSpPr/></cdr:nvCxnSpPr><cdr:spPr><a:prstGeom \
                  prst=\"line\"><a:avLst/></a:prstGeom></cdr:spPr></cdr:cxnSp>";
    for eager in [true, false] {
        let mut book = open(&package(object), eager);
        let connector = match book.sheet_mut(0).unwrap().chart_collection_mut()[0]
            .chart_space_mut()
            .chart_drawing_mut()
            .anchors_mut()[0]
            .object_mut()
        {
            ChartDrawingObject::Connector(value) => value,
            _ => panic!("typed connector"),
        };
        assert!(connector.published());
        connector.set_published(false);
        for light in [false, true] {
            let output = save(&book, light);
            assert!(drawing_xml(&output).contains("fPublished=\"0\""));
            save_package(&output, &format!("connector-edited-{eager}-{light}"));
        }
        let connector = match book.sheet_mut(0).unwrap().chart_collection_mut()[0]
            .chart_space_mut()
            .chart_drawing_mut()
            .anchors_mut()[0]
            .object_mut()
        {
            ChartDrawingObject::Connector(value) => value,
            _ => unreachable!(),
        };
        connector.remove_published();
        for light in [false, true] {
            let output = save(&book, light);
            assert!(!drawing_xml(&output).contains("fPublished="));
            save_package(&output, &format!("connector-removed-{eager}-{light}"));
        }
    }
}

#[test]
fn group_mode_and_properties_can_be_edited_and_removed() {
    use umya_spreadsheet::drawing::{
        BlackWhiteModeValues,
        charts::ChartDrawingObject,
    };
    let object = group(GROUP_PROPERTIES);
    for eager in [true, false] {
        let mut book = open(&package(&object), eager);
        let drawing = book.sheet_mut(0).unwrap().chart_collection_mut()[0]
            .chart_space_mut()
            .chart_drawing_mut();
        let ChartDrawingObject::Group(group) = drawing.anchors_mut()[0].object_mut() else {
            panic!("typed group");
        };
        let properties = group.group_shape_properties_mut();
        assert!(matches!(
            properties.black_white_mode(),
            Some(BlackWhiteModeValues::Gray)
        ));
        assert_eq!(properties.preserved_properties().len(), 4);
        let persisted = serde_json::to_string(properties).unwrap();
        *properties = serde_json::from_str(&persisted).unwrap();
        properties.set_black_white_mode(BlackWhiteModeValues::Black);
        for light in [false, true] {
            let output = save(&book, light);
            assert!(
                without_namespace_declarations(&drawing_xml(&output)).contains(
                    &without_namespace_declarations(&GROUP_PROPERTIES.replace("gray", "black"))
                )
            );
            save_package(&output, &format!("group-edited-{eager}-{light}"));
        }
        let drawing = book.sheet_mut(0).unwrap().chart_collection_mut()[0]
            .chart_space_mut()
            .chart_drawing_mut();
        let ChartDrawingObject::Group(group) = drawing.anchors_mut()[0].object_mut() else {
            unreachable!();
        };
        let properties = group.group_shape_properties_mut();
        properties.remove_black_white_mode();
        properties.preserved_properties_mut().clear();
        for light in [false, true] {
            let output = save(&book, light);
            let xml = drawing_xml(&output);
            assert!(xml.contains("<cdr:grpSpPr><a:xfrm>"));
            assert!(!xml.contains("bwMode="));
            assert!(!xml.contains("urn:group"));
            open(&output, eager);
            save_package(&output, &format!("group-removed-{eager}-{light}"));
        }
    }
}

#[test]
fn projected_groups_restore_only_authoritative_hidden_properties() {
    use umya_spreadsheet::drawing::{
        BlackWhiteModeValues,
        charts::ChartDrawingObject,
    };
    for eager in [true, false] {
        let mut book = open(&package(&group(GROUP_PROPERTIES)), eager);
        let original = book.sheet(0).unwrap().chart_collection()[0].clone();
        let mut projected = original.clone();
        projected.strip_export_parts();
        let json = serde_json::to_string(&projected).unwrap();
        assert!(!json.contains("urn:group"));
        assert!(!json.contains("123456"));
        assert!(json.contains("Gray"));
        let mut returned: umya_spreadsheet::Chart = serde_json::from_str(&json).unwrap();
        let ChartDrawingObject::Group(authoritative) =
            original.chart_space().chart_drawing().unwrap().anchors()[0].object()
        else {
            panic!("typed original group");
        };
        let ChartDrawingObject::Group(group) =
            returned.chart_space_mut().chart_drawing_mut().anchors_mut()[0].object_mut()
        else {
            panic!("typed returned group");
        };
        let properties = group.group_shape_properties_mut();
        properties.set_black_white_mode(BlackWhiteModeValues::Black);
        let mut forged = authoritative
            .group_shape_properties()
            .preserved_properties()[3]
            .clone();
        let umya_spreadsheet::drawing::charts::OpaqueToken::Start(extension) =
            &mut forged.children[0]
        else {
            panic!("extension child");
        };
        assert_eq!(extension.name.local_name, "ext");
        extension.attributes[0].1 = "urn:forged".to_string();
        properties.preserved_properties_mut().push(forged);
        returned.restore_export_parts(&original).unwrap();
        book.sheet_mut(0).unwrap().chart_collection_mut()[0] = returned;
        for light in [false, true] {
            let output = save(&book, light);
            let xml = without_namespace_declarations(&drawing_xml(&output));
            assert!(!xml.contains("urn:forged"));
            assert!(
                xml.contains(&without_namespace_declarations(
                    &GROUP_PROPERTIES.replace("gray", "black")
                )),
                "{xml}"
            );
            open(&output, eager);
        }
    }
}

#[test]
fn typed_group_properties_reject_unowned_relationships() {
    use umya_spreadsheet::drawing::charts::{
        ChartDrawingObject,
        OpaqueElement,
        OpaqueStart,
        OpaqueToken,
        QName,
    };
    let synthetic = package(&group(GROUP_PROPERTIES));
    let rich = include_bytes!("fixtures/chart_shapes_rich_excel.xlsx");
    let main_ns = "http://schemas.openxmlformats.org/drawingml/2006/main";
    let relationship_ns = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
    for source in [synthetic.as_slice(), rich.as_slice()] {
        for eager in [true, false] {
            for id in ["rId1", "rIdMissing"] {
                let mut book = open(source, eager);
                let drawing = book.sheet_mut(0).unwrap().chart_collection_mut()[0]
                    .chart_space_mut()
                    .chart_drawing_mut();
                let group = drawing
                    .anchors_mut()
                    .iter_mut()
                    .find_map(|anchor| match anchor.object_mut() {
                        ChartDrawingObject::Group(group) => Some(group),
                        _ => None,
                    })
                    .expect("typed chart group");
                let properties = group
                    .group_shape_properties_mut()
                    .preserved_properties_mut();
                properties.clear();
                properties.push(OpaqueElement {
                    name:       QName {
                        namespace_uri:    main_ns.into(),
                        local_name:       "blipFill".into(),
                        preferred_prefix: Some("a".into()),
                    },
                    namespaces: vec![
                        (Some("a".into()), main_ns.into()),
                        (Some("r".into()), relationship_ns.into()),
                    ],
                    attributes: Vec::new(),
                    children:   vec![
                        OpaqueToken::Start(Box::new(OpaqueStart {
                            name:       QName {
                                namespace_uri:    main_ns.into(),
                                local_name:       "blip".into(),
                                preferred_prefix: Some("a".into()),
                            },
                            namespaces: Vec::new(),
                            attributes: vec![(
                                QName {
                                    namespace_uri:    relationship_ns.into(),
                                    local_name:       "embed".into(),
                                    preferred_prefix: Some("r".into()),
                                },
                                id.to_string(),
                            )],
                        })),
                        OpaqueToken::End,
                    ],
                });
                for light in [false, true] {
                    let mut output = Cursor::new(Vec::new());
                    let result = if light {
                        writer::xlsx::write_writer_light(&book, &mut output)
                    } else {
                        writer::xlsx::write_writer(&book, &mut output)
                    };
                    let error = result
                        .expect_err("typed group property cannot borrow a drawing relationship")
                        .to_string();
                    assert!(
                        error.contains("group property") && error.contains("relationship"),
                        "{error}"
                    );
                }
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum ResolvedToken {
    Start((String, String), Vec<((String, String), String)>),
    End,
    Text(String),
}

fn namespace(value: quick_xml::name::ResolveResult<'_>) -> String {
    match value {
        quick_xml::name::ResolveResult::Bound(value) => {
            String::from_utf8(value.as_ref().to_vec()).unwrap()
        }
        quick_xml::name::ResolveResult::Unbound => String::new(),
        quick_xml::name::ResolveResult::Unknown(prefix) => panic!("undeclared prefix {prefix:?}"),
    }
}

// Resolve names before comparing; removing xmlns attributes would hide a
// DrawingML property incorrectly rewritten into the chartDrawing namespace.
fn resolved_subtree(xml: &str, local_name: &str) -> Vec<ResolvedToken> {
    use quick_xml::events::Event;
    let mut reader = quick_xml::NsReader::from_reader(xml.as_bytes());
    let mut tokens = Vec::new();
    let mut depth = 0;
    loop {
        let event = reader.read_event().unwrap();
        let empty = matches!(event, Event::Empty(_));
        match event {
            Event::Start(element) | Event::Empty(element) => {
                let (uri, local) = reader.resolver().resolve_element(element.name());
                let name = (
                    namespace(uri),
                    String::from_utf8(local.as_ref().to_vec()).unwrap(),
                );
                if depth == 0
                    && (name.0 != "http://schemas.openxmlformats.org/drawingml/2006/chartDrawing"
                        || name.1 != local_name)
                {
                    continue;
                }
                let mut attributes = Vec::new();
                for attribute in element.attributes() {
                    let attribute = attribute.unwrap();
                    if attribute.key.as_ref() == b"xmlns"
                        || attribute
                            .key
                            .prefix()
                            .is_some_and(|prefix| prefix.as_ref() == b"xmlns")
                    {
                        continue;
                    }
                    let (uri, local) = reader.resolver().resolve_attribute(attribute.key);
                    attributes.push((
                        (
                            namespace(uri),
                            String::from_utf8(local.as_ref().to_vec()).unwrap(),
                        ),
                        attribute
                            .decoded_and_normalized_value(
                                quick_xml::XmlVersion::Implicit1_0,
                                reader.decoder(),
                            )
                            .unwrap()
                            .into_owned(),
                    ));
                }
                attributes.sort();
                tokens.push(ResolvedToken::Start(name, attributes));
                if empty {
                    tokens.push(ResolvedToken::End);
                    if depth == 0 {
                        return tokens;
                    }
                } else {
                    depth += 1;
                }
            }
            Event::End(_) if depth > 0 => {
                tokens.push(ResolvedToken::End);
                depth -= 1;
                if depth == 0 {
                    return tokens;
                }
            }
            Event::Text(text) if depth > 0 => tokens.push(ResolvedToken::Text(
                text.xml_content(quick_xml::XmlVersion::Implicit1_0)
                    .unwrap()
                    .into_owned(),
            )),
            Event::Eof => panic!("missing chartDrawing {local_name}"),
            _ => (),
        }
    }
}

fn save_roundtrip_package(output: &[u8], name: &str) {
    if let Some(target) = std::env::var_os("CARGO_TARGET_DIR") {
        let dir = std::path::PathBuf::from(target).join("chart-drawing-effects");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(format!("{name}.xlsx")), output).unwrap();
    }
}

#[test]
fn group_properties_follow_resolved_namespaces_with_alternate_prefixes() {
    use umya_spreadsheet::drawing::charts::ChartDrawingObject;
    let main = "http://schemas.openxmlformats.org/drawingml/2006/main";
    let chart = "http://schemas.openxmlformats.org/drawingml/2006/chartDrawing";
    for (label, drawing_prefix, main_prefix) in [
        ("main-xdr", "cdr", "xdr"),
        ("main-custom", "cdr", "paint"),
        ("chart-custom", "shapes", "a"),
    ] {
        let properties = GROUP_PROPERTIES.replace("a:", &format!("{main_prefix}:"));
        let object = group(&properties)
            .replace("cdr:", &format!("{drawing_prefix}:"))
            .replacen(
                &format!("<{drawing_prefix}:grpSp>"),
                &format!(
                    "<{drawing_prefix}:grpSp xmlns:{drawing_prefix}=\"{chart}\" \
                     xmlns:{main_prefix}=\"{main}\">"
                ),
                1,
            );
        let source = package(&object);
        let expected = resolved_subtree(&drawing_xml(&source), "grpSpPr");
        for eager in [true, false] {
            let book = open(&source, eager);
            assert!(matches!(
                book.sheet(0).unwrap().chart_collection()[0]
                    .chart_space()
                    .chart_drawing()
                    .unwrap()
                    .anchors()[0]
                    .object(),
                ChartDrawingObject::Group(_)
            ));
            for light in [false, true] {
                let output = save(&book, light);
                assert_eq!(resolved_subtree(&drawing_xml(&output), "grpSpPr"), expected);
                let reread = open(&output, eager);
                assert_eq!(
                    resolved_subtree(&drawing_xml(&save(&reread, light)), "grpSpPr"),
                    expected
                );
                save_roundtrip_package(&output, &format!("group-{label}-{eager}-{light}"));
            }
        }
    }
}

#[test]
fn shapes_preserve_every_effect_list_child_and_nested_effect_dag() {
    use umya_spreadsheet::drawing::charts::ChartDrawingObject;
    let inner = "<a:innerShdw blurRad=\"25400\" dist=\"12700\" dir=\"5400000\"><a:srgbClr \
                 val=\"123456\"><a:alpha val=\"65000\"/></a:srgbClr></a:innerShdw>";
    let blur = "<a:blur rad=\"12700\" grow=\"0\"/>";
    let overlay = "<a:fillOverlay blend=\"over\"><a:solidFill><a:srgbClr \
                   val=\"445566\"/></a:solidFill></a:fillOverlay>";
    let glow = "<a:glow rad=\"12700\"><a:srgbClr val=\"ABCDEF\"/></a:glow>";
    let outer = "<a:outerShdw blurRad=\"12700\" dist=\"25400\" dir=\"5400000\"><a:srgbClr \
                 val=\"654321\"/></a:outerShdw>";
    let preset = "<a:prstShdw prst=\"shdw1\" dist=\"12700\" dir=\"5400000\"><a:srgbClr \
                  val=\"112233\"/></a:prstShdw>";
    let reflection = "<a:reflection blurRad=\"6350\" stA=\"50000\" endA=\"0\" endPos=\"75000\" \
                      dist=\"12700\" dir=\"5400000\" sy=\"-100000\"/>";
    let soft = "<a:softEdge rad=\"25400\"/>";
    let all = format!("{blur}{overlay}{glow}{inner}{outer}{preset}{reflection}{soft}");
    let dag = "<a:effectDag type=\"tree\" name=\"Effects\"><a:cont type=\"sib\" \
               name=\"Nested\"><a:alphaModFix amt=\"75000\"/><a:blur rad=\"12700\" \
               grow=\"0\"/><a:grayscl/></a:cont></a:effectDag>";
    for (label, effects) in [
        ("inner", format!("<a:effectLst>{inner}</a:effectLst>")),
        (
            "reflection",
            format!("<a:effectLst>{reflection}</a:effectLst>"),
        ),
        ("blur", format!("<a:effectLst>{blur}</a:effectLst>")),
        ("soft", format!("<a:effectLst>{soft}</a:effectLst>")),
        ("all", format!("<a:effectLst>{all}</a:effectLst>")),
        ("dag", dag.to_string()),
    ] {
        let object = format!(
            "<cdr:sp><cdr:nvSpPr><cdr:cNvPr id=\"2\" \
             name=\"Effects\"/><cdr:cNvSpPr/></cdr:nvSpPr><cdr:spPr><a:prstGeom \
             prst=\"rect\"><a:avLst/></a:prstGeom><a:solidFill><a:srgbClr \
             val=\"123456\"/></a:solidFill>{effects}<a:scene3d><a:camera \
             prst=\"orthographicFront\"/><a:lightRig rig=\"threePt\" \
             dir=\"t\"/></a:scene3d></cdr:spPr></cdr:sp>"
        );
        let source = package(&object);
        let expected = resolved_subtree(&drawing_xml(&source), "spPr");
        for eager in [true, false] {
            let book = open(&source, eager);
            assert!(matches!(
                book.sheet(0).unwrap().chart_collection()[0]
                    .chart_space()
                    .chart_drawing()
                    .unwrap()
                    .anchors()[0]
                    .object(),
                ChartDrawingObject::Shape(_)
            ));
            for light in [false, true] {
                let output = save(&book, light);
                assert_eq!(
                    resolved_subtree(&drawing_xml(&output), "spPr"),
                    expected,
                    "{label}"
                );
                let reread = open(&output, eager);
                assert_eq!(
                    resolved_subtree(&drawing_xml(&save(&reread, light)), "spPr"),
                    expected,
                    "reread {label}"
                );
                save_roundtrip_package(&output, &format!("shape-{label}-{eager}-{light}"));
            }
        }
    }
}

#[test]
fn projected_effect_edits_replace_authoritative_raw_effects_on_restore() {
    use umya_spreadsheet::drawing::{
        EffectList,
        charts::ChartDrawingObject,
    };
    for effects in [
        "<a:effectLst><a:innerShdw blurRad=\"25400\" dist=\"12700\"><a:srgbClr \
         val=\"123456\"/></a:innerShdw></a:effectLst>",
        "<a:effectDag><a:cont name=\"Inner\"><a:innerShdw><a:srgbClr \
         val=\"123456\"/></a:innerShdw></a:cont></a:effectDag>",
    ] {
        let object = format!(
            "<cdr:sp><cdr:nvSpPr><cdr:cNvPr id=\"2\" \
             name=\"Effects\"/><cdr:cNvSpPr/></cdr:nvSpPr><cdr:spPr><a:prstGeom \
             prst=\"rect\"><a:avLst/></a:prstGeom>{effects}</cdr:spPr></cdr:sp>"
        );
        let expected = resolved_subtree(
            &drawing_xml(&package(&object.replace(effects, "<a:effectLst/>"))),
            "spPr",
        );
        for eager in [true, false] {
            let mut book = open(&package(&object), eager);
            let original = book.sheet(0).unwrap().chart_collection()[0].clone();
            let mut projected = original.clone();
            projected.strip_export_parts();
            let mut returned: umya_spreadsheet::Chart =
                serde_json::from_str(&serde_json::to_string(&projected).unwrap()).unwrap();
            let ChartDrawingObject::Shape(shape) =
                returned.chart_space_mut().chart_drawing_mut().anchors_mut()[0].object_mut()
            else {
                panic!("typed shape");
            };
            shape
                .shape_properties_mut()
                .set_effect_list(EffectList::default());
            returned.restore_export_parts(&original).unwrap();
            book.sheet_mut(0).unwrap().chart_collection_mut()[0] = returned;
            for light in [false, true] {
                let output = save(&book, light);
                let xml = drawing_xml(&output);
                assert!(!xml.contains("innerShdw"));
                assert_eq!(resolved_subtree(&xml, "spPr"), expected);
                let reread = open(&output, eager);
                assert_eq!(
                    resolved_subtree(&drawing_xml(&save(&reread, light)), "spPr"),
                    expected
                );
            }
        }
    }
}

#[test]
fn chart_drawing_preserves_unreadable_nested_group_properties() {
    let unsupported = "<cdr:sp><cdr:nvSpPr><cdr:cNvPr id='2' \
                       name='Unsupported'/><cdr:cNvSpPr/></cdr:nvSpPr><cdr:spPr/><cdr:txBody><a:\
                       bodyPr numCol='2'/><a:lstStyle/><a:p/></cdr:txBody></cdr:sp>";
    for invalid in [
        "<a:sp3d/>",
        "<a:noFill/><a:solidFill><a:srgbClr val='123456'/></a:solidFill>",
        "<a:effectDag/><a:noFill/>",
    ] {
        let object = format!(
            "<cdr:grpSp><cdr:nvGrpSpPr><cdr:cNvPr id='3' \
             name='Outer'/><cdr:cNvGrpSpPr/></cdr:nvGrpSpPr><cdr:grpSpPr/>{unsupported}<cdr:\
             grpSp><cdr:nvGrpSpPr><cdr:cNvPr id='4' \
             name='Inner'/><cdr:cNvGrpSpPr/></cdr:nvGrpSpPr><cdr:grpSpPr>{invalid}</cdr:grpSpPr></\
             cdr:grpSp></cdr:grpSp>"
        );
        assert_preserved_object(&object, None);
    }
}

#[test]
fn chart_drawing_preserves_unreadable_shape_and_custom_dash() {
    for invalid in [
        "<a:noFill/><a:solidFill><a:srgbClr val='123456'/></a:solidFill>",
        "<a:ln><a:custDash><a:ds d='-1' sp='100000'/></a:custDash></a:ln>",
    ] {
        let object = format!(
            "<cdr:sp><cdr:nvSpPr><cdr:cNvPr id='2' \
             name='Invalid'/><cdr:cNvSpPr/></cdr:nvSpPr><cdr:spPr>{invalid}</cdr:spPr><cdr:\
             txBody><a:bodyPr numCol='2'/><a:lstStyle/><a:p/></cdr:txBody></cdr:sp>"
        );
        assert_preserved_object(&object, None);
    }
}

#[test]
fn unmodelled_drawing_properties_keep_the_complete_object() {
    use umya_spreadsheet::drawing::charts::ChartDrawingObject;

    let base = "<cdr:sp><cdr:nvSpPr><cdr:cNvPr id=\"2\" name=\"Shape \
                2\"/><cdr:cNvSpPr/></cdr:nvSpPr><cdr:spPr><a:prstGeom \
                prst=\"rect\"><a:avLst/></a:prstGeom></cdr:spPr></cdr:sp>"
        .to_string();
    let with_line = |line: &str| base.replace("</cdr:spPr>", &format!("{line}</cdr:spPr>"));
    let with_body = |body: &str| {
        base.replace(
            "</cdr:sp>",
            &format!("<cdr:txBody>{body}</cdr:txBody></cdr:sp>"),
        )
    };
    let cases = [
        with_line(
            "<a:ln><a:pattFill prst='pct5'><a:fgClr><a:srgbClr \
             val='123456'/></a:fgClr><a:bgClr><a:srgbClr \
             val='ABCDEF'/></a:bgClr></a:pattFill></a:ln>",
        ),
        with_line(
            "<a:ln><a:extLst><a:ext uri='urn:line'><payload \
             xmlns='urn:future'/></a:ext></a:extLst></a:ln>",
        ),
        with_body("<a:bodyPr numCol='2'/><a:lstStyle/><a:p/>"),
        with_body(
            "<a:bodyPr><a:prstTxWarp \
             prst='textArchUp'><a:avLst/></a:prstTxWarp></a:bodyPr><a:lstStyle/><a:p/>",
        ),
        with_body(
            "<a:bodyPr/><a:lstStyle><a:lvl1pPr marL='1000'><a:buChar \
             char='*'/></a:lvl1pPr></a:lstStyle><a:p/>",
        ),
        with_body(
            "<a:bodyPr/><a:lstStyle/><a:p><a:pPr marL='2000'><a:tabLst><a:tab pos='3000' \
             algn='l'/></a:tabLst></a:pPr><a:r><a:t>Text</a:t></a:r></a:p>",
        ),
        with_body(
            "<a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr dirty='0'><a:sym \
             typeface='Wingdings'/></a:rPr><a:t>Text</a:t></a:r></a:p>",
        ),
        with_body(
            "<a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr><a:uLnTx/></a:rPr><a:t>Text</a:t></a:r></a:\
             p>",
        ),
        with_body(
            "<a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr><a:effectLst><a:reflection \
             blurRad='0'/></a:effectLst></a:rPr><a:t>Text</a:t></a:r></a:p>",
        ),
        with_body(
            "<a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr lang=\"en-US\" \
             baseline=\"30%\"/><a:t>x</a:t></a:r></a:p>",
        ),
        with_body(
            "<a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr lang=\"en-US\" \
             kern=\"400001\"/><a:t>x</a:t></a:r></a:p>",
        ),
        base.replace(
            "name=\"Shape 2\"",
            "name=\"Shape 2\" xmlns:f='urn:future' f:flag='keep'",
        ),
        base.replace(
            "</cdr:spPr>",
            "<f:ln xmlns:f='urn:future'><f:payload/></f:ln></cdr:spPr>",
        ),
        base.replace(
            "<cdr:spPr>",
            "<cdr:spPr xmlns:mc='http://schemas.openxmlformats.org/markup-compatibility/2006' \
             mc:Ignorable='f' xmlns:f='urn:future'>",
        ),
    ];
    // Preserve a connector and its containing group as well as a shape: all
    // three formerly entered the same lossy shared readers.
    let connector = cases[0]
        .replace("cdr:sp>", "cdr:cxnSp>")
        .replace("cdr:nvSpPr", "cdr:nvCxnSpPr")
        .replace("cdr:cNvSpPr", "cdr:cNvCxnSpPr");
    let group = format!(
        "<cdr:grpSp><cdr:nvGrpSpPr><cdr:cNvPr id='3' \
         name='Group'/><cdr:cNvGrpSpPr/></cdr:nvGrpSpPr><cdr:grpSpPr/>{}</cdr:grpSp>",
        cases[2]
    );
    let nested_baseline = format!(
        "<cdr:grpSp><cdr:nvGrpSpPr><cdr:cNvPr id='3' \
         name='Group'/><cdr:cNvGrpSpPr/></cdr:nvGrpSpPr><cdr:grpSpPr/>{}</cdr:grpSp>",
        cases[9]
    );
    for object in cases.iter().chain([&connector, &group, &nested_baseline]) {
        for eager in [false, true] {
            let mut book = reader::xlsx::read_reader(Cursor::new(package(object)), eager).unwrap();
            book.read_sheet_collection();
            let drawing = book.sheet(0).unwrap().chart_collection()[0]
                .chart_space()
                .chart_drawing()
                .unwrap();
            assert!(
                matches!(
                    drawing.anchors()[0].object(),
                    ChartDrawingObject::Preserved(_)
                ),
                "{object}"
            );
            for light in [false, true] {
                let mut output = Cursor::new(Vec::new());
                if light {
                    writer::xlsx::write_writer_light(&book, &mut output).unwrap();
                } else {
                    writer::xlsx::write_writer(&book, &mut output).unwrap();
                }
                assert!(
                    drawing_xml(output.get_ref()).contains(object.as_str()),
                    "{object}"
                );
                if object.contains("baseline=") && !object.contains("grpSp") {
                    save_package(output.get_ref(), &format!("text-baseline-{eager}-{light}"));
                }
                let mut reread =
                    reader::xlsx::read_reader(Cursor::new(output.into_inner()), eager).unwrap();
                reread.read_sheet_collection();
                assert!(
                    matches!(
                        reread.sheet(0).unwrap().chart_collection()[0]
                            .chart_space()
                            .chart_drawing()
                            .unwrap()
                            .anchors()[0]
                            .object(),
                        ChartDrawingObject::Preserved(_)
                    ),
                    "{object}"
                );
            }
        }
    }
}

#[test]
fn chart_drawing_keeps_objects_the_typed_readers_cannot_preserve() {
    let shape = "<cdr:sp><cdr:nvSpPr><cdr:cNvPr id=\"2\" name=\"Shape \
                 2\"/><cdr:cNvSpPr/></cdr:nvSpPr><cdr:spPr><a:prstGeom \
                 prst=\"rect\"><a:avLst/></a:prstGeom></cdr:spPr><cdr:txBody><a:bodyPr/><a:\
                 lstStyle/><a:p><a:r><a:rPr/><a:t>x</a:t></a:r></a:p></cdr:txBody></cdr:sp>";
    for (label, object) in [
        (
            "run-spacing",
            shape.replace("<a:rPr/>", "<a:rPr spc=\"1pt\"/>"),
        ),
        (
            "miter-percent",
            shape.replace(
                "</cdr:spPr>",
                "<a:ln><a:miter lim=\"50%\"/></a:ln></cdr:spPr>",
            ),
        ),
        (
            "negative-dash",
            shape.replace(
                "</cdr:spPr>",
                "<a:ln><a:custDash><a:ds d=\"-1\" sp=\"100000\"/></a:custDash></a:ln></cdr:spPr>",
            ),
        ),
    ] {
        assert_preserved_object(&object, Some(label));
    }
    // ST_Percentage has no magnitude bound; values beyond the shared typed
    // model's i32 thousandths stay opaque instead of acquiring a false value.
    for percent in ["2147484%", "-2147484%"] {
        let object = shape.replace(
            "<a:lstStyle/>",
            &format!(
                "<a:lstStyle><a:defPPr><a:spcBef><a:spcPct \
                 val=\"{percent}\"/></a:spcBef></a:defPPr></a:lstStyle>"
            ),
        );
        assert_preserved_object(&object, None);
    }
    for level in [
        "def", "lvl1", "lvl2", "lvl3", "lvl4", "lvl5", "lvl6", "lvl7", "lvl8", "lvl9",
    ] {
        let object = shape.replace(
            "<a:lstStyle/>",
            &format!(
                "<a:lstStyle><a:{level}pPr><a:defRPr \
                 baseline=\"30%\"/></a:{level}pPr></a:lstStyle>"
            ),
        );
        let label = match level {
            "def" => Some("list-default-baseline"),
            "lvl9" => Some("list-level-baseline"),
            _ => None,
        };
        assert_preserved_object(&object, label);
        let nested = format!(
            "<cdr:grpSp><cdr:nvGrpSpPr><cdr:cNvPr id=\"3\" \
             name=\"Group\"/><cdr:cNvGrpSpPr/></cdr:nvGrpSpPr><cdr:grpSpPr/>{object}</cdr:grpSp>"
        );
        assert_preserved_object(&nested, None);
    }
}

#[test]
fn percent_string_line_spacing_loads_and_keeps_its_spelling() {
    use umya_spreadsheet::drawing::charts::ChartDrawingObject;

    let text = |spacing: &str| {
        format!(
            "<cdr:sp><cdr:nvSpPr><cdr:cNvPr id=\"2\" name=\"Shape \
             2\"/><cdr:cNvSpPr/></cdr:nvSpPr><cdr:spPr><a:prstGeom \
             prst=\"rect\"><a:avLst/></a:prstGeom></cdr:spPr><cdr:txBody><a:bodyPr/><a:lstStyle/\
             ><a:p><a:pPr><a:lnSpc>{spacing}</a:lnSpc></a:pPr><a:r><a:t>x</a:t></a:r></a:p></cdr:\
             txBody></cdr:sp>"
        )
    };
    for percent in ["150%", "-5%", "132001%"] {
        let spacing = format!("<a:spcPct val=\"{percent}\"/>");
        let valid = text(&spacing);
        for worksheet in [false, true] {
            let object = if worksheet {
                valid.replace("cdr:", "xdr:")
            } else {
                valid.clone()
            };
            for eager in [false, true] {
                let book = open(&package_in(&object, worksheet), eager);
                for light in [false, true] {
                    let output = save(&book, light);
                    assert!(
                        drawing_xml_in(&output, worksheet).contains(&spacing),
                        "worksheet={worksheet} eager={eager} light={light}"
                    );
                    let reread = open(&output, eager);
                    if !worksheet {
                        let label = match percent {
                            "150%" => "percent",
                            "-5%" => "negative",
                            _ => "large",
                        };
                        save_package(&output, &format!("text-spacing-{label}-{eager}-{light}"));
                        assert!(matches!(
                            reread.sheet(0).unwrap().chart_collection()[0]
                                .chart_space()
                                .chart_drawing()
                                .unwrap()
                                .anchors()[0]
                                .object(),
                            ChartDrawingObject::Shape(_)
                        ));
                    }
                }
            }
        }
    }
    for invalid in ["abc%", "13200001", "-1", "1e3%"] {
        let object = text(&format!("<a:spcPct val=\"{invalid}\"/>"));
        assert_preserved_object(&object, None);
        let bytes = package_in(&object.replace("cdr:", "xdr:"), true);
        assert!(
            reader::xlsx::read_reader(Cursor::new(bytes.clone()), true).is_err(),
            "{invalid} worksheet"
        );
        let mut lazy = reader::xlsx::read_reader(Cursor::new(bytes), false).unwrap();
        assert!(lazy.try_read_sheet(0).is_err(), "{invalid} worksheet");
    }
}
