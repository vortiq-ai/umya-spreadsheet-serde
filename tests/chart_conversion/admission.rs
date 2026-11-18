//! The checks that run before a conversion is replayed, the parts it keeps
//! that the rule table does not name, and what its report and its data roles
//! hold. Each case starts from a measured part that converts and changes one
//! thing about it.
use umya_spreadsheet::{
    drawing::charts::{
        ChartGroupMut,
        SeriesPartKind,
    },
    helper::chart_conversion::{
        ChangeKind,
        ChartConversionRefusal,
        ChartSubtype,
        ConvertedPart,
    },
};

use super::{
    conversion::{
        chart,
        chart_part,
        convert,
        replaced,
        replaced_all,
    },
    measured::{
        Part,
        measured,
        read_part,
    },
    workbook::workbook_with,
};

fn part(variant: &str, source: &str, series: Option<usize>) -> &'static Part {
    measured()
        .parts
        .iter()
        .find(|part| {
            part.variant == variant
                && part.subtype.as_deref() == Some(source)
                && series.is_none_or(|series| part.series == series)
        })
        .unwrap_or_else(|| panic!("no {variant} {source} part"))
}

/// The first target the chart in `xml` converts to with a change.
fn converting_target(xml: &str) -> ChartSubtype {
    *ChartSubtype::ALL
        .iter()
        .find(|target| {
            convert(&mut workbook_with(xml), **target)
                .is_ok_and(|conversion| !conversion.changes().is_empty())
        })
        .expect("the measured part converts to some target")
}

/// `edited` is refused as `Unsupported` for `target` and keeps its bytes.
fn refused_unchanged(edited: &str, target: ChartSubtype, what: &str) {
    let mut book = workbook_with(edited);
    let before = chart_part(&book);
    let refusal = convert(&mut book, target).err();
    assert!(
        matches!(refusal, Some(ChartConversionRefusal::Unsupported { to, .. }) if to == target),
        "{what}: {refusal:?}"
    );
    assert_eq!(
        chart_part(&book),
        before,
        "{what}: the refusal changed the chart"
    );
}

fn bare_clustered_column() -> String {
    read_part(part("bare", "ClusteredColumn", None))
}

#[test]
fn a_chart_without_series_is_refused_and_left_alone() {
    let xml = bare_clustered_column();
    let target = converting_target(&xml);
    let mut edited = xml;
    while let Some(start) = edited.find("<c:ser>") {
        let end = start + edited[start..].find("</c:ser>").unwrap() + "</c:ser>".len();
        edited.replace_range(start..end, "");
    }
    refused_unchanged(&edited, target, "no series");
}

#[test]
fn a_surface_without_band_formats_is_refused_and_left_alone() {
    let xml = read_part(part("bare", "Surface3D", None));
    let target = converting_target(&xml);
    let start = xml.find("<c:bandFmts").unwrap();
    let end = xml.find("</c:bandFmts>").map_or_else(
        || start + xml[start..].find("/>").unwrap() + 2,
        |end| end + "</c:bandFmts>".len(),
    );
    let edited = format!("{}{}", &xml[..start], &xml[end..]);
    refused_unchanged(&edited, target, "no band formats");
}

/// Section 4.2: an automatic wireframe surface converts with three series;
/// with two it is a shape no round measured.
#[test]
fn an_unmeasured_restricted_shape_waits_and_leaves_the_chart_alone() {
    let xml = read_part(part("bare", "WireframeSurface3D", Some(3)));
    let target = converting_target(&xml);
    let last = xml.rfind("<c:ser>").unwrap();
    let end = last + xml[last..].find("</c:ser>").unwrap() + "</c:ser>".len();
    let edited = format!("{}{}", &xml[..last], &xml[end..]);
    refused_unchanged(&edited, target, "two automatic wireframe series");
}

/// Two series with one `c:idx` cannot be told apart; series whose `c:order`
/// differs from their plot-area order are a class no round measured.
#[test]
fn series_identity_and_order_are_checked_before_anything_changes() {
    let xml = bare_clustered_column();
    let target = converting_target(&xml);
    let duplicate = replaced(&xml, r#"<c:idx val="1"/>"#, r#"<c:idx val="0"/>"#);
    refused_unchanged(&duplicate, target, "a repeated c:idx");
    let reordered = replaced(&xml, r#"<c:order val="0"/>"#, r#"<c:order val="5"/>"#);
    refused_unchanged(&reordered, target, "c:order out of plot-area order");
}

/// A measured part by its fixture path.
fn by_fixture(fixture: &str) -> String {
    read_part(
        measured()
            .parts
            .iter()
            .find(|part| part.fixture == fixture)
            .unwrap_or_else(|| panic!("no part {fixture}")),
    )
}

/// The same scatter into scatter with lines and markers: x cells 1..6
/// convert; x cells 5..10 are a class the measured rounds ingested so far do
/// not hold, so it waits with the chart unchanged. Once the round that
/// measured it is ingested, it converts.
#[test]
fn x_values_select_by_their_cell_values() {
    let xml = by_fixture("before/conversions2/bare/scatterLinesNoMarkers.chart.xml");
    let x = "Data!$F$2:$F$7</c:f>";
    assert_eq!(
        xml.matches(x).count(),
        2,
        "both series read their x from F2:F7"
    );
    let target = ChartSubtype::ScatterLinesWithMarkers;
    convert(&mut workbook_with(&xml), target).expect("x = 1..6 converts");
    let mut book = workbook_with(&xml);
    let sheet = book.sheet_mut(0).unwrap();
    for row in 2..=7_u32 {
        sheet
            .cell_mut((6, row))
            .set_value_number(f64::from(row + 3));
    }
    let before = chart_part(&book);
    assert!(
        before[before.find(x).unwrap()..]
            .split("<c:v>")
            .nth(1)
            .is_some_and(|value| value.starts_with("5<")),
        "the x cache reads the cells"
    );
    let refusal = convert(&mut book, target).err();
    assert!(
        matches!(refusal, Some(ChartConversionRefusal::Unsupported { to, .. }) if to == target),
        "x = 5..10: {refusal:?}"
    );
    assert_eq!(
        chart_part(&book),
        before,
        "x = 5..10: the refusal changed the chart"
    );
}

/// Extensions of a group and an axis the conversion keeps come back
/// exactly, mixed content included.
#[test]
fn kept_groups_and_axes_keep_their_extensions() {
    let extension = concat!(
        r#"<c:extLst><c:ext uri="{00000000-0000-0000-0000-00000000AB12}" "#,
        r#"xmlns:t="urn:example:test"><t:mark>A<t:b/>B</t:mark></c:ext></c:extLst>"#
    );
    let xml = bare_clustered_column();
    let xml = replaced(&xml, "</c:barChart>", &format!("{extension}</c:barChart>"));
    let xml = replaced(&xml, "</c:catAx>", &format!("{extension}</c:catAx>"));
    // The marks as umya writes them, text around the child in order.
    let marks = |text: &str| -> Vec<String> {
        text.match_indices("<t:mark>")
            .map(|(at, _)| text[at..at + text[at..].find("</t:mark>").unwrap()].to_owned())
            .collect()
    };
    let unconverted = marks(&chart_part(&workbook_with(&xml)));
    assert_eq!(unconverted.len(), 2);
    assert!(
        unconverted
            .iter()
            .all(|mark| mark.starts_with("<t:mark>A<t:b") && mark.ends_with('B')),
        "{unconverted:?}"
    );
    let mut book = workbook_with(&xml);
    convert(&mut book, ChartSubtype::StackedColumn).unwrap();
    assert_eq!(marks(&chart_part(&book)), unconverted);
}

/// A part the target family does not allow is removed and reported.
#[test]
fn family_removals_are_reported() {
    let xml = replaced_all(
        &bare_clustered_column(),
        "<c:cat>",
        r#"<c:pictureOptions><c:applyToFront val="1"/></c:pictureOptions><c:cat>"#,
    );
    let mut book = workbook_with(&xml);
    assert!(chart_part(&book).contains("pictureOptions"));
    let conversion = convert(&mut book, ChartSubtype::Pie).unwrap();
    assert!(!chart_part(&book).contains("pictureOptions"));
    let removed: Vec<Option<usize>> = conversion
        .changes()
        .iter()
        .filter(|change| {
            change.part == ConvertedPart::SeriesPart(SeriesPartKind::PictureOptions)
                && change.change == ChangeKind::Removed
        })
        .map(|change| change.series_index)
        .collect();
    assert_eq!(removed, [Some(0), Some(1), Some(2)]);
}

/// Data roles that move are reported as moved, once per series, never as
/// removed, and keep their references: a line chart's categories become x
/// values, a scatter chart's x values become categories.
#[test]
fn categories_that_become_x_values_are_reported_as_moved() {
    let references = |text: &str| -> Vec<String> {
        text.match_indices("<c:f>")
            .map(|(at, _)| text[at..at + text[at..].find("</c:f>").unwrap()].to_owned())
            .collect()
    };
    let moves = [
        (
            "before/conversions2/bare/line.chart.xml",
            ChartSubtype::ScatterLines,
            (SeriesPartKind::Categories, "<c:cat>"),
            SeriesPartKind::XValues,
        ),
        (
            "before/conversions2/bare/scatterLines.chart.xml",
            ChartSubtype::Line,
            (SeriesPartKind::XValues, "<c:xVal>"),
            SeriesPartKind::Categories,
        ),
    ];
    for (fixture, target, (from, element), to) in moves {
        let xml = by_fixture(fixture);
        let series = xml.matches(element).count();
        let mut book = workbook_with(&xml);
        let before = references(&chart_part(&book));
        let conversion = convert(&mut book, target)
            .unwrap_or_else(|refusal| panic!("{fixture} to {target:?}: {refusal:?}"));
        let count = |part: SeriesPartKind, change: ChangeKind| {
            conversion
                .changes()
                .iter()
                .filter(|found| {
                    found.part == ConvertedPart::SeriesPart(part) && found.change == change
                })
                .count()
        };
        assert_eq!(
            count(from, ChangeKind::MovedTo(to)),
            series,
            "{fixture}: {:?}",
            conversion.changes()
        );
        assert_eq!(count(from, ChangeKind::Removed), 0, "{fixture}");
        let after = chart_part(&book);
        assert!(!after.contains(element), "{fixture}: {element} is left");
        assert_eq!(references(&after), before, "{fixture}");
    }
}

/// A reference whose column is zero cannot be written; the conversion
/// refuses it instead of reaching the writer.
#[test]
fn a_reference_to_column_zero_is_refused_without_writing() {
    let xml = bare_clustered_column();
    let target = converting_target(&xml);
    let mut book = workbook_with(&xml);
    let group = chart(&mut book)
        .plot_area_mut()
        .chart_groups_mut()
        .next()
        .unwrap();
    let ChartGroupMut::BarChart(group) = group else {
        panic!("a bar chart");
    };
    group.area_chart_series_list_mut().area_chart_series_mut()[0]
        .values_mut()
        .unwrap()
        .number_reference_mut()
        .formula_mut()
        .address_mut()
        .range_mut()
        .coordinate_start_col_mut()
        .unwrap()
        .set_num(0);
    let before = format!("{:?}", chart(&mut book));
    let refusal = convert(&mut book, target).err();
    assert!(
        matches!(refusal, Some(ChartConversionRefusal::Unsupported { .. })),
        "{refusal:?}"
    );
    assert_eq!(format!("{:?}", chart(&mut book)), before);
}
