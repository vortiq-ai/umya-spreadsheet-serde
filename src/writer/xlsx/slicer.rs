use std::io;

use super::XlsxError;
use crate::{
    helper::const_str::{
        PKG_SLICER_CACHES,
        PKG_SLICERS,
        SLICER_CACHE_TYPE,
        SLICER_TYPE,
    },
    structs::{
        Slicer,
        SlicerCache,
        SlicerKinds,
        SlicerListKind,
        Workbook,
        WorkbookExtension,
        Worksheet,
        WriterManager,
        package_extension::{
            ExtensionPart,
            split_part_name,
        },
    },
    xml::xml_error,
};

const XML_DECLARATION: &[u8] = b"<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n";

/// A relationship the writer must emit for a part or external target an
/// extension carries.
#[derive(Clone, Debug)]
pub(crate) struct CarriedRelationship {
    pub(crate) relationship_type: String,
    /// Target relative to the owner part; the URL for an external target.
    pub(crate) target:            String,
    pub(crate) external:          bool,
}

/// The parts written for one worksheet's extension lists.
#[derive(Clone, Debug, Default)]
pub(crate) struct SheetExtensionParts {
    pub(crate) pivot_slicers: Option<i32>,
    pub(crate) table_slicers: Option<i32>,
    /// Relationships of the preserved extensions, in the order of the
    /// extensions and of the parts each names.
    pub(crate) carried:       Vec<CarriedRelationship>,
}

fn with_declaration(body: Vec<u8>) -> Vec<u8> {
    let mut bytes = XML_DECLARATION.to_vec();
    bytes.extend(body);
    bytes
}

/// Write the parts the preserved extensions of one owner carry and return the
/// relationships to emit for them.
fn write_carried<'a, W: io::Seek + io::Write>(
    parts: impl Iterator<Item = &'a ExtensionPart>,
    owner: &str,
    writer_mng: &mut WriterManager<W>,
) -> Result<Vec<CarriedRelationship>, XlsxError> {
    let mut relationships = Vec::new();
    for part in parts {
        if let Some(target) = &part.external_target {
            relationships.push(CarriedRelationship {
                relationship_type: part.relationship_type.clone(),
                target:            target.clone(),
                external:          true,
            });
            continue;
        }
        let file = part.file.as_ref().ok_or_else(|| {
            xml_error(format!(
                "extension relationship '{}' has no part",
                part.source_id
            ))
        })?;
        let (directory, stem, extension) = split_part_name(&file.source_name);
        let name = writer_mng.allocate_part(directory, stem, extension);
        writer_mng.add_typed_bin(&name, &file.content_type, &file.data)?;
        relationships.push(CarriedRelationship {
            relationship_type: part.relationship_type.clone(),
            target:            crate::structs::drawing_parts::relative_target(owner, &name)?,
            external:          false,
        });
    }
    Ok(relationships)
}

/// Target of the slicer cache part written for the cache at `index`, relative
/// to the `xl` directory.
pub(crate) fn cache_target(index: usize) -> String {
    format!("slicerCaches/slicerCache{}.xml", index + 1)
}

/// Write every workbook slicer cache part and the parts the preserved
/// workbook extensions carry. Returns the relationships of the latter.
pub(crate) fn write_caches<W: io::Seek + io::Write>(
    wb: &Workbook,
    writer_mng: &mut WriterManager<W>,
) -> Result<Vec<CarriedRelationship>, XlsxError> {
    for (index, cache) in wb.slicer_caches().iter().enumerate() {
        let target = format!("{PKG_SLICER_CACHES}/slicerCache{}.xml", index + 1);
        let bytes = with_declaration(
            cache
                .to_xml()
                .map_err(|error| xml_error(format!("slicer cache '{}': {error}", cache.name)))?,
        );
        writer_mng.add_typed_bin(&target, SLICER_CACHE_TYPE, &bytes)?;
    }
    write_carried(
        wb.extensions
            .iter()
            .filter_map(|entry| match entry {
                WorkbookExtension::Other(extension) => Some(extension.parts.iter()),
                WorkbookExtension::SlicerCaches(_) => None,
            })
            .flatten(),
        "xl/workbook.xml",
        writer_mng,
    )
}

fn cache_of<'a>(wb: &'a Workbook, slicer: &Slicer) -> Result<&'a SlicerCache, XlsxError> {
    let wanted = slicer.cache.to_lowercase();
    wb.slicer_caches()
        .iter()
        .find(|cache| cache.name.to_lowercase() == wanted)
        .ok_or_else(|| {
            xml_error(format!(
                "unresolved slicer binding: slicer '{}' references unknown slicer cache '{}'",
                slicer.name, slicer.cache
            ))
        })
}

/// Split a worksheet's slicers by the kind of cache they bind to.
fn partition<'a>(
    wb: &Workbook,
    worksheet: &'a Worksheet,
) -> Result<(Vec<&'a Slicer>, Vec<&'a Slicer>), XlsxError> {
    let mut pivot = Vec::new();
    let mut table = Vec::new();
    for slicer in worksheet.slicers() {
        match SlicerListKind::of_cache(cache_of(wb, slicer)?) {
            SlicerListKind::Pivot => pivot.push(slicer),
            SlicerListKind::Table => table.push(slicer),
        }
    }
    Ok((pivot, table))
}

/// The slicer lists a worksheet needs.
pub(crate) fn kinds(wb: &Workbook, worksheet: &Worksheet) -> Result<SlicerKinds, XlsxError> {
    let (pivot, table) = partition(wb, worksheet)?;
    Ok(SlicerKinds {
        pivot: !pivot.is_empty(),
        table: !table.is_empty(),
    })
}

/// Write the slicer part(s) and the carried parts of one worksheet.
pub(crate) fn write_worksheet_parts<W: io::Seek + io::Write>(
    wb: &Workbook,
    worksheet: &Worksheet,
    writer_mng: &mut WriterManager<W>,
) -> Result<SheetExtensionParts, XlsxError> {
    let (pivot, table) = partition(wb, worksheet)?;
    let mut numbers = SheetExtensionParts::default();
    for (slicers, slot) in [
        (pivot, &mut numbers.pivot_slicers),
        (table, &mut numbers.table_slicers),
    ] {
        if slicers.is_empty() {
            continue;
        }
        let views: Vec<Slicer> = slicers.into_iter().cloned().collect();
        let bytes = with_declaration(Slicer::to_xml(&views).map_err(|error| {
            xml_error(format!("slicers of sheet '{}': {error}", worksheet.name()))
        })?);
        let name = writer_mng.allocate_part(PKG_SLICERS, "slicer", "xml");
        writer_mng.add_typed_bin(&name, SLICER_TYPE, &bytes)?;
        let number = name
            .trim_start_matches(&format!("{PKG_SLICERS}/slicer"))
            .trim_end_matches(".xml")
            .parse::<i32>()
            .map_err(|_| xml_error("invalid slicer part name"))?;
        *slot = Some(number);
    }
    numbers.carried = write_carried(
        worksheet.carried_parts(),
        "xl/worksheets/sheet.xml",
        writer_mng,
    )?;
    Ok(numbers)
}
