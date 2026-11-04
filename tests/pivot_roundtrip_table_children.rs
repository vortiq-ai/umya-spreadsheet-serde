use std::io::{
    Cursor,
    Read,
    Write,
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
const PAGE_FIELDS: &str = r#"<pageFields count="1"><pageField fld="0" item="1" hier="-1" name="RegionPage" cap="Region"/></pageFields>"#;
const FILTERS: &str = r#"<filters count="1"><filter fld="0" type="captionEqual" id="7" name="Only East" description="Office filter" stringValue1="East"><autoFilter ref="A1:A3"><filterColumn colId="0"><filters><filter val="East"/></filters></filterColumn></autoFilter></filter></filters>"#;
const ROW_USAGE: &str = r#"<rowHierarchiesUsage count="1"><rowHierarchyUsage hierarchyUsage="0"/></rowHierarchiesUsage>"#;
const COLUMN_USAGE: &str = r#"<colHierarchiesUsage count="1"><colHierarchyUsage hierarchyUsage="1"/></colHierarchiesUsage>"#;

fn relationships(entries: &[(&str, &str, &str)]) -> String {
    let children: String = entries
        .iter()
        .map(|(id, kind, target)| {
            format!(r#"<Relationship Id="{id}" Type="{REL}/{kind}" Target="{target}"/>"#)
        })
        .collect();
    format!(
        r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">{children}</Relationships>"#
    )
}

fn fixture(page_fields: &str, tail: &str) -> Vec<u8> {
    let parts = [
        ("[Content_Types].xml", r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/><Override PartName="/xl/worksheets/sheet2.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/><Override PartName="/xl/pivotTables/pivotTable1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.pivotTable+xml"/><Override PartName="/xl/pivotCache/pivotCacheDefinition1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.pivotCacheDefinition+xml"/></Types>"#.into()),
        ("_rels/.rels", relationships(&[("office", "officeDocument", "xl/workbook.xml")])),
        ("xl/workbook.xml", format!(r#"<workbook xmlns="{NS}" xmlns:r="{REL}"><sheets><sheet name="Pivot" sheetId="1" r:id="pivotSheet"/><sheet name="Notes" sheetId="2" r:id="notesSheet"/></sheets><pivotCaches><pivotCache cacheId="42" r:id="cache"/></pivotCaches></workbook>"#)),
        ("xl/_rels/workbook.xml.rels", relationships(&[("pivotSheet", "worksheet", "worksheets/sheet1.xml"), ("notesSheet", "worksheet", "worksheets/sheet2.xml"), ("cache", "pivotCacheDefinition", "pivotCache/pivotCacheDefinition1.xml")])),
        ("xl/worksheets/sheet1.xml", format!(r#"<worksheet xmlns="{NS}" xmlns:r="{REL}"><sheetData/><pivotTableParts count="1"><pivotTablePart r:id="pivot"/></pivotTableParts></worksheet>"#)),
        ("xl/worksheets/sheet2.xml", format!(r#"<worksheet xmlns="{NS}"><sheetData/></worksheet>"#)),
        ("xl/worksheets/_rels/sheet1.xml.rels", relationships(&[("pivot", "pivotTable", "../pivotTables/pivotTable1.xml")])),
        ("xl/pivotTables/pivotTable1.xml", format!(r#"<pivotTableDefinition xmlns="{NS}" name="RegionPivot" cacheId="42" dataCaption="Values"><location ref="D1:E3" firstHeaderRow="0" firstDataRow="1" firstDataCol="1"/><pivotFields count="2"><pivotField axis="axisPage"><items count="2"><item x="0"/><item x="1"/></items></pivotField><pivotField dataField="1"/></pivotFields>{page_fields}<dataFields count="1"><dataField name="Total" fld="1" subtotal="sum"/></dataFields><pivotTableStyleInfo name="PivotStyleLight16" showRowHeaders="1" showColHeaders="1" showRowStripes="0" showColStripes="0" showLastColumn="1"/>{tail}</pivotTableDefinition>"#)),
        ("xl/pivotTables/_rels/pivotTable1.xml.rels", relationships(&[("cache", "pivotCacheDefinition", "../pivotCache/pivotCacheDefinition1.xml")])),
        ("xl/pivotCache/pivotCacheDefinition1.xml", format!(r#"<pivotCacheDefinition xmlns="{NS}"><cacheSource type="worksheet"><worksheetSource ref="A1:B3" sheet="Pivot"/></cacheSource><cacheFields count="2"><cacheField name="Region"><sharedItems count="2"><s v="West"/><s v="East"/></sharedItems></cacheField><cacheField name="Revenue"><sharedItems containsNumber="1" count="2"><n v="10"/><n v="20"/></sharedItems></cacheField></cacheFields></pivotCacheDefinition>"#)),
    ];
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, xml) in parts {
        zip.start_file(name, SimpleFileOptions::default()).unwrap();
        zip.write_all(xml.as_bytes()).unwrap();
    }
    zip.finish().unwrap().into_inner()
}

fn pivot_xml(bytes: &[u8]) -> String {
    let mut zip = ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut xml = String::new();
    zip.by_name("xl/pivotTables/pivotTable1.xml")
        .unwrap()
        .read_to_string(&mut xml)
        .unwrap();
    xml
}

fn expanded_empty_elements(xml: &str) -> String {
    let mut reader = quick_xml::Reader::from_str(xml);
    let mut writer = quick_xml::Writer::new(Vec::new());
    loop {
        match reader.read_event().unwrap() {
            quick_xml::events::Event::Empty(start) => {
                writer
                    .write_event(quick_xml::events::Event::Start(start.borrow()))
                    .unwrap();
                writer
                    .write_event(quick_xml::events::Event::End(start.to_end()))
                    .unwrap();
            }
            quick_xml::events::Event::Eof => break,
            event => writer.write_event(event).unwrap(),
        }
    }
    String::from_utf8(writer.into_inner()).unwrap()
}

#[test]
fn native_page_fields_filters_and_hierarchy_usage_survive_two_saves() {
    let tail = format!("{FILTERS}{ROW_USAGE}{COLUMN_USAGE}");
    for eager in [true, false] {
        let mut book =
            reader::xlsx::read_reader(Cursor::new(fixture(PAGE_FIELDS, &tail)), eager).unwrap();
        book.sheet_mut(1)
            .unwrap()
            .cell_mut("A1")
            .set_value("Unrelated notes edit");
        for _ in 0..2 {
            let mut bytes = Vec::new();
            writer::xlsx::write_writer(&book, &mut bytes).unwrap();
            let xml = pivot_xml(&bytes);
            for expected in [PAGE_FIELDS, FILTERS, ROW_USAGE, COLUMN_USAGE] {
                assert!(
                    xml.contains(&expanded_empty_elements(expected)),
                    "missing {expected}: {xml}"
                );
            }
            let ordered = [
                "<pageFields",
                "<dataFields",
                "<pivotTableStyleInfo",
                "<filters count=",
                "<rowHierarchiesUsage",
                "<colHierarchiesUsage",
            ];
            let positions: Vec<_> = ordered.iter().map(|name| xml.find(name).unwrap()).collect();
            assert!(positions.windows(2).all(|pair| pair[0] < pair[1]), "{xml}");
            book = reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap();
            assert_eq!(book.sheet(1).unwrap().value("A1"), "Unrelated notes edit");
        }
    }
}

#[test]
fn duplicate_native_table_child_containers_are_explicit_import_errors() {
    for (pages, tail) in [
        (format!("{PAGE_FIELDS}{PAGE_FIELDS}"), String::new()),
        (String::new(), format!("{FILTERS}{FILTERS}")),
        (String::new(), format!("{ROW_USAGE}{ROW_USAGE}")),
        (String::new(), format!("{COLUMN_USAGE}{COLUMN_USAGE}")),
    ] {
        assert!(reader::xlsx::read_reader(Cursor::new(fixture(&pages, &tail)), true).is_err());
    }
}

#[test]
fn malformed_native_table_child_payloads_are_explicit_import_errors() {
    for (pages, tail) in [
        (r#"<pageFields count="1"><pageField/></pageFields>"#, ""),
        (
            r#"<pageFields count="1"><pageField fld="region"/></pageFields>"#,
            "",
        ),
        (
            r#"<pageFields count="2"><pageField fld="0"/></pageFields>"#,
            "",
        ),
        (r#"<pageFields count="0"/>"#, ""),
        (
            "",
            r#"<filters><filter fld="0" type="captionEqual" id="7"/></filters>"#,
        ),
        (
            "",
            r#"<filters><filter fld="0" type="notAFilter" id="7"><autoFilter/></filter></filters>"#,
        ),
        (
            "",
            r#"<filters><filter fld="0" type="captionEqual" id="-1"><autoFilter/></filter></filters>"#,
        ),
        (
            "",
            r#"<rowHierarchiesUsage><rowHierarchyUsage/></rowHierarchiesUsage>"#,
        ),
        (
            "",
            r#"<colHierarchiesUsage><colHierarchyUsage hierarchyUsage="first"/></colHierarchiesUsage>"#,
        ),
    ] {
        assert!(reader::xlsx::read_reader(Cursor::new(fixture(pages, tail)), true).is_err());
    }
}

#[test]
fn native_filter_allows_empty_auto_filter_and_optional_count() {
    let tail =
        r#"<filters><filter fld="0" type="captionEqual" id="7"><autoFilter/></filter></filters>"#;
    let book = reader::xlsx::read_reader(Cursor::new(fixture("", tail)), true).unwrap();
    let mut bytes = Vec::new();
    writer::xlsx::write_writer(&book, &mut bytes).unwrap();
    assert!(pivot_xml(&bytes).contains(&expanded_empty_elements(tail)));
}

#[cfg(feature = "serde")]
#[test]
fn retained_native_table_children_survive_serde_persistence() {
    let tail = format!("{FILTERS}{ROW_USAGE}{COLUMN_USAGE}");
    let book = reader::xlsx::read_reader(Cursor::new(fixture(PAGE_FIELDS, &tail)), true).unwrap();
    let definition = book.sheet(0).unwrap().pivot_tables()[0].pivot_table_definition();
    let persisted: umya_spreadsheet::PivotTableDefinition =
        serde_json::from_slice(&serde_json::to_vec(definition).unwrap()).unwrap();
    let xml = String::from_utf8(persisted.to_xml().unwrap()).unwrap();
    for expected in [PAGE_FIELDS, FILTERS, ROW_USAGE, COLUMN_USAGE] {
        assert!(
            xml.contains(&expanded_empty_elements(expected)),
            "missing {expected}: {xml}"
        );
    }
}

#[test]
fn prefixed_native_table_fields_and_retained_children_use_namespace_identity() {
    let tail = format!("{FILTERS}{ROW_USAGE}{COLUMN_USAGE}");
    let mut input = ZipArchive::new(Cursor::new(fixture(PAGE_FIELDS, &tail))).unwrap();
    let mut output = ZipWriter::new(Cursor::new(Vec::new()));
    for index in 0..input.len() {
        let mut part = input.by_index(index).unwrap();
        let name = part.name().unwrap().into_owned();
        let mut xml = String::new();
        part.read_to_string(&mut xml).unwrap();
        if name == "xl/pivotTables/pivotTable1.xml" {
            xml = xml
                .replace('<', "<s:")
                .replace("<s:/", "</s:")
                .replace(&format!(r#"xmlns="{NS}""#), &format!(r#"xmlns:s="{NS}""#));
        }
        output
            .start_file(name, SimpleFileOptions::default())
            .unwrap();
        output.write_all(xml.as_bytes()).unwrap();
    }
    let mut book =
        reader::xlsx::read_reader(Cursor::new(output.finish().unwrap().into_inner()), true)
            .unwrap();
    book.sheet_mut(1)
        .unwrap()
        .cell_mut("A1")
        .set_value("Edited");
    let definition = book.sheet(0).unwrap().pivot_tables()[0].pivot_table_definition();
    assert_eq!(definition.pivot_fields().list().len(), 2);
    assert_eq!(definition.data_fields().list().len(), 1);
    let mut bytes = Vec::new();
    writer::xlsx::write_writer(&book, &mut bytes).unwrap();
    let reread = reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap();
    let definition = reread.sheet(0).unwrap().pivot_tables()[0].pivot_table_definition();
    assert_eq!(definition.pivot_fields().list().len(), 2);
    assert_eq!(definition.data_fields().list().len(), 1);
    assert_eq!(reread.sheet(1).unwrap().value("A1"), "Edited");
}

#[test]
fn expanded_native_location_and_style_elements_retain_attributes() {
    let mut input = ZipArchive::new(Cursor::new(fixture("", ""))).unwrap();
    let mut output = ZipWriter::new(Cursor::new(Vec::new()));
    for index in 0..input.len() {
        let mut part = input.by_index(index).unwrap();
        let name = part.name().unwrap().into_owned();
        let mut xml = String::new();
        part.read_to_string(&mut xml).unwrap();
        if name == "xl/pivotTables/pivotTable1.xml" {
            xml = xml.replace(r#"<location ref="D1:E3" firstHeaderRow="0" firstDataRow="1" firstDataCol="1"/>"#, r#"<location ref="D1:E3" firstHeaderRow="0" firstDataRow="1" firstDataCol="1"></location>"#)
                .replace(r#"<pivotTableStyleInfo name="PivotStyleLight16" showRowHeaders="1" showColHeaders="1" showRowStripes="0" showColStripes="0" showLastColumn="1"/>"#, r#"<pivotTableStyleInfo name="PivotStyleLight16" showRowHeaders="1" showColHeaders="1" showRowStripes="0" showColStripes="0" showLastColumn="1"></pivotTableStyleInfo>"#);
        }
        output
            .start_file(name, SimpleFileOptions::default())
            .unwrap();
        output.write_all(xml.as_bytes()).unwrap();
    }
    let mut book =
        reader::xlsx::read_reader(Cursor::new(output.finish().unwrap().into_inner()), true)
            .unwrap();
    let definition = book.sheet(0).unwrap().pivot_tables()[0].pivot_table_definition();
    assert_eq!(definition.location().reference(), "D1:E3");
    assert_eq!(definition.pivot_table_style().name(), "PivotStyleLight16");
    book.sheet_mut(1)
        .unwrap()
        .cell_mut("A1")
        .set_value("Edited");
    let mut bytes = Vec::new();
    writer::xlsx::write_writer(&book, &mut bytes).unwrap();
    let reread = reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap();
    let definition = reread.sheet(0).unwrap().pivot_tables()[0].pivot_table_definition();
    assert_eq!(definition.location().reference(), "D1:E3");
    assert_eq!(definition.pivot_table_style().name(), "PivotStyleLight16");
}

fn fixture_with_data_field(field: &str) -> Vec<u8> {
    fixture_with_data_fields(&format!("<dataFields count=\"1\">{field}</dataFields>"))
}

fn fixture_with_data_fields(fields: &str) -> Vec<u8> {
    let mut input = ZipArchive::new(Cursor::new(fixture("", ""))).unwrap();
    let mut output = ZipWriter::new(Cursor::new(Vec::new()));
    for index in 0..input.len() {
        let mut part = input.by_index(index).unwrap();
        let name = part.name().unwrap().into_owned();
        let mut xml = String::new();
        part.read_to_string(&mut xml).unwrap();
        if name == "xl/pivotTables/pivotTable1.xml" {
            xml = xml.replace(r#"<dataFields count="1"><dataField name="Total" fld="1" subtotal="sum"/></dataFields>"#, fields);
        }
        output
            .start_file(name, SimpleFileOptions::default())
            .unwrap();
        output.write_all(xml.as_bytes()).unwrap();
    }
    output.finish().unwrap().into_inner()
}

#[test]
fn empty_present_data_fields_are_rejected_but_absent_collections_are_valid() {
    for fields in [
        "<dataFields/>",
        "<dataFields count=\"0\"/>",
        "<dataFields count=\"0\"></dataFields>",
    ] {
        assert!(
            reader::xlsx::read_reader(Cursor::new(fixture_with_data_fields(fields)), true).is_err(),
            "empty required-child container must not silently disappear: {fields}"
        );
    }
    let book = reader::xlsx::read_reader(Cursor::new(fixture_with_data_fields("")), true).unwrap();
    assert!(
        book.sheet(0).unwrap().pivot_tables()[0]
            .pivot_table_definition()
            .data_fields()
            .list()
            .is_empty()
    );
}

#[test]
fn omitted_data_field_reference_defaults_do_not_overwrite_explicit_zero() {
    for (attributes, expected_field, expected_item) in [
        ("", -1, 1_048_832),
        ("baseField=\"0\" baseItem=\"0\"", 0, 0),
    ] {
        let book = reader::xlsx::read_reader(
            Cursor::new(fixture_with_data_field(&format!(
                "<dataField fld=\"1\" {attributes}/>"
            ))),
            true,
        )
        .unwrap();
        let field = &book.sheet(0).unwrap().pivot_tables()[0]
            .pivot_table_definition()
            .data_fields()
            .list()[0];
        assert_eq!(field.base_fie_id(), expected_field);
        assert_eq!(field.base_item(), expected_item);
        let mut bytes = Vec::new();
        writer::xlsx::write_writer(&book, &mut bytes).unwrap();
        let xml = pivot_xml(&bytes);
        if attributes.is_empty() {
            assert!(
                !xml.contains("baseField="),
                "absence must stay absent: {xml}"
            );
            assert!(
                !xml.contains("baseItem="),
                "absence must stay absent: {xml}"
            );
        } else {
            assert!(xml.contains("baseField=\"0\""));
            assert!(xml.contains("baseItem=\"0\""));
        }
    }
}

#[test]
fn native_data_field_calculation_and_number_format_survive_lazy_save() {
    let mut book = reader::xlsx::read_reader(Cursor::new(fixture_with_data_field(r#"<dataField name="Total" fld="1" subtotal="sum" showDataAs="percentOfRow" numFmtId="4"/>"#)), false).unwrap();
    book.sheet_mut(1)
        .unwrap()
        .cell_mut("A1")
        .set_value("Edited elsewhere");
    let mut bytes = Vec::new();
    writer::xlsx::write_writer(&book, &mut bytes).unwrap();
    let xml = pivot_xml(&bytes);
    assert!(xml.contains(r#"showDataAs="percentOfRow""#), "{xml}");
    assert!(xml.contains(r#"numFmtId="4""#), "{xml}");
}

#[test]
fn native_data_field_with_extensions_survives_lazy_save() {
    let mut book = reader::xlsx::read_reader(Cursor::new(fixture_with_data_field(r#"<dataField name="Total" fld="1" subtotal="sum"><extLst><ext uri="urn:authored-data-field"><future:payload xmlns:future="urn:authored-pivot" value="preserved"/></ext></extLst></dataField>"#)), false).unwrap();
    book.sheet_mut(1)
        .unwrap()
        .cell_mut("A1")
        .set_value("Edited elsewhere");
    let mut bytes = Vec::new();
    writer::xlsx::write_writer(&book, &mut bytes).unwrap();
    let reread = reader::xlsx::read_reader(Cursor::new(&bytes), true).unwrap();
    assert_eq!(
        reread.sheet(0).unwrap().pivot_tables()[0]
            .pivot_table_definition()
            .data_fields()
            .list()
            .len(),
        1
    );
    assert!(pivot_xml(&bytes).contains("urn:authored-data-field"));
}

#[test]
fn malformed_native_data_fields_return_errors_instead_of_panicking() {
    for field in [
        r#"<dataField fld="invalid"/>"#,
        r#"<dataField fld="1" baseField="2147483648"/>"#,
        r#"<dataField fld="1" baseItem="-1"/>"#,
        r#"<dataField fld="1" showDataAs="invented"/>"#,
        r#"<dataField fld="1" subtotal="invented"/>"#,
        r#"<dataField fld="1" numFmtId="4294967296"/>"#,
        r#"<dataField fld="1"><extLst/><extLst/></dataField>"#,
        r#"<dataField fld="1"><unknown/></dataField>"#,
    ] {
        let result = reader::xlsx::read_reader(Cursor::new(fixture_with_data_field(field)), true);
        assert!(
            result.is_err(),
            "invalid native data field was accepted: {field}"
        );
    }
}
