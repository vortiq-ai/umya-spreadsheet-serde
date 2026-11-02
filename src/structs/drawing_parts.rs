use std::{
    collections::{
        HashMap,
        HashSet,
    },
    io,
};

use crate::{
    drawing::spreadsheet::ChartExStyleBundle,
    helper::const_str::{
        CHART_COLOR_STYLE_NS,
        CHART_COLOR_STYLE_TYPE,
        CHART_STYLE_NS,
        CHART_STYLE_TYPE,
        DRAWINGS_NS,
        IMAGE_NS,
    },
    structs::{
        Worksheet,
        WriterManager,
        XlsxError,
        XlsxErrorKind,
        raw::{
            RawFile,
            RawRelationship,
            RawRelationships,
            RawWorksheet,
        },
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default)]
pub(crate) struct DrawingParts {
    drawing_relationships: Option<RawRelationships>,
    chart_parts:           Vec<String>,
    relationships:         Vec<RawRelationships>,
    files:                 Vec<RawFile>,
    #[cfg_attr(feature = "serde", serde(default))]
    content_types:         Vec<(String, String)>,
}

pub(crate) struct PreparedDrawingParts {
    value:         DrawingParts,
    locations:     Vec<(usize, usize)>,
    style_bundles: HashMap<String, ChartExStyleBundle>,
}

impl PreparedDrawingParts {
    /// Hands every native extended chart the style and colour parts its own
    /// package relationships name, so the chart owns them from now on.
    pub(crate) fn bind_style_bundles(&self, sheet: &mut Worksheet) {
        let mut bind =
            |_: crate::drawing::spreadsheet::DrawingFrameKey,
             frame: &mut crate::drawing::spreadsheet::ExtendedChartFrame| {
                if let Some(bundle) = frame
                    .source_part()
                    .and_then(|source| self.style_bundles.get(source))
                {
                    frame.bind_style_bundle(bundle.clone());
                }
            };
        sheet
            .worksheet_drawing_mut()
            .visit_extended_frames_mut(&mut bind);
        for ole in sheet.ole_objects_mut().ole_object_mut() {
            ole.two_cell_anchor_mut()
                .visit_extended_frames_mut(&mut bind);
        }
    }

    pub(crate) fn commit(mut self, raw: &mut RawWorksheet) -> Box<DrawingParts> {
        for (list_index, edge_index) in self.locations {
            let edge =
                &mut raw.relationships_list_mut()[list_index].relationship_list_mut()[edge_index];
            self.value.files.push(std::mem::take(edge.raw_file_mut()));
        }
        Box::new(self.value)
    }
}

impl DrawingParts {
    pub(crate) fn preserved_files(files: Vec<RawFile>) -> Self {
        Self {
            files,
            ..Self::default()
        }
    }

    pub(crate) fn prepare(
        raw: &RawWorksheet,
        sheet: &Worksheet,
        session: &crate::reader::xlsx::SourceCacheSession,
        provider: &mut dyn crate::reader::xlsx::ChartPartProvider,
    ) -> Result<Option<PreparedDrawingParts>, XlsxError> {
        let Some(sheet_relationships) = raw.worksheet_relationships()? else {
            return Ok(None);
        };
        let drawings: Vec<_> = sheet_relationships
            .relationship_list()
            .iter()
            .filter(|edge| edge.get_type() == DRAWINGS_NS)
            .collect();
        if drawings.is_empty() {
            return Ok(None);
        }
        if drawings.len() != 1 {
            return Err(part_error("worksheet has multiple drawing parts"));
        }
        let drawing = drawings[0].raw_file();
        let Some(drawing_rels) = raw.relationships_of_part(drawing.file_target())? else {
            return Ok(None);
        };
        let mut source_ids = HashSet::new();
        if drawing_rels
            .relationship_list()
            .iter()
            .any(|edge| edge.id().is_empty() || !source_ids.insert(edge.id()))
        {
            return Err(part_error(
                "invalid or duplicate source drawing relationship id",
            ));
        }
        let modeled_media: HashSet<String> = sheet
            .media_object_collection()
            .into_iter()
            .chain(sheet.worksheet_drawing().additional_media_objects())
            .map(|image| format!("xl/media/{}", image.image_name()))
            .collect();
        let mut chart_parts = HashSet::new();
        sheet
            .worksheet_drawing()
            .visit_standard_frames(&mut |_, frame| {
                chart_parts.extend(frame.source_parts().iter().cloned());
            });
        let mut extended_sources = HashSet::new();
        sheet
            .worksheet_drawing()
            .visit_extended_frames(&mut |_, frame| {
                if let Some(part) = frame.source_part() {
                    chart_parts.insert(part.to_owned());
                    extended_sources.insert(part.to_owned());
                }
            });
        for ole in sheet.ole_objects().ole_object() {
            ole.two_cell_anchor()
                .visit_standard_frames(&mut |_, frame| {
                    chart_parts.extend(frame.source_parts().iter().cloned());
                });
            ole.two_cell_anchor()
                .visit_extended_frames(&mut |_, frame| {
                    if let Some(part) = frame.source_part() {
                        chart_parts.insert(part.to_owned());
                        extended_sources.insert(part.to_owned());
                    }
                });
        }
        let mut supplemental_ids = HashSet::new();
        sheet.visit_drawing_relationships(&mut |_, id| {
            if !id.is_empty() {
                supplemental_ids.insert(id.to_owned());
            }
        });
        let mut queue: Vec<String> = chart_parts.iter().cloned().collect();
        let mut file_targets = HashSet::new();
        for edge in drawing_rels.relationship_list() {
            if !supplemental_ids.contains(edge.id()) || edge.target_mode() == "External" {
                continue;
            }
            let target = edge.raw_file().file_target();
            if edge.get_type() != IMAGE_NS || !modeled_media.contains(target) {
                file_targets.insert(target.to_owned());
                queue.push(target.to_owned());
            }
        }
        let mut owners = HashMap::new();
        for (index, list) in raw.relationships_list().iter().enumerate() {
            if let Some(owner) = relationships_owner(list.file_target()) {
                if let Some(&previous) = owners.get(&owner) {
                    if relationship_identity(&raw.relationships_list()[previous])
                        != relationship_identity(list)
                    {
                        return Err(part_error("conflicting source relationship parts"));
                    }
                } else {
                    owners.insert(owner, index);
                }
            }
        }
        let mut selected_lists = HashSet::new();
        let mut visited = HashSet::new();
        while let Some(owner) = queue.pop() {
            if !visited.insert(owner.clone()) {
                continue;
            }
            if let Some(&index) = owners.get(&owner) {
                selected_lists.insert(index);
                for edge in raw.relationships_list()[index].relationship_list() {
                    if edge.target_mode() == "External" {
                        continue;
                    }
                    let target = edge.raw_file().file_target().to_owned();
                    if target.is_empty() {
                        return Err(part_error("internal drawing dependency has no target"));
                    }
                    if !chart_parts.contains(&target) && !modeled_media.contains(&target) {
                        file_targets.insert(target.clone());
                    }
                    queue.push(target);
                }
            }
        }
        let mut retained_files: Vec<RawFile> = sheet
            .drawing_parts()
            .map(|parts| {
                parts
                    .files
                    .iter()
                    .filter(|file| file_targets.contains(file.file_target()))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        let mut retained_targets: HashSet<String> = retained_files
            .iter()
            .map(|file| file.file_target().to_owned())
            .collect();
        let mut file_locations = HashMap::new();
        for (list_index, list) in raw.relationships_list().iter().enumerate() {
            for (edge_index, edge) in list.relationship_list().iter().enumerate() {
                let file = edge.raw_file();
                if file_targets.contains(file.file_target())
                    && !retained_targets.contains(file.file_target())
                {
                    if file.file_data().is_empty() && file.source_file().is_none() {
                        if matches!(
                            edge.get_type(),
                            crate::helper::const_str::CHART_NS
                                | crate::helper::const_str::CHART_EX_NS
                        ) {
                            let mut recovered = RawFile::default();
                            recovered.set_file_target(file.file_target());
                            provider.with_part(file.file_target(), session, &mut |data| {
                                recovered.set_file_data(data);
                                Ok(())
                            })?;
                            retained_targets.insert(file.file_target().to_owned());
                            retained_files.push(recovered);
                            continue;
                        }
                        return Err(part_error(format!(
                            "missing drawing dependency: {}",
                            file.file_target()
                        )));
                    }
                    file_locations
                        .entry(file.file_target().to_owned())
                        .or_insert((list_index, edge_index));
                }
            }
        }
        if file_locations.len() + retained_files.len() != file_targets.len() {
            return Err(part_error("unresolved drawing dependencies"));
        }
        let mut selected_lists: Vec<_> = selected_lists.into_iter().collect();
        selected_lists.sort_unstable();
        let relationships = selected_lists
            .into_iter()
            .map(|index| metadata_only(&raw.relationships_list()[index]))
            .collect();
        let mut value = Self {
            drawing_relationships: Some(metadata_only(drawing_rels)),
            chart_parts: {
                let mut parts: Vec<_> = chart_parts.into_iter().collect();
                parts.sort();
                parts
            },
            relationships,
            files: retained_files,
            content_types: Vec::new(),
        };
        let mut locations: Vec<_> = file_locations.into_values().collect();
        locations.sort_unstable();
        value.content_types = locations
            .iter()
            .map(|(list_index, edge_index)| {
                let part = raw.relationships_list()[*list_index].relationship_list()[*edge_index]
                    .raw_file()
                    .file_target();
                let content_type = session.content_type(part).ok_or_else(|| {
                    part_error(format!(
                        "missing content type for drawing dependency: {part}"
                    ))
                })?;
                Ok((part.to_owned(), content_type.to_owned()))
            })
            .collect::<Result<_, XlsxError>>()?;
        for file in &value.files {
            let part = file.file_target();
            let content_type = session.content_type(part).ok_or_else(|| {
                part_error(format!(
                    "missing content type for drawing dependency: {part}"
                ))
            })?;
            value
                .content_types
                .push((part.to_owned(), content_type.to_owned()));
        }
        let mut style_bundles = HashMap::new();
        for source in &extended_sources {
            style_bundles.insert(
                source.clone(),
                chart_style_bundle(raw, &value.files, source, session)?,
            );
        }
        Ok(Some(PreparedDrawingParts {
            value,
            locations,
            style_bundles,
        }))
    }

    pub(crate) fn prepare_relationships(
        &self,
        sheet: &Worksheet,
    ) -> Result<Vec<(String, String)>, XlsxError> {
        use crate::drawing::spreadsheet::DrawingRelationshipKind;
        let mut needed = Vec::new();
        sheet.visit_drawing_relationships(&mut |kind, id| needed.push((kind, id.to_owned())));
        let mut seen = HashSet::new();
        let mut entries = Vec::new();
        for (kind, id) in needed {
            if id.is_empty() {
                if matches!(kind, DrawingRelationshipKind::Sound) {
                    return Err(part_error("sound relationship id is empty"));
                }
                continue;
            }
            let edge = self
                .drawing_relationships
                .as_ref()
                .and_then(|list| list.find_relationship_by_rid(&id))
                .ok_or_else(|| part_error(format!("missing drawing relationship: {id}")))?;
            let expected = match kind {
                DrawingRelationshipKind::Hyperlink => Some(
                    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink",
                ),
                DrawingRelationshipKind::Sound => Some(
                    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/audio",
                ),
                DrawingRelationshipKind::Extension => None,
            };
            if expected.is_some_and(|expected| edge.get_type() != expected) {
                return Err(part_error("drawing relationship has the wrong type"));
            }
            if seen.insert(id.clone()) {
                entries.push(("AUX".into(), id.clone()));
            }
        }
        Ok(entries)
    }

    pub(crate) fn drawing_relationships(&self) -> Option<&RawRelationships> {
        self.drawing_relationships.as_ref()
    }

    pub(crate) fn reserve<W: io::Seek + io::Write>(&self, manager: &mut WriterManager<W>) {
        if let Some(relationships) = &self.drawing_relationships {
            manager.register_preserved_relationships(relationships, true);
        }
        for file in &self.files {
            manager.reserve_part(file.file_target());
        }
        for list in &self.relationships {
            manager.reserve_part(list.file_target());
            manager.register_preserved_relationships(list, false);
        }
        for part in &self.chart_parts {
            manager.reserve_part(part);
        }
    }

    /// Whether a chart's style or colour edge points at a preserved part
    /// holding exactly the given payload.
    pub(crate) fn style_edge_matches(
        &self,
        edge: &RawRelationship,
        expected: Option<&[u8]>,
    ) -> Result<bool, XlsxError> {
        let Some(expected) = expected else {
            return Ok(false);
        };
        let target = edge.raw_file().file_target();
        let Some(file) = self.files.iter().find(|file| file.file_target() == target) else {
            return Ok(false);
        };
        Ok(file.read_data()? == expected)
    }

    /// Style and colour parts that only chart parts no longer present in the
    /// sheet, or edges of a chart that name another chart's payload, referred
    /// to. A chart moved elsewhere writes its own copy there.
    fn orphaned_chart_style_parts(
        &self,
        live_charts: &HashSet<String>,
        bundles: &HashMap<String, Vec<ChartExStyleBundle>>,
    ) -> Result<HashSet<&str>, XlsxError> {
        let chart_parts: HashSet<&str> = self.chart_parts.iter().map(String::as_str).collect();
        let mut candidates = HashSet::new();
        let mut referenced = HashSet::new();
        for list in &self.relationships {
            let owner = relationships_owner(list.file_target());
            let dead_chart = owner
                .as_deref()
                .is_some_and(|owner| chart_parts.contains(owner) && !live_charts.contains(owner));
            let bundle = owner.as_deref().and_then(|owner| bundles.get(owner));
            for edge in list.relationship_list() {
                if edge.target_mode() == "External" {
                    continue;
                }
                let target = edge.raw_file().file_target();
                let kind = edge.get_type();
                let is_style = matches!(kind, CHART_STYLE_NS | CHART_COLOR_STYLE_NS);
                let owned = match (dead_chart, bundle) {
                    _ if !is_style => true,
                    (true, _) => false,
                    (false, Some(bundles)) => {
                        let mut owned = false;
                        for bundle in bundles {
                            owned |= self.style_edge_matches(
                                edge,
                                if kind == CHART_STYLE_NS {
                                    bundle.style.as_deref()
                                } else {
                                    bundle.colors.as_deref()
                                },
                            )?;
                        }
                        owned
                    }
                    (false, None) => true,
                };
                if is_style && !owned {
                    candidates.insert(target);
                } else {
                    referenced.insert(target);
                }
            }
        }
        candidates.retain(|target| !referenced.contains(target));
        Ok(candidates)
    }

    pub(crate) fn write_common<W: io::Seek + io::Write>(
        &self,
        manager: &mut WriterManager<W>,
        live_charts: &HashSet<String>,
        bundles: &HashMap<String, Vec<ChartExStyleBundle>>,
    ) -> Result<(), XlsxError> {
        let orphaned = self.orphaned_chart_style_parts(live_charts, bundles)?;
        for (part, content_type) in &self.content_types {
            if orphaned.contains(part.as_str()) {
                continue;
            }
            if manager.preserve_source_part(part)? {
                manager.register_content_type(part, content_type)?;
            }
        }
        let chart_parts: HashSet<&str> = self.chart_parts.iter().map(String::as_str).collect();
        for list in &self.relationships {
            if relationships_owner(list.file_target())
                .as_deref()
                .is_some_and(|owner| {
                    chart_parts.contains(owner)
                        && !self.files.iter().any(|file| file.file_target() == owner)
                })
            {
                continue;
            }
            list.write_to(manager, None)?;
        }
        for file in &self.files {
            if orphaned.contains(file.file_target()) {
                continue;
            }
            if manager.preserve_source_part(file.file_target())? {
                file.write_to(manager)?;
            }
        }
        Ok(())
    }

    /// The bytes and content type of a preserved part, for a chart that
    /// copies the part instead of sharing it.
    pub(crate) fn preserved_part_copy(
        &self,
        target: &str,
    ) -> Result<Option<(Vec<u8>, String)>, XlsxError> {
        let Some(file) = self.files.iter().find(|file| file.file_target() == target) else {
            return Ok(None);
        };
        let content_type = self
            .content_types
            .iter()
            .find(|(part, _)| part == target)
            .map(|(_, content_type)| content_type.clone())
            .ok_or_else(|| part_error("preserved part has no content type"))?;
        let mut file = file.clone();
        file.load_file_data_from_source()?;
        Ok(Some((file.file_data().to_vec(), content_type)))
    }

    pub(crate) fn chart_relationships(
        &self,
        sources: &[String],
    ) -> Result<RawRelationships, XlsxError> {
        let mut result = RawRelationships::default();
        let mut ids = HashMap::new();
        for source in sources {
            let path = relationships_path(source)?;
            if let Some(list) = self
                .relationships
                .iter()
                .find(|list| list.file_target() == path)
            {
                for edge in list.relationship_list() {
                    let edge = edge.clone();
                    let identity = (
                        edge.get_type().to_owned(),
                        edge.target().to_owned(),
                        edge.target_mode().to_owned(),
                    );
                    if let Some(previous) = ids.insert(edge.id().to_owned(), identity.clone()) {
                        if previous != identity {
                            return Err(part_error(
                                "merged chart parts have conflicting relationship ids",
                            ));
                        }
                    } else {
                        result.add_relationship_list(edge);
                    }
                }
            }
        }
        Ok(result)
    }

    pub(crate) fn matching_chart_image(
        &self,
        relationships: &RawRelationships,
        image: &crate::MediaObject,
        modeled_media: &[&crate::MediaObject],
    ) -> Result<Option<String>, XlsxError> {
        for edge in relationships.relationship_list() {
            if edge.get_type() != IMAGE_NS
                || edge.target_mode() == "External"
                || edge.raw_file().file_name() != image.image_name()
            {
                continue;
            }
            let target = edge.raw_file().file_target();
            let matches = if let Some(file) =
                self.files.iter().find(|file| file.file_target() == target)
            {
                let mut file = file.clone();
                file.load_file_data_from_source()?;
                file.file_data() == image.image_data()
            } else {
                // Modeled drawing pictures own their media instead of the raw
                // sidecar. Match the complete package path and actual bytes.
                let modeled = modeled_media
                    .iter()
                    .find(|media| format!("xl/media/{}", media.image_name()) == target)
                    .ok_or_else(|| part_error("preserved chart image bytes are missing"))?;
                modeled.image_data() == image.image_data()
            };
            if matches {
                return Ok(Some(edge.id().to_owned()));
            }
        }
        Ok(None)
    }

    pub(crate) fn update_dependency_fingerprint(&self, digest: &mut sha2::Sha256) {
        use sha2::Digest as _;
        let mut field = |value: &[u8]| {
            digest.update(value.len().to_le_bytes());
            digest.update(value);
        };
        field(
            &(usize::from(self.drawing_relationships.is_some()) + self.relationships.len())
                .to_le_bytes(),
        );
        for relationships in self.drawing_relationships.iter().chain(&self.relationships) {
            field(relationships.file_target().as_bytes());
            field(&relationships.relationship_list().len().to_le_bytes());
            for edge in relationships.relationship_list() {
                field(edge.id().as_bytes());
                field(edge.get_type().as_bytes());
                field(edge.target().as_bytes());
                field(edge.target_mode().as_bytes());
                field(edge.raw_file().file_target().as_bytes());
            }
        }
        field(&self.files.len().to_le_bytes());
        for file in &self.files {
            field(file.file_target().as_bytes());
            field(file.file_data());
            field(&[u8::from(file.source_file().is_some())]);
            if let Some(path) = file.source_file() {
                field(path.to_string_lossy().as_bytes());
            }
        }
        field(&self.content_types.len().to_le_bytes());
        for (target, content_type) in &self.content_types {
            field(target.as_bytes());
            field(content_type.as_bytes());
        }
        field(&self.chart_parts.len().to_le_bytes());
        for part in &self.chart_parts {
            field(part.as_bytes());
        }
    }

    pub(crate) fn write_chart_relationships<W: io::Seek + io::Write>(
        relationships: &mut RawRelationships,
        output: &str,
        manager: &mut WriterManager<W>,
    ) -> Result<(), XlsxError> {
        relationships.set_file_target(relationships_path(output)?);
        for edge in relationships.relationship_list_mut() {
            if edge.target_mode() != "External" {
                edge.set_target(relative_target(output, edge.raw_file().file_target())?);
            }
        }
        relationships.write_to(manager, None)
    }
}

/// The style and colour payloads a chart part names through its own
/// relationships. A chart declaring several parts of one kind is rejected
/// rather than silently reduced to one of them.
fn chart_style_bundle(
    raw: &RawWorksheet,
    retained: &[RawFile],
    chart: &str,
    session: &crate::reader::xlsx::SourceCacheSession,
) -> Result<ChartExStyleBundle, XlsxError> {
    let mut bundle = ChartExStyleBundle::default();
    let Some(list) = raw.relationships_of_part(chart)? else {
        return Ok(bundle);
    };
    for edge in list.relationship_list() {
        let (slot, expected) = match edge.get_type() {
            CHART_STYLE_NS => (&mut bundle.style, CHART_STYLE_TYPE),
            CHART_COLOR_STYLE_NS => (&mut bundle.colors, CHART_COLOR_STYLE_TYPE),
            _ => continue,
        };
        if edge.target_mode() == "External" {
            return Err(part_error("chart style part cannot be external"));
        }
        // Excel refuses a package whose style or colour part is declared
        // with any other content type, so the part is never imported as one.
        let declared = session.content_type(edge.raw_file().file_target());
        if declared != Some(expected) {
            return Err(part_error(format!(
                "chart part {} has content type {}, expected {expected}",
                edge.raw_file().file_target(),
                declared.unwrap_or("(none)")
            )));
        }
        if slot.is_some() {
            return Err(part_error(format!(
                "chart {chart} declares several style or colour parts"
            )));
        }
        let target = edge.raw_file().file_target();
        // The package keeps one copy of a part shared by several edges.
        let bytes = retained
            .iter()
            .chain(
                raw.relationships_list()
                    .iter()
                    .flat_map(RawRelationships::relationship_list)
                    .map(RawRelationship::raw_file),
            )
            .find(|file| {
                file.file_target() == target
                    && (!file.file_data().is_empty() || file.source_file().is_some())
            })
            .ok_or_else(|| part_error(format!("chart style part has no bytes: {target}")))?
            .read_data()?;
        if bytes.is_empty() {
            return Err(part_error(format!(
                "chart style part has no bytes: {target}"
            )));
        }
        *slot = Some(bytes);
    }
    Ok(bundle)
}

fn metadata_only(source: &RawRelationships) -> RawRelationships {
    let mut value = RawRelationships::default();
    value.set_file_target(source.file_target());
    for original in source.relationship_list() {
        let mut edge = RawRelationship::default();
        edge.set_id(original.id());
        edge.set_type(original.get_type());
        edge.set_target(original.target());
        edge.set_target_mode(original.target_mode());
        let mut file = RawFile::default();
        file.set_file_target(original.raw_file().file_target());
        edge.set_raw_file(file);
        value.add_relationship_list(edge);
    }
    value
}

pub(crate) fn relationships_path(part: &str) -> Result<String, XlsxError> {
    let part = part.trim_start_matches('/');
    if part.is_empty() {
        return Err(part_error("package part name is empty"));
    }
    Ok(match part.rsplit_once('/') {
        Some((directory, name)) => format!("{directory}/_rels/{name}.rels"),
        None => format!("_rels/{part}.rels"),
    })
}
pub(crate) fn relationships_owner(path: &str) -> Option<String> {
    let path = path.trim_start_matches('/');
    if let Some(name) = path.strip_prefix("_rels/") {
        return Some(name.strip_suffix(".rels")?.to_owned());
    }
    let (directory, name) = path.rsplit_once("/_rels/")?;
    Some(format!("{directory}/{}", name.strip_suffix(".rels")?))
}
pub(crate) fn relative_target(owner: &str, target: &str) -> Result<String, XlsxError> {
    let owner = owner.trim_start_matches('/');
    let from: Vec<_> = owner
        .rsplit_once('/')
        .map_or_else(Vec::new, |(directory, _)| directory.split('/').collect());
    let to: Vec<_> = target.split('/').collect();
    if to
        .iter()
        .any(|part| part.is_empty() || matches!(*part, "." | ".."))
    {
        return Err(part_error("noncanonical package dependency target"));
    }
    let common = from.iter().zip(&to).take_while(|(a, b)| a == b).count();
    let mut result = vec![".."; from.len() - common];
    result.extend_from_slice(&to[common..]);
    Ok(result.join("/"))
}
fn part_error(message: impl Into<String>) -> XlsxError {
    XlsxError::with_message(XlsxErrorKind::Xml, message.into())
}

fn relationship_identity(list: &RawRelationships) -> Vec<(&str, &str, &str, &str, &str)> {
    list.relationship_list()
        .iter()
        .map(|edge| {
            (
                edge.id(),
                edge.get_type(),
                edge.target(),
                edge.target_mode(),
                edge.raw_file().file_target(),
            )
        })
        .collect()
}

#[cfg(test)]
mod root_part_tests {
    use super::{
        relationships_owner,
        relationships_path,
        relative_target,
    };

    #[test]
    fn root_level_parts_pair_with_their_relationships() {
        assert_eq!(
            relationships_path("pivotTable1.xml").unwrap(),
            "_rels/pivotTable1.xml.rels"
        );
        assert_eq!(
            relationships_path("/pivotTable1.xml").unwrap(),
            "_rels/pivotTable1.xml.rels"
        );
        assert_eq!(
            relationships_owner("_rels/pivotTable1.xml.rels").as_deref(),
            Some("pivotTable1.xml")
        );
        assert_eq!(
            relationships_owner("xl/_rels/workbook.xml.rels").as_deref(),
            Some("xl/workbook.xml")
        );
        assert_eq!(
            relative_target("pivotTable1.xml", "xl/pivotCache/c.xml").unwrap(),
            "xl/pivotCache/c.xml"
        );
        assert_eq!(
            relative_target("xl/drawings/d.xml", "xl/charts/c.xml").unwrap(),
            "../charts/c.xml"
        );
    }
}
