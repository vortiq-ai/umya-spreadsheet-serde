//! A series axis writes its crossing unless it is told to omit it, as Excel
//! saves the series axis of some 3-D charts; an axis saved without a crossing
//! reads and saves without one.
use std::io::Cursor;

use umya_spreadsheet::{
    Workbook,
    drawing::charts::{
        Crosses,
        PlotArea,
        SeriesAxis,
    },
    reader,
};

use super::{
    conversion::{
        chart,
        chart_part,
    },
    fixture::write_workbook,
    measured::{
        measured,
        read_part,
    },
    workbook::workbook_with,
};

const AUTO_ZERO: &str = r#"<c:crosses val="autoZero"/>"#;

/// A measured 3-D column chart; each of its axes crosses at `autoZero`.
fn column_3d() -> String {
    let part = measured()
        .parts
        .iter()
        .find(|part| part.fixture == "before/conversions2/bare/3dCol.chart.xml")
        .expect("the bare 3-D column part");
    read_part(part)
}

/// The part with the `autoZero` crossing of its first `axis` replaced.
fn with_crossing(xml: &str, axis: &str, crossing: &str) -> String {
    let start = xml.find(&format!("<{axis}>")).expect("the axis");
    let end = start + xml[start..].find(&format!("</{axis}>")).unwrap();
    let at = start
        + xml[start..end]
            .find(AUTO_ZERO)
            .expect("an autoZero crossing");
    format!("{}{crossing}{}", &xml[..at], &xml[at + AUTO_ZERO.len()..])
}

fn plot(book: &mut Workbook) -> &mut PlotArea {
    chart(book).plot_area_mut()
}

/// The series axis as written, without its end tag.
fn series_axis(book: &Workbook) -> String {
    let xml = chart_part(book);
    let start = xml.find("<c:serAx>").expect("a series axis");
    let end = start + xml[start..].find("</c:serAx>").unwrap();
    xml[start..end].to_owned()
}

/// The number of crossing elements the series axis writes: `c:crosses` and
/// `c:crossesAt` both start with `<c:crosses`.
fn crossings(axis: &str) -> usize {
    axis.matches("<c:crosses").count()
}

/// The workbook saved and read back.
fn reread(book: &Workbook) -> Workbook {
    reader::xlsx::read_reader(Cursor::new(write_workbook(book)), true).unwrap()
}

#[test]
fn a_new_series_axis_writes_its_crossing() {
    let mut book = workbook_with(&column_3d());
    let live = plot(&mut book).series_axis()[0].clone();
    let mut fresh = SeriesAxis::default();
    fresh
        .set_axis_id(live.axis_id().clone())
        .set_tick_crossing_axis(live.tick_crossing_axis().clone());
    plot(&mut book).series_axis_mut()[0] = fresh;
    let axis = series_axis(&book);
    assert!(axis.contains(AUTO_ZERO), "{axis}");
    assert_eq!(crossings(&axis), 1, "{axis}");
}

#[test]
fn an_omitted_crossing_stays_omitted_after_saving() {
    let mut book = workbook_with(&column_3d());
    assert!(series_axis(&book).contains(AUTO_ZERO));
    plot(&mut book).series_axis_mut()[0].remove_crosses();
    let omitted = series_axis(&book);
    assert_eq!(crossings(&omitted), 0, "{omitted}");
    assert_eq!(series_axis(&reread(&book)), omitted);
}

#[test]
fn omitting_the_crossing_drops_a_crossing_value() {
    let mut book = workbook_with(&column_3d());
    plot(&mut book).series_axis_mut()[0]
        .set_crosses_at(2.0)
        .remove_crosses();
    assert_eq!(plot(&mut book).series_axis()[0].crosses_at(), None);
    let omitted = series_axis(&book);
    assert_eq!(crossings(&omitted), 0, "{omitted}");
}

/// Each setter of the crossing writes one again after it was omitted.
#[test]
fn crossing_setters_bring_an_omitted_crossing_back() {
    let edits: [(&str, fn(&mut SeriesAxis), &str); 4] = [
        (
            "crosses_mut",
            |axis| {
                axis.crosses_mut();
            },
            AUTO_ZERO,
        ),
        (
            "set_crosses",
            |axis| {
                axis.set_crosses(Crosses::default());
            },
            AUTO_ZERO,
        ),
        (
            "set_crosses_at",
            |axis| {
                axis.set_crosses_at(2.0);
            },
            r#"<c:crossesAt val="2"/>"#,
        ),
        (
            "remove_crosses_at",
            |axis| {
                axis.remove_crosses_at();
            },
            AUTO_ZERO,
        ),
    ];
    for (setter, edit, written) in edits {
        let mut book = workbook_with(&column_3d());
        let axis = &mut plot(&mut book).series_axis_mut()[0];
        axis.remove_crosses();
        edit(axis);
        let axis = series_axis(&book);
        assert!(axis.contains(written), "{setter}: {axis}");
        assert_eq!(crossings(&axis), 1, "{setter}: {axis}");
    }
}

/// The crossing of a category axis saved without one says so, and still does
/// when it is given to the series axis.
#[test]
fn a_crossing_taken_from_an_axis_without_one_is_still_omitted() {
    let mut book = workbook_with(&with_crossing(&column_3d(), "c:catAx", ""));
    let crosses = plot(&mut book).category_axis()[0].crosses().clone();
    plot(&mut book).series_axis_mut()[0].set_crosses(crosses);
    let axis = series_axis(&book);
    assert_eq!(crossings(&axis), 0, "{axis}");
}

/// A saved series axis reads and saves with the crossing it was saved with:
/// none, a named one, or a value.
#[test]
fn a_saved_series_axis_keeps_the_crossing_it_was_saved_with() {
    for (saved, written) in [
        ("", None),
        (AUTO_ZERO, Some(AUTO_ZERO)),
        (
            r#"<c:crosses val="max"/>"#,
            Some(r#"<c:crosses val="max"/>"#),
        ),
        (
            r#"<c:crossesAt val="3"/>"#,
            Some(r#"<c:crossesAt val="3"/>"#),
        ),
    ] {
        let book = workbook_with(&with_crossing(&column_3d(), "c:serAx", saved));
        let again = reread(&book);
        for (when, book) in [("as read", book), ("saved again", again)] {
            let axis = series_axis(&book);
            match written {
                None => assert_eq!(crossings(&axis), 0, "none saved, {when}: {axis}"),
                Some(written) => {
                    assert!(axis.contains(written), "{saved} {when}: {axis}");
                    assert_eq!(crossings(&axis), 1, "{saved} {when}: {axis}");
                }
            }
        }
    }
}
