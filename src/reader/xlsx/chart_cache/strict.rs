use super::{
    CacheShape,
    HashSet,
    IndexRun,
};
#[cfg(test)]
use super::{
    StateCell,
    note_work,
};

#[cfg(test)]
thread_local! {
    pub(super) static SCANS: StateCell<(usize, usize)> = const { StateCell::new((0, 0)) };
}

#[cfg(test)]
pub(super) fn note_scan(accepted: bool) {
    SCANS.with(|seen| {
        let (accepts, rejects) = seen.get();
        seen.set((
            accepts + usize::from(accepted),
            rejects + usize::from(!accepted),
        ));
    });
}

/// Every consumed byte belongs to an exact token, a canonical decimal, or a
/// value slice. The only skipped bytes are leading ASCII XML whitespace.
pub(super) struct ByteCache<'a> {
    data:     &'a [u8],
    position: usize,
    prefix:   &'a [u8],
    count:    u32,
}

impl<'a> ByteCache<'a> {
    pub(super) fn new(data: &'a [u8]) -> Option<Self> {
        let mut scan = Self {
            data,
            position: 0,
            prefix: &[],
            count: 0,
        };
        while matches!(data.get(scan.position), Some(b' ' | b'\t' | b'\r' | b'\n')) {
            scan.position += 1;
        }
        scan.take(b"<")?;
        let start = scan.position;
        let end = start + data.get(start..)?.iter().position(|&byte| byte == b'>')?;
        let name = data.get(start..end)?.strip_suffix(b"strCache")?;
        if !name.is_empty() {
            let name = name.strip_suffix(b":")?;
            if !name
                .first()
                .is_some_and(|byte| byte.is_ascii_alphabetic() || *byte == b'_')
                || !name
                    .iter()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"_.-".contains(byte))
            {
                return None;
            }
        }
        scan.prefix = &data[start..start + name.len()];
        scan.take_tag(b"", b"strCache>")?;
        scan.take(b"<")?;
        scan.take_tag(b"", b"ptCount val=\"")?;
        scan.count = scan.decimal()?;
        scan.take(b"\"/>")?;
        Some(scan)
    }

    fn take(&mut self, token: &[u8]) -> Option<()> {
        if !self.data.get(self.position..)?.starts_with(token) {
            return None;
        }
        self.position += token.len();
        Some(())
    }

    fn take_tag(&mut self, before: &[u8], after: &[u8]) -> Option<()> {
        self.take(before)?;
        self.take(self.prefix)?;
        self.take(after)
    }

    fn decimal(&mut self) -> Option<u32> {
        let start = self.position;
        let mut value = 0u32;
        while let Some(byte) = self
            .data
            .get(self.position)
            .filter(|byte| byte.is_ascii_digit())
        {
            value = value
                .checked_mul(10)?
                .checked_add(u32::from(*byte - b'0'))?;
            self.position += 1;
        }
        if self.position == start || (self.position > start + 1 && self.data[start] == b'0') {
            return None;
        }
        Some(value)
    }

    pub(super) fn next(&mut self) -> Result<Option<(u32, &'a str)>, ()> {
        self.take(b"<").ok_or(())?;
        if self.data.get(self.position) == Some(&b'/') {
            self.take_tag(b"/", b"strCache>").ok_or(())?;
            return (self.position == self.data.len()).then_some(None).ok_or(());
        }
        self.take_tag(b"", b"pt idx=\"").ok_or(())?;
        let index = self.decimal().ok_or(())?;
        self.take(b"\"><").ok_or(())?;
        self.take_tag(b"", b"v>").ok_or(())?;
        let start = self.position;
        self.position += self.data[start..]
            .iter()
            .position(|&byte| byte == b'<')
            .ok_or(())?;
        let value = std::str::from_utf8(&self.data[start..self.position]).map_err(|_| ())?;
        self.take_tag(b"</", b"v>").ok_or(())?;
        self.take_tag(b"</", b"pt>").ok_or(())?;
        Ok(Some((index, value)))
    }
}

pub(super) fn shape(data: &[u8], remaining_points: usize) -> Option<CacheShape> {
    let mut scan = ByteCache::new(data)?;
    #[cfg(test)]
    note_work(|counts| counts.shape_scans += 1);
    let mut shape = CacheShape {
        point_count:                Some(scan.count),
        index_runs:                 Some(Vec::new()),
        run_charge:                 0,
        current_run_len:            0,
        counted_dense:              true,
        repeated:                   HashSet::new(),
        #[cfg(test)]
        shape_backing:              None,
    };
    let mut count = 0;
    let mut previous = None;
    let mut ascending = true;
    while let Some((index, _)) = scan.next().ok()? {
        ascending &= previous.is_none_or(|prior| prior < index);
        shape.record_index(index, &mut count, &mut previous);
        if shape.run_charge > remaining_points {
            shape.index_runs = None;
            #[cfg(test)]
            shape.observe_shape_rejection();
        }
    }
    shape.counted_dense &= shape.point_count.map(u64::from) == Some(count);
    if shape.counted_dense {
        shape.index_runs = Some(Vec::new());
        shape.run_charge = 0;
    } else if count == 0 && scan.count > 0 {
        shape.run_charge = 1;
        if remaining_points == 0 {
            shape.index_runs = None;
            #[cfg(test)]
            shape.observe_shape_rejection();
        } else {
            shape.index_runs = Some(vec![IndexRun::default()]);
        }
    }
    if !ascending {
        #[cfg(test)]
        note_work(|counts| counts.index_scans += 1);
        let mut seen = HashSet::new();
        let mut scan = ByteCache::new(data)?;
        while let Some((index, _)) = scan.next().ok()? {
            if !seen.insert(index) {
                shape.repeated.insert(index);
            }
        }
    }
    Some(shape)
}
