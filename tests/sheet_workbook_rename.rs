#![cfg(feature = "serde")]

#[path = "support/slicer_package_fixture.rs"]
mod fixture;

use std::io::Cursor;

use umya_spreadsheet::{
    Workbook,
    reader,
    writer,
    writer::streaming_writer::StreamingWriter,
};

fn save(book: &Workbook, streaming: bool) -> Vec<u8> {
    if !streaming {
        let mut output = Vec::new();
        writer::xlsx::write_writer(book, &mut output).unwrap();
        return output;
    }
    let names = book
        .sheet_collection_no_check()
        .iter()
        .map(|sheet| sheet.name().to_owned())
        .collect::<Vec<_>>();
    let zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let mut writer = StreamingWriter::new(zip, book.clone());
    for name in names {
        let sheet = writer.take_sheet(&name).unwrap();
        writer.flush_sheet(sheet).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

#[test]
fn workbook_rename_updates_every_object_owner() {
    // Excel-authored inputs and the rename rules measured in lib-chartspace,
    // lib-chartdrawing, lib-timeline and the sheet-operation Excel probe.
    let cases: &[(&str, &[u8], &str)] = &[
        (
            "linked-text",
            include_bytes!("fixtures/chart_drawing_linked_excel.xlsx"),
            "Data",
        ),
        (
            "pivot-chart-source",
            include_bytes!("fixtures/chart_space_parts/pivotchart.xlsx"),
            "Data",
        ),
        (
            "pivot-chart-owner",
            include_bytes!("fixtures/chart_space_parts/pivotchart.xlsx"),
            "Pivot",
        ),
        (
            "extended-chart",
            include_bytes!("fixtures/extended_charts/sheet-deletion/named-source.xlsx"),
            "Sheet1",
        ),
        (
            "table-slicer",
            include_bytes!("fixtures/slicers/excel-authored/table-slicer.xlsx"),
            "Data",
        ),
        (
            "pivot-slicer",
            include_bytes!("fixtures/slicers/excel-authored/pivot-slicers.xlsx"),
            "Data",
        ),
        (
            "timeline",
            include_bytes!("fixtures/slicers/excel-authored/timeline.xlsx"),
            "Data",
        ),
        (
            "retained-cache",
            include_bytes!("fixtures/timelines/excel-authored/orphan-pivot-cache.xlsx"),
            "Data",
        ),
        (
            "custom-style",
            include_bytes!("fixtures/timelines/excel-authored/two-pivots-custom-style.xlsx"),
            "Data",
        ),
    ];
    for (case, input, source) in cases {
        for (reader_name, read) in fixture::readers() {
            let mut coordinated = read(input).unwrap();
            let index = coordinated
                .sheet_collection_no_check()
                .iter()
                .position(|sheet| sheet.name() == *source)
                .unwrap();
            coordinated.set_sheet_name(index, "Bob's Data").unwrap();
            for streaming in [false, true] {
                if *case == "extended-chart" && streaming {
                    let mut stream = StreamingWriter::new(
                        zip::ZipWriter::new(Cursor::new(Vec::new())),
                        coordinated.clone(),
                    );
                    let source = stream.take_sheet("Bob's Data").unwrap();
                    stream.flush_sheet(source).unwrap();
                    let host = stream.take_sheet("ChartHost").unwrap();
                    let error = stream.flush_sheet(host).unwrap_err();
                    assert!(
                        error
                            .to_string()
                            .contains("ChartEx source worksheet is unavailable: Bob's Data")
                    );
                    assert!(stream.finish().is_err());
                    continue;
                }
                let output = save(&coordinated, streaming);
                let reread = reader::xlsx::read_reader(Cursor::new(&output), true).unwrap();
                assert_eq!(reread.sheet(index).unwrap().name(), "Bob's Data");
                for sheet in reread.sheet_collection() {
                    for pivot in sheet.pivot_tables() {
                        if *source == "Data" {
                            assert_eq!(
                                pivot
                                    .pivot_cache_definition()
                                    .cache_source()
                                    .worksheet_source()
                                    .unwrap()
                                    .address()
                                    .sheet_name(),
                                "Bob's Data"
                            );
                        }
                    }
                }
                for (_, cache) in reread.retained_pivot_caches() {
                    assert_eq!(
                        cache
                            .cache_source()
                            .worksheet_source()
                            .unwrap()
                            .address()
                            .sheet_name(),
                        "Bob's Data"
                    );
                }
                if *case == "extended-chart" {
                    let names = reread.defined_names().iter().chain(
                        reread
                            .sheet_collection()
                            .iter()
                            .flat_map(|sheet| sheet.defined_names()),
                    );
                    assert_eq!(
                        names
                            .filter(|name| name.name().starts_with("_xlchart."))
                            .map(|name| name.address())
                            .collect::<Vec<_>>(),
                        [
                            "'Bob''s Data'!$A$2:$B$5",
                            "'Bob''s Data'!$C$1",
                            "'Bob''s Data'!$C$2:$C$5"
                        ]
                    );
                }
                fixture::keep_output(
                    &format!("sheet-rename-{case}-{reader_name}-{streaming}"),
                    &output,
                );
            }
        }
    }
}
