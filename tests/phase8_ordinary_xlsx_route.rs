#![cfg(feature = "serde")]
//! Ordinary XLSX path: create a real chart with the library writer, add the
//! fixture XML nodes to that XLSX, then import/export/re-read with the library.
use std::io::{Cursor, Read};
use umya_spreadsheet::{new_file, Chart, ChartType, Workbook};
use zip::{write::SimpleFileOptions, ZipArchive, ZipWriter};

fn save(book: &Workbook) -> Vec<u8> {
    let mut bytes = Vec::new();
    umya_spreadsheet::writer::xlsx::write_writer(book, &mut Cursor::new(&mut bytes)).unwrap();
    bytes
}
fn record_output(bytes: &[u8], case: &str, route: &str) {
    use sha2::{Digest, Sha256};
    if let Some(dir) = std::env::var_os("PHASE8_OUTPUT_DIR") {
        let source = std::env::var("PHASE8_SOURCE_ID").expect("source identity required when capturing outputs");
        let run = std::env::var("PHASE8_RUN_ID").expect("run identity required when capturing outputs");
        assert_eq!(source.len(), 64);
        assert!(source.bytes().all(|b| b.is_ascii_hexdigit()));
        assert_eq!(run.len(), 64); assert!(run.bytes().all(|b| b.is_ascii_hexdigit()));
        let dir = std::path::PathBuf::from(dir); std::fs::create_dir_all(&dir).unwrap();
        let stem = format!("{case}--{route}");
        std::fs::write(dir.join(format!("{stem}.xlsx")), bytes).unwrap();
        let digest: String = Sha256::digest(bytes).iter().map(|byte| format!("{byte:02x}")).collect();
        std::fs::write(dir.join(format!("{stem}.receipt")), format!("{}\n{}\n{}\n", source.to_ascii_lowercase(), run.to_ascii_lowercase(), digest)).unwrap();
    }
}
fn assert_export_rejected(book: &Workbook) {
    let mut bytes = Vec::new();
    let error = umya_spreadsheet::writer::xlsx::write_writer(book, &mut Cursor::new(&mut bytes)).unwrap_err();
    assert!(error.to_string().contains("chart export rejected"), "{error}");
    assert!(bytes.is_empty(), "rejection must precede external output");
}
fn assert_file_rejected(book: &Workbook) {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let root = std::env::temp_dir().join(format!("phase8-refusal-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    std::fs::create_dir(&root).unwrap();
    let path = root.join("existing.xlsx"); let temporary = root.join("existing.xlsxtmp");
    std::fs::write(&path, b"original destination").unwrap();
    std::fs::write(&temporary, b"original temporary").unwrap();
    assert!(umya_spreadsheet::writer::xlsx::write(book, &path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"original destination");
    assert_eq!(std::fs::read(&temporary).unwrap(), b"original temporary");
    std::fs::remove_file(path).unwrap(); std::fs::remove_file(temporary).unwrap(); std::fs::remove_dir(root).unwrap();
}
fn load(bytes: &[u8]) -> Workbook {
    umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap()
}
fn chart(book: &Workbook) -> &Chart {
    book.sheet_collection()
        .iter()
        .flat_map(|s| s.chart_collection())
        .next()
        .unwrap()
}
fn chart_mut(book: &mut Workbook) -> &mut Chart {
    book.sheet_collection_mut().iter_mut().flat_map(|s| s.chart_collection_mut()).next().unwrap()
}
fn chart_xml(bytes: &[u8]) -> String {
    let mut z = ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut s = String::new();
    z.by_name("xl/charts/chart1.xml")
        .unwrap()
        .read_to_string(&mut s)
        .unwrap();
    s
}
fn with_chart_xml(bytes: &[u8], new_chart: &str) -> Vec<u8> {
    let mut input = ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut out = ZipWriter::new(Cursor::new(Vec::new()));
    for i in 0..input.len() {
        let mut f = input.by_index(i).unwrap();
        let name = f.name().to_owned();
        let opts = SimpleFileOptions::default().compression_method(f.compression());
        let mut body = Vec::new();
        f.read_to_end(&mut body).unwrap();
        if name == "xl/charts/chart1.xml" {
            body = new_chart.as_bytes().to_vec()
        }
        out.start_file(name, opts).unwrap();
        std::io::Write::write_all(&mut out, &body).unwrap();
    }
    out.finish().unwrap().into_inner()
}
fn fixture(unit: &str) -> Vec<u8> {
    fixture_with_unit(&format!("<c:builtInUnit val=\"{unit}\"/>"))
}
fn fixture_with_unit(unit_node: &str) -> Vec<u8> {
    let mut book = new_file();
    for row in 1..=3 {
        book.sheet_mut(0)
            .unwrap()
            .cell_mut((2, row))
            .set_value_number(row * 10);
    }
    let mut from = umya_spreadsheet::drawing::spreadsheet::MarkerType::default();
    from.set_coordinate("E1");
    let mut to = umya_spreadsheet::drawing::spreadsheet::MarkerType::default();
    to.set_coordinate("M20");
    let mut chart = Chart::default();
    chart.new_chart(&ChartType::LineChart, from, to, vec!["Sheet1!B1:B3"]);
    book.sheet_mut(0).unwrap().add_chart(chart);
    let bytes = save(&book);
    let mut xml = chart_xml(&bytes);
    let nodes=format!("<c:pivotFmts><c:pivotFmt><c:idx val=\"7\"/><c:dLbl><c:idx val=\"7\"/><c:tx><c:strRef><c:f>Sheet1!$B$1</c:f></c:strRef></c:tx></c:dLbl></c:pivotFmt></c:pivotFmts>");
    let plot = "<c:plotArea";
    assert!(xml.contains(plot));
    xml = xml.replacen(plot, &format!("{nodes}{plot}"), 1);
    let axis = "</c:valAx>";
    assert!(xml.contains(axis));
    let du=format!("<c:dispUnits>{unit_node}<c:dispUnitsLbl><c:tx><c:strRef><c:f>Sheet1!$A$1</c:f></c:strRef></c:tx></c:dispUnitsLbl></c:dispUnits>");
    xml = xml.replacen(axis, &format!("{du}{axis}"), 1);
    with_chart_xml(&bytes, &xml)
}
#[test]
fn all_unit_and_pivot_references_round_trip_through_real_xlsx() {
    for unit in [
        "hundreds",
        "thousands",
        "tenThousands",
        "hundredThousands",
        "millions",
        "tenMillions",
        "hundredMillions",
        "billions",
        "trillions",
    ] {
        let input = fixture(unit);
        let imported = load(&input);
        let model = chart(&imported).chart_space().chart();
        let p = model.pivot_formats().unwrap();
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].index().val(), 7);
        assert_eq!(
            p[0].label()
                .unwrap()
                .text()
                .unwrap()
                .string_reference()
                .unwrap()
                .formula()
                .address_str(),
            "Sheet1!$B$1"
        );
        let axis = &model.plot_area().value_axis()[0];
        let du = axis.display_units().unwrap();
        assert!(du.has_valid_required_choice());
        assert_eq!(
            du.label()
                .unwrap()
                .text()
                .unwrap()
                .string_reference()
                .unwrap()
                .formula()
                .address_str(),
            "Sheet1!$A$1"
        );
        let json = serde_json::to_value(model).unwrap();
        let json_model: umya_spreadsheet::drawing::charts::Chart = serde_json::from_value(json).unwrap();
        let mut json_book = load(&input);
        chart_mut(&mut json_book).chart_space_mut().set_chart(json_model);
        let json_output = save(&json_book);
        record_output(&json_output, &format!("unit-{unit}"), "json");
        assert!(chart_xml(&json_output).contains(&format!("<c:builtInUnit val=\"{unit}\"/>")));
        assert_eq!(chart(&load(&json_output)).chart_space().chart().pivot_formats().unwrap()[0].index().val(), 7);
        let mut wire: umya_spreadsheet::drawing::charts::Chart =
            bincode::deserialize(&bincode::serialize(model).unwrap()).unwrap();
        let mut updated = 0;
        for formula in wire.formula_mut() {
            match formula.address_str().as_str() {
                "Sheet1!$A$1" => {
                    formula.set_address_str("Sheet1!$A$9");
                    updated += 1;
                }
                "Sheet1!$B$1" => {
                    formula.set_address_str("Sheet1!$B$9");
                    updated += 1;
                }
                _ => {}
            }
        }
        assert_eq!(
            updated, 2,
            "phase8 refs must participate in normal formula mutation exactly once"
        );
        let mutated_json = serde_json::to_value(&wire).unwrap();
        let mutated: umya_spreadsheet::drawing::charts::Chart =
            serde_json::from_value(mutated_json).unwrap();
        assert_eq!(
            mutated.pivot_formats().unwrap()[0]
                .label()
                .unwrap()
                .text()
                .unwrap()
                .string_reference()
                .unwrap()
                .formula()
                .address_str(),
            "Sheet1!$B$9"
        );
        let mut wire_book = load(&input);
        chart_mut(&mut wire_book).chart_space_mut().set_chart(wire);
        let wire_output = save(&wire_book);
        record_output(&wire_output, &format!("unit-{unit}"), "bincode");
        let wire_xml = chart_xml(&wire_output);
        assert!(wire_xml.contains("Sheet1!$A$9"));
        assert!(wire_xml.contains("Sheet1!$B$9"));
        let wire_reload = load(&wire_output);
        assert_eq!(chart(&wire_reload).chart_space().chart().pivot_formats().unwrap()[0]
            .label().unwrap().text().unwrap().string_reference().unwrap().formula().address_str(), "Sheet1!$B$9");
        let output = save(&imported);
        record_output(&output, &format!("unit-{unit}"), "original");
        let output_xml = chart_xml(&output);
        assert!(output_xml.contains(&format!("<c:builtInUnit val=\"{unit}\"/>")));
        assert!(output_xml.contains("<c:pivotFmts>"));
        let reloaded = load(&output);
        assert_eq!(
            chart(&reloaded)
                .chart_space()
                .chart()
                .pivot_formats()
                .unwrap()[0]
                .index()
                .val(),
            7
        );
    }
}

#[test]
fn invalid_choices_and_missing_pivot_index_are_not_repaired() {
    let unknown = fixture("not-an-xsd-unit");
    let imported = load(&unknown);
    let units = chart(&imported)
        .chart_space()
        .chart()
        .plot_area()
        .value_axis()[0]
        .display_units()
        .unwrap();
    assert!(!units.has_valid_required_choice());
    assert_export_rejected(&imported);

    let good = fixture("thousands");
    let good_xml = chart_xml(&good);
    let no_choice_xml = good_xml.replace("<c:builtInUnit val=\"thousands\"/>", "");
    assert_ne!(good_xml, no_choice_xml);
    let no_choice = load(&with_chart_xml(&good, &no_choice_xml));
    assert!(!chart(&no_choice)
        .chart_space()
        .chart()
        .plot_area()
        .value_axis()[0]
        .display_units()
        .unwrap()
        .has_valid_required_choice());
    assert_export_rejected(&no_choice);

    let missing_idx_xml = good_xml.replace("<c:idx val=\"7\"/>", "<c:idx/>");
    assert_ne!(good_xml, missing_idx_xml);
    let missing_idx = load(&with_chart_xml(&good, &missing_idx_xml));
    let pivot = &chart(&missing_idx)
        .chart_space()
        .chart()
        .pivot_formats()
        .unwrap()[0];
    assert!(!pivot.index_is_present());
    assert_export_rejected(&missing_idx);
}

#[test]
fn custom_display_unit_retains_lexical_value_through_xlsx() {
    let input = fixture_with_unit("<c:custUnit val=\"1.00E+3\"/>");
    let imported = load(&input);
    let units = chart(&imported)
        .chart_space()
        .chart()
        .plot_area()
        .value_axis()[0]
        .display_units()
        .unwrap();
    let umya_spreadsheet::drawing::charts::DisplayUnit::Custom(value) = units.unit() else {
        panic!("custUnit choice changed");
    };
    assert_eq!(value.attribute(), Some("1.00E+3"));
    assert_eq!(value.value(), Some(1000.0));
    assert!(chart_xml(&save(&imported)).contains("<c:custUnit val=\"1.00E+3\"/>"));
}

#[test]
fn scalar_empty_and_expanded_forms_import_with_explicit_presence() {
    let chart_ns = "http://schemas.openxmlformats.org/drawingml/2006/chart";
    for (case, node) in [
        ("builtin-empty", "<c:builtInUnit val=\"millions\"/>".to_owned()),
        ("builtin-expanded", "<c:builtInUnit val=\"millions\"></c:builtInUnit>".to_owned()),
        ("custom-expanded", "<c:custUnit val=\"1.00E+3\"></c:custUnit>".to_owned()),
        ("builtin-alias", format!("<alt:builtInUnit xmlns:alt=\"{chart_ns}\" val=\"millions\"/>")),
        ("custom-alias", format!("<alt:custUnit xmlns:alt=\"{chart_ns}\" val=\"1.00E+3\"></alt:custUnit>")),
        ("builtin-default-ns", format!("<builtInUnit xmlns=\"{chart_ns}\" val=\"millions\"></builtInUnit>")),
    ] {
        let book = load(&fixture_with_unit(&node));
        let units = chart(&book).chart_space().chart().plot_area().value_axis()[0].display_units().unwrap();
        assert!(units.has_valid_required_choice(), "{node}");
        assert!(!units.has_conflict(), "{node}");
        let output = save(&book);
        record_output(&output, &format!("scalar-{case}"), "original");
        let roundtrip = load(&output);
        assert!(chart(&roundtrip).chart_space().chart().plot_area().value_axis()[0]
            .display_units().unwrap().has_valid_required_choice());
        let model = chart(&book).chart_space().chart();
        let json: umya_spreadsheet::drawing::charts::Chart = serde_json::from_slice(&serde_json::to_vec(model).unwrap()).unwrap();
        let bin: umya_spreadsheet::drawing::charts::Chart = bincode::deserialize(&bincode::serialize(model).unwrap()).unwrap();
        for (route, restored) in [("json", json), ("bincode", bin)] {
            let mut out = load(&fixture_with_unit(&node));
            chart_mut(&mut out).chart_space_mut().set_chart(restored);
            let bytes = save(&out); record_output(&bytes, &format!("scalar-{case}"), route);
            assert!(chart(&load(&bytes)).chart_space().chart().plot_area().value_axis()[0].display_units().unwrap().has_valid_required_choice());
        }
    }
    for node in ["<c:custUnit/>", "<c:custUnit val=\"bad\"/>", "<c:builtInUnit val=\"bad\"/>"] {
        let book = load(&fixture_with_unit(node));
        assert!(!chart(&book).chart_space().chart().plot_area().value_axis()[0]
            .display_units().unwrap().has_valid_required_choice());
    }
}

#[test]
fn invalid_order_and_duplicate_indices_are_retained_and_diagnosed_after_codecs() {
    let source = fixture_with_unit("<c:builtInUnit val=\"thousands\"/><c:extLst/>");
    let xml = chart_xml(&source).replace("<c:idx val=\"7\"/>", "<c:idx val=\"7\"/><c:idx val=\"8\"/>");
    let source = with_chart_xml(&source, &xml);
    let book = load(&source);
    let model = chart(&book).chart_space().chart();
    assert_eq!(model.phase8_conflicts().len(), 2);
    assert_eq!(model.pivot_formats().unwrap()[0].duplicate_indices(), &[Some("8".to_owned())]);
    assert!(!model.pivot_formats().unwrap()[0].index_is_present());
    for restored in [
        serde_json::from_slice::<umya_spreadsheet::drawing::charts::Chart>(&serde_json::to_vec(model).unwrap()).unwrap(),
        bincode::deserialize::<umya_spreadsheet::drawing::charts::Chart>(&bincode::serialize(model).unwrap()).unwrap(),
    ] {
        assert_eq!(restored.phase8_conflicts().len(), 2);
        assert_eq!(restored.pivot_formats().unwrap()[0].duplicate_indices(), &[Some("8".to_owned())]);
        let mut restored_book = load(&source);
        chart_mut(&mut restored_book).chart_space_mut().set_chart(restored);
        assert_export_rejected(&restored_book);
    }
    assert_export_rejected(&book);
}

#[test]
fn every_workbook_writer_rejects_before_stream_or_existing_file_changes() {
    let source = fixture("bad-unit");
    let book = load(&source);
    let mut bytes = Vec::new();
    let err = umya_spreadsheet::writer::xlsx::write_writer_light(&book, &mut Cursor::new(&mut bytes)).unwrap_err();
    assert!(err.to_string().contains("chart export rejected"));
    assert!(bytes.is_empty());
    let root = std::env::temp_dir().join(format!("phase8-invalid-{}-{:?}", std::process::id(), std::thread::current().id()));
    std::fs::create_dir(&root).unwrap();
    let path = root.join("existing.xlsx");
    let temp_path = root.join("existing.xlsxtmp");
    std::fs::write(&path, b"original destination").unwrap();
    std::fs::write(&temp_path, b"original temporary file").unwrap();
    for result in [
        umya_spreadsheet::writer::xlsx::write(&book, &path),
        umya_spreadsheet::writer::xlsx::write_light(&book, &path),
        umya_spreadsheet::writer::xlsx::write_with_password(&book, &path, "test-password"),
        umya_spreadsheet::writer::xlsx::write_with_password_light(&book, &path, "test-password"),
    ] {
        assert!(result.unwrap_err().to_string().contains("chart export rejected"));
        assert_eq!(std::fs::read(&path).unwrap(), b"original destination");
        assert_eq!(std::fs::read(&temp_path).unwrap(), b"original temporary file");
    }
    std::fs::remove_file(path).unwrap();
    std::fs::remove_file(temp_path).unwrap();
    std::fs::remove_dir(root).unwrap();
}

#[test]
fn duplicate_choices_and_unknown_subtree_remain_diagnosed_through_codecs() {
    let source = fixture_with_unit("<c:builtInUnit val=\"millions\"/><c:custUnit val=\"2.00\"/><c:unknown><c:builtInUnit val=\"billions\"/></c:unknown>");
    let book = load(&source);
    let model = chart(&book).chart_space().chart();
    let units = model.plot_area().value_axis()[0].display_units().unwrap();
    assert!(units.has_conflict());
    assert_eq!(units.extra_units().len(), 1);
    let umya_spreadsheet::drawing::charts::DisplayUnit::Custom(custom) = &units.extra_units()[0] else { panic!("duplicate choice lost") };
    assert_eq!(custom.attribute(), Some("2.00"));
    assert_eq!(units.effective_builtin(), Some(umya_spreadsheet::drawing::charts::BuiltInUnit::Millions));
    for model in [
        serde_json::from_slice::<umya_spreadsheet::drawing::charts::Chart>(&serde_json::to_vec(model).unwrap()).unwrap(),
        bincode::deserialize::<umya_spreadsheet::drawing::charts::Chart>(&bincode::serialize(model).unwrap()).unwrap(),
    ] {
        let du = model.plot_area().value_axis()[0].display_units().unwrap();
        assert!(du.has_conflict());
        assert_eq!(du.extra_units().len(), 1);
        let mut restored = load(&source);
        chart_mut(&mut restored).chart_space_mut().set_chart(model);
        assert_export_rejected(&restored);
    }
}

#[test]
fn foreign_namespace_lookalike_is_not_treated_as_chart_unit() {
    let source = fixture_with_unit("<z:builtInUnit xmlns:z=\"urn:not-chart\" val=\"millions\"/>");
    let book = load(&source);
    let model = chart(&book).chart_space().chart();
    assert!(model.plot_area().value_axis()[0].display_units().unwrap().has_conflict());
    assert_export_rejected(&book);
}

#[test]
fn scalar_bodies_validate_references_cdata_and_nested_content() {
    for (body, valid) in [("", true), ("&#32;", true), ("<![CDATA[ \t]]>", true),
        ("&amp;", false), ("&#65;", false), ("<![CDATA[A]]>", false), ("<c:unknown/>", false)] {
        for tag in ["builtInUnit", "custUnit"] {
            let value = if tag == "custUnit" { "1.00E+3" } else { "millions" };
            let node = format!("<c:{tag} val=\"{value}\">{body}</c:{tag}>");
            let input = fixture_with_unit(&node); let book = load(&input);
            let model = chart(&book).chart_space().chart();
            assert_eq!(!model.phase8_conflicts().is_empty(), !valid, "{node}");
            for restored in [
                serde_json::from_slice::<umya_spreadsheet::drawing::charts::Chart>(&serde_json::to_vec(model).unwrap()).unwrap(),
                bincode::deserialize::<umya_spreadsheet::drawing::charts::Chart>(&bincode::serialize(model).unwrap()).unwrap(),
            ] {
                let mut out = load(&input); chart_mut(&mut out).chart_space_mut().set_chart(restored);
                if valid { let _ = load(&save(&out)); } else { assert_export_rejected(&out); }
            }
        }
        let input = fixture("millions");
        let xml = chart_xml(&input).replace("<c:idx val=\"7\"/>", &format!("<c:idx val=\"7\">{body}</c:idx>"));
        let book = load(&with_chart_xml(&input, &xml));
        assert_eq!(!chart(&book).chart_space().chart().phase8_conflicts().is_empty(), !valid, "idx body={body}");
        if valid { let _ = load(&save(&book)); } else { assert_export_rejected(&book); }
    }
    for node in [
        "<c:idx val=\"7\"></c:idx>",
        "<q:idx xmlns:q=\"http://schemas.openxmlformats.org/drawingml/2006/chart\" val=\"7\"></q:idx>",
        "<idx xmlns=\"http://schemas.openxmlformats.org/drawingml/2006/chart\" val=\"7\"/>",
    ] {
        let input = fixture("millions");
        let xml = chart_xml(&input).replace("<c:idx val=\"7\"/>", node);
        let book = load(&with_chart_xml(&input, &xml));
        assert!(chart(&book).chart_space().chart().pivot_formats().unwrap()[0].index_is_present(), "{node}");
        let _ = load(&save(&book));
    }
    let input = fixture("millions");
    let broken = chart_xml(&input).replace("<c:idx val=\"7\"/>", "<c:idx val=\"7\"><c:nested>");
    let broken = with_chart_xml(&input, &broken);
    assert!(std::panic::catch_unwind(|| load(&broken)).is_err(), "truncated scalar must not be accepted");
}

#[test]
fn expanded_and_empty_pivot_container_keep_presence_through_codecs() {
    let source = fixture("millions"); let xml = chart_xml(&source);
    let start = xml.find("<c:pivotFmts>").unwrap();
    let end = xml.find("</c:pivotFmts>").unwrap() + "</c:pivotFmts>".len();
    for (case, replacement, present) in [("absent", "", false), ("empty", "<c:pivotFmts/>", true), ("expanded", "<c:pivotFmts></c:pivotFmts>", true)] {
        let modified = format!("{}{}{}", &xml[..start], replacement, &xml[end..]);
        let input = with_chart_xml(&source, &modified); let book = load(&input);
        let model = chart(&book).chart_space().chart();
        assert_eq!(model.pivot_formats().is_some(), present);
        let original = save(&book); record_output(&original, &format!("pivot-{case}"), "original");
        assert_eq!(chart(&load(&original)).chart_space().chart().pivot_formats().is_some(), present);
        for (route, restored) in [
            ("json", serde_json::from_slice::<umya_spreadsheet::drawing::charts::Chart>(&serde_json::to_vec(model).unwrap()).unwrap()),
            ("bincode", bincode::deserialize::<umya_spreadsheet::drawing::charts::Chart>(&bincode::serialize(model).unwrap()).unwrap()),
        ] {
            let mut out = load(&input); chart_mut(&mut out).chart_space_mut().set_chart(restored);
            let bytes = save(&out); let reread = load(&bytes);
            record_output(&bytes, &format!("pivot-{case}"), route);
            assert_eq!(chart(&reread).chart_space().chart().pivot_formats().is_some(), present);
            assert_eq!(chart_xml(&bytes).contains("<c:pivotFmts>"), present);
        }
    }
}

#[test]
fn duplicate_label_text_stays_typed_and_adjustable_but_export_refuses() {
    let source = fixture("millions"); let xml = chart_xml(&source);
    let second = "<c:spPr/><c:tx><c:strRef><c:f>Sheet1!$A$2</c:f></c:strRef></c:tx>";
    let modified = xml.replacen("</c:dispUnitsLbl>", &format!("{second}</c:dispUnitsLbl>"), 1);
    let input = with_chart_xml(&source, &modified); let book = load(&input);
    let model = chart(&book).chart_space().chart();
    assert!(!model.phase8_conflicts().is_empty());
    for mut restored in [
        serde_json::from_slice::<umya_spreadsheet::drawing::charts::Chart>(&serde_json::to_vec(model).unwrap()).unwrap(),
        bincode::deserialize::<umya_spreadsheet::drawing::charts::Chart>(&bincode::serialize(model).unwrap()).unwrap(),
    ] {
        let label = restored.plot_area().value_axis()[0].display_units().unwrap().label().unwrap();
        assert!(!label.is_valid());
        assert_eq!(label.extra_children().len(), 1);
        assert!(matches!(label.extra_children()[0].child(), umya_spreadsheet::drawing::charts::DisplayLabelChild::Text(_)));
        let mut seen = Vec::new();
        for f in restored.formula_mut() { seen.push(f.address_str().to_owned()); }
        assert!(seen.iter().any(|v| v == "Sheet1!$A$1"));
        assert!(seen.iter().any(|v| v == "Sheet1!$A$2"));
        let mut out = load(&input); chart_mut(&mut out).chart_space_mut().set_chart(restored); assert_export_rejected(&out);
    }
}

#[test]
fn contradictory_stored_branches_cannot_be_hidden_by_clearing_parser_flags() {
    let input = fixture("millions"); let book = load(&input);
    let model = chart(&book).chart_space().chart();
    let mut json = serde_json::to_value(model).unwrap();
    let units = &mut json["plot_area"]["value_axis"][0]["parts"]["display_units"];
    let extra_label = units["label"].clone();
    let mut scalar = umya_spreadsheet::drawing::charts::XmlDouble::default();
    scalar.set_attribute(Some("2.00".into()));
    units["conflicts"] = serde_json::json!([]);
    units["extra_units"] = serde_json::json!([umya_spreadsheet::drawing::charts::DisplayUnit::Custom(scalar)]);
    units["extra_labels"] = serde_json::json!([extra_label]);
    units["label"]["valid"] = false.into();
    let pivot = &mut json["parts"]["pivot_formats"][0];
    pivot["invalid_index"] = false.into(); pivot["index_present"] = true.into();
    pivot["duplicate_indices"] = serde_json::json!(["8"]);
    pivot["extra_shapes"] = serde_json::json!([umya_spreadsheet::drawing::charts::ShapeProperties::default()]);
    let restored: umya_spreadsheet::drawing::charts::Chart = serde_json::from_value(json).unwrap();
    assert_eq!(restored.phase8_conflicts().len(), 2);
    assert!(!restored.pivot_formats().unwrap()[0].index_is_present());
    for model in [restored.clone(), bincode::deserialize(&bincode::serialize(&restored).unwrap()).unwrap()] {
        assert_eq!(model.phase8_conflicts().len(), 2);
        let units = model.plot_area().value_axis()[0].display_units().unwrap();
        assert_eq!(units.extra_units().len(), 1); assert_eq!(units.extra_labels().len(), 1);
        let mut out = load(&input); chart_mut(&mut out).chart_space_mut().set_chart(model);
        assert_export_rejected(&out);
    }
}

fn pivot_source(chart: &Chart) -> bool {
    chart.chart_space().chart().pivot_formats().unwrap()[0].label().unwrap().text().unwrap()
        .string_reference().unwrap().string_cache().source().is_some()
}

fn pivot_field_references(chart: &Chart) -> Vec<&umya_spreadsheet::drawing::charts::StringReference> {
    let label = chart.chart_space().chart().pivot_formats().unwrap().iter().find_map(|pivot| pivot.label()).unwrap();
    label_field_references(label)
}
fn label_field_references(label: &umya_spreadsheet::drawing::charts::DataLabel) -> Vec<&umya_spreadsheet::drawing::charts::StringReference> {
    use umya_spreadsheet::drawing::charts::{ExtensionNode, ExtensionPayloadValue};
    let ExtensionNode::Known(payload) = &label.extensions().unwrap().entries[0].nodes[0] else { panic!("typed extension") };
    let ExtensionPayloadValue::FieldTable(table) = &payload.value else { panic!("typed field table") };
    table.entries().iter().map(|entry| entry.reference()).collect()
}

#[test]
fn pivot_label_field_table_caches_follow_ordinary_capture_strip_and_adoption() {
    let base = fixture("millions");
    let xml = chart_xml(&base);
    for case in ["unique", "duplicate-guid", "missing-guid", "missing-label-index", "duplicate-label"] {
        let entry = |guid: Option<&str>, cell: &str, value: &str| {
            let guid = guid.map_or_else(String::new, |g| format!("<c15:txfldGUID>{g}</c15:txfldGUID>"));
            format!("<c15:dlblFTEntry>{guid}<c15:f>Missing!${cell}$1</c15:f><c15:dlblFieldTableCache><c:ptCount val=\"1\"/><c:pt idx=\"0\"><c:v>{value}</c:v></c:pt></c15:dlblFieldTableCache></c15:dlblFTEntry>")
        };
        let mut entries = entry(if case == "missing-guid" { None } else { Some("alpha") }, "C", "kept-field");
        if case == "duplicate-guid" { entries.push_str(&entry(Some("alpha"), "D", "duplicate-field")); }
        let ext = format!("<c:extLst><c:ext uri=\"{{CE6537A1-D6FC-4f65-9D91-7224C49458BB}}\"><c15:dlblFieldTable xmlns:c15=\"http://schemas.microsoft.com/office/drawing/2012/chart\">{entries}</c15:dlblFieldTable></c:ext></c:extLst>");
        assert_eq!(xml.matches("</c:dLbl>").count(), 1);
        let mut modified = xml.replacen("</c:dLbl>", &format!("{ext}</c:dLbl>"), 1);
        if case == "missing-label-index" { modified = modified.replacen("<c:dLbl><c:idx val=\"7\"/>", "<c:dLbl>", 1); }
        if case == "duplicate-label" {
            let start = modified.find("<c:dLbl>").unwrap();
            let end = modified.find("</c:dLbl>").unwrap() + "</c:dLbl>".len();
            let extra = modified[start..end].replace("Missing!$C$1", "Missing!$E$1").replace("kept-field", "extra-field");
            modified.insert_str(end, &extra);
        }
        let input = with_chart_xml(&base, &modified);
        for eager in [true, false] {
            let mut book = umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(&input), eager).unwrap();
            let original = book.sheet_mut(0).unwrap().chart_collection()[0].clone();
            let refs = pivot_field_references(&original);
            assert_eq!(refs.len(), if case == "duplicate-guid" { 2 } else { 1 });
            assert!(refs.iter().all(|r| r.string_cache().source().is_some()));
            for reference in &refs {
                let source_formula = reference.string_cache().source().unwrap().formula();
                assert!(!source_formula.contains('\0'));
                assert_eq!(source_formula, reference.formula().address_str());
            }
            let extras = &original.chart_space().chart().pivot_formats().unwrap()[0].extra_labels();
            if case == "duplicate-label" { assert!(label_field_references(&extras[0])[0].string_cache().source().is_some()); }
            assert_eq!(book.chart_source_cache_report().kept_caches, refs.len() + usize::from(case == "duplicate-label"));
            let json: Chart = serde_json::from_slice(&serde_json::to_vec(&original).unwrap()).unwrap();
            let binary: Chart = bincode::deserialize(&bincode::serialize(&original).unwrap()).unwrap();
            for server in [original, json, binary] {
                let mut client = server.clone(); client.strip_source_caches();
                assert!(pivot_field_references(&client).iter().all(|r| r.string_cache().source().is_none()));
                client.adopt_source_caches(&server);
                assert!(pivot_field_references(&client).iter().all(|r| r.string_cache().source().is_some() == (case == "unique")), "{case}");
                if case == "duplicate-label" {
                    let extras = client.chart_space().chart().pivot_formats().unwrap()[0].extra_labels();
                    assert!(label_field_references(&extras[0])[0].string_cache().source().is_none());
                }
                if case == "unique" {
                    use umya_spreadsheet::drawing::charts::{ExtensionNode, ExtensionPayloadValue};
                    for (source_duplicate, incoming_duplicate) in [(true, false), (false, true), (true, true)] {
                        let mut source = server.clone();
                        let mut incoming = server.clone();
                        for (target, duplicate, swap) in [(&mut source, source_duplicate, false), (&mut incoming, incoming_duplicate, true)] {
                            if duplicate {
                                let owner = target.chart_space_mut().chart_mut();
                                let real = owner.pivot_formats().unwrap()[0].clone();
                                let mut empty = umya_spreadsheet::drawing::charts::PivotFormat::default();
                                let mut index = umya_spreadsheet::drawing::charts::Index::default(); index.set_val(real.index().val()); empty.set_index(index);
                                owner.set_pivot_formats(if swap { vec![empty, real] } else { vec![real, empty] });
                            }
                        }
                        incoming.strip_source_caches(); incoming.adopt_source_caches(&source);
                        assert!(pivot_field_references(&incoming)[0].string_cache().source().is_none(), "refless duplicate owner prevents nested field transfer");
                    }
                    let mut changed = server.clone();
                    let label = changed.chart_space_mut().chart_mut().pivot_formats_mut().unwrap()[0].label_mut().unwrap();
                    let ExtensionNode::Known(payload) = &mut label.extensions_mut().unwrap().entries[0].nodes[0] else { panic!("typed extension") };
                    let ExtensionPayloadValue::FieldTable(table) = &mut payload.value else { panic!("typed field table") };
                    table.entries_mut()[0].reference_mut().formula_mut().set_address_str("Missing!$C$2");
                    changed.strip_source_caches(); changed.adopt_source_caches(&server);
                    assert!(pivot_field_references(&changed)[0].string_cache().source().is_none());
                    *chart_mut(&mut book) = client;
                    let output = chart_xml(&save(&book));
                    assert!(output.contains("<c:v>kept-field</c:v>"));
                }
            }
        }
    }
}

#[test]
fn pivot_label_field_table_respects_global_point_and_text_caps() {
    use umya_spreadsheet::drawing::charts::{MAX_SOURCE_CACHE_POINTS, MAX_SOURCE_CACHE_TEXT_BYTES};
    let base = fixture("millions");
    let xml = chart_xml(&base);
    for text_cap in [false, true] {
        let values = if text_cap { vec!["x".repeat(MAX_SOURCE_CACHE_TEXT_BYTES)] } else { vec!["x".to_owned(); MAX_SOURCE_CACHE_POINTS] };
        let points: String = values.iter().enumerate().map(|(index,value)| format!("<c:pt idx=\"{index}\"><c:v>{value}</c:v></c:pt>")).collect();
        let field = format!("<c15:dlblFTEntry><c15:txfldGUID>alpha</c15:txfldGUID><c15:f>Missing!$C$1</c15:f><c15:dlblFieldTableCache><c:ptCount val=\"{}\"/>{points}</c15:dlblFieldTableCache></c15:dlblFTEntry>", values.len());
        let late = "<c15:dlblFTEntry><c15:txfldGUID>beta</c15:txfldGUID><c15:f>Missing!$D$1</c15:f><c15:dlblFieldTableCache><c:ptCount val=\"1\"/><c:pt idx=\"0\"><c:v>late</c:v></c:pt></c15:dlblFieldTableCache></c15:dlblFTEntry>";
        let ext = format!("<c:extLst><c:ext uri=\"{{CE6537A1-D6FC-4f65-9D91-7224C49458BB}}\"><c15:dlblFieldTable xmlns:c15=\"http://schemas.microsoft.com/office/drawing/2012/chart\">{field}{late}</c15:dlblFieldTable></c:ext></c:extLst>");
        let input = with_chart_xml(&base, &xml.replacen("</c:dLbl>", &format!("{ext}</c:dLbl>"), 1));
        for eager in [true, false] {
            let mut book = umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(&input), eager).unwrap();
            let refs = pivot_field_references(&book.sheet_mut(0).unwrap().chart_collection()[0]);
            assert_eq!(refs.len(), 2);
            assert!(refs[0].string_cache().source().is_some());
            assert!(refs[1].string_cache().source().is_none());
            let report = book.chart_source_cache_report();
            assert_eq!(report.kept_caches, 1);
            assert_eq!(report.over_cap_caches, 1);
            assert_eq!(report.kept_points, if text_cap { 1 } else { MAX_SOURCE_CACHE_POINTS });
            assert_eq!(report.kept_text_bytes, if text_cap { MAX_SOURCE_CACHE_TEXT_BYTES } else { MAX_SOURCE_CACHE_POINTS });
        }
    }
}

fn display_units_source(chart: &Chart) -> bool {
    chart.chart_space().chart().plot_area().value_axis()[0].display_units().unwrap()
        .label().unwrap().text().unwrap().string_reference().unwrap().string_cache().source().is_some()
}

#[test]
fn phase8_cache_roles_follow_existing_title_under_point_and_text_caps() {
    use umya_spreadsheet::drawing::charts::{MAX_SOURCE_CACHE_POINTS, MAX_SOURCE_CACHE_TEXT_BYTES};
    fn reference(formula: &str, values: &[String]) -> String {
        let points: String = values.iter().enumerate().map(|(i, value)| format!("<c:pt idx=\"{i}\"><c:v>{value}</c:v></c:pt>")).collect();
        format!("<c:strRef><c:f>{formula}</c:f><c:strCache><c:ptCount val=\"{}\"/>{points}</c:strCache></c:strRef>", values.len())
    }
    for text_cap in [false, true] {
        let base = fixture("millions");
        let mut xml = chart_xml(&base);
        let title_values = vec![if text_cap { "x".repeat(MAX_SOURCE_CACHE_TEXT_BYTES - 1) } else { "t".into() }];
        let pivot_values = vec!["p".to_owned(); if text_cap { 1 } else { MAX_SOURCE_CACHE_POINTS - 1 }];
        let title = reference("Missing!$C$1", &title_values);
        let pivot = reference("Missing!$B$1", &pivot_values);
        let display = reference("Missing!$A$1", &["d".into()]);
        assert!(!xml.contains("<c:title>"));
        xml = xml.replacen("<c:chart>", &format!("<c:chart><c:title><c:tx>{title}</c:tx></c:title>"), 1);
        xml = xml.replacen("<c:strRef><c:f>Sheet1!$B$1</c:f></c:strRef>", &pivot, 1);
        xml = xml.replacen("<c:strRef><c:f>Sheet1!$A$1</c:f></c:strRef>", &display, 1);
        let input = with_chart_xml(&base, &xml);
        for eager in [true, false] {
            let mut book = umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(&input), eager).unwrap();
            let charts = book.sheet_mut(0).unwrap().chart_collection();
            assert_eq!(charts.len(), 1);
            let chart = &charts[0];
            assert!(chart.chart_space().chart().title().unwrap().chart_text().unwrap().string_reference().unwrap().string_cache().source().is_some());
            assert!(pivot_source(chart));
            assert!(!display_units_source(chart));
            let report = book.chart_source_cache_report();
            assert_eq!(report.kept_caches, 2);
            assert_eq!(report.over_cap_caches, 1);
            assert_eq!(report.reduced_caches, 0);
            assert_eq!(report.kept_points, if text_cap { 2 } else { MAX_SOURCE_CACHE_POINTS });
            assert_eq!(report.kept_text_bytes, if text_cap { MAX_SOURCE_CACHE_TEXT_BYTES } else { MAX_SOURCE_CACHE_POINTS });
        }
    }
}

#[test]
fn old_series_axis_trendline_and_extension_caches_precede_phase8_under_both_caps() {
    use umya_spreadsheet::drawing::charts::{MAX_SOURCE_CACHE_POINTS, MAX_SOURCE_CACHE_TEXT_BYTES};
    fn cached(formula: &str, values: &[String], wrapper: &str) -> String {
        let points: String = values.iter().enumerate().map(|(i, value)| format!("<c:pt idx=\"{i}\"><c:v>{value}</c:v></c:pt>")).collect();
        format!("<c:f>{formula}</c:f><{wrapper}><c:ptCount val=\"{}\"/>{points}</{wrapper}>", values.len())
    }
    for text_cap in [false, true] {
        let base = fixture("millions");
        let old = |cell: &str, value: &str| cached(&format!("Missing!${cell}$1"), &[value.to_owned()], "c:strCache");
        let title = old("A", "T");
        let series = old("B", "S");
        let category = old("C", "C");
        let category_axis = old("D", "A");
        let value_axis = old("E", "V");
        let trend = old("F", "R");
        let extension = cached("Missing!$G$1", &["E".into()], "c15:dlblRangeCache").replace("<c:f>", "<c15:f>").replace("</c:f>", "</c15:f>");
        let date_axis = old("J", "Q");
        let series_axis = old("K", "Z");
        let pivot_values = if text_cap { vec!["p".repeat(MAX_SOURCE_CACHE_TEXT_BYTES - 9)] } else { vec!["p".into(); MAX_SOURCE_CACHE_POINTS - 9] };
        let pivot = cached("Missing!$H$1", &pivot_values, "c:strCache");
        let display = old("I", "D");
        let xml = format!(concat!(
            "<c:chartSpace xmlns:c=\"http://schemas.openxmlformats.org/drawingml/2006/chart\" xmlns:c15=\"http://schemas.microsoft.com/office/drawing/2012/chart\">",
            "<c:chart><c:title><c:tx><c:strRef>{title}</c:strRef></c:tx></c:title>",
            "<c:pivotFmts><c:pivotFmt><c:idx val=\"7\"/><c:dLbl><c:idx val=\"7\"/><c:tx><c:strRef>{pivot}</c:strRef></c:tx></c:dLbl></c:pivotFmt></c:pivotFmts>",
            "<c:plotArea><c:lineChart><c:grouping val=\"standard\"/><c:ser><c:idx val=\"0\"/><c:order val=\"0\"/>",
            "<c:tx><c:strRef>{series}</c:strRef></c:tx>",
            "<c:trendline><c:trendlineType val=\"linear\"/><c:trendlineLbl><c:tx><c:strRef>{trend}</c:strRef></c:tx></c:trendlineLbl></c:trendline>",
            "<c:cat><c:strRef>{category}</c:strRef></c:cat>",
            "<c:extLst><c:ext uri=\"{{02D57815-91ED-43cb-92C2-25804820EDAC}}\"><c15:datalabelsRange>{extension}</c15:datalabelsRange></c:ext></c:extLst></c:ser>",
            "<c:axId val=\"1\"/><c:axId val=\"2\"/></c:lineChart>",
            "<c:catAx><c:axId val=\"1\"/><c:scaling/><c:axPos val=\"b\"/><c:title><c:tx><c:strRef>{category_axis}</c:strRef></c:tx></c:title><c:crossAx val=\"2\"/></c:catAx>",
            "<c:valAx><c:axId val=\"2\"/><c:scaling/><c:axPos val=\"l\"/><c:title><c:tx><c:strRef>{value_axis}</c:strRef></c:tx></c:title><c:crossAx val=\"1\"/>",
            "<c:dispUnits><c:builtInUnit val=\"millions\"/><c:dispUnitsLbl><c:tx><c:strRef>{display}</c:strRef></c:tx></c:dispUnitsLbl></c:dispUnits></c:valAx>",
            "<c:dateAx><c:axId val=\"3\"/><c:scaling/><c:axPos val=\"b\"/><c:title><c:tx><c:strRef>{date_axis}</c:strRef></c:tx></c:title><c:crossAx val=\"2\"/></c:dateAx>",
            "<c:serAx><c:axId val=\"4\"/><c:scaling/><c:axPos val=\"b\"/><c:title><c:tx><c:strRef>{series_axis}</c:strRef></c:tx></c:title><c:crossAx val=\"2\"/></c:serAx>",
            "</c:plotArea></c:chart></c:chartSpace>"), title=title, series=series, category=category, category_axis=category_axis, value_axis=value_axis, date_axis=date_axis, series_axis=series_axis, trend=trend, extension=extension, pivot=pivot, display=display);
        let input = with_chart_xml(&base, &xml);
        for eager in [true, false] {
            let mut book = umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(&input), eager).unwrap();
            let chart = &book.sheet_mut(0).unwrap().chart_collection()[0];
            assert!(pivot_source(chart));
            assert!(!display_units_source(chart));
            let report = *book.chart_source_cache_report();
            assert_eq!(report.kept_caches, 10);
            assert_eq!(report.over_cap_caches, 1);
            assert_eq!(report.kept_points, if text_cap { 10 } else { MAX_SOURCE_CACHE_POINTS });
            assert_eq!(report.kept_text_bytes, if text_cap { MAX_SOURCE_CACHE_TEXT_BYTES } else { MAX_SOURCE_CACHE_POINTS });
            let output = chart_xml(&save(&book));
            for value in ["T", "S", "C", "A", "V", "Q", "Z", "R", "E"] {
                assert!(output.contains(&format!("<c:v>{value}</c:v>")), "old cache {value} must survive");
            }
            assert!(!output.contains("<c:v>D</c:v>"), "late display cache must not consume priority");
        }
    }
}

#[test]
fn phase8_cache_adoption_guards_both_roles_against_stale_and_ambiguous_owners() {
    let base = fixture("millions");
    let mut xml = chart_xml(&base);
    for (cell, value) in [("$B$1", "pivot"), ("$A$1", "axis")] {
        let old = format!("<c:f>Sheet1!{cell}</c:f>");
        let new = format!("<c:f>Missing!{cell}</c:f><c:strCache><c:ptCount val=\"1\"/><c:pt idx=\"0\"><c:v>{value}</c:v></c:pt></c:strCache>");
        assert_eq!(xml.matches(&old).count(), 1);
        xml = xml.replacen(&old, &new, 1);
    }
    let input = with_chart_xml(&base, &xml);
    for eager in [true, false] {
        let mut book = umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(&input), eager).unwrap();
        let previous = book.sheet_mut(0).unwrap().chart_collection()[0].clone();
        assert!(pivot_source(&previous) && display_units_source(&previous));
        let report = *book.chart_source_cache_report();
        let _ = book.sheet_mut(0).unwrap();
        assert_eq!(*book.chart_source_cache_report(), report, "repeat access must not charge twice");
        let mut clean = previous.clone();
        clean.strip_source_caches(); clean.adopt_source_caches(&previous);
        assert!(pivot_source(&clean) && display_units_source(&clean));

        let mut stale = previous.clone();
        let owner = stale.chart_space_mut().chart_mut();
        owner.pivot_formats_mut().unwrap()[0].label_mut().unwrap().text_mut().unwrap().string_reference_mut().unwrap().formula_mut().set_address_str("Missing!$B$2");
        owner.plot_area_mut().value_axis_mut()[0].display_units_mut().unwrap().label_mut().unwrap().text_mut().unwrap().string_reference_mut().unwrap().formula_mut().set_address_str("Missing!$A$2");
        stale.strip_source_caches(); stale.adopt_source_caches(&previous);
        assert!(!pivot_source(&stale) && !display_units_source(&stale));

        let mut stale_source = previous.clone();
        let owner = stale_source.chart_space_mut().chart_mut();
        owner.pivot_formats_mut().unwrap()[0].label_mut().unwrap().text_mut().unwrap().string_reference_mut().unwrap().formula_mut().set_address_str("Missing!$B$2");
        owner.plot_area_mut().value_axis_mut()[0].display_units_mut().unwrap().label_mut().unwrap().text_mut().unwrap().string_reference_mut().unwrap().formula_mut().set_address_str("Missing!$A$2");
        let mut matched = stale_source.clone();
        matched.strip_source_caches(); matched.adopt_source_caches(&stale_source);
        assert!(!pivot_source(&matched) && !display_units_source(&matched), "matching edited formulas cannot adopt stale source-formula guards");

        let mut missing_axis = previous.clone();
        *missing_axis.chart_space_mut().chart_mut().plot_area_mut().value_axis_mut()[0].axis_id_mut() = Default::default();
        let mut missing_client = missing_axis.clone();
        missing_client.strip_source_caches(); missing_client.adopt_source_caches(&missing_axis);
        assert!(!display_units_source(&missing_client), "missing axis ID is not default zero");
        let mut zero_axis = previous.clone();
        zero_axis.chart_space_mut().chart_mut().plot_area_mut().value_axis_mut()[0].axis_id_mut().set_val(0);
        let mut zero_client = zero_axis.clone();
        zero_client.strip_source_caches(); zero_client.adopt_source_caches(&zero_axis);
        assert!(display_units_source(&zero_client), "explicit zero axis ID is a stable identity");

        for duplicate_on_source in [false, true] {
            let mut old = previous.clone();
            let mut incoming = previous.clone();
            let target = if duplicate_on_source { &mut old } else { &mut incoming };
            let owner = target.chart_space_mut().chart_mut();
            let pivot = owner.pivot_formats().unwrap()[0].clone();
            owner.set_pivot_formats(vec![pivot.clone(), pivot]);
            let axis = owner.plot_area().value_axis()[0].clone();
            owner.plot_area_mut().value_axis_mut().push(axis);
            incoming.strip_source_caches(); incoming.adopt_source_caches(&old);
            assert!(!pivot_source(&incoming) && !display_units_source(&incoming), "duplicates on either side disqualify both role identities");
        }
    }
}

#[test]
fn duplicate_refless_pivot_and_axis_owners_cannot_transfer_caches_by_swap() {
    use umya_spreadsheet::drawing::charts::{Index, PivotFormat, ValueAxis};
    fn arrange(chart: &mut Chart, pivot_role: bool, duplicate: bool, swap: bool) {
        let owner = chart.chart_space_mut().chart_mut();
        if pivot_role {
            let real = owner.pivot_formats().unwrap()[0].clone();
            let mut empty = PivotFormat::default();
            let mut index = Index::default(); index.set_val(real.index().val()); empty.set_index(index);
            owner.set_pivot_formats(if !duplicate { vec![real] } else if swap { vec![empty, real] } else { vec![real, empty] });
        } else {
            let real = owner.plot_area().value_axis()[0].clone();
            let mut empty = ValueAxis::default(); empty.axis_id_mut().set_val(real.axis_id().val());
            *owner.plot_area_mut().value_axis_mut() = if !duplicate { vec![real] } else if swap { vec![empty, real] } else { vec![real, empty] };
        }
    }
    fn has_cache(chart: &Chart, pivot_role: bool) -> bool {
        let owner = chart.chart_space().chart();
        if pivot_role {
            owner.pivot_formats().unwrap().iter().filter_map(|p| p.label()).filter_map(|l| l.text()).filter_map(|t| t.string_reference()).any(|r| r.string_cache().source().is_some())
        } else {
            owner.plot_area().value_axis().iter().filter_map(|a| a.display_units()).filter_map(|u| u.label()).filter_map(|l| l.text()).filter_map(|t| t.string_reference()).any(|r| r.string_cache().source().is_some())
        }
    }
    let base = fixture("millions");
    let mut xml = chart_xml(&base);
    for (cell,value) in [("$B$1", "pivot"), ("$A$1", "axis")] {
        let old = format!("<c:f>Sheet1!{cell}</c:f>");
        let new = format!("<c:f>Missing!{cell}</c:f><c:strCache><c:ptCount val=\"1\"/><c:pt idx=\"0\"><c:v>{value}</c:v></c:pt></c:strCache>");
        assert_eq!(xml.matches(&old).count(), 1);
        xml = xml.replacen(&old, &new, 1);
    }
    let input = with_chart_xml(&base, &xml);
    for eager in [true, false] {
        let mut book = umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(&input), eager).unwrap();
        let original = book.sheet_mut(0).unwrap().chart_collection()[0].clone();
        let json: Chart = serde_json::from_slice(&serde_json::to_vec(&original).unwrap()).unwrap();
        let binary: Chart = bincode::deserialize(&bincode::serialize(&original).unwrap()).unwrap();
        for server in [original, json, binary] {
            for pivot_role in [true, false] {
                assert!(has_cache(&server, pivot_role));
                for (source_duplicate, incoming_duplicate, swap) in [(false,false,false), (true,false,false), (false,true,false), (true,true,true)] {
                    let mut source = server.clone(); arrange(&mut source, pivot_role, source_duplicate, false);
                    let mut incoming = server.clone(); arrange(&mut incoming, pivot_role, incoming_duplicate, swap);
                    incoming.strip_source_caches(); incoming.adopt_source_caches(&source);
                    assert_eq!(has_cache(&incoming, pivot_role), !source_duplicate && !incoming_duplicate,
                        "pivot={pivot_role} source_duplicate={source_duplicate} incoming_duplicate={incoming_duplicate} swap={swap}");
                }
            }
        }
    }
}

#[test]
fn duplicate_display_units_containers_preserve_both_caches_but_never_adopt() {
    let base = fixture("millions");
    let xml = chart_xml(&base);
    let original = "<c:f>Sheet1!$A$1</c:f>";
    assert_eq!(xml.matches(original).count(), 1);
    let xml = xml.replacen(original, "<c:f>Missing!$A$1</c:f><c:strCache><c:ptCount val=\"1\"/><c:pt idx=\"0\"><c:v>primary</c:v></c:pt></c:strCache>", 1);
    let start = xml.find("<c:dispUnits>").unwrap();
    let end = xml.find("</c:dispUnits>").unwrap() + "</c:dispUnits>".len();
    let extra = xml[start..end].replace("Missing!$A$1", "Missing!$D$1").replace("<c:v>primary</c:v>", "<c:v>extra</c:v>");
    let unique_input = with_chart_xml(&base, &xml);
    let duplicate_input = with_chart_xml(&base, &format!("{}{}{}", &xml[..end], extra, &xml[end..]));
    fn extra_source(chart: &Chart) -> bool {
        let units = &chart.chart_space().chart().plot_area().value_axis()[0].extra_display_units()[0];
        units.label().unwrap().text().unwrap().string_reference().unwrap().string_cache().source().is_some()
    }
    for eager in [true, false] {
        let mut unique = umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(&unique_input), eager).unwrap();
        let unique_chart = unique.sheet_mut(0).unwrap().chart_collection()[0].clone();
        assert!(display_units_source(&unique_chart));
        let mut control = unique_chart.clone();
        control.strip_source_caches(); control.adopt_source_caches(&unique_chart);
        assert!(display_units_source(&control), "a unique axis owner adopts normally");

        let mut duplicate = umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(&duplicate_input), eager).unwrap();
        let original = duplicate.sheet_mut(0).unwrap().chart_collection()[0].clone();
        assert!(display_units_source(&original) && extra_source(&original));
        assert_eq!(original.chart_space().chart().plot_area().value_axis()[0].extra_display_units().len(), 1);
        let json: Chart = serde_json::from_slice(&serde_json::to_vec(&original).unwrap()).unwrap();
        let binary: Chart = bincode::deserialize(&bincode::serialize(&original).unwrap()).unwrap();
        for model in [original, json, binary] {
            assert!(display_units_source(&model) && extra_source(&model));
            let mut out = load(&duplicate_input);
            *chart_mut(&mut out) = model.clone();
            assert_export_rejected(&out);
            assert_file_rejected(&out);

            let mut duplicate_client = model.clone();
            duplicate_client.strip_source_caches(); duplicate_client.adopt_source_caches(&unique_chart);
            assert!(!display_units_source(&duplicate_client) && !extra_source(&duplicate_client), "duplicate incoming container must not adopt");
            let mut unique_client = unique_chart.clone();
            unique_client.strip_source_caches(); unique_client.adopt_source_caches(&model);
            assert!(!display_units_source(&unique_client), "duplicate source container must not supply a unique client");
        }
    }
}

#[test]
fn duplicate_pivot_containers_never_adopt_even_primary_source_cache() {
    let base = fixture("millions");
    let xml = chart_xml(&base).replace("<c:f>Sheet1!$B$1</c:f>",
        "<c:f>Missing!$B$1</c:f><c:strCache><c:ptCount val=\"1\"/><c:pt idx=\"0\"><c:v>cached</c:v></c:pt></c:strCache>");
    let single = load(&with_chart_xml(&base, &xml));
    let previous = chart(&single).clone();
    assert!(pivot_source(&previous), "fixture must import a real source cache");
    let mut control = previous.clone(); control.strip_source_caches();
    assert!(!pivot_source(&control)); control.adopt_source_caches(&previous);
    assert!(pivot_source(&control), "unique explicit identity must adopt");
    let mut no_index = previous.clone();
    no_index.chart_space_mut().chart_mut().pivot_formats_mut().unwrap()[0].set_index(Default::default());
    no_index.strip_source_caches(); no_index.adopt_source_caches(&previous);
    assert!(!pivot_source(&no_index), "missing identity must not adopt by order or default zero");
    let mut zero_source = previous.clone();
    let mut zero = umya_spreadsheet::drawing::charts::Index::default(); zero.set_val(0);
    zero_source.chart_space_mut().chart_mut().pivot_formats_mut().unwrap()[0].set_index(zero);
    let mut zero_client = zero_source.clone(); zero_client.strip_source_caches();
    zero_client.adopt_source_caches(&zero_source);
    assert!(pivot_source(&zero_client), "explicit zero is a real stable identity");
    let begin = xml.find("<c:pivotFmts>").unwrap();
    let end = xml.find("</c:pivotFmts>").unwrap() + "</c:pivotFmts>".len();
    let original_container = &xml[begin..end];
    for index in ["7", "8"] {
        let duplicate = original_container.replace("val=\"7\"", &format!("val=\"{index}\""));
        let input = with_chart_xml(&base, &format!("{}{}{}", &xml[..end], duplicate, &xml[end..]));
        let book = load(&input); let previous = chart(&book).clone();
        assert!(pivot_source(&previous));
        assert_eq!(previous.chart_space().chart().duplicate_pivot_containers().len(), 1);
        let mut client = previous.clone(); client.strip_source_caches();
        assert!(!pivot_source(&client));
        client.adopt_source_caches(&previous);
        assert!(!pivot_source(&client), "ambiguous pivot container must not adopt primary identity");
        let duplicate = &client.chart_space().chart().duplicate_pivot_containers()[0].pivot_formats().unwrap()[0];
        assert!(duplicate.label().unwrap().text().unwrap().string_reference().unwrap().string_cache().source().is_none());
    }
}

#[test]
fn absent_typed_index_setter_and_json_are_rejected_but_explicit_zero_is_valid() {
    let input = fixture("millions"); let mut book = load(&input);
    let owner = chart_mut(&mut book).chart_space_mut().chart_mut();
    owner.pivot_formats_mut().unwrap()[0].set_index(Default::default());
    assert!(!owner.pivot_formats().unwrap()[0].index_is_present());
    assert!(!owner.phase8_conflicts().is_empty());
    let mut json = serde_json::to_value(&*owner).unwrap();
    json["parts"]["pivot_formats"][0]["index_present"] = true.into();
    let from_json: umya_spreadsheet::drawing::charts::Chart = serde_json::from_value(json).unwrap();
    let from_bin: umya_spreadsheet::drawing::charts::Chart = bincode::deserialize(&bincode::serialize(&from_json).unwrap()).unwrap();
    for model in [from_json, from_bin] {
        assert!(!model.pivot_formats().unwrap()[0].index_is_present());
        let mut out = load(&input); chart_mut(&mut out).chart_space_mut().set_chart(model);
        assert_export_rejected(&out); assert_file_rejected(&out);
    }
    let mut zero = umya_spreadsheet::drawing::charts::Index::default(); zero.set_val(0);
    let owner = chart_mut(&mut book).chart_space_mut().chart_mut();
    owner.pivot_formats_mut().unwrap()[0].set_index(zero);
    assert!(owner.pivot_formats().unwrap()[0].index_is_present());
    assert!(owner.phase8_conflicts().is_empty());
    let output = save(&book); let reread = load(&output);
    let pivot = &chart(&reread).chart_space().chart().pivot_formats().unwrap()[0];
    assert!(pivot.index_is_present()); assert_eq!(pivot.index().val(), 0);
}

#[test]
fn pivot_label_missing_its_own_required_index_is_rejected() {
    let base = fixture("millions");
    let xml = chart_xml(&base);
    let valid = "<c:dLbl><c:idx val=\"7\"/>";
    assert_eq!(xml.matches(valid).count(), 1);
    let input = with_chart_xml(&base, &xml.replacen(valid, "<c:dLbl>", 1));
    let book = load(&input);
    assert!(!chart(&book).chart_space().chart().phase8_conflicts().is_empty());
    assert_export_rejected(&book);
    assert_file_rejected(&book);
}

#[test]
fn malformed_native_marker_persists_and_rejects_without_losing_destination() {
    let source = fixture("millions"); let xml = chart_xml(&source);
    for marker in ["<c:marker><c:symbol/></c:marker>", "<c:marker><c:size val=\"1\"/></c:marker>"] {
        let input = with_chart_xml(&source, &xml.replacen("<c:dLbl>", &format!("{marker}<c:dLbl>"), 1));
        let book = load(&input); let owner = chart(&book).chart_space().chart();
        assert!(!owner.phase8_conflicts().is_empty());
        assert!(owner.pivot_formats().unwrap()[0].marker().is_some());
        for model in [
            serde_json::from_slice::<umya_spreadsheet::drawing::charts::Chart>(&serde_json::to_vec(owner).unwrap()).unwrap(),
            bincode::deserialize::<umya_spreadsheet::drawing::charts::Chart>(&bincode::serialize(owner).unwrap()).unwrap(),
        ] {
            assert!(model.pivot_formats().unwrap()[0].marker().is_some());
            assert!(!model.phase8_conflicts().is_empty());
            let mut out = load(&input); chart_mut(&mut out).chart_space_mut().set_chart(model);
            assert_export_rejected(&out); assert_file_rejected(&out);
        }
    }
    for marker in ["<c:marker><c:size val=\"7\"/></c:marker>", "<c:marker><c:spPr/></c:marker>"] {
        let input = with_chart_xml(&source, &xml.replacen("<c:dLbl>", &format!("{marker}<c:dLbl>"), 1));
        let book = load(&input); assert!(chart(&book).chart_space().chart().phase8_conflicts().is_empty());
        let _ = load(&save(&book));
    }
}

#[test]
fn element_only_phase8_owners_reject_significant_text_across_codecs() {
    let base = fixture("millions");
    let xml = chart_xml(&base);
    for owner in ["dispUnits", "dispUnitsLbl", "pivotFmts", "pivotFmt"] {
        let tag = format!("<c:{owner}>");
        assert_eq!(xml.matches(&tag).count(), 1, "{owner}");
        for content in ["garbage", "<![CDATA[garbage]]>", "&#65;"] {
            let input = with_chart_xml(&base, &xml.replacen(&tag, &format!("{tag}{content}"), 1));
            for eager in [true, false] {
                let mut book = umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(&input), eager).unwrap();
                let imported = book.sheet_mut(0).unwrap().chart_collection()[0].chart_space().chart().clone();
                assert!(!imported.phase8_conflicts().is_empty(), "{owner} {content}");
                let json: umya_spreadsheet::drawing::charts::Chart = serde_json::from_slice(&serde_json::to_vec(&imported).unwrap()).unwrap();
                let binary: umya_spreadsheet::drawing::charts::Chart = bincode::deserialize(&bincode::serialize(&imported).unwrap()).unwrap();
                for model in [imported, json, binary] {
                    assert!(!model.phase8_conflicts().is_empty(), "{owner} {content}");
                    let mut output = load(&input);
                    chart_mut(&mut output).chart_space_mut().set_chart(model);
                    assert_export_rejected(&output);
                    assert_file_rejected(&output);
                }
            }
        }
        let whitespace = " \t\n<![CDATA[ \t]]>&#x20;&#9;";
        let input = with_chart_xml(&base, &xml.replacen(&tag, &format!("{tag}{whitespace}"), 1));
        for eager in [true, false] {
            let mut book = umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(&input), eager).unwrap();
            assert!(book.sheet_mut(0).unwrap().chart_collection()[0].chart_space().chart().phase8_conflicts().is_empty(), "valid whitespace in {owner}");
            let output = save(&book);
            assert!(chart(&load(&output)).chart_space().chart().phase8_conflicts().is_empty());
        }
    }
}
