use std::io::{
    Cursor,
    Read,
    Write,
};

use umya_spreadsheet::{
    Workbook,
    drawing::charts::ChartExSeriesLayout,
    reader,
    writer,
};

#[path = "support/chart_xml_fixture.rs"]
#[allow(dead_code)]
mod fixture;

#[cfg(feature = "serde")]
use umya_spreadsheet::Worksheet;

const CX: &str = "http://schemas.microsoft.com/office/drawing/2014/chartex";
const C: &str = "http://schemas.openxmlformats.org/drawingml/2006/chart";
const R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const NATIVE_TYPE: &str = "application/vnd.ms-office.chartex+xml";
const SOUND: &[u8] = b"RIFF\x26\0\0\0WAVEfmt \x10\0\0\0\x01\0\x01\0\x40\x1f\0\0\x80\x3e\0\0\x02\0\x10\0data\x02\0\0\0\0\0";

const STYLE: &[u8] = br#"<s:chartStyle xmlns:s="urn:example:chart-style" id="201"/>"#;

fn native_xml() -> String {
    format!(
        r#"<cx:chartSpace xmlns:cx="{CX}" xmlns:r="{R}"><cx:chartData><cx:externalData r:id="book" autoUpdate="0"/><cx:data id="7"><cx:strDim type="cat"><cx:lvl ptCount="1"><cx:pt idx="0">A</cx:pt></cx:lvl><cx:lvl ptCount="1"><cx:pt idx="0">Parent</cx:pt></cx:lvl></cx:strDim><cx:numDim type="val"><cx:lvl ptCount="1"><cx:pt idx="0">12.5</cx:pt></cx:lvl></cx:numDim></cx:data></cx:chartData><cx:chart><cx:plotArea><cx:plotAreaRegion><cx:series layoutId="waterfall"><cx:dataId val="7"/></cx:series></cx:plotAreaRegion></cx:plotArea></cx:chart></cx:chartSpace>"#
    )
}
fn package(native: bool, omit_style: bool, hyperlink: bool) -> Vec<u8> {
    let mut book = umya_spreadsheet::new_file();
    book.sheet_mut(0).unwrap().add_chart(fixture::base_chart());
    let source = fixture::write_workbook(&book);
    let mut input = zip::ZipArchive::new(Cursor::new(&source)).unwrap();
    let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for index in 0..input.len() {
        let mut entry = input.by_index(index).unwrap();
        let name = entry.name().unwrap().to_string();
        let mut data = Vec::new();
        entry.read_to_end(&mut data).unwrap();
        if name == "xl/charts/chart1.xml" && native {
            data = native_xml().into_bytes();
        }
        if name == "xl/drawings/drawing1.xml" {
            let mut xml = String::from_utf8(data).unwrap();
            if native {
                xml = xml.replace(C, CX);
            }
            if hyperlink {
                let start = xml.find("<xdr:cNvPr").unwrap();
                let end = start + xml[start..].find("/>").unwrap();
                xml.replace_range(end..end+2,&format!(r#"><a:hlinkClick xmlns:r="{R}" r:id="link-old"><a:snd r:embed="sound-old" name="beep.wav"/></a:hlinkClick></xdr:cNvPr>"#));
            }
            data = xml.into_bytes();
        }
        if name == "xl/drawings/_rels/drawing1.xml.rels" {
            let mut xml = String::from_utf8(data).unwrap();
            if native {
                xml = xml.replace(
                    &format!("{R}/chart"),
                    "http://schemas.microsoft.com/office/2014/relationships/chartEx",
                );
            }
            if hyperlink {
                xml=xml.replace("</Relationships>",&format!(r#"<Relationship Id="link-old" Type="{R}/hyperlink" Target="https://example.com/?a=1&amp;b=2" TargetMode="External"/><Relationship Id="sound-old" Type="{R}/audio" Target="../media/beep.wav"/></Relationships>"#));
            }
            data = xml.into_bytes();
        }
        if name == "[Content_Types].xml" {
            let mut xml = String::from_utf8(data).unwrap();
            if native {
                xml = xml.replace(
                    "application/vnd.openxmlformats-officedocument.drawingml.chart+xml",
                    NATIVE_TYPE,
                );
            }
            xml=xml.replace("</Types>",r#"<Override PartName="/xl/charts/chartStyle1.xml" ContentType="application/vnd.example.chartstyle+xml"/></Types>"#);
            xml = xml.replace("</Types>", r#"<Default Extension="xlsx" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"/></Types>"#);
            if hyperlink {
                xml = xml.replace(
                    "</Types>",
                    r#"<Default Extension="wav" ContentType="audio/wav"/></Types>"#,
                );
            }
            data = xml.into_bytes();
        }
        add(&mut output, &name, &data);
    }
    let relationships = format!(
        r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="style" Type="urn:example:chart-style" Target="chartStyle1.xml"/><Relationship Id="book" Type="{R}/package" Target="../embeddings/source.xlsx"/></Relationships>"#
    );
    add(
        &mut output,
        "xl/charts/_rels/chart1.xml.rels",
        relationships.as_bytes(),
    );
    if !omit_style {
        add(&mut output, "xl/charts/chartStyle1.xml", STYLE);
    }
    add(&mut output, "xl/embeddings/source.xlsx", &source);
    if hyperlink {
        add(&mut output, "xl/media/beep.wav", SOUND);
    }
    output.finish().unwrap().into_inner()
}
fn add(output: &mut zip::ZipWriter<Cursor<Vec<u8>>>, name: &str, data: &[u8]) {
    output
        .start_file(name, zip::write::SimpleFileOptions::default())
        .unwrap();
    output.write_all(data).unwrap();
}
fn write(book: &Workbook) -> Vec<u8> {
    let mut bytes = Vec::new();
    writer::xlsx::write_writer(book, &mut bytes).unwrap();
    bytes
}
fn part(bytes: &[u8], name: &str) -> Vec<u8> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut data = Vec::new();
    archive
        .by_name(name)
        .unwrap()
        .read_to_end(&mut data)
        .unwrap();
    data
}
fn xml(bytes: &[u8], name: &str) -> String {
    String::from_utf8(part(bytes, name)).unwrap()
}
fn assert_dependencies(bytes: &[u8]) {
    assert_eq!(part(bytes, "xl/charts/chartStyle1.xml"), STYLE);
    assert!(xml(bytes, "xl/charts/_rels/chart1.xml.rels").contains("../embeddings/source.xlsx"));
    let embedded = part(bytes, "xl/embeddings/source.xlsx");
    assert!(zip::ZipArchive::new(Cursor::new(embedded)).is_ok());
    assert!(xml(bytes, "[Content_Types].xml").contains("application/vnd.example.chartstyle+xml"));
}

#[test]
fn grouped_chart_frames_preserve_native_order_keys_and_dependencies() {
    for native in [false, true] {
        let source = package(native, false, true);
        let mut archive = zip::ZipArchive::new(Cursor::new(&source)).unwrap();
        let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for index in 0..archive.len() {
            let mut entry = archive.by_index(index).unwrap();
            let name = entry.name().unwrap().to_string();
            let mut data = Vec::new();
            entry.read_to_end(&mut data).unwrap();
            if name == "xl/drawings/drawing1.xml" {
                let mut xml = String::from_utf8(data).unwrap();
                let start = xml.find("<xdr:graphicFrame").unwrap();
                let end = start
                    + xml[start..].find("</xdr:graphicFrame>").unwrap()
                    + "</xdr:graphicFrame>".len();
                let frame = &xml[start..end];
                let group = |id: u32, content: &str| {
                    format!(
                        r#"<xdr:grpSp><xdr:nvGrpSpPr><xdr:cNvPr id="{id}" name="Group{id}"/><xdr:cNvGrpSpPr/></xdr:nvGrpSpPr><xdr:grpSpPr/>{content}</xdr:grpSp>"#,
                    )
                };
                let shape = |id: u32, label: &str| {
                    format!(
                        r#"<xdr:sp><xdr:nvSpPr><xdr:cNvPr id="{id}" name="{label}"/><xdr:cNvSpPr/></xdr:nvSpPr><xdr:spPr/></xdr:sp>"#,
                    )
                };
                let content = format!(
                    "{}{}{}",
                    shape(102, "BeforeFrame"),
                    group(101, frame),
                    shape(103, "AfterFrame")
                );
                xml.replace_range(start..end, &group(100, &content));
                data = xml.into_bytes();
            }
            add(&mut output, &name, &data);
        }
        let input = output.finish().unwrap().into_inner();
        for eager in [true, false] {
            let mut book = reader::xlsx::read_reader(Cursor::new(&input), eager).unwrap();
            if !eager {
                book.try_read_sheet(0).unwrap();
            }
            let drawing = book.sheet(0).unwrap().worksheet_drawing();
            let mut keys = Vec::new();
            drawing.visit_standard_frames(&mut |key, _| keys.push(key.relationship_key()));
            drawing.visit_extended_frames(&mut |key, _| keys.push(key.relationship_key()));
            assert_eq!(keys, vec!["1"]);
            if native {
                book.sheet_mut(0)
                    .unwrap()
                    .worksheet_drawing_mut()
                    .visit_extended_frames_mut(&mut |_, frame| {
                        frame.chart_space_mut().chart_mut().unwrap().series_mut()[0]
                            .set_layout(ChartExSeriesLayout::Sunburst);
                    });
            }
            let written = write(&book);
            let xml = xml(&written, "xl/drawings/drawing1.xml");
            assert_eq!(xml.matches("<xdr:grpSp>").count(), 2);
            assert!(xml.find("BeforeFrame").unwrap() < xml.find("<xdr:graphicFrame").unwrap());
            assert!(xml.find("<xdr:graphicFrame").unwrap() < xml.find("AfterFrame").unwrap());
            assert!(xml.contains("hlinkClick"));
            assert_dependencies(&written);
            assert_eq!(part(&written, "xl/media/beep.wav"), SOUND);
            let reread = reader::xlsx::read_reader(Cursor::new(&written), true).unwrap();
            let mut after = Vec::new();
            let drawing = reread.sheet(0).unwrap().worksheet_drawing();
            drawing.visit_standard_frames(&mut |key, _| after.push(key.relationship_key()));
            drawing.visit_extended_frames(&mut |key, _| after.push(key.relationship_key()));
            assert_eq!(after, keys);
            if native {
                drawing.visit_extended_frames(&mut |_, frame| {
                    assert_eq!(
                        frame.chart_space().chart().unwrap().series()[0].layout(),
                        ChartExSeriesLayout::Sunburst
                    );
                });
            }
        }
    }
}

#[test]
fn invalid_metadata_content_fails_eager_reads_and_rolls_back_lazy_reads() {
    let source = package(false, false, false);
    for owner in ["xdr:nvGraphicFramePr", "xdr:cNvGraphicFramePr", "a:graphic"] {
        for content in ["lost", "<![CDATA[lost]]>", "&#65;", "&missing;", "&#0;"] {
            let bytes = replace_part(&source, "xl/drawings/drawing1.xml", |xml| {
                if owner == "xdr:cNvGraphicFramePr" {
                    xml.replace(
                        &format!("<{owner}/>"),
                        &format!("<{owner}>{content}</{owner}>"),
                    )
                } else {
                    xml.replace(&format!("<{owner}>"), &format!("<{owner}>{content}"))
                }
            });
            assert!(
                reader::xlsx::read_reader(Cursor::new(&bytes), true).is_err(),
                "accepted {content:?} in {owner}"
            );
            let mut book = reader::xlsx::read_reader(Cursor::new(bytes), false).unwrap();
            for _ in 0..2 {
                assert!(book.try_read_sheet(0).is_err());
                assert!(book.sheet(0).is_err());
            }
        }
        for content in [
            "<![CDATA[ \t\r\n]]>",
            "&#x20;&#x9;&#xD;&#xA;",
            "<!--comment--><?allowed data?>",
        ] {
            let bytes = replace_part(&source, "xl/drawings/drawing1.xml", |xml| {
                if owner == "xdr:cNvGraphicFramePr" {
                    xml.replace(
                        &format!("<{owner}/>"),
                        &format!("<{owner}>{content}</{owner}>"),
                    )
                } else {
                    xml.replace(&format!("<{owner}>"), &format!("<{owner}>{content}"))
                }
            });
            reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap();
        }
    }
}

#[test]
fn drawing_metadata_preserves_aliases_default_namespaces_and_qname_values() {
    for default_namespace in [false, true] {
        let source = replace_part(
            &package(false, false, true),
            "xl/drawings/drawing1.xml",
            |xml| {
                let mut xml = xml
                    .replace("xdr:", "sheet:")
                    .replace("xmlns:xdr", "xmlns:sheet")
                    .replace("a:", "draw:")
                    .replace("xmlns:a", "xmlns:draw")
                    .replace("r:", "edge:")
                    .replace("xmlns:r", "xmlns:edge");
                let root_end = xml.find('>').unwrap();
                let root = xml[root_end + 1..].find("<sheet:wsDr").unwrap() + root_end + 1;
                let root_end = root + xml[root..].find('>').unwrap();
                xml.insert_str(
                    root_end,
                    r#" xmlns:xdr="urn:foreign-xdr" xmlns:a="urn:foreign-a""#,
                );
                xml = xml.replace("</sheet:cNvPr>", r#"<draw:extLst><draw:ext uri="urn:scope"><future:payload xmlns:future="urn:future" kind="xdr:Payload" other="a:Marker"/></draw:ext></draw:extLst></sheet:cNvPr>"#);
                xml = xml.replace("<sheet:cNvGraphicFramePr/>", r#"<sheet:cNvGraphicFramePr><draw:graphicFrameLocks noDrilldown="true"><draw:extLst><draw:ext uri="urn:lock"><future:payload xmlns:future="urn:future" xmlns:edge="http://schemas.openxmlformats.org/officeDocument/2006/relationships" edge:id="sound-old" kind="xdr:Lock"/></draw:ext></draw:extLst></draw:graphicFrameLocks><draw:extLst><draw:ext uri="urn:owner"><future:payload xmlns:future="urn:future" kind="a:Owner"/></draw:ext></draw:extLst></sheet:cNvGraphicFramePr>"#);
                if default_namespace {
                    xml = xml.replace("sheet:", "").replace("xmlns:sheet=", "xmlns=");
                }
                xml
            },
        );
        for eager in [false, true] {
            let mut book = reader::xlsx::read_reader(Cursor::new(&source), eager).unwrap();
            book.try_read_sheet(0).unwrap();
            assert_eq!(book.sheet(0).unwrap().chart_collection().len(), 1);
            let output = write(&book);
            let metadata = xml(&output, "xl/drawings/drawing1.xml");
            assert!(metadata.contains(r#"kind="xdr:Payload""#));
            assert!(metadata.contains(r#"other="a:Marker""#));
            assert!(metadata.contains(r#"xmlns:xdr="urn:foreign-xdr""#));
            assert!(metadata.contains(r#"xmlns:a="urn:foreign-a""#));
            assert!(metadata.contains("noDrilldown="));
            assert!(metadata.contains(r#"kind="xdr:Lock""#));
            assert!(metadata.contains(r#"kind="a:Owner""#));
            let reread = reader::xlsx::read_reader(Cursor::new(&output), true).unwrap();
            assert_eq!(reread.sheet(0).unwrap().chart_collection().len(), 1);
            assert_dependencies(&write(&reread));
        }
        let rebound = replace_part(&source, "xl/drawings/drawing1.xml", |xml| {
            if default_namespace {
                xml.replace("<cNvPr ", r#"<cNvPr xmlns="urn:foreign-xdr" "#)
            } else {
                xml.replace("sheet:cNvPr", "xdr:cNvPr")
            }
        });
        assert!(reader::xlsx::read_reader(Cursor::new(rebound), true).is_err());
    }
}

#[test]
fn grouped_ole_chart_frames_keep_leaf_parts_and_dependencies() {
    use umya_spreadsheet::drawing::spreadsheet::{
        DrawingObject,
        GroupShape,
        TwoCellAnchor,
    };

    for native in [false, true] {
        let mut book =
            reader::xlsx::read_reader(Cursor::new(package(native, false, true)), true).unwrap();
        let drawing = book.sheet_mut(0).unwrap().worksheet_drawing_mut();
        let content = if native {
            let mut chart = drawing.extended_charts_mut().remove(0);
            chart.chart_space_mut().chart_mut().unwrap().series_mut()[0]
                .set_layout(ChartExSeriesLayout::Sunburst);
            DrawingObject::ExtendedFrame(Box::new(chart.frame().clone()))
        } else {
            let chart = drawing.chart_collection_mut().remove(0);
            DrawingObject::GraphicFrame(Box::new(
                chart.two_cell_anchor().graphic_frame().unwrap().clone(),
            ))
        };
        drawing.set_object_order(Vec::new());
        let mut group = GroupShape::default();
        group
            .non_visual_group_shape_properties_mut()
            .non_visual_drawing_properties_mut()
            .set_id(9001)
            .set_name("OLE group");
        group.add_object(content);
        let mut anchor = TwoCellAnchor::default();
        anchor.set_group_shape(group).set_is_alternate_content(true);
        let mut object = umya_spreadsheet::OleObject::default();
        object.set_shape_id(4501);
        object.set_object_extension("bin");
        object.set_object_data([42]);
        object
            .embedded_object_properties_mut()
            .image_mut()
            .set_image_name("OLE.png")
            .set_image_data(OLE_PREVIEW);
        object.set_two_cell_anchor(anchor);
        book.sheet_mut(0)
            .unwrap()
            .ole_objects_mut()
            .set_ole_object(object);
        let written = write(&book);
        assert_dependencies(&written);
        assert_eq!(part(&written, "xl/media/beep.wav"), SOUND);
        for eager in [false, true] {
            let mut reread = reader::xlsx::read_reader(Cursor::new(&written), eager).unwrap();
            reread.try_read_sheet(0).unwrap();
            let object = &reread.sheet(0).unwrap().ole_objects().ole_object()[0];
            assert_eq!(object.shape_id(), Some(4501));
            assert_eq!(object.object_data().unwrap(), [42]);
            let anchor = object.two_cell_anchor();
            let mut keys = Vec::new();
            anchor.visit_standard_frames(&mut |key, _| keys.push(key.relationship_key()));
            anchor.visit_extended_frames(&mut |key, frame| {
                keys.push(key.relationship_key());
                assert_eq!(
                    frame.chart_space().chart().unwrap().series()[0].layout(),
                    ChartExSeriesLayout::Sunburst
                );
            });
            assert_eq!(keys, ["1"]);
            let saved = write(&reread);
            assert_dependencies(&saved);
            assert_eq!(part(&saved, "xl/media/beep.wav"), SOUND);
            let xml = xml(&saved, "xl/drawings/drawing1.xml");
            assert!(xml.contains("OLE group"));
            assert!(xml.contains("hlinkClick"));
            assert!(xml.contains("_x0000_s4501"));
        }
    }
}

#[test]
fn native_chart_edits_and_dependencies_survive_ordinary_eager_and_lazy_routes() {
    for eager in [true, false] {
        let mut book =
            reader::xlsx::read_reader(Cursor::new(package(true, false, false)), eager).unwrap();
        book.read_sheet(0);
        assert!(book.sheet(0).unwrap().chart_collection().is_empty());
        let chart = &mut book
            .sheet_mut(0)
            .unwrap()
            .worksheet_drawing_mut()
            .extended_charts_mut()[0];
        chart.chart_space_mut().chart_mut().unwrap().series_mut()[0]
            .set_layout(ChartExSeriesLayout::Sunburst);
        let bytes = write(&book);
        assert_dependencies(&bytes);
        assert!(xml(&bytes, "[Content_Types].xml").contains(NATIVE_TYPE));
        let reread = reader::xlsx::read_reader(Cursor::new(&bytes), true).unwrap();
        assert_eq!(
            reread
                .sheet(0)
                .unwrap()
                .worksheet_drawing()
                .extended_charts()[0]
                .chart_space()
                .chart()
                .unwrap()
                .series()[0]
                .layout(),
            ChartExSeriesLayout::Sunburst
        );
    }
}

#[test]
fn standard_chart_source_dependencies_survive_materialization() {
    let mut book =
        reader::xlsx::read_reader(Cursor::new(package(false, false, false)), false).unwrap();
    book.read_sheet(0);
    assert_eq!(book.sheet(0).unwrap().chart_collection().len(), 1);
    assert_dependencies(&write(&book));
}

#[test]
fn a_new_standard_chart_does_not_take_an_imported_extended_part_name() {
    let mut book =
        reader::xlsx::read_reader(Cursor::new(package(true, false, false)), true).unwrap();
    book.sheet_mut(0).unwrap().add_chart(fixture::base_chart());
    let bytes = write(&book);
    assert_dependencies(&bytes);
    assert!(xml(&bytes, "xl/charts/chart1.xml").contains("cx:chartSpace"));
    assert!(xml(&bytes, "xl/charts/chart2.xml").contains("c:chartSpace"));
    let reread = reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap();
    let drawing = reread.sheet(0).unwrap().worksheet_drawing();
    assert_eq!(drawing.chart_collection().len(), 1);
    assert_eq!(drawing.extended_charts().len(), 1);
}

#[cfg(feature = "serde")]
#[test]
fn worksheet_json_and_bincode_keep_owned_dependencies() {
    let book = reader::xlsx::read_reader(Cursor::new(package(true, false, false)), true).unwrap();
    let sheet = book.sheet(0).unwrap();
    let json: Worksheet = serde_json::from_value(serde_json::to_value(sheet).unwrap()).unwrap();
    let binary: Worksheet = bincode::deserialize(&bincode::serialize(sheet).unwrap()).unwrap();
    for sheet in [json, binary] {
        let mut restored = umya_spreadsheet::new_file();
        *restored.sheet_mut(0).unwrap() = sheet;
        assert_dependencies(&write(&restored));
    }
}

#[test]
fn missing_dependency_does_not_commit_a_partial_lazy_sheet() {
    let mut book =
        reader::xlsx::read_reader(Cursor::new(package(true, true, false)), false).unwrap();
    for _ in 0..2 {
        let error = book.sheet_mut(0).unwrap_err();
        assert!(
            error.to_string().contains("missing drawing dependency"),
            "{error}"
        );
        assert!(book.sheet(0).is_err());
    }
}

#[test]
fn drawing_hyperlinks_and_sounds_rebind_away_from_generated_chart_ids() {
    let book = reader::xlsx::read_reader(Cursor::new(package(true, false, true)), true).unwrap();
    let bytes = write(&book);
    assert_dependencies(&bytes);
    assert_eq!(part(&bytes, "xl/media/beep.wav"), SOUND);
    let relationships = xml(&bytes, "xl/drawings/_rels/drawing1.xml.rels");
    assert!(relationships.contains("https://example.com/?a=1&amp;b=2"));
    assert!(relationships.contains("../media/beep.wav"));
    let reread = reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap();
    let properties = reread
        .sheet(0)
        .unwrap()
        .worksheet_drawing()
        .extended_charts()[0]
        .frame_properties()
        .non_visual_drawing_properties();
    let hyperlink = properties.hyperlink_click().unwrap();
    assert!(
        hyperlink
            .relationship_id
            .as_ref()
            .unwrap()
            .starts_with("rId")
    );
    assert!(
        hyperlink
            .sound
            .as_ref()
            .unwrap()
            .relationship_id
            .starts_with("rId")
    );
    assert_ne!(
        hyperlink.relationship_id.as_ref().unwrap(),
        &hyperlink.sound.as_ref().unwrap().relationship_id
    );
}

fn replace_part(bytes: &[u8], target: &str, replace: impl FnOnce(String) -> String) -> Vec<u8> {
    let replacement = replace(xml(bytes, target)).into_bytes();
    let mut input = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for index in 0..input.len() {
        let mut entry = input.by_index(index).unwrap();
        let name = entry.name().unwrap().to_string();
        let mut data = Vec::new();
        entry.read_to_end(&mut data).unwrap();
        add(
            &mut output,
            &name,
            if name == target { &replacement } else { &data },
        );
    }
    output.finish().unwrap().into_inner()
}

#[test]
fn malformed_standard_chart_references_return_errors() {
    let source = package(false, false, false);
    for replacement in [
        "",
        "r:id=\"missing\"",
        "r:id=\"rId1\" r:id=\"rId1\"",
        "unbound:id=\"rId1\"",
    ] {
        let bytes = replace_part(&source, "xl/drawings/drawing1.xml", |xml| {
            xml.replace("r:id=\"rId1\"", replacement)
        });
        assert!(
            reader::xlsx::read_reader(Cursor::new(bytes), true).is_err(),
            "{replacement}"
        );
    }
}

#[test]
fn malformed_nonvisual_properties_return_errors() {
    let source = package(false, false, false);
    for replacement in [
        "<xdr:cNvPr id=\"not-an-integer\" name=\"Chart\"/>",
        "<xdr:cNvPr id=\"2\" name=\"Chart\"><a:hlinkClick history=\"maybe\"/></xdr:cNvPr>",
        "<xdr:cNvPr id=\"2\" name=\"Chart\"><a:extLst><a:ext/></a:extLst></xdr:cNvPr>",
    ] {
        let bytes = replace_part(&source, "xl/drawings/drawing1.xml", |mut xml| {
            let start = xml.find("<xdr:cNvPr").unwrap();
            let end = start + xml[start..].find("/>").unwrap() + 2;
            xml.replace_range(start..end, replacement);
            xml
        });
        assert!(
            reader::xlsx::read_reader(Cursor::new(bytes), true).is_err(),
            "{replacement}"
        );
    }
}

fn mixed_package() -> Vec<u8> {
    let mut book = umya_spreadsheet::new_file();
    let chart = fixture::base_chart();
    let mut ole = umya_spreadsheet::OleObject::default();
    ole.set_object_extension("bin");
    ole.set_object_data([0_u8]);
    ole.embedded_object_properties_mut()
        .image_mut()
        .set_image_name("OLE.png")
        .set_image_data(OLE_PREVIEW);
    ole.set_two_cell_anchor(chart.two_cell_anchor().clone());
    book.sheet_mut(0).unwrap().add_chart(chart);
    book.sheet_mut(0)
        .unwrap()
        .ole_objects_mut()
        .set_ole_object(ole);
    let bytes = replace_part(&write(&book), "xl/charts/chart1.xml", |_| native_xml());
    let bytes = replace_part(&bytes, "xl/drawings/_rels/drawing1.xml.rels", |xml| {
        xml.replace(
            &format!("{R}/chart"),
            "http://schemas.microsoft.com/office/2014/relationships/chartEx",
        )
    });
    let bytes = replace_part(&bytes, "xl/drawings/drawing1.xml", |xml| {
        let start = xml.find("<xdr:twoCellAnchor").unwrap();
        let end = start
            + xml[start..].find("</xdr:twoCellAnchor>").unwrap()
            + "</xdr:twoCellAnchor>".len();
        let native = xml[start..end].replace(C, CX);
        let absolute = r#"<xdr:absoluteAnchor><xdr:pos x="0" y="0"/><xdr:ext cx="10" cy="20"/><xdr:sp><xdr:nvSpPr><xdr:cNvPr id="99" name="absolute"/><xdr:cNvSpPr/></xdr:nvSpPr><xdr:spPr/></xdr:sp><xdr:clientData/></xdr:absoluteAnchor>"#;
        let ole = r#"<mc:AlternateContent xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><mc:Choice xmlns:a14="http://schemas.microsoft.com/office/drawing/2010/main" Requires="a14"><xdr:twoCellAnchor><xdr:from><xdr:col>0</xdr:col><xdr:colOff>0</xdr:colOff><xdr:row>0</xdr:row><xdr:rowOff>0</xdr:rowOff></xdr:from><xdr:to><xdr:col>1</xdr:col><xdr:colOff>0</xdr:colOff><xdr:row>1</xdr:row><xdr:rowOff>0</xdr:rowOff></xdr:to><xdr:sp><xdr:nvSpPr><xdr:cNvPr id="88" name="OLE"><a:extLst><a:ext uri="{63B3BB69-23CF-44E3-9099-C40C66FF867C}"><a14:compatExt spid="_x0000_s1025"/></a:ext></a:extLst></xdr:cNvPr><xdr:cNvSpPr/></xdr:nvSpPr><xdr:spPr/></xdr:sp><xdr:clientData/></xdr:twoCellAnchor></mc:Choice></mc:AlternateContent>"#;
        format!("{}{native}{ole}{absolute}</xdr:wsDr>", &xml[..start])
    });
    bytes
}

#[test]
fn mixed_ole_extended_and_absolute_owners_keep_their_own_dispatch() {
    let bytes = mixed_package();
    let drawing = xml(&bytes, "xl/drawings/drawing1.xml");
    let mut namespaces = quick_xml::NsReader::from_reader(drawing.as_bytes());
    loop {
        let (namespace, event) = namespaces.read_resolved_event().unwrap();
        assert!(!matches!(
            namespace,
            quick_xml::name::ResolveResult::Unknown(_)
        ));
        if matches!(event, quick_xml::events::Event::Eof) {
            break;
        }
    }
    let book = reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap();
    let sheet = book.sheet(0).unwrap();
    assert_eq!(sheet.worksheet_drawing().extended_charts().len(), 1);
    assert_eq!(sheet.worksheet_drawing().absolute_anchors().len(), 1);
    assert_eq!(sheet.ole_objects().ole_object().len(), 1);
    assert!(
        sheet.ole_objects().ole_object()[0]
            .two_cell_anchor()
            .is_alternate_content()
    );
    let reread = reader::xlsx::read_reader(Cursor::new(write(&book)), true).unwrap();
    assert_eq!(
        reread
            .sheet(0)
            .unwrap()
            .worksheet_drawing()
            .extended_charts()
            .len(),
        1
    );
    assert_eq!(
        reread
            .sheet(0)
            .unwrap()
            .worksheet_drawing()
            .absolute_anchors()
            .len(),
        1
    );
}

#[cfg(feature = "serde")]
#[test]
fn worksheet_dependencies_keep_default_content_types() {
    let bytes = replace_part(&package(true, false, false), "[Content_Types].xml", |xml| {
        xml.replace(r#"<Override PartName="/xl/charts/chartStyle1.xml" ContentType="application/vnd.example.chartstyle+xml"/>"#, "")
            .replace("application/xml", "application/vnd.example.chartstyle+xml")
    });
    let book = reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap();
    let sheet: Worksheet =
        serde_json::from_value(serde_json::to_value(book.sheet(0).unwrap()).unwrap()).unwrap();
    let mut restored = umya_spreadsheet::new_file();
    *restored.sheet_mut(0).unwrap() = sheet;
    assert_dependencies(&write(&restored));
}

#[test]
fn missing_dependency_content_type_does_not_commit_a_lazy_sheet() {
    let bytes = replace_part(&package(true, false, false), "[Content_Types].xml", |xml| {
        xml.replace(r#"<Override PartName="/xl/charts/chartStyle1.xml" ContentType="application/vnd.example.chartstyle+xml"/>"#, "")
            .replace(r#"<Default Extension="xml" ContentType="application/xml"/>"#, "")
    });
    let mut book = reader::xlsx::read_reader(Cursor::new(bytes), false).unwrap();
    for _ in 0..2 {
        let error = book.sheet_mut(0).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("missing content type for drawing dependency"),
            "{error}"
        );
        assert!(book.sheet(0).is_err());
    }
}

#[test]
fn ole_hyperlinks_share_drawing_relationship_rebinding() {
    let bytes = replace_part(&mixed_package(), "xl/drawings/drawing1.xml", |xml| {
        xml.replace(r#"<xdr:cNvPr id="88" name="OLE">"#, &format!(r#"<xdr:cNvPr id="88" name="OLE"><a:hlinkClick xmlns:r="{R}" r:id="ole-link" tooltip="OLE link"/>"#))
    });
    let bytes = replace_part(&bytes, "xl/drawings/_rels/drawing1.xml.rels", |xml| {
        xml.replace("</Relationships>", &format!(r#"<Relationship Id="ole-link" Type="{R}/hyperlink" Target="https://example.com/?ole=1&amp;chart=2" TargetMode="External"/></Relationships>"#))
    });
    let book = reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap();
    let bytes = write(&book);
    assert!(
        xml(&bytes, "xl/drawings/_rels/drawing1.xml.rels")
            .contains("https://example.com/?ole=1&amp;chart=2")
    );
    let reread = reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap();
    let properties = reread.sheet(0).unwrap().ole_objects().ole_object()[0]
        .two_cell_anchor()
        .shape()
        .unwrap()
        .non_visual_shape_properties()
        .non_visual_drawing_properties();
    assert_eq!(
        properties.hyperlink_click().unwrap().tooltip.as_deref(),
        Some("OLE link")
    );
    assert!(
        properties
            .hyperlink_click()
            .unwrap()
            .relationship_id
            .as_ref()
            .unwrap()
            .starts_with("rId")
    );
}

#[test]
fn malformed_shape_extensions_return_errors() {
    let bytes = replace_part(&mixed_package(), "xl/drawings/drawing1.xml", |xml| {
        xml.replace(
            "<xdr:spPr/>",
            "<xdr:spPr><a:extLst><a:ext/></a:extLst></xdr:spPr>",
        )
    });
    assert!(reader::xlsx::read_reader(Cursor::new(bytes), true).is_err());
}

#[test]
fn unsupported_and_ambiguous_ole_choices_return_errors() {
    for duplicate in [false, true] {
        let bytes = replace_part(&mixed_package(), "xl/drawings/drawing1.xml", |mut xml| {
            if duplicate {
                let start = xml.find("<mc:Choice").unwrap();
                let end = start + xml[start..].find("</mc:Choice>").unwrap() + "</mc:Choice>".len();
                let choice = xml[start..end].to_owned();
                xml.insert_str(end, &choice);
                xml
            } else {
                xml.replace("Requires=\"a14\"", "Requires=\"xdr\"")
            }
        });
        assert!(reader::xlsx::read_reader(Cursor::new(bytes), true).is_err());
    }
}

#[test]
fn nonvisual_scalars_accept_xml_whitespace() {
    let bytes = replace_part(
        &package(false, false, false),
        "xl/drawings/drawing1.xml",
        |mut xml| {
            let start = xml.find("<xdr:cNvPr").unwrap();
            let end = start + xml[start..].find("/>").unwrap() + 2;
            xml.replace_range(
                start..end,
                r#"<xdr:cNvPr id=" &#x9;7&#xA; " name="chart" hidden=" true "/>"#,
            );
            xml
        },
    );
    let book = reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap();
    let frame = book.sheet(0).unwrap().chart_collection()[0]
        .two_cell_anchor()
        .graphic_frame()
        .unwrap();
    assert_eq!(
        frame
            .non_visual_graphic_frame_properties()
            .non_visual_drawing_properties()
            .id(),
        7
    );
    assert!(
        frame
            .non_visual_graphic_frame_properties()
            .non_visual_drawing_properties()
            .hidden()
    );
}

#[test]
fn mixed_ole_and_typed_owners_keep_stacking_order() {
    let bytes = replace_part(&mixed_package(), "xl/drawings/drawing1.xml", |mut xml| {
        let start = xml.find("<mc:AlternateContent").unwrap();
        let end = start
            + xml[start..].find("</mc:AlternateContent>").unwrap()
            + "</mc:AlternateContent>".len();
        let ole = xml[start..end].to_owned();
        xml.replace_range(start..end, "");
        let first = xml.find("<xdr:twoCellAnchor").unwrap();
        xml.insert_str(first, &ole);
        xml
    });
    let book = reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap();
    let output = write(&book);
    let drawing = xml(&output, "xl/drawings/drawing1.xml");
    let ole = drawing.find("name=\"OLE\"").unwrap();
    let native = drawing.find("<cx:chart").unwrap();
    let absolute = drawing.find("name=\"absolute\"").unwrap();
    assert!(ole < native && native < absolute, "{drawing}");
}

#[test]
fn reordered_ole_chart_frames_keep_their_own_chart_parts() {
    let bytes = replace_part(&mixed_package(), "xl/drawings/drawing1.xml", |mut xml| {
        let frame_start = xml.find("<xdr:graphicFrame").unwrap();
        let frame_end = frame_start
            + xml[frame_start..].find("</xdr:graphicFrame>").unwrap()
            + "</xdr:graphicFrame>".len();
        let frame = xml[frame_start..frame_end]
            .replace(CX, C)
            .replace("r:id=\"rId1\"", "r:id=\"rId2\"")
            .replace("id=\"1\"", "id=\"88\"")
            .replace(r#"<xdr:cNvPr id="88" name=""/>"#, r#"<xdr:cNvPr id="88" name=""><a:extLst><a:ext uri="{63B3BB69-23CF-44E3-9099-C40C66FF867C}"><a14:compatExt xmlns:a14="http://schemas.microsoft.com/office/drawing/2010/main" spid="_x0000_s1025"/></a:ext></a:extLst></xdr:cNvPr>"#);
        let shape_start = xml.find("<xdr:sp>").unwrap();
        let shape_end =
            shape_start + xml[shape_start..].find("</xdr:sp>").unwrap() + "</xdr:sp>".len();
        xml.replace_range(shape_start..shape_end, &frame);
        let start = xml.find("<mc:AlternateContent").unwrap();
        let end = start
            + xml[start..].find("</mc:AlternateContent>").unwrap()
            + "</mc:AlternateContent>".len();
        let ole = xml[start..end].to_owned();
        xml.replace_range(start..end, "");
        let first = xml.find("<xdr:twoCellAnchor").unwrap();
        xml.insert_str(first, &ole);
        xml
    });
    let bytes = replace_part(&bytes, "xl/drawings/_rels/drawing1.xml.rels", |xml| {
        xml.replace(
            r#"Id="rId2" Type="http://schemas.microsoft.com/office/2014/relationships/chartEx""#,
            &format!(r#"Id="rId2" Type="{R}/chart""#),
        )
    });
    let book = reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap();
    let output = write(&book);
    let relationships = xml(&output, "xl/drawings/_rels/drawing1.xml.rels");
    assert!(
        relationships.contains(&format!(
            r#"Id="rId1" Type="{R}/chart" Target="../charts/chart2.xml""#
        )),
        "{relationships}"
    );
    let reread = reader::xlsx::read_reader(Cursor::new(output), true).unwrap();
    let sheet = reread.sheet(0).unwrap();
    assert!(
        sheet.ole_objects().ole_object()[0]
            .two_cell_anchor()
            .graphic_frame()
            .is_some()
    );
    assert_eq!(sheet.worksheet_drawing().extended_charts().len(), 1);
}

#[test]
fn nonempty_ole_fallbacks_fail_without_discarding_content() {
    let bytes = replace_part(&mixed_package(), "xl/drawings/drawing1.xml", |xml| {
        xml.replace(
            "</mc:AlternateContent>",
            "<mc:Fallback><xdr:absoluteAnchor/></mc:Fallback></mc:AlternateContent>",
        )
    });
    assert!(reader::xlsx::read_reader(Cursor::new(bytes), true).is_err());
}

const OLE_PREVIEW: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0,
    0, 0, 31, 21, 196, 137, 0, 0, 0, 11, 73, 68, 65, 84, 120, 156, 99, 248, 15, 4, 0, 9, 251, 3,
    253, 251, 94, 107, 43, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];

fn ole_identity_package() -> Vec<u8> {
    use umya_spreadsheet::drawing::spreadsheet::{
        Shape,
        TwoCellAnchor,
    };
    let mut book = umya_spreadsheet::new_file();
    book.sheet_mut(0).unwrap().add_chart(fixture::base_chart());
    for (id, name, data, width) in [(11, "First", 11, 11), (22, "Second", 22, 22)] {
        let mut shape = Shape::default();
        shape
            .non_visual_shape_properties_mut()
            .non_visual_drawing_properties_mut()
            .set_id(id)
            .set_name(name);
        let mut anchor = TwoCellAnchor::default();
        anchor.set_shape(shape).set_is_alternate_content(true);
        let mut object = umya_spreadsheet::OleObject::default();
        object.set_shape_id(if id == 11 { 4501 } else { 4502 });
        object
            .set_requires("x14")
            .set_prog_id(name)
            .set_two_cell_anchor(anchor)
            .set_object_data([data]);
        object.set_object_extension("bin");
        object
            .embedded_object_properties_mut()
            .image_mut()
            .set_image_name(format!("{name}.png"))
            .set_image_data(OLE_PREVIEW);
        object
            .shape_mut()
            .set_style(format!("position:absolute;width:{width}pt"));
        book.sheet_mut(0)
            .unwrap()
            .ole_objects_mut()
            .set_ole_object(object);
    }
    let bytes = write(&book);
    let drawing = xml(&bytes, "xl/drawings/drawing1.xml");
    assert!(drawing.contains(r#"id="11" name="First""#));
    assert!(drawing.contains(r#"id="22" name="Second""#));
    for identity in [4501, 4502] {
        assert!(drawing.contains(&format!(r#"spid="_x0000_s{identity}""#)));
    }
    bytes
}

fn reverse_worksheet_ole_objects(source: &[u8]) -> Vec<u8> {
    replace_part(source, "xl/worksheets/sheet1.xml", |mut xml| {
        let start = xml.find("<oleObjects>").unwrap() + "<oleObjects>".len();
        let middle = start
            + xml[start..].find("</mc:AlternateContent>").unwrap()
            + "</mc:AlternateContent>".len();
        let end = middle
            + xml[middle..].find("</mc:AlternateContent>").unwrap()
            + "</mc:AlternateContent>".len();
        let reordered = format!("{}{}", &xml[middle..end], &xml[start..middle]);
        xml.replace_range(start..end, &reordered);
        xml
    })
}

#[test]
fn ole_owners_bind_to_source_identity_instead_of_list_position() {
    let source = reverse_worksheet_ole_objects(&ole_identity_package());
    let book = reader::xlsx::read_reader(Cursor::new(source), true).unwrap();
    let objects = book.sheet(0).unwrap().ole_objects().ole_object();
    for (object, expected, payload, width) in [
        (&objects[0], "Second", 22, 22),
        (&objects[1], "First", 11, 11),
    ] {
        assert_eq!(object.prog_id(), expected);
        assert_eq!(object.object_data().unwrap(), [payload]);
        assert_eq!(
            object
                .two_cell_anchor()
                .shape()
                .unwrap()
                .non_visual_shape_properties()
                .non_visual_drawing_properties()
                .name(),
            expected
        );
        assert_eq!(
            object.shape().style(),
            format!("position:absolute;width:{width}pt")
        );
    }
    let output = write(&book);
    let reread = reader::xlsx::read_reader(Cursor::new(output), true).unwrap();
    for object in reread.sheet(0).unwrap().ole_objects().ole_object() {
        assert_eq!(
            object
                .two_cell_anchor()
                .shape()
                .unwrap()
                .non_visual_shape_properties()
                .non_visual_drawing_properties()
                .name(),
            object.prog_id()
        );
    }
}

#[test]
fn ole_source_shape_ids_survive_all_written_representations() {
    let book = reader::xlsx::read_reader(Cursor::new(ole_identity_package()), true).unwrap();
    let output = write(&book);
    let worksheet = xml(&output, "xl/worksheets/sheet1.xml");
    let drawing = xml(&output, "xl/drawings/drawing1.xml");
    let vml = xml(&output, "xl/drawings/vmlDrawing1.vml");
    for id in [4501, 4502] {
        assert_eq!(worksheet.matches(&format!(r#"shapeId="{id}""#)).count(), 2);
        assert_eq!(
            drawing.matches(&format!(r#"spid="_x0000_s{id}""#)).count(),
            1
        );
        assert_eq!(vml.matches(&format!(r#"id="_x0000_s{id}""#)).count(), 1);
    }
}

#[test]
fn duplicate_ole_source_shape_ids_are_rejected_atomically() {
    let bytes = replace_part(&ole_identity_package(), "xl/worksheets/sheet1.xml", |xml| {
        xml.replace(r#"shapeId="4502""#, r#"shapeId="4501""#)
    });
    assert!(reader::xlsx::read_reader(Cursor::new(bytes.clone()), true).is_err());
    let mut book = reader::xlsx::read_reader(Cursor::new(bytes), false).unwrap();
    for _ in 0..2 {
        assert!(book.sheet_mut(0).is_err());
        assert!(book.sheet(0).is_err());
    }
}

#[test]
fn unmatched_ole_drawing_identities_are_rejected_atomically() {
    for drawing in [true, false] {
        let bytes = replace_part(&ole_identity_package(), "xl/drawings/drawing1.xml", |xml| {
            if drawing {
                xml.replace("_x0000_s4502", "_x0000_s9999")
            } else {
                let start = xml.find("<mc:AlternateContent").unwrap();
                let end = start
                    + xml[start..].find("</mc:AlternateContent>").unwrap()
                    + "</mc:AlternateContent>".len();
                let mut result = xml;
                result.replace_range(start..end, "");
                result
            }
        });
        assert!(reader::xlsx::read_reader(Cursor::new(bytes.clone()), true).is_err());
        let mut book = reader::xlsx::read_reader(Cursor::new(bytes), false).unwrap();
        for _ in 0..2 {
            assert!(book.sheet_mut(0).is_err());
            assert!(book.sheet(0).is_err());
        }
    }
}

#[test]
fn ole_only_sheets_emit_their_drawing_owners() {
    use umya_spreadsheet::drawing::spreadsheet::{
        Shape,
        TwoCellAnchor,
    };
    let mut book = umya_spreadsheet::new_file();
    let mut shape = Shape::default();
    shape
        .non_visual_shape_properties_mut()
        .non_visual_drawing_properties_mut()
        .set_id(7)
        .set_name("Embedded");
    let mut anchor = TwoCellAnchor::default();
    anchor.set_shape(shape).set_is_alternate_content(true);
    let mut object = umya_spreadsheet::OleObject::default();
    object
        .set_requires("x14")
        .set_prog_id("Package")
        .set_object_data([1]);
    object.set_object_extension("bin");
    object
        .embedded_object_properties_mut()
        .image_mut()
        .set_image_name("Embedded.png")
        .set_image_data(OLE_PREVIEW);
    object.set_two_cell_anchor(anchor);
    book.sheet_mut(0)
        .unwrap()
        .ole_objects_mut()
        .set_ole_object(object);
    let bytes = write(&book);
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    assert!(
        archive.by_name("xl/drawings/drawing1.xml").is_ok(),
        "OLE drawing omitted"
    );
}

#[test]
fn malformed_spreadsheet_properties_fail_without_panics_and_roll_back() {
    for properties in [
        r#"<xdr:spPr><a:ln w="bad"/></xdr:spPr>"#,
        r#"<xdr:spPr><a:prstGeom/></xdr:spPr>"#,
        r#"<xdr:spPr><a:solidFill><a:schemeClr/></a:solidFill></xdr:spPr>"#,
        r#"<xdr:spPr><a:xfrm><a:off x="bad" y="0"/></a:xfrm></xdr:spPr>"#,
        r#"<xdr:spPr/><xdr:txBody><a:bodyPr rot="bad"/><a:lstStyle/><a:p/></xdr:txBody>"#,
        r#"<xdr:spPr/><xdr:style><a:lnRef idx="bad"/></xdr:style>"#,
    ] {
        let source = replace_part(&mixed_package(), "xl/drawings/drawing1.xml", |xml| {
            xml.replace("<xdr:spPr/>", properties)
        });
        let eager = std::panic::catch_unwind(|| {
            reader::xlsx::read_reader(Cursor::new(source.clone()), true)
        });
        assert!(
            eager.is_ok(),
            "malformed drawing property panicked: {properties}"
        );
        assert!(
            eager.unwrap().is_err(),
            "malformed drawing property accepted: {properties}"
        );
        let mut book = reader::xlsx::read_reader(Cursor::new(source), false).unwrap();
        for _ in 0..2 {
            assert!(
                book.sheet_mut(0).is_err(),
                "malformed lazy drawing accepted: {properties}"
            );
            assert!(book.sheet(0).is_err());
        }
    }
}

#[test]
fn self_closing_spreadsheet_properties_match_expanded_properties() {
    for expanded in [false, true] {
        let properties = if expanded {
            r#"<xdr:spPr><a:prstGeom prst="rect"></a:prstGeom><a:ln w="42"></a:ln><a:xfrm rot="12000"></a:xfrm><a:noFill></a:noFill><a:effectLst></a:effectLst></xdr:spPr><xdr:txBody><a:bodyPr rot="60000"></a:bodyPr><a:lstStyle></a:lstStyle><a:p></a:p></xdr:txBody>"#
        } else {
            r#"<xdr:spPr><a:prstGeom prst="rect"/><a:ln w="42"/><a:xfrm rot="12000"/><a:noFill/><a:effectLst/></xdr:spPr><xdr:txBody><a:bodyPr rot="60000"/><a:lstStyle/><a:p/></xdr:txBody>"#
        };
        let source = replace_part(&mixed_package(), "xl/drawings/drawing1.xml", |xml| {
            xml.replace("<xdr:spPr/>", properties)
        });
        let book = reader::xlsx::read_reader(Cursor::new(source), true).unwrap();
        let shape = book.sheet(0).unwrap().ole_objects().ole_object()[0]
            .two_cell_anchor()
            .shape()
            .unwrap();
        assert_eq!(shape.shape_properties().outline().unwrap().width(), 42);
        assert!(shape.shape_properties().no_fill().is_some());
        let output = write(&book);
        let drawing = xml(&output, "xl/drawings/drawing1.xml");
        assert!(drawing.contains(r#"rot="12000""#));
        assert!(drawing.contains(r#"rot="60000""#));
        assert!(reader::xlsx::read_reader(Cursor::new(output), true).is_ok());
    }
}

#[cfg(feature = "serde")]
#[test]
fn bitmap_fill_public_paths_keep_the_same_serialized_field_layout() {
    #[derive(serde::Serialize)]
    struct BlipFillFields<'a> {
        rotate_with_shape: umya_spreadsheet::BooleanValue,
        blip:              &'a umya_spreadsheet::drawing::Blip,
        source_rectangle:  Option<&'a umya_spreadsheet::drawing::SourceRectangle>,
        stretch:           &'a umya_spreadsheet::drawing::Stretch,
    }
    let mut common = umya_spreadsheet::drawing::BlipFill::default();
    common.set_rotate_with_shape(true);
    let mut rectangle = umya_spreadsheet::drawing::SourceRectangle::default();
    rectangle.set_l("-10%");
    common.set_source_rectangle(rectangle);
    let picture: umya_spreadsheet::drawing::spreadsheet::BlipFill = common.clone();
    let rotate: umya_spreadsheet::BooleanValue = serde_json::from_str(r#"{"value":true}"#).unwrap();
    let fields = BlipFillFields {
        rotate_with_shape: rotate,
        blip:              common.blip(),
        source_rectangle:  common.source_rectangle(),
        stretch:           common.stretch(),
    };
    assert_eq!(
        serde_json::to_value(&common).unwrap(),
        serde_json::to_value(&fields).unwrap()
    );
    assert_eq!(
        bincode::serialize(&common).unwrap(),
        bincode::serialize(&fields).unwrap()
    );
    assert_eq!(
        bincode::serialize(&common).unwrap(),
        bincode::serialize(&picture).unwrap()
    );
}

fn bitmap_extension_package(picture: bool) -> Vec<u8> {
    let source = mixed_package();
    let mut archive = zip::ZipArchive::new(Cursor::new(&source)).unwrap();
    let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).unwrap();
        let name = entry.name().unwrap().to_string();
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).unwrap();
        add(&mut output, &name, &bytes);
    }
    add(&mut output, "xl/media/crop.png", OLE_PREVIEW);
    let source = output.finish().unwrap().into_inner();
    let source = replace_part(&source, "xl/drawings/_rels/drawing1.xml.rels", |xml| {
        xml.replace("</Relationships>", &format!(r#"<Relationship Id="bitmap" Type="{R}/image" Target="../media/crop.png"/><Relationship Id="bitmap-aux" Type="{R}/hyperlink" Target="https://example.com/bitmap?x=1&amp;y=2" TargetMode="External"/></Relationships>"#))
    });
    replace_part(&source, "xl/drawings/drawing1.xml", |mut xml| {
        let fill = format!(
            r#"<a:blipFill><a:blip xmlns:r="{R}" r:embed="bitmap"><a:extLst><a:ext uri="{{28A0092B-C50C-407E-A947-70E740481C1C}}"><a14:useLocalDpi xmlns:a14="http://schemas.microsoft.com/office/drawing/2010/main" val="1"/></a:ext><a:ext uri="urn:bitmap-future"><q:payload xmlns:q="urn:bitmap-payload" xmlns:alias="urn:bitmap-kind" kind="alias:Image" r:embed="bitmap-aux"/></a:ext></a:extLst></a:blip><a:srcRect l="-25000" t="2.5%"/><a:stretch><a:fillRect l="-3.25%" b="12500"/></a:stretch></a:blipFill>"#
        );
        if picture {
            let start = xml.find("<xdr:sp>").unwrap();
            let end = start + xml[start..].find("</xdr:sp>").unwrap() + "</xdr:sp>".len();
            let properties_start = xml[start..end].find("<xdr:cNvPr").unwrap() + start;
            let properties_end = properties_start
                + xml[properties_start..end].find("</xdr:cNvPr>").unwrap()
                + "</xdr:cNvPr>".len();
            let properties = &xml[properties_start..properties_end];
            let fill = fill
                .replace("<a:blipFill>", "<xdr:blipFill>")
                .replace("</a:blipFill>", "</xdr:blipFill>");
            let replacement = format!(
                "<xdr:pic><xdr:nvPicPr>{properties}<xdr:cNvPicPr/></xdr:nvPicPr>{fill}<xdr:spPr/\
                 ></xdr:pic>"
            );
            xml.replace_range(start..end, &replacement);
            xml
        } else {
            xml.replacen("<xdr:spPr/>", &format!("<xdr:spPr>{fill}</xdr:spPr>"), 1)
        }
    })
}

#[test]
fn imported_bitmap_extensions_keep_namespaces_dependencies_and_native_offsets() {
    for picture in [false, true] {
        let mut book =
            reader::xlsx::read_reader(Cursor::new(bitmap_extension_package(picture)), true)
                .unwrap();
        let anchor = book
            .sheet_mut(0)
            .unwrap()
            .ole_objects_mut()
            .ole_object_mut()[0]
            .two_cell_anchor_mut();
        let fill = if picture {
            anchor.picture_mut().unwrap().blip_fill_mut()
        } else {
            anchor
                .shape_mut()
                .unwrap()
                .shape_properties_mut()
                .blip_fill_mut()
                .unwrap()
        };
        assert_eq!(
            fill.stretch().fill_rectangle().unwrap().left(),
            Some("-3.25%")
        );
        fill.stretch_mut()
            .fill_rectangle_mut()
            .unwrap()
            .set_left("-8.75%");
        let output = write(&book);
        assert_eq!(part(&output, "xl/media/crop.png"), OLE_PREVIEW);
        let drawing = xml(&output, "xl/drawings/drawing1.xml");
        assert!(drawing.contains(r#"val="1""#));
        assert!(drawing.contains("urn:bitmap-future"));
        assert!(drawing.contains(r#"kind="alias:Image""#));
        assert!(drawing.contains(r#"xmlns:alias="urn:bitmap-kind""#));
        assert!(drawing.contains(r#"l="-8.75%""#));
        assert!(drawing.contains(if picture {
            "<xdr:blipFill"
        } else {
            "<a:blipFill"
        }));
        assert!(!drawing.contains("bitmap-aux"));
        assert!(
            xml(&output, "xl/drawings/_rels/drawing1.xml.rels")
                .contains("https://example.com/bitmap?x=1&amp;y=2")
        );
        assert!(reader::xlsx::read_reader(Cursor::new(output), true).is_ok());
    }
}
