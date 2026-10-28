//! Registered extension owners through ordinary package import and Chart
//! codecs.
#![cfg(feature = "serde")]
#[path = "chart_extensions/root_dispatch.rs"]
mod chart_root_dispatch;

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
            ExtensionNode,
            ExtensionPayloadValue,
            MAX_SOURCE_CACHE_POINTS,
            MAX_SOURCE_CACHE_TEXT_BYTES,
            SeriesFamily,
            SeriesPartKind,
        },
        spreadsheet::MarkerType,
    },
};
struct Case {
    name:         &'static str,
    supported:    bool,
    source_valid: bool,
    xml:          &'static str,
}
const CASES: &[Case] = &[
    Case {
        name:         "registered-ser-invertSolidFillFmt",
        supported:    true,
        source_valid: true,
        xml:          include_str!(
            "fixtures/chart_extensions/registered-ser-invertSolidFillFmt.xml"
        ),
    },
    Case {
        name:         "registered-ser-filteredSeriesTitle",
        supported:    true,
        source_valid: true,
        xml:          include_str!(
            "fixtures/chart_extensions/registered-ser-filteredSeriesTitle.xml"
        ),
    },
    Case {
        name:         "registered-ser-filteredCategoryTitle",
        supported:    true,
        source_valid: true,
        xml:          include_str!(
            "fixtures/chart_extensions/registered-ser-filteredCategoryTitle.xml"
        ),
    },
    Case {
        name:         "registered-ser-categoryFilterExceptions",
        supported:    true,
        source_valid: true,
        xml:          include_str!(
            "fixtures/chart_extensions/registered-ser-categoryFilterExceptions.xml"
        ),
    },
    Case {
        name:         "registered-ser-datalabelsRange",
        supported:    true,
        source_valid: true,
        xml:          include_str!("fixtures/chart_extensions/registered-ser-datalabelsRange.xml"),
    },
    Case {
        name:         "registered-lineChart-filteredLineSeries",
        supported:    true,
        source_valid: true,
        xml:          include_str!(
            "fixtures/chart_extensions/registered-lineChart-filteredLineSeries.xml"
        ),
    },
    Case {
        name:         "registered-line3DChart-filteredLineSeries",
        supported:    true,
        source_valid: true,
        xml:          include_str!(
            "fixtures/chart_extensions/registered-line3DChart-filteredLineSeries.xml"
        ),
    },
    Case {
        name:         "registered-scatterChart-filteredScatterSeries",
        supported:    true,
        source_valid: true,
        xml:          include_str!(
            "fixtures/chart_extensions/registered-scatterChart-filteredScatterSeries.xml"
        ),
    },
    Case {
        name:         "registered-radarChart-filteredRadarSeries",
        supported:    true,
        source_valid: true,
        xml:          include_str!(
            "fixtures/chart_extensions/registered-radarChart-filteredRadarSeries.xml"
        ),
    },
    Case {
        name:         "registered-barChart-filteredBarSeries",
        supported:    true,
        source_valid: true,
        xml:          include_str!(
            "fixtures/chart_extensions/registered-barChart-filteredBarSeries.xml"
        ),
    },
    Case {
        name:         "registered-bar3DChart-filteredBarSeries",
        supported:    true,
        source_valid: true,
        xml:          include_str!(
            "fixtures/chart_extensions/registered-bar3DChart-filteredBarSeries.xml"
        ),
    },
    Case {
        name:         "registered-areaChart-filteredAreaSeries",
        supported:    true,
        source_valid: true,
        xml:          include_str!(
            "fixtures/chart_extensions/registered-areaChart-filteredAreaSeries.xml"
        ),
    },
    Case {
        name:         "registered-area3DChart-filteredAreaSeries",
        supported:    true,
        source_valid: true,
        xml:          include_str!(
            "fixtures/chart_extensions/registered-area3DChart-filteredAreaSeries.xml"
        ),
    },
    Case {
        name:         "registered-bubbleChart-filteredBubbleSeries",
        supported:    true,
        source_valid: true,
        xml:          include_str!(
            "fixtures/chart_extensions/registered-bubbleChart-filteredBubbleSeries.xml"
        ),
    },
    Case {
        name:         "registered-surfaceChart-filteredSurfaceSeries",
        supported:    false,
        source_valid: true,
        xml:          include_str!(
            "fixtures/chart_extensions/registered-surfaceChart-filteredSurfaceSeries.xml"
        ),
    },
    Case {
        name:         "registered-surface3DChart-filteredSurfaceSeries",
        supported:    false,
        source_valid: true,
        xml:          include_str!(
            "fixtures/chart_extensions/registered-surface3DChart-filteredSurfaceSeries.xml"
        ),
    },
    Case {
        name:         "registered-numRef-fullRef",
        supported:    true,
        source_valid: true,
        xml:          include_str!("fixtures/chart_extensions/registered-numRef-fullRef.xml"),
    },
    Case {
        name:         "registered-numRef-levelRef",
        supported:    true,
        source_valid: true,
        xml:          include_str!("fixtures/chart_extensions/registered-numRef-levelRef.xml"),
    },
    Case {
        name:         "registered-numRef-formulaRef",
        supported:    true,
        source_valid: true,
        xml:          include_str!("fixtures/chart_extensions/registered-numRef-formulaRef.xml"),
    },
    Case {
        name:         "registered-strRef-fullRef",
        supported:    true,
        source_valid: true,
        xml:          include_str!("fixtures/chart_extensions/registered-strRef-fullRef.xml"),
    },
    Case {
        name:         "registered-strRef-levelRef",
        supported:    true,
        source_valid: true,
        xml:          include_str!("fixtures/chart_extensions/registered-strRef-levelRef.xml"),
    },
    Case {
        name:         "registered-strRef-formulaRef",
        supported:    true,
        source_valid: true,
        xml:          include_str!("fixtures/chart_extensions/registered-strRef-formulaRef.xml"),
    },
    Case {
        name:         "registered-multiLvlStrRef-fullRef",
        supported:    true,
        source_valid: true,
        xml:          include_str!(
            "fixtures/chart_extensions/registered-multiLvlStrRef-fullRef.xml"
        ),
    },
    Case {
        name:         "registered-multiLvlStrRef-levelRef",
        supported:    true,
        source_valid: true,
        xml:          include_str!(
            "fixtures/chart_extensions/registered-multiLvlStrRef-levelRef.xml"
        ),
    },
    Case {
        name:         "registered-multiLvlStrRef-formulaRef",
        supported:    true,
        source_valid: true,
        xml:          include_str!(
            "fixtures/chart_extensions/registered-multiLvlStrRef-formulaRef.xml"
        ),
    },
    Case {
        name:         "registered-dLbls-showLeaderLines",
        supported:    true,
        source_valid: true,
        xml:          include_str!(
            "fixtures/chart_extensions/registered-dLbls-showLeaderLines.xml"
        ),
    },
    Case {
        name:         "registered-dLbls-leaderLines",
        supported:    true,
        source_valid: true,
        xml:          include_str!("fixtures/chart_extensions/registered-dLbls-leaderLines.xml"),
    },
    Case {
        name:         "registered-dLbls-tx",
        supported:    true,
        source_valid: true,
        xml:          include_str!("fixtures/chart_extensions/registered-dLbls-tx.xml"),
    },
    Case {
        name:         "registered-dLbls-dlblFieldTable",
        supported:    true,
        source_valid: true,
        xml:          include_str!("fixtures/chart_extensions/registered-dLbls-dlblFieldTable.xml"),
    },
    Case {
        name:         "registered-dLbls-showDataLabelsRange",
        supported:    true,
        source_valid: true,
        xml:          include_str!(
            "fixtures/chart_extensions/registered-dLbls-showDataLabelsRange.xml"
        ),
    },
    Case {
        name:         "registered-dLbls-spPr",
        supported:    true,
        source_valid: true,
        xml:          include_str!("fixtures/chart_extensions/registered-dLbls-spPr.xml"),
    },
    Case {
        name:         "registered-dLbls-layout",
        supported:    true,
        source_valid: true,
        xml:          include_str!("fixtures/chart_extensions/registered-dLbls-layout.xml"),
    },
    Case {
        name:         "registered-dLbl-dlblFieldTable",
        supported:    true,
        source_valid: true,
        xml:          include_str!("fixtures/chart_extensions/registered-dLbl-dlblFieldTable.xml"),
    },
    Case {
        name:         "registered-dLbl-xForSave",
        supported:    true,
        source_valid: true,
        xml:          include_str!("fixtures/chart_extensions/registered-dLbl-xForSave.xml"),
    },
    Case {
        name:         "registered-dLbl-showDataLabelsRange",
        supported:    true,
        source_valid: true,
        xml:          include_str!(
            "fixtures/chart_extensions/registered-dLbl-showDataLabelsRange.xml"
        ),
    },
    Case {
        name:         "registered-dLbl-spPr",
        supported:    true,
        source_valid: true,
        xml:          include_str!("fixtures/chart_extensions/registered-dLbl-spPr.xml"),
    },
    Case {
        name:         "registered-dLbl-layout",
        supported:    true,
        source_valid: true,
        xml:          include_str!("fixtures/chart_extensions/registered-dLbl-layout.xml"),
    },
    Case {
        name:         "registered-ser-uniqueId",
        supported:    true,
        source_valid: true,
        xml:          include_str!("fixtures/chart_extensions/registered-ser-uniqueId.xml"),
    },
    Case {
        name:         "registered-dLbl-uniqueId",
        supported:    true,
        source_valid: true,
        xml:          include_str!("fixtures/chart_extensions/registered-dLbl-uniqueId.xml"),
    },
    Case {
        name:         "registered-dPt-uniqueId",
        supported:    true,
        source_valid: true,
        xml:          include_str!("fixtures/chart_extensions/registered-dPt-uniqueId.xml"),
    },
    Case {
        name:         "ordered-multi-child-repeated-uri",
        supported:    true,
        source_valid: true,
        xml:          include_str!(
            "fixtures/chart_extensions/ordered-multi-child-repeated-uri.xml"
        ),
    },
    Case {
        name:         "alternate-prefix-default-chart-namespace",
        supported:    true,
        source_valid: true,
        xml:          include_str!(
            "fixtures/chart_extensions/alternate-prefix-default-chart-namespace.xml"
        ),
    },
    Case {
        name:         "filtered-series-nested-registered-extension",
        supported:    true,
        source_valid: true,
        xml:          include_str!(
            "fixtures/chart_extensions/filtered-series-nested-registered-extension.xml"
        ),
    },
    Case {
        name:         "unknown-namespaces-mixed-content-nested-reference",
        supported:    true,
        source_valid: true,
        xml:          include_str!(
            "fixtures/chart_extensions/unknown-namespaces-mixed-content-nested-reference.xml"
        ),
    },
    Case {
        name:         "invalid-boolean",
        supported:    true,
        source_valid: false,
        xml:          include_str!("fixtures/chart_extensions/invalid-boolean.xml"),
    },
    Case {
        name:         "missing-invert-shape",
        supported:    true,
        source_valid: false,
        xml:          include_str!("fixtures/chart_extensions/missing-invert-shape.xml"),
    },
    Case {
        name:         "missing-id-val",
        supported:    true,
        source_valid: false,
        xml:          include_str!("fixtures/chart_extensions/missing-id-val.xml"),
    },
    Case {
        name:         "empty-id-val-valid",
        supported:    true,
        source_valid: true,
        xml:          include_str!("fixtures/chart_extensions/empty-id-val-valid.xml"),
    },
    Case {
        name:         "wrong-owner",
        supported:    true,
        source_valid: false,
        xml:          include_str!("fixtures/chart_extensions/wrong-owner.xml"),
    },
    Case {
        name:         "wrong-invert-family",
        supported:    true,
        source_valid: false,
        xml:          include_str!("fixtures/chart_extensions/wrong-invert-family.xml"),
    },
    Case {
        name:         "wrong-namespace-known-local",
        supported:    true,
        source_valid: false,
        xml:          include_str!("fixtures/chart_extensions/wrong-namespace-known-local.xml"),
    },
    Case {
        name:         "known-extension-extra-child",
        supported:    true,
        source_valid: false,
        xml:          include_str!("fixtures/chart_extensions/known-extension-extra-child.xml"),
    },
    Case {
        name:         "known-extension-significant-text",
        supported:    true,
        source_valid: false,
        xml:          include_str!(
            "fixtures/chart_extensions/known-extension-significant-text.xml"
        ),
    },
    Case {
        name:         "misordered-field-entry",
        supported:    true,
        source_valid: false,
        xml:          include_str!("fixtures/chart_extensions/misordered-field-entry.xml"),
    },
    Case {
        name:         "missing-range-formula",
        supported:    true,
        source_valid: false,
        xml:          include_str!("fixtures/chart_extensions/missing-range-formula.xml"),
    },
    Case {
        name:         "empty-known-label-shell",
        supported:    true,
        source_valid: false,
        xml:          include_str!("fixtures/chart_extensions/empty-known-label-shell.xml"),
    },
];
fn chart(book: &Workbook) -> &Chart {
    book.sheet(0)
        .unwrap()
        .chart_collection()
        .first()
        .expect("chart")
}
fn chart_mut(book: &mut Workbook) -> &mut Chart {
    book.sheet_mut(0)
        .unwrap()
        .chart_collection_mut()
        .first_mut()
        .expect("chart")
}
fn series(book: &Workbook) -> &[AreaChartSeries] {
    chart(book)
        .chart_space()
        .chart()
        .plot_area()
        .chart_groups()
        .next()
        .unwrap()
        .series()
        .area_chart_series()
}
fn first_series_mut(book: &mut Workbook) -> &mut AreaChartSeries {
    &mut chart_mut(book)
        .chart_space_mut()
        .chart_mut()
        .plot_area_mut()
        .chart_groups_mut()
        .next()
        .unwrap()
        .series_mut()
        .area_chart_series_mut()[0]
}
fn save(book: &Workbook, tag: &str) -> Vec<u8> {
    let mut output = Vec::new();
    umya_spreadsheet::writer::xlsx::write_writer(book, &mut Cursor::new(&mut output)).unwrap();
    if let Some(dir) = std::env::var_os("UMYA_EXTENSION_SCHEMA_EXPORT_DIR") {
        let dir = std::path::PathBuf::from(dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(format!("{tag}.xlsx")), &output).unwrap();
    }
    output
}
fn load(bytes: &[u8], eager: bool) -> Workbook {
    umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(bytes), eager).unwrap()
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
fn replace_part(bytes: &[u8], xml: &str) -> Vec<u8> {
    let mut input = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for i in 0..input.len() {
        let mut file = input.by_index(i).unwrap();
        let name = file.name().to_owned();
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
fn marker(cell: &str) -> MarkerType {
    let mut value = MarkerType::default();
    value.set_coordinate(cell);
    value
}
fn base() -> Vec<u8> {
    let mut book = umya_spreadsheet::new_file();
    book.new_sheet("O'Brien & Sons").unwrap();
    let mut chart = Chart::default();
    chart.new_chart(
        &ChartType::LineChart,
        marker("E1"),
        marker("M18"),
        vec!["Sheet1!$B$1:$B$3"],
    );
    book.sheet_mut(0).unwrap().add_chart(chart);
    let mut bytes = Vec::new();
    umya_spreadsheet::writer::xlsx::write_writer(&book, &mut Cursor::new(&mut bytes)).unwrap();
    bytes
}
fn codec_variants(book: &Workbook) -> [Chart; 3] {
    let original = chart(book).clone();
    let expected = serde_json::to_value(&original).unwrap();
    let json: Chart = serde_json::from_value(expected.clone()).unwrap();
    let binary: Chart = bincode::deserialize(&bincode::serialize(&original).unwrap()).unwrap();
    assert_eq!(serde_json::to_value(&json).unwrap(), expected);
    assert_eq!(serde_json::to_value(&binary).unwrap(), expected);
    [original, json, binary]
}
#[test]
fn every_supported_registry_fixture_keeps_complete_chart_state_through_both_codecs_and_export() {
    assert_eq!(CASES.len(), 56);
    assert_eq!(CASES.iter().filter(|case| !case.supported).count(), 2);
    // Surface groups remain explicitly unsupported; their two independent
    // input controls are retained and validated, never claimed as roundtrips.
    let container = base();
    let mut exercised = 0;
    for case in CASES.iter().filter(|case| case.supported) {
        let mut book = load(&replace_part(&container, case.xml), true);
        for (variant, value) in codec_variants(&book).into_iter().enumerate() {
            *chart_mut(&mut book) = value;
            let output = save(&book, &format!("registry-{}-{variant}", case.name));
            let reloaded = load(&output, true);
            assert!(
                !reloaded.sheet(0).unwrap().chart_collection().is_empty(),
                "{}",
                case.name
            );
        }
        exercised += 1;
    }
    assert_eq!(exercised, 54);
    assert_eq!(CASES.iter().filter(|case| case.source_valid).count(), 45);
}
#[test]
fn typed_shape_output_preserves_opaque_qname_values_under_all_canonical_prefix_collisions() {
    let input =
        include_str!("fixtures/chart_extensions/typed-shape-nested-opaque-prefix-collision.xml");
    let mut book = load(&replace_part(&base(), input), true);
    for (variant, value) in codec_variants(&book).into_iter().enumerate() {
        *chart_mut(&mut book) = value;
        let output = save(&book, &format!("namespace-collision-{variant}"));
        let xml = part(&output);
        assert!(xml.contains("c:Future a:Future c14:Future c15:Future"));
        assert!(xml.contains("<![CDATA[raw < & text]]>"));
        assert!(xml.contains("<!--keep-->"));
        assert!(xml.contains("<?opaque keep?>"));
        assert!(xml.contains("urn:future:rebound"));
        assert!(xml.contains("c15:Rebound"));
        let reloaded = load(&output, true);
        assert_eq!(series(&reloaded)[0].index().val(), 7);
    }
}
#[test]
fn nested_defined_references_adjust_once_but_unknown_future_reference_languages_stay_opaque() {
    let input = include_str!(
        "fixtures/chart_extensions/unknown-namespaces-mixed-content-nested-reference.xml"
    );
    let mut book = load(&replace_part(&base(), input), true);
    let copied: Chart = bincode::deserialize(
        &bincode::serialize(
            &serde_json::from_value::<Chart>(serde_json::to_value(chart(&book)).unwrap()).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    *chart_mut(&mut book) = copied;
    book.insert_new_row("O'Brien & Sons", 2, 1);
    let output = save(&book, "reference-adjustment");
    let xml = part(&output);
    assert!(xml.contains("$A$1:$A$6"));
    assert!(!xml.contains("$A$1:$A$5"));
    assert!(!xml.contains("$A$1:$A$7"));
    assert!(xml.contains("NOT A STANDARD REFERENCE"));
    assert!(xml.contains("<![CDATA[raw < & text]]>"));
    assert!(xml.contains("<?opaque keep?>"));
    let reloaded = load(&output, true);
    assert!(part(&save(&reloaded, "reference-adjustment-reloaded")).contains("$A$1:$A$6"));
}
fn cache(formula: &str, count: usize, value: impl Fn(usize) -> String, wrapper: &str) -> String {
    let mut xml = format!("<c:f>{formula}</c:f><{wrapper}><c:ptCount val=\"{count}\"/>");
    for index in 0..count {
        xml.push_str(&format!(
            "<c:pt idx=\"{index}\"><c:v>{}</c:v></c:pt>",
            value(index)
        ));
    }
    xml.push_str(&format!("</{wrapper}>"));
    xml
}
fn capacity_part(text_cap: bool) -> String {
    let count = if text_cap {
        1
    } else {
        MAX_SOURCE_CACHE_POINTS - 3
    };
    let large = cache(
        &format!("Absent!$A$1:$A${count}"),
        count,
        |_| {
            if text_cap {
                "x".repeat(MAX_SOURCE_CACHE_TEXT_BYTES - 3)
            } else {
                "o".into()
            }
        },
        "c:strCache",
    );
    let later = cache("Absent!$B$1", 1, |_| "L".into(), "c:strCache");
    let axis = cache("Absent!$C$1", 1, |_| "A".into(), "c:strCache");
    let trend = cache("Absent!$E$1", 1, |_| "T".into(), "c:strCache");
    let range = cache("Absent!$D$1", 1, |_| "new".into(), "c15:dlblRangeCache")
        .replace("<c:f>", "<c15:f>")
        .replace("</c:f>", "</c15:f>");
    format!(concat!("<c:chartSpace xmlns:c=\"http://schemas.openxmlformats.org/drawingml/2006/chart\" xmlns:c15=\"http://schemas.microsoft.com/office/drawing/2012/chart\"><c:chart><c:plotArea><c:lineChart><c:grouping val=\"standard\"/>",
        "<c:ser><c:idx val=\"0\"/><c:order val=\"0\"/><c:trendline><c:trendlineType val=\"linear\"/><c:trendlineLbl><c:tx><c:strRef>{trend}</c:strRef></c:tx></c:trendlineLbl></c:trendline>",
        "<c:extLst><c:ext uri=\"{{02D57815-91ED-43cb-92C2-25804820EDAC}}\"><c15:datalabelsRange>{range}</c15:datalabelsRange></c:ext></c:extLst></c:ser>",
        "<c:ser><c:idx val=\"1\"/><c:order val=\"1\"/><c:cat><c:strRef>{large}</c:strRef></c:cat></c:ser>",
        "<c:ser><c:idx val=\"2\"/><c:order val=\"2\"/><c:cat><c:strRef>{later}</c:strRef></c:cat></c:ser><c:axId val=\"1\"/><c:axId val=\"2\"/></c:lineChart>",
        "<c:catAx><c:axId val=\"1\"/><c:scaling/><c:axPos val=\"b\"/><c:title><c:tx><c:strRef>{axis}</c:strRef></c:tx></c:title><c:crossAx val=\"2\"/></c:catAx>",
        "<c:valAx><c:axId val=\"2\"/><c:scaling/><c:axPos val=\"l\"/><c:crossAx val=\"1\"/></c:valAx></c:plotArea></c:chart></c:chartSpace>"),trend=trend,range=range,large=large,later=later,axis=axis)
}
fn assert_capacity_priority(book: &Workbook, text_cap: bool) {
    let report = book.chart_source_cache_report();
    assert_eq!(report.kept_caches, 4);
    assert_eq!(report.over_cap_caches, 1);
    assert_eq!(report.reduced_caches, 0);
    assert_eq!(
        report.kept_points,
        if text_cap { 4 } else { MAX_SOURCE_CACHE_POINTS }
    );
    assert_eq!(
        report.kept_text_bytes,
        if text_cap {
            MAX_SOURCE_CACHE_TEXT_BYTES
        } else {
            MAX_SOURCE_CACHE_POINTS
        }
    );
    assert_eq!(report.over_cap_points, 1);
    assert_eq!(report.over_cap_text_bytes, 3);
    let series = series(book);
    assert_eq!(series.len(), 3);
    for value in &series[1..] {
        assert!(
            value
                .category_axis_data()
                .unwrap()
                .string_reference()
                .unwrap()
                .string_cache()
                .source()
                .is_some()
        );
    }
    assert!(
        series[0].trendlines()[0]
            .label()
            .unwrap()
            .text()
            .unwrap()
            .string_reference()
            .unwrap()
            .string_cache()
            .source()
            .is_some()
    );
    let axis = chart(book)
        .chart_space()
        .chart()
        .plot_area()
        .category_axis()[0]
        .title()
        .unwrap()
        .chart_text()
        .unwrap()
        .string_reference()
        .unwrap();
    assert!(axis.string_cache().source().is_some());
    let ExtensionNode::Known(payload) = &series[0].extensions().unwrap().entries[0].nodes[0] else {
        panic!("modeled label range")
    };
    let ExtensionPayloadValue::DataLabelsRange(range) = &payload.value else {
        panic!("label range")
    };
    assert!(range.reference().string_cache().source().is_none());
}
#[test]
fn new_extension_cache_follows_every_old_series_axis_and_trend_label_at_both_caps_eager_and_lazy() {
    for text_cap in [false, true] {
        let bytes = replace_part(&base(), &capacity_part(text_cap));
        let eager = load(&bytes, true);
        assert_capacity_priority(&eager, text_cap);
        let output = save(
            &eager,
            if text_cap {
                "capacity-text"
            } else {
                "capacity-points"
            },
        );
        assert!(!part(&output).contains(">new</"));
        for collection in [false, true] {
            let mut lazy = load(&bytes, false);
            assert_eq!(lazy.chart_source_cache_report().kept_caches, 0);
            if collection {
                lazy.read_sheet_collection();
            } else {
                lazy.read_sheet(0);
            }
            assert_capacity_priority(&lazy, text_cap);
            let report = *lazy.chart_source_cache_report();
            let before = serde_json::to_value(chart(&lazy)).unwrap();
            lazy.read_sheet(0);
            assert_eq!(*lazy.chart_source_cache_report(), report);
            assert_eq!(serde_json::to_value(chart(&lazy)).unwrap(), before);
        }
    }
}
#[test]
fn live_family_diagnostics_preserve_invert_fill_until_explicit_compatible_part_removal() {
    let input = include_str!("fixtures/chart_extensions/registered-ser-invertSolidFillFmt.xml");
    let mut book = load(&replace_part(&base(), input), true);
    {
        let stored = first_series_mut(&mut book);
        assert!(stored.extensions().is_some());
        let diagnostics = stored.extensions().unwrap().export_diagnostics_for_owner(
            None,
            "ser",
            Some(SeriesFamily::Line),
        );
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].reason.contains("NotAllowed"));
        assert_eq!(stored.extensions().unwrap().entries.len(), 1);
    }
    assert!(part(&save(&book, "invert-before-explicit-removal")).contains("invertSolidFillFmt"));
    let list = chart_mut(&mut book)
        .chart_space_mut()
        .chart_mut()
        .plot_area_mut()
        .chart_groups_mut()
        .next()
        .unwrap()
        .series_mut();
    let removed = list.remove_parts_not_allowed_in(SeriesFamily::Line);
    assert_eq!(
        removed
            .iter()
            .map(|record| (record.series_index, record.part, record.occurrence))
            .collect::<Vec<_>>(),
        vec![(0, SeriesPartKind::Extensions, 0)]
    );
    assert!(
        list.remove_parts_not_allowed_in(SeriesFamily::Line)
            .is_empty()
    );
    assert!(series(&book)[0].extensions().unwrap().entries.is_empty());
    assert!(!part(&save(&book, "invert-after-explicit-removal")).contains("invertSolidFillFmt"));
}

fn field_table_part(group: bool, guids: [Option<&str>; 2]) -> String {
    let mut entries = String::new();
    for (i, guid) in guids.into_iter().enumerate() {
        let guid = guid.map_or_else(String::new, |guid| {
            format!("<c15:txfldGUID>{guid}</c15:txfldGUID>")
        });
        let f = if i == 0 { "Absent!$A$1" } else { "Absent!$B$1" };
        let value = if i == 0 { "left" } else { "right" };
        entries.push_str(&format!(
            "<c15:dlblFTEntry>{guid}<c15:f>{f}</c15:f><c15:dlblFieldTableCache><c:ptCount \
             val=\"1\"/><c:pt \
             idx=\"0\"><c:v>{value}</c:v></c:pt></c15:dlblFieldTableCache></c15:dlblFTEntry>"
        ));
    }
    let labels = format!(
        "<c:dLbls><c:extLst><c:ext \
         uri=\"{{CE6537A1-D6FC-4f65-9D91-7224C49458BB}}\"><c15:dlblFieldTable>{entries}</c15:\
         dlblFieldTable></c:ext></c:extLst></c:dLbls>"
    );
    let (inside, outside) = if group {
        (String::new(), labels)
    } else {
        (labels, String::new())
    };
    format!("<c:chartSpace xmlns:c=\"http://schemas.openxmlformats.org/drawingml/2006/chart\" xmlns:c15=\"http://schemas.microsoft.com/office/drawing/2012/chart\"><c:chart><c:plotArea><c:barChart><c:barDir val=\"col\"/><c:ser><c:idx val=\"7\"/><c:order val=\"8\"/>{inside}</c:ser>{outside}<c:axId val=\"1\"/><c:axId val=\"2\"/></c:barChart></c:plotArea></c:chart></c:chartSpace>")
}
fn field_sources(book: &Workbook, group: bool) -> Vec<bool> {
    let labels = if group {
        chart(book)
            .chart_space()
            .chart()
            .plot_area()
            .bar_chart()
            .unwrap()
            .data_labels()
    } else {
        series(book)[0].data_labels().unwrap()
    };
    let ExtensionNode::Known(payload) = &labels.extensions().unwrap().entries[0].nodes[0] else {
        panic!("typed field table")
    };
    let ExtensionPayloadValue::FieldTable(table) = &payload.value else {
        panic!("field table")
    };
    table
        .entries()
        .iter()
        .map(|entry| entry.reference().string_cache().source().is_some())
        .collect()
}
#[test]
fn field_cache_adoption_uses_present_unique_guid_before_formula_even_for_group_labels() {
    for group in [false, true] {
        for (name, guids, expected) in [
            ("unique", [Some("alpha"), Some("beta")], vec![true, true]),
            (
                "duplicate",
                [Some("same"), Some("same")],
                vec![false, false],
            ),
            ("missing", [None, Some("beta")], vec![false, true]),
            ("present-empty", [Some(""), Some("beta")], vec![true, true]),
        ] {
            let mut book = load(
                &replace_part(&base(), &field_table_part(group, guids)),
                true,
            );
            assert_eq!(
                field_sources(&book, group),
                vec![true, true],
                "actual source caches must exist before testing guards"
            );
            for (variant, previous) in codec_variants(&book).into_iter().enumerate() {
                let mut incoming = previous.clone();
                incoming.strip_source_caches();
                *chart_mut(&mut book) = incoming;
                assert_eq!(field_sources(&book, group), vec![false, false]);
                chart_mut(&mut book).adopt_source_caches(&previous);
                assert_eq!(
                    field_sources(&book, group),
                    expected,
                    "{name}, group={group}"
                );
                save(&book, &format!("adoption-{group}-{name}-{variant}"));
                // A formula edit cannot inherit a matching GUID's stale data.
                if name == "unique" {
                    let labels = if group {
                        chart_mut(&mut book)
                            .chart_space_mut()
                            .chart_mut()
                            .plot_area_mut()
                            .bar_chart_mut()
                            .unwrap()
                            .data_labels_mut()
                    } else {
                        first_series_mut(&mut book).data_labels_mut().unwrap()
                    };
                    let ExtensionNode::Known(payload) =
                        &mut labels.extensions_mut().unwrap().entries[0].nodes[0]
                    else {
                        panic!("typed field table")
                    };
                    let ExtensionPayloadValue::FieldTable(table) = &mut payload.value else {
                        panic!("field table")
                    };
                    table.entries_mut()[0]
                        .reference_mut()
                        .formula_mut()
                        .set_address_str("Absent!$C$1");
                    chart_mut(&mut book).adopt_source_caches(&previous);
                    assert_eq!(field_sources(&book, group), vec![false, true]);
                }
            }
        }
    }
}
#[test]
fn invalid_opaque_namespace_is_diagnosed_and_export_is_atomic_without_storage_mutation() {
    use umya_spreadsheet::drawing::charts::{
        QName,
        SeriesPartConflictReason,
    };
    let input = include_str!(
        "fixtures/chart_extensions/unknown-namespaces-mixed-content-nested-reference.xml"
    );
    let mut book = load(&replace_part(&base(), input), true);
    {
        let entry = &mut first_series_mut(&mut book)
            .extensions_mut()
            .unwrap()
            .entries[0];
        let ExtensionNode::Unknown(root) = &mut entry.nodes[0] else {
            panic!("opaque future root")
        };
        root.attributes.push((
            QName {
                namespace_uri:    "urn:missing".into(),
                local_name:       "type".into(),
                preferred_prefix: Some("missing".into()),
            },
            "missing:Value".into(),
        ));
    }
    let before = serde_json::to_value(chart(&book)).unwrap();
    let diagnostics = series(&book)[0]
        .extensions()
        .unwrap()
        .export_diagnostics_for_owner(Some(&book), "ser", Some(SeriesFamily::Bar));
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].reason.contains("undeclared prefix"));
    assert!(
        chart(&book)
            .chart_space()
            .chart()
            .plot_area()
            .series_part_conflicts()
            .iter()
            .any(|value| value.part == SeriesPartKind::Extensions
                && value.reason == SeriesPartConflictReason::InvalidValue)
    );
    let output = save(&book, "invalid-stored-namespace");
    assert!(!part(&output).contains("urn:future:outer"));
    assert_eq!(serde_json::to_value(chart(&book)).unwrap(), before);
}

#[test]
fn typed_shape_mutation_allocates_a_fresh_prefix_without_rebinding_opaque_attribute_values() {
    use umya_spreadsheet::drawing::NoFill;
    let input =
        include_str!("fixtures/chart_extensions/typed-mutation-needs-fresh-drawing-prefix.xml");
    let mut book = load(&replace_part(&base(), input), true);
    {
        let labels = first_series_mut(&mut book).data_labels_mut().unwrap();
        let ExtensionNode::Known(payload) =
            &mut labels.labels_mut()[0].extensions_mut().unwrap().entries[0].nodes[0]
        else {
            panic!("typed label shape")
        };
        let ExtensionPayloadValue::LabelShape(shape) = &mut payload.value else {
            panic!("label shape")
        };
        shape.set_no_fill(NoFill::default());
    }
    for (variant, value) in codec_variants(&book).into_iter().enumerate() {
        *chart_mut(&mut book) = value;
        let output = save(&book, &format!("fresh-prefix-{variant}"));
        let xml = part(&output);
        assert!(xml.contains("chartns"));
        assert!(xml.contains("a:Future"));
        assert!(xml.contains("urn:future:a"));
        let reloaded = load(&output, true);
        assert!(!reloaded.sheet(0).unwrap().chart_collection().is_empty());
    }
}
#[test]
fn unknown_wrappers_never_admit_native_lookalike_caches_or_adjust_their_formula_language() {
    let original = include_str!(
        "fixtures/chart_extensions/unknown-namespaces-mixed-content-nested-reference.xml"
    );
    let fake = concat!(
        "<c:strRef><c:f>Sheet1!$A$1</c:f><c:strCache><c:ptCount val=\"1\"/><c:pt \
         idx=\"0\"><c:v>FAKE_CACHE</c:v></c:pt></c:strCache></c:strRef>",
        "<c:numRef><c:f>Sheet1!$B$1</c:f><c:numCache><c:ptCount val=\"1\"/><c:pt \
         idx=\"0\"><c:v>99</c:v></c:pt></c:numCache></c:numRef>"
    );
    let input = original.replacen("before<![CDATA[", &format!("{fake}before<![CDATA["), 1);
    for eager in [true, false] {
        let mut book = load(&replace_part(&base(), &input), eager);
        if !eager {
            book.read_sheet_collection();
        }
        assert_eq!(book.chart_source_cache_report().kept_caches, 0);
        assert_eq!(book.chart_source_cache_report().over_cap_caches, 0);
        let copy: Chart = bincode::deserialize(&bincode::serialize(chart(&book)).unwrap()).unwrap();
        *chart_mut(&mut book) = copy;
        book.insert_new_row("Sheet1", 1, 1);
        let output = save(&book, &format!("unknown-lookalike-{eager}"));
        let xml = part(&output);
        assert!(xml.contains("Sheet1!$A$1"));
        assert!(xml.contains("Sheet1!$B$1"));
        assert!(!xml.contains("Sheet1!$A$2"));
        assert!(xml.contains("FAKE_CACHE"));
    }
}
#[test]
fn known_reference_children_require_their_actual_expanded_namespace_not_local_spelling() {
    use umya_spreadsheet::drawing::charts::SeriesPartConflictReason;
    let original = include_str!("fixtures/chart_extensions/registered-ser-datalabelsRange.xml");
    for cache_foreign in [false, true] {
        let input = if cache_foreign {
            original
                .replace(
                    "<c15:dlblRangeCache>",
                    "<future:dlblRangeCache xmlns:future=\"urn:foreign\">",
                )
                .replace("</c15:dlblRangeCache>", "</future:dlblRangeCache>")
        } else {
            original
                .replace("<c15:f>", "<future:f xmlns:future=\"urn:foreign\">")
                .replace("</c15:f>", "</future:f>")
        };
        let book = load(&replace_part(&base(), &input), true);
        let ExtensionNode::Known(payload) =
            &series(&book)[0].extensions().unwrap().entries[0].nodes[0]
        else {
            panic!("typed range")
        };
        let ExtensionPayloadValue::DataLabelsRange(range) = &payload.value else {
            panic!("range")
        };
        if cache_foreign {
            assert!(!range.cache_present());
            assert_eq!(book.chart_source_cache_report().kept_caches, 0);
            assert_eq!(
                range.conflict(),
                Some(SeriesPartConflictReason::InvalidValue)
            );
        } else {
            assert_eq!(
                range.conflict(),
                Some(SeriesPartConflictReason::MissingRequiredChild)
            );
        }
        let output = save(&book, &format!("foreign-known-child-{cache_foreign}"));
        assert!(!part(&output).contains("urn:foreign"));
    }
    let original = field_table_part(false, [Some("alpha"), Some("beta")]);
    for namespace in [
        "http://schemas.microsoft.com/office/drawing/2012/chart",
        "urn:foreign",
    ] {
        let input = original.replacen(
            "<c15:txfldGUID>alpha</c15:txfldGUID>",
            &format!("<field:txfldGUID xmlns:field=\"{namespace}\">alpha</field:txfldGUID>"),
            1,
        );
        let mut book = load(&replace_part(&base(), &input), true);
        let previous = chart(&book).clone();
        chart_mut(&mut book).strip_source_caches();
        chart_mut(&mut book).adopt_source_caches(&previous);
        let foreign = namespace == "urn:foreign";
        assert_eq!(field_sources(&book, false), vec![!foreign, true]);
        save(&book, &format!("field-guid-namespace-{foreign}"));
    }
}

#[test]
fn invalid_point_marker_and_shape_extensions_do_not_drop_valid_native_siblings() {
    let input = include_str!(
        "fixtures/chart_extensions/invalid-point-and-marker-extension-native-siblings.xml"
    );
    let mut book = load(&replace_part(&base(), input), true);
    for (variant, value) in codec_variants(&book).into_iter().enumerate() {
        *chart_mut(&mut book) = value;
        let point = &series(&book)[0].data_points()[0];
        assert_eq!(point.index().val(), Some(9));
        assert_eq!(
            point.marker().unwrap().size().unwrap().parsed_val(),
            Some(7)
        );
        assert_eq!(
            point
                .picture_options()
                .unwrap()
                .apply_to_end()
                .unwrap()
                .value(),
            Some(true)
        );
        let diagnostics =
            point
                .extensions()
                .unwrap()
                .export_diagnostics_for_owner(Some(&book), "dPt", None);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].reason, "MissingRequiredChild");
        assert_eq!(
            point
                .marker()
                .unwrap()
                .extensions()
                .unwrap()
                .export_diagnostics_for_owner(Some(&book), "marker", None)[0]
                .reason,
            "NotAllowed"
        );
        assert_eq!(
            point
                .shape_properties()
                .unwrap()
                .extensions()
                .unwrap()
                .export_diagnostics_for_owner(Some(&book), "spPr", None)[0]
                .reason,
            "MissingRequiredChild"
        );
        assert!(
            chart(&book)
                .chart_space()
                .chart()
                .plot_area()
                .series_part_conflicts()
                .iter()
                .any(|conflict| conflict.part == SeriesPartKind::DataPoints)
        );
        let before = serde_json::to_value(chart(&book)).unwrap();
        let output = save(&book, &format!("point-extension-siblings-{variant}"));
        let xml = part(&output);
        assert!(xml.contains("<c:dPt><c:idx val=\"9\"/>"));
        assert!(xml.contains("<c:symbol val=\"circle\"/>"));
        assert!(xml.contains("ED7D31"));
        assert!(xml.contains("<a:noFill/>"));
        assert!(xml.contains("<c:applyToEnd val=\"true\"/>"));
        assert!(!xml.contains("uniqueId"));
        assert!(!xml.contains("urn:future:payload"));
        assert_eq!(serde_json::to_value(chart(&book)).unwrap(), before);
        let reloaded = load(&output, true);
        assert_eq!(series(&reloaded)[0].data_points()[0].index().val(), Some(9));
        assert_eq!(
            series(&reloaded)[0].data_points()[0]
                .marker()
                .unwrap()
                .size()
                .unwrap()
                .parsed_val(),
            Some(7)
        );
    }
}
#[test]
fn specialized_label_references_preserve_invalid_source_order_instead_of_legalizing_it() {
    use umya_spreadsheet::drawing::charts::SeriesPartConflictReason;
    let range = include_str!("fixtures/chart_extensions/registered-ser-datalabelsRange.xml");
    let start = range.find("<c15:f>").unwrap();
    let end = start + range[start..].find("</c15:f>").unwrap() + "</c15:f>".len();
    let formula = &range[start..end];
    let range = range.replacen(formula, "", 1).replacen(
        "</c15:dlblRangeCache>",
        &format!("</c15:dlblRangeCache>{formula}"),
        1,
    );
    let field = field_table_part(false, [Some("alpha"), Some("beta")]).replacen(
        "<c15:txfldGUID>alpha</c15:txfldGUID><c15:f>Absent!$A$1</c15:f>",
        "<c15:f>Absent!$A$1</c15:f><c15:txfldGUID>alpha</c15:txfldGUID>",
        1,
    );
    for (name, input) in [("range", range), ("field", field)] {
        let mut book = load(&replace_part(&base(), &input), true);
        for (variant, value) in codec_variants(&book).into_iter().enumerate() {
            *chart_mut(&mut book) = value;
            let list = if name == "range" {
                series(&book)[0].extensions().unwrap()
            } else {
                series(&book)[0]
                    .data_labels()
                    .unwrap()
                    .extensions()
                    .unwrap()
            };
            let ExtensionNode::Known(payload) = &list.entries[0].nodes[0] else {
                panic!("typed reference owner")
            };
            assert_eq!(
                payload.conflict(),
                Some(SeriesPartConflictReason::InvalidValue)
            );
            let before = serde_json::to_value(chart(&book)).unwrap();
            assert!(before.to_string().contains("Invalid"));
            let output = save(&book, &format!("reference-order-{name}-{variant}"));
            assert!(!part(&output).contains(if name == "range" {
                "datalabelsRange"
            } else {
                "dlblFieldTable"
            }));
            assert_eq!(serde_json::to_value(chart(&book)).unwrap(), before);
            assert!(
                !load(&output, true)
                    .sheet(0)
                    .unwrap()
                    .chart_collection()
                    .is_empty()
            );
        }
    }
}
