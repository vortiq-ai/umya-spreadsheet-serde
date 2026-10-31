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
const XDR: &str = "http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing";
const R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const PNG: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0,
    0, 0, 31, 21, 196, 137, 0, 0, 0, 11, 73, 68, 65, 84, 120, 156, 99, 0, 1, 0, 0, 5, 0, 1, 13, 10,
    45, 180, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];

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

fn picture(effects: &str, fill: &str, tail: &str) -> String {
    format!(
        r#"<xdr:pic><xdr:nvPicPr><xdr:cNvPr id="7" name="Bitmap"/><xdr:cNvPicPr/></xdr:nvPicPr><xdr:blipFill><a:blip r:embed="image">{effects}</a:blip>{fill}</xdr:blipFill><xdr:spPr/>{tail}</xdr:pic>"#
    )
}

fn package(object: &str) -> Vec<u8> {
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
            data = format!(r#"<xdr:wsDr xmlns:xdr="{XDR}" xmlns:a="{A}" xmlns:r="{R}"><xdr:twoCellAnchor><xdr:from><xdr:col>0</xdr:col><xdr:colOff>0</xdr:colOff><xdr:row>0</xdr:row><xdr:rowOff>0</xdr:rowOff></xdr:from><xdr:to><xdr:col>2</xdr:col><xdr:colOff>0</xdr:colOff><xdr:row>2</xdr:row><xdr:rowOff>0</xdr:rowOff></xdr:to>{object}<xdr:clientData/></xdr:twoCellAnchor></xdr:wsDr>"#).into_bytes();
        } else if name == "xl/drawings/_rels/drawing1.xml.rels" {
            data = format!(r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="image" Type="{R}/image" Target="../media/image.png"/></Relationships>"#).into_bytes();
        } else if name == "[Content_Types].xml" {
            data = String::from_utf8(data)
                .unwrap()
                .replace(
                    "</Types>",
                    r#"<Default Extension="png" ContentType="image/png"/></Types>"#,
                )
                .into_bytes();
        }
        output
            .start_file(&name, zip::write::SimpleFileOptions::default())
            .unwrap();
        output.write_all(&data).unwrap();
    }
    output
        .start_file(
            "xl/media/image.png",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
    output.write_all(PNG).unwrap();
    output.finish().unwrap().into_inner()
}

fn roundtrip(source: &[u8], expected: &[&str], absent: Option<&str>) {
    for eager in [true, false] {
        let mut book = reader::xlsx::read_reader(Cursor::new(source), eager).unwrap();
        book.try_read_sheet(0).unwrap();
        book.sheet_mut(0)
            .unwrap()
            .cell_mut("A1")
            .set_value_number(42);
        for _ in 0..2 {
            let output = write(&book);
            let xml = String::from_utf8(part(&output, "xl/drawings/drawing1.xml")).unwrap();
            let mut offset = 0;
            for value in expected {
                offset += xml[offset..]
                    .find(value)
                    .unwrap_or_else(|| panic!("missing {value}: {xml}"))
                    + value.len();
            }
            if let Some(value) = absent {
                assert!(!xml.contains(value), "{xml}");
            }
            let mut archive = zip::ZipArchive::new(Cursor::new(&output)).unwrap();
            let images: Vec<_> = (0..archive.len())
                .filter_map(|i| {
                    let mut entry = archive.by_index(i).unwrap();
                    if !entry.name().unwrap().starts_with("xl/media/") {
                        return None;
                    }
                    let mut data = Vec::new();
                    entry.read_to_end(&mut data).unwrap();
                    Some(data)
                })
                .collect();
            assert_eq!(images, [PNG]);
            book = reader::xlsx::read_reader(Cursor::new(output), true).unwrap();
            assert_eq!(book.sheet(0).unwrap().cell("A1").unwrap().value(), "42");
        }
    }
}

#[test]
fn blip_effects_preserve_document_order_and_nested_payloads() {
    let effects = r#"<a:alphaBiLevel thresh="25000"/><a:alphaCeiling/><a:alphaFloor/><a:alphaInv/><a:alphaMod><a:cont/></a:alphaMod><a:alphaModFix amt="50000"/><a:alphaRepl a="75000"/><a:biLevel thresh="40000"/><a:blur rad="12700" grow="0"/><a:clrChange><a:clrFrom><a:srgbClr val="FF0000"/></a:clrFrom><a:clrTo><a:srgbClr val="00FF00"/></a:clrTo></a:clrChange><a:clrRepl><a:srgbClr val="123456"/></a:clrRepl><a:duotone><a:srgbClr val="000000"/><a:srgbClr val="FFFFFF"/></a:duotone><a:fillOverlay blend="mult"><a:solidFill><a:srgbClr val="ABCDEF"/></a:solidFill></a:fillOverlay><a:grayscl/><a:hsl hue="60000" sat="10000" lum="-10000"/><a:lum bright="10000" contrast="20000"/><a:tint hue="120000" amt="30000"/><a:alphaModFix amt="25000"/>"#;
    roundtrip(
        &package(&picture(effects, "<a:stretch/>", "")),
        &[
            "<a:alphaBiLevel",
            "<a:alphaCeiling",
            "<a:alphaFloor",
            "<a:alphaInv",
            "<a:alphaMod",
            "<a:cont",
            "<a:alphaModFix",
            "amt=\"50000\"",
            "<a:alphaRepl",
            "<a:biLevel",
            "<a:blur",
            "<a:clrChange",
            "<a:clrFrom",
            "FF0000",
            "<a:clrTo",
            "00FF00",
            "<a:clrRepl",
            "123456",
            "<a:duotone",
            "000000",
            "FFFFFF",
            "<a:fillOverlay",
            "ABCDEF",
            "<a:grayscl",
            "<a:hsl",
            "<a:lum",
            "<a:tint",
            "<a:alphaModFix",
            "amt=\"25000\"",
        ],
        None,
    );
}

#[test]
fn tiled_bitmap_fill_keeps_offsets_scales_flip_and_alignment() {
    let tile = r#"<a:tile tx="100" ty="200" sx="50000" sy="50000" flip="xy" algn="ctr"/>"#;
    roundtrip(
        &package(&picture("", tile, "")),
        &[
            "<a:tile",
            "tx=\"100\"",
            "ty=\"200\"",
            "sx=\"50000\"",
            "sy=\"50000\"",
            "flip=\"xy\"",
            "algn=\"ctr\"",
        ],
        Some("<a:stretch"),
    );
}

#[test]
fn picture_style_survives_roundtrip() {
    let style = r#"<xdr:style><a:lnRef idx="0"><a:schemeClr val="accent1"/></a:lnRef><a:fillRef idx="1"><a:schemeClr val="accent2"/></a:fillRef><a:effectRef idx="0"><a:schemeClr val="accent3"/></a:effectRef><a:fontRef idx="minor"><a:schemeClr val="lt1"/></a:fontRef></xdr:style>"#;
    roundtrip(
        &package(&picture("", "<a:stretch/>", style)),
        &["<xdr:style", "accent1", "accent2", "accent3", "minor"],
        None,
    );
}

#[test]
fn invalid_bitmap_children_and_conflicting_fill_modes_are_errors() {
    for (effects, fill) in [
        ("<a:unknownEffect/>", "<a:stretch/>"),
        ("<a:alphaModFix amt=\"bogus\"/>", "<a:stretch/>"),
        ("<a:alphaModFix amt=\"-1\"/>", "<a:stretch/>"),
        ("<a:alphaBiLevel/>", "<a:stretch/>"),
        ("<a:alphaRepl a=\"100001\"/>", "<a:stretch/>"),
        ("<a:alphaMod/>", "<a:stretch/>"),
        (
            "<a:alphaMod><a:cont><a:unknownEffect/></a:cont></a:alphaMod>",
            "<a:stretch/>",
        ),
        (
            "<a:clrRepl><a:srgbClr val=\"not-hex\"/></a:clrRepl>",
            "<a:stretch/>",
        ),
        (
            "<a:clrChange><a:clrTo/><a:clrFrom/></a:clrChange>",
            "<a:stretch/>",
        ),
        (
            "<a:duotone><a:srgbClr val=\"FFFFFF\"/></a:duotone>",
            "<a:stretch/>",
        ),
        (
            "<a:fillOverlay blend=\"unknown\"><a:noFill/></a:fillOverlay>",
            "<a:stretch/>",
        ),
        ("<a:extLst/><a:alphaModFix amt=\"50000\"/>", "<a:stretch/>"),
        ("<a:extLst/><a:extLst/>", "<a:stretch/>"),
        ("", "<a:tile/><a:stretch/>"),
        ("", "<a:stretch/><a:tile/>"),
        ("", "<a:tile tx=\"27273042316901\"/>"),
        ("", "<a:tile flip=\"diagonal\"/>"),
        ("", "<a:tile algn=\"middle\"/>"),
        ("", "<a:tile sx=\"2147483648\"/>"),
        ("", "<a:tile><a:stretch/></a:tile>"),
    ] {
        let source = package(&picture(effects, fill, ""));
        assert!(
            reader::xlsx::read_reader(Cursor::new(&source), true).is_err(),
            "{effects} {fill}"
        );
        let mut lazy = reader::xlsx::read_reader(Cursor::new(&source), false).unwrap();
        assert!(lazy.try_read_sheet(0).is_err(), "{effects} {fill}");
    }
}

#[test]
fn effect_percentage_notation_and_large_blur_radius_survive_roundtrip() {
    roundtrip(
        &package(&picture(
            "<a:alphaBiLevel thresh=\"50%\"/><a:alphaModFix amt=\"200%\"/><a:hsl \
             sat=\"-20.5%\"/><a:blur rad=\"27273042316900\" grow=\"true\"/>",
            "<a:stretch/>",
            "",
        )),
        &[
            "thresh=\"50%\"",
            "amt=\"200%\"",
            "sat=\"-20.5%\"",
            "rad=\"27273042316900\"",
            "grow=\"true\"",
        ],
        None,
    );
}

#[test]
fn image_relationships_inside_effects_are_rebound_and_preserved() {
    roundtrip(
        &package(&picture(
            "<a:fillOverlay blend=\"mult\"><a:blipFill><a:blip \
             r:embed=\"image\"/><a:tile/></a:blipFill></a:fillOverlay>",
            "<a:stretch/>",
            "",
        )),
        &["<a:fillOverlay", "<a:blipFill", "r:embed=\"rId", "<a:tile"],
        None,
    );
}

#[test]
fn absent_tile_attributes_and_signed_scales_are_preserved() {
    roundtrip(
        &package(&picture(
            "",
            "<a:tile tx=\"1mm\" ty=\"-2.5pt\" sx=\"50%\" sy=\"50.25%\"/>",
            "",
        )),
        &[
            "<a:tile",
            "tx=\"1mm\"",
            "ty=\"-2.5pt\"",
            "sx=\"50%\"",
            "sy=\"50.25%\"",
        ],
        Some("<a:stretch"),
    );
    roundtrip(
        &package(&picture("", "<a:tile/>", "")),
        &["<a:tile"],
        Some("<a:stretch"),
    );
    roundtrip(
        &package(&picture(
            "",
            "<a:tile tx=\"-27273042329600\" ty=\"27273042316900\" sx=\"-50000\" sy=\"0\"/>",
            "",
        )),
        &[
            "<a:tile",
            "tx=\"-27273042329600\"",
            "ty=\"27273042316900\"",
            "sx=\"-50000\"",
            "sy=\"0\"",
        ],
        Some("<a:stretch"),
    );
}

#[cfg(feature = "serde")]
#[test]
fn bitmap_models_accept_older_json_without_added_fields() {
    use umya_spreadsheet::drawing::{
        Blip,
        BlipFill,
        spreadsheet::Picture,
    };
    let mut blip = serde_json::to_value(Blip::default()).unwrap();
    blip.as_object_mut().unwrap().remove("effects");
    let blip: Blip = serde_json::from_value(blip).unwrap();
    assert!(blip.effects().is_empty());

    let mut fill = serde_json::to_value(BlipFill::default()).unwrap();
    fill.as_object_mut().unwrap().remove("tile");
    let fill: BlipFill = serde_json::from_value(fill).unwrap();
    assert!(fill.tile().is_none());

    let mut picture = serde_json::to_value(Picture::default()).unwrap();
    picture.as_object_mut().unwrap().remove("shape_style");
    let picture: Picture = serde_json::from_value(picture).unwrap();
    assert!(picture.shape_style().is_none());
}

#[cfg(feature = "serde")]
#[test]
fn bitmap_effects_and_tile_persist_through_worksheet_serde() {
    let source = package(&picture(
        "<a:alphaModFix amt=\"50000\"/>",
        "<a:tile tx=\"100\" sy=\"50000\"/>",
        "",
    ));
    let mut book = reader::xlsx::read_reader(Cursor::new(source), true).unwrap();
    let sheet: umya_spreadsheet::Worksheet =
        serde_json::from_slice(&serde_json::to_vec(book.sheet(0).unwrap()).unwrap()).unwrap();
    *book.sheet_mut(0).unwrap() = sheet;
    roundtrip(
        &write(&book),
        &[
            "<a:alphaModFix",
            "amt=\"50000\"",
            "<a:tile",
            "tx=\"100\"",
            "sy=\"50000\"",
        ],
        Some("<a:stretch"),
    );
}
