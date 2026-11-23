use std::{
    collections::HashMap,
    fs::File,
    io,
};

use quick_xml::{
    Reader,
    escape,
    events::{
        BytesStart,
        Event,
    },
};

use super::{
    XlsxError,
    driver::{
        get_attribute,
        get_attribute_value,
        try_xml_read_loop,
        zip_by_name,
    },
};
use crate::{
    AutoFilter,
    XlsxErrorKind,
    helper::formula::FormulaToken,
    structs::{
        Cell,
        Cells,
        Columns,
        ConditionalFormatting,
        DataValidations,
        Hyperlink,
        OleObjects,
        Row,
        SharedStringTable,
        SharedStyles,
        SheetProtection,
        Stylesheet,
        Worksheet,
        office2010::excel::DataValidations as DataValidations2010,
        raw::{
            RawRelationships,
            RawWorksheet,
        },
    },
};

/// Refuse a worksheet part whose text is not valid in its declared encoding;
/// the readers below cannot report it from inside their element parsers.
fn check_part_text(raw_data_of_worksheet: &RawWorksheet) -> Result<(), XlsxError> {
    let file = raw_data_of_worksheet.worksheet_file();
    if let Some(source_file) = file.source_file() {
        if !file.has_file_data() {
            let mut archive = zip::read::ZipArchive::new(File::open(source_file)?)?;
            let source = zip_by_name(&mut archive, file.file_target())?;
            return Ok(crate::reader::spreadsheet_xml::validate_part_text(
                io::BufReader::new(source),
            )?);
        }
    }
    if let Some(package) = file.source_package() {
        if !file.has_file_data() {
            let mut archive = zip::read::ZipArchive::new(io::Cursor::new(package))?;
            let source = zip_by_name(&mut archive, file.file_target())?;
            return Ok(crate::reader::spreadsheet_xml::validate_part_text(
                io::BufReader::new(source),
            )?);
        }
    }
    Ok(crate::reader::spreadsheet_xml::validate_part_text(
        io::Cursor::new(file.file_data()),
    )?)
}

/// The relationship ids named by a worksheet's `legacyDrawing` and
/// `legacyDrawingHF` elements, taken as the worksheet is parsed, so a sheet is
/// decompressed once: they bind the sheet's VML parts (comments, header and
/// footer pictures).
#[derive(Default)]
pub(crate) struct LegacyDrawingIds {
    pub(crate) legacy:        Option<String>,
    pub(crate) header_footer: Option<String>,
}

impl LegacyDrawingIds {
    /// Record `element` when it is a `legacyDrawing` or `legacyDrawingHF`: its
    /// relationship id is required, and each element may appear once.
    fn capture(&mut self, element: &BytesStart<'_>) -> Result<(), XlsxError> {
        let slot = match element.local_name().as_ref() {
            b"legacyDrawing" => &mut self.legacy,
            b"legacyDrawingHF" => &mut self.header_footer,
            _ => return Ok(()),
        };
        let mut id = None;
        for attribute in element.attributes() {
            let attribute = attribute.map_err(crate::xml::xml_error)?;
            if attribute.key.local_name().as_ref() == b"id" {
                id = Some(
                    attribute
                        .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                        .map_err(crate::xml::xml_error)?
                        .into_owned(),
                );
            }
        }
        if slot
            .replace(id.ok_or_else(|| {
                crate::xml::xml_error("legacy drawing element has no relationship id")
            })?)
            .is_some()
        {
            return Err(crate::xml::xml_error(
                "worksheet declares a legacy drawing element more than once",
            ));
        }
        Ok(())
    }
}

pub(crate) fn read(
    worksheet: &mut Worksheet,
    raw_data_of_worksheet: &RawWorksheet,
    shared_string_table: &SharedStringTable,
    stylesheet: &Stylesheet,
) -> Result<LegacyDrawingIds, XlsxError> {
    check_part_text(raw_data_of_worksheet)?;
    if let Some(source_file) = raw_data_of_worksheet.worksheet_file().source_file() {
        if !raw_data_of_worksheet.worksheet_file().has_file_data() {
            let file = File::open(source_file)?;
            let mut archive = zip::read::ZipArchive::new(file)?;
            let source = zip_by_name(
                &mut archive,
                raw_data_of_worksheet.worksheet_file().file_target(),
            )?;
            let reader = io::BufReader::new(source);
            return read_from_reader(
                worksheet,
                reader,
                raw_data_of_worksheet,
                shared_string_table,
                stylesheet,
            );
        }
    }
    // A sheet deferred in an in-memory package is parsed as it is decompressed,
    // never held whole.
    if let Some(package) = raw_data_of_worksheet.worksheet_file().source_package() {
        if !raw_data_of_worksheet.worksheet_file().has_file_data() {
            let mut archive = zip::read::ZipArchive::new(io::Cursor::new(package))?;
            let source = zip_by_name(
                &mut archive,
                raw_data_of_worksheet.worksheet_file().file_target(),
            )?;
            let reader = io::BufReader::new(source);
            return read_from_reader(
                worksheet,
                reader,
                raw_data_of_worksheet,
                shared_string_table,
                stylesheet,
            );
        }
    }

    let data = io::Cursor::new(raw_data_of_worksheet.worksheet_file().file_data());
    read_from_reader(
        worksheet,
        data,
        raw_data_of_worksheet,
        shared_string_table,
        stylesheet,
    )
}

fn read_from_reader<R: io::BufRead>(
    worksheet: &mut Worksheet,
    data: R,
    raw_data_of_worksheet: &RawWorksheet,
    shared_string_table: &SharedStringTable,
    stylesheet: &Stylesheet,
) -> Result<LegacyDrawingIds, XlsxError> {
    let mut reader = Reader::from_reader(
        crate::reader::spreadsheet_xml::SpreadsheetXml::open_with_all_extensions(data)?,
    );
    reader.config_mut().trim_text(true);
    let mut formula_shared_list: HashMap<u32, (String, Vec<FormulaToken>)> = HashMap::new();
    let mut styles = SharedStyles::default();
    let mut legacy_drawing_ids = LegacyDrawingIds::default();
    worksheet.cells_crate_mut().reserve(65536);
    try_xml_read_loop!(
        reader,
        Event::Start(ref e) => match e.name().into_inner() {
            b"sheetPr" => {
                for a in e.attributes().with_checks(false) {
                    match a {
                        Ok(ref attr) if attr.key.0 == b"codeName" => {
                            worksheet.set_code_name(get_attribute_value(attr)?);
                        }
                        Ok(_) | Err(_) => {}
                    }
                }
            }
            b"sheetViews" => {
                worksheet
                    .sheet_views_mut()
                    .set_attributes(&mut reader, e);
            }
            b"sheetFormatPr" => {
                worksheet
                    .sheet_format_properties_mut()
                    .set_attributes(&mut reader, e);
            }
            b"selection" => {
                for a in e.attributes().with_checks(false) {
                    match a {
                        Ok(ref attr) if attr.key.0 == b"activeCell" => {
                            worksheet.set_active_cell(get_attribute_value(attr)?);
                        }
                        Ok(_) | Err(_) => {}
                    }
                }
            }
            b"row" => {
                let mut obj = Row::default();
                obj.set_attributes(
                    &mut reader,
                    e,
                    worksheet.cells_crate_mut(),
                    shared_string_table,
                    stylesheet,
                    &mut formula_shared_list,
                    false,
                    &mut styles,
                );
                worksheet.set_row_dimension(obj);
            }
            _ if crate::reader::spreadsheet_xml::is_native_start(e, b"autoFilter") => {
                let mut obj = AutoFilter::default();
                obj.set_attributes(&mut reader, e, false).map_err(crate::xml::xml_error)?;
                worksheet.set_auto_filter_obj(obj);
            }
            _ if crate::structs::is_preserved_sheet_element(e) => {
                let node = crate::drawing::charts::OpaqueElement::read(
                    &mut reader,
                    e,
                    false,
                    &crate::drawing::charts::NamespaceScope::default(),
                )
                .map_err(crate::xml::xml_error)?;
                worksheet.preserve_element(node);
            }
            _ if crate::reader::spreadsheet_xml::is_native_start(e, b"ext") => {
                let node = crate::drawing::charts::OpaqueElement::read(
                    &mut reader,
                    e,
                    false,
                    &crate::drawing::charts::NamespaceScope::default(),
                )
                .map_err(crate::xml::xml_error)?;
                crate::structs::slicer::read_worksheet_extension(
                    &mut worksheet.extensions,
                    &mut worksheet.slicer_part_references,
                    node,
                )
                .map_err(crate::xml::xml_error)?;
            }
            b"ext"
                if e.try_get_attribute(b"uri").ok().flatten().is_some_and(|uri| {
                    crate::structs::slicer::is_data_validations_uri(uri.value.as_ref())
                }) =>
            {
                worksheet
                    .extensions
                    .push(crate::structs::slicer::WorksheetExtension::DataValidations);
            }
            b"cols" => {
                let mut obj = Columns::default();
                obj.set_attributes(&mut reader, e, stylesheet);
                worksheet.set_column_dimensions_crate(obj);
            }
            b"mergeCells" => {
                worksheet
                    .merge_cells_crate_mut()
                    .set_attributes(&mut reader, e);
            }
            b"conditionalFormatting" => {
                let mut obj = ConditionalFormatting::default();
                obj.set_attributes(&mut reader, e, stylesheet.differential_formats())?;
                worksheet.add_conditional_formatting_collection(obj);
            }
            b"dataValidations" => {
                let mut obj = DataValidations::default();
                obj.set_attributes(&mut reader, e);
                worksheet.set_data_validations(obj);
            }
            b"x14:dataValidations" => {
                let mut obj = DataValidations2010::default();
                obj.set_attributes(&mut reader, e);
                worksheet.set_data_validations_2010(obj);
            }
            name if crate::reader::driver::local_name(name) == b"oleObjects" => {
                let mut obj = OleObjects::default();
                let relationships = raw_data_of_worksheet.worksheet_relationships()?.ok_or_else(|| {
                    XlsxError::with_message(
                        XlsxErrorKind::NotFound,
                        "worksheet relationships for oleObjects",
                    )
                })?;
                obj.set_attributes(
                    &mut reader,
                    e,
                    relationships,
                    &crate::drawing::charts::NamespaceScope::default(),
                )?;
                worksheet.set_ole_objects(obj);
            }
            b"headerFooter" => {
                worksheet
                    .header_footer_mut()
                    .set_attributes(&mut reader, e);
            }
            b"rowBreaks" => {
                worksheet
                    .row_breaks_mut()
                    .set_attributes(&mut reader, e);
            }
            b"colBreaks" => {
                worksheet
                    .column_breaks_mut()
                    .set_attributes(&mut reader, e);
            }
            b"legacyDrawing" | b"legacyDrawingHF" => legacy_drawing_ids.capture(e)?,
            _ => (),
        },
        Event::Empty(ref e) => match e.name().into_inner() {
            b"sheetPr" => {
                for a in e.attributes().with_checks(false) {
                    match a {
                        Ok(ref attr) if attr.key.0 == b"codeName" => {
                            worksheet.set_code_name(get_attribute_value(attr)?);
                        }
                        Ok(_) | Err(_) => {}
                    }
                }
            }
            b"tabColor" => {
                worksheet
                    .tab_color_mut()
                    .set_attributes(&mut reader, e, true);
            }
            b"pageSetUpPr" => {
                if let Some(v) = get_attribute(e, b"fitToPage") {
                    worksheet.page_setup_mut().set_fit_to_page_str(&v);
                }
            }
            b"sheetFormatPr" => {
                worksheet
                    .sheet_format_properties_mut()
                    .set_attributes(&mut reader, e);
            }
            b"selection" => {
                for a in e.attributes().with_checks(false) {
                    match a {
                        Ok(ref attr) if attr.key.0 == b"activeCell" => {
                            worksheet.set_active_cell(get_attribute_value(attr)?);
                        }
                        Ok(_) | Err(_) => {}
                    }
                }
            }
            b"row" => {
                let mut obj = Row::default();
                obj.set_attributes(
                    &mut reader,
                    e,
                    worksheet.cells_crate_mut(),
                    shared_string_table,
                    stylesheet,
                    &mut formula_shared_list,
                    true,
                    &mut styles,
                );
                worksheet.set_row_dimension(obj);
            }
            _ if crate::reader::spreadsheet_xml::is_native_start(e, b"autoFilter") => {
                let mut obj = AutoFilter::default();
                obj.set_attributes(&mut reader, e, true).map_err(crate::xml::xml_error)?;
                worksheet.set_auto_filter_obj(obj);
            }
            _ if crate::structs::is_preserved_sheet_element(e) => {
                let node = crate::drawing::charts::OpaqueElement::read(
                    &mut reader,
                    e,
                    true,
                    &crate::drawing::charts::NamespaceScope::default(),
                )
                .map_err(crate::xml::xml_error)?;
                worksheet.preserve_element(node);
            }
            _ if crate::reader::spreadsheet_xml::is_native_start(e, b"ext") => {
                let node = crate::drawing::charts::OpaqueElement::read(
                    &mut reader,
                    e,
                    true,
                    &crate::drawing::charts::NamespaceScope::default(),
                )
                .map_err(crate::xml::xml_error)?;
                crate::structs::slicer::read_worksheet_extension(
                    &mut worksheet.extensions,
                    &mut worksheet.slicer_part_references,
                    node,
                )
                .map_err(crate::xml::xml_error)?;
            }
            b"pageMargins" => {
                worksheet
                    .page_margins_mut()
                    .set_attributes(&mut reader, e);
            }
            b"hyperlink" => {
                let (coor, hyperlink) = get_hyperlink(
                    e,
                    raw_data_of_worksheet.worksheet_relationships()?
                )?;
                worksheet.cell_mut(coor).set_hyperlink(hyperlink);
            }
            b"printOptions" => {
                worksheet
                    .print_options_mut()
                    .set_attributes(&mut reader, e);
            }
            b"pageSetup" => {
                worksheet.page_setup_mut().set_attributes(
                    &mut reader,
                    e,
                    raw_data_of_worksheet.worksheet_relationships()?,
                );
            }
            b"sheetProtection" => {
                let mut obj = SheetProtection::default();
                obj.set_attributes(
                    &mut reader,
                    e,
                );
                worksheet.set_sheet_protection(obj);
            }
            b"legacyDrawing" | b"legacyDrawingHF" => legacy_drawing_ids.capture(e)?,
            _ => (),
        },
        Event::Eof => break,
    );

    worksheet.cells_crate_mut().build_indexes();

    Ok(legacy_drawing_ids)
}

pub(crate) fn read_lite(
    raw_data_of_worksheet: &RawWorksheet,
    shared_string_table: &SharedStringTable,
    stylesheet: &Stylesheet,
) -> Result<Cells, XlsxError> {
    check_part_text(raw_data_of_worksheet)?;
    if let Some(source_file) = raw_data_of_worksheet.worksheet_file().source_file() {
        if !raw_data_of_worksheet.worksheet_file().has_file_data() {
            let file = File::open(source_file)?;
            let mut archive = zip::read::ZipArchive::new(file)?;
            let source = zip_by_name(
                &mut archive,
                raw_data_of_worksheet.worksheet_file().file_target(),
            )?;
            let reader = io::BufReader::new(source);
            return read_lite_from_reader(reader, shared_string_table, stylesheet);
        }
    }
    if let Some(package) = raw_data_of_worksheet.worksheet_file().source_package() {
        if !raw_data_of_worksheet.worksheet_file().has_file_data() {
            let mut archive = zip::read::ZipArchive::new(io::Cursor::new(package))?;
            let source = zip_by_name(
                &mut archive,
                raw_data_of_worksheet.worksheet_file().file_target(),
            )?;
            let reader = io::BufReader::new(source);
            return read_lite_from_reader(reader, shared_string_table, stylesheet);
        }
    }

    let data = io::Cursor::new(raw_data_of_worksheet.worksheet_file().file_data());
    read_lite_from_reader(data, shared_string_table, stylesheet)
}

fn read_lite_from_reader<R: io::BufRead>(
    data: R,
    shared_string_table: &SharedStringTable,
    stylesheet: &Stylesheet,
) -> Result<Cells, XlsxError> {
    let mut reader =
        Reader::from_reader(crate::reader::spreadsheet_xml::SpreadsheetXml::open(data)?);
    reader.config_mut().trim_text(true);

    let mut cells = Cells::default();
    cells.reserve(65536);
    let mut formula_shared_list: HashMap<u32, (String, Vec<FormulaToken>)> = HashMap::new();
    let mut styles = SharedStyles::default();
    try_xml_read_loop!(
        reader,
        Event::Start(ref e) => {
            if e.name().into_inner() == b"row" {
                let mut obj = Row::default();
                obj.set_attributes(
                    &mut reader,
                    e,
                    &mut cells,
                    shared_string_table,
                    stylesheet,
                    &mut formula_shared_list,
                    false,
                    &mut styles,
                );
            }
        },
        Event::Empty(ref e) => {
            if e.name().into_inner() == b"row" {
                let mut obj = Row::default();
                obj.set_attributes(
                    &mut reader,
                    e,
                    &mut cells,
                    shared_string_table,
                    stylesheet,
                    &mut formula_shared_list,
                    true,
                    &mut styles,
                );
            }
        },
        Event::Eof => break,
    );

    cells.build_indexes();
    Ok(cells)
}

pub(crate) fn read_cells_stream<F>(
    raw_data_of_worksheet: &RawWorksheet,
    shared_string_table: &SharedStringTable,
    stylesheet: &Stylesheet,
    callback: F,
) -> Result<(), XlsxError>
where
    F: FnMut(&Cell),
{
    check_part_text(raw_data_of_worksheet)?;
    if let Some(source_file) = raw_data_of_worksheet.worksheet_file().source_file() {
        if !raw_data_of_worksheet.worksheet_file().has_file_data() {
            let file = File::open(source_file)?;
            let mut archive = zip::read::ZipArchive::new(file)?;
            let source = zip_by_name(
                &mut archive,
                raw_data_of_worksheet.worksheet_file().file_target(),
            )?;
            let reader = io::BufReader::new(source);
            return read_cells_stream_from_reader(
                reader,
                shared_string_table,
                stylesheet,
                callback,
            );
        }
    }
    if let Some(package) = raw_data_of_worksheet.worksheet_file().source_package() {
        if !raw_data_of_worksheet.worksheet_file().has_file_data() {
            let mut archive = zip::read::ZipArchive::new(io::Cursor::new(package))?;
            let source = zip_by_name(
                &mut archive,
                raw_data_of_worksheet.worksheet_file().file_target(),
            )?;
            let reader = io::BufReader::new(source);
            return read_cells_stream_from_reader(
                reader,
                shared_string_table,
                stylesheet,
                callback,
            );
        }
    }

    let data = io::Cursor::new(raw_data_of_worksheet.worksheet_file().file_data());
    read_cells_stream_from_reader(data, shared_string_table, stylesheet, callback)
}

fn read_cells_stream_from_reader<R, F>(
    data: R,
    shared_string_table: &SharedStringTable,
    stylesheet: &Stylesheet,
    mut callback: F,
) -> Result<(), XlsxError>
where
    R: io::BufRead,
    F: FnMut(&Cell),
{
    let mut reader =
        Reader::from_reader(crate::reader::spreadsheet_xml::SpreadsheetXml::open(data)?);
    reader.config_mut().trim_text(true);
    let mut formula_shared_list: HashMap<u32, (String, Vec<FormulaToken>)> = HashMap::new();
    let mut styles = SharedStyles::default();

    try_xml_read_loop!(
        reader,
        Event::Start(ref e) => {
            if e.name().into_inner() == b"row" {
                read_row_cells_stream(
                    &mut reader,
                    shared_string_table,
                    stylesheet,
                    &mut formula_shared_list,
                    &mut styles,
                    &mut callback,
                )?;
            }
        },
        Event::Eof => break,
    );

    Ok(())
}

fn read_row_cells_stream<R, F>(
    reader: &mut Reader<R>,
    shared_string_table: &SharedStringTable,
    stylesheet: &Stylesheet,
    formula_shared_list: &mut HashMap<u32, (String, Vec<FormulaToken>)>,
    styles: &mut SharedStyles,
    callback: &mut F,
) -> Result<(), XlsxError>
where
    R: io::BufRead,
    F: FnMut(&Cell),
{
    try_xml_read_loop!(
        reader,
        Event::Empty(ref e) => {
            if e.name().into_inner() == b"c" {
                let mut obj = Cell::default();
                obj.set_attributes(
                    reader,
                    e,
                    shared_string_table,
                    stylesheet,
                    true,
                    formula_shared_list,
                    styles,
                );
                callback(&obj);
            }
        },
        Event::Start(ref e) => {
            if e.name().into_inner() == b"c" {
                let mut obj = Cell::default();
                obj.set_attributes(
                    reader,
                    e,
                    shared_string_table,
                    stylesheet,
                    false,
                    formula_shared_list,
                    styles,
                );
                callback(&obj);
            }
        },
        Event::End(ref e) => {
            if e.name().into_inner() == b"row" {
                return Ok(());
            }
        },
        Event::Eof => {
            return Err(XlsxError::with_message(
                XlsxErrorKind::Xml,
                "could not find row end element",
            ));
        }
    );
}

fn get_hyperlink(
    e: &BytesStart<'_>,
    raw_relationships: Option<&RawRelationships>,
) -> Result<(String, Hyperlink), XlsxError> {
    let mut hyperlink = Hyperlink::default();

    let coordition = get_attribute(e, b"ref").unwrap_or_default();
    if let Some(v) = get_attribute(e, b"location") {
        hyperlink.set_url(escape::unescape(&v)?.to_string());
        hyperlink.set_location(true);
    }
    if let Some(v) = get_attribute(e, b"tooltip") {
        hyperlink.set_tooltip(escape::unescape(&v)?.to_string());
    }
    if let Some(v) = get_attribute(e, b"r:id") {
        // A dangling r:id (no relationship, or no relationships part at all)
        // leaves the hyperlink without a URL instead of aborting the read.
        if let Some(relationship) =
            raw_relationships.and_then(|rels| rels.find_relationship_by_rid(&v))
        {
            hyperlink.set_url(relationship.target());
        }
    }
    Ok((coordition, hyperlink))
}
