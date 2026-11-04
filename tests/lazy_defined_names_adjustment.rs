use std::io::Cursor;

use umya_spreadsheet::{
    Workbook,
    drawing::charts::{
        ReferenceCell,
        ReferenceRect,
    },
    structs::DefinedName,
};

fn input() -> Vec<u8> {
    let mut book = umya_spreadsheet::new_file();
    book.set_sheet_name(0, "Data").unwrap();
    for name in ["Other", "Unrelated", "Destination"] {
        book.new_sheet(name).unwrap();
    }
    book.sheet_mut(0)
        .unwrap()
        .cell_mut("B2")
        .set_value_number(7);
    book.sheet_mut(2)
        .unwrap()
        .cell_mut("A1")
        .set_value("untouched raw sheet");
    for (label, address) in [("RemoteRange", "Data!$B$2:$D$5"), ("OwnRange", "$C$2:$D$5")] {
        let mut name = DefinedName::default();
        name.set_name(label).set_address(address);
        name.set_local_sheet_id(1);
        book.sheet_mut(1).unwrap().add_defined_names(name);
    }
    let mut global = DefinedName::default();
    global
        .set_name("GlobalSum")
        .set_address("SUM(Data!$B$2:$D$5)");
    book.add_defined_names(global);
    let mut bytes = Vec::new();
    umya_spreadsheet::writer::xlsx::write_writer(&book, &mut bytes).unwrap();
    bytes
}

fn names(book: &Workbook) -> Vec<(String, Vec<(String, String, u32)>)> {
    book.sheet_collection_no_check()
        .iter()
        .map(|sheet| {
            (
                sheet.name().to_owned(),
                sheet
                    .defined_names()
                    .iter()
                    .map(|name| {
                        (
                            name.name().to_owned(),
                            name.address(),
                            name.local_sheet_id(),
                        )
                    })
                    .collect(),
            )
        })
        .collect()
}

fn assert_edit(edit: impl Fn(&mut Workbook), expected: &str) {
    let bytes = input();
    let mut eager = umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(&bytes), true).unwrap();
    let mut lazy = umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(&bytes), false).unwrap();
    for index in 0..4 {
        assert!(
            lazy.sheet(index).is_err(),
            "fixture must start entirely raw"
        );
    }
    edit(&mut eager);
    edit(&mut lazy);
    assert_eq!(names(&lazy), names(&eager));
    assert_eq!(
        lazy.defined_names()[0].address(),
        eager.defined_names()[0].address()
    );
    let index_of = |name| {
        lazy.sheet_collection_no_check()
            .iter()
            .position(|sheet| sheet.name() == name)
            .unwrap()
    };
    let other = index_of("Other");
    let unrelated = index_of("Unrelated");
    assert!(lazy.sheet(other).is_err(), "names-only host must stay raw");
    assert!(
        lazy.sheet(unrelated).is_err(),
        "unrelated host must stay raw"
    );
    let local_names = lazy.sheet_collection_no_check()[other].defined_names();
    assert_eq!(local_names[0].address(), expected);
    assert_eq!(local_names[1].address(), "$C$2:$D$5");

    let mut output = Vec::new();
    umya_spreadsheet::writer::xlsx::write_writer(&lazy, &mut output).unwrap();
    assert!(
        lazy.sheet(other).is_err(),
        "raw export must not materialize names-only host"
    );
    assert!(
        lazy.sheet(unrelated).is_err(),
        "raw export must not materialize unrelated host"
    );
    let reread = umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(output), true).unwrap();
    assert_eq!(names(&reread), names(&eager));
    assert_eq!(
        reread
            .sheet_by_name("Unrelated")
            .unwrap()
            .cell("A1")
            .unwrap()
            .value(),
        "untouched raw sheet"
    );
}

#[test]
fn lazy_scoped_names_follow_sheet_rename_without_opening_their_host() {
    assert_edit(
        |book| book.set_sheet_name(0, "Bob's Data").unwrap(),
        "'Bob''s Data'!$B$2:$D$5",
    );
}

#[test]
fn lazy_scoped_names_follow_sheet_deletion_without_opening_their_host() {
    assert_edit(|book| book.remove_sheet(0).unwrap(), "#REF!");
}

#[test]
fn lazy_scoped_names_follow_row_insertion_once_without_opening_their_host() {
    assert_edit(|book| book.insert_new_row("Data", 4, 1), "Data!$B$2:$D$6");
}

#[test]
fn lazy_scoped_names_follow_row_removal_once_without_opening_their_host() {
    assert_edit(|book| book.remove_row("Data", 3, 1), "Data!$B$2:$D$4");
}

#[test]
fn lazy_scoped_names_follow_column_insertion_once_without_opening_their_host() {
    assert_edit(
        |book| book.insert_new_column("Data", "C", 1),
        "Data!$B$2:$E$5",
    );
}

#[test]
fn lazy_scoped_names_follow_column_removal_once_without_opening_their_host() {
    assert_edit(|book| book.remove_column("Data", "C", 1), "Data!$B$2:$C$5");
}

fn move_range(book: &mut Workbook, destination: &str) {
    book.move_range(
        "Data",
        ReferenceRect {
            first: ReferenceCell {
                column: 2,
                row:    2,
            },
            last:  ReferenceCell {
                column: 4,
                row:    5,
            },
        },
        destination,
        ReferenceCell {
            column: 5,
            row:    8,
        },
    )
    .unwrap();
}

#[test]
fn lazy_scoped_names_follow_same_sheet_move_without_opening_their_host() {
    assert_edit(|book| move_range(book, "Data"), "Data!$E$8:$G$11");
}

#[test]
fn lazy_scoped_names_follow_cross_sheet_move_without_opening_their_host() {
    assert_edit(
        |book| move_range(book, "Destination"),
        "Destination!$E$8:$G$11",
    );
}
