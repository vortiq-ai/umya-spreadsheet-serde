//! `<c:f>` carries a formula as XML text, so a sheet name or literal label
//! containing `&`, `<` or `>` has to be escaped on write. The reader already
//! unescapes what it reads, and the sibling formula nodes `<f>` and
//! `<definedName>` already escape on write; `<c:f>` used not to, which produced
//! a chart part Excel refuses to open.
use std::io::{
    Cursor,
    Read,
};

use umya_spreadsheet::{
    Chart,
    ChartType,
    drawing,
};

fn chart_part(bytes: &[u8]) -> String {
    let mut archive = ::zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut xml = String::new();
    archive
        .by_name("xl/charts/chart1.xml")
        .unwrap()
        .read_to_string(&mut xml)
        .unwrap();
    xml
}

#[test]
fn chart_reference_is_escaped_exactly_once() {
    let mut book = umya_spreadsheet::new_file();
    book.new_sheet("A & B").unwrap();
    let sheet = book.sheet_by_name_mut("A & B").unwrap();
    sheet.get_cell_mut("A1").set_value_number(7);
    sheet.get_cell_mut("A2").set_value_number(13);

    let mut from_marker = drawing::spreadsheet::MarkerType::default();
    let mut to_marker = drawing::spreadsheet::MarkerType::default();
    from_marker.set_coordinate("C1");
    to_marker.set_coordinate("D5");
    let mut chart = Chart::default();
    chart.new_chart(
        &ChartType::LineChart,
        from_marker,
        to_marker,
        vec!["'A & B'!$A$1:$A$2"],
    );
    book.sheet_by_name_mut("A & B").unwrap().add_chart(chart);

    let mut buf = Cursor::new(Vec::new());
    umya_spreadsheet::writer::xlsx::write_writer(&book, &mut buf).unwrap();
    let bytes = buf.into_inner();
    let xml = chart_part(&bytes);

    let bodies: Vec<&str> = xml
        .split("<c:f>")
        .skip(1)
        .map(|rest| rest.split_once("</c:f>").unwrap().0)
        .collect();
    assert!(!bodies.is_empty(), "the chart wrote no <c:f> at all");
    assert!(
        bodies.iter().any(|body| body.contains("&amp;")),
        "the ampersand in the sheet name was written raw: {bodies:?}"
    );
    assert!(
        !xml.contains("&amp;amp;"),
        "the reference was escaped twice: {bodies:?}"
    );

    // The round trip must give the address back exactly as it went in.
    let read = umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(&bytes), true).unwrap();
    let mut chart = read.sheet_by_name("A & B").unwrap().chart_collection()[0].clone();
    let decoded = chart
        .plot_area_mut()
        .line_chart()
        .unwrap()
        .area_chart_series_list()
        .area_chart_series()[0]
        .values()
        .unwrap()
        .number_reference()
        .formula()
        .address_str()
        .to_owned();
    assert_eq!(decoded, "'A & B'!$A$1:$A$2");
}
