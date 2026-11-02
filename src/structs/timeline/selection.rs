use std::{
    collections::HashSet,
    io::Cursor,
};

use chrono::{
    Datelike,
    NaiveDate,
};
use quick_xml::Writer;

use super::model::{
    Timeline,
    TimelineCache,
    TimelineLevel,
    TimelineRange,
    TimelineSelection,
    date_time_text,
};
use crate::{
    drawing::charts::{
        OpaqueElement,
        OpaqueToken,
    },
    helper::const_str::SHEET_MAIN_NS,
    structs::{
        SLICER_2013_NS,
        SlicerPivotTable,
        SlicerPivotTarget,
    },
    xml::{
        attr,
        required_unsigned,
        root,
    },
};

const PIVOT_FILTER_URI: &str = "{0605FD5F-26C8-4aeb-8148-2DB25E43C511}";

fn binding_error(cache: &str, message: impl std::fmt::Display) -> String {
    format!("unresolved timeline binding: timeline cache '{cache}' {message}")
}

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

    /// The coarsest time level the range aligns to: whole years, whole
    /// quarters, whole months, else days (what Excel stores as the
    /// `selectionLevel` of the views).
    #[must_use]
    pub fn selection_level_of(selection: TimelineSelection) -> TimelineLevel {
        let TimelineSelection::Range { start, end } = selection else {
            return TimelineLevel::Months;
        };
        let next_day = end.succ_opt();
        let starts_month = start.day() == 1;
        let ends_month = next_day.is_some_and(|day| day.day() == 1);
        if !(starts_month && ends_month) {
            return TimelineLevel::Days;
        }
        let aligned = |step: u32| {
            (start.month() - 1).is_multiple_of(step)
                && next_day.is_some_and(|day| (day.month() - 1).is_multiple_of(step))
        };
        if aligned(12) {
            TimelineLevel::Years
        } else if aligned(3) {
            TimelineLevel::Quarters
        } else {
            TimelineLevel::Months
        }
    }
}

/// Excel's date serial of `date`.
fn serial(date: NaiveDate, date1904: bool) -> Result<i64, String> {
    let (base, earliest) = if date1904 {
        (
            NaiveDate::from_ymd_opt(1904, 1, 1),
            NaiveDate::from_ymd_opt(1904, 1, 1),
        )
    } else {
        (
            NaiveDate::from_ymd_opt(1899, 12, 30),
            NaiveDate::from_ymd_opt(1900, 3, 1),
        )
    };
    let (base, earliest) = (
        base.ok_or("invalid date base")?,
        earliest.ok_or("invalid earliest date")?,
    );
    if date < earliest {
        return Err(format!(
            "{date} is before the first date a timeline can select"
        ));
    }
    Ok((date - base).num_days())
}

/// The date of the Excel serial `value`.
fn date_of_serial(value: &str, date1904: bool) -> Result<NaiveDate, String> {
    let days: i64 = value
        .parse()
        .map_err(|_| format!("'{value}' is not a date serial"))?;
    let base = if date1904 {
        NaiveDate::from_ymd_opt(1904, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(1899, 12, 30)
    }
    .ok_or("invalid date base")?;
    base.checked_add_signed(chrono::Duration::days(days))
        .ok_or_else(|| format!("date serial {days} is out of range"))
}

/// The range a pivot date filter selects, as the timeline over its field
/// takes it over.
pub(crate) fn range_of_date_filter(
    kind: &str,
    values: &[String],
    date1904: bool,
) -> Result<TimelineSelection, String> {
    match (kind, values) {
        ("dateEqual", [day]) => {
            let day = date_of_serial(day, date1904)?;
            Ok(TimelineSelection::Range {
                start: day,
                end:   day,
            })
        }
        ("dateBetween", [start, end]) => Ok(TimelineSelection::Range {
            start: date_of_serial(start, date1904)?,
            end:   date_of_serial(end, date1904)?,
        }),
        _ => Err(format!(
            "the pivot field holds a {kind} filter, which a timeline cannot take over"
        )),
    }
}

/// The text of the pivot `filter` Excel writes for a timeline selection.
fn filter_xml(
    selection: TimelineSelection,
    field: u32,
    name: &str,
    id: u32,
    date1904: bool,
) -> Result<Option<(String, &'static str)>, String> {
    let TimelineSelection::Range { start, end } = selection else {
        return Ok(None);
    };
    let (filter_type, custom) = if start == end {
        (
            "dateEqual",
            format!(r#"<customFilter val="{}"/>"#, serial(start, date1904)?),
        )
    } else {
        (
            "dateBetween",
            format!(
                r#"<customFilter operator="greaterThanOrEqual" val="{}"/><customFilter operator="lessThanOrEqual" val="{}"/>"#,
                serial(start, date1904)?,
                serial(end, date1904)?
            ),
        )
    };
    let text = format!(
        r#"<filter fld="{field}" type="{filter_type}" evalOrder="-1" id="{id}" name="{}"><autoFilter ref="A1"><filterColumn colId="0"><customFilters{}>{custom}</customFilters></filterColumn></autoFilter><extLst><ext uri="{PIVOT_FILTER_URI}" xmlns:x15="{SLICER_2013_NS}"><x15:pivotFilter useWholeDay="1"/></ext></extLst></filter>"#,
        quick_xml::escape::escape(crate::xml::encode_xstring(name)),
        if start == end { "" } else { r#" and="1""# },
    );
    Ok(Some((text, filter_type)))
}

/// The start and end token index (inclusive) of each element directly inside
/// `tokens`.
fn top_level_ranges(tokens: &[OpaqueToken]) -> Result<Vec<(usize, usize)>, String> {
    let mut ranges = Vec::new();
    let mut depth = 0usize;
    let mut first = 0;
    for (index, token) in tokens.iter().enumerate() {
        match token {
            OpaqueToken::Start(_) => {
                if depth == 0 {
                    first = index;
                }
                depth += 1;
            }
            OpaqueToken::End => {
                depth = depth.checked_sub(1).ok_or("unbalanced pivot filters")?;
                if depth == 0 {
                    ranges.push((first, index));
                }
            }
            _ => (),
        }
    }
    if depth != 0 {
        return Err("unbalanced pivot filters".into());
    }
    Ok(ranges)
}

fn filter_start(
    tokens: &[OpaqueToken],
    index: usize,
) -> Option<&crate::drawing::charts::OpaqueStart> {
    match tokens.get(index) {
        Some(OpaqueToken::Start(start)) => Some(start),
        _ => None,
    }
}

fn start_attribute<'a>(
    start: &'a crate::drawing::charts::OpaqueStart,
    name: &str,
) -> Option<&'a str> {
    start
        .attributes
        .iter()
        .find(|(key, _)| key.is("", name))
        .map(|(_, value)| value.as_str())
}

/// The pivot `filters` element with the date filter of `field` replaced by
/// `replacement` (or removed). A filter of the field that is not a date filter
/// is an error, and so is a malformed `filters` element. Returns the new
/// element (`None` when no filter is left) and the filter that was replaced.
pub(crate) fn replace_date_filter(
    existing: Option<&OpaqueElement>,
    field: u32,
    field_name: &str,
    selection: TimelineSelection,
    date1904: bool,
) -> Result<Option<OpaqueElement>, String> {
    let tokens: &[OpaqueToken] = existing.map_or(&[], |element| element.children.as_slice());
    let ranges = top_level_ranges(tokens)?;
    let mut kept: Vec<OpaqueToken> = Vec::new();
    let mut replaced_at = None;
    let mut used_ids = HashSet::new();
    let mut count = 0u32;
    for (first, last) in &ranges {
        let start = filter_start(tokens, *first).ok_or("malformed pivot filters")?;
        if !start.name.is(SHEET_MAIN_NS, "filter") {
            return Err("pivot filters hold an unexpected element".into());
        }
        let own_field = start_attribute(start, "fld")
            .and_then(|value| value.parse::<u32>().ok())
            .ok_or("pivot filter lacks its field")?;
        let id = start_attribute(start, "id").and_then(|value| value.parse::<u32>().ok());
        if own_field == field {
            if !start_attribute(start, "type").is_some_and(|kind| kind.starts_with("date")) {
                return Err(format!(
                    "pivot field '{field_name}' has a filter that is not a date filter"
                ));
            }
            if replaced_at.is_some() {
                return Err(format!("pivot field '{field_name}' has two filters"));
            }
            replaced_at = Some((kept.len(), id));
            continue;
        }
        used_ids.extend(id);
        kept.extend_from_slice(&tokens[*first..=*last]);
        count += 1;
    }
    let reused_id = replaced_at.and_then(|(_, id)| id);
    let id = reused_id.unwrap_or_else(|| used_ids.iter().max().map_or(2, |max| max + 1));
    let new_tokens = match filter_xml(selection, field, field_name, id, date1904)? {
        None => Vec::new(),
        Some((text, _)) => {
            let wrapper = format!(r#"<filters xmlns="{SHEET_MAIN_NS}">{text}</filters>"#);
            root(wrapper.as_bytes())?.children
        }
    };
    if !new_tokens.is_empty() {
        count += 1;
        // A filter added for a field goes first, as Excel writes it; a
        // replaced one keeps its place.
        let at = replaced_at.map_or(0, |(at, _)| at);
        kept.splice(at..at, new_tokens);
    }
    if count == 0 {
        return Ok(None);
    }
    let mut element = match existing {
        Some(element) => element.clone(),
        None => root(format!(r#"<filters xmlns="{SHEET_MAIN_NS}"/>"#).as_bytes())?,
    };
    element.children = kept;
    if let Some((_, value)) = element
        .attributes
        .iter_mut()
        .find(|(key, _)| key.is("", "count"))
    {
        *value = count.to_string();
    } else {
        element.attributes.push((
            crate::drawing::charts::QName {
                namespace_uri:    String::new(),
                local_name:       "count".into(),
                preferred_prefix: None,
            },
            count.to_string(),
        ));
    }
    Ok(Some(element))
}

fn element_bytes(element: Option<&OpaqueElement>) -> Result<Option<Vec<u8>>, String> {
    let Some(element) = element else {
        return Ok(None);
    };
    let mut writer = Writer::new(Cursor::new(Vec::new()));
    element.write_to(
        &mut writer,
        &crate::drawing::charts::NamespaceScope::with_bindings([(None, SHEET_MAIN_NS.to_owned())]),
    )?;
    Ok(Some(writer.into_inner().into_inner()))
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

#[derive(Clone, Debug)]
struct PlannedFilters {
    sheet_id: u32,
    name:     String,
    filters:  Option<OpaqueElement>,
}

type Guard = (u32, String, u32, Option<Vec<u8>>);

/// A validated change of a timeline selection: every linked pivot table can
/// take the date filter, and nothing was changed yet.
#[derive(Clone, Debug)]
pub struct TimelineSelectionPlan {
    selection: TimelineSelection,
    date1904:  bool,
    cache_xml: Vec<u8>,
    pivots:    Vec<PlannedFilters>,
    guards:    Vec<Guard>,
}

impl TimelineCache {
    /// Check that `selection` is a legal selection of this timeline and that
    /// every linked pivot table can take its date filter.
    ///
    /// # Errors
    /// Returns an error naming the timeline cache when a binding does not
    /// resolve, the range ends before it starts, or a pivot field holds a
    /// filter that is not a date filter. A range outside the dates of the data
    /// is accepted, as Excel accepts it; the bounds do not change.
    pub fn plan_selection(
        &self,
        selection: TimelineSelection,
        targets: &[SlicerPivotTarget<'_>],
        date1904: bool,
    ) -> Result<TimelineSelectionPlan, String> {
        self.validate()?;
        if let TimelineSelection::Range { start, end } = selection {
            if start > end {
                return Err(format!(
                    "timeline cache '{}' cannot select a range that ends before it starts",
                    self.name
                ));
            }
        }
        let mut pivots = Vec::new();
        let mut guards = Vec::new();
        for link in self.linked_pivots() {
            let matching: Vec<_> = targets
                .iter()
                .filter(|target| {
                    target.sheet_id == link.sheet_id
                        && target.table.pivot_table_definition().name() == link.name
                })
                .collect();
            if matching.len() != 1 {
                return Err(binding_error(
                    &self.name,
                    format!(
                        "has a missing or ambiguous pivot table '{}' on sheet id {}",
                        link.name, link.sheet_id
                    ),
                ));
            }
            let table = &*matching[0].table;
            let cache = table.pivot_cache_definition();
            if cache.slicer_cache_id() != Some(self.state.pivot_cache_id) {
                return Err(binding_error(
                    &self.name,
                    format!(
                        "names pivot cache id {}, which is not the pivot cache of table '{}'",
                        self.state.pivot_cache_id, link.name
                    ),
                ));
            }
            let fields: Vec<_> = cache
                .cache_fields()
                .list()
                .iter()
                .enumerate()
                .filter(|(_, field)| field.name() == self.source_name)
                .collect();
            if fields.len() != 1 {
                return Err(binding_error(
                    &self.name,
                    format!(
                        "has a missing or ambiguous source field '{}' in table '{}'",
                        self.source_name, link.name
                    ),
                ));
            }
            let index = u32::try_from(fields[0].0)
                .map_err(|_| "pivot source field index out of range".to_owned())?;
            if table
                .pivot_table_definition()
                .pivot_fields()
                .list()
                .get(fields[0].0)
                .is_none()
            {
                return Err(binding_error(
                    &self.name,
                    format!(
                        "has no pivot field for '{}' in table '{}'",
                        self.source_name, link.name
                    ),
                ));
            }
            let existing = table.pivot_table_definition().filters();
            guards.push((
                link.sheet_id,
                link.name.clone(),
                index,
                element_bytes(existing)?,
            ));
            let filters =
                replace_date_filter(existing, index, &self.source_name, selection, date1904)
                    .map_err(|error| {
                        binding_error(&self.name, format!("table '{}': {error}", link.name))
                    })?;
            if let Some(filters) = &filters {
                element_bytes(Some(filters))?;
            }
            pivots.push(PlannedFilters {
                sheet_id: link.sheet_id,
                name: link.name.clone(),
                filters,
            });
        }
        Ok(TimelineSelectionPlan {
            selection,
            date1904,
            cache_xml: self.to_xml()?,
            pivots,
            guards,
        })
    }
}

impl TimelineSelectionPlan {
    /// Apply the selection to the cache and to every linked pivot table.
    ///
    /// # Errors
    /// Returns an error, with nothing changed, when the cache or a pivot
    /// table changed since the plan was made.
    pub fn apply(
        self,
        cache: &mut TimelineCache,
        targets: &mut [SlicerPivotTarget<'_>],
    ) -> Result<(), String> {
        if cache.to_xml()? != self.cache_xml {
            return Err("timeline cache changed after selection validation".into());
        }
        let fresh = cache.plan_selection(self.selection, targets, self.date1904)?;
        if fresh.guards != self.guards {
            return Err("pivot filters changed after selection validation".into());
        }
        // Planning above validated every binding before the first mutation.
        for planned in fresh.pivots {
            let target = targets
                .iter_mut()
                .find(|target| {
                    target.sheet_id == planned.sheet_id
                        && target.table.pivot_table_definition().name() == planned.name
                })
                .expect("validated pivot target");
            target
                .table
                .pivot_table_definition_mut()
                .set_filters(planned.filters);
        }
        cache.set_selected_state(self.selection);
        Ok(())
    }
}

impl TimelineCache {
    /// Record `selection` in the cache state the way Excel does.
    pub(crate) fn set_selected_state(&mut self, selection: TimelineSelection) {
        match selection {
            TimelineSelection::All => {
                self.state.selection = None;
                "unknown".clone_into(&mut self.state.filter_type);
            }
            TimelineSelection::Range { start, end } => {
                self.state.selection = Some(TimelineRange::from_dates(start, end));
                (if start == end {
                    "dateEqual"
                } else {
                    "dateBetween"
                })
                .clone_into(&mut self.state.filter_type);
            }
        }
    }
}

/// The bounds Excel stores for the dates of a field: January 1 of the year of
/// the first date to January 1 of the year after the last one.
pub(crate) fn bounds_of(first: NaiveDate, last: NaiveDate) -> Result<TimelineRange, String> {
    let start = NaiveDate::from_ymd_opt(first.year(), 1, 1).ok_or("date out of range")?;
    let end = NaiveDate::from_ymd_opt(last.year() + 1, 1, 1).ok_or("date out of range")?;
    Ok(TimelineRange {
        start_date: date_time_text(start),
        end_date:   date_time_text(end),
    })
}
