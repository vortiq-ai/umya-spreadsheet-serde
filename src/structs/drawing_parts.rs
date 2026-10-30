use std::{
    collections::{
        HashMap,
        HashSet,
    },
    io,
};

use crate::{
    helper::const_str::{
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
    value:     DrawingParts,
    locations: Vec<(usize, usize)>,
}

impl PreparedDrawingParts {
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
    pub(crate) fn prepare(
        raw: &RawWorksheet,
        sheet: &Worksheet,
        session: &crate::reader::xlsx::SourceCacheSession,
    ) -> Result<Option<PreparedDrawingParts>, XlsxError> {
        let Some(sheet_relationships) = raw.worksheet_relationships() else {
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
        let drawing_rels_path = relationships_path(drawing.file_target())?;
        let Some(drawing_rels) = raw
            .relationships_list()
            .iter()
            .find(|list| list.file_target() == drawing_rels_path)
        else {
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
        sheet
            .worksheet_drawing()
            .visit_extended_frames(&mut |_, frame| {
                if let Some(part) = frame.source_part() {
                    chart_parts.insert(part.to_owned());
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
        let mut file_locations = HashMap::new();
        for (list_index, list) in raw.relationships_list().iter().enumerate() {
            for (edge_index, edge) in list.relationship_list().iter().enumerate() {
                let file = edge.raw_file();
                if file_targets.contains(file.file_target()) {
                    if file.file_data().is_empty() && file.source_file().is_none() {
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
        if file_locations.len() != file_targets.len() {
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
            files: Vec::new(),
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
        Ok(Some(PreparedDrawingParts { value, locations }))
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
        for file in &self.files {
            manager.reserve_part(file.file_target());
        }
        for list in &self.relationships {
            manager.reserve_part(list.file_target());
        }
        for part in &self.chart_parts {
            manager.reserve_part(part);
        }
    }

    pub(crate) fn write_common<W: io::Seek + io::Write>(
        &self,
        manager: &mut WriterManager<W>,
    ) -> Result<(), XlsxError> {
        for (part, content_type) in &self.content_types {
            manager.register_content_type(part, content_type)?;
        }
        let chart_parts: HashSet<&str> = self.chart_parts.iter().map(String::as_str).collect();
        for list in &self.relationships {
            if relationships_owner(list.file_target())
                .as_deref()
                .is_some_and(|owner| chart_parts.contains(owner))
            {
                continue;
            }
            list.write_to(manager, None)?;
        }
        for file in &self.files {
            file.write_to(manager)?;
        }
        Ok(())
    }

    pub(crate) fn write_chart_dependencies<W: io::Seek + io::Write>(
        &self,
        sources: &[String],
        output: &str,
        manager: &mut WriterManager<W>,
    ) -> Result<(), XlsxError> {
        let mut result = RawRelationships::default();
        result.set_file_target(relationships_path(output)?);
        let mut ids = HashMap::new();
        for source in sources {
            let path = relationships_path(source)?;
            if let Some(list) = self
                .relationships
                .iter()
                .find(|list| list.file_target() == path)
            {
                for edge in list.relationship_list() {
                    let mut edge = edge.clone();
                    if edge.target_mode() != "External" {
                        edge.set_target(relative_target(output, edge.raw_file().file_target())?);
                    }
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
        result.write_to(manager, None)
    }
}

fn metadata_only(source: &RawRelationships) -> RawRelationships {
    let mut value = RawRelationships::default();
    value.set_file_target(source.file_target());
    for original in source.relationship_list() {
        let mut edge = crate::structs::raw::RawRelationship::default();
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
    let (directory, name) = part
        .rsplit_once('/')
        .ok_or_else(|| part_error("package part has no directory"))?;
    Ok(format!("{directory}/_rels/{name}.rels"))
}
fn relationships_owner(path: &str) -> Option<String> {
    let (directory, name) = path.rsplit_once("/_rels/")?;
    Some(format!("{directory}/{}", name.strip_suffix(".rels")?))
}
pub(crate) fn relative_target(owner: &str, target: &str) -> Result<String, XlsxError> {
    let (directory, _) = owner
        .rsplit_once('/')
        .ok_or_else(|| part_error("package owner has no directory"))?;
    let from: Vec<_> = directory.split('/').collect();
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
