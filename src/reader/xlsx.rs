use std::{
    fs::File,
    io,
    path::Path,
    sync::RwLock,
};

use super::driver;
use crate::{
    XlsxError,
    XlsxErrorKind,
    helper::const_str::{
        COMMENTS_NS,
        DRAWINGS_NS,
        PIVOT_TABLE_NS,
        TABLE_NS,
        THEME_NS,
        THREADED_COMMENT_NS,
        VML_DRAWING_NS,
    },
    structs::{
        Cell,
        SharedStringTable,
        Stylesheet,
        Workbook,
        Worksheet,
        drawing::Theme,
        raw::RawWorksheet,
    },
};

pub(crate) mod chart;
mod chart_cache;
pub(crate) mod comment;
mod content_types;
mod doc_props_app;
mod doc_props_core;
mod doc_props_custom;
pub(crate) mod drawing;
pub(crate) mod iso_date;
mod jsa_project_bin;
mod pivot_cache;
mod pivot_table;
mod rels;
mod shared_strings;
mod styles;
pub(crate) mod table;
mod theme;
pub(crate) mod threaded_comment;
mod vba_project_bin;
pub(crate) mod vml_drawing;
mod workbook;
mod workbook_rels;
pub(crate) mod worksheet;

pub(crate) use self::chart_cache::{
    ChartPartProvider,
    EagerChartPartProvider,
    LazyChartPartProvider,
    SourceCacheSession,
    capture_chart_source_caches,
    finalize_parsed_string_cache,
    register_parsed_string_cache,
};

/// read spreadsheet from arbitrary reader.
/// # Arguments
/// * `reader` - reader to read from.
/// # Return value
/// * `Result` - OK is `Workbook`. Err is error message.
pub fn read_reader<R: io::Read + io::Seek>(
    reader: R,
    with_sheet_read: bool,
) -> Result<Workbook, XlsxError> {
    read_reader_with_source(reader, with_sheet_read, None)
}

fn read_reader_with_source<R: io::Read + io::Seek>(
    reader: R,
    with_sheet_read: bool,
    source_file: Option<&Path>,
) -> Result<Workbook, XlsxError> {
    let mut arv = zip::read::ZipArchive::new(reader)?;

    let mut book = workbook::read(&mut arv)?;
    doc_props_app::read(&mut arv, &mut book)?;
    doc_props_core::read(&mut arv, &mut book)?;
    doc_props_custom::read(&mut arv, &mut book)?;
    vba_project_bin::read(&mut arv, &mut book)?;
    jsa_project_bin::read(&mut arv, &mut book)?;
    content_types::read(&mut arv, &mut book)?;
    let workbook_rel = workbook_rels::read(&mut arv, &mut book)?;

    for (_, rel_type, target) in &workbook_rel {
        if rel_type == crate::structs::cell_metadata::REL {
            let path = driver::join_paths("xl", target);
            let mut xml = Vec::new();
            let part = driver::zip_by_name(&mut arv, &path)?;
            let cap = crate::structs::cell_metadata::MAX_METADATA_BYTES;
            if part.size() > cap {
                return Err(XlsxError::with_message(
                    XlsxErrorKind::Xml,
                    "Spreadsheet metadata exceeds the size limit",
                ));
            }
            io::Read::read_to_end(&mut io::Read::take(part, cap + 1), &mut xml)?;
            if xml.len() as u64 > cap {
                return Err(XlsxError::with_message(
                    XlsxErrorKind::Xml,
                    "Spreadsheet metadata exceeds the size limit",
                ));
            }
            book.cell_metadata = Some(crate::structs::cell_metadata::CellMetadata::from_xml(
                &xml, path,
            )?);
        }
    }

    book.set_theme(Theme::default_value());
    for (_, type_value, rel_target) in &workbook_rel {
        if type_value == THEME_NS {
            let theme = theme::read(&mut arv, rel_target)?;
            book.set_theme(theme);
        }
    }

    shared_strings::read(&mut arv, &mut book)?;
    styles::read(&mut arv, &mut book)?;

    for sheet in book.sheet_collection_unchecked_mut() {
        for (rel_id, _, rel_target) in &workbook_rel {
            if sheet.r_id() != rel_id {
                continue;
            }
            let mut raw_worksheet = RawWorksheet::default();
            match source_file {
                Some(source_file) => raw_worksheet.read_lazy(&mut arv, rel_target, source_file)?,
                None => raw_worksheet.read(&mut arv, rel_target, with_sheet_read)?,
            }
            sheet.set_raw_data_of_worksheet(raw_worksheet);
        }
    }

    if with_sheet_read {
        materialize_eager_sheets(&mut book, &mut arv)?;
    }

    Ok(book)
}

/// read spreadsheet file.
/// # Arguments
/// * `path` - file path to read.
/// # Return value
/// * `Result` - OK is Workbook. Err is error message.
/// # Examples
/// ```
/// let path = std::path::Path::new("./tests/test_files/aaa.xlsx");
/// let mut book = umya_spreadsheet::reader::xlsx::read(path).unwrap();
/// ```
#[inline]
pub fn read<P: AsRef<Path>>(path: P) -> Result<Workbook, XlsxError> {
    let file = File::open(path)?;
    read_reader(file, true)
}

/// lazy read spreadsheet file.
/// Delays the loading of the worksheet until it is needed.
/// When loading a file with a large amount of data, response improvement can be
/// expected. # Arguments
/// * `path` - file path to read.
/// # Return value
/// * `Result` - OK is Workbook. Err is error message.
/// # Examples
/// ```
/// let path = std::path::Path::new("./tests/test_files/aaa.xlsx");
/// let mut book = umya_spreadsheet::reader::xlsx::lazy_read(path).unwrap();
/// ```
#[inline]
pub fn lazy_read(path: &Path) -> Result<Workbook, XlsxError> {
    let file = File::open(path)?;
    read_reader_with_source(file, false, Some(path))
}

/// Stream cells from a worksheet without deserializing the worksheet into
/// memory.
///
/// This is intended for very large sheets where building a full [`Worksheet`]
/// would allocate too much memory. The callback receives one parsed [`Cell`] at
/// a time; the cell is dropped before the next cell is parsed.
pub fn read_sheet_by_name_stream<P, F>(
    path: P,
    sheet_name: &str,
    callback: F,
) -> Result<(), XlsxError>
where
    P: AsRef<Path>,
    F: FnMut(&Cell),
{
    let book = lazy_read(path.as_ref())?;
    let Some(worksheet) = book
        .sheet_collection_no_check()
        .iter()
        .find(|worksheet| worksheet.name() == sheet_name)
    else {
        return Err(XlsxError::with_message(
            XlsxErrorKind::NotFound,
            format!("worksheet '{sheet_name}'"),
        ));
    };
    let shared_string_table = book.shared_string_table();
    let shared_string_table = &*shared_string_table.read().unwrap();
    worksheet::read_cells_stream(
        worksheet.raw_data_of_worksheet(),
        shared_string_table,
        book.stylesheet(),
        callback,
    )
}

pub(crate) fn raw_to_deserialize_by_worksheet(
    worksheet: &mut Worksheet,
    shared_string_table: &RwLock<SharedStringTable>,
    stylesheet: &Stylesheet,
    session: &mut SourceCacheSession,
    provider: &mut dyn ChartPartProvider,
) -> Result<(), XlsxError> {
    if worksheet.is_deserialized() {
        return Ok(());
    }

    let mut parsed = worksheet.materialization_target();
    let mut raw = worksheet.take_raw_data_of_worksheet();
    let mut sheet_session = session.for_sheet();
    let mut chart_parts_added = false;
    let mut chart_donors = std::collections::HashMap::new();
    let result = (|| {
        let shared_string_table = &*shared_string_table.read().unwrap();
        worksheet::read(&mut parsed, &raw, shared_string_table, stylesheet)?;
        raw.load_relationship_file_data_from_source()?;
        let mut chart_parts = std::collections::HashMap::new();
        for (list_index, relationships) in raw.relationships_list_mut().iter_mut().enumerate() {
            for (relationship_index, relationship) in
                relationships.relationship_list_mut().iter_mut().enumerate()
            {
                if relationship.get_type() != crate::helper::const_str::CHART_NS {
                    continue;
                }
                let file = relationship.raw_file_mut();
                let target = file.file_target().to_string();
                let pending = chart_parts
                    .entry(target.clone())
                    .or_insert_with(|| (Vec::new(), file.source_file_owned()));
                let donor = chart_donors.entry(target).or_insert(None);
                if pending.0.is_empty() && file.has_file_data() {
                    pending.0 = file.take_file_data();
                    *donor = Some((list_index, relationship_index));
                }
            }
        }
        provider.add_pending(chart_parts);
        chart_parts_added = true;

        if let Some(relationships) = raw.worksheet_relationships() {
            for relationship in relationships.relationship_list() {
                match relationship.get_type() {
                    DRAWINGS_NS => {
                        drawing::read(
                            &mut parsed,
                            relationship.raw_file(),
                            raw.drawing_relationships(),
                            &mut sheet_session,
                            provider,
                        )?;
                    }
                    COMMENTS_NS => comment::read(&mut parsed, relationship.raw_file())?,
                    THREADED_COMMENT_NS => {
                        threaded_comment::read(&mut parsed, relationship.raw_file())?;
                    }
                    TABLE_NS => table::read(&mut parsed, relationship.raw_file())?,
                    PIVOT_TABLE_NS => {
                        pivot_table::read(
                            &mut parsed,
                            relationship.raw_file(),
                            raw.pivot_table_relationships(),
                            raw.pivot_cache_relationships(),
                        )?;
                    }
                    _ => {}
                }
            }
            for relationship in relationships.relationship_list() {
                if relationship.get_type() == VML_DRAWING_NS {
                    vml_drawing::read(
                        &mut parsed,
                        relationship.raw_file(),
                        raw.vml_drawing_relationships(),
                    )?;
                }
            }
        }
        Ok(())
    })();
    if chart_parts_added {
        provider.finish_sheet(&sheet_session);
    }
    match result {
        Ok(()) => {
            session.commit_sheet(&sheet_session);
            *worksheet = parsed;
            Ok(())
        }
        Err(error) => {
            if chart_parts_added {
                for (target, donor) in chart_donors {
                    let recovered = provider.recover_part(&target, &sheet_session);
                    if let (Some((list_index, relationship_index)), Some(data)) = (donor, recovered)
                    {
                        let file = raw.relationships_list_mut()[list_index].relationship_list_mut()
                            [relationship_index]
                            .raw_file_mut();
                        *file.file_data_mut() = data;
                    }
                }
            }
            worksheet.set_raw_data_of_worksheet(raw);
            Err(error)
        }
    }
}

fn materialize_eager_sheets<R: io::Read + io::Seek>(
    book: &mut Workbook,
    archive: &mut zip::read::ZipArchive<R>,
) -> Result<(), XlsxError> {
    let shared_string_table = book.shared_string_table();
    let stylesheet = book.stylesheet().clone();
    let mut session = SourceCacheSession::deferred();
    let mut provider = EagerChartPartProvider::new(archive);
    let mut deserialized = Vec::new();
    for (index, worksheet) in book.sheet_collection_unchecked_mut().iter_mut().enumerate() {
        if !worksheet.is_deserialized() {
            raw_to_deserialize_by_worksheet(
                worksheet,
                &shared_string_table,
                &stylesheet,
                &mut session,
                &mut provider,
            )?;
            deserialized.push(index);
        }
    }
    capture_chart_source_caches(book, &deserialized, &mut session, &mut provider);
    Ok(())
}
