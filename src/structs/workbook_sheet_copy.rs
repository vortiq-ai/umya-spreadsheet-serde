use std::collections::{
    HashMap,
    HashSet,
};

use super::Workbook;
use crate::{
    DefinedName,
    Worksheet,
    XlsxError,
    XlsxErrorKind,
    drawing::charts::{
        ChartReferenceEdit,
        chart_reference_roles::visit_mut,
        reference_edit_engine::{
            PreparedReferenceEdit,
            edit_prepared_formula_text,
        },
    },
    helper::{
        address::{
            name_eq_ignore_case,
            split_reference_address,
            unescape_sheet_name,
        },
        formula::chart_formula_operands,
    },
    structs::{
        SlicerCacheExtension,
        SlicerSource,
    },
};

fn copy_error(message: &'static str) -> XlsxError {
    XlsxError::with_message(XlsxErrorKind::InvalidArgument, message)
}

fn next_id(ids: impl Iterator<Item = u32>) -> Result<u32, XlsxError> {
    ids.max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or_else(|| copy_error("copy identities exhausted"))
}

fn numbered_name(
    base: &str,
    separator: &str,
    first: u32,
    taken: &mut HashSet<String>,
) -> Result<String, XlsxError> {
    let mut suffix = first;
    loop {
        let name = format!("{base}{separator}{suffix}");
        if taken.insert(name.to_lowercase()) {
            return Ok(name);
        }
        suffix = suffix
            .checked_add(1)
            .ok_or_else(|| copy_error("copy names exhausted"))?;
    }
}

fn rewrite_tables(text: &str, tables: &HashMap<String, String>) -> String {
    let mut result = text.to_owned();
    let replacements = chart_formula_operands(text)
        .into_iter()
        .filter_map(|operand| {
            let bracket = operand.text.find('[').unwrap_or(operand.text.len());
            tables
                .get(&operand.text[..bracket].to_lowercase())
                .map(|name| (operand.span, format!("{name}{}", &operand.text[bracket..])))
        })
        .collect::<Vec<_>>();
    for (span, value) in replacements.into_iter().rev() {
        result.replace_range(span, &value);
    }
    result
}

fn copied_view_name(name: &str, taken: &mut HashSet<String>) -> Result<String, XlsxError> {
    let base = name
        .rsplit_once(' ')
        .filter(|(_, suffix)| suffix.parse::<u32>().is_ok())
        .map_or(name, |(base, _)| base);
    numbered_name(base, " ", 1, taken)
}

impl Worksheet {
    // Existing native identities distinguish an in-workbook duplicate from
    // an imported sheet whose sheetId alone happens to collide.
    pub(super) fn has_same_copy_identities(&self, other: &Self) -> bool {
        let drawing = self.worksheet_drawing();
        let other_drawing = other.worksheet_drawing();
        drawing.object_order().len() == other_drawing.object_order().len()
            && drawing
                .object_order()
                .iter()
                .all(|key| other_drawing.object_order().contains(key))
            && drawing.chart_collection().len() == other_drawing.chart_collection().len()
            && drawing.extended_charts().len() == other_drawing.extended_charts().len()
            && self.tables().len() == other.tables().len()
            && self.tables().iter().all(|table| {
                other.tables().iter().any(|candidate| {
                    table.id() == candidate.id()
                        && table.name() == candidate.name()
                        && table.display_name() == candidate.display_name()
                })
            })
            && self.slicers().len() == other.slicers().len()
            && self.slicers().iter().all(|view| {
                other.slicers().iter().any(|candidate| {
                    view.uid == candidate.uid
                        && view.name == candidate.name
                        && view.cache == candidate.cache
                })
            })
            && self.timelines().len() == other.timelines().len()
            && self.timelines().iter().all(|view| {
                other.timelines().iter().any(|candidate| {
                    view.uid == candidate.uid
                        && view.name == candidate.name
                        && view.cache == candidate.cache
                })
            })
    }
}

impl Workbook {
    pub(super) fn read_sheet_copy(
        &mut self,
        source: usize,
        copy: &mut Worksheet,
    ) -> Result<(), XlsxError> {
        if !copy.is_deserialized() {
            // Inspect the candidate in the source slot: appending it before
            // allocating identities would create duplicate slicer bindings.
            std::mem::swap(&mut self.work_sheet_collection[source], copy);
            let result = self.try_read_sheet(source).map(|_| ());
            std::mem::swap(&mut self.work_sheet_collection[source], copy);
            result?;
        }
        Ok(())
    }

    /// Prepare the copied owner and its workbook dependencies before insertion.
    /// The caller stages this on a workbook clone so a refusal is atomic.
    pub(super) fn prepare_sheet_copy(
        &mut self,
        source: usize,
        copy: &mut Worksheet,
    ) -> Result<(), XlsxError> {
        self.try_read_sheet_collection()?;
        let old = self.work_sheet_collection[source].name().to_owned();
        let old_id = self.work_sheet_collection[source]
            .sheet_id()
            .parse::<u32>()
            .map_err(|_| copy_error("invalid source sheet id"))?;
        let id = next_id(
            self.work_sheet_collection
                .iter()
                .map(|sheet| sheet.sheet_id().parse::<u32>())
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| copy_error("invalid worksheet id"))?
                .into_iter(),
        )?;
        copy.set_sheet_id(id.to_string());

        // Imported tables already have IDs. Assign the writer's existing
        // implicit IDs to memory-authored tables now, before deriving
        // the copy identities.
        let mut ids = self
            .work_sheet_collection
            .iter()
            .flat_map(Worksheet::tables)
            .filter_map(crate::Table::id)
            .collect::<HashSet<_>>();
        for sheet in &mut self.work_sheet_collection {
            for table in sheet.tables_mut() {
                if table.id().is_none() {
                    let mut id = 1u32;
                    while ids.contains(&id) {
                        id = id
                            .checked_add(1)
                            .ok_or_else(|| copy_error("table ids exhausted"))?;
                    }
                    table.set_id(id);
                    ids.insert(id);
                }
            }
        }
        let all_names = self.defined_names.iter().chain(
            self.work_sheet_collection
                .iter()
                .flat_map(Worksheet::defined_names),
        );
        let mut taken = all_names
            .map(|name| name.name().to_lowercase())
            .collect::<HashSet<_>>();
        taken.extend(
            self.work_sheet_collection
                .iter()
                .flat_map(Worksheet::tables)
                .flat_map(|table| {
                    [
                        table.name().to_lowercase(),
                        table.display_name().to_lowercase(),
                    ]
                }),
        );
        taken.extend(
            self.slicer_caches
                .iter()
                .map(|cache| cache.name.to_lowercase()),
        );
        taken.extend(
            self.timeline_caches
                .iter()
                .map(|cache| cache.name.to_lowercase()),
        );
        let mut table_names = HashMap::new();
        let mut table_ids = HashMap::new();
        for table in copy.tables_mut() {
            let old_table_id = table.id().or_else(|| {
                self.work_sheet_collection[source]
                    .tables()
                    .iter()
                    .find(|source| name_eq_ignore_case(source.name(), table.name()))
                    .and_then(crate::Table::id)
            });
            let id = next_id(ids.iter().copied())?;
            ids.insert(id);
            let first = id
                .checked_add(1)
                .ok_or_else(|| copy_error("table names exhausted"))?;
            let name = numbered_name(table.name(), "", first, &mut taken)?;
            table_names.insert(table.name().to_lowercase(), name.clone());
            table_names.insert(table.display_name().to_lowercase(), name.clone());
            if let Some(old_id) = old_table_id {
                table_ids.insert(old_id, id);
            }
            table.set_id(id);
            table.set_name(&name);
            table.set_display_name(&name);
        }

        self.copy_sheet_names(source, copy, &old)?;
        let edit = ChartReferenceEdit::RenameSheet {
            old: old.clone(),
            new: copy.name().to_owned(),
        };
        let new = copy.name().to_owned();
        let prepared = PreparedReferenceEdit::new(&edit)
            .map_err(|_| copy_error("invalid sheet copy references"))?;
        let rewrite = |text: &str| {
            rewrite_tables(
                &edit_prepared_formula_text(text, &old, &prepared, false).text,
                &table_names,
            )
        };
        for name in copy.defined_names_mut() {
            name.set_address(rewrite(&name.address()));
        }
        for cell in copy.cells_mut() {
            if let Some(original) = cell.cell_value().formula_obj() {
                let mut formula = original.clone();
                formula.rewrite_reference_texts(&rewrite);
                cell.cell_value_mut().set_formula_obj(formula);
            }
        }
        copy.visit_chart_spaces_mut(&mut |space| {
            visit_mut(space, &mut |_, mut target| {
                target.replace_for_sheet_rename(&rewrite(&target.formula().address_str()));
            });
            space.rename_pivot_source_sheet(&old, &new);
        });
        copy.worksheet_drawing_mut()
            .visit_extended_spaces_mut(&mut |space| {
                space.visit_formulas_mut(&mut |formula| {
                    formula.replace_reference_text_preserving_address_identity(&rewrite(
                        &formula.address_str(),
                    ));
                });
            });
        let extension_formulas = copy.prepare_extension_formula_edits(&old, &prepared);
        copy.apply_extension_formula_edits(extension_formulas);
        self.copy_sheet_slicer_dependencies(copy, old_id, id, &table_ids, &mut taken)?;
        self.prepare_sheet_copy_chart_names(source, copy)?;
        Ok(())
    }

    fn copy_sheet_names(
        &self,
        source: usize,
        copy: &mut Worksheet,
        old: &str,
    ) -> Result<(), XlsxError> {
        let scope = u32::try_from(self.work_sheet_collection.len())
            .map_err(|_| copy_error("too many worksheets"))?;
        let managed = self
            .slicer_caches
            .iter()
            .map(|cache| cache.name.to_lowercase())
            .chain(
                self.timeline_caches
                    .iter()
                    .map(|cache| cache.name.to_lowercase()),
            )
            .collect::<HashSet<_>>();
        let mut names = Vec::new();
        let mut candidates = self
            .defined_names
            .iter()
            .chain(
                self.work_sheet_collection
                    .iter()
                    .flat_map(Worksheet::defined_names),
            )
            .collect::<Vec<_>>();
        candidates.sort_by_key(|name| !name.has_local_sheet_id());
        for name in candidates {
            if (!name.has_local_sheet_id() && name.name().starts_with("_xlchart."))
                || managed.contains(&name.name().to_lowercase())
            {
                continue;
            }
            let local = name.has_local_sheet_id();
            let refers_to_source = chart_formula_operands(&name.address())
                .iter()
                .any(|operand| {
                    let (sheet, _) = split_reference_address(operand.text);
                    (!sheet.is_empty() && name_eq_ignore_case(&unescape_sheet_name(sheet), old))
                        || self.work_sheet_collection[source]
                            .tables()
                            .iter()
                            .any(|table| {
                                let table_name =
                                    operand.text.split('[').next().unwrap_or(operand.text);
                                name_eq_ignore_case(table_name, table.name())
                                    || name_eq_ignore_case(table_name, table.display_name())
                            })
                });
            if (local && name.local_sheet_id() as usize == source) || (!local && refers_to_source) {
                let mut name = name.clone();
                name.set_local_sheet_id(scope);
                if !names
                    .iter()
                    .any(|existing: &DefinedName| name_eq_ignore_case(existing.name(), name.name()))
                {
                    names.push(name);
                }
            }
        }
        *copy.defined_names_mut() = names;
        Ok(())
    }

    fn copy_sheet_slicer_dependencies(
        &mut self,
        copy: &mut Worksheet,
        old_id: u32,
        id: u32,
        table_ids: &HashMap<u32, u32>,
        taken: &mut HashSet<String>,
    ) -> Result<(), XlsxError> {
        let mut view_names = self
            .work_sheet_collection
            .iter()
            .flat_map(|sheet| {
                sheet
                    .slicers()
                    .iter()
                    .map(|view| view.name.to_lowercase())
                    .chain(
                        sheet
                            .timelines()
                            .iter()
                            .map(|view| view.name.to_lowercase()),
                    )
            })
            .collect::<HashSet<_>>();
        let pivots = copy
            .pivot_tables()
            .iter()
            .map(|pivot| pivot.pivot_table_definition().name().to_lowercase())
            .collect::<HashSet<_>>();
        let remap_links = |links: &mut Vec<crate::structs::SlicerPivotTable>| {
            links.retain(|link| {
                link.sheet_id == old_id && pivots.contains(&link.name.to_lowercase())
            });
            for link in links {
                link.sheet_id = id;
            }
        };
        let mut caches = HashMap::new();
        let mut frames = HashMap::new();
        for view in copy.slicers_mut() {
            let original_name = view.name.clone();
            let key = view.cache.to_lowercase();
            let name = if let Some(name) = caches.get(&key) {
                String::clone(name)
            } else {
                let mut cache = self
                    .slicer_caches
                    .iter()
                    .find(|cache| name_eq_ignore_case(&cache.name, &view.cache))
                    .cloned()
                    .ok_or_else(|| copy_error("copied slicer references a missing cache"))?;
                let owns_source = match &cache.source {
                    Some(SlicerSource::Table(source)) => table_ids.contains_key(&source.table_id),
                    _ => cache.linked_pivots().into_iter().any(|link| {
                        link.sheet_id == old_id && pivots.contains(&link.name.to_lowercase())
                    }),
                };
                if !owns_source {
                    caches.insert(key, cache.name.clone());
                    view.name = copied_view_name(&view.name, &mut view_names)?;
                    view.uid = None;
                    frames.insert(original_name, view.name.clone());
                    continue;
                }
                let name = numbered_name(&cache.name, "", 1, taken)?;
                cache.name.clone_from(&name);
                cache.uid = None;
                if let Some(SlicerSource::Table(source)) = &mut cache.source {
                    if let Some(id) = table_ids.get(&source.table_id) {
                        source.table_id = *id;
                    }
                }
                if let Some(links) = &mut cache.pivot_tables {
                    remap_links(links);
                }
                for extension in &mut cache.extensions {
                    if let SlicerCacheExtension::PivotTables { tables, .. } = extension {
                        remap_links(tables);
                    }
                }
                self.copy_managed_name(&view.cache, &name);
                self.slicer_caches.push(cache);
                caches.insert(key, name.clone());
                name
            };
            view.cache = name;
            view.name = copied_view_name(&view.name, &mut view_names)?;
            view.uid = None;
            frames.insert(original_name, view.name.clone());
        }
        let mut timeline_caches = HashMap::new();
        for view in copy.timelines_mut() {
            let original_name = view.name.clone();
            let key = view.cache.to_lowercase();
            let name = if let Some(name) = timeline_caches.get(&key) {
                String::clone(name)
            } else {
                let mut cache = self
                    .timeline_caches
                    .iter()
                    .find(|cache| name_eq_ignore_case(&cache.name, &view.cache))
                    .cloned()
                    .ok_or_else(|| copy_error("copied timeline references a missing cache"))?;
                if !cache.linked_pivots().into_iter().any(|link| {
                    link.sheet_id == old_id && pivots.contains(&link.name.to_lowercase())
                }) {
                    timeline_caches.insert(key, cache.name.clone());
                    view.name = copied_view_name(&view.name, &mut view_names)?;
                    view.uid = None;
                    frames.insert(original_name, view.name.clone());
                    continue;
                }
                let name = numbered_name(&cache.name, "", 1, taken)?;
                cache.name.clone_from(&name);
                cache.uid = None;
                if let Some(links) = &mut cache.pivot_tables {
                    remap_links(links);
                }
                self.copy_managed_name(&view.cache, &name);
                self.timeline_caches.push(cache);
                timeline_caches.insert(key, name.clone());
                name
            };
            view.cache = name;
            view.name = copied_view_name(&view.name, &mut view_names)?;
            view.uid = None;
            frames.insert(original_name, view.name.clone());
        }
        for frame in copy.worksheet_drawing_mut().slicer_frames_mut() {
            if let Some(name) = frames.get(frame.slicer_name()) {
                frame.set_slicer_name(name);
            }
        }
        Ok(())
    }

    fn copy_managed_name(&mut self, old: &str, new: &str) {
        if let Some(mut name) = self
            .defined_names
            .iter()
            .chain(
                self.work_sheet_collection
                    .iter()
                    .flat_map(Worksheet::defined_names),
            )
            .find(|name| !name.has_local_sheet_id() && name_eq_ignore_case(name.name(), old))
            .cloned()
        {
            name.set_name(new);
            self.defined_names.push(name);
        }
    }
}
