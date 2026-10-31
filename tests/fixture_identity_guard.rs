//! Excel-authored fixtures are committed to a shared repository, so none may
//! carry the author's identity or machine paths. Fixtures are produced by
//! `chart_ex_excel_author.ps1`, which makes Excel remove the document
//! properties and the workbook path before saving; this guard fails when a
//! fixture under any `excel-authored` directory still has an author name, a
//! last-modified-by name, an `absPath`, or a drive or user path in any part.
use std::{
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

fn excel_authored_fixtures(directory: &Path, found: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            excel_authored_fixtures(&path, found);
        } else if path.extension().is_some_and(|ext| ext == "xlsx")
            && path.parent().is_some_and(|parent| {
                parent
                    .file_name()
                    .is_some_and(|name| name == "excel-authored")
            })
        {
            found.push(path);
        }
    }
}

fn non_empty_element(xml: &str, tag: &str) -> bool {
    let open = format!("<{tag}>");
    xml.match_indices(&open).any(|(start, _)| {
        let rest = &xml[start + open.len()..];
        !rest.starts_with(&format!("</{tag}>"))
    })
}

/// Every identity finding in one package, as `part: what`.
fn identity_findings(package: &[u8]) -> Vec<String> {
    let mut archive = zip::ZipArchive::new(Cursor::new(package)).unwrap();
    let mut findings = Vec::new();
    for index in 0..archive.len() {
        let mut file = archive.by_index(index).unwrap();
        let name = file.name().unwrap().into_owned();
        let mut data = Vec::new();
        file.read_to_end(&mut data).unwrap();
        let lower = String::from_utf8_lossy(&data).to_ascii_lowercase();
        if name == "docProps/core.xml" {
            for tag in ["dc:creator", "cp:lastmodifiedby"] {
                if non_empty_element(&lower, tag) {
                    findings.push(format!("{name}: non-empty {tag}"));
                }
            }
        }
        if lower.contains("abspath") {
            findings.push(format!("{name}: absPath"));
        }
        if lower.contains("\\users\\") || lower.contains("/users/") || lower.contains("/home/") {
            findings.push(format!("{name}: user path"));
        }
        let bytes = lower.as_bytes();
        if bytes
            .windows(3)
            .any(|w| w[0].is_ascii_lowercase() && w[1] == b':' && w[2] == b'\\')
        {
            findings.push(format!("{name}: drive path"));
        }
    }
    findings
}

#[test]
fn excel_authored_fixtures_carry_no_identity_or_machine_paths() {
    let mut fixtures = Vec::new();
    excel_authored_fixtures(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures"),
        &mut fixtures,
    );
    assert!(
        fixtures.len() >= 8,
        "expected the Excel-authored ChartEx fixtures, found {fixtures:?}"
    );
    for path in fixtures {
        let findings = identity_findings(&std::fs::read(&path).unwrap());
        assert!(findings.is_empty(), "{}: {findings:?}", path.display());
    }
}

#[test]
fn the_identity_scan_catches_names_and_paths() {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default();
    for (name, body) in [
        (
            "docProps/core.xml",
            "<cp:coreProperties><dc:creator>Some Person</dc:creator><cp:lastModifiedBy>Some \
             Person</cp:lastModifiedBy></cp:coreProperties>",
        ),
        (
            "xl/workbook.xml",
            "<workbook><x15ac:absPath url=\"D:\\Work\\Files\\\"/></workbook>",
        ),
        ("xl/other.xml", "<a>C:\\Users\\someone\\Documents</a>"),
    ] {
        writer.start_file(name, options).unwrap();
        writer.write_all(body.as_bytes()).unwrap();
    }
    let package = writer.finish().unwrap().into_inner();
    let findings = identity_findings(&package);
    for expected in [
        "docProps/core.xml: non-empty dc:creator",
        "docProps/core.xml: non-empty cp:lastmodifiedby",
        "xl/workbook.xml: absPath",
        "xl/workbook.xml: drive path",
        "xl/other.xml: user path",
        "xl/other.xml: drive path",
    ] {
        assert!(
            findings.iter().any(|finding| finding == expected),
            "{expected} missing from {findings:?}"
        );
    }
    let clean = {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        writer.start_file("docProps/core.xml", options).unwrap();
        writer
            .write_all(b"<cp:coreProperties><dc:creator></dc:creator></cp:coreProperties>")
            .unwrap();
        writer.finish().unwrap().into_inner()
    };
    assert!(identity_findings(&clean).is_empty());
}
