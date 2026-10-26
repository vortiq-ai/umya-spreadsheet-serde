use super::*;

const FORMULA: &str = "Sheet1!A1";
const EQUAL: &str =
    "<c:strCache><c:ptCount val=\"1\"/><c:pt idx=\"0\"><c:v>1</c:v></c:pt></c:strCache>";
const STALE: &str =
    "<c:strCache><c:ptCount val=\"1\"/><c:pt idx=\"0\"><c:v>x</c:v></c:pt></c:strCache>";
const EMPTY: &str =
    "<c:strCache><c:ptCount val=\"1\"/><c:pt idx=\"0\"><c:v></c:v></c:pt></c:strCache>";

fn book_with_one() -> Workbook {
    let mut book = crate::new_file();
    book.sheet_mut(0)
        .unwrap()
        .cell_mut("A1")
        .set_value_string("1");
    book
}

// Parse every descriptor from its own actual offset in one shared part.
fn parse_part(
    session: &SourceCacheSession,
    target: &str,
    references: &[(&str, &str)],
) -> Vec<StringReference> {
    let mut xml = String::from("<root>");
    for (formula, cache) in references {
        xml.push_str(&format!("<c:strRef><c:f>{formula}</c:f>{cache}</c:strRef>"));
    }
    xml.push_str("</root>");
    session.begin_chart_part(target);
    session.store_part(session.part_for(target), xml.as_bytes().into());
    let mut reader = Reader::from_reader(xml.as_bytes());
    let mut buffer = Vec::new();
    let mut parsed = Vec::new();
    loop {
        buffer.clear();
        match reader.read_event_into(&mut buffer).unwrap() {
            Event::Start(root) if local_name(root.name().into_inner()) == b"strRef" => {
                let mut reference = StringReference::default();
                reference.set_attributes(&mut reader, &root, session);
                parsed.push(reference);
            }
            Event::Eof => break,
            _ => (),
        }
    }
    session.end_chart_part();
    parsed
}

pub(super) fn add_part(
    book: &mut Workbook,
    session: &SourceCacheSession,
    target: &str,
    references: &[(&str, &str)],
) {
    for reference in parse_part(session, target, references) {
        book.sheet_mut(0)
            .unwrap()
            .add_chart(chart_with_reference(reference));
    }
}

fn measured_provider() -> LazyChartPartProvider {
    let mut provider = lazy_provider();
    provider.memo.measure_allocations = true;
    provider
}

pub(super) fn capture(
    book: &mut Workbook,
    session: &mut SourceCacheSession,
    provider: &mut dyn ChartPartProvider,
) -> crate::drawing::charts::WorkCounts {
    note_work(|counts| *counts = crate::drawing::charts::WorkCounts::default());
    capture_chart_source_caches(book, &[0], session, provider);
    crate::drawing::charts::work_counts()
}

fn sources(book: &Workbook) -> Vec<Option<SourceCache>> {
    let mut sources = Vec::new();
    for chart in book.sheet_collection_no_check()[0].chart_collection() {
        visit_string_references(&mut chart.chart_space().clone(), &mut |_, reference| {
            sources.push(reference.string_cache().source().cloned());
        });
    }
    sources
}

fn assert_source(source: &Option<SourceCache>, formula: &str, expected: Facts) {
    let source = source.as_ref().expect("kept source");
    assert_eq!(source.formula(), formula);
    assert_eq!(facts_of(source), expected);
}

fn assert_rejections(session: &SourceCacheSession, zero_points: bool, rejected: bool) {
    assert_eq!(
        session.rejection_backing().shape,
        zero_points.then_some(CandidateBacking {
            index_runs: None,
            exceptions: None,
        })
    );
    assert_eq!(
        session.rejection_backing().exception,
        rejected.then_some(CandidateBacking {
            index_runs: None,
            exceptions: Some((0, 0)),
        })
    );
}

fn assert_memo_bounds(provider: &LazyChartPartProvider) {
    let seen = &provider.memo.observation;
    // These are allocator bytes, including the moved descriptor's capacity,
    // table allocation, vector backing and both nested exception strings.
    assert!(seen.measured_insertions > 0);
    assert!(seen.entries > 0 && seen.entries <= MAX_MEMO_ENTRIES);
    assert!(
        seen.allocated_peak <= MAX_MEMO_BYTES,
        "{}",
        seen.allocated_peak
    );
    assert!(provider.memo.entries.is_empty());
    assert_eq!(provider.memo.entries.capacity(), 0);
    assert_eq!(provider.memo.bytes, 0);
}

fn raw_data(book: &mut Workbook, name: &str) {
    // Imported sheet names and formulas may be arbitrarily long. Construct
    // that reader state without applying the public new-sheet name validator.
    Workbook::add_new_sheet_crate(book, "2", name);
    let mut raw = RawWorksheet::default();
    raw.worksheet_file_mut().set_file_data(
        b"<worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\"><sheetData/></worksheet>",
    );
    book.sheet_collection_unchecked_mut()[1].set_raw_data_of_worksheet(raw);
}

// Repeated dense reductions must charge and store every occurrence while doing
// the decode work only once, for both eliminated and kept outcomes.
#[test]
fn identical_dense_caches_share_work_and_match_fresh_capture() {
    for (cache, report, kept) in [
        (EQUAL, (0, 0, 0, 2, 2, 0, 0, 0), false),
        (STALE, (2, 2, 4, 0, 0, 0, 0, 0), true),
    ] {
        let mut book = book_with_one();
        let mut session = SourceCacheSession::deferred();
        add_part(&mut book, &session, "same.xml", &[(FORMULA, cache); 2]);
        let mut provider = measured_provider();
        let work = capture(&mut book, &mut session, &mut provider);
        assert_eq!(
            (work.shape_scans, work.decode_scans, work.index_scans),
            (0, 1, 0)
        );
        assert_eq!(work.kept_values, 1);
        assert_eq!(provider.memo.observation.hits, 1);
        // Equal labels eliminate two caches. Each stale occurrence stores one
        // exception, with one byte each for value "x" and loaded display "1".
        assert_eq!(report_of(&book), report);
        let stored = sources(&book);
        assert_eq!(stored.len(), 2);
        for source in &stored {
            if kept {
                assert_source(
                    source,
                    FORMULA,
                    facts(Some(1), &[], &[(0, Some("x"), Some("1"))]),
                );
            } else {
                assert!(source.is_none());
            }
        }
        let mut fresh = book_with_one();
        let mut fresh_session = SourceCacheSession::deferred();
        add_part(&mut fresh, &fresh_session, "first.xml", &[(FORMULA, cache)]);
        add_part(
            &mut fresh,
            &fresh_session,
            "second.xml",
            &[(FORMULA, cache)],
        );
        let fresh_work = capture(&mut fresh, &mut fresh_session, &mut lazy_provider());
        assert_eq!(fresh_work.decode_scans, 2);
        assert_eq!(sources(&fresh), stored);
        assert_eq!(report_of(&fresh), report);
        assert_memo_bounds(&provider);
    }
}

// Eager ZIP replay must share its part memo between a title and category,
// just as the lazy provider shares it across consecutive chart references.
#[test]
fn eager_titles_and_categories_share_one_dense_scan() {
    let package = two_reference_part_package();
    note_work(|counts| *counts = crate::drawing::charts::WorkCounts::default());
    let book = crate::reader::xlsx::read_reader(Cursor::new(package), true).unwrap();
    let work = crate::drawing::charts::work_counts();
    assert_eq!(
        (work.shape_scans, work.decode_scans, work.index_scans),
        (0, 1, 0)
    );
    // Two L/1 exceptions, each one point/two bytes, below both P/T caps.
    assert_eq!(report_of(&book), (2, 2, 4, 0, 0, 0, 0, 0));
    let stored = sources(&book);
    assert_eq!(stored.len(), 2);
    for source in &stored {
        assert_source(
            source,
            "Sheet1!$A$1",
            facts(Some(1), &[], &[(0, Some("L"), Some("1"))]),
        );
    }
}

// Equal shape/formula is insufficient: one differing byte must force the real
// decoder and preserve that different cached label.
#[test]
fn one_different_cache_byte_requires_another_scan() {
    let different = STALE.replace(">x<", ">y<");
    let mut book = book_with_one();
    let mut session = SourceCacheSession::deferred();
    add_part(
        &mut book,
        &session,
        "same.xml",
        &[(FORMULA, STALE), (FORMULA, &different)],
    );
    let mut provider = measured_provider();
    assert_eq!(
        capture(&mut book, &mut session, &mut provider).decode_scans,
        2
    );
    assert_eq!(provider.memo.observation.hits, 0);
    // Two one-point exceptions, each with two bytes of text.
    assert_eq!(report_of(&book), (2, 2, 4, 0, 0, 0, 0, 0));
    for (source, value) in sources(&book).iter().zip(["x", "y"]) {
        assert_source(
            source,
            FORMULA,
            facts(Some(1), &[], &[(0, Some(value), Some("1"))]),
        );
    }
    assert_memo_bounds(&provider);
}

// A rejected repeat at zero points must rebuild both observations, including
// the empty exception backing, rather than copying a prior observation.
#[test]
fn identical_rejections_at_zero_points_preserve_charges_and_backing() {
    let mut book = book_with_one();
    book.chart_source_cache_report_mut().kept_points = MAX_SOURCE_CACHE_POINTS;
    let mut session = SourceCacheSession::deferred();
    add_part(&mut book, &session, "same.xml", &[(FORMULA, STALE); 2]);
    let mut provider = measured_provider();
    assert_eq!(
        capture(&mut book, &mut session, &mut provider).decode_scans,
        1
    );
    // P prior points leave zero; each x/1 mismatch proposes one point/two
    // bytes.
    assert_eq!(
        report_of(&book),
        (0, MAX_SOURCE_CACHE_POINTS, 0, 2, 0, 2, 2, 4)
    );
    assert_eq!(sources(&book), vec![None, None]);
    assert_rejections(&session, true, true);
    assert_eq!(provider.memo.observation.hits, 1);
    assert_memo_bounds(&provider);
}

// A kept memo entry cannot bypass later point or text admission, even though
// it still owns a complete source that could be cloned.
#[test]
fn a_kept_repeat_is_rejected_when_either_budget_is_spent() {
    for points_limit in [true, false] {
        let mut book = book_with_one();
        let before = book.chart_source_cache_report_mut();
        if points_limit {
            before.kept_points = MAX_SOURCE_CACHE_POINTS - 1;
        } else {
            before.kept_text_bytes = MAX_SOURCE_CACHE_TEXT_BYTES - 2;
        }
        let mut session = SourceCacheSession::deferred();
        add_part(&mut book, &session, "same.xml", &[(FORMULA, STALE); 2]);
        let mut provider = measured_provider();
        assert_eq!(
            capture(&mut book, &mut session, &mut provider).decode_scans,
            1
        );
        // One remaining point or two remaining bytes admits exactly one x/1
        // exception. The second occurrence proposes the same one point/two
        // bytes.
        let expected = if points_limit {
            (1, MAX_SOURCE_CACHE_POINTS, 2, 1, 0, 1, 1, 2)
        } else {
            (1, 1, MAX_SOURCE_CACHE_TEXT_BYTES, 1, 0, 1, 1, 2)
        };
        assert_eq!(report_of(&book), expected);
        let stored = sources(&book);
        assert_source(
            &stored[0],
            FORMULA,
            facts(Some(1), &[], &[(0, Some("x"), Some("1"))]),
        );
        assert!(stored[1].is_none());
        assert_rejections(&session, points_limit, true);
        assert_memo_bounds(&provider);
    }
}

// An intervening admission spends the last point. A cached rejection's prior
// nonzero-budget shape observation must not hide the new zero-budget rejection.
#[test]
fn rejected_repeats_rebuild_shape_after_an_intervening_admission() {
    let two = cache_with_indices(0..2)
        .replace(">0<", ">x<")
        .replace(">1<", ">x<");
    let mut book = book_with_one();
    book.sheet_mut(0)
        .unwrap()
        .cell_mut("A2")
        .set_value_string("1");
    book.chart_source_cache_report_mut().kept_points = MAX_SOURCE_CACHE_POINTS - 1;
    let mut session = SourceCacheSession::deferred();
    add_part(
        &mut book,
        &session,
        "same.xml",
        &[
            ("Sheet1!A1:A2", &two),
            (FORMULA, STALE),
            ("Sheet1!A1:A2", &two),
        ],
    );
    let mut provider = measured_provider();
    assert_eq!(
        capture(&mut book, &mut session, &mut provider).decode_scans,
        2
    );
    // P-1 prior points: two mismatches cannot fit; the intervening one can.
    // Each rejected two-point cache proposes 2*(1+1)=4 text bytes.
    assert_eq!(
        report_of(&book),
        (1, MAX_SOURCE_CACHE_POINTS, 2, 2, 0, 2, 4, 8)
    );
    let stored = sources(&book);
    assert!(stored[0].is_none() && stored[2].is_none());
    assert_source(
        &stored[1],
        FORMULA,
        facts(Some(1), &[], &[(0, Some("x"), Some("1"))]),
    );
    assert_rejections(&session, true, true);
    assert_eq!(provider.memo.observation.hits, 1);
    assert_memo_bounds(&provider);
}

fn dense_values(points: usize, value: &str) -> String {
    let mut xml = format!("<c:strCache><c:ptCount val=\"{points}\"/>");
    for index in 0..points {
        xml.push_str(&format!("<c:pt idx=\"{index}\"><c:v>{value}</c:v></c:pt>"));
    }
    xml.push_str("</c:strCache>");
    xml
}

// The counting-only outcome must retain totals for a cache already over cap
// on its first occurrence, without retaining any of its rejected exceptions.
#[test]
fn dense_cache_over_cap_from_the_start_reuses_only_its_totals() {
    let points = MAX_SOURCE_CACHE_POINTS + 1;
    let xml = dense_values(points, "x");
    let formula = format!("Sheet1!A1:A{points}");
    let mut book = crate::new_file();
    let mut session = SourceCacheSession::deferred();
    add_part(
        &mut book,
        &session,
        "same.xml",
        &[(formula.as_str(), xml.as_str()); 2],
    );
    let mut provider = measured_provider();
    assert_eq!(
        capture(&mut book, &mut session, &mut provider).decode_scans,
        1
    );
    // All P+1 values differ from the empty cells: one point/one byte each.
    assert_eq!(report_of(&book), (0, 0, 0, 2, 0, 2, 2 * points, 2 * points));
    assert_eq!(sources(&book), vec![None, None]);
    assert_rejections(&session, false, true);
    assert_memo_bounds(&provider);
}

// Shape-dependent caches must rescan: missing/wrong counts, empty positive
// counts and repeated indices all lack the proof required for dense reuse.
#[test]
fn unproven_cache_shapes_are_reduced_freshly_for_each_occurrence() {
    let absent = EQUAL.replace("<c:ptCount val=\"1\"/>", "");
    let mismatch = EQUAL.replace("val=\"1\"", "val=\"2\"");
    let positive_empty = "<c:strCache><c:ptCount val=\"1\"/></c:strCache>";
    let repeated = dense_values(2, "1").replace("idx=\"1\"", "idx=\"0\"");
    let cases = [
        (
            absent.as_str(),
            facts(None, &[(0, 1)], &[]),
            (2, 2, 0, 0, 0, 0, 0, 0),
            0,
        ),
        (
            mismatch.as_str(),
            facts(Some(2), &[(0, 1)], &[]),
            (2, 2, 0, 0, 0, 0, 0, 0),
            0,
        ),
        (
            positive_empty,
            facts(Some(1), &[(0, 0)], &[]),
            (2, 2, 0, 0, 0, 0, 0, 0),
            0,
        ),
        (
            repeated.as_str(),
            facts(
                Some(2),
                &[(0, 1), (0, 1)],
                &[(0, Some("1"), Some("1")), (0, Some("1"), Some("1"))],
            ),
            (2, 8, 8, 0, 0, 0, 0, 0),
            2,
        ),
    ];
    for (xml, expected, report, index_scans) in cases {
        let mut book = book_with_one();
        let mut session = SourceCacheSession::deferred();
        add_part(&mut book, &session, "same.xml", &[(FORMULA, xml); 2]);
        assert!(
            session
                .descriptors
                .borrow()
                .values()
                .all(|source| source.summary.is_none())
        );
        let mut provider = measured_provider();
        let work = capture(&mut book, &mut session, &mut provider);
        assert_eq!(
            (work.shape_scans, work.decode_scans, work.index_scans),
            (2, 2, index_scans)
        );
        // The first three shapes cost one run each, no exceptions/text.
        // Repeats cost two runs plus two equal-but-required exceptions each,
        // with 2*(1+1)=4 bytes per cache. All are far below P/T caps.
        assert_eq!(report_of(&book), report);
        for source in sources(&book) {
            assert_source(&source, FORMULA, expected.clone());
        }
        assert_eq!(provider.memo.observation.hits, 0);
        assert_eq!(provider.memo.observation.measured_insertions, 0);
    }
}

// Zero-charge raw-sheet sources still need stored clones and independent
// observations, even after all logical point capacity has been consumed.
#[test]
fn raw_shape_repeats_are_kept_even_with_no_point_budget() {
    let mut book = crate::new_file();
    raw_data(&mut book, "Data");
    book.chart_source_cache_report_mut().kept_points = MAX_SOURCE_CACHE_POINTS;
    let mut session = SourceCacheSession::deferred();
    add_part(&mut book, &session, "same.xml", &[("Data!A1", EMPTY); 2]);
    let mut provider = measured_provider();
    assert_eq!(
        capture(&mut book, &mut session, &mut provider).decode_scans,
        1
    );
    // Raw empty A1 equals the empty cached value: no exception/run charge;
    // retain_shape keeps both counted sources despite zero remaining points.
    assert_eq!(
        report_of(&book),
        (2, MAX_SOURCE_CACHE_POINTS, 0, 0, 0, 0, 0, 0)
    );
    for source in sources(&book) {
        assert_source(&source, "Data!A1", facts(Some(1), &[], &[]));
    }
    assert_rejections(&session, true, false);
    assert_eq!(provider.memo.observation.hits, 1);
    assert_memo_bounds(&provider);
}

// Admission classification and retain_shape are occurrence inputs, so replay
// must not reuse a prior outcome when either changes. Drive the same replay
// entry used by capture, with real parsed descriptors and resolved cells.
#[test]
fn replay_separates_shape_retention_and_sheet_classification() {
    let book = book_with_one();
    let session = SourceCacheSession::deferred();
    let mut references = parse_part(&session, "same.xml", &[(FORMULA, EQUAL); 4]);
    let names = HashMap::from([("Sheet1".to_string(), 0)]);
    let streamed = HashMap::new();
    let mut provider = measured_provider();
    note_work(|counts| *counts = crate::drawing::charts::WorkCounts::default());
    for (index, reference) in references.iter_mut().enumerate() {
        let request = Request::new(reference, &names);
        let classes = [if index < 2 { FRESH_SHEET } else { 0 }];
        let mut resolver = request.resolve(&book, &classes, &streamed);
        let mut formats = PreparedFormats::default();
        let mut source = session.take(reference).unwrap();
        let formula_pointer = source.formula.as_ptr();
        let budget = ReplayBudget {
            expected_points: request.expected_points(),
            retain_shape:    index != 0,
            points:          MAX_SOURCE_CACHE_POINTS,
            text:            MAX_SOURCE_CACHE_TEXT_BYTES,
        };
        provider.with_memoized_part("same.xml", &session, &mut |part, memo| {
            let result = replay_with_memo(
                part.unwrap(),
                memo,
                &mut source,
                &budget,
                (request.sheet, classes[0], resolver.classification()),
                |point, scratch| resolver.display(point, scratch, &mut formats),
            )
            .unwrap();
            if index == 0 {
                assert!(matches!(result, DeferredReduction::Reduced));
            } else {
                let DeferredReduction::Candidate {
                    source: Some(kept),
                    points,
                    text_bytes,
                    ..
                } = result
                else {
                    panic!("zero-charge kept source");
                };
                assert_eq!((points, text_bytes), (0, 0));
                assert_eq!(kept.formula(), FORMULA);
                assert_eq!(facts_of(&kept), facts(Some(1), &[], &[]));
            }
            if index < 3 {
                // The key owns the original String allocation, not a copy.
                assert!(source.formula.is_empty());
                assert_eq!(
                    memo.entries.last().unwrap().formula.as_ptr(),
                    formula_pointer
                );
            }
        });
    }
    assert_eq!(crate::drawing::charts::work_counts().decode_scans, 3);
    assert_eq!(provider.memo.observation.hits, 1);
    provider.finish_sheet(&session);
    assert_memo_bounds(&provider);
}

// Reacquiring A after visiting B must not reuse A's old offset keys. Exercise
// both real providers and the A/B/A visitation used by merged chart parts.
#[test]
fn part_switches_and_revisited_parts_force_fresh_replay() {
    for revisit in [false, true] {
        for eager in [false, true] {
            let mut book = book_with_one();
            let mut session = SourceCacheSession::deferred();
            let mut a = parse_part(&session, "a.xml", &[(FORMULA, STALE); 2]).into_iter();
            let mut b = parse_part(&session, "b.xml", &[(FORMULA, STALE)]).into_iter();
            let mut ordered = vec![a.next().unwrap(), b.next().unwrap()];
            if revisit {
                ordered.push(a.next().unwrap());
            }
            for reference in ordered {
                book.sheet_mut(0)
                    .unwrap()
                    .add_chart(chart_with_reference(reference));
            }
            let count = if revisit { 3 } else { 2 };
            let work = if eager {
                let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
                for part in session.parts.borrow().iter() {
                    zip.start_file(&part.target, zip::write::SimpleFileOptions::default())
                        .unwrap();
                    zip.write_all(part.data.as_deref().unwrap()).unwrap();
                }
                let mut archive = zip::ZipArchive::new(zip.finish().unwrap()).unwrap();
                let mut provider = EagerChartPartProvider::new(&mut archive);
                provider.memo.measure_allocations = true;
                let work = capture(&mut book, &mut session, &mut provider);
                assert_eq!(provider.memo.observation.hits, 0);
                assert!(provider.memo.entries.is_empty());
                assert_eq!(provider.memo.bytes, 0);
                assert!(provider.memo.observation.allocated_peak <= MAX_MEMO_BYTES);
                work
            } else {
                let mut provider = measured_provider();
                let work = capture(&mut book, &mut session, &mut provider);
                assert_eq!(provider.memo.observation.hits, 0);
                assert_memo_bounds(&provider);
                work
            };
            assert_eq!(work.decode_scans, count);
            // Each occurrence costs one x/1 exception, one point/two bytes.
            assert_eq!(report_of(&book), (count, count, 2 * count, 0, 0, 0, 0, 0));
            for source in sources(&book) {
                assert_source(
                    &source,
                    FORMULA,
                    facts(Some(1), &[], &[(0, Some("x"), Some("1"))]),
                );
            }
        }
    }
}

fn many_distinct(
    book: &mut Workbook,
    session: &SourceCacheSession,
    formulas: &[String],
    cache: &str,
) {
    let mut specs: Vec<_> = formulas
        .iter()
        .map(|formula| (formula.as_str(), cache))
        .collect();
    // An old entry stays usable after saturation; a key beyond the limit
    // still reduces correctly when seen again.
    specs.push((formulas[0].as_str(), cache));
    specs.push((formulas.last().unwrap().as_str(), cache));
    add_part(book, session, "many.xml", &specs);
}

// Reduced entries consume no logical source budget, but their formulas and
// the table must still stop at the memo's entry/byte limits.
#[test]
fn hundreds_of_reduced_caches_bound_memo_storage() {
    let mut book = crate::new_file();
    let mut session = SourceCacheSession::deferred();
    let formulas: Vec<_> = (1..=300).map(|row| format!("Sheet1!A{row}")).collect();
    many_distinct(&mut book, &session, &formulas, EMPTY);
    let mut provider = measured_provider();
    assert_eq!(
        capture(&mut book, &mut session, &mut provider).decode_scans,
        301
    );
    // Every one-cell range reproduces its empty label: 300+2 eliminations,
    // no kept/rejected point or byte charges (independent of P/T).
    assert_eq!(report_of(&book), (0, 0, 0, 302, 302, 0, 0, 0));
    assert!(sources(&book).iter().all(Option::is_none));
    assert_eq!(provider.memo.observation.entries, MAX_MEMO_ENTRIES);
    assert_eq!(provider.memo.observation.hits, 1);
    assert_memo_bounds(&provider);
}

// Rejected entries also need formula accounting: logical exhaustion must not
// permit an unbounded rejection table or disable already retained entries.
#[test]
fn hundreds_of_rejected_caches_bound_memo_storage() {
    let mut book = crate::new_file();
    book.chart_source_cache_report_mut().kept_points = MAX_SOURCE_CACHE_POINTS;
    let mut session = SourceCacheSession::deferred();
    let formulas: Vec<_> = (1..=300).map(|row| format!("Sheet1!A{row}")).collect();
    many_distinct(&mut book, &session, &formulas, STALE);
    let mut provider = measured_provider();
    assert_eq!(
        capture(&mut book, &mut session, &mut provider).decode_scans,
        301
    );
    // P already consumed. Each x/empty mismatch proposes one point/one byte.
    assert_eq!(
        report_of(&book),
        (0, MAX_SOURCE_CACHE_POINTS, 0, 302, 0, 302, 302, 302)
    );
    assert!(sources(&book).iter().all(Option::is_none));
    assert_eq!(provider.memo.observation.entries, MAX_MEMO_ENTRIES);
    assert_eq!(provider.memo.observation.hits, 1);
    assert_rejections(&session, true, true);
    assert_memo_bounds(&provider);
}

// Shape-only kept candidates have zero logical charge but still own two
// formula strings per entry, so they cannot bypass memo limits.
#[test]
fn hundreds_of_zero_charge_kept_caches_bound_memo_storage() {
    let mut book = crate::new_file();
    raw_data(&mut book, "Data");
    let mut session = SourceCacheSession::deferred();
    let formulas: Vec<_> = (1..=300).map(|row| format!("Data!A{row}")).collect();
    many_distinct(&mut book, &session, &formulas, EMPTY);
    let mut provider = measured_provider();
    assert_eq!(
        capture(&mut book, &mut session, &mut provider).decode_scans,
        301
    );
    // Raw empty cells match every cached empty label; shape is kept for all
    // 302 occurrences, charging zero points and zero text bytes against P/T.
    assert_eq!(report_of(&book), (302, 0, 0, 0, 0, 0, 0, 0));
    let mut expected = formulas.clone();
    expected.push(formulas[0].clone());
    expected.push(formulas.last().unwrap().clone());
    for (source, formula) in sources(&book).iter().zip(expected) {
        assert_source(source, &formula, facts(Some(1), &[], &[]));
    }
    assert_eq!(provider.memo.observation.entries, MAX_MEMO_ENTRIES);
    assert_eq!(provider.memo.observation.hits, 1);
    assert_memo_bounds(&provider);
}

// A huge rejected payload must not be retained by the memo; only its totals
// and owned formula survive for the repeated occurrence.
#[test]
fn a_large_rejected_repeat_retains_only_a_small_memo_entry() {
    let bytes = MAX_SOURCE_CACHE_TEXT_BYTES + 1;
    let xml = dense_values(1, &"x".repeat(bytes));
    let mut book = crate::new_file();
    let mut session = SourceCacheSession::deferred();
    add_part(
        &mut book,
        &session,
        "large.xml",
        &[(FORMULA, xml.as_str()); 2],
    );
    let formula_capacity = session
        .descriptors
        .borrow()
        .values()
        .next()
        .unwrap()
        .formula
        .capacity();
    let mut provider = measured_provider();
    assert_eq!(
        capture(&mut book, &mut session, &mut provider).decode_scans,
        1
    );
    // Each x^(T+1)/empty exception proposes one point and T+1 bytes.
    assert_eq!(report_of(&book), (0, 0, 0, 2, 0, 2, 2, 2 * bytes));
    assert_eq!(sources(&book), vec![None, None]);
    assert_eq!(provider.memo.observation.entries, 1);
    assert_eq!(provider.memo.observation.hits, 1);
    assert!(
        provider.memo.observation.allocated_peak
            <= MAX_MEMO_ENTRIES * size_of::<MemoEntry>() + formula_capacity
    );
    assert_memo_bounds(&provider);
}

// Formula bytes are excluded from logical admission, so long zero-charge
// sources must exhaust the byte limit before exhausting the entry limit.
#[test]
fn long_formulas_bound_zero_charge_kept_memo_storage() {
    let name = "S".repeat(16 * 1024);
    let mut book = crate::new_file();
    raw_data(&mut book, &name);
    let mut session = SourceCacheSession::deferred();
    let formulas: Vec<_> = (1..=80).map(|row| format!("{name}!A{row}")).collect();
    many_distinct(&mut book, &session, &formulas, EMPTY);
    // Key allocations come from parsed descriptors, not the input strings.
    // Greedily charge the fixed table, moved key and exact-length source clone.
    let key_capacities: HashMap<_, _> = session
        .descriptors
        .borrow()
        .values()
        .map(|source| (source.formula.clone(), source.formula.capacity()))
        .collect();
    let mut expected_bytes = MAX_MEMO_ENTRIES * size_of::<MemoEntry>();
    let mut expected_entries = 0;
    for formula in &formulas {
        let charge = key_capacities[formula] + formula.len();
        if expected_entries < MAX_MEMO_ENTRIES && expected_bytes + charge <= MAX_MEMO_BYTES {
            expected_bytes += charge;
            expected_entries += 1;
        }
    }
    let mut provider = measured_provider();
    assert_eq!(
        capture(&mut book, &mut session, &mut provider).decode_scans,
        81
    );
    // Equal empty cells + retain_shape: all 80+2 are kept, with no P/T charge.
    // Later occurrences still keep their sources after memo saturation.
    assert_eq!(report_of(&book), (82, 0, 0, 0, 0, 0, 0, 0));
    let mut expected = formulas.clone();
    expected.push(formulas[0].clone());
    expected.push(formulas.last().unwrap().clone());
    let stored = sources(&book);
    assert_eq!(stored.len(), expected.len());
    for (source, formula) in stored.iter().zip(expected) {
        assert_source(source, &formula, facts(Some(1), &[], &[]));
    }
    assert_eq!(provider.memo.observation.entries, expected_entries);
    assert_eq!(provider.memo.observation.hits, 1);
    assert_memo_bounds(&provider);
}

// Nested value/display allocations must be charged before cloning, or a
// handful of kept exceptions can silently exceed the memo's byte budget.
#[test]
fn long_exception_strings_bound_kept_memo_storage() {
    const TEXT: usize = 24 * 1024;
    let display = "d".repeat(TEXT);
    let mut book = crate::new_file();
    book.sheet_mut(0)
        .unwrap()
        .cell_mut("A1")
        .set_value_string(&display);
    let mut session = SourceCacheSession::deferred();
    let values: Vec<_> = (0..40)
        .map(|index| format!("{index:03}{}", "x".repeat(TEXT - 3)))
        .collect();
    let caches: Vec<_> = values.iter().map(|value| dense_values(1, value)).collect();
    let mut specs: Vec<_> = caches
        .iter()
        .map(|cache| (FORMULA, cache.as_str()))
        .collect();
    specs.push((FORMULA, &caches[0]));
    specs.push((FORMULA, &caches[39]));
    add_part(&mut book, &session, "long-exceptions.xml", &specs);
    // Derive the saturation boundary from inputs before replay. Even using
    // the largest key, 20 kept clones plus all 19 rejected keys fit; using
    // the smallest key, 21 kept clones cannot fit.
    let (smallest_key, largest_key) = session
        .descriptors
        .borrow()
        .values()
        .map(|source| source.formula.capacity())
        .fold((usize::MAX, 0), |(smallest, largest), capacity| {
            (smallest.min(capacity), largest.max(capacity))
        });
    let table_bytes = MAX_MEMO_ENTRIES * size_of::<MemoEntry>();
    let clone_bytes = FORMULA.len() + size_of::<SourceException>() + 2 * TEXT;
    assert!(table_bytes + 20 * (largest_key + clone_bytes) + 19 * largest_key <= MAX_MEMO_BYTES);
    assert!(table_bytes + 21 * (smallest_key + clone_bytes) > MAX_MEMO_BYTES);
    let mut provider = measured_provider();
    capture(&mut book, &mut session, &mut provider);
    // Every mismatch costs one point and 2*24 KiB. T admits floor(T/49152)
    // =21, well below P; all remaining 42-21=21 are rejected whole.
    let kept = MAX_SOURCE_CACHE_TEXT_BYTES / (2 * TEXT);
    assert_eq!(kept, 21);
    assert_eq!(
        report_of(&book),
        (
            kept,
            kept,
            kept * 2 * TEXT,
            42 - kept,
            0,
            42 - kept,
            42 - kept,
            (42 - kept) * 2 * TEXT
        )
    );
    let stored = sources(&book);
    assert_eq!(stored.len(), 42);
    for (index, source) in stored.iter().enumerate() {
        if index < kept {
            assert_source(
                source,
                FORMULA,
                facts(
                    Some(1),
                    &[],
                    &[(0, Some(values[index].as_str()), Some(display.as_str()))],
                ),
            );
        } else {
            assert!(source.is_none());
        }
    }
    // The table plus 20 kept clones fit; the 21st does not. The next 19
    // rejected entries store only keys. Both the first and last cache hit.
    assert_eq!(provider.memo.observation.measured_insertions, 39);
    assert_eq!(provider.memo.observation.entries, 39);
    assert_eq!(provider.memo.observation.hits, 2);
    assert_memo_bounds(&provider);
}

// Merged chart spaces can obtain their title and category from different
// serialized parts. Byte-identical caches across that boundary must rescan.
#[test]
fn merged_titles_and_categories_do_not_share_a_part_memo() {
    let package = rewrite_package_part(
        &merged_title_capture_package(),
        "xl/charts/chart2.xml",
        |xml| {
            xml.replace("Sheet1!$B$1", "Sheet1!$A$1")
                .replace("<c:v>B</c:v>", "<c:v>A</c:v>")
        },
    );
    let mut book = crate::reader::xlsx::read_reader(Cursor::new(package.clone()), false).unwrap();
    let shared_strings = book.shared_string_table();
    let stylesheet = book.stylesheet().clone();
    let mut session = SourceCacheSession::deferred();
    let mut provider = measured_provider();
    crate::reader::xlsx::raw_to_deserialize_by_worksheet(
        &mut book.sheet_collection_unchecked_mut()[0],
        &shared_strings,
        &stylesheet,
        &mut session,
        &mut provider,
    );
    let work = capture(&mut book, &mut session, &mut provider);
    assert_eq!(work.decode_scans, 2);
    assert_eq!(provider.memo.observation.hits, 0);
    // Two A/1 mismatches each cost one point/two bytes; canonical title and
    // category survive independently, both well below the P/T caps.
    assert_eq!(report_of(&book), (2, 2, 4, 0, 0, 0, 0, 0));
    let stored = sources(&book);
    assert_eq!(stored.len(), 2);
    for source in &stored {
        assert_source(
            source,
            "Sheet1!$A$1",
            facts(Some(1), &[], &[(0, Some("A"), Some("1"))]),
        );
    }
    assert_memo_bounds(&provider);
    note_work(|counts| *counts = crate::drawing::charts::WorkCounts::default());
    let eager = crate::reader::xlsx::read_reader(Cursor::new(package), true).unwrap();
    assert_eq!(crate::drawing::charts::work_counts().decode_scans, 2);
    assert_eq!(report_of(&eager), (2, 2, 4, 0, 0, 0, 0, 0));
    assert_eq!(sources(&eager), stored);
}
