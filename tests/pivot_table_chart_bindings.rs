#![cfg(feature = "serde")]
#[path = "support/slicer_package_fixture.rs"]
mod fixture;

use std::io::{
    Cursor,
    Read,
};

use fixture::*;
use umya_spreadsheet::{
    Workbook,
    reader,
    writer,
};

const PIVOT_CHART: &[u8] = include_bytes!("fixtures/chart_space_parts/pivotchart.xlsx");

fn chart_parts(bytes: &[u8]) -> Vec<String> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut charts = Vec::new();
    for index in 0..archive.len() {
        let mut part = archive.by_index(index).unwrap();
        let name = part.name().unwrap();
        if name.starts_with("xl/charts/chart") && name.ends_with(".xml") {
            let mut xml = String::new();
            part.read_to_string(&mut xml).unwrap();
            charts.push(xml);
        }
    }
    charts
}

fn plotted_values(xml: &str, owner: &[u8]) -> Vec<Vec<String>> {
    use quick_xml::events::Event;
    let mut reader = quick_xml::Reader::from_str(xml);
    let mut series = Vec::new();
    let mut points = None;
    loop {
        match reader.read_event().unwrap() {
            Event::Start(element) if element.name().as_ref() == owner => {
                points = Some(Vec::new());
            }
            Event::Empty(element)
                if points.is_some() && element.name().as_ref() == b"c:ptCount" =>
            {
                assert_eq!(
                    element
                        .try_get_attribute("val")
                        .unwrap()
                        .unwrap()
                        .value
                        .as_ref(),
                    b"3"
                );
            }
            Event::Start(element) if points.is_some() && element.name().as_ref() == b"c:pt" => {
                let index = element.try_get_attribute("idx").unwrap().unwrap();
                assert_eq!(
                    std::str::from_utf8(&index.value)
                        .unwrap()
                        .parse::<usize>()
                        .unwrap(),
                    points.as_ref().unwrap().len()
                );
            }
            Event::Start(element) if points.is_some() && element.name().as_ref() == b"c:v" => {
                points.as_mut().unwrap().push(
                    reader
                        .read_text(element.name())
                        .unwrap()
                        .decode()
                        .unwrap()
                        .into_owned(),
                );
            }
            Event::End(element) if element.name().as_ref() == owner => {
                series.push(points.take().unwrap());
            }
            Event::Eof => return series,
            _ => (),
        }
    }
}

#[test]
fn sheet_rename_keeps_plotted_pivot_values_on_export_and_deletion() {
    // The saved Excel fixture plots E/N/S, a = 15/10/30 and b = 25/20/40.
    // Its formula ranges also include grand totals 55 and 85, which are not
    // points.
    let expected = vec![vec!["15", "10", "30"], vec!["25", "20", "40"]];
    assert_eq!(
        plotted_values(&chart_parts(PIVOT_CHART)[0], b"c:numCache"),
        expected
    );
    let mut source = read_eager(PIVOT_CHART).unwrap();
    let chart = source.sheet_by_name("Pivot").unwrap().chart_collection()[0].clone();
    source.new_sheet("Charts").unwrap().add_chart(chart);
    let copied_input = write(&source, false);
    for eager in [false, true] {
        for operation in ["export", "sheet"] {
            let input = if operation == "sheet" {
                copied_input.as_slice()
            } else {
                PIVOT_CHART
            };
            let mut book = reader::xlsx::read_reader(Cursor::new(input), eager).unwrap();
            let pivot = book
                .sheet_collection_no_check()
                .iter()
                .position(|sheet| sheet.name() == "Pivot")
                .unwrap();
            book.set_sheet_name(pivot, "Sales 2026").unwrap();
            if operation == "sheet" {
                book.remove_sheet_by_name("Sales 2026").unwrap();
            }
            for light in [false, true] {
                let output = write(&book, light);
                emit(
                    &format!("pivot-sheet-renamed-{operation}-{eager}"),
                    &output,
                    light,
                );
                let reread = read_eager(&output).unwrap();
                for bytes in [output, write(&reread, light)] {
                    let charts = chart_parts(&bytes);
                    assert_eq!(charts.len(), 1);
                    for xml in charts {
                        let frozen = operation != "export";
                        assert_eq!(
                            plotted_values(&xml, if frozen { b"c:numLit" } else { b"c:numCache" }),
                            expected,
                            "{operation}, eager={eager}, light={light}"
                        );
                        assert_eq!(
                            plotted_values(&xml, b"c:cat"),
                            vec![vec!["E", "N", "S"], vec!["E", "N", "S"]],
                            "{operation}, eager={eager}, light={light}"
                        );
                        assert_eq!(xml.contains("<c:pivotSource>"), !frozen);
                        if frozen {
                            assert!(!xml.contains("<c:f>"));
                        } else {
                            assert!(xml.contains("'Sales 2026'!"));
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn sheet_rename_does_not_revalidate_an_edited_pivot_formula_cache() {
    for eager in [false, true] {
        let mut book = reader::xlsx::read_reader(Cursor::new(PIVOT_CHART), eager).unwrap();
        book.try_read_sheet_collection().unwrap();
        book.sheet_by_name_mut("Pivot")
            .unwrap()
            .chart_collection_mut()[0]
            .area_chart_series_list_mut()
            .area_chart_series_mut()[0]
            .values_mut()
            .unwrap()
            .number_reference_mut()
            .formula_mut()
            .set_address_str("Pivot!$C$5:$C$7");
        let index = book
            .sheet_collection_no_check()
            .iter()
            .position(|sheet| sheet.name() == "Pivot")
            .unwrap();
        book.set_sheet_name(index, "Sales 2026").unwrap();
        for light in [false, true] {
            let output = write(&book, light);
            assert_eq!(
                plotted_values(&chart_parts(&output)[0], b"c:numCache"),
                vec![vec!["25", "20", "40"], vec!["25", "20", "40"]]
            );
            let reread = read_eager(&output).unwrap();
            assert_eq!(
                plotted_values(&chart_parts(&write(&reread, light))[0], b"c:numCache"),
                vec![vec!["25", "20", "40"], vec!["25", "20", "40"]]
            );
        }
    }
}

#[test]
fn detached_sheet_rename_preserves_the_plotted_pivot_cache() {
    let mut book = read_eager(PIVOT_CHART).unwrap();
    let index = book
        .sheet_collection_no_check()
        .iter()
        .position(|sheet| sheet.name() == "Pivot")
        .unwrap();
    let edit = umya_spreadsheet::drawing::charts::ChartReferenceEdit::RenameSheet {
        old: "Pivot".into(),
        new: "Sales 2026".into(),
    };
    let report =
        book.sheet_mut(index).unwrap().chart_collection_mut()[0].adjust_references("Pivot", &edit);
    assert!(report.changed > 0);
    book.set_sheet_name(index, "Sales 2026").unwrap();
    for light in [false, true] {
        let output = write(&book, light);
        assert_eq!(
            plotted_values(&chart_parts(&output)[0], b"c:numCache"),
            vec![vec!["15", "10", "30"], vec!["25", "20", "40"]]
        );
    }
}

fn write(book: &Workbook, light: bool) -> Vec<u8> {
    let mut bytes = Vec::new();
    if light {
        writer::xlsx::write_writer_light(book, &mut bytes).unwrap();
    } else {
        writer::xlsx::write_writer(book, &mut bytes).unwrap();
    }
    bytes
}

fn emit(name: &str, bytes: &[u8], light: bool) {
    if let Some(target) = std::env::var_os("CARGO_TARGET_DIR") {
        let directory = std::path::PathBuf::from(target).join("chart-space-validation");
        std::fs::create_dir_all(&directory).unwrap();
        let suffix = if light { "light" } else { "normal" };
        std::fs::write(directory.join(format!("{name}-{suffix}.xlsx")), bytes).unwrap();
    }
}

#[test]
fn frozen_pivot_chart_categories_without_a_source_cache_use_chart_overflow_text() {
    // The edited category range no longer matches the cached range, so the
    // freeze reads the cells. Excel's chart labels show 7 '#' for a date serial
    // outside its range.
    for eager in [false, true] {
        let mut book = reader::xlsx::read_reader(Cursor::new(PIVOT_CHART), eager).unwrap();
        book.try_read_sheet_collection().unwrap();
        let sheet = book.sheet_by_name_mut("Pivot").unwrap();
        for (address, serial) in [("A5", 1e20), ("A6", -1.0), ("A7", 1e21)] {
            let cell = sheet.cell_mut(address);
            cell.set_value_number(serial);
            cell.style_mut()
                .number_format_mut()
                .set_format_code("yyyy-mm-dd");
        }
        for series in sheet.chart_collection_mut()[0]
            .area_chart_series_list_mut()
            .area_chart_series_mut()
        {
            series
                .category_axis_data_mut()
                .unwrap()
                .string_reference_mut()
                .unwrap()
                .formula_mut()
                .set_address_str("Pivot!$A$5:$A$7");
        }
        let chart = sheet.chart_collection()[0].clone();
        book.new_sheet("Charts").unwrap().add_chart(chart);
        book.remove_sheet_by_name("Pivot").unwrap();
        for light in [false, true] {
            let output = write(&book, light);
            for bytes in [output.clone(), write(&read_eager(&output).unwrap(), light)] {
                assert_eq!(
                    plotted_values(&chart_parts(&bytes)[0], b"c:cat"),
                    vec![vec!["#######"; 3]; 2],
                    "eager={eager}, light={light}"
                );
            }
        }
    }
}
