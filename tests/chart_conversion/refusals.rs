//! What the operation decides before it converts anything: the series counts
//! Excel accepts, and a chart that already is the target.
use umya_spreadsheet::{
    Workbook,
    drawing::charts::{
        AreaChart,
        ChartGroup,
        LineChart,
    },
    helper::chart_conversion::{
        ChartConversionRefusal,
        ChartSource,
        ChartSubtype,
    },
};

use super::{
    conversion::{
        assert_unchanged,
        chart,
        chart_part,
        convert,
        count_refusal,
        excel_accepted,
        find_part,
        inserted_after,
        observations_by_part,
        open,
        replaced_between,
        series_count,
        set_series_count,
        subtype,
        without_between,
    },
    measured::{
        measured,
        read_part,
    },
    workbook::workbook_with,
};

/// Excel made 283 conversions of a chart to the type it already was.
const MEASURED_IDENTITIES: usize = 283;

/// Excel refused 2,034 conversions and made 20,293.
const MEASURED_REFUSALS: usize = 2_034;
const MEASURED_SUCCESSES: usize = 20_293;

/// Of the refusals, 26 were conversions to the type the chart already was.
const MEASURED_SAME_TYPE_REFUSALS: usize = 26;

/// Fails unless converting the chart to `target` is accepted as the type it
/// already is, with an empty report and the chart part as it was.
fn assert_no_op(book: &mut Workbook, target: ChartSubtype, what: &str) {
    let before = chart_part(book);
    let conversion =
        convert(book, target).unwrap_or_else(|refusal| panic!("{what}: refused with {refusal:?}"));
    assert_eq!(conversion.from(), ChartSource::Subtype(target), "{what}");
    assert_eq!(conversion.to(), target, "{what}");
    assert!(
        conversion.changes().is_empty(),
        "{what}: {:?}",
        conversion.changes()
    );
    assert!(
        conversion.series_paint().is_empty(),
        "{what}: {:?}",
        conversion.series_paint()
    );
    assert_unchanged(book, &before, what);
}

/// The measured pie that styles each of its 12 values, with `values` values,
/// each of them styled and cached.
fn styled_pie(values: usize) -> String {
    let part = measured()
        .parts
        .iter()
        .find(|part| {
            part.fixture
                .ends_with("many-values-12-styled/pie.chart.xml")
        })
        .expect("the measurement holds a styled pie of 12 values");
    let text = read_part(part);

    let first = text
        .find("<c:dPt>")
        .expect("a styled pie styles its points");
    let end = text
        .rfind("</c:dPt>")
        .expect("a styled pie styles its points")
        + "</c:dPt>".len();
    let styles: String = (0..values)
        .map(|point| {
            format!(
                "<c:dPt><c:idx val=\"{point}\"/><c:bubble3D \
                 val=\"0\"/><c:spPr><a:solidFill><a:srgbClr val=\"FF0000\"/></a:solidFill><a:ln \
                 w=\"19050\"><a:solidFill><a:schemeClr \
                 val=\"lt1\"/></a:solidFill></a:ln><a:effectLst/></c:spPr></c:dPt>"
            )
        })
        .collect();
    let text = format!("{}{styles}{}", &text[..first], &text[end..]);

    let last_row = values + 1;
    let names: String = (0..values)
        .map(|point| format!("<c:pt idx=\"{point}\"><c:v>C{}</c:v></c:pt>", point + 1))
        .collect();
    let numbers: String = (0..values)
        .map(|point| format!("<c:pt idx=\"{point}\"><c:v>{}</c:v></c:pt>", 10 + point))
        .collect();
    let text = replaced_between(
        &text,
        "<c:cat>",
        "</c:cat>",
        &format!(
            "<c:cat><c:strRef><c:f>Data!$AN$2:$AN${last_row}</c:f><c:strCache><c:ptCount \
             val=\"{values}\"/>{names}</c:strCache></c:strRef></c:cat>"
        ),
    );
    replaced_between(
        &text,
        "<c:val>",
        "</c:val>",
        &format!(
            "<c:val><c:numRef><c:f>Data!$AO$2:$AO${last_row}</c:f><c:numCache><c:\
             formatCode>General</c:formatCode><c:ptCount \
             val=\"{values}\"/>{numbers}</c:numCache></c:numRef></c:val>"
        ),
    )
}

#[test]
fn same_subtype_is_a_no_op() {
    let by_part = observations_by_part();
    let mut identities = 0;
    for (index, part) in measured().parts.iter().enumerate() {
        for observation in &by_part[index] {
            if observation.ok && part.subtype.as_deref() == Some(observation.target.as_str()) {
                assert_no_op(&mut open(part), subtype(&observation.target), &part.fixture);
                identities += 1;
            }
        }
    }
    assert_eq!(identities, MEASURED_IDENTITIES);

    // Charts beyond what was measured are accepted as well.
    let surface = find_part("Surface3D", 3);
    let mut nine = open(surface);
    set_series_count(&mut nine, 9);
    assert_eq!(series_count(&mut nine), 9);
    assert_no_op(
        &mut nine,
        ChartSubtype::Surface3D,
        "a surface of nine series",
    );

    let mut without_bands = workbook_with(&without_between(
        &read_part(surface),
        "<c:bandFmts>",
        "</c:bandFmts>",
    ));
    assert!(
        chart(&mut without_bands)
            .plot_area()
            .surface_3d_chart()
            .expect("a surface chart")
            .band_formats()
            .is_none()
    );
    assert_no_op(
        &mut without_bands,
        ChartSubtype::Surface3D,
        "a surface without bands",
    );

    let mut pie = workbook_with(&styled_pie(60));
    assert_no_op(&mut pie, ChartSubtype::Pie, "a styled pie of 60 values");

    let mut empty = open(find_part("ClusteredColumn", 3));
    set_series_count(&mut empty, 0);
    assert_eq!(series_count(&mut empty), 0);
    assert_no_op(
        &mut empty,
        ChartSubtype::ClusteredColumn,
        "a column chart without series",
    );
}

#[test]
fn series_count_refusals_follow_excel_and_leave_the_chart_alone() {
    let by_part = observations_by_part();
    let (mut refused, mut accepted) = (0, 0);
    for (index, part) in measured().parts.iter().enumerate() {
        let mut book = open(part);
        let original = chart(&mut book).clone();
        let before = chart_part(&book);
        for observation in &by_part[index] {
            // Each observation starts from the part as it was measured, so a
            // conversion of one never changes another.
            *chart(&mut book) = original.clone();
            let found = convert(&mut book, subtype(&observation.target)).err();
            let what = format!("{} to {}", part.fixture, observation.target);
            if observation.ok {
                // Excel made the conversion, so no count rule may refuse it.
                // What the operation does with the pair is not asserted.
                assert!(
                    !matches!(
                        found,
                        Some(
                            ChartConversionRefusal::SeriesCountMismatch { .. }
                                | ChartConversionRefusal::NotValidForKind
                        )
                    ),
                    "{what}: Excel made it, the operation refused it for its count: {found:?}"
                );
                accepted += 1;
            } else {
                assert_eq!(
                    found,
                    count_refusal(&observation.target, part.series),
                    "{what}: Excel refused it, {} series",
                    part.series
                );
                assert_unchanged(&book, &before, &what);
                refused += 1;
            }
        }
    }
    assert_eq!(refused, MEASURED_REFUSALS);
    assert_eq!(accepted, MEASURED_SUCCESSES);
}

#[test]
fn refusal_precedence_is_fixed() {
    // The chart is read first: three groups and one series into a surface or a
    // stock chart are refused for their groups, not their series count.
    let mut three = open(find_part("ClusteredColumn", 1));
    chart(&mut three)
        .plot_area_mut()
        .add_chart_group(ChartGroup::LineChart(LineChart::default()))
        .add_chart_group(ChartGroup::AreaChart(AreaChart::default()));
    assert_eq!(series_count(&mut three), 1);
    let before = chart_part(&three);
    for target in [ChartSubtype::Surface3D, ChartSubtype::HighLowClose] {
        assert_eq!(
            convert(&mut three, target).err(),
            Some(ChartConversionRefusal::MultipleGroups),
            "three groups and one series into {target:?}"
        );
    }
    assert_unchanged(&three, &before, "three groups and one series");

    let column = read_part(find_part("ClusteredColumn", 1));
    let mut cylinder = workbook_with(&inserted_after(
        &column,
        "</c:val>",
        "<c:shape val=\"cylinder\"/>",
    ));
    assert_eq!(
        convert(&mut cylinder, ChartSubtype::Surface3D).err(),
        Some(ChartConversionRefusal::Unsupported {
            from: None,
            to:   ChartSubtype::Surface3D,
        }),
        "an unrecognised column chart of one series into a surface"
    );

    // A surface needs two series.
    let surface = find_part("Surface3D", 1);
    assert!(!excel_accepted(surface, "Contour"));
    let mut book = open(surface);
    let before = chart_part(&book);
    assert_eq!(
        convert(&mut book, ChartSubtype::Contour).err(),
        Some(ChartConversionRefusal::NotValidForKind),
        "a surface of one series into a contour chart"
    );
    assert_unchanged(&book, &before, "a surface of one series");

    // A stock type needs its exact number of series.
    let stock = find_part("HighLowClose", 2);
    assert!(!excel_accepted(stock, "OpenHighLowClose"));
    let mut book = open(stock);
    let before = chart_part(&book);
    assert_eq!(
        convert(&mut book, ChartSubtype::OpenHighLowClose).err(),
        Some(ChartConversionRefusal::SeriesCountMismatch {
            required: 4,
            actual:   2,
        }),
        "a high-low-close chart of two series into open-high-low-close"
    );
    assert_unchanged(&book, &before, "a high-low-close chart of two series");

    // The count checks come before the same type: Excel refused each of these
    // conversions to the type the chart already was.
    let by_part = observations_by_part();
    let mut same_type = 0;
    for (index, part) in measured().parts.iter().enumerate() {
        for observation in &by_part[index] {
            if observation.ok || part.subtype.as_deref() != Some(observation.target.as_str()) {
                continue;
            }
            assert_eq!(
                convert(&mut open(part), subtype(&observation.target)).err(),
                count_refusal(&observation.target, part.series),
                "{} to its own type {}",
                part.fixture,
                observation.target
            );
            same_type += 1;
        }
    }
    assert_eq!(same_type, MEASURED_SAME_TYPE_REFUSALS);
}

#[test]
fn series_counts_above_u8_saturate() {
    for (series, target, required) in [
        (300, ChartSubtype::HighLowClose, 3),
        (256, ChartSubtype::VolumeOpenHighLowClose, 5),
    ] {
        let mut book = open(find_part("ClusteredColumn", 1));
        set_series_count(&mut book, series);
        assert_eq!(series_count(&mut book), series);
        let before = chart_part(&book);
        let found = convert(&mut book, target).err();
        assert_eq!(
            found,
            Some(ChartConversionRefusal::SeriesCountMismatch {
                required,
                actual: 255,
            }),
            "{series} series into {target:?}"
        );
        // The count is a cap, so the message does not claim the chart has 255.
        assert_eq!(
            found.map(|refusal| refusal.to_string()),
            Some(format!(
                "the target needs exactly {required} series and the chart has 255 or more"
            )),
            "{series} series into {target:?}"
        );
        assert_unchanged(&book, &before, "a chart of more than 255 series");
    }

    // A count below the cap is shown as it is.
    assert_eq!(
        ChartConversionRefusal::SeriesCountMismatch {
            required: 4,
            actual:   2,
        }
        .to_string(),
        "the target needs exactly 4 series and the chart has 2"
    );
}
