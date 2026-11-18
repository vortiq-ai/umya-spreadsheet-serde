//! Excel's measured chart-type conversions, as test data: the before parts it
//! was given, what Excel reported about each, the extractor that reads a part
//! the way the measurement did, and umya's own read and write of every part.
use std::{
    collections::{
        BTreeMap,
        BTreeSet,
    },
    fs,
    path::Path,
};

use sha2::{
    Digest,
    Sha256,
};

#[path = "chart_conversion/classification.rs"]
mod classification;
#[path = "chart_conversion/conversion.rs"]
mod conversion;
#[path = "support/chart_xml_fixture.rs"]
mod fixture;
#[path = "chart_conversion/measured.rs"]
mod measured;
#[path = "chart_conversion/projection.rs"]
mod projection;
#[path = "chart_conversion/refusals.rs"]
mod refusals;
#[path = "chart_conversion/workbook.rs"]
mod workbook;
#[path = "chart_conversion/xml.rs"]
mod xml;

use measured::{
    Part,
    fixtures,
    measured,
    read_part,
};
use projection::{
    diff_paths,
    paths_of,
    slots_of,
};
use workbook::workbook_with;

/// 763 native parts, 10 combination layouts, 10 combination probes and 3
/// scatter parts the driver flagged invalid.
const PARTS: usize = 786;

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .to_vec()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// The bytes with every line feed turned into a carriage return and a line
/// feed, as a Windows text-mode write does.
fn with_text_mode_line_ends(bytes: &[u8]) -> Vec<u8> {
    let mut translated = Vec::with_capacity(bytes.len() + bytes.len() / 50);
    for &byte in bytes {
        if byte == b'\n' {
            translated.push(b'\r');
        }
        translated.push(byte);
    }
    translated
}

/// The paths of the files under `directory`, relative to `root`.
fn files_under(directory: &Path, root: &Path, found: &mut BTreeSet<String>) {
    for entry in fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            files_under(&path, root, found);
        } else {
            let relative = path.strip_prefix(root).unwrap();
            found.insert(relative.to_str().unwrap().replace('\\', "/"));
        }
    }
}

/// How the slots and paths of `xml` differ from the measured ones of `part`,
/// and how many of the measured paths it carries unchanged.
fn compare(part: &Part, xml: &str) -> (Vec<String>, usize) {
    let mut found = Vec::new();
    let slots = slots_of(xml);
    for (key, expected) in &part.slots {
        match slots.get(key) {
            Some(slot) if slot == expected => {}
            Some(slot) => found.push(format!("slot {key}: measured {expected:?}, found {slot:?}")),
            None => found.push(format!("slot {key}: measured {expected:?}, found none")),
        }
    }
    for key in slots.keys().filter(|key| !part.slots.contains_key(*key)) {
        found.push(format!("slot {key}: found, never measured"));
    }
    let paths = diff_paths(&part.paths, &paths_of(xml));
    found.extend(
        paths
            .dropped
            .iter()
            .map(|path| format!("path {path}: dropped")),
    );
    found.extend(paths.added.iter().map(|path| format!("path {path}: added")));
    found.extend(
        paths
            .changed
            .iter()
            .map(|(path, (was, now))| format!("path {path}: measured {was:?}, found {now:?}")),
    );
    (found, paths.carried)
}

/// `ser[2]/dPt[4]` as `ser[]/dPt[]`, so a difference that every series or
/// point shows reads as one line.
fn without_indices(text: &str) -> String {
    let mut plain = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(open) = rest.find('[') {
        let (head, tail) = rest.split_at(open + 1);
        plain.push_str(head);
        let digits = tail.bytes().take_while(u8::is_ascii_digit).count();
        rest = if tail[digits..].starts_with(']') {
            &tail[digits..]
        } else {
            tail
        };
    }
    plain.push_str(rest);
    plain
}

/// Fails with the distinct differences, the most common first, each with the
/// number of parts that show it and the first of them.
fn assert_no_differences(what: &str, mut read: impl FnMut(&Part) -> String) {
    let mut differing = 0;
    let (mut carried, mut measured_paths) = (0, 0);
    let mut seen: BTreeMap<String, Vec<&str>> = BTreeMap::new();
    for part in &measured().parts {
        let (found, carries) = compare(part, &read(part));
        differing += usize::from(!found.is_empty());
        carried += carries;
        measured_paths += part.paths.len();
        let distinct: BTreeSet<String> = found
            .iter()
            .map(|difference| without_indices(difference))
            .collect();
        for difference in distinct {
            seen.entry(difference).or_default().push(&part.fixture);
        }
    }
    let mut ranked: Vec<_> = seen.iter().collect();
    ranked.sort_by_key(|(_, parts)| std::cmp::Reverse(parts.len()));
    let report: Vec<String> = ranked
        .iter()
        .take(60)
        .map(|(difference, parts)| {
            format!("{:>4} parts, first {}: {difference}", parts.len(), parts[0])
        })
        .collect();
    assert!(
        differing == 0,
        "{what}: {differing} of {PARTS} parts differ from the measurement, {carried} of \
         {measured_paths} measured paths carried, {} distinct differences:\n{}",
        seen.len(),
        report.join("\n")
    );
}

#[test]
fn before_parts_match_their_recorded_hashes() {
    let measurement = measured();
    assert_eq!(measurement.parts.len(), PARTS);

    let mut on_disk = BTreeSet::new();
    files_under(&fixtures().join("before"), &fixtures(), &mut on_disk);
    let recorded: BTreeSet<String> = measurement
        .parts
        .iter()
        .map(|part| part.fixture.clone())
        .collect();
    let unrecorded: Vec<_> = on_disk.difference(&recorded).collect();
    assert!(
        unrecorded.is_empty(),
        "checked-in parts the measurement does not record: {unrecorded:?}"
    );
    let missing: Vec<_> = recorded.difference(&on_disk).collect();
    assert!(
        missing.is_empty(),
        "recorded parts that are not checked in: {missing:?}"
    );

    for part in &measurement.parts {
        let bytes = fs::read(fixtures().join(&part.fixture)).unwrap();
        assert_eq!(sha256_hex(&bytes), part.sha256, "{}", part.fixture);
        let left_on_disk = if part.translated {
            with_text_mode_line_ends(&bytes)
        } else {
            bytes
        };
        assert_eq!(
            sha256_hex(&left_on_disk),
            part.sidecar_sha256,
            "{} as the measurement left it on disk",
            part.fixture
        );
    }
}

#[test]
fn extractor_matches_the_measurement() {
    assert_no_differences("extracting the part", read_part);
}

#[test]
fn umya_round_trip_keeps_every_before_part() {
    assert_no_differences("reading and writing the part", |part| {
        fixture::chart_xml(&workbook_with(&read_part(part)))
    });
}
