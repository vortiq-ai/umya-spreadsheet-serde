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
        WORKSHEET_NS,
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
mod slicer;
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

/// The relationship ids named by the worksheet's `legacyDrawing` and
/// `legacyDrawingHF` elements.
fn legacy_drawing_ids(sheet_xml: &[u8]) -> Result<(Option<String>, Option<String>), XlsxError> {
    use quick_xml::events::Event;

    let mut reader = quick_xml::Reader::from_reader(sheet_xml);
    let (mut legacy, mut header_footer) = (None, None);
    loop {
        let event = reader.read_event()?;
        match &event {
            Event::Start(e) | Event::Empty(e) => {
                let slot = match e.local_name().as_ref() {
                    b"legacyDrawing" => &mut legacy,
                    b"legacyDrawingHF" => &mut header_footer,
                    _ => continue,
                };
                let mut id = None;
                for attribute in e.attributes() {
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
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok((legacy, header_footer))
}

#[cfg(test)]
thread_local! {
    /// The worksheet parts scanned for metadata cells, in scan order.
    static SCANNED_SHEET_PARTS: std::cell::RefCell<Vec<(&'static str, String)>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// The reference (empty when it has none) of the first cell of the worksheet
/// part that carries a `cm` attribute. Only cells in the `SpreadsheetML` main
/// namespace and unqualified attributes count: namespace declarations such as
/// `xmlns:cm`, foreign-namespace attributes and foreign elements named `c`
/// do not.
fn first_metadata_cell<R: io::Read>(sheet: R) -> Result<Option<String>, XlsxError> {
    use quick_xml::{
        events::Event,
        name::ResolveResult,
    };

    let mut reader = quick_xml::NsReader::from_reader(io::BufReader::new(sheet));
    let mut buffer = Vec::new();
    loop {
        let (namespace, event) = reader.read_resolved_event_into(&mut buffer)?;
        match event {
            Event::Start(e) | Event::Empty(e)
                if e.local_name().as_ref() == b"c"
                    && matches!(
                        namespace,
                        ResolveResult::Bound(ns)
                            if ns.as_ref() == crate::helper::const_str::SHEET_MAIN_NS.as_bytes()
                    ) =>
            {
                let mut reference = String::new();
                let mut has_metadata = false;
                for item in e.attributes() {
                    let item = item.map_err(crate::xml::xml_error)?;
                    let key = item.key.as_ref();
                    if key != b"cm" && key != b"r" {
                        continue;
                    }
                    let value = item
                        .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                        .map_err(crate::xml::xml_error)?
                        .into_owned();
                    if key == b"cm" {
                        has_metadata = true;
                    } else {
                        reference = value;
                    }
                }
                if has_metadata {
                    return Ok(Some(reference));
                }
            }
            Event::Eof => return Ok(None),
            _ => {}
        }
        buffer.clear();
    }
}

/// Every worksheet part of the package as `(rel target, part path)`.
fn worksheet_part_paths(workbook_rel: &[(String, String, String)]) -> Vec<String> {
    workbook_rel
        .iter()
        .filter(|(_, rel_type, _)| rel_type == WORKSHEET_NS)
        .map(|(_, _, target)| driver::join_paths("xl", target))
        .collect()
}

/// What one worksheet part uses of styles.
#[derive(Default)]
struct StyleUse {
    /// The first cell `s` other than 0, as (cell reference, value).
    cell:   Option<(String, String)>,
    /// The first column `style` other than 0, as (first column, value).
    column: Option<(String, String)>,
}

/// Reads the style references of a worksheet part in one namespace-resolved
/// stream: `s` of `c` and `style` of `col`, both in the
/// `SpreadsheetML` main namespace and unqualified.
fn style_use<R: io::Read>(sheet: R) -> Result<StyleUse, XlsxError> {
    use quick_xml::{
        events::Event,
        name::ResolveResult,
    };

    let mut found = StyleUse::default();
    let mut reader = quick_xml::NsReader::from_reader(io::BufReader::new(sheet));
    let mut buffer = Vec::new();
    loop {
        buffer.clear();
        let (namespace, event) = reader.read_resolved_event_into(&mut buffer)?;
        match event {
            Event::Start(e) | Event::Empty(e)
                if matches!(
                    namespace,
                    ResolveResult::Bound(ns)
                        if ns.as_ref() == crate::helper::const_str::SHEET_MAIN_NS.as_bytes()
                ) =>
            {
                let (identity, attribute): (&[u8], &[u8]) = match e.local_name().as_ref() {
                    b"c" => (b"r", b"s"),
                    b"col" => (b"min", b"style"),
                    _ => continue,
                };
                let (mut id, mut value) = (String::new(), None);
                for item in e.attributes() {
                    let item = item.map_err(crate::xml::xml_error)?;
                    let key = item.key.as_ref();
                    if key != attribute && key != identity {
                        continue;
                    }
                    let text = item
                        .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                        .map_err(crate::xml::xml_error)?
                        .into_owned();
                    if key == attribute {
                        value = Some(text);
                    } else {
                        id = text;
                    }
                }
                let Some(value) = value else {
                    continue;
                };
                // Excel ignores a style it cannot read as an unsigned 32-bit
                // index (`abc`, `-1`, `4294967296`).
                if value.parse::<u32>().is_ok_and(|index| index != 0) {
                    let slot = if e.local_name().as_ref() == b"c" {
                        &mut found.cell
                    } else {
                        &mut found.column
                    };
                    slot.get_or_insert((id, value));
                }
            }
            Event::Eof => return Ok(found),
            _ => {}
        }
    }
}

/// Excel opens a package without a styles part only while every cell and
/// column uses style 0: any cell `s` or column `style` of 1 or more makes it
/// refuse the file, so import refuses it too. (Row styles are ignored and
/// dropped by the worksheet boundary.) Every worksheet part is streamed once,
/// materialized or not.
fn validate_styles_without_styles_part<R: io::Read + io::Seek>(
    arv: &mut zip::ZipArchive<R>,
    workbook_rel: &[(String, String, String)],
) -> Result<(), XlsxError> {
    for path in worksheet_part_paths(workbook_rel) {
        #[cfg(test)]
        SCANNED_SHEET_PARTS.with(|scanned| scanned.borrow_mut().push(("styles", path.clone())));
        let used = style_use(driver::zip_by_name(arv, &path)?)?;
        if let Some((what, (id, value))) = used
            .cell
            .map(|cell| ("cell", cell))
            .or_else(|| used.column.map(|column| ("column", column)))
        {
            return Err(XlsxError::with_message(
                XlsxErrorKind::Xml,
                format!(
                    "the package has no styles part but {what} {id:?} of {path} uses style \
                     {value:?}"
                ),
            ));
        }
    }
    Ok(())
}

/// Excel opens a workbook with several metadata relationships only while no
/// cell uses the metadata; with a `cm` cell it rejects the workbook. Every
/// worksheet part is checked, materialized or not.
fn reject_duplicate_metadata_with_cell_use<R: io::Read + io::Seek>(
    arv: &mut zip::ZipArchive<R>,
    workbook_rel: &[(String, String, String)],
) -> Result<(), XlsxError> {
    for path in worksheet_part_paths(workbook_rel) {
        #[cfg(test)]
        SCANNED_SHEET_PARTS.with(|scanned| scanned.borrow_mut().push(("metadata", path.clone())));
        let part = driver::zip_by_name(arv, &path)?;
        if let Some(reference) = first_metadata_cell(part)? {
            return Err(XlsxError::with_message(
                XlsxErrorKind::Xml,
                format!(
                    "workbook declares more than one spreadsheet metadata relationship while cell \
                     {reference:?} of {path} uses cell metadata"
                ),
            ));
        }
    }
    Ok(())
}

/// Reads the bytes of the workbook connections part. The writer owns the
/// output part name, so any valid input name imports; the part must exist,
/// be declared with the connections content type and fit the size cap.
fn read_connections_part<R: io::Read + io::Seek>(
    arv: &mut zip::read::ZipArchive<R>,
    book: &Workbook,
    target: &str,
) -> Result<Vec<u8>, XlsxError> {
    let path = driver::join_paths("xl", target);
    let connections_error = |message: String| XlsxError::with_message(XlsxErrorKind::Xml, message);
    if book.package_content_types().get(&path)
        != Some(crate::structs::cell_metadata::CONNECTIONS_CONTENT_TYPE)
    {
        return Err(connections_error(format!(
            "connections part {path} is not declared with the connections content type"
        )));
    }
    read_capped(arv, &path, "Workbook connections exceed the size limit")
}

/// The bytes of a package part, rejected with `message` above the metadata
/// size cap.
fn read_capped<R: io::Read + io::Seek>(
    arv: &mut zip::read::ZipArchive<R>,
    path: &str,
    message: &'static str,
) -> Result<Vec<u8>, XlsxError> {
    let part = driver::zip_by_name(arv, path)?;
    let cap = crate::structs::cell_metadata::MAX_METADATA_BYTES;
    let too_big = || XlsxError::with_message(XlsxErrorKind::Xml, message);
    if part.size() > cap {
        return Err(too_big());
    }
    let mut xml = Vec::new();
    io::Read::read_to_end(&mut io::Read::take(part, cap + 1), &mut xml)?;
    if xml.len() as u64 > cap {
        return Err(too_big());
    }
    Ok(xml)
}

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
    let external_ids = workbook_rels::read_external_ids(&mut arv)?;
    slicer::read_workbook_slicer_caches(&mut arv, &mut book, &workbook_rel, &external_ids)?;

    let mut has_duplicate_metadata = false;
    for (_, rel_type, target) in &workbook_rel {
        if rel_type == crate::structs::cell_metadata::REL {
            // Excel opens a workbook with several metadata relationships when
            // no cell uses the metadata, and keeps one part: the first.
            if book.cell_metadata.is_some() {
                has_duplicate_metadata = true;
                continue;
            }
            let path = driver::join_paths("xl", target);
            let xml = read_capped(
                &mut arv,
                &path,
                "Spreadsheet metadata exceeds the size limit",
            )?;
            // The writer owns the output part name.
            book.cell_metadata = Some(crate::structs::cell_metadata::CellMetadata::from_xml(
                &xml,
                crate::structs::cell_metadata::METADATA_PATH.into(),
            )?);
        }
    }
    if has_duplicate_metadata {
        // One scan of the worksheet parts, however many relationships repeat.
        reject_duplicate_metadata_with_cell_use(&mut arv, &workbook_rel)?;
    }

    for (_, rel_type, target) in &workbook_rel {
        if rel_type == crate::structs::cell_metadata::CONNECTIONS_REL {
            if book.connections.is_some() {
                return Err(XlsxError::with_message(
                    XlsxErrorKind::Xml,
                    "workbook declares more than one connections relationship",
                ));
            }
            book.connections = Some(read_connections_part(&mut arv, &book, target)?);
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
    let has_styles_part = styles::read(&mut arv, &mut book)?;
    if !has_styles_part {
        validate_styles_without_styles_part(&mut arv, &workbook_rel)?;
    }

    for sheet in book.sheet_collection_unchecked_mut() {
        for (rel_id, _, rel_target) in &workbook_rel {
            if sheet.r_id() != rel_id {
                continue;
            }
            let mut raw_worksheet = RawWorksheet::default();
            // Like Excel, a package without a styles part drops the row
            // styles it ignores when the sheet is written.
            raw_worksheet.set_drops_row_styles(!has_styles_part);
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
    book.validate_slicers(None)?;

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

pub(crate) struct WorksheetMaterialization {
    original:       Worksheet,
    raw:            RawWorksheet,
    drawing_parts:  Option<crate::structs::drawing_parts::PreparedDrawingParts>,
    chart_donors:   std::collections::HashMap<String, Option<(usize, usize)>>,
    previous_parts: Vec<(String, Box<[u8]>)>,
}

impl WorksheetMaterialization {
    fn commit(mut self, worksheet: &mut Worksheet) {
        worksheet.set_drawing_parts(self.drawing_parts.map(|parts| parts.commit(&mut self.raw)));
    }

    fn rollback(
        mut self,
        worksheet: &mut Worksheet,
        session: &SourceCacheSession,
        provider: &mut dyn ChartPartProvider,
    ) {
        for (target, donor) in self.chart_donors {
            let recovered = provider.recover_part(&target, session);
            if let (Some((list_index, relationship_index)), Some(data)) = (donor, recovered) {
                *self.raw.relationships_list_mut()[list_index].relationship_list_mut()
                    [relationship_index]
                    .raw_file_mut()
                    .file_data_mut() = data;
            }
        }
        session.restore_parts(self.previous_parts);
        self.original.set_raw_data_of_worksheet(self.raw);
        *worksheet = self.original;
    }
}

fn rollback_worksheet_materializations(
    book: &mut Workbook,
    staged: Vec<(usize, WorksheetMaterialization)>,
    session: &SourceCacheSession,
    provider: &mut dyn ChartPartProvider,
) {
    provider.finish_sheet(session);
    for (index, transaction) in staged.into_iter().rev() {
        transaction.rollback(
            &mut book.sheet_collection_unchecked_mut()[index],
            session,
            provider,
        );
    }
}

pub(crate) fn finish_worksheet_materializations(
    book: &mut Workbook,
    staged: Vec<(usize, WorksheetMaterialization)>,
    session: &mut SourceCacheSession,
    provider: &mut dyn ChartPartProvider,
) -> Result<(), XlsxError> {
    let fresh: Vec<_> = staged.iter().map(|(index, _)| *index).collect();
    let report = *book.chart_source_cache_report();
    if let Err(error) = capture_chart_source_caches(book, &fresh, session, provider) {
        rollback_worksheet_materializations(book, staged, session, provider);
        *book.chart_source_cache_report_mut() = report;
        return Err(error);
    }
    if let Err(error) = book.validate_slicers(None) {
        rollback_worksheet_materializations(book, staged, session, provider);
        *book.chart_source_cache_report_mut() = report;
        return Err(error);
    }
    for (index, transaction) in staged {
        transaction.commit(&mut book.sheet_collection_unchecked_mut()[index]);
    }
    Ok(())
}

#[cfg(test)]
pub(crate) fn raw_to_deserialize_by_worksheet(
    worksheet: &mut Worksheet,
    shared_string_table: &RwLock<SharedStringTable>,
    stylesheet: &Stylesheet,
    session: &mut SourceCacheSession,
    provider: &mut dyn ChartPartProvider,
) -> Result<(), XlsxError> {
    if let Some(staged) = prepare_worksheet_materialization(
        worksheet,
        shared_string_table,
        stylesheet,
        session,
        provider,
    )? {
        staged.commit(worksheet);
    }
    Ok(())
}

pub(crate) fn prepare_worksheet_materialization(
    worksheet: &mut Worksheet,
    shared_string_table: &RwLock<SharedStringTable>,
    stylesheet: &Stylesheet,
    session: &mut SourceCacheSession,
    provider: &mut dyn ChartPartProvider,
) -> Result<Option<WorksheetMaterialization>, XlsxError> {
    if worksheet.is_deserialized() {
        return Ok(None);
    }

    let mut parsed = worksheet.materialization_target();
    let mut raw = worksheet.take_raw_data_of_worksheet();
    let mut sheet_session = session.for_sheet();
    let mut chart_parts_added = false;
    let mut chart_donors = std::collections::HashMap::new();
    let mut drawing_parts = None;
    let result = (|| {
        let shared_string_table = &*shared_string_table
            .read()
            .map_err(|error| crate::xml::xml_error(error.to_string()))?;
        worksheet::read(&mut parsed, &raw, shared_string_table, stylesheet)?;
        raw.load_relationship_file_data_from_source()?;
        slicer::read_worksheet_slicers(&mut parsed, &raw, &sheet_session)?;
        let mut chart_parts = std::collections::HashMap::new();
        for (list_index, relationships) in raw.relationships_list_mut().iter_mut().enumerate() {
            for (relationship_index, relationship) in
                relationships.relationship_list_mut().iter_mut().enumerate()
            {
                if !matches!(
                    relationship.get_type(),
                    crate::helper::const_str::CHART_NS
                        | crate::helper::const_str::CHART_EX_NS
                        | crate::helper::const_str::CHART_USER_SHAPES_NS
                ) {
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
        for target in chart_parts.keys() {
            if let Some(relationships) = raw.relationships_of_part(target)? {
                sheet_session.register_chart_relationships(target, relationships);
            }
        }
        provider.add_pending(chart_parts);
        chart_parts_added = true;

        let sheet_xml = raw.worksheet_file().file_data();
        let (legacy_id, header_footer_id) = if sheet_xml
            .windows(b"legacyDrawing".len())
            .any(|window| window == b"legacyDrawing")
        {
            legacy_drawing_ids(sheet_xml)?
        } else {
            (None, None)
        };
        for (element, id) in [
            ("legacyDrawing", &legacy_id),
            ("legacyDrawingHF", &header_footer_id),
        ] {
            let Some(id) = id else {
                continue;
            };
            let names_vml = raw.worksheet_relationships()?.is_some_and(|rels| {
                rels.relationship_list()
                    .iter()
                    .any(|rel| rel.id() == id && rel.get_type() == VML_DRAWING_NS)
            });
            if !names_vml {
                return Err(crate::xml::xml_error(format!(
                    "{element} r:id {id} does not name a VML drawing relationship"
                )));
            }
        }
        if let Some(relationships) = raw.worksheet_relationships()? {
            for relationship in relationships.relationship_list() {
                match relationship.get_type() {
                    DRAWINGS_NS => {
                        drawing::read(
                            &mut parsed,
                            relationship.raw_file(),
                            raw.relationships_of_part(relationship.raw_file().file_target())?,
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
                            &raw,
                            &sheet_session,
                        )?;
                    }
                    _ => {}
                }
            }
            for relationship in relationships.relationship_list() {
                if relationship.get_type() != VML_DRAWING_NS {
                    continue;
                }
                // A VML part is bound only through the element that names it.
                // Like Excel, a part named by neither element is left unbound.
                for (id, header_footer) in [(&legacy_id, false), (&header_footer_id, true)] {
                    if id.as_deref() == Some(relationship.id()) {
                        vml_drawing::read(
                            &mut parsed,
                            relationship.raw_file(),
                            raw.relationships_of_part(relationship.raw_file().file_target())?,
                            header_footer,
                            &sheet_session,
                        )?;
                    }
                }
            }
        }
        drawing_parts = crate::structs::drawing_parts::DrawingParts::prepare(
            &raw,
            &parsed,
            &sheet_session,
            provider,
        )?;
        if let Some(prepared) = &drawing_parts {
            prepared.bind_style_bundles(&mut parsed);
            prepared.bind_chart_drawing_parts(&raw, &mut parsed, &sheet_session)?;
        }
        Ok(())
    })();
    if chart_parts_added {
        provider.finish_sheet(&sheet_session);
    }
    match result {
        Ok(()) => {
            let previous_parts = session.commit_sheet(&sheet_session);
            let original = std::mem::replace(worksheet, parsed);
            Ok(Some(WorksheetMaterialization {
                original,
                raw,
                drawing_parts,
                chart_donors,
                previous_parts,
            }))
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
    session.set_content_types(book.package_content_types());
    session.set_pivot_cache_bindings(book.pivot_cache_bindings());
    let mut provider = EagerChartPartProvider::new(archive);
    let mut staged = Vec::new();
    for index in 0..book.sheet_collection_no_check().len() {
        if !book.sheet_collection_no_check()[index].is_deserialized() {
            match prepare_worksheet_materialization(
                &mut book.sheet_collection_unchecked_mut()[index],
                &shared_string_table,
                &stylesheet,
                &mut session,
                &mut provider,
            ) {
                Ok(Some(transaction)) => staged.push((index, transaction)),
                Ok(None) => (),
                Err(error) => {
                    rollback_worksheet_materializations(book, staged, &session, &mut provider);
                    return Err(error);
                }
            }
        }
    }
    finish_worksheet_materializations(book, staged, &mut session, &mut provider)?;
    Ok(())
}

#[cfg(test)]
mod metadata_scan_tests {
    use std::io::{
        Cursor,
        Write,
    };

    use super::SCANNED_SHEET_PARTS;

    const MAIN: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
    const REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
    const PKG: &str = "http://schemas.openxmlformats.org/package/2006/relationships";

    fn package(metadata_relationships: usize) -> Vec<u8> {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let mut workbook_rels = format!(
            r#"<Relationship Id="s1" Type="{REL}/worksheet" Target="worksheets/sheet1.xml"/><Relationship Id="s2" Type="{REL}/worksheet" Target="worksheets/sheet2.xml"/>"#
        );
        for index in 0..metadata_relationships {
            workbook_rels.push_str(&format!(
                r#"<Relationship Id="m{index}" Type="{REL}/sheetMetadata" Target="metadata.xml"/>"#
            ));
        }
        let sheet = format!(
            r#"<worksheet xmlns="{MAIN}"><sheetData><row r="1"><c r="A1"><v>1</v></c></row></sheetData></worksheet>"#
        );
        let parts = [
            (
                "[Content_Types].xml",
                format!(
                    r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/><Override PartName="/xl/worksheets/sheet2.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/><Override PartName="/xl/metadata.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheetMetadata+xml"/></Types>"#
                ),
            ),
            (
                "_rels/.rels",
                format!(
                    r#"<Relationships xmlns="{PKG}"><Relationship Id="o" Type="{REL}/officeDocument" Target="xl/workbook.xml"/></Relationships>"#
                ),
            ),
            (
                "xl/workbook.xml",
                format!(
                    r#"<workbook xmlns="{MAIN}" xmlns:r="{REL}"><sheets><sheet name="A" sheetId="1" r:id="s1"/><sheet name="B" sheetId="2" r:id="s2"/></sheets></workbook>"#
                ),
            ),
            (
                "xl/_rels/workbook.xml.rels",
                format!(r#"<Relationships xmlns="{PKG}">{workbook_rels}</Relationships>"#),
            ),
            ("xl/worksheets/sheet1.xml", sheet.clone()),
            ("xl/worksheets/sheet2.xml", sheet),
            ("xl/metadata.xml", format!(r#"<metadata xmlns="{MAIN}"/>"#)),
        ];
        for (name, body) in parts {
            zip.start_file(name, zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(body.as_bytes()).unwrap();
        }
        zip.finish().unwrap().into_inner()
    }

    /// The worksheet parts scanned for `purpose`, in scan order.
    fn scanned_parts(purpose: &str) -> Vec<String> {
        SCANNED_SHEET_PARTS.with(|scanned| {
            scanned
                .borrow()
                .iter()
                .filter(|(scan, _)| *scan == purpose)
                .map(|(_, path)| path.clone())
                .collect()
        })
    }

    /// A package without a styles part is validated with one stream per
    /// worksheet part, whatever else is scanned.
    #[test]
    fn every_worksheet_part_is_scanned_once_for_styles_without_a_styles_part() {
        for relationships in [1, 50] {
            for eager in [true, false] {
                SCANNED_SHEET_PARTS.with(|scanned| scanned.borrow_mut().clear());
                super::read_reader(Cursor::new(package(relationships)), eager).unwrap();
                let scanned = scanned_parts("styles");
                assert_eq!(
                    scanned,
                    ["xl/worksheets/sheet1.xml", "xl/worksheets/sheet2.xml"],
                    "{relationships} relationships eager={eager}"
                );
            }
        }
    }

    /// However many metadata relationships repeat, each worksheet part is
    /// scanned for metadata cells once, for eager and lazy import.
    #[test]
    fn every_worksheet_part_is_scanned_once_however_many_metadata_relationships() {
        for relationships in [1, 2, 50] {
            for eager in [true, false] {
                SCANNED_SHEET_PARTS.with(|scanned| scanned.borrow_mut().clear());
                super::read_reader(Cursor::new(package(relationships)), eager).unwrap();
                let scanned = scanned_parts("metadata");
                let expected: Vec<String> = if relationships > 1 {
                    vec![
                        "xl/worksheets/sheet1.xml".into(),
                        "xl/worksheets/sheet2.xml".into(),
                    ]
                } else {
                    Vec::new()
                };
                assert_eq!(
                    scanned, expected,
                    "{relationships} relationships eager={eager}"
                );
            }
        }
    }
}
