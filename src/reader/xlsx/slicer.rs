use std::{
    collections::HashSet,
    io,
};

use super::{
    chart_cache::SourceCacheSession,
    driver,
};
use crate::{
    XlsxError,
    helper::const_str::{
        SLICER_CACHE_REL_NS,
        SLICER_REL_NS,
    },
    structs::{
        Slicer,
        SlicerCache,
        Workbook,
        WorkbookExtension,
        Worksheet,
        WorksheetExtension,
        drawing_parts::relationships_path,
        package_extension::{
            ExtensionFile,
            ExtensionPart,
            PreservedExtension,
        },
        raw::RawWorksheet,
    },
    xml::xml_error,
};

/// Read every workbook slicer cache listed in the workbook extension lists.
/// Each entry resolves through the workbook's own relationships by exact id
/// and relationship type.
pub(crate) fn read_workbook_slicer_caches<R: io::Read + io::Seek>(
    archive: &mut zip::read::ZipArchive<R>,
    book: &mut Workbook,
    workbook_relationships: &[(String, String, String)],
    external_ids: &[String],
) -> Result<(), XlsxError> {
    resolve_workbook_extension_parts(archive, book, workbook_relationships, external_ids)?;
    let references = std::mem::take(&mut book.slicer_refs);
    let mut used = HashSet::new();
    for reference in references {
        if !used.insert(reference.relationship_id.clone()) {
            return Err(xml_error(format!(
                "slicer cache relationship {} is listed more than once",
                reference.relationship_id
            )));
        }
        let (_, relationship_type, target) = workbook_relationships
            .iter()
            .find(|(id, ..)| *id == reference.relationship_id)
            .ok_or_else(|| {
                xml_error(format!(
                    "slicer cache relationship {} does not exist",
                    reference.relationship_id
                ))
            })?;
        if relationship_type != SLICER_CACHE_REL_NS {
            return Err(xml_error(format!(
                "relationship {} is not a slicer cache relationship",
                reference.relationship_id
            )));
        }
        let path = driver::join_paths("xl", target);
        let rels_path = relationships_path(&path)?;
        if archive.by_name(&rels_path).is_ok() {
            return Err(xml_error(format!(
                "slicer cache part {path} has relationships, which are not supported"
            )));
        }
        let mut bytes = Vec::new();
        io::Read::read_to_end(&mut driver::zip_by_name(archive, &path)?, &mut bytes)?;
        let cache = SlicerCache::from_xml(&bytes)
            .map_err(|error| xml_error(format!("slicer cache part {path}: {error}")))?;
        book.slicer_caches_mut().push(cache);
    }
    Ok(())
}

/// Resolve every relationship the preserved workbook extensions name through
/// the workbook relationships, keeping the referenced parts.
fn resolve_workbook_extension_parts<R: io::Read + io::Seek>(
    archive: &mut zip::read::ZipArchive<R>,
    book: &mut Workbook,
    workbook_relationships: &[(String, String, String)],
    external_ids: &[String],
) -> Result<(), XlsxError> {
    let content_types = book.package_content_types();
    for entry in &mut book.extensions {
        let WorkbookExtension::Other(extension) = entry else {
            continue;
        };
        let mut parts = Vec::new();
        for id in extension.relationship_ids() {
            let (_, relationship_type, target) = workbook_relationships
                .iter()
                .find(|(known, ..)| *known == id)
                .ok_or_else(|| {
                    xml_error(format!(
                        "workbook extension relationship {id} does not exist"
                    ))
                })?;
            if external_ids.contains(&id) {
                parts.push(ExtensionPart {
                    source_id:         id,
                    relationship_type: relationship_type.clone(),
                    external_target:   Some(target.clone()),
                    file:              None,
                });
                continue;
            }
            let path = driver::join_paths("xl", target);
            if archive.by_name(&relationships_path(&path)?).is_ok() {
                return Err(xml_error(format!(
                    "workbook extension part {path} has relationships, which are not supported"
                )));
            }
            let content_type = content_types.get(&path).ok_or_else(|| {
                xml_error(format!(
                    "workbook extension part {path} has no content type"
                ))
            })?;
            let mut data = Vec::new();
            io::Read::read_to_end(&mut driver::zip_by_name(archive, &path)?, &mut data)?;
            parts.push(ExtensionPart {
                source_id:         id,
                relationship_type: relationship_type.clone(),
                external_target:   None,
                file:              Some(ExtensionFile {
                    source_name: path,
                    content_type: content_type.to_owned(),
                    data,
                }),
            });
        }
        extension.parts = parts;
    }
    Ok(())
}

/// Resolve every relationship the preserved worksheet extensions name through
/// the worksheet part relationships, keeping the referenced parts.
fn resolve_worksheet_extension_parts(
    worksheet: &mut Worksheet,
    raw: &RawWorksheet,
    session: &SourceCacheSession,
) -> Result<(), XlsxError> {
    let extensions = worksheet
        .extensions
        .iter_mut()
        .filter_map(|entry| match entry {
            WorksheetExtension::Other(extension) => Some(extension),
            _ => None,
        });
    for extension in extensions {
        resolve_extension_parts(extension, raw, session)?;
    }
    for extension in &mut worksheet.preserved_elements {
        resolve_extension_parts(extension, raw, session)?;
    }
    for extension in worksheet.conditional_extension_entries_mut() {
        resolve_extension_parts(extension, raw, session)?;
    }
    Ok(())
}

/// Resolve the relationships one preserved extension names.
fn resolve_extension_parts(
    extension: &mut PreservedExtension,
    raw: &RawWorksheet,
    session: &SourceCacheSession,
) -> Result<(), XlsxError> {
    {
        let ids = extension.relationship_ids();
        if ids.is_empty() {
            return Ok(());
        }
        let relationships = raw
            .relationships_of_part(raw.worksheet_file().file_target())?
            .ok_or_else(|| {
                xml_error("worksheet extension names a relationship but the worksheet has none")
            })?;
        let mut parts = Vec::new();
        for id in ids {
            let relationship = relationships.find_relationship_by_rid(&id).ok_or_else(|| {
                xml_error(format!(
                    "worksheet extension relationship {id} does not exist"
                ))
            })?;
            if relationship.target_mode() == "External" {
                parts.push(ExtensionPart {
                    source_id:         id,
                    relationship_type: relationship.get_type().to_owned(),
                    external_target:   Some(relationship.target().to_owned()),
                    file:              None,
                });
                continue;
            }
            let part = relationship.raw_file();
            if raw.relationships_of_part(part.file_target())?.is_some() {
                return Err(xml_error(format!(
                    "worksheet extension part {} has relationships, which are not supported",
                    part.file_target()
                )));
            }
            let content_type = session.content_type(part.file_target()).ok_or_else(|| {
                xml_error(format!(
                    "worksheet extension part {} has no content type",
                    part.file_target()
                ))
            })?;
            parts.push(ExtensionPart {
                source_id:         id,
                relationship_type: relationship.get_type().to_owned(),
                external_target:   None,
                file:              Some(ExtensionFile {
                    source_name:  part.file_target().to_owned(),
                    content_type: content_type.to_owned(),
                    data:         part.file_data().to_vec(),
                }),
            });
        }
        extension.parts = parts;
    }
    Ok(())
}

/// Read the slicer parts listed in the extension lists of a worksheet and
/// resolve the parts of its other extensions. Each entry resolves through the
/// worksheet part relationships.
pub(crate) fn read_worksheet_slicers(
    worksheet: &mut Worksheet,
    raw: &RawWorksheet,
    session: &SourceCacheSession,
) -> Result<(), XlsxError> {
    resolve_worksheet_extension_parts(worksheet, raw, session)?;
    let references = std::mem::take(&mut worksheet.slicer_part_references);
    if references.is_empty() {
        return Ok(());
    }
    let relationships = raw
        .relationships_of_part(raw.worksheet_file().file_target())?
        .ok_or_else(|| xml_error("worksheet lists slicers but has no relationships"))?;
    let mut used = HashSet::new();
    for reference in references {
        if !used.insert(reference.relationship_id.clone()) {
            return Err(xml_error(format!(
                "slicer relationship {} is listed more than once",
                reference.relationship_id
            )));
        }
        let relationship = relationships
            .find_relationship_by_rid(&reference.relationship_id)
            .ok_or_else(|| {
                xml_error(format!(
                    "slicer relationship {} does not exist",
                    reference.relationship_id
                ))
            })?;
        if relationship.get_type() != SLICER_REL_NS || relationship.target_mode() == "External" {
            return Err(xml_error(format!(
                "relationship {} is not an internal slicer relationship",
                reference.relationship_id
            )));
        }
        let part = relationship.raw_file();
        if raw.relationships_of_part(part.file_target())?.is_some() {
            return Err(xml_error(format!(
                "slicer part {} has relationships, which are not supported",
                part.file_target()
            )));
        }
        let slicers = Slicer::from_xml(part.file_data())
            .map_err(|error| xml_error(format!("slicer part {}: {error}", part.file_target())))?;
        worksheet.slicers_mut().extend(slicers);
    }
    Ok(())
}
