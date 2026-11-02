//! Editing timelines the way an Excel user can: create one for a date field of
//! a pivot table, select or clear a date range, change the level, connect
//! pivot tables, rename and delete.

use chrono::NaiveDate;

use super::{
    model::{
        Timeline,
        TimelineCache,
        TimelineLevel,
        TimelineSelection,
        TimelineState,
        parse_date_time,
    },
    selection::{
        bounds_of,
        pivot_date_filter,
        range_of_date_filter,
        replace_date_filter,
    },
};
use crate::{
    drawing::spreadsheet::{
        DrawingAnchor,
        DrawingObjectKey,
        SlicerFrame,
        SlicerFrameCompatibility,
        SlicerFrameKind,
    },
    structs::{
        DefinedName,
        PivotCacheSlicerProperties,
        SharedItemValue,
        SlicerPivotTable,
        SlicerPivotTarget,
        TIMELINE_DRAWING_NS,
        Workbook,
        Worksheet,
        XlsxError,
    },
    xml::xml_error,
};

/// A timeline to create (`SlicerCaches.Add2(pivot table, field, ,
/// xlTimeline)` followed by `Slicers.Add`).
#[derive(Clone, Debug)]
pub struct NewTimeline<'a> {
    /// Name of the worksheet that shows the timeline.
    pub sheet:       &'a str,
    /// Name of the timeline view, unique among the slicers and timelines of
    /// the workbook.
    pub name:        &'a str,
    /// Name of the worksheet holding the pivot table.
    pub pivot_sheet: &'a str,
    pub pivot_table: &'a str,
    /// Name of the date field of the pivot cache.
    pub field:       &'a str,
    /// Where the timeline sits on its worksheet.
    pub anchor:      DrawingAnchor,
}

fn same(left: &str, right: &str) -> bool {
    left.to_lowercase() == right.to_lowercase()
}

fn sheet_index(book: &Workbook, name: &str) -> Result<usize, XlsxError> {
    book.sheet_collection_no_check()
        .iter()
        .position(|sheet| sheet.name() == name)
        .ok_or_else(|| xml_error(format!("sheet '{name}' does not exist")))
}

fn sheet_id_of(sheet: &Worksheet) -> Result<u32, XlsxError> {
    sheet
        .sheet_id()
        .parse::<u32>()
        .map_err(|_| xml_error(format!("sheet '{}' has no numeric sheet id", sheet.name())))
}

const EMU_PER_PIXEL: i64 = 9525;

/// The width of column `index` (zero based) in EMU: Excel rounds a column to
/// whole pixels of the 7 pixel digit width of the default font, and a hidden
/// column has none. A sheet without a default width has 64 pixels.
#[allow(clippy::cast_possible_truncation)]
fn column_emu(sheet: &Worksheet, index: u32) -> i64 {
    let column = sheet
        .column_dimensions()
        .iter()
        .find(|column| column.col_num() == index + 1);
    if column.is_some_and(crate::structs::Column::hidden) {
        return 0;
    }
    let pixels = match column {
        Some(column) => (column.width() * 7.0).round() as i64,
        None => sheet
            .sheet_format_properties()
            .default_column_width_opt()
            .map_or(64, |width| (width * 7.0).round() as i64),
    };
    pixels * EMU_PER_PIXEL
}

/// The height of row `index` (zero based) in EMU: whole pixels, none when
/// hidden.
#[allow(clippy::cast_possible_truncation)]
fn row_emu(sheet: &Worksheet, index: u32) -> i64 {
    let row = sheet.row_dimension(index + 1);
    if row.is_some_and(crate::structs::Row::hidden) {
        return 0;
    }
    let points = row
        .and_then(crate::structs::Row::height_opt)
        .unwrap_or_else(|| sheet.sheet_format_properties().default_row_height());
    (points * 4.0 / 3.0).round() as i64 * EMU_PER_PIXEL
}

/// The position of a marker from the top left of the sheet, in EMU.
fn marker_emu(sheet: &Worksheet, marker: &crate::drawing::spreadsheet::MarkerType) -> (i64, i64) {
    (
        (0..marker.col())
            .map(|index| column_emu(sheet, index))
            .sum::<i64>()
            + i64::from(marker.col_off()),
        (0..marker.row())
            .map(|index| row_emu(sheet, index))
            .sum::<i64>()
            + i64::from(marker.row_off()),
    )
}

/// The `mc:Fallback` shape Excel draws for a timeline in a reader without
/// timeline support: a white box at the absolute position and size of the
/// frame that says what it is.
fn timeline_fallback(
    sheet: &Worksheet,
    anchor: &DrawingAnchor,
) -> Result<crate::drawing::charts::OpaqueElement, String> {
    let ((x, y), (cx, cy)) = match anchor {
        DrawingAnchor::OneCell { from, extent } => {
            (marker_emu(sheet, from), (extent.cx(), extent.cy()))
        }
        DrawingAnchor::TwoCell { from, to, .. } => {
            let (start, end) = (marker_emu(sheet, from), marker_emu(sheet, to));
            (start, (end.0 - start.0, end.1 - start.1))
        }
        DrawingAnchor::Absolute { position, extent } => {
            ((position.x(), position.y()), (extent.cx(), extent.cy()))
        }
    };
    let xml = format!(
        concat!(
            r#"<mc:Fallback xmlns:mc="{mc}" xmlns:xdr="{xdr}" xmlns:a="{a}">"#,
            r#"<xdr:sp macro="" textlink=""><xdr:nvSpPr><xdr:cNvPr id="0" name=""/>"#,
            r#"<xdr:cNvSpPr><a:spLocks noTextEdit="1"/></xdr:cNvSpPr></xdr:nvSpPr>"#,
            r#"<xdr:spPr><a:xfrm><a:off x="{x}" y="{y}"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm>"#,
            r#"<a:prstGeom prst="rect"><a:avLst/></a:prstGeom>"#,
            r#"<a:solidFill><a:prstClr val="white"/></a:solidFill>"#,
            r#"<a:ln w="1"><a:solidFill><a:prstClr val="green"/></a:solidFill></a:ln></xdr:spPr>"#,
            r#"<xdr:txBody><a:bodyPr vertOverflow="clip" horzOverflow="clip"/><a:lstStyle/>"#,
            r#"<a:p><a:r><a:rPr lang="en-US" sz="1100"/>"#,
            r#"<a:t>Timeline: Works in Excel 2013 or higher. Do not move or resize.</a:t>"#,
            r#"</a:r></a:p></xdr:txBody></xdr:sp></mc:Fallback>"#
        ),
        mc = crate::helper::const_str::MC_NS,
        xdr = crate::helper::const_str::SHEET_DRAWING_NS,
        a = crate::helper::const_str::DRAWINGML_MAIN_NS,
        x = x,
        y = y,
        cx = cx,
        cy = cy,
    );
    crate::xml::root(xml.as_bytes())
}

fn cache_position(book: &Workbook, name: &str) -> Result<usize, XlsxError> {
    book.timeline_caches()
        .iter()
        .position(|cache| same(&cache.name, name))
        .ok_or_else(|| xml_error(format!("timeline cache '{name}' does not exist")))
}

/// The date a pivot cache date item or bound names, at day precision.
fn date_of(text: &str) -> Result<NaiveDate, XlsxError> {
    parse_date_time(text)
        .map(|date_time| date_time.date())
        .map_err(xml_error)
}

/// The first and last date of a pivot cache field.
fn field_dates(field: &crate::structs::CacheField) -> Result<(NaiveDate, NaiveDate), XlsxError> {
    let shared = field.shared_items();
    let properties = shared.properties();
    if let (Some(first), Some(last)) = (&properties.min_date, &properties.max_date) {
        return Ok((date_of(first)?, date_of(last)?));
    }
    let mut dates = Vec::new();
    for item in shared.items() {
        if let SharedItemValue::Date(text) = item.scalar() {
            dates.push(date_of(text)?);
        }
    }
    match (dates.iter().min(), dates.iter().max()) {
        (Some(first), Some(last)) => Ok((*first, *last)),
        _ => Err(xml_error(format!(
            "the dates of pivot field '{}' are unknown",
            field.name()
        ))),
    }
}

fn identifier(text: &str) -> String {
    text.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

impl Workbook {
    fn view_names(&self) -> Vec<String> {
        self.sheet_collection_no_check()
            .iter()
            .flat_map(|sheet| {
                sheet
                    .slicers()
                    .iter()
                    .map(|slicer| slicer.name.clone())
                    .chain(
                        sheet
                            .timelines()
                            .iter()
                            .map(|timeline| timeline.name.clone()),
                    )
            })
            .collect()
    }

    fn cache_names(&self) -> Vec<String> {
        self.slicer_caches()
            .iter()
            .map(|cache| cache.name.clone())
            .chain(
                self.timeline_caches()
                    .iter()
                    .map(|cache| cache.name.clone()),
            )
            .chain(
                self.defined_names()
                    .iter()
                    .map(|name| name.name().to_owned()),
            )
            .collect()
    }

    /// The `(sheet index, table index)` of the pivot table `table` on the sheet
    /// `sheet`.
    fn pivot_position(&self, sheet: &str, table: &str) -> Result<(usize, usize), XlsxError> {
        let sheet_at = sheet_index(self, sheet)?;
        let tables: Vec<usize> = self.sheet_collection_no_check()[sheet_at]
            .pivot_tables()
            .iter()
            .enumerate()
            .filter(|(_, candidate)| candidate.pivot_table_definition().name() == table)
            .map(|(index, _)| index)
            .collect();
        match tables.as_slice() {
            [index] => Ok((sheet_at, *index)),
            [] => Err(xml_error(format!(
                "pivot table '{table}' does not exist on sheet '{sheet}'"
            ))),
            _ => Err(xml_error(format!(
                "pivot table '{table}' is ambiguous on sheet '{sheet}'"
            ))),
        }
    }

    /// The x14 pivot cache id of the pivot cache of a pivot table, given one
    /// when the cache has none (every pivot table sharing the cache gets it).
    fn ensure_slicer_cache_id(&mut self, sheet: usize, table: usize) -> u32 {
        let (existing, workbook_cache_id) = {
            let table = &self.sheet_collection_no_check()[sheet].pivot_tables()[table];
            (
                table.pivot_cache_definition().slicer_cache_id(),
                table.pivot_table_definition().cache_id(),
            )
        };
        if let Some(id) = existing {
            return id;
        }
        let next = self
            .sheet_collection_no_check()
            .iter()
            .flat_map(Worksheet::pivot_tables)
            .filter_map(|table| table.pivot_cache_definition().slicer_cache_id())
            .chain(
                self.retained_pivot_caches()
                    .filter_map(|(_, definition)| definition.slicer_cache_id()),
            )
            .max()
            .map_or(1, |max| max + 1);
        let (_, sheets) = self.timeline_parts_mut();
        for sheet in sheets {
            for table in sheet.pivot_tables_mut() {
                if table.pivot_table_definition().cache_id() == workbook_cache_id {
                    let mut properties = table
                        .pivot_cache_definition()
                        .slicer_properties()
                        .cloned()
                        .unwrap_or_else(PivotCacheSlicerProperties::default);
                    properties.pivot_cache_id = Some(next);
                    table
                        .pivot_cache_definition_mut()
                        .set_slicer_properties(properties);
                }
            }
        }
        next
    }

    /// Create a timeline for a date field of a pivot table, like Excel's
    /// `SlicerCaches.Add2` for a timeline followed by `Slicers.Add`. A timeline
    /// cache for the field exists once: when it does, only the view is added,
    /// and the pivot table must be connected to it already.
    ///
    /// # Errors
    /// Returns an error, with nothing changed, when a sheet, pivot table or
    /// field does not exist, the field holds no dates, or a name is taken.
    pub fn add_timeline(&mut self, new: &NewTimeline<'_>) -> Result<(), XlsxError> {
        self.try_read_sheet_collection()?;
        let host = sheet_index(self, new.sheet)?;
        let (pivot_sheet, pivot_table) = self.pivot_position(new.pivot_sheet, new.pivot_table)?;
        if new.name.is_empty() {
            return Err(xml_error("a timeline needs a name"));
        }
        if self.view_names().iter().any(|name| same(name, new.name)) {
            return Err(xml_error(format!(
                "a slicer or timeline named '{}' already exists",
                new.name
            )));
        }
        let (field_name, field_index, first, last) = {
            let table = &self.sheet_collection_no_check()[pivot_sheet].pivot_tables()[pivot_table];
            let fields: Vec<_> = table
                .pivot_cache_definition()
                .cache_fields()
                .list()
                .iter()
                .enumerate()
                .filter(|(_, field)| field.name() == new.field)
                .collect();
            let [(field_index, field)] = fields.as_slice() else {
                return Err(xml_error(format!(
                    "pivot field '{}' is missing or ambiguous in pivot table '{}'",
                    new.field, new.pivot_table
                )));
            };
            if field.shared_items().properties().contains_date != Some(true) {
                return Err(xml_error(format!(
                    "pivot field '{}' does not hold dates, so it has no timeline",
                    new.field
                )));
            }
            let (first, last) = field_dates(field)?;
            (field.name().to_owned(), *field_index, first, last)
        };
        let sheet_id = sheet_id_of(&self.sheet_collection_no_check()[pivot_sheet])?;
        let link = SlicerPivotTable {
            sheet_id,
            name: new.pivot_table.to_owned(),
        };
        let mut adopted = None;
        let connected = self
            .timeline_caches()
            .iter()
            .any(|cache| cache.source_name == field_name && cache.linked_pivots().contains(&&link));
        if !connected {
            let table = &self.sheet_collection_no_check()[pivot_sheet].pivot_tables()[pivot_table];
            // Excel takes over the date range a field already filters by.
            if let Some((kind, values)) = u32::try_from(field_index)
                .ok()
                .and_then(|index| pivot_date_filter(table, index))
            {
                adopted = Some(
                    range_of_date_filter(&kind, &values, self.date1904()).map_err(|message| {
                        xml_error(format!(
                            "pivot table '{}': field '{field_name}': {message}",
                            new.pivot_table
                        ))
                    })?,
                );
            }
        }
        let pivot_cache_id = self.ensure_slicer_cache_id(pivot_sheet, pivot_table);
        let existing = self.timeline_caches().iter().position(|cache| {
            cache.state.pivot_cache_id == pivot_cache_id && cache.source_name == field_name
        });
        let (cache_name, selection_level) = if let Some(index) = existing {
            let cache = &self.timeline_caches()[index];
            if !cache.linked_pivots().contains(&&link) {
                return Err(xml_error(format!(
                    "timeline cache '{}' of field '{field_name}' exists and is not connected to \
                     pivot table '{}'; connect it first",
                    cache.name, new.pivot_table
                )));
            }
            (
                cache.name.clone(),
                TimelineCache::selection_level_of(cache.selection().map_err(xml_error)?),
            )
        } else {
            let taken = self.cache_names();
            let base = format!("NativeTimeline_{}", identifier(&field_name));
            let mut name = base.clone();
            let mut number = 1;
            while taken.iter().any(|existing| same(existing, &name)) {
                name = format!("{base}{number}");
                number += 1;
            }
            let mut address = DefinedName::default();
            address.set_name(name.clone());
            address.set_address("#N/A");
            self.defined_names_mut().push(address);
            self.timeline_caches_mut().push(TimelineCache {
                name:         name.clone(),
                source_name:  field_name.clone(),
                uid:          None,
                pivot_tables: Some(vec![link]),
                state:        TimelineState {
                    single_range_filter_state: None,
                    minimal_refresh_version: 6,
                    last_refresh_version: 6,
                    pivot_cache_id,
                    filter_type: "unknown".into(),
                    selection: None,
                    bounds: bounds_of(first, last).map_err(xml_error)?,
                    extensions: None,
                },
                pivot_filter: None,
                extensions:   None,
            });
            (name, TimelineLevel::Months)
        };
        let sheet = self.sheet_mut(host)?;
        let fallback = timeline_fallback(sheet, &new.anchor).map_err(xml_error)?;
        sheet.timelines_mut().push(Timeline {
            name: new.name.to_owned(),
            cache: cache_name.clone(),
            caption: Some(field_name.clone()),
            selection_level,
            ..Timeline::default()
        });
        let mut frame = SlicerFrame::new_timeline(new.anchor.clone(), new.name);
        frame
            .frame_properties_mut()
            .non_visual_drawing_properties_mut()
            .set_name(new.name);
        frame.set_compatibility(Some(SlicerFrameCompatibility {
            required_namespaces: vec![("tsle".to_owned(), TIMELINE_DRAWING_NS.to_owned())],
            fallback: Some(fallback),
            ..SlicerFrameCompatibility::default()
        }));
        sheet.worksheet_drawing_mut().add_slicer_frame(frame);
        if let Some(selection) = adopted {
            // Excel takes the range into the timeline and drops the pivot
            // table's own filter for the field.
            self.set_timeline_selection(&cache_name, selection)?;
            let date1904 = self.date1904();
            let definition = self.sheet_mut(pivot_sheet)?.pivot_tables_mut()[pivot_table]
                .pivot_table_definition_mut();
            let filters = replace_date_filter(
                definition.filters(),
                u32::try_from(field_index).map_err(|_| xml_error("pivot field index"))?,
                &field_name,
                TimelineSelection::All,
                date1904,
            )
            .map_err(xml_error)?;
            definition.set_filters(filters);
        }
        Ok(())
    }

    /// Select a date range of a timeline, or clear it, like Excel's
    /// `TimelineState.SetFilterDateRange` and `SlicerCache.ClearDateFilter`.
    /// Every pivot table the timeline filters takes the filter together, or
    /// none does.
    ///
    /// # Errors
    /// Returns an error naming the timeline cache, with nothing changed, when
    /// a linked pivot table does not resolve, cannot take the date filter, or
    /// the range lies outside the dates of the data.
    pub fn set_timeline_selection(
        &mut self,
        cache: &str,
        selection: TimelineSelection,
    ) -> Result<(), XlsxError> {
        self.try_read_sheet_collection()?;
        let date1904 = self.date1904();
        let position = cache_position(self, cache)?;
        let (caches, sheets) = self.timeline_parts_mut();
        let cache = &mut caches[position];
        let mut targets = Vec::new();
        for sheet in &mut *sheets {
            let Ok(sheet_id) = sheet.sheet_id().parse::<u32>() else {
                continue;
            };
            for table in sheet.pivot_tables_mut() {
                targets.push(SlicerPivotTarget { sheet_id, table });
            }
        }
        let plan = cache
            .plan_selection(selection, &targets, date1904)
            .map_err(xml_error)?;
        plan.apply(cache, &mut targets).map_err(xml_error)?;
        drop(targets);
        if matches!(selection, TimelineSelection::Range { .. }) {
            let level = TimelineCache::selection_level_of(selection);
            for sheet in sheets {
                for timeline in sheet.timelines_mut() {
                    if same(&timeline.cache, &cache.name) {
                        timeline.selection_level = level;
                    }
                }
            }
        }
        Ok(())
    }

    /// Change the time level a timeline view shows (years, quarters, months or
    /// days).
    ///
    /// # Errors
    /// Returns an error when no timeline has the name.
    pub fn set_timeline_level(
        &mut self,
        timeline: &str,
        level: TimelineLevel,
    ) -> Result<(), XlsxError> {
        self.timeline_mut(timeline)?.level = level;
        Ok(())
    }

    /// The timeline view called `name`, to change its caption, header,
    /// selection label, time level control, scrollbar, scroll position or
    /// style.
    ///
    /// # Errors
    /// Returns an error when no timeline has the name.
    pub fn timeline_mut(&mut self, name: &str) -> Result<&mut Timeline, XlsxError> {
        self.try_read_sheet_collection()?;
        let (_, sheets) = self.timeline_parts_mut();
        sheets
            .iter_mut()
            .flat_map(Worksheet::timelines_mut)
            .find(|timeline| same(&timeline.name, name))
            .ok_or_else(|| xml_error(format!("timeline '{name}' does not exist")))
    }

    /// Rename a timeline view; its drawing frame follows.
    ///
    /// # Errors
    /// Returns an error when no timeline has the name or the new name is
    /// taken.
    pub fn rename_timeline(&mut self, name: &str, new_name: &str) -> Result<(), XlsxError> {
        self.try_read_sheet_collection()?;
        if new_name.is_empty() {
            return Err(xml_error("a timeline needs a name"));
        }
        if !same(name, new_name) && self.view_names().iter().any(|taken| same(taken, new_name)) {
            return Err(xml_error(format!(
                "a slicer or timeline named '{new_name}' already exists"
            )));
        }
        let (_, sheets) = self.timeline_parts_mut();
        for sheet in sheets {
            let Some(timeline) = sheet
                .timelines_mut()
                .iter_mut()
                .find(|timeline| same(&timeline.name, name))
            else {
                continue;
            };
            let old = std::mem::replace(&mut timeline.name, new_name.to_owned());
            for frame in sheet.worksheet_drawing_mut().slicer_frames_mut() {
                if frame.kind() == SlicerFrameKind::Timeline && frame.slicer_name() == old {
                    frame.set_slicer_name(new_name);
                    let properties = frame
                        .frame_properties_mut()
                        .non_visual_drawing_properties_mut();
                    if properties.name() == old {
                        properties.set_name(new_name);
                    }
                }
            }
            return Ok(());
        }
        Err(xml_error(format!("timeline '{name}' does not exist")))
    }

    /// Connect a pivot table to a timeline, like `SlicerCache.PivotTables.
    /// AddPivotTable`: the pivot table takes the timeline's current date
    /// filter.
    ///
    /// # Errors
    /// Returns an error, with nothing changed, when the pivot table is already
    /// connected, reads another pivot cache, lacks the timeline's field or
    /// cannot take the filter.
    pub fn connect_timeline_pivot(
        &mut self,
        cache: &str,
        pivot_sheet: &str,
        pivot_table: &str,
    ) -> Result<(), XlsxError> {
        self.try_read_sheet_collection()?;
        let date1904 = self.date1904();
        let position = cache_position(self, cache)?;
        let (sheet_at, _) = self.pivot_position(pivot_sheet, pivot_table)?;
        let sheet_id = sheet_id_of(&self.sheet_collection_no_check()[sheet_at])?;
        let link = SlicerPivotTable {
            sheet_id,
            name: pivot_table.to_owned(),
        };
        let (caches, sheets) = self.timeline_parts_mut();
        let cache = &mut caches[position];
        if cache.linked_pivots().contains(&&link) {
            return Err(xml_error(format!(
                "pivot table '{pivot_table}' is already connected to timeline cache '{}'",
                cache.name
            )));
        }
        let mut probe = cache.clone();
        probe.pivot_tables = Some(vec![link.clone()]);
        let mut targets = Vec::new();
        for sheet in sheets {
            let Ok(id) = sheet.sheet_id().parse::<u32>() else {
                continue;
            };
            for table in sheet.pivot_tables_mut() {
                targets.push(SlicerPivotTarget {
                    sheet_id: id,
                    table,
                });
            }
        }
        let selection = cache.selection().map_err(xml_error)?;
        let plan = probe
            .plan_selection(selection, &targets, date1904)
            .map_err(xml_error)?;
        plan.apply(&mut probe, &mut targets).map_err(xml_error)?;
        cache.pivot_tables.get_or_insert_with(Vec::new).push(link);
        Ok(())
    }

    /// Disconnect a pivot table from a timeline. The pivot table keeps the date
    /// filter it has, as in Excel.
    ///
    /// # Errors
    /// Returns an error when the pivot table is not connected.
    pub fn disconnect_timeline_pivot(
        &mut self,
        cache: &str,
        pivot_sheet: &str,
        pivot_table: &str,
    ) -> Result<(), XlsxError> {
        self.try_read_sheet_collection()?;
        let position = cache_position(self, cache)?;
        let (sheet_at, _) = self.pivot_position(pivot_sheet, pivot_table)?;
        let sheet_id = sheet_id_of(&self.sheet_collection_no_check()[sheet_at])?;
        let cache = &mut self.timeline_caches_mut()[position];
        let links = cache
            .pivot_tables
            .as_mut()
            .filter(|links| {
                links
                    .iter()
                    .any(|link| link.sheet_id == sheet_id && link.name == pivot_table)
            })
            .ok_or_else(|| {
                xml_error(format!(
                    "pivot table '{pivot_table}' is not connected to timeline cache '{}'",
                    cache.name
                ))
            })?;
        links.retain(|link| !(link.sheet_id == sheet_id && link.name == pivot_table));
        if links.is_empty() {
            cache.pivot_tables = None;
        }
        Ok(())
    }

    /// Delete a timeline view with its drawing frame. The last view of a
    /// timeline cache takes the cache and its name with it; the pivot tables
    /// keep their date filters, as in Excel.
    ///
    /// # Errors
    /// Returns an error when no timeline has the name.
    pub fn remove_timeline(&mut self, name: &str) -> Result<(), XlsxError> {
        self.try_read_sheet_collection()?;
        let (caches, sheets) = self.timeline_parts_mut();
        let mut removed = None;
        for sheet in &mut *sheets {
            let Some(index) = sheet
                .timelines()
                .iter()
                .position(|timeline| same(&timeline.name, name))
            else {
                continue;
            };
            let timeline = sheet.timelines_mut().remove(index);
            let drawing = sheet.worksheet_drawing_mut();
            let frame_ids: Vec<u32> = drawing
                .slicer_frames()
                .iter()
                .filter(|frame| {
                    frame.kind() == SlicerFrameKind::Timeline
                        && frame.slicer_name() == timeline.name
                })
                .map(SlicerFrame::frame_id)
                .collect();
            drawing
                .slicer_frames_mut()
                .retain(|frame| !frame_ids.contains(&frame.frame_id()));
            let order = drawing
                .object_order()
                .iter()
                .filter(
                    |key| !matches!(key, DrawingObjectKey::Slicer(id) if frame_ids.contains(id)),
                )
                .cloned()
                .collect();
            drawing.set_object_order(order);
            removed = Some(timeline.cache);
            break;
        }
        let cache =
            removed.ok_or_else(|| xml_error(format!("timeline '{name}' does not exist")))?;
        let used = sheets
            .iter()
            .flat_map(Worksheet::timelines)
            .any(|timeline| same(&timeline.cache, &cache));
        if !used {
            caches.retain(|existing| !same(&existing.name, &cache));
            self.defined_names_mut()
                .retain(|defined| defined.has_local_sheet_id() || !same(defined.name(), &cache));
            self.remove_unused_retained_pivot_caches();
        }
        Ok(())
    }
}

impl Worksheet {
    /// The drawing frame of the timeline view `name`: its position and size.
    #[must_use]
    pub fn timeline_frame(&self, name: &str) -> Option<&SlicerFrame> {
        self.worksheet_drawing()
            .slicer_frames()
            .iter()
            .find(|frame| frame.kind() == SlicerFrameKind::Timeline && frame.slicer_name() == name)
    }

    pub fn timeline_frame_mut(&mut self, name: &str) -> Option<&mut SlicerFrame> {
        self.worksheet_drawing_mut()
            .slicer_frames_mut()
            .iter_mut()
            .find(|frame| frame.kind() == SlicerFrameKind::Timeline && frame.slicer_name() == name)
    }
}
