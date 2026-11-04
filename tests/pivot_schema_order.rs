use std::io::{
    Cursor,
    Read,
    Write,
};

use umya_spreadsheet::{
    CacheField,
    CacheSource,
    PivotTable,
    Workbook,
    WorksheetSource,
    new_file,
};
use zip::{
    ZipArchive,
    ZipWriter,
    write::SimpleFileOptions,
};

const PIVOT_PART: &str = "xl/pivotTables/pivotTable1.xml";
const PIVOT_XML: &str = r#"<pivotTableDefinition xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" name="Pivot" cacheId="0" dataCaption="Values"><location ref="D1:E3" firstHeaderRow="0" firstDataRow="1" firstDataCol="1"/><pivotFields count="1"><pivotField/></pivotFields><formats count="1"><format dxfId="0"><pivotArea field="0"/></format></formats><conditionalFormats count="1"><conditionalFormat priority="3" scope="field" type="row"><pivotAreas count="1"><pivotArea field="0"/></pivotAreas></conditionalFormat></conditionalFormats><chartFormats count="1"><chartFormat chart="4" format="2" series="0"><pivotArea field="0"/></chartFormat></chartFormats><pivotHierarchies count="1"><pivotHierarchy caption="Team"/></pivotHierarchies><pivotTableStyleInfo name="PivotStyleLight16" showRowHeaders="1" showColHeaders="1" showRowStripes="0" showColStripes="0" showLastColumn="1"/></pivotTableDefinition>"#;

fn save(book: &Workbook) -> Vec<u8> {
    let mut bytes = Vec::new();
    umya_spreadsheet::writer::xlsx::write_writer(book, &mut Cursor::new(&mut bytes)).unwrap();
    bytes
}

fn fixture() -> Vec<u8> {
    let mut book = new_file();
    let mut pivot = PivotTable::default();
    pivot.pivot_table_definition_mut().set_name("Pivot");
    let mut field = CacheField::default();
    field.set_name("Team");
    pivot
        .pivot_cache_definition_mut()
        .cache_fields_mut()
        .list_mut()
        .push(field);
    pivot
        .pivot_cache_definition_mut()
        .set_cache_source(CacheSource::new_worksheet(WorksheetSource::new_simple(
            "Sheet1", "A1:A2",
        )));
    book.sheet_mut(0).unwrap().cell_mut("A1").set_value("Team");
    book.sheet_mut(0).unwrap().cell_mut("A2").set_value("A");
    book.sheet_mut(0).unwrap().add_pivot_table(pivot);
    let mut input = ZipArchive::new(Cursor::new(save(&book))).unwrap();
    let mut output = ZipWriter::new(Cursor::new(Vec::new()));
    for index in 0..input.len() {
        let mut part = input.by_index(index).unwrap();
        let name = part.name().unwrap().into_owned();
        let options = SimpleFileOptions::default().compression_method(part.compression());
        let mut body = Vec::new();
        part.read_to_end(&mut body).unwrap();
        output.start_file(&name, options).unwrap();
        if name == PIVOT_PART {
            output.write_all(PIVOT_XML.as_bytes()).unwrap();
        } else if name == "xl/styles.xml" {
            let styles = String::from_utf8(body).unwrap();
            assert!(styles.contains("<tableStyles "));
            output
                .write_all(
                    styles
                        .replacen(
                            "<tableStyles ",
                            "<dxfs count=\"1\"><dxf/></dxfs><tableStyles ",
                            1,
                        )
                        .as_bytes(),
                )
                .unwrap();
        } else {
            output.write_all(&body).unwrap();
        }
    }
    output.finish().unwrap().into_inner()
}

#[test]
fn pivot_formatting_precedes_hierarchies_and_style_in_exported_xlsx() {
    let mut book =
        umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(fixture()), true).unwrap();
    book.sheet_mut(0).unwrap().pivot_tables_mut()[0]
        .pivot_table_definition_mut()
        .set_name("EditedPivot");
    let bytes = save(&book);
    let mut archive = ZipArchive::new(Cursor::new(&bytes)).unwrap();
    let mut xml = String::new();
    archive
        .by_name(PIVOT_PART)
        .unwrap()
        .read_to_string(&mut xml)
        .unwrap();
    let expected = [
        "formats",
        "conditionalFormats",
        "chartFormats",
        "pivotHierarchies",
        "pivotTableStyleInfo",
    ];
    let positions: Vec<_> = expected
        .iter()
        .map(|name| xml.find(&format!("<{name} ")).unwrap())
        .collect();
    assert!(
        positions.windows(2).all(|pair| pair[0] < pair[1]),
        "expected {expected:?} in schema order: {xml}"
    );
    let reread = umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap();
    let definition = reread.sheet(0).unwrap().pivot_tables()[0].pivot_table_definition();
    assert_eq!(definition.name(), "EditedPivot");
    assert_eq!(definition.formats().unwrap().formats[0].dxf_id, Some(0));
    assert_eq!(
        definition.conditional_formats().unwrap().formats[0].priority,
        3
    );
    let chart_format = &definition.chart_formats().unwrap().formats[0];
    assert_eq!(chart_format.chart, 4);
    assert_eq!(chart_format.format, 2);
    assert_eq!(chart_format.series, Some(false));
    assert_eq!(
        definition.hierarchies().unwrap().hierarchies[0]
            .caption
            .as_deref(),
        Some("Team")
    );
}
