#![cfg(feature = "serde")]
//! An element written as a start tag immediately followed by its end tag is the
//! same element as the self-closing form (XML 1.0, section 3.1), so a chart
//! part must import the same either way.
use std::{
    collections::{
        BTreeMap,
        HashMap,
    },
    hash::Hash,
    io::{
        Cursor,
        Read,
        Write,
    },
    path::{
        Path,
        PathBuf,
    },
};

use quick_xml::{
    Reader,
    Writer,
    events::Event,
};
use serde::{
    Serialize,
    de::DeserializeOwned,
};
use sha2::{
    Digest,
    Sha256,
};

use crate as umya_spreadsheet;
use crate::{
    Workbook,
    Worksheet,
    XlsxError,
    XlsxErrorKind,
};

fn is_drawing_part(name: &str) -> bool {
    name.strip_prefix("xl/drawings/drawing")
        .and_then(|file| file.strip_suffix(".xml"))
        .is_some_and(|digits| !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()))
}

fn is_chart_part(name: &str) -> bool {
    let Some(file) = name.strip_prefix("xl/charts/") else {
        return false;
    };
    let stem = file.strip_suffix(".xml").unwrap_or("");
    let digits = stem
        .strip_prefix("chartEx")
        .or_else(|| stem.strip_prefix("chart"))
        .unwrap_or("");
    !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())
}

/// Every self-closing element outside extLst becomes a start tag plus an end
/// tag. Returns the rewritten XML and how many elements were split.
fn split_empty_elements(xml: &[u8]) -> (Vec<u8>, usize) {
    let mut reader = Reader::from_reader(xml);
    let mut writer = Writer::new(Vec::new());
    let mut split = 0;
    // Leave extension payloads unchanged, including registered typed payloads.
    // This regression covers the chart/drawing owners outside those lists.
    let mut in_extensions = 0;
    loop {
        match reader.read_event().unwrap() {
            Event::Eof => break,
            Event::Start(element) if element.local_name().as_ref() == b"extLst" => {
                in_extensions += 1;
                writer.write_event(Event::Start(element)).unwrap();
            }
            Event::End(element) if element.local_name().as_ref() == b"extLst" => {
                in_extensions -= 1;
                writer.write_event(Event::End(element)).unwrap();
            }
            Event::Empty(element) if in_extensions == 0 => {
                split += 1;
                let end = element.to_end().into_owned();
                writer.write_event(Event::Start(element)).unwrap();
                writer.write_event(Event::End(end)).unwrap();
            }
            other => writer.write_event(other).unwrap(),
        }
    }
    (writer.into_inner(), split)
}

/// Worksheet drawings (`xdr:wsDr`) go through the typed drawing reader. A
/// chart's own drawing (`c:userShapes`, also under `xl/drawings`) is a
/// preserved package part written back byte for byte, so no reader owns its
/// leaves and it stays as authored.
fn is_worksheet_drawing(xml: &[u8]) -> bool {
    let mut reader = Reader::from_reader(xml);
    loop {
        match reader.read_event().unwrap() {
            Event::Start(root) | Event::Empty(root) => {
                return root.local_name().as_ref() == b"wsDr";
            }
            Event::Eof => return false,
            _ => (),
        }
    }
}

/// The package with its chart/drawing parts split, and the number of elements
/// split.
fn split_chart_parts(package: &[u8]) -> (Vec<u8>, usize) {
    let mut input = zip::ZipArchive::new(Cursor::new(package)).unwrap();
    let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let mut total = 0;
    for index in 0..input.len() {
        let mut entry = input.by_index(index).unwrap();
        let name = entry.name().unwrap().into_owned();
        let mut data = Vec::new();
        entry.read_to_end(&mut data).unwrap();
        if is_chart_part(&name) || (is_drawing_part(&name) && is_worksheet_drawing(&data)) {
            let (split, count) = split_empty_elements(&data);
            data = split;
            total += count;
        }
        output
            .start_file(&name, zip::write::SimpleFileOptions::default())
            .unwrap();
        output.write_all(&data).unwrap();
    }
    (output.finish().unwrap().into_inner(), total)
}

fn has_chart_or_drawing_part(package: &[u8]) -> bool {
    let archive = zip::ZipArchive::new(Cursor::new(package)).unwrap();
    let found = archive
        .file_names()
        .any(|name| name.is_ok_and(|name| is_chart_part(&name) || is_drawing_part(&name)));
    found
}

struct Imported {
    full:       Vec<u8>,
    light:      Vec<u8>,
    projection: Vec<serde_json::Value>,
    binary:     Vec<u8>,
    sheets:     Vec<Worksheet>,
}

fn outcome(mut book: Workbook) -> Result<Imported, XlsxError> {
    book.try_read_sheet_collection()?;
    let mut full = Cursor::new(Vec::new());
    umya_spreadsheet::writer::xlsx::write_writer(&book, &mut full).unwrap();
    let mut light = Cursor::new(Vec::new());
    umya_spreadsheet::writer::xlsx::write_writer_light(&book, &mut light).unwrap();
    Ok(Imported {
        full:       full.into_inner(),
        light:      light.into_inner(),
        projection: book
            .sheet_collection()
            .iter()
            .flat_map(|sheet| sheet.chart_collection())
            .map(|chart| serde_json::to_value(chart).unwrap())
            .collect(),
        binary:     bincode::serialize(book.sheet_collection()).unwrap(),
        sheets:     book.sheet_collection().to_vec(),
    })
}

fn eager(package: &[u8]) -> Result<Imported, XlsxError> {
    umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(package.to_vec()), true)
        .and_then(outcome)
}

fn lazy(package: &[u8], label: &str) -> Result<Imported, XlsxError> {
    let path: PathBuf =
        std::env::temp_dir().join(format!("umya-paired-{label}-{}.xlsx", std::process::id()));
    std::fs::write(&path, package).unwrap();
    let imported = umya_spreadsheet::reader::xlsx::lazy_read(&path).and_then(outcome);
    std::fs::remove_file(&path).unwrap();
    imported
}

fn parts(package: &[u8]) -> Vec<(String, Vec<u8>)> {
    let mut archive = zip::ZipArchive::new(Cursor::new(package)).unwrap();
    let mut parts = Vec::new();
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).unwrap();
        let mut data = Vec::new();
        entry.read_to_end(&mut data).unwrap();
        parts.push((entry.name().unwrap().into_owned(), data));
    }
    parts.sort();
    parts
}

fn first_difference(a: &serde_json::Value, b: &serde_json::Value, path: &str) -> Option<String> {
    if a == b {
        return None;
    }
    match (a, b) {
        (serde_json::Value::Object(a), serde_json::Value::Object(b)) => {
            for key in a.keys().chain(b.keys()) {
                let next = format!("{path}/{key}");
                match (a.get(key), b.get(key)) {
                    (Some(a), Some(b)) => {
                        if let Some(diff) = first_difference(a, b, &next) {
                            return Some(diff);
                        }
                    }
                    _ => return Some(format!("{next}: missing key")),
                }
            }
        }
        (serde_json::Value::Array(a), serde_json::Value::Array(b)) if a.len() == b.len() => {
            for (index, (a, b)) in a.iter().zip(b).enumerate() {
                if let Some(diff) = first_difference(a, b, &format!("{path}/{index}")) {
                    return Some(diff);
                }
            }
        }
        _ => (),
    }
    Some(format!("{path}: {a} != {b}"))
}

type SourceParts = BTreeMap<String, Vec<u8>>;

fn source_hash(parts: &SourceParts, name: &str) -> String {
    sha256(&parts[name])
}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

// These generated historical exports omitted an embedded image. Verify their
// recorded provenance and the broken edge, then require the lazy reader's
// specific missing-part error instead of treating invalid packages as valid.
fn historical_export_missing_image(path: &Path, package: &[u8], source: &SourceParts) -> bool {
    const CLASSES: [&str; 5] = [
        "168fb290c4b8",
        "47ebd1cad36f",
        "7fd0288662ac",
        "c60b36940803",
        "d1bbfefa5a7f",
    ];
    let parent = path.parent().unwrap();
    if path.file_name().unwrap() != "fill_rectangle.xlsx"
        || !parent
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| CLASSES.contains(&name))
    {
        return false;
    }
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(parent.join("provenance.json")).unwrap()).unwrap();
    let case = manifest["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["export"] == "fill_rectangle.xlsx")
        .unwrap();
    assert_eq!(
        case["output_sha256"]["export"].as_str(),
        Some(sha256(package).as_str())
    );
    assert!(!source.contains_key("xl/media/pixel.png"));
    let mut reader = Reader::from_reader(source["xl/drawings/_rels/drawing1.xml.rels"].as_slice());
    let mut missing_image = false;
    loop {
        match reader.read_event().unwrap() {
            Event::Start(element) | Event::Empty(element)
                if element.local_name().as_ref() == b"Relationship" =>
            {
                use crate::reader::driver::get_attribute;
                missing_image |= get_attribute(&element, b"Id").as_deref() == Some("rId2")
                    && get_attribute(&element, b"Type").as_deref()
                        == Some(crate::helper::const_str::IMAGE_NS)
                    && get_attribute(&element, b"Target").as_deref() == Some("../media/pixel.png")
                    && get_attribute(&element, b"TargetMode").as_deref() != Some("External");
            }
            Event::Eof => break,
            _ => (),
        }
    }
    assert!(
        missing_image,
        "{}: missing image relationship",
        path.display()
    );
    let drawing = crate::xml::root(&source["xl/drawings/drawing1.xml"]).unwrap();
    let used = drawing.children.iter().any(|token| {
        let crate::drawing::charts::OpaqueToken::Start(element) = token else {
            return false;
        };
        element.attributes.iter().any(|(name, value)| {
            name.is(crate::helper::const_str::REL_OFC_NS, "embed") && value == "rId2"
        })
    });
    assert!(used, "{}: image relationship must be used", path.display());
    true
}

fn assert_missing_part(result: Result<Imported, XlsxError>, what: &str) {
    let error = match result {
        Err(error) => error,
        Ok(_) => panic!("{what}: missing image must fail lazy import"),
    };
    assert_eq!(error.kind(), XlsxErrorKind::Zip, "{what}: {error}");
    assert!(
        matches!(
            std::error::Error::source(&error)
                .and_then(|source| source.downcast_ref::<zip::result::ZipError>()),
            Some(zip::result::ZipError::FileNotFound)
        ),
        "{what}: {error}"
    );
}

// The existing missing-attribute integration regression requires rejection of
// this original package: its externalLinkPath has neither Id nor Target.
fn malformed_relationship_fixture(path: &Path, package: &[u8]) -> bool {
    if path != Path::new("tests/test_files/64450.xlsx") {
        return false;
    }
    assert_eq!(
        sha256(package),
        "203eaf3275f3d4edaf000728defcece4bc44306852f27b59c9371e52d2338dec"
    );
    true
}

fn assert_missing_relationship_attribute(result: Result<Imported, XlsxError>, what: &str) {
    let error = result
        .err()
        .unwrap_or_else(|| panic!("{what}: invalid relationship must fail import"));
    assert_eq!(error.kind(), XlsxErrorKind::Io, "{what}: {error}");
    let source = std::error::Error::source(&error)
        .and_then(|source| source.downcast_ref::<std::io::Error>())
        .unwrap();
    assert_eq!(source.kind(), std::io::ErrorKind::InvalidData, "{what}");
    assert_eq!(
        source.to_string(),
        "missing relationship attribute",
        "{what}"
    );
}

// Source provenance must describe the actual input, whose bytes deliberately
// differ here. Verify it independently, then change only those fields in the
// expected state. All semantic fields and opaque extension content stay exact.
fn expect_split_provenance(value: &mut serde_json::Value, source: &[SourceParts; 2]) {
    match value {
        serde_json::Value::Object(object) => {
            if let Some(hash) = object.get("source_part_sha256") {
                let part = object["chart_part"].as_str().unwrap();
                assert_eq!(hash.as_str(), Some(source_hash(&source[0], part).as_str()));
                object.insert(
                    "source_part_sha256".into(),
                    source_hash(&source[1], part).into(),
                );
            }
            if let (Some(target), Some(data)) = (object.get("file_target"), object.get("file_data"))
            {
                let part = target["value"].as_str().unwrap();
                if !data.as_array().unwrap().is_empty() {
                    assert_eq!(
                        *data,
                        serde_json::to_value(&source[0][part]).unwrap(),
                        "retained source {part}"
                    );
                    object.insert(
                        "file_data".into(),
                        serde_json::to_value(&source[1][part]).unwrap(),
                    );
                }
            }
            for (key, child) in object {
                if key != "file_data" {
                    expect_split_provenance(child, source);
                }
            }
        }
        serde_json::Value::Array(array) => {
            for child in array {
                expect_split_provenance(child, source);
            }
        }
        _ => (),
    }
}

fn expected_component<T: Serialize + DeserializeOwned>(
    original: &T,
    source: &[SourceParts; 2],
) -> T {
    let mut json = serde_json::to_value(original).unwrap();
    let unchanged: T = serde_json::from_value(json.clone()).unwrap();
    assert_eq!(
        bincode::serialize(original).unwrap(),
        bincode::serialize(&unchanged).unwrap(),
        "constructing expected state must preserve every binary field"
    );
    expect_split_provenance(&mut json, source);
    serde_json::from_value(json).unwrap()
}

// HashMap iteration is randomized even for two imports of the same package.
// Adopt only the actual map's layout: every expected key must exist, and every
// value still comes exclusively from the expected state.
fn align_map_order<K: Clone + Eq + Hash, V: Clone>(
    expected: &mut HashMap<K, V>,
    actual: &HashMap<K, V>,
) {
    let mut ordered = actual.clone();
    for (key, value) in &mut ordered {
        *value = expected.remove(key).expect("unexpected map key");
    }
    assert!(expected.is_empty(), "missing map key");
    *expected = ordered;
}

fn assert_same(original: &Imported, split: &Imported, what: &str, source: &[SourceParts; 2]) {
    let mut expected_projection = serde_json::to_value(&original.projection).unwrap();
    expect_split_provenance(&mut expected_projection, source);
    let split_projection = serde_json::to_value(&split.projection).unwrap();
    if let Some(diff) = first_difference(&expected_projection, &split_projection, "$") {
        panic!("{what}: projection first differing JSON path: {diff}");
    }
    for (kind, a, b) in [
        ("full writer", &original.full, &split.full),
        ("light writer", &original.light, &split.light),
    ] {
        let (a, b) = (parts(a), parts(b));
        assert_eq!(
            a.iter().map(|(name, _)| name).collect::<Vec<_>>(),
            b.iter().map(|(name, _)| name).collect::<Vec<_>>(),
            "{what}: {kind} part list"
        );
        for ((name, a), (_, b)) in a.iter().zip(&b) {
            let at = a
                .iter()
                .zip(b)
                .position(|(x, y)| x != y)
                .unwrap_or(a.len().min(b.len()));
            let around = |x: &[u8]| {
                String::from_utf8_lossy(&x[at.saturating_sub(120)..(at + 120).min(x.len())])
                    .into_owned()
            };
            assert!(
                a == b,
                "{what}: {kind} part {name} differs at byte {at}:
{}
{}",
                around(a),
                around(b)
            );
        }
    }
    // Work inside the reader's unit module to adjust the private raw sidecar
    // without adding production test hooks or stripping any binary state.
    let mut expected_sheets = original.sheets.clone();
    assert_eq!(expected_sheets.len(), split.sheets.len());
    for (sheet, actual) in expected_sheets.iter_mut().zip(&split.sheets) {
        align_map_order(
            sheet.collection_to_hashmap_mut(),
            actual.collection_to_hashmap(),
        );
        align_map_order(
            sheet.row_dimensions_to_hashmap_mut(),
            actual.row_dimensions_to_hashmap(),
        );
        sheet.set_worksheet_drawing(expected_component(sheet.worksheet_drawing(), source));
        let parts = sheet
            .drawing_parts()
            .map(|parts| Box::new(expected_component(parts, source)));
        sheet.set_drawing_parts(parts);
    }
    let expected_binary = bincode::serialize(&expected_sheets).unwrap();
    if expected_binary != split.binary {
        let expected = format!("{expected_sheets:?}");
        let actual = format!("{:?}", split.sheets);
        let at = expected
            .bytes()
            .zip(actual.bytes())
            .position(|(a, b)| a != b)
            .unwrap_or(expected.len().min(actual.len()));
        let around = |value: &str| {
            String::from_utf8_lossy(
                &value.as_bytes()[at.saturating_sub(120)..(at + 180).min(value.len())],
            )
            .into_owned()
        };
        panic!(
            "{what}: sheet state first differing debug byte {at}:\n{}\n{}",
            around(&expected),
            around(&actual)
        );
    }
}

fn fixtures(dir: &Path, found: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            fixtures(&path, found);
        } else if path.extension().is_some_and(|ext| ext == "xlsx") {
            found.push(path);
        }
    }
}

#[test]
fn split_elements_import_like_self_closing_elements_in_every_chart_and_drawing_fixture() {
    let mut found = Vec::new();
    for root in ["tests/fixtures", "tests/test_files"] {
        fixtures(Path::new(root), &mut found);
    }
    found.sort();
    let mut checked = 0;
    let mut charts_checked = 0;
    let mut rejected_lazy = 0;
    let mut rejected_relationships = 0;
    for path in found {
        let package = std::fs::read(&path).unwrap();
        if !has_chart_or_drawing_part(&package) {
            continue;
        }
        eprintln!("checking {}", path.display());
        let (split, count) = split_chart_parts(&package);
        assert!(count > 0, "{}: nothing was split", path.display());
        let what = path.display().to_string();
        let label = checked.to_string();
        let source: [SourceParts; 2] = [
            parts(&package).into_iter().collect(),
            parts(&split).into_iter().collect(),
        ];
        charts_checked += usize::from(source[0].keys().any(|name| is_chart_part(name)));
        if malformed_relationship_fixture(&path, &package) {
            for (form, bytes) in [("original", &package), ("paired", &split)] {
                assert_missing_relationship_attribute(
                    eager(bytes),
                    &format!("{what} {form} eager"),
                );
                assert_missing_relationship_attribute(
                    lazy(bytes, &label),
                    &format!("{what} {form} lazy"),
                );
            }
            rejected_relationships += 1;
        } else {
            if checked == 0 {
                let unchanged_source = [
                    parts(&package).into_iter().collect(),
                    parts(&package).into_iter().collect(),
                ];
                assert_same(
                    &eager(&package).unwrap(),
                    &eager(&package).unwrap(),
                    &format!("{what} identical input control"),
                    &unchanged_source,
                );
            }
            assert_same(
                &eager(&package).unwrap(),
                &eager(&split).unwrap(),
                &format!("{what} eager"),
                &source,
            );
            if historical_export_missing_image(&path, &package, &source[0]) {
                assert_missing_part(lazy(&package, &label), &format!("{what} original lazy"));
                assert_missing_part(lazy(&split, &label), &format!("{what} paired lazy"));
                rejected_lazy += 1;
            } else {
                assert_same(
                    &lazy(&package, &label).unwrap(),
                    &lazy(&split, &label).unwrap(),
                    &format!("{what} lazy"),
                    &source,
                );
            }
        }
        checked += 1;
    }
    assert!(
        charts_checked >= 20,
        "only {charts_checked} chart fixtures were checked"
    );
    assert_eq!(rejected_lazy, 5, "historical missing-media cases");
    assert_eq!(rejected_relationships, 1, "malformed relationship input");
    eprintln!("checked {checked} chart/drawing fixtures, eager/lazy and full/light writers");
}
