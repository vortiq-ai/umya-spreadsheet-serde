#![cfg(feature = "serde")]
//! The chart style/colour parts and the compatibility namespace the library
//! writes for a created chart must be exactly what Excel wrote for the same
//! kind (`fixtures/extended_charts/excel-authored`, produced by
//! `chart_ex_excel_author.ps1`).
use std::io::{
    Cursor,
    Read,
};

use sha2::{
    Digest,
    Sha256,
};
use umya_spreadsheet::{
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

const MANIFEST: &str = include_str!("fixtures/extended_charts/excel-authored/manifest.json");

fn formula(text: &str) -> ChartExFormula {
    ChartExFormula::new(text)
}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
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

fn requires(drawing: &[u8]) -> String {
    let text = String::from_utf8(drawing.to_vec()).unwrap();
    let (_, rest) = text.split_once("Requires=\"").expect("mc:Choice Requires");
    rest.split_once('"').unwrap().0.to_owned()
}

fn created(builder: ChartExBuilder) -> Vec<u8> {
    let mut book = umya_spreadsheet::new_file();
    for row in 1..=4 {
        let sheet = book.sheet_mut(0).unwrap();
        sheet.cell_mut((1, row)).set_value(format!("Group {row}"));
        sheet.cell_mut((2, row)).set_value(format!("Leaf {row}"));
        sheet.cell_mut((3, row)).set_value_number(row * 10);
    }
    book.sheet_mut(0)
        .unwrap()
        .worksheet_drawing_mut()
        .add_extended_chart(ExtendedChart::new(
            DrawingAnchor::default(),
            builder.build().unwrap(),
        ));
    let mut bytes = Vec::new();
    writer::xlsx::write_writer(&book, &mut bytes).unwrap();
    // The package must stay readable.
    let reread = reader::xlsx::read_reader(Cursor::new(&bytes), true).unwrap();
    assert_eq!(
        reread
            .sheet(0)
            .unwrap()
            .worksheet_drawing()
            .extended_charts()
            .len(),
        1
    );
    bytes
}

#[test]
fn created_style_parts_and_requirements_equal_the_excel_authored_samples() {
    let categories = || formula("Sheet1!$A$1:$B$4");
    let values = || formula("Sheet1!$C$1:$C$4");
    let samples: [(&str, &[u8], ChartExBuilder); 8] = [
        (
            "sunburst",
            include_bytes!("fixtures/extended_charts/excel-authored/sunburst.xlsx"),
            ChartExBuilder::sunburst(categories(), values()),
        ),
        (
            "treemap",
            include_bytes!("fixtures/extended_charts/excel-authored/treemap.xlsx"),
            ChartExBuilder::treemap(categories(), values(), ChartExParentLabelLayout::Banner),
        ),
        (
            "waterfall",
            include_bytes!("fixtures/extended_charts/excel-authored/waterfall.xlsx"),
            ChartExBuilder::waterfall(categories(), values(), [], true),
        ),
        (
            "histogram",
            include_bytes!("fixtures/extended_charts/excel-authored/histogram.xlsx"),
            ChartExBuilder::histogram(values(), ChartExBinning::default()),
        ),
        (
            "pareto",
            include_bytes!("fixtures/extended_charts/excel-authored/pareto.xlsx"),
            ChartExBuilder::pareto(values(), ChartExBinning::default()),
        ),
        (
            "box-whisker",
            include_bytes!("fixtures/extended_charts/excel-authored/box-whisker.xlsx"),
            ChartExBuilder::box_whisker(
                [values()],
                ChartExQuartileMethod::Exclusive,
                &ChartExSeriesElementVisibilities::default(),
            ),
        ),
        (
            "funnel",
            include_bytes!("fixtures/extended_charts/excel-authored/funnel.xlsx"),
            ChartExBuilder::funnel(categories(), values()),
        ),
        (
            "region-map",
            include_bytes!("fixtures/extended_charts/excel-authored/region-map.xlsx"),
            ChartExBuilder::region_map(formula("Sheet1!$A$1:$A$4"), values(), "en-US", "US"),
        ),
    ];
    assert!(MANIFEST.contains("Excel 16.0 build"));
    for (kind, sample, builder) in samples {
        // The committed sample is the file whose hashes were recorded.
        let section = MANIFEST
            .split_once(&format!("\"{kind}\""))
            .unwrap_or_else(|| panic!("manifest has {kind}"))
            .1;
        let section = section.split_once('}').unwrap().0;
        let style = entry(sample, "xl/charts/style1.xml");
        let colors = entry(sample, "xl/charts/colors1.xml");
        for hash in [sha256(sample), sha256(&style), sha256(&colors)] {
            assert!(section.contains(&hash), "{kind}: {hash} not in manifest");
        }
        let package = created(builder);
        assert_eq!(
            entry(&package, "xl/charts/style1.xml"),
            style,
            "{kind} style part"
        );
        assert_eq!(
            entry(&package, "xl/charts/colors1.xml"),
            colors,
            "{kind} colour part"
        );
        assert_eq!(
            requires(&entry(&package, "xl/drawings/drawing1.xml")),
            requires(&entry(sample, "xl/drawings/drawing1.xml")),
            "{kind} Requires namespace"
        );
        assert!(section.contains(&format!(
            "\"{}\"",
            requires(&entry(&package, "xl/drawings/drawing1.xml"))
        )));
    }
}
