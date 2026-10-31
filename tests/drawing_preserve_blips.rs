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

const R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

fn write(book: &Workbook) -> Vec<u8> {
    let mut output = Vec::new();
    writer::xlsx::write_writer(book, &mut output).unwrap();
    output
}

fn package(blip: &str, grouped: bool) -> Vec<u8> {
    let mut book = umya_spreadsheet::new_file();
    book.sheet_mut(0).unwrap().add_chart(fixture::base_chart());
    let source = write(&book);
    let mut input = zip::ZipArchive::new(Cursor::new(source)).unwrap();
    let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for index in 0..input.len() {
        let mut entry = input.by_index(index).unwrap();
        let name = entry.name().unwrap().to_string();
        let mut data = Vec::new();
        entry.read_to_end(&mut data).unwrap();
        if name == "xl/drawings/drawing1.xml" {
            let picture = format!(
                r#"<xdr:pic><xdr:nvPicPr><xdr:cNvPr id="7" name="Linked image"/><xdr:cNvPicPr/></xdr:nvPicPr><xdr:blipFill>{blip}<a:stretch/></xdr:blipFill><xdr:spPr/></xdr:pic>"#
            );
            let object = if grouped {
                format!(
                    r#"<xdr:grpSp><xdr:nvGrpSpPr><xdr:cNvPr id="6" name="Group"/><xdr:cNvGrpSpPr/></xdr:nvGrpSpPr><xdr:grpSpPr/>{picture}</xdr:grpSp>"#
                )
            } else {
                picture
            };
            data = format!(r#"<xdr:wsDr xmlns:xdr="http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="{R}"><xdr:oneCellAnchor><xdr:from><xdr:col>2</xdr:col><xdr:colOff>17</xdr:colOff><xdr:row>3</xdr:row><xdr:rowOff>29</xdr:rowOff></xdr:from><xdr:ext cx="5400" cy="7800"/>{object}<xdr:clientData/></xdr:oneCellAnchor></xdr:wsDr>"#).into_bytes();
        } else if name == "xl/drawings/_rels/drawing1.xml.rels" {
            if !blip.contains("r:link=") {
                continue;
            }
            data = format!(r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="linked" Type="{R}/image" Target="https://example.com/image.png?a=1&amp;b=2" TargetMode="External"/></Relationships>"#).into_bytes();
        }
        output
            .start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        output.write_all(&data).unwrap();
    }
    output.finish().unwrap().into_inner()
}

fn part(bytes: &[u8], name: &str) -> String {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut text = String::new();
    archive
        .by_name(name)
        .unwrap()
        .read_to_string(&mut text)
        .unwrap();
    text
}

#[test]
fn linked_blips_keep_external_relationships_without_embedded_media() {
    roundtrip(
        r#"<a:blip r:link="linked" cstate="print"><a:alphaModFix amt="50000"/><a:grayscl/></a:blip>"#,
        true,
    );
}

#[test]
fn blips_without_relationships_keep_effects_without_inventing_media() {
    roundtrip(
        r#"<a:blip cstate="print"><a:alphaModFix amt="50000"/><a:grayscl/></a:blip>"#,
        false,
    );
}

fn roundtrip(blip: &str, linked: bool) {
    for (eager, grouped) in [(true, false), (false, false), (true, true), (false, true)] {
        let mut book =
            reader::xlsx::read_reader(Cursor::new(package(blip, grouped)), eager).unwrap();
        book.try_read_sheet(0).unwrap();
        book.sheet_mut(0)
            .unwrap()
            .cell_mut("A1")
            .set_value_number(42);
        for _ in 0..2 {
            let output = write(&book);
            let xml = part(&output, "xl/drawings/drawing1.xml");
            assert!(xml.contains(r#"id="7""#));
            assert!(xml.contains(r#"name="Linked image""#));
            assert!(xml.contains(r#"cx="5400" cy="7800""#));
            assert!(xml.contains(r#"cstate="print""#));
            assert!(xml.contains(r#"amt="50000""#));
            assert!(xml.find("a:alphaModFix").unwrap() < xml.find("a:grayscl").unwrap());
            assert!(!xml.contains("r:embed="));
            assert!(part(&output, "[Content_Types].xml").contains(
                r#"ContentType="application/vnd.openxmlformats-officedocument.drawing+xml""#
            ));
            if linked {
                assert!(xml.contains("r:link="));
                let relationships = part(&output, "xl/drawings/_rels/drawing1.xml.rels");
                assert!(relationships.contains(r#"TargetMode="External""#));
                assert!(relationships.contains("https://example.com/image.png?a=1&amp;b=2"));
                assert!(relationships.contains(&format!(r#"Type="{R}/image""#)));
            }
            let archive = zip::ZipArchive::new(Cursor::new(&output)).unwrap();
            assert!(
                !archive
                    .file_names()
                    .any(|name| name.unwrap().starts_with("xl/media/"))
            );
            book = reader::xlsx::read_reader(Cursor::new(output), true).unwrap();
            assert_eq!(book.sheet(0).unwrap().cell("A1").unwrap().value(), "42");
        }
    }
}

#[test]
fn linked_blips_report_missing_relationships_explicitly() {
    for eager in [true, false] {
        let source = package(r#"<a:blip r:link="missing"/>"#, false);
        if eager {
            assert!(reader::xlsx::read_reader(Cursor::new(source), true).is_err());
        } else {
            let mut book = reader::xlsx::read_reader(Cursor::new(source), false).unwrap();
            assert!(book.try_read_sheet(0).is_err());
        }
    }
}

#[cfg(feature = "serde")]
#[test]
fn older_blip_and_one_cell_anchor_json_load_without_optional_state() {
    let mut blip = serde_json::to_value(umya_spreadsheet::drawing::Blip::default()).unwrap();
    blip.as_object_mut().unwrap().remove("link");
    serde_json::from_value::<umya_spreadsheet::drawing::Blip>(blip).unwrap();
    let mut anchor =
        serde_json::to_value(umya_spreadsheet::drawing::spreadsheet::OneCellAnchor::default())
            .unwrap();
    anchor.as_object_mut().unwrap().remove("connection_shape");
    serde_json::from_value::<umya_spreadsheet::drawing::spreadsheet::OneCellAnchor>(anchor)
        .unwrap();
}
