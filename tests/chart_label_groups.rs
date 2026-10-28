//! All supported family delegates through chart codecs and ordinary XLSX.
//! The constructor supplies a package container, not the expected oracle:
//! owner presence and flag spellings are independent, literal expectations.
#![cfg(feature = "serde")]
use std::io::{
    Cursor,
    Read,
    Write,
};

use umya_spreadsheet::{
    Chart,
    ChartType,
    Workbook,
    drawing::{
        charts::{
            ChartGroupMut,
            DataLabels,
        },
        spreadsheet::MarkerType,
    },
};

#[derive(Clone, Copy, Debug)]
enum Mutation {
    Current,
    Deprecated,
    Setter,
}
fn cases() -> [(ChartType, &'static str); 13] {
    use ChartType::*;
    [
        (LineChart, "lineChart"),
        (Line3DChart, "line3DChart"),
        (PieChart, "pieChart"),
        (Pie3DChart, "pie3DChart"),
        (DoughnutChart, "doughnutChart"),
        (ScatterChart, "scatterChart"),
        (BarChart, "barChart"),
        (Bar3DChart, "bar3DChart"),
        (RadarChart, "radarChart"),
        (BubbleChart, "bubbleChart"),
        (AreaChart, "areaChart"),
        (Area3DChart, "area3DChart"),
        (OfPieChart, "ofPieChart"),
    ]
}
fn marker(cell: &str) -> MarkerType {
    let mut v = MarkerType::default();
    v.set_coordinate(cell);
    v
}
fn make_book(kind: ChartType) -> Workbook {
    let mut book = umya_spreadsheet::new_file();
    for row in 1..=3 {
        book.sheet_mut(0)
            .unwrap()
            .cell_mut((2, row))
            .set_value_number(row * 10);
    }
    let refs = match kind {
        ChartType::ScatterChart => vec!["Sheet1!B1:B3"; 2],
        ChartType::BubbleChart => vec!["Sheet1!B1:B3"; 3],
        ChartType::DoughnutChart => vec!["Sheet1!B1:B3"; 2],
        _ => vec!["Sheet1!B1:B3"],
    };
    let mut chart = Chart::default();
    chart.new_chart(&kind, marker("E1"), marker("M20"), refs);
    book.sheet_mut(0).unwrap().add_chart(chart);
    book
}
fn save(book: &Workbook) -> Vec<u8> {
    let mut out = Vec::new();
    umya_spreadsheet::writer::xlsx::write_writer(book, &mut Cursor::new(&mut out)).unwrap();
    if let Some(dir) = std::env::var_os("UMYA_LABEL_SCHEMA_EXPORT_DIR") {
        use std::sync::atomic::{
            AtomicUsize,
            Ordering,
        };
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let dir = std::path::PathBuf::from(dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(format!(
                "families-{:06}.xlsx",
                NEXT.fetch_add(1, Ordering::Relaxed)
            )),
            &out,
        )
        .unwrap();
    }
    out
}
fn load(bytes: &[u8]) -> Workbook {
    umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap()
}
fn chart(book: &Workbook) -> &Chart {
    &book.sheet(0).unwrap().chart_collection()[0]
}
fn chart_mut(book: &mut Workbook) -> &mut Chart {
    &mut book.sheet_mut(0).unwrap().chart_collection_mut()[0]
}
fn persist(book: &Workbook) -> Workbook {
    let json = serde_json::to_vec(chart(book)).unwrap();
    let from_json: Chart = serde_json::from_slice(&json).unwrap();
    let binary = bincode::serialize(&from_json).unwrap();
    let restored: Chart = bincode::deserialize(&binary).unwrap();
    let mut result = book.clone();
    *chart_mut(&mut result) = restored;
    result
}
fn xml(book: &Workbook) -> String {
    let bytes = save(book);
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut text = String::new();
    zip.by_name("xl/charts/chart1.xml")
        .unwrap()
        .read_to_string(&mut text)
        .unwrap();
    text
}
fn group<'a>(xml: &'a str, tag: &str) -> &'a str {
    let open = format!("<c:{tag}>");
    let close = format!("</c:{tag}>");
    let start = xml.find(&open).unwrap();
    let end = start + xml[start..].find(&close).unwrap() + close.len();
    &xml[start..end]
}
fn after_series(group: &str) -> &str {
    let end = group.rfind("</c:ser>").unwrap() + "</c:ser>".len();
    &group[end..]
}
fn omit_group_labels(book: &Workbook, tag: &str) -> Workbook {
    let input = save(book);
    let mut source = zip::ZipArchive::new(Cursor::new(input)).unwrap();
    let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for i in 0..source.len() {
        let mut member = source.by_index(i).unwrap();
        let name = member.name().to_owned();
        let mut bytes = Vec::new();
        member.read_to_end(&mut bytes).unwrap();
        if name == "xl/charts/chart1.xml" {
            let mut text = String::from_utf8(bytes).unwrap();
            let open = format!("<c:{tag}>");
            let close = format!("</c:{tag}>");
            let start = text.find(&open).unwrap();
            let end = start + text[start..].find(&close).unwrap();
            let end_of_series =
                start + text[start..end].rfind("</c:ser>").unwrap() + "</c:ser>".len();
            if let Some(offset) = text[end_of_series..end].find("<c:dLbls>") {
                let from = end_of_series + offset;
                let to = from + text[from..].find("</c:dLbls>").unwrap() + "</c:dLbls>".len();
                assert!(to <= end);
                text.replace_range(from..to, "");
            } else if let Some(offset) = text[end_of_series..end].find("<c:dLbls/>") {
                let from = end_of_series + offset;
                text.replace_range(from..from + "<c:dLbls/>".len(), "");
            }
            bytes = text.into_bytes();
        }
        output
            .start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        output.write_all(&bytes).unwrap();
    }
    load(&output.finish().unwrap().into_inner())
}
// The compatibility matrix intentionally exercises deprecated mutable aliases.
#[allow(deprecated)]
fn edit_group(group: ChartGroupMut<'_>, mode: Mutation) {
    macro_rules! edit {
        ($v:expr) => {{
            match mode {
                Mutation::Current => {
                    $v.data_labels_mut().show_value_mut().set_val(true);
                }
                Mutation::Deprecated => {
                    $v.get_data_labels_mut().get_show_value_mut().set_val(true);
                }
                Mutation::Setter => {
                    let mut labels = DataLabels::default();
                    labels.show_value_mut().set_val(true);
                    $v.set_data_labels(labels);
                }
            }
        }};
    }
    match group {
        ChartGroupMut::LineChart(v) => edit!(v),
        ChartGroupMut::Line3DChart(v) => edit!(v),
        ChartGroupMut::PieChart(v) => edit!(v),
        ChartGroupMut::Pie3DChart(v) => edit!(v),
        ChartGroupMut::DoughnutChart(v) => edit!(v),
        ChartGroupMut::ScatterChart(v) => edit!(v),
        ChartGroupMut::BarChart(v) => edit!(v),
        ChartGroupMut::Bar3DChart(v) => edit!(v),
        ChartGroupMut::RadarChart(v) => edit!(v),
        ChartGroupMut::BubbleChart(v) => edit!(v),
        ChartGroupMut::AreaChart(v) => edit!(v),
        ChartGroupMut::OfPieChart(v) => edit!(v),
        ChartGroupMut::Area3DChart(v) => {
            if v.data_labels().is_none() {
                v.set_data_labels(DataLabels::default());
            }
            match mode {
                Mutation::Current => {
                    v.data_labels_mut().unwrap().show_value_mut().set_val(true);
                }
                Mutation::Deprecated => {
                    v.get_data_labels_mut()
                        .unwrap()
                        .get_show_value_mut()
                        .set_val(true);
                }
                Mutation::Setter => {
                    let mut labels = DataLabels::default();
                    labels.show_value_mut().set_val(true);
                    v.set_data_labels(labels);
                }
            }
        }
    }
}
#[test]
fn every_family_keeps_missing_group_owner_missing_until_a_real_delegate_edit() {
    for (kind, tag) in cases() {
        for mode in [Mutation::Current, Mutation::Deprecated, Mutation::Setter] {
            let source = omit_group_labels(&make_book(kind), tag);
            let mut book = persist(&source);
            let before = xml(&book);
            let tail = after_series(group(&before, tag));
            assert!(!tail.contains("<c:dLbls"), "{kind:?} {mode:?}: {tail}");
            {
                let mut groups = chart_mut(&mut book)
                    .chart_space_mut()
                    .chart_mut()
                    .plot_area_mut()
                    .chart_groups_mut();
                let group = groups.next().unwrap();
                assert_eq!(group.kind(), kind);
                edit_group(group, mode);
            }
            let after = xml(&book);
            let tail = after_series(group(&after, tag));
            assert!(tail.contains("<c:dLbls"), "{kind:?} {mode:?}: {tail}");
            assert!(
                tail.contains("<c:showVal val=\"1\"/>"),
                "{kind:?} {mode:?}: {tail}"
            );
            let roundtrip = persist(&load(&save(&book)));
            let after = xml(&roundtrip);
            let tail = after_series(group(&after, tag));
            assert!(
                tail.contains("<c:showVal val=\"1\"/>"),
                "reload {kind:?} {mode:?}: {tail}"
            );
        }
    }
}
