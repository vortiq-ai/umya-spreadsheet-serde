use std::io::{
    Cursor,
    Read,
    Write,
};

use umya_spreadsheet::{
    reader,
    writer,
};

#[path = "support/chart_xml_fixture.rs"]
#[allow(dead_code)]
mod fixture;

const CX: &str = "http://schemas.microsoft.com/office/drawing/2014/chartex";
const A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
// A complete, independently authored one-pixel PNG.
const IMAGE: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0,
    0, 0, 31, 21, 196, 137, 0, 0, 0, 11, 73, 68, 65, 84, 120, 156, 99, 0, 1, 0, 0, 5, 0, 1, 13, 10,
    45, 180, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];

fn add(output: &mut zip::ZipWriter<Cursor<Vec<u8>>>, name: &str, data: &[u8]) {
    output
        .start_file(name, zip::write::SimpleFileOptions::default())
        .unwrap();
    output.write_all(data).unwrap();
}

fn package() -> Vec<u8> {
    let mut book = umya_spreadsheet::new_file();
    book.sheet_mut(0).unwrap().add_chart(fixture::base_chart());
    let source = fixture::write_workbook(&book);
    let mut input = zip::ZipArchive::new(Cursor::new(source)).unwrap();
    let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let chart = format!(
        r#"<cx:chartSpace xmlns:cx="{CX}" xmlns:a="{A}" xmlns:r="{R}"><cx:chartData><cx:data id="0"><cx:numDim type="val"><cx:lvl ptCount="1"><cx:pt idx="0">10</cx:pt></cx:lvl></cx:numDim></cx:data></cx:chartData><cx:chart><cx:plotArea><cx:plotAreaRegion><cx:series layoutId="waterfall"><cx:dataId val="0"/></cx:series></cx:plotAreaRegion></cx:plotArea></cx:chart><cx:spPr><a:blipFill><a:blip r:embed="background"/><a:stretch><a:fillRect/></a:stretch></a:blipFill></cx:spPr></cx:chartSpace>"#
    );
    for index in 0..input.len() {
        let mut entry = input.by_index(index).unwrap();
        let name = entry.name().unwrap().to_string();
        let mut data = Vec::new();
        entry.read_to_end(&mut data).unwrap();
        match name.as_str() {
            "xl/charts/chart1.xml" => data = chart.as_bytes().to_vec(),
            "xl/drawings/drawing1.xml" => {
                data = String::from_utf8(data)
                    .unwrap()
                    .replace("http://schemas.openxmlformats.org/drawingml/2006/chart", CX)
                    .into_bytes()
            }
            "xl/drawings/_rels/drawing1.xml.rels" => {
                data = String::from_utf8(data)
                    .unwrap()
                    .replace(
                        &format!("{R}/chart"),
                        "http://schemas.microsoft.com/office/2014/relationships/chartEx",
                    )
                    .into_bytes()
            }
            "[Content_Types].xml" => {
                data = String::from_utf8(data)
                    .unwrap()
                    .replace(
                        "application/vnd.openxmlformats-officedocument.drawingml.chart+xml",
                        "application/vnd.ms-office.chartex+xml",
                    )
                    .replace(
                        "</Types>",
                        r#"<Default Extension="png" ContentType="image/png"/></Types>"#,
                    )
                    .into_bytes()
            }
            _ => (),
        }
        add(&mut output, &name, &data);
    }
    add(&mut output, "xl/charts/_rels/chart1.xml.rels", format!(r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="background" Type="{R}/image" Target="../media/background.png"/></Relationships>"#).as_bytes());
    add(&mut output, "xl/media/background.png", IMAGE);
    output.finish().unwrap().into_inner()
}

fn part(bytes: &[u8], name: &str) -> Vec<u8> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut data = Vec::new();
    archive
        .by_name(name)
        .unwrap()
        .read_to_end(&mut data)
        .unwrap();
    data
}

fn replace_part(bytes: &[u8], target: &str, data: &[u8]) -> Vec<u8> {
    let mut input = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for index in 0..input.len() {
        let mut entry = input.by_index(index).unwrap();
        let name = entry.name().unwrap().to_string();
        let mut original = Vec::new();
        entry.read_to_end(&mut original).unwrap();
        add(
            &mut output,
            &name,
            if name == target { data } else { &original },
        );
    }
    output.finish().unwrap().into_inner()
}

#[test]
fn chart_picture_fill_requires_its_own_unique_image_relationship() {
    let valid = package();
    for relationships in [
        format!(
            r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="other" Type="{R}/image" Target="../media/background.png"/></Relationships>"#
        ),
        format!(
            r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="background" Type="{R}/hyperlink" Target="../media/background.png"/></Relationships>"#
        ),
        format!(
            r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="background" Type="{R}/image" Target="../media/background.png"/><Relationship Id="background" Type="{R}/image" Target="../media/background.png"/></Relationships>"#
        ),
    ] {
        let input = replace_part(
            &valid,
            "xl/charts/_rels/chart1.xml.rels",
            relationships.as_bytes(),
        );
        assert!(
            reader::xlsx::read_reader(Cursor::new(&input), true).is_err(),
            "{relationships}"
        );
        let mut lazy = reader::xlsx::read_reader(Cursor::new(input), false).unwrap();
        assert!(lazy.try_read_sheet(0).is_err(), "{relationships}");
        // A failed native materialization remains retryable, rather than
        // publishing a chart which has discarded its missing image.
        assert!(lazy.try_read_sheet(0).is_err(), "{relationships}");
    }
}

#[test]
fn chart_background_picture_fill_keeps_its_image_relationship_and_bytes() {
    for eager in [true, false] {
        let mut book = reader::xlsx::read_reader(Cursor::new(package()), eager).unwrap();
        book.try_read_sheet(0).unwrap();
        book.sheet_mut(0)
            .unwrap()
            .worksheet_drawing_mut()
            .visit_extended_frames_mut(&mut |_, frame| {
                frame.chart_space_mut().chart_mut().unwrap().series_mut()[0].set_hidden(Some(true));
            });
        let mut output = Vec::new();
        writer::xlsx::write_writer(&book, &mut output).unwrap();
        let chart = String::from_utf8(part(&output, "xl/charts/chart1.xml")).unwrap();
        assert!(
            chart.contains("<a:blipFill"),
            "picture fill was lost: {chart}"
        );
        let relationships =
            String::from_utf8(part(&output, "xl/charts/_rels/chart1.xml.rels")).unwrap();
        let embed = chart
            .split("r:embed=\"")
            .nth(1)
            .unwrap()
            .split('"')
            .next()
            .unwrap();
        assert!(relationships.contains(&format!("Id=\"{embed}\"")));
        assert!(relationships.contains(&format!("{R}/image")));
        assert!(relationships.contains("../media/background.png"));
        assert_eq!(part(&output, "xl/media/background.png"), IMAGE);
        reader::xlsx::read_reader(Cursor::new(output), true).unwrap();
    }
}
