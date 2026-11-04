use std::io::{
    Cursor,
    Read,
    Write,
};

use umya_spreadsheet::{
    PivotCacheDefinition,
    reader,
    writer,
};
use zip::{
    ZipArchive,
    ZipWriter,
    write::SimpleFileOptions,
};

fn fixture(connection_id: &str) -> Vec<u8> {
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    let parts = [
        ("[Content_Types].xml", r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/><Override PartName="/xl/pivotTables/pivotTable1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.pivotTable+xml"/><Override PartName="/xl/pivotCache/pivotCacheDefinition1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.pivotCacheDefinition+xml"/></Types>"#.to_string()),
        ("_rels/.rels", r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#.to_string()),
        ("xl/workbook.xml", r#"<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="Sheet1" sheetId="1" r:id="rId1"/></sheets><pivotCaches><pivotCache cacheId="0" r:id="rId2"/></pivotCaches></workbook>"#.to_string()),
        ("xl/_rels/workbook.xml.rels", r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/pivotCacheDefinition" Target="pivotCache/pivotCacheDefinition1.xml"/></Relationships>"#.to_string()),
        ("xl/worksheets/sheet1.xml", r#"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheetData/><pivotTableParts count="1"><pivotTablePart r:id="rId1"/></pivotTableParts></worksheet>"#.to_string()),
        ("xl/worksheets/_rels/sheet1.xml.rels", r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/pivotTable" Target="../pivotTables/pivotTable1.xml"/></Relationships>"#.to_string()),
        ("xl/pivotTables/pivotTable1.xml", r#"<pivotTableDefinition xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" name="CubePivot" cacheId="0" dataCaption="Values"><location ref="D1:E3" firstHeaderRow="0" firstDataRow="1" firstDataCol="1"/><pivotFields count="1"><pivotField/></pivotFields></pivotTableDefinition>"#.to_string()),
        ("xl/pivotTables/_rels/pivotTable1.xml.rels", r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/pivotCacheDefinition" Target="../pivotCache/pivotCacheDefinition1.xml"/></Relationships>"#.to_string()),
        ("xl/pivotCache/pivotCacheDefinition1.xml", format!(r#"<pivotCacheDefinition xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><cacheSource type="external" connectionId="{connection_id}"/><cacheFields count="1"><cacheField name="[Measures].[Revenue]" databaseField="0"/></cacheFields></pivotCacheDefinition>"#)),
    ];
    for (name, mut xml) in parts {
        if name == "[Content_Types].xml" {
            xml = xml.replace("</Types>", r#"<Override PartName="/xl/connections.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.connections+xml"/></Types>"#);
        } else if name == "xl/_rels/workbook.xml.rels" {
            xml = xml.replace("</Relationships>", r#"<Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/connections" Target="connections.xml"/></Relationships>"#);
        } else if name == "xl/pivotCache/pivotCacheDefinition1.xml" {
            xml = xml.replace("</pivotCacheDefinition>", r#"<calculatedMembers count="1"><calculatedMember name="Margin" mdx="[Measures].[Revenue]*0.2" memberName="[Measures].[Margin]" hierarchy="[Measures]"/></calculatedMembers></pivotCacheDefinition>"#);
        }
        zip.start_file(name, SimpleFileOptions::default()).unwrap();
        zip.write_all(xml.as_bytes()).unwrap();
    }
    zip.start_file("xl/connections.xml", SimpleFileOptions::default())
        .unwrap();
    zip.write_all(br#"<connections xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><connection id="17" name="SalesCube" type="5" refreshedVersion="3"><dbPr connection="Provider=MSOLAP;Data Source=localhost;Initial Catalog=Sales" command="SalesCube" commandType="1"/><olapPr local="0"/></connection></connections>"#).unwrap();
    zip.finish().unwrap().into_inner()
}

#[test]
fn external_pivot_cache_connection_survives_unrelated_edit_and_reload() {
    let mut book = reader::xlsx::read_reader(Cursor::new(fixture("17")), true).unwrap();
    book.sheet_mut(0)
        .unwrap()
        .cell_mut("A1")
        .set_value("Edited");
    let mut bytes = Vec::new();
    writer::xlsx::write_writer(&book, &mut bytes).unwrap();
    let mut archive = ZipArchive::new(Cursor::new(&bytes)).unwrap();
    let mut xml = String::new();
    archive
        .by_name("xl/pivotCache/pivotCacheDefinition1.xml")
        .unwrap()
        .read_to_string(&mut xml)
        .unwrap();
    assert!(xml.contains("connectionId=\"17\""), "{xml}");
    let reread = reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap();
    assert_eq!(reread.sheet(0).unwrap().value("A1"), "Edited");
    let cache = reread.sheet(0).unwrap().pivot_tables()[0]
        .pivot_cache_definition()
        .to_xml()
        .unwrap();
    assert!(
        String::from_utf8(cache)
            .unwrap()
            .contains("connectionId=\"17\"")
    );
}

#[test]
fn malformed_external_connection_id_is_an_explicit_import_error() {
    assert!(reader::xlsx::read_reader(Cursor::new(fixture("seventeen")), true).is_err());
    assert!(PivotCacheDefinition::from_xml(br#"<pivotCacheDefinition xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><cacheSource type="external" connectionId="seventeen"/><cacheFields count="0"/></pivotCacheDefinition>"#).is_err());
}

#[test]
fn calculation_and_cube_metadata_survive_an_edit_on_another_sheet() {
    const BEFORE_CALCULATIONS: &str = r#"<kpis count="1"><kpi uniqueName="[KPIs].[Margin]" value="[Measures].[Margin]" caption="Margin"/></kpis><tupleCache><entries count="1"><s v="[Date].[2024]"/></entries></tupleCache>"#;
    const AFTER_CALCULATIONS: &str = r#"<dimensions count="1"><dimension name="Date" uniqueName="[Date]" caption="Date"/></dimensions><measureGroups count="1"><measureGroup name="Sales" caption="Sales"/></measureGroups><maps count="1"><map measureGroup="0" dimension="0"/></maps>"#;
    let mut input = ZipArchive::new(Cursor::new(fixture("17"))).unwrap();
    let mut output = ZipWriter::new(Cursor::new(Vec::new()));
    for index in 0..input.len() {
        let mut part = input.by_index(index).unwrap();
        let name = part.name().unwrap().into_owned();
        let mut xml = String::new();
        part.read_to_string(&mut xml).unwrap();
        if name == "xl/pivotCache/pivotCacheDefinition1.xml" {
            xml = xml.replace(
                "<calculatedMembers",
                &format!("{BEFORE_CALCULATIONS}<calculatedMembers"),
            );
            xml = xml.replace(
                "</pivotCacheDefinition>",
                &format!("{AFTER_CALCULATIONS}</pivotCacheDefinition>"),
            );
        } else if name == "xl/workbook.xml" {
            xml = xml.replace(
                "</sheets>",
                r#"<sheet name="Edits" sheetId="2" r:id="edits"/></sheets>"#,
            );
        } else if name == "xl/_rels/workbook.xml.rels" {
            xml = xml.replace("</Relationships>", r#"<Relationship Id="edits" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet2.xml"/></Relationships>"#);
        } else if name == "[Content_Types].xml" {
            xml = xml.replace("</Types>", r#"<Override PartName="/xl/worksheets/sheet2.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/></Types>"#);
        }
        output
            .start_file(name, SimpleFileOptions::default())
            .unwrap();
        output.write_all(xml.as_bytes()).unwrap();
    }
    output
        .start_file("xl/worksheets/sheet2.xml", SimpleFileOptions::default())
        .unwrap();
    output.write_all(br#"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData/></worksheet>"#).unwrap();
    let input = output.finish().unwrap().into_inner();
    for eager in [true, false] {
        let mut book = reader::xlsx::read_reader(Cursor::new(&input), eager).unwrap();
        book.sheet_mut(1)
            .unwrap()
            .cell_mut("A1")
            .set_value("Edited elsewhere");
        for _ in 0..2 {
            let mut bytes = Vec::new();
            writer::xlsx::write_writer(&book, &mut bytes).unwrap();
            let mut archive = ZipArchive::new(Cursor::new(&bytes)).unwrap();
            let mut xml = String::new();
            archive
                .by_name("xl/pivotCache/pivotCacheDefinition1.xml")
                .unwrap()
                .read_to_string(&mut xml)
                .unwrap();
            let mut prior = 0;
            for name in [
                "kpis",
                "tupleCache",
                "calculatedMembers",
                "dimensions",
                "measureGroups",
                "maps",
            ] {
                let position = xml
                    .find(&format!("<{name}"))
                    .unwrap_or_else(|| panic!("missing {name}: {xml}"));
                assert!(position > prior, "schema order: {xml}");
                prior = position;
            }
            for expected in [
                r#"uniqueName="[KPIs].[Margin]""#,
                r#"v="[Date].[2024]""#,
                r#"uniqueName="[Date]""#,
                r#"caption="Sales""#,
                r#"measureGroup="0" dimension="0""#,
            ] {
                assert!(xml.contains(expected), "{xml}");
            }
            book = reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap();
            assert_eq!(book.sheet(1).unwrap().value("A1"), "Edited elsewhere");
        }
    }
}

#[test]
fn olap_calculated_member_survives_unrelated_edit_and_reload() {
    let mut book = reader::xlsx::read_reader(Cursor::new(fixture("17")), true).unwrap();
    book.sheet_mut(0)
        .unwrap()
        .cell_mut("A1")
        .set_value("Edited");
    let mut bytes = Vec::new();
    writer::xlsx::write_writer(&book, &mut bytes).unwrap();
    let mut archive = ZipArchive::new(Cursor::new(&bytes)).unwrap();
    let mut xml = String::new();
    archive
        .by_name("xl/pivotCache/pivotCacheDefinition1.xml")
        .unwrap()
        .read_to_string(&mut xml)
        .unwrap();
    for expected in [
        "calculatedMembers",
        "name=\"Margin\"",
        "mdx=\"[Measures].[Revenue]*0.2\"",
        "memberName=\"[Measures].[Margin]\"",
        "hierarchy=\"[Measures]\"",
    ] {
        assert!(xml.contains(expected), "missing {expected}: {xml}");
    }
    let reread = reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap();
    let cache = reread.sheet(0).unwrap().pivot_tables()[0]
        .pivot_cache_definition()
        .to_xml()
        .unwrap();
    assert!(
        String::from_utf8(cache)
            .unwrap()
            .contains("mdx=\"[Measures].[Revenue]*0.2\"")
    );
}

fn element_containing<'a>(xml: &'a str, tag: &str, needle: &str) -> &'a str {
    xml.split(&format!("<{tag}"))
        .skip(1)
        .find(|element| element.split('>').next().unwrap().contains(needle))
        .unwrap_or_else(|| panic!("no <{tag}> containing {needle}: {xml}"))
        .split('>')
        .next()
        .unwrap()
}

#[test]
fn olap_workbook_connection_part_and_relationship_survive_unrelated_edit() {
    for eager in [true, false] {
        let mut book = reader::xlsx::read_reader(Cursor::new(fixture("17")), eager).unwrap();
        book.sheet_mut(0)
            .unwrap()
            .cell_mut("A1")
            .set_value("Edited");
        let mut bytes = Vec::new();
        writer::xlsx::write_writer(&book, &mut bytes).unwrap();
        let connection = archive_text(&bytes, "xl/connections.xml")
            .expect("external pivot connection part must survive");
        assert!(connection.contains("id=\"17\""));
        assert!(connection.contains("command=\"SalesCube\""));
        let rels = archive_text(&bytes, "xl/_rels/workbook.xml.rels").unwrap();
        let relationship = element_containing(&rels, "Relationship", "relationships/connections");
        assert!(
            relationship.contains("Target=\"connections.xml\""),
            "{rels}"
        );
        let mut ids: Vec<&str> = rels
            .split("Id=\"")
            .skip(1)
            .map(|rest| rest.split('"').next().unwrap())
            .collect();
        let relationship_count = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), relationship_count, "duplicate rId: {rels}");
        let types = archive_text(&bytes, "[Content_Types].xml").unwrap();
        let declared = element_containing(&types, "Override", "/xl/connections.xml");
        assert!(
            declared.contains(
                "ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.\
                 connections+xml\""
            ),
            "{types}"
        );
        let reread = reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap();
        assert_eq!(reread.sheet(0).unwrap().pivot_tables().len(), 1);
    }
}

fn rewrite_parts(source: &[u8], edit: impl Fn(&str, String) -> String) -> Vec<u8> {
    let mut input = ZipArchive::new(Cursor::new(source)).unwrap();
    let mut output = ZipWriter::new(Cursor::new(Vec::new()));
    for index in 0..input.len() {
        let mut part = input.by_index(index).unwrap();
        let name = part.name().unwrap().into_owned();
        let mut xml = String::new();
        part.read_to_string(&mut xml).unwrap();
        let xml = edit(&name, xml);
        output
            .start_file(name, SimpleFileOptions::default())
            .unwrap();
        output.write_all(xml.as_bytes()).unwrap();
    }
    output.finish().unwrap().into_inner()
}

fn import_error(source: Vec<u8>, eager: bool) -> String {
    reader::xlsx::read_reader(Cursor::new(source), eager)
        .expect_err("import must fail")
        .to_string()
}

fn retarget_connections(target: &str) -> Vec<u8> {
    rewrite_parts(&fixture("17"), |name, xml| {
        if name == "xl/_rels/workbook.xml.rels" {
            xml.replace(
                "Target=\"connections.xml\"",
                &format!("Target=\"{target}\""),
            )
        } else {
            xml
        }
    })
}

#[test]
fn second_connections_relationship_is_an_explicit_import_error() {
    let source = rewrite_parts(&fixture("17"), |name, xml| {
        if name == "xl/_rels/workbook.xml.rels" {
            xml.replace("</Relationships>", r#"<Relationship Id="rId4" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/connections" Target="connections.xml"/></Relationships>"#)
        } else {
            xml
        }
    });
    for eager in [true, false] {
        let message = import_error(source.clone(), eager);
        assert!(
            message.contains("more than one connections relationship"),
            "{message}"
        );
    }
}

#[test]
fn connections_target_naming_another_kind_of_part_is_an_explicit_import_error() {
    for target in [
        "workbook.xml",
        "styles.xml",
        "worksheets/sheet1.xml",
        "pivotCache/pivotCacheDefinition1.xml",
    ] {
        for eager in [true, false] {
            let message = import_error(retarget_connections(target), eager);
            assert!(
                message.contains("connections content type"),
                "{target}: {message}"
            );
        }
    }
}

#[test]
fn connections_relationship_to_a_missing_part_is_an_explicit_import_error() {
    let source = rewrite_parts(&fixture("17"), |name, xml| {
        if name == "xl/_rels/workbook.xml.rels" {
            xml.replace("Target=\"connections.xml\"", "Target=\"data/missing.xml\"")
        } else if name == "[Content_Types].xml" {
            xml.replace("/xl/connections.xml", "/xl/data/missing.xml")
        } else {
            xml
        }
    });
    let message = import_error(source, true);
    assert!(message.contains("not found"), "{message}");
}

/// Moves a package part, keeping its content-type override and relationship
/// target consistent.
fn relocate_connections(new_part: &str, new_target: &str) -> Vec<u8> {
    let mut input = ZipArchive::new(Cursor::new(fixture("17"))).unwrap();
    let mut output = ZipWriter::new(Cursor::new(Vec::new()));
    for index in 0..input.len() {
        let mut part = input.by_index(index).unwrap();
        let name = part.name().unwrap().into_owned();
        let mut xml = String::new();
        part.read_to_string(&mut xml).unwrap();
        let name = if name == "xl/connections.xml" {
            new_part.to_owned()
        } else {
            name
        };
        if name == "[Content_Types].xml" {
            xml = xml.replace("/xl/connections.xml", &format!("/{new_part}"));
        } else if name == "xl/_rels/workbook.xml.rels" {
            xml = xml.replace(
                "Target=\"connections.xml\"",
                &format!("Target=\"{new_target}\""),
            );
        }
        output
            .start_file(name, SimpleFileOptions::default())
            .unwrap();
        output.write_all(xml.as_bytes()).unwrap();
    }
    output.finish().unwrap().into_inner()
}

/// The package with the given new parts appended.
fn with_parts(source: &[u8], parts: &[(&str, &str)]) -> Vec<u8> {
    let mut input = ZipArchive::new(Cursor::new(source)).unwrap();
    let mut output = ZipWriter::new(Cursor::new(Vec::new()));
    for index in 0..input.len() {
        let mut part = input.by_index(index).unwrap();
        let name = part.name().unwrap().into_owned();
        let mut bytes = Vec::new();
        part.read_to_end(&mut bytes).unwrap();
        output
            .start_file(name, SimpleFileOptions::default())
            .unwrap();
        output.write_all(&bytes).unwrap();
    }
    for (name, body) in parts {
        output
            .start_file(*name, SimpleFileOptions::default())
            .unwrap();
        output.write_all(body.as_bytes()).unwrap();
    }
    output.finish().unwrap().into_inner()
}

fn archive_text(bytes: &[u8], name: &str) -> Option<String> {
    let mut archive = ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut text = String::new();
    archive.by_name(name).ok()?.read_to_string(&mut text).ok()?;
    Some(text)
}

#[test]
fn connections_part_with_any_valid_name_is_saved_under_the_writer_name() {
    for (part, target) in [
        ("xl/data/conn.xml", "data/conn.xml"),
        ("connections.xml", "/connections.xml"),
        ("xl/metadata.xml", "metadata.xml"),
    ] {
        for eager in [true, false] {
            let mut book =
                reader::xlsx::read_reader(Cursor::new(relocate_connections(part, target)), eager)
                    .unwrap();
            // A new writer-owned part must not collide with the imported name.
            let cm = book.ensure_dynamic_array_metadata();
            assert_eq!(cm, 1);
            book.sheet_mut(0).unwrap().cell_mut("A1").set_value("x");
            let mut bytes = Vec::new();
            writer::xlsx::write_writer(&book, &mut bytes).unwrap();
            let connections = archive_text(&bytes, "xl/connections.xml").unwrap();
            assert!(connections.contains("command=\"SalesCube\""), "{part}");
            let metadata = archive_text(&bytes, "xl/metadata.xml").unwrap();
            assert!(metadata.contains("<metadata"), "{part}: {metadata}");
            assert!(!metadata.contains("SalesCube"), "{part}");
            if part != "xl/metadata.xml" {
                assert!(
                    archive_text(&bytes, part).is_none(),
                    "{part} must not remain"
                );
            }
            let rels = archive_text(&bytes, "xl/_rels/workbook.xml.rels").unwrap();
            let relationship =
                element_containing(&rels, "Relationship", "relationships/connections");
            assert!(
                relationship.contains("Target=\"connections.xml\""),
                "{rels}"
            );
            let reread = reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap();
            assert_eq!(reread.sheet(0).unwrap().pivot_tables().len(), 1);
        }
    }
}

const DYNAMIC_METADATA: &str = r#"<metadata xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:d="http://schemas.microsoft.com/office/spreadsheetml/2017/dynamicarray"><metadataTypes count="1"><metadataType name="XLDAPR"/></metadataTypes><futureMetadata name="XLDAPR" count="1"><bk><extLst><ext uri="existing"><d:dynamicArrayProperties fDynamic="1"/></ext></extLst></bk></futureMetadata><cellMetadata count="1"><bk><rc t="1" v="0"/></bk></cellMetadata></metadata>"#;
const EMPTY_METADATA: &str =
    r#"<metadata xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"/>"#;

/// A workbook with two spreadsheet metadata relationships, in the given order,
/// to two different parts.
fn two_metadata_parts(first: &str, second: &str) -> Vec<u8> {
    let source = rewrite_parts(&fixture("17"), |name, xml| {
        match name {
        "xl/_rels/workbook.xml.rels" => xml.replace("</Relationships>", r#"<Relationship Id="rM1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/sheetMetadata" Target="metadata1.xml"/><Relationship Id="rM2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/sheetMetadata" Target="metadata2.xml"/></Relationships>"#),
        "[Content_Types].xml" => xml.replace("</Types>", r#"<Override PartName="/xl/metadata1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheetMetadata+xml"/><Override PartName="/xl/metadata2.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheetMetadata+xml"/></Types>"#),
        _ => xml,
    }
    });
    with_parts(
        &source,
        &[("xl/metadata1.xml", first), ("xl/metadata2.xml", second)],
    )
}

/// Excel opens a workbook that declares several metadata relationships when no
/// cell uses the metadata, so import binds the first relationship and ignores
/// the others instead of rejecting the workbook.
#[test]
fn first_of_several_metadata_relationships_is_the_one_bound() {
    for (first, second, first_is_dynamic) in [
        (DYNAMIC_METADATA, EMPTY_METADATA, true),
        (EMPTY_METADATA, DYNAMIC_METADATA, false),
    ] {
        let source = two_metadata_parts(first, second);
        for eager in [true, false] {
            let mut book = reader::xlsx::read_reader(Cursor::new(&source), eager).unwrap();
            book.try_read_sheet(0).unwrap();
            assert_eq!(
                book.is_dynamic_array_metadata(1),
                first_is_dynamic,
                "eager={eager} first_is_dynamic={first_is_dynamic}"
            );
        }
    }
}

#[test]
fn connections_part_with_another_content_type_is_an_explicit_import_error() {
    let source = rewrite_parts(&fixture("17"), |name, xml| {
        if name == "[Content_Types].xml" {
            xml.replace(
                "application/vnd.openxmlformats-officedocument.spreadsheetml.connections+xml",
                "application/vnd.openxmlformats-officedocument.spreadsheetml.sheetMetadata+xml",
            )
        } else {
            xml
        }
    });
    let message = import_error(source, true);
    assert!(message.contains("connections content type"), "{message}");
}

#[test]
fn oversized_connections_part_is_an_explicit_import_error() {
    let padding = " ".repeat(16 * 1024 * 1024 + 1);
    let source = rewrite_parts(&fixture("17"), |name, xml| {
        if name == "xl/connections.xml" {
            xml.replace("</connections>", &format!("{padding}</connections>"))
        } else {
            xml
        }
    });
    for eager in [true, false] {
        let message = import_error(source.clone(), eager);
        assert!(message.contains("size limit"), "{message}");
    }
}

const SHEET_NS: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";

/// Adds a second worksheet part to the package, `second_cell` being the XML
/// of the cell it holds.
fn with_second_sheet(source: &[u8], second_cell: &str) -> Vec<u8> {
    let source = rewrite_parts(source, |name, xml| {
        match name {
        "xl/workbook.xml" => xml.replace(
            "</sheets>",
            r#"<sheet name="Sheet2" sheetId="2" r:id="rSheet2"/></sheets>"#,
        ),
        "xl/_rels/workbook.xml.rels" => xml.replace(
            "</Relationships>",
            r#"<Relationship Id="rSheet2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet2.xml"/></Relationships>"#,
        ),
        "[Content_Types].xml" => xml.replace(
            "</Types>",
            r#"<Override PartName="/xl/worksheets/sheet2.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/></Types>"#,
        ),
        _ => xml,
    }
    });
    let sheet = format!(
        r#"<worksheet xmlns="{SHEET_NS}"><sheetData><row r="1">{second_cell}</row></sheetData></worksheet>"#
    );
    with_parts(&source, &[("xl/worksheets/sheet2.xml", &sheet)])
}

/// Sheet data whose cell carries a metadata reference.
const METADATA_CELL_DATA: &str =
    r#"<sheetData><row r="1"><c r="A1" cm="1"><v>1</v></c></row></sheetData>"#;

/// Excel rejects a workbook with several metadata relationships once any cell
/// uses the metadata, so import rejects it too, for every worksheet part
/// whether or not it is materialized.
#[test]
fn several_metadata_relationships_with_a_metadata_cell_are_an_explicit_import_error() {
    let two = two_metadata_parts(DYNAMIC_METADATA, EMPTY_METADATA);
    let cases = [
        (
            "first sheet",
            with_first_sheet_data(&two, METADATA_CELL_DATA),
            r#"cell "A1" of xl/worksheets/sheet1.xml uses cell metadata"#,
        ),
        (
            "later sheet",
            with_second_sheet(&two, r#"<c r="B7" cm="1"><v>1</v></c>"#),
            r#"cell "B7" of xl/worksheets/sheet2.xml uses cell metadata"#,
        ),
    ];
    for (name, source, expected) in cases {
        for eager in [true, false] {
            let message = import_error(source.clone(), eager);
            assert!(
                message.contains("more than one spreadsheet metadata relationship")
                    && message.contains(expected),
                "{name} eager={eager}: {message}"
            );
        }
    }
}

#[test]
fn several_metadata_relationships_are_accepted_when_only_other_cells_exist() {
    let source = with_second_sheet(
        &two_metadata_parts(DYNAMIC_METADATA, EMPTY_METADATA),
        r#"<c r="B7"><v>1</v></c>"#,
    );
    for eager in [true, false] {
        reader::xlsx::read_reader(Cursor::new(&source), eager).unwrap();
    }
}

#[test]
fn single_metadata_relationship_with_a_metadata_cell_is_accepted() {
    // Excel opens one metadata relationship with cells using it.
    let source = rewrite_parts(
        &with_first_sheet_data(
            &two_metadata_parts(DYNAMIC_METADATA, EMPTY_METADATA),
            METADATA_CELL_DATA,
        ),
        |name, xml| {
            if name == "xl/_rels/workbook.xml.rels" {
                xml.replace(
                    r#"<Relationship Id="rM2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/sheetMetadata" Target="metadata2.xml"/>"#,
                    "",
                )
            } else {
                xml
            }
        },
    );
    for eager in [true, false] {
        let book = reader::xlsx::read_reader(Cursor::new(&source), eager).unwrap();
        assert!(book.is_dynamic_array_metadata(1), "eager={eager}");
    }
}

fn with_first_sheet_data(source: &[u8], sheet_data: &str) -> Vec<u8> {
    rewrite_parts(source, |name, xml| {
        if name == "xl/worksheets/sheet1.xml" {
            xml.replace("<sheetData/>", sheet_data)
        } else {
            xml
        }
    })
}

/// Only an unqualified `cm` attribute on a SpreadsheetML cell is a metadata
/// reference: a namespace declaration named `cm`, a foreign-namespace
/// attribute and a foreign element named `c` are not.
#[test]
fn several_metadata_relationships_ignore_lookalikes_of_the_cell_metadata_attribute() {
    let two = two_metadata_parts(DYNAMIC_METADATA, EMPTY_METADATA);
    for (name, sheet_data) in [
        (
            "xmlns:cm declaration",
            r#"<sheetData><row r="1"><c r="A1" xmlns:cm="urn:unused"><v>1</v></c></row></sheetData>"#,
        ),
        (
            "foreign-namespace attribute",
            r#"<sheetData><row r="1"><c r="A1" xmlns:x="urn:foreign" x:cm="1"><v>1</v></c></row></sheetData>"#,
        ),
        (
            "prefixed foreign element",
            r#"<sheetData><row r="1"><x:c xmlns:x="urn:foreign" r="A1" cm="1"/></row></sheetData>"#,
        ),
        (
            "default-namespace foreign element",
            r#"<sheetData><row r="1"><c xmlns="urn:foreign" r="A1" cm="1"/></row></sheetData>"#,
        ),
    ] {
        let source = with_first_sheet_data(&two, sheet_data);
        for eager in [true, false] {
            reader::xlsx::read_reader(Cursor::new(&source), eager)
                .unwrap_or_else(|error| panic!("{name} eager={eager}: {error}"));
        }
    }
    // A prefixed element in the SpreadsheetML main namespace is a cell too.
    let source = with_first_sheet_data(
        &two,
        r#"<sheetData xmlns:x="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><row r="1"><x:c r="A1" cm="1"><v>1</v></x:c></row></sheetData>"#,
    );
    for eager in [true, false] {
        let message = import_error(source.clone(), eager);
        assert!(
            message.contains(r#"cell "A1" of xl/worksheets/sheet1.xml uses cell metadata"#),
            "prefixed main-namespace cell eager={eager}: {message}"
        );
    }
    // A genuine attribute next to such a declaration is still a metadata use.
    let source = with_first_sheet_data(
        &two,
        r#"<sheetData><row r="1"><c r="A1" xmlns:cm="urn:unused" cm="1"><v>1</v></c></row></sheetData>"#,
    );
    for eager in [true, false] {
        let message = import_error(source.clone(), eager);
        assert!(
            message.contains(r#"cell "A1" of xl/worksheets/sheet1.xml uses cell metadata"#),
            "eager={eager}: {message}"
        );
    }
}
