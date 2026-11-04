//! Package wiring for native slicers: extension lists, part references and
//! identity-based binding checks.
//!
//! Slicer caches are workbook parts listed in `x14:slicerCaches` (pivot
//! caches) or `x15:slicerCaches` (table slicers). Slicer views are worksheet
//! parts listed in an `x14:slicerList` under the x14 or the x15 extension URI.
//! Every entry is resolved through the relationship of its exact owner part.

use std::collections::HashSet;

use super::{
    SLICER_2013_NS,
    SLICER_NS,
    model::{
        SlicerCache,
        SlicerCacheExtension,
        SlicerPivotTable,
        SlicerSource,
    },
};
use crate::{
    drawing::{
        charts::{
            NamespaceScope,
            OpaqueElement,
        },
        spreadsheet::DrawingObjectKey,
    },
    helper::const_str::{
        REL_OFC_NS,
        SHEET_MAIN_NS,
    },
    structs::{
        Workbook,
        Worksheet,
        XlsxError,
        package_extension::PreservedExtension,
    },
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
    xml::{
        attributes,
        direct_children,
        expect,
        no_children,
        required_attribute,
        xml_error,
    },
};

const WORKBOOK_PIVOT_URI: &str = "{BBE1A952-AA13-448e-AADC-164F8A28A991}";
const WORKBOOK_TABLE_URI: &str = "{46BE6895-7355-4a93-B00E-2C351335B9C9}";
const WORKSHEET_PIVOT_URI: &str = "{A8765BA9-456A-4dab-B4F3-ACF838C121DE}";
const WORKSHEET_TABLE_URI: &str = "{3A4CF648-6AED-40f4-86FF-DC5316D8AED3}";
const WORKBOOK_TIMELINE_URI: &str = "{D0CA8CA8-9F24-4464-BF8E-62219DCF47F9}";
const SLICER_PIVOT_CACHES_URI: &str = "{876F7934-8845-4945-9796-88D515C7AA90}";
const TIMELINE_PIVOT_CACHES_URI: &str = "{A2CB5862-8E78-49c6-8D9D-AF26E26ADB89}";
const WORKSHEET_TIMELINE_URI: &str = "{7E03D99C-DC04-49d9-9315-930204A7B6E9}";
/// The worksheet `x14:dataValidations` extension.
pub(crate) const DATA_VALIDATIONS_URI: &str = "{CCE6A557-97BC-4b89-ADB6-D9C93CAAB3DF}";

/// Which extension list (and therefore which part kind) an entry belongs to.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum SlicerListKind {
    /// `x14` lists: slicers over pivot caches.
    Pivot,
    /// `x15` lists: slicers over tables.
    Table,
    /// `x15` lists: timelines over pivot caches.
    Timeline,
}

impl SlicerListKind {
    pub(crate) fn of_cache(cache: &SlicerCache) -> Self {
        match cache.source {
            Some(SlicerSource::Table(_)) => Self::Table,
            _ => Self::Pivot,
        }
    }
}

/// A relationship id found in an extension list, resolved later through the
/// owner part's own relationships.
#[derive(Clone, Debug)]
pub(crate) struct SlicerPartReference {
    pub(crate) kind:            SlicerListKind,
    pub(crate) relationship_id: String,
}

/// The workbook extension that declares the pivot caches kept without a pivot
/// table. Excel writes `x14:pivotCaches` for a cache a slicer reads, also when
/// a timeline reads it too, and `x15:timelineCachePivotCaches` for one only
/// timelines read.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RetainedPivotCacheList {
    Slicer,
    Timeline,
}

impl RetainedPivotCacheList {
    const ALL: [Self; 2] = [Self::Slicer, Self::Timeline];

    fn uri(self) -> &'static str {
        match self {
            Self::Slicer => SLICER_PIVOT_CACHES_URI,
            Self::Timeline => TIMELINE_PIVOT_CACHES_URI,
        }
    }

    /// The prefix, namespace and local name of the list element.
    fn list(self) -> (&'static str, &'static str, &'static str) {
        match self {
            Self::Slicer => ("x14", SLICER_NS, "pivotCaches"),
            Self::Timeline => ("x15", SLICER_2013_NS, "timelineCachePivotCaches"),
        }
    }
}

/// An entry of the workbook extension list, in source order.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub(crate) enum WorkbookExtension {
    SlicerCaches(SlicerListKind),
    /// The retained pivot caches as read: workbook cache id and relationship
    /// id. The writer lists the caches the workbook retains when it writes.
    RetainedPivotCaches(RetainedPivotCacheList, Vec<(u32, String)>),
    Other(PreservedExtension),
}

/// An entry of a worksheet extension list, in source order.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub(crate) enum WorksheetExtension {
    /// The `x14:dataValidations` extension, modelled by the worksheet.
    DataValidations,
    SlicerList(SlicerListKind),
    Other(PreservedExtension),
}

/// Which slicer lists a worksheet needs, decided by the kinds of caches its
/// slicers bind to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct SlicerKinds {
    pub(crate) pivot:    bool,
    pub(crate) table:    bool,
    pub(crate) timeline: bool,
}

/// Relationship ids written into a worksheet's extension lists, by the
/// relationship ids the writer allocated.
#[derive(Clone, Debug, Default)]
pub(crate) struct WorksheetExtensionIds {
    pub(crate) pivot_slicers: Option<String>,
    pub(crate) table_slicers: Option<String>,
    pub(crate) timelines:     Option<String>,
    /// One entry per `Other` extension, in order: the new ids of its parts.
    pub(crate) carried:       Vec<Vec<String>>,
}

/// Whether `uri` names the worksheet data validations extension, which the
/// worksheet models itself instead of keeping it as an opaque entry.
#[must_use]
pub(crate) fn is_data_validations_uri(uri: &[u8]) -> bool {
    DATA_VALIDATIONS_URI.as_bytes().eq_ignore_ascii_case(uri)
}

fn uri_is(extension: &OpaqueElement, expected: &str) -> bool {
    crate::xml::attr(extension, "uri").is_some_and(|uri| uri.eq_ignore_ascii_case(expected))
}

fn list_entries(
    list: &OpaqueElement,
    list_namespace: &str,
    list_name: &str,
    entry_namespace: &str,
    entry_name: &str,
    kind: SlicerListKind,
    exactly_one: bool,
) -> Result<Vec<SlicerPartReference>, String> {
    expect(list, list_namespace, list_name)?;
    attributes(list, &[], &[])?;
    let entries = direct_children(list)?;
    if entries.is_empty() {
        return Err(format!("empty {list_name} extension"));
    }
    if exactly_one && entries.len() != 1 {
        return Err(format!(
            "{list_name} extension must name exactly one part, found {}",
            entries.len()
        ));
    }
    entries
        .iter()
        .map(|entry| {
            expect(entry, entry_namespace, entry_name)?;
            attributes(entry, &[], &[(REL_OFC_NS, "id")])?;
            no_children(entry)?;
            let id = required_attribute(entry, REL_OFC_NS, "id")?;
            if id.is_empty() {
                return Err(format!("empty {entry_name} relationship id"));
            }
            Ok(SlicerPartReference {
                kind,
                relationship_id: id.to_owned(),
            })
        })
        .collect()
}

/// Whether a timeline extension lists nothing: Excel leaves such an empty
/// extension (or an empty list) behind when the last timeline is deleted. It
/// is kept as it is instead of being read as a list of timelines.
fn lists_no_timelines(extension: &OpaqueElement) -> Result<bool, String> {
    attributes(extension, &["uri"], &[])?;
    let children = direct_children(extension)?;
    match children.as_slice() {
        [] => Ok(true),
        [list] => Ok(direct_children(list)?.is_empty()),
        _ => Ok(false),
    }
}

fn single_list_child(extension: &OpaqueElement) -> Result<OpaqueElement, String> {
    attributes(extension, &["uri"], &[])?;
    let mut children = direct_children(extension)?;
    if children.len() != 1 {
        return Err("slicer extension needs exactly one list".into());
    }
    Ok(children.remove(0))
}

/// The `pivotCache` entries (workbook cache id, relationship id) of a retained
/// pivot cache list.
fn retained_pivot_caches(
    extension: &OpaqueElement,
    kind: RetainedPivotCacheList,
) -> Result<Vec<(u32, String)>, String> {
    let (_, namespace, name) = kind.list();
    let list = single_list_child(extension)?;
    expect(&list, namespace, name)?;
    attributes(&list, &[], &[])?;
    let mut caches = Vec::new();
    let mut ids = HashSet::new();
    let mut relationships = HashSet::new();
    for cache in direct_children(&list)? {
        expect(&cache, SHEET_MAIN_NS, "pivotCache")?;
        attributes(&cache, &["cacheId"], &[(REL_OFC_NS, "id")])?;
        no_children(&cache)?;
        let id = crate::xml::required_unsigned(&cache, "cacheId")?;
        let relationship = required_attribute(&cache, REL_OFC_NS, "id")?.to_owned();
        if relationship.is_empty() || !ids.insert(id) || !relationships.insert(relationship.clone())
        {
            return Err(format!("invalid or duplicate {name} pivot cache identity"));
        }
        caches.push((id, relationship));
    }
    if caches.is_empty() {
        return Err(format!("empty {name} extension"));
    }
    Ok(caches)
}

/// Classify the workbook `ext` entries. Slicer cache lists become references;
/// every other entry is kept verbatim and in order.
pub(crate) fn read_workbook_extensions(
    entries: &[OpaqueElement],
) -> Result<(Vec<WorkbookExtension>, Vec<SlicerPartReference>), String> {
    let mut extensions = Vec::new();
    let mut references = Vec::new();
    let mut seen = HashSet::new();
    for entry in entries {
        expect(entry, SHEET_MAIN_NS, "ext")?;
        if let Some(kind) = RetainedPivotCacheList::ALL
            .into_iter()
            .find(|kind| uri_is(entry, kind.uri()))
        {
            if extensions.iter().any(|entry| {
                matches!(entry, WorkbookExtension::RetainedPivotCaches(listed, _) if *listed == kind)
            }) {
                return Err(format!("duplicate {} extension", kind.list().2));
            }
            extensions.push(WorkbookExtension::RetainedPivotCaches(
                kind,
                retained_pivot_caches(entry, kind)?,
            ));
            continue;
        }
        let kind = if uri_is(entry, WORKBOOK_PIVOT_URI) {
            Some((
                SlicerListKind::Pivot,
                SLICER_NS,
                "x14:slicerCaches extension",
                ("slicerCaches", SLICER_NS, "slicerCache"),
            ))
        } else if uri_is(entry, WORKBOOK_TABLE_URI) {
            Some((
                SlicerListKind::Table,
                SLICER_2013_NS,
                "x15:slicerCaches extension",
                ("slicerCaches", SLICER_NS, "slicerCache"),
            ))
        } else if uri_is(entry, WORKBOOK_TIMELINE_URI) {
            Some((
                SlicerListKind::Timeline,
                SLICER_2013_NS,
                "x15:timelineCacheRefs extension",
                ("timelineCacheRefs", SLICER_2013_NS, "timelineCacheRef"),
            ))
        } else {
            None
        };
        let Some((kind, namespace, label, (list_name, entry_namespace, entry_name))) = kind else {
            extensions.push(WorkbookExtension::Other(PreservedExtension::new(
                entry.clone(),
            )));
            continue;
        };
        if kind == SlicerListKind::Timeline && lists_no_timelines(entry)? {
            extensions.push(WorkbookExtension::Other(PreservedExtension::new(
                entry.clone(),
            )));
            continue;
        }
        if !seen.insert(kind) {
            return Err(format!("duplicate {label}"));
        }
        let list = single_list_child(entry)?;
        references.extend(list_entries(
            &list,
            namespace,
            list_name,
            entry_namespace,
            entry_name,
            kind,
            false,
        )?);
        extensions.push(WorkbookExtension::SlicerCaches(kind));
    }
    Ok((extensions, references))
}

/// Read one worksheet `ext` into the entries of the worksheet. A slicer list
/// must name exactly one part and may appear once per kind; every other
/// extension is kept verbatim.
pub(crate) fn read_worksheet_extension(
    extensions: &mut Vec<WorksheetExtension>,
    references: &mut Vec<SlicerPartReference>,
    extension: OpaqueElement,
) -> Result<(), String> {
    expect(&extension, SHEET_MAIN_NS, "ext")?;
    // Excel writes the list element in the x14 namespace under both extension
    // URIs; the URI only names the kind of slicers listed.
    let (kind, namespace) = if uri_is(&extension, WORKSHEET_PIVOT_URI) {
        (SlicerListKind::Pivot, SLICER_NS)
    } else if uri_is(&extension, WORKSHEET_TABLE_URI) {
        (SlicerListKind::Table, SLICER_NS)
    } else if uri_is(&extension, WORKSHEET_TIMELINE_URI) {
        (SlicerListKind::Timeline, SLICER_2013_NS)
    } else if uri_is(&extension, DATA_VALIDATIONS_URI) {
        extensions.push(WorksheetExtension::DataValidations);
        return Ok(());
    } else {
        extensions.push(WorksheetExtension::Other(PreservedExtension::new(
            extension,
        )));
        return Ok(());
    };
    if kind == SlicerListKind::Timeline && lists_no_timelines(&extension)? {
        extensions.push(WorksheetExtension::Other(PreservedExtension::new(
            extension,
        )));
        return Ok(());
    }
    if extensions
        .iter()
        .any(|entry| matches!(entry, WorksheetExtension::SlicerList(listed) if *listed == kind))
    {
        return Err(format!("worksheet lists {kind:?} slicers more than once"));
    }
    let list = single_list_child(&extension)?;
    let (list_name, entry_namespace, entry_name) = if kind == SlicerListKind::Timeline {
        ("timelineRefs", SLICER_2013_NS, "timelineRef")
    } else {
        ("slicerList", SLICER_NS, "slicer")
    };
    references.extend(list_entries(
        &list,
        namespace,
        list_name,
        entry_namespace,
        entry_name,
        kind,
        true,
    )?);
    extensions.push(WorksheetExtension::SlicerList(kind));
    Ok(())
}

fn write_ids(
    writer: &mut quick_xml::Writer<std::io::Cursor<Vec<u8>>>,
    kind: SlicerListKind,
    ids: &[String],
    list: &str,
    entry: &str,
) {
    // Workbook table slicer caches are listed by an x15 element, the worksheet
    // slicer lists by an x14 element under both extension URIs. Timelines are
    // listed by x15 elements throughout.
    let workbook_list = list == "slicerCaches";
    let (uri, root_prefix, root_namespace, root_name, entry_name) = match kind {
        SlicerListKind::Pivot => (
            if workbook_list {
                WORKBOOK_PIVOT_URI
            } else {
                WORKSHEET_PIVOT_URI
            },
            "x14",
            SLICER_NS,
            format!("x14:{list}"),
            format!("x14:{entry}"),
        ),
        SlicerListKind::Table if workbook_list => (
            WORKBOOK_TABLE_URI,
            "x15",
            SLICER_2013_NS,
            format!("x15:{list}"),
            format!("x14:{entry}"),
        ),
        SlicerListKind::Table => (
            WORKSHEET_TABLE_URI,
            "x15",
            SLICER_2013_NS,
            format!("x14:{list}"),
            format!("x14:{entry}"),
        ),
        SlicerListKind::Timeline => (
            if list == "timelineCacheRefs" {
                WORKBOOK_TIMELINE_URI
            } else {
                WORKSHEET_TIMELINE_URI
            },
            "x15",
            SLICER_2013_NS,
            format!("x15:{list}"),
            format!("x15:{entry}"),
        ),
    };
    write_start_tag(
        writer,
        "ext",
        vec![
            ("uri", uri).into(),
            (format!("xmlns:{root_prefix}").as_str(), root_namespace).into(),
        ],
        false,
    );
    let mut list_attributes: crate::structs::AttrCollection = Vec::new();
    if kind == SlicerListKind::Table {
        list_attributes.push(("xmlns:x14", SLICER_NS).into());
    }
    write_start_tag(writer, root_name.as_str(), list_attributes, false);
    for id in ids {
        write_start_tag(
            writer,
            entry_name.as_str(),
            vec![("r:id", id.as_str()).into()],
            true,
        );
    }
    write_end_tag(writer, root_name.as_str());
    write_end_tag(writer, "ext");
}

/// The namespace declarations of the worksheet root element, which the
/// extension entries written inside it can rely on.
pub(crate) fn worksheet_extension_scope() -> NamespaceScope {
    use crate::helper::const_str::{
        MC_NS,
        SHEET_DRAWING_NS,
        SHEET_MS_MAIN_NS,
        SHEETML_AC_NS,
    };
    NamespaceScope::with_bindings(
        [
            (None, SHEET_MAIN_NS),
            (Some("r"), REL_OFC_NS),
            (Some("xdr"), SHEET_DRAWING_NS),
            (Some("x14"), SHEET_MS_MAIN_NS),
            (Some("mc"), MC_NS),
            (Some("x14ac"), SHEETML_AC_NS),
        ]
        .map(|(prefix, uri): (Option<&str>, &str)| (prefix.map(str::to_owned), uri.to_owned())),
    )
}

/// The namespace declarations of the workbook root element.
pub(crate) fn workbook_extension_scope() -> NamespaceScope {
    use crate::helper::const_str::{
        MC_NS,
        SHEET_MS_REVISION_NS,
        X15_NS,
        XR2_NS,
        XR6_NS,
        XR10_NS,
    };
    NamespaceScope::with_bindings(
        [
            (None, SHEET_MAIN_NS),
            (Some("r"), REL_OFC_NS),
            (Some("mc"), MC_NS),
            (Some("x15"), X15_NS),
            (Some("xr"), SHEET_MS_REVISION_NS),
            (Some("xr2"), XR2_NS),
            (Some("xr6"), XR6_NS),
            (Some("xr10"), XR10_NS),
        ]
        .map(|(prefix, uri): (Option<&str>, &str)| (prefix.map(str::to_owned), uri.to_owned())),
    )
}

fn write_preserved(
    writer: &mut quick_xml::Writer<std::io::Cursor<Vec<u8>>>,
    extension: &PreservedExtension,
    new_ids: &[String],
    scope: &NamespaceScope,
) -> Result<(), XlsxError> {
    let element = extension.rebound(new_ids).map_err(xml_error)?;
    element.write_to(writer, scope).map_err(xml_error)
}

/// Write the workbook `extLst`: the source entries in their original order,
/// with each slicer cache list replaced by the caches actually written and
/// each retained pivot cache list by the caches `retained_ids` puts in it.
/// `carried_ids` holds, for every preserved entry in order, the relationship
/// ids allocated for the parts it names.
pub(crate) fn write_workbook_extensions(
    writer: &mut quick_xml::Writer<std::io::Cursor<Vec<u8>>>,
    extensions: &[WorkbookExtension],
    pivot_ids: &[String],
    table_ids: &[String],
    timeline_ids: &[String],
    retained_ids: &[(RetainedPivotCacheList, u32, String)],
    carried_ids: &[Vec<String>],
) -> Result<(), XlsxError> {
    let ids_of = |kind| match kind {
        SlicerListKind::Pivot => pivot_ids,
        SlicerListKind::Table => table_ids,
        SlicerListKind::Timeline => timeline_ids,
    };
    let retained_in = |kind| retained_ids.iter().filter(move |(list, ..)| *list == kind);
    let listed = |kind| {
        extensions.iter().any(
            |entry| matches!(entry, WorkbookExtension::SlicerCaches(listed) if *listed == kind),
        )
    };
    let fresh_retained = |kind| {
        let listed = extensions.iter().any(|entry| {
            matches!(entry, WorkbookExtension::RetainedPivotCaches(listed, _) if *listed == kind)
        });
        (retained_in(kind).next().is_some() && !listed)
            .then(|| WorkbookExtension::RetainedPivotCaches(kind, Vec::new()))
    };
    // Excel lists the slicer pivot caches before the slicer caches.
    let fresh = fresh_retained(RetainedPivotCacheList::Slicer)
        .into_iter()
        .chain(
            [
                SlicerListKind::Pivot,
                SlicerListKind::Table,
                SlicerListKind::Timeline,
            ]
            .into_iter()
            .filter(|kind| !ids_of(*kind).is_empty() && !listed(*kind))
            .map(WorkbookExtension::SlicerCaches),
        )
        .chain(fresh_retained(RetainedPivotCacheList::Timeline))
        .collect::<Vec<_>>();
    let mut ordered: Vec<(&WorkbookExtension, Option<usize>)> =
        fresh.iter().map(|entry| (entry, None)).collect();
    let mut carried_index = 0;
    for entry in extensions {
        match entry {
            WorkbookExtension::RetainedPivotCaches(kind, _) => {
                if retained_in(*kind).next().is_some() {
                    ordered.push((entry, None));
                }
            }
            WorkbookExtension::SlicerCaches(kind) => {
                if !ids_of(*kind).is_empty() {
                    ordered.push((entry, None));
                }
            }
            WorkbookExtension::Other(_) => {
                ordered.push((entry, Some(carried_index)));
                carried_index += 1;
            }
        }
    }
    if ordered.is_empty() {
        return Ok(());
    }
    write_start_tag(writer, "extLst", Vec::new(), false);
    for (entry, carried) in ordered {
        match entry {
            WorkbookExtension::RetainedPivotCaches(kind, _) => {
                let (prefix, namespace, name) = kind.list();
                let declaration = format!("xmlns:{prefix}");
                let list = format!("{prefix}:{name}");
                write_start_tag(
                    writer,
                    "ext",
                    vec![
                        ("uri", kind.uri()).into(),
                        (declaration.as_str(), namespace).into(),
                    ],
                    false,
                );
                write_start_tag(writer, list.as_str(), Vec::new(), false);
                for (_, id, relationship) in retained_in(*kind) {
                    write_start_tag(
                        writer,
                        "pivotCache",
                        vec![
                            ("cacheId", id.to_string()).into(),
                            ("r:id", relationship.as_str()).into(),
                        ],
                        true,
                    );
                }
                write_end_tag(writer, list.as_str());
                write_end_tag(writer, "ext");
            }
            WorkbookExtension::SlicerCaches(kind) => {
                let (list, entry) = if *kind == SlicerListKind::Timeline {
                    ("timelineCacheRefs", "timelineCacheRef")
                } else {
                    ("slicerCaches", "slicerCache")
                };
                write_ids(writer, *kind, ids_of(*kind), list, entry);
            }
            WorkbookExtension::Other(extension) => {
                let ids = carried
                    .and_then(|index| carried_ids.get(index))
                    .ok_or_else(|| xml_error("workbook extension has no relationship ids"))?;
                write_preserved(writer, extension, ids, &workbook_extension_scope())?;
            }
        }
    }
    write_end_tag(writer, "extLst");
    Ok(())
}

/// Write a worksheet `extLst`: the source entries in their original order,
/// slicer lists replaced by the parts actually written and the modelled data
/// validations written from the model. Entries the model gained since the read
/// follow the source entries.
pub(crate) fn write_worksheet_extensions(
    writer: &mut quick_xml::Writer<std::io::Cursor<Vec<u8>>>,
    extensions: &[WorksheetExtension],
    data_validations: Option<&crate::structs::office2010::excel::DataValidations>,
    ids: &WorksheetExtensionIds,
) -> Result<(), XlsxError> {
    let list_id = |kind| match kind {
        SlicerListKind::Pivot => ids.pivot_slicers.as_ref(),
        SlicerListKind::Table => ids.table_slicers.as_ref(),
        SlicerListKind::Timeline => ids.timelines.as_ref(),
    };
    let has_dv_entry = extensions
        .iter()
        .any(|entry| matches!(entry, WorksheetExtension::DataValidations));
    let mut ordered: Vec<(&WorksheetExtension, Option<usize>)> = Vec::new();
    let dv_entry = WorksheetExtension::DataValidations;
    if data_validations.is_some() && !has_dv_entry {
        ordered.push((&dv_entry, None));
    }
    let mut carried_index = 0;
    for entry in extensions {
        match entry {
            WorksheetExtension::DataValidations => {
                if data_validations.is_some() {
                    ordered.push((entry, None));
                }
            }
            WorksheetExtension::SlicerList(kind) => {
                if list_id(*kind).is_some() {
                    ordered.push((entry, None));
                }
            }
            WorksheetExtension::Other(_) => {
                ordered.push((entry, Some(carried_index)));
                carried_index += 1;
            }
        }
    }
    let fresh = [
        SlicerListKind::Pivot,
        SlicerListKind::Table,
        SlicerListKind::Timeline,
    ]
    .into_iter()
    .filter(|kind| {
        let listed_already = extensions
            .iter()
            .any(|entry| matches!(entry, WorksheetExtension::SlicerList(listed) if listed == kind));
        list_id(*kind).is_some() && !listed_already
    })
    .map(WorksheetExtension::SlicerList)
    .collect::<Vec<_>>();
    ordered.extend(fresh.iter().map(|entry| (entry, None)));
    if ordered.is_empty() {
        return Ok(());
    }
    write_start_tag(writer, "extLst", Vec::new(), false);
    for (entry, carried) in ordered {
        match entry {
            WorksheetExtension::DataValidations => {
                if let Some(validations) = data_validations {
                    validations.write_to(writer);
                }
            }
            WorksheetExtension::SlicerList(kind) => {
                let id = list_id(*kind).expect("listed kinds have an id");
                let (list, entry) = if *kind == SlicerListKind::Timeline {
                    ("timelineRefs", "timelineRef")
                } else {
                    ("slicerList", "slicer")
                };
                write_ids(writer, *kind, std::slice::from_ref(id), list, entry);
            }
            WorksheetExtension::Other(extension) => {
                let new_ids = carried
                    .and_then(|index| ids.carried.get(index))
                    .ok_or_else(|| xml_error("worksheet extension has no relationship ids"))?;
                write_preserved(writer, extension, new_ids, &worksheet_extension_scope())?;
            }
        }
    }
    write_end_tag(writer, "extLst");
    Ok(())
}

/// Remove matching links; true when the cache had links and none are left.
fn unlink_pivot_tables(
    cache: &mut SlicerCache,
    leaves: impl Fn(&SlicerPivotTable) -> bool,
) -> bool {
    let before = cache.linked_pivots().len();
    if let Some(tables) = &mut cache.pivot_tables {
        tables.retain(|link| !leaves(link));
        if tables.is_empty() {
            cache.pivot_tables = None;
        }
    }
    for extension in &mut cache.extensions {
        if let SlicerCacheExtension::PivotTables { tables, .. } = extension {
            tables.retain(|link| !leaves(link));
        }
    }
    cache.extensions.retain(|extension| {
        !matches!(
            extension,
            SlicerCacheExtension::PivotTables { tables, .. } if tables.is_empty()
        )
    });
    before > 0 && cache.linked_pivots().is_empty()
}

fn binding_error(message: impl std::fmt::Display) -> XlsxError {
    xml_error(format!("unresolved slicer binding: {message}"))
}

fn sheet_by_id(workbook: &Workbook, sheet_id: u32) -> Option<&Worksheet> {
    workbook
        .sheet_collection_no_check()
        .iter()
        .find(|sheet| sheet.sheet_id().parse::<u32>() == Ok(sheet_id))
}

/// Check that the pivot table a slicer cache links to exists exactly once on
/// its sheet and belongs to the pivot cache the slicer cache names.
fn check_pivot_link(
    cache: &SlicerCache,
    link: &SlicerPivotTable,
    sheet: &Worksheet,
    pivot_cache_id: u32,
) -> Result<(), XlsxError> {
    let tables: Vec<_> = sheet
        .pivot_tables()
        .iter()
        .filter(|table| table.pivot_table_definition().name() == link.name)
        .collect();
    match tables.as_slice() {
        [] => Err(binding_error(format!(
            "slicer cache '{}' references missing pivot table '{}' on sheet id {}",
            cache.name, link.name, link.sheet_id
        ))),
        [table] => {
            if table.pivot_cache_definition().slicer_cache_id() == Some(pivot_cache_id) {
                Ok(())
            } else {
                Err(binding_error(format!(
                    "slicer cache '{}' pivot cache id {pivot_cache_id} differs from the pivot \
                     cache of table '{}'",
                    cache.name, link.name
                )))
            }
        }
        _ => Err(binding_error(format!(
            "slicer cache '{}' pivot table '{}' is ambiguous on sheet id {}",
            cache.name, link.name, link.sheet_id
        ))),
    }
}

fn pivot_cache_id_of(cache: &SlicerCache) -> Option<u32> {
    match &cache.source {
        Some(SlicerSource::TabularPivot(source)) => Some(source.pivot_cache_id),
        Some(SlicerSource::Olap(source)) => Some(source.pivot_cache_id),
        _ => None,
    }
}

impl Workbook {
    /// Make every slicer binding of the workbook inspectable before a sheet is
    /// removed, so the removal can reconcile them without failing half way.
    pub(crate) fn prepare_slicer_reconciliation(&mut self) -> Result<(), XlsxError> {
        if self.slicer_caches().is_empty() && self.timeline_caches().is_empty() {
            return Ok(());
        }
        self.try_read_sheet_collection().map(|_| ())
    }

    /// Remove caches whose last view was deleted with its drawing. All view
    /// hosts must be read before checking whether a cache still has a view.
    pub(crate) fn reconcile_removed_drawing_caches(&mut self, candidates: &HashSet<String>) {
        if candidates.is_empty() {
            return;
        }
        let surviving: HashSet<String> = self
            .sheet_collection_no_check()
            .iter()
            .flat_map(|sheet| {
                sheet
                    .slicers()
                    .iter()
                    .map(|view| view.cache.to_lowercase())
                    .chain(
                        sheet
                            .timelines()
                            .iter()
                            .map(|view| view.cache.to_lowercase()),
                    )
            })
            .collect();
        let dropped: HashSet<String> = candidates.difference(&surviving).cloned().collect();
        if !dropped.is_empty() {
            self.slicer_caches_mut()
                .retain(|cache| !dropped.contains(&cache.name.to_lowercase()));
            self.timeline_caches_mut()
                .retain(|cache| !dropped.contains(&cache.name.to_lowercase()));
            self.remove_slicer_caches_named(&dropped);
            self.remove_unused_retained_pivot_caches();
        }
    }

    /// Follow the removal of `removed` the way Excel does: the slicers of the
    /// sheet go with it, its pivot tables leave the pivot slicer caches that
    /// listed them, a pivot slicer cache left without pivot tables and a table
    /// slicer cache whose table was on the sheet are removed, and with a
    /// removed cache its slicers and drawing frames on the other sheets.
    pub(crate) fn reconcile_slicers_after_sheet_removal(&mut self, removed: &Worksheet) {
        if self.slicer_caches().is_empty() && self.timeline_caches().is_empty() {
            return;
        }
        let removed_id = removed.sheet_id().parse::<u32>().ok();
        let removed_tables: HashSet<u32> = removed
            .tables()
            .iter()
            .filter_map(crate::structs::Table::id)
            .collect();
        let mut dropped = self.unlink_timelines_of_removed_sheet(removed);
        self.slicer_caches_mut().retain_mut(|cache| {
            let drop = if let Some(SlicerSource::Table(source)) = &cache.source {
                removed_tables.contains(&source.table_id)
            } else {
                unlink_pivot_tables(cache, |link| Some(link.sheet_id) == removed_id)
            };
            if drop {
                dropped.insert(cache.name.to_lowercase());
            }
            !drop
        });
        self.remove_slicer_caches_named(&dropped);
        self.remove_unused_retained_pivot_caches();
        self.retain_pivot_caches_of(removed.pivot_tables());
    }

    /// Whole row/column coverage removes the model, keeping disconnected
    /// pivot consumers and their caches as Excel does.
    pub(crate) fn reconcile_consumers_after_model_removal(
        &mut self,
        removed_sheet_id: &str,
        removed_tables: &[crate::Table],
        removed_pivots: &[crate::PivotTable],
    ) {
        let table_ids: HashSet<_> = removed_tables.iter().filter_map(crate::Table::id).collect();
        let sheet_id = removed_sheet_id.parse::<u32>().ok();
        let leaves = |link: &SlicerPivotTable| {
            Some(link.sheet_id) == sheet_id
                && removed_pivots
                    .iter()
                    .any(|pivot| pivot.pivot_table_definition().name() == link.name)
        };
        let mut dropped = HashSet::new();
        self.slicer_caches_mut().retain_mut(|cache| {
            let removed_table = matches!(&cache.source, Some(SlicerSource::Table(source))
                if table_ids.contains(&source.table_id));
            if removed_table {
                dropped.insert(cache.name.to_lowercase());
                false
            } else {
                unlink_pivot_tables(cache, leaves);
                true
            }
        });
        for cache in self.timeline_caches_mut() {
            if let Some(tables) = &mut cache.pivot_tables {
                tables.retain(|link| !leaves(link));
                if tables.is_empty() {
                    cache.pivot_tables = None;
                }
            }
        }
        self.remove_slicer_caches_named(&dropped);
        self.retain_pivot_caches_of(removed_pivots);
    }

    /// The list that declares a pivot cache kept without a pivot table, by
    /// what still reads it: `Slicer` when a pivot slicer cache does, else
    /// `Timeline` when a timeline cache does, else none (Excel then drops the
    /// cache).
    pub(crate) fn retained_pivot_cache_list(
        &self,
        definition: &crate::structs::PivotCacheDefinition,
    ) -> Option<RetainedPivotCacheList> {
        let id = definition.slicer_cache_id()?;
        if self
            .slicer_caches()
            .iter()
            .any(|cache| pivot_cache_id_of(cache) == Some(id))
        {
            Some(RetainedPivotCacheList::Slicer)
        } else if self
            .timeline_caches()
            .iter()
            .any(|cache| cache.state.pivot_cache_id == id)
        {
            Some(RetainedPivotCacheList::Timeline)
        } else {
            None
        }
    }

    /// Keep the pivot caches of the `removed` pivot tables that no pivot table
    /// left carries but a slicer or timeline cache still reads: Excel keeps
    /// their definitions and records.
    pub(crate) fn retain_pivot_caches_of<'a>(
        &mut self,
        removed: impl IntoIterator<Item = &'a crate::structs::PivotTable>,
    ) {
        for table in removed {
            let id = table.pivot_table_definition().cache_id();
            let definition = table.pivot_cache_definition();
            let carried = self
                .sheet_collection_no_check()
                .iter()
                .flat_map(Worksheet::pivot_tables)
                .any(|table| table.pivot_table_definition().cache_id() == id)
                || self.pivot_orphans.iter().any(|(known, _)| *known == id);
            if !carried && self.retained_pivot_cache_list(definition).is_some() {
                self.pivot_orphans.push((id, definition.clone()));
            }
        }
    }

    /// Remove what belongs to the slicer or timeline caches named in `dropped`
    /// (lowercase): their defined names, and their views and frames on every
    /// sheet.
    fn remove_slicer_caches_named(&mut self, dropped: &HashSet<String>) {
        if dropped.is_empty() {
            return;
        }
        // Excel names every slicer cache with a workbook name of the same name
        // and removes it with the cache.
        let names_a_dropped_cache = |name: &crate::structs::DefinedName| {
            !name.has_local_sheet_id() && dropped.contains(&name.name().to_lowercase())
        };
        self.defined_names_mut()
            .retain(|name| !names_a_dropped_cache(name));
        for sheet in self.sheet_collection_unchecked_mut() {
            sheet
                .defined_names_mut()
                .retain(|name| !names_a_dropped_cache(name));
        }
        for sheet in self.sheet_collection_unchecked_mut() {
            let gone: Vec<String> = sheet
                .slicers()
                .iter()
                .filter(|slicer| dropped.contains(&slicer.cache.to_lowercase()))
                .map(|slicer| slicer.name.clone())
                .collect();
            let gone_timelines: Vec<String> = sheet
                .timelines()
                .iter()
                .filter(|timeline| dropped.contains(&timeline.cache.to_lowercase()))
                .map(|timeline| timeline.name.clone())
                .collect();
            if gone.is_empty() && gone_timelines.is_empty() {
                continue;
            }
            sheet
                .slicers_mut()
                .retain(|slicer| !gone.contains(&slicer.name));
            sheet
                .timelines_mut()
                .retain(|timeline| !gone_timelines.contains(&timeline.name));
            let drawing = sheet.worksheet_drawing_mut();
            let frame_ids: Vec<u32> = drawing
                .slicer_frames()
                .iter()
                .filter(|frame| {
                    let names = match frame.kind() {
                        crate::drawing::spreadsheet::SlicerFrameKind::Slicer => &gone,
                        crate::drawing::spreadsheet::SlicerFrameKind::Timeline => &gone_timelines,
                    };
                    names.iter().any(|name| name == frame.slicer_name())
                })
                .map(crate::drawing::spreadsheet::SlicerFrame::frame_id)
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
        }
    }

    /// Check one worksheet about to be written by a streaming writer against
    /// the workbook slicer caches: its slicers and slicer frames, and the pivot
    /// tables the caches link to on it. `other_slicer_names` are the lowercase
    /// names of the slicers of every other sheet. Returns the lowercase names
    /// of the slicers of `worksheet`.
    pub(crate) fn validate_sheet_slicers(
        &self,
        worksheet: &Worksheet,
        other_slicer_names: &HashSet<String>,
    ) -> Result<Vec<String>, XlsxError> {
        let mut names = Vec::new();
        for slicer in worksheet.slicers() {
            slicer.validate().map_err(binding_error)?;
            let name = slicer.name.to_lowercase();
            if other_slicer_names.contains(&name) || names.contains(&name) {
                return Err(binding_error(format!("duplicate slicer '{}'", slicer.name)));
            }
            names.push(name);
            let wanted = slicer.cache.to_lowercase();
            if !self
                .slicer_caches()
                .iter()
                .any(|cache| cache.name.to_lowercase() == wanted)
            {
                return Err(binding_error(format!(
                    "slicer '{}' on sheet '{}' references unknown slicer cache '{}'",
                    slicer.name,
                    worksheet.name(),
                    slicer.cache
                )));
            }
        }
        for frame in worksheet.worksheet_drawing().slicer_frames() {
            if frame.kind() == crate::drawing::spreadsheet::SlicerFrameKind::Slicer
                && !worksheet
                    .slicers()
                    .iter()
                    .any(|slicer| slicer.name == frame.slicer_name())
            {
                return Err(binding_error(format!(
                    "drawing frame on sheet '{}' references unknown slicer '{}'",
                    worksheet.name(),
                    frame.slicer_name()
                )));
            }
        }
        let mut taken = other_slicer_names.clone();
        taken.extend(names.iter().cloned());
        names.extend(self.validate_sheet_timelines(worksheet, &taken)?);
        for cache in self.slicer_caches() {
            let Some(pivot_cache_id) = pivot_cache_id_of(cache) else {
                continue;
            };
            for link in cache.linked_pivots() {
                if worksheet.sheet_id().parse::<u32>() == Ok(link.sheet_id) {
                    check_pivot_link(cache, link, worksheet, pivot_cache_id)?;
                }
            }
        }
        Ok(names)
    }

    /// Check the slicer caches of a workbook whose sheets were streamed out:
    /// every sheet a cache links to was written, and every table a cache binds
    /// to was written with that id.
    pub(crate) fn validate_streamed_slicer_caches(
        &self,
        written_table_ids: &HashSet<u32>,
        written_pivot_cache_ids: &HashSet<u32>,
    ) -> Result<(), XlsxError> {
        let mut cache_names = HashSet::new();
        for cache in self.slicer_caches() {
            cache.validate().map_err(binding_error)?;
            if !cache_names.insert(cache.name.to_lowercase()) {
                return Err(binding_error(format!(
                    "duplicate slicer cache '{}'",
                    cache.name
                )));
            }
            for link in cache.linked_pivots() {
                if sheet_by_id(self, link.sheet_id).is_none() {
                    return Err(binding_error(format!(
                        "slicer cache '{}' references missing sheet id {}",
                        cache.name, link.sheet_id
                    )));
                }
            }
            if let Some(SlicerSource::Table(source)) = &cache.source {
                if !written_table_ids.contains(&source.table_id) {
                    return Err(binding_error(format!(
                        "slicer cache '{}' references missing table id {}",
                        cache.name, source.table_id
                    )));
                }
            }
            // The pivot cache a slicer cache reads must have been written with
            // a pivot table; otherwise the slicer cache would name a cache the
            // package no longer holds.
            if let Some(pivot_cache_id) = pivot_cache_id_of(cache) {
                if !written_pivot_cache_ids.contains(&pivot_cache_id) {
                    return Err(binding_error(format!(
                        "slicer cache '{}' references missing pivot cache id {pivot_cache_id}",
                        cache.name
                    )));
                }
            }
        }
        self.validate_streamed_timeline_caches(written_pivot_cache_ids)
    }

    /// Check that every slicer binding resolves by identity: slicer to cache
    /// by name, pivot slicer cache to pivot tables by `(tabId, name)` and to
    /// its pivot cache by the x14 pivot cache id (never the workbook
    /// `cacheId`), table slicer cache to its table by `tableId`.
    ///
    /// Worksheets that are not deserialized yet cannot be inspected; bindings
    /// that depend on them are checked once they are read (and always at
    /// write time). `written_table_ids` holds the table ids of every sheet,
    /// including raw ones, when the writer has collected them.
    pub(crate) fn validate_slicers(
        &self,
        written_table_ids: Option<&HashSet<u32>>,
    ) -> Result<(), XlsxError> {
        let sheets = self.sheet_collection_no_check();
        // At write time every sheet a slicer depends on has been read, so the
        // checks are complete; while reading, unread sheets defer them.
        let complete = written_table_ids.is_some() || sheets.iter().all(Worksheet::is_inspectable);
        let mut cache_names = HashSet::new();
        for cache in self.slicer_caches() {
            cache.validate().map_err(binding_error)?;
            // Excel compares cache names without regard to case.
            if !cache_names.insert(cache.name.to_lowercase()) {
                return Err(binding_error(format!(
                    "duplicate slicer cache '{}'",
                    cache.name
                )));
            }
            match &cache.source {
                Some(SlicerSource::TabularPivot(source)) => {
                    self.check_pivot_cache(cache, source.pivot_cache_id, complete)?;
                }
                Some(SlicerSource::Olap(source)) => {
                    self.check_pivot_cache(cache, source.pivot_cache_id, complete)?;
                }
                Some(SlicerSource::Table(source)) => {
                    let found = match written_table_ids {
                        Some(ids) => ids.contains(&source.table_id),
                        None => sheets
                            .iter()
                            .filter(|sheet| sheet.is_inspectable())
                            .flat_map(Worksheet::tables)
                            .any(|table| table.id() == Some(source.table_id)),
                    };
                    if !found && complete {
                        return Err(binding_error(format!(
                            "slicer cache '{}' references missing table id {}",
                            cache.name, source.table_id
                        )));
                    }
                }
                None => {}
            }
        }
        let mut slicer_names = HashSet::new();
        for sheet in sheets.iter().filter(|sheet| sheet.is_inspectable()) {
            for slicer in sheet.slicers() {
                slicer.validate().map_err(binding_error)?;
                if !slicer_names.insert(slicer.name.to_lowercase()) {
                    return Err(binding_error(format!("duplicate slicer '{}'", slicer.name)));
                }
                if !cache_names.contains(&slicer.cache.to_lowercase()) {
                    return Err(binding_error(format!(
                        "slicer '{}' on sheet '{}' references unknown slicer cache '{}'",
                        slicer.name,
                        sheet.name(),
                        slicer.cache
                    )));
                }
            }
            for frame in sheet.worksheet_drawing().slicer_frames() {
                if frame.kind() == crate::drawing::spreadsheet::SlicerFrameKind::Slicer
                    && !sheet
                        .slicers()
                        .iter()
                        .any(|slicer| slicer.name == frame.slicer_name())
                {
                    return Err(binding_error(format!(
                        "drawing frame on sheet '{}' references unknown slicer '{}'",
                        sheet.name(),
                        frame.slicer_name()
                    )));
                }
            }
        }
        self.validate_timelines(complete)
    }

    fn check_pivot_cache(
        &self,
        cache: &SlicerCache,
        pivot_cache_id: u32,
        complete: bool,
    ) -> Result<(), XlsxError> {
        for link in cache.linked_pivots() {
            let sheet = sheet_by_id(self, link.sheet_id).ok_or_else(|| {
                binding_error(format!(
                    "slicer cache '{}' references missing sheet id {}",
                    cache.name, link.sheet_id
                ))
            })?;
            if !sheet.is_inspectable() {
                if complete {
                    return Err(binding_error(format!(
                        "slicer cache '{}' links sheet '{}', which was not read",
                        cache.name,
                        sheet.name()
                    )));
                }
                continue;
            }
            check_pivot_link(cache, link, sheet, pivot_cache_id)?;
        }
        let found = self
            .sheet_collection_no_check()
            .iter()
            .filter(|sheet| sheet.is_inspectable())
            .flat_map(Worksheet::pivot_tables)
            .map(crate::structs::PivotTable::pivot_cache_definition)
            .chain(
                self.retained_pivot_caches()
                    .map(|(_, definition)| definition),
            )
            .any(|definition| definition.slicer_cache_id() == Some(pivot_cache_id));
        if !found && complete {
            return Err(binding_error(format!(
                "slicer cache '{}' references missing pivot cache id {pivot_cache_id}",
                cache.name
            )));
        }
        Ok(())
    }
}
