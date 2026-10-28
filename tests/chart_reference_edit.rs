//! Stage 9 reference-owner regression tests; broader edit matrix remains open.
use umya_spreadsheet::{Chart, drawing::charts::{ChartReferenceEdit, ChartText, Formula, ReferenceCell, ReferenceRect, StringReference, Title}};

#[test]
fn replacing_a_compound_formula_with_an_address_clears_the_old_authoritative_branch() {
    let mut formula = Formula::default();
    formula.set_string_value("SUM(Data!A1,Data!B2)");
    formula.set_address_str("'Bob''s Data'!$B$3:$B$6");
    assert_eq!(formula.address_str(), "'Bob''s Data'!$B$3:$B$6");
    #[cfg(feature = "serde")]
    {
        let json: Formula = serde_json::from_slice(&serde_json::to_vec(&formula).unwrap()).unwrap();
        let binary: Formula = bincode::deserialize(&bincode::serialize(&json).unwrap()).unwrap();
        assert_eq!(binary.address_str(), "'Bob''s Data'!$B$3:$B$6");
    }
    formula.set_string_value("SUM(Data!A1,Data!B2)");
    assert_eq!(formula.address_str(), "SUM(Data!A1,Data!B2)");
}

#[test]
fn approved_move_edit_exposes_one_based_inclusive_geometry() {
    let edit = ChartReferenceEdit::MoveRange {
        source_sheet: "Data".into(),
        source: ReferenceRect { first: ReferenceCell { column: 2, row: 2 }, last: ReferenceCell { column: 2, row: 5 } },
        destination_sheet: "Other".into(),
        destination: ReferenceCell { column: 5, row: 8 },
    };
    assert_eq!(edit.clone(), edit);
}

#[test]
fn title_reference_structural_insert_adjusts_once_and_preserves_literal() {
    let mut reference = StringReference::default();
    reference.formula_mut().set_string_value("SUM(Data!$B$2:$B$5,\"Data!$B$2\")");
    let mut text = ChartText::default(); text.set_string_reference(reference);
    let mut title = Title::default(); title.set_chart_text(text);
    let mut chart = Chart::default();
    chart.new_chart(&umya_spreadsheet::ChartType::LineChart, Default::default(), Default::default(), vec![]);
    chart.chart_space_mut().chart_mut().set_title(title);
    let report = chart.adjust_references("Host", &ChartReferenceEdit::InsertRows { sheet: "Data".into(), at: 4, count: 1 });
    let value = chart.chart_space().chart().title().unwrap().chart_text().unwrap().string_reference().unwrap().formula().address_str();
    assert_eq!(value, "SUM(Data!$B$2:$B$6,\"Data!$B$2\")");
    assert_eq!(report.visited, 1);
    assert_eq!(report.changed, 1);
    assert_eq!(report.invalidated, 0);
}

#[test]
fn public_chart_noop_leaves_formula_bytes_and_report_unchanged() {
    let original = "SUM(Data!$B$2:$B$5,\"Data!$B$2\",MyNamedRange)";
    let mut reference = StringReference::default(); reference.formula_mut().set_string_value(original);
    let mut text = ChartText::default(); text.set_string_reference(reference);
    let mut title = Title::default(); title.set_chart_text(text);
    let mut chart = Chart::default();
    chart.new_chart(&umya_spreadsheet::ChartType::LineChart, Default::default(), Default::default(), vec![]);
    chart.chart_space_mut().chart_mut().set_title(title);
    let report = chart.adjust_references("Host", &ChartReferenceEdit::InsertRows { sheet: "Data".into(), at: 4, count: 0 });
    assert_eq!(chart.chart_space().chart().title().unwrap().chart_text().unwrap().string_reference().unwrap().formula().address_str(), original);
    assert_eq!(report.visited, 1); assert_eq!(report.changed, 0); assert_eq!(report.invalidated, 0);
}

#[test]
fn public_workbook_cross_sheet_move_relocates_cells_and_updates_chart_only_host() {
    let mut book = umya_spreadsheet::new_file();
    book.new_sheet("Other").unwrap(); book.new_sheet("Host").unwrap();
    book.sheet_mut(0).unwrap().cell_mut((2,2)).set_value("moved");
    let mut reference = StringReference::default(); reference.formula_mut().set_address_str("Sheet1!$B$2");
    let mut text = ChartText::default(); text.set_string_reference(reference);
    let mut title = Title::default(); title.set_chart_text(text);
    let mut chart = Chart::default();
    chart.new_chart(&umya_spreadsheet::ChartType::LineChart, Default::default(), Default::default(), vec!["Host!$A$1"]);
    chart.chart_space_mut().chart_mut().set_title(title);
    book.sheet_mut(2).unwrap().add_chart(chart);
    book.move_range("Sheet1", ReferenceRect { first: ReferenceCell { column: 2, row: 2 }, last: ReferenceCell { column: 2, row: 2 } }, "Other", ReferenceCell { column: 5, row: 8 }).unwrap();
    assert!(book.sheet(0).unwrap().cell((2,2)).is_none());
    assert_eq!(book.sheet(1).unwrap().cell((5,8)).unwrap().value(), "moved");
    let chart = &book.sheet(2).unwrap().chart_collection()[0];
    assert_eq!(chart.chart_space().chart().title().unwrap().chart_text().unwrap().string_reference().unwrap().formula().address_str(), "Other!$E$8");
}

#[test]
fn public_workbook_move_invalid_input_preserves_source_and_destination() {
    let mut book = umya_spreadsheet::new_file(); book.new_sheet("Other").unwrap();
    book.sheet_mut(0).unwrap().cell_mut((2,2)).set_value("source");
    book.sheet_mut(1).unwrap().cell_mut((5,8)).set_value("destination");
    let source = ReferenceRect { first: ReferenceCell { column: 2, row: 2 }, last: ReferenceCell { column: 2, row: 2 } };
    assert!(book.move_range("Sheet1", source.clone(), "Other", ReferenceCell { column: 0, row: 8 }).is_err());
    assert!(book.move_range("Sheet1", source, "Missing", ReferenceCell { column: 5, row: 8 }).is_err());
    assert_eq!(book.sheet(0).unwrap().cell((2,2)).unwrap().value(), "source");
    assert_eq!(book.sheet(1).unwrap().cell((5,8)).unwrap().value(), "destination");
}

#[test]
fn moved_formula_uses_captured_excel_cross_sheet_cut_semantics() {
    let mut book = umya_spreadsheet::new_file();
    book.new_sheet("Other").unwrap();
    book.sheet_mut(0).unwrap().cell_mut((3,3)).set_formula("SUM(A1:B2)");
    book.sheet_mut(1).unwrap().cell_mut((7,9)).set_value("destination");
    let source = ReferenceRect { first: ReferenceCell { column: 2, row: 2 }, last: ReferenceCell { column: 3, row: 3 } };
    let result = book.move_range("Sheet1", source, "Other", ReferenceCell { column: 6, row: 8 });
    result.unwrap();
    assert!(book.sheet(0).unwrap().cell((3,3)).is_none());
    assert_eq!(book.sheet(1).unwrap().cell((7,9)).unwrap().formula(), "SUM(F8:F8)");
}

#[test]
fn public_workbook_move_adjusts_nonmoved_formula_and_name_once() {
    let mut book = umya_spreadsheet::new_file();
    book.new_sheet("Other").unwrap();
    book.new_sheet("Host").unwrap();
    book.sheet_mut(0).unwrap().cell_mut((2,2)).set_value("moved");
    book.sheet_mut(2).unwrap().cell_mut((1,1)).set_formula("Sheet1!$B$2").set_formula_result_number(7);
    book.sheet_mut(2).unwrap().add_defined_name("Target", "Sheet1!$B$2").unwrap();
    book.move_range("Sheet1", ReferenceRect { first: ReferenceCell { column: 2, row: 2 }, last: ReferenceCell { column: 2, row: 2 } }, "Other", ReferenceCell { column: 5, row: 8 }).unwrap();
    let host = book.sheet(2).unwrap();
    assert_eq!(host.cell((1,1)).unwrap().formula(), "Other!$E$8");
    assert_eq!(host.defined_names()[0].address(), "Other!$E$8");
}

#[test]
fn workbook_row_insert_adjusts_remote_chart_once_without_touching_its_unqualified_reference() {
    let mut book = umya_spreadsheet::new_file();
    book.new_sheet("Host").unwrap();
    let mut remote = StringReference::default();
    remote.formula_mut().set_string_value("SUM(Sheet1!$B$2:$B$5,$C$2:$C$5)");
    let mut text = ChartText::default(); text.set_string_reference(remote);
    let mut title = Title::default(); title.set_chart_text(text);
    let mut chart = Chart::default();
    chart.new_chart(&umya_spreadsheet::ChartType::LineChart, Default::default(), Default::default(), vec!["Host!$A$1"]);
    chart.chart_space_mut().chart_mut().set_title(title);
    book.sheet_mut(1).unwrap().add_chart(chart);
    book.insert_new_row("Sheet1", 4, 1);
    let chart = &book.sheet(1).unwrap().chart_collection()[0];
    let value = chart.chart_space().chart().title().unwrap().chart_text().unwrap().string_reference().unwrap().formula().address_str();
    assert_eq!(value, "SUM(Sheet1!$B$2:$B$6,$C$2:$C$5)");
}

#[test]
fn workbook_rename_then_delete_adjusts_remote_chart_and_cell_once() {
    let mut book = umya_spreadsheet::new_file();
    book.new_sheet("Host").unwrap();
    book.sheet_mut(1).unwrap().cell_mut((1,1)).set_formula("Sheet1!$B$2");
    let mut reference = StringReference::default(); reference.formula_mut().set_address_str("Sheet1!$B$2");
    let mut text = ChartText::default(); text.set_string_reference(reference);
    let mut title = Title::default(); title.set_chart_text(text);
    let mut chart = Chart::default();
    chart.new_chart(&umya_spreadsheet::ChartType::LineChart, Default::default(), Default::default(), vec!["Host!$A$1"]);
    chart.chart_space_mut().chart_mut().set_title(title);
    book.sheet_mut(1).unwrap().add_chart(chart);
    book.set_sheet_name(0, "Data").unwrap();
    let host = book.sheet(1).unwrap();
    assert_eq!(host.cell((1,1)).unwrap().formula(), "Data!$B$2");
    let title_formula = || host.chart_collection()[0].chart_space().chart().title().unwrap().chart_text().unwrap().string_reference().unwrap().formula().address_str();
    assert_eq!(title_formula(), "Data!$B$2");
    book.remove_sheet_by_name("Data").unwrap();
    let host = book.sheet(0).unwrap();
    assert_eq!(host.cell((1,1)).unwrap().formula(), "#REF!");
    assert_eq!(host.chart_collection()[0].chart_space().chart().title().unwrap().chart_text().unwrap().string_reference().unwrap().formula().address_str(), "#REF!");
}

#[test]
fn moved_and_stationary_cell_formulas_use_distinct_cut_contexts() {
    let mut book = umya_spreadsheet::new_file();
    book.sheet_mut(0).unwrap().cell_mut((3,3)).set_formula("SUM(A1:B2)");
    book.sheet_mut(0).unwrap().cell_mut((5,3)).set_formula("SUM(A1:B2)");
    let source = ReferenceRect { first: ReferenceCell { column: 1, row: 2 }, last: ReferenceCell { column: 3, row: 3 } };
    book.move_range("Sheet1", source, "Sheet1", ReferenceCell { column: 1, row: 4 }).unwrap();
    let sheet = book.sheet(0).unwrap();
    assert_eq!(sheet.cell((3,5)).unwrap().formula(), "SUM(A1:B4)");
    assert_eq!(sheet.cell((5,3)).unwrap().formula(), "SUM(A1:B2)");
}

#[test]
fn worksheet_local_and_other_sheet_edits_each_visit_chart_once() {
    let mut book = umya_spreadsheet::new_file();
    book.new_sheet("Host").unwrap();
    let mut reference = StringReference::default();
    reference.formula_mut().set_string_value("SUM(Sheet1!$B$2:$B$5,$C$2:$C$5)");
    let mut text = ChartText::default(); text.set_string_reference(reference);
    let mut title = Title::default(); title.set_chart_text(text);
    let mut chart = Chart::default();
    chart.new_chart(&umya_spreadsheet::ChartType::LineChart, Default::default(), Default::default(), vec!["Host!$A$1"]);
    chart.chart_space_mut().chart_mut().set_title(title);
    book.sheet_mut(1).unwrap().add_chart(chart);
    book.sheet_mut(1).unwrap().insert_new_row_from_other_sheet("Sheet1", 4, 1);
    let value = book.sheet(1).unwrap().chart_collection()[0].chart_space().chart().title().unwrap().chart_text().unwrap().string_reference().unwrap().formula().address_str();
    assert_eq!(value, "SUM(Sheet1!$B$2:$B$6,$C$2:$C$5)");
    book.sheet_mut(1).unwrap().insert_new_row(4, 1);
    let value = book.sheet(1).unwrap().chart_collection()[0].chart_space().chart().title().unwrap().chart_text().unwrap().string_reference().unwrap().formula().address_str();
    assert_eq!(value, "SUM(Sheet1!$B$2:$B$6,$C$2:$C$6)");
}

#[test]
fn workbook_row_insert_adjusts_global_and_remote_local_defined_names_once() {
    let mut book = umya_spreadsheet::new_file();
    book.new_sheet("Host").unwrap();
    let mut global = umya_spreadsheet::structs::DefinedName::default();
    global.set_name("GlobalRef").set_address("Sheet1!$B$2:$B$5");
    book.add_defined_names(global);
    book.sheet_mut(1).unwrap().add_defined_name("LocalRef", "Sheet1!$B$2:$B$5").unwrap();
    book.insert_new_row("Sheet1", 4, 1);
    assert_eq!(book.defined_names()[0].address(), "Sheet1!$B$2:$B$6");
    assert_eq!(book.sheet(1).unwrap().defined_names()[0].address(), "Sheet1!$B$2:$B$6");
}



#[test]
fn invalid_structural_edit_preserves_chart_and_worksheet_before_geometry() {
    let mut book = umya_spreadsheet::new_file();
    book.sheet_mut(0).unwrap().cell_mut((1,1)).set_value("untouched");
    let mut reference = StringReference::default(); reference.formula_mut().set_address_str("Sheet1!$A$1");
    let mut text = ChartText::default(); text.set_string_reference(reference);
    let mut title = Title::default(); title.set_chart_text(text);
    let mut chart = Chart::default();
    chart.new_chart(&umya_spreadsheet::ChartType::LineChart, Default::default(), Default::default(), vec![]);
    chart.chart_space_mut().chart_mut().set_title(title);
    let report = chart.adjust_references("Sheet1", &ChartReferenceEdit::InsertRows { sheet: "Sheet1".into(), at: 0, count: 1 });
    assert_eq!(report.visited, 1);
    assert_eq!(report.changed, 0);
    assert_eq!(chart.chart_space().chart().title().unwrap().chart_text().unwrap().string_reference().unwrap().formula().address_str(), "Sheet1!$A$1");
    book.sheet_mut(0).unwrap().insert_new_row(0, 1);
    assert_eq!(book.sheet(0).unwrap().cell((1,1)).unwrap().value(), "untouched");
}

#[test]
fn bare_default_chart_reference_adjustment_is_an_empty_noop() {
    let mut chart = Chart::default();
    let report = chart.adjust_references("Sheet1", &ChartReferenceEdit::InsertRows { sheet: "Sheet1".into(), at: 1, count: 1 });
    assert_eq!(report.visited, 0);
    assert_eq!(report.changed, 0);
    assert_eq!(report.invalidated, 0);
    assert_eq!(report.preserved_opaque_extensions, 0);
}

#[test]
fn workbook_rename_adjusts_authoritative_shared_formula_view_and_preserves_metadata() {
    let mut book = umya_spreadsheet::new_file();
    book.new_sheet("Host").unwrap();
    let mut formula = umya_spreadsheet::structs::CellFormula::default();
    formula.set_text("Sheet1!$B$2").set_text_view("Sheet1!$B$3").set_shared_index(17);
    book.sheet_mut(1).unwrap().cell_mut((1,1)).cell_value_mut().set_formula_obj(formula);
    book.set_sheet_name(0, "Data").unwrap();
    let value = book.sheet(1).unwrap().cell((1,1)).unwrap().cell_value();
    assert_eq!(value.formula(), "Data!$B$3");
    assert_eq!(value.formula_obj().unwrap().shared_index(), 17);
}

#[test]
fn local_sheet_id_scope_survives_rename_delete_and_reader_roundtrip() {
    let mut book = umya_spreadsheet::new_file();
    book.new_sheet("Keep").unwrap(); book.new_sheet("Later").unwrap();
    for (index, label, target) in [(1, "KeepLocal", "Keep!$A$1"), (2, "LaterLocal", "Later!$A$1")] {
        let mut name = umya_spreadsheet::structs::DefinedName::default();
        name.set_name(label).set_address(target); name.set_local_sheet_id(index as u32);
        book.sheet_mut(index).unwrap().add_defined_names(name);
    }
    let mut stored_global = umya_spreadsheet::structs::DefinedName::default();
    stored_global.set_name("LaterStoredGlobal").set_address("Later!$B$1"); stored_global.set_local_sheet_id(2);
    book.add_defined_names(stored_global);
    book.set_sheet_name(1, "Renamed").unwrap();
    assert_eq!(book.sheet(1).unwrap().defined_names()[0].address(), "Renamed!$A$1");
    book.remove_sheet(0).unwrap();
    assert_eq!(book.sheet(0).unwrap().defined_names()[0].local_sheet_id(), 0);
    assert_eq!(book.sheet(1).unwrap().defined_names()[0].local_sheet_id(), 1);
    assert_eq!(book.defined_names()[0].local_sheet_id(), 1);
    let mut bytes = Vec::new();
    umya_spreadsheet::writer::xlsx::write_writer(&book, &mut std::io::Cursor::new(&mut bytes)).unwrap();
    for eager in [true, false] {
        let mut reread = umya_spreadsheet::reader::xlsx::read_reader(std::io::Cursor::new(bytes.clone()), eager).unwrap();
        for (index, expected_name, expected_id) in [(0, "KeepLocal", 0), (1, "LaterLocal", 1), (1, "LaterStoredGlobal", 1)] {
            let sheet = reread.sheet_mut(index).unwrap();
            let name = sheet.defined_names().iter().find(|name| name.name() == expected_name).unwrap();
            assert_eq!(name.local_sheet_id(), expected_id);
        }
        assert_eq!(reread.sheet(0).unwrap().name(), "Renamed");
        assert_eq!(reread.sheet(1).unwrap().name(), "Later");
    }
}

#[test]
fn row_edit_preserves_error_result_but_invalidates_stale_numeric_result() {
    use umya_spreadsheet::{CellErrorType, CellRawValue};
    let mut book = umya_spreadsheet::new_file(); book.new_sheet("Host").unwrap();
    let host = book.sheet_mut(1).unwrap();
    host.cell_mut((1,1)).set_formula("Sheet1!A1").set_formula_result_error(CellErrorType::Name);
    host.cell_mut((2,1)).set_formula("Sheet1!A1").set_formula_result_number(42);
    book.insert_new_row("Sheet1", 1, 1);
    let host = book.sheet(1).unwrap();
    assert_eq!(host.cell((1,1)).unwrap().formula(), "Sheet1!A2");
    assert_eq!(host.cell((1,1)).unwrap().raw_value(), &CellRawValue::Error(CellErrorType::Name));
    assert_eq!(host.cell((2,1)).unwrap().formula(), "Sheet1!A2");
    assert_eq!(host.cell((2,1)).unwrap().raw_value(), &CellRawValue::Empty);
}
