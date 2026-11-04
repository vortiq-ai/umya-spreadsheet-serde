//! Authored reference grammar regressions, not additional native Excel
//! captures.
#![cfg(feature = "serde")]

use std::io::Cursor;

use umya_spreadsheet::{
    Chart,
    Workbook,
    drawing::charts::{
        ChartReferenceEdit,
        ReferenceCell,
        ReferenceRect,
    },
    structs::DefinedName,
};

#[path = "support/chart_xml_fixture.rs"]
pub mod fixture;

#[derive(Clone, Copy, Debug)]
enum Role {
    Values,
    Categories,
    SeriesName,
    LabelRange,
}
const ROLES: [Role; 4] = [
    Role::Values,
    Role::Categories,
    Role::SeriesName,
    Role::LabelRange,
];

fn chart(input: &str, role: Role) -> Chart {
    let input = quick_xml::escape::escape(input);
    let child = match role {
        Role::Values => format!("<c:val><c:numRef><c:f>{input}</c:f></c:numRef></c:val>"),
        Role::Categories => format!("<c:cat><c:strRef><c:f>{input}</c:f></c:strRef></c:cat>"),
        Role::SeriesName => format!("<c:tx><c:strRef><c:f>{input}</c:f></c:strRef></c:tx>"),
        Role::LabelRange => format!(
            "<c:extLst><c:ext \
             uri=\"{{02D57815-91ED-43cb-92C2-25804820EDAC}}\"><c15:datalabelsRange><c15:\
             f>{input}</c15:f><c15:dlblRangeCache><c:ptCount \
             val=\"0\"/></c15:dlblRangeCache></c15:datalabelsRange></c:ext></c:extLst>"
        ),
    };
    let mut chart = fixture::imported_chart(&format!(
        "<c:chartSpace xmlns:c=\"http://schemas.openxmlformats.org/drawingml/2006/chart\" xmlns:c15=\"http://schemas.microsoft.com/office/drawing/2012/chart\"><c:chart><c:plotArea><c:lineChart><c:grouping val=\"standard\"/><c:ser><c:idx val=\"1\"/><c:order val=\"1\"/>{child}</c:ser><c:axId val=\"1\"/><c:axId val=\"2\"/></c:lineChart><c:catAx><c:axId val=\"1\"/><c:scaling><c:orientation val=\"minMax\"/></c:scaling><c:axPos val=\"b\"/><c:crossAx val=\"2\"/></c:catAx><c:valAx><c:axId val=\"2\"/><c:scaling><c:orientation val=\"minMax\"/></c:scaling><c:axPos val=\"l\"/><c:crossAx val=\"1\"/></c:valAx></c:plotArea></c:chart></c:chartSpace>"
    ));
    // Geometry deletion is independent of reference editing. Keep this fixture
    // outside the edited row/column ranges even when Data also hosts the chart.
    chart
        .two_cell_anchor_mut()
        .from_marker_mut()
        .set_coordinate("T20");
    chart
        .two_cell_anchor_mut()
        .to_marker_mut()
        .set_coordinate("AB40");
    chart
}

fn formula(chart: &Chart) -> String {
    let mut copy = chart.clone();
    let formulas = copy.chart_space_mut().chart_mut().formula_mut();
    assert_eq!(formulas.len(), 1);
    formulas[0].address_str()
}

fn edits() -> Vec<(ChartReferenceEdit, &'static str)> {
    vec![
        (
            ChartReferenceEdit::RenameSheet {
                old: "Data".into(),
                new: "Bob's Data".into(),
            },
            "'Bob''s Data'!$B$2:$B$3,'Bob''s Data'!$B$5:$B$6",
        ),
        (
            ChartReferenceEdit::DeleteSheet {
                sheet:       "Data".into(),
                sheet_order: vec![
                    "Host".into(),
                    "Data".into(),
                    "Other".into(),
                    "Unrelated".into(),
                ],
            },
            "#REF!,#REF!",
        ),
        (
            ChartReferenceEdit::InsertRows {
                sheet: "Data".into(),
                at:    2,
                count: 1,
            },
            "Data!$B$3:$B$4,Data!$B$6:$B$7",
        ),
        (
            ChartReferenceEdit::DeleteRows {
                sheet: "Data".into(),
                rows:  vec![1],
            },
            "Data!$B$1:$B$2,Data!$B$4:$B$5",
        ),
        (
            ChartReferenceEdit::InsertColumns {
                sheet: "Data".into(),
                at:    2,
                count: 1,
            },
            "Data!$C$2:$C$3,Data!$C$5:$C$6",
        ),
        (
            ChartReferenceEdit::DeleteColumns {
                sheet:   "Data".into(),
                columns: vec![1],
            },
            "Data!$A$2:$A$3,Data!$A$5:$A$6",
        ),
        (
            ChartReferenceEdit::MoveRange {
                source_sheet:      "Data".into(),
                source:            ReferenceRect {
                    first: ReferenceCell {
                        column: 2,
                        row:    2,
                    },
                    last:  ReferenceCell {
                        column: 2,
                        row:    6,
                    },
                },
                destination_sheet: "Other".into(),
                destination:       ReferenceCell {
                    column: 5,
                    row:    8,
                },
            },
            "Other!$E$8:$E$9,Other!$E$11:$E$12",
        ),
    ]
}

fn apply(book: &mut Workbook, edit: &ChartReferenceEdit) {
    match edit {
        ChartReferenceEdit::RenameSheet { old, new } => {
            let index = book
                .sheet_collection_no_check()
                .iter()
                .position(|sheet| sheet.name() == old)
                .unwrap();
            book.set_sheet_name(index, new).unwrap();
        }
        ChartReferenceEdit::DeleteSheet { sheet, .. } => book.remove_sheet_by_name(sheet).unwrap(),
        ChartReferenceEdit::InsertRows { sheet, at, count } => {
            book.insert_new_row(sheet, *at, *count);
        }
        ChartReferenceEdit::DeleteRows { sheet, rows } => {
            for row in rows {
                book.remove_row(sheet, *row, 1);
            }
        }
        ChartReferenceEdit::InsertColumns { sheet, at, count } => {
            book.insert_new_column_by_index(sheet, *at, *count);
        }
        ChartReferenceEdit::DeleteColumns { sheet, columns } => {
            for column in columns {
                book.remove_column_by_index(sheet, *column, 1);
            }
        }
        ChartReferenceEdit::MoveRange {
            source_sheet,
            source,
            destination_sheet,
            destination,
        } => {
            book.move_range(source_sheet, *source, destination_sheet, *destination)
                .unwrap();
        }
    }
}

fn input_bytes(input: &str, role: Role, same_host: bool) -> Vec<u8> {
    let mut book = umya_spreadsheet::new_file();
    book.set_sheet_name(0, "Host").unwrap();
    for name in ["Data", "Other", "Unrelated"] {
        book.new_sheet(name).unwrap();
    }
    book.sheet_mut(3)
        .unwrap()
        .cell_mut("A1")
        .set_value("unrelated");
    let mut name = DefinedName::default();
    name.set_name("Sales")
        .set_address("Data!$B$2:$B$5")
        .set_local_sheet_id(1);
    book.sheet_mut(1).unwrap().add_defined_names(name);
    book.sheet_mut(usize::from(same_host))
        .unwrap()
        .add_chart(chart(input, role));
    fixture::write_workbook(&book)
}

fn assert_workbook(
    input: &str,
    expected: &str,
    role: Role,
    edit: &ChartReferenceEdit,
    same_host: bool,
    host_must_open: bool,
) {
    let bytes = input_bytes(input, role, same_host);
    for eager in [true, false] {
        let mut book =
            umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(&bytes), eager).unwrap();
        apply(&mut book, edit);
        if !eager {
            let unrelated = book
                .sheet_collection_no_check()
                .iter()
                .position(|sheet| sheet.name() == "Unrelated")
                .unwrap();
            assert!(
                book.sheet(unrelated).is_err(),
                "unrelated raw sheet was materialized"
            );
            if !same_host {
                assert_eq!(book.sheet(0).is_ok(), host_must_open, "{input}: {edit:?}");
            }
        }
        let exported = fixture::write_workbook(&book);
        let reread =
            umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(exported), true).unwrap();
        let found = reread
            .sheet_collection()
            .iter()
            .flat_map(|sheet| sheet.chart_collection())
            .next()
            .unwrap();
        assert_eq!(
            formula(found),
            expected,
            "{role:?}: {input}, {edit:?}, eager={eager}"
        );
    }
}

#[test]
fn top_level_and_parenthesized_unions_follow_every_public_edit_and_role() {
    for role in ROLES {
        for wrapped in [false, true] {
            let input = if wrapped {
                "(Data!$B$2:$B$3,Data!$B$5:$B$6)"
            } else {
                "Data!$B$2:$B$3,Data!$B$5:$B$6"
            };
            for (edit, expected) in edits() {
                let expected = if matches!(edit, ChartReferenceEdit::DeleteSheet { .. })
                    && matches!(role, Role::LabelRange)
                {
                    "#REF!".to_owned()
                } else if wrapped {
                    format!("({expected})")
                } else {
                    expected.to_owned()
                };
                let mut value = chart(input, role);
                value.adjust_references("Host", &edit);
                assert_eq!(formula(&value), expected, "{role:?} {edit:?}");
                assert_workbook(input, &expected, role, &edit, false, true);
            }
        }
    }
}

#[test]
fn lazy_discovery_finds_a_dependency_only_in_the_second_union_member() {
    let edit = ChartReferenceEdit::RenameSheet {
        old: "Data".into(),
        new: "Bob's Data".into(),
    };
    for role in ROLES {
        assert_workbook(
            "Other!$B$2:$B$3,Data!$B$5:$B$6",
            "Other!$B$2:$B$3,'Bob''s Data'!$B$5:$B$6",
            role,
            &edit,
            false,
            true,
        );
    }
}

#[test]
fn scoped_names_follow_sheet_rename_in_every_role() {
    for role in ROLES {
        for eager_same_host in [false, true] {
            let rename = ChartReferenceEdit::RenameSheet {
                old: "Data".into(),
                new: "Bob's Data".into(),
            };
            assert_workbook(
                "Data!Sales",
                "'Bob''s Data'!Sales",
                role,
                &rename,
                eager_same_host,
                true,
            );
        }
    }
}

#[test]
fn scoped_name_text_stays_named_through_coordinate_edits_without_opening_a_raw_chart_host() {
    for role in ROLES {
        for (edit, _) in edits().into_iter().skip(2) {
            assert_workbook("Data!Sales", "Data!Sales", role, &edit, false, false);
            assert_workbook("Data!Sales", "Data!Sales", role, &edit, true, true);
        }
    }
}

#[test]
fn quoted_scope_rename_preserves_apostrophes_and_named_identity() {
    let mut value = chart("'Bob''s Data'!Sales", Role::Values);
    value.adjust_references(
        "Host",
        &ChartReferenceEdit::RenameSheet {
            old: "Bob's Data".into(),
            new: "Renamed".into(),
        },
    );
    assert_eq!(formula(&value), "Renamed!Sales");
}

#[test]
fn external_named_references_are_not_adopted_as_local_scopes() {
    for input in ["'[Book.xlsx]Data'!Sales", "[0]!Sales", "[12]Data!Sales"] {
        let mut value = chart(input, Role::Values);
        value.adjust_references(
            "Host",
            &ChartReferenceEdit::RenameSheet {
                old: "Data".into(),
                new: "Renamed".into(),
            },
        );
        assert_eq!(formula(&value), input);
    }
}

fn assert_named_delete(collision: bool, quoted: bool, local_name: &str, global_name: &str) {
    // Excel promotes deleted local names; an existing global wins collisions.
    for role in ROLES {
        let bytes = input_bytes(&format!("Data!{local_name}"), role, false);
        let mut source =
            umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap();
        source.sheet_mut(1).unwrap().defined_names_mut()[0]
            .set_name(local_name)
            .set_hidden(true);
        if quoted {
            source.set_sheet_name(1, "Bob's Data").unwrap();
            source.sheet_mut(0).unwrap().chart_collection_mut()[0] =
                chart(&format!("'Bob''s Data'!{local_name}"), role);
        }
        if collision {
            source
                .sheet_mut(0)
                .unwrap()
                .cell_mut("D1")
                .set_value_number(999);
            let mut global = DefinedName::default();
            global.set_name(global_name).set_address("Host!$D$1");
            source.add_defined_names(global);
        }
        let cell_reference = if quoted {
            format!("'Bob''s Data'!{local_name}")
        } else {
            format!("Data!{local_name}")
        };
        source
            .sheet_mut(0)
            .unwrap()
            .cell_mut("A10")
            .set_formula(cell_reference);
        let mut later = DefinedName::default();
        later
            .set_name("Later")
            .set_address("Other!$A$1")
            .set_local_sheet_id(2);
        source.sheet_mut(2).unwrap().add_defined_names(later);
        let bytes = fixture::write_workbook(&source);
        for eager in [true, false] {
            let mut book =
                umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(&bytes), eager).unwrap();
            book.remove_sheet_by_name(if quoted { "Bob's Data" } else { "Data" })
                .unwrap();
            if !eager {
                assert!(
                    book.sheet(0).is_ok(),
                    "dependent chart host must be discovered"
                );
                assert!(
                    book.sheet(2).is_err(),
                    "unrelated worksheet must remain raw"
                );
            }
            let exported = fixture::write_workbook(&book);
            let reread =
                umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(exported), true).unwrap();
            assert_eq!(
                formula(&reread.sheet(0).unwrap().chart_collection()[0]),
                format!("[0]!{local_name}"),
                "{role:?}, collision={collision}, quoted={quoted}, eager={eager}"
            );
            let names: Vec<_> = reread
                .defined_names()
                .iter()
                .chain(
                    reread
                        .sheet_collection()
                        .iter()
                        .flat_map(|sheet| sheet.defined_names()),
                )
                .collect();
            assert_eq!(names.len(), 2);
            assert_eq!(
                names
                    .iter()
                    .filter(|name| !name.has_local_sheet_id())
                    .count(),
                1
            );
            let sales: Vec<_> = names
                .iter()
                .filter(|name| match local_name {
                    "Sales" => name.name().eq_ignore_ascii_case("Sales"),
                    "ärger" => matches!(name.name(), "ärger" | "Ärger"),
                    _ => unreachable!(),
                })
                .collect();
            assert_eq!(
                sales.len(),
                1,
                "deleted local must not coexist with its fallback"
            );
            assert_eq!(
                sales[0].name(),
                if collision { global_name } else { local_name }
            );
            assert_eq!(
                reread.sheet(0).unwrap().cell("A10").unwrap().formula(),
                format!("[0]!{local_name}")
            );
            assert!(
                !sales[0].has_local_sheet_id(),
                "fallback must be workbook-scoped"
            );
            assert_eq!(
                sales[0].hidden(),
                !collision,
                "preserve the winning name's metadata"
            );
            assert_eq!(
                sales[0].address(),
                if collision { "Host!$D$1" } else { "#REF!" }
            );
            let later: Vec<_> = names.iter().filter(|name| name.name() == "Later").collect();
            assert_eq!(later.len(), 1);
            assert!(later[0].has_local_sheet_id());
            assert_eq!(later[0].local_sheet_id(), 1);
            assert_eq!(later[0].address(), "Other!$A$1");
            if collision {
                assert_eq!(
                    reread.sheet(0).unwrap().cell("D1").unwrap().value_number(),
                    Some(999.0)
                );
            }
        }
    }
}

#[test]
fn deleted_local_name_promotes_to_current_workbook_ref_tombstone() {
    assert_named_delete(false, false, "Sales", "Sales");
}

#[test]
fn deleted_local_name_uses_existing_global_name_on_collision() {
    assert_named_delete(true, false, "Sales", "Sales");
}

#[test]
fn deleted_quoted_local_scope_uses_the_same_excel_fallback() {
    assert_named_delete(false, true, "Sales", "Sales");
    assert_named_delete(true, true, "Sales", "Sales");
}

#[test]
fn deleted_local_name_uses_existing_non_ascii_case_variant_global() {
    assert_named_delete(true, false, "ärger", "Ärger");
    assert_named_delete(true, true, "ärger", "Ärger");
}
