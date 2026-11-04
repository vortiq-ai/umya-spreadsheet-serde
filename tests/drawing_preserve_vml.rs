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
fn native_controls_keep_optional_ids_anchors_state_and_order() {
    let kinds = [
        "Checkbox", "Radio", "Scroll", "Spin", "Button", "List", "Drop", "GBox", "Label", "Edit",
        "Dialog", "Group", "Movie", "LineA", "RectA", "Rect", "Shape",
    ];
    let source = control_package("Checkbox");
    let mut shapes = String::from(r#"<v:shapetype id="control-type" coordsize="21600,21600"/>"#);
    for (index, kind) in kinds.iter().enumerate() {
        let identifier = if index % 2 == 0 {
            String::new()
        } else {
            format!(r#" id="control-{index}""#)
        };
        shapes.push_str(&format!(r##"<v:shape{identifier} type="#control-type" style="position:absolute"><x:ClientData ObjectType="{kind}"><x:Anchor>{index}, 1, 2, 3, 4, 5, 6, 7</x:Anchor><x:Checked>1</x:Checked><x:Val>17</x:Val><x:FmlaLink>$C$4</x:FmlaLink></x:ClientData></v:shape>"##));
    }
    let vml = format!(
        r#"<xml xmlns:v="urn:schemas-microsoft-com:vml" xmlns:x="urn:schemas-microsoft-com:office:excel">{shapes}</xml>"#
    );
    let source = rewrite(&source, &[("xl/drawings/vmlDrawing1.vml", vml)]);
    for eager in [false, true] {
        let mut book = reader::xlsx::read_reader(Cursor::new(&source), eager).unwrap();
        book.sheet_mut(0)
            .unwrap()
            .cell_mut("B2")
            .set_value_number(43);
        #[cfg(feature = "serde")]
        {
            let saved = bincode::serialize(book.sheet(0).unwrap()).unwrap();
            *book.sheet_mut(0).unwrap() = bincode::deserialize(&saved).unwrap();
        }
        let output = write(&book);
        let emitted = part(&output, "xl/drawings/vmlDrawing1.vml");
        let mut previous = 0;
        for (index, kind) in kinds.iter().enumerate() {
            let position = emitted.find(&format!(r#"ObjectType="{kind}""#)).unwrap();
            assert!(position > previous);
            previous = position;
            assert!(emitted.contains(&format!(
                "<x:Anchor>{index}, 1, 2, 3, 4, 5, 6, 7</x:Anchor>"
            )));
            if index % 2 != 0 {
                assert!(emitted.contains(&format!(r#"id="control-{index}""#)));
            }
        }
        assert_eq!(
            emitted.matches("<x:Checked>1</x:Checked>").count(),
            kinds.len()
        );
        assert_eq!(emitted.matches("<x:Val>17</x:Val>").count(), kinds.len());
        assert_eq!(
            emitted.matches("<x:FmlaLink>$C$4</x:FmlaLink>").count(),
            kinds.len()
        );
        assert!(emitted.find("v:shapetype").unwrap() < emitted.find("v:shape ").unwrap());
        assert!(part(&output, "xl/worksheets/_rels/sheet1.xml.rels").contains("/vmlDrawing"));
        assert!(
            part(&output, "[Content_Types].xml")
                .contains("application/vnd.openxmlformats-officedocument.vmlDrawing")
        );
        let reread = reader::xlsx::read_reader(Cursor::new(output), true).unwrap();
        assert_eq!(reread.sheet(0).unwrap().cell("B2").unwrap().value(), "43");
        assert!(part(&write(&reread), "xl/drawings/vmlDrawing1.vml").contains("<x:Val>17</x:Val>"));
    }
}

#[cfg(feature = "serde")]
#[test]
fn worksheets_without_native_vml_state_load_from_saved_json() {
    let book = umya_spreadsheet::new_file();
    let mut json = serde_json::to_value(book.sheet(0).unwrap()).unwrap();
    json.as_object_mut().unwrap().remove("native_vml_drawing");
    let _: umya_spreadsheet::Worksheet = serde_json::from_value(json).unwrap();
}

const PNG: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0,
    0, 0, 31, 21, 196, 137, 0, 0, 0, 11, 73, 68, 65, 84, 120, 156, 99, 0, 1, 0, 0, 5, 0, 1, 13, 10,
    45, 180, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];

#[test]
fn native_shape_relationships_keep_images_and_external_targets() {
    let source = control_package("Button");
    let xml = part(&source, "xl/drawings/vmlDrawing1.vml")
        .replace("<xml ", r#"<xml xmlns:o="urn:schemas-microsoft-com:office:office" "#)
        .replace("<x:ClientData", r#"<v:imagedata o:relid="native-image"/><v:fill o:relid="native-external"/><x:ClientData"#);
    let source = rewrite(
        &source,
        &[
            ("xl/drawings/vmlDrawing1.vml", xml),
            (
                "[Content_Types].xml",
                part(&source, "[Content_Types].xml").replace(
                    "</Types>",
                    r#"<Default Extension="png" ContentType="image/png"/></Types>"#,
                ),
            ),
            (
                "xl/drawings/_rels/vmlDrawing1.vml.rels",
                format!(
                    r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="native-image" Type="{R}/image" Target="../media/native-control.png"/><Relationship Id="native-external" Type="{R}/image" Target="https://example.com/control.png?a=1&amp;b=2" TargetMode="External"/></Relationships>"#
                ),
            ),
        ],
    );
    let mut zip = zip::ZipWriter::new_append(Cursor::new(source)).unwrap();
    zip.start_file(
        "xl/media/native-control.png",
        zip::write::SimpleFileOptions::default(),
    )
    .unwrap();
    zip.write_all(PNG).unwrap();
    let source = zip.finish().unwrap().into_inner();
    for eager in [false, true] {
        let mut book = reader::xlsx::read_reader(Cursor::new(&source), eager).unwrap();
        book.sheet_mut(0)
            .unwrap()
            .cell_mut("B2")
            .set_value_number(43);
        let output = write(&book);
        let vml = part(&output, "xl/drawings/vmlDrawing1.vml");
        assert!(vml.contains(r#"o:relid="native-image""#));
        assert!(vml.contains(r#"o:relid="native-external""#));
        let relationships = part(&output, "xl/drawings/_rels/vmlDrawing1.vml.rels");
        assert!(relationships.contains(r#"Id="native-image""#));
        assert!(relationships.contains(r#"Target="../media/native-control.png""#));
        assert!(relationships.contains(
            r#"Target="https://example.com/control.png?a=1&amp;b=2" TargetMode="External""#
        ));
        assert!(
            part(&output, "[Content_Types].xml")
                .contains(r#"Extension="png" ContentType="image/png""#)
        );
        let mut zip = zip::ZipArchive::new(Cursor::new(&output)).unwrap();
        let mut image = Vec::new();
        zip.by_name("xl/media/native-control.png")
            .unwrap()
            .read_to_end(&mut image)
            .unwrap();
        assert_eq!(image, PNG);
        assert!(reader::xlsx::read_reader(Cursor::new(output), true).is_ok());
    }
}

#[test]
fn textbox_html_with_unclosed_line_break_keeps_text_and_controls() {
    let vml = r##"<xml xmlns:v="urn:schemas-microsoft-com:vml" xmlns:x="urn:schemas-microsoft-com:office:excel"><v:shape id="_x0000_s1025" type="#_x0000_t201"><v:textbox style="mso-direction-alt:auto"><div style="text-align:center"><font face="Arial" size="220" color="#000000">Button<br>
      Two</font></div></v:textbox><x:ClientData ObjectType="Button"><x:Anchor>0, 0, 0, 0, 2, 0, 2, 0</x:Anchor><x:TextHAlign>Center</x:TextHAlign></x:ClientData></v:shape></xml>"##;
    let source = rewrite(
        &control_package("Button"),
        &[("xl/drawings/vmlDrawing1.vml", vml.to_string())],
    );
    for eager in [false, true] {
        let book = reader::xlsx::read_reader(Cursor::new(&source), eager).unwrap();
        let output = write(&book);
        let emitted = part(&output, "xl/drawings/vmlDrawing1.vml");
        assert!(emitted.contains(r##"<font face="Arial" size="220" color="#000000">Button"##));
        assert!(!emitted.contains("<br></br>"), "{emitted}");
        if eager {
            // A parsed and re-emitted textbox writes the void element empty.
            assert!(emitted.contains("Button<br/>"), "{emitted}");
            assert!(!emitted.contains("Button<br>"), "{emitted}");
        } else {
            // An unedited lazy copy keeps the bytes Excel wrote.
            assert!(
                emitted.contains(
                    "Button<br>
      Two"
                ),
                "{emitted}"
            );
            assert!(!emitted.contains("<br/>"), "{emitted}");
        }
        assert!(emitted.contains("Two</font></div>"));
        assert!(emitted.contains(r#"ObjectType="Button""#));
        assert!(emitted.contains("<x:TextHAlign>Center</x:TextHAlign>"));
        let reread = reader::xlsx::read_reader(Cursor::new(output), true).unwrap();
        let again = part(&write(&reread), "xl/drawings/vmlDrawing1.vml");
        assert!(again.contains("Two</font></div>"));
        assert!(!again.contains("<br></br>"), "{again}");
    }
}

#[test]
fn mismatched_end_tag_outside_html_void_elements_is_still_rejected() {
    let vml = r#"<xml xmlns:v="urn:schemas-microsoft-com:vml"><v:shape id="s"><v:textbox><div><b>x</div></v:textbox></v:shape></xml>"#;
    let source = rewrite(
        &control_package("Button"),
        &[("xl/drawings/vmlDrawing1.vml", vml.to_string())],
    );
    let message = reader::xlsx::read_reader(Cursor::new(&source), true)
        .expect_err("mismatched end tag must be rejected")
        .to_string();
    assert!(
        message.contains("</div>") && message.contains("</b>"),
        "{message}"
    );
}

#[test]
fn textbox_line_break_under_a_default_namespace_is_still_written_empty() {
    let vml = r##"<xml xmlns="urn:schemas-microsoft-com:vml" xmlns:x="urn:schemas-microsoft-com:office:excel"><shape id="_x0000_s1025" type="#_x0000_t201"><textbox><div>Button<br>
      Two</div></textbox><x:ClientData ObjectType="Button"><x:Anchor>0, 0, 0, 0, 2, 0, 2, 0</x:Anchor></x:ClientData></shape></xml>"##;
    let source = rewrite(
        &control_package("Button"),
        &[("xl/drawings/vmlDrawing1.vml", vml.to_string())],
    );
    let book = reader::xlsx::read_reader(Cursor::new(&source), true).unwrap();
    let emitted = part(&write(&book), "xl/drawings/vmlDrawing1.vml");
    assert!(emitted.contains("Button<br/>"), "{emitted}");
    assert!(!emitted.contains("<br></br>"), "{emitted}");
    assert!(emitted.contains("Two</div>"), "{emitted}");
}
