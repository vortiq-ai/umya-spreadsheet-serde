#![cfg(feature = "serde")]

#[path = "support/slicer_package_fixture.rs"]
mod fixture;

use std::io::Cursor;

use fixture::{
    keep_output,
    readers,
    save,
    unpack,
};
use umya_spreadsheet::writer::streaming_writer::StreamingWriter;

#[test]
fn a_disconnected_timeline_on_another_sheet_keeps_its_removed_pivots_cache() {
    // Excel: cut T from Report onto Data, then delete Report. T and its
    // selection survive; the pivot cache moves to timelineCachePivotCaches.
    let bytes =
        include_bytes!("fixtures/timelines/excel-authored/disconnected-surviving-view.xlsx");
    let original = unpack(bytes);
    let records = &original["xl/pivotCache/pivotCacheRecords1.xml"];
    for (reader, read) in readers() {
        let mut book = read(bytes).unwrap();
        book.try_read_sheet_collection().unwrap();
        assert_eq!(book.sheet_by_name("Data").unwrap().timelines()[0].name, "T");
        assert_eq!(
            book.sheet_by_name("Report").unwrap().timelines()[0].name,
            "T2"
        );
        assert!(book.timeline_caches()[0].linked_pivots().is_empty());
        book.remove_sheet_by_name("Report").unwrap();
        let retained: Vec<_> = book.retained_pivot_caches().collect();
        assert_eq!(retained.len(), 1, "{reader}");
        assert_eq!(retained[0].0, 0);
        assert_eq!(retained[0].1.slicer_cache_id(), Some(1_941_512_192));
        assert_eq!(retained[0].1.raw_cache_records(), records.as_bytes());
        assert_eq!(book.timeline_caches().len(), 1);
        let cache = &book.timeline_caches()[0];
        assert_eq!(cache.name, "NativeTimeline_Day");
        assert!(cache.linked_pivots().is_empty());
        let selection = cache.state.selection.as_ref().unwrap();
        assert_eq!(selection.start_date, "2024-01-01T00:00:00");
        assert_eq!(selection.end_date, "2024-02-29T00:00:00");
        let normal = save(&book);
        let mut stream = StreamingWriter::new(zip::ZipWriter::new(Cursor::new(Vec::new())), book);
        let data = stream.take_sheet("Data").unwrap();
        stream.flush_sheet(data).unwrap();
        let streamed = stream.finish().unwrap().into_inner();
        for (writer, output) in [("normal", normal), ("streaming", streamed)] {
            let parts = unpack(&output);
            assert!(parts["xl/workbook.xml"].contains("<x15:timelineCachePivotCaches>"));
            assert!(!parts["xl/workbook.xml"].contains("<pivotCaches>"));
            assert_eq!(&parts["xl/pivotCache/pivotCacheRecords1.xml"], records);
            for (_, reread) in readers() {
                let mut again = reread(&output).unwrap();
                again.try_read_sheet_collection().unwrap();
                assert_eq!(again.retained_pivot_caches().count(), 1);
                assert_eq!(again.timeline_caches()[0].name, "NativeTimeline_Day");
                let data = again.sheet_by_name("Data").unwrap();
                assert!(data.pivot_tables().is_empty());
                assert_eq!(data.timelines().len(), 1);
                assert_eq!(data.timelines()[0].name, "T");
            }
            keep_output(
                &format!("timeline-disconnected-surviving-view-{reader}-{writer}"),
                &output,
            );
        }
    }
}

#[test]
fn removing_the_last_disconnected_timeline_views_removes_their_cache() {
    let bytes = include_bytes!("fixtures/timelines/excel-authored/disconnected-pivot.xlsx");
    for (reader, read) in readers() {
        let mut book = read(bytes).unwrap();
        book.try_read_sheet_collection().unwrap();
        assert_eq!(book.timeline_caches().len(), 1, "{reader}");
        assert_eq!(book.timeline_caches()[0].name, "NativeTimeline_Day");
        assert!(book.timeline_caches()[0].linked_pivots().is_empty());
        let report = book.sheet_by_name("Report").unwrap();
        assert_eq!(
            report
                .timelines()
                .iter()
                .map(|view| view.name.as_str())
                .collect::<Vec<_>>(),
            ["T", "T2"]
        );

        book.remove_sheet_by_name("Report").unwrap();
        assert!(book.timeline_caches().is_empty(), "{reader}");
        assert!(
            book.defined_names()
                .iter()
                .all(|name| name.name() != "NativeTimeline_Day")
        );

        let normal = save(&book);
        let mut stream = StreamingWriter::new(zip::ZipWriter::new(Cursor::new(Vec::new())), book);
        let data = stream.take_sheet("Data").unwrap();
        stream.flush_sheet(data).unwrap();
        let streamed = stream.finish().unwrap().into_inner();
        for (writer, output) in [("normal", normal), ("streaming", streamed)] {
            let parts = unpack(&output);
            assert!(
                parts.keys().all(|path| !path.contains("/timeline")),
                "{reader} {writer}"
            );
            assert!(!parts["xl/_rels/workbook.xml.rels"].contains("/timelineCache"));
            assert!(!parts["[Content_Types].xml"].contains("timelineCache+xml"));
            assert!(!parts["xl/workbook.xml"].contains("NativeTimeline_Day"));
            for (rereader, reread) in readers() {
                let mut again = reread(&output).unwrap();
                again.try_read_sheet_collection().unwrap();
                assert!(
                    again.timeline_caches().is_empty(),
                    "{reader} {writer} {rereader}"
                );
                assert_eq!(again.sheet_collection_no_check().len(), 1);
                assert_eq!(again.sheet_by_name("Data").unwrap().name(), "Data");
            }
            keep_output(
                &format!("timeline-disconnected-sheet-removed-{reader}-{writer}"),
                &output,
            );
        }
    }
}
