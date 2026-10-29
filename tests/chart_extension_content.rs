//! Element-only known payloads and lossless opaque reference promotion.
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
            ChartGroupRef,
            ChartReferenceEdit,
            ExtensionList,
            SeriesFamily,
        },
        spreadsheet::MarkerType,
    },
};

fn chart(book: &Workbook) -> &Chart {
    &book.sheet(0).unwrap().chart_collection()[0]
}

fn chart_mut(book: &mut Workbook) -> &mut Chart {
    &mut book.sheet_mut(0).unwrap().chart_collection_mut()[0]
}

fn save(book: &Workbook) -> Vec<u8> {
    let mut bytes = Vec::new();
    umya_spreadsheet::writer::xlsx::write_writer(book, &mut Cursor::new(&mut bytes)).unwrap();
    bytes
}

fn load(bytes: &[u8], eager: bool) -> Workbook {
    let mut book = umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(bytes), eager).unwrap();
    book.try_read_sheet(0).unwrap();
    book
}

fn part(bytes: &[u8]) -> String {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut xml = String::new();
    zip.by_name("xl/charts/chart1.xml")
        .unwrap()
        .read_to_string(&mut xml)
        .unwrap();
    xml
}

fn package(xml: &str) -> Vec<u8> {
    let mut book = umya_spreadsheet::new_file();
    let mut first = MarkerType::default();
    first.set_coordinate("E1");
    let mut last = MarkerType::default();
    last.set_coordinate("M18");
    let mut value = Chart::default();
    value.new_chart(&ChartType::LineChart, first, last, vec!["Sheet1!$B$1:$B$3"]);
    book.sheet_mut(0).unwrap().add_chart(value);
    let bytes = save(&book);
    let mut input = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for i in 0..input.len() {
        let mut file = input.by_index(i).unwrap();
        let name = file.name().unwrap().into_owned();
        let mut data = Vec::new();
        file.read_to_end(&mut data).unwrap();
        if name == "xl/charts/chart1.xml" {
            data = xml.as_bytes().to_vec();
        }
        output
            .start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        output.write_all(&data).unwrap();
    }
    output.finish().unwrap().into_inner()
}

fn variants(book: &Workbook) -> [Chart; 3] {
    let original = chart(book).clone();
    let json = serde_json::from_value(serde_json::to_value(&original).unwrap()).unwrap();
    let binary = bincode::deserialize(&bincode::serialize(&original).unwrap()).unwrap();
    [original, json, binary]
}

struct KnownCase {
    xml:    &'static str,
    local:  &'static str,
    prefix: &'static str,
    owner:  &'static str,
}

const KNOWN: &[KnownCase] = &[
    KnownCase {
        xml:    include_str!("fixtures/chart_extensions/registered-numRef-fullRef.xml"),
        local:  "fullRef",
        prefix: "c15",
        owner:  "numRef",
    },
    KnownCase {
        xml:    include_str!("fixtures/chart_extensions/registered-numRef-levelRef.xml"),
        local:  "levelRef",
        prefix: "c15",
        owner:  "numRef",
    },
    KnownCase {
        xml:    include_str!("fixtures/chart_extensions/registered-numRef-formulaRef.xml"),
        local:  "formulaRef",
        prefix: "c15",
        owner:  "numRef",
    },
    KnownCase {
        xml:    include_str!("fixtures/chart_extensions/registered-ser-filteredSeriesTitle.xml"),
        local:  "filteredSeriesTitle",
        prefix: "c15",
        owner:  "ser",
    },
    KnownCase {
        xml:    include_str!("fixtures/chart_extensions/registered-ser-filteredCategoryTitle.xml"),
        local:  "filteredCategoryTitle",
        prefix: "c15",
        owner:  "ser",
    },
    KnownCase {
        xml:    include_str!(
            "fixtures/chart_extensions/registered-ser-categoryFilterExceptions.xml"
        ),
        local:  "categoryFilterExceptions",
        prefix: "c15",
        owner:  "ser",
    },
    KnownCase {
        xml:    include_str!(
            "fixtures/chart_extensions/registered-ser-categoryFilterExceptions.xml"
        ),
        local:  "categoryFilterException",
        prefix: "c15",
        owner:  "ser",
    },
    KnownCase {
        xml:    include_str!("fixtures/chart_extensions/registered-ser-invertSolidFillFmt.xml"),
        local:  "invertSolidFillFmt",
        prefix: "c14",
        owner:  "ser",
    },
    KnownCase {
        xml:    include_str!(
            "fixtures/chart_extensions/registered-lineChart-filteredLineSeries.xml"
        ),
        local:  "filteredLineSeries",
        prefix: "c15",
        owner:  "lineChart",
    },
    KnownCase {
        xml:    include_str!("fixtures/chart_extensions/registered-barChart-filteredBarSeries.xml"),
        local:  "filteredBarSeries",
        prefix: "c15",
        owner:  "barChart",
    },
    KnownCase {
        xml:    include_str!(
            "fixtures/chart_extensions/registered-areaChart-filteredAreaSeries.xml"
        ),
        local:  "filteredAreaSeries",
        prefix: "c15",
        owner:  "areaChart",
    },
    KnownCase {
        xml:    include_str!(
            "fixtures/chart_extensions/registered-radarChart-filteredRadarSeries.xml"
        ),
        local:  "filteredRadarSeries",
        prefix: "c15",
        owner:  "radarChart",
    },
    KnownCase {
        xml:    include_str!(
            "fixtures/chart_extensions/registered-bubbleChart-filteredBubbleSeries.xml"
        ),
        local:  "filteredBubbleSeries",
        prefix: "c15",
        owner:  "bubbleChart",
    },
    KnownCase {
        xml:    include_str!(
            "fixtures/chart_extensions/registered-scatterChart-filteredScatterSeries.xml"
        ),
        local:  "filteredScatterSeries",
        prefix: "c15",
        owner:  "scatterChart",
    },
];

fn list<'a>(book: &'a Workbook, owner: &str) -> &'a ExtensionList {
    let group = chart(book)
        .chart_space()
        .chart()
        .plot_area()
        .chart_groups()
        .next()
        .unwrap();
    let series = &group.series().area_chart_series()[0];
    match owner {
        "ser" => series.extensions().unwrap(),
        "numRef" => series
            .values()
            .unwrap()
            .number_reference()
            .extensions()
            .unwrap(),
        _ => match group {
            ChartGroupRef::LineChart(v) => v.extensions().unwrap(),
            ChartGroupRef::BarChart(v) => v.extensions().unwrap(),
            ChartGroupRef::AreaChart(v) => v.extensions().unwrap(),
            ChartGroupRef::PieChart(v) => v.extensions().unwrap(),
            ChartGroupRef::RadarChart(v) => v.extensions().unwrap(),
            ChartGroupRef::BubbleChart(v) => v.extensions().unwrap(),
            ChartGroupRef::ScatterChart(v) => v.extensions().unwrap(),
            _ => panic!("unexpected fixture group"),
        },
    }
}

fn insert_content(case: &KnownCase, content: &str) -> String {
    let open = format!("<{}:{}>", case.prefix, case.local);
    let close = format!("</{}:{}>", case.prefix, case.local);
    case.xml
        .replacen(&open, &format!("{open}{content}"), 1)
        .replacen(&close, &format!("{content}{close}"), 1)
}

#[test]
fn known_element_only_payloads_reject_foreign_children_text_and_cdata_without_legalizing() {
    for case in KNOWN {
        for content in [
            "<x:note xmlns:x=\"urn:future\">keep</x:note>",
            "significant",
            "<![CDATA[significant]]>",
        ] {
            for eager in [false, true] {
                let mut book = load(&package(&insert_content(case, content)), eager);
                for value in variants(&book) {
                    *chart_mut(&mut book) = value;
                    let list = list(&book, case.owner);
                    assert!(
                        list.conflict().is_some(),
                        "{} / {content} / eager={eager}",
                        case.local
                    );
                    assert_eq!(
                        list.export_diagnostics_for_owner(
                            Some(&book),
                            case.owner,
                            Some(SeriesFamily::Bar)
                        )
                        .len(),
                        1,
                        "{}",
                        case.local
                    );
                    let before = serde_json::to_value(chart(&book)).unwrap();
                    let output = save(&book);
                    assert!(
                        !part(&output).contains(&format!("<{}:{}>", case.prefix, case.local)),
                        "{} was legalized",
                        case.local
                    );
                    assert_eq!(serde_json::to_value(chart(&book)).unwrap(), before);
                    assert_eq!(
                        load(&output, true)
                            .sheet(0)
                            .unwrap()
                            .chart_collection()
                            .len(),
                        1
                    );
                }
            }
        }
    }
}

#[test]
fn known_element_only_payloads_accept_whitespace_comments_and_processing_instructions() {
    for case in KNOWN {
        let mut book = load(
            &package(&insert_content(case, " \n\t<!--keep--><?payload keep?>")),
            true,
        );
        for value in variants(&book) {
            *chart_mut(&mut book) = value;
            assert!(
                list(&book, case.owner).conflict().is_none(),
                "{}",
                case.local
            );
            assert!(
                part(&save(&book)).contains(&format!("<{}:{}>", case.prefix, case.local)),
                "{}",
                case.local
            );
        }
    }
}

#[test]
fn duplicate_singleton_payload_children_are_not_legalized() {
    for case in KNOWN
        .iter()
        .filter(|case| !case.local.starts_with("categoryFilterException"))
    {
        let open = format!("<{}:{}>", case.prefix, case.local);
        let close = format!("</{}:{}>", case.prefix, case.local);
        let start = case.xml.find(&open).unwrap() + open.len();
        let end = case.xml.find(&close).unwrap();
        let xml = case
            .xml
            .replacen(&close, &format!("{}{close}", &case.xml[start..end]), 1);
        let mut book = load(&package(&xml), true);
        for value in variants(&book) {
            *chart_mut(&mut book) = value;
            assert!(
                list(&book, case.owner).conflict().is_some(),
                "{}",
                case.local
            );
            assert_eq!(
                list(&book, case.owner)
                    .export_diagnostics_for_owner(Some(&book), case.owner, Some(SeriesFamily::Bar))
                    .len(),
                1
            );
            assert!(!part(&save(&book)).contains(&open), "{}", case.local);
        }
    }
}

#[test]
fn duplicate_category_exception_children_retain_invalidity() {
    let case = &KNOWN[6];
    let xml = case.xml.replacen(
        "</c15:categoryFilterException>",
        "<c15:sqref>Sheet1!$A$2</c15:sqref></c15:categoryFilterException>",
        1,
    );
    let mut book = load(&package(&xml), true);
    for value in variants(&book) {
        *chart_mut(&mut book) = value;
        assert!(list(&book, "ser").conflict().is_some());
        assert_eq!(
            list(&book, "ser")
                .export_diagnostics_for_owner(Some(&book), "ser", Some(SeriesFamily::Bar))
                .len(),
            1
        );
        assert!(!part(&save(&book)).contains("categoryFilterExceptions"));
    }
}

fn opaque_chart(candidate: &str) -> String {
    let template = include_str!(
        "fixtures/chart_extensions/unknown-namespaces-mixed-content-nested-reference.xml"
    );
    let start = template.find("<c15:fullRef>").unwrap();
    let end = template.find("</c15:fullRef>").unwrap() + "</c15:fullRef>".len();
    format!("{}{}{}", &template[..start], candidate, &template[end..])
}

#[test]
fn opaque_reference_candidates_preserve_unrepresentable_content_through_codecs_reload_and_edit() {
    for local in ["fullRef", "levelRef", "formulaRef"] {
        for (attributes, body) in [
            (
                "",
                "<c15:sqref>Sheet1!$A$2</c15:sqref><x:note>keep</x:note>tail",
            ),
            ("", "leading<c15:sqref>Sheet1!$A$2</c15:sqref>trailing"),
            (
                "",
                "<!--keep--><c15:sqref>Sheet1!$A$2</c15:sqref><?payload keep?><![CDATA[raw < & \
                 text]]>",
            ),
            ("", "<c15:sqref x:flag=\"keep\">Sheet1!$A$2</c15:sqref>"),
            (
                "",
                "<c15:sqref xmlns:z=\"urn:future:sqref\">Sheet1!$A$2</c15:sqref>",
            ),
            (
                "",
                "<c15:sqref>Sheet1!$A$2<!--keep--><?payload keep?><![CDATA[tail]]></c15:sqref>",
            ),
            (
                "",
                "<c15:sqref>Sheet1!$A$2</c15:sqref><c15:sqref>Sheet1!$A$3</c15:sqref>",
            ),
        ] {
            let candidate = format!("<c15:{local}{attributes}>{body}</c15:{local}>");
            for eager in [false, true] {
                let mut book = load(&package(&opaque_chart(&candidate)), eager);
                for value in variants(&book) {
                    *chart_mut(&mut book) = value;
                    let report = chart_mut(&mut book).adjust_references(
                        "Sheet1",
                        &ChartReferenceEdit::InsertRows {
                            sheet: "Sheet1".into(),
                            at:    1,
                            count: 1,
                        },
                    );
                    assert_eq!(report.changed, 0, "{candidate}");
                    assert!(report.preserved_opaque_extensions > 0);
                    assert!(part(&save(&book)).contains(&candidate), "lost {candidate}");
                    let mut reloaded = load(&save(&book), eager);
                    reloaded.insert_new_row("Sheet1", 1, 1);
                    assert!(
                        part(&save(&reloaded)).contains(&candidate),
                        "reload/edit lost {candidate}"
                    );
                }
            }
        }
    }
}

#[test]
fn exact_opaque_terminal_reference_promotes_and_adjusts_once() {
    for local in ["fullRef", "levelRef", "formulaRef"] {
        let candidate = format!("<c15:{local}><c15:sqref>Sheet1!$A$2</c15:sqref></c15:{local}>");
        let expected = format!("<c15:{local}><c15:sqref>Sheet1!$A$3</c15:sqref></c15:{local}>");
        let mut book = load(&package(&opaque_chart(&candidate)), true);
        for value in variants(&book) {
            *chart_mut(&mut book) = value;
            let report = chart_mut(&mut book).adjust_references(
                "Sheet1",
                &ChartReferenceEdit::InsertRows {
                    sheet: "Sheet1".into(),
                    at:    1,
                    count: 1,
                },
            );
            assert_eq!(report.changed, 1);
            let xml = part(&save(&book));
            assert!(xml.contains(&expected), "{xml}");
            assert!(!xml.contains("Sheet1!$A$4"));
            assert!(part(&save(&load(&save(&book), true))).contains(&expected));
        }
    }
}

#[test]
fn inherited_alias_and_default_namespace_terminal_references_adjust_once() {
    const NAMESPACE: &str = "http://schemas.microsoft.com/office/drawing/2012/chart";
    for local in ["fullRef", "levelRef", "formulaRef"] {
        for prefix in ["r:", ""] {
            let candidate =
                format!("<c15:{local}><{prefix}sqref>Sheet1!$A$2</{prefix}sqref></c15:{local}>");
            let input = opaque_chart(&candidate).replacen(
                "<x:future xmlns:x=\"urn:future:outer\"",
                &format!(
                    "<x:future xmlns:x=\"urn:future:outer\" xmlns:r=\"{NAMESPACE}\" \
                     xmlns=\"{NAMESPACE}\""
                ),
                1,
            );
            let expected = format!("<c15:{local}><c15:sqref>Sheet1!$A$3</c15:sqref></c15:{local}>");
            let edited_again =
                format!("<c15:{local}><c15:sqref>Sheet1!$A$4</c15:sqref></c15:{local}>");
            for eager in [false, true] {
                let mut book = load(&package(&input), eager);
                for value in variants(&book) {
                    *chart_mut(&mut book) = value;
                    let report = chart_mut(&mut book).adjust_references(
                        "Sheet1",
                        &ChartReferenceEdit::InsertRows {
                            sheet: "Sheet1".into(),
                            at:    1,
                            count: 1,
                        },
                    );
                    assert_eq!(report.changed, 1, "{candidate} / eager={eager}");
                    assert!(part(&save(&book)).contains(&expected), "{candidate}");
                    let mut reloaded = load(&save(&book), eager);
                    assert!(part(&save(&reloaded)).contains(&expected));
                    reloaded.insert_new_row("Sheet1", 1, 1);
                    let xml = part(&save(&reloaded));
                    assert!(xml.contains(&edited_again), "{candidate}: {xml}");
                    assert!(!xml.contains("Sheet1!$A$5"));
                }
            }
        }
    }
}
