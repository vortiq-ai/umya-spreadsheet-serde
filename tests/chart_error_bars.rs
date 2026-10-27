//! Native error bars through ordinary XLSX and chart persistence routes.
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
            ErrorBarType,
            ErrorBarValueType,
            NumericDataSource,
            SeriesPartConflictReason,
        },
        spreadsheet::MarkerType,
    },
};

fn save(book: &Workbook) -> Vec<u8> {
    let mut bytes = Vec::new();
    umya_spreadsheet::writer::xlsx::write_writer(book, &mut Cursor::new(&mut bytes)).unwrap();
    bytes
}
fn load(bytes: &[u8]) -> Workbook {
    umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap()
}
fn chart(book: &Workbook) -> &Chart {
    book.sheet_collection()
        .iter()
        .flat_map(|s| s.chart_collection())
        .next()
        .expect("a chart")
}
fn series(book: &Workbook) -> &AreaChartSeries {
    &chart(book)
        .chart_space()
        .chart()
        .plot_area()
        .chart_groups()
        .next()
        .unwrap()
        .series()
        .area_chart_series()[0]
}
fn xml(bytes: &[u8]) -> String {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut text = String::new();
    archive
        .by_name("xl/charts/chart1.xml")
        .unwrap()
        .read_to_string(&mut text)
        .unwrap();
    text
}
fn source(kind: &ChartType, children: &str, prefix: &str) -> Workbook {
    let mut book = umya_spreadsheet::new_file();
    for row in 1..=3 {
        book.sheet_mut(0)
            .unwrap()
            .cell_mut((2, row))
            .set_value_number(row * 10);
    }
    let mut from = MarkerType::default();
    from.set_coordinate("E1");
    let mut to = MarkerType::default();
    to.set_coordinate("M20");
    let mut item = Chart::default();
    let refs = match kind {
        ChartType::ScatterChart => vec!["Sheet1!B1:B3", "Sheet1!B1:B3"],
        ChartType::BubbleChart => vec!["Sheet1!B1:B3"; 3],
        ChartType::DoughnutChart => vec!["Sheet1!B1:B3"; 2],
        _ => vec!["Sheet1!B1:B3"],
    };
    item.new_chart(kind, from, to, refs);
    book.sheet_mut(0).unwrap().add_chart(item);
    let bytes = save(&book);
    let mut input = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for n in 0..input.len() {
        let mut file = input.by_index(n).unwrap();
        let name = file.name().to_owned();
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).unwrap();
        if name == "xl/charts/chart1.xml" {
            let mut text = String::from_utf8(bytes).unwrap();
            let start = text.find("<c:ser>").unwrap();
            let end = start + text[start..].find("</c:ser>").unwrap() + 8;
            text.replace_range(
                start..end,
                &format!("<c:ser><c:idx val=\"7\"/><c:order val=\"8\"/>{children}</c:ser>"),
            );
            text = match prefix {
                "" => text.replace("xmlns:c=", "xmlns=").replace("c:", ""),
                "c" => text,
                p => text
                    .replace("xmlns:c=", &format!("xmlns:{p}="))
                    .replace("c:", &format!("{p}:")),
            };
            bytes = text.into_bytes();
        }
        output
            .start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        output.write_all(&bytes).unwrap();
    }
    load(&output.finish().unwrap().into_inner())
}
fn stored(book: &Workbook) -> Vec<Workbook> {
    let json: Chart = serde_json::from_slice(&serde_json::to_vec(chart(book)).unwrap()).unwrap();
    let binary: Chart = bincode::deserialize(&bincode::serialize(chart(book)).unwrap()).unwrap();
    let mut books = vec![book.clone()];
    for value in [json, binary] {
        let mut copy = book.clone();
        *copy
            .sheet_collection_mut()
            .iter_mut()
            .flat_map(|s| s.chart_collection_mut())
            .next()
            .expect("a chart") = value;
        books.push(copy);
    }
    books
}
fn bars(book: &Workbook) -> Vec<String> {
    let xml = xml(&save(book));
    let mut rest = xml.as_str();
    let mut result = Vec::new();
    while let Some(start) = rest.find("<c:errBars>") {
        rest = &rest[start..];
        let end = rest.find("</c:errBars>").unwrap() + 12;
        result.push(rest[..end].to_owned());
        rest = &rest[end..];
    }
    result
}
const DEFAULTS: &str = "<c:errBars><c:errBarType/><c:errValType/><c:noEndCap/></c:errBars>";
const LITERAL: &str = concat!(
    "<c:errBars><c:errDir val=\"y\"/><c:errBarType val=\"minus\"/><c:errValType val=\"cust\"/>",
    "<c:plus><c:numLit><c:formatCode>0.00</c:formatCode><c:ptCount val=\"0003\"/>",
    "<c:pt idx=\"9\" formatCode=\"0%\"><c:v>1.2500E+1</c:v></c:pt>",
    "<c:pt idx=\"2\"><c:v></c:v></c:pt><c:pt idx=\"9\"><c:v>-2</c:v></c:pt>",
    "</c:numLit></c:plus><c:val val=\"1.2500E+1\"/></c:errBars>"
);

#[test]
fn error_bars_keep_defaults_lexical_literals_sparse_repeated_points_and_storage() {
    for prefix in ["c", "z", ""] {
        let book = source(
            &ChartType::AreaChart,
            &format!("{DEFAULTS}{LITERAL}"),
            prefix,
        );
        witness(
            &format!(
                "prefix-{}",
                if prefix.is_empty() { "default" } else { prefix }
            ),
            &book,
        );
        for stored in stored(&book) {
            assert_eq!(series(&stored).index().val(), 7);
            assert_eq!(series(&stored).order().val(), 8);
            let parts = series(&stored).error_bars();
            assert_eq!(parts.len(), 2);
            assert!(parts[0].bar_type().attribute().is_none());
            assert_eq!(parts[0].bar_type_value(), Some(ErrorBarType::Both));
            assert_eq!(
                parts[0].value_type_value(),
                Some(ErrorBarValueType::FixedValue)
            );
            assert_eq!(parts[0].no_end_cap().unwrap().attribute(), None);
            assert_eq!(parts[1].value().unwrap().attribute(), Some("1.2500E+1"));
            let Some(NumericDataSource::Literal(lit)) = parts[1].plus() else {
                panic!("literal choice")
            };
            assert_eq!(lit.point_count().unwrap().attribute(), Some("0003"));
            assert_eq!(
                lit.points()
                    .iter()
                    .map(|p| p.index().val())
                    .collect::<Vec<_>>(),
                vec![Some(9), Some(2), Some(9)]
            );
            assert_eq!(
                lit.points().iter().map(|p| p.value()).collect::<Vec<_>>(),
                vec![Some("1.2500E+1"), Some(""), Some("-2")]
            );
            assert_eq!(bars(&stored), vec![DEFAULTS.to_owned(), LITERAL.to_owned()]);
            assert!(
                chart(&stored)
                    .chart_space()
                    .chart()
                    .plot_area()
                    .series_part_conflicts()
                    .is_empty()
            );
        }
    }
}
#[test]
fn unknown_subtrees_cannot_change_parent_or_literal_values() {
    let text = concat!(
        "<c:errBars><c:future><c:errBarType val=\"plus\"/><c:val val=\"999\"/></c:future>",
        "<c:errBarType/><c:errValType/><c:plus><c:numLit><c:future><c:pt \
         idx=\"999\"><c:v>999</c:v></c:pt></c:future>",
        "<c:pt idx=\"4\"><c:future><c:v>999</c:v></c:future><c:v>3</c:v></c:pt></c:numLit></c:\
         plus></c:errBars>"
    );
    let book = source(&ChartType::LineChart, text, "c");
    witness("contained", &book);
    let parts = series(&book).error_bars();
    assert_eq!(parts[0].bar_type_value(), Some(ErrorBarType::Both));
    assert!(parts[0].value().is_none());
    let Some(NumericDataSource::Literal(lit)) = parts[0].plus() else {
        panic!("literal")
    };
    assert_eq!(lit.points().len(), 1);
    assert_eq!(lit.points()[0].index().val(), Some(4));
    assert_eq!(lit.points()[0].value(), Some("3"));
}
#[test]
fn invalid_sources_are_retained_reported_and_not_emitted() {
    for (name, clear_plus, clear_minus) in [
        ("invalid-missing-sources-both", false, false),
        ("invalid-missing-sources-minus", true, false),
        ("invalid-missing-sources-plus", false, true),
        ("cleared-missing-sources", true, true),
    ] {
        let mut book = source(
            &ChartType::LineChart,
            "<c:errBars><c:errBarType/><c:errValType/><c:plus/><c:minus/></c:errBars>",
            "c",
        );
        let error = &mut book.sheet_mut(0).unwrap().chart_collection_mut()[0]
            .chart_space_mut()
            .chart_mut()
            .plot_area_mut()
            .line_chart_mut()
            .unwrap()
            .area_chart_series_list_mut()
            .area_chart_series_mut()[0]
            .error_bars_mut()[0];
        if clear_plus {
            error.set_plus(None);
        }
        if clear_minus {
            error.set_minus(None);
        }
        let valid = clear_plus && clear_minus;
        witness(name, &book);
        for stored in stored(&book) {
            assert_eq!(
                series(&stored).error_bars()[0].conflict(),
                (!valid).then_some(SeriesPartConflictReason::MissingRequiredChild)
            );
            assert_eq!(bars(&stored).len(), usize::from(valid));
        }
    }
    assert_eq!(
        bincode::serialize(&umya_spreadsheet::drawing::charts::ErrorBars::default())
            .unwrap()
            .len(),
        36
    );
    for (case, (content, reason)) in [
        (
            "<c:errBars><c:errValType/></c:errBars>",
            SeriesPartConflictReason::MissingRequiredChild,
        ),
        (
            "<c:errBars><c:errBarType/><c:errValType/><c:plus/></c:errBars>",
            SeriesPartConflictReason::MissingRequiredChild,
        ),
        (
            "<c:errBars><c:errDir/><c:errBarType/><c:errValType/></c:errBars>",
            SeriesPartConflictReason::InvalidValue,
        ),
        (
            "<c:errBars><c:errBarType val=\"future\"/><c:errValType/></c:errBars>",
            SeriesPartConflictReason::InvalidValue,
        ),
        (
            "<c:errBars><c:errBarType/><c:errValType/><c:plus><c:numRef><c:f>Sheet1!B1</c:f></c:\
             numRef><c:numLit/></c:plus></c:errBars>",
            SeriesPartConflictReason::ConflictingChoice,
        ),
        (
            "<c:errBars><c:errBarType/><c:errValType/><c:plus><c:numLit><c:pt><c:v>3</c:v></c:\
             pt></c:numLit></c:plus></c:errBars>",
            SeriesPartConflictReason::InvalidValue,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let book = source(&ChartType::LineChart, content, "c");
        witness(&format!("invalid-{case}"), &book);
        for stored in stored(&book) {
            assert_eq!(series(&stored).error_bars().len(), 1);
            assert_eq!(series(&stored).error_bars()[0].conflict(), Some(reason));
            assert!(bars(&stored).is_empty());
            let conflicts = chart(&stored)
                .chart_space()
                .chart()
                .plot_area()
                .series_part_conflicts();
            assert_eq!(conflicts.len(), 1);
            assert_eq!(conflicts[0].reason, reason);
            assert_eq!(conflicts[0].occurrence, 0);
        }
    }
    let book = source(
        &ChartType::LineChart,
        "<c:errBars><c:errBarType/><c:errValType/><c:plus><c:numLit/><c:numRef><c:f>Sheet1!B1</c:\
         f></c:numRef></c:plus></c:errBars>",
        "c",
    );
    witness("invalid-literal-first-choice", &book);
    assert!(matches!(
        series(&book).error_bars()[0].plus(),
        Some(NumericDataSource::Literal(_))
    ));
    assert!(matches!(
        series(&book).error_bars()[0].extra_plus(),
        [NumericDataSource::Reference(_)]
    ));
}
#[test]
fn family_limits_report_every_excess_occurrence_without_changing_storage() {
    for (kind, limit) in [
        (ChartType::BarChart, 1),
        (ChartType::Bar3DChart, 1),
        (ChartType::LineChart, 1),
        (ChartType::Line3DChart, 1),
        (ChartType::AreaChart, 2),
        (ChartType::Area3DChart, 2),
        (ChartType::ScatterChart, 2),
        (ChartType::BubbleChart, 2),
        (ChartType::PieChart, 0),
        (ChartType::Pie3DChart, 0),
        (ChartType::OfPieChart, 0),
        (ChartType::DoughnutChart, 0),
        (ChartType::RadarChart, 0),
    ] {
        let book = source(&kind, &DEFAULTS.repeat(3), "c");
        witness(&format!("family-{kind:?}"), &book);
        for stored in stored(&book) {
            assert_eq!(series(&stored).error_bars().len(), 3);
            assert_eq!(bars(&stored).len(), limit);
            let conflicts = chart(&stored)
                .chart_space()
                .chart()
                .plot_area()
                .series_part_conflicts();
            assert_eq!(conflicts.len(), 3 - limit);
            for (occurrence, c) in (limit..3).zip(conflicts) {
                assert_eq!(c.occurrence, occurrence);
                assert_eq!(
                    c.reason,
                    if limit == 0 {
                        SeriesPartConflictReason::NotAllowed
                    } else {
                        SeriesPartConflictReason::TooMany
                    }
                );
            }
        }
    }
}
#[test]
fn numeric_reference_source_cache_is_ignored_and_generated_from_cells() {
    let text = "<c:errBars><c:errBarType/><c:errValType/><c:minus><c:numRef><c:f>Sheet1!$B$1:\
                $B$3</c:f><c:numCache><c:ptCount val=\"1\"/><c:pt \
                idx=\"0\"><c:v>999999</c:v></c:pt></c:numCache></c:numRef></c:minus></c:errBars>";
    let book = source(&ChartType::LineChart, text, "c");
    witness("regenerated-reference", &book);
    let reference = series(&book).error_bars()[0]
        .minus()
        .unwrap()
        .reference()
        .unwrap();
    assert_eq!(reference.formula().address_str(), "Sheet1!$B$1:$B$3");
    let emitted = bars(&book).remove(0);
    assert!(!emitted.contains("999999"));
    assert!(emitted.contains("<c:v>10</c:v>"));
    assert!(emitted.contains("<c:v>30</c:v>"));
}
#[test]
fn reference_formula_text_regenerates_cells_after_cdata_entities_and_storage() {
    let text = concat!(
        "<c:errBars><c:errBarType/><c:errValType/><c:minus><c:numRef>",
        "<c:f><![CDATA[Sheet1!$B$1:$B$]]>&#51;</c:f>",
        "<c:numCache><c:ptCount val=\"1\"/><c:pt idx=\"0\"><c:v>999999</c:v>",
        "</c:pt></c:numCache></c:numRef></c:minus></c:errBars>",
    );
    for prefix in ["c", "z", ""] {
        let book = source(&ChartType::LineChart, text, prefix);
        let name = if prefix.is_empty() { "default" } else { prefix };
        witness(&format!("formula-text-{name}"), &book);
        for stored in stored(&book) {
            let reference = series(&stored).error_bars()[0]
                .minus()
                .unwrap()
                .reference()
                .unwrap();
            assert_eq!(reference.formula().address_str(), "Sheet1!$B$1:$B$3");
            let emitted = bars(&stored).remove(0);
            assert!(emitted.contains("<c:f>Sheet1!$B$1:$B$3</c:f>"));
            for value in [10, 20, 30] {
                assert!(emitted.contains(&format!("<c:v>{value}</c:v>")));
            }
            assert!(!emitted.contains("999999"));
            assert!(!emitted.contains("CDATA"));
            let reread = load(&save(&stored));
            assert_eq!(
                series(&reread).error_bars()[0]
                    .minus()
                    .unwrap()
                    .reference()
                    .unwrap()
                    .formula()
                    .address_str(),
                "Sheet1!$B$1:$B$3"
            );
        }
    }
}
#[test]
fn typed_literal_edit_is_authoritative_after_json_and_binary_reload() {
    let mut book = source(&ChartType::LineChart, LITERAL, "c");
    let item = &mut book.sheet_mut(0).unwrap().chart_collection_mut()[0]
        .chart_space_mut()
        .chart_mut()
        .plot_area_mut()
        .line_chart_mut()
        .unwrap()
        .area_chart_series_list_mut()
        .area_chart_series_mut()[0];
    let literal = item.error_bars_mut()[0]
        .plus_mut()
        .unwrap()
        .literal_mut()
        .unwrap();
    literal.points_mut()[0].set_value(Some("42".into()));
    witness("typed-literal", &book);
    for stored in stored(&book) {
        let emitted = bars(&stored).remove(0);
        assert!(emitted.contains("<c:v>42</c:v>"));
        assert!(!emitted.contains("<c:v>1.2500E+1</c:v>"));
        assert!(emitted.contains("<c:val val=\"1.2500E+1\"/>"));
    }
}

fn witness(name: &str, book: &Workbook) {
    if let Some(directory) = std::env::var_os("UMYA_ERROR_BAR_EXPORT_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        for (index, book) in stored(book).into_iter().enumerate() {
            std::fs::write(directory.join(format!("{name}-{index}.xlsx")), save(&book)).unwrap();
        }
    }
}

#[test]
fn populated_point_and_trend_supplements_load_without_an_error_bar_field() {
    use umya_spreadsheet::drawing::charts::SeriesParts;
    let literal = r#"{"data_points":[{"index":{"attribute":"9"},"invert_if_negative":null,"marker":null,"bubble_3d":null,"explosion":null,"shape_properties":null,"picture_options":null}],"trendlines":[{"name":"Kept","shape_properties":null,"type_element":true,"duplicate_children":false,"kind_attribute":null,"order":null,"period":null,"forward":null,"backward":null,"intercept":null,"display_r_squared":null,"display_equation":null,"label":null}]}"#;
    let parts: SeriesParts = serde_json::from_str(literal).unwrap();
    assert!(parts.error_bars().is_empty());
    assert_eq!(parts.data_points()[0].index().val(), Some(9));
    assert_eq!(parts.trendlines()[0].name(), Some("Kept"));
    let restored: SeriesParts = bincode::deserialize(&bincode::serialize(&parts).unwrap()).unwrap();
    assert!(restored.error_bars().is_empty());
    assert_eq!(restored.data_points()[0].index().val(), Some(9));
    assert_eq!(restored.trendlines()[0].name(), Some("Kept"));
}
#[test]
fn explicit_type_cleanup_keeps_original_target_occurrences_and_common_children() {
    use umya_spreadsheet::drawing::charts::{
        DataPoint,
        SeriesFamily,
        SeriesPartKind,
        Trendline,
    };
    let mut book = source(&ChartType::AreaChart, &DEFAULTS.repeat(3), "c");
    let list = book.sheet_mut(0).unwrap().chart_collection_mut()[0].area_chart_series_list_mut();
    let series = &mut list.area_chart_series_mut()[0];
    let mut point = DataPoint::default();
    point.index_mut().set_val(9);
    series.data_points_mut().push(point);
    let mut trend = Trendline::default();
    trend.set_name(Some("Kept".into()));
    series.trendlines_mut().push(trend);
    let removed = list.remove_parts_not_allowed_in(SeriesFamily::Line);
    assert_eq!(
        removed
            .iter()
            .map(|r| (r.part, r.occurrence))
            .collect::<Vec<_>>(),
        vec![
            (SeriesPartKind::ErrorBars, 1),
            (SeriesPartKind::ErrorBars, 2)
        ]
    );
    assert_eq!(list.area_chart_series()[0].error_bars().len(), 1);
    assert_eq!(
        list.area_chart_series()[0].data_points()[0].index().val(),
        Some(9)
    );
    assert_eq!(
        list.area_chart_series()[0].trendlines()[0].name(),
        Some("Kept")
    );
    let removed = list.remove_parts_not_allowed_in(SeriesFamily::Pie);
    assert_eq!(
        removed
            .iter()
            .filter(|r| r.part == SeriesPartKind::ErrorBars)
            .map(|r| r.occurrence)
            .collect::<Vec<_>>(),
        vec![0]
    );
    assert!(list.area_chart_series()[0].error_bars().is_empty());
    assert_eq!(
        list.area_chart_series()[0].data_points()[0].index().val(),
        Some(9)
    );
}
#[test]
fn shared_formula_owner_reaches_primary_and_conflicting_sources_but_not_literals() {
    let text = concat!(
        "<c:errBars><c:errBarType/><c:errValType/><c:plus><c:numRef><c:f>Sheet1!B1</c:f></c:\
         numRef><c:numLit><c:pt \
         idx=\"0\"><c:v>123</c:v></c:pt></c:numLit><c:numRef><c:f>Sheet1!B2</c:f></c:numRef></c:\
         plus><c:minus><c:numRef><c:f>Sheet1!B3</c:f></c:numRef><c:numRef><c:f>Sheet1!C1</c:f></c:\
         numRef></c:minus></c:errBars>"
    );
    let mut book = source(&ChartType::LineChart, text, "c");
    let item = &mut book.sheet_mut(0).unwrap().chart_collection_mut()[0]
        .area_chart_series_list_mut()
        .area_chart_series_mut()[0];
    let formulas = item.formula_mut();
    assert_eq!(
        formulas.iter().map(|f| f.address_str()).collect::<Vec<_>>(),
        vec!["Sheet1!B1", "Sheet1!B2", "Sheet1!B3", "Sheet1!C1"]
    );
    for (i, formula) in formulas.into_iter().enumerate() {
        formula.set_address_str(format!("Sheet1!D{}", i + 1));
    }
    let errors = &item.error_bars()[0];
    assert_eq!(
        errors.extra_plus()[0].literal().unwrap().points()[0].value(),
        Some("123")
    );
    assert_eq!(
        errors.conflict(),
        Some(SeriesPartConflictReason::ConflictingChoice)
    );
    for restored in stored(&book) {
        let item = &series(&restored).error_bars()[0];
        assert_eq!(
            item.plus()
                .unwrap()
                .reference()
                .unwrap()
                .formula()
                .address_str(),
            "Sheet1!D1"
        );
        assert_eq!(
            item.extra_minus()[0]
                .reference()
                .unwrap()
                .formula()
                .address_str(),
            "Sheet1!D4"
        );
        assert!(bars(&restored).is_empty());
    }
}
#[test]
fn invalid_first_occurrences_do_not_promote_later_valid_error_bars_into_the_limit() {
    use umya_spreadsheet::drawing::charts::SeriesPartKind;
    let book = source(
        &ChartType::LineChart,
        &format!("<c:errBars><c:errValType/></c:errBars>{DEFAULTS}"),
        "c",
    );
    for restored in stored(&book) {
        let conflicts = chart(&restored)
            .chart_space()
            .chart()
            .plot_area()
            .series_part_conflicts();
        assert_eq!(
            conflicts
                .iter()
                .map(|c| (c.part, c.occurrence, c.reason))
                .collect::<Vec<_>>(),
            vec![
                (
                    SeriesPartKind::ErrorBars,
                    0,
                    SeriesPartConflictReason::MissingRequiredChild
                ),
                (
                    SeriesPartKind::ErrorBars,
                    1,
                    SeriesPartConflictReason::TooMany
                )
            ]
        );
        assert!(bars(&restored).is_empty());
        assert_eq!(series(&restored).error_bars().len(), 2);
    }
    witness("original-ordinal-limit", &book);
}
#[test]
fn every_error_bar_enum_spelling_remains_distinct_through_persistence() {
    use umya_spreadsheet::drawing::charts::{
        ErrorBars,
        ScalarElement,
    };
    let mut value = ErrorBars::default();
    assert_eq!(value.bar_type_value(), None);
    assert_eq!(value.value_type_value(), None);
    let mut bar_type = ScalarElement::default();
    bar_type.set_attribute(Some(ErrorBarType::Minus));
    value.set_bar_type(bar_type);
    value.set_value_type(ScalarElement::default());
    assert_eq!(value.bar_type_value(), Some(ErrorBarType::Minus));
    assert_eq!(
        value.value_type_value(),
        Some(ErrorBarValueType::FixedValue)
    );

    for direction in ["x", "y"] {
        for bar_type in ["both", "minus", "plus"] {
            for value_type in ["cust", "fixedVal", "percentage", "stdDev", "stdErr"] {
                let input = format!(
                    "<c:errBars><c:errDir val=\"{direction}\"/><c:errBarType \
                     val=\"{bar_type}\"/><c:errValType val=\"{value_type}\"/><c:noEndCap val=\" \
                     false \"/><c:val val=\" 1.2500E+1 \"/></c:errBars>"
                );
                let book = source(&ChartType::LineChart, &input, "c");
                for restored in stored(&book) {
                    assert_eq!(bars(&restored), vec![input.clone()]);
                    assert_eq!(
                        series(&restored).error_bars()[0]
                            .no_end_cap()
                            .unwrap()
                            .value(),
                        Some(false)
                    );
                    assert_eq!(
                        series(&restored).error_bars()[0].value().unwrap().value(),
                        Some(12.5)
                    );
                }
                witness(&format!("enum-{direction}-{bar_type}-{value_type}"), &book);
            }
        }
    }
}

#[test]
fn actual_workbook_preserves_fixed_error_bar_shape_scalars_and_persistence() {
    use umya_spreadsheet::drawing::charts::{
        ErrorBarDirection,
        ErrorBarType,
        ErrorBarValueType,
    };
    let book = load(include_bytes!("fixtures/chart_error_bars.xlsx"));
    let item = chart(&book)
        .chart_space()
        .chart()
        .plot_area()
        .chart_groups()
        .flat_map(|g| g.series().area_chart_series())
        .find(|s| !s.error_bars().is_empty())
        .expect("the fifth source series owns the error bar");
    assert_eq!(item.error_bars().len(), 1);
    let error = &item.error_bars()[0];
    assert!(matches!(
        error.direction().unwrap().attribute(),
        Some(ErrorBarDirection::X)
    ));
    assert_eq!(error.bar_type_value(), Some(ErrorBarType::Minus));
    assert_eq!(
        error.value_type_value(),
        Some(ErrorBarValueType::FixedValue)
    );
    assert_eq!(error.no_end_cap().unwrap().attribute(), Some("1"));
    assert_eq!(error.value().unwrap().attribute(), Some("10"));
    assert_eq!(
        error.shape_properties().unwrap().outline().unwrap().width(),
        9525
    );
    for restored in stored(&book) {
        let emitted = bars(&restored);
        assert_eq!(emitted.len(), 1);
        assert!(
            emitted[0].contains("<a:schemeClr val=\"bg1\"><a:lumMod val=\"65000\"/></a:schemeClr>")
        );
        assert!(emitted[0].contains("<a:noFill/>"));
        assert!(emitted[0].contains("<a:round/>"));
        assert!(emitted[0].contains("<a:effectLst/>"));
        assert!(emitted[0].contains("<c:val val=\"10\"/>"));
    }
    witness("real-error-bars", &book);
}
#[test]
fn expanded_error_bar_and_literal_leaves_match_empty_forms_across_namespaces() {
    let original = format!("{DEFAULTS}{LITERAL}");
    let mut reader = quick_xml::Reader::from_str(&original);
    let mut expanded = String::new();
    loop {
        match reader.read_event().unwrap() {
            quick_xml::events::Event::Empty(e) => {
                let name = std::str::from_utf8(e.name().into_inner())
                    .unwrap()
                    .to_owned();
                expanded.push('<');
                expanded.push_str(std::str::from_utf8(e.as_ref()).unwrap());
                expanded.push('>');
                expanded.push_str(&format!("</{name}>"));
            }
            quick_xml::events::Event::Eof => break,
            event => {
                let mut writer = quick_xml::Writer::new(Vec::new());
                writer.write_event(event).unwrap();
                expanded.push_str(&String::from_utf8(writer.into_inner()).unwrap());
            }
        }
    }
    for prefix in ["c", "z", ""] {
        let book = source(&ChartType::AreaChart, &expanded, prefix);
        for restored in stored(&book) {
            assert_eq!(
                bars(&restored),
                vec![DEFAULTS.to_owned(), LITERAL.to_owned()]
            );
        }
        witness(
            &format!(
                "expanded-{}",
                if prefix.is_empty() { "default" } else { prefix }
            ),
            &book,
        );
    }
}

#[test]
fn literal_text_preserves_cdata_entities_and_significant_spaces_once() {
    let input = concat!(
        "<c:errBars><c:errBarType/><c:errValType/><c:plus><c:numLit>",
        "<c:formatCode>  <![CDATA[0.00 & [x]]]>&amp; &#x41;  </c:formatCode>",
        "<c:pt idx=\"0\"><c:v>  <![CDATA[<x>&raw]]>&amp; &#65; &amp;lt;  </c:v></c:pt>",
        "</c:numLit></c:plus></c:errBars>"
    );
    for prefix in ["c", "z", ""] {
        let book = source(&ChartType::LineChart, input, prefix);
        for restored in stored(&book) {
            let literal = series(&restored).error_bars()[0]
                .plus()
                .unwrap()
                .literal()
                .unwrap();
            assert_eq!(literal.format_code(), Some("  0.00 & [x]& A  "));
            assert_eq!(literal.points()[0].value(), Some("  <x>&raw& A &lt;  "));
            let emitted = bars(&restored).remove(0);
            assert!(emitted.contains("<c:formatCode>  0.00 &amp; [x]&amp; A  </c:formatCode>"));
            assert!(emitted.contains("<c:v>  &lt;x&gt;&amp;raw&amp; A &amp;lt;  </c:v>"));
            let reloaded = load(&save(&restored));
            assert_eq!(bars(&reloaded), bars(&restored));
        }
        witness(
            &format!(
                "literal-text-{}",
                if prefix.is_empty() { "default" } else { prefix }
            ),
            &book,
        );
    }
}
