#![cfg(feature = "serde")]

use std::{
    collections::{
        BTreeMap,
        BTreeSet,
    },
    io::{
        Cursor,
        Read,
    },
};

use quick_xml::{
    Reader,
    events::Event,
};
use serde_json::{
    Value,
    json,
};
use umya_spreadsheet::{
    DefinedName,
    SheetStateValues,
    Table,
    TableColumn,
    Workbook,
    reader,
    writer,
    writer::streaming_writer::StreamingWriter,
};

const ORACLE: &str = include_str!("fixtures/sheet_operations/copy-known-answers.json");

#[derive(Default)]
struct Node {
    depth:      usize,
    kind:       String,
    attributes: BTreeMap<String, String>,
    text:       String,
}

fn xml_nodes(xml: &[u8]) -> Vec<Node> {
    let mut reader = Reader::from_reader(xml);
    let mut stack: Vec<Node> = Vec::new();
    let mut nodes = Vec::new();
    loop {
        let event = reader.read_event().unwrap();
        let empty = matches!(&event, Event::Empty(_));
        match event {
            Event::Start(element) | Event::Empty(element) => {
                let node = Node {
                    depth:      stack.len(),
                    kind:       String::from_utf8(element.local_name().as_ref().to_vec()).unwrap(),
                    attributes: element
                        .attributes()
                        .map(|attribute| {
                            let attribute = attribute.unwrap();
                            (
                                String::from_utf8(attribute.key.as_ref().to_vec()).unwrap(),
                                attribute
                                    .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                                    .unwrap()
                                    .into_owned(),
                            )
                        })
                        .collect(),
                    text:       String::new(),
                };
                if empty {
                    nodes.push(node);
                } else {
                    stack.push(node);
                }
            }
            Event::Text(text) => {
                let text = text.xml10_content().unwrap();
                for node in &mut stack {
                    node.text.push_str(&text);
                }
            }
            Event::GeneralRef(reference) => {
                let entity = reference.decode().unwrap();
                let text = quick_xml::escape::unescape(&format!("&{entity};"))
                    .unwrap()
                    .into_owned();
                for node in &mut stack {
                    node.text.push_str(&text);
                }
            }
            Event::End(_) => nodes.push(stack.pop().unwrap()),
            Event::Eof => break,
            _ => (),
        }
    }
    assert!(stack.is_empty());
    nodes
}

fn parts(bytes: &[u8]) -> BTreeMap<String, Vec<u8>> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut parts = BTreeMap::new();
    for index in 0..archive.len() {
        let mut part = archive.by_index(index).unwrap();
        let name = part.name().unwrap().into_owned();
        let mut bytes = Vec::new();
        part.read_to_end(&mut bytes).unwrap();
        assert!(parts.insert(name, bytes).is_none(), "duplicate ZIP entry");
    }
    parts
}

fn write(book: &Workbook, streaming: bool) -> Vec<u8> {
    if !streaming {
        let mut bytes = Vec::new();
        writer::xlsx::write_writer(book, &mut bytes).unwrap();
        return bytes;
    }
    let names: Vec<_> = book
        .sheet_collection_no_check()
        .iter()
        .map(|sheet| sheet.name().to_owned())
        .collect();
    let mut writer =
        StreamingWriter::new(zip::ZipWriter::new(Cursor::new(Vec::new())), book.clone());
    for name in names {
        let sheet = writer.take_sheet(&name).unwrap();
        writer.flush_sheet(sheet).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

fn sorted(mut values: Vec<Value>) -> Vec<Value> {
    values.sort_by_cached_key(Value::to_string);
    values
}

fn oracle_nodes(expected: &Value, prefix: &str, kind: &str) -> Vec<Value> {
    sorted(
        expected["parts"]
            .as_object()
            .unwrap()
            .iter()
            .filter(|(path, _)| path.starts_with(prefix))
            .flat_map(|(_, part)| part["nodes"].as_array().unwrap())
            .filter(|node| node["kind"] == kind)
            .cloned()
            .collect(),
    )
}

fn actual_nodes(parts: &BTreeMap<String, Vec<u8>>, prefix: &str, kind: &str) -> Vec<Node> {
    parts
        .iter()
        .filter(|(path, _)| path.starts_with(prefix) && path.ends_with(".xml"))
        .flat_map(|(_, xml)| xml_nodes(xml))
        .filter(|node| {
            node.kind == kind
                && (kind != "pivotTableDefinition" || node.depth == 0)
                && (kind != "sp" || node.attributes.contains_key("textlink"))
        })
        .collect()
}

fn compare_nodes(parts: &BTreeMap<String, Vec<u8>>, expected: &Value, prefix: &str, kind: &str) {
    let expected = oracle_nodes(expected, prefix, kind);
    let actual = actual_nodes(parts, prefix, kind);
    assert_eq!(actual.len(), expected.len(), "{prefix} {kind} count");
    let mut remaining: Vec<_> = actual.iter().collect();
    for wanted in expected {
        let index = remaining
            .iter()
            .position(|node| {
                wanted
                    .as_object()
                    .unwrap()
                    .iter()
                    .all(|(key, value)| match key.as_str() {
                        "kind" => node.kind == value.as_str().unwrap(),
                        "text" => node.text == value.as_str().unwrap(),
                        _ if key.contains(':') => true,
                        _ => node.attributes.get(key).map(String::as_str) == value.as_str(),
                    })
            })
            .unwrap_or_else(|| {
                panic!(
                    "missing Excel {prefix} {kind}: {wanted}; actual={:?}",
                    remaining
                        .iter()
                        .map(|node| (&node.attributes, &node.text))
                        .collect::<Vec<_>>()
                )
            });
        remaining.remove(index);
    }
    assert!(remaining.is_empty());
}

fn append_preserving_selection(
    book: &mut Workbook,
    copy: umya_spreadsheet::Worksheet,
    expected: &Value,
) -> Value {
    // Preserve file selection on append; Excel's interactive Copy activation is
    // not this API's contract.
    let baseline = reader::xlsx::read_reader(Cursor::new(write(book, false)), true).unwrap();
    let selections = |book: &Workbook| -> Vec<Vec<bool>> {
        book.sheet_collection_no_check()
            .iter()
            .map(|sheet| {
                sheet
                    .sheets_views()
                    .sheet_view_list()
                    .iter()
                    .map(|view| view.tab_selected())
                    .collect()
            })
            .collect()
    };
    let mut selected = selections(&baseline);
    selected.push(
        copy.sheets_views()
            .sheet_view_list()
            .iter()
            .map(|view| view.tab_selected())
            .collect(),
    );
    let active = baseline.workbook_view().active_tab();
    book.add_sheet(copy).unwrap();
    assert_eq!(
        book.workbook_view().active_tab(),
        active,
        "append preserves activeTab"
    );
    let saved = reader::xlsx::read_reader(Cursor::new(write(book, false)), true).unwrap();
    assert_eq!(
        selections(&saved),
        selected,
        "append preserves every tabSelected"
    );
    let mut expected = expected.clone();
    expected["active_tab"] = json!(active);
    for (sheet, selected) in expected["sheets"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .zip(selected)
    {
        sheet["selected"] = json!(selected.first().copied().unwrap_or(false));
    }
    expected
}

fn compare_workbook(parts: &BTreeMap<String, Vec<u8>>, expected: &Value) {
    let nodes = xml_nodes(&parts["xl/workbook.xml"]);
    let relationships = xml_nodes(&parts["xl/_rels/workbook.xml.rels"]);
    let sheets: Vec<_> = nodes
        .iter()
        .filter(|node| node.kind == "sheet")
        .map(|node| {
            let target = &relationships
                .iter()
                .find(|rel| rel.attributes.get("Id") == node.attributes.get("r:id"))
                .unwrap()
                .attributes["Target"];
            let path = if target.starts_with('/') {
                target.trim_start_matches('/').to_owned()
            } else {
                format!("xl/{target}")
            };
            let selected = xml_nodes(&parts[&path])
                .iter()
                .find(|node| node.kind == "sheetView")
                .is_some_and(|view| {
                    view.attributes
                        .get("tabSelected")
                        .is_some_and(|value| value == "1" || value == "true")
                });
            json!({
                "name": node.attributes["name"], "id": node.attributes["sheetId"],
                "state": node.attributes.get("state").map_or("visible", String::as_str),
                "selected": selected,
            })
        })
        .collect();
    assert_eq!(sheets, *expected["sheets"].as_array().unwrap());
    let active = nodes
        .iter()
        .find(|node| node.kind == "workbookView")
        .unwrap()
        .attributes
        .get("activeTab")
        .map_or(0, |value| value.parse::<u32>().unwrap());
    assert_eq!(active, expected["active_tab"].as_u64().unwrap() as u32);
    let names = sorted(
        nodes
            .iter()
            .filter(|node| node.kind == "definedName")
            .map(|node| {
                json!({
                    "name": node.attributes["name"],
                    "scope": node.attributes.get("localSheetId").map_or("", String::as_str),
                    "value": node.text,
                })
            })
            .collect(),
    );
    assert_eq!(names, sorted(expected["names"].as_array().unwrap().clone()));
}

fn compare_tables(parts: &BTreeMap<String, Vec<u8>>, expected: &Value) {
    let wanted = sorted(
        expected["parts"]
            .as_object()
            .unwrap()
            .iter()
            .filter(|(path, _)| path.starts_with("xl/tables/"))
            .map(|(_, part)| {
                let attributes = &part["attributes"];
                json!([
                    attributes["id"],
                    attributes["name"],
                    attributes["displayName"],
                    attributes["ref"]
                ])
            })
            .collect(),
    );
    let actual = sorted(
        actual_nodes(parts, "xl/tables/", "table")
            .iter()
            .map(|node| {
                json!([
                    node.attributes["id"],
                    node.attributes["name"],
                    node.attributes["displayName"],
                    node.attributes["ref"]
                ])
            })
            .collect(),
    );
    assert_eq!(actual, wanted, "table identities and extents");
}

fn compare_cache_headers(
    parts: &BTreeMap<String, Vec<u8>>,
    expected: &Value,
    prefix: &str,
    root: &str,
) {
    let wanted = sorted(
        expected["parts"]
            .as_object()
            .unwrap()
            .iter()
            .filter(|(path, _)| path.starts_with(prefix))
            .map(|(_, part)| json!([part["attributes"]["name"], part["attributes"]["sourceName"]]))
            .collect(),
    );
    let actual = sorted(
        actual_nodes(parts, prefix, root)
            .iter()
            .map(|node| json!([node.attributes["name"], node.attributes["sourceName"]]))
            .collect(),
    );
    assert_eq!(actual, wanted, "{root} identities");
    for (path, part) in expected["parts"]
        .as_object()
        .unwrap()
        .iter()
        .filter(|(path, _)| path.starts_with(prefix))
    {
        let name = part["attributes"]["name"].as_str().unwrap();
        let (_, xml) = parts
            .iter()
            .filter(|(path, _)| path.starts_with(prefix) && path.ends_with(".xml"))
            .find(|(_, xml)| {
                xml_nodes(xml).iter().any(|node| {
                    node.kind == root
                        && node.attributes.get("name").map(String::as_str) == Some(name)
                })
            })
            .unwrap();
        let actual_part = BTreeMap::from([(path.clone(), xml.clone())]);
        let mut expected_part = json!({"parts": {}});
        expected_part["parts"][path] = part.clone();
        for kind in ["pivotTable", "tableSlicerCache", "state"] {
            compare_nodes(&actual_part, &expected_part, prefix, kind);
        }
    }
}

fn compare_pivots(parts: &BTreeMap<String, Vec<u8>>, expected: &Value) {
    let wanted: Vec<_> = expected["parts"]
        .as_object()
        .unwrap()
        .iter()
        .filter(|(path, _)| path.starts_with("xl/pivotTables/"))
        .map(|(_, part)| &part["attributes"])
        .collect();
    let actual = actual_nodes(parts, "xl/pivotTables/", "pivotTableDefinition");
    assert_eq!(
        sorted(
            actual
                .iter()
                .map(|node| json!(node.attributes["name"]))
                .collect()
        ),
        sorted(
            wanted
                .iter()
                .map(|attributes| attributes["name"].clone())
                .collect()
        ),
        "pivot names remain scoped to their sheets"
    );
    let wanted_cache_ids: BTreeSet<_> = wanted
        .iter()
        .map(|attributes| attributes["cacheId"].as_str().unwrap())
        .collect();
    let actual_cache_ids: BTreeSet<_> = actual
        .iter()
        .map(|node| node.attributes["cacheId"].as_str())
        .collect();
    assert_eq!(
        actual_cache_ids.len(),
        wanted_cache_ids.len(),
        "copied pivots share the original cache"
    );
    let groups = |pairs: Vec<(String, String)>| {
        let mut groups: BTreeMap<String, Vec<Value>> = BTreeMap::new();
        for (id, name) in pairs {
            groups.entry(id).or_default().push(Value::String(name));
        }
        sorted(
            groups
                .into_values()
                .map(|names| Value::Array(sorted(names)))
                .collect(),
        )
    };
    assert_eq!(
        groups(
            actual
                .iter()
                .map(|node| (
                    node.attributes["cacheId"].clone(),
                    node.attributes["name"].clone()
                ))
                .collect()
        ),
        groups(
            wanted
                .iter()
                .map(|attributes| (
                    attributes["cacheId"].as_str().unwrap().to_owned(),
                    attributes["name"].as_str().unwrap().to_owned()
                ))
                .collect()
        ),
        "each copied pivot keeps the same cache sharing group as Excel"
    );
    compare_nodes(parts, expected, "xl/pivotCache/", "worksheetSource");
}

fn chart_bindings(parts: &BTreeMap<String, Vec<u8>>, expected: &Value) {
    for (prefix, kind) in [
        ("xl/charts/", "f"),
        ("xl/charts/", "nf"),
        ("xl/worksheets/", "f"),
    ] {
        compare_nodes(parts, expected, prefix, kind);
    }
    let expected_bindings: Vec<_> = oracle_nodes(expected, "xl/charts/", "pivotSource")
        .iter()
        .map(|node| {
            node["text"]
                .as_str()
                .unwrap()
                .rsplit(']')
                .next()
                .unwrap()
                .to_owned()
        })
        .collect();
    let actual_bindings: Vec<_> = actual_nodes(parts, "xl/charts/", "pivotSource")
        .iter()
        .map(|node| node.text.rsplit(']').next().unwrap().to_owned())
        .collect();
    assert_eq!(
        sorted(actual_bindings.into_iter().map(Value::String).collect()),
        sorted(expected_bindings.into_iter().map(Value::String).collect()),
        "PivotChart sheet/table binding"
    );
}

fn compare_saved(bytes: &[u8], expected: &Value) {
    let parts = parts(bytes);
    compare_workbook(&parts, expected);
    compare_tables(&parts, expected);
    compare_pivots(&parts, expected);
    for (prefix, root) in [
        ("xl/slicerCaches/", "slicerCacheDefinition"),
        ("xl/timelineCaches/", "timelineCacheDefinition"),
    ] {
        compare_cache_headers(&parts, expected, prefix, root);
        compare_nodes(&parts, expected, prefix, "pivotTable");
    }
    for (prefix, kind) in [
        ("xl/slicerCaches/", "tableSlicerCache"),
        ("xl/slicers/", "slicer"),
        ("xl/timelines/", "timeline"),
        ("xl/timelineCaches/", "state"),
        ("xl/drawings/", "slicer"),
        ("xl/drawings/", "timeline"),
        ("xl/drawings/", "sp"),
    ] {
        compare_nodes(&parts, expected, prefix, kind);
    }
    chart_bindings(&parts, expected);
}

fn compare_allocated_identities(book: &Workbook, expected: &Value) {
    let sheets = book.sheet_collection_no_check();
    let expected_sheets = expected["sheets"].as_array().unwrap();
    assert_eq!(sheets.len(), expected_sheets.len());
    for (sheet, wanted) in sheets.iter().zip(expected_sheets) {
        assert_eq!(sheet.name(), wanted["name"].as_str().unwrap());
        assert_eq!(sheet.sheet_id(), wanted["id"].as_str().unwrap());
    }
    let expected_tables = sorted(
        expected["parts"]
            .as_object()
            .unwrap()
            .iter()
            .filter(|(path, _)| path.starts_with("xl/tables/"))
            .map(|(_, part)| {
                json!([
                    part["attributes"]["name"],
                    part["attributes"]["id"]
                        .as_str()
                        .unwrap()
                        .parse::<u32>()
                        .unwrap()
                ])
            })
            .collect(),
    );
    let actual_tables = sorted(
        sheets
            .iter()
            .flat_map(|sheet| sheet.tables())
            .map(|table| json!([table.name(), table.id().unwrap()]))
            .collect(),
    );
    assert_eq!(
        actual_tables, expected_tables,
        "table identities are allocated by add_sheet"
    );
    for (prefix, actual) in [
        (
            "xl/slicerCaches/",
            book.slicer_caches()
                .iter()
                .map(|cache| json!(cache.name))
                .collect(),
        ),
        (
            "xl/timelineCaches/",
            book.timeline_caches()
                .iter()
                .map(|cache| json!(cache.name))
                .collect(),
        ),
    ] {
        let wanted = sorted(
            expected["parts"]
                .as_object()
                .unwrap()
                .iter()
                .filter(|(path, _)| path.starts_with(prefix))
                .map(|(_, part)| part["attributes"]["name"].clone())
                .collect(),
        );
        assert_eq!(
            sorted(actual),
            wanted,
            "cache identities are allocated by add_sheet"
        );
    }
    for (prefix, kind, actual) in [
        (
            "xl/slicers/",
            "slicer",
            sheets
                .iter()
                .flat_map(|sheet| sheet.slicers())
                .map(|view| json!(view.name))
                .collect(),
        ),
        (
            "xl/timelines/",
            "timeline",
            sheets
                .iter()
                .flat_map(|sheet| sheet.timelines())
                .map(|view| json!(view.name))
                .collect(),
        ),
    ] {
        let wanted = sorted(
            oracle_nodes(expected, prefix, kind)
                .iter()
                .map(|node| node["name"].clone())
                .collect(),
        );
        assert_eq!(
            sorted(actual),
            wanted,
            "view identities are allocated by add_sheet"
        );
    }
}

fn cases() -> Vec<(&'static str, &'static str)> {
    let mut cases = vec![
        ("table", "sheet_operations/table-copy.xlsx"),
        ("combined", "sheet_operations/combined-copy.xlsx"),
        ("table-imported", "slicers/excel-authored/table-slicer.xlsx"),
        ("pivot", "slicers/excel-authored/pivot-slicers.xlsx"),
        ("pivot-data", "slicers/excel-authored/pivot-slicers.xlsx"),
        (
            "timeline",
            "timelines/excel-authored/two-pivots-custom-style.xlsx",
        ),
        ("drawing", "chart_shapes_rich_excel.xlsx"),
        ("linked", "chart_drawing_linked_excel.xlsx"),
        ("pivotchart", "chart_space_parts/pivotchart.xlsx"),
        (
            "chartex-remote-host",
            "extended_charts/sheet-deletion/named-source.xlsx",
        ),
        (
            "chartex-remote-data",
            "extended_charts/sheet-deletion/named-source.xlsx",
        ),
    ];
    cases.extend([
        (
            "chartex-waterfall",
            "extended_charts/excel-authored/waterfall.xlsx",
        ),
        (
            "chartex-treemap",
            "extended_charts/excel-authored/treemap.xlsx",
        ),
        (
            "chartex-sunburst",
            "extended_charts/excel-authored/sunburst.xlsx",
        ),
        (
            "chartex-histogram",
            "extended_charts/excel-authored/histogram.xlsx",
        ),
        (
            "chartex-pareto",
            "extended_charts/excel-authored/pareto.xlsx",
        ),
        (
            "chartex-box-whisker",
            "extended_charts/excel-authored/box-whisker.xlsx",
        ),
        (
            "chartex-funnel",
            "extended_charts/excel-authored/funnel.xlsx",
        ),
        (
            "chartex-region-map",
            "extended_charts/excel-authored/region-map.xlsx",
        ),
    ]);
    cases
}

fn persist(bytes: &[u8], label: &str) {
    let directory =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/sheet-operations-validation");
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join(format!("{label}.xlsx")), bytes).unwrap();
}

#[test]
fn same_workbook_sheet_copies_match_excel_identities_bindings_and_formulas() {
    let oracle: Value = serde_json::from_str(ORACLE.trim_start_matches('\u{feff}')).unwrap();
    for (case, fixture) in cases() {
        let input = std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures")
                .join(fixture),
        )
        .unwrap();
        for eager in [true, false] {
            for streaming in [false, true] {
                let mut book = reader::xlsx::read_reader(Cursor::new(&input), eager).unwrap();
                let source = oracle[case]["source"].as_str().unwrap();
                let mut copy = book.sheet_by_name_mut(source).unwrap().clone();
                copy.set_name("Copy");
                let expected = append_preserving_selection(&mut book, copy, &oracle[case]["after"]);
                compare_allocated_identities(&book, &expected);
                if streaming && matches!(case, "chartex-remote-host" | "chartex-remote-data") {
                    // Without retained source state, the canonical streaming
                    // boundary rejects a host after its source was flushed.
                    let mut stream = StreamingWriter::new(
                        zip::ZipWriter::new(Cursor::new(Vec::new())),
                        book.clone(),
                    );
                    let source = stream.take_sheet("Sheet1").unwrap();
                    stream.flush_sheet(source).unwrap();
                    let host = stream.take_sheet("ChartHost").unwrap();
                    let error = stream.flush_sheet(host).unwrap_err();
                    assert!(
                        error
                            .to_string()
                            .contains("ChartEx source worksheet is unavailable: Sheet1")
                    );
                    assert!(stream.finish().is_err());
                    continue;
                }
                let bytes = write(&book, streaming);
                persist(&bytes, &format!("copy-{case}-{eager}-{streaming}"));
                compare_saved(&bytes, &expected);
                if case == "timeline" {
                    let before = parts(&input);
                    let after = parts(&bytes);
                    for kind in ["timelineStyle", "timelineStyleElement"] {
                        let attributes = |xml: &[u8]| {
                            sorted(
                                xml_nodes(xml)
                                    .into_iter()
                                    .filter(|node| node.kind == kind)
                                    .map(|node| json!(node.attributes))
                                    .collect(),
                            )
                        };
                        assert_eq!(
                            attributes(&after["xl/styles.xml"]),
                            attributes(&before["xl/styles.xml"]),
                            "copied timeline keeps its custom style definition"
                        );
                    }
                }
                let restored = reader::xlsx::read_reader(Cursor::new(&bytes), true).unwrap();
                assert_eq!(restored.sheet_by_name("Copy").unwrap().name(), "Copy");
                compare_saved(&write(&restored, false), &expected);
            }
        }
    }
}

#[test]
fn repeated_sheet_copies_allocate_the_next_excel_table_and_slicer_names() {
    let oracle: Value = serde_json::from_str(ORACLE.trim_start_matches('\u{feff}')).unwrap();
    for eager in [true, false] {
        for streaming in [false, true] {
            let mut book = reader::xlsx::read_reader(
                Cursor::new(include_bytes!("fixtures/sheet_operations/table-copy.xlsx")),
                eager,
            )
            .unwrap();
            let mut expected = oracle["table-twice"]["after"].clone();
            for name in ["Copy", "Again"] {
                let mut copy = book.sheet_by_name_mut("Data").unwrap().clone();
                copy.set_name(name);
                expected = append_preserving_selection(&mut book, copy, &expected);
            }
            compare_allocated_identities(&book, &expected);
            let bytes = write(&book, streaming);
            persist(&bytes, &format!("copy-table-twice-{eager}-{streaming}"));
            compare_saved(&bytes, &expected);
            let restored = reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap();
            assert_eq!(
                restored.sheet_by_name("Again").unwrap().tables()[0].name(),
                "Scores4"
            );
        }
    }
}

#[test]
fn copying_a_copy_allocates_independent_names_and_rebases_its_own_references() {
    let oracle: Value = serde_json::from_str(ORACLE.trim_start_matches('\u{feff}')).unwrap();
    for eager in [true, false] {
        for streaming in [false, true] {
            let mut book = reader::xlsx::read_reader(
                Cursor::new(include_bytes!("fixtures/sheet_operations/table-copy.xlsx")),
                eager,
            )
            .unwrap();
            let mut expected = oracle["copy-of-copy"].clone();
            for (source, name) in [("Data", "Copy"), ("Data", "Again"), ("Copy", "CopyOfCopy")] {
                let mut copy = book.sheet_by_name_mut(source).unwrap().clone();
                copy.set_name(name);
                expected = append_preserving_selection(&mut book, copy, &expected);
            }
            compare_allocated_identities(&book, &expected);
            let bytes = write(&book, streaming);
            persist(&bytes, &format!("copy-of-copy-{eager}-{streaming}"));
            compare_saved(&bytes, &expected);
            let restored = reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap();
            let copy = restored.sheet_by_name("CopyOfCopy").unwrap();
            assert_eq!(copy.tables()[0].name(), "Scores35");
            assert_eq!(copy.tables()[0].id(), Some(4));
            assert_eq!(copy.slicers()[0].name, "Team 3");
            assert_eq!(copy.slicers()[0].cache, "Slicer_Team11");
        }
    }
}

#[test]
fn workbook_sheet_rename_then_clone_reconciles_both_source_and_copy_references() {
    let oracle: Value = serde_json::from_str(ORACLE.trim_start_matches('\u{feff}')).unwrap();
    let expected = &oracle["rename-copy"];
    assert!(
        expected.is_object(),
        "missing Excel rename-copy measurement"
    );
    for eager in [true, false] {
        for streaming in [false, true] {
            let mut book = reader::xlsx::read_reader(
                Cursor::new(include_bytes!("fixtures/sheet_operations/table-copy.xlsx")),
                eager,
            )
            .unwrap();
            book.set_sheet_name(0, "Bob's Data").unwrap();
            let mut copy = book.sheet_by_name_mut("Bob's Data").unwrap().clone();
            copy.set_name("Copy");
            let expected = append_preserving_selection(&mut book, copy, expected);
            compare_allocated_identities(&book, &expected);
            let bytes = write(&book, streaming);
            persist(
                &bytes,
                &format!("copy-after-workbook-rename-{eager}-{streaming}"),
            );
            compare_saved(&bytes, &expected);
            let restored = reader::xlsx::read_reader(Cursor::new(&bytes), true).unwrap();
            assert_eq!(
                restored
                    .sheet_by_name("Bob's Data")
                    .unwrap()
                    .cell("D2")
                    .unwrap()
                    .formula(),
                "'Bob''s Data'!B2"
            );
            assert_eq!(
                restored
                    .sheet_by_name("Copy")
                    .unwrap()
                    .cell("D2")
                    .unwrap()
                    .formula(),
                "Copy!B2"
            );
            assert_eq!(
                restored
                    .sheet_by_name("Copy")
                    .unwrap()
                    .cell("D1")
                    .unwrap()
                    .formula(),
                "SUM(Scores3[Points])"
            );
            compare_saved(&write(&restored, false), &expected);
        }
    }
}

#[test]
fn copied_local_name_keeps_precedence_over_the_same_spelled_global() {
    let oracle: Value = serde_json::from_str(ORACLE.trim_start_matches('\u{feff}')).unwrap();
    let expected = &oracle["local-global-shadow"];
    assert!(
        expected.is_object(),
        "missing Excel local-global-shadow measurement"
    );
    for eager in [true, false] {
        for streaming in [false, true] {
            let mut book = reader::xlsx::read_reader(
                Cursor::new(include_bytes!("fixtures/sheet_operations/table-copy.xlsx")),
                eager,
            )
            .unwrap();
            let mut global = DefinedName::default();
            global.set_name("Sales").set_address("Data!$B$2");
            book.add_defined_names(global);
            let mut local = DefinedName::default();
            local.set_name("Sales").set_address("Data!$B$3");
            local.set_local_sheet_id(0);
            let source = book.sheet_by_name_mut("Data").unwrap();
            source.add_defined_names(local);
            source.cell_mut("D3").set_formula("Sales");
            let mut copy = source.clone();
            copy.set_name("Copy");
            let expected = append_preserving_selection(&mut book, copy, expected);
            compare_allocated_identities(&book, &expected);
            let bytes = write(&book, streaming);
            persist(
                &bytes,
                &format!("copy-local-global-shadow-{eager}-{streaming}"),
            );
            compare_saved(&bytes, &expected);
            let restored = reader::xlsx::read_reader(Cursor::new(&bytes), true).unwrap();
            assert_eq!(
                restored
                    .sheet_by_name("Copy")
                    .unwrap()
                    .cell("D3")
                    .unwrap()
                    .formula(),
                "Sales"
            );
            compare_saved(&write(&restored, false), &expected);
        }
    }
}

#[test]
fn copied_structured_global_name_becomes_local_to_the_new_table() {
    let text = include_str!("fixtures/sheet_operations/scoped-cache-known-answers.json");
    let oracle: Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).unwrap();
    let expected = &oracle["structured-names-copy"];
    assert!(
        expected.is_object(),
        "missing Excel structured-names-copy measurement"
    );
    for eager in [true, false] {
        for streaming in [false, true] {
            let mut book = reader::xlsx::read_reader(
                Cursor::new(include_bytes!("fixtures/sheet_operations/table-copy.xlsx")),
                eager,
            )
            .unwrap();
            let mut name = DefinedName::default();
            name.set_name("StructuredRows").set_address("Scores[#All]");
            book.add_defined_names(name);
            let mut copy = book.sheet_by_name_mut("Data").unwrap().clone();
            copy.set_name("Copy");
            let expected = append_preserving_selection(&mut book, copy, expected);
            compare_allocated_identities(&book, &expected);
            let bytes = write(&book, streaming);
            persist(
                &bytes,
                &format!("copy-structured-global-name-{eager}-{streaming}"),
            );
            compare_saved(&bytes, &expected);
            let restored = reader::xlsx::read_reader(Cursor::new(&bytes), true).unwrap();
            compare_saved(&write(&restored, false), &expected);
        }
    }
}

#[test]
fn sheet_copy_identity_allocation_respects_native_gaps_and_defined_name_collisions() {
    let oracle: Value = serde_json::from_str(ORACLE.trim_start_matches('\u{feff}')).unwrap();
    for case in ["sheet-hole", "table-hole", "table-name-collision"] {
        for eager in [true, false] {
            for streaming in [false, true] {
                let mut book = reader::xlsx::read_reader(
                    Cursor::new(include_bytes!("fixtures/sheet_operations/table-copy.xlsx")),
                    eager,
                )
                .unwrap();
                match case {
                    "sheet-hole" => {
                        book.new_sheet("Sheet2").unwrap();
                        book.set_sheet_id(1, "3").unwrap();
                    }
                    "table-hole" => {
                        let sheet = book.new_sheet("Other").unwrap();
                        sheet.cell_mut("C1").set_value("Heading");
                        sheet.cell_mut("C2").set_value_number(2);
                        let mut table = Table::new("Kept", ("C1", "C2"));
                        table.set_id(3);
                        table.add_column(TableColumn::new("Heading"));
                        sheet.add_table(table);
                    }
                    "table-name-collision" => {
                        for name in ["Scores3", "Slicer_Team1"] {
                            let mut defined = DefinedName::default();
                            defined.set_name(name).set_address("42");
                            book.add_defined_names(defined);
                        }
                    }
                    _ => unreachable!(),
                }
                let mut copy = book.sheet_by_name_mut("Data").unwrap().clone();
                copy.set_name("Copy");
                let expected = append_preserving_selection(&mut book, copy, &oracle[case]);
                compare_allocated_identities(&book, &expected);
                let bytes = write(&book, streaming);
                persist(&bytes, &format!("copy-{case}-{eager}-{streaming}"));
                compare_saved(&bytes, &expected);
                let restored = reader::xlsx::read_reader(Cursor::new(&bytes), true).unwrap();
                compare_saved(&write(&restored, false), &expected);
            }
        }
    }
}

#[test]
fn copying_remote_slicer_and_timeline_views_shares_the_existing_source_cache() {
    let oracle: Value = serde_json::from_str(ORACLE.trim_start_matches('\u{feff}')).unwrap();
    for kind in ["table", "pivot", "timeline"] {
        let input = std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
                "tests/fixtures/sheet_operations/remote-{kind}-base.xlsx"
            )),
        )
        .unwrap();
        for source in ["Host", "Data"] {
            let case = format!("remote-{kind}-{source}");
            for eager in [true, false] {
                for streaming in [false, true] {
                    let mut book = reader::xlsx::read_reader(Cursor::new(&input), eager).unwrap();
                    let mut copy = book.sheet_by_name_mut(source).unwrap().clone();
                    copy.set_name("Copy");
                    let expected =
                        append_preserving_selection(&mut book, copy, &oracle[&case]["after"]);
                    compare_allocated_identities(&book, &expected);
                    let bytes = write(&book, streaming);
                    persist(&bytes, &format!("copy-{case}-{eager}-{streaming}"));
                    compare_saved(&bytes, &expected);
                    let restored = reader::xlsx::read_reader(Cursor::new(&bytes), true).unwrap();
                    compare_saved(&write(&restored, false), &expected);
                }
            }
        }
    }
}

#[test]
fn hidden_sheet_copy_and_very_hidden_refusal_match_excel() {
    let oracle: Value = serde_json::from_str(ORACLE.trim_start_matches('\u{feff}')).unwrap();
    for (case, state) in [
        ("copy-hidden", SheetStateValues::Hidden),
        ("copy-veryHidden", SheetStateValues::VeryHidden),
    ] {
        let expected = &oracle[case];
        for eager in [true, false] {
            for streaming in [false, true] {
                let mut book = reader::xlsx::read_reader(
                    Cursor::new(include_bytes!("fixtures/sheet_operations/visibility.xlsx")),
                    eager,
                )
                .unwrap();
                let mut copy = {
                    let source = book.sheet_by_name_mut("Alpha").unwrap();
                    source.set_state(state.clone());
                    source.clone()
                };
                copy.set_name("Copy");
                let before = write(&book, false);
                let expected = if expected["accepted"].as_bool().unwrap() {
                    append_preserving_selection(&mut book, copy, expected)
                } else {
                    assert!(book.add_sheet(copy).is_err());
                    expected.clone()
                };
                if !expected["accepted"].as_bool().unwrap() {
                    assert_eq!(
                        write(&book, false),
                        before,
                        "refused very-hidden copy is atomic"
                    );
                }
                compare_allocated_identities(&book, &expected);
                let bytes = write(&book, streaming);
                persist(&bytes, &format!("{case}-{eager}-{streaming}"));
                compare_saved(&bytes, &expected);
                let restored = reader::xlsx::read_reader(Cursor::new(&bytes), true).unwrap();
                compare_saved(&write(&restored, false), &expected);
            }
        }
    }
}

#[test]
fn scoped_named_pivot_cache_sources_follow_sheet_operations_exactly() {
    let text = include_str!("fixtures/sheet_operations/scoped-cache-known-answers.json");
    let oracle: Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).unwrap();
    for operation in ["rename-workbook", "copy", "delete"] {
        let expected = &oracle[if operation.starts_with("rename-") {
            "rename"
        } else {
            operation
        }];
        for eager in [true, false] {
            for streaming in [false, true] {
                let mut book = reader::xlsx::read_reader(
                    Cursor::new(include_bytes!(
                        "fixtures/sheet_operations/scoped-cache.xlsx"
                    )),
                    eager,
                )
                .unwrap();
                let mut expected = expected.clone();
                match operation {
                    "rename-workbook" => book.set_sheet_name(0, "Bob's Data").unwrap(),
                    "copy" => {
                        let mut copy = book.sheet_by_name_mut("Data").unwrap().clone();
                        copy.set_name("Copy");
                        expected = append_preserving_selection(&mut book, copy, &expected);
                    }
                    "delete" => book.remove_sheet_by_name("Data").unwrap(),
                    _ => unreachable!(),
                }
                compare_allocated_identities(&book, &expected);
                let bytes = write(&book, streaming);
                persist(
                    &bytes,
                    &format!("scoped-cache-{operation}-{eager}-{streaming}"),
                );
                compare_saved(&bytes, &expected);
                let restored = reader::xlsx::read_reader(Cursor::new(&bytes), true).unwrap();
                compare_saved(&write(&restored, false), &expected);
            }
        }
    }
}

fn resolve(owner: &str, target: &str) -> String {
    let mut components: Vec<_> = owner.split('/').collect();
    components.pop();
    for component in target.split('/') {
        match component {
            ".." => {
                components.pop().unwrap();
            }
            "." => (),
            _ => components.push(component),
        }
    }
    components.join("/")
}

#[test]
fn copied_charts_own_their_style_color_and_user_drawing_parts() {
    let input = include_bytes!("fixtures/chart_shapes_rich_excel.xlsx");
    for streaming in [false, true] {
        let mut book = reader::xlsx::read_reader(Cursor::new(input), true).unwrap();
        let mut copy = book.sheet_mut(0).unwrap().clone();
        copy.set_name("Copy");
        book.add_sheet(copy).unwrap();
        let bytes = write(&book, streaming);
        let parts = parts(&bytes);
        let mut styles = BTreeSet::new();
        let mut colors = BTreeSet::new();
        let mut drawings = BTreeSet::new();
        let charts: Vec<_> = parts
            .keys()
            .filter(|path| path.starts_with("xl/charts/chart") && path.ends_with(".xml"))
            .collect();
        assert_eq!(charts.len(), 2);
        for chart in charts {
            let (directory, file) = chart.rsplit_once('/').unwrap();
            let relationships = xml_nodes(&parts[&format!("{directory}/_rels/{file}.rels")]);
            for (kind, targets) in [
                ("chartStyle", &mut styles),
                ("chartColorStyle", &mut colors),
                ("chartUserShapes", &mut drawings),
            ] {
                let relation = relationships
                    .iter()
                    .find(|node| {
                        node.kind == "Relationship"
                            && node.attributes["Type"].ends_with(&format!("/{kind}"))
                    })
                    .unwrap();
                let target = resolve(chart, &relation.attributes["Target"]);
                assert!(parts.contains_key(&target));
                assert!(targets.insert(target), "each copied chart owns its {kind}");
            }
        }
        for targets in [&styles, &colors] {
            let targets: Vec<_> = targets.iter().collect();
            assert_eq!(
                parts[targets[0]], parts[targets[1]],
                "private parts retain their Excel-authored contents"
            );
        }
        for drawing in drawings {
            assert_eq!(
                xml_nodes(&parts[&drawing])
                    .iter()
                    .filter(|node| node.kind == "relSizeAnchor")
                    .count(),
                6
            );
        }
    }
}

#[test]
fn an_imported_sheet_with_a_colliding_sheet_id_keeps_its_distinct_table_identity() {
    let mut original = umya_spreadsheet::new_file();
    let source = original.sheet_mut(0).unwrap();
    source.cell_mut("A1").set_value("Heading");
    source.cell_mut("A2").set_value_number(1);
    let mut table = Table::new("OriginalTable", ("A1", "A2"));
    table.set_id(33).add_column(TableColumn::new("Heading"));
    source.add_table(table);
    let input = write(&original, false);
    let mut imported = umya_spreadsheet::new_file();
    imported
        .set_sheet_id(0, original.sheet(0).unwrap().sheet_id())
        .unwrap();
    let sheet = imported.sheet_mut(0).unwrap();
    sheet.set_name("Imported");
    sheet.cell_mut("A1").set_value("Heading");
    sheet.cell_mut("A2").set_value_number(2);
    let mut table = Table::new("ImportedTable", ("A1", "A2"));
    table.set_id(44).add_column(TableColumn::new("Heading"));
    sheet.add_table(table);
    let incoming = write(&imported, false);
    for eager in [true, false] {
        for incoming_eager in [true, false] {
            let mut book = reader::xlsx::read_reader(Cursor::new(&input), eager).unwrap();
            let imported =
                reader::xlsx::read_reader(Cursor::new(&incoming), incoming_eager).unwrap();
            book.add_sheet(imported.sheet_collection_no_check()[0].clone())
                .unwrap();
            assert_eq!(
                book.workbook_view().active_tab(),
                original.workbook_view().active_tab()
            );
            if !eager {
                assert!(
                    book.sheet(0).is_err(),
                    "import fallback keeps the original sheet lazy"
                );
            }
            if !incoming_eager {
                assert!(
                    book.sheet_by_name("Imported").is_err(),
                    "import fallback keeps the incoming sheet lazy"
                );
            } else {
                assert_eq!(
                    book.sheet_by_name("Imported").unwrap().tables()[0].id(),
                    Some(44)
                );
                assert_eq!(
                    book.sheet_by_name("Imported").unwrap().tables()[0].name(),
                    "ImportedTable"
                );
            }
            if !eager && !incoming_eager {
                // Canonical raw import does not merge two packages' table1.xml
                // owners. Keep that validation; explicit materialization uses
                // their distinct typed table identities before export.
                let mut output = Vec::new();
                let error = writer::xlsx::write_writer(&book, &mut output).unwrap_err();
                assert!(error.to_string().contains("table part has multiple owners"));
                assert!(output.is_empty());
                book.try_read_sheet_collection().unwrap();
            }
            for streaming in [false, true] {
                let bytes = write(&book, streaming);
                persist(
                    &bytes,
                    &format!("import-distinct-table-{eager}-{incoming_eager}-{streaming}"),
                );
                let restored = reader::xlsx::read_reader(Cursor::new(&bytes), true).unwrap();
                assert_eq!(
                    restored.workbook_view().active_tab(),
                    original.workbook_view().active_tab()
                );
                let imported = restored.sheet_by_name("Imported").unwrap();
                assert_eq!(imported.tables()[0].id(), Some(44));
                assert_eq!(imported.tables()[0].name(), "ImportedTable");
                assert_ne!(imported.sheet_id(), original.sheet(0).unwrap().sheet_id());
            }
        }
    }
}

#[test]
fn raw_sheet_clones_allocate_copy_identities_with_raw_or_materialized_sources() {
    let oracle: Value = serde_json::from_str(ORACLE.trim_start_matches('\u{feff}')).unwrap();
    let input = include_bytes!("fixtures/sheet_operations/table-copy.xlsx");
    for materialize_source in [false, true] {
        let mut book = reader::xlsx::read_reader(Cursor::new(input), false).unwrap();
        let mut copy = book.sheet_collection_no_check()[0].clone();
        copy.set_name("Copy");
        if materialize_source {
            book.sheet_mut(0).unwrap();
        }
        let baseline = reader::xlsx::read_reader(Cursor::new(input), true).unwrap();
        let active = baseline.workbook_view().active_tab();
        let mut selected = baseline
            .sheet_collection_no_check()
            .iter()
            .map(|sheet| {
                sheet
                    .sheets_views()
                    .sheet_view_list()
                    .first()
                    .is_some_and(|view| view.tab_selected())
            })
            .collect::<Vec<_>>();
        selected.push(selected[0]);
        let mut expected = oracle["table"]["after"].clone();
        expected["active_tab"] = json!(active);
        for (sheet, selected) in expected["sheets"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .zip(selected)
        {
            sheet["selected"] = json!(selected);
        }
        book.add_sheet(copy).unwrap();
        assert_eq!(book.workbook_view().active_tab(), active);
        compare_allocated_identities(&book, &expected);
        for streaming in [false, true] {
            let bytes = write(&book, streaming);
            persist(
                &bytes,
                &format!("copy-raw-{materialize_source}-{streaming}"),
            );
            compare_saved(&bytes, &expected);
            let restored = reader::xlsx::read_reader(Cursor::new(&bytes), true).unwrap();
            assert_eq!(
                restored.sheet_by_name("Copy").unwrap().tables()[0].id(),
                Some(2)
            );
            assert_eq!(
                restored.sheet_by_name("Copy").unwrap().tables()[0].name(),
                "Scores3"
            );
            compare_saved(&write(&restored, false), &expected);
        }
    }
}
