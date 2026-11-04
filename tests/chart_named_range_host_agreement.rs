//! A sheet-qualified defined name used as a chart source exports the same
//! chart part whether the workbook was read eagerly or lazily.
#![cfg(feature = "serde")]

use std::io::{
    Cursor,
    Read,
};

use umya_spreadsheet::{
    DefinedName,
    Workbook,
    drawing::{
        charts::ChartExSpace,
        spreadsheet::ExtendedChart,
    },
};

#[path = "support/chart_xml_fixture.rs"]
pub mod fixture;

fn part(bytes: &[u8], path: &str) -> String {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut text = String::new();
    archive
        .by_name(path)
        .unwrap()
        .read_to_string(&mut text)
        .unwrap();
    text
}

fn source(extended: bool) -> Vec<u8> {
    let mut book = umya_spreadsheet::new_file();
    book.set_sheet_name(0, "Host").unwrap();
    book.new_sheet("Data").unwrap();
    book.new_sheet("Unrelated").unwrap();
    for (row, value) in [(2, 10), (3, 20), (4, 30), (5, 40)] {
        book.sheet_mut(1)
            .unwrap()
            .cell_mut((2, row))
            .set_value_number(value);
    }
    let mut name = DefinedName::default();
    name.set_name("Sales")
        .set_address("Data!$B$2:$B$5")
        .set_local_sheet_id(1);
    book.sheet_mut(1).unwrap().add_defined_names(name);
    if extended {
        let xml = "<cx:chartSpace xmlns:cx=\"http://schemas.microsoft.com/office/drawing/2014/chartex\"><cx:chartData><cx:data id=\"1\"><cx:numDim type=\"val\"><cx:f>Data!Sales</cx:f></cx:numDim></cx:data></cx:chartData><cx:chart><cx:plotArea><cx:plotAreaRegion><cx:series layoutId=\"waterfall\"><cx:dataId val=\"1\"/></cx:series></cx:plotAreaRegion></cx:plotArea></cx:chart></cx:chartSpace>";
        book.sheet_mut(0)
            .unwrap()
            .worksheet_drawing_mut()
            .add_extended_chart(ExtendedChart::new(
                Default::default(),
                ChartExSpace::from_xml(xml.as_bytes()).unwrap(),
            ));
    } else {
        let xml = "<c:chartSpace xmlns:c=\"http://schemas.openxmlformats.org/drawingml/2006/chart\"><c:chart><c:plotArea><c:lineChart><c:grouping val=\"standard\"/><c:ser><c:idx val=\"0\"/><c:order val=\"0\"/><c:val><c:numRef><c:f>Data!Sales</c:f><c:numCache><c:formatCode>General</c:formatCode><c:ptCount val=\"4\"/><c:pt idx=\"0\"><c:v>10</c:v></c:pt><c:pt idx=\"1\"><c:v>20</c:v></c:pt><c:pt idx=\"2\"><c:v>30</c:v></c:pt><c:pt idx=\"3\"><c:v>40</c:v></c:pt></c:numCache></c:numRef></c:val></c:ser><c:axId val=\"1\"/><c:axId val=\"2\"/></c:lineChart><c:catAx><c:axId val=\"1\"/><c:scaling><c:orientation val=\"minMax\"/></c:scaling><c:axPos val=\"b\"/><c:crossAx val=\"2\"/></c:catAx><c:valAx><c:axId val=\"2\"/><c:scaling><c:orientation val=\"minMax\"/></c:scaling><c:axPos val=\"l\"/><c:crossAx val=\"1\"/></c:valAx></c:plotArea></c:chart></c:chartSpace>";
        let mut chart = fixture::imported_chart(xml);
        chart
            .two_cell_anchor_mut()
            .from_marker_mut()
            .set_coordinate("T20");
        chart
            .two_cell_anchor_mut()
            .to_marker_mut()
            .set_coordinate("AB40");
        book.sheet_mut(0).unwrap().add_chart(chart);
    }
    fixture::write_workbook(&book)
}

fn edited(bytes: &[u8], eager: bool, edit: impl Fn(&mut Workbook)) -> Vec<u8> {
    let mut book = umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(bytes), eager).unwrap();
    edit(&mut book);
    if !eager {
        assert!(book.sheet(2).is_err(), "unrelated sheet stays lazy");
    }
    fixture::write_workbook(&book)
}

fn assert_eager_equals_lazy(extended: bool, path: &str, edit: impl Fn(&mut Workbook)) -> String {
    let bytes = source(extended);
    let eager = part(&edited(&bytes, true, &edit), path);
    let lazy = part(&edited(&bytes, false, &edit), path);
    assert_eq!(eager, lazy);
    eager
}

#[test]
fn standard_chart_named_range_exports_identically_after_row_insert_and_delete() {
    let inserted = assert_eager_equals_lazy(false, "xl/charts/chart1.xml", |book| {
        book.insert_new_row("Data", 3, 1);
    });
    let deleted = assert_eager_equals_lazy(false, "xl/charts/chart1.xml", |book| {
        book.remove_row("Data", 3, 1);
    });
    for chart in [inserted, deleted] {
        assert!(chart.contains("<c:f>Data!Sales</c:f>"), "{chart}");
    }
}

#[test]
fn extended_chart_named_range_exports_identically_after_row_insert_and_delete() {
    let inserted = assert_eager_equals_lazy(true, "xl/charts/chartEx1.xml", |book| {
        book.insert_new_row("Data", 3, 1);
    });
    let deleted = assert_eager_equals_lazy(true, "xl/charts/chartEx1.xml", |book| {
        book.remove_row("Data", 3, 1);
    });
    assert!(
        inserted.contains("ptCount=\"5\"")
            && inserted.contains("<cx:pt idx=\"2\">20</cx:pt>")
            && !inserted.contains("idx=\"1\""),
        "{inserted}"
    );
    assert!(
        deleted.contains("ptCount=\"3\"")
            && deleted.contains("<cx:pt idx=\"1\">30</cx:pt>")
            && !deleted.contains(">20</cx:pt>"),
        "{deleted}"
    );
}

#[test]
fn only_an_extended_chart_host_opens_for_an_edit_through_a_qualified_name() {
    for (extended, host_opens) in [(false, false), (true, true)] {
        let bytes = source(extended);
        let mut book =
            umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(&bytes), false).unwrap();
        book.insert_new_row("Data", 3, 1);
        assert_eq!(
            book.sheet(0).is_ok(),
            host_opens,
            "extended={extended}: the chart host sheet opens only when its output depends on the \
             edit"
        );
        assert!(book.sheet(2).is_err(), "unrelated sheet stays lazy");
    }
}
