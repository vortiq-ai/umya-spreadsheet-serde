//! What a conversion reads from the categories and the bands as Excel's
//! measurements file them: the data class every series' categories share
//! admits a pair, the categories a created axis' template is chosen by, and
//! the bands a surface group holds.
use umya_spreadsheet::{
    Workbook,
    helper::chart_conversion::{
        ChartConversionRefusal,
        ChartSource,
        ChartSubtype,
    },
};

use super::{
    conversion::{
        chart,
        chart_part,
        convert,
        replaced_between,
        without_between,
    },
    measured::part_xml,
    workbook::workbook_with,
};

/// ISO dates, as Excel's measured date categories are formatted.
const ISO_DATES: &str = r"yyyy\-mm\-dd";

fn typed(book: &mut Workbook) -> String {
    format!("{:?}", chart(book))
}

/// Fails unless converting `xml` to `target` is refused as unsupported from
/// `source`, with the chart unchanged.
fn assert_refused(xml: &str, source: ChartSubtype, target: ChartSubtype, what: &str) {
    let mut book = workbook_with(xml);
    let before = typed(&mut book);
    assert_eq!(
        convert(&mut book, target),
        Err(ChartConversionRefusal::Unsupported {
            from: Some(ChartSource::Subtype(source)),
            to:   target,
        }),
        "{what}"
    );
    assert_eq!(
        typed(&mut book),
        before,
        "{what}: the refusal changed the chart"
    );
}

/// Categories of six numbers in `column` of `Data`, formatted `format`.
fn number_categories(column: char, format: &str, values: [u32; 6]) -> String {
    let points: String = values
        .iter()
        .enumerate()
        .map(|(index, value)| format!(r#"<c:pt idx="{index}"><c:v>{value}</c:v></c:pt>"#))
        .collect();
    let reference = format!("Data!${column}$2:${column}$7");
    [
        "<c:cat><c:numRef><c:f>",
        &reference,
        "</c:f><c:numCache><c:formatCode>",
        format,
        r#"</c:formatCode><c:ptCount val="6"/>"#,
        &points,
        "</c:numCache></c:numRef></c:cat>",
    ]
    .concat()
}

/// `xml` with the categories of its series at `positions` replaced by
/// `categories`.
fn with_categories(xml: &str, positions: &[usize], categories: &str) -> String {
    let mut edited = xml.to_owned();
    for &at in positions {
        let series = edited.match_indices("<c:ser>").nth(at).unwrap().0;
        let start = series + edited[series..].find("<c:cat>").unwrap();
        let end = start + edited[start..].find("</c:cat>").unwrap() + "</c:cat>".len();
        edited.replace_range(start..end, categories);
    }
    edited
}

/// The element `name` of `xml` from its start tag to its end tag.
fn element(xml: &str, name: &str) -> String {
    let start = xml.find(&format!("<{name}>")).unwrap();
    let end = start + xml[start..].find(&format!("</{name}>")).unwrap() + name.len() + 3;
    xml[start..end].to_owned()
}

/// Every measured column chart has text categories. A pie made from General
/// numbers, or from a mixture of text and dates, is a class no round
/// converted, though a mixture would fold to `other` elsewhere.
#[test]
fn categories_of_a_class_no_round_converted_wait_for_one() {
    let xml = part_xml("before/conversions2/bare/colClustered.chart.xml");
    let (source, target) = (ChartSubtype::ClusteredColumn, ChartSubtype::Pie);
    convert(&mut workbook_with(&xml), target).unwrap();
    let general = number_categories('A', "General", [1, 2, 3, 4, 5, 6]);
    assert_refused(
        &with_categories(&xml, &[0, 1, 2], &general),
        source,
        target,
        "General numbers",
    );
    let dates = number_categories('N', ISO_DATES, [45000, 45001, 45002, 45003, 45004, 45005]);
    assert_refused(
        &with_categories(&xml, &[1, 2], &dates),
        source,
        target,
        "text, then dates",
    );
}

/// Excel files a created axis under the data class of the categories and
/// the spacing of the first numeric ones: daily dates followed by monthly
/// ones take the daily row's date axis.
#[test]
fn a_created_axis_follows_the_first_numeric_categories() {
    let xml = part_xml("before/conversions2/bare/barOfPie@ohlc.chart.xml");
    let target = ChartSubtype::OpenHighLowClose;
    let mut daily = workbook_with(&xml);
    convert(&mut daily, target).unwrap();
    let expected = element(&chart_part(&daily), "c:dateAx");
    let months = [44927, 44958, 44986, 45017, 45047, 45078];
    let monthly = number_categories('Z', ISO_DATES, months);
    let mut book = workbook_with(&with_categories(&xml, &[1, 2, 3], &monthly));
    convert(&mut book, target).unwrap();
    assert_eq!(element(&chart_part(&book), "c:dateAx"), expected);
}

/// The band cell Excel was measured on is keyed by the bands a surface group
/// holds: an empty band list holds none, a class no round converted, and a
/// group without one is refused before any rule is read.
#[test]
fn surface_bands_are_read_by_the_bands_the_group_holds() {
    let xml = part_xml("before/conversions2/bare/surface.chart.xml");
    let (source, target) = (ChartSubtype::Surface3D, ChartSubtype::WireframeSurface3D);
    convert(&mut workbook_with(&xml), target).unwrap();
    let empty = replaced_between(&xml, "<c:bandFmts>", "</c:bandFmts>", "<c:bandFmts/>");
    assert_refused(&empty, source, target, "an empty band list");
    let absent = without_between(&xml, "<c:bandFmts>", "</c:bandFmts>");
    assert_refused(&absent, source, target, "no band list");
}
