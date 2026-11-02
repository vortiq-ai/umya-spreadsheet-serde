#![cfg(feature = "serde")]

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
};

#[path = "support/canonical_xml.rs"]
mod canonical_xml;

use canonical_xml::{
    XmlToken,
    canonical_xml,
};
use sha2::{
    Digest,
    Sha256,
};
use umya_spreadsheet::{
    Chart,
    Workbook,
    reader,
    writer,
};

fn sha256(path: &Path) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::new();
    for byte in Sha256::digest(fs::read(path).unwrap()) {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 15)]));
    }
    encoded
}

fn drawing_parts(book: &Workbook) -> BTreeMap<String, Vec<XmlToken>> {
    let mut bytes = Vec::new();
    writer::xlsx::write_writer(book, &mut bytes).unwrap();
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut parts = BTreeMap::new();
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).unwrap();
        let name = entry.name().unwrap().to_string();
        if (name.starts_with("xl/charts/") || name.starts_with("xl/drawings/"))
            && name.ends_with(".xml")
        {
            let mut xml = Vec::new();
            entry.read_to_end(&mut xml).unwrap();
            parts.insert(name, canonical_xml(&xml));
        }
    }
    assert!(!parts.is_empty(), "fixture has no chart or drawing XML");
    parts
}

fn compare_parts(actual: &Workbook, expected: &Workbook, fixture: &Path) -> Result<(), String> {
    let actual = drawing_parts(actual);
    let expected = drawing_parts(expected);
    if actual.keys().ne(expected.keys()) {
        return Err(format!("{} part names differ", fixture.display()));
    }
    for (name, actual) in actual {
        let expected = &expected[&name];
        let mismatch = actual.iter().zip(expected).position(|(a, b)| a != b);
        if let Some(index) = mismatch {
            let common_tail = actual[index..]
                .iter()
                .rev()
                .zip(expected[index..].iter().rev())
                .take_while(|(a, b)| a == b)
                .count();
            return Err(format!(
                "{} {name} token {index}: actual {:?}, expected {:?}",
                fixture.display(),
                &actual[index..actual.len() - common_tail],
                &expected[index..expected.len() - common_tail]
            ));
        }
        if actual.len() != expected.len() {
            return Err(format!("{} {name} token count differs", fixture.display()));
        }
    }
    Ok(())
}

fn fixtures() -> Vec<(PathBuf, serde_json::Value)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/persisted_charts");
    let mut fixtures = Vec::new();
    for entry in fs::read_dir(root).unwrap() {
        let directory = entry.unwrap().path();
        let provenance = directory.join("provenance.json");
        if provenance.is_file() {
            let manifest: serde_json::Value =
                serde_json::from_slice(&fs::read(provenance).unwrap()).unwrap();
            for case in manifest["cases"].as_array().expect("provenance cases") {
                fixtures.push((directory.clone(), case.clone()));
            }
        }
    }
    assert!(
        !fixtures.is_empty(),
        "no generated historical chart fixtures"
    );
    fixtures.sort_by_key(|(path, case)| (path.clone(), case["json"].as_str().unwrap().to_owned()));
    fixtures
}

fn check_fixture(directory: &Path, case: &serde_json::Value) -> Result<(), String> {
    let json = directory.join(case["json"].as_str().unwrap());
    let export = directory.join(case["export"].as_str().unwrap());
    let charts: Vec<Chart> = serde_json::from_slice(&fs::read(&json).unwrap())
        .map_err(|error| format!("{}: {error}", json.display()))?;
    assert!(!charts.is_empty(), "{} contains no charts", json.display());
    let source = fs::read(export).unwrap();
    let mut oracle = reader::xlsx::read_reader(Cursor::new(&source), true).unwrap();
    let mut restored = umya_spreadsheet::new_file();
    if let Some(drawing) = case["drawing_json"].as_str() {
        let drawing = serde_json::from_slice(&fs::read(directory.join(drawing)).unwrap())
            .map_err(|error| format!("{}: {error}", directory.join(drawing).display()))?;
        restored
            .sheet_mut(0)
            .unwrap()
            .set_worksheet_drawing(drawing);
    }
    *restored.sheet_mut(0).unwrap().chart_collection_mut() = charts;
    // Exercise the ordinary edit/save route identically in both branches.
    for book in [&mut oracle, &mut restored] {
        book.sheet_mut(0)
            .unwrap()
            .cell_mut("Z100")
            .set_value_string("edited after restore");
    }
    compare_parts(&restored, &oracle, &json)?;
    let binary = bincode::serialize(oracle.sheet(0).unwrap().chart_collection()).unwrap();
    let decoded: Vec<Chart> = bincode::deserialize(&binary).unwrap();
    assert_eq!(
        binary,
        bincode::serialize(&decoded).unwrap(),
        "{}",
        json.display()
    );
    let mut bytes = Vec::new();
    writer::xlsx::write_writer(&restored, &mut bytes).unwrap();
    let reread = reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap();
    compare_parts(&reread, &oracle, &json)
}

#[test]
fn historical_charts_keep_old_writer_drawingml_meaning() {
    let mut checked = 0;
    let mut failures = Vec::new();
    for (directory, case) in fixtures() {
        if let Err(error) = check_fixture(&directory, &case) {
            failures.push(error);
        }
        checked += 1;
    }
    assert_eq!(checked, 145);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn charts_with_legend_presence_keep_old_writer_drawingml_meaning() {
    let mut checked = 0;
    for (directory, case) in fixtures() {
        let json = fs::read(directory.join(case["json"].as_str().unwrap())).unwrap();
        if !json
            .windows(b"\"legend_present\"".len())
            .any(|value| value == b"\"legend_present\"")
        {
            continue;
        }
        if !matches!(
            case["name"].as_str().unwrap(),
            "gradient_stops" | "tile_rectangle"
        ) {
            check_fixture(&directory, &case).unwrap();
            checked += 1;
        }
    }
    assert!(checked > 0);
}

#[test]
fn historical_chart_colours_and_text_keep_old_writer_drawingml_meaning() {
    let mut checked = 0;
    for (directory, case) in fixtures() {
        if !matches!(
            case["name"].as_str().unwrap(),
            "standard"
                | "scheme_transforms"
                | "rgb_transforms"
                | "preset_alpha"
                | "pattern_colours"
                | "shape_auto_fit"
                | "normal_auto_fit"
                | "no_auto_fit"
        ) {
            continue;
        }
        let json = fs::read(directory.join(case["json"].as_str().unwrap())).unwrap();
        if !json
            .windows(b"\"legend_present\"".len())
            .any(|value| value == b"\"legend_present\"")
        {
            continue;
        }
        check_fixture(&directory, &case).unwrap();
        checked += 1;
    }
    assert!(checked >= 8);
}

#[test]
fn charts_without_persisted_legend_presence_restore_old_writer_legend() {
    let mut checked = 0;
    for (directory, case) in fixtures() {
        if case["name"].as_str().unwrap() != "standard" {
            continue;
        }
        let json = directory.join(case["json"].as_str().unwrap());
        let bytes = fs::read(&json).unwrap();
        if bytes
            .windows(b"\"legend_present\"".len())
            .any(|value| value == b"\"legend_present\"")
        {
            continue;
        }
        let export = fs::read(directory.join(case["export"].as_str().unwrap())).unwrap();
        let oracle = reader::xlsx::read_reader(Cursor::new(export), true).unwrap();
        assert!(
            oracle.sheet(0).unwrap().chart_collection()[0]
                .chart_space()
                .chart()
                .legend_present()
        );
        check_fixture(&directory, &case).unwrap();
        checked += 1;
    }
    assert!(checked > 0);
}

#[test]
fn historical_gradient_positions_keep_old_writer_drawingml_meaning() {
    let mut checked = 0;
    for (directory, case) in fixtures() {
        if matches!(
            case["name"].as_str().unwrap(),
            "gradient_stops" | "tile_rectangle"
        ) {
            check_fixture(&directory, &case).unwrap();
            checked += 1;
        }
    }
    assert!(checked > 0);
}

#[test]
fn generated_fixtures_match_their_recorded_provenance() {
    let generator =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/legacy-chart-json/src/main.rs");
    let source_hash = sha256(&generator);
    for (directory, case) in fixtures() {
        let provenance: serde_json::Value =
            serde_json::from_slice(&fs::read(directory.join("provenance.json")).unwrap()).unwrap();
        assert_eq!(
            provenance["generator_source_sha256"].as_str().unwrap(),
            source_hash
        );
        let input = directory.join(case["input"].as_str().unwrap());
        assert_eq!(sha256(&input), case["input_sha256"].as_str().unwrap());
        for kind in ["json", "export", "drawing_json"] {
            let output = directory.join(case[kind].as_str().unwrap());
            assert_eq!(
                sha256(&output),
                case["output_sha256"][kind].as_str().unwrap()
            );
        }
    }
}

#[test]
fn generated_fixtures_cover_every_available_historical_schema_class() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let history: serde_json::Value = serde_json::from_slice(
        &fs::read(root.join("tools/legacy-chart-json/schema-history/classes.json")).unwrap(),
    )
    .unwrap();
    let fixtures = root.join("tests/fixtures/persisted_charts");
    let input_count = fs::read_dir(fixtures.join("inputs"))
        .unwrap()
        .map(Result::unwrap)
        .filter(|entry| {
            entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "xlsx")
        })
        .count();
    for class in history["classes"].as_array().unwrap() {
        let name = &class["sha256"].as_str().unwrap()[..12];
        let provenance: serde_json::Value =
            serde_json::from_slice(&fs::read(fixtures.join(name).join("provenance.json")).unwrap())
                .unwrap();
        assert_eq!(provenance["shape_class"].as_str().unwrap(), name);
        assert!(
            class["pins"]
                .as_array()
                .unwrap()
                .contains(&provenance["pin"])
        );
        assert_eq!(provenance["cases"].as_array().unwrap().len(), input_count);
    }
}

#[test]
fn current_reader_charts_and_drawings_keep_derived_binary_bytes() {
    use umya_spreadsheet::drawing::spreadsheet::WorksheetDrawing;
    for (directory, case) in fixtures() {
        let source = fs::read(directory.join(case["export"].as_str().unwrap())).unwrap();
        let book = reader::xlsx::read_reader(Cursor::new(source), true).unwrap();
        let drawing = book.sheet(0).unwrap().worksheet_drawing();
        let binary = bincode::serialize(drawing).unwrap();
        let decoded: WorksheetDrawing = bincode::deserialize(&binary).unwrap();
        assert_eq!(
            binary,
            bincode::serialize(&decoded).unwrap(),
            "{}",
            directory.display()
        );
        let charts = book.sheet(0).unwrap().chart_collection();
        let binary = bincode::serialize(charts).unwrap();
        let decoded: Vec<Chart> = bincode::deserialize(&binary).unwrap();
        assert_eq!(
            binary,
            bincode::serialize(&decoded).unwrap(),
            "{}",
            directory.display()
        );
    }
}

fn legacy_colour(value: &serde_json::Value, key: &str) -> Option<serde_json::Value> {
    match value {
        serde_json::Value::Object(fields) => {
            if fields.get(key).is_some_and(|value| !value.is_null()) && fields.contains_key("val") {
                return Some(value.clone());
            }
            fields.values().find_map(|value| legacy_colour(value, key))
        }
        serde_json::Value::Array(values) => {
            values.iter().find_map(|value| legacy_colour(value, key))
        }
        _ => None,
    }
}

#[test]
fn generated_colours_restore_every_legacy_transform_in_old_writer_order() {
    use umya_spreadsheet::drawing::{
        PresetColor,
        RgbColorModelHex,
        SchemeColor,
    };
    let expected = [
        ("lum", Some("21000")),
        ("lumMod", Some("32000")),
        ("lumOff", Some("43000")),
        ("sat", Some("54000")),
        ("satMod", Some("65000")),
        ("shade", Some("76000")),
        ("alpha", Some("87000")),
        ("tint", Some("98000")),
    ];
    let mut checked = 0;
    for (directory, case) in fixtures() {
        let json = directory.join(case["json"].as_str().unwrap());
        let value: serde_json::Value = serde_json::from_slice(&fs::read(&json).unwrap()).unwrap();
        let transforms = match case["name"].as_str().unwrap() {
            "scheme_transforms" => {
                let value =
                    legacy_colour(&value, "luminance").expect("populated legacy scheme colour");
                serde_json::from_value::<SchemeColor>(value)
                    .unwrap()
                    .transforms()
                    .to_vec()
            }
            "rgb_transforms" => {
                let value =
                    legacy_colour(&value, "luminance").expect("populated legacy RGB colour");
                serde_json::from_value::<RgbColorModelHex>(value)
                    .unwrap()
                    .transforms()
                    .to_vec()
            }
            "preset_alpha" => {
                let value = legacy_colour(&value, "alpha").expect("populated legacy preset colour");
                let colour = serde_json::from_value::<PresetColor>(value).unwrap();
                assert_eq!(colour.transforms().len(), 1);
                assert_eq!(colour.transforms()[0].xml_name(), "alpha");
                assert_eq!(colour.transforms()[0].value(), Some("67000"));
                checked += 1;
                continue;
            }
            _ => continue,
        };
        assert_eq!(
            transforms
                .iter()
                .map(|value| (value.xml_name(), value.value()))
                .collect::<Vec<_>>(),
            expected,
            "{}",
            json.display()
        );
        checked += 1;
    }
    assert!(checked >= 3);
}

#[test]
fn chart_space_parts_added_later_default_when_the_persisted_chart_lacks_them() {
    use umya_spreadsheet::drawing::charts::{
        ChartSpace,
        PrintSettings,
    };
    let charts: serde_json::Value = serde_json::from_slice(include_bytes!(
        "fixtures/persisted_charts/7fd0288662ac/standard.json"
    ))
    .unwrap();
    let space = charts[0]["two_cell_anchor"]["graphic_frame"]["graphic"]["graphic_data"]
        ["chart_space"]
        .clone();
    for key in [
        "pivot_source",
        "text_properties",
        "external_data",
        "user_shapes",
        "color_map_override",
        "protection",
        "pivot_options",
    ] {
        assert!(space.get(key).is_none(), "{key} predates this revision");
    }
    let plot = &space["chart"]["plot_area"];
    assert!(plot.get("data_table").is_none() && plot.get("extensions").is_none());
    let space: ChartSpace = serde_json::from_value(space).unwrap();
    assert!(space.pivot_source().is_none());
    assert!(space.text_properties().is_none());
    assert!(space.external_data().is_none());
    assert!(space.user_shapes().is_none());
    assert!(space.chart().plot_area().data_table().is_none());
    assert!(space.chart().plot_area().extensions().is_none());
    assert!(space.color_map_override().is_none());
    assert!(space.protection().is_none());
    assert!(space.pivot_options().is_none());

    // Print settings persisted before the parts became optional held the
    // header/footer and page setup as empty objects and the margins in full.
    let margins = serde_json::to_value({
        let mut settings = PrintSettings::default();
        settings.page_margins_mut().set_left(0.7).set_top(0.75);
        settings
    })
    .unwrap()["page_margins"]
        .clone();
    let old: PrintSettings = serde_json::from_value(serde_json::json!({
        "header_footer": {},
        "page_margins": margins,
        "page_setup": {},
    }))
    .unwrap();
    let header_footer = old.header_footer().unwrap();
    assert_eq!(header_footer.odd_header(), None);
    assert_eq!(header_footer.align_with_margins(), None);
    assert_eq!(old.page_setup().unwrap().orientation(), None);
    assert!((old.page_margins().unwrap().left() - 0.7).abs() < 1e-12);
    assert!(old.legacy_drawing_header_footer().is_none());
}

#[test]
fn series_values_without_a_literal_slot_keep_their_reference() {
    use umya_spreadsheet::drawing::charts::Values;
    let mut reference =
        serde_json::to_value(umya_spreadsheet::drawing::charts::NumberReference::default())
            .unwrap();
    reference.as_object_mut().unwrap().remove("pivot_cache");
    let mut old: Values =
        serde_json::from_value(serde_json::json!({"number_reference": reference})).unwrap();
    assert!(old.number_literal().is_none());
    old.number_reference_mut()
        .formula_mut()
        .set_address_str("Pivot!$B$4:$B$6");
    assert_eq!(
        old.number_reference().formula().address_str(),
        "Pivot!$B$4:$B$6"
    );
}
