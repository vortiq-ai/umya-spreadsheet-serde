#![cfg(feature = "serde")]
//! Workbooks Excel itself authored (`slicer_excel_author.ps1`, committed under
//! `tests/fixtures/slicers/excel-authored` with a SHA-256 manifest): read with
//! every reader, edit something unrelated, write, and check that everything
//! the Excel file binds survives with the parts and relationship ids the
//! writer owns. `slicer_excel_check.ps1` opens the same files and the output
//! of these tests in Excel and compares what Excel reports.
#[path = "support/slicer_package_fixture.rs"]
mod fixture;

use fixture::*;
use serde_json::Value;
use sha2::{
    Digest,
    Sha256,
};
use umya_spreadsheet::SlicerSource;

const DIRECTORY: &str = "tests/fixtures/slicers/excel-authored";

fn manifest() -> Value {
    serde_json::from_slice(&std::fs::read(format!("{DIRECTORY}/manifest.json")).unwrap()).unwrap()
}

/// The authored workbook, checked against the manifest hash.
fn sample(name: &str) -> Vec<u8> {
    let bytes = std::fs::read(format!("{DIRECTORY}/{name}.xlsx")).unwrap();
    let hash: String = Sha256::digest(&bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    assert_eq!(
        manifest()[name]["workbook_sha256"].as_str(),
        Some(hash.as_str()),
        "{name} does not match the manifest"
    );
    bytes
}

/// The text without namespace declarations, whose placement the writer owns.
fn normalized(xml: &str) -> String {
    let mut result = String::new();
    let mut rest = xml;
    while let Some(start) = rest.find(" xmlns") {
        result.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let equals = after.find('=').unwrap();
        let quote = after[equals + 1..].chars().next().unwrap();
        let close = after[equals + 2..].find(quote).unwrap();
        rest = &after[equals + 2 + close + 1..];
    }
    result.push_str(rest);
    collapse_empty_elements(&result)
}

/// `<a b="1"></a>` written as `<a b="1"/>`: the same element either way.
fn collapse_empty_elements(xml: &str) -> String {
    let mut result = xml.to_owned();
    let mut from = 0;
    while let Some(found) = result[from..].find("></") {
        let at = from + found;
        let close_end = result[at + 3..].find('>').map(|end| at + 3 + end);
        let open_start = result[..at].rfind('<');
        if let (Some(close_end), Some(open_start)) = (close_end, open_start) {
            let name = &result[at + 3..close_end];
            let open = &result[open_start + 1..at];
            let open_name = open.split([' ', '/']).next().unwrap_or("");
            if open_name == name && !open.starts_with('/') && !open.ends_with('/') {
                result.replace_range(at..=close_end, "/>");
                from = open_start;
                continue;
            }
        }
        from = at + 1;
    }
    result
}

/// Every `<ext ...>...</ext>` of the last `extLst` of `xml`, as
/// `(uri, normalized text)` in order.
fn extension_entries(xml: &str) -> Vec<(String, String)> {
    let Some(list) = xml.rfind("<extLst>") else {
        return Vec::new();
    };
    let mut rest = &xml[list..];
    let mut entries = Vec::new();
    while let Some(start) = rest.find("<ext ") {
        let end = rest[start..].find("</ext>").unwrap() + start + "</ext>".len();
        let block = &rest[start..end];
        entries.push((attribute(block, "uri").unwrap(), normalized(block)));
        rest = &rest[end..];
    }
    entries
}

fn relationship_ids(block: &str) -> Vec<String> {
    let mut ids = Vec::new();
    let mut rest = block;
    while let Some(at) = rest.find(" r:id=\"") {
        let after = &rest[at + " r:id=\"".len()..];
        let end = after.find('"').unwrap();
        ids.push(after[..end].to_owned());
        rest = &after[end..];
    }
    ids
}

fn part_bytes(parts: &Parts, name: &str) -> String {
    parts
        .get(name)
        .unwrap_or_else(|| panic!("missing part {name}"))
        .clone()
}

/// The relationship type and part content each `r:id` of `block` resolves to.
fn resolved(parts: &Parts, owner: &str, block: &str) -> Vec<(String, String)> {
    relationship_ids(block)
        .iter()
        .map(|id| {
            let (kind, part) = resolve(parts, owner, id);
            (kind, part_bytes(parts, &part))
        })
        .collect()
}

fn is_slicer_extension(uri: &str) -> bool {
    [
        "{BBE1A952-AA13-448e-AADC-164F8A28A991}",
        "{46BE6895-7355-4a93-B00E-2C351335B9C9}",
        "{A8765BA9-456A-4dab-B4F3-ACF838C121DE}",
        "{3A4CF648-6AED-40f4-86FF-DC5316D8AED3}",
    ]
    .contains(&uri)
}

/// Every extension of every owner keeps its order, its XML and the parts its
/// relationships name; the slicer lists are rebuilt for the parts written.
fn assert_extensions_preserved(source: &Parts, output: &Parts, label: &str) {
    let owners: Vec<String> = source
        .keys()
        .filter(|name| name.starts_with("xl/worksheets/sheet") || *name == "xl/workbook.xml")
        .cloned()
        .collect();
    for owner in owners {
        let before = extension_entries(&source[&owner]);
        let after = extension_entries(&output[&owner]);
        assert_eq!(
            before.iter().map(|(uri, _)| uri).collect::<Vec<_>>(),
            after.iter().map(|(uri, _)| uri).collect::<Vec<_>>(),
            "{label}: {owner} extension order"
        );
        for ((uri, source_text), (_, output_text)) in before.iter().zip(&after) {
            let source_resolved = resolved(source, &owner, source_text);
            let output_resolved = resolved(output, &owner, output_text);
            assert_eq!(
                source_resolved.len(),
                output_resolved.len(),
                "{label}: {owner} {uri} relationship count"
            );
            if is_slicer_extension(uri) {
                // The parts are regenerated, so compare their kinds only.
                for (before, after) in source_resolved.iter().zip(&output_resolved) {
                    assert_eq!(
                        before.0, after.0,
                        "{label}: {owner} {uri} relationship type"
                    );
                }
                continue;
            }
            for (before, after) in source_resolved.iter().zip(&output_resolved) {
                assert_eq!(before, after, "{label}: {owner} {uri} carried part");
            }
            let strip = |text: &str| {
                let mut result = text.to_owned();
                for id in relationship_ids(text) {
                    result = result.replace(&format!("r:id=\"{id}\""), "r:id=\"\"");
                }
                result
            };
            assert_eq!(
                strip(source_text),
                strip(output_text),
                "{label}: {owner} {uri} entry"
            );
        }
    }
}

fn round_trip(name: &'static str, label: &str, read: Reader) -> (Parts, Parts) {
    let bytes = sample(name);
    let source = unpack(&bytes);
    let mut book = read(&bytes).unwrap();
    book.try_read_sheet_collection().unwrap();
    book.sheet_by_name_mut("Data")
        .unwrap()
        .cell_mut("H1")
        .set_value("edited");
    let output = save(&book);
    keep_output(&format!("excel-{name}-{label}"), &output);
    (source, unpack(&output))
}

#[test]
fn excel_authored_extensions_survive_every_reader() {
    for name in [
        "pivot-slicers",
        "table-slicer",
        "timeline",
        "sheet-extensions",
    ] {
        for (label, read) in readers() {
            let (source, output) = round_trip(name, label, read);
            assert_extensions_preserved(&source, &output, &format!("{name}/{label}"));
            // Everything the first save wrote is stable under a second one.
            let mut book = read(&sample(name)).unwrap();
            book.try_read_sheet_collection().unwrap();
            let first_bytes = save(&book);
            let mut again = read_eager(&first_bytes).unwrap();
            again.try_read_sheet_collection().unwrap();
            let second = unpack(&save(&again));
            let first = unpack(&first_bytes);
            for part in first.keys().filter(|part| {
                part.starts_with("xl/worksheets/sheet") || *part == "xl/workbook.xml"
            }) {
                let texts = |parts: &Parts| -> Vec<(String, Vec<(String, String)>)> {
                    extension_entries(&parts[part])
                        .into_iter()
                        .map(|(uri, text)| (uri, resolved(parts, part, &text)))
                        .collect()
                };
                assert_eq!(
                    texts(&first),
                    texts(&second),
                    "{name}/{label}: {part} second save"
                );
            }
        }
    }
}

#[test]
fn pivot_slicers_bind_what_excel_reports() {
    for (label, read) in readers() {
        let mut book = read(&sample("pivot-slicers")).unwrap();
        book.try_read_sheet_collection().unwrap();
        let caches = book.slicer_caches();
        let state = &manifest()["pivot-slicers"]["slicers"];
        assert_eq!(
            caches.len(),
            state.as_array().unwrap().len(),
            "{label}: cache count Excel reported"
        );
        for expected in state.as_array().unwrap() {
            let name = expected["name"].as_str().unwrap();
            let cache = caches
                .iter()
                .find(|cache| cache.name == name)
                .unwrap_or_else(|| panic!("{label}: cache {name}"));
            let mut pivots: Vec<_> = cache
                .pivot_tables
                .as_ref()
                .unwrap()
                .iter()
                .map(|pivot| pivot.name.clone())
                .collect();
            pivots.sort();
            let wanted: Vec<_> = expected["pivots"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| value.as_str().unwrap().to_owned())
                .collect();
            assert_eq!(pivots, wanted, "{label}: {name} pivot tables");
            let slicers: Vec<_> = book
                .sheet_by_name("Report")
                .unwrap()
                .slicers()
                .iter()
                .filter(|slicer| slicer.cache == name)
                .collect();
            assert_eq!(
                slicers.len() as u64,
                expected["slicers"].as_u64().unwrap(),
                "{label}: {name} slicer count"
            );
        }
        // Excel reported Opp deselected and ETV selected on Slicer_Team.
        let team = caches
            .iter()
            .find(|cache| cache.name == "Slicer_Team")
            .unwrap();
        let Some(SlicerSource::TabularPivot(tabular)) = &team.source else {
            panic!("{label}: tabular source");
        };
        assert_eq!(
            tabular
                .items
                .as_ref()
                .unwrap()
                .items
                .iter()
                .map(|item| (item.index, item.selected))
                .collect::<Vec<_>>(),
            vec![(0, Some(true)), (1, None)],
            "{label}: selected items"
        );
    }
}

#[test]
fn table_slicer_binds_what_excel_reports() {
    for (label, read) in readers() {
        let mut book = read(&sample("table-slicer")).unwrap();
        book.try_read_sheet_collection().unwrap();
        let expected = &manifest()["table-slicer"]["slicers"][0];
        let cache = &book.slicer_caches()[0];
        assert_eq!(cache.name, expected["name"].as_str().unwrap(), "{label}");
        let Some(SlicerSource::Table(table)) = &cache.source else {
            panic!("{label}: table source");
        };
        let sheet = book.sheet_by_name("Data").unwrap();
        let bound = sheet
            .tables()
            .iter()
            .find(|candidate| candidate.id() == Some(table.table_id))
            .unwrap_or_else(|| panic!("{label}: table id {}", table.table_id));
        assert_eq!(bound.name(), expected["table"].as_str().unwrap(), "{label}");
        assert_eq!(sheet.slicers().len(), 1, "{label}");
        assert_eq!(sheet.slicers()[0].cache, cache.name, "{label}");
    }
}

#[test]
fn timeline_parts_and_drawing_frame_are_carried() {
    for (label, read) in readers() {
        let (source, output) = round_trip("timeline", label, read);
        let drawing = |parts: &Parts| {
            parts
                .iter()
                .find(|(name, _)| name.starts_with("xl/drawings/drawing"))
                .unwrap()
                .1
                .clone()
        };
        // The `tsle` graphic frame and its fallback shape are written back as
        // read.
        assert_eq!(
            normalized(&drawing(&source)),
            normalized(&drawing(&output)),
            "{label}: timeline drawing"
        );
        let cache_part = |parts: &Parts| {
            parts
                .iter()
                .find(|(name, _)| name.starts_with("xl/timelineCaches/"))
                .map(|(_, xml)| xml.clone())
                .unwrap()
        };
        assert_eq!(
            cache_part(&source),
            cache_part(&output),
            "{label}: timeline cache"
        );
        let timeline = |parts: &Parts| {
            parts
                .iter()
                .find(|(name, _)| name.starts_with("xl/timelines/"))
                .map(|(_, xml)| xml.clone())
                .unwrap()
        };
        assert_eq!(timeline(&source), timeline(&output), "{label}: timeline");
        // Both parts keep the content type Excel declared.
        for prefix in ["xl/timelineCaches/", "xl/timelines/"] {
            let name_of = |parts: &Parts| {
                parts
                    .keys()
                    .find(|name| name.starts_with(prefix))
                    .unwrap()
                    .clone()
            };
            assert_eq!(
                content_type(&source, &name_of(&source)),
                content_type(&output, &name_of(&output)),
                "{label}: {prefix} content type"
            );
        }
    }
}

#[test]
fn sheet_extension_elements_and_conditional_formats_round_trip() {
    for (label, read) in readers() {
        let (source, output) = round_trip("sheet-extensions", label, read);
        for sheet in ["Data", "Report"] {
            let before = part_bytes(&source, &sheet_part_of(&source, sheet));
            let after = part_bytes(&output, &sheet_part_of(&output, sheet));
            for element in ["protectedRanges", "ignoredErrors", "conditionalFormatting"] {
                let block = |xml: &str| -> Vec<String> {
                    let mut blocks = Vec::new();
                    let mut rest = xml;
                    while let Some(start) = rest.find(&format!("<{element}")) {
                        let close = format!("</{element}>");
                        let end = rest[start..]
                            .find(&close)
                            .map(|at| at + start + close.len())
                            .unwrap();
                        blocks.push(normalized(&rest[start..end]));
                        rest = &rest[end..];
                    }
                    blocks
                };
                assert_eq!(block(&before), block(&after), "{label}: {sheet} {element}");
            }
        }
        assert!(
            part_bytes(&output, &sheet_part_of(&output, "Report"))
                .contains("<conditionalFormatting pivot=\"1\""),
            "{label}: the pivot conditional format keeps its pivot attribute"
        );
    }
}
