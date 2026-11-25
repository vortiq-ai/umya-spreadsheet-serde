//! `read_shared` reads a package held in memory as `read_reader` reads it, but
//! takes each sheet's XML from the package only when the sheet is read, and
//! parses it as it is decompressed.

use std::{
    io::Cursor,
    path::PathBuf,
    sync::Arc,
};

use umya_spreadsheet::{
    Workbook,
    reader::xlsx::{
        read_reader,
        read_shared,
    },
    writer::xlsx::write_writer,
};

fn test_workbooks() -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir("tests/test_files")
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "xlsx"))
        .collect();
    paths.sort();
    paths
}

/// What is compared, in a fixed order. `Cells` and other parts keep `HashMap`s,
/// which a serializer writes in each map's own order, so two books read
/// separately are compared through sorted views: every cell (value, formula,
/// style), the row and column dimensions, merges, conditional formats, names,
/// comments and validations, and how many charts and images each sheet has.
fn sheets(book: &Workbook) -> Vec<String> {
    let mut out = Vec::new();
    for sheet in book.sheet_collection_no_check() {
        out.push(format!("sheet {}", sheet.name()));
        out.extend(sheet.cells_sorted().iter().map(|cell| format!("{cell:?}")));
        let mut rows: Vec<String> = sheet
            .row_dimensions()
            .iter()
            .map(|row| format!("{row:?}"))
            .collect();
        rows.sort();
        out.extend(rows);
        out.extend(
            sheet
                .column_dimensions()
                .iter()
                .map(|column| format!("{column:?}")),
        );
        out.extend(
            sheet
                .merge_cells()
                .iter()
                .map(umya_spreadsheet::Range::range),
        );
        out.extend(
            sheet
                .conditional_formatting_collection()
                .iter()
                .map(|formatting| format!("{formatting:?}")),
        );
        out.extend(sheet.defined_names().iter().map(|name| format!("{name:?}")));
        out.extend(
            sheet
                .comments()
                .iter()
                .map(|comment| format!("{comment:?}")),
        );
        out.extend(
            sheet
                .threaded_comments()
                .iter()
                .map(|comment| format!("{comment:?}")),
        );
        out.push(format!("{:?}", sheet.data_validations()));
        out.push(format!(
            "{} charts, {} images",
            sheet.chart_collection().len(),
            sheet.image_collection().len()
        ));
        // Each picture's image, by its bytes: an image a shared package reads only when a
        // picture takes it is the same image.
        out.extend(sheet.image_collection().iter().map(|image| {
            format!(
                "image {} {} bytes, {:016x}",
                image.image_name(),
                image.image_data().len(),
                fnv(image.image_data())
            )
        }));
    }
    out
}

/// FNV-1a over `bytes`: a fingerprint for comparing images, not a hash to trust.
fn fnv(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, &byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

/// A chart's picture fill is read from the package into the chart when its sheet is read, and
/// then the chart holds its image itself: nothing in it keeps the package. A chart stored
/// after an import outlives the file it came from (the engine maps the upload and deletes it
/// once the import returns), and a deferred image in it would be written without its bytes.
#[test]
fn a_chart_picture_fill_is_read_into_the_chart_and_keeps_no_package() {
    use umya_spreadsheet::{
        Chart,
        ChartType,
        drawing::{
            BlipFill,
            charts::ShapeProperties,
            spreadsheet::MarkerType,
        },
        new_file,
    };
    let image: Vec<u8> = b"\x89PNG\r\n\x1a\n"
        .iter()
        .copied()
        .chain((0..8192u32).map(|i| (i.wrapping_mul(2_654_435_761) >> 24) as u8))
        .collect();
    let mut book = new_file();
    for row in 1..=3 {
        book.sheet_mut(0).unwrap().cell_mut((2, row)).set_value_number(row * 10);
    }
    let (mut from, mut to) = (MarkerType::default(), MarkerType::default());
    from.set_coordinate("E1");
    to.set_coordinate("M20");
    let mut chart = Chart::default();
    chart.new_chart(&ChartType::LineChart, from, to, vec!["Sheet1!B1:B3"]);
    let mut fill = BlipFill::default();
    fill.blip_mut().image_mut().set_image_name("fill.png").set_image_data(image.clone());
    let mut shape = ShapeProperties::default();
    shape.set_blip_fill(fill);
    chart.chart_space_mut().set_shape_properties(shape);
    book.sheet_mut(0).unwrap().add_chart(chart);
    let mut written = Vec::new();
    write_writer(&book, &mut Cursor::new(&mut written)).unwrap();

    let fill_of = |chart: &Chart| -> Vec<u8> {
        chart
            .chart_space()
            .shape_properties()
            .and_then(|shape| shape.blip_fill())
            .map(|fill| fill.blip().image().image_data().to_vec())
            .unwrap_or_default()
    };
    let eager = read_reader(Cursor::new(&written), true).unwrap();
    assert_eq!(fill_of(&eager.sheet(0).unwrap().chart_collection()[0]), image, "read_reader");

    let package = Arc::new(written);
    let mut shared = read_shared(Arc::clone(&package), false).unwrap();
    shared.try_read_sheet(0).unwrap();
    let kept = shared.sheet(0).unwrap().chart_collection()[0].clone();
    drop(shared);
    assert_eq!(Arc::strong_count(&package), 1, "the chart keeps a handle to the package");
    assert_eq!(fill_of(&kept), image, "read_shared, the package gone");
}

/// The first line where two signatures differ, not both whole books.
fn assert_same(left: &[String], right: &[String], label: &str) {
    if let Some(index) = (0..left.len().max(right.len())).find(|&i| left.get(i) != right.get(i)) {
        panic!(
            "{label}: differs at line {index}:
  {:?}
  {:?}",
            left.get(index),
            right.get(index)
        );
    }
}

fn read_every_sheet(mut book: Workbook) -> Workbook {
    for index in 0..book.sheet_count() {
        book.try_read_sheet(index).unwrap();
    }
    book
}

/// Every test workbook `read_reader` reads gives the same sheets read from a
/// shared package, read all at once and read one sheet at a time.
#[test]
fn every_test_workbook_reads_the_same_from_a_shared_package() {
    let mut compared = 0;
    for path in test_workbooks() {
        let bytes = std::fs::read(&path).unwrap();
        let Ok(eager) = read_reader(Cursor::new(&bytes), true) else {
            continue;
        };
        let shared = read_shared(Arc::new(bytes.clone()), true)
            .unwrap_or_else(|error| panic!("{path:?} read eagerly from a shared package: {error}"));
        assert_same(
            &sheets(&shared),
            &sheets(&eager),
            &format!("{path:?}, every sheet at once"),
        );

        let lazy = read_every_sheet(read_reader(Cursor::new(&bytes), false).unwrap());
        let shared_lazy = read_every_sheet(read_shared(Arc::new(bytes), false).unwrap());
        assert_same(
            &sheets(&shared_lazy),
            &sheets(&lazy),
            &format!("{path:?}, one sheet at a time"),
        );
        compared += 1;
    }
    assert!(
        compared >= 50,
        "only {compared} test workbooks were compared"
    );
}

/// A workbook whose sheets were never read writes them from the package:
/// written and read back, it is what the same workbook read by `read_reader`
/// (and never read either) writes.
#[test]
fn sheets_never_read_are_written_from_the_package() {
    let write = |book: &Workbook| -> Option<Vec<u8>> {
        let mut written = Vec::new();
        write_writer(book, &mut Cursor::new(&mut written)).ok()?;
        Some(written)
    };
    let mut compared = 0;
    for path in test_workbooks() {
        let bytes = std::fs::read(&path).unwrap();
        let Ok(unread) = read_reader(Cursor::new(&bytes), false) else {
            continue;
        };
        let Some(from_reader) = write(&unread) else {
            continue;
        };
        let shared_unread = read_shared(Arc::new(bytes), false).unwrap();
        let from_shared = write(&shared_unread)
            .unwrap_or_else(|| panic!("{path:?}: the reader's book writes, the shared one must"));
        // A book written without reading its sheets does not always read back
        // (a sheet whose comments need the sheet read first, say); that
        // is the reader's limit, not this one's.
        let Ok(reread_reader) = read_reader(Cursor::new(&from_reader), true) else {
            eprintln!("{path:?}: written unread by read_reader, it does not read back; skipped");
            continue;
        };
        let reread_shared = read_reader(Cursor::new(&from_shared), true).unwrap_or_else(|error| {
            panic!("{path:?}: read_reader's unread book reads back, the shared one: {error}")
        });
        assert_same(
            &sheets(&reread_shared),
            &sheets(&reread_reader),
            &format!("{path:?}"),
        );
        compared += 1;
    }
    assert!(
        compared >= 50,
        "only {compared} test workbooks were compared"
    );
}
