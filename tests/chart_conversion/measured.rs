//! The checked-in measurement: what Excel reported for every before part.
//! `measured.json` is stored deflated in a zip, as interned tables that the
//! parts and observations refer to by position.
use std::{
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
    /// The checked-in part, relative to `fixtures()`.
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
}

/// One conversion Excel was asked to make.
pub struct Observation {
    /// The part converted, as its position in `Measured::parts`.
    pub part:   usize,
    /// The type asked for, as `ChartSubtype` spells it in `Debug`.
    pub target: String,
    /// Whether Excel made the conversion.
    pub ok:     bool,
}

pub struct Measured {
    pub parts:        Vec<Part>,
    pub observations: Vec<Observation>,
}

pub fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/chart_conversion")
}

pub fn measured() -> &'static Measured {
    static MEASURED: OnceLock<Measured> = OnceLock::new();
    MEASURED.get_or_init(load)
}

/// The text of a before part.
pub fn read_part(part: &Part) -> String {
    String::from_utf8(fs::read(fixtures().join(&part.fixture)).unwrap()).unwrap()
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
        })
        .collect();
    let observations = document["observations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|observation| Observation {
            part:   position(&observation["part"]),
            target: observation["target"].as_str().unwrap().to_owned(),
            ok:     observation["ok"].as_bool().unwrap(),
        })
        .collect();
    Measured {
        parts,
        observations,
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
