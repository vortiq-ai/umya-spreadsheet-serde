use std::{
    collections::BTreeMap,
    io::{
        Cursor,
        Read,
    },
    path::{
        Path,
        PathBuf,
    },
};

use quick_xml::{
    NsReader,
    events::Event,
    name::ResolveResult,
};
use serde_json::Value;
use umya_spreadsheet::{
    Workbook,
    reader,
    writer,
    writer::streaming_writer::StreamingWriter,
};

const DRAWING_NS: &str = "http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing";
const MAIN_NS: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const OBJECT_NAMES: [&str; 7] = [
    "ExtendedChart",
    "Picture",
    "PivotSlicer",
    "Rectangle",
    "StandardChart",
    "TableSlicer",
    "Timeline",
];

type Name = (String, String);

#[derive(Debug)]
struct Element {
    name:       Name,
    attributes: BTreeMap<Name, String>,
    children:   Vec<Element>,
    text:       String,
}

impl Element {
    fn is(&self, namespace: &str, local: &str) -> bool {
        self.name.0 == namespace && self.name.1 == local
    }

    fn attribute(&self, local: &str) -> Option<&str> {
        self.attributes
            .get(&(String::new(), local.to_owned()))
            .map(String::as_str)
    }

    fn child(&self, namespace: &str, local: &str) -> Option<&Self> {
        self.children
            .iter()
            .find(|child| child.is(namespace, local))
    }
}

fn namespace(value: ResolveResult<'_>) -> String {
    match value {
        ResolveResult::Bound(value) => String::from_utf8(value.as_ref().to_vec()).unwrap(),
        ResolveResult::Unbound => String::new(),
        ResolveResult::Unknown(prefix) => panic!("undeclared XML prefix {prefix:?}"),
    }
}

fn parse(xml: &str) -> Element {
    let mut reader = NsReader::from_str(xml);
    let mut stack: Vec<Element> = Vec::new();
    loop {
        let event = reader.read_event().unwrap();
        let empty = matches!(event, Event::Empty(_));
        match event {
            Event::Start(start) | Event::Empty(start) => {
                let (uri, local) = reader.resolver().resolve_element(start.name());
                let mut element = Element {
                    name:       (
                        namespace(uri),
                        String::from_utf8(local.as_ref().to_vec()).unwrap(),
                    ),
                    attributes: BTreeMap::new(),
                    children:   Vec::new(),
                    text:       String::new(),
                };
                for attribute in start.attributes() {
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
                    let key = (
                        namespace(uri),
                        String::from_utf8(local.as_ref().to_vec()).unwrap(),
                    );
                    let value = attribute
                        .decoded_and_normalized_value(
                            quick_xml::XmlVersion::Implicit1_0,
                            reader.decoder(),
                        )
                        .unwrap()
                        .into_owned();
                    assert!(element.attributes.insert(key, value).is_none());
                }
                if empty {
                    if let Some(parent) = stack.last_mut() {
                        parent.children.push(element);
                    } else {
                        return element;
                    }
                } else {
                    stack.push(element);
                }
            }
            Event::End(_) => {
                let element = stack.pop().unwrap();
                if let Some(parent) = stack.last_mut() {
                    parent.children.push(element);
                } else {
                    return element;
                }
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
            Event::Eof => panic!("missing XML root"),
            _ => (),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
struct Transform {
    namespace: String,
    position:  Option<[i64; 2]>,
    extent:    Option<[i64; 2]>,
}

#[derive(Debug, PartialEq, Eq)]
struct Geometry {
    kind:       String,
    edit_as:    String,
    from:       Option<[i64; 4]>,
    to:         Option<[i64; 4]>,
    position:   Option<[i64; 2]>,
    extent:     Option<[i64; 2]>,
    transforms: Vec<Transform>,
}

fn marker(element: Option<&Element>) -> Option<[i64; 4]> {
    element.map(|element| {
        ["col", "colOff", "row", "rowOff"].map(|name| {
            element
                .child(DRAWING_NS, name)
                .unwrap()
                .text
                .parse()
                .unwrap()
        })
    })
}

fn pair(element: Option<&Element>, names: [&str; 2]) -> Option<[i64; 2]> {
    element.map(|element| names.map(|name| element.attribute(name).unwrap().parse().unwrap()))
}

fn transforms(element: &Element, result: &mut Vec<Transform>) {
    if element.name.1 == "xfrm" && matches!(element.name.0.as_str(), MAIN_NS | DRAWING_NS) {
        result.push(Transform {
            namespace: element.name.0.clone(),
            position:  pair(element.child(MAIN_NS, "off"), ["x", "y"]),
            extent:    pair(element.child(MAIN_NS, "ext"), ["cx", "cy"]),
        });
    }
    for child in &element.children {
        transforms(child, result);
    }
}

fn object_name(element: &Element) -> Option<&str> {
    if element.is(DRAWING_NS, "cNvPr") {
        if let Some(name) = element.attribute("name") {
            if OBJECT_NAMES.contains(&name) {
                return Some(name);
            }
        }
    }
    element.children.iter().find_map(object_name)
}

fn insert_anchor(result: &mut BTreeMap<String, Geometry>, anchor: &Element) {
    assert_eq!(anchor.name.0, DRAWING_NS);
    assert!(matches!(
        anchor.name.1.as_str(),
        "twoCellAnchor" | "oneCellAnchor" | "absoluteAnchor"
    ));
    let name = object_name(anchor).expect("an anchor without a measured object name");
    let mut values = Vec::new();
    transforms(anchor, &mut values);
    let geometry = Geometry {
        kind:       anchor.name.1.clone(),
        edit_as:    anchor.attribute("editAs").unwrap_or("twoCell").to_owned(),
        from:       marker(anchor.child(DRAWING_NS, "from")),
        to:         marker(anchor.child(DRAWING_NS, "to")),
        position:   pair(anchor.child(DRAWING_NS, "pos"), ["x", "y"]),
        extent:     pair(anchor.child(DRAWING_NS, "ext"), ["cx", "cy"]),
        transforms: values,
    };
    assert!(
        result.insert(name.to_owned(), geometry).is_none(),
        "duplicate object {name}"
    );
}

fn expected_geometry(measurement: &Value) -> BTreeMap<String, Geometry> {
    let mut result = BTreeMap::new();
    for (name, anchor) in measurement["geometry"].as_object().unwrap() {
        let geometry = Geometry {
            kind:       anchor["kind"].as_str().unwrap().to_owned(),
            edit_as:    anchor["edit_as"].as_str().unwrap().to_owned(),
            from:       serde_json::from_value(anchor["from"].clone()).unwrap(),
            to:         serde_json::from_value(anchor["to"].clone()).unwrap(),
            position:   serde_json::from_value(anchor["position"].clone()).unwrap(),
            extent:     serde_json::from_value(anchor["extent"].clone()).unwrap(),
            transforms: anchor["transforms"]
                .as_array()
                .unwrap()
                .iter()
                .map(|transform| Transform {
                    namespace: transform["namespace"].as_str().unwrap().to_owned(),
                    position:  serde_json::from_value(transform["position"].clone()).unwrap(),
                    extent:    serde_json::from_value(transform["extent"].clone()).unwrap(),
                })
                .collect(),
        };
        assert!(result.insert(name.clone(), geometry).is_none());
    }
    let mut names: Vec<_> = measurement["shapes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|shape| shape["name"].as_str().unwrap())
        .collect();
    names.sort_unstable();
    assert_eq!(result.keys().map(String::as_str).collect::<Vec<_>>(), names);
    result
}

fn package_geometry(bytes: &[u8]) -> BTreeMap<String, Geometry> {
    let mut result = BTreeMap::new();
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    for index in 0..zip.len() {
        let mut part = zip.by_index(index).unwrap();
        let path = part.name().unwrap();
        if !path.starts_with("xl/drawings/drawing") || !path.ends_with(".xml") {
            continue;
        }
        let mut xml = String::new();
        part.read_to_string(&mut xml).unwrap();
        let root = parse(&xml);
        assert!(root.is(DRAWING_NS, "wsDr"));
        for anchor in &root.children {
            insert_anchor(&mut result, anchor);
        }
    }
    result
}

fn edit(book: &mut Workbook, operation: &Value, worksheet: bool, text_column: bool) {
    if operation["noop"].as_bool() == Some(true) {
        return;
    }
    if let Some(steps) = operation["steps"].as_array() {
        for step in steps {
            edit(book, step, worksheet, text_column);
        }
        return;
    }
    let index = u32::try_from(operation["index"].as_u64().unwrap()).unwrap();
    let count = u32::try_from(operation["count"].as_u64().unwrap()).unwrap();
    let insert = operation["insert"].as_bool().unwrap();
    let row = operation["axis"] == "row";
    let column = umya_spreadsheet::helper::coordinate::string_from_column_index(index);
    if worksheet {
        let sheet = book.sheet_by_name_mut("Drawing").unwrap();
        match (row, insert, text_column) {
            (true, true, _) => sheet.insert_new_row(index, count),
            (true, false, _) => sheet.remove_row(index, count),
            (false, true, true) => sheet.insert_new_column(&column, count),
            (false, false, true) => sheet.remove_column(&column, count),
            (false, true, false) => sheet.insert_new_column_by_index(index, count),
            (false, false, false) => sheet.remove_column_by_index(index, count),
        }
    } else {
        match (row, insert, text_column) {
            (true, true, _) => book.insert_new_row("Drawing", index, count),
            (true, false, _) => book.remove_row("Drawing", index, count),
            (false, true, true) => book.insert_new_column("Drawing", &column, count),
            (false, false, true) => book.remove_column("Drawing", &column, count),
            (false, true, false) => book.insert_new_column_by_index("Drawing", index, count),
            (false, false, false) => book.remove_column_by_index("Drawing", index, count),
        }
    }
}

fn write(book: Workbook, streaming: bool) -> Vec<u8> {
    if !streaming {
        let mut bytes = Vec::new();
        writer::xlsx::write_writer(&book, &mut bytes).unwrap();
        return bytes;
    }
    let mut names: Vec<_> = book
        .sheet_collection_no_check()
        .iter()
        .map(|sheet| sheet.name().to_owned())
        .collect();
    // ChartEx must resolve its source cells before those sheets are consumed.
    names.sort_by_key(|name| name != "Drawing");
    let mut writer = StreamingWriter::new(zip::ZipWriter::new(Cursor::new(Vec::new())), book);
    for name in names {
        let sheet = writer.take_sheet(&name).unwrap();
        writer.flush_sheet(sheet).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

fn fixture_directory() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/drawing_anchors")
}

#[derive(Debug, PartialEq, Eq)]
struct ConsumerMetadata {
    slicer_caches:   Vec<(String, String)>,
    timeline_caches: Vec<(String, String)>,
    views:           Vec<(String, String, String)>,
    names:           Vec<(String, String, Option<u32>)>,
    pivots:          Vec<PivotBinding>,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct PivotBinding {
    sheet:       String,
    name:        String,
    source:      String,
    source_name: String,
    fields:      Vec<String>,
    records:     u32,
}

fn expected_metadata(measurement: &Value) -> ConsumerMetadata {
    let metadata = &measurement["metadata"];
    ConsumerMetadata {
        slicer_caches:   serde_json::from_value(metadata["slicer_caches"].clone()).unwrap(),
        timeline_caches: serde_json::from_value(metadata["timeline_caches"].clone()).unwrap(),
        views:           serde_json::from_value(metadata["views"].clone()).unwrap(),
        names:           serde_json::from_value(metadata["names"].clone()).unwrap(),
        pivots:          metadata["pivots"]
            .as_array()
            .unwrap()
            .iter()
            .map(|pivot| PivotBinding {
                sheet:       pivot["sheet"].as_str().unwrap().to_owned(),
                name:        pivot["name"].as_str().unwrap().to_owned(),
                source:      pivot["source"].as_str().unwrap().to_owned(),
                source_name: pivot["source_name"].as_str().unwrap().to_owned(),
                fields:      serde_json::from_value(pivot["fields"].clone()).unwrap(),
                records:     serde_json::from_value(pivot["records"].clone()).unwrap(),
            })
            .collect(),
    }
}

fn consumer_metadata(book: &Workbook, cache_names: &[String]) -> ConsumerMetadata {
    let mut result = ConsumerMetadata {
        slicer_caches:   book
            .slicer_caches()
            .iter()
            .map(|cache| (cache.name.clone(), cache.source_name.clone()))
            .collect(),
        timeline_caches: book
            .timeline_caches()
            .iter()
            .map(|cache| (cache.name.clone(), cache.source_name.clone()))
            .collect(),
        views:           Vec::new(),
        names:           Vec::new(),
        pivots:          Vec::new(),
    };
    for name in book.defined_names().iter().chain(
        book.sheet_collection()
            .iter()
            .flat_map(|sheet| sheet.defined_names()),
    ) {
        if cache_names
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(name.name()))
        {
            result.names.push((
                name.name().to_owned(),
                name.address(),
                name.has_local_sheet_id().then(|| name.local_sheet_id()),
            ));
        }
    }
    for sheet in book.sheet_collection() {
        for view in sheet.slicers() {
            result.views.push((
                sheet.name().to_owned(),
                view.name.clone(),
                view.cache.clone(),
            ));
        }
        for view in sheet.timelines() {
            result.views.push((
                sheet.name().to_owned(),
                view.name.clone(),
                view.cache.clone(),
            ));
        }
        for pivot in sheet.pivot_tables() {
            let definition = pivot.pivot_table_definition();
            let cache = pivot.pivot_cache_definition();
            let source = cache.cache_source().worksheet_source().unwrap();
            // Excel renumbers cacheId even on a no-op reopen/SaveAs (see the
            // tracked COM control). Pin the resolved cache, not its ZIP-local
            // ID.
            result.pivots.push(PivotBinding {
                sheet:       sheet.name().to_owned(),
                name:        definition.name().to_owned(),
                source:      source.address().address(),
                source_name: source.name().to_owned(),
                fields:      cache
                    .cache_fields()
                    .list()
                    .iter()
                    .map(|field| field.name().to_owned())
                    .collect(),
                records:     cache.record_count(),
            });
        }
    }
    result.slicer_caches.sort();
    result.timeline_caches.sort();
    result.views.sort();
    result.names.sort();
    result.pivots.sort();
    result
}

fn tracked_names(book: &Workbook) -> Vec<String> {
    let mut names: Vec<_> = book
        .slicer_caches()
        .iter()
        .map(|cache| cache.name.clone())
        .chain(
            book.timeline_caches()
                .iter()
                .map(|cache| cache.name.clone()),
        )
        .collect();
    // The no-op Excel control also removes an input orphan (_xlchart.v1.3).
    // Track the identities actually referenced by the source drawing, so that
    // this authoring normalization does not become a grid-edit expectation.
    for sheet in book.sheet_collection() {
        sheet
            .worksheet_drawing()
            .visit_extended_spaces(&mut |space| {
                space.visit_formulas(&mut |formula| {
                    let name = formula.address_str();
                    if name.starts_with("_xlchart.") && !names.contains(&name) {
                        names.push(name);
                    }
                });
            });
    }
    names
}

#[test]
fn structural_edits_match_excels_drawing_placement_in_both_writers() {
    let directory = fixture_directory();
    let manifest: Value =
        serde_json::from_slice(&std::fs::read(directory.join("known-answers.json")).unwrap())
            .unwrap();
    let measurements = manifest.as_object().unwrap();
    assert!(manifest["excel"].as_str().unwrap().starts_with("Excel 16."));
    assert_eq!(
        manifest["size-hidden-predecessor-insert-twice"]["operation"]["steps"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    for prefix in ["size-hidden-predecessor", "size-hidden-inside"] {
        for axis in ["row", "column"] {
            assert!(measurements.contains_key(&format!("{prefix}-{axis}-insert-before")));
        }
    }
    for mode in ["size", "move", "fixed"] {
        for origin in ["origin", "offset"] {
            let base = format!("{mode}-{origin}-base");
            assert_eq!(
                expected_geometry(&measurements[&base])
                    .keys()
                    .map(String::as_str)
                    .collect::<Vec<_>>(),
                OBJECT_NAMES
            );
        }
        for axis in ["row", "column"] {
            assert!(measurements.contains_key(&format!("{mode}-origin-{axis}-delete-zero")));
            for operation in [
                "insert-before",
                "delete-before",
                "insert-first",
                "insert-inside",
                "insert-second",
                "delete-first",
                "delete-second",
                "delete-whole",
            ] {
                assert!(measurements.contains_key(&format!("{mode}-offset-{axis}-{operation}")));
            }
        }
    }
    for prefix in [
        "size-nonuniform",
        "move-nonuniform",
        "fixed-nonuniform",
        "onecell-offset",
        "absolute-offset",
    ] {
        assert!(measurements.contains_key(&format!("{prefix}-base")));
        for axis in ["row", "column"] {
            for operation in [
                "insert-before",
                "delete-before",
                "insert-first",
                "insert-inside",
                "insert-second",
                "delete-first",
                "delete-second",
                "delete-whole",
            ] {
                assert!(measurements.contains_key(&format!("{prefix}-{axis}-{operation}")));
            }
        }
    }
    for prefix in ["size-root", "size-root-hidden"] {
        assert!(measurements.contains_key(&format!("{prefix}-base")));
        for axis in ["row", "column"] {
            assert!(measurements.contains_key(&format!("{prefix}-{axis}-insert-zero")));
        }
    }
    for base in measurements.keys().filter(|name| name.ends_with("-base")) {
        let opened = base.replace("-base", "-noop");
        assert!(measurements.contains_key(&opened), "{opened}");
    }
    for axis in ["row", "column"] {
        for operation in ["insert-at-to", "insert-at-from"] {
            assert!(measurements.contains_key(&format!("to-boundary-{axis}-{operation}")));
        }
    }
    for prefix in ["onecell-origin", "absolute-origin"] {
        assert!(measurements.contains_key(&format!("{prefix}-base")));
        for axis in ["row", "column"] {
            assert!(measurements.contains_key(&format!("{prefix}-{axis}-delete-zero")));
        }
    }
    let output = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/drawing-anchor-validation");
    std::fs::create_dir_all(&output).unwrap();
    let mut cases: Vec<_> = measurements
        .iter()
        .filter(|(name, _)| name.as_str() != "excel" && !name.ends_with("-reopened"))
        .collect();
    cases.sort_by_key(|(name, _)| !name.starts_with("size-root"));
    for (name, measurement) in cases {
        let measurement = opened(&measurements, name, measurement);
        let operation = &measurement["operation"];
        let base = operation["base"].as_str().unwrap_or(name);
        let base_measurement = opened(&measurements, base, &measurements[base]);
        let source = std::fs::read(directory.join(format!("{base}.xlsx"))).unwrap();
        let source_book = reader::xlsx::read_reader(Cursor::new(source.clone()), true).unwrap();
        let candidates = tracked_names(&source_book);
        for (reader_name, eager) in [("eager", true), ("deferred", false)] {
            for (owner, worksheet) in [("workbook", false), ("worksheet", true)] {
                // A worksheet-only edit cannot release what the workbook owns,
                // so it leaves everything unchanged where Excel
                // removes a slicer, a timeline or a ChartEx.
                let refused =
                    worksheet && removes_workbook_bound_object(base_measurement, measurement);
                let (answer, reopened) = if refused {
                    (base_measurement, None)
                } else {
                    (measurement, measurements.get(&format!("{name}-reopened")))
                };
                let mut book =
                    reader::xlsx::read_reader(Cursor::new(source.clone()), eager).unwrap();
                if !operation.is_null() {
                    edit(&mut book, operation, worksheet, eager);
                }
                for (writer_name, streaming) in [("ordinary", false), ("streaming", true)] {
                    // The Excel open check names the answer by the file's
                    // prefix.
                    let answered = if refused { base } else { name };
                    let label = format!("{answered}-{owner}-{reader_name}-{writer_name}");
                    let bytes = write(book.clone(), streaming);
                    std::fs::write(output.join(format!("{label}.xlsx")), &bytes).unwrap();
                    assert_eq!(
                        comparable(name, package_geometry(&bytes)),
                        comparable(name, expected_geometry(answer)),
                        "{label}"
                    );
                    let reread = reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap();
                    assert_eq!(
                        consumer_metadata(&reread, &candidates),
                        expected_metadata(answer),
                        "{label} consumer metadata"
                    );
                    // Excel refreshes ChartEx fallbacks again when it opens its
                    // own result, where its saved rectangle
                    // was stale.
                    assert_eq!(
                        comparable(name, package_geometry(&write(reread, false))),
                        comparable(name, expected_geometry(reopened.unwrap_or(answer))),
                        "{label} reread"
                    );
                }
            }
        }
    }
}

/// The measured inputs whose ChartEx fallback rectangle the library cannot
/// reproduce: the Normal font is Arial 14, not Calibri 11, so a column is not
/// seven pixels per digit (the `OPEN b` input in the answer). Everything else
/// of these cases, the anchors and every other transform, is still compared.
const UNMEASURED_NORMAL_FONT: [&str; 2] = ["move-arial14", "fixed-arial14"];

fn comparable(name: &str, mut geometry: BTreeMap<String, Geometry>) -> BTreeMap<String, Geometry> {
    if UNMEASURED_NORMAL_FONT
        .iter()
        .any(|input| name.starts_with(input))
    {
        if let Some(chart) = geometry.get_mut("ExtendedChart") {
            chart
                .transforms
                .retain(|transform| transform.namespace != MAIN_NS);
        }
    }
    geometry
}

/// Excel's open-and-save of an authored base, which refreshes ChartEx
/// fallbacks, instead of the authored file's own answer.
fn opened<'a>(
    measurements: &'a serde_json::Map<String, Value>,
    name: &str,
    measurement: &'a Value,
) -> &'a Value {
    name.strip_suffix("-base")
        .and_then(|prefix| measurements.get(&format!("{prefix}-noop")))
        .unwrap_or(measurement)
}

/// Whether Excel's answer removes an object whose bindings the workbook owns.
fn removes_workbook_bound_object(base: &Value, result: &Value) -> bool {
    let (before, after) = (expected_geometry(base), expected_geometry(result));
    ["ExtendedChart", "PivotSlicer", "TableSlicer", "Timeline"]
        .iter()
        .any(|name| before.contains_key(*name) && !after.contains_key(*name))
}

#[test]
fn drawing_deletions_on_a_taken_streaming_sheet_follow_excel_or_are_refused() {
    let directory = fixture_directory();
    let manifest: Value =
        serde_json::from_slice(&std::fs::read(directory.join("known-answers.json")).unwrap())
            .unwrap();
    let output = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/drawing-anchor-validation");
    std::fs::create_dir_all(&output).unwrap();
    for mode in ["size", "move", "fixed"] {
        for axis in ["row", "column"] {
            for deletion in ["delete-first", "delete-second", "delete-whole"] {
                let name = format!("{mode}-offset-{axis}-{deletion}");
                let measurements = manifest.as_object().unwrap();
                let base_name = format!("{mode}-offset-base");
                let base_measurement = opened(measurements, &base_name, &measurements[&base_name]);
                // The taken sheet is edited without its workbook: removing a
                // slicer, timeline or ChartEx is refused and changes nothing.
                let refused = removes_workbook_bound_object(base_measurement, &manifest[&name]);
                let measurement = if refused {
                    base_measurement
                } else {
                    &manifest[&name]
                };
                let operation = &manifest[&name]["operation"];
                let book =
                    reader::xlsx::read(directory.join(format!("{mode}-offset-base.xlsx"))).unwrap();
                let candidates = tracked_names(&book);
                let expected = expected_metadata(measurement);
                let sources = book
                    .sheet_collection()
                    .iter()
                    .filter(|sheet| sheet.name() != "Drawing")
                    .map(|sheet| sheet.name().to_owned())
                    .collect::<Vec<_>>();
                let mut writer =
                    StreamingWriter::new(zip::ZipWriter::new(Cursor::new(Vec::new())), book);
                let mut drawing = writer.take_sheet("Drawing").unwrap();
                let index = u32::try_from(operation["index"].as_u64().unwrap()).unwrap();
                let count = u32::try_from(operation["count"].as_u64().unwrap()).unwrap();
                if axis == "row" {
                    drawing.remove_row(index, count);
                } else {
                    drawing.remove_column_by_index(index, count);
                }
                writer.flush_sheet(drawing).unwrap();
                for source in &sources {
                    let sheet = writer.take_sheet(source).unwrap();
                    writer.flush_sheet(sheet).unwrap();
                }
                let bytes = writer.finish().unwrap().into_inner();
                std::fs::write(
                    output.join(format!(
                        "{}-workbook-streamed.xlsx",
                        if refused { &base_name } else { &name }
                    )),
                    &bytes,
                )
                .unwrap();
                assert_eq!(
                    package_geometry(&bytes),
                    expected_geometry(measurement),
                    "{name}"
                );
                let reread = reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap();
                assert_eq!(consumer_metadata(&reread, &candidates), expected, "{name}");
            }
        }
    }
}

#[cfg(feature = "serde")]
#[test]
fn workbook_drawing_deletion_survives_json_before_either_writer() {
    let directory = fixture_directory();
    let manifest: Value =
        serde_json::from_slice(&std::fs::read(directory.join("known-answers.json")).unwrap())
            .unwrap();
    for axis in ["row", "column"] {
        let name = format!("size-offset-{axis}-delete-whole");
        let measurement = &manifest[&name];
        let mut book = reader::xlsx::read(directory.join("size-offset-base.xlsx")).unwrap();
        let candidates = tracked_names(&book);
        let expected = expected_metadata(measurement);
        edit(&mut book, &measurement["operation"], false, true);
        // The workbook released the caches and names inside the operation, so
        // the worksheet carries no pending state.
        let saved = serde_json::to_value(book.sheet_by_name("Drawing").unwrap()).unwrap();
        for key in [
            "removed_drawing_caches",
            "removed_chart_formulas",
            "fallback_sync",
        ] {
            assert!(saved.get(key).is_none(), "{key}");
        }
        let restored = serde_json::from_value(saved).unwrap();
        *book.sheet_by_name_mut("Drawing").unwrap() = restored;
        for streaming in [false, true] {
            let bytes = write(book.clone(), streaming);
            assert_eq!(package_geometry(&bytes), expected_geometry(measurement));
            let reread = reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap();
            assert_eq!(consumer_metadata(&reread, &candidates), expected);
        }
    }
}

#[cfg(feature = "serde")]
#[test]
fn fallback_open_geometry_survives_json_between_structural_edits() {
    let directory = fixture_directory();
    let manifest: Value =
        serde_json::from_slice(&std::fs::read(directory.join("known-answers.json")).unwrap())
            .unwrap();
    let measurement = &manifest["size-hidden-predecessor-insert-twice"];
    let steps = measurement["operation"]["steps"].as_array().unwrap();
    let mut book = reader::xlsx::read(directory.join("size-hidden-predecessor-base.xlsx")).unwrap();
    let candidates = tracked_names(&book);
    let expected = expected_metadata(measurement);
    edit(&mut book, &steps[0], true, true);
    let saved = serde_json::to_value(book.sheet_by_name("Drawing").unwrap()).unwrap();
    *book.sheet_by_name_mut("Drawing").unwrap() = serde_json::from_value(saved).unwrap();
    edit(&mut book, &steps[1], true, true);
    for streaming in [false, true] {
        let bytes = write(book.clone(), streaming);
        assert_eq!(package_geometry(&bytes), expected_geometry(measurement));
        let reread = reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap();
        assert_eq!(consumer_metadata(&reread, &candidates), expected);
    }
}

#[test]
fn fallback_rectangle_follows_the_grid_at_open_not_at_the_first_edit() {
    let directory = fixture_directory();
    let manifest: Value =
        serde_json::from_slice(&std::fs::read(directory.join("known-answers.json")).unwrap())
            .unwrap();
    let opened = expected_geometry(&manifest["size-hidden-predecessor-noop"]);
    let authored = expected_geometry(&manifest["size-hidden-predecessor-base"]);
    let chart = |geometry: &BTreeMap<String, Geometry>| {
        geometry["ExtendedChart"]
            .transforms
            .iter()
            .find(|transform| transform.namespace == MAIN_NS)
            .map(|transform| (transform.position, transform.extent))
            .unwrap()
    };
    assert_ne!(chart(&opened), chart(&authored));
    let mut book = reader::xlsx::read(directory.join("size-hidden-predecessor-base.xlsx")).unwrap();
    assert_eq!(
        chart(&package_geometry(&write(book.clone(), false))),
        chart(&opened)
    );
    // A grid change between opening and the first edit does not move the
    // rectangle Excel computed when it opened the workbook.
    let sheet = book.sheet_by_name_mut("Drawing").unwrap();
    sheet.column_dimension_mut("B").set_width(40.0);
    sheet.row_dimension_mut(2).set_height(60.0);
    book.insert_new_row("Drawing", 30, 1);
    assert_eq!(
        chart(&package_geometry(&write(book, false))),
        chart(&opened)
    );
}

#[test]
fn nonfinite_and_oversized_dimensions_do_not_panic_a_structural_edit() {
    let directory = fixture_directory();
    let manifest: Value =
        serde_json::from_slice(&std::fs::read(directory.join("known-answers.json")).unwrap())
            .unwrap();
    let base = expected_geometry(&manifest["size-offset-base"]);
    for (width, height) in [
        (f64::NAN, f64::NAN),
        (f64::INFINITY, f64::NEG_INFINITY),
        (f64::MAX, f64::MAX),
    ] {
        let mut book = reader::xlsx::read(directory.join("size-offset-base.xlsx")).unwrap();
        let sheet = book.sheet_by_name_mut("Drawing").unwrap();
        sheet.column_dimension_mut("B").set_width(width);
        sheet.row_dimension_mut(2).set_height(height);
        // Row 2 is above every object; deleting it moves each marker up a row.
        book.remove_row("Drawing", 2, 1);
        book.insert_new_column_by_index("Drawing", 20, 1);
        let moved = package_geometry(&write(book, false));
        for (name, geometry) in &base {
            let expected =
                |marker: Option<[i64; 4]>| marker.map(|[c, co, r, ro]| [c, co, r - 1, ro]);
            assert_eq!(moved[name].from, expected(geometry.from), "{name}");
            assert_eq!(moved[name].to, expected(geometry.to), "{name}");
        }
    }
    // A sheet without any drawing has no grid to build.
    let mut book = umya_spreadsheet::new_file();
    let sheet = book.sheet_mut(0).unwrap();
    sheet.column_dimension_mut("A").set_width(f64::NAN);
    sheet.row_dimension_mut(1).set_height(f64::INFINITY);
    book.insert_new_row("Sheet1", 1, 1);
    book.remove_column_by_index("Sheet1", 1, 1);
}

#[cfg(feature = "serde")]
#[test]
fn persisted_worksheets_from_earlier_revisions_still_load() {
    let mut book = reader::xlsx::read(fixture_directory().join("size-offset-base.xlsx")).unwrap();
    let candidates = tracked_names(&book);
    let expected = consumer_metadata(&book, &candidates);
    let mut saved = serde_json::to_value(book.sheet_by_name("Drawing").unwrap()).unwrap();
    let object = saved.as_object_mut().unwrap();
    object.insert(
        "removed_drawing_caches".into(),
        serde_json::json!(["cache"]),
    );
    object.insert(
        "removed_chart_formulas".into(),
        serde_json::json!(["_xlchart.v1.0"]),
    );
    let restored: umya_spreadsheet::Worksheet = serde_json::from_value(saved).unwrap();
    let current = serde_json::to_value(&restored).unwrap();
    assert!(current.get("removed_drawing_caches").is_none());
    assert!(current.get("removed_chart_formulas").is_none());
    *book.sheet_by_name_mut("Drawing").unwrap() = restored;
    for streaming in [false, true] {
        let bytes = write(book.clone(), streaming);
        let reread = reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap();
        assert_eq!(consumer_metadata(&reread, &candidates), expected);
    }
}

#[test]
fn deleting_one_drawing_view_keeps_caches_used_by_other_views() {
    use umya_spreadsheet::drawing::spreadsheet::DrawingAnchor;

    let mut book = reader::xlsx::read(fixture_directory().join("size-offset-base.xlsx")).unwrap();
    let candidates = tracked_names(&book);
    let original = consumer_metadata(&book, &candidates);
    let sheet = book.sheet_by_name_mut("Drawing").unwrap();
    for mut view in sheet.slicers().to_vec() {
        view.name.push_str("Outside");
        view.uid = None;
        sheet.slicers_mut().push(view);
    }
    for mut view in sheet.timelines().to_vec() {
        view.name.push_str("Outside");
        view.uid = None;
        sheet.timelines_mut().push(view);
    }
    for mut frame in sheet.worksheet_drawing().slicer_frames().to_vec() {
        let name = format!("{}Outside", frame.slicer_name());
        frame.set_slicer_name(&name);
        let id = frame.frame_id() + 100;
        frame
            .frame_properties_mut()
            .non_visual_drawing_properties_mut()
            .set_id(id)
            .set_name(name);
        let DrawingAnchor::TwoCell { from, to, .. } = frame.anchor_mut() else {
            panic!("Excel-authored baseline must have two-cell slicer frames");
        };
        from.set_row(from.row() + 40);
        to.set_row(to.row() + 40);
        sheet.worksheet_drawing_mut().add_slicer_frame(frame);
    }
    book.remove_row("Drawing", 4, 15);
    for streaming in [false, true] {
        let reread =
            reader::xlsx::read_reader(Cursor::new(write(book.clone(), streaming)), true).unwrap();
        let retained = consumer_metadata(&reread, &candidates);
        assert_eq!(retained.slicer_caches, original.slicer_caches);
        assert_eq!(retained.timeline_caches, original.timeline_caches);
        let cache_names_only = |names: &[(String, String, Option<u32>)]| {
            names
                .iter()
                .filter(|(name, ..)| {
                    candidates.iter().any(|cache| {
                        !cache.starts_with("_xlchart.") && cache.eq_ignore_ascii_case(name)
                    })
                })
                .cloned()
                .collect::<Vec<_>>()
        };
        assert_eq!(
            cache_names_only(&retained.names),
            cache_names_only(&original.names)
        );
        assert_eq!(retained.pivots, original.pivots);
        let mut views = original
            .views
            .iter()
            .map(|(sheet, name, cache)| (sheet.clone(), format!("{name}Outside"), cache.clone()))
            .collect::<Vec<_>>();
        views.sort();
        assert_eq!(retained.views, views);
    }
}

#[test]
fn a_taken_streaming_sheet_refuses_to_remove_drawings_the_workbook_owns() {
    let book = reader::xlsx::read(fixture_directory().join("size-offset-base.xlsx")).unwrap();
    let candidates = tracked_names(&book);
    let original = consumer_metadata(&book, &candidates);
    let geometry = package_geometry(&write(book.clone(), false));
    let mut writer = StreamingWriter::new(zip::ZipWriter::new(Cursor::new(Vec::new())), book);
    let mut drawing = writer.take_sheet("Drawing").unwrap();
    drawing.remove_row(4, 15);
    writer.flush_sheet(drawing).unwrap();
    for name in ["Pivot", "Data"] {
        let sheet = writer.take_sheet(name).unwrap();
        writer.flush_sheet(sheet).unwrap();
    }
    let bytes = writer.finish().unwrap().into_inner();
    assert_eq!(package_geometry(&bytes), geometry);
    let reread = reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap();
    assert_eq!(consumer_metadata(&reread, &candidates), original);
}
