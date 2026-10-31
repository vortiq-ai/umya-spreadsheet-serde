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
#[path = "support/chart_xml_fixture.rs"]
#[allow(dead_code)]
mod fixture;
const A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const X: &str = "http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing";
const R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const MC: &str = "http://schemas.openxmlformats.org/markup-compatibility/2006";
fn write(book: &Workbook) -> Vec<u8> {
    let mut out = Vec::new();
    writer::xlsx::write_writer(book, &mut out).unwrap();
    out
}
fn part(bytes: &[u8], name: &str) -> String {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut out = String::new();
    zip.by_name(name).unwrap().read_to_string(&mut out).unwrap();
    out
}
fn anchor(object: &str) -> String {
    format!(
        r#"<xdr:oneCellAnchor><xdr:from><xdr:col>2</xdr:col><xdr:colOff>17</xdr:colOff><xdr:row>3</xdr:row><xdr:rowOff>29</xdr:rowOff></xdr:from><xdr:ext cx="5400" cy="7800"/>{object}<xdr:clientData/></xdr:oneCellAnchor>"#
    )
}
fn shape(id: u32) -> String {
    format!(
        r#"<xdr:sp><xdr:nvSpPr><xdr:cNvPr id="{id}" name="Shape{id}"/><xdr:cNvSpPr/></xdr:nvSpPr><xdr:spPr/></xdr:sp>"#
    )
}
fn frame(uri: &str, content: &str) -> String {
    format!(
        r#"<xdr:graphicFrame macro="keep"><xdr:nvGraphicFramePr><xdr:cNvPr id="7" name="Payload"/><xdr:cNvGraphicFramePr/></xdr:nvGraphicFramePr><xdr:xfrm><a:off x="100" y="200"/><a:ext cx="300" cy="400"/></xdr:xfrm><a:graphic><a:graphicData uri="{uri}">{content}</a:graphicData></a:graphic></xdr:graphicFrame>"#
    )
}
fn package(drawing: &str) -> Vec<u8> {
    let mut book = umya_spreadsheet::new_file();
    book.sheet_mut(0).unwrap().add_chart(fixture::base_chart());
    let source = write(&book);
    let mut input = zip::ZipArchive::new(Cursor::new(source)).unwrap();
    let mut out = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for i in 0..input.len() {
        let mut entry = input.by_index(i).unwrap();
        let name = entry.name().unwrap().to_string();
        let mut data = Vec::new();
        entry.read_to_end(&mut data).unwrap();
        if name == "xl/drawings/drawing1.xml" {
            data=format!(r#"<xdr:wsDr xmlns:xdr="{X}" xmlns:a="{A}" xmlns:r="{R}" xmlns:mc="{MC}" xmlns:dgm="http://schemas.openxmlformats.org/drawingml/2006/diagram" xmlns:u="urn:future">{drawing}</xdr:wsDr>"#).into_bytes();
        } else if name == "xl/drawings/_rels/drawing1.xml.rels" {
            data=format!(r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="shared-chart" Type="{R}/chart" Target="../charts/chart1.xml"/><Relationship Id="data" Type="{R}/diagramData" Target="../diagrams/data1.xml"/><Relationship Id="layout" Type="{R}/diagramLayout" Target="../diagrams/layout1.xml"/><Relationship Id="style" Type="{R}/diagramQuickStyle" Target="../diagrams/style1.xml"/><Relationship Id="colors" Type="{R}/diagramColors" Target="../diagrams/colors1.xml"/><Relationship Id="ink" Type="{R}/customXml" Target="../ink/ink1.xml"/><Relationship Id="external" Type="{R}/image" Target="https://example.com/image.png?a=1&amp;b=2" TargetMode="External"/></Relationships>"#).into_bytes();
        } else if name == "[Content_Types].xml" {
            data=String::from_utf8(data).unwrap().replace("</Types>",r#"<Override PartName="/xl/diagrams/data1.xml" ContentType="application/vnd.openxmlformats-officedocument.drawingml.diagramData+xml"/><Override PartName="/xl/diagrams/layout1.xml" ContentType="application/vnd.openxmlformats-officedocument.drawingml.diagramLayout+xml"/><Override PartName="/xl/diagrams/style1.xml" ContentType="application/vnd.openxmlformats-officedocument.drawingml.diagramStyle+xml"/><Override PartName="/xl/diagrams/colors1.xml" ContentType="application/vnd.openxmlformats-officedocument.drawingml.diagramColors+xml"/><Override PartName="/xl/diagrams/drawing1.xml" ContentType="application/vnd.ms-office.drawingml.diagramDrawing+xml"/><Override PartName="/xl/ink/ink1.xml" ContentType="application/inkml+xml"/></Types>"#).into_bytes();
        }
        out.start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        out.write_all(&data).unwrap();
    }
    for (name, xml) in [
        (
            "xl/diagrams/data1.xml",
            r#"<dgm:dataModel xmlns:dgm="http://schemas.openxmlformats.org/drawingml/2006/diagram" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><dgm:ptLst/><dgm:cxnLst/><dgm:extLst><a:ext uri="http://schemas.microsoft.com/office/drawing/2008/diagram"><dsp:dataModelExt xmlns:dsp="http://schemas.microsoft.com/office/drawing/2008/diagram" relId="drawing" minVer="http://schemas.openxmlformats.org/drawingml/2006/diagram"/></a:ext></dgm:extLst></dgm:dataModel>"#,
        ),
        (
            "xl/diagrams/layout1.xml",
            r#"<dgm:layoutDef xmlns:dgm="http://schemas.openxmlformats.org/drawingml/2006/diagram" uniqueId="urn:layout"><dgm:layoutNode name="root"/></dgm:layoutDef>"#,
        ),
        (
            "xl/diagrams/style1.xml",
            r#"<dgm:styleDef xmlns:dgm="http://schemas.openxmlformats.org/drawingml/2006/diagram" uniqueId="urn:style"><dgm:styleLbl name="root"/></dgm:styleDef>"#,
        ),
        (
            "xl/diagrams/colors1.xml",
            r#"<dgm:colorsDef xmlns:dgm="http://schemas.openxmlformats.org/drawingml/2006/diagram" uniqueId="urn:colors"/>"#,
        ),
        (
            "xl/diagrams/drawing1.xml",
            r#"<dsp:drawing xmlns:dsp="http://schemas.microsoft.com/office/drawing/2008/diagram"><dsp:spTree><dsp:nvGrpSpPr><dsp:cNvPr id="0" name=""/><dsp:cNvGrpSpPr/></dsp:nvGrpSpPr><dsp:grpSpPr/></dsp:spTree></dsp:drawing>"#,
        ),
        (
            "xl/diagrams/_rels/data1.xml.rels",
            r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="drawing" Type="http://schemas.microsoft.com/office/2007/relationships/diagramDrawing" Target="drawing1.xml"/></Relationships>"#,
        ),
        (
            "xl/charts/_rels/chart1.xml.rels",
            r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="metadata" Type="urn:chart-metadata" Target="../diagrams/style1.xml"/></Relationships>"#,
        ),
        (
            "xl/ink/ink1.xml",
            r#"<ink xmlns="http://www.w3.org/2003/InkML"><trace>0 0, 1 1</trace></ink>"#,
        ),
    ] {
        out.start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        out.write_all(xml.replace("/ >", "/>").as_bytes()).unwrap();
    }
    out.finish().unwrap().into_inner()
}
fn roundtrip(drawing: &str, fragments: &[&str], dependencies: &[&str]) {
    roundtrip_source(package(drawing), fragments, dependencies);
}
fn roundtrip_source(source: Vec<u8>, fragments: &[&str], dependencies: &[&str]) {
    for eager in [true, false] {
        let mut book = reader::xlsx::read_reader(Cursor::new(&source), eager).unwrap();
        book.try_read_sheet(0).unwrap();
        #[cfg(feature = "serde")]
        {
            let saved = serde_json::to_vec(book.sheet(0).unwrap()).unwrap();
            let restored: umya_spreadsheet::Worksheet = serde_json::from_slice(&saved).unwrap();
            *book.sheet_mut(0).unwrap() = restored;
        }
        book.sheet_mut(0)
            .unwrap()
            .cell_mut("Z99")
            .set_value_number(42);
        for _ in 0..2 {
            let output = write(&book);
            let xml = part(&output, "xl/drawings/drawing1.xml");
            let mut offset = 0;
            for fragment in fragments {
                offset += xml[offset..]
                    .find(fragment)
                    .unwrap_or_else(|| panic!("missing {fragment}: {xml}"))
                    + fragment.len();
            }
            if !dependencies.is_empty() {
                let rels = part(&output, "xl/drawings/_rels/drawing1.xml.rels");
                let types = part(&output, "[Content_Types].xml");
                for dependency in dependencies {
                    if dependency.contains("_rels") {
                        assert_eq!(
                            relationships(&part(&output, dependency)),
                            relationships(&part(&source, dependency))
                        );
                    } else {
                        assert_eq!(part(&output, dependency), part(&source, dependency));
                        assert!(
                            types.contains(&format!("PartName=\"/{dependency}\"")),
                            "{types}"
                        );
                    }
                }
                assert!(!rels.is_empty());
            }
            book = reader::xlsx::read_reader(Cursor::new(output), true).unwrap();
            assert_eq!(book.sheet(0).unwrap().cell("Z99").unwrap().value(), "42");
        }
    }
}
#[test]
fn diagram_graphic_data_preserves_all_relationship_roles_and_transitive_parts() {
    let payload = r#"<dgm:relIds r:dm="data" r:lo="layout" r:qs="style" r:cs="colors"/>"#;
    let drawing = format!(
        "{}{}{}",
        anchor(&shape(6)),
        anchor(&frame(
            "http://schemas.openxmlformats.org/drawingml/2006/diagram",
            payload
        )),
        anchor(&shape(8))
    );
    roundtrip(
        &drawing,
        &[
            "Shape6",
            "Payload",
            "graphicData",
            "r:dm=",
            "r:lo=",
            "r:qs=",
            "r:cs=",
            "Shape8",
        ],
        &[
            "xl/diagrams/data1.xml",
            "xl/diagrams/layout1.xml",
            "xl/diagrams/style1.xml",
            "xl/diagrams/colors1.xml",
            "xl/diagrams/_rels/data1.xml.rels",
            "xl/diagrams/drawing1.xml",
        ],
    );
}
#[test]
fn tables_ink_and_unknown_graphic_data_keep_ordered_payloads() {
    for (uri, payload, expected) in [
        (
            "http://schemas.openxmlformats.org/drawingml/2006/table",
            r#"<a:tbl><a:tblPr firstRow="1"/><a:tblGrid><a:gridCol w="914400"/></a:tblGrid><a:tr h="370840"><a:tc><a:txBody><a:bodyPr/><a:lstStyle/><a:p/></a:txBody><a:tcPr/></a:tc></a:tr></a:tbl>"#,
            vec![
                "a:tblPr",
                "firstRow=\"1\"",
                "a:tblGrid",
                "w=\"914400\"",
                "a:tr",
                "h=\"370840\"",
                "a:tc",
            ],
        ),
        (
            "urn:ink",
            r#"<u:ink r:id="ink" state="active"><u:stroke seq="2"/><u:stroke seq="1"/></u:ink>"#,
            vec!["state=\"active\"", "seq=\"2\"", "seq=\"1\""],
        ),
        (
            "urn:future",
            r#"<u:payload flag="on"><u:first/><u:second/></u:payload>"#,
            vec!["flag=\"on\"", "u:first", "u:second"],
        ),
    ] {
        roundtrip(
            &anchor(&frame(uri, payload)),
            &expected,
            if uri == "urn:ink" {
                &["xl/ink/ink1.xml"]
            } else {
                &[]
            },
        );
    }
}
#[test]
fn unknown_alternate_content_preserves_both_branches_and_their_relationships() {
    let alternate = format!(
        r#"<mc:AlternateContent><mc:Choice Requires="u">{}</mc:Choice><mc:Fallback>{}</mc:Fallback></mc:AlternateContent>"#,
        anchor(&frame("urn:future", r#"<u:data r:id="ink"/>"#)),
        anchor(&frame("urn:future", r#"<u:data r:id="data"/>"#))
    );
    roundtrip(
        &format!("{}{}{}", anchor(&shape(6)), alternate, anchor(&shape(8))),
        &[
            "Shape6",
            "mc:AlternateContent",
            "Requires=\"u\"",
            "mc:Fallback",
            "Shape8",
        ],
        &[
            "xl/ink/ink1.xml",
            "xl/diagrams/data1.xml",
            "xl/diagrams/drawing1.xml",
        ],
    );
}
#[test]
fn anchor_and_group_content_parts_keep_ink_relationship_and_position() {
    for object in [
        r#"<xdr:contentPart r:id="ink"/>"#.to_owned(),
        format!(
            r#"<xdr:grpSp><xdr:nvGrpSpPr><xdr:cNvPr id="7" name="InkGroup"/><xdr:cNvGrpSpPr/></xdr:nvGrpSpPr><xdr:grpSpPr/>{}<xdr:contentPart r:id="ink"/>{}</xdr:grpSp>"#,
            shape(9),
            shape(10)
        ),
    ] {
        roundtrip(
            &format!(
                "{}{}{}",
                anchor(&shape(6)),
                anchor(&object),
                anchor(&shape(8))
            ),
            &["Shape6", "xdr:contentPart", "Shape8"],
            &["xl/ink/ink1.xml"],
        );
    }
}

#[test]
fn supported_alternate_choices_use_namespace_identity() {
    let alternate = format!(
        r#"<mc:AlternateContent><mc:Choice xmlns:supported="{X}" Requires="supported">{}</mc:Choice><mc:Fallback>{}</mc:Fallback></mc:AlternateContent>"#,
        anchor(&shape(7)),
        anchor(&shape(99))
    );
    let source = package(&alternate);
    let mut book = reader::xlsx::read_reader(Cursor::new(source), true).unwrap();
    book.sheet_mut(0)
        .unwrap()
        .cell_mut("Z99")
        .set_value_number(42);
    assert_eq!(
        book.sheet(0)
            .unwrap()
            .worksheet_drawing()
            .one_cell_anchor_collection()
            .len(),
        1
    );
    let output = write(&book);
    let xml = part(&output, "xl/drawings/drawing1.xml");
    assert!(xml.contains("Shape7"));
    assert!(!xml.contains("Shape99"));
    assert!(reader::xlsx::read_reader(Cursor::new(output), true).is_ok());
}
#[test]
fn unknown_alternate_inside_anchor_keeps_both_payloads() {
    let object = format!(
        r#"<mc:AlternateContent><mc:Choice Requires="u">{}</mc:Choice><mc:Fallback>{}</mc:Fallback></mc:AlternateContent>"#,
        frame("urn:future", r#"<u:data r:id="ink"/>"#),
        shape(9)
    );
    roundtrip(
        &anchor(&object),
        &[
            "mc:AlternateContent",
            "mc:Choice",
            "Payload",
            "mc:Fallback",
            "Shape9",
        ],
        &["xl/ink/ink1.xml"],
    );
}
#[cfg(feature = "serde")]
#[test]
fn older_drawing_json_without_preserved_objects_loads() {
    let drawing = umya_spreadsheet::drawing::spreadsheet::WorksheetDrawing::default();
    let mut json = serde_json::to_value(drawing).unwrap();
    json.as_object_mut().unwrap().remove("preserved_objects");
    let _: umya_spreadsheet::drawing::spreadsheet::WorksheetDrawing =
        serde_json::from_value(json).unwrap();
}

fn relationships(xml: &str) -> Vec<std::collections::BTreeMap<String, String>> {
    let mut reader = quick_xml::Reader::from_str(xml);
    let mut result = Vec::new();
    loop {
        match reader.read_event().unwrap() {
            quick_xml::events::Event::Start(start) | quick_xml::events::Event::Empty(start)
                if start.local_name().as_ref() == b"Relationship" =>
            {
                let attributes = start
                    .attributes()
                    .map(|attribute| {
                        let attribute = attribute.unwrap();
                        (
                            String::from_utf8(attribute.key.as_ref().to_vec()).unwrap(),
                            attribute
                                .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                                .unwrap()
                                .into_owned(),
                        )
                    })
                    .collect();
                result.push(attributes);
            }
            quick_xml::events::Event::Eof => break,
            _ => (),
        }
    }
    result
}

#[test]
fn native_chart_names_inside_unknown_choices_do_not_select_a_chart_owner() {
    let object = format!(
        r#"<mc:AlternateContent><mc:Choice Requires="u"><u:future xmlns:cx="http://schemas.microsoft.com/office/drawing/2014/chartex"><cx:chart r:id="ink"/></u:future></mc:Choice><mc:Fallback>{}</mc:Fallback></mc:AlternateContent>"#,
        shape(9)
    );
    roundtrip(
        &anchor(&object),
        &[
            "mc:AlternateContent",
            "u:future",
            "cx:chart",
            "mc:Fallback",
            "Shape9",
        ],
        &["xl/ink/ink1.xml"],
    );
}

#[test]
fn opaque_payloads_keep_native_identity_and_geometry_validation() {
    for object in [
        frame("urn:future", "<u:data/>").replace("id=\"7\"", "id=\"bogus\""),
        frame("urn:future", "<u:data/>").replace("id=\"7\"", ""),
    ] {
        let source = package(&anchor(&object));
        assert!(reader::xlsx::read_reader(Cursor::new(source), true).is_err());
    }
    let source =
        package(&anchor(&frame("urn:future", "<u:data/>")).replace("cx=\"5400\"", "cx=\"-1\""));
    assert!(reader::xlsx::read_reader(Cursor::new(source), true).is_err());
}
#[test]
fn supported_choices_inside_groups_are_selected_by_namespace() {
    let group = format!(
        r#"<xdr:grpSp><xdr:nvGrpSpPr><xdr:cNvPr id="7" name="Group"/><xdr:cNvGrpSpPr/></xdr:nvGrpSpPr><xdr:grpSpPr/><mc:AlternateContent><mc:Choice Requires="xdr">{}</mc:Choice><mc:Fallback>{}</mc:Fallback></mc:AlternateContent></xdr:grpSp>"#,
        frame("urn:future", "<u:data/>").replace("id=\"7\"", "id=\"11\""),
        shape(99)
    );
    let source = package(&anchor(&group));
    let mut book = reader::xlsx::read_reader(Cursor::new(source), true).unwrap();
    book.sheet_mut(0)
        .unwrap()
        .cell_mut("Z99")
        .set_value_number(42);
    let output = write(&book);
    let xml = part(&output, "xl/drawings/drawing1.xml");
    assert!(xml.contains("Payload"));
    assert!(!xml.contains("Shape99"));
    assert!(reader::xlsx::read_reader(Cursor::new(output), true).is_ok());
}

#[test]
fn opaque_and_modeled_references_share_source_chart_dependencies() {
    let opaque = anchor(&frame("urn:future", r#"<u:chart r:id="shared-chart"/>"#));
    let modeled=anchor(&frame("http://schemas.openxmlformats.org/drawingml/2006/chart",r#"<c:chart xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart" r:id="shared-chart"/>"#).replace("id=\"7\"","id=\"11\""));
    roundtrip(
        &format!("{opaque}{modeled}"),
        &[
            "urn:future",
            "u:chart",
            "http://schemas.openxmlformats.org/drawingml/2006/chart",
        ],
        &[
            "xl/charts/chart1.xml",
            "xl/charts/_rels/chart1.xml.rels",
            "xl/diagrams/style1.xml",
        ],
    );
}

#[test]
fn supported_choices_inside_shape_properties_select_native_geometry() {
    let object=shape(7).replace("<xdr:spPr/>",r#"<xdr:spPr><mc:AlternateContent><mc:Choice Requires="a"><a:custGeom><a:avLst/><a:gdLst/><a:ahLst/><a:cxnLst/><a:rect l="l" t="t" r="r" b="b"/><a:pathLst/></a:custGeom></mc:Choice><mc:Fallback><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></mc:Fallback></mc:AlternateContent></xdr:spPr>"#);
    let source = package(&anchor(&object));
    let mut book = reader::xlsx::read_reader(Cursor::new(source), true).unwrap();
    book.sheet_mut(0)
        .unwrap()
        .cell_mut("Z99")
        .set_value_number(42);
    let output = write(&book);
    let xml = part(&output, "xl/drawings/drawing1.xml");
    assert!(xml.contains("a:custGeom"));
    assert!(!xml.contains("a:prstGeom"));
    assert!(reader::xlsx::read_reader(Cursor::new(output), true).is_ok());
}

#[test]
fn opaque_chart_dependencies_preserve_nested_user_shape_charts() {
    let source = package(&anchor(&frame(
        "urn:future",
        r#"<u:chart r:id="shared-chart"/>"#,
    )));
    let chart = part(&source, "xl/charts/chart1.xml").replace(
        "</c:chartSpace>",
        &format!(r#"<c:userShapes xmlns:r="{R}" r:id="shapes"/></c:chartSpace>"#),
    );
    let types = part(&source,"[Content_Types].xml").replace("</Types>",r#"<Override PartName="/xl/drawings/chartShapes1.xml" ContentType="application/vnd.openxmlformats-officedocument.drawingml.chartshapes+xml"/><Override PartName="/xl/charts/chart2.xml" ContentType="application/vnd.openxmlformats-officedocument.drawingml.chart+xml"/></Types>"#);
    let replacements = [
        ("xl/charts/chart1.xml",chart),
        ("[Content_Types].xml",types),
        ("xl/charts/_rels/chart1.xml.rels",format!(r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="shapes" Type="{R}/chartUserShapes" Target="../drawings/chartShapes1.xml"/></Relationships>"#)),
        ("xl/drawings/chartShapes1.xml",format!(r#"<cdr:userShapes xmlns:cdr="http://schemas.openxmlformats.org/drawingml/2006/chartDrawing" xmlns:a="{A}" xmlns:r="{R}"><cdr:relSizeAnchor><cdr:from><cdr:x>0</cdr:x><cdr:y>0</cdr:y></cdr:from><cdr:to><cdr:x>1</cdr:x><cdr:y>1</cdr:y></cdr:to><cdr:graphicFrame><cdr:nvGraphicFramePr><cdr:cNvPr id="2" name="Nested chart"/><cdr:cNvGraphicFramePr/></cdr:nvGraphicFramePr><cdr:xfrm><a:off x="0" y="0"/><a:ext cx="914400" cy="914400"/></cdr:xfrm><a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/chart"><c:chart xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart" r:id="nested"/></a:graphicData></a:graphic></cdr:graphicFrame></cdr:relSizeAnchor></cdr:userShapes>"#)),
        ("xl/drawings/_rels/chartShapes1.xml.rels",format!(r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="nested" Type="{R}/chart" Target="../charts/chart2.xml"/></Relationships>"#)),
        ("xl/charts/chart2.xml",r#"<c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart"><c:chart><c:plotArea><c:pieChart/></c:plotArea></c:chart></c:chartSpace>"#.to_owned()),
    ];
    let mut input = zip::ZipArchive::new(Cursor::new(&source)).unwrap();
    let mut out = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for index in 0..input.len() {
        let mut entry = input.by_index(index).unwrap();
        let name = entry.name().unwrap().to_string();
        if replacements.iter().any(|(target, _)| *target == name) {
            continue;
        }
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).unwrap();
        out.start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        out.write_all(&bytes).unwrap();
    }
    for (name, xml) in replacements {
        out.start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        out.write_all(xml.as_bytes()).unwrap();
    }
    roundtrip_source(
        out.finish().unwrap().into_inner(),
        &["urn:future", "u:chart"],
        &[
            "xl/charts/chart1.xml",
            "xl/charts/_rels/chart1.xml.rels",
            "xl/drawings/chartShapes1.xml",
            "xl/drawings/_rels/chartShapes1.xml.rels",
            "xl/charts/chart2.xml",
        ],
    );
}
