use crate::{
    PivotTable,
    XlsxError,
    structs::{
        PivotCacheDefinition,
        raw::RawFile,
    },
};

/// Read the pivot caches Excel keeps without a pivot table for slicers
/// (`x14:pivotCaches`) and timelines (`x15:timelineCachePivotCaches`).
pub(crate) fn read_retained_caches<R: std::io::Read + std::io::Seek>(
    archive: &mut zip::read::ZipArchive<R>,
    book: &mut crate::Workbook,
    relationships: &[(String, String, String)],
    external_ids: &[String],
) -> Result<(), XlsxError> {
    use crate::{
        helper::const_str::{
            PIVOT_CACHE_DEF_NS,
            PIVOT_CACHE_REC_NS,
            REL_NS,
        },
        structs::slicer::WorkbookExtension,
        xml::{
            direct_children,
            expect,
            required,
            xml_error,
        },
    };
    let references: Vec<_> = book
        .extensions
        .iter()
        .filter_map(|entry| match entry {
            WorkbookExtension::RetainedPivotCaches(_, caches) => Some(caches.as_slice()),
            _ => None,
        })
        .flatten()
        .cloned()
        .collect();
    for (id, relationship) in references {
        let (_, kind, target) = relationships
            .iter()
            .find(|(known, ..)| *known == relationship)
            .ok_or_else(|| {
                xml_error(format!(
                    "retained pivot cache relationship {relationship} does not exist"
                ))
            })?;
        if kind != PIVOT_CACHE_DEF_NS || external_ids.contains(&relationship) {
            return Err(xml_error(format!(
                "invalid retained pivot cache relationship {relationship}"
            )));
        }
        let path = super::driver::join_paths("xl", target);
        let mut bytes = Vec::new();
        std::io::Read::read_to_end(&mut super::driver::zip_by_name(archive, &path)?, &mut bytes)?;
        let mut definition = PivotCacheDefinition::from_xml(&bytes).map_err(xml_error)?;
        if !definition.id().is_empty() {
            let rels_path = crate::structs::drawing_parts::relationships_path(&path)?;
            let mut bytes = Vec::new();
            std::io::Read::read_to_end(
                &mut super::driver::zip_by_name(archive, &rels_path)?,
                &mut bytes,
            )?;
            let rels = crate::xml::root(&bytes).map_err(xml_error)?;
            expect(&rels, REL_NS, "Relationships").map_err(xml_error)?;
            let mut records = None;
            for rel in direct_children(&rels).map_err(xml_error)? {
                expect(&rel, REL_NS, "Relationship").map_err(xml_error)?;
                if required(&rel, "Id").map_err(xml_error)? != definition.id() {
                    return Err(xml_error("unsupported retained pivot cache relationship"));
                }
                if records.is_some()
                    || required(&rel, "Type").map_err(xml_error)? != PIVOT_CACHE_REC_NS
                    || crate::xml::attr(&rel, "TargetMode").is_some_and(|mode| mode != "Internal")
                {
                    return Err(xml_error(
                        "invalid retained pivot cache records relationship",
                    ));
                }
                let directory = path.rsplit_once('/').map_or("", |(directory, _)| directory);
                let target = required(&rel, "Target").map_err(xml_error)?;
                let record_path = super::driver::join_paths(directory, &target);
                let mut data = Vec::new();
                std::io::Read::read_to_end(
                    &mut super::driver::zip_by_name(archive, &record_path)?,
                    &mut data,
                )?;
                records = Some(data);
            }
            definition.set_raw_cache_records(
                records.ok_or_else(|| {
                    xml_error("missing retained pivot cache records relationship")
                })?,
            );
        }
        if book
            .pivot_cache_bindings()
            .iter()
            .any(|(known, _)| *known == id.to_string())
        {
            return Err(xml_error(format!("duplicate workbook pivot cache {id}")));
        }
        book.add_pivot_caches((relationship, id.to_string(), target.clone()));
        book.pivot_orphans.push((id, definition));
    }
    Ok(())
}

pub(crate) fn read(
    raw_file: &RawFile,
    pivot_table: &mut PivotTable,
    raw_cache_records: Option<&RawFile>,
) -> Result<(), XlsxError> {
    let mut pivot_cache_def =
        PivotCacheDefinition::from_xml(raw_file.file_data()).map_err(crate::xml::xml_error)?;

    // Preserve raw cache records if available from the template.
    if let Some(records_file) = raw_cache_records {
        pivot_cache_def.set_raw_cache_records(records_file.file_data().to_vec());
    }

    pivot_table.set_pivot_cache_definition(pivot_cache_def);
    Ok(())
}
