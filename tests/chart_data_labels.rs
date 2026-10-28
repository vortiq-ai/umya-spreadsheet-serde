//! Label behavior against a pinned external XLSX, through real Chart codecs.
//! Expected flags, formulas and child order below are independently specified;
//! no candidate export is used as the expected-value oracle.
#![cfg(feature = "serde")]
use std::io::{
    Cursor,
    Read,
    Write,
};

use umya_spreadsheet::{
    Chart,
    Workbook,
    drawing::charts::{
        AreaChartSeries,
        DataLabels,
        ExtensionBooleanKind,
        ExtensionNode,
        ExtensionPayloadValue,
        SeriesPartConflictReason,
        ShowBubbleSize,
        ShowCategoryName,
        ShowLeaderLines,
        ShowLegendKey,
        ShowPercent,
        ShowSeriesName,
        ShowValue,
    },
};

const PINNED: &[u8] = include_bytes!("fixtures/pinned_chart_input.xlsx");
const LABEL_URI: &str = "{CE6537A1-D6FC-4f65-9D91-7224C49458BB}";
const RANGE_URI: &str = "{02D57815-91ED-43cb-92C2-25804820EDAC}";
const FLAGS: [&str; 7] = [
    "showLegendKey",
    "showVal",
    "showCatName",
    "showSerName",
    "showPercent",
    "showBubbleSize",
    "showLeaderLines",
];

fn read(bytes: &[u8]) -> Workbook {
    umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap()
}
fn write(book: &Workbook) -> Vec<u8> {
    let mut output = Vec::new();
    umya_spreadsheet::writer::xlsx::write_writer(book, &mut Cursor::new(&mut output)).unwrap();
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
                "ordinary-{:06}.xlsx",
                NEXT.fetch_add(1, Ordering::Relaxed)
            )),
            &output,
        )
        .unwrap();
    }
    output
}
fn chart(book: &Workbook) -> &Chart {
    book.sheet_collection()
        .iter()
        .flat_map(|s| s.chart_collection())
        .next()
        .unwrap()
}
fn chart_mut(book: &mut Workbook) -> &mut Chart {
    book.sheet_collection_mut()
        .iter_mut()
        .flat_map(|s| s.chart_collection_mut())
        .next()
        .unwrap()
}
fn first_series(book: &Workbook) -> &AreaChartSeries {
    &chart(book)
        .chart_space()
        .chart()
        .plot_area()
        .scatter_chart()
        .unwrap()
        .area_chart_series_list()
        .area_chart_series()[0]
}
fn first_series_mut(book: &mut Workbook) -> &mut AreaChartSeries {
    &mut chart_mut(book)
        .chart_space_mut()
        .chart_mut()
        .plot_area_mut()
        .scatter_chart_mut()
        .unwrap()
        .area_chart_series_list_mut()
        .area_chart_series_mut()[0]
}
fn replace_chart1_from(source: &[u8], mut edit: impl FnMut(&mut String)) -> Vec<u8> {
    let mut input = zip::ZipArchive::new(Cursor::new(source)).unwrap();
    let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for index in 0..input.len() {
        let mut member = input.by_index(index).unwrap();
        let name = member.name().to_owned();
        let mut bytes = Vec::new();
        member.read_to_end(&mut bytes).unwrap();
        if name == "xl/charts/chart1.xml" {
            let mut xml = String::from_utf8(bytes).unwrap();
            edit(&mut xml);
            bytes = xml.into_bytes();
        }
        output
            .start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        output.write_all(&bytes).unwrap();
    }
    output.finish().unwrap().into_inner()
}
fn replace_chart1(edit: impl FnMut(&mut String)) -> Vec<u8> {
    replace_chart1_from(PINNED, edit)
}
fn source_with_labels(labels: &str, series_ext: Option<&str>) -> Vec<u8> {
    replace_chart1(|xml| {
        let start = xml.find("<c:ser>").unwrap();
        let end = start + xml[start..].find("</c:ser>").unwrap();
        let original = &xml[start..end];
        assert!(!original.contains("<c:dLbls"), "pinned fixture changed");
        let x = start + original.find("<c:xVal>").unwrap();
        xml.insert_str(x, labels);
        if let Some(ext) = series_ext {
            let end = start + xml[start..].find("</c:ser>").unwrap();
            // The added point and group labels may own earlier extLst nodes;
            // the pinned source's final extLst is the direct series owner.
            if let Some(offset) = xml[start..end].rfind("</c:extLst>") {
                let child = ext
                    .strip_prefix("<c:extLst>")
                    .unwrap()
                    .strip_suffix("</c:extLst>")
                    .unwrap();
                xml.insert_str(start + offset, child);
            } else {
                xml.insert_str(end, ext);
            }
        }
    })
}
fn input_with_labels(labels: &str, series_ext: Option<&str>) -> Workbook {
    read(&source_with_labels(labels, series_ext))
}
fn xml(book: &Workbook) -> String {
    let bytes = write(book);
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut text = String::new();
    zip.by_name("xl/charts/chart1.xml")
        .unwrap()
        .read_to_string(&mut text)
        .unwrap();
    text
}
fn section<'a>(s: &'a str, begin: &str, end: &str) -> &'a str {
    let start = s.find(begin).unwrap();
    let stop = start + s[start..].find(end).unwrap() + end.len();
    &s[start..stop]
}
fn series_labels(xml: &str) -> &str {
    section(
        section(xml, "<c:ser>", "</c:ser>"),
        "<c:dLbls>",
        "</c:dLbls>",
    )
}
fn group_flag_tail(labels: &str) -> &str {
    &labels[labels.find("</c:dLbl>").unwrap() + "</c:dLbl>".len()..]
}
fn named_element<'a>(xml: &'a str, local: &str) -> &'a str {
    let position = xml.find(local).unwrap();
    let start = xml[..position].rfind('<').unwrap();
    let name = &xml[start + 1..position + local.len()];
    let close = format!("</{name}>");
    let end = start + xml[start..].find(&close).unwrap() + close.len();
    &xml[start..end]
}
fn json_then_binary(book: &Workbook) -> Workbook {
    let chart_json = serde_json::to_vec(chart(book)).unwrap();
    let from_json: Chart = serde_json::from_slice(&chart_json).unwrap();
    let binary = bincode::serialize(&from_json).unwrap();
    let from_binary: Chart = bincode::deserialize(&binary).unwrap();
    let mut result = book.clone();
    *chart_mut(&mut result) = from_binary;
    result
}
fn flag_markup(index: usize, lexical: Option<&str>) -> String {
    match lexical {
        None => String::new(),
        Some("") => format!("<c:{}/>", FLAGS[index]),
        Some(v) => format!("<c:{} val=\"{}\"/>", FLAGS[index], v),
    }
}
fn label_fixture(target: usize, lexical: Option<&str>) -> (String, usize) {
    let sentinel = if target == 0 { 1 } else { 0 };
    let mut flags = [
        String::new(),
        String::new(),
        String::new(),
        String::new(),
        String::new(),
        String::new(),
        String::new(),
    ];
    flags[sentinel] = flag_markup(sentinel, Some(""));
    flags[target] = flag_markup(target, lexical);
    let point = concat!(
        "<c:dLbl><c:idx val=\"9\"/><c:dLblPos val=\"outEnd\"/>",
        "<c:showVal val=\"0\"/><c:separator>point &amp; text</c:separator></c:dLbl>"
    );
    let mut content = format!("<c:dLbls>{point}");
    for flag in &flags[..6] {
        content.push_str(flag);
    }
    content.push_str("<c:separator>owner &amp; text</c:separator>");
    content.push_str(&flags[6]);
    content.push_str("</c:dLbls>");
    (content, sentinel)
}
#[derive(Clone, Copy, Debug)]
enum Mutation {
    Current,
    Deprecated,
    Setter,
}
fn flag_value(labels: &DataLabels, index: usize) -> Option<bool> {
    match index {
        0 => Some(labels.show_legend_key().val()),
        1 => Some(labels.show_value().val()),
        2 => Some(labels.show_category_name().val()),
        3 => Some(labels.show_series_name().val()),
        4 => Some(labels.show_percent().val()),
        5 => Some(labels.show_bubble_size().val()),
        6 => labels.show_leader_lines().map(ShowLeaderLines::val),
        _ => panic!("unknown flag"),
    }
}
// The compatibility matrix intentionally exercises deprecated mutable aliases.
#[allow(deprecated)]
fn mutate_flag(labels: &mut DataLabels, flag: usize, method: Mutation, value: bool) -> bool {
    macro_rules! leaf {
        ($current:ident, $deprecated:ident, $setter:ident, $ty:ty) => {{
            match method {
                Mutation::Current => {
                    labels.$current().set_val(value);
                }
                Mutation::Deprecated => {
                    labels.$deprecated().set_val(value);
                }
                Mutation::Setter => {
                    let mut v = <$ty>::default();
                    v.set_val(value);
                    labels.$setter(v);
                }
            }
            true
        }};
    }
    match flag {
        0 => leaf!(
            show_legend_key_mut,
            get_show_legend_key_mut,
            set_show_legend_key,
            ShowLegendKey
        ),
        1 => leaf!(
            show_value_mut,
            get_show_value_mut,
            set_show_value,
            ShowValue
        ),
        2 => leaf!(
            show_category_name_mut,
            get_show_category_name_mut,
            set_show_category_name,
            ShowCategoryName
        ),
        3 => leaf!(
            show_series_name_mut,
            get_show_series_name_mut,
            set_show_series_name,
            ShowSeriesName
        ),
        4 => leaf!(
            show_percent_mut,
            get_show_percent_mut,
            set_show_percent,
            ShowPercent
        ),
        5 => leaf!(
            show_bubble_size_mut,
            get_show_bubble_size_mut,
            set_show_bubble_size,
            ShowBubbleSize
        ),
        6 => match method {
            Mutation::Current => labels
                .show_leader_lines_mut()
                .map(|v| {
                    v.set_val(value);
                })
                .is_some(),
            Mutation::Deprecated => labels
                .get_show_leader_lines_mut()
                .map(|v| {
                    v.set_val(value);
                })
                .is_some(),
            Mutation::Setter => {
                let mut v = ShowLeaderLines::default();
                v.set_val(value);
                labels.set_show_leader_lines(v);
                true
            }
        },
        _ => panic!("unknown flag"),
    }
}
#[test]
// The compatibility matrix intentionally exercises deprecated mutable aliases.
#[allow(deprecated)]
fn every_label_flag_mutation_path_survives_whole_chart_json_binary_xlsx_and_reread() {
    for target in 0..FLAGS.len() {
        for lexical in [None, Some(""), Some("true"), Some("false")] {
            for method in [Mutation::Current, Mutation::Deprecated, Mutation::Setter] {
                let (fixture, sentinel) = label_fixture(target, lexical);
                let input = input_with_labels(&fixture, None);
                let mut book = json_then_binary(&input);
                let typed = flag_value(first_series(&book).data_labels().unwrap(), target);
                let expected = match lexical {
                    None if target == 6 => None,
                    None => Some(false),
                    Some("") | Some("true") => Some(true),
                    Some("false") => Some(false),
                    _ => unreachable!(),
                };
                assert_eq!(typed, expected, "semantic default {target} {lexical:?}");
                let initial = xml(&book);
                let group = group_flag_tail(series_labels(&initial));
                assert!(
                    group.contains(&flag_markup(sentinel, Some(""))),
                    "initial {target} {lexical:?} {method:?}: {group}"
                );
                if let Some(v) = lexical {
                    assert!(
                        group.contains(&flag_markup(target, Some(v))),
                        "initial {target} {lexical:?}: {group}"
                    );
                } else {
                    assert!(!group.contains(&format!("<c:{}", FLAGS[target])));
                }
                if matches!(method, Mutation::Setter) {
                    let saved = first_series(&book).data_labels().unwrap().clone();
                    first_series_mut(&mut book).set_data_labels(saved);
                }
                let labels = match method {
                    Mutation::Deprecated => {
                        first_series_mut(&mut book).get_data_labels_mut().unwrap()
                    }
                    _ => first_series_mut(&mut book).data_labels_mut().unwrap(),
                };
                let modified = mutate_flag(labels, target, method, false);
                let out = xml(&book);
                let labels = series_labels(&out);
                let group = group_flag_tail(labels);
                assert!(labels.contains("<c:idx val=\"9\"/>"));
                assert!(labels.contains("<c:showVal val=\"0\"/>"));
                assert!(labels.contains("point &amp; text"));
                assert!(
                    group.contains(&flag_markup(sentinel, Some(""))),
                    "sentinel {target} {method:?}: {group}"
                );
                assert!(group.contains("owner &amp; text"));
                if modified {
                    assert!(
                        group.contains(&flag_markup(target, Some("0"))),
                        "target {target} {method:?}: {group}"
                    );
                } else {
                    assert_eq!(target, 6);
                    assert!(lexical.is_none());
                    assert!(!group.contains("<c:showLeaderLines"));
                }
                let reload = json_then_binary(&read(&write(&book)));
                let out = xml(&reload);
                let group = group_flag_tail(series_labels(&out));
                assert!(group.contains(&flag_markup(sentinel, Some(""))));
                if modified {
                    assert!(group.contains(&flag_markup(target, Some("0"))));
                } else {
                    assert!(!group.contains("<c:showLeaderLines"));
                }
            }
        }
    }
}

#[test]
fn chart_json_binary_preserves_populated_field_table_range_roles_and_child_order() {
    let labels = include_str!("fixtures/source_label_roles.xml").trim();
    let range = include_str!("fixtures/source_series_range.xml").trim();
    assert!(range.contains(RANGE_URI));
    let mut book = json_then_binary(&input_with_labels(labels, Some(range)));
    for pass in 0..2 {
        let output = xml(&book);
        let series = section(&output, "<c:ser>", "</c:ser>");
        let point = section(series_labels(&output), "<c:dLbl>", "</c:dLbl>");
        assert!(point.contains("'Chart A'!$D$2<"));
        assert!(point.contains(">Red</"));
        assert!(
            quick_xml::escape::unescape(point)
                .unwrap()
                .contains("'Chart A'!$B$3<")
        );
        assert!(
            quick_xml::escape::unescape(series)
                .unwrap()
                .contains("'Chart A'!$B$3:$B$5<")
        );
        let field_cache = named_element(point, "dlblFieldTableCache");
        assert!(field_cache.contains(">3</"));
        let range_cache = named_element(series, "dlblRangeCache");
        let three = range_cache.find(">3</").unwrap();
        let two = range_cache.find(">2</").unwrap();
        let eight = range_cache.find(">8</").unwrap();
        assert!(three < two && two < eight);
        let group = group_flag_tail(series_labels(&output));
        let a = group.find("showDataLabelsRange").unwrap();
        let b = group.find("showLeaderLines").unwrap();
        let c = group.find("leaderLines").unwrap();
        assert!(
            a < b && b < c,
            "extension children moved or regrouped: {group}"
        );
        assert_eq!(group.matches(LABEL_URI).count(), 1);
        if pass == 0 {
            let reread = json_then_binary(&read(&write(&book)));
            let actual = xml(&reread);
            let restored = section(&actual, "<c:ser>", "</c:ser>");
            assert!(
                quick_xml::escape::unescape(restored)
                    .unwrap()
                    .contains("'Chart A'!$B$3:$B$5<")
            );
            assert!(
                quick_xml::escape::unescape(restored)
                    .unwrap()
                    .contains("'Chart A'!$B$3<")
            );
            book = reread;
        }
    }
}

// The compatibility matrix intentionally exercises deprecated mutable aliases.
#[allow(deprecated)]
fn edited_first_run(book: &mut Workbook, method: Option<Mutation>) {
    let rich = first_series_mut(book)
        .data_labels_mut()
        .unwrap()
        .labels_mut()[0]
        .text_mut()
        .unwrap()
        .rich_text_mut()
        .unwrap();
    let paragraph = &mut rich.paragraph_mut()[0];
    let mut run = paragraph.remove_run(0).unwrap();
    match method {
        None => run.set_text("edited"),
        Some(Mutation::Current) => {
            run.run_properties_mut().set_bold("1");
        }
        Some(Mutation::Deprecated) => {
            run.get_run_properties_mut().set_bold("1");
        }
        Some(Mutation::Setter) => {
            let mut props = umya_spreadsheet::drawing::RunProperties::default();
            props.set_bold("1");
            run.set_run_properties(props);
        }
    }
    assert!(paragraph.insert_run(0, run));
}

#[test]
fn rich_label_run_presence_and_positioned_fields_follow_ordinary_chart_mutations() {
    let labels = concat!(
        "<c:dLbls><c:dLbl><c:idx val=\"9\"/><c:layout><c:manualLayout>",
        "<c:xMode val=\"edge\"/><c:x val=\"0.1\"/></c:manualLayout></c:layout>",
        "<c:tx><c:rich><a:bodyPr vert=\"vert270\" anchorCtr=\"true\"/><a:p><a:pPr lvl=\"2\"/>",
        "<a:r><a:t>before</a:t></a:r><a:fld id=\"{01234567-89AB-CDEF-0123-456789ABCDEF}\" \
         type=\"slidenum\"><a:t>4</a:t></a:fld>",
        "<a:r><a:rPr u=\"wavyDbl\" kern=\"1200\" \
         baseline=\"-2000\"><a:uFillTx/></a:rPr><a:t>after</a:t></a:r>",
        "</a:p></c:rich></c:tx><c:numFmt formatCode=\"0.00\"/><c:dLblPos \
         val=\"t\"/><c:showVal/></c:dLbl></c:dLbls>"
    );
    for method in [Mutation::Current, Mutation::Deprecated, Mutation::Setter] {
        let mut book = json_then_binary(&input_with_labels(labels, None));
        edited_first_run(&mut book, None);
        let written = xml(&book);
        let point = section(series_labels(&written), "<c:dLbl>", "</c:dLbl>");
        let rich = section(point, "<c:rich>", "</c:rich>");
        assert!(rich.contains("vert=\"vert270\""));
        assert!(rich.contains("anchorCtr=\"true\""));
        assert!(!rich.contains("<a:lstStyle"));
        assert!(rich.contains("lvl=\"2\""));
        let first = section(rich, "<a:r>", "</a:r>");
        assert!(first.contains(">edited</a:t>"));
        assert!(!first.contains("<a:rPr"));
        let fld = rich.find("<a:fld").unwrap();
        let edited = rich.find(">edited</a:t>").unwrap();
        let after = rich.find(">after</a:t>").unwrap();
        assert!(edited < fld && fld < after);
        assert!(rich.contains("u=\"wavyDbl\""));
        assert!(rich.contains("kern=\"1200\""));
        assert!(rich.contains("baseline=\"-2000\""));
        assert!(rich.contains("<a:uFillTx/>"));
        assert!(point.contains("<c:numFmt formatCode=\"0.00\"/>"));
        book = json_then_binary(&read(&write(&book)));
        edited_first_run(&mut book, Some(method));
        let written = xml(&book);
        let point = section(series_labels(&written), "<c:dLbl>", "</c:dLbl>");
        let rich = section(point, "<c:rich>", "</c:rich>");
        let first = section(rich, "<a:r>", "</a:r>");
        assert!(first.contains("<a:rPr"));
        assert!(first.contains("b=\"1\""));
        assert!(rich.find(">edited</a:t>").unwrap() < rich.find("<a:fld").unwrap());
        assert!(rich.find("<a:fld").unwrap() < rich.find(">after</a:t>").unwrap());
        let reread = json_then_binary(&read(&write(&book)));
        let persisted = xml(&reread);
        let point = section(series_labels(&persisted), "<c:dLbl>", "</c:dLbl>");
        assert!(point.contains("<a:fld"));
        assert!(point.contains("b=\"1\""));
    }
}

fn force_inconsistent_series_owner(value: &mut serde_json::Value) -> bool {
    match value {
        serde_json::Value::Object(object) => {
            let target = object.contains_key("show_legend_key")
                && object
                    .get("parts")
                    .and_then(|p| p.get("labels"))
                    .and_then(|v| v.as_array())
                    .is_some_and(|labels| !labels.is_empty());
            if target {
                object.get_mut("parts").unwrap()["owner_present"] = serde_json::json!(false);
                object.get_mut("parts").unwrap()["present_flags"] = serde_json::json!(0);
                return true;
            }
            object.values_mut().any(force_inconsistent_series_owner)
        }
        serde_json::Value::Array(array) => array.iter_mut().any(force_inconsistent_series_owner),
        _ => false,
    }
}

#[test]
fn inconsistent_series_owner_json_reports_conflict_without_dropping_stored_children() {
    let (labels, _) = label_fixture(1, Some("true"));
    let mut book = input_with_labels(&labels, None);
    let mut value = serde_json::to_value(chart(&book)).unwrap();
    assert!(force_inconsistent_series_owner(&mut value));
    let from_json: Chart = serde_json::from_value(value).unwrap();
    let bytes = bincode::serialize(&from_json).unwrap();
    *chart_mut(&mut book) = bincode::deserialize(&bytes).unwrap();
    assert_eq!(
        first_series(&book).data_labels().unwrap().conflict(),
        Some(SeriesPartConflictReason::ConflictingChoice)
    );
    assert!(
        chart(&book)
            .chart_space()
            .chart()
            .plot_area()
            .series_part_conflicts()
            .iter()
            .any(|c| c.reason == SeriesPartConflictReason::ConflictingChoice)
    );
    let written = xml(&book);
    let labels = series_labels(&written);
    let tail = group_flag_tail(labels);
    assert!(labels.contains("<c:idx val=\"9\"/>"));
    assert!(tail.contains("<c:showVal val=\"true\"/>"));
    let reread = json_then_binary(&read(&write(&book)));
    let written = xml(&reread);
    let labels = series_labels(&written);
    assert!(labels.contains("<c:idx val=\"9\"/>"));
    assert!(group_flag_tail(labels).contains("<c:showVal val=\"true\"/>"));
}

fn assert_unknown_namespace_and_mixed_content(xml: &str) {
    use quick_xml::{
        NsReader,
        events::Event,
        name::{
            PrefixDeclaration,
            ResolveResult,
        },
    };
    let mut reader = NsReader::from_reader(xml.as_bytes());
    let mut in_child = false;
    let mut child_seen = 0;
    let mut sequence = Vec::new();
    loop {
        match reader.read_event().unwrap() {
            Event::Start(e) => {
                let (ns, local) = reader.resolver().resolve_element(e.name());
                if in_child && local.as_ref() == b"leaf" {
                    assert!(matches!(ns,ResolveResult::Bound(v) if v.as_ref()==b"urn:default"));
                    sequence.push("leaf".to_owned());
                }
                if local.as_ref() == b"child"
                    && matches!(ns,ResolveResult::Bound(v) if v.as_ref()==b"urn:inner")
                {
                    child_seen += 1;
                    in_child = true;
                    let bound = reader.resolver().bindings().any(|(prefix, uri)| {
                        matches!(prefix,PrefixDeclaration::Named(p) if p==b"q")
                            && uri.as_ref() == b"urn:attr"
                    });
                    assert!(bound, "QName value prefix lost its in-scope binding");
                    let typ = e
                        .attributes()
                        .map(Result::unwrap)
                        .any(|a| a.key.as_ref() == b"type" && a.value.as_ref() == b"q:Kind");
                    assert!(typ, "QName-valued attribute was lost or rewritten");
                }
            }
            Event::Empty(e) if in_child => {
                let (ns, local) = reader.resolver().resolve_element(e.name());
                if local.as_ref() == b"leaf" {
                    assert!(matches!(ns,ResolveResult::Bound(v) if v.as_ref()==b"urn:default"));
                    sequence.push("leaf".to_owned());
                }
            }
            Event::Text(e) if in_child => {
                sequence.push(format!("text:{}", String::from_utf8_lossy(e.as_ref())))
            }
            Event::CData(e) if in_child => {
                sequence.push(format!("cdata:{}", String::from_utf8_lossy(e.as_ref())))
            }
            Event::Comment(e) if in_child => {
                sequence.push(format!("comment:{}", String::from_utf8_lossy(e.as_ref())))
            }
            Event::PI(e) if in_child => {
                sequence.push(format!("pi:{}", String::from_utf8_lossy(e.as_ref())))
            }
            Event::End(e) if e.local_name().as_ref() == b"child" && in_child => in_child = false,
            Event::Eof => break,
            _ => (),
        }
    }
    assert_eq!(child_seen, 1);
    assert_eq!(
        sequence,
        [
            "text:left",
            "cdata:x<y",
            "comment:keep",
            "pi:audit keep",
            "text:right",
            "leaf"
        ]
    );
}

#[test]
fn opaque_extension_keeps_qname_attribute_binding_rebinding_default_alias_and_mixed_content() {
    let labels = include_str!("fixtures/source_unknown_namespace.xml").trim();
    let source = source_with_labels(labels, None);
    let source = replace_chart1_from(&source, |xml| {
        let start = xml.find("<c:chartSpace").unwrap();
        let close = start + xml[start..].find('>').unwrap();
        xml.insert_str(close, " xmlns:q=\"urn:wrong-outer\"");
    });
    let book = json_then_binary(&read(&source));
    let first = xml(&book);
    assert_unknown_namespace_and_mixed_content(&first);
    let reread = json_then_binary(&read(&write(&book)));
    let second = xml(&reread);
    assert_unknown_namespace_and_mixed_content(&second);
}

#[test]
fn two_empty_known_label_shells_stay_stored_and_produce_two_distinct_export_records() {
    use sha2::{
        Digest,
        Sha256,
    };
    let labels = format!(
        concat!(
            "<c:dLbls><c:dLbl><c:idx val=\"9\"/><c:extLst><c:ext uri=\"{}\"/></c:extLst></c:dLbl>",
            "<c:extLst><c:ext uri=\"{}\"/></c:extLst></c:dLbls>"
        ),
        LABEL_URI, LABEL_URI
    );
    let source = source_with_labels(&labels, None);
    let hash = Sha256::digest(&source)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let initial = read(&source);
    let stored = json_then_binary(&initial);
    let labels = first_series(&stored).data_labels().unwrap();
    let group = labels.extensions().unwrap();
    let point = labels.labels()[0].extensions().unwrap();
    assert_eq!(group.entries.len(), 1);
    assert_eq!(point.entries.len(), 1);
    assert!(group.entries[0].is_empty_known_label_shell("dLbls"));
    assert!(point.entries[0].is_empty_known_label_shell("dLbl"));
    let g = group.empty_known_label_omissions(&hash).unwrap();
    let p = point.empty_known_label_omissions(&hash).unwrap();
    assert_eq!(g.len(), 1);
    assert_eq!(p.len(), 1);
    for record in [&g[0], &p[0]] {
        assert_eq!(record.input_sha256, hash);
        assert_eq!(record.chart_part, "xl/charts/chart1.xml");
        assert_eq!(record.extension_occurrence, 0);
        assert_eq!(record.uri, LABEL_URI);
    }
    assert_ne!(g[0].owner_path, p[0].owner_path);
    assert_ne!(
        group.source_part_sha256(),
        hash,
        "part hash must not stand in for whole input"
    );
    let written = xml(&stored);
    assert!(!series_labels(&written).contains(LABEL_URI));
    // Export must not mutate the authoritative stored chart.
    assert_eq!(
        first_series(&stored)
            .data_labels()
            .unwrap()
            .extensions()
            .unwrap()
            .entries
            .len(),
        1
    );
    assert_eq!(
        first_series(&stored).data_labels().unwrap().labels()[0]
            .extensions()
            .unwrap()
            .entries
            .len(),
        1
    );
    let reread = read(&write(&stored));
    let written = xml(&reread);
    assert!(!series_labels(&written).contains(LABEL_URI));
}

#[test]
fn moved_registered_extension_is_reported_for_its_live_owner_without_a_panic() {
    let labels=format!(concat!("<c:dLbls><c:dLbl><c:idx val=\"9\"/></c:dLbl>",
        "<c:extLst><c:ext uri=\"{}\" xmlns:c15=\"http://schemas.microsoft.com/office/drawing/2012/chart\">",
        "<c15:showLeaderLines val=\"1\"/></c:ext></c:extLst></c:dLbls>"),LABEL_URI);
    let mut book = json_then_binary(&input_with_labels(&labels, None));
    {
        let labels = first_series_mut(&mut book).data_labels_mut().unwrap();
        let moved = labels.extensions().unwrap().clone();
        labels.set_extensions(None);
        labels.labels_mut()[0].set_extensions(Some(moved));
    }
    assert!(
        chart(&book)
            .chart_space()
            .chart()
            .plot_area()
            .series_part_conflicts()
            .iter()
            .any(|v| v.reason == SeriesPartConflictReason::NotAllowed)
    );
    let assert_stored_payload = |book: &Workbook| {
        let extension = first_series(book).data_labels().unwrap().labels()[0]
            .extensions()
            .unwrap();
        assert_eq!(extension.entries.len(), 1);
        assert!(
            matches!(&extension.entries[0].nodes[0],ExtensionNode::Known(payload)
            if matches!(&payload.value,ExtensionPayloadValue::Boolean(value)
                if value.kind==ExtensionBooleanKind::ShowLeaderLines && value.value.attribute()==Some("1")))
        );
    };
    assert_stored_payload(&book);
    // Entire Chart codecs retain authoritative typed kind and lexical value;
    // no raw root-name shadow is required to make the JSON assertion pass.
    let persisted = json_then_binary(&book);
    assert_stored_payload(&persisted);
    let written = xml(&book);
    let labels = series_labels(&written);
    assert!(labels.contains("<c:idx val=\"9\"/>"));
    assert!(
        !labels.contains("<c15:showLeaderLines"),
        "illegal point owner emitted"
    );
    assert_stored_payload(&book);
}

#[test]
fn both_label_delete_choice_branches_survive_full_chart_roundtrip() {
    let labels = concat!(
        "<c:dLbls><c:dLbl><c:idx val=\"9\"/><c:delete val=\"1\"/></c:dLbl>",
        "<c:delete/></c:dLbls>"
    );
    let book = json_then_binary(&input_with_labels(labels, None));
    for bytes in [write(&book), write(&read(&write(&book)))] {
        let reread = read(&bytes);
        let exported = xml(&reread);
        let labels = series_labels(&exported);
        assert!(labels.contains("<c:dLbl><c:idx val=\"9\"/><c:delete val=\"1\"/></c:dLbl>"));
        assert!(group_flag_tail(labels).contains("<c:delete/>"));
        assert!(!group_flag_tail(labels).contains("<c:showVal"));
    }
}

#[test]
fn conflicting_delete_choice_is_reported_and_does_not_erase_stored_flag_or_point() {
    let (labels, _) = label_fixture(1, Some("true"));
    let mut book = json_then_binary(&input_with_labels(&labels, None));
    let mut delete = umya_spreadsheet::drawing::charts::XmlBoolean::default();
    delete.set_value(true);
    first_series_mut(&mut book)
        .data_labels_mut()
        .unwrap()
        .set_delete(Some(delete));
    assert_eq!(
        first_series(&book).data_labels().unwrap().conflict(),
        Some(SeriesPartConflictReason::ConflictingChoice)
    );
    assert!(
        chart(&book)
            .chart_space()
            .chart()
            .plot_area()
            .series_part_conflicts()
            .iter()
            .any(|c| c.reason == SeriesPartConflictReason::ConflictingChoice)
    );
    let before = serde_json::to_value(chart(&book)).unwrap();
    let output = xml(&book);
    let labels = series_labels(&output);
    let tail = group_flag_tail(labels);
    assert!(labels.contains("<c:idx val=\"9\"/>"));
    assert!(tail.contains("<c:delete val=\"1\"/>"));
    assert!(!tail.contains("<c:showVal"));
    assert_eq!(
        serde_json::to_value(chart(&book)).unwrap(),
        before,
        "export changed stored source"
    );
    let json: Chart = serde_json::from_value(before).unwrap();
    let binary: Chart = bincode::deserialize(&bincode::serialize(&json).unwrap()).unwrap();
    let mut restored = book.clone();
    *chart_mut(&mut restored) = binary;
    assert_eq!(
        first_series(&restored).data_labels().unwrap().conflict(),
        Some(SeriesPartConflictReason::ConflictingChoice)
    );
    assert!(
        first_series(&restored)
            .data_labels()
            .unwrap()
            .show_value()
            .val()
    );
}

fn make_optional_series_label_owner_inconsistent(value: &mut serde_json::Value) -> bool {
    match value {
        serde_json::Value::Object(object) => {
            let target = object.contains_key("show_legend_key")
                && object
                    .get("parts")
                    .and_then(|p| p.get("separator"))
                    .and_then(|v| v.as_str())
                    == Some("series-owner-sentinel");
            if target {
                let parts = object.get_mut("parts").unwrap();
                parts["separator"] = serde_json::Value::Null;
                parts["owner_present"] = serde_json::json!(false);
                parts["present_flags"] = serde_json::json!(0);
                return true;
            }
            object
                .values_mut()
                .any(make_optional_series_label_owner_inconsistent)
        }
        serde_json::Value::Array(array) => array
            .iter_mut()
            .any(make_optional_series_label_owner_inconsistent),
        _ => false,
    }
}
#[test]
fn series_some_owner_wins_over_inconsistent_empty_inner_absence() {
    let mut book = input_with_labels(
        "<c:dLbls><c:separator>series-owner-sentinel</c:separator></c:dLbls>",
        None,
    );
    let mut value = serde_json::to_value(chart(&book)).unwrap();
    assert!(make_optional_series_label_owner_inconsistent(&mut value));
    let from_json: Chart = serde_json::from_value(value).unwrap();
    *chart_mut(&mut book) = bincode::deserialize(&bincode::serialize(&from_json).unwrap()).unwrap();
    assert!(first_series(&book).data_labels().is_some());
    assert!(
        chart(&book)
            .chart_space()
            .chart()
            .plot_area()
            .series_part_conflicts()
            .iter()
            .any(|c| c.reason == SeriesPartConflictReason::ConflictingChoice)
    );
    let out = xml(&book);
    let series = section(&out, "<c:ser>", "</c:ser>");
    assert!(series.contains("<c:dLbls></c:dLbls>"));
    let reread = json_then_binary(&read(&write(&book)));
    assert!(first_series(&reread).data_labels().is_some());
    let out = xml(&reread);
    let series = section(&out, "<c:ser>", "</c:ser>");
    assert!(series.contains("<c:dLbls></c:dLbls>"));
}

#[test]
fn direct_chart_json_and_direct_chart_bincode_each_preserve_imported_label_state() {
    let (labels, _) = label_fixture(1, Some("true"));
    let source = input_with_labels(&labels, None);
    let before = serde_json::to_value(chart(&source)).unwrap();
    for json_route in [true, false] {
        let restored: Chart = if json_route {
            serde_json::from_slice(&serde_json::to_vec(chart(&source)).unwrap()).unwrap()
        } else {
            bincode::deserialize(&bincode::serialize(chart(&source)).unwrap()).unwrap()
        };
        assert_eq!(serde_json::to_value(&restored).unwrap(), before);
        let mut book = source.clone();
        *chart_mut(&mut book) = restored;
        let first = xml(&book);
        let labels = series_labels(&first);
        let tail = group_flag_tail(labels);
        assert!(tail.contains("<c:showVal val=\"true\"/>"));
        assert!(tail.contains("<c:showLegendKey/>"));
        assert!(labels.contains("<c:idx val=\"9\"/>"));
        first_series_mut(&mut book)
            .data_labels_mut()
            .unwrap()
            .show_value_mut()
            .set_val(false);
        let reread = read(&write(&book));
        let second = xml(&reread);
        let tail = group_flag_tail(series_labels(&second));
        assert!(tail.contains("<c:showVal val=\"0\"/>"));
        assert!(tail.contains("<c:showLegendKey/>"));
    }
}

#[test]
fn source_cache_boundary_does_not_materialize_or_repair_false_label_owner_metadata() {
    let labels=format!("<c:dLbls><c:separator>series-owner-sentinel</c:separator><c:extLst><c:ext uri=\"{LABEL_URI}\" xmlns:c15=\"http://schemas.microsoft.com/office/drawing/2012/chart\"><c15:showLeaderLines val=\"1\"/></c:ext></c:extLst></c:dLbls>");
    let mut book = input_with_labels(&labels, None);
    let mut value = serde_json::to_value(chart(&book)).unwrap();
    assert!(make_optional_series_label_owner_inconsistent(&mut value));
    *chart_mut(&mut book) = serde_json::from_value(value).unwrap();
    // The pinned source may contain ordinary caches. First strip those, then
    // compare the entire authoritative chart across an idempotent boundary.
    assert_eq!(
        serde_json::to_value(first_series(&book).data_labels().unwrap()).unwrap()["parts"]
            ["owner_present"],
        serde_json::json!(false)
    );
    chart_mut(&mut book).strip_source_caches();
    let before = serde_json::to_value(chart(&book)).unwrap();
    assert_eq!(
        serde_json::to_value(first_series(&book).data_labels().unwrap()).unwrap()["parts"]
            ["owner_present"],
        serde_json::json!(false)
    );
    assert!(
        chart(&book)
            .chart_space()
            .chart()
            .plot_area()
            .series_part_conflicts()
            .iter()
            .any(|conflict| conflict.reason == SeriesPartConflictReason::ConflictingChoice)
    );
    chart_mut(&mut book).strip_source_caches();
    assert_eq!(serde_json::to_value(chart(&book)).unwrap(), before);
    let restored: Chart = bincode::deserialize(&bincode::serialize(chart(&book)).unwrap()).unwrap();
    assert_eq!(serde_json::to_value(&restored).unwrap(), before);
}

#[test]
fn list_style_aliases_and_expanded_spacing_keep_modeled_children_and_bound_unknown_subtrees() {
    let labels = concat!(
        "<c:dLbls><c:dLbl><c:idx val=\"9\"/><c:tx><c:rich>",
        "<a:bodyPr/><a:lstStyle><a:lvl1pPr algn=\"ctr\"><a:spcBef>",
        "<a:spcPct val=\"125000\"></a:spcPct></a:spcBef><a:spcAft>",
        "<a:spcPct val=\"75000\"/></a:spcAft><a:defRPr u=\"sng\"/>",
        "</a:lvl1pPr></a:lstStyle><a:p><a:r><a:t>kept</a:t></a:r></a:p>",
        "</c:rich></c:tx></c:dLbl></c:dLbls>"
    );
    for alias in [false, true] {
        let input =
            if alias {
                labels.replace("a:", "d:").replace("<c:rich>",
                "<c:rich xmlns:d=\"http://schemas.openxmlformats.org/drawingml/2006/main\">")
                .replace("</d:lvl1pPr>",
                    "<d:future><d:spcBef><d:spcPct val=\"999\"/></d:spcBef></d:future></d:lvl1pPr>")
            } else {
                labels.to_owned()
            };
        let book = json_then_binary(&input_with_labels(&input, None));
        for current in [book.clone(), read(&write(&book))] {
            let output = xml(&current);
            let list = section(&output, "<a:lstStyle>", "</a:lstStyle>");
            assert!(list.contains("algn=\"ctr\""));
            assert!(list.contains("<a:spcBef><a:spcPct val=\"125000\"/></a:spcBef>"));
            assert!(list.contains("<a:spcAft><a:spcPct val=\"75000\"/></a:spcAft>"));
            assert!(list.contains("u=\"sng\""));
            assert!(!list.contains("999"));
            assert!(output.contains(">kept</a:t>"));
        }
    }
}

fn separate_chart_routes(book: &Workbook) -> [Workbook; 3] {
    let mut json = book.clone();
    *chart_mut(&mut json) =
        serde_json::from_slice(&serde_json::to_vec(chart(book)).unwrap()).unwrap();
    let mut binary = book.clone();
    *chart_mut(&mut binary) =
        bincode::deserialize(&bincode::serialize(chart(book)).unwrap()).unwrap();
    [book.clone(), json, binary]
}
fn assert_text_owners(book: &Workbook, expected: Option<&str>) {
    let labels = first_series(book).data_labels().unwrap();
    assert_eq!(labels.separator(), expected);
    let point = &labels.labels()[0];
    assert_eq!(point.separator(), expected);
    let paragraph = &point.text().unwrap().rich_text().unwrap().paragraph()[0];
    assert_eq!(paragraph.run()[0].text(), expected.unwrap_or(""));
    assert_eq!(paragraph.fields()[0].field.text.as_deref(), expected);
    assert_eq!(paragraph.run()[1].text(), "sentinel");
}
#[test]
fn separators_fields_and_runs_decode_cdata_entities_and_whitespace_once_through_all_chart_routes() {
    for (source, expected) in [
        ("<![CDATA[raw <& literal &amp;]]>", "raw <& literal &amp;"),
        (
            "  before<![CDATA[<&]]>&amp;&#65;&apos;after  ",
            "  before<&&A'after  ",
        ),
        (" \t spaces \n ", " \t spaces \n "),
        ("", ""),
    ] {
        let labels = format!(
            concat!(
                "<c:dLbls><c:dLbl><c:idx val=\"9\"/><c:tx><c:rich><a:bodyPr/><a:p>",
                "<a:r><a:t>{source}</a:t></a:r><a:fld \
                 id=\"{{01234567-89AB-CDEF-0123-456789ABCDEF}}\"><a:t>{source}</a:t></a:fld>",
                "<a:r><a:t>sentinel</a:t></a:r></a:p></c:rich></c:tx><c:separator>{source}</c:\
                 separator></c:dLbl>",
                "<c:separator>{source}</c:separator></c:dLbls>"
            ),
            source = source
        );
        for route in separate_chart_routes(&input_with_labels(&labels, None)) {
            assert_text_owners(&route, Some(expected));
            let reloaded = read(&write(&route));
            assert_text_owners(&reloaded, Some(expected));
            assert!(xml(&reloaded).contains("sentinel"));
        }
    }
    let absent = concat!(
        "<c:dLbls><c:dLbl><c:idx val=\"9\"/><c:tx><c:rich><a:bodyPr/><a:p>",
        "<a:r><a:t/></a:r><a:fld id=\"{01234567-89AB-CDEF-0123-456789ABCDEF}\"/>",
        "<a:r><a:t>sentinel</a:t></a:r></a:p></c:rich></c:tx></c:dLbl></c:dLbls>"
    );
    for route in separate_chart_routes(&input_with_labels(absent, None)) {
        assert_text_owners(&route, None);
        assert_text_owners(&read(&write(&route)), None);
    }
}
#[test]
fn padded_boolean_flags_keep_lexical_values_until_only_the_edited_flag_is_canonicalized() {
    for lexical in [" true ", " false ", " 0 ", " 1 "] {
        let expected = matches!(lexical, " true " | " 1 ");
        let flags = FLAGS
            .iter()
            .map(|name| format!("<c:{name} val=\"{lexical}\"/>"))
            .collect::<String>();
        let labels = format!(
            "<c:dLbls><c:dLbl><c:idx val=\"9\"/><c:showVal \
             val=\"{lexical}\"/></c:dLbl>{flags}</c:dLbls>"
        );
        let source = input_with_labels(&labels, None);
        for route in separate_chart_routes(&source) {
            let typed = first_series(&route).data_labels().unwrap();
            assert_eq!(typed.conflict(), None);
            for bit in 0..7 {
                assert_eq!(flag_value(typed, bit), Some(expected));
            }
            assert_eq!(
                typed.labels()[0]
                    .flags()
                    .show_value
                    .as_ref()
                    .unwrap()
                    .attribute(),
                Some(lexical)
            );
            let reloaded = read(&write(&route));
            let typed = first_series(&reloaded).data_labels().unwrap();
            assert_eq!(typed.conflict(), None);
            for bit in 0..7 {
                assert_eq!(flag_value(typed, bit), Some(expected));
            }
            let encoded = serde_json::to_value(typed).unwrap();
            assert_eq!(
                encoded["parts"]["lexical_flags"].as_array().unwrap().len(),
                7
            );
        }
        for target in 0..7 {
            for method in [Mutation::Current, Mutation::Deprecated, Mutation::Setter] {
                let mut edited = json_then_binary(&source);
                assert!(mutate_flag(
                    first_series_mut(&mut edited).data_labels_mut().unwrap(),
                    target,
                    method,
                    !expected
                ));
                let edited = json_then_binary(&edited);
                let written = write(&edited);
                let reloaded = read(&written);
                let typed = first_series(&reloaded).data_labels().unwrap();
                for bit in 0..7 {
                    assert_eq!(
                        flag_value(typed, bit),
                        Some(if bit == target { !expected } else { expected })
                    );
                }
                assert_eq!(typed.conflict(), None);
                let before = first_series(&edited).data_labels().unwrap();
                let encoded = serde_json::to_value(before).unwrap();
                let retained = encoded["parts"]["lexical_flags"].as_array().unwrap();
                assert_eq!(retained.len(), 6);
                assert!(
                    retained
                        .iter()
                        .all(|entry| entry[0] != serde_json::json!(target))
                );
                assert!(
                    retained
                        .iter()
                        .all(|entry| entry[1]["attribute"] == serde_json::json!(lexical))
                );
            }
        }
    }
}
