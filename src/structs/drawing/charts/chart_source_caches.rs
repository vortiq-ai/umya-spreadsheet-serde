// What a chart file cached for a category or text reference, kept in the
// reference's `StringCache`.
//
// A chart is exported with the labels the cells display, so a cached label is
// only worth keeping when the cells cannot reproduce it. The cache therefore
// keeps only those exceptions, and the shape of the cache when it is not the
// dense one the cells give. A workbook admits whole caches up to a budget.
use std::collections::HashMap;

use super::{
    ChartSpace,
    StringReference,
};
use crate::structs::CacheDisplay;

/// Most stored points a workbook may keep, counted over every stored cache:
/// the exception points plus the index runs.
pub const MAX_SOURCE_CACHE_POINTS: usize = 16_384;

/// Most text bytes a workbook may keep: the UTF-8 bytes of every stored
/// `value` and `loaded_display`.
pub const MAX_SOURCE_CACHE_TEXT_BYTES: usize = 1_048_576;

/// What the cache of one string reference stored in the file, reduced to what
/// the cells cannot reproduce.
///
/// `index_runs` lists the point indices in file order. When it is empty the
/// indices are the dense `0..point_count`. `formula` is the reference the file
/// had; a cache is only written back while the reference still has it. This
/// guard also protects formula edits made in place. Repeated indices retain
/// every occurrence as an exception in source order. Runs use wide endpoints.
///
/// Trust only reader-produced caches and trusted server persistence. Plain
/// serde does not validate trust: declared counts and run lengths control
/// export iteration. Clear client-created caches or adopt from server data.
/// The logical import caps exclude formula text, allocation overhead, raw XML,
/// parsing temporaries and later copies or snapshots.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub struct SourceCache {
    formula:     String,
    point_count: Option<u32>,
    index_runs:  Vec<IndexRun>,
    exceptions:  Vec<SourceException>,
}

/// `len` consecutive point indices starting at `start`.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct IndexRun {
    pub(crate) start: u32,
    pub(crate) len:   u32,
}

/// A point whose stored text is not the text its cell displayed when the file
/// was loaded. `value` is `None` when the point had no `c:v`, and
/// `loaded_display` is `None` when no cell could be resolved for it.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub struct SourceException {
    index:          u32,
    value:          Option<String>,
    loaded_display: Option<String>,
}

/// Import outcome for source caches, after reduction and workbook admission.
/// This is not live accounting after callers edit or strip charts. Every
/// occurrence is charged independently, including repeated formulas.
/// `dropped_caches = reduced_caches + over_cap_caches`. Point costs count
/// records (exceptions plus runs), never expanded run lengths. The caps are
/// logical import budgets, not heap limits. Backend budget warnings should
/// use `over_cap_caches > 0`, since reproducible elimination loses no labels.
///
/// Eager import finishes this report before returning. Lazy reports accumulate
/// in sheet-opening order, so winners near the cap can differ. References
/// into sheets materialized in an earlier call conservatively use generated
/// output, even when no edit occurred; source labels can be lost that way.
/// Still-raw references retain shape so export can stream their labels; this
/// can change kept/reduced counts compared with eager import. Repeated access
/// never recharges a cache.
///
/// Once a reproducible cache over a materialized sheet has been eliminated,
/// later deletion of its data sheet uses generated missing-sheet output,
/// which can be an empty cache instead of the original shape with empty
/// values. Import budgeting, conservative lazy freshness exclusions, this
/// shape difference and incremental raw-sheet retention distinguish this
/// representation from retaining every imported point.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct ChartSourceCacheReport {
    /// Caches kept in their charts.
    pub kept_caches:         usize,
    /// Stored points of the kept caches: exceptions plus index runs.
    pub kept_points:         usize,
    /// Stored text bytes of the kept caches.
    pub kept_text_bytes:     usize,
    /// Reduced caches plus caches rejected by the budget.
    pub dropped_caches:      usize,
    /// Reproducible caches and conservative lazy freshness exclusions.
    pub reduced_caches:      usize,
    /// Whole reduced candidates rejected by the workbook budget.
    pub over_cap_caches:     usize,
    /// Proposed stored entries of rejected candidates.
    pub over_cap_points:     usize,
    /// Proposed UTF-8 text bytes of rejected candidates.
    pub over_cap_text_bytes: usize,
}

impl SourceCache {
    pub(crate) fn set_formula(&mut self, formula: String) {
        self.formula = formula;
    }

    /// The reference the file had.
    #[must_use]
    pub fn formula(&self) -> &str {
        &self.formula
    }

    /// The `c:ptCount` the file declared, if any.
    #[must_use]
    pub fn point_count(&self) -> Option<u32> {
        self.point_count
    }

    /// The point indices in file order; empty means dense `0..point_count`.
    #[must_use]
    pub fn index_runs(&self) -> &[IndexRun] {
        &self.index_runs
    }

    /// The points the cells cannot reproduce, in file order.
    #[must_use]
    pub fn exceptions(&self) -> &[SourceException] {
        &self.exceptions
    }

    /// Exceptions plus index runs.
    #[must_use]
    pub fn stored_points(&self) -> usize {
        self.exceptions.len().saturating_add(self.index_runs.len())
    }

    /// UTF-8 bytes of the stored `value` and `loaded_display` texts.
    #[must_use]
    pub fn stored_text_bytes(&self) -> usize {
        self.exceptions
            .iter()
            .map(|exception| {
                exception
                    .value
                    .as_ref()
                    .map_or(0, String::len)
                    .saturating_add(exception.loaded_display.as_ref().map_or(0, String::len))
            })
            .fold(0, usize::saturating_add)
    }

    #[cfg(test)]
    pub(crate) fn retained_capacities(&self) -> (usize, usize) {
        (self.exceptions.capacity(), self.index_runs.capacity())
    }

    /// Heap storage owned by this source, including spare capacity.
    pub(crate) fn retained_bytes(&self) -> Option<usize> {
        self.storage_bytes(true)
    }

    /// Clones of Vec and String allocate their lengths, not spare capacity.
    pub(crate) fn cloned_retained_bytes(&self) -> Option<usize> {
        self.storage_bytes(false)
    }

    fn storage_bytes(&self, capacity: bool) -> Option<usize> {
        let text_bytes = |text: &String| {
            if capacity {
                text.capacity()
            } else {
                text.len()
            }
        };
        let runs = if capacity {
            self.index_runs.capacity()
        } else {
            self.index_runs.len()
        };
        let exceptions = if capacity {
            self.exceptions.capacity()
        } else {
            self.exceptions.len()
        };
        let mut bytes = text_bytes(&self.formula)
            .checked_add(runs.checked_mul(size_of::<IndexRun>())?)?
            .checked_add(exceptions.checked_mul(size_of::<SourceException>())?)?;
        for exception in &self.exceptions {
            for text in [&exception.value, &exception.loaded_display]
                .into_iter()
                .flatten()
            {
                bytes = bytes.checked_add(text_bytes(text))?;
            }
        }
        Some(bytes)
    }

    /// Every point index of the cache, in file order.
    pub fn indices(&self) -> impl Iterator<Item = u32> + '_ {
        let dense = if self.index_runs.is_empty() {
            0..self.point_count.unwrap_or(0)
        } else {
            0..0
        };
        dense.chain(self.index_runs.iter().copied().flat_map(IndexRun::indices))
    }

    /// The cache exactly as the file stored it: every point is an exception
    /// whose load display is not known yet. A workbook read reduces it with
    /// [`Self::reduce_borrowed`].
    pub(crate) fn as_read(
        formula: String,
        point_count: Option<u32>,
        points: Vec<(u32, Option<String>)>,
    ) -> Self {
        let index_runs = Self::runs(point_count, &points);
        Self {
            formula,
            point_count,
            index_runs,
            exceptions: points
                .into_iter()
                .map(|(index, value)| SourceException {
                    index,
                    value,
                    loaded_display: None,
                })
                .collect(),
        }
    }

    pub(crate) fn from_reduced(
        formula: String,
        point_count: Option<u32>,
        mut index_runs: Vec<IndexRun>,
        mut exceptions: Vec<SourceException>,
    ) -> Self {
        index_runs.shrink_to_fit();
        exceptions.shrink_to_fit();
        Self {
            formula,
            point_count,
            index_runs,
            exceptions,
        }
    }

    /// Point indices as runs; empty when they are exactly the dense
    /// `0..point_count`.
    fn runs(point_count: Option<u32>, points: &[(u32, Option<String>)]) -> Vec<IndexRun> {
        let counted_dense = point_count.map(|declared| declared as usize) == Some(points.len())
            && points
                .iter()
                .enumerate()
                .all(|(position, (index, _))| *index as usize == position);
        if counted_dense {
            return Vec::new();
        }
        let mut runs: Vec<IndexRun> = Vec::new();
        for (index, _) in points {
            match runs.last_mut() {
                Some(run)
                    if run.len < u32::MAX
                        && u64::from(run.start) + u64::from(run.len) == u64::from(*index) =>
                {
                    run.len += 1;
                }
                _ => runs.push(IndexRun {
                    start: *index,
                    len:   1,
                }),
            }
        }
        if runs.is_empty() && point_count.is_some_and(|declared| declared > 0) {
            runs.push(IndexRun::default());
        }
        runs
    }

    /// Reduces a cache as read to what the cells cannot reproduce, given what
    /// each point's cell displayed at load. `None` means the cache is dense,
    /// has `expected_points` points and every point equals its load display,
    /// so exporting from the cells reproduces it.
    #[cfg(test)]
    pub(crate) fn reduce(
        self,
        expected_points: Option<u64>,
        retain_shape: bool,
        mut display: impl FnMut(u32) -> Option<String>,
    ) -> Option<Self> {
        self.reduce_borrowed(expected_points, retain_shape, |index, scratch| {
            display(index).map(|text| {
                *scratch = text;
                CacheDisplay::Scratch
            })
        })
    }

    pub(crate) fn reduce_borrowed<'a>(
        self,
        expected_points: Option<u64>,
        retain_shape: bool,
        mut display: impl FnMut(u32, &mut String) -> Option<CacheDisplay<'a>>,
    ) -> Option<Self> {
        let mut scratch = String::new();
        let count = self.exceptions.len();
        let counted_dense = self.index_runs.is_empty()
            && self.point_count.map(|declared| declared as usize) == Some(count);

        // Strictly increasing indices cannot repeat. The usual dense cache
        // therefore needs no duplicate-detection allocation.
        let increasing = self
            .exceptions
            .windows(2)
            .all(|pair| pair[0].index < pair[1].index);
        let mut repeated = std::collections::HashSet::new();
        if !increasing {
            let mut seen = std::collections::HashSet::new();
            for point in &self.exceptions {
                if !seen.insert(point.index) {
                    repeated.insert(point.index);
                }
            }
        }
        let mut exceptions: Vec<SourceException> = self
            .exceptions
            .into_iter()
            .filter_map(|mut point| {
                let display = display(point.index, &mut scratch);
                if display.is_none()
                    || point.value.as_deref() != display.as_ref().map(|text| text.text(&scratch))
                    || (!repeated.is_empty() && repeated.contains(&point.index))
                {
                    point.loaded_display = display.map(|text| text.into_owned(&scratch));
                    Some(point)
                } else {
                    None
                }
            })
            .collect();

        if !retain_shape
            && counted_dense
            && expected_points == Some(count as u64)
            && exceptions.is_empty()
        {
            return None;
        }
        // Filtering may reuse the full parsed allocation. Retain only the
        // sparse records, including for a shape-only cache.
        exceptions.shrink_to_fit();
        let mut index_runs = self.index_runs;
        index_runs.shrink_to_fit();
        Some(Self {
            formula: self.formula,
            point_count: self.point_count,
            index_runs,
            exceptions,
        })
    }
}

impl IndexRun {
    /// The first index of the run.
    #[must_use]
    pub fn start(&self) -> u32 {
        self.start
    }

    /// How many consecutive indices the run covers.
    #[must_use]
    pub fn len(&self) -> u32 {
        self.len
    }

    /// Whether the run covers no index.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    fn indices(self) -> impl Iterator<Item = u32> {
        (u64::from(self.start)..u64::from(self.start) + u64::from(self.len))
            .filter_map(|index| u32::try_from(index).ok())
    }
}

impl SourceException {
    pub(crate) fn imported(
        index: u32,
        value: Option<String>,
        loaded_display: Option<String>,
    ) -> Self {
        Self {
            index,
            value,
            loaded_display,
        }
    }

    /// The point index.
    #[must_use]
    pub fn index(&self) -> u32 {
        self.index
    }

    /// The text the file stored, `None` when the point had no `c:v`.
    #[must_use]
    pub fn value(&self) -> Option<&str> {
        self.value.as_deref()
    }

    /// What the cell displayed when the file was loaded, `None` when no cell
    /// could be resolved.
    #[must_use]
    pub fn loaded_display(&self) -> Option<&str> {
        self.loaded_display.as_deref()
    }
}

/// Identity uses existing explicit values only. Presentation, group kind,
/// axis bindings and traversal order never participate in adoption.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum ReferenceIdentity {
    ChartTitle,
    Category(u32),
    SeriesText(u32),
    AxisTitle(u32),
    TrendlineLabel(u32, String),
}

// Define the traversal once for borrowed and mutable references. Each visit
// carries its identity, so capture and boundary operations cannot disagree
// about reference locations or maintain a parallel list of roles.
macro_rules! string_reference_traversal {
    ($name:ident, ($($mutable:tt)*), ($($borrow:lifetime)?),
     $chart:ident, $title:ident, $text:ident,
     $reference:ident, $plot:ident, $groups:ident, $series:ident, $items:ident,
     $category:ident, $trends:ident, $label:ident, $label_text:ident, [$($axes:ident),*]) => {
        pub(crate) fn $name $(<$borrow>)? (
            space: & $($borrow)? $($mutable)* ChartSpace,
            visit: &mut dyn FnMut(
                Option<ReferenceIdentity>,
                & $($borrow)? $($mutable)* StringReference,
            ),
        ) {
            let chart = space.$chart();
            if let Some(reference) = chart
                .$title()
                .and_then(|title| title.$text())
                .and_then(|text| text.$reference())
            {
                visit(Some(ReferenceIdentity::ChartTitle), reference);
            }

            let plot = chart.$plot();
            for group in plot.$groups() {
                for series in group.$series().$items() {
                    let identity = series.index().explicit_value();
                    if let Some(reference) = series
                        .$category()
                        .and_then(|data| data.$reference())
                    {
                        visit(identity.map(ReferenceIdentity::Category), reference);
                    }
                    if let Some(reference) = series
                        .$text()
                        .and_then(|text| text.$reference())
                    {
                        visit(identity.map(ReferenceIdentity::SeriesText), reference);
                    }
                }
            }

            $(for axis in plot.$axes() {
                let identity = axis.axis_id().explicit_value();
                if let Some(reference) = axis
                    .$title()
                    .and_then(|title| title.$text())
                    .and_then(|text| text.$reference())
                {
                    visit(identity.map(ReferenceIdentity::AxisTitle), reference);
                }
            })*

            // Existing roles keep their admission order and priority globally,
            // including later series and axis titles. New label roles follow
            // in their own document order through this same traversal owner.
            for group in plot.$groups() {
                for series in group.$series().$items() {
                    let identity = series.index().explicit_value();
                    for trendline in series.$trends() {
                        if let Some(reference) = trendline
                            .$label()
                            .and_then(|label| label.$label_text())
                            .and_then(|text| text.$reference())
                        {
                            let key = identity.map(|id| ReferenceIdentity::TrendlineLabel(
                                id, reference.formula().address_str(),
                            ));
                            visit(key,reference);
                        }
                    }
                }
            }
        }
    };
}

string_reference_traversal!(
    visit_string_references,
    (mut),
    (),
    chart_mut,
    title_mut,
    chart_text_mut,
    string_reference_mut,
    plot_area_mut,
    chart_groups_mut,
    series_mut,
    area_chart_series_mut,
    category_axis_data_mut,
    trendlines_iter_mut, label_mut, text_mut,
    [category_axis_mut, date_axis_mut, value_axis_mut, series_axis_mut]
);
string_reference_traversal!(
    visit_string_references_ref,
    (),
    ('a),
    chart,
    title,
    chart_text,
    string_reference,
    plot_area,
    chart_groups,
    series,
    area_chart_series,
    category_axis_data,
    trendlines, label, text,
    [category_axis, date_axis, value_axis, series_axis]
);

/// Removes every source cache of the chart.
pub(crate) fn clear_source_caches(space: &mut ChartSpace) {
    visit_string_references(space, &mut |_, reference| {
        reference.string_cache_mut().set_source(None);
    });
}

/// Count identities before considering formulas, guards or cache presence.
/// Missing identities and duplicates on either side cannot match. Only the
/// cache of a unique, unchanged, server-owned reference is cloned.
pub(crate) fn adopt_source_caches(space: &mut ChartSpace, previous: &ChartSpace) {
    let mut incoming_counts = HashMap::new();
    visit_string_references_ref(space, &mut |identity, _| {
        if let Some(identity) = identity {
            *incoming_counts.entry(identity).or_insert(0usize) += 1;
        }
    });
    let mut old = HashMap::new();
    visit_string_references_ref(previous, &mut |identity, reference| {
        if let Some(identity) = identity {
            let entry = old.entry(identity).or_insert((0usize, reference));
            entry.0 += 1;
        }
    });
    visit_string_references(space, &mut |identity, reference| {
        reference.string_cache_mut().set_source(None);
        let Some(identity) = identity else {
            return;
        };
        if incoming_counts.get(&identity) != Some(&1) {
            return;
        }
        let Some((1, previous)) = old.get(&identity) else {
            return;
        };
        let formula = previous.formula().address_str();
        if reference.formula().address_str() != formula {
            return;
        }
        let source = previous
            .string_cache()
            .source()
            .filter(|source| source.formula() == formula)
            .cloned();
        reference.string_cache_mut().set_source(source);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drawing::charts::StringCache;

    // Capacity accounting must include unused backing and both nested strings;
    // clone preflight must instead predict allocations from their lengths.
    #[test]
    fn retained_source_bytes_include_spare_capacity_and_nested_strings() {
        let (source, allocated) = crate::test_alloc::observe(|| {
            let mut formula = String::with_capacity(97);
            formula.push('f');
            let mut value = String::with_capacity(101);
            value.push('v');
            let mut display = String::with_capacity(103);
            display.push('d');
            let mut index_runs = Vec::with_capacity(4);
            index_runs.push(IndexRun { start: 0, len: 1 });
            let mut exceptions = Vec::with_capacity(3);
            exceptions.push(SourceException::imported(0, Some(value), Some(display)));
            SourceCache {
                formula,
                point_count: Some(1),
                index_runs,
                exceptions,
            }
        });
        let retained =
            97 + 4 * size_of::<IndexRun>() + 3 * size_of::<SourceException>() + 101 + 103;
        let cloned = 1 + size_of::<IndexRun>() + size_of::<SourceException>() + 1 + 1;
        assert_eq!(source.retained_bytes(), Some(retained));
        assert_eq!(allocated.live_bytes, retained as isize);
        assert_eq!(allocated.peak_bytes, retained);
        assert_eq!(source.cloned_retained_bytes(), Some(cloned));
        let (copy, allocated) = crate::test_alloc::observe(|| source.clone());
        assert_eq!(copy, source);
        assert_eq!(copy.retained_bytes(), Some(cloned));
        assert_eq!(allocated.live_bytes, cloned as isize);
        assert_eq!(allocated.peak_bytes, cloned);
    }

    #[test]
    fn sparse_storage_releases_dense_allocations() {
        assert_eq!(size_of::<StringCache>(), size_of::<usize>());
        let points = (0..100_000)
            .map(|index| {
                (
                    index,
                    Some(if index == 42 {
                        "source".into()
                    } else {
                        "1".into()
                    }),
                )
            })
            .collect();
        let reduced = SourceCache::as_read("Sheet1!A1:A100000".into(), Some(100_000), points)
            .reduce(Some(100_000), false, |_| Some("1".into()))
            .unwrap();
        assert_eq!(reduced.exceptions.len(), 1);
        assert!(reduced.exceptions.capacity() <= 2);
        assert_eq!(reduced.index_runs.capacity(), 0);
        let all = SourceCache::as_read(
            "Sheet1!A1:A100000".into(),
            Some(100_000),
            (0..100_000)
                .map(|index| (index, Some("1".into())))
                .collect(),
        );
        assert!(
            all.reduce(Some(100_000), false, |_| Some("1".into()))
                .is_none()
        );
        let shape = SourceCache::as_read(
            "Sheet1!A1:A100001".into(),
            Some(100_001),
            (0..100_001)
                .filter(|index| *index != 1)
                .map(|index| (index, Some("1".into())))
                .collect(),
        )
        .reduce(Some(100_001), false, |_| Some("1".into()))
        .unwrap();
        assert_eq!(shape.index_runs.len(), 2);
        assert!(shape.index_runs.capacity() <= 2);
        assert!(shape.exceptions.capacity() <= 2);
        eprintln!(
            "architecture={} pointer={} string_cache={} source_cache={} exception={} \
             runs_capacity={} exceptions_capacity={}",
            std::env::consts::ARCH,
            size_of::<usize>(),
            size_of::<StringCache>(),
            size_of::<SourceCache>(),
            size_of::<SourceException>(),
            reduced.index_runs.capacity(),
            reduced.exceptions.capacity(),
        );
        eprintln!(
            "chart={} chart_space={} two_cell_anchor={} series={} shape_runs_capacity={} \
             shape_exceptions_capacity={} retained_text_capacity={}",
            size_of::<crate::Chart>(),
            size_of::<ChartSpace>(),
            size_of::<crate::drawing::spreadsheet::TwoCellAnchor>(),
            size_of::<crate::drawing::charts::AreaChartSeries>(),
            shape.index_runs.capacity(),
            shape.exceptions.capacity(),
            reduced
                .exceptions
                .iter()
                .map(|point| {
                    point.value.as_ref().map_or(0, String::capacity)
                        + point.loaded_display.as_ref().map_or(0, String::capacity)
                })
                .sum::<usize>(),
        );
    }
}
