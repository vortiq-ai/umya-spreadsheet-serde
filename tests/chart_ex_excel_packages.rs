#![cfg(feature = "serde")]
//! Package-level scenarios for created and moved extended charts. Each test
//! asserts the package structure and, when `CARGO_TARGET_DIR` is set, leaves
//! the package under `chart-ex-validation/` for `chart_ex_excel_open.ps1`,
//! which opens every package in Excel and checks what Excel bound.
use std::{
    collections::BTreeSet,
    io::{
        Cursor,
        Read,
        Write,
    },
    path::PathBuf,
};

use umya_spreadsheet::{
    Workbook,
    drawing::{
        charts::*,
        spreadsheet::{
            DrawingAnchor,
            ExtendedChart,
        },
    },
    reader,
    writer,
};

const PARETO: &[u8] = include_bytes!("fixtures/extended_charts/creation-pareto.xlsx");

fn formula(text: &str) -> ChartExFormula {
    ChartExFormula::new(text)
}

fn emit(name: &str, bytes: &[u8]) {
    if let Some(target) = std::env::var_os("CARGO_TARGET_DIR") {
        let directory = PathBuf::from(target).join("chart-ex-validation");
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join(format!("{name}.xlsx")), bytes).unwrap();
    }
}

fn entry(bytes: &[u8], name: &str) -> Vec<u8> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut data = Vec::new();
    archive
        .by_name(name)
        .unwrap()
        .read_to_end(&mut data)
        .unwrap();
    data
}

fn text(bytes: &[u8], name: &str) -> String {
    String::from_utf8(entry(bytes, name)).unwrap()
}

fn names(bytes: &[u8]) -> Vec<String> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    (0..archive.len())
        .map(|index| {
            archive
                .by_index_raw(index)
                .unwrap()
                .name()
                .unwrap()
                .into_owned()
        })
        .collect()
}

fn write_book(book: &Workbook) -> Vec<u8> {
    let mut bytes = Vec::new();
    writer::xlsx::write_writer(book, &mut bytes).unwrap();
    bytes
}

fn data_book() -> Workbook {
    let mut book = umya_spreadsheet::new_file();
    for row in 1..=4 {
        let sheet = book.sheet_mut(0).unwrap();
        sheet.cell_mut((1, row)).set_value(format!("Group {row}"));
        sheet.cell_mut((2, row)).set_value(format!("Leaf {row}"));
        sheet.cell_mut((3, row)).set_value_number(row * 10);
        // A second numeric column: the second box-and-whisker group.
        sheet.cell_mut((4, row)).set_value_number(row * 10 - 5);
    }
    book
}

/// A chart Excel can draw: a real size, and each chart of a sheet beside the
/// previous one so none covers another.
fn add_chart(book: &mut Workbook, sheet: usize, chart: ChartExSpace) {
    let drawing = book.sheet_mut(sheet).unwrap().worksheet_drawing_mut();
    let slot = u32::try_from(drawing.extended_charts().len()).unwrap();
    let mut from = umya_spreadsheet::drawing::spreadsheet::MarkerType::default();
    from.set_col(5 + slot * 8).set_row(1);
    let mut extent = umya_spreadsheet::drawing::spreadsheet::Extent::default();
    extent.set_cx(4_500_000).set_cy(3_000_000);
    drawing.add_extended_chart(ExtendedChart::new(
        DrawingAnchor::OneCell { from, extent },
        chart,
    ));
}

/// Every chart relationship part has exactly one style and one colour edge,
/// every edge points at an existing part, and no style or colour part is
/// left without a chart that refers to it.
fn assert_style_graph(bytes: &[u8]) {
    let all = names(bytes);
    let mut referenced = BTreeSet::new();
    let mut charts = 0;
    for name in all
        .iter()
        .filter(|name| name.starts_with("xl/charts/_rels/chartEx") && name.ends_with(".xml.rels"))
    {
        charts += 1;
        let rels = text(bytes, name);
        let edges: Vec<&str> = rels.split("<Relationship ").skip(1).collect();
        for kind in ["chartStyle", "chartColorStyle"] {
            let matching: Vec<_> = edges
                .iter()
                .filter(|edge| {
                    edge.contains(&format!(
                        "Type=\"http://schemas.microsoft.com/office/2011/relationships/{kind}\""
                    ))
                })
                .collect();
            assert_eq!(matching.len(), 1, "{name} needs one {kind} edge: {rels}");
            let target = matching[0]
                .split_once("Target=\"")
                .unwrap()
                .1
                .split_once('"')
                .unwrap()
                .0;
            let part = format!("xl/charts/{target}");
            assert!(all.contains(&part), "{name} targets missing {part}");
            referenced.insert(part);
        }
    }
    assert!(charts > 0);
    let present: BTreeSet<String> = all
        .into_iter()
        .filter(|name| {
            (name.starts_with("xl/charts/style") || name.starts_with("xl/charts/colors"))
                && !name.contains("_rels")
        })
        .collect();
    assert_eq!(present, referenced, "orphaned or missing style parts");
    let types = text(bytes, "[Content_Types].xml");
    for part in &present {
        let content_type = if part.contains("style") {
            "application/vnd.ms-office.chartstyle+xml"
        } else {
            "application/vnd.ms-office.chartcolorstyle+xml"
        };
        assert!(
            types.contains(&format!(
                "PartName=\"/{part}\" ContentType=\"{content_type}\""
            )),
            "{part} lacks its content type"
        );
    }
}

fn rewrite(bytes: &[u8], mut change: impl FnMut(&str, Vec<u8>) -> Option<Vec<u8>>) -> Vec<u8> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for index in 0..archive.len() {
        let mut file = archive.by_index(index).unwrap();
        let name = file.name().unwrap().into_owned();
        let mut data = Vec::new();
        file.read_to_end(&mut data).unwrap();
        if let Some(data) = change(&name, data) {
            output
                .start_file(&name, zip::write::SimpleFileOptions::default())
                .unwrap();
            output.write_all(&data).unwrap();
        }
    }
    output.finish().unwrap().into_inner()
}

fn imported_pareto() -> Workbook {
    reader::xlsx::read_reader(Cursor::new(PARETO), true).unwrap()
}

#[test]
fn an_imported_chart_moved_to_another_sheet_keeps_its_style_dependencies() {
    let mut book = imported_pareto();
    book.new_sheet("Moved").unwrap();
    let chart = book
        .sheet_mut(0)
        .unwrap()
        .worksheet_drawing_mut()
        .extended_charts_mut()
        .remove(0);
    book.sheet_mut(1)
        .unwrap()
        .worksheet_drawing_mut()
        .add_extended_chart(chart);
    let bytes = write_book(&book);
    assert_style_graph(&bytes);
    let charts = names(&bytes)
        .iter()
        .filter(|name| name.starts_with("xl/charts/chartEx"))
        .count();
    assert_eq!(charts, 1);
    emit("moved-sheet", &bytes);
    let reread = reader::xlsx::read_reader(Cursor::new(&bytes), true).unwrap();
    assert_eq!(
        reread
            .sheet(1)
            .unwrap()
            .worksheet_drawing()
            .extended_charts()
            .len(),
        1
    );
    // Saving the moved package again keeps the graph consistent.
    assert_style_graph(&write_book(&reread));
}

#[test]
fn an_imported_chart_moved_to_a_new_workbook_keeps_its_style_dependencies() {
    let mut source = imported_pareto();
    let chart = source
        .sheet_mut(0)
        .unwrap()
        .worksheet_drawing_mut()
        .extended_charts_mut()
        .remove(0);
    let mut fresh = umya_spreadsheet::new_file();
    // The reader keeps the workbook names on the sheet they were written for.
    let definitions: Vec<_> = source.sheet(0).unwrap().defined_names().to_vec();
    assert_eq!(definitions.len(), 1);
    for definition in definitions {
        fresh.add_defined_names(definition);
    }
    for row in 4..=13 {
        fresh
            .sheet_mut(0)
            .unwrap()
            .cell_mut((3, row))
            .set_value_number(row);
    }
    fresh
        .sheet_mut(0)
        .unwrap()
        .worksheet_drawing_mut()
        .add_extended_chart(chart);
    let bytes = write_book(&fresh);
    assert_style_graph(&bytes);
    emit("moved-workbook", &bytes);
    // The source workbook no longer has the chart and must not keep its
    // style parts behind.
    let remaining = write_book(&source);
    assert!(
        !names(&remaining)
            .iter()
            .any(|name| name.starts_with("xl/charts/style") || name.starts_with("xl/charts/colors")),
        "{:?}",
        names(&remaining)
    );
}

#[test]
fn a_package_saved_without_style_parts_regains_them_when_written_again() {
    let mut book = data_book();
    add_chart(
        &mut book,
        0,
        ChartExBuilder::sunburst(formula("Sheet1!$A$1:$B$4"), formula("Sheet1!$C$1:$C$4"))
            .title("Sales & costs")
            .series_name("Revenue")
            .build()
            .unwrap(),
    );
    let current = write_book(&book);
    // The shape this library wrote before it added the parts.
    let legacy = rewrite(&current, |name, data| {
        if name == "xl/charts/style1.xml"
            || name == "xl/charts/colors1.xml"
            || name == "xl/charts/_rels/chartEx1.xml.rels"
        {
            return None;
        }
        if name == "[Content_Types].xml" {
            let types = String::from_utf8(data).unwrap();
            let mut kept = String::new();
            let mut rest = types.as_str();
            while let Some(start) = rest.find("<Override ") {
                kept.push_str(&rest[..start]);
                let end = rest[start..].find("/>").unwrap() + start + 2;
                let element = &rest[start..end];
                if !element.contains("/xl/charts/style1.xml")
                    && !element.contains("/xl/charts/colors1.xml")
                {
                    kept.push_str(element);
                }
                rest = &rest[end..];
            }
            kept.push_str(rest);
            return Some(kept.into_bytes());
        }
        Some(data)
    });
    assert!(!names(&legacy).iter().any(|name| name.contains("style1")));
    let reread = reader::xlsx::read_reader(Cursor::new(&legacy), true).unwrap();
    let resaved = write_book(&reread);
    assert_style_graph(&resaved);
    assert_eq!(
        entry(&resaved, "xl/charts/style1.xml"),
        entry(&current, "xl/charts/style1.xml")
    );
    emit("legacy-resaved", &resaved);
    // A normal re-save of the current output stays stable.
    let again = write_book(&reader::xlsx::read_reader(Cursor::new(&current), true).unwrap());
    assert_style_graph(&again);
    emit("resaved", &again);
}

#[test]
fn a_created_chart_in_an_imported_workbook_gets_its_own_numbered_parts() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/extended_charts/creation-pareto.xlsx"
    );
    for (eager, label) in [
        (true, "imported-plus-created"),
        (false, "imported-plus-created-lazy"),
    ] {
        let mut book = if eager {
            imported_pareto()
        } else {
            let mut book = reader::xlsx::lazy_read(std::path::Path::new(path)).unwrap();
            book.try_read_sheet(0).unwrap();
            book
        };
        add_chart(
            &mut book,
            0,
            ChartExBuilder::funnel(formula("Sheet1!$B$4:$B$13"), formula("Sheet1!$C$4:$C$13"))
                .title("Funnel")
                .build()
                .unwrap(),
        );
        let bytes = write_book(&book);
        assert_style_graph(&bytes);
        // The imported chart keeps Excel's own parts byte for byte; the new
        // chart gets the next numbers and the funnel defaults (style 419).
        assert_eq!(
            entry(&bytes, "xl/charts/style1.xml"),
            entry(PARETO, "xl/charts/style1.xml")
        );
        assert!(text(&bytes, "xl/charts/style2.xml").contains(" id=\"419\">"));
        assert!(
            names(&bytes)
                .iter()
                .any(|name| name == "xl/charts/colors2.xml")
        );
        emit(label, &bytes);
    }
}

#[test]
fn two_created_charts_in_one_sheet_have_separate_parts_and_namespaces() {
    let mut book = data_book();
    add_chart(
        &mut book,
        0,
        ChartExBuilder::funnel(formula("Sheet1!$A$1:$A$4"), formula("Sheet1!$C$1:$C$4"))
            .title("Funnel")
            .build()
            .unwrap(),
    );
    add_chart(
        &mut book,
        0,
        ChartExBuilder::treemap(
            formula("Sheet1!$A$1:$B$4"),
            formula("Sheet1!$C$1:$C$4"),
            ChartExParentLabelLayout::Overlapping,
        )
        .title("Treemap")
        .build()
        .unwrap(),
    );
    let bytes = write_book(&book);
    assert_style_graph(&bytes);
    let drawing = text(&bytes, "xl/drawings/drawing1.xml");
    assert!(drawing.contains("Requires=\"cx2\""));
    assert!(drawing.contains("Requires=\"cx1\""));
    emit("two-charts", &bytes);
}

#[test]
fn named_creation_matches_excel_naming_across_charts_and_kinds() {
    let mut book = data_book();
    for chart in [
        ChartExBuilder::sunburst(formula("Sheet1!$A$1:$B$4"), formula("Sheet1!$C$1:$C$4")),
        ChartExBuilder::funnel(formula("Sheet1!$A$1:$A$4"), formula("Sheet1!$C$1:$C$4")),
        ChartExBuilder::region_map(
            formula("Sheet1!$A$1:$A$4"),
            formula("Sheet1!$C$1:$C$4"),
            "en-US",
            "US",
        ),
    ] {
        let space = chart
            .title("Named")
            .series_name("Revenue")
            .build_with_defined_names(&mut book)
            .unwrap();
        add_chart(&mut book, 0, space);
    }
    // One workbook-wide counter; the version follows the chart kind (v1 for
    // the original kinds, v2 funnel, v5 region map), as Excel numbers them.
    let defined: Vec<String> = book
        .defined_names()
        .iter()
        .map(|name| name.name().to_owned())
        .collect();
    assert_eq!(
        defined,
        [
            "_xlchart.v1.0",
            "_xlchart.v1.1",
            "_xlchart.v2.2",
            "_xlchart.v2.3",
            "_xlchart.v5.4",
            "_xlchart.v5.5"
        ]
    );
    let bytes = write_book(&book);
    assert_style_graph(&bytes);
    let workbook = text(&bytes, "xl/workbook.xml");
    assert!(workbook.contains("_xlchart.v5.5"));
    emit("defined-names", &bytes);
}

#[test]
fn histogram_bins_are_written_in_the_form_excel_reads() {
    let values = [1, 3, 4, 8, 9, 12, 15, 21, 22, 30];
    let build = |binning: ChartExBinning| {
        let mut book = umya_spreadsheet::new_file();
        for (row, value) in (1..).zip(values) {
            book.sheet_mut(0)
                .unwrap()
                .cell_mut((1, row))
                .set_value_number(value);
        }
        add_chart(
            &mut book,
            0,
            ChartExBuilder::histogram(formula("Sheet1!$A$1:$A$10"), binning)
                .title("Bins")
                .build()
                .unwrap(),
        );
        write_book(&book)
    };
    let by_count = build(ChartExBinning {
        bin_count: Some(4),
        ..Default::default()
    });
    assert!(text(&by_count, "xl/charts/chartEx1.xml").contains("<cx:binCount val=\"4\""));
    emit("histogram-bin-count", &by_count);
    let mut size = XmlDouble::default();
    size.set_value(10.0);
    let by_size = build(ChartExBinning {
        bin_size: Some(size),
        ..Default::default()
    });
    assert!(text(&by_size, "xl/charts/chartEx1.xml").contains("<cx:binSize val=\"10\""));
    emit("histogram-bin-size", &by_size);
    // Excel rejects the element-text spelling of the same value; the control
    // package pins that, so the attribute form cannot be "simplified".
    let text_form = rewrite(&by_count, |name, data| {
        if name == "xl/charts/chartEx1.xml" {
            let xml = String::from_utf8(data).unwrap();
            let start = xml.find("<cx:binCount val=\"4\"").unwrap();
            let end = xml[start..].find("/>").map(|end| start + end + 2).unwrap();
            let end_tag = xml[start..].find("</cx:binCount>");
            let end = end_tag.map_or(end, |tag| start + tag + "</cx:binCount>".len());
            return Some(
                format!(
                    "{}<cx:binCount>4</cx:binCount>{}",
                    &xml[..start],
                    &xml[end..]
                )
                .into_bytes(),
            );
        }
        Some(data)
    });
    emit("histogram-bin-count-text-form", &text_form);
}

#[test]
fn a_chart_without_series_cannot_select_excel_default_parts() {
    let mut book = data_book();
    let mut chart =
        ChartExBuilder::funnel(formula("Sheet1!$A$1:$A$4"), formula("Sheet1!$C$1:$C$4"))
            .build()
            .unwrap();
    chart.chart_mut().unwrap().series_mut().clear();
    add_chart(&mut book, 0, chart);
    let mut bytes = Vec::new();
    assert!(writer::xlsx::write_writer(&book, &mut bytes).is_err());
}

/// The pareto package with style and colour payloads that differ from every
/// Excel default, so a substituted default is distinguishable from the
/// chart's own bytes.
fn customized_pareto(style_id: &str, colors_id: &str) -> Workbook {
    let bytes = rewrite(PARETO, |name, data| {
        let swap = |from: &str, to: String| {
            let xml = String::from_utf8(data.clone()).unwrap();
            assert!(xml.contains(from), "{name} lacks {from}");
            Some(xml.replacen(from, &to, 1).into_bytes())
        };
        match name {
            "xl/charts/style1.xml" => swap(" id=\"366\"", format!(" id=\"{style_id}\"")),
            "xl/charts/colors1.xml" => swap(" id=\"10\"", format!(" id=\"{colors_id}\"")),
            _ => Some(data),
        }
    });
    reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap()
}

/// For each extended chart of a sheet, the style part it is bound to in the
/// written package with its id, and the colour part with its id, found
/// through the chart's own relationships.
fn style_payloads(bytes: &[u8], book: &Workbook, sheet: usize) -> Vec<(String, String, String)> {
    book.sheet(sheet)
        .unwrap()
        .worksheet_drawing()
        .extended_charts()
        .iter()
        .map(|chart| {
            let source = chart.frame().source_part().unwrap().to_owned();
            let (directory, file) = source.rsplit_once('/').unwrap();
            let rels = text(bytes, &format!("{directory}/_rels/{file}.rels"));
            let target = |kind: &str| {
                let edge = rels
                    .split("<Relationship ")
                    .skip(1)
                    .find(|edge| edge.contains(&format!("relationships/{kind}\"")))
                    .unwrap_or_else(|| panic!("{source} has no {kind} edge: {rels}"));
                let target = edge
                    .split_once("Target=\"")
                    .unwrap()
                    .1
                    .split_once('"')
                    .unwrap()
                    .0;
                format!("xl/charts/{target}")
            };
            let id = |part: &str| {
                let xml = text(bytes, part);
                let after = xml.split_once(" id=\"").unwrap().1;
                after.split_once('"').unwrap().0.to_owned()
            };
            let (style, colors) = (target("chartStyle"), target("chartColorStyle"));
            (style.clone(), id(&style), id(&colors))
        })
        .collect()
}

#[test]
fn a_customized_chart_keeps_its_own_style_and_colours_when_moved_to_another_sheet() {
    let mut book = customized_pareto("777", "12");
    book.new_sheet("Moved").unwrap();
    let chart = book
        .sheet_mut(0)
        .unwrap()
        .worksheet_drawing_mut()
        .extended_charts_mut()
        .remove(0);
    book.sheet_mut(1)
        .unwrap()
        .worksheet_drawing_mut()
        .add_extended_chart(chart);
    let bytes = write_book(&book);
    assert_style_graph(&bytes);
    let reread = reader::xlsx::read_reader(Cursor::new(&bytes), true).unwrap();
    let payloads = style_payloads(&bytes, &reread, 1);
    assert_eq!(payloads.len(), 1);
    assert_eq!(
        (payloads[0].1.as_str(), payloads[0].2.as_str()),
        ("777", "12")
    );
    emit("customized-moved-sheet", &bytes);
    // A second save of the moved package keeps the same payloads.
    let again = write_book(&reread);
    assert_style_graph(&again);
    let reread = reader::xlsx::read_reader(Cursor::new(&again), true).unwrap();
    let payloads = style_payloads(&again, &reread, 1);
    assert_eq!(
        (payloads[0].1.as_str(), payloads[0].2.as_str()),
        ("777", "12")
    );
}

#[test]
fn a_customized_chart_keeps_its_own_style_and_colours_in_a_new_workbook() {
    let mut source = customized_pareto("777", "12");
    let chart = source
        .sheet_mut(0)
        .unwrap()
        .worksheet_drawing_mut()
        .extended_charts_mut()
        .remove(0);
    let mut fresh = umya_spreadsheet::new_file();
    for definition in source.sheet(0).unwrap().defined_names().to_vec() {
        fresh.add_defined_names(definition);
    }
    for row in 4..=13 {
        fresh
            .sheet_mut(0)
            .unwrap()
            .cell_mut((3, row))
            .set_value_number(row);
    }
    fresh
        .sheet_mut(0)
        .unwrap()
        .worksheet_drawing_mut()
        .add_extended_chart(chart);
    let bytes = write_book(&fresh);
    assert_style_graph(&bytes);
    let reread = reader::xlsx::read_reader(Cursor::new(&bytes), true).unwrap();
    let payloads = style_payloads(&bytes, &reread, 0);
    assert_eq!(payloads.len(), 1);
    assert_eq!(
        (payloads[0].1.as_str(), payloads[0].2.as_str()),
        ("777", "12")
    );
    emit("customized-moved-workbook", &bytes);
}

#[test]
fn a_moved_chart_never_reuses_the_style_parts_of_a_chart_already_at_its_destination() {
    // Both packages name their chart, style and colour parts alike, so the
    // moved chart's source paths collide at the destination.
    let mut source = customized_pareto("777", "12");
    let mut destination = customized_pareto("888", "14");
    destination.new_sheet("Moved").unwrap();
    let chart = source
        .sheet_mut(0)
        .unwrap()
        .worksheet_drawing_mut()
        .extended_charts_mut()
        .remove(0);
    destination
        .sheet_mut(1)
        .unwrap()
        .worksheet_drawing_mut()
        .add_extended_chart(chart);
    let bytes = write_book(&destination);
    assert_style_graph(&bytes);
    let reread = reader::xlsx::read_reader(Cursor::new(&bytes), true).unwrap();
    let resident = style_payloads(&bytes, &reread, 0);
    let moved = style_payloads(&bytes, &reread, 1);
    assert_eq!(resident.len(), 1);
    assert_eq!(moved.len(), 1);
    assert_eq!(
        (resident[0].1.as_str(), resident[0].2.as_str()),
        ("888", "14")
    );
    assert_eq!((moved[0].1.as_str(), moved[0].2.as_str()), ("777", "12"));
    assert_ne!(resident[0].0, moved[0].0);
    emit("customized-occupied-destination", &bytes);
}

#[test]
fn copies_of_a_customized_chart_each_own_their_style_and_colour_parts() {
    let mut book = customized_pareto("777", "12");
    book.new_sheet("Copy").unwrap();
    let original = book.sheet(0).unwrap().worksheet_drawing().extended_charts()[0].clone();
    // An edited copy beside the original is another chart: its own chart part
    // and its own style and colour parts, carrying the same payloads.
    let mut edited = original.clone();
    edited
        .frame_mut()
        .frame_properties_mut()
        .non_visual_drawing_properties_mut()
        .set_id(40);
    edited
        .frame_mut()
        .chart_space_mut()
        .chart_mut()
        .unwrap()
        .series_mut()[0]
        .set_hidden(Some(true));
    book.sheet_mut(0)
        .unwrap()
        .worksheet_drawing_mut()
        .add_extended_chart(edited);
    // An unedited copy on another sheet is the same chart and shares the part.
    book.sheet_mut(1)
        .unwrap()
        .worksheet_drawing_mut()
        .add_extended_chart(original);
    let bytes = write_book(&book);
    assert_style_graph(&bytes);
    let reread = reader::xlsx::read_reader(Cursor::new(&bytes), true).unwrap();
    let first = style_payloads(&bytes, &reread, 0);
    let second = style_payloads(&bytes, &reread, 1);
    assert_eq!(first.len(), 2);
    assert_eq!(second.len(), 1);
    for (_, style, colors) in first.iter().chain(&second) {
        assert_eq!((style.as_str(), colors.as_str()), ("777", "12"));
    }
    assert_ne!(
        first[0].0, first[1].0,
        "charts share a style part: {first:?}"
    );
    assert_eq!(first[0].0, second[0].0);
    emit("customized-copies", &bytes);
}

#[test]
fn every_box_and_whisker_group_is_a_series_excel_reads() {
    let mut book = data_book();
    let chart = ChartExBuilder::box_whisker(
        [formula("Sheet1!$C$1:$C$4"), formula("Sheet1!$D$1:$D$4")],
        ChartExQuartileMethod::Inclusive,
        &ChartExSeriesElementVisibilities {
            mean_marker: Some(true),
            outliers: Some(false),
            ..Default::default()
        },
    )
    .title("Groups")
    .build()
    .unwrap();
    add_chart(&mut book, 0, chart);
    let bytes = write_book(&book);
    assert_style_graph(&bytes);
    let reread = reader::xlsx::read_reader(Cursor::new(&bytes), true).unwrap();
    let chart = reread
        .sheet(0)
        .unwrap()
        .worksheet_drawing()
        .extended_charts()[0]
        .frame()
        .chart_space();
    assert_eq!(chart.chart().unwrap().series().len(), 2);
    assert_eq!(chart.data().unwrap().data.len(), 2);
    emit("box-whisker-groups", &bytes);
}

/// Excel's own packages of every kind pass through read and write. The Excel
/// check opens each result beside its original and compares what Excel
/// builds from both.
#[test]
fn excel_authored_charts_of_every_kind_survive_read_and_write() {
    let directory = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/extended_charts/excel-authored/"
    );
    let namespaces = |bytes: &[u8]| {
        let drawing = text(bytes, "xl/drawings/drawing1.xml");
        ["cx1", "cx2", "cx4"].map(|prefix| drawing.contains(&format!("Requires=\"{prefix}\"")))
    };
    let chart_parts = |bytes: &[u8]| {
        names(bytes)
            .into_iter()
            .filter(|name| name.starts_with("xl/charts/chartEx"))
            .count()
    };
    for kind in [
        "sunburst",
        "treemap",
        "waterfall",
        "histogram",
        "pareto",
        "box-whisker",
        "funnel",
        "region-map",
    ] {
        let original = std::fs::read(format!("{directory}{kind}.xlsx")).unwrap();
        let book = reader::xlsx::read_reader(Cursor::new(&original), true).unwrap();
        let bytes = write_book(&book);
        assert_style_graph(&bytes);
        assert_eq!(chart_parts(&bytes), chart_parts(&original), "{kind}");
        assert_eq!(namespaces(&bytes), namespaces(&original), "{kind}");
        assert!(
            namespaces(&bytes).contains(&true),
            "{kind} has no extended-chart namespace requirement"
        );
        let reread = reader::xlsx::read_reader(Cursor::new(&bytes), true).unwrap();
        assert_eq!(
            reread
                .sheet(0)
                .unwrap()
                .worksheet_drawing()
                .extended_charts()
                .len(),
            1,
            "{kind}"
        );
        emit(&format!("roundtrip-{kind}"), &bytes);
    }
}

/// Excel refuses a package whose style or colour part is declared with another
/// content type (measured with `Workbooks.Open`), so the library does not
/// import one, whether the workbook is read eagerly or lazily.
#[test]
fn a_style_or_colour_part_with_a_wrong_content_type_is_an_import_error() {
    let sample = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/extended_charts/excel-authored/sunburst.xlsx"
    ))
    .unwrap();
    let directory = PathBuf::from(
        std::env::var_os("CARGO_TARGET_DIR").unwrap_or_else(|| std::env::temp_dir().into()),
    )
    .join("chart-ex-content-types");
    std::fs::create_dir_all(&directory).unwrap();
    for (label, correct) in [
        ("style", "application/vnd.ms-office.chartstyle+xml"),
        ("colour", "application/vnd.ms-office.chartcolorstyle+xml"),
    ] {
        let wrong = rewrite(&sample, |name, data| {
            if name == "[Content_Types].xml" {
                let xml = String::from_utf8(data).unwrap();
                assert!(xml.contains(correct));
                return Some(xml.replace(correct, "application/xml").into_bytes());
            }
            Some(data)
        });
        let error = reader::xlsx::read_reader(Cursor::new(&wrong), true)
            .err()
            .unwrap_or_else(|| panic!("eager read accepted a {label} part typed application/xml"));
        assert!(
            error.to_string().contains("content type"),
            "{label}: {error}"
        );
        let path = directory.join(format!("wrong-{label}.xlsx"));
        std::fs::write(&path, &wrong).unwrap();
        let error = match reader::xlsx::lazy_read(&path) {
            Err(error) => error,
            Ok(mut book) => book.try_read_sheet(0).err().unwrap_or_else(|| {
                panic!("lazy read accepted a {label} part typed application/xml")
            }),
        };
        assert!(
            error.to_string().contains("content type"),
            "{label}: {error}"
        );
    }
    // The unmodified sample is accepted by the same entry points.
    reader::xlsx::read_reader(Cursor::new(&sample), true).unwrap();
}

/// The sunburst sample with a picture fill that links an external image.
fn sunburst_with_external_image(url: &str) -> Workbook {
    let sample = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/extended_charts/excel-authored/sunburst.xlsx"
    ))
    .unwrap();
    let bytes = rewrite(&sample, |name, data| {
        let mut xml = String::from_utf8(data.clone()).ok()?;
        match name {
            "xl/charts/chartEx1.xml" => {
                xml = xml.replace(
                    "</cx:chart></cx:chartSpace>",
                    "</cx:chart><cx:spPr><a:blipFill><a:blip \
                     r:link=\"external1\"/><a:stretch/></a:blipFill></cx:spPr></cx:chartSpace>",
                );
                assert!(xml.contains("external1"));
            }
            "xl/charts/_rels/chartEx1.xml.rels" => {
                xml = xml.replace(
                    "</Relationships>",
                    &format!(
                        "<Relationship Id=\"external1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/image\" Target=\"{url}\" TargetMode=\"External\"/></Relationships>"
                    ),
                );
            }
            _ => return Some(data),
        }
        Some(xml.into_bytes())
    });
    reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap()
}

/// The external image edges (target, mode) of every extended chart of every
/// sheet of a written package, found through each chart's own relationships.
fn external_images(bytes: &[u8]) -> Vec<(String, Vec<(String, bool)>)> {
    let book = reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap();
    let mut result = Vec::new();
    for sheet in book.sheet_collection() {
        for chart in sheet.worksheet_drawing().extended_charts() {
            let source = chart.frame().source_part().unwrap().to_owned();
            let (directory, file) = source.rsplit_once('/').unwrap();
            let rels = text(bytes, &format!("{directory}/_rels/{file}.rels"));
            let edges = rels
                .split("<Relationship ")
                .skip(1)
                .filter(|edge| edge.contains("relationships/image\""))
                .map(|edge| {
                    let target = edge
                        .split_once("Target=\"")
                        .unwrap()
                        .1
                        .split_once('"')
                        .unwrap()
                        .0
                        .to_owned();
                    (target, edge.contains("TargetMode=\"External\""))
                })
                .collect();
            result.push((source, edges));
        }
    }
    result
}

#[test]
fn charts_that_differ_only_in_an_external_image_url_keep_their_own_url() {
    let first = "https://example.com/first.png";
    let second = "https://example.com/second.png";
    for (urls, shared) in [([first, second], false), ([first, first], true)] {
        let mut fresh = umya_spreadsheet::new_file();
        for (index, url) in urls.into_iter().enumerate() {
            let source = sunburst_with_external_image(url);
            let mut sheet = source.sheet(0).unwrap().clone();
            sheet.set_name(format!("Copy{index}"));
            // Both copies bind the same names, which the workbook holds once.
            if index == 0 {
                for definition in sheet.defined_names().to_vec() {
                    fresh.add_defined_names(definition);
                }
            }
            sheet.defined_names_mut().clear();
            fresh.add_sheet(sheet).unwrap();
        }
        let bytes = write_book(&fresh);
        let images = external_images(&bytes);
        assert_eq!(images.len(), 2, "{images:?}");
        for (index, url) in urls.into_iter().enumerate() {
            assert_eq!(
                images[index].1,
                [(url.to_owned(), true)],
                "chart {index}: {images:?}"
            );
        }
        // Equal charts are one part; charts with another URL are not.
        assert_eq!(images[0].0 == images[1].0, shared, "{images:?}");
    }
}
