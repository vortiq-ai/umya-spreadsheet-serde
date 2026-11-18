//! A chart part as a plain element tree.
//!
//! The conversion reads the chart the way umya's own writer emits it, edits
//! that tree the way the rule table says, and hands the edited subtrees back
//! to umya's own readers. Every element keeps its prefix (`c:` or `a:`), so
//! readers that match local names accept it unchanged.
use std::{
    fmt::Write as _,
    io::Cursor,
};

use quick_xml::{
    Reader,
    Writer,
    events::{
        BytesEnd,
        BytesPI,
        BytesStart,
        BytesText,
        Event,
    },
};

use super::rule::{
    Namespace,
    Node,
    NodeValue,
};
use crate::{
    XlsxError,
    reader::driver::decode_xml_character_event,
};

/// One element: its prefixed name, attributes in document order, children
/// and text.
///
/// A leaf keeps its text in `text`. Anything else between the tags of an
/// element with element children (text, comments, processing instructions)
/// is kept in order as a child named `#text`, `#comment` or `#pi` holding it
/// in `text`, so a subtree the conversion does not edit reads back exactly as
/// it was. Those names never match a local name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Element {
    pub(super) name:       String,
    pub(super) attributes: Vec<(String, String)>,
    pub(super) children:   Vec<Element>,
    pub(super) text:       String,
}

impl Element {
    pub(super) fn new(name: impl Into<String>) -> Self {
        Self {
            name:       name.into(),
            attributes: Vec::new(),
            children:   Vec::new(),
            text:       String::new(),
        }
    }

    /// `<c:name val="value"/>`.
    pub(super) fn value(name: &str, value: &str) -> Self {
        let mut element = Self::new(format!("c:{name}"));
        element
            .attributes
            .push(("val".to_owned(), value.to_owned()));
        element
    }

    /// The name without its prefix.
    pub(super) fn local(&self) -> &str {
        self.name
            .split_once(':')
            .map_or(self.name.as_str(), |(_, local)| local)
    }

    pub(super) fn attribute(&self, name: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }

    pub(super) fn child(&self, local: &str) -> Option<&Self> {
        self.children.iter().find(|child| child.local() == local)
    }

    pub(super) fn child_mut(&mut self, local: &str) -> Option<&mut Self> {
        self.children
            .iter_mut()
            .find(|child| child.local() == local)
    }

    /// The first descendant named `local`, depth first.
    pub(super) fn find(&self, local: &str) -> Option<&Self> {
        self.children.iter().find_map(|child| {
            if child.local() == local {
                Some(child)
            } else {
                child.find(local)
            }
        })
    }

    pub(super) fn children_named<'a>(&'a self, local: &'a str) -> impl Iterator<Item = &'a Self> {
        self.children
            .iter()
            .filter(move |child| child.local() == local)
    }

    /// The `val` attribute of the first child `local`; `""` when the child
    /// has none.
    pub(super) fn val(&self, local: &str) -> Option<&str> {
        self.child(local)
            .map(|child| child.attribute("val").unwrap_or_default())
    }

    /// Removes every child named `local`.
    pub(super) fn remove(&mut self, local: &str) {
        self.children.retain(|child| child.local() != local);
    }

    /// Replaces the first child with `child`'s name, or appends it.
    pub(super) fn put(&mut self, child: Self) {
        let local = child.local().to_owned();
        match self.child_mut(&local) {
            Some(slot) => *slot = child,
            None => self.children.push(child),
        }
    }

    /// A comparable rendering: name, sorted attributes, trimmed text and
    /// children in order, extension lists left out.
    pub(super) fn canonical(&self) -> String {
        let mut attributes: Vec<_> = self.attributes.iter().collect();
        attributes.sort();
        let mut out = format!("<{}", self.local());
        for (key, value) in attributes {
            let key = key.split_once(':').map_or(key.as_str(), |(_, local)| local);
            let _ = write!(out, " {key}={value:?}");
        }
        out.push('>');
        out.push_str(self.text.trim());
        for child in self
            .children
            .iter()
            .filter(|child| child.local() != "extLst")
        {
            out.push_str(&child.canonical());
        }
        out.push_str("</>");
        out
    }

    /// The element as XML text, leaves self-closed the way umya writes them.
    pub(super) fn to_xml(&self) -> Vec<u8> {
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        self.write(&mut writer);
        writer.into_inner().into_inner()
    }

    fn write(&self, writer: &mut Writer<Cursor<Vec<u8>>>) {
        // Writing into memory cannot fail.
        let content = match self.name.as_str() {
            TEXT => Some(Event::Text(BytesText::new(&self.text))),
            COMMENT => Some(Event::Comment(BytesText::from_escaped(self.text.as_str()))),
            PI => Some(Event::PI(BytesPI::new(self.text.as_str()))),
            _ => None,
        };
        if let Some(event) = content {
            writer.write_event(event).ok();
            return;
        }
        let mut start = BytesStart::new(self.name.as_str());
        for (key, value) in &self.attributes {
            start.push_attribute((key.as_str(), value.as_str()));
        }
        if self.children.is_empty() && self.text.is_empty() {
            writer.write_event(Event::Empty(start)).ok();
            return;
        }
        writer.write_event(Event::Start(start)).ok();
        if !self.text.is_empty() {
            writer
                .write_event(Event::Text(BytesText::new(&self.text)))
                .ok();
        }
        for child in &self.children {
            child.write(writer);
        }
        writer
            .write_event(Event::End(BytesEnd::new(self.name.as_str())))
            .ok();
    }
}

const TEXT: &str = "#text";
const COMMENT: &str = "#comment";
const PI: &str = "#pi";

/// A content node of `name` holding `text`.
fn content(name: &str, text: String) -> Element {
    let mut node = Element::new(name);
    node.text = text;
    node
}

/// A closed element: a leaf's text moves into `text`.
fn closed(mut element: Element) -> Element {
    if element.children.iter().all(|child| child.name == TEXT) {
        element.text = element.children.drain(..).map(|child| child.text).collect();
    }
    element
}

/// Parses XML text into its root element.
pub(super) fn parse(bytes: &[u8]) -> Option<Element> {
    let mut reader = Reader::from_reader(bytes);
    let mut stack: Vec<Element> = Vec::new();
    loop {
        let event = reader.read_event().ok()?;
        let element = match event {
            Event::Start(ref start) => {
                stack.push(opened(start)?);
                continue;
            }
            Event::Empty(ref start) => opened(start)?,
            Event::End(_) => closed(stack.pop()?),
            Event::Text(_) | Event::CData(_) | Event::GeneralRef(_) => {
                let text = decode_xml_character_event(&event).ok()??;
                if let Some(parent) = stack.last_mut() {
                    match parent.children.last_mut() {
                        Some(last) if last.name == TEXT => last.text.push_str(&text),
                        _ => parent.children.push(content(TEXT, text)),
                    }
                }
                continue;
            }
            Event::Comment(ref text) => content(COMMENT, String::from_utf8(text.to_vec()).ok()?),
            Event::PI(ref pi) => content(PI, String::from_utf8(pi.to_vec()).ok()?),
            Event::Eof => return None,
            Event::Decl(_) | Event::DocType(_) => continue,
        };
        match stack.last_mut() {
            Some(parent) => parent.children.push(element),
            None if element.name.starts_with('#') => (),
            None => return Some(element),
        }
    }
}

fn opened(start: &BytesStart<'_>) -> Option<Element> {
    let mut element = Element::new(std::str::from_utf8(start.name().as_ref()).ok()?);
    for attribute in start.attributes() {
        let attribute = attribute.ok()?;
        let key = std::str::from_utf8(attribute.key.as_ref()).ok()?.to_owned();
        let value = attribute
            .normalized_value(quick_xml::XmlVersion::Implicit1_0)
            .ok()?
            .into_owned();
        element.attributes.push((key, value));
    }
    Some(element)
}

/// What umya's writer emits for one part, as a tree.
pub(super) fn written(
    write: impl FnOnce(&mut Writer<Cursor<Vec<u8>>>) -> Result<(), XlsxError>,
) -> Option<Element> {
    let mut writer = Writer::new(Cursor::new(Vec::new()));
    write(&mut writer).ok()?;
    parse(&writer.into_inner().into_inner())
}

/// Hands `element` to one of umya's readers, positioned after its start tag.
/// `read` receives whether the element is empty.
pub(super) fn read(
    element: &Element,
    read: impl FnOnce(&mut Reader<&[u8]>, &BytesStart<'_>, bool) -> Result<(), XlsxError>,
) -> Option<()> {
    let bytes = element.to_xml();
    let mut reader = Reader::from_reader(bytes.as_slice());
    loop {
        match reader.read_event().ok()? {
            Event::Start(start) => return read(&mut reader, &start.into_owned(), false).ok(),
            Event::Empty(start) => return read(&mut reader, &start.into_owned(), true).ok(),
            Event::Eof => return None,
            _ => (),
        }
    }
}

/// The ids a template's axis references take.
#[derive(Clone, Copy, Debug)]
pub(super) struct TemplateIds {
    pub(super) axis:  u32,
    pub(super) cross: u32,
}

/// A generated template as an element, its axis references filled in.
pub(super) fn instantiate(node: &Node, ids: TemplateIds) -> Element {
    let prefix = match node.namespace {
        Namespace::Chart => "c",
        Namespace::Drawing => "a",
    };
    let mut element = Element::new(format!("{prefix}:{}", node.name));
    for attribute in node.attributes {
        let value = match attribute.value {
            NodeValue::Literal(text) => text.to_owned(),
            NodeValue::AxisId => ids.axis.to_string(),
            NodeValue::CrossAxisId => ids.cross.to_string(),
        };
        element.attributes.push((attribute.name.to_owned(), value));
    }
    element.children = node
        .children
        .iter()
        .map(|child| instantiate(child, ids))
        .collect();
    element
}
