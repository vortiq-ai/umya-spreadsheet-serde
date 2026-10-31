use std::io::{
    Cursor,
    Read,
    Write,
};

use umya_spreadsheet::{
    DefinedName,
    Workbook,
    drawing::{
        charts::*,
        spreadsheet::ExtendedChart,
    },
    reader,
    writer,
};

fn save(book: &Workbook) -> Result<Vec<u8>, umya_spreadsheet::XlsxError> {
    let mut output = Vec::new();
    writer::xlsx::write_writer(book, &mut output)?;
    Ok(output)
}

fn source(definition: &str, category: bool) -> Vec<u8> {
    let mut book = umya_spreadsheet::new_file();
    let mut name = DefinedName::default();
    name.set_name("Label").set_address("\"seed\"");
    book.add_defined_names(name);
    let dimension = if category {
        "<cx:strDim type=\"cat\"><cx:f>Label</cx:f></cx:strDim>"
    } else {
        "<cx:numDim type=\"val\"><cx:lvl ptCount=\"1\"><cx:pt \
         idx=\"0\">10</cx:pt></cx:lvl></cx:numDim>"
    };
    let xml = format!(
        "<cx:chartSpace xmlns:cx=\"http://schemas.microsoft.com/office/drawing/2014/chartex\"><cx:chartData><cx:data id=\"1\">{dimension}</cx:data></cx:chartData><cx:chart><cx:title><cx:tx><cx:txData><cx:f>Label</cx:f><cx:v>stale</cx:v></cx:txData></cx:tx></cx:title><cx:plotArea><cx:plotAreaRegion><cx:series layoutId=\"waterfall\"><cx:dataId val=\"1\"/></cx:series></cx:plotAreaRegion></cx:plotArea></cx:chart></cx:chartSpace>"
    );
    book.sheet_mut(0)
        .unwrap()
        .worksheet_drawing_mut()
        .add_extended_chart(ExtendedChart::new(
            Default::default(),
            ChartExSpace::from_xml(xml.as_bytes()).unwrap(),
        ));
    let seed = save(&book).unwrap();
    let mut input = zip::ZipArchive::new(Cursor::new(seed)).unwrap();
    let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for index in 0..input.len() {
        let mut file = input.by_index(index).unwrap();
        let path = file.name().unwrap().into_owned();
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).unwrap();
        if path == "xl/workbook.xml" {
            let xml = String::from_utf8(bytes).unwrap();
            let definition = definition.replace('&', "&amp;").replace('<', "&lt;");
            assert!(xml.contains(">\"seed\"</definedName>"));
            bytes = xml
                .replace(
                    ">\"seed\"</definedName>",
                    &format!(">{definition}</definedName>"),
                )
                .into_bytes();
        } else if path == "xl/charts/chartEx1.xml" {
            bytes = xml.as_bytes().to_vec();
        }
        output
            .start_file(path, zip::write::SimpleFileOptions::default())
            .unwrap();
        output.write_all(&bytes).unwrap();
    }
    output.finish().unwrap().into_inner()
}

fn edited(input: &[u8]) -> Workbook {
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
    book
}

#[test]
fn quoted_concatenation_requires_an_evaluator_for_titles_and_categories() {
    for category in [false, true] {
        let input = source("\"A\"&\"B\"", category);
        let Err(error) = save(&edited(&input)) else {
            panic!("concatenation cannot become a literal label");
        };
        assert!(
            error.to_string().contains("unsupported ChartEx expression"),
            "{error}"
        );
    }
}

#[test]
fn doubled_quotes_and_empty_string_are_real_literals() {
    for (definition, expected) in [
        ("\"A\"\"B\"", "A\"B"),
        ("\"\"", ""),
        ("\"A\"\"\"\"B\"", "A\"\"B"),
    ] {
        for category in [false, true] {
            let input = source(definition, category);
            let bytes = save(&edited(&input)).unwrap();
            if category {
                let mut archive = zip::ZipArchive::new(Cursor::new(&bytes)).unwrap();
                let mut xml = String::new();
                archive
                    .by_name("xl/charts/chartEx1.xml")
                    .unwrap()
                    .read_to_string(&mut xml)
                    .unwrap();
                let expected = quick_xml::escape::escape(expected);
                assert!(xml.contains("ptCount=\"1\""), "{xml}");
                assert!(
                    xml.contains(&format!("<cx:pt idx=\"0\">{expected}</cx:pt>")),
                    "{xml}"
                );
            }
            let book = reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap();
            let chart =
                book.sheet(0).unwrap().worksheet_drawing().extended_charts()[0].chart_space();
            let title = chart
                .chart()
                .unwrap()
                .title
                .as_ref()
                .unwrap()
                .text
                .as_ref()
                .unwrap()
                .tx_data
                .as_ref()
                .unwrap();
            assert_eq!(title.value(), Some(expected));
            if category {
                let ChartExDimension::String(dimension) =
                    &chart.data().unwrap().data[0].dimensions[0]
                else {
                    panic!("string dimension")
                };
                let ChartExDimensionSource::Reference(reference) = &dimension.source else {
                    panic!("reference dimension")
                };
                let levels = WorkbookChartExResolver::new(&book)
                    .string_levels("Sheet1", reference)
                    .unwrap();
                assert_eq!(levels[0].points[0].value, expected);
            }
        }
    }
}

#[test]
fn supplied_evaluator_exports_the_computed_label() {
    struct Evaluator;
    impl ChartExDataResolver for Evaluator {
        fn text_value(
            &mut self,
            _: &str,
            formula: &ChartExFormula,
        ) -> Result<Option<String>, String> {
            assert_eq!(formula.text(), "Label");
            Ok(Some("AB".into()))
        }

        fn string_levels(
            &mut self,
            _: &str,
            _: &ChartExStringReference,
        ) -> Result<Vec<ChartExStringLevel>, String> {
            Err("unexpected string reference".into())
        }

        fn numeric_levels(
            &mut self,
            _: &str,
            _: &ChartExNumericReference,
        ) -> Result<Vec<ChartExNumericLevel>, String> {
            Err("unexpected numeric reference".into())
        }
    }
    let input = source("\"A\"&\"B\"", false);
    let book = edited(&input);
    let chart = book.sheet(0).unwrap().worksheet_drawing().extended_charts()[0].chart_space();
    let output = chart.to_xml("Sheet1", &mut Evaluator).unwrap();
    let reread = ChartExSpace::from_xml(&output).unwrap();
    let title = reread
        .chart()
        .unwrap()
        .title
        .as_ref()
        .unwrap()
        .text
        .as_ref()
        .unwrap()
        .tx_data
        .as_ref()
        .unwrap();
    assert_eq!(title.value(), Some("AB"));
}
