use std::{
    collections::{
        HashMap,
        HashSet,
    },
    fs::File,
    io,
    io::Cursor,
    path::{
        Path,
        PathBuf,
    },
    time::SystemTime,
};

use quick_xml::Writer;
use sha2::{
    Digest,
    Sha256,
};
use zip::result::ZipResult;

use crate::{
    helper::const_str::{
        CHART_EX_TYPE,
        CHART_TYPE,
        COMMENTS_TYPE,
        CORE_PROPS_TYPE,
        CUSTOM_PROPS_TYPE,
        DRAWING_TYPE,
        OLE_OBJECT_TYPE,
        PIVOT_CACHE_DEF_TYPE,
        PIVOT_CACHE_REC_TYPE,
        PIVOT_TABLE_TYPE,
        PKG_CHARTS,
        PKG_DRAWINGS,
        PKG_EMBEDDINGS,
        PKG_PRNTR_SETTINGS,
        PKG_TABLES,
        SHARED_STRINGS_TYPE,
        SHEET_TYPE,
        STYLES_TYPE,
        TABLE_TYPE,
        THEME_TYPE,
        VBA_TYPE,
        WORKBOOK_MACRO_TYPE,
        WORKBOOK_TYPE,
        XPROPS_TYPE,
    },
    reader::driver::zip_by_name,
    structs::{
        Workbook,
        XlsxError,
    },
    writer::driver::make_file_from_bin,
};
pub struct WriterManager<W: io::Seek + io::Write> {
    files:                 Vec<String>,
    written_parts:         HashMap<String, WrittenPart>,
    reserved_parts:        HashSet<String>,
    content_types:         HashMap<String, String>,
    arv:                   zip::ZipWriter<W>,
    is_light:              bool,
    table_no:              i32,
    table_ids:             HashSet<u32>,
    written_table_ids:     HashSet<u32>,
    reserved_table_parts:  HashSet<String>,
    pivot_table_no:        i32,
    pivot_cache_hash_list: Vec<String>,
}

impl<W: io::Seek + io::Write> WriterManager<W> {
    #[inline]
    pub fn new(arv: zip::ZipWriter<W>) -> Self {
        WriterManager {
            files: Vec::new(),
            written_parts: HashMap::new(),
            reserved_parts: HashSet::new(),
            content_types: HashMap::new(),
            arv,
            is_light: false,
            table_no: 0,
            table_ids: HashSet::new(),
            written_table_ids: HashSet::new(),
            reserved_table_parts: HashSet::new(),
            pivot_table_no: 0,
            pivot_cache_hash_list: Vec::new(),
        }
    }

    #[inline]
    pub fn finish(self) -> ZipResult<W> {
        self.arv.finish()
    }

    #[inline]
    pub fn set_is_light(&mut self, value: bool) -> &mut Self {
        self.is_light = value;
        self
    }

    #[inline]
    #[must_use]
    pub fn get_is_light(&self) -> bool {
        self.is_light
    }

    #[inline]
    #[must_use]
    pub fn get_num_tables(&self) -> i32 {
        self.table_no
    }

    #[inline]
    pub fn next_table_no(&mut self) -> i32 {
        loop {
            self.table_no += 1;
            let path = format!("{PKG_TABLES}/table{}.xml", self.table_no);
            if !self.reserved_table_parts.contains(&path) {
                return self.table_no;
            }
        }
    }

    pub(crate) fn reserve_table_ids(&mut self, workbook: &Workbook) -> Result<(), XlsxError> {
        for sheet in workbook.sheet_collection_no_check() {
            if sheet.is_deserialized() {
                for table in sheet.tables() {
                    if let Some(id) = table.id() {
                        self.reserve_table_id(id)?;
                    }
                }
            } else {
                for relationships in sheet.raw_data_of_worksheet().relationships_list() {
                    for relationship in relationships.relationship_list() {
                        if relationship.get_type() != crate::helper::const_str::TABLE_NS {
                            continue;
                        }
                        let mut file = relationship.raw_file().clone();
                        if !self.reserved_table_parts.insert(file.file_target().into()) {
                            return Err(table_identity_error("table part has multiple owners"));
                        }
                        file.load_file_data_from_source()?;
                        let id = raw_table_id(file.file_data())?;
                        self.reserve_table_id(id)?;
                    }
                }
            }
        }
        Ok(())
    }

    fn reserve_table_id(&mut self, id: u32) -> Result<(), XlsxError> {
        if !self.table_ids.insert(id) {
            return Err(table_identity_error(format!("duplicate table id: {id}")));
        }
        Ok(())
    }

    pub(crate) fn table_id(&mut self, imported: Option<u32>) -> Result<u32, XlsxError> {
        let id = if let Some(id) = imported {
            id
        } else {
            let mut next = 1u32;
            while self.table_ids.contains(&next) {
                next = next
                    .checked_add(1)
                    .ok_or_else(|| table_identity_error("table ids exhausted"))?;
            }
            next
        };
        if !self.written_table_ids.insert(id) {
            return Err(table_identity_error(format!(
                "duplicate written table id: {id}"
            )));
        }
        self.table_ids.insert(id);
        Ok(id)
    }

    #[inline]
    pub fn next_pivot_table_no(&mut self) -> i32 {
        self.pivot_table_no += 1;
        self.pivot_table_no
    }

    #[inline]
    pub fn get_pivot_cache_no(&mut self, hash: &str) -> (bool, i32) {
        if let Some(v) = self.pivot_cache_hash_list.iter().position(|x| x == hash) {
            (true, i32::try_from(v + 1).unwrap())
        } else {
            self.pivot_cache_hash_list.push(hash.to_string());
            (
                false,
                i32::try_from(self.pivot_cache_hash_list.len()).unwrap(),
            )
        }
    }

    pub(crate) fn reserve_drawing_parts(&mut self, wb: &Workbook) {
        for sheet in wb.sheet_collection_no_check() {
            if sheet.is_deserialized() {
                if let Some(parts) = sheet.drawing_parts() {
                    parts.reserve(self);
                }
            } else {
                for relationships in sheet.raw_data_of_worksheet().relationships_list() {
                    self.reserve_part(relationships.file_target());
                    for edge in relationships.relationship_list() {
                        if edge.target_mode() != "External" {
                            self.reserve_part(edge.raw_file().file_target());
                        }
                    }
                }
            }
        }
    }

    pub(crate) fn reserve_part(&mut self, target: &str) {
        self.reserved_parts.insert(target.to_owned());
    }

    pub(crate) fn add_typed_bin(
        &mut self,
        target: &str,
        content_type: &str,
        data: &[u8],
    ) -> Result<(), XlsxError> {
        self.register_content_type(target, content_type)?;
        self.add_bin(target, data)
    }

    pub(crate) fn register_content_type(
        &mut self,
        target: &str,
        content_type: &str,
    ) -> Result<(), XlsxError> {
        super::content_types::validate_content_type(content_type).map_err(part_error)?;
        if self
            .content_types
            .get(target)
            .is_some_and(|previous| previous != content_type)
        {
            return Err(part_error(format!(
                "conflicting content types for {target}"
            )));
        }
        self.content_types
            .insert(target.into(), content_type.into());
        Ok(())
    }

    fn identical_part(&self, target: &str, candidate: &WrittenPart) -> Result<bool, XlsxError> {
        let Some(previous) = self.written_parts.get(target) else {
            return Ok(false);
        };
        if previous.digest()? != candidate.digest()? {
            return Err(part_error(format!(
                "different package payloads target {target}"
            )));
        }
        Ok(true)
    }

    pub(crate) fn add_writer(
        &mut self,
        target: &str,
        writer: Writer<Cursor<Vec<u8>>>,
    ) -> Result<(), XlsxError> {
        self.add_bin(target, &writer.into_inner().into_inner())
    }

    pub(crate) fn add_bin(&mut self, target: &str, data: &[u8]) -> Result<(), XlsxError> {
        let identity = WrittenPart::Bytes(Sha256::digest(data).into());
        if self.identical_part(target, &identity)? {
            return Ok(());
        }
        make_file_from_bin(target, &mut self.arv, data, None, self.is_light)?;
        self.files.push(target.to_string());
        self.written_parts.insert(target.to_string(), identity);
        Ok(())
    }

    pub(crate) fn add_raw_copy<P: AsRef<Path>>(
        &mut self,
        target: &str,
        source_path: P,
        source_target: &str,
    ) -> Result<(), XlsxError> {
        let path = source_path.as_ref();
        let file = File::open(path)?;
        let metadata = file.metadata()?;
        let identity = WrittenPart::Raw(Box::new(RawPartSource {
            path:        path.into(),
            target:      source_target.into(),
            file_length: metadata.len(),
            modified:    metadata.modified()?,
        }));
        if self.identical_part(target, &identity)? {
            return Ok(());
        }
        let mut archive = zip::read::ZipArchive::new(file)?;
        let source = zip_by_name(&mut archive, source_target)?;
        self.arv.raw_copy_file_rename(source, target)?;
        self.files.push(target.to_string());
        self.written_parts.insert(target.to_string(), identity);
        Ok(())
    }

    pub(crate) fn add_extended_chart(
        &mut self,
        preferred: Option<&str>,
        data: &[u8],
    ) -> Result<String, XlsxError> {
        self.add_chart_part(preferred, data, CHART_EX_TYPE, "chartEx")
    }

    pub(crate) fn add_standard_chart(
        &mut self,
        preferred: Option<&str>,
        data: &[u8],
    ) -> Result<String, XlsxError> {
        self.add_chart_part(preferred, data, CHART_TYPE, "chart")
    }

    fn add_chart_part(
        &mut self,
        preferred: Option<&str>,
        data: &[u8],
        content_type: &str,
        prefix: &str,
    ) -> Result<String, XlsxError> {
        if let Some(target) = preferred {
            if target.starts_with('/')
                || target.contains('\\')
                || target
                    .split('/')
                    .any(|part| part.is_empty() || matches!(part, "." | ".."))
            {
                return Err(part_error("invalid chart part target"));
            }
            if let Some(previous) = self.written_parts.get(target) {
                if previous.digest()? == <[u8; 32]>::from(Sha256::digest(data)) {
                    self.add_typed_bin(target, content_type, data)?;
                    return Ok(target.into());
                }
            } else {
                self.add_typed_bin(target, content_type, data)?;
                return Ok(target.into());
            }
        }
        let mut number = 1u32;
        loop {
            let target = format!("xl/charts/{prefix}{number}.xml");
            if !self.reserved_parts.contains(&target) && !self.written_parts.contains_key(&target) {
                self.add_typed_bin(&target, content_type, data)?;
                return Ok(target);
            }
            number = number
                .checked_add(1)
                .ok_or_else(|| part_error("chart part numbers exhausted"))?;
        }
    }

    #[inline]
    pub(crate) fn get_arv_mut(&mut self) -> &mut zip::ZipWriter<W> {
        &mut self.arv
    }

    #[inline]
    pub(crate) fn file_list_sort(&mut self) -> &mut Self {
        self.files.sort();
        self
    }

    #[inline]
    pub(crate) fn check_file_exist(&mut self, file_path: &str) -> bool {
        self.file_list_sort();
        self.files.iter().any(|file| file == file_path)
    }

    pub(crate) fn add_file_at_drawing(
        &mut self,
        writer: Writer<Cursor<Vec<u8>>>,
    ) -> Result<i32, XlsxError> {
        let mut index = 0;
        loop {
            index += 1;
            let file_path = format!("{PKG_DRAWINGS}/drawing{index}.xml");
            if !self.reserved_parts.contains(&file_path) && !self.check_file_exist(&file_path) {
                self.add_writer(&file_path, writer)?;
                return Ok(index);
            }
        }
    }

    pub(crate) fn add_file_at_vml_drawing(
        &mut self,
        writer: Writer<Cursor<Vec<u8>>>,
    ) -> Result<i32, XlsxError> {
        let mut index = 0;
        loop {
            index += 1;
            let file_path = format!("{PKG_DRAWINGS}/vmlDrawing{index}.vml");
            if !self.reserved_parts.contains(&file_path) && !self.check_file_exist(&file_path) {
                self.add_writer(&file_path, writer)?;
                return Ok(index);
            }
        }
    }

    pub(crate) fn add_file_at_comment(
        &mut self,
        writer: Writer<Cursor<Vec<u8>>>,
    ) -> Result<i32, XlsxError> {
        let mut index = 0;
        loop {
            index += 1;
            let file_path = format!("xl/comments{index}.xml");
            if !self.reserved_parts.contains(&file_path) && !self.check_file_exist(&file_path) {
                self.add_writer(&file_path, writer)?;
                return Ok(index);
            }
        }
    }

    pub(crate) fn add_file_at_threaded_comment(
        &mut self,
        writer: Writer<Cursor<Vec<u8>>>,
    ) -> Result<i32, XlsxError> {
        let mut index = 0;
        loop {
            index += 1;
            let file_path = format!("xl/threadedComments/threadedComment{index}.xml");
            if !self.reserved_parts.contains(&file_path) && !self.check_file_exist(&file_path) {
                self.add_writer(&file_path, writer)?;
                return Ok(index);
            }
        }
    }

    pub(crate) fn add_file_at_chart(
        &mut self,
        writer: Writer<Cursor<Vec<u8>>>,
    ) -> Result<i32, XlsxError> {
        let mut index = 0;
        loop {
            index += 1;
            let file_path = format!("{PKG_CHARTS}/chart{index}.xml");
            if !self.reserved_parts.contains(&file_path) && !self.check_file_exist(&file_path) {
                self.add_writer(&file_path, writer)?;
                return Ok(index);
            }
        }
    }

    pub(crate) fn add_file_at_ole_object(&mut self, writer: &[u8]) -> Result<i32, XlsxError> {
        let mut index = 0;
        loop {
            index += 1;
            let file_path = format!("{PKG_EMBEDDINGS}/oleObject{index}.bin");
            if !self.check_file_exist(&file_path) {
                self.add_bin(&file_path, writer)?;
                return Ok(index);
            }
        }
    }

    pub(crate) fn add_file_at_excel(&mut self, writer: &[u8]) -> Result<i32, XlsxError> {
        let mut index = 0;
        loop {
            index += 1;
            let file_path = format!("{PKG_EMBEDDINGS}/Microsoft_Excel_Worksheet{index}.xlsx");
            if !self.check_file_exist(&file_path) {
                self.add_bin(&file_path, writer)?;
                return Ok(index);
            }
        }
    }

    pub(crate) fn add_file_at_printer_settings(&mut self, writer: &[u8]) -> Result<i32, XlsxError> {
        let mut index = 0;
        loop {
            index += 1;
            let file_path = format!("{PKG_PRNTR_SETTINGS}/printerSettings{index}.bin");
            if !self.check_file_exist(&file_path) {
                self.add_bin(&file_path, writer)?;
                return Ok(index);
            }
        }
    }

    #[inline]
    pub(crate) fn add_file_at_table(
        &mut self,
        writer: Writer<Cursor<Vec<u8>>>,
        table_no: i32,
    ) -> Result<i32, XlsxError> {
        let file_path = format!("{PKG_TABLES}/table{table_no}.xml");
        self.add_writer(&file_path, writer)?;
        Ok(table_no)
    }

    #[inline]
    pub(crate) fn add_file_at_pivot_table(
        &mut self,
        writer: Writer<Cursor<Vec<u8>>>,
        pivot_table_no: i32,
    ) -> Result<i32, XlsxError> {
        let file_path = format!("xl/pivotTables/pivotTable{pivot_table_no}.xml");
        self.add_writer(&file_path, writer)?;
        Ok(pivot_table_no)
    }

    #[inline]
    pub(crate) fn add_file_at_pivot_cache(
        &mut self,
        writer: Writer<Cursor<Vec<u8>>>,
        pivot_cache_no: i32,
    ) -> Result<i32, XlsxError> {
        let file_path = format!("xl/pivotCache/pivotCacheDefinition{pivot_cache_no}.xml");
        self.add_writer(&file_path, writer)?;
        Ok(pivot_cache_no)
    }

    #[inline]
    pub(crate) fn add_file_at_pivot_cache_records(
        &mut self,
        data: &[u8],
        pivot_cache_no: i32,
    ) -> Result<(), XlsxError> {
        let file_path = format!("xl/pivotCache/pivotCacheRecords{pivot_cache_no}.xml");
        if !self.check_file_exist(&file_path) {
            self.add_bin(&file_path, data)?;
        }
        Ok(())
    }

    #[inline]
    pub(crate) fn has_extension(&self, extension: &str) -> bool {
        let extension = format!(".{extension}");
        self.files.iter().any(|file| file.ends_with(&extension))
    }

    pub(crate) fn make_context_type_override(&mut self, wb: &Workbook) -> Vec<(String, String)> {
        self.file_list_sort();

        let mut list: Vec<(String, String)> = Vec::new();
        for file in &self.files {
            let file = format!("/{file}");
            let mut content_type = "";

            // Override workbook
            if file.starts_with("/xl/workbook.xml") {
                content_type = if wb.has_macros() {
                    WORKBOOK_MACRO_TYPE
                } else {
                    WORKBOOK_TYPE
                };
            }

            // Override sheet
            if file.starts_with("/xl/worksheets/sheet") {
                content_type = SHEET_TYPE;
            }

            // Override table
            if file.starts_with("/xl/tables/table") {
                content_type = TABLE_TYPE;
            }

            // Override pivot table
            if file.starts_with("/xl/pivotTables/pivotTable") {
                content_type = PIVOT_TABLE_TYPE;
            }

            // Override pivot cache definition
            if file.starts_with("/xl/pivotCache/pivotCacheDefinition") {
                content_type = PIVOT_CACHE_DEF_TYPE;
            }

            // Override pivot cache records
            if file.starts_with("/xl/pivotCache/pivotCacheRecords") {
                content_type = PIVOT_CACHE_REC_TYPE;
            }

            // Override comments
            if file.starts_with("/xl/comments") {
                content_type = COMMENTS_TYPE;
            }

            // Override theme
            if file.starts_with("/xl/theme/theme") {
                content_type = THEME_TYPE;
            }

            // Override styles
            if file.starts_with("/xl/styles.xml") {
                content_type = STYLES_TYPE;
            }

            // Override sharedStrings
            if file.starts_with("/xl/sharedStrings.xml") {
                content_type = SHARED_STRINGS_TYPE;
            }

            // Override drawing
            if file.starts_with("/xl/drawings/drawing") {
                content_type = DRAWING_TYPE;
            }

            // Override chart
            if numbered_xml_part(&file, "/xl/charts/chartEx") {
                content_type = CHART_EX_TYPE;
            } else if numbered_xml_part(&file, "/xl/charts/chart") {
                content_type = CHART_TYPE;
            }

            // Override embeddings
            if file.starts_with("/xl/embeddings/oleObject") {
                content_type = OLE_OBJECT_TYPE;
            }

            // Override xl/vbaProject.bin
            if file.starts_with("/xl/vbaProject.bin") {
                content_type = VBA_TYPE;
            }

            // Override docProps/core
            if file.starts_with("/docProps/core.xml") {
                content_type = CORE_PROPS_TYPE;
            }

            // Override docProps/app
            if file.starts_with("/docProps/app.xml") {
                content_type = XPROPS_TYPE;
            }

            // Override docProps/custom
            if file.starts_with("/docProps/custom.xml") {
                content_type = CUSTOM_PROPS_TYPE;
            }

            if wb
                .cell_metadata
                .as_ref()
                .is_some_and(|metadata| file == format!("/{}", metadata.path))
            {
                content_type = crate::structs::cell_metadata::CONTENT_TYPE;
            }

            // Override Unsupported
            if content_type.is_empty() {
                for (old_part_name, old_content_type) in wb.backup_context_types() {
                    if **old_part_name == file {
                        content_type = old_content_type;
                    }
                }
            }

            if let Some(registered) = self.content_types.get(file.trim_start_matches('/')) {
                content_type = registered;
            }
            if !content_type.is_empty() {
                list.push((file, content_type.to_string()));
            }
        }
        list
    }
}

fn table_identity_error(message: impl Into<String>) -> XlsxError {
    XlsxError::with_message(crate::XlsxErrorKind::Xml, message.into())
}

fn raw_table_id(xml: &[u8]) -> Result<u32, XlsxError> {
    use quick_xml::{
        Reader,
        events::Event,
    };

    use crate::drawing::charts::NamespaceScope;

    let mut reader = Reader::from_reader(xml);
    loop {
        match reader.read_event()? {
            Event::Decl(_) | Event::Comment(_) | Event::PI(_) => (),
            Event::Text(text) if text.iter().all(u8::is_ascii_whitespace) => (),
            Event::Start(root) | Event::Empty(root) => {
                let scope = NamespaceScope::default()
                    .child(&root)
                    .map_err(table_identity_error)?;
                let name = scope
                    .resolve(root.name().as_ref(), false)
                    .map_err(table_identity_error)?;
                if !name.is(crate::helper::const_str::SHEET_MAIN_NS, "table") {
                    return Err(table_identity_error("invalid table part root"));
                }
                for attribute in root.attributes() {
                    let attribute = attribute.map_err(|e| table_identity_error(e.to_string()))?;
                    if attribute.key.as_ref() == b"id" {
                        let value =
                            attribute.normalized_value(quick_xml::XmlVersion::Implicit1_0)?;
                        return value
                            .trim()
                            .parse()
                            .map_err(|_| table_identity_error("invalid table id"));
                    }
                }
                return Err(table_identity_error("table part requires id"));
            }
            _ => return Err(table_identity_error("missing table part root")),
        }
    }
}

#[derive(Debug)]
enum WrittenPart {
    Bytes([u8; 32]),
    Raw(Box<RawPartSource>),
}
#[derive(Debug)]
struct RawPartSource {
    path:        PathBuf,
    target:      String,
    file_length: u64,
    modified:    SystemTime,
}
impl WrittenPart {
    fn digest(&self) -> Result<[u8; 32], XlsxError> {
        match self {
            Self::Bytes(digest) => Ok(*digest),
            Self::Raw(source) => {
                let file = File::open(&source.path)?;
                let metadata = file.metadata()?;
                if metadata.len() != source.file_length || metadata.modified()? != source.modified {
                    return Err(part_error("source package changed during export"));
                }
                let mut archive = zip::read::ZipArchive::new(file)?;
                let mut part = zip_by_name(&mut archive, &source.target)?;
                let mut digest = Sha256::new();
                let mut buffer = [0u8; 8192];
                loop {
                    let count = io::Read::read(&mut part, &mut buffer)?;
                    if count == 0 {
                        break;
                    }
                    digest.update(&buffer[..count]);
                }
                Ok(digest.finalize().into())
            }
        }
    }
}
fn numbered_xml_part(path: &str, prefix: &str) -> bool {
    path.strip_prefix(prefix)
        .and_then(|suffix| suffix.strip_suffix(".xml"))
        .is_some_and(|number| {
            !number.is_empty() && number.bytes().all(|value| value.is_ascii_digit())
        })
}
fn part_error(message: impl Into<String>) -> XlsxError {
    XlsxError::with_message(crate::XlsxErrorKind::Xml, message.into())
}

#[cfg(test)]
mod package_part_tests {
    use super::*;

    #[test]
    fn duplicate_destinations_require_identical_payloads() {
        let archive = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let mut manager = WriterManager::new(archive);
        manager.add_bin("xl/media/sound.wav", b"one").unwrap();
        manager.add_bin("xl/media/sound.wav", b"one").unwrap();
        assert!(manager.add_bin("xl/media/sound.wav", b"two").is_err());
        let output = manager.finish().unwrap();
        let archive = zip::ZipArchive::new(output).unwrap();
        assert_eq!(archive.len(), 1);
    }

    #[test]
    fn native_part_types_do_not_capture_chart_style_names() {
        let mut manager = WriterManager::new(zip::ZipWriter::new(Cursor::new(Vec::new())));
        manager
            .add_bin("xl/charts/chart1.xml", b"standard")
            .unwrap();
        manager
            .add_bin("xl/charts/chartEx1.xml", b"extended")
            .unwrap();
        manager
            .add_bin("xl/charts/chartStyle1.xml", b"style")
            .unwrap();
        let mut book = Workbook::default();
        book.set_backup_context_types(vec![(
            "/xl/charts/chartStyle1.xml".into(),
            "native-style".into(),
        )]);
        let types = manager.make_context_type_override(&book);
        assert!(types.contains(&("/xl/charts/chart1.xml".into(), CHART_TYPE.into())));
        assert!(types.contains(&("/xl/charts/chartEx1.xml".into(), CHART_EX_TYPE.into())));
        assert!(types.contains(&("/xl/charts/chartStyle1.xml".into(), "native-style".into())));
    }
}
