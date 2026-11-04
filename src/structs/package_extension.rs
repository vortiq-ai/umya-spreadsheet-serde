//! Extension list entries (`<ext uri=...>`) kept verbatim together with the
//! package parts they reference.
//!
//! An `ext` that carries relationship attributes (`r:id`, `r:embed`, ...) is
//! only meaningful with the parts those relationships point at. Reading
//! resolves every relationship through the exact owner part and keeps the
//! referenced part bytes; writing allocates new part names and relationship
//! ids and rewrites the attributes, so nothing dangles.

use std::collections::HashMap;

use crate::{
    drawing::charts::{
        OpaqueElement,
        OpaqueToken,
    },
    helper::const_str::REL_OFC_NS,
};

/// A package part carried through a save because an extension names it.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub(crate) struct ExtensionFile {
    /// Package path the part had in the source.
    pub(crate) source_name:  String,
    pub(crate) content_type: String,
    pub(crate) data:         Vec<u8>,
}

/// One relationship an extension refers to.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub(crate) struct ExtensionPart {
    /// The relationship id the preserved element uses.
    pub(crate) source_id:         String,
    pub(crate) relationship_type: String,
    /// Target of an external relationship.
    pub(crate) external_target:   Option<String>,
    /// The referenced part of an internal relationship.
    pub(crate) file:              Option<ExtensionFile>,
}

/// An extension entry kept exactly as read.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub(crate) struct PreservedExtension {
    pub(crate) element: OpaqueElement,
    /// Resolved relationships, one per distinct id of `element`, in order of
    /// first use. Empty until the owner's relationships were read.
    pub(crate) parts:   Vec<ExtensionPart>,
}

impl PreservedExtension {
    pub(crate) fn new(element: OpaqueElement) -> Self {
        Self {
            element,
            parts: Vec::new(),
        }
    }

    /// Distinct relationship ids the element names, in order of first use.
    pub(crate) fn relationship_ids(&self) -> Vec<String> {
        distinct_relationship_ids(&self.element)
    }

    /// Write the element with the relationship ids the writer allocated for
    /// it: `ids` yields the list for each extension in the order they are
    /// written.
    pub(crate) fn write_rebound(
        &self,
        writer: &mut quick_xml::Writer<std::io::Cursor<Vec<u8>>>,
        ids: &mut dyn Iterator<Item = Vec<String>>,
    ) -> Result<(), crate::XlsxError> {
        let new_ids = ids
            .next()
            .ok_or_else(|| crate::xml::xml_error("extension has no relationship ids"))?;
        self.rebound(&new_ids)
            .map_err(crate::xml::xml_error)?
            .write_to(writer, &crate::structs::slicer::worksheet_extension_scope())
            .map_err(crate::xml::xml_error)
    }

    /// The element with each relationship id replaced by the id the writer
    /// allocated for it. `new_ids` is parallel to `parts`.
    pub(crate) fn rebound(&self, new_ids: &[String]) -> Result<OpaqueElement, String> {
        if new_ids.len() != self.parts.len() {
            return Err("extension relationship count differs from its parts".into());
        }
        let map: HashMap<&str, &str> = self
            .parts
            .iter()
            .zip(new_ids)
            .map(|(part, id)| (part.source_id.as_str(), id.as_str()))
            .collect();
        let mut missing = None;
        let mut element = self.element.clone();
        rewrite_relationship_attributes(&mut element, &mut |value| {
            if let Some(new) = map.get(value.as_str()) {
                *value = (*new).to_owned();
            } else {
                missing = Some(value.clone());
            }
        });
        match missing {
            Some(id) => Err(format!("extension relationship '{id}' was never resolved")),
            None => Ok(element),
        }
    }
}

fn is_relationship_attribute(name: &crate::drawing::charts::QName) -> bool {
    name.namespace_uri == REL_OFC_NS
}

/// Visit the value of every relationship-namespace attribute of `element` and
/// its descendants.
pub(crate) fn visit_relationship_attributes(element: &OpaqueElement, visit: &mut dyn FnMut(&str)) {
    for (name, value) in &element.attributes {
        if is_relationship_attribute(name) {
            visit(value);
        }
    }
    for token in &element.children {
        if let OpaqueToken::Start(start) = token {
            for (name, value) in &start.attributes {
                if is_relationship_attribute(name) {
                    visit(value);
                }
            }
        }
    }
}

/// Mutable counterpart of [`visit_relationship_attributes`].
pub(crate) fn rewrite_relationship_attributes(
    element: &mut OpaqueElement,
    rewrite: &mut dyn FnMut(&mut String),
) {
    for (name, value) in &mut element.attributes {
        if is_relationship_attribute(name) {
            rewrite(value);
        }
    }
    for token in &mut element.children {
        if let OpaqueToken::Start(start) = token {
            for (name, value) in &mut start.attributes {
                if is_relationship_attribute(name) {
                    rewrite(value);
                }
            }
        }
    }
}

pub(crate) fn distinct_relationship_ids(element: &OpaqueElement) -> Vec<String> {
    let mut ids: Vec<String> = Vec::new();
    visit_relationship_attributes(element, &mut |id| {
        if !ids.iter().any(|known| known == id) {
            ids.push(id.to_owned());
        }
    });
    ids
}

/// Directory, file stem and extension of a package path, used to name the
/// replacement part like its source.
pub(crate) fn split_part_name(path: &str) -> (&str, &str, &str) {
    let (directory, file) = path.rsplit_once('/').unwrap_or(("", path));
    match file.rsplit_once('.') {
        Some((stem, extension)) => (
            directory,
            stem.trim_end_matches(|c: char| c.is_ascii_digit()),
            extension,
        ),
        None => (directory, file, ""),
    }
}
