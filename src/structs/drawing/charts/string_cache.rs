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
    helper::address::unescape_sheet_name,
    reader::driver::{
        get_attribute,
        local_name,
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
    /// workbook has established canonical admission order.
    pub(crate) fn set_deferred_attributes<R: std::io::BufRead>(
        reader: &mut Reader<R>,
        formula: String,
    ) -> (Option<String>, Self) {
        let mut cached_value = None;
        let mut buffer = Vec::new();
        loop {
            buffer.clear();
            match reader.read_event_into(&mut buffer).unwrap() {
                Event::Start(e) if local_name(e.name().into_inner()) == b"pt" => {
                    let index = get_attribute(&e, b"idx");
                    let value = read_point_value(reader);
                    if index.as_deref() == Some("0") {
                        cached_value = Some(value.unwrap_or_default());
                    }
                }
                Event::End(e) if local_name(e.name().into_inner()) == b"strCache" => {
                    return (cached_value, Self::empty_source(formula));
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

/// Visit the indices from a complete `c:strCache` XML element without
/// materializing its points. This is also the lazy raw-sheet request scanner.
pub(crate) fn visit_deferred_indices(data: &[u8], mut visit: impl FnMut(u32)) {
    let mut reader = Reader::from_reader(Cursor::new(data));
    reader.config_mut().trim_text(true);
    let mut buffer = Vec::new();
    loop {
        buffer.clear();
        match reader.read_event_into(&mut buffer).unwrap() {
            Event::Start(e) if local_name(e.name().into_inner()) == b"pt" => {
                if let Some(index) = get_attribute(&e, b"idx").and_then(|index| index.parse().ok())
                {
                    visit(index);
                }
                let mut skipped = Vec::new();
                reader.read_to_end_into(e.name(), &mut skipped).unwrap();
            }
            Event::Empty(e) if local_name(e.name().into_inner()) == b"pt" => {
                if let Some(index) = get_attribute(&e, b"idx").and_then(|index| index.parse().ok())
                {
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
}
