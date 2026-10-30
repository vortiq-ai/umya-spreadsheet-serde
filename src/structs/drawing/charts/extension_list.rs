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
    pub(crate) fn append_list(&mut self, other: Self) {
        let same_name = |left: &QName, right: &QName| {
            left.namespace_uri == right.namespace_uri && left.local_name == right.local_name
        };
        self.invalid |= other.invalid
            || !same_name(&self.name, &other.name)
            || !same_name(&self.owner, &other.owner)
            || self.owner_path != other.owner_path
            || self.chart_part != other.chart_part
            || self.source_part_sha256 != other.source_part_sha256;
        self.entries.extend(other.entries);
    }

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
        owner: &BytesStart,
        empty: bool,
        session: &SourceCacheSession,
    ) -> Result<Self, crate::XlsxError> {
        let context = session
            .extension_context(reader.buffer_position(), owner)
            .ok_or_else(|| crate::xml::xml_error("missing original extension namespace context"))?;
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
            return Ok(result);
        }
        let config = reader.config().clone();
        reader.config_mut().trim_text(false);
        let parsed = (|| {
            let end = owner.name().as_ref().to_vec();
            let mut buffer = Vec::new();
            let family = family_from_path(&result.owner_path);
            loop {
                match reader.read_event_into(&mut buffer)? {
                    ref event @ (Event::Start(ref child) | Event::Empty(ref child)) => {
                        let empty = matches!(event, Event::Empty(_));
                        let name = context
                            .scope
                            .child(child)
                            .and_then(|scope| scope.resolve(child.name().as_ref(), false))
                            .map_err(crate::xml::xml_error)?;
                        if name.namespace_uri == result.name.namespace_uri
                            && name.local_name == "ext"
                        {
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
                                .map_err(crate::xml::xml_error)?,
                            );
                        } else {
                            result.invalid = true;
                            if !empty {
                                crate::reader::driver::skip_element(reader, child)?;
                            }
                        }
                    }
                    Event::End(child) if child.name().as_ref() == end => return Ok(result),
                    ref event @ (Event::Text(_) | Event::CData(_) | Event::GeneralRef(_)) => {
                        let value = crate::reader::driver::decode_xml_character_event(event)?
                            .ok_or_else(|| crate::xml::xml_error("invalid extension list text"))?;
                        result.invalid |=
                            !value.chars().all(|c| matches!(c, ' ' | '\t' | '\r' | '\n'));
                    }
                    Event::Eof => return Err(crate::xml::xml_error("unterminated extension list")),
                    Event::Comment(_) | Event::PI(_) => (),
                    _ => result.invalid = true,
                }
                buffer.clear();
            }
        })();
        *reader.config_mut() = config;
        parsed
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

    fn omission_reason(
        &self,
        entry: &Extension,
        owner: &str,
        family: Option<SeriesFamily>,
    ) -> Option<String> {
        if self.invalid {
            return Some("invalid extension-list content".into());
        }
        entry
            .conflict_for_owner(owner, family.map(family_name))
            .map(|reason| format!("{reason:?}"))
            .or_else(|| entry.validate_namespaces().err())
    }

    /// Reports each invalid occurrence without mutating or normalizing storage.
    /// Workbook-less owners cannot silently generate replacement source caches.
    #[must_use]
    pub fn export_diagnostics_for_owner(
        &self,
        wb: Option<&Workbook>,
        owner: &str,
        family: Option<SeriesFamily>,
    ) -> Vec<ExtensionExportDiagnostic> {
        let mut diagnostics = Vec::new();
        if self.invalid && self.entries.is_empty() {
            diagnostics.push(ExtensionExportDiagnostic {
                chart_part:           self.chart_part.clone(),
                owner_path:           self.owner_path.clone(),
                extension_occurrence: 0,
                uri:                  None,
                reason:               "invalid extension-list content or missing namespace context"
                    .into(),
            });
        }
        for (occurrence, entry) in self.entries.iter().enumerate() {
            if entry.is_empty_known_label_shell(owner) {
                continue;
            }
            let reason = self.omission_reason(entry, owner, family).or_else(|| {
                entry
                    .materialized(wb)
                    .and_then(|root| root.materialized_in(wb, &NamespaceScope::default()))
                    .err()
            });
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

    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        wb: &Workbook,
    ) -> Result<(), crate::XlsxError> {
        self.write_for_owner(writer, Some(wb), &self.owner.local_name, None)
    }

    pub(crate) fn write_without_workbook(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
    ) -> Result<(), crate::XlsxError> {
        self.write_for_owner(writer, None, &self.owner.local_name, None)
    }

    pub(crate) fn write_for_owner(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        wb: Option<&Workbook>,
        owner: &str,
        family: Option<SeriesFamily>,
    ) -> Result<(), crate::XlsxError> {
        self.write_for_owner_in_scope(writer, wb, owner, family, &NamespaceScope::default())
    }

    pub(crate) fn write_for_owner_in_scope(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        wb: Option<&Workbook>,
        owner: &str,
        family: Option<SeriesFamily>,
        inherited: &NamespaceScope,
    ) -> Result<(), crate::XlsxError> {
        let namespace = if owner == "spPr" {
            "http://schemas.openxmlformats.org/drawingml/2006/main"
        } else {
            NS_C
        };
        if self.invalid {
            return Ok(());
        }
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
        for entry in &self.entries {
            if entry.is_empty_known_label_shell(owner) {
                continue;
            }
            // Stored invalid occurrences are diagnosed and omitted, not
            // repaired.
            if self.omission_reason(entry, owner, family).is_some() {
                continue;
            }
            let element = entry
                .materialized(wb)
                .and_then(|root| root.with_native_root(namespace, "ext"))
                .and_then(|entry| entry.materialized_in(wb, &NamespaceScope::default()))
                .map_err(crate::xml::xml_error)?;
            append_element(&mut root, element);
        }
        if root.children.is_empty() && !self.entries.is_empty() {
            return Ok(());
        }
        root.namespaces.push((
            root.name.preferred_prefix.clone(),
            root.name.namespace_uri.clone(),
        ));
        root.compact_namespaces(inherited)
            .map_err(crate::xml::xml_error)?;
        // Stage the validated list before appending it to the chart.
        let mut staged = Writer::new(Cursor::new(Vec::new()));
        root.write_to(&mut staged, inherited)
            .map_err(crate::xml::xml_error)?;
        writer
            .get_mut()
            .write_all(&staged.into_inner().into_inner())?;
        Ok(())
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
                // Surface filtering has a normative registration but no typed
                // Surface series model. Preserve its payload through the
                // existing opaque route rather than claiming typed support.
                let unmodeled = name.is(NS_C15, "filteredSurfaceSeries");
                if (registered || reference) && !unmodeled {
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

#[cfg(test)]
mod tests {
    use super::{
        super::extension_registry::URI_FILTER,
        *,
    };

    #[test]
    fn extension_rendering_errors_leave_the_destination_unchanged() {
        let name = |local: &str| QName {
            namespace_uri:    NS_C.into(),
            local_name:       local.into(),
            preferred_prefix: Some("c".into()),
        };
        let list = ExtensionList {
            entries:            vec![authored_extension(
                "ser",
                URI_FILTER,
                "<c15:filteredSeriesTitle><c15:tx><c:v>Title</c:v></c15:tx></c15:\
                 filteredSeriesTitle>",
            )],
            name:               name("extLst"),
            owner:              name("ser"),
            owner_path:         "/chartSpace/chart/plotArea/lineChart/ser".into(),
            chart_part:         "xl/charts/chart1.xml".into(),
            source_part_sha256: "a".repeat(64),
            invalid:            false,
        };
        assert!(
            list.omission_reason(&list.entries[0], "ser", None)
                .is_none()
        );
        let mut writer = Writer::new(Cursor::new(b"unchanged".to_vec()));
        let error = list.write_without_workbook(&mut writer).unwrap_err();
        assert_eq!(error.kind(), crate::XlsxErrorKind::Xml);
        assert!(error.to_string().contains("workbook"), "{error}");
        assert_eq!(writer.into_inner().into_inner(), b"unchanged");
    }

    #[test]
    fn appending_lists_retains_occurrences_and_rejects_mismatched_provenance() {
        let name = |local: &str| QName {
            namespace_uri:    NS_C.into(),
            local_name:       local.into(),
            preferred_prefix: Some("c".into()),
        };
        let source = ExtensionList {
            entries:            vec![authored_extension(
                "chartSpace",
                "urn:future",
                "<c15:future/>",
            )],
            name:               name("extLst"),
            owner:              name("chartSpace"),
            owner_path:         "/chartSpace[1]".into(),
            chart_part:         "xl/charts/chart1.xml".into(),
            source_part_sha256: "a".repeat(64),
            invalid:            false,
        };
        let mut compatible = source.clone();
        let mut alias = source.clone();
        alias.name.preferred_prefix = Some("alias".into());
        alias.owner.preferred_prefix = Some("alias".into());
        compatible.append_list(alias);
        assert_eq!(compatible.entries.len(), 2);
        assert_eq!(compatible.conflict(), None);
        for changed in ["invalid", "name", "owner", "path", "part", "hash"] {
            let mut current = source.clone();
            let mut incoming = source.clone();
            match changed {
                "invalid" => incoming.invalid = true,
                "name" => incoming.name.namespace_uri = "urn:foreign".into(),
                "owner" => incoming.owner.local_name = "chart".into(),
                "path" => incoming.owner_path.push_str("/other[1]"),
                "part" => incoming.chart_part = "xl/charts/chart2.xml".into(),
                "hash" => incoming.source_part_sha256 = "b".repeat(64),
                _ => unreachable!(),
            }
            current.append_list(incoming);
            assert_eq!(current.entries.len(), 2, "{changed}");
            assert_eq!(current.owner_path, source.owner_path);
            assert_eq!(current.chart_part, source.chart_part);
            assert!(current.conflict().is_some(), "{changed}");
        }
    }

    fn authored_extension(owner: &str, uri: &str, payload: &str) -> Extension {
        let xml = format!(
            "<c:ext xmlns:c=\"{NS_C}\" xmlns:c15=\"{NS_C15}\" uri=\"{uri}\">{payload}</c:ext>"
        );
        let mut reader = Reader::from_str(&xml);
        let Event::Start(start) = reader.read_event().unwrap() else {
            panic!("authored extension start");
        };
        let entry = read_extension(
            &mut reader,
            &start,
            false,
            &NamespaceScope::default(),
            owner,
            None,
            &SourceCacheSession::deferred(),
        )
        .expect("well-formed registered or opaque extension must not fail typed dispatch");
        assert!(matches!(reader.read_event().unwrap(), Event::Eof));
        entry
    }

    #[test]
    fn registered_unmodeled_surface_payloads_are_preserved_without_typed_dispatch_failure() {
        for owner in ["surfaceChart", "surface3DChart"] {
            for payload in [
                "<c15:filteredSurfaceSeries/>",
                "<s:filteredSurfaceSeries xmlns:s=\"http://schemas.microsoft.com/office/drawing/2012/chart\" keep=\"opaque\"><s:ser><c:idx val=\"7\"/></s:ser></s:filteredSurfaceSeries>",
            ] {
                let entry = authored_extension(owner, URI_FILTER, payload);
                assert_eq!(entry.nodes.len(), 1);
                let ExtensionNode::Unknown(value) = &entry.nodes[0] else {
                    panic!("unmodeled surface payload must not claim typed support");
                };
                assert!(registered(entry.uri.as_deref().unwrap(), owner, None, &value.name));
                assert!(value.name.is(NS_C15, "filteredSurfaceSeries"));
                assert_eq!(entry.conflict_for_owner(owner, None), None);
                let rendered = entry.materialized(None).unwrap();
                let mut writer = Writer::new(Cursor::new(Vec::new()));
                rendered.emit(&mut writer).unwrap();
                let xml = String::from_utf8(writer.into_inner().into_inner()).unwrap();
                assert!(xml.contains("filteredSurfaceSeries"));
                if payload.contains("keep=") {
                    assert!(xml.contains("keep=\"opaque\""));
                    assert!(xml.contains("val=\"7\""));
                }
            }
        }
    }

    #[test]
    fn unmodeled_surface_payload_still_rejects_a_wrong_registered_owner() {
        let entry = authored_extension("lineChart", URI_FILTER, "<c15:filteredSurfaceSeries/>");
        assert!(matches!(entry.nodes[0], ExtensionNode::Unknown(_)));
        assert_eq!(
            entry.conflict_for_owner("lineChart", None),
            Some(SeriesPartConflictReason::NotAllowed)
        );
    }

    #[test]
    fn payload_variants_without_selected_registration_are_not_promoted() {
        for (owner, uri, payload) in [
            ("pieChart", URI_FILTER, "<c15:filteredPieSeries/>"),
            (
                "dLbls",
                URI_LABEL,
                "<c15:numFmt formatCode=\"0\" sourceLinked=\"0\"/>",
            ),
            ("dLbls", URI_LABEL, "<c15:autoCat val=\"1\"/>"),
        ] {
            let entry = authored_extension(owner, uri, payload);
            assert!(matches!(entry.nodes[0], ExtensionNode::Unknown(_)));
            assert_eq!(
                entry.conflict_for_owner(owner, None),
                Some(SeriesPartConflictReason::NotAllowed)
            );
        }
    }

    #[test]
    fn registrations_outside_the_selected_uri_set_remain_opaque() {
        for (owner, uri, payload) in [
            (
                "catAx",
                "{F40574EE-89B7-4290-83BB-5DA773EAF853}",
                "<c15:numFmt formatCode=\"0\" sourceLinked=\"0\"/>",
            ),
            (
                "strLit",
                "{E218EAE0-A670-482B-9A64-EF95B99D9540}",
                "<c15:autoCat val=\"1\"/>",
            ),
        ] {
            assert!(!known_uri(uri));
            let entry = authored_extension(owner, uri, payload);
            assert!(matches!(entry.nodes[0], ExtensionNode::Unknown(_)));
            assert_eq!(entry.conflict_for_owner(owner, None), None);
            assert!(entry.materialized(None).is_ok());
        }
    }

    #[test]
    fn missing_namespace_context_returns_an_error_before_consuming_content() {
        for xml in [
            "<c:extLst xmlns:c=\"http://schemas.openxmlformats.org/drawingml/2006/chart\"/>",
            "<c:extLst xmlns:c=\"http://schemas.openxmlformats.org/drawingml/2006/chart\"><c:ext \
             uri=\"urn:future\"/></c:extLst>",
        ] {
            let mut reader = Reader::from_str(xml);
            let event = reader.read_event().unwrap();
            let empty = matches!(event, Event::Empty(_));
            let (Event::Start(start) | Event::Empty(start)) = event else {
                panic!("extension-list start");
            };
            let before = reader.buffer_position();
            let error =
                ExtensionList::read(&mut reader, &start, empty, &SourceCacheSession::deferred())
                    .unwrap_err();
            assert!(
                error
                    .to_string()
                    .contains("missing original extension namespace context")
            );
            assert_eq!(reader.buffer_position(), before);
        }
    }
}
