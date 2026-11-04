//! Shared XML token and scalar utilities.
use std::io::Cursor;

use quick_xml::{
    Reader,
    Writer,
    events::Event,
};

use crate::{
    drawing::charts::{
        NamespaceScope,
        OpaqueElement,
        OpaqueToken,
    },
    structs::{
        XlsxError,
        XlsxErrorKind,
    },
    writer::driver::write_start_tag,
};

pub(crate) fn trim_xml_whitespace(value: &str) -> &str {
    value.trim_matches(|c| matches!(c, ' ' | '\t' | '\r' | '\n'))
}
pub(crate) fn parse_bool(value: &str) -> Result<bool, String> {
    match trim_xml_whitespace(value) {
        "1" | "true" => Ok(true),
        "0" | "false" => Ok(false),
        _ => Err(format!("invalid XML boolean: {value}")),
    }
}
pub(crate) fn parse_u32(value: &str) -> Result<u32, String> {
    trim_xml_whitespace(value)
        .parse()
        .map_err(|_| format!("invalid XML unsigned integer: {value}"))
}
pub(crate) fn parse_i32(value: &str) -> Result<i32, String> {
    trim_xml_whitespace(value)
        .parse()
        .map_err(|_| format!("invalid XML integer: {value}"))
}
pub(crate) fn parse_double(value: &str) -> Result<f64, String> {
    let value = trim_xml_whitespace(value);
    let invalid = || format!("invalid XML double: {value}");
    match value {
        "INF" => return Ok(f64::INFINITY),
        "-INF" => return Ok(f64::NEG_INFINITY),
        "NaN" => return Ok(f64::NAN),
        _ => (),
    }
    let unsigned = value
        .strip_prefix('+')
        .or_else(|| value.strip_prefix('-'))
        .unwrap_or(value);
    let mut exponent_split = unsigned.split(['e', 'E']);
    let mantissa = exponent_split.next().ok_or_else(invalid)?;
    if let Some(exponent) = exponent_split.next() {
        let digits = exponent
            .strip_prefix('+')
            .or_else(|| exponent.strip_prefix('-'))
            .unwrap_or(exponent);
        if digits.is_empty() || !digits.bytes().all(|c| c.is_ascii_digit()) {
            return Err(invalid());
        }
    }
    if exponent_split.next().is_some() {
        return Err(invalid());
    }
    let mut dot_split = mantissa.split('.');
    let whole = dot_split.next().ok_or_else(invalid)?;
    let fraction = dot_split.next().unwrap_or("");
    if dot_split.next().is_some()
        || (whole.is_empty() && fraction.is_empty())
        || !whole
            .bytes()
            .chain(fraction.bytes())
            .all(|c| c.is_ascii_digit())
    {
        return Err(invalid());
    }
    value.parse().map_err(|_| invalid())
}
pub(crate) fn attr<'a>(node: &'a OpaqueElement, name: &str) -> Option<&'a str> {
    attr_of(&node.attributes, name)
}
pub(crate) fn attr_of<'a>(
    attributes: &'a [(crate::drawing::charts::QName, String)],
    name: &str,
) -> Option<&'a str> {
    attributes
        .iter()
        .find(|(key, _)| key.is("", name))
        .map(|(_, value)| value.as_str())
}
pub(crate) fn required(node: &OpaqueElement, name: &str) -> Result<String, String> {
    attr(node, name)
        .map(str::to_owned)
        .ok_or_else(|| format!("{} requires {name}", node.name.local_name))
}
pub(crate) fn optional_xstring_attribute(node: &OpaqueElement, name: &str) -> Option<String> {
    attr(node, name).map(|value| crate::helper::utils::unescape_xstring(value).into_owned())
}
pub(crate) fn unsigned(node: &OpaqueElement, name: &str) -> Result<Option<u32>, String> {
    attr(node, name)
        .map(|value| parse_u32(value).map_err(|_| format!("invalid {name}")))
        .transpose()
}
pub(crate) fn signed(node: &OpaqueElement, name: &str) -> Result<Option<i32>, String> {
    attr(node, name)
        .map(|value| parse_i32(value).map_err(|_| format!("invalid signed {name}")))
        .transpose()
}
pub(crate) fn required_unsigned(node: &OpaqueElement, name: &str) -> Result<u32, String> {
    unsigned(node, name)?.ok_or_else(|| format!("{} requires {name}", node.name.local_name))
}
pub(crate) fn boolean(node: &OpaqueElement, name: &str) -> Result<Option<bool>, String> {
    attr(node, name)
        .map(|value| parse_bool(value).map_err(|_| format!("invalid {name}")))
        .transpose()
}
pub(crate) fn double(node: &OpaqueElement, name: &str) -> Result<Option<f64>, String> {
    attr(node, name)
        .map(|value| parse_double(value).map_err(|_| format!("invalid {name}")))
        .transpose()
}
pub(crate) fn double_lexical(value: f64) -> String {
    if value.is_nan() {
        "NaN".into()
    } else if value == f64::INFINITY {
        "INF".into()
    } else if value == f64::NEG_INFINITY {
        "-INF".into()
    } else {
        value.to_string()
    }
}
pub(crate) fn expect(node: &OpaqueElement, namespace: &str, name: &str) -> Result<(), String> {
    if node.name.is(namespace, name) {
        Ok(())
    } else {
        Err(format!("expected qualified {name}"))
    }
}
pub(crate) fn attributes(
    node: &OpaqueElement,
    allowed: &[&str],
    qualified: &[(&str, &str)],
) -> Result<(), String> {
    for (name, _) in &node.attributes {
        if name.namespace_uri.is_empty() && allowed.contains(&name.local_name.as_str())
            || qualified
                .iter()
                .any(|(namespace, local)| name.is(namespace, local))
        {
            continue;
        }
        return Err(format!(
            "unsupported attribute {} on {}",
            name.lexical(),
            node.name.local_name
        ));
    }
    Ok(())
}
pub(crate) fn direct_children(node: &OpaqueElement) -> Result<Vec<OpaqueElement>, String> {
    let mut result = Vec::new();
    let mut index = 0;
    while index < node.children.len() {
        match &node.children[index] {
            OpaqueToken::Start(start) => {
                let first = index + 1;
                let mut depth = 1;
                index += 1;
                while index < node.children.len() && depth != 0 {
                    match node.children[index] {
                        OpaqueToken::Start(_) => depth += 1,
                        OpaqueToken::End => depth -= 1,
                        _ => (),
                    }
                    if depth != 0 {
                        index += 1;
                    }
                }
                if depth != 0 {
                    return Err("unterminated XML child".into());
                }
                let explicit_xml = node
                    .namespaces
                    .iter()
                    .chain(&start.namespaces)
                    .any(|(prefix, _)| prefix.as_deref() == Some("xml"));
                let bindings = NamespaceScope::with_bindings(
                    node.namespaces
                        .iter()
                        .cloned()
                        .chain(start.namespaces.iter().cloned()),
                );
                result.push(OpaqueElement {
                    name:       start.name.clone(),
                    namespaces: bindings
                        .effective_bindings()
                        .into_iter()
                        .filter(|(prefix, _)| prefix.as_deref() != Some("xml") || explicit_xml)
                        .collect(),
                    attributes: start.attributes.clone(),
                    children:   node.children[first..index].to_vec(),
                });
            }
            OpaqueToken::Text(value) | OpaqueToken::CData(value)
                if trim_xml_whitespace(value).is_empty() => {}
            OpaqueToken::Comment(_) | OpaqueToken::ProcessingInstruction(_) => (),
            _ => return Err("unexpected XML element content".into()),
        }
        index += 1;
    }
    Ok(result)
}
pub(crate) fn no_children(node: &OpaqueElement) -> Result<(), String> {
    if direct_children(node)?.is_empty() {
        Ok(())
    } else {
        Err(format!("unexpected child in {}", node.name.local_name))
    }
}
pub(crate) fn root(bytes: &[u8]) -> Result<OpaqueElement, String> {
    let mut reader = Reader::from_reader(bytes);
    let mut root = None;
    loop {
        match reader.read_event().map_err(|error| error.to_string())? {
            Event::Start(start) | Event::Empty(start) if root.is_some() => {
                return Err(format!(
                    "multiple XML roots: {}",
                    String::from_utf8_lossy(start.name().as_ref())
                ));
            }
            ref event @ (Event::Start(ref start) | Event::Empty(ref start)) => {
                root = Some(OpaqueElement::read(
                    &mut reader,
                    start,
                    matches!(event, Event::Empty(_)),
                    &NamespaceScope::default(),
                )?);
            }
            Event::Decl(_) if root.is_none() => {}
            Event::Comment(_) | Event::PI(_) => {}
            Event::Text(value)
                if value
                    .iter()
                    .all(|b: &u8| matches!(b, b' ' | b'\t' | b'\r' | b'\n')) => {}
            Event::Eof => return root.ok_or_else(|| "missing XML document root".into()),
            _ => return Err("invalid XML document content".into()),
        }
    }
}
pub(crate) fn encode_xstring(value: &str) -> String {
    crate::helper::utils::escape_xstring(value).into_owned()
}
pub(crate) fn optional<T: ToString>(
    values: &mut Vec<(&'static str, String)>,
    name: &'static str,
    value: Option<T>,
) {
    if let Some(value) = value {
        values.push((name, value.to_string()));
    }
}
pub(crate) fn start(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    name: &str,
    attributes: Vec<(&str, String)>,
    empty: bool,
) {
    write_start_tag(
        writer,
        name.to_owned(),
        attributes
            .into_iter()
            .map(|(key, value)| (key, value).into())
            .collect(),
        empty,
    );
}
pub(crate) fn xml_error(message: impl std::fmt::Display) -> XlsxError {
    XlsxError::with_message(XlsxErrorKind::Xml, message.to_string())
}
pub(crate) fn attribute<'a>(
    node: &'a OpaqueElement,
    namespace: &str,
    name: &str,
) -> Result<Option<&'a str>, String> {
    let mut values = node
        .attributes
        .iter()
        .filter(|(key, _)| key.is(namespace, name));
    let value = values.next().map(|(_, value)| value.as_str());
    if values.next().is_some() {
        return Err(format!("duplicate {name} attribute"));
    }
    Ok(value)
}
pub(crate) fn required_attribute<'a>(
    node: &'a OpaqueElement,
    namespace: &str,
    name: &str,
) -> Result<&'a str, String> {
    attribute(node, namespace, name)?.ok_or_else(|| format!("missing {name} attribute"))
}
pub(crate) fn child<'a>(
    children: &'a [OpaqueElement],
    namespace: &str,
    name: &str,
) -> Result<Option<&'a OpaqueElement>, String> {
    let mut values = children.iter().filter(|node| node.name.is(namespace, name));
    let value = values.next();
    if values.next().is_some() {
        return Err(format!("duplicate {name} element"));
    }
    Ok(value)
}
pub(crate) fn required_child<'a>(
    children: &'a [OpaqueElement],
    namespace: &str,
    name: &str,
) -> Result<&'a OpaqueElement, String> {
    child(children, namespace, name)?.ok_or_else(|| format!("missing {name} element"))
}
