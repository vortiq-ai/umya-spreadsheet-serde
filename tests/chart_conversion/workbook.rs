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
    xml::{
        CHART,
        Element,
        parse,
    },
};

/// The workbook read back from a file whose chart is `part`.
pub fn workbook_with(part: &str) -> Workbook {
    let mut book = umya_spreadsheet::new_file();
    let sheet = book.sheet_mut(0).unwrap();
    sheet.set_name("Data");
    fill(sheet, &parse(part));
    sheet.add_chart(base_chart());
    let file = replace_chart_part(&write_workbook(&book), part);
    reader::xlsx::read_reader(Cursor::new(file), true).unwrap()
}

/// Every reference in the part is one column of `Data`; its cache holds one
/// value per cell, and a number cache the format of those cells.
fn fill(sheet: &mut Worksheet, space: &Element) {
    for (kind, cache_kind, numeric) in [("numRef", "numCache", true), ("strRef", "strCache", false)]
    {
        for reference in space.all_named(CHART, kind) {
            let formula = &reference.c("f").expect("a reference has c:f").text;
            let range = formula.strip_prefix("Data!").expect("a Data reference");
            let (first, last) = range.split_once(':').unwrap_or((range, range));
            let (column, first_row) = coordinate(first);
            assert_eq!(column, coordinate(last).0, "{formula} spans columns");

            let cache = reference.c(cache_kind).expect("a reference has a cache");
            let number_format = cache.c("formatCode").map(|code| code.text.as_str());
            for point in cache.all_c("pt") {
                let offset: u32 = point.attribute("idx").unwrap().parse().unwrap();
                let value = point.c("v").expect("a point has c:v").text.as_str();
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
