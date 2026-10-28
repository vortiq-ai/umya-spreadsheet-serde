//! Unknown chart-extension content in an authoritative flat preorder tape.
//! Known nodes are replaced with typed terminal reference records; no raw XML
//! shadow or recursive opaque ownership is retained.
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::{
        BytesCData,
        BytesEnd,
        BytesPI,
        BytesStart,
        BytesText,
        Event,
    },
};

use super::extension_names::{
    NamespaceScope,
    QName,
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct OpaqueElement {
    pub name:       QName,
    pub namespaces: Vec<(Option<String>, String)>,
    pub attributes: Vec<(QName, String)>,
    /// Flat preorder child tape. Start/End pairs delimit nested elements.
    pub children:   Vec<OpaqueToken>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct OpaqueStart {
    pub name:       QName,
    pub namespaces: Vec<(Option<String>, String)>,
    pub attributes: Vec<(QName, String)>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerminalReferenceKind {
    Full,
    Level,
    Formula,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct TerminalReference {
    pub(crate) kind:             TerminalReferenceKind,
    pub(crate) preferred_prefix: Option<String>,
    pub(crate) namespaces:       Vec<(Option<String>, String)>,
    pub(crate) attributes:       Vec<(QName, String)>,
    pub(crate) sequence:         Box<super::ReferenceSequence>,
    pub(crate) invalid:          bool,
}
impl TerminalReference {
    #[must_use]
    pub fn kind(&self) -> TerminalReferenceKind {
        self.kind
    }

    #[must_use]
    pub fn sequence(&self) -> &super::ReferenceSequence {
        &self.sequence
    }

    pub fn sequence_mut(&mut self) -> &mut super::ReferenceSequence {
        &mut self.sequence
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub enum OpaqueToken {
    Start(Box<OpaqueStart>),
    End,
    Reference(Box<TerminalReference>),
    Text(String),
    CData(String),
    Comment(String),
    ProcessingInstruction(String),
}

// The enclosing extension list still owns its known/unknown top-level nodes.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub enum ExtensionNode {
    Known(Box<super::ExtensionPayload>),
    Unknown(Box<OpaqueElement>),
    Text(String),
    CData(String),
    Comment(String),
    ProcessingInstruction(String),
}
pub type OpaqueNode = ExtensionNode;

fn read_start(
    e: &BytesStart,
    inherited: &NamespaceScope,
) -> Result<(OpaqueStart, NamespaceScope), String> {
    let scope = inherited.child(e)?;
    let mut attributes = Vec::new();
    for attr in e.attributes() {
        let attr = attr.map_err(|e| e.to_string())?;
        let raw = attr.key.into_inner();
        if raw == b"xmlns" || raw.starts_with(b"xmlns:") {
            continue;
        }
        attributes.push((
            scope.resolve(raw, true)?,
            attr.normalized_value(quick_xml::XmlVersion::Implicit1_0)
                .map_err(|e| e.to_string())?
                .into_owned(),
        ));
    }
    Ok((
        OpaqueStart {
            name: scope.resolve(e.name().into_inner(), false)?,
            namespaces: NamespaceScope::declarations(e)?,
            attributes,
        },
        scope,
    ))
}

fn start_scope(start: &OpaqueStart, inherited: &NamespaceScope) -> Result<NamespaceScope, String> {
    let mut e = BytesStart::new(start.name.lexical());
    for (prefix, uri) in &start.namespaces {
        let key = prefix
            .as_ref()
            .map_or_else(|| "xmlns".into(), |p| format!("xmlns:{p}"));
        e.push_attribute((key.as_str(), uri.as_str()));
    }
    inherited.child(&e)
}

fn validate_start(
    start: &OpaqueStart,
    inherited: &NamespaceScope,
) -> Result<NamespaceScope, String> {
    let scope = start_scope(start, inherited)?;
    if scope.resolve(start.name.lexical().as_bytes(), false)? != start.name {
        return Err("opaque element binding changed".into());
    }
    let mut names = std::collections::HashSet::new();
    for (name, _) in &start.attributes {
        if !names.insert((&name.namespace_uri, &name.local_name)) {
            return Err("duplicate expanded attribute name".into());
        }
        if scope.resolve(name.lexical().as_bytes(), true)? != *name {
            return Err("opaque attribute binding changed".into());
        }
    }
    Ok(scope)
}

fn emit_start(start: &OpaqueStart, writer: &mut Writer<Cursor<Vec<u8>>>) -> Result<(), String> {
    let name = start.name.lexical();
    let mut e = BytesStart::new(name);
    for (prefix, uri) in &start.namespaces {
        let key = prefix
            .as_ref()
            .map_or_else(|| "xmlns".into(), |p| format!("xmlns:{p}"));
        e.push_attribute((key.as_str(), uri.as_str()));
    }
    for (name, value) in &start.attributes {
        let name = name.lexical();
        e.push_attribute((name.as_str(), value.as_str()));
    }
    writer
        .write_event(Event::Start(e))
        .map_err(|e| e.to_string())
}

impl OpaqueElement {
    fn root_start(&self) -> OpaqueStart {
        OpaqueStart {
            name:       self.name.clone(),
            namespaces: self.namespaces.clone(),
            attributes: self.attributes.clone(),
        }
    }

    pub(crate) fn read<R: std::io::BufRead>(
        reader: &mut Reader<R>,
        e: &BytesStart,
        empty: bool,
        inherited: &NamespaceScope,
    ) -> Result<Self, String> {
        let config = reader.config().clone();
        reader.config_mut().trim_text(false);
        let result = Self::read_inner(reader, e, empty, inherited, None);
        *reader.config_mut() = config;
        result
    }

    pub(crate) fn read_promoted<R: std::io::BufRead>(
        reader: &mut Reader<R>,
        e: &BytesStart,
        empty: bool,
        inherited: &NamespaceScope,
        session: &crate::reader::xlsx::SourceCacheSession,
    ) -> Result<Self, String> {
        let config = reader.config().clone();
        reader.config_mut().trim_text(false);
        let result = Self::read_inner(reader, e, empty, inherited, Some(session));
        *reader.config_mut() = config;
        result
    }

    fn read_inner<R: std::io::BufRead>(
        reader: &mut Reader<R>,
        e: &BytesStart,
        empty: bool,
        inherited: &NamespaceScope,
        session: Option<&crate::reader::xlsx::SourceCacheSession>,
    ) -> Result<Self, String> {
        let (root, scope) = read_start(e, inherited)?;
        let mut result = Self {
            name:       root.name,
            namespaces: root.namespaces,
            attributes: root.attributes,
            children:   Vec::new(),
        };
        if empty {
            return Ok(result);
        }
        let mut frames = vec![(scope, e.name().into_inner().to_vec())];
        let mut buf = Vec::new();
        loop {
            match reader
                .read_event_into(&mut buf)
                .map_err(|e| e.to_string())?
            {
                ref event @ (Event::Start(ref child) | Event::Empty(ref child)) => {
                    let child_empty = matches!(event, Event::Empty(_));
                    let scope = &frames.last().expect("root frame exists").0;
                    let child_scope = scope.child(child)?;
                    let name = child_scope.resolve(child.name().into_inner(), false)?;
                    if let Some(session) = session.filter(|_| {
                        name.namespace_uri == super::extension_registry::NS_C15
                            && matches!(
                                name.local_name.as_str(),
                                "fullRef" | "levelRef" | "formulaRef"
                            )
                    }) {
                        let value = super::ExtensionPayload::read(
                            reader,
                            child,
                            child_empty,
                            scope,
                            session,
                        )?
                        .into_terminal()?;
                        result
                            .children
                            .push(OpaqueToken::Reference(Box::new(value)));
                    } else {
                        let (start, child_scope) = read_start(child, scope)?;
                        result.children.push(OpaqueToken::Start(Box::new(start)));
                        if child_empty {
                            result.children.push(OpaqueToken::End);
                        } else {
                            frames.push((child_scope, child.name().into_inner().to_vec()));
                        }
                    }
                }
                Event::Text(text) => {
                    let value = text.decode().map_err(|e| e.to_string())?;
                    result.push_text(
                        quick_xml::escape::unescape(&value)
                            .map_err(|e| e.to_string())?
                            .into_owned(),
                    );
                }
                Event::GeneralRef(reference) => {
                    let value = format!("&{};", reference.decode().map_err(|e| e.to_string())?);
                    result.push_text(
                        quick_xml::escape::unescape(&value)
                            .map_err(|e| e.to_string())?
                            .into_owned(),
                    );
                }
                Event::CData(v) => result.children.push(OpaqueToken::CData(
                    v.decode().map_err(|e| e.to_string())?.into_owned(),
                )),
                Event::Comment(v) => result.children.push(OpaqueToken::Comment(
                    v.decode().map_err(|e| e.to_string())?.into_owned(),
                )),
                Event::PI(v) => result.children.push(OpaqueToken::ProcessingInstruction(
                    std::str::from_utf8(v.as_ref())
                        .map_err(|e| e.to_string())?
                        .to_owned(),
                )),
                Event::End(v) => {
                    if v.name().into_inner()
                        != frames.last().expect("root frame exists").1.as_slice()
                    {
                        return Err("mismatched opaque QName".into());
                    }
                    frames.pop();
                    if frames.is_empty() {
                        return Ok(result);
                    }
                    result.children.push(OpaqueToken::End);
                }
                Event::Eof => return Err("unterminated opaque element".into()),
                Event::DocType(_) | Event::Decl(_) => {
                    return Err("declarations are not permitted in extension content".into());
                }
            }
            buf.clear();
        }
    }

    fn push_text(&mut self, text: String) {
        if let Some(OpaqueToken::Text(previous)) = self.children.last_mut() {
            previous.push_str(&text);
        } else {
            self.children.push(OpaqueToken::Text(text));
        }
    }

    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        inherited: &NamespaceScope,
    ) -> Result<(), String> {
        let rendered = self.materialized_in(None, inherited)?;
        rendered.validate(inherited)?;
        rendered.emit(writer)
    }

    pub(crate) fn materialized(
        &self,
        wb: Option<&crate::structs::Workbook>,
    ) -> Result<Self, String> {
        self.materialized_in(wb, &NamespaceScope::default())
    }

    pub(crate) fn materialized_in(
        &self,
        wb: Option<&crate::structs::Workbook>,
        inherited: &NamespaceScope,
    ) -> Result<Self, String> {
        self.validate(inherited)?;
        let mut scopes = vec![start_scope(&self.root_start(), inherited)?];
        let mut result = Self {
            name:       self.name.clone(),
            namespaces: self.namespaces.clone(),
            attributes: self.attributes.clone(),
            children:   Vec::new(),
        };
        for token in &self.children {
            match token {
                OpaqueToken::Start(start) => {
                    scopes.push(start_scope(start, scopes.last().unwrap())?);
                    result.children.push(token.clone());
                }
                OpaqueToken::End => {
                    if scopes.len() == 1 {
                        return Err("unmatched opaque end".into());
                    }
                    scopes.pop();
                    result.children.push(OpaqueToken::End);
                }
                OpaqueToken::Reference(reference) => {
                    let mut rendered =
                        super::ExtensionPayload::from_terminal(reference).rendered(wb)?;
                    rendered.fix_namespaces(scopes.last().unwrap())?;
                    rendered.validate(scopes.last().unwrap())?;
                    result
                        .children
                        .push(OpaqueToken::Start(Box::new(OpaqueStart {
                            name:       rendered.name,
                            namespaces: rendered.namespaces,
                            attributes: rendered.attributes,
                        })));
                    result.children.extend(rendered.children);
                    result.children.push(OpaqueToken::End);
                }
                _ => result.children.push(token.clone()),
            }
        }
        if scopes.len() != 1 {
            return Err("unterminated opaque subtree".into());
        }
        Ok(result)
    }

    pub(crate) fn with_native_root(mut self, namespace: &str, local: &str) -> Result<Self, String> {
        self.validate(&NamespaceScope::default())?;
        self.name.namespace_uri = namespace.into();
        self.name.local_name = local.into();
        let mut scope = start_scope(&self.root_start(), &NamespaceScope::default())?;
        Self::fix_name(&mut self.name, false, &mut scope, &mut self.namespaces)?;
        Ok(self)
    }

    pub(crate) fn fix_namespaces(&mut self, inherited: &NamespaceScope) -> Result<(), String> {
        let mut root = self.root_start();
        let scope = Self::fix_start(&mut root, inherited)?;
        self.name = root.name;
        self.namespaces = root.namespaces;
        self.attributes = root.attributes;
        let mut scopes = vec![scope];
        for token in &mut self.children {
            match token {
                OpaqueToken::Start(start) => {
                    let scope = Self::fix_start(start, scopes.last().unwrap())?;
                    scopes.push(scope);
                }
                OpaqueToken::End => {
                    if scopes.len() == 1 {
                        return Err("unmatched opaque end".into());
                    }
                    scopes.pop();
                }
                _ => {}
            }
        }
        if scopes.len() != 1 {
            return Err("unterminated opaque subtree".into());
        }
        Ok(())
    }

    fn fix_start(
        start: &mut OpaqueStart,
        inherited: &NamespaceScope,
    ) -> Result<NamespaceScope, String> {
        let mut unique: Vec<(Option<String>, String)> = Vec::new();
        for (prefix, uri) in &start.namespaces {
            if let Some((_, old)) = unique.iter_mut().find(|(p, _)| p == prefix) {
                old.clone_from(uri);
            } else {
                unique.push((prefix.clone(), uri.clone()));
            }
        }
        start.namespaces = unique;
        let mut scope = start_scope(start, inherited)?;
        Self::fix_name(&mut start.name, false, &mut scope, &mut start.namespaces)?;
        for (name, _) in &mut start.attributes {
            Self::fix_name(name, true, &mut scope, &mut start.namespaces)?;
        }
        Ok(scope)
    }

    fn fix_name(
        name: &mut QName,
        attribute: bool,
        scope: &mut NamespaceScope,
        declarations: &mut Vec<(Option<String>, String)>,
    ) -> Result<(), String> {
        if scope
            .resolve(name.lexical().as_bytes(), attribute)
            .is_ok_and(|resolved| resolved == *name)
        {
            return Ok(());
        }
        if name.namespace_uri.is_empty() {
            if attribute {
                name.preferred_prefix = None;
                return Ok(());
            }
            declarations.push((None, String::new()));
            *scope = NamespaceScope::with_bindings(
                scope
                    .effective_bindings()
                    .into_iter()
                    .chain([(None, String::new())]),
            );
            name.preferred_prefix = None;
            return Ok(());
        }
        if let Some((prefix, _)) = scope
            .effective_bindings()
            .iter()
            .find(|(prefix, uri)| *uri == name.namespace_uri && (!attribute || prefix.is_some()))
        {
            name.preferred_prefix = prefix.clone();
            return Ok(());
        }
        let mut next = 1;
        loop {
            let prefix = format!("chartns{next}");
            next += 1;
            if scope
                .bindings()
                .iter()
                .any(|(p, _)| p.as_deref() == Some(prefix.as_str()))
            {
                continue;
            }
            let binding = (Some(prefix.clone()), name.namespace_uri.clone());
            declarations.push(binding.clone());
            *scope = NamespaceScope::with_bindings(
                scope.effective_bindings().into_iter().chain([binding]),
            );
            name.preferred_prefix = Some(prefix);
            return Ok(());
        }
    }

    pub(crate) fn validate(&self, inherited: &NamespaceScope) -> Result<(), String> {
        let mut scopes = vec![validate_start(&self.root_start(), inherited)?];
        for token in &self.children {
            match token {
                OpaqueToken::Start(start) => {
                    let scope = validate_start(start, scopes.last().unwrap())?;
                    scopes.push(scope);
                }
                OpaqueToken::End => {
                    if scopes.len() == 1 {
                        return Err("unmatched opaque end".into());
                    }
                    scopes.pop();
                }
                OpaqueToken::Reference(v) => super::ExtensionPayload::from_terminal(v)
                    .validate_namespaces(scopes.last().unwrap())?,
                OpaqueToken::CData(v) if v.contains("]]>") => {
                    return Err("CDATA terminator in content".into());
                }
                OpaqueToken::Comment(v) if v.contains("--") || v.ends_with('-') => {
                    return Err("invalid XML comment".into());
                }
                OpaqueToken::ProcessingInstruction(v) if v.contains("?>") => {
                    return Err("invalid processing instruction".into());
                }
                _ => {}
            }
        }
        if scopes.len() != 1 {
            return Err("unterminated opaque subtree".into());
        }
        Ok(())
    }

    pub(crate) fn emit(&self, writer: &mut Writer<Cursor<Vec<u8>>>) -> Result<(), String> {
        emit_start(&self.root_start(), writer)?;
        let mut names = Vec::new();
        for token in &self.children {
            match token {
                OpaqueToken::Start(v) => {
                    emit_start(v, writer)?;
                    names.push(v.name.lexical());
                }
                OpaqueToken::End => {
                    let name = names.pop().ok_or("unmatched opaque end")?;
                    writer
                        .write_event(Event::End(BytesEnd::new(name)))
                        .map_err(|e| e.to_string())?;
                }
                OpaqueToken::Reference(_) => return Err("typed node was not materialized".into()),
                OpaqueToken::Text(v) => writer
                    .write_event(Event::Text(BytesText::new(v)))
                    .map_err(|e| e.to_string())?,
                OpaqueToken::CData(v) => writer
                    .write_event(Event::CData(BytesCData::new(v)))
                    .map_err(|e| e.to_string())?,
                OpaqueToken::Comment(v) => writer
                    .write_event(Event::Comment(BytesText::from_escaped(v)))
                    .map_err(|e| e.to_string())?,
                OpaqueToken::ProcessingInstruction(v) => writer
                    .write_event(Event::PI(BytesPI::new(v)))
                    .map_err(|e| e.to_string())?,
            }
        }
        if !names.is_empty() {
            return Err("unterminated opaque subtree".into());
        }
        writer
            .write_event(Event::End(BytesEnd::new(self.name.lexical())))
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

impl ExtensionNode {}
