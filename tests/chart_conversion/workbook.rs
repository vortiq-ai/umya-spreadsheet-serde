//! A workbook around a before part: the part is its chart, and its `Data`
//! sheet holds the values the part's series refer to, taken from the part's
//! own caches.
use std::io::Cursor;

use umya_spreadsheet::{
    Workbook,
    Worksheet,
    reader,
};

use super::{
    fixture::{
        base_chart,
        replace_chart_part,
        write_workbook,
    },
    measured::Cells,
    xml::{
        CHART,
        Element,
        parse,
    },
};

/// The workbook read back from a file whose chart is `part`.
pub fn workbook_with(part: &str) -> Workbook {
    workbook_with_cells(part, &[])
}

/// The workbook read back from a file whose chart is `part`, its references
/// without a cache reading `cells`.
pub fn workbook_with_cells(part: &str, cells: &[Cells]) -> Workbook {
    let mut book = umya_spreadsheet::new_file();
    let sheet = book.sheet_mut(0).unwrap();
    sheet.set_name("Data");
    fill(sheet, &parse(part), cells);
    sheet.add_chart(base_chart());
    let file = replace_chart_part(&write_workbook(&book), part);
    reader::xlsx::read_reader(Cursor::new(file), true).unwrap()
}

/// The saved file of a workbook whose first sheet, `Chart`, holds only the
/// chart `part`, whose second sheet, `Data`, holds the values its series
/// refer to, and whose further sheets, empty, are named by `more`.
pub fn file_with_chart_sheet(part: &str, more: &[&str]) -> Vec<u8> {
    let mut book = umya_spreadsheet::new_file();
    let sheet = book.sheet_mut(0).unwrap();
    sheet.set_name("Chart");
    sheet.add_chart(base_chart());
    fill(book.new_sheet("Data").unwrap(), &parse(part), &[]);
    for name in more {
        book.new_sheet(*name).unwrap();
    }
    replace_chart_part(&write_workbook(&book), part)
}

/// Every reference in the part is one column of `Data`; its cache, or the
/// `cells` of its formula when it has none, holds one value per cell, and a
/// number cache the format of those cells.
fn fill(sheet: &mut Worksheet, space: &Element, cells: &[Cells]) {
    for (kind, cache_kind, numeric) in [("numRef", "numCache", true), ("strRef", "strCache", false)]
    {
        for reference in space.all_named(CHART, kind) {
            let formula = &reference.c("f").expect("a reference has c:f").text;
            let range = formula.strip_prefix("Data!").expect("a Data reference");
            let (first, last) = range.split_once(':').unwrap_or((range, range));
            let (column, first_row) = coordinate(first);
            assert_eq!(column, coordinate(last).0, "{formula} spans columns");

            let (number_format, points): (Option<&str>, Vec<(u32, &str)>) =
                match reference.c(cache_kind) {
                    Some(cache) => (
                        cache.c("formatCode").map(|code| code.text.as_str()),
                        cache
                            .all_c("pt")
                            .map(|point| {
                                let offset = point.attribute("idx").unwrap().parse().unwrap();
                                (offset, point.c("v").expect("a point has c:v").text.as_str())
                            })
                            .collect(),
                    ),
                    None => {
                        let held = cells
                            .iter()
                            .find(|held| held.formula == *formula && held.numeric == numeric)
                            .unwrap_or_else(|| panic!("{formula} has neither a cache nor cells"));
                        (
                            held.format.as_deref(),
                            held.points
                                .iter()
                                .map(|(offset, value)| (*offset, value.as_str()))
                                .collect(),
                        )
                    }
                };
            for (offset, value) in points {
                let cell = sheet.cell_mut((column, first_row + offset));
                if numeric {
                    cell.set_value_number(value.parse::<f64>().unwrap());
                    if let Some(code) = number_format {
                        cell.style_mut().number_format_mut().set_format_code(code);
                    }
                } else {
                    cell.set_value_string(value);
                }
            }
        }
    }
}

/// The column and row of `$C$5`, both counted from 1.
fn coordinate(address: &str) -> (u32, u32) {
    let address = address.replace('$', "");
    let split = address.find(|c: char| c.is_ascii_digit()).unwrap();
    let column = address[..split].bytes().fold(0, |column, letter| {
        column * 26 + u32::from(letter - b'A' + 1)
    });
    (column, address[split..].parse().unwrap())
}
