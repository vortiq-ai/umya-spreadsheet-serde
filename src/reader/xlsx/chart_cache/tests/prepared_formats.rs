use std::panic::catch_unwind as unwind;

use super::{
    repeats::{
        add_part,
        capture,
    },
    *,
};

// An imported styles.xml can exhaust the escape VM. Both eager and lazy
// chart reads must return a literal first, or raise raw bounds before the
// saved escape error. Patch the package so the writer cannot mask read order.
#[test]
fn chart_reads_preserve_literal_returns_and_format_failure_order() {
    let mut book = crate::new_file();
    let cell = book.sheet_mut(0).unwrap().cell_mut("A1");
    cell.set_value_number(-1);
    cell.style_mut()
        .number_format_mut()
        .set_format_code("0.0000000000");
    add_part(
        &mut book,
        &SourceCacheSession::immediate(),
        "formats.xml",
        &[(
            "Sheet1!A1",
            "<c:strCache><c:ptCount val=\"1\"/><c:pt idx=\"0\"><c:v>seed</c:v></c:pt></c:strCache>",
        )],
    );
    let mut package = Vec::new();
    crate::writer::xlsx::write_writer(&book, &mut Cursor::new(&mut package)).unwrap();
    let literal = "x".repeat(2_000_000);
    let code = format!("0;\"{literal}\"");
    let raw_code = format!("0;0;0;0;0;[Red]0{literal}");
    for (code, positive, expected) in [
        (code.as_str(), false, None),
        (
            code.as_str(),
            true,
            Some(
                "called `Result::unwrap()` on an `Err` value: RuntimeError(BacktrackLimitExceeded)",
            ),
        ),
        (
            raw_code.as_str(),
            true,
            Some("index out of bounds: the len is 5 but the index is 5"),
        ),
    ] {
        let attribute = format!("formatCode=\"{}\"", quick_xml::escape::escape(code));
        let mut bytes = rewrite_package_bytes(
            &package,
            "xl/styles.xml",
            b"formatCode=\"0.0000000000\"",
            attribute.as_bytes(),
        );
        if positive {
            bytes = rewrite_package_bytes(
                &bytes,
                "xl/worksheets/sheet1.xml",
                b"<v>-1</v>",
                b"<v>1</v>",
            );
        }
        for eager in [true, false] {
            let before = crate::helper::number_format::preparation_count();
            let observed = unwind(|| {
                let mut imported =
                    crate::reader::xlsx::read_reader(Cursor::new(bytes.clone()), eager).unwrap();
                let sheet = imported.sheet_mut(0).unwrap();
                assert_eq!(sheet.name(), "Sheet1");
                if expected.is_none() {
                    assert_eq!(
                        crate::helper::number_format::preparation_count() - before,
                        1,
                        "eager={eager}"
                    );
                    assert_eq!(sheet.cell("A1").unwrap().formatted_value(), literal);
                }
            });
            match (observed, expected) {
                (Ok(()), None) => (),
                (Err(payload), Some(expected)) => {
                    assert_eq!(panic_message(payload), expected, "eager={eager}")
                }
                (Ok(()), Some(expected)) => panic!("eager={eager}: missing panic {expected}"),
                (Err(payload), None) => {
                    panic!("eager={eager}: unexpected panic {}", panic_message(payload))
                }
            }
        }
    }
}

// Distinct format codes are prepared once across different cells and caches,
// and a second capture owns a fresh map rather than keeping a global cache.
#[test]
fn numeric_chart_displays_prepare_each_code_once_per_capture() {
    for _ in 0..2 {
        let mut book = crate::new_file();
        for (row, code) in [(1, "0.00"), (2, "0.00"), (3, "0.0%"), (4, "0.0%")] {
            let cell = book.sheet_mut(0).unwrap().cell_mut((1, row));
            cell.set_value_number(0.5);
            cell.style_mut().number_format_mut().set_format_code(code);
        }
        let mut session = SourceCacheSession::deferred();
        // Different formulae and cache bytes avoid repeat-memo reuse.
        add_part(
            &mut book,
            &session,
            "formats.xml",
            &[
                (
                    "Sheet1!A1:A2",
                    "<c:strCache><c:ptCount val=\"2\"/><c:pt \
                     idx=\"0\"><c:v>0.50</c:v></c:pt><c:pt \
                     idx=\"1\"><c:v>0.50</c:v></c:pt></c:strCache>",
                ),
                (
                    "Sheet1!A3:A4",
                    "<c:strCache><c:ptCount val=\"2\"/><c:pt \
                     idx=\"0\"><c:v>50.0%</c:v></c:pt><c:pt \
                     idx=\"1\"><c:v>50.0%</c:v></c:pt></c:strCache>",
                ),
            ],
        );
        let before = crate::helper::number_format::preparation_count();
        capture(&mut book, &mut session, &mut lazy_provider());
        assert_eq!(
            crate::helper::number_format::preparation_count() - before,
            2
        );
        // Half formatted to two decimal places and half*100 to one decimal
        // percent reproduce all four labels, so both sources are eliminated.
        assert_eq!(book.chart_source_cache_report().reduced_caches, 2);
        assert_eq!(book.chart_source_cache_report().kept_caches, 0);
        let mut package = Vec::new();
        crate::writer::xlsx::write_writer(&book, &mut Cursor::new(&mut package)).unwrap();
        for eager in [true, false] {
            let before = crate::helper::number_format::preparation_count();
            let mut imported =
                crate::reader::xlsx::read_reader(Cursor::new(package.clone()), eager).unwrap();
            assert_eq!(imported.sheet_mut(0).unwrap().name(), "Sheet1");
            assert_eq!(
                crate::helper::number_format::preparation_count() - before,
                2
            );
            assert_eq!(imported.chart_source_cache_report().reduced_caches, 2);
            assert_eq!(imported.chart_source_cache_report().kept_caches, 0);
        }
    }
}
