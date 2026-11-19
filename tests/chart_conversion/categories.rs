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
        replaced_between,
        without_between,
    },
    measured::{
        measured,
        part_xml,
    },
    workbook::workbook_with,
    xml::{
        Element,
        parse,
    },
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

/// The chart part `target` makes of the measured part at `fixture`, read
/// with the cells its references read.
fn converted(fixture: &str, target: ChartSubtype) -> String {
    let part = measured()
        .parts
        .iter()
        .find(|part| part.fixture == fixture)
        .unwrap_or_else(|| panic!("no measured part {fixture}"));
    let mut book = open(part);
    convert(&mut book, target).unwrap();
    chart_part(&book)
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
/// into `pie`).
#[test]
fn categories_of_a_measured_class_convert_into_a_pie() {
    let xml = part_xml("before/conversions2/bare/colClustered.chart.xml");
    let text = elements(&xml, "c:cat").remove(0);
    let general = number_categories('A', "General", [1, 2, 3, 4, 5, 6]);
    let dates = number_categories('N', ISO_DATES, [45000, 45001, 45002, 45003, 45004, 45005]);
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
    ] {
        let mut book = workbook_with(&edited);
        convert(&mut book, ChartSubtype::Pie).unwrap_or_else(|error| panic!("{what}: {error:?}"));
        let part = chart_part(&book);
        assert_eq!(elements(&part, "c:cat"), expected, "{what}");
        assert_no_axis(&part);
    }
}

/// No round converted the categories of a chart whose series hold three
/// classes: text, ISO dates and General numbers into a pie wait for one.
#[test]
fn categories_of_a_class_no_round_converted_wait_for_one() {
    let xml = part_xml("before/conversions2/bare/colClustered.chart.xml");
    let (source, target) = (ChartSubtype::ClusteredColumn, ChartSubtype::Pie);
    convert(&mut workbook_with(&xml), target).unwrap();
    let dates = number_categories('N', ISO_DATES, [45000, 45001, 45002, 45003, 45004, 45005]);
    let general = number_categories('A', "General", [1, 2, 3, 4, 5, 6]);
    let mixed = with_categories(&with_categories(&xml, &[1], &dates), &[2], &general);
    assert_refused(&mixed, source, target, "text, dates and General numbers");
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
