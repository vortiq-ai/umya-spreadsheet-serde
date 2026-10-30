//! Streaming xlsx writer.
//! Streaming writer provide us a way to incrementally write `WorkSheet` data
//! one at a time. This is useful in situation where a file has a large number
//! of Worksheet, that cannot all fit into memory.
//!
//! Limitiation: a `WorkSheet`, once flushed, cannot be modified or read back
//! into memory.
//!
//! # Example
//! ```no_run
//! use std::io::Cursor;
//!
//! use umya_spreadsheet::{
//!     new_file,
//!     writer::streaming_writer::StreamingWriter,
//! };
//!
//! let mut book = new_file();
//! let _ = book.new_sheet("Sheet2");
//! let zip_writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
//! let mut sw = StreamingWriter::new(zip_writer, book);
//! let sheet = sw.take_sheet("Sheet1").unwrap();
//! sw.flush_sheet(sheet).unwrap();
//! let sheet = sw.take_sheet("Sheet2").unwrap();
//! sw.flush_sheet(sheet).unwrap();
//! let _writer = sw.finish().unwrap();
//! ```
use std::{
    collections::{
        HashMap,
        HashSet,
    },
    io,
};

use crate::{
    Stylesheet,
    structs::{
        Workbook,
        Worksheet,
        WriterManager,
        XlsxError,
    },
    writer::xlsx::{
        chart,
        comment,
        content_types,
        doc_props_app,
        doc_props_core,
        doc_props_custom,
        drawing,
        drawing_rels,
        embeddings,
        jsa_project_bin,
        media,
        person,
        pivot_cache,
        pivot_table,
        printer_settings,
        rels,
        shared_strings,
        styles,
        table,
        theme,
        threaded_comment,
        vba_project_bin,
        vml_drawing,
        vml_drawing_rels,
        workbook,
        workbook_rels,
        worksheet,
        worksheet_rels,
    },
};

pub struct StreamingWriter<W: io::Write + io::Seek> {
    writer_manager: WriterManager<W>,
    work_book:      Workbook,

    available:       Vec<Worksheet>,
    stylesheet:      Stylesheet,
    has_macros:      bool,
    sheet_no:        i32,
    source_order:    HashMap<String, usize>,
    preflight_error: Option<XlsxError>,
    poisoned:        bool,
}

impl<W: io::Write + io::Seek> StreamingWriter<W> {
    pub fn new(zip_writer: zip::ZipWriter<W>, mut work_book: Workbook) -> Self {
        let stylesheet = work_book.stylesheet().clone();
        let has_macros = work_book.has_macros();
        let mut writer_manager = WriterManager::new(zip_writer);
        let preflight_error = writer_manager.reserve_table_ids(&work_book).err();
        writer_manager.reserve_drawing_parts(&work_book);
        let available = work_book.take_all_sheets();
        let source_order = available
            .iter()
            .enumerate()
            .map(|(index, sheet)| (sheet.sheet_id().into(), index))
            .collect();
        Self {
            writer_manager,
            work_book,
            available,
            stylesheet,
            has_macros,
            sheet_no: 0,
            source_order,
            preflight_error,
            poisoned: false,
        }
    }

    // finish the final part in writer::xlsx, the part after looping through all
    // sheet.
    // return owner ship of our writer to caller
    // also consume our write manager
    pub fn finish(mut self) -> Result<W, XlsxError> {
        if let Some(error) = self.preflight_error.take() {
            return Err(error);
        }
        if self.poisoned {
            return Err(streaming_error("cannot finish after a failed sheet flush"));
        }
        // from xlsx::write_zip_to_writer
        // Add docProps
        doc_props_app::write(&self.work_book, &mut self.writer_manager)?;
        doc_props_core::write(&self.work_book, &mut self.writer_manager)?;
        doc_props_custom::write(&self.work_book, &mut self.writer_manager)?;
        vba_project_bin::write(&self.work_book, &mut self.writer_manager)?;
        jsa_project_bin::write(&self.work_book, &mut self.writer_manager)?;
        rels::write(&self.work_book, &mut self.writer_manager)?;
        theme::write(self.work_book.theme(), &mut self.writer_manager)?;
        person::write(&self.work_book, &mut self.writer_manager)?;

        self.writer_manager.file_list_sort();
        shared_strings::write(
            &self.work_book.shared_string_table(),
            &mut self.writer_manager,
        )?;
        styles::write(&self.stylesheet, &mut self.writer_manager)?;
        workbook::write(&self.work_book, &mut self.writer_manager)?;

        let has_shared_string_table = self
            .work_book
            .shared_string_table()
            .read()
            .unwrap()
            .has_value();
        workbook_rels::write(
            &self.work_book,
            has_shared_string_table,
            &mut self.writer_manager,
        )?;
        content_types::write(&self.work_book, &mut self.writer_manager)?;

        Ok(self.writer_manager.finish()?)
    }

    pub fn take_sheet(&mut self, name: &str) -> Option<Worksheet> {
        let pos = self.available.iter().position(|ws| ws.name() == name)?;
        Some(self.available.remove(pos))
    }

    /// Writes and consumes a worksheet. Flushed cells are no longer available
    /// for editing.
    pub fn flush_sheet(&mut self, mut worksheet: Worksheet) -> Result<(), XlsxError> {
        if let Some(error) = self.preflight_error.take() {
            self.poisoned = true;
            return Err(error);
        }
        if self.poisoned {
            return Err(streaming_error("cannot flush after a failed sheet flush"));
        }
        let name = worksheet.name().to_owned();
        if self
            .work_book
            .sheet_collection_no_check()
            .iter()
            .any(|sheet| sheet.name().eq_ignore_ascii_case(&name))
        {
            return Err(streaming_error("worksheet was already flushed"));
        }
        if let Some(index) = self
            .available
            .iter()
            .position(|sheet| sheet.name().eq_ignore_ascii_case(&name))
        {
            self.available.remove(index);
        }
        let mut ids = HashSet::new();
        for sheet in self
            .work_book
            .sheet_collection_no_check()
            .iter()
            .chain(&self.available)
        {
            let id = sheet
                .sheet_id()
                .parse::<u32>()
                .map_err(|_| streaming_error("invalid worksheet id"))?;
            ids.insert(id);
        }
        let requested = if worksheet.sheet_id().is_empty() {
            None
        } else {
            Some(
                worksheet
                    .sheet_id()
                    .parse::<u32>()
                    .map_err(|_| streaming_error("invalid worksheet id"))?,
            )
        };
        if requested.is_none_or(|id| ids.contains(&id)) {
            let mut id = 1u32;
            while ids.contains(&id) {
                id = id
                    .checked_add(1)
                    .ok_or_else(|| streaming_error("worksheet ids exhausted"))?;
            }
            worksheet.set_sheet_id(id.to_string());
        }
        let flushed = self.work_book.take_all_sheets();
        let unavailable: Vec<String> = flushed.iter().map(|sheet| sheet.name().into()).collect();
        let mut context = flushed;
        context.append(&mut self.available);
        context.push(worksheet);
        let mut names = HashSet::new();
        if context
            .iter()
            .any(|sheet| !names.insert(sheet.name().to_lowercase()))
        {
            self.poisoned = true;
            self.available = context;
            return Err(streaming_error(
                "duplicate worksheet names in streaming context",
            ));
        }
        context.sort_by_key(|sheet| {
            self.source_order
                .get(sheet.sheet_id())
                .copied()
                .unwrap_or(usize::MAX)
        });
        let index = context
            .iter()
            .position(|sheet| sheet.name() == name)
            .unwrap();
        drop(self.work_book.replace_sheet_collection(context));
        let result = self.flush_context(index, &unavailable);
        let mut context = self.work_book.take_all_sheets();
        let current = context.remove(index);
        let mut flushed = Vec::new();
        for sheet in context {
            if unavailable.iter().any(|name| name == sheet.name()) {
                flushed.push(sheet);
            } else {
                self.available.push(sheet);
            }
        }
        flushed.sort_by_key(|sheet| {
            unavailable
                .iter()
                .position(|name| name == sheet.name())
                .unwrap()
        });
        match result {
            Ok(stub) => {
                flushed.push(stub);
                drop(self.work_book.replace_sheet_collection(flushed));
                Ok(())
            }
            Err(error) => {
                self.available.push(current);
                drop(self.work_book.replace_sheet_collection(flushed));
                self.poisoned = true;
                Err(error)
            }
        }
    }

    fn flush_context(
        &mut self,
        index: usize,
        unavailable: &[String],
    ) -> Result<Worksheet, XlsxError> {
        self.work_book.sheet_mut(index)?;
        let worksheet = self.work_book.sheet(index)?;
        self.sheet_no += 1;
        // TODO: allow caller to specify worksheet number
        let worksheet_no = self.sheet_no;

        worksheet::write(
            worksheet_no,
            worksheet,
            &self.work_book.shared_string_table(),
            &mut self.stylesheet,
            self.has_macros,
            &mut self.writer_manager,
        )?;

        let chart_parts = chart::write_parts(
            worksheet,
            &self.work_book,
            &mut self.writer_manager,
            unavailable,
        )?;

        // Add drawing and its relationships
        let (drawing_no, rel_list) = drawing::write(worksheet, &mut self.writer_manager)?;
        drawing_rels::write(
            worksheet,
            &drawing_no,
            &chart_parts,
            &rel_list,
            &mut self.writer_manager,
        )?;

        // Add vml drawing and its relationships
        let (vml_drawing_no, rel_list) = vml_drawing::write(worksheet, &mut self.writer_manager)?;
        vml_drawing_rels::write(
            worksheet,
            &vml_drawing_no,
            &rel_list,
            &mut self.writer_manager,
        )?;

        // Add comments
        let comment_no = comment::write(worksheet, &mut self.writer_manager)?;

        // Add threaded_comment
        let threaded_comment_no = threaded_comment::write(worksheet, &mut self.writer_manager)?;

        // Add ole_object and excel
        let (ole_object_no_list, excel_no_list) =
            embeddings::write(worksheet, &mut self.writer_manager)?;

        // Add Media
        media::write(worksheet, &mut self.writer_manager)?;
        drawing::write_dependencies(worksheet, &mut self.writer_manager)?;

        // Add printer settings
        let printer_settings_no = worksheet
            .page_setup()
            .object_data()
            .map_or_else(String::new, |_| {
                printer_settings::write(worksheet, &mut self.writer_manager).unwrap_or_default()
            });

        // Add tables
        let table_no_list = table::write(worksheet, &mut self.writer_manager)?;

        // Add pivot tables and caches
        let pivot_table_no_list = pivot_table::write(worksheet, &mut self.writer_manager)?;
        let pivot_cache_no_list = pivot_cache::write(worksheet, &mut self.writer_manager)?;

        // Add worksheet relationships
        worksheet_rels::write(
            worksheet,
            &worksheet_no.to_string(),
            &drawing_no,
            &vml_drawing_no,
            &comment_no,
            &threaded_comment_no,
            &ole_object_no_list,
            &excel_no_list,
            &printer_settings_no,
            &table_no_list,
            &pivot_table_no_list,
            &pivot_cache_no_list,
            &mut self.writer_manager,
        )?;

        let mut stub = Worksheet::default();
        stub.set_name(worksheet.name());
        stub.set_sheet_id(worksheet.sheet_id());
        if worksheet.has_state() {
            stub.set_state_str(worksheet.state_str());
        }
        let defined = worksheet.defined_names();
        if !defined.is_empty() {
            let mut defined = defined.to_vec();
            for name in &mut defined {
                if name.has_local_sheet_id() {
                    name.set_local_sheet_id(
                        u32::try_from(self.sheet_no - 1)
                            .map_err(|_| streaming_error("sheet index exceeds u32"))?,
                    );
                }
            }
            stub.set_defined_names(defined);
        }
        Ok(stub)
    }
}

fn streaming_error(message: &'static str) -> XlsxError {
    XlsxError::with_message(crate::XlsxErrorKind::InvalidArgument, message)
}
