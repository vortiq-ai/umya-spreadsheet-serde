use std::{
    fs,
    fs::File,
    io,
    io::Read,
    path::{
        Path,
        PathBuf,
    },
};

use super::driver;
use crate::{
    XlsxError,
    helper::crypt::encrypt,
    structs::{
        Workbook,
        WriterManager,
    },
};

pub(crate) mod chart;
pub(crate) mod comment;
pub(crate) mod content_types;
pub(crate) mod doc_props_app;
pub(crate) mod doc_props_core;
pub(crate) mod doc_props_custom;
pub(crate) mod drawing;
pub(crate) mod drawing_rels;
pub(crate) mod embeddings;
pub(crate) mod jsa_project_bin;
pub(crate) mod media;
pub(crate) mod person;
pub(crate) mod pivot_cache;
pub(crate) mod pivot_table;
pub(crate) mod printer_settings;
pub(crate) mod rels;
pub(crate) mod shared_strings;
pub(crate) mod slicer;
pub(crate) mod styles;
pub(crate) mod table;
pub(crate) mod theme;
pub(crate) mod threaded_comment;
pub(crate) mod vba_project_bin;
pub(crate) mod vml_drawing;
pub(crate) mod vml_drawing_rels;
pub(crate) mod workbook;
pub(crate) mod workbook_rels;
pub(crate) mod worksheet;
pub(crate) mod worksheet_rels;

fn validate_chart_exports(wb: &Workbook) -> Result<(), XlsxError> {
    // Validate before creating the ZIP writer or emitting any chart payload.
    // The original typed state is left untouched and remains available to the
    // caller.
    for (sheet_index, sheet) in wb.sheet_collection_no_check().iter().enumerate() {
        if !sheet.has_drawing_object() {
            continue;
        }
        let mut chart_index = 0;
        let mut failure = None;
        sheet.visit_chart_spaces(&mut |space| {
            let conflicts = space.export_conflicts(wb);
            if !conflicts.is_empty() && failure.is_none() {
                failure = Some(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "chart export rejected at sheet[{sheet_index}] chart[{chart_index}]: {}",
                        conflicts.join(", ")
                    ),
                ));
            }
            chart_index += 1;
        });
        if let Some(error) = failure {
            return Err(error.into());
        }
    }
    chart::validate_picture_exports(wb)
}

/// Writes the workbook-level parts whose relationships `workbook_rels` emits
/// (cell metadata and connections); both writers must call this so a
/// relationship is never written without its part.
pub(crate) fn write_workbook_data_parts<W: io::Write + io::Seek>(
    wb: &Workbook,
    writer_manager: &mut WriterManager<W>,
) -> Result<(), XlsxError> {
    if let Some(metadata) = &wb.cell_metadata {
        let writer = quick_xml::Writer::new(io::Cursor::new(metadata.xml()));
        writer_manager.add_writer(&metadata.path, writer)?;
    }

    if let Some(data) = &wb.connections {
        let writer = quick_xml::Writer::new(io::Cursor::new(data.clone()));
        writer_manager.add_writer(crate::structs::cell_metadata::CONNECTIONS_PATH, writer)?;
    }
    Ok(())
}

fn write_zip_to_writer<W: io::Write + io::Seek>(
    wb: &Workbook,
    writer: W,
    is_light: bool,
) -> Result<(), XlsxError> {
    let prepared = pivot_cache::prepare_workbook(wb)?;
    let wb = prepared.as_ref();
    validate_chart_exports(wb)?;
    let arv = zip::ZipWriter::new(writer);
    let mut writer_manager = WriterManager::new(arv);
    writer_manager.reserve_pivot_caches(&pivot_cache::declared_caches(wb)?.order);
    writer_manager.reserve_table_ids(wb)?;
    wb.validate_slicers(Some(writer_manager.table_ids()))?;
    pivot_cache::write_retained_caches(wb, &mut writer_manager)?;
    writer_manager.reserve_drawing_parts(wb);
    chart::prepare_picture_graph(wb, &mut writer_manager)?;

    {
        writer_manager.set_is_light(is_light);

        write_workbook_data_parts(wb, &mut writer_manager)?;

        // Add docProps
        doc_props_app::write(wb, &mut writer_manager)?;
        doc_props_core::write(wb, &mut writer_manager)?;
        doc_props_custom::write(wb, &mut writer_manager)?;
        vba_project_bin::write(wb, &mut writer_manager)?;
        jsa_project_bin::write(wb, &mut writer_manager)?;
        rels::write(wb, &mut writer_manager)?;
        theme::write(wb.theme(), &mut writer_manager)?;
        person::write(wb, &mut writer_manager)?;

        let shared_string_table = wb.shared_string_table();
        let mut stylesheet = wb.stylesheet().clone();
        stylesheet.set_export_default_value();
        if !wb.timeline_caches().is_empty() {
            stylesheet.ensure_timeline_styles();
        }

        // Process each worksheet
        wb.sheet_collection_no_check()
            .iter()
            .enumerate()
            .try_for_each(|(index, worksheet)| {
                let worksheet_no = index + 1;
                if worksheet.is_deserialized() {
                    let slicers = slicer::kinds(wb, worksheet)?;
                    worksheet::write(
                        worksheet_no.try_into().unwrap(),
                        worksheet,
                        &shared_string_table,
                        &mut stylesheet,
                        wb.has_macros(),
                        slicers,
                        &mut writer_manager,
                    )
                } else {
                    worksheet
                        .raw_data_of_worksheet()
                        .write(worksheet_no.try_into().unwrap(), &mut writer_manager)
                }
            })?;

        // Process objects associated with worksheets
        wb.sheet_collection_no_check()
            .iter()
            .enumerate()
            .try_for_each(|(index, worksheet)| {
                let worksheet_no = index + 1;
                if !worksheet.is_deserialized() {
                    return Ok(());
                }

                let chart_parts = chart::write_parts(worksheet, wb, &mut writer_manager, &[])?;

                // Add drawing and its relationships
                let (drawing_no, rel_list) = drawing::write(worksheet, &mut writer_manager)?;
                drawing_rels::write(
                    worksheet,
                    &drawing_no,
                    &chart_parts,
                    &rel_list,
                    &mut writer_manager,
                )?;

                // Add vml drawing and its relationships
                let (vml_drawing_no, rel_list) =
                    vml_drawing::write(worksheet, &mut writer_manager)?;
                vml_drawing_rels::write(
                    worksheet,
                    &vml_drawing_no,
                    &rel_list,
                    &mut writer_manager,
                )?;
                let header_footer_vml_no =
                    vml_drawing::write_header_footer(worksheet, &mut writer_manager)?;

                // Add comments
                let comment_no = comment::write(worksheet, &mut writer_manager)?;

                // Add threaded_comment
                let threaded_comment_no = threaded_comment::write(worksheet, &mut writer_manager)?;

                // Add ole_object and excel
                let (ole_object_no_list, excel_no_list) =
                    embeddings::write(worksheet, &mut writer_manager)?;

                // Add Media
                media::write(worksheet, &mut writer_manager)?;
                drawing::write_dependencies(worksheet, &mut writer_manager)?;

                // Add printer settings
                let printer_settings_no = worksheet
                    .page_setup()
                    .object_data()
                    .map_or_else(String::new, |_| {
                        printer_settings::write(worksheet, &mut writer_manager).unwrap_or_default()
                    });

                // Add tables
                let table_no_list = table::write(worksheet, &mut writer_manager)?;

                // Add pivot tables and caches
                let (pivot_table_no_list, pivot_cache_no_list) =
                    pivot_cache::write_sheet_parts(worksheet, &mut writer_manager)?;

                // Add slicer parts
                let slicer_parts =
                    slicer::write_worksheet_parts(wb, worksheet, &mut writer_manager)?;

                // Add worksheet relationships
                worksheet_rels::write(
                    worksheet,
                    &worksheet_no.to_string(),
                    &drawing_no,
                    &vml_drawing_no,
                    &header_footer_vml_no,
                    &comment_no,
                    &threaded_comment_no,
                    &ole_object_no_list,
                    &excel_no_list,
                    &printer_settings_no,
                    &table_no_list,
                    &pivot_table_no_list,
                    &pivot_cache_no_list,
                    &slicer_parts,
                    &mut writer_manager,
                )
            })?;

        // Add slicer caches
        let workbook_extension_parts = slicer::write_caches(wb, &mut writer_manager)?;

        // Finalize file list and add remaining components
        writer_manager.file_list_sort();
        shared_strings::write(&shared_string_table, &mut writer_manager)?;
        styles::write(&stylesheet, &mut writer_manager)?;
        workbook::write(wb, &mut writer_manager)?;

        let has_shared_string_table = shared_string_table.read().unwrap().has_value();
        workbook_rels::write(
            wb,
            has_shared_string_table,
            &workbook_extension_parts,
            &mut writer_manager,
        )?;
        content_types::write(wb, &mut writer_manager)?;
    }

    writer_manager.finish()?;
    Ok(())
}

fn make_buffer(wb: &Workbook, is_light: bool) -> Result<Vec<u8>, XlsxError> {
    let mut cursor = io::Cursor::new(Vec::new());
    write_zip_to_writer(wb, &mut cursor, is_light)?;
    Ok(cursor.into_inner())
}

/// write spreadsheet file to arbitrary writer.
/// # Arguments
/// * `wb` - Workbook structs object.
/// * `writer` - writer to write to.
/// # Return value
/// * `Result` - OK is void. Err is error message.
#[inline]
pub fn write_writer<W: io::Write>(wb: &Workbook, mut writer: W) -> Result<(), XlsxError> {
    let buffer = make_buffer(wb, false)?;
    writer.write_all(&buffer)?;
    Ok(())
}

/// write spreadsheet file to arbitrary writer.
/// # Arguments
/// * `wb` - Workbook structs object.
/// * `writer` - writer to write to.
/// # Return value
/// * `Result` - OK is void. Err is error message.
#[inline]
pub fn write_writer_light<W: io::Write>(wb: &Workbook, mut writer: W) -> Result<(), XlsxError> {
    let buffer = make_buffer(wb, true)?;
    writer.write_all(&buffer)?;
    Ok(())
}

/// write spreadsheet file.
/// # Arguments
/// * `wb` - Workbook structs object.
/// * `path` - file path to save.
/// # Return value
/// * `Result` - OK is void. Err is error message.
/// # Examples
/// ```
/// let mut book = umya_spreadsheet::new_file();
/// let path = std::path::Path::new("./tests/result_files/zzz.xlsx");
/// let _unused = umya_spreadsheet::writer::xlsx::write(&book, path);
/// ```
pub fn write<P: AsRef<Path>>(wb: &Workbook, path: P) -> Result<(), XlsxError> {
    validate_chart_exports(wb)?;
    let path_tmp = make_tmp_path(path.as_ref());
    if let Err(v) = write_zip_to_writer(wb, io::BufWriter::new(File::create(&path_tmp)?), false) {
        fs::remove_file(path_tmp)?;
        return Err(v);
    }
    fs::rename(path_tmp, path)?;
    Ok(())
}

/// write spreadsheet file.
/// # Arguments
/// * `wb` - Workbook structs object.
/// * `path` - file path to save.
/// # Return value
/// * `Result` - OK is void. Err is error message.
/// # Examples
/// ```
/// let mut book = umya_spreadsheet::new_file();
/// let path = std::path::Path::new("./tests/result_files/zzz.xlsx");
/// let _unused = umya_spreadsheet::writer::xlsx::write_light(&book, path);
/// ```
pub fn write_light<P: AsRef<Path>>(wb: &Workbook, path: P) -> Result<(), XlsxError> {
    validate_chart_exports(wb)?;
    let path_tmp = make_tmp_path(path.as_ref());
    if let Err(v) = write_zip_to_writer(wb, io::BufWriter::new(File::create(&path_tmp)?), true) {
        fs::remove_file(path_tmp)?;
        return Err(v);
    }
    fs::rename(path_tmp, path)?;
    Ok(())
}

/// write spreadsheet file with password.
/// # Arguments
/// * `wb` - Workbook structs object.
/// * `path` - file path to save.
/// * `password` - password.
/// # Return value
/// * `Result` - OK is void. Err is error message.
/// # Examples
/// ```
/// let mut book = umya_spreadsheet::new_file();
/// let path = std::path::Path::new("./tests/result_files/zzz_password.xlsx");
/// let _unused = umya_spreadsheet::writer::xlsx::write_with_password(&book, path, "password");
/// ```
pub fn write_with_password<P: AsRef<Path>>(
    wb: &Workbook,
    path: P,
    password: &str,
) -> Result<(), XlsxError> {
    let path_tmp = make_tmp_path(path.as_ref());
    let buffer = make_buffer(wb, false)?;

    // set password
    if let Err(v) = encrypt(&path_tmp, &buffer, password) {
        fs::remove_file(&path_tmp).ok();
        return Err(v);
    }

    fs::rename(path_tmp, path)?;
    Ok(())
}

/// write spreadsheet file with password.
/// # Arguments
/// * `wb` - Workbook structs object.
/// * `path` - file path to save.
/// * `password` - password.
/// # Return value
/// * `Result` - OK is void. Err is error message.
/// # Examples
/// ```
/// let mut book = umya_spreadsheet::new_file();
/// let path = std::path::Path::new("./tests/result_files/zzz_password.xlsx");
/// let _unused =
///     umya_spreadsheet::writer::xlsx::write_with_password_light(&book, path, "password");
/// ```
pub fn write_with_password_light<P: AsRef<Path>>(
    wb: &Workbook,
    path: P,
    password: &str,
) -> Result<(), XlsxError> {
    let path_tmp = make_tmp_path(path.as_ref());
    let buffer = make_buffer(wb, true)?;

    // set password
    if let Err(v) = encrypt(&path_tmp, &buffer, password) {
        fs::remove_file(&path_tmp).ok();
        return Err(v);
    }

    fs::rename(path_tmp, path)?;
    Ok(())
}

/// write spreadsheet file with password.
/// # Arguments
/// * `from_path` - file path from readfile.
/// * `to_path` - file path to save.
/// * `password` - password.
/// # Return value
/// * `Result` - OK is void. Err is error message.
/// # Examples
/// ```
/// let from_path = std::path::Path::new("./tests/test_files/aaa.xlsx");
/// let to_path = std::path::Path::new("./tests/result_files/zzz_password2.xlsx");
/// let _unused = umya_spreadsheet::writer::xlsx::set_password(&from_path, &to_path, "password");
/// ```
pub fn set_password<P: AsRef<Path>>(
    from_path: P,
    to_path: P,
    password: &str,
) -> Result<(), XlsxError> {
    let mut file = File::open(from_path)?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;

    // set password
    encrypt(&to_path, &buffer, password)
}

/// Returns the temporary path used while writing `path`.
/// Fix #328, add default extension
fn make_tmp_path(path: &Path) -> PathBuf {
    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("xlsx");
    path.with_extension(format!("{extension}tmp"))
}
