use std::panic::{
    AssertUnwindSafe as CatchSafe,
    catch_unwind as catch_panic,
};

use super::*;

#[derive(Debug, PartialEq)]
enum Snapshot {
    Reduced,
    Candidate {
        source:            Option<SourceCache>,
        points:            usize,
        text:              usize,
        shape_backing:     Option<CandidateBacking>,
        exception_backing: Option<CandidateBacking>,
    },
}

fn snapshot(reduction: DeferredReduction) -> Snapshot {
    match reduction {
        DeferredReduction::Reduced => Snapshot::Reduced,
        DeferredReduction::Candidate {
            source,
            points,
            text_bytes,
            rejection_backing,
        } => Snapshot::Candidate {
            source,
            points,
            text: text_bytes,
            shape_backing: rejection_backing.shape,
            exception_backing: rejection_backing.exception,
        },
    }
}

fn caught(operation: impl FnOnce() -> DeferredReduction) -> Result<Snapshot, String> {
    catch_panic(CatchSafe(operation))
        .map(snapshot)
        .map_err(|payload| match payload.downcast::<String>() {
            Ok(message) => *message,
            Err(payload) => (*payload.downcast::<&str>().unwrap()).to_owned(),
        })
}

fn descriptor(data: &[u8]) -> DeferredSource {
    DeferredSource {
        part:    0,
        start:   0,
        end:     data.len() as u64,
        formula: "Missing!A1:A100000".into(),
        summary: None,
    }
}

fn budget(points: usize, text: usize) -> ReplayBudget {
    ReplayBudget {
        expected_points: None,
        retain_shape: true,
        points,
        text,
    }
}

// Both branches are the real reduction/replay entry points. Manufactured
// descriptors also exercise malformed spans that pass 1 normally stops first.
fn compare(
    data: &[u8],
    summary: Option<DenseSummary>,
    limits: &ReplayBudget,
    accepted: bool,
) -> Result<Snapshot, String> {
    let mut source = descriptor(data);
    source.summary = summary;
    let oracle = caught(|| {
        reduce_deferred_cache(
            data,
            &source,
            limits.expected_points,
            limits.retain_shape,
            limits.points,
            limits.text,
            |_, _| None,
        )
    });
    strict::SCANS.with(|seen| seen.set((0, 0)));
    let actual = caught(|| {
        replay_with_memo(
            data,
            &mut DenseMemo::default(),
            &mut source,
            limits,
            (None, 0, 0),
            |_, _| None,
        )
        .unwrap()
    });
    assert_eq!(
        strict::SCANS.with(StateCell::get),
        if accepted { (1, 0) } else { (0, 1) }
    );
    assert_eq!(
        actual,
        oracle,
        "span {:?}",
        String::from_utf8_lossy(&data[..data.len().min(200)])
    );
    actual
}

fn cache(prefix: &str, values: &[(&str, &str)], count: &str) -> String {
    let mut xml = format!("<{prefix}strCache><{prefix}ptCount val=\"{count}\"/>");
    for (index, value) in values {
        xml.push_str(&format!(
            "<{prefix}pt idx=\"{index}\"><{prefix}v>{value}</{prefix}v></{prefix}pt>"
        ));
    }
    xml.push_str(&format!("</{prefix}strCache>"));
    xml
}

// Exact prefix bytes, leading XML whitespace, decoding and all value-content
// edges must survive the fast replay with their independently supplied text.
#[test]
fn compact_cache_values_preserve_prefixes_and_text_bytes() {
    let long = "界".repeat(8192);
    let values = [
        ("", ""),
        (" > ", " > "),
        ("\r", "\r"),
        ("\r\n", "\r\n"),
        ("&#13;", "\r"),
        ("  café界  ", "  café界  "),
        ("&amp;&lt;&gt;&quot;&apos;&#65;&#x1F642;", "&<>\"'A🙂"),
        (long.as_str(), long.as_str()),
    ];
    for prefix in ["c:", "", "x:", "c.d-e_f:"] {
        for leading in ["", " \t\r\n"] {
            let raw: Vec<_> = values
                .iter()
                .enumerate()
                .map(|(i, (raw, _))| (i.to_string(), *raw))
                .collect();
            let raw: Vec<_> = raw.iter().map(|(i, raw)| (i.as_str(), *raw)).collect();
            let xml = format!("{leading}{}", cache(prefix, &raw, "8"));
            let result = compare(
                xml.as_bytes(),
                Some(DenseSummary { point_count: 8 }),
                &budget(8, usize::MAX),
                true,
            )
            .unwrap();
            let Snapshot::Candidate {
                source: Some(source),
                points,
                text,
                ..
            } = result
            else {
                panic!("kept values")
            };
            assert_eq!(points, 8);
            assert_eq!(
                text,
                values.iter().map(|(_, text)| text.len()).sum::<usize>()
            );
            let expected: Vec<_> = values
                .iter()
                .enumerate()
                .map(|(i, (_, text))| (i as u32, Some(*text), None))
                .collect();
            assert_eq!(facts_of(&source), facts(Some(8), &[], &expected));
        }
    }
}

// Counts from empty to large must use byte replay, retain exactly one exception
// per unavailable display, and retain no partial exceptions after a cap switch.
#[test]
fn compact_cache_sizes_match_tokenized_reduction_at_caps() {
    for prefix in ["c:", "", "x:", "c.d-e_f:"] {
        for count in [0, 1, 2, 31, 1000, 100_000] {
            let mut xml = format!("<{prefix}strCache><{prefix}ptCount val=\"{count}\"/>");
            for index in 0..count {
                xml.push_str(&format!(
                    "<{prefix}pt idx=\"{index}\"><{prefix}v>x</{prefix}v></{prefix}pt>"
                ));
            }
            xml.push_str(&format!("</{prefix}strCache>"));
            let summary = (count > 0).then_some(DenseSummary { point_count: count });
            for cap in [0, count as usize] {
                let result =
                    compare(xml.as_bytes(), summary, &budget(cap, usize::MAX), true).unwrap();
                let Snapshot::Candidate {
                    source,
                    points,
                    text,
                    ..
                } = result
                else {
                    panic!("candidate")
                };
                assert_eq!((points, text), (count as usize, count as usize));
                assert_eq!(source.is_some(), cap >= count as usize);
                if let Some(source) = source {
                    assert_eq!(source.exceptions().len(), count as usize);
                    assert!(source.index_runs().is_empty());
                    for (index, exception) in source.exceptions().iter().enumerate() {
                        assert_eq!(exception.index(), index as u32);
                        assert_eq!(exception.value(), Some("x"));
                        assert_eq!(exception.loaded_display(), None);
                    }
                }
            }
        }
    }
}

// Every excluded syntax must select fallback. The unchanged quick_xml path is
// the required oracle for permissive parsing, replacement and exact panics.
#[test]
fn noncompact_cache_syntax_restarts_with_identical_results_and_panics() {
    let base = cache("c:", &[("0", "a")], "1");
    let mut cases = vec![
        base.replace("><", "> \t\r\n<"),
        base.replace("</c:pt>", "</c:pt >"),
        base.replace('"', "'"),
        base.replace("idx=\"0\"", "idx=\"0\" other=\"x\""),
        base.replace("<c:strCache>", "<c:strCache xmlns:c=\"urn:c\">"),
        base.replace("c:v", "x:v"),
        base.replace("<c:v>a</c:v>", ""),
        base.replace("<c:v>a</c:v>", "<c:v/>"),
        base.replace("<c:v>a</c:v>", "<c:v>a</c:v><c:v>b</c:v>"),
        base.replace("<c:pt idx=\"0\"><c:v>a</c:v></c:pt>", "<c:pt idx=\"0\"/>"),
        base.replace(
            "<c:ptCount val=\"1\"/>",
            "<c:ptCount val=\"1\"/><c:ptCount val=\"2\"/>",
        ),
        base.replace("<c:ptCount val=\"1\"/>", "")
            .replace("</c:strCache>", "<c:ptCount val=\"1\"/></c:strCache>"),
        base.replace("<c:v>a</c:v>", "<c:v><![CDATA[a]]></c:v>"),
        base.replace("<c:v>a</c:v>", "<c:v><nested>a</nested></c:v>"),
        base.replace("</c:pt>", "<c:extLst><nested/></c:extLst></c:pt>"),
        base.replace("<c:pt ", "<!--comment--><c:pt "),
        base.replace("<c:pt ", "<?instruction x?><c:pt "),
        base.replace("<c:pt ", "<!DOCTYPE c:pt><c:pt "),
        base.replace("idx=\"0\"", "idx=\"0\" idx=\"1\""),
        base.replace("idx=\"0\"", "x:idx=\"0\""),
        base.replace("val=\"1\"", "val=\"4294967296\""),
        base.replace("val=\"1\"", "val=\"01\""),
        base.replace("val=\"1\"", "val=\"+1\""),
        base.replace("val=\"1\"", "val=\"&#49;\""),
        format!("\u{feff}{base}"),
        format!("\u{a0}{base}"),
        format!("{base} "),
        format!("{base}<extra/>"),
        base.replace("c:", ":"),
        base.replace("c:", "1c:"),
        base.replace("c:", "c::"),
        base.replace("<c:v>a", "<c:v>&unknown;"),
        base.replace("<c:v>a", "<c:v>&#0;"),
        base.replace("<c:v>a", "<c:v>&unfinished"),
        base.replace("</c:v>", "</c:wrong>"),
        base.replace("</c:pt>", ""),
        base.replace("</c:strCache>", ""),
    ];
    for index in ["00", "+0", "4294967296", "&#49;", "", "-1"] {
        cases.push(base.replace("idx=\"0\"", &format!("idx=\"{index}\"")));
    }
    let mut bytes: Vec<_> = cases.iter().map(|xml| xml.as_bytes().to_vec()).collect();
    bytes.push(base.replace("<c:v>a", "<c:v>INVALID").as_bytes().to_vec());
    let last = bytes.last_mut().unwrap();
    let start = last
        .windows(7)
        .position(|bytes| bytes == b"INVALID")
        .unwrap();
    last[start] = 0xff;
    for data in bytes {
        for cap in [0, usize::MAX] {
            drop(compare(&data, None, &budget(cap, cap), false));
        }
    }
}

fn unwrap_message(error: &dyn std::fmt::Debug) -> String {
    format!("called `Result::unwrap()` on an `Err` value: {error:?}")
}

// Independent outcomes catch a fallback that agrees with itself while losing
// whitespace handling, empty/last values, raw CDATA markers or exact failures.
#[test]
fn rejected_syntax_preserves_literal_values_and_failure_messages() {
    let base = cache("c:", &[("0", "a")], "1");
    for (xml, expected) in [
        (base.replace("><", "> \t\r\n<"), "a"),
        (base.replace("<c:v>a</c:v>", "<c:v/>"), ""),
        (
            base.replace("<c:v>a</c:v>", "<c:v>a</c:v><c:v>b</c:v>"),
            "b",
        ),
        (
            base.replace("<c:v>a</c:v>", "<c:v><![CDATA[a]]></c:v>"),
            "<![CDATA[a]]>",
        ),
    ] {
        let Snapshot::Candidate {
            source: Some(source),
            points,
            text,
            shape_backing,
            exception_backing,
        } = compare(xml.as_bytes(), None, &budget(1, 13), false).unwrap()
        else {
            panic!("one kept exception")
        };
        // One dense point, unavailable display: one exception, no runs;
        // CDATA's 9-byte opener + a + 3-byte closer is the largest text (13).
        assert_eq!((points, text), (1, expected.len()));
        assert_eq!(
            facts_of(&source),
            facts(Some(1), &[], &[(0, Some(expected), None)])
        );
        assert_eq!(shape_backing, None);
        assert_eq!(exception_backing, None);
    }
    let unknown = base.replace("<c:v>a", "<c:v>&unknown;");
    let truncated = base.replace("</c:strCache>", "");
    for limits in [budget(0, 0), budget(1, 13)] {
        assert_eq!(
            compare(unknown.as_bytes(), None, &limits, false),
            Err(unwrap_message(
                &quick_xml::escape::unescape("&unknown;").unwrap_err()
            )),
        );
        assert_eq!(
            compare(truncated.as_bytes(), None, &limits, false),
            Err("Error: Could not find c:strCache end element".to_owned()),
        );
    }
}

// Invalid text must still panic, even after the cache has exhausted its caps;
// restart must preserve the original decoder's full payload and position.
#[test]
fn malformed_text_still_panics_after_restart_at_zero_budget() {
    let mut malformed: Vec<_> = ["&unknown;", "&#0;", "&unfinished"]
        .into_iter()
        .map(|value| cache("c:", &[("0", value)], "1").into_bytes())
        .collect();
    let mut invalid_utf8 = cache("c:", &[("0", "x")], "1").into_bytes();
    let at = invalid_utf8
        .windows(3)
        .position(|bytes| bytes == b">x<")
        .unwrap()
        + 1;
    invalid_utf8[at] = 0xff;
    malformed.push(invalid_utf8);
    for xml in malformed {
        assert!(compare(&xml, None, &budget(0, 0), false).is_err());
    }
}

// A late syntax rejection discards a thousand tentative exceptions or the
// over-cap switch; work counters still include the discarded decode work.
#[test]
fn late_rejection_discards_tentative_charges_and_counts_both_scans() {
    let mut xml = String::from("<c:strCache><c:ptCount val=\"1001\"/>");
    for index in 0..1001 {
        let value = if index == 1000 { "<c:v>b</c:v>" } else { "" };
        xml.push_str(&format!("<c:pt idx=\"{index}\"><c:v>a</c:v>{value}</c:pt>"));
    }
    xml.push_str("</c:strCache>");
    for cap in [0, 500, 1001] {
        note_work(|work| *work = crate::drawing::charts::WorkCounts::default());
        // Dense pass-1 proofs permit repeated values; this point layout is
        // flat.
        let result = compare(
            xml.as_bytes(),
            Some(DenseSummary { point_count: 1001 }),
            &budget(cap, usize::MAX),
            false,
        )
        .unwrap();
        let work = crate::drawing::charts::work_counts();
        // compare runs the oracle once, then the partial attempt and fallback.
        assert_eq!(work.decode_scans, 3);
        assert_eq!(work.kept_values, 1002 + 1000 + 1002);
        let Snapshot::Candidate {
            source,
            points,
            text,
            ..
        } = result
        else {
            panic!("candidate")
        };
        assert_eq!((points, text), (1001, 1001));
        assert_eq!(source.is_some(), cap == 1001);
        if let Some(source) = source {
            assert_eq!(source.exceptions().len(), 1001);
            assert_eq!(source.exceptions()[1000].value(), Some("b"));
        }
    }
}

// Whole capture must publish only the completed result, and memo hits must
// bypass either scanner regardless of which path produced the first result.
#[test]
fn repeated_capture_publishes_one_charge_per_occurrence_after_either_scan() {
    for compact in [true, false] {
        let mut xml = cache("c:", &[("0", "x")], "1");
        if !compact {
            xml = xml.replace("</c:strCache>", "<!--late--></c:strCache>");
        }
        let mut book = crate::new_file();
        book.sheet_mut(0)
            .unwrap()
            .cell_mut("A1")
            .set_value_string("1");
        let mut session = SourceCacheSession::deferred();
        let reference_xml = format!("<c:strRef><c:f>Sheet1!A1</c:f>{xml}</c:strRef>");
        let part = format!("<root>{reference_xml}{reference_xml}</root>");
        session.begin_chart_part("repeat.xml");
        session.store_part(session.part_for("repeat.xml"), part.as_bytes().into());
        let mut reader = Reader::from_reader(part.as_bytes());
        let mut buffer = Vec::new();
        loop {
            buffer.clear();
            match reader.read_event_into(&mut buffer).unwrap() {
                Event::Start(root) if local_name(root.name().into_inner()) == b"strRef" => {
                    let mut reference = StringReference::default();
                    reference.set_attributes(&mut reader, &root, &session);
                    book.sheet_mut(0)
                        .unwrap()
                        .add_chart(chart_with_reference(reference));
                }
                Event::Eof => break,
                _ => (),
            }
        }
        session.end_chart_part();
        strict::SCANS.with(|seen| seen.set((0, 0)));
        note_work(|work| *work = crate::drawing::charts::WorkCounts::default());
        let mut provider = lazy_provider();
        capture_chart_source_caches(&mut book, &[0], &mut session, &mut provider);
        assert_eq!(
            strict::SCANS.with(StateCell::get),
            if compact { (1, 0) } else { (0, 1) }
        );
        assert_eq!(provider.memo.observation.hits, 1);
        assert_eq!(report_of(&book), (2, 2, 4, 0, 0, 0, 0, 0));
        let work = crate::drawing::charts::work_counts();
        assert_eq!(work.decode_scans, if compact { 1 } else { 2 });
        assert_eq!(work.kept_values, if compact { 1 } else { 2 });
        let expected = facts(Some(1), &[], &[(0, Some("x"), Some("1"))]);
        for chart in book.sheet_collection_no_check()[0].chart_collection() {
            visit_string_references(&mut chart.chart_space().clone(), &mut |_, reference| {
                assert_eq!(
                    facts_of(reference.string_cache().source().unwrap()),
                    expected
                );
            });
        }
    }
}

// Entity decoding in byte replay must reuse the existing decoder's scratch,
// rather than introduce one allocation per value into an otherwise equal cache.
#[test]
fn compact_entity_values_reuse_decoded_storage() {
    let mut xml = String::from("<c:strCache><c:ptCount val=\"2000\"/>");
    for index in 0..2000 {
        xml.push_str(&format!("<c:pt idx=\"{index}\"><c:v>A&amp;B</c:v></c:pt>"));
    }
    xml.push_str("</c:strCache>");
    let mut source = descriptor(xml.as_bytes());
    source.summary = Some(DenseSummary { point_count: 2000 });
    let mut limits = budget(usize::MAX, usize::MAX);
    limits.expected_points = Some(2000);
    limits.retain_shape = false;
    let mut memo = DenseMemo::default();
    strict::SCANS.with(|seen| seen.set((0, 0)));
    let (result, seen) = crate::test_alloc::observe(|| {
        replay_with_memo(
            xml.as_bytes(),
            &mut memo,
            &mut source,
            &limits,
            (None, 0, 0),
            |_, _| Some(CacheDisplay::Borrowed("A&B")),
        )
        .unwrap()
    });
    assert!(matches!(result, DeferredReduction::Reduced));
    assert_eq!(strict::SCANS.with(StateCell::get), (1, 0));
    // One decoded buffer (three bytes, String's minimum capacity eight) and
    // one fixed memo table; the already-owned formula moves without copying.
    assert_eq!(seen.total(), 2);
    assert_eq!(
        seen.total_bytes(),
        8 + MAX_MEMO_ENTRIES * size_of::<MemoEntry>()
    );
    assert_eq!(memo.entries.len(), 1);
}

// The strict grammar permits sparse, repeated and maximal canonical indices;
// byte eligibility must not accidentally substitute density for shape rules.
#[test]
fn compact_indices_preserve_runs_repeats_and_decimal_limits() {
    for (indices, count, runs, points) in [
        (vec!["0", "2", "4"], "3", vec![(0, 1), (2, 1), (4, 1)], 6),
        (vec!["2", "1", "2"], "3", vec![(2, 1), (1, 2)], 5),
        (vec!["4294967295"], "4294967295", vec![(u32::MAX, 1)], 2),
        (vec![], "1", vec![(0, 0)], 1),
    ] {
        let values: Vec<_> = indices.iter().map(|index| (*index, "x")).collect();
        let xml = cache("c:", &values, count);
        let result = compare(xml.as_bytes(), None, &budget(usize::MAX, usize::MAX), true).unwrap();
        let Snapshot::Candidate {
            source: Some(source),
            points: charged,
            text,
            ..
        } = result
        else {
            panic!("kept shape")
        };
        assert_eq!(charged, points);
        assert_eq!(text, indices.len());
        let expected: Vec<_> = indices
            .iter()
            .map(|index| (index.parse().unwrap(), Some("x"), None))
            .collect();
        assert_eq!(
            facts_of(&source),
            facts(Some(count.parse().unwrap()), &runs, &expected)
        );
    }
}
