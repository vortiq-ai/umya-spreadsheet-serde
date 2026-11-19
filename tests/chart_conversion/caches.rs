//! A source cache may close with its own extension list, as Excel writes one
//! in a PivotChart's number cache. Excel keeps such a list on every cache
//! kind, in a PivotChart or not: through a save, an edit of the cells the
//! cache shows and a change of chart type, where it moves with its series.
//! The chart reads with it and writes it back the same way.
use std::io::Cursor;

use umya_spreadsheet::{
    Workbook,
    helper::chart_conversion::ChartSubtype,
    reader,
};

use super::{
    conversion::{
        chart_part,
        convert,
        inserted_after,
        replaced,
        replaced_between,
    },
    fixture::{
        replace_chart_part,
        write_workbook,
    },
    measured::part_xml,
    workbook::workbook_with,
    xml::{
        CHART,
        Element,
        parse,
    },
};

/// Three clustered-column series of six points; series 1 reads its name
/// from `Data!$B$1`, its categories from `Data!$A$2:$A$7` and its values from
/// `Data!$B$2:$B$7`.
const COLUMNS: &str = "before/conversions2/bare/colClustered.chart.xml";

/// An extension that declares the namespace of its content itself.
const OWN_BINDING: &str = concat!(
    r#"<c:ext uri="{00000000-0000-0000-0000-0000000000C1}" xmlns:vendor="urn:example:vendor">"#,
    r#"<vendor:note vendor:kind="cache">kept</vendor:note></c:ext>"#
);
/// An extension whose content uses a prefix the chart space declares.
const INHERITED_BINDING: &str =
    r#"<c:ext uri="{00000000-0000-0000-0000-0000000000C2}"><outer:mark val="1"/></c:ext>"#;
const OUTER: &str = r#" xmlns:outer="urn:example:outer""#;

fn extension_list() -> String {
    format!("<c:extLst>{OWN_BINDING}{INHERITED_BINDING}</c:extLst>")
}

/// An extension list whose content names `mark`.
fn marked_list(mark: &str) -> String {
    format!(
        concat!(
            r#"<c:extLst><c:ext uri="{{00000000-0000-0000-0000-0000000000C6}}" "#,
            r#"xmlns:vendor="urn:example:vendor"><vendor:note>{}</vendor:note></c:ext>"#,
            "</c:extLst>"
        ),
        mark
    )
}

/// `xml` as a PivotChart: the chart space names the pivot table it shows.
pub fn pivot(xml: &str) -> String {
    replaced(
        xml,
        "<c:chart>",
        concat!(
            "<c:pivotSource><c:name>[book.xlsx]Data!PivotTable1</c:name>",
            r#"<c:fmtId val="0"/></c:pivotSource><c:chart>"#
        ),
    )
}

/// `xml` with the chart space declaring the `outer` prefix.
fn with_outer_prefix(xml: &str) -> String {
    inserted_after(xml, "<c:chartSpace", OUTER)
}

/// `xml` with `list` as the last child of the first cache that ends with the
/// tag `close`.
fn closing_with(xml: &str, close: &str, list: &str) -> String {
    let end = xml
        .find(close)
        .unwrap_or_else(|| panic!("{close} is not in the part"));
    format!("{}{list}{}", &xml[..end], &xml[end..])
}

/// `xml` with `list(at)` as the last child of the `at`th cache that ends with
/// the tag `close`, for every such cache.
fn closing_each(xml: &str, close: &str, list: impl Fn(usize) -> String) -> String {
    let mut pieces = xml.split(close);
    let mut out = pieces.next().unwrap_or_default().to_owned();
    for (at, piece) in pieces.enumerate() {
        out.push_str(&list(at));
        out.push_str(close);
        out.push_str(piece);
    }
    out
}

/// `COLUMNS`, its chart space declaring the `outer` prefix, with each string
/// and number cache closing with a list that names the cache: `text <at>` for
/// the `at`th string cache, `number <at>` for the `at`th number cache.
fn columns_with_marked_lists() -> String {
    let xml = with_outer_prefix(&part_xml(COLUMNS));
    let xml = closing_each(&xml, "</c:strCache>", |at| {
        marked_list(&format!("text {at}"))
    });
    closing_each(&xml, "</c:numCache>", |at| {
        marked_list(&format!("number {at}"))
    })
}

/// Every string and number cache of `written` closes with the list of the
/// same cache of `input`.
fn assert_lists_kept(written: &str, input: &str, what: &str) {
    for (cache, count) in [("strCache", 6), ("numCache", 3)] {
        assert_eq!(
            parse(written).all_named(CHART, cache).len(),
            count,
            "{what}"
        );
        for at in 0..count {
            let expected = cache_extensions(input, cache, at);
            assert_ne!(expected, "None");
            assert_eq!(
                cache_extensions(written, cache, at),
                expected,
                "{what}: {cache} {at}"
            );
        }
    }
}

/// The `extLst` of the `at`th `c:<cache>` of the chart part, with its
/// namespaces resolved, attributes keyed by namespace and text.
fn cache_extensions(xml: &str, cache: &str, at: usize) -> String {
    let space = parse(xml);
    let caches = space.all_named(CHART, cache);
    format!("{:?}", caches[at].c("extLst"))
}

/// The chart part with every name resolved to its namespace. Where a
/// namespace is declared, and in which order, is not part of it: a re-read
/// list declares inherited prefixes in another order.
fn resolved(xml: &str) -> String {
    format!("{:?}", parse(xml))
}

/// The workbook written by `book` read again, lazily, its sheet then read.
fn reread_lazily(book: &Workbook) -> Workbook {
    let mut book = reader::xlsx::read_reader(Cursor::new(write_workbook(book)), false).unwrap();
    book.try_read_sheet(0).unwrap();
    book
}

/// A PivotChart's number cache is the cache it shows, stored with the chart,
/// so the extension list it closes with is written with it: the same
/// elements, attributes and namespaces, after a read, a write and a lazy
/// re-read.
#[test]
fn a_pivot_number_cache_keeps_its_extension_list() {
    let input = with_outer_prefix(&pivot(&part_xml(COLUMNS)));
    let input = closing_with(&input, "</c:numCache>", &extension_list());
    let expected = cache_extensions(&input, "numCache", 0);
    assert!(expected.contains(r#""urn:example:outer""#), "{expected}");
    let book = workbook_with(&input);
    let written = chart_part(&book);
    assert_eq!(cache_extensions(&written, "numCache", 0), expected);
    assert_eq!(cache_extensions(&written, "numCache", 1), "None");
    assert_eq!(
        resolved(&chart_part(&reread_lazily(&book))),
        resolved(&written)
    );
}

/// `xml` without the extension list that closes any string, multi-level
/// string or number cache.
fn without_cache_extensions(xml: &str) -> String {
    let mut xml = xml.to_owned();
    for end in ["</c:strCache>", "</c:multiLvlStrCache>", "</c:numCache>"] {
        let mut pieces: Vec<&str> = xml.split(end).collect();
        let last = pieces.len() - 1;
        for piece in &mut pieces[..last] {
            if piece.ends_with("</c:extLst>") {
                *piece = &piece[..piece.rfind("<c:extLst").unwrap()];
            }
        }
        xml = pieces.join(end);
    }
    xml
}

/// Every cache closes with the extension list it was read with, in a
/// PivotChart or not: string, multi-level string and number caches write the
/// same elements, attributes and namespaces, and apart from those lists the
/// chart writes what the same chart without them writes, read eagerly or
/// lazily.
#[test]
fn every_cache_keeps_its_extension_list() {
    let multi_level = concat!(
        r#"<c:cat><c:multiLvlStrRef><c:f>Data!$A$2:$B$7</c:f><c:multiLvlStrCache>"#,
        r#"<c:ptCount val="6"/><c:lvl><c:pt idx="0"><c:v>C1</c:v></c:pt>"#,
        r#"<c:pt idx="1"><c:v>C2</c:v></c:pt><c:pt idx="2"><c:v>C3</c:v></c:pt>"#,
        r#"<c:pt idx="3"><c:v>C4</c:v></c:pt><c:pt idx="4"><c:v>C5</c:v></c:pt>"#,
        r#"<c:pt idx="5"><c:v>C6</c:v></c:pt></c:lvl><c:lvl><c:pt idx="0"><c:v>G1</c:v></c:pt>"#,
        r#"<c:pt idx="3"><c:v>G2</c:v></c:pt></c:lvl></c:multiLvlStrCache></c:multiLvlStrRef></c:cat>"#
    );
    for pivot_chart in [false, true] {
        let mut base = with_outer_prefix(&part_xml(COLUMNS));
        if pivot_chart {
            base = pivot(&base);
        }
        let base = replaced_between(&base, "<c:cat>", "</c:cat>", multi_level);
        let mut input = base.clone();
        for close in ["</c:strCache>", "</c:multiLvlStrCache>", "</c:numCache>"] {
            input = closing_each(&input, close, |_| extension_list());
        }
        let book = workbook_with(&input);
        let written = chart_part(&book);
        for (cache, count) in [("strCache", 5), ("multiLvlStrCache", 1), ("numCache", 3)] {
            assert_eq!(parse(&written).all_named(CHART, cache).len(), count);
            for at in 0..count {
                let expected = cache_extensions(&input, cache, at);
                assert!(expected.contains(r#""urn:example:outer""#), "{expected}");
                assert_eq!(
                    cache_extensions(&written, cache, at),
                    expected,
                    "pivot: {pivot_chart}, {cache} {at}"
                );
            }
        }
        assert_eq!(
            without_cache_extensions(&written),
            chart_part(&workbook_with(&base)),
            "pivot: {pivot_chart}"
        );
        assert_eq!(
            resolved(&chart_part(&reread_lazily(&book))),
            resolved(&written)
        );
    }
}

/// A cache shows its cells: after two of them change, the categories and
/// values are written with the new text and number, and every cache still
/// closes with its own list.
#[test]
fn a_cache_keeps_its_extension_list_when_its_cells_change() {
    let input = columns_with_marked_lists();
    let mut book = workbook_with(&input);
    let sheet = book.sheet_mut(0).unwrap();
    sheet.cell_mut((1, 3)).set_value_string("Changed");
    sheet.cell_mut((2, 3)).set_value_number(42.0);
    let written = chart_part(&book);
    let space = parse(&written);
    let second = |cache: &Element| {
        let point = cache.all_c("pt").nth(1).unwrap();
        (
            point.attribute("idx").unwrap().to_owned(),
            point.c("v").unwrap().text.clone(),
        )
    };
    let categories = space.all_named(CHART, "strCache")[1];
    assert_eq!(second(categories), ("1".to_owned(), "Changed".to_owned()));
    let values = space.all_named(CHART, "numCache")[0];
    assert_eq!(second(values), ("1".to_owned(), "42".to_owned()));
    assert_lists_kept(&written, &input, "after the edit");
}

/// A change of chart type keeps each series with its caches, so every cache
/// closes with its own list in the series it came from.
#[test]
fn a_cache_keeps_its_extension_list_through_a_conversion() {
    let input = columns_with_marked_lists();
    let mut book = workbook_with(&input);
    convert(&mut book, ChartSubtype::Line).unwrap();
    let written = chart_part(&book);
    assert!(parse(&written).descendant_c("lineChart").is_some());
    assert_lists_kept(&written, &input, "after the conversion");
}

/// A number cache's format code is its own `c:formatCode`: a `formatCode` of
/// another vendor inside the cache's extension list is not read as the code,
/// so the cache is written with its own code and that list.
#[test]
fn a_number_cache_takes_only_its_own_format_code() {
    let base = part_xml(COLUMNS);
    let vendor_code = concat!(
        r#"<c:extLst><c:ext uri="{00000000-0000-0000-0000-0000000000C4}" "#,
        r#"xmlns:vendor="urn:example:vendor"><vendor:formatCode>0.00%</vendor:formatCode>"#,
        r#"</c:ext></c:extLst>"#
    );
    let input = closing_with(&base, "</c:numCache>", vendor_code);
    let written = chart_part(&workbook_with(&input));
    let codes: Vec<_> = parse(&written)
        .all_named(CHART, "numCache")
        .iter()
        .map(|cache| cache.c("formatCode").map(|code| code.text.clone()))
        .collect();
    assert_eq!(codes, vec![Some("General".to_owned()); 3]);
    assert_eq!(
        cache_extensions(&written, "numCache", 0),
        cache_extensions(&input, "numCache", 0)
    );
    assert_eq!(
        without_cache_extensions(&written),
        chart_part(&workbook_with(&base))
    );
}

/// The workbook `file` read eagerly, and read lazily with its sheet then read.
fn read_both_ways(file: &[u8]) -> [Workbook; 2] {
    let eager = reader::xlsx::read_reader(Cursor::new(file), true).unwrap();
    let mut lazy = reader::xlsx::read_reader(Cursor::new(file), false).unwrap();
    lazy.try_read_sheet(0).unwrap();
    [eager, lazy]
}

/// A string cache's points are its own `c:ptCount` and `c:pt` children:
/// another vendor's count and point inside the cache's extension list are not
/// read as its data, on the first read and when the cache is read again for
/// writing, eagerly or lazily; the list is written back with the cache.
#[test]
fn a_string_cache_reads_only_its_own_points() {
    let base = part_xml(COLUMNS);
    let vendor_points = concat!(
        r#"<c:extLst><c:ext uri="{00000000-0000-0000-0000-0000000000C5}" "#,
        r#"xmlns:vendor="urn:example:vendor"><vendor:ptCount val="9"/>"#,
        r#"<vendor:pt idx="7"><c:v>X</c:v></vendor:pt></c:ext></c:extLst>"#
    );
    let input = closing_with(&base, "</c:strCache></c:strRef></c:cat>", vendor_points);
    let expected = chart_part(&workbook_with(&base));
    let file = replace_chart_part(&write_workbook(&workbook_with(&base)), &input);
    for book in read_both_ways(&file) {
        let written = chart_part(&book);
        let space = parse(&written);
        let categories = space
            .descendant_c("cat")
            .unwrap()
            .descendant_c("strCache")
            .unwrap();
        assert_eq!(categories.c("ptCount").unwrap().attribute("val"), Some("6"));
        let points: Vec<_> = categories
            .all_c("pt")
            .map(|point| {
                (
                    point.attribute("idx").unwrap().to_owned(),
                    point.c("v").unwrap().text.clone(),
                )
            })
            .collect();
        let own: Vec<_> = (0..6)
            .map(|at| (at.to_string(), format!("C{}", at + 1)))
            .collect();
        assert_eq!(points, own);
        assert_eq!(
            cache_extensions(&written, "strCache", 1),
            cache_extensions(&input, "strCache", 1)
        );
        assert_eq!(without_cache_extensions(&written), expected);
    }
}

/// A point's value is its `c:v`: another element after it in the point is not
/// read as the value, short or long, so the cache is written as the same
/// chart without that element writes it, eagerly or lazily.
#[test]
fn a_point_keeps_its_value_before_another_child() {
    let base = with_outer_prefix(&part_xml(COLUMNS));
    let expected = chart_part(&workbook_with(&base));
    let point = r#"<c:pt idx="1"><c:v>C2</c:v></c:pt>"#;
    for child in ["<y>z</y>", "<outer:q>z</outer:q>"] {
        // The categories of the first series.
        let input = base.replacen(
            point,
            &format!(r#"<c:pt idx="1"><c:v>C2</c:v>{child}</c:pt>"#),
            1,
        );
        assert_ne!(input, base);
        let file = replace_chart_part(&write_workbook(&workbook_with(&base)), &input);
        for book in read_both_ways(&file) {
            assert_eq!(chart_part(&book), expected, "{child}");
        }
    }
}
