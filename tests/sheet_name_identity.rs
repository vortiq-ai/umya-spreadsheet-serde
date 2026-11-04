#![cfg(feature = "serde")]

use std::io::Cursor;

use umya_spreadsheet::{
    Workbook,
    Worksheet,
    XlsxErrorKind,
    structs::DefinedName,
};

// Native Excel rejects these duplicate spellings but permits renaming the same
// sheet between them. This is not a claim about every Unicode case mapping.
const CASE_PAIRS: [(&str, &str, &str); 3] = [
    ("Data", "data", "data!$A$1"),
    ("ÄSheet", "äsheet", "'äsheet'!$A$1"),
    ("ÉSheet", "ésheet", "'ésheet'!$A$1"),
];

fn workbook_bytes(name: &str) -> Vec<u8> {
    let mut book = umya_spreadsheet::new_file();
    book.set_sheet_name(0, "Host").unwrap();
    book.new_sheet(name)
        .unwrap()
        .cell_mut("A1")
        .set_value_number(17);
    book.new_sheet("Other").unwrap();
    book.new_sheet("Unrelated")
        .unwrap()
        .cell_mut("A1")
        .set_value("keep");
    book.sheet_mut(0)
        .unwrap()
        .cell_mut("A1")
        .set_formula(format!("{name}!$A$1"));
    let mut defined = DefinedName::default();
    defined
        .set_name("Source")
        .set_address(format!("{name}!$A$1"))
        .set_hidden(true);
    book.add_defined_names(defined);
    write(&book)
}

fn write(book: &Workbook) -> Vec<u8> {
    let mut bytes = Cursor::new(Vec::new());
    umya_spreadsheet::writer::xlsx::write_writer(book, &mut bytes).unwrap();
    bytes.into_inner()
}

fn defined_names(book: &Workbook) -> Vec<&DefinedName> {
    book.defined_names()
        .iter()
        .chain(
            book.sheet_collection_no_check()
                .iter()
                .flat_map(|sheet| sheet.defined_names()),
        )
        .collect()
}

fn load(bytes: &[u8], eager: bool) -> Workbook {
    umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(bytes), eager).unwrap()
}

fn snapshot(book: &Workbook) -> (Vec<u8>, String) {
    (
        bincode::serialize(book.sheet_collection_no_check()).unwrap(),
        format!("{book:?}"),
    )
}

#[test]
fn case_variant_sheet_names_are_rejected_without_changing_the_workbook() {
    for (name, duplicate, _) in CASE_PAIRS {
        let bytes = workbook_bytes(name);
        for eager in [true, false] {
            for operation in ["new", "add", "rename"] {
                let mut book = load(&bytes, eager);
                let before = snapshot(&book);
                let error = match operation {
                    "new" => book.new_sheet(duplicate).unwrap_err(),
                    "add" => {
                        let mut sheet = Worksheet::default();
                        sheet.set_name(duplicate);
                        book.add_sheet(sheet).unwrap_err()
                    }
                    "rename" => book.set_sheet_name(2, duplicate).unwrap_err(),
                    _ => unreachable!(),
                };
                assert_eq!(
                    error.kind(),
                    XlsxErrorKind::NameDuplicate,
                    "{name}/{duplicate}, {operation}, eager={eager}"
                );
                assert_eq!(
                    snapshot(&book),
                    before,
                    "rejected {operation} must not mutate or materialize sheets"
                );
                if !eager {
                    for index in 0..4 {
                        assert!(
                            book.sheet(index).is_err(),
                            "rejected {operation} materialized sheet {index}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn a_sheet_can_change_only_its_case_and_update_its_references() {
    for (name, renamed, expected_reference) in CASE_PAIRS {
        let bytes = workbook_bytes(name);
        for eager in [true, false] {
            let mut book = load(&bytes, eager);
            book.set_sheet_name(1, renamed).unwrap();
            assert_eq!(book.sheet_collection_no_check()[1].name(), renamed);
            assert_eq!(book.sheet_collection_no_check().len(), 4);
            assert_eq!(
                book.sheet(0).unwrap().cell("A1").unwrap().formula(),
                expected_reference
            );
            let names = defined_names(&book);
            assert_eq!(names.len(), 1);
            assert!(!names[0].has_local_sheet_id());
            assert_eq!(names[0].name(), "Source");
            assert_eq!(names[0].address(), expected_reference);
            assert!(names[0].hidden());
            if !eager {
                assert!(
                    book.sheet(3).is_err(),
                    "unrelated worksheet must remain raw"
                );
            }
            let exported = write(&book);
            let reread = load(&exported, true);
            assert_eq!(reread.sheet_collection()[1].name(), renamed);
            assert_eq!(
                reread.sheet(1).unwrap().cell("A1").unwrap().value_number(),
                Some(17.0)
            );
            assert_eq!(
                reread.sheet(0).unwrap().cell("A1").unwrap().formula(),
                expected_reference
            );
            let names = defined_names(&reread);
            assert_eq!(names.len(), 1);
            assert!(!names[0].has_local_sheet_id());
            assert_eq!(names[0].name(), "Source");
            assert_eq!(names[0].address(), expected_reference);
            assert!(names[0].hidden());
            assert_eq!(reread.sheet(3).unwrap().cell("A1").unwrap().value(), "keep");
        }
    }
}

#[test]
fn deleting_from_a_bypassed_case_duplicate_workbook_reports_name_duplicate_atomically() {
    for (name, duplicate, _) in CASE_PAIRS {
        let bytes = workbook_bytes(name);
        for eager in [true, false] {
            for by_name in [true, false] {
                let mut book = load(&bytes, eager);
                // Direct worksheet mutation can bypass the workbook guard.
                book.try_read_sheet(2).unwrap();
                book.sheet_mut(2).unwrap().set_name(duplicate);
                let before = snapshot(&book);
                let error = if by_name {
                    book.remove_sheet_by_name(name).unwrap_err()
                } else {
                    book.remove_sheet(1).unwrap_err()
                };
                assert_eq!(
                    error.kind(),
                    XlsxErrorKind::NameDuplicate,
                    "{name}/{duplicate}, by_name={by_name}, eager={eager}"
                );
                assert_eq!(
                    snapshot(&book),
                    before,
                    "duplicate-name rejection must precede loading and mutation"
                );
                if !eager {
                    for index in [0, 1, 3] {
                        assert!(
                            book.sheet(index).is_err(),
                            "deletion preflight materialized sheet {index}"
                        );
                    }
                }
            }
        }
    }
}
