use std::io::{
    Cursor,
    Read,
    Write,
};

use umya_spreadsheet::{
    Workbook,
    drawing::charts::{
        ChartDrawingPlacement,
        ChartDrawingPoint,
    },
    reader,
    writer,
};

const EXCEL: &[u8] = include_bytes!("fixtures/chart_drawing_excel.xlsx");
const RELATIONSHIPS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

fn input(user_shapes: &str, inherited: bool) -> Vec<u8> {
    let mut source = zip::ZipArchive::new(Cursor::new(EXCEL)).unwrap();
    let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for index in 0..source.len() {
        let mut member = source.by_index(index).unwrap();
        let name = member.name().unwrap().into_owned();
        let mut data = Vec::new();
        member.read_to_end(&mut data).unwrap();
        if name == "xl/charts/chart1.xml" {
            let mut xml = String::from_utf8(data).unwrap();
            assert!(xml.contains("<c:userShapes r:id=\"rId1\"/>"));
            xml = xml.replace("<c:userShapes r:id=\"rId1\"/>", user_shapes);
            if inherited {
                xml = xml.replace(
                    "<c:chartSpace ",
                    &format!("<c:chartSpace xmlns:rel=\"{RELATIONSHIPS}\" "),
                );
            }
            data = xml.into_bytes();
        }
        output
            .start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        output.write_all(&data).unwrap();
    }
    output.finish().unwrap().into_inner()
}

fn drawing(book: &Workbook) -> &umya_spreadsheet::drawing::charts::ChartDrawing {
    book.sheet(0).unwrap().chart_collection()[0]
        .chart_space()
        .chart_drawing()
        .unwrap()
}

#[test]
fn chart_drawing_relationship_ids_follow_namespace_bindings() {
    for inherited in [true, false] {
        for empty in [true, false] {
            let declaration = if inherited {
                String::new()
            } else {
                format!(" xmlns:rel=\"{RELATIONSHIPS}\"")
            };
            let suffix = if empty { "/>" } else { "></c:userShapes>" };
            let bytes = input(
                &format!("<c:userShapes{declaration} rel:id=\"rId1\"{suffix}"),
                inherited,
            );
            let path = std::env::temp_dir().join(format!(
                "umya-chart-drawing-namespace-{}-{inherited}-{empty}.xlsx",
                std::process::id()
            ));
            std::fs::write(&path, &bytes).unwrap();
            for lazy in [false, true] {
                let mut book = if lazy {
                    let mut book = reader::xlsx::lazy_read(&path).unwrap();
                    book.try_read_sheet(0).unwrap();
                    book
                } else {
                    reader::xlsx::read_reader(Cursor::new(&bytes), true).unwrap()
                };
                assert_eq!(drawing(&book).anchors().len(), 3);
                assert_eq!(
                    drawing(&book).anchors()[0]
                        .shape()
                        .unwrap()
                        .text_body()
                        .unwrap()
                        .paragraph()[0]
                        .run()[0]
                        .text(),
                    "Hello chart"
                );
                let placement = ChartDrawingPlacement::Relative {
                    from: ChartDrawingPoint::new(0.25, 0.5),
                    to:   ChartDrawingPoint::new(0.75, 0.625),
                };
                book.sheet_mut(0).unwrap().chart_collection_mut()[0]
                    .chart_space_mut()
                    .chart_drawing_mut()
                    .anchors_mut()[0]
                    .set_placement(placement);
                for light in [false, true] {
                    let mut output = Vec::new();
                    if light {
                        writer::xlsx::write_writer_light(&book, &mut output).unwrap();
                    } else {
                        writer::xlsx::write_writer(&book, &mut output).unwrap();
                    }
                    let mut archive = zip::ZipArchive::new(Cursor::new(&output)).unwrap();
                    let mut chart_xml = String::new();
                    archive
                        .by_name("xl/charts/chart1.xml")
                        .unwrap()
                        .read_to_string(&mut chart_xml)
                        .unwrap();
                    assert!(
                        chart_xml.contains("<c:userShapes r:id=\"rId1\"/>"),
                        "{chart_xml}"
                    );
                    drop(archive);
                    let reread = reader::xlsx::read_reader(Cursor::new(output), true).unwrap();
                    assert_eq!(drawing(&reread).anchors().len(), 3);
                    assert_eq!(drawing(&reread).anchors()[0].placement(), placement);
                }
            }
            std::fs::remove_file(path).unwrap();
        }
    }
}

#[test]
fn a_foreign_id_attribute_cannot_bind_a_chart_drawing() {
    for suffix in ["/>", "></c:userShapes>"] {
        let bytes = input(
            &format!("<c:userShapes xmlns:r=\"urn:foreign\" r:id=\"rId1\"{suffix}"),
            false,
        );
        let error = reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap_err();
        assert_eq!(
            error.to_string(),
            "XML error: xl/charts/chart1.xml: c:userShapes requires a non-empty r:id"
        );
    }
}
