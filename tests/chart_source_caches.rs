#![cfg(feature = "serde")]

use std::{
    fmt::Write,
    io::{
        Cursor,
        Read,
    },
};

use quick_xml::escape::{
    escape,
    partial_escape,
};
use umya_spreadsheet::{
    Chart,
    ChartType,
    Workbook,
    drawing::{
        charts::{
            CategoryAxisData,
            MAX_SOURCE_CACHE_POINTS,
            MAX_SOURCE_CACHE_TEXT_BYTES,
            StringReference,
        },
        spreadsheet::MarkerType,
    },
};

fn marker(coordinate: &str) -> MarkerType {
    let mut marker = MarkerType::default();
    marker.set_coordinate(coordinate);
    marker
}

fn write(book: &Workbook) -> Vec<u8> {
    let mut bytes = Vec::new();
    umya_spreadsheet::writer::xlsx::write_writer(book, &mut Cursor::new(&mut bytes)).unwrap();
    bytes
}

fn read_book(bytes: &[u8]) -> Workbook {
    umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap()
}

fn part(bytes: &[u8], name: &str) -> String {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut xml = String::new();
    archive
        .by_name(name)
        .unwrap()
        .read_to_string(&mut xml)
        .unwrap();
    xml
}

fn rewrite_part(bytes: &[u8], name: &str, edit: impl FnOnce(String) -> String) -> Vec<u8> {
    let mut source = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let mut edit = Some(edit);
    for index in 0..source.len() {
        let mut entry = source.by_index(index).unwrap();
        let mut data = Vec::new();
        entry.read_to_end(&mut data).unwrap();
        if entry.name() == name {
            data = edit.take().unwrap()(String::from_utf8(data).unwrap()).into_bytes();
        }
        output
            .start_file(entry.name(), zip::write::SimpleFileOptions::default())
            .unwrap();
        std::io::Write::write_all(&mut output, &data).unwrap();
    }
    assert!(edit.is_none(), "{name} was not rewritten");
    output.finish().unwrap().into_inner()
}

/// Replaces the cache of the `n`-th `c:cat` of `xml` with `cache`.
fn replace_category_cache(xml: &str, n: usize, cache: &str) -> String {
    let mut from = 0;
    let mut category = 0;
    for _ in 0..=n {
        category = from + xml[from..].find("<c:cat>").unwrap();
        from = category + 1;
    }
    let start = category + xml[category..].find("<c:strCache>").unwrap();
    let end = start + xml[start..].find("</c:strCache>").unwrap() + "</c:strCache>".len();
    format!("{}{cache}{}", &xml[..start], &xml[end..])
}

/// The `c:strRef` of the `n`-th category reference of a chart part.
fn category_reference(xml: &str, n: usize) -> &str {
    xml.split("<c:cat>")
        .nth(n + 1)
        .unwrap()
        .split_once("</c:cat>")
        .unwrap()
        .0
}

/// A `c:strCache` of points as a file stores them. A point without a value has
/// no `c:v`.
fn cache_xml(count: Option<usize>, points: &[Point]) -> String {
    let mut cache = String::from("<c:strCache>");
    if let Some(count) = count {
        write!(cache, "<c:ptCount val=\"{count}\"/>").unwrap();
    }
    for point in points {
        let written = match &point.value {
            Some(value) => write!(
                cache,
                "<c:pt idx=\"{}\"><c:v>{}</c:v></c:pt>",
                point.index,
                escape(value)
            ),
            None => write!(cache, "<c:pt idx=\"{}\"/>", point.index),
        };
        written.unwrap();
    }
    cache.push_str("</c:strCache>");
    cache
}

fn dense_cache(count: usize, label: impl Fn(usize) -> String) -> String {
    let points: Vec<Point> = (0..count)
        .map(|index| Point {
            index,
            value: Some(label(index)),
        })
        .collect();
    cache_xml(Some(count), &points)
}

fn reference_xml(formula: &str, cache: &str) -> String {
    format!(
        "<c:strRef><c:f>{}</c:f>{cache}</c:strRef>",
        partial_escape(formula)
    )
}

/// A line chart on `sheet` of `book` with one series per formula in
/// `categories`.
fn add_chart(book: &mut Workbook, sheet: &str, categories: &[String]) {
    let mut chart = Chart::default();
    chart.new_chart(
        &ChartType::LineChart,
        marker("H1"),
        marker("P18"),
        vec!["Sheet1!$Z$1:$Z$3"; categories.len()],
    );
    for (series, formula) in categories.iter().enumerate() {
        let mut data = CategoryAxisData::default();
        let mut reference = StringReference::default();
        reference.formula_mut().set_address_str(formula.clone());
        data.set_string_reference(reference);
        chart.area_chart_series_list_mut().area_chart_series_mut()[series]
            .set_category_axis_data(data);
    }
    book.sheet_by_name_mut(sheet).unwrap().add_chart(chart);
}

/// `rows` numbers `1..=rows` in each of `columns` columns of `Sheet1`.
fn number_workbook(columns: u32, rows: u32) -> Workbook {
    let mut book = umya_spreadsheet::new_file();
    let sheet = book.sheet_mut(0).unwrap();
    for column in 1..=columns {
        for row in 1..=rows {
            sheet.cell_mut((column, row)).set_value_number(row);
        }
    }
    book
}

fn column_range(column: char, rows: usize) -> String {
    format!("Sheet1!${column}$1:${column}${rows}")
}

/// A workbook whose first chart has one series per `(column range, cache)`.
fn one_chart_workbook(columns: u32, rows: u32, caches: &[(String, String)]) -> Vec<u8> {
    let mut book = number_workbook(columns, rows);
    let formulas: Vec<String> = caches.iter().map(|(formula, _)| formula.clone()).collect();
    add_chart(&mut book, "Sheet1", &formulas);
    rewrite_part(&write(&book), "xl/charts/chart1.xml", |mut xml| {
        for (index, (_, cache)) in caches.iter().enumerate() {
            xml = replace_category_cache(&xml, index, cache);
        }
        xml
    })
}

/// What the writer generates from numbered cells for `rows` rows.
fn generated(formula: &str, rows: usize) -> String {
    let mut cache = format!("<c:strCache><c:ptCount val=\"{rows}\"/>");
    for index in 0..rows {
        write!(
            cache,
            "<c:pt idx=\"{index}\"><c:v>{}</c:v></c:pt>",
            index + 1
        )
        .unwrap();
    }
    cache.push_str("</c:strCache>");
    reference_xml(formula, &cache)
}

fn text_bytes(label: impl Fn(usize) -> String, rows: usize) -> usize {
    (0..rows)
        .map(|index| label(index).len() + (index + 1).to_string().len())
        .sum()
}

/// The source caches found in `value`, a chart as JSON.
fn find_sources(value: &serde_json::Value, found: &mut Vec<serde_json::Value>) {
    match value {
        serde_json::Value::Object(map) => {
            if let Some(source) = map.get("source").filter(|source| !source.is_null()) {
                assert!(source.is_object(), "unexpected source: {source}");
                assert_eq!(source.as_object().unwrap().len(), 4, "{source}");
                assert!(source.get("point_count").is_some(), "{source}");
                assert!(source["formula"].is_string(), "{source}");
                assert!(source["index_runs"].is_array(), "{source}");
                assert!(source["exceptions"].is_array(), "{source}");
                assert!(
                    source["point_count"].is_null()
                        || source["point_count"]
                            .as_u64()
                            .is_some_and(|n| n <= u64::from(u32::MAX)),
                    "{source}"
                );
                for run in source["index_runs"].as_array().unwrap() {
                    assert_eq!(run.as_object().unwrap().len(), 2, "{run}");
                    for field in ["start", "len"] {
                        assert!(
                            run[field]
                                .as_u64()
                                .is_some_and(|n| n <= u64::from(u32::MAX)),
                            "{run}"
                        );
                    }
                }
                for point in source["exceptions"].as_array().unwrap() {
                    assert_eq!(point.as_object().unwrap().len(), 3, "{point}");
                    assert!(
                        point["index"]
                            .as_u64()
                            .is_some_and(|n| n <= u64::from(u32::MAX)),
                        "{point}"
                    );
                    for field in ["value", "loaded_display"] {
                        assert!(
                            point[field].is_null() || point[field].is_string(),
                            "{point}"
                        );
                    }
                }
                found.push(source.clone());
            }
            for child in map.values() {
                find_sources(child, found);
            }
        }
        serde_json::Value::Array(items) => {
            for child in items {
                find_sources(child, found);
            }
        }
        _ => {}
    }
}

/// The source caches stored in the charts of every worksheet, as JSON, in
/// worksheet and chart order.
fn stored_caches(book: &Workbook) -> Vec<serde_json::Value> {
    let mut found = Vec::new();
    for sheet in book.sheet_collection_no_check() {
        find_sources(
            &serde_json::to_value(sheet.worksheet_drawing()).unwrap(),
            &mut found,
        );
    }
    found
}

fn formulas(caches: &[serde_json::Value]) -> Vec<String> {
    caches
        .iter()
        .map(|cache| cache["formula"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn source_caches_over_the_budget_export_from_cell_display() {
    let label = |prefix: &'static str| move |index: usize| format!("{prefix}{}", index + 1);
    let series = |order: &[(char, usize, &'static str)]| -> Vec<(String, String)> {
        order
            .iter()
            .map(|(column, rows, prefix)| {
                (
                    column_range(*column, *rows),
                    dense_cache(*rows, label(prefix)),
                )
            })
            .collect()
    };
    let a = ('A', 6000, "North ");
    let b = ('B', 6000, "South ");
    let c = ('C', 6000, "East ");
    let d = ('D', 2000, "West ");

    // Whole caches are admitted in file order. The third cache does not fit
    // any more and is dropped whole; the smaller fourth one still does.
    let bytes = one_chart_workbook(4, 6000, &series(&[a, b, c, d]));
    let book = read_book(&bytes);
    let report = *book.chart_source_cache_report();
    let kept_bytes = text_bytes(label("North "), 6000)
        + text_bytes(label("South "), 6000)
        + text_bytes(label("West "), 2000);
    assert_eq!(report.kept_caches, 3);
    assert_eq!(report.kept_points, 14_000);
    assert_eq!(report.kept_text_bytes, kept_bytes);
    assert_eq!(report.dropped_caches, 1);
    assert_eq!((report.reduced_caches, report.over_cap_caches), (0, 1));
    reconcile_admission(&book);
    assert_eq!(report.over_cap_points, 6000);
    assert_eq!(report.over_cap_text_bytes, text_bytes(label("East "), 6000));

    let exported = part(&write(&book), "xl/charts/chart1.xml");
    for (index, (column, rows, prefix)) in [a, b, c, d].into_iter().enumerate() {
        let formula = column_range(column, rows);
        let expected = if column == 'C' {
            generated(&formula, rows)
        } else {
            reference_xml(&formula, &dense_cache(rows, label(prefix)))
        };
        assert_eq!(category_reference(&exported, index), expected, "{column}");
    }
    assert_eq!(
        formulas(&stored_caches(&book)),
        [
            column_range('A', 6000),
            column_range('B', 6000),
            column_range('D', 2000)
        ]
    );

    // The same bytes always admit the same caches, and the order is the
    // order of the file, not the size of the cache.
    let again = read_book(&bytes);
    assert_eq!(*again.chart_source_cache_report(), report);
    assert_eq!(stored_caches(&again), stored_caches(&book));
    let reversed = read_book(&one_chart_workbook(4, 6000, &series(&[d, c, b, a])));
    let report = reversed.chart_source_cache_report();
    assert_eq!((report.kept_caches, report.dropped_caches), (3, 1));
    assert_eq!(
        formulas(&stored_caches(&reversed)),
        [
            column_range('D', 2000),
            column_range('C', 6000),
            column_range('B', 6000),
        ]
    );
    let exported = part(&write(&reversed), "xl/charts/chart1.xml");
    assert_eq!(
        category_reference(&exported, 0),
        reference_xml(&column_range('D', 2000), &dense_cache(2000, label("West ")))
    );
    assert_eq!(
        category_reference(&exported, 1),
        reference_xml(&column_range('C', 6000), &dense_cache(6000, label("East ")))
    );
    assert_eq!(
        category_reference(&exported, 2),
        reference_xml(
            &column_range('B', 6000),
            &dense_cache(6000, label("South "))
        )
    );
    assert_eq!(
        category_reference(&exported, 3),
        generated(&column_range('A', 6000), 6000)
    );
}

/// A workbook whose `Sheet1` and `Second` each host one chart with one
/// category series over the given range, both with the given caches.
fn two_sheet_workbook(rows: usize, charts: [(&str, &str); 2]) -> Vec<u8> {
    let mut book = number_workbook(2, rows as u32);
    book.new_sheet("Second").unwrap();
    add_chart(&mut book, "Sheet1", &[charts[0].0.to_string()]);
    add_chart(&mut book, "Second", &[charts[1].0.to_string()]);
    let mut bytes = write(&book);
    for (index, (_, cache)) in charts.iter().enumerate() {
        bytes = rewrite_part(
            &bytes,
            &format!("xl/charts/chart{}.xml", index + 1),
            |xml| replace_category_cache(&xml, 0, cache),
        );
    }
    bytes
}

#[test]
fn source_cache_budget_is_per_workbook_in_worksheet_order() {
    let rows = 10_000;
    let cache = dense_cache(rows, |index| format!("Label {}", index + 1));
    let first = column_range('A', rows);
    let second = column_range('B', rows);
    let bytes = two_sheet_workbook(rows, [(&first, &cache), (&second, &cache)]);
    let reversed = rewrite_part(&bytes, "xl/workbook.xml", |xml| {
        let start = xml.find("<sheets>").unwrap() + "<sheets>".len();
        let middle = start + xml[start..].find("/>").unwrap() + 2;
        let end = xml.find("</sheets>").unwrap();
        format!(
            "{}{}{}{}",
            &xml[..start],
            &xml[middle..end],
            &xml[start..middle],
            &xml[end..]
        )
    });
    for (bytes, formulas) in [(&bytes, [&first, &second]), (&reversed, [&second, &first])] {
        let book = read_book(bytes);
        let report = book.chart_source_cache_report();
        assert_eq!((report.kept_caches, report.dropped_caches), (1, 1));
        assert_eq!(
            (report.kept_points, report.over_cap_points),
            (10_000, 10_000)
        );
        assert_eq!(
            report.kept_text_bytes,
            text_bytes(|index| format!("Label {}", index + 1), rows)
        );
        assert_eq!(report.over_cap_text_bytes, report.kept_text_bytes);
        assert_eq!(self::formulas(&stored_caches(&book)), [formulas[0].clone()]);
        let output = write(&book);
        assert_eq!(
            category_reference(&part(&output, "xl/charts/chart1.xml"), 0),
            reference_xml(formulas[0], &cache)
        );
        assert_eq!(
            category_reference(&part(&output, "xl/charts/chart2.xml"), 0),
            generated(formulas[1], rows)
        );
        reconcile_admission(&book);
    }
}

#[test]
fn stored_points_count_every_series_that_stores_them() {
    // Two series over the same range store the same points twice.
    let rows = 10_000;
    let formula = column_range('A', rows);
    let cache = dense_cache(rows, |index| format!("Label {}", index + 1));
    let book = read_book(&one_chart_workbook(
        1,
        rows as u32,
        &[
            (formula.clone(), cache.clone()),
            (formula.clone(), cache.clone()),
        ],
    ));
    let report = book.chart_source_cache_report();
    assert_eq!((report.kept_caches, report.dropped_caches), (1, 1));
    assert_eq!(report.kept_points, 10_000);
    let exported = part(&write(&book), "xl/charts/chart1.xml");
    assert_eq!(
        category_reference(&exported, 0),
        reference_xml(&formula, &cache)
    );
    assert_eq!(category_reference(&exported, 1), generated(&formula, rows));
}

#[test]
fn every_series_keeps_the_cache_its_own_file_stored() {
    let rows = 5;
    let formula = column_range('A', rows);
    let first = dense_cache(rows, |index| format!("First {index}"));
    let second = dense_cache(rows, |index| format!("Second {index}"));
    let book = read_book(&one_chart_workbook(
        1,
        rows as u32,
        &[
            (formula.clone(), first.clone()),
            (formula.clone(), second.clone()),
        ],
    ));
    assert_eq!(book.chart_source_cache_report().kept_caches, 2);
    let exported = part(&write(&book), "xl/charts/chart1.xml");
    assert_eq!(
        category_reference(&exported, 0),
        reference_xml(&formula, &first)
    );
    assert_eq!(
        category_reference(&exported, 1),
        reference_xml(&formula, &second)
    );
}

#[test]
fn a_renamed_sheet_exports_the_current_display_not_stale_text() {
    let rows = 12;
    let cache = dense_cache(rows, |index| format!("Q {}", index + 1));
    let mut book = read_book(&one_chart_workbook(
        1,
        rows as u32,
        &[(column_range('A', rows), cache.clone())],
    ));
    assert_eq!(stored_caches(&book).len(), 1);
    let formula = column_range('A', rows);
    assert_eq!(
        category_reference(&part(&write(&book), "xl/charts/chart1.xml"), 0),
        reference_xml(&formula, &cache)
    );

    book.sheet_by_name_mut("Sheet1")
        .unwrap()
        .set_name("Renamed");
    // The chart still points at the old name: no stale labels either.
    let exported = part(&write(&book), "xl/charts/chart1.xml");
    let empty = dense_cache(rows, |_| String::new());
    assert_eq!(
        category_reference(&exported, 0),
        reference_xml(&formula, &empty)
    );

    // Once the chart follows the rename, the cells are what it shows.
    book.sheet_by_name_mut("Renamed")
        .unwrap()
        .cell_mut("A3")
        .set_value_number(42);
    book.sheet_by_name_mut("Renamed")
        .unwrap()
        .chart_collection_mut()[0]
        .area_chart_series_list_mut()
        .area_chart_series_mut()[0]
        .category_axis_data_mut()
        .unwrap()
        .string_reference_mut()
        .unwrap()
        .formula_mut()
        .set_address_str("Renamed!$A$1:$A$12");
    assert_eq!(
        category_reference(&part(&write(&book), "xl/charts/chart1.xml"), 0),
        reference_xml(
            "Renamed!$A$1:$A$12",
            &dense_cache(12, |index| {
                if index == 2 {
                    "42".into()
                } else {
                    (index + 1).to_string()
                }
            })
        )
    );
}

#[test]
fn source_cache_budget_limits_are_exact() {
    // One category cell displaying "1", so a label of `n` bytes stores
    // `n + 1` text bytes.
    let single = |label_bytes: usize| {
        let bytes = one_chart_workbook(
            1,
            1,
            &[(
                column_range('A', 1),
                dense_cache(1, |_| "x".repeat(label_bytes)),
            )],
        );
        read_book(&bytes)
    };
    let exactly = single(MAX_SOURCE_CACHE_TEXT_BYTES - 1);
    let report = exactly.chart_source_cache_report();
    assert_eq!(report.kept_text_bytes, MAX_SOURCE_CACHE_TEXT_BYTES);
    assert_eq!((report.kept_caches, report.dropped_caches), (1, 0));

    let over = single(MAX_SOURCE_CACHE_TEXT_BYTES);
    let report = over.chart_source_cache_report();
    assert_eq!(report.kept_text_bytes, 0);
    assert_eq!(report.over_cap_text_bytes, MAX_SOURCE_CACHE_TEXT_BYTES + 1);
    assert_eq!((report.kept_caches, report.dropped_caches), (0, 1));
    assert!(stored_caches(&over).is_empty());

    for (label, fits) in [
        ("\u{e9}".repeat(524_287) + "x", true),
        ("\u{e9}".repeat(524_288), false),
    ] {
        let formula = column_range('A', 1);
        let cache = dense_cache(1, |_| label.clone());
        let book = read_book(&one_chart_workbook(
            1,
            1,
            &[(formula.clone(), cache.clone())],
        ));
        let report = book.chart_source_cache_report();
        if fits {
            assert_eq!(report.kept_text_bytes, MAX_SOURCE_CACHE_TEXT_BYTES);
            assert_eq!((report.kept_caches, report.over_cap_caches), (1, 0));
        } else {
            assert_eq!(report.over_cap_text_bytes, MAX_SOURCE_CACHE_TEXT_BYTES + 1);
            assert_eq!((report.kept_caches, report.over_cap_caches), (0, 1));
        }
        let expected = if fits {
            reference_xml(&formula, &cache)
        } else {
            generated(&formula, 1)
        };
        assert_eq!(
            category_reference(&part(&write(&book), "xl/charts/chart1.xml"), 0),
            expected
        );
        reconcile_admission(&book);
    }
    assert_eq!(
        category_reference(&part(&write(&exactly), "xl/charts/chart1.xml"), 0),
        reference_xml(
            &column_range('A', 1),
            &dense_cache(1, |_| "x".repeat(MAX_SOURCE_CACHE_TEXT_BYTES - 1))
        )
    );
    assert_eq!(
        category_reference(&part(&write(&over), "xl/charts/chart1.xml"), 0),
        generated(&column_range('A', 1), 1)
    );

    // Long labels fill the text budget long before the point budget.
    let long = |index: usize| format!("{index:0>1000}");
    let mut caches: Vec<(String, String)> = ['A', 'B', 'C', 'D']
        .into_iter()
        .map(|column| (column_range(column, 300), dense_cache(300, long)))
        .collect();
    caches.push((column_range('A', 1), dense_cache(1, |_| "tail".into())));
    let book = read_book(&one_chart_workbook(4, 300, &caches));
    let per_cache = text_bytes(long, 300);
    assert_eq!(per_cache, 300_792);
    assert!(
        per_cache * 4 > MAX_SOURCE_CACHE_TEXT_BYTES && per_cache * 3 < MAX_SOURCE_CACHE_TEXT_BYTES
    );
    let report = book.chart_source_cache_report();
    assert_eq!((report.kept_caches, report.dropped_caches), (4, 1));
    assert_eq!(report.kept_text_bytes, 902_381);
    assert_eq!(report.kept_points, 901);
    assert_eq!(report.over_cap_text_bytes, per_cache);
    assert_eq!((report.over_cap_caches, report.over_cap_points), (1, 300));
    let exported = part(&write(&book), "xl/charts/chart1.xml");
    for (index, (formula, cache)) in caches.iter().enumerate() {
        let expected = if index == 3 {
            generated(formula, 300)
        } else {
            reference_xml(formula, cache)
        };
        assert_eq!(category_reference(&exported, index), expected);
    }

    // The point budget is exact too.
    let points = |count: usize| {
        let bytes = one_chart_workbook(
            1,
            count as u32,
            &[(
                column_range('A', count),
                dense_cache(count, |index| format!("p{index}")),
            )],
        );
        read_book(&bytes)
    };
    let exactly = points(MAX_SOURCE_CACHE_POINTS);
    let report = exactly.chart_source_cache_report();
    assert_eq!(report.kept_points, MAX_SOURCE_CACHE_POINTS);
    assert_eq!((report.kept_caches, report.dropped_caches), (1, 0));
    let over = points(MAX_SOURCE_CACHE_POINTS + 1);
    let report = over.chart_source_cache_report();
    assert_eq!(report.kept_points, 0);
    assert_eq!(report.over_cap_points, MAX_SOURCE_CACHE_POINTS + 1);
    assert_eq!((report.kept_caches, report.dropped_caches), (0, 1));
    for (book, count, admitted) in [
        (&exactly, MAX_SOURCE_CACHE_POINTS, true),
        (&over, MAX_SOURCE_CACHE_POINTS + 1, false),
    ] {
        let formula = column_range('A', count);
        let expected = if admitted {
            reference_xml(&formula, &dense_cache(count, |index| format!("p{index}")))
        } else {
            generated(&formula, count)
        };
        assert_eq!(
            category_reference(&part(&write(book), "xl/charts/chart1.xml"), 0),
            expected
        );
        reconcile_admission(book);
    }
}

#[test]
fn caches_the_cells_reproduce_are_not_stored() {
    let rows = 50;
    let formula = column_range('A', rows);
    let bytes = one_chart_workbook(
        1,
        rows as u32,
        &[(
            formula.clone(),
            dense_cache(rows, |index| (index + 1).to_string()),
        )],
    );
    let book = read_book(&bytes);
    let report = book.chart_source_cache_report();
    assert_eq!((report.reduced_caches, report.dropped_caches), (1, 1));
    assert_eq!((report.kept_caches, report.over_cap_caches), (0, 0));
    assert!(stored_caches(&book).is_empty());
    assert_eq!(
        category_reference(&part(&write(&book), "xl/charts/chart1.xml"), 0),
        generated(&formula, rows)
    );
}

/// A workbook read from a file whose category cache labels `Q 1..Q 12` the
/// numbers 1..12, and the stored chart of its first sheet.
fn quarter_book() -> (Workbook, String, String) {
    let rows = 12;
    let formula = column_range('A', rows);
    let labels = dense_cache(rows, |index| format!("Q {}", index + 1));
    let book = read_book(&one_chart_workbook(
        1,
        rows as u32,
        &[(formula.clone(), labels.clone())],
    ));
    (book, formula, labels)
}

/// Writes `chart` on a fresh sheet of numbers, as the server would.
fn export_chart(chart: &Chart) -> String {
    let mut book = number_workbook(1, 12);
    book.sheet_mut(0).unwrap().add_chart(chart.clone());
    category_reference(&part(&write(&book), "xl/charts/chart1.xml"), 0).to_string()
}

#[test]
fn strip_removes_source_caches() {
    let (book, formula, labels) = quarter_book();
    let stored = &book.sheet(0).unwrap().chart_collection()[0];
    assert_eq!(export_chart(stored), reference_xml(&formula, &labels));

    let mut stripped = stored.clone();
    stripped.strip_source_caches();
    let json = serde_json::to_value(&stripped).unwrap();
    let mut found = Vec::new();
    find_sources(&json, &mut found);
    assert!(found.is_empty(), "{json}");
    assert_eq!(export_chart(&stripped), generated(&formula, 12));
    // The stored chart keeps its caches.
    assert_eq!(stored_caches(&book).len(), 1);
}

#[test]
fn adopt_keeps_server_source_caches() {
    let (book, formula, labels) = quarter_book();
    let stored = &book.sheet(0).unwrap().chart_collection()[0];

    // A client gets the chart without caches and sends it back changed
    // elsewhere: the server's labels come back.
    let mut client = stored.clone();
    client.strip_source_caches();
    client.adopt_source_caches(stored);
    assert_eq!(export_chart(&client), reference_xml(&formula, &labels));

    // A reference that now has another formula has no cache to keep.
    let mut moved = stored.clone();
    moved.strip_source_caches();
    moved.area_chart_series_list_mut().area_chart_series_mut()[0]
        .category_axis_data_mut()
        .unwrap()
        .string_reference_mut()
        .unwrap()
        .formula_mut()
        .set_address_str("Sheet1!$A$1:$A$11");
    moved.adopt_source_caches(stored);
    assert_eq!(export_chart(&moved), generated("Sheet1!$A$1:$A$11", 11));
}

#[test]
fn adopt_ignores_client_supplied_caches() {
    let (book, formula, labels) = quarter_book();
    let stored = &book.sheet(0).unwrap().chart_collection()[0];

    // The client's chart carries a cache for the same formula, but a
    // different one: the stored chart's is the one kept.
    let forged_labels = dense_cache(12, |index| format!("Forged {}", index + 1));
    let forged_book = read_book(&one_chart_workbook(
        1,
        12,
        &[(formula.clone(), forged_labels.clone())],
    ));
    let mut client = forged_book.sheet(0).unwrap().chart_collection()[0].clone();
    assert_eq!(
        export_chart(&client),
        reference_xml(&formula, &forged_labels)
    );
    client.adopt_source_caches(stored);
    assert_eq!(export_chart(&client), reference_xml(&formula, &labels));

    // And when the stored chart has none for it, the client's is dropped.
    let mut bare = stored.clone();
    bare.strip_source_caches();
    let mut client = forged_book.sheet(0).unwrap().chart_collection()[0].clone();
    client.adopt_source_caches(&bare);
    assert_eq!(export_chart(&client), generated(&formula, 12));

    // A chart created by a client has none either.
    let mut created = forged_book.sheet(0).unwrap().chart_collection()[0].clone();
    created.clear_source_caches();
    assert_eq!(export_chart(&created), generated(&formula, 12));

    let mut unmatched = forged_book.sheet(0).unwrap().chart_collection()[0].clone();
    unmatched
        .area_chart_series_list_mut()
        .area_chart_series_mut()[0]
        .category_axis_data_mut()
        .unwrap()
        .string_reference_mut()
        .unwrap()
        .formula_mut()
        .set_address_str("Sheet1!$A$1:$A$11");
    unmatched.adopt_source_caches(stored);
    assert_eq!(export_chart(&unmatched), generated("Sheet1!$A$1:$A$11", 11));
}

/// A small deterministic generator so the cases are the same on every run.
struct Random(u64);

impl Random {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, bound: usize) -> usize {
        usize::try_from(self.next() % bound as u64).unwrap()
    }

    fn chance(&mut self, percent: usize) -> bool {
        self.below(100) < percent
    }
}

/// One stored point of the generated cache.
struct Point {
    index: usize,
    value: Option<String>,
}

/// What a cell shows. `None` is an empty cell.
type Shown = Option<String>;

fn shown(cells: &[Shown], index: usize) -> Option<String> {
    cells
        .get(index)
        .map(|cell| cell.clone().unwrap_or_default())
}

/// The reference rule: a point keeps the text the file stored while its cell
/// still shows what it showed at load, and shows the cell's text otherwise.
fn reference_model(
    count: Option<usize>,
    points: &[Point],
    loaded: &[Shown],
    now: &[Shown],
) -> String {
    let mut cache = String::from("<c:strCache>");
    if let Some(count) = count {
        write!(cache, "<c:ptCount val=\"{count}\"/>").unwrap();
    }
    for point in points {
        let value = if shown(now, point.index) == shown(loaded, point.index) {
            point.value.clone()
        } else {
            Some(shown(now, point.index).unwrap_or_default())
        };
        write!(cache, "<c:pt idx=\"{}\">", point.index).unwrap();
        if let Some(value) = value {
            write!(cache, "<c:v>{}</c:v>", escape(value)).unwrap();
        }
        cache.push_str("</c:pt>");
    }
    cache.push_str("</c:strCache>");
    cache
}

fn label(random: &mut Random) -> String {
    let words = ["Q", "North", "a & b", "<x>", " pad ", "Zeta", "7"];
    format!("{} {}", words[random.below(words.len())], random.below(30))
}

#[test]
fn sparse_source_caches_preserve_labels_until_cells_change_over_random_edits() {
    let mut random = Random(0x9E37_79B9_7F4A_7C15);
    for case in 0..250 {
        let data_rows = 1 + random.below(14);
        let range_rows = data_rows + random.below(4);
        let formula = column_range('A', range_rows);

        // The cells at load: numbers, text, or nothing.
        let loaded: Vec<Shown> = (0..range_rows)
            .map(|row| {
                if row >= data_rows || random.chance(10) {
                    None
                } else if random.chance(25) {
                    Some(label(&mut random))
                } else {
                    Some((100 + random.below(900)).to_string())
                }
            })
            .collect();
        let mut book = umya_spreadsheet::new_file();
        for (row, cell) in loaded.iter().enumerate() {
            let target = book.sheet_mut(0).unwrap().cell_mut((1, row as u32 + 1));
            match cell {
                Some(text) if text.parse::<u32>().is_ok() => {
                    target.set_value_number(text.parse::<u32>().unwrap());
                }
                Some(text) => {
                    target.set_value_string(text.clone());
                }
                None => {}
            }
        }
        add_chart(&mut book, "Sheet1", std::slice::from_ref(&formula));

        // The cache the file stored: any order, gaps, repeats, texts that
        // match the cells or not, points without a value.
        let mut points = Vec::new();
        for _ in 0..random.below(range_rows + 4) {
            let index = match random.below(10) {
                0 => range_rows + random.below(3),
                1 if !points.is_empty() => points.len() - 1 - random.below(points.len()),
                _ => random.below(range_rows),
            };
            let value = match random.below(4) {
                0 => None,
                1 => Some(label(&mut random)),
                _ => shown(&loaded, index).or_else(|| Some(String::new())),
            };
            points.push(Point { index, value });
        }
        let count = match random.below(3) {
            0 => None,
            1 => Some(range_rows),
            _ => Some(points.len() + random.below(5)),
        };
        let cache = cache_xml(count, &points);
        let bytes = rewrite_part(&write(&book), "xl/charts/chart1.xml", |xml| {
            replace_category_cache(&xml, 0, &cache)
        });

        // Random edits after the load.
        let mut book = read_book(&bytes);
        let mut now = loaded.clone();
        for _ in 0..random.below(6) {
            let row = random.below(range_rows);
            let target = book.sheet_mut(0).unwrap().cell_mut((1, row as u32 + 1));
            if random.chance(50) {
                let number = 100 + random.below(900);
                target.set_value_number(number as u32);
                now[row] = Some(number.to_string());
            } else {
                let text = label(&mut random);
                target.set_value_string(text.clone());
                now[row] = Some(text);
            }
        }

        let expected = if case % 7 == 0 {
            let sheet = book.sheet_mut(0).unwrap();
            for (row, value) in [(1, 7), (2, 8), (3, 9)] {
                sheet.cell_mut((2, row)).set_value_number(value);
            }
            let changed = "Sheet1!$B$1:$B$3";
            sheet.chart_collection_mut()[0]
                .area_chart_series_list_mut()
                .area_chart_series_mut()[0]
                .category_axis_data_mut()
                .unwrap()
                .string_reference_mut()
                .unwrap()
                .formula_mut()
                .set_address_str(changed);
            reference_xml(changed, &dense_cache(3, |index| (index + 7).to_string()))
        } else {
            if let Some(row) = now.iter().position(|value| {
                value
                    .as_ref()
                    .is_some_and(|value| value.parse::<u32>().is_ok())
            }) {
                let unchanged = case % 2 == 0;
                book.sheet_mut(0)
                    .unwrap()
                    .cell_mut((1, row as u32 + 1))
                    .style_mut()
                    .number_format_mut()
                    .set_format_code(if unchanged { "0" } else { "0.00" });
                if !unchanged {
                    now[row] = Some(format!("{}.00", now[row].as_ref().unwrap()));
                }
            }
            reference_xml(&formula, &reference_model(count, &points, &loaded, &now))
        };
        let exported = part(&write(&book), "xl/charts/chart1.xml");
        assert_eq!(category_reference(&exported, 0), expected, "case {case}");
    }
}

#[test]
fn shape_only_caches_and_duplicate_occurrences_are_charged() {
    let formula = column_range('A', 4);
    let points: Vec<Point> = [0, 2, 3]
        .into_iter()
        .map(|index| Point {
            index,
            value: Some((index + 1).to_string()),
        })
        .collect();
    let cache = cache_xml(Some(4), &points);
    let book = read_book(&one_chart_workbook(
        1,
        4,
        &[(formula.clone(), cache.clone())],
    ));
    let report = book.chart_source_cache_report();
    assert_eq!(
        (
            report.kept_caches,
            report.kept_points,
            report.kept_text_bytes
        ),
        (1, 2, 0)
    );
    let stored = stored_caches(&book);
    assert_eq!(
        stored[0]["index_runs"],
        serde_json::json!([
            {"start": 0, "len": 1}, {"start": 2, "len": 2}
        ])
    );
    assert_eq!(stored[0]["exceptions"], serde_json::json!([]));
    assert_eq!(
        category_reference(&part(&write(&book), "xl/charts/chart1.xml"), 0),
        reference_xml(&formula, &cache)
    );

    // Keep both occurrences even when the first equals the load display.
    let points = [
        Point {
            index: 0,
            value: Some("1".into()),
        },
        Point {
            index: 0,
            value: Some("other".into()),
        },
    ];
    let cache = cache_xml(Some(2), &points);
    let book = read_book(&one_chart_workbook(1, 4, &[(formula.clone(), cache)]));
    let report = book.chart_source_cache_report();
    assert_eq!((report.kept_points, report.kept_text_bytes), (4, 8));
    assert_eq!(
        category_reference(&part(&write(&book), "xl/charts/chart1.xml"), 0),
        reference_xml(
            &formula,
            &reference_model(Some(2), &points, &[Some("1".into())], &[Some("1".into())])
        )
    );
}

#[test]
fn lazy_admission_follows_opening_order_and_reports_incrementally() {
    let first = column_range('A', 10_000);
    let second = column_range('B', 10_000);
    let cache = dense_cache(10_000, |index| format!("Label {index}"));
    let bytes = two_sheet_workbook(10_000, [(&first, &cache), (&second, &cache)]);
    let eager = read_book(&bytes);
    assert_eq!(formulas(&stored_caches(&eager)), [first.clone()]);
    let mut lazy = umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(&bytes), false).unwrap();
    assert_eq!(lazy.chart_source_cache_report().kept_caches, 0);
    assert!(stored_caches(&lazy).is_empty());
    lazy.sheet_mut(1).unwrap();
    assert_eq!(lazy.chart_source_cache_report().kept_caches, 1);
    assert_eq!(lazy.chart_source_cache_report().over_cap_caches, 0);
    assert_eq!(formulas(&stored_caches(&lazy)), [second.clone()]);
    lazy.sheet_mut(0).unwrap();
    assert_eq!(
        lazy.chart_source_cache_report(),
        eager.chart_source_cache_report()
    );
    assert_eq!(formulas(&stored_caches(&lazy)), [second.clone()]);
    for (name, formula, admitted) in [
        ("xl/charts/chart1.xml", &first, false),
        ("xl/charts/chart2.xml", &second, true),
    ] {
        let expected = if admitted {
            reference_xml(formula, &cache)
        } else {
            generated(formula, 10_000)
        };
        assert_eq!(category_reference(&part(&write(&lazy), name), 0), expected);
    }
    let report = *lazy.chart_source_cache_report();
    lazy.read_sheet(1).read_sheet(0).read_sheet_collection();
    assert_eq!(*lazy.chart_source_cache_report(), report);
}

#[test]
fn earlier_materialized_sheets_never_supply_a_new_import_baseline() {
    let formula = column_range('A', 3);
    let small = dense_cache(3, |index| format!("Q {}", index + 1));
    let bytes = two_sheet_workbook(3, [(&formula, &small), (&formula, &small)]);
    for edited in [false, true] {
        let mut lazy =
            umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(&bytes), false).unwrap();
        lazy.read_sheet(0);
        if edited {
            lazy.sheet_mut(0)
                .unwrap()
                .cell_mut("A2")
                .set_value_number(42);
        }
        lazy.read_sheet(1);
        assert_eq!(lazy.chart_source_cache_report().kept_caches, 1);
        assert_eq!(lazy.chart_source_cache_report().reduced_caches, 1);
        assert_eq!(stored_caches(&lazy).len(), 1);
        let expected = dense_cache(3, |index| {
            if edited && index == 1 {
                "42".into()
            } else {
                (index + 1).to_string()
            }
        });
        assert_eq!(
            category_reference(&part(&write(&lazy), "xl/charts/chart2.xml"), 0),
            reference_xml(&formula, &expected)
        );
    }
    // A collection call materializes all fresh sheets before capturing.
    let eager = read_book(&bytes);
    let mut lazy = umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(&bytes), false).unwrap();
    lazy.read_sheet_collection();
    assert_eq!(stored_caches(&lazy), stored_caches(&eager));
    assert_eq!(
        lazy.chart_source_cache_report(),
        eager.chart_source_cache_report()
    );
    assert_eq!(
        part(&write(&lazy), "xl/charts/chart2.xml"),
        part(&write(&eager), "xl/charts/chart2.xml")
    );
}

#[test]
fn reproducible_categories_on_a_raw_sheet_export_their_labels() {
    let formula = column_range('A', 3);
    let small = dense_cache(3, |index| (index + 1).to_string());
    let bytes = two_sheet_workbook(3, [(&formula, &small), (&formula, &small)]);
    let mut lazy = umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(&bytes), false).unwrap();
    lazy.read_sheet(1);
    let caches = stored_caches(&lazy);
    assert_eq!(caches.len(), 1);
    assert_eq!(caches[0]["point_count"], 3);
    assert_eq!(caches[0]["exceptions"], serde_json::json!([]));
    assert_eq!(caches[0]["index_runs"], serde_json::json!([]));
    let report = *lazy.chart_source_cache_report();
    assert_eq!(
        (
            report.kept_caches,
            report.kept_points,
            report.kept_text_bytes
        ),
        (1, 0, 0)
    );
    assert!(lazy.sheet(0).is_err());
    assert_eq!(
        category_reference(&part(&write(&lazy), "xl/charts/chart2.xml"), 0),
        reference_xml(&formula, &small)
    );
    assert!(lazy.sheet(0).is_err());
    assert_eq!(*lazy.chart_source_cache_report(), report);
    assert_eq!(
        read_book(&bytes).chart_source_cache_report().reduced_caches,
        2
    );
}

#[test]
fn adoption_preserves_formula_occurrences_and_empty_slots_without_multiplication() {
    let formula = column_range('A', 3);
    let first = dense_cache(3, |index| format!("First {index}"));
    let second = dense_cache(3, |index| format!("Second {index}"));
    let generated_cache = dense_cache(3, |index| (index + 1).to_string());
    let caches = [first.clone(), generated_cache.clone(), second.clone()]
        .map(|cache| (formula.clone(), cache));
    let book = read_book(&one_chart_workbook(1, 3, &caches));
    let stored = &book.sheet(0).unwrap().chart_collection()[0];
    let before = serde_json::to_vec(stored).unwrap();
    let mut client = stored.clone();
    client.strip_source_caches();
    let mut extra = client.area_chart_series_list_mut().area_chart_series_mut()[0].clone();
    extra.index_mut().set_val(3);
    client
        .area_chart_series_list_mut()
        .add_area_chart_series(extra);
    client.set_title("Client edit");
    client.adopt_source_caches(stored);
    let mut target = number_workbook(1, 3);
    target.sheet_mut(0).unwrap().add_chart(client);
    let xml = part(&write(&target), "xl/charts/chart1.xml");
    assert!(xml.contains("Client edit"));
    for (index, cache) in [first, generated_cache.clone(), second, generated_cache]
        .iter()
        .enumerate()
    {
        assert_eq!(
            category_reference(&xml, index),
            reference_xml(&formula, cache)
        );
    }
    assert_eq!(serde_json::to_vec(stored).unwrap(), before);

    // An old cache whose reference has been retargeted is never adopted.
    let mut previous = stored.clone();
    previous
        .area_chart_series_list_mut()
        .area_chart_series_mut()[0]
        .category_axis_data_mut()
        .unwrap()
        .string_reference_mut()
        .unwrap()
        .formula_mut()
        .set_address_str("Sheet1!$A$1:$A$2");
    let mut client = previous.clone();
    client.adopt_source_caches(&previous);
    let mut found = Vec::new();
    find_sources(&serde_json::to_value(&client).unwrap(), &mut found);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0]["exceptions"][0]["value"], "Second 0");
}

#[test]
fn strip_and_clear_are_idempotent_and_preserve_other_chart_content() {
    let (book, ..) = quarter_book();
    let original = &book.sheet(0).unwrap().chart_collection()[0];
    let restored: Chart = bincode::deserialize(&bincode::serialize(original).unwrap()).unwrap();
    assert_eq!(
        serde_json::to_value(&restored).unwrap(),
        serde_json::to_value(original).unwrap()
    );
    let mut stripped = original.clone();
    stripped.strip_source_caches();
    let bytes = bincode::serialize(&stripped).unwrap();
    let json = serde_json::to_vec(&stripped).unwrap();
    stripped.strip_source_caches();
    assert_eq!(bincode::serialize(&stripped).unwrap(), bytes);
    assert_eq!(serde_json::to_vec(&stripped).unwrap(), json);
    let mut cleared = original.clone();
    cleared.clear_source_caches();
    assert_eq!(serde_json::to_vec(&cleared).unwrap(), json);
    cleared.clear_source_caches();
    assert_eq!(serde_json::to_vec(&cleared).unwrap(), json);
    let mut found = Vec::new();
    find_sources(&serde_json::to_value(&cleared).unwrap(), &mut found);
    assert!(found.is_empty());
    assert_eq!(book.chart_source_cache_report().kept_caches, 1);
}

#[test]
fn stored_caches_follow_the_display_rule_for_shapes_formats_and_unavailable_cells() {
    let formula = column_range('A', 4);
    let loaded = [Some("1".into()), None, Some("3".into()), Some("4".into())];
    let cases = [
        vec![
            Point {
                index: 0,
                value: Some("1".into()),
            },
            Point {
                index: 0,
                value: Some("source & <x>".into()),
            },
            Point {
                index: u32::MAX as usize - 1,
                value: Some("preceding".into()),
            },
            Point {
                index: u32::MAX as usize,
                value: Some("unavailable".into()),
            },
        ],
        [0, 2, 3]
            .map(|index| Point {
                index,
                value: shown(&loaded, index),
            })
            .into(),
        vec![
            Point {
                index: 1,
                value: None,
            },
            Point {
                index: 3,
                value: Some(" pad ".into()),
            },
        ],
        vec![
            Point {
                index: u32::MAX as usize,
                value: Some("isolated".into()),
            },
            Point {
                index: 1,
                value: Some(String::new()),
            },
            Point {
                index: 2,
                value: None,
            },
        ],
    ];
    for points in cases {
        for count in [None, Some(4), Some(0)] {
            let mut initial = number_workbook(1, 4);
            initial
                .sheet_mut(0)
                .unwrap()
                .cell_mut("A2")
                .set_value_string("");
            add_chart(&mut initial, "Sheet1", std::slice::from_ref(&formula));
            let bytes = rewrite_part(&write(&initial), "xl/charts/chart1.xml", |xml| {
                replace_category_cache(&xml, 0, &cache_xml(count, &points))
            });
            let mut book = read_book(&bytes);
            let sheet = book.sheet_mut(0).unwrap();
            sheet
                .cell_mut("A1")
                .style_mut()
                .number_format_mut()
                .set_format_code("0.00");
            sheet.cell_mut("A4").set_value_number(4.1);
            sheet
                .cell_mut("A4")
                .style_mut()
                .number_format_mut()
                .set_format_code("0");
            let now = [
                Some("1.00".into()),
                None,
                Some("3".into()),
                Some("4".into()),
            ];
            let expected = reference_xml(&formula, &reference_model(count, &points, &loaded, &now));
            assert_eq!(
                category_reference(&part(&write(&book), "xl/charts/chart1.xml"), 0),
                expected
            );
        }
    }

    // The original rule retains text for an unavailable reference whose
    // display was unavailable at import too (None equals None).
    let cache = dense_cache(1, |_| "missing source".into());
    let missing = "Missing!$A$1:$A$1";
    let book = read_book(&one_chart_workbook(
        1,
        1,
        &[(missing.into(), cache.clone())],
    ));
    assert_eq!(
        category_reference(&part(&write(&book), "xl/charts/chart1.xml"), 0),
        reference_xml(missing, &cache)
    );
}

#[test]
fn every_reference_location_is_reduced_stripped_and_adopted() {
    use umya_spreadsheet::drawing::charts as native;

    let (book, ..) = quarter_book();
    let mut chart = book.sheet(0).unwrap().chart_collection()[0].clone();
    let reference = chart.area_chart_series_list_mut().area_chart_series_mut()[0]
        .category_axis_data()
        .unwrap()
        .string_reference()
        .unwrap()
        .clone();
    let mut text = native::ChartText::default();
    text.set_string_reference(reference.clone());
    let mut title = native::Title::default();
    title.set_chart_text(text.clone());
    chart.chart_space_mut().chart_mut().set_title(title.clone());
    let mut series = chart.area_chart_series_list_mut().area_chart_series_mut()[0].clone();
    series.set_chart_text(text);
    chart
        .area_chart_series_list_mut()
        .set_area_chart_series(vec![series.clone()]);
    macro_rules! group {
        ($kind:ident) => {{
            let mut group = native::$kind::default();
            group
                .area_chart_series_list_mut()
                .add_area_chart_series(series.clone());
            native::ChartGroup::$kind(group)
        }};
    }
    let plot = chart.chart_space_mut().chart_mut().plot_area_mut();
    for group in [
        group!(Line3DChart),
        group!(PieChart),
        group!(Pie3DChart),
        group!(DoughnutChart),
        group!(ScatterChart),
        group!(BarChart),
        group!(Bar3DChart),
        group!(RadarChart),
        group!(BubbleChart),
        group!(AreaChart),
        group!(Area3DChart),
        group!(OfPieChart),
        group!(LineChart),
    ] {
        plot.add_chart_group(group);
    }
    for (index, group) in plot.chart_groups_mut().enumerate() {
        group.series_mut().area_chart_series_mut()[0]
            .index_mut()
            .set_val(index as u32);
    }
    let mut category = native::CategoryAxis::default();
    category.axis_id_mut().set_val(100);
    category.set_title(title.clone());
    plot.set_category_axis(vec![category]);
    let mut date = native::DateAxis::default();
    date.axis_id_mut().set_val(101);
    date.set_title(title.clone());
    plot.set_date_axis(vec![date]);
    let mut value = native::ValueAxis::default();
    value.axis_id_mut().set_val(102);
    value.set_title(title.clone());
    plot.set_value_axis(vec![value]);
    let mut axis = native::SeriesAxis::default();
    axis.axis_id_mut().set_val(103);
    axis.set_title(title);
    plot.set_series_axis(vec![axis]);

    // Keep a native multi-level cache and numeric reference alongside the
    // source string caches, and require byte equality after strip/adopt.
    let mut level = native::MultiLevelStringCache::default();
    level.set_point_count(1);
    let mut point = native::MultiLevelStringPoint::default();
    point.set_index(0).set_value("Hierarchy");
    level.levels_mut().push(vec![point]);
    let mut multi = native::MultiLevelStringReference::default();
    multi.set_cache(level);
    chart.area_chart_series_list_mut().area_chart_series_mut()[0]
        .category_axis_data_mut()
        .unwrap()
        .set_multi_level_string_reference(multi);
    let original = serde_json::to_value(&chart).unwrap();
    let mut sources = Vec::new();
    find_sources(&original, &mut sources);
    assert_eq!(sources.len(), 33); // title + 14 groups * 2 + four axis titles
    let mut stripped = chart.clone();
    stripped.strip_source_caches();
    let mut found = Vec::new();
    find_sources(&serde_json::to_value(&stripped).unwrap(), &mut found);
    assert!(found.is_empty());
    let mut expected = original.clone();
    fn remove_sources(value: &mut serde_json::Value) {
        match value {
            serde_json::Value::Object(map) => {
                if let Some(source) = map.get_mut("source") {
                    *source = serde_json::Value::Null;
                }
                for child in map.values_mut() {
                    remove_sources(child);
                }
            }
            serde_json::Value::Array(items) => {
                for child in items {
                    remove_sources(child);
                }
            }
            _ => {}
        }
    }
    remove_sources(&mut expected);
    assert_eq!(serde_json::to_value(&stripped).unwrap(), expected);
    stripped.adopt_source_caches(&chart);
    assert_eq!(serde_json::to_value(&stripped).unwrap(), original);

    // Serialize the complete synthetic chart through XLSX so capture, rather
    // than just the boundary traversal, visits titles and every plot group.
    let mut book = number_workbook(1, 12);
    book.sheet_mut(0).unwrap().add_chart(chart);
    let mut imported = read_book(&write(&book));
    assert_eq!(imported.chart_source_cache_report().kept_caches, 33);
    reconcile_admission(&imported);
    imported.sheet_mut(0).unwrap().chart_collection_mut()[0].strip_source_caches();
    assert!(stored_caches(&imported).is_empty());
}

fn reconcile_admission(book: &Workbook) {
    let caches = stored_caches(book);
    let report = book.chart_source_cache_report();
    let mut points = 0;
    let mut bytes = 0;
    for cache in &caches {
        let exceptions = cache["exceptions"].as_array().unwrap();
        points += exceptions.len() + cache["index_runs"].as_array().unwrap().len();
        for exception in exceptions {
            for field in ["value", "loaded_display"] {
                bytes += exception[field].as_str().map_or(0, str::len);
            }
        }
    }
    assert_eq!(caches.len(), report.kept_caches);
    assert_eq!(points, report.kept_points);
    assert_eq!(bytes, report.kept_text_bytes);
    assert!(points <= MAX_SOURCE_CACHE_POINTS);
    assert!(bytes <= MAX_SOURCE_CACHE_TEXT_BYTES);
    assert_eq!(
        report.dropped_caches,
        report.reduced_caches + report.over_cap_caches
    );
}

#[test]
fn imported_source_caches_all_pass_admission() {
    let formula = column_range('A', 3);
    let cache = dense_cache(3, |index| format!("Q {index}"));
    let mut book = read_book(&two_sheet_workbook(
        3,
        [(&formula, &cache), (&formula, &cache)],
    ));
    let chart = book.sheet(0).unwrap().chart_collection()[0].clone();
    // Read both OOXML anchor forms. Native standalone frames are constructed
    // before capture in the reader's admission test.
    let mut initial = number_workbook(1, 3);
    initial.sheet_mut(0).unwrap().add_chart(chart);
    let bytes = rewrite_part(&write(&initial), "xl/drawings/drawing1.xml", |xml| {
        let start = xml.find("<xdr:twoCellAnchor>").unwrap();
        let end = xml.find("</xdr:twoCellAnchor>").unwrap() + "</xdr:twoCellAnchor>".len();
        let anchor = &xml[start..end];
        let frame_start = anchor.find("<xdr:graphicFrame").unwrap();
        let frame_end = anchor.find("</xdr:graphicFrame>").unwrap() + "</xdr:graphicFrame>".len();
        let graphic = &anchor[frame_start..frame_end];
        let one = format!(
            "<xdr:oneCellAnchor><xdr:from><xdr:col>0</xdr:col><xdr:colOff>0</xdr:colOff><xdr:\
             row>0</xdr:row><xdr:rowOff>0</xdr:rowOff></xdr:from><xdr:ext cx=\"1\" \
             cy=\"1\"/>{graphic}<xdr:clientData/></xdr:oneCellAnchor>"
        );
        format!("{}{anchor}{one}{}", &xml[..start], &xml[end..])
    });
    book = read_book(&bytes);
    assert_eq!(book.chart_source_cache_report().kept_caches, 2);
    reconcile_admission(&book);
    // A bare graphic frame is not an OOXML drawing-root child.
    let drawing = book.sheet(0).unwrap().worksheet_drawing();
    assert_eq!(drawing.one_cell_anchor_collection().len(), 1);
    assert!(drawing.graphic_frame_collection().is_empty());
}

fn poison_sources(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            if let Some(source) = map.get_mut("source").filter(|source| source.is_object()) {
                for exception in source["exceptions"].as_array_mut().unwrap() {
                    exception["value"] = "Forged".into();
                }
            }
            for child in map.values_mut() {
                poison_sources(child);
            }
        }
        serde_json::Value::Array(values) => {
            for child in values {
                poison_sources(child);
            }
        }
        _ => {}
    }
}

#[test]
fn adoption_uses_roles_and_series_identity_through_topology_edits() {
    use umya_spreadsheet::drawing::charts as native;

    let formula = column_range('A', 3);
    let labels = [
        dense_cache(3, |index| format!("First {index}")),
        dense_cache(3, |index| (index + 1).to_string()),
        dense_cache(3, |index| format!("Last {index}")),
    ];
    let caches = labels
        .iter()
        .map(|cache| (formula.clone(), cache.clone()))
        .collect::<Vec<_>>();
    let book = read_book(&one_chart_workbook(1, 3, &caches));
    let mut previous = book.sheet(0).unwrap().chart_collection()[0].clone();
    let title_book = read_book(&one_chart_workbook(
        1,
        3,
        &[(
            formula.clone(),
            dense_cache(3, |index| format!("Title {index}")),
        )],
    ));
    let title_reference = title_book.sheet(0).unwrap().chart_collection()[0]
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
        .clone();
    let mut text = native::ChartText::default();
    text.set_string_reference(title_reference);
    let mut title = native::Title::default();
    title.set_chart_text(text);
    previous.chart_space_mut().chart_mut().set_title(title);
    let before = serde_json::to_value(&previous).unwrap();
    for order in [
        vec![1, 2],
        vec![3, 0, 1, 2],
        vec![0, 3, 1, 2],
        vec![2, 1, 0],
        vec![0, 1, 2, 3],
        vec![0, 0, 1, 2],
    ] {
        let mut json = before.clone();
        poison_sources(&mut json);
        let mut client: Chart = serde_json::from_value(json).unwrap();
        let old = client
            .area_chart_series_list_mut()
            .area_chart_series_mut()
            .to_vec();
        let mut extra = old[0].clone();
        extra.index_mut().set_val(99);
        let series: Vec<_> = order
            .iter()
            .map(|index| {
                if *index == 3 {
                    extra.clone()
                } else {
                    old[*index].clone()
                }
            })
            .collect();
        client
            .area_chart_series_list_mut()
            .set_area_chart_series(series);
        // Removing a title must not shift its same-formula cache to a series.
        client.set_title("Client title");
        client.adopt_source_caches(&previous);
        let mut target = number_workbook(1, 3);
        target.sheet_mut(0).unwrap().add_chart(client.clone());
        let xml = part(&write(&target), "xl/charts/chart1.xml");
        assert!(xml.contains("Client title"));
        let mut expected_sources = 0;
        for (position, identity) in order.iter().enumerate() {
            let unique = order.iter().filter(|index| *index == identity).count() == 1;
            let expected = if *identity < 3 && unique {
                if *identity != 1 {
                    expected_sources += 1;
                }
                reference_xml(&formula, &labels[*identity])
            } else {
                generated(&formula, 3)
            };
            assert_eq!(
                category_reference(&xml, position),
                expected,
                "order={order:?}"
            );
        }
        let mut found = Vec::new();
        find_sources(&serde_json::to_value(&client).unwrap(), &mut found);
        assert_eq!(found.len(), expected_sources);
        let adopted = serde_json::to_value(&client).unwrap();
        client.adopt_source_caches(&previous);
        assert_eq!(serde_json::to_value(&client).unwrap(), adopted);
    }
    assert_eq!(serde_json::to_value(&previous).unwrap(), before);
    // An absent series identity cannot borrow the explicit zero identity.
    let mut client = previous.clone();
    client.set_title("Literal");
    client.area_chart_series_list_mut().area_chart_series_mut()[0]
        .set_index(native::Index::default());
    client.adopt_source_caches(&previous);
    let mut found = Vec::new();
    find_sources(&serde_json::to_value(&client).unwrap(), &mut found);
    assert_eq!(found.len(), 1);
}

#[test]
fn cell_only_streaming_does_not_parse_unrelated_charts_or_sheets() {
    let mut book = number_workbook(1, 3);
    book.new_sheet("Host").unwrap();
    book.new_sheet("Broken").unwrap();
    add_chart(&mut book, "Host", &[column_range('A', 3)]);
    let bytes = rewrite_part(&write(&book), "xl/charts/chart1.xml", |xml| {
        xml.replace("</c:strCache>", "")
    });
    let bytes = rewrite_part(&bytes, "xl/worksheets/sheet3.xml", |_| "<invalid".into());
    let file = std::env::temp_dir().join(format!(
        "chart-stream-{}-{}.xlsx",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
    ));
    std::fs::write(&file, &bytes).unwrap();
    let mut lazy = umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(&bytes), false).unwrap();
    assert_eq!(lazy.chart_source_cache_report().kept_caches, 0);
    assert_eq!(
        lazy.sheet_mut(0)
            .unwrap()
            .cell("A2")
            .unwrap()
            .formatted_value(),
        "2"
    );
    let mut lazy = umya_spreadsheet::reader::xlsx::lazy_read(&file).unwrap();
    assert_eq!(
        lazy.sheet_mut(0)
            .unwrap()
            .cell("A3")
            .unwrap()
            .formatted_value(),
        "3"
    );
    let mut cells = Vec::new();
    umya_spreadsheet::reader::xlsx::read_sheet_by_name_stream(&file, "Sheet1", |cell| {
        cells.push(cell.formatted_value());
    })
    .unwrap();
    std::fs::remove_file(&file).unwrap();
    assert_eq!(cells, ["1", "2", "3"]);
}

#[test]
fn safe_lazy_materialization_matches_eager_caches_reports_and_export() {
    for raw_target in [false, true] {
        let mut initial = number_workbook(1, 3);
        let sheet = initial.new_sheet("Second").unwrap();
        for row in 1..=3 {
            sheet.cell_mut((1, row)).set_value_number(row);
        }
        let second = "Second!$A$1:$A$3";
        let first = if raw_target {
            second
        } else {
            "Sheet1!$A$1:$A$3"
        };
        add_chart(&mut initial, "Sheet1", &[first.into()]);
        add_chart(&mut initial, "Second", &[second.into()]);
        let cache = dense_cache(3, |index| format!("Source {index}"));
        let mut bytes = write(&initial);
        for name in ["xl/charts/chart1.xml", "xl/charts/chart2.xml"] {
            bytes = rewrite_part(&bytes, name, |xml| replace_category_cache(&xml, 0, &cache));
        }
        let eager = read_book(&bytes);
        let expected_caches = [
            serde_json::json!({
                "formula": first,
                "point_count": 3,
                "index_runs": [],
                "exceptions": [
                    {"index": 0, "value": "Source 0", "loaded_display": "1"},
                    {"index": 1, "value": "Source 1", "loaded_display": "2"},
                    {"index": 2, "value": "Source 2", "loaded_display": "3"},
                ],
            }),
            serde_json::json!({
                "formula": second,
                "point_count": 3,
                "index_runs": [],
                "exceptions": [
                    {"index": 0, "value": "Source 0", "loaded_display": "1"},
                    {"index": 1, "value": "Source 1", "loaded_display": "2"},
                    {"index": 2, "value": "Source 2", "loaded_display": "3"},
                ],
            }),
        ];
        assert_eq!(stored_caches(&eager), expected_caches);
        let report = eager.chart_source_cache_report();
        assert_eq!(
            (
                report.kept_caches,
                report.kept_points,
                report.kept_text_bytes,
                report.dropped_caches,
                report.reduced_caches,
                report.over_cap_caches,
                report.over_cap_points,
                report.over_cap_text_bytes,
            ),
            (2, 6, 54, 0, 0, 0, 0, 0)
        );
        for collection in [false, true] {
            let mut lazy =
                umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(&bytes), false).unwrap();
            if collection {
                lazy.read_sheet_collection();
            } else {
                lazy.read_sheet(0);
                assert_eq!(lazy.chart_source_cache_report().kept_caches, 1);
                reconcile_admission(&lazy);
                lazy.read_sheet(1);
            }
            assert_eq!(stored_caches(&lazy), expected_caches);
            let report = lazy.chart_source_cache_report();
            assert_eq!(
                (
                    report.kept_caches,
                    report.kept_points,
                    report.kept_text_bytes,
                    report.dropped_caches,
                    report.reduced_caches,
                    report.over_cap_caches,
                    report.over_cap_points,
                    report.over_cap_text_bytes,
                ),
                (2, 6, 54, 0, 0, 0, 0, 0)
            );
            let expected = write(&eager);
            let actual = write(&lazy);
            for name in ["xl/charts/chart1.xml", "xl/charts/chart2.xml"] {
                assert_eq!(part(&actual, name), part(&expected, name));
            }
            for exported in [&expected, &actual] {
                assert_eq!(
                    category_reference(&part(&exported, "xl/charts/chart1.xml"), 0),
                    reference_xml(first, &cache)
                );
                assert_eq!(
                    category_reference(&part(&exported, "xl/charts/chart2.xml"), 0),
                    reference_xml(second, &cache)
                );
            }
            let report = *lazy.chart_source_cache_report();
            lazy.sheet_mut(0).unwrap();
            lazy.read_sheet(1).read_sheet_collection();
            assert_eq!(*lazy.chart_source_cache_report(), report);
            reconcile_admission(&lazy);
        }
    }
}

fn chart_sources(chart: &Chart) -> Vec<serde_json::Value> {
    let mut found = Vec::new();
    find_sources(&serde_json::to_value(chart).unwrap(), &mut found);
    found
}

#[test]
fn adoption_rejects_duplicate_and_missing_identities_before_checking_formulas() {
    use umya_spreadsheet::drawing::charts as native;

    let (book, ..) = quarter_book();
    let previous = &book.sheet(0).unwrap().chart_collection()[0];
    let mut duplicate = previous.clone();
    let mut extra = duplicate
        .area_chart_series_list_mut()
        .area_chart_series_mut()[0]
        .clone();
    extra
        .category_axis_data_mut()
        .unwrap()
        .string_reference_mut()
        .unwrap()
        .formula_mut()
        .set_address_str("Sheet1!$A$1:$A$11");
    extra
        .category_axis_data_mut()
        .unwrap()
        .string_reference_mut()
        .unwrap()
        .set_string_cache(native::StringCache::default());
    duplicate
        .area_chart_series_list_mut()
        .add_area_chart_series(extra);
    let old_bytes = serde_json::to_vec(&duplicate).unwrap();
    let mut client = previous.clone();
    client.adopt_source_caches(&duplicate);
    assert!(chart_sources(&client).is_empty());
    assert_eq!(serde_json::to_vec(&duplicate).unwrap(), old_bytes);

    // A duplicate incoming ID is ambiguous even when the other reference
    // has a different formula and no cache. A forged first cache cannot win.
    let mut client = duplicate.clone();
    client.adopt_source_caches(previous);
    assert!(chart_sources(&client).is_empty());

    let mut absent = previous.clone();
    absent.area_chart_series_list_mut().area_chart_series_mut()[0]
        .set_index(native::Index::default());
    let mut client = previous.clone();
    client.adopt_source_caches(&absent);
    assert!(chart_sources(&client).is_empty());
    absent.adopt_source_caches(previous);
    assert!(chart_sources(&absent).is_empty());

    // Explicit zero is a usable identity and its order is presentation only.
    let mut client = previous.clone();
    client.area_chart_series_list_mut().area_chart_series_mut()[0]
        .order_mut()
        .set_val(99);
    client.adopt_source_caches(previous);
    assert_eq!(chart_sources(&client), chart_sources(previous));
    assert_eq!(
        client.area_chart_series_list_mut().area_chart_series_mut()[0]
            .order()
            .val(),
        99
    );
}

#[test]
fn adoption_follows_series_ids_across_group_kinds_and_axis_moves() {
    use umya_spreadsheet::drawing::charts as native;

    let formula = column_range('A', 3);
    let labels = [
        dense_cache(3, |index| format!("First {index}")),
        dense_cache(3, |index| format!("Second {index}")),
    ];
    let bytes = one_chart_workbook(1, 3, &labels.clone().map(|cache| (formula.clone(), cache)));
    let book = read_book(&bytes);
    let mut previous = book.sheet(0).unwrap().chart_collection()[0].clone();
    let series = previous
        .area_chart_series_list_mut()
        .area_chart_series_mut()
        .to_vec();
    let mut plot = native::PlotArea::default();
    // Two containers of the same kind, with distinct series identities.
    for item in &series {
        let mut group = native::LineChart::default();
        group
            .area_chart_series_list_mut()
            .add_area_chart_series(item.clone());
        plot.add_chart_group(native::ChartGroup::LineChart(group));
    }
    previous.chart_space_mut().chart_mut().set_plot_area(plot);
    let before = serde_json::to_vec(&previous).unwrap();
    for changed_kind in [false, true] {
        let mut plot = native::PlotArea::default();
        for item in series.iter().rev() {
            let mut axis = native::AxisId::default();
            axis.set_val(900);
            if changed_kind {
                let mut group = native::BarChart::default();
                group
                    .area_chart_series_list_mut()
                    .add_area_chart_series(item.clone());
                group.axis_id_mut().push(axis);
                plot.add_chart_group(native::ChartGroup::BarChart(group));
            } else {
                let mut group = native::LineChart::default();
                group
                    .area_chart_series_list_mut()
                    .add_area_chart_series(item.clone());
                group.axis_id_mut().push(axis);
                plot.add_chart_group(native::ChartGroup::LineChart(group));
            }
        }
        let mut client = previous.clone();
        client.chart_space_mut().chart_mut().set_plot_area(plot);
        let expected = serde_json::to_value(&client).unwrap();
        let mut forged = expected.clone();
        poison_sources(&mut forged);
        client = serde_json::from_value(forged).unwrap();
        client.adopt_source_caches(&previous);
        assert_eq!(serde_json::to_value(&client).unwrap(), expected);
        assert_eq!(chart_sources(&client).len(), 2);
        let mut target = number_workbook(1, 3);
        target.sheet_mut(0).unwrap().add_chart(client);
        let xml = part(&write(&target), "xl/charts/chart1.xml");
        for (index, cache) in labels.iter().rev().enumerate() {
            assert_eq!(
                category_reference(&xml, index),
                reference_xml(&formula, cache)
            );
        }
    }
    assert_eq!(serde_json::to_vec(&previous).unwrap(), before);

    // Duplicate identities across groups are ambiguous just like duplicates
    // within one series list, even when the second source is absent.
    let mut duplicated = previous.clone();
    for group in duplicated
        .chart_space_mut()
        .chart_mut()
        .plot_area_mut()
        .chart_groups_mut()
    {
        group.series_mut().area_chart_series_mut()[0]
            .index_mut()
            .set_val(0);
    }
    let mut incoming = previous.clone();
    incoming.adopt_source_caches(&duplicated);
    assert_eq!(chart_sources(&incoming).len(), 0);
    duplicated.adopt_source_caches(&previous);
    assert!(chart_sources(&duplicated).is_empty());
}

#[test]
fn adoption_matches_axis_title_ids_without_axis_kind_and_rejects_ambiguity() {
    use umya_spreadsheet::drawing::charts as native;

    let (book, ..) = quarter_book();
    let mut previous = book.sheet(0).unwrap().chart_collection()[0].clone();
    let reference = previous
        .area_chart_series_list_mut()
        .area_chart_series_mut()[0]
        .category_axis_data()
        .unwrap()
        .string_reference()
        .unwrap()
        .clone();
    let mut text = native::ChartText::default();
    text.set_string_reference(reference);
    let mut title = native::Title::default();
    title.set_chart_text(text);
    let mut category = native::CategoryAxis::default();
    category.axis_id_mut().set_val(0);
    category.set_title(title.clone());
    previous
        .chart_space_mut()
        .chart_mut()
        .plot_area_mut()
        .set_category_axis(vec![category]);
    let before = serde_json::to_value(&previous).unwrap();
    let mut incoming = previous.clone();
    let mut date = native::DateAxis::default();
    date.axis_id_mut().set_val(0);
    date.set_title(title);
    let plot = incoming.chart_space_mut().chart_mut().plot_area_mut();
    plot.set_category_axis(vec![]);
    plot.set_date_axis(vec![date.clone()]);
    incoming.strip_source_caches();
    incoming.adopt_source_caches(&previous);
    assert_eq!(chart_sources(&incoming).len(), 2);
    let mut target = number_workbook(1, 12);
    target.sheet_mut(0).unwrap().add_chart(incoming.clone());
    let xml = part(&write(&target), "xl/charts/chart1.xml");
    let axis = xml
        .split_once("<c:dateAx>")
        .unwrap()
        .1
        .split_once("</c:dateAx>")
        .unwrap()
        .0;
    assert!(axis.contains("<c:v>Q 12</c:v>"));

    // Same role and ID across different axis kinds is ambiguous. Count the
    // duplicate even when its source is absent or its guard is invalid.
    let mut duplicate = previous.clone();
    date.title_mut()
        .unwrap()
        .chart_text_mut()
        .unwrap()
        .string_reference_mut()
        .unwrap()
        .formula_mut()
        .set_address_str("Sheet1!$A$1:$A$11");
    duplicate
        .chart_space_mut()
        .chart_mut()
        .plot_area_mut()
        .set_date_axis(vec![date]);
    let mut client = previous.clone();
    client.adopt_source_caches(&duplicate);
    assert_eq!(chart_sources(&client).len(), 1);
    duplicate.adopt_source_caches(&previous);
    assert_eq!(chart_sources(&duplicate).len(), 1);

    for missing_previous in [false, true] {
        let mut absent = previous.clone();
        absent
            .chart_space_mut()
            .chart_mut()
            .plot_area_mut()
            .category_axis_mut()[0]
            .set_axis_id(native::AxisId::default());
        let mut client = if missing_previous {
            previous.clone()
        } else {
            absent.clone()
        };
        client.adopt_source_caches(if missing_previous { &absent } else { &previous });
        assert_eq!(chart_sources(&client).len(), 1);
    }
    assert_eq!(serde_json::to_value(&previous).unwrap(), before);
}

#[test]
fn source_cache_boundaries_preserve_a_chart_without_a_graphic_frame() {
    let mut empty = Chart::default();
    let original = serde_json::to_value(&empty).unwrap();
    empty.strip_source_caches();
    empty.clear_source_caches();
    empty.adopt_source_caches(&Chart::default());
    assert_eq!(serde_json::to_value(&empty).unwrap(), original);
    assert!(chart_sources(&empty).is_empty());
    let (book, ..) = quarter_book();
    let stored = &book.sheet(0).unwrap().chart_collection()[0];
    empty.adopt_source_caches(stored);
    assert_eq!(serde_json::to_value(&empty).unwrap(), original);
    let mut incoming = stored.clone();
    incoming.adopt_source_caches(&empty);
    assert!(chart_sources(&incoming).is_empty());
    assert_eq!(chart_sources(stored).len(), 1);
}
