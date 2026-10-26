use super::{
    super::*,
    *,
};

fn request(address: &str) -> Request {
    let mut reference = StringReference::default();
    reference.formula_mut().set_address_str(address);
    Request::new(&reference, &HashMap::from([(String::from("Sheet1"), 0)]))
}

fn slot_bytes(slots: &RangeSlots<'_>, index: usize) -> usize {
    slots.ranges[index].buffer.bytes().unwrap()
}

fn coverage(slots: &RangeSlots<'_>, index: usize) -> usize {
    match &slots.ranges[index].buffer {
        Buffer::Cells(values) => values.len(),
        Buffer::Strings(values) => values.len(),
    }
}

fn assert_displays(
    slots: &RangeSlots<'_>,
    slot: Option<usize>,
    request: &Request,
    book: &Workbook,
    classes: &[u8],
    streamed: &HashMap<usize, HashMap<Coordinate, String>>,
    points: &[u32],
    expected: &[Option<&str>],
) {
    let mut cached = request.resolve(book, classes, streamed);
    let mut direct = request.resolve(book, classes, streamed);
    let mut scratch = String::new();
    let mut direct_scratch = String::new();
    let mut formats = PreparedFormats::default();
    let mut direct_formats = PreparedFormats::default();
    for (&point, &expected) in points.iter().zip(expected) {
        let actual = slots.display(slot, &mut cached, point, &mut scratch, &mut formats);
        let direct = direct.display(point, &mut direct_scratch, &mut direct_formats);
        assert_eq!(actual.as_ref().map(|value| value.text(&scratch)), expected);
        assert_eq!(
            actual.as_ref().map(|value| value.text(&scratch)),
            direct.as_ref().map(|value| value.text(&direct_scratch)),
        );
    }
}

// A whole-column reference must be sized by the proven cache count, while
// an uncovered large point still returns its actual cell through lookup.
#[test]
fn whole_column_one_point_cache_uses_one_slot() {
    let mut book = crate::new_file();
    book.sheet_mut(0)
        .unwrap()
        .cell_mut("A1")
        .set_value_string("first");
    book.sheet_mut(0)
        .unwrap()
        .cell_mut("A1000000")
        .set_value_string("last");
    let request = request("Sheet1!A1:A1048576");
    let streamed = HashMap::new();
    let mut slots = RangeSlots::default();
    slots.begin_part(0);
    let (index, seen) = crate::test_alloc::observe(|| {
        slots
            .cover(&request, &book, &[FRESH_SHEET], &streamed, 1)
            .unwrap()
    });
    assert_eq!(coverage(&slots, index), 1);
    assert_eq!(slot_bytes(&slots, index), size_of::<Option<&Cell>>());
    assert_eq!(
        seen.live_bytes,
        isize::try_from(slots.bytes().unwrap()).unwrap()
    );
    assert!(seen.peak_bytes <= PART_BYTES);
    assert_displays(
        &slots,
        Some(index),
        &request,
        &book,
        &[FRESH_SHEET],
        &streamed,
        &[0, 999999, 1048576],
        &[Some("first"), Some("last"), None],
    );
}

// Fresh exact reservations must survive a small Vec growth boundary and
// shrinking requests must keep the covered prefix, on eager and lazy cells.
#[test]
fn growing_and_shorter_caches_preserve_covered_and_uncovered_displays() {
    let mut book = crate::new_file();
    book.sheet_mut(0)
        .unwrap()
        .cell_mut("A1")
        .set_value_string("first");
    book.sheet_mut(0)
        .unwrap()
        .cell_mut("A5")
        .set_value_string("fifth");
    let numeric = book.sheet_mut(0).unwrap().cell_mut("A9");
    numeric.set_value_number(0.5);
    numeric
        .style_mut()
        .number_format_mut()
        .set_format_code("0.00");
    let formatted = numeric.formatted_value();
    let request = request("Sheet1!A1:A10");
    let streamed = HashMap::from([(
        0,
        HashMap::from([
            ((1, 1), String::from("first")),
            ((1, 5), String::from("fifth")),
            ((1, 9), formatted.clone()),
        ]),
    )]);
    for class in [FRESH_SHEET, RAW_SHEET] {
        let mut slots = RangeSlots::default();
        slots.begin_part(0);
        let ((index, retained), seen) = crate::test_alloc::observe(|| {
            let index = slots
                .cover(&request, &book, &[class], &streamed, 3)
                .unwrap();
            let old = slot_bytes(&slots, index);
            assert_eq!(
                slots.cover(&request, &book, &[class], &streamed, 5),
                Some(index)
            );
            let grown = slot_bytes(&slots, index);
            assert_eq!(old, 3 * size_of::<Option<&Cell>>());
            assert_eq!(grown, 5 * size_of::<Option<&Cell>>());
            assert_eq!(
                slots.cover(&request, &book, &[class], &streamed, 2),
                Some(index)
            );
            assert_eq!(coverage(&slots, index), 5);
            (index, slots.bytes().unwrap())
        });
        assert_eq!(seen.live_bytes, isize::try_from(retained).unwrap());
        assert_eq!(seen.peak_bytes, retained + 3 * size_of::<Option<&Cell>>());
        assert_eq!(seen.realloc_to_bytes, 0);
        assert!(slot_bytes(&slots, index) <= RANGE_BYTES);
        assert_displays(
            &slots,
            Some(index),
            &request,
            &book,
            &[class],
            &streamed,
            &[0, 1, 4, 8, 10],
            &[
                Some("first"),
                Some(""),
                Some("fifth"),
                Some(&formatted),
                None,
            ],
        );
        // Cover the formatted cell too: both the slots and fallback must
        // agree with the public formatted_value() result.
        slots.cover(&request, &book, &[class], &streamed, 9);
        assert_displays(
            &slots,
            Some(index),
            &request,
            &book,
            &[class],
            &streamed,
            &[8],
            &[Some(&formatted)],
        );
        let retained = slots.bytes().unwrap();
        let ((), released) =
            crate::test_alloc::observe_with_live_bytes(retained, || slots.begin_part(1));
        assert_eq!(released.live_bytes, 0);
        assert_eq!(released.deallocated_bytes, retained);
    }
}

// The exact per-range capacity must fit on every pointer width. One more
// slot must allocate nothing and preserve a nonempty covered cell.
#[test]
fn range_limit_refuses_growth_without_changing_storage_or_displays() {
    let max = u32::try_from(RANGE_BYTES / size_of::<Option<&Cell>>()).unwrap();
    let initial = max * 3 / 4;
    let mut book = crate::new_file();
    book.sheet_mut(0)
        .unwrap()
        .cell_mut("A50000")
        .set_value_string("covered");
    book.sheet_mut(0)
        .unwrap()
        .cell_mut((1, max + 1))
        .set_value_string("beyond");
    let request = request("Sheet1!A1:A1048576");
    let streamed = HashMap::from([(
        0,
        HashMap::from([
            ((1, 50000), String::from("covered")),
            ((1, max + 1), String::from("beyond")),
        ]),
    )]);
    for class in [FRESH_SHEET, RAW_SHEET] {
        let mut slots = RangeSlots::default();
        let index = slots
            .cover(&request, &book, &[class], &streamed, initial)
            .unwrap();
        slots.cover(&request, &book, &[class], &streamed, max);
        assert_eq!(coverage(&slots, index), usize::try_from(max).unwrap());
        assert_eq!(slot_bytes(&slots, index), RANGE_BYTES);
        let before = slots.bytes().unwrap();
        let old = slot_bytes(&slots, index);
        let (result, seen) = crate::test_alloc::observe_with_live_bytes(before, || {
            slots.cover(&request, &book, &[class], &streamed, max + 1)
        });
        assert_eq!(result, Some(index));
        assert_eq!(seen.total_bytes(), 0);
        assert_eq!(seen.deallocated_bytes, 0);
        assert_eq!(seen.live_bytes, isize::try_from(before).unwrap());
        assert_eq!(slot_bytes(&slots, index), old);
        assert_eq!(coverage(&slots, index), usize::try_from(max).unwrap());
        assert_displays(
            &slots,
            Some(index),
            &request,
            &book,
            &[class],
            &streamed,
            &[49999, max - 1, max],
            &[Some("covered"), Some(""), Some("beyond")],
        );
    }
}

// Four buffers using three quarters of the per-range limit leave less than
// a fresh full buffer free. Transient growth must preserve covered contents
// and later indices must still resolve directly, on every pointer width.
#[test]
fn part_limit_counts_table_storage_and_transient_growth() {
    let max = u32::try_from(RANGE_BYTES / size_of::<Option<&Cell>>()).unwrap();
    let initial = max * 3 / 4;
    let mut book = crate::new_file();
    book.sheet_mut(0)
        .unwrap()
        .cell_mut((1, max))
        .set_value_string("beyond");
    book.sheet_mut(0)
        .unwrap()
        .cell_mut("A50000")
        .set_value_string("covered");
    let requests: Vec<_> = (0..24)
        .map(|col| {
            let col = char::from(b'A' + col);
            request(&format!("Sheet1!{col}1:{col}1048576"))
        })
        .collect();
    let streamed = HashMap::from([(
        0,
        HashMap::from([
            ((1, 50000), String::from("covered")),
            ((1, max), String::from("beyond")),
        ]),
    )]);
    for class in [FRESH_SHEET, RAW_SHEET] {
        let mut slots = RangeSlots::default();
        let ((), first) = crate::test_alloc::observe(|| {
            for request in &requests[..4] {
                slots
                    .cover(request, &book, &[class], &streamed, initial)
                    .unwrap();
            }
        });
        assert_eq!(
            first.live_bytes,
            isize::try_from(slots.bytes().unwrap()).unwrap()
        );
        assert!(first.peak_bytes <= PART_BYTES);
        let old = slot_bytes(&slots, 0);
        let total = slots.bytes().unwrap();
        let (_, refused) = crate::test_alloc::observe_with_live_bytes(total, || {
            slots.cover(&requests[0], &book, &[class], &streamed, max)
        });
        assert_eq!(refused.total_bytes(), 0);
        assert_eq!(refused.deallocated_bytes, 0);
        assert_eq!(slot_bytes(&slots, 0), old);
        assert_eq!(refused.live_bytes, isize::try_from(total).unwrap());
        assert_displays(
            &slots,
            Some(0),
            &requests[0],
            &book,
            &[class],
            &streamed,
            &[49999, max - 1],
            &[Some("covered"), Some("beyond")],
        );
        let (_, many) = crate::test_alloc::observe_with_live_bytes(total, || {
            for request in &requests[4..] {
                slots.cover(request, &book, &[class], &streamed, initial);
            }
        });
        assert_eq!(
            many.live_bytes,
            isize::try_from(slots.bytes().unwrap()).unwrap()
        );
        assert!(many.peak_bytes <= PART_BYTES);
        assert!(
            slots
                .ranges
                .iter()
                .all(|entry| entry.buffer.bytes().unwrap() <= RANGE_BYTES)
        );
        for (index, request) in requests.iter().enumerate() {
            let slot = slots
                .ranges
                .iter()
                .position(|entry| entry.range == *request.range.as_ref().unwrap());
            assert_displays(
                &slots,
                slot,
                request,
                &book,
                &[class],
                &streamed,
                &[max - 1],
                &[Some(if index == 0 { "beyond" } else { "" })],
            );
        }
    }
}

// A full part must refuse the table's fresh reservation before allocating,
// while retaining nonempty covered slots and resolving the new range directly.
#[test]
fn part_limit_refuses_another_range_without_allocating_table_storage() {
    let pointer_bytes = size_of::<Option<&Cell>>();
    let max = RANGE_BYTES / pointer_bytes;
    let table_bytes = 4 * size_of::<RangeEntry<'_>>();
    let last = max - table_bytes.div_ceil(pointer_bytes);
    let mut book = crate::new_file();
    book.sheet_mut(0)
        .unwrap()
        .cell_mut("A50000")
        .set_value_string("covered");
    book.sheet_mut(0)
        .unwrap()
        .cell_mut("E1")
        .set_value_string("direct");
    let requests: Vec<_> = ['A', 'B', 'C', 'D', 'E']
        .into_iter()
        .map(|col| request(&format!("Sheet1!{col}1:{col}1048576")))
        .collect();
    let streamed = HashMap::from([(
        0,
        HashMap::from([
            ((1, 50000), String::from("covered")),
            ((5, 1), String::from("direct")),
        ]),
    )]);
    for class in [FRESH_SHEET, RAW_SHEET] {
        let mut slots = RangeSlots::default();
        for (index, request) in requests[..4].iter().enumerate() {
            let count = u32::try_from(if index == 3 { last } else { max }).unwrap();
            slots
                .cover(request, &book, &[class], &streamed, count)
                .unwrap();
            assert_eq!(coverage(&slots, index), usize::try_from(count).unwrap());
        }
        let before = slots.bytes().unwrap();
        assert!(PART_BYTES - before < pointer_bytes);
        assert!(before + 5 * size_of::<RangeEntry<'_>>() > PART_BYTES);
        let (slot, seen) = crate::test_alloc::observe_with_live_bytes(before, || {
            slots.cover(&requests[4], &book, &[class], &streamed, 1)
        });
        assert!(slot.is_none());
        assert_eq!(slots.ranges.len(), 4);
        assert_eq!(seen.total_bytes(), 0);
        assert_eq!(seen.deallocated_bytes, 0);
        assert_eq!(seen.live_bytes, isize::try_from(before).unwrap());
        assert_displays(
            &slots,
            Some(0),
            &requests[0],
            &book,
            &[class],
            &streamed,
            &[49999],
            &[Some("covered")],
        );
        assert_displays(
            &slots,
            slot,
            &requests[4],
            &book,
            &[class],
            &streamed,
            &[0],
            &[Some("direct")],
        );
    }
}

// Many distinct dense ranges must stop adding table storage at the entry
// limit; ranges beyond it must retain exactly the direct lookup displays.
#[test]
fn ranges_beyond_the_entry_limit_resolve_without_allocating_slots() {
    let mut book = crate::new_file();
    let limit = u32::try_from(MAX_RANGES).unwrap();
    book.sheet_mut(0)
        .unwrap()
        .cell_mut((1, limit + 1))
        .set_value_string("beyond");
    let requests: Vec<_> = (1..=limit + 3)
        .map(|row| request(&format!("Sheet1!A{row}:A{}", row + 1)))
        .collect();
    let streamed = HashMap::from([(0, HashMap::from([((1, limit + 1), String::from("beyond"))]))]);
    for class in [FRESH_SHEET, RAW_SHEET] {
        let mut slots = RangeSlots::default();
        for request in &requests[..MAX_RANGES] {
            slots.cover(request, &book, &[class], &streamed, 1).unwrap();
        }
        let before = slots.bytes().unwrap();
        let ((), seen) = crate::test_alloc::observe_with_live_bytes(before, || {
            for request in &requests[MAX_RANGES..] {
                assert!(
                    slots
                        .cover(request, &book, &[class], &streamed, 1)
                        .is_none()
                );
            }
        });
        assert_eq!(slots.ranges.len(), MAX_RANGES);
        assert_eq!(slots.bytes(), Some(before));
        assert_eq!(seen.total_bytes(), 0);
        assert_eq!(seen.deallocated_bytes, 0);
        assert_eq!(seen.live_bytes, isize::try_from(before).unwrap());
        for (index, request) in requests[MAX_RANGES..].iter().enumerate() {
            assert_displays(
                &slots,
                None,
                request,
                &book,
                &[class],
                &streamed,
                &[0, 1, 2],
                &[Some(if index == 0 { "beyond" } else { "" }), Some(""), None],
            );
        }
    }
}

// Slot fallback must preserve the single-point literal and unavailable
// semantics even when no range can be covered.
#[test]
fn literals_and_unresolved_requests_keep_their_displays_without_slots() {
    let book = crate::new_file();
    let streamed = HashMap::new();
    let mut slots = RangeSlots::default();
    for (address, expected) in [
        ("\"literal\"", [Some("literal"), None]),
        ("Missing!A1:A2", [None, None]),
    ] {
        let request = request(address);
        let slot = slots.cover(&request, &book, &[FRESH_SHEET], &streamed, 2);
        assert!(slot.is_none());
        assert_displays(
            &slots,
            slot,
            &request,
            &book,
            &[FRESH_SHEET],
            &streamed,
            &[0, 1],
            &expected,
        );
    }
}

// Filling a grown rectangle must resume at the old prefix and wrap rows
// exactly as direct lookup does, including nonsequential display requests.
#[test]
fn rectangular_growth_preserves_row_order_and_existing_slots() {
    let mut book = crate::new_file();
    let mut values = HashMap::new();
    for (col, row, text) in [
        (2, 2, "first"),
        (3, 2, "second"),
        (2, 3, "next"),
        (4, 5, "last"),
    ] {
        book.sheet_mut(0)
            .unwrap()
            .cell_mut((col, row))
            .set_value_string(text);
        values.insert((col, row), text.to_string());
    }
    let streamed = HashMap::from([(0, values)]);
    let request = request("Sheet1!B2:D5");
    for class in [FRESH_SHEET, RAW_SHEET] {
        let mut slots = RangeSlots::default();
        for count in [2, 7, 12] {
            let index = slots
                .cover(&request, &book, &[class], &streamed, count)
                .unwrap();
            assert_displays(
                &slots,
                Some(index),
                &request,
                &book,
                &[class],
                &streamed,
                &[0, 1, 3, 11, 2, 12],
                &[
                    Some("first"),
                    Some("second"),
                    Some("next"),
                    Some("last"),
                    Some(""),
                    None,
                ],
            );
        }
    }
}

// Pass 1 must prove dense indices before any slot allocation. A sparse large
// point must resolve directly even when a dense occurrence covered this range.
#[test]
fn sparse_large_indices_bypass_range_slots() {
    let mut book = crate::new_file();
    book.sheet_mut(0)
        .unwrap()
        .cell_mut("A1000000")
        .set_value_string("far");
    let session = SourceCacheSession::deferred();
    let xml = "<c:strRef><c:f>Sheet1!A1:A1048576</c:f><c:strCache><c:ptCount val=\"1\"/><c:pt \
               idx=\"999999\"><c:v>far</c:v></c:pt></c:strCache></c:strRef>";
    session.begin_chart_part("sparse.xml");
    session.store_part(session.part_for("sparse.xml"), xml.as_bytes().into());
    let mut reader = Reader::from_reader(xml.as_bytes());
    let mut buffer = Vec::new();
    let Event::Start(root) = reader.read_event_into(&mut buffer).unwrap() else {
        panic!("missing reference")
    };
    let mut reference = StringReference::default();
    reference.set_attributes(&mut reader, &root, &session);
    session.end_chart_part();
    let request = request("Sheet1!A1:A1048576");
    let streamed = HashMap::new();
    let CapturedSource::Deferred(source) = session.take_for_capture(&mut reference).unwrap() else {
        panic!("missing deferred source")
    };
    assert!(source.summary.is_none());
    let mut slots = RangeSlots::default();
    let (index, seen) = crate::test_alloc::observe(|| {
        slots.cover_source(&source, &request, &book, &[FRESH_SHEET], &streamed)
    });
    assert!(index.is_none());
    assert_eq!(seen.total_bytes(), 0);
    assert_eq!(seen.live_bytes, 0);
    assert_displays(
        &slots,
        index,
        &request,
        &book,
        &[FRESH_SHEET],
        &streamed,
        &[999999],
        &[Some("far")],
    );
    let mut resolver = request.resolve(&book, &[FRESH_SHEET], &streamed);
    let mut formats = PreparedFormats::default();
    let reduced = reduce_deferred_cache(
        xml.as_bytes().get(source.range().unwrap()).unwrap(),
        &source,
        request.expected_points(),
        false,
        usize::MAX,
        usize::MAX,
        |point, scratch| slots.display(index, &mut resolver, point, scratch, &mut formats),
    );
    // The one sparse run costs one point and no text. Missing the far cell
    // would add a far/empty exception and text bytes instead.
    assert!(matches!(
        reduced,
        DeferredReduction::Candidate {
            points: 1,
            text_bytes: 0,
            ..
        }
    ));
}

// In-place growth could double a three-quarter-limit buffer beyond 1 MiB.
// Fresh growth to fifteen sixteenths must retain exactly the requested bytes.
#[test]
fn large_growth_keeps_capacity_and_transient_bytes_within_limits() {
    let max = u32::try_from(RANGE_BYTES / size_of::<Option<&Cell>>()).unwrap();
    let initial = max * 3 / 4;
    let grown = max * 15 / 16;
    let mut book = crate::new_file();
    book.sheet_mut(0)
        .unwrap()
        .cell_mut((1, grown))
        .set_value_string("edge");
    let request = request("Sheet1!A1:A1048576");
    let streamed = HashMap::from([(0, HashMap::from([((1, grown), String::from("edge"))]))]);
    for class in [FRESH_SHEET, RAW_SHEET] {
        let mut slots = RangeSlots::default();
        let index = slots
            .cover(&request, &book, &[class], &streamed, initial)
            .unwrap();
        let before = slots.bytes().unwrap();
        let old = slot_bytes(&slots, index);
        let (_, seen) = crate::test_alloc::observe_with_live_bytes(before, || {
            slots.cover(&request, &book, &[class], &streamed, grown)
        });
        let requested = usize::try_from(grown).unwrap() * size_of::<Option<&Cell>>();
        assert_eq!(seen.allocated_bytes, requested);
        assert_eq!(seen.realloc_to_bytes, 0);
        assert_eq!(seen.deallocated_bytes, old);
        assert_eq!(slot_bytes(&slots, index), requested);
        assert!(slot_bytes(&slots, index) <= RANGE_BYTES);
        assert_eq!(
            seen.live_bytes,
            isize::try_from(slots.bytes().unwrap()).unwrap()
        );
        assert_eq!(seen.peak_bytes, before + requested);
        assert!(seen.peak_bytes <= PART_BYTES);
        assert_displays(
            &slots,
            Some(index),
            &request,
            &book,
            &[class],
            &streamed,
            &[initial - 1, grown - 1, max],
            &[Some(""), Some("edge"), Some("")],
        );
    }
}
