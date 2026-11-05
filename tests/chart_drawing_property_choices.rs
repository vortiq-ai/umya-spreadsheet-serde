#![cfg(feature = "serde")]
use std::io::{
    Cursor,
    Read,
    Write,
};

use umya_spreadsheet::{
    Workbook,
    reader,
    writer,
};

const FIXTURE: &[u8] = include_bytes!("fixtures/chart_drawing_excel.xlsx");
const GEOMETRY: &str = "<a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom>";
const SOLID: &str = "<a:solidFill><a:srgbClr val=\"123456\"/></a:solidFill>";
const PATTERN: &str = "<a:pattFill prst=\"pct5\"><a:fgClr><a:srgbClr \
                       val=\"123456\"/></a:fgClr><a:bgClr><a:srgbClr \
                       val=\"ABCDEF\"/></a:bgClr></a:pattFill>";
const GRADIENT: &str = "<a:gradFill><a:gsLst><a:gs pos=\"0\"><a:srgbClr \
                        val=\"123456\"/></a:gs><a:gs pos=\"100000\"><a:srgbClr \
                        val=\"ABCDEF\"/></a:gs></a:gsLst><a:lin ang=\"0\" \
                        scaled=\"1\"/></a:gradFill>";
fn package(object: &str) -> Vec<u8> {
    let xml = format!("<c:userShapes xmlns:c=\"http://schemas.openxmlformats.org/drawingml/2006/chart\" xmlns:cdr=\"http://schemas.openxmlformats.org/drawingml/2006/chartDrawing\" xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\"><cdr:relSizeAnchor><cdr:from><cdr:x>0</cdr:x><cdr:y>0</cdr:y></cdr:from><cdr:to><cdr:x>1</cdr:x><cdr:y>1</cdr:y></cdr:to>{object}</cdr:relSizeAnchor></c:userShapes>");
    let mut input = zip::ZipArchive::new(Cursor::new(FIXTURE)).unwrap();
    let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for index in 0..input.len() {
        let mut file = input.by_index(index).unwrap();
        let name = file.name().unwrap().to_string();
        let mut data = Vec::new();
        file.read_to_end(&mut data).unwrap();
        if name.starts_with("xl/drawings/") && String::from_utf8_lossy(&data).contains("userShapes")
        {
            data = xml.as_bytes().to_vec();
        }
        output
            .start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        output.write_all(&data).unwrap();
    }
    output.finish().unwrap().into_inner()
}
fn open(bytes: &[u8], eager: bool) -> Workbook {
    let mut book = reader::xlsx::read_reader(Cursor::new(bytes.to_vec()), eager).unwrap();
    book.read_sheet_collection();
    book
}
fn save(book: &Workbook, light: bool) -> Vec<u8> {
    let mut output = Cursor::new(Vec::new());
    if light {
        writer::xlsx::write_writer_light(book, &mut output).unwrap();
    } else {
        writer::xlsx::write_writer(book, &mut output).unwrap();
    }
    output.into_inner()
}
fn drawing_xml(bytes: &[u8]) -> String {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    for index in 0..archive.len() {
        let mut file = archive.by_index(index).unwrap();
        if file.name().unwrap().starts_with("xl/drawings/") {
            let mut data = String::new();
            file.read_to_string(&mut data).unwrap();
            if data.contains("userShapes") {
                return data;
            }
        }
    }
    panic!("chart drawing missing")
}
#[derive(Debug, PartialEq, Eq)]
enum ResolvedToken {
    Start((String, String), Vec<((String, String), String)>),
    End,
    Text(String),
}

fn namespace(value: quick_xml::name::ResolveResult<'_>) -> String {
    match value {
        quick_xml::name::ResolveResult::Bound(value) => {
            String::from_utf8(value.as_ref().to_vec()).unwrap()
        }
        quick_xml::name::ResolveResult::Unbound => String::new(),
        quick_xml::name::ResolveResult::Unknown(prefix) => panic!("undeclared prefix {prefix:?}"),
    }
}

// Resolve names before comparing; removing xmlns attributes would hide a
// DrawingML property incorrectly rewritten into the chartDrawing namespace.
fn resolved_subtree(xml: &str, local_name: &str) -> Vec<ResolvedToken> {
    use quick_xml::events::Event;
    let mut reader = quick_xml::NsReader::from_reader(xml.as_bytes());
    let mut tokens = Vec::new();
    let mut depth = 0;
    loop {
        let event = reader.read_event().unwrap();
        let empty = matches!(event, Event::Empty(_));
        match event {
            Event::Start(element) | Event::Empty(element) => {
                let (uri, local) = reader.resolver().resolve_element(element.name());
                let name = (
                    namespace(uri),
                    String::from_utf8(local.as_ref().to_vec()).unwrap(),
                );
                if depth == 0
                    && (name.0 != "http://schemas.openxmlformats.org/drawingml/2006/chartDrawing"
                        || name.1 != local_name)
                {
                    continue;
                }
                let mut attributes = Vec::new();
                for attribute in element.attributes() {
                    let attribute = attribute.unwrap();
                    if attribute.key.as_ref() == b"xmlns"
                        || attribute
                            .key
                            .prefix()
                            .is_some_and(|prefix| prefix.as_ref() == b"xmlns")
                    {
                        continue;
                    }
                    let (uri, local) = reader.resolver().resolve_attribute(attribute.key);
                    attributes.push((
                        (
                            namespace(uri),
                            String::from_utf8(local.as_ref().to_vec()).unwrap(),
                        ),
                        attribute
                            .decoded_and_normalized_value(
                                quick_xml::XmlVersion::Implicit1_0,
                                reader.decoder(),
                            )
                            .unwrap()
                            .into_owned(),
                    ));
                }
                attributes.sort();
                tokens.push(ResolvedToken::Start(name, attributes));
                if empty {
                    tokens.push(ResolvedToken::End);
                    if depth == 0 {
                        return tokens;
                    }
                } else {
                    depth += 1;
                }
            }
            Event::End(_) if depth > 0 => {
                tokens.push(ResolvedToken::End);
                depth -= 1;
                if depth == 0 {
                    return tokens;
                }
            }
            Event::Text(text) if depth > 0 => tokens.push(ResolvedToken::Text(
                text.xml_content(quick_xml::XmlVersion::Implicit1_0)
                    .unwrap()
                    .into_owned(),
            )),
            Event::Eof => panic!("missing chartDrawing {local_name}"),
            _ => (),
        }
    }
}

fn shape(properties: &str) -> String {
    format!(
        "<cdr:sp><cdr:nvSpPr><cdr:cNvPr id=\"2\" \
         name=\"Choices\"/><cdr:cNvSpPr/></cdr:nvSpPr>{properties}</cdr:sp>"
    )
}

fn properties(fill: &str) -> String {
    format!("<cdr:spPr>{GEOMETRY}{fill}</cdr:spPr>")
}

fn projected(chart: &umya_spreadsheet::Chart) -> umya_spreadsheet::Chart {
    let mut chart = chart.clone();
    chart.strip_export_parts();
    json_roundtrip(&chart)
}

fn json_roundtrip(chart: &umya_spreadsheet::Chart) -> umya_spreadsheet::Chart {
    serde_json::from_str(&serde_json::to_string(chart).unwrap()).unwrap()
}

fn shape_properties(
    chart: &umya_spreadsheet::Chart,
) -> &umya_spreadsheet::drawing::spreadsheet::ShapeProperties {
    let umya_spreadsheet::drawing::charts::ChartDrawingObject::Shape(shape) =
        chart.chart_space().chart_drawing().unwrap().anchors()[0].object()
    else {
        panic!("typed shape");
    };
    shape.shape_properties()
}

fn shape_properties_mut(
    chart: &mut umya_spreadsheet::Chart,
) -> &mut umya_spreadsheet::drawing::spreadsheet::ShapeProperties {
    let umya_spreadsheet::drawing::charts::ChartDrawingObject::Shape(shape) =
        chart.chart_space_mut().chart_drawing_mut().anchors_mut()[0].object_mut()
    else {
        panic!("typed shape");
    };
    shape.shape_properties_mut()
}

fn assert_written(book: &Workbook, expected: &[ResolvedToken], eager: bool, name: &str) {
    for light in [false, true] {
        let output = save(book, light);
        assert_eq!(
            resolved_subtree(&drawing_xml(&output), "spPr"),
            expected,
            "{name}"
        );
        let reread = open(&output, eager);
        assert_eq!(
            resolved_subtree(&drawing_xml(&save(&reread, light)), "spPr"),
            expected,
            "reread {name}"
        );
        if let Some(target) = std::env::var_os("CARGO_TARGET_DIR") {
            let dir = std::path::PathBuf::from(target).join("chart-drawing-property-choices");
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join(format!("{name}-{eager}-{light}.xlsx")), output).unwrap();
        }
    }
}

#[test]
fn projected_fill_choices_replace_every_competing_fill() {
    for (from, source_fill, to, target_fill) in [
        ("pattern", PATTERN, "solid", SOLID),
        ("solid", SOLID, "pattern", PATTERN),
        ("pattern", PATTERN, "gradient", GRADIENT),
        ("gradient", GRADIENT, "pattern", PATTERN),
        ("gradient", GRADIENT, "solid", SOLID),
        ("solid", SOLID, "gradient", GRADIENT),
        ("pattern", PATTERN, "pattern", PATTERN),
    ] {
        let source = package(&shape(&properties(source_fill)));
        let donor_bytes = package(&shape(&properties(target_fill)));
        let expected = resolved_subtree(&drawing_xml(&donor_bytes), "spPr");
        let donor = open(&donor_bytes, true);
        let donor = shape_properties(&donor.sheet(0).unwrap().chart_collection()[0]);
        for eager in [true, false] {
            let mut book = open(&source, eager);
            let original = book.sheet(0).unwrap().chart_collection()[0].clone();
            let mut returned = projected(&original);
            match to {
                "solid" => {
                    shape_properties_mut(&mut returned)
                        .set_solid_fill(donor.solid_fill().unwrap().clone());
                }
                "pattern" if from != to => {
                    shape_properties_mut(&mut returned)
                        .set_pattern_fill(donor.pattern_fill().unwrap().clone());
                }
                "gradient" => {
                    shape_properties_mut(&mut returned)
                        .set_gradient_fill(donor.gradient_fill().unwrap().clone());
                }
                _ => (),
            }
            // The server receives the client's edited visible JSON before
            // restoring hidden XML.
            let mut returned = json_roundtrip(&returned);
            returned.restore_export_parts(&original).unwrap();
            book.sheet_mut(0).unwrap().chart_collection_mut()[0] = returned;
            assert_written(&book, &expected, eager, &format!("fill-{from}-to-{to}"));
        }
    }
}

#[test]
fn glow_preserves_every_color_choice_and_its_transforms() {
    for (label, color, extra) in [
        (
            "srgb",
            "<a:srgbClr val=\"ABCDEF\"><a:alpha val=\"65000\"/></a:srgbClr>",
            "",
        ),
        (
            "scheme",
            "<a:schemeClr val=\"accent1\"><a:lumMod val=\"60000\"/></a:schemeClr>",
            "",
        ),
        (
            "scrgb",
            "<a:scrgbClr r=\"10000\" g=\"20000\" b=\"30000\"><a:alpha val=\"65000\"/></a:scrgbClr>",
            "",
        ),
        (
            "hsl",
            "<a:hslClr hue=\"5400000\" sat=\"50000\" lum=\"50000\"><a:lumMod \
             val=\"60000\"/></a:hslClr>",
            "",
        ),
        (
            "system",
            "<a:sysClr val=\"windowText\" lastClr=\"123456\"><a:alpha val=\"65000\"/></a:sysClr>",
            "",
        ),
        (
            "preset",
            "<a:prstClr val=\"red\"><a:lumMod val=\"60000\"/></a:prstClr>",
            "",
        ),
        (
            "mixed",
            "<a:srgbClr val=\"ABCDEF\"><a:alpha val=\"65000\"/></a:srgbClr>",
            "<a:softEdge rad=\"25400\"/>",
        ),
    ] {
        let properties = format!(
            "<cdr:spPr>{GEOMETRY}{SOLID}<a:effectLst><a:glow \
             rad=\"12700\">{color}</a:glow>{extra}</a:effectLst></cdr:spPr>"
        );
        let source = package(&shape(&properties));
        let expected = resolved_subtree(&drawing_xml(&source), "spPr");
        for eager in [true, false] {
            let mut book = open(&source, eager);
            assert_written(&book, &expected, eager, &format!("glow-{label}-direct"));
            let original = book.sheet(0).unwrap().chart_collection()[0].clone();
            let mut returned = projected(&original);
            returned.restore_export_parts(&original).unwrap();
            book.sheet_mut(0).unwrap().chart_collection_mut()[0] = returned;
            assert_written(&book, &expected, eager, &format!("glow-{label}-restored"));
        }
    }
}

#[test]
fn empty_shape_and_connector_properties_keep_all_children_absent() {
    for (label, sp_pr) in [
        ("empty", "<cdr:spPr/>"),
        ("paired", "<cdr:spPr></cdr:spPr>"),
    ] {
        for connector in [false, true] {
            let object = if connector {
                format!(
                    "<cdr:cxnSp><cdr:nvCxnSpPr><cdr:cNvPr id=\"2\" \
                     name=\"Choices\"/><cdr:cNvCxnSpPr/></cdr:nvCxnSpPr>{sp_pr}</cdr:cxnSp>"
                )
            } else {
                shape(sp_pr)
            };
            let source = package(&object);
            let expected = resolved_subtree(&drawing_xml(&source), "spPr");
            for eager in [true, false] {
                let mut book = open(&source, eager);
                let object = book.sheet(0).unwrap().chart_collection()[0]
                    .chart_space()
                    .chart_drawing()
                    .unwrap()
                    .anchors()[0]
                    .object();
                assert!(matches!(
                    (connector, object),
                    (
                        false,
                        umya_spreadsheet::drawing::charts::ChartDrawingObject::Shape(_)
                    ) | (
                        true,
                        umya_spreadsheet::drawing::charts::ChartDrawingObject::Connector(_)
                    )
                ));
                assert_written(
                    &book,
                    &expected,
                    eager,
                    &format!("absent-{connector}-{label}-direct"),
                );
                let original = book.sheet(0).unwrap().chart_collection()[0].clone();
                let mut returned = projected(&original);
                returned.restore_export_parts(&original).unwrap();
                book.sheet_mut(0).unwrap().chart_collection_mut()[0] = returned;
                assert_written(
                    &book,
                    &expected,
                    eager,
                    &format!("absent-{connector}-{label}-restored"),
                );
            }
        }
    }
}

#[test]
fn legacy_hidden_pattern_restoration_respects_visible_fill_edits() {
    use umya_spreadsheet::drawing::charts::ChartDrawingObject;
    let group = format!(
        "<cdr:grpSp><cdr:nvGrpSpPr><cdr:cNvPr id=\"2\" \
         name=\"Legacy\"/><cdr:cNvGrpSpPr/></cdr:nvGrpSpPr><cdr:grpSpPr>{PATTERN}</cdr:\
         grpSpPr>{}</cdr:grpSp>",
        shape(&properties("")).replace("id=\"2\"", "id=\"3\"")
    );
    let donor = open(&package(&group), true);
    let ChartDrawingObject::Group(group) = donor.sheet(0).unwrap().chart_collection()[0]
        .chart_space()
        .chart_drawing()
        .unwrap()
        .anchors()[0]
        .object()
    else {
        panic!("typed group");
    };
    let raw_pattern = group.group_shape_properties().preserved_properties();
    assert_eq!(raw_pattern.len(), 1);
    for (target, target_fill) in [
        ("solid", SOLID),
        ("gradient", GRADIENT),
        ("pattern", PATTERN),
    ] {
        let donor = open(&package(&shape(&properties(target_fill))), true);
        let target_properties = shape_properties(&donor.sheet(0).unwrap().chart_collection()[0]);
        let expected = resolved_subtree(
            &drawing_xml(&package(&shape(&properties(target_fill)))),
            "spPr",
        );
        for eager in [true, false] {
            let mut book = open(&package(&shape(&properties(PATTERN))), eager);
            let mut original = book.sheet(0).unwrap().chart_collection()[0].clone();
            let mut legacy = serde_json::to_value(shape_properties(&original)).unwrap();
            legacy["pattern_fill"] = serde_json::Value::Null;
            legacy["preserved_properties"] = serde_json::to_value(raw_pattern).unwrap();
            *shape_properties_mut(&mut original) = serde_json::from_value(legacy).unwrap();
            let mut returned = projected(&original);
            match target {
                "solid" => {
                    shape_properties_mut(&mut returned)
                        .set_solid_fill(target_properties.solid_fill().unwrap().clone());
                }
                "gradient" => {
                    shape_properties_mut(&mut returned)
                        .set_gradient_fill(target_properties.gradient_fill().unwrap().clone());
                }
                _ => (),
            }
            let mut returned = json_roundtrip(&returned);
            returned.restore_export_parts(&original).unwrap();
            book.sheet_mut(0).unwrap().chart_collection_mut()[0] = returned;
            assert_written(
                &book,
                &expected,
                eager,
                &format!("fill-legacy-pattern-to-{target}"),
            );
        }
    }
}

#[test]
fn pattern_fill_defaults_in_older_json_and_roundtrips_in_binary() {
    use umya_spreadsheet::drawing::spreadsheet::ShapeProperties;
    for fill in [PATTERN, SOLID, GRADIENT, ""] {
        let book = open(&package(&shape(&properties(fill))), true);
        let properties = shape_properties(&book.sheet(0).unwrap().chart_collection()[0]);
        let binary = bincode::serialize(properties).unwrap();
        let decoded: ShapeProperties = bincode::deserialize(&binary).unwrap();
        assert_eq!(bincode::serialize(&decoded).unwrap(), binary);
        assert_eq!(
            serde_json::to_value(&decoded).unwrap(),
            serde_json::to_value(properties).unwrap()
        );
        let mut legacy = serde_json::to_value(properties).unwrap();
        legacy.as_object_mut().unwrap().remove("pattern_fill");
        let decoded: ShapeProperties = serde_json::from_value(legacy).unwrap();
        assert!(decoded.pattern_fill().is_none());
    }
}

#[test]
fn projected_pattern_fill_removal_keeps_the_fill_absent() {
    let source = package(&shape(&properties(PATTERN)));
    let expected = resolved_subtree(&drawing_xml(&package(&shape(&properties("")))), "spPr");
    for eager in [true, false] {
        let mut book = open(&source, eager);
        let original = book.sheet(0).unwrap().chart_collection()[0].clone();
        let mut returned = projected(&original);
        shape_properties_mut(&mut returned).remove_pattern_fill();
        let mut returned = json_roundtrip(&returned);
        returned.restore_export_parts(&original).unwrap();
        book.sheet_mut(0).unwrap().chart_collection_mut()[0] = returned;
        assert_written(&book, &expected, eager, "pattern-removed");
    }
}

#[test]
fn pattern_fill_without_a_preset_keeps_the_attribute_absent() {
    let source = package(&shape(&properties("<a:pattFill/>")));
    let expected = resolved_subtree(&drawing_xml(&source), "spPr");
    for eager in [true, false] {
        let mut book = open(&source, eager);
        assert_written(&book, &expected, eager, "pattern-no-preset-direct");
        let original = book.sheet(0).unwrap().chart_collection()[0].clone();
        let mut returned = projected(&original);
        returned.restore_export_parts(&original).unwrap();
        book.sheet_mut(0).unwrap().chart_collection_mut()[0] = returned;
        assert_written(&book, &expected, eager, "pattern-no-preset-restored");
    }
}

const CUSTOM_GEOMETRY: &str = "<a:custGeom><a:avLst/><a:gdLst/><a:ahLst/><a:cxnLst/><a:rect \
                               l=\"0\" t=\"0\" r=\"r\" b=\"b\"/><a:pathLst><a:path w=\"10\" \
                               h=\"10\"><a:moveTo><a:pt x=\"0\" y=\"0\"/></a:moveTo><a:lnTo><a:pt \
                               x=\"10\" y=\"10\"/></a:lnTo><a:close/></a:path></a:pathLst></a:\
                               custGeom>";
const ELLIPSE: &str = "<a:prstGeom prst=\"ellipse\"><a:avLst/></a:prstGeom>";

#[test]
fn projected_geometry_choice_wins_over_restored_custom_geometry() {
    for (label, source_geometry, edited_geometry) in [
        ("custom-to-ellipse", CUSTOM_GEOMETRY, Some(ELLIPSE)),
        ("custom-unchanged", CUSTOM_GEOMETRY, None),
        ("preset-to-ellipse", GEOMETRY, Some(ELLIPSE)),
        ("preset-unchanged", GEOMETRY, None),
    ] {
        let with_geometry =
            |geometry: &str| package(&shape(&format!("<cdr:spPr>{geometry}{SOLID}</cdr:spPr>")));
        let source = with_geometry(source_geometry);
        let expected_bytes = with_geometry(edited_geometry.unwrap_or(source_geometry));
        let expected = resolved_subtree(&drawing_xml(&expected_bytes), "spPr");
        let donor = open(&with_geometry(ELLIPSE), true);
        let donor = shape_properties(&donor.sheet(0).unwrap().chart_collection()[0]);
        for eager in [true, false] {
            let mut book = open(&source, eager);
            let original = book.sheet(0).unwrap().chart_collection()[0].clone();
            let mut returned = projected(&original);
            if edited_geometry.is_some() {
                shape_properties_mut(&mut returned).set_geometry(donor.geometry().clone());
            }
            let mut returned = json_roundtrip(&returned);
            returned.restore_export_parts(&original).unwrap();
            book.sheet_mut(0).unwrap().chart_collection_mut()[0] = returned;
            assert_written(&book, &expected, eager, label);
        }
    }
}
