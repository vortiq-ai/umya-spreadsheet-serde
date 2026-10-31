use std::io;

use quick_xml::{
    Reader,
    events::Event,
};

use super::{
    XlsxError,
    driver::{
        get_attribute,
        get_required_attribute,
    },
};
use crate::{
    helper::const_str::PKG_WORKBOOK,
    structs::{
        DefinedName,
        Workbook,
        WorkbookProtection,
        WorkbookView,
        Worksheet,
    },
    try_xml_read_loop,
};

pub(crate) fn read<R: io::Read + io::Seek>(
    arv: &mut zip::read::ZipArchive<R>,
) -> Result<Workbook, XlsxError> {
    crate::reader::spreadsheet_xml::validate_workbook(io::BufReader::new(
        super::driver::zip_by_name(arv, PKG_WORKBOOK)?,
    ))?;
    let r = io::BufReader::new(super::driver::zip_by_name(arv, PKG_WORKBOOK)?);
    let mut reader = Reader::from_reader(
        crate::reader::spreadsheet_xml::SpreadsheetXml::open_with_all_extensions(r)?,
    );
    reader.config_mut().trim_text(true);
    let mut wb = Workbook::default();

    let mut defined_names: Vec<DefinedName> = Vec::new();
    let mut sheet_ids = std::collections::HashSet::new();
    let mut pivot_cache_ids = std::collections::HashSet::new();
    let mut pivot_cache_relationships = std::collections::HashSet::new();
    let mut extension_entries = Vec::new();

    try_xml_read_loop!(
        reader,
        ref n @ (Event::Empty(ref e) | Event::Start(ref e)) => {
            let is_empty = matches!(n, Event::Empty(_));
            match e.name().into_inner() {
                b"workbookView" => {
                    let mut obj = WorkbookView::default();
                    obj.set_attributes(&mut reader, e);
                    wb.set_workbook_view(obj);
                }
                b"workbookPr" => {
                    if let Some(v) = get_attribute(e, b"date1904") {
                        wb.set_date1904(v == "1" || v == "true");
                    }
                }
                b"workbookProtection" => {
                    let mut obj = WorkbookProtection::default();
                    obj.set_attributes(&mut reader, e);
                    wb.set_workbook_protection(obj);
                }
                b"sheet" => {
                    let raw_id = get_required_attribute(e, b"sheetId")?;
                    let id=raw_id.parse::<u32>().map_err(|_| XlsxError::with_message(crate::XlsxErrorKind::Xml,"invalid worksheet id"))?;
                    if !sheet_ids.insert(id) {
                        return Err(XlsxError::with_message(crate::XlsxErrorKind::Xml,"duplicate worksheet id"));
                    }
                    let mut worksheet = Worksheet::default();
                    worksheet.set_attributes_from_wookbook(&mut reader, e, is_empty);
                    wb.add_sheet(worksheet)?;
                }
                b"pivotCache" => {
                    let cache_id = get_required_attribute(e, b"cacheId")?;
                    let r_id = get_required_attribute(e, b"r:id")?;
                    let cache_id = crate::xml::parse_u32(&cache_id).map_err(crate::xml::xml_error)?;
                    if !pivot_cache_ids.insert(cache_id)
                        || r_id.is_empty()
                        || !pivot_cache_relationships.insert(r_id.clone())
                    {
                        return Err(crate::xml::xml_error("duplicate or invalid workbook pivot cache binding"));
                    }
                    wb.add_pivot_caches((r_id, cache_id.to_string(), String::new()));
                }
                b"definedName" => {
                    let mut obj = DefinedName::default();
                    obj.set_attributes(&mut reader, e);
                    defined_names.push(obj);
                }
                _ if crate::reader::spreadsheet_xml::is_native_start(e, b"ext") => {
                    extension_entries.push(
                        crate::drawing::charts::OpaqueElement::read(
                            &mut reader,
                            e,
                            is_empty,
                            &crate::drawing::charts::NamespaceScope::default(),
                        )
                        .map_err(crate::xml::xml_error)?,
                    );
                }
                _ => (),
            }
        },
        Event::Eof => break
    );

    let (extensions, slicer_refs) =
        crate::structs::slicer::read_workbook_extensions(&extension_entries)
            .map_err(crate::xml::xml_error)?;
    wb.extensions = extensions;
    wb.slicer_refs = slicer_refs;

    for defined_name in &defined_names {
        if defined_name.has_local_sheet_id() {
            let local_sheet_id = defined_name.local_sheet_id() as usize;
            // A name scoped to a sheet that does not exist is dropped (Excel
            // removes it when it repairs the file) instead of aborting the
            // read.
            if let Ok(sheet) = wb.sheet_mut(local_sheet_id) {
                sheet.add_defined_names(defined_name.clone());
            }
        } else {
            if let Some(v) = defined_name.address_obj().first() {
                if let Ok(s) = wb.sheet_by_name_mut(v.sheet_name()) {
                    s.add_defined_names(defined_name.clone());
                    continue;
                }
            }
            wb.add_defined_names(defined_name.clone());
        }
    }

    Ok(wb)
}
