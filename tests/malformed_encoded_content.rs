//! A package part that declares a non-UTF-8 encoding and carries bytes that are
//! not valid in it is rejected with an error from the reader; no part may make
//! the library panic. The packages are authored here: the text bytes are an
//! incomplete Shift_JIS character (the lead byte 0x82 needs a trail byte, and
//! the next byte is `<` or a quote), placed in every kind of position the
//! readers of the worksheet, shared strings and styles parts consume.
#[path = "support/slicer_package_fixture.rs"]
mod fixture;

use std::io::{
    Cursor,
    Write,
};

use umya_spreadsheet::{
    Workbook,
    XlsxError,
};
use zip::{
    ZipWriter,
    write::SimpleFileOptions,
};

const MAIN: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
const DECLARATION: &str = r#"<?xml version="1.0" encoding="Shift_JIS" standalone="yes"?>"#;
const REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

fn package(sheet: &[u8], shared_strings: &[u8], styles: &[u8]) -> Vec<u8> {
    let types = format!(
        r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/><Override PartName="/xl/sharedStrings.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sharedStrings+xml"/><Override PartName="/xl/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"/></Types>"#
    );
    let root_rels = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#;
    let workbook = format!(
        r#"<workbook xmlns="{MAIN}" xmlns:r="{REL}"><sheets><sheet name="Plan1" sheetId="1" r:id="rId1"/></sheets></workbook>"#
    );
    let workbook_rels = format!(
        r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="{REL}/worksheet" Target="sheet1.xml"/><Relationship Id="rId2" Type="{REL}/sharedStrings" Target="sharedStrings.xml"/><Relationship Id="rId3" Type="{REL}/styles" Target="styles.xml"/></Relationships>"#
    );
    let parts: [(&str, &[u8]); 7] = [
        ("[Content_Types].xml", types.as_bytes()),
        ("_rels/.rels", root_rels.as_bytes()),
        ("xl/workbook.xml", workbook.as_bytes()),
        ("xl/_rels/workbook.xml.rels", workbook_rels.as_bytes()),
        ("xl/sheet1.xml", sheet),
        ("xl/sharedStrings.xml", shared_strings),
        ("xl/styles.xml", styles),
    ];
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, data) in parts {
        zip.start_file(name, SimpleFileOptions::default()).unwrap();
        zip.write_all(data).unwrap();
    }
    zip.finish().unwrap().into_inner()
}

fn good_sheet() -> Vec<u8> {
    format!(
        r#"<worksheet xmlns="{MAIN}"><sheetData><row r="1"><c r="A1" t="s"><v>0</v></c></row></sheetData></worksheet>"#
    )
    .into_bytes()
}

fn good_shared_strings() -> Vec<u8> {
    format!(r#"<sst xmlns="{MAIN}" count="1" uniqueCount="1"><si><t>plain</t></si></sst>"#)
        .into_bytes()
}

fn good_styles() -> Vec<u8> {
    format!(
        r#"<styleSheet xmlns="{MAIN}"><fonts count="1"><font><sz val="11"/><name val="Calibri"/></font></fonts><fills count="1"><fill><patternFill patternType="none"/></fill></fills><borders count="1"><border><left/><right/><top/><bottom/><diagonal/></border></borders><cellXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellXfs></styleSheet>"#
    )
    .into_bytes()
}

/// `template` with `{BAD}` replaced by an incomplete Shift_JIS character and
/// the declaration put in front.
fn declared(template: &str) -> Vec<u8> {
    let (before, after) = template.split_once("{BAD}").expect("marker");
    let mut bytes = DECLARATION.as_bytes().to_vec();
    bytes.extend_from_slice(before.as_bytes());
    bytes.push(0x82);
    bytes.extend_from_slice(after.as_bytes());
    bytes
}

fn declared_good(template: Vec<u8>) -> Vec<u8> {
    let mut bytes = DECLARATION.as_bytes().to_vec();
    bytes.extend(template);
    bytes
}

fn read(bytes: Vec<u8>) -> Result<Workbook, XlsxError> {
    let mut book = umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(bytes), true)?;
    book.try_read_sheet_collection()?;
    Ok(book)
}

fn expect_rejected(label: &str, bytes: Vec<u8>) {
    let result = read(bytes);
    let error = result.err().unwrap_or_else(|| panic!("{label}: must fail"));
    let message = error.to_string();
    assert!(!message.is_empty(), "{label}");
}

#[test]
fn the_well_formed_encoded_package_reads() {
    let book = read(package(
        &declared_good(good_sheet()),
        &declared_good(good_shared_strings()),
        &declared_good(good_styles()),
    ))
    .unwrap();
    assert_eq!(
        book.sheet_by_name("Plan1")
            .unwrap()
            .cell("A1")
            .unwrap()
            .value(),
        "plain"
    );
}

#[test]
fn complete_shift_jis_characters_in_shared_strings_and_styles_are_read() {
    // 0x82 0xA0 is HIRAGANA LETTER A (U+3042) in Shift_JIS.
    let mut shared = DECLARATION.as_bytes().to_vec();
    shared.extend_from_slice(
        format!(r#"<sst xmlns="{MAIN}" count="1" uniqueCount="1"><si><t>"#).as_bytes(),
    );
    shared.extend_from_slice(&[0x82, 0xA0]);
    shared.extend_from_slice(b"</t></si></sst>");
    let mut styles = DECLARATION.as_bytes().to_vec();
    styles.extend_from_slice(
        format!(r#"<styleSheet xmlns="{MAIN}"><fonts count="1"><font><sz val="11"/><name val=""#)
            .as_bytes(),
    );
    styles.extend_from_slice(&[0x82, 0xA0]);
    styles.extend_from_slice(
        br#""/></font></fonts><fills count="1"><fill><patternFill patternType="none"/></fill></fills><borders count="1"><border><left/><right/><top/><bottom/><diagonal/></border></borders><cellXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellXfs></styleSheet>"#,
    );
    let sheet = format!(
        r#"<worksheet xmlns="{MAIN}"><sheetData><row r="1"><c r="A1" s="0" t="s"><v>0</v></c></row></sheetData></worksheet>"#
    );
    let book = read(package(
        &declared_good(sheet.into_bytes()),
        &shared,
        &styles,
    ))
    .unwrap();
    let sheet = book.sheet_by_name("Plan1").unwrap();
    assert_eq!(sheet.cell("A1").unwrap().value(), "\u{3042}");
    assert_eq!(sheet.style("A1").font().unwrap().name(), "\u{3042}");
}

#[test]
fn incomplete_characters_in_the_worksheet_are_errors() {
    let templates = [
        // text of an inline string
        format!(
            r#"<worksheet xmlns="{MAIN}"><sheetData><row r="1"><c r="A1" t="inlineStr"><is><t>ab{{BAD}}</t></is></c></row></sheetData></worksheet>"#
        ),
        // text of a formula
        format!(
            r#"<worksheet xmlns="{MAIN}"><sheetData><row r="1"><c r="A1"><f>ab{{BAD}}</f></c></row></sheetData></worksheet>"#
        ),
        // attribute value
        format!(
            r#"<worksheet xmlns="{MAIN}"><sheetData><row r="1"><c r="A1" t="inlineStr" s="{{BAD}}"><is><t>x</t></is></c></row></sheetData></worksheet>"#
        ),
        // sheet view attribute
        format!(
            r#"<worksheet xmlns="{MAIN}"><sheetViews><sheetView workbookViewId="0" view="{{BAD}}"/></sheetViews><sheetData/></worksheet>"#
        ),
    ];
    for (index, template) in templates.iter().enumerate() {
        expect_rejected(
            &format!("worksheet position {index}"),
            package(
                &declared(template),
                &declared_good(good_shared_strings()),
                &declared_good(good_styles()),
            ),
        );
    }
}

#[test]
fn incomplete_characters_in_shared_strings_are_errors() {
    let templates = [
        format!(r#"<sst xmlns="{MAIN}" count="1" uniqueCount="1"><si><t>ab{{BAD}}</t></si></sst>"#),
        format!(
            r#"<sst xmlns="{MAIN}" count="1" uniqueCount="1"><si><r><rPr><rFont val="{{BAD}}"/></rPr><t>x</t></r></si></sst>"#
        ),
        format!(
            r#"<sst xmlns="{MAIN}" count="1" uniqueCount="1"><si><r><t>ab{{BAD}}</t></r></si></sst>"#
        ),
        format!(
            r#"<sst xmlns="{MAIN}" count="1" uniqueCount="1"><si><t>x</t><phoneticPr fontId="{{BAD}}"/></si></sst>"#
        ),
        format!(
            r#"<sst xmlns="{MAIN}" count="1" uniqueCount="1"><si><t>x</t><rPh sb="0" eb="1"><t>{{BAD}}</t></rPh></si></sst>"#
        ),
    ];
    for (index, template) in templates.iter().enumerate() {
        expect_rejected(
            &format!("shared strings position {index}"),
            package(
                &declared_good(good_sheet()),
                &declared(template),
                &declared_good(good_styles()),
            ),
        );
    }
}

#[test]
fn incomplete_characters_in_styles_are_errors() {
    let fonts = |font: &str| {
        format!(
            r#"<styleSheet xmlns="{MAIN}"><fonts count="1">{font}</fonts><fills count="1"><fill><patternFill patternType="none"/></fill></fills><borders count="1"><border><left/><right/><top/><bottom/><diagonal/></border></borders><cellXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellXfs></styleSheet>"#
        )
    };
    let templates = [
        fonts(r#"<font><sz val="11"/><name val="{BAD}"/></font>"#),
        fonts(r#"<font><sz val="{BAD}"/><name val="Calibri"/></font>"#),
        fonts(r#"<font><color rgb="{BAD}"/><name val="Calibri"/></font>"#),
        format!(
            r#"<styleSheet xmlns="{MAIN}"><numFmts count="1"><numFmt numFmtId="164" formatCode="{{BAD}}"/></numFmts></styleSheet>"#
        ),
        format!(
            r#"<styleSheet xmlns="{MAIN}"><cellXfs count="1"><xf numFmtId="{{BAD}}" fontId="0" fillId="0" borderId="0"/></cellXfs></styleSheet>"#
        ),
        format!(
            r#"<styleSheet xmlns="{MAIN}"><dxfs count="1"><dxf><numFmt numFmtId="164" formatCode="ab{{BAD}}"/></dxf></dxfs></styleSheet>"#
        ),
    ];
    for (index, template) in templates.iter().enumerate() {
        expect_rejected(
            &format!("styles position {index}"),
            package(
                &declared_good(good_sheet()),
                &declared_good(good_shared_strings()),
                &declared(template),
            ),
        );
    }
}

#[test]
fn invalid_utf8_in_undeclared_parts_is_an_error() {
    let bad = |template: &str| {
        let (before, after) = template.split_once("{BAD}").expect("marker");
        let mut bytes = before.as_bytes().to_vec();
        bytes.push(0xFF);
        bytes.extend_from_slice(after.as_bytes());
        bytes
    };
    let sheet = bad(&format!(
        r#"<worksheet xmlns="{MAIN}"><sheetData><row r="1"><c r="A1" t="inlineStr"><is><t>ab{{BAD}}</t></is></c></row></sheetData></worksheet>"#
    ));
    expect_rejected(
        "worksheet",
        package(&sheet, &good_shared_strings(), &good_styles()),
    );
    let shared = bad(&format!(
        r#"<sst xmlns="{MAIN}" count="1" uniqueCount="1"><si><t>ab{{BAD}}</t></si></sst>"#
    ));
    expect_rejected(
        "shared strings",
        package(&good_sheet(), &shared, &good_styles()),
    );
    let styles = bad(&format!(
        r#"<styleSheet xmlns="{MAIN}"><fonts count="1"><font><name val="{{BAD}}"/></font></fonts></styleSheet>"#
    ));
    expect_rejected(
        "styles",
        package(&good_sheet(), &good_shared_strings(), &styles),
    );
}

// ---- where the declaration is looked for ----------------------------------

fn first_cell(book: &mut Workbook) -> String {
    book.try_read_sheet_collection().unwrap();
    book.sheet_by_name("Plan1")
        .unwrap()
        .cell("A1")
        .unwrap()
        .value()
        .to_string()
}

fn inline_text_sheet(prefix: &[u8], text: &[u8]) -> Vec<u8> {
    let mut sheet = prefix.to_vec();
    sheet.extend_from_slice(
        format!(
            r#"<worksheet xmlns="{MAIN}"><sheetData><row r="1"><c r="A1" t="inlineStr"><is><t>"#
        )
        .as_bytes(),
    );
    sheet.extend_from_slice(text);
    sheet.extend_from_slice(b"</t></is></c></row></sheetData></worksheet>");
    sheet
}

#[test]
fn a_comment_that_looks_like_a_declaration_is_not_one() {
    let sheet = inline_text_sheet(
        br#"<!-- <?xml version="1.0" encoding="made-up"?> -->"#,
        "caf\u{e9}".as_bytes(),
    );
    let bytes = package(&sheet, &good_shared_strings(), &good_styles());
    for (label, read) in fixture::readers() {
        let mut book = read(&bytes).unwrap_or_else(|error| panic!("{label}: {error}"));
        assert_eq!(first_cell(&mut book), "caf\u{e9}", "{label}");
    }
}

#[test]
fn a_declaration_after_a_byte_order_mark_is_still_read() {
    let mut prefix = vec![0xEF, 0xBB, 0xBF];
    prefix.extend_from_slice(br#"<?xml version="1.0" encoding="ISO-8859-1"?>"#);
    let sheet = inline_text_sheet(&prefix, b"caf\xE9");
    let bytes = package(&sheet, &good_shared_strings(), &good_styles());
    for (label, read) in fixture::readers() {
        let mut book = read(&bytes).unwrap_or_else(|error| panic!("{label}: {error}"));
        assert_eq!(first_cell(&mut book), "caf\u{e9}", "{label}");
    }
}

#[test]
fn a_long_declaration_is_read_whatever_the_buffering() {
    // 9000 spaces separate the XML name from its attributes: far past any
    // buffer a source hands out at once.
    let mut prefix = b"<?xml".to_vec();
    prefix.extend(std::iter::repeat_n(b' ', 9000));
    prefix.extend_from_slice(br#" version="1.0" encoding="ISO-8859-1"?>"#);
    let sheet = inline_text_sheet(&prefix, b"caf\xE9");
    let bytes = package(&sheet, &good_shared_strings(), &good_styles());
    for (label, read) in fixture::readers() {
        let mut book = read(&bytes).unwrap_or_else(|error| panic!("{label}: {error}"));
        assert_eq!(first_cell(&mut book), "caf\u{e9}", "{label}");
    }
}

// ---- UTF-16 parts and declaration grammar ---------------------------------
// Excel (measured, evidence/utf16-and-declarations): a worksheet, sharedStrings
// or styles part in UTF-16LE or UTF-16BE opens with a byte order mark and no
// declaration, with a byte order mark and encoding="UTF-16", and without a
// byte order mark when it declares encoding="UTF-16"; it is refused when the
// declaration says UTF-8. It opens declarations with single quotes, tabs or
// line breaks as separators, no encoding and no standalone; it refuses a
// missing version, standalone="maybe", a form feed as separator, version 2.0,
// encoding before version, an unquoted value and unknown attributes.

const CAFE: &str = "caf\u{e9}";

fn utf16(text: &str, big_endian: bool, byte_order_mark: bool) -> Vec<u8> {
    let mut bytes = Vec::new();
    if byte_order_mark {
        bytes.extend_from_slice(if big_endian {
            &[0xFE, 0xFF]
        } else {
            &[0xFF, 0xFE]
        });
    }
    for unit in text.encode_utf16() {
        bytes.extend_from_slice(&if big_endian {
            unit.to_be_bytes()
        } else {
            unit.to_le_bytes()
        });
    }
    bytes
}

fn shared_text_sheet() -> String {
    format!(
        r#"<worksheet xmlns="{MAIN}"><sheetData><row r="1"><c r="A1" s="0" t="s"><v>0</v></c></row></sheetData></worksheet>"#
    )
}

fn cafe_shared_strings() -> String {
    format!(r#"<sst xmlns="{MAIN}" count="1" uniqueCount="1"><si><t>{CAFE}</t></si></sst>"#)
}

fn cafe_styles() -> String {
    format!(
        r#"<styleSheet xmlns="{MAIN}"><fonts count="1"><font><sz val="11"/><name val="{CAFE}"/></font></fonts><fills count="1"><fill><patternFill patternType="none"/></fill></fills><borders count="1"><border><left/><right/><top/><bottom/><diagonal/></border></borders><cellXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellXfs></styleSheet>"#
    )
}

/// The package with `which` (sheet, strings, styles) replaced by `part`.
fn package_with(which: &str, part: Vec<u8>) -> Vec<u8> {
    let sheet = shared_text_sheet().into_bytes();
    let strings = cafe_shared_strings().into_bytes();
    let styles = cafe_styles().into_bytes();
    match which {
        "sheet" => package(&part, &strings, &styles),
        "strings" => package(&sheet, &part, &styles),
        _ => package(&sheet, &strings, &part),
    }
}

fn text_of(book: &mut Workbook) -> (String, String) {
    book.try_read_sheet_collection().unwrap();
    let sheet = book.sheet_by_name("Plan1").unwrap();
    let font = sheet
        .style("A1")
        .font()
        .map(|font| font.name().to_string())
        .unwrap_or_default();
    (sheet.cell("A1").unwrap().value().to_string(), font)
}

#[test]
fn utf16_parts_are_read_in_both_byte_orders() {
    let documents = [
        ("sheet", shared_text_sheet()),
        ("strings", cafe_shared_strings()),
        ("styles", cafe_styles()),
    ];
    for (which, body) in documents {
        for big_endian in [false, true] {
            // (byte order mark, declaration)
            for (mark, declaration) in [
                (true, ""),
                (true, r#"<?xml version="1.0" encoding="UTF-16"?>"#),
                (false, r#"<?xml version="1.0" encoding="UTF-16"?>"#),
            ] {
                let bytes = package_with(
                    which,
                    utf16(&format!("{declaration}{body}"), big_endian, mark),
                );
                for (label, read) in fixture::readers() {
                    let mut book = read(&bytes).unwrap_or_else(|error| {
                        panic!(
                            "{label} {which} big={big_endian} mark={mark} {declaration}: {error}"
                        )
                    });
                    let (value, font) = text_of(&mut book);
                    match which {
                        "sheet" | "strings" => assert_eq!(value, CAFE, "{label} {which}"),
                        _ => assert_eq!(font, CAFE, "{label} {which}"),
                    }
                }
            }
        }
    }
}

#[test]
fn utf16_parts_that_declare_utf8_or_hold_malformed_text_are_errors() {
    let body = shared_text_sheet();
    let mut cases: Vec<(String, Vec<u8>)> = Vec::new();
    for big_endian in [false, true] {
        for mark in [true, false] {
            cases.push((
                format!("declares UTF-8 big={big_endian} mark={mark}"),
                utf16(
                    &format!(r#"<?xml version="1.0" encoding="UTF-8"?>{body}"#),
                    big_endian,
                    mark,
                ),
            ));
        }
        // A high surrogate without its low surrogate, and a cut unit.
        let mut lone = utf16(
            r#"<?xml version="1.0" encoding="UTF-16"?><a>"#,
            big_endian,
            true,
        );
        lone.extend_from_slice(&if big_endian {
            [0xD8, 0x00]
        } else {
            [0x00, 0xD8]
        });
        lone.extend_from_slice(&utf16("</a>", big_endian, false));
        cases.push((format!("lone surrogate big={big_endian}"), lone));
        let mut cut = utf16(&body, big_endian, true);
        cut.push(b'x');
        cases.push((format!("cut unit big={big_endian}"), cut));
    }
    for which in ["sheet", "strings", "styles"] {
        for (name, part) in &cases {
            let bytes = package_with(which, part.clone());
            for (label, read) in fixture::readers() {
                let result = read(&bytes).and_then(|mut book| {
                    book.try_read_sheet_collection()?;
                    Ok(book)
                });
                assert!(result.is_err(), "{label} {which} {name}: must fail");
            }
        }
    }
}

#[test]
fn declarations_follow_the_grammar_excel_accepts() {
    let accepted = [
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
        "<?xml version='1.0' encoding='UTF-8' standalone='no'?>",
        r#"<?xml version="1.0"?>"#,
        r#"<?xml version="1.0" encoding="UTF-8"?>"#,
        r#"<?xml version="1.0" standalone="yes"?>"#,
        "<?xml version=\"1.0\"\r\n encoding=\"UTF-8\"?>",
        "<?xml version=\"1.0\"\tencoding=\"UTF-8\"?>",
        r#"<?xml version = "1.0" encoding = "UTF-8" ?>"#,
    ];
    for declaration in accepted {
        let sheet = inline_text_sheet(declaration.as_bytes(), CAFE.as_bytes());
        let bytes = package(&sheet, &good_shared_strings(), &good_styles());
        for (label, read) in fixture::readers() {
            let mut book =
                read(&bytes).unwrap_or_else(|error| panic!("{label} {declaration:?}: {error}"));
            assert_eq!(first_cell(&mut book), CAFE, "{label} {declaration:?}");
        }
    }
}

#[test]
fn malformed_declarations_are_errors() {
    let refused = [
        "<?xml encoding='UTF-8'?>",
        "<?xml version='1.0' standalone='maybe'?>",
        "<?xml version=\"1.0\"\x0Cencoding=\"UTF-8\"?>",
        r#"<?xml version="2.0" encoding="UTF-8"?>"#,
        r#"<?xml encoding="UTF-8" version="1.0"?>"#,
        "<?xml version=1.0?>",
        r#"<?xml version="1.0" junk="x"?>"#,
        r#"<?xml version="1.0" encoding="UTF-8" encoding="UTF-8"?>"#,
        r#"<?xml version="1.0" standalone="yes" encoding="UTF-8"?>"#,
        r#"<?xml version="1.0""#,
    ];
    for declaration in refused {
        let sheet = inline_text_sheet(declaration.as_bytes(), b"x");
        let bytes = package(&sheet, &good_shared_strings(), &good_styles());
        for (label, read) in fixture::readers() {
            let result = read(&bytes).and_then(|mut book| {
                book.try_read_sheet_collection()?;
                Ok(book)
            });
            assert!(result.is_err(), "{label} {declaration:?}: must fail");
        }
    }
}

#[test]
fn declarations_of_any_length_read_the_same_in_every_mode() {
    // Around the size the declaration reader used to give up at, and past the
    // buffer a file-backed source hands out.
    for spaces in [8_000usize, 1_048_576 - 16, 1_048_576 + 16 * 1024] {
        let mut prefix = b"<?xml".to_vec();
        prefix.extend(std::iter::repeat_n(b' ', spaces));
        prefix.extend_from_slice(br#"version="1.0" encoding="ISO-8859-1"?>"#);
        let sheet = inline_text_sheet(&prefix, b"caf\xE9");
        let bytes = package(&sheet, &good_shared_strings(), &good_styles());
        for (label, read) in fixture::readers() {
            let mut book =
                read(&bytes).unwrap_or_else(|error| panic!("{label} {spaces} spaces: {error}"));
            assert_eq!(first_cell(&mut book), CAFE, "{label} {spaces} spaces");
        }
    }
}
