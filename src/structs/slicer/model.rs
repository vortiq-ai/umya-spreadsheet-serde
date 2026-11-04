use crate::drawing::charts::OpaqueElement;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SlicerSortOrder {
    #[default]
    Ascending,
    Descending,
    Natural,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SlicerCrossFilter {
    None,
    #[default]
    ItemsWithDataFirst,
    ShowItemsWithNoData,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum SlicerItemKey {
    PivotItem(u32),
    OlapMember {
        unique_name: String,
        parents:     Vec<String>,
    },
    TableValue(SlicerTableValue),
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum SlicerTableValue {
    Blank,
    Text(String),
    Number(String),
    Boolean(bool),
    Error(String),
    Date(String),
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum SlicerSelection {
    #[default]
    All,
    Items(Vec<SlicerItemKey>),
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlicerPivotTable {
    pub sheet_id: u32,
    pub name:     String,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default)]
pub struct SlicerCache {
    pub name:               String,
    pub source_name:        String,
    pub uid:                Option<String>,
    pub pivot_tables:       Option<Vec<SlicerPivotTable>>,
    pub source:             Option<SlicerSource>,
    pub extensions:         Vec<SlicerCacheExtension>,
    pub extensions_present: bool,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub enum SlicerSource {
    TabularPivot(TabularSlicerCache),
    Olap(OlapSlicerCache),
    Table(TableSlicerCache),
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub enum SlicerCacheExtension {
    TableSource {
        uri: String,
    },
    HideItemsWithNoData {
        uri:      String,
        settings: SlicerHideItemsWithNoData,
    },
    PivotTables {
        uri:    String,
        tables: Vec<SlicerPivotTable>,
    },
    Future(OpaqueElement),
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default)]
pub struct TabularSlicerCache {
    pub pivot_cache_id:   u32,
    pub sort_order:       Option<SlicerSortOrder>,
    pub custom_list_sort: Option<bool>,
    pub show_missing:     Option<bool>,
    pub cross_filter:     Option<SlicerCrossFilter>,
    pub items:            Option<TabularSlicerItems>,
    pub extensions:       Option<OpaqueElement>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default)]
pub struct TabularSlicerItems {
    pub count: Option<u32>,
    pub items: Vec<TabularSlicerItem>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TabularSlicerItem {
    pub index:    u32,
    pub selected: Option<bool>,
    pub no_data:  Option<bool>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default)]
pub struct OlapSlicerCache {
    pub pivot_cache_id: u32,
    pub levels:         OlapSlicerLevels,
    pub selections:     OlapSlicerSelections,
    pub extensions:     Option<OpaqueElement>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default)]
pub struct OlapSlicerLevels {
    pub count:  Option<u32>,
    pub levels: Vec<OlapSlicerLevel>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default)]
pub struct OlapSlicerLevel {
    pub unique_name:    String,
    pub source_caption: Option<String>,
    pub count:          u32,
    pub sort_order:     Option<SlicerSortOrder>,
    pub cross_filter:   Option<SlicerCrossFilter>,
    pub ranges:         Option<Vec<OlapSlicerRange>>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default)]
pub struct OlapSlicerRange {
    pub start_item: u32,
    pub items:      Vec<OlapSlicerItem>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default)]
pub struct OlapSlicerItem {
    pub unique_name: String,
    pub caption:     Option<String>,
    pub no_data:     Option<bool>,
    pub parents:     Vec<String>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OlapSlicerSelection {
    pub unique_name: String,
    pub parents:     Vec<String>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default)]
pub struct OlapSlicerSelections {
    pub count:      Option<u32>,
    pub selections: Vec<OlapSlicerSelection>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default)]
pub struct TableSlicerCache {
    pub table_id:         u32,
    pub column_id:        u32,
    pub sort_order:       Option<SlicerSortOrder>,
    pub custom_list_sort: Option<bool>,
    pub cross_filter:     Option<SlicerCrossFilter>,
    pub extensions:       Option<OpaqueElement>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default)]
pub struct SlicerHideItemsWithNoData {
    pub count:  Option<u32>,
    pub levels: Vec<SlicerHiddenLevel>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default)]
pub struct SlicerHiddenLevel {
    pub unique_name: String,
    pub count:       u32,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default)]
pub struct Slicer {
    pub name:            String,
    pub cache:           String,
    pub uid:             Option<String>,
    pub caption:         Option<String>,
    pub start_item:      Option<u32>,
    pub column_count:    Option<u32>,
    pub show_caption:    Option<bool>,
    pub level:           Option<u32>,
    pub style:           Option<String>,
    pub locked_position: Option<bool>,
    pub row_height:      u32,
    pub extensions:      Option<OpaqueElement>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default)]
pub struct SlicerDrawing {
    pub name:       String,
    pub extensions: Option<OpaqueElement>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default)]
pub struct PivotCacheSlicerProperties {
    pub slicer_data:                         Option<bool>,
    pub pivot_cache_id:                      Option<u32>,
    pub support_subquery_non_visual:         Option<bool>,
    pub support_subquery_calculated_members: Option<bool>,
    pub support_add_calculated_members:      Option<bool>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub enum PivotCacheExtension {
    SlicerProperties {
        uri:        String,
        properties: PivotCacheSlicerProperties,
    },
    Future(OpaqueElement),
}
