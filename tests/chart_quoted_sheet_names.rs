#![cfg(feature = "serde")]

use std::io::Cursor;

use umya_spreadsheet::{
    Chart,
    ChartType,
    Workbook,
    drawing::charts::{
        ChartReferenceEdit,
        ChartText,
        ReferenceCell,
        ReferenceRect,
        StringReference,
        Title,
    },
    structs::DefinedName,
};

// Double quotes belong to the name; apostrophes delimit the qualifier.
const SHEETS: [(&str, &str, &str); 3] = [
    ("\"Data\"", "'\"Data\"'", "'\"Data\":Other'"),
    ("Data\"", "'Data\"'", "'Data\":Other'"),
    ("\"D'x\"", "'\"D''x\"'", "'\"D''x\":Other'"),
];

fn chart(input: &str) -> Chart {
    let mut chart = Chart::default();
    chart.new_chart(
        &ChartType::LineChart,
        Default::default(),
        Default::default(),
        vec![],
    );
    chart
        .two_cell_anchor_mut()
        .from_marker_mut()
        .set_coordinate("T20");
    chart
        .two_cell_anchor_mut()
        .to_marker_mut()
        .set_coordinate("AB40");
    let mut reference = StringReference::default();
    reference.formula_mut().set_string_value(input);
    let mut text = ChartText::default();
    text.set_string_reference(reference);
    let mut title = Title::default();
    title.set_chart_text(text);
    chart.chart_space_mut().chart_mut().set_title(title);
    chart
}

fn chart_formula(chart: &Chart) -> String {
    let mut chart = chart.clone();
    let formulas = chart.chart_space_mut().chart_mut().formula_mut();
    assert_eq!(formulas.len(), 1);
    formulas[0].address_str()
}

fn write(book: &Workbook) -> Vec<u8> {
    let mut bytes = Cursor::new(Vec::new());
    umya_spreadsheet::writer::xlsx::write_writer(book, &mut bytes).unwrap();
    bytes.into_inner()
}

fn load(bytes: &[u8], eager: bool) -> Workbook {
    umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(bytes), eager).unwrap()
}

fn input_bytes(input: &str) -> Vec<u8> {
    let mut book = umya_spreadsheet::new_file();
    book.set_sheet_name(0, "Host").unwrap();
    for name in [
        "Data",
        SHEETS[0].0,
        SHEETS[1].0,
        SHEETS[2].0,
        "Other",
        "Unrelated",
    ] {
        book.new_sheet(name)
            .unwrap()
            .cell_mut("B2")
            .set_value_number(17);
    }
    let host = book.sheet_mut(0).unwrap();
    host.add_chart(chart(input));
    host.cell_mut("A10").set_formula(input);
    let mut name = DefinedName::default();
    name.set_name("Source").set_address(input).set_hidden(true);
    book.add_defined_names(name);
    write(&book)
}

fn assert_hosts(book: &Workbook, expected: &str) {
    let host = book.sheet(0).unwrap();
    assert_eq!(host.chart_collection().len(), 1);
    assert_eq!(chart_formula(&host.chart_collection()[0]), expected);
    assert_eq!(host.cell("A10").unwrap().formula(), expected);
    let names: Vec<_> = book
        .defined_names()
        .iter()
        .chain(
            book.sheet_collection_no_check()
                .iter()
                .flat_map(|sheet| sheet.defined_names()),
        )
        .collect();
    assert_eq!(names.len(), 1);
    assert_eq!(names[0].name(), "Source");
    assert!(!names[0].has_local_sheet_id());
    assert!(names[0].hidden());
    assert_eq!(names[0].address(), expected);
}

fn apply(book: &mut Workbook, operation: &str, target: &str) {
    match operation {
        "rename" => {
            let index = book
                .sheet_collection_no_check()
                .iter()
                .position(|sheet| sheet.name() == target)
                .unwrap();
            book.set_sheet_name(index, "Renamed").unwrap();
        }
        "delete-sheet" => book.remove_sheet_by_name(target).unwrap(),
        "insert-row" => {
            book.insert_new_row(target, 1, 1);
        }
        "delete-row" => {
            book.remove_row(target, 1, 1);
        }
        "insert-column" => {
            book.insert_new_column_by_index(target, 2, 1);
        }
        "delete-column" => {
            book.remove_column_by_index(target, 1, 1);
        }
        "move" => {
            book.move_range(
                target,
                ReferenceRect {
                    first: ReferenceCell {
                        column: 2,
                        row:    2,
                    },
                    last:  ReferenceCell {
                        column: 2,
                        row:    3,
                    },
                },
                "Other",
                ReferenceCell {
                    column: 5,
                    row:    8,
                },
            )
            .unwrap();
        }
        _ => unreachable!(),
    }
}

#[test]
fn quoted_sheet_names_target_only_their_real_sheet_across_public_edits() {
    for (name, qualifier, _) in SHEETS {
        let input = format!("SUM({qualifier}!$B$2:$B$3)");
        let bytes = input_bytes(&input);
        assert_hosts(&load(&bytes, true), &input);
        for (operation, target_area) in [
            ("rename", "Renamed!$B$2:$B$3".to_owned()),
            ("delete-sheet", "#REF!".to_owned()),
            ("insert-row", format!("{qualifier}!$B$3:$B$4")),
            ("delete-row", format!("{qualifier}!$B$1:$B$2")),
            ("insert-column", format!("{qualifier}!$C$2:$C$3")),
            ("delete-column", format!("{qualifier}!$A$2:$A$3")),
            ("move", "Other!$E$8:$E$9".to_owned()),
        ] {
            for is_target in [false, true] {
                let expected = if is_target {
                    format!("SUM({target_area})")
                } else {
                    input.clone()
                };
                for eager in [true, false] {
                    let mut book = load(&bytes, eager);
                    apply(&mut book, operation, if is_target { name } else { "Data" });
                    if !eager {
                        let unrelated = book
                            .sheet_collection_no_check()
                            .iter()
                            .position(|sheet| sheet.name() == "Unrelated")
                            .unwrap();
                        assert!(
                            book.sheet(unrelated).is_err(),
                            "{name}, {operation}: unrelated sheet materialized"
                        );
                        assert_eq!(
                            book.sheet(0).is_ok(),
                            is_target,
                            "{name}, {operation}: wrong host discovery"
                        );
                    }
                    assert_hosts(&load(&write(&book), true), &expected);
                }
            }
        }
    }
}

#[test]
fn quoted_error_and_three_dimensional_endpoints_preserve_literal_quotes() {
    for (name, qualifier, endpoints) in SHEETS {
        for (input, expected) in [
            (format!("{qualifier}!#REF!"), "Renamed!#REF!"),
            (format!("{endpoints}!$B$2:$B$3"), "Renamed:Other!$B$2:$B$3"),
        ] {
            for is_target in [false, true] {
                let mut value = chart(&input);
                let report = value.adjust_references(
                    "Host",
                    &ChartReferenceEdit::RenameSheet {
                        old: if is_target { name } else { "Data" }.into(),
                        new: "Renamed".into(),
                    },
                );
                assert_eq!(
                    chart_formula(&value),
                    if is_target { expected } else { &input }
                );
                assert_eq!(report.visited, 1);
                assert_eq!(report.changed, usize::from(is_target));
                assert_eq!(report.invalidated, 0);
            }
        }
    }
}

#[test]
fn simple_quoted_sheet_addresses_preserve_identity_through_ingestion_and_codecs() {
    for (name, qualifier, _) in SHEETS {
        let input = format!("{qualifier}!$B$2:$B$3");
        let mut address = umya_spreadsheet::structs::Address::default();
        address.set_address(&input);
        assert_eq!(
            address.sheet_name(),
            qualifier
                .strip_prefix('\'')
                .unwrap()
                .strip_suffix('\'')
                .unwrap()
        );
        assert_eq!(address.range().range(), "$B$2:$B$3");
        let address: umya_spreadsheet::structs::Address =
            serde_json::from_slice(&serde_json::to_vec(&address).unwrap()).unwrap();
        let restored_address: umya_spreadsheet::structs::Address =
            bincode::deserialize(&bincode::serialize(&address).unwrap()).unwrap();
        assert_eq!(restored_address.sheet_name(), address.sheet_name());
        assert_eq!(restored_address.range().range(), "$B$2:$B$3");
        let mut formula = umya_spreadsheet::drawing::charts::Formula::default();
        formula.set_address_str(&input);
        assert_eq!(formula.address_str(), input);
        let formula: umya_spreadsheet::drawing::charts::Formula =
            serde_json::from_slice(&serde_json::to_vec(&formula).unwrap()).unwrap();
        let formula: umya_spreadsheet::drawing::charts::Formula =
            bincode::deserialize(&bincode::serialize(&formula).unwrap()).unwrap();
        assert_eq!(formula.address_str(), input);
        let mut defined = DefinedName::default();
        defined
            .set_name("Source")
            .set_address(&input)
            .set_hidden(true);
        assert_eq!(defined.address(), input);
        let defined: DefinedName =
            serde_json::from_slice(&serde_json::to_vec(&defined).unwrap()).unwrap();
        let defined: DefinedName =
            bincode::deserialize(&bincode::serialize(&defined).unwrap()).unwrap();
        assert_eq!(defined.name(), "Source");
        assert!(defined.hidden());
        assert_eq!(defined.address(), input);
        // Workbook I/O must retain the simple chart/cell/name target.
        let bytes = input_bytes(&input);
        for eager in [true, false] {
            let mut book = load(&bytes, eager);
            book.try_read_sheet(0).unwrap();
            assert_hosts(&book, &input);
            assert_hosts(&load(&write(&book), true), &input);
            assert!(
                book.sheet_collection_no_check()
                    .iter()
                    .any(|sheet| sheet.name() == name)
            );
        }
    }
}

#[test]
fn simple_chart_references_follow_renames_between_quote_distinct_sheet_names() {
    for (quoted_name, quoted_qualifier, plain_name, plain_qualifier) in [
        ("\"Data\"", "'\"Data\"'", "Data", "Data"),
        ("Data\"", "'Data\"'", "Data", "Data"),
        ("\"D'x\"", "'\"D''x\"'", "D'x", "'D''x'"),
    ] {
        for reverse in [false, true] {
            let (old, old_qualifier, new, new_qualifier) = if reverse {
                (plain_name, plain_qualifier, quoted_name, quoted_qualifier)
            } else {
                (quoted_name, quoted_qualifier, plain_name, plain_qualifier)
            };
            let input = format!("{old_qualifier}!$B$2:$B$3");
            let expected = format!("{new_qualifier}!$B$2:$B$3");
            let mut address_chart = chart(&input);
            let mut formulas = address_chart.chart_space_mut().chart_mut().formula_mut();
            assert_eq!(formulas.len(), 1);
            formulas[0].set_address_str(&input);
            assert!(!formulas[0].address().sheet_name().is_empty());
            assert_eq!(formulas[0].address().range().range(), "$B$2:$B$3");
            for codec in ["native", "json", "binary"] {
                let stored_chart: Chart = match codec {
                    "native" => address_chart.clone(),
                    "json" => serde_json::from_slice(&serde_json::to_vec(&address_chart).unwrap())
                        .unwrap(),
                    "binary" => {
                        bincode::deserialize(&bincode::serialize(&address_chart).unwrap()).unwrap()
                    }
                    _ => unreachable!(),
                };
                assert_eq!(chart_formula(&stored_chart), input);
                let mut source = umya_spreadsheet::new_file();
                source.set_sheet_name(0, "Host").unwrap();
                source
                    .new_sheet(old)
                    .unwrap()
                    .cell_mut("B2")
                    .set_value_number(17);
                source
                    .new_sheet("Unrelated")
                    .unwrap()
                    .cell_mut("A1")
                    .set_value("keep");
                source.sheet_mut(0).unwrap().add_chart(stored_chart);
                source
                    .sheet_mut(0)
                    .unwrap()
                    .cell_mut("A10")
                    .set_formula(&input);
                let mut name = DefinedName::default();
                name.set_name("Source").set_address(&input).set_hidden(true);
                source.add_defined_names(name);
                let bytes = write(&source);
                for eager in [true, false] {
                    let mut book = load(&bytes, eager);
                    book.set_sheet_name(1, new).unwrap();
                    assert_eq!(book.sheet_collection_no_check()[1].name(), new);
                    assert_hosts(&book, &expected);
                    if !eager {
                        assert!(book.sheet(2).is_err(), "unrelated sheet was materialized");
                    }
                    let reread = load(&write(&book), true);
                    assert_hosts(&reread, &expected);
                    assert_eq!(reread.sheet_collection()[1].name(), new);
                    assert_eq!(
                        reread.sheet(1).unwrap().cell("B2").unwrap().value_number(),
                        Some(17.0)
                    );
                    assert_eq!(reread.sheet(2).unwrap().cell("A1").unwrap().value(), "keep");
                }
            }
        }
    }
}

#[test]
fn qualified_lookup_distinguishes_literal_quotes_from_the_plain_sheet() {
    for (quoted, qualifier, plain, plain_qualifier) in [
        ("\"Data\"", "'\"Data\"'", "Data", "Data"),
        ("Data\"", "'Data\"'", "Data", "Data"),
        ("\"D'x\"", "'\"D''x\"'", "D'x", "'D''x'"),
    ] {
        let mut source = umya_spreadsheet::new_file();
        source.set_sheet_name(0, "Host").unwrap();
        source
            .new_sheet(plain)
            .unwrap()
            .cell_mut("B2")
            .set_value_number(11);
        source
            .new_sheet(quoted)
            .unwrap()
            .cell_mut("B2")
            .set_value_number(21);
        source.new_sheet("Unrelated").unwrap();
        let bytes = write(&source);
        for eager in [true, false] {
            let mut book = load(&bytes, eager);
            book.try_read_sheet(1).unwrap().try_read_sheet(2).unwrap();
            for (input, expected) in [
                (format!("{qualifier}!B2"), 21.0),
                (format!("{plain_qualifier}!B2"), 11.0),
            ] {
                let values = book.cell_value_by_address(&input);
                assert_eq!(values.len(), 1);
                assert_eq!(
                    values[0].value_number(),
                    Some(expected),
                    "{input}, eager={eager}"
                );
            }
            if !eager {
                assert!(
                    book.sheet(3).is_err(),
                    "lookup must not load unrelated sheets"
                );
            }
        }
    }
}

#[test]
fn coordinate_rebasing_preserves_literal_quote_identity_and_existing_qualifiers() {
    for (_, qualifier) in [
        ("\"Data\"", "'\"Data\"'"),
        ("Data\"", "'Data\"'"),
        ("\"D'x\"", "'\"D''x\"'"),
        ("Bob's Data", "'Bob''s Data'"),
        ("ÄSheet", "'ÄSheet'"),
        ("[Book.xlsx]Data", "'[Book.xlsx]Data'"),
    ] {
        let input = format!("{qualifier}!B2+{qualifier}!$C$3");
        let expected = format!("{qualifier}!B3+{qualifier}!$C$3");
        let mut cell = umya_spreadsheet::Cell::default();
        cell.set_coordinate("A1")
            .set_formula(&input)
            .set_coordinate("A2");
        assert_eq!(cell.formula(), expected);
    }
}

fn with_shared_formula(bytes: &[u8], master: &str) -> Vec<u8> {
    use std::io::{
        Read,
        Write,
    };
    let master = quick_xml::escape::escape(master);
    let sheet = format!(
        concat!(
            r#"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData>"#,
            r#"<row r="1"><c r="A1"><f t="shared" si="7" ref="A1:A2">{master}</f><v>1</v></c></row>"#,
            r#"<row r="2"><c r="A2"><f t="shared" si="7"/><v>2</v></c></row>"#,
            "</sheetData></worksheet>"
        ),
        master = master,
    );
    let mut input = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let mut replaced = 0;
    for index in 0..input.len() {
        let mut entry = input.by_index(index).unwrap();
        let mut data = Vec::new();
        entry.read_to_end(&mut data).unwrap();
        if entry.name().unwrap() == "xl/worksheets/sheet1.xml" {
            data = sheet.as_bytes().to_vec();
            replaced += 1;
        }
        output
            .start_file(
                entry.name().unwrap(),
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
        output.write_all(&data).unwrap();
    }
    assert_eq!(replaced, 1);
    output.finish().unwrap().into_inner()
}

#[test]
fn shared_formula_expansion_keeps_literal_quotes_and_absolute_reference_metadata() {
    for (_, qualifier, _) in SHEETS {
        let master = format!("{qualifier}!B2+{qualifier}!$C$3");
        let follower = format!("{qualifier}!B3+{qualifier}!$C$3");
        let bytes = with_shared_formula(&input_bytes("1"), &master);
        for eager in [true, false] {
            let mut book = load(&bytes, eager);
            book.try_read_sheet(0).unwrap();
            for (coordinate, expected, value) in
                [("A1", master.as_str(), 1.0), ("A2", follower.as_str(), 2.0)]
            {
                let cell = book.sheet(0).unwrap().cell(coordinate).unwrap();
                assert_eq!(cell.formula(), expected);
                assert_eq!(cell.formula_shared_index(), Some(7));
                assert_eq!(cell.value_number(), Some(value));
            }
            if !eager {
                assert!(
                    book.sheet(6).is_err(),
                    "shared expansion must not load unrelated sheets"
                );
            }
            let reread = load(&write(&book), true);
            assert_eq!(
                reread.sheet(0).unwrap().cell("A2").unwrap().formula(),
                follower
            );
            assert_eq!(
                reread
                    .sheet(0)
                    .unwrap()
                    .cell("A2")
                    .unwrap()
                    .formula_shared_index(),
                Some(7)
            );
        }
    }
}
