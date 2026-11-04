#![cfg(feature = "serde")]

use std::{
    collections::BTreeMap,
    io::{
        Cursor,
        Read,
    },
    path::PathBuf,
};

use quick_xml::{
    NsReader,
    events::Event,
    name::ResolveResult,
};
use umya_spreadsheet::{
    DefinedName,
    Workbook,
    drawing::{
        charts::{
            ChartExDimension,
            ChartExDimensionSource,
            ChartExFormula,
            ChartReferenceEdit,
        },
        spreadsheet::ExtendedChart,
    },
    reader,
    writer,
};

const NAMED_SOURCE: &[u8] =
    include_bytes!("fixtures/extended_charts/sheet-deletion/named-source.xlsx");
const DIRECT_SOURCE: &[u8] =
    include_bytes!("fixtures/extended_charts/sheet-deletion/direct-source.xlsx");
const EXCEL_AFTER: &str =
    include_str!("fixtures/extended_charts/sheet-deletion/excel-after-delete-chart-ex.xml");

#[derive(Debug, PartialEq, Eq)]
enum XmlToken {
    Start((String, String), BTreeMap<(String, String), String>),
    End,
    Text(String),
}

fn namespace(value: ResolveResult<'_>) -> String {
    match value {
        ResolveResult::Bound(value) => String::from_utf8(value.as_ref().to_vec()).unwrap(),
        ResolveResult::Unbound => String::new(),
        ResolveResult::Unknown(prefix) => panic!("undeclared XML prefix {prefix:?}"),
    }
}

fn append_text(tokens: &mut Vec<XmlToken>, text: &str) {
    if let Some(XmlToken::Text(previous)) = tokens.last_mut() {
        previous.push_str(text);
    } else {
        tokens.push(XmlToken::Text(text.to_owned()));
    }
}

/// The tokens of a part with namespaces resolved. Line breaks inside the
/// base64 `binary` text are not significant (Excel wraps it, the library does
/// not).
fn canonical_xml(xml: &[u8]) -> Vec<XmlToken> {
    let mut reader = NsReader::from_reader(xml);
    let mut tokens = Vec::new();
    let mut open = Vec::new();
    loop {
        let event = reader.read_event().unwrap();
        let empty = matches!(&event, Event::Empty(_));
        match event {
            Event::Start(element) | Event::Empty(element) => {
                let (uri, local) = reader.resolver().resolve_element(element.name());
                let name = (
                    namespace(uri),
                    String::from_utf8(local.as_ref().to_vec()).unwrap(),
                );
                let mut attributes = BTreeMap::new();
                for attribute in element.attributes() {
                    let attribute = attribute.unwrap();
                    if attribute.key.as_ref() == b"xmlns"
                        || attribute
                            .key
                            .prefix()
                            .is_some_and(|prefix| prefix.as_ref() == b"xmlns")
                    {
                        continue;
                    }
                    let (uri, local) = reader.resolver().resolve_attribute(attribute.key);
                    let key = (
                        namespace(uri),
                        String::from_utf8(local.as_ref().to_vec()).unwrap(),
                    );
                    let value = attribute
                        .decoded_and_normalized_value(
                            quick_xml::XmlVersion::Implicit1_0,
                            reader.decoder(),
                        )
                        .unwrap()
                        .into_owned();
                    assert!(attributes.insert(key, value).is_none());
                }
                if empty {
                    tokens.push(XmlToken::Start(name, attributes));
                    tokens.push(XmlToken::End);
                } else {
                    open.push(name.1.clone());
                    tokens.push(XmlToken::Start(name, attributes));
                }
            }
            Event::End(_) => {
                open.pop();
                tokens.push(XmlToken::End);
            }
            Event::Text(text) => {
                let text = text
                    .xml_content(quick_xml::XmlVersion::Implicit1_0)
                    .unwrap();
                if open.last().is_some_and(|name| name == "binary") {
                    append_text(&mut tokens, &text.replace(['\n', '\r'], ""));
                } else {
                    append_text(&mut tokens, &text);
                }
            }
            Event::CData(text) => append_text(
                &mut tokens,
                &text
                    .xml_content(quick_xml::XmlVersion::Implicit1_0)
                    .unwrap(),
            ),
            Event::GeneralRef(reference) => {
                let entity = format!("&{};", reference.decode().unwrap());
                append_text(&mut tokens, &quick_xml::escape::unescape(&entity).unwrap());
            }
            Event::Decl(_) | Event::Comment(_) | Event::PI(_) => (),
            Event::Eof => break,
            event => panic!("unexpected chart XML event {event:?}"),
        }
    }
    tokens
}

fn part(bytes: &[u8], path: &str) -> Vec<u8> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut content = Vec::new();
    archive
        .by_name(path)
        .unwrap()
        .read_to_end(&mut content)
        .unwrap();
    content
}

fn output_directory() -> PathBuf {
    PathBuf::from(std::env::var_os("CARGO_TARGET_DIR").unwrap_or_else(|| "target".into()))
        .join("chart-ex-sheet-deletion")
}

fn emit(name: &str, bytes: &[u8]) {
    let directory = output_directory();
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join(format!("{name}.xlsx")), bytes).unwrap();
}

fn stream_write(book: Workbook) -> Vec<u8> {
    let sheet_names: Vec<_> = book
        .sheet_collection()
        .iter()
        .map(|sheet| sheet.name().to_owned())
        .collect();
    let zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let mut streaming = writer::streaming_writer::StreamingWriter::new(zip, book);
    for name in sheet_names {
        let sheet = streaming.take_sheet(&name).unwrap();
        streaming.flush_sheet(sheet).unwrap();
    }
    streaming.finish().unwrap().into_inner()
}

fn write_bytes(book: &Workbook) -> [(&'static str, Vec<u8>); 2] {
    let mut normal = Vec::new();
    writer::xlsx::write_writer(book, &mut normal).unwrap();
    [
        ("normal", normal),
        ("streaming", stream_write(book.clone())),
    ]
}

fn write_both(book: &Workbook, stem: &str) -> [(String, Vec<u8>); 2] {
    write_bytes(book).map(|(writer_name, bytes)| {
        let name = format!("{stem}-{writer_name}");
        emit(&name, &bytes);
        (name, bytes)
    })
}

fn add_ordinary_name(book: &mut Workbook) {
    let mut name = DefinedName::default();
    name.set_name("OrdinaryRange")
        .set_address("Sheet1!$A$2:$A$5");
    book.add_defined_names(name);
}

fn all_defined_names(book: &Workbook) -> Vec<&DefinedName> {
    book.defined_names()
        .iter()
        .chain(
            book.sheet_collection_no_check()
                .iter()
                .flat_map(|sheet| sheet.defined_names()),
        )
        .collect()
}

/// `_xlchart` definitions of a package, read straight from its workbook.xml.
fn chart_names(bytes: &[u8]) -> BTreeMap<String, String> {
    let workbook = String::from_utf8(part(bytes, "xl/workbook.xml")).unwrap();
    let mut names = BTreeMap::new();
    for definition in workbook.split("<definedName ").skip(1) {
        let name = definition.split("name=\"").nth(1).unwrap();
        let name = &name[..name.find('"').unwrap()];
        if name.starts_with("_xlchart.") {
            let text = &definition[definition.find('>').unwrap() + 1..];
            names.insert(name.to_owned(), text[..text.find('<').unwrap()].to_owned());
        }
    }
    names
}

/// Excel removes the hidden names of the deleted sheet and leaves the others.
fn surviving_chart_names(input: &[u8]) -> BTreeMap<String, String> {
    chart_names(input)
        .into_iter()
        .filter(|(_, address)| !address.starts_with("Sheet1!"))
        .collect()
}

fn assert_deleted_name_table(book: &Workbook, expect_ordinary_name_ref: bool) {
    let names = all_defined_names(book);
    assert!(
        names
            .iter()
            .all(|name| !name.address().starts_with("Sheet1!")),
        "names of the deleted sheet remain: {names:?}"
    );
    if expect_ordinary_name_ref {
        assert!(
            names
                .iter()
                .any(|name| { name.name() == "OrdinaryRange" && name.address() == "#REF!" }),
            "ordinary defined name did not retain its existing deletion behavior: {names:?}"
        );
    }
}

fn assert_chart_part(bytes: &[u8], case: &Case, label: &str) {
    let mut written = canonical_xml(&part(bytes, "xl/charts/chartEx1.xml"));
    if case.export_caches_range {
        written = without_levels_beside_formulas(written);
    }
    assert_eq!(
        written,
        canonical_xml(case.after.as_bytes()),
        "{label}: chartEx structure differs from Excel's saved after file"
    );
}

fn without_levels_beside_formulas(tokens: Vec<XmlToken>) -> Vec<XmlToken> {
    let mut kept = Vec::new();
    // Whether each open element has a formula child, and the depth skipped.
    let mut has_formula = Vec::new();
    let mut skipped = 0;
    for token in tokens {
        match &token {
            XmlToken::Start((_, local), _) => {
                if skipped > 0 {
                    skipped += 1;
                    continue;
                }
                if local == "lvl" && has_formula.last() == Some(&true) {
                    skipped = 1;
                    continue;
                }
                if local == "f" {
                    if let Some(parent) = has_formula.last_mut() {
                        *parent = true;
                    }
                }
                has_formula.push(false);
            }
            XmlToken::End => {
                if skipped > 0 {
                    skipped -= 1;
                    continue;
                }
                has_formula.pop();
            }
            XmlToken::Text(_) if skipped > 0 => continue,
            XmlToken::Text(_) => (),
        }
        kept.push(token);
    }
    kept
}

/// The written package equals Excel's result and survives a reread: the
/// reread workbook, written again by both writers, gives Excel's result too.
fn assert_excel_after(case: &Case, bytes: &[u8], label: &str) {
    let input = case.input;
    assert_chart_part(bytes, case, label);
    assert_eq!(
        chart_names(bytes),
        surviving_chart_names(input),
        "{label}: _xlchart names of the written workbook"
    );
    let reread = reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap();
    assert_eq!(reread.sheet_collection().len(), 1, "{label}");
    assert_eq!(reread.sheet(0).unwrap().name(), "ChartHost", "{label}");
    assert_eq!(
        reread
            .sheet(0)
            .unwrap()
            .worksheet_drawing()
            .extended_charts()
            .len(),
        1,
        "{label}: host chart disappeared"
    );
    assert_deleted_name_table(&reread, true);
    assert!(
        (reread.workbook_view().active_tab() as usize) < reread.sheet_collection().len(),
        "{label}: active tab is past the last sheet"
    );
    for (writer_name, again) in write_bytes(&reread) {
        assert_chart_part(&again, case, &format!("{label} reread {writer_name}"));
        assert_eq!(
            chart_names(&again),
            surviving_chart_names(input),
            "{label} reread {writer_name}: _xlchart names"
        );
    }
}

fn delete_sheet_edit() -> ChartReferenceEdit {
    ChartReferenceEdit::DeleteSheet {
        sheet:       "Sheet1".into(),
        sheet_order: vec!["ChartHost".into(), "Sheet1".into()],
    }
}

fn read(input: &[u8], eager: bool) -> Workbook {
    let mut book = reader::xlsx::read_reader(Cursor::new(input), eager).unwrap();
    add_ordinary_name(&mut book);
    book
}

fn put_name_formula(book: &mut Workbook, name: &str) {
    let chart = host_chart_mut(book);
    let data = chart.chart_space_mut().data_collection.as_mut().unwrap();
    let ChartExDimension::String(dimension) = &mut data.data[0].dimensions[0] else {
        panic!("first dimension is not a string dimension");
    };
    let ChartExDimensionSource::Reference(reference) = &mut dimension.source else {
        panic!("first dimension is not a reference");
    };
    reference.set_name_formula(Some(ChartExFormula::new(name)));
}

fn read_case(case: &Case, eager: bool) -> Workbook {
    let mut book = read(case.input, eager);
    if let Some(name) = case.name_formula {
        put_name_formula(&mut book, name);
    }
    book
}

fn host_chart_mut(book: &mut Workbook) -> &mut ExtendedChart {
    let host = book
        .sheet_collection_no_check()
        .iter()
        .position(|sheet| sheet.name() == "ChartHost")
        .unwrap();
    &mut book
        .read_sheet(host)
        .sheet_mut(host)
        .unwrap()
        .worksheet_drawing_mut()
        .extended_charts_mut()[0]
}

struct Case {
    name:                &'static str,
    input:               &'static [u8],
    after:               &'static str,
    /// A dimension formula survives on the host sheet. The library exports
    /// that range's cached levels; Excel's COM output carries none.
    export_caches_range: bool,
    /// Put this name formula back on the first (category) dimension before the
    /// deletion: Excel drops `cx:nf` of this kind on load, so no fixture has
    /// one.
    name_formula:        Option<&'static str>,
}

macro_rules! case {
    ($name:literal) => {
        Case {
            name:                $name,
            input:               include_bytes!(concat!(
                "fixtures/extended_charts/sheet-deletion/kinds/",
                $name,
                ".xlsx"
            )),
            after:               include_str!(concat!(
                "fixtures/extended_charts/sheet-deletion/kinds/",
                $name,
                "-after.xml"
            )),
            export_caches_range: false,
            name_formula:        None,
        }
    };
}

/// Every chartEx kind Excel can author offline, and every other formula site,
/// each saved by Excel with cached levels, and Excel's result after deleting
/// its source sheet (`kinds/*-after.xml`).
fn cases() -> Vec<Case> {
    vec![
        case!("sunburst"),
        case!("treemap"),
        case!("waterfall"),
        case!("histogram"),
        case!("pareto"),
        case!("boxwhisker"),
        case!("funnel"),
        case!("regionmap"),
        case!("title-formula"),
        case!("title-formula-direct"),
        case!("axis-title-formula"),
        case!("axis-title-formula-direct"),
        case!("units-label-formula"),
        case!("units-label-formula-direct"),
        Case {
            export_caches_range: true,
            // `cx:nf` on the deleted sheet while `cx:f` survives on the host.
            name_formula: Some("_xlchart.v1.1"),
            ..case!("name-formula-deleted")
        },
        // Excel's after file also drops the unused `_xlchart.v1.0` (its input
        // was an Excel resave, so the name had no user on load). The library
        // only removes names of the deleted sheet and leaves this one: the
        // written names are asserted to equal the input's surviving names.
        case!("dimension-deleted-unused-surviving-name"),
    ]
}

fn source_case(name: &'static str, input: &'static [u8]) -> Case {
    Case {
        name,
        input,
        after: EXCEL_AFTER,
        export_caches_range: false,
        name_formula: None,
    }
}

fn workbook_removal_cases() -> Vec<Case> {
    let mut all = cases();
    all.push(source_case("named-source", NAMED_SOURCE));
    all.push(source_case("direct-source", DIRECT_SOURCE));
    all
}

#[test]
fn workbook_sheet_removal_matches_excel_for_every_chart_kind_and_formula_site() {
    for case in workbook_removal_cases() {
        for eager in [true, false] {
            let mut book = read_case(&case, eager);
            book.remove_sheet_by_name("Sheet1").unwrap();
            assert_deleted_name_table(&book, true);
            let stem = format!(
                "workbook-{}-{}",
                case.name,
                if eager { "eager" } else { "lazy" }
            );
            for (writer_name, bytes) in write_both(&book, &stem) {
                assert_excel_after(&case, &bytes, &writer_name);
            }
        }
    }
}

const DIRECT_ADDRESSES: [(&str, &str); 3] = [
    ("_xlchart.v1.0", "Sheet1!$A$2:$B$5"),
    ("_xlchart.v1.1", "Sheet1!$C$1"),
    ("_xlchart.v1.2", "Sheet1!$C$2:$C$5"),
];

fn make_direct(chart: &mut ExtendedChart) {
    chart.chart_space_mut().visit_formulas_mut(&mut |formula| {
        let text = formula.address_str();
        if let Some((_, address)) = DIRECT_ADDRESSES.iter().find(|(name, _)| *name == text) {
            formula.set_string_value(*address);
        }
    });
}

fn chart_formulas(chart: &ExtendedChart) -> Vec<String> {
    let mut formulas = Vec::new();
    chart
        .chart_space()
        .visit_formulas(&mut |formula| formulas.push(formula.address_str()));
    formulas
}

#[test]
fn workbook_sheet_removal_matches_excel_for_direct_operands_set_in_place() {
    for eager in [true, false] {
        let mut book = read(NAMED_SOURCE, eager);
        let chart = host_chart_mut(&mut book);
        make_direct(chart);
        let mut direct = chart_formulas(chart);
        direct.sort();
        assert_eq!(
            direct,
            ["Sheet1!$A$2:$B$5", "Sheet1!$C$1", "Sheet1!$C$2:$C$5"]
        );
        book.remove_sheet_by_name("Sheet1").unwrap();
        assert_deleted_name_table(&book, true);
        assert!(chart_formulas(host_chart_mut(&mut book)).is_empty());
        for (writer_name, bytes) in write_both(
            &book,
            &format!(
                "workbook-direct-in-place-{}",
                if eager { "eager" } else { "lazy" }
            ),
        ) {
            assert_excel_after(
                &source_case("direct-in-place", NAMED_SOURCE),
                &bytes,
                &writer_name,
            );
        }
    }
}

fn remove_chart(book: &mut Workbook) -> ExtendedChart {
    let host = book
        .sheet_collection()
        .iter()
        .position(|sheet| sheet.name() == "ChartHost")
        .unwrap();
    book.sheet_mut(host)
        .unwrap()
        .worksheet_drawing_mut()
        .extended_charts_mut()
        .remove(0)
}

fn add_chart(book: &mut Workbook, chart: ExtendedChart) {
    let host = book
        .sheet_collection()
        .iter()
        .position(|sheet| sheet.name() == "ChartHost")
        .unwrap();
    book.sheet_mut(host)
        .unwrap()
        .worksheet_drawing_mut()
        .add_extended_chart(chart);
}

fn write_detached_result(mut book: Workbook, chart: ExtendedChart, case: &Case, label: &str) {
    book.remove_sheet_by_name("Sheet1").unwrap();
    add_chart(&mut book, chart);
    for (writer_name, bytes) in write_both(&book, label) {
        assert_excel_after(case, &bytes, &writer_name);
    }
}

#[test]
fn detached_direct_adjustment_matches_excel_after() {
    let mut book = read(NAMED_SOURCE, true);
    let mut chart = remove_chart(&mut book);
    make_direct(&mut chart);
    let report = chart
        .adjust_references("ChartHost", &delete_sheet_edit())
        .unwrap();
    assert!(report.defined_names.is_empty());
    assert!(report.unused_defined_names.is_empty());
    assert!(chart_formulas(&chart).is_empty());
    write_detached_result(
        book,
        chart,
        &source_case("direct", NAMED_SOURCE),
        "detached-direct",
    );
}
