//! Runs the operation on the charts of the measured parts and reads what it
//! reports, through its public API only. The expectations the tests hold it to
//! come from the measurement, never from the operation.
use umya_spreadsheet::{
    Workbook,
    drawing::charts::{
        AreaChartSeries,
        Chart,
        ChartGroupMut,
    },
    helper::chart_conversion::{
        ChartConversion,
        ChartConversionRefusal,
        ChartSource,
        ChartSubtype,
    },
};

use super::{
    fixture::chart_xml,
    measured::{
        Observation,
        Part,
        measured,
        read_part,
    },
    workbook::workbook_with,
};

/// The chart of the workbook's first sheet.
pub fn chart(book: &mut Workbook) -> &mut Chart {
    book.sheet_mut(0).unwrap().chart_collection_mut()[0]
        .chart_space_mut()
        .chart_mut()
}

/// The workbook around a measured part.
pub fn open(part: &Part) -> Workbook {
    workbook_with(&read_part(part))
}

/// The first native part of `subtype` that holds `series` series.
pub fn find_part(subtype: &str, series: usize) -> &'static Part {
    measured()
        .parts
        .iter()
        .find(|part| {
            part.class == "native"
                && part.subtype.as_deref() == Some(subtype)
                && part.series == series
        })
        .unwrap_or_else(|| panic!("no native {subtype} part holds {series} series"))
}

/// The measured observations of each part, in measurement order.
pub fn observations_by_part() -> Vec<Vec<&'static Observation>> {
    let mut by_part = vec![Vec::new(); measured().parts.len()];
    for observation in &measured().observations {
        by_part[observation.part].push(observation);
    }
    by_part
}

/// The subtype the measurement spells `name`.
pub fn subtype(name: &str) -> ChartSubtype {
    *ChartSubtype::ALL
        .iter()
        .find(|candidate| format!("{candidate:?}") == name)
        .unwrap_or_else(|| panic!("{name} is no ChartSubtype"))
}

pub fn convert(
    book: &mut Workbook,
    target: ChartSubtype,
) -> Result<ChartConversion, ChartConversionRefusal> {
    book.convert_chart_type(0, 0, target)
}

/// What the operation reads a chart as. The public API gives no direct view of
/// a chart's source, so the chart is converted to a clustered column chart: a
/// chart that already is one returns its report, any other recognised chart
/// is refused with its source.
#[derive(Debug, PartialEq, Eq)]
pub enum Reading {
    /// A subtype, as `ChartSubtype` spells it in `Debug`.
    Subtype(String),
    /// A combination layout, as `ComboLayout` spells it in `Debug`.
    Combo(String),
    Unrecognised,
    MultipleGroups,
}

pub fn read(book: &mut Workbook) -> Reading {
    let source = match convert(book, ChartSubtype::ClusteredColumn) {
        Ok(conversion) => conversion.from(),
        Err(ChartConversionRefusal::Unsupported {
            from: Some(source), ..
        }) => source,
        Err(ChartConversionRefusal::Unsupported { from: None, .. }) => {
            return Reading::Unrecognised;
        }
        Err(ChartConversionRefusal::MultipleGroups) => return Reading::MultipleGroups,
        Err(other) => panic!("a column chart target has no series count rule: {other:?}"),
    };
    match source {
        ChartSource::Subtype(subtype) => Reading::Subtype(format!("{subtype:?}")),
        ChartSource::Combo(layout) => Reading::Combo(format!("{layout:?}")),
    }
}

/// The chart part the workbook writes.
pub fn chart_part(book: &Workbook) -> String {
    chart_xml(book)
}

/// Fails unless the workbook writes the chart part it wrote `before`.
pub fn assert_unchanged(book: &Workbook, before: &str, what: &str) {
    assert!(
        chart_part(book) == before,
        "{what}: the chart part is not the one written before the conversion"
    );
}

/// How many series the chart's plot groups hold together.
pub fn series_count(book: &mut Workbook) -> usize {
    chart(book)
        .plot_area()
        .chart_groups()
        .map(|group| group.series().area_chart_series().len())
        .sum()
}

/// The chart's only plot group with `count` series, each a copy of its first.
pub fn set_series_count(book: &mut Workbook, count: usize) {
    let group = chart(book)
        .plot_area_mut()
        .chart_groups_mut()
        .next()
        .expect("the chart has a plot group");
    let series = match group {
        ChartGroupMut::BarChart(group) => group.area_chart_series_list_mut(),
        ChartGroupMut::Surface3DChart(group) => group.area_chart_series_list_mut(),
        other => panic!("no series list is edited for {other:?}"),
    };
    let first = series.area_chart_series()[0].clone();
    let copies: Vec<AreaChartSeries> = (0..count)
        .map(|position| {
            let number = u32::try_from(position).unwrap();
            let mut copy = first.clone();
            copy.index_mut().set_val(number);
            copy.order_mut().set_val(number);
            copy
        })
        .collect();
    series.set_area_chart_series(copies);
}

/// `text` with its only `from` replaced by `to`.
pub fn replaced(text: &str, from: &str, to: &str) -> String {
    assert_eq!(
        text.matches(from).count(),
        1,
        "{from} is not in the part once"
    );
    text.replacen(from, to, 1)
}

/// `text` with every `from` replaced by `to`.
pub fn replaced_all(text: &str, from: &str, to: &str) -> String {
    assert!(text.contains(from), "{from} is not in the part");
    text.replace(from, to)
}

/// `text` with its last `from` replaced by `to`.
pub fn replaced_last(text: &str, from: &str, to: &str) -> String {
    let start = text
        .rfind(from)
        .unwrap_or_else(|| panic!("{from} is not in the part"));
    format!("{}{to}{}", &text[..start], &text[start + from.len()..])
}

/// `text` with `insertion` after the first `after`.
pub fn inserted_after(text: &str, after: &str, insertion: &str) -> String {
    let end = text
        .find(after)
        .unwrap_or_else(|| panic!("{after} is not in the part"))
        + after.len();
    format!("{}{insertion}{}", &text[..end], &text[end..])
}

/// `text` with everything from the first `start` to the next `end`, both
/// included, replaced by `replacement`.
pub fn replaced_between(text: &str, start: &str, end: &str, replacement: &str) -> String {
    let from = text
        .find(start)
        .unwrap_or_else(|| panic!("{start} is not in the part"));
    let to = from
        + text[from..]
            .find(end)
            .unwrap_or_else(|| panic!("{end} does not follow {start}"))
        + end.len();
    format!("{}{replacement}{}", &text[..from], &text[to..])
}

/// `text` without everything from the first `start` to the next `end`.
pub fn without_between(text: &str, start: &str, end: &str) -> String {
    replaced_between(text, start, end, "")
}

/// Whether Excel made the conversion of `part` to `target`, spelled as the
/// measurement spells it.
pub fn excel_accepted(part: &Part, target: &str) -> bool {
    let position = measured()
        .parts
        .iter()
        .position(|candidate| std::ptr::eq(candidate, part))
        .expect("a measured part");
    measured()
        .observations
        .iter()
        .find(|observation| observation.part == position && observation.target == target)
        .unwrap_or_else(|| {
            panic!(
                "Excel was not asked to convert {} to {target}",
                part.fixture
            )
        })
        .ok
}

/// Excel's series-count rules for `target` (spelled as the measurement spells
/// it) and a chart of `series` series: what the operation must refuse with, or
/// `None` when Excel accepts the count. Every surface type needs two series.
/// High-low-close needs three, open-high-low-close and volume high-low-close
/// four, and volume open-high-low-close five.
pub fn count_refusal(target: &str, series: usize) -> Option<ChartConversionRefusal> {
    let exactly = |required: u8| {
        (series != usize::from(required)).then(|| ChartConversionRefusal::SeriesCountMismatch {
            required,
            actual: u8::try_from(series).unwrap_or(u8::MAX),
        })
    };
    match target {
        "Surface3D" | "WireframeSurface3D" | "Contour" | "WireframeContour" => {
            (series < 2).then_some(ChartConversionRefusal::NotValidForKind)
        }
        "HighLowClose" => exactly(3),
        "OpenHighLowClose" | "VolumeHighLowClose" => exactly(4),
        "VolumeOpenHighLowClose" => exactly(5),
        _ => None,
    }
}
