//! Production cache capture regressions for the stage9 diagnostic-owner phase.
use super::*;
use crate::drawing::charts::{
    Formula,
    chart_reference_roles::{
        ReferenceTargetRef,
        tests::diagnostic_invert_reference_space,
        visit_ref,
    },
};

fn parsed_reference(address: &str, value: &str) -> StringReference {
    let mut reference = StringReference::default();
    let mut formula = Formula::default();
    formula.set_address_str(address);
    reference.set_formula(formula);
    reference
        .string_cache_mut()
        .set_source(Some(SourceCache::as_read(
            address.into(),
            Some(1),
            vec![(0, Some(value.into()))],
        )));
    reference
}

fn cohort(reference: StringReference, split_sheets: bool) -> (Workbook, Vec<usize>) {
    cohort_with_legacy(
        reference,
        parsed_reference("Missing!$B$1", "legacy"),
        split_sheets,
    )
}

fn cohort_with_legacy(
    reference: StringReference,
    legacy_reference: StringReference,
    split_sheets: bool,
) -> (Workbook, Vec<usize>) {
    let mut book = crate::new_file();
    let mut diagnostic = Chart::default();
    diagnostic.new_chart(
        &ChartType::LineChart,
        MarkerType::default(),
        MarkerType::default(),
        Vec::new(),
    );
    *diagnostic.chart_space_mut() = diagnostic_invert_reference_space(reference);
    book.sheet_mut(0).unwrap().add_chart(diagnostic);
    let legacy = chart_with_reference(legacy_reference);
    let fresh = if split_sheets {
        book.new_sheet("Second").unwrap().add_chart(legacy);
        vec![0, 1]
    } else {
        book.sheet_mut(0).unwrap().add_chart(legacy);
        vec![0]
    };
    (book, fresh)
}

fn sources(book: &Workbook) -> HashMap<String, bool> {
    let mut result = HashMap::new();
    for sheet in book.sheet_collection_no_check() {
        for chart in sheet.chart_collection() {
            visit_ref(chart.chart_space(), &mut |_, target| {
                if let ReferenceTargetRef::String(reference) = target {
                    assert!(
                        result
                            .insert(
                                reference.formula().address_str(),
                                reference.string_cache().source().is_some()
                            )
                            .is_none()
                    );
                }
            });
        }
    }
    result
}

fn exact_invert_diagnostics(
    book: &Workbook,
) -> (
    Vec<crate::drawing::charts::SeriesPartConflict>,
    Vec<crate::drawing::charts::ExtensionExportDiagnostic>,
) {
    use crate::drawing::charts::{
        SeriesFamily,
        SeriesPartConflict,
        SeriesPartConflictReason,
        SeriesPartKind,
    };
    let chart = &book.sheet_collection_no_check()[0].chart_collection()[0];
    let plot = chart.chart_space().chart().plot_area();
    let conflicts = plot.series_part_conflicts();
    assert_eq!(
        conflicts,
        vec![SeriesPartConflict {
            group_index:  0,
            series_index: 0,
            part:         SeriesPartKind::Extensions,
            occurrence:   0,
            reason:       SeriesPartConflictReason::TooMany,
        }]
    );
    let group = plot.chart_groups().next().unwrap();
    let extensions = group.series().area_chart_series()[0].extensions().unwrap();
    let fill = extensions.entries[0]
        .nodes
        .iter()
        .find_map(|node| {
            if let crate::drawing::charts::ExtensionNode::Known(payload) = node {
                if let crate::drawing::charts::ExtensionPayloadValue::InvertSolidFill(fill) =
                    &payload.value
                {
                    return Some(fill);
                }
            }
            // All other typed/text/opaque node kinds are not this requested
            // owner.
            None
        })
        .unwrap();
    assert!(fill.shape_properties().is_some());
    assert_eq!(fill.extra_shapes().len(), 1);
    let diagnostics =
        extensions.export_diagnostics_for_owner(Some(book), "ser", Some(SeriesFamily::Bar));
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].extension_occurrence, 0);
    assert_eq!(
        diagnostics[0].uri.as_deref(),
        Some("{6F2FDCE9-48DA-4B69-8628-5D25D57E5C99}")
    );
    assert_eq!(diagnostics[0].reason, "TooMany");
    (conflicts, diagnostics)
}

fn stored_charts(book: &Workbook) -> Vec<Vec<serde_json::Value>> {
    book.sheet_collection_no_check()
        .iter()
        .map(|sheet| {
            sheet
                .chart_collection()
                .iter()
                .map(|chart| serde_json::to_value(chart).unwrap())
                .collect()
        })
        .collect()
}

#[derive(Default, Debug)]
struct ExportSummary {
    bar_series:           usize,
    bar_direction:        Vec<String>,
    bar_indices:          Vec<String>,
    bar_orders:           Vec<String>,
    bar_axes:             Vec<String>,
    formulas:             Vec<String>,
    string_values:        Vec<String>,
    string_point_counts:  Vec<String>,
    string_point_indices: Vec<String>,
    invalid_entries:      usize,
    invert_payloads:      usize,
}
fn chart_summary(xml: &[u8]) -> ExportSummary {
    use quick_xml::{
        NsReader,
        name::ResolveResult,
    };
    const C: &str = "http://schemas.openxmlformats.org/drawingml/2006/chart";
    const C14: &str = "http://schemas.microsoft.com/office/drawing/2007/8/2/chart";
    const C15: &str = "http://schemas.microsoft.com/office/drawing/2012/chart";
    fn path_is(path: &[(String, String)], names: &[&str]) -> bool {
        path.len() == names.len()
            && path
                .iter()
                .zip(names)
                .all(|((ns, local), name)| ns.as_str() == C && local.as_str() == *name)
    }
    let mut reader = NsReader::from_reader(xml);
    let mut stack = Vec::new();
    let mut result = ExportSummary::default();
    let mut text: Option<(bool, String)> = None;
    loop {
        let event = reader.read_event().unwrap();
        let empty = matches!(event, Event::Empty(..));
        match event {
            Event::Start(element) | Event::Empty(element) => {
                let (namespace, local) = reader.resolver().resolve_element(element.name());
                let namespace = match namespace {
                    ResolveResult::Bound(value) => {
                        std::str::from_utf8(value.as_ref()).unwrap().to_owned()
                    }
                    ResolveResult::Unbound => String::new(),
                    ResolveResult::Unknown(..) => panic!("unbound namespace in emitted chart part"),
                };
                let local = std::str::from_utf8(local.as_ref()).unwrap().to_owned();
                let attribute = |name: &[u8]| {
                    element
                        .attributes()
                        .map(Result::unwrap)
                        .find(|a| a.key.as_ref() == name)
                        .map(|a| {
                            a.decoded_and_normalized_value(
                                quick_xml::XmlVersion::Implicit1_0,
                                reader.decoder(),
                            )
                            .unwrap()
                            .into_owned()
                        })
                };
                if namespace == C
                    && local == "ext"
                    && attribute(b"uri").as_deref()
                        == Some("{6F2FDCE9-48DA-4B69-8628-5D25D57E5C99}")
                {
                    result.invalid_entries += 1;
                }
                if namespace == C14 && local == "invertSolidFillFmt" {
                    result.invert_payloads += 1;
                }
                stack.push((namespace.clone(), local.clone()));
                if path_is(
                    &stack,
                    &["chartSpace", "chart", "plotArea", "barChart", "ser"],
                ) {
                    result.bar_series += 1;
                }
                if path_is(
                    &stack,
                    &["chartSpace", "chart", "plotArea", "barChart", "barDir"],
                ) {
                    result.bar_direction.push(attribute(b"val").unwrap());
                }
                if path_is(
                    &stack,
                    &["chartSpace", "chart", "plotArea", "barChart", "ser", "idx"],
                ) {
                    result.bar_indices.push(attribute(b"val").unwrap());
                }
                if path_is(
                    &stack,
                    &[
                        "chartSpace",
                        "chart",
                        "plotArea",
                        "barChart",
                        "ser",
                        "order",
                    ],
                ) {
                    result.bar_orders.push(attribute(b"val").unwrap());
                }
                if path_is(
                    &stack,
                    &["chartSpace", "chart", "plotArea", "barChart", "axId"],
                ) {
                    result.bar_axes.push(attribute(b"val").unwrap());
                }
                let in_string_reference =
                    stack.iter().any(|(ns, name)| ns == C && name == "strRef");
                if namespace == C && in_string_reference && local == "ptCount" {
                    result.string_point_counts.push(attribute(b"val").unwrap());
                }
                if namespace == C && in_string_reference && local == "pt" {
                    result.string_point_indices.push(attribute(b"idx").unwrap());
                }
                if (namespace == C && local == "f")
                    || (namespace == C15 && matches!(local.as_str(), "f" | "sqref"))
                {
                    assert!(text.is_none());
                    text = Some((false, String::new()));
                } else if namespace == C
                    && local == "v"
                    && stack.iter().any(|(ns, local)| ns == C && local == "strRef")
                {
                    assert!(text.is_none());
                    text = Some((true, String::new()));
                }
                if empty {
                    stack.pop();
                }
            }
            event @ (Event::Text(..) | Event::CData(..) | Event::GeneralRef(..)) => {
                if let Some((_, value)) = &mut text {
                    value.push_str(
                        &crate::reader::driver::decode_xml_character_event(&event).unwrap(),
                    );
                }
            }
            Event::End(..) => {
                let (_, local) = stack.last().unwrap();
                if matches!(local.as_str(), "f" | "sqref" | "v") {
                    if let Some((cache, value)) = text.take() {
                        if cache {
                            result.string_values.push(value);
                        } else {
                            result.formulas.push(value);
                        }
                    }
                }
                stack.pop();
            }
            Event::Eof => break,
            Event::Decl(..) | Event::PI(..) | Event::Comment(..) | Event::DocType(..) => (),
        }
    }
    assert!(stack.is_empty());
    assert!(text.is_none());
    result
}

fn record_diagnostic_output(bytes: &[u8], split_sheets: bool) {
    use sha2::{
        Digest,
        Sha256,
    };
    if let Some(root) = std::env::var_os("PHASE9_OUTPUT_DIR") {
        let source = std::env::var("PHASE9_SOURCE_ID").expect("source identity required");
        let run = std::env::var("PHASE9_RUN_ID").expect("run identity required");
        for identity in [&source, &run] {
            assert_eq!(identity.len(), 64);
            assert!(identity.bytes().all(|v| v.is_ascii_hexdigit()));
        }
        let stem = if split_sheets {
            "diagnostic-global-cache-two-hosts--direct"
        } else {
            "diagnostic-global-cache-one-host--direct"
        };
        let root = std::path::PathBuf::from(root);
        std::fs::create_dir_all(&root).unwrap();
        let digest: String = Sha256::digest(bytes)
            .iter()
            .map(|v| format!("{v:02x}"))
            .collect();
        std::fs::write(root.join(format!("{stem}.xlsx")), bytes).unwrap();
        std::fs::write(
            root.join(format!("{stem}.receipt")),
            format!("{source}\n{run}\n{digest}\n"),
        )
        .unwrap();
    }
}

#[test]
fn global_legacy_prefix_wins_real_tight_budget_across_charts_and_fresh_sheets() {
    for split_sheets in [false, true] {
        let (mut book, fresh) = cohort(parsed_reference("Missing!$A$1", "deferred"), split_sheets);
        let diagnostics = exact_invert_diagnostics(&book);
        book.chart_source_cache_report_mut().kept_points = MAX_SOURCE_CACHE_POINTS - 1;
        let mut session = SourceCacheSession::immediate();
        let mut provider = lazy_provider();
        reset_chart_part_buffer_observation();
        capture_chart_source_caches(&mut book, &fresh, &mut session, &mut provider);
        assert_eq!(
            &*session.capture_formulas.borrow(),
            &["Missing!$B$1", "Missing!$A$1"]
        );
        assert_eq!(
            sources(&book),
            HashMap::from([
                ("Missing!$A$1".into(), false),
                ("Missing!$B$1".into(), true),
            ])
        );
        let report = book.chart_source_cache_report();
        assert_eq!(report.kept_points, MAX_SOURCE_CACHE_POINTS);
        assert_eq!(report.kept_caches, 1);
        assert_eq!(report.over_cap_caches, 1);
        let observation = chart_part_buffer_observation();
        assert_eq!(observation.lazy_sheet_boundaries, 2 * fresh.len());
        assert!(observation.lazy_pending_parts_are_exact);
        assert!(session.descriptors.borrow().is_empty());
        assert_eq!(exact_invert_diagnostics(&book), diagnostics);
        let stored = stored_charts(&book);
        let legacy = book
            .sheet_collection_no_check()
            .iter()
            .flat_map(|sheet| sheet.chart_collection())
            .nth(1)
            .unwrap();
        let mut expected_formulas = Vec::new();
        visit_ref(legacy.chart_space(), &mut |_, target| {
            expected_formulas.push(target.formula().address_str());
        });
        let mut bytes = Cursor::new(Vec::new());
        crate::writer::xlsx::write_writer(&book, &mut bytes).unwrap();
        assert_eq!(stored_charts(&book), stored);
        assert_eq!(exact_invert_diagnostics(&book), diagnostics);
        let bytes = bytes.into_inner();
        let mut archive = zip::ZipArchive::new(Cursor::new(&bytes)).unwrap();
        let mut parts: Vec<_> = archive
            .file_names()
            .filter(|name| name.starts_with("xl/charts/") && name.ends_with(".xml"))
            .map(str::to_owned)
            .collect();
        parts.sort();
        assert_eq!(parts, ["xl/charts/chart1.xml", "xl/charts/chart2.xml"]);
        let mut first = Vec::new();
        archive
            .by_name(&parts[0])
            .unwrap()
            .read_to_end(&mut first)
            .unwrap();
        let first = chart_summary(&first);
        assert_eq!(first.invalid_entries, 0);
        assert_eq!(first.invert_payloads, 0);
        assert_eq!(first.bar_series, 1);
        assert_eq!(first.bar_direction, ["col"]);
        assert_eq!(first.bar_indices, ["7"]);
        assert_eq!(first.bar_orders, ["8"]);
        assert_eq!(first.bar_axes, ["1", "2"]);
        assert!(first.formulas.is_empty());
        let mut second = Vec::new();
        archive
            .by_name(&parts[1])
            .unwrap()
            .read_to_end(&mut second)
            .unwrap();
        let second = chart_summary(&second);
        assert_eq!(second.formulas, expected_formulas);
        assert_eq!(second.string_values, ["legacy"]);
        assert_eq!(second.string_point_counts, ["1"]);
        assert_eq!(second.string_point_indices, ["0"]);
        assert_eq!(second.invalid_entries, 0);
        assert_eq!(second.invert_payloads, 0);
        record_diagnostic_output(&bytes, split_sheets);
    }
}

#[test]
fn actual_deferred_provider_releases_parts_and_finishes_after_both_global_phases() {
    let mut session = SourceCacheSession::deferred();
    let reference = deferred_reference(
        &session,
        concat!(
            "<c:strRef><c:f>Missing!$A$1</c:f><c:strCache><c:ptCount val=\"1\"/>",
            "<c:pt idx=\"0\"><c:v>deferred</c:v></c:pt></c:strCache></c:strRef>",
        ),
    );
    assert!(reference.string_cache().source_key().is_some());
    assert_eq!(session.descriptors.borrow().len(), 1);
    let part = session.part_for("deferred.xml");
    let legacy = deferred_reference_at(
        &session,
        "legacy.xml",
        concat!(
            "<c:strRef><c:f>Missing!$B$1</c:f><c:strCache><c:ptCount val=\"1\"/>",
            "<c:pt idx=\"0\"><c:v>legacy</c:v></c:pt></c:strCache></c:strRef>",
        ),
    );
    assert!(legacy.string_cache().source_key().is_some());
    assert_eq!(session.descriptors.borrow().len(), 2);
    let (mut book, fresh) = cohort_with_legacy(reference, legacy, true);
    book.chart_source_cache_report_mut().kept_points = MAX_SOURCE_CACHE_POINTS - 2;
    let mut provider = lazy_provider();
    reset_chart_part_buffer_observation();
    capture_chart_source_caches(&mut book, &fresh, &mut session, &mut provider);
    assert_eq!(
        &*session.capture_formulas.borrow(),
        &["Missing!$B$1", "Missing!$A$1"]
    );
    assert!(sources(&book).values().all(|kept| *kept));
    assert_eq!(book.chart_source_cache_report().kept_caches, 2);
    assert_eq!(
        book.chart_source_cache_report().kept_points,
        MAX_SOURCE_CACHE_POINTS
    );
    assert!(session.descriptors.borrow().is_empty());
    assert_eq!(provider.expected_part, Some(part));
    assert_eq!(provider.last_released, Some(part));
    assert!(provider.current.is_none());
    assert!(session.parts.borrow()[part].data.is_some());
    let observation = chart_part_buffer_observation();
    assert_eq!(observation.lazy_sheet_boundaries, 4);
    assert!(observation.lazy_pending_parts_are_exact);
}

#[test]
fn deferred_string_leaf_without_source_still_receives_one_callback() {
    let mut reference = StringReference::default();
    reference.formula_mut().set_address_str("Missing!$A$1");
    let (mut book, fresh) = cohort(reference, false);
    let mut session = SourceCacheSession::immediate();
    let mut provider = lazy_provider();
    reset_chart_part_buffer_observation();
    capture_chart_source_caches(&mut book, &fresh, &mut session, &mut provider);
    assert_eq!(
        &*session.capture_formulas.borrow(),
        &["Missing!$B$1", "Missing!$A$1"]
    );
    assert_eq!(chart_part_buffer_observation().lazy_sheet_boundaries, 2);
}

#[test]
fn no_deferred_leaf_preserves_single_pass_provider_lifecycle() {
    let mut book = crate::new_file();
    book.sheet_mut(0)
        .unwrap()
        .add_chart(chart_with_reference(parsed_reference(
            "Missing!$B$1",
            "legacy",
        )));
    let mut session = SourceCacheSession::immediate();
    let mut provider = lazy_provider();
    reset_chart_part_buffer_observation();
    capture_chart_source_caches(&mut book, &[0], &mut session, &mut provider);
    assert_eq!(&*session.capture_formulas.borrow(), &["Missing!$B$1"]);
    assert_eq!(chart_part_buffer_observation().lazy_sheet_boundaries, 1);
}
