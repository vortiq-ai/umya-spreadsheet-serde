#![cfg(feature = "serde")]

use std::io::Cursor;

use umya_spreadsheet::*;

fn roundtrip(book: &Workbook) -> Workbook {
    let mut bytes = Vec::new();
    writer::xlsx::write_writer(book, &mut bytes).unwrap();
    reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap()
}

fn fixture(with_filter: bool, totals: u32) -> (Workbook, SlicerCache) {
    let mut table = Table::default();
    table.set_name("Data");
    table.set_id(41);
    table.set_area(("A1", "B4"));
    table.set_totals_row_count(totals);
    for (id, name) in [(7, "Value"), (8, "Other")] {
        let mut column = TableColumn::new(name);
        column.set_id(id);
        table.add_column(column);
    }
    if with_filter {
        table.set_auto_filter("A1:B4");
        let mut column = FilterColumn::default();
        column
            .set_column_id(1)
            .set_rule(Some(FilterRule::Values(ValueFilters {
                values: vec![Some("X".into())],
                ..Default::default()
            })));
        table.auto_filter_mut().add_filter_column(column);
    }
    let mut book = new_file();
    let sheet = book.sheet_mut(0).unwrap();
    sheet.cell_mut("A1").set_value("Value");
    sheet.cell_mut("B1").set_value("Other");
    sheet.cell_mut("A2").set_value("A");
    sheet.cell_mut("B2").set_value("X");
    sheet.add_table(table);
    let cache = SlicerCache {
        name: "TableCache".into(),
        source_name: "Value".into(),
        source: Some(SlicerSource::Table(TableSlicerCache {
            table_id: 41,
            column_id: 7,
            ..Default::default()
        })),
        ..Default::default()
    };
    (roundtrip(&book), cache)
}

fn selection() -> SlicerSelection {
    SlicerSelection::Items(vec![SlicerItemKey::TableValue(SlicerTableValue::Text(
        "A".into(),
    ))])
}

#[test]
fn moved_filtered_column_rejects_stale_selection_before_mutation() {
    let (mut book, cache) = fixture(true, 0);
    let sheet = book.sheet_mut(0).unwrap();
    let plan = cache
        .plan_table_selection(selection(), &sheet.tables()[0])
        .unwrap();
    sheet.tables_mut()[0].columns_mut().swap(0, 1);
    sheet.cell_mut("A1").set_value("Other");
    sheet.cell_mut("B1").set_value("Value");
    sheet.cell_mut("A2").set_value("X");
    sheet.cell_mut("B2").set_value("A");
    let before = serde_json::to_vec(&book.sheet(0).unwrap().tables()[0]).unwrap();
    assert!(
        plan.apply(&cache, &mut book.sheet_mut(0).unwrap().tables_mut()[0])
            .is_err()
    );
    assert_eq!(
        serde_json::to_vec(&book.sheet(0).unwrap().tables()[0]).unwrap(),
        before
    );
}

#[test]
fn selection_creates_missing_table_filter_and_excludes_totals() {
    for totals in [0, 1] {
        let (mut book, cache) = fixture(false, totals);
        let table = &mut book.sheet_mut(0).unwrap().tables_mut()[0];
        assert!(table.auto_filter().range().range().is_empty());
        cache
            .plan_table_selection(selection(), table)
            .unwrap()
            .apply(&cache, table)
            .unwrap();
        let reread = roundtrip(&book);
        let table = &reread.sheet(0).unwrap().tables()[0];
        assert_eq!(
            table.auto_filter().range().range(),
            if totals == 0 { "A1:B4" } else { "A1:B3" }
        );
        assert_eq!(table.auto_filter().filter_columns().len(), 1);
        let column = &table.auto_filter().filter_columns()[0];
        assert_eq!(column.column_id(), 0);
        let Some(FilterRule::Values(values)) = column.rule() else {
            panic!("missing selection")
        };
        assert_eq!(values.values, vec![Some("A".into())]);
    }
}
