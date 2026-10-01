//! Round-trip tests for the `customWidth` column attribute.
//!
//! `customWidth` tells Excel whether a column's width was AUTHORED or merely
//! inherited from the sheet default. Excel autofits an inherited column and
//! leaves an authored one alone, so the flag decides layout behaviour, not just
//! a byte in the XML.
//!
//! The writer used to stamp `customWidth="1"` on every column unconditionally.
//! Once the reader began preserving the attribute, open-and-save started
//! rewriting every inherited width as authored -- changing the layout of
//! columns the user never touched. Nothing in the suite covered the attribute,
//! on either side, which is why the asymmetry survived.

extern crate umya_spreadsheet;
extern crate zip;

use std::io::{
    Cursor,
    Read,
};

use umya_spreadsheet::{
    Workbook,
    new_file,
    reader,
    writer,
};

fn to_bytes(book: &Workbook) -> Vec<u8> {
    let mut out = Vec::new();
    writer::xlsx::write_writer(book, &mut out).expect("write xlsx");
    out
}

/// The raw `sheet1.xml`. Asserting on the bytes is the point: the reader could
/// agree with the writer about a wrong value and a round-trip-only test would
/// never notice.
fn sheet1_xml(bytes: &[u8]) -> String {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes.to_vec())).expect("valid zip");
    let mut f = zip
        .by_name("xl/worksheets/sheet1.xml")
        .expect("sheet1.xml is present");
    let mut s = String::new();
    f.read_to_string(&mut s).expect("sheet1.xml is utf-8");
    s
}

/// Every `<col .../>` element in the sheet, as raw substrings.
fn col_elements(xml: &str) -> Vec<String> {
    xml.match_indices("<col ")
        .map(|(i, _)| {
            let rest = &xml[i..];
            let end = rest.find("/>").map_or(rest.len(), |e| e + 2);
            rest[..end].to_string()
        })
        .collect()
}

#[test]
fn set_width_marks_the_width_as_authored() {
    // Mirrors `Row::set_height`, which has always set `custom_height`. A caller
    // that sets a width means it; without this the conditional writer would
    // emit the width and then tell Excel it was free to autofit over it.
    let mut book = new_file();
    let sheet = book.sheet_by_name_mut("Sheet1").unwrap();
    sheet.column_dimension_mut("A").set_width(24.0);
    assert!(sheet.column_dimension("A").unwrap().custom_width());

    let cols = col_elements(&sheet1_xml(&to_bytes(&book)));
    assert_eq!(cols.len(), 1, "{cols:?}");
    assert!(cols[0].contains("customWidth=\"1\""), "{}", cols[0]);
}

#[test]
fn an_inherited_width_is_not_written_as_authored() {
    let mut book = new_file();
    let sheet = book.sheet_by_name_mut("Sheet1").unwrap();
    sheet
        .column_dimension_mut("A")
        .set_width(24.0)
        .set_custom_width(false);

    let cols = col_elements(&sheet1_xml(&to_bytes(&book)));
    assert_eq!(cols.len(), 1, "{cols:?}");
    assert!(
        !cols[0].contains("customWidth"),
        "an inherited width was stamped as authored: {}",
        cols[0]
    );
    // The width itself is still written -- it is the default the column
    // inherits.
    assert!(cols[0].contains("width="), "{}", cols[0]);
}

#[test]
fn custom_width_survives_a_full_round_trip() {
    let mut book = new_file();
    {
        let sheet = book.sheet_by_name_mut("Sheet1").unwrap();
        sheet.column_dimension_mut("A").set_width(24.0);
        sheet
            .column_dimension_mut("B")
            .set_width(11.0)
            .set_custom_width(false);
    }

    let back = reader::xlsx::read_reader(Cursor::new(to_bytes(&book)), true).expect("read back");
    let sheet = back.sheet_by_name("Sheet1").unwrap();
    assert!(sheet.column_dimension("A").unwrap().custom_width());
    assert!(!sheet.column_dimension("B").unwrap().custom_width());
}

/// 🚨 The reason `custom_width` had to join `Column::hash_code`.
///
/// Adjacent columns coalesce into a single `<col min= max=>` run when their
/// hashes match, and the run is written from the FIRST column of the group. Two
/// columns identical in width, hidden and bestFit but differing in
/// `customWidth` would therefore collapse into one element, and the second
/// column would silently take the first's flag.
///
/// Before `custom_width` was added to the hash this test read back B as
/// authored.
#[test]
fn columns_differing_only_in_custom_width_do_not_coalesce() {
    let mut book = new_file();
    {
        let sheet = book.sheet_by_name_mut("Sheet1").unwrap();
        sheet.column_dimension_mut("A").set_width(24.0);
        sheet
            .column_dimension_mut("B")
            .set_width(24.0) // identical width -- only the flag differs
            .set_custom_width(false);
    }

    let xml = sheet1_xml(&to_bytes(&book));
    let cols = col_elements(&xml);
    assert_eq!(
        cols.len(),
        2,
        "the two columns coalesced and one lost its customWidth: {xml}"
    );

    let back = reader::xlsx::read_reader(Cursor::new(to_bytes(&book)), true).expect("read back");
    let sheet = back.sheet_by_name("Sheet1").unwrap();
    assert!(sheet.column_dimension("A").unwrap().custom_width());
    assert!(
        !sheet.column_dimension("B").unwrap().custom_width(),
        "B inherited A's customWidth through run coalescing"
    );
}

/// Columns that agree on every written attribute must still coalesce -- the
/// hash change must not split runs that Excel expects to be merged.
#[test]
fn identical_columns_still_coalesce_into_one_run() {
    let mut book = new_file();
    {
        let sheet = book.sheet_by_name_mut("Sheet1").unwrap();
        for col in ["A", "B", "C"] {
            sheet.column_dimension_mut(col).set_width(24.0);
        }
    }

    let xml = sheet1_xml(&to_bytes(&book));
    let cols = col_elements(&xml);
    assert_eq!(cols.len(), 1, "identical columns stopped coalescing: {xml}");
    assert!(cols[0].contains("min=\"1\""), "{}", cols[0]);
    assert!(cols[0].contains("max=\"3\""), "{}", cols[0]);
}
