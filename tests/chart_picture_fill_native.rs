#![cfg(feature = "serde")]

use std::io::{
    Cursor,
    Read,
};

use umya_spreadsheet::{
    Chart,
    Workbook,
    drawing::{
        BlipFill,
        charts::{
            ChartExSpace,
            ShapeProperties,
            WorkbookChartExResolver,
        },
        spreadsheet::{
            DrawingAnchor,
            ExtendedChart,
        },
    },
    reader,
    writer,
};

#[path = "support/chart_xml_fixture.rs"]
#[allow(dead_code)]
mod fixture;

const FIRST: &[u8] = include_bytes!("../images/sample1.png");
const SECOND: &[u8] = include_bytes!("../images/sample2.png");

fn picture(data: &[u8]) -> ShapeProperties {
    let mut fill = BlipFill::default();
    fill.blip_mut()
        .image_mut()
        .set_image_name("same.png")
        .set_image_data(data);
    let mut shape = ShapeProperties::default();
    shape.set_blip_fill(fill);
    shape
}

fn extended() -> ChartExSpace {
    ChartExSpace::from_xml(br#"<cx:chartSpace xmlns:cx="http://schemas.microsoft.com/office/drawing/2014/chartex"><cx:chartData><cx:data id="0"><cx:numDim type="val"><cx:lvl ptCount="1"><cx:pt idx="0">10</cx:pt></cx:lvl></cx:numDim></cx:data></cx:chartData><cx:chart><cx:plotArea><cx:plotAreaRegion><cx:series layoutId="waterfall"><cx:dataId val="0"/></cx:series></cx:plotAreaRegion></cx:plotArea></cx:chart></cx:chartSpace>"#).unwrap()
}

fn bytes(book: &Workbook) -> Vec<u8> {
    let mut output = Vec::new();
    writer::xlsx::write_writer(book, &mut output).unwrap();
    output
}

fn single_picture_book(is_extended: bool, data: &[u8]) -> Workbook {
    let mut book = umya_spreadsheet::new_file();
    if is_extended {
        let mut space = extended();
        space.shape_properties = Some(picture(data));
        book.sheet_mut(0)
            .unwrap()
            .worksheet_drawing_mut()
            .add_extended_chart(ExtendedChart::new(DrawingAnchor::default(), space));
    } else {
        let mut chart = fixture::base_chart();
        chart.chart_space_mut().set_shape_properties(picture(data));
        book.sheet_mut(0).unwrap().add_chart(chart);
    }
    book
}

fn image_parts(bytes: &[u8]) -> std::collections::BTreeMap<String, Vec<u8>> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut parts = std::collections::BTreeMap::new();
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).unwrap();
        let name = entry.name().unwrap().to_string();
        if name.starts_with("xl/media/") {
            let mut data = Vec::new();
            entry.read_to_end(&mut data).unwrap();
            parts.insert(name, data);
        }
    }
    parts
}

fn image_relationship_count(bytes: &[u8], chart: &str) -> usize {
    let relations = read_part(bytes, &format!("xl/charts/_rels/{chart}.rels"));
    let mut reader = quick_xml::Reader::from_reader(relations.as_slice());
    let mut count = 0;
    loop {
        match reader.read_event().unwrap() {
            quick_xml::events::Event::Empty(element) => {
                count += usize::from(element.attributes().any(|attribute| {
                    let attribute = attribute.unwrap();
                    attribute.key.as_ref() == b"Type"
                        && attribute.normalized_value(quick_xml::XmlVersion::Implicit1_0).unwrap()
                            == "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image"
                }));
            }
            quick_xml::events::Event::Eof => return count,
            _ => (),
        }
    }
}

#[test]
fn repeated_picture_roundtrips_keep_media_parts_and_relationships_stable() {
    for is_extended in [false, true] {
        let chart = if is_extended {
            "chartEx1.xml"
        } else {
            "chart1.xml"
        };
        let mut output = bytes(&single_picture_book(is_extended, FIRST));
        let initial_parts = image_parts(&output);
        assert_eq!(initial_parts.len(), 1);
        for eager in [true, false, true] {
            let mut book = reader::xlsx::read_reader(Cursor::new(output), eager).unwrap();
            book.try_read_sheet(0).unwrap();
            output = bytes(&book);
            let actual_parts = image_parts(&output);
            assert_eq!(
                actual_parts.keys().collect::<Vec<_>>(),
                initial_parts.keys().collect::<Vec<_>>()
            );
            for (path, expected) in &initial_parts {
                assert!(
                    actual_parts[path] == *expected,
                    "image bytes changed at {path}"
                );
            }
            assert_eq!(image_relationship_count(&output, chart), 1);
            assert_eq!(bound_image(&output, &format!("xl/charts/{chart}")), FIRST);
        }
    }
}

#[test]
fn replacing_a_picture_does_not_retain_unreferenced_old_image_bytes() {
    for is_extended in [false, true] {
        let original = bytes(&single_picture_book(is_extended, FIRST));
        let mut book = reader::xlsx::read_reader(Cursor::new(original), true).unwrap();
        let shape = if is_extended {
            book.sheet_mut(0)
                .unwrap()
                .worksheet_drawing_mut()
                .extended_charts_mut()[0]
                .chart_space_mut()
                .shape_properties
                .as_mut()
                .unwrap()
        } else {
            book.sheet_mut(0).unwrap().chart_collection_mut()[0]
                .chart_space_mut()
                .shape_properties_mut()
                .unwrap()
        };
        shape
            .blip_fill_mut()
            .unwrap()
            .blip_mut()
            .image_mut()
            .set_image_data(SECOND);
        let output = bytes(&book);
        let media = image_parts(&output);
        assert_eq!(media.len(), 1, "obsolete media must not accumulate");
        assert!(media.values().all(|data| data == SECOND));
        let chart = if is_extended {
            "chartEx1.xml"
        } else {
            "chart1.xml"
        };
        assert_eq!(image_relationship_count(&output, chart), 1);
        assert_eq!(bound_image(&output, &format!("xl/charts/{chart}")), SECOND);
    }
}

#[test]
fn missing_picture_auxiliary_edges_fail_before_output_or_file_creation() {
    for is_extended in [false, true] {
        let mut book = single_picture_book(is_extended, FIRST);
        let shape = if is_extended {
            book.sheet_mut(0)
                .unwrap()
                .worksheet_drawing_mut()
                .extended_charts_mut()[0]
                .chart_space_mut()
                .shape_properties
                .as_mut()
                .unwrap()
        } else {
            book.sheet_mut(0).unwrap().chart_collection_mut()[0]
                .chart_space_mut()
                .shape_properties_mut()
                .unwrap()
        };
        shape
            .blip_fill_mut()
            .unwrap()
            .blip_mut()
            .set_link(Some("missing-image-edge".into()));
        let before = serde_json::to_value(book.sheet(0).unwrap().worksheet_drawing()).unwrap();
        let mut output = b"untouched output".to_vec();
        assert!(writer::xlsx::write_writer(&book, &mut output).is_err());
        assert_eq!(output, b"untouched output");
        let missing_parent = std::env::temp_dir().join(format!(
            "umya-picture-preflight-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
        ));
        assert!(!missing_parent.exists());
        let error = writer::xlsx::write(&book, missing_parent.join("book.xlsx")).unwrap_err();
        assert_eq!(error.kind(), umya_spreadsheet::XlsxErrorKind::Xml);
        assert_eq!(
            serde_json::to_value(book.sheet(0).unwrap().worksheet_drawing()).unwrap(),
            before
        );
    }
}

#[test]
fn shared_chart_source_keeps_nonpicture_dependencies_in_both_owner_orders() {
    for cross_sheet in [false, true] {
        for picture_first in [false, true] {
            let original = bytes(&single_picture_book(false, FIRST));
            let mut book = reader::xlsx::read_reader(Cursor::new(original), true).unwrap();
            let mut alias = book.sheet(0).unwrap().chart_collection()[0].clone();
            if !cross_sheet {
                alias
                    .two_cell_anchor_mut()
                    .graphic_frame_mut()
                    .unwrap()
                    .non_visual_graphic_frame_properties_mut()
                    .non_visual_drawing_properties_mut()
                    .set_id(42)
                    .set_name("Shared source alias");
            }
            let change = |chart: &mut Chart, picture_owner: bool| {
                if picture_owner {
                    chart
                        .chart_space_mut()
                        .set_shape_properties(picture(SECOND));
                } else {
                    chart
                        .chart_space_mut()
                        .shape_properties_mut()
                        .unwrap()
                        .set_no_fill(umya_spreadsheet::drawing::NoFill::default());
                }
            };
            change(
                &mut book.sheet_mut(0).unwrap().chart_collection_mut()[0],
                picture_first,
            );
            change(&mut alias, !picture_first);
            if cross_sheet {
                let mut copy = book.sheet(0).unwrap().clone();
                copy.set_name("Other");
                *copy.chart_collection_mut() = vec![alias];
                book.add_sheet(copy).unwrap();
            } else {
                book.sheet_mut(0).unwrap().add_chart(alias);
            }
            let output = bytes(&book);
            let media = image_parts(&output);
            assert_eq!(
                media.len(),
                2,
                "source image is still required by the nonpicture owner's relationship part"
            );
            assert!(media.values().any(|data| data == FIRST));
            assert!(media.values().any(|data| data == SECOND));
            let reloaded = reader::xlsx::read_reader(Cursor::new(output), true).unwrap();
            assert_eq!(
                reloaded.sheet(0).unwrap().chart_collection().len(),
                if cross_sheet { 1 } else { 2 }
            );
        }
    }
}

#[test]
fn streaming_picture_export_preserves_image_identity_without_growth() {
    for is_extended in [false, true] {
        let initial = bytes(&single_picture_book(is_extended, FIRST));
        let expected = image_parts(&initial);
        let mut current = initial;
        for _ in 0..2 {
            let book = reader::xlsx::read_reader(Cursor::new(current), true).unwrap();
            let archive = zip::ZipWriter::new(Cursor::new(Vec::new()));
            let mut stream = writer::streaming_writer::StreamingWriter::new(archive, book);
            let sheet = stream.take_sheet("Sheet1").unwrap();
            stream.flush_sheet(sheet).unwrap();
            current = stream.finish().unwrap().into_inner();
            let media = image_parts(&current);
            assert_eq!(
                media.keys().collect::<Vec<_>>(),
                expected.keys().collect::<Vec<_>>()
            );
            for (path, data) in &expected {
                assert!(
                    media[path] == *data,
                    "streamed image bytes changed at {path}"
                );
            }
            reader::xlsx::read_reader(Cursor::new(&current), true).unwrap();
        }
    }
}

#[test]
fn drawing_and_chart_can_share_the_same_source_image_part() {
    let mut book = single_picture_book(false, FIRST);
    let mut drawing_image = umya_spreadsheet::Image::default();
    let mut marker = umya_spreadsheet::drawing::spreadsheet::MarkerType::default();
    marker.set_coordinate("D4");
    drawing_image.new_image_with_dimensions(32, 32, "chartPicture1.png", FIRST, marker);
    book.sheet_mut(0).unwrap().add_image(drawing_image);
    let input = bytes(&book);
    assert_eq!(
        image_parts(&input).len(),
        1,
        "fixture must genuinely share the media part"
    );
    for eager in [true, false] {
        let mut loaded = reader::xlsx::read_reader(Cursor::new(&input), eager).unwrap();
        loaded.try_read_sheet(0).unwrap();
        let output = bytes(&loaded);
        assert_eq!(image_parts(&output).len(), 1);
        assert_eq!(bound_image(&output, "xl/charts/chart1.xml"), FIRST);
        let reloaded = reader::xlsx::read_reader(Cursor::new(output), true).unwrap();
        assert_eq!(reloaded.sheet(0).unwrap().image_collection().len(), 1);
        assert!(reloaded.sheet(0).unwrap().image_collection()[0].image_data() == FIRST);
    }
}

#[test]
fn streaming_picture_dependency_changes_after_take_are_explicit_errors() {
    for is_extended in [false, true] {
        let input = bytes(&single_picture_book(is_extended, FIRST));
        let book = reader::xlsx::read_reader(Cursor::new(input), true).unwrap();
        let archive = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let mut stream = writer::streaming_writer::StreamingWriter::new(archive, book);
        let mut sheet = stream.take_sheet("Sheet1").unwrap();
        let shape = if is_extended {
            sheet.worksheet_drawing_mut().extended_charts_mut()[0]
                .chart_space_mut()
                .shape_properties
                .as_mut()
                .unwrap()
        } else {
            sheet.chart_collection_mut()[0]
                .chart_space_mut()
                .shape_properties_mut()
                .unwrap()
        };
        shape
            .blip_fill_mut()
            .unwrap()
            .blip_mut()
            .image_mut()
            .set_image_data(SECOND);
        assert!(
            stream.flush_sheet(sheet).is_err(),
            "changed dependencies must not use the constructor's stale plan"
        );
        assert!(
            stream.finish().is_err(),
            "a refused flush must not finish a partial workbook"
        );
    }
}

#[test]
fn streaming_picture_transfer_between_source_owners_is_rejected() {
    let mut initial = single_picture_book(false, FIRST);
    let mut other = fixture::base_chart();
    other
        .two_cell_anchor_mut()
        .graphic_frame_mut()
        .unwrap()
        .non_visual_graphic_frame_properties_mut()
        .non_visual_drawing_properties_mut()
        .set_id(42)
        .set_name("Other source owner");
    other
        .chart_space_mut()
        .set_shape_properties(ShapeProperties::default());
    initial.sheet_mut(0).unwrap().add_chart(other);
    let mut book = reader::xlsx::read_reader(Cursor::new(bytes(&initial)), true).unwrap();
    book.sheet_mut(0).unwrap().chart_collection_mut()[0]
        .chart_space_mut()
        .shape_properties_mut()
        .unwrap()
        .blip_fill_mut()
        .unwrap()
        .blip_mut()
        .image_mut()
        .set_image_data(SECOND);
    let archive = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let mut stream = writer::streaming_writer::StreamingWriter::new(archive, book);
    let mut sheet = stream.take_sheet("Sheet1").unwrap();
    let transferred = sheet.chart_collection()[0]
        .chart_space()
        .shape_properties()
        .unwrap()
        .blip_fill()
        .unwrap()
        .clone();
    sheet.chart_collection_mut()[0]
        .chart_space_mut()
        .shape_properties_mut()
        .unwrap()
        .set_no_fill(umya_spreadsheet::drawing::NoFill::default());
    sheet.chart_collection_mut()[1]
        .chart_space_mut()
        .shape_properties_mut()
        .unwrap()
        .set_blip_fill(transferred);
    assert!(
        stream.flush_sheet(sheet).is_err(),
        "identical image bytes moving to another source owner invalidate the prepared graph"
    );
    assert!(stream.finish().is_err());
}

fn read_part(bytes: &[u8], path: &str) -> Vec<u8> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut data = Vec::new();
    archive
        .by_name(path)
        .unwrap()
        .read_to_end(&mut data)
        .unwrap();
    data
}

fn bound_image(bytes: &[u8], chart_path: &str) -> Vec<u8> {
    let chart = String::from_utf8(read_part(bytes, chart_path)).unwrap();
    let embed = chart
        .split("r:embed=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();
    let (_, name) = chart_path.rsplit_once('/').unwrap();
    let relations = read_part(bytes, &format!("xl/charts/_rels/{name}.rels"));
    let mut xml = quick_xml::Reader::from_reader(relations.as_slice());
    let mut target = None;
    while let Ok(event) = xml.read_event() {
        match event {
            quick_xml::events::Event::Empty(element) => {
                let attrs: std::collections::HashMap<_, _> = element
                    .attributes()
                    .map(|attribute| {
                        let attribute = attribute.unwrap();
                        (
                            attribute.key.as_ref().to_vec(),
                            attribute
                                .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                                .unwrap()
                                .into_owned(),
                        )
                    })
                    .collect();
                if attrs.get(b"Id".as_slice()).map(String::as_str) == Some(embed) {
                    assert_eq!(
                        attrs.get(b"Type".as_slice()).unwrap(),
                        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image"
                    );
                    assert!(target.is_none(), "duplicate image relationship id");
                    target = Some(attrs.get(b"Target".as_slice()).unwrap().clone());
                }
            }
            quick_xml::events::Event::Eof => break,
            _ => (),
        }
    }
    let target = target.unwrap();
    let target = target.strip_prefix("../media/").unwrap();
    read_part(bytes, &format!("xl/media/{target}"))
}

#[test]
fn standard_and_extended_native_images_do_not_deduplicate_by_filename() {
    let mut book = umya_spreadsheet::new_file();
    let mut standard = fixture::base_chart();
    standard
        .chart_space_mut()
        .set_shape_properties(picture(FIRST));
    book.sheet_mut(0).unwrap().add_chart(standard);
    let mut space = extended();
    space.shape_properties = Some(picture(SECOND));
    book.sheet_mut(0)
        .unwrap()
        .worksheet_drawing_mut()
        .add_extended_chart(ExtendedChart::new(DrawingAnchor::default(), space));
    let first = bytes(&book);
    assert_eq!(bound_image(&first, "xl/charts/chart1.xml"), FIRST);
    assert_eq!(bound_image(&first, "xl/charts/chartEx1.xml"), SECOND);
    for eager in [true, false] {
        let mut reloaded = reader::xlsx::read_reader(Cursor::new(&first), eager).unwrap();
        reloaded.try_read_sheet(0).unwrap();
        reloaded.sheet_mut(0).unwrap().chart_collection_mut()[0]
            .chart_space_mut()
            .shape_properties_mut()
            .unwrap()
            .blip_fill_mut()
            .unwrap()
            .blip_mut()
            .image_mut()
            .set_image_data(SECOND);
        let saved = bytes(&reloaded);
        assert_eq!(bound_image(&saved, "xl/charts/chart1.xml"), SECOND);
        assert_eq!(bound_image(&saved, "xl/charts/chartEx1.xml"), SECOND);
        reader::xlsx::read_reader(Cursor::new(saved), true).unwrap();
    }
}

#[test]
fn picture_bytes_and_package_bindings_are_absent_from_browser_projection() {
    let mut authoritative = fixture::base_chart();
    authoritative
        .chart_space_mut()
        .set_shape_properties(picture(FIRST));
    let mut projected = authoritative.clone();
    projected.strip_export_parts();
    assert!(
        projected
            .chart_space()
            .shape_properties()
            .unwrap()
            .blip_fill()
            .is_none()
    );
    let json = serde_json::to_value(&projected).unwrap();
    assert!(!json.to_string().contains("picture_relationship_id"));
    let mut incoming: Chart = serde_json::from_value(json).unwrap();
    incoming.restore_export_parts(&authoritative).unwrap();
    assert_eq!(
        incoming
            .chart_space()
            .shape_properties()
            .unwrap()
            .blip_fill()
            .unwrap()
            .blip()
            .image()
            .image_data(),
        FIRST
    );
    assert_eq!(
        serde_json::to_value(incoming).unwrap(),
        serde_json::to_value(authoritative).unwrap()
    );
}

#[test]
fn supplied_export_bindings_cannot_authorize_standalone_or_package_images() {
    let mut space = extended();
    space.shape_properties = Some(picture(FIRST));
    let mut json = serde_json::to_value(space).unwrap();
    json["shape_properties"]["parts"]["picture_relationship_id"] = "supplied-rId".into();
    let supplied: ChartExSpace = serde_json::from_value(json).unwrap();
    let mut book = umya_spreadsheet::new_file();
    assert!(
        supplied
            .to_xml("Sheet1", &mut WorkbookChartExResolver::new(&book))
            .is_err()
    );
    book.sheet_mut(0)
        .unwrap()
        .worksheet_drawing_mut()
        .add_extended_chart(ExtendedChart::new(DrawingAnchor::default(), supplied));
    let output = bytes(&book);
    let xml = String::from_utf8(read_part(&output, "xl/charts/chartEx1.xml")).unwrap();
    assert!(!xml.contains("supplied-rId"));
    assert_eq!(bound_image(&output, "xl/charts/chartEx1.xml"), FIRST);
}

#[test]
fn blip_legacy_json_and_current_binary_keep_their_distinct_contracts() {
    let current = serde_json::to_value(BlipFill::default().blip()).unwrap();
    let mut legacy = current.clone();
    for key in ["effects", "extensions", "link"] {
        legacy.as_object_mut().unwrap().remove(key);
    }
    let old: umya_spreadsheet::drawing::Blip = serde_json::from_value(legacy).unwrap();
    assert!(old.extensions().is_some());
    let explicit: umya_spreadsheet::drawing::Blip =
        serde_json::from_value(current.clone()).unwrap();
    assert!(explicit.extensions().is_none());
    let encoded = bincode::serialize(&explicit).unwrap();
    let derived_sequence = bincode::serialize(&(
        explicit.image(),
        explicit.cstate(),
        Vec::<umya_spreadsheet::drawing::BlipEffect>::new(),
        None::<Box<umya_spreadsheet::drawing::ExtensionList>>,
        None::<String>,
    ))
    .unwrap();
    assert_eq!(
        encoded, derived_sequence,
        "current derived field sequence must not change"
    );
    let decoded: umya_spreadsheet::drawing::Blip = bincode::deserialize(&encoded).unwrap();
    assert_eq!(serde_json::to_value(decoded).unwrap(), current);
}

#[test]
fn copies_of_an_imported_chart_with_different_pictures_each_keep_their_own_image() {
    let imported =
        reader::xlsx::read_reader(Cursor::new(bytes(&single_picture_book(true, FIRST))), true)
            .unwrap();
    let original = imported
        .sheet(0)
        .unwrap()
        .worksheet_drawing()
        .extended_charts()[0]
        .clone();
    assert!(original.frame().source_part().is_some());
    let mut fresh = umya_spreadsheet::new_file();
    for (frame_id, picture) in [(10, FIRST), (11, SECOND), (12, FIRST)] {
        let mut copy = original.clone();
        copy.frame_mut()
            .frame_properties_mut()
            .non_visual_drawing_properties_mut()
            .set_id(frame_id);
        copy.frame_mut()
            .chart_space_mut()
            .shape_properties
            .as_mut()
            .unwrap()
            .blip_fill_mut()
            .unwrap()
            .blip_mut()
            .image_mut()
            .set_image_data(picture);
        fresh
            .sheet_mut(0)
            .unwrap()
            .worksheet_drawing_mut()
            .add_extended_chart(copy);
    }
    let written = bytes(&fresh);
    let reread = reader::xlsx::read_reader(Cursor::new(&written), true).unwrap();
    let charts = reread
        .sheet(0)
        .unwrap()
        .worksheet_drawing()
        .extended_charts();
    assert_eq!(charts.len(), 3);
    let bound: Vec<Vec<u8>> = charts
        .iter()
        .map(|chart| bound_image(&written, chart.frame().source_part().unwrap()))
        .collect();
    assert_eq!(bound[0], FIRST);
    assert_eq!(bound[1], SECOND);
    assert_eq!(bound[2], FIRST);
    // The two copies with the same picture are the same chart and share a
    // part; the copy with another picture has its own.
    let parts: Vec<_> = charts
        .iter()
        .map(|chart| chart.frame().source_part().unwrap().to_owned())
        .collect();
    assert_ne!(parts[0], parts[1]);
    assert_eq!(parts[0], parts[2]);
}
