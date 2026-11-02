#![cfg(feature = "serde")]

use std::io::{
    Cursor,
    Read,
    Write,
};

use umya_spreadsheet::{
    Workbook,
    drawing::{
        charts::{
            ChartDrawingObject,
            ChartReferenceEdit,
            Formula,
        },
        spreadsheet::{
            DrawingObject,
            GroupShape,
            Shape,
        },
    },
    reader,
    writer,
};

const EXCEL: &[u8] = include_bytes!("fixtures/chart_drawing_excel.xlsx");
const LINKED: &[u8] = include_bytes!("fixtures/chart_drawing_linked_excel.xlsx");
const RICH: &[u8] = include_bytes!("fixtures/chart_shapes_rich_excel.xlsx");

fn with_preserved_link(group: bool) -> (Vec<u8>, String) {
    let mut source = zip::ZipArchive::new(Cursor::new(RICH)).unwrap();
    let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let mut original = String::new();
    for index in 0..source.len() {
        let mut file = source.by_index(index).unwrap();
        let name = file.name().unwrap().into_owned();
        let mut data = Vec::new();
        file.read_to_end(&mut data).unwrap();
        if name == "xl/drawings/drawing2.xml" {
            let xml = String::from_utf8(data).unwrap();
            let first = xml.find("<cdr:sp ").unwrap();
            let last = xml[first..].find("</cdr:sp>").unwrap() + first + "</cdr:sp>".len();
            let shape = xml[first..last].replacen("textlink=\"\"", "textlink = 'Sheet1!$A$2'", 1);
            let picture_start = xml.find("<cdr:pic>").unwrap();
            let picture_end = xml[picture_start..].find("</cdr:pic>").unwrap()
                + picture_start
                + "</cdr:pic>".len();
            let object = if group {
                format!(
                    "<cdr:grpSp><cdr:nvGrpSpPr><cdr:cNvPr id=\"11\" \
                     name=\"Mixed\"/><cdr:cNvGrpSpPr/></cdr:nvGrpSpPr><cdr:grpSpPr/>{shape}{}</\
                     cdr:grpSp>",
                    &xml[picture_start..picture_end]
                )
            } else {
                shape.replace("</cdr:spPr>", "<a:blipFill xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\"><a:blip r:embed=\"rId1\"/><a:stretch><a:fillRect/></a:stretch></a:blipFill></cdr:spPr>")
            };
            let root = if group {
                "<objects:grpSp"
            } else {
                "<objects:sp"
            };
            original = object.replace("cdr:", "objects:").replacen(root,
                &format!("{root} xmlns:objects=\"http://schemas.openxmlformats.org/drawingml/2006/chartDrawing\""), 1);
            let anchor_start = xml.find("<cdr:relSizeAnchor").unwrap();
            let rewritten = format!(
                "{}{}{original}</cdr:relSizeAnchor></c:userShapes>",
                &xml[..anchor_start],
                &xml[anchor_start..first]
            );
            data = rewritten.into_bytes();
        }
        output
            .start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        output.write_all(&data).unwrap();
    }
    assert!(!original.is_empty());
    (output.finish().unwrap().into_inner(), original)
}

fn with_attributes(attributes: &str) -> Vec<u8> {
    with_attributes_in(EXCEL, attributes)
}

fn with_attributes_in(package: &[u8], attributes: &str) -> Vec<u8> {
    let mut source = zip::ZipArchive::new(Cursor::new(package)).unwrap();
    let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let mut replaced = false;
    for index in 0..source.len() {
        let mut file = source.by_index(index).unwrap();
        let name = file.name().unwrap().into_owned();
        let mut data = Vec::new();
        file.read_to_end(&mut data).unwrap();
        if name.starts_with("xl/drawings/") && name.ends_with(".xml") {
            let mut xml = String::from_utf8(data).unwrap();
            if xml.contains("<c:userShapes") {
                let start = xml.find("<cdr:sp ").unwrap();
                let end = start + xml[start..].find('>').unwrap() + 1;
                xml.replace_range(start..end, &format!("<cdr:sp {attributes}>"));
                replaced = true;
            }
            data = xml.into_bytes();
        }
        output
            .start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        output.write_all(&data).unwrap();
    }
    assert!(replaced);
    output.finish().unwrap().into_inner()
}

fn open(bytes: &[u8], eager: bool) -> Workbook {
    let mut book = reader::xlsx::read_reader(Cursor::new(bytes.to_vec()), eager).unwrap();
    book.read_sheet_collection();
    book
}

fn save(book: &Workbook, light: bool) -> Vec<u8> {
    let mut out = Cursor::new(Vec::new());
    if light {
        writer::xlsx::write_writer_light(book, &mut out).unwrap();
    } else {
        writer::xlsx::write_writer(book, &mut out).unwrap();
    }
    out.into_inner()
}

fn first_shape(book: &Workbook) -> &Shape {
    book.sheet(0).unwrap().chart_collection()[0]
        .chart_space()
        .chart_drawing()
        .unwrap()
        .anchors()[0]
        .shape()
        .unwrap()
}

fn first_shape_mut(book: &mut Workbook) -> &mut Shape {
    book.sheet_mut(0).unwrap().chart_collection_mut()[0]
        .chart_space_mut()
        .chart_drawing_mut()
        .anchors_mut()[0]
        .shape_mut()
        .unwrap()
}

fn chart_drawing_xml(bytes: &[u8]) -> String {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    for index in 0..archive.len() {
        let mut file = archive.by_index(index).unwrap();
        if file.name().unwrap().starts_with("xl/drawings/")
            && file.name().unwrap().ends_with(".xml")
        {
            let mut xml = String::new();
            file.read_to_string(&mut xml).unwrap();
            if xml.contains("<c:userShapes") {
                return xml;
            }
        }
    }
    panic!("chart drawing part");
}

#[test]
fn chart_shape_link_and_flags_survive_read_edit_write() {
    // Excel COM stores the cell link without a leading '='. The booleans and
    // their defaults come from dml-chartDrawing.xsd CT_Shape.
    let bytes = with_attributes_in(
        LINKED,
        "macro=\"\" textlink=\"Data!$A$2\" fLocksText=\"false\" fPublished=\"true\"",
    );
    for eager in [true, false] {
        let mut book = open(&bytes, eager);
        assert_eq!(
            first_shape(&book).text_link().unwrap().address_str(),
            "Data!$A$2"
        );
        assert!(!first_shape(&book).locks_text());
        assert!(first_shape(&book).published());
        let fields = first_shape(&book).text_body().unwrap().paragraph()[0].fields();
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].field.kind.as_deref(), Some("TxLink"));
        assert_eq!(fields[0].field.text.as_deref(), Some("linked text"));
        first_shape_mut(&mut book)
            .text_link_mut()
            .unwrap()
            .set_string_value("Data!$A$3");
        for light in [false, true] {
            let output = save(&book, light);
            let xml = chart_drawing_xml(&output);
            assert!(xml.contains("textlink=\"Data!$A$3\""), "{xml}");
            assert!(xml.contains("fLocksText=\"0\""), "{xml}");
            assert!(xml.contains("fPublished=\"1\""), "{xml}");
            if let Some(target) = std::env::var_os("CARGO_TARGET_DIR") {
                let directory = std::path::PathBuf::from(target).join("chart-drawing-attributes");
                std::fs::create_dir_all(&directory).unwrap();
                std::fs::write(directory.join(format!("edited-{light}.xlsx")), &output).unwrap();
            }
            let reread = open(&output, true);
            assert_eq!(
                first_shape(&reread).text_link().unwrap().address_str(),
                "Data!$A$3"
            );
            assert!(!first_shape(&reread).locks_text());
            assert!(first_shape(&reread).published());
            let fields = first_shape(&reread).text_body().unwrap().paragraph()[0].fields();
            assert_eq!(fields.len(), 1);
            assert_eq!(fields[0].field.kind.as_deref(), Some("TxLink"));
            assert_eq!(fields[0].field.text.as_deref(), Some("linked text"));
        }
    }
}

#[test]
fn linked_chart_text_follows_workbook_insert_rename_and_delete() {
    let bytes = with_attributes("textlink=\"Sheet1!$A$2\"");
    for eager in [true, false] {
        let mut book = open(&bytes, eager);
        book.insert_new_row("Sheet1", 2, 1);
        assert_eq!(
            first_shape(&book).text_link().unwrap().address_str(),
            "Sheet1!$A$3"
        );
        book.set_sheet_name(0, "Bob's Data").unwrap();
        assert_eq!(
            first_shape(&book).text_link().unwrap().address_str(),
            "'Bob''s Data'!$A$3"
        );
        let chart = book.sheet(0).unwrap().chart_collection()[0].clone();
        book.new_sheet("Host").unwrap().add_chart(chart);
        book.remove_sheet(0).unwrap();
        // Excel serializes textlink="#REF!" when its source sheet is deleted.
        assert_eq!(
            first_shape(&book).text_link().unwrap().address_str(),
            "#REF!"
        );
        for light in [false, true] {
            let output = save(&book, light);
            assert!(chart_drawing_xml(&output).contains("textlink=\"#REF!\""));
            assert_eq!(
                first_shape(&open(&output, true))
                    .text_link()
                    .unwrap()
                    .address_str(),
                "#REF!"
            );
        }
    }
}

#[test]
fn nested_shape_text_links_use_the_chart_reference_role() {
    let mut book = open(&with_attributes("textlink=\"$A$2\""), true);
    let shape = first_shape(&book).clone();
    let mut inner = GroupShape::default();
    inner
        .non_visual_group_shape_properties_mut()
        .non_visual_drawing_properties_mut()
        .set_id(5);
    inner.add_object(DrawingObject::Shape(Box::new(shape)));
    let mut outer = GroupShape::default();
    outer
        .non_visual_group_shape_properties_mut()
        .non_visual_drawing_properties_mut()
        .set_id(6);
    outer.add_object(DrawingObject::Group(Box::new(inner)));
    let chart = &mut book.sheet_mut(0).unwrap().chart_collection_mut()[0];
    *chart.chart_space_mut().chart_drawing_mut().anchors_mut()[0].object_mut() =
        ChartDrawingObject::Group(Box::new(outer));
    for group in chart
        .chart_space_mut()
        .chart_mut()
        .plot_area_mut()
        .chart_groups_mut()
    {
        group.series_mut().set_area_chart_series(Vec::new());
    }
    let report = chart.adjust_references(
        "Sheet1",
        &ChartReferenceEdit::InsertRows {
            sheet: "Sheet1".into(),
            at:    2,
            count: 1,
        },
    );
    assert_eq!(report.visited, 1);
    assert_eq!(report.changed, 1);
    assert_eq!(report.invalidated, 0);
    let output = save(&book, false);
    let xml = chart_drawing_xml(&output);
    assert!(xml.contains("textlink=\"$A$3\""), "{xml}");
    let reread = open(&output, true);
    let object = reread.sheet(0).unwrap().chart_collection()[0]
        .chart_space()
        .chart_drawing()
        .unwrap()
        .anchors()[0]
        .object();
    let ChartDrawingObject::Group(outer) = object else {
        panic!("outer group")
    };
    let DrawingObject::Group(inner) = &outer.content()[0] else {
        panic!("inner group")
    };
    let DrawingObject::Shape(shape) = &inner.content()[0] else {
        panic!("linked shape")
    };
    assert_eq!(shape.text_link().unwrap().address_str(), "$A$3");
}

#[test]
fn shape_link_removal_and_old_json_keep_schema_defaults() {
    let mut book = open(&with_attributes("textlink=\"Sheet1!$A$2\""), true);
    first_shape_mut(&mut book)
        .set_text_link(None)
        .set_locks_text(true)
        .set_published(false);
    for light in [false, true] {
        let output = save(&book, light);
        let reread = open(&output, true);
        assert!(first_shape(&reread).text_link().is_none());
        assert!(first_shape(&reread).locks_text());
        assert!(!first_shape(&reread).published());
    }
    let mut json = serde_json::to_value(first_shape(&book)).unwrap();
    for key in ["text_link", "locks_text", "published"] {
        json.as_object_mut().unwrap().remove(key);
    }
    let old: Shape = serde_json::from_value(json).unwrap();
    assert_eq!(old.text_link().unwrap().address_str(), "");
    assert!(old.locks_text());
    assert!(!old.published());
    let mut link = Formula::default();
    link.set_string_value("Sheet1!$B$4");
    first_shape_mut(&mut book).set_text_link(Some(link));
    assert_eq!(
        first_shape(&open(&save(&book, false), true))
            .text_link()
            .unwrap()
            .address_str(),
        "Sheet1!$B$4"
    );
}

#[test]
fn preserved_shape_links_follow_edits_without_rewriting_other_bytes() {
    for group in [false, true] {
        let (bytes, original_xml) = with_preserved_link(group);
        for eager in [true, false] {
            let mut book = open(&bytes, eager);
            let chart = &book.sheet(0).unwrap().chart_collection()[0];
            let anchor = &chart.chart_space().chart_drawing().unwrap().anchors()[0];
            assert!(matches!(anchor.object(), ChartDrawingObject::Preserved(_)));
            assert_eq!(anchor.preserved_text_links().len(), 1);
            assert_eq!(anchor.preserved_text_links()[0].shape_id(), 2);
            assert_eq!(
                anchor.preserved_text_links()[0].formula().address_str(),
                "Sheet1!$A$2"
            );
            for light in [false, true] {
                assert!(chart_drawing_xml(&save(&book, light)).contains(&original_xml));
            }
            book.insert_new_row("Sheet1", 2, 1);
            for light in [false, true] {
                let output = save(&book, light);
                assert!(
                    chart_drawing_xml(&output)
                        .contains(&original_xml.replace("Sheet1!$A$2", "Sheet1!$A$3"))
                );
                let reread = open(&output, true);
                assert_eq!(
                    reread.sheet(0).unwrap().chart_collection()[0]
                        .chart_space()
                        .chart_drawing()
                        .unwrap()
                        .anchors()[0]
                        .preserved_text_links()[0]
                        .formula()
                        .address_str(),
                    "Sheet1!$A$3"
                );
            }
            let original = book.sheet(0).unwrap().chart_collection()[0].clone();
            let mut projected = original.clone();
            projected.strip_export_parts();
            let mut returned: umya_spreadsheet::Chart =
                serde_json::from_slice(&serde_json::to_vec(&projected).unwrap()).unwrap();
            returned.chart_space_mut().chart_drawing_mut().anchors_mut()[0]
                .preserved_text_links_mut()[0]
                .formula_mut()
                .set_string_value("Sheet1!$A$4");
            returned.restore_export_parts(&original).unwrap();
            book.sheet_mut(0).unwrap().chart_collection_mut()[0] = returned;
            for light in [false, true] {
                let output = save(&book, light);
                assert!(
                    chart_drawing_xml(&output)
                        .contains(&original_xml.replace("Sheet1!$A$2", "Sheet1!$A$4"))
                );
                let reread = open(&output, true);
                assert_eq!(
                    reread.sheet(0).unwrap().chart_collection()[0]
                        .chart_space()
                        .chart_drawing()
                        .unwrap()
                        .anchors()[0]
                        .preserved_text_links()[0]
                        .formula()
                        .address_str(),
                    "Sheet1!$A$4"
                );
            }
        }
    }
}

#[test]
fn lazy_shape_links_select_hosts_without_chart_series_dependencies() {
    let mut source = open(EXCEL, true);
    source
        .new_sheet("Data")
        .unwrap()
        .cell_mut("A2")
        .set_value("linked");
    let mut link = Formula::default();
    link.set_string_value("Data!$A$2");
    first_shape_mut(&mut source).set_text_link(Some(link));
    let bytes = save(&source, false);
    for edit in ["insert", "rename", "delete"] {
        let mut book = reader::xlsx::read_reader(Cursor::new(&bytes), false).unwrap();
        assert_eq!(
            book.sheet(0).unwrap_err().kind(),
            umya_spreadsheet::XlsxErrorKind::NotDeserialized
        );
        let expected = match edit {
            "insert" => {
                book.insert_new_row("Data", 2, 1);
                "Data!$A$3"
            }
            "rename" => {
                book.set_sheet_name(1, "Renamed Data").unwrap();
                "'Renamed Data'!$A$2"
            }
            "delete" => {
                book.remove_sheet(1).unwrap();
                "#REF!"
            }
            _ => unreachable!(),
        };
        assert_eq!(
            first_shape(&book).text_link().unwrap().address_str(),
            expected
        );
        for light in [false, true] {
            let output = save(&book, light);
            let reread = open(&output, true);
            assert_eq!(
                first_shape(&reread).text_link().unwrap().address_str(),
                expected
            );
        }
    }
    let foreign = with_attributes_in(
        &bytes,
        "xmlns:foreign=\"urn:foreign\" foreign:textlink=\"Data!$A$2\"",
    );
    let mut book = reader::xlsx::read_reader(Cursor::new(foreign), false).unwrap();
    book.set_sheet_name(1, "Renamed Data").unwrap();
    assert_eq!(
        book.sheet(0).unwrap_err().kind(),
        umya_spreadsheet::XlsxErrorKind::NotDeserialized
    );
}
