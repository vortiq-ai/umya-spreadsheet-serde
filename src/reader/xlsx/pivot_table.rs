use quick_xml::{
    Reader,
    Writer,
    events::Event,
};

use crate::{
    XlsxError,
    XlsxErrorKind,
    helper::const_str::{
        PIVOT_CACHE_DEF_NS,
        PIVOT_CACHE_REC_NS,
    },
    reader::xlsx::{
        chart_cache::SourceCacheSession,
        pivot_cache,
    },
    structs::{
        PivotTable,
        PivotTableDefinition,
        Worksheet,
        raw::{
            RawFile,
            RawWorksheet,
        },
    },
};

impl PivotTableDefinition {
    pub(crate) fn canonical_legacy_table_child(bytes: Vec<u8>) -> Result<Vec<u8>, XlsxError> {
        let mut canonical = Vec::new();
        std::io::Read::read_to_end(
            &mut crate::reader::spreadsheet_xml::SpreadsheetXml::open(std::io::Cursor::new(bytes))?,
            &mut canonical,
        )?;
        let mut reader = Reader::from_reader(canonical.as_slice());
        let mut events = Vec::new();
        loop {
            let event = reader.read_event()?;
            if matches!(event, Event::Eof) {
                break;
            }
            events.push(event.into_owned());
        }
        let mut writer = Writer::new(Vec::new());
        let mut index = 0;
        let mut depth = 0;
        while index < events.len() {
            if let Event::Start(start) = &events[index] {
                let mut end = index + 1;
                while matches!(
                    events.get(end),
                    Some(Event::Text(text)) if text.iter().all(u8::is_ascii_whitespace)
                ) {
                    end += 1;
                }
                if depth > 0
                    && matches!(
                        events.get(end),
                        Some(Event::End(tag)) if tag.name() == start.name()
                    )
                {
                    writer.write_event(Event::Empty(start.borrow()))?;
                    index = end + 1;
                    continue;
                }
                depth += 1;
            } else if matches!(events[index], Event::End(_)) {
                depth -= 1;
            }
            writer.write_event(events[index].borrow())?;
            index += 1;
        }
        Ok(writer.into_inner())
    }
}

#[allow(clippy::redundant_closure_for_method_calls)]
pub(crate) fn read(
    worksheet: &mut Worksheet,
    pivot_table_file: &RawFile,
    raw: &RawWorksheet,
    session: &SourceCacheSession,
) -> Result<(), XlsxError> {
    let mut pivot_table = PivotTable::default();
    pivot_table.set_pivot_table_definition(
        PivotTableDefinition::from_xml(pivot_table_file.file_data())
            .map_err(crate::xml::xml_error)?,
    );
    let rrs_list = raw
        .relationships_of_part(pivot_table_file.file_target())?
        .ok_or_else(|| crate::xml::xml_error("missing pivot table cache relationships"))?;
    let mut cache_definitions = rrs_list
        .relationship_list()
        .iter()
        .filter(|relationship| relationship.get_type() == PIVOT_CACHE_DEF_NS);
    let cache_definition = cache_definitions.next().ok_or_else(|| {
        XlsxError::with_message(
            XlsxErrorKind::NotFound,
            format!("relationship with type {PIVOT_CACHE_DEF_NS}"),
        )
    })?;
    if cache_definitions.next().is_some() {
        return Err(crate::xml::xml_error(
            "pivot table declares more than one pivot cache definition relationship",
        ));
    }
    let cache_file = cache_definition.raw_file();
    let table_cache_id = pivot_table.pivot_table_definition().cache_id().to_string();
    let workbook_cache_id = session
        .pivot_cache_id_of(cache_file.file_target())
        .ok_or_else(|| {
            crate::xml::xml_error(format!(
                "pivot cache definition part {} is not declared by the workbook pivotCaches",
                cache_file.file_target()
            ))
        })?;
    if workbook_cache_id != table_cache_id {
        return Err(crate::xml::xml_error(format!(
            "pivot table cacheId {table_cache_id} is bound to a cache the workbook declares as \
             cacheId {workbook_cache_id}"
        )));
    }
    pivot_cache::read(cache_file, &mut pivot_table, None)?;
    let records_id = pivot_table.pivot_cache_definition().id();
    if !records_id.is_empty() {
        let records = raw
            .relationships_of_part(cache_file.file_target())?
            .and_then(|rels| rels.find_relationship_by_rid(records_id))
            .filter(|rel| rel.get_type() == PIVOT_CACHE_REC_NS)
            .ok_or_else(|| {
                crate::xml::xml_error(format!(
                    "missing pivot cache records relationship {records_id}"
                ))
            })?;
        pivot_table
            .pivot_cache_definition_mut()
            .set_raw_cache_records(records.raw_file().file_data().to_vec());
    }

    worksheet.add_pivot_table(pivot_table);
    Ok(())
}
