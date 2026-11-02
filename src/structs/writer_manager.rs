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
        CHART_COLOR_STYLE_TYPE,
        CHART_EX_TYPE,
        CHART_STYLE_TYPE,
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
    files:                   Vec<String>,
    written_parts:           HashMap<String, WrittenPart>,
    reserved_parts:          HashSet<String>,
    content_types:           HashMap<String, String>,
    arv:                     zip::ZipWriter<W>,
    is_light:                bool,
    table_no:                i32,
    table_ids:               HashSet<u32>,
    written_table_ids:       HashSet<u32>,
    reserved_table_parts:    HashSet<String>,
    pivot_table_no:          i32,
    allocated_parts:         HashMap<String, u32>,
    pivot_cache_hash_list:   Vec<String>,
    written_pivot_caches:    HashSet<i32>,
    preserved_graph:         HashMap<String, Vec<String>>,
    original_graph:          HashMap<String, Vec<String>>,
    preserved_roots:         HashSet<String>,
    preserved_media:         HashSet<String>,
    preserved_graph_error:   Option<String>,
    chart_media:             HashMap<(String, [u8; 32]), String>,
    rewritten_picture_graph: bool,
    rewritten_chart_edges:   HashMap<String, HashSet<String>>,
    preserved_chart_owners:  HashSet<String>,
    extended_charts:         HashMap<String, [u8; 32]>,
    claimed_style_parts:     HashSet<String>,
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
            allocated_parts: HashMap::new(),
            pivot_cache_hash_list: Vec::new(),
            written_pivot_caches: HashSet::new(),
            preserved_graph: HashMap::new(),
            original_graph: HashMap::new(),
            preserved_roots: HashSet::new(),
            preserved_media: HashSet::new(),
            preserved_graph_error: None,
            chart_media: HashMap::new(),
            rewritten_picture_graph: false,
            rewritten_chart_edges: HashMap::new(),
            preserved_chart_owners: HashSet::new(),
            extended_charts: HashMap::new(),
            claimed_style_parts: HashSet::new(),
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
                let sheet_relationships = sheet
                    .raw_data_of_worksheet()
                    .worksheet_relationships()?
                    .map(super::raw::RawRelationships::file_target);
                for relationships in sheet.raw_data_of_worksheet().relationships_list() {
                    self.register_preserved_relationships(
                        relationships,
                        sheet_relationships == Some(relationships.file_target()),
                    );
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

    pub(crate) fn written_table_ids(&self) -> &HashSet<u32> {
        &self.written_table_ids
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

    /// Table ids that are reserved or written, across every sheet.
    #[inline]
    pub(crate) fn table_ids(&self) -> &HashSet<u32> {
        &self.table_ids
    }

    /// A part name `<directory>/<stem><n>.<extension>` that no source part
    /// reserves and no written part occupies.
    pub(crate) fn allocate_part(&mut self, directory: &str, stem: &str, extension: &str) -> String {
        let counter = self
            .allocated_parts
            .entry(format!("{directory}/{stem}.{extension}"))
            .or_insert(0);
        loop {
            *counter += 1;
            let path = format!("{directory}/{stem}{counter}.{extension}");
            if !self.reserved_parts.contains(&path) && !self.written_parts.contains_key(&path) {
                return path;
            }
        }
    }

    #[inline]
    pub fn next_pivot_table_no(&mut self) -> i32 {
        self.pivot_table_no += 1;
        self.pivot_table_no
    }

    /// The number of the pivot cache `id`, reserving the next one when the
    /// workbook does not declare it. Reported with whether its definition was
    /// already written.
    #[inline]
    pub fn get_pivot_cache_no(&mut self, hash: &str) -> (bool, i32) {
        let no = self.reserve_pivot_cache(hash);
        (!self.written_pivot_caches.insert(no), no)
    }

    /// Number the pivot caches the workbook declares, in its declared order,
    /// before any sheet is written: declarations and parts then agree whatever
    /// order the sheets are written in.
    pub(crate) fn reserve_pivot_caches(&mut self, ids: &[String]) {
        for id in ids {
            self.reserve_pivot_cache(id);
        }
    }

    fn reserve_pivot_cache(&mut self, id: &str) -> i32 {
        let position = self
            .pivot_cache_hash_list
            .iter()
            .position(|known| known == id)
            .unwrap_or_else(|| {
                self.pivot_cache_hash_list.push(id.to_string());
                self.pivot_cache_hash_list.len() - 1
            });
        i32::try_from(position + 1).unwrap()
    }

    /// Whether the definition part of pivot cache number `no` was written.
    pub(crate) fn pivot_cache_written(&self, no: i32) -> bool {
        self.written_pivot_caches.contains(&no)
    }

    /// The reserved number of the pivot cache `id`.
    pub(crate) fn pivot_cache_number(&self, id: &str) -> Option<i32> {
        self.pivot_cache_hash_list
            .iter()
            .position(|known| known == id)
            .map(|position| i32::try_from(position + 1).unwrap())
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
                            if matches!(
                                edge.get_type(),
                                crate::helper::const_str::CHART_NS
                                    | crate::helper::const_str::CHART_EX_NS
                            ) {
                                self.preserved_chart_owners
                                    .insert(edge.raw_file().file_target().to_owned());
                            }
                        }
                    }
                }
            }
        }
    }

    pub(crate) fn reserve_part(&mut self, target: &str) {
        self.reserved_parts.insert(target.to_owned());
    }

    pub(crate) fn register_preserved_relationships(
        &mut self,
        relationships: &super::raw::RawRelationships,
        root: bool,
    ) {
        let Some(owner) = super::drawing_parts::relationships_owner(relationships.file_target())
        else {
            self.preserved_graph_error = Some("invalid preserved relationship owner".into());
            return;
        };
        let mut targets = Vec::new();
        for edge in relationships.relationship_list() {
            if edge.target_mode() == "External" {
                continue;
            }
            let target = edge.raw_file().file_target();
            if target.is_empty() {
                self.preserved_graph_error = Some("preserved dependency has no target".into());
                continue;
            }
            if edge.get_type() == crate::helper::const_str::IMAGE_NS {
                self.preserved_media.insert(target.to_owned());
            }
            targets.push(target.to_owned());
        }
        targets.sort();
        targets.dedup();
        if self
            .original_graph
            .get(&owner)
            .is_some_and(|old| old != &targets)
        {
            self.preserved_graph_error = Some("conflicting preserved dependency graph".into());
        } else {
            self.original_graph.insert(owner.clone(), targets.clone());
            if !self.rewritten_chart_edges.contains_key(&owner) {
                self.preserved_graph.insert(owner.clone(), targets);
            }
        }
        if root {
            self.preserved_roots.insert(owner);
        }
    }

    pub(crate) fn replace_chart_relationship_graph(
        &mut self,
        sources: &[String],
        relationships: &super::raw::RawRelationships,
    ) -> Result<(), XlsxError> {
        if let Some(error) = &self.preserved_graph_error {
            return Err(part_error(error.clone()));
        }
        let targets: Vec<_> = relationships
            .relationship_list()
            .iter()
            .filter(|edge| edge.target_mode() != "External")
            .map(|edge| edge.raw_file().file_target().to_owned())
            .collect();
        if targets.iter().any(String::is_empty) {
            return Err(part_error("chart dependency has no target"));
        }
        for source in sources {
            let rewritten = self
                .rewritten_chart_edges
                .entry(source.clone())
                .or_default();
            if self.preserved_chart_owners.contains(source) {
                if let Some(original) = self.original_graph.get(source) {
                    rewritten.extend(original.iter().cloned());
                }
            }
            rewritten.extend(targets.iter().cloned());
            self.preserved_graph
                .insert(source.clone(), rewritten.iter().cloned().collect());
        }
        self.rewritten_picture_graph |= !sources.is_empty();
        Ok(())
    }

    pub(crate) fn preserve_chart_owners(&mut self, sources: &[String]) {
        self.preserved_chart_owners.extend(sources.iter().cloned());
        for source in sources {
            if let (Some(original), Some(rewritten)) = (
                self.original_graph.get(source),
                self.rewritten_chart_edges.get_mut(source),
            ) {
                rewritten.extend(original.iter().cloned());
                self.preserved_graph
                    .insert(source.clone(), rewritten.iter().cloned().collect());
            }
        }
    }

    pub(crate) fn preserve_source_part(&self, target: &str) -> Result<bool, XlsxError> {
        if let Some(error) = &self.preserved_graph_error {
            return Err(part_error(error.clone()));
        }
        if !self.rewritten_picture_graph || !self.preserved_media.contains(target) {
            return Ok(true);
        }
        let mut reachable = HashSet::new();
        let mut pending: Vec<_> = self.preserved_roots.iter().map(String::as_str).collect();
        while let Some(owner) = pending.pop() {
            if !reachable.insert(owner) {
                continue;
            }
            if let Some(edges) = self.preserved_graph.get(owner) {
                pending.extend(edges.iter().map(String::as_str));
            }
        }
        Ok(reachable.contains(target))
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

    /// Writes an extended chart part. Identical chart XML owning different
    /// style payloads is a different chart and gets its own part name.
    pub(crate) fn add_extended_chart(
        &mut self,
        preferred: Option<&str>,
        data: &[u8],
        owner: [u8; 32],
    ) -> Result<String, XlsxError> {
        let preferred = preferred.filter(|target| {
            self.extended_charts
                .get(*target)
                .is_none_or(|previous| *previous == owner)
        });
        self.add_chart_part(preferred, data, CHART_EX_TYPE, "chartEx")
    }

    /// Whether an identical extended chart (same XML, same style payloads)
    /// already wrote its part and relationships; every frame of that chart
    /// shares them.
    pub(crate) fn extended_chart_written(&self, target: &str) -> bool {
        self.extended_charts.contains_key(target)
    }

    /// A style or colour part belongs to one chart; a second chart pointing
    /// at it would share what Excel writes once per chart.
    pub(crate) fn style_part_claimed(&self, target: &str) -> bool {
        self.claimed_style_parts.contains(target)
    }

    pub(crate) fn claim_style_part(&mut self, target: &str) {
        self.claimed_style_parts.insert(target.to_owned());
    }

    pub(crate) fn record_extended_chart(&mut self, target: &str, owner: [u8; 32]) {
        self.extended_charts.insert(target.to_owned(), owner);
    }

    /// Writes the requested style and/or colour parts of an extended chart
    /// under one shared number, returning the part names of those written.
    pub(crate) fn add_extended_chart_style_parts(
        &mut self,
        style: Option<&[u8]>,
        colors: Option<&[u8]>,
    ) -> Result<(Option<String>, Option<String>), XlsxError> {
        let mut number = 1u32;
        loop {
            let style_target = format!("{PKG_CHARTS}/style{number}.xml");
            let colors_target = format!("{PKG_CHARTS}/colors{number}.xml");
            let taken = |manager: &Self, target: &str| {
                manager.reserved_parts.contains(target)
                    || manager.written_parts.contains_key(target)
            };
            if !(style.is_some() && taken(self, &style_target)
                || colors.is_some() && taken(self, &colors_target))
            {
                let mut written = (None, None);
                if let Some(data) = style {
                    self.add_typed_bin(&style_target, CHART_STYLE_TYPE, data)?;
                    written.0 = Some(style_target);
                }
                if let Some(data) = colors {
                    self.add_typed_bin(&colors_target, CHART_COLOR_STYLE_TYPE, data)?;
                    written.1 = Some(colors_target);
                }
                return Ok(written);
            }
            number = number
                .checked_add(1)
                .ok_or_else(|| part_error("chart style part numbers exhausted"))?;
        }
    }

    pub(crate) fn add_chart_image(
        &mut self,
        image: &super::MediaObject,
    ) -> Result<String, XlsxError> {
        let extension = image
            .image_name()
            .rsplit_once('.')
            .map(|(_, extension)| extension.to_ascii_lowercase())
            .ok_or_else(|| part_error("chart image filename has no extension"))?;
        let content_type = match extension.as_str() {
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "gif" => "image/gif",
            "bmp" => "image/bmp",
            "tif" | "tiff" => "image/tiff",
            "emf" => "image/x-emf",
            "wmf" => "image/x-wmf",
            "svg" => "image/svg+xml",
            _ => return Err(part_error("unsupported chart image extension")),
        };
        if image.image_data().is_empty() {
            return Err(part_error("embedded chart image has no bytes"));
        }
        let identity = (
            content_type.to_owned(),
            Sha256::digest(image.image_data()).into(),
        );
        if let Some(target) = self.chart_media.get(&identity) {
            return Ok(target.clone());
        }
        let mut number = 1u32;
        loop {
            let target = format!("xl/media/chartPicture{number}.{extension}");
            if !self.reserved_parts.contains(&target) && !self.written_parts.contains_key(&target) {
                self.add_typed_bin(&target, content_type, image.image_data())?;
                self.chart_media.insert(identity, target.clone());
                return Ok(target);
            }
            number = number
                .checked_add(1)
                .ok_or_else(|| part_error("chart image part numbers exhausted"))?;
        }
    }

    /// A new part `{directory}/{stem}{n}.xml` holding `data`: a chart owns its
    /// drawing, style and colour parts instead of sharing them.
    pub(crate) fn add_chart_private_part(
        &mut self,
        directory: &str,
        stem: &str,
        data: &[u8],
        content_type: &str,
    ) -> Result<String, XlsxError> {
        let mut number = 1u32;
        loop {
            let target = format!("{directory}/{stem}{number}.xml");
            if !self.reserved_parts.contains(&target) && !self.written_parts.contains_key(&target) {
                self.add_typed_bin(&target, content_type, data)?;
                return Ok(target);
            }
            number = number
                .checked_add(1)
                .ok_or_else(|| part_error("drawing part numbers exhausted"))?;
        }
    }

    /// Whether a chart part is already written (by another frame).
    pub(crate) fn standard_chart_written(&self, target: &str) -> bool {
        self.written_parts.contains_key(target)
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

            if wb.connections.is_some()
                && file == format!("/{}", crate::structs::cell_metadata::CONNECTIONS_PATH)
            {
                content_type = crate::structs::cell_metadata::CONNECTIONS_CONTENT_TYPE;
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
