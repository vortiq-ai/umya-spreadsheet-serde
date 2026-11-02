//! Identity-based binding checks for timelines, and the follow-up of a removed
//! worksheet.
//!
//! Timeline caches are workbook parts listed in `x15:timelineCacheRefs`;
//! timeline views are worksheet parts listed in `x15:timelineRefs`. A timeline
//! cache names its pivot tables by `(tabId, name)` and its pivot cache by the
//! x14 pivot cache id.

use std::collections::HashSet;

use super::model::{
    BUILTIN_TIMELINE_STYLES,
    TimelineCache,
};
use crate::{
    drawing::spreadsheet::SlicerFrameKind,
    structs::{
        SlicerPivotTable,
        Workbook,
        Worksheet,
        XlsxError,
    },
};

fn binding_error(message: impl std::fmt::Display) -> XlsxError {
    crate::xml::xml_error(format!("unresolved timeline binding: {message}"))
}

fn sheet_by_id(workbook: &Workbook, sheet_id: u32) -> Option<&Worksheet> {
    workbook
        .sheet_collection_no_check()
        .iter()
        .find(|sheet| sheet.sheet_id().parse::<u32>() == Ok(sheet_id))
}

/// Check that the pivot table a timeline cache links to exists exactly once on
/// its sheet, reads the pivot cache the timeline cache names and has the
/// timeline's source field.
fn check_pivot_link(
    cache: &TimelineCache,
    link: &SlicerPivotTable,
    sheet: &Worksheet,
) -> Result<(), XlsxError> {
    let tables: Vec<_> = sheet
        .pivot_tables()
        .iter()
        .filter(|table| table.pivot_table_definition().name() == link.name)
        .collect();
    let [table] = tables.as_slice() else {
        return Err(binding_error(if tables.is_empty() {
            format!(
                "timeline cache '{}' references missing pivot table '{}' on sheet id {}",
                cache.name, link.name, link.sheet_id
            )
        } else {
            format!(
                "timeline cache '{}' pivot table '{}' is ambiguous on sheet id {}",
                cache.name, link.name, link.sheet_id
            )
        }));
    };
    let definition = table.pivot_cache_definition();
    if definition.slicer_cache_id() != Some(cache.state.pivot_cache_id) {
        return Err(binding_error(format!(
            "timeline cache '{}' pivot cache id {} differs from the pivot cache of table '{}'",
            cache.name, cache.state.pivot_cache_id, link.name
        )));
    }
    let fields = definition
        .cache_fields()
        .list()
        .iter()
        .filter(|field| field.name() == cache.source_name)
        .count();
    if fields != 1 {
        return Err(binding_error(format!(
            "timeline cache '{}' source field '{}' is missing or ambiguous in pivot table '{}'",
            cache.name, cache.source_name, link.name
        )));
    }
    Ok(())
}

impl Workbook {
    fn timeline_style_known(&self, name: &str) -> bool {
        BUILTIN_TIMELINE_STYLES.contains(&name)
            || self
                .timeline_styles()
                .is_some_and(|styles| styles.styles.iter().any(|style| style.name == name))
    }

    /// Check the timeline views of one worksheet: their caches, styles and
    /// drawing frames, and that their names differ from `taken` (lowercase
    /// names of the other views). Returns the lowercase names of the views.
    fn check_sheet_timelines(
        &self,
        worksheet: &Worksheet,
        taken: &HashSet<String>,
    ) -> Result<Vec<String>, XlsxError> {
        let mut names = Vec::new();
        for timeline in worksheet.timelines() {
            timeline.validate().map_err(binding_error)?;
            let name = timeline.name.to_lowercase();
            if taken.contains(&name) || names.contains(&name) {
                return Err(binding_error(format!(
                    "duplicate timeline '{}'",
                    timeline.name
                )));
            }
            names.push(name);
            let wanted = timeline.cache.to_lowercase();
            if !self
                .timeline_caches()
                .iter()
                .any(|cache| cache.name.to_lowercase() == wanted)
            {
                return Err(binding_error(format!(
                    "timeline '{}' on sheet '{}' references unknown timeline cache '{}'",
                    timeline.name,
                    worksheet.name(),
                    timeline.cache
                )));
            }
            if let Some(style) = &timeline.style {
                if !self.timeline_style_known(style) {
                    return Err(binding_error(format!(
                        "timeline '{}' on sheet '{}' uses unknown timeline style '{style}'",
                        timeline.name,
                        worksheet.name()
                    )));
                }
            }
        }
        for frame in worksheet.worksheet_drawing().slicer_frames() {
            if frame.kind() == SlicerFrameKind::Timeline
                && !worksheet
                    .timelines()
                    .iter()
                    .any(|timeline| timeline.name == frame.slicer_name())
            {
                return Err(binding_error(format!(
                    "drawing frame on sheet '{}' references unknown timeline '{}'",
                    worksheet.name(),
                    frame.slicer_name()
                )));
            }
        }
        Ok(names)
    }

    /// Check one worksheet about to be written by a streaming writer: its
    /// timelines and frames, and the pivot tables the timeline caches link to
    /// on it. Returns the lowercase names of its timelines.
    pub(crate) fn validate_sheet_timelines(
        &self,
        worksheet: &Worksheet,
        other_names: &HashSet<String>,
    ) -> Result<Vec<String>, XlsxError> {
        let names = self.check_sheet_timelines(worksheet, other_names)?;
        for cache in self.timeline_caches() {
            for link in cache.linked_pivots() {
                if worksheet.sheet_id().parse::<u32>() == Ok(link.sheet_id) {
                    check_pivot_link(cache, link, worksheet)?;
                }
            }
        }
        Ok(names)
    }

    /// Check the timeline caches of a workbook whose sheets were streamed
    /// out: every sheet a cache links to was written, and the pivot cache it
    /// reads was written.
    pub(crate) fn validate_streamed_timeline_caches(
        &self,
        written_pivot_cache_ids: &HashSet<u32>,
    ) -> Result<(), XlsxError> {
        let mut names = self.cache_names_of_slicers();
        for cache in self.timeline_caches() {
            cache.validate().map_err(binding_error)?;
            if !names.insert(cache.name.to_lowercase()) {
                return Err(binding_error(format!(
                    "duplicate timeline cache '{}'",
                    cache.name
                )));
            }
            for link in cache.linked_pivots() {
                if sheet_by_id(self, link.sheet_id).is_none() {
                    return Err(binding_error(format!(
                        "timeline cache '{}' references missing sheet id {}",
                        cache.name, link.sheet_id
                    )));
                }
            }
            if !written_pivot_cache_ids.contains(&cache.state.pivot_cache_id) {
                return Err(binding_error(format!(
                    "timeline cache '{}' references missing pivot cache id {}",
                    cache.name, cache.state.pivot_cache_id
                )));
            }
        }
        Ok(())
    }

    fn cache_names_of_slicers(&self) -> HashSet<String> {
        self.slicer_caches()
            .iter()
            .map(|cache| cache.name.to_lowercase())
            .collect()
    }

    /// Check that every timeline binding resolves by identity: view to cache
    /// by name, cache to pivot tables by `(tabId, name)` and to its pivot
    /// cache by the x14 pivot cache id, and cache to its source field by name.
    /// Bindings that depend on worksheets that are not read yet are checked
    /// once they are read (and always at write time, when `complete`).
    pub(crate) fn validate_timelines(&self, complete: bool) -> Result<(), XlsxError> {
        let mut cache_names = self.cache_names_of_slicers();
        for cache in self.timeline_caches() {
            cache.validate().map_err(binding_error)?;
            if !cache_names.insert(cache.name.to_lowercase()) {
                return Err(binding_error(format!(
                    "duplicate timeline cache '{}'",
                    cache.name
                )));
            }
            for link in cache.linked_pivots() {
                let sheet = sheet_by_id(self, link.sheet_id).ok_or_else(|| {
                    binding_error(format!(
                        "timeline cache '{}' references missing sheet id {}",
                        cache.name, link.sheet_id
                    ))
                })?;
                if !sheet.is_inspectable() {
                    if complete {
                        return Err(binding_error(format!(
                            "timeline cache '{}' links sheet '{}', which was not read",
                            cache.name,
                            sheet.name()
                        )));
                    }
                    continue;
                }
                check_pivot_link(cache, link, sheet)?;
            }
            let found = self
                .sheet_collection_no_check()
                .iter()
                .filter(|sheet| sheet.is_inspectable())
                .flat_map(Worksheet::pivot_tables)
                .any(|table| {
                    table.pivot_cache_definition().slicer_cache_id()
                        == Some(cache.state.pivot_cache_id)
                });
            let retained = self.retained_pivot_caches().any(|(_, definition)| {
                definition.slicer_cache_id() == Some(cache.state.pivot_cache_id)
            });
            if !found && !retained && complete {
                return Err(binding_error(format!(
                    "timeline cache '{}' references missing pivot cache id {}",
                    cache.name, cache.state.pivot_cache_id
                )));
            }
        }
        let mut taken: HashSet<String> = self
            .sheet_collection_no_check()
            .iter()
            .filter(|sheet| sheet.is_inspectable())
            .flat_map(|sheet| {
                sheet
                    .slicers()
                    .iter()
                    .map(|slicer| slicer.name.to_lowercase())
            })
            .collect();
        for sheet in self
            .sheet_collection_no_check()
            .iter()
            .filter(|sheet| sheet.is_inspectable())
        {
            taken.extend(self.check_sheet_timelines(sheet, &taken)?);
        }
        Ok(())
    }

    /// Follow sheet removal: unlink its pivot tables and remove caches whose
    /// last linked pivot or last timeline view disappeared with the sheet.
    /// Returns the lowercase names of the removed caches.
    pub(crate) fn unlink_timelines_of_removed_sheet(
        &mut self,
        removed: &Worksheet,
    ) -> HashSet<String> {
        let removed_id = removed.sheet_id().parse::<u32>().ok();
        let removed_views: HashSet<_> = removed
            .timelines()
            .iter()
            .map(|view| view.cache.to_lowercase())
            .collect();
        let surviving_views: HashSet<_> = self
            .sheet_collection_no_check()
            .iter()
            .flat_map(Worksheet::timelines)
            .map(|view| view.cache.to_lowercase())
            .collect();
        let mut dropped = HashSet::new();
        self.timeline_caches_mut().retain_mut(|cache| {
            let before = cache.linked_pivots().len();
            if let Some(tables) = &mut cache.pivot_tables {
                tables.retain(|link| Some(link.sheet_id) != removed_id);
                if tables.is_empty() {
                    cache.pivot_tables = None;
                }
            }
            let name = cache.name.to_lowercase();
            let drop = (before > 0 && cache.linked_pivots().is_empty())
                || (removed_views.contains(&name) && !surviving_views.contains(&name));
            if drop {
                dropped.insert(name);
            }
            !drop
        });
        dropped
    }
}
