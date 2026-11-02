#![cfg(feature = "serde")]

use std::{
    collections::HashSet,
    io::{
        Cursor,
        Read,
        Write,
    },
};

use quick_xml::{
    Reader,
    events::Event,
};
use umya_spreadsheet::{
    Workbook,
    reader,
    writer,
};

const RICH: &[u8] = include_bytes!("fixtures/chart_shapes_rich_excel.xlsx");
const SIMPLE: &[u8] = include_bytes!("fixtures/chart_drawing_excel.xlsx");
const USER_SHAPES: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/chartUserShapes";
const IMAGE: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image";
const STYLE: &str = "http://schemas.microsoft.com/office/2011/relationships/chartStyle";
const COLORS: &str = "http://schemas.microsoft.com/office/2011/relationships/chartColorStyle";

fn open(data: &[u8]) -> Workbook {
    reader::xlsx::read_reader(Cursor::new(data), true).unwrap()
}

fn save(book: &Workbook, light: bool, label: &str) -> Vec<u8> {
    let mut output = Vec::new();
    if light {
        writer::xlsx::write_writer_light(book, &mut output).unwrap();
    } else {
        writer::xlsx::write_writer(book, &mut output).unwrap();
    }
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target/chart-drawing-validation");
    std::fs::create_dir_all(&path).unwrap();
    std::fs::write(path.join(format!("{label}-{light}.xlsx")), &output).unwrap();
    output
}

fn entry(data: &[u8], name: &str) -> Vec<u8> {
    let mut zip = zip::ZipArchive::new(Cursor::new(data)).unwrap();
    let mut result = Vec::new();
    zip.by_name(name)
        .unwrap_or_else(|_| panic!("missing {name}"))
        .read_to_end(&mut result)
        .unwrap();
    result
}

fn names(data: &[u8]) -> Vec<String> {
    let mut zip = zip::ZipArchive::new(Cursor::new(data)).unwrap();
    (0..zip.len())
        .map(|i| zip.by_index(i).unwrap().name().unwrap().into_owned())
        .collect()
}

fn resolve(owner: &str, target: &str) -> String {
    let mut parts: Vec<_> = owner.split('/').collect();
    parts.pop();
    for part in target.split('/') {
        match part {
            ".." => {
                parts.pop().unwrap();
            }
            "." => (),
            value => parts.push(value),
        }
    }
    parts.join("/")
}

fn relationships(data: &[u8], owner: &str) -> Vec<(String, String, String)> {
    let (dir, file) = owner.rsplit_once('/').unwrap();
    let xml = entry(data, &format!("{dir}/_rels/{file}.rels"));
    let mut reader = Reader::from_reader(xml.as_slice());
    let mut result = Vec::new();
    loop {
        match reader.read_event().unwrap() {
            Event::Empty(e) | Event::Start(e) if e.local_name().as_ref() == b"Relationship" => {
                let get = |key: &[u8]| {
                    e.attributes()
                        .map(Result::unwrap)
                        .find(|a| a.key.as_ref() == key)
                        .map(|a| {
                            a.normalized_value(quick_xml::XmlVersion::Implicit1_0)
                                .unwrap()
                                .into_owned()
                        })
                        .unwrap()
                };
                result.push((get(b"Id"), get(b"Type"), resolve(owner, &get(b"Target"))));
            }
            Event::Eof => break,
            _ => (),
        }
    }
    result
}

fn verify_pictures(data: &[u8], charts: usize) {
    let parts = names(data);
    assert_eq!(
        parts.len(),
        parts.iter().collect::<HashSet<_>>().len(),
        "no duplicate ZIP entries"
    );
    let charts_found: Vec<_> = parts
        .iter()
        .filter(|p| p.starts_with("xl/charts/chart") && p.ends_with(".xml"))
        .collect();
    assert_eq!(charts_found.len(), charts);
    let original = String::from_utf8(entry(RICH, "xl/drawings/drawing2.xml")).unwrap();
    let picture = &original[original.find("<cdr:pic>").unwrap()
        ..original.find("</cdr:pic>").unwrap() + "</cdr:pic>".len()];
    let expected_png = entry(RICH, "xl/media/image1.png");
    let mut drawings = HashSet::new();
    let mut styles = HashSet::new();
    let mut colors = HashSet::new();
    for chart in charts_found {
        let edges = relationships(data, chart);
        for (kind, source, owned) in [
            (STYLE, "xl/charts/style1.xml", &mut styles),
            (COLORS, "xl/charts/colors1.xml", &mut colors),
        ] {
            let target = &edges
                .iter()
                .find(|(_, relation, _)| relation == kind)
                .unwrap_or_else(|| panic!("missing {kind} on {chart}"))
                .2;
            assert!(
                owned.insert(target.clone()),
                "Excel requires private style and color parts per chart"
            );
            assert_eq!(entry(data, target), entry(RICH, source));
        }
        let (id, _, drawing) = edges
            .iter()
            .find(|(_, kind, _)| kind == USER_SHAPES)
            .unwrap();
        let xml = String::from_utf8(entry(data, chart)).unwrap();
        assert!(xml.contains(&format!("<c:userShapes r:id=\"{id}\"/>")));
        assert!(
            drawings.insert(drawing.clone()),
            "each chart owns its drawing"
        );
        let xml = String::from_utf8(entry(data, drawing)).unwrap();
        assert_eq!(xml.matches("<cdr:relSizeAnchor>").count(), 6);
        assert!(
            xml.contains(picture),
            "opaque picture bytes must match Excel"
        );
        let edges = relationships(data, drawing);
        let (id, _, image) = edges.iter().find(|(_, kind, _)| kind == IMAGE).unwrap();
        assert_eq!(id, "rId1");
        assert_eq!(entry(data, image), expected_png);
        assert!(
            String::from_utf8(entry(data, "[Content_Types].xml"))
                .unwrap()
                .contains("image/png")
        );
    }
    let book = open(data);
    let count: usize = book
        .sheet_collection_no_check()
        .iter()
        .map(|sheet| sheet.chart_collection().len())
        .sum();
    assert_eq!(count, charts);
}

#[test]
fn charts_cloned_on_one_sheet_have_independent_parent_parts() {
    let mut book = open(SIMPLE);
    let chart = book.sheet(0).unwrap().chart_collection()[0].clone();
    book.sheet_mut(0).unwrap().add_chart(chart);
    for light in [false, true] {
        let data = save(&book, light, "same-sheet");
        let charts = names(&data)
            .into_iter()
            .filter(|p| p.starts_with("xl/charts/chart") && p.ends_with(".xml"))
            .collect::<Vec<_>>();
        assert_eq!(charts.len(), 2);
        let mut drawings = HashSet::new();
        for chart in charts {
            let drawing = relationships(&data, &chart)
                .into_iter()
                .find(|(_, kind, _)| kind == USER_SHAPES)
                .unwrap()
                .2;
            assert!(drawings.insert(drawing.clone()));
            let xml = String::from_utf8(entry(&data, &drawing)).unwrap();
            assert_eq!(xml.matches("<cdr:relSizeAnchor>").count(), 3);
            assert!(xml.contains("<a:t>Hello chart</a:t>"));
        }
        assert_eq!(open(&data).sheet(0).unwrap().chart_collection().len(), 2);
    }
}

#[test]
fn rich_chart_drawing_graph_survives_round_trip_and_copies() {
    let source = open(RICH);
    for light in [false, true] {
        verify_pictures(&save(&source, light, "rich-roundtrip"), 1);
    }
    let chart = source.sheet(0).unwrap().chart_collection()[0].clone();
    let mut same_sheet = source.clone();
    same_sheet.sheet_mut(0).unwrap().add_chart(chart.clone());
    let mut same_book = source.clone();
    same_book
        .new_sheet("Copy")
        .unwrap()
        .add_chart(chart.clone());
    let mut other_book = umya_spreadsheet::new_file();
    other_book.sheet_mut(0).unwrap().add_chart(chart.clone());
    let persisted: umya_spreadsheet::Chart =
        serde_json::from_str(&serde_json::to_string(&chart).unwrap()).unwrap();
    let mut persisted_book = umya_spreadsheet::new_file();
    persisted_book.sheet_mut(0).unwrap().add_chart(persisted);
    for light in [false, true] {
        verify_pictures(&save(&same_sheet, light, "rich-same-sheet"), 2);
        verify_pictures(&save(&same_book, light, "rich-other-sheet"), 2);
        verify_pictures(&save(&other_book, light, "rich-other-workbook"), 1);
        verify_pictures(&save(&persisted_book, light, "rich-persisted"), 1);
    }
}

#[test]
fn lazy_copies_own_their_media_after_the_source_file_is_removed() {
    let path = std::env::temp_dir().join(format!("umya-rich-copy-{}.xlsx", std::process::id()));
    std::fs::write(&path, RICH).unwrap();
    let mut source = reader::xlsx::lazy_read(&path).unwrap();
    source.try_read_sheet(0).unwrap();
    let chart = source.sheet(0).unwrap().chart_collection()[0].clone();
    drop(source);
    std::fs::remove_file(path).unwrap();
    let mut target = umya_spreadsheet::new_file();
    target.sheet_mut(0).unwrap().add_chart(chart);
    for light in [false, true] {
        verify_pictures(&save(&target, light, "rich-lazy-copy"), 1);
    }
}

#[test]
fn drawing_dependency_graph_is_restored_from_the_authoritative_chart() {
    let source = open(RICH);
    let original = source.sheet(0).unwrap().chart_collection()[0].clone();
    let mut projected = original.clone();
    projected.strip_export_parts();
    let json = serde_json::to_string(&projected).unwrap();
    assert!(
        !json.contains("image1.png"),
        "package identities are hidden"
    );
    let mut incoming = serde_json::to_value(&original).unwrap();
    assert!(
        !json.contains("rId1"),
        "existing relationship bindings are hidden"
    );
    fn corrupt(value: &mut serde_json::Value) {
        match value {
            serde_json::Value::Object(map) => {
                if let Some(parts) = map
                    .get_mut("dependency_parts")
                    .filter(|value| !value.is_null())
                {
                    let files = parts["files"].as_array_mut().unwrap();
                    assert_eq!(files.len(), 1, "Excel fixture has one picture dependency");
                    files[0]["file_data"] = serde_json::json!([0, 1, 2, 3]);
                }
                for value in map.values_mut() {
                    corrupt(value);
                }
            }
            serde_json::Value::Array(values) => {
                for value in values {
                    corrupt(value);
                }
            }
            serde_json::Value::String(value) if value == "rId1" => *value = "rId999".to_owned(),
            _ => (),
        }
    }
    corrupt(&mut incoming);
    let mut returned: umya_spreadsheet::Chart = serde_json::from_value(incoming).unwrap();
    returned.restore_export_parts(&original).unwrap();
    let mut target = umya_spreadsheet::new_file();
    target.sheet_mut(0).unwrap().add_chart(returned);
    for light in [false, true] {
        verify_pictures(&save(&target, light, "rich-restored"), 1);
    }
}

#[test]
fn replacing_a_drawing_transfers_its_picture_relationships() {
    let mut replacement_image = entry(RICH, "xl/media/image1.png");
    replacement_image.extend_from_slice(b"replacement picture");
    let mut input = zip::ZipArchive::new(Cursor::new(RICH)).unwrap();
    let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for index in 0..input.len() {
        let mut member = input.by_index(index).unwrap();
        let name = member.name().unwrap().into_owned();
        let mut data = Vec::new();
        member.read_to_end(&mut data).unwrap();
        if name == "xl/media/image1.png" {
            data.clone_from(&replacement_image);
        } else if name == "xl/drawings/drawing2.xml" {
            data = String::from_utf8(data)
                .unwrap()
                .replace("name=\"Pic\"", "name=\"Replacement picture\"")
                .into_bytes();
        }
        output
            .start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        output.write_all(&data).unwrap();
    }
    let replacement = output.finish().unwrap().into_inner();
    for eager in [true, false] {
        let mut book = reader::xlsx::read_reader(Cursor::new(RICH), eager).unwrap();
        let mut source = reader::xlsx::read_reader(Cursor::new(&replacement), eager).unwrap();
        book.try_read_sheet(0).unwrap();
        source.try_read_sheet(0).unwrap();
        let donor = source.sheet(0).unwrap().chart_collection()[0].clone();
        let drawing = donor.chart_space().chart_drawing().unwrap().clone();
        let drawing: umya_spreadsheet::drawing::charts::ChartDrawing =
            serde_json::from_str(&serde_json::to_string(&drawing).unwrap()).unwrap();
        book.sheet_mut(0).unwrap().add_chart(donor);
        drop(source);
        for legacy in [false, true] {
            for mutable in [false, true] {
                let mut book = book.clone();
                let chart = &mut book.sheet_mut(0).unwrap().chart_collection_mut()[0];
                if legacy {
                    *chart = legacy_drawing_graph(chart);
                }
                if mutable {
                    *chart.chart_space_mut().chart_drawing_mut() = drawing.clone();
                } else {
                    chart.chart_space_mut().set_chart_drawing(drawing.clone());
                }
                for light in [false, true] {
                    let bytes = save(
                        &book,
                        light,
                        &format!("replacement-drawing-{eager}-{legacy}-{mutable}"),
                    );
                    for chart in ["xl/charts/chart1.xml", "xl/charts/chart2.xml"] {
                        let drawing = relationships(&bytes, chart)
                            .into_iter()
                            .find(|(_, kind, _)| kind == USER_SHAPES)
                            .unwrap()
                            .2;
                        let xml = String::from_utf8(entry(&bytes, &drawing)).unwrap();
                        assert!(xml.contains("name=\"Replacement picture\""));
                        let (id, _, image) = relationships(&bytes, &drawing)
                            .into_iter()
                            .find(|(_, kind, _)| kind == IMAGE)
                            .unwrap();
                        assert_eq!(id, "rId1");
                        assert_eq!(entry(&bytes, &image), replacement_image);
                    }
                    assert_eq!(open(&bytes).sheet(0).unwrap().chart_collection().len(), 2);
                }
            }
        }
    }
}

fn legacy_drawing_graph(chart: &umya_spreadsheet::Chart) -> umya_spreadsheet::Chart {
    fn relocate(value: &mut serde_json::Value) -> usize {
        match value {
            serde_json::Value::Object(map) => {
                if let Some(drawing) = map.get_mut("chart_drawing").and_then(|v| v.as_object_mut())
                {
                    let mut graph = drawing.remove("dependency_parts").unwrap();
                    assert!(!graph.is_null());
                    graph["chart_style_bundle"] =
                        map["chart_drawing_parts"]["chart_style_bundle"].clone();
                    map.insert("chart_drawing_parts".to_owned(), graph);
                    1
                } else {
                    map.values_mut().map(relocate).sum()
                }
            }
            serde_json::Value::Array(values) => values.iter_mut().map(relocate).sum(),
            _ => 0,
        }
    }
    let mut json = serde_json::to_value(chart).unwrap();
    assert_eq!(relocate(&mut json), 1);
    serde_json::from_value(json).unwrap()
}

#[test]
fn legacy_drawing_graphs_export_and_restore_without_losing_media() {
    let source = open(RICH);
    let chart = &source.sheet(0).unwrap().chart_collection()[0];
    let legacy = legacy_drawing_graph(chart);
    let mut projected = legacy.clone();
    projected.strip_export_parts();
    projected.restore_export_parts(&legacy).unwrap();
    for (label, chart) in [
        ("rich-legacy-graph", legacy),
        ("rich-legacy-restored", projected),
    ] {
        let mut target = umya_spreadsheet::new_file();
        target.sheet_mut(0).unwrap().add_chart(chart);
        for light in [false, true] {
            verify_pictures(&save(&target, light, label), 1);
        }
    }
}

#[test]
fn legacy_removed_drawings_cannot_supply_dependencies_to_an_unbound_replacement() {
    fn remove_drawing(value: &mut serde_json::Value) {
        match value {
            serde_json::Value::Object(map) => {
                if let Some(drawing) = map.get_mut("chart_drawing") {
                    *drawing = serde_json::Value::Null;
                } else {
                    for value in map.values_mut() {
                        remove_drawing(value);
                    }
                }
            }
            serde_json::Value::Array(values) => values.iter_mut().for_each(remove_drawing),
            _ => (),
        }
    }
    let source = open(RICH);
    let chart = &source.sheet(0).unwrap().chart_collection()[0];
    let mut removed = serde_json::to_value(legacy_drawing_graph(chart)).unwrap();
    remove_drawing(&mut removed);
    let mut removed: umya_spreadsheet::Chart = serde_json::from_value(removed).unwrap();
    let mut unbound = serde_json::to_value(chart.chart_space().chart_drawing().unwrap()).unwrap();
    unbound.as_object_mut().unwrap().remove("dependency_parts");
    *removed.chart_space_mut().chart_drawing_mut() = serde_json::from_value(unbound).unwrap();
    let mut target = umya_spreadsheet::new_file();
    target.sheet_mut(0).unwrap().add_chart(removed);
    for light in [false, true] {
        let mut output = Vec::new();
        let error = if light {
            writer::xlsx::write_writer_light(&target, &mut output)
        } else {
            writer::xlsx::write_writer(&target, &mut output)
        }
        .unwrap_err();
        assert!(error.to_string().contains("relationship rId1"), "{error}");
    }
}

#[test]
fn copied_pictures_keep_their_owner_when_package_names_collide() {
    let mut zip = zip::ZipArchive::new(Cursor::new(RICH)).unwrap();
    let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let original_image = entry(RICH, "xl/media/image1.png");
    let mut other_image = original_image.clone();
    // PNG decoders permit trailing bytes; keep valid image data with a distinct
    // payload.
    other_image.extend_from_slice(b"another workbook");
    for i in 0..zip.len() {
        let mut member = zip.by_index(i).unwrap();
        let name = member.name().unwrap().into_owned();
        let mut data = Vec::new();
        member.read_to_end(&mut data).unwrap();
        if name == "xl/media/image1.png" {
            data.clone_from(&other_image);
        }
        output
            .start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        output.write_all(&data).unwrap();
    }
    let mut target = open(&output.finish().unwrap().into_inner());
    let source = open(RICH);
    target
        .sheet_mut(0)
        .unwrap()
        .add_chart(source.sheet(0).unwrap().chart_collection()[0].clone());
    for light in [false, true] {
        let data = save(&target, light, "picture-name-collision");
        for (chart, expected) in [
            ("xl/charts/chart1.xml", &other_image),
            ("xl/charts/chart2.xml", &original_image),
        ] {
            let drawing = relationships(&data, chart)
                .into_iter()
                .find(|(_, kind, _)| kind == USER_SHAPES)
                .unwrap()
                .2;
            let image = relationships(&data, &drawing)
                .into_iter()
                .find(|(_, kind, _)| kind == IMAGE)
                .unwrap()
                .2;
            assert_eq!(entry(&data, &image), *expected);
        }
        assert_eq!(open(&data).sheet(0).unwrap().chart_collection().len(), 2);
    }
}

#[test]
fn duplicate_imported_chart_ids_are_rejected_instead_of_reassigned() {
    let mut input = zip::ZipArchive::new(Cursor::new(SIMPLE)).unwrap();
    let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for i in 0..input.len() {
        let mut member = input.by_index(i).unwrap();
        let name = member.name().unwrap().into_owned();
        let mut data = Vec::new();
        member.read_to_end(&mut data).unwrap();
        if name == "xl/drawings/drawing1.xml" {
            let xml = String::from_utf8(data).unwrap();
            let start = xml.find("<xdr:twoCellAnchor>").unwrap();
            let end = xml.find("</xdr:twoCellAnchor>").unwrap() + "</xdr:twoCellAnchor>".len();
            data = format!("{}{}{}", &xml[..end], &xml[start..end], &xml[end..]).into_bytes();
        }
        output
            .start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        output.write_all(&data).unwrap();
    }
    let bytes = output.finish().unwrap().into_inner();
    let error = reader::xlsx::read_reader(Cursor::new(&bytes), true).unwrap_err();
    assert!(error.to_string().contains("duplicate"), "{error}");
    let mut lazy = reader::xlsx::read_reader(Cursor::new(&bytes), false).unwrap();
    let error = lazy.try_read_sheet(0).unwrap_err();
    assert!(error.to_string().contains("duplicate"), "{error}");
}

#[test]
fn removing_a_copied_drawing_keeps_private_parent_style_ownership() {
    let mut book = open(RICH);
    book.sheet_mut(0).unwrap().chart_collection_mut()[0]
        .chart_space_mut()
        .remove_chart_drawing();
    let chart = book.sheet(0).unwrap().chart_collection()[0].clone();
    book.sheet_mut(0).unwrap().add_chart(chart);
    for light in [false, true] {
        let data = save(&book, light, "rich-removed-copy");
        let mut styles = HashSet::new();
        let mut colors = HashSet::new();
        for chart in ["xl/charts/chart1.xml", "xl/charts/chart2.xml"] {
            assert!(
                !String::from_utf8(entry(&data, chart))
                    .unwrap()
                    .contains("userShapes")
            );
            for (_, kind, target) in relationships(&data, chart) {
                let (owned, expected) = if kind == STYLE {
                    (&mut styles, "xl/charts/style1.xml")
                } else if kind == COLORS {
                    (&mut colors, "xl/charts/colors1.xml")
                } else {
                    panic!("unexpected relationship {kind}");
                };
                assert!(owned.insert(target.clone()));
                assert_eq!(entry(&data, &target), entry(RICH, expected));
            }
        }
        assert_eq!(styles.len(), 2);
        assert_eq!(colors.len(), 2);
        assert_eq!(open(&data).sheet(0).unwrap().chart_collection().len(), 2);
    }
}

#[test]
fn a_chart_without_its_drawing_model_copies_its_preserved_drawing_part() {
    // JSON saved before the drawing model existed has no `chart_drawing`: the
    // chart keeps the drawing part it was read with, and a copy of it on
    // another sheet owns an exact copy of that part.
    let mut book = open(SIMPLE);
    let chart = book.sheet(0).unwrap().chart_collection()[0].clone();
    let mut value = serde_json::to_value(&chart).unwrap();
    let space = value["two_cell_anchor"]["graphic_frame"]["graphic"]["graphic_data"]["chart_space"]
        .as_object_mut()
        .unwrap();
    assert!(space.remove("chart_drawing").is_some());
    assert!(space.remove("chart_drawing_parts").is_some());
    let legacy: umya_spreadsheet::Chart = serde_json::from_value(value).unwrap();
    assert!(legacy.chart_space().chart_drawing().is_none());
    assert!(legacy.chart_space().user_shapes().is_some());
    let mut returned = legacy.clone();
    returned.strip_export_parts();
    let mut returned: umya_spreadsheet::Chart =
        serde_json::from_str(&serde_json::to_string(&returned).unwrap()).unwrap();
    returned.restore_export_parts(&legacy).unwrap();
    assert!(returned.chart_space().user_shapes().is_some());
    book.new_sheet("Copy").unwrap().add_chart(returned);
    for light in [false, true] {
        let data = save(&book, light, "legacy-copy");
        let mut drawings = Vec::new();
        for chart in ["xl/charts/chart1.xml", "xl/charts/chart2.xml"] {
            let edges = relationships(&data, chart);
            assert_eq!(edges.len(), 1, "{chart}: {edges:?}");
            let (id, kind, target) = &edges[0];
            assert_eq!(kind, USER_SHAPES);
            let xml = String::from_utf8(entry(&data, chart)).unwrap();
            assert!(
                xml.contains(&format!("<c:userShapes r:id=\"{id}\"/>")),
                "{xml}"
            );
            drawings.push(target.clone());
        }
        assert_ne!(drawings[0], drawings[1], "each chart owns its drawing part");
        assert_eq!(
            entry(&data, &drawings[1]),
            entry(SIMPLE, "xl/drawings/drawing2.xml"),
            "the copy holds the preserved part byte for byte"
        );
        let reread = open(&data);
        for sheet in ["Sheet1", "Copy"] {
            let space = reread.sheet_by_name(sheet).unwrap().chart_collection()[0].chart_space();
            assert_eq!(space.chart_drawing().unwrap().anchors().len(), 3, "{sheet}");
        }
    }
}

#[test]
fn a_copied_chart_with_shapes_and_a_chart_area_picture_keeps_both() {
    let mut source = open(RICH);
    // The drawing's own picture is image1.png; the chart-area picture must
    // differ so each owner's bytes are checked separately (bytes after IEND
    // are ignored).
    let drawing_picture = entry(RICH, "xl/media/image1.png");
    let mut chart_area_picture = drawing_picture.clone();
    chart_area_picture.extend_from_slice(b"chart-area");
    let mut fill = umya_spreadsheet::drawing::BlipFill::default();
    fill.blip_mut()
        .image_mut()
        .set_image_name("chart-area.png")
        .set_image_data(chart_area_picture.clone());
    let mut properties = umya_spreadsheet::drawing::charts::ShapeProperties::default();
    properties.set_blip_fill(fill);
    source.sheet_mut(0).unwrap().chart_collection_mut()[0]
        .chart_space_mut()
        .set_shape_properties(properties);
    let fixture_drawing = String::from_utf8(entry(RICH, "xl/drawings/drawing2.xml")).unwrap();
    let anchors = ["<cdr:relSizeAnchor", "<cdr:absSizeAnchor"]
        .iter()
        .map(|tag| fixture_drawing.matches(tag).count())
        .sum::<usize>();
    assert!(anchors > 0, "{fixture_drawing}");
    let mut target = umya_spreadsheet::new_file();
    target
        .sheet_mut(0)
        .unwrap()
        .add_chart(source.sheet(0).unwrap().chart_collection()[0].clone());
    for light in [false, true] {
        let data = save(&target, light, "chart-area-picture-copy");
        let edges = relationships(&data, "xl/charts/chart1.xml");
        let shapes: Vec<_> = edges
            .iter()
            .filter(|(_, kind, _)| kind == USER_SHAPES)
            .collect();
        assert_eq!(shapes.len(), 1, "{edges:?}");
        let images: Vec<_> = edges.iter().filter(|(_, kind, _)| kind == IMAGE).collect();
        assert_eq!(images.len(), 1, "{edges:?}");
        assert_ne!(shapes[0].0, images[0].0, "one relationship id per edge");
        let xml = String::from_utf8(entry(&data, "xl/charts/chart1.xml")).unwrap();
        assert!(
            xml.contains(&format!("<c:userShapes r:id=\"{}\"/>", shapes[0].0)),
            "{xml}"
        );
        assert!(
            xml.contains(&format!("r:embed=\"{}\"", images[0].0)),
            "{xml}"
        );
        assert_eq!(entry(&data, &images[0].2), chart_area_picture);
        let drawing_edges = relationships(&data, &shapes[0].2);
        let drawing_image = &drawing_edges
            .iter()
            .find(|(_, kind, _)| kind == IMAGE)
            .unwrap()
            .2;
        assert_eq!(entry(&data, drawing_image), drawing_picture);
        let reread = open(&data);
        let space = reread.sheet(0).unwrap().chart_collection()[0].chart_space();
        assert_eq!(space.chart_drawing().unwrap().anchors().len(), anchors);
        assert_eq!(
            space
                .shape_properties()
                .unwrap()
                .blip_fill()
                .unwrap()
                .blip()
                .image()
                .image_data(),
            chart_area_picture
        );
    }
}

fn projected_without_drawing(chart: &umya_spreadsheet::Chart) -> umya_spreadsheet::Chart {
    let mut projected = chart.clone();
    projected.strip_export_parts();
    let mut value = serde_json::to_value(&projected).unwrap();
    let space =
        &mut value["two_cell_anchor"]["graphic_frame"]["graphic"]["graphic_data"]["chart_space"];
    assert!(space["chart_drawing"].is_object());
    assert!(space["user_shapes"].is_object());
    space["chart_drawing"] = serde_json::Value::Null;
    let mut returned: umya_spreadsheet::Chart =
        serde_json::from_str(&serde_json::to_string(&value).unwrap()).unwrap();
    returned.restore_export_parts(chart).unwrap();
    returned
}

#[test]
fn a_null_projected_drawing_removes_its_package_graph() {
    for (label, fixture) in [("simple", SIMPLE), ("picture", RICH)] {
        for eager in [false, true] {
            let mut book = reader::xlsx::read_reader(Cursor::new(fixture), eager).unwrap();
            book.try_read_sheet(0).unwrap();
            let original = book.sheet(0).unwrap().chart_collection()[0].clone();
            book.sheet_mut(0).unwrap().chart_collection_mut()[0] =
                projected_without_drawing(&original);
            for light in [false, true] {
                let data = save(&book, light, &format!("projection-null-{label}-{eager}"));
                let parts = names(&data);
                let xml = String::from_utf8(entry(&data, "xl/charts/chart1.xml")).unwrap();
                assert!(!xml.contains("userShapes"), "{xml}");
                for part in parts.iter().filter(|part| part.ends_with(".rels")) {
                    assert!(
                        !String::from_utf8(entry(&data, part))
                            .unwrap()
                            .contains(USER_SHAPES)
                    );
                }
                let types = String::from_utf8(entry(&data, "[Content_Types].xml")).unwrap();
                assert!(!types.contains("chartshapes"), "{types}");
                assert!(!parts.iter().any(|part| part.starts_with("xl/media/")));
                assert!(
                    !parts
                        .iter()
                        .any(|part| part.starts_with("xl/drawings/_rels/drawing2"))
                );
                assert!(!parts.iter().any(|part| part == "xl/drawings/drawing2.xml"));
                assert!(
                    open(&data).sheet(0).unwrap().chart_collection()[0]
                        .chart_space()
                        .chart_drawing()
                        .is_none()
                );
            }
        }
    }
}

#[test]
fn a_null_projected_drawing_does_not_take_a_sibling_charts_drawing() {
    for eager in [false, true] {
        let mut book = reader::xlsx::read_reader(Cursor::new(RICH), eager).unwrap();
        book.try_read_sheet(0).unwrap();
        let original = book.sheet(0).unwrap().chart_collection()[0].clone();
        let mut sibling = original.clone();
        sibling.chart_space_mut().chart_drawing_mut().add_text_box(
            "Sibling only",
            umya_spreadsheet::drawing::charts::ChartDrawingPoint::new(0.1, 0.1),
            umya_spreadsheet::drawing::charts::ChartDrawingPoint::new(0.5, 0.5),
        );
        book.sheet_mut(0).unwrap().add_chart(sibling);
        book.sheet_mut(0).unwrap().chart_collection_mut()[0] = projected_without_drawing(&original);
        for light in [false, true] {
            let data = save(&book, light, &format!("projection-null-sibling-{eager}"));
            let first = String::from_utf8(entry(&data, "xl/charts/chart1.xml")).unwrap();
            assert!(!first.contains("userShapes"));
            assert!(
                relationships(&data, "xl/charts/chart1.xml")
                    .iter()
                    .all(|(_, kind, _)| kind != USER_SHAPES)
            );
            let edges = relationships(&data, "xl/charts/chart2.xml");
            let drawings: Vec<_> = edges
                .iter()
                .filter(|(_, kind, _)| kind == USER_SHAPES)
                .collect();
            assert_eq!(drawings.len(), 1);
            let (id, _, drawing) = drawings[0];
            let second = String::from_utf8(entry(&data, "xl/charts/chart2.xml")).unwrap();
            assert!(second.contains(&format!("<c:userShapes r:id=\"{id}\"/>")));
            let xml = String::from_utf8(entry(&data, drawing)).unwrap();
            assert_eq!(xml.matches("<cdr:relSizeAnchor>").count(), 7);
            assert!(xml.contains("Sibling only"));
            let image = relationships(&data, drawing)
                .into_iter()
                .find(|(_, kind, _)| kind == IMAGE)
                .unwrap()
                .2;
            assert_eq!(entry(&data, &image), entry(RICH, "xl/media/image1.png"));
            let types = String::from_utf8(entry(&data, "[Content_Types].xml")).unwrap();
            assert_eq!(
                types
                    .matches(
                        "application/vnd.openxmlformats-officedocument.drawingml.chartshapes+xml"
                    )
                    .count(),
                1
            );
            let reread = open(&data);
            let charts = reread.sheet(0).unwrap().chart_collection();
            assert_eq!(charts.len(), 2);
            assert!(charts[0].chart_space().chart_drawing().is_none());
            assert_eq!(
                charts[1]
                    .chart_space()
                    .chart_drawing()
                    .unwrap()
                    .anchors()
                    .len(),
                7
            );
        }
    }
}
