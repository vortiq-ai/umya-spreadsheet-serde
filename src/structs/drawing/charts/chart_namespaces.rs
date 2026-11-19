//! Current-chart namespace metadata only. Original XML and cache spans stay
//! unchanged.
use std::{
    borrow::Cow,
    collections::HashMap,
};

use quick_xml::{
    NsReader,
    XmlVersion,
    events::{
        Event,
        attributes::Attribute,
    },
    name::{
        PrefixDeclaration,
        QName as XmlQName,
    },
};
use sha2::{
    Digest,
    Sha256,
};

use super::{
    extension_names::{
        NamespaceScope,
        QName,
    },
    extension_registry::NS_C15,
};
const NS_C: &str = "http://schemas.openxmlformats.org/drawingml/2006/chart";
const NS_A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const HEX_DIGITS: &[u8; 16] = b"0123456789abcdef";

fn sha256_hex(data: &[u8]) -> String {
    let digest = Sha256::digest(data);
    let mut encoded = String::with_capacity(digest.len() * 2);
    for &byte in &digest {
        encoded.push(char::from(HEX_DIGITS[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX_DIGITS[usize::from(byte & 0x0f)]));
    }
    encoded
}
#[derive(Clone, Debug)]
pub(crate) struct ExtensionReadContext {
    pub scope:              NamespaceScope,
    pub container:          QName,
    pub owner:              QName,
    pub owner_path:         String,
    pub source_part_sha256: String,
    pub chart_part:         String,
}
#[derive(Default)]
pub(crate) struct ChartNamespaces {
    entries: HashMap<u64, ExtensionReadContext>,
}
struct Frame {
    name:     QName,
    path:     String,
    children: HashMap<(String, String), usize>,
    /// Its `pt` children are cache or literal points.
    points:   bool,
}
fn decoded_namespace(value: &[u8]) -> Result<Cow<'_, str>, String> {
    Attribute {
        key:   XmlQName(b"xmlns"),
        value: Cow::Borrowed(value),
    }
    .normalized_value(XmlVersion::Implicit1_0)
    .map_err(|e| e.to_string())
}
impl ChartNamespaces {
    pub fn get(
        &self,
        offset: u64,
        element: &quick_xml::events::BytesStart,
    ) -> Option<ExtensionReadContext> {
        let context = self.entries.get(&offset)?;
        (context
            .scope
            .resolve(element.name().into_inner(), false)
            .ok()?
            == context.container)
            .then(|| context.clone())
    }

    pub fn read(data: &[u8], part: &str) -> Result<Self, String> {
        let mut reader = NsReader::from_reader(data);
        let mut result = Self::default();
        let mut stack: Vec<Frame> = Vec::new();
        let mut hash = None;
        loop {
            match reader.read_event().map_err(|e| e.to_string())? {
                ref event @ (Event::Start(ref e) | Event::Empty(ref e)) => {
                    let empty = matches!(event, Event::Empty(_));
                    // Snapshot bindings only at boundaries; no cache payload
                    // values are decoded.
                    let mut bindings = Vec::new();
                    // No chart reader looks up an extension list inside a
                    // point, so every `pt` of a cache or literal is passed
                    // over before name resolution: a cache of millions of
                    // points costs no text decoding or per-point metadata.
                    if e.local_name().as_ref() == b"pt" && stack.last().is_some_and(|f| f.points) {
                        if !empty {
                            reader.read_to_end(e.name()).map_err(|e| e.to_string())?;
                        }
                        continue;
                    }
                    let (resolved, local) = reader.resolver().resolve_element(e.name());
                    let uri = match resolved {
                        quick_xml::name::ResolveResult::Bound(ns) => {
                            decoded_namespace(ns.into_inner())?.into_owned()
                        }
                        quick_xml::name::ResolveResult::Unbound => String::new(),
                        quick_xml::name::ResolveResult::Unknown(_) => {
                            return Err("undeclared chart namespace prefix".into());
                        }
                    };
                    let local = std::str::from_utf8(local.as_ref())
                        .map_err(|e| e.to_string())?
                        .to_owned();
                    let prefix = e
                        .name()
                        .prefix()
                        .map(|p| {
                            std::str::from_utf8(p.as_ref())
                                .map(str::to_owned)
                                .map_err(|e| e.to_string())
                        })
                        .transpose()?;
                    let name = QName {
                        namespace_uri:    uri.clone(),
                        local_name:       local.clone(),
                        preferred_prefix: prefix,
                    };
                    let occurrence = if let Some(parent) = stack.last_mut() {
                        let count = parent
                            .children
                            .entry((uri.clone(), local.clone()))
                            .or_default();
                        *count += 1;
                        *count
                    } else {
                        1
                    };
                    let path = format!(
                        "{}/{{{}}}{}[{}]",
                        stack.last().map_or("", |f| f.path.as_str()),
                        uri,
                        local,
                        occurrence
                    );
                    if local == "extLst" && (uri == NS_C || uri == NS_A) {
                        for (prefix, namespace) in reader.resolver().bindings() {
                            let prefix = match prefix {
                                PrefixDeclaration::Default => None,
                                PrefixDeclaration::Named(p) => Some(
                                    std::str::from_utf8(p)
                                        .map_err(|e| e.to_string())?
                                        .to_owned(),
                                ),
                            };
                            bindings.push((
                                prefix,
                                decoded_namespace(namespace.as_ref())?.into_owned(),
                            ));
                        }
                        let scope = NamespaceScope::with_bindings(bindings);
                        let Some(owner) = stack.last() else {
                            return Err("extension list without owning element".into());
                        };
                        let source_part_sha256 =
                            hash.get_or_insert_with(|| sha256_hex(data)).clone();
                        result.entries.insert(
                            reader.buffer_position(),
                            ExtensionReadContext {
                                scope,
                                container: name.clone(),
                                owner: owner.name.clone(),
                                owner_path: owner.path.clone(),
                                source_part_sha256,
                                chart_part: part.into(),
                            },
                        );
                    }
                    // A cache holds its own extLst after its points, so it
                    // is walked like any owner; only its points are skipped.
                    if !empty {
                        let points = (uri == NS_C
                            && matches!(
                                local.as_str(),
                                "numCache" | "strCache" | "numLit" | "strLit"
                            ))
                            || (uri == NS_C
                                && local == "lvl"
                                && stack.last().is_some_and(|f| {
                                    f.name.namespace_uri == NS_C
                                        && f.name.local_name == "multiLvlStrCache"
                                }))
                            || (uri == NS_C15
                                && matches!(
                                    local.as_str(),
                                    "dlblRangeCache" | "dlblFieldTableCache"
                                ));
                        stack.push(Frame {
                            name,
                            path,
                            children: HashMap::new(),
                            points,
                        });
                    }
                }
                Event::End(_) => {
                    stack.pop();
                }
                Event::Eof => return Ok(result),
                Event::DocType(_) => {
                    return Err("DTD is not allowed in chart namespace metadata".into());
                }
                _ => (),
            }
        }
    }
}
