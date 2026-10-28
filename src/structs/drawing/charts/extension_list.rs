//! Ordered extension occurrences, authoritative known payloads and opaque
//! future XML.
use std::io::{
    Cursor,
    Write,
};

use quick_xml::{
    Reader,
    Writer,
    events::{
        BytesStart,
        Event,
    },
};

use super::{
    ExtensionNode,
    ExtensionPayload,
    NamespaceScope,
    OpaqueElement,
    OpaqueStart,
    OpaqueToken,
    QName,
    SeriesFamily,
    SeriesPartConflictReason,
    extension_registry::{
        NS_C,
        NS_C15,
        URI_LABEL,
        known_uri,
        registered,
    },
};
use crate::{
    reader::xlsx::SourceCacheSession,
    structs::Workbook,
};
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct Extension {
    pub uri:        Option<String>,
    pub namespaces: Vec<(Option<String>, String)>,
    pub attributes: Vec<(QName, String)>,
    pub nodes:      Vec<ExtensionNode>,
    name:           QName,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct ExtensionList {
    pub entries:        Vec<Extension>,
    name:               QName,
    owner:              QName,
    owner_path:         String,
    chart_part:         String,
    source_part_sha256: String,
    invalid:            bool,
}
/// One occurrence-level export decision. The whole-input hash must come from
/// the ordinary importing/exporting caller; chart-part hashes are not
/// substitutes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmptyKnownLabelExtensionOmitted {
    pub input_sha256:         String,
    pub chart_part:           String,
    pub owner_path:           String,
    pub extension_occurrence: usize,
    pub uri:                  String,
}
/// A stored extension that cannot be emitted under its actual current owner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExtensionExportDiagnostic {
    pub chart_part:           String,
    pub owner_path:           String,
    pub extension_occurrence: usize,
    pub uri:                  Option<String>,
    pub reason:               String,
}
impl Extension {
    #[must_use]
    pub fn is_empty_known_label_shell(&self, owner: &str) -> bool {
        matches!(owner, "dLbls" | "dLbl")
            && self
                .uri
                .as_deref()
                .is_some_and(|u| u.eq_ignore_ascii_case(URI_LABEL))
            && self.nodes.iter().all(
                |n| matches!(n,ExtensionNode::Text(s)|ExtensionNode::CData(s) if xml_whitespace(s)),
            )
    }

    fn conflict_for_owner(
        &self,
        owner: &str,
        family: Option<&str>,
    ) -> Option<SeriesPartConflictReason> {
        if owner == "spPr" && self.uri.is_none() {
            return Some(SeriesPartConflictReason::MissingRequiredChild);
        }
        if self.is_empty_known_label_shell(owner) {
            return None;
        }
        if let Some(uri) = self.uri.as_deref().filter(|uri| known_uri(uri)) {
            for node in &self.nodes {
                let name = match node {
                    ExtensionNode::Known(v) => v.expanded_name(),
                    ExtensionNode::Unknown(v) => v.name.clone(),
                    ExtensionNode::Text(s) | ExtensionNode::CData(s) if xml_whitespace(s) => {
                        continue;
                    }
                    ExtensionNode::Comment(_) | ExtensionNode::ProcessingInstruction(_) => continue,
                    _ => return Some(SeriesPartConflictReason::InvalidValue),
                };
                if !registered(uri, owner, family, &name) {
                    return Some(SeriesPartConflictReason::NotAllowed);
                }
            }
            if !self
                .nodes
                .iter()
                .any(|n| matches!(n, ExtensionNode::Known(_) | ExtensionNode::Unknown(_)))
            {
                return Some(SeriesPartConflictReason::MissingRequiredChild);
            }
        }
        self.nodes.iter().find_map(node_conflict)
    }

    fn as_opaque(&self) -> OpaqueElement {
        let mut attributes = self.attributes.clone();
        if let Some(uri) = &self.uri {
            attributes.insert(
                0,
                (
                    QName {
                        namespace_uri:    String::new(),
                        local_name:       "uri".into(),
                        preferred_prefix: None,
                    },
                    uri.clone(),
                ),
            );
        }
        let mut root = OpaqueElement {
            name: self.name.clone(),
            namespaces: self.namespaces.clone(),
            attributes,
            children: Vec::new(),
        };
        for node in &self.nodes {
            match node {
                ExtensionNode::Known(_) => (),
                ExtensionNode::Unknown(value) => append_element(&mut root, value.as_ref().clone()),
                ExtensionNode::Text(value) => root.children.push(OpaqueToken::Text(value.clone())),
                ExtensionNode::CData(value) => {
                    root.children.push(OpaqueToken::CData(value.clone()));
                }
                ExtensionNode::Comment(value) => {
                    root.children.push(OpaqueToken::Comment(value.clone()));
                }
                ExtensionNode::ProcessingInstruction(value) => root
                    .children
                    .push(OpaqueToken::ProcessingInstruction(value.clone())),
            }
        }
        root
    }

    fn validate_namespaces(&self) -> Result<(), String> {
        self.as_opaque().validate(&NamespaceScope::default())?;
        let scope = NamespaceScope::with_bindings(self.namespaces.clone());
        for node in &self.nodes {
            if let ExtensionNode::Known(payload) = node {
                payload.validate_namespaces(&scope)?;
            }
        }
        Ok(())
    }

    fn materialized(&self, wb: Option<&Workbook>) -> Result<OpaqueElement, String> {
        self.validate_namespaces()?;
        let mut root = self.as_opaque();
        root.children.clear();
        for node in &self.nodes {
            match node {
                ExtensionNode::Known(value) => {
                    // Fresh typed output uses canonical prefixes. Resolve them
                    // before assembling the flat opaque tape, whose initial
                    // validation must still protect every original binding.
                    let mut child = value.rendered(wb)?;
                    let scope = NamespaceScope::with_bindings(self.namespaces.clone());
                    child.fix_namespaces(&scope)?;
                    child.validate(&scope)?;
                    append_element(&mut root, child);
                }
                ExtensionNode::Unknown(value) => append_element(&mut root, value.as_ref().clone()),
                ExtensionNode::Text(value) => root.children.push(OpaqueToken::Text(value.clone())),
                ExtensionNode::CData(value) => {
                    root.children.push(OpaqueToken::CData(value.clone()));
                }
                ExtensionNode::Comment(value) => {
                    root.children.push(OpaqueToken::Comment(value.clone()));
                }
                ExtensionNode::ProcessingInstruction(value) => root
                    .children
                    .push(OpaqueToken::ProcessingInstruction(value.clone())),
            }
        }
        Ok(root)
    }
}
impl ExtensionList {
    #[must_use]
    pub fn source_part_sha256(&self) -> &str {
        &self.source_part_sha256
    }

    #[must_use]
    pub fn chart_part(&self) -> &str {
        &self.chart_part
    }

    #[must_use]
    pub fn owner_path(&self) -> &str {
        &self.owner_path
    }

    /// No workbook identity is fabricated. This method requires the genuine
    /// input identity held by the ordinary import/C4/harness context.
    pub fn empty_known_label_omissions(
        &self,
        input_sha256: &str,
    ) -> Result<Vec<EmptyKnownLabelExtensionOmitted>, String> {
        if input_sha256.len() != 64 || !input_sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("whole-input SHA256 is required for omission accounting".into());
        }
        Ok(self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| e.is_empty_known_label_shell(&self.owner.local_name))
            .map(|(occurrence, e)| EmptyKnownLabelExtensionOmitted {
                input_sha256:         input_sha256.into(),
                chart_part:           self.chart_part.clone(),
                owner_path:           self.owner_path.clone(),
                extension_occurrence: occurrence,
                uri:                  e.uri.clone().expect("known label URI"),
            })
            .collect())
    }

    pub(crate) fn read<R: std::io::BufRead>(
        reader: &mut Reader<R>,
        e: &BytesStart,
        empty: bool,
        session: &SourceCacheSession,
    ) -> Self {
        let context = session
            .extension_context(reader.buffer_position(), e)
            .expect("extension boundary requires original chart namespace context");
        let mut result = Self {
            entries:            Vec::new(),
            name:               context.container,
            owner:              context.owner,
            owner_path:         context.owner_path,
            chart_part:         context.chart_part,
            source_part_sha256: context.source_part_sha256,
            invalid:            false,
        };
        if empty {
            return result;
        }
        let config = reader.config().clone();
        reader.config_mut().trim_text(false);
        let end = e.name().into_inner().to_vec();
        let mut buf = Vec::new();
        let family = family_from_path(&result.owner_path);
        loop {
            buf.clear();
            match reader
                .read_event_into(&mut buf)
                .expect("valid extension-list XML")
            {
                ref event @ (Event::Start(ref child) | Event::Empty(ref child)) => {
                    let empty = matches!(event, Event::Empty(_));
                    let scope = context
                        .scope
                        .child(child)
                        .expect("valid extension namespace");
                    let name = scope
                        .resolve(child.name().into_inner(), false)
                        .expect("bound extension QName");
                    if name.namespace_uri == result.name.namespace_uri && name.local_name == "ext" {
                        result.entries.push(
                            read_extension(
                                reader,
                                child,
                                empty,
                                &context.scope,
                                &result.owner.local_name,
                                family,
                                session,
                            )
                            .expect("valid extension content"),
                        );
                    } else {
                        result.invalid = true;
                        if !empty {
                            crate::reader::driver::skip_element(reader, child);
                        }
                    }
                }
                Event::End(child) if child.name().into_inner() == end.as_slice() => break,
                Event::Text(text) => {
                    let value = text.decode().expect("extension whitespace");
                    result.invalid |= !value.chars().all(|c| matches!(c, ' ' | '\t' | '\r' | '\n'));
                }
                Event::Eof => panic!("unterminated extension list"),
                Event::Comment(_) | Event::PI(_) => (),
                _ => result.invalid = true,
            }
        }
        *reader.config_mut() = config;
        result
    }

    #[must_use]
    pub fn conflict(&self) -> Option<SeriesPartConflictReason> {
        if self.invalid {
            return Some(SeriesPartConflictReason::InvalidValue);
        }
        self.entries.iter().find_map(|entry| {
            entry.nodes.iter().find_map(node_conflict).or_else(|| {
                entry
                    .validate_namespaces()
                    .err()
                    .map(|_| SeriesPartConflictReason::InvalidValue)
            })
        })
    }

    pub(crate) fn conflict_for_owner(
        &self,
        owner: &str,
        family: Option<SeriesFamily>,
    ) -> Option<SeriesPartConflictReason> {
        if self.invalid {
            return Some(SeriesPartConflictReason::InvalidValue);
        }
        let family = family.map(family_name);
        self.entries.iter().find_map(|entry| {
            if owner == "spPr" && entry.uri.is_none() {
                return Some(SeriesPartConflictReason::MissingRequiredChild);
            }
            entry.conflict_for_owner(owner, family).or_else(|| {
                entry
                    .validate_namespaces()
                    .err()
                    .map(|_| SeriesPartConflictReason::InvalidValue)
            })
        })
    }

    /// Reports each invalid occurrence without mutating or normalizing storage.
    /// Workbook-less owners cannot silently generate replacement source caches.
    pub fn export_diagnostics_for_owner(
        &self,
        wb: Option<&Workbook>,
        owner: &str,
        family: Option<SeriesFamily>,
    ) -> Vec<ExtensionExportDiagnostic> {
        let mut diagnostics = Vec::new();
        for (occurrence, entry) in self.entries.iter().enumerate() {
            if entry.is_empty_known_label_shell(owner) {
                continue;
            }
            let reason = if self.invalid {
                Some("invalid extension-list content".into())
            } else if let Some(reason) = entry.conflict_for_owner(owner, family.map(family_name)) {
                Some(format!("{reason:?}"))
            } else {
                entry
                    .materialized(wb)
                    .and_then(|root| root.materialized_in(wb, &NamespaceScope::default()))
                    .err()
            };
            if let Some(reason) = reason {
                diagnostics.push(ExtensionExportDiagnostic {
                    chart_part: self.chart_part.clone(),
                    owner_path: self.owner_path.clone(),
                    extension_occurrence: occurrence,
                    uri: entry.uri.clone(),
                    reason,
                });
            }
        }
        diagnostics
    }

    pub(crate) fn remove_not_allowed_for_owner(
        &mut self,
        owner: &str,
        family: Option<SeriesFamily>,
    ) -> Vec<usize> {
        let mut removed = Vec::new();
        let mut occurrence = 0;
        self.entries.retain(|entry| {
            let keep = entry.conflict_for_owner(owner, family.map(family_name))
                != Some(SeriesPartConflictReason::NotAllowed);
            if !keep {
                removed.push(occurrence);
            }
            occurrence += 1;
            keep
        });
        removed
    }

    pub(crate) fn write_to(&self, writer: &mut Writer<Cursor<Vec<u8>>>, wb: &Workbook) {
        self.write_for_owner(writer, Some(wb), &self.owner.local_name, None);
    }

    pub(crate) fn write_without_workbook(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        self.write_for_owner(writer, None, &self.owner.local_name, None);
    }

    pub(crate) fn write_for_owner(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        wb: Option<&Workbook>,
        owner: &str,
        family: Option<SeriesFamily>,
    ) {
        let namespace = if owner == "spPr" {
            "http://schemas.openxmlformats.org/drawingml/2006/main"
        } else {
            NS_C
        };
        if self.invalid {
            return;
        }
        let mut rendered = Vec::new();
        for entry in &self.entries {
            if entry.is_empty_known_label_shell(owner)
                || (owner == "spPr" && entry.uri.is_none())
                || entry
                    .conflict_for_owner(owner, family.map(family_name))
                    .is_some()
            {
                continue;
            }
            if let Ok(element) = entry
                .materialized(wb)
                .and_then(|root| root.with_native_root(namespace, "ext"))
                .and_then(|entry| entry.materialized_in(wb, &NamespaceScope::default()))
            {
                rendered.push(element);
            }
        }
        // Preserve an empty imported list as an empty list. Known shell
        // omission alone never creates unknown extension normalization.
        let mut root = OpaqueElement {
            name:       QName {
                namespace_uri:    namespace.into(),
                local_name:       "extLst".into(),
                preferred_prefix: self.name.preferred_prefix.clone(),
            },
            namespaces: Vec::new(),
            attributes: Vec::new(),
            children:   Vec::new(),
        };
        for element in rendered {
            append_element(&mut root, element);
        }
        root.namespaces.push((
            root.name.preferred_prefix.clone(),
            root.name.namespace_uri.clone(),
        ));
        if root.validate(&NamespaceScope::default()).is_err() {
            return;
        }
        let mut staged = Writer::new(Cursor::new(Vec::new()));
        if root.emit(&mut staged).is_err() {
            return;
        }
        // Cursor<Vec<u8>> cannot fail an I/O write. Validation and
        // materialization completed atomically before the enclosing
        // chart writer is touched.
        writer
            .get_mut()
            .write_all(&staged.into_inner().into_inner())
            .expect("in-memory chart writer");
    }
}
fn read_extension<R: std::io::BufRead>(
    reader: &mut Reader<R>,
    e: &BytesStart,
    empty: bool,
    inherited: &NamespaceScope,
    owner: &str,
    family: Option<&str>,
    session: &SourceCacheSession,
) -> Result<Extension, String> {
    let scope = inherited.child(e)?;
    let name = scope.resolve(e.name().into_inner(), false)?;
    let mut entry = Extension {
        uri: None,
        namespaces: scope.effective_bindings(),
        attributes: Vec::new(),
        nodes: Vec::new(),
        name,
    };
    for attr in e.attributes() {
        let attr = attr.map_err(|e| e.to_string())?;
        let key = attr.key.into_inner();
        if key == b"xmlns" || key.starts_with(b"xmlns:") {
            continue;
        }
        let name = scope.resolve(key, true)?;
        let value = attr
            .normalized_value(quick_xml::XmlVersion::Implicit1_0)
            .map_err(|e| e.to_string())?
            .into_owned();
        if name.namespace_uri.is_empty() && name.local_name == "uri" {
            entry.uri = Some(value);
        } else {
            entry.attributes.push((name, value));
        }
    }
    if empty {
        return Ok(entry);
    }
    let end = e.name().into_inner().to_vec();
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match reader
            .read_event_into(&mut buf)
            .map_err(|e| e.to_string())?
        {
            ref event @ (Event::Start(ref child) | Event::Empty(ref child)) => {
                let empty = matches!(event, Event::Empty(_));
                let child_scope = scope.child(child)?;
                let name = child_scope.resolve(child.name().into_inner(), false)?;
                let registered = entry
                    .uri
                    .as_deref()
                    .is_some_and(|uri| registered(uri, owner, family, &name));
                let reference = name.namespace_uri == NS_C15
                    && matches!(
                        name.local_name.as_str(),
                        "fullRef" | "levelRef" | "formulaRef"
                    );
                if registered || reference {
                    entry
                        .nodes
                        .push(ExtensionNode::Known(Box::new(ExtensionPayload::read(
                            reader, child, empty, &scope, session,
                        )?)));
                } else {
                    entry.nodes.push(ExtensionNode::Unknown(Box::new(
                        OpaqueElement::read_promoted(reader, child, empty, &scope, session)?,
                    )));
                }
            }
            Event::Text(text) => {
                let text = text.decode().map_err(|e| e.to_string())?;
                push_text(
                    &mut entry.nodes,
                    quick_xml::escape::unescape(&text)
                        .map_err(|e| e.to_string())?
                        .into_owned(),
                );
            }
            Event::GeneralRef(reference) => {
                let text = format!("&{};", reference.decode().map_err(|e| e.to_string())?);
                push_text(
                    &mut entry.nodes,
                    quick_xml::escape::unescape(&text)
                        .map_err(|e| e.to_string())?
                        .into_owned(),
                );
            }
            Event::CData(text) => entry.nodes.push(ExtensionNode::CData(
                text.decode().map_err(|e| e.to_string())?.into_owned(),
            )),
            Event::Comment(text) => entry.nodes.push(ExtensionNode::Comment(
                text.decode().map_err(|e| e.to_string())?.into_owned(),
            )),
            Event::PI(pi) => entry.nodes.push(ExtensionNode::ProcessingInstruction(
                std::str::from_utf8(pi.as_ref())
                    .map_err(|e| e.to_string())?
                    .into(),
            )),
            Event::End(child) if child.name().into_inner() == end.as_slice() => return Ok(entry),
            Event::Eof => return Err("unterminated extension occurrence".into()),
            Event::End(_) | Event::Decl(_) | Event::DocType(_) => {
                return Err("invalid extension content".into());
            }
        }
    }
}
fn push_text(nodes: &mut Vec<ExtensionNode>, text: String) {
    if let Some(ExtensionNode::Text(previous)) = nodes.last_mut() {
        previous.push_str(&text);
    } else {
        nodes.push(ExtensionNode::Text(text));
    }
}
fn node_conflict(node: &ExtensionNode) -> Option<SeriesPartConflictReason> {
    match node {
        ExtensionNode::Known(v) => v.conflict(),
        ExtensionNode::Unknown(value) => value.children.iter().find_map(|token| match token {
            OpaqueToken::Reference(reference) => {
                ExtensionPayload::from_terminal(reference).conflict()
            }
            _ => None,
        }),
        _ => None,
    }
}
fn family_name(family: SeriesFamily) -> &'static str {
    match family {
        SeriesFamily::Area => "area",
        SeriesFamily::Bar => "bar",
        SeriesFamily::Bubble => "bubble",
        SeriesFamily::Line => "line",
        SeriesFamily::Pie => "pie",
        SeriesFamily::Radar => "radar",
        SeriesFamily::Scatter => "scatter",
    }
}
fn family_from_path(path: &str) -> Option<&'static str> {
    for (names, family) in [
        ("barChart|bar3DChart", "bar"),
        ("bubbleChart", "bubble"),
        ("areaChart|area3DChart", "area"),
        ("lineChart|line3DChart|stockChart", "line"),
        ("pieChart|pie3DChart|ofPieChart|doughnutChart", "pie"),
        ("radarChart", "radar"),
        ("scatterChart", "scatter"),
    ] {
        if names
            .split('|')
            .any(|name| path.contains(&format!("}}{name}[")))
        {
            return Some(family);
        }
    }
    None
}

fn xml_whitespace(text: &str) -> bool {
    text.chars().all(|c| matches!(c, ' ' | '\t' | '\r' | '\n'))
}

fn append_element(root: &mut OpaqueElement, child: OpaqueElement) {
    root.children.push(OpaqueToken::Start(Box::new(OpaqueStart {
        name:       child.name,
        namespaces: child.namespaces,
        attributes: child.attributes,
    })));
    root.children.extend(child.children);
    root.children.push(OpaqueToken::End);
}
