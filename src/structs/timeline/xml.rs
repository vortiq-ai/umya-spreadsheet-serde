use std::io::Cursor;

use quick_xml::Writer;

use super::model::{
    Timeline,
    TimelineCache,
    TimelineLevel,
    TimelinePivotFilter,
    TimelineRange,
    TimelineState,
    TimelineStyle,
    TimelineStyles,
    parse_date_time,
};
use crate::{
    drawing::charts::{
        NamespaceScope,
        OpaqueElement,
    },
    helper::const_str::{
        MC_NS,
        SHEET_MAIN_NS,
    },
    structs::{
        SLICER_2013_NS,
        SlicerPivotTable,
    },
    writer::driver::write_end_tag,
    xml::{
        attr,
        boolean,
        direct_children,
        encode_xstring,
        expect,
        no_children,
        optional_xstring_attribute,
        required,
        required_unsigned,
        root,
        start,
        unsigned,
    },
};

const REVISION_NS: &str = "http://schemas.microsoft.com/office/spreadsheetml/2016/revision10";
pub(crate) const TIMELINE_STYLES_URI: &str = "{9260A510-F301-46a8-8635-F512D64BE5F5}";
pub(crate) const TIMELINE_STYLE_DXFS_URI: &str = "{A0A4C193-F2C1-4fcb-8827-314CF55A85BB}";
/// The `uri`s of the stylesheet extensions of the timeline styles.
pub(crate) const TIMELINE_STYLE_EXTENSIONS: &[&str] =
    &[TIMELINE_STYLES_URI, TIMELINE_STYLE_DXFS_URI];
const DEFAULT_TIMELINE_STYLE: &str = "TimeSlicerStyleLight1";
const STYLE_ELEMENT_TYPES: [&str; 7] = [
    "selectionLabel",
    "timeLevel",
    "periodLabel1",
    "periodLabel2",
    "selectedTimeBlock",
    "unselectedTimeBlock",
    "selectedTimeBlockSpace",
];

fn string(node: &OpaqueElement, name: &str) -> Result<String, String> {
    Ok(crate::helper::utils::unescape_xstring_owned(required(
        node, name,
    )?))
}

fn attributes(node: &OpaqueElement, allowed: &[&str]) -> Result<(), String> {
    let qualified: &[(&str, &str)] = if matches!(
        node.name.local_name.as_str(),
        "timeline" | "timelineCacheDefinition"
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

fn uid_of(node: &OpaqueElement) -> Option<String> {
    node.attributes
        .iter()
        .find(|(name, _)| name.is(REVISION_NS, "uid"))
        .map(|(_, value)| value.clone())
}

fn extension_list(node: &OpaqueElement) -> Result<OpaqueElement, String> {
    expect(node, SLICER_2013_NS, "extLst")?;
    Ok(node.clone())
}

fn range(node: &OpaqueElement) -> Result<TimelineRange, String> {
    attributes(node, &["startDate", "endDate"])?;
    no_children(node)?;
    let value = TimelineRange {
        start_date: required(node, "startDate")?,
        end_date:   required(node, "endDate")?,
    };
    value.start()?;
    value.end()?;
    Ok(value)
}

fn pivot_tables(node: &OpaqueElement) -> Result<Vec<SlicerPivotTable>, String> {
    attributes(node, &[])?;
    let values = direct_children(node)?
        .iter()
        .map(|child| {
            expect(child, SLICER_2013_NS, "pivotTable")?;
            attributes(child, &["tabId", "name"])?;
            no_children(child)?;
            Ok(SlicerPivotTable {
                sheet_id: required_unsigned(child, "tabId")?,
                name:     string(child, "name")?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    if values.is_empty() {
        return Err("empty timeline pivot-table list".into());
    }
    Ok(values)
}

fn state(node: &OpaqueElement) -> Result<TimelineState, String> {
    attributes(
        node,
        &[
            "singleRangeFilterState",
            "minimalRefreshVersion",
            "lastRefreshVersion",
            "pivotCacheId",
            "filterType",
        ],
    )?;
    let filter_type = required(node, "filterType")?;
    if !crate::structs::pivot_table_definition::is_pivot_filter_type(&filter_type) {
        return Err(format!("invalid timeline filter type '{filter_type}'"));
    }
    let mut selection = None;
    let mut bounds = None;
    let mut extensions = None;
    for child in direct_children(node)? {
        if child.name.is(SLICER_2013_NS, "selection") {
            unique(&mut selection, range(&child)?, "timeline selection")?;
        } else if child.name.is(SLICER_2013_NS, "bounds") {
            unique(&mut bounds, range(&child)?, "timeline bounds")?;
        } else if child.name.is(SLICER_2013_NS, "extLst") {
            unique(
                &mut extensions,
                extension_list(&child)?,
                "timeline state extensions",
            )?;
        } else {
            return Err("unsupported timeline state child".into());
        }
    }
    Ok(TimelineState {
        single_range_filter_state: boolean(node, "singleRangeFilterState")?,
        minimal_refresh_version: required_unsigned(node, "minimalRefreshVersion")?,
        last_refresh_version: required_unsigned(node, "lastRefreshVersion")?,
        pivot_cache_id: required_unsigned(node, "pivotCacheId")?,
        filter_type,
        selection,
        bounds: bounds.ok_or("missing timeline bounds")?,
        extensions,
    })
}

fn pivot_filter(node: &OpaqueElement) -> Result<TimelinePivotFilter, String> {
    attributes(node, &["useWholeDay", "fld", "id", "name", "description"])?;
    let mut auto_filter = None;
    for child in direct_children(node)? {
        if child.name.is(SHEET_MAIN_NS, "autoFilter") || child.name.is(SLICER_2013_NS, "autoFilter")
        {
            unique(&mut auto_filter, child, "timeline pivot filter autoFilter")?;
        } else {
            return Err("unsupported timeline pivot filter child".into());
        }
    }
    Ok(TimelinePivotFilter {
        use_whole_day: boolean(node, "useWholeDay")?,
        field: required_unsigned(node, "fld")?,
        id: required_unsigned(node, "id")?,
        name: optional_xstring_attribute(node, "name"),
        description: optional_xstring_attribute(node, "description"),
        auto_filter,
    })
}

fn cache(node: &OpaqueElement) -> Result<TimelineCache, String> {
    expect(node, SLICER_2013_NS, "timelineCacheDefinition")?;
    attributes(node, &["name", "sourceName"])?;
    let mut pivots = None;
    let mut timeline_state = None;
    let mut filter = None;
    let mut extensions = None;
    for child in direct_children(node)? {
        if child.name.is(SLICER_2013_NS, "pivotTables") {
            unique(&mut pivots, pivot_tables(&child)?, "timeline pivot tables")?;
        } else if child.name.is(SLICER_2013_NS, "state") {
            unique(&mut timeline_state, state(&child)?, "timeline state")?;
        } else if child.name.is(SLICER_2013_NS, "timelinePivotFilter") {
            unique(&mut filter, pivot_filter(&child)?, "timeline pivot filter")?;
        } else if child.name.is(SLICER_2013_NS, "extLst") {
            unique(
                &mut extensions,
                extension_list(&child)?,
                "timeline cache extensions",
            )?;
        } else {
            return Err("unsupported timeline cache child".into());
        }
    }
    let value = TimelineCache {
        name: string(node, "name")?,
        source_name: string(node, "sourceName")?,
        uid: uid_of(node),
        pivot_tables: pivots,
        state: timeline_state.ok_or("missing timeline cache state")?,
        pivot_filter: filter,
        extensions,
    };
    value.validate()?;
    Ok(value)
}

fn level(node: &OpaqueElement, name: &str) -> Result<TimelineLevel, String> {
    let index = required_unsigned(node, name)?;
    TimelineLevel::from_index(index).ok_or_else(|| format!("invalid timeline {name} {index}"))
}

impl TimelineCache {
    /// # Errors
    /// Returns an error when the part is not a valid timeline cache.
    pub fn from_xml(bytes: &[u8]) -> Result<Self, String> {
        cache(&root(bytes)?)
    }
}

impl Timeline {
    /// # Errors
    /// Returns an error when the part is not a valid timeline list.
    pub fn from_xml(bytes: &[u8]) -> Result<Vec<Self>, String> {
        let node = root(bytes)?;
        expect(&node, SLICER_2013_NS, "timelines")?;
        attributes(&node, &[])?;
        let values = direct_children(&node)?
            .iter()
            .map(Self::from_element)
            .collect::<Result<Vec<_>, _>>()?;
        if values.is_empty() {
            return Err("empty timeline view list".into());
        }
        Ok(values)
    }

    fn from_element(node: &OpaqueElement) -> Result<Self, String> {
        expect(node, SLICER_2013_NS, "timeline")?;
        attributes(
            node,
            &[
                "name",
                "cache",
                "caption",
                "showHeader",
                "showSelectionLabel",
                "showTimeLevel",
                "showHorizontalScrollbar",
                "level",
                "selectionLevel",
                "scrollPosition",
                "style",
            ],
        )?;
        let scroll_position = attr(node, "scrollPosition").map(str::to_owned);
        if let Some(value) = &scroll_position {
            parse_date_time(value)?;
        }
        let mut value = Self {
            name: string(node, "name")?,
            cache: string(node, "cache")?,
            uid: uid_of(node),
            caption: optional_xstring_attribute(node, "caption"),
            show_header: boolean(node, "showHeader")?,
            show_selection_label: boolean(node, "showSelectionLabel")?,
            show_time_level: boolean(node, "showTimeLevel")?,
            show_horizontal_scrollbar: boolean(node, "showHorizontalScrollbar")?,
            level: level(node, "level")?,
            selection_level: level(node, "selectionLevel")?,
            scroll_position,
            style: optional_xstring_attribute(node, "style"),
            extensions: None,
        };
        for child in direct_children(node)? {
            unique(
                &mut value.extensions,
                extension_list(&child)?,
                "timeline view extensions",
            )?;
        }
        value.validate()?;
        Ok(value)
    }
}

fn scope() -> NamespaceScope {
    NamespaceScope::with_bindings([
        (None, SLICER_2013_NS.into()),
        (Some("x".into()), SHEET_MAIN_NS.into()),
        (Some("x15".into()), SLICER_2013_NS.into()),
        (Some("xr10".into()), REVISION_NS.into()),
        (Some("mc".into()), MC_NS.into()),
    ])
}

fn flag(attributes: &mut Vec<(&'static str, String)>, name: &'static str, value: Option<bool>) {
    if let Some(value) = value {
        attributes.push((name, if value { "1" } else { "0" }.into()));
    }
}

fn write_range(writer: &mut Writer<Cursor<Vec<u8>>>, name: &str, value: &TimelineRange) {
    start(
        writer,
        name,
        vec![
            ("startDate", value.start_date.clone()),
            ("endDate", value.end_date.clone()),
        ],
        true,
    );
}

fn write_extensions(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    extensions: Option<&OpaqueElement>,
) -> Result<(), String> {
    if let Some(extension) = extensions {
        extension.write_to(writer, &scope())?;
    }
    Ok(())
}

impl TimelineCache {
    /// # Errors
    /// Returns an error when the cache is not valid.
    pub fn to_xml(&self) -> Result<Vec<u8>, String> {
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        self.write_to(&mut writer)?;
        Ok(writer.into_inner().into_inner())
    }

    fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>) -> Result<(), String> {
        self.validate()?;
        let mut attributes = vec![
            ("xmlns", SLICER_2013_NS.to_owned()),
            ("xmlns:x15", SLICER_2013_NS.to_owned()),
            ("xmlns:mc", MC_NS.to_owned()),
            ("mc:Ignorable", "xr10".to_owned()),
            ("xmlns:xr10", REVISION_NS.to_owned()),
            ("name", encode_xstring(&self.name)),
        ];
        if let Some(uid) = &self.uid {
            attributes.push(("xr10:uid", uid.clone()));
        }
        attributes.push(("sourceName", encode_xstring(&self.source_name)));
        start(writer, "timelineCacheDefinition", attributes, false);
        if let Some(pivots) = &self.pivot_tables {
            start(writer, "pivotTables", Vec::new(), false);
            for pivot in pivots {
                start(
                    writer,
                    "pivotTable",
                    vec![
                        ("tabId", pivot.sheet_id.to_string()),
                        ("name", encode_xstring(&pivot.name)),
                    ],
                    true,
                );
            }
            write_end_tag(writer, "pivotTables");
        }
        let state = &self.state;
        let mut attributes = Vec::new();
        flag(
            &mut attributes,
            "singleRangeFilterState",
            state.single_range_filter_state,
        );
        attributes.extend([
            (
                "minimalRefreshVersion",
                state.minimal_refresh_version.to_string(),
            ),
            ("lastRefreshVersion", state.last_refresh_version.to_string()),
            ("pivotCacheId", state.pivot_cache_id.to_string()),
            ("filterType", state.filter_type.clone()),
        ]);
        start(writer, "state", attributes, false);
        if let Some(selection) = &state.selection {
            write_range(writer, "selection", selection);
        }
        write_range(writer, "bounds", &state.bounds);
        write_extensions(writer, state.extensions.as_ref())?;
        write_end_tag(writer, "state");
        if let Some(filter) = &self.pivot_filter {
            let mut attributes = Vec::new();
            flag(&mut attributes, "useWholeDay", filter.use_whole_day);
            attributes.push(("fld", filter.field.to_string()));
            attributes.push(("id", filter.id.to_string()));
            if let Some(name) = &filter.name {
                attributes.push(("name", encode_xstring(name)));
            }
            if let Some(description) = &filter.description {
                attributes.push(("description", encode_xstring(description)));
            }
            start(
                writer,
                "timelinePivotFilter",
                attributes,
                filter.auto_filter.is_none(),
            );
            if let Some(auto_filter) = &filter.auto_filter {
                auto_filter.write_to(writer, &scope())?;
                write_end_tag(writer, "timelinePivotFilter");
            }
        }
        write_extensions(writer, self.extensions.as_ref())?;
        write_end_tag(writer, "timelineCacheDefinition");
        Ok(())
    }
}

impl Timeline {
    /// # Errors
    /// Returns an error when a view is not valid or the list is empty.
    pub fn to_xml(values: &[Self]) -> Result<Vec<u8>, String> {
        if values.is_empty() {
            return Err("empty timeline view list".into());
        }
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        start(
            &mut writer,
            "timelines",
            vec![
                ("xmlns", SLICER_2013_NS.to_owned()),
                ("xmlns:mc", MC_NS.to_owned()),
                ("mc:Ignorable", "x xr10".to_owned()),
                ("xmlns:x", SHEET_MAIN_NS.to_owned()),
                ("xmlns:xr10", REVISION_NS.to_owned()),
            ],
            false,
        );
        for value in values {
            value.validate()?;
            let mut attributes = vec![("name", encode_xstring(&value.name))];
            if let Some(uid) = &value.uid {
                attributes.push(("xr10:uid", uid.clone()));
            }
            attributes.push(("cache", encode_xstring(&value.cache)));
            if let Some(caption) = &value.caption {
                attributes.push(("caption", encode_xstring(caption)));
            }
            flag(&mut attributes, "showHeader", value.show_header);
            flag(
                &mut attributes,
                "showSelectionLabel",
                value.show_selection_label,
            );
            flag(&mut attributes, "showTimeLevel", value.show_time_level);
            flag(
                &mut attributes,
                "showHorizontalScrollbar",
                value.show_horizontal_scrollbar,
            );
            attributes.push(("level", value.level.index().to_string()));
            attributes.push(("selectionLevel", value.selection_level.index().to_string()));
            if let Some(position) = &value.scroll_position {
                attributes.push(("scrollPosition", position.clone()));
            }
            if let Some(style) = &value.style {
                attributes.push(("style", encode_xstring(style)));
            }
            start(
                &mut writer,
                "timeline",
                attributes,
                value.extensions.is_none(),
            );
            if value.extensions.is_some() {
                write_extensions(&mut writer, value.extensions.as_ref())?;
                write_end_tag(&mut writer, "timeline");
            }
        }
        write_end_tag(&mut writer, "timelines");
        Ok(writer.into_inner().into_inner())
    }
}

impl TimelineStyles {
    /// The timeline styles of the stylesheet `ext` entries of a timeline style
    /// `uri`, `None` when there are none.
    pub(crate) fn read_extensions(extensions: &[OpaqueElement]) -> Result<Option<Self>, String> {
        let mut styles = None;
        let mut dxfs = None;
        for extension in extensions {
            expect(extension, SHEET_MAIN_NS, "ext")?;
            let mut payloads = direct_children(extension)?;
            if payloads.len() != 1 {
                return Err("a timeline style extension needs exactly one element".into());
            }
            let payload = payloads.remove(0);
            if attr(extension, "uri")
                .is_some_and(|uri| uri.eq_ignore_ascii_case(TIMELINE_STYLES_URI))
            {
                let mut value = Self::default();
                value.read_styles_payload(&payload)?;
                if styles.replace(value).is_some() {
                    return Err("duplicate timeline styles extension".into());
                }
            } else {
                expect(&payload, SLICER_2013_NS, "dxfs")?;
                for format in direct_children(&payload)? {
                    expect(&format, SHEET_MAIN_NS, "dxf")?;
                }
                if dxfs.replace(payload).is_some() {
                    return Err("duplicate timeline style differential formats".into());
                }
            }
        }
        if styles.is_none() && dxfs.is_none() {
            return Ok(None);
        }
        let mut value = styles.unwrap_or_default();
        value.dxfs = dxfs;
        value.validate_dxf_references()?;
        Ok(Some(value))
    }

    fn validate_dxf_references(&self) -> Result<(), String> {
        let dxf_count = match &self.dxfs {
            Some(dxfs) => direct_children(dxfs)?.len(),
            None => 0,
        };
        for style in &self.styles {
            for (kind, dxf_id) in &style.elements {
                if let Some(dxf_id) = dxf_id {
                    if *dxf_id as usize >= dxf_count {
                        return Err(format!(
                            "timeline style {} element {kind} references dxfId {dxf_id}, but \
                             x15:dxfs has {dxf_count} entries",
                            style.name
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    /// Read the `x15:timelineStyles` payload of its stylesheet extension.
    fn read_styles_payload(&mut self, payload: &OpaqueElement) -> Result<(), String> {
        expect(payload, SLICER_2013_NS, "timelineStyles")?;
        attributes(payload, &["defaultTimelineStyle"])?;
        self.default_style = required(payload, "defaultTimelineStyle")?;
        for child in direct_children(payload)? {
            expect(&child, SLICER_2013_NS, "timelineStyle")?;
            attributes(&child, &["name"])?;
            let mut style = TimelineStyle {
                name:     required(&child, "name")?,
                elements: Vec::new(),
            };
            for group in direct_children(&child)? {
                expect(&group, SLICER_2013_NS, "timelineStyleElements")?;
                attributes(&group, &[])?;
                for element in direct_children(&group)? {
                    expect(&element, SLICER_2013_NS, "timelineStyleElement")?;
                    attributes(&element, &["type", "dxfId"])?;
                    no_children(&element)?;
                    let kind = required(&element, "type")?;
                    if !STYLE_ELEMENT_TYPES.contains(&kind.as_str()) {
                        return Err(format!("invalid timeline style element type '{kind}'"));
                    }
                    style.elements.push((kind, unsigned(&element, "dxfId")?));
                }
                if style.elements.is_empty() {
                    return Err("empty timeline style element list".into());
                }
            }
            self.styles.push(style);
        }
        Ok(())
    }

    /// Write the `ext` entries of the timeline styles and their differential
    /// formats.
    pub(crate) fn write_extensions(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
    ) -> Result<(), String> {
        if let Some(dxfs) = &self.dxfs {
            start(
                writer,
                "ext",
                vec![
                    ("uri", TIMELINE_STYLE_DXFS_URI.to_owned()),
                    ("xmlns:x15", SLICER_2013_NS.to_owned()),
                ],
                false,
            );
            dxfs.write_to(
                writer,
                &NamespaceScope::with_bindings([
                    (None, SHEET_MAIN_NS.into()),
                    (Some("x15".into()), SLICER_2013_NS.into()),
                ]),
            )?;
            write_end_tag(writer, "ext");
        }
        start(
            writer,
            "ext",
            vec![
                ("uri", TIMELINE_STYLES_URI.to_owned()),
                ("xmlns:x15", SLICER_2013_NS.to_owned()),
            ],
            false,
        );
        start(
            writer,
            "x15:timelineStyles",
            vec![(
                "defaultTimelineStyle",
                if self.default_style.is_empty() {
                    DEFAULT_TIMELINE_STYLE
                } else {
                    &self.default_style
                }
                .to_owned(),
            )],
            self.styles.is_empty(),
        );
        if !self.styles.is_empty() {
            for style in &self.styles {
                start(
                    writer,
                    "x15:timelineStyle",
                    vec![("name", style.name.clone())],
                    style.elements.is_empty(),
                );
                if !style.elements.is_empty() {
                    start(writer, "x15:timelineStyleElements", Vec::new(), false);
                    for (kind, dxf) in &style.elements {
                        let mut attributes = vec![("type", kind.clone())];
                        if let Some(dxf) = dxf {
                            attributes.push(("dxfId", dxf.to_string()));
                        }
                        start(writer, "x15:timelineStyleElement", attributes, true);
                    }
                    write_end_tag(writer, "x15:timelineStyleElements");
                    write_end_tag(writer, "x15:timelineStyle");
                }
            }
            write_end_tag(writer, "x15:timelineStyles");
        }
        write_end_tag(writer, "ext");
        Ok(())
    }
}
