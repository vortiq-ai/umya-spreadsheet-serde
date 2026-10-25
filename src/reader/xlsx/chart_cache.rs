// Reduces newly materialized charts one cache at a time. Only references
// into still-raw sheets share temporary coordinate lookups across caches.
#[cfg(test)]
use std::cell::Cell as Flag;
use std::{
    cell::{
        Cell as StateCell,
        RefCell,
    },
    collections::{
        HashMap,
        HashSet,
    },
    ops::Range,
    rc::Rc,
};

use quick_xml::{
    Reader,
    events::Event,
};

#[cfg(test)]
use crate::structs::drawing::charts::note_work;
use crate::{
    helper::address::unescape_sheet_name,
    reader::driver::{
        attribute_u32,
        local_name,
    },
    structs::{
        Cell,
        Workbook,
        drawing::charts::{
            DenseSummary,
            IndexRun,
            IndexedRange,
            MAX_SOURCE_CACHE_POINTS,
            MAX_SOURCE_CACHE_TEXT_BYTES,
            PointScratch,
            SourceCache,
            SourceException,
            StringReference,
            literal_text,
            parse_point_count,
            read_point_into,
            visit_deferred_indices,
            visit_string_references,
        },
    },
};

type Coordinate = (u32, u32);

#[cfg(test)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct ChartPartBufferObservation {
    eager_live:                       usize,
    eager_peak:                       usize,
    eager_openings:                   usize,
    failed_reads_have_no_buffer:      bool,
    lazy_source_reads_have_no_buffer: bool,
    lazy_sheet_boundaries:            usize,
    lazy_pending_parts_are_exact:     bool,
}

#[cfg(test)]
thread_local! {
    static CHART_PART_BUFFER_OBSERVATION: RefCell<ChartPartBufferObservation> = RefCell::new(
        ChartPartBufferObservation {
            failed_reads_have_no_buffer: true,
            lazy_source_reads_have_no_buffer: true,
            lazy_pending_parts_are_exact: true,
            ..ChartPartBufferObservation::default()
        },
    );
}

#[cfg(test)]
fn reset_chart_part_buffer_observation() {
    CHART_PART_BUFFER_OBSERVATION.with(|observation| {
        *observation.borrow_mut() = ChartPartBufferObservation {
            failed_reads_have_no_buffer: true,
            lazy_source_reads_have_no_buffer: true,
            lazy_pending_parts_are_exact: true,
            ..ChartPartBufferObservation::default()
        };
    });
}

#[cfg(test)]
fn chart_part_buffer_observation() -> ChartPartBufferObservation {
    CHART_PART_BUFFER_OBSERVATION.with(|observation| *observation.borrow())
}

#[cfg(test)]
fn observe_eager_part_acquire(data: &PartBuffer, read_succeeded: bool) {
    CHART_PART_BUFFER_OBSERVATION.with(|observation| {
        let mut observation = observation.borrow_mut();
        observation.eager_openings += 1;
        if !read_succeeded {
            observation.failed_reads_have_no_buffer &= data.data.is_empty();
        }
    });
}

/// One eager chart-part allocation. Test builds count it live from its
/// creation until the owner is destroyed, wherever the owner has been moved.
struct PartBuffer {
    data:    Vec<u8>,
    #[cfg(test)]
    counted: bool,
}

impl PartBuffer {
    fn new(data: Vec<u8>) -> Self {
        #[cfg(test)]
        let counted = data.capacity() != 0;
        #[cfg(test)]
        CHART_PART_BUFFER_OBSERVATION.with(|observation| {
            let mut observation = observation.borrow_mut();
            observation.eager_live += usize::from(counted);
            observation.eager_peak = observation.eager_peak.max(observation.eager_live);
        });
        Self {
            data,
            #[cfg(test)]
            counted,
        }
    }
}

#[cfg(test)]
impl Drop for PartBuffer {
    fn drop(&mut self) {
        let counted = usize::from(self.counted);
        let _ = CHART_PART_BUFFER_OBSERVATION.try_with(|observation| {
            let mut observation = observation.borrow_mut();
            observation.eager_live = observation.eager_live.saturating_sub(counted);
        });
    }
}

#[cfg(test)]
fn observe_eager_raw_parts(
    pending: &HashMap<String, (Vec<u8>, Option<std::path::PathBuf>)>,
) -> usize {
    let live = pending
        .values()
        .filter(|(data, _)| data.capacity() != 0)
        .count();
    CHART_PART_BUFFER_OBSERVATION.with(|observation| {
        let mut observation = observation.borrow_mut();
        observation.eager_live += live;
        observation.eager_peak = observation.eager_peak.max(observation.eager_live);
    });
    live
}

#[cfg(test)]
fn observe_eager_raw_release(released: usize) {
    CHART_PART_BUFFER_OBSERVATION.with(|observation| {
        observation.borrow_mut().eager_live -= released;
    });
}

#[cfg(test)]
fn observe_lazy_source_read(data: &[u8]) {
    CHART_PART_BUFFER_OBSERVATION.with(|observation| {
        observation.borrow_mut().lazy_source_reads_have_no_buffer &= data.is_empty();
    });
}

#[cfg(test)]
fn observe_lazy_sheet_boundary(provider: &LazyChartPartProvider, session: &SourceCacheSession) {
    CHART_PART_BUFFER_OBSERVATION.with(|observation| {
        let mut observation = observation.borrow_mut();
        observation.lazy_sheet_boundaries += 1;
        observation.lazy_pending_parts_are_exact &= provider.expected_part.is_none_or(|part| {
            provider.current.is_none()
                && provider.last_released == Some(part)
                && session
                    .parts
                    .borrow()
                    .get(part)
                    .and_then(|part| part.data.as_ref())
                    .is_some_and(|data| {
                        let _: &Box<[u8]> = data;
                        true
                    })
        });
    });
}

struct DeferredSource {
    part:    usize,
    start:   u64,
    end:     u64,
    formula: String,
    /// What reading the cache proved about its shape, when it proved density.
    summary: Option<DenseSummary>,
}

struct ChartPart {
    target: String,
    data:   Option<Box<[u8]>>,
}

pub(crate) trait ChartPartProvider {
    fn add_pending(&mut self, pending: HashMap<String, (Vec<u8>, Option<std::path::PathBuf>)>);

    fn with_part(
        &mut self,
        target: &str,
        session: &SourceCacheSession,
        visit: &mut dyn FnMut(Option<&[u8]>),
    );

    fn finish_sheet(&mut self, session: &SourceCacheSession);
}

pub(crate) struct LazyChartPartProvider {
    pending:       HashMap<String, (Vec<u8>, Option<std::path::PathBuf>)>,
    current:       Option<(usize, Vec<u8>)>,
    #[cfg(test)]
    last_released: Option<usize>,
    #[cfg(test)]
    expected_part: Option<usize>,
}

impl LazyChartPartProvider {
    pub(crate) fn new(pending: HashMap<String, (Vec<u8>, Option<std::path::PathBuf>)>) -> Self {
        Self {
            pending,
            current: None,
            #[cfg(test)]
            last_released: None,
            #[cfg(test)]
            expected_part: None,
        }
    }

    fn release_current(&mut self, session: &SourceCacheSession) {
        let Some((part, data)) = self.current.take() else {
            return;
        };
        let exact = data.into_boxed_slice();
        session.store_part(part, exact);
        #[cfg(test)]
        {
            self.last_released = Some(part);
        }
    }
}

impl ChartPartProvider for LazyChartPartProvider {
    fn add_pending(&mut self, pending: HashMap<String, (Vec<u8>, Option<std::path::PathBuf>)>) {
        self.pending.extend(pending);
        #[cfg(test)]
        {
            self.expected_part = None;
        }
    }

    fn with_part(
        &mut self,
        target: &str,
        session: &SourceCacheSession,
        visit: &mut dyn FnMut(Option<&[u8]>),
    ) {
        let part = session.part_for(target);
        #[cfg(test)]
        {
            self.expected_part = Some(part);
        }
        if self.current.as_ref().map(|(current, _)| *current) != Some(part) {
            self.release_current(session);
            let data = session.take_part(part).map_or_else(
                || {
                    let (data, source_file) = self.pending.remove(target).unwrap_or_default();
                    if data.is_empty() {
                        let data = crate::structs::raw::RawFile::read_from_source_or_empty(
                            target,
                            source_file.as_deref(),
                        );
                        #[cfg(test)]
                        if source_file.is_some() {
                            observe_lazy_source_read(&data);
                        }
                        data
                    } else {
                        data
                    }
                },
                Vec::from,
            );
            self.current = Some((part, data));
        }
        visit(Some(&self.current.as_ref().unwrap().1));
    }

    fn finish_sheet(&mut self, session: &SourceCacheSession) {
        self.release_current(session);
        #[cfg(test)]
        observe_lazy_sheet_boundary(self, session);
    }
}

pub(crate) struct EagerChartPartProvider<'a, R> {
    archive: &'a mut zip::read::ZipArchive<R>,
    current: Option<(String, PartBuffer, bool)>,
}

impl<'a, R: std::io::Read + std::io::Seek> EagerChartPartProvider<'a, R> {
    pub(crate) fn new(archive: &'a mut zip::read::ZipArchive<R>) -> Self {
        Self {
            archive,
            current: None,
        }
    }

    fn release_current(&mut self) {
        self.current = None;
    }

    fn read_part(&mut self, target: &str) {
        self.release_current();
        let mut data = Vec::new();
        let Ok(entry) = crate::reader::driver::zip_by_name(self.archive, target) else {
            self.current = Some((target.into(), PartBuffer::new(data), false));
            #[cfg(test)]
            observe_eager_part_acquire(&self.current.as_ref().unwrap().1, false);
            return;
        };
        let reservation =
            usize::try_from(entry.size().min(isize::MAX as u64)).unwrap_or(usize::MAX);
        let _ = data.try_reserve_exact(reservation);
        let mut entry = std::io::BufReader::new(entry);
        let succeeded = std::io::Read::read_to_end(&mut entry, &mut data).is_ok();
        if !succeeded {
            data.clear();
        }
        self.current = Some((target.into(), PartBuffer::new(data), succeeded));
        #[cfg(test)]
        {
            let (_, data, succeeded) = self.current.as_ref().unwrap();
            observe_eager_part_acquire(data, *succeeded);
        }
    }
}

impl<R: std::io::Read + std::io::Seek> ChartPartProvider for EagerChartPartProvider<'_, R> {
    fn add_pending(&mut self, pending: HashMap<String, (Vec<u8>, Option<std::path::PathBuf>)>) {
        #[cfg(test)]
        let released = observe_eager_raw_parts(&pending);
        drop(pending);
        #[cfg(test)]
        observe_eager_raw_release(released);
    }

    fn with_part(
        &mut self,
        target: &str,
        _session: &SourceCacheSession,
        visit: &mut dyn FnMut(Option<&[u8]>),
    ) {
        if self.current.as_ref().map(|(current, ..)| current.as_str()) != Some(target) {
            self.read_part(target);
        }
        let (_, data, succeeded) = self.current.as_ref().unwrap();
        visit(succeeded.then_some(data.data.as_slice()));
    }

    fn finish_sheet(&mut self, _session: &SourceCacheSession) {
        self.release_current();
    }
}

impl DeferredSource {
    fn range(&self) -> Option<Range<usize>> {
        Some(usize::try_from(self.start).ok()?..usize::try_from(self.end).ok()?)
    }
}

enum CapturedSource {
    Deferred(DeferredSource),
    Parsed(SourceCache),
}

enum DeferredReduction {
    Reduced,
    Candidate {
        source:            Option<SourceCache>,
        points:            usize,
        text_bytes:        usize,
        #[cfg(test)]
        rejection_backing: RejectionBacking,
    },
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct CandidateBacking {
    index_runs: Option<(usize, usize)>,
    exceptions: Option<(usize, usize)>,
}

#[cfg(test)]
impl CandidateBacking {
    fn capture(
        index_runs: Option<&Vec<IndexRun>>,
        exceptions: Option<&Vec<SourceException>>,
    ) -> Self {
        Self {
            index_runs: index_runs.map(|runs| (runs.len(), runs.capacity())),
            exceptions: exceptions.map(|values| (values.len(), values.capacity())),
        }
    }
}

#[cfg(test)]
#[derive(Clone, Copy, Default)]
struct RejectionBacking {
    shape:     Option<CandidateBacking>,
    exception: Option<CandidateBacking>,
}

#[cfg(test)]
impl DeferredReduction {
    fn rejection_backing(&self) -> RejectionBacking {
        match self {
            Self::Reduced => RejectionBacking::default(),
            Self::Candidate {
                rejection_backing, ..
            } => *rejection_backing,
        }
    }
}

struct CacheShape {
    point_count:     Option<u32>,
    index_runs:      Option<Vec<IndexRun>>,
    run_charge:      usize,
    current_run_len: u32,
    counted_dense:   bool,
    repeated:        HashSet<u32>,
    #[cfg(test)]
    shape_backing:   Option<CandidateBacking>,
}

impl CacheShape {
    // Only the first rejection is the release point; a later over-budget point
    // must not hide a delayed release.
    #[cfg(test)]
    fn observe_shape_rejection(&mut self) {
        if self.shape_backing.is_none() {
            self.shape_backing = Some(CandidateBacking::capture(self.index_runs.as_ref(), None));
        }
    }

    fn record_index(&mut self, index: u32, count: &mut u64, previous: &mut Option<u32>) {
        if u64::from(index) != *count {
            self.counted_dense = false;
        }
        *count += 1;
        let continues = previous.is_some_and(|prior| u64::from(prior) + 1 == u64::from(index))
            && self.current_run_len < u32::MAX;
        if continues {
            self.current_run_len += 1;
        } else {
            self.run_charge = self.run_charge.saturating_add(1);
            self.current_run_len = 1;
        }
        *previous = Some(index);
        if let Some(runs) = &mut self.index_runs {
            if continues {
                if let Some(run) = runs.last_mut() {
                    if run.len < u32::MAX {
                        run.len += 1;
                        return;
                    }
                }
            }
            runs.push(IndexRun {
                start: index,
                len:   1,
            });
        }
    }
}

/// The shape of a cache whose reading proved positive counted density. It is
/// what `deferred_shape` finds for that cache, without scanning it.
fn dense_shape(
    summary: DenseSummary,
    #[cfg_attr(not(test), allow(unused_variables))] remaining_points: usize,
) -> CacheShape {
    CacheShape {
        point_count:                Some(summary.point_count),
        index_runs:                 Some(Vec::new()),
        run_charge:                 0,
        current_run_len:            0,
        counted_dense:              true,
        repeated:                   HashSet::new(),
        // The scan charges the first run before it recognizes density, so a
        // cache that cannot afford one records the rejection first.
        #[cfg(test)]
        shape_backing:              (remaining_points == 0).then_some(CandidateBacking {
            index_runs: None,
            exceptions: None,
        }),
    }
}

fn deferred_shape(data: &[u8], remaining_points: usize) -> CacheShape {
    #[cfg(test)]
    note_work(|counts| counts.shape_scans += 1);
    let mut shape = CacheShape {
        point_count:                None,
        index_runs:                 Some(Vec::new()),
        run_charge:                 0,
        current_run_len:            0,
        counted_dense:              true,
        repeated:                   HashSet::new(),
        #[cfg(test)]
        shape_backing:              None,
    };
    let mut count = 0u64;
    let mut ascending = true;
    let mut previous = None;
    let mut reader = Reader::from_reader(std::io::Cursor::new(data));
    reader.config_mut().trim_text(true);
    let mut buffer = Vec::new();
    let mut skipped = Vec::new();
    loop {
        buffer.clear();
        match reader.read_event_into(&mut buffer).unwrap() {
            Event::Start(e) => match local_name(e.name().into_inner()) {
                b"pt" => {
                    if let Some(index) = attribute_u32(&e, b"idx") {
                        ascending &= previous.is_none_or(|prior| prior < index);
                        shape.record_index(index, &mut count, &mut previous);
                        if shape.run_charge > remaining_points {
                            shape.index_runs = None;
                            #[cfg(test)]
                            shape.observe_shape_rejection();
                        }
                    }
                    skipped.clear();
                    reader.read_to_end_into(e.name(), &mut skipped).unwrap();
                }
                b"ptCount" => shape.point_count = parse_point_count(&e),
                _ => (),
            },
            Event::Empty(e) => match local_name(e.name().into_inner()) {
                b"pt" => {
                    if let Some(index) = attribute_u32(&e, b"idx") {
                        ascending &= previous.is_none_or(|prior| prior < index);
                        shape.record_index(index, &mut count, &mut previous);
                        if shape.run_charge > remaining_points {
                            shape.index_runs = None;
                            #[cfg(test)]
                            shape.observe_shape_rejection();
                        }
                    }
                }
                b"ptCount" => shape.point_count = parse_point_count(&e),
                b"strCache" => break,
                _ => (),
            },
            Event::End(e) if local_name(e.name().into_inner()) == b"strCache" => break,
            Event::Eof => panic!("Error: Could not find {} end element", "c:strCache"),
            _ => (),
        }
    }
    let counted_dense = shape.counted_dense && shape.point_count.map(u64::from) == Some(count);
    shape.counted_dense = counted_dense;
    if counted_dense {
        shape.index_runs = Some(Vec::new());
        shape.run_charge = 0;
    } else if count == 0 && shape.point_count.is_some_and(|count| count > 0) {
        shape.run_charge = 1;
        if shape.run_charge > remaining_points {
            shape.index_runs = None;
            #[cfg(test)]
            shape.observe_shape_rejection();
        } else {
            shape.index_runs = Some(vec![IndexRun::default()]);
        }
    }
    if !ascending {
        let mut seen = HashSet::new();
        visit_deferred_indices(data, |index| {
            if !seen.insert(index) {
                shape.repeated.insert(index);
            }
        });
    }
    shape
}

fn reduce_deferred_cache(
    data: &[u8],
    source: &DeferredSource,
    expected_points: Option<u64>,
    retain_shape: bool,
    remaining_points: usize,
    remaining_text_bytes: usize,
    mut display: impl FnMut(u32) -> Option<String>,
) -> DeferredReduction {
    let mut shape = match source.summary {
        Some(summary) => dense_shape(summary, remaining_points),
        None => deferred_shape(data, remaining_points),
    };
    let mut points = shape.run_charge;
    let mut text_bytes: usize = 0;
    let mut counting = shape.index_runs.is_none();
    let mut exceptions = Vec::new();
    #[cfg(test)]
    let mut exception_rejection_backing = None;
    #[cfg(test)]
    note_work(|counts| counts.decode_scans += 1);
    let mut reader = Reader::from_reader(std::io::Cursor::new(data));
    reader.config_mut().trim_text(true);
    let mut buffer = Vec::new();
    let mut scratch = PointScratch::default();
    loop {
        buffer.clear();
        let event = reader.read_event_into(&mut buffer).unwrap();
        let point = match event {
            Event::Start(e) if local_name(e.name().into_inner()) == b"pt" => {
                let index = attribute_u32(&e, b"idx");
                let value = read_point_into(&mut reader, &mut scratch, true).value;
                index.map(|index| (index, value))
            }
            Event::Empty(e) if local_name(e.name().into_inner()) == b"pt" => {
                attribute_u32(&e, b"idx").map(|index| (index, None))
            }
            Event::Empty(e) if local_name(e.name().into_inner()) == b"strCache" => break,
            Event::End(e) if local_name(e.name().into_inner()) == b"strCache" => break,
            Event::Eof => panic!("Error: Could not find {} end element", "c:strCache"),
            _ => None,
        };
        let Some((index, value)) = point else {
            continue;
        };
        let loaded_display = display(index);
        if loaded_display.is_none() || value != loaded_display || shape.repeated.contains(&index) {
            points = points.saturating_add(1);
            text_bytes = text_bytes
                .saturating_add(value.as_ref().map_or(0, String::len))
                .saturating_add(loaded_display.as_ref().map_or(0, String::len));
            if !counting && (points > remaining_points || text_bytes > remaining_text_bytes) {
                exceptions = Vec::new();
                shape.index_runs = None;
                #[cfg(test)]
                {
                    exception_rejection_backing = Some(CandidateBacking::capture(
                        shape.index_runs.as_ref(),
                        Some(&exceptions),
                    ));
                }
                counting = true;
            }
            if !counting {
                exceptions.push(SourceException::imported(index, value, loaded_display));
            }
        }
    }
    if !retain_shape
        && shape.counted_dense
        && expected_points == shape.point_count.map(u64::from)
        && points == 0
    {
        return DeferredReduction::Reduced;
    }
    DeferredReduction::Candidate {
        source: (!counting).then(|| {
            SourceCache::from_reduced(
                source.formula.clone(),
                shape.point_count,
                shape.index_runs.unwrap_or_default(),
                exceptions,
            )
        }),
        points,
        text_bytes,
        #[cfg(test)]
        rejection_backing: RejectionBacking {
            shape:     shape.shape_backing,
            exception: exception_rejection_backing,
        },
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SourceCacheSessionMode {
    Deferred,
    Immediate,
}

/// Owns imported cache payloads until the workbook has a stable traversal
/// order for admission. References retain guarded placeholders meanwhile.
#[derive(Clone)]
pub(crate) struct SourceCacheSession {
    mode:                         SourceCacheSessionMode,
    descriptors:                  Rc<RefCell<HashMap<usize, DeferredSource>>>,
    parts:                        Rc<RefCell<Vec<ChartPart>>>,
    current_part:                 Rc<StateCell<Option<usize>>>,
    #[cfg(test)]
    registered_keys:              Rc<RefCell<Vec<usize>>>,
    #[cfg(test)]
    drop_next_capture_descriptor: Rc<Flag<bool>>,
    #[cfg(test)]
    capture_formulas:             Rc<RefCell<Vec<String>>>,
    #[cfg(test)]
    wanted_coordinates:           Rc<RefCell<Option<HashMap<usize, HashSet<Coordinate>>>>>,
    #[cfg(test)]
    rejection_backing:            Rc<RefCell<RejectionBacking>>,
}

impl SourceCacheSession {
    pub(crate) fn deferred() -> Self {
        Self {
            mode:                                      SourceCacheSessionMode::Deferred,
            descriptors:                               Rc::new(RefCell::new(HashMap::new())),
            parts:                                     Rc::new(RefCell::new(Vec::new())),
            current_part:                              Rc::new(StateCell::new(None)),
            #[cfg(test)]
            registered_keys:                           Rc::new(RefCell::new(Vec::new())),
            #[cfg(test)]
            drop_next_capture_descriptor:              Rc::new(Flag::new(false)),
            #[cfg(test)]
            capture_formulas:                          Rc::new(RefCell::new(Vec::new())),
            #[cfg(test)]
            wanted_coordinates:                        Rc::new(RefCell::new(None)),
            #[cfg(test)]
            rejection_backing:                         Rc::new(RefCell::new(
                RejectionBacking::default(),
            )),
        }
    }

    #[cfg(test)]
    pub(crate) fn immediate() -> Self {
        Self {
            mode:                                      SourceCacheSessionMode::Immediate,
            descriptors:                               Rc::new(RefCell::new(HashMap::new())),
            parts:                                     Rc::new(RefCell::new(Vec::new())),
            current_part:                              Rc::new(StateCell::new(None)),
            #[cfg(test)]
            registered_keys:                           Rc::new(RefCell::new(Vec::new())),
            #[cfg(test)]
            drop_next_capture_descriptor:              Rc::new(Flag::new(false)),
            #[cfg(test)]
            capture_formulas:                          Rc::new(RefCell::new(Vec::new())),
            #[cfg(test)]
            wanted_coordinates:                        Rc::new(RefCell::new(None)),
            #[cfg(test)]
            rejection_backing:                         Rc::new(RefCell::new(
                RejectionBacking::default(),
            )),
        }
    }

    pub(crate) fn immediate_subsession(&self) -> Self {
        Self {
            mode:                                      SourceCacheSessionMode::Immediate,
            descriptors:                               Rc::clone(&self.descriptors),
            parts:                                     Rc::clone(&self.parts),
            current_part:                              Rc::clone(&self.current_part),
            #[cfg(test)]
            registered_keys:                           Rc::clone(&self.registered_keys),
            #[cfg(test)]
            drop_next_capture_descriptor:              Rc::clone(
                &self.drop_next_capture_descriptor,
            ),
            #[cfg(test)]
            capture_formulas:                          Rc::clone(&self.capture_formulas),
            #[cfg(test)]
            wanted_coordinates:                        Rc::clone(&self.wanted_coordinates),
            #[cfg(test)]
            rejection_backing:                         Rc::clone(&self.rejection_backing),
        }
    }

    pub(crate) fn is_deferred(&self) -> bool {
        self.mode == SourceCacheSessionMode::Deferred
    }

    pub(crate) fn begin_chart_part(&self, target: &str) {
        if !self.is_deferred() {
            return;
        }
        self.current_part.set(Some(self.part_for(target)));
    }

    pub(crate) fn end_chart_part(&self) {
        self.current_part.set(None);
    }

    fn part_for(&self, target: &str) -> usize {
        let mut parts = self.parts.borrow_mut();
        if let Some(part) = parts.iter().position(|part| part.target == target) {
            return part;
        }
        parts.push(ChartPart {
            target: target.into(),
            data:   None,
        });
        parts.len() - 1
    }

    fn part_target(&self, part: usize) -> Option<String> {
        self.parts
            .borrow()
            .get(part)
            .map(|part| part.target.clone())
    }

    fn store_part(&self, part: usize, data: Box<[u8]>) {
        if let Some(part) = self.parts.borrow_mut().get_mut(part) {
            part.data = Some(data);
        }
    }

    fn take_part(&self, part: usize) -> Option<Box<[u8]>> {
        self.parts.borrow_mut().get_mut(part)?.data.take()
    }

    fn register_parsed_source(
        &self,
        cache: &mut crate::drawing::charts::StringCache,
        span: Option<(u64, u64)>,
        summary: Option<DenseSummary>,
    ) {
        let Some(key) = cache.source_key() else {
            return;
        };
        #[cfg(test)]
        self.registered_keys.borrow_mut().push(key);
        self.clear_allocated_source(cache);
        let mut descriptors = self.descriptors.borrow_mut();
        if self.mode == SourceCacheSessionMode::Deferred {
            let formula = cache
                .source()
                .map_or_else(String::new, |source| source.formula().into());
            let mut source = cache.replace_source_value(SourceCache::as_read(
                format!("{formula}\0"),
                None,
                Vec::new(),
            ));
            if let Some(source) = source.as_mut() {
                source.set_formula(formula.clone());
            }
            if source.is_some() {
                let Some(part) = self.current_part.get() else {
                    debug_assert!(false, "missing deferred chart part");
                    cache.set_source(None);
                    return;
                };
                let Some((start, end)) = span else {
                    debug_assert!(false, "missing deferred chart cache span");
                    cache.set_source(None);
                    return;
                };
                descriptors.insert(
                    key,
                    DeferredSource {
                        part,
                        start,
                        end,
                        formula,
                        summary,
                    },
                );
            }
        }
    }

    fn clear_allocated_source(&self, cache: &crate::drawing::charts::StringCache) {
        if let Some(key) = cache.source_key() {
            self.descriptors.borrow_mut().remove(&key);
        }
    }

    fn finalize_parsed_source(
        &self,
        cache: &mut crate::drawing::charts::StringCache,
        formula: String,
    ) {
        if self.mode == SourceCacheSessionMode::Immediate {
            cache.finalize_source_formula(formula);
            if let Some(key) = cache.source_key() {
                self.descriptors.borrow_mut().remove(&key);
            }
            return;
        }
        let Some(key) = cache.source_key() else {
            return;
        };
        let mut descriptors = self.descriptors.borrow_mut();
        let Some(source) = descriptors.get_mut(&key) else {
            debug_assert!(false, "missing deferred chart source cache descriptor");
            cache.set_source(None);
            return;
        };
        cache.finalize_source_formula(format!("{formula}\0"));
        source.formula = formula;
    }

    /// Restore the uncaptured image frames before the drawing becomes visible
    /// to materialization. They intentionally retain the parser's source.
    pub(crate) fn hydrate_image_frames(
        &self,
        drawing: &mut crate::drawing::spreadsheet::WorksheetDrawing,
        provider: &mut dyn ChartPartProvider,
    ) {
        #[cfg(test)]
        let _observation = crate::drawing::charts::DeferredHydrationSourceBoxObservation::begin();
        visit_image_frames(drawing, &mut |reference| {
            if reference.string_cache().source().is_none() {
                return;
            }
            if let Some(source) = self.take(reference) {
                if let Some(source) = self.materialize_with_provider(&source, provider) {
                    self.replace(reference, source);
                } else {
                    self.clear(reference);
                }
            } else {
                debug_assert!(
                    false,
                    "missing deferred image chart source cache descriptor"
                );
                self.clear(reference);
            }
        });
    }

    fn take(&self, reference: &mut StringReference) -> Option<DeferredSource> {
        let key = reference.string_cache().source_key()?;
        self.descriptors.borrow_mut().remove(&key)
    }

    #[cfg(test)]
    fn materialize(&self, source: DeferredSource) -> SourceCache {
        let parts = self.parts.borrow();
        let data = parts
            .get(source.part)
            .and_then(|part| part.data.as_deref())
            .and_then(|part| source.range().and_then(|range| part.get(range)))
            .unwrap_or_default();
        crate::drawing::charts::materialize_deferred_cache(data, source.formula)
    }

    fn materialize_with_provider(
        &self,
        source: &DeferredSource,
        provider: &mut dyn ChartPartProvider,
    ) -> Option<SourceCache> {
        let mut materialized = None;
        if let Some(target) = self.part_target(source.part) {
            provider.with_part(&target, self, &mut |part| {
                let Some(part) = part else {
                    return;
                };
                let data = source.range().and_then(|range| part.get(range));
                let Some(data) = data else {
                    return;
                };
                materialized = Some(crate::drawing::charts::materialize_deferred_cache(
                    data,
                    source.formula.clone(),
                ));
            });
        }
        materialized
    }

    fn with_deferred_indices(
        &self,
        reference: &StringReference,
        mut visit: impl FnMut(u32),
    ) -> Option<()> {
        let key = reference.string_cache().source_key()?;
        let descriptors = self.descriptors.borrow();
        let source = descriptors.get(&key)?;
        let parts = self.parts.borrow();
        let data = parts
            .get(source.part)?
            .data
            .as_deref()?
            .get(source.range()?)?;
        visit_deferred_indices(data, &mut visit);
        Some(())
    }

    fn clear(&self, reference: &mut StringReference) {
        if let Some(key) = reference.string_cache().source_key() {
            self.descriptors.borrow_mut().remove(&key);
        }
        reference.string_cache_mut().set_source(None);
    }

    fn replace(&self, reference: &mut StringReference, source: SourceCache) {
        reference.string_cache_mut().replace_or_set_source(source);
        self.clear_allocated_source(reference.string_cache());
    }

    fn take_for_capture(&self, reference: &mut StringReference) -> Option<CapturedSource> {
        match self.mode {
            SourceCacheSessionMode::Immediate => {
                if let Some(key) = reference.string_cache().source_key() {
                    self.descriptors.borrow_mut().remove(&key);
                }
                reference
                    .string_cache_mut()
                    .take_source()
                    .map(CapturedSource::Parsed)
            }
            SourceCacheSessionMode::Deferred => {
                if !reference.string_cache().source_is_deferred() {
                    return None;
                }
                let source = self.take(reference);
                if source.is_none() {
                    debug_assert!(false, "missing deferred chart source cache descriptor");
                }
                self.clear(reference);
                source.map(CapturedSource::Deferred)
            }
        }
    }

    #[cfg(test)]
    fn last_registered_key(&self) -> Option<usize> {
        self.registered_keys.borrow().last().copied()
    }

    #[cfg(test)]
    fn arm_descriptor_loss(&self) {
        self.drop_next_capture_descriptor.set(true);
    }

    #[cfg(test)]
    fn descriptor_loss_was_consumed(&self) -> bool {
        !self.drop_next_capture_descriptor.get()
    }

    #[cfg(test)]
    fn capture_formulas(&self) -> Vec<String> {
        self.capture_formulas.borrow().clone()
    }

    #[cfg(test)]
    fn wanted_coordinates(&self) -> Option<HashMap<usize, HashSet<Coordinate>>> {
        self.wanted_coordinates.borrow().clone()
    }

    #[cfg(test)]
    fn rejection_backing(&self) -> RejectionBacking {
        *self.rejection_backing.borrow()
    }
}

pub(crate) fn register_parsed_string_cache(
    session: &SourceCacheSession,
    cache: &mut crate::drawing::charts::StringCache,
    span: Option<(u64, u64)>,
    summary: Option<DenseSummary>,
) {
    session.register_parsed_source(cache, span, summary);
}

pub(crate) fn finalize_parsed_string_cache(
    session: &SourceCacheSession,
    cache: &mut crate::drawing::charts::StringCache,
    formula: String,
) {
    session.finalize_parsed_source(cache, formula);
}

/// Capture only sheets transitioning from raw in this call. Older materialized
/// sheets may already contain edits and cannot supply an import baseline.
pub(crate) fn capture_chart_source_caches(
    book: &mut Workbook,
    fresh: &[usize],
    session: &mut SourceCacheSession,
    provider: &mut dyn ChartPartProvider,
) {
    if fresh.is_empty() {
        return;
    }
    let names: HashMap<String, usize> = book
        .sheet_collection_no_check()
        .iter()
        .enumerate()
        .map(|(index, sheet)| (sheet.name().to_string(), index))
        .collect();
    let raw: HashSet<usize> = book
        .sheet_collection_no_check()
        .iter()
        .enumerate()
        .filter(|(_, sheet)| !sheet.is_deserialized())
        .map(|(index, _)| index)
        .collect();
    let fresh_set: HashSet<usize> = fresh.iter().copied().collect();
    let mut wanted: HashMap<usize, HashSet<Coordinate>> = HashMap::new();
    if !raw.is_empty() {
        for index in fresh {
            visit_drawing(
                book.sheet_collection_unchecked_mut()[*index].worksheet_drawing_mut(),
                &mut |reference| {
                    let request = Request::new(reference, &names);
                    if let Some(index) = request.sheet.filter(|index| raw.contains(index)) {
                        let mut add_point = |point| {
                            if let Some(coordinate) = request
                                .range
                                .as_ref()
                                .and_then(|range| range.coordinate(point))
                            {
                                wanted.entry(index).or_default().insert(coordinate);
                            }
                        };
                        if session
                            .with_deferred_indices(reference, &mut add_point)
                            .is_none()
                        {
                            if let Some(source) = reference.string_cache().source() {
                                source.indices().for_each(add_point);
                            }
                        }
                    }
                },
            );
        }
    }
    #[cfg(test)]
    {
        *session.wanted_coordinates.borrow_mut() = Some(wanted.clone());
    }
    let mut streamed: HashMap<usize, HashMap<Coordinate, String>> = HashMap::new();
    if !wanted.is_empty() {
        let strings = book.shared_string_table();
        let strings = strings.read().unwrap();
        for (index, coordinates) in wanted {
            let mut values = HashMap::new();
            let read = super::worksheet::read_cells_stream(
                book.sheet_collection_no_check()[index].raw_data_of_worksheet(),
                &strings,
                book.stylesheet(),
                |cell: &Cell| {
                    let coordinate = (cell.coordinate().col_num(), cell.coordinate().row_num());
                    if coordinates.contains(&coordinate) {
                        values.insert(coordinate, cell.formatted_value());
                    }
                },
            );
            if read.is_ok() {
                streamed.insert(index, values);
            }
        }
    }

    let mut report = *book.chart_source_cache_report();
    for index in fresh {
        // Move only the drawing while resolving cells through the workbook's
        // indexed storage; there are no copied cell or cache display arrays.
        let mut drawing =
            std::mem::take(book.sheet_collection_unchecked_mut()[*index].worksheet_drawing_mut());
        visit_drawing(&mut drawing, &mut |reference| {
            let request = Request::new(reference, &names);
            #[cfg(test)]
            if session.drop_next_capture_descriptor.replace(false) {
                if let Some(key) = reference.string_cache().source_key() {
                    session.descriptors.borrow_mut().remove(&key);
                }
            }
            #[cfg(test)]
            session
                .capture_formulas
                .borrow_mut()
                .push(reference.formula().address_str());
            let source = session.take_for_capture(reference);
            let Some(source) = source else {
                return;
            };
            let earlier = request
                .sheet
                .is_some_and(|index| !raw.contains(&index) && !fresh_set.contains(&index));
            if earlier {
                report.reduced_caches += 1;
                report.dropped_caches += 1;
                return;
            }
            let retain_shape = request.sheet.is_some_and(|index| raw.contains(&index));
            let remaining_points = MAX_SOURCE_CACHE_POINTS.saturating_sub(report.kept_points);
            let remaining_text = MAX_SOURCE_CACHE_TEXT_BYTES.saturating_sub(report.kept_text_bytes);
            let reduced = match source {
                CapturedSource::Parsed(source) => {
                    match source.reduce(request.expected_points(), retain_shape, |point| {
                        request.display(book, &raw, &streamed, point)
                    }) {
                        Some(cache) => {
                            let points = cache.stored_points();
                            let text_bytes = cache.stored_text_bytes();
                            DeferredReduction::Candidate {
                                source: (points <= remaining_points
                                    && text_bytes <= remaining_text)
                                    .then_some(cache),
                                points,
                                text_bytes,
                                #[cfg(test)]
                                rejection_backing: RejectionBacking::default(),
                            }
                        }
                        None => DeferredReduction::Reduced,
                    }
                }
                CapturedSource::Deferred(source) => {
                    let Some(target) = session.part_target(source.part) else {
                        debug_assert!(false, "invalid deferred chart cache span");
                        return;
                    };
                    let mut reduced = None;
                    provider.with_part(&target, session, &mut |data| {
                        let Some(data) = data else {
                            return;
                        };
                        let Some(data) = source.range().and_then(|range| data.get(range)) else {
                            debug_assert!(false, "invalid deferred chart cache span");
                            return;
                        };
                        reduced = Some(reduce_deferred_cache(
                            data,
                            &source,
                            request.expected_points(),
                            retain_shape,
                            remaining_points,
                            remaining_text,
                            |point| request.display(book, &raw, &streamed, point),
                        ));
                    });
                    let Some(reduced) = reduced else {
                        return;
                    };
                    #[cfg(test)]
                    {
                        *session.rejection_backing.borrow_mut() = reduced.rejection_backing();
                    }
                    reduced
                }
            };
            match reduced {
                DeferredReduction::Reduced => {
                    report.reduced_caches += 1;
                    report.dropped_caches += 1;
                }
                DeferredReduction::Candidate {
                    source: Some(cache),
                    points,
                    text_bytes,
                    ..
                } => {
                    report.kept_caches += 1;
                    report.kept_points += points;
                    report.kept_text_bytes += text_bytes;
                    session.replace(reference, cache);
                }
                DeferredReduction::Candidate {
                    source: None,
                    points,
                    text_bytes,
                    ..
                } => {
                    report.dropped_caches += 1;
                    report.over_cap_caches += 1;
                    report.over_cap_points = report.over_cap_points.saturating_add(points);
                    report.over_cap_text_bytes =
                        report.over_cap_text_bytes.saturating_add(text_bytes);
                }
            }
        });
        #[cfg(debug_assertions)]
        {
            let worksheet = &mut book.sheet_collection_unchecked_mut()[*index];
            audit_surviving_sources(&mut drawing, worksheet.ole_objects_mut());
        }
        *book.sheet_collection_unchecked_mut()[*index].worksheet_drawing_mut() = drawing;
        provider.finish_sheet(session);
    }
    *book.chart_source_cache_report_mut() = report;
}

/// Visits, in a fixed order, every string reference of the charts of a
/// worksheet, including charts held by drawing frames.
fn visit_drawing(
    drawing: &mut crate::drawing::spreadsheet::WorksheetDrawing,
    visit: &mut dyn FnMut(&mut StringReference),
) {
    for chart in drawing.chart_collection_mut() {
        visit_string_references(chart.chart_space_mut(), &mut |_, reference| {
            visit(reference);
        });
    }
    for anchor in drawing.one_cell_anchor_collection_mut() {
        if let Some(frame) = anchor.graphic_frame_mut() {
            visit_string_references(
                frame.graphic_mut().graphic_data_mut().chart_space_mut(),
                &mut |_, reference| visit(reference),
            );
        }
    }
    for frame in drawing.graphic_frame_collection_mut() {
        visit_string_references(
            frame.graphic_mut().graphic_data_mut().chart_space_mut(),
            &mut |_, reference| visit(reference),
        );
    }
}

/// Visits only the image-classified one-cell frame carriers. OLE anchors and
/// image two-cell anchors are deliberately absent from this route.
fn visit_image_frames(
    drawing: &mut crate::drawing::spreadsheet::WorksheetDrawing,
    visit: &mut dyn FnMut(&mut StringReference),
) {
    for image in drawing.image_collection_mut() {
        if let Some(frame) = image
            .one_cell_anchor_mut()
            .and_then(|anchor| anchor.graphic_frame_mut())
        {
            visit_string_references(
                frame.graphic_mut().graphic_data_mut().chart_space_mut(),
                &mut |_, reference| visit(reference),
            );
        }
    }
}

#[cfg(debug_assertions)]
fn audit_surviving_sources(
    drawing: &mut crate::drawing::spreadsheet::WorksheetDrawing,
    ole_objects: &mut crate::structs::OleObjects,
) {
    let mut audit = |reference: &mut StringReference| {
        debug_assert!(
            !reference.string_cache().source_is_deferred(),
            "deferred chart source cache escaped materialization"
        );
    };
    visit_drawing(drawing, &mut audit);
    visit_image_frames(drawing, &mut audit);
    for ole in ole_objects.ole_object_mut() {
        if let Some(frame) = ole.two_cell_anchor_mut().graphic_frame_mut() {
            visit_string_references(
                frame.graphic_mut().graphic_data_mut().chart_space_mut(),
                &mut |_, reference| audit(reference),
            );
        }
    }
}

struct Request {
    sheet:   Option<usize>,
    range:   Option<IndexedRange>,
    literal: Option<String>,
}

impl Request {
    fn new(reference: &StringReference, names: &HashMap<String, usize>) -> Self {
        let formula = reference.formula();
        let literal = formula.has_string_value();
        Self {
            sheet:   (!literal)
                .then(|| names.get(unescape_sheet_name(formula.address().sheet_name()).as_ref()))
                .flatten()
                .copied(),
            range:   IndexedRange::from_formula(formula),
            literal: literal.then(|| literal_text(formula)),
        }
    }

    fn expected_points(&self) -> Option<u64> {
        if self.literal.is_some() {
            Some(1)
        } else {
            self.range.as_ref().map(IndexedRange::len)
        }
    }

    fn display(
        &self,
        book: &Workbook,
        raw: &HashSet<usize>,
        streamed: &HashMap<usize, HashMap<Coordinate, String>>,
        point: u32,
    ) -> Option<String> {
        if let Some(text) = &self.literal {
            return (point == 0).then(|| text.clone());
        }
        let index = self.sheet?;
        let coordinate = self.range.as_ref()?.coordinate(point)?;
        if raw.contains(&index) {
            streamed
                .get(&index)
                .map(|values| values.get(&coordinate).cloned().unwrap_or_default())
        } else {
            Some(
                book.sheet_collection_no_check()[index]
                    .cells_crate()
                    .get(coordinate)
                    .map_or_else(String::new, Cell::formatted_value),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::{
        Cursor,
        Read,
        Write,
    };
    #[cfg(debug_assertions)]
    use std::panic::{
        AssertUnwindSafe,
        catch_unwind,
    };

    use quick_xml::Writer;

    use super::*;
    #[cfg(debug_assertions)]
    use crate::OleObject;
    use crate::{
        Chart,
        ChartType,
        drawing::{
            charts::CategoryAxisData,
            spreadsheet::MarkerType,
        },
        structs::raw::RawWorksheet,
    };

    fn deferred_reference(session: &SourceCacheSession, xml: &str) -> StringReference {
        const TARGET: &str = "deferred.xml";
        session.begin_chart_part(TARGET);
        session.store_part(session.part_for(TARGET), xml.as_bytes().into());
        let mut reader = Reader::from_reader(xml.as_bytes());
        let mut buffer = Vec::new();
        let Event::Start(root) = reader.read_event_into(&mut buffer).unwrap() else {
            panic!("missing string reference");
        };
        let mut reference = StringReference::default();
        reference.set_attributes(&mut reader, &root, session);
        session.end_chart_part();
        reference
    }

    fn lazy_provider() -> LazyChartPartProvider {
        LazyChartPartProvider::new(HashMap::new())
    }

    fn chart_with_reference(reference: StringReference) -> Chart {
        let mut category = CategoryAxisData::default();
        category.set_string_reference(reference);
        let mut chart = Chart::default();
        chart.new_chart(
            &ChartType::LineChart,
            MarkerType::default(),
            MarkerType::default(),
            vec!["Sheet1!B1"],
        );
        chart.area_chart_series_list_mut().area_chart_series_mut()[0]
            .set_category_axis_data(category);
        chart
    }

    #[test]
    fn reader_image_hydration_allocates_no_temporary_source_boxes() {
        for cache in [
            concat!(
                "<c:strCache><c:ptCount val=\"1\"/>",
                "<c:pt idx=\"0\"><c:v>L</c:v></c:pt></c:strCache>"
            ),
            "<c:strCache/>",
        ] {
            let book = crate::reader::xlsx::read_reader(
                Cursor::new(image_deferred_cache_package(cache)),
                true,
            )
            .unwrap();
            assert_eq!(
                crate::drawing::charts::DeferredHydrationSourceBoxObservation::count(),
                0
            );
            let source = book
                .sheet(0)
                .unwrap()
                .worksheet_drawing()
                .image_collection()[0]
                .one_cell_anchor()
                .unwrap()
                .graphic_frame()
                .unwrap()
                .graphic()
                .graphic_data()
                .chart_space()
                .chart()
                .plot_area()
                .chart_groups()
                .next()
                .unwrap()
                .series()
                .area_chart_series()[0]
                .category_axis_data()
                .unwrap()
                .string_reference()
                .unwrap()
                .string_cache()
                .source()
                .unwrap();
            assert_eq!(source.formula(), "Sheet1!$A$1");
            assert_eq!(source.point_count(), cache.contains("ptCount").then_some(1));
        }
    }

    #[test]
    fn admitted_replacement_clears_a_descriptor_at_its_new_box_address() {
        let session = SourceCacheSession::deferred();
        let mut reference = deferred_reference(
            &session,
            "<c:strRef><c:f>F</c:f><c:strCache><c:pt \
             idx=\"0\"><c:v>L</c:v></c:pt></c:strCache></c:strRef>",
        );
        let admitted = session.take_for_capture(&mut reference).unwrap();
        reference
            .string_cache_mut()
            .set_source(Some(SourceCache::as_read(
                "placeholder".into(),
                None,
                Vec::new(),
            )));
        let key = reference.string_cache().source_key().unwrap();
        session.descriptors.borrow_mut().insert(
            key,
            DeferredSource {
                part:    0,
                start:   0,
                end:     0,
                formula: "stale".into(),
                summary: None,
            },
        );

        let CapturedSource::Deferred(admitted) = admitted else {
            panic!("deferred source was materialized");
        };
        session.replace(&mut reference, session.materialize(admitted));

        let key = reference.string_cache().source_key().unwrap();
        assert!(!session.descriptors.borrow().contains_key(&key));
    }

    #[cfg(debug_assertions)]
    #[test]
    fn missing_descriptor_during_real_capture_asserts_in_debug() {
        let (mut unaffected, mut unaffected_session) = materialize_descriptor_loss();
        let mut unaffected_provider = lazy_provider();
        capture_chart_source_caches(
            &mut unaffected,
            &[0],
            &mut unaffected_session,
            &mut unaffected_provider,
        );

        let (mut book, mut session) = materialize_descriptor_loss();
        session.arm_descriptor_loss();
        let mut provider = lazy_provider();
        let failure = catch_unwind(AssertUnwindSafe(|| {
            capture_chart_source_caches(&mut book, &[0], &mut session, &mut provider);
        }))
        .expect_err("the armed deferred session must lose its own descriptor");
        assert!(session.descriptor_loss_was_consumed());
        let message = failure.downcast_ref::<String>().map_or_else(
            || failure.downcast_ref::<&str>().copied().unwrap_or_default(),
            String::as_str,
        );
        assert!(message.contains("missing deferred chart source cache descriptor"));
    }

    #[cfg(debug_assertions)]
    #[test]
    fn session_end_audit_checks_canonical_image_and_ole_guards_without_ole_lookup() {
        let mut canonical = imported_audit_package();
        audit_sheet(&mut canonical);
        poison_first_reference(
            canonical.sheet_mut(0).unwrap().chart_collection_mut()[0].chart_space_mut(),
        );
        assert!(catch_unwind(AssertUnwindSafe(|| audit_sheet(&mut canonical))).is_err());

        let mut image =
            crate::reader::xlsx::read_reader(Cursor::new(grouped_one_cell_audit_package()), true)
                .unwrap();
        audit_sheet(&mut image);
        poison_first_reference(
            image
                .sheet_mut(0)
                .unwrap()
                .worksheet_drawing_mut()
                .image_collection_mut()[0]
                .one_cell_anchor_mut()
                .unwrap()
                .graphic_frame_mut()
                .unwrap()
                .graphic_mut()
                .graphic_data_mut()
                .chart_space_mut(),
        );
        assert!(catch_unwind(AssertUnwindSafe(|| audit_sheet(&mut image))).is_err());

        let mut ole =
            crate::reader::xlsx::read_reader(Cursor::new(ole_audit_package()), true).unwrap();
        audit_sheet(&mut ole);
        poison_first_reference(
            ole.sheet_mut(0).unwrap().ole_objects_mut().ole_object_mut()[0]
                .two_cell_anchor_mut()
                .graphic_frame_mut()
                .unwrap()
                .graphic_mut()
                .graphic_data_mut()
                .chart_space_mut(),
        );
        assert!(catch_unwind(AssertUnwindSafe(|| audit_sheet(&mut ole))).is_err());
    }

    #[cfg(debug_assertions)]
    fn audit_sheet(book: &mut Workbook) {
        let sheet = &mut book.sheet_collection_unchecked_mut()[0];
        let mut drawing = std::mem::take(sheet.worksheet_drawing_mut());
        audit_surviving_sources(&mut drawing, sheet.ole_objects_mut());
        *sheet.worksheet_drawing_mut() = drawing;
    }

    #[cfg(debug_assertions)]
    fn poison_first_reference(chart_space: &mut crate::drawing::charts::ChartSpace) {
        let mut poisoned = false;
        visit_string_references(chart_space, &mut |_, reference| {
            if !poisoned {
                let formula = reference
                    .string_cache()
                    .source()
                    .unwrap()
                    .formula()
                    .to_string();
                reference
                    .string_cache_mut()
                    .finalize_source_formula(format!("{formula}\0"));
                poisoned = true;
            }
        });
        assert!(poisoned);
    }

    #[cfg(not(debug_assertions))]
    #[test]
    fn missing_descriptor_during_real_capture_uses_generated_export_in_release() {
        let (mut book, mut session) = materialize_descriptor_loss();
        session.arm_descriptor_loss();
        let mut provider = lazy_provider();
        capture_chart_source_caches(&mut book, &[0], &mut session, &mut provider);
        assert!(session.descriptor_loss_was_consumed());
        let reference = book.sheet(0).unwrap().chart_collection()[0]
            .chart_space()
            .chart()
            .plot_area()
            .chart_groups()
            .next()
            .unwrap()
            .series()
            .area_chart_series()[0]
            .category_axis_data()
            .unwrap()
            .string_reference()
            .unwrap();
        assert!(reference.string_cache().source().is_none());
        let report = book.chart_source_cache_report();
        assert_eq!(
            (
                report.kept_caches,
                report.kept_points,
                report.kept_text_bytes,
                report.dropped_caches,
                report.reduced_caches,
                report.over_cap_caches,
                report.over_cap_points,
                report.over_cap_text_bytes,
            ),
            (0, 0, 0, 0, 0, 0, 0, 0)
        );
        let mut output = Vec::new();
        crate::writer::xlsx::write_writer(&book, &mut Cursor::new(&mut output)).unwrap();
        let mut archive = zip::ZipArchive::new(Cursor::new(output)).unwrap();
        let mut chart = String::new();
        archive
            .by_name("xl/charts/chart1.xml")
            .unwrap()
            .read_to_string(&mut chart)
            .unwrap();
        assert!(chart.contains(
            "<c:strRef><c:f>Sheet1!$A$1</c:f><c:strCache><c:ptCount val=\"1\"/><c:pt \
             idx=\"0\"><c:v>1</c:v></c:pt></c:strCache></c:strRef>"
        ));
    }

    fn descriptor_loss_package() -> Vec<u8> {
        let formula = "Sheet1!$A$1";
        let mut book = crate::new_file();
        book.sheet_mut(0)
            .unwrap()
            .cell_mut("A1")
            .set_value_number(1);
        let mut reference = StringReference::default();
        reference.formula_mut().set_address_str(formula);
        reference
            .string_cache_mut()
            .set_source(Some(SourceCache::as_read(
                formula.into(),
                Some(1),
                vec![(0, Some("L".into()))],
            )));
        book.sheet_mut(0)
            .unwrap()
            .add_chart(chart_with_reference(reference));
        let mut bytes = Vec::new();
        crate::writer::xlsx::write_writer(&book, &mut Cursor::new(&mut bytes)).unwrap();
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for index in 0..archive.len() {
            let mut entry = archive.by_index(index).unwrap();
            let name = entry.name().to_string();
            let mut data = Vec::new();
            entry.read_to_end(&mut data).unwrap();
            if name == "xl/charts/chart1.xml" {
                let xml = String::from_utf8(data).unwrap();
                let start = xml.find("<c:cat>").unwrap();
                let end = start + xml[start..].find("</c:cat>").unwrap() + "</c:cat>".len();
                let category = concat!(
                    "<c:cat><c:strRef><c:f>Sheet1!$A$1</c:f><c:strCache>",
                    "<c:ptCount val=\"1\"/><c:pt idx=\"0\"><c:v>L</c:v></c:pt>",
                    "</c:strCache></c:strRef></c:cat>"
                );
                data = format!("{}{category}{}", &xml[..start], &xml[end..]).into_bytes();
            }
            output
                .start_file(name, zip::write::SimpleFileOptions::default())
                .unwrap();
            output.write_all(&data).unwrap();
        }
        let bytes = output.finish().unwrap().into_inner();
        let mut archive = zip::ZipArchive::new(Cursor::new(&bytes)).unwrap();
        let mut xml = String::new();
        archive
            .by_name("xl/charts/chart1.xml")
            .unwrap()
            .read_to_string(&mut xml)
            .unwrap();
        assert!(xml.contains(concat!(
            "<c:cat><c:strRef><c:f>Sheet1!$A$1</c:f><c:strCache>",
            "<c:ptCount val=\"1\"/><c:pt idx=\"0\"><c:v>L</c:v></c:pt>",
            "</c:strCache></c:strRef></c:cat>"
        )));
        bytes
    }

    fn materialize_descriptor_loss() -> (Workbook, SourceCacheSession) {
        let mut book =
            crate::reader::xlsx::read_reader(Cursor::new(descriptor_loss_package()), false)
                .unwrap();
        let shared_strings = book.shared_string_table();
        let stylesheet = book.stylesheet().clone();
        let mut session = SourceCacheSession::deferred();
        let mut provider = lazy_provider();
        crate::reader::xlsx::raw_to_deserialize_by_worksheet(
            &mut book.sheet_collection_unchecked_mut()[0],
            &shared_strings,
            &stylesheet,
            &mut session,
            &mut provider,
        );
        (book, session)
    }

    fn rewrite_package_part(
        bytes: &[u8],
        part: &str,
        edit: impl FnOnce(String) -> String,
    ) -> Vec<u8> {
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let mut edit = Some(edit);
        for index in 0..archive.len() {
            let mut entry = archive.by_index(index).unwrap();
            let name = entry.name().to_string();
            let mut data = Vec::new();
            entry.read_to_end(&mut data).unwrap();
            if name == part {
                data = edit.take().unwrap()(String::from_utf8(data).unwrap()).into_bytes();
            }
            output
                .start_file(name, zip::write::SimpleFileOptions::default())
                .unwrap();
            output.write_all(&data).unwrap();
        }
        assert!(edit.is_none(), "{part} was not rewritten");
        output.finish().unwrap().into_inner()
    }

    fn replace_chart_cache(xml: String, cache: &str) -> String {
        let start = xml.find("<c:strCache>").unwrap();
        let end = start + xml[start..].find("</c:strCache>").unwrap() + "</c:strCache>".len();
        format!("{}{cache}{}", &xml[..start], &xml[end..])
    }

    fn deferred_cache_package(cache: &str) -> Vec<u8> {
        rewrite_package_part(&descriptor_loss_package(), "xl/charts/chart1.xml", |xml| {
            replace_chart_cache(xml, cache)
        })
    }

    fn image_deferred_cache_package(cache: &str) -> Vec<u8> {
        rewrite_package_part(
            &deferred_cache_package(cache),
            "xl/drawings/drawing1.xml",
            |xml| {
                let start = xml.find("<xdr:to>").unwrap();
                let end = start + xml[start..].find("</xdr:to>").unwrap() + "</xdr:to>".len();
                format!(
                    "{}<xdr:ext cx=\"1\" cy=\"1\"/><xdr:grpSp><xdr:nvGrpSpPr><xdr:cNvPr id=\"1\" \
                     \\
                 name=\"Group \\
                 1\"/><xdr:cNvGrpSpPr/><xdr:nvPr/></xdr:nvGrpSpPr><xdr:grpSpPr/></xdr:grpSp>{}",
                    &xml[..start],
                    &xml[end..]
                )
                .replace("xdr:twoCellAnchor", "xdr:oneCellAnchor")
            },
        )
    }

    fn cache_with_indices(indices: impl IntoIterator<Item = u32>) -> String {
        let indices = indices.into_iter().collect::<Vec<_>>();
        let mut cache = format!("<c:strCache><c:ptCount val=\"{}\"/>", indices.len());
        for index in indices {
            use std::fmt::Write as _;
            write!(cache, "<c:pt idx=\"{index}\"><c:v>{index}</c:v></c:pt>").unwrap();
        }
        cache.push_str("</c:strCache>");
        cache
    }

    fn capture_deferred_cache(cache: &str) -> (Workbook, SourceCacheSession) {
        let mut book =
            crate::reader::xlsx::read_reader(Cursor::new(deferred_cache_package(cache)), false)
                .unwrap();
        let shared_strings = book.shared_string_table();
        let stylesheet = book.stylesheet().clone();
        let mut session = SourceCacheSession::deferred();
        let mut provider = lazy_provider();
        crate::reader::xlsx::raw_to_deserialize_by_worksheet(
            &mut book.sheet_collection_unchecked_mut()[0],
            &shared_strings,
            &stylesheet,
            &mut session,
            &mut provider,
        );
        capture_chart_source_caches(&mut book, &[0], &mut session, &mut provider);
        (book, session)
    }

    #[test]
    fn shape_rejection_releases_run_backing_before_value_decoding() {
        let mut cache = format!(
            "<c:strCache><c:ptCount val=\"{}\"/>",
            MAX_SOURCE_CACHE_POINTS + 2
        );
        for index in 0..=MAX_SOURCE_CACHE_POINTS as u32 {
            let value = if index == 0 { 1 } else { index * 2 };
            use std::fmt::Write as _;
            write!(
                cache,
                "<c:pt idx=\"{}\"><c:v>{value}</c:v></c:pt>",
                index * 2
            )
            .unwrap();
        }
        cache.push_str("<c:pt idx=\"0\"><c:v>1</c:v></c:pt></c:strCache>");
        let (book, session) = capture_deferred_cache(&cache);
        assert_eq!(
            session.rejection_backing().shape,
            Some(CandidateBacking {
                index_runs: None,
                exceptions: None,
            })
        );
        assert_eq!(session.rejection_backing().exception, None);
        let report = book.chart_source_cache_report();
        assert_eq!(
            (
                report.kept_caches,
                report.kept_points,
                report.kept_text_bytes,
                report.dropped_caches,
                report.reduced_caches,
                report.over_cap_caches,
                report.over_cap_points,
                report.over_cap_text_bytes
            ),
            (0, 0, 0, 1, 0, 1, 32_772, 76_373)
        );
    }

    #[test]
    fn exception_rejection_releases_backing_before_later_repeated_points_decode() {
        let mut cache = format!(
            "<c:strCache><c:ptCount val=\"{}\"/>",
            MAX_SOURCE_CACHE_POINTS + 3
        );
        for index in 0..=MAX_SOURCE_CACHE_POINTS as u32 + 2 {
            let value = if index == 0 { 1 } else { index };
            use std::fmt::Write as _;
            write!(cache, "<c:pt idx=\"{index}\"><c:v>{value}</c:v></c:pt>").unwrap();
        }
        cache.push_str("<c:pt idx=\"0\"><c:v>1</c:v></c:pt></c:strCache>");
        let (book, session) = capture_deferred_cache(&cache);
        assert_eq!(session.rejection_backing().shape, None);
        assert_eq!(
            session.rejection_backing().exception,
            Some(CandidateBacking {
                index_runs: None,
                exceptions: Some((0, 0)),
            })
        );
        let report = book.chart_source_cache_report();
        assert_eq!(
            (
                report.kept_caches,
                report.kept_points,
                report.kept_text_bytes,
                report.dropped_caches,
                report.reduced_caches,
                report.over_cap_caches,
                report.over_cap_points,
                report.over_cap_text_bytes
            ),
            (0, 0, 0, 1, 0, 1, 16_390, 70_828)
        );
    }

    #[test]
    fn reader_reduced_dense_cache_has_no_dense_run_capacity() {
        let rows = 100_000u32;
        let formula = format!("Sheet1!$A$1:$A${rows}");
        let mut imported = crate::new_file();
        for row in 1..=rows {
            imported
                .sheet_mut(0)
                .unwrap()
                .cell_mut((1, row))
                .set_value_number(row);
        }
        let mut reference = StringReference::default();
        reference.formula_mut().set_address_str(&formula);
        imported
            .sheet_mut(0)
            .unwrap()
            .add_chart(chart_with_reference(reference));
        let mut bytes = Vec::new();
        crate::writer::xlsx::write_writer(&imported, &mut Cursor::new(&mut bytes)).unwrap();
        let mut cache = format!("<c:strCache><c:ptCount val=\"{rows}\"/>");
        for index in 0..rows {
            let value = if index == 42 {
                "source".to_string()
            } else {
                (index + 1).to_string()
            };
            use std::fmt::Write as _;
            write!(cache, "<c:pt idx=\"{index}\"><c:v>{value}</c:v></c:pt>").unwrap();
        }
        cache.push_str("</c:strCache>");
        bytes = rewrite_package_part(&bytes, "xl/charts/chart1.xml", |xml| {
            replace_chart_cache(xml, &cache)
        });
        let book = crate::reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap();
        let source = book.sheet(0).unwrap().chart_collection()[0]
            .chart_space()
            .chart()
            .plot_area()
            .chart_groups()
            .next()
            .unwrap()
            .series()
            .area_chart_series()[0]
            .category_axis_data()
            .unwrap()
            .string_reference()
            .unwrap()
            .string_cache()
            .source()
            .unwrap();
        let (exceptions, index_runs) = source.retained_capacities();
        assert!(exceptions <= 2);
        assert_eq!(index_runs, 0);
    }

    #[test]
    fn skipped_subtrees_request_only_the_modeled_raw_coordinate() {
        let mut book =
            crate::reader::xlsx::read_reader(Cursor::new(skipped_subtree_request_package()), false)
                .unwrap();
        assert!(!book.sheet_collection_unchecked_mut()[1].is_deserialized());
        let shared_strings = book.shared_string_table();
        let stylesheet = book.stylesheet().clone();
        let mut session = SourceCacheSession::deferred();
        let mut provider = lazy_provider();
        crate::reader::xlsx::raw_to_deserialize_by_worksheet(
            &mut book.sheet_collection_unchecked_mut()[0],
            &shared_strings,
            &stylesheet,
            &mut session,
            &mut provider,
        );
        capture_chart_source_caches(&mut book, &[0], &mut session, &mut provider);
        let wanted = session.wanted_coordinates().unwrap();
        assert_eq!(wanted.len(), 1);
        assert_eq!(wanted[&1], HashSet::from([(1, 1)]));
        assert!(!book.sheet_collection_unchecked_mut()[1].is_deserialized());
    }

    fn skipped_subtree_request_package() -> Vec<u8> {
        let formula = "Data!$A$1";
        let mut book = crate::new_file();
        book.sheet_mut(0).unwrap().set_name("Host");
        book.new_sheet("Data")
            .unwrap()
            .cell_mut("A1")
            .set_value_number(1);
        let mut reference = StringReference::default();
        reference.formula_mut().set_address_str(formula);
        reference
            .string_cache_mut()
            .set_source(Some(SourceCache::as_read(
                formula.into(),
                Some(1),
                vec![(0, Some("A".into()))],
            )));
        book.sheet_mut(0)
            .unwrap()
            .add_chart(chart_with_reference(reference));
        let mut bytes = Vec::new();
        crate::writer::xlsx::write_writer(&book, &mut Cursor::new(&mut bytes)).unwrap();
        rewrite_package_part(&bytes, "xl/charts/chart1.xml", |xml| {
            let cache = concat!(
                "<c:strCache><c:ptCount val=\"1\"/>",
                "<c:pt idx=\"0\"><c:v>A</c:v></c:pt></c:strCache>"
            );
            let modeled = format!(
                "<c:idx val=\"0\"/><c:order \
                 val=\"0\"/><c:cat><c:strRef><c:f>Data!$A$1</c:f>{cache}</c:strRef></c:cat><c:\
                 val><c:numRef><c:f>Host!$B$1</c:f></c:numRef></c:val>"
            );
            let series_extension = format!(
                "<c:extLst><c:ext \
                 uri=\"series\"><c:cat><c:strRef><c:f>Data!$B$1</c:f>{cache}</c:strRef></c:cat></\
                 c:ext></c:extLst>"
            );
            let family_extension = format!(
                "<c:extLst><c:ext \
                 uri=\"family\"><c:cat><c:strRef><c:f>Data!$C$1</c:f>{cache}</c:strRef></c:cat></\
                 c:ext></c:extLst>"
            );
            let chart_extension = format!(
                "<c:extLst><c:ext \
                 uri=\"chart\"><c:cat><c:strRef><c:f>Data!$D$1</c:f>{cache}</c:strRef></c:cat></c:\
                 ext></c:extLst>"
            );
            let start = xml.find("<c:ser>").unwrap();
            let end = start + xml[start..].find("</c:ser>").unwrap() + "</c:ser>".len();
            format!(
                "{}<c:ser>{modeled}{series_extension}</c:ser>{}",
                &xml[..start],
                &xml[end..]
            )
            .replacen(
                "</c:lineChart>",
                &format!("{family_extension}</c:lineChart>"),
                1,
            )
            .replacen("</c:chart>", &format!("{chart_extension}</c:chart>"), 1)
        })
    }

    #[test]
    fn merged_chart_parts_capture_the_title_before_the_category() {
        reset_chart_part_buffer_observation();
        let eager =
            crate::reader::xlsx::read_reader(Cursor::new(merged_title_capture_package()), true)
                .unwrap();
        assert_eq!(eager.chart_source_cache_report().kept_caches, 2);
        assert_eq!(
            chart_part_buffer_observation(),
            ChartPartBufferObservation {
                eager_live:                       0,
                eager_peak:                       1,
                eager_openings:                   4,
                failed_reads_have_no_buffer:      true,
                lazy_source_reads_have_no_buffer: true,
                lazy_sheet_boundaries:            0,
                lazy_pending_parts_are_exact:     true,
            }
        );

        let mut book =
            crate::reader::xlsx::read_reader(Cursor::new(merged_title_capture_package()), false)
                .unwrap();
        let shared_strings = book.shared_string_table();
        let stylesheet = book.stylesheet().clone();
        let mut session = SourceCacheSession::deferred();
        let mut provider = lazy_provider();
        crate::reader::xlsx::raw_to_deserialize_by_worksheet(
            &mut book.sheet_collection_unchecked_mut()[0],
            &shared_strings,
            &stylesheet,
            &mut session,
            &mut provider,
        );
        capture_chart_source_caches(&mut book, &[0], &mut session, &mut provider);
        assert_eq!(session.capture_formulas(), ["Sheet1!$B$1", "Sheet1!$A$1"]);
        let report = book.chart_source_cache_report();
        assert_eq!(
            (
                report.kept_caches,
                report.kept_points,
                report.kept_text_bytes
            ),
            (2, 2, 4)
        );
    }

    #[test]
    fn eager_provider_opens_a_part_once_for_consecutive_references() {
        reset_chart_part_buffer_observation();
        let book =
            crate::reader::xlsx::read_reader(Cursor::new(two_reference_part_package()), true)
                .unwrap();
        assert_eq!(book.chart_source_cache_report().kept_caches, 2);
        let observation = chart_part_buffer_observation();
        assert_eq!(observation.eager_openings, 2);
        assert_eq!(observation.eager_peak, 1);
        assert_eq!(observation.eager_live, 0);
    }

    #[test]
    fn lazy_two_sheet_materialization_compacts_pending_chart_parts_before_advancing() {
        reset_chart_part_buffer_observation();
        let mut book =
            crate::reader::xlsx::read_reader(Cursor::new(two_sheet_chart_part_package()), false)
                .unwrap();
        book.read_sheet_collection();
        let observation = chart_part_buffer_observation();
        // Both sheets cross a boundary twice: once when deserialized and once
        // when captured, so dropping either release point changes the count.
        assert_eq!(observation.lazy_sheet_boundaries, 4);
        assert!(observation.lazy_pending_parts_are_exact);
    }

    fn two_sheet_chart_part_package() -> Vec<u8> {
        let mut book = crate::new_file();
        book.new_sheet("Second").unwrap();
        for (sheet, formula, value) in [(0, "Sheet1!$A$1", "first"), (1, "Second!$A$1", "second")] {
            book.sheet_collection_unchecked_mut()[sheet]
                .cell_mut("A1")
                .set_value(value);
            let mut reference = StringReference::default();
            reference.formula_mut().set_address_str(formula);
            reference
                .string_cache_mut()
                .set_source(Some(SourceCache::as_read(
                    formula.into(),
                    Some(1),
                    vec![(0, Some(value.into()))],
                )));
            book.sheet_collection_unchecked_mut()[sheet].add_chart(chart_with_reference(reference));
        }
        let mut bytes = Vec::new();
        crate::writer::xlsx::write_writer(&book, &mut Cursor::new(&mut bytes)).unwrap();
        bytes
    }

    #[test]
    fn unreadable_chart_parts_discard_partial_bytes() {
        fn corrupt_chart_crc(bytes: &[u8]) -> Vec<u8> {
            let mut corrupt = bytes.to_vec();
            let mut offset = 0;
            while offset + 46 <= corrupt.len() {
                if corrupt[offset..].starts_with(b"PK\x01\x02") {
                    let name_len =
                        u16::from_le_bytes([corrupt[offset + 28], corrupt[offset + 29]]) as usize;
                    let extra_len =
                        u16::from_le_bytes([corrupt[offset + 30], corrupt[offset + 31]]) as usize;
                    let comment_len =
                        u16::from_le_bytes([corrupt[offset + 32], corrupt[offset + 33]]) as usize;
                    let name_start = offset + 46;
                    let name_end = name_start + name_len;
                    if corrupt[name_start..name_end] == *b"xl/charts/chart1.xml" {
                        corrupt[offset + 16..offset + 20].copy_from_slice(&0u32.to_le_bytes());
                        return corrupt;
                    }
                    offset = name_end + extra_len + comment_len;
                } else {
                    offset += 1;
                }
            }
            panic!("chart entry was not found in the ZIP directory");
        }

        let bytes = corrupt_chart_crc(&deferred_cache_package(
            "<c:strCache><c:ptCount val=\"1\"/><c:pt idx=\"0\"><c:v>A</c:v></c:pt></c:strCache>",
        ));
        reset_chart_part_buffer_observation();
        crate::reader::xlsx::read_reader(Cursor::new(&bytes), true).unwrap();
        assert!(chart_part_buffer_observation().failed_reads_have_no_buffer);

        let path = std::env::temp_dir().join(format!(
            "umya-chart-part-observation-{}-{}.xlsx",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
        ));
        std::fs::write(&path, &bytes).unwrap();
        reset_chart_part_buffer_observation();
        let mut lazy = crate::reader::xlsx::lazy_read(&path).unwrap();
        lazy.sheet_mut(0).unwrap();
        std::fs::remove_file(path).unwrap();
        let observation = chart_part_buffer_observation();
        assert!(observation.lazy_source_reads_have_no_buffer);
        assert!(observation.lazy_pending_parts_are_exact);
    }

    fn merged_title_capture_package() -> Vec<u8> {
        let mut book = crate::new_file();
        book.sheet_mut(0)
            .unwrap()
            .cell_mut("A1")
            .set_value_number(1);
        book.sheet_mut(0)
            .unwrap()
            .cell_mut("B1")
            .set_value_number(1);
        for (formula, label) in [("Sheet1!$A$1", "A"), ("Sheet1!$B$1", "B")] {
            let mut reference = StringReference::default();
            reference.formula_mut().set_address_str(formula);
            reference
                .string_cache_mut()
                .set_source(Some(SourceCache::as_read(
                    formula.into(),
                    Some(1),
                    vec![(0, Some(label.into()))],
                )));
            book.sheet_mut(0)
                .unwrap()
                .add_chart(chart_with_reference(reference));
        }
        let mut bytes = Vec::new();
        crate::writer::xlsx::write_writer(&book, &mut Cursor::new(&mut bytes)).unwrap();
        bytes = rewrite_package_part(&bytes, "xl/charts/chart1.xml", |xml| {
            let start = xml.find("<c:cat>").unwrap();
            let end = start + xml[start..].find("</c:cat>").unwrap() + "</c:cat>".len();
            let category = concat!(
                "<c:cat><c:strRef><c:f>Sheet1!$A$1</c:f><c:strCache>",
                "<c:ptCount val=\"1\"/><c:pt idx=\"0\"><c:v>A</c:v></c:pt>",
                "</c:strCache></c:strRef></c:cat>"
            );
            format!("{}{category}{}", &xml[..start], &xml[end..])
        });
        bytes = rewrite_package_part(&bytes, "xl/charts/chart2.xml", |xml| {
            let title = concat!(
                "<c:title><c:tx><c:strRef><c:f>Sheet1!$B$1</c:f><c:strCache>",
                "<c:ptCount val=\"1\"/><c:pt idx=\"0\"><c:v>B</c:v></c:pt>",
                "</c:strCache></c:strRef></c:tx></c:title>"
            );
            let chart_start = xml.find("<c:chart>").unwrap() + "<c:chart>".len();
            let group_start = xml.find("<c:lineChart>").unwrap();
            let group_end = group_start
                + xml[group_start..].find("</c:lineChart>").unwrap()
                + "</c:lineChart>".len();
            format!(
                "{}{}{}{}",
                &xml[..chart_start],
                title,
                &xml[chart_start..group_start],
                &xml[group_end..]
            )
        });
        rewrite_package_part(&bytes, "xl/drawings/drawing1.xml", |xml| {
            let first_start = xml.find("<xdr:twoCellAnchor>").unwrap();
            let first_end = first_start
                + xml[first_start..].find("</xdr:twoCellAnchor>").unwrap()
                + "</xdr:twoCellAnchor>".len();
            let second_start = first_end + xml[first_end..].find("<xdr:twoCellAnchor>").unwrap();
            let second_end = second_start
                + xml[second_start..].find("</xdr:twoCellAnchor>").unwrap()
                + "</xdr:twoCellAnchor>".len();
            let first = &xml[first_start..first_end];
            let second = &xml[second_start..second_end];
            let chart_start = second.find("<c:chart ").unwrap();
            let chart_end = chart_start + second[chart_start..].find("/>").unwrap() + 2;
            let graphic_end = first.find("</a:graphicData>").unwrap();
            let merged = format!(
                "{}{}{}",
                &first[..graphic_end],
                &second[chart_start..chart_end],
                &first[graphic_end..]
            );
            format!("{}{}{}", &xml[..first_start], merged, &xml[second_end..])
        })
    }

    fn two_reference_part_package() -> Vec<u8> {
        rewrite_package_part(&descriptor_loss_package(), "xl/charts/chart1.xml", |xml| {
            let title = concat!(
                "<c:title><c:tx><c:strRef><c:f>Sheet1!$A$1</c:f><c:strCache>",
                "<c:ptCount val=\"1\"/><c:pt idx=\"0\"><c:v>L</c:v></c:pt>",
                "</c:strCache></c:strRef></c:tx></c:title>"
            );
            xml.replacen("<c:plotArea>", &format!("{title}<c:plotArea>"), 1)
        })
    }

    #[cfg(debug_assertions)]
    fn imported_audit_package() -> Workbook {
        crate::reader::xlsx::read_reader(Cursor::new(descriptor_loss_package()), true).unwrap()
    }

    #[cfg(debug_assertions)]
    fn grouped_one_cell_audit_package() -> Vec<u8> {
        rewrite_package_part(
            &descriptor_loss_package(),
            "xl/drawings/drawing1.xml",
            |xml| {
                let start = xml.find("<xdr:to>").unwrap();
                let end = start + xml[start..].find("</xdr:to>").unwrap() + "</xdr:to>".len();
                format!(
                    "{}<xdr:ext cx=\"1\" cy=\"1\"/><xdr:grpSp><xdr:nvGrpSpPr><xdr:cNvPr id=\"1\" \
                     name=\"Group \
                     1\"/><xdr:cNvGrpSpPr/><xdr:nvPr/></xdr:nvGrpSpPr><xdr:grpSpPr/></xdr:grpSp>{}",
                    &xml[..start],
                    &xml[end..]
                )
                .replace("xdr:twoCellAnchor", "xdr:oneCellAnchor")
            },
        )
    }

    #[cfg(debug_assertions)]
    fn ole_audit_package() -> Vec<u8> {
        let formula = "Sheet1!$A$1";
        let mut book = crate::new_file();
        book.sheet_mut(0)
            .unwrap()
            .cell_mut("A1")
            .set_value_number(1);
        let mut reference = StringReference::default();
        reference.formula_mut().set_address_str(formula);
        reference
            .string_cache_mut()
            .set_source(Some(SourceCache::as_read(
                formula.into(),
                Some(1),
                vec![(0, Some("L".into()))],
            )));
        book.sheet_mut(0)
            .unwrap()
            .add_chart(chart_with_reference(reference));
        let anchor = book.sheet(0).unwrap().chart_collection()[0]
            .two_cell_anchor()
            .clone();
        let mut object = OleObject::default();
        object.set_object_extension("bin");
        object.set_object_data([0]);
        object.set_two_cell_anchor(anchor);
        book.sheet_mut(0)
            .unwrap()
            .ole_objects_mut()
            .set_ole_object(object);
        let mut bytes = Vec::new();
        crate::writer::xlsx::write_writer(&book, &mut Cursor::new(&mut bytes)).unwrap();
        rewrite_package_part(&bytes, "xl/drawings/drawing1.xml", |xml| {
            let start = xml.find("<xdr:twoCellAnchor>").unwrap();
            let end = start
                + xml[start..].find("</xdr:twoCellAnchor>").unwrap()
                + "</xdr:twoCellAnchor>".len();
            let anchor = &xml[start..end];
            let drawing_end = xml.find("</xdr:wsDr>").unwrap();
            format!(
                "{}<mc:AlternateContent xmlns:mc=\"http://schemas.openxmlformats.org/markup-compatibility/2006\"><mc:Choice xmlns:a14=\"http://schemas.microsoft.com/office/drawing/2010/main\" Requires=\"a14\">{anchor}</mc:Choice></mc:AlternateContent>{}",
                &xml[..start],
                &xml[drawing_end..]
            )
        })
    }

    #[test]
    fn failed_raw_streams_are_unavailable_at_capture_and_stored_export() {
        let missing = std::env::temp_dir().join(format!(
            "unavailable-chart-{}-{}.xlsx",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
        ));
        let mut book = crate::new_file();
        book.new_sheet("Data").unwrap();
        let mut raw = RawWorksheet::default();
        raw.worksheet_file_mut().set_source_file(missing);
        book.sheet_collection_unchecked_mut()[1].set_raw_data_of_worksheet(raw);
        let mut chart = Chart::default();
        chart.new_chart(
            &ChartType::LineChart,
            MarkerType::default(),
            MarkerType::default(),
            vec!["Sheet1!B1:B2"],
        );
        let mut reference = StringReference::default();
        reference.formula_mut().set_address_str("Data!A1:A2");
        reference
            .string_cache_mut()
            .set_source(Some(SourceCache::as_read(
                "Data!A1:A2".into(),
                Some(2),
                vec![(0, Some("source".into())), (1, None)],
            )));
        let mut category = CategoryAxisData::default();
        category.set_string_reference(reference);
        chart.area_chart_series_list_mut().area_chart_series_mut()[0]
            .set_category_axis_data(category);
        book.sheet_collection_unchecked_mut()[0].add_chart(chart);
        let mut session = SourceCacheSession::immediate();
        let mut provider = lazy_provider();
        capture_chart_source_caches(&mut book, &[0], &mut session, &mut provider);
        let chart = &book.sheet_collection_no_check()[0].chart_collection()[0];
        let reference = chart
            .chart_space()
            .chart()
            .plot_area()
            .chart_groups()
            .next()
            .unwrap()
            .series()
            .area_chart_series()[0]
            .category_axis_data()
            .unwrap()
            .string_reference()
            .unwrap();
        let source = reference.string_cache().source().unwrap();
        assert_eq!(source.exceptions().len(), 2);
        assert!(
            source
                .exceptions()
                .iter()
                .all(|point| point.loaded_display().is_none())
        );
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        reference
            .string_cache()
            .write_to(&mut writer, reference.formula(), &book);
        assert_eq!(
            String::from_utf8(writer.into_inner().into_inner()).unwrap(),
            "<c:strCache><c:ptCount val=\"2\"/><c:pt idx=\"0\"><c:v>source</c:v></c:pt><c:pt \
             idx=\"1\"></c:pt></c:strCache>"
        );
        assert_eq!(book.chart_source_cache_report().kept_points, 2);
    }

    #[cfg(feature = "serde")]
    #[test]
    fn all_native_drawing_carriers_pass_source_cache_admission() {
        use quick_xml::{
            Reader,
            events::Event,
        };

        use crate::drawing::spreadsheet::{
            OneCellAnchor,
            TwoCellAnchor,
        };

        // A native standalone frame is held outside chart_collection. Parse
        // its cache before admission, just like the two anchor forms.
        let xml = concat!(
            "<c:strRef><c:f>Sheet1!$A$1</c:f><c:strCache>",
            "<c:ptCount val=\"1\"/><c:pt idx=\"0\"><c:v>source</c:v></c:pt>",
            "</c:strCache></c:strRef>"
        );
        let mut reader = Reader::from_reader(xml.as_bytes());
        let mut buffer = Vec::new();
        let Event::Start(root) = reader.read_event_into(&mut buffer).unwrap() else {
            panic!("missing string reference");
        };
        let mut reference = StringReference::default();
        let session = SourceCacheSession::immediate();
        reference.set_attributes(&mut reader, &root, &session);
        let mut category = CategoryAxisData::default();
        category.set_string_reference(reference);
        let mut chart = Chart::default();
        chart.new_chart(
            &ChartType::LineChart,
            MarkerType::default(),
            MarkerType::default(),
            vec!["Sheet1!B1"],
        );
        chart.area_chart_series_list_mut().area_chart_series_mut()[0]
            .set_category_axis_data(category);
        let frame = chart.two_cell_anchor().graphic_frame().unwrap().clone();
        let mut one = OneCellAnchor::default();
        one.set_graphic_frame(frame.clone());
        let mut standalone = TwoCellAnchor::default();
        standalone.set_graphic_frame(frame);
        let mut book = crate::new_file();
        book.sheet_mut(0)
            .unwrap()
            .cell_mut("A1")
            .set_value_number(1);
        let drawing = book.sheet_collection_unchecked_mut()[0].worksheet_drawing_mut();
        drawing.add_chart_collection(chart);
        drawing.add_one_cell_anchor_collection(one);
        drawing.add_two_cell_anchor_collection(standalone);

        fn sources(value: &serde_json::Value) -> Vec<&serde_json::Value> {
            match value {
                serde_json::Value::Object(map) => {
                    let mut found = Vec::new();
                    if let Some(source) = map.get("source").filter(|source| !source.is_null()) {
                        assert!(source.is_object(), "unexpected source: {source}");
                        found.push(source);
                    }
                    for child in map.values() {
                        found.extend(sources(child));
                    }
                    found
                }
                serde_json::Value::Array(values) => values.iter().flat_map(sources).collect(),
                _ => Vec::new(),
            }
        }

        let before = serde_json::to_value(drawing).unwrap();
        let parsed = sources(&before);
        assert_eq!(parsed.len(), 3);
        assert!(
            parsed
                .iter()
                .all(|source| source["exceptions"][0]["loaded_display"].is_null())
        );
        let mut session = SourceCacheSession::immediate();
        let mut provider = lazy_provider();
        capture_chart_source_caches(&mut book, &[0], &mut session, &mut provider);
        let drawing = book.sheet_collection_no_check()[0].worksheet_drawing();
        assert_eq!(drawing.chart_collection().len(), 1);
        assert_eq!(drawing.one_cell_anchor_collection().len(), 1);
        assert_eq!(drawing.graphic_frame_collection().len(), 1);
        let after = serde_json::to_value(drawing).unwrap();
        let admitted = sources(&after);
        let expected = serde_json::json!({
            "formula": "Sheet1!$A$1",
            "point_count": 1,
            "index_runs": [],
            "exceptions": [{"index": 0, "value": "source", "loaded_display": "1"}]
        });
        let mut points = 0;
        let mut bytes = 0;
        for source in &admitted {
            assert_eq!(**source, expected);
            let exceptions = source["exceptions"].as_array().unwrap();
            points += exceptions.len() + source["index_runs"].as_array().unwrap().len();
            for point in exceptions {
                for field in ["value", "loaded_display"] {
                    bytes += point[field].as_str().map_or(0, str::len);
                }
            }
        }
        let report = book.chart_source_cache_report();
        assert_eq!(admitted.len(), 3);
        assert_eq!(admitted.len(), report.kept_caches);
        assert_eq!(points, report.kept_points);
        assert_eq!(bytes, report.kept_text_bytes);
        assert_eq!((points, bytes), (3, 21));
        assert_eq!(report.dropped_caches, 0);
        assert!(points <= MAX_SOURCE_CACHE_POINTS);
        assert!(bytes <= MAX_SOURCE_CACHE_TEXT_BYTES);
    }

    type Report = (usize, usize, usize, usize, usize, usize, usize, usize);
    type Facts = (
        Option<u32>,
        Vec<(u32, u32)>,
        Vec<(u32, Option<String>, Option<String>)>,
    );

    fn report_of(book: &Workbook) -> Report {
        let report = book.chart_source_cache_report();
        (
            report.kept_caches,
            report.kept_points,
            report.kept_text_bytes,
            report.dropped_caches,
            report.reduced_caches,
            report.over_cap_caches,
            report.over_cap_points,
            report.over_cap_text_bytes,
        )
    }

    fn category_source(book: &Workbook) -> Option<&SourceCache> {
        book.sheet(0).unwrap().chart_collection()[0]
            .chart_space()
            .chart()
            .plot_area()
            .chart_groups()
            .next()
            .unwrap()
            .series()
            .area_chart_series()[0]
            .category_axis_data()
            .unwrap()
            .string_reference()
            .unwrap()
            .string_cache()
            .source()
    }

    fn facts_of(source: &SourceCache) -> Facts {
        (
            source.point_count(),
            source
                .index_runs()
                .iter()
                .map(|run| (run.start(), run.len()))
                .collect(),
            source
                .exceptions()
                .iter()
                .map(|exception| {
                    (
                        exception.index(),
                        exception.value().map(str::to_string),
                        exception.loaded_display().map(str::to_string),
                    )
                })
                .collect(),
        )
    }

    fn facts(
        point_count: Option<u32>,
        runs: &[(u32, u32)],
        exceptions: &[(u32, Option<&str>, Option<&str>)],
    ) -> Facts {
        (
            point_count,
            runs.to_vec(),
            exceptions
                .iter()
                .map(|(index, value, display)| {
                    (
                        *index,
                        value.map(str::to_string),
                        display.map(str::to_string),
                    )
                })
                .collect(),
        )
    }

    fn string_reference_xml(cache: &str) -> String {
        format!("<c:strRef><c:f>Sheet1!$A$1</c:f>{cache}</c:strRef>")
    }

    fn descriptor_summary(
        session: &SourceCacheSession,
        reference: &StringReference,
    ) -> Option<DenseSummary> {
        let key = reference.string_cache().source_key().unwrap();
        session.descriptors.borrow().get(&key).unwrap().summary
    }

    fn descriptor_span(session: &SourceCacheSession, reference: &StringReference) -> Vec<u8> {
        let key = reference.string_cache().source_key().unwrap();
        let descriptors = session.descriptors.borrow();
        let source = descriptors.get(&key).unwrap();
        let parts = session.parts.borrow();
        parts[source.part].data.as_deref().unwrap()[source.range().unwrap()].to_vec()
    }

    /// The workbook after a deferred capture, with the work counters of the
    /// parsing phase and of the whole capture.
    fn capture_with_work(
        cache: &str,
        kept_points_before: usize,
    ) -> (
        Workbook,
        SourceCacheSession,
        crate::drawing::charts::WorkCounts,
        crate::drawing::charts::WorkCounts,
    ) {
        let mut book =
            crate::reader::xlsx::read_reader(Cursor::new(deferred_cache_package(cache)), false)
                .unwrap();
        let shared_strings = book.shared_string_table();
        let stylesheet = book.stylesheet().clone();
        let mut session = SourceCacheSession::deferred();
        let mut provider = lazy_provider();
        note_work(|counts| *counts = crate::drawing::charts::WorkCounts::default());
        crate::reader::xlsx::raw_to_deserialize_by_worksheet(
            &mut book.sheet_collection_unchecked_mut()[0],
            &shared_strings,
            &stylesheet,
            &mut session,
            &mut provider,
        );
        let parsed = crate::drawing::charts::work_counts();
        book.chart_source_cache_report_mut().kept_points = kept_points_before;
        capture_chart_source_caches(&mut book, &[0], &mut session, &mut provider);
        (book, session, parsed, crate::drawing::charts::work_counts())
    }

    fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
        match payload.downcast::<String>() {
            Ok(message) => *message,
            Err(payload) => (*payload.downcast::<&str>().unwrap()).to_string(),
        }
    }

    fn unwrap_message(error: &dyn std::fmt::Debug) -> String {
        format!("called `Result::unwrap()` on an `Err` value: {error:?}")
    }

    fn read_failure(bytes: Vec<u8>) -> String {
        use std::panic::catch_unwind as unwind;
        let outcome =
            unwind(|| crate::reader::xlsx::read_reader(Cursor::new(bytes), true).map(|_| ()));
        panic_message(outcome.expect_err("the read should fail"))
    }

    /// Replaces the first occurrence of `from` in a package part by `to`.
    fn rewrite_package_bytes(bytes: &[u8], part: &str, from: &[u8], to: &[u8]) -> Vec<u8> {
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let mut replaced = false;
        for index in 0..archive.len() {
            let mut entry = archive.by_index(index).unwrap();
            let name = entry.name().to_string();
            let mut data = Vec::new();
            entry.read_to_end(&mut data).unwrap();
            if name == part {
                let at = data
                    .windows(from.len())
                    .position(|window| window == from)
                    .unwrap();
                data.splice(at..at + from.len(), to.iter().copied());
                replaced = true;
            }
            output
                .start_file(name, zip::write::SimpleFileOptions::default())
                .unwrap();
            output.write_all(&data).unwrap();
        }
        assert!(replaced, "{part} was not rewritten");
        output.finish().unwrap().into_inner()
    }

    // Every cache is parsed by the real first pass and then captured. The
    // summary, the cached name, the kept source and the report are derived by
    // hand from the shape rules (the chart's range is one cell showing "1"),
    // and a summary must agree with a scan of the recorded span.
    #[test]
    fn first_pass_summaries_name_and_kept_sources_follow_the_cache_shape() {
        const NO_REPORT: Report = (0, 0, 0, 1, 1, 0, 0, 0);
        let pt =
            |index: &str, value: &str| format!("<c:pt idx=\"{index}\"><c:v>{value}</c:v></c:pt>");
        let cache = |body: &str| format!("<c:strCache>{body}</c:strCache>");
        let two = format!("{}{}", pt("0", "1"), pt("1", "b"));
        let cases: Vec<(
            &str,
            String,
            Option<u32>,
            Option<&str>,
            Report,
            Option<Facts>,
        )> = vec![
            (
                "dense",
                cache(&format!("<c:ptCount val=\"2\"/>{two}")),
                Some(2),
                Some("1"),
                (1, 1, 1, 0, 0, 0, 0, 0),
                Some(facts(Some(2), &[], &[(1, Some("b"), None)])),
            ),
            (
                "ptCount mismatch",
                cache(&format!("<c:ptCount val=\"3\"/>{two}")),
                None,
                Some("1"),
                (1, 2, 1, 0, 0, 0, 0, 0),
                Some(facts(Some(3), &[(0, 2)], &[(1, Some("b"), None)])),
            ),
            (
                "no ptCount",
                cache(&two),
                None,
                Some("1"),
                (1, 2, 1, 0, 0, 0, 0, 0),
                Some(facts(None, &[(0, 2)], &[(1, Some("b"), None)])),
            ),
            (
                "two ptCounts",
                cache(&format!(
                    "<c:ptCount val=\"2\"/><c:ptCount val=\"2\"/>{two}"
                )),
                None,
                Some("1"),
                (1, 1, 1, 0, 0, 0, 0, 0),
                Some(facts(Some(2), &[], &[(1, Some("b"), None)])),
            ),
            (
                "invalid later ptCount after a valid one",
                cache(&format!(
                    "<c:ptCount val=\"2\"/>{two}<c:ptCount val=\"x\"/>"
                )),
                None,
                Some("1"),
                (1, 2, 1, 0, 0, 0, 0, 0),
                Some(facts(None, &[(0, 2)], &[(1, Some("b"), None)])),
            ),
            (
                "duplicated zero index at the end",
                cache(&format!("<c:ptCount val=\"3\"/>{two}{}", pt("0", "1"))),
                None,
                Some("1"),
                (1, 5, 5, 0, 0, 0, 0, 0),
                Some(facts(
                    Some(3),
                    &[(0, 2), (0, 1)],
                    &[
                        (0, Some("1"), Some("1")),
                        (1, Some("b"), None),
                        (0, Some("1"), Some("1")),
                    ],
                )),
            ),
            (
                "unparseable index",
                cache(&format!(
                    "<c:ptCount val=\"2\"/>{}{}",
                    pt("0", "1"),
                    pt("x", "b")
                )),
                None,
                Some("1"),
                (1, 1, 0, 0, 0, 0, 0, 0),
                Some(facts(Some(2), &[(0, 1)], &[])),
            ),
            (
                "index written as a character reference is not parsed",
                cache(&format!(
                    "<c:ptCount val=\"2\"/>{}{}",
                    pt("0", "1"),
                    pt("&#49;", "b")
                )),
                None,
                Some("1"),
                (1, 1, 0, 0, 0, 0, 0, 0),
                Some(facts(Some(2), &[(0, 1)], &[])),
            ),
            (
                "plus zero parses as zero but is not the cached name",
                cache(&format!("<c:ptCount val=\"1\"/>{}", pt("+0", "1"))),
                Some(1),
                None,
                NO_REPORT,
                None,
            ),
            (
                "double zero parses as zero but is not the cached name",
                cache(&format!("<c:ptCount val=\"1\"/>{}", pt("00", "1"))),
                Some(1),
                None,
                NO_REPORT,
                None,
            ),
            (
                "first duplicate index attribute wins even when unparseable",
                cache(&format!(
                    "<c:ptCount val=\"1\"/><c:pt idx=\"x\" idx=\"0\"><c:v>1</c:v></c:pt>"
                )),
                None,
                None,
                (1, 1, 0, 0, 0, 0, 0, 0),
                Some(facts(Some(1), &[(0, 0)], &[])),
            ),
            (
                "first duplicate index attribute wins when it is zero",
                cache(&format!(
                    "<c:ptCount val=\"1\"/><c:pt idx=\"0\" idx=\"x\"><c:v>1</c:v></c:pt>"
                )),
                Some(1),
                Some("1"),
                NO_REPORT,
                None,
            ),
            (
                "prefixed index attribute is not the index",
                cache(&format!(
                    "<c:ptCount val=\"1\"/><c:pt x:idx=\"0\"><c:v>1</c:v></c:pt>"
                )),
                None,
                None,
                (1, 1, 0, 0, 0, 0, 0, 0),
                Some(facts(Some(1), &[(0, 0)], &[])),
            ),
            (
                "index overflowing u32",
                cache(&format!("<c:ptCount val=\"1\"/>{}", pt("4294967296", "1"))),
                None,
                None,
                (1, 1, 0, 0, 0, 0, 0, 0),
                Some(facts(Some(1), &[(0, 0)], &[])),
            ),
            (
                "self-closing zero point",
                cache("<c:ptCount val=\"1\"/><c:pt idx=\"0\"/>"),
                Some(1),
                None,
                (1, 1, 1, 0, 0, 0, 0, 0),
                Some(facts(Some(1), &[], &[(0, None, Some("1"))])),
            ),
            (
                "gap",
                cache(&format!(
                    "<c:ptCount val=\"2\"/>{}{}",
                    pt("0", "1"),
                    pt("2", "b")
                )),
                None,
                Some("1"),
                (1, 3, 1, 0, 0, 0, 0, 0),
                Some(facts(Some(2), &[(0, 1), (2, 1)], &[(2, Some("b"), None)])),
            ),
            (
                "non-ascending",
                cache(&format!(
                    "<c:ptCount val=\"2\"/>{}{}",
                    pt("1", "b"),
                    pt("0", "1")
                )),
                None,
                Some("1"),
                (1, 3, 1, 0, 0, 0, 0, 0),
                Some(facts(Some(2), &[(1, 1), (0, 1)], &[(1, Some("b"), None)])),
            ),
            (
                "extension list carrying a point count",
                cache(&format!(
                    "<c:ptCount val=\"1\"/>{}<c:extLst><c:ext><c:ptCount \
                     val=\"1\"/></c:ext></c:extLst>",
                    pt("0", "1")
                )),
                None,
                Some("1"),
                NO_REPORT,
                None,
            ),
            (
                "extension list carrying a point",
                cache(&format!(
                    "<c:ptCount val=\"1\"/>{}<c:extLst><c:ext>{}</c:ext></c:extLst>",
                    pt("0", "1"),
                    pt("5", "z")
                )),
                None,
                Some("1"),
                (1, 3, 1, 0, 0, 0, 0, 0),
                Some(facts(Some(1), &[(0, 1), (5, 1)], &[(5, Some("z"), None)])),
            ),
            (
                "label and point count after a nested empty cache",
                cache(&format!(
                    "<c:ptCount val=\"2\"/><c:strCache/>{}<c:ptCount val=\"1\"/>",
                    pt("0", "Q")
                )),
                None,
                Some("Q"),
                (1, 1, 0, 0, 0, 0, 0, 0),
                Some(facts(Some(2), &[(0, 0)], &[])),
            ),
            (
                "zero point count in a paired cache",
                cache("<c:ptCount val=\"0\"/>"),
                None,
                None,
                (1, 0, 0, 0, 0, 0, 0, 0),
                Some(facts(Some(0), &[], &[])),
            ),
            (
                "zero point count in a self-closing cache",
                "<c:strCache/>".to_string(),
                None,
                None,
                (1, 0, 0, 0, 0, 0, 0, 0),
                Some(facts(None, &[], &[])),
            ),
        ];
        for (name, cache, summary, cached, report, source) in cases {
            let session = SourceCacheSession::deferred();
            let reference = deferred_reference(&session, &string_reference_xml(&cache));
            assert_eq!(
                descriptor_summary(&session, &reference),
                summary.map(|point_count| DenseSummary { point_count }),
                "{name}: summary"
            );
            assert_eq!(reference.cached_value(), cached, "{name}: cached name");
            if let Some(summary) = descriptor_summary(&session, &reference) {
                let span = descriptor_span(&session, &reference);
                let scanned = deferred_shape(&span, usize::MAX);
                let dense = dense_shape(summary, usize::MAX);
                assert_eq!(scanned.point_count, dense.point_count, "{name}: count");
                assert_eq!(scanned.run_charge, dense.run_charge, "{name}: charge");
                assert_eq!(scanned.counted_dense, dense.counted_dense, "{name}: dense");
                assert_eq!(scanned.index_runs, dense.index_runs, "{name}: runs");
                assert!(scanned.repeated.is_empty() && dense.repeated.is_empty());
                assert_eq!(
                    scanned.shape_backing, dense.shape_backing,
                    "{name}: backing"
                );
            }
            let (book, ..) = capture_with_work(&cache, 0);
            assert_eq!(report_of(&book), report, "{name}: report");
            assert_eq!(
                category_source(&book).map(facts_of),
                source,
                "{name}: kept source"
            );
        }
    }

    // A point inside a point ends at the first point end, so the label after
    // it is still the cached name and the cache takes the scanning path.
    #[test]
    fn nested_points_keep_the_cached_name_and_the_scanning_replay() {
        for inner in ["c:pt", "x:pt"] {
            let cache = format!(
                "<c:strCache><c:ptCount val=\"2\"/><c:pt idx=\"1\"><{inner} \
                 idx=\"7\"></{inner}><c:pt idx=\"0\"><c:v>X</c:v></c:pt></c:pt></c:strCache>"
            );
            let session = SourceCacheSession::deferred();
            let reference = deferred_reference(&session, &string_reference_xml(&cache));
            assert_eq!(reference.cached_value(), Some("X"), "{inner}");
            assert_eq!(descriptor_summary(&session, &reference), None, "{inner}");
            let (book, _, parsed, captured) = capture_with_work(&cache, 0);
            assert_eq!(captured.shape_scans, 1, "{inner}");
            assert_eq!(captured.decode_scans, 1, "{inner}");
            assert_eq!(parsed.kept_values, 1, "{inner}");
            assert_eq!(report_of(&book), (1, 3, 2, 0, 0, 0, 0, 0), "{inner}");
            assert_eq!(
                category_source(&book).map(facts_of),
                Some(facts(
                    Some(2),
                    &[(1, 1)],
                    &[(1, None, None), (0, Some("X"), Some("1"))],
                )),
                "{inner}"
            );
        }
    }

    // A malformed value in a nonzero point fails the read even when a later
    // cache overwrites the one that holds it.
    #[test]
    fn overwritten_cache_fails_on_an_invalid_entity_in_a_nonzero_point() {
        let cache = concat!(
            "<c:strCache><c:ptCount val=\"2\"/>",
            "<c:pt idx=\"0\"><c:v>first</c:v></c:pt>",
            "<c:pt idx=\"1\"><c:v>&bogus;</c:v></c:pt></c:strCache>",
            "<c:strCache/>"
        );
        assert_eq!(
            read_failure(deferred_cache_package(cache)),
            unwrap_message(&quick_xml::escape::unescape("&bogus;").unwrap_err())
        );
    }

    #[test]
    fn overwritten_cache_fails_on_invalid_utf8_in_a_nonzero_point() {
        let cache = concat!(
            "<c:strCache><c:ptCount val=\"2\"/>",
            "<c:pt idx=\"0\"><c:v>first</c:v></c:pt>",
            "<c:pt idx=\"1\"><c:v>aZZb</c:v></c:pt></c:strCache>",
            "<c:strCache/>"
        );
        let bytes = rewrite_package_bytes(
            &deferred_cache_package(cache),
            "xl/charts/chart1.xml",
            b"ZZ",
            b"\xFF",
        );
        assert_eq!(
            read_failure(bytes),
            unwrap_message(&quick_xml::encoding::EncodingError::Utf8(
                std::str::from_utf8(std::hint::black_box(b"a\xFFb")).unwrap_err()
            ))
        );
    }

    // The index of a paired point is converted to text as it always was, so
    // invalid UTF-8 there fails with the failure of that conversion.
    #[test]
    fn paired_point_index_that_is_not_utf8_fails_as_the_attribute_conversion_does() {
        let xml = b"<c:strCache><c:pt idx=\"\xFF\"><c:v>x</c:v></c:pt></c:strCache>";
        let mut reader = Reader::from_reader(&xml[..]);
        let mut buffer = Vec::new();
        assert!(matches!(
            reader.read_event_into(&mut buffer).unwrap(),
            Event::Start(_)
        ));
        use std::panic::{
            AssertUnwindSafe as Safe,
            catch_unwind as unwind,
        };
        let outcome = unwind(Safe(|| {
            crate::drawing::charts::StringCache::set_deferred_attributes(&mut reader, "F".into()).0
        }));
        assert_eq!(
            panic_message(outcome.expect_err("the read should fail")),
            unwrap_message(&String::from_utf8(vec![0xFF]).unwrap_err())
        );
    }

    // The first pass never read the point count or the index of a
    // self-closing point, so a cache that is overwritten keeps reading fine
    // when those attributes are not UTF-8.
    #[test]
    fn overwritten_cache_with_attributes_that_are_not_utf8_still_reads() {
        for (name, cache, placeholder) in [
            (
                "point count",
                concat!(
                    "<c:strCache><c:ptCount val=\"ZQZQ\"/>",
                    "<c:pt idx=\"0\"><c:v>1</c:v></c:pt></c:strCache><c:strCache/>"
                ),
                &b"ZQZQ"[..],
            ),
            (
                "self-closing point index",
                concat!(
                    "<c:strCache><c:ptCount val=\"1\"/><c:pt idx=\"ZQZQ\"/></c:strCache>",
                    "<c:strCache/>"
                ),
                &b"ZQZQ"[..],
            ),
        ] {
            let bytes = rewrite_package_bytes(
                &deferred_cache_package(cache),
                "xl/charts/chart1.xml",
                placeholder,
                b"\xFF\xFE",
            );
            let book = crate::reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap();
            assert_eq!(report_of(&book), (1, 0, 0, 0, 0, 0, 0, 0), "{name}");
        }
    }

    // A positive dense cache that cannot afford its first run still records
    // the shape rejection before it is recognized as dense, and its first
    // exception is then rejected from an empty backing.
    #[test]
    fn dense_cache_after_the_point_budget_is_spent_records_both_rejections() {
        let cache = concat!(
            "<c:strCache><c:ptCount val=\"1\"/>",
            "<c:pt idx=\"0\"><c:v>L</c:v></c:pt></c:strCache>"
        );
        let (book, session, ..) = capture_with_work(cache, MAX_SOURCE_CACHE_POINTS);
        assert_eq!(
            session.rejection_backing().shape,
            Some(CandidateBacking {
                index_runs: None,
                exceptions: None,
            })
        );
        assert_eq!(
            session.rejection_backing().exception,
            Some(CandidateBacking {
                index_runs: None,
                exceptions: Some((0, 0)),
            })
        );
        assert_eq!(
            report_of(&book),
            (0, MAX_SOURCE_CACHE_POINTS, 0, 1, 0, 1, 1, 2)
        );
        assert!(category_source(&book).is_none());
    }

    // The scans that run are those the shape needs: a proven dense cache is
    // decoded once and never scanned for its shape.
    #[test]
    fn replay_scans_only_what_the_cache_shape_needs() {
        let two = |first: &str, second: &str| {
            format!(
                "<c:strCache><c:ptCount val=\"2\"/><c:pt idx=\"{first}\"><c:v>b</c:v></c:pt><c:pt \
                 idx=\"{second}\"><c:v>b</c:v></c:pt></c:strCache>"
            )
        };
        // (cache, shape scans, index scans) of the replay; every point is
        // decoded by exactly one decode scan.
        for (name, cache, shapes, indices) in [
            ("dense", two("0", "1"), 0, 0),
            ("sparse ascending", two("0", "2"), 1, 0),
            ("non-ascending", two("1", "0"), 1, 1),
        ] {
            let (_, _, parsed, captured) = capture_with_work(&cache, 0);
            assert_eq!(
                parsed.kept_values, 1,
                "{name}: first pass keeps the zero value"
            );
            assert_eq!(
                (parsed.shape_scans, parsed.decode_scans, parsed.index_scans),
                (0, 0, 0),
                "{name}: first pass scans nothing"
            );
            assert_eq!(captured.shape_scans, shapes, "{name}: shape scans");
            assert_eq!(captured.index_scans, indices, "{name}: index scans");
            assert_eq!(captured.decode_scans, 1, "{name}: decode scans");
            assert_eq!(
                captured.kept_values - parsed.kept_values,
                2,
                "{name}: each point decoded once"
            );
        }
    }

    // Each paired point at index zero materializes its value in the first
    // pass, and the last one is the cached name.
    #[test]
    fn first_pass_materializes_one_value_per_zero_index_point() {
        let session = SourceCacheSession::deferred();
        note_work(|counts| *counts = crate::drawing::charts::WorkCounts::default());
        let reference = deferred_reference(
            &session,
            &string_reference_xml(concat!(
                "<c:strCache><c:ptCount val=\"5\"/>",
                "<c:pt idx=\"0\"><c:v>first</c:v></c:pt>",
                "<c:pt idx=\"1\"><c:v>x</c:v></c:pt>",
                "<c:pt idx=\"2\"><c:v>y</c:v></c:pt>",
                "<c:pt idx=\"0\"><c:v>second</c:v></c:pt>",
                "<c:pt idx=\"3\"><c:v>z</c:v></c:pt></c:strCache>"
            )),
        );
        assert_eq!(reference.cached_value(), Some("second"));
        let counts = crate::drawing::charts::work_counts();
        assert_eq!(counts.kept_values, 2);
        assert_eq!(
            (counts.shape_scans, counts.decode_scans, counts.index_scans),
            (0, 0, 0)
        );
    }

    fn labeled_cache(order: impl IntoIterator<Item = u32>, count: usize) -> String {
        let mut cache = format!("<c:strCache><c:ptCount val=\"{count}\"/>");
        for index in order {
            cache.push_str(&format!(
                "<c:pt idx=\"{index}\"><c:v>V{index:05}</c:v></c:pt>"
            ));
        }
        cache.push_str("</c:strCache>");
        cache
    }

    fn labels(len: u32) -> Vec<String> {
        (0..len).map(|index| format!("V{index:05}")).collect()
    }

    // A buffer reused for every point grows by doubling, so one that never
    // holds more than 32 bytes allocates at most 1 + log2(32) = 6 times.
    const BUFFER_GROWTH: usize = 6;
    // The parser's stacks of open elements grow to the nesting depth.
    const PARSER_STATE: usize = 8;
    const POINTS: u32 = 2000;
    // A vector of 8-byte runs grown by doubling from 4: 1 + log2(2048 / 4).
    const RUN_GROWTH: usize = 11;
    // The set of seen indices doubles its table up to 2048 slots.
    const SEEN_GROWTH: usize = 13;

    fn first_event_consumed(xml: &[u8]) -> Reader<&[u8]> {
        let mut reader = Reader::from_reader(xml);
        let mut buffer = Vec::new();
        assert!(matches!(
            reader.read_event_into(&mut buffer).unwrap(),
            Event::Start(_)
        ));
        reader
    }

    // First-pass allocations do not depend on the number of points: besides
    // the kept name and source box, only the three reused buffers grow. Seen
    // per point they were: the index text, the point reader's event buffer,
    // the value text buffer, the decoded value and its copy.
    #[test]
    fn first_pass_allocates_nothing_per_point() {
        let cache = labeled_cache(0..POINTS, POINTS as usize);
        let mut reader = first_event_consumed(cache.as_bytes());
        let formula = String::from("Sheet1!$A$1");
        let ((name, _, summary), seen) = crate::test_alloc::observe(|| {
            crate::drawing::charts::StringCache::set_deferred_attributes(&mut reader, formula)
        });
        assert_eq!(name.as_deref(), Some("V00000"));
        assert_eq!(
            summary,
            Some(DenseSummary {
                point_count: POINTS,
            })
        );
        // Three reused buffers, the cached name, the source box, parser state.
        let allowed = 3 * BUFFER_GROWTH + 1 + 1 + PARSER_STATE;
        assert!(
            seen.total() <= allowed,
            "{seen:?} exceeds {allowed} for {POINTS} points"
        );
    }

    // Discarded values that carry entities and character references are
    // validated without building their text: the allowance is that of plain
    // values, the kept name plus the fixed buffers.
    #[test]
    fn first_pass_validates_entity_values_without_allocating() {
        let mut cache = format!("<c:strCache><c:ptCount val=\"{POINTS}\"/>");
        for index in 0..POINTS {
            cache.push_str(&format!(
                "<c:pt idx=\"{index}\"><c:v>A&amp;B&#x42;&lt;&#67;</c:v></c:pt>"
            ));
        }
        cache.push_str("</c:strCache>");
        let mut reader = first_event_consumed(cache.as_bytes());
        let formula = String::from("Sheet1!$A$1");
        let ((name, _, summary), seen) = crate::test_alloc::observe(|| {
            crate::drawing::charts::StringCache::set_deferred_attributes(&mut reader, formula)
        });
        assert_eq!(name.as_deref(), Some("A&BB<C"));
        assert_eq!(
            summary,
            Some(DenseSummary {
                point_count: POINTS,
            })
        );
        // Three reused buffers, the cached name, the source box, parser state.
        let allowed = 3 * BUFFER_GROWTH + 1 + 1 + PARSER_STATE;
        assert!(
            seen.total() <= allowed,
            "{seen:?} exceeds {allowed} for {POINTS} points"
        );
    }

    // The shape scan keeps one run per gap, which grows one vector, and
    // allocates nothing else per point. It used to allocate the index text and
    // a skip buffer for every point.
    #[test]
    fn shape_scan_allocates_only_its_runs() {
        let cache = labeled_cache((0..POINTS).map(|index| index * 2), POINTS as usize);
        let (shape, seen) =
            crate::test_alloc::observe(|| deferred_shape(cache.as_bytes(), usize::MAX));
        assert_eq!(shape.index_runs.as_ref().unwrap().len(), POINTS as usize);
        assert_eq!(shape.run_charge, POINTS as usize);
        // Event and skip buffers, the runs, the point count text, parser state.
        let allowed = 2 * BUFFER_GROWTH + RUN_GROWTH + 1 + PARSER_STATE;
        assert!(
            seen.total() <= allowed,
            "{seen:?} exceeds {allowed} for {POINTS} points"
        );
    }

    #[test]
    fn indices_scan_allocates_nothing_per_point() {
        let cache = labeled_cache((0..POINTS).rev(), POINTS as usize);
        let mut visited = 0u32;
        let ((), seen) = crate::test_alloc::observe(|| {
            visit_deferred_indices(cache.as_bytes(), |_| visited += 1);
        });
        assert_eq!(visited, POINTS);
        // Event and skip buffers, parser state.
        let allowed = 2 * BUFFER_GROWTH + PARSER_STATE;
        assert!(
            seen.total() <= allowed,
            "{seen:?} exceeds {allowed} for {POINTS} points"
        );
    }

    // Replay allocates, per point, the decoded value and the display text it
    // compares it with, and nothing else that grows with the cache. Each
    // scan adds only its reused buffers and what it keeps.
    #[test]
    fn replay_allocates_only_values_displays_and_kept_runs() {
        let displays = labels(2 * POINTS);
        let per_point = 2 * POINTS as usize;
        let state = 3 * BUFFER_GROWTH + PARSER_STATE;
        let shape = 2 * BUFFER_GROWTH + RUN_GROWTH + 1 + PARSER_STATE;
        let indices = 2 * BUFFER_GROWTH + SEEN_GROWTH + PARSER_STATE;
        // One shrink of the kept runs and one copy of the formula.
        let shrink = 2;
        let dense_cache = labeled_cache(0..POINTS, POINTS as usize);
        let sparse_cache = labeled_cache((0..POINTS).map(|index| index * 2), POINTS as usize);
        let reverse_cache = labeled_cache((0..POINTS).rev(), POINTS as usize);
        let cases = [
            (
                "dense",
                &dense_cache,
                Some(DenseSummary {
                    point_count: POINTS,
                }),
                per_point + state,
            ),
            (
                "sparse ascending",
                &sparse_cache,
                None,
                per_point + state + shape + shrink,
            ),
            (
                "non-ascending",
                &reverse_cache,
                None,
                per_point + state + shape + indices + shrink,
            ),
        ];
        for (name, cache, summary, allowed) in cases {
            let source = DeferredSource {
                part: 0,
                start: 0,
                end: 0,
                formula: String::from("F"),
                summary,
            };
            let (reduced, seen) = crate::test_alloc::observe(|| {
                reduce_deferred_cache(
                    cache.as_bytes(),
                    &source,
                    Some(u64::from(POINTS)),
                    false,
                    usize::MAX,
                    usize::MAX,
                    |index| Some(displays[index as usize].clone()),
                )
            });
            assert_eq!(
                matches!(reduced, DeferredReduction::Reduced),
                summary.is_some(),
                "{name}"
            );
            assert!(
                seen.total() >= per_point,
                "{name}: {seen:?} misses the per-point values and displays"
            );
            assert!(
                seen.total() <= allowed,
                "{name}: {seen:?} exceeds {allowed} for {POINTS} points"
            );
        }
    }
}
