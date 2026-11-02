//! A comment box follows its cell through a structural edit. The expected
//! anchors are Excel 16.0 build 20527's own saves of the same edits (the
//! `CommentMeasurements` mode of `drawing_anchor_excel.ps1`; logs
//! `lib-opsanchors-r3-excel-comment-box-03.log`). The comment is in C5; the
//! `origin` box starts in A1 and the `inside` box in C3.

use std::path::Path;

use umya_spreadsheet::{
    Workbook,
    reader,
    writer,
};

type Anchor = [u32; 8];

fn anchor_after(base: &str, edit: impl FnOnce(&mut Workbook)) -> Anchor {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/drawing_anchors")
        .join(format!("{base}.xlsx"));
    let mut book = reader::xlsx::read(path).unwrap();
    edit(&mut book);
    let mut bytes = Vec::new();
    writer::xlsx::write_writer(&book, &mut std::io::Cursor::new(&mut bytes)).unwrap();
    let reread = reader::xlsx::read_reader(std::io::Cursor::new(bytes), true).unwrap();
    let comments = reread.sheet_by_name("Drawing").unwrap().comments();
    assert_eq!(comments.len(), 1);
    let anchor = comments[0].anchor();
    [
        anchor.left_column(),
        anchor.left_offset(),
        anchor.top_row(),
        anchor.top_offset(),
        anchor.right_column(),
        anchor.right_offset(),
        anchor.bottom_row(),
        anchor.bottom_offset(),
    ]
}

#[test]
fn comment_boxes_move_with_their_cell_and_stop_at_the_sheet_edge() {
    let unchanged_origin = [0, 0, 0, 0, 2, 16, 3, 19];
    let unchanged_inside = [2, 0, 2, 0, 4, 16, 5, 19];
    assert_eq!(
        anchor_after("comment-box-origin-base", |_| {}),
        unchanged_origin
    );
    assert_eq!(
        anchor_after("comment-box-inside-base", |_| {}),
        unchanged_inside
    );

    // Deleting row 1 / column A leaves the clamped box where it was.
    assert_eq!(
        anchor_after("comment-box-origin-base", |book| book
            .remove_row("Drawing", 1, 1)),
        unchanged_origin
    );
    assert_eq!(
        anchor_after("comment-box-origin-base", |book| {
            book.remove_column_by_index("Drawing", 1, 1);
        }),
        unchanged_origin
    );
    assert_eq!(
        anchor_after("comment-box-origin-base", |book| book
            .insert_new_row("Drawing", 1, 1)),
        [0, 0, 1, 0, 2, 16, 4, 19]
    );
    assert_eq!(
        anchor_after("comment-box-origin-base", |book| {
            book.insert_new_column_by_index("Drawing", 1, 1);
        }),
        [1, 0, 0, 0, 3, 16, 3, 19]
    );

    // A row edit inside the box moves the whole box with the cell; a column
    // edit to the right of the cell does not touch it.
    assert_eq!(
        anchor_after("comment-box-inside-base", |book| book
            .remove_row("Drawing", 4, 1)),
        [2, 0, 1, 0, 4, 16, 4, 19]
    );
    assert_eq!(
        anchor_after("comment-box-inside-base", |book| book
            .insert_new_row("Drawing", 4, 1)),
        [2, 0, 3, 0, 4, 16, 6, 19]
    );
    assert_eq!(
        anchor_after("comment-box-inside-base", |book| {
            book.remove_column_by_index("Drawing", 4, 1);
        }),
        unchanged_inside
    );
    assert_eq!(
        anchor_after("comment-box-inside-base", |book| {
            book.insert_new_column_by_index("Drawing", 4, 1);
        }),
        unchanged_inside
    );
}
