//! Removing rows or columns moves and shrinks the ranges of base conditional
//! formats the way Excel does. Every expectation below was measured in Excel
//! (delete the rows or columns of a sheet holding a color scale on C2:C7 and an
//! icon set on E2:F7,H2:H3 and read each format's AppliesTo).
use umya_spreadsheet::{
    ConditionalFormatting,
    ConditionalFormattingRule,
    Workbook,
    structs::ConditionalFormatValues,
};

fn sheet_with_formats() -> Workbook {
    let mut book = umya_spreadsheet::new_file();
    let sheet = book.sheet_mut(0).unwrap();
    for sqref in ["C2:C7", "E2:F7 H2:H3"] {
        let mut rule = ConditionalFormattingRule::default();
        rule.set_type(ConditionalFormatValues::ColorScale)
            .set_priority(1);
        let mut group = ConditionalFormatting::default();
        group.sequence_of_references_mut().set_sqref(sqref);
        group.add_conditional_collection(rule);
        sheet.add_conditional_formatting_collection(group);
    }
    book
}

fn applies_to(book: &Workbook) -> Vec<String> {
    book.sheet(0)
        .unwrap()
        .conditional_formatting_collection()
        .iter()
        .map(|group| group.sequence_of_references().get_sqref())
        .collect()
}

#[test]
fn removing_rows_follows_excel_for_every_position_of_the_removed_block() {
    // (first row, row count, Excel's result)
    let cases: [(u32, u32, &[&str]); 8] = [
        (1, 1, &["C1:C6", "E1:F6 H1:H2"]),
        (4, 1, &["C2:C6", "E2:F6 H2:H3"]),
        (2, 1, &["C2:C6", "E2:F6 H2"]),
        (7, 1, &["C2:C6", "E2:F6 H2:H3"]),
        (8, 1, &["C2:C7", "E2:F7 H2:H3"]),
        (2, 2, &["C2:C5", "E2:F5"]),
        (6, 4, &["C2:C5", "E2:F5 H2:H3"]),
        (2, 6, &[]),
    ];
    for (row, count, expected) in cases {
        let mut book = sheet_with_formats();
        book.remove_row("Sheet1", row, count);
        assert_eq!(
            applies_to(&book),
            expected,
            "removing {count} row(s) at {row}"
        );
    }
}

#[test]
fn removing_rows_that_cover_the_formats_removes_them() {
    let mut book = sheet_with_formats();
    book.remove_row("Sheet1", 1, 8);
    assert!(applies_to(&book).is_empty());
}

#[test]
fn removing_columns_follows_excel() {
    // (first column, column count, Excel's result)
    let cases: [(u32, u32, &[&str]); 3] = [
        (1, 1, &["B2:B7", "D2:E7 G2:G3"]),
        (3, 1, &["D2:E7 G2:G3"]),
        (4, 1, &["C2:C7", "D2:E7 G2:G3"]),
    ];
    for (column, count, expected) in cases {
        let mut book = sheet_with_formats();
        book.remove_column_by_index("Sheet1", column, count);
        assert_eq!(
            applies_to(&book),
            expected,
            "removing {count} column(s) at {column}"
        );
    }
}

#[test]
fn whole_row_and_whole_column_formats_keep_their_form_like_excel() {
    // (applies to, removal, Excel's result); Excel measured on a color scale
    // over the range after deleting the rows or columns.
    type Remove = fn(&mut Workbook);
    let cases: [(&str, Remove, &str); 8] = [
        ("A:A", |book| book.remove_row("Sheet1", 2, 1), "A:A"),
        ("5:5", |book| book.remove_column("Sheet1", "B", 1), "5:5"),
        ("A:C", |book| book.remove_column("Sheet1", "B", 2), "A:A"),
        ("1:3", |book| book.remove_row("Sheet1", 2, 2), "1:1"),
        ("A:C", |book| book.remove_column("Sheet1", "A", 2), "A:A"),
        ("1:3", |book| book.remove_row("Sheet1", 1, 2), "1:1"),
        ("B:B", |book| book.remove_column("Sheet1", "A", 1), "A:A"),
        ("3:3", |book| book.remove_row("Sheet1", 1, 1), "2:2"),
    ];
    for (range, remove, expected) in cases {
        let mut book = umya_spreadsheet::new_file();
        let mut rule = ConditionalFormattingRule::default();
        rule.set_type(ConditionalFormatValues::ColorScale)
            .set_priority(1);
        let mut group = ConditionalFormatting::default();
        group.sequence_of_references_mut().set_sqref(range);
        group.add_conditional_collection(rule);
        book.sheet_mut(0)
            .unwrap()
            .add_conditional_formatting_collection(group);
        remove(&mut book);
        assert_eq!(applies_to(&book), [expected], "{range}");
    }
}

#[test]
fn whole_row_and_whole_column_formats_removed_with_their_line_are_dropped() {
    for (range, remove) in [
        (
            "A:A",
            (|book: &mut Workbook| book.remove_column("Sheet1", "A", 1)) as fn(&mut Workbook),
        ),
        ("2:2", |book| book.remove_row("Sheet1", 2, 1)),
    ] {
        let mut book = umya_spreadsheet::new_file();
        let mut group = ConditionalFormatting::default();
        group.sequence_of_references_mut().set_sqref(range);
        book.sheet_mut(0)
            .unwrap()
            .add_conditional_formatting_collection(group);
        remove(&mut book);
        assert!(applies_to(&book).is_empty(), "{range}");
    }
}
