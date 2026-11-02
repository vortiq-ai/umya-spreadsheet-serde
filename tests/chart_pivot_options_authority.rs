#![cfg(feature = "serde")]

use std::io::{
    Cursor,
    Read,
    Write,
};

use quick_xml::{
    Reader,
    events::Event,
};
use umya_spreadsheet::{
    Workbook,
    drawing::charts::PivotChartOptions,
    reader,
    writer,
};

const FIXTURE: &[u8] = include_bytes!("fixtures/chart_space_parts/pivot-buttons-default.xlsx");
const FIELD_URI: &str = "{781A3756-C4B2-4CAC-9D66-4F8BD8637D16}";
const EXPAND_URI: &str = "{E28EC0CA-F0BB-4C9C-879D-F8772B89E7AC}";
const FLAGS: [&str; 5] = [
    "c14:dropZoneFilter",
    "c14:dropZoneCategories",
    "c14:dropZoneData",
    "c14:dropZonesVisible",
    "c16:showExpandCollapseFieldButtons",
];

fn chart_xml(bytes: &[u8]) -> String {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut xml = String::new();
    archive
        .by_name("xl/charts/chart1.xml")
        .unwrap()
        .read_to_string(&mut xml)
        .unwrap();
    xml
}

fn opaque_options_fixture() -> Vec<u8> {
    let mut xml = chart_xml(FIXTURE);
    for flag in FLAGS {
        let from = format!("<{flag} val=\"1\"/>");
        assert_eq!(xml.matches(&from).count(), 1);
        xml = xml.replace(&from, &format!("<{flag} val=\"true\"/>"));
    }
    xml = xml.replace(
        "</c:extLst></c:chartSpace>",
        "<c:ext uri=\"urn:keep\" \
         xmlns:k=\"urn:keep\"><k:value>KEEP</k:value></c:ext></c:extLst></c:chartSpace>",
    );
    let mut input = zip::ZipArchive::new(Cursor::new(FIXTURE)).unwrap();
    let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for index in 0..input.len() {
        let mut entry = input.by_index(index).unwrap();
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).unwrap();
        if entry.name().unwrap() == "xl/charts/chart1.xml" {
            bytes = xml.as_bytes().to_vec();
        }
        output
            .start_file(
                entry.name().unwrap(),
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
        output.write_all(&bytes).unwrap();
    }
    output.finish().unwrap().into_inner()
}

fn read(bytes: &[u8]) -> Workbook {
    reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap()
}

fn write(book: &Workbook, light: bool) -> Vec<u8> {
    let mut output = Cursor::new(Vec::new());
    if light {
        writer::xlsx::write_writer_light(book, &mut output).unwrap();
    } else {
        writer::xlsx::write_writer(book, &mut output).unwrap();
    }
    output.into_inner()
}

fn option_flags(xml: &str) -> Vec<(String, String)> {
    let mut parser = Reader::from_str(xml);
    let mut flags = Vec::new();
    loop {
        match parser.read_event().unwrap() {
            Event::Start(tag) | Event::Empty(tag) => {
                let name = String::from_utf8(tag.name().as_ref().to_vec()).unwrap();
                if FLAGS.contains(&name.as_str()) || name == "c14:dropZoneSeries" {
                    let value = tag.try_get_attribute("val").unwrap().unwrap();
                    flags.push((
                        name,
                        value
                            .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                            .unwrap()
                            .into_owned(),
                    ));
                }
            }
            Event::Eof => return flags,
            _ => (),
        }
    }
}

#[test]
fn restored_opaque_pivot_options_cannot_be_overwritten_by_client_flags() {
    let mut book = read(&opaque_options_fixture());
    let authority = book.sheet_by_name("Pivot").unwrap().chart_collection()[0].clone();
    assert!(authority.chart_space().pivot_options().is_none());
    let mut client = authority.clone();
    client.strip_export_parts();
    let mut options = PivotChartOptions::default();
    options
        .set_show_legend_buttons(true)
        .set_show_expand_collapse_buttons(true);
    client.chart_space_mut().set_pivot_options(options);
    client.restore_export_parts(&authority).unwrap();
    book.sheet_by_name_mut("Pivot")
        .unwrap()
        .chart_collection_mut()[0] = client;
    for light in [false, true] {
        let output = write(&book, light);
        let xml = chart_xml(&output);
        assert_eq!(xml.matches(FIELD_URI).count(), 1);
        assert_eq!(xml.matches(EXPAND_URI).count(), 1);
        assert_eq!(
            option_flags(&xml),
            FLAGS.map(|name| (name.to_owned(), "true".to_owned()))
        );
        assert!(xml.contains("KEEP"));
        let reread = read(&output);
        assert!(
            reread.sheet_by_name("Pivot").unwrap().chart_collection()[0]
                .chart_space()
                .pivot_options()
                .is_none()
        );
    }
}

#[test]
fn unlinking_a_pivot_chart_removes_opaque_boolean_options() {
    for removal in ["link", "sheet"] {
        let mut book = read(&opaque_options_fixture());
        let chart = book.sheet_by_name("Pivot").unwrap().chart_collection()[0].clone();
        book.new_sheet("Charts").unwrap().add_chart(chart);
        match removal {
            "link" => {
                book.sheet_by_name_mut("Charts")
                    .unwrap()
                    .chart_collection_mut()[0]
                    .chart_space_mut()
                    .remove_pivot_source();
            }
            "sheet" => {
                let index = book
                    .sheet_collection_no_check()
                    .iter()
                    .position(|sheet| sheet.name() == "Pivot")
                    .unwrap();
                book.remove_sheet(index).unwrap();
            }
            _ => unreachable!(),
        }
        for light in [false, true] {
            let output = write(&book, light);
            let reread = read(&output);
            let chart = &reread.sheet_by_name("Charts").unwrap().chart_collection()[0];
            assert!(chart.chart_space().pivot_source().is_none());
            assert!(chart.chart_space().pivot_options().is_none());
            let extensions = chart.chart_space().extensions().unwrap();
            assert_eq!(extensions.entries.len(), 1, "{removal} light={light}");
            assert_eq!(extensions.entries[0].uri.as_deref(), Some("urn:keep"));
        }
    }
}
