use std::io::{
    Cursor,
    Read,
    Write,
};

use umya_spreadsheet::{
    Color,
    reader,
    writer,
};
use zip::write::SimpleFileOptions;

fn workbook_with_accent(color: &str) -> Vec<u8> {
    let mut source = Vec::new();
    writer::xlsx::write_writer(&umya_spreadsheet::new_file(), &mut source).unwrap();
    let mut archive = zip::ZipArchive::new(Cursor::new(source)).unwrap();
    let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for index in 0..archive.len() {
        let mut part = archive.by_index(index).unwrap();
        let name = part.name().unwrap().to_string();
        if name == "xl/theme/theme1.xml" {
            let mut xml = String::new();
            part.read_to_string(&mut xml).unwrap();
            let original = r#"<a:accent1><a:srgbClr val="4472C4"/></a:accent1>"#;
            assert!(xml.contains(original));
            let xml = xml.replace(original, &format!("<a:accent1>{color}</a:accent1>"));
            output
                .start_file(name, SimpleFileOptions::default())
                .unwrap();
            output.write_all(xml.as_bytes()).unwrap();
        } else {
            output.raw_copy_file(part).unwrap();
        }
    }
    output.finish().unwrap().into_inner()
}

#[test]
#[ignore = "the DrawingML colour model is held at the fleet pin"]
fn theme_colors_apply_ordered_drawingml_transforms_before_and_after_round_trip() {
    for (color, expected) in [
        (
            r#"<a:sysClr val="windowText" lastClr="FF0000"><a:lumMod val="0"/></a:sysClr>"#,
            "000000",
        ),
        (
            r#"<a:sysClr val="windowText" lastClr="000000"><a:lumOff val="100000"/></a:sysClr>"#,
            "FFFFFF",
        ),
        (
            r#"<a:sysClr val="windowText" lastClr="FF0000"><a:lumMod val="0"/><a:lumOff val="100%"/></a:sysClr>"#,
            "FFFFFF",
        ),
        (
            r#"<a:sysClr val="windowText" lastClr="FF0000"><a:lumOff val="100%"/><a:lumMod val="0"/></a:sysClr>"#,
            "000000",
        ),
        (
            r#"<a:srgbClr val="FF0000"><a:lumMod val="0"/></a:srgbClr>"#,
            "000000",
        ),
        (
            r#"<a:srgbClr val="000000"><a:lumOff val="100000"/></a:srgbClr>"#,
            "FFFFFF",
        ),
        (
            r#"<a:srgbClr val="FF0000"><a:hueOff val="7200000"/></a:srgbClr>"#,
            "00FF00",
        ),
        (
            r#"<a:srgbClr val="FF0000"><a:satMod val="0"/></a:srgbClr>"#,
            "808080",
        ),
        (
            r#"<a:srgbClr val="4472C4"><a:lumMod val="60000"/><a:lumOff val="40000"/></a:srgbClr>"#,
            "8FAADC",
        ),
        (r#"<a:srgbClr val="FF0000"><a:comp/></a:srgbClr>"#, "00FFFF"),
        (r#"<a:srgbClr val="FF0000"><a:inv/></a:srgbClr>"#, "00FFFF"),
        (r#"<a:srgbClr val="FF0000"><a:gray/></a:srgbClr>"#, "383838"),
        (
            r#"<a:srgbClr val="FF0000"><a:tint val="50000"/></a:srgbClr>"#,
            "FFBCBC",
        ),
        (
            r#"<a:srgbClr val="FF0000"><a:shade val="50000"/></a:srgbClr>"#,
            "BC0000",
        ),
        (
            r#"<a:srgbClr val="FF0000"><a:lumMod val="0"/><a:lumOff val="50000"/></a:srgbClr>"#,
            "808080",
        ),
        (
            r#"<a:srgbClr val="FF0000"><a:alpha val="0"/><a:alphaOff val="50000"/><a:alphaMod val="50000"/></a:srgbClr>"#,
            "FF0000",
        ),
        (
            r#"<a:srgbClr val="FF0000"><a:hue val="10800000"/><a:hueMod val="50000"/></a:srgbClr>"#,
            "80FF00",
        ),
        (
            r#"<a:srgbClr val="FF0000"><a:sat val="0"/><a:satOff val="100000"/><a:lum val="50000"/></a:srgbClr>"#,
            "FF0000",
        ),
        (
            r#"<a:srgbClr val="FFFFFF"><a:red val="0"/><a:green val="0"/><a:blue val="0"/><a:redOff val="100000"/><a:greenOff val="100000"/><a:blueOff val="100000"/><a:redMod val="0"/><a:greenMod val="0"/><a:blueMod val="0"/></a:srgbClr>"#,
            "000000",
        ),
        (
            r#"<a:srgbClr val="808080"><a:gamma/></a:srgbClr>"#,
            "BCBCBC",
        ),
        (
            r#"<a:srgbClr val="808080"><a:invGamma/></a:srgbClr>"#,
            "373737",
        ),
    ] {
        let mut book =
            reader::xlsx::read_reader(Cursor::new(workbook_with_accent(color)), true).unwrap();
        let mut theme_color = Color::default();
        theme_color.set_theme_index(4);
        assert_eq!(
            theme_color.argb_with_theme(book.theme()).as_deref(),
            Some(expected),
            "{color}"
        );
        book.sheet_mut(0)
            .unwrap()
            .cell_mut("A1")
            .set_value("edited");
        let mut saved = Vec::new();
        writer::xlsx::write_writer(&book, &mut saved).unwrap();
        let restored = reader::xlsx::read_reader(Cursor::new(saved), true).unwrap();
        assert_eq!(
            theme_color.argb_with_theme(restored.theme()).as_deref(),
            Some(expected),
            "{color}"
        );
    }
}

#[test]
fn theme_system_color_without_last_color_remains_unresolved_with_transforms() {
    let book = reader::xlsx::read_reader(
        Cursor::new(workbook_with_accent(
            r#"<a:sysClr val="windowText"><a:lumOff val="100000"/></a:sysClr>"#,
        )),
        true,
    )
    .unwrap();
    let mut color = Color::default();
    color.set_theme_index(4);
    assert!(color.argb_with_theme(book.theme()).is_none());
}
