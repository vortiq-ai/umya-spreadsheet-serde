use std::io::{
    Cursor,
    Read,
    Write,
};

use umya_spreadsheet::{
    reader,
    writer,
};
#[path = "support/chart_xml_fixture.rs"]
#[allow(dead_code)]
mod fixture;
const A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const X: &str = "http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing";
fn package(properties: &str, style: &str) -> Vec<u8> {
    let mut book = umya_spreadsheet::new_file();
    book.sheet_mut(0).unwrap().add_chart(fixture::base_chart());
    let mut original = Vec::new();
    writer::xlsx::write_writer(&book, &mut original).unwrap();
    let mut input = zip::ZipArchive::new(Cursor::new(original)).unwrap();
    let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for i in 0..input.len() {
        let mut entry = input.by_index(i).unwrap();
        let name = entry.name().unwrap().to_string();
        let mut data = Vec::new();
        entry.read_to_end(&mut data).unwrap();
        if name == "xl/drawings/drawing1.xml" {
            let object = if properties.starts_with("<xdr:grpSp>") {
                properties.to_string()
            } else {
                format!(
                    r#"<xdr:sp><xdr:nvSpPr><xdr:cNvPr id="9" name="Preserved"/><xdr:cNvSpPr/></xdr:nvSpPr><xdr:spPr>{properties}</xdr:spPr>{style}</xdr:sp>"#
                )
            };
            data = format!(r#"<xdr:wsDr xmlns:xdr="{X}" xmlns:a="{A}"><xdr:twoCellAnchor><xdr:from><xdr:col>0</xdr:col><xdr:colOff>0</xdr:colOff><xdr:row>0</xdr:row><xdr:rowOff>0</xdr:rowOff></xdr:from><xdr:to><xdr:col>3</xdr:col><xdr:colOff>0</xdr:colOff><xdr:row>4</xdr:row><xdr:rowOff>0</xdr:rowOff></xdr:to>{object}<xdr:clientData/></xdr:twoCellAnchor></xdr:wsDr>"#).into_bytes();
        }
        if name == "xl/drawings/_rels/drawing1.xml.rels" && properties.starts_with("<xdr:grpSp>") {
            data = String::from_utf8(data).unwrap().replace("</Relationships>", r#"<Relationship Id="groupAux" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://example.com/group?x=1&amp;y=2" TargetMode="External"/></Relationships>"#).into_bytes();
        }
        output
            .start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        output.write_all(&data).unwrap();
    }
    output.finish().unwrap().into_inner()
}
fn roundtrip(source: Vec<u8>, expected: &[&str]) {
    for eager in [true, false] {
        let mut book = reader::xlsx::read_reader(Cursor::new(&source), eager).unwrap();
        book.try_read_sheet(0).unwrap();
        book.sheet_mut(0)
            .unwrap()
            .cell_mut("A1")
            .set_value_number(42);
        for _ in 0..2 {
            #[cfg(feature = "serde")]
            {
                let json =
                    serde_json::to_value(book.sheet(0).unwrap().worksheet_drawing()).unwrap();
                let restored = serde_json::from_value(json).unwrap();
                book.sheet_mut(0).unwrap().set_worksheet_drawing(restored);
            }
            let mut bytes = Vec::new();
            writer::xlsx::write_writer(&book, &mut bytes).unwrap();
            let mut zip = zip::ZipArchive::new(Cursor::new(&bytes)).unwrap();
            let mut xml = String::new();
            zip.by_name("xl/drawings/drawing1.xml")
                .unwrap()
                .read_to_string(&mut xml)
                .unwrap();
            if expected.contains(&"urn:preserved-group") {
                let mut relationships = String::new();
                zip.by_name("xl/drawings/_rels/drawing1.xml.rels")
                    .unwrap()
                    .read_to_string(&mut relationships)
                    .unwrap();
                assert!(relationships.contains("https://example.com/group?x=1&amp;y=2"));
                assert!(relationships.contains("TargetMode=\"External\""));
                let mut reader = quick_xml::Reader::from_str(&relationships);
                loop {
                    match reader.read_event().unwrap() {
                        quick_xml::events::Event::Empty(start)
                        | quick_xml::events::Event::Start(start)
                            if start.name().as_ref() == b"Relationship" =>
                        {
                            let attributes: Vec<_> = start
                                .attributes()
                                .map(|attribute| attribute.unwrap())
                                .collect();
                            if attributes.iter().any(|attribute| {
                                attribute.key.as_ref() == b"Target"
                                    && attribute.value.starts_with(b"https://example.com/group")
                            }) {
                                let id = attributes
                                    .iter()
                                    .find(|attribute| attribute.key.as_ref() == b"Id")
                                    .unwrap();
                                assert!(xml.contains(&format!(
                                    "r:id=\"{}\"",
                                    std::str::from_utf8(&id.value).unwrap()
                                )));
                            }
                        }
                        quick_xml::events::Event::Eof => break,
                        _ => (),
                    }
                }
            }
            let mut content_types = String::new();
            zip.by_name("[Content_Types].xml")
                .unwrap()
                .read_to_string(&mut content_types)
                .unwrap();
            assert!(
                content_types.contains("application/vnd.openxmlformats-officedocument.drawing+xml")
            );
            let mut offset = 0;
            for value in expected {
                offset += xml[offset..]
                    .find(value)
                    .unwrap_or_else(|| panic!("missing {value}: {xml}"))
                    + value.len();
            }
            if expected.contains(&"<a:custGeom") {
                assert!(
                    !xml.contains("<a:prstGeom"),
                    "custom geometry must not gain a default preset"
                );
            }
            book = reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap();
            assert_eq!(book.sheet(0).unwrap().cell("A1").unwrap().value(), "42");
        }
    }
}
#[test]
fn custom_geometry_pattern_fill_and_three_dimensional_effects_survive() {
    let properties = r#"<a:custGeom><a:avLst/><a:gdLst/><a:ahLst/><a:cxnLst/><a:rect l="0" t="0" r="r" b="b"/><a:pathLst><a:path w="100" h="100"><a:moveTo><a:pt x="0" y="0"/></a:moveTo><a:lnTo><a:pt x="100" y="100"/></a:lnTo></a:path></a:pathLst></a:custGeom><a:pattFill prst="pct5"><a:fgClr><a:srgbClr val="123456"/></a:fgClr><a:bgClr><a:srgbClr val="ABCDEF"/></a:bgClr></a:pattFill><a:effectDag name="effects"><a:cont name="nested"/></a:effectDag><a:scene3d><a:camera prst="orthographicFront"/><a:lightRig rig="threePt" dir="t"/></a:scene3d><a:sp3d z="12700"><a:bevelT w="25400" h="12700"/></a:sp3d>"#;
    roundtrip(
        package(properties, ""),
        &[
            "<a:custGeom",
            "<a:avLst",
            "<a:pathLst",
            "w=\"100\"",
            "<a:moveTo",
            "<a:lnTo",
            "<a:pattFill",
            "prst=\"pct5\"",
            "123456",
            "ABCDEF",
            "<a:effectDag",
            "name=\"effects\"",
            "name=\"nested\"",
            "<a:scene3d",
            "orthographicFront",
            "threePt",
            "<a:sp3d",
            "z=\"12700\"",
            "<a:bevelT",
        ],
    );
}
#[test]
fn style_references_preserve_all_color_choices_and_transform_order() {
    for color in [
        r#"<a:srgbClr val="123456"><a:alpha val="75000"/><a:shade val="50000"/></a:srgbClr>"#,
        r#"<a:scrgbClr r="100000" g="50000" b="0"><a:tint val="25000"/></a:scrgbClr>"#,
        r#"<a:hslClr hue="120000" sat="50000" lum="75000"/>"#,
        r#"<a:sysClr val="windowText" lastClr="102030"><a:lumMod val="50000"/></a:sysClr>"#,
        r#"<a:prstClr val="red"><a:inv/></a:prstClr>"#,
    ] {
        let style = format!(
            r#"<xdr:style><a:lnRef idx="1">{color}</a:lnRef><a:fillRef idx="2">{color}</a:fillRef><a:effectRef idx="3">{color}</a:effectRef><a:fontRef idx="minor">{color}</a:fontRef></xdr:style>"#
        );
        let tags: Vec<_> = color
            .split('<')
            .filter(|tag| !tag.is_empty() && !tag.starts_with('/'))
            .map(|tag| format!("<{}", tag.split('>').next().unwrap().trim_end_matches('/')))
            .collect();
        let mut expected = Vec::new();
        for reference in ["<a:lnRef", "<a:fillRef", "<a:effectRef", "<a:fontRef"] {
            expected.push(reference);
            expected.extend(tags.iter().map(String::as_str));
        }
        roundtrip(package("", &style), &expected);
    }
}

#[test]
fn group_fills_effects_scene_and_extensions_survive_in_order() {
    let object = r#"<xdr:grpSp><xdr:nvGrpSpPr><xdr:cNvPr id="10" name="Group"/><xdr:cNvGrpSpPr/></xdr:nvGrpSpPr><xdr:grpSpPr bwMode="gray"><a:pattFill prst="pct10"><a:fgClr><a:srgbClr val="112233"/></a:fgClr><a:bgClr><a:srgbClr val="AABBCC"/></a:bgClr></a:pattFill><a:effectDag name="groupEffects"><a:cont/></a:effectDag><a:scene3d><a:camera prst="orthographicFront"/><a:lightRig rig="threePt" dir="t"/></a:scene3d><a:extLst><a:ext uri="urn:preserved-group"><q:payload xmlns:q="urn:group-payload" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" r:id="groupAux" value="retained"/></a:ext></a:extLst></xdr:grpSpPr></xdr:grpSp>"#;
    roundtrip(
        package(object, ""),
        &[
            "<xdr:grpSp",
            "id=\"10\"",
            "<xdr:grpSpPr",
            "bwMode=\"gray\"",
            "<a:pattFill",
            "pct10",
            "112233",
            "AABBCC",
            "<a:effectDag",
            "groupEffects",
            "<a:scene3d",
            "orthographicFront",
            "threePt",
            "<a:extLst",
            "urn:preserved-group",
            "urn:group-payload",
            "retained",
        ],
    );
}

#[cfg(feature = "serde")]
#[test]
fn older_shape_properties_json_omitting_preserved_children_loads() {
    let value = umya_spreadsheet::drawing::spreadsheet::ShapeProperties::default();
    let mut json = serde_json::to_value(value).unwrap();
    json.as_object_mut().unwrap().remove("preserved_properties");
    let _: umya_spreadsheet::drawing::spreadsheet::ShapeProperties =
        serde_json::from_value(json).unwrap();
}

#[test]
fn conflicting_shape_property_choices_remain_explicit_errors() {
    for properties in [
        "<a:grpFill/><a:noFill/>",
        "<a:effectDag/><a:effectLst/>",
        "<a:scene3d/><a:grpFill/>",
    ] {
        assert!(reader::xlsx::read_reader(Cursor::new(package(properties, "")), true).is_err());
    }
}

#[cfg(feature = "serde")]
#[test]
fn older_style_reference_json_omitting_new_color_choice_loads() {
    let value = umya_spreadsheet::drawing::StyleMatrixReferenceType::default();
    let mut json = serde_json::to_value(value).unwrap();
    json.as_object_mut().unwrap().remove("color_choice");
    let _: umya_spreadsheet::drawing::StyleMatrixReferenceType =
        serde_json::from_value(json).unwrap();
}
