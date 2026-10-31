use std::{
    env,
    fs,
    io::Cursor,
    path::Path,
};

fn main() {
    let arguments: Vec<_> = env::args().collect();
    assert_eq!(
        arguments.len(),
        3,
        "usage: legacy-chart-json INPUT.xlsx OUTPUT_DIR"
    );
    let input = Path::new(&arguments[1]);
    let output = Path::new(&arguments[2]);
    fs::create_dir_all(output).unwrap();
    let bytes = fs::read(input).unwrap();
    let workbook = umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap();
    let sheet = workbook.sheet_collection().first().unwrap();
    let charts: Vec<umya_spreadsheet::Chart> = sheet.chart_collection().to_vec();
    assert!(
        !charts.is_empty(),
        "the authored workbook must contain a chart"
    );
    let name = input.file_stem().unwrap().to_str().unwrap();
    fs::write(
        output.join(format!("{name}.json")),
        serde_json::to_vec(&charts).unwrap(),
    )
    .unwrap();
    // Grouped drawings are not in the old consumer's chart collection. Preserve
    // their independently imported old serde as a companion fixture.
    fs::write(
        output.join(format!("{name}.drawing.json")),
        serde_json::to_vec(sheet.worksheet_drawing()).unwrap(),
    )
    .unwrap();
    let mut exported = Vec::new();
    umya_spreadsheet::writer::xlsx::write_writer(&workbook, &mut Cursor::new(&mut exported))
        .unwrap();
    fs::write(output.join(format!("{name}.xlsx")), exported).unwrap();
}
