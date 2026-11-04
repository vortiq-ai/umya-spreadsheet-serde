use std::io::Cursor;

use umya_spreadsheet::{
    AutoFilter,
    FilterSortBy,
    FilterSortMethod,
    Workbook,
    reader,
    writer,
};

fn roundtrip(book: &Workbook) -> Workbook {
    let mut bytes = Vec::new();
    writer::xlsx::write_writer(book, &mut bytes).unwrap();
    reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap()
}

fn filtered_book(range: &str, columns: &[u32]) -> Workbook {
    let rules = columns
        .iter()
        .map(|column| {
            format!(
                "<filterColumn colId=\"{column}\"><filters><filter \
                 val=\"Keep\"/></filters></filterColumn>"
            )
        })
        .collect::<String>();
    let mut book = umya_spreadsheet::new_file();
    book.sheet_mut(0).unwrap().set_auto_filter(range);
    *book.sheet_mut(0).unwrap().auto_filter_mut().unwrap() = AutoFilter::from_xml(
        format!(
            "<autoFilter xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" \
             ref=\"{range}\">{rules}</autoFilter>"
        )
        .as_bytes(),
    )
    .unwrap();
    roundtrip(&book)
}

fn assert_filter(book: &Workbook, range: &str, columns: &[u32]) {
    for book in [book.clone(), roundtrip(book)] {
        let filter = book.sheet(0).unwrap().auto_filter().unwrap();
        assert_eq!(filter.range().range(), range);
        assert_eq!(
            filter
                .filter_columns()
                .iter()
                .map(|column| column.column_id())
                .collect::<Vec<_>>(),
            columns,
        );
        for column in filter.filter_columns() {
            assert!(
                String::from_utf8(column.to_xml().unwrap())
                    .unwrap()
                    .contains("val=\"Keep\""),
            );
        }
    }
}

#[test]
fn deleting_entire_filter_range_removes_the_filter() {
    let mut book = umya_spreadsheet::new_file();
    book.sheet_mut(0).unwrap().set_auto_filter("A1:C3");
    let mut bytes = Vec::new();
    writer::xlsx::write_writer(&book, &mut bytes).unwrap();
    let mut book = reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap();

    book.remove_column("Sheet1", "A", 3);
    assert!(
        book.sheet(0).unwrap().auto_filter().is_none(),
        "deleting every filter column must remove the worksheet filter"
    );
    let mut bytes = Vec::new();
    writer::xlsx::write_writer(&book, &mut bytes).unwrap();
    let book = reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap();
    assert!(book.sheet(0).unwrap().auto_filter().is_none());
}

#[test]
fn deleting_all_filter_rows_or_columns_removes_rules_after_reload() {
    for (start, count) in [(3, 6), (2, 8)] {
        let mut book = filtered_book("C3:F8", &[0, 1, 3]);
        book.remove_row("Sheet1", start, count);
        assert!(book.sheet(0).unwrap().auto_filter().is_none());
        assert!(roundtrip(&book).sheet(0).unwrap().auto_filter().is_none());
    }
    for (start, count) in [("C", 4), ("B", 6)] {
        let mut book = filtered_book("C3:F8", &[0, 1, 3]);
        book.remove_column("Sheet1", start, count);
        assert!(book.sheet(0).unwrap().auto_filter().is_none());
        assert!(roundtrip(&book).sheet(0).unwrap().auto_filter().is_none());
    }
}

#[test]
fn deleting_the_row_or_column_of_a_single_cell_filter_removes_it() {
    let mut book = filtered_book("D5", &[0]);
    book.remove_row("Sheet1", 5, 1);
    assert!(book.sheet(0).unwrap().auto_filter().is_none());
    assert!(roundtrip(&book).sheet(0).unwrap().auto_filter().is_none());
    let mut book = filtered_book("D5", &[0]);
    book.remove_column("Sheet1", "D", 1);
    assert!(book.sheet(0).unwrap().auto_filter().is_none());
    assert!(roundtrip(&book).sheet(0).unwrap().auto_filter().is_none());
}

#[test]
fn partial_row_deletions_clip_the_filter_to_surviving_rows() {
    for (start, count, expected) in [
        (2, 2, "C2:F6"),
        (5, 2, "C3:F6"),
        (6, 5, "C3:F5"),
        (1, 1, "C2:F7"),
        (9, 2, "C3:F8"),
    ] {
        let mut book = filtered_book("C3:F8", &[0, 1, 3]);
        book.remove_row("Sheet1", start, count);
        assert_filter(&book, expected, &[0, 1, 3]);
    }
}

#[test]
fn partial_column_deletions_clip_the_filter_and_keep_surviving_rules() {
    for (start, count, expected, columns) in [
        ("B", 2, "B3:D8", &[0, 2][..]),
        ("D", 1, "C3:E8", &[0, 2][..]),
        ("E", 4, "C3:D8", &[0, 1][..]),
        ("A", 1, "B3:E8", &[0, 1, 3][..]),
        ("G", 2, "C3:F8", &[0, 1, 3][..]),
    ] {
        let mut book = filtered_book("C3:F8", &[0, 1, 3]);
        book.remove_column("Sheet1", start, count);
        assert_filter(&book, expected, columns);
    }
}

#[test]
fn deletions_before_a_single_cell_filter_shift_it_without_dropping_rules() {
    let mut book = filtered_book("D5", &[0]);
    book.remove_row("Sheet1", 1, 2);
    assert_filter(&book, "D3", &[0]);
    let mut book = filtered_book("D5", &[0]);
    book.remove_column("Sheet1", "A", 2);
    assert_filter(&book, "B5", &[0]);
}

#[test]
fn zero_length_deletions_leave_the_filter_and_rules_unchanged() {
    let mut book = filtered_book("C3:F8", &[0, 1, 3]);
    book.remove_row("Sheet1", 3, 0);
    book.remove_column("Sheet1", "C", 0);
    assert_filter(&book, "C3:F8", &[0, 1, 3]);
}

fn sorted_book(reference: &str, column_sort: bool, conditions: &[(&str, &str)]) -> Workbook {
    let conditions = conditions
        .iter()
        .map(|(reference, identity)| {
            format!(
                "<sortCondition ref=\"{reference}\" descending=\"1\" sortBy=\"value\" \
                 customList=\"{identity}\"/>"
            )
        })
        .collect::<String>();
    let mut book = umya_spreadsheet::new_file();
    book.sheet_mut(0).unwrap().set_auto_filter("B2:G9");
    *book.sheet_mut(0).unwrap().auto_filter_mut().unwrap() = AutoFilter::from_xml(
        format!(
            "<autoFilter xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" \
             xmlns:f=\"urn:sort-future\" ref=\"B2:G9\"><sortState ref=\"{reference}\" \
             columnSort=\"{column_sort}\" caseSensitive=\"1\" \
             sortMethod=\"stroke\">{conditions}<extLst><ext uri=\"urn:sort-future\"><f:flag \
             value=\"keep\"/></ext></extLst></sortState></autoFilter>"
        )
        .as_bytes(),
    )
    .unwrap();
    roundtrip(&book)
}

fn assert_sort(book: &Workbook, reference: &str, column_sort: bool, conditions: &[(&str, &str)]) {
    for book in [book.clone(), roundtrip(book)] {
        let filter = book.sheet(0).unwrap().auto_filter().unwrap();
        let sort = filter.sort_state().unwrap();
        assert_eq!(sort.reference.range(), reference);
        assert_eq!(sort.column_sort, Some(column_sort));
        assert_eq!(sort.case_sensitive, Some(true));
        assert_eq!(sort.method, Some(FilterSortMethod::Stroke));
        assert_eq!(
            sort.conditions
                .iter()
                .map(|condition| (
                    condition.reference.range(),
                    condition.custom_list.as_deref().unwrap(),
                ))
                .collect::<Vec<_>>(),
            conditions
                .iter()
                .map(|(reference, identity)| (reference.to_string(), *identity))
                .collect::<Vec<_>>(),
        );
        for condition in &sort.conditions {
            assert_eq!(condition.descending, Some(true));
            assert_eq!(condition.sort_by, Some(FilterSortBy::Value));
        }
        let xml = String::from_utf8(filter.to_xml().unwrap()).unwrap();
        assert!(xml.contains("urn:sort-future"));
        assert!(xml.contains("value=\"keep\""));
    }
}

#[test]
fn partial_row_deletions_clip_sort_state_and_condition_references() {
    for (start, count, reference, conditions) in [
        (
            2,
            2,
            "C2:F6",
            [("C2:C6", "First"), ("D2:D6", "Middle"), ("F2:F6", "Last")],
        ),
        (
            5,
            2,
            "C3:F6",
            [("C3:C6", "First"), ("D3:D6", "Middle"), ("F3:F6", "Last")],
        ),
        (
            6,
            5,
            "C3:F5",
            [("C3:C5", "First"), ("D3:D5", "Middle"), ("F3:F5", "Last")],
        ),
    ] {
        let mut book = sorted_book(
            "C3:F8",
            false,
            &[("C3:C8", "First"), ("D3:D8", "Middle"), ("F3:F8", "Last")],
        );
        book.remove_row("Sheet1", start, count);
        assert_sort(&book, reference, false, &conditions);
    }
}

#[test]
fn partial_column_deletions_clip_sort_state_and_condition_references() {
    for (start, count, reference, conditions) in [
        (
            "B",
            2,
            "B3:D8",
            [("B3:D3", "First"), ("B5:D5", "Middle"), ("B8:D8", "Last")],
        ),
        (
            "D",
            1,
            "C3:E8",
            [("C3:E3", "First"), ("C5:E5", "Middle"), ("C8:E8", "Last")],
        ),
        (
            "E",
            4,
            "C3:D8",
            [("C3:D3", "First"), ("C5:D5", "Middle"), ("C8:D8", "Last")],
        ),
    ] {
        let mut book = sorted_book(
            "C3:F8",
            true,
            &[("C3:F3", "First"), ("C5:F5", "Middle"), ("C8:F8", "Last")],
        );
        book.remove_column("Sheet1", start, count);
        assert_sort(&book, reference, true, &conditions);
    }
}

#[test]
fn fully_deleted_sort_states_are_removed_while_the_filter_survives() {
    for (reference, start, count) in [("C3:F8", 3, 6), ("D4:E7", 3, 5), ("D5", 5, 1)] {
        let mut book = sorted_book(reference, false, &[(reference, "Removed")]);
        book.remove_row("Sheet1", start, count);
        for book in [book.clone(), roundtrip(&book)] {
            assert!(
                book.sheet(0)
                    .unwrap()
                    .auto_filter()
                    .unwrap()
                    .sort_state()
                    .is_none()
            );
        }
    }
    for (reference, start, count) in [("C3:F8", "C", 4), ("D4:E7", "C", 4), ("D5", "D", 1)] {
        let mut book = sorted_book(reference, false, &[(reference, "Removed")]);
        book.remove_column("Sheet1", start, count);
        for book in [book.clone(), roundtrip(&book)] {
            assert!(
                book.sheet(0)
                    .unwrap()
                    .auto_filter()
                    .unwrap()
                    .sort_state()
                    .is_none()
            );
        }
    }
}

#[test]
fn deleted_sort_conditions_leave_survivors_in_their_authored_order() {
    let mut book = sorted_book(
        "C3:F8",
        false,
        &[("F3:F8", "Last"), ("D3:D8", "Middle"), ("C3:C8", "First")],
    );
    book.remove_column("Sheet1", "D", 1);
    assert_sort(
        &book,
        "C3:E8",
        false,
        &[("E3:E8", "Last"), ("C3:C8", "First")],
    );
    let mut book = sorted_book(
        "C3:F8",
        true,
        &[("C8:F8", "Last"), ("C5:F5", "Middle"), ("C3:F3", "First")],
    );
    book.remove_row("Sheet1", 5, 1);
    assert_sort(
        &book,
        "C3:F7",
        true,
        &[("C7:F7", "Last"), ("C3:F3", "First")],
    );
}

#[test]
fn deleting_every_sort_condition_preserves_the_surviving_sort_state_and_extensions() {
    // CT_SortState permits zero sortCondition children. The owner still has
    // a valid reference and must not lose its unrelated attributes/extensions.
    let mut book = sorted_book("C3:F8", false, &[("D5", "Removed")]);
    book.remove_column("Sheet1", "D", 1);
    assert_sort(&book, "C3:E8", false, &[]);
}
