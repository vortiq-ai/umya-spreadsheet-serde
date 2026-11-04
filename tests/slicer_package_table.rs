#[path = "support/slicer_package_fixture.rs"]
mod fixture;

use fixture::*;
use umya_spreadsheet::{
    SlicerSource,
    Workbook,
    XlsxError,
};

fn assert_model(book: &Workbook, label: &str) {
    let caches = book.slicer_caches();
    assert_eq!(caches.len(), 1, "{label}");
    assert_eq!(caches[0].name, "Slicer_Team", "{label}");
    let Some(SlicerSource::Table(table)) = &caches[0].source else {
        panic!("{label}: table source expected");
    };
    assert_eq!(table.table_id, 5, "{label}");
    let slicers = book.sheet_by_name("Data").unwrap().slicers();
    assert_eq!(slicers.len(), 1, "{label}");
    assert_eq!(slicers[0].cache, "Slicer_Team", "{label}");
    assert_eq!(slicers[0].column_count, Some(3), "{label}");
    assert_eq!(slicers[0].row_height, 241_300, "{label}");
}

fn assert_package(parts: &Parts, label: &str) {
    let workbook = &parts["xl/workbook.xml"];
    assert_eq!(
        ext_uris(workbook),
        vec!["{46BE6895-7355-4a93-B00E-2C351335B9C9}"],
        "{label}"
    );
    assert!(!workbook.contains("{BBE1A952"), "{label}");
    let entries = tags(workbook, "<x14:slicerCache");
    assert_eq!(entries.len(), 1, "{label}");
    let (kind, cache) = resolve(
        parts,
        "xl/workbook.xml",
        &attribute(entries[0], "r:id").unwrap(),
    );
    assert_eq!(kind, SLICER_CACHE_REL, "{label}");
    assert_eq!(
        content_type(parts, &cache).as_deref(),
        Some(SLICER_CACHE_TYPE),
        "{label}"
    );
    assert!(parts[&cache].contains("tableId=\"5\""), "{label}");
    assert!(
        workbook.contains("<definedName name=\"Slicer_Team\">#N/A</definedName>"),
        "{label}: {workbook}"
    );

    let sheet_part = sheet_part_of(parts, "Data");
    let sheet = &parts[&sheet_part];
    assert_eq!(
        ext_uris(sheet),
        vec!["{3A4CF648-6AED-40f4-86FF-DC5316D8AED3}"],
        "{label}"
    );
    let entries = tags(sheet, "<x14:slicer");
    assert_eq!(entries.len(), 1, "{label}");
    let (kind, slicer) = resolve(parts, &sheet_part, &attribute(entries[0], "r:id").unwrap());
    assert_eq!(kind, SLICER_REL, "{label}");
    assert_eq!(
        content_type(parts, &slicer).as_deref(),
        Some(SLICER_TYPE),
        "{label}"
    );
    assert!(parts[&slicer].contains("columnCount=\"3\""), "{label}");
    assert!(parts[&slicer].contains("rowHeight=\"241300\""), "{label}");
    let table_id = attribute(tags(sheet, "<tablePart")[0], "r:id").unwrap();
    let (_, table) = resolve(parts, &sheet_part, &table_id);
    assert!(parts[&table].contains("id=\"5\""), "{label}");
}

#[test]
fn table_slicers_round_trip_through_every_reader() {
    let input = pack(table_package());
    keep_output("table-authored", &input);
    let readers: Vec<(&str, fn(&[u8]) -> Result<Workbook, XlsxError>)> = vec![
        ("eager", read_eager),
        ("deferred", read_deferred),
        ("lazy-file", |bytes| read_lazy_file("table", bytes)),
    ];
    for (label, read) in &readers {
        let mut book = read(&input).unwrap();
        book.try_read_sheet_collection().unwrap();
        assert_model(&book, label);
        book.sheet_by_name_mut("Data")
            .unwrap()
            .cell_mut("D1")
            .set_value("edited");
        let bytes = save(&book);
        keep_output(&format!("table-{label}"), &bytes);
        assert_package(&unpack(&bytes), label);
        let mut reread = read_eager(&bytes).unwrap();
        reread.try_read_sheet_collection().unwrap();
        assert_model(&reread, label);
    }
}

#[test]
fn table_slicer_for_unknown_table_is_an_error() {
    let mut parts = table_package();
    let xml = parts["xl/slicerCaches/slicerCache9.xml"].replace("tableId=\"5\"", "tableId=\"6\"");
    parts.insert("xl/slicerCaches/slicerCache9.xml".into(), xml);
    let message = match read_eager(&pack(parts)) {
        Err(error) => error.to_string(),
        Ok(book) => try_save(&book).expect_err("unknown table").to_string(),
    };
    assert!(message.contains("Slicer_Team"), "{message}");
}

#[test]
fn table_slicer_extension_relationship_must_resolve() {
    let mut parts = table_package();
    let rels = parts["xl/_rels/workbook.xml.rels"].replace("Id=\"tableCache\"", "Id=\"lost\"");
    parts.insert("xl/_rels/workbook.xml.rels".into(), rels);
    let error = read_eager(&pack(parts))
        .err()
        .expect("missing relationship");
    assert!(error.to_string().contains("tableCache"), "{error}");
}
