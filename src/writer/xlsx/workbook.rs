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
        let id = index.to_string();
        let r_id = format!("rId{index}");
        attributes.push(("name", worksheet.name()).into());
        attributes.push(("sheetId", &id).into());
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

    // calcPr
    //
    // `iterate` must be carried through. A workbook with a DELIBERATE circular
    // reference (LBO: debt -> interest -> cash flow -> debt) sets it, and Excel
    // converges the model. Writing a bare `calcId` dropped it, so Excel refused
    // to converge a file it had computed fine before the round trip — we
    // turned a gap into a broken workbook. Measured 2026-08-27 on a
    // customer model: source `<calcPr calcId="191028" fullCalcOnLoad="1"
    // iterate="1"/>`, our output `<calcPr calcId="122211"/>`.
    let iterate_count_str;
    let iterate_delta_str;
    let mut calc_attrs: crate::structs::AttrCollection = vec![("calcId", "122211").into()];
    // Missing caches are invisible to non-recalculating readers, while present
    // caches may be stale or produced by a different calculation engine. When
    // requested, ask Excel to replace them on open. (Our `calcId` is
    // deliberately low, but that alone is not a contract; `fullCalcOnLoad`
    // is.)
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
    if !pivot_cache_definition_collection.is_empty() {
        write_start_tag(&mut writer, "pivotCaches", vec![], false);
        for (_, val2, _) in pivot_cache_definition_collection {
            let r_id = format!("rId{index}");
            write_start_tag(
                &mut writer,
                "pivotCache",
                vec![("cacheId", &val2).into(), ("r:id", &r_id).into()],
                true,
            );
            index += 1;
        }
        write_end_tag(&mut writer, "pivotCaches");
    }

    write_end_tag(&mut writer, "workbook");

    writer_mng.add_writer(PKG_WORKBOOK, writer)
}
