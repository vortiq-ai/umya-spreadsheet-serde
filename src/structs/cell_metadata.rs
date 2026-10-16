//! Spreadsheet cell metadata. Unknown records and XML extensions are retained.
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
pub(crate) const MAX_METADATA_BYTES: u64 = 16 * 1024 * 1024;
const MAX_METADATA_EVENTS: usize = 250_000;
use quick_xml::{
    NsReader, Writer,
    events::{BytesStart, Event},
    name::ResolveResult,
};

pub(crate) const MAIN: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
pub(crate) const DYNAMIC: &str =
    "http://schemas.microsoft.com/office/spreadsheetml/2017/dynamicarray";
pub(crate) const REL: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/sheetMetadata";
pub(crate) const CONTENT_TYPE: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheetMetadata+xml";

#[derive(Clone, Debug)]
struct Node {
    start: BytesStart<'static>,
    ns: Arc<str>,
    children: Vec<Item>,
}
#[derive(Clone, Debug)]
enum Item {
    Node(Node),
    Event(Event<'static>),
}
impl Node {
    fn is(&self, name: &[u8]) -> bool {
        self.ns.as_ref() == MAIN && self.start.local_name().as_ref() == name
    }
    fn attr(&self, name: &[u8]) -> Option<String> {
        self.start
            .attributes()
            .flatten()
            .find(|a| a.key.as_ref() == name)
            .and_then(|a| {
                a.normalized_value(quick_xml::XmlVersion::Implicit1_0)
                    .ok()
                    .map(|v| v.into_owned())
            })
    }
    fn nodes(&self) -> impl Iterator<Item = &Node> {
        self.children.iter().filter_map(|x| match x {
            Item::Node(n) => Some(n),
            _ => None,
        })
    }
    fn count(&self, name: &[u8]) -> usize {
        self.nodes().filter(|n| n.is(name)).count()
    }
    fn push(&mut self, node: Node) {
        self.children.push(Item::Node(node));
    }
    fn count_attr(&mut self, name: &[u8]) {
        let count = self.count(name).to_string();
        let name = String::from_utf8_lossy(self.start.name().as_ref()).into_owned();
        let mut start = BytesStart::new(name);
        for attribute in self.start.attributes().flatten() {
            if attribute.key.as_ref() != b"count" {
                start.push_attribute(attribute);
            }
        }
        start.push_attribute(("count", count.as_str()));
        self.start = start;
    }
    fn write(&self, writer: &mut Writer<Vec<u8>>) {
        writer
            .write_event(Event::Start(self.start.borrow()))
            .unwrap();
        for item in &self.children {
            match item {
                Item::Node(n) => n.write(writer),
                Item::Event(e) => writer.write_event(e.borrow()).unwrap(),
            }
        }
        writer.write_event(Event::End(self.start.to_end())).unwrap();
    }
}
fn parse(xml: &[u8]) -> Result<Node, crate::XlsxError> {
    let mut reader = NsReader::from_reader(xml);
    let mut stack: Vec<Node> = Vec::new();
    let mut root = None;
    let mut namespaces: HashMap<Vec<u8>, Arc<str>> = HashMap::new();
    let mut event_count = 0usize;
    let mut namespace_bytes = 0u64;
    if xml.len() as u64 > MAX_METADATA_BYTES {
        return Err(crate::XlsxError::CellError(
            "Spreadsheet metadata exceeds the size limit".into(),
        ));
    }
    loop {
        let event = reader
            .read_event()
            .map_err(|e| crate::XlsxError::CellError(e.to_string()))?;
        event_count += 1;
        if event_count > MAX_METADATA_EVENTS {
            return Err(crate::XlsxError::CellError(
                "Spreadsheet metadata exceeds the event limit".into(),
            ));
        }
        let empty = matches!(event, Event::Empty(_));
        match event {
            Event::Start(e) | Event::Empty(e) => {
                if stack.is_empty() && root.is_some() {
                    return Err(crate::XlsxError::CellError(
                        "Multiple metadata XML roots".into(),
                    ));
                }
                for attribute in e.attributes() {
                    let attribute = attribute
                        .map_err(|error| crate::XlsxError::CellError(error.to_string()))?;
                    if attribute.key.as_ref() != b"xmlns"
                        && !attribute.key.as_ref().starts_with(b"xmlns:")
                        && matches!(
                            reader.resolver().resolve_attribute(attribute.key).0,
                            ResolveResult::Unknown(_)
                        )
                    {
                        return Err(crate::XlsxError::CellError(
                            "Unbound spreadsheet metadata attribute namespace prefix".into(),
                        ));
                    }
                }
                // Unknown extension XML is retained as a tree and written/dropped
                // recursively. Bound its depth before building that tree so a
                // hostile metadata part cannot exhaust the process stack later.
                if stack.len() >= 128 {
                    return Err(crate::XlsxError::CellError(
                        "Spreadsheet metadata XML exceeds the nesting limit".into(),
                    ));
                }
                let (ns, _) = reader.resolver().resolve_element(e.name());
                let ns = match ns {
                    ResolveResult::Bound(ns) => {
                        namespace_bytes = namespace_bytes.saturating_add(ns.as_ref().len() as u64);
                        if namespace_bytes > MAX_METADATA_BYTES * 4 {
                            return Err(crate::XlsxError::CellError(
                                "Spreadsheet metadata exceeds the namespace work limit".into(),
                            ));
                        }
                        if let Some(namespace) = namespaces.get(ns.as_ref()) {
                            namespace.clone()
                        } else {
                            let namespace =
                                Arc::<str>::from(String::from_utf8_lossy(ns.as_ref()).into_owned());
                            namespaces.insert(ns.as_ref().to_vec(), namespace.clone());
                            namespace
                        }
                    }
                    ResolveResult::Unknown(_) => {
                        return Err(crate::XlsxError::CellError(
                            "Unbound spreadsheet metadata namespace prefix".into(),
                        ));
                    }
                    ResolveResult::Unbound => Arc::<str>::from(""),
                };
                let node = Node {
                    start: e.into_owned(),
                    ns,
                    children: Vec::new(),
                };
                if empty {
                    if let Some(parent) = stack.last_mut() {
                        parent.push(node);
                    } else {
                        root = Some(node);
                    }
                } else {
                    stack.push(node);
                }
            }
            Event::End(_) => {
                let Some(node) = stack.pop() else {
                    return Err(crate::XlsxError::CellError("Invalid metadata XML".into()));
                };
                if let Some(parent) = stack.last_mut() {
                    parent.push(node);
                } else {
                    root = Some(node);
                }
            }
            Event::Eof => {
                return root
                    .ok_or_else(|| crate::XlsxError::CellError("Incomplete metadata XML".into()));
            }
            e => {
                if let Some(parent) = stack.last_mut() {
                    parent.children.push(Item::Event(e.into_owned()));
                } else if let Event::Text(text) = e {
                    if !text.iter().all(u8::is_ascii_whitespace) {
                        return Err(crate::XlsxError::CellError(
                            "Text outside metadata XML root".into(),
                        ));
                    }
                }
            }
        }
    }
}
fn node(xml: &str) -> Node {
    parse(xml.as_bytes()).expect("static metadata XML")
}

#[derive(Clone, Debug, Default)]
struct MetadataIndex {
    dynamic: HashSet<u32>,
    pure_dynamic: Option<u32>,
    types: Vec<Arc<str>>,
    records: HashMap<u32, Vec<(usize, usize)>>,
    extensions: HashMap<(usize, usize), HashSet<(Arc<str>, String)>>,
    reusable: HashMap<(usize, Arc<str>, Vec<u8>), u32>,
}
#[derive(Clone, Debug)]
pub(crate) struct CellMetadata {
    root: Node,
    pub(crate) path: String,
    index: MetadataIndex,
}
impl Default for CellMetadata {
    fn default() -> Self {
        Self {
            root: node(&format!(r#"<metadata xmlns="{MAIN}"/>"#)),
            path: "xl/metadata.xml".into(),
            index: MetadataIndex::default(),
        }
    }
}
impl CellMetadata {
    pub(crate) fn from_xml(xml: &[u8], path: String) -> Result<Self, crate::XlsxError> {
        let root = parse(xml)?;
        if !root.is(b"metadata") {
            return Err(crate::XlsxError::CellError(
                "Invalid spreadsheet metadata root".into(),
            ));
        }
        // Duplicate type/future names are ambiguous and could amplify index
        // work by making many type entries reference the same large block set.
        let mut type_names = HashSet::new();
        if let Some(types) = root.nodes().find(|n| n.is(b"metadataTypes")) {
            for metadata_type in types.nodes().filter(|n| n.is(b"metadataType")) {
                if !type_names.insert(metadata_type.attr(b"name").unwrap_or_default()) {
                    return Err(crate::XlsxError::CellError(
                        "Duplicate spreadsheet metadata type".into(),
                    ));
                }
            }
        }
        let mut future_names = HashSet::new();
        for future in root.nodes().filter(|n| n.is(b"futureMetadata")) {
            if !future_names.insert(future.attr(b"name").unwrap_or_default()) {
                return Err(crate::XlsxError::CellError(
                    "Duplicate spreadsheet future metadata type".into(),
                ));
            }
        }
        let mut metadata = Self {
            root,
            path,
            index: MetadataIndex::default(),
        };
        metadata.reindex();
        Ok(metadata)
    }
    pub(crate) fn xml(&self) -> Vec<u8> {
        let mut w = Writer::new(Vec::new());
        self.root.write(&mut w);
        w.into_inner()
    }
    fn reindex(&mut self) {
        let mut index = MetadataIndex::default();
        let types: Vec<_> = self
            .root
            .nodes()
            .find(|n| n.is(b"metadataTypes"))
            .map(|n| {
                n.nodes()
                    .filter(|n| n.is(b"metadataType"))
                    .map(|n| Arc::<str>::from(n.attr(b"name").unwrap_or_default()))
                    .collect()
            })
            .unwrap_or_default();
        // Decode each future block once; cm lookups below are constant-time.
        let mut future = HashMap::new();
        let mut indexed_reusable = HashSet::new();
        for metadata in self.root.nodes().filter(|n| n.is(b"futureMetadata")) {
            let name = Arc::<str>::from(metadata.attr(b"name").unwrap_or_default());
            let blocks: Vec<_> = metadata
                .nodes()
                .filter(|n| n.is(b"bk"))
                .map(|block| {
                    let properties: Vec<_> = extension_properties(block)
                        .map(|(uri, property)| {
                            let mut writer = Writer::new(Vec::new());
                            property.write(&mut writer);
                            (
                                uri,
                                property.ns.clone(),
                                String::from_utf8_lossy(property.start.local_name().as_ref())
                                    .into_owned(),
                                writer.into_inner(),
                            )
                        })
                        .collect();
                    (has_dynamic_property(block), properties)
                })
                .collect();
            future.entry(name).or_insert(blocks);
        }
        let type_futures: Vec<_> = types.iter().map(|name| future.get(name)).collect();
        for (t, blocks) in type_futures.iter().enumerate() {
            if let Some(blocks) = blocks {
                for (v, (_, properties)) in blocks.iter().enumerate() {
                    index.extensions.insert(
                        (t, v),
                        properties
                            .iter()
                            .map(|(_, namespace, local, _)| (namespace.clone(), local.clone()))
                            .collect(),
                    );
                }
            }
        }
        if let Some(cells) = self.root.nodes().find(|n| n.is(b"cellMetadata")) {
            for (at, block) in cells.nodes().filter(|n| n.is(b"bk")).enumerate() {
                let cm = (at + 1) as u32;
                let children: Vec<_> = block.nodes().collect();
                let pure = children.len() == 1 && children[0].is(b"rc");
                for record in children.iter().filter(|n| n.is(b"rc")) {
                    let Some(t) = record
                        .attr(b"t")
                        .and_then(|t| t.parse::<usize>().ok())
                        .and_then(|t| t.checked_sub(1))
                        .filter(|&t| t < types.len())
                    else {
                        continue;
                    };
                    let Some(v) = record.attr(b"v").and_then(|v| v.parse::<usize>().ok()) else {
                        continue;
                    };
                    let Some((dynamic, properties)) =
                        type_futures[t].and_then(|blocks| blocks.get(v))
                    else {
                        continue;
                    };
                    index.records.entry(cm).or_default().push((t, v));
                    if types[t].as_ref() == "XLDAPR" && *dynamic {
                        index.dynamic.insert(cm);
                        if pure && properties.len() == 1 && index.pure_dynamic.is_none() {
                            index.pure_dynamic = Some(cm);
                        }
                    }
                    if pure && properties.len() == 1 && indexed_reusable.insert((t, v)) {
                        let (uri, _, _, xml) = &properties[0];
                        index
                            .reusable
                            .entry((t, uri.clone(), xml.clone()))
                            .or_insert(cm);
                    }
                }
            }
        }
        index.types = types;
        self.index = index;
    }
    pub(crate) fn is_dynamic(&self, cm: u32) -> bool {
        self.index.dynamic.contains(&cm)
    }
    pub(crate) fn ensure_dynamic(&mut self) -> u32 {
        if let Some(cm) = self.index.pure_dynamic {
            return cm;
        }
        let properties = node(&format!(
            r#"<xda:dynamicArrayProperties xmlns:xda="{DYNAMIC}" fDynamic="1" fCollapsed="0"/>"#
        ));
        self.append_record(
            "XLDAPR",
            "{bdbb8cdc-fa1e-496e-a857-3c3f30c029c3}",
            properties,
        )
    }
    pub(crate) fn is_extension(
        &self,
        cm: u32,
        type_name: &str,
        namespace: &str,
        local_name: &str,
    ) -> bool {
        self.index.records.get(&cm).is_some_and(|records| {
            records.iter().any(|(t, v)| {
                self.index.types[*t].as_ref() == type_name
                    && self
                        .index
                        .extensions
                        .get(&(*t, *v))
                        .is_some_and(|properties| {
                            properties
                                .contains(&(Arc::<str>::from(namespace), local_name.to_owned()))
                        })
            })
        })
    }
    pub(crate) fn ensure_extension(
        &mut self,
        type_name: &str,
        extension_uri: &str,
        properties_xml: &str,
    ) -> Result<u32, crate::XlsxError> {
        let property = parse(properties_xml.as_bytes())?;
        if type_name.is_empty()
            || extension_uri.is_empty()
            || property.ns.is_empty()
            || property.ns.as_ref() == MAIN
        {
            return Err(crate::XlsxError::CellError(
                "Cell metadata extensions require a type, URI, and namespaced property".into(),
            ));
        }
        let mut expected = Writer::new(Vec::new());
        property.write(&mut expected);
        let expected = expected.into_inner();
        if let Some(t) = self
            .index
            .types
            .iter()
            .position(|name| name.as_ref() == type_name)
        {
            if let Some(&cm) =
                self.index
                    .reusable
                    .get(&(t, Arc::<str>::from(extension_uri), expected))
            {
                return Ok(cm);
            }
        }
        Ok(self.append_record(type_name, extension_uri, property))
    }
    fn append_record(&mut self, type_name: &str, extension_uri: &str, property: Node) -> u32 {
        if !self.root.nodes().any(|n| n.is(b"metadataTypes")) {
            self.root
                .children
                .insert(0, Item::Node(element("metadataTypes", &[("count", "0")])));
        }
        let types = self
            .root
            .children
            .iter_mut()
            .find_map(|x| match x {
                Item::Node(n) if n.is(b"metadataTypes") => Some(n),
                _ => None,
            })
            .unwrap();
        let type_index = types
            .nodes()
            .filter(|n| n.is(b"metadataType"))
            .position(|n| n.attr(b"name").as_deref() == Some(type_name));
        let t = if let Some(index) = type_index {
            index + 1
        } else {
            types.push(element(
                "metadataType",
                &[
                    ("name", type_name),
                    ("minSupportedVersion", "120000"),
                    ("copy", "1"),
                    ("pasteAll", "1"),
                    ("pasteValues", "1"),
                    ("merge", "1"),
                    ("splitFirst", "1"),
                    ("rowColShift", "1"),
                    ("clearFormats", "1"),
                    ("clearComments", "1"),
                    ("assign", "1"),
                    ("coerce", "1"),
                    ("cellMeta", "1"),
                ],
            ));
            types.count_attr(b"metadataType");
            types.count(b"metadataType")
        };
        if !self
            .root
            .nodes()
            .any(|n| n.is(b"futureMetadata") && n.attr(b"name").as_deref() == Some(type_name))
        {
            let at = self.root.children.iter().position(|x| matches!(x, Item::Node(n) if n.is(b"cellMetadata") || n.is(b"valueMetadata") || n.is(b"extLst"))).unwrap_or(self.root.children.len());
            self.root.children.insert(
                at,
                Item::Node(element(
                    "futureMetadata",
                    &[("name", type_name), ("count", "0")],
                )),
            );
        }
        let future = self
            .root
            .children
            .iter_mut()
            .find_map(|x| match x {
                Item::Node(n)
                    if n.is(b"futureMetadata") && n.attr(b"name").as_deref() == Some(type_name) =>
                {
                    Some(n)
                }
                _ => None,
            })
            .unwrap();
        let v = future.count(b"bk");
        let mut extension = element("ext", &[("uri", extension_uri)]);
        extension.push(property);
        let mut extensions = element("extLst", &[]);
        extensions.push(extension);
        let mut block = element("bk", &[]);
        block.push(extensions);
        future.push(block);
        future.count_attr(b"bk");
        if !self.root.nodes().any(|n| n.is(b"cellMetadata")) {
            let at = self
                .root
                .children
                .iter()
                .position(
                    |x| matches!(x, Item::Node(n) if n.is(b"valueMetadata") || n.is(b"extLst")),
                )
                .unwrap_or(self.root.children.len());
            self.root
                .children
                .insert(at, Item::Node(element("cellMetadata", &[("count", "0")])));
        }
        let cells = self
            .root
            .children
            .iter_mut()
            .find_map(|x| match x {
                Item::Node(n) if n.is(b"cellMetadata") => Some(n),
                _ => None,
            })
            .unwrap();
        let mut block = element("bk", &[]);
        block.push(element(
            "rc",
            &[("t", &t.to_string()), ("v", &v.to_string())],
        ));
        cells.push(block);
        cells.count_attr(b"bk");
        let cm = cells.count(b"bk") as u32;
        self.reindex();
        cm
    }
}
fn element(name: &str, attributes: &[(&str, &str)]) -> Node {
    let mut start = BytesStart::new(name.to_owned());
    start.push_attribute(("xmlns", MAIN));
    for &(key, value) in attributes {
        start.push_attribute((key, value));
    }
    Node {
        start,
        ns: MAIN.into(),
        children: Vec::new(),
    }
}
fn extension_properties(block: &Node) -> impl Iterator<Item = (Arc<str>, &Node)> {
    block
        .nodes()
        .filter(|n| n.is(b"extLst"))
        .flat_map(|n| n.nodes())
        .filter(|n| n.is(b"ext"))
        .flat_map(|extension| {
            let uri = Arc::<str>::from(extension.attr(b"uri").unwrap_or_default());
            extension
                .nodes()
                .map(move |property| (uri.clone(), property))
        })
}

fn has_dynamic_property(n: &Node) -> bool {
    // Only the standard extension path counts; a similarly named foreign element does not.
    n.nodes()
        .filter(|n| n.is(b"extLst"))
        .flat_map(|n| n.nodes())
        .filter(|n| n.is(b"ext"))
        .flat_map(|n| n.nodes())
        .any(|n| {
            n.ns.as_ref() == DYNAMIC
                && n.start.local_name().as_ref() == b"dynamicArrayProperties"
                && matches!(n.attr(b"fDynamic").as_deref(), Some("1" | "true"))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Read};

    #[test]
    fn cell_metadata_rejects_excessive_unknown_extension_depth() {
        let xml = format!(
            "<metadata xmlns=\"{MAIN}\">{}{}</metadata>",
            "<extension>".repeat(512),
            "</extension>".repeat(512),
        );
        assert!(CellMetadata::from_xml(xml.as_bytes(), "xl/metadata.xml".into()).is_err());
    }

    #[test]
    fn cell_metadata_resolves_indices_and_preserves_unknown_records() {
        // Prefixes, an unrelated first type/block, a false record, and a foreign
        // lookalike ensure cm != 0 alone never means dynamic.
        let xml = format!(
            r#"<s:metadata xmlns:s="{MAIN}" xmlns:d="{DYNAMIC}" xmlns:u="urn:other">
          <s:metadataTypes count="2"><s:metadataType name="OTHER"/><s:metadataType name="XLDAPR"/></s:metadataTypes>
          <s:futureMetadata name="OTHER" count="1"><s:bk><u:keep x="1"/></s:bk></s:futureMetadata>
          <s:futureMetadata name="XLDAPR" count="3">
            <s:bk><s:extLst><s:ext uri="x"><d:dynamicArrayProperties fDynamic="0"/></s:ext></s:extLst></s:bk>
            <s:bk><s:extLst><s:ext uri="x"><d:dynamicArrayProperties fDynamic="true" fCollapsed="0"/></s:ext></s:extLst></s:bk>
            <s:bk><s:extLst><s:ext uri="x"><u:dynamicArrayProperties fDynamic="1"/></s:ext></s:extLst></s:bk>
          </s:futureMetadata>
          <s:cellMetadata count="5"><s:bk><s:rc t="1" v="1"/></s:bk><s:bk><s:rc t="2" v="0"/></s:bk><s:bk><s:rc t="2" v="1"/></s:bk><s:bk><s:rc t="2" v="2"/></s:bk><s:bk><s:rc t="9" v="1"/></s:bk></s:cellMetadata>
          <s:valueMetadata count="1"><s:bk><s:rc t="1" v="0"/></s:bk></s:valueMetadata><s:extLst><u:keep/></s:extLst>
        </s:metadata>"#
        );
        let mut m = CellMetadata::from_xml(xml.as_bytes(), "xl/metadata.xml".into()).unwrap();
        for cm in [0, 1, 2, 4, 5, 6, u32::MAX] {
            assert!(!m.is_dynamic(cm), "cm={cm}");
        }
        assert!(m.is_dynamic(3));
        assert_eq!(m.ensure_dynamic(), 3);
        let saved = String::from_utf8(m.xml()).unwrap();
        assert!(saved.contains("u:keep"));
        assert!(saved.contains("valueMetadata"));
        let m = CellMetadata::from_xml(saved.as_bytes(), m.path).unwrap();
        assert!(m.is_dynamic(3));
    }

    #[test]
    fn cell_metadata_appends_without_reindexing_existing_records() {
        let xml = format!(
            r#"<metadata xmlns="{MAIN}"><metadataTypes count="1"><metadataType name="OTHER"/></metadataTypes><futureMetadata name="OTHER" count="1"><bk/></futureMetadata><cellMetadata count="1"><bk><rc t="1" v="0"/></bk></cellMetadata><valueMetadata count="0"/></metadata>"#
        );
        let mut m = CellMetadata::from_xml(xml.as_bytes(), "xl/metadata.xml".into()).unwrap();
        assert_eq!(m.ensure_dynamic(), 2);
        assert!(!m.is_dynamic(1));
        assert!(m.is_dynamic(2));
        let xml = String::from_utf8(m.xml()).unwrap();
        assert!(xml.contains("count=\"2\""));
        assert!(xml.contains("t=\"2\" v=\"0\""));
        let mut m = CellMetadata::from_xml(xml.as_bytes(), m.path).unwrap();
        assert_eq!(m.ensure_dynamic(), 2);
    }

    #[test]
    fn cell_metadata_appends_after_existing_non_dynamic_future_record() {
        let xml = format!(
            r#"<metadata xmlns="{MAIN}" xmlns:d="{DYNAMIC}"><metadataTypes count="1"><metadataType name="XLDAPR"/></metadataTypes><futureMetadata name="XLDAPR" count="1"><bk><extLst><ext uri="existing"><d:dynamicArrayProperties fDynamic="0"/></ext></extLst></bk></futureMetadata><cellMetadata count="1"><bk><rc t="1" v="0"/></bk></cellMetadata></metadata>"#
        );
        let mut metadata =
            CellMetadata::from_xml(xml.as_bytes(), "xl/metadata.xml".into()).unwrap();
        assert_eq!(metadata.ensure_dynamic(), 2);
        assert!(!metadata.is_dynamic(1));
        assert!(metadata.is_dynamic(2));
        let saved = String::from_utf8(metadata.xml()).unwrap();
        assert!(saved.contains("t=\"1\" v=\"1\""));
        let mut metadata = CellMetadata::from_xml(saved.as_bytes(), metadata.path).unwrap();
        assert_eq!(metadata.ensure_dynamic(), 2);
    }

    #[test]
    fn cell_metadata_mixed_dynamic_block_is_not_reused() {
        let xml = format!(
            r#"<metadata xmlns="{MAIN}" xmlns:d="{DYNAMIC}"><metadataTypes count="2"><metadataType name="OTHER"/><metadataType name="XLDAPR"/></metadataTypes><futureMetadata name="XLDAPR" count="1"><bk><extLst><ext uri="existing"><d:dynamicArrayProperties fDynamic="1"/></ext></extLst></bk></futureMetadata><cellMetadata count="1"><bk><rc t="1" v="0"/><rc t="2" v="0"/></bk></cellMetadata></metadata>"#
        );
        let mut metadata =
            CellMetadata::from_xml(xml.as_bytes(), "xl/metadata.xml".into()).unwrap();
        assert!(metadata.is_dynamic(1));
        assert_eq!(metadata.ensure_dynamic(), 2);
        assert!(metadata.is_dynamic(1));
        assert_eq!(metadata.ensure_dynamic(), 2);
        let saved = String::from_utf8(metadata.xml()).unwrap();
        assert!(saved.contains("name=\"OTHER\""));
        assert!(saved.contains("t=\"1\" v=\"0\""));
    }

    #[test]
    fn cell_metadata_dynamic_future_with_foreign_extension_is_not_reused() {
        let xml = format!(
            r#"<metadata xmlns="{MAIN}" xmlns:d="{DYNAMIC}"><metadataTypes count="1"><metadataType name="XLDAPR"/></metadataTypes><futureMetadata name="XLDAPR" count="1"><bk><extLst><ext uri="existing"><d:dynamicArrayProperties fDynamic="1"/></ext><ext uri="urn:foreign"><opaque xmlns="urn:foreign"/></ext></extLst></bk></futureMetadata><cellMetadata count="1"><bk><rc t="1" v="0"/></bk></cellMetadata></metadata>"#
        );
        let mut metadata = CellMetadata::from_xml(xml.as_bytes(), "xl/metadata.xml".into()).unwrap();
        assert!(metadata.is_dynamic(1));
        assert!(metadata.is_extension(1, "XLDAPR", "urn:foreign", "opaque"));
        let new_cm = metadata.ensure_dynamic();
        assert_eq!(new_cm, 2);
        assert!(metadata.is_dynamic(new_cm));
        assert!(!metadata.is_extension(new_cm, "XLDAPR", "urn:foreign", "opaque"));
        assert_eq!(metadata.ensure_dynamic(), new_cm);
        let saved = metadata.xml();
        let metadata = CellMetadata::from_xml(&saved, metadata.path).unwrap();
        assert!(metadata.is_extension(1, "XLDAPR", "urn:foreign", "opaque"));
        assert!(!metadata.is_extension(new_cm, "XLDAPR", "urn:foreign", "opaque"));
    }

    #[test]
    fn cell_metadata_generic_extension_roundtrips_and_escapes_attributes() {
        let mut metadata = CellMetadata::default();
        let property = r#"<pendingArray xmlns="urn:test:pending"/>"#;
        let cm = metadata
            .ensure_extension("CUSTOM&TYPE", "urn:test?x=1&y=2", property)
            .unwrap();
        assert!(!metadata.is_dynamic(cm));
        assert!(metadata.is_extension(cm, "CUSTOM&TYPE", "urn:test:pending", "pendingArray"));
        assert!(!metadata.is_extension(cm, "CUSTOM&TYPE", "urn:other", "pendingArray"));
        assert_eq!(
            metadata
                .ensure_extension("CUSTOM&TYPE", "urn:test?x=1&y=2", property)
                .unwrap(),
            cm
        );
        let mut loaded = CellMetadata::from_xml(&metadata.xml(), metadata.path).unwrap();
        assert!(loaded.is_extension(cm, "CUSTOM&TYPE", "urn:test:pending", "pendingArray"));
        assert_eq!(
            loaded
                .ensure_extension("CUSTOM&TYPE", "urn:test?x=1&y=2", property)
                .unwrap(),
            cm
        );
        assert!(
            loaded
                .ensure_extension("BAD", "uri", "<x xmlns='urn:x'/><y/>")
                .is_err()
        );
        assert!(loaded.ensure_extension("BAD", "uri", "<x/>").is_err());
        assert!(
            loaded
                .ensure_extension("BAD", "uri", "<x xmlns='urn:x'><bad:unbound/></x>")
                .is_err()
        );
        assert!(
            loaded
                .ensure_extension("BAD", "uri", "<x xmlns='urn:x' bad:attribute='1'/>")
                .is_err()
        );
        let invalid = format!(r#"<metadata xmlns="{MAIN}"><bad:extension/></metadata>"#);
        assert!(CellMetadata::from_xml(invalid.as_bytes(), "xl/metadata.xml".into()).is_err());
    }

    #[test]
    fn cell_metadata_rejects_excessive_shallow_events_and_bytes() {
        let xml = format!(
            r#"<metadata xmlns="{MAIN}">{}</metadata>"#,
            "<other/>".repeat(MAX_METADATA_EVENTS)
        );
        assert!(CellMetadata::from_xml(xml.as_bytes(), "xl/metadata.xml".into()).is_err());
        let bytes = vec![b' '; MAX_METADATA_BYTES as usize + 1];
        assert!(CellMetadata::from_xml(&bytes, "xl/metadata.xml".into()).is_err());
    }

    #[test]
    fn cell_metadata_reader_rejects_compressed_oversized_part() {
        use std::io::Write;
        let mut book = crate::new_file();
        book.ensure_dynamic_array_metadata();
        let mut bytes = Vec::new();
        crate::writer::xlsx::write_writer(&book, &mut bytes).unwrap();
        let mut source = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for i in 0..source.len() {
            let mut file = source.by_index(i).unwrap();
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);
            output.start_file(file.name(), options).unwrap();
            if file.name() == "xl/metadata.xml" {
                output
                    .write_all(&vec![b' '; MAX_METADATA_BYTES as usize + 1])
                    .unwrap();
            } else {
                std::io::copy(&mut file, &mut output).unwrap();
            }
        }
        let bytes = output.finish().unwrap().into_inner();
        assert!(bytes.len() < 100_000, "fixture must be compressed");
        assert!(crate::reader::xlsx::read_reader(Cursor::new(bytes), true).is_err());
    }

    #[test]
    fn cell_metadata_package_resolves_parent_and_absolute_targets_on_roundtrip() {
        use std::io::Write;
        for target in ["../metadata.xml", "/metadata.xml", "/xl/metadata.xml"] {
            let mut book = crate::new_file();
            let cm = book.ensure_dynamic_array_metadata();
            let mut bytes = Vec::new();
            crate::writer::xlsx::write_writer(&book, &mut bytes).unwrap();
            let mut source = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
            let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
            for i in 0..source.len() {
                let mut file = source.by_index(i).unwrap();
                let name = file.name().to_owned();
                let mut contents = Vec::new();
                file.read_to_end(&mut contents).unwrap();
                let root_part = target != "/xl/metadata.xml";
                let output_name = if root_part && name == "xl/metadata.xml" { "metadata.xml" } else { &name };
                if name == "xl/_rels/workbook.xml.rels" {
                    contents = String::from_utf8(contents).unwrap()
                        .replace("Target=\"metadata.xml\"", &format!("Target=\"{target}\""))
                        .into_bytes();
                } else if root_part && name == "[Content_Types].xml" {
                    contents = String::from_utf8(contents).unwrap()
                        .replace("/xl/metadata.xml", "/metadata.xml").into_bytes();
                }
                output.start_file(output_name, zip::write::SimpleFileOptions::default()).unwrap();
                output.write_all(&contents).unwrap();
            }
            let bytes = output.finish().unwrap().into_inner();
            let loaded = crate::reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap();
            assert!(loaded.is_dynamic_array_metadata(cm), "{target}");
            let mut bytes = Vec::new();
            crate::writer::xlsx::write_writer(&loaded, &mut bytes).unwrap();
            let reloaded = crate::reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap();
            assert!(reloaded.is_dynamic_array_metadata(cm), "{target}");
        }
    }

    #[test]
    fn cell_metadata_native_package_roundtrip() {
        let mut book = crate::new_file();
        let cm = book.ensure_dynamic_array_metadata();
        let ws = book.sheet_mut(0).unwrap();
        let mut formula = crate::CellFormula::default();
        formula.set_formula_type(crate::CellFormulaValues::Array);
        formula.set_reference("A1:A3");
        formula.set_text("_xlfn.SEQUENCE(3)");
        let cell = ws.cell_mut("A1");
        cell.set_cell_meta_index(cm);
        cell.cell_value_mut().set_formula_obj(formula.clone());
        formula.set_reference("C1:C3");
        ws.cell_mut("C1").cell_value_mut().set_formula_obj(formula);
        let mut bytes = Vec::new();
        crate::writer::xlsx::write_writer(&book, &mut bytes).unwrap();
        let mut archive = zip::ZipArchive::new(Cursor::new(&bytes)).unwrap();
        let mut rels = String::new();
        archive
            .by_name("xl/_rels/workbook.xml.rels")
            .unwrap()
            .read_to_string(&mut rels)
            .unwrap();
        assert!(rels.contains(REL));
        let mut types = String::new();
        archive
            .by_name("[Content_Types].xml")
            .unwrap()
            .read_to_string(&mut types)
            .unwrap();
        assert!(types.contains(CONTENT_TYPE));
        let mut sheet = String::new();
        archive
            .by_name("xl/worksheets/sheet1.xml")
            .unwrap()
            .read_to_string(&mut sheet)
            .unwrap();
        assert!(sheet.contains(&format!("cm=\"{cm}\"")));
        let book = crate::reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap();
        let ws = book.sheet(0).unwrap();
        assert!(book.is_dynamic_array_metadata(ws.cell("A1").unwrap().cell_meta_index()));
        assert!(!book.is_dynamic_array_metadata(ws.cell("C1").unwrap().cell_meta_index()));
        assert_eq!(
            ws.cell("C1").unwrap().formula_obj().unwrap().reference(),
            "C1:C3"
        );
    }
}
