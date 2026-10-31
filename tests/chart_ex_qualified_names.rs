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
    reader,
    writer,
};

fn save(book: &Workbook) -> Vec<u8> {
    let mut bytes = Cursor::new(Vec::new());
    writer::xlsx::write_writer(book, &mut bytes).unwrap();
    bytes.into_inner()
}

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

fn deletion_refreshes_named_source(formula: &str, name: &str, local: bool) {
    let mut source = umya_spreadsheet::new_file();
    source.set_sheet_name(0, "Target").unwrap();
    source.new_sheet("Host").unwrap();
    source.new_sheet("Unrelated").unwrap();
    for (row, value) in [(1, 10), (2, 20), (3, 30)] {
        source
            .sheet_mut(0)
            .unwrap()
            .cell_mut((2, row))
            .set_value_number(value);
    }
    let mut definition = DefinedName::default();
    definition.set_name(name).set_address("Target!$B$1:$B$3");
    if local {
        definition.set_local_sheet_id(0);
        source.sheet_mut(0).unwrap().add_defined_names(definition);
    } else {
        source.add_defined_names(definition);
    }
    let xml = format!(
        "<cx:chartSpace xmlns:cx=\"http://schemas.microsoft.com/office/drawing/2014/chartex\"><cx:chartData><cx:data id=\"1\"><cx:numDim type=\"val\"><cx:f>{formula}</cx:f></cx:numDim></cx:data></cx:chartData><cx:chart><cx:plotArea><cx:plotAreaRegion><cx:series layoutId=\"waterfall\"><cx:dataId val=\"1\"/></cx:series></cx:plotAreaRegion></cx:plotArea></cx:chart></cx:chartSpace>"
    );
    source
        .sheet_mut(1)
        .unwrap()
        .worksheet_drawing_mut()
        .add_extended_chart(ExtendedChart::new(
            Default::default(),
            ChartExSpace::from_xml(xml.as_bytes()).unwrap(),
        ));
    let input = save(&source);
    let mut charts = Vec::new();
    for eager in [true, false] {
        let mut book = reader::xlsx::read_reader(Cursor::new(&input), eager).unwrap();
        book.remove_row("Target", 2, 1);
        let output = save(&book);
        let chart = part(&output, "xl/charts/chartEx1.xml");
        assert!(
            chart.contains("ptCount=\"2\""),
            "{formula}, eager={eager}: {chart}"
        );
        assert!(chart.contains("<cx:pt idx=\"0\">10</cx:pt>"), "{chart}");
        assert!(chart.contains("<cx:pt idx=\"1\">30</cx:pt>"), "{chart}");
        assert!(!chart.contains(">20</cx:pt>"), "{chart}");
        assert!(chart.contains(&format!(">{formula}</cx:f>")), "{chart}");
        let workbook = part(&output, "xl/workbook.xml");
        assert!(
            workbook.contains(">Target!$B$1:$B$2</definedName>"),
            "{workbook}"
        );
        let reread = reader::xlsx::read_reader(Cursor::new(output), true).unwrap();
        assert_eq!(reread.sheet(0).unwrap().value("B2"), "30");
        if !eager {
            assert!(book.sheet(2).is_err(), "unrelated sheet stays lazy");
        }
        charts.push(chart);
    }
    assert_eq!(charts[0], charts[1]);
}

#[test]
fn sheet_qualified_local_name_refreshes_after_row_deletion() {
    deletion_refreshes_named_source("Target!Sales", "Sales", true);
}

#[test]
fn sheet_qualified_chart_name_refreshes_after_row_deletion() {
    deletion_refreshes_named_source("Target!_xlchart.v1.0", "_xlchart.v1.0", true);
}

#[test]
fn current_workbook_qualified_name_refreshes_after_row_deletion() {
    deletion_refreshes_named_source("[0]!Sales", "Sales", false);
}
