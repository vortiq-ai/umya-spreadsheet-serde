// c:strCache
#[cfg(test)]
use std::cell::Cell as StateCell;
use std::{
    collections::{
        HashMap,
        HashSet,
    },
    io::Cursor,
};

#[cfg(test)]
thread_local! {
    static OBSERVE_DEFERRED_HYDRATION_SOURCE_BOXES: StateCell<bool> =
        const { StateCell::new(false) };
    static DEFERRED_HYDRATION_SOURCE_BOXES: StateCell<usize> =
        const { StateCell::new(0) };
    static WORK_COUNTS: StateCell<WorkCounts> = const {
        StateCell::new(WorkCounts {
            shape_scans:  0,
            decode_scans: 0,
            index_scans:  0,
            kept_values:  0,
        })
    };
}

/// How often the cache readers did each kind of per-cache work on this thread.
/// Each counter sits at the site that does the work.
#[cfg(test)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct WorkCounts {
    pub(crate) shape_scans:  usize,
    pub(crate) decode_scans: usize,
    pub(crate) index_scans:  usize,
    pub(crate) kept_values:  usize,
}

#[cfg(test)]
pub(crate) fn work_counts() -> WorkCounts {
    WORK_COUNTS.with(StateCell::get)
}

#[cfg(test)]
pub(crate) fn note_work(update: impl FnOnce(&mut WorkCounts)) {
    WORK_COUNTS.with(|counts| {
        let mut value = counts.get();
        update(&mut value);
        counts.set(value);
    });
}

#[cfg(test)]
pub(crate) struct DeferredHydrationSourceBoxObservation;

#[cfg(test)]
impl DeferredHydrationSourceBoxObservation {
    pub(crate) fn begin() -> Self {
        OBSERVE_DEFERRED_HYDRATION_SOURCE_BOXES.with(|observe| observe.set(true));
        DEFERRED_HYDRATION_SOURCE_BOXES.with(|count| count.set(0));
        Self
    }

    pub(crate) fn count() -> usize {
        DEFERRED_HYDRATION_SOURCE_BOXES.with(StateCell::get)
    }
}

#[cfg(test)]
impl Drop for DeferredHydrationSourceBoxObservation {
    fn drop(&mut self) {
        OBSERVE_DEFERRED_HYDRATION_SOURCE_BOXES.with(|observe| observe.set(false));
    }
}

#[cfg(test)]
fn note_deferred_hydration_source_box() {
    if OBSERVE_DEFERRED_HYDRATION_SOURCE_BOXES.with(StateCell::get) {
        DEFERRED_HYDRATION_SOURCE_BOXES.with(|count| count.set(count.get() + 1));
    }
}

use quick_xml::{
    Reader,
    Writer,
    events::{
        BytesStart,
        Event,
    },
};

use super::chart_source_caches::SourceCache;
use crate::{
    Cell,
    drawing::charts::{
        Formula,
        NumericValue,
    },
    helper::{
        address::unescape_sheet_name,
        utils::{
            unescape_xml_text,
            validate_xml_text,
        },
    },
    reader::driver::{
        attribute_str,
        attribute_u32,
        get_attribute,
        local_name,
        try_attribute_u32,
        xml_read_loop,
    },
    structs::{
        Cells,
        Workbook,
    },
    writer::driver::{
        write_end_tag,
        write_start_tag,
        write_text_node,
    },
};

/// A string cache. A chart is exported with the labels the referenced cells
/// display; `source` keeps only the labels the file stored that the cells
/// cannot reproduce, and is `None` for a cache the cells reproduce.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct StringCache {
    #[cfg_attr(feature = "serde", serde(default))]
    source: Option<Box<SourceCache>>,
}

/// Maps a cache point index onto a cell of a rectangular range, row by row.
pub(crate) struct IndexedRange {
    col_start: u32,
    row_start: u32,
    width:     u64,
    len:       u64,
}

impl IndexedRange {
    pub(crate) fn from_formula(formula: &Formula) -> Option<Self> {
        if formula.has_string_value() {
            return None;
        }
        let range = formula.address().range();
        let col_start = range.coordinate_start_col()?.num();
        let row_start = range.coordinate_start_row()?.num();
        let (col_end, row_end) = match (range.coordinate_end_col(), range.coordinate_end_row()) {
            (None, None) => (col_start, row_start),
            (Some(col), Some(row)) => (col.num(), row.num()),
            _ => return None,
        };
        if row_start == 0 || col_start == 0 {
            return None;
        }
        let width = u64::from(col_end.checked_sub(col_start)?) + 1;
        let height = u64::from(row_end.checked_sub(row_start)?) + 1;
        Some(Self {
            col_start,
            row_start,
            width,
            len: width * height,
        })
    }

    /// How many cells the range covers.
    pub(crate) fn len(&self) -> u64 {
        self.len
    }

    /// `(col, row)` of the cell a point index refers to, if inside the range.
    pub(crate) fn coordinate(&self, index: u32) -> Option<(u32, u32)> {
        let index = u64::from(index);
        if index >= self.len {
            return None;
        }
        let col = u64::from(self.col_start) + index % self.width;
        let row = u64::from(self.row_start) + index / self.width;
        Some((u32::try_from(col).ok()?, u32::try_from(row).ok()?))
    }
}

/// The text a literal (non-reference) formula contributes to a cache.
pub(crate) fn literal_text(formula: &Formula) -> String {
    let value = formula.address_str();
    match value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
    {
        Some(value) => value.replace("\"\"", "\""),
        None => value,
    }
}

/// Where the current display text of cache points comes from.
enum DisplaySource<'a> {
    Literal(String),
    Cells {
        cells: &'a Cells,
        range: Option<IndexedRange>,
    },
    /// A worksheet that is still raw: only the requested cells were read.
    Streamed {
        values: HashMap<(u32, u32), String>,
        range:  Option<IndexedRange>,
    },
    Unavailable,
}

impl<'a> DisplaySource<'a> {
    fn new(formula: &Formula, wb: &'a Workbook, indices: impl Iterator<Item = u32>) -> Self {
        if formula.has_string_value() {
            return Self::Literal(literal_text(formula));
        }
        let address = formula.address();
        let name = unescape_sheet_name(address.sheet_name());
        let range = IndexedRange::from_formula(formula);
        if let Ok(sheet) = wb.sheet_by_name(&name) {
            return Self::Cells {
                cells: sheet.cells_crate(),
                range,
            };
        }
        let Some(sheet) = wb
            .sheet_collection_no_check()
            .iter()
            .find(|sheet| sheet.name() == name && !sheet.is_deserialized())
        else {
            return Self::Unavailable;
        };
        let wanted: HashSet<(u32, u32)> = indices
            .filter_map(|index| range.as_ref()?.coordinate(index))
            .collect();
        let mut values = HashMap::new();
        let shared_string_table = wb.shared_string_table();
        let read = crate::reader::xlsx::worksheet::read_cells_stream(
            sheet.raw_data_of_worksheet(),
            &shared_string_table.read().unwrap(),
            wb.stylesheet(),
            |cell: &Cell| {
                let coordinate = (cell.coordinate().col_num(), cell.coordinate().row_num());
                if wanted.contains(&coordinate) {
                    values.insert(coordinate, cell.formatted_value());
                }
            },
        );
        if read.is_err() {
            return Self::Unavailable;
        }
        Self::Streamed { values, range }
    }

    fn display(&self, index: u32) -> Option<String> {
        match self {
            Self::Literal(text) => (index == 0).then(|| text.clone()),
            Self::Cells { cells, range } => {
                let coordinate = range.as_ref()?.coordinate(index)?;
                Some(
                    cells
                        .get(coordinate)
                        .map_or_else(String::new, Cell::formatted_value),
                )
            }
            Self::Streamed { values, range } => {
                let coordinate = range.as_ref()?.coordinate(index)?;
                Some(values.get(&coordinate).cloned().unwrap_or_default())
            }
            Self::Unavailable => None,
        }
    }
}

impl StringCache {
    /// What the file stored that the cells cannot reproduce, if anything.
    #[must_use]
    pub fn source(&self) -> Option<&SourceCache> {
        self.source.as_deref()
    }

    pub(crate) fn set_source(&mut self, source: Option<SourceCache>) {
        self.source = source.map(Box::new);
    }

    pub(crate) fn replace_or_set_source(&mut self, source: SourceCache) {
        if let Some(current) = self.source.as_deref_mut() {
            *current = source;
        } else {
            self.source = Some(Box::new(source));
        }
    }

    pub(crate) fn source_key(&self) -> Option<usize> {
        self.source
            .as_deref()
            .map(|source| std::ptr::from_ref(source).addr())
    }

    pub(crate) fn source_is_deferred(&self) -> bool {
        self.source()
            .is_some_and(|source| source.formula().ends_with('\0'))
    }

    pub(crate) fn replace_source_value(&mut self, source: SourceCache) -> Option<SourceCache> {
        self.source
            .as_deref_mut()
            .map(|current| std::mem::replace(current, source))
    }

    pub(crate) fn finalize_source_formula(&mut self, formula: String) {
        if let Some(source) = self.source.as_deref_mut() {
            source.set_formula(formula);
        }
    }

    pub(crate) fn take_source(&mut self) -> Option<SourceCache> {
        self.source.take().map(|source| *source)
    }

    /// The cache of an imported `c:strCache` that has no points at all.
    pub(crate) fn empty_source(formula: String) -> Self {
        Self {
            source: Some({
                #[cfg(test)]
                note_deferred_hydration_source_box();
                Box::new(SourceCache::as_read(formula, None, Vec::new()))
            }),
        }
    }

    /// Consumes a deferred cache while retaining only the lexical cached name.
    /// The source points are replayed from its recorded XML span after the
    /// workbook has established canonical admission order. Every point is read
    /// with the point grammar of the replay, so a malformed value fails here
    /// whether or not the cache survives. The summary is `Some` only when the
    /// cache is provably dense and counted by one `ptCount`.
    pub(crate) fn set_deferred_attributes<R: std::io::BufRead>(
        reader: &mut Reader<R>,
        formula: String,
    ) -> (Option<String>, Self, Option<DenseSummary>) {
        let mut cached_value = None;
        let mut proof = DenseProof::default();
        let mut scratch = PointScratch::default();
        let mut buffer = Vec::new();
        loop {
            buffer.clear();
            match reader.read_event_into(&mut buffer).unwrap() {
                Event::Start(e) => match local_name(e.name().into_inner()) {
                    b"pt" => {
                        let index = attribute_str(&e, b"idx");
                        let zero = index.as_deref() == Some("0");
                        proof.point(index.as_deref().and_then(|index| index.parse().ok()));
                        let point = read_point_into(reader, &mut scratch, zero);
                        proof.broken |= !point.flat;
                        if zero {
                            cached_value = Some(point.value.unwrap_or_default());
                        }
                    }
                    b"ptCount" => proof.count(&e),
                    b"strCache" => proof.broken = true,
                    _ => (),
                },
                Event::Empty(e) => match local_name(e.name().into_inner()) {
                    b"pt" => match try_attribute_u32(&e, b"idx") {
                        Ok(index) => proof.point(index),
                        Err(_) => proof.broken = true,
                    },
                    b"ptCount" => proof.count(&e),
                    b"strCache" => proof.broken = true,
                    _ => (),
                },
                Event::End(e) if local_name(e.name().into_inner()) == b"strCache" => {
                    return (cached_value, Self::empty_source(formula), proof.finish());
                }
                Event::Eof => panic!("Error: Could not find {} end element", "c:strCache"),
                _ => (),
            }
        }
    }

    /// Reads a `c:strCache` of the reference `formula`: the label at index
    /// zero, which series names use, and every point the file stored.
    pub(crate) fn set_attributes<R: std::io::BufRead>(
        reader: &mut Reader<R>,
        _e: &BytesStart,
        formula: String,
    ) -> (Option<String>, Self) {
        let (cached_value, source) = Self::read_source_attributes(reader, formula);
        (
            cached_value,
            Self {
                source: Some({
                    #[cfg(test)]
                    note_deferred_hydration_source_box();
                    Box::new(source)
                }),
            },
        )
    }

    pub(crate) fn read_source_attributes<R: std::io::BufRead>(
        reader: &mut Reader<R>,
        formula: String,
    ) -> (Option<String>, SourceCache) {
        let mut cached_value = None;
        let mut point_count = None;
        let mut points = Vec::new();
        xml_read_loop!(
            reader,
            Event::Start(ref e) => match local_name(e.name().into_inner()) {
                b"ptCount" => {
                    point_count = parse_point_count(e);
                }
                b"pt" => {
                    let index = get_attribute(e, b"idx");
                    let value = read_point_value(reader);
                    if index.as_deref() == Some("0") {
                        cached_value = Some(value.clone().unwrap_or_default());
                    }
                    if let Some(index) = index.and_then(|index| index.parse().ok()) {
                        points.push((index, value));
                    }
                }
                _ => (),
            },
            Event::Empty(ref e) => match local_name(e.name().into_inner()) {
                b"ptCount" => {
                    point_count = parse_point_count(e);
                }
                b"pt" => {
                    if let Some(index) =
                        get_attribute(e, b"idx").and_then(|index| index.parse().ok())
                    {
                        points.push((index, None));
                    }
                }
                _ => (),
            },
            Event::End(ref e) => {
                if local_name(e.name().0) == b"strCache" {
                    let source = SourceCache::as_read(formula, point_count, points);
                    return (cached_value, source);
                }
            },
            Event::Eof => panic!("Error: Could not find {} end element", "c:strCache")
        );
    }

    /// Writes the cache of `formula`. A source cache that was read under the
    /// same formula keeps the labels the cells cannot reproduce; otherwise
    /// the cache is generated from the cells.
    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        formula: &Formula,
        wb: &Workbook,
    ) {
        match &self.source {
            Some(source) if source.formula() == formula.address_str() => {
                Self::write_stored(writer, source, formula, wb);
            }
            _ => Self::write_generated(writer, formula, wb),
        }
    }

    fn write_generated(writer: &mut Writer<Cursor<Vec<u8>>>, formula: &Formula, wb: &Workbook) {
        let cell_value_list = if formula.has_string_value() {
            let value = formula.address_str();
            let value = match value
                .strip_prefix('"')
                .and_then(|value| value.strip_suffix('"'))
            {
                Some(value) => value.replace("\"\"", "\""),
                None => value,
            };
            vec![value]
        } else {
            let address = formula.address();
            match wb.sheet_by_name(&unescape_sheet_name(address.sheet_name())) {
                Ok(sheet) => sheet
                    .cells_crate()
                    .iter_all_cells_by_range_sorted_by_row(&address.range().range())
                    .map(|cell| cell.map_or_else(String::new, Cell::formatted_value))
                    .collect(),
                Err(_) => vec![],
            }
        };
        let coll_value_count = cell_value_list.len().to_string();
        // c:strCache
        write_start_tag(writer, "c:strCache", vec![], false);

        // c:ptCount
        write_start_tag(
            writer,
            "c:ptCount",
            vec![("val", &coll_value_count).into()],
            true,
        );

        for (idx, cell_value) in cell_value_list.into_iter().enumerate() {
            // c:pt
            write_start_tag(writer, "c:pt", vec![("idx", idx.to_string()).into()], false);

            // c:v
            write_start_tag(writer, "c:v", vec![], false);
            write_text_node(writer, cell_value);
            write_end_tag(writer, "c:v");

            write_end_tag(writer, "c:pt");
        }

        write_end_tag(writer, "c:strCache");
    }

    /// Writes the stored points in file order. A point the cells reproduce gets
    /// its current display text. A stored exception keeps the text the file
    /// had while its cell still displays what it displayed at load, and
    /// otherwise gets the current display text too.
    fn write_stored(
        writer: &mut Writer<Cursor<Vec<u8>>>,
        cache: &SourceCache,
        formula: &Formula,
        wb: &Workbook,
    ) {
        let current = DisplaySource::new(formula, wb, cache.indices());

        // c:strCache
        write_start_tag(writer, "c:strCache", vec![], false);

        // c:ptCount
        if let Some(count) = cache.point_count() {
            write_start_tag(
                writer,
                "c:ptCount",
                vec![("val", count.to_string()).into()],
                true,
            );
        }

        let mut exceptions = cache.exceptions().iter().peekable();
        for index in cache.indices() {
            let display = current.display(index);
            let value = match exceptions.next_if(|exception| exception.index() == index) {
                Some(exception) if display.as_deref() == exception.loaded_display() => {
                    exception.value().map(str::to_string)
                }
                _ => Some(display.unwrap_or_default()),
            };
            write_point(writer, &index.to_string(), value.as_deref());
        }

        write_end_tag(writer, "c:strCache");
    }
}

pub(crate) fn parse_point_count(e: &BytesStart) -> Option<u32> {
    get_attribute(e, b"val").and_then(|value| value.parse().ok())
}

/// Reads the `c:v` of a `c:pt` up to its end tag: `None` when it has none.
pub(crate) fn read_point_value<R: std::io::BufRead>(reader: &mut Reader<R>) -> Option<String> {
    let mut value = None;
    xml_read_loop!(
        reader,
        Event::Start(ref e) => {
            if local_name(e.name().into_inner()) == b"v" {
                let mut text = NumericValue::default();
                text.set_attributes(reader, e);
                value = Some(text.text().to_string());
            }
        },
        Event::Empty(ref e) => {
            if local_name(e.name().into_inner()) == b"v" {
                value = Some(String::new());
            }
        },
        Event::End(ref e) => {
            if local_name(e.name().0) == b"pt" {
                return value;
            }
        },
        Event::Eof => panic!("Error: Could not find {} end element", "c:pt")
    );
}

/// What reading a deferred cache proved about its shape: positive density
/// counted by exactly one `ptCount`. Points are at indices `0..point_count`,
/// each stored once and in order. A cache with any other shape has no summary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct DenseSummary {
    pub(crate) point_count: u32,
}

/// Accumulates the evidence for a `DenseSummary` while the cache is read.
/// Anything unusual makes the cache unproven, never an error.
#[derive(Default)]
struct DenseProof {
    ordinal:     u64,
    counts:      usize,
    point_count: Option<u32>,
    broken:      bool,
}

impl DenseProof {
    fn point(&mut self, index: Option<u32>) {
        if index.is_some_and(|index| u64::from(index) == self.ordinal) {
            self.ordinal += 1;
        } else {
            self.broken = true;
        }
    }

    fn count(&mut self, e: &BytesStart) {
        self.counts += 1;
        match try_attribute_u32(e, b"val") {
            Ok(count) => self.point_count = count,
            Err(_) => self.broken = true,
        }
    }

    fn finish(self) -> Option<DenseSummary> {
        let count = self.point_count?;
        (!self.broken && self.counts == 1 && count > 0 && u64::from(count) == self.ordinal)
            .then_some(DenseSummary { point_count: count })
    }
}

/// The buffers a scan reuses for every point, so that scanning a cache
/// allocates for what it keeps and not for each point.
#[derive(Default)]
pub(crate) struct PointScratch {
    events: Vec<u8>,
    text:   Vec<u8>,
}

#[cfg(test)]
impl PointScratch {
    pub(crate) fn text_capacity(&self) -> usize {
        self.text.capacity()
    }
}

/// A point read by `read_point_into`. `flat` is false when the point held an
/// element other than `c:v`, which makes the span the point covers
/// ambiguous between scanners.
pub(crate) struct PointRead {
    pub(crate) value: Option<String>,
    pub(crate) flat:  bool,
}

/// `read_point_value` with reusable buffers. Every `c:v` is read and checked
/// exactly as `read_point_value` does; `value` is built only when `keep` is
/// set, and is then the text `read_point_value` returns.
pub(crate) fn read_point_into<R: std::io::BufRead>(
    reader: &mut Reader<R>,
    scratch: &mut PointScratch,
    keep: bool,
) -> PointRead {
    let mut value = None;
    let mut flat = true;
    loop {
        scratch.events.clear();
        let event = match reader.read_event_into(&mut scratch.events) {
            Ok(event) => event,
            Err(e) => panic!("Error at position {}: {e:?}", reader.buffer_position()),
        };
        match event {
            Event::Start(ref e) => {
                if local_name(e.name().into_inner()) == b"v" {
                    // `read_text_into` appends to its buffer.
                    scratch.text.clear();
                    let text = reader.read_text_into(e.name(), &mut scratch.text).unwrap();
                    if keep {
                        #[cfg(test)]
                        note_work(|counts| counts.kept_values += 1);
                        value = Some(unescape_xml_text(&text));
                    } else {
                        validate_xml_text(&text);
                    }
                } else {
                    flat = false;
                }
            }
            Event::Empty(ref e) => {
                if local_name(e.name().into_inner()) == b"v" {
                    if keep {
                        value = Some(String::new());
                    }
                } else {
                    flat = false;
                }
            }
            Event::End(ref e) => {
                if local_name(e.name().0) == b"pt" {
                    return PointRead { value, flat };
                }
            }
            Event::Eof => panic!("Error: Could not find {} end element", "c:pt"),
            _ => (),
        }
    }
}

/// Visit the indices from a complete `c:strCache` XML element without
/// materializing its points. This is also the lazy raw-sheet request scanner.
pub(crate) fn visit_deferred_indices(data: &[u8], mut visit: impl FnMut(u32)) {
    let mut reader = Reader::from_reader(Cursor::new(data));
    reader.config_mut().trim_text(true);
    #[cfg(test)]
    note_work(|counts| counts.index_scans += 1);
    let mut buffer = Vec::new();
    let mut skipped = Vec::new();
    loop {
        buffer.clear();
        match reader.read_event_into(&mut buffer).unwrap() {
            Event::Start(e) if local_name(e.name().into_inner()) == b"pt" => {
                if let Some(index) = attribute_u32(&e, b"idx") {
                    visit(index);
                }
                skipped.clear();
                reader.read_to_end_into(e.name(), &mut skipped).unwrap();
            }
            Event::Empty(e) if local_name(e.name().into_inner()) == b"pt" => {
                if let Some(index) = attribute_u32(&e, b"idx") {
                    visit(index);
                }
            }
            Event::Empty(e) if local_name(e.name().into_inner()) == b"strCache" => return,
            Event::End(e) if local_name(e.name().into_inner()) == b"strCache" => return,
            Event::Eof => panic!("Error: Could not find {} end element", "c:strCache"),
            _ => (),
        }
    }
}

/// The two non-canonical image carriers retain the parser's existing,
/// unreduced representation. They are deliberately outside deferred
/// admission and report accounting.
pub(crate) fn materialize_deferred_cache(data: &[u8], formula: String) -> SourceCache {
    let mut reader = Reader::from_reader(Cursor::new(data));
    reader.config_mut().trim_text(true);
    let mut buffer = Vec::new();
    loop {
        buffer.clear();
        match reader.read_event_into(&mut buffer).unwrap() {
            Event::Start(e) if local_name(e.name().into_inner()) == b"strCache" => {
                return StringCache::read_source_attributes(&mut reader, formula).1;
            }
            Event::Empty(e) if local_name(e.name().into_inner()) == b"strCache" => {
                return SourceCache::as_read(formula, None, Vec::new());
            }
            Event::Eof => panic!("Error: Could not find {} start element", "c:strCache"),
            _ => (),
        }
    }
}

fn write_point(writer: &mut Writer<Cursor<Vec<u8>>>, index: &str, value: Option<&str>) {
    // c:pt
    write_start_tag(writer, "c:pt", vec![("idx", index).into()], false);

    // c:v
    if let Some(value) = value {
        write_start_tag(writer, "c:v", vec![], false);
        write_text_node(writer, value);
        write_end_tag(writer, "c:v");
    }

    write_end_tag(writer, "c:pt");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        drawing::charts::StringReference,
        reader::xlsx::SourceCacheSession,
    };

    fn parse(xml: &str) -> StringReference {
        let mut reader = Reader::from_reader(xml.as_bytes());
        let mut buf = Vec::new();
        let Event::Start(e) = reader.read_event_into(&mut buf).unwrap() else {
            panic!("missing string reference root");
        };
        let mut reference = StringReference::default();
        let session = SourceCacheSession::immediate();
        reference.set_attributes(&mut reader, &e, &session);
        reference
    }

    #[test]
    fn cached_name_uses_index_zero_and_decodes_entities() {
        let reference = parse(
            "<strRef><f>Missing!$A$1</f><strCache><ptCount val=\"2\"/><pt idx=\"1\"><v>Other \
             label</v></pt><pt idx=\"0\"><v>Revenue &amp; Sales</v></pt></strCache></strRef>",
        );
        assert_eq!(reference.cached_value(), Some("Revenue & Sales"));
        assert_eq!(reference.formula().address_str(), "Missing!$A$1");
    }

    #[test]
    fn cache_does_not_substitute_another_index_for_a_missing_name() {
        let reference = parse(
            "<c:strRef><c:f>Missing!$A$1</c:f><c:strCache><c:pt idx=\"1\"><c:v>Other \
             label</c:v></c:pt></c:strCache></c:strRef>",
        );
        assert_eq!(reference.cached_value(), None);
    }

    #[cfg(feature = "serde")]
    #[test]
    fn cached_name_serde_roundtrips_and_older_references_deserialize() {
        let reference = parse(
            "<strRef><f>Missing!$A$1</f><strCache><pt \
             idx=\"0\"><v>Revenue</v></pt></strCache></strRef>",
        );
        let mut value = serde_json::to_value(&reference).unwrap();
        let restored: StringReference = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(restored.cached_value(), Some("Revenue"));
        value.as_object_mut().unwrap().remove("cached_value");
        let restored: StringReference = serde_json::from_value(value).unwrap();
        assert_eq!(restored.cached_value(), None);
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

    /// A reader positioned just after the start tag of the first element.
    fn after_start(xml: &[u8], trim: bool) -> Reader<&[u8]> {
        let mut reader = Reader::from_reader(xml);
        reader.config_mut().trim_text(trim);
        let mut buf = Vec::new();
        assert!(matches!(
            reader.read_event_into(&mut buf).unwrap(),
            Event::Start(_)
        ));
        reader
    }

    fn catch<T>(operation: impl FnOnce() -> T) -> Result<T, String> {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(operation)).map_err(panic_message)
    }

    enum Expect {
        Value(Option<&'static str>),
        Panic(String),
    }

    // The value decoder returns the text `read_point_value` returns, and fails
    // with the payload it fails with, for every shape of point content. The
    // expectations are specified by hand and by the XML escaping and encoding
    // rules; the retained reader is the differential oracle for the trimming
    // reader configuration.
    #[test]
    fn point_value_decoder_matches_the_retained_reader_for_every_content_shape() {
        let long = "L".repeat(500);
        let long_then_short = format!("<c:pt idx=\"0\"><c:v>{long}</c:v><c:v>s</c:v></c:pt>");
        let cases: Vec<(&str, Vec<u8>, Expect)> = vec![
            (
                "plain",
                b"<c:pt idx=\"0\"><c:v>a</c:v></c:pt>".to_vec(),
                Expect::Value(Some("a")),
            ),
            (
                "repeated value, last wins",
                b"<c:pt idx=\"0\"><c:v>first</c:v><c:v>second</c:v></c:pt>".to_vec(),
                Expect::Value(Some("second")),
            ),
            (
                "invalid earlier value still fails",
                b"<c:pt idx=\"0\"><c:v>&bogus;</c:v><c:v>ok</c:v></c:pt>".to_vec(),
                Expect::Panic(unwrap_message(
                    &quick_xml::escape::unescape("&bogus;").unwrap_err(),
                )),
            ),
            (
                "value prefix differs from point prefix",
                b"<c:pt idx=\"0\"><x:v>val</x:v></c:pt>".to_vec(),
                Expect::Value(Some("val")),
            ),
            (
                "nested element of the value name stays in the text",
                b"<c:pt idx=\"0\"><x:v>a<x:v>b</x:v>c</x:v></c:pt>".to_vec(),
                Expect::Value(Some("a<x:v>b</x:v>c")),
            ),
            (
                "surrounding whitespace",
                b"<c:pt idx=\"0\"><c:v>  spaced  </c:v></c:pt>".to_vec(),
                Expect::Value(Some("  spaced  ")),
            ),
            (
                "unicode",
                "<c:pt idx=\"0\"><c:v>Zo\u{eb} \u{5317}\u{4eac} \u{1f600}</c:v></c:pt>"
                    .as_bytes()
                    .to_vec(),
                Expect::Value(Some("Zo\u{eb} \u{5317}\u{4eac} \u{1f600}")),
            ),
            (
                "entities",
                b"<c:pt idx=\"0\"><c:v>a &amp; b &lt;c&gt; &#65;&#x42;</c:v></c:pt>".to_vec(),
                Expect::Value(Some("a & b <c> AB")),
            ),
            (
                "raw markup inside the value",
                b"<c:pt idx=\"0\"><c:v>a<b>c</b>d</c:v></c:pt>".to_vec(),
                Expect::Value(Some("a<b>c</b>d")),
            ),
            (
                "cdata inside the value keeps its markers",
                b"<c:pt idx=\"0\"><c:v>x<![CDATA[<y>&amp;]]>z</c:v></c:pt>".to_vec(),
                Expect::Value(Some("x<![CDATA[<y>&]]>z")),
            ),
            (
                "missing value",
                b"<c:pt idx=\"0\"></c:pt>".to_vec(),
                Expect::Value(None),
            ),
            (
                "paired empty value",
                b"<c:pt idx=\"0\"><c:v></c:v></c:pt>".to_vec(),
                Expect::Value(Some("")),
            ),
            (
                "self-closing value",
                b"<c:pt idx=\"0\"><c:v/></c:pt>".to_vec(),
                Expect::Value(Some("")),
            ),
            (
                "long value then a short replacement",
                long_then_short.into_bytes(),
                Expect::Value(Some("s")),
            ),
            (
                "invalid entity",
                b"<c:pt idx=\"0\"><c:v>&bogus;</c:v></c:pt>".to_vec(),
                Expect::Panic(unwrap_message(
                    &quick_xml::escape::unescape("&bogus;").unwrap_err(),
                )),
            ),
            (
                "invalid utf-8",
                b"<c:pt idx=\"0\"><c:v>a\xFFb</c:v></c:pt>".to_vec(),
                Expect::Panic(unwrap_message(&quick_xml::encoding::EncodingError::Utf8(
                    std::str::from_utf8(std::hint::black_box(b"a\xFFb")).unwrap_err(),
                ))),
            ),
            (
                "point truncated after its value",
                b"<c:pt idx=\"0\"><c:v>a</c:v>".to_vec(),
                Expect::Panic("Error: Could not find c:pt end element".into()),
            ),
        ];
        for (name, xml, expect) in cases {
            let old = catch(|| read_point_value(&mut after_start(&xml, false)));
            let kept = catch(|| {
                read_point_into(
                    &mut after_start(&xml, false),
                    &mut PointScratch::default(),
                    true,
                )
                .value
            });
            let discarded = catch(|| {
                read_point_into(
                    &mut after_start(&xml, false),
                    &mut PointScratch::default(),
                    false,
                )
                .value
            });
            match expect {
                Expect::Value(value) => {
                    assert_eq!(
                        old,
                        Ok(value.map(str::to_string)),
                        "{name}: retained reader"
                    );
                    assert_eq!(kept, old, "{name}: kept value");
                    assert_eq!(discarded, Ok(None), "{name}: discarded value");
                }
                Expect::Panic(message) => {
                    assert_eq!(old, Err(message.clone()), "{name}: retained reader");
                    assert_eq!(kept, old, "{name}: kept failure");
                    assert_eq!(discarded, old.map(|_| None), "{name}: discarded failure");
                }
            }
            let trimmed_old = catch(|| read_point_value(&mut after_start(&xml, true)));
            let trimmed_new = catch(|| {
                read_point_into(
                    &mut after_start(&xml, true),
                    &mut PointScratch::default(),
                    true,
                )
                .value
            });
            assert_eq!(trimmed_new, trimmed_old, "{name}: trimming reader");
        }
    }

    // Failures of the point reader and of the value reader keep their exact
    // messages. Positions are byte offsets after the offending tag: the
    // points are 14 bytes of start tag, 5 of `<c:v>`, 1 of text and 6 of
    // `</c:v>` (26), followed by the 6-byte `</c:z>` (32) or 7-byte
    // `</x:pt>` (33). A value cut off by the end of input misses the end tag
    // of its own `c:v`.
    #[test]
    fn point_value_decoder_keeps_event_and_value_failures() {
        for (name, xml, message) in [
            (
                "value never closed",
                &b"<c:pt idx=\"0\"><c:v>abc"[..],
                "called `Result::unwrap()` on an `Err` value: IllFormed(MissingEndTag(\"c:v\"))",
            ),
            (
                "unmatched end event",
                &b"<c:pt idx=\"0\"><c:v>a</c:v></c:z>"[..],
                "Error at position 32: IllFormed(MismatchedEndTag { expected: \"c:pt\", found: \
                 \"c:z\" })",
            ),
            (
                "mismatched point end",
                &b"<c:pt idx=\"0\"><c:v>a</c:v></x:pt>"[..],
                "Error at position 33: IllFormed(MismatchedEndTag { expected: \"c:pt\", found: \
                 \"x:pt\" })",
            ),
        ] {
            let old = catch(|| read_point_value(&mut after_start(xml, false)));
            let new = catch(|| {
                read_point_into(
                    &mut after_start(xml, false),
                    &mut PointScratch::default(),
                    true,
                )
                .value
            });
            assert_eq!(old, Err(message.to_string()), "{name}: retained reader");
            assert_eq!(new, Err(message.to_string()), "{name}: point decoder");
        }
    }

    // The text buffer is cleared before every value, so its capacity follows
    // the longest value and not the total text of the cache.
    #[test]
    fn point_value_scratch_follows_the_longest_value_not_the_total() {
        let longest = 1500;
        let mut cache = String::from("<c:strCache>");
        for index in 0..3000 {
            let value = if index == 1000 {
                "M".repeat(longest)
            } else {
                "abcdefghij".to_string()
            };
            cache.push_str(&format!("<c:pt idx=\"{index}\"><c:v>{value}</c:v></c:pt>"));
        }
        cache.push_str("</c:strCache>");
        let mut reader = Reader::from_reader(cache.as_bytes());
        let mut buffer = Vec::new();
        let mut scratch = PointScratch::default();
        let mut values = Vec::new();
        loop {
            buffer.clear();
            match reader.read_event_into(&mut buffer).unwrap() {
                Event::Start(e) if local_name(e.name().into_inner()) == b"pt" => {
                    values.push(
                        read_point_into(&mut reader, &mut scratch, true)
                            .value
                            .unwrap(),
                    );
                }
                Event::Eof => break,
                _ => (),
            }
        }
        assert_eq!(values.len(), 3000);
        assert_eq!(values[1000], "M".repeat(longest));
        assert_eq!(values[1001], "abcdefghij");
        assert_eq!(values[2999], "abcdefghij");
        let total: usize = values.iter().map(String::len).sum();
        assert_eq!(total, 2999 * 10 + longest);
        // Growing a buffer at most doubles it past the longest value.
        assert!(scratch.text_capacity() >= longest);
        assert!(scratch.text_capacity() <= 2 * longest);
    }
}
