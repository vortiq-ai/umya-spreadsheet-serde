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
    caches::pivot,
    conversion::{
        chart,
        chart_part,
        convert,
        replaced,
        replaced_all,
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

/// The axis Excel writes for the radar's dates at irregular spacing: the same
/// date axis by days.
fn daily_axis() -> String {
    monthly_axis().replace(
        r#"<c:baseTimeUnit val="months"/>"#,
        r#"<c:baseTimeUnit val="days"/>"#,
    )
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
/// not monthly, and their smallest gap is 28 days: Excel gives them months;
/// the table does not hold that cell yet.
#[test]
fn the_1900_date_system_keeps_29_february_1900() {
    let mut book = radar_with_categories([1.0, 32.0, 61.0, 92.0, 122.0, 153.0], false);
    assert_eq!(line_date_axis(&mut book), monthly_axis());
    let mut book = radar_with_categories([1.0, 32.0, 60.0, 92.0, 122.0, 153.0], false);
    assert_refused(&mut book);
}

/// A serial falls on the day its whole part names, the day the measured
/// classes were filed by: 44926.999995 is the end of 31 December 2022, not
/// 1 January 2023, so categories that start there are not monthly, and their
/// smallest gap is 28 days: Excel gives them months; the table does not hold
/// that cell yet.
#[test]
fn a_serial_falls_on_the_day_of_its_whole_part() {
    let months = [44927.0, 44958.0, 44986.0, 45017.0, 45047.0, 45078.0];
    let mut book = radar_with_categories(months, false);
    assert_eq!(line_date_axis(&mut book), monthly_axis());
    let mut late = months;
    late[0] = 44926.999995;
    let mut book = radar_with_categories(late, false);
    assert_refused(&mut book);
}

/// Dates a week apart have a smallest gap under 28 days, so they take the
/// irregular dates' cell: Excel gives them a base unit of days.
#[test]
fn weekly_dates_get_a_date_axis_by_days() {
    let weeks = [44927.0, 44934.0, 44941.0, 44948.0, 44955.0, 44962.0];
    let mut book = radar_with_categories(weeks, false);
    assert_eq!(line_date_axis(&mut book), daily_axis());
}

/// Dates a year apart have a smallest gap of 365 days or more, which Excel
/// gives a base unit of years: no cell holds it yet, so they are refused.
#[test]
fn yearly_dates_wait_for_a_measured_cell() {
    let years = [44927.0, 45292.0, 45658.0, 46023.0, 46388.0, 46753.0];
    let mut book = radar_with_categories(years, false);
    assert_refused(&mut book);
}

/// Dates that repeat a day have no gap Excel's base unit was measured on, so
/// they are refused.
#[test]
fn repeated_dates_wait_for_a_measured_cell() {
    let pairs = [44927.0, 44927.0, 44958.0, 44958.0, 44986.0, 44986.0];
    let mut book = radar_with_categories(pairs, false);
    assert_refused(&mut book);
}

/// The gap rule reads the day a serial's whole part names: a time of day
/// before the first date leaves a smallest gap of 28 whole days, which Excel
/// gives months, so the categories are refused.
#[test]
fn a_time_of_day_does_not_shorten_the_smallest_gap() {
    let dates = [45000.5, 45028.0, 45056.0, 45084.0, 45112.0, 45140.0];
    let mut book = radar_with_categories(dates, false);
    assert_refused(&mut book);
}

/// Converting `book`'s radar to a line chart is refused and leaves it as it
/// was.
fn assert_refused(book: &mut Workbook) {
    let before = typed(book);
    assert_eq!(
        convert(book, ChartSubtype::Line),
        Err(refused(ChartSubtype::Radar, ChartSubtype::Line))
    );
    assert_eq!(typed(book), before);
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

/// A PivotChart stores its category cache, so the format code it is read by
/// is the cache's own: a format code inside another vendor's extension is
/// none, and the categories read as a cache without one does, not as dates.
#[test]
fn a_format_code_is_the_one_its_owner_holds() {
    let dated = pivot(&part_xml(MONTHLY_RADAR));
    assert_eq!(line_date_axis(&mut workbook_with(&dated)), monthly_axis());
    let code = r"<c:formatCode>yyyy\-mm\-dd</c:formatCode>";
    let categories_end = "</c:numCache></c:numRef></c:cat>";
    let uncoded = replaced_all(&dated, &format!("<c:numCache>{code}"), "<c:numCache>");
    let vendor_code = concat!(
        r#"<c:extLst><c:ext uri="{00000000-0000-0000-0000-0000000000C3}" "#,
        r#"xmlns:vendor="urn:example:vendor"><vendor:formatCode>yyyy\-mm\-dd"#,
        r#"</vendor:formatCode></c:ext></c:extLst>"#
    );
    let vendor = replaced_all(
        &uncoded,
        categories_end,
        &format!("{vendor_code}{categories_end}"),
    );
    let mut without = workbook_with(&uncoded);
    let mut with = workbook_with(&vendor);
    let kept = concat!(
        r#"<vendor:formatCode>yyyy\-mm\-dd</vendor:formatCode></c:ext></c:extLst>"#,
        "</c:numCache></c:numRef></c:cat>"
    );
    assert_eq!(chart_part(&with).matches(kept).count(), 3);
    let expected = convert(&mut without, ChartSubtype::Line);
    let monthly = monthly_axis();
    assert_ne!(
        element(&chart_part(&without), "c:dateAx"),
        Some(monthly.as_str())
    );
    assert_eq!(convert(&mut with, ChartSubtype::Line), expected);
    assert_eq!(
        without_category_extensions(&chart_part(&with)),
        chart_part(&without)
    );
}

/// `xml` without the extension list that closes each category number cache.
fn without_category_extensions(xml: &str) -> String {
    let end = "</c:numCache></c:numRef></c:cat>";
    let mut pieces: Vec<&str> = xml.split(end).collect();
    let last = pieces.len() - 1;
    for piece in &mut pieces[..last] {
        if piece.ends_with("</c:extLst>") {
            *piece = &piece[..piece.rfind("<c:extLst").unwrap()];
        }
    }
    pieces.join(end)
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
