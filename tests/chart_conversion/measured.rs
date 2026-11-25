//! The checked-in measurement: what Excel reported for every before part.
//! `measured.json` is stored deflated in a zip, as interned tables that the
//! parts and observations refer to by position; each measurement's before
//! parts are one zip, `before/<measurement>.zip`.
use std::{
    collections::BTreeMap,
    fs,
    io::{
        Cursor,
        Read,
    },
    path::{
        Path,
        PathBuf,
    },
    sync::OnceLock,
};

use serde_json::Value;

use super::projection::{
    Json,
    Paths,
    Slots,
};

pub struct Part {
    /// The part's path, `before/<measurement>/<entry>`: the entry of
    /// `before/<measurement>.zip` under `fixtures()`.
    pub fixture:        String,
    /// The sha256 the measurement recorded for the part's XML.
    pub sha256:         String,
    /// The sha256 of the file the measurement left on disk.
    pub sidecar_sha256: String,
    /// Whether that file went through a text-mode write that turned every line
    /// feed into a carriage return and a line feed.
    pub translated:     bool,
    pub slots:          Slots,
    pub paths:          Paths,
    /// `native`, `combo`, `probe` or `invalid_scatter`: a part of one chart
    /// type, of one of the four combination layouts, of a combination Excel
    /// builds but this operation does not read, or a scatter part the
    /// measurement flagged invalid.
    pub class:          String,
    /// The layout of a `combo` part, as `ComboLayout` spells it in `Debug`.
    pub layout:         Option<String>,
    /// The type Excel reopened the part as, as `ChartSubtype` spells it in
    /// `Debug`. A combination part and a probe have none.
    pub subtype:        Option<String>,
    /// The number of series the part holds.
    pub series:         usize,
    /// The fixture family the part belongs to, such as `bare` or `rich`.
    pub variant:        String,
    /// The cells each reference without a cache reads, as the caches Excel
    /// rebuilt from them show them.
    pub cells:          Vec<Cells>,
}

/// The cells one reference reads.
pub struct Cells {
    /// The reference's formula.
    pub formula: String,
    /// Whether they hold numbers (a `numCache`) or text (a `strCache`).
    pub numeric: bool,
    /// The number format of numbers.
    pub format:  Option<String>,
    /// Each point's position in the reference and its value.
    pub points:  Vec<(u32, String)>,
}

/// One conversion Excel was asked to make.
pub struct Observation {
    /// The part converted, as its position in `Measured::parts`.
    pub part:      usize,
    /// The type asked for, as `ChartSubtype` spells it in `Debug`.
    pub target:    String,
    /// Whether Excel made the conversion.
    pub ok:        bool,
    /// `admitted`, `identity`, `refused:<cause>` or `refused:<cause>:<guard>`.
    pub operation: String,
    /// The type Excel reopened the after part as, as `ChartSubtype` spells it
    /// in `Debug`; empty when Excel refused.
    pub after:     String,
    /// Excel's after part, read the way `slots_of` reads a part.
    pub slots:     Slots,
    /// The element paths Excel added, dropped and changed.
    pub pdiff:     Option<ExpectedPaths>,
    /// The report the conversion gives: `[series or null, part, change]`.
    pub changes:   Vec<Json>,
    /// The paint of each series: `[part, colour]`.
    pub paint:     Vec<Json>,
}

/// Excel's path diff between the before part and its after part.
pub struct ExpectedPaths {
    pub added:   Vec<String>,
    pub dropped: Vec<String>,
    /// Path, before value, after value.
    pub changed: Vec<(String, String, String)>,
}

pub struct Measured {
    pub parts:        Vec<Part>,
    pub observations: Vec<Observation>,
    /// The path patterns whose values a conversion chooses itself: each a
    /// regular expression and why.
    pub pdiff_ignore: Vec<(String, String)>,
    /// Per admitted pair, the observation whose package Excel opens.
    pub witnesses:    Vec<usize>,
}

pub fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/chart_conversion")
}

pub fn measured() -> &'static Measured {
    static MEASURED: OnceLock<Measured> = OnceLock::new();
    MEASURED.get_or_init(load)
}

/// The checked-in before parts by path, read from every zip under `before/`.
pub fn before_parts() -> &'static BTreeMap<String, Vec<u8>> {
    static PARTS: OnceLock<BTreeMap<String, Vec<u8>>> = OnceLock::new();
    PARTS.get_or_init(|| {
        let mut parts = BTreeMap::new();
        for entry in fs::read_dir(fixtures().join("before")).unwrap() {
            let path = entry.unwrap().path();
            let measurement = path
                .file_name()
                .and_then(|name| name.to_str()?.strip_suffix(".zip"))
                .unwrap_or_else(|| panic!("{path:?} is not a measurement's zip"))
                .to_owned();
            let mut archive = zip::ZipArchive::new(Cursor::new(fs::read(&path).unwrap())).unwrap();
            for index in 0..archive.len() {
                let mut file = archive.by_index(index).unwrap();
                let mut bytes = Vec::new();
                file.read_to_end(&mut bytes).unwrap();
                let name = format!("before/{measurement}/{}", file.name().unwrap());
                assert!(
                    parts.insert(name, bytes).is_none(),
                    "{path:?} repeats an entry"
                );
            }
        }
        parts
    })
}

/// The text of a before part.
pub fn read_part(part: &Part) -> String {
    let bytes = before_parts()
        .get(&part.fixture)
        .unwrap_or_else(|| panic!("{} is not checked in", part.fixture));
    String::from_utf8(bytes.clone()).unwrap()
}

/// The text of the measured before part at `fixture`.
pub fn part_xml(fixture: &str) -> String {
    let part = measured()
        .parts
        .iter()
        .find(|part| part.fixture == fixture)
        .unwrap_or_else(|| panic!("no measured part {fixture}"));
    read_part(part)
}

fn load() -> Measured {
    let archive = fs::read(fixtures().join("measured.json.zip")).unwrap();
    let mut archive = zip::ZipArchive::new(Cursor::new(archive)).unwrap();
    let mut bytes = Vec::new();
    archive
        .by_name("measured.json")
        .unwrap()
        .read_to_end(&mut bytes)
        .unwrap();
    let document: Value = serde_json::from_slice(&bytes).unwrap();

    let keys = strings(&document["keys"]);
    let paths = strings(&document["paths"]);
    let values: Vec<Json> = document["values"]
        .as_array()
        .unwrap()
        .iter()
        .map(json)
        .collect();
    let parts = document["parts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|part| Part {
            fixture:        part["fixture"].as_str().unwrap().to_owned(),
            sha256:         part["sha256"].as_str().unwrap().to_owned(),
            sidecar_sha256: part["sidecar_sha256"].as_str().unwrap().to_owned(),
            translated:     part["translated"].as_bool().unwrap(),
            slots:          pairs(&part["slots"])
                .map(|(key, value)| (keys[key].to_owned(), values[value].clone()))
                .collect(),
            paths:          pairs(&part["paths"])
                .map(|(path, value)| match &values[value] {
                    Json::Text(text) => (paths[path].to_owned(), text.clone()),
                    other => panic!("path {} holds {other:?}, not text", paths[path]),
                })
                .collect(),
            class:          part["class"].as_str().unwrap().to_owned(),
            layout:         part["layout"].as_str().map(str::to_owned),
            subtype:        part["subtype"].as_str().map(str::to_owned),
            series:         usize::try_from(part["series"].as_u64().unwrap()).unwrap(),
            variant:        part["variant"].as_str().unwrap_or_default().to_owned(),
            cells:          part
                .get("cells")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .map(cells)
                .collect(),
        })
        .collect();
    let observations = document["observations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|observation| Observation {
            part:      position(&observation["part"]),
            target:    observation["target"].as_str().unwrap().to_owned(),
            ok:        observation["ok"].as_bool().unwrap(),
            operation: observation["operation"]
                .as_str()
                .unwrap_or_default()
                .to_owned(),
            after:     observation["type"].as_str().unwrap_or_default().to_owned(),
            slots:     observation
                .get("slots")
                .map(|slots| {
                    pairs(slots)
                        .map(|(key, value)| (keys[key].to_owned(), values[value].clone()))
                        .collect()
                })
                .unwrap_or_default(),
            pdiff:     observation
                .get("pdiff")
                .filter(|pdiff| pdiff.is_object())
                .map(|pdiff| {
                    let listed = |name: &str| -> Vec<String> {
                        pdiff[name]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|path| paths[position(path)].to_owned())
                            .collect()
                    };
                    let text = |id: &Value| match &values[position(id)] {
                        Json::Text(text) => text.clone(),
                        other => panic!("a changed path holds {other:?}, not text"),
                    };
                    ExpectedPaths {
                        added:   listed("added"),
                        dropped: listed("dropped"),
                        changed: pdiff["changed"]
                            .as_array()
                            .unwrap()
                            .chunks(3)
                            .map(|change| {
                                (
                                    paths[position(&change[0])].to_owned(),
                                    text(&change[1]),
                                    text(&change[2]),
                                )
                            })
                            .collect(),
                    }
                }),
            changes:   ids(observation.get("changes"))
                .map(|id| values[id].clone())
                .collect(),
            paint:     ids(observation.get("paint"))
                .map(|id| values[id].clone())
                .collect(),
        })
        .collect();
    let pdiff_ignore = document["pdiff_ignore"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| {
            (
                entry[0].as_str().unwrap().to_owned(),
                entry[1].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    let witnesses = document["witnesses"]
        .as_array()
        .unwrap()
        .iter()
        .map(position)
        .collect();
    Measured {
        parts,
        observations,
        pdiff_ignore,
        witnesses,
    }
}

/// `[formula, cache kind, format or null, [[index, value], ...]]`.
fn cells(entry: &Value) -> Cells {
    Cells {
        formula: entry[0].as_str().unwrap().to_owned(),
        numeric: match entry[1].as_str().unwrap() {
            "numCache" => true,
            "strCache" => false,
            other => panic!("cells of a {other}"),
        },
        format:  entry[2].as_str().map(str::to_owned),
        points:  entry[3]
            .as_array()
            .unwrap()
            .iter()
            .map(|point| {
                (
                    u32::try_from(point[0].as_u64().unwrap()).unwrap(),
                    point[1].as_str().unwrap().to_owned(),
                )
            })
            .collect(),
    }
}

fn strings(table: &Value) -> Vec<&str> {
    table
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item.as_str().unwrap())
        .collect()
}

/// A flat list `[a, b, a, b, ...]` of table positions as `(a, b)` pairs.
fn pairs(flat: &Value) -> impl Iterator<Item = (usize, usize)> + '_ {
    flat.as_array()
        .unwrap()
        .chunks(2)
        .map(|pair| (position(&pair[0]), position(&pair[1])))
}

/// A list of table positions; none when the field is missing.
fn ids(list: Option<&Value>) -> impl Iterator<Item = usize> + '_ {
    list.and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(position)
}

fn position(id: &Value) -> usize {
    usize::try_from(id.as_u64().unwrap()).unwrap()
}

fn json(value: &Value) -> Json {
    match value {
        Value::Null => Json::Null,
        Value::Number(number) => Json::Count(number.as_u64().expect("a count")),
        Value::String(text) => Json::Text(text.clone()),
        Value::Array(items) => Json::List(items.iter().map(json).collect()),
        Value::Object(entries) => Json::Map(
            entries
                .iter()
                .map(|(key, value)| (key.clone(), json(value)))
                .collect(),
        ),
        Value::Bool(_) => panic!("the measurement's tables hold no booleans"),
    }
}
