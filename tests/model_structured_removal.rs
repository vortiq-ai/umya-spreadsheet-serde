use std::io::Cursor;

use umya_spreadsheet::{
    Chart,
    ChartType,
    DefinedName,
    Workbook,
    drawing::spreadsheet::MarkerType,
    reader,
    writer,
};

fn write(book: &Workbook, light: bool) -> Vec<u8> {
    let mut bytes = Vec::new();
    if light {
        writer::xlsx::write_writer_light(book, &mut bytes).unwrap();
    } else {
        writer::xlsx::write_writer(book, &mut bytes).unwrap();
    }
    bytes
}

fn read(bytes: &[u8], eager: bool) -> Workbook {
    let mut book = reader::xlsx::read_reader(Cursor::new(bytes), eager).unwrap();
    book.try_read_sheet_collection().unwrap();
    book
}

fn input() -> Vec<u8> {
    let mut book = read(
        include_bytes!("fixtures/model_operations/geometry-table.xlsx"),
        true,
    );
    book.new_sheet("Refs").unwrap();
    for sheet_name in ["Data", "Refs"] {
        let index = book
            .sheet_collection()
            .iter()
            .position(|sheet| sheet.name() == sheet_name)
            .unwrap();
        let sheet = book.sheet_by_name_mut(sheet_name).unwrap();
        sheet.cell_mut("K20").set_formula("SUM(Sales[Amount])");
        sheet.cell_mut("K21").set_formula("SUM(Sales[Region])");
        sheet
            .cell_mut("K22")
            .set_formula("IF(TRUE,\"Sales[Amount]\",SUM(Sales[Amount]))");
        let mut name = DefinedName::default();
        name.set_name(format!("{}Amount", sheet.name()));
        name.set_address("SUM(Sales[Amount])");
        name.set_local_sheet_id(u32::try_from(index).unwrap());
        sheet.add_defined_names(name);
        let mut name = DefinedName::default();
        name.set_name("PlotAmount");
        name.set_address("Sales[Amount]");
        name.set_local_sheet_id(u32::try_from(index).unwrap());
        sheet.add_defined_names(name);
        let mut from = MarkerType::default();
        from.set_coordinate("M20");
        let mut to = MarkerType::default();
        to.set_coordinate("R30");
        let mut chart = Chart::default();
        let formula = format!("{sheet_name}!PlotAmount");
        chart.new_chart(&ChartType::LineChart, from, to, vec![&formula]);
        sheet.add_chart(chart);
    }
    let mut name = DefinedName::default();
    name.set_name("AllAmount");
    name.set_address("SUM(Sales[Amount])");
    book.add_defined_names(name);
    write(&book, false)
}

fn chart_formula(sheet: &umya_spreadsheet::Worksheet) -> String {
    sheet.chart_collection()[0]
        .chart_space()
        .chart()
        .plot_area()
        .line_chart()
        .unwrap()
        .area_chart_series_list()
        .area_chart_series()[0]
        .values()
        .unwrap()
        .number_reference()
        .formula()
        .address_str()
}

// Excel Round 1 identity probes measured #REF! for deleted table/column
// operands, including operands nested in SUM and defined names; literal strings
// and a chart's named formula survive. The chart uses the same range-name
// mechanism as the measured Excel identity fixture.
#[test]
fn workbook_table_and_column_removal_invalidates_structured_consumers() {
    let input = input();
    for eager in [false, true] {
        for operation in ["rows", "columns", "column"] {
            let mut book = read(&input, eager);
            match operation {
                "rows" => book.remove_row("Data", 5, 6),
                "columns" => book.remove_column("Data", "C", 3),
                "column" => book.remove_column("Data", "E", 1),
                _ => unreachable!(),
            }
            for light in [false, true] {
                let output = write(&book, light);
                let reread = read(&output, true);
                for model in [&book, &reread] {
                    let sheet = model.sheet_by_name("Refs").unwrap();
                    assert_eq!(sheet.cell("K20").unwrap().formula(), "SUM(#REF!)");
                    assert_eq!(
                        sheet.cell("K21").unwrap().formula(),
                        if operation == "column" {
                            "SUM(Sales[Region])"
                        } else {
                            "SUM(#REF!)"
                        }
                    );
                    assert_eq!(
                        sheet.cell("K22").unwrap().formula(),
                        "IF(TRUE,\"Sales[Amount]\",SUM(#REF!))"
                    );
                    assert_eq!(
                        model
                            .defined_names()
                            .iter()
                            .find(|name| name.name() == "AllAmount")
                            .unwrap()
                            .address(),
                        "SUM(#REF!)"
                    );
                    assert_eq!(
                        sheet
                            .defined_names()
                            .iter()
                            .find(|name| name.name() == "RefsAmount")
                            .unwrap()
                            .address(),
                        "SUM(#REF!)"
                    );
                    assert_eq!(
                        sheet
                            .defined_names()
                            .iter()
                            .find(|name| name.name() == "PlotAmount")
                            .unwrap()
                            .address(),
                        "#REF!"
                    );
                    assert_eq!(chart_formula(sheet), "Refs!PlotAmount");
                }
                keep_output(&format!("structured-{operation}-{eager}-{light}"), &output);
            }
        }
    }
}

#[test]
fn worksheet_column_removal_updates_its_local_structured_consumers() {
    let input = input();
    for eager in [false, true] {
        let mut book = read(&input, eager);
        book.sheet_by_name_mut("Data")
            .unwrap()
            .remove_column("E", 1);
        for light in [false, true] {
            let output = write(&book, light);
            let reread = read(&output, true);
            for model in [&book, &reread] {
                let sheet = model.sheet_by_name("Data").unwrap();
                assert_eq!(sheet.cell("J20").unwrap().formula(), "SUM(#REF!)");
                assert_eq!(
                    sheet
                        .defined_names()
                        .iter()
                        .find(|name| name.name() == "DataAmount")
                        .unwrap()
                        .address(),
                    "SUM(#REF!)"
                );
                assert_eq!(
                    sheet
                        .defined_names()
                        .iter()
                        .find(|name| name.name() == "PlotAmount")
                        .unwrap()
                        .address(),
                    "#REF!"
                );
                assert_eq!(chart_formula(sheet), "Data!PlotAmount");
            }
            keep_output(&format!("structured-local-column-{eager}-{light}"), &output);
        }
    }
}

#[test]
fn unsupported_partial_structured_range_removal_is_atomic() {
    for eager in [false, true] {
        for worksheet in [false, true] {
            let mut book = read(&input(), eager);
            book.sheet_by_name_mut("Data")
                .unwrap()
                .cell_mut("K23")
                .set_formula("SUM(Sales[[Region]:[Amount]])");
            let before = format!("{book:?}");
            if worksheet {
                book.sheet_by_name_mut("Data")
                    .unwrap()
                    .remove_column("E", 1);
            } else {
                book.remove_column("Data", "E", 1);
            }
            assert_eq!(format!("{book:?}"), before);
            for light in [false, true] {
                let output = write(&book, light);
                assert_eq!(
                    read(&output, true)
                        .sheet_by_name("Data")
                        .unwrap()
                        .cell("K23")
                        .unwrap()
                        .formula(),
                    "SUM(Sales[[Region]:[Amount]])"
                );
            }
        }
    }
}

fn keep_output(label: &str, bytes: &[u8]) {
    if let Some(directory) = std::env::var_os("SLICER_PACKAGE_OUTPUT_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join(format!("{label}.xlsx")), bytes).unwrap();
    }
}
