#![cfg(feature = "serde")]

use std::{
    collections::HashMap,
    io::{
        Cursor,
        Read,
    },
    path::PathBuf,
};

use quick_xml::{
    Reader,
    events::Event,
};
use rstest::rstest;
use serde_json::{
    Map,
    Value,
    json,
};
use sha2::{
    Digest,
    Sha256,
};
use umya_spreadsheet::{
    CellFormulaValues,
    Workbook,
    Worksheet,
    drawing::spreadsheet::{
        DrawingAnchor,
        MarkerType,
    },
    helper::{
        coordinate::coordinate_from_index,
        range::get_coordinate_list,
    },
    reader,
    writer,
};

const BASE: &[u8] = include_bytes!("fixtures/range_operations/base.xlsx");
const ANSWERS: &str = include_str!("fixtures/range_operations/answers.json");

fn open(bytes: &[u8]) -> Workbook {
    reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap()
}

fn save(book: &Workbook, light: bool, label: &str) -> Vec<u8> {
    let mut output = Vec::new();
    if light {
        writer::xlsx::write_writer_light(book, &mut output).unwrap();
    } else {
        writer::xlsx::write_writer(book, &mut output).unwrap();
    }
    let directory =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/range-operations-validation");
    std::fs::create_dir_all(&directory).unwrap();
    let suffix = if light { "light" } else { "normal" };
    std::fs::write(directory.join(format!("{label}-{suffix}.xlsx")), &output).unwrap();
    output
}

fn sorted(mut values: Vec<Value>) -> Value {
    values.sort_by_cached_key(Value::to_string);
    Value::Array(values)
}

fn conditional_format(range: &str, formula: &str) -> Value {
    // COM separates areas with commas; OOXML sqref separates them with spaces.
    let mut areas: Vec<_> = range.split([' ', ',']).collect();
    areas.sort_unstable();
    json!({"range": areas, "formula": formula})
}

fn assert_sheet(sheet: &Worksheet, expected: &Value, label: &str) {
    // Excel's Formula/CurrentArray properties expose the master formula for
    // every array cell; OOXML stores that formula only on its top-left cell.
    let arrays: HashMap<_, _> = sheet
        .cells()
        .into_iter()
        .filter_map(|cell| cell.formula_obj())
        .filter(|formula| formula.formula_type() == &CellFormulaValues::Array)
        .flat_map(|formula| {
            get_coordinate_list(formula.reference())
                .into_iter()
                .map(move |coordinate| (coordinate, (formula.text(), formula.reference())))
        })
        .collect();
    let cells: Map<String, Value> = sheet
        .cells()
        .into_iter()
        .filter(|cell| !cell.formula().is_empty() || !cell.value().is_empty())
        .map(|cell| {
            let array = arrays.get(&(cell.coordinate().col_num(), cell.coordinate().row_num()));
            let formula = array.map(|(text, _)| *text)
                .or_else(|| (!cell.formula().is_empty()).then_some(cell.formula()));
            let value = formula.is_none().then(|| cell.value().into_owned());
            let style = cell.style();
            (
                cell.coordinate().to_string(),
                json!({
                    "formula": formula,
                    "value": value,
                    "format": style.number_format().map_or("General", |format| format.format_code()),
                    "bold": style.font().is_some_and(|font| font.bold()),
                    "array": array.map(|(_, reference)| *reference),
                    "hyperlink": cell.hyperlink().map(|link| link.url()),
                }),
            )
        })
        .collect();
    assert_eq!(Value::Object(cells), expected["cells"], "{label}: cells");

    let merges = sheet
        .merge_cells()
        .iter()
        .map(|range| json!(range.range()))
        .collect();
    assert_eq!(
        sorted(merges),
        sorted(expected["merges"].as_array().unwrap().clone()),
        "{label}: merges"
    );

    let comments = sheet
        .comments()
        .iter()
        .map(|comment| {
            let text = comment
                .text()
                .text()
                .map(|text| text.value().to_owned())
                .or_else(|| {
                    comment
                        .text()
                        .rich_text()
                        .map(|text| text.text().into_owned())
                })
                .unwrap();
            json!({"cell": comment.coordinate().to_string(), "text": text})
        })
        .collect();
    assert_eq!(
        sorted(comments),
        sorted(expected["comments"].as_array().unwrap().clone()),
        "{label}: comments"
    );

    let validations = sheet
        .data_validations()
        .into_iter()
        .flat_map(|validations| validations.data_validation_list())
        .flat_map(|validation| {
            validation
                .sequence_of_references()
                .range_collection()
                .iter()
                .flat_map(|range| get_coordinate_list(&range.range()))
                .map(move |(col, row)| {
                    json!({
                        "cell": coordinate_from_index(col, row),
                        "formula": format!("={}", validation.formula1()),
                    })
                })
        })
        .collect();
    assert_eq!(
        sorted(validations),
        sorted(expected["validations"].as_array().unwrap().clone()),
        "{label}: validations"
    );

    let formats = sheet
        .conditional_formatting_collection()
        .iter()
        .flat_map(|format| {
            format.conditional_collection().iter().map(move |rule| {
                conditional_format(
                    &format.sequence_of_references().get_sqref(),
                    &format!("={}", rule.formula().unwrap().address_str()),
                )
            })
        })
        .collect();
    let expected_formats = expected["conditional_formats"]
        .as_array()
        .unwrap()
        .iter()
        .map(|format| {
            conditional_format(
                format["range"].as_str().unwrap(),
                format["formula"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        sorted(formats),
        sorted(expected_formats),
        "{label}: conditional formats"
    );

    let mut shapes = Vec::new();
    let shape = |kind, from: &MarkerType, to: &MarkerType| {
        json!({
            "type": kind, "from": from.coordinate(), "to": to.coordinate(),
        })
    };
    for chart in sheet.chart_collection() {
        let anchor = chart.two_cell_anchor();
        shapes.push(shape(3, anchor.from_marker(), anchor.to_marker()));
    }
    for image in sheet.image_collection() {
        let anchor = image
            .two_cell_anchor()
            .expect("Excel authored a two-cell picture anchor");
        shapes.push(shape(13, anchor.from_marker(), anchor.to_marker()));
    }
    for frame in sheet.worksheet_drawing().slicer_frames() {
        let DrawingAnchor::TwoCell { from, to, .. } = frame.anchor() else {
            panic!("Excel authored a two-cell slicer anchor");
        };
        shapes.push(shape(25, from, to));
    }
    for comment in sheet.comments() {
        let anchor = comment.anchor();
        shapes.push(json!({
            "type": 4,
            "from": coordinate_from_index(anchor.left_column() + 1, anchor.top_row() + 1),
            "to": coordinate_from_index(anchor.right_column() + 1, anchor.bottom_row() + 1),
        }));
    }
    let expected_shapes = expected["shapes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|shape| {
            json!({
                "type": shape["type"], "from": shape["from"], "to": shape["to"],
            })
        })
        .collect();
    assert_eq!(
        sorted(shapes),
        sorted(expected_shapes),
        "{label}: drawing anchors"
    );

    let slicers = sheet
        .slicers()
        .iter()
        .map(|slicer| {
            let frame = sheet
                .worksheet_drawing()
                .slicer_frames()
                .iter()
                .find(|frame| frame.slicer_name() == slicer.name)
                .unwrap();
            let DrawingAnchor::TwoCell { from, .. } = frame.anchor() else {
                panic!("Excel authored a two-cell slicer anchor");
            };
            json!({"name": slicer.name, "caption": slicer.caption, "cell": from.coordinate()})
        })
        .collect();
    assert_eq!(
        sorted(slicers),
        sorted(expected["slicers"].as_array().unwrap().clone()),
        "{label}: slicer views"
    );
}

fn assert_chart_references(bytes: &[u8], expected: &Value, label: &str) {
    let mut package = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut references = Vec::new();
    for index in 0..package.len() {
        let mut part = package.by_index(index).unwrap();
        let path = part.name().unwrap();
        if !path.starts_with("xl/charts/chart") || !path.ends_with(".xml") {
            continue;
        }
        let mut xml = String::new();
        part.read_to_string(&mut xml).unwrap();
        let mut reader = Reader::from_str(&xml);
        loop {
            match reader.read_event().unwrap() {
                Event::Start(element) if element.local_name().as_ref() == b"f" => {
                    references.push(json!(
                        reader
                            .read_text(element.name())
                            .unwrap()
                            .decode()
                            .unwrap()
                            .into_owned()
                    ));
                }
                Event::Eof => break,
                _ => (),
            }
        }
    }
    // The measured fixture has no literal category or series-title references:
    // each SERIES argument containing '!' is a saved chart reference.
    let expected_references = expected["series"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|series| series.as_str().unwrap().split(','))
        .filter(|argument| argument.contains('!'))
        .map(|argument| json!(argument))
        .collect();
    assert_eq!(
        sorted(references),
        sorted(expected_references),
        "{label}: chart references"
    );
}

fn assert_drawing_order(bytes: &[u8], expected: &Value, label: &str) {
    let mut package = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut xml = String::new();
    package
        .by_name("xl/drawings/drawing1.xml")
        .unwrap()
        .read_to_string(&mut xml)
        .unwrap();
    let mut reader = Reader::from_str(&xml);
    let mut kind = None;
    let mut order = Vec::<u64>::new();
    loop {
        match reader.read_event().unwrap() {
            Event::Start(element) | Event::Empty(element) => match element.local_name().as_ref() {
                b"twoCellAnchor" => kind = None,
                b"chart" => kind = Some(3),
                b"pic" => kind = Some(13),
                b"slicer" => kind = Some(25),
                _ => (),
            },
            Event::End(element) if element.local_name().as_ref() == b"twoCellAnchor" => {
                order.push(kind.unwrap())
            }
            Event::Eof => break,
            _ => (),
        }
    }
    let expected: Vec<_> = expected["shapes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|shape| shape["type"].as_u64().unwrap())
        .filter(|kind| *kind != 4)
        .collect();
    assert_eq!(order, expected, "{label}: drawing stacking order");
}

#[rstest]
#[case("cut-negative")]
#[case("copy-negative")]
#[case("cut-positive")]
#[case("copy-positive")]
#[case("cut-overlap")]
#[case("copy-overlap")]
#[case("cut-rule-subset")]
#[case("copy-rule-subset")]
#[case("cut-array")]
#[case("copy-array")]
#[case("copy-partial-array")]
#[case("copy-array-child")]
#[case("cut-shared")]
#[case("copy-shared")]
#[case("cut-edge")]
#[case("copy-edge")]
#[case("cut-blank-rules")]
#[case("copy-blank-rules")]
fn range_operations_match_excel_saves(#[case] name: &str) {
    let answers: Value = serde_json::from_str(ANSWERS.trim_start_matches('\u{feff}')).unwrap();
    assert_eq!(
        Sha256::digest(BASE)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>(),
        answers["base_sha256"].as_str().unwrap()
    );
    let case = &answers["cases"][name];
    let source = case["source"].as_str().unwrap();
    let sheet_name = case["sheet"].as_str().unwrap();
    let row = i32::try_from(case["row"].as_i64().unwrap()).unwrap();
    let column = i32::try_from(case["column"].as_i64().unwrap()).unwrap();
    let mut book = open(BASE);
    let sheet = book.sheet_by_name_mut(sheet_name).unwrap();
    if case["move"].as_bool().unwrap() {
        sheet.move_range(source, row, column);
    } else {
        sheet.copy_range(source, row, column);
    }
    assert_sheet(sheet, &case["state"], name);
    // Worksheet has no access to a different sheet's formula consumers. The
    // workbook-level move entrypoint coordinates those edits.
    assert_eq!(
        book.sheet_by_name("Other")
            .unwrap()
            .cell("A1")
            .unwrap()
            .formula(),
        "Data!M18"
    );
    for light in [false, true] {
        let output = save(&book, light, name);
        // Data owns the fixture's charts; the other measured worksheets have
        // no chart objects, as assert_sheet's drawing inventory also checks.
        if sheet_name == "Data" {
            assert_chart_references(&output, &case["state"], name);
            assert_drawing_order(&output, &case["state"], name);
        }
        let reread = open(&output);
        assert_sheet(
            reread.sheet_by_name(sheet_name).unwrap(),
            &case["state"],
            name,
        );
    }
}

#[rstest]
#[case(false)]
#[case(true)]
fn blank_source_cells_replace_destination_cells(#[case] moving: bool) {
    let mut book = umya_spreadsheet::new_file();
    let sheet = book.sheet_mut(0).unwrap();
    sheet.set_name("Data");
    sheet.cell_mut("C3").set_value("source");
    sheet.cell_mut("A1").set_value("old");
    sheet.cell_mut("B2").set_value("overwritten by a blank");
    sheet
        .cell_mut("B2")
        .hyperlink_mut()
        .set_url("https://example.com/old");
    if moving {
        sheet.move_range("C3:D4", -2, -2);
    } else {
        sheet.copy_range("C3:D4", -2, -2);
    }
    assert_eq!(sheet.value("A1"), "source");
    assert!(sheet.value("B2").is_empty());
    assert!(
        sheet
            .cell("B2")
            .is_none_or(|cell| cell.hyperlink().is_none())
    );
    assert_eq!(sheet.value("C3"), if moving { "" } else { "source" });
    for light in [false, true] {
        let label = if moving { "blank-cut" } else { "blank-copy" };
        let output = save(&book, light, label);
        let reread = open(&output);
        let sheet = reread.sheet(0).unwrap();
        assert_eq!(sheet.value("A1"), "source");
        assert!(sheet.value("B2").is_empty());
        assert!(
            sheet
                .cell("B2")
                .is_none_or(|cell| cell.hyperlink().is_none())
        );
    }
}

#[rstest]
#[case("A1", i32::MIN, 0)]
#[case("A1", 0, i32::MIN)]
#[case("B2", i32::MAX, 0)]
#[case("B2", 0, i32::MAX)]
#[case("A1", -1, 0)]
#[case("A1", 0, -1)]
#[case("XFD1048576", 1, 0)]
#[case("XFD1048576", 0, 1)]
#[should_panic(expected = "Out of Range.")]
fn invalid_destinations_keep_the_existing_panic_contract(
    #[case] source: &str,
    #[case] row: i32,
    #[case] column: i32,
) {
    let mut book = umya_spreadsheet::new_file();
    book.sheet_mut(0).unwrap().move_range(source, row, column);
}

#[rstest]
#[case(false)]
#[case(true)]
fn legal_offsets_reach_both_grid_corners(#[case] moving: bool) {
    let mut book = umya_spreadsheet::new_file();
    let sheet = book.sheet_mut(0).unwrap();
    sheet.cell_mut("XFD1048576").set_value("corner");
    if moving {
        sheet.move_range("XFD1048576", -1_048_575, -16_383);
        sheet.move_range("A1", 1_048_575, 16_383);
    } else {
        sheet.copy_range("XFD1048576", -1_048_575, -16_383);
        assert_eq!(sheet.value("A1"), "corner");
        sheet.copy_range("A1", 1_048_575, 16_383);
    }
    assert_eq!(sheet.value("XFD1048576"), "corner");
}

#[rstest]
#[case(false)]
#[case(true)]
#[should_panic(expected = "Cannot change part of an array.")]
fn partial_arrays_are_rejected_before_relocation(#[case] moving: bool) {
    let mut book = open(BASE);
    let sheet = book.sheet_by_name_mut("Arrays").unwrap();
    if moving {
        sheet.move_range("C3", 5, 3);
    } else {
        sheet.copy_range("J10", -7, -7);
    }
}
