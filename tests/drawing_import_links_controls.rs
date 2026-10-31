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

const R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

fn write(book: &Workbook) -> Vec<u8> {
    let mut output = Vec::new();
    writer::xlsx::write_writer(book, &mut output).unwrap();
    output
}

fn rewrite(source: &[u8], replacements: &[(&str, String)]) -> Vec<u8> {
    let mut input = zip::ZipArchive::new(Cursor::new(source)).unwrap();
    let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for index in 0..input.len() {
        let mut entry = input.by_index(index).unwrap();
        let name = entry.name().unwrap().to_string();
        if replacements.iter().any(|(target, _)| *target == name) {
            continue;
        }
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).unwrap();
        output
            .start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        output.write_all(&bytes).unwrap();
    }
    for (name, content) in replacements {
        output
            .start_file(*name, zip::write::SimpleFileOptions::default())
            .unwrap();
        output.write_all(content.as_bytes()).unwrap();
    }
    output.finish().unwrap().into_inner()
}

fn part(source: &[u8], name: &str) -> String {
    let mut archive = zip::ZipArchive::new(Cursor::new(source)).unwrap();
    let mut value = String::new();
    archive
        .by_name(name)
        .unwrap()
        .read_to_string(&mut value)
        .unwrap();
    value
}

#[test]
fn external_hyperlink_targets_are_decoded_once() {
    for url in [
        "https://example.com/?a=1&b=2",
        "https://example.com/?q=&amp;",
    ] {
        let mut book = umya_spreadsheet::new_file();
        let mut link = umya_spreadsheet::Hyperlink::default();
        link.set_url(url);
        book.sheet_mut(0)
            .unwrap()
            .cell_mut("A1")
            .set_hyperlink(link);
        let mut book = reader::xlsx::read_reader(Cursor::new(write(&book)), true).unwrap();
        assert_eq!(
            book.sheet(0)
                .unwrap()
                .cell("A1")
                .unwrap()
                .hyperlink()
                .unwrap()
                .url(),
            url
        );
        book.sheet_mut(0)
            .unwrap()
            .cell_mut("B2")
            .set_value_number(42);
        let reread = reader::xlsx::read_reader(Cursor::new(write(&book)), true).unwrap();
        assert_eq!(
            reread
                .sheet(0)
                .unwrap()
                .cell("A1")
                .unwrap()
                .hyperlink()
                .unwrap()
                .url(),
            url
        );
    }
}

fn control_package(kind: &str) -> Vec<u8> {
    let mut book = umya_spreadsheet::new_file();
    book.sheet_mut(0)
        .unwrap()
        .cell_mut("A1")
        .set_value_number(42);
    let source = write(&book);
    let sheet = part(&source, "xl/worksheets/sheet1.xml").replace(
        "</worksheet>",
        &format!(r#"<legacyDrawing xmlns:r="{R}" r:id="control"/></worksheet>"#),
    );
    rewrite(
        &source,
        &[
            (
                "[Content_Types].xml",
                part(&source, "[Content_Types].xml").replace(
                    "</Types>",
                    r#"<Default Extension="vml" ContentType="application/vnd.openxmlformats-officedocument.vmlDrawing"/></Types>"#,
                ),
            ),
            ("xl/worksheets/sheet1.xml", sheet),
            (
                "xl/worksheets/_rels/sheet1.xml.rels",
                format!(
                    r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="control" Type="{R}/vmlDrawing" Target="../drawings/vmlDrawing1.vml"/></Relationships>"#
                ),
            ),
            (
                "xl/drawings/vmlDrawing1.vml",
                format!(
                    r#"<xml xmlns:v="urn:schemas-microsoft-com:vml" xmlns:x="urn:schemas-microsoft-com:office:excel"><v:shape id="_x0000_s1025"><x:ClientData ObjectType="{kind}"><x:Anchor>0, 0, 0, 0, 2, 0, 2, 0</x:Anchor><x:Checked>1</x:Checked></x:ClientData></v:shape></xml>"#
                ),
            ),
        ],
    )
}

#[test]
fn native_form_controls_do_not_require_ole_owners() {
    for kind in [
        "Checkbox", "Radio", "Scroll", "Spin", "Button", "List", "Drop", "Label", "Edit", "GBox",
        "Dialog", "Group", "Movie", "LineA", "RectA", "Rect", "Shape",
    ] {
        for eager in [true, false] {
            let mut book =
                reader::xlsx::read_reader(Cursor::new(control_package(kind)), eager).unwrap();
            let sheet = book.sheet_mut(0).unwrap();
            assert_eq!(sheet.cell("A1").unwrap().value(), "42");
            assert!(sheet.ole_objects().ole_object().is_empty());
            sheet.cell_mut("B2").set_value_number(1);
            let output = write(&book);
            let vml = part(&output, "xl/drawings/vmlDrawing1.vml");
            assert!(vml.contains(&format!(r#"ObjectType="{kind}""#)));
            assert!(vml.contains("<x:Checked>1</x:Checked>"));
            assert!(reader::xlsx::read_reader(Cursor::new(output), true).is_ok());
        }
    }
}

#[test]
fn native_vml_shapes_accept_optional_and_non_ole_identifiers() {
    let source = control_package("Checkbox");
    let original = part(&source, "xl/drawings/vmlDrawing1.vml");
    for identifier in ["", r#"id="checkbox-one""#] {
        let source = rewrite(
            &source,
            &[(
                "xl/drawings/vmlDrawing1.vml",
                original.replace(r#"id="_x0000_s1025""#, identifier),
            )],
        );
        let mut book = reader::xlsx::read_reader(Cursor::new(source), true).unwrap();
        book.sheet_mut(0)
            .unwrap()
            .cell_mut("B2")
            .set_value_number(1);
        let output = write(&book);
        let vml = part(&output, "xl/drawings/vmlDrawing1.vml");
        assert!(vml.contains(r#"ObjectType="Checkbox""#));
        if !identifier.is_empty() {
            assert!(vml.contains(identifier));
        }
    }
}

#[test]
fn one_cell_connectors_keep_identity_geometry_and_stack_order() {
    let source = write(&umya_spreadsheet::new_file());
    let sheet = part(&source, "xl/worksheets/sheet1.xml").replace(
        "</worksheet>",
        &format!(r#"<drawing xmlns:r="{R}" r:id="drawing"/></worksheet>"#),
    );
    let mut anchors = String::new();
    for id in [6, 7, 8] {
        let object = if id == 7 {
            r#"<xdr:cxnSp><xdr:nvCxnSpPr><xdr:cNvPr id="7" name="Connector"/><xdr:cNvCxnSpPr/></xdr:nvCxnSpPr><xdr:spPr><a:xfrm><a:off x="100" y="200"/><a:ext cx="300" cy="400"/></a:xfrm><a:prstGeom prst="line"><a:avLst/></a:prstGeom></xdr:spPr></xdr:cxnSp>"#.to_owned()
        } else {
            format!(
                r#"<xdr:sp><xdr:nvSpPr><xdr:cNvPr id="{id}" name="Shape{id}"/><xdr:cNvSpPr/></xdr:nvSpPr><xdr:spPr/></xdr:sp>"#
            )
        };
        anchors.push_str(&format!(r#"<xdr:oneCellAnchor><xdr:from><xdr:col>2</xdr:col><xdr:colOff>17</xdr:colOff><xdr:row>3</xdr:row><xdr:rowOff>29</xdr:rowOff></xdr:from><xdr:ext cx="5400" cy="7800"/>{object}<xdr:clientData/></xdr:oneCellAnchor>"#));
    }
    let source = rewrite(
        &source,
        &[
            (
                "[Content_Types].xml",
                part(&source, "[Content_Types].xml").replace(
                    "</Types>",
                    r#"<Override PartName="/xl/drawings/drawing1.xml" ContentType="application/vnd.openxmlformats-officedocument.drawing+xml"/></Types>"#,
                ),
            ),
            ("xl/worksheets/sheet1.xml", sheet),
            (
                "xl/worksheets/_rels/sheet1.xml.rels",
                format!(
                    r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="drawing" Type="{R}/drawing" Target="../drawings/drawing1.xml"/></Relationships>"#
                ),
            ),
            (
                "xl/drawings/drawing1.xml",
                format!(
                    r#"<xdr:wsDr xmlns:xdr="http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">{anchors}</xdr:wsDr>"#
                ),
            ),
        ],
    );
    let mut book = reader::xlsx::read_reader(Cursor::new(source), true).unwrap();
    book.sheet_mut(0)
        .unwrap()
        .cell_mut("A1")
        .set_value_number(42);
    let output = write(&book);
    let drawing = part(&output, "xl/drawings/drawing1.xml");
    assert!(
        !drawing.contains("<xdr:style>"),
        "absent connector style was fabricated: {drawing}"
    );
    assert!(drawing.find("Shape6").unwrap() < drawing.find("Connector").unwrap());
    assert!(drawing.find("Connector").unwrap() < drawing.find("Shape8").unwrap());
    for expected in [
        r#"id="7""#,
        "<xdr:col>2</xdr:col>",
        "<xdr:colOff>17</xdr:colOff>",
        "<xdr:row>3</xdr:row>",
        "<xdr:rowOff>29</xdr:rowOff>",
        r#"cx="5400" cy="7800""#,
        r#"x="100" y="200""#,
        r#"cx="300" cy="400""#,
        r#"prst="line""#,
    ] {
        assert!(drawing.contains(expected), "missing {expected}: {drawing}");
    }
    assert!(reader::xlsx::read_reader(Cursor::new(output), true).is_ok());
}
