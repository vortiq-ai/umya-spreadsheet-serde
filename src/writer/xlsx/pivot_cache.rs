use std::io;

use quick_xml::{
    Writer,
    events::{
        BytesDecl,
        Event,
    },
};

use super::{
    XlsxError,
    driver::write_new_line,
};
use crate::structs::{
    PivotCacheDefinition,
    Workbook,
    Worksheet,
    WriterManager,
};

/// Whether a sheet that was not read has a relationship of `relationship_type`.
fn raw_sheet_has_relationship(
    sheet: &Worksheet,
    relationship_type: &str,
) -> Result<bool, XlsxError> {
    Ok(!sheet.is_deserialized()
        && sheet
            .raw_data_of_worksheet()
            .worksheet_relationships()?
            .is_some_and(|rels| {
                rels.relationship_list()
                    .iter()
                    .any(|rel| rel.get_type() == relationship_type)
            }))
}

/// The workbook with every sheet the package needs to describe consistently
/// read: sheets with pivot tables (which declare the workbook pivot caches),
/// sheets a slicer cache links to and sheets with slicers (which the slicer
/// bindings are validated against). Every writer prepares the workbook here.
pub(crate) fn prepare_workbook(wb: &Workbook) -> Result<std::borrow::Cow<'_, Workbook>, XlsxError> {
    let mut has_pivots = false;
    let mut read_sheets = Vec::new();
    for (index, sheet) in wb.sheet_collection_no_check().iter().enumerate() {
        let pivots = !sheet.pivot_tables().is_empty()
            || raw_sheet_has_relationship(sheet, crate::helper::const_str::PIVOT_TABLE_NS)?;
        let linked = !sheet.is_deserialized()
            && wb
                .slicer_caches()
                .iter()
                .flat_map(crate::structs::SlicerCache::linked_pivots)
                .any(|link| sheet.sheet_id().parse::<u32>() == Ok(link.sheet_id));
        if pivots
            || linked
            || raw_sheet_has_relationship(sheet, crate::helper::const_str::SLICER_REL_NS)?
        {
            read_sheets.push(index);
        }
        has_pivots |= pivots;
    }
    if read_sheets.is_empty() {
        return Ok(std::borrow::Cow::Borrowed(wb));
    }
    let mut prepared = wb.clone();
    for index in read_sheets {
        prepared.try_read_sheet(index)?;
    }
    if !has_pivots {
        return Ok(std::borrow::Cow::Owned(prepared));
    }
    let declared = declared_caches(&prepared)?;
    let caches = declared
        .order
        .iter()
        .enumerate()
        .map(|(index, id)| {
            (
                String::new(),
                id.clone(),
                format!("pivotCache/pivotCacheDefinition{}.xml", index + 1),
            )
        })
        .collect();
    prepared.set_pivot_cache_collection(caches);
    Ok(std::borrow::Cow::Owned(prepared))
}

/// The pivot caches the pivot tables of the workbook use: their ids in the
/// order the workbook declares them (the order of the sheets and of the pivot
/// tables on them) and the hash of the one definition each id has.
#[derive(Default)]
pub(crate) struct DeclaredCaches {
    pub(crate) order:  Vec<String>,
    pub(crate) hashes: std::collections::HashMap<String, String>,
}

pub(crate) fn declared_caches(wb: &Workbook) -> Result<DeclaredCaches, XlsxError> {
    let mut declared = DeclaredCaches::default();
    for sheet in wb.sheet_collection_no_check() {
        for pivot in sheet.pivot_tables() {
            let id = pivot.pivot_table_definition().cache_id().to_string();
            let hash = pivot.pivot_cache_definition().hash_code()?;
            match declared.hashes.get(&id) {
                Some(previous) if *previous != hash => {
                    return Err(crate::xml::xml_error(format!(
                        "conflicting definitions for pivot cache {id}"
                    )));
                }
                Some(_) => (),
                None => {
                    declared.order.push(id.clone());
                    declared.hashes.insert(id, hash);
                }
            }
        }
    }
    Ok(declared)
}

/// Check the pivot tables of a sheet about to be written against the caches
/// the workbook declared when the writer was prepared: a sheet edited in the
/// meantime may neither name a cache that was not declared nor change the
/// definition of one that was.
pub(crate) fn validate_sheet_caches(
    worksheet: &Worksheet,
    declared: &DeclaredCaches,
) -> Result<(), XlsxError> {
    for pivot in worksheet.pivot_tables() {
        let id = pivot.pivot_table_definition().cache_id().to_string();
        let Some(expected) = declared.hashes.get(&id) else {
            return Err(crate::xml::xml_error(format!(
                "sheet '{}': pivot table uses pivot cache {id}, which the workbook does not \
                 declare",
                worksheet.name()
            )));
        };
        if *expected != pivot.pivot_cache_definition().hash_code()? {
            return Err(crate::xml::xml_error(format!(
                "sheet '{}': the definition of pivot cache {id} changed after the workbook was \
                 prepared for writing",
                worksheet.name()
            )));
        }
    }
    Ok(())
}

pub(crate) fn write_relationship<W: io::Seek + io::Write>(
    path: &str,
    id: &str,
    kind: &str,
    target: &str,
    writer_mng: &mut WriterManager<W>,
) -> Result<(), XlsxError> {
    use crate::writer::driver::{
        write_end_tag,
        write_start_tag,
    };
    let mut writer = Writer::new(io::Cursor::new(Vec::new()));
    write_start_tag(
        &mut writer,
        "Relationships",
        vec![("xmlns", crate::helper::const_str::REL_NS).into()],
        false,
    );
    write_start_tag(
        &mut writer,
        "Relationship",
        vec![
            ("Id", id).into(),
            ("Type", kind).into(),
            ("Target", target).into(),
        ],
        true,
    );
    write_end_tag(&mut writer, "Relationships");
    writer_mng.add_writer(path, writer)
}

fn write_definition<W: io::Seek + io::Write>(
    definition: &PivotCacheDefinition,
    no: i32,
    writer_mng: &mut WriterManager<W>,
) -> Result<(), XlsxError> {
    let mut definition = definition.clone();
    if definition.has_raw_cache_records() {
        definition.set_id("rId1");
        write_relationship(
            &format!("xl/pivotCache/_rels/pivotCacheDefinition{no}.xml.rels"),
            "rId1",
            crate::helper::const_str::PIVOT_CACHE_REC_NS,
            &format!("pivotCacheRecords{no}.xml"),
            writer_mng,
        )?;
    } else if !definition.id().is_empty() {
        return Err(crate::xml::xml_error(
            "pivot cache records relationship has no records part",
        ));
    }
    let mut writer = Writer::new(io::Cursor::new(Vec::new()));
    writer.write_event(Event::Decl(BytesDecl::new(
        "1.0",
        Some("UTF-8"),
        Some("yes"),
    )))?;
    write_new_line(&mut writer);
    definition.write_to(&mut writer)?;
    writer_mng.add_file_at_pivot_cache(writer, no)?;
    if definition.has_raw_cache_records() {
        writer_mng.add_file_at_pivot_cache_records(definition.raw_cache_records(), no)?;
    }
    Ok(())
}

/// Write the pivot caches and pivot tables of one worksheet, returning the
/// pivot table numbers and the pivot cache numbers for its relationships.
/// The caches are numbered first so that the pivot tables find them; both
/// writers use this one order.
pub(crate) fn write_sheet_parts<W: io::Seek + io::Write>(
    worksheet: &Worksheet,
    writer_mng: &mut WriterManager<W>,
) -> Result<(Vec<String>, Vec<String>), XlsxError> {
    let pivot_cache_no_list = write(worksheet, writer_mng)?;
    let pivot_table_no_list = super::pivot_table::write(worksheet, writer_mng)?;
    Ok((pivot_table_no_list, pivot_cache_no_list))
}

pub(crate) fn write<W: io::Seek + io::Write>(
    worksheet: &Worksheet,
    writer_mng: &mut WriterManager<W>,
) -> Result<Vec<String>, XlsxError> {
    let mut pivot_cache_no_list = Vec::<String>::new();
    for pivot_table in worksheet.pivot_tables() {
        let id = pivot_table.pivot_table_definition().cache_id().to_string();
        let (find, no) = writer_mng.get_pivot_cache_no(&id);
        if find {
            continue;
        }
        write_definition(pivot_table.pivot_cache_definition(), no, writer_mng)?;

        pivot_cache_no_list.push(no.to_string());
    }
    Ok(pivot_cache_no_list)
}
