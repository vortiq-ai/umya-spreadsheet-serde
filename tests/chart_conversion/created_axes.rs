//! The axes a conversion creates or keeps, compared with what Excel wrote for
//! the same pair. The expected subtrees are Excel's after parts, copied as
//! text; only the axis ids and the crossing references, which a conversion
//! chooses itself, are left out of the comparison.
use umya_spreadsheet::helper::chart_conversion::ChartSubtype;

use super::{
    conversion::{
        chart_part,
        convert,
        replaced_all,
    },
    measured::part_xml,
    workbook::workbook_with,
    xml::{
        CHART,
        Element,
        parse,
    },
};

/// Excel's after part for a bare pie converted to a clustered column chart:
/// the two axes it created. Every created axis in the baseline is unstyled.
const PIE_TO_COLUMN_AXES: [&str; 2] = [
    concat!(
        r#"<c:catAx xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart">"#,
        r#"<c:axId val="1405428688"/><c:scaling><c:orientation val="minMax"/></c:scaling>"#,
        r#"<c:delete val="0"/><c:axPos val="b"/>"#,
        r#"<c:numFmt formatCode="General" sourceLinked="1"/>"#,
        r#"<c:majorTickMark val="out"/><c:minorTickMark val="none"/>"#,
        r#"<c:tickLblPos val="nextTo"/><c:crossAx val="1405429168"/>"#,
        r#"<c:auto val="1"/><c:lblAlgn val="ctr"/><c:lblOffset val="100"/>"#,
        r#"<c:noMultiLvlLbl val="0"/></c:catAx>"#
    ),
    concat!(
        r#"<c:valAx xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart">"#,
        r#"<c:axId val="1405429168"/><c:scaling><c:orientation val="minMax"/></c:scaling>"#,
        r#"<c:delete val="0"/><c:axPos val="l"/><c:majorGridlines/>"#,
        r#"<c:numFmt formatCode="General" sourceLinked="1"/>"#,
        r#"<c:majorTickMark val="out"/><c:minorTickMark val="none"/>"#,
        r#"<c:tickLblPos val="nextTo"/><c:crossAx val="1405428688"/>"#,
        r#"<c:crossBetween val="between"/></c:valAx>"#
    ),
];

/// Excel's after part for a bare pie converted to a scatter chart: two value
/// axes.
const PIE_TO_SCATTER_AXES: [&str; 2] = [
    concat!(
        r#"<c:valAx xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart">"#,
        r#"<c:axId val="1400128640"/><c:scaling><c:orientation val="minMax"/></c:scaling>"#,
        r#"<c:delete val="0"/><c:axPos val="b"/>"#,
        r#"<c:majorTickMark val="out"/><c:minorTickMark val="none"/>"#,
        r#"<c:tickLblPos val="nextTo"/><c:crossAx val="1400129600"/>"#,
        r#"<c:crossBetween val="midCat"/></c:valAx>"#
    ),
    concat!(
        r#"<c:valAx xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart">"#,
        r#"<c:axId val="1400129600"/><c:scaling><c:orientation val="minMax"/></c:scaling>"#,
        r#"<c:delete val="0"/><c:axPos val="l"/><c:majorGridlines/>"#,
        r#"<c:numFmt formatCode="General" sourceLinked="1"/>"#,
        r#"<c:majorTickMark val="out"/><c:minorTickMark val="none"/>"#,
        r#"<c:tickLblPos val="nextTo"/><c:crossAx val="1400128640"/>"#,
        r#"<c:crossBetween val="midCat"/></c:valAx>"#
    ),
];

const AXES: [&str; 4] = ["catAx", "valAx", "dateAx", "serAx"];

/// `element` and everything under it as one line, with the id and crossing
/// values blanked.
fn canonical(element: &Element) -> String {
    let attributes: String = element
        .attributes
        .iter()
        .map(|(key, value)| {
            let blanked = element.namespace == CHART
                && matches!(element.name.as_str(), "axId" | "crossAx")
                && key == "val";
            format!(" {key}={}", if blanked { "*" } else { value })
        })
        .collect();
    let children: String = element.children.iter().map(canonical).collect();
    format!(
        "<{{{}}}{}{attributes}>{}{children}</>",
        element.namespace,
        element.name,
        element.text.trim()
    )
}

/// The plot area's axes in document order, each as `canonical` spells it.
fn written_axes(xml: &str) -> Vec<String> {
    let root = parse(xml);
    let plot_area = root
        .c("chart")
        .and_then(|chart| chart.c("plotArea"))
        .expect("a chart part has a plot area");
    plot_area
        .children
        .iter()
        .filter(|child| AXES.iter().any(|axis| child.is(CHART, axis)))
        .map(canonical)
        .collect()
}

/// Where the axes `xml` writes differ from `expected`; empty when they are
/// the same elements in the same order.
fn axis_differences(xml: &str, expected: &[&str]) -> Vec<String> {
    let written = written_axes(xml);
    let expected: Vec<String> = expected
        .iter()
        .map(|axis| canonical(&parse(axis)))
        .collect();
    let mut out = Vec::new();
    if written.len() != expected.len() {
        out.push(format!(
            "{} axes written, Excel wrote {}",
            written.len(),
            expected.len()
        ));
    }
    for (index, (ours, excel)) in written.iter().zip(&expected).enumerate() {
        if ours != excel {
            out.push(format!("axis {index}: Excel {excel}, here {ours}"));
        }
    }
    out
}

fn converted_part(fixture: &str, target: ChartSubtype) -> String {
    let mut book = workbook_with(&part_xml(fixture));
    convert(&mut book, target).expect("a baseline pair converts");
    chart_part(&book)
}

/// The axes a pie gains match Excel's element for element, and the
/// comparison reports an extra `txPr` and a reordered child.
#[test]
fn created_axes_equal_excels_subtree() {
    let fixture = "before/conversions2/bare/pie.chart.xml";
    for (target, expected) in [
        (ChartSubtype::ClusteredColumn, &PIE_TO_COLUMN_AXES),
        (ChartSubtype::Scatter, &PIE_TO_SCATTER_AXES),
    ] {
        let xml = converted_part(fixture, target);
        assert_eq!(
            axis_differences(&xml, expected),
            Vec::<String>::new(),
            "{target:?}"
        );
    }

    let xml = converted_part(fixture, ChartSubtype::ClusteredColumn);
    let tick = r#"<c:tickLblPos val="nextTo"/>"#;
    let extra = replaced_all(
        &xml,
        &format!("{tick}<c:crossAx"),
        &format!("{tick}<c:txPr/><c:crossAx"),
    );
    let reordered = replaced_all(
        &xml,
        r#"<c:majorTickMark val="out"/><c:minorTickMark val="none"/>"#,
        r#"<c:minorTickMark val="none"/><c:majorTickMark val="out"/>"#,
    );
    for (what, mutated) in [("an extra txPr", extra), ("a reordered child", reordered)] {
        assert!(
            !axis_differences(&mutated, &PIE_TO_COLUMN_AXES).is_empty(),
            "{what} is not reported"
        );
    }
}

/// The rotation of the first value axis title.
fn value_title_rotation(xml: &str) -> Option<String> {
    let root = parse(xml);
    let axis = root.c("chart")?.c("plotArea")?.c("valAx")?;
    axis.c("title")?
        .descendant_c("rich")?
        .a("bodyPr")?
        .attribute("rot")
        .map(str::to_owned)
}

/// Excel keeps the value axis title of a surface when it becomes a contour,
/// rotated a quarter turn; a changed rotation is reported.
#[test]
fn a_surface_becoming_a_contour_keeps_the_value_axis_title_rotation() {
    let xml = converted_part(
        "before/conversions2/rich/surface.chart.xml",
        ChartSubtype::Contour,
    );
    assert_eq!(value_title_rotation(&xml).as_deref(), Some("-5400000"));
    let changed = replaced_all(&xml, r#"rot="-5400000""#, r#"rot="0""#);
    assert_ne!(value_title_rotation(&changed).as_deref(), Some("-5400000"));
}
