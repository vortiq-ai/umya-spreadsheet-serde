//! What a conversion reads from the categories and the bands as Excel's
//! measurements file them: the data classes the series' categories hold admit
//! a pair, the categories a created axis' template is chosen by, and the bands
//! a surface group holds; and what Excel's measurement of every category class
//! makes of them.
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
        open,
        replaced_all,
        replaced_between,
        without_between,
    },
    measured::{
        Cells,
        Part,
        measured,
        part_xml,
        read_part,
    },
    workbook::{
        workbook_with,
        workbook_with_cells,
    },
    xml::{
        CHART,
        Element,
        parse,
    },
};

/// ISO dates, as Excel's measured date categories are formatted.
const ISO_DATES: &str = r"yyyy\-mm\-dd";

/// Pies of three series whose categories are six numbers in `Data!$AN$2:$AN$7`,
/// by how Excel's measurement formats them.
const ISO_DAILY_PIE: &str = "before/conversions9/iso-date/pie@bare-3s-iso-daily.chart.xml";
const ISO_MONTHLY_PIE: &str = "before/conversions9/iso-date/pie@bare-3s-iso-monthly.chart.xml";
const SHORT_DATE_PIE: &str = "before/conversions9/date-format/pie@bare-3s-date-mdy.chart.xml";
const MONTH_YEAR_PIE: &str = "before/conversions9/date-format/pie@bare-3s-date-mmmyy.chart.xml";
const GENERAL_PIE: &str = "before/conversions9/general/pie@bare-3s-general-irregular.chart.xml";
const TWO_DECIMALS_PIE: &str = "before/conversions9/date-format/pie@bare-3s-numeric-0.00.chart.xml";
const LITERAL_PIE: &str = "before/conversions9/other/pie@bare-3s-numLit.chart.xml";
/// The same pie with a number reference saved without its cache, over cells
/// formatted General.
const CACHELESS_PIE: &str = "before/conversions9/other/pie@bare-3s-numRef-nocache.chart.xml";
/// The column of `Data` those categories are in.
const CATEGORY_COLUMN: u32 = 40;

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

/// Every element `name` of `xml`, each from its start tag to its end tag.
fn elements(xml: &str, name: &str) -> Vec<String> {
    let (start, end) = (format!("<{name}>"), format!("</{name}>"));
    xml.match_indices(&start)
        .map(|(at, _)| {
            let close = at + xml[at..].find(&end).unwrap() + end.len();
            xml[at..close].to_owned()
        })
        .collect()
}

/// The measured part at `fixture`.
fn part_at(fixture: &str) -> &'static Part {
    measured()
        .parts
        .iter()
        .find(|part| part.fixture == fixture)
        .unwrap_or_else(|| panic!("no measured part {fixture}"))
}

/// The chart part `target` makes of the measured part at `fixture`, read
/// with the cells its references read.
fn converted(fixture: &str, target: ChartSubtype) -> String {
    let mut book = open(part_at(fixture));
    convert(&mut book, target).unwrap_or_else(|error| panic!("{fixture}: {error:?}"));
    chart_part(&book)
}

/// `xml` with every number format code `from` written as `to`.
fn in_code(xml: &str, from: &str, to: &str) -> String {
    replaced_all(
        xml,
        &format!(r#"formatCode="{from}""#),
        &format!(r#"formatCode="{to}""#),
    )
}

/// Every axis `name` of `xml`, each as its children in canonical form without
/// the ids of the axis and of the axis it crosses, which Excel numbers in
/// each part on its own.
fn axes_without_ids(xml: &str, name: &str) -> Vec<String> {
    parse(xml)
        .all_named(CHART, name)
        .into_iter()
        .map(|axis| {
            axis.children
                .iter()
                .filter(|child| !matches!(child.name.as_str(), "axId" | "crossAx"))
                .map(canonical)
                .collect()
        })
        .collect()
}

/// The number format id of the first category cell of `book`'s `Data` sheet;
/// ids below 164 are Excel's built-in formats.
fn category_format_id(book: &Workbook) -> u32 {
    let cell = book
        .sheet(0)
        .unwrap()
        .cell((CATEGORY_COLUMN, 2))
        .expect("a category cell");
    let format = cell.style().number_format().expect("a number format");
    format.number_format_id()
}

/// Fails unless the plot area of `xml` holds no axis.
fn assert_no_axis(xml: &str) {
    for axis in ["c:catAx", "c:dateAx", "c:valAx", "c:serAx"] {
        assert!(!xml.contains(&format!("<{axis}>")), "{axis} in {xml}");
    }
}

/// `<c:pt>` elements holding `values`, from index 0.
fn points(values: &[&str]) -> String {
    values
        .iter()
        .enumerate()
        .map(|(index, value)| format!(r#"<c:pt idx="{index}"><c:v>{value}</c:v></c:pt>"#))
        .collect()
}

/// The part `xml` with its `from` axis renamed `to` and passed through
/// `edits`.
fn edited_axis(xml: &str, from: &str, to: &str, edits: &[(&str, &str)]) -> String {
    let start = xml.find(&format!("<{from}>")).unwrap();
    let end = start + xml[start..].find(&format!("</{from}>")).unwrap() + from.len() + 3;
    let renamed = xml[start..end]
        .replacen(&format!("<{from}>"), &format!("<{to}>"), 1)
        .replacen(&format!("</{from}>"), &format!("</{to}>"), 1);
    let axis = edits.iter().fold(renamed, |axis, (old, new)| {
        assert!(axis.contains(old), "{old} not in {axis}");
        axis.replacen(old, new, 1)
    });
    format!("{}{axis}{}", &xml[..start], &xml[end..])
}

/// An element as text with its attributes in name order, which XML leaves
/// free.
fn canonical(element: &Element) -> String {
    let mut attributes = element.attributes.clone();
    attributes.sort();
    let children: String = element.children.iter().map(canonical).collect();
    format!(
        "<{{{}}}{} {attributes:?} {:?}>{children}</>",
        element.namespace, element.name, element.text
    )
}

/// The chart axis `name` of the part `xml`, canonical.
fn axis(xml: &str, name: &str) -> Option<String> {
    parse(xml).descendant_c(name).map(canonical)
}

/// Excel makes a pie of a column chart whose categories are General numbers,
/// or text in the first series and ISO dates in the others, with every
/// series' categories as they were and no axis
/// (`before/conversions9/general/colClustered@bare-3s-general-1toN.chart.xml`
/// and `before/conversions9/mixed/colClustered@bare-3s-mix-text-date.chart.xml`
/// into `pie`). Each series keeps its own categories, so text, ISO dates and
/// General numbers, one class per series, convert as each class does.
#[test]
fn categories_of_a_measured_class_convert_into_a_pie() {
    let xml = part_xml("before/conversions2/bare/colClustered.chart.xml");
    let text = elements(&xml, "c:cat").remove(0);
    let general = number_categories('A', "General", [1, 2, 3, 4, 5, 6]);
    let dates = number_categories('N', ISO_DATES, [45000, 45001, 45002, 45003, 45004, 45005]);
    // Beside text in column A, the General numbers of a mixture sit in a column
    // of their own.
    let numbers = number_categories('M', "General", [1, 2, 3, 4, 5, 6]);
    for (edited, expected, what) in [
        (
            with_categories(&xml, &[0, 1, 2], &general),
            [general.as_str(), general.as_str(), general.as_str()],
            "General numbers",
        ),
        (
            with_categories(&xml, &[1, 2], &dates),
            [text.as_str(), dates.as_str(), dates.as_str()],
            "text, then dates",
        ),
        (
            with_categories(&with_categories(&xml, &[1], &dates), &[2], &numbers),
            [text.as_str(), dates.as_str(), numbers.as_str()],
            "text, dates and General numbers",
        ),
    ] {
        let mut book = workbook_with(&edited);
        convert(&mut book, ChartSubtype::Pie).unwrap_or_else(|error| panic!("{what}: {error:?}"));
        let part = chart_part(&book);
        assert_eq!(elements(&part, "c:cat"), expected, "{what}");
        assert_no_axis(&part);
    }
}

/// No round measured numbers formatted `#,##0`, and the measured classes give
/// a line different category axes (a date axis for dates, a category axis for
/// numbers), so such categories wait on a line. A mixture waits too when one
/// series holds them: the first series' text alone would convert, as the
/// unedited chart does, and decide the axis, but every series' class must be
/// one the pair converts.
#[test]
fn categories_of_a_format_no_round_measured_wait_on_a_category_axis() {
    let xml = part_xml("before/conversions2/bare/colClustered.chart.xml");
    let (source, target) = (ChartSubtype::ClusteredColumn, ChartSubtype::Line);
    convert(&mut workbook_with(&xml), target).unwrap();
    let thousands = number_categories('N', "#,##0", [1200, 3400, 560, 7800, 910, 2300]);
    let edited = with_categories(&xml, &[0, 1, 2], &thousands);
    assert_refused(&edited, source, target, "numbers formatted #,##0");
    let mixture = with_categories(&xml, &[1, 2], &thousands);
    assert_refused(
        &mixture,
        source,
        target,
        "text, then numbers formatted #,##0",
    );
}

/// Numbers in a format no round measured keep every series' categories in a
/// pie with no axis, as Excel keeps those of every measured number class
/// there (the `bare-3s` column charts of `before/conversions9` whose
/// categories are `general-irregular`, `numeric-0.00`, `date-mdy` and `numLit`,
/// into `pie`). Beside text in the first series they convert as well, each
/// series keeping its own categories.
#[test]
fn categories_of_a_format_no_round_measured_keep_their_categories_in_a_pie() {
    let xml = part_xml("before/conversions2/bare/colClustered.chart.xml");
    let text = elements(&xml, "c:cat").remove(0);
    let thousands = number_categories('N', "#,##0", [1200, 3400, 560, 7800, 910, 2300]);
    for (edited, expected, what) in [
        (
            with_categories(&xml, &[0, 1, 2], &thousands),
            [thousands.as_str(), thousands.as_str(), thousands.as_str()],
            "numbers formatted #,##0",
        ),
        (
            with_categories(&xml, &[1, 2], &thousands),
            [text.as_str(), thousands.as_str(), thousands.as_str()],
            "text, then numbers formatted #,##0",
        ),
    ] {
        let mut book = workbook_with(&edited);
        convert(&mut book, ChartSubtype::Pie).unwrap_or_else(|error| panic!("{what}: {error:?}"));
        let part = chart_part(&book);
        assert_eq!(elements(&part, "c:cat"), expected, "{what}");
        assert_no_axis(&part);
    }
}

/// Series without categories stay without them in a pie, which has no axis
/// (`before/conversions9/absent/colClustered@bare-3s-absent.chart.xml` into
/// `pie`).
#[test]
fn absent_categories_stay_absent_in_a_pie() {
    let part = converted(
        "before/conversions9/absent/colClustered@bare-3s-absent.chart.xml",
        ChartSubtype::Pie,
    );
    assert_eq!(elements(&part, "c:ser").len(), 3);
    assert!(elements(&part, "c:cat").is_empty(), "{part}");
    assert_no_axis(&part);
}

/// A multi-level category reference keeps both levels in a doughnut, as
/// Excel wrote it
/// (`before/conversions9/other/colClustered@bare-3s-multiLvl.chart.xml`
/// into `doughnut`).
#[test]
fn multi_level_categories_keep_both_levels_in_a_doughnut() {
    let part = converted(
        "before/conversions9/other/colClustered@bare-3s-multiLvl.chart.xml",
        ChartSubtype::Doughnut,
    );
    let levels = [
        "<c:cat><c:multiLvlStrRef><c:f>Data!$AM$2:$AN$7</c:f><c:multiLvlStrCache>",
        r#"<c:ptCount val="6"/><c:lvl>"#,
        &points(&["X1", "X2", "X3", "X4", "X5", "X6"]),
        r#"</c:lvl><c:lvl><c:pt idx="0"><c:v>G1</c:v></c:pt><c:pt idx="3"><c:v>G2</c:v></c:pt>"#,
        "</c:lvl></c:multiLvlStrCache></c:multiLvlStrRef></c:cat>",
    ]
    .concat();
    assert_eq!(
        elements(&part, "c:cat"),
        [levels.as_str(), levels.as_str(), levels.as_str()]
    );
    assert_no_axis(&part);
}

/// Literal categories stay literal in a pie of pie, as Excel wrote them
/// (`before/conversions9/other/colClustered@bare-3s-numLit.chart.xml` into
/// `pieOfPie`).
#[test]
fn literal_categories_stay_literal_in_a_pie_of_pie() {
    let part = converted(
        "before/conversions9/other/colClustered@bare-3s-numLit.chart.xml",
        ChartSubtype::PieOfPie,
    );
    let literal = [
        r#"<c:cat><c:numLit><c:formatCode>General</c:formatCode><c:ptCount val="6"/>"#,
        &points(&["1.5", "3.2", "10", "0.25", "7.75", "4"]),
        "</c:numLit></c:cat>",
    ]
    .concat();
    assert_eq!(
        elements(&part, "c:cat"),
        [literal.as_str(), literal.as_str(), literal.as_str()]
    );
    assert_no_axis(&part);
}

/// A text reference saved without its cache comes back with the cache its
/// cells give, as Excel rebuilt it
/// (`before/conversions9/other/colClustered@bare-3s-strRef-nocache.chart.xml`
/// into `pie`).
#[test]
fn a_reference_without_a_cache_comes_back_with_the_cells_in_a_pie() {
    let part = converted(
        "before/conversions9/other/colClustered@bare-3s-strRef-nocache.chart.xml",
        ChartSubtype::Pie,
    );
    let text = [
        r#"<c:cat><c:strRef><c:f>Data!$AN$2:$AN$7</c:f><c:strCache><c:ptCount val="6"/>"#,
        &points(&["X1", "X2", "X3", "X4", "X5", "X6"]),
        "</c:strCache></c:strRef></c:cat>",
    ]
    .concat();
    assert_eq!(
        elements(&part, "c:cat"),
        [text.as_str(), text.as_str(), text.as_str()]
    );
    assert_no_axis(&part);
}

/// A mixture keeps each series' own categories in an exploded pie: ISO
/// dates in the first series, General numbers in the others
/// (`before/conversions9/mixed/colClustered@bare-3s-mix-date-general.chart.xml`
/// into `pieExploded`).
#[test]
fn a_mixture_keeps_each_series_categories_in_an_exploded_pie() {
    let part = converted(
        "before/conversions9/mixed/colClustered@bare-3s-mix-date-general.chart.xml",
        ChartSubtype::ExplodedPie,
    );
    let dates = number_categories('A', ISO_DATES, [45000, 45001, 45002, 45003, 45004, 45005])
        .replace("Data!$A$2:$A$7", "Data!$AN$2:$AN$7");
    let general = number_categories('A', "General", [1, 2, 3, 4, 5, 6])
        .replace("Data!$A$2:$A$7", "Data!$AL$2:$AL$7");
    assert_eq!(
        elements(&part, "c:cat"),
        [dates.as_str(), general.as_str(), general.as_str()]
    );
    assert_no_axis(&part);
}

/// ISO dates at irregular spacing make the radar's category axis a date axis
/// by days on a line, as Excel wrote it: the same axis without its label
/// alignment and multi-level flag, and with a base unit of days
/// (`before/conversions9/iso-date/radar@bare-3s-iso-irregular.chart.xml` into
/// `line`).
#[test]
fn irregular_dates_get_a_day_date_axis_on_a_line() {
    let fixture = "before/conversions9/iso-date/radar@bare-3s-iso-irregular.chart.xml";
    let excel = edited_axis(
        &part_xml(fixture),
        "c:catAx",
        "c:dateAx",
        &[
            (r#"<c:lblAlgn val="ctr"/>"#, ""),
            (
                r#"<c:noMultiLvlLbl val="0"/>"#,
                r#"<c:baseTimeUnit val="days"/>"#,
            ),
        ],
    );
    let part = converted(fixture, ChartSubtype::Line);
    let expected = axis(&excel, "dateAx").expect("Excel's axis");
    assert_eq!(axis(&part, "dateAx"), Some(expected));
    assert!(elements(&part, "c:catAx").is_empty(), "{part}");
}

/// ISO dates at irregular spacing make a column chart's date axis a category
/// axis on a radar, as Excel wrote it: the same axis with major gridlines,
/// centred labels and the multi-level flag set, and without a base unit
/// (`before/conversions9/iso-date/colClustered@bare-3s-iso-irregular.chart.xml`
/// into `radar`).
#[test]
fn irregular_dates_get_a_category_axis_on_a_radar() {
    let fixture = "before/conversions9/iso-date/colClustered@bare-3s-iso-irregular.chart.xml";
    let excel = edited_axis(
        &part_xml(fixture),
        "c:dateAx",
        "c:catAx",
        &[
            (
                r#"<c:axPos val="b"/>"#,
                r#"<c:axPos val="b"/><c:majorGridlines/>"#,
            ),
            (
                r#"<c:lblOffset val="100"/><c:baseTimeUnit val="days"/>"#,
                r#"<c:lblAlgn val="ctr"/><c:lblOffset val="100"/><c:noMultiLvlLbl val="1"/>"#,
            ),
        ],
    );
    let part = converted(fixture, ChartSubtype::Radar);
    let expected = axis(&excel, "catAx").expect("Excel's axis");
    assert_eq!(axis(&part, "catAx"), Some(expected));
    assert!(elements(&part, "c:dateAx").is_empty(), "{part}");
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

/// Dates formatted `m/d/yyyy` on consecutive days give a pie's line the date
/// axis by days Excel creates for ISO dates on consecutive days, in their own
/// code (`before/conversions9/date-format/pie@bare-3s-date-mdy.chart.xml` and
/// `before/conversions9/iso-date/pie@bare-3s-iso-daily.chart.xml` into
/// `line`).
#[test]
fn short_dates_on_consecutive_days_get_a_day_date_axis_in_their_code() {
    let iso = converted(ISO_DAILY_PIE, ChartSubtype::Line);
    let part = converted(SHORT_DATE_PIE, ChartSubtype::Line);
    assert_eq!(
        axis(&part, "dateAx"),
        axis(&in_code(&iso, ISO_DATES, "m/d/yyyy"), "dateAx")
    );
    assert!(
        element(&part, "c:dateAx").contains(r#"<c:baseTimeUnit val="days"/>"#),
        "{part}"
    );
}

/// Dates formatted `mmm\-yy` on firsts of consecutive months give a pie's line
/// the date axis by months Excel creates for ISO dates on firsts of months,
/// in their own code
/// (`before/conversions9/date-format/pie@bare-3s-date-mmmyy.chart.xml` and
/// `before/conversions9/iso-date/pie@bare-3s-iso-monthly.chart.xml` into
/// `line`).
#[test]
fn month_dates_on_firsts_of_months_get_a_month_date_axis_in_their_code() {
    let iso = converted(ISO_MONTHLY_PIE, ChartSubtype::Line);
    let part = converted(MONTH_YEAR_PIE, ChartSubtype::Line);
    assert_eq!(
        axis(&part, "dateAx"),
        axis(&in_code(&iso, ISO_DATES, r"mmm\-yy"), "dateAx")
    );
    assert!(
        element(&part, "c:dateAx").contains(r#"<c:baseTimeUnit val="months"/>"#),
        "{part}"
    );
}

/// Numbers formatted `0.00` give a pie's column chart the category axis Excel
/// creates for irregular General numbers, in their own code
/// (`before/conversions9/date-format/pie@bare-3s-numeric-0.00.chart.xml` and
/// `before/conversions9/general/pie@bare-3s-general-irregular.chart.xml` into
/// `colClustered`).
#[test]
fn numbers_in_two_decimals_get_a_category_axis_in_their_code() {
    let general = converted(GENERAL_PIE, ChartSubtype::ClusteredColumn);
    let part = converted(TWO_DECIMALS_PIE, ChartSubtype::ClusteredColumn);
    // The value axis is General too; only the category axis is compared.
    assert_eq!(
        axis(&part, "catAx"),
        axis(&in_code(&general, "General", "0.00"), "catAx")
    );
    assert!(elements(&part, "c:dateAx").is_empty(), "{part}");
}

/// Number literals stay number literals on a pie's line, which takes the
/// category axis Excel creates for General numbers, a literal's own code
/// being General
/// (`before/conversions9/other/pie@bare-3s-numLit.chart.xml` and
/// `before/conversions9/general/pie@bare-3s-general-irregular.chart.xml` into
/// `line`).
#[test]
fn number_literals_keep_their_form_on_a_line() {
    let part = converted(LITERAL_PIE, ChartSubtype::Line);
    let literal = [
        r#"<c:cat><c:numLit><c:formatCode>General</c:formatCode><c:ptCount val="6"/>"#,
        &points(&["1.5", "3.2", "10", "0.25", "7.75", "4"]),
        "</c:numLit></c:cat>",
    ]
    .concat();
    assert_eq!(
        elements(&part, "c:cat"),
        [literal.as_str(), literal.as_str(), literal.as_str()]
    );
    assert_eq!(
        axis(&part, "catAx"),
        axis(&converted(GENERAL_PIE, ChartSubtype::Line), "catAx")
    );
}

/// Number literals were measured only at irregular values: literals counting
/// 1 to N are a spacing no round measured, so on a pie's line, where the
/// measured classes' axes differ, they wait, and in a doughnut, where every
/// measured class keeps its categories, they keep theirs
/// (`before/conversions9/other/pie@bare-3s-numLit.chart.xml` into `line` and
/// `doughnut`).
#[test]
fn number_literals_counting_from_one_wait_on_a_category_axis() {
    let xml = part_xml(LITERAL_PIE);
    convert(&mut workbook_with(&xml), ChartSubtype::Line).unwrap();
    let irregular = points(&["1.5", "3.2", "10", "0.25", "7.75", "4"]);
    let counting = replaced_all(&xml, &irregular, &points(&["1", "2", "3", "4", "5", "6"]));
    assert_refused(
        &counting,
        ChartSubtype::Pie,
        ChartSubtype::Line,
        "literals counting 1 to 6",
    );
    let mut book = workbook_with(&counting);
    convert(&mut book, ChartSubtype::Doughnut).unwrap();
    let part = chart_part(&book);
    let literal = elements(&counting, "c:cat").remove(0);
    assert_eq!(
        elements(&part, "c:cat"),
        [literal.as_str(), literal.as_str(), literal.as_str()]
    );
    assert_no_axis(&part);
}

/// A volume stock takes its secondary category axis from series 2, the first
/// series of its stock group: ISO dates in series 1 and General numbers in the
/// others keep the date axis of all-ISO categories for the volume and give
/// the stock group the hidden category axis of all-General categories
/// (`before/conversions9/mixed/line@bare-4s-mix-date-general.chart.xml`,
/// `before/conversions9/iso-date/line@bare-4s-iso-daily.chart.xml` and
/// `before/conversions9/general/line@bare-4s-general-1toN.chart.xml` into
/// `stockVHLC`).
#[test]
fn a_volume_stock_takes_its_secondary_axis_from_its_second_series() {
    let target = ChartSubtype::VolumeHighLowClose;
    let mixed = converted(
        "before/conversions9/mixed/line@bare-4s-mix-date-general.chart.xml",
        target,
    );
    let dates = converted(
        "before/conversions9/iso-date/line@bare-4s-iso-daily.chart.xml",
        target,
    );
    let general = converted(
        "before/conversions9/general/line@bare-4s-general-1toN.chart.xml",
        target,
    );
    // All-ISO categories give both groups a date axis, all-General ones both a
    // category axis, each primary first.
    assert_eq!(axes_without_ids(&dates, "dateAx").len(), 2);
    assert_eq!(axes_without_ids(&general, "catAx").len(), 2);
    assert_eq!(
        axes_without_ids(&mixed, "dateAx"),
        axes_without_ids(&dates, "dateAx")[..1]
    );
    assert_eq!(
        axes_without_ids(&mixed, "catAx"),
        axes_without_ids(&general, "catAx")[1..]
    );
}

/// A reference saved without its cache, over cells in the built-in short-date
/// format 14, reads as `m/d/yyyy`, the code Excel's cache holds for those
/// cells (`before/conversions9/date-format/pie@bare-3s-date-mdy.xlsx`: the
/// cells' `numFmtId="14"`, the chart cache's `m/d/yyyy`), so it converts as
/// the cached part does.
#[test]
fn a_reference_without_a_cache_reads_the_built_in_short_date_as_excel_does() {
    let cached = part_xml(SHORT_DATE_PIE);
    let cache = element(&element(&cached, "c:cat"), "c:numCache");
    let cacheless = replaced_all(&cached, &cache, "");
    let cells = [Cells {
        formula: "Data!$AN$2:$AN$7".to_owned(),
        numeric: true,
        format:  Some("m/d/yyyy".to_owned()),
        points:  (0..6).map(|at| (at, (45000 + at).to_string())).collect(),
    }];
    let mut book = workbook_with_cells(&cacheless, &cells);
    assert_eq!(category_format_id(&book), 14);
    convert(&mut book, ChartSubtype::Line).unwrap();
    assert_eq!(
        axis(&chart_part(&book), "dateAx"),
        axis(&converted(SHORT_DATE_PIE, ChartSubtype::Line), "dateAx")
    );
}

/// A reference saved without its cache, over cells in another built-in
/// format, here 2 (`0.00`), has no code a round measured Excel rebuilding, so
/// its class is unmeasured: it keeps its categories in a doughnut, as every
/// measured class does there, and waits on a radar, where they differ
/// (`before/conversions9/date-format/pie@bare-3s-numeric-0.00.chart.xml` and
/// `before/conversions9/other/pie@bare-3s-numRef-nocache.chart.xml` into
/// `radar` and `doughnut`).
#[test]
fn a_reference_without_a_cache_in_another_built_in_format_is_an_unmeasured_class() {
    let part = part_at(CACHELESS_PIE);
    convert(&mut open(part), ChartSubtype::Radar).unwrap();
    let cells: Vec<Cells> = part
        .cells
        .iter()
        .map(|held| Cells {
            formula: held.formula.clone(),
            numeric: held.numeric,
            format:  held.numeric.then(|| "0.00".to_owned()),
            points:  held.points.clone(),
        })
        .collect();
    let mut book = workbook_with_cells(&read_part(part), &cells);
    assert_eq!(category_format_id(&book), 2);
    let before = typed(&mut book);
    assert_eq!(
        convert(&mut book, ChartSubtype::Radar),
        Err(ChartConversionRefusal::Unsupported {
            from: Some(ChartSource::Subtype(ChartSubtype::Pie)),
            to:   ChartSubtype::Radar,
        })
    );
    assert_eq!(typed(&mut book), before);
    let categories = elements(&chart_part(&book), "c:cat");
    convert(&mut book, ChartSubtype::Doughnut).unwrap();
    let converted = chart_part(&book);
    assert_eq!(elements(&converted, "c:cat"), categories);
    assert_no_axis(&converted);
}
