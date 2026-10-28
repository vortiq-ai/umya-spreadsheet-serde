//! Ordinary chart-part admission by expanded root name. ChartEx remains unsupported.
use std::io::{Cursor, Read, Write};

use umya_spreadsheet::{drawing::{charts::Date1904, spreadsheet::MarkerType}, Chart, ChartType};

fn package_with_chart_part(replace: impl Fn(&str) -> String) -> Vec<u8> {
    let mut book = umya_spreadsheet::new_file();
    let mut chart = Chart::default();
    chart.new_chart(
        &ChartType::LineChart,
        MarkerType::default(),
        MarkerType::default(),
        vec!["Sheet1!$A$1:$A$2"],
    );
    chart.chart_space_mut().set_date1904(Date1904::default());
    book.sheet_mut(0).unwrap().add_chart(chart);
    let mut bytes = Vec::new();
    umya_spreadsheet::writer::xlsx::write_writer(&book, &mut Cursor::new(&mut bytes)).unwrap();
    let mut input = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let mut replaced = 0;
    for index in 0..input.len() {
        let mut entry = input.by_index(index).unwrap();
        let name = entry.name().to_owned();
        let mut data = Vec::new();
        entry.read_to_end(&mut data).unwrap();
        if name == "xl/charts/chart1.xml" {
            data = replace(std::str::from_utf8(&data).unwrap()).into_bytes();
            replaced += 1;
        }
        output.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
        output.write_all(&data).unwrap();
    }
    assert_eq!(replaced, 1);
    output.finish().unwrap().into_inner()
}

#[test]
fn ordinary_root_alias_and_default_namespace_are_admitted() {
    for alias in [true, false] {
        let bytes = package_with_chart_part(|original| {
            let (declaration, prefix) = if alias { ("xmlns:alias=", "alias:") } else { ("xmlns=", "") };
            assert_eq!(original.matches("<c:date1904 val=\"0\"/>").count(), 1);
            original.replace("<c:date1904 val=\"0\"/>", "<c:date1904 val=\"1\"/>")
                .replace("xmlns:c=", declaration)
                .replace("<c:", &format!("<{prefix}"))
                .replace("</c:", &format!("</{prefix}"))
        });
        for eager in [true, false] {
            let mut book = umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(&bytes), eager).unwrap();
            let charts = book.sheet_mut(0).unwrap().chart_collection();
            assert_eq!(charts.len(), 1);
            let chart = &charts[0];
            assert!(chart.chart_space().date1904().unwrap().val());
        }
    }
}

#[test]
fn chartex_root_is_not_imported_as_an_ordinary_chart() {
    // Same expanded root as sb012, with a native-looking descendant as a
    // sentinel. This tests rejection, not ChartEx preservation or support.
    let bytes = package_with_chart_part(|original| {
        let root = original.find("<c:chartSpace").unwrap();
        format!(r#"<cx:chartSpace xmlns:cx="http://schemas.microsoft.com/office/drawing/2014/chartex">{}</cx:chartSpace>"#, &original[root..])
    });
    for eager in [true, false] {
        let mut book = umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(&bytes), eager).unwrap();
        assert!(book.sheet_mut(0).unwrap().chart_collection().is_empty());
    }
}

#[test]
#[should_panic(expected = "invalid chart root expanded name")]
fn wrong_root_in_ordinary_chart_namespace_remains_invalid() {
    let bytes = package_with_chart_part(|_| r#"<c:notChartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart"></c:notChartSpace>"#.to_owned());
    let _ = umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(bytes), true);
}

#[test]
#[should_panic(expected = "Could not index chart namespace scope")]
fn unbound_chart_root_prefix_remains_invalid() {
    let bytes = package_with_chart_part(|_| r#"<c:chartSpace><c:date1904 val="1"/></c:chartSpace>"#.to_owned());
    let _ = umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(bytes), true);
}

#[test]
fn markup_compatibility_style_uses_expanded_names_and_survives_codecs() {
    const MC: &str = "http://schemas.openxmlformats.org/markup-compatibility/2006";
    const C: &str = "http://schemas.openxmlformats.org/drawingml/2006/chart";
    const C14: &str = "http://schemas.microsoft.com/office/drawing/2007/8/2/chart";
    for variant in 0..4 {
        let input = package_with_chart_part(|original| {
            assert_eq!(original.matches("<c:style val=\"2\"/>").count(), 1);
            let mut xml = original.replace("<c:style val=\"2\"/>", "");
            let root = xml.find("<c:chartSpace").unwrap();
            let end = root + xml[root..].find('>').unwrap();
            let inherited = format!(" xmlns:mc=\"{MC}\" xmlns:k=\"{C}\"");
            xml.insert_str(end, &inherited);
            let insert = end + inherited.len() + 1;
            let block = match variant {
                0 => format!("<mc:AlternateContent><mc:Choice xmlns:c14=\"{C14}\" Requires=\"c14\"><c14:style val=\"102\"/></mc:Choice><mc:Fallback><k:style val=\"2\"/></mc:Fallback></mc:AlternateContent>"),
                1 => format!("<m:AlternateContent xmlns:m=\"{MC}\"><m:Choice xmlns:c14=\"{C14}\" Requires=\"c14\"><c14:style val=\"102\"/></m:Choice><m:Fallback><k:style val=\"2\"/></m:Fallback></m:AlternateContent>"),
                2 => format!("<AlternateContent xmlns=\"{MC}\"><Choice xmlns:c14=\"{C14}\" Requires=\"c14\"><c14:style val=\"102\"/></Choice><Fallback><k:style val=\"2\"/></Fallback></AlternateContent>"),
                _ => format!("<f:AlternateContent xmlns:f=\"urn:foreign\"><f:Fallback><k:style val=\"2\"/></f:Fallback></f:AlternateContent>"),
            };
            xml.insert_str(insert, &block);
            xml
        });
        for eager in [true, false] {
            let mut book = umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(&input), eager).unwrap();
            let chart = &book.sheet_mut(0).unwrap().chart_collection()[0];
            if variant == 3 {
                assert!(chart.chart_space().style().is_none());
                continue;
            }
            let style = chart.chart_space().style().unwrap();
            assert!(style.include_alternate_content());
            assert_eq!(style.val(), "2");
            let original = chart.clone();
            let json: Chart = serde_json::from_slice(&serde_json::to_vec(&original).unwrap()).unwrap();
            let binary: Chart = bincode::deserialize(&bincode::serialize(&original).unwrap()).unwrap();
            for value in [original, json, binary] {
                book.sheet_mut(0).unwrap().chart_collection_mut()[0] = value;
                let mut output = Vec::new();
                umya_spreadsheet::writer::xlsx::write_writer(&book, &mut Cursor::new(&mut output)).unwrap();
                let mut archive = zip::ZipArchive::new(Cursor::new(&output)).unwrap();
                let mut xml = String::new();
                archive.by_name("xl/charts/chart1.xml").unwrap().read_to_string(&mut xml).unwrap();
                assert_eq!(xml.matches("<mc:AlternateContent").count(), 1);
                assert_eq!(xml.matches("<c14:style val=\"102\"/>").count(), 1);
                assert_eq!(xml.matches("<c:style val=\"2\"/>").count(), 1);
                let mut reread = umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(&output), eager).unwrap();
                let style = reread.sheet_mut(0).unwrap().chart_collection()[0].chart_space().style().unwrap();
                assert!(style.include_alternate_content());
                assert_eq!(style.val(), "2");
            }
        }
    }
}
