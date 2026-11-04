use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use super::{
    SLICER_2013_NS,
    SLICER_NS,
    model::{
        OlapSlicerCache,
        OlapSlicerItem,
        OlapSlicerLevel,
        OlapSlicerLevels,
        OlapSlicerRange,
        OlapSlicerSelection,
        OlapSlicerSelections,
        PivotCacheExtension,
        PivotCacheSlicerProperties,
        Slicer,
        SlicerCache,
        SlicerCacheExtension,
        SlicerCrossFilter,
        SlicerDrawing,
        SlicerHiddenLevel,
        SlicerHideItemsWithNoData,
        SlicerPivotTable,
        SlicerSortOrder,
        SlicerSource,
        TableSlicerCache,
        TabularSlicerCache,
        TabularSlicerItem,
        TabularSlicerItems,
    },
};
use crate::{
    drawing::{
        charts::{
            NamespaceScope,
            OpaqueElement,
        },
        spreadsheet::SlicerFrameKind,
    },
    writer::driver::write_end_tag,
    xml::{
        attr,
        boolean,
        direct_children,
        encode_xstring,
        expect,
        no_children,
        optional,
        optional_xstring_attribute,
        required,
        required_unsigned,
        root,
        start,
        unsigned,
    },
};

const SHEET_NS: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
const REVISION_NS: &str = "http://schemas.microsoft.com/office/spreadsheetml/2016/revision10";
const MC_NS: &str = "http://schemas.openxmlformats.org/markup-compatibility/2006";
const TABLE_URI: &str = "{2F2917AC-EB37-4324-AD4E-5DD8C200BD13}";
const HIDE_URI: &str = "{470722E0-AACD-4C17-9CDC-17EF765DBC7E}";
const PIVOTS_URI: &str = "{03082B11-2C62-411c-B77F-237D8FCFBE4C}";
const PIVOT_CACHE_URI: &str = "{725AE2AE-9491-48be-B2B4-4EB974FC3084}";

fn string(node: &OpaqueElement, name: &str) -> Result<String, String> {
    Ok(crate::helper::utils::unescape_xstring_owned(required(
        node, name,
    )?))
}

fn attributes(node: &OpaqueElement, allowed: &[&str]) -> Result<(), String> {
    let qualified: &[(&str, &str)] = if matches!(
        node.name.local_name.as_str(),
        "slicer" | "slicerCacheDefinition"
    ) {
        &[(REVISION_NS, "uid"), (MC_NS, "Ignorable")]
    } else {
        &[(MC_NS, "Ignorable")]
    };
    crate::xml::attributes(node, allowed, qualified)
}

fn unique<T>(slot: &mut Option<T>, value: T, name: &str) -> Result<(), String> {
    if slot.is_some() {
        return Err(format!("duplicate {name}"));
    }
    *slot = Some(value);
    Ok(())
}

fn sort_order(node: &OpaqueElement, olap: bool) -> Result<Option<SlicerSortOrder>, String> {
    attr(node, "sortOrder")
        .map(|value| match value {
            "ascending" => Ok(SlicerSortOrder::Ascending),
            "descending" => Ok(SlicerSortOrder::Descending),
            "natural" if olap => Ok(SlicerSortOrder::Natural),
            _ => Err("invalid slicer sort order".into()),
        })
        .transpose()
}

fn cross_filter(node: &OpaqueElement) -> Result<Option<SlicerCrossFilter>, String> {
    attr(node, "crossFilter")
        .map(|value| match value {
            "none" => Ok(SlicerCrossFilter::None),
            "showItemsWithDataAtTop" => Ok(SlicerCrossFilter::ItemsWithDataFirst),
            "showItemsWithNoData" => Ok(SlicerCrossFilter::ShowItemsWithNoData),
            _ => Err("invalid slicer cross filter".into()),
        })
        .transpose()
}

fn pivot_tables(node: &OpaqueElement) -> Result<Vec<SlicerPivotTable>, String> {
    attributes(node, &[])?;
    let values = direct_children(node)?
        .iter()
        .map(|child| {
            expect(child, SLICER_NS, "pivotTable")?;
            attributes(child, &["tabId", "name"])?;
            no_children(child)?;
            Ok(SlicerPivotTable {
                sheet_id: required_unsigned(child, "tabId")?,
                name:     string(child, "name")?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    if values.is_empty() {
        return Err("empty slicer pivot-table list".into());
    }
    Ok(values)
}

fn tabular(node: &OpaqueElement) -> Result<TabularSlicerCache, String> {
    attributes(
        node,
        &[
            "pivotCacheId",
            "sortOrder",
            "customListSort",
            "showMissing",
            "crossFilter",
        ],
    )?;
    let mut value = TabularSlicerCache {
        pivot_cache_id: required_unsigned(node, "pivotCacheId")?,
        sort_order: sort_order(node, false)?,
        custom_list_sort: boolean(node, "customListSort")?,
        show_missing: boolean(node, "showMissing")?,
        cross_filter: cross_filter(node)?,
        ..Default::default()
    };
    for child in direct_children(node)? {
        if child.name.is(SLICER_NS, "items") {
            attributes(&child, &["count"])?;
            let mut items = TabularSlicerItems {
                count: unsigned(&child, "count")?,
                items: Vec::new(),
            };
            for item in direct_children(&child)? {
                expect(&item, SLICER_NS, "i")?;
                attributes(&item, &["x", "s", "nd"])?;
                no_children(&item)?;
                items.items.push(TabularSlicerItem {
                    index:    required_unsigned(&item, "x")?,
                    selected: boolean(&item, "s")?,
                    no_data:  boolean(&item, "nd")?,
                });
            }
            unique(&mut value.items, items, "tabular items")?;
        } else if child.name.is(SLICER_NS, "extLst") {
            unique(&mut value.extensions, child, "tabular extensions")?;
        } else {
            return Err("unsupported tabular slicer child".into());
        }
    }
    Ok(value)
}

fn parents(node: &OpaqueElement) -> Result<Vec<String>, String> {
    direct_children(node)?
        .iter()
        .map(|child| {
            expect(child, SLICER_NS, "p")?;
            attributes(child, &["n"])?;
            no_children(child)?;
            string(child, "n")
        })
        .collect()
}

fn olap(node: &OpaqueElement) -> Result<OlapSlicerCache, String> {
    attributes(node, &["pivotCacheId"])?;
    let mut levels = None;
    let mut selections = None;
    let mut extensions = None;
    for child in direct_children(node)? {
        if child.name.is(SLICER_NS, "levels") {
            attributes(&child, &["count"])?;
            let mut value = OlapSlicerLevels {
                count:  unsigned(&child, "count")?,
                levels: Vec::new(),
            };
            for level in direct_children(&child)? {
                expect(&level, SLICER_NS, "level")?;
                attributes(
                    &level,
                    &[
                        "uniqueName",
                        "sourceCaption",
                        "count",
                        "sortOrder",
                        "crossFilter",
                    ],
                )?;
                let mut item = OlapSlicerLevel {
                    unique_name:    string(&level, "uniqueName")?,
                    source_caption: optional_xstring_attribute(&level, "sourceCaption"),
                    count:          required_unsigned(&level, "count")?,
                    sort_order:     sort_order(&level, true)?,
                    cross_filter:   cross_filter(&level)?,
                    ranges:         None,
                };
                for ranges in direct_children(&level)? {
                    expect(&ranges, SLICER_NS, "ranges")?;
                    attributes(&ranges, &[])?;
                    let mut values = Vec::new();
                    for range in direct_children(&ranges)? {
                        expect(&range, SLICER_NS, "range")?;
                        attributes(&range, &["startItem"])?;
                        let mut data = OlapSlicerRange {
                            start_item: required_unsigned(&range, "startItem")?,
                            items:      Vec::new(),
                        };
                        for leaf in direct_children(&range)? {
                            expect(&leaf, SLICER_NS, "i")?;
                            attributes(&leaf, &["n", "c", "nd"])?;
                            data.items.push(OlapSlicerItem {
                                unique_name: string(&leaf, "n")?,
                                caption:     optional_xstring_attribute(&leaf, "c"),
                                no_data:     boolean(&leaf, "nd")?,
                                parents:     parents(&leaf)?,
                            });
                        }
                        values.push(data);
                    }
                    unique(&mut item.ranges, values, "OLAP ranges")?;
                }
                value.levels.push(item);
            }
            unique(&mut levels, value, "OLAP levels")?;
        } else if child.name.is(SLICER_NS, "selections") {
            attributes(&child, &["count"])?;
            let mut value = OlapSlicerSelections {
                count:      unsigned(&child, "count")?,
                selections: Vec::new(),
            };
            for selection in direct_children(&child)? {
                expect(&selection, SLICER_NS, "selection")?;
                attributes(&selection, &["n"])?;
                value.selections.push(OlapSlicerSelection {
                    unique_name: string(&selection, "n")?,
                    parents:     parents(&selection)?,
                });
            }
            unique(&mut selections, value, "OLAP selections")?;
        } else if child.name.is(SLICER_NS, "extLst") {
            unique(&mut extensions, child, "OLAP extensions")?;
        } else {
            return Err("unsupported OLAP slicer child".into());
        }
    }
    Ok(OlapSlicerCache {
        pivot_cache_id: required_unsigned(node, "pivotCacheId")?,
        levels: levels.ok_or("missing OLAP levels")?,
        selections: selections.ok_or("missing OLAP selections")?,
        extensions,
    })
}

fn table(node: &OpaqueElement) -> Result<TableSlicerCache, String> {
    attributes(
        node,
        &[
            "tableId",
            "column",
            "sortOrder",
            "customListSort",
            "crossFilter",
        ],
    )?;
    let mut value = TableSlicerCache {
        table_id:         required_unsigned(node, "tableId")?,
        column_id:        required_unsigned(node, "column")?,
        sort_order:       sort_order(node, false)?,
        custom_list_sort: boolean(node, "customListSort")?,
        cross_filter:     cross_filter(node)?,
        extensions:       None,
    };
    for child in direct_children(node)? {
        expect(&child, SLICER_2013_NS, "extLst")?;
        unique(&mut value.extensions, child, "table slicer extensions")?;
    }
    Ok(value)
}

fn cache(node: &OpaqueElement) -> Result<SlicerCache, String> {
    expect(node, SLICER_NS, "slicerCacheDefinition")?;
    attributes(node, &["name", "sourceName"])?;
    let mut value = SlicerCache {
        name: string(node, "name")?,
        source_name: string(node, "sourceName")?,
        uid: node
            .attributes
            .iter()
            .find(|(name, _)| name.is(REVISION_NS, "uid"))
            .map(|(_, value)| value.clone()),
        ..Default::default()
    };
    let mut extension_list = false;
    for child in direct_children(node)? {
        if child.name.is(SLICER_NS, "pivotTables") {
            unique(
                &mut value.pivot_tables,
                pivot_tables(&child)?,
                "slicer pivot tables",
            )?;
        } else if child.name.is(SLICER_NS, "data") {
            attributes(&child, &[])?;
            let data = direct_children(&child)?;
            if data.len() != 1 {
                return Err("slicer data requires one source".into());
            }
            let source = if data[0].name.is(SLICER_NS, "tabular") {
                SlicerSource::TabularPivot(tabular(&data[0])?)
            } else if data[0].name.is(SLICER_NS, "olap") {
                SlicerSource::Olap(olap(&data[0])?)
            } else {
                return Err("unsupported slicer data source".into());
            };
            unique(&mut value.source, source, "slicer source")?;
        } else if child.name.is(SLICER_NS, "extLst") {
            if extension_list {
                return Err("duplicate slicer cache extension list".into());
            }
            extension_list = true;
            value.extensions_present = true;
            attributes(&child, &[])?;
            for extension in direct_children(&child)? {
                expect(&extension, SHEET_NS, "ext")?;
                let payloads = direct_children(&extension)?;
                let uri = attr(&extension, "uri").unwrap_or("");
                let known = payloads.len() == 1 && payloads[0].name.namespace_uri == SLICER_2013_NS;
                let expected = if uri.eq_ignore_ascii_case(TABLE_URI) {
                    Some("tableSlicerCache")
                } else if uri.eq_ignore_ascii_case(HIDE_URI) {
                    Some("slicerCacheHideItemsWithNoData")
                } else if uri.eq_ignore_ascii_case(PIVOTS_URI) {
                    Some("slicerCachePivotTables")
                } else {
                    None
                };
                if let Some(expected) = expected {
                    if !known || payloads[0].name.local_name != expected {
                        return Err(format!("malformed known slicer extension {uri}"));
                    }
                }
                if known
                    && uri.eq_ignore_ascii_case(TABLE_URI)
                    && payloads[0].name.local_name == "tableSlicerCache"
                {
                    attributes(&extension, &["uri"])?;
                    unique(
                        &mut value.source,
                        SlicerSource::Table(table(&payloads[0])?),
                        "table slicer source",
                    )?;
                    value
                        .extensions
                        .push(SlicerCacheExtension::TableSource { uri: uri.into() });
                } else if known
                    && uri.eq_ignore_ascii_case(HIDE_URI)
                    && payloads[0].name.local_name == "slicerCacheHideItemsWithNoData"
                {
                    attributes(&extension, &["uri"])?;
                    let payload = &payloads[0];
                    attributes(payload, &["count"])?;
                    let mut settings = SlicerHideItemsWithNoData {
                        count:  unsigned(payload, "count")?,
                        levels: Vec::new(),
                    };
                    for level in direct_children(payload)? {
                        expect(&level, SLICER_2013_NS, "slicerCacheOlapLevelName")?;
                        attributes(&level, &["uniqueName", "count"])?;
                        no_children(&level)?;
                        settings.levels.push(SlicerHiddenLevel {
                            unique_name: string(&level, "uniqueName")?,
                            count:       required_unsigned(&level, "count")?,
                        });
                    }
                    value
                        .extensions
                        .push(SlicerCacheExtension::HideItemsWithNoData {
                            uri: uri.into(),
                            settings,
                        });
                } else if known
                    && uri.eq_ignore_ascii_case(PIVOTS_URI)
                    && payloads[0].name.local_name == "slicerCachePivotTables"
                {
                    attributes(&extension, &["uri"])?;
                    value.extensions.push(SlicerCacheExtension::PivotTables {
                        uri:    uri.into(),
                        tables: pivot_tables(&payloads[0])?,
                    });
                } else {
                    value
                        .extensions
                        .push(SlicerCacheExtension::Future(extension));
                }
            }
        } else {
            return Err("unsupported slicer cache child".into());
        }
    }
    value.validate()?;
    Ok(value)
}

impl SlicerCache {
    pub fn from_xml(bytes: &[u8]) -> Result<Self, String> {
        cache(&root(bytes)?)
    }

    pub(crate) fn read_xml<R: std::io::BufRead>(
        reader: &mut Reader<R>,
        start: &BytesStart,
        empty: bool,
        inherited: &NamespaceScope,
    ) -> Result<Self, String> {
        cache(&OpaqueElement::read(reader, start, empty, inherited)?)
    }
}

impl Slicer {
    pub fn from_xml(bytes: &[u8]) -> Result<Vec<Self>, String> {
        let node = root(bytes)?;
        expect(&node, SLICER_NS, "slicers")?;
        attributes(&node, &[])?;
        let values = direct_children(&node)?
            .iter()
            .map(Self::from_element)
            .collect::<Result<Vec<_>, _>>()?;
        if values.is_empty() {
            return Err("empty slicer view list".into());
        }
        Ok(values)
    }

    fn from_element(node: &OpaqueElement) -> Result<Self, String> {
        expect(node, SLICER_NS, "slicer")?;
        attributes(
            node,
            &[
                "name",
                "cache",
                "caption",
                "startItem",
                "columnCount",
                "showCaption",
                "level",
                "style",
                "lockedPosition",
                "rowHeight",
            ],
        )?;
        let mut value = Self {
            name:            string(node, "name")?,
            cache:           string(node, "cache")?,
            uid:             node
                .attributes
                .iter()
                .find(|(name, _)| name.is(REVISION_NS, "uid"))
                .map(|(_, value)| value.clone()),
            caption:         optional_xstring_attribute(node, "caption"),
            start_item:      unsigned(node, "startItem")?,
            column_count:    unsigned(node, "columnCount")?,
            show_caption:    boolean(node, "showCaption")?,
            level:           unsigned(node, "level")?,
            style:           optional_xstring_attribute(node, "style"),
            locked_position: boolean(node, "lockedPosition")?,
            row_height:      required_unsigned(node, "rowHeight")?,
            extensions:      None,
        };
        for child in direct_children(node)? {
            expect(&child, SLICER_NS, "extLst")?;
            unique(&mut value.extensions, child, "slicer view extensions")?;
        }
        value.validate()?;
        Ok(value)
    }
}

impl SlicerDrawing {
    pub(crate) fn read_xml<R: std::io::BufRead>(
        reader: &mut Reader<R>,
        start: &BytesStart,
        empty: bool,
        inherited: &NamespaceScope,
        kind: SlicerFrameKind,
    ) -> Result<Self, String> {
        let node = OpaqueElement::read(reader, start, empty, inherited)?;
        expect(&node, kind.namespace(), kind.element_name())?;
        attributes(&node, &["name"])?;
        let mut value = Self {
            name:       required(&node, "name")?,
            extensions: None,
        };
        for child in direct_children(&node)? {
            expect(&child, kind.namespace(), "extLst")?;
            unique(&mut value.extensions, child, "slicer drawing extensions")?;
        }
        Ok(value)
    }
}

pub(crate) fn read_pivot_cache_extensions<R: std::io::BufRead>(
    reader: &mut Reader<R>,
    start: &BytesStart,
    empty: bool,
    inherited: &NamespaceScope,
) -> Result<Vec<PivotCacheExtension>, String> {
    let node = OpaqueElement::read(reader, start, empty, inherited)?;
    expect(&node, SHEET_NS, "extLst")?;
    let mut properties = None;
    let mut future = Vec::new();
    for extension in direct_children(&node)? {
        expect(&extension, SHEET_NS, "ext")?;
        let payloads = direct_children(&extension)?;
        if attr(&extension, "uri").is_some_and(|uri| uri.eq_ignore_ascii_case(PIVOT_CACHE_URI))
            && (payloads.len() != 1 || !payloads[0].name.is(SLICER_NS, "pivotCacheDefinition"))
        {
            return Err("malformed known pivot-cache slicer extension".into());
        }
        if attr(&extension, "uri").is_some_and(|uri| uri.eq_ignore_ascii_case(PIVOT_CACHE_URI))
            && payloads.len() == 1
            && payloads[0].name.is(SLICER_NS, "pivotCacheDefinition")
        {
            attributes(&extension, &["uri"])?;
            let payload = &payloads[0];
            attributes(
                payload,
                &[
                    "slicerData",
                    "pivotCacheId",
                    "supportSubqueryNonVisual",
                    "supportSubqueryCalcMem",
                    "supportAddCalcMems",
                ],
            )?;
            no_children(payload)?;
            unique(
                &mut properties,
                PivotCacheSlicerProperties {
                    slicer_data:                         boolean(payload, "slicerData")?,
                    pivot_cache_id:                      unsigned(payload, "pivotCacheId")?,
                    support_subquery_non_visual:         boolean(
                        payload,
                        "supportSubqueryNonVisual",
                    )?,
                    support_subquery_calculated_members: boolean(
                        payload,
                        "supportSubqueryCalcMem",
                    )?,
                    support_add_calculated_members:      boolean(payload, "supportAddCalcMems")?,
                },
                "pivot cache slicer properties",
            )?;
            future.push(PivotCacheExtension::SlicerProperties {
                uri:        required(&extension, "uri")?,
                properties: properties.clone().expect("parsed slicer properties"),
            });
        } else {
            future.push(PivotCacheExtension::Future(extension));
        }
    }
    Ok(future)
}

fn common_namespaces() -> Vec<(&'static str, String)> {
    vec![
        ("xmlns", SLICER_NS.into()),
        ("xmlns:x", SHEET_NS.into()),
        ("xmlns:x15", SLICER_2013_NS.into()),
        ("xmlns:xr10", REVISION_NS.into()),
        ("xmlns:mc", MC_NS.into()),
        ("mc:Ignorable", "x15 xr10".into()),
    ]
}

fn scope() -> NamespaceScope {
    NamespaceScope::with_bindings([
        (None, SLICER_NS.into()),
        (Some("x".into()), SHEET_NS.into()),
        (Some("x15".into()), SLICER_2013_NS.into()),
        (Some("xr10".into()), REVISION_NS.into()),
        (Some("mc".into()), MC_NS.into()),
    ])
}

fn write_sort(attributes: &mut Vec<(&'static str, String)>, value: Option<SlicerSortOrder>) {
    optional(
        attributes,
        "sortOrder",
        value.map(|value| match value {
            SlicerSortOrder::Ascending => "ascending",
            SlicerSortOrder::Descending => "descending",
            SlicerSortOrder::Natural => "natural",
        }),
    );
}

fn write_cross(attributes: &mut Vec<(&'static str, String)>, value: Option<SlicerCrossFilter>) {
    optional(
        attributes,
        "crossFilter",
        value.map(|value| match value {
            SlicerCrossFilter::None => "none",
            SlicerCrossFilter::ItemsWithDataFirst => "showItemsWithDataAtTop",
            SlicerCrossFilter::ShowItemsWithNoData => "showItemsWithNoData",
        }),
    );
}

fn write_pivots(writer: &mut Writer<Cursor<Vec<u8>>>, name: &str, values: &[SlicerPivotTable]) {
    start(writer, name, Vec::new(), false);
    for value in values {
        start(
            writer,
            "pivotTable",
            vec![
                ("tabId", value.sheet_id.to_string()),
                ("name", encode_xstring(&value.name)),
            ],
            true,
        );
    }
    write_end_tag(writer, name.to_owned());
}

fn write_parents(writer: &mut Writer<Cursor<Vec<u8>>>, values: &[String]) {
    for value in values {
        start(writer, "p", vec![("n", encode_xstring(value))], true);
    }
}

fn write_tabular(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    value: &TabularSlicerCache,
) -> Result<(), String> {
    let mut attributes = vec![("pivotCacheId", value.pivot_cache_id.to_string())];
    write_sort(&mut attributes, value.sort_order);
    optional(&mut attributes, "customListSort", value.custom_list_sort);
    optional(&mut attributes, "showMissing", value.show_missing);
    write_cross(&mut attributes, value.cross_filter);
    start(writer, "tabular", attributes, false);
    if let Some(items) = &value.items {
        let mut attributes = Vec::new();
        optional(&mut attributes, "count", items.count);
        start(writer, "items", attributes, false);
        for item in &items.items {
            let mut attributes = vec![("x", item.index.to_string())];
            optional(&mut attributes, "s", item.selected);
            optional(&mut attributes, "nd", item.no_data);
            start(writer, "i", attributes, true);
        }
        write_end_tag(writer, "items");
    }
    if let Some(extension) = &value.extensions {
        extension.write_to(writer, &scope())?;
    }
    write_end_tag(writer, "tabular");
    Ok(())
}

fn write_olap(writer: &mut Writer<Cursor<Vec<u8>>>, value: &OlapSlicerCache) -> Result<(), String> {
    start(
        writer,
        "olap",
        vec![("pivotCacheId", value.pivot_cache_id.to_string())],
        false,
    );
    let mut attributes = Vec::new();
    optional(&mut attributes, "count", value.levels.count);
    start(writer, "levels", attributes, false);
    for level in &value.levels.levels {
        let mut attributes = vec![
            ("uniqueName", encode_xstring(&level.unique_name)),
            ("count", level.count.to_string()),
        ];
        optional(
            &mut attributes,
            "sourceCaption",
            level.source_caption.as_deref().map(encode_xstring),
        );
        write_sort(&mut attributes, level.sort_order);
        write_cross(&mut attributes, level.cross_filter);
        start(writer, "level", attributes, false);
        if let Some(ranges) = &level.ranges {
            start(writer, "ranges", Vec::new(), false);
            for range in ranges {
                start(
                    writer,
                    "range",
                    vec![("startItem", range.start_item.to_string())],
                    false,
                );
                for item in &range.items {
                    let mut attributes = vec![("n", encode_xstring(&item.unique_name))];
                    optional(
                        &mut attributes,
                        "c",
                        item.caption.as_deref().map(encode_xstring),
                    );
                    optional(&mut attributes, "nd", item.no_data);
                    start(writer, "i", attributes, item.parents.is_empty());
                    if !item.parents.is_empty() {
                        write_parents(writer, &item.parents);
                        write_end_tag(writer, "i");
                    }
                }
                write_end_tag(writer, "range");
            }
            write_end_tag(writer, "ranges");
        }
        write_end_tag(writer, "level");
    }
    write_end_tag(writer, "levels");
    let mut attributes = Vec::new();
    optional(&mut attributes, "count", value.selections.count);
    start(writer, "selections", attributes, false);
    for selection in &value.selections.selections {
        start(
            writer,
            "selection",
            vec![("n", encode_xstring(&selection.unique_name))],
            selection.parents.is_empty(),
        );
        if !selection.parents.is_empty() {
            write_parents(writer, &selection.parents);
            write_end_tag(writer, "selection");
        }
    }
    write_end_tag(writer, "selections");
    if let Some(extension) = &value.extensions {
        extension.write_to(writer, &scope())?;
    }
    write_end_tag(writer, "olap");
    Ok(())
}

fn write_table(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    value: &TableSlicerCache,
) -> Result<(), String> {
    let mut attributes = vec![
        ("tableId", value.table_id.to_string()),
        ("column", value.column_id.to_string()),
    ];
    write_sort(&mut attributes, value.sort_order);
    optional(&mut attributes, "customListSort", value.custom_list_sort);
    write_cross(&mut attributes, value.cross_filter);
    start(
        writer,
        "x15:tableSlicerCache",
        attributes,
        value.extensions.is_none(),
    );
    if let Some(extension) = &value.extensions {
        extension.write_to(writer, &scope())?;
        write_end_tag(writer, "x15:tableSlicerCache");
    }
    Ok(())
}

impl SlicerCache {
    pub fn to_xml(&self) -> Result<Vec<u8>, String> {
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        self.write_to(&mut writer)?;
        Ok(writer.into_inner().into_inner())
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) -> Result<(), String> {
        self.validate()?;
        let mut attributes = common_namespaces();
        attributes.extend([
            ("name", encode_xstring(&self.name)),
            ("sourceName", encode_xstring(&self.source_name)),
        ]);
        optional(&mut attributes, "xr10:uid", self.uid.as_deref());
        start(writer, "slicerCacheDefinition", attributes, false);
        if let Some(pivots) = &self.pivot_tables {
            write_pivots(writer, "pivotTables", pivots);
        }
        if let Some(source) = &self.source {
            if !matches!(source, SlicerSource::Table(_)) {
                start(writer, "data", Vec::new(), false);
                match source {
                    SlicerSource::TabularPivot(value) => write_tabular(writer, value)?,
                    SlicerSource::Olap(value) => write_olap(writer, value)?,
                    SlicerSource::Table(_) => unreachable!(),
                }
                write_end_tag(writer, "data");
            }
        }
        let missing_table_extension = matches!(self.source, Some(SlicerSource::Table(_)))
            && !self
                .extensions
                .iter()
                .any(|value| matches!(value, SlicerCacheExtension::TableSource { .. }));
        if self.extensions_present || !self.extensions.is_empty() || missing_table_extension {
            start(writer, "extLst", Vec::new(), false);
            for extension in &self.extensions {
                match extension {
                    SlicerCacheExtension::TableSource { uri } => {
                        start(writer, "x:ext", vec![("uri", uri.clone())], false);
                        let Some(SlicerSource::Table(value)) = &self.source else {
                            return Err("table extension has no table source".into());
                        };
                        write_table(writer, value)?;
                        write_end_tag(writer, "x:ext");
                    }
                    SlicerCacheExtension::HideItemsWithNoData { uri, settings } => {
                        start(writer, "x:ext", vec![("uri", uri.clone())], false);
                        let mut attributes = Vec::new();
                        optional(&mut attributes, "count", settings.count);
                        start(
                            writer,
                            "x15:slicerCacheHideItemsWithNoData",
                            attributes,
                            settings.levels.is_empty(),
                        );
                        if !settings.levels.is_empty() {
                            for level in &settings.levels {
                                start(
                                    writer,
                                    "x15:slicerCacheOlapLevelName",
                                    vec![
                                        ("uniqueName", encode_xstring(&level.unique_name)),
                                        ("count", level.count.to_string()),
                                    ],
                                    true,
                                );
                            }
                            write_end_tag(writer, "x15:slicerCacheHideItemsWithNoData");
                        }
                        write_end_tag(writer, "x:ext");
                    }
                    SlicerCacheExtension::PivotTables { uri, tables } => {
                        start(writer, "x:ext", vec![("uri", uri.clone())], false);
                        write_pivots(writer, "x15:slicerCachePivotTables", tables);
                        write_end_tag(writer, "x:ext");
                    }
                    SlicerCacheExtension::Future(value) => value.write_to(writer, &scope())?,
                }
            }
            if missing_table_extension {
                start(writer, "x:ext", vec![("uri", TABLE_URI.into())], false);
                if let Some(SlicerSource::Table(value)) = &self.source {
                    write_table(writer, value)?;
                }
                write_end_tag(writer, "x:ext");
            }
            write_end_tag(writer, "extLst");
        }
        write_end_tag(writer, "slicerCacheDefinition");
        Ok(())
    }
}

impl Slicer {
    pub fn to_xml(values: &[Self]) -> Result<Vec<u8>, String> {
        if values.is_empty() {
            return Err("empty slicer view list".into());
        }
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        start(&mut writer, "slicers", common_namespaces(), false);
        for value in values {
            value.validate()?;
            let mut attributes = vec![
                ("name", encode_xstring(&value.name)),
                ("cache", encode_xstring(&value.cache)),
                ("rowHeight", value.row_height.to_string()),
            ];
            optional(&mut attributes, "xr10:uid", value.uid.as_deref());
            optional(
                &mut attributes,
                "caption",
                value.caption.as_deref().map(encode_xstring),
            );
            optional(&mut attributes, "startItem", value.start_item);
            optional(&mut attributes, "columnCount", value.column_count);
            optional(&mut attributes, "showCaption", value.show_caption);
            optional(&mut attributes, "level", value.level);
            optional(
                &mut attributes,
                "style",
                value.style.as_deref().map(encode_xstring),
            );
            optional(&mut attributes, "lockedPosition", value.locked_position);
            start(
                &mut writer,
                "slicer",
                attributes,
                value.extensions.is_none(),
            );
            if let Some(extension) = &value.extensions {
                extension.write_to(&mut writer, &scope())?;
                write_end_tag(&mut writer, "slicer");
            }
        }
        write_end_tag(&mut writer, "slicers");
        Ok(writer.into_inner().into_inner())
    }
}

impl SlicerDrawing {
    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        kind: SlicerFrameKind,
    ) -> Result<(), String> {
        if self.name.is_empty() {
            return Err("slicer drawing requires a view name".into());
        }
        let prefix = kind.prefix();
        let element = format!("{prefix}:{}", kind.element_name());
        start(
            writer,
            &element,
            vec![
                (&format!("xmlns:{prefix}"), kind.namespace().into()),
                ("name", self.name.clone()),
            ],
            self.extensions.is_none(),
        );
        if let Some(extension) = &self.extensions {
            let inherited =
                NamespaceScope::with_bindings([(Some(prefix.into()), kind.namespace().into())]);
            extension.write_to(writer, &inherited)?;
            write_end_tag(writer, element);
        }
        Ok(())
    }
}

pub(crate) fn write_pivot_cache_extensions(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    extensions: &[PivotCacheExtension],
    present: bool,
) -> Result<(), String> {
    if extensions.is_empty() && !present {
        return Ok(());
    }
    let inherited = NamespaceScope::with_bindings([(None, SHEET_NS.into())]);
    start(writer, "extLst", Vec::new(), false);
    for extension in extensions {
        match extension {
            PivotCacheExtension::SlicerProperties { uri, properties } => {
                start(
                    writer,
                    "ext",
                    vec![("uri", uri.clone()), ("xmlns:x14", SLICER_NS.into())],
                    false,
                );
                let mut a = Vec::new();
                optional(&mut a, "slicerData", properties.slicer_data);
                optional(&mut a, "pivotCacheId", properties.pivot_cache_id);
                optional(
                    &mut a,
                    "supportSubqueryNonVisual",
                    properties.support_subquery_non_visual,
                );
                optional(
                    &mut a,
                    "supportSubqueryCalcMem",
                    properties.support_subquery_calculated_members,
                );
                optional(
                    &mut a,
                    "supportAddCalcMems",
                    properties.support_add_calculated_members,
                );
                start(writer, "x14:pivotCacheDefinition", a, true);
                write_end_tag(writer, "ext");
            }
            PivotCacheExtension::Future(node) => node.write_to(writer, &inherited)?,
        }
    }
    write_end_tag(writer, "extLst");
    Ok(())
}
