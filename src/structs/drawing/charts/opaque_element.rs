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

/// A start tag read in place; `first` is checked before `attributes` (an
/// extension's `uri`).
pub(crate) struct StartView<'a> {
    pub(crate) name:       &'a QName,
    pub(crate) namespaces: &'a [(Option<String>, String)],
    pub(crate) first:      Option<&'a QName>,
    pub(crate) attributes: &'a [(QName, String)],
}
impl<'a> From<&'a OpaqueStart> for StartView<'a> {
    fn from(start: &'a OpaqueStart) -> Self {
        Self {
            name:       &start.name,
            namespaces: &start.namespaces,
            first:      None,
            attributes: &start.attributes,
        }
    }
}

/// One token of a tape, read in place.
pub(crate) enum TapeItem<'a> {
    Start(StartView<'a>),
    End,
    Reference(&'a TerminalReference),
    CData(&'a str),
    Comment(&'a str),
    ProcessingInstruction(&'a str),
    Text,
}
impl<'a> From<&'a OpaqueToken> for TapeItem<'a> {
    fn from(token: &'a OpaqueToken) -> Self {
        match token {
            OpaqueToken::Start(start) => Self::Start(start.as_ref().into()),
            OpaqueToken::End => Self::End,
            OpaqueToken::Reference(reference) => Self::Reference(reference),
            OpaqueToken::CData(value) => Self::CData(value),
            OpaqueToken::Comment(value) => Self::Comment(value),
            OpaqueToken::ProcessingInstruction(value) => Self::ProcessingInstruction(value),
            OpaqueToken::Text(_) => Self::Text,
        }
    }
}

/// `validate`'s walk over a tape fed one token at a time.
pub(crate) struct TapeCheck {
    scopes: Vec<NamespaceScope>,
}
impl TapeCheck {
    pub(crate) fn new(root: &StartView<'_>, inherited: &NamespaceScope) -> Result<Self, String> {
        Ok(Self {
            scopes: vec![validate_start(root, inherited)?],
        })
    }

    pub(crate) fn item(&mut self, token: TapeItem<'_>) -> Result<(), String> {
        match token {
            TapeItem::Start(start) => {
                let scope = validate_start(&start, self.scopes.last().unwrap())?;
                self.scopes.push(scope);
            }
            TapeItem::End => {
                if self.scopes.len() == 1 {
                    return Err("unmatched opaque end".into());
                }
                self.scopes.pop();
            }
            TapeItem::Reference(v) => super::ExtensionPayload::from_terminal(v)
                .validate_namespaces(self.scopes.last().unwrap())?,
            TapeItem::CData(v) if v.contains("]]>") => {
                return Err("CDATA terminator in content".into());
            }
            TapeItem::Comment(v) if v.contains("--") || v.ends_with('-') => {
                return Err("invalid XML comment".into());
            }
            TapeItem::ProcessingInstruction(v) if v.contains("?>") => {
                return Err("invalid processing instruction".into());
            }
            _ => {}
        }
        Ok(())
    }

    pub(crate) fn finish(self) -> Result<(), String> {
        if self.scopes.len() != 1 {
            return Err("unterminated opaque subtree".into());
        }
        Ok(())
    }
}

fn start_scope(start: &OpaqueStart, inherited: &NamespaceScope) -> Result<NamespaceScope, String> {
    scope_of(&start.name, &start.namespaces, inherited)
}

fn scope_of(
    name: &QName,
    namespaces: &[(Option<String>, String)],
    inherited: &NamespaceScope,
) -> Result<NamespaceScope, String> {
    let mut e = BytesStart::new(name.lexical());
    for (prefix, uri) in namespaces {
        let key = prefix
            .as_ref()
            .map_or_else(|| "xmlns".into(), |p| format!("xmlns:{p}"));
        e.push_attribute((key.as_str(), uri.as_str()));
    }
    inherited.child(&e)
}

fn validate_start(
    start: &StartView<'_>,
    inherited: &NamespaceScope,
) -> Result<NamespaceScope, String> {
    let scope = scope_of(start.name, start.namespaces, inherited)?;
    if !scope.resolves_to(start.name, false)? {
        return Err("opaque element binding changed".into());
    }
    let mut names = std::collections::HashSet::new();
    for name in start
        .first
        .into_iter()
        .chain(start.attributes.iter().map(|(name, _)| name))
    {
        if !names.insert((&name.namespace_uri, &name.local_name)) {
            return Err("duplicate expanded attribute name".into());
        }
        if !scope.resolves_to(name, true)? {
            return Err("opaque attribute binding changed".into());
        }
    }
    Ok(scope)
}

/// HTML void elements: an HTML parser treats a written end tag for them as a
/// second element, so an empty, unprefixed one is written as `<name/>`. The
/// VML textbox reader closes exactly the same set of elements.
const HTML_VOID_ELEMENTS: [&str; 14] = [
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];

pub(crate) fn is_html_void_name(name: &[u8]) -> bool {
    HTML_VOID_ELEMENTS
        .iter()
        .any(|void| name.eq_ignore_ascii_case(void.as_bytes()))
}

fn start_event(start: &OpaqueStart) -> BytesStart<'static> {
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
    e
}

fn emit_start(start: &OpaqueStart, writer: &mut Writer<Cursor<Vec<u8>>>) -> Result<(), String> {
    writer
        .write_event(Event::Start(start_event(start)))
        .map_err(|e| e.to_string())
}

fn emit_empty(start: &OpaqueStart, writer: &mut Writer<Cursor<Vec<u8>>>) -> Result<(), String> {
    writer
        .write_event(Event::Empty(start_event(start)))
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

    pub(crate) fn view(&self) -> StartView<'_> {
        StartView {
            name:       &self.name,
            namespaces: &self.namespaces,
            first:      None,
            attributes: &self.attributes,
        }
    }

    /// Whether the child tape closes every element it opens, and no more.
    pub(crate) fn balanced(&self) -> bool {
        let mut depth = 0_usize;
        for token in &self.children {
            match token {
                OpaqueToken::Start(_) => depth += 1,
                OpaqueToken::End if depth == 0 => return false,
                OpaqueToken::End => depth -= 1,
                _ => (),
            }
        }
        depth == 0
    }

    /// The start tag of this element: namespace declarations, then attributes.
    pub(crate) fn start_event(&self) -> BytesStart<'static> {
        start_event(&self.root_start())
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

    pub(crate) fn read_detached<R: std::io::BufRead>(
        reader: &mut Reader<R>,
        element: &BytesStart,
        empty: bool,
        inherited: &NamespaceScope,
    ) -> Result<Self, String> {
        let mut value = Self::read(reader, element, empty, inherited)?;
        value.namespaces = inherited
            .child(element)?
            .effective_bindings()
            .into_iter()
            .filter(|(prefix, _)| prefix.as_deref() != Some("xml"))
            .collect();
        Ok(value)
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
                    if session.is_some_and(|_| {
                        name.namespace_uri == super::extension_registry::NS_C15
                            && matches!(
                                name.local_name.as_str(),
                                "fullRef" | "levelRef" | "formulaRef"
                            )
                    }) {
                        let candidate = Self::read_inner(reader, child, child_empty, scope, None)?;
                        if let Some(reference) = candidate.terminal_reference() {
                            result
                                .children
                                .push(OpaqueToken::Reference(Box::new(reference)));
                        } else {
                            result
                                .children
                                .push(OpaqueToken::Start(Box::new(OpaqueStart {
                                    name:       candidate.name,
                                    namespaces: candidate.namespaces,
                                    attributes: candidate.attributes,
                                })));
                            result.children.extend(candidate.children);
                            result.children.push(OpaqueToken::End);
                        }
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

    /// A terminal cannot retain metadata or mixed content on its `sqref` child.
    /// Leave every other shape on the opaque tape, without a typed shadow.
    fn terminal_reference(&self) -> Option<TerminalReference> {
        let [
            OpaqueToken::Start(start),
            OpaqueToken::Text(text),
            OpaqueToken::End,
        ] = self.children.as_slice()
        else {
            return None;
        };
        if !start.name.is(super::extension_registry::NS_C15, "sqref")
            || !start.namespaces.is_empty()
            || !start.attributes.is_empty()
        {
            return None;
        }
        let mut sequence = super::ReferenceSequence::default();
        sequence.set_text(text);
        if !sequence.is_valid() {
            return None;
        }
        let kind = match self.name.local_name.as_str() {
            "fullRef" => TerminalReferenceKind::Full,
            "levelRef" => TerminalReferenceKind::Level,
            "formulaRef" => TerminalReferenceKind::Formula,
            _ => return None,
        };
        Some(TerminalReference {
            kind,
            preferred_prefix: self.name.preferred_prefix.clone(),
            namespaces: self.namespaces.clone(),
            attributes: self.attributes.clone(),
            sequence: Box::new(sequence),
            invalid: false,
        })
    }

    pub(crate) fn write_to(
        &self,
        writer: &mut Writer<Cursor<Vec<u8>>>,
        inherited: &NamespaceScope,
    ) -> Result<(), String> {
        let mut rendered = self.materialized_in(None, inherited)?;
        let inherited_bindings = inherited.effective_bindings();
        rendered
            .namespaces
            .retain(|binding| !inherited_bindings.contains(binding));
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
        self.references_rendered(wb, inherited)
    }

    /// `materialized_in` for a tree that is validated in `inherited`, or whose
    /// checks its entry already implies when it `writes_as_checked`: the tree
    /// itself when it holds no typed reference.
    pub(crate) fn with_references_rendered(
        self,
        wb: Option<&crate::structs::Workbook>,
        inherited: &NamespaceScope,
    ) -> Result<Self, String> {
        if !self
            .children
            .iter()
            .any(|token| matches!(token, OpaqueToken::Reference(_)))
        {
            return Ok(self);
        }
        self.references_rendered(wb, inherited)
    }

    fn references_rendered(
        &self,
        wb: Option<&crate::structs::Workbook>,
        inherited: &NamespaceScope,
    ) -> Result<Self, String> {
        let mut scopes = vec![scope_of(&self.name, &self.namespaces, inherited)?];
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

    /// The tree under the native root name. The tree is validated in the
    /// default scope, or implied valid by its entry's checks when it
    /// `writes_as_checked`; renaming keeps it valid.
    pub(crate) fn with_native_root(mut self, namespace: &str, local: &str) -> Result<Self, String> {
        self.name.namespace_uri = namespace.into();
        self.name.local_name = local.into();
        let mut scope = scope_of(&self.name, &self.namespaces, &NamespaceScope::default())?;
        Self::fix_name(&mut self.name, false, &mut scope, &mut self.namespaces)?;
        Ok(self)
    }

    /// Validates the tree in `inherited`, then drops each declaration its
    /// parent already binds: every name resolves as before, so the tree stays
    /// valid.
    pub(crate) fn compact_namespaces(&mut self, inherited: &NamespaceScope) -> Result<(), String> {
        fn compact(
            name: &QName,
            namespaces: &mut Vec<(Option<String>, String)>,
            parent: &NamespaceScope,
        ) -> Result<NamespaceScope, String> {
            namespaces.retain(|(prefix, uri)| !parent.binds(prefix.as_deref(), uri));
            scope_of(name, namespaces, parent)
        }

        self.validate(inherited)?;
        let mut scopes = vec![compact(&self.name, &mut self.namespaces, inherited)?];
        for token in &mut self.children {
            match token {
                OpaqueToken::Start(start) => {
                    let parent = scopes.last().ok_or("missing opaque parent scope")?;
                    let start = &mut **start;
                    let scope = compact(&start.name, &mut start.namespaces, parent)?;
                    scopes.push(scope);
                }
                OpaqueToken::End => {
                    if scopes.len() == 1 {
                        return Err("unmatched opaque end".into());
                    }
                    scopes.pop();
                }
                _ => (),
            }
        }
        if scopes.len() != 1 {
            return Err("unterminated opaque subtree".into());
        }
        Ok(())
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
        let mut bindings = BytesStart::new("bindings");
        for (prefix, uri) in &start.namespaces {
            let name = prefix
                .as_ref()
                .map_or_else(|| "xmlns".into(), |prefix| format!("xmlns:{prefix}"));
            bindings.push_attribute((name.as_str(), uri.as_str()));
        }
        let mut scope = inherited.child(&bindings)?;
        Self::fix_name(&mut start.name, false, &mut scope, &mut start.namespaces)?;
        for (name, _) in &mut start.attributes {
            Self::fix_name(name, true, &mut scope, &mut start.namespaces)?;
        }
        start_scope(start, inherited)
    }

    fn fix_name(
        name: &mut QName,
        attribute: bool,
        scope: &mut NamespaceScope,
        declarations: &mut Vec<(Option<String>, String)>,
    ) -> Result<(), String> {
        if scope.resolves_to(name, attribute).is_ok_and(|same| same) {
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
        let mut check = TapeCheck::new(&self.view(), inherited)?;
        for token in &self.children {
            check.item(token.into())?;
        }
        check.finish()
    }

    pub(crate) fn emit(&self, writer: &mut Writer<Cursor<Vec<u8>>>) -> Result<(), String> {
        emit_start(&self.root_start(), writer)?;
        let mut names = Vec::new();
        let mut tokens = self.children.iter().peekable();
        while let Some(token) = tokens.next() {
            match token {
                OpaqueToken::Start(v) => {
                    if v.name.lexical() == v.name.local_name
                        && is_html_void_name(v.name.local_name.as_bytes())
                        && matches!(tokens.peek(), Some(OpaqueToken::End))
                    {
                        tokens.next();
                        emit_empty(v, writer)?;
                        continue;
                    }
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
