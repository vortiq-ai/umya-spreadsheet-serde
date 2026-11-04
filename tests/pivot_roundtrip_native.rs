use std::{
    collections::BTreeMap,
    io::{
        Cursor,
        Read,
        Write,
    },
};

use umya_spreadsheet::{
    reader,
    writer,
};
use zip::{
    ZipArchive,
    ZipWriter,
    write::SimpleFileOptions,
};

const NS: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
const REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const RECORDS: &str = r#"<pivotCacheRecords xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" count="2"><r><x v="0"/></r><r><x v="1"/></r></pivotCacheRecords>"#;
const CONSOLIDATION_SOURCE: &str = r#"<cacheSource type="consolidation"><consolidation autoPage="0"><pages count="1"><page count="1"><pageItem name="North"/></page></pages><rangeSets count="1"><rangeSet i1="0" ref="A1:A3" sheet="Sheet1"/></rangeSets></consolidation></cacheSource>"#;

fn relationships(entries: &[(&str, &str, &str)]) -> String {
    let body: String = entries
        .iter()
        .map(|(id, kind, target)| {
            format!(r#"<Relationship Id="{id}" Type="{REL}/{kind}" Target="{target}"/>"#)
        })
        .collect();
    format!(
        r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">{body}</Relationships>"#
    )
}

fn fixture(distinct_caches: bool, calculated_items: bool) -> Vec<u8> {
    let mut parts = BTreeMap::new();
    parts.insert(
        "_rels/.rels".to_string(),
        relationships(&[("office", "officeDocument", "xl/workbook.xml")]),
    );
    let second_cache = if distinct_caches {
        r#"<pivotCache cacheId="73" r:id="otherCache"/>"#
    } else {
        ""
    };
    parts.insert("xl/workbook.xml".into(), format!(r#"<workbook xmlns="{NS}" xmlns:r="{REL}"><sheets><sheet name="Sheet1" sheetId="1" r:id="sheet"/></sheets><pivotCaches><pivotCache cacheId="41" r:id="cache"/>{second_cache}</pivotCaches></workbook>"#));
    let mut workbook_rels = vec![
        ("sheet", "worksheet", "worksheets/sheet1.xml"),
        (
            "cache",
            "pivotCacheDefinition",
            "pivotCache/pivotCacheDefinition9.xml",
        ),
    ];
    if distinct_caches {
        workbook_rels.push((
            "otherCache",
            "pivotCacheDefinition",
            "pivotCache/pivotCacheDefinition12.xml",
        ));
    }
    parts.insert(
        "xl/_rels/workbook.xml.rels".into(),
        relationships(&workbook_rels),
    );
    parts.insert("xl/worksheets/sheet1.xml".into(), format!(r#"<worksheet xmlns="{NS}" xmlns:r="{REL}"><sheetData/><pivotTableParts count="2"><pivotTablePart r:id="first"/><pivotTablePart r:id="second"/></pivotTableParts></worksheet>"#));
    parts.insert(
        "xl/worksheets/_rels/sheet1.xml.rels".into(),
        relationships(&[
            ("first", "pivotTable", "../pivotTables/pivotTable8.xml"),
            ("second", "pivotTable", "../pivotTables/pivotTable3.xml"),
        ]),
    );
    for (part, name, cache_id, cache_file) in [
        ("pivotTable8", "QuarterPivot", 41, "pivotCacheDefinition9"),
        (
            "pivotTable3",
            "OtherPivot",
            if distinct_caches { 73 } else { 41 },
            if distinct_caches {
                "pivotCacheDefinition12"
            } else {
                "pivotCacheDefinition9"
            },
        ),
    ] {
        parts.insert(format!("xl/pivotTables/{part}.xml"), format!(r#"<pivotTableDefinition xmlns="{NS}" name="{name}" cacheId="{cache_id}" dataCaption="Values"><location ref="D1:E4" firstHeaderRow="0" firstDataRow="1" firstDataCol="1"/><pivotFields count="1"><pivotField><items count="2"><item x="0"/><item x="1"/></items></pivotField></pivotFields><formats count="1"><format><pivotArea field="0"/></format></formats><conditionalFormats count="1"><conditionalFormat priority="3" scope="field" type="row"><pivotAreas count="1"><pivotArea field="0"/></pivotAreas></conditionalFormat></conditionalFormats><chartFormats count="1"><chartFormat chart="4" format="2" series="0"><pivotArea field="0"/></chartFormat></chartFormats></pivotTableDefinition>"#));
        parts.insert(
            format!("xl/pivotTables/_rels/{part}.xml.rels"),
            relationships(&[(
                "nativeCache",
                "pivotCacheDefinition",
                &format!("../pivotCache/{cache_file}.xml"),
            )]),
        );
    }
    for (file, label, slicer_id) in [
        ("pivotCacheDefinition9", "Quarter", 902),
        ("pivotCacheDefinition12", "Region", 903),
    ]
    .into_iter()
    .take(if distinct_caches { 2 } else { 1 })
    {
        let calculated = if calculated_items {
            r#"<calculatedItems count="1"><calculatedItem field="0" formula="Q1+Q2"><pivotArea field="0"/></calculatedItem></calculatedItems>"#
        } else {
            ""
        };
        parts.insert(format!("xl/pivotCache/{file}.xml"), format!(r#"<pivotCacheDefinition xmlns="{NS}" xmlns:r="{REL}" xmlns:x14="http://schemas.microsoft.com/office/spreadsheetml/2009/9/main" r:id="records" recordCount="2"><cacheSource type="worksheet"><worksheetSource sheet="Sheet1" ref="A1:A3"/></cacheSource><cacheFields count="1"><cacheField name="{label}" formula="Amount*2" databaseField="0"><sharedItems count="2"><s v="Jan"/><s v="Apr"/></sharedItems><fieldGroup base="0"><discretePr count="2"><x v="0"/><x v="1"/></discretePr><groupItems count="2"><s v="Q1"/><s v="Q2"/></groupItems></fieldGroup></cacheField></cacheFields>{calculated}<extLst><ext uri="{{725AE2AE-9491-48be-B2B4-4EB974FC3084}}"><x14:pivotCacheDefinition pivotCacheId="{slicer_id}"/></ext></extLst></pivotCacheDefinition>"#));
        parts.insert(
            format!("xl/pivotCache/_rels/{file}.xml.rels"),
            relationships(&[(
                "records",
                "pivotCacheRecords",
                &format!("{file}Records.xml"),
            )]),
        );
        parts.insert(format!("xl/pivotCache/{file}Records.xml"), RECORDS.into());
    }
    let overrides: String = parts.keys().filter_map(|path| {
        let kind = if path == "xl/workbook.xml" {"sheet.main"} else if path == "xl/worksheets/sheet1.xml" {"worksheet"} else if path.starts_with("xl/pivotTables/") && path.ends_with(".xml") {"pivotTable"} else if path.starts_with("xl/pivotCache/") && path.ends_with("Records.xml") {"pivotCacheRecords"} else if path.starts_with("xl/pivotCache/") && path.ends_with(".xml") {"pivotCacheDefinition"} else { return None; };
        Some(format!(r#"<Override PartName="/{path}" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.{kind}+xml"/>"#))
    }).collect();
    parts.insert("[Content_Types].xml".into(), format!(r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/>{overrides}</Types>"#));
    pack(parts)
}

fn pack(parts: BTreeMap<String, String>) -> Vec<u8> {
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    for (path, xml) in parts {
        zip.start_file(path, SimpleFileOptions::default()).unwrap();
        zip.write_all(xml.as_bytes()).unwrap();
    }
    zip.finish().unwrap().into_inner()
}

fn save(book: &umya_spreadsheet::Workbook) -> Vec<u8> {
    let mut bytes = Vec::new();
    writer::xlsx::write_writer(book, &mut bytes).unwrap();
    bytes
}

fn parts(bytes: &[u8]) -> BTreeMap<String, String> {
    let mut zip = ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut parts = BTreeMap::new();
    for index in 0..zip.len() {
        let mut part = zip.by_index(index).unwrap();
        let name = part.name().unwrap().into_owned();
        let mut xml = String::new();
        part.read_to_string(&mut xml).unwrap();
        assert!(parts.insert(name, xml).is_none(), "duplicate ZIP part");
    }
    parts
}

#[test]
fn shared_native_cache_relationships_records_and_ids_survive_two_saves() {
    for eager in [true, false] {
        let mut book =
            reader::xlsx::read_reader(Cursor::new(fixture(false, false)), eager).unwrap();
        book.sheet_mut(0)
            .unwrap()
            .cell_mut("Z1")
            .set_value("Unrelated edit");
        for _ in 0..2 {
            let bytes = save(&book);
            let output = parts(&bytes);
            let caches: Vec<_> = output
                .keys()
                .filter(|path| {
                    path.starts_with("xl/pivotCache/")
                        && path.ends_with(".xml")
                        && !path.ends_with("Records1.xml")
                })
                .collect();
            assert_eq!(caches, vec!["xl/pivotCache/pivotCacheDefinition1.xml"]);
            assert!(output["xl/workbook.xml"].contains("cacheId=\"41\""));
            assert!(
                output["xl/_rels/workbook.xml.rels"]
                    .contains("Target=\"pivotCache/pivotCacheDefinition1.xml\"")
            );
            for table in [1, 2] {
                let rels = format!("xl/pivotTables/_rels/pivotTable{table}.xml.rels");
                assert!(
                    output.contains_key(&rels),
                    "missing native cache relationship: {rels}"
                );
                assert!(
                    output[&rels].contains("Target=\"../pivotCache/pivotCacheDefinition1.xml\"")
                );
                assert!(
                    output[&format!("xl/pivotTables/pivotTable{table}.xml")]
                        .contains("cacheId=\"41\"")
                );
            }
            assert_eq!(output["xl/pivotCache/pivotCacheRecords1.xml"], RECORDS);
            assert!(
                output["xl/pivotCache/_rels/pivotCacheDefinition1.xml.rels"]
                    .contains("Target=\"pivotCacheRecords1.xml\"")
            );
            for kind in ["pivotCacheDefinition", "pivotCacheRecords", "pivotTable"] {
                assert!(
                    output["[Content_Types].xml"].contains(&format!("spreadsheetml.{kind}+xml"))
                );
            }
            book = reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap();
            assert_eq!(book.sheet(0).unwrap().value("Z1"), "Unrelated edit");
            for pivot in book.sheet(0).unwrap().pivot_tables() {
                let cache = pivot.pivot_cache_definition();
                assert_eq!(cache.slicer_cache_id(), Some(902));
                assert_eq!(cache.raw_cache_records(), RECORDS.as_bytes());
                assert_eq!(
                    cache.cache_fields().list()[0].items(),
                    &[
                        umya_spreadsheet::SharedItemValue::String("Q1".into()),
                        umya_spreadsheet::SharedItemValue::String("Q2".into())
                    ]
                );
                assert_eq!(
                    cache.cache_fields().list()[0]
                        .properties()
                        .formula
                        .as_deref(),
                    Some("Amount*2")
                );
                let definition = pivot.pivot_table_definition();
                assert_eq!(definition.formats().unwrap().formats.len(), 1);
                assert_eq!(
                    definition.conditional_formats().unwrap().formats[0].priority,
                    3
                );
                assert_eq!(definition.chart_formats().unwrap().formats[0].chart, 4);
            }
        }
    }
}

#[test]
fn calculated_cache_items_survive_unrelated_edit() {
    let mut book = reader::xlsx::read_reader(Cursor::new(fixture(false, true)), true).unwrap();
    book.sheet_mut(0)
        .unwrap()
        .cell_mut("Z1")
        .set_value("Edited");
    for _ in 0..2 {
        let bytes = save(&book);
        let output = parts(&bytes);
        let cache = &output["xl/pivotCache/pivotCacheDefinition1.xml"];
        assert!(cache.contains("calculatedItems"), "{cache}");
        assert!(cache.contains("formula=\"Q1+Q2\""), "{cache}");
        assert!(cache.find("<calculatedItems").unwrap() < cache.find("<extLst").unwrap());
        book = reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap();
        assert_eq!(book.sheet(0).unwrap().value("Z1"), "Edited");
    }
}

#[test]
fn distinct_native_caches_on_one_sheet_bind_by_part_identity() {
    let book = reader::xlsx::read_reader(Cursor::new(fixture(true, false)), true).unwrap();
    let tables = book.sheet(0).unwrap().pivot_tables();
    assert_eq!(
        tables[0].pivot_cache_definition().slicer_cache_id(),
        Some(902)
    );
    assert_eq!(
        tables[1].pivot_cache_definition().slicer_cache_id(),
        Some(903)
    );
}

#[test]
fn missing_native_cache_relationship_is_an_explicit_import_error() {
    let mut input = parts(&fixture(false, false));
    input.remove("xl/pivotTables/_rels/pivotTable8.xml.rels");
    input.remove("xl/pivotTables/_rels/pivotTable3.xml.rels");
    assert!(reader::xlsx::read_reader(Cursor::new(pack(input)), true).is_err());
}

#[test]
fn cache_records_binding_resolves_the_declared_relationship_id() {
    let mut input = parts(&fixture(false, false));
    input.insert(
        "xl/pivotCache/_rels/pivotCacheDefinition9.xml.rels".into(),
        relationships(&[(
            "unreferenced",
            "pivotCacheRecords",
            "pivotCacheDefinition9Records.xml",
        )]),
    );
    assert!(reader::xlsx::read_reader(Cursor::new(pack(input)), true).is_err());
}

#[test]
fn duplicate_workbook_cache_ids_are_an_explicit_import_error() {
    let mut input = parts(&fixture(true, false));
    let workbook = input.get_mut("xl/workbook.xml").unwrap();
    *workbook = workbook.replace("cacheId=\"73\"", "cacheId=\"41\"");
    assert!(reader::xlsx::read_reader(Cursor::new(pack(input)), true).is_err());
}

#[test]
fn workbook_cache_binding_accounts_for_the_metadata_relationship() {
    let mut input = parts(&fixture(false, false));
    input.insert(
        "xl/metadata.xml".into(),
        format!(r#"<metadata xmlns="{NS}"/>"#),
    );
    let rels = input.get_mut("xl/_rels/workbook.xml.rels").unwrap();
    *rels = rels.replace("</Relationships>", &format!(r#"<Relationship Id="metadata" Type="{REL}/sheetMetadata" Target="metadata.xml"/></Relationships>"#));
    let types = input.get_mut("[Content_Types].xml").unwrap();
    *types = types.replace("</Types>", r#"<Override PartName="/xl/metadata.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheetMetadata+xml"/></Types>"#);
    let mut book = reader::xlsx::read_reader(Cursor::new(pack(input)), true).unwrap();
    book.sheet_mut(0)
        .unwrap()
        .cell_mut("Z1")
        .set_value("Edited");
    let bytes = save(&book);
    let output = parts(&bytes);
    assert!(output["xl/workbook.xml"].contains(r#"<pivotCache cacheId="41" r:id="rId3""#));
    assert!(output["xl/_rels/workbook.xml.rels"].contains(&format!(r#"Id="rId3" Type="{REL}/pivotCacheDefinition" Target="pivotCache/pivotCacheDefinition1.xml""#)));
    let reread = reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap();
    assert_eq!(
        reread.sheet(0).unwrap().pivot_tables()[0]
            .pivot_cache_definition()
            .slicer_cache_id(),
        Some(902)
    );
}

#[test]
fn nonconventional_pivot_part_names_preserve_native_cache_binding() {
    let input = parts(&fixture(false, false))
        .into_iter()
        .map(|(path, xml)| {
            (
                path.replace("pivotTable8", "quarterReport")
                    .replace("pivotTable3", "yearReport")
                    .replace("pivotCacheDefinition9", "quarterCache"),
                xml.replace("pivotTable8", "quarterReport")
                    .replace("pivotTable3", "yearReport")
                    .replace("pivotCacheDefinition9", "quarterCache"),
            )
        })
        .collect();
    let book = reader::xlsx::read_reader(Cursor::new(pack(input)), true).unwrap();
    assert_eq!(
        book.sheet(0).unwrap().pivot_tables()[0]
            .pivot_cache_definition()
            .slicer_cache_id(),
        Some(902)
    );
}

#[test]
fn calculated_item_optional_formula_and_invalid_payloads_follow_the_schema() {
    let valid = fixture(false, true);
    let mut input = parts(&valid);
    let cache = input
        .get_mut("xl/pivotCache/pivotCacheDefinition9.xml")
        .unwrap();
    *cache = cache.replace(r#" formula="Q1+Q2""#, "");
    let book = reader::xlsx::read_reader(Cursor::new(pack(input)), true).unwrap();
    let bytes = save(&book);
    reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap();
    for (old, new) in [
        (
            r#"<calculatedItems count="1">"#,
            r#"<calculatedItems count="0">"#,
        ),
        (
            r#"<calculatedItem field="0""#,
            r#"<calculatedItem field="-1""#,
        ),
        (
            r#"<pivotArea field="0"/></calculatedItem>"#,
            "</calculatedItem>",
        ),
    ] {
        let mut input = parts(&valid);
        let cache = input
            .get_mut("xl/pivotCache/pivotCacheDefinition9.xml")
            .unwrap();
        assert!(cache.contains(old));
        *cache = cache.replace(old, new);
        assert!(reader::xlsx::read_reader(Cursor::new(pack(input)), true).is_err());
    }
}

#[test]
fn calculation_markup_compatibility_attributes_are_preserved() {
    let mut input = parts(&fixture(false, true));
    let cache = input
        .get_mut("xl/pivotCache/pivotCacheDefinition9.xml")
        .unwrap();
    *cache = cache.replace(r#"<calculatedItems count="1">"#, r#"<calculatedItems xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" xmlns:future="urn:pivot-future" mc:Ignorable="future" count="1">"#);
    let mut book = reader::xlsx::read_reader(Cursor::new(pack(input)), true).unwrap();
    book.sheet_mut(0)
        .unwrap()
        .cell_mut("Z1")
        .set_value("Edited");
    let output = parts(&save(&book));
    assert!(output["xl/pivotCache/pivotCacheDefinition1.xml"].contains(r#"mc:Ignorable="future""#));
}

#[test]
fn prefixed_cache_definition_preserves_source_grouping_and_records() {
    let mut input = parts(&fixture(false, true));
    let cache = input
        .get_mut("xl/pivotCache/pivotCacheDefinition9.xml")
        .unwrap();
    for name in [
        "pivotCacheDefinition",
        "cacheSource",
        "worksheetSource",
        "cacheFields",
        "cacheField",
        "sharedItems",
        "s",
        "fieldGroup",
        "discretePr",
        "x",
        "groupItems",
        "calculatedItems",
        "calculatedItem",
        "pivotArea",
        "extLst",
        "ext",
    ] {
        *cache = cache
            .replace(&format!("<{name} "), &format!("<s:{name} "))
            .replace(&format!("<{name}>"), &format!("<s:{name}>"))
            .replace(&format!("</{name}>"), &format!("</s:{name}>"));
    }
    *cache = cache.replace(
        "<s:pivotCacheDefinition ",
        &format!(r#"<s:pivotCacheDefinition xmlns:s="{NS}" "#),
    );
    let mut book = reader::xlsx::read_reader(Cursor::new(pack(input)), true).unwrap();
    book.sheet_mut(0)
        .unwrap()
        .cell_mut("Z1")
        .set_value("Edited");
    let bytes = save(&book);
    let book = reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap();
    let cache = book.sheet(0).unwrap().pivot_tables()[0].pivot_cache_definition();
    assert_eq!(
        cache
            .cache_source()
            .worksheet_source()
            .unwrap()
            .address()
            .sheet_name(),
        "Sheet1"
    );
    assert_eq!(cache.cache_fields().list()[0].items().len(), 2);
    assert_eq!(cache.slicer_cache_id(), Some(902));
    assert_eq!(cache.raw_cache_records(), RECORDS.as_bytes());
}

#[test]
fn consolidation_cache_source_preserves_pages_and_ranges() {
    let mut input = parts(&fixture(false, false));
    let cache = input
        .get_mut("xl/pivotCache/pivotCacheDefinition9.xml")
        .unwrap();
    *cache = cache.replace(r#"<cacheSource type="worksheet"><worksheetSource sheet="Sheet1" ref="A1:A3"/></cacheSource>"#, CONSOLIDATION_SOURCE);
    assert!(cache.contains("consolidation"));
    let mut book = reader::xlsx::read_reader(Cursor::new(pack(input)), true).unwrap();
    book.sheet_mut(0)
        .unwrap()
        .cell_mut("Z1")
        .set_value("Edited");
    for _ in 0..2 {
        let bytes = save(&book);
        let output = parts(&bytes);
        let cache = &output["xl/pivotCache/pivotCacheDefinition1.xml"];
        for expected in [
            r#"type="consolidation""#,
            r#"autoPage="0""#,
            r#"name="North""#,
            r#"i1="0" ref="A1:A3" sheet="Sheet1""#,
        ] {
            assert!(cache.contains(expected), "{cache}");
        }
        book = reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap();
    }
}

#[test]
fn invalid_consolidation_pages_and_ranges_are_explicit_import_errors() {
    for (old, new) in [
        (r#"<pageItem name="North"/>"#, "<pageItem/>"),
        (r#"<pages count="1">"#, r#"<pages count="2">"#),
        (r#"i1="0""#, r#"i1="-1""#),
        (
            r#"<rangeSets count="1"><rangeSet i1="0" ref="A1:A3" sheet="Sheet1"/></rangeSets>"#,
            "",
        ),
    ] {
        let mut input = parts(&fixture(false, false));
        let cache = input
            .get_mut("xl/pivotCache/pivotCacheDefinition9.xml")
            .unwrap();
        *cache = cache.replace(r#"<cacheSource type="worksheet"><worksheetSource sheet="Sheet1" ref="A1:A3"/></cacheSource>"#, &CONSOLIDATION_SOURCE.replace(old, new));
        assert!(reader::xlsx::read_reader(Cursor::new(pack(input)), true).is_err());
    }
}

fn import_error(input: BTreeMap<String, String>) -> String {
    reader::xlsx::read_reader(Cursor::new(pack(input)), true)
        .expect_err("import must fail")
        .to_string()
}

#[test]
fn pivot_table_cache_id_disagreeing_with_the_workbook_binding_is_an_explicit_import_error() {
    let mut input = parts(&fixture(true, false));
    let table = input.get_mut("xl/pivotTables/pivotTable8.xml").unwrap();
    *table = table.replace("cacheId=\"41\"", "cacheId=\"73\"");
    let message = import_error(input);
    assert!(
        message.contains(
            "pivot table cacheId 73 is bound to a cache the workbook declares as cacheId 41"
        ),
        "{message}"
    );
}

#[test]
fn pivot_table_cache_part_missing_from_the_workbook_caches_is_an_explicit_import_error() {
    let mut input = parts(&fixture(true, false));
    let workbook = input.get_mut("xl/workbook.xml").unwrap();
    *workbook = workbook.replace(r#"<pivotCache cacheId="73" r:id="otherCache"/>"#, "");
    let message = import_error(input);
    assert!(
        message.contains("not declared by the workbook pivotCaches"),
        "{message}"
    );
}

#[test]
fn several_cache_definition_relationships_on_one_pivot_table_are_an_explicit_import_error() {
    let mut input = parts(&fixture(true, false));
    let rels = input
        .get_mut("xl/pivotTables/_rels/pivotTable8.xml.rels")
        .unwrap();
    *rels = rels.replace("</Relationships>", &format!(r#"<Relationship Id="second" Type="{REL}/pivotCacheDefinition" Target="../pivotCache/pivotCacheDefinition12.xml"/></Relationships>"#));
    let message = import_error(input);
    assert!(
        message.contains("more than one pivot cache definition relationship"),
        "{message}"
    );
}

#[test]
fn distinct_native_caches_keep_their_binding_after_save_and_reread() {
    for eager in [true, false] {
        let mut book = reader::xlsx::read_reader(Cursor::new(fixture(true, false)), eager).unwrap();
        book.sheet_mut(0)
            .unwrap()
            .cell_mut("Z1")
            .set_value("Edited");
        let reread = reader::xlsx::read_reader(Cursor::new(save(&book)), true).unwrap();
        let tables = reread.sheet(0).unwrap().pivot_tables();
        let bindings: Vec<_> = tables
            .iter()
            .map(|table| {
                (
                    table.pivot_table_definition().cache_id(),
                    table.pivot_cache_definition().slicer_cache_id(),
                )
            })
            .collect();
        assert_eq!(bindings, [(41, Some(902)), (73, Some(903))]);
    }
}

#[test]
fn root_level_pivot_table_part_binds_its_own_cache() {
    let mut input = parts(&fixture(false, false));
    let table = input.remove("xl/pivotTables/pivotTable8.xml").unwrap();
    let rels = input
        .remove("xl/pivotTables/_rels/pivotTable8.xml.rels")
        .unwrap();
    input.insert("pivotTable8.xml".into(), table);
    input.insert(
        "_rels/pivotTable8.xml.rels".into(),
        rels.replace(
            "../pivotCache/pivotCacheDefinition9.xml",
            "/xl/pivotCache/pivotCacheDefinition9.xml",
        ),
    );
    let sheet_rels = input
        .get_mut("xl/worksheets/_rels/sheet1.xml.rels")
        .unwrap();
    *sheet_rels = sheet_rels.replace("../pivotTables/pivotTable8.xml", "/pivotTable8.xml");
    let types = input.get_mut("[Content_Types].xml").unwrap();
    *types = types.replace("/xl/pivotTables/pivotTable8.xml", "/pivotTable8.xml");
    for eager in [true, false] {
        let mut book = reader::xlsx::read_reader(Cursor::new(pack(input.clone())), eager).unwrap();
        book.try_read_sheet(0).unwrap();
        let tables = book.sheet(0).unwrap().pivot_tables();
        assert_eq!(tables.len(), 2);
        assert_eq!(tables[0].pivot_table_definition().name(), "QuarterPivot");
        assert_eq!(
            tables[0].pivot_cache_definition().slicer_cache_id(),
            Some(902)
        );
    }
}

#[test]
fn workbook_cache_part_names_match_case_insensitively() {
    let mut input = parts(&fixture(false, false));
    let rels = input.get_mut("xl/_rels/workbook.xml.rels").unwrap();
    *rels = rels.replace(
        "pivotCache/pivotCacheDefinition9.xml",
        "PIVOTCACHE/PivotCacheDefinition9.xml",
    );
    let mut book = reader::xlsx::read_reader(Cursor::new(pack(input)), true).unwrap();
    book.try_read_sheet(0).unwrap();
    assert_eq!(book.sheet(0).unwrap().pivot_tables().len(), 2);
}

#[test]
fn internal_relationship_resolving_to_no_part_is_an_explicit_import_error() {
    // Excel rejects a package with an internal relationship whose target ends
    // in an empty, `.` or `..` segment.
    for target in ["/", "../..", "", ".", ".."] {
        let mut input = parts(&fixture(false, false));
        let rels = input
            .get_mut("xl/worksheets/_rels/sheet1.xml.rels")
            .unwrap();
        *rels = rels.replace("</Relationships>", &format!(r#"<Relationship Id="empty" Type="{REL}/customXml" Target="{target}"/></Relationships>"#));
        for eager in [true, false] {
            let message = reader::xlsx::read_reader(Cursor::new(pack(input.clone())), eager)
                .expect_err("import must fail")
                .to_string();
            assert!(
                message.contains("relationship empty") && message.contains("no package part"),
                "{target}: {message}"
            );
        }
    }
}

/// Excel opens a package whose internal relationship names a part that does
/// not exist, as long as the target is a valid part name.
#[test]
fn internal_relationship_naming_a_missing_part_is_accepted() {
    for target in ["../worksheets", "/xl"] {
        let mut input = parts(&fixture(false, false));
        let rels = input
            .get_mut("xl/worksheets/_rels/sheet1.xml.rels")
            .unwrap();
        *rels = rels.replace("</Relationships>", &format!(r#"<Relationship Id="folder" Type="{REL}/customXml" Target="{target}"/></Relationships>"#));
        for eager in [true, false] {
            let mut book =
                reader::xlsx::read_reader(Cursor::new(pack(input.clone())), eager).unwrap();
            book.try_read_sheet(0).unwrap();
            assert_eq!(
                book.sheet(0).unwrap().pivot_tables().len(),
                2,
                "{target} eager={eager}"
            );
        }
    }
}

#[test]
fn external_relationships_do_not_need_a_part() {
    let mut input = parts(&fixture(false, false));
    let rels = input
        .get_mut("xl/worksheets/_rels/sheet1.xml.rels")
        .unwrap();
    *rels = rels.replace("</Relationships>", &format!(r#"<Relationship Id="link" Type="{REL}/hyperlink" Target="https://example.com/a" TargetMode="External"/></Relationships>"#));
    let mut book = reader::xlsx::read_reader(Cursor::new(pack(input)), true).unwrap();
    book.try_read_sheet(0).unwrap();
    assert_eq!(book.sheet(0).unwrap().pivot_tables().len(), 2);
}

#[test]
fn worksheet_with_a_non_conventional_part_name_keeps_its_relationships() {
    let input = parts(&fixture(false, false))
        .into_iter()
        .map(|(path, xml)| {
            (
                path.replace("sheet1.xml", "Data.xml"),
                xml.replace("worksheets/sheet1.xml", "worksheets/Data.xml"),
            )
        })
        .collect::<BTreeMap<_, _>>();
    assert!(input.contains_key("xl/worksheets/_rels/Data.xml.rels"));
    for eager in [true, false] {
        let mut book = reader::xlsx::read_reader(Cursor::new(pack(input.clone())), eager).unwrap();
        book.try_read_sheet(0).unwrap();
        assert_eq!(
            book.sheet(0).unwrap().pivot_tables().len(),
            2,
            "eager={eager}"
        );
    }
}

#[test]
fn cyclic_internal_relationships_are_read_once() {
    let mut input = parts(&fixture(false, false));
    let rels = input
        .get_mut("xl/worksheets/_rels/sheet1.xml.rels")
        .unwrap();
    *rels = rels.replace("</Relationships>", &format!(r#"<Relationship Id="loop" Type="{REL}/customXml" Target="../extra1.xml"/></Relationships>"#));
    input.insert("xl/extra1.xml".into(), "<extra/>".into());
    input.insert(
        "xl/_rels/extra1.xml.rels".into(),
        relationships(&[("back", "customXml", "worksheets/sheet1.xml")]),
    );
    for eager in [true, false] {
        let mut book = reader::xlsx::read_reader(Cursor::new(pack(input.clone())), eager).unwrap();
        book.try_read_sheet(0).unwrap();
        assert_eq!(book.sheet(0).unwrap().pivot_tables().len(), 2);
    }
}
