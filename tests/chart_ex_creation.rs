#![cfg(feature = "serde")]

use std::io::{
    Cursor,
    Read,
};

use umya_spreadsheet::{
    drawing::{
        charts::*,
        spreadsheet::{
            DrawingAnchor,
            ExtendedChart,
            Extent,
        },
    },
    reader,
    writer,
};

fn formula(text: &str) -> ChartExFormula {
    ChartExFormula::new(text)
}

fn constructors() -> Vec<ChartExBuilder> {
    let categories = || formula("Sheet1!$A$1:$B$4");
    let values = || formula("Sheet1!$C$1:$C$4");
    vec![
        ChartExBuilder::sunburst(categories(), values()),
        ChartExBuilder::treemap(categories(), values(), ChartExParentLabelLayout::Banner),
        ChartExBuilder::waterfall(categories(), values(), [0, 3], true),
        ChartExBuilder::histogram(
            values(),
            ChartExBinning {
                bin_count: Some(3),
                ..Default::default()
            },
        ),
        ChartExBuilder::pareto(
            values(),
            ChartExBinning {
                bin_count: Some(3),
                ..Default::default()
            },
        ),
        ChartExBuilder::box_whisker(
            [values()],
            ChartExQuartileMethod::Inclusive,
            &ChartExSeriesElementVisibilities {
                mean_marker: Some(true),
                outliers: Some(false),
                ..Default::default()
            },
        ),
        ChartExBuilder::funnel(categories(), values()),
        ChartExBuilder::region_map(formula("Sheet1!$A$1:$A$4"), values(), "en-US", "US"),
    ]
}

fn package_xml(bytes: &[u8], path: &str) -> String {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut xml = String::new();
    archive
        .by_name(path)
        .unwrap()
        .read_to_string(&mut xml)
        .unwrap();
    xml
}

fn emit(name: &str, bytes: &[u8]) {
    if let Some(target) = std::env::var_os("CARGO_TARGET_DIR") {
        let directory = std::path::PathBuf::from(target).join("chart-ex-validation");
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join(format!("{name}.xlsx")), bytes).unwrap();
    }
}

#[test]
fn created_chart_kinds_roundtrip_through_a_worksheet_package() {
    let layouts = [
        ChartExSeriesLayout::Sunburst,
        ChartExSeriesLayout::Treemap,
        ChartExSeriesLayout::Waterfall,
        ChartExSeriesLayout::ClusteredColumn,
        ChartExSeriesLayout::ClusteredColumn,
        ChartExSeriesLayout::BoxWhisker,
        ChartExSeriesLayout::Funnel,
        ChartExSeriesLayout::RegionMap,
    ];
    // Style ids Excel itself assigned when it inserted each chart kind with
    // its default style (observed in Excel-authored packages).
    let excel_style_ids = [381, 410, 395, 366, 366, 406, 419, 494];
    let kinds = [
        "sunburst",
        "treemap",
        "waterfall",
        "histogram",
        "pareto",
        "box-whisker",
        "funnel",
        "region-map",
    ];
    for (((builder, layout), kind), style_id) in constructors()
        .into_iter()
        .zip(layouts)
        .zip(kinds)
        .zip(excel_style_ids)
    {
        let mut book = umya_spreadsheet::new_file();
        for row in 1..=4 {
            let sheet = book.sheet_mut(0).unwrap();
            sheet.cell_mut((1, row)).set_value(format!("Group {row}"));
            sheet.cell_mut((2, row)).set_value(format!("Leaf {row}"));
            sheet.cell_mut((3, row)).set_value_number(row * 10);
            // A second numeric column no created chart refers to: the Excel
            // check reads it to prove a chart bound to the right cells.
            sheet.cell_mut((4, row)).set_value_number(row * 10 - 5);
        }
        if kind == "sunburst" {
            let mut plain = Vec::new();
            writer::xlsx::write_writer(&book, &mut plain).unwrap();
            emit("control-library", &plain);
        }
        let built = builder
            .title("Sales & costs")
            .series_name("Revenue")
            .build()
            .unwrap();
        let expected = serde_json::to_value(&built).unwrap();
        let mut extent = Extent::default();
        extent.set_cx(5_000_000).set_cy(3_000_000);
        book.sheet_mut(0)
            .unwrap()
            .worksheet_drawing_mut()
            .add_extended_chart(ExtendedChart::new(
                DrawingAnchor::OneCell {
                    from: Default::default(),
                    extent,
                },
                built,
            ));
        let mut bytes = Vec::new();
        writer::xlsx::write_writer(&book, &mut bytes).unwrap();
        emit(kind, &bytes);
        let chart_xml = package_xml(&bytes, "xl/charts/chartEx1.xml");
        assert!(chart_xml.contains(&format!("layoutId=\"{}\"", layout.as_str())));
        assert!(
            package_xml(&bytes, "[Content_Types].xml")
                .contains("application/vnd.ms-office.chartex+xml")
        );
        assert!(
            package_xml(&bytes, "xl/drawings/_rels/drawing1.xml.rels")
                .contains("http://schemas.microsoft.com/office/2014/relationships/chartEx")
        );
        assert_schema_sequence(&chart_xml);
        assert_style_parts(&bytes, style_id, kind);
        for eager in [false, true] {
            let mut reread = reader::xlsx::read_reader(Cursor::new(&bytes), eager).unwrap();
            reread.try_read_sheet(0).unwrap();
            let actual = &reread
                .sheet(0)
                .unwrap()
                .worksheet_drawing()
                .extended_charts()[0];
            let mut actual = actual.chart_space().clone();
            actual.strip_imported_caches();
            assert_eq!(
                serde_json::to_value(actual).unwrap(),
                expected,
                "{}",
                layout.as_str()
            );
        }
    }
}

// Excel rejects a native extended chart that lacks its style and colour
// parts, so a created chart must carry both with their relationships and
// content types.
fn assert_style_parts(bytes: &[u8], style_id: u32, kind: &str) {
    let rels = package_xml(bytes, "xl/charts/_rels/chartEx1.xml.rels");
    let edges: Vec<&str> = rels.split("<Relationship ").skip(1).collect();
    assert_eq!(edges.len(), 2, "{kind}: {rels}");
    let has = |kind_uri: &str, target: &str| {
        edges.iter().any(|edge| {
            edge.contains(&format!("Type=\"{kind_uri}\""))
                && edge.contains(&format!("Target=\"{target}\""))
        })
    };
    assert!(
        has(
            "http://schemas.microsoft.com/office/2011/relationships/chartStyle",
            "style1.xml"
        ),
        "{kind}: {rels}"
    );
    assert!(
        has(
            "http://schemas.microsoft.com/office/2011/relationships/chartColorStyle",
            "colors1.xml"
        ),
        "{kind}: {rels}"
    );
    let types = package_xml(bytes, "[Content_Types].xml");
    assert!(types.contains(
        "PartName=\"/xl/charts/style1.xml\" \
         ContentType=\"application/vnd.ms-office.chartstyle+xml\""
    ));
    assert!(types.contains(
        "PartName=\"/xl/charts/colors1.xml\" \
         ContentType=\"application/vnd.ms-office.chartcolorstyle+xml\""
    ));
    let style = package_xml(bytes, "xl/charts/style1.xml");
    assert!(
        style.starts_with("<cs:chartStyle xmlns:cs=\"http://schemas.microsoft.com/office/drawing/2012/chartStyle\""),
        "{kind}"
    );
    assert!(
        style
            .split_once("id=\"")
            .is_some_and(|(_, rest)| rest.starts_with(&format!("{style_id}\""))),
        "{kind}: expected Excel style id {style_id}"
    );
    let colors = package_xml(bytes, "xl/charts/colors1.xml");
    assert!(colors.starts_with("<cs:colorStyle"));
    assert!(colors.contains("meth=\"cycle\" id=\"10\""));
}

fn assert_schema_sequence(xml: &str) {
    // MS-ODRAWXML CT_ChartSpace, CT_Chart, CT_PlotArea, CT_Series and
    // CT_SeriesLayoutProperties sequences; independently transcribed.
    let sequence = |owner: &str| -> Option<&[&str]> {
        match owner {
            "chartSpace" => Some(&[
                "chartData",
                "chart",
                "dashboard",
                "spPr",
                "txPr",
                "clrMapOvr",
                "fmtOvrs",
                "printSettings",
                "extLst",
            ]),
            "chartData" => Some(&["externalData", "data", "extLst"]),
            // A data block's dimensions are an unbounded choice.
            "data" => Some(&["strDim", "numDim", "extLst"]),
            "strDim" | "numDim" => Some(&["f", "nf", "lvl"]),
            "lvl" => Some(&["pt"]),
            "f" | "nf" | "pt" | "v" | "binCount" | "binSize" | "axisId" => Some(&[]),
            "chart" => Some(&["title", "plotArea", "legend", "extLst"]),
            "title" => Some(&["tx", "spPr", "txPr", "offset", "extLst"]),
            "tx" => Some(&["txData", "rich"]),
            "txData" => Some(&["f", "v"]),
            "plotArea" => Some(&["plotAreaRegion", "axis", "spPr", "extLst"]),
            "plotAreaRegion" => Some(&["plotSurface", "series", "extLst"]),
            "axis" => Some(&[
                "catScaling",
                "valScaling",
                "title",
                "units",
                "majorGridlines",
                "minorGridlines",
                "majorTickMarks",
                "minorTickMarks",
                "tickLabels",
                "numFmt",
                "spPr",
                "txPr",
                "extLst",
            ]),
            "series" => Some(&[
                "tx",
                "spPr",
                "valueColors",
                "valueColorPositions",
                "dataPt",
                "dataLabels",
                "dataId",
                "layoutPr",
                "axisId",
                "extLst",
            ]),
            "layoutPr" => Some(&[
                "parentLabelLayout",
                "regionLabelLayout",
                "visibility",
                "aggregation",
                "binning",
                "geography",
                "statistics",
                "subtotals",
                "extLst",
            ]),
            "binning" => Some(&["binSize", "binCount"]),
            "units" => Some(&["unitsLabel", "extLst"]),
            "subtotals" => Some(&["idx"]),
            "catScaling" | "valScaling" | "parentLabelLayout" | "visibility" | "statistics"
            | "idx" | "dataId" => Some(&[]),
            _ => None,
        }
    };
    let mut reader = quick_xml::Reader::from_str(xml);
    let mut stack: Vec<(String, usize)> = Vec::new();
    loop {
        let event = reader.read_event().unwrap();
        match &event {
            quick_xml::events::Event::Start(element) | quick_xml::events::Event::Empty(element) => {
                let name = String::from_utf8(element.local_name().as_ref().to_vec()).unwrap();
                if let Some((parent, previous)) = stack.last_mut() {
                    if let Some(order) = sequence(parent) {
                        let mut position = order
                            .iter()
                            .position(|entry| *entry == name)
                            .expect("schema child");
                        if parent == "data" && name == "numDim" {
                            position = 0;
                        }
                        assert!(position >= *previous, "out-of-order {parent}/{name}");
                        *previous = position;
                    }
                }
                if matches!(&event, quick_xml::events::Event::Start(_)) {
                    stack.push((name, 0));
                }
            }
            quick_xml::events::Event::End(_) => {
                stack.pop().unwrap();
            }
            quick_xml::events::Event::Eof => break,
            _ => (),
        }
    }
}

#[test]
fn named_creation_registers_real_sources_without_colliding_with_saved_names() {
    let mut book = umya_spreadsheet::new_file();
    for row in 1..=4 {
        book.sheet_mut(0)
            .unwrap()
            .cell_mut((1, row))
            .set_value("Group");
        book.sheet_mut(0)
            .unwrap()
            .cell_mut((2, row))
            .set_value("Leaf");
        book.sheet_mut(0)
            .unwrap()
            .cell_mut((3, row))
            .set_value_number(row * 10);
    }
    let mut reserved = umya_spreadsheet::DefinedName::default();
    reserved
        .set_name("_xlchart.v1.0")
        .set_address("Sheet1!$A$1");
    book.add_defined_names(reserved);
    let chart = ChartExBuilder::sunburst(formula("Sheet1!$A$1:$B$4"), formula("Sheet1!$C$1:$C$4"))
        .build_with_defined_names(&mut book)
        .unwrap();
    let dimensions = &chart.data().unwrap().data[0].dimensions;
    let ChartExDimension::String(categories) = &dimensions[0] else {
        panic!("category dimension")
    };
    let ChartExDimensionSource::Reference(categories) = &categories.source else {
        panic!("source reference")
    };
    assert_eq!(categories.formula().text(), "_xlchart.v1.1");
    let names = book.defined_names();
    assert!(
        names
            .iter()
            .any(|name| name.name() == "_xlchart.v1.1" && name.address() == "Sheet1!$A$1:$B$4")
    );
    assert!(
        names
            .iter()
            .any(|name| name.name() == "_xlchart.v1.2" && name.address() == "Sheet1!$C$1:$C$4")
    );
    assert!(
        names
            .iter()
            .filter(|name| name.name().starts_with("_xlchart.v1.") && name.name() != "_xlchart.v1.0")
            .all(umya_spreadsheet::DefinedName::hidden)
    );
    book.sheet_mut(0)
        .unwrap()
        .worksheet_drawing_mut()
        .add_extended_chart(ExtendedChart::new(DrawingAnchor::default(), chart));
    let mut bytes = Vec::new();
    writer::xlsx::write_writer(&book, &mut bytes).unwrap();
    let xml = package_xml(&bytes, "xl/charts/chartEx1.xml");
    assert!(xml.contains(">_xlchart.v1.1</cx:f>"));
    assert!(xml.contains(">_xlchart.v1.2</cx:f>"));
    assert_schema_sequence(&xml);
    let reread = reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap();
    assert!(
        reread
            .defined_names()
            .iter()
            .chain(
                reread
                    .sheet_collection_no_check()
                    .iter()
                    .flat_map(|sheet| sheet.defined_names())
            )
            .any(|name| name.name() == "_xlchart.v1.1" && name.address() == "Sheet1!$A$1:$B$4")
    );
    assert_eq!(
        reread
            .sheet(0)
            .unwrap()
            .worksheet_drawing()
            .extended_charts()[0]
            .chart_space()
            .chart()
            .unwrap()
            .series()[0]
            .layout(),
        ChartExSeriesLayout::Sunburst
    );
}

#[test]
fn histogram_creation_rejects_conflicting_and_nonpositive_bins() {
    for binning in [
        ChartExBinning {
            bin_count: Some(0),
            ..Default::default()
        },
        ChartExBinning {
            bin_size: Some(XmlDouble::default()),
            bin_count: Some(2),
            ..Default::default()
        },
    ] {
        assert!(
            ChartExBuilder::histogram(formula("Sheet1!C1:C4"), binning)
                .build()
                .is_err()
        );
    }
}

#[test]
fn named_creation_avoids_sheet_scoped_names_shadowing_generated_sources() {
    let mut book = umya_spreadsheet::new_file();
    let mut reserved = umya_spreadsheet::DefinedName::default();
    reserved
        .set_name("_XLCHART.V1.0")
        .set_address("Sheet1!$D$1");
    reserved.set_local_sheet_id(0);
    book.sheet_mut(0)
        .unwrap()
        .defined_names_mut()
        .push(reserved);
    let chart = ChartExBuilder::sunburst(formula("Sheet1!$A$1:$B$4"), formula("Sheet1!$C$1:$C$4"))
        .build_with_defined_names(&mut book)
        .unwrap();
    let ChartExDimension::String(categories) = &chart.data().unwrap().data[0].dimensions[0] else {
        panic!("category dimension")
    };
    let ChartExDimensionSource::Reference(categories) = &categories.source else {
        panic!("source reference")
    };
    assert_eq!(categories.formula().text(), "_xlchart.v1.1");
}

#[test]
fn pareto_creation_uses_an_owned_line_and_a_percentage_axis() {
    let chart = ChartExBuilder::pareto(formula("Sheet1!C1:C4"), ChartExBinning::default())
        .build()
        .unwrap();
    let series = chart.chart().unwrap().series();
    assert_eq!(series[0].axis_ids, [1]);
    assert_eq!(series[1].owner_idx, Some(0));
    assert!(series[1].data_id.is_none());
    assert_eq!(series[1].axis_ids, [2]);
    let axis = chart
        .chart()
        .unwrap()
        .axes()
        .iter()
        .find(|axis| axis.id == 2)
        .unwrap();
    let scaling = axis.value_scaling.as_ref().unwrap();
    let ChartExAutomaticNumber::Number(min) = scaling.min.as_ref().unwrap() else {
        panic!("minimum")
    };
    let ChartExAutomaticNumber::Number(max) = scaling.max.as_ref().unwrap() else {
        panic!("maximum")
    };
    assert_eq!(min.value(), Some(0.0));
    assert_eq!(max.value(), Some(1.0));
    assert_eq!(
        axis.units.as_ref().unwrap().unit,
        Some(ChartExAxisUnit::Percentage)
    );
}

#[test]
fn box_whisker_creation_uses_numeric_groups_without_category_dimensions() {
    let chart = ChartExBuilder::box_whisker(
        [formula("Sheet1!C1:C4"), formula("Sheet1!D1:D4")],
        ChartExQuartileMethod::Exclusive,
        &ChartExSeriesElementVisibilities::default(),
    )
    .build()
    .unwrap();
    assert_eq!(chart.data().unwrap().data.len(), 2);
    assert!(matches!(
        &chart.data().unwrap().data[0].dimensions[..],
        [ChartExDimension::Numeric(_)]
    ));
}

#[test]
fn named_creation_rejects_relative_sources_without_mutating_names() {
    for values in [
        "C1:C4",
        "LocalSales",
        "Sheet1!C1:C4",
        "Sheet1!$C1:$C$4",
        "Sheet1!C$1:$C$4",
    ] {
        let mut book = umya_spreadsheet::new_file();
        let result = ChartExBuilder::sunburst(formula("Sheet1!$A$1:$B$4"), formula(values))
            .build_with_defined_names(&mut book);
        assert!(
            result.is_err(),
            "host-relative source {values} cannot become a workbook name"
        );
        assert!(book.defined_names().is_empty());
    }
}

#[test]
fn empty_box_groups_return_an_error_after_fluent_title_and_name_edits() {
    assert!(
        ChartExBuilder::box_whisker(
            [],
            ChartExQuartileMethod::Exclusive,
            &ChartExSeriesElementVisibilities::default()
        )
        .title("Empty groups")
        .series_name("Missing")
        .build()
        .is_err()
    );
}

#[test]
fn created_axis_and_title_layouts_follow_excel_chart_parts() {
    let chart = ChartExBuilder::histogram(formula("Sheet1!C1:C4"), ChartExBinning::default())
        .title("Distribution")
        .build()
        .unwrap();
    let title = chart.chart().unwrap().title.as_ref().unwrap();
    assert_eq!(title.position, Some(ChartExSidePos::Top));
    assert_eq!(title.align, Some(ChartExPosAlign::Center));
    assert_eq!(title.overlay, Some(false));
    let axes = chart.chart().unwrap().axes();
    let gap = axes[0]
        .category_scaling
        .as_ref()
        .unwrap()
        .gap_width
        .as_ref()
        .unwrap();
    let ChartExAutomaticNumber::Number(gap) = &gap.0 else {
        panic!("numeric gap width")
    };
    assert_eq!(gap.value(), Some(0.0));
    assert!(axes.iter().all(|axis| axis.tick_labels.is_some()));
    assert!(axes[1].major_gridlines.is_some());
}

#[test]
fn excel_fixtures_document_the_derived_line_and_numeric_group_models() {
    let pareto = reader::xlsx::read_reader(
        Cursor::new(include_bytes!("fixtures/extended_charts/creation-pareto.xlsx").as_slice()),
        true,
    )
    .unwrap();
    let chart = pareto
        .sheet(0)
        .unwrap()
        .worksheet_drawing()
        .extended_charts()[0]
        .chart_space();
    let series = chart.chart().unwrap().series();
    assert_eq!(series[0].axis_ids, [1]);
    assert_eq!(series[1].owner_idx, Some(0));
    assert!(series[1].data_id.is_none());
    assert_eq!(series[1].axis_ids, [2]);
    let box_whisker = reader::xlsx::read_reader(
        Cursor::new(
            include_bytes!("fixtures/extended_charts/creation-box-whisker.xlsx").as_slice(),
        ),
        true,
    )
    .unwrap();
    let chart = box_whisker
        .sheet(0)
        .unwrap()
        .worksheet_drawing()
        .extended_charts()[0]
        .chart_space();
    assert_eq!(chart.data().unwrap().data.len(), 3);
    assert_eq!(chart.chart().unwrap().series().len(), 3);
    for data in &chart.data().unwrap().data {
        assert!(matches!(
            &data.dimensions[..],
            [ChartExDimension::Numeric(_)]
        ));
    }
}

fn book_with_data() -> umya_spreadsheet::Workbook {
    let mut book = umya_spreadsheet::new_file();
    for row in 1..=4 {
        let sheet = book.sheet_mut(0).unwrap();
        sheet.cell_mut((1, row)).set_value(format!("Region {row}"));
        sheet.cell_mut((3, row)).set_value_number(row * 10);
    }
    book
}

fn add_chart(book: &mut umya_spreadsheet::Workbook, chart: ChartExSpace) {
    book.sheet_mut(0)
        .unwrap()
        .worksheet_drawing_mut()
        .add_extended_chart(ExtendedChart::new(DrawingAnchor::default(), chart));
}

fn entry_names(bytes: &[u8]) -> Vec<String> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut names: Vec<String> = (0..archive.len())
        .map(|index| {
            archive
                .by_index_raw(index)
                .unwrap()
                .name()
                .unwrap()
                .into_owned()
        })
        .collect();
    names.sort();
    names
}

#[test]
fn region_map_creation_names_the_geography_and_leaves_the_cache_to_excel() {
    let mut book = book_with_data();
    let chart = ChartExBuilder::region_map(
        formula("Sheet1!$A$1:$A$4"),
        formula("Sheet1!$C$1:$C$4"),
        "fr-FR",
        "FR",
    )
    .build()
    .unwrap();
    add_chart(&mut book, chart);
    let mut bytes = Vec::new();
    writer::xlsx::write_writer(&book, &mut bytes).unwrap();
    let xml = package_xml(&bytes, "xl/charts/chartEx1.xml");
    assert!(xml.contains("layoutId=\"regionMap\""));
    assert!(xml.contains("<cx:numDim type=\"colorVal\">"));
    assert!(xml.contains("<cx:geography "));
    assert!(xml.contains("cultureLanguage=\"fr-FR\""));
    assert!(xml.contains("cultureRegion=\"FR\""));
    assert!(!xml.contains("geoCache"));
    assert_schema_sequence(&xml);
    assert_style_parts(&bytes, 494, "region-map");
    for (language, region) in [
        ("", "US"),
        ("en-US", " "),
        ("en-US ", "US"),
        ("en-US", " US"),
    ] {
        assert!(
            ChartExBuilder::region_map(
                formula("Sheet1!$A$1:$A$4"),
                formula("Sheet1!$C$1:$C$4"),
                language,
                region,
            )
            .build()
            .is_err()
        );
    }
}

#[test]
fn every_created_chart_gets_its_own_numbered_style_and_colour_parts() {
    let mut book = book_with_data();
    add_chart(
        &mut book,
        ChartExBuilder::funnel(formula("Sheet1!$A$1:$A$4"), formula("Sheet1!$C$1:$C$4"))
            .build()
            .unwrap(),
    );
    add_chart(
        &mut book,
        ChartExBuilder::treemap(
            formula("Sheet1!$A$1:$A$4"),
            formula("Sheet1!$C$1:$C$4"),
            ChartExParentLabelLayout::Overlapping,
        )
        .build()
        .unwrap(),
    );
    let mut bytes = Vec::new();
    writer::xlsx::write_writer(&book, &mut bytes).unwrap();
    let names = entry_names(&bytes);
    for part in [
        "xl/charts/style1.xml",
        "xl/charts/style2.xml",
        "xl/charts/colors1.xml",
        "xl/charts/colors2.xml",
        "xl/charts/_rels/chartEx1.xml.rels",
        "xl/charts/_rels/chartEx2.xml.rels",
    ] {
        assert!(names.iter().any(|name| name == part), "missing {part}");
    }
    // Each chart references only its own parts; the style ids follow the
    // chart kind that owns them, not the part number.
    for (index, layout, style_id) in [(1, "funnel", 419), (2, "treemap", 410)] {
        let chart = package_xml(&bytes, &format!("xl/charts/chartEx{index}.xml"));
        assert!(chart.contains(&format!("layoutId=\"{layout}\"")));
        let rels = package_xml(&bytes, &format!("xl/charts/_rels/chartEx{index}.xml.rels"));
        let own = rels
            .matches(&format!("Target=\"style{index}.xml\""))
            .count()
            + rels
                .matches(&format!("Target=\"colors{index}.xml\""))
                .count();
        assert_eq!(own, 2, "{rels}");
        let style = package_xml(&bytes, &format!("xl/charts/style{index}.xml"));
        assert!(style.contains(&format!(" id=\"{style_id}\">")));
    }
}

#[test]
fn created_chart_keeps_its_style_parts_when_the_saved_package_is_written_again() {
    let mut book = book_with_data();
    add_chart(
        &mut book,
        ChartExBuilder::sunburst(formula("Sheet1!$A$1:$A$4"), formula("Sheet1!$C$1:$C$4"))
            .title("First")
            .build()
            .unwrap(),
    );
    let mut first = Vec::new();
    writer::xlsx::write_writer(&book, &mut first).unwrap();
    let mut reread = reader::xlsx::read_reader(Cursor::new(&first), true).unwrap();
    reread
        .sheet_mut(0)
        .unwrap()
        .worksheet_drawing_mut()
        .extended_charts_mut()[0]
        .chart_space_mut()
        .chart_mut()
        .unwrap()
        .title = Some(ChartExChartTitle {
        text: Some(ChartExText {
            tx_data: Some(ChartExTextData::literal("Second")),
            rich:    None,
        }),
        ..Default::default()
    });
    let mut second = Vec::new();
    writer::xlsx::write_writer(&reread, &mut second).unwrap();
    assert!(package_xml(&second, "xl/charts/chartEx1.xml").contains("Second"));
    assert_style_parts(&second, 381, "sunburst after rewrite");
    assert_eq!(
        package_xml(&second, "xl/charts/style1.xml"),
        package_xml(&first, "xl/charts/style1.xml")
    );
    assert_eq!(
        entry_names(&second)
            .iter()
            .filter(|name| name.starts_with("xl/charts/"))
            .count(),
        entry_names(&first)
            .iter()
            .filter(|name| name.starts_with("xl/charts/"))
            .count()
    );
}

#[test]
fn series_name_needs_a_single_data_series() {
    let groups = ChartExBuilder::box_whisker(
        [formula("Sheet1!C1:C4"), formula("Sheet1!D1:D4")],
        ChartExQuartileMethod::Exclusive,
        &ChartExSeriesElementVisibilities::default(),
    )
    .series_name("Only the first?")
    .build();
    assert!(groups.is_err());
    let pareto = ChartExBuilder::pareto(formula("Sheet1!C1:C4"), ChartExBinning::default())
        .series_name("Revenue")
        .build()
        .unwrap();
    let series = pareto.chart().unwrap().series();
    assert!(series[0].text.is_some());
    assert!(series[1].text.is_none());
}

#[test]
fn created_series_carry_excel_style_unique_ids() {
    let chart = ChartExBuilder::pareto(formula("Sheet1!C1:C4"), ChartExBinning::default())
        .build()
        .unwrap();
    let ids: Vec<String> = chart
        .chart()
        .unwrap()
        .series()
        .iter()
        .map(|series| series.unique_id.clone().expect("unique id"))
        .collect();
    assert_ne!(ids[0], ids[1]);
    for (index, id) in ids.iter().enumerate() {
        // {0000000N-XXXX-XXXX-XXXX-XXXXXXXXXXXX}, uppercase hex.
        let body = id.strip_prefix('{').unwrap().strip_suffix('}').unwrap();
        let groups: Vec<&str> = body.split('-').collect();
        assert_eq!(
            groups.iter().map(|group| group.len()).collect::<Vec<_>>(),
            [8, 4, 4, 4, 12],
            "{id}"
        );
        assert!(body.chars().all(|c| c == '-' || c.is_ascii_hexdigit()));
        assert_eq!(body, body.to_ascii_uppercase());
        assert_eq!(groups[0], format!("{:08X}", index + 1));
    }
    let mut book = book_with_data();
    add_chart(&mut book, chart);
    let mut bytes = Vec::new();
    writer::xlsx::write_writer(&book, &mut bytes).unwrap();
    let xml = package_xml(&bytes, "xl/charts/chartEx1.xml");
    for id in &ids {
        assert!(xml.contains(&format!("uniqueId=\"{id}\"")));
    }
}
