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
/// The same categories formatted `mmm\-yy`, a code Excel was measured on only
/// on firsts of consecutive months, so whether dates are such firsts decides
/// whether they convert.
const MONTH_YEAR_RADAR: &str = "before/conversions9/date-format/radar@bare-3s-date-mmmyy.chart.xml";
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

/// The radar `fixture` with its categories set to `serials`, in a workbook of
/// the 1904 date system when `date1904` holds.
fn radar_with_categories(fixture: &str, serials: [f64; 6], date1904: bool) -> Workbook {
    let mut book = workbook_with(&part_xml(fixture));
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

/// The axis Excel writes for the radar's dates a year or more apart: the
/// same date axis by years.
fn yearly_axis() -> String {
    monthly_axis().replace(
        r#"<c:baseTimeUnit val="months"/>"#,
        r#"<c:baseTimeUnit val="years"/>"#,
    )
}

/// The axis Excel writes for the `mmm\-yy` radar on a line: a date axis by
/// months in that code
/// (`before/conversions9/date-format/radar@bare-3s-date-mmmyy.chart.xml` into
/// `line`).
fn month_year_axis() -> String {
    let axis = line_date_axis(&mut workbook_with(&part_xml(MONTH_YEAR_RADAR)));
    assert!(axis.contains(r#"<c:baseTimeUnit val="months"/>"#), "{axis}");
    assert!(axis.contains(r#"formatCode="mmm\-yy""#), "{axis}");
    axis
}

/// 1904-01-01 to 1904-06-01, 1904 being a leap year, are serials 0, 31, 60,
/// 91, 121 and 152 of the 1904 date system: firsts of consecutive months,
/// where dates formatted `mmm\-yy` convert. Read in the 1900 system they are
/// not, and wait.
#[test]
fn a_1904_workbook_reads_its_dates_in_the_1904_system() {
    let serials = [0.0, 31.0, 60.0, 91.0, 121.0, 152.0];
    let mut book = radar_with_categories(MONTH_YEAR_RADAR, serials, true);
    assert_eq!(line_date_axis(&mut book), month_year_axis());
    let mut book = radar_with_categories(MONTH_YEAR_RADAR, serials, false);
    assert_refused(&mut book);
}

/// Excel's 1900 system counts 1900-01-01 as 1 and keeps a 29 February 1900 at
/// 60, so 1 March 1900 is 61. As ISO dates both sets convert, since a
/// smallest gap of 28 days or more gives a date axis by months (the radars
/// `spacing-serials-1-32-61` and `spacing-serials-1-32-60` of
/// `before/conversions9c/spacing` into `line`); formatted `mmm\-yy` they
/// convert with 61 and wait with 60.
#[test]
fn the_1900_date_system_keeps_29_february_1900() {
    let firsts = [1.0, 32.0, 61.0, 92.0, 122.0, 153.0];
    let leap = [1.0, 32.0, 60.0, 92.0, 122.0, 153.0];
    for serials in [firsts, leap] {
        let mut book = radar_with_categories(MONTHLY_RADAR, serials, false);
        assert_eq!(line_date_axis(&mut book), monthly_axis(), "{serials:?}");
    }
    let mut book = radar_with_categories(MONTH_YEAR_RADAR, firsts, false);
    assert_eq!(line_date_axis(&mut book), month_year_axis());
    let mut book = radar_with_categories(MONTH_YEAR_RADAR, leap, false);
    assert_refused(&mut book);
}

/// A serial falls on the day its whole part names: 44926.999995 is the end of
/// 31 December 2022, not 1 January 2023, so categories that start there are
/// not firsts of months. As ISO dates their smallest gap in whole days, 28,
/// gives months
/// (`before/conversions9c/spacing/radar@bare-3s-spacing-late-start.chart.xml`
/// into `line`); formatted `mmm\-yy` they wait.
#[test]
fn a_serial_falls_on_the_day_of_its_whole_part() {
    let months = [44927.0, 44958.0, 44986.0, 45017.0, 45047.0, 45078.0];
    let mut late = months;
    late[0] = 44926.999995;
    let mut book = radar_with_categories(MONTHLY_RADAR, late, false);
    assert_eq!(line_date_axis(&mut book), monthly_axis());
    let mut book = radar_with_categories(MONTH_YEAR_RADAR, months, false);
    assert_eq!(line_date_axis(&mut book), month_year_axis());
    let mut book = radar_with_categories(MONTH_YEAR_RADAR, late, false);
    assert_refused(&mut book);
}

/// Dates a week apart have a smallest gap under 28 days, so they take the
/// irregular dates' cell: Excel gives them a base unit of days.
#[test]
fn weekly_dates_get_a_date_axis_by_days() {
    let weeks = [44927.0, 44934.0, 44941.0, 44948.0, 44955.0, 44962.0];
    let mut book = radar_with_categories(MONTHLY_RADAR, weeks, false);
    assert_eq!(line_date_axis(&mut book), daily_axis());
}

/// Dates whose smallest gap is 365 days or more get a date axis by years, and
/// a gap of 364 days still one by months
/// (`before/conversions9c/spacing/radar@bare-3s-spacing-year-1st.chart.xml`,
/// `before/conversions9c/spacing/radar@bare-3s-spacing-gap-365.chart.xml` and
/// `before/conversions9c/spacing/radar@bare-3s-spacing-gap-364.chart.xml` into
/// `line`).
#[test]
fn yearly_dates_get_a_date_axis_by_years() {
    for (serials, expected) in [
        (
            [44927.0, 45292.0, 45658.0, 46023.0, 46388.0, 46753.0],
            yearly_axis(),
        ),
        (
            [45000.0, 45730.0, 46095.0, 47190.0, 47920.0, 49380.0],
            yearly_axis(),
        ),
        (
            [45000.0, 45728.0, 46092.0, 47184.0, 47912.0, 49368.0],
            monthly_axis(),
        ),
    ] {
        let mut book = radar_with_categories(MONTHLY_RADAR, serials, false);
        assert_eq!(line_date_axis(&mut book), expected, "{serials:?}");
    }
}

/// Dates that repeat a day have no smallest gap Excel's base unit was
/// measured on: no round converted them, and on a line, where the measured
/// classes' axes differ, they wait.
#[test]
fn repeated_dates_wait_for_a_measured_cell() {
    let pairs = [44927.0, 44927.0, 44958.0, 44958.0, 44986.0, 44986.0];
    let mut book = radar_with_categories(MONTHLY_RADAR, pairs, false);
    assert_refused(&mut book);
}

/// The gap rule reads the day a serial's whole part names: a time of day
/// before the first date leaves a smallest gap of 28 whole days, which Excel
/// gives months
/// (`before/conversions9c/spacing/radar@bare-3s-spacing-gap-28.chart.xml` into
/// `line`), not the 27.5 days that would give days, as 27 do
/// (`before/conversions9c/spacing/radar@bare-3s-spacing-gap-27.chart.xml`).
/// Both measured gaps are whole days: Excel was not measured on a time of day,
/// so months here are inferred from whole days, not measured.
#[test]
fn a_time_of_day_does_not_shorten_the_smallest_gap() {
    let dates = [45000.5, 45028.0, 45056.0, 45084.0, 45112.0, 45140.0];
    let mut book = radar_with_categories(MONTHLY_RADAR, dates, false);
    assert_eq!(line_date_axis(&mut book), monthly_axis());
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
