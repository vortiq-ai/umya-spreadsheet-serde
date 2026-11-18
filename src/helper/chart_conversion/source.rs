//! The chart before a conversion: the tree umya's writer emits for it and the
//! classes the rule table selects by (section 3.1).
use std::collections::BTreeSet;

use super::{
    ChartSource,
    ChartSubtype,
    model::{
        POINT_ACCENT_MAX,
        accent_paint,
        fill_of,
        paint_of,
    },
    plan::{
        Step,
        Unresolved,
    },
    slots::{
        CategoryDataClass,
        ColorClass,
        DateSpacing,
        MarkerClass,
        SeriesStyleClass,
        SourceGroupClass,
        SourceSeriesRole,
        ValueCountClass,
    },
    xml::{
        self,
        Element,
    },
};
use crate::{
    Workbook,
    helper::{
        address::{
            split_address,
            unescape_sheet_name,
        },
        coordinate::index_from_coordinate,
        date::excel_to_date_time_jiff_checked,
    },
    structs::{
        ColumnReference,
        RowReference,
        drawing::charts::{
            Chart,
            Formula,
            chart_reference_roles::{
                self,
                ReferenceTargetRef,
            },
        },
    },
};

/// One series of the source, in series order.
#[derive(Debug)]
pub(super) struct SourceSeries {
    /// The series' `c:idx`, which it keeps.
    pub(super) index:    u32,
    /// The source plot group it sits in.
    pub(super) group:    usize,
    pub(super) element:  Element,
    pub(super) style:    SeriesStyleClass,
    pub(super) fill:     ColorClass,
    pub(super) line:     ColorClass,
    pub(super) marker:   MarkerClass,
    pub(super) role:     SourceSeriesRole,
    pub(super) group_of: SourceGroupClass,
    /// Its value count: the `ptCount` of its values, as Excel's cache holds it.
    pub(super) values:   Option<usize>,
}

/// The source chart.
#[derive(Debug)]
pub(super) struct Source {
    pub(super) from:     ChartSource,
    /// `c:chart` as umya writes it into the workbook: every cache holds the
    /// cell values its reference reads, as Excel's caches do.
    pub(super) chart:    Element,
    pub(super) series:   Vec<SourceSeries>,
    /// Whether any series has shape properties.
    pub(super) styled:   bool,
    /// The style the 3-D scene and the group trees are selected by.
    pub(super) scene:    SeriesStyleClass,
    /// Whether the series hold x values instead of categories.
    pub(super) has_x:    bool,
    /// The class of the series' categories, and of their x values.
    pub(super) category: (CategoryDataClass, DateSpacing),
    pub(super) x:        (CategoryDataClass, DateSpacing),
}

const fn style_of(styled: bool) -> SeriesStyleClass {
    if styled {
        SeriesStyleClass::Styled
    } else {
        SeriesStyleClass::Automatic
    }
}

/// The colour class of a fill (section 4.1): a complete palette subtree at
/// the series' own index is the series accent.
pub(super) fn color_class(fill: Option<&Element>, index: u32) -> ColorClass {
    let Some(fill) = fill else {
        return ColorClass::Absent;
    };
    let paint = paint_of(fill);
    if paint == "noFill" {
        return ColorClass::NoFill;
    }
    let alpha = paint
        .strip_suffix(']')
        .and_then(|rest| rest.rsplit_once("[alpha="))
        .map(|(_, alpha)| alpha);
    let accent = usize::try_from(index)
        .ok()
        .filter(|index| *index <= POINT_ACCENT_MAX)
        .and_then(|index| accent_paint(index, alpha));
    if accent.as_deref() == Some(paint.as_str()) {
        ColorClass::SeriesAccent
    } else {
        ColorClass::Explicit
    }
}

/// The value count of a series: the first `ptCount` of its values.
fn values_of(series: &Element) -> Option<usize> {
    ["val", "yVal"]
        .into_iter()
        .find_map(|role| series.child(role))
        .and_then(|data| data.find("ptCount"))
        .and_then(|count| count.attribute("val")?.parse().ok())
}

/// The sheets, by index, whose cells the references of `chart` read when umya
/// writes it, once every reference is one umya can write: an address the
/// writer can spell (no zero column or row), and for a reference that reads
/// cells, a range the cell reader projects (two complete corners, the first
/// not after the second). A sheet the workbook does not hold is no sheet; its
/// cells read as empty.
pub(super) fn referenced_sheets(
    chart: &Chart,
    workbook: &Workbook,
) -> Result<Vec<usize>, Unresolved> {
    let mut sheets = BTreeSet::new();
    let mut invalid = false;
    let mut whole_line = false;
    chart_reference_roles::visit_chart_ref(chart, &mut |_, target| {
        // A multi-level reference writes its own cache, a pivot chart's
        // numbers come from the pivot cache, and a bare formula is metadata.
        let reads_cells = match &target {
            ReferenceTargetRef::Number(reference) => reference.pivot_cache().is_none(),
            ReferenceTargetRef::String(_) => true,
            ReferenceTargetRef::MultiLevel(_) | ReferenceTargetRef::Formula(_) => false,
        };
        let formula = target.formula();
        match reference_class(formula, reads_cells) {
            ReferenceClass::Invalid => invalid = true,
            ReferenceClass::WholeLine => whole_line = true,
            ReferenceClass::Readable if reads_cells && !formula.has_string_value() => {
                let name = unescape_sheet_name(formula.address().sheet_name());
                if let Ok(index) = workbook.find_sheet_index_by_name(&name) {
                    sheets.insert(index);
                }
            }
            ReferenceClass::Readable => {}
        }
    });
    if invalid {
        return Err(Unresolved::Invalid);
    }
    if whole_line {
        return Err(Unresolved::Pending(Step::WholeLineReference));
    }
    Ok(sheets.into_iter().collect())
}

enum ReferenceClass {
    Readable,
    /// A whole column or row.
    WholeLine,
    /// An address the writer cannot spell, or a range the cell reader cannot
    /// iterate.
    Invalid,
}

/// How umya can write `formula`, read as a reference that reads cells or not.
fn reference_class(formula: &Formula, reads_cells: bool) -> ReferenceClass {
    if formula.has_string_value() {
        // The address parser takes complete cells only, so a whole column or
        // row read from a file is kept as text.
        return if reads_cells && spells_whole_line(&formula.address_str()) {
            ReferenceClass::WholeLine
        } else {
            ReferenceClass::Readable
        };
    }
    let range = formula.address().range();
    let start = (
        range.coordinate_start_col().map(ColumnReference::num),
        range.coordinate_start_row().map(RowReference::num),
    );
    // A single cell is stored without an end.
    let end = match (
        range.coordinate_end_col().map(ColumnReference::num),
        range.coordinate_end_row().map(RowReference::num),
    ) {
        (None, None) => start,
        end => end,
    };
    if [start.0, start.1, end.0, end.1].contains(&Some(0)) {
        return ReferenceClass::Invalid;
    }
    if !reads_cells || start == (None, None) {
        return ReferenceClass::Readable;
    }
    match (start, end) {
        ((Some(first_col), Some(first_row)), (Some(last_col), Some(last_row))) => {
            if first_col > last_col || first_row > last_row {
                ReferenceClass::Invalid
            } else {
                ReferenceClass::Readable
            }
        }
        _ => ReferenceClass::WholeLine,
    }
}

/// Whether `text` spells a whole column (`Data!$F:$F`) or a whole row
/// (`Data!$2:$2`).
fn spells_whole_line(text: &str) -> bool {
    let (_, range) = split_address(text);
    let Some((first, last)) = range.split_once(':') else {
        return false;
    };
    let shape = |corner: &str| {
        let (col, row, ..) = index_from_coordinate(corner);
        (col.is_some(), row.is_some())
    };
    matches!(
        (shape(first), shape(last)),
        ((true, false), (true, false)) | ((false, true), (false, true))
    )
}

/// The month of a serial that is the first day of a month, counted from year
/// 0; `None` for any other day.
fn month_of(serial: &str) -> Option<i64> {
    let date = excel_to_date_time_jiff_checked(serial.parse().ok()?)
        .ok()?
        .date();
    (date.day() == 1).then_some(i64::from(date.year()) * 12 + i64::from(date.month()))
}

/// Section 3.1's spacing of numeric categories: consecutive serials (General
/// numbers only as exactly 1 to N) are daily, consecutive first days of a
/// month monthly.
fn spacing(values: &[&str], general: bool) -> DateSpacing {
    let numbers: Option<Vec<f64>> = values.iter().map(|value| value.parse().ok()).collect();
    let Some(numbers) = numbers.filter(|numbers| numbers.len() >= 2) else {
        return DateSpacing::Other;
    };
    if general {
        let counting = numbers
            .iter()
            .zip(1_u32..)
            .all(|(number, position)| (*number - f64::from(position)).abs() < f64::EPSILON);
        return if counting {
            DateSpacing::Daily
        } else {
            DateSpacing::Other
        };
    }
    if numbers
        .windows(2)
        .all(|pair| (pair[1] - pair[0] - 1.0).abs() < f64::EPSILON)
    {
        return DateSpacing::Daily;
    }
    let months: Option<Vec<i64>> = values.iter().map(|value| month_of(value)).collect();
    if months.is_some_and(|months| months.windows(2).all(|pair| pair[1] - pair[0] == 1)) {
        DateSpacing::Monthly
    } else {
        DateSpacing::Other
    }
}

/// The number format of ISO dates.
const DATE_CODE: &str = "yyyy\\-mm\\-dd";

/// The data and spacing class of one series' categories or x values, read
/// from their cache.
fn data_class(data: Option<&Element>) -> (CategoryDataClass, DateSpacing) {
    let Some(data) = data else {
        return (CategoryDataClass::Absent, DateSpacing::Absent);
    };
    if data
        .child("strRef")
        .is_some_and(|reference| reference.child("strCache").is_some())
    {
        return (CategoryDataClass::TextReference, DateSpacing::Absent);
    }
    let Some(cache) = data
        .child("numRef")
        .and_then(|reference| reference.child("numCache"))
    else {
        return (CategoryDataClass::Other, DateSpacing::Absent);
    };
    let code = cache.find("formatCode").map(|code| code.text.as_str());
    let values: Vec<&str> = cache
        .children_named("pt")
        .filter_map(|point| point.child("v").map(|value| value.text.as_str()))
        .collect();
    let class = match code {
        Some(DATE_CODE) => CategoryDataClass::DateSerialReference,
        Some("General") => CategoryDataClass::GeneralReference,
        _ => CategoryDataClass::Other,
    };
    (class, spacing(&values, code == Some("General")))
}

/// The one class every series has, or `Other` when they differ.
fn uniform(
    mut classes: impl Iterator<Item = (CategoryDataClass, DateSpacing)>,
) -> (CategoryDataClass, DateSpacing) {
    let Some(first) = classes.next() else {
        return (CategoryDataClass::Absent, DateSpacing::Absent);
    };
    if classes.all(|class| class == first) {
        first
    } else {
        (CategoryDataClass::Other, DateSpacing::Other)
    }
}

impl Source {
    /// Reads `chart`, classified as `from`, with the cell values of
    /// `workbook`; its references passed `referenced_sheets`. `None` when
    /// umya cannot write it.
    pub(super) fn read(chart: &Chart, workbook: &Workbook, from: ChartSource) -> Option<Self> {
        let tree = xml::written(|writer| chart.write_to(writer, workbook))?;
        let area = tree.child("plotArea")?;
        let groups: Vec<&Element> = area
            .children
            .iter()
            .filter(|child| child.local().ends_with("Chart"))
            .collect();
        let mut series = Vec::new();
        for (group, element) in groups.iter().enumerate() {
            let group_of = match (from, element.local()) {
                (ChartSource::Subtype(_), _) => SourceGroupClass::Single,
                (ChartSource::Combo(_), "areaChart") => SourceGroupClass::ComboArea,
                (ChartSource::Combo(_), "lineChart") => SourceGroupClass::ComboLine,
                (ChartSource::Combo(_), _) => SourceGroupClass::ComboBar,
            };
            for ser in element.children_named("ser") {
                let index = ser.val("idx").and_then(|idx| idx.parse().ok())?;
                let shape = ser.child("spPr");
                let marker = match ser.child("marker").and_then(|marker| marker.val("symbol")) {
                    None => MarkerClass::Absent,
                    Some("none") => MarkerClass::Hidden,
                    Some(_) => MarkerClass::Shown,
                };
                let line = shape.and_then(|shape| shape.child("ln")).and_then(fill_of);
                series.push(SourceSeries {
                    index,
                    group,
                    style: style_of(shape.is_some()),
                    fill: color_class(shape.and_then(fill_of), index),
                    line: color_class(line, index),
                    marker,
                    role: SourceSeriesRole::Ordinary,
                    group_of,
                    values: values_of(ser),
                    element: ser.clone(),
                });
            }
        }
        let first = series.first().map(|first| first.index);
        for entry in &mut series {
            entry.role = role_of(from, entry, first, &groups);
        }
        let styled = series
            .iter()
            .any(|entry| entry.style == SeriesStyleClass::Styled);
        let varied = groups
            .iter()
            .any(|group| group.val("varyColors") == Some("1"));
        let scene_styled = if varied {
            series.len() == 1
                && series
                    .first()
                    .is_some_and(|entry| entry.element.child("dPt").is_some())
        } else {
            styled
        };
        let has_x = series
            .iter()
            .all(|entry| entry.element.child("cat").is_none())
            && series
                .iter()
                .any(|entry| entry.element.child("xVal").is_some());
        let category = uniform(
            series
                .iter()
                .map(|entry| data_class(entry.element.child("cat"))),
        );
        let x = uniform(
            series
                .iter()
                .map(|entry| data_class(entry.element.child("xVal"))),
        );
        Some(Self {
            from,
            chart: tree,
            series,
            styled,
            scene: style_of(scene_styled),
            has_x,
            category,
            x,
        })
    }

    /// The category data and spacing class of the categories after data-role
    /// remapping: x values that become categories are classed as categories;
    /// into an xy target x stays x and selects nothing.
    pub(super) const fn categories(&self, xy_target: bool) -> (CategoryDataClass, DateSpacing) {
        match (self.has_x, xy_target) {
            (true, false) => self.x,
            (true, true) => (CategoryDataClass::Absent, DateSpacing::Absent),
            (false, _) => self.category,
        }
    }

    /// The style the axes' trees are selected by.
    pub(super) const fn style(&self) -> SeriesStyleClass {
        style_of(self.styled)
    }

    /// The source plot group `group`, if the source has it.
    pub(super) fn group(&self, group: usize) -> Option<&Element> {
        self.chart
            .child("plotArea")?
            .children
            .iter()
            .filter(|child| child.local().ends_with("Chart"))
            .nth(group)
    }
}

impl SourceSeries {
    /// The series' value count, as the table's count class.
    pub(super) const fn value_count(&self) -> ValueCountClass {
        match self.values {
            Some(6) => ValueCountClass::Six,
            Some(7) => ValueCountClass::Seven,
            Some(12) => ValueCountClass::Twelve,
            Some(16) => ValueCountClass::Sixteen,
            Some(24) => ValueCountClass::TwentyFour,
            Some(50) => ValueCountClass::Fifty,
            _ => ValueCountClass::Other,
        }
    }
}

/// Section 3.1's source role: surface first or remaining, volume bar or stock.
fn role_of(
    from: ChartSource,
    series: &SourceSeries,
    first: Option<u32>,
    groups: &[&Element],
) -> SourceSeriesRole {
    let surface = matches!(
        from,
        ChartSource::Subtype(
            ChartSubtype::Surface3D
                | ChartSubtype::WireframeSurface3D
                | ChartSubtype::Contour
                | ChartSubtype::WireframeContour
        )
    );
    let volume = matches!(
        from,
        ChartSource::Subtype(
            ChartSubtype::VolumeHighLowClose | ChartSubtype::VolumeOpenHighLowClose
        )
    );
    if surface {
        return if Some(series.index) == first {
            SourceSeriesRole::SurfaceFirst
        } else {
            SourceSeriesRole::SurfaceRemaining
        };
    }
    if volume {
        let bar = groups
            .get(series.group)
            .is_some_and(|group| group.local() == "barChart");
        return if bar {
            SourceSeriesRole::Volume
        } else {
            SourceSeriesRole::Stock
        };
    }
    SourceSeriesRole::Ordinary
}
