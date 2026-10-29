//! Keep the slice/void/chaining API used by existing fork clients.
use umya_spreadsheet::{
    Workbook,
    Worksheet,
};

#[test]
fn existing_workbook_access_signatures_remain_available() {
    let _: fn(&mut Workbook, &str, u32, u32) = Workbook::insert_new_row;
    let _: fn(&mut Workbook, &str, &str, u32) = Workbook::insert_new_column;
    let _: fn(&mut Workbook, &str, u32, u32) = Workbook::insert_new_column_by_index;
    let _: fn(&mut Workbook, &str, u32, u32) = Workbook::remove_row;
    let _: fn(&mut Workbook, &str, &str, u32) = Workbook::remove_column;
    let _: fn(&mut Workbook, &str, u32, u32) = Workbook::remove_column_by_index;

    let mut book = umya_spreadsheet::new_file();
    book.new_sheet("Second").unwrap();
    let sheets: &[Worksheet] = book.sheet_collection();
    assert_eq!(sheets.len(), 2);
    assert_eq!(
        sheets.iter().map(Worksheet::name).collect::<Vec<_>>(),
        ["Sheet1", "Second"]
    );
    let _: &Worksheet = book.active_sheet();
    let _: &mut Worksheet = book.active_sheet_mut();
    let _: &mut [Worksheet] = book.sheet_collection_mut();
    let _: &mut Workbook = book
        .read_sheet(0)
        .read_sheet_by_name("Second")
        .read_sheet_collection();
}

#[test]
fn fallible_loaded_access_preserves_active_sheet_error_kinds() {
    let mut book = umya_spreadsheet::new_file();
    book.set_active_sheet(10);
    assert_eq!(
        book.try_active_sheet().unwrap_err().kind(),
        umya_spreadsheet::XlsxErrorKind::NotFound
    );
    assert_eq!(
        book.try_active_sheet_mut().unwrap_err().kind(),
        umya_spreadsheet::XlsxErrorKind::NotFound
    );
}
