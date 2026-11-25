//! Every conversion Excel made, run through the operation and held against
//! Excel's own after part; every conversion Excel refused, held against its
//! refusal.
//!
//! `admitted-baseline.txt` lists every admitted observation that matches
//! Excel today. A listed observation must keep matching, and one that starts
//! to match must be listed, so the matching set only grows through a
//! reviewed change of that file. Every other admitted observation is pending:
//! it must be refused as `Unsupported` from its measured source with the
//! chart unchanged. A conversion that runs is never allowed to differ.

use std::{
    collections::{
        BTreeMap,
        BTreeSet,
    },
    fmt::Write as _,
    fs,
    io::{
        Cursor,
        Write as _,
    },
    path::PathBuf,
};

use rstest::rstest;
use umya_spreadsheet::{
    helper::chart_conversion::{
        ChangeKind,
        ChartConversion,
        ChartConversionRefusal,
        ChartSource,
        ChartSubtype,
        ConversionChange,
        ConvertedPart,
    },
    reader,
};

use super::{
    conversion::{
        chart_part,
        convert,
        open,
        subtype,
    },
    fixture::{
        chart_xml,
        write_workbook,
    },
    measured::{
        Observation,
        Part,
        measured,
    },
    projection::{
        Json,
        Paths,
        Slots,
        diff_paths,
        paths_of,
        slots_of,
    },
};

/// The admitted observations that match Excel, one `<fixture> -> <target>`
/// per line.
const BASELINE: &str = include_str!("admitted-baseline.txt");

fn baseline() -> BTreeSet<&'static str> {
    BASELINE
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect()
}

/// With `CHART_CONVERSION_KEYS` naming a file of observation keys (one per
/// line, like the baseline), a run covers those observations only, and each
/// of them must match Excel.
fn selected() -> Option<BTreeSet<String>> {
    let path = std::env::var_os("CHART_CONVERSION_KEYS")?;
    let text = fs::read_to_string(&path).unwrap_or_else(|error| panic!("{path:?}: {error}"));
    Some(
        text.lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .map(str::to_owned)
            .collect(),
    )
}

/// An observation's key: its before part and its target.
fn key(part: &Part, observation: &Observation) -> String {
    let fixture = part
        .fixture
        .trim_start_matches("before/")
        .trim_end_matches(".chart.xml");
    format!("{fixture} -> {}", observation.target)
}

/// The path patterns whose values a conversion chooses itself: axis ids,
/// axis crossings and a group's axis references. Their presence still counts.
const IGNORED: [&str; 3] = [
    r"^/chartSpace/chart/plotArea/(catAx|valAx|dateAx|serAx)(\[\d+\])?/axId$",
    r"^/chartSpace/chart/plotArea/(catAx|valAx|dateAx|serAx)(\[\d+\])?/crossAx$",
    r"^/chartSpace/chart/plotArea/GROUP/axId(\[\d+\])?$",
];

/// `[n]` or nothing at the start of `text`, and the rest.
fn without_index(text: &str) -> &str {
    let Some(rest) = text.strip_prefix('[') else {
        return text;
    };
    let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
    match rest[digits..].strip_prefix(']') {
        Some(after) if digits > 0 => after,
        _ => text,
    }
}

/// Whether `path` matches one of `IGNORED`.
fn ignored(path: &str) -> bool {
    if let Some(rest) = path.strip_prefix("/chartSpace/chart/plotArea/GROUP/axId") {
        return without_index(rest).is_empty();
    }
    let Some(rest) = path.strip_prefix("/chartSpace/chart/plotArea/") else {
        return false;
    };
    ["catAx", "valAx", "dateAx", "serAx"].iter().any(|axis| {
        rest.strip_prefix(axis)
            .map(without_index)
            .is_some_and(|tail| tail == "/axId" || tail == "/crossAx")
    })
}

/// The source a part is, as the measurement names it.
fn source_name(part: &Part) -> &str {
    part.subtype
        .as_deref()
        .or(part.layout.as_deref())
        .unwrap_or_default()
}

/// The source the measurement names, as the operation reports it.
fn expected_source(part: &Part) -> String {
    match (&part.subtype, &part.layout) {
        (Some(name), _) => format!("{:?}", ChartSource::Subtype(subtype(name))),
        (None, Some(layout)) => format!("Combo({layout})"),
        (None, None) => "no source".to_owned(),
    }
}

enum Verdict {
    Matches,
    Pending,
    Fails(Vec<String>),
}

/// A report entry as the measurement writes it.
fn change_json(change: &ConversionChange) -> Json {
    let part = match change.part {
        ConvertedPart::Scene => "Scene".to_owned(),
        ConvertedPart::Group => "Group".to_owned(),
        ConvertedPart::Axis { role, secondary } => {
            format!("Axis:{role:?}{}", if secondary { ":secondary" } else { "" })
        }
        ConvertedPart::SeriesPart(kind) => format!("SeriesPart:{kind:?}"),
        ConvertedPart::SeriesFill => "SeriesFill".to_owned(),
        ConvertedPart::SeriesLine => "SeriesLine".to_owned(),
        other => format!("{other:?}"),
    };
    let kind = match change.change {
        ChangeKind::MovedTo(kind) => format!("MovedTo:{kind:?}"),
        other => format!("{other:?}"),
    };
    Json::List(vec![
        change
            .series_index
            .map_or(Json::Null, |series| Json::Count(series as u64)),
        Json::Text(part),
        Json::Text(kind),
    ])
}

/// Slots with every axis id replaced by the position of its axis, so two
/// charts that wire the same topology with different ids compare equal.
fn topology(slots: &Slots) -> Slots {
    let mut names: BTreeMap<String, String> = BTreeMap::new();
    for (key, value) in slots {
        if let (true, Json::Text(id)) = (key.starts_with("ax.") && key.ends_with(".id"), value) {
            let axis = key.trim_end_matches(".id").to_owned();
            names.insert(id.clone(), axis);
        }
    }
    let rename = |value: &Json| match value {
        Json::Text(id) if id == "0" => Json::Text("0".to_owned()),
        Json::Text(id) => Json::Text(
            names
                .get(id)
                .cloned()
                .unwrap_or_else(|| format!("unknown axis {id}")),
        ),
        other => other.clone(),
    };
    slots
        .iter()
        .map(|(key, value)| {
            let value = if key.ends_with(".id") || key.ends_with(".crossAx") {
                rename(value)
            } else if key.ends_with(".axIds") {
                match value {
                    Json::List(ids) => Json::List(ids.iter().map(rename).collect()),
                    other => other.clone(),
                }
            } else {
                value.clone()
            };
            (key.clone(), value)
        })
        .collect()
}

fn path_differences(part: &Part, output: &Paths, observation: &Observation) -> Vec<String> {
    let Some(expected) = &observation.pdiff else {
        return vec!["the measurement holds no path diff".to_owned()];
    };
    let found = diff_paths(&part.paths, output);
    let mut out = Vec::new();
    let compare = |what: &str, ours: &[String], excel: &[String], out: &mut Vec<String>| {
        for path in ours.iter().filter(|path| !excel.contains(path)) {
            out.push(format!("path {path}: {what} here, not by Excel"));
        }
        for path in excel.iter().filter(|path| !ours.contains(path)) {
            out.push(format!("path {path}: {what} by Excel, not here"));
        }
    };
    compare("added", &found.added, &expected.added, &mut out);
    compare("dropped", &found.dropped, &expected.dropped, &mut out);
    let excel: BTreeMap<&str, (&str, &str)> = expected
        .changed
        .iter()
        .filter(|(path, ..)| !ignored(path))
        .map(|(path, was, now)| (path.as_str(), (was.as_str(), now.as_str())))
        .collect();
    let ours: BTreeMap<&str, (&str, &str)> = found
        .changed
        .iter()
        .filter(|(path, _)| !ignored(path))
        .map(|(path, (was, now))| (path.as_str(), (was.as_str(), now.as_str())))
        .collect();
    if ours != excel {
        out.push(format!("changed paths: Excel {excel:?}, here {ours:?}"));
    }
    out
}

fn compare_report(
    conversion: &ChartConversion,
    observation: &Observation,
    part: &Part,
) -> Vec<String> {
    let mut out = Vec::new();
    let expected_from = expected_source(part);
    if format!("{:?}", conversion.from()) != expected_from {
        out.push(format!(
            "from(): {:?}, measured {expected_from}",
            conversion.from()
        ));
    }
    if format!("{:?}", conversion.to()) != observation.target {
        out.push(format!("to(): {:?}", conversion.to()));
    }
    let changes: Vec<Json> = conversion.changes().iter().map(change_json).collect();
    if changes != observation.changes {
        out.push(format!(
            "changes(): {changes:?}, Excel's slot diff {:?}",
            observation.changes
        ));
    }
    let paint: Vec<Json> = conversion
        .series_paint()
        .iter()
        .map(|paint| {
            Json::List(vec![
                Json::Text(format!("{:?}", paint.part)),
                Json::Text(format!("{:?}", paint.color)),
            ])
        })
        .collect();
    if paint != observation.paint {
        out.push(format!(
            "series_paint(): {paint:?}, measured {:?}",
            observation.paint
        ));
    }
    out
}

/// Converts the observation's part and holds the result against Excel.
fn check(index: usize, observation: &Observation) -> Verdict {
    let part = &measured().parts[observation.part];
    let mut book = open(part);
    let before = chart_part(&book);
    let target = subtype(&observation.target);
    let conversion = match convert(&mut book, target) {
        Ok(conversion) => conversion,
        Err(refusal) => {
            // Pending: refused from the measured source, the chart untouched.
            let mut found = Vec::new();
            if let ChartConversionRefusal::Unsupported {
                from: Some(from),
                to,
            } = refusal
            {
                if format!("{from:?}") != expected_source(part) || to != target {
                    found.push(format!("refused from {from:?} to {to:?}"));
                }
            } else {
                found.push(format!("refused: {refusal:?}"));
            }
            if chart_part(&book) != before {
                found.push("the refusal changed the chart".to_owned());
            }
            return if found.is_empty() {
                Verdict::Pending
            } else {
                Verdict::Fails(found)
            };
        }
    };
    let output = chart_part(&book);
    let mut found = compare_report(&conversion, observation, part);
    found.extend(path_differences(part, &paths_of(&output), observation));
    let (slots, excel) = (topology(&slots_of(&output)), topology(&observation.slots));
    for key in excel
        .keys()
        .chain(slots.keys().filter(|key| !excel.contains_key(*key)))
    {
        if slots.get(key) != excel.get(key) {
            found.push(format!(
                "slot {key}: Excel {:?}, here {:?}",
                excel.get(key),
                slots.get(key)
            ));
        }
    }
    let written = write_workbook(&book);
    let reread = reader::xlsx::read_reader(Cursor::new(written.clone()), true).unwrap();
    if chart_xml(&reread) != output {
        found.push("write, re-read, write changes the chart part".to_owned());
    }
    if found.is_empty() {
        write_witness(index, observation, part, &written);
        Verdict::Matches
    } else {
        Verdict::Fails(found)
    }
}

/// With `CARGO_TARGET_DIR` set, a pair's witness is written as a package Excel
/// opens later, with one line of `expected.tsv`.
fn write_witness(index: usize, observation: &Observation, part: &Part, package: &[u8]) {
    let Some(target_dir) = std::env::var_os("CARGO_TARGET_DIR") else {
        return;
    };
    if !measured().witnesses.contains(&index) {
        return;
    }
    let root = PathBuf::from(target_dir).join("chart-conversion-validation");
    let file = format!(
        "{}/{}__{}.xlsx",
        part.variant,
        source_name(part),
        observation.target
    );
    let path = root.join(&file);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, package).unwrap();
    let line = format!("{file}\t{}\t{}\n", observation.after, part.series);
    fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(root.join("expected.tsv"))
        .unwrap()
        .write_all(line.as_bytes())
        .unwrap();
}

#[test]
fn the_path_ignore_list_is_the_axis_ids() {
    let listed: Vec<&str> = measured()
        .pdiff_ignore
        .iter()
        .map(|(pattern, _)| pattern.as_str())
        .collect();
    assert_eq!(listed, IGNORED);
    for (path, expected) in [
        ("/chartSpace/chart/plotArea/catAx/axId", true),
        ("/chartSpace/chart/plotArea/valAx[1]/crossAx", true),
        ("/chartSpace/chart/plotArea/GROUP/axId[0]", true),
        ("/chartSpace/chart/plotArea/GROUP/axId", true),
        ("/chartSpace/chart/plotArea/catAx/crosses", false),
        ("/chartSpace/chart/plotArea/GROUP/axIds", false),
    ] {
        assert_eq!(ignored(path), expected, "{path}");
    }
}

#[rstest]
fn every_admitted_conversion_matches_excel(
    #[values(
        "ClusteredColumn",
        "StackedColumn",
        "PercentStackedColumn",
        "ClusteredBar",
        "StackedBar",
        "PercentStackedBar",
        "Line",
        "StackedLine",
        "PercentStackedLine",
        "LineWithMarkers",
        "StackedLineWithMarkers",
        "PercentStackedLineWithMarkers",
        "Area",
        "StackedArea",
        "PercentStackedArea",
        "Pie",
        "ExplodedPie",
        "PieOfPie",
        "BarOfPie",
        "Doughnut",
        "ExplodedDoughnut",
        "Scatter",
        "ScatterSmoothWithMarkers",
        "ScatterSmooth",
        "ScatterLinesWithMarkers",
        "ScatterLines",
        "Bubble",
        "Bubble3DEffect",
        "Radar",
        "RadarWithMarkers",
        "FilledRadar",
        "ClusteredColumn3D",
        "StackedColumn3D",
        "PercentStackedColumn3D",
        "Column3D",
        "ClusteredBar3D",
        "StackedBar3D",
        "PercentStackedBar3D",
        "Line3D",
        "Area3D",
        "StackedArea3D",
        "PercentStackedArea3D",
        "Pie3D",
        "ExplodedPie3D",
        "Surface3D",
        "WireframeSurface3D",
        "Contour",
        "WireframeContour",
        "HighLowClose",
        "OpenHighLowClose",
        "VolumeHighLowClose",
        "VolumeOpenHighLowClose",
        "StackedAreaColumn",
        "ColumnLine",
        "ColumnLineSecondary",
        "ColumnColumnSecondary"
    )]
    source: &str,
) {
    let listed = baseline();
    let selected = selected();
    let mut report = String::new();
    let (mut matches, mut pending, mut failures) = (0, 0, 0);
    for (index, observation) in measured().observations.iter().enumerate() {
        let part = &measured().parts[observation.part];
        if observation.operation != "admitted" || source_name(part) != source {
            continue;
        }
        let name = key(part, observation);
        if selected.as_ref().is_some_and(|keys| !keys.contains(&name)) {
            continue;
        }
        let required = selected.is_some() || listed.contains(name.as_str());
        let verdict = match (check(index, observation), required) {
            (Verdict::Pending, true) => Verdict::Fails(vec![
                "refused, but the baseline or CHART_CONVERSION_KEYS lists it as matching"
                    .to_owned(),
            ]),
            (Verdict::Matches, false) => {
                Verdict::Fails(vec!["matches Excel: add it to the baseline".to_owned()])
            }
            (verdict, _) => verdict,
        };
        match verdict {
            Verdict::Matches => {
                matches += 1;
                writeln!(report, "match: {name}").unwrap();
            }
            Verdict::Pending => {
                pending += 1;
                writeln!(report, "pending: {name}").unwrap();
            }
            Verdict::Fails(found) => {
                failures += 1;
                writeln!(report, "FAILS: {name}: {}", found.join("; ")).unwrap();
            }
        }
    }
    println!("{report}{source}: {matches} match Excel, {pending} pending, {failures} fail");
    assert_eq!(
        failures, 0,
        "{source}: {failures} conversions differ from Excel\n{report}"
    );
}

/// Every baseline line, and every key `CHART_CONVERSION_KEYS` selects, names
/// an admitted observation.
#[test]
fn the_baseline_names_admitted_observations() {
    let admitted: BTreeSet<String> = measured()
        .observations
        .iter()
        .filter(|observation| observation.operation == "admitted")
        .map(|observation| key(&measured().parts[observation.part], observation))
        .collect();
    let stale: Vec<&str> = baseline()
        .into_iter()
        .filter(|line| !admitted.contains(*line))
        .collect();
    assert!(stale.is_empty(), "not admitted: {stale:?}");
    let unknown: Vec<String> = selected()
        .unwrap_or_default()
        .into_iter()
        .filter(|line| !admitted.contains(line))
        .collect();
    assert!(unknown.is_empty(), "selected, not admitted: {unknown:?}");
}

/// Excel's own refusals are refused with their cause. A measurement the
/// table guards (a class or shape no fit admits yet) is pending: refused as
/// `Unsupported` with the chart unchanged until a round measures it.
#[test]
fn every_guarded_measurement_is_refused_with_its_listed_cause() {
    let mut found = Vec::new();
    for observation in &measured().observations {
        let Some(cause) = observation.operation.strip_prefix("refused:") else {
            continue;
        };
        let cause = cause.split(':').next().unwrap_or_default();
        let part = &measured().parts[observation.part];
        let mut book = open(part);
        let before = chart_part(&book);
        let name = format!("{} -> {}", part.fixture, observation.target);
        match convert(&mut book, subtype(&observation.target)) {
            Ok(_) => found.push(format!("{name}: converted, Excel refused with {cause}")),
            Err(refusal) => {
                let variant = format!("{refusal:?}");
                if variant.split([' ', '{']).next() != Some(cause) {
                    found.push(format!("{name}: {variant}, Excel refused with {cause}"));
                }
            }
        }
        if chart_part(&book) != before {
            found.push(format!("{name}: the refused conversion changed the chart"));
        }
    }
    assert!(found.is_empty(), "{}", found.join("\n"));
}

/// Two runs on one matching observation per target give two `Ok` reports
/// that agree, and the same bytes. Targets no observation converts yet are
/// printed; all 52 are covered when the conversion is complete.
#[test]
fn conversion_is_deterministic() {
    let listed = baseline();
    let mut found = Vec::new();
    let mut covered: Vec<&str> = Vec::new();
    for observation in &measured().observations {
        let part = &measured().parts[observation.part];
        if covered.contains(&observation.target.as_str())
            || !listed.contains(key(part, observation).as_str())
        {
            continue;
        }
        covered.push(observation.target.as_str());
        let target = subtype(&observation.target);
        let (mut first, mut second) = (open(part), open(part));
        match (convert(&mut first, target), convert(&mut second, target)) {
            (Ok(one), Ok(two)) => {
                if format!("{one:?}") != format!("{two:?}")
                    || chart_part(&first) != chart_part(&second)
                {
                    found.push(format!("{}: two runs differ", key(part, observation)));
                }
            }
            reports => found.push(format!("{}: {reports:?}", key(part, observation))),
        }
    }
    let uncovered: Vec<String> = ChartSubtype::ALL
        .iter()
        .map(|target| format!("{target:?}"))
        .filter(|target| !covered.contains(&target.as_str()))
        .collect();
    println!(
        "determinism: {} of {} targets; not converted yet: {uncovered:?}",
        covered.len(),
        ChartSubtype::COUNT
    );
    assert!(found.is_empty(), "{}", found.join("\n"));
}

/// The values of the first series' categories when they are ISO dates.
fn date_serials(xml: &str) -> Option<Vec<i64>> {
    let start = xml.find("<c:cat>")?;
    let categories = &xml[start..start + xml[start..].find("</c:cat>")?];
    if !categories.contains(r"<c:formatCode>yyyy\-mm\-dd</c:formatCode>") {
        return None;
    }
    categories
        .split("<c:v>")
        .skip(1)
        .map(|value| value[..value.find("</c:v>")?].parse().ok())
        .collect()
}

/// Date categories convert by their spacing: some daily and some monthly
/// ones match Excel.
#[test]
fn date_categories_convert_by_their_spacing() {
    let listed = baseline();
    let epoch = chrono::NaiveDate::from_ymd_opt(1899, 12, 30).unwrap();
    let (mut daily, mut monthly) = (0, 0);
    for observation in &measured().observations {
        let part = &measured().parts[observation.part];
        if !listed.contains(key(part, observation).as_str()) {
            continue;
        }
        let Some(serials) = date_serials(&super::measured::read_part(part)) else {
            continue;
        };
        let dates: Vec<chrono::NaiveDate> = serials
            .iter()
            .map(|serial| epoch + chrono::Days::new(u64::try_from(*serial).unwrap()))
            .collect();
        if serials.windows(2).all(|pair| pair[1] - pair[0] == 1) {
            daily += 1;
        } else if dates.iter().all(|date| chrono::Datelike::day(date) == 1)
            && dates.windows(2).all(|pair| {
                let month = |date: &chrono::NaiveDate| {
                    i64::from(chrono::Datelike::year(date)) * 12
                        + i64::from(chrono::Datelike::month(date))
                };
                month(&pair[1]) - month(&pair[0]) == 1
            })
        {
            monthly += 1;
        }
    }
    println!("date categories matching Excel: {daily} daily, {monthly} monthly");
    assert!(daily > 0 && monthly > 0);
}

#[test]
fn automatic_series_paint_reports_the_series_accent() {
    let part = measured()
        .parts
        .iter()
        .find(|part| part.variant == "bare" && part.subtype.as_deref() == Some("ClusteredColumn"))
        .unwrap();
    let mut book = open(part);
    let conversion = convert(&mut book, subtype("StackedColumn")).unwrap();
    assert_eq!(conversion.series_paint().len(), part.series);
    for paint in conversion.series_paint() {
        assert_eq!(
            format!("{:?}/{:?}", paint.part, paint.color),
            "Fill/SeriesAccent"
        );
    }
}
