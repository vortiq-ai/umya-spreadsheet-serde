use std::collections::HashSet;

use super::model::{
    Timeline,
    TimelineCache,
    TimelineSelection,
};
use crate::{
    drawing::charts::OpaqueElement,
    helper::const_str::SHEET_MAIN_NS,
    structs::SlicerPivotTable,
    xml::{
        attr,
        required_unsigned,
    },
};

impl Timeline {
    /// # Errors
    /// Returns an error when the view lacks its name or its cache.
    pub fn validate(&self) -> Result<(), String> {
        if self.name.is_empty() || self.cache.is_empty() {
            return Err("timeline name and cache are required".into());
        }
        if let Some(position) = &self.scroll_position {
            super::model::parse_date_time(position)?;
        }
        Ok(())
    }
}

impl TimelineCache {
    /// # Errors
    /// Returns an error when the cache identity, links, state or selection
    /// are inconsistent.
    pub fn validate(&self) -> Result<(), String> {
        if self.name.is_empty() || self.source_name.is_empty() {
            return Err("timeline cache name and source are required".into());
        }
        let mut pivots = HashSet::new();
        for pivot in self.linked_pivots() {
            if pivot.name.is_empty() || !pivots.insert((pivot.sheet_id, &pivot.name)) {
                return Err(format!(
                    "timeline cache '{}' has a duplicate or unnamed pivot binding",
                    self.name
                ));
            }
        }
        if self.pivot_tables.as_ref().is_some_and(Vec::is_empty) {
            return Err(format!(
                "timeline cache '{}' has an empty pivot-table list",
                self.name
            ));
        }
        // Excel cannot open a cache whose `timelinePivotFilter` has no
        // `autoFilter`.
        if self
            .pivot_filter
            .as_ref()
            .is_some_and(|filter| filter.auto_filter.is_none())
        {
            return Err(format!(
                "timeline cache '{}' has a timeline pivot filter without its autoFilter, which \
                 Excel cannot open",
                self.name
            ));
        }
        let state = &self.state;
        if !crate::structs::pivot_table_definition::is_pivot_filter_type(&state.filter_type) {
            return Err(format!(
                "timeline cache '{}' has an invalid filter type",
                self.name
            ));
        }
        let (bounds_start, bounds_end) = (state.bounds.start()?, state.bounds.end()?);
        if bounds_start > bounds_end {
            return Err(format!("timeline cache '{}' has empty bounds", self.name));
        }
        if let Some(selection) = &state.selection {
            if selection.start()? > selection.end()? {
                return Err(format!(
                    "timeline cache '{}' selects an empty range",
                    self.name
                ));
            }
        }
        Ok(())
    }

    #[must_use]
    pub fn linked_pivots(&self) -> Vec<&SlicerPivotTable> {
        self.pivot_tables.iter().flatten().collect()
    }

    /// The selected range, from the cache state.
    ///
    /// # Errors
    /// Returns an error when the stored dates are not valid.
    pub fn selection(&self) -> Result<TimelineSelection, String> {
        match &self.state.selection {
            None => Ok(TimelineSelection::All),
            Some(range) => Ok(TimelineSelection::Range {
                start: range.start()?.date(),
                end:   range.end()?.date(),
            }),
        }
    }
}

/// The filters element a pivot table of a timeline holds for the timeline
/// field, for inspection: the `type` of the date filter and its `val`s.
#[must_use]
pub fn pivot_date_filter(
    table: &crate::structs::PivotTable,
    field: u32,
) -> Option<(String, Vec<String>)> {
    let filters = table.pivot_table_definition().filters()?;
    let children = crate::xml::direct_children(filters).ok()?;
    let filter = children.into_iter().find(|child| {
        child.name.is(SHEET_MAIN_NS, "filter")
            && required_unsigned(child, "fld").ok() == Some(field)
    })?;
    let kind = attr(&filter, "type")?.to_owned();
    let mut values = Vec::new();
    collect_values(&filter, &mut values);
    Some((kind, values))
}

fn collect_values(node: &OpaqueElement, values: &mut Vec<String>) {
    let Ok(children) = crate::xml::direct_children(node) else {
        return;
    };
    for child in children {
        if child.name.local_name == "customFilter" {
            if let Some(value) = attr(&child, "val") {
                values.push(value.to_owned());
            }
        } else {
            collect_values(&child, values);
        }
    }
}
