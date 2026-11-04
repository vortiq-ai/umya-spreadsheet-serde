use std::{
    io::Cursor,
    path::PathBuf,
};

use serde_json::{
    Value,
    json,
};
use umya_spreadsheet::{
    Workbook,
    Worksheet,
    XlsxErrorKind,
    structs::{
        DefinedName,
        SheetStateValues,
    },
    writer::streaming_writer::StreamingWriter,
};

const INPUT: &[u8] = include_bytes!("fixtures/sheet_operations/visibility.xlsx");

fn known(case: &str) -> Value {
    let text = include_str!("fixtures/sheet_operations/visibility-known-answers.json");
    serde_json::from_str::<Value>(text.trim_start_matches('\u{feff}')).unwrap()[case].clone()
}

fn save(book: &Workbook, streaming: bool) -> Vec<u8> {
    if streaming {
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
    } else {
        let mut output = Cursor::new(Vec::new());
        umya_spreadsheet::writer::xlsx::write_writer(book, &mut output).unwrap();
        output.into_inner()
    }
}

fn assert_saved(book: &Workbook, case: &str, reader: &str) {
    let expected = known(case);
    for streaming in [false, true] {
        let output = save(book, streaming);
        let again =
            umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(&output), true).unwrap();
        let sheets: Vec<_> = again
            .sheet_collection_no_check()
            .iter()
            .map(|sheet| json!({"name": sheet.name(), "id": sheet.sheet_id(), "state": sheet.sheet_state(), "selected": sheet.sheets_views().sheet_view_list().first().is_some_and(|view| view.tab_selected())}))
            .collect();
        assert_eq!(
            json!(sheets),
            expected["sheets"],
            "{case}/{reader}/{streaming}"
        );
        assert_eq!(
            again.workbook_view().active_tab(),
            expected["active_tab"].as_u64().unwrap() as u32
        );
        let mut names: Vec<_> = again
            .defined_names()
            .iter()
            .chain(again.sheet_collection_no_check().iter().flat_map(|sheet| sheet.defined_names()))
            .map(|name| json!({"name": name.name(), "scope": if name.has_local_sheet_id() { name.local_sheet_id().to_string() } else { String::new() }, "value": name.address()}))
            .collect();
        names.sort_by_key(Value::to_string);
        let mut expected_names = expected["names"].as_array().unwrap().clone();
        expected_names.sort_by_key(Value::to_string);
        assert_eq!(names, expected_names, "{case}/{reader}/{streaming}");
        if let Some(formula) = expected["formulas"].as_array().unwrap().first() {
            assert_eq!(
                again.sheet(0).unwrap().cell("A10").unwrap().formula(),
                formula["value"].as_str().unwrap()
            );
        }
        if let Some(directory) = std::env::var_os("SLICER_PACKAGE_OUTPUT_DIR") {
            let directory = PathBuf::from(directory);
            std::fs::create_dir_all(&directory).unwrap();
            std::fs::write(
                directory.join(format!("visibility-{case}-{reader}-{streaming}.xlsx")),
                output,
            )
            .unwrap();
        }
    }
}

#[test]
fn sheet_visibility_activation_deletion_and_names_match_excel() {
    for eager in [true, false] {
        for case in [
            "hide-active",
            "hide-active-at-end",
            "hide-active-next-hidden",
            "very-hide-active",
            "activate-hidden",
            "activate-out-of-range",
            "delete-active",
            "delete-before-active",
            "delete-final",
            "delete-last-visible",
            "delete-referenced-local",
            "delete-referenced-local-global",
        ] {
            let mut book =
                umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(INPUT), eager).unwrap();
            match case {
                "hide-active" => {
                    book.sheet_mut(1)
                        .unwrap()
                        .set_sheet_state("hidden".to_owned());
                }
                "hide-active-at-end" => {
                    book.set_active_sheet(2);
                    book.sheet_mut(2)
                        .unwrap()
                        .set_state(SheetStateValues::Hidden);
                }
                "hide-active-next-hidden" => {
                    book.sheet_mut(2)
                        .unwrap()
                        .set_state(SheetStateValues::Hidden);
                    book.sheet_mut(1)
                        .unwrap()
                        .set_state(SheetStateValues::Hidden);
                }
                "very-hide-active" => {
                    book.sheet_mut(1)
                        .unwrap()
                        .set_state(SheetStateValues::VeryHidden);
                }
                "activate-hidden" => {
                    book.sheet_mut(0)
                        .unwrap()
                        .set_state(SheetStateValues::Hidden);
                    book.set_active_sheet(0);
                    assert_eq!(book.workbook_view().active_tab(), 1);
                }
                "activate-out-of-range" => {
                    book.set_active_sheet(99);
                    assert_eq!(book.workbook_view().active_tab(), 1);
                }
                "delete-active" => book.remove_sheet(1).unwrap(),
                "delete-before-active" => book.remove_sheet(0).unwrap(),
                "delete-final" => {
                    book.remove_sheet(2).unwrap();
                    book.remove_sheet(0).unwrap();
                    assert_eq!(
                        book.remove_sheet(0).unwrap_err().kind(),
                        XlsxErrorKind::InvalidArgument
                    );
                }
                "delete-last-visible" => {
                    book.sheet_mut(0)
                        .unwrap()
                        .set_state(SheetStateValues::Hidden);
                    book.sheet_mut(2)
                        .unwrap()
                        .set_state(SheetStateValues::Hidden);
                    assert_eq!(
                        book.remove_sheet(1).unwrap_err().kind(),
                        XlsxErrorKind::InvalidArgument
                    );
                }
                "delete-referenced-local" | "delete-referenced-local-global" => {
                    let mut local = DefinedName::default();
                    local
                        .set_name("Sales")
                        .set_address("Beta!$A$1")
                        .set_local_sheet_id(1);
                    local.set_hidden(true);
                    book.sheet_mut(1).unwrap().add_defined_names(local);
                    if case.ends_with("-global") {
                        let mut global = DefinedName::default();
                        global.set_name("Sales").set_address("Alpha!$A$1");
                        book.add_defined_names(global);
                    }
                    book.sheet_mut(0)
                        .unwrap()
                        .cell_mut("A10")
                        .set_formula("Beta!Sales");
                    book.remove_sheet(1).unwrap();
                }
                _ => unreachable!(),
            }
            assert_saved(&book, case, if eager { "eager" } else { "lazy" });
        }
    }
}

#[test]
fn both_state_setters_share_the_serialized_representation() {
    let mut book = umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(INPUT), true).unwrap();
    let sheet = book.sheet_mut(1).unwrap();
    sheet.set_sheet_state("hidden".to_owned());
    assert!(matches!(sheet.state(), SheetStateValues::Hidden));
    sheet.set_state(SheetStateValues::VeryHidden);
    assert_eq!(sheet.sheet_state(), "veryHidden");
    sheet.set_sheet_state("visible".to_owned());
    assert!(matches!(sheet.state(), SheetStateValues::Visible));
}

fn selected_views(book: &Workbook) -> Vec<Vec<bool>> {
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
}

#[test]
fn appending_new_and_imported_sheets_preserves_selection() {
    // Appending is a file operation: preserve selection instead of Excel's Add
    // UI activation.
    for eager in [true, false] {
        for operation in ["new", "detached", "imported"] {
            let mut book =
                umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(INPUT), eager).unwrap();
            let before =
                umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(INPUT), true).unwrap();
            let mut selected = selected_views(&before);
            match operation {
                "new" => {
                    book.new_sheet("Appended").unwrap();
                    selected.push(Vec::new());
                }
                "detached" => {
                    let mut sheet = Worksheet::default();
                    sheet.set_name("Appended");
                    selected.push(
                        sheet
                            .sheets_views()
                            .sheet_view_list()
                            .iter()
                            .map(|view| view.tab_selected())
                            .collect(),
                    );
                    book.add_sheet(sheet).unwrap();
                }
                "imported" => {
                    let mut imported =
                        umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(INPUT), true)
                            .unwrap();
                    imported.set_sheet_id(1, "42").unwrap();
                    let mut sheet = imported.sheet(1).unwrap().clone();
                    sheet.set_name("Appended");
                    selected.push(
                        sheet
                            .sheets_views()
                            .sheet_view_list()
                            .iter()
                            .map(|view| view.tab_selected())
                            .collect(),
                    );
                    book.add_sheet(sheet).unwrap();
                }
                _ => unreachable!(),
            }
            assert_eq!(
                book.workbook_view().active_tab(),
                before.workbook_view().active_tab()
            );
            if !eager {
                for index in 0..3 {
                    assert!(book.sheet(index).is_err(), "append must retain lazy sheets");
                }
            }
            for streaming in [false, true] {
                let output = save(&book, streaming);
                let restored =
                    umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(&output), true)
                        .unwrap();
                assert_eq!(
                    restored.workbook_view().active_tab(),
                    before.workbook_view().active_tab()
                );
                assert_eq!(
                    selected_views(&restored),
                    selected,
                    "{operation}/{eager}/{streaming}"
                );
                if let Some(directory) = std::env::var_os("SLICER_PACKAGE_OUTPUT_DIR") {
                    let directory = PathBuf::from(directory);
                    std::fs::create_dir_all(&directory).unwrap();
                    std::fs::write(
                        directory.join(format!("append-{operation}-{eager}-{streaming}.xlsx")),
                        output,
                    )
                    .unwrap();
                }
            }
        }
    }
}

#[test]
fn building_a_workbook_by_appending_sheets_keeps_the_first_sheet_active() {
    let mut book = umya_spreadsheet::new_file_empty_worksheet();
    for name in ["First", "Second", "Third"] {
        book.new_sheet(name).unwrap();
        assert_eq!(book.workbook_view().active_tab(), 0);
    }
    let selected = selected_views(&book);
    for streaming in [false, true] {
        let output = save(&book, streaming);
        let restored =
            umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(&output), true).unwrap();
        assert_eq!(restored.workbook_view().active_tab(), 0);
        assert_eq!(restored.active_sheet().name(), "First");
        assert_eq!(selected_views(&restored), selected);
        if let Some(directory) = std::env::var_os("SLICER_PACKAGE_OUTPUT_DIR") {
            let directory = PathBuf::from(directory);
            std::fs::create_dir_all(&directory).unwrap();
            std::fs::write(
                directory.join(format!("append-empty-{streaming}.xlsx")),
                output,
            )
            .unwrap();
        }
    }
}

#[test]
fn hiding_every_sheet_is_rejected_by_both_existing_writer_error_paths() {
    for state in [SheetStateValues::Hidden, SheetStateValues::VeryHidden] {
        let mut book =
            umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(INPUT), true).unwrap();
        for index in 0..3 {
            book.sheet_mut(index).unwrap().set_state(state.clone());
        }
        let mut output = Cursor::new(Vec::new());
        let error = umya_spreadsheet::writer::xlsx::write_writer(&book, &mut output).unwrap_err();
        assert_eq!(error.kind(), XlsxErrorKind::InvalidArgument);
        let writer = StreamingWriter::new(zip::ZipWriter::new(Cursor::new(Vec::new())), book);
        assert_eq!(
            writer.finish().unwrap_err().kind(),
            XlsxErrorKind::InvalidArgument
        );
    }
}

#[test]
fn raw_out_of_range_active_tab_is_rejected_instead_of_written() {
    let mut book = umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(INPUT), true).unwrap();
    book.workbook_view_mut().set_active_tab(99);
    let mut output = Cursor::new(Vec::new());
    assert_eq!(
        umya_spreadsheet::writer::xlsx::write_writer(&book, &mut output)
            .unwrap_err()
            .kind(),
        XlsxErrorKind::InvalidArgument
    );
    let writer = StreamingWriter::new(zip::ZipWriter::new(Cursor::new(Vec::new())), book);
    assert_eq!(
        writer.finish().unwrap_err().kind(),
        XlsxErrorKind::InvalidArgument
    );
}
