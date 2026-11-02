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
        write_end_tag,
        write_new_line,
        write_start_tag,
    },
};
use crate::{
    helper::const_str::{
        MC_IGNORABLE_WB,
        MC_NS,
        PKG_WORKBOOK,
        REL_OFC_NS,
        SHEET_MAIN_NS,
        SHEET_MS_REVISION_NS,
        X15_NS,
        XR2_NS,
        XR6_NS,
        XR10_NS,
    },
    structs::{
        SlicerListKind,
        Workbook,
        WriterManager,
    },
};

pub(crate) fn write<W: io::Seek + io::Write>(
    wb: &Workbook,
    writer_mng: &mut WriterManager<W>,
) -> Result<(), XlsxError> {
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

    // workbook
    write_start_tag(
        &mut writer,
        "workbook",
        vec![
            ("xmlns", SHEET_MAIN_NS).into(),
            ("xmlns:r", REL_OFC_NS).into(),
            ("xmlns:mc", MC_NS).into(),
            ("xmlns:x15", X15_NS).into(),
            ("xmlns:xr", SHEET_MS_REVISION_NS).into(),
            ("xmlns:xr2", XR2_NS).into(),
            ("xmlns:xr6", XR6_NS).into(),
            ("xmlns:xr10", XR10_NS).into(),
            ("mc:Ignorable", MC_IGNORABLE_WB).into(),
        ],
        false,
    );

    // fileVersion
    write_start_tag(
        &mut writer,
        "fileVersion",
        vec![
            ("appName", "xl").into(),
            ("lastEdited", "5").into(),
            ("lowestEdited", "4").into(),
            ("rupBuild", "9302").into(),
        ],
        true,
    );

    // workbookPr
    let mut attributes: crate::structs::AttrCollection = Vec::new();
    if wb.date1904() {
        attributes.push(("date1904", "1").into());
    }
    attributes.push(("filterPrivacy", "1").into());
    // attributes.push(("defaultThemeVersion", "124226").into());
    if wb.has_macros() {
        attributes.push(("codeName", wb.code_name().unwrap_or("ThisWorkbook")).into());
    }
    write_start_tag(&mut writer, "workbookPr", attributes, true);

    // workbookProtection
    if let Some(v) = wb.workbook_protection() {
        v.write_to(&mut writer);
    }

    // bookViews
    write_start_tag(&mut writer, "bookViews", vec![], false);

    // workbookView
    wb.workbook_view().write_to(&mut writer);

    write_end_tag(&mut writer, "bookViews");

    // // sheets
    write_start_tag(&mut writer, "sheets", vec![], false);

    let mut index = 1;
    for worksheet in wb.sheet_collection_no_check() {
        let mut attributes: crate::structs::AttrCollection = Vec::new();
        let id = worksheet.sheet_id();
        let r_id = format!("rId{index}");
        attributes.push(("name", worksheet.name()).into());
        attributes.push(("sheetId", id).into());
        attributes.push(("r:id", &r_id).into());
        if worksheet.has_state() {
            attributes.push(("state", worksheet.state_str()).into());
        }

        // sheet
        write_start_tag(&mut writer, "sheet", attributes, true);

        index += 1;
    }
    write_end_tag(&mut writer, "sheets");

    // definedNames
    if wb.has_defined_names() {
        write_start_tag(&mut writer, "definedNames", vec![], false);

        for defined_name in wb.defined_names() {
            defined_name.write_to(&mut writer);
        }
        for sheet in wb.sheet_collection_no_check() {
            for defined_name in sheet.defined_names() {
                defined_name.write_to(&mut writer);
            }
        }

        write_end_tag(&mut writer, "definedNames");
    }

    // Preserve iterative calculation settings for intentional circular
    // formulas.
    let iterate_count_str;
    let iterate_delta_str;
    let mut calc_attrs: crate::structs::AttrCollection = vec![("calcId", "122211").into()];
    // A low calcId alone does not guarantee recalculation. Set fullCalcOnLoad
    // only when requested, so Excel replaces missing or stale formula caches.
    if wb.full_calc_on_load() {
        calc_attrs.push(("fullCalcOnLoad", "1").into());
    }
    if wb.iterate_enabled() {
        calc_attrs.push(("iterate", "1").into());
        if let Some((count, delta)) = wb.iterative_calc() {
            iterate_count_str = count.to_string();
            iterate_delta_str = delta.to_string();
            calc_attrs.push(("iterateCount", &iterate_count_str).into());
            calc_attrs.push(("iterateDelta", &iterate_delta_str).into());
        }
    }
    write_start_tag(&mut writer, "calcPr", calc_attrs, true);

    // pivotCaches
    let pivot_cache_definition_collection = wb.pivot_caches();
    let retained_ids = super::workbook_rels::retained_pivot_cache_relationship_ids(wb);
    let pivot_caches: Vec<_> = pivot_cache_definition_collection
        .iter()
        .enumerate()
        .filter(|(_, (_, id, _))| {
            !retained_ids
                .iter()
                .any(|(_, known, _)| known.to_string() == *id)
        })
        .collect();
    if wb.cell_metadata.is_some() {
        index += 1;
    }
    if !pivot_caches.is_empty() {
        write_start_tag(&mut writer, "pivotCaches", vec![], false);
        for (offset, (_, val2, _)) in pivot_caches {
            let r_id = format!("rId{}", index + offset);
            write_start_tag(
                &mut writer,
                "pivotCache",
                vec![("cacheId", val2).into(), ("r:id", &r_id).into()],
                true,
            );
        }
        write_end_tag(&mut writer, "pivotCaches");
    }

    // extLst: slicer cache lists and the other source extensions
    let mut pivot_ids = Vec::new();
    let mut table_ids = Vec::new();
    for (kind, id) in super::workbook_rels::slicer_cache_relationship_ids(wb) {
        match kind {
            SlicerListKind::Pivot => pivot_ids.push(id),
            SlicerListKind::Table => table_ids.push(id),
            // Timeline caches are listed by their own relationship ids.
            SlicerListKind::Timeline => {}
        }
    }
    crate::structs::slicer::write_workbook_extensions(
        &mut writer,
        &wb.extensions,
        &pivot_ids,
        &table_ids,
        &super::workbook_rels::timeline_cache_relationship_ids(wb),
        &retained_ids,
        &super::workbook_rels::carried_relationship_ids(wb),
    )?;

    write_end_tag(&mut writer, "workbook");

    writer_mng.add_writer(PKG_WORKBOOK, writer)
}
