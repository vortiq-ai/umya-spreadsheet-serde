#![cfg(feature = "serde")]
//! Failed sheet reads must not lose raw data or earlier successful cache
//! baselines.
use std::{
    io::{
        Cursor,
        Read,
        Write,
    },
    path::PathBuf,
};

use umya_spreadsheet::{
    Chart,
    ChartType,
    Workbook,
    XlsxErrorKind,
    drawing::{
        charts::{
            CategoryAxisData,
            StringReference,
        },
        spreadsheet::MarkerType,
    },
};

#[path = "support/chart_xml_fixture.rs"]
pub mod fixture;

const BROKEN_XML: &str = "<worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\"><sheetData><row r=\"1\"><c r=\"A1\"><v>42</v></c></row></wrong></worksheet>";

fn replace_part(bytes: &[u8], name: &str, replacement: &[u8]) -> Vec<u8> {
    let mut source = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let mut replaced = false;
    for index in 0..source.len() {
        let mut entry = source.by_index(index).unwrap();
        let mut data = Vec::new();
        entry.read_to_end(&mut data).unwrap();
        let entry_name = entry.name().unwrap().into_owned();
        if entry_name == name {
            data = replacement.to_vec();
            replaced = true;
        }
        output
            .start_file(entry_name, zip::write::SimpleFileOptions::default())
            .unwrap();
        output.write_all(&data).unwrap();
    }
    assert!(replaced);
    output.finish().unwrap().into_inner()
}

fn part(bytes: &[u8], name: &str) -> String {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut text = String::new();
    archive
        .by_name(name)
        .unwrap()
        .read_to_string(&mut text)
        .unwrap();
    text
}

fn snapshot_categories(mut xml: String, formula: &str) -> String {
    let start = xml.find("<c:cat>").unwrap();
    let end = start + xml[start..].find("</c:cat>").unwrap() + "</c:cat>".len();
    xml.replace_range(
        start..end,
        &format!(
            "<c:cat><c:strRef><c:f>{formula}</c:f><c:strCache><c:ptCount val=\"2\"/><c:pt \
             idx=\"0\"><c:v>Original one</c:v></c:pt><c:pt idx=\"1\"><c:v>Original \
             two</c:v></c:pt></c:strCache></c:strRef></c:cat>"
        ),
    );
    xml
}

fn valid_input(with_second_chart: bool) -> Vec<u8> {
    let mut source = umya_spreadsheet::new_file();
    source.new_sheet("Broken").unwrap();
    source.new_sheet("Unrelated").unwrap();
    for row in 1..=2 {
        for index in [0, 1] {
            source
                .sheet_mut(index)
                .unwrap()
                .cell_mut((1, row))
                .set_value_number(row);
        }
    }
    source
        .sheet_mut(1)
        .unwrap()
        .add_defined_name("Local", "Broken!$A$1")
        .unwrap();
    source
        .sheet_mut(2)
        .unwrap()
        .cell_mut("A1")
        .set_value("leave raw");
    let mut chart = Chart::default();
    chart.new_chart(
        &ChartType::LineChart,
        MarkerType::default(),
        MarkerType::default(),
        vec!["Sheet1!$A$1:$A$2"],
    );
    let mut reference = StringReference::default();
    reference.formula_mut().set_address_str("Sheet1!$A$1:$A$2");
    let mut categories = CategoryAxisData::default();
    categories.set_string_reference(reference);
    chart.area_chart_series_list_mut().area_chart_series_mut()[0]
        .set_category_axis_data(categories);
    if with_second_chart {
        let mut second = chart.clone();
        second.area_chart_series_list_mut().area_chart_series_mut()[0]
            .category_axis_data_mut()
            .unwrap()
            .string_reference_mut()
            .unwrap()
            .formula_mut()
            .set_address_str("Broken!$A$1:$A$2");
        source.sheet_mut(1).unwrap().add_chart(second);
    }
    source.sheet_mut(0).unwrap().add_chart(chart);
    let chart_xml = snapshot_categories(fixture::chart_xml(&source), "Sheet1!$A$1:$A$2");
    let mut bytes = fixture::replace_chart_part(&fixture::write_workbook(&source), &chart_xml);
    if with_second_chart {
        let second = snapshot_categories(part(&bytes, "xl/charts/chart2.xml"), "Broken!$A$1:$A$2");
        bytes = replace_part(&bytes, "xl/charts/chart2.xml", second.as_bytes());
    }
    bytes
}

fn input() -> Vec<u8> {
    replace_part(
        &valid_input(false),
        "xl/worksheets/sheet2.xml",
        BROKEN_XML.as_bytes(),
    )
}

fn snapshot(book: &Workbook, index: usize) -> Vec<u8> {
    bincode::serialize(&book.sheet_collection_no_check()[index]).unwrap()
}

fn file_path() -> PathBuf {
    std::env::temp_dir().join(format!(
        "umya-lazy-failed-sheet-{}-{}.xlsx",
        std::process::id(),
        std::thread::current()
            .name()
            .unwrap_or("test")
            .replace("::", "-")
    ))
}

#[test]
fn failed_sheet_read_preserves_successful_prefix_cache_and_raw_retry_state() {
    let bytes = input();
    let path = file_path();
    std::fs::write(&path, &bytes).unwrap();
    for mut book in [
        umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(&bytes), false).unwrap(),
        umya_spreadsheet::reader::xlsx::lazy_read(&path).unwrap(),
    ] {
        let broken = snapshot(&book, 1);
        let unrelated = snapshot(&book, 2);
        assert_eq!(book.chart_source_cache_report().kept_caches, 0);
        assert_eq!(
            book.try_read_sheet_collection().unwrap_err().kind(),
            XlsxErrorKind::Xml
        );
        assert_eq!(book.sheet(0).unwrap().value("A1"), "1");
        assert_eq!(book.chart_source_cache_report().kept_caches, 1);
        assert_eq!(
            book.sheet(1).unwrap_err().kind(),
            XlsxErrorKind::NotDeserialized
        );
        assert_eq!(
            book.sheet(2).unwrap_err().kind(),
            XlsxErrorKind::NotDeserialized
        );
        assert_eq!(snapshot(&book, 1), broken);
        assert_eq!(snapshot(&book, 2), unrelated);
        let loaded = snapshot(&book, 0);
        let report = *book.chart_source_cache_report();
        for _ in 0..2 {
            assert_eq!(
                book.try_read_sheet_collection().unwrap_err().kind(),
                XlsxErrorKind::Xml
            );
            assert_eq!(
                book.try_read_sheet(1).unwrap_err().kind(),
                XlsxErrorKind::Xml
            );
            assert_eq!(book.sheet_mut(1).unwrap_err().kind(), XlsxErrorKind::Xml);
            assert_eq!(snapshot(&book, 0), loaded);
            assert_eq!(snapshot(&book, 1), broken);
            assert_eq!(snapshot(&book, 2), unrelated);
            assert_eq!(*book.chart_source_cache_report(), report);
        }
        let output = fixture::write_workbook(&book);
        let mut archive = zip::ZipArchive::new(Cursor::new(output)).unwrap();
        let mut actual = String::new();
        archive
            .by_name("xl/worksheets/sheet2.xml")
            .unwrap()
            .read_to_string(&mut actual)
            .unwrap();
        assert_eq!(actual, BROKEN_XML);
        assert_eq!(snapshot(&book, 0), loaded);
    }
    std::fs::remove_file(path).unwrap();
}

#[test]
fn failed_single_sheet_access_does_not_materialize_or_charge_other_sheets() {
    let bytes = input();
    let mut book = umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(bytes), false).unwrap();
    let before = serde_json::to_value(book.sheet_collection_no_check()).unwrap();
    assert_eq!(
        book.try_read_sheet(1).unwrap_err().kind(),
        XlsxErrorKind::Xml
    );
    assert_eq!(
        serde_json::to_value(book.sheet_collection_no_check()).unwrap(),
        before
    );
    assert_eq!(book.chart_source_cache_report().kept_caches, 0);
    book.try_read_sheet(0).unwrap();
    assert_eq!(book.chart_source_cache_report().kept_caches, 1);
    assert_eq!(
        book.sheet(2).unwrap_err().kind(),
        XlsxErrorKind::NotDeserialized
    );
}

#[test]
fn eager_read_returns_required_worksheet_xml_errors() {
    assert_eq!(
        umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(input()), true)
            .unwrap_err()
            .kind(),
        XlsxErrorKind::Xml
    );
}

#[test]
fn late_drawing_xml_error_rolls_back_visited_chart_parts_and_cache_admission() {
    let valid = valid_input(true);
    let control = umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(&valid), true).unwrap();
    assert_eq!(control.chart_source_cache_report().kept_caches, 2);
    assert_eq!(control.sheet(1).unwrap().chart_collection().len(), 1);
    let mut drawing = part(&valid, "xl/drawings/drawing2.xml");
    assert!(drawing.ends_with("</xdr:wsDr>"), "{drawing}");
    drawing.push_str("<bad></wrong>");
    let bytes = replace_part(&valid, "xl/drawings/drawing2.xml", drawing.as_bytes());
    let path = file_path();
    std::fs::write(&path, &bytes).unwrap();
    for mut book in [
        umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(&bytes), false).unwrap(),
        umya_spreadsheet::reader::xlsx::lazy_read(&path).unwrap(),
    ] {
        let broken = snapshot(&book, 1);
        let unrelated = snapshot(&book, 2);
        assert_eq!(
            book.try_read_sheet_collection().unwrap_err().kind(),
            XlsxErrorKind::Xml
        );
        assert_eq!(book.chart_source_cache_report().kept_caches, 1);
        assert_eq!(
            book.sheet(1).unwrap_err().kind(),
            XlsxErrorKind::NotDeserialized
        );
        assert_eq!(snapshot(&book, 1), broken);
        assert!(book.sheet_collection_no_check()[1].cell("A1").is_none());
        assert_eq!(snapshot(&book, 2), unrelated);
        let loaded = snapshot(&book, 0);
        let report = *book.chart_source_cache_report();
        for _ in 0..2 {
            assert_eq!(
                book.try_read_sheet(1).unwrap_err().kind(),
                XlsxErrorKind::Xml
            );
            assert_eq!(
                book.try_read_sheet_collection().unwrap_err().kind(),
                XlsxErrorKind::Xml
            );
            assert_eq!(snapshot(&book, 0), loaded);
            assert_eq!(snapshot(&book, 1), broken);
            assert_eq!(snapshot(&book, 2), unrelated);
            assert_eq!(*book.chart_source_cache_report(), report);
        }
    }
    std::fs::remove_file(path).unwrap();
}

fn aliased_chart_input() -> Vec<u8> {
    let valid = valid_input(true);
    let mut drawing = part(&valid, "xl/drawings/drawing2.xml");
    let start = drawing.find("<xdr:twoCellAnchor").unwrap();
    let end = drawing.find("</xdr:twoCellAnchor>").unwrap() + "</xdr:twoCellAnchor>".len();
    let frame = &drawing[start..end];
    let id_start = frame.find("r:id=\"").unwrap() + "r:id=\"".len();
    let id_end = id_start + frame[id_start..].find('"').unwrap();
    let id = &frame[id_start..id_end];
    let alias = frame
        .replace(&format!("r:id=\"{id}\""), "r:id=\"rIdAlias\"")
        .replace("id=\"1\"", "id=\"99\"");
    let mut relationships = part(&valid, "xl/drawings/_rels/drawing2.xml.rels");
    let rel_start = relationships.find("<Relationship ").unwrap();
    let rel_end = rel_start + relationships[rel_start..].find("/>").unwrap() + 2;
    let relationship =
        relationships[rel_start..rel_end].replace(&format!("Id=\"{id}\""), "Id=\"rIdAlias\"");
    assert!(relationship.contains("Id=\"rIdAlias\""));
    relationships.insert_str(rel_end, &relationship);
    drawing.insert_str(end, &alias);
    let bytes = replace_part(&valid, "xl/drawings/drawing2.xml", drawing.as_bytes());
    replace_part(
        &bytes,
        "xl/drawings/_rels/drawing2.xml.rels",
        relationships.as_bytes(),
    )
}

fn chart_raw_files(sheet: &serde_json::Value) -> Vec<&serde_json::Value> {
    sheet["raw_data_of_worksheet"]["relationships_list"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|list| list["relationship_list"].as_array().unwrap())
        .filter(|relationship| {
            relationship["r_type"]["value"]
                .as_str()
                .unwrap()
                .ends_with("/chart")
        })
        .map(|relationship| &relationship["raw_file"])
        .collect()
}

#[test]
fn aliased_chart_targets_survive_late_failure_and_retry() {
    let valid = aliased_chart_input();
    let mut control =
        umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(&valid), true).unwrap();
    assert_eq!(control.sheet(1).unwrap().chart_collection().len(), 2);
    // Byte-identical aliases legitimately share one emitted part. Give the
    // second actual frame distinct native metadata to exercise independent
    // part allocation without changing its source cache payload.
    control.sheet_mut(1).unwrap().chart_collection_mut()[1]
        .chart_space_mut()
        .editing_language_mut()
        .set_val("fr-FR");
    let exported = fixture::write_workbook(&control);
    // Export assigns leaf keys from actual native frame identities, not
    // collection ordinals. Follow the package's chart relationships.
    let relationships = part(&exported, "xl/drawings/_rels/drawing2.xml.rels");
    let mut reader = quick_xml::Reader::from_str(&relationships);
    let mut targets = Vec::new();
    loop {
        match reader.read_event().unwrap() {
            quick_xml::events::Event::Empty(element)
                if element.local_name().as_ref() == b"Relationship" =>
            {
                let attributes: Vec<_> = element.attributes().map(Result::unwrap).collect();
                if attributes.iter().any(|attribute| {
                    attribute.key.as_ref() == b"Type" && attribute.value.ends_with(b"/chart")
                }) {
                    let target = attributes
                        .iter()
                        .find(|attribute| attribute.key.as_ref() == b"Target")
                        .unwrap();
                    let target = target
                        .decoded_and_normalized_value(
                            quick_xml::XmlVersion::Implicit1_0,
                            reader.decoder(),
                        )
                        .unwrap();
                    targets.push(format!("xl/{}", target.strip_prefix("../").unwrap()));
                }
            }
            quick_xml::events::Event::Eof => break,
            _ => (),
        }
    }
    assert_eq!(targets.len(), 2);
    assert_ne!(targets[0], targets[1]);
    for (target, language) in targets.iter().zip(["en-GB", "fr-FR"]) {
        let xml = part(&exported, target);
        assert!(xml.contains("Original one"));
        assert!(xml.contains(&format!("<c:lang val=\"{language}\"/>")));
    }
    let mut drawing = part(&valid, "xl/drawings/drawing2.xml");
    drawing.push_str("<bad></wrong>");
    let bytes = replace_part(&valid, "xl/drawings/drawing2.xml", drawing.as_bytes());
    let path = file_path();
    std::fs::write(&path, &bytes).unwrap();
    for mut book in [
        umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(&bytes), false).unwrap(),
        umya_spreadsheet::reader::xlsx::lazy_read(&path).unwrap(),
    ] {
        let broken = snapshot(&book, 1);
        let raw = serde_json::to_value(&book.sheet_collection_no_check()[1]).unwrap();
        let files = chart_raw_files(&raw);
        assert_eq!(files.len(), 2);
        assert_eq!(files[0]["file_data"], files[1]["file_data"]);
        assert!(!files[0]["file_data"].as_array().unwrap().is_empty());
        let unrelated = snapshot(&book, 2);
        for _ in 0..3 {
            assert_eq!(
                book.try_read_sheet_collection().unwrap_err().kind(),
                XlsxErrorKind::Xml
            );
            assert_eq!(snapshot(&book, 1), broken);
            assert_eq!(snapshot(&book, 2), unrelated);
            assert!(book.sheet_collection_no_check()[1].cell("A1").is_none());
            assert_eq!(book.chart_source_cache_report().kept_caches, 1);
            assert_eq!(
                book.try_read_sheet(1).unwrap_err().kind(),
                XlsxErrorKind::Xml
            );
            assert_eq!(snapshot(&book, 1), broken);
            assert_eq!(book.chart_source_cache_report().kept_caches, 1);
        }
    }
    std::fs::remove_file(path).unwrap();
}

#[test]
fn source_backed_empty_chart_alias_buffers_remain_empty_after_failure() {
    let valid = aliased_chart_input();
    let mut drawing = part(&valid, "xl/drawings/drawing2.xml");
    drawing.push_str("<bad></wrong>");
    let bytes = replace_part(&valid, "xl/drawings/drawing2.xml", drawing.as_bytes());
    let path = file_path();
    std::fs::write(&path, &bytes).unwrap();
    let lazy = umya_spreadsheet::reader::xlsx::lazy_read(&path).unwrap();
    let mut broken = serde_json::to_value(&lazy.sheet_collection_no_check()[1]).unwrap();
    for list in broken["raw_data_of_worksheet"]["relationships_list"]
        .as_array_mut()
        .unwrap()
    {
        for relationship in list["relationship_list"].as_array_mut().unwrap() {
            if relationship["r_type"]["value"]
                .as_str()
                .unwrap()
                .ends_with("/chart")
            {
                assert_eq!(
                    relationship["raw_file"]["source_file"],
                    serde_json::to_value(&path).unwrap()
                );
                relationship["raw_file"]["file_data"] = serde_json::json!([]);
            }
        }
    }
    assert_eq!(chart_raw_files(&broken).len(), 2);
    let mut book = umya_spreadsheet::new_file();
    book.add_sheet(serde_json::from_value(broken).unwrap())
        .unwrap();
    let broken = snapshot(&book, 1);
    for _ in 0..3 {
        assert_eq!(
            book.try_read_sheet(1).unwrap_err().kind(),
            XlsxErrorKind::Xml
        );
        assert_eq!(snapshot(&book, 1), broken);
        assert_eq!(book.chart_source_cache_report().kept_caches, 0);
    }
    std::fs::remove_file(path).unwrap();
}
