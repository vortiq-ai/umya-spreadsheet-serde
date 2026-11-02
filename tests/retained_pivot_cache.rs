#![cfg(feature = "serde")]
//! Pivot caches kept without a pivot table for the slicers and timelines that
//! still read them. Expected values come from Excel: the two `*-without-pivot-
//! tables` fixtures are Excel's own saves after clearing every pivot table
//! (`TableRange2.Clear()`), of `pivot-slicers.xlsx` and of
//! `two-pivots-custom-style.xlsx` with a slicer added on `Team`.

#[path = "support/slicer_package_fixture.rs"]
mod fixture;

use std::io::Cursor;

use fixture::{
    Parts,
    attribute,
    ext_uris,
    keep_output,
    readers,
    resolve,
    save,
    tags,
    unpack,
};
use sha2::{
    Digest,
    Sha256,
};
use umya_spreadsheet::{
    Workbook,
    writer::{
        self,
        streaming_writer::StreamingWriter,
    },
};

const EXCEL_DIRECTORY: &str = "tests/fixtures/timelines/excel-authored";
const SLICER_PIVOT_CACHES_URI: &str = "{876F7934-8845-4945-9796-88D515C7AA90}";
const TIMELINE_PIVOT_CACHES_URI: &str = "{A2CB5862-8E78-49c6-8D9D-AF26E26ADB89}";
const PIVOT_CACHE_DEFINITION_REL: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/pivotCacheDefinition";
const PIVOT_CACHE_RECORDS_REL: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/pivotCacheRecords";

/// An Excel fixture of this directory, checked against its manifest hash.
fn excel(name: &str) -> Vec<u8> {
    let bytes = std::fs::read(format!("{EXCEL_DIRECTORY}/{name}.xlsx")).unwrap();
    let hash: String = Sha256::digest(&bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(format!("{EXCEL_DIRECTORY}/manifest.json")).unwrap())
            .unwrap();
    assert_eq!(
        manifest[name]["workbook_sha256"].as_str(),
        Some(hash.as_str()),
        "{name} does not match the manifest"
    );
    bytes
}

/// The package written by the normal, the light and the streaming writer.
fn written(book: &Workbook) -> Vec<(&'static str, Vec<u8>)> {
    let mut light = Vec::new();
    writer::xlsx::write_writer_light(book, &mut light).unwrap();
    let names: Vec<_> = book
        .sheet_collection_no_check()
        .iter()
        .map(|sheet| sheet.name().to_owned())
        .collect();
    let mut stream =
        StreamingWriter::new(zip::ZipWriter::new(Cursor::new(Vec::new())), book.clone());
    for name in names {
        let sheet = stream.take_sheet(&name).unwrap();
        stream.flush_sheet(sheet).unwrap();
    }
    let streamed = stream.finish().unwrap().into_inner();
    vec![
        ("normal", save(book)),
        ("light", light),
        ("streaming", streamed),
    ]
}

/// The pivot cache entries of the workbook extension `uri`, each resolved to
/// its definition part.
fn retained_entries(parts: &Parts, uri: &str) -> Vec<(String, String)> {
    let workbook = &parts["xl/workbook.xml"];
    let Some(start) = workbook.find(&format!("<ext uri=\"{uri}\"")) else {
        return Vec::new();
    };
    let end = start + workbook[start..].find("</ext>").unwrap();
    tags(&workbook[start..end], "<pivotCache")
        .into_iter()
        .map(|entry| {
            let (kind, part) =
                resolve(parts, "xl/workbook.xml", &attribute(entry, "r:id").unwrap());
            assert_eq!(kind, PIVOT_CACHE_DEFINITION_REL);
            (attribute(entry, "cacheId").unwrap(), part)
        })
        .collect()
}

/// The records part of a pivot cache definition part.
fn records_of<'a>(parts: &'a Parts, definition: &str) -> &'a str {
    let id = attribute(tags(&parts[definition], "<pivotCacheDefinition")[0], "r:id").unwrap();
    let (kind, part) = resolve(parts, definition, &id);
    assert_eq!(kind, PIVOT_CACHE_RECORDS_REL);
    &parts[&part]
}

/// What a definition part says about its cache: worksheet source, field
/// names and x14 pivot cache id.
fn cache_facts(xml: &str) -> (Vec<Option<String>>, Vec<String>, Option<String>) {
    let source = tags(xml, "<worksheetSource")[0];
    (
        vec![attribute(source, "sheet"), attribute(source, "ref")],
        tags(xml, "<cacheField")
            .into_iter()
            .map(|field| attribute(field, "name").unwrap())
            .collect(),
        tags(xml, "<x14:pivotCacheDefinition")
            .first()
            .and_then(|tag| attribute(tag, "pivotCacheId")),
    )
}

/// The slicer cache part named `name`, as the text after its root start tag.
fn slicer_cache<'a>(parts: &'a Parts, name: &str) -> &'a str {
    let needle = format!("name=\"{name}\"");
    let xml = parts
        .values()
        .find(|xml| xml.contains("<slicerCacheDefinition") && xml.contains(&needle))
        .unwrap_or_else(|| panic!("no slicer cache part {name}"));
    &xml[xml.find("<slicerCacheDefinition").unwrap()..]
}

/// The cached items of a slicer cache part: index, selected, without data
/// (`xsd:boolean`, so `1` and `true` are the same).
fn items(xml: &str) -> Vec<(String, bool, bool)> {
    let flag = |tag: &str, name: &str| {
        attribute(tag, name).is_some_and(|value| value == "1" || value == "true")
    };
    tags(xml, "<i")
        .into_iter()
        .map(|tag| {
            (
                attribute(tag, "x").unwrap(),
                flag(tag, "s"),
                flag(tag, "nd"),
            )
        })
        .collect()
}

fn defined_names(parts: &Parts) -> Vec<String> {
    tags(&parts["xl/workbook.xml"], "<definedName")
        .into_iter()
        .map(|tag| attribute(tag, "name").unwrap())
        .collect()
}

#[test]
fn excel_files_with_retained_slicer_pivot_caches_are_read_and_written_as_excel_wrote_them() {
    // Excel lists a cache read by a slicer and by timelines in x14:pivotCaches
    // only.
    for (name, slicers, timelines, x14_id) in [
        (
            "slicers-without-pivot-tables",
            vec!["Slicer_Team", "Slicer_Player"],
            vec![],
            2_056_832_194,
        ),
        (
            "timeline-and-slicer-without-pivot-tables",
            vec!["Slicer_Team"],
            vec!["NativeTimeline_Day", "NativeTimeline_Ship"],
            2_060_443_253,
        ),
    ] {
        let source = excel(name);
        let excel = unpack(&source);
        assert!(
            !excel["xl/workbook.xml"].contains("<pivotCaches>"),
            "{name}"
        );
        let entries = retained_entries(&excel, SLICER_PIVOT_CACHES_URI);
        assert_eq!(entries.len(), 1, "{name}");
        assert!(retained_entries(&excel, TIMELINE_PIVOT_CACHES_URI).is_empty());
        for slicer in &slicers {
            assert!(!slicer_cache(&excel, slicer).contains("pivotTables"));
        }
        let excel_records = records_of(&excel, &entries[0].1);
        let check = |book: &Workbook, label: &str| {
            let sheets = book.sheet_collection_no_check();
            assert!(
                sheets.iter().all(|sheet| sheet.pivot_tables().is_empty()),
                "{label}"
            );
            assert_eq!(
                sheets
                    .iter()
                    .map(|sheet| sheet.slicers().len())
                    .sum::<usize>(),
                slicers.len(),
                "{label}"
            );
            assert_eq!(
                book.sheet_by_name("Report").unwrap().slicers().len(),
                slicers.len(),
                "{label}"
            );
            assert_eq!(
                book.slicer_caches()
                    .iter()
                    .map(|cache| (cache.name.as_str(), cache.linked_pivots().len()))
                    .collect::<Vec<_>>(),
                slicers.iter().map(|name| (*name, 0)).collect::<Vec<_>>(),
                "{label}"
            );
            assert_eq!(
                book.timeline_caches()
                    .iter()
                    .map(|cache| (cache.name.as_str(), cache.linked_pivots().len()))
                    .collect::<Vec<_>>(),
                timelines.iter().map(|name| (*name, 0)).collect::<Vec<_>>(),
                "{label}"
            );
            let retained: Vec<_> = book.retained_pivot_caches().collect();
            assert_eq!(retained.len(), 1, "{label}");
            assert_eq!(retained[0].1.slicer_cache_id(), Some(x14_id), "{label}");
            assert_eq!(
                retained[0].1.raw_cache_records(),
                excel_records.as_bytes(),
                "{label}"
            );
        };
        for (reader, read) in readers() {
            let mut book = read(&source).unwrap();
            book.try_read_sheet_collection().unwrap();
            check(&book, &format!("{name} {reader}"));
            for (writer, bytes) in written(&book) {
                let label = format!("{name} {reader} {writer}");
                let parts = unpack(&bytes);
                assert!(
                    !parts["xl/workbook.xml"].contains("<pivotCaches>"),
                    "{label}"
                );
                assert_eq!(
                    ext_uris(&parts["xl/workbook.xml"]),
                    ext_uris(&excel["xl/workbook.xml"]),
                    "{label}"
                );
                let written = retained_entries(&parts, SLICER_PIVOT_CACHES_URI);
                assert_eq!(written.len(), 1, "{label}");
                assert_eq!(written[0].0, entries[0].0, "{label}");
                assert_eq!(
                    cache_facts(&parts[&written[0].1]),
                    cache_facts(&excel[&entries[0].1]),
                    "{label}"
                );
                assert_eq!(defined_names(&parts), defined_names(&excel), "{label}");
                for slicer in &slicers {
                    let ours = slicer_cache(&parts, slicer);
                    let theirs = slicer_cache(&excel, slicer);
                    assert!(!ours.contains("pivotTables"), "{label} {slicer}");
                    assert_eq!(
                        attribute(tags(ours, "<tabular")[0], "pivotCacheId"),
                        attribute(tags(theirs, "<tabular")[0], "pivotCacheId"),
                        "{label} {slicer}"
                    );
                    assert_eq!(items(ours), items(theirs), "{label} {slicer}");
                }
                assert_eq!(records_of(&parts, &written[0].1), excel_records, "{label}");
                for (rereader, reread) in readers() {
                    let mut again = reread(&bytes).unwrap();
                    again.try_read_sheet_collection().unwrap();
                    check(&again, &format!("{label} {rereader}"));
                }
                keep_output(&format!("{name}-{reader}-{writer}"), &bytes);
            }
        }
    }
}
