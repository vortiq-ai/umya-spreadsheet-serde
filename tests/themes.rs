use std::io::{
    Cursor,
    Read,
    Write,
};

use umya_spreadsheet::{
    reader,
    writer,
};
use zip::write::SimpleFileOptions;

const PART: &str = "xl/theme/theme1.xml";

fn source() -> Vec<u8> {
    let mut bytes = Vec::new();
    writer::xlsx::write_writer(&umya_spreadsheet::new_file(), &mut bytes).unwrap();
    bytes
}

fn theme_xml(source: &[u8]) -> String {
    let mut archive = zip::ZipArchive::new(Cursor::new(source)).unwrap();
    let mut xml = String::new();
    archive
        .by_name(PART)
        .unwrap()
        .read_to_string(&mut xml)
        .unwrap();
    xml
}

fn replace_theme(source: &[u8], xml: &str) -> Vec<u8> {
    let mut archive = zip::ZipArchive::new(Cursor::new(source)).unwrap();
    let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for index in 0..archive.len() {
        let part = archive.by_index(index).unwrap();
        let name = part.name().unwrap().to_string();
        if name == PART {
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
fn ordinary_theme_import_uses_namespace_bindings() {
    let source = source();
    let original = theme_xml(&source);
    let aliased = original
        .replace("xmlns:a=", "xmlns:d=")
        .replace("<a:", "<d:")
        .replace("</a:", "</d:");
    let defaulted = original
        .replace("xmlns:a=", "xmlns=")
        .replace("<a:", "<")
        .replace("</a:", "</");
    for xml in [aliased, defaulted] {
        let book =
            reader::xlsx::read_reader(Cursor::new(replace_theme(&source, &xml)), true).unwrap();
        assert_eq!(book.theme().name(), "Office Theme");
        assert_eq!(
            book.theme()
                .theme_elements()
                .color_scheme()
                .accent1()
                .val()
                .unwrap(),
            "4472C4"
        );
        assert_eq!(
            book.theme()
                .theme_elements()
                .font_scheme()
                .major_font()
                .latin_font()
                .typeface(),
            "Calibri Light"
        );
        assert_eq!(book.theme().source_xml(), Some(xml.as_str()));
        let mut saved = Vec::new();
        writer::xlsx::write_writer(&book, &mut saved).unwrap();
        assert_eq!(theme_xml(&saved), xml);
    }
}

#[test]
fn malformed_shared_theme_values_return_errors_without_panicking() {
    let source = source();
    let original = theme_xml(&source);
    let invalid = [
        original.replacen("val=\"44546A\"", "", 1),
        original.replacen("<a:latin ", "<a:latin pitchFamily=\"bad\" ", 1),
        original.replacen("w=\"6350\"", "w=\"bad\"", 1),
        original.replace("</a:theme>", ""),
    ];
    for xml in invalid {
        assert_ne!(
            xml, original,
            "authored invalid case did not change the source"
        );
        let bytes = replace_theme(&source, &xml);
        let parsed =
            std::panic::catch_unwind(|| reader::xlsx::read_reader(Cursor::new(bytes), true));
        assert!(parsed.is_ok(), "theme import panicked");
        assert!(parsed.unwrap().is_err(), "invalid Theme data was accepted");
    }
}

#[cfg(feature = "serde")]
#[test]
fn native_theme_color_choices_remain_native_without_fabricating_rgb_hex() {
    use umya_spreadsheet::{
        Color,
        drawing::{
            ColorTransform,
            Theme,
        },
    };
    let source = source();
    let original = theme_xml(&source);
    for color in [
        r#"<a:hslClr hue="60000" sat="50.25%" lum="25000"><a:hueOff val="120000"/></a:hslClr>"#,
        r#"<a:scrgbClr r="25%" g="50000" b="-5%"><a:hueOff val="120000"/></a:scrgbClr>"#,
        r#"<a:prstClr val="white"><a:hueOff val="120000"/></a:prstClr>"#,
    ] {
        let xml = original.replace(
            r#"<a:accent1><a:srgbClr val="4472C4"/></a:accent1>"#,
            &format!("<a:accent1>{color}</a:accent1>"),
        );
        assert_ne!(xml, original);
        let mut book =
            reader::xlsx::read_reader(Cursor::new(replace_theme(&source, &xml)), true).unwrap();
        let role = book.theme().theme_elements().color_scheme().accent1();
        assert_eq!(role.val(), None);
        assert_eq!(
            book.theme().theme_elements().color_scheme().color_map()[4],
            None
        );
        let mut cell_color = Color::default();
        cell_color.set_theme_index(4).set_tint(0.25);
        assert!(cell_color.argb_with_theme(book.theme()).is_none());
        assert!(cell_color.has_theme_index());
        assert_eq!(cell_color.theme_index(), 4);
        let role = book
            .theme_mut()
            .theme_elements_mut()
            .color_scheme_mut()
            .accent1_mut();
        let transforms = if color.contains("hslClr") {
            let hsl = role.hsl_color_mut().unwrap();
            hsl.set_hue("180000").unwrap();
            hsl.transforms_mut()
        } else if color.contains("scrgbClr") {
            let rgb = role.rgb_color_model_percentage_mut().unwrap();
            rgb.set_red("75.5%").unwrap();
            rgb.transforms_mut()
        } else {
            role.preset_color_mut().unwrap().transforms_mut()
        };
        transforms.push(ColorTransform::Gamma);
        let expected = book.theme().clone();
        let json: Theme = serde_json::from_slice(&serde_json::to_vec(&expected).unwrap()).unwrap();
        let binary: Theme = bincode::deserialize(&bincode::serialize(&expected).unwrap()).unwrap();
        let mut outputs = Vec::new();
        for restored in [expected, json, binary] {
            *book.theme_mut() = restored;
            let mut bytes = Vec::new();
            writer::xlsx::write_writer(&book, &mut bytes).unwrap();
            let saved = theme_xml(&bytes);
            assert!(saved.contains("<a:gamma/>"));
            assert!(saved.contains(if color.contains("hslClr") {
                r#"hue="180000""#
            } else if color.contains("scrgbClr") {
                r#"r="75.5%""#
            } else {
                r#"val="white""#
            }));
            let reloaded = reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap();
            assert_eq!(
                reloaded
                    .theme()
                    .theme_elements()
                    .color_scheme()
                    .accent1()
                    .val(),
                None
            );
            outputs.push(saved);
        }
        assert!(outputs.windows(2).all(|pair| pair[0] == pair[1]));
    }
}
