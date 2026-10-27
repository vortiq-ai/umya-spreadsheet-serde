//! Native trendlines retain optional attribute presence and share workbook
//! cache authority.
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
            AreaChartSeries,
            DataPoint,
            Index,
            MAX_SOURCE_CACHE_POINTS,
            MAX_SOURCE_CACHE_TEXT_BYTES,
            NumberingFormat,
            SeriesFamily,
            SeriesPartConflictReason,
            SeriesPartKind,
            Trendline,
            TrendlineType,
            XmlBoolean,
            XmlDouble,
            XmlUnsignedInt,
        },
        spreadsheet::MarkerType,
    },
};
fn marker(coordinate: &str) -> MarkerType {
    let mut marker = MarkerType::default();
    marker.set_coordinate(coordinate);
    marker
}

fn base_book(chart_type: &ChartType, series: usize) -> Workbook {
    let mut book = umya_spreadsheet::new_file();
    let sheet = book.sheet_mut(0).unwrap();
    for row in 1..=3 {
        sheet
            .cell_mut((1, row))
            .set_value_string(format!("Row {row}"));
        sheet.cell_mut((2, row)).set_value_number(row * 10);
        sheet.cell_mut((3, row)).set_value_number(row * 100);
    }
    let sources = ["Sheet1!$B$1:$B$3", "Sheet1!$C$1:$C$3"];
    // A scatter series takes two references and a bubble series three; a
    // doughnut series takes its categories, then its values.
    let sources = match chart_type {
        ChartType::DoughnutChart => vec!["Sheet1!$A$1:$A$3", sources[0]],
        ChartType::ScatterChart => vec![sources[0], sources[1]],
        ChartType::BubbleChart => vec![sources[0], sources[1], sources[1]],
        _ => sources[..series].to_vec(),
    };
    let mut chart = Chart::default();
    chart.new_chart(chart_type, marker("E1"), marker("M18"), sources);
    sheet.add_chart(chart);
    book
}

fn write(book: &Workbook) -> Vec<u8> {
    let mut bytes = Vec::new();
    umya_spreadsheet::writer::xlsx::write_writer(book, &mut Cursor::new(&mut bytes)).unwrap();
    bytes
}

fn read(bytes: &[u8]) -> Workbook {
    umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap()
}

fn chart_part(bytes: &[u8]) -> String {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut xml = String::new();
    archive
        .by_name("xl/charts/chart1.xml")
        .unwrap()
        .read_to_string(&mut xml)
        .unwrap();
    xml
}

fn with_chart_part(bytes: &[u8], edit: impl Fn(&str) -> String) -> Vec<u8> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut out = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for index in 0..archive.len() {
        let mut file = archive.by_index(index).unwrap();
        let name = file.name().to_string();
        let mut data = Vec::new();
        file.read_to_end(&mut data).unwrap();
        if name == "xl/charts/chart1.xml" {
            data = edit(&String::from_utf8(data).unwrap()).into_bytes();
        }
        out.start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        out.write_all(&data).unwrap();
    }
    out.finish().unwrap().into_inner()
}

fn replace_first_series(xml: &str, series: &str) -> String {
    let start = xml.find("<c:ser>").expect("a series in the chart part");
    let end = start + xml[start..].find("</c:ser>").expect("series end") + "</c:ser>".len();
    format!("{}<c:ser>{series}</c:ser>{}", &xml[..start], &xml[end..])
}

fn chart_of(book: &Workbook) -> &Chart {
    &book.sheet(0).unwrap().chart_collection()[0]
}

fn first_series(book: &Workbook) -> &AreaChartSeries {
    &chart_of(book)
        .chart_space()
        .chart()
        .plot_area()
        .chart_groups()
        .next()
        .expect("one chart group")
        .series()
        .area_chart_series()[0]
}
fn series_xml(book: &Workbook) -> String {
    let xml = chart_part(&write(book));
    let start = xml.find("<c:ser>").unwrap();
    let end = start + xml[start..].find("</c:ser>").unwrap() + 8;
    xml[start..end].to_string()
}
fn input(kind: &ChartType, children: &str) -> Workbook {
    let bytes = write(&base_book(kind, 1));
    read(&with_chart_part(&bytes, |xml| {
        replace_first_series(
            xml,
            &format!("<c:idx val=\"1\"/><c:order val=\"1\"/>{children}"),
        )
    }))
}
fn persisted(book: &Workbook) -> [Workbook; 2] {
    let chart = chart_of(book);
    let json: Chart = serde_json::from_str(&serde_json::to_string(chart).unwrap()).unwrap();
    let binary: Chart = bincode::deserialize(&bincode::serialize(chart).unwrap()).unwrap();
    let bytes = write(book);
    [json, binary].map(|chart| {
        let mut book = read(&bytes);
        book.sheet_mut(0).unwrap().chart_collection_mut()[0] = chart;
        book
    })
}

fn cases() -> serde_json::Value {
    serde_json::from_str(include_str!("fixtures/trendline_cases.json")).unwrap()
}
fn trends(book: &Workbook) -> &[Trendline] {
    first_series(book).trendlines()
}
fn conflicts(
    book: &Workbook,
) -> Vec<(
    usize,
    usize,
    SeriesPartKind,
    usize,
    SeriesPartConflictReason,
)> {
    chart_of(book)
        .chart_space()
        .chart()
        .plot_area()
        .series_part_conflicts()
        .into_iter()
        .map(|v| {
            (
                v.group_index,
                v.series_index,
                v.part,
                v.occurrence,
                v.reason,
            )
        })
        .collect()
}
fn series_mut(book: &mut Workbook) -> &mut AreaChartSeries {
    &mut book.sheet_mut(0).unwrap().chart_collection_mut()[0]
        .area_chart_series_list_mut()
        .area_chart_series_mut()[0]
}
fn chart_mut(book: &mut Workbook) -> &mut Chart {
    &mut book.sheet_mut(0).unwrap().chart_collection_mut()[0]
}
fn save(name: &str, book: &Workbook) {
    if let Some(path) = std::env::var_os("UMYA_TRENDLINE_EXPORT_DIR") {
        let path = std::path::PathBuf::from(path);
        std::fs::create_dir_all(&path).unwrap();
        for (suffix, book) in std::iter::once(("original".to_owned(), book.clone())).chain(
            persisted(book)
                .into_iter()
                .enumerate()
                .map(|(i, b)| (format!("reload-{i}"), b)),
        ) {
            std::fs::write(path.join(format!("{name}-{suffix}.xlsx")), write(&book)).unwrap();
        }
    }
}
fn input_attribute(xml: &str, tag: &str) -> Option<Option<String>> {
    let start = xml.find(&format!("<c:{tag}"))?;
    let end = start + xml[start..].find('>')? + 1;
    let mut reader = quick_xml::Reader::from_str(&xml[start..end]);
    match reader.read_event().unwrap() {
        quick_xml::events::Event::Empty(e) | quick_xml::events::Event::Start(e) => Some(
            e.attributes()
                .map(Result::unwrap)
                .find(|a| a.key.as_ref() == b"val")
                .map(|a| {
                    a.normalized_value(quick_xml::XmlVersion::Implicit1_0)
                        .unwrap()
                        .into_owned()
                }),
        ),
        _ => panic!("authored scalar"),
    }
}
#[test]
fn all_algorithms_and_optional_scalar_attributes_survive_import_persistence_export() {
    let cases = cases();
    let cases = cases["positives"].as_array().unwrap();
    assert_eq!(cases.len(), 36);
    for case in cases {
        let name = case["name"].as_str().unwrap();
        let xml = case["xml"].as_str().unwrap();
        let book = input(&ChartType::LineChart, xml);
        assert!(
            conflicts(&book).is_empty(),
            "{name}: {:?}",
            conflicts(&book)
        );
        assert_eq!(trends(&book).len(), 1, "{name}");
        let trend = &trends(&book)[0];
        let kind = input_attribute(xml, "trendlineType").unwrap();
        assert_eq!(trend.kind_attribute(), kind.as_deref(), "{name}");
        let expected_kind = match kind.as_deref().unwrap_or("linear") {
            "exp" => TrendlineType::Exponential,
            "linear" => TrendlineType::Linear,
            "log" => TrendlineType::Logarithmic,
            "movingAvg" => TrendlineType::MovingAverage,
            "poly" => TrendlineType::Polynomial,
            "power" => TrendlineType::Power,
            _ => panic!("authored algorithm"),
        };
        assert_eq!(trend.kind(), Some(expected_kind));
        for (tag, scalar) in [("order", trend.order()), ("period", trend.period())] {
            let expected = input_attribute(xml, tag);
            assert_eq!(
                scalar.map(|v| v.attribute().map(str::to_owned)),
                expected,
                "{name} {tag}"
            );
        }
        for (tag, scalar) in [
            ("forward", trend.forward()),
            ("backward", trend.backward()),
            ("intercept", trend.intercept()),
        ] {
            assert_eq!(
                scalar.map(|v| v.attribute().map(str::to_owned)),
                input_attribute(xml, tag),
                "{name} {tag}"
            );
        }
        for (tag, scalar) in [
            ("dispRSqr", trend.display_r_squared()),
            ("dispEq", trend.display_equation()),
        ] {
            assert_eq!(
                scalar.map(|v| v.attribute().map(str::to_owned)),
                input_attribute(xml, tag),
                "{name} {tag}"
            );
        }
        if let Some(expected) = case["semantic_expectations"]["name"].as_str() {
            assert_eq!(trend.name(), Some(expected));
        }
        if let Some(expected) = case["semantic_expectations"]["semantic_scalar"].as_u64() {
            let got = if name.starts_with("order") {
                trend.order_value()
            } else {
                trend.period_value()
            };
            assert_eq!(got, Some(u32::try_from(expected).unwrap()));
        }
        let before = serde_json::to_value(trend).unwrap();
        let output = series_xml(&book);
        for restored in persisted(&book) {
            assert_eq!(
                serde_json::to_value(&trends(&restored)[0]).unwrap(),
                before,
                "{name}"
            );
            assert_eq!(series_xml(&restored), output, "{name}");
        }
        let reloaded = read(&write(&book));
        assert_eq!(series_xml(&reloaded), output, "{name} ordinary reread");
        save(name, &book);
    }
}
#[test]
fn malformed_required_and_present_scalar_states_remain_diagnosable_without_invalid_output() {
    let cases = cases();
    let cases = cases["negatives"].as_array().unwrap();
    assert_eq!(cases.len(), 21);
    for case in cases {
        let name = case["name"].as_str().unwrap();
        let book = input(&ChartType::LineChart, case["xml"].as_str().unwrap());
        let expected = match name {
            "missing-kind" | "missing-format-code" => {
                SeriesPartConflictReason::MissingRequiredChild
            }
            "label-choice" => SeriesPartConflictReason::ConflictingChoice,
            "duplicate-kind" => SeriesPartConflictReason::TooMany,
            _ => SeriesPartConflictReason::InvalidValue,
        };
        for restored in std::iter::once(book.clone()).chain(persisted(&book)) {
            assert_eq!(trends(&restored).len(), 1, "{name}");
            assert_eq!(
                conflicts(&restored),
                vec![(0, 0, SeriesPartKind::Trendlines, 0, expected)],
                "{name}"
            );
            assert!(!series_xml(&restored).contains("<c:trendline>"), "{name}");
        }
        save(&format!("invalid-{name}"), &book);
    }
    for bad in ["1e", "+INF", "Infinity", "\u{a0}2"] {
        let book = input(
            &ChartType::LineChart,
            &format!("<c:trendline><c:trendlineType/><c:forward val=\"{bad}\"/></c:trendline>"),
        );
        assert_eq!(
            conflicts(&book)[0].4,
            SeriesPartConflictReason::InvalidValue
        );
    }
}
#[test]
fn trendline_labels_own_every_supported_child_and_preserve_empty_vs_absent() {
    let cases = cases();
    let all = cases["positives"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["name"] == "label-all")
        .unwrap();
    let book = input(&ChartType::LineChart, all["xml"].as_str().unwrap());
    let label = trends(&book)[0].label().unwrap();
    assert!(label.layout().is_some());
    assert!(label.layout().unwrap().is_empty());
    assert_eq!(
        label
            .text()
            .unwrap()
            .string_reference()
            .unwrap()
            .formula()
            .address_str(),
        "Data!$A$1"
    );
    assert_eq!(label.numbering_format().unwrap().format_code(), "0.00");
    assert_eq!(
        label.numbering_format().unwrap().source_linked_value(),
        Some(false)
    );
    assert!(label.shape_properties().is_some());
    assert_eq!(
        label.text_properties().unwrap().paragraph()[0].run()[0].text(),
        "Label style"
    );
    let xml = series_xml(&book);
    let positions = ["<c:layout", "<c:tx>", "<c:numFmt", "<c:spPr>", "<c:txPr>"]
        .map(|tag| xml.find(tag).unwrap());
    assert!(positions.windows(2).all(|p| p[0] < p[1]));
    assert!(
        trends(&input(
            &ChartType::LineChart,
            "<c:trendline><c:trendlineType/></c:trendline>"
        ))[0]
            .label()
            .is_none()
    );
    assert!(
        trends(&input(
            &ChartType::LineChart,
            "<c:trendline><c:trendlineType/><c:trendlineLbl/></c:trendline>"
        ))[0]
            .label()
            .is_some()
    );
}
#[test]
fn typed_edits_change_only_the_owned_scalar_and_keep_other_lexical_values() {
    let mut book = input(
        &ChartType::LineChart,
        "<c:trendline><c:trendlineType/><c:order val=\" 02 \"/><c:forward \
         val=\"1.2500E+1\"/><c:dispEq val=\"false\"/></c:trendline>",
    );
    let trend = &mut series_mut(&mut book).trendlines_mut()[0];
    trend.set_kind(Some(TrendlineType::Polynomial));
    trend.order_mut().unwrap().set_val(4);
    let mut intercept = XmlDouble::default();
    intercept.set_value(-2.5);
    trend.set_intercept(intercept);
    let expected = "<c:trendline><c:trendlineType val=\"poly\"/><c:order val=\"4\"/><c:forward \
                    val=\"1.2500E+1\"/><c:intercept val=\"-2.5\"/><c:dispEq \
                    val=\"false\"/></c:trendline>";
    assert!(series_xml(&book).contains(expected));
    for restored in persisted(&book) {
        assert!(series_xml(&restored).contains(expected));
    }
    save("typed-edits", &book);
    let mut trend = Trendline::default();
    let mut order = XmlUnsignedInt::default();
    order.set_val(6);
    trend.set_order(order);
    let mut flag = XmlBoolean::default();
    flag.set_value(true);
    trend.set_display_equation(flag);
    assert_eq!(trend.order_value(), Some(6));
    assert_eq!(trend.kind(), Some(TrendlineType::Linear));
}
#[test]
fn family_output_diagnostics_and_explicit_cleanup_preserve_order_and_common_points() {
    let families = [
        ChartType::BarChart,
        ChartType::Bar3DChart,
        ChartType::LineChart,
        ChartType::Line3DChart,
        ChartType::AreaChart,
        ChartType::Area3DChart,
        ChartType::ScatterChart,
        ChartType::BubbleChart,
        ChartType::PieChart,
        ChartType::Pie3DChart,
        ChartType::DoughnutChart,
        ChartType::OfPieChart,
        ChartType::RadarChart,
    ];
    for kind in families {
        let mut book = base_book(&kind, 1);
        let series = series_mut(&mut book);
        for name in ["first", "first", "last"] {
            let mut trend = Trendline::default();
            trend.set_name(Some(name.into()));
            series.trendlines_mut().push(trend);
        }
        let mut point = DataPoint::default();
        point.index_mut().set_val(9);
        series.data_points_mut().push(point);
        let legal = !matches!(
            SeriesFamily::from(&kind),
            SeriesFamily::Pie | SeriesFamily::Radar
        );
        assert_eq!(
            conflicts(&book),
            if legal {
                vec![]
            } else {
                (0..3)
                    .map(|i| {
                        (
                            0,
                            0,
                            SeriesPartKind::Trendlines,
                            i,
                            SeriesPartConflictReason::NotAllowed,
                        )
                    })
                    .collect()
            }
        );
        let xml = series_xml(&book);
        assert_eq!(
            xml.matches("<c:trendline>").count(),
            if legal { 3 } else { 0 }
        );
        if legal {
            assert!(xml.find("</c:dPt>").unwrap() < xml.find("<c:trendline>").unwrap());
            let last = xml.rfind("</c:trendline>").unwrap();
            let source = xml
                .find(
                    if matches!(kind, ChartType::ScatterChart | ChartType::BubbleChart) {
                        "<c:xVal>"
                    } else {
                        "<c:val>"
                    },
                )
                .unwrap();
            assert!(last < source);
        }
        save(&format!("family-{kind:?}"), &book);
        let removed = chart_mut(&mut book)
            .area_chart_series_list_mut()
            .remove_parts_not_allowed_in(SeriesFamily::Pie);
        assert_eq!(
            removed
                .iter()
                .filter(|r| r.part == SeriesPartKind::Trendlines)
                .map(|r| (r.series_index, r.occurrence))
                .collect::<Vec<_>>(),
            vec![(0, 0), (0, 1), (0, 2)]
        );
        assert!(trends(&book).is_empty());
        assert_eq!(first_series(&book).data_points()[0].index().val(), Some(9));
    }
}
#[test]
fn invalid_and_forbidden_occurrences_report_every_original_position() {
    let good = "<c:trendline><c:trendlineType/></c:trendline>";
    let bad = "<c:trendline><c:trendlineType val=\"bad\"/></c:trendline>";
    let book = input(&ChartType::LineChart, &format!("{good}{bad}{good}{bad}"));
    assert_eq!(
        conflicts(&book),
        vec![
            (
                0,
                0,
                SeriesPartKind::Trendlines,
                1,
                SeriesPartConflictReason::InvalidValue
            ),
            (
                0,
                0,
                SeriesPartKind::Trendlines,
                3,
                SeriesPartConflictReason::InvalidValue
            )
        ]
    );
    for restored in persisted(&book) {
        assert_eq!(trends(&restored).len(), 4);
        assert_eq!(series_xml(&restored).matches("<c:trendline>").count(), 2);
    }
}
fn label_ref(formula: &str, value: &str) -> String {
    format!(
        "<c:trendline><c:trendlineType/><c:trendlineLbl><c:tx><c:strRef><c:f>{formula}</c:f><c:\
         strCache><c:ptCount val=\"1\"/><c:pt \
         idx=\"0\"><c:v>{value}</c:v></c:pt></c:strCache></c:strRef></c:tx></c:trendlineLbl></c:\
         trendline>"
    )
}
fn reference(trend: &Trendline) -> &umya_spreadsheet::drawing::charts::StringReference {
    trend
        .label()
        .unwrap()
        .text()
        .unwrap()
        .string_reference()
        .unwrap()
}
fn has_source(trend: &Trendline) -> bool {
    reference(trend).string_cache().source().is_some()
}
#[test]
fn missing_cached_values_remain_absent_through_import_persistence_and_export() {
    // Malformed source cache: retain None as the existing cache contract
    // requires; the independent schema gate must reject its unchanged output.
    let fixtures: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/trendline_sparse_cache_cases.json")).unwrap();
    let xml = fixtures["missing_value"]["xml"].as_str().unwrap();
    let mut book = input(&ChartType::LineChart, xml);
    let source = reference(&trends(&book)[0])
        .string_cache()
        .source()
        .unwrap();
    assert_eq!(source.point_count(), Some(9));
    assert_eq!(source.indices().collect::<Vec<_>>(), vec![2, 2, 7]);
    assert_eq!(
        source
            .exceptions()
            .iter()
            .map(|e| (e.index(), e.value()))
            .collect::<Vec<_>>(),
        vec![(2, Some("First")), (2, None), (7, Some(""))]
    );
    assert_eq!(book.chart_source_cache_report().kept_caches, 1);
    for restored in persisted(&book) {
        assert!(has_source(&trends(&restored)[0]));
        assert_eq!(series_xml(&restored), series_xml(&book));
        let source = reference(&trends(&restored)[0])
            .string_cache()
            .source()
            .unwrap();
        assert_eq!(
            source
                .exceptions()
                .iter()
                .map(|e| (e.index(), e.value()))
                .collect::<Vec<_>>(),
            vec![(2, Some("First")), (2, None), (7, Some(""))]
        );
    }
    save_negative_cache(&book);
    series_mut(&mut book).trendlines_mut()[0]
        .label_mut()
        .unwrap()
        .text_mut()
        .unwrap()
        .string_reference_mut()
        .unwrap()
        .formula_mut()
        .set_address_str("Missing!$B$1");
    assert!(has_source(&trends(&book)[0]));
    assert!(!series_xml(&book).contains("First"));
    chart_mut(&mut book).strip_source_caches();
    assert!(!has_source(&trends(&book)[0]));
}
#[test]
fn label_cache_adoption_uses_unique_series_and_formula_roles_not_trendline_positions() {
    let book = input(
        &ChartType::LineChart,
        &format!(
            "{}{}",
            label_ref("Missing!$A$1", "Alpha"),
            label_ref("Missing!$B$1", "Beta")
        ),
    );
    let previous = chart_of(&book).clone();
    let mut client = previous.clone();
    client.strip_source_caches();
    client.area_chart_series_list_mut().area_chart_series_mut()[0]
        .trendlines_mut()
        .reverse();
    client.adopt_source_caches(&previous);
    let values = client.area_chart_series_list_mut().area_chart_series()[0]
        .trendlines()
        .iter()
        .map(|t| {
            reference(t).string_cache().source().unwrap().exceptions()[0]
                .value()
                .unwrap()
                .to_owned()
        })
        .collect::<Vec<_>>();
    assert_eq!(values, vec!["Beta", "Alpha"]);
    let mut duplicate = previous.clone();
    let series = &mut duplicate
        .area_chart_series_list_mut()
        .area_chart_series_mut()[0];
    let first = series.trendlines()[0].clone();
    series.trendlines_mut().push(first);
    client = duplicate.clone();
    client.strip_source_caches();
    client.adopt_source_caches(&duplicate);
    let ts = client.area_chart_series_list_mut().area_chart_series()[0].trendlines();
    assert!(!has_source(&ts[0]));
    assert!(has_source(&ts[1]));
    assert!(!has_source(&ts[2]));
    client = duplicate.clone();
    client.strip_source_caches();
    client.adopt_source_caches(&previous);
    let ts = client.area_chart_series_list_mut().area_chart_series()[0].trendlines();
    assert!(!has_source(&ts[0]));
    assert!(has_source(&ts[1]));
    assert!(!has_source(&ts[2]));
    client = previous.clone();
    client.area_chart_series_list_mut().area_chart_series_mut()[0].set_index(Index::default());
    client.adopt_source_caches(&previous);
    assert!(
        client.area_chart_series_list_mut().area_chart_series()[0]
            .trendlines()
            .iter()
            .all(|t| !has_source(t))
    );
    client = previous.clone();
    let t = &mut client.area_chart_series_list_mut().area_chart_series_mut()[0].trendlines_mut()[0];
    t.label_mut()
        .unwrap()
        .text_mut()
        .unwrap()
        .string_reference_mut()
        .unwrap()
        .formula_mut()
        .set_address_str("Missing!$C$1");
    client.adopt_source_caches(&previous);
    assert!(!has_source(
        &client.area_chart_series_list_mut().area_chart_series()[0].trendlines()[0]
    ));
}
#[test]
fn workbook_cache_caps_keep_existing_series_roles_before_label_roles() {
    for text_cap in [false, true] {
        let points = if text_cap { 1 } else { MAX_SOURCE_CACHE_POINTS };
        let value = if text_cap {
            "x".repeat(MAX_SOURCE_CACHE_TEXT_BYTES)
        } else {
            "x".into()
        };
        let mut cache = format!(
            "<c:cat><c:strRef><c:f>Missing!$A$1</c:f><c:strCache><c:ptCount val=\"{points}\"/>"
        );
        for index in 0..points {
            cache.push_str(&format!("<c:pt idx=\"{index}\"><c:v>{value}</c:v></c:pt>"));
        }
        cache.push_str("</c:strCache></c:strRef></c:cat>");
        let book = input(
            &ChartType::LineChart,
            &format!("{}{cache}", label_ref("Missing!$B$1", "Later")),
        );
        let report = book.chart_source_cache_report();
        assert_eq!((report.kept_caches, report.over_cap_caches), (1, 1));
        assert_eq!(report.kept_points, points);
        assert_eq!(
            report.kept_text_bytes,
            if text_cap {
                MAX_SOURCE_CACHE_TEXT_BYTES
            } else {
                MAX_SOURCE_CACHE_POINTS
            }
        );
        assert!(
            first_series(&book)
                .category_axis_data()
                .unwrap()
                .string_reference()
                .unwrap()
                .string_cache()
                .source()
                .is_some()
        );
        assert!(!has_source(&trends(&book)[0]));
    }
    let book = input(
        &ChartType::LineChart,
        &format!(
            "{}{}",
            label_ref("Missing!$A$1", "One"),
            label_ref("Missing!$A$1", "Two")
        ),
    );
    assert_eq!(
        (
            book.chart_source_cache_report().kept_caches,
            book.chart_source_cache_report().kept_points
        ),
        (2, 2)
    );
}
#[test]
fn label_cache_lazy_capture_paths_charge_once_and_match_eager_admission() {
    let bytes = with_chart_part(&write(&base_book(&ChartType::LineChart, 1)), |xml| {
        replace_first_series(
            xml,
            &format!(
                "<c:idx val=\"1\"/><c:order val=\"1\"/>{}",
                label_ref("Sheet1!$A$1", "Imported")
            ),
        )
    });
    let eager = read(&bytes);
    let expected =
        serde_json::to_value(reference(&trends(&eager)[0]).string_cache().source()).unwrap();
    for collection in [false, true] {
        let mut lazy =
            umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(&bytes), false).unwrap();
        if collection {
            lazy.read_sheet_collection();
        } else {
            lazy.read_sheet(0);
        }
        assert_eq!(
            serde_json::to_value(reference(&trends(&lazy)[0]).string_cache().source()).unwrap(),
            expected
        );
        let report = *lazy.chart_source_cache_report();
        lazy.read_sheet(0);
        assert_eq!(*lazy.chart_source_cache_report(), report);
        assert_eq!(report.kept_caches, 1);
        assert_eq!(series_xml(&lazy), series_xml(&eager));
    }
    let path =
        std::env::temp_dir().join(format!("umya-trendline-lazy-{}.xlsx", std::process::id()));
    std::fs::write(&path, &bytes).unwrap();
    let mut lazy = umya_spreadsheet::reader::xlsx::lazy_read(&path).unwrap();
    lazy.sheet_mut(0).unwrap();
    let report = *lazy.chart_source_cache_report();
    lazy.sheet_mut(0).unwrap();
    assert_eq!(*lazy.chart_source_cache_report(), report);
    assert_eq!(report.kept_caches, 1);
    assert_eq!(series_xml(&lazy), series_xml(&eager));
    std::fs::remove_file(path).unwrap();
}
#[test]
fn number_formats_keep_omitted_word_padded_and_invalid_linkage_at_shared_owners() {
    for (lexical, value) in [
        (None, None),
        (Some("true"), Some(true)),
        (Some(" false "), Some(false)),
        (Some("0"), Some(false)),
        (Some("yes"), None),
    ] {
        let attribute = lexical
            .map(|s| format!(" sourceLinked=\"{s}\""))
            .unwrap_or_default();
        let bytes = write(&base_book(&ChartType::LineChart, 1));
        let bytes = with_chart_part(&bytes, |xml| {
            let start = xml.find("<c:numFmt").unwrap();
            let end = start + xml[start..].find("/>").unwrap() + 2;
            format!(
                "{}<c:numFmt formatCode=\"0.000\"{attribute}/>{}",
                &xml[..start],
                &xml[end..]
            )
        });
        let book = read(&bytes);
        let name = match lexical {
            None => "absent",
            Some("true") => "word",
            Some(" false ") => "padded",
            Some("0") => "numeric",
            _ => "invalid",
        };
        save(&format!("axis-number-format-{name}"), &book);
        let format = chart_of(&book)
            .chart_space()
            .chart()
            .plot_area()
            .value_axis()[0]
            .numbering_format();
        assert_eq!(format.source_linked_value(), value);
        assert_eq!(format.is_valid(), lexical != Some("yes"));
        for restored in std::iter::once(book.clone()).chain(persisted(&book)) {
            let xml = chart_part(&write(&restored));
            if lexical == Some("yes") {
                assert!(!xml.contains("formatCode=\"0.000\""));
            } else {
                assert!(xml.contains(&format!("<c:numFmt formatCode=\"0.000\"{attribute}/>")));
            }
        }
    }
    let mut format = NumberingFormat::default();
    format
        .set_format_code("")
        .set_source_linked_attribute(Some(" false ".into()));
    assert!(format.is_valid());
    assert_eq!(format.source_linked_value(), Some(false));
    format.set_source_linked(true);
    assert_eq!(format.source_linked_attribute(), Some("1"));
    assert_eq!(format.source_linked_value(), Some(true));
    #[allow(deprecated)]
    {
        assert!(format.get_source_linked());
        assert_eq!(format.get_format_code(), "");
    }
}
#[test]
fn source_cache_walk_does_not_allocate_empty_series_supplements() {
    let mut chart = Chart::default();
    chart.new_chart(
        &ChartType::LineChart,
        marker("E1"),
        marker("M18"),
        vec!["Sheet1!$A$1"],
    );
    let before = bincode::serialize(&chart).unwrap();
    assert!(
        chart.area_chart_series_list_mut().area_chart_series()[0]
            .parts()
            .is_none()
    );
    chart.strip_source_caches();
    assert_eq!(bincode::serialize(&chart).unwrap(), before);
    chart.adopt_source_caches(&chart.clone());
    assert_eq!(bincode::serialize(&chart).unwrap(), before);
}
#[test]
fn appended_series_vector_and_number_format_wire_costs_are_exact() {
    let series = AreaChartSeries::default();
    assert_eq!(bincode::serialize(&series).unwrap(), vec![0; 16]);
    let mut series = series;
    series.parts_mut();
    assert_eq!(bincode::serialize(&series).unwrap().len(), 32); // Fifteen absent originals, Some, two eight-byte vectors.
    let format = NumberingFormat::default();
    assert_eq!(bincode::serialize(&format).unwrap(), vec![0, 0, 0]);
    assert_eq!(
        serde_json::to_string(&format).unwrap(),
        r#"{"format_code":{"value":null},"source_linked":{"value":null},"parts":null}"#
    );
    let mut imported = format;
    imported
        .set_format_code("0")
        .set_source_linked_attribute(Some(" false ".into()));
    let bytes = bincode::serialize(&imported).unwrap();
    assert_eq!(bytes.len(), 29);
    assert_eq!(serde_json::to_vec(&imported).unwrap().len(), 110);
    let restored: NumberingFormat = bincode::deserialize(&bytes).unwrap();
    assert_eq!(restored.source_linked_attribute(), Some(" false "));
    assert_eq!(restored.source_linked_value(), Some(false));
}

#[test]
fn expanded_aliased_label_delegates_match_empty_forms_and_contain_unknown_descendants() {
    let rich = concat!(
        "<c:rich><a:bodyPr rot=\"60000\"><a:spAutoFit/></a:bodyPr><a:lstStyle/>",
        "<a:p><a:pPr algn=\"ctr\"><a:lnSpc><a:spcPct val=\"120000\"/></a:lnSpc>",
        "<a:defRPr b=\"1\"><a:latin typeface=\"Calibri\"/></a:defRPr></a:pPr>",
        "<a:r><a:rPr i=\"1\"><a:ln w=\"12700\"><a:solidFill><a:schemeClr \
         val=\"accent1\"/></a:solidFill><a:bevel/></a:ln>",
        "<a:solidFill><a:srgbClr val=\"4472C4\"/></a:solidFill>",
        "<a:effectLst><a:glow rad=\"12700\"><a:schemeClr val=\"accent2\"/></a:glow></a:effectLst>",
        "<a:latin typeface=\"Arial\"/><a:ea typeface=\"MS Gothic\"/><a:cs typeface=\"Arial\"/>",
        "</a:rPr><a:t>  Fit &amp; label  </a:t></a:r>",
        "<a:r><a:rPr><a:gradFill><a:gsLst><a:gs pos=\"0\"><a:srgbClr val=\"FFFFFF\"/></a:gs>",
        "<a:gs pos=\"100000\"><a:srgbClr val=\"000000\"/></a:gs></a:gsLst><a:lin ang=\"0\" \
         scaled=\"1\"/></a:gradFill></a:rPr><a:t>Gradient</a:t></a:r>",
        "<a:r><a:rPr><a:noFill/></a:rPr><a:t>Unfilled</a:t></a:r>",
        "<a:r><a:rPr><a:cs typeface=\"Arial\"/></a:rPr><a:t>Complex</a:t></a:r>",
        "<a:endParaRPr lang=\"en-US\"/></a:p></c:rich>"
    );
    let layout = "<c:layout><c:manualLayout><c:layoutTarget val=\"inner\"/><c:xMode \
                  val=\"factor\"/><c:yMode val=\"factor\"/><c:wMode val=\"factor\"/><c:hMode \
                  val=\"factor\"/><c:x val=\"0.1\"/><c:y val=\"0.2\"/><c:w val=\"0.3\"/><c:h \
                  val=\"0.4\"/></c:manualLayout></c:layout>";
    let xml_input=format!("<c:trendline xmlns:d=\"http://schemas.openxmlformats.org/drawingml/2006/main\"><c:name>Own</c:name><c:trendlineType/><c:trendlineLbl>{layout}<c:tx>{rich}</c:tx><c:txPr><a:bodyPr/><a:lstStyle/><a:p/></c:txPr></c:trendlineLbl></c:trendline>");
    let original = input(&ChartType::LineChart, &xml_input);
    let expected = series_xml(&original);
    let label = trends(&original)[0].label().unwrap();
    assert_eq!(
        label.text().unwrap().rich_text().unwrap().paragraph()[0].run()[0].text(),
        "  Fit & label  "
    );
    assert_eq!(
        label
            .layout()
            .unwrap()
            .manual_layout()
            .unwrap()
            .left()
            .unwrap()
            .val(),
        0.1
    );
    // Expand every EMPTY leaf and change only the DrawingML prefix. Each
    // modeled owner must consume precisely its own subtree before its sibling.
    let mut reader = quick_xml::Reader::from_str(&xml_input);
    let mut expanded = String::new();
    loop {
        match reader.read_event().unwrap() {
            quick_xml::events::Event::Empty(e) => {
                let tag = std::str::from_utf8(e.name().into_inner())
                    .unwrap()
                    .to_owned();
                expanded.push('<');
                expanded.push_str(std::str::from_utf8(e.as_ref()).unwrap());
                expanded.push('>');
                expanded.push_str(&format!("</{tag}>"));
            }
            quick_xml::events::Event::Eof => break,
            event => {
                let mut writer = quick_xml::Writer::new(Vec::new());
                writer.write_event(event).unwrap();
                expanded.push_str(&String::from_utf8(writer.into_inner()).unwrap());
            }
        }
    }
    let expanded = expanded.replace("a:", "d:");
    for (name, xml) in [("empty", xml_input), ("expanded-alias", expanded)] {
        let book = input(&ChartType::LineChart, &xml);
        assert!(conflicts(&book).is_empty());
        assert_eq!(series_xml(&book), expected, "{name}");
        for restored in persisted(&book) {
            assert_eq!(series_xml(&restored), expected);
        }
        save(&format!("label-delegates-{name}"), &book);
    }
    let nested = rich.replace(
        "<a:r><a:rPr",
        "<z:future xmlns:z=\"urn:future\"><a:p><a:r><a:t>Wrong</a:t></a:r></a:p></z:future><a:\
         r><a:rPr",
    );
    let contained = input(
        &ChartType::LineChart,
        &format!(
            "<c:trendline><c:trendlineType/><c:trendlineLbl><c:tx>{nested}</c:tx></c:\
             trendlineLbl></c:trendline>"
        ),
    );
    assert_eq!(
        trends(&contained)[0]
            .label()
            .unwrap()
            .text()
            .unwrap()
            .rich_text()
            .unwrap()
            .paragraph()[0]
            .run()[0]
            .text(),
        "  Fit & label  "
    );
    save("label-delegates-contained", &contained);
}
#[test]
fn shared_number_format_dual_states_are_invalid_until_an_explicit_setter_resolves_them() {
    let value = serde_json::json!({"format_code":{"value":"0"},"source_linked":{"value":true},"parts":{"source_linked":{"attribute":"false"}}});
    let imported: NumberingFormat = serde_json::from_value(value.clone()).unwrap();
    assert!(!imported.is_valid());
    assert_eq!(imported.source_linked_value(), None);
    assert_eq!(imported.source_linked_attribute(), Some("false"));
    assert_eq!(serde_json::to_value(&imported).unwrap(), value);
    let mut reloaded: NumberingFormat =
        bincode::deserialize(&bincode::serialize(&imported).unwrap()).unwrap();
    assert!(!reloaded.is_valid());
    assert_eq!(reloaded.source_linked_value(), None);
    assert_eq!(reloaded.source_linked_attribute(), Some("false"));
    assert_eq!(serde_json::to_value(&reloaded).unwrap(), value);
    reloaded.set_source_linked(false);
    assert!(reloaded.is_valid());
    assert_eq!(reloaded.source_linked_attribute(), Some("0"));
    assert_eq!(reloaded.source_linked_value(), Some(false));
}

#[test]
fn manual_layout_defaults_and_coordinate_lexemes_survive_shared_owner_roundtrips() {
    use umya_spreadsheet::drawing::charts::{
        LayoutModeValues,
        LayoutTargetValues,
    };
    let children = "<c:layoutTarget/><c:xMode/><c:yMode/><c:wMode/><c:hMode/><c:x val=\" \
                    1.2500E-1 \"/><c:y val=\"-0\"/><c:w val=\"0.3000\"/><c:h val=\"0.4\"/>";
    let book = input(
        &ChartType::LineChart,
        &format!(
            "<c:trendline><c:trendlineType/><c:trendlineLbl><c:layout><c:manualLayout>{children}</\
             c:manualLayout></c:layout></c:trendlineLbl></c:trendline>"
        ),
    );
    let layout = trends(&book)[0]
        .label()
        .unwrap()
        .layout()
        .unwrap()
        .manual_layout()
        .unwrap();
    assert!(matches!(
        layout.layout_target().unwrap().val(),
        LayoutTargetValues::Outer
    ));
    assert!(matches!(
        layout.left_mode().unwrap().val(),
        LayoutModeValues::Factor
    ));
    assert!(matches!(
        layout.top_mode().unwrap().val(),
        LayoutModeValues::Factor
    ));
    assert!(matches!(
        layout.width_mode().unwrap().val(),
        LayoutModeValues::Factor
    ));
    assert!(matches!(
        layout.height_mode().unwrap().val(),
        LayoutModeValues::Factor
    ));
    assert_eq!(layout.left().unwrap().parsed_val(), Some(0.125));
    assert_eq!(layout.left().unwrap().lexical_val(), Some(" 1.2500E-1 "));
    for restored in std::iter::once(book.clone()).chain(persisted(&book)) {
        assert!(conflicts(&restored).is_empty());
        assert!(series_xml(&restored).contains(children));
    }
    save("manual-layout-defaults", &book);
    let mut edited = book.clone();
    let layout = series_mut(&mut edited).trendlines_mut()[0]
        .label_mut()
        .unwrap()
        .layout_mut()
        .unwrap()
        .manual_layout_mut()
        .unwrap();
    layout.left_mut().unwrap().set_val(0.5);
    layout
        .left_mode_mut()
        .unwrap()
        .set_val(LayoutModeValues::Edge);
    assert_eq!(layout.left().unwrap().lexical_val(), None);
    assert!(series_xml(&edited).contains("<c:xMode val=\"edge\"/>"));
    assert!(series_xml(&edited).contains("<c:x val=\"0.5\"/>"));
    assert!(series_xml(&edited).contains("<c:w val=\"0.3000\"/>"));
    save("manual-layout-edits", &edited);
}
#[test]
fn invalid_manual_layout_leaves_remain_stored_and_never_fabricate_values() {
    for child in [
        "<c:x/>",
        "<c:y val=\"abc\"/>",
        "<c:w val=\"inf\"/>",
        "<c:h val=\"1e\"/>",
        "<c:xMode val=\" factor \"/>",
        "<c:yMode val=\"bogus\"/>",
        "<c:wMode val=\"bogus\"/>",
        "<c:hMode val=\"bogus\"/>",
        "<c:layoutTarget val=\"bogus\"/>",
    ] {
        let book = input(
            &ChartType::LineChart,
            &format!(
                "<c:trendline><c:trendlineType/><c:trendlineLbl><c:layout><c:\
                 manualLayout>{child}</c:manualLayout></c:layout></c:trendlineLbl></c:trendline>"
            ),
        );
        for restored in std::iter::once(book.clone()).chain(persisted(&book)) {
            assert_eq!(
                conflicts(&restored),
                vec![(
                    0,
                    0,
                    SeriesPartKind::Trendlines,
                    0,
                    SeriesPartConflictReason::InvalidValue
                )]
            );
            assert!(!series_xml(&restored).contains("<c:trendline>"));
        }
    }
}
#[test]
fn unknown_subtrees_cannot_overwrite_trendline_label_or_series_siblings() {
    let xml = concat!(
        "<c:trendline><c:name>Own</c:name><c:trendlineType/>",
        "<z:future xmlns:z=\"urn:future\"><c:trendlineType \
         val=\"bad\"/><c:name>Wrong</c:name></z:future>",
        "<c:trendlineLbl><c:layout><z:future xmlns:z=\"urn:future\"><c:manualLayout><c:x \
         val=\"0.9\"/></c:manualLayout></z:future></c:layout>",
        "<c:tx><z:future \
         xmlns:z=\"urn:future\"><c:rich><a:p><a:r><a:t>Wrong</a:t></a:r></a:p></c:rich></z:future>",
        "<c:strRef><c:f>Missing!$A$1</c:f><z:future \
         xmlns:z=\"urn:future\"><c:f>Missing!$B$1</c:f></z:future></c:strRef></c:tx></c:\
         trendlineLbl></c:trendline>"
    );
    let book = input(
        &ChartType::LineChart,
        &format!("{xml}<c:marker><c:symbol val=\"diamond\"/></c:marker>"),
    );
    let trend = &trends(&book)[0];
    assert_eq!(trend.name(), Some("Own"));
    assert_eq!(trend.kind(), Some(TrendlineType::Linear));
    assert!(trend.label().unwrap().layout().unwrap().is_empty());
    assert_eq!(reference(trend).formula().address_str(), "Missing!$A$1");
    assert!(trend.label().unwrap().text().unwrap().rich_text().is_none());
    assert!(series_xml(&book).contains("<c:symbol val=\"diamond\"/>"));
    assert!(conflicts(&book).is_empty());
    save("contained-label-fields", &book);
}

#[test]
fn imported_workbook_keeps_both_distinct_trendline_shapes_and_boolean_presence() {
    let book = read(include_bytes!("fixtures/chart_trendlines.xlsx"));
    let chart = book
        .sheet_collection()
        .iter()
        .flat_map(|s| s.chart_collection())
        .next()
        .unwrap();
    let series: Vec<_> = chart
        .chart_space()
        .chart()
        .plot_area()
        .chart_groups()
        .flat_map(|g| g.series().area_chart_series())
        .filter(|s| !s.trendlines().is_empty())
        .collect();
    assert_eq!(series.len(), 2);
    for series in &series {
        assert_eq!(series.trendlines().len(), 1);
        let t = &series.trendlines()[0];
        assert_eq!(t.kind(), Some(TrendlineType::Linear));
        assert_eq!(t.kind_attribute(), Some("linear"));
        assert_eq!(t.display_r_squared().unwrap().attribute(), Some("0"));
        assert_eq!(t.display_equation().unwrap().attribute(), Some("0"));
        let outline = t.shape_properties().unwrap().outline().unwrap();
        assert_eq!(outline.width(), 19050);
        assert_eq!(outline.cap_type(), Some("rnd"));
    }
    for restored in std::iter::once(book.clone()).chain(persisted(&book)) {
        let xml = chart_part(&write(&restored));
        let mut tail = xml.as_str();
        for accent in ["accent1", "accent2"] {
            let start = tail.find("<c:trendline>").unwrap();
            tail = &tail[start..];
            let end = tail.find("</c:trendline>").unwrap() + "</c:trendline>".len();
            let trend = &tail[..end];
            assert!(trend.contains(&format!("<a:schemeClr val=\"{accent}\"/>")));
            assert!(trend.contains("<a:prstDash val=\"sysDot\"/>"));
            assert!(trend.contains("<a:effectLst/>"));
            assert!(trend.contains(
                "<c:trendlineType val=\"linear\"/><c:dispRSqr val=\"0\"/><c:dispEq val=\"0\"/>"
            ));
            tail = &tail[end..];
        }
        assert!(!tail.contains("<c:trendline>"));
    }
    save("real-trendlines", &book);
}

#[test]
fn trendline_names_decode_references_once_and_preserve_significant_text() {
    let book = input(
        &ChartType::LineChart,
        "<c:trendline><c:name>  &amp;lt; &#x41; <![CDATA[&literal]]>  \
         </c:name><c:trendlineType/></c:trendline>",
    );
    assert_eq!(trends(&book)[0].name(), Some("  &lt; A &literal  "));
    for restored in persisted(&book) {
        assert_eq!(trends(&restored)[0].name(), Some("  &lt; A &literal  "));
    }
    let again = read(&write(&book));
    assert_eq!(trends(&again)[0].name(), Some("  &lt; A &literal  "));
    save("name-references", &book);
}

#[test]
fn layout_coordinate_setters_keep_special_doubles_through_json_and_binary() {
    use umya_spreadsheet::drawing::charts::{
        Height,
        Left,
        Top,
        Width,
    };
    for value in [f64::INFINITY, f64::NEG_INFINITY, f64::NAN] {
        macro_rules! check {
            ($owner:ty) => {{
                            let mut scalar=<$owner>::default();
                            scalar.set_val(value);
                            assert!(scalar.is_valid());
                            let json:$owner=serde_json::from_str(
                                &serde_json::to_string(&scalar).unwrap(),
                            ).unwrap();
                            let binary:$owner=bincode::deserialize(
                                &bincode::serialize(&scalar).unwrap(),
                            ).unwrap();
                            for restored in [json,binary] {
                                assert!(restored.is_valid());
                                let actual=restored.parsed_val().unwrap();
                                assert!(if value.is_nan(){actual.is_nan()}else{actual==value});
                            }
                        }};
        }
        check!(Left);
        check!(Top);
        check!(Width);
        check!(Height);
    }
}

#[test]
fn earlier_label_caches_never_displace_later_existing_series_or_axis_roles_at_either_cap() {
    for text_cap in [false, true] {
        for axis in [false, true] {
            let count = if text_cap { 1 } else { MAX_SOURCE_CACHE_POINTS };
            let value = if text_cap {
                "x".repeat(MAX_SOURCE_CACHE_TEXT_BYTES)
            } else {
                "x".into()
            };
            let mut reference = format!(
                "<c:strRef><c:f>Missing!$A$1</c:f><c:strCache><c:ptCount val=\"{count}\"/>"
            );
            for index in 0..count {
                reference.push_str(&format!("<c:pt idx=\"{index}\"><c:v>{value}</c:v></c:pt>"));
            }
            reference.push_str("</c:strCache></c:strRef>");
            let base = write(&base_book(&ChartType::LineChart, 1));
            let bytes = with_chart_part(&base, |xml| {
                let xml = replace_first_series(
                    xml,
                    &format!(
                        "<c:idx val=\"1\"/><c:order val=\"1\"/>{}",
                        label_ref("Missing!$B$1", "Earlier new label")
                    ),
                );
                if axis {
                    let axis_start = xml.find("<c:valAx>").unwrap();
                    let at = axis_start + xml[axis_start..].find("<c:numFmt").unwrap();
                    format!(
                        "{}<c:title><c:tx>{reference}</c:tx></c:title>{}",
                        &xml[..at],
                        &xml[at..]
                    )
                } else {
                    let at = xml.find("</c:ser>").unwrap() + "</c:ser>".len();
                    format!(
                        "{}<c:ser><c:idx val=\"2\"/><c:order \
                         val=\"2\"/><c:cat>{reference}</c:cat></c:ser>{}",
                        &xml[..at],
                        &xml[at..]
                    )
                }
            });
            let eager = read(&bytes);
            let assert_priority = |book: &Workbook| {
                let report = book.chart_source_cache_report();
                assert_eq!((report.kept_caches, report.over_cap_caches), (1, 1));
                assert_eq!(report.kept_points, count);
                assert_eq!(
                    report.kept_text_bytes,
                    if text_cap {
                        MAX_SOURCE_CACHE_TEXT_BYTES
                    } else {
                        MAX_SOURCE_CACHE_POINTS
                    }
                );
                assert!(!has_source(&trends(book)[0]));
                let plot = chart_of(book).chart_space().chart().plot_area();
                let old = if axis {
                    plot.value_axis()[0]
                        .title()
                        .unwrap()
                        .chart_text()
                        .unwrap()
                        .string_reference()
                        .unwrap()
                } else {
                    plot.chart_groups()
                        .next()
                        .unwrap()
                        .series()
                        .area_chart_series()[1]
                        .category_axis_data()
                        .unwrap()
                        .string_reference()
                        .unwrap()
                };
                assert!(old.string_cache().source().is_some());
            };
            assert_priority(&eager);
            for collection in [false, true] {
                let mut lazy =
                    umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(&bytes), false)
                        .unwrap();
                if collection {
                    lazy.read_sheet_collection();
                } else {
                    lazy.read_sheet(0);
                }
                assert_priority(&lazy);
                let report = *lazy.chart_source_cache_report();
                lazy.read_sheet(0);
                assert_eq!(*lazy.chart_source_cache_report(), report);
            }
        }
    }
}

#[test]
fn populated_point_supplements_from_the_previous_model_load_without_trendlines() {
    use umya_spreadsheet::drawing::charts::SeriesParts;
    // Literal previous model payload: a populated supplement, not a missing
    // outer option and not a current payload with a field removed at runtime.
    let literal = r#"{"data_points":[{"index":{"attribute":"9"},"invert_if_negative":{"attribute":"false"},"marker":null,"bubble_3d":null,"explosion":null,"shape_properties":null,"picture_options":null}]}"#;
    let parts: SeriesParts = serde_json::from_str(literal).unwrap();
    assert_eq!(parts.data_points().len(), 1);
    assert_eq!(parts.data_points()[0].index().val(), Some(9));
    assert_eq!(
        parts.data_points()[0].invert_if_negative().unwrap().value(),
        Some(false)
    );
    assert!(parts.trendlines().is_empty());
    let binary: SeriesParts = bincode::deserialize(&bincode::serialize(&parts).unwrap()).unwrap();
    assert_eq!(binary.data_points()[0].index().val(), Some(9));
    assert!(binary.trendlines().is_empty());
    let mut book = base_book(&ChartType::LineChart, 1);
    *series_mut(&mut book).parts_mut() = parts;
    assert!(
        series_xml(&book)
            .contains("<c:dPt><c:idx val=\"9\"/><c:invertIfNegative val=\"false\"/></c:dPt>")
    );
    assert!(!series_xml(&book).contains("<c:trendline>"));
}
#[test]
fn repeated_label_singletons_stay_diagnosable_through_persistence_and_refuse_output() {
    for (name, children) in [
        (
            "formats",
            "<c:numFmt formatCode=\"A\"/><c:numFmt formatCode=\"B\"/>",
        ),
        ("layouts", "<c:layout/><c:layout/>"),
        (
            "texts",
            "<c:tx><c:strRef><c:f>Missing!$A$1</c:f></c:strRef></c:tx><c:tx><c:strRef><c:\
             f>Missing!$B$1</c:f></c:strRef></c:tx>",
        ),
        ("shapes", "<c:spPr/><c:spPr/>"),
        (
            "text-properties",
            "<c:txPr><a:bodyPr/><a:lstStyle/><a:p/></c:txPr><c:txPr><a:bodyPr/><a:lstStyle/><a:p/\
             ></c:txPr>",
        ),
    ] {
        let book = input(
            &ChartType::LineChart,
            &format!(
                "<c:trendline><c:trendlineType/><c:trendlineLbl>{children}</c:trendlineLbl></c:\
                 trendline>"
            ),
        );
        for restored in std::iter::once(book.clone()).chain(persisted(&book)) {
            assert_eq!(trends(&restored).len(), 1);
            assert_eq!(
                conflicts(&restored),
                vec![(
                    0,
                    0,
                    SeriesPartKind::Trendlines,
                    0,
                    SeriesPartConflictReason::TooMany
                )],
                "{name}"
            );
            assert_eq!(
                serde_json::to_value(trends(&restored)[0].label().unwrap()).unwrap()
                    ["duplicate_children"],
                true
            );
            assert!(!series_xml(&restored).contains("<c:trendline>"), "{name}");
        }
        save(&format!("duplicate-label-{name}"), &book);
    }
}

fn save_negative_cache(book: &Workbook) {
    if let Some(path) = std::env::var_os("UMYA_TRENDLINE_EXPORT_DIR") {
        let directory = std::path::PathBuf::from(path).join("negative-cache");
        std::fs::create_dir_all(&directory).unwrap();
        for (suffix, book) in std::iter::once(("original".to_owned(), book.clone())).chain(
            persisted(book)
                .into_iter()
                .enumerate()
                .map(|(i, b)| (format!("reload-{i}"), b)),
        ) {
            std::fs::write(
                directory.join(format!("missing-value-{suffix}.xlsx")),
                write(&book),
            )
            .unwrap();
        }
    }
}
#[test]
fn legal_sparse_label_caches_keep_repeated_indices_holes_and_present_empty_values() {
    let fixtures: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/trendline_sparse_cache_cases.json")).unwrap();
    let book = input(
        &ChartType::LineChart,
        fixtures["present_empty_value"]["xml"].as_str().unwrap(),
    );
    for restored in std::iter::once(book.clone()).chain(persisted(&book)) {
        let source = reference(&trends(&restored)[0])
            .string_cache()
            .source()
            .unwrap();
        assert_eq!(source.point_count(), Some(9));
        assert_eq!(source.indices().collect::<Vec<_>>(), vec![2, 2, 7]);
        assert_eq!(
            source
                .exceptions()
                .iter()
                .map(|e| (e.index(), e.value()))
                .collect::<Vec<_>>(),
            vec![(2, Some("First")), (2, Some("")), (7, Some(""))]
        );
        assert_eq!(restored.chart_source_cache_report().kept_caches, 1);
        assert_eq!(series_xml(&restored), series_xml(&book));
        assert!(conflicts(&restored).is_empty());
    }
    save("sparse-label-cache", &book);
}
