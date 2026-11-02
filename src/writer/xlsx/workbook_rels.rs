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
    driver::{
        make_file_from_writer,
        write_end_tag,
        write_new_line,
        write_start_tag,
    },
};
use crate::{
    helper::const_str::{
        JSA_PROJECT_NS,
        PERSION_NS,
        PIVOT_CACHE_DEF_NS,
        PKG_WORKBOOK_RELS,
        REL_NS,
        SHARED_STRINGS_NS,
        SLICER_CACHE_REL_NS,
        STYLES_NS,
        THEME_NS,
        TIMELINE_CACHE_REL_NS,
        VBA_PROJECT_NS,
        WORKSHEET_NS,
    },
    structs::{
        RetainedPivotCacheList,
        SlicerListKind,
        Workbook,
        WorkbookExtension,
        WriterManager,
    },
};

/// Relationship ids (and their kinds) of the workbook slicer caches, in cache
/// order. The caches follow the pivot cache relationships, so the workbook XML
/// can reference them by the numbers computed here.
pub(crate) fn slicer_cache_relationship_ids(wb: &Workbook) -> Vec<(SlicerListKind, String)> {
    let first = 1
        + wb.sheet_collection_no_check().len()
        + usize::from(wb.cell_metadata.is_some())
        + wb.pivot_caches().len();
    wb.slicer_caches()
        .iter()
        .enumerate()
        .map(|(offset, cache)| {
            (
                SlicerListKind::of_cache(cache),
                format!("rId{}", first + offset),
            )
        })
        .collect()
}

/// Relationship ids of the workbook timeline caches, in cache order. They
/// follow the slicer cache relationships.
pub(crate) fn timeline_cache_relationship_ids(wb: &Workbook) -> Vec<String> {
    let first = 1
        + wb.sheet_collection_no_check().len()
        + usize::from(wb.cell_metadata.is_some())
        + wb.pivot_caches().len()
        + wb.slicer_caches().len();
    (first..first + wb.timeline_caches().len())
        .map(|id| format!("rId{id}"))
        .collect()
}

/// The retained pivot caches with the extension list that declares each,
/// their workbook cache id and their relationship id.
pub(crate) fn retained_pivot_cache_relationship_ids(
    wb: &Workbook,
) -> Vec<(RetainedPivotCacheList, u32, String)> {
    let first = 1 + wb.sheet_collection_no_check().len() + usize::from(wb.cell_metadata.is_some());
    wb.pivot_caches()
        .iter()
        .enumerate()
        .filter_map(|(offset, (_, id, _))| {
            let (id, definition) = wb
                .pivot_orphans
                .iter()
                .find(|(known, _)| known.to_string() == *id)?;
            Some((
                wb.retained_pivot_cache_list(definition)?,
                *id,
                format!("rId{}", first + offset),
            ))
        })
        .collect()
}

/// Relationship ids of the parts the preserved workbook extensions carry, one
/// list per extension in order. They follow the timeline cache relationships.
pub(crate) fn carried_relationship_ids(wb: &Workbook) -> Vec<Vec<String>> {
    let mut next = 1
        + wb.sheet_collection_no_check().len()
        + usize::from(wb.cell_metadata.is_some())
        + wb.pivot_caches().len()
        + wb.slicer_caches().len()
        + wb.timeline_caches().len();
    let mut lists = Vec::new();
    for entry in &wb.extensions {
        if let WorkbookExtension::Other(extension) = entry {
            lists.push(
                (next..next + extension.parts.len())
                    .map(|id| format!("rId{id}"))
                    .collect(),
            );
            next += extension.parts.len();
        }
    }
    lists
}

pub(crate) fn write<W: io::Seek + io::Write>(
    wb: &Workbook,
    has_shared_string_table: bool,
    carried: &[super::slicer::CarriedRelationship],
    writer_mng: &mut WriterManager<W>,
) -> Result<(), XlsxError> {
    let is_light = writer_mng.get_is_light();
    let mut writer = Writer::new(io::Cursor::new(Vec::new()));
    // XML header
    writer
        .write_event(Event::Decl(BytesDecl::new(
            "1.0",
            Some("UTF-8"),
            Some("yes"),
        )))
        .unwrap();
    write_new_line(&mut writer);

    // relationships
    let root_tag_name = "Relationships";
    let attributes: crate::structs::AttrCollection = vec![("xmlns", REL_NS).into()];
    write_start_tag(&mut writer, root_tag_name, attributes, false);

    let mut index = 1;

    // relationships worksheet
    for _ in wb.sheet_collection_no_check() {
        let path_str = format!("worksheets/sheet{index}.xml");
        write_relationship(&mut writer, &index.to_string(), WORKSHEET_NS, &path_str, "");
        index += 1;
    }

    if wb.cell_metadata.is_some() {
        write_relationship(
            &mut writer,
            &index.to_string(),
            crate::structs::cell_metadata::REL,
            "metadata.xml",
            "",
        );
        index += 1;
    }

    // relationships pivot_cache_definition
    for (_, _, pivot_cache_definition) in wb.pivot_caches() {
        write_relationship(
            &mut writer,
            &index.to_string(),
            PIVOT_CACHE_DEF_NS,
            &pivot_cache_definition,
            "",
        );
        index += 1;
    }

    // relationships slicer caches
    for (offset, _) in wb.slicer_caches().iter().enumerate() {
        write_relationship(
            &mut writer,
            &index.to_string(),
            SLICER_CACHE_REL_NS,
            &super::slicer::cache_target(offset),
            "",
        );
        index += 1;
    }

    // relationships timeline caches
    for (offset, _) in wb.timeline_caches().iter().enumerate() {
        write_relationship(
            &mut writer,
            &index.to_string(),
            TIMELINE_CACHE_REL_NS,
            &super::slicer::timeline_cache_target(offset),
            "",
        );
        index += 1;
    }

    // relationships of the preserved extension parts
    for relationship in carried {
        write_relationship(
            &mut writer,
            &index.to_string(),
            &relationship.relationship_type,
            &relationship.target,
            if relationship.external {
                "External"
            } else {
                ""
            },
        );
        index += 1;
    }

    // relationship styles.xml
    write_relationship(&mut writer, &index.to_string(), STYLES_NS, "styles.xml", "");
    index += 1;

    // relationship theme/theme1.xml
    write_relationship(
        &mut writer,
        &index.to_string(),
        THEME_NS,
        "theme/theme1.xml",
        "",
    );
    index += 1;

    // relationship sharedStrings.xml
    if has_shared_string_table {
        write_relationship(
            &mut writer,
            &index.to_string(),
            SHARED_STRINGS_NS,
            "sharedStrings.xml",
            "",
        );
        index += 1;
    }

    // relationships for vbaProject if needed
    if wb.has_macros() {
        write_relationship(
            &mut writer,
            &index.to_string(),
            VBA_PROJECT_NS,
            "vbaProject.bin",
            "",
        );
        index += 1;
    }

    // relationships for jsaProject if needed
    if wb.has_jsa_macros() {
        write_relationship(
            &mut writer,
            &index.to_string(),
            JSA_PROJECT_NS,
            "jsaProject.bin",
            "",
        );
        index += 1;
    }

    // relationship for persion
    if wb.has_threaded_comments() {
        write_relationship(
            &mut writer,
            &index.to_string(),
            PERSION_NS,
            "persons/person.xml",
            "",
        );
        index += 1;
    }

    if wb.connections.is_some() {
        write_relationship(
            &mut writer,
            &index.to_string(),
            crate::structs::cell_metadata::CONNECTIONS_REL,
            "connections.xml",
            "",
        );
    }

    write_end_tag(&mut writer, root_tag_name);
    make_file_from_writer(
        PKG_WORKBOOK_RELS,
        writer_mng.get_arv_mut(),
        writer,
        None,
        is_light,
    )?;
    Ok(())
}

fn write_relationship(
    writer: &mut Writer<io::Cursor<Vec<u8>>>,
    p_id: &str,
    p_type: &str,
    p_target: &str,
    p_target_mode: &str,
) {
    let tag_name = "Relationship";
    let mut attributes: crate::structs::AttrCollection = Vec::new();
    let r_id = format!("rId{p_id}");
    attributes.push(("Id", &r_id).into());
    attributes.push(("Type", p_type).into());
    attributes.push(("Target", p_target).into());
    if !p_target_mode.is_empty() {
        attributes.push(("TargetMode", p_target_mode).into());
    }
    write_start_tag(writer, tag_name, attributes, true);
}
