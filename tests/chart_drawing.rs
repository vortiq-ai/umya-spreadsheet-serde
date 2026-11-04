#![cfg(feature = "serde")]
//! Shapes and text boxes inside a chart (`c:userShapes`, the chart drawing
//! part), through the ordinary route: Excel-authored package -> reader ->
//! public API -> writer -> package inspection and reread.
//!
//! Expected values come from the files Excel 365 wrote (`chart_drawing_excel`:
//! a text box "Hello chart", a red rectangle with a green outline and a
//! straight connector; `chart_drawing_none_excel`: the same chart without
//! shapes) and from dml-chartDrawing.xsd. Measured with Excel: a text box and
//! a shape are `cdr:relSizeAnchor` entries with fractions of the chart; the
//! chart names its drawing part with a `chartUserShapes` relationship and a
//! `c:userShapes r:id` element after `c:printSettings`; a chart without shapes
//! has neither the part nor the relationship.
use std::io::{
    Cursor,
    Read,
    Write,
};

use umya_spreadsheet::{
    Workbook,
    drawing::charts::{
        ChartDrawing,
        ChartDrawingObject,
        ChartDrawingPlacement,
        ChartDrawingPoint,
    },
    reader,
    writer,
};

const EXCEL: &[u8] = include_bytes!("fixtures/chart_drawing_excel.xlsx");
const EXCEL_NONE: &[u8] = include_bytes!("fixtures/chart_drawing_none_excel.xlsx");
const USER_SHAPES: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/chartUserShapes";
const USER_SHAPES_TYPE: &str =
    "application/vnd.openxmlformats-officedocument.drawingml.chartshapes+xml";

fn open(bytes: &[u8]) -> Workbook {
    reader::xlsx::read_reader(Cursor::new(bytes.to_vec()), true).unwrap()
}

fn save(book: &Workbook) -> Vec<u8> {
    let mut out = Cursor::new(Vec::new());
    writer::xlsx::write_writer(book, &mut out).unwrap();
    out.into_inner()
}

/// Saves the workbook and, when `CARGO_TARGET_DIR` is set, also leaves the
/// package where the Excel check script finds it.
fn save_as(name: &str, book: &Workbook) -> Vec<u8> {
    let bytes = save(book);
    if let Some(target) = std::env::var_os("CARGO_TARGET_DIR") {
        let directory = std::path::PathBuf::from(target).join("chart-drawing-validation");
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join(format!("{name}.xlsx")), &bytes).unwrap();
    }
    bytes
}

fn entry(package: &[u8], name: &str) -> Option<Vec<u8>> {
    let mut archive = zip::ZipArchive::new(Cursor::new(package)).unwrap();
    let mut file = archive.by_name(name).ok()?;
    let mut data = Vec::new();
    file.read_to_end(&mut data).unwrap();
    Some(data)
}

fn text(package: &[u8], name: &str) -> String {
    String::from_utf8(entry(package, name).unwrap_or_else(|| panic!("missing {name}"))).unwrap()
}

fn names(package: &[u8]) -> Vec<String> {
    let mut archive = zip::ZipArchive::new(Cursor::new(package)).unwrap();
    (0..archive.len())
        .map(|index| archive.by_index(index).unwrap().name().unwrap().to_string())
        .collect()
}

fn drawing(book: &Workbook) -> Option<&ChartDrawing> {
    book.sheet(0).unwrap().chart_collection()[0]
        .chart_space()
        .chart_drawing()
}

fn drawing_mut(book: &mut Workbook) -> &mut ChartDrawing {
    book.sheet_mut(0).unwrap().chart_collection_mut()[0]
        .chart_space_mut()
        .chart_drawing_mut()
}

/// The chart drawing part the package's chart points at, with its text.
fn drawing_part(package: &[u8]) -> (String, String) {
    let chart = text(package, "xl/charts/chart1.xml");
    let rels = text(package, "xl/charts/_rels/chart1.xml.rels");
    let edge = rels
        .split("<Relationship ")
        .find(|edge| edge.contains(USER_SHAPES))
        .expect("chart has a chartUserShapes relationship");
    let id = edge
        .split("Id=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();
    assert!(
        chart.contains(&format!("<c:userShapes r:id=\"{id}\"/>")),
        "{chart}"
    );
    let target = edge
        .split("Target=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();
    let part = format!("xl/{}", target.trim_start_matches("../"));
    let override_xml = format!("PartName=\"/{part}\" ContentType=\"{USER_SHAPES_TYPE}\"");
    assert!(
        text(package, "[Content_Types].xml").contains(&override_xml),
        "content type of {part}"
    );
    let data = text(package, &part);
    (part, data)
}

fn shape_name(object: &ChartDrawingObject) -> String {
    match object {
        ChartDrawingObject::Shape(shape) => shape
            .non_visual_shape_properties()
            .non_visual_drawing_properties()
            .name()
            .to_owned(),
        ChartDrawingObject::Connector(connector) => connector
            .non_visual_connection_shape_properties()
            .non_visual_drawing_properties()
            .name()
            .to_owned(),
        other => panic!("unexpected object {other:?}"),
    }
}

#[test]
fn reads_what_excel_wrote() {
    let book = open(EXCEL);
    let drawing = drawing(&book).expect("chart has a drawing");
    let anchors = drawing.anchors();
    assert_eq!(anchors.len(), 3);

    assert_eq!(
        anchors[0].placement(),
        ChartDrawingPlacement::Relative {
            from: ChartDrawingPoint::new(0.135, 0.176),
            to:   ChartDrawingPoint::new(0.435, 0.296),
        }
    );
    let text_box = anchors[0].shape().expect("text box is a shape");
    assert_eq!(shape_name(anchors[0].object()), "TextBox 1");
    assert!(
        text_box
            .non_visual_shape_properties()
            .non_visual_shape_drawing_properties()
            .text_box()
    );
    assert_eq!(
        text_box.text_body().unwrap().paragraph()[0].run()[0].text(),
        "Hello chart"
    );

    assert_eq!(shape_name(anchors[1].object()), "Rectangle 2");
    let rectangle = anchors[1].shape().unwrap();
    assert_eq!(rectangle.shape_properties().geometry().geometry(), "rect");
    assert_eq!(
        rectangle
            .shape_properties()
            .solid_fill()
            .unwrap()
            .rgb_color_model_hex()
            .unwrap()
            .val(),
        "FF0000"
    );
    assert_eq!(
        rectangle
            .shape_properties()
            .outline()
            .unwrap()
            .solid_fill()
            .unwrap()
            .rgb_color_model_hex()
            .unwrap()
            .val(),
        "00FF00"
    );

    assert_eq!(shape_name(anchors[2].object()), "Straight Connector 3");
    assert!(matches!(
        anchors[2].object(),
        ChartDrawingObject::Connector(_)
    ));
    assert_eq!(
        anchors[2].placement(),
        ChartDrawingPlacement::Relative {
            from: ChartDrawingPoint::new(0.035, 0.616),
            to:   ChartDrawingPoint::new(0.51, 0.616),
        }
    );
}

#[test]
fn unedited_chart_writes_the_same_shapes_back() {
    let package = save_as("unedited", &open(EXCEL));
    let (_, part) = drawing_part(&package);
    assert!(part.starts_with("<?xml"), "{part}");
    assert!(part.contains("<c:userShapes "), "{part}");
    for fragment in [
        "<cdr:relSizeAnchor><cdr:from><cdr:x>0.135</cdr:x><cdr:y>0.176</cdr:y></cdr:from><cdr:\
         to><cdr:x>0.435</cdr:x><cdr:y>0.296</cdr:y></cdr:to><cdr:sp ",
        "name=\"TextBox 1\"",
        "<cdr:cNvSpPr txBox=\"1\"/>",
        "<a:t>Hello chart</a:t>",
        "<cdr:from><cdr:x>0.51</cdr:x><cdr:y>0.256</cdr:y></cdr:from><cdr:to><cdr:x>0.71</cdr:\
         x><cdr:y>0.416</cdr:y></cdr:to>",
        "<a:srgbClr val=\"FF0000\"/>",
        "<cdr:cxnSp ",
        "<cdr:from><cdr:x>0.035</cdr:x><cdr:y>0.616</cdr:y></cdr:from>",
    ] {
        assert!(part.contains(fragment), "missing {fragment} in {part}");
    }
    assert_eq!(part.matches("<cdr:relSizeAnchor>").count(), 3, "{part}");
    assert!(!part.contains("xdr:"), "{part}");

    let reread = open(&package);
    let shapes = drawing(&reread).unwrap();
    assert_eq!(shapes.anchors().len(), 3);
    assert_eq!(shape_name(shapes.anchors()[0].object()), "TextBox 1");
}

#[test]
fn edits_a_text_box_and_a_shape() {
    let mut book = open(EXCEL);
    {
        let anchors = drawing_mut(&mut book).anchors_mut();
        let text_box = anchors[0].shape_mut().unwrap();
        let paragraph = &mut text_box.text_body_mut().unwrap().paragraph_mut()[0];
        let mut run = paragraph.run()[0].clone();
        run.set_text("Edited");
        paragraph.remove_run(0).unwrap();
        assert!(paragraph.insert_run(0, run));
        anchors[0].set_placement(ChartDrawingPlacement::Relative {
            from: ChartDrawingPoint::new(0.25, 0.5),
            to:   ChartDrawingPoint::new(0.75, 0.625),
        });
        anchors[1]
            .shape_mut()
            .unwrap()
            .shape_properties_mut()
            .geometry_mut()
            .set_geometry("ellipse");
    }
    let package = save_as("edited", &book);
    let (_, part) = drawing_part(&package);
    assert!(part.contains("<a:t>Edited</a:t>"), "{part}");
    assert!(!part.contains("Hello chart"), "{part}");
    assert!(
        part.contains(
            "<cdr:from><cdr:x>0.25</cdr:x><cdr:y>0.5</cdr:y></cdr:from><cdr:to><cdr:x>0.75</cdr:\
             x><cdr:y>0.625</cdr:y></cdr:to>"
        ),
        "{part}"
    );
    assert!(part.contains("prst=\"ellipse\""), "{part}");

    let reread = open(&package);
    let anchors = drawing(&reread).unwrap().anchors();
    assert_eq!(
        anchors[0].shape().unwrap().text_body().unwrap().paragraph()[0].run()[0].text(),
        "Edited"
    );
    assert_eq!(
        anchors[1]
            .shape()
            .unwrap()
            .shape_properties()
            .geometry()
            .geometry(),
        "ellipse"
    );
}

#[test]
fn adds_a_text_box_and_a_shape_to_a_chart_without_shapes() {
    let mut book = open(EXCEL_NONE);
    assert!(drawing(&book).is_none());
    {
        let drawing = drawing_mut(&mut book);
        drawing.add_text_box(
            "Note",
            ChartDrawingPoint::new(0.1, 0.1),
            ChartDrawingPoint::new(0.4, 0.2),
        );
        drawing.add_shape(
            "rect",
            ChartDrawingPoint::new(0.5, 0.5),
            ChartDrawingPoint::new(0.75, 0.75),
            Some("0000FF"),
            Some("FFFF00"),
        );
    }
    let package = save_as("added", &book);
    let (part, data) = drawing_part(&package);
    assert_eq!(data.matches("<cdr:relSizeAnchor>").count(), 2, "{data}");
    // Excel numbers a new object after the highest id and names it by kind.
    assert!(
        data.contains("<cdr:cNvPr id=\"2\" name=\"TextBox 1\""),
        "{data}"
    );
    assert!(
        data.contains("<cdr:cNvPr id=\"3\" name=\"Shape 2\""),
        "{data}"
    );
    assert!(data.contains("<a:t>Note</a:t>"), "{data}");
    assert!(data.contains("<a:srgbClr val=\"0000FF\"/>"), "{data}");
    assert!(data.contains("<a:srgbClr val=\"FFFF00\"/>"), "{data}");
    assert!(part.starts_with("xl/"), "{part}");

    let reread = open(&package);
    assert_eq!(drawing(&reread).unwrap().anchors().len(), 2);
}

#[test]
fn removing_every_shape_removes_the_part_and_the_relationship() {
    let mut book = open(EXCEL);
    let (old_part, _) = drawing_part(&save(&book));
    while drawing(&book).is_some_and(|drawing| !drawing.is_empty()) {
        drawing_mut(&mut book).remove_anchor(0).unwrap();
    }
    let package = save_as("removed", &book);
    let chart = text(&package, "xl/charts/chart1.xml");
    assert!(!chart.contains("userShapes"), "{chart}");
    assert!(
        entry(&package, "xl/charts/_rels/chart1.xml.rels").is_none_or(|rels| !String::from_utf8(
            rels
        )
        .unwrap()
        .contains(USER_SHAPES))
    );
    assert!(
        !names(&package).contains(&old_part),
        "{:?}",
        names(&package)
    );
    assert!(
        !text(&package, "[Content_Types].xml").contains(USER_SHAPES_TYPE),
        "content type stays declared"
    );
    assert!(drawing(&open(&package)).is_none());
}

#[test]
fn a_chart_copied_to_another_workbook_brings_its_shapes() {
    let source = open(EXCEL);
    let chart = source.sheet(0).unwrap().chart_collection()[0].clone();
    let mut target = umya_spreadsheet::new_file();
    target.sheet_mut(0).unwrap().add_chart(chart);
    let package = save_as("copied", &target);
    let (_, part) = drawing_part(&package);
    assert_eq!(part.matches("<cdr:relSizeAnchor>").count(), 3, "{part}");
    assert!(part.contains("<a:t>Hello chart</a:t>"), "{part}");
    assert_eq!(drawing(&open(&package)).unwrap().anchors().len(), 3);
}

#[test]
fn the_projection_keeps_the_shapes_and_restores_from_the_original() {
    let book = open(EXCEL);
    let original = book.sheet(0).unwrap().chart_collection()[0].clone();
    let mut projection = original.clone();
    projection.strip_export_parts();
    assert_eq!(
        projection
            .chart_space()
            .chart_drawing()
            .unwrap()
            .anchors()
            .len(),
        3
    );
    let json = serde_json::to_string(&projection).unwrap();
    let mut returned: umya_spreadsheet::Chart = serde_json::from_str(&json).unwrap();
    returned.chart_space_mut().chart_drawing_mut().anchors_mut()[0].set_placement(
        ChartDrawingPlacement::Relative {
            from: ChartDrawingPoint::new(0.0, 0.0),
            to:   ChartDrawingPoint::new(0.5, 0.5),
        },
    );
    returned.restore_export_parts(&original).unwrap();

    let mut edited = open(EXCEL);
    edited.sheet_mut(0).unwrap().chart_collection_mut()[0] = returned;
    let package = save(&edited);
    let (_, part) = drawing_part(&package);
    assert!(
        part.contains(
            "<cdr:from><cdr:x>0</cdr:x><cdr:y>0</cdr:y></cdr:from><cdr:to><cdr:x>0.5</cdr:x><cdr:\
             y>0.5</cdr:y></cdr:to>"
        ),
        "{part}"
    );
    assert_eq!(part.matches("<cdr:relSizeAnchor>").count(), 3, "{part}");
}

/// A package whose chart drawing part is replaced with `drawing` and which
/// optionally gets a picture the drawing refers to.
fn with_drawing(drawing: &str, picture: Option<&[u8]>) -> Vec<u8> {
    let (part, _) = drawing_part(EXCEL);
    let mut input = zip::ZipArchive::new(Cursor::new(EXCEL)).unwrap();
    let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default();
    let rels_name = part.replace("drawings/", "drawings/_rels/") + ".rels";
    for index in 0..input.len() {
        let mut file = input.by_index(index).unwrap();
        let name = file.name().unwrap().to_string();
        let mut data = Vec::new();
        file.read_to_end(&mut data).unwrap();
        if name == part {
            data = drawing.as_bytes().to_vec();
        }
        if name == "[Content_Types].xml" && picture.is_some() {
            let xml = String::from_utf8(data).unwrap();
            data = xml
                .replace(
                    "<Default ",
                    "<Default Extension=\"png\" ContentType=\"image/png\"/><Default ",
                )
                .into_bytes();
        }
        output.start_file(name, options).unwrap();
        output.write_all(&data).unwrap();
    }
    if let Some(png) = picture {
        output.start_file(rels_name, options).unwrap();
        output
            .write_all(
                b"<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId7\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/image\" Target=\"../media/image9.png\"/></Relationships>",
            )
            .unwrap();
        output.start_file("xl/media/image9.png", options).unwrap();
        output.write_all(png).unwrap();
    }
    output.finish().unwrap().into_inner()
}

const PNG: &[u8] = include_bytes!("fixtures/chart_drawing_picture.png");

#[test]
fn absolute_anchors_and_pictures_survive() {
    let xml = "<c:userShapes xmlns:c=\"http://schemas.openxmlformats.org/drawingml/2006/chart\" \
               xmlns:cdr=\"http://schemas.openxmlformats.org/drawingml/2006/chartDrawing\" \
               xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" \
               xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\">\
               <cdr:absSizeAnchor><cdr:from><cdr:x>0.5</cdr:x><cdr:y>0.25</cdr:y></cdr:from>\
               <cdr:ext cx=\"914400\" cy=\"457200\"/><cdr:pic macro=\"\"><cdr:nvPicPr>\
               <cdr:cNvPr id=\"2\" name=\"Picture 1\"/><cdr:cNvPicPr/></cdr:nvPicPr>\
               <cdr:blipFill><a:blip r:embed=\"rId7\"/><a:stretch><a:fillRect/></a:stretch>\
               </cdr:blipFill><cdr:spPr><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom>\
               </cdr:spPr></cdr:pic></cdr:absSizeAnchor></c:userShapes>";
    let book = open(&with_drawing(xml, Some(PNG)));
    let anchors = drawing(&book).unwrap().anchors();
    assert_eq!(anchors.len(), 1);
    assert_eq!(
        anchors[0].placement(),
        ChartDrawingPlacement::Absolute {
            from: ChartDrawingPoint::new(0.5, 0.25),
            cx:   914_400,
            cy:   457_200,
        }
    );
    assert!(matches!(
        anchors[0].object(),
        ChartDrawingObject::Preserved(_)
    ));

    let package = save_as("picture", &book);
    let (part, data) = drawing_part(&package);
    assert!(
        data.contains(
            "<cdr:absSizeAnchor><cdr:from><cdr:x>0.5</cdr:x><cdr:y>0.25</cdr:y></cdr:from><cdr:\
             ext cx=\"914400\" cy=\"457200\"/>"
        ),
        "{data}"
    );
    assert!(data.contains("r:embed=\"rId7\""), "{data}");
    let rels_name = part.replace("drawings/", "drawings/_rels/") + ".rels";
    let rels = text(&package, &rels_name);
    assert!(rels.contains("Id=\"rId7\""), "{rels}");
    let target = rels
        .split("Target=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();
    assert_eq!(
        entry(
            &package,
            &format!("xl/{}", target.trim_start_matches("../"))
        )
        .unwrap(),
        PNG
    );
}

#[test]
fn invalid_chart_drawings_are_errors() {
    for (case, body) in [
        (
            "marker outside the chart",
            "<cdr:relSizeAnchor><cdr:from><cdr:x>1.5</cdr:x><cdr:y>0</cdr:y></cdr:from><cdr:\
             to><cdr:x>1</cdr:x><cdr:y>1</cdr:y></cdr:to><cdr:sp \
             macro=\"\"><cdr:nvSpPr><cdr:cNvPr id=\"2\" \
             name=\"a\"/><cdr:cNvSpPr/></cdr:nvSpPr><cdr:spPr/></cdr:sp></cdr:relSizeAnchor>",
        ),
        (
            "missing object",
            "<cdr:relSizeAnchor><cdr:from><cdr:x>0</cdr:x><cdr:y>0</cdr:y></cdr:from><cdr:to><cdr:\
             x>1</cdr:x><cdr:y>1</cdr:y></cdr:to></cdr:relSizeAnchor>",
        ),
        ("foreign anchor", "<cdr:other/>"),
    ] {
        let xml = format!(
            "<c:userShapes xmlns:c=\"http://schemas.openxmlformats.org/drawingml/2006/chart\" xmlns:cdr=\"http://schemas.openxmlformats.org/drawingml/2006/chartDrawing\" xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\">{body}</c:userShapes>"
        );
        let result = reader::xlsx::read_reader(Cursor::new(with_drawing(&xml, None)), true);
        assert!(result.is_err(), "{case} must not import");
    }
}

fn lazy_book(bytes: &[u8], label: &str) -> (Workbook, std::path::PathBuf) {
    let path = std::env::temp_dir().join(format!(
        "umya-chart-drawing-{label}-{}.xlsx",
        std::process::id()
    ));
    std::fs::write(&path, bytes).unwrap();
    (reader::xlsx::lazy_read(&path).unwrap(), path)
}

#[test]
fn the_lazy_reader_reads_and_rolls_back_like_the_eager_one() {
    let (mut book, path) = lazy_book(EXCEL, "ok");
    book.try_read_sheet(0).unwrap();
    let anchors = drawing(&book).unwrap().anchors();
    assert_eq!(anchors.len(), 3);
    assert_eq!(shape_name(anchors[0].object()), "TextBox 1");
    let (_, part) = drawing_part(&save(&book));
    assert_eq!(part.matches("<cdr:relSizeAnchor>").count(), 3, "{part}");
    std::fs::remove_file(path).unwrap();

    let broken = with_drawing(
        "<c:userShapes xmlns:c=\"http://schemas.openxmlformats.org/drawingml/2006/chart\"><c:bad/></c:userShapes>",
        None,
    );
    let (mut book, path) = lazy_book(&broken, "broken");
    for _ in 0..2 {
        assert!(book.try_read_sheet(0).is_err());
    }
    std::fs::remove_file(path).unwrap();
}

#[test]
fn persisted_charts_without_a_drawing_still_load_and_drawings_round_trip() {
    let book = open(EXCEL);
    let chart = book.sheet(0).unwrap().chart_collection()[0].clone();
    let mut value = serde_json::to_value(&chart).unwrap();
    let json = serde_json::to_string(&chart).unwrap();
    assert!(json.contains("\"chart_drawing\""), "field is persisted");
    let again: umya_spreadsheet::Chart = serde_json::from_str(&json).unwrap();
    assert_eq!(
        again.chart_space().chart_drawing().unwrap().anchors().len(),
        3
    );

    // A revision before the field existed has no key at all.
    fn without(value: &mut serde_json::Value) {
        match value {
            serde_json::Value::Object(map) => {
                map.remove("chart_drawing");
                map.remove("chart_drawing_parts");
                map.values_mut().for_each(without);
            }
            serde_json::Value::Array(items) => items.iter_mut().for_each(without),
            _ => (),
        }
    }
    without(&mut value);
    assert!(!value.to_string().contains("chart_drawing"));
    let old: umya_spreadsheet::Chart = serde_json::from_value(value).unwrap();
    assert!(old.chart_space().chart_drawing().is_none());
}
