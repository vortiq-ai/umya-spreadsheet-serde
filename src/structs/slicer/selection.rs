use std::collections::{
    HashMap,
    HashSet,
};

use super::model::{
    OlapSlicerSelection,
    Slicer,
    SlicerCache,
    SlicerCacheExtension,
    SlicerItemKey,
    SlicerPivotTable,
    SlicerSelection,
    SlicerSortOrder,
    SlicerSource,
    SlicerTableValue,
};
use crate::structs::{
    Item,
    PivotField,
    PivotTable,
};

pub struct SlicerPivotTarget<'a> {
    pub sheet_id: u32,
    pub table:    &'a mut PivotTable,
}

#[derive(Clone, Debug)]
pub struct SlicerSelectionPlan {
    selection: SlicerSelection,
    cache_xml: Vec<u8>,
    pivots:    Vec<(u32, String, usize, PivotField)>,
    guards:    Vec<(u32, String, usize, u32, Vec<u8>)>,
}

impl Slicer {
    pub fn validate(&self) -> Result<(), String> {
        if self.name.is_empty() || self.cache.is_empty() {
            return Err("slicer name and cache are required".into());
        }
        if self.column_count == Some(0) {
            return Err("slicer column count must be positive".into());
        }
        Ok(())
    }
}

impl SlicerCache {
    pub fn validate(&self) -> Result<(), String> {
        if self.name.is_empty() || self.source_name.is_empty() {
            return Err("slicer cache name and source are required".into());
        }
        let mut pivots = HashSet::new();
        for pivot in self.linked_pivots() {
            if pivot.name.is_empty() || !pivots.insert((pivot.sheet_id, &pivot.name)) {
                return Err("duplicate or unnamed slicer pivot binding".into());
            }
        }
        match &self.source {
            Some(SlicerSource::TabularPivot(source)) => {
                if source.sort_order == Some(SlicerSortOrder::Natural) {
                    return Err("natural sort requires an OLAP source".into());
                }
                if let Some(items) = &source.items {
                    if items.items.is_empty() {
                        return Err("tabular cached items must not be empty".into());
                    }
                    if items
                        .count
                        .is_some_and(|count| count as usize != items.items.len())
                    {
                        return Err("slicer item count mismatch".into());
                    }
                    let mut indexes = HashSet::new();
                    if items.items.iter().any(|item| !indexes.insert(item.index)) {
                        return Err("duplicate slicer item index".into());
                    }
                }
            }
            Some(SlicerSource::Olap(source)) => {
                if source.levels.levels.is_empty() || source.selections.selections.is_empty() {
                    return Err("OLAP levels and selections must not be empty".into());
                }
                let mut level_names = HashSet::new();
                for level in &source.levels.levels {
                    if level.unique_name.is_empty() || !level_names.insert(&level.unique_name) {
                        return Err("duplicate or empty OLAP level identity".into());
                    }
                    let mut indexes = HashSet::new();
                    let mut members = HashSet::new();
                    for range in level.ranges.iter().flatten() {
                        if range.items.is_empty() {
                            return Err("OLAP cache range must contain items".into());
                        }
                        for (offset, item) in range.items.iter().enumerate() {
                            let index = range
                                .start_item
                                .checked_add(
                                    u32::try_from(offset)
                                        .map_err(|_| "OLAP range exceeds unsigned index space")?,
                                )
                                .ok_or("OLAP range index overflow")?;
                            if index >= level.count || !indexes.insert(index) {
                                return Err("overlapping or out-of-range OLAP cache indexes".into());
                            }
                            if item.unique_name.is_empty()
                                || !members.insert((&item.unique_name, &item.parents))
                            {
                                return Err("duplicate or empty OLAP cached member identity".into());
                            }
                        }
                    }
                }
                if source
                    .levels
                    .count
                    .is_some_and(|count| count as usize != source.levels.levels.len())
                    || source
                        .selections
                        .count
                        .is_some_and(|count| count as usize != source.selections.selections.len())
                {
                    return Err("OLAP slicer count mismatch".into());
                }
                let mut keys = HashSet::new();
                for item in &source.selections.selections {
                    if item.unique_name.is_empty()
                        || !keys.insert((&item.unique_name, &item.parents))
                    {
                        return Err("duplicate or empty OLAP selection identity".into());
                    }
                }
            }
            Some(SlicerSource::Table(source)) => {
                if source.sort_order == Some(SlicerSortOrder::Natural) {
                    return Err("natural sort requires an OLAP source".into());
                }
            }
            None => {}
        }
        let markers = self
            .extensions
            .iter()
            .filter(|entry| matches!(entry, SlicerCacheExtension::TableSource { .. }))
            .count();
        if markers > 1 || markers > 0 && !matches!(self.source, Some(SlicerSource::Table(_))) {
            return Err("invalid table slicer source ownership".into());
        }
        Ok(())
    }

    #[must_use]
    pub fn linked_pivots(&self) -> Vec<&SlicerPivotTable> {
        self.pivot_tables
            .iter()
            .flatten()
            .chain(
                self.extensions
                    .iter()
                    .filter_map(|entry| match entry {
                        SlicerCacheExtension::PivotTables { tables, .. } => Some(tables),
                        _ => None,
                    })
                    .flatten(),
            )
            .collect()
    }

    pub fn selection(&self) -> Result<SlicerSelection, String> {
        self.validate()?;
        match &self.source {
            Some(SlicerSource::TabularPivot(source)) => {
                let items = source.items.as_ref().ok_or("slicer has no cached items")?;
                if items
                    .items
                    .iter()
                    .all(|item| item.selected.unwrap_or(false))
                {
                    return Ok(SlicerSelection::All);
                }
                Ok(SlicerSelection::Items(
                    items
                        .items
                        .iter()
                        .filter(|item| item.selected.unwrap_or(false))
                        .map(|item| SlicerItemKey::PivotItem(item.index))
                        .collect(),
                ))
            }
            Some(SlicerSource::Olap(source)) => Ok(SlicerSelection::Items(
                source
                    .selections
                    .selections
                    .iter()
                    .map(|item| SlicerItemKey::OlapMember {
                        unique_name: item.unique_name.clone(),
                        parents:     item.parents.clone(),
                    })
                    .collect(),
            )),
            Some(SlicerSource::Table(_)) => {
                Err("table slicer selection requires the bound table filter".into())
            }
            None => Err("slicer cache has no source".into()),
        }
    }

    pub fn plan_selection(
        &self,
        selection: SlicerSelection,
        targets: &[SlicerPivotTarget<'_>],
    ) -> Result<SlicerSelectionPlan, String> {
        self.validate()?;
        let Some(SlicerSource::TabularPivot(source)) = &self.source else {
            return Err("tabular pivot selection requires a tabular pivot source".into());
        };
        let selected = match &selection {
            SlicerSelection::All => None,
            SlicerSelection::Items(keys) => {
                if keys.is_empty() {
                    return Err("slicer selection must contain at least one item".into());
                }
                let available: HashSet<_> = source
                    .items
                    .as_ref()
                    .ok_or("slicer has no cached items")?
                    .items
                    .iter()
                    .map(|item| item.index)
                    .collect();
                let mut selected = HashSet::new();
                for key in keys {
                    match key {
                        SlicerItemKey::PivotItem(index)
                            if available.contains(index) && selected.insert(*index) => {}
                        _ => return Err("invalid or duplicate slicer item identity".into()),
                    }
                }
                Some(selected)
            }
        };
        let mut prepared = Vec::new();
        let mut guards = Vec::new();
        let links = self.linked_pivots();
        if links.is_empty() {
            return Err("slicer has no linked pivot tables".into());
        }
        for link in links {
            let matching: Vec<_> = targets
                .iter()
                .filter(|target| {
                    target.sheet_id == link.sheet_id
                        && target.table.pivot_table_definition().name() == link.name
                })
                .collect();
            if matching.len() != 1 {
                return Err(format!(
                    "missing or ambiguous pivot binding {} on sheet {}",
                    link.name, link.sheet_id
                ));
            }
            let table = &*matching[0].table;
            let cache = table.pivot_cache_definition();
            if cache.slicer_cache_id() != Some(source.pivot_cache_id) {
                return Err("slicer and pivot cache identities differ".into());
            }
            let fields: Vec<_> = cache
                .cache_fields()
                .list()
                .iter()
                .enumerate()
                .filter(|(_, field)| field.name() == self.source_name)
                .collect();
            if fields.len() != 1 {
                return Err("missing or ambiguous slicer source field".into());
            }
            let (index, cache_field) = fields[0];
            let item_count = cache_field.items().len();
            let mut source_items = cache_field.shared_items().clone();
            *source_items.items_mut() = cache_field.items().to_vec();
            guards.push((
                link.sheet_id,
                link.name.clone(),
                index,
                table.pivot_table_definition().cache_id(),
                source_items.to_xml()?,
            ));
            if source.items.as_ref().is_some_and(|items| {
                items
                    .items
                    .iter()
                    .any(|item| item.index as usize >= item_count)
            }) {
                return Err("slicer item is outside its source field".into());
            }
            let mut field = table
                .pivot_table_definition()
                .pivot_fields()
                .list()
                .get(index)
                .ok_or("pivot source field is missing")?
                .clone();
            if field.items().list().is_empty() && selected.is_some() {
                for item_index in 0..item_count {
                    let mut item = Item::default();
                    item.set_index(
                        u32::try_from(item_index)
                            .map_err(|_| "pivot source item index out of range")?,
                    );
                    field.items_mut().list_mut().push(item);
                }
            }
            for item in field.items_mut().list_mut() {
                if !item.is_data_item() {
                    continue;
                }
                if !item.has_index() || item.index() as usize >= item_count {
                    return Err("invalid pivot source item identity".into());
                }
                item.set_hidden(Some(
                    selected
                        .as_ref()
                        .is_some_and(|indexes| !indexes.contains(&item.index())),
                ));
            }
            field.properties_mut().multiple_item_selection_allowed = Some(selected.is_some());
            prepared.push((link.sheet_id, link.name.clone(), index, field));
        }
        Ok(SlicerSelectionPlan {
            selection,
            cache_xml: self.to_xml()?,
            pivots: prepared,
            guards,
        })
    }
}

impl SlicerSelectionPlan {
    pub fn apply(
        self,
        cache: &mut SlicerCache,
        targets: &mut [SlicerPivotTarget<'_>],
    ) -> Result<(), String> {
        if cache.to_xml()? != self.cache_xml {
            return Err("slicer cache changed after selection validation".into());
        }
        let fresh = cache.plan_selection(self.selection.clone(), targets)?;
        if fresh.guards != self.guards {
            return Err("native pivot source identities changed after selection validation".into());
        }
        // Preparation above validates every binding before the first mutation.
        for (sheet_id, name, index, field) in fresh.pivots {
            let target = targets
                .iter_mut()
                .find(|target| {
                    target.sheet_id == sheet_id
                        && target.table.pivot_table_definition().name() == name
                })
                .expect("validated pivot target");
            target
                .table
                .pivot_table_definition_mut()
                .pivot_fields_mut()
                .list_mut()[index] = field;
        }
        if let Some(SlicerSource::TabularPivot(source)) = &mut cache.source {
            if let Some(items) = &mut source.items {
                for item in &mut items.items {
                    item.selected = Some(match &self.selection {
                        SlicerSelection::All => true,
                        SlicerSelection::Items(keys) => {
                            keys.contains(&SlicerItemKey::PivotItem(item.index))
                        }
                    });
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct SlicerTableSelectionPlan {
    cache_xml:       Vec<u8>,
    table_id:        u32,
    column_id:       u32,
    filter_bindings: Vec<(u32, u32)>,
    rule:            Option<crate::structs::filter_rule::FilterRule>,
}

impl SlicerCache {
    pub fn plan_table_selection(
        &self,
        selection: SlicerSelection,
        table: &crate::structs::Table,
    ) -> Result<SlicerTableSelectionPlan, String> {
        self.validate()?;
        let Some(SlicerSource::Table(source)) = &self.source else {
            return Err("table selection requires a table slicer source".into());
        };
        table_column(table, source.table_id, source.column_id)?;
        let filter_bindings = table_filter_bindings(table)?;
        if table.auto_filter().range().range().is_empty() {
            table_filter_range(table)?;
        }
        let rule = match selection {
            SlicerSelection::All => None,
            SlicerSelection::Items(keys) => {
                if keys.is_empty() {
                    return Err("slicer selection must contain at least one value".into());
                }
                let mut values = Vec::new();
                let mut blank = false;
                let mut identities = HashSet::new();
                let mut date_groups = Vec::new();
                for key in keys {
                    let SlicerItemKey::TableValue(value) = key else {
                        return Err("table slicer requires table value identities".into());
                    };
                    let (identity, lexical) = match value {
                        SlicerTableValue::Blank => ("blank".into(), None),
                        SlicerTableValue::Text(value) => (format!("text:{value}"), Some(value)),
                        SlicerTableValue::Number(value) => {
                            let number = value
                                .parse::<f64>()
                                .map_err(|_| "invalid numeric table value")?;
                            if !number.is_finite() {
                                return Err("numeric table values must be finite".into());
                            }
                            let canonical = if number == 0.0 {
                                "0".into()
                            } else {
                                number.to_string()
                            };
                            (format!("number:{canonical}"), Some(canonical))
                        }
                        SlicerTableValue::Boolean(value) => (
                            format!("boolean:{value}"),
                            Some(if value { "TRUE" } else { "FALSE" }.into()),
                        ),
                        SlicerTableValue::Error(value) => {
                            if !matches!(
                                value.as_str(),
                                "#NULL!"
                                    | "#DIV/0!"
                                    | "#VALUE!"
                                    | "#REF!"
                                    | "#NAME?"
                                    | "#NUM!"
                                    | "#N/A"
                                    | "#GETTING_DATA"
                            ) {
                                return Err("invalid table error value".into());
                            }
                            (format!("error:{value}"), Some(value))
                        }
                        SlicerTableValue::Date(value) => {
                            use chrono::Datelike;
                            let date = chrono::NaiveDate::parse_from_str(&value, "%Y-%m-%d")
                                .map_err(|_| {
                                    "table date values require a valid ISO calendar date"
                                })?;
                            let year = u16::try_from(date.year())
                                .map_err(|_| "table date year out of range")?;
                            date_groups.push(crate::structs::filter_rule::DateGroupFilter {
                                year,
                                month: Some(
                                    u16::try_from(date.month())
                                        .map_err(|_| "table date month out of range")?,
                                ),
                                day: Some(
                                    u16::try_from(date.day())
                                        .map_err(|_| "table date day out of range")?,
                                ),
                                hour: None,
                                minute: None,
                                second: None,
                                grouping: crate::structs::filter_rule::DateTimeGrouping::Day,
                            });
                            let identity = format!("date:{date}");
                            if !identities.insert(identity) {
                                return Err("duplicate table date identity".into());
                            }
                            continue;
                        }
                    };
                    if !identities.insert(identity) {
                        return Err("duplicate table selection identity".into());
                    }
                    if let Some(value) = lexical {
                        if values
                            .iter()
                            .any(|existing| existing == &Some(value.clone()))
                        {
                            return Err("selected table identities have indistinguishable native \
                                        filter values"
                                .into());
                        }
                        values.push(Some(value));
                    } else {
                        blank = true;
                    }
                }
                Some(crate::structs::filter_rule::FilterRule::Values(
                    crate::structs::filter_rule::ValueFilters {
                        blank: Some(blank),
                        calendar_type: None,
                        values,
                        date_groups,
                    },
                ))
            }
        };
        Ok(SlicerTableSelectionPlan {
            cache_xml: self.to_xml()?,
            table_id: source.table_id,
            column_id: source.column_id,
            filter_bindings,
            rule,
        })
    }
}

fn table_column(
    table: &crate::structs::Table,
    table_id: u32,
    column_id: u32,
) -> Result<usize, String> {
    if table.id() != Some(table_id) {
        return Err("table slicer binding points to another table".into());
    }
    let indexes: Vec<_> = table
        .columns()
        .iter()
        .enumerate()
        .filter(|(_, column)| column.id() == Some(column_id))
        .map(|(index, _)| index)
        .collect();
    if indexes.len() != 1 {
        return Err("missing or ambiguous stable table column identity".into());
    }
    let index = indexes[0];
    let mut columns = HashSet::new();
    for filter in table.auto_filter().filter_columns() {
        if filter.column_id() as usize >= table.columns().len()
            || !columns.insert(filter.column_id())
        {
            return Err("invalid table filter column binding".into());
        }
    }
    Ok(index)
}

impl SlicerTableSelectionPlan {
    pub fn apply(
        self,
        cache: &SlicerCache,
        table: &mut crate::structs::Table,
    ) -> Result<(), String> {
        if cache.to_xml()? != self.cache_xml {
            return Err("slicer cache changed after table selection validation".into());
        }
        let index = u32::try_from(table_column(table, self.table_id, self.column_id)?)
            .map_err(|_| "table column index out of range")?;
        if table_filter_bindings(table)? != self.filter_bindings {
            return Err("table filter column identities changed after selection validation".into());
        }
        let range = if table.auto_filter().range().range().is_empty() {
            Some(table_filter_range(table)?)
        } else {
            None
        };
        if let Some(range) = range {
            table.set_auto_filter(range);
        }
        let columns = table.auto_filter_mut().filter_columns_mut();
        if let Some(column) = columns
            .iter_mut()
            .find(|column| column.column_id() == index)
        {
            column.set_rule(self.rule);
        } else if self.rule.is_some() {
            let mut column = crate::structs::FilterColumn::default();
            column.set_column_id(index).set_rule(self.rule);
            columns.push(column);
        }
        Ok(())
    }
}

fn table_filter_bindings(table: &crate::structs::Table) -> Result<Vec<(u32, u32)>, String> {
    table
        .auto_filter()
        .filter_columns()
        .iter()
        .map(|filter| {
            let column = table
                .columns()
                .get(filter.column_id() as usize)
                .ok_or("invalid table filter column binding")?;
            let id = column
                .id()
                .ok_or("table filter column has no stable identity")?;
            if table
                .columns()
                .iter()
                .filter(|column| column.id() == Some(id))
                .count()
                != 1
            {
                return Err("ambiguous stable table filter column identity".into());
            }
            Ok((filter.column_id(), id))
        })
        .collect()
}

fn table_filter_range(table: &crate::structs::Table) -> Result<String, String> {
    if !table.is_ok() {
        return Err("table selection requires a valid table area".into());
    }
    let (start, end) = table.area();
    let end_row = end
        .row_num()
        .checked_sub(table.totals_row_count())
        .filter(|row| *row >= start.row_num())
        .ok_or("table totals leave no header and data range")?;
    let mut end = end.clone();
    end.set_row_num(end_row);
    Ok(format!("{start}:{end}"))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OlapPivotMemberBinding {
    pub member:           OlapSlicerSelection,
    pub pivot_item_index: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OlapPivotBinding {
    pub sheet_id:         u32,
    pub pivot_table_name: String,
    pub pivot_cache_id:   u32,
    pub field_index:      usize,
    pub members:          Vec<OlapPivotMemberBinding>,
    pub all_member:       Option<OlapSlicerSelection>,
}

#[derive(Clone, Debug)]
pub struct OlapSlicerSelectionPlan {
    cache_xml:      Vec<u8>,
    requested:      SlicerSelection,
    bindings:       Vec<OlapPivotBinding>,
    native_sources: Vec<(u32, String, u32, String)>,
}

impl SlicerCache {
    pub fn plan_olap_selection(
        &self,
        requested: SlicerSelection,
        targets: &[SlicerPivotTarget<'_>],
        bindings: &[OlapPivotBinding],
    ) -> Result<OlapSlicerSelectionPlan, String> {
        prepare_olap(self, &requested, targets, bindings)?;
        Ok(OlapSlicerSelectionPlan {
            cache_xml: self.to_xml()?,
            requested,
            bindings: bindings.to_vec(),
            native_sources: olap_source_snapshots(self, targets)?,
        })
    }
}

#[derive(Clone, Debug)]
struct OlapFieldChange {
    sheet_id:    u32,
    table_name:  String,
    field_index: usize,
    field:       PivotField,
}
#[derive(Clone, Debug)]
struct OlapHierarchyChange {
    sheet_id:        u32,
    table_name:      String,
    hierarchy_index: usize,
    hierarchy:       crate::structs::PivotTableHierarchy,
}
struct PreparedOlapSelection {
    members:     Vec<OlapSlicerSelection>,
    fields:      Vec<OlapFieldChange>,
    hierarchies: Vec<OlapHierarchyChange>,
}

fn ancestors(
    name: &str,
    graph: &HashMap<String, Vec<String>>,
    all: Option<&str>,
    visiting: &mut HashSet<String>,
) -> Result<HashSet<String>, String> {
    if !visiting.insert(name.into()) {
        return Err("cyclic cached OLAP parent lineage".into());
    }
    let mut result = HashSet::new();
    for parent in graph
        .get(name)
        .ok_or("missing cached OLAP member lineage")?
    {
        result.insert(parent.clone());
        if Some(parent.as_str()) == all {
            continue;
        }
        if !graph.contains_key(parent) {
            return Err(
                "incomplete OLAP parent context; a verified provider binding is required".into(),
            );
        }
        result.extend(ancestors(parent, graph, all, visiting)?);
    }
    visiting.remove(name);
    Ok(result)
}

fn prepare_olap(
    cache: &SlicerCache,
    requested: &SlicerSelection,
    targets: &[SlicerPivotTarget<'_>],
    bindings: &[OlapPivotBinding],
) -> Result<PreparedOlapSelection, String> {
    cache.validate()?;
    let Some(SlicerSource::Olap(source)) = &cache.source else {
        return Err("OLAP selection requires an OLAP source".into());
    };
    let cached: HashSet<_> = source
        .levels
        .levels
        .iter()
        .flat_map(|level| level.ranges.iter().flatten())
        .flat_map(|range| &range.items)
        .map(|item| (&item.unique_name, &item.parents))
        .collect();
    let selected = match requested {
        SlicerSelection::All => None,
        SlicerSelection::Items(keys) => {
            if keys.is_empty() {
                return Err("OLAP selection requires at least one member".into());
            }
            let mut result = Vec::new();
            let mut seen = HashSet::new();
            for key in keys {
                let SlicerItemKey::OlapMember {
                    unique_name,
                    parents,
                } = key
                else {
                    return Err("OLAP selection requires member identities".into());
                };
                if !cached.contains(&(unique_name, parents)) || !seen.insert((unique_name, parents))
                {
                    return Err("unknown or duplicate OLAP member identity".into());
                }
                result.push(OlapSlicerSelection {
                    unique_name: unique_name.clone(),
                    parents:     parents.clone(),
                });
            }
            Some(result)
        }
    };
    let links = cache.linked_pivots();
    if links.is_empty() {
        return Err("OLAP slicer has no linked pivots".into());
    }
    let mut cache_members = None;
    let mut changes = PreparedOlapSelection {
        members:     Vec::new(),
        fields:      Vec::new(),
        hierarchies: Vec::new(),
    };
    let mut binding_count = 0;
    for link in links {
        let matches: Vec<_> = targets
            .iter()
            .filter(|target| {
                target.sheet_id == link.sheet_id
                    && target.table.pivot_table_definition().name() == link.name
            })
            .collect();
        if matches.len() != 1 {
            return Err("missing or ambiguous OLAP target".into());
        }
        let table = &*matches[0].table;
        let native_cache = table.pivot_cache_definition();
        if native_cache.slicer_cache_id() != Some(source.pivot_cache_id) {
            return Err("OLAP cache namespace identity mismatch".into());
        }
        let native_hierarchies = native_cache
            .hierarchies()
            .ok_or("OLAP cache has no native hierarchy context")?;
        let matching: Vec<_> = native_hierarchies
            .hierarchies
            .iter()
            .enumerate()
            .filter(|(_, hierarchy)| hierarchy.unique_name == cache.source_name)
            .collect();
        if matching.len() != 1 {
            return Err("missing or ambiguous native hierarchy identity".into());
        }
        let (hierarchy_index, hierarchy) = matching[0];
        let mut view = table
            .pivot_table_definition()
            .hierarchies()
            .and_then(|hierarchies| hierarchies.hierarchies.get(hierarchy_index))
            .ok_or("OLAP view has no bound native pivot hierarchy")?
            .clone();
        let fields: Vec<_> = bindings
            .iter()
            .filter(|binding| {
                binding.sheet_id == link.sheet_id && binding.pivot_table_name == link.name
            })
            .collect();
        if fields.is_empty() {
            return Err("missing explicit native/provider OLAP field bindings".into());
        }
        binding_count += fields.len();
        let all_name = hierarchy
            .all_unique_name
            .clone()
            .filter(|name| !name.is_empty());
        let mut field_indexes = HashSet::new();
        let mut bound_levels = HashSet::new();
        let mut graph = HashMap::new();
        let mut selected_fields = HashSet::new();
        for binding in &fields {
            if binding.pivot_cache_id != source.pivot_cache_id
                || !field_indexes.insert(binding.field_index)
            {
                return Err("duplicate or mismatched OLAP field binding".into());
            }
            let field = native_cache
                .cache_fields()
                .list()
                .get(binding.field_index)
                .ok_or("OLAP source field is missing")?;
            let level = field.properties().level.unwrap_or(0) as usize;
            if field.properties().hierarchy.unwrap_or(0)
                != i32::try_from(hierarchy_index)
                    .map_err(|_| "hierarchy index exceeds signed range")?
                || field.is_ignored()
                || field.properties().member_property_field == Some(true)
            {
                return Err("OLAP field does not belong to the selected native hierarchy".into());
            }
            if let Some(usage) = &hierarchy.fields_usage {
                if usage.fields.get(level).copied() != i32::try_from(binding.field_index).ok() {
                    return Err("OLAP field usage ordinal/back-reference mismatch".into());
                }
            }
            let cached_level = source
                .levels
                .levels
                .iter()
                .find(|cached_level| cached_level.unique_name == field.name())
                .ok_or("OLAP binding does not identify a cached level")?;
            if !bound_levels.insert(field.name()) {
                return Err("OLAP binding does not identify a unique cached level".into());
            }
            if let Some(all) = &binding.all_member {
                if all.unique_name.is_empty()
                    || !all.parents.is_empty()
                    || all_name.as_ref() != Some(&all.unique_name)
                {
                    return Err(
                        "OLAP all-member context contradicts native hierarchy identity".into(),
                    );
                }
            }
            let native_field = table
                .pivot_table_definition()
                .pivot_fields()
                .list()
                .get(binding.field_index)
                .ok_or("OLAP pivot field is missing")?;
            let mut indexes = HashSet::new();
            for item in native_field.items().list() {
                if item.is_data_item() && (!item.has_index() || !indexes.insert(item.index())) {
                    return Err("invalid native OLAP pivot-item identity".into());
                }
            }
            let mut mapped = HashSet::new();
            let mut member_keys = HashSet::new();
            for member in &binding.members {
                if member.member.unique_name.is_empty()
                    || !mapped.insert(member.pivot_item_index)
                    || !indexes.contains(&member.pivot_item_index)
                    || !member_keys.insert((&member.member.unique_name, &member.member.parents))
                {
                    return Err("ambiguous or nonexistent OLAP member-to-item identity".into());
                }
                if let Some(names) = field.cached_unique_names() {
                    if names
                        .iter()
                        .find(|name| name.index == member.pivot_item_index)
                        .is_some_and(|name| name.unique_name != member.member.unique_name)
                    {
                        return Err("provider OLAP member contradicts the native cached unique \
                                    name"
                            .into());
                    }
                }
                let occurrences: Vec<_> = cached_level
                    .ranges
                    .iter()
                    .flatten()
                    .flat_map(|range| &range.items)
                    .filter(|item| item.unique_name == member.member.unique_name)
                    .collect();
                if !occurrences.is_empty()
                    && !occurrences
                        .iter()
                        .any(|item| item.parents == member.member.parents)
                {
                    return Err("provider OLAP parents contradict cached member ancestry".into());
                }
                if let Some(previous) = graph.insert(
                    member.member.unique_name.clone(),
                    member.member.parents.clone(),
                ) {
                    if previous != member.member.parents {
                        return Err("ambiguous OLAP parent occurrence identity".into());
                    }
                }
                if selected
                    .as_ref()
                    .is_some_and(|members| members.contains(&member.member))
                {
                    selected_fields.insert(binding.field_index);
                }
            }
        }
        if bound_levels.len() != source.levels.levels.len() {
            return Err("OLAP binding context omits cached hierarchy levels".into());
        }
        if let Some(members) = &selected {
            for member in members {
                if fields
                    .iter()
                    .flat_map(|binding| &binding.members)
                    .filter(|entry| entry.member == *member)
                    .count()
                    != 1
                {
                    return Err("selected OLAP occurrence has no unique native item binding".into());
                }
            }
        }
        let members = if let Some(members) = &selected {
            members.clone()
        } else {
            vec![OlapSlicerSelection {
                unique_name: all_name
                    .clone()
                    .ok_or("clearing an OLAP slicer requires a native all-member identity")?,
                parents:     Vec::new(),
            }]
        };
        if cache_members
            .as_ref()
            .is_some_and(|previous| previous != &members)
        {
            return Err("linked OLAP pivots disagree on hierarchy selection identity".into());
        }
        cache_members = Some(members);
        let mixed = selected_fields.len() > 1;
        let mut lineage = HashMap::new();
        if mixed {
            for name in graph.keys() {
                lineage.insert(
                    name.clone(),
                    ancestors(name, &graph, all_name.as_deref(), &mut HashSet::new())?,
                );
            }
        }
        view.members.clear();
        view.include_new_items_in_filter = Some(selected.is_some());
        view.multiple_item_selection_allowed = Some(selected.is_some());
        for binding in fields {
            let native_field = &native_cache.cache_fields().list()[binding.field_index];
            let mut field =
                table.pivot_table_definition().pivot_fields().list()[binding.field_index].clone();
            let mut allowed = HashSet::new();
            if let Some(selected) = &selected {
                for candidate in &binding.members {
                    let related = selected.iter().any(|member| {
                        member == &candidate.member
                            || (mixed
                                && (lineage
                                    .get(&candidate.member.unique_name)
                                    .is_some_and(|parents| parents.contains(&member.unique_name))
                                    || lineage.get(&member.unique_name).is_some_and(|parents| {
                                        parents.contains(&candidate.member.unique_name)
                                    })))
                    });
                    if related {
                        allowed.insert(candidate.pivot_item_index);
                    }
                }
            }
            let restricted =
                selected.is_some() && (mixed || selected_fields.contains(&binding.field_index));
            if mixed {
                for item in field.items().list() {
                    if item.is_data_item()
                        && !binding
                            .members
                            .iter()
                            .any(|member| member.pivot_item_index == item.index())
                    {
                        return Err(
                            "partial OLAP cache needs complete verified-provider parent bindings \
                             for mixed-level selection"
                                .into(),
                        );
                    }
                }
            }
            for item in field.items_mut().list_mut() {
                if item.is_data_item() {
                    item.set_hidden(Some(restricted && !allowed.contains(&item.index())));
                }
            }
            field.properties_mut().multiple_item_selection_allowed = Some(restricted);
            if restricted {
                let names: Vec<_> = binding
                    .members
                    .iter()
                    .filter(|member| allowed.contains(&member.pivot_item_index))
                    .map(|member| member.member.unique_name.clone())
                    .collect();
                if names.is_empty() {
                    return Err("OLAP hierarchy selection produces an empty required level".into());
                }
                view.members.push(crate::structs::PivotHierarchyMembers {
                    count:   Some(
                        u32::try_from(names.len())
                            .map_err(|_| "OLAP hierarchy member count out of range")?,
                    ),
                    level:   Some(native_field.properties().level.unwrap_or(0)),
                    members: names,
                });
            }
            changes.fields.push(OlapFieldChange {
                sheet_id: link.sheet_id,
                table_name: link.name.clone(),
                field_index: binding.field_index,
                field,
            });
        }
        changes.hierarchies.push(OlapHierarchyChange {
            sheet_id: link.sheet_id,
            table_name: link.name.clone(),
            hierarchy_index,
            hierarchy: view,
        });
    }
    if binding_count != bindings.len() {
        return Err("OLAP context includes an unrelated pivot binding".into());
    }
    changes.members = cache_members.ok_or("OLAP cache has no selection targets")?;
    Ok(changes)
}

impl SlicerCache {
    pub fn plan_cached_olap_selection(
        &self,
        requested: SlicerSelection,
        targets: &[SlicerPivotTarget<'_>],
    ) -> Result<OlapSlicerSelectionPlan, String> {
        let bindings = self.olap_bindings(targets)?;
        self.plan_olap_selection(requested, targets, &bindings)
    }

    pub fn olap_bindings(
        &self,
        targets: &[SlicerPivotTarget<'_>],
    ) -> Result<Vec<OlapPivotBinding>, String> {
        self.validate()?;
        let Some(SlicerSource::Olap(source)) = &self.source else {
            return Err("native OLAP bindings require an OLAP slicer source".into());
        };
        let links = self.linked_pivots();
        if links.is_empty() {
            return Err("OLAP slicer has no linked pivot tables".into());
        }
        let mut result = Vec::new();
        for link in links {
            let matches: Vec<_> = targets
                .iter()
                .filter(|target| {
                    target.sheet_id == link.sheet_id
                        && target.table.pivot_table_definition().name() == link.name
                })
                .collect();
            if matches.len() != 1 {
                return Err("missing or ambiguous OLAP pivot binding".into());
            }
            let table = &*matches[0].table;
            let cache = table.pivot_cache_definition();
            if cache.slicer_cache_id() != Some(source.pivot_cache_id) {
                return Err("OLAP cache namespace identity mismatch".into());
            }
            let hierarchies = cache
                .hierarchies()
                .ok_or("OLAP pivot cache has no native hierarchy context")?;
            let matching: Vec<_> = hierarchies
                .hierarchies
                .iter()
                .enumerate()
                .filter(|(_, hierarchy)| hierarchy.unique_name == self.source_name)
                .collect();
            if matching.len() != 1 {
                return Err("missing or ambiguous native OLAP hierarchy identity".into());
            }
            let (hierarchy_index, hierarchy) = matching[0];
            let usage = hierarchy
                .fields_usage
                .as_ref()
                .ok_or("OLAP hierarchy has no native field usage")?;
            let all_member = hierarchy
                .all_unique_name
                .as_ref()
                .filter(|name| !name.is_empty())
                .map(|name| OlapSlicerSelection {
                    unique_name: name.clone(),
                    parents:     Vec::new(),
                });
            let mut fields_seen = HashSet::new();
            let mut levels_seen = HashSet::new();
            for (level_index, index) in usage.fields.iter().enumerate() {
                if *index == -1 {
                    continue;
                }
                let field_index =
                    usize::try_from(*index).map_err(|_| "invalid native OLAP field usage index")?;
                if !fields_seen.insert(field_index) {
                    return Err("duplicate native OLAP field usage".into());
                }
                let native_field = cache
                    .cache_fields()
                    .list()
                    .get(field_index)
                    .ok_or("OLAP field usage points outside the cache fields")?;
                if native_field.properties().hierarchy.unwrap_or(0)
                    != i32::try_from(hierarchy_index)
                        .map_err(|_| "OLAP hierarchy index exceeds signed range")?
                    || native_field.properties().level.unwrap_or(0) as usize != level_index
                {
                    return Err("OLAP field usage hierarchy/level back-reference mismatch".into());
                }
                if native_field.is_ignored()
                    || native_field.properties().member_property_field == Some(true)
                {
                    continue;
                }
                let levels: Vec<_> = source
                    .levels
                    .levels
                    .iter()
                    .filter(|level| level.unique_name == native_field.name())
                    .collect();
                if levels.is_empty() {
                    continue;
                }
                if levels.len() != 1 || !levels_seen.insert(&levels[0].unique_name) {
                    return Err("ambiguous native OLAP level identity".into());
                }
                let level = levels[0];
                let unique_names = native_field.cached_unique_names().ok_or(
                    "OLAP field has no native cached MDX identities; a real data-provider refresh \
                     is required",
                )?;
                let field = table
                    .pivot_table_definition()
                    .pivot_fields()
                    .list()
                    .get(field_index)
                    .ok_or("OLAP cache field has no native pivot field")?;
                let mut native_indexes = HashSet::new();
                for item in field.items().list() {
                    if !item.is_data_item() {
                        continue;
                    }
                    if !item.has_index() || !native_indexes.insert(item.index()) {
                        return Err("missing or duplicate native OLAP pivot-item index".into());
                    }
                }
                let mut members = Vec::new();
                for item in level.ranges.iter().flatten().flat_map(|range| &range.items) {
                    let names: Vec<_> = unique_names
                        .iter()
                        .filter(|name| name.unique_name == item.unique_name)
                        .collect();
                    if names.len() != 1 || !native_indexes.contains(&names[0].index) {
                        return Err("cached OLAP MDX member has no unique native pivot-item \
                                    binding"
                            .into());
                    }
                    members.push(OlapPivotMemberBinding {
                        member:           OlapSlicerSelection {
                            unique_name: item.unique_name.clone(),
                            parents:     item.parents.clone(),
                        },
                        pivot_item_index: names[0].index,
                    });
                }
                result.push(OlapPivotBinding {
                    sheet_id: link.sheet_id,
                    pivot_table_name: link.name.clone(),
                    pivot_cache_id: source.pivot_cache_id,
                    field_index,
                    members,
                    all_member: all_member.clone(),
                });
            }
            if levels_seen.len() != source.levels.levels.len() {
                return Err("cached OLAP levels are missing native cache-field bindings".into());
            }
        }
        Ok(result)
    }
}

impl OlapSlicerSelectionPlan {
    pub fn apply(
        self,
        cache: &mut SlicerCache,
        targets: &mut [SlicerPivotTarget<'_>],
    ) -> Result<(), String> {
        if cache.to_xml()? != self.cache_xml {
            return Err("OLAP slicer changed after selection validation".into());
        }
        if olap_source_snapshots(cache, targets)? != self.native_sources {
            return Err("native OLAP source identities changed after selection validation".into());
        }
        let prepared = prepare_olap(cache, &self.requested, targets, &self.bindings)?;
        let member_count = u32::try_from(prepared.members.len())
            .map_err(|_| "OLAP selection member count out of range")?;
        for change in prepared.fields {
            let target = targets
                .iter_mut()
                .find(|target| {
                    target.sheet_id == change.sheet_id
                        && target.table.pivot_table_definition().name() == change.table_name
                })
                .expect("validated OLAP target");
            target
                .table
                .pivot_table_definition_mut()
                .pivot_fields_mut()
                .list_mut()[change.field_index] = change.field;
        }
        for change in prepared.hierarchies {
            let target = targets
                .iter_mut()
                .find(|target| {
                    target.sheet_id == change.sheet_id
                        && target.table.pivot_table_definition().name() == change.table_name
                })
                .expect("validated OLAP hierarchy target");
            target
                .table
                .pivot_table_definition_mut()
                .hierarchies_mut()
                .as_mut()
                .expect("validated native hierarchy collection")
                .hierarchies[change.hierarchy_index] = change.hierarchy;
        }
        let members = prepared.members;
        let Some(SlicerSource::Olap(source)) = &mut cache.source else {
            unreachable!("validated OLAP source")
        };
        source.selections.count = source.selections.count.map(|_| member_count);
        source.selections.selections = members;
        Ok(())
    }
}

fn olap_source_snapshots(
    cache: &SlicerCache,
    targets: &[SlicerPivotTarget<'_>],
) -> Result<Vec<(u32, String, u32, String)>, String> {
    let mut result = Vec::new();
    for link in cache.linked_pivots() {
        let target = targets
            .iter()
            .find(|target| {
                target.sheet_id == link.sheet_id
                    && target.table.pivot_table_definition().name() == link.name
            })
            .ok_or("missing OLAP snapshot binding")?;
        result.push((
            link.sheet_id,
            link.name.clone(),
            target.table.pivot_table_definition().cache_id(),
            target
                .table
                .pivot_cache_definition()
                .hash_code()
                .map_err(|error| error.to_string())?,
        ));
    }
    Ok(result)
}
