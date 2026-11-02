#![cfg(feature = "serde")]
//! Custom table, pivot table and slicer styles. The fixtures under
//! `tests/fixtures/table-styles/excel-authored` were authored by Excel:
//! duplicates of `TableStyleMedium2`, `PivotStyleMedium9` and
//! `SlicerStyleLight1` under new names, with a few elements formatted, applied
//! to a table, a pivot table and slicers, and set as the workbook defaults.
//! Every expected value
//! below is read off those files' XML or off what Excel reported through COM,
//! never off library output.
#[path = "support/slicer_package_fixture.rs"]
mod fixture;

use fixture::*;
use umya_spreadsheet::{
    Border,
    HorizontalAlignmentValues,
    SlicerStyleDefinition,
    SlicerStyleElementType,
    SlicerStyleItemDefinition,
    Style,
    TableStyleDefinition,
    TableStyleElementDefinition,
    TableStyleElementType,
    VerticalAlignmentValues,
    Workbook,
};

const DIRECTORY: &str = "tests/fixtures/table-styles/excel-authored";

fn sample(name: &str) -> Vec<u8> {
    std::fs::read(format!("{DIRECTORY}/{name}.xlsx")).unwrap()
}

fn styles_xml(bytes: &[u8]) -> String {
    unpack(bytes).remove("xl/styles.xml").unwrap()
}

fn with_styles_xml(bytes: &[u8], xml: String) -> Vec<u8> {
    let mut parts = unpack(bytes);
    parts.insert("xl/styles.xml".into(), xml);
    zip_parts(&parts)
}

fn zip_parts(parts: &std::collections::BTreeMap<String, String>) -> Vec<u8> {
    let mut rebuilt = std::io::Cursor::new(Vec::new());
    {
        use std::io::Write;
        let mut zip = zip::ZipWriter::new(&mut rebuilt);
        for (path, text) in parts {
            zip.start_file(path, zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(text.as_bytes()).unwrap();
        }
        zip.finish().unwrap();
    }
    rebuilt.into_inner()
}

fn package_with_ampersand_style_names() -> Vec<u8> {
    let mut parts = unpack(&sample("s1-custom"));
    for xml in parts.values_mut() {
        for (name, escaped) in [
            ("MyTable", "Table R&amp;D"),
            ("MyPivot", "Pivot R&amp;D"),
            ("MySlicer", "Slicer R&amp;D"),
        ] {
            *xml = xml.replace(name, escaped);
        }
    }
    let slicer = parts.get_mut("xl/slicers/slicer1.xml").unwrap();
    assert!(slicer.contains(r#"<slicer name="TblSl""#));
    *slicer = slicer.replace(
        r#"<slicer name="TblSl""#,
        r#"<slicer name="TblSl" style="Slicer R&amp;D""#,
    );
    zip_parts(&parts)
}

/// The text of the `index`-th `dxf` of the first `<dxfs` list of `xml`, or of
/// the `x14:dxfs` list.
fn dxf_text(xml: &str, list: &str, index: usize) -> String {
    let start = xml.find(&format!("<{list} ")).unwrap();
    let end = xml[start..].find(&format!("</{list}>")).unwrap() + start;
    xml[start..end]
        .split("<dxf>")
        .skip(1)
        .nth(index)
        .map(|rest| format!("<dxf>{}", &rest[..rest.find("</dxf>").unwrap() + 6]))
        .unwrap()
}

/// Create a solid fill using the public style API, as Excel's UI does.
fn color(rgb: &str) -> Style {
    let mut style = Style::default();
    style.set_background_color(rgb);
    style
}

fn fill_argb(style: &Style) -> String {
    // Excel-authored dxfs may use bgColor, while the solid-fill API writes
    // fgColor only.
    let fill = style.fill().and_then(|fill| fill.pattern_fill()).unwrap();
    fill.background_color()
        .or_else(|| fill.foreground_color())
        .unwrap()
        .argb_str()
}

fn red_bold() -> Style {
    let mut style = Style::default();
    style.font_mut().set_bold(true);
    style.font_mut().color_mut().set_argb_str("FFFF0000");
    style
}

fn save_light(book: &Workbook) -> Vec<u8> {
    let mut bytes = Vec::new();
    umya_spreadsheet::writer::xlsx::write_writer_light(book, &mut bytes).unwrap();
    bytes
}

fn table_element_format(
    book: &Workbook,
    style_name: &str,
    element_type: TableStyleElementType,
) -> Style {
    book.custom_table_styles()
        .into_iter()
        .find(|style| style.name == style_name)
        .unwrap()
        .elements
        .into_iter()
        .find(|element| element.element_type == element_type)
        .unwrap()
        .format
        .unwrap()
}

fn aligned_style() -> Style {
    let mut style = Style::default();
    let alignment = style.alignment_mut();
    alignment.set_horizontal(HorizontalAlignmentValues::Center);
    alignment.set_vertical(VerticalAlignmentValues::Center);
    alignment.set_wrap_text(true);
    alignment.set_text_rotation(45);
    alignment.set_indent(2);
    style
}

fn combined_format() -> Style {
    let mut style = red_bold();
    style.set_background_color("FF00FF00");
    style
        .borders_mut()
        .left_mut()
        .set_border_style(Border::BORDER_THIN);
    let alignment = style.alignment_mut();
    alignment.set_horizontal(HorizontalAlignmentValues::Center);
    alignment.set_vertical(VerticalAlignmentValues::Center);
    alignment.set_wrap_text(true);
    style
}

fn assert_custom_style_names(book: &mut Workbook) {
    assert_eq!(book.default_table_style(), "Table R&D");
    assert_eq!(book.default_pivot_table_style(), "Pivot R&D");
    assert_eq!(book.default_slicer_style(), "Slicer R&D");
    let table_names: Vec<_> = book
        .custom_table_styles()
        .into_iter()
        .map(|style| style.name)
        .collect();
    assert_eq!(table_names, ["Pivot R&D", "Table R&D"]);
    assert_eq!(book.custom_slicer_styles()[0].name, "Slicer R&D");
    book.try_read_sheet_by_name("Data").unwrap();
    let data = book.sheet_by_name("Data").unwrap();
    assert_eq!(data.tables()[0].style_info().unwrap().name(), "Table R&D");
    assert_eq!(data.slicers()[0].style.as_deref(), Some("Slicer R&D"));
    book.try_read_sheet_by_name("Report").unwrap();
    let report = book.sheet_by_name("Report").unwrap();
    assert_eq!(
        report.pivot_tables()[0]
            .pivot_table_definition()
            .pivot_table_style()
            .name(),
        "Pivot R&D"
    );
}

fn assert_encoded_style_names(xml: &str) {
    let table_styles = tags(xml, "<tableStyles")[0];
    assert_eq!(
        attribute(table_styles, "defaultTableStyle").as_deref(),
        Some("Table R&amp;D")
    );
    assert_eq!(
        attribute(table_styles, "defaultPivotStyle").as_deref(),
        Some("Pivot R&amp;D")
    );
    assert_eq!(
        attribute(tags(xml, "<x14:slicerStyles")[0], "defaultSlicerStyle").as_deref(),
        Some("Slicer R&amp;D")
    );
    let names: Vec<_> = tags(xml, "<tableStyle")
        .iter()
        .filter_map(|tag| attribute(tag, "name"))
        .collect();
    assert!(names.contains(&"Table R&amp;D".to_owned()));
    assert!(names.contains(&"Pivot R&amp;D".to_owned()));
    assert!(names.contains(&"Slicer R&amp;D".to_owned()));
    assert!(xml.contains(r#"<x14:slicerStyle name="Slicer R&amp;D""#));
    assert!(!xml.contains("&amp;amp;"), "attribute values encode once");
}

fn assert_dxf_children_are_in_schema_order(xml: &str) {
    let dxf = dxf_text(xml, "dxfs", 0);
    let positions = ["<font", "<fill", "<alignment", "<border"].map(|child| {
        dxf.find(child)
            .unwrap_or_else(|| panic!("missing {child}: {dxf}"))
    });
    assert!(
        positions.windows(2).all(|pair| pair[0] < pair[1]),
        "expected font, fill, alignment, border in {dxf}"
    );
}

const ALIGNMENT_XML: &str = r#"<alignment horizontal="center" vertical="center" wrapText="1" textRotation="45" indent="2"/>"#;

fn assert_alignment_xml(xml: &str) {
    let table_dxf = dxf_text(xml, "dxfs", 0);
    let slicer_dxf = dxf_text(xml, "x14:dxfs", 0);
    assert_eq!(tags(&table_dxf, "<alignment"), vec![ALIGNMENT_XML]);
    assert_eq!(tags(&slicer_dxf, "<alignment"), vec![ALIGNMENT_XML]);
}

fn paired_alignment_package(bytes: &[u8]) -> Vec<u8> {
    let xml = styles_xml(bytes);
    let paired_tag = ALIGNMENT_XML
        .strip_suffix("/>")
        .map(|start| format!("{start}></alignment>"))
        .unwrap();
    assert_eq!(xml.matches(ALIGNMENT_XML).count(), 2);
    with_styles_xml(bytes, xml.replace(ALIGNMENT_XML, &paired_tag))
}

fn assert_alignment_values(style: &Style) {
    let alignment = style.alignment().unwrap();
    assert_eq!(alignment.horizontal(), &HorizontalAlignmentValues::Center);
    assert_eq!(alignment.vertical(), &VerticalAlignmentValues::Center);
    assert!(alignment.wrap_text());
    assert_eq!(alignment.text_rotation(), 45);
    assert_eq!(alignment.indent(), 2);
}

fn assert_authored_alignment(book: &Workbook) {
    let table_format = table_element_format(book, "Aligned", TableStyleElementType::HeaderRow);
    assert_alignment_values(&table_format);

    let slicers = book.custom_slicer_styles();
    let slicer_format = slicers
        .iter()
        .find(|style| style.name == "AlignedSlicer")
        .unwrap()
        .items
        .iter()
        .find(|item| item.element_type == SlicerStyleElementType::SelectedItemWithData)
        .unwrap();
    assert_alignment_values(slicer_format.format.as_ref().unwrap());
}

#[test]
fn excel_custom_styles_round_trip_through_every_reader() {
    let source = sample("s1-custom");
    let source_xml = styles_xml(&source);
    for (reader_name, read) in readers() {
        let book = read(&source).unwrap();
        let output = save(&book);
        keep_output(&format!("roundtrip-s1-custom-{reader_name}"), &output);
        let xml = styles_xml(&output);

        // tableStyles: names, kinds, element order, counts and sizes. The
        // fixture's own counts are "12", "10" and "7": Excel counts the eight
        // slicer item elements of MySlicer next to its two table elements.
        let source_counts: Vec<_> = tags(&source_xml, "<tableStyle")
            .iter()
            .map(|t| attribute(t, "count").unwrap())
            .collect();
        assert_eq!(source_counts, ["12", "10", "7"]);
        let styles = tags(&xml, "<tableStyle");
        let names: Vec<_> = styles
            .iter()
            .map(|t| attribute(t, "name").unwrap())
            .collect();
        assert_eq!(names, ["MyPivot", "MySlicer", "MyTable"], "{reader_name}");
        let counts: Vec<_> = styles
            .iter()
            .map(|t| attribute(t, "count").unwrap())
            .collect();
        assert_eq!(counts, ["12", "10", "7"], "{reader_name}");
        assert_eq!(attribute(styles[0], "table").as_deref(), Some("0"));
        assert_eq!(attribute(styles[0], "pivot"), None);
        assert_eq!(attribute(styles[1], "pivot").as_deref(), Some("0"));
        assert_eq!(attribute(styles[1], "table").as_deref(), Some("0"));
        assert_eq!(attribute(styles[2], "pivot").as_deref(), Some("0"));
        assert_eq!(attribute(styles[2], "table"), None);
        let elements = tags(&xml, "<tableStyleElement");
        assert_eq!(elements.len(), 12 + 2 + 7);
        assert_eq!(
            elements[0],
            r#"<tableStyleElement type="wholeTable" dxfId="13"/>"#
        );
        let my_table_stripe = elements
            .iter()
            .find(|tag| {
                attribute(tag, "type").as_deref() == Some("firstRowStripe") && tag.contains("size")
            })
            .unwrap();
        assert_eq!(attribute(my_table_stripe, "size").as_deref(), Some("2"));
        let table_styles = tags(&xml, "<tableStyles")[0];
        assert_eq!(
            attribute(table_styles, "defaultTableStyle").as_deref(),
            Some("MyTable")
        );
        assert_eq!(
            attribute(table_styles, "defaultPivotStyle").as_deref(),
            Some("MyPivot")
        );

        // slicer styles extension: default, the eight item states, their own
        // dxf list
        let ext = tags(&xml, "<x14:slicerStyles")[0];
        assert_eq!(
            attribute(ext, "defaultSlicerStyle").as_deref(),
            Some("MySlicer")
        );
        assert_eq!(tags(&xml, "<x14:slicerStyleElement").len(), 8);
        assert_eq!(
            attribute(tags(&xml, "<x14:dxfs")[0], "count").as_deref(),
            Some("8")
        );
        let uris = ext_uris(&xml);
        let position_of = |uri: &str| uris.iter().position(|u| u == uri).unwrap();
        assert!(
            position_of("{46F421CA-312F-682f-3DD2-61675219B42D}")
                < position_of("{EB79DEF2-80B8-43e5-95BD-54CBDDF9020C}"),
            "x14:dxfs precedes x14:slicerStyles"
        );

        // every format an element points to reads back as Excel wrote it (the
        // writer spells empty border sides, which Excel treats as "no change")
        let source_book = read(&source).unwrap();
        let back = read(&output).unwrap();
        assert_eq!(
            back.custom_table_styles(),
            source_book.custom_table_styles(),
            "{reader_name}"
        );
        assert_eq!(
            back.custom_slicer_styles(),
            source_book.custom_slicer_styles(),
            "{reader_name}"
        );
        let source_whole_table = dxf_text(&source_xml, "dxfs", 20);
        let output_whole_table = dxf_text(&xml, "dxfs", 20);
        let horizontal_border = |dxf: &str| {
            let start = dxf.find("<horizontal").unwrap();
            let end = dxf[start..].find("</horizontal>").unwrap() + start + "</horizontal>".len();
            let horizontal = &dxf[start..end];
            assert_eq!(attribute(horizontal, "style").as_deref(), Some("thin"));
            let color = tags(horizontal, "<color")[0];
            (
                attribute(color, "theme").unwrap(),
                attribute(color, "tint").unwrap().parse::<f64>().unwrap(),
            )
        };
        // XML decimal formatting may use a shorter round-trip spelling; compare
        // the numeric tint from Excel's fixture, along with the border shape.
        assert_eq!(
            horizontal_border(&output_whole_table),
            horizontal_border(&source_whole_table),
            "the inner horizontal border of the whole-table format is kept"
        );

        // a second round trip changes nothing
        let again = read(&output).unwrap();
        assert_eq!(styles_xml(&save(&again)), xml, "{reader_name}");
    }
}

#[test]
fn ampersands_in_style_names_round_trip_and_remove_by_the_original_name() {
    let source = package_with_ampersand_style_names();
    let writers: [(&str, fn(&Workbook) -> Vec<u8>); 2] = [("full", save), ("light", save_light)];

    for (reader_name, read) in readers() {
        let mut book = read(&source).unwrap();
        assert_custom_style_names(&mut book);

        for (writer_name, write) in writers {
            let first = write(&book);
            keep_output(
                &format!("style-names-{reader_name}-{writer_name}-first"),
                &first,
            );
            let first_xml = styles_xml(&first);
            assert_encoded_style_names(&first_xml);
            let first_parts = unpack(&first);
            assert!(first_parts["xl/slicers/slicer1.xml"].contains(r#"style="Slicer R&amp;D""#));
            assert!(first_parts["xl/tables/table1.xml"].contains(r#"name="Table R&amp;D""#));
            assert!(
                first_parts["xl/pivotTables/pivotTable1.xml"].contains(r#"name="Pivot R&amp;D""#)
            );
            assert!(!first_parts["xl/tables/table1.xml"].contains("&amp;amp;"));
            assert!(!first_parts["xl/pivotTables/pivotTable1.xml"].contains("&amp;amp;"));

            let mut second_book = read(&first).unwrap();
            assert_custom_style_names(&mut second_book);
            let second = write(&second_book);
            keep_output(
                &format!("style-names-{reader_name}-{writer_name}-second"),
                &second,
            );
            assert_eq!(styles_xml(&second), first_xml);
            let second_parts = unpack(&second);
            assert!(second_parts["xl/slicers/slicer1.xml"].contains(r#"style="Slicer R&amp;D""#));
            assert!(second_parts["xl/tables/table1.xml"].contains(r#"name="Table R&amp;D""#));
            assert!(
                second_parts["xl/pivotTables/pivotTable1.xml"].contains(r#"name="Pivot R&amp;D""#)
            );

            let mut removed_book = read(&second).unwrap();
            assert!(removed_book.remove_custom_table_style("Table R&D").unwrap());
            assert!(removed_book.remove_custom_table_style("Pivot R&D").unwrap());
            assert_eq!(removed_book.default_table_style(), "TableStyleMedium2");
            assert_eq!(
                removed_book.default_pivot_table_style(),
                "PivotStyleLight16"
            );
            assert!(!removed_book.remove_custom_table_style("Table R&D").unwrap());
            assert_eq!(
                removed_book.sheet_by_name("Data").unwrap().tables()[0]
                    .style_info()
                    .unwrap()
                    .name(),
                "TableStyleMedium2"
            );
            assert_eq!(
                removed_book.sheet_by_name("Report").unwrap().pivot_tables()[0]
                    .pivot_table_definition()
                    .pivot_table_style()
                    .name(),
                "PivotStyleLight16"
            );
            assert!(
                removed_book
                    .remove_custom_slicer_style("Slicer R&D")
                    .unwrap()
            );
            assert!(
                !removed_book
                    .remove_custom_slicer_style("Slicer R&D")
                    .unwrap()
            );
            assert_eq!(removed_book.default_slicer_style(), "SlicerStyleLight1");
            assert!(removed_book.custom_slicer_styles().is_empty());
            assert!(
                removed_book
                    .sheet_by_name("Data")
                    .unwrap()
                    .slicers()
                    .iter()
                    .all(|slicer| slicer.style.is_none())
            );

            let removed = write(&removed_book);
            keep_output(
                &format!("style-names-{reader_name}-{writer_name}-removed"),
                &removed,
            );
            let removed_xml = styles_xml(&removed);
            assert!(!removed_xml.contains("Slicer R&amp;D"));
            assert!(!removed_xml.contains("Table R&amp;D"));
            assert!(!removed_xml.contains("Pivot R&amp;D"));
            assert_eq!(
                attribute(
                    tags(&removed_xml, "<x14:slicerStyles")[0],
                    "defaultSlicerStyle"
                )
                .as_deref(),
                Some("SlicerStyleLight1")
            );
            assert!(!unpack(&removed)["xl/slicers/slicer1.xml"].contains("style="));
            let mut reread = read(&removed).unwrap();
            reread.try_read_sheet_by_name("Data").unwrap();
            assert!(reread.custom_slicer_styles().is_empty());
            assert!(
                reread
                    .sheet_by_name("Data")
                    .unwrap()
                    .slicers()
                    .iter()
                    .all(|slicer| slicer.style.is_none())
            );
        }
    }
}

#[test]
fn table_styles_precede_mru_colors_in_both_writers() {
    let source = sample("s1-custom");
    let source_xml = styles_xml(&source);
    let with_colors = source_xml.replace(
        "</tableStyles>",
        "</tableStyles><colors><mruColors><color rgb=\"FFFF0000\"/></mruColors></colors>",
    );
    assert_ne!(with_colors, source_xml);
    let source = with_styles_xml(&source, with_colors);
    let writers: [(&str, fn(&Workbook) -> Vec<u8>); 2] = [("full", save), ("light", save_light)];

    for (reader_name, read) in readers() {
        let book = read(&source).unwrap();
        for (writer_name, write) in writers {
            let output = write(&book);
            keep_output(
                &format!("stylesheet-order-{reader_name}-{writer_name}"),
                &output,
            );
            let xml = styles_xml(&output);
            let table_styles = xml.find("<tableStyles ").unwrap();
            let colors = xml.find("<colors>").unwrap();
            let extensions = xml.find("<extLst>").unwrap();
            assert!(
                table_styles < colors && colors < extensions,
                "CT_Stylesheet order is tableStyles, colors, extLst: {xml}"
            );
            assert!(xml.contains(r#"<color rgb="FFFF0000"/>"#));
        }
    }
}

#[test]
fn differential_formats_write_all_children_in_schema_order_in_both_writers() {
    let mut book = umya_spreadsheet::new_file();
    book.set_custom_table_style(&TableStyleDefinition {
        name:              "Combined".into(),
        applies_to_tables: true,
        applies_to_pivots: false,
        elements:          vec![TableStyleElementDefinition {
            element_type: TableStyleElementType::HeaderRow,
            stripe_size:  1,
            format:       Some(combined_format()),
        }],
    })
    .unwrap();

    let writers: [(&str, fn(&Workbook) -> Vec<u8>); 2] = [("full", save), ("light", save_light)];
    for (writer_name, write) in writers {
        let output = write(&book);
        keep_output(&format!("combined-dxf-{writer_name}"), &output);
        let xml = styles_xml(&output);
        assert_dxf_children_are_in_schema_order(&xml);
        for (_reader_name, read) in readers() {
            assert_dxf_children_are_in_schema_order(&styles_xml(&write(&read(&output).unwrap())));
        }
    }
}

#[test]
fn excel_custom_styles_read_back_as_typed_definitions() {
    let book = read_eager(&sample("s1-custom")).unwrap();
    let styles = book.custom_table_styles();
    let names: Vec<_> = styles.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(
        names,
        ["MyPivot", "MyTable"],
        "the slicer style is not a table style"
    );
    let table = styles.iter().find(|s| s.name == "MyTable").unwrap();
    assert!(table.applies_to_tables && !table.applies_to_pivots);
    assert_eq!(table.elements.len(), 7);
    let element =
        |t: TableStyleElementType| table.elements.iter().find(|e| e.element_type == t).unwrap();
    // dxf 20: whole table fill 00FF00; dxf 19: header row bold red; dxf 15:
    // stripe 0000FF, size 2
    assert_eq!(
        fill_argb(
            element(TableStyleElementType::WholeTable)
                .format
                .as_ref()
                .unwrap()
        ),
        "FF00FF00"
    );
    let header = element(TableStyleElementType::HeaderRow)
        .format
        .as_ref()
        .unwrap();
    assert!(header.font().unwrap().bold());
    assert_eq!(header.font().unwrap().color().argb_str(), "FFFF0000");
    let stripe = element(TableStyleElementType::FirstRowStripe);
    assert_eq!(stripe.stripe_size, 2);
    assert_eq!(fill_argb(stripe.format.as_ref().unwrap()), "FF0000FF");
    let pivot = styles.iter().find(|s| s.name == "MyPivot").unwrap();
    assert!(pivot.applies_to_pivots && !pivot.applies_to_tables);
    assert_eq!(pivot.elements.len(), 12);

    let slicers = book.custom_slicer_styles();
    assert_eq!(slicers.len(), 1);
    assert_eq!(slicers[0].name, "MySlicer");
    assert_eq!(slicers[0].items.len(), 8);
    // x14 dxf 7 (unselectedItemWithData): bgColor FFFF0000 on an FFFFFFFF
    // foreground
    let unselected = slicers[0]
        .items
        .iter()
        .find(|i| i.element_type == SlicerStyleElementType::UnselectedItemWithData)
        .unwrap();
    assert_eq!(fill_argb(unselected.format.as_ref().unwrap()), "FFFF0000");
    assert!(slicers[0].whole_slicer.is_some() && slicers[0].header.is_some());

    assert_eq!(book.default_table_style(), "MyTable");
    assert_eq!(book.default_pivot_table_style(), "MyPivot");
    assert_eq!(book.default_slicer_style(), "MySlicer");
}

#[test]
fn paired_dxf_font_leaves_keep_their_explicit_values_through_package_round_trip() {
    let source = sample("s1-custom");
    let source_xml = styles_xml(&source);
    let original_dxf = dxf_text(&source_xml, "dxfs", 19);
    assert!(original_dxf.contains("<b/>"));
    assert!(original_dxf.contains(r#"<i val="0"/>"#));

    let paired_dxf = original_dxf
        .replace("<b/>", "<b></b>")
        .replace(r#"<i val="0"/>"#, r#"<i val="0"></i>"#);
    assert!(paired_dxf.contains("<b></b>"));
    assert!(paired_dxf.contains(r#"<i val="0"></i>"#));
    let paired_xml = source_xml.replace(&original_dxf, &paired_dxf);
    assert_ne!(paired_xml, source_xml);
    let paired_source = with_styles_xml(&source, paired_xml);

    for (shape, package) in [("empty", &source), ("paired", &paired_source)] {
        for (reader_name, read) in readers() {
            let book = read(package).unwrap();
            let header = table_element_format(&book, "MyTable", TableStyleElementType::HeaderRow);
            let font = header.font().unwrap();
            assert!(font.bold(), "{shape}, {reader_name}: bold defaults to on");
            assert!(
                !font.italic(),
                "{shape}, {reader_name}: explicit italic off"
            );
            assert_eq!(font.color().argb_str(), "FFFF0000");

            let output = save(&book);
            keep_output(&format!("font-{shape}-{reader_name}"), &output);
            let output_dxf = dxf_text(&styles_xml(&output), "dxfs", 19);
            assert_eq!(tags(&output_dxf, "<b"), vec!["<b/>"]);
            assert_eq!(tags(&output_dxf, "<i"), vec![r#"<i val="0"/>"#]);

            let reread = read(&output).unwrap();
            let reread_header =
                table_element_format(&reread, "MyTable", TableStyleElementType::HeaderRow);
            let reread_font = reread_header.font().unwrap();
            assert!(reread_font.bold(), "{shape}, {reader_name}: reread bold");
            assert!(
                !reread_font.italic(),
                "{shape}, {reader_name}: reread italic off"
            );
            assert_eq!(reread_font.color().argb_str(), "FFFF0000");
        }
    }
}

#[test]
fn empty_dxf_border_survives_package_round_trip() {
    let source = sample("s1-custom");
    let source_xml = styles_xml(&source);
    let original_dxf = dxf_text(&source_xml, "dxfs", 19);
    let border_dxf = "<dxf><border/></dxf>";
    let rebuilt = with_styles_xml(&source, source_xml.replace(&original_dxf, border_dxf));

    for (reader_name, read) in readers() {
        let book = read(&rebuilt).unwrap();
        let format = table_element_format(&book, "MyTable", TableStyleElementType::HeaderRow);
        assert!(
            format.borders().is_some(),
            "{reader_name}: empty border is present"
        );

        let output = save(&book);
        keep_output(&format!("empty-dxf-border-{reader_name}"), &output);
        let output_dxf = dxf_text(&styles_xml(&output), "dxfs", 19);
        assert_eq!(tags(&output_dxf, "<border").len(), 1);

        let reread = read(&output).unwrap();
        let reread_format =
            table_element_format(&reread, "MyTable", TableStyleElementType::HeaderRow);
        assert!(
            reread_format.borders().is_some(),
            "{reader_name}: reread border"
        );
    }
}

#[test]
fn api_authored_table_and_slicer_alignment_round_trip_through_both_writers() {
    let mut book = umya_spreadsheet::new_file();
    book.set_custom_table_style(&TableStyleDefinition {
        name:              "Aligned".into(),
        applies_to_tables: true,
        applies_to_pivots: false,
        elements:          vec![TableStyleElementDefinition {
            element_type: TableStyleElementType::HeaderRow,
            stripe_size:  1,
            format:       Some(aligned_style()),
        }],
    })
    .unwrap();
    book.set_custom_slicer_style(&SlicerStyleDefinition {
        name:         "AlignedSlicer".into(),
        whole_slicer: None,
        header:       None,
        items:        vec![SlicerStyleItemDefinition {
            element_type: SlicerStyleElementType::SelectedItemWithData,
            format:       Some(aligned_style()),
        }],
    })
    .unwrap();

    for (writer_name, output) in [("full", save(&book)), ("light", save_light(&book))] {
        let output_xml = styles_xml(&output);
        assert_alignment_xml(&output_xml);
        keep_output(&format!("alignment-{writer_name}"), &output);

        for (reader_name, read) in readers() {
            let reread = read(&output).unwrap();
            assert_authored_alignment(&reread);
            let round_trip = save(&reread);
            assert_alignment_xml(&styles_xml(&round_trip));
            let final_read = read(&round_trip).unwrap();
            assert_authored_alignment(&final_read);
            keep_output(
                &format!("alignment-{writer_name}-{reader_name}"),
                &round_trip,
            );
        }

        let paired = paired_alignment_package(&output);
        keep_output(&format!("alignment-paired-{writer_name}"), &paired);
        for (reader_name, read) in readers() {
            let reread = read(&paired).unwrap();
            assert_authored_alignment(&reread);
            let round_trip = save(&reread);
            assert_alignment_xml(&styles_xml(&round_trip));
            let final_read = read(&round_trip).unwrap();
            assert_authored_alignment(&final_read);
            keep_output(
                &format!("alignment-paired-{writer_name}-{reader_name}"),
                &round_trip,
            );
        }
    }
}

#[test]
fn authored_styles_are_written_in_schema_order_and_read_back() {
    let mut book = umya_spreadsheet::new_file();
    book.set_custom_table_style(&TableStyleDefinition {
        name:              "Tidy".into(),
        applies_to_tables: true,
        applies_to_pivots: false,
        elements:          vec![
            TableStyleElementDefinition {
                element_type: TableStyleElementType::HeaderRow,
                stripe_size:  1,
                format:       Some(red_bold()),
            },
            TableStyleElementDefinition {
                element_type: TableStyleElementType::FirstColumnStripe,
                stripe_size:  3,
                format:       Some(color("FF0000FF")),
            },
            TableStyleElementDefinition {
                element_type: TableStyleElementType::LastColumn,
                stripe_size:  1,
                format:       None,
            },
        ],
    })
    .unwrap();
    book.set_custom_slicer_style(&SlicerStyleDefinition {
        name:         "Chips".into(),
        whole_slicer: Some(color("FFEEEEEE")),
        header:       None,
        items:        vec![SlicerStyleItemDefinition {
            element_type: SlicerStyleElementType::SelectedItemWithData,
            format:       Some(color("FF00FF00")),
        }],
    })
    .unwrap();
    book.set_default_table_style("Tidy").unwrap();
    book.set_default_slicer_style("Chips").unwrap();

    let output = save(&book);
    keep_output("authored-tidy-chips", &output);
    let xml = styles_xml(&output);
    let tidy = &xml[xml.find(r#"<tableStyle name="Tidy""#).unwrap()..];
    assert!(
        tidy.starts_with(r#"<tableStyle name="Tidy" pivot="0" count="3">"#),
        "{tidy}"
    );
    let elements = tags(&xml, "<tableStyleElement");
    assert_eq!(
        elements.len(),
        3 + 1,
        "Tidy has three elements, Chips one (wholeTable)"
    );
    assert_eq!(
        attribute(elements[1], "type").as_deref(),
        Some("firstColumnStripe")
    );
    assert_eq!(attribute(elements[1], "size").as_deref(), Some("3"));
    assert_eq!(
        attribute(elements[2], "dxfId"),
        None,
        "a format-less element has no dxfId"
    );
    let chips = tags(&xml, "<tableStyle")[1];
    // one table element (wholeTable) plus one slicer item element
    assert_eq!(attribute(chips, "count").as_deref(), Some("2"));
    assert_eq!(tags(&xml, "<x14:slicerStyleElement").len(), 1);
    assert_eq!(attribute(chips, "pivot").as_deref(), Some("0"));
    assert_eq!(attribute(chips, "table").as_deref(), Some("0"));
    // formats: two Tidy formats plus the Chips whole-slicer format
    assert_eq!(
        attribute(tags(&xml, "<dxfs")[0], "count").as_deref(),
        Some("3")
    );
    assert_eq!(
        attribute(tags(&xml, "<x14:dxfs")[0], "count").as_deref(),
        Some("1")
    );
    assert!(xml.contains(r#"defaultTableStyle="Tidy""#));
    assert!(xml.contains(r#"defaultSlicerStyle="Chips""#));
    assert_eq!(tags(&xml, "<x14:slicerStyleElement").len(), 1);

    for (reader_name, read) in readers() {
        let back = read(&output).unwrap();
        assert_eq!(
            back.custom_table_styles(),
            book.custom_table_styles(),
            "{reader_name}"
        );
        assert_eq!(
            back.custom_slicer_styles(),
            book.custom_slicer_styles(),
            "{reader_name}"
        );
        let styles = back.custom_table_styles();
        assert_eq!(styles[0].elements[1].stripe_size, 3);
        assert_eq!(
            fill_argb(styles[0].elements[1].format.as_ref().unwrap()),
            "FF0000FF"
        );
        assert!(styles[0].elements[2].format.is_none());
        assert_eq!(back.default_table_style(), "Tidy");
    }
}

#[test]
fn removing_a_style_in_use_falls_back_to_the_workbook_default_as_excel_does() {
    // Excel (COM): with custom defaults DefT/DefP/DefS, deleting the styles a
    // table, a pivot table and a slicer used moved all three onto the defaults.
    let mut book = read_eager(&sample("s1-custom")).unwrap();
    let in_use = |name: &str| TableStyleDefinition {
        name:              name.into(),
        applies_to_tables: true,
        applies_to_pivots: true,
        elements:          vec![TableStyleElementDefinition {
            element_type: TableStyleElementType::WholeTable,
            stripe_size:  1,
            format:       Some(color("FFFFFF00")),
        }],
    };
    book.set_custom_table_style(&in_use("DefaultKept")).unwrap();
    book.set_default_table_style("DefaultKept").unwrap();
    book.set_default_pivot_table_style("DefaultKept").unwrap();
    assert!(book.remove_custom_table_style("MyTable").unwrap());
    assert!(book.remove_custom_table_style("MyPivot").unwrap());
    let sheet = book.sheet_by_name("Data").unwrap();
    assert_eq!(
        sheet.tables()[0].style_info().unwrap().name(),
        "DefaultKept"
    );
    let report = book.sheet_by_name("Report").unwrap();
    assert_eq!(
        report.pivot_tables()[0]
            .pivot_table_definition()
            .pivot_table_style()
            .name(),
        "DefaultKept"
    );
    let output = save(&book);
    keep_output("removed-in-use-fallback", &output);
    let xml = styles_xml(&output);
    assert!(!xml.contains("MyTable") && !xml.contains("MyPivot"));
    assert_eq!(
        tags(&xml, "<tableStyle").len(),
        2,
        "DefaultKept and the MySlicer entry"
    );

    // a slicer style in use: the slicer's style falls back to the default
    // slicer style
    book.set_custom_slicer_style(&SlicerStyleDefinition {
        name:         "Other".into(),
        whole_slicer: None,
        header:       None,
        items:        vec![],
    })
    .unwrap();
    for sheet in book.sheet_collection_mut() {
        for slicer in sheet.slicers_mut() {
            slicer.style = Some("Other".into());
        }
    }
    assert!(book.remove_custom_slicer_style("Other").unwrap());
    assert!(
        book.sheet_collection()
            .iter()
            .flat_map(|sheet| sheet.slicers())
            .all(|slicer| slicer.style.is_none())
    );
    assert!(
        !book.remove_custom_slicer_style("Other").unwrap(),
        "already removed"
    );
    assert!(
        !book.remove_custom_table_style("MySlicer").unwrap(),
        "a slicer style is not a table style"
    );
}

#[test]
fn removing_a_default_style_resets_to_excels_own_default() {
    // s5-after-delete-default.xlsx: Excel after deleting the custom default
    // table style
    let excel = styles_xml(&sample("s5-after-delete-default"));
    let expected_table = attribute(tags(&excel, "<tableStyles")[0], "defaultTableStyle").unwrap();
    let expected_pivot = attribute(tags(&excel, "<tableStyles")[0], "defaultPivotStyle").unwrap();
    let expected_slicer =
        attribute(tags(&excel, "<x14:slicerStyles")[0], "defaultSlicerStyle").unwrap();
    assert_eq!(
        (
            expected_table.as_str(),
            expected_pivot.as_str(),
            expected_slicer.as_str()
        ),
        (
            "TableStyleMedium2",
            "PivotStyleLight16",
            "SlicerStyleLight1"
        )
    );

    let mut book = read_eager(&sample("s1-custom")).unwrap();
    assert!(book.remove_custom_table_style("MyTable").unwrap());
    assert!(book.remove_custom_table_style("MyPivot").unwrap());
    assert!(book.remove_custom_slicer_style("MySlicer").unwrap());
    assert_eq!(book.default_table_style(), expected_table);
    assert_eq!(book.default_pivot_table_style(), expected_pivot);
    assert_eq!(book.default_slicer_style(), expected_slicer);
    let output = save(&book);
    keep_output("removed-defaults-reset", &output);
    let xml = styles_xml(&output);
    assert!(xml.contains(&format!(r#"defaultTableStyle="{expected_table}""#)));
    assert!(xml.contains(&format!(r#"defaultPivotStyle="{expected_pivot}""#)));
    assert_eq!(tags(&xml, "<tableStyle").len(), 0);
    assert!(tags(&xml, "<tableStyles")[0].contains(r#"count="0""#));
}

#[test]
fn style_in_use_on_a_table_falls_back_when_removed() {
    let mut book = read_eager(&sample("s2-fresh-in-use")).unwrap();
    assert_eq!(
        book.sheet_by_name("Data").unwrap().tables()[0]
            .style_info()
            .unwrap()
            .name(),
        "Fresh"
    );
    assert!(book.remove_custom_table_style("Fresh").unwrap());
    // Excel: the table reports TableStyleMedium2 and the file writes count="0"
    assert_eq!(
        book.sheet_by_name("Data").unwrap().tables()[0]
            .style_info()
            .unwrap()
            .name(),
        "TableStyleMedium2"
    );
    let output = save(&book);
    keep_output("removed-table-style-in-use", &output);
    let xml = styles_xml(&output);
    assert!(tags(&xml, "<tableStyles")[0].contains(r#"count="0""#));
}

#[test]
fn invalid_style_definitions_are_refused() {
    let mut book = umya_spreadsheet::new_file();
    let definition = |name: &str| TableStyleDefinition {
        name:              name.into(),
        applies_to_tables: true,
        applies_to_pivots: false,
        elements:          vec![],
    };
    assert!(
        book.set_custom_table_style(&definition("TableStyleMedium2"))
            .is_err()
    );
    assert!(
        book.set_custom_table_style(&definition("PivotStyleLight16"))
            .is_err()
    );
    assert!(
        book.set_custom_table_style(&definition("TableStyleMedium29"))
            .is_ok(),
        "past the built-in range"
    );
    assert!(book.set_custom_table_style(&definition("")).is_err());
    let mut neither = definition("Neither");
    neither.applies_to_tables = false;
    assert!(book.set_custom_table_style(&neither).is_err());
    let mut twice = definition("Twice");
    twice.elements = vec![
        TableStyleElementDefinition {
            element_type: TableStyleElementType::HeaderRow,
            stripe_size:  1,
            format:       None,
        };
        2
    ];
    assert!(book.set_custom_table_style(&twice).is_err());
    let mut wide_header = definition("WideHeader");
    wide_header.elements = vec![TableStyleElementDefinition {
        element_type: TableStyleElementType::HeaderRow,
        stripe_size:  2,
        format:       None,
    }];
    assert!(book.set_custom_table_style(&wide_header).is_err());
    assert!(book.set_default_table_style("Nope").is_err());
    assert!(
        book.set_default_pivot_table_style("TableStyleMedium2")
            .is_err()
    );
    assert!(book.set_default_slicer_style("Nope").is_err());
    // a name held by a table style cannot be taken by a slicer style, and vice
    // versa
    let slicer = |name: &str| SlicerStyleDefinition {
        name:         name.into(),
        whole_slicer: None,
        header:       None,
        items:        vec![],
    };
    assert!(
        book.set_custom_slicer_style(&slicer("TableStyleMedium29"))
            .is_err()
    );
    book.set_custom_slicer_style(&slicer("Round")).unwrap();
    assert!(book.set_custom_table_style(&definition("Round")).is_err());
    assert!(
        book.set_custom_slicer_style(&slicer("Round")).is_ok(),
        "replacing keeps the name"
    );
    assert_eq!(book.custom_slicer_styles().len(), 1);
}

#[test]
fn leaf_elements_may_use_start_and_end_tags() {
    let source = sample("s1-custom");
    let xml = styles_xml(&source);
    let rewritten = xml
        .replace(
            r#"<tableStyleElement type="headerRow" dxfId="19"/>"#,
            r#"<tableStyleElement type="headerRow" dxfId="19"></tableStyleElement>"#,
        )
        .replace(
            r#"<x14:slicerStyleElement type="selectedItemWithData" dxfId="5"/>"#,
            r#"<x14:slicerStyleElement type="selectedItemWithData" dxfId="5"></x14:slicerStyleElement>"#,
        );
    assert_ne!(
        rewritten, xml,
        "the fixture still has the self-closing forms"
    );
    let rebuilt = with_styles_xml(&source, rewritten);
    for (reader_name, read) in readers() {
        let book = read(&rebuilt).unwrap();
        let table = book
            .custom_table_styles()
            .into_iter()
            .find(|style| style.name == "MyTable")
            .unwrap();
        assert!(
            table
                .elements
                .iter()
                .any(|e| e.element_type == TableStyleElementType::HeaderRow && e.format.is_some()),
            "{reader_name}"
        );
        assert_eq!(
            book.custom_slicer_styles()[0].items.len(),
            8,
            "{reader_name}"
        );
    }
}

#[test]
fn an_unknown_table_style_element_type_is_an_error() {
    let source = sample("s1-custom");
    let xml = styles_xml(&source).replace(
        r#"<tableStyleElement type="wholeTable" dxfId="13"/>"#,
        r#"<tableStyleElement type="wholeTabel" dxfId="13"/>"#,
    );
    let rebuilt = with_styles_xml(&source, xml);
    let error = read_eager(&rebuilt).unwrap_err().to_string();
    assert!(error.contains("wholeTabel"), "{error}");
}

#[test]
fn a_table_style_dxf_id_outside_the_main_list_is_an_error() {
    let source = sample("s1-custom");
    let xml = styles_xml(&source).replace(
        r#"<tableStyleElement type="wholeTable" dxfId="13"/>"#,
        r#"<tableStyleElement type="wholeTable" dxfId="21"/>"#,
    );
    let rebuilt = with_styles_xml(&source, xml);
    let error = read_eager(&rebuilt).unwrap_err().to_string();
    assert!(error.contains("MyPivot"), "{error}");
    assert!(error.contains("dxfId 21"), "{error}");
}

#[test]
fn a_slicer_style_dxf_id_outside_its_list_is_an_error() {
    let source = sample("s1-custom");
    let xml = styles_xml(&source).replace(
        r#"<x14:slicerStyleElement type="selectedItemWithData" dxfId="5"/>"#,
        r#"<x14:slicerStyleElement type="selectedItemWithData" dxfId="8"/>"#,
    );
    let rebuilt = with_styles_xml(&source, xml);
    let error = read_eager(&rebuilt).unwrap_err().to_string();
    assert!(error.contains("MySlicer"), "{error}");
    assert!(error.contains("dxfId 8"), "{error}");
}
