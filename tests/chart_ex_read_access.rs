use std::io::Cursor;

use umya_spreadsheet::{
    drawing::{
        charts::*,
        spreadsheet::ExtendedChart,
    },
    reader,
    writer,
};

const CX: &str = "http://schemas.microsoft.com/office/drawing/2014/chartex";

fn document(layout: &str, title: &str, name: &str, dimensions: &str) -> ChartExSpace {
    ChartExSpace::from_xml(format!(
        r#"<cx:chartSpace xmlns:cx="{CX}" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><cx:chartData><cx:data id="7">{dimensions}</cx:data></cx:chartData><cx:chart><cx:title><cx:tx>{title}</cx:tx></cx:title><cx:plotArea><cx:plotAreaRegion><cx:series layoutId="{layout}"><cx:tx>{name}</cx:tx><cx:dataId val="7"/></cx:series></cx:plotAreaRegion></cx:plotArea></cx:chart></cx:chartSpace>"#
    ).as_bytes()).unwrap()
}

fn saved_book(chart: ChartExSpace) -> Vec<u8> {
    let mut book = umya_spreadsheet::new_file();
    let sheet = book.sheet_mut(0).unwrap();
    for (cell, text) in [
        ("A1", "North"),
        ("A2", "South"),
        ("B1", "Red"),
        ("B2", "Blue"),
        ("D1", "Revenue"),
        ("E1", "Quarterly"),
    ] {
        sheet.cell_mut(cell).set_value(text);
    }
    sheet.cell_mut("C1").set_value_number(12);
    sheet.cell_mut("C2").set_value_number(25);
    sheet
        .worksheet_drawing_mut()
        .add_extended_chart(ExtendedChart::new(Default::default(), chart));
    let mut bytes = Vec::new();
    writer::xlsx::write_writer(&book, &mut bytes).unwrap();
    bytes
}

#[test]
fn every_layout_resolves_current_hierarchy_values_title_and_series_name() {
    for layout in [
        "sunburst",
        "treemap",
        "waterfall",
        "clusteredColumn",
        "paretoLine",
        "boxWhisker",
        "funnel",
        "regionMap",
    ] {
        for eager in [false, true] {
            let chart = document(
                layout,
                "<cx:txData><cx:f>Sheet1!E1</cx:f></cx:txData>",
                "<cx:txData><cx:f>Sheet1!D1</cx:f></cx:txData>",
                "<cx:strDim type=\"cat\"><cx:f \
                 dir=\"col\">Sheet1!A1:B2</cx:f></cx:strDim><cx:numDim \
                 type=\"val\"><cx:f>Sheet1!C1:C2</cx:f></cx:numDim>",
            );
            let mut book =
                reader::xlsx::read_reader(Cursor::new(saved_book(chart)), eager).unwrap();
            book.sheet_mut(0)
                .unwrap()
                .cell_mut("C2")
                .set_value_number(40);
            book.sheet_mut(0)
                .unwrap()
                .cell_mut("E1")
                .set_value("Updated title");
            let mut bytes = Vec::new();
            writer::xlsx::write_writer(&book, &mut bytes).unwrap();
            let book = reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap();
            let chart =
                book.sheet(0).unwrap().worksheet_drawing().extended_charts()[0].chart_space();
            let series = &chart.chart().unwrap().series()[0];
            let mut resolver = WorkbookChartExResolver::new(&book);
            assert_eq!(
                chart
                    .title_text("Sheet1", &mut resolver)
                    .unwrap()
                    .as_deref(),
                Some("Updated title")
            );
            assert_eq!(
                series
                    .name_text("Sheet1", &mut resolver)
                    .unwrap()
                    .as_deref(),
                Some("Revenue")
            );
            let data = chart
                .resolved_series_data(series, "Sheet1", &mut resolver)
                .unwrap()
                .unwrap();
            assert_eq!(data.id, 7);
            let ChartExDimension::String(strings) = &data.dimensions[0] else {
                panic!("category")
            };
            let ChartExDimensionSource::Literal(levels) = &strings.source else {
                panic!("resolved categories")
            };
            assert_eq!(levels.len(), 2);
            assert_eq!(
                levels[0]
                    .points
                    .iter()
                    .map(|p| p.value.as_str())
                    .collect::<Vec<_>>(),
                ["North", "South"]
            );
            assert_eq!(
                levels[1]
                    .points
                    .iter()
                    .map(|p| p.value.as_str())
                    .collect::<Vec<_>>(),
                ["Red", "Blue"]
            );
            let ChartExDimension::Numeric(numbers) = &data.dimensions[1] else {
                panic!("values")
            };
            let ChartExDimensionSource::Literal(levels) = &numbers.source else {
                panic!("resolved values")
            };
            assert_eq!(levels[0].point_count, 2);
            assert_eq!(
                levels[0]
                    .points
                    .iter()
                    .map(|p| p.value.value())
                    .collect::<Vec<_>>(),
                [Some(12.0), Some(40.0)]
            );
        }
    }
}

#[test]
fn rich_text_preserves_fields_breaks_and_paragraphs_and_literal_data_stays_sparse() {
    let rich = r#"<cx:rich><a:bodyPr/><a:p><a:r><a:t>A</a:t></a:r><a:br/><a:fld id="{01234567-89AB-CDEF-0123-456789ABCDEF}"><a:t>4</a:t></a:fld><a:br/><a:r><a:t>B</a:t></a:r></a:p><a:p><a:r><a:t>Second</a:t></a:r></a:p></cx:rich>"#;
    let chart = document(
        "sunburst",
        rich,
        "<cx:txData><cx:v>Series one</cx:v></cx:txData>",
        "<cx:strDim type=\"cat\"><cx:lvl ptCount=\"4\"><cx:pt \
         idx=\"3\">Last</cx:pt></cx:lvl></cx:strDim><cx:numDim type=\"val\"><cx:lvl \
         ptCount=\"4\"><cx:pt idx=\"3\">8.5</cx:pt></cx:lvl></cx:numDim>",
    );
    let book = reader::xlsx::read_reader(Cursor::new(saved_book(chart)), true).unwrap();
    let chart = book.sheet(0).unwrap().worksheet_drawing().extended_charts()[0].chart_space();
    let mut resolver = WorkbookChartExResolver::new(&book);
    assert_eq!(
        chart
            .title_text("Sheet1", &mut resolver)
            .unwrap()
            .as_deref(),
        Some("A\n4\nB\nSecond")
    );
    let series = &chart.chart().unwrap().series()[0];
    assert_eq!(
        series
            .name_text("Sheet1", &mut resolver)
            .unwrap()
            .as_deref(),
        Some("Series one")
    );
    let data = chart
        .resolved_series_data(series, "Sheet1", &mut resolver)
        .unwrap()
        .unwrap();
    let ChartExDimension::String(dimension) = &data.dimensions[0] else {
        panic!("category")
    };
    let ChartExDimensionSource::Literal(levels) = &dimension.source else {
        panic!("literal")
    };
    assert_eq!(levels[0].point_count, 4);
    assert_eq!(levels[0].points.len(), 1);
    assert_eq!(levels[0].points[0].index, 3);
}

#[test]
fn unresolved_or_ambiguous_series_data_is_an_explicit_error() {
    let chart = document(
        "waterfall",
        "<cx:txData><cx:v>Title</cx:v></cx:txData>",
        "<cx:txData><cx:v>Name</cx:v></cx:txData>",
        "<cx:numDim type=\"val\"><cx:lvl ptCount=\"0\"/></cx:numDim>",
    );
    let mut book = reader::xlsx::read_reader(Cursor::new(saved_book(chart)), true).unwrap();
    let mut chart = book
        .sheet_mut(0)
        .unwrap()
        .worksheet_drawing_mut()
        .extended_charts_mut()[0]
        .chart_space()
        .clone();
    let mut resolver = WorkbookChartExResolver::new(&book);
    let mut series = chart.chart().unwrap().series()[0].clone();
    series.set_data_id(Some(90));
    assert!(
        chart
            .resolved_series_data(&series, "Sheet1", &mut resolver)
            .unwrap_err()
            .contains("unresolved")
    );
    series.set_data_id(Some(7));
    let duplicate = chart.data().unwrap().data[0].clone();
    chart.data_mut().unwrap().data.push(duplicate);
    assert!(
        chart
            .resolved_series_data(&series, "Sheet1", &mut resolver)
            .unwrap_err()
            .contains("ambiguous")
    );
    series.set_data_id(None);
    assert!(
        chart
            .resolved_series_data(&series, "Sheet1", &mut resolver)
            .unwrap()
            .is_none()
    );
}

#[test]
fn a_reference_broken_by_an_edit_is_an_error_not_empty_data() {
    let book = umya_spreadsheet::new_file();
    let broken = ChartExBuilder::funnel(
        ChartExFormula::new("Sheet1!#REF!"),
        ChartExFormula::new("Sheet1!$C$1:$C$4"),
    )
    .build()
    .unwrap();
    let series = &broken.chart().unwrap().series()[0];
    let mut resolver = WorkbookChartExResolver::new(&book);
    let error = broken
        .resolved_series_data(series, "Sheet1", &mut resolver)
        .unwrap_err();
    assert!(error.contains("#REF!"), "{error}");
    let text = ChartExText {
        tx_data: Some(ChartExTextData::reference(ChartExFormula::new(
            "Sheet1!#REF!",
        ))),
        rich:    None,
    };
    assert!(
        text.resolved_text("Sheet1", &mut resolver)
            .unwrap_err()
            .contains("#REF!")
    );
}

fn level_values(data: &ChartExData) -> (Vec<String>, Vec<f64>) {
    let ChartExDimension::String(strings) = &data.dimensions[0] else {
        panic!("category")
    };
    let ChartExDimensionSource::Literal(levels) = &strings.source else {
        panic!("resolved categories")
    };
    let categories = levels[0].points.iter().map(|p| p.value.clone()).collect();
    let ChartExDimension::Numeric(numbers) = &data.dimensions[1] else {
        panic!("values")
    };
    let ChartExDimensionSource::Literal(levels) = &numbers.source else {
        panic!("resolved values")
    };
    let values = levels[0]
        .points
        .iter()
        .map(|p| p.value.value().unwrap())
        .collect();
    (categories, values)
}

#[test]
fn a_sheet_named_like_the_error_value_is_a_valid_source() {
    let mut book = umya_spreadsheet::new_file();
    book.new_sheet("#REF!").unwrap();
    let sheet = book.sheet_mut(1).unwrap();
    for (row, (label, number)) in [("North", 12), ("South", 25), ("East", 7)]
        .into_iter()
        .enumerate()
    {
        let row = u32::try_from(row).unwrap() + 1;
        sheet.cell_mut((1, row)).set_value(label);
        sheet.cell_mut((3, row)).set_value_number(number);
    }
    let chart = ChartExBuilder::funnel(
        ChartExFormula::new("'#REF!'!$A$1:$A$3"),
        ChartExFormula::new("'#REF!'!$C$1:$C$3"),
    )
    .build()
    .unwrap();
    let series = &chart.chart().unwrap().series()[0];
    let mut resolver = WorkbookChartExResolver::new(&book);
    let data = chart
        .resolved_series_data(series, "Sheet1", &mut resolver)
        .unwrap()
        .unwrap();
    assert_eq!(
        level_values(&data),
        (
            vec!["North".to_owned(), "South".to_owned(), "East".to_owned()],
            vec![12.0, 25.0, 7.0]
        )
    );
    let text = ChartExText {
        tx_data: Some(ChartExTextData::reference(ChartExFormula::new(
            "'#REF!'!$A$2",
        ))),
        rich:    None,
    };
    assert_eq!(
        text.resolved_text("Sheet1", &mut resolver).unwrap(),
        Some("South".to_owned())
    );
}

#[test]
fn a_defined_name_that_resolves_to_the_error_value_is_an_invalid_reference() {
    let mut book = umya_spreadsheet::new_file();
    let mut broken = umya_spreadsheet::DefinedName::default();
    broken.set_name("_xlchart.v1.0").set_address("Sheet1!#REF!");
    book.add_defined_names(broken);
    let mut valid = umya_spreadsheet::DefinedName::default();
    valid.set_name("_xlchart.v1.1").set_address("Sheet1!$A$1");
    book.add_defined_names(valid);
    book.sheet_mut(0).unwrap().cell_mut("A1").set_value("Alpha");
    book.sheet_mut(0)
        .unwrap()
        .cell_mut("C1")
        .set_value_number(3);
    let mut resolver = WorkbookChartExResolver::new(&book);
    // Data reached through the broken name.
    let chart = ChartExBuilder::funnel(
        ChartExFormula::new("_xlchart.v1.0"),
        ChartExFormula::new("Sheet1!$C$1:$C$1"),
    )
    .build()
    .unwrap();
    let series = &chart.chart().unwrap().series()[0];
    let error = chart
        .resolved_series_data(series, "Sheet1", &mut resolver)
        .unwrap_err();
    assert!(
        error.starts_with("ChartEx source reference is invalid"),
        "{error}"
    );
    // Text reached through the broken name, and the same through a valid one.
    let text = |name: &str| ChartExText {
        tx_data: Some(ChartExTextData::reference(ChartExFormula::new(name))),
        rich:    None,
    };
    let error = text("_xlchart.v1.0")
        .resolved_text("Sheet1", &mut resolver)
        .unwrap_err();
    assert!(
        error.starts_with("ChartEx source reference is invalid"),
        "{error}"
    );
    assert_eq!(
        text("_xlchart.v1.1")
            .resolved_text("Sheet1", &mut resolver)
            .unwrap(),
        Some("Alpha".to_owned())
    );
    // A series name and the title take the same route.
    let mut named = chart.clone();
    named.chart_mut().unwrap().series_mut()[0].text = Some(text("_xlchart.v1.0"));
    let error = named.chart().unwrap().series()[0]
        .name_text("Sheet1", &mut resolver)
        .unwrap_err();
    assert!(
        error.starts_with("ChartEx source reference is invalid"),
        "{error}"
    );
}
