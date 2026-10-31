use std::io::Cursor;

use umya_spreadsheet::{
    drawing::{
        charts::*,
        spreadsheet::ExtendedChart,
    },
    reader,
    writer,
};

#[test]
fn created_model_can_be_exported_with_imported_and_fresh_drawing_owners() {
    let mut imported = reader::xlsx::read_reader(
        Cursor::new(include_bytes!("fixtures/extended_charts/creation-pareto.xlsx").as_slice()),
        true,
    )
    .unwrap();
    let original = imported
        .sheet(0)
        .unwrap()
        .worksheet_drawing()
        .extended_charts()[0]
        .clone();
    let model = ChartExBuilder::pareto(
        ChartExFormula::new("Sheet1!$C$1:$C$4"),
        ChartExBinning {
            bin_count: Some(3),
            ..Default::default()
        },
    )
    .title("Sales & costs")
    .series_name("Revenue")
    .build()
    .unwrap();
    let mut fresh = umya_spreadsheet::new_file();
    for row in 1..=4 {
        imported
            .sheet_mut(0)
            .unwrap()
            .cell_mut((3, row))
            .set_value_number(row * 10);
        fresh
            .sheet_mut(0)
            .unwrap()
            .cell_mut((3, row))
            .set_value_number(row * 10);
    }
    let mut new_drawing = ExtendedChart::new(original.anchor().clone(), model.clone());
    *new_drawing.frame_properties_mut() = original.frame_properties().clone();
    *new_drawing.transform_mut() = original.transform().clone();
    *new_drawing.client_data_mut() = original.client_data().clone();
    new_drawing.set_compatibility(original.compatibility().cloned());
    new_drawing
        .frame_mut()
        .set_macro_name(original.frame().macro_name().map(str::to_owned));
    fresh
        .sheet_mut(0)
        .unwrap()
        .worksheet_drawing_mut()
        .add_extended_chart(new_drawing);
    imported
        .sheet_mut(0)
        .unwrap()
        .worksheet_drawing_mut()
        .extended_charts_mut()[0]
        .frame_mut()
        .set_chart_space(model);
    for (kind, book) in [
        ("imported-owner-pareto", imported),
        ("fresh-owner-pareto", fresh),
    ] {
        let mut bytes = Vec::new();
        writer::xlsx::write_writer(&book, &mut bytes).unwrap();
        let reread = reader::xlsx::read_reader(Cursor::new(&bytes), true).unwrap();
        assert_eq!(
            reread
                .sheet(0)
                .unwrap()
                .worksheet_drawing()
                .extended_charts()[0]
                .chart_space()
                .chart()
                .unwrap()
                .series()[1]
                .owner_idx,
            Some(0)
        );
        if let Some(target) = std::env::var_os("CARGO_TARGET_DIR") {
            let directory = std::path::PathBuf::from(target).join("chart-ex-validation");
            std::fs::create_dir_all(&directory).unwrap();
            std::fs::write(directory.join(format!("{kind}.xlsx")), bytes).unwrap();
        }
    }
}
