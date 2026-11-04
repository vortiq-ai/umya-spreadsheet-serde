#![cfg(feature = "serde")]

use std::io::{
    Cursor,
    Read,
    Write,
};

use umya_spreadsheet::{
    Chart,
    reader,
    writer,
};

const CHART_RELATIONSHIPS: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const PACKAGE_RELATIONSHIPS: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
const OF_PIE_CASE: usize = 7;
const BAR_CASE: usize = 10;
const IMAGE: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0,
    0, 0, 31, 21, 196, 137, 0, 0, 0, 11, 73, 68, 65, 84, 120, 156, 99, 0, 1, 0, 0, 5, 0, 1, 13, 10,
    45, 180, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];

fn add_part(bytes: &[u8], target: &str, data: &[u8]) -> Vec<u8> {
    let mut input = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for index in 0..input.len() {
        let mut entry = input.by_index(index).unwrap();
        let name = entry.name().unwrap().into_owned();
        assert_ne!(name, target, "part already exists: {target}");
        let mut contents = Vec::new();
        entry.read_to_end(&mut contents).unwrap();
        output
            .start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        output.write_all(&contents).unwrap();
    }
    output
        .start_file(target, zip::write::SimpleFileOptions::default())
        .unwrap();
    output.write_all(data).unwrap();
    output.finish().unwrap().into_inner()
}

fn replace_part(bytes: &[u8], target: &str, data: &[u8]) -> Vec<u8> {
    let mut input = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let mut replaced = false;
    for index in 0..input.len() {
        let mut entry = input.by_index(index).unwrap();
        let name = entry.name().unwrap().into_owned();
        let mut contents = Vec::new();
        entry.read_to_end(&mut contents).unwrap();
        if name == target {
            contents = data.to_vec();
            replaced = true;
        }
        output
            .start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        output.write_all(&contents).unwrap();
    }
    assert!(replaced, "missing part: {target}");
    output.finish().unwrap().into_inner()
}

fn part(bytes: &[u8], name: &str) -> Vec<u8> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut contents = Vec::new();
    archive
        .by_name(name)
        .unwrap()
        .read_to_end(&mut contents)
        .unwrap();
    contents
}

fn chart_fixture_part(case: usize) -> Vec<u8> {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/chart_group_settings/excel-authored/groups.xlsx");
    let bytes = std::fs::read(path).unwrap();
    part(&bytes, &format!("xl/charts/chart{case}.xml"))
}

fn package_with_series_line_picture(case: usize) -> Vec<u8> {
    let mut chart = String::from_utf8(chart_fixture_part(case)).unwrap();
    assert_eq!(chart.matches("<a:blip ").count(), 0);
    assert_eq!(chart.matches("<c:serLines/>").count(), 1);
    let series_lines = r#"<c:serLines><c:spPr><a:blipFill><a:blip r:embed="seriesLineImage"/><a:stretch><a:fillRect/></a:stretch></a:blipFill></c:spPr></c:serLines>"#;
    chart = chart.replacen("<c:serLines/>", series_lines, 1);

    let mut book = umya_spreadsheet::new_file();
    book.sheet_mut(0).unwrap().add_chart(fixture::base_chart());
    let mut package = fixture::write_workbook(&book);
    package = replace_part(&package, "xl/charts/chart1.xml", chart.as_bytes());

    let content_types = String::from_utf8(part(&package, "[Content_Types].xml")).unwrap();
    let content_types = if content_types.contains("Extension=\"png\"") {
        content_types
    } else {
        content_types.replacen(
            "</Types>",
            "<Default Extension=\"png\" ContentType=\"image/png\"/></Types>",
            1,
        )
    };
    package = replace_part(&package, "[Content_Types].xml", content_types.as_bytes());
    package = add_part(
        &package,
        "xl/charts/_rels/chart1.xml.rels",
        format!(
            r#"<Relationships xmlns="{PACKAGE_RELATIONSHIPS}"><Relationship Id="seriesLineImage" Type="{CHART_RELATIONSHIPS}/image" Target="../media/series-line.png"/></Relationships>"#
        )
        .as_bytes(),
    );
    add_part(&package, "xl/media/series-line.png", IMAGE)
}

fn chart_mut(book: &mut umya_spreadsheet::Workbook) -> &mut Chart {
    &mut book.sheet_mut(0).unwrap().chart_collection_mut()[0]
}

fn write(book: &umya_spreadsheet::Workbook) -> Vec<u8> {
    let mut bytes = Cursor::new(Vec::new());
    writer::xlsx::write_writer(book, &mut bytes).unwrap();
    bytes.into_inner()
}

fn assert_series_line_picture_package(bytes: &[u8]) {
    let chart = String::from_utf8(part(bytes, "xl/charts/chart1.xml")).unwrap();
    assert_eq!(chart.matches("<a:blip ").count(), 1, "{chart}");
    let series_lines = chart
        .split("<c:serLines>")
        .nth(1)
        .unwrap()
        .split("</c:serLines>")
        .next()
        .unwrap();
    assert!(series_lines.contains("<a:blipFill>"), "{series_lines}");
    let relationship_id = series_lines
        .split("r:embed=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();

    let relationships = String::from_utf8(part(bytes, "xl/charts/_rels/chart1.xml.rels")).unwrap();
    let relationship = relationships
        .split("<Relationship")
        .find(|entry| entry.contains(&format!("Id=\"{relationship_id}\"")))
        .unwrap();
    assert!(
        relationship.contains(&format!("Type=\"{CHART_RELATIONSHIPS}/image\"")),
        "{relationship}"
    );
    let target = relationship
        .split("Target=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();
    let media_part = format!("xl/{}", target.strip_prefix("../").unwrap());
    assert_eq!(part(bytes, &media_part), IMAGE);
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let media_parts: Vec<_> = (0..archive.len())
        .map(|index| {
            archive
                .by_index(index)
                .unwrap()
                .name()
                .unwrap()
                .into_owned()
        })
        .filter(|name| name.starts_with("xl/media/"))
        .collect();
    assert_eq!(media_parts, vec![media_part]);
    reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap();
}

#[test]
fn package_writer_plans_bar_and_of_pie_series_line_images() {
    for case in [OF_PIE_CASE, BAR_CASE] {
        let book =
            reader::xlsx::read_reader(Cursor::new(package_with_series_line_picture(case)), true)
                .unwrap();
        assert_series_line_picture_package(&write(&book));
    }
}

#[test]
fn stripped_json_restoration_keeps_bar_and_of_pie_series_line_images() {
    for case in [OF_PIE_CASE, BAR_CASE] {
        let mut book =
            reader::xlsx::read_reader(Cursor::new(package_with_series_line_picture(case)), true)
                .unwrap();
        let previous = chart_mut(&mut book).clone();
        let mut projection = previous.clone();
        projection.strip_export_parts();
        let persisted = serde_json::to_vec(&projection).unwrap();
        let mut restored: Chart = serde_json::from_slice(&persisted).unwrap();
        restored.restore_export_parts(&previous).unwrap();
        *chart_mut(&mut book) = restored;

        assert_series_line_picture_package(&write(&book));
    }
}

#[path = "support/chart_xml_fixture.rs"]
#[allow(dead_code)]
mod fixture;
