// Include as a cfg(test) child of charts after source-module integration.
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::Event,
};

use super::{
    SeriesPartConflictReason,
    extension_names::NamespaceScope,
    extension_payload_leaves::{
        ChartUniqueId,
        InvertSolidFill,
        NS_C14,
    },
    opaque_element::{
        OpaqueElement,
        OpaqueToken,
    },
};
fn invert(input: &str) -> InvertSolidFill {
    let mut reader = Reader::from_reader(input.as_bytes());
    let mut buf = Vec::new();
    match reader.read_event_into(&mut buf).unwrap() {
        Event::Start(e) => {
            InvertSolidFill::read(&mut reader, &e, false, &NamespaceScope::default())
        }
        Event::Empty(e) => InvertSolidFill::read(&mut reader, &e, true, &NamespaceScope::default()),
        _ => panic!("root"),
    }
}
fn opaque(input: &str, scope: &NamespaceScope) -> Result<OpaqueElement, String> {
    let mut reader = Reader::from_reader(input.as_bytes());
    let mut buf = Vec::new();
    match reader
        .read_event_into(&mut buf)
        .map_err(|e| e.to_string())?
    {
        Event::Start(e) => OpaqueElement::read(&mut reader, &e, false, scope),
        Event::Empty(e) => OpaqueElement::read(&mut reader, &e, true, scope),
        _ => Err("root".into()),
    }
}
#[test]
fn unique_id_requires_presence_but_accepts_empty_and_non_guid_strings() {
    let missing = ChartUniqueId::default();
    assert_eq!(
        missing.conflict(),
        Some(SeriesPartConflictReason::MissingRequiredChild)
    );
    for value in [
        "",
        "lexical ID, not a GUID",
        "{00000000-0000-0000-0000-000000000001}",
    ] {
        let mut id = ChartUniqueId::default();
        id.set_value(Some(value.into()));
        assert_eq!(id.value(), Some(value));
        assert!(id.conflict().is_none());
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        id.write_to(&mut writer);
        assert_eq!(
            String::from_utf8(writer.into_inner().into_inner()).unwrap(),
            format!("<c16:uniqueId val=\"{value}\"/>")
        );
    }
}
#[test]
fn invert_fill_uses_real_shape_owner_and_c14_child_name() {
    let value = invert(&format!(
        "<q:invertSolidFillFmt xmlns:q=\"{NS_C14}\" xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\"><q:spPr><a:noFill/></q:spPr></q:invertSolidFillFmt>"
    ));
    assert!(value.conflict().is_none());
    let mut writer = Writer::new(Cursor::new(Vec::new()));
    value.write_to(&mut writer);
    assert_eq!(
        String::from_utf8(writer.into_inner().into_inner()).unwrap(),
        "<c14:invertSolidFillFmt><c14:spPr><a:noFill/></c14:spPr></c14:invertSolidFillFmt>"
    );
    let wrong = invert(&format!(
        "<q:invertSolidFillFmt xmlns:q=\"{NS_C14}\"><x:spPr \
         xmlns:x=\"urn:wrong\"/></q:invertSolidFillFmt>"
    ));
    assert_eq!(
        wrong.conflict(),
        Some(SeriesPartConflictReason::MissingRequiredChild)
    );
    let nested = invert(&format!(
        "<q:invertSolidFillFmt \
         xmlns:q=\"{NS_C14}\"><q:future><q:spPr/></q:future></q:invertSolidFillFmt>"
    ));
    assert_eq!(
        nested.conflict(),
        Some(SeriesPartConflictReason::MissingRequiredChild)
    );
    let duplicate = invert(&format!(
        "<q:invertSolidFillFmt xmlns:q=\"{NS_C14}\"><q:spPr/><q:spPr/></q:invertSolidFillFmt>"
    ));
    assert_eq!(duplicate.extra_shapes().len(), 1);
    assert_eq!(
        duplicate.conflict(),
        Some(SeriesPartConflictReason::TooMany)
    );
}
#[test]
fn opaque_tree_keeps_namespace_rebinding_and_all_mixed_node_kinds_without_shadow_xml() {
    let input = "<x:future xmlns:x=\"urn:outer\" xmlns:y=\"urn:attr\" \
                 y:flag=\"A&amp;B\">before<![CDATA[raw < & text]]><!--keep--><?opaque \
                 keep?><x:future xmlns:x=\"urn:inner\"/>after</x:future>";
    let scope = NamespaceScope::default();
    let tree = opaque(input, &scope).unwrap();
    assert_eq!(tree.name.namespace_uri, "urn:outer");
    assert_eq!(tree.attributes[0].0.namespace_uri, "urn:attr");
    assert_eq!(tree.attributes[0].1, "A&B");
    assert_eq!(tree.children.len(), 7);
    assert!(matches!(&tree.children[0],OpaqueToken::Text(s) if s=="before"));
    assert!(matches!(&tree.children[1],OpaqueToken::CData(s) if s=="raw < & text"));
    assert!(matches!(&tree.children[2],OpaqueToken::Comment(s) if s=="keep"));
    assert!(matches!(&tree.children[3],OpaqueToken::ProcessingInstruction(s) if s=="opaque keep"));
    assert!(matches!(&tree.children[4],OpaqueToken::Start(e) if e.name.namespace_uri=="urn:inner"));
    assert!(matches!(&tree.children[5], OpaqueToken::End));
    assert!(matches!(&tree.children[6],OpaqueToken::Text(value) if value=="after"));
    let mut writer = Writer::new(Cursor::new(Vec::new()));
    tree.write_to(&mut writer, &scope).unwrap();
    let output = writer.into_inner().into_inner();
    assert_eq!(
        signature(&opaque(std::str::from_utf8(&output).unwrap(), &scope).unwrap()),
        signature(&tree)
    );
    let mut edited = tree.clone();
    edited.attributes[0].1 = "changed & typed".into();
    edited.children[0] = OpaqueToken::Text("new".into());
    let mut writer = Writer::new(Cursor::new(Vec::new()));
    edited.write_to(&mut writer, &scope).unwrap();
    let output = String::from_utf8(writer.into_inner().into_inner()).unwrap();
    assert!(output.contains("changed &amp; typed"));
    assert!(!output.contains("before"));
}
#[test]
fn default_element_namespace_does_not_leak_into_unqualified_attributes() {
    let tree = opaque(
        "<future xmlns=\"urn:default\" flag=\"x\"><child xmlns=\"\"/></future>",
        &NamespaceScope::default(),
    )
    .unwrap();
    assert_eq!(tree.name.namespace_uri, "urn:default");
    assert_eq!(tree.attributes[0].0.namespace_uri, "");
    assert!(matches!(&tree.children[0],OpaqueToken::Start(e) if e.name.namespace_uri.is_empty()));
    assert!(opaque("<x:future/>", &NamespaceScope::default()).is_err());
    assert!(opaque("<future><child></future>", &NamespaceScope::default()).is_err());
}

// Every retained opaque field participates in this equality; modeled nodes are
// forbidden in the opaque-only fixture rather than ignored by the comparison.
#[derive(Debug, PartialEq, Eq)]
struct OpaqueSignature {
    name:       super::QName,
    namespaces: Vec<(Option<String>, String)>,
    attributes: Vec<(super::QName, String)>,
    children:   Vec<NodeSignature>,
}
#[derive(Debug, PartialEq, Eq)]
enum NodeSignature {
    Start {
        name:       super::QName,
        namespaces: Vec<(Option<String>, String)>,
        attributes: Vec<(super::QName, String)>,
    },
    End,
    Text(String),
    CData(String),
    Comment(String),
    ProcessingInstruction(String),
}
fn signature(value: &OpaqueElement) -> OpaqueSignature {
    OpaqueSignature {
        name:       value.name.clone(),
        namespaces: value.namespaces.clone(),
        attributes: value.attributes.clone(),
        children:   value
            .children
            .iter()
            .map(|node| match node {
                OpaqueToken::Start(value) => NodeSignature::Start {
                    name:       value.name.clone(),
                    namespaces: value.namespaces.clone(),
                    attributes: value.attributes.clone(),
                },
                OpaqueToken::End => NodeSignature::End,
                OpaqueToken::Reference(_) => {
                    panic!("opaque-only fixture unexpectedly contains a modeled reference")
                }
                OpaqueToken::Text(value) => NodeSignature::Text(value.clone()),
                OpaqueToken::CData(value) => NodeSignature::CData(value.clone()),
                OpaqueToken::Comment(value) => NodeSignature::Comment(value.clone()),
                OpaqueToken::ProcessingInstruction(value) => {
                    NodeSignature::ProcessingInstruction(value.clone())
                }
            })
            .collect(),
    }
}
