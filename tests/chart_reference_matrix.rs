#![cfg(feature = "serde")]
//! Basic 32-owner by seven-edit matrix through the ordinary XLSX reader,
//! live Chart API, both persistence codecs, writer, and reader again.
//! Inputs are authored synthetic specimens, not native Excel witnesses.
use std::io::{Cursor, Read, Write};

use quick_xml::{events::Event, Reader};
use sha2::{Digest, Sha256};
use umya_spreadsheet::{
    drawing::charts::{ChartReferenceEdit, ReferenceCell, ReferenceRect},
    Chart, ChartType, Workbook,
};
use zip::{write::SimpleFileOptions, ZipArchive, ZipWriter};

const ORIGINAL: &str = "Data!$B$2:$B$5";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Scenario {
    RenameSheet,
    DeleteSheet,
    InsertRows,
    InsertColumns,
    DeleteRows,
    DeleteColumns,
    MoveRange,
    DeleteAllRows,
    DeleteAllColumns,
}

fn save(book: &Workbook) -> Vec<u8> {
    let mut bytes = Vec::new();
    umya_spreadsheet::writer::xlsx::write_writer(book, &mut Cursor::new(&mut bytes)).unwrap();
    bytes
}

fn load(bytes: &[u8]) -> Workbook {
    umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap()
}

fn chart(book: &Workbook) -> &Chart {
    book.sheet_collection().iter().flat_map(|s| s.chart_collection()).next().unwrap()
}

fn chart_mut(book: &mut Workbook) -> &mut Chart {
    book.sheet_collection_mut().iter_mut().flat_map(|s| s.chart_collection_mut()).next().unwrap()
}

fn with_chart_xml(bytes: &[u8], xml: &str) -> Vec<u8> {
    let mut input = ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut output = ZipWriter::new(Cursor::new(Vec::new()));
    let mut replaced = 0;
    for i in 0..input.len() {
        let mut part = input.by_index(i).unwrap();
        let name = part.name().to_owned();
        let mut data = Vec::new();
        part.read_to_end(&mut data).unwrap();
        if name == "xl/charts/chart1.xml" {
            data = xml.as_bytes().to_vec();
            replaced += 1;
        }
        output.start_file(name, SimpleFileOptions::default()).unwrap();
        output.write_all(&data).unwrap();
    }
    assert_eq!(replaced, 1);
    output.finish().unwrap().into_inner()
}

fn chart_xml(bytes: &[u8]) -> String {
    let mut input = ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut result = String::new();
    input.by_name("xl/charts/chart1.xml").unwrap().read_to_string(&mut result).unwrap();
    result
}

fn reference_texts(xml: &str) -> Vec<String> {
    let mut reader = Reader::from_str(xml);
    let mut text = None;
    let mut result = Vec::new();
    loop {
        match reader.read_event().unwrap() {
            Event::Start(element) => {
                if matches!(element.local_name().as_ref(), b"f" | b"sqref") {
                    assert!(text.is_none());
                    text = Some(String::new());
                }
            }
            Event::Text(e) => {
                if let Some(value) = &mut text {
                    value.push_str(&e.decode().unwrap());
                }
            }
            Event::GeneralRef(e) => {
                if let Some(value) = &mut text {
                    let reference = e.decode().unwrap();
                    value.push_str(&quick_xml::escape::unescape(&format!("&{reference};")).unwrap());
                }
            }
            Event::End(element) => {
                if matches!(element.local_name().as_ref(), b"f" | b"sqref") {
                    result.push(text.take().expect("reference start"));
                }
            }
            Event::Eof => break,
            Event::Empty(..)
            | Event::CData(..)
            | Event::Comment(..)
            | Event::Decl(..)
            | Event::PI(..)
            | Event::DocType(..) => (),
        }
    }
    assert!(text.is_none());
    result
}

fn fixture(xml: &str) -> Workbook {
    let mut book = umya_spreadsheet::new_file();
    book.set_sheet_name(0, "Host").unwrap();
    book.new_sheet("Data").unwrap();
    book.new_sheet("Other").unwrap();
    for row in 1..=12 {
        book.sheet_mut(1).unwrap().cell_mut((2, row)).set_value_number(row * 10);
    }
    let mut from = umya_spreadsheet::drawing::spreadsheet::MarkerType::default();
    from.set_coordinate("E1");
    let mut to = umya_spreadsheet::drawing::spreadsheet::MarkerType::default();
    to.set_coordinate("M20");
    let mut chart = Chart::default();
    chart.new_chart(&ChartType::LineChart, from, to, vec![ORIGINAL]);
    book.sheet_mut(0).unwrap().add_chart(chart);
    load(&with_chart_xml(&save(&book), xml))
}

fn edit(scenario: Scenario) -> (ChartReferenceEdit, &'static str, &'static str) {
    match scenario {
        Scenario::RenameSheet => (ChartReferenceEdit::RenameSheet { old: "Data".into(), new: "Bob's Data".into() }, "rename", "'Bob''s Data'!$B$2:$B$5"),
        Scenario::DeleteSheet => (ChartReferenceEdit::DeleteSheet { sheet: "Data".into(), sheet_order: vec!["Host".into(), "Data".into(), "Other".into()] }, "delete_sheet", "#REF!"),
        Scenario::InsertRows => (ChartReferenceEdit::InsertRows { sheet: "Data".into(), at: 4, count: 1 }, "insert_rows", "Data!$B$2:$B$6"),
        Scenario::InsertColumns => (ChartReferenceEdit::InsertColumns { sheet: "Data".into(), at: 2, count: 1 }, "insert_columns", "Data!$C$2:$C$5"),
        Scenario::DeleteRows => (ChartReferenceEdit::DeleteRows { sheet: "Data".into(), rows: vec![3, 3] }, "delete_rows", "Data!$B$2:$B$4"),
        Scenario::DeleteColumns => (ChartReferenceEdit::DeleteColumns { sheet: "Data".into(), columns: vec![1, 1] }, "delete_columns", "Data!$A$2:$A$5"),
        Scenario::MoveRange => (ChartReferenceEdit::MoveRange {
            source_sheet: "Data".into(),
            source: ReferenceRect { first: ReferenceCell { column: 2, row: 2 }, last: ReferenceCell { column: 2, row: 5 } },
            destination_sheet: "Other".into(), destination: ReferenceCell { column: 5, row: 8 },
        }, "move", "Other!$E$8:$E$11"),
        Scenario::DeleteAllRows => (ChartReferenceEdit::DeleteRows { sheet: "Data".into(), rows: vec![2, 3, 4, 5] }, "delete_all_rows", "Data!#REF!"),
        Scenario::DeleteAllColumns => (ChartReferenceEdit::DeleteColumns { sheet: "Data".into(), columns: vec![2] }, "delete_all_columns", "Data!#REF!"),
    }
}

fn record_output(bytes: &[u8], role: &str, edit: &str, route: &str) {
    if let Some(root) = std::env::var_os("PHASE9_OUTPUT_DIR") {
        let source = std::env::var("PHASE9_SOURCE_ID").expect("source identity required");
        let run = std::env::var("PHASE9_RUN_ID").expect("run identity required");
        for identity in [&source, &run] {
            assert_eq!(identity.len(), 64);
            assert!(identity.bytes().all(|v| v.is_ascii_hexdigit()));
        }
        let root = std::path::PathBuf::from(root);
        std::fs::create_dir_all(&root).unwrap();
        let stem = format!("{role}--{edit}--{route}");
        let digest: String = Sha256::digest(bytes).iter().map(|v| format!("{v:02x}")).collect();
        std::fs::write(root.join(format!("{stem}.xlsx")), bytes).unwrap();
        std::fs::write(root.join(format!("{stem}.receipt")), format!("{source}\n{run}\n{digest}\n")).unwrap();
    }
}

fn exercise(role: &str, xml: &str, operation: Scenario) {
    exercise_variant(role, xml, operation);
    if operation == Scenario::DeleteRows {
        exercise_variant(role, xml, Scenario::DeleteAllRows);
    } else if operation == Scenario::DeleteColumns {
        exercise_variant(role, xml, Scenario::DeleteAllColumns);
    }
}

fn exercise_variant(role: &str, xml: &str, operation: Scenario) {
    assert_eq!(reference_texts(xml).iter().filter(|v| v.as_str() == ORIGINAL).count(), 1);
    let mut book = fixture(xml);
    let (edit, name, ordinary_expected) = edit(operation);
    let complete_deletion = matches!(operation, Scenario::DeleteAllRows | Scenario::DeleteAllColumns);
    let expected = if complete_deletion && role == "data_labels_range" { "#REF!" } else { ordinary_expected };
    let original = save(&book);
    assert_eq!(reference_texts(&chart_xml(&original)).iter().filter(|v| v.as_str() == ORIGINAL).count(), 1, "{role}: target was not imported");
    let report = chart_mut(&mut book).adjust_references("Host", &edit);
    assert!(report.visited >= 1, "{role}: native target not visited");
    assert_eq!(report.changed, 1, "{role}/{name}: not exactly once");
    assert_eq!(report.invalidated, usize::from(operation == Scenario::DeleteSheet || complete_deletion), "{role}/{name}");
    let changed = chart(&book).clone();
    let json: Chart = serde_json::from_slice(&serde_json::to_vec(&changed).unwrap()).unwrap();
    let binary: Chart = bincode::deserialize(&bincode::serialize(&changed).unwrap()).unwrap();
    for (route, model) in [("direct", changed), ("json", json), ("bincode", binary)] {
        let mut output_book = book.clone();
        *chart_mut(&mut output_book) = model;
        let output = save(&output_book);
        let texts = reference_texts(&chart_xml(&output));
        assert_eq!(texts.iter().filter(|v| v.as_str() == expected).count(), 1, "{role}/{name}/{route}: {texts:?}");
        assert!(!texts.iter().any(|v| v == ORIGINAL), "{role}/{name}/{route}: stale original");
        let mut reread = load(&output);
        let before = serde_json::to_value(chart(&reread)).unwrap();
        let noop = chart_mut(&mut reread).adjust_references("Host", &ChartReferenceEdit::InsertRows { sheet: "Data".into(), at: 1, count: 0 });
        assert_eq!(noop.changed, 0);
        assert_eq!(noop.invalidated, 0);
        assert_eq!(serde_json::to_value(chart(&reread)).unwrap(), before, "{role}: noop materialized owners");
        let reread_output = save(&reread);
        assert_eq!(reference_texts(&chart_xml(&reread_output)), texts, "{role}/{name}/{route}: reread lost references");
        record_output(&output, role, name, route);
    }
}


// Explicit source-owned case names keep the fail-closed acceptance inventory
// independent of macro expansion or observed test output.
#[test]
fn values_rename() {
    exercise("values", include_str!("fixtures/chart_reference_edits/values.xml"), Scenario::RenameSheet);
}

#[test]
fn values_delete_sheet() {
    exercise("values", include_str!("fixtures/chart_reference_edits/values.xml"), Scenario::DeleteSheet);
}

#[test]
fn values_insert_rows() {
    exercise("values", include_str!("fixtures/chart_reference_edits/values.xml"), Scenario::InsertRows);
}

#[test]
fn values_insert_columns() {
    exercise("values", include_str!("fixtures/chart_reference_edits/values.xml"), Scenario::InsertColumns);
}

#[test]
fn values_delete_rows() {
    exercise("values", include_str!("fixtures/chart_reference_edits/values.xml"), Scenario::DeleteRows);
}

#[test]
fn values_delete_columns() {
    exercise("values", include_str!("fixtures/chart_reference_edits/values.xml"), Scenario::DeleteColumns);
}

#[test]
fn values_move_range() {
    exercise("values", include_str!("fixtures/chart_reference_edits/values.xml"), Scenario::MoveRange);
}

#[test]
fn x_values_rename() {
    exercise("x_values", include_str!("fixtures/chart_reference_edits/x_values.xml"), Scenario::RenameSheet);
}

#[test]
fn x_values_delete_sheet() {
    exercise("x_values", include_str!("fixtures/chart_reference_edits/x_values.xml"), Scenario::DeleteSheet);
}

#[test]
fn x_values_insert_rows() {
    exercise("x_values", include_str!("fixtures/chart_reference_edits/x_values.xml"), Scenario::InsertRows);
}

#[test]
fn x_values_insert_columns() {
    exercise("x_values", include_str!("fixtures/chart_reference_edits/x_values.xml"), Scenario::InsertColumns);
}

#[test]
fn x_values_delete_rows() {
    exercise("x_values", include_str!("fixtures/chart_reference_edits/x_values.xml"), Scenario::DeleteRows);
}

#[test]
fn x_values_delete_columns() {
    exercise("x_values", include_str!("fixtures/chart_reference_edits/x_values.xml"), Scenario::DeleteColumns);
}

#[test]
fn x_values_move_range() {
    exercise("x_values", include_str!("fixtures/chart_reference_edits/x_values.xml"), Scenario::MoveRange);
}

#[test]
fn y_values_rename() {
    exercise("y_values", include_str!("fixtures/chart_reference_edits/y_values.xml"), Scenario::RenameSheet);
}

#[test]
fn y_values_delete_sheet() {
    exercise("y_values", include_str!("fixtures/chart_reference_edits/y_values.xml"), Scenario::DeleteSheet);
}

#[test]
fn y_values_insert_rows() {
    exercise("y_values", include_str!("fixtures/chart_reference_edits/y_values.xml"), Scenario::InsertRows);
}

#[test]
fn y_values_insert_columns() {
    exercise("y_values", include_str!("fixtures/chart_reference_edits/y_values.xml"), Scenario::InsertColumns);
}

#[test]
fn y_values_delete_rows() {
    exercise("y_values", include_str!("fixtures/chart_reference_edits/y_values.xml"), Scenario::DeleteRows);
}

#[test]
fn y_values_delete_columns() {
    exercise("y_values", include_str!("fixtures/chart_reference_edits/y_values.xml"), Scenario::DeleteColumns);
}

#[test]
fn y_values_move_range() {
    exercise("y_values", include_str!("fixtures/chart_reference_edits/y_values.xml"), Scenario::MoveRange);
}

#[test]
fn bubble_size_rename() {
    exercise("bubble_size", include_str!("fixtures/chart_reference_edits/bubble_size.xml"), Scenario::RenameSheet);
}

#[test]
fn bubble_size_delete_sheet() {
    exercise("bubble_size", include_str!("fixtures/chart_reference_edits/bubble_size.xml"), Scenario::DeleteSheet);
}

#[test]
fn bubble_size_insert_rows() {
    exercise("bubble_size", include_str!("fixtures/chart_reference_edits/bubble_size.xml"), Scenario::InsertRows);
}

#[test]
fn bubble_size_insert_columns() {
    exercise("bubble_size", include_str!("fixtures/chart_reference_edits/bubble_size.xml"), Scenario::InsertColumns);
}

#[test]
fn bubble_size_delete_rows() {
    exercise("bubble_size", include_str!("fixtures/chart_reference_edits/bubble_size.xml"), Scenario::DeleteRows);
}

#[test]
fn bubble_size_delete_columns() {
    exercise("bubble_size", include_str!("fixtures/chart_reference_edits/bubble_size.xml"), Scenario::DeleteColumns);
}

#[test]
fn bubble_size_move_range() {
    exercise("bubble_size", include_str!("fixtures/chart_reference_edits/bubble_size.xml"), Scenario::MoveRange);
}

#[test]
fn category_number_rename() {
    exercise("category_number", include_str!("fixtures/chart_reference_edits/category_number.xml"), Scenario::RenameSheet);
}

#[test]
fn category_number_delete_sheet() {
    exercise("category_number", include_str!("fixtures/chart_reference_edits/category_number.xml"), Scenario::DeleteSheet);
}

#[test]
fn category_number_insert_rows() {
    exercise("category_number", include_str!("fixtures/chart_reference_edits/category_number.xml"), Scenario::InsertRows);
}

#[test]
fn category_number_insert_columns() {
    exercise("category_number", include_str!("fixtures/chart_reference_edits/category_number.xml"), Scenario::InsertColumns);
}

#[test]
fn category_number_delete_rows() {
    exercise("category_number", include_str!("fixtures/chart_reference_edits/category_number.xml"), Scenario::DeleteRows);
}

#[test]
fn category_number_delete_columns() {
    exercise("category_number", include_str!("fixtures/chart_reference_edits/category_number.xml"), Scenario::DeleteColumns);
}

#[test]
fn category_number_move_range() {
    exercise("category_number", include_str!("fixtures/chart_reference_edits/category_number.xml"), Scenario::MoveRange);
}

#[test]
fn category_string_rename() {
    exercise("category_string", include_str!("fixtures/chart_reference_edits/category_string.xml"), Scenario::RenameSheet);
}

#[test]
fn category_string_delete_sheet() {
    exercise("category_string", include_str!("fixtures/chart_reference_edits/category_string.xml"), Scenario::DeleteSheet);
}

#[test]
fn category_string_insert_rows() {
    exercise("category_string", include_str!("fixtures/chart_reference_edits/category_string.xml"), Scenario::InsertRows);
}

#[test]
fn category_string_insert_columns() {
    exercise("category_string", include_str!("fixtures/chart_reference_edits/category_string.xml"), Scenario::InsertColumns);
}

#[test]
fn category_string_delete_rows() {
    exercise("category_string", include_str!("fixtures/chart_reference_edits/category_string.xml"), Scenario::DeleteRows);
}

#[test]
fn category_string_delete_columns() {
    exercise("category_string", include_str!("fixtures/chart_reference_edits/category_string.xml"), Scenario::DeleteColumns);
}

#[test]
fn category_string_move_range() {
    exercise("category_string", include_str!("fixtures/chart_reference_edits/category_string.xml"), Scenario::MoveRange);
}

#[test]
fn category_multilevel_rename() {
    exercise("category_multilevel", include_str!("fixtures/chart_reference_edits/category_multilevel.xml"), Scenario::RenameSheet);
}

#[test]
fn category_multilevel_delete_sheet() {
    exercise("category_multilevel", include_str!("fixtures/chart_reference_edits/category_multilevel.xml"), Scenario::DeleteSheet);
}

#[test]
fn category_multilevel_insert_rows() {
    exercise("category_multilevel", include_str!("fixtures/chart_reference_edits/category_multilevel.xml"), Scenario::InsertRows);
}

#[test]
fn category_multilevel_insert_columns() {
    exercise("category_multilevel", include_str!("fixtures/chart_reference_edits/category_multilevel.xml"), Scenario::InsertColumns);
}

#[test]
fn category_multilevel_delete_rows() {
    exercise("category_multilevel", include_str!("fixtures/chart_reference_edits/category_multilevel.xml"), Scenario::DeleteRows);
}

#[test]
fn category_multilevel_delete_columns() {
    exercise("category_multilevel", include_str!("fixtures/chart_reference_edits/category_multilevel.xml"), Scenario::DeleteColumns);
}

#[test]
fn category_multilevel_move_range() {
    exercise("category_multilevel", include_str!("fixtures/chart_reference_edits/category_multilevel.xml"), Scenario::MoveRange);
}

#[test]
fn series_title_rename() {
    exercise("series_title", include_str!("fixtures/chart_reference_edits/series_title.xml"), Scenario::RenameSheet);
}

#[test]
fn series_title_delete_sheet() {
    exercise("series_title", include_str!("fixtures/chart_reference_edits/series_title.xml"), Scenario::DeleteSheet);
}

#[test]
fn series_title_insert_rows() {
    exercise("series_title", include_str!("fixtures/chart_reference_edits/series_title.xml"), Scenario::InsertRows);
}

#[test]
fn series_title_insert_columns() {
    exercise("series_title", include_str!("fixtures/chart_reference_edits/series_title.xml"), Scenario::InsertColumns);
}

#[test]
fn series_title_delete_rows() {
    exercise("series_title", include_str!("fixtures/chart_reference_edits/series_title.xml"), Scenario::DeleteRows);
}

#[test]
fn series_title_delete_columns() {
    exercise("series_title", include_str!("fixtures/chart_reference_edits/series_title.xml"), Scenario::DeleteColumns);
}

#[test]
fn series_title_move_range() {
    exercise("series_title", include_str!("fixtures/chart_reference_edits/series_title.xml"), Scenario::MoveRange);
}

#[test]
fn chart_title_rename() {
    exercise("chart_title", include_str!("fixtures/chart_reference_edits/chart_title.xml"), Scenario::RenameSheet);
}

#[test]
fn chart_title_delete_sheet() {
    exercise("chart_title", include_str!("fixtures/chart_reference_edits/chart_title.xml"), Scenario::DeleteSheet);
}

#[test]
fn chart_title_insert_rows() {
    exercise("chart_title", include_str!("fixtures/chart_reference_edits/chart_title.xml"), Scenario::InsertRows);
}

#[test]
fn chart_title_insert_columns() {
    exercise("chart_title", include_str!("fixtures/chart_reference_edits/chart_title.xml"), Scenario::InsertColumns);
}

#[test]
fn chart_title_delete_rows() {
    exercise("chart_title", include_str!("fixtures/chart_reference_edits/chart_title.xml"), Scenario::DeleteRows);
}

#[test]
fn chart_title_delete_columns() {
    exercise("chart_title", include_str!("fixtures/chart_reference_edits/chart_title.xml"), Scenario::DeleteColumns);
}

#[test]
fn chart_title_move_range() {
    exercise("chart_title", include_str!("fixtures/chart_reference_edits/chart_title.xml"), Scenario::MoveRange);
}

#[test]
fn category_axis_title_rename() {
    exercise("category_axis_title", include_str!("fixtures/chart_reference_edits/category_axis_title.xml"), Scenario::RenameSheet);
}

#[test]
fn category_axis_title_delete_sheet() {
    exercise("category_axis_title", include_str!("fixtures/chart_reference_edits/category_axis_title.xml"), Scenario::DeleteSheet);
}

#[test]
fn category_axis_title_insert_rows() {
    exercise("category_axis_title", include_str!("fixtures/chart_reference_edits/category_axis_title.xml"), Scenario::InsertRows);
}

#[test]
fn category_axis_title_insert_columns() {
    exercise("category_axis_title", include_str!("fixtures/chart_reference_edits/category_axis_title.xml"), Scenario::InsertColumns);
}

#[test]
fn category_axis_title_delete_rows() {
    exercise("category_axis_title", include_str!("fixtures/chart_reference_edits/category_axis_title.xml"), Scenario::DeleteRows);
}

#[test]
fn category_axis_title_delete_columns() {
    exercise("category_axis_title", include_str!("fixtures/chart_reference_edits/category_axis_title.xml"), Scenario::DeleteColumns);
}

#[test]
fn category_axis_title_move_range() {
    exercise("category_axis_title", include_str!("fixtures/chart_reference_edits/category_axis_title.xml"), Scenario::MoveRange);
}

#[test]
fn date_axis_title_rename() {
    exercise("date_axis_title", include_str!("fixtures/chart_reference_edits/date_axis_title.xml"), Scenario::RenameSheet);
}

#[test]
fn date_axis_title_delete_sheet() {
    exercise("date_axis_title", include_str!("fixtures/chart_reference_edits/date_axis_title.xml"), Scenario::DeleteSheet);
}

#[test]
fn date_axis_title_insert_rows() {
    exercise("date_axis_title", include_str!("fixtures/chart_reference_edits/date_axis_title.xml"), Scenario::InsertRows);
}

#[test]
fn date_axis_title_insert_columns() {
    exercise("date_axis_title", include_str!("fixtures/chart_reference_edits/date_axis_title.xml"), Scenario::InsertColumns);
}

#[test]
fn date_axis_title_delete_rows() {
    exercise("date_axis_title", include_str!("fixtures/chart_reference_edits/date_axis_title.xml"), Scenario::DeleteRows);
}

#[test]
fn date_axis_title_delete_columns() {
    exercise("date_axis_title", include_str!("fixtures/chart_reference_edits/date_axis_title.xml"), Scenario::DeleteColumns);
}

#[test]
fn date_axis_title_move_range() {
    exercise("date_axis_title", include_str!("fixtures/chart_reference_edits/date_axis_title.xml"), Scenario::MoveRange);
}

#[test]
fn value_axis_title_rename() {
    exercise("value_axis_title", include_str!("fixtures/chart_reference_edits/value_axis_title.xml"), Scenario::RenameSheet);
}

#[test]
fn value_axis_title_delete_sheet() {
    exercise("value_axis_title", include_str!("fixtures/chart_reference_edits/value_axis_title.xml"), Scenario::DeleteSheet);
}

#[test]
fn value_axis_title_insert_rows() {
    exercise("value_axis_title", include_str!("fixtures/chart_reference_edits/value_axis_title.xml"), Scenario::InsertRows);
}

#[test]
fn value_axis_title_insert_columns() {
    exercise("value_axis_title", include_str!("fixtures/chart_reference_edits/value_axis_title.xml"), Scenario::InsertColumns);
}

#[test]
fn value_axis_title_delete_rows() {
    exercise("value_axis_title", include_str!("fixtures/chart_reference_edits/value_axis_title.xml"), Scenario::DeleteRows);
}

#[test]
fn value_axis_title_delete_columns() {
    exercise("value_axis_title", include_str!("fixtures/chart_reference_edits/value_axis_title.xml"), Scenario::DeleteColumns);
}

#[test]
fn value_axis_title_move_range() {
    exercise("value_axis_title", include_str!("fixtures/chart_reference_edits/value_axis_title.xml"), Scenario::MoveRange);
}

#[test]
fn series_axis_title_rename() {
    exercise("series_axis_title", include_str!("fixtures/chart_reference_edits/series_axis_title.xml"), Scenario::RenameSheet);
}

#[test]
fn series_axis_title_delete_sheet() {
    exercise("series_axis_title", include_str!("fixtures/chart_reference_edits/series_axis_title.xml"), Scenario::DeleteSheet);
}

#[test]
fn series_axis_title_insert_rows() {
    exercise("series_axis_title", include_str!("fixtures/chart_reference_edits/series_axis_title.xml"), Scenario::InsertRows);
}

#[test]
fn series_axis_title_insert_columns() {
    exercise("series_axis_title", include_str!("fixtures/chart_reference_edits/series_axis_title.xml"), Scenario::InsertColumns);
}

#[test]
fn series_axis_title_delete_rows() {
    exercise("series_axis_title", include_str!("fixtures/chart_reference_edits/series_axis_title.xml"), Scenario::DeleteRows);
}

#[test]
fn series_axis_title_delete_columns() {
    exercise("series_axis_title", include_str!("fixtures/chart_reference_edits/series_axis_title.xml"), Scenario::DeleteColumns);
}

#[test]
fn series_axis_title_move_range() {
    exercise("series_axis_title", include_str!("fixtures/chart_reference_edits/series_axis_title.xml"), Scenario::MoveRange);
}

#[test]
fn trendline_label_rename() {
    exercise("trendline_label", include_str!("fixtures/chart_reference_edits/trendline_label.xml"), Scenario::RenameSheet);
}

#[test]
fn trendline_label_delete_sheet() {
    exercise("trendline_label", include_str!("fixtures/chart_reference_edits/trendline_label.xml"), Scenario::DeleteSheet);
}

#[test]
fn trendline_label_insert_rows() {
    exercise("trendline_label", include_str!("fixtures/chart_reference_edits/trendline_label.xml"), Scenario::InsertRows);
}

#[test]
fn trendline_label_insert_columns() {
    exercise("trendline_label", include_str!("fixtures/chart_reference_edits/trendline_label.xml"), Scenario::InsertColumns);
}

#[test]
fn trendline_label_delete_rows() {
    exercise("trendline_label", include_str!("fixtures/chart_reference_edits/trendline_label.xml"), Scenario::DeleteRows);
}

#[test]
fn trendline_label_delete_columns() {
    exercise("trendline_label", include_str!("fixtures/chart_reference_edits/trendline_label.xml"), Scenario::DeleteColumns);
}

#[test]
fn trendline_label_move_range() {
    exercise("trendline_label", include_str!("fixtures/chart_reference_edits/trendline_label.xml"), Scenario::MoveRange);
}

#[test]
fn point_label_rename() {
    exercise("point_label", include_str!("fixtures/chart_reference_edits/point_label.xml"), Scenario::RenameSheet);
}

#[test]
fn point_label_delete_sheet() {
    exercise("point_label", include_str!("fixtures/chart_reference_edits/point_label.xml"), Scenario::DeleteSheet);
}

#[test]
fn point_label_insert_rows() {
    exercise("point_label", include_str!("fixtures/chart_reference_edits/point_label.xml"), Scenario::InsertRows);
}

#[test]
fn point_label_insert_columns() {
    exercise("point_label", include_str!("fixtures/chart_reference_edits/point_label.xml"), Scenario::InsertColumns);
}

#[test]
fn point_label_delete_rows() {
    exercise("point_label", include_str!("fixtures/chart_reference_edits/point_label.xml"), Scenario::DeleteRows);
}

#[test]
fn point_label_delete_columns() {
    exercise("point_label", include_str!("fixtures/chart_reference_edits/point_label.xml"), Scenario::DeleteColumns);
}

#[test]
fn point_label_move_range() {
    exercise("point_label", include_str!("fixtures/chart_reference_edits/point_label.xml"), Scenario::MoveRange);
}

#[test]
fn group_point_label_rename() {
    exercise("group_point_label", include_str!("fixtures/chart_reference_edits/group_point_label.xml"), Scenario::RenameSheet);
}

#[test]
fn group_point_label_delete_sheet() {
    exercise("group_point_label", include_str!("fixtures/chart_reference_edits/group_point_label.xml"), Scenario::DeleteSheet);
}

#[test]
fn group_point_label_insert_rows() {
    exercise("group_point_label", include_str!("fixtures/chart_reference_edits/group_point_label.xml"), Scenario::InsertRows);
}

#[test]
fn group_point_label_insert_columns() {
    exercise("group_point_label", include_str!("fixtures/chart_reference_edits/group_point_label.xml"), Scenario::InsertColumns);
}

#[test]
fn group_point_label_delete_rows() {
    exercise("group_point_label", include_str!("fixtures/chart_reference_edits/group_point_label.xml"), Scenario::DeleteRows);
}

#[test]
fn group_point_label_delete_columns() {
    exercise("group_point_label", include_str!("fixtures/chart_reference_edits/group_point_label.xml"), Scenario::DeleteColumns);
}

#[test]
fn group_point_label_move_range() {
    exercise("group_point_label", include_str!("fixtures/chart_reference_edits/group_point_label.xml"), Scenario::MoveRange);
}

#[test]
fn error_plus_rename() {
    exercise("error_plus", include_str!("fixtures/chart_reference_edits/error_plus.xml"), Scenario::RenameSheet);
}

#[test]
fn error_plus_delete_sheet() {
    exercise("error_plus", include_str!("fixtures/chart_reference_edits/error_plus.xml"), Scenario::DeleteSheet);
}

#[test]
fn error_plus_insert_rows() {
    exercise("error_plus", include_str!("fixtures/chart_reference_edits/error_plus.xml"), Scenario::InsertRows);
}

#[test]
fn error_plus_insert_columns() {
    exercise("error_plus", include_str!("fixtures/chart_reference_edits/error_plus.xml"), Scenario::InsertColumns);
}

#[test]
fn error_plus_delete_rows() {
    exercise("error_plus", include_str!("fixtures/chart_reference_edits/error_plus.xml"), Scenario::DeleteRows);
}

#[test]
fn error_plus_delete_columns() {
    exercise("error_plus", include_str!("fixtures/chart_reference_edits/error_plus.xml"), Scenario::DeleteColumns);
}

#[test]
fn error_plus_move_range() {
    exercise("error_plus", include_str!("fixtures/chart_reference_edits/error_plus.xml"), Scenario::MoveRange);
}

#[test]
fn error_minus_rename() {
    exercise("error_minus", include_str!("fixtures/chart_reference_edits/error_minus.xml"), Scenario::RenameSheet);
}

#[test]
fn error_minus_delete_sheet() {
    exercise("error_minus", include_str!("fixtures/chart_reference_edits/error_minus.xml"), Scenario::DeleteSheet);
}

#[test]
fn error_minus_insert_rows() {
    exercise("error_minus", include_str!("fixtures/chart_reference_edits/error_minus.xml"), Scenario::InsertRows);
}

#[test]
fn error_minus_insert_columns() {
    exercise("error_minus", include_str!("fixtures/chart_reference_edits/error_minus.xml"), Scenario::InsertColumns);
}

#[test]
fn error_minus_delete_rows() {
    exercise("error_minus", include_str!("fixtures/chart_reference_edits/error_minus.xml"), Scenario::DeleteRows);
}

#[test]
fn error_minus_delete_columns() {
    exercise("error_minus", include_str!("fixtures/chart_reference_edits/error_minus.xml"), Scenario::DeleteColumns);
}

#[test]
fn error_minus_move_range() {
    exercise("error_minus", include_str!("fixtures/chart_reference_edits/error_minus.xml"), Scenario::MoveRange);
}

#[test]
fn display_unit_label_rename() {
    exercise("display_unit_label", include_str!("fixtures/chart_reference_edits/display_unit_label.xml"), Scenario::RenameSheet);
}

#[test]
fn display_unit_label_delete_sheet() {
    exercise("display_unit_label", include_str!("fixtures/chart_reference_edits/display_unit_label.xml"), Scenario::DeleteSheet);
}

#[test]
fn display_unit_label_insert_rows() {
    exercise("display_unit_label", include_str!("fixtures/chart_reference_edits/display_unit_label.xml"), Scenario::InsertRows);
}

#[test]
fn display_unit_label_insert_columns() {
    exercise("display_unit_label", include_str!("fixtures/chart_reference_edits/display_unit_label.xml"), Scenario::InsertColumns);
}

#[test]
fn display_unit_label_delete_rows() {
    exercise("display_unit_label", include_str!("fixtures/chart_reference_edits/display_unit_label.xml"), Scenario::DeleteRows);
}

#[test]
fn display_unit_label_delete_columns() {
    exercise("display_unit_label", include_str!("fixtures/chart_reference_edits/display_unit_label.xml"), Scenario::DeleteColumns);
}

#[test]
fn display_unit_label_move_range() {
    exercise("display_unit_label", include_str!("fixtures/chart_reference_edits/display_unit_label.xml"), Scenario::MoveRange);
}

#[test]
fn pivot_format_label_rename() {
    exercise("pivot_format_label", include_str!("fixtures/chart_reference_edits/pivot_format_label.xml"), Scenario::RenameSheet);
}

#[test]
fn pivot_format_label_delete_sheet() {
    exercise("pivot_format_label", include_str!("fixtures/chart_reference_edits/pivot_format_label.xml"), Scenario::DeleteSheet);
}

#[test]
fn pivot_format_label_insert_rows() {
    exercise("pivot_format_label", include_str!("fixtures/chart_reference_edits/pivot_format_label.xml"), Scenario::InsertRows);
}

#[test]
fn pivot_format_label_insert_columns() {
    exercise("pivot_format_label", include_str!("fixtures/chart_reference_edits/pivot_format_label.xml"), Scenario::InsertColumns);
}

#[test]
fn pivot_format_label_delete_rows() {
    exercise("pivot_format_label", include_str!("fixtures/chart_reference_edits/pivot_format_label.xml"), Scenario::DeleteRows);
}

#[test]
fn pivot_format_label_delete_columns() {
    exercise("pivot_format_label", include_str!("fixtures/chart_reference_edits/pivot_format_label.xml"), Scenario::DeleteColumns);
}

#[test]
fn pivot_format_label_move_range() {
    exercise("pivot_format_label", include_str!("fixtures/chart_reference_edits/pivot_format_label.xml"), Scenario::MoveRange);
}

#[test]
fn data_labels_range_rename() {
    exercise("data_labels_range", include_str!("fixtures/chart_reference_edits/data_labels_range.xml"), Scenario::RenameSheet);
}

#[test]
fn data_labels_range_delete_sheet() {
    exercise("data_labels_range", include_str!("fixtures/chart_reference_edits/data_labels_range.xml"), Scenario::DeleteSheet);
}

#[test]
fn data_labels_range_insert_rows() {
    exercise("data_labels_range", include_str!("fixtures/chart_reference_edits/data_labels_range.xml"), Scenario::InsertRows);
}

#[test]
fn data_labels_range_insert_columns() {
    exercise("data_labels_range", include_str!("fixtures/chart_reference_edits/data_labels_range.xml"), Scenario::InsertColumns);
}

#[test]
fn data_labels_range_delete_rows() {
    exercise("data_labels_range", include_str!("fixtures/chart_reference_edits/data_labels_range.xml"), Scenario::DeleteRows);
}

#[test]
fn data_labels_range_delete_columns() {
    exercise("data_labels_range", include_str!("fixtures/chart_reference_edits/data_labels_range.xml"), Scenario::DeleteColumns);
}

#[test]
fn data_labels_range_move_range() {
    exercise("data_labels_range", include_str!("fixtures/chart_reference_edits/data_labels_range.xml"), Scenario::MoveRange);
}

#[test]
fn field_table_entry_rename() {
    exercise("field_table_entry", include_str!("fixtures/chart_reference_edits/field_table_entry.xml"), Scenario::RenameSheet);
}

#[test]
fn field_table_entry_delete_sheet() {
    exercise("field_table_entry", include_str!("fixtures/chart_reference_edits/field_table_entry.xml"), Scenario::DeleteSheet);
}

#[test]
fn field_table_entry_insert_rows() {
    exercise("field_table_entry", include_str!("fixtures/chart_reference_edits/field_table_entry.xml"), Scenario::InsertRows);
}

#[test]
fn field_table_entry_insert_columns() {
    exercise("field_table_entry", include_str!("fixtures/chart_reference_edits/field_table_entry.xml"), Scenario::InsertColumns);
}

#[test]
fn field_table_entry_delete_rows() {
    exercise("field_table_entry", include_str!("fixtures/chart_reference_edits/field_table_entry.xml"), Scenario::DeleteRows);
}

#[test]
fn field_table_entry_delete_columns() {
    exercise("field_table_entry", include_str!("fixtures/chart_reference_edits/field_table_entry.xml"), Scenario::DeleteColumns);
}

#[test]
fn field_table_entry_move_range() {
    exercise("field_table_entry", include_str!("fixtures/chart_reference_edits/field_table_entry.xml"), Scenario::MoveRange);
}

#[test]
fn filtered_series_title_rename() {
    exercise("filtered_series_title", include_str!("fixtures/chart_reference_edits/filtered_series_title.xml"), Scenario::RenameSheet);
}

#[test]
fn filtered_series_title_delete_sheet() {
    exercise("filtered_series_title", include_str!("fixtures/chart_reference_edits/filtered_series_title.xml"), Scenario::DeleteSheet);
}

#[test]
fn filtered_series_title_insert_rows() {
    exercise("filtered_series_title", include_str!("fixtures/chart_reference_edits/filtered_series_title.xml"), Scenario::InsertRows);
}

#[test]
fn filtered_series_title_insert_columns() {
    exercise("filtered_series_title", include_str!("fixtures/chart_reference_edits/filtered_series_title.xml"), Scenario::InsertColumns);
}

#[test]
fn filtered_series_title_delete_rows() {
    exercise("filtered_series_title", include_str!("fixtures/chart_reference_edits/filtered_series_title.xml"), Scenario::DeleteRows);
}

#[test]
fn filtered_series_title_delete_columns() {
    exercise("filtered_series_title", include_str!("fixtures/chart_reference_edits/filtered_series_title.xml"), Scenario::DeleteColumns);
}

#[test]
fn filtered_series_title_move_range() {
    exercise("filtered_series_title", include_str!("fixtures/chart_reference_edits/filtered_series_title.xml"), Scenario::MoveRange);
}

#[test]
fn filtered_category_string_rename() {
    exercise("filtered_category_string", include_str!("fixtures/chart_reference_edits/filtered_category_string.xml"), Scenario::RenameSheet);
}

#[test]
fn filtered_category_string_delete_sheet() {
    exercise("filtered_category_string", include_str!("fixtures/chart_reference_edits/filtered_category_string.xml"), Scenario::DeleteSheet);
}

#[test]
fn filtered_category_string_insert_rows() {
    exercise("filtered_category_string", include_str!("fixtures/chart_reference_edits/filtered_category_string.xml"), Scenario::InsertRows);
}

#[test]
fn filtered_category_string_insert_columns() {
    exercise("filtered_category_string", include_str!("fixtures/chart_reference_edits/filtered_category_string.xml"), Scenario::InsertColumns);
}

#[test]
fn filtered_category_string_delete_rows() {
    exercise("filtered_category_string", include_str!("fixtures/chart_reference_edits/filtered_category_string.xml"), Scenario::DeleteRows);
}

#[test]
fn filtered_category_string_delete_columns() {
    exercise("filtered_category_string", include_str!("fixtures/chart_reference_edits/filtered_category_string.xml"), Scenario::DeleteColumns);
}

#[test]
fn filtered_category_string_move_range() {
    exercise("filtered_category_string", include_str!("fixtures/chart_reference_edits/filtered_category_string.xml"), Scenario::MoveRange);
}

#[test]
fn filtered_category_number_rename() {
    exercise("filtered_category_number", include_str!("fixtures/chart_reference_edits/filtered_category_number.xml"), Scenario::RenameSheet);
}

#[test]
fn filtered_category_number_delete_sheet() {
    exercise("filtered_category_number", include_str!("fixtures/chart_reference_edits/filtered_category_number.xml"), Scenario::DeleteSheet);
}

#[test]
fn filtered_category_number_insert_rows() {
    exercise("filtered_category_number", include_str!("fixtures/chart_reference_edits/filtered_category_number.xml"), Scenario::InsertRows);
}

#[test]
fn filtered_category_number_insert_columns() {
    exercise("filtered_category_number", include_str!("fixtures/chart_reference_edits/filtered_category_number.xml"), Scenario::InsertColumns);
}

#[test]
fn filtered_category_number_delete_rows() {
    exercise("filtered_category_number", include_str!("fixtures/chart_reference_edits/filtered_category_number.xml"), Scenario::DeleteRows);
}

#[test]
fn filtered_category_number_delete_columns() {
    exercise("filtered_category_number", include_str!("fixtures/chart_reference_edits/filtered_category_number.xml"), Scenario::DeleteColumns);
}

#[test]
fn filtered_category_number_move_range() {
    exercise("filtered_category_number", include_str!("fixtures/chart_reference_edits/filtered_category_number.xml"), Scenario::MoveRange);
}

#[test]
fn filtered_category_multilevel_rename() {
    exercise("filtered_category_multilevel", include_str!("fixtures/chart_reference_edits/filtered_category_multilevel.xml"), Scenario::RenameSheet);
}

#[test]
fn filtered_category_multilevel_delete_sheet() {
    exercise("filtered_category_multilevel", include_str!("fixtures/chart_reference_edits/filtered_category_multilevel.xml"), Scenario::DeleteSheet);
}

#[test]
fn filtered_category_multilevel_insert_rows() {
    exercise("filtered_category_multilevel", include_str!("fixtures/chart_reference_edits/filtered_category_multilevel.xml"), Scenario::InsertRows);
}

#[test]
fn filtered_category_multilevel_insert_columns() {
    exercise("filtered_category_multilevel", include_str!("fixtures/chart_reference_edits/filtered_category_multilevel.xml"), Scenario::InsertColumns);
}

#[test]
fn filtered_category_multilevel_delete_rows() {
    exercise("filtered_category_multilevel", include_str!("fixtures/chart_reference_edits/filtered_category_multilevel.xml"), Scenario::DeleteRows);
}

#[test]
fn filtered_category_multilevel_delete_columns() {
    exercise("filtered_category_multilevel", include_str!("fixtures/chart_reference_edits/filtered_category_multilevel.xml"), Scenario::DeleteColumns);
}

#[test]
fn filtered_category_multilevel_move_range() {
    exercise("filtered_category_multilevel", include_str!("fixtures/chart_reference_edits/filtered_category_multilevel.xml"), Scenario::MoveRange);
}

#[test]
fn extension_label_text_rename() {
    exercise("extension_label_text", include_str!("fixtures/chart_reference_edits/extension_label_text.xml"), Scenario::RenameSheet);
}

#[test]
fn extension_label_text_delete_sheet() {
    exercise("extension_label_text", include_str!("fixtures/chart_reference_edits/extension_label_text.xml"), Scenario::DeleteSheet);
}

#[test]
fn extension_label_text_insert_rows() {
    exercise("extension_label_text", include_str!("fixtures/chart_reference_edits/extension_label_text.xml"), Scenario::InsertRows);
}

#[test]
fn extension_label_text_insert_columns() {
    exercise("extension_label_text", include_str!("fixtures/chart_reference_edits/extension_label_text.xml"), Scenario::InsertColumns);
}

#[test]
fn extension_label_text_delete_rows() {
    exercise("extension_label_text", include_str!("fixtures/chart_reference_edits/extension_label_text.xml"), Scenario::DeleteRows);
}

#[test]
fn extension_label_text_delete_columns() {
    exercise("extension_label_text", include_str!("fixtures/chart_reference_edits/extension_label_text.xml"), Scenario::DeleteColumns);
}

#[test]
fn extension_label_text_move_range() {
    exercise("extension_label_text", include_str!("fixtures/chart_reference_edits/extension_label_text.xml"), Scenario::MoveRange);
}

#[test]
fn full_sqref_rename() {
    exercise("full_sqref", include_str!("fixtures/chart_reference_edits/full_sqref.xml"), Scenario::RenameSheet);
}

#[test]
fn full_sqref_delete_sheet() {
    exercise("full_sqref", include_str!("fixtures/chart_reference_edits/full_sqref.xml"), Scenario::DeleteSheet);
}

#[test]
fn full_sqref_insert_rows() {
    exercise("full_sqref", include_str!("fixtures/chart_reference_edits/full_sqref.xml"), Scenario::InsertRows);
}

#[test]
fn full_sqref_insert_columns() {
    exercise("full_sqref", include_str!("fixtures/chart_reference_edits/full_sqref.xml"), Scenario::InsertColumns);
}

#[test]
fn full_sqref_delete_rows() {
    exercise("full_sqref", include_str!("fixtures/chart_reference_edits/full_sqref.xml"), Scenario::DeleteRows);
}

#[test]
fn full_sqref_delete_columns() {
    exercise("full_sqref", include_str!("fixtures/chart_reference_edits/full_sqref.xml"), Scenario::DeleteColumns);
}

#[test]
fn full_sqref_move_range() {
    exercise("full_sqref", include_str!("fixtures/chart_reference_edits/full_sqref.xml"), Scenario::MoveRange);
}

#[test]
fn level_sqref_rename() {
    exercise("level_sqref", include_str!("fixtures/chart_reference_edits/level_sqref.xml"), Scenario::RenameSheet);
}

#[test]
fn level_sqref_delete_sheet() {
    exercise("level_sqref", include_str!("fixtures/chart_reference_edits/level_sqref.xml"), Scenario::DeleteSheet);
}

#[test]
fn level_sqref_insert_rows() {
    exercise("level_sqref", include_str!("fixtures/chart_reference_edits/level_sqref.xml"), Scenario::InsertRows);
}

#[test]
fn level_sqref_insert_columns() {
    exercise("level_sqref", include_str!("fixtures/chart_reference_edits/level_sqref.xml"), Scenario::InsertColumns);
}

#[test]
fn level_sqref_delete_rows() {
    exercise("level_sqref", include_str!("fixtures/chart_reference_edits/level_sqref.xml"), Scenario::DeleteRows);
}

#[test]
fn level_sqref_delete_columns() {
    exercise("level_sqref", include_str!("fixtures/chart_reference_edits/level_sqref.xml"), Scenario::DeleteColumns);
}

#[test]
fn level_sqref_move_range() {
    exercise("level_sqref", include_str!("fixtures/chart_reference_edits/level_sqref.xml"), Scenario::MoveRange);
}

#[test]
fn formula_sqref_rename() {
    exercise("formula_sqref", include_str!("fixtures/chart_reference_edits/formula_sqref.xml"), Scenario::RenameSheet);
}

#[test]
fn formula_sqref_delete_sheet() {
    exercise("formula_sqref", include_str!("fixtures/chart_reference_edits/formula_sqref.xml"), Scenario::DeleteSheet);
}

#[test]
fn formula_sqref_insert_rows() {
    exercise("formula_sqref", include_str!("fixtures/chart_reference_edits/formula_sqref.xml"), Scenario::InsertRows);
}

#[test]
fn formula_sqref_insert_columns() {
    exercise("formula_sqref", include_str!("fixtures/chart_reference_edits/formula_sqref.xml"), Scenario::InsertColumns);
}

#[test]
fn formula_sqref_delete_rows() {
    exercise("formula_sqref", include_str!("fixtures/chart_reference_edits/formula_sqref.xml"), Scenario::DeleteRows);
}

#[test]
fn formula_sqref_delete_columns() {
    exercise("formula_sqref", include_str!("fixtures/chart_reference_edits/formula_sqref.xml"), Scenario::DeleteColumns);
}

#[test]
fn formula_sqref_move_range() {
    exercise("formula_sqref", include_str!("fixtures/chart_reference_edits/formula_sqref.xml"), Scenario::MoveRange);
}

#[test]
fn filter_exception_sqref_rename() {
    exercise("filter_exception_sqref", include_str!("fixtures/chart_reference_edits/filter_exception_sqref.xml"), Scenario::RenameSheet);
}

#[test]
fn filter_exception_sqref_delete_sheet() {
    exercise("filter_exception_sqref", include_str!("fixtures/chart_reference_edits/filter_exception_sqref.xml"), Scenario::DeleteSheet);
}

#[test]
fn filter_exception_sqref_insert_rows() {
    exercise("filter_exception_sqref", include_str!("fixtures/chart_reference_edits/filter_exception_sqref.xml"), Scenario::InsertRows);
}

#[test]
fn filter_exception_sqref_insert_columns() {
    exercise("filter_exception_sqref", include_str!("fixtures/chart_reference_edits/filter_exception_sqref.xml"), Scenario::InsertColumns);
}

#[test]
fn filter_exception_sqref_delete_rows() {
    exercise("filter_exception_sqref", include_str!("fixtures/chart_reference_edits/filter_exception_sqref.xml"), Scenario::DeleteRows);
}

#[test]
fn filter_exception_sqref_delete_columns() {
    exercise("filter_exception_sqref", include_str!("fixtures/chart_reference_edits/filter_exception_sqref.xml"), Scenario::DeleteColumns);
}

#[test]
fn filter_exception_sqref_move_range() {
    exercise("filter_exception_sqref", include_str!("fixtures/chart_reference_edits/filter_exception_sqref.xml"), Scenario::MoveRange);
}

#[test]
fn filter_exception_label_rename() {
    exercise("filter_exception_label", include_str!("fixtures/chart_reference_edits/filter_exception_label.xml"), Scenario::RenameSheet);
}

#[test]
fn filter_exception_label_delete_sheet() {
    exercise("filter_exception_label", include_str!("fixtures/chart_reference_edits/filter_exception_label.xml"), Scenario::DeleteSheet);
}

#[test]
fn filter_exception_label_insert_rows() {
    exercise("filter_exception_label", include_str!("fixtures/chart_reference_edits/filter_exception_label.xml"), Scenario::InsertRows);
}

#[test]
fn filter_exception_label_insert_columns() {
    exercise("filter_exception_label", include_str!("fixtures/chart_reference_edits/filter_exception_label.xml"), Scenario::InsertColumns);
}

#[test]
fn filter_exception_label_delete_rows() {
    exercise("filter_exception_label", include_str!("fixtures/chart_reference_edits/filter_exception_label.xml"), Scenario::DeleteRows);
}

#[test]
fn filter_exception_label_delete_columns() {
    exercise("filter_exception_label", include_str!("fixtures/chart_reference_edits/filter_exception_label.xml"), Scenario::DeleteColumns);
}

#[test]
fn filter_exception_label_move_range() {
    exercise("filter_exception_label", include_str!("fixtures/chart_reference_edits/filter_exception_label.xml"), Scenario::MoveRange);
}
