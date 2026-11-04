#![cfg(feature = "serde")]
//! Errors from reading a chart part name the part they came from.
use std::io::{
    Cursor,
    Read,
    Write,
};

use umya_spreadsheet::{
    XlsxErrorKind,
    drawing::{
        charts::{
            ChartExBuilder,
            ChartExFormula,
        },
        spreadsheet::{
            DrawingAnchor,
            ExtendedChart,
        },
    },
    reader,
    writer,
};

#[path = "support/chart_xml_fixture.rs"]
pub mod fixture;

fn rewrite_part(bytes: &[u8], name: &str, edit: impl FnOnce(String) -> String) -> Vec<u8> {
    let mut input = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let mut edit = Some(edit);
    for index in 0..input.len() {
        let mut entry = input.by_index(index).unwrap();
        let mut data = Vec::new();
        entry.read_to_end(&mut data).unwrap();
        if entry.name().unwrap() == name {
            data = edit.take().unwrap()(String::from_utf8(data).unwrap()).into_bytes();
        }
        output
            .start_file(
                entry.name().unwrap(),
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
        output.write_all(&data).unwrap();
    }
    assert!(edit.is_none(), "{name} was not rewritten");
    output.finish().unwrap().into_inner()
}

#[test]
fn an_unresolvable_chart_ex_data_id_names_the_part() {
    let mut book = umya_spreadsheet::new_file();
    book.sheet_mut(0).unwrap().cell_mut((1, 1)).set_value("a");
    book.sheet_mut(0)
        .unwrap()
        .cell_mut((2, 1))
        .set_value_number(1);
    let chart = ChartExBuilder::funnel(
        ChartExFormula::new("Sheet1!$A$1:$A$1"),
        ChartExFormula::new("Sheet1!$B$1:$B$1"),
    )
    .build()
    .unwrap();
    book.sheet_mut(0)
        .unwrap()
        .worksheet_drawing_mut()
        .add_extended_chart(ExtendedChart::new(DrawingAnchor::default(), chart));
    let mut bytes = Vec::new();
    writer::xlsx::write_writer(&book, &mut bytes).unwrap();
    let broken = rewrite_part(&bytes, "xl/charts/chartEx1.xml", |xml| {
        let start = xml.find("<cx:dataId val=\"").unwrap() + "<cx:dataId val=\"".len();
        let end = start + xml[start..].find('"').unwrap();
        format!("{}999{}", &xml[..start], &xml[end..])
    });
    let error = reader::xlsx::read_reader(Cursor::new(broken), true).unwrap_err();
    assert_eq!(error.kind(), XlsxErrorKind::Xml);
    assert!(
        error.to_string().contains("xl/charts/chartEx1.xml"),
        "{error}"
    );
}

#[test]
fn an_invalid_standard_chart_root_names_the_part() {
    let mut book = umya_spreadsheet::new_file();
    book.sheet_mut(0).unwrap().add_chart(fixture::base_chart());
    let broken = fixture::replace_chart_part(
        &fixture::write_workbook(&book),
        "<c:notAChart xmlns:c=\"http://schemas.openxmlformats.org/drawingml/2006/chart\"></c:notAChart>",
    );
    let error = reader::xlsx::read_reader(Cursor::new(broken), true).unwrap_err();
    assert_eq!(error.kind(), XlsxErrorKind::Xml);
    assert!(
        error.to_string().contains("xl/charts/chart1.xml"),
        "{error}"
    );
}

#[test]
fn a_missing_part_in_a_lazy_read_names_the_part_once() {
    let mut book = umya_spreadsheet::new_file();
    book.sheet_mut(0).unwrap().add_chart(fixture::base_chart());
    let bytes = fixture::write_workbook(&book);
    let mut input = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for index in 0..input.len() {
        let mut entry = input.by_index(index).unwrap();
        let name = entry.name().unwrap().into_owned();
        if name == "xl/charts/chart1.xml" {
            continue;
        }
        let mut data = Vec::new();
        entry.read_to_end(&mut data).unwrap();
        output
            .start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        output.write_all(&data).unwrap();
    }
    let broken = output.finish().unwrap().into_inner();
    let mut lazy = reader::xlsx::read_reader(Cursor::new(broken), false).unwrap();
    let error = lazy.try_read_sheet(0).unwrap_err();
    assert_eq!(error.kind(), XlsxErrorKind::NotFound);
    assert_eq!(
        error.to_string().matches("xl/charts/chart1.xml").count(),
        1,
        "{error}"
    );
}

#[test]
fn a_data_id_naming_another_part_still_gets_the_part_prefix() {
    let mut book = umya_spreadsheet::new_file();
    book.sheet_mut(0).unwrap().cell_mut((1, 1)).set_value("a");
    book.sheet_mut(0)
        .unwrap()
        .cell_mut((2, 1))
        .set_value_number(1);
    let chart = ChartExBuilder::funnel(
        ChartExFormula::new("Sheet1!$A$1:$A$1"),
        ChartExFormula::new("Sheet1!$B$1:$B$1"),
    )
    .build()
    .unwrap();
    book.sheet_mut(0)
        .unwrap()
        .worksheet_drawing_mut()
        .add_extended_chart(ExtendedChart::new(DrawingAnchor::default(), chart));
    let mut bytes = Vec::new();
    writer::xlsx::write_writer(&book, &mut bytes).unwrap();
    let broken = rewrite_part(&bytes, "xl/charts/chartEx1.xml", |xml| {
        let start = xml.find("<cx:dataId val=\"").unwrap() + "<cx:dataId val=\"".len();
        let end = start + xml[start..].find('"').unwrap();
        format!(
            "{}xl/charts/chartEx1.xml.rels{}",
            &xml[..start],
            &xml[end..]
        )
    });
    let error = reader::xlsx::read_reader(Cursor::new(broken), true).unwrap_err();
    assert_eq!(error.kind(), XlsxErrorKind::Xml);
    assert_eq!(
        error.message(),
        Some(
            "xl/charts/chartEx1.xml: invalid ChartEx unsigned integer: xl/charts/chartEx1.xml.rels"
        )
    );
}
