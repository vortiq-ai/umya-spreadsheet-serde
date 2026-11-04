// oleObjects
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::BytesStart,
};

use super::OleObject;
use crate::{
    structs::raw::RawRelationships,
    writer::driver::{
        write_end_tag,
        write_start_tag,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Default, Debug)]
pub struct OleObjects {
    ole_object: Vec<OleObject>,
}

impl OleObjects {
    #[inline]
    #[must_use]
    pub fn ole_object(&self) -> &[OleObject] {
        &self.ole_object
    }

    #[inline]
    #[must_use]
    #[deprecated(since = "3.0.0", note = "Use ole_object()")]
    pub fn get_ole_object(&self) -> &[OleObject] {
        self.ole_object()
    }

    #[inline]
    pub fn ole_object_mut(&mut self) -> &mut Vec<OleObject> {
        &mut self.ole_object
    }

    #[inline]
    #[deprecated(since = "3.0.0", note = "Use ole_object_mut()")]
    pub fn get_ole_object_mut(&mut self) -> &mut Vec<OleObject> {
        self.ole_object_mut()
    }

    #[inline]
    pub fn set_ole_object(&mut self, value: OleObject) -> &mut Self {
        self.ole_object.push(value);
        self
    }

    pub(crate) fn set_attributes<R: std::io::BufRead>(
        &mut self,
        reader: &mut Reader<R>,
        element: &BytesStart,
        relationships: &RawRelationships,
        scope: &crate::drawing::charts::NamespaceScope,
    ) -> Result<(), crate::XlsxError> {
        use crate::{
            drawing::charts::{
                NamespaceScope,
                OpaqueElement,
            },
            helper::const_str::{
                MC_NS,
                SHEET_MAIN_NS,
                SHEET_MS_MAIN_NS,
            },
            xml::{
                attribute,
                direct_children,
                required_attribute,
                xml_error,
            },
        };
        let element = OpaqueElement::read(reader, element, false, scope).map_err(xml_error)?;
        crate::xml::expect(&element, SHEET_MAIN_NS, "oleObjects").map_err(xml_error)?;
        let mut parsed = Self::default();
        let mut identities = std::collections::HashSet::new();
        for child in direct_children(&element).map_err(xml_error)? {
            let object = if child.name.is(SHEET_MAIN_NS, "oleObject") {
                OleObject::from_element(&child, relationships).map_err(xml_error)?
            } else if child.name.is(MC_NS, "AlternateContent") {
                let branches = direct_children(&child).map_err(xml_error)?;
                if branches.is_empty()
                    || branches.len() > 2
                    || !branches[0].name.is(MC_NS, "Choice")
                    || branches
                        .get(1)
                        .is_some_and(|branch| !branch.name.is(MC_NS, "Fallback"))
                {
                    return Err(xml_error("unsupported OLE object compatibility branches"));
                }
                let choice = &branches[0];
                let requires = required_attribute(choice, "", "Requires").map_err(xml_error)?;
                let bindings = NamespaceScope::with_bindings(choice.namespaces.clone());
                let prefixes: Vec<_> = requires.split_ascii_whitespace().collect();
                if prefixes.is_empty() {
                    return Err(xml_error("missing OLE object compatibility requirements"));
                }
                for prefix in prefixes {
                    let name = bindings
                        .resolve(format!("{prefix}:required").as_bytes(), false)
                        .map_err(xml_error)?;
                    if name.namespace_uri != SHEET_MS_MAIN_NS {
                        return Err(xml_error(
                            "unsupported OLE object compatibility requirement",
                        ));
                    }
                }
                let children = direct_children(choice).map_err(xml_error)?;
                if children.len() != 1 {
                    return Err(xml_error("OLE compatibility choice requires one object"));
                }
                let mut object =
                    OleObject::from_element(&children[0], relationships).map_err(xml_error)?;
                object.set_requires("x14");
                if let Some(fallback) = branches.get(1) {
                    let children = direct_children(fallback).map_err(xml_error)?;
                    if children.len() != 1 {
                        return Err(xml_error("OLE compatibility fallback requires one object"));
                    }
                    crate::xml::expect(&children[0], SHEET_MAIN_NS, "oleObject")
                        .map_err(xml_error)?;
                    let primary = direct_children(choice).map_err(xml_error)?;
                    for (namespace, name) in [
                        ("", "shapeId"),
                        ("", "progId"),
                        ("", "dvAspect"),
                        ("", "link"),
                        ("", "oleUpdate"),
                        ("", "autoLoad"),
                        (crate::helper::const_str::REL_OFC_NS, "id"),
                    ] {
                        if attribute(&children[0], namespace, name).map_err(xml_error)?
                            != attribute(&primary[0], namespace, name).map_err(xml_error)?
                        {
                            return Err(xml_error(format!("conflicting OLE fallback {name}")));
                        }
                    }
                }
                object
            } else {
                return Err(xml_error("unsupported OLE objects child"));
            };
            let identity = object
                .shape_id()
                .ok_or_else(|| xml_error("missing OLE object shape identity"))?;
            if !identities.insert(identity) {
                return Err(xml_error("duplicate OLE object shape identity"));
            }
            parsed.set_ole_object(object);
        }
        *self = parsed;
        Ok(())
    }

    pub(crate) fn resolved_shape_ids(&self) -> Result<Vec<usize>, crate::XlsxError> {
        let mut used = std::collections::HashSet::new();
        let mut identities = Vec::with_capacity(self.ole_object.len());
        for object in &self.ole_object {
            if !object.requires().is_empty() && object.requires() != "x14" {
                return Err(crate::xml::xml_error(
                    "unsupported OLE object compatibility requirement",
                ));
            }
            let identity = object.shape_id().or(object
                .drawing_properties()
                .map(crate::drawing::spreadsheet::NonVisualDrawingProperties::legacy_shape_id)
                .transpose()
                .map_err(crate::xml::xml_error)?
                .flatten());
            if let Some(identity) = identity {
                if !used.insert(identity) {
                    return Err(crate::xml::xml_error("duplicate OLE object shape identity"));
                }
            }
            identities.push(identity);
        }
        let mut next = 1025_u32;
        identities
            .into_iter()
            .map(|identity| {
                let identity = if let Some(identity) = identity {
                    identity
                } else {
                    while used.contains(&next) {
                        next = next.checked_add(1).ok_or_else(|| {
                            crate::xml::xml_error("OLE object shape identity space exhausted")
                        })?;
                    }
                    used.insert(next);
                    next
                };
                usize::try_from(identity).map_err(crate::xml::xml_error)
            })
            .collect()
    }

    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        r_id: usize,
    ) -> Result<(), crate::XlsxError> {
        let shape_ids = self.resolved_shape_ids()?;
        if !self.ole_object.is_empty() {
            // oleObjects
            write_start_tag(writer, "oleObjects", vec![], false);

            // mc:AlternateContent
            let mut r = r_id;
            for (obj, shape_id) in self.ole_object.iter().zip(shape_ids) {
                obj.write_to(writer, r, shape_id);
                r += 2;
            }

            write_end_tag(writer, "oleObjects");
        }
        Ok(())
    }
}

pub(crate) fn parse_legacy_shape_id(value: &str) -> Result<u32, String> {
    let digits = value
        .strip_prefix("_x0000_s")
        .ok_or("invalid legacy OLE shape identifier")?;
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err("invalid legacy OLE shape identifier".into());
    }
    let id = digits
        .parse::<u32>()
        .map_err(|_| "invalid legacy OLE shape identifier")?;
    if !(1025..=268_435_456).contains(&id) {
        return Err("legacy OLE shape identifier is out of range".into());
    }
    Ok(id)
}
