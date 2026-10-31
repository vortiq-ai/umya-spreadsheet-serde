use std::io::{
    Cursor,
    Read,
    Write,
};

use umya_spreadsheet::{
    DefinedName,
    drawing::{
        charts::*,
        spreadsheet::ExtendedChart,
    },
    reader,
    writer,
};

const CX: &str = "http://schemas.microsoft.com/office/drawing/2014/chartex";

fn document(formula: &str, direction: &str) -> String {
    format!(
        r#"<cx:chartSpace xmlns:cx="{CX}"><cx:chartData><cx:data id="0"><cx:numDim type="val"><cx:f dir="{direction}">{formula}</cx:f><cx:lvl ptCount="2"><cx:pt idx="0">10</cx:pt><cx:pt idx="1">20</cx:pt></cx:lvl></cx:numDim></cx:data></cx:chartData><cx:chart><cx:plotArea><cx:plotAreaRegion><cx:series layoutId="waterfall"><cx:dataId val="0"/></cx:series></cx:plotAreaRegion></cx:plotArea></cx:chart></cx:chartSpace>"#
    )
}

fn fixture(formula: &str, direction: &str) -> Vec<u8> {
    let mut book = umya_spreadsheet::new_file();
    for (cell, value) in [("A1", 10), ("A2", 20), ("B1", 30)] {
        book.sheet_mut(0)
            .unwrap()
            .cell_mut(cell)
            .set_value_number(value);
    }
    for name in ["XFE1", "ABCD1", "A", "a1_name", "A1048577", "A0"] {
        let mut definition = DefinedName::default();
        definition.set_name(name).set_address("Sheet1!$A$1:$A$2");
        book.add_defined_names(definition);
    }
    let chart = ChartExSpace::from_xml(document("Sheet1!A1:A2", "col").as_bytes()).unwrap();
    book.sheet_mut(0)
        .unwrap()
        .worksheet_drawing_mut()
        .add_extended_chart(ExtendedChart::new(Default::default(), chart));
    let mut bytes = Vec::new();
    writer::xlsx::write_writer(&book, &mut bytes).unwrap();
    let mut input = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let chart_xml = document(formula, direction);
    let mut replaced = false;
    for index in 0..input.len() {
        let mut file = input.by_index(index).unwrap();
        let name = file.name().unwrap().into_owned();
        let mut data = Vec::new();
        file.read_to_end(&mut data).unwrap();
        if name.starts_with("xl/charts/") && name.ends_with(".xml") {
            data = chart_xml.as_bytes().to_vec();
            replaced = true;
        }
        output
            .start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        output.write_all(&data).unwrap();
    }
    assert!(replaced);
    output.finish().unwrap().into_inner()
}

fn roundtrip(formula: &str, direction: &str, count: u32, expected: &[(u32, f64)]) {
    for eager in [false, true] {
        let mut book =
            reader::xlsx::read_reader(Cursor::new(fixture(formula, direction)), eager).unwrap();
        book.sheet_mut(0)
            .unwrap()
            .worksheet_drawing_mut()
            .extended_charts_mut()[0]
            .chart_space_mut()
            .chart_mut()
            .unwrap()
            .series_mut()[0]
            .set_hidden(Some(true));
        let mut bytes = Vec::new();
        writer::xlsx::write_writer(&book, &mut bytes)
            .unwrap_or_else(|error| panic!("{formula}, eager={eager}: {error}"));
        let mut archive = zip::ZipArchive::new(Cursor::new(&bytes)).unwrap();
        let chart_name = archive
            .file_names()
            .map(Result::unwrap)
            .find(|name| name.starts_with("xl/charts/") && name.ends_with(".xml"))
            .unwrap()
            .into_owned();
        let mut xml = String::new();
        archive
            .by_name(&chart_name)
            .unwrap()
            .read_to_string(&mut xml)
            .unwrap();
        let exported = ChartExSpace::from_xml(xml.as_bytes()).unwrap();
        let ChartExDimension::Numeric(dimension) = &exported.data().unwrap().data[0].dimensions[0]
        else {
            panic!("numeric dimension")
        };
        let ChartExDimensionSource::Reference(reference) = &dimension.source else {
            panic!("reference dimension")
        };
        assert_eq!(reference.formula().text(), formula);
        assert!(xml.contains(&format!(r#"ptCount="{count}""#)), "{xml}");
        for (index, value) in expected {
            assert!(
                xml.contains(&format!(r#"<cx:pt idx="{index}">{value}</cx:pt>"#)),
                "{xml}"
            );
        }
        assert_eq!(xml.matches("<cx:pt ").count(), expected.len());
        drop(archive);
        let reread = reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap();
        assert_eq!(
            reread
                .sheet(0)
                .unwrap()
                .worksheet_drawing()
                .extended_charts()[0]
                .chart_space()
                .chart()
                .unwrap()
                .series()[0]
                .hidden,
            Some(true)
        );
    }
}

#[test]
fn lowercase_and_anchored_cells_export_like_uppercase_references() {
    for formula in ["Sheet1!A1:A2", "Sheet1!a1:a2", "sHeEt1!$a$1:a$2"] {
        roundtrip(formula, "col", 2, &[(0, 10.0), (1, 20.0)]);
    }
}

#[test]
fn whole_columns_export_the_full_axis_and_current_numeric_points() {
    roundtrip("Sheet1!$a:$a", "col", 1_048_576, &[(0, 10.0), (1, 20.0)]);
}

#[test]
fn whole_rows_export_the_full_axis_and_current_numeric_points() {
    roundtrip("Sheet1!$1:$1", "row", 16_384, &[(0, 10.0), (1, 30.0)]);
}

#[test]
fn real_names_are_not_misclassified_as_cell_or_axis_references() {
    for name in ["XFE1", "ABCD1", "A", "a1_name", "A1048577", "A0"] {
        roundtrip(name, "col", 2, &[(0, 10.0), (1, 20.0)]);
    }
}

#[test]
fn malformed_or_out_of_grid_anchored_sources_fail_explicitly() {
    for formula in [
        "Sheet1!$a$0",
        "Sheet1!$xFe$1",
        "Sheet1!a1:a99999999999999999999",
        "Sheet1!a1:b",
        "Sheet1!$a:$$a",
    ] {
        let mut book =
            reader::xlsx::read_reader(Cursor::new(fixture(formula, "col")), true).unwrap();
        book.sheet_mut(0)
            .unwrap()
            .worksheet_drawing_mut()
            .extended_charts_mut()[0]
            .chart_space_mut()
            .chart_mut()
            .unwrap()
            .series_mut()[0]
            .set_hidden(Some(true));
        let error = writer::xlsx::write_writer(&book, &mut Vec::new()).unwrap_err();
        assert!(
            error.to_string().contains("requires a resolver"),
            "{formula}: {error}"
        );
    }
}
