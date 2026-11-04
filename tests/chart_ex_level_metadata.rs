#![cfg(feature = "serde")]

use std::io::Cursor;

use umya_spreadsheet::{
    drawing::{
        charts::*,
        spreadsheet::ExtendedChart,
    },
    reader,
    writer,
};

#[test]
fn reference_metadata_preserves_named_level_positions_and_explicit_numeric_formats() {
    let xml = r#"<cx:chartSpace xmlns:cx="http://schemas.microsoft.com/office/drawing/2014/chartex"><cx:chartData><cx:data id="0"><cx:strDim type="cat"><cx:f>Sheet1!$A$1:$C$2</cx:f><cx:lvl ptCount="2"/><cx:lvl name="Middle" ptCount="2"/><cx:lvl ptCount="2"/></cx:strDim><cx:numDim type="size"><cx:f>Sheet1!$D$1:$D$2</cx:f><cx:lvl formatCode="0.00" ptCount="2"/></cx:numDim></cx:data></cx:chartData><cx:chart><cx:plotArea><cx:plotAreaRegion><cx:series layoutId="sunburst"><cx:dataId val="0"/></cx:series></cx:plotAreaRegion></cx:plotArea></cx:chart></cx:chartSpace>"#;
    let mut source = umya_spreadsheet::new_file();
    for row in 1..=2 {
        for column in 1..=3 {
            source
                .sheet_mut(0)
                .unwrap()
                .cell_mut((column, row))
                .set_value(format!("Level {column}/{row}"));
        }
        source
            .sheet_mut(0)
            .unwrap()
            .cell_mut((4, row))
            .set_value_number(row * 7);
    }
    source
        .sheet_mut(0)
        .unwrap()
        .worksheet_drawing_mut()
        .add_extended_chart(ExtendedChart::new(
            Default::default(),
            ChartExSpace::from_xml(xml.as_bytes()).unwrap(),
        ));
    let mut input = Vec::new();
    writer::xlsx::write_writer(&source, &mut input).unwrap();
    let mut book = reader::xlsx::read_reader(Cursor::new(input), true).unwrap();
    book.sheet_mut(0)
        .unwrap()
        .worksheet_drawing_mut()
        .extended_charts_mut()[0]
        .chart_space_mut()
        .chart_mut()
        .unwrap()
        .series_mut()[0]
        .set_hidden(Some(true));
    let mut output = Vec::new();
    writer::xlsx::write_writer(&book, &mut output).unwrap();
    let reread = reader::xlsx::read_reader(Cursor::new(output), true).unwrap();
    let space = reread
        .sheet(0)
        .unwrap()
        .worksheet_drawing()
        .extended_charts()[0]
        .chart_space();
    let ChartExDimension::String(categories) = &space.data().unwrap().data[0].dimensions[0] else {
        panic!("category dimension")
    };
    let ChartExDimensionSource::Reference(categories) = &categories.source else {
        panic!("category reference")
    };
    assert_eq!(categories.level_properties().len(), 2);
    assert_eq!(categories.level_properties()[0].name, None);
    assert_eq!(
        categories.level_properties()[1].name.as_deref(),
        Some("Middle")
    );
    let ChartExDimension::Numeric(values) = &space.data().unwrap().data[0].dimensions[1] else {
        panic!("size dimension")
    };
    let ChartExDimensionSource::Reference(values) = &values.source else {
        panic!("size reference")
    };
    assert_eq!(values.level_properties().len(), 1);
    assert_eq!(
        values.level_properties()[0].format_code.as_deref(),
        Some("0.00")
    );
    let mut resolver = WorkbookChartExResolver::new(&reread);
    let resolved = space
        .resolved_series_data(&space.chart().unwrap().series()[0], "Sheet1", &mut resolver)
        .unwrap()
        .unwrap();
    let ChartExDimension::String(categories) = &resolved.dimensions[0] else {
        panic!("resolved category dimension")
    };
    let ChartExDimensionSource::Literal(levels) = &categories.source else {
        panic!("resolved levels")
    };
    assert_eq!(levels.len(), 3);
    assert_eq!(levels[1].name.as_deref(), Some("Middle"));
    assert_eq!(levels[2].points[1].value, "Level 3/2");
}
