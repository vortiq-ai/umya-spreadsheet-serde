//! The cells a conversion reads. A workbook read lazily converts as the same
//! file read eagerly; a sheet is read only when a reference reads its cells;
//! a reference whose range umya's cell reader cannot project is refused
//! before anything is written, with the chart unchanged.
use std::io::{
    Cursor,
    Read,
    Write,
};

use umya_spreadsheet::{
    Workbook,
    drawing::charts::{
        ChartGroupMut,
        Formula,
    },
    helper::chart_conversion::{
        ChartConversionRefusal,
        ChartSource,
        ChartSubtype,
    },
    reader,
};

use super::{
    conversion::{
        chart,
        chart_part,
        convert,
        replaced,
    },
    fixture::{
        replace_chart_part,
        write_workbook,
    },
    measured::part_xml,
    workbook::{
        file_with_chart_sheet,
        workbook_with,
    },
};

/// Three series of six values, named by one cell each (`Data!$B$1`).
const COLUMNS: &str = "before/conversions2/bare/colClustered.chart.xml";
const SOURCE: ChartSource = ChartSource::Subtype(ChartSubtype::ClusteredColumn);

fn read(file: &[u8], eagerly: bool) -> Workbook {
    reader::xlsx::read_reader(Cursor::new(file.to_vec()), eagerly).unwrap()
}

/// `file` with its part `name` holding `data`.
fn with_part(file: &[u8], name: &str, data: &[u8]) -> Vec<u8> {
    let mut input = zip::ZipArchive::new(Cursor::new(file)).unwrap();
    let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let mut replaced = false;
    for index in 0..input.len() {
        let mut entry = input.by_index(index).unwrap();
        let entry_name = entry.name().unwrap().into_owned();
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).unwrap();
        if entry_name == name {
            bytes = data.to_vec();
            replaced = true;
        }
        output
            .start_file(entry_name, zip::write::SimpleFileOptions::default())
            .unwrap();
        output.write_all(&bytes).unwrap();
    }
    assert!(replaced, "{name} is not in the file");
    output.finish().unwrap().into_inner()
}

/// A worksheet part whose text is not the UTF-8 it declares: the byte 0xFF,
/// in a comment, begins no UTF-8 character.
fn corrupt_sheet() -> Vec<u8> {
    let mut sheet = concat!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
        r#"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">"#,
        "<sheetData/><!-- "
    )
    .as_bytes()
    .to_vec();
    sheet.push(0xFF);
    sheet.extend_from_slice(b" --></worksheet>");
    sheet
}

/// `file` with its sheet at `index` (counted from zero) unreadable. A fresh
/// lazy read of it fails to read that sheet, and only that one.
fn with_corrupt_sheet(file: &[u8], index: usize) -> Vec<u8> {
    let corrupt = with_part(
        file,
        &format!("xl/worksheets/sheet{}.xml", index + 1),
        &corrupt_sheet(),
    );
    let mut book = read(&corrupt, false);
    assert!(book.try_read_sheet(index).is_err(), "sheet {index} reads");
    corrupt
}

/// The chart as a typed value, materialising its sheet.
fn typed(book: &mut Workbook) -> String {
    format!("{:?}", chart(book))
}

/// Excel's bubble sizes for a series of six values: a literal of six 1s.
fn six_unit_sizes() -> String {
    let points: String = (0..6)
        .map(|index| format!(r#"<c:pt idx="{index}"><c:v>1</c:v></c:pt>"#))
        .collect();
    format!(
        r#"<c:bubbleSize><c:numLit><c:formatCode>General</c:formatCode><c:ptCount val="6"/>{points}</c:numLit></c:bubbleSize>"#
    )
}

/// The chart on a sheet of its own, its values on `Data`: read lazily, both
/// sheets wait to be read, and the conversion reads them.
#[test]
fn a_lazily_read_workbook_converts_as_it_does_read_eagerly() {
    let xml = part_xml(COLUMNS);
    // A series named by one cell is a range without an end; it converts.
    assert!(
        xml.contains("<c:f>Data!$B$1</c:f>"),
        "series names in one cell"
    );
    let file = file_with_chart_sheet(&xml, &[]);
    let mut eager = read(&file, true);
    let mut lazy = read(&file, false);
    assert!(
        lazy.sheet(0).is_err() && lazy.sheet(1).is_err(),
        "both sheets wait"
    );
    let eager_report = convert(&mut eager, ChartSubtype::Bubble).unwrap();
    let lazy_report = convert(&mut lazy, ChartSubtype::Bubble).unwrap();
    assert_eq!(lazy_report, eager_report);
    let converted = chart_part(&lazy);
    assert_eq!(converted, chart_part(&eager));
    assert_eq!(
        converted.matches(&six_unit_sizes()).count(),
        3,
        "{converted}"
    );
}

/// The checks that need no cells answer first; then a sheet the chart reads
/// that cannot be read refuses the conversion, and the chart is unchanged.
#[test]
fn a_sheet_that_cannot_be_read_refuses_after_the_checks_that_need_no_cells() {
    let file = with_corrupt_sheet(&file_with_chart_sheet(&part_xml(COLUMNS), &[]), 1);
    let mut book = read(&file, false);
    let unchanged = typed(&mut read(&file, false));
    let same = convert(&mut book, ChartSubtype::ClusteredColumn).unwrap();
    assert!(same.changes().is_empty(), "{same:?}");
    assert_eq!(
        convert(&mut book, ChartSubtype::OpenHighLowClose),
        Err(ChartConversionRefusal::SeriesCountMismatch {
            required: 4,
            actual:   3,
        })
    );
    assert_eq!(
        convert(&mut book, ChartSubtype::Bubble),
        Err(ChartConversionRefusal::Unsupported {
            from: Some(SOURCE),
            to:   ChartSubtype::Bubble,
        })
    );
    assert_eq!(typed(&mut book), unchanged);
}

#[test]
fn a_chart_whose_sheet_cannot_be_read_is_not_found() {
    let file = with_corrupt_sheet(&file_with_chart_sheet(&part_xml(COLUMNS), &[]), 0);
    assert_eq!(
        convert(&mut read(&file, false), ChartSubtype::Bubble),
        Err(ChartConversionRefusal::ChartNotFound)
    );
}

/// A full-range extension names a sheet without reading its cells: the
/// conversion leaves that sheet unread, and it may be one that cannot be read.
#[test]
fn a_sheet_named_only_by_reference_metadata_is_not_read() {
    let full_range = concat!(
        r#"<c:extLst><c:ext uri="{02D57815-91ED-43cb-92C2-25804820EDAC}" "#,
        r#"xmlns:c15="http://schemas.microsoft.com/office/drawing/2012/chart">"#,
        r#"<c15:fullRef><c15:sqref>Meta!$B$2:$B$9</c15:sqref></c15:fullRef>"#,
        r#"</c:ext></c:extLst></c:numRef>"#
    );
    let xml = part_xml(COLUMNS);
    let first_values = xml.find("<c:val>").unwrap();
    let end = first_values + xml[first_values..].find("</c:numRef>").unwrap();
    let xml = format!(
        "{}{full_range}{}",
        &xml[..end],
        &xml[end + "</c:numRef>".len()..]
    );
    let file = file_with_chart_sheet(&xml, &["Meta"]);
    let mut readable = read(&file, true);
    let mut lazy = read(&with_corrupt_sheet(&file, 2), false);
    let target = ChartSubtype::StackedColumn;
    let expected = convert(&mut readable, target).unwrap();
    assert_eq!(convert(&mut lazy, target).unwrap(), expected);
    assert!(lazy.sheet(2).is_err(), "the conversion read Meta");
    let converted = chart_part(&lazy);
    assert!(converted.contains("<c15:sqref>Meta!$B$2:$B$9</c15:sqref>"));
    assert_eq!(converted, chart_part(&readable));
}

/// Converts the same file read eagerly and read lazily with the sheet at
/// `corrupt` unreadable, to the first target the eager read converts with a
/// change: the conversion reads no cell of that sheet, so both give one report
/// and one chart part.
fn assert_converts_without_reading(file: &[u8], corrupt: usize) -> String {
    let (target, expected) = ChartSubtype::ALL
        .iter()
        .find_map(|&target| {
            convert(&mut read(file, true), target)
                .ok()
                .filter(|report| !report.changes().is_empty())
                .map(|report| (target, report))
        })
        .expect("the workbook converts to some target");
    let mut readable = read(file, true);
    let mut lazy = read(&with_corrupt_sheet(file, corrupt), false);
    convert(&mut readable, target).unwrap();
    assert_eq!(convert(&mut lazy, target).unwrap(), expected, "{target:?}");
    assert!(
        lazy.sheet(corrupt).is_err(),
        "the conversion read the sheet"
    );
    let converted = chart_part(&lazy);
    assert_eq!(converted, chart_part(&readable), "{target:?}");
    converted
}

/// A multi-level category reference writes its own cache and reads no cells:
/// the sheet it names may be one that cannot be read.
#[test]
fn a_sheet_named_only_by_a_multi_level_reference_is_not_read() {
    let levels = concat!(
        r#"<c:cat><c:multiLvlStrRef><c:f>Meta!$A$2:$B$7</c:f><c:multiLvlStrCache>"#,
        r#"<c:ptCount val="6"/><c:lvl><c:pt idx="0"><c:v>C1</c:v></c:pt></c:lvl>"#,
        r#"</c:multiLvlStrCache></c:multiLvlStrRef></c:cat>"#
    );
    // Every series is multi-level: a mixture with single-level categories is
    // a different case, and the conversion waits on it.
    let mut xml = part_xml(COLUMNS);
    let mut from = 0;
    while let Some(found) = xml[from..].find("<c:cat>") {
        let start = from + found;
        let end = start + xml[start..].find("</c:cat>").unwrap() + "</c:cat>".len();
        xml = format!("{}{levels}{}", &xml[..start], &xml[end..]);
        from = start + levels.len();
    }
    let file = file_with_chart_sheet(&xml, &["Meta"]);
    let converted = assert_converts_without_reading(&file, 2);
    assert!(
        converted.contains("<c:f>Meta!$A$2:$B$7</c:f>"),
        "{converted}"
    );
}

/// The first series' values formula.
fn values_formula(book: &mut Workbook) -> &mut Formula {
    let ChartGroupMut::BarChart(group) = chart(book)
        .plot_area_mut()
        .chart_groups_mut()
        .next()
        .unwrap()
    else {
        panic!("a bar chart");
    };
    group.area_chart_series_list_mut().area_chart_series_mut()[0]
        .values_mut()
        .unwrap()
        .number_reference_mut()
        .formula_mut()
}

/// Fails unless converting `book` to a bubble chart is refused as
/// `Unsupported` with the typed chart unchanged.
fn refused_unchanged(mut book: Workbook, what: &str) {
    let before = typed(&mut book);
    assert_eq!(
        convert(&mut book, ChartSubtype::Bubble),
        Err(ChartConversionRefusal::Unsupported {
            from: Some(SOURCE),
            to:   ChartSubtype::Bubble,
        }),
        "{what}"
    );
    assert_eq!(
        typed(&mut book),
        before,
        "{what}: the refusal changed the chart"
    );
}

/// Whole columns and rows, saved in a file or set on the typed reference;
/// zero coordinates and reversed ranges, set on the typed reference.
#[test]
fn ranges_the_cell_reader_cannot_project_are_refused_unchanged() {
    let xml = part_xml(COLUMNS);
    let saved = write_workbook(&workbook_with(&xml));
    for (whole, spelled) in [("column", "Data!$B:$B"), ("row", "Data!$2:$2")] {
        let edited = replaced(
            &xml,
            "<c:f>Data!$B$2:$B$7</c:f>",
            &format!("<c:f>{spelled}</c:f>"),
        );
        let mut book = read(&replace_chart_part(&saved, &edited), true);
        assert_eq!(values_formula(&mut book).address_str(), spelled);
        refused_unchanged(book, &format!("a whole {whole} saved in the file"));
    }
    let edits: [(&str, fn(&mut Formula)); 5] = [
        ("a whole column set on the reference", |formula| {
            formula.address_mut().range_mut().set_range("$B:$B");
        }),
        ("a whole row set on the reference", |formula| {
            formula.address_mut().range_mut().set_range("$2:$2");
        }),
        ("a zero column", |formula| {
            let range = formula.address_mut().range_mut();
            range.coordinate_start_col_mut().unwrap().set_num(0);
        }),
        ("a zero row", |formula| {
            let range = formula.address_mut().range_mut();
            range.coordinate_start_row_mut().unwrap().set_num(0);
        }),
        ("a reversed range", |formula| {
            formula.address_mut().range_mut().set_range("$B$7:$B$2");
        }),
    ];
    for (what, edit) in edits {
        let mut book = workbook_with(&xml);
        edit(values_formula(&mut book));
        refused_unchanged(book, what);
    }
}
