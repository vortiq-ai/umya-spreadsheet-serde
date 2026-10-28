//! Exact source-derived byte hypotheses checked against the real serializers.
//! These assertions must run in the joint gate; this file itself is not a
//! receipt.
#![cfg(feature = "serde")]
use std::io::{
    Cursor,
    Read,
    Write,
};

use umya_spreadsheet::{
    Chart,
    Workbook,
    drawing::{
        Run,
        RunProperties,
        charts::{
            DataLabels,
            ShowBubbleSize,
            ShowCategoryName,
            ShowLeaderLines,
            ShowLegendKey,
            ShowPercent,
            ShowSeriesName,
            ShowValue,
            TextProperties,
        },
    },
};
const PINNED: &[u8] = include_bytes!("fixtures/pinned_chart_input.xlsx");
fn first_chart(book: &Workbook) -> &Chart {
    book.sheet_collection()
        .iter()
        .flat_map(|s| s.chart_collection())
        .next()
        .unwrap()
}
fn first_chart_mut(book: &mut Workbook) -> &mut Chart {
    book.sheet_collection_mut()
        .iter_mut()
        .flat_map(|s| s.chart_collection_mut())
        .next()
        .unwrap()
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
                "ledger-{:06}.xlsx",
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
fn input(group_present: bool) -> Workbook {
    let mut source = zip::ZipArchive::new(Cursor::new(PINNED)).unwrap();
    let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for i in 0..source.len() {
        let mut member = source.by_index(i).unwrap();
        let name = member.name().to_owned();
        let mut bytes = Vec::new();
        member.read_to_end(&mut bytes).unwrap();
        if name == "xl/charts/chart1.xml" {
            let mut xml = String::from_utf8(bytes).unwrap();
            let group_start = xml.find("<c:scatterChart>").unwrap();
            let group_end = group_start + xml[group_start..].find("</c:scatterChart>").unwrap();
            let after_series = group_start
                + xml[group_start..group_end].rfind("</c:ser>").unwrap()
                + "</c:ser>".len();
            if let Some(offset) = xml[after_series..group_end].find("<c:dLbls>") {
                let from = after_series + offset;
                let to = from + xml[from..].find("</c:dLbls>").unwrap() + "</c:dLbls>".len();
                assert!(to <= group_end);
                xml.replace_range(from..to, "");
            }
            let group_start = xml.find("<c:scatterChart>").unwrap();
            let group_end = group_start + xml[group_start..].find("</c:scatterChart>").unwrap();
            if group_present {
                let pos = group_start
                    + xml[group_start..group_end].rfind("</c:ser>").unwrap()
                    + "</c:ser>".len();
                xml.insert_str(pos, "<c:dLbls/>");
            }
            let start = xml.find("<c:ser>").unwrap();
            let end = start + xml[start..].find("</c:ser>").unwrap();
            assert!(!xml[start..end].contains("<c:dLbls"));
            let pos = start + xml[start..end].find("<c:xVal>").unwrap();
            xml.insert_str(
                pos,
                concat!(
                    "<c:dLbls><c:dLbl><c:idx val=\"9\"/><c:tx><c:rich><a:bodyPr/>",
                    "<a:p><a:r><a:t>reference</a:t></a:r></a:p></c:rich></c:tx></c:dLbl></c:dLbls>"
                ),
            );
            bytes = xml.into_bytes();
        }
        output
            .start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        output.write_all(&bytes).unwrap();
    }
    load(&output.finish().unwrap().into_inner())
}
#[derive(serde::Serialize)]
struct LegacyLabels<'a> {
    show_legend_key:    &'a ShowLegendKey,
    show_value:         &'a ShowValue,
    show_category_name: &'a ShowCategoryName,
    show_series_name:   &'a ShowSeriesName,
    show_percent:       &'a ShowPercent,
    show_bubble_size:   &'a ShowBubbleSize,
    show_leader_lines:  Option<&'a ShowLeaderLines>,
    text_properties:    Option<&'a TextProperties>,
}
fn legacy_labels(value: &DataLabels) -> LegacyLabels<'_> {
    LegacyLabels {
        show_legend_key:    value.show_legend_key(),
        show_value:         value.show_value(),
        show_category_name: value.show_category_name(),
        show_series_name:   value.show_series_name(),
        show_percent:       value.show_percent(),
        show_bubble_size:   value.show_bubble_size(),
        show_leader_lines:  value.show_leader_lines(),
        text_properties:    value.text_properties(),
    }
}
#[derive(serde::Serialize)]
struct LegacyRun<'a> {
    text:           &'a str,
    run_properties: &'a RunProperties,
}
fn group_labels(book: &Workbook) -> &DataLabels {
    first_chart(book)
        .chart_space()
        .chart()
        .plot_area()
        .scatter_chart()
        .unwrap()
        .data_labels()
}
fn point_run(book: &Workbook) -> &Run {
    &first_chart(book)
        .chart_space()
        .chart()
        .plot_area()
        .scatter_chart()
        .unwrap()
        .area_chart_series_list()
        .area_chart_series()[0]
        .data_labels()
        .unwrap()
        .labels()[0]
        .text()
        .unwrap()
        .rich_text()
        .unwrap()
        .paragraph()[0]
        .run()[0]
}
fn chart1(book: &Workbook) -> String {
    let bytes = save(book);
    let mut z = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut xml = String::new();
    z.by_name("xl/charts/chart1.xml")
        .unwrap()
        .read_to_string(&mut xml)
        .unwrap();
    xml
}
fn group_tail(xml: &str) -> &str {
    let a = xml.find("<c:scatterChart>").unwrap();
    let b = a + xml[a..].find("</c:scatterChart>").unwrap();
    let group = &xml[a..b];
    &group[group.rfind("</c:ser>").unwrap() + "</c:ser>".len()..]
}
fn assert_reference_run_has_no_properties(xml: &str) {
    let text = xml.find(">reference</a:t>").unwrap();
    let start = xml[..text].rfind("<a:r>").unwrap();
    assert!(!xml[start..text].contains("<a:rPr"));
}
#[test]
fn imported_group_presence_and_run_presence_have_exact_independent_byte_costs() {
    for present in [false, true] {
        let mut book = input(present);
        let group = group_labels(&book);
        let old = bincode::serialize(&legacy_labels(group)).unwrap();
        let now = bincode::serialize(group).unwrap();
        assert_eq!(&now[..old.len()], old.as_slice());
        assert_eq!(
            now.len(),
            old.len() + 32,
            "outer option + fully encoded absent/present owner supplement"
        );
        let plain = bincode::serialize(&DataLabels::default()).unwrap();
        assert_eq!(now.len(), plain.len() + 31);
        assert_eq!(&now[now.len() - 10..now.len() - 8], &[1, u8::from(present)]);
        assert_eq!(
            &now[now.len() - 8..],
            &[0; 8],
            "appended empty lexical flag vector"
        );
        let run = point_run(&book);
        let old = bincode::serialize(&LegacyRun {
            text:           run.text(),
            run_properties: run.run_properties(),
        })
        .unwrap();
        let now = bincode::serialize(run).unwrap();
        assert_eq!(&now[..old.len()], old.as_slice());
        assert_eq!(&now[old.len()..], &[1, 0]);
        let from_json: Chart =
            serde_json::from_slice(&serde_json::to_vec(first_chart(&book)).unwrap()).unwrap();
        *first_chart_mut(&mut book) =
            bincode::deserialize(&bincode::serialize(&from_json).unwrap()).unwrap();
        let emitted = chart1(&book);
        assert_eq!(group_tail(&emitted).contains("<c:dLbls"), present);
        assert_reference_run_has_no_properties(&emitted);
        let reread = load(&save(&book));
        let emitted = chart1(&reread);
        assert_eq!(group_tail(&emitted).contains("<c:dLbls"), present);
        assert_reference_run_has_no_properties(&emitted);
    }
}
fn remove_first_absent_group_owner(value: &mut serde_json::Value) -> bool {
    match value {
        serde_json::Value::Object(object) => {
            if object.contains_key("show_legend_key")
                && object.get("parts").and_then(|v| v.get("owner_present"))
                    == Some(&serde_json::json!(false))
            {
                object.get_mut("parts").unwrap()["owner_present"] = serde_json::Value::Null;
                return true;
            }
            object.values_mut().any(remove_first_absent_group_owner)
        }
        serde_json::Value::Array(v) => v.iter_mut().any(remove_first_absent_group_owner),
        _ => false,
    }
}
#[test]
fn whole_chart_presence_field_cost_is_one_byte_and_json_default_is_compatible() {
    let book = input(false);
    let complete = serde_json::to_value(first_chart(&book)).unwrap();
    let mut legacy = complete.clone();
    assert!(remove_first_absent_group_owner(&mut legacy));
    let present: Chart = serde_json::from_value(complete).unwrap();
    let defaulted: Chart = serde_json::from_value(legacy).unwrap();
    assert_eq!(
        bincode::serialize(&present).unwrap().len(),
        bincode::serialize(&defaulted).unwrap().len() + 1
    );
    let mut run = serde_json::to_value(point_run(&book)).unwrap();
    run.as_object_mut().unwrap().remove("parts");
    let restored: Run = serde_json::from_value(run).unwrap();
    assert_eq!(restored.text(), "reference");
}
