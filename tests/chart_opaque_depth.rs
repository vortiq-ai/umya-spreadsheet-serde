//! Deep opaque extension lifecycle through ordinary XLSX and Chart codecs.
#![cfg(feature = "serde")]
use std::io::{
    Cursor,
    Read,
    Write,
};

use umya_spreadsheet::{
    Chart,
    ChartType,
    Workbook,
    drawing::{
        charts::{
            ExtensionNode,
            OpaqueElement,
            OpaqueToken,
        },
        spreadsheet::MarkerType,
    },
};

fn save(book: &Workbook) -> Vec<u8> {
    let mut bytes = Vec::new();
    umya_spreadsheet::writer::xlsx::write_writer(book, &mut Cursor::new(&mut bytes)).unwrap();
    bytes
}
fn load(bytes: &[u8]) -> Workbook {
    umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap()
}
fn chart(book: &Workbook) -> &Chart {
    book.sheet_collection()
        .iter()
        .flat_map(|s| s.chart_collection())
        .next()
        .unwrap()
}
fn chart_mut(book: &mut Workbook) -> &mut Chart {
    book.sheet_collection_mut()
        .iter_mut()
        .flat_map(|s| s.chart_collection_mut())
        .next()
        .unwrap()
}
fn xml(bytes: &[u8]) -> String {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut result = String::new();
    archive
        .by_name("xl/charts/chart1.xml")
        .unwrap()
        .read_to_string(&mut result)
        .unwrap();
    result
}
fn replace(bytes: &[u8], chart_xml: &str) -> Vec<u8> {
    let mut input = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for index in 0..input.len() {
        let mut entry = input.by_index(index).unwrap();
        let name = entry.name().to_owned();
        let mut data = Vec::new();
        entry.read_to_end(&mut data).unwrap();
        if name == "xl/charts/chart1.xml" {
            data = chart_xml.as_bytes().to_vec();
        }
        output
            .start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        output.write_all(&data).unwrap();
    }
    output.finish().unwrap().into_inner()
}
fn base() -> Vec<u8> {
    let mut book = umya_spreadsheet::new_file();
    let mut from = MarkerType::default();
    from.set_coordinate("E1");
    let mut to = MarkerType::default();
    to.set_coordinate("M18");
    let mut c = Chart::default();
    c.new_chart(&ChartType::LineChart, from, to, vec!["Sheet1!$B$1:$B$3"]);
    book.sheet_mut(0).unwrap().add_chart(c);
    save(&book)
}
fn first_opaque(book: &Workbook) -> &OpaqueElement {
    let series = &chart(book)
        .chart_space()
        .chart()
        .plot_area()
        .chart_groups()
        .next()
        .unwrap()
        .series()
        .area_chart_series()[0];
    let extension = &series.extensions().unwrap().entries[0];
    match &extension.nodes[0] {
        ExtensionNode::Unknown(value) => value,
        _ => panic!("expected unknown opaque payload"),
    }
}

#[test]
fn deep_opaque_tree_import_clone_json_binary_export_reread_and_drop() {
    const DEPTH: usize = 4096;
    let input = base();
    let original = xml(&input);
    let mut opaque = String::from("<c:extLst><c:ext uri=\"urn:deep-test\"><z:root xmlns:z=\"urn:deep\" xmlns:q=\"urn:attr\" xmlns:c15=\"http://schemas.microsoft.com/office/drawing/2012/chart\" q:kind=\"q:Type\">");
    for _ in 0..DEPTH {
        opaque.push_str("<z:n>");
    }
    opaque.push_str(
        "before<![CDATA[<raw>]]><!--note--><?test \
         value?><c15:fullRef><c15:sqref>Sheet1!$A$1</c15:sqref></c15:fullRef>after",
    );
    for _ in 0..DEPTH {
        opaque.push_str("</z:n>");
    }
    opaque.push_str("</z:root></c:ext></c:extLst>");
    let modified = original.replacen("</c:ser>", &format!("{opaque}</c:ser>"), 1);
    assert_ne!(modified, original);
    let input = replace(&input, &modified);
    let book = load(&input);
    let root = first_opaque(&book);
    assert_eq!(
        root.children
            .iter()
            .filter(|v| matches!(v, OpaqueToken::Start(_)))
            .count(),
        DEPTH
    );
    assert_eq!(
        root.children
            .iter()
            .filter(|v| matches!(v, OpaqueToken::End))
            .count(),
        DEPTH
    );
    assert_eq!(
        root.children
            .iter()
            .filter(|v| matches!(v, OpaqueToken::Reference(_)))
            .count(),
        1
    );
    let initial = chart(&book).clone();
    let json: Chart = serde_json::from_slice(&serde_json::to_vec(&initial).unwrap()).unwrap();
    let binary: Chart = bincode::deserialize(&bincode::serialize(&initial).unwrap()).unwrap();
    for (route, mut value) in [("original", initial), ("json", json), ("bincode", binary)] {
        let mut adjusted = 0;
        for formula in value.chart_space_mut().chart_mut().formula_mut() {
            if formula.address_str() == "Sheet1!$A$1" {
                formula.set_address_str("Sheet1!$A$2");
                adjusted += 1;
            }
        }
        assert_eq!(adjusted, 1, "typed terminal reference remains reachable");
        let mut output_book = load(&input);
        *chart_mut(&mut output_book) = value;
        let output = save(&output_book);
        if let Some(dir) = std::env::var_os("UMYA_OPAQUE_DEPTH_EXPORT_DIR") {
            let dir = std::path::PathBuf::from(dir);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join(format!("deep-{route}.xlsx")), &output).unwrap();
        }
        let document = xml(&output);
        assert_eq!(document.matches("<z:n>").count(), DEPTH);
        assert!(document.contains("q:kind=\"q:Type\""));
        assert!(document.contains("Sheet1!$A$2"));
        assert!(document.contains("<![CDATA[<raw>]]>"));
        assert!(document.contains("<!--note-->"));
        let reread = load(&output);
        assert_eq!(
            first_opaque(&reread)
                .children
                .iter()
                .filter(|v| matches!(v, OpaqueToken::Start(_)))
                .count(),
            DEPTH
        );
        drop(reread);
        drop(output_book);
    }
    drop(book);
}
