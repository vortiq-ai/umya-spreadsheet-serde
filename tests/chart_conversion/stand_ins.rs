//! The data roles a conversion leaves in place write as their source wrote
//! them, in each stored form a public route converts, next to the series
//! owners most likely to read them; a role the target family does not write
//! never reaches the result. Each case edits a measured part that converts.
use std::io::Cursor;

use umya_spreadsheet::{
    Workbook,
    drawing::charts::{
        AreaChartSeries,
        BubbleSize,
        SeriesPartKind,
        XValues,
        YValues,
    },
    helper::chart_conversion::{
        ChangeKind,
        ChartSubtype,
        ConversionChange,
        ConvertedPart,
    },
    reader,
};

use super::{
    conversion::{
        chart,
        chart_part,
        convert,
        replaced,
    },
    fixture::{
        replace_chart_part,
        write_workbook,
    },
    measured::{
        measured,
        read_part,
    },
    workbook::workbook_with,
};

/// The before part `name` of the bare round.
fn bare(name: &str) -> String {
    let fixture = format!("before/conversions2/bare/{name}.chart.xml");
    read_part(
        measured()
            .parts
            .iter()
            .find(|part| part.fixture == fixture)
            .unwrap_or_else(|| panic!("no part {fixture}")),
    )
}

fn first_series(book: &mut Workbook) -> &mut AreaChartSeries {
    &mut chart(book)
        .plot_area_mut()
        .chart_groups_mut()
        .next()
        .unwrap()
        .series_mut()
        .area_chart_series_mut()[0]
}

/// The workbook whose `Data` sheet holds the values `cells` refers to and
/// whose chart is `part`: a cache `part` edits is no longer what the cells
/// hold.
fn workbook_with_cells(cells: &str, part: &str) -> Workbook {
    let file = replace_chart_part(&write_workbook(&workbook_with(cells)), part);
    reader::xlsx::read_reader(Cursor::new(file), true).unwrap()
}

/// Where each `c:ser` of `part` starts and ends.
fn series_spans(part: &str) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    let mut from = 0;
    while let Some(start) = part[from..].find("<c:ser>") {
        let start = from + start;
        let end = start + part[start..].find("</c:ser>").expect("a closed series") + 8;
        spans.push((start, end));
        from = end;
    }
    spans
}

/// The child `role` of the series at `position` in `part`, as written.
fn role_at<'a>(part: &'a str, position: usize, role: &str) -> Option<&'a str> {
    let (start, end) = *series_spans(part).get(position)?;
    let series = &part[start..end];
    let close = format!("</c:{role}>");
    let from = series.find(&format!("<c:{role}>"))?;
    let to = from + series[from..].find(&close)? + close.len();
    Some(&series[from..to])
}

/// `part` with `insertion` put right before the child `role` of its first
/// series.
fn before_first_role(part: &str, role: &str, insertion: &str) -> String {
    let (start, _) = series_spans(part)[0];
    let at = start
        + part[start..]
            .find(&format!("<c:{role}>"))
            .expect("the role");
    format!("{}{insertion}{}", &part[..at], &part[at..])
}

/// `part` with the child `role` of its first series replaced by `child`.
fn with_first_role(part: &str, role: &str, child: &str) -> String {
    let (start, end) = series_spans(part)[0];
    let old = role_at(part, 0, role).expect("the role");
    let series = replaced(&part[start..end], old, child);
    format!("{}{series}{}", &part[..start], &part[end..])
}

const ROLES: [&str; 5] = ["cat", "val", "xVal", "yVal", "bubbleSize"];

const LABELS: &str = concat!(
    r#"<c:dLbls><c:showLegendKey val="0"/><c:showVal val="1"/><c:showCatName val="0"/>"#,
    r#"<c:showSerName val="0"/><c:showPercent val="0"/><c:showBubbleSize val="0"/></c:dLbls>"#
);
const TRENDLINE: &str = concat!(
    r#"<c:trendline><c:trendlineType val="linear"/><c:dispRSqr val="0"/>"#,
    r#"<c:dispEq val="0"/></c:trendline>"#
);
const ERROR_BARS: &str = concat!(
    r#"<c:errBars><c:errBarType val="both"/><c:errValType val="fixedVal"/>"#,
    r#"<c:noEndCap val="0"/><c:val val="1"/></c:errBars>"#
);

/// A stored form of a data role a public route converts. Multi-level and
/// literal categories are not here: every target waits for an Excel round
/// on them.
#[derive(Clone, Copy, Debug)]
enum Form {
    /// A string reference whose cache holds a label the cells do not.
    StoredLabels,
    /// A number reference with a format code and an extension of its own.
    FormattedNumbers,
    /// Numbers typed as an array constant.
    NumberLiteral,
    /// A pivot chart's numbers, which come from its cache, not the cells.
    PivotCache,
    /// A number cache that closes with an extension list of its own.
    NumberCacheExtensions,
    /// A string cache that closes with an extension list of its own.
    TextCacheExtensions,
}

/// An extension list whose content names `mark`, declaring its own
/// namespace.
fn cache_list(mark: &str) -> String {
    format!(
        concat!(
            r#"<c:extLst><c:ext uri="{{00000000-0000-0000-0000-0000000000C7}}" "#,
            r#"xmlns:vendor="urn:example:vendor"><vendor:note>{}</vendor:note></c:ext>"#,
            "</c:extLst>"
        ),
        mark
    )
}

/// The first series' role of `part` that holds text categories.
fn text_categories(part: &str) -> &'static str {
    ["cat", "xVal"]
        .into_iter()
        .find(|role| role_at(part, 0, role).is_some_and(|text| text.contains("<c:strRef>")))
        .expect("text categories")
}

impl Form {
    /// The first series' role the form is given, and the source `part` with
    /// it: the chart part, the part whose caches the cells hold, and text
    /// only the form writes.
    fn apply(self, part: &str) -> (String, String, &'static str) {
        let numbers = ["val", "yVal"]
            .into_iter()
            .find(|role| role_at(part, 0, role).is_some())
            .expect("numbers");
        let written = role_at(part, 0, numbers).unwrap();
        match self {
            Self::StoredLabels => {
                let role = text_categories(part);
                let labels = role_at(part, 0, role).unwrap();
                let first = r#"<c:pt idx="0"><c:v>"#;
                let edited = replaced(labels, first, &format!("{first}stored "));
                (
                    with_first_role(part, role, &edited),
                    part.to_owned(),
                    "stored ",
                )
            }
            Self::FormattedNumbers => {
                let formula =
                    &written[written.find("<c:f>").unwrap() + 5..written.find("</c:f>").unwrap()];
                let extension = format!(
                    concat!(
                        r#"<c:extLst><c:ext uri="{{02D57815-91ED-43cb-92C2-25804820EDAC}}" "#,
                        r#"xmlns:c15="http://schemas.microsoft.com/office/drawing/2012/chart">"#,
                        r#"<c15:fullRef><c15:sqref>{}</c15:sqref></c15:fullRef></c:ext></c:extLst>"#
                    ),
                    formula
                );
                let formatted = replaced(written, "General", "0.00");
                let formatted = replaced(
                    &formatted,
                    "</c:numRef>",
                    &format!("{extension}</c:numRef>"),
                );
                let part = with_first_role(part, numbers, &formatted);
                (part.clone(), part, "<c15:fullRef>")
            }
            Self::NumberLiteral => {
                let points: String = (0..6)
                    .map(|at| format!(r#"<c:pt idx="{at}"><c:v>{}.25</c:v></c:pt>"#, at + 1))
                    .collect();
                let literal = format!(
                    r#"<c:{numbers}><c:numLit><c:formatCode>0.00</c:formatCode><c:ptCount val="6"/>{points}</c:numLit></c:{numbers}>"#
                );
                let part = with_first_role(part, numbers, &literal);
                (part.clone(), part, "<c:numLit>")
            }
            Self::PivotCache => {
                let pivot = replaced(
                    part,
                    "<c:chart>",
                    concat!(
                        r#"<c:pivotSource><c:name>[book.xlsx]Data!PivotTable1</c:name>"#,
                        r#"<c:fmtId val="0"/></c:pivotSource><c:chart>"#
                    ),
                );
                let end = written.find("</c:v>").expect("a value");
                let cached = format!("{}.75{}", &written[..end], &written[end..]);
                (
                    with_first_role(&pivot, numbers, &cached),
                    part.to_owned(),
                    ".75</c:v>",
                )
            }
            Self::NumberCacheExtensions => {
                let listed = replaced(
                    written,
                    "</c:numCache>",
                    &format!("{}</c:numCache>", cache_list("number cache list")),
                );
                let part = with_first_role(part, numbers, &listed);
                (part.clone(), part, "number cache list")
            }
            Self::TextCacheExtensions => {
                let role = text_categories(part);
                let labels = role_at(part, 0, role).unwrap();
                let listed = replaced(
                    labels,
                    "</c:strCache>",
                    &format!("{}</c:strCache>", cache_list("text cache list")),
                );
                let part = with_first_role(part, role, &listed);
                (part.clone(), part, "text cache list")
            }
        }
    }
}

/// A role whose content the plan leaves as the source wrote it is written as
/// the source wrote it, whatever its stored form and whatever owners sit
/// next to it: each measured source, its first series given the form, data
/// labels, a trendline where its family has one and error bars where the
/// target removes them, converts to targets of other families that keep
/// its roles in place, and every role both write is byte-equal to the
/// source's own.
#[test]
fn unchanged_roles_write_as_their_source_wrote_them() {
    use ChartSubtype::{
        Area,
        Bubble,
        ClusteredBar,
        ClusteredColumn,
        Doughnut,
        Line,
        Pie,
        Radar,
        ScatterLines,
    };
    let pairs: [(&str, &[ChartSubtype]); 8] = [
        ("colClustered", &[Line, Area, Radar, Pie]),
        ("line", &[ClusteredColumn, Area, Radar, Doughnut]),
        ("area", &[ClusteredBar, Line, Radar]),
        ("radar", &[ClusteredColumn, Line, Pie]),
        ("pie", &[ClusteredColumn, Line, Doughnut]),
        ("doughnut", &[ClusteredColumn, Line, Pie]),
        ("scatterLines", &[Bubble]),
        ("bubble", &[ScatterLines]),
    ];
    for (source, targets) in pairs {
        let plain = bare(source);
        let next_to = ["cat", "xVal"]
            .into_iter()
            .find(|role| role_at(&plain, 0, role).is_some())
            .expect("categories or x values");
        let with_trendline = !matches!(source, "radar" | "pie" | "doughnut");
        for form in [
            Form::StoredLabels,
            Form::FormattedNumbers,
            Form::NumberLiteral,
            Form::PivotCache,
            Form::NumberCacheExtensions,
            Form::TextCacheExtensions,
        ] {
            if matches!(form, Form::StoredLabels | Form::TextCacheExtensions)
                && source == "scatterLines"
            {
                // Its x values are numbers: the bubble source holds text ones.
                continue;
            }
            for &target in targets {
                // Error bars wait for their direction into every family
                // but the ones that remove them.
                let with_error_bars = with_trendline && matches!(target, Radar | Pie | Doughnut);
                let owners = format!(
                    "{LABELS}{}{}",
                    if with_trendline { TRENDLINE } else { "" },
                    if with_error_bars { ERROR_BARS } else { "" }
                );
                let dressed = before_first_role(&plain, next_to, &owners);
                let (part, cells, marker) = form.apply(&dressed);
                let mut book = workbook_with_cells(&cells, &part);
                let before = chart_part(&book);
                let case = format!("{source} {form:?} -> {target:?}");
                assert!(
                    series_spans(&before)
                        .first()
                        .is_some_and(|&(start, end)| before[start..end].contains(marker)),
                    "{case}: the source writes the form"
                );
                assert!(
                    before.contains("<c:dLbls>")
                        && before.contains("<c:trendline>") == with_trendline,
                    "{case}: the source writes the owners"
                );
                convert(&mut book, target).unwrap_or_else(|refusal| panic!("{case}: {refusal:?}"));
                let after = chart_part(&book);
                let mut compared = 0;
                for position in 0..series_spans(&before).len() {
                    for role in ROLES {
                        if let (Some(was), Some(now)) = (
                            role_at(&before, position, role),
                            role_at(&after, position, role),
                        ) {
                            assert_eq!(now, was, "{case}: series {position} {role}");
                            compared += 1;
                        }
                    }
                }
                assert!(
                    compared >= 2 * series_spans(&before).len(),
                    "{case}: {compared} roles"
                );
            }
        }
    }
}

/// The first series' child of `kind`, as a test gives it to a series whose
/// family does not write it: built from the series' own values or
/// categories.
#[derive(Clone, Copy, Debug)]
enum Hidden {
    NumericX,
    TextX,
    Y,
    Sizes,
}

impl Hidden {
    fn kind(self) -> SeriesPartKind {
        match self {
            Self::NumericX | Self::TextX => SeriesPartKind::XValues,
            Self::Y => SeriesPartKind::YValues,
            Self::Sizes => SeriesPartKind::BubbleSize,
        }
    }

    fn give(self, series: &mut AreaChartSeries) {
        let values = series.values().expect("values").number_reference().clone();
        match self {
            Self::NumericX => {
                let mut role = XValues::default();
                role.set_number_reference(values);
                series.set_x_values(role);
            }
            Self::TextX => {
                let mut role = XValues::default();
                role.set_text_source(series.category_axis_data().expect("categories").clone());
                series.set_x_values(role);
            }
            Self::Y => {
                let mut role = YValues::default();
                role.set_number_reference(values);
                series.set_y_values(role);
            }
            Self::Sizes => {
                let mut role = BubbleSize::default();
                role.set_number_reference(values);
                series.set_bubble_size(role);
            }
        }
    }
}

/// The series parts in the order a report sorts them, after the chart-level
/// parts and before the series' paint.
const PART_ORDER: [SeriesPartKind; 17] = [
    SeriesPartKind::Explosion,
    SeriesPartKind::InvertIfNegative,
    SeriesPartKind::Marker,
    SeriesPartKind::Categories,
    SeriesPartKind::Values,
    SeriesPartKind::XValues,
    SeriesPartKind::YValues,
    SeriesPartKind::BubbleSize,
    SeriesPartKind::Bubble3D,
    SeriesPartKind::Smooth,
    SeriesPartKind::Shape,
    SeriesPartKind::PictureOptions,
    SeriesPartKind::DataPoints,
    SeriesPartKind::Trendlines,
    SeriesPartKind::DataLabels,
    SeriesPartKind::Extensions,
    SeriesPartKind::ErrorBars,
];

/// `changes` with the first series' `kind` reported removed, at its place:
/// after the series' parts that sort before it.
fn with_removal(changes: &[ConversionChange], kind: SeriesPartKind) -> Vec<ConversionChange> {
    let rank = |kind: SeriesPartKind| PART_ORDER.iter().position(|other| *other == kind);
    let before = |change: &ConversionChange| match change.series_index {
        None => true,
        Some(0) => match change.part {
            ConvertedPart::SeriesPart(other) => rank(other) < rank(kind),
            _ => false,
        },
        Some(_) => false,
    };
    let at = changes.iter().take_while(|change| before(change)).count();
    let mut expected = changes.to_vec();
    expected.insert(
        at,
        ConversionChange {
            series_index: Some(0),
            part:         ConvertedPart::SeriesPart(kind),
            change:       ChangeKind::Removed,
        },
    );
    expected
}

/// A role the line series stores and its writer leaves out never reaches
/// the result: a target whose move writes the role writes the moved
/// content, and a target whose family has no such role removes it. Either
/// way the result writes what the same conversion of the part without the
/// role writes; a removal is reported once more, at its rank.
#[test]
fn a_hidden_role_never_reaches_the_result() {
    let line = bare("line");
    let written = chart_part(&workbook_with(&line));
    for hidden in [Hidden::NumericX, Hidden::TextX, Hidden::Y, Hidden::Sizes] {
        for target in [
            ChartSubtype::Scatter,
            ChartSubtype::ScatterLines,
            ChartSubtype::Bubble,
            ChartSubtype::Pie,
        ] {
            let mut plain = workbook_with(&line);
            let plain_changes = convert(&mut plain, target).unwrap().changes().to_vec();
            let mut book = workbook_with(&line);
            hidden.give(first_series(&mut book));
            assert_eq!(
                chart_part(&book),
                written,
                "{hidden:?}: a line series leaves it out"
            );
            let changes = convert(&mut book, target)
                .unwrap_or_else(|refusal| panic!("{hidden:?} -> {target:?}: {refusal:?}"))
                .changes()
                .to_vec();
            assert!(
                chart_part(&book) == chart_part(&plain),
                "{hidden:?} -> {target:?}: the result is not the plain part's"
            );
            // A scatter or bubble series writes x and y values, and a bubble
            // series its sizes, from the line series' categories and values.
            let moved_over = match target {
                ChartSubtype::Pie => false,
                ChartSubtype::Bubble => true,
                _ => hidden.kind() != SeriesPartKind::BubbleSize,
            };
            let expected = if moved_over {
                plain_changes
            } else {
                with_removal(&plain_changes, hidden.kind())
            };
            assert_eq!(changes, expected, "{hidden:?} -> {target:?}");
        }
    }
}
