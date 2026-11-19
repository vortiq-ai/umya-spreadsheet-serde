//! The classes a source is selected by, read the way Excel reads them: dates
//! in the workbook's date system on the day their whole part names, numbers
//! compared exactly, the generated accent of any series index, and a value
//! count from its own owner.
use umya_spreadsheet::{
    Workbook,
    helper::chart_conversion::{
        ChartConversionRefusal,
        ChartSource,
        ChartSubtype,
        PaintColor,
    },
};

use super::{
    conversion::{
        chart,
        chart_part,
        convert,
        replaced,
    },
    measured::part_xml,
    workbook::workbook_with,
};

/// Three series of first-of-month categories, 2023-01-01 to 2023-06-01, in
/// `Data!$AN$2:$AN$7`.
const MONTHLY_RADAR: &str = "before/conversions5/monthly-dates/radar.chart.xml";
const CATEGORY_COLUMN: u32 = 40;

/// The chart as a typed value.
fn typed(book: &mut Workbook) -> String {
    format!("{:?}", chart(book))
}

fn refused(source: ChartSubtype, target: ChartSubtype) -> ChartConversionRefusal {
    ChartConversionRefusal::Unsupported {
        from: Some(ChartSource::Subtype(source)),
        to:   target,
    }
}

/// The element `name` of `xml` from its start tag to its end tag.
fn element<'a>(xml: &'a str, name: &str) -> Option<&'a str> {
    let start = xml.find(&format!("<{name}>"))?;
    let end = start + xml[start..].find(&format!("</{name}>"))? + name.len() + 3;
    Some(&xml[start..end])
}

/// `xml` with its `at`th series passed through `edit`.
fn with_series(xml: &str, at: usize, edit: impl FnOnce(&str) -> String) -> String {
    let start = xml.match_indices("<c:ser>").nth(at).unwrap().0;
    let end = start + xml[start..].find("</c:ser>").unwrap();
    format!("{}{}{}", &xml[..start], edit(&xml[start..end]), &xml[end..])
}

/// `series`, whose `c:idx` is `from`, with the index `to`.
fn renumbered(series: &str, from: usize, to: usize) -> String {
    replaced(
        series,
        &format!(r#"<c:ser><c:idx val="{from}"/>"#),
        &format!(r#"<c:ser><c:idx val="{to}"/>"#),
    )
}

/// The monthly radar with its categories set to `serials`, in a workbook of
/// the 1904 date system when `date1904` holds.
fn radar_with_categories(serials: [f64; 6], date1904: bool) -> Workbook {
    let mut book = workbook_with(&part_xml(MONTHLY_RADAR));
    book.set_date1904(date1904);
    book.sheet_mut(0).unwrap().chart_collection_mut()[0]
        .chart_space_mut()
        .date1904_mut()
        .unwrap()
        .set_val(date1904);
    let sheet = book.sheet_mut(0).unwrap();
    for (row, serial) in (2..).zip(serials) {
        sheet
            .cell_mut((CATEGORY_COLUMN, row))
            .set_value_number(serial);
    }
    book
}

/// The date axis a conversion of `book` to a line chart writes.
fn line_date_axis(book: &mut Workbook) -> String {
    convert(book, ChartSubtype::Line).unwrap();
    element(&chart_part(book), "c:dateAx").unwrap().to_owned()
}

/// The axis Excel writes for the measured monthly radar: a date axis by
/// months.
fn monthly_axis() -> String {
    let axis = line_date_axis(&mut workbook_with(&part_xml(MONTHLY_RADAR)));
    assert!(axis.contains(r#"<c:baseTimeUnit val="months"/>"#), "{axis}");
    axis
}

/// 1904-01-01 to 1904-06-01, 1904 being a leap year, are serials 0, 31, 60,
/// 91, 121 and 152 of the 1904 date system.
#[test]
fn a_1904_workbook_reads_its_dates_in_the_1904_system() {
    let mut book = radar_with_categories([0.0, 31.0, 60.0, 91.0, 121.0, 152.0], true);
    assert_eq!(line_date_axis(&mut book), monthly_axis());
}

/// Excel's 1900 system counts 1900-01-01 as 1 and keeps a 29 February 1900 at
/// 60, so 1 March 1900 is 61. Categories with 60 in place of 61 are therefore
/// not monthly: a spacing no round measured for this pair.
#[test]
fn the_1900_date_system_keeps_29_february_1900() {
    let mut book = radar_with_categories([1.0, 32.0, 61.0, 92.0, 122.0, 153.0], false);
    assert_eq!(line_date_axis(&mut book), monthly_axis());
    let mut book = radar_with_categories([1.0, 32.0, 60.0, 92.0, 122.0, 153.0], false);
    let before = typed(&mut book);
    assert_eq!(
        convert(&mut book, ChartSubtype::Line),
        Err(refused(ChartSubtype::Radar, ChartSubtype::Line))
    );
    assert_eq!(typed(&mut book), before);
}

/// A serial falls on the day its whole part names, the day the measured
/// classes were filed by: 44926.999995 is the end of 31 December 2022, not
/// 1 January 2023, so categories that start there are not monthly.
#[test]
fn a_serial_falls_on_the_day_of_its_whole_part() {
    let months = [44927.0, 44958.0, 44986.0, 45017.0, 45047.0, 45078.0];
    let mut book = radar_with_categories(months, false);
    assert_eq!(line_date_axis(&mut book), monthly_axis());
    let mut late = months;
    late[0] = 44926.999995;
    let mut book = radar_with_categories(late, false);
    let before = typed(&mut book);
    assert_eq!(
        convert(&mut book, ChartSubtype::Line),
        Err(refused(ChartSubtype::Radar, ChartSubtype::Line))
    );
    assert_eq!(typed(&mut book), before);
}

/// General x values count from one only when they are exactly 1 to N: the
/// double just below 1 makes them General numbers of another spacing, a class
/// round 8 measured that the rule table does not hold yet.
#[test]
fn general_x_values_count_from_one_exactly() {
    let xml = part_xml("before/conversions2/bare/scatterLinesNoMarkers.chart.xml");
    let target = ChartSubtype::ScatterLinesWithMarkers;
    convert(&mut workbook_with(&xml), target).unwrap();
    let mut book = workbook_with(&xml);
    let below_one = f64::from_bits(1.0_f64.to_bits() - 1);
    assert_eq!(below_one.to_string(), "0.9999999999999999");
    book.sheet_mut(0)
        .unwrap()
        .cell_mut((6, 2))
        .set_value_number(below_one);
    let before = typed(&mut book);
    assert_eq!(
        convert(&mut book, target),
        Err(refused(ChartSubtype::ScatterLines, target))
    );
    assert_eq!(typed(&mut book), before);
}

/// The accent Excel generates for series 51, in its ninth luminance tier.
const ACCENT4_TIER_NINE: &str = concat!(
    r#"<a:schemeClr val="accent4"><a:lumMod val="50000"/>"#,
    r#"<a:lumOff val="50000"/></a:schemeClr>"#
);
const ACCENT1: &str = r#"<a:schemeClr val="accent1"/>"#;

/// A series painted in the accent Excel generates for its index is a series
/// accent at any index: it takes the accent rule, here the only one
/// measured, and its paint is reported so. The luminance tiers repeat past
/// the ninth: index 51 is accent 4 in the ninth tier, index 54 accent 1 in
/// the first, plain.
#[test]
fn series_accents_past_the_measured_indices_are_series_accents() {
    let xml = part_xml("before/conversions2/rich/surfaceTopWireframe.chart.xml");
    let target = ChartSubtype::Contour;
    let accents = |xml: &str| {
        let mut xml = xml.to_owned();
        for (at, index, accent) in [
            (0, 51, ACCENT4_TIER_NINE),
            (1, 54, ACCENT1),
            (2, 56, r#"<a:schemeClr val="accent3"/>"#),
        ] {
            xml = with_series(&xml, at, |series| {
                let own = format!(r#"<a:schemeClr val="accent{}"/>"#, at + 1);
                replaced(&renumbered(series, at, index), &own, accent)
            });
        }
        xml
    };
    let mut measured = workbook_with(&xml);
    let expected = convert(&mut measured, target).unwrap();
    assert_eq!(expected.series_paint().len(), 3, "{expected:?}");
    assert!(
        expected
            .series_paint()
            .iter()
            .all(|paint| paint.color == PaintColor::SeriesAccent),
        "{expected:?}"
    );
    let mut book = workbook_with(&accents(&xml));
    assert_eq!(convert(&mut book, target), Ok(expected));
    assert_eq!(chart_part(&book), accents(&chart_part(&measured)));
}

/// A literal of values whose count sits only in another vendor's extension
/// has no count, so its series cannot take one bubble size per point; with
/// its own `ptCount` it does.
#[test]
fn a_value_count_is_the_one_its_owner_holds() {
    let xml = part_xml("before/conversions2/bare/colClustered.chart.xml");
    let points = concat!(
        r#"<c:pt idx="0"><c:v>5</c:v></c:pt><c:pt idx="1"><c:v>7</c:v></c:pt>"#,
        r#"<c:pt idx="2"><c:v>9</c:v></c:pt>"#
    );
    let vendor_count = concat!(
        r#"<c:extLst><c:ext uri="{00000000-0000-0000-0000-0000000000C3}" "#,
        r#"xmlns:vendor="urn:example:vendor"><vendor:ptCount val="3"/></c:ext></c:extLst>"#
    );
    let literal = |count: &str| {
        let mut edited = xml.clone();
        let values = [
            "<c:val><c:numLit><c:formatCode>General</c:formatCode>",
            count,
            points,
            vendor_count,
            "</c:numLit></c:val>",
        ]
        .concat();
        for column in ['B', 'C', 'D'] {
            let start = edited
                .find(&format!("<c:val><c:numRef><c:f>Data!${column}$2"))
                .unwrap();
            let end = start + edited[start..].find("</c:val>").unwrap() + "</c:val>".len();
            edited.replace_range(start..end, &values);
        }
        edited
    };
    let target = ChartSubtype::Bubble;
    let mut uncounted = workbook_with(&literal(""));
    let before = typed(&mut uncounted);
    assert_eq!(
        convert(&mut uncounted, target),
        Err(refused(ChartSubtype::ClusteredColumn, target))
    );
    assert_eq!(typed(&mut uncounted), before);
    let mut counted = workbook_with(&literal(r#"<c:ptCount val="3"/>"#));
    convert(&mut counted, target).unwrap();
    let sizes = concat!(
        r#"<c:bubbleSize><c:numLit><c:formatCode>General</c:formatCode><c:ptCount val="3"/>"#,
        r#"<c:pt idx="0"><c:v>1</c:v></c:pt><c:pt idx="1"><c:v>1</c:v></c:pt>"#,
        r#"<c:pt idx="2"><c:v>1</c:v></c:pt></c:numLit></c:bubbleSize>"#
    );
    let converted = chart_part(&counted);
    assert_eq!(converted.matches(sizes).count(), 3, "{converted}");
}
