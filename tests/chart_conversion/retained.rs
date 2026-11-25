//! What a conversion keeps of the chart umya holds: data roles move whole as
//! their writer writes them, groups and axes the result retains keep the
//! state their writer leaves out or the conversion waits, and what the target
//! family's cleanup removes is reported. Each case edits a measured part
//! that converts.
use umya_spreadsheet::{
    Workbook,
    drawing::charts::{
        AreaChartSeries,
        CategoryAxis,
        ExtensionList,
        ExtensionNode,
        MajorGridlines,
        NumericData,
        QName,
        SeriesPartKind,
        ShapeProperties,
        ValueAxis,
    },
    helper::chart_conversion::{
        ChangeKind,
        ChartConversionRefusal,
        ChartSource,
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
        replaced_between,
    },
    measured::{
        measured,
        read_part,
    },
    workbook::workbook_with,
};

const COLUMN: &str = "before/conversions2/bare/colClustered.chart.xml";
const LINE: &str = "before/conversions2/bare/line.chart.xml";
const DATE_LINE: &str = "before/conversions3/date-categories/line.chart.xml";
const BUBBLE: &str = "before/conversions2/bare/bubble.chart.xml";
const SCATTER_LINES: &str = "before/conversions2/bare/scatterLines.chart.xml";

/// An extension umya does not know, with mixed content.
const OPAQUE: &str = concat!(
    r#"<c:extLst><c:ext uri="{00000000-0000-0000-0000-00000000AB12}" "#,
    r#"xmlns:t="urn:example:test"><t:mark>A<t:b/>B</t:mark></c:ext></c:extLst>"#
);

/// The column part's first series' own extension.
const FIRST_UNIQUE_ID: &str = concat!(
    r#"<c:ext uri="{C3380CC4-5D6E-409C-BE32-E72D297353CC}" "#,
    r#"xmlns:c16="http://schemas.microsoft.com/office/drawing/2014/chart">"#,
    r#"<c16:uniqueId val="{00000000-469A-4366-BBBA-E5B5ADF15095}"/></c:ext>"#
);

/// An extension bar and bubble series hold and every other family drops.
const INVERT_FILL: &str = concat!(
    r#"<c:ext uri="{6F2FDCE9-48DA-4B69-8628-5D25D57E5C99}" "#,
    r#"xmlns:c14="http://schemas.microsoft.com/office/drawing/2007/8/2/chart">"#,
    r#"<c14:invertSolidFillFmt><c14:spPr><a:solidFill><a:srgbClr val="ED7D31"/>"#,
    r#"</a:solidFill></c14:spPr></c14:invertSolidFillFmt></c:ext>"#
);

/// The before part at `fixture`.
fn part_at(fixture: &str) -> String {
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

/// The first series' content in the chart part `part`.
fn first_series_xml(part: &str) -> &str {
    let series = &part[part.find("<c:ser>").expect("a series")..];
    &series[..series.find("</c:ser>").expect("a closed series")]
}

/// The first series' `c:extLst` in the chart part `part`.
fn first_extensions(part: &str) -> &str {
    let series = first_series_xml(part);
    &series[series.find("<c:extLst").expect("an extension list")..]
}

/// The changes the conversion reports for the series at `position`.
fn series_changes(
    changes: &[umya_spreadsheet::helper::chart_conversion::ConversionChange],
    position: usize,
) -> Vec<(ConvertedPart, ChangeKind)> {
    changes
        .iter()
        .filter(|change| change.series_index == Some(position))
        .map(|change| (change.part, change.change))
        .collect()
}

/// The list as stored, and what umya reports when it leaves entries out for
/// `owner`.
fn stored(book: &Workbook, list: Option<&ExtensionList>, owner: &str) -> (String, Vec<String>) {
    (
        format!("{list:?}"),
        list.map(|list| {
            list.export_diagnostics_for_owner(Some(book), owner, None)
                .into_iter()
                .map(|diagnostic| format!("{diagnostic:?}"))
                .collect()
        })
        .unwrap_or_default(),
    )
}

fn refused_unchanged(book: &mut Workbook, from: ChartSubtype, to: ChartSubtype) {
    let before = format!("{:?}", chart(book));
    let refusal = convert(book, to).err();
    assert_eq!(
        refusal,
        Some(ChartConversionRefusal::Unsupported {
            from: Some(ChartSource::Subtype(from)),
            to,
        })
    );
    assert_eq!(format!("{:?}", chart(book)), before);
}

/// A literal that becomes y values keeps its format code, point count,
/// indices and values, after a write and a read too.
#[test]
fn literal_values_become_y_values_whole() {
    let literal = concat!(
        r#"<c:val><c:numLit><c:formatCode>0.0</c:formatCode><c:ptCount val="6"/>"#,
        r#"<c:pt idx="0"><c:v>1.5</c:v></c:pt><c:pt idx="1"><c:v>2.5</c:v></c:pt>"#,
        r#"<c:pt idx="2"><c:v>3.5</c:v></c:pt><c:pt idx="3"><c:v>4.5</c:v></c:pt>"#,
        r#"<c:pt idx="4"><c:v>5.5</c:v></c:pt><c:pt idx="5"><c:v>6.5</c:v></c:pt>"#,
        "</c:numLit></c:val>"
    );
    let xml = replaced_between(&part_at(LINE), "<c:val>", "</c:val>", literal);
    let mut book = workbook_with(&xml);
    convert(&mut book, ChartSubtype::ScatterLines).unwrap();
    let read = |book: &mut Workbook| {
        let literal = first_series(book)
            .y_values()
            .and_then(|values| values.number_literal())
            .expect("the y values are a literal")
            .clone();
        (
            literal.format_code().map(str::to_owned),
            literal.point_count().and_then(|count| count.val()),
            literal
                .points()
                .iter()
                .map(|point| (point.index().val(), point.value().map(str::to_owned)))
                .collect::<Vec<_>>(),
        )
    };
    let expected = (
        Some("0.0".to_owned()),
        Some(6),
        ["1.5", "2.5", "3.5", "4.5", "5.5", "6.5"]
            .into_iter()
            .zip(0..)
            .map(|(value, index)| (Some(index), Some(value.to_owned())))
            .collect::<Vec<_>>(),
    );
    assert_eq!(read(&mut book), expected);
    assert_eq!(read(&mut workbook_with(&chart_part(&book))), expected);
}

/// Date categories become x values as the number reference they are, with
/// no text source, after a write and a read too.
#[test]
fn date_categories_become_a_numeric_x_reference() {
    let mut book = workbook_with(&part_at(DATE_LINE));
    convert(&mut book, ChartSubtype::ScatterLines).unwrap();
    let read = |book: &mut Workbook| {
        let x = first_series(book).x_values().expect("x values");
        (
            x.text_source().is_none(),
            x.number_literal().is_none(),
            x.number_reference().formula().address_str(),
        )
    };
    let expected = (true, true, "Data!$A$2:$A$7".to_owned());
    assert_eq!(read(&mut book), expected);
    assert_eq!(read(&mut workbook_with(&chart_part(&book))), expected);
}

/// x values that hold a literal under a text source are written as the text,
/// and become categories holding the text alone.
#[test]
fn x_values_written_as_text_become_text_categories() {
    let mut book = workbook_with(&part_at(BUBBLE));
    let x = first_series(&mut book).x_values_mut().expect("x values");
    let text = x.text_source().expect("the x values are text").clone();
    // A literal replaces the text source; the text source set after it keeps
    // the literal.
    x.set_number_literal(NumericData::default());
    x.set_text_source(text);
    assert!(x.number_literal().is_some() && x.text_source().is_some());
    convert(&mut book, ChartSubtype::Line).unwrap();
    let categories = first_series(&mut book)
        .category_axis_data()
        .expect("categories");
    assert_eq!(
        (
            categories
                .string_reference()
                .map(|reference| reference.formula().address_str()),
            categories.number_literal().is_none(),
            categories.number_reference().is_none(),
        ),
        (Some("Data!$J$1:$J$7".to_owned()), true, true)
    );
}

/// Categories holding a number reference and a literal are written as
/// neither and read as no source: the conversion refuses, the chart
/// unchanged.
#[test]
fn categories_holding_two_numeric_sources_are_refused() {
    let mut book = workbook_with(&part_at(DATE_LINE));
    first_series(&mut book)
        .category_axis_data_mut()
        .expect("categories")
        .set_number_literal(NumericData::default());
    refused_unchanged(&mut book, ChartSubtype::Line, ChartSubtype::ScatterLines);
}

/// A column series whose stored extensions and error bars a pie series
/// cannot hold loses exactly those, written or not; what survives is written
/// as before, and each removed part is reported once, at its rank.
#[test]
fn a_family_cleanup_keeps_what_the_target_writes_and_reports_the_rest() {
    let column = part_at(COLUMN);
    // Both lists lead with what goes, so the survivors move up.
    let xml = replaced(
        &column,
        FIRST_UNIQUE_ID,
        &format!("{INVERT_FILL}{FIRST_UNIQUE_ID}"),
    );
    // The first error bar has no type and is never written; a column series
    // writes only its first, so the second is stored but not written either.
    let error_bars = concat!(
        r#"<c:errBars><c:errValType val="fixedVal"/><c:noEndCap val="0"/>"#,
        r#"<c:val val="1"/></c:errBars>"#,
        r#"<c:errBars><c:errBarType val="both"/><c:errValType val="fixedVal"/>"#,
        r#"<c:noEndCap val="0"/><c:val val="2"/></c:errBars>"#
    );
    let first =
        r#"<c:v>S1</c:v></c:pt></c:strCache></c:strRef></c:tx><c:invertIfNegative val="0"/>"#;
    let xml = replaced(&xml, first, &format!("{first}{error_bars}"));
    let mut book = workbook_with(&xml);
    let written = chart_part(&book);
    assert!(written.contains("invertSolidFillFmt") && !written.contains("errBars"));
    assert_eq!(first_series(&mut book).error_bars().len(), 2);
    let conversion = convert(&mut book, ChartSubtype::Pie).unwrap();
    let series = first_series(&mut book);
    assert!(series.error_bars().is_empty());
    let uris: Vec<Option<String>> = series
        .extensions()
        .expect("extensions")
        .entries
        .iter()
        .map(|entry| entry.uri.clone())
        .collect();
    assert_eq!(
        uris,
        [Some("{C3380CC4-5D6E-409C-BE32-E72D297353CC}".to_owned())]
    );
    let written = chart_part(&book);
    assert!(!written.contains("invertSolidFillFmt") && !written.contains("errBars"));
    // The survivor is written as umya writes the unedited part's list.
    assert_eq!(
        first_extensions(&written),
        first_extensions(&chart_part(&workbook_with(&column)))
    );
    // The report of the unedited part, with the two removals after the other
    // series parts and before the series' paint.
    let plain = convert(&mut workbook_with(&column), ChartSubtype::Pie).unwrap();
    let (parts, paint): (Vec<_>, Vec<_>) = series_changes(plain.changes(), 0)
        .into_iter()
        .partition(|(part, _)| matches!(part, ConvertedPart::SeriesPart(_)));
    let removed = [SeriesPartKind::Extensions, SeriesPartKind::ErrorBars]
        .map(|kind| (ConvertedPart::SeriesPart(kind), ChangeKind::Removed));
    let expected: Vec<_> = parts.into_iter().chain(removed).chain(paint).collect();
    assert_eq!(series_changes(conversion.changes(), 0), expected);
    assert_eq!(
        series_changes(conversion.changes(), 1),
        series_changes(plain.changes(), 1)
    );
}

/// A series whose only extension the target family removes keeps an empty
/// stored list, and the conversion reports the removal: a column series
/// writes the extension, a line series leaves it out and writes no list.
#[test]
fn a_series_whose_only_extension_goes_converts() {
    let column = replaced(&part_at(COLUMN), FIRST_UNIQUE_ID, INVERT_FILL);
    let line = replaced_between(
        &part_at(LINE),
        r#"<c:ext uri="{C3380CC4-5D6E-409C-BE32-E72D297353CC}""#,
        "</c:ext>",
        INVERT_FILL,
    );
    for (xml, source_writes) in [(column, true), (line, false)] {
        let mut book = workbook_with(&xml);
        let written = chart_part(&book);
        assert_eq!(written.contains("invertSolidFillFmt"), source_writes);
        assert_eq!(
            first_series_xml(&written).contains("<c:extLst"),
            source_writes
        );
        let conversion = convert(&mut book, ChartSubtype::Pie).unwrap();
        assert!(
            first_series(&mut book)
                .extensions()
                .is_some_and(|list| list.entries.is_empty())
        );
        let converted = chart_part(&book);
        assert!(!converted.contains("invertSolidFillFmt"));
        // The emptied list closes the series and holds no entry.
        assert!(!first_extensions(&converted).contains("<c:ext "));
        let removed = series_changes(conversion.changes(), 0)
            .into_iter()
            .filter(|(part, _)| *part == ConvertedPart::SeriesPart(SeriesPartKind::Extensions))
            .collect::<Vec<_>>();
        assert_eq!(
            removed,
            [(
                ConvertedPart::SeriesPart(SeriesPartKind::Extensions),
                ChangeKind::Removed
            )]
        );
    }
}

/// A category axis the conversion keeps keeps the extensions its writer
/// leaves out, with the same diagnostics: its own, whose namespace is not
/// declared, and its scaling's, which a scaling does not take.
#[test]
fn a_kept_axis_keeps_the_extensions_its_writer_leaves_out() {
    let column = part_at(COLUMN);
    let undeclared = replaced(&column, "</c:catAx>", &format!("{OPAQUE}</c:catAx>"));
    let scaling = replaced(
        &column,
        r#"<c:catAx><c:axId val="715099920"/><c:scaling><c:orientation val="minMax"/>"#,
        &format!(
            r#"<c:catAx><c:axId val="715099920"/><c:scaling><c:orientation val="minMax"/><c:extLst>{FIRST_UNIQUE_ID}</c:extLst>"#
        ),
    );
    // What each entry writes when it is written.
    for (xml, scaling_owner, payload) in [
        (undeclared, false, "<t:mark>"),
        (scaling, true, "<c:orientation val=\"minMax\"/><c:extLst>"),
    ] {
        let mut book = workbook_with(&xml);
        if !scaling_owner {
            let entry = &mut chart(&mut book).plot_area_mut().category_axis_mut()[0]
                .extensions_mut()
                .expect("the axis extension")
                .entries[0];
            let ExtensionNode::Unknown(root) = &mut entry.nodes[0] else {
                panic!("an opaque payload");
            };
            root.attributes.push((
                QName {
                    namespace_uri:    "urn:missing".into(),
                    local_name:       "type".into(),
                    preferred_prefix: Some("missing".into()),
                },
                "missing:Value".into(),
            ));
        }
        let state = |book: &mut Workbook| {
            let axis = chart(book).plot_area().category_axis()[0].clone();
            if scaling_owner {
                stored(book, axis.scaling().extensions(), "scaling")
            } else {
                stored(book, axis.extensions(), "catAx")
            }
        };
        let before = state(&mut book);
        assert_eq!(before.1.len(), 1, "one entry is left out: {before:?}");
        assert!(!chart_part(&book).contains(payload));
        convert(&mut book, ChartSubtype::StackedColumn).unwrap();
        assert_eq!(state(&mut book), before);
    }
}

/// Series lines a bar group stores and its writer leaves out, two shapes in
/// one line, cannot be carried into a changed group: the conversion waits,
/// and the chart keeps the line.
#[test]
fn a_changed_group_with_lines_its_writer_leaves_out_waits() {
    let line = r#"<c:serLines><c:spPr><a:ln w="9525"/></c:spPr><c:spPr><a:ln w="12700"/></c:spPr></c:serLines>"#;
    let xml = replaced(
        &part_at(COLUMN),
        r#"<c:overlap val="-27"/>"#,
        &format!(r#"<c:overlap val="-27"/>{line}"#),
    );
    let mut book = workbook_with(&xml);
    let lines = |book: &mut Workbook| {
        chart(book)
            .plot_area()
            .bar_chart()
            .expect("a bar group")
            .series_lines()
            .len()
    };
    assert_eq!(lines(&mut book), 1);
    assert!(!chart_part(&book).contains("serLines"));
    refused_unchanged(
        &mut book,
        ChartSubtype::ClusteredColumn,
        ChartSubtype::StackedColumn,
    );
    assert_eq!(lines(&mut book), 1);
}

/// `xml` with `edit` made to its first element from `start` to `end`.
fn edited_first(xml: &str, start: &str, end: &str, edit: impl FnOnce(&str) -> String) -> String {
    let from = xml.find(start).unwrap_or_else(|| panic!("no {start}"));
    let to = from + xml[from..].find(end).expect("a closed element") + end.len();
    format!("{}{}{}", &xml[..from], edit(&xml[from..to]), &xml[to..])
}

/// An axis' extension list and the one of the shape a case puts a diagnosed
/// extension in: the axis' own, or its major gridlines'.
type AxisState = (Option<ExtensionList>, Option<ExtensionList>);

fn category_state(axis: &CategoryAxis, gridlines: bool) -> AxisState {
    let shape = if gridlines {
        axis.major_gridlines()
            .and_then(MajorGridlines::shape_properties)
    } else {
        axis.shape_properties()
    };
    (
        axis.extensions().cloned(),
        shape.and_then(ShapeProperties::extensions).cloned(),
    )
}

fn value_state(axis: &ValueAxis, gridlines: bool) -> AxisState {
    let shape = if gridlines {
        axis.major_gridlines()
            .and_then(MajorGridlines::shape_properties)
    } else {
        axis.shape_properties()
    };
    (
        axis.extensions().cloned(),
        shape.and_then(ShapeProperties::extensions).cloned(),
    )
}

/// An axis that changes type keeps its id, and with it its extension list
/// and the stored state of its shapes, an extension its writer diagnoses and
/// leaves out included: a line chart's category axis with a diagnosed
/// extension in its own shape becomes a value axis, a scatter chart's x axis
/// with one in its gridlines' shape becomes a category axis.
#[test]
fn a_retyped_axis_keeps_the_state_of_the_axis_with_its_id() {
    // An extension a shape cannot write without its uri.
    let diagnosed = r#"<a:extLst><a:ext xmlns:t="urn:example:test"><t:mark/></a:ext></a:extLst>"#;
    let line = edited_first(&part_at(LINE), "<c:catAx>", "</c:catAx>", |axis| {
        let axis = replaced(
            axis,
            "<a:effectLst/></c:spPr>",
            &format!("<a:effectLst/>{diagnosed}</c:spPr>"),
        );
        replaced(&axis, "</c:catAx>", &format!("{OPAQUE}</c:catAx>"))
    });
    let scatter = edited_first(&part_at(SCATTER_LINES), "<c:valAx>", "</c:valAx>", |axis| {
        let axis = replaced(
            axis,
            "<a:effectLst/></c:spPr></c:majorGridlines>",
            &format!("<a:effectLst/>{diagnosed}</c:spPr></c:majorGridlines>"),
        );
        replaced(&axis, "</c:valAx>", &format!("{OPAQUE}</c:valAx>"))
    });
    let cases = [
        (line, 1_469_039_311, ChartSubtype::ScatterLines, false),
        (scatter, 1_465_614_736, ChartSubtype::Line, true),
    ];
    for (xml, id, target, gridlines) in cases {
        let mut book = workbook_with(&xml);
        let area = chart(&mut book).plot_area().clone();
        let before = if gridlines {
            area.value_axis()
                .iter()
                .find(|axis| axis.axis_id().val() == id)
                .map(|axis| value_state(axis, gridlines))
        } else {
            area.category_axis()
                .iter()
                .find(|axis| axis.axis_id().val() == id)
                .map(|axis| category_state(axis, gridlines))
        }
        .expect("the axis with the id");
        let diagnostics = |book: &Workbook, state: &AxisState| {
            state
                .1
                .as_ref()
                .map(|list| stored(book, Some(list), "spPr").1)
        };
        assert_eq!(
            diagnostics(&book, &before).map(|found| found.len()),
            Some(1),
            "{target:?}: the shape's extension is left out"
        );
        assert!(!chart_part(&book).contains("<t:mark/>"));
        assert!(before.0.is_some(), "{target:?}: the axis' own extension");
        let expected = (format!("{before:?}"), diagnostics(&book, &before));
        convert(&mut book, target).unwrap();
        let area = chart(&mut book).plot_area().clone();
        let after = if gridlines {
            area.category_axis()
                .iter()
                .find(|axis| axis.axis_id().val() == id)
                .map(|axis| category_state(axis, gridlines))
        } else {
            area.value_axis()
                .iter()
                .find(|axis| axis.axis_id().val() == id)
                .map(|axis| value_state(axis, gridlines))
        }
        .expect("the retyped axis with the id");
        assert_eq!(
            (format!("{after:?}"), diagnostics(&book, &after)),
            expected,
            "{target:?}"
        );
    }
}

/// Two live axes with one id cannot be told apart: the conversion refuses,
/// the chart unchanged.
#[test]
fn axes_sharing_an_id_are_refused() {
    let mut book = workbook_with(&part_at(LINE));
    chart(&mut book).plot_area_mut().value_axis_mut()[0]
        .axis_id_mut()
        .set_val(1_469_039_311);
    refused_unchanged(&mut book, ChartSubtype::Line, ChartSubtype::ScatterLines);
}
