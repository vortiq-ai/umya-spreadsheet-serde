use std::collections::BTreeMap;

use quick_xml::{
    NsReader,
    events::Event,
    name::ResolveResult,
};

#[derive(Debug, PartialEq, Eq)]
pub enum XmlToken {
    Start((String, String), BTreeMap<(String, String), String>),
    End,
    Text(String),
}

fn namespace(value: ResolveResult<'_>) -> String {
    match value {
        ResolveResult::Bound(value) => String::from_utf8(value.as_ref().to_vec()).unwrap(),
        ResolveResult::Unbound => String::new(),
        ResolveResult::Unknown(prefix) => panic!("undeclared XML prefix {prefix:?}"),
    }
}

fn append_text(tokens: &mut Vec<XmlToken>, text: &str) {
    if let Some(XmlToken::Text(previous)) = tokens.last_mut() {
        previous.push_str(text);
    } else {
        tokens.push(XmlToken::Text(text.to_owned()));
    }
}

pub fn canonical_xml(xml: &[u8]) -> Vec<XmlToken> {
    let mut reader = NsReader::from_reader(xml);
    let mut tokens = Vec::new();
    let mut depth = 0usize;
    loop {
        let event = reader.read_event().unwrap();
        let empty = matches!(event, Event::Empty(_));
        match event {
            Event::Start(element) | Event::Empty(element) => {
                let (uri, local) = reader.resolver().resolve_element(element.name());
                let name = (
                    namespace(uri),
                    String::from_utf8(local.as_ref().to_vec()).unwrap(),
                );
                let mut attributes = BTreeMap::new();
                for attribute in element.attributes() {
                    let attribute = attribute.unwrap();
                    if attribute.key.as_ref() == b"xmlns"
                        || attribute
                            .key
                            .prefix()
                            .is_some_and(|value| value.as_ref() == b"xmlns")
                    {
                        continue;
                    }
                    let (uri, local) = reader.resolver().resolve_attribute(attribute.key);
                    let key = (
                        namespace(uri),
                        String::from_utf8(local.as_ref().to_vec()).unwrap(),
                    );
                    let value = attribute
                        .decoded_and_normalized_value(
                            quick_xml::XmlVersion::Implicit1_0,
                            reader.decoder(),
                        )
                        .unwrap()
                        .into_owned();
                    assert!(
                        attributes.insert(key, value).is_none(),
                        "duplicate XML attribute"
                    );
                }
                tokens.push(XmlToken::Start(name, attributes));
                if empty {
                    tokens.push(XmlToken::End);
                } else {
                    depth += 1;
                }
            }
            Event::End(_) => {
                depth -= 1;
                tokens.push(XmlToken::End);
            }
            Event::Text(text) if depth == 0 => {
                assert!(
                    text.xml_content(quick_xml::XmlVersion::Implicit1_0)
                        .unwrap()
                        .chars()
                        .all(|character| matches!(character, ' ' | '\t' | '\r' | '\n')),
                    "non-whitespace text outside document element"
                );
            }
            Event::Text(text) => append_text(
                &mut tokens,
                &text
                    .xml_content(quick_xml::XmlVersion::Implicit1_0)
                    .unwrap(),
            ),
            Event::CData(text) => append_text(
                &mut tokens,
                &text
                    .xml_content(quick_xml::XmlVersion::Implicit1_0)
                    .unwrap(),
            ),
            Event::GeneralRef(reference) => {
                let entity = format!("&{};", reference.decode().unwrap());
                append_text(&mut tokens, &quick_xml::escape::unescape(&entity).unwrap());
            }
            Event::Decl(_) | Event::Comment(_) | Event::PI(_) => (),
            Event::Eof => break,
            event => panic!("unexpected chart XML event {event:?}"),
        }
    }
    tokens
}
