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
        COMMENTS_NS,
        DRAWINGS_NS,
        HYPERLINK_NS,
        IMAGE_NS,
        OLE_OBJECT_NS,
        PACKAGE_NS,
        PIVOT_TABLE_NS,
        PKG_SHEET_RELS,
        PRINTER_SETTINGS_NS,
        REL_NS,
        SLICER_REL_NS,
        TABLE_NS,
        THREADED_COMMENT_NS,
        TIMELINE_REL_NS,
        VML_DRAWING_NS,
    },
    structs::{
        SlicerKinds,
        Worksheet,
        WriterManager,
    },
    xml::xml_error,
};

/// The relationship ids of one worksheet, allocated once. The worksheet XML
/// (`r:id` attributes) and the relationships part both read their ids from
/// here, so they cannot drift apart.
#[derive(Clone, Copy, Debug)]
pub(crate) struct RelationshipIds {
    /// First id of the external hyperlink relationships.
    pub(crate) hyperlinks:      i32,
    pub(crate) page_setup:      Option<i32>,
    pub(crate) drawing:         Option<i32>,
    pub(crate) legacy_drawing:  Option<i32>,
    pub(crate) header_footer:   Option<i32>,
    /// First id of the table relationships.
    pub(crate) tables:          i32,
    /// First id of the pivot table relationships.
    pub(crate) pivot_tables:    i32,
    /// First id of the OLE relationships: each object takes two ids, the
    /// embedded object and then its preview image.
    pub(crate) ole_objects:     i32,
    pub(crate) comments:        Option<i32>,
    pub(crate) threaded:        Option<i32>,
    /// First id of the relationships of the preserved extensions.
    pub(crate) extension_parts: i32,
    pub(crate) pivot_slicers:   Option<i32>,
    pub(crate) table_slicers:   Option<i32>,
    pub(crate) timelines:       Option<i32>,
}

impl RelationshipIds {
    pub(crate) fn of(worksheet: &Worksheet, slicers: SlicerKinds) -> Result<Self, XlsxError> {
        let mut next = 1i32;
        let mut take = |count: usize| -> Result<i32, XlsxError> {
            let first = next;
            next = i32::try_from(count)
                .ok()
                .and_then(|count| next.checked_add(count))
                .ok_or_else(|| {
                    xml_error("too many worksheet relationships for relationship ids")
                })?;
            Ok(first)
        };
        let external_hyperlinks = worksheet
            .hyperlink_collection()
            .iter()
            .filter(|(_, hyperlink)| !hyperlink.location())
            .count();
        let hyperlinks = take(external_hyperlinks)?;
        let page_setup = worksheet
            .page_setup()
            .object_data()
            .is_some()
            .then(|| take(1))
            .transpose()?;
        let drawing = worksheet
            .has_drawing_object()
            .then(|| take(1))
            .transpose()?;
        let legacy_drawing = worksheet
            .has_legacy_drawing()
            .then(|| take(1))
            .transpose()?;
        let header_footer = worksheet
            .has_header_footer_vml()
            .then(|| take(1))
            .transpose()?;
        let tables = take(worksheet.tables().len())?;
        let pivot_tables = take(worksheet.pivot_tables().len())?;
        let ole_objects = take(worksheet.ole_objects().ole_object().len() * 2)?;
        let comments = worksheet.has_comments().then(|| take(1)).transpose()?;
        let threaded = worksheet
            .has_threaded_comments()
            .then(|| take(1))
            .transpose()?;
        let extension_parts = take(worksheet.extension_relationship_count())?;
        let pivot_slicers = if slicers.pivot { Some(take(1)?) } else { None };
        let table_slicers = if slicers.table { Some(take(1)?) } else { None };
        let timelines = if slicers.timeline {
            Some(take(1)?)
        } else {
            None
        };
        Ok(Self {
            hyperlinks,
            page_setup,
            drawing,
            legacy_drawing,
            header_footer,
            tables,
            pivot_tables,
            ole_objects,
            comments,
            threaded,
            extension_parts,
            pivot_slicers,
            table_slicers,
            timelines,
        })
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn write<W: io::Seek + io::Write>(
    worksheet: &Worksheet,
    worksheet_no: &str,
    drawing_no: &str,
    vml_drawing_no: &str,
    header_footer_vml_no: &str,
    comment_no: &str,
    threaded_comment_no: &str,
    ole_object_no_list: &[String],
    excel_no_list: &[String],
    printer_settings_no: &str,
    table_no_list: &[String],
    pivot_table_no_list: &[String],
    pivot_cache_no_list: &[String],
    slicer_parts: &super::slicer::SheetExtensionParts,
    writer_mng: &mut WriterManager<W>,
) -> Result<(), XlsxError> {
    let mut is_write = false;

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
    write_start_tag(
        &mut writer,
        "Relationships",
        vec![("xmlns", REL_NS).into()],
        false,
    );

    let ids = RelationshipIds::of(
        worksheet,
        SlicerKinds {
            pivot:    slicer_parts.pivot_slicers.is_some(),
            table:    slicer_parts.table_slicers.is_some(),
            timeline: slicer_parts.timelines.is_some(),
        },
    )?;

    // Write hyperlink relationships
    let mut hyperlink_id = ids.hyperlinks;
    for (_, hyperlink) in worksheet.hyperlink_collection() {
        if !hyperlink.location() {
            is_write = write_relationship(
                &mut writer,
                &hyperlink_id.to_string(),
                HYPERLINK_NS,
                hyperlink.url(),
                "External",
            );
            hyperlink_id += 1;
        }
    }

    // write pageSetup
    if let Some(id) = ids.page_setup {
        let object_name = format!("printerSettings{printer_settings_no}.bin");
        is_write = write_relationship(
            &mut writer,
            &id.to_string(),
            PRINTER_SETTINGS_NS,
            format!("../printerSettings/{object_name}").as_str(),
            "",
        );
    }

    // write drawing relationships
    if let Some(id) = ids.drawing {
        is_write = write_relationship(
            &mut writer,
            &id.to_string(),
            DRAWINGS_NS,
            format!("../drawings/drawing{drawing_no}.xml").as_str(),
            "",
        );
    }

    // Write vmlDrawing relationship
    if let Some(id) = ids.legacy_drawing {
        is_write = write_relationship(
            &mut writer,
            &id.to_string(),
            VML_DRAWING_NS,
            format!("../drawings/vmlDrawing{vml_drawing_no}.vml").as_str(),
            "",
        );
    }

    // Write header/footer vmlDrawing relationship
    if let Some(id) = ids.header_footer {
        is_write = write_relationship(
            &mut writer,
            &id.to_string(),
            VML_DRAWING_NS,
            format!("../drawings/vmlDrawing{header_footer_vml_no}.vml").as_str(),
            "",
        );
    }

    // write table relationships
    for (id, table_no) in (ids.tables..).zip(table_no_list) {
        is_write = write_relationship(
            &mut writer,
            &id.to_string(),
            TABLE_NS,
            format!("../tables/table{table_no}.xml").as_str(),
            "",
        );
    }

    // write pivot table relationships
    for (id, pivot_table_no) in (ids.pivot_tables..).zip(pivot_table_no_list) {
        is_write = write_relationship(
            &mut writer,
            &id.to_string(),
            PIVOT_TABLE_NS,
            format!("../pivotTables/pivotTable{pivot_table_no}.xml").as_str(),
            "",
        );
    }

    // write pivot cache relationships (handled at workbook level, not worksheet
    // level) Pivot caches are referenced from the workbook, not individual
    // worksheets
    let _ = pivot_cache_no_list; // Suppress unused warning

    // Write ole_objects: each takes the id pair (object, preview image)
    let mut excel_no_list = excel_no_list.iter();
    let mut ole_object_no_list = ole_object_no_list.iter();
    for (index, ole_object) in worksheet.ole_objects().ole_object().iter().enumerate() {
        let object_id = ids.ole_objects + 2 * i32::try_from(index).map_err(xml_error)?;
        if ole_object.is_xlsx() {
            let excel_no = excel_no_list.next().unwrap();
            let object_name = format!("Microsoft_Excel_Worksheet{excel_no}.xlsx");
            write_relationship(
                &mut writer,
                &object_id.to_string(),
                PACKAGE_NS,
                format!("../embeddings/{object_name}").as_str(),
                "",
            );
        }
        if ole_object.is_bin() {
            let ole_object_no = ole_object_no_list.next().unwrap();
            let object_name = format!("oleObject{ole_object_no}.bin");
            write_relationship(
                &mut writer,
                &object_id.to_string(),
                OLE_OBJECT_NS,
                format!("../embeddings/{object_name}").as_str(),
                "",
            );
        }

        // Each object keeps the id pair (object, preview image) whether or
        // not it has a preview, so ids stay those the sheet XML names.
        if ole_object.has_preview_image() {
            let image_name = ole_object.embedded_object_properties().image().image_name();
            write_relationship(
                &mut writer,
                &(object_id + 1).to_string(),
                IMAGE_NS,
                format!("../media/{image_name}").as_str(),
                "",
            );
        }
        is_write = true;
    }

    // Write comments relationship
    if let Some(id) = ids.comments {
        is_write = write_relationship(
            &mut writer,
            &id.to_string(),
            COMMENTS_NS,
            format!("../comments{comment_no}.xml").as_str(),
            "",
        );
    }

    // Write threadedComment relationship
    if let Some(id) = ids.threaded {
        is_write = write_relationship(
            &mut writer,
            &id.to_string(),
            THREADED_COMMENT_NS,
            format!("../threadedComments/threadedComment{threaded_comment_no}.xml").as_str(),
            "",
        );
    }

    // Relationships of the preserved extensions, then the slicer lists; the
    // worksheet XML references them by the same ids.
    if slicer_parts.carried.len() != worksheet.extension_relationship_count() {
        return Err(xml_error(
            "worksheet extension relationships differ from the parts written for them",
        ));
    }
    for (id, carried) in (ids.extension_parts..).zip(&slicer_parts.carried) {
        is_write = write_relationship(
            &mut writer,
            &id.to_string(),
            &carried.relationship_type,
            &carried.target,
            if carried.external { "External" } else { "" },
        );
    }
    for (id, part_no) in [
        (ids.pivot_slicers, slicer_parts.pivot_slicers),
        (ids.table_slicers, slicer_parts.table_slicers),
    ] {
        if let (Some(id), Some(part_no)) = (id, part_no) {
            is_write = write_relationship(
                &mut writer,
                &id.to_string(),
                SLICER_REL_NS,
                format!("../slicers/slicer{part_no}.xml").as_str(),
                "",
            );
        }
    }

    if let (Some(id), Some(part_no)) = (ids.timelines, slicer_parts.timelines) {
        is_write = write_relationship(
            &mut writer,
            &id.to_string(),
            TIMELINE_REL_NS,
            format!("../timelines/timeline{part_no}.xml").as_str(),
            "",
        );
    }

    write_end_tag(&mut writer, "Relationships");

    if is_write {
        let file_path = format!("{PKG_SHEET_RELS}{worksheet_no}.xml.rels");
        writer_mng.add_writer(&file_path, writer)?;
    }
    Ok(())
}

fn write_relationship(
    writer: &mut Writer<io::Cursor<Vec<u8>>>,
    p_id: &str,
    p_type: &str,
    p_target: &str,
    p_target_mode: &str,
) -> bool {
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
    true
}
