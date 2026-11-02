#![cfg(feature = "serde")]

use std::{
    collections::BTreeMap,
    io::{
        Cursor,
        Read,
        Write,
    },
    path::PathBuf,
};

use quick_xml::{
    Reader,
    events::Event,
};
use umya_spreadsheet::{
    Workbook,
    drawing::charts::ChartExSpace,
    reader,
    writer,
};

const EXCEL_SAVED_NO_SERIES: &[u8] =
    include_bytes!("fixtures/extended_charts/sunburst_chart_without_series.xlsx");
const AUTHORED_NO_SERIES_WITH_DATA: &[u8] =
    include_bytes!("fixtures/extended_charts/sunburst_chart_without_series_with_chart_data.xlsx");
const EXCEL_AUTHORED_SERIES: &[u8] =
    include_bytes!("fixtures/extended_charts/excel-authored/sunburst.xlsx");

#[derive(Debug, PartialEq, Eq)]
struct XmlElement {
    name:     String,
    attrs:    BTreeMap<String, String>,
    text:     String,
    children: Vec<XmlElement>,
}

fn append_element(stack: &mut [XmlElement], roots: &mut Vec<XmlElement>, element: XmlElement) {
    if let Some(parent) = stack.last_mut() {
        parent.children.push(element);
    } else {
        roots.push(element);
    }
}

fn xml_element(event: &quick_xml::events::BytesStart<'_>, reader: &Reader<&[u8]>) -> XmlElement {
    let mut attrs = BTreeMap::new();
    for attr in event.attributes() {
        let attr = attr.unwrap();
        if attr.key.as_ref() == b"xmlns" || attr.key.as_ref().starts_with(b"xmlns:") {
            continue;
        }
        attrs.insert(
            String::from_utf8(attr.key.as_ref().to_vec()).unwrap(),
            attr.decoded_and_normalized_value(quick_xml::XmlVersion::Implicit1_0, reader.decoder())
                .unwrap()
                .into_owned(),
        );
    }
    XmlElement {
        name: String::from_utf8(event.name().as_ref().to_vec()).unwrap(),
        attrs,
        text: String::new(),
        children: Vec::new(),
    }
}

fn parse_xml(bytes: &[u8]) -> XmlElement {
    let mut reader = Reader::from_reader(bytes);
    reader.config_mut().trim_text(true);
    let mut stack = Vec::new();
    let mut roots = Vec::new();
    loop {
        match reader.read_event().unwrap() {
            Event::Start(event) => stack.push(xml_element(&event, &reader)),
            Event::Empty(event) => {
                append_element(&mut stack, &mut roots, xml_element(&event, &reader))
            }
            Event::End(_) => {
                let element = stack.pop().unwrap();
                append_element(&mut stack, &mut roots, element);
            }
            Event::Text(text) => {
                if let Some(element) = stack.last_mut() {
                    element.text.push_str(
                        &text
                            .xml_content(quick_xml::XmlVersion::Implicit1_0)
                            .unwrap(),
                    );
                }
            }
            Event::Eof => break,
            Event::Decl(_) | Event::Comment(_) | Event::PI(_) | Event::DocType(_) => (),
            event => panic!("unexpected XML event: {event:?}"),
        }
    }
    assert!(stack.is_empty());
    assert_eq!(roots.len(), 1);
    roots.pop().unwrap()
}

fn descendants<'a>(element: &'a XmlElement, name: &str, result: &mut Vec<&'a XmlElement>) {
    if element.name == name {
        result.push(element);
    }
    for child in &element.children {
        descendants(child, name, result);
    }
}

fn chart_ex_part(package: &[u8]) -> Vec<u8> {
    let mut archive = zip::ZipArchive::new(Cursor::new(package)).unwrap();
    let mut parts = (0..archive.len())
        .filter_map(|index| {
            let mut file = archive.by_index(index).unwrap();
            let name = file.name().unwrap();
            if name.starts_with("xl/charts/chartEx") && name.ends_with(".xml") {
                let mut bytes = Vec::new();
                file.read_to_end(&mut bytes).unwrap();
                Some(bytes)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(parts.len(), 1);
    parts.pop().unwrap()
}

fn chart_model(book: &Workbook) -> &ChartExSpace {
    book.sheet(0)
        .unwrap()
        .worksheet_drawing()
        .extended_charts()
        .first()
        .unwrap()
        .chart_space()
}

fn assert_blank_chart(space: &ChartExSpace, has_chart_data: bool) {
    let chart = space.chart().unwrap();
    assert_eq!(chart.series().len(), 0);
    assert!(chart.title.is_some());
    assert!(
        chart
            .plot_area
            .as_ref()
            .and_then(|plot| plot.plot_area_region.as_ref())
            .is_some()
    );
    assert_eq!(space.data().is_some(), has_chart_data);
}

fn assert_excel_structure(actual: &[u8], expected: &[u8], has_chart_data: bool) {
    let expected = parse_xml(expected);
    let actual = parse_xml(actual);
    if !has_chart_data {
        assert_eq!(actual, expected, "Excel's empty ChartEx structure changed");
    }
    assert_eq!(actual.name, "cx:chartSpace");
    let root_children = actual
        .children
        .iter()
        .map(|child| child.name.as_str())
        .collect::<Vec<_>>();
    let expected_root_children = if has_chart_data {
        vec!["cx:chartData", "cx:chart"]
    } else {
        vec!["cx:chart"]
    };
    assert_eq!(root_children, expected_root_children);
    if has_chart_data {
        let mut actual_data = Vec::new();
        let mut expected_data = Vec::new();
        descendants(&actual, "cx:data", &mut actual_data);
        descendants(&expected, "cx:data", &mut expected_data);
        assert_eq!(actual_data.len(), expected_data.len());
        for (actual, expected) in actual_data.iter().zip(expected_data) {
            assert_eq!(actual.attrs.get("id"), expected.attrs.get("id"));
            let dimensions = |data: &XmlElement| {
                data.children
                    .iter()
                    .filter(|child| matches!(child.name.as_str(), "cx:strDim" | "cx:numDim"))
                    .map(|child| child.name.clone())
                    .collect::<Vec<_>>()
            };
            assert_eq!(dimensions(actual), dimensions(expected));
        }
    }
    let mut titles = Vec::new();
    let mut regions = Vec::new();
    let mut series = Vec::new();
    let mut data_collections = Vec::new();
    descendants(&actual, "cx:title", &mut titles);
    descendants(&actual, "cx:plotAreaRegion", &mut regions);
    descendants(&actual, "cx:series", &mut series);
    descendants(&actual, "cx:chartData", &mut data_collections);
    assert_eq!(titles.len(), 1);
    assert_eq!(titles[0].attrs.get("pos").map(String::as_str), Some("t"));
    assert_eq!(
        titles[0].attrs.get("align").map(String::as_str),
        Some("ctr")
    );
    assert_eq!(
        titles[0].attrs.get("overlay").map(String::as_str),
        Some("0")
    );
    assert_eq!(regions.len(), 1);
    assert!(series.is_empty());
    assert_eq!(data_collections.len(), usize::from(has_chart_data));
}

fn emit(name: &str, bytes: &[u8]) {
    let Some(target) = std::env::var_os("CARGO_TARGET_DIR") else {
        return;
    };
    let directory = PathBuf::from(target).join("chartex-empty-series-validation");
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join(format!("{name}.xlsx")), bytes).unwrap();
}

fn write(book: &Workbook, light: bool) -> Vec<u8> {
    let mut output = Vec::new();
    if light {
        writer::xlsx::write_writer_light(book, &mut output).unwrap();
    } else {
        writer::xlsx::write_writer(book, &mut output).unwrap();
    }
    output
}

fn read(package: &[u8], eager: bool) -> Workbook {
    let mut book = reader::xlsx::read_reader(Cursor::new(package), eager).unwrap();
    if !eager {
        book.try_read_sheet(0).unwrap();
    }
    book
}

fn round_trip_empty_series(source: &[u8], name: &str, has_chart_data: bool) {
    let expected_xml = chart_ex_part(source);
    assert_excel_structure(&expected_xml, &expected_xml, has_chart_data);

    for eager in [true, false] {
        let book = read(source, eager);
        assert_blank_chart(chart_model(&book), has_chart_data);
        let before = serde_json::to_value(chart_model(&book)).unwrap();
        for light in [false, true] {
            let output = write(&book, light);
            let written_xml = chart_ex_part(&output);
            assert_excel_structure(&written_xml, &expected_xml, has_chart_data);

            let reread = read(&output, eager);
            assert_blank_chart(chart_model(&reread), has_chart_data);
            if !has_chart_data {
                assert_eq!(serde_json::to_value(chart_model(&reread)).unwrap(), before);
            }

            let suffix = if eager { "eager" } else { "lazy" };
            let writer = if light { "light" } else { "full" };
            emit(&format!("{name}-{suffix}-{writer}"), &output);
        }
    }
}

#[test]
fn excel_saved_sunburst_without_series_round_trips_without_chart_data() {
    round_trip_empty_series(EXCEL_SAVED_NO_SERIES, "excel-saved-no-series", false);
}

#[test]
fn authored_sunburst_without_series_keeps_its_chart_data() {
    round_trip_empty_series(
        AUTHORED_NO_SERIES_WITH_DATA,
        "authored-no-series-with-data",
        true,
    );
}

#[test]
fn chart_ex_without_data_defaults_in_persisted_json() {
    let book = reader::xlsx::read_reader(Cursor::new(EXCEL_SAVED_NO_SERIES), true).unwrap();
    let mut json = serde_json::to_value(chart_model(&book)).unwrap();
    json.as_object_mut().unwrap().remove("data_collection");
    let restored: ChartExSpace = serde_json::from_value(json).unwrap();
    assert!(restored.data().is_none());
    assert!(restored.validate().is_ok());
}

fn rewrite_chart_ex(package: &[u8], mut modify: impl FnMut(&mut String)) -> Vec<u8> {
    let mut archive = zip::ZipArchive::new(Cursor::new(package)).unwrap();
    let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let mut changed = false;
    for index in 0..archive.len() {
        let mut file = archive.by_index(index).unwrap();
        let name = file.name().unwrap().into_owned();
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).unwrap();
        if name.starts_with("xl/charts/chartEx") && name.ends_with(".xml") {
            let mut xml = String::from_utf8(bytes).unwrap();
            modify(&mut xml);
            bytes = xml.into_bytes();
            changed = true;
        }
        output
            .start_file(&name, zip::write::SimpleFileOptions::default())
            .unwrap();
        output.write_all(&bytes).unwrap();
    }
    assert!(changed);
    output.finish().unwrap().into_inner()
}

fn rejected_by_both_readers(package: &[u8], expected: &str) {
    for eager in [true, false] {
        let result = reader::xlsx::read_reader(Cursor::new(package), eager);
        let error = match result {
            Err(error) => error.to_string(),
            Ok(mut book) if !eager => match book.try_read_sheet(0) {
                Err(error) => error.to_string(),
                Ok(_) => panic!("invalid ChartEx was accepted by the lazy reader"),
            },
            Ok(_) => panic!("invalid ChartEx was accepted by the eager reader"),
        };
        assert!(error.contains(expected), "unexpected error: {error}");
    }
}

#[test]
fn invalid_chart_ex_data_references_and_blocks_remain_rejected() {
    let missing_reference = rewrite_chart_ex(EXCEL_AUTHORED_SERIES, |xml| {
        let prefix = "<cx:dataId val=\"";
        let start = xml.find(prefix).expect("fixture series data id") + prefix.len();
        let end = start + xml[start..].find('"').unwrap();
        xml.replace_range(start..end, "4294967295");
    });
    rejected_by_both_readers(&missing_reference, "unresolved ChartEx data id 4294967295");

    let duplicate_data_id = rewrite_chart_ex(AUTHORED_NO_SERIES_WITH_DATA, |xml| {
        let root = parse_xml(xml.as_bytes());
        let mut data = Vec::new();
        descendants(&root, "cx:data", &mut data);
        let id = data[0].attrs.get("id").unwrap();
        let duplicate = format!(
            "<cx:data id=\"{id}\"><cx:strDim type=\"cat\"><cx:lvl \
             ptCount=\"0\"/></cx:strDim></cx:data>"
        );
        *xml = xml.replace("</cx:chartData>", &format!("{duplicate}</cx:chartData>"));
    });
    rejected_by_both_readers(&duplicate_data_id, "ambiguous ChartEx data id 0");

    let dimensionless_data = rewrite_chart_ex(AUTHORED_NO_SERIES_WITH_DATA, |xml| {
        let start = xml.find("<cx:data ").unwrap();
        let content_start = xml[start..].find('>').unwrap() + start + 1;
        let content_end = xml[content_start..].find("</cx:data>").unwrap() + content_start;
        xml.replace_range(content_start..content_end, "");
    });
    rejected_by_both_readers(&dimensionless_data, "ChartEx data needs a dimension");
}
