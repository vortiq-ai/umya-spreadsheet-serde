#![cfg(feature = "serde")]

use std::{
    collections::BTreeMap,
    io::{
        Cursor,
        Read,
        Write,
    },
};

use umya_spreadsheet::{
    Chart,
    Workbook,
    drawing::charts::{
        ReferenceCell,
        ReferenceRect,
    },
};

#[path = "support/chart_xml_fixture.rs"]
pub mod fixture;

fn structural_xml() -> String {
    let error_bars = "<c:errBars><c:errDir val=\"y\"/><c:errBarType val=\"both\"/><c:errValType \
                      val=\"cust\"/><c:plus><c:numRef><c:f>Data!$E$2:$E$5</c:f></c:numRef></c:\
                      plus><c:minus><c:numRef><c:f>Data!$F$2:$F$5</c:f></c:numRef></c:minus></c:\
                      errBars>";
    fixture::all_owner_xml()
        .replace(
            "</c:trendline><c:cat>",
            &format!("</c:trendline>{error_bars}<c:cat>"),
        )
        .replace(
            "<c:dLbl><c:idx val=\"9\"/></c:dLbl>",
            "<c:dLbl><c:idx \
             val=\"9\"/><c:tx><c:strRef><c:f>Data!$G$2:$G$5</c:f></c:strRef></c:tx></c:dLbl>",
        )
        .replace(
            "<c:dPt><c:idx val=\"9\"/>",
            "<c:dPt><c:idx val=\"1\"/></c:dPt><c:dPt><c:idx val=\"9\"/></c:dPt><c:dPt><c:idx \
             val=\"9\"/>",
        )
}

fn structural_book() -> Workbook {
    let mut book = fixture::imported_workbook(&structural_xml());
    book.new_sheet("Data").unwrap();
    assert_eq!(
        formula_counts(&fixture::chart_xml(&book)),
        expected_roles("Data", COLUMNS, Some((2, 5))),
        "authored role denominator"
    );
    book
}

fn formula_counts(xml: &str) -> BTreeMap<String, usize> {
    let mut result = BTreeMap::new();
    for tail in xml.split("<c:f>").skip(1) {
        let text = tail.split_once("</c:f>").unwrap().0;
        let text = quick_xml::escape::unescape(text).unwrap().into_owned();
        *result.entry(text).or_default() += 1;
    }
    result
}

// This ledger comes from the authored input: two ordinary categories/values,
// two plus/minus error references, two point-label references, one display-unit
// label and one pivot label. Expected edits never use a candidate formula.
fn expected_roles(
    sheet: &str,
    columns: [Option<&str>; 7],
    rows: Option<(u32, u32)>,
) -> BTreeMap<String, usize> {
    let mut result = BTreeMap::new();
    for (role, count) in [(0, 2), (1, 2), (2, 1), (3, 1), (4, 2), (5, 2), (6, 2)] {
        let text = match (columns[role], rows) {
            (Some(column), _) if role == 2 || role == 3 => format!("{sheet}!${column}$1"),
            (Some(column), Some((first, last))) => {
                format!("{sheet}!${column}${first}:${column}${last}")
            }
            // Excel capture-r3 ordinary complete row/column deletion retains
            // the surviving sheet qualifier in ordinary chart c:f roles.
            // Immutable after.xlsx SHA-256 witnesses (not candidate output):
            // rows bdbc7d1b9451748706f05e8c4fbac3cf4ba698bff0c5967c1b977e1da51ef2a7
            // cols 5a1a8add49e7726c2f6b249b336fd18d72dd3084e2c58dcdc38b979be01b6d9a
            // Sheet deletion has a distinct bare-#REF! assertion below.
            _ => format!("{sheet}!#REF!"),
        };
        *result.entry(text).or_default() += count;
    }
    result
}

const COLUMNS: [Option<&str>; 7] = [
    Some("A"),
    Some("B"),
    Some("C"),
    Some("D"),
    Some("E"),
    Some("F"),
    Some("G"),
];

fn assert_edited_projection(mut book: Workbook, expected: BTreeMap<String, usize>) {
    let basis = book.sheet(0).unwrap().chart_collection()[0].clone();
    let before = bincode::serialize(&basis).unwrap();
    let mut replacement = basis.clone();
    replacement.strip_export_parts();
    replacement.set_title("metadata update after source edit");
    let report = replacement.restore_export_parts(&basis).unwrap();
    assert_eq!(report.matched_series, 2, "{report:?}");
    assert_eq!(report.ambiguous_series, 0, "{report:?}");
    for group in replacement.chart_space().chart().plot_area().chart_groups() {
        let points = group.series().area_chart_series()[0].data_points();
        assert_eq!(
            points
                .iter()
                .map(|point| point.index().val().unwrap())
                .collect::<Vec<_>>(),
            [1, 9, 9]
        );
    }
    book.sheet_mut(0).unwrap().chart_collection_mut()[0] = replacement;
    assert_eq!(formula_counts(&fixture::chart_xml(&book)), expected);
    assert_eq!(
        bincode::serialize(&basis).unwrap(),
        before,
        "authoritative basis changed"
    );
    let reloaded = umya_spreadsheet::reader::xlsx::read_reader(
        Cursor::new(fixture::write_workbook(&book)),
        true,
    )
    .unwrap();
    assert_eq!(formula_counts(&fixture::chart_xml(&reloaded)), expected);
}

#[test]
fn ordinary_rename_updates_hidden_roles_before_metadata_restoration() {
    let mut book = structural_book();
    book.set_sheet_name(1, "Bob's Data").unwrap();
    assert_edited_projection(book, expected_roles("'Bob''s Data'", COLUMNS, Some((2, 5))));
}

#[test]
fn ordinary_sheet_deletion_restores_error_references_not_predelete_formulas() {
    let mut book = structural_book();
    book.remove_sheet_by_name("Data").unwrap();
    assert_edited_projection(book, BTreeMap::from([("#REF!".into(), 12)]));
}

#[test]
fn insertion_before_source_rows_adjusts_each_hidden_role_exactly_once() {
    let mut book = structural_book();
    book.insert_new_row("Data", 2, 1);
    assert_edited_projection(book, expected_roles("Data", COLUMNS, Some((3, 6))));
}

#[test]
fn insertion_inside_source_rows_expands_ranges_without_renumbering_points() {
    let mut book = structural_book();
    book.insert_new_row("Data", 4, 1);
    assert_edited_projection(book, expected_roles("Data", COLUMNS, Some((2, 6))));
}

#[test]
fn insertion_after_source_rows_keeps_every_reference_unchanged() {
    let mut book = structural_book();
    book.insert_new_row("Data", 6, 1);
    assert_edited_projection(book, expected_roles("Data", COLUMNS, Some((2, 5))));
}

#[test]
fn deleting_inside_source_rows_shrinks_hidden_ranges_before_restoration() {
    let mut book = structural_book();
    book.remove_row("Data", 3, 1);
    assert_edited_projection(book, expected_roles("Data", COLUMNS, Some((2, 4))));
}

#[test]
fn deleting_all_source_rows_keeps_unaffected_single_cell_label_references() {
    let mut book = structural_book();
    book.remove_row("Data", 2, 4);
    assert_edited_projection(book, expected_roles("Data", COLUMNS, None));
}

#[test]
fn column_insertion_adjusts_value_error_label_and_units_roles_exactly_once() {
    let mut book = structural_book();
    book.insert_new_column("Data", "B", 1);
    let columns = [
        Some("A"),
        Some("C"),
        Some("D"),
        Some("E"),
        Some("F"),
        Some("G"),
        Some("H"),
    ];
    assert_edited_projection(book, expected_roles("Data", columns, Some((2, 5))));
}

#[test]
fn deleting_the_value_column_does_not_delete_unrelated_hidden_references() {
    let mut book = structural_book();
    book.remove_column("Data", "B", 1);
    let columns = [
        Some("A"),
        None,
        Some("B"),
        Some("C"),
        Some("D"),
        Some("E"),
        Some("F"),
    ];
    assert_edited_projection(book, expected_roles("Data", columns, Some((2, 5))));
}

#[test]
fn moving_values_does_not_retarget_stationary_custom_errors_or_point_labels() {
    let mut book = structural_book();
    book.move_range(
        "Data",
        ReferenceRect {
            first: ReferenceCell {
                column: 2,
                row:    2,
            },
            last:  ReferenceCell {
                column: 2,
                row:    5,
            },
        },
        "Data",
        ReferenceCell {
            column: 8,
            row:    8,
        },
    )
    .unwrap();
    let mut expected = expected_roles("Data", COLUMNS, Some((2, 5)));
    assert_eq!(expected.remove("Data!$B$2:$B$5"), Some(2));
    expected.insert("Data!$H$8:$H$11".into(), 2);
    assert_edited_projection(book, expected);
}

#[test]
fn changing_editable_values_keeps_original_custom_errors_and_point_indices() {
    let mut book = structural_book();
    let basis = book.sheet(0).unwrap().chart_collection()[0].clone();
    let mut current = basis.clone();
    current.strip_export_parts();
    for group in current.plot_area_mut().chart_groups_mut() {
        group.series_mut().area_chart_series_mut()[0]
            .values_mut()
            .unwrap()
            .number_reference_mut()
            .formula_mut()
            .set_address_str("Data!$H$8:$H$11");
    }
    current.restore_export_parts(&basis).unwrap();
    book.sheet_mut(0).unwrap().chart_collection_mut()[0] = current;
    let mut expected = expected_roles("Data", COLUMNS, Some((2, 5)));
    assert_eq!(expected.remove("Data!$B$2:$B$5"), Some(2));
    expected.insert("Data!$H$8:$H$11".into(), 2);
    assert_eq!(formula_counts(&fixture::chart_xml(&book)), expected);
    for group in book.sheet(0).unwrap().chart_collection()[0]
        .chart_space()
        .chart()
        .plot_area()
        .chart_groups()
    {
        assert_eq!(
            group.series().area_chart_series()[0]
                .data_points()
                .iter()
                .map(|point| point.index().val().unwrap())
                .collect::<Vec<_>>(),
            [1, 9, 9]
        );
    }
}

fn cached_chart(label: &str) -> Chart {
    let source = format!(
        "<c:strCache><c:ptCount val=\"4\"/><c:pt idx=\"0\"><c:v>{label}</c:v></c:pt><c:pt \
         idx=\"1\"><c:v>second</c:v></c:pt><c:pt idx=\"2\"><c:v>third</c:v></c:pt><c:pt \
         idx=\"3\"><c:v>fourth</c:v></c:pt></c:strCache>"
    );
    let xml = structural_xml().replace(
        "<c:f>Data!$A$2:$A$5</c:f>",
        &format!("<c:f>Data!$A$2:$A$5</c:f>{source}"),
    );
    fixture::imported_chart(&xml)
}

fn category_source(chart: &Chart) -> Option<&umya_spreadsheet::drawing::charts::SourceCache> {
    chart
        .chart_space()
        .chart()
        .plot_area()
        .chart_groups()
        .next()
        .unwrap()
        .series()
        .area_chart_series()[0]
        .category_axis_data()
        .unwrap()
        .string_reference()
        .unwrap()
        .string_cache()
        .source()
}

#[test]
fn replacement_discards_supplied_caches_and_only_restores_the_matched_trusted_cache() {
    let basis = cached_chart("trusted server");
    let expected = category_source(&basis).unwrap().clone();
    let mut current = cached_chart("untrusted client");
    assert_ne!(category_source(&current), Some(&expected));
    let report = current.restore_export_parts(&basis).unwrap();
    assert!(report.restored_caches >= 1, "{report:?}");
    assert_eq!(category_source(&current), Some(&expected));
    assert_eq!(category_source(&basis), Some(&expected));
}

#[test]
fn actual_formula_changes_invalidate_cache_eligibility_without_guard_retagging() {
    let basis = cached_chart("trusted server");
    let old_guard = category_source(&basis).unwrap().formula().to_string();
    let mut current = basis.clone();
    current.strip_export_parts();
    for group in current.plot_area_mut().chart_groups_mut() {
        group.series_mut().area_chart_series_mut()[0]
            .category_axis_data_mut()
            .unwrap()
            .string_reference_mut()
            .unwrap()
            .formula_mut()
            .set_address_str("Data!$A$3:$A$6");
    }
    current.restore_export_parts(&basis).unwrap();
    assert!(category_source(&current).is_none());
    assert_eq!(category_source(&basis).unwrap().formula(), old_guard);
}

#[test]
fn history_restores_exact_chart_snapshots_instead_of_inverting_deleted_references() {
    let mut book = structural_book();
    let before = bincode::serialize(&book.sheet(0).unwrap().chart_collection()[0]).unwrap();
    book.remove_sheet_by_name("Data").unwrap();
    let after = bincode::serialize(&book.sheet(0).unwrap().chart_collection()[0]).unwrap();
    assert_ne!(before, after);
    assert_eq!(
        formula_counts(&fixture::chart_xml(&book)),
        BTreeMap::from([("#REF!".into(), 12)])
    );
    // Native history installs complete authoritative states directly.
    book.sheet_mut(0).unwrap().chart_collection_mut()[0] = bincode::deserialize(&before).unwrap();
    assert_eq!(
        bincode::serialize(&book.sheet(0).unwrap().chart_collection()[0]).unwrap(),
        before
    );
    assert_eq!(
        formula_counts(&fixture::chart_xml(&book)),
        expected_roles("Data", COLUMNS, Some((2, 5)))
    );
    book.sheet_mut(0).unwrap().chart_collection_mut()[0] = bincode::deserialize(&after).unwrap();
    assert_eq!(
        bincode::serialize(&book.sheet(0).unwrap().chart_collection()[0]).unwrap(),
        after
    );
    assert_eq!(
        formula_counts(&fixture::chart_xml(&book)),
        BTreeMap::from([("#REF!".into(), 12)])
    );
}

fn with_one_cell_chart(bytes: &[u8]) -> Vec<u8> {
    let mut input = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for index in 0..input.len() {
        let mut entry = input.by_index(index).unwrap();
        let mut data = Vec::new();
        entry.read_to_end(&mut data).unwrap();
        if entry.name().unwrap() == "xl/drawings/drawing1.xml" {
            let xml = String::from_utf8(data).unwrap();
            let start = xml.find("<xdr:to>").unwrap();
            let end = start + xml[start..].find("</xdr:to>").unwrap() + "</xdr:to>".len();
            data = format!(
                "{}<xdr:ext cx=\"1200\" cy=\"2400\"/>{}",
                &xml[..start],
                &xml[end..]
            )
            .replace("xdr:twoCellAnchor", "xdr:oneCellAnchor")
            .into_bytes();
        }
        output
            .start_file(
                entry.name().unwrap(),
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
        output.write_all(&data).unwrap();
    }
    output.finish().unwrap().into_inner()
}

#[test]
fn ordinary_structural_edits_visit_real_one_cell_chart_frames_exactly_once() {
    let bytes = with_one_cell_chart(&fixture::write_workbook(&structural_book()));
    let mut book = umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap();
    assert_eq!(
        book.sheet(0)
            .unwrap()
            .worksheet_drawing()
            .one_cell_anchor_collection()
            .len(),
        1
    );
    book.insert_new_row("Data", 2, 1);
    assert_eq!(
        formula_counts(&fixture::chart_xml(&book)),
        expected_roles("Data", COLUMNS, Some((3, 6)))
    );
    let exported = fixture::write_workbook(&book);
    let mut archive = zip::ZipArchive::new(Cursor::new(&exported)).unwrap();
    let mut drawing = String::new();
    archive
        .by_name("xl/drawings/drawing1.xml")
        .unwrap()
        .read_to_string(&mut drawing)
        .unwrap();
    assert!(drawing.contains("<xdr:oneCellAnchor>"), "{drawing}");
    assert!(!drawing.contains("<xdr:twoCellAnchor>"), "{drawing}");
    assert!(drawing.contains("cx=\"1200\" cy=\"2400\""), "{drawing}");
}
