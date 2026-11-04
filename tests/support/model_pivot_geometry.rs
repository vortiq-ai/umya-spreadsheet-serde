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

use crate::{
    Location,
    PivotTable,
    Workbook,
    reader,
    writer,
};

fn read_fixture(name: &str, eager: bool) -> Workbook {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/model_operations")
        .join(format!("{name}.xlsx"));
    let mut book =
        reader::xlsx::read_reader(Cursor::new(std::fs::read(path).unwrap()), eager).unwrap();
    book.sheet_by_name_mut("Pivot").unwrap();
    book
}

fn pivot(book: &Workbook) -> &PivotTable {
    &book.sheet_by_name("Pivot").unwrap().pivot_tables()[0]
}

fn geometry(location: &Location) -> (String, u32, u32, u32, Option<u32>, Option<u32>) {
    (
        location.reference().into(),
        location.first_header_row(),
        location.first_data_row(),
        location.first_data_col(),
        location.row_page_count(),
        location.column_page_count(),
    )
}

fn location_attributes(bytes: &[u8]) -> Option<BTreeMap<String, String>> {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    for index in 0..zip.len() {
        let mut part = zip.by_index(index).unwrap();
        let name = part.name().unwrap();
        if !name.starts_with("xl/pivotTables/pivotTable") || !name.ends_with(".xml") {
            continue;
        }
        let mut xml = String::new();
        part.read_to_string(&mut xml).unwrap();
        let mut reader = quick_xml::Reader::from_str(&xml);
        loop {
            match reader.read_event().unwrap() {
                quick_xml::events::Event::Empty(element)
                    if element.local_name().as_ref() == b"location" =>
                {
                    return Some(
                        element
                            .attributes()
                            .map(|attribute| {
                                let attribute = attribute.unwrap();
                                (
                                    std::str::from_utf8(attribute.key.as_ref()).unwrap().into(),
                                    attribute
                                        .decoded_and_normalized_value(
                                            quick_xml::XmlVersion::Implicit1_0,
                                            reader.decoder(),
                                        )
                                        .unwrap()
                                        .into_owned(),
                                )
                            })
                            .collect(),
                    );
                }
                quick_xml::events::Event::Eof => break,
                _ => (),
            }
        }
    }
    None
}

#[derive(Clone, Copy)]
struct Edit {
    rows:   bool,
    insert: bool,
    start:  u32,
    count:  u32,
}

impl Edit {
    fn apply(self, book: &mut Workbook, worksheet: bool) {
        if worksheet {
            let sheet = book.sheet_by_name_mut("Pivot").unwrap();
            match (self.rows, self.insert) {
                (true, true) => sheet.insert_new_row(self.start, self.count),
                (true, false) => sheet.remove_row(self.start, self.count),
                (false, true) => sheet.insert_new_column_by_index(self.start, self.count),
                (false, false) => sheet.remove_column_by_index(self.start, self.count),
            }
        } else {
            match (self.rows, self.insert) {
                (true, true) => book.insert_new_row("Pivot", self.start, self.count),
                (true, false) => book.remove_row("Pivot", self.start, self.count),
                (false, true) => book.insert_new_column_by_index("Pivot", self.start, self.count),
                (false, false) => book.remove_column_by_index("Pivot", self.start, self.count),
            }
        }
    }
}

#[test]
fn pivot_geometry_matches_excel_through_public_edits_and_both_writers() {
    let answers: serde_json::Value = serde_json::from_str(include_str!(
        "../fixtures/model_operations/pivot-known-answers.json"
    ))
    .unwrap();
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/model-operations/pivot");
    std::fs::create_dir_all(&directory).unwrap();
    for case in answers["cases"].as_array().unwrap() {
        let label = case["case"].as_str().unwrap();
        let edit = Edit {
            rows:   case["axis"] == "row",
            insert: case["operation"] == "insert",
            start:  case["start"].as_u64().unwrap() as u32,
            count:  case["count"].as_u64().unwrap() as u32,
        };
        let expected_count = usize::from(case["removed"] != true);
        for eager in [false, true] {
            for worksheet in [false, true] {
                let mut book = read_fixture(case["base"].as_str().unwrap(), eager);
                let before = format!("{book:?}");
                edit.apply(&mut book, worksheet);
                if worksheet && case["removed"] == true {
                    assert_eq!(
                        format!("{book:?}"),
                        before,
                        "{label}: worksheet model removal is refused"
                    );
                    for light in [false, true] {
                        let mut bytes = Vec::new();
                        if light {
                            writer::xlsx::write_writer_light(&book, &mut bytes).unwrap();
                        } else {
                            writer::xlsx::write_writer(&book, &mut bytes).unwrap();
                        }
                        let reread = reader::xlsx::read_reader(Cursor::new(&bytes), true).unwrap();
                        let output = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                            .join("target/model-operations/worksheet-refused");
                        std::fs::create_dir_all(&output).unwrap();
                        std::fs::write(output.join(format!("{label}-{eager}-{light}.xlsx")), bytes)
                            .unwrap();
                        assert_eq!(
                            reread.sheet_by_name("Pivot").unwrap().pivot_tables().len(),
                            1,
                            "{label}"
                        );
                    }
                    continue;
                }
                if case["refused"] == true {
                    assert_eq!(format!("{book:?}"), before, "{label}");
                }
                assert_eq!(
                    book.sheet_by_name("Pivot").unwrap().pivot_tables().len(),
                    expected_count,
                    "{label}"
                );
                if expected_count != 0 {
                    assert_eq!(
                        serde_json::to_value(geometry(
                            pivot(&book).pivot_table_definition().location()
                        ))
                        .unwrap(),
                        case["state"],
                        "{label}"
                    );
                }
                for light in [false, true] {
                    let mut bytes = Vec::new();
                    if light {
                        writer::xlsx::write_writer_light(&book, &mut bytes).unwrap();
                    } else {
                        writer::xlsx::write_writer(&book, &mut bytes).unwrap();
                    }
                    assert_eq!(
                        serde_json::to_value(location_attributes(&bytes)).unwrap(),
                        case["attributes"],
                        "{label}"
                    );
                    let reread = reader::xlsx::read_reader(Cursor::new(&bytes), true).unwrap();
                    assert_eq!(
                        reread.sheet_by_name("Pivot").unwrap().pivot_tables().len(),
                        expected_count,
                        "{label}"
                    );
                    if expected_count != 0 {
                        assert_eq!(
                            serde_json::to_value(geometry(
                                pivot(&reread).pivot_table_definition().location()
                            ))
                            .unwrap(),
                            case["state"],
                            "{label}"
                        );
                    }
                    std::fs::write(
                        directory.join(format!("{label}-{eager}-{worksheet}-{light}.xlsx")),
                        bytes,
                    )
                    .unwrap();
                }
            }
        }
    }
}

#[test]
fn pivot_geometry_preflight_preserves_refused_and_removed_models() {
    let answers: serde_json::Value = serde_json::from_str(include_str!(
        "../fixtures/model_operations/pivot-known-answers.json"
    ))
    .unwrap();
    for case in answers["cases"].as_array().unwrap() {
        let label = case["case"].as_str().unwrap();
        let book = read_fixture(case["base"].as_str().unwrap(), true);
        let mut prepared = pivot(&book).clone();
        let before = prepared.pivot_table_definition().to_xml().unwrap();
        let start = case["start"].as_u64().unwrap() as u32;
        let count = case["count"].as_u64().unwrap() as u32;
        let (col, cols, row, rows) = if case["axis"] == "row" {
            (0, 0, start, count)
        } else {
            (start, count, 0, 0)
        };
        let definition = prepared.pivot_table_definition_mut().location_mut();
        let result = if case["operation"] == "insert" {
            definition
                .insert_geometry(col, cols, row, rows)
                .map(|()| false)
        } else {
            definition.remove_geometry(col, cols, row, rows)
        };
        assert_eq!(result.is_err(), case["refused"] == true, "{label}");
        if let Ok(removed) = result {
            assert_eq!(removed, case["removed"] == true, "{label}");
            if !removed {
                assert_eq!(
                    serde_json::to_value(geometry(prepared.pivot_table_definition().location()))
                        .unwrap(),
                    case["state"],
                    "{label}"
                );
                continue;
            }
        }
        assert_eq!(
            prepared.pivot_table_definition().to_xml().unwrap(),
            before,
            "{label}"
        );
    }
}

#[test]
fn pivot_geometry_rejects_invalid_locations_and_out_of_grid_edits_atomically() {
    let book = read_fixture("geometry-pivot", true);
    for (reference, col, cols, row, rows) in [
        ("C8:D11", 0, 1, 0, 0),
        ("C8:D11", 0, 0, 0, 1),
        ("C8:D11", 1, 16_384, 0, 0),
        ("C8:D11", 0, 0, 1, 1_048_576),
        ("A1:A1:A1", 1, 1, 0, 0),
        ("C0:D11", 1, 1, 0, 0),
        ("D8:C11", 1, 1, 0, 0),
        ("C1:D11", 1, 1, 0, 0),
    ] {
        let mut prepared = pivot(&book).clone();
        prepared
            .pivot_table_definition_mut()
            .location_mut()
            .set_reference(reference);
        let before = format!("{prepared:?}");
        assert!(
            prepared
                .pivot_table_definition_mut()
                .location_mut()
                .insert_geometry(col, cols, row, rows)
                .is_err()
        );
        assert_eq!(format!("{prepared:?}"), before);
    }
}

#[test]
fn incomplete_pivot_geometry_does_not_block_distant_edits() {
    for empty in [false, true] {
        let mut book = read_fixture("geometry-pivot", true);
        let sheet = book.sheet_by_name_mut("Pivot").unwrap();
        let location = sheet.pivot_tables_mut()[0]
            .pivot_table_definition_mut()
            .location_mut();
        if empty {
            *location = Location::default();
        } else {
            location.set_row_page_count(2);
            location.set_column_page_count(0);
        }
        let before = geometry(location);
        sheet.cell_mut("Z500").set_value("sentinel");
        sheet.insert_new_row(500, 1);
        assert_eq!(sheet.cell("Z501").unwrap().value(), "sentinel");
        sheet.remove_row(500, 1);
        assert_eq!(sheet.cell("Z500").unwrap().value(), "sentinel");
        assert_eq!(
            geometry(sheet.pivot_tables()[0].pivot_table_definition().location()),
            before
        );
    }
}

#[test]
fn incomplete_page_counts_do_not_block_columns_after_the_known_footprint() {
    let mut book = read_fixture("geometry-pivot", true);
    let sheet = book.sheet_by_name_mut("Pivot").unwrap();
    let location = sheet.pivot_tables_mut()[0]
        .pivot_table_definition_mut()
        .location_mut();
    *location = Location::default();
    location.set_reference("C8:D11").set_column_page_count(1);
    let before = geometry(location);
    sheet.cell_mut("E20").set_value("sentinel");
    sheet.insert_new_column_by_index(5, 1);
    assert_eq!(sheet.cell("F20").unwrap().value(), "sentinel");
    sheet.remove_column_by_index(5, 1);
    assert_eq!(sheet.cell("E20").unwrap().value(), "sentinel");
    assert_eq!(
        geometry(sheet.pivot_tables()[0].pivot_table_definition().location()),
        before
    );
}
