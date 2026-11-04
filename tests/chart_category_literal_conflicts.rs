//! Category choices report invalid literals without changing valid native data.
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
            SeriesPartConflictReason,
            SeriesPartKind,
        },
        spreadsheet::MarkerType,
    },
};

fn save(book: &Workbook) -> Vec<u8> {
    let mut bytes = Vec::new();
    umya_spreadsheet::writer::xlsx::write_writer(book, &mut bytes).unwrap();
    bytes
}

fn source(kind: ChartType, categories: &str) -> Workbook {
    let mut book = umya_spreadsheet::new_file();
    for row in 1..=3 {
        book.sheet_mut(0)
            .unwrap()
            .cell_mut((1, row))
            .set_value_string(format!("row {row}"));
        book.sheet_mut(0)
            .unwrap()
            .cell_mut((2, row))
            .set_value_number(row * 10);
    }
    let mut from = MarkerType::default();
    from.set_coordinate("E1");
    let mut to = MarkerType::default();
    to.set_coordinate("M20");
    let scatter = matches!(kind, ChartType::ScatterChart);
    let mut chart = Chart::default();
    chart.new_chart(
        &kind,
        from,
        to,
        if scatter {
            vec!["Sheet1!B1:B3"; 2]
        } else {
            vec!["Sheet1!B1:B3"]
        },
    );
    book.sheet_mut(0).unwrap().add_chart(chart);
    let bytes = save(&book);
    let mut input = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for index in 0..input.len() {
        let mut file = input.by_index(index).unwrap();
        let name = file.name().unwrap().into_owned();
        let mut data = Vec::new();
        file.read_to_end(&mut data).unwrap();
        if name == "xl/charts/chart1.xml" {
            let mut xml = String::from_utf8(data).unwrap();
            let start = xml.find("<c:ser>").unwrap();
            let end = start + xml[start..].find("</c:ser>").unwrap() + "</c:ser>".len();
            let values = if scatter {
                "<c:xVal><c:numRef><c:f>Sheet1!$B$1:$B$3</c:f></c:numRef></c:xVal><c:yVal><c:\
                 numRef><c:f>Sheet1!$B$1:$B$3</c:f></c:numRef></c:yVal>"
            } else {
                "<c:val><c:numRef><c:f>Sheet1!$B$1:$B$3</c:f></c:numRef></c:val>"
            };
            xml.replace_range(
                start..end,
                &format!(
                    "<c:ser><c:idx val=\"7\"/><c:order val=\"8\"/>{categories}{values}</c:ser>"
                ),
            );
            data = xml.into_bytes();
        }
        output
            .start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        output.write_all(&data).unwrap();
    }
    umya_spreadsheet::reader::xlsx::read_reader(
        Cursor::new(output.finish().unwrap().into_inner()),
        true,
    )
    .unwrap()
}

fn chart(book: &Workbook) -> &Chart {
    book.sheet_collection()
        .iter()
        .flat_map(|sheet| sheet.chart_collection())
        .next()
        .unwrap()
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

fn stored(book: &Workbook) -> Vec<Workbook> {
    let json = serde_json::from_slice::<Chart>(&serde_json::to_vec(chart(book)).unwrap()).unwrap();
    let binary = bincode::deserialize::<Chart>(&bincode::serialize(chart(book)).unwrap()).unwrap();
    let mut copies = vec![book.clone()];
    for model in [json, binary] {
        let mut copy = book.clone();
        *copy
            .sheet_collection_mut()
            .iter_mut()
            .flat_map(|sheet| sheet.chart_collection_mut())
            .next()
            .unwrap() = model;
        copies.push(copy);
    }
    copies
}

fn assert_conflict(categories: &str, reason: SeriesPartConflictReason) {
    for book in stored(&source(ChartType::BarChart, categories)) {
        let conflicts = chart(&book)
            .chart_space()
            .chart()
            .plot_area()
            .series_part_conflicts();
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].part, SeriesPartKind::Categories);
        assert_eq!(conflicts[0].occurrence, 0);
        assert_eq!(conflicts[0].reason, reason);
    }
}

#[test]
fn category_literal_missing_point_value_reports_missing_required_child() {
    assert_conflict(
        "<c:cat><c:strLit><c:ptCount val=\"1\"/><c:pt idx=\"0\"/></c:strLit></c:cat>",
        SeriesPartConflictReason::MissingRequiredChild,
    );
}

#[test]
fn category_literal_invalid_point_index_reports_invalid_value() {
    assert_conflict(
        "<c:cat><c:strLit><c:ptCount val=\"1\"/><c:pt \
         idx=\"x\"><c:v>label</c:v></c:pt></c:strLit></c:cat>",
        SeriesPartConflictReason::InvalidValue,
    );
}

#[test]
fn category_literal_duplicate_count_reports_invalid_value() {
    assert_conflict(
        "<c:cat><c:strLit><c:ptCount val=\"1\"/><c:ptCount val=\"1\"/><c:pt \
         idx=\"0\"><c:v>label</c:v></c:pt></c:strLit></c:cat>",
        SeriesPartConflictReason::InvalidValue,
    );
}

#[test]
fn empty_category_choice_reports_missing_required_child() {
    assert_conflict(
        "<c:cat></c:cat>",
        SeriesPartConflictReason::MissingRequiredChild,
    );
}

#[test]
fn empty_category_wrappers_keep_presence_in_both_xml_forms() {
    for xml in ["<c:cat/>", "<c:cat></c:cat>"] {
        assert_conflict(xml, SeriesPartConflictReason::MissingRequiredChild);
        let book = source(ChartType::BarChart, xml);
        assert!(series(&book).category_axis_data().is_some());
    }
}

#[test]
fn empty_category_reference_choices_keep_their_native_choice_in_both_xml_forms() {
    for (index, name) in ["strRef", "numRef", "multiLvlStrRef"]
        .into_iter()
        .enumerate()
    {
        for child in [format!("<c:{name}/>"), format!("<c:{name}></c:{name}>")] {
            for book in stored(&source(
                ChartType::BarChart,
                &format!("<c:cat>{child}</c:cat>"),
            )) {
                let categories = series(&book).category_axis_data().unwrap();
                assert_eq!(categories.string_reference().is_some(), index == 0);
                assert_eq!(categories.number_reference().is_some(), index == 1);
                assert_eq!(
                    categories.multi_level_string_reference().is_some(),
                    index == 2
                );
                assert!(categories.string_literal().is_none());
                assert_eq!(
                    series(&book)
                        .values()
                        .unwrap()
                        .number_reference()
                        .formula()
                        .address_str(),
                    "Sheet1!$B$1:$B$3"
                );
            }
        }
    }
}

#[test]
fn empty_numeric_series_wrappers_keep_presence_without_consuming_following_values() {
    for (kind, name) in [
        (ChartType::ScatterChart, "val"),
        (ChartType::BarChart, "xVal"),
        (ChartType::BarChart, "yVal"),
        (ChartType::BarChart, "bubbleSize"),
    ] {
        for xml in [format!("<c:{name}/>"), format!("<c:{name}></c:{name}>")] {
            for book in stored(&source(kind, &xml)) {
                let series = series(&book);
                let reference = match name {
                    "val" => series.values().unwrap().number_reference(),
                    "xVal" => series.x_values().unwrap().number_reference(),
                    "yVal" => series.y_values().unwrap().number_reference(),
                    "bubbleSize" => series.bubble_size().unwrap().number_reference(),
                    _ => unreachable!(),
                };
                assert!(reference.formula().address_str().is_empty());
                let following = if name == "val" {
                    series.x_values().unwrap().number_reference()
                } else {
                    series.values().unwrap().number_reference()
                };
                assert_eq!(following.formula().address_str(), "Sheet1!$B$1:$B$3");
            }
        }
    }
}

#[test]
fn multiple_modeled_category_choices_report_conflicting_choice() {
    assert_conflict(
        "<c:cat><c:strRef><c:f>Sheet1!$A$1:$A$3</c:f></c:strRef><c:strLit><c:ptCount \
         val=\"0\"/></c:strLit></c:cat>",
        SeriesPartConflictReason::ConflictingChoice,
    );
}

#[test]
fn foreign_category_wrappers_do_not_promote_nested_native_literals() {
    let book = source(
        ChartType::BarChart,
        "<c:cat><c:strRef><c:f>Sheet1!$A$1:$A$3</c:f></c:strRef><x:future \
         xmlns:x=\"urn:foreign-category\"><c:strLit><c:ptCount val=\"1\"/><c:pt \
         idx=\"0\"><c:v>poison</c:v></c:pt></c:strLit></x:future></c:cat>",
    );
    let categories = series(&book).category_axis_data().unwrap();
    assert!(categories.string_literal().is_none());
    assert_eq!(
        categories
            .string_reference()
            .unwrap()
            .formula()
            .address_str(),
        "Sheet1!$A$1:$A$3"
    );
}

#[test]
fn unsupported_category_family_reports_only_not_allowed() {
    let book = source(
        ChartType::ScatterChart,
        "<c:cat><c:strLit><c:pt idx=\"0\"/></c:strLit></c:cat>",
    );
    let conflicts = chart(&book)
        .chart_space()
        .chart()
        .plot_area()
        .series_part_conflicts();
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].part, SeriesPartKind::Categories);
    assert_eq!(conflicts[0].reason, SeriesPartConflictReason::NotAllowed);
}

#[test]
fn valid_empty_category_literal_preserves_its_native_choice_and_absent_count() {
    for book in stored(&source(ChartType::BarChart, "<c:cat><c:strLit/></c:cat>")) {
        let literal = series(&book)
            .category_axis_data()
            .unwrap()
            .string_literal()
            .unwrap();
        assert!(literal.point_count().is_none());
        assert!(literal.string_point_list().is_empty());
        assert!(
            chart(&book)
                .chart_space()
                .chart()
                .plot_area()
                .series_part_conflicts()
                .is_empty()
        );
        let bytes = save(&book);
        let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        let mut xml = String::new();
        zip.by_name("xl/charts/chart1.xml")
            .unwrap()
            .read_to_string(&mut xml)
            .unwrap();
        assert!(xml.contains("<c:cat><c:strLit></c:strLit></c:cat>"));
    }
}

#[test]
fn valid_sparse_repeated_category_literals_retain_count_presence_and_point_order() {
    for count in ["", "<c:ptCount val=\"0004\"/>"] {
        let literal = format!(
            "<c:strLit>{count}<c:pt idx=\"9\"><c:v>nine</c:v></c:pt><c:pt \
             idx=\"2\"><c:v>two</c:v></c:pt><c:pt idx=\"9\"><c:v>repeat</c:v></c:pt></c:strLit>"
        );
        for book in stored(&source(
            ChartType::BarChart,
            &format!("<c:cat>{literal}</c:cat>"),
        )) {
            assert!(
                chart(&book)
                    .chart_space()
                    .chart()
                    .plot_area()
                    .series_part_conflicts()
                    .is_empty()
            );
            let categories = series(&book)
                .category_axis_data()
                .unwrap()
                .string_literal()
                .unwrap();
            assert_eq!(categories.point_count().is_some(), !count.is_empty());
            assert_eq!(
                categories
                    .string_point_list()
                    .iter()
                    .map(|point| (
                        point.index().unwrap().val().unwrap(),
                        point.numeric_value().text()
                    ))
                    .collect::<Vec<_>>(),
                vec![(9, "nine"), (2, "two"), (9, "repeat")]
            );
            let bytes = save(&book);
            let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
            let mut xml = String::new();
            zip.by_name("xl/charts/chart1.xml")
                .unwrap()
                .read_to_string(&mut xml)
                .unwrap();
            assert!(xml.contains(&format!("<c:cat>{literal}</c:cat>")), "{xml}");
        }
    }
}

#[test]
fn valid_reference_and_hierarchical_categories_still_export() {
    for child in [
        "<c:strRef><c:f>Sheet1!$A$1:$A$3</c:f></c:strRef>",
        "<c:numRef><c:f>Sheet1!$B$1:$B$3</c:f></c:numRef>",
        "<c:multiLvlStrRef><c:f>Sheet1!$A$1:$B$3</c:f><c:multiLvlStrCache><c:ptCount \
         val=\"2\"/><c:lvl><c:pt idx=\"9\"><c:v>outer</c:v></c:pt></c:lvl><c:lvl><c:pt \
         idx=\"2\"><c:v>inner</c:v></c:pt></c:lvl></c:multiLvlStrCache></c:multiLvlStrRef>",
    ] {
        for book in stored(&source(
            ChartType::BarChart,
            &format!("<c:cat>{child}</c:cat>"),
        )) {
            assert!(
                chart(&book)
                    .chart_space()
                    .chart()
                    .plot_area()
                    .series_part_conflicts()
                    .is_empty()
            );
            let bytes = save(&book);
            let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
            let mut xml = String::new();
            zip.by_name("xl/charts/chart1.xml")
                .unwrap()
                .read_to_string(&mut xml)
                .unwrap();
            assert!(xml.contains("<c:cat>"));
            if child.contains("multiLvlStrRef") {
                assert!(xml.contains(child));
            } else {
                assert!(xml.contains(if child.contains("<c:numRef>") {
                    "<c:cat><c:numRef>"
                } else {
                    "<c:cat><c:strRef>"
                }));
            }
        }
    }
}
