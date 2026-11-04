use super::{
    CacheDisplay,
    Cell,
    Coordinate,
    DeferredSource,
    DisplayResolver,
    HashMap,
    IndexedRange,
    PreparedFormats,
    RAW_SHEET,
    Request,
    ResolvedCells,
    Workbook,
};

const RANGE_BYTES: usize = 1024 * 1024;
const PART_BYTES: usize = 4 * RANGE_BYTES;
const MAX_RANGES: usize = 256;

enum Buffer<'a> {
    Cells(Vec<Option<&'a Cell>>),
    Strings(Vec<Option<&'a String>>),
}

impl Buffer<'_> {
    fn bytes(&self) -> Option<usize> {
        match self {
            Self::Cells(slots) => slots.capacity().checked_mul(size_of::<Option<&Cell>>()),
            Self::Strings(slots) => slots.capacity().checked_mul(size_of::<Option<&String>>()),
        }
    }
}

struct RangeEntry<'a> {
    sheet:  usize,
    range:  IndexedRange,
    buffer: Buffer<'a>,
}

/// References are owned only by this part's table. Length is the covered
/// prefix; a missing cell inside that prefix is distinct from no coverage.
#[derive(Default)]
pub(super) struct RangeSlots<'a> {
    part:   Option<usize>,
    ranges: Vec<RangeEntry<'a>>,
}

impl<'a> RangeSlots<'a> {
    pub(super) fn begin_part(&mut self, part: usize) {
        if self.part != Some(part) {
            self.ranges = Vec::new();
            self.part = Some(part);
        }
    }

    fn bytes(&self) -> Option<usize> {
        self.ranges.iter().try_fold(
            self.ranges
                .capacity()
                .checked_mul(size_of::<RangeEntry<'_>>())?,
            |total, entry| total.checked_add(entry.buffer.bytes()?),
        )
    }

    pub(super) fn cover_source(
        &mut self,
        source: &DeferredSource,
        request: &Request,
        book: &'a Workbook,
        classes: &[u8],
        streamed: &'a HashMap<usize, HashMap<Coordinate, String>>,
    ) -> Option<usize> {
        self.begin_part(source.part);
        let count = source.summary?.point_count;
        self.cover(request, book, classes, streamed, count)
    }

    pub(super) fn cover(
        &mut self,
        request: &Request,
        book: &'a Workbook,
        classes: &[u8],
        streamed: &'a HashMap<usize, HashMap<Coordinate, String>>,
        count: u32,
    ) -> Option<usize> {
        let sheet = request.sheet?;
        let range = request.range.as_ref()?;
        let source = if classes[sheet] & RAW_SHEET != 0 {
            ResolvedCells::Streamed(streamed.get(&sheet)?)
        } else {
            ResolvedCells::Materialized(book.sheet_collection_no_check()[sheet].cells_crate())
        };
        let count = usize::try_from(range.len().min(u64::from(count))).ok()?;
        let existing = self
            .ranges
            .iter()
            .position(|entry| entry.sheet == sheet && entry.range == *range);
        let index = if let Some(index) = existing {
            index
        } else {
            if self.ranges.len() >= MAX_RANGES
                || count == 0
                || count.checked_mul(size_of::<Option<&Cell>>())? > RANGE_BYTES
            {
                return None;
            }
            if self.ranges.len() == self.ranges.capacity() {
                let capacity = self.ranges.len().checked_add(1)?;
                let requested = capacity.checked_mul(size_of::<RangeEntry<'_>>())?;
                let total = self.bytes()?;
                if total.checked_add(requested)? > PART_BYTES {
                    return None;
                }
                let mut next = Vec::new();
                if next.try_reserve_exact(capacity).is_err() {
                    return None;
                }
                let actual = next.capacity().checked_mul(size_of::<RangeEntry<'_>>())?;
                if total.checked_add(actual)? > PART_BYTES {
                    return None;
                }
                next.append(&mut self.ranges);
                self.ranges = next;
            }
            let buffer = match source {
                ResolvedCells::Materialized(_) => Buffer::Cells(Vec::new()),
                ResolvedCells::Streamed(_) => Buffer::Strings(Vec::new()),
            };
            self.ranges.push(RangeEntry {
                sheet,
                range: range.clone(),
                buffer,
            });
            self.ranges.len() - 1
        };
        let total = self.bytes()?;
        let mut stepper = range.stepper();
        match (&mut self.ranges[index].buffer, source) {
            (Buffer::Cells(slots), ResolvedCells::Materialized(cells)) => {
                grow(slots, count, total, |point| {
                    cells.get(stepper.coordinate(point)?)
                });
            }
            (Buffer::Strings(slots), ResolvedCells::Streamed(values)) => {
                grow(slots, count, total, |point| {
                    values.get(&stepper.coordinate(point)?)
                });
            }
            _ => return None,
        }
        Some(index)
    }

    pub(super) fn display<'b>(
        &self,
        slot: Option<usize>,
        resolver: &mut DisplayResolver<'b>,
        point: u32,
        scratch: &mut String,
        formats: &mut PreparedFormats,
    ) -> Option<CacheDisplay<'b>>
    where
        'a: 'b,
    {
        if let Some(entry) = slot.and_then(|index| self.ranges.get(index)) {
            match &entry.buffer {
                Buffer::Cells(slots) => {
                    if let Some(cell) = slots.get(point as usize) {
                        return Some(cell.map_or(CacheDisplay::Borrowed(""), |cell| {
                            cell.cache_display_prepared(scratch, formats)
                        }));
                    }
                }
                Buffer::Strings(slots) => {
                    if let Some(value) = slots.get(point as usize) {
                        return Some(CacheDisplay::Borrowed(value.map_or("", String::as_str)));
                    }
                }
            }
        }
        resolver.display(point, scratch, formats)
    }
}

/// A fresh buffer keeps the old one intact on every rejected growth, and
/// preflight includes both buffers in the part's transient allocation peak.
fn grow<'a, T>(
    slots: &mut Vec<Option<&'a T>>,
    count: usize,
    total: usize,
    mut lookup: impl FnMut(u32) -> Option<&'a T>,
) {
    if count <= slots.len() {
        return;
    }
    let (Ok(start), Ok(end)) = (u32::try_from(slots.len()), u32::try_from(count)) else {
        return;
    };
    let Some(requested) = count.checked_mul(size_of::<Option<&T>>()) else {
        return;
    };
    if requested > RANGE_BYTES
        || total
            .checked_add(requested)
            .is_none_or(|bytes| bytes > PART_BYTES)
    {
        return;
    }
    let mut next = Vec::new();
    if next.try_reserve_exact(count).is_err() {
        return;
    }
    let Some(actual) = next.capacity().checked_mul(size_of::<Option<&T>>()) else {
        return;
    };
    if actual > RANGE_BYTES
        || total
            .checked_add(actual)
            .is_none_or(|bytes| bytes > PART_BYTES)
    {
        return;
    }
    next.extend_from_slice(slots);
    for point in start..end {
        next.push(lookup(point));
    }
    *slots = next;
}

#[cfg(test)]
#[path = "tests/range_slots.rs"]
mod tests;
