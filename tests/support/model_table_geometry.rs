use std::{
    io::Cursor,
    path::PathBuf,
};

use crate::{
    Table,
    Workbook,
    reader,
    writer,
};

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/model_operations")
}

fn geometry(
    table: &Table,
) -> (
    String,
    String,
    u32,
    bool,
    Vec<(Option<u32>, String, Option<String>, String)>,
) {
    (
        format!("{}:{}", table.area().0, table.area().1),
        table.auto_filter().range().range(),
        table.totals_row_count(),
        table.totals_row_shown(),
        table
            .columns()
            .iter()
            .map(|column| {
                (
                    column.id(),
                    column.name().into(),
                    column.totals_row_label().map(str::to_owned),
                    column.totals_row_function_str().into(),
                )
            })
            .collect(),
    )
}

fn edit_cells(
    book: &mut Workbook,
    worksheet: bool,
    row: bool,
    insert: bool,
    start: u32,
    count: u32,
) {
    if worksheet {
        let sheet = book.sheet_by_name_mut("Data").unwrap();
        match (row, insert) {
            (true, true) => sheet.insert_new_row(start, count),
            (true, false) => sheet.remove_row(start, count),
            (false, true) => sheet.insert_new_column_by_index(start, count),
            (false, false) => sheet.remove_column_by_index(start, count),
        }
    } else {
        match (row, insert) {
            (true, true) => book.insert_new_row("Data", start, count),
            (true, false) => book.remove_row("Data", start, count),
            (false, true) => book.insert_new_column_by_index("Data", start, count),
            (false, false) => book.remove_column_by_index("Data", start, count),
        }
    }
}

fn answers() -> serde_json::Value {
    serde_json::from_str(include_str!(
        "../fixtures/model_operations/table-known-answers.json"
    ))
    .unwrap()
}

fn assert_cells(sheet: &crate::Worksheet, expected: &serde_json::Value, label: &str) {
    for expected in expected.as_array().unwrap() {
        let coordinate = expected[0].as_str().unwrap();
        let actual = sheet.cell(coordinate).map_or_else(String::new, |cell| {
            if cell.is_formula() {
                cell.formula().to_owned()
            } else {
                cell.value().to_string()
            }
        });
        assert_eq!(
            actual,
            expected[1].as_str().unwrap(),
            "{label} {coordinate}"
        );
    }
}

fn assert_retained_row(sheet: &crate::Worksheet, case: &serde_json::Value, label: &str) {
    if let Some(retained) = case.get("retainedRow").filter(|value| !value.is_null()) {
        assert_eq!(
            sheet.row_dimension(6).unwrap().height(),
            retained["height"].as_f64().unwrap(),
            "{label}"
        );
        assert_eq!(retained["color"], 255, "{label}: Excel RGB red");
        assert_eq!(
            sheet
                .cell("F6")
                .unwrap()
                .style()
                .background_color()
                .unwrap()
                .argb_str(),
            "FFFF0000",
            "{label}"
        );
    }
}

#[test]
fn table_geometry_matches_excel_through_both_cell_edit_paths_and_writers() {
    let answers = answers();
    for case in answers["cases"].as_array().unwrap() {
        let label = case["case"].as_str().unwrap();
        let row = case["axis"] == "row";
        let insert = case["operation"] == "insert";
        let start = case["start"].as_u64().unwrap() as u32;
        let count = case["count"].as_u64().unwrap() as u32;
        let expected_tables = if case["removed"] == true {
            serde_json::json!([])
        } else {
            serde_json::json!([case["state"]])
        };
        let mut written_tables = expected_tables.clone();
        for table in written_tables.as_array_mut().unwrap() {
            for column in table[4].as_array_mut().unwrap() {
                if column[2].is_null() {
                    column[2] = serde_json::json!("");
                }
            }
        }
        for eager in [false, true] {
            for worksheet in [false, true] {
                let mut book = reader::xlsx::read_reader(
                    Cursor::new(
                        std::fs::read(
                            fixtures().join(format!("{}.xlsx", case["base"].as_str().unwrap())),
                        )
                        .unwrap(),
                    ),
                    eager,
                )
                .unwrap();
                book.sheet_by_name_mut("Data").unwrap();
                if label == "table-extra-column-name-collision" {
                    let sheet = book.sheet_by_name_mut("Data").unwrap();
                    sheet.tables_mut()[0].columns_mut()[1].set_name("Column1".into());
                    sheet.cell_mut("D5").set_value("Column1");
                }
                if label == "table-extra-column-id-gap" {
                    edit_cells(&mut book, worksheet, false, false, 4, 1);
                }
                if label.starts_with("table-sentinels-") {
                    for coordinate in ["F6", "Z6", "C11", "F11", "Z11", "C20", "F20", "Z20"] {
                        book.sheet_by_name_mut("Data")
                            .unwrap()
                            .cell_mut(coordinate)
                            .set_value(coordinate);
                    }
                    let sheet = book.sheet_by_name_mut("Data").unwrap();
                    sheet
                        .row_dimension_mut(6)
                        .set_height(37.0)
                        .set_custom_height(true);
                    sheet
                        .cell_mut("F6")
                        .style_mut()
                        .set_background_color("FFFF0000");
                    let refs = book.new_sheet("Refs").unwrap();
                    for (index, formula) in [
                        "Data!E9",
                        "Data!E10",
                        "Data!C11",
                        "SUM(Data!E6:E9)",
                        "SUM(Data!E8:E9)",
                        "SUM(Data!E7:E8)",
                        "SUM(Data!E6:E8)",
                        "SUM(Data!C6:E9)",
                        "SUM(Data!F6:F9)",
                        "SUM(Data!E5:E9)",
                        "Data!E6",
                        "Data!F6",
                        "SUM(Data!E6:E6)",
                        "SUM(Data!E5:E6)",
                    ]
                    .into_iter()
                    .enumerate()
                    {
                        refs.cell_mut((1, index as u32 + 1)).set_formula(formula);
                    }
                }
                let before = format!("{book:?}");
                edit_cells(&mut book, worksheet, row, insert, start, count);
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
                            reread.sheet_by_name("Data").unwrap().tables().len(),
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
                    serde_json::to_value(
                        book.sheet_by_name("Data")
                            .unwrap()
                            .tables()
                            .iter()
                            .map(geometry)
                            .collect::<Vec<_>>()
                    )
                    .unwrap(),
                    expected_tables,
                    "{label} eager={eager} worksheet={worksheet}"
                );
                assert_cells(book.sheet_by_name("Data").unwrap(), &case["cells"], label);
                assert_retained_row(book.sheet_by_name("Data").unwrap(), case, label);
                if label.starts_with("table-sentinels-") && !worksheet {
                    assert_cells(
                        book.sheet_by_name("Refs").unwrap(),
                        &case["references"],
                        label,
                    );
                }
                for light in [false, true] {
                    let mut bytes = Vec::new();
                    if light {
                        writer::xlsx::write_writer_light(&book, &mut bytes).unwrap();
                    } else {
                        writer::xlsx::write_writer(&book, &mut bytes).unwrap();
                    }
                    let reread = reader::xlsx::read_reader(Cursor::new(&bytes), true).unwrap();
                    assert_eq!(
                        serde_json::to_value(
                            reread
                                .sheet_by_name("Data")
                                .unwrap()
                                .tables()
                                .iter()
                                .map(geometry)
                                .collect::<Vec<_>>()
                        )
                        .unwrap(),
                        written_tables,
                        "{label} eager={eager} worksheet={worksheet} light={light}"
                    );
                    assert_cells(reread.sheet_by_name("Data").unwrap(), &case["cells"], label);
                    assert_retained_row(reread.sheet_by_name("Data").unwrap(), case, label);
                    if label.starts_with("table-sentinels-") && !worksheet {
                        assert_cells(
                            reread.sheet_by_name("Refs").unwrap(),
                            &case["references"],
                            label,
                        );
                    }
                    let output = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                        .join("target/model-operations/table");
                    std::fs::create_dir_all(&output).unwrap();
                    std::fs::write(
                        output.join(format!("{label}-{eager}-{worksheet}-{light}.xlsx")),
                        bytes,
                    )
                    .unwrap();
                }
            }
        }
    }
}

#[test]
fn invalid_table_edits_are_atomic_and_filter_payload_is_preserved() {
    let book = reader::xlsx::read(fixtures().join("geometry-table.xlsx")).unwrap();
    let mut table = book.sheet_by_name("Data").unwrap().tables()[0].clone();
    let before = geometry(&table);
    assert!(table.insert_geometry(0, 1, 0, 0).is_err());
    assert!(table.insert_geometry(4, 16_384, 0, 0).is_err());
    assert!(table.insert_geometry(0, 0, 8, 1_048_576).is_err());
    assert!(table.remove_geometry(4, 1, 5, 1).is_err());
    assert_eq!(geometry(&table), before);
    let mut invalid = table.clone();
    invalid.set_area(((3, u32::MAX), (5, u32::MAX)));
    let invalid_before = geometry(&invalid);
    assert!(invalid.insert_geometry(0, 0, 1, 1).is_err());
    assert!(invalid.remove_geometry(0, 0, 1, 1).is_err());
    assert_eq!(geometry(&invalid), invalid_before);
    let mut filter = crate::FilterColumn::default();
    filter.set_column_id(1).set_hidden_button(true);
    table.auto_filter_mut().add_filter_column(filter);
    table.insert_geometry(4, 1, 0, 0).unwrap();
    assert_eq!(table.auto_filter().filter_columns()[0].column_id(), 2);
    assert!(table.auto_filter().filter_columns()[0].hidden_button());
    table.remove_geometry(4, 1, 0, 0).unwrap();
    assert_eq!(table.auto_filter().filter_columns()[0].column_id(), 1);
    table.remove_geometry(4, 1, 0, 0).unwrap();
    assert!(table.auto_filter().filter_columns().is_empty());
}

#[test]
fn inserting_into_an_in_memory_table_preserves_the_serialized_column_identities() {
    let mut book = reader::xlsx::read(fixtures().join("geometry-table.xlsx")).unwrap();
    for column in &mut book.sheet_by_name_mut("Data").unwrap().tables_mut()[0].columns {
        column.id = None;
    }
    book.insert_new_column_by_index("Data", 4, 1);
    let expected = answers();
    let expected = expected["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["case"] == "table-column-insert-4-1")
        .unwrap();
    let table = &book.sheet_by_name("Data").unwrap().tables()[0];
    assert_eq!(
        serde_json::to_value(geometry(table)).unwrap(),
        expected["state"]
    );
    let table = &mut book.sheet_by_name_mut("Data").unwrap().tables_mut()[0];
    table.columns[0].id = table.columns[1].id;
    let before = geometry(table);
    assert!(table.insert_geometry(4, 1, 0, 0).is_err());
    assert_eq!(geometry(table), before);
}

#[test]
fn other_sheet_reference_edits_ignore_the_receiving_sheets_models() {
    for header_in_cut in [false, true] {
        for insert in [false, true] {
            let mut book = reader::xlsx::read(fixtures().join("geometry-table.xlsx")).unwrap();
            let sheet = book.sheet_by_name_mut("Data").unwrap();
            sheet.cell_mut("Z30").set_formula("Report!A6");
            sheet.cell_mut("Z31").set_formula("Report!A10");
            let before_table = geometry(&sheet.tables()[0]);
            let start = if header_in_cut { 5 } else { 6 };
            if insert {
                sheet.insert_new_row_from_other_sheet("Report", start, 4);
            } else {
                sheet.remove_row_from_other_sheet("Report", start, 4);
            }
            assert_eq!(geometry(&sheet.tables()[0]), before_table);
            assert_eq!(
                sheet.cell("Z30").unwrap().formula(),
                if insert { "Report!A10" } else { "Report!#REF!" }
            );
            assert_eq!(
                sheet.cell("Z31").unwrap().formula(),
                if insert { "Report!A14" } else { "Report!A6" }
            );
            for light in [false, true] {
                let mut bytes = Vec::new();
                if light {
                    writer::xlsx::write_writer_light(&book, &mut bytes).unwrap();
                } else {
                    writer::xlsx::write_writer(&book, &mut bytes).unwrap();
                }
                let reread = reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap();
                let sheet = reread.sheet_by_name("Data").unwrap();
                assert_eq!(
                    sheet.cell("Z30").unwrap().formula(),
                    if insert { "Report!A10" } else { "Report!#REF!" }
                );
                assert_eq!(
                    sheet.cell("Z31").unwrap().formula(),
                    if insert { "Report!A14" } else { "Report!A6" }
                );
            }
        }
    }
}

#[test]
fn an_unpopulated_table_does_not_block_edits_after_its_area() {
    let mut book = crate::new_file();
    let sheet = book.sheet_by_name_mut("Sheet1").unwrap();
    sheet.add_table(Table::new("T", ("A1", "C4")));
    sheet.cell_mut("A500").set_value("sentinel");
    sheet.insert_new_row(500, 1);
    assert_eq!(sheet.cell("A501").unwrap().value(), "sentinel");
    sheet.remove_row(500, 1);
    assert_eq!(sheet.cell("A500").unwrap().value(), "sentinel");
    assert!(sheet.tables()[0].columns().is_empty());
}

#[test]
fn deleting_below_a_header_only_or_invalid_totals_table_does_not_retain_a_row() {
    for totals in [0, 6] {
        let mut book = reader::xlsx::read(fixtures().join("geometry-table.xlsx")).unwrap();
        let sheet = book.sheet_by_name_mut("Data").unwrap();
        sheet.tables_mut()[0].set_area(("C5", "E5"));
        sheet.tables_mut()[0].totals_row_count.set_value(totals);
        sheet.cell_mut("Z6").set_value("deleted");
        sheet.cell_mut("Z7").set_value("moved");
        sheet.remove_row(6, 1);
        assert_eq!(sheet.cell("Z6").unwrap().value(), "moved");
        assert_eq!(sheet.tables()[0].area().1.row_num(), 5);
    }
}
