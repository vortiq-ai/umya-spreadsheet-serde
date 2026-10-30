//! Namespace-aware input for the legacy `SpreadsheetML` part parsers.
//!
//! Those parsers (including nested style/cell readers) dispatch on unprefixed
//! names. Resolve namespaces once, then feed them a canonical token stream.
//! Only one XML event is buffered: ZIP entries and stored raw XML stay intact.
//! Attribute names are resolved by URI too; values and text are unchanged.

use std::io::{
    self,
    BufRead,
    Read,
};

use quick_xml::{
    NsReader,
    Writer,
    events::{
        BytesEnd,
        BytesStart,
        Event,
    },
    name::{
        Namespace,
        PrefixDeclaration,
        QName,
        ResolveResult,
    },
};

use crate::helper::const_str::SHEET_MAIN_NS;

pub(crate) const RESOLVED_ATTRIBUTES: &[u8] = b"__umya_resolved_attributes";
const DOCUMENT_REL_NS: &[u8] =
    b"http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const XML_NS: &[u8] = b"http://www.w3.org/XML/1998/namespace";

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

pub(crate) struct SpreadsheetXml<R> {
    reader:       NsReader<R>,
    scratch:      Vec<u8>,
    output:       Vec<u8>,
    consumed:     usize,
    finished:     bool,
    native_depth: usize,
}

impl<R: BufRead> SpreadsheetXml<R> {
    pub(crate) fn new(source: R) -> Self {
        Self {
            reader:       NsReader::from_reader(source),
            scratch:      Vec::new(),
            output:       Vec::new(),
            consumed:     0,
            finished:     false,
            native_depth: 0,
        }
    }
}

fn is_native_owner<R>(reader: &NsReader<R>, element: &BytesStart<'_>) -> bool {
    let (namespace, local) = reader.resolver().resolve_element(element.name());
    matches!(namespace, ResolveResult::Bound(Namespace(uri)) if uri == SHEET_MAIN_NS.as_bytes())
        && matches!(local.as_ref(), b"oleObjects" | b"autoFilter")
}

pub(crate) fn is_native_start(element: &BytesStart<'_>, local: &[u8]) -> bool {
    if element.local_name().as_ref() != local {
        return false;
    }
    crate::drawing::charts::NamespaceScope::default()
        .child(element)
        .and_then(|scope| scope.resolve(element.name().as_ref(), false))
        .is_ok_and(|name| name.namespace_uri == SHEET_MAIN_NS)
}

// Native subtree readers resolve qualified names themselves. Give their root
// the source bindings, not the legacy adapter's rewritten names/attributes.
fn native_start<R>(
    reader: &NsReader<R>,
    element: &BytesStart<'_>,
) -> io::Result<BytesStart<'static>> {
    let mut result = element.to_owned();
    result.clear_attributes();
    for attribute in element.attributes() {
        let attribute = attribute.map_err(|error| invalid(error.to_string()))?;
        if attribute.key.as_namespace_binding().is_none() {
            result.push_attribute(attribute);
        }
    }
    for (prefix, namespace) in reader.resolver().bindings() {
        let key = match prefix {
            PrefixDeclaration::Default => b"xmlns".to_vec(),
            PrefixDeclaration::Named(prefix) => [b"xmlns:", prefix].concat(),
        };
        result.push_attribute((key.as_slice(), namespace.as_ref()));
    }
    Ok(result)
}

fn canonical_name<R>(reader: &NsReader<R>, name: QName<'_>) -> io::Result<Option<Vec<u8>>> {
    let (namespace, local) = reader.resolver().resolve_element(name);
    Ok(match namespace {
        ResolveResult::Bound(Namespace(ns)) if ns == SHEET_MAIN_NS.as_bytes() => {
            name.prefix().map(|_| local.as_ref().to_vec())
        }
        // A foreign default namespace must not masquerade as e.g. <sheet> or
        // <row>. The private prefix only prevents dispatch in the legacy parser;
        // this token stream is internal and is never written back to the XLSX.
        ResolveResult::Bound(_) | ResolveResult::Unbound if name.prefix().is_none() => {
            let mut qualified = b"__umya_foreign:".to_vec();
            qualified.extend_from_slice(local.as_ref());
            Some(qualified)
        }
        ResolveResult::Unknown(prefix) => {
            return Err(invalid(format!(
                "Undeclared XML prefix: {}",
                String::from_utf8_lossy(&prefix)
            )));
        }
        // Preserve extension element names (notably x14:dataValidations).
        _ => None,
    })
}

fn canonical_start<R>(
    reader: &NsReader<R>,
    element: &BytesStart<'_>,
) -> io::Result<BytesStart<'static>> {
    let name =
        canonical_name(reader, element.name())?.unwrap_or_else(|| element.name().as_ref().to_vec());
    let mut result = BytesStart::new(String::from_utf8(name).map_err(|e| invalid(e.to_string()))?);
    let mut expanded_names = std::collections::HashSet::new();
    for attr in element.attributes() {
        let attr = attr.map_err(|e| invalid(e.to_string()))?;
        // Namespace declarations have already been consumed by NsReader.
        if attr.key.as_ref() == b"xmlns"
            || attr.key.prefix().is_some_and(|p| p.as_ref() == b"xmlns")
        {
            continue;
        }
        let (namespace, local) = reader.resolver().resolve_attribute(attr.key);
        let uri = match namespace {
            ResolveResult::Bound(Namespace(uri)) => uri,
            ResolveResult::Unbound => b"",
            ResolveResult::Unknown(prefix) => {
                return Err(invalid(format!(
                    "Undeclared XML attribute prefix: {}",
                    String::from_utf8_lossy(&prefix)
                )));
            }
        };
        if !expanded_names.insert((uri.to_vec(), local.as_ref().to_vec())) {
            return Err(invalid("Duplicate expanded XML attribute name"));
        }
        let prefix: &[u8] = match uri {
            DOCUMENT_REL_NS => b"r:",
            XML_NS => b"xml:",
            b"" => b"",
            // Unsupported foreign attributes must never satisfy a known
            // attribute lookup, even if the producer uses the prefix `r`.
            _ => continue,
        };
        let mut key = prefix.to_vec();
        key.extend_from_slice(local.as_ref());
        if key != RESOLVED_ATTRIBUTES {
            result.push_attribute((key.as_slice(), attr.value.as_ref()));
        }
    }
    // Disable the legacy driver's namespace-blind local-name fallback only for
    // this internal, URI-resolved token stream. Other part readers are
    // unchanged.
    result.push_attribute((RESOLVED_ATTRIBUTES, b"1".as_slice()));
    Ok(result)
}

/// The legacy nested workbook readers are infallible. Validate the small
/// metadata part before handing it to them, without buffering the entire part.
pub(crate) fn validate_workbook<R: BufRead>(source: R) -> io::Result<()> {
    let mut reader = quick_xml::Reader::from_reader(SpreadsheetXml::new(source));
    let mut scratch = Vec::new();
    let mut root_seen = false;
    loop {
        match reader
            .read_event_into(&mut scratch)
            .map_err(|e| invalid(e.to_string()))?
        {
            Event::Start(ref element) | Event::Empty(ref element) => {
                if !root_seen {
                    if element.name().as_ref() != b"workbook" {
                        return Err(invalid(
                            "Expected a workbook root in the SpreadsheetML main namespace",
                        ));
                    }
                    root_seen = true;
                }
                if element.name().as_ref() == b"sheet" {
                    for key in [b"r:id".as_slice(), b"name", b"sheetId"] {
                        if element
                            .try_get_attribute(key)
                            .map_err(|e| invalid(e.to_string()))?
                            .is_none_or(|a| a.value.is_empty())
                        {
                            return Err(invalid(format!(
                                "Worksheet is missing required attribute {} in its expected \
                                 namespace",
                                String::from_utf8_lossy(key)
                            )));
                        }
                    }
                }
            }
            Event::Eof => break,
            _ => (),
        }
        scratch.clear();
    }
    if !root_seen {
        return Err(invalid("Missing SpreadsheetML workbook root"));
    }
    Ok(())
}

impl<R: BufRead> BufRead for SpreadsheetXml<R> {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        if self.consumed == self.output.len() && !self.finished {
            self.scratch.clear();
            self.output.clear();
            self.consumed = 0;
            let event = self
                .reader
                .read_event_into(&mut self.scratch)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
            let event = match event {
                Event::Start(element) if self.native_depth > 0 => {
                    self.native_depth += 1;
                    Event::Start(element)
                }
                Event::Start(element) if is_native_owner(&self.reader, &element) => {
                    self.native_depth = 1;
                    Event::Start(native_start(&self.reader, &element)?)
                }
                Event::Empty(element) if self.native_depth > 0 => Event::Empty(element),
                Event::Empty(element) if is_native_owner(&self.reader, &element) => {
                    Event::Empty(native_start(&self.reader, &element)?)
                }
                Event::End(element) if self.native_depth > 0 => {
                    self.native_depth -= 1;
                    Event::End(element)
                }
                Event::Start(element) => Event::Start(canonical_start(&self.reader, &element)?),
                Event::Empty(element) => Event::Empty(canonical_start(&self.reader, &element)?),
                Event::End(element) => match canonical_name(&self.reader, element.name())? {
                    Some(name) => {
                        Event::End(BytesEnd::new(String::from_utf8(name).map_err(|error| {
                            io::Error::new(io::ErrorKind::InvalidData, error)
                        })?))
                    }
                    None => Event::End(element),
                },
                Event::Eof => {
                    self.finished = true;
                    Event::Eof
                }
                other => other,
            };
            Writer::new(&mut self.output).write_event(event)?;
        }
        Ok(&self.output[self.consumed..])
    }

    fn consume(&mut self, amount: usize) {
        self.consumed += amount.min(self.output.len() - self.consumed);
    }
}

impl<R: BufRead> Read for SpreadsheetXml<R> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if output.is_empty() {
            return Ok(0);
        }
        let available = self.fill_buf()?;
        let amount = available.len().min(output.len());
        output[..amount].copy_from_slice(&available[..amount]);
        self.consume(amount);
        Ok(amount)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_filter_retains_inherited_bindings_and_rejects_source_marker() {
        for prefix in ["", "s:"] {
            let xml = format!(
                r#"<worksheet xmlns="{SHEET_MAIN_NS}" xmlns:s="{SHEET_MAIN_NS}" xmlns:rev="http://schemas.microsoft.com/office/spreadsheetml/2014/revision"><{prefix}autoFilter ref="A1:B5" rev:uid="native-uid"><{prefix}filterColumn colId="0"><{prefix}filters><{prefix}filter val="A"/></{prefix}filters></{prefix}filterColumn></{prefix}autoFilter><sheetData/></worksheet>"#
            );
            let mut reader = quick_xml::Reader::from_reader(SpreadsheetXml::new(xml.as_bytes()));
            let mut buffer = Vec::new();
            let mut found = false;
            let mut resumed = false;
            loop {
                match reader.read_event_into(&mut buffer).unwrap() {
                    Event::Start(owner) if is_native_start(&owner, b"autoFilter") => {
                        let mut filter = crate::AutoFilter::default();
                        filter.set_attributes(&mut reader, &owner, false).unwrap();
                        assert_eq!(filter.filter_columns().len(), 1);
                        assert!(
                            String::from_utf8_lossy(&filter.to_xml().unwrap())
                                .contains("native-uid")
                        );
                        found = true;
                    }
                    Event::Empty(owner) if owner.name().as_ref() == b"sheetData" => resumed = true,
                    Event::Eof => break,
                    _ => (),
                }
                buffer.clear();
            }
            assert!(found && resumed);
        }
        let xml = format!(
            r#"<worksheet xmlns="{SHEET_MAIN_NS}"><autoFilter ref="A1:B5" __umya_resolved_attributes="1"/></worksheet>"#
        );
        let mut reader = quick_xml::Reader::from_reader(SpreadsheetXml::new(xml.as_bytes()));
        let mut buffer = Vec::new();
        loop {
            match reader.read_event_into(&mut buffer).unwrap() {
                Event::Empty(owner) if is_native_start(&owner, b"autoFilter") => {
                    assert!(
                        crate::AutoFilter::default()
                            .set_attributes(&mut reader, &owner, true)
                            .is_err()
                    );
                    break;
                }
                Event::Eof => panic!("native filter not found"),
                _ => (),
            }
            buffer.clear();
        }
    }

    #[test]
    fn native_ole_subtree_retains_inherited_namespaces_and_attributes() {
        use crate::{
            drawing::charts::{
                NamespaceScope,
                OpaqueElement,
            },
            helper::const_str::{
                MC_NS,
                REL_OFC_NS,
                SHEET_MS_MAIN_NS,
            },
        };

        let xml = format!(
            r#"<s:worksheet xmlns:s="{SHEET_MAIN_NS}" xmlns:compat="{MC_NS}" xmlns:next="{SHEET_MS_MAIN_NS}" xmlns:rel="{REL_OFC_NS}" xmlns:custom="urn:custom"><s:oleObjects><compat:AlternateContent><compat:Choice Requires="next"><s:oleObject shapeId="4501" rel:id="rId1" custom:kind="next:Value"/></compat:Choice><compat:Fallback><s:oleObject xmlns:rel="urn:foreign" shapeId="4501" rel:id="not-a-relationship"/></compat:Fallback></compat:AlternateContent></s:oleObjects><s:sheetData/></s:worksheet>"#
        );
        let mut reader = quick_xml::Reader::from_reader(SpreadsheetXml::new(xml.as_bytes()));
        let mut buffer = Vec::new();
        let mut captured = None;
        let mut resumed = false;
        loop {
            match reader.read_event_into(&mut buffer).unwrap() {
                Event::Start(owner) if owner.local_name().as_ref() == b"oleObjects" => {
                    captured = Some(
                        OpaqueElement::read(&mut reader, &owner, false, &NamespaceScope::default())
                            .unwrap(),
                    );
                }
                Event::Empty(owner) if owner.name().as_ref() == b"sheetData" => resumed = true,
                Event::Eof => break,
                _ => (),
            }
            buffer.clear();
        }
        let owner = captured.unwrap();
        assert!(owner.name.is(SHEET_MAIN_NS, "oleObjects"));
        let compatibility = crate::xml::direct_children(&owner).unwrap().remove(0);
        assert!(compatibility.name.is(MC_NS, "AlternateContent"));
        let branches = crate::xml::direct_children(&compatibility).unwrap();
        let choice = &branches[0];
        let bindings = NamespaceScope::with_bindings(choice.namespaces.clone());
        assert_eq!(
            bindings
                .resolve(b"next:Value", false)
                .unwrap()
                .namespace_uri,
            SHEET_MS_MAIN_NS
        );
        let primary = crate::xml::direct_children(choice).unwrap().remove(0);
        assert_eq!(
            crate::xml::attribute(&primary, REL_OFC_NS, "id").unwrap(),
            Some("rId1")
        );
        assert_eq!(
            crate::xml::attribute(&primary, "urn:custom", "kind").unwrap(),
            Some("next:Value")
        );
        let fallback = crate::xml::direct_children(&branches[1]).unwrap().remove(0);
        assert_eq!(
            crate::xml::attribute(&fallback, "urn:foreign", "id").unwrap(),
            Some("not-a-relationship")
        );
        assert!(
            crate::xml::attribute(&fallback, REL_OFC_NS, "id")
                .unwrap()
                .is_none()
        );
        assert!(resumed);
    }

    #[test]
    fn native_ole_subtree_rejects_undeclared_prefixes() {
        use crate::drawing::charts::{
            NamespaceScope,
            OpaqueElement,
        };

        for child in [
            "<missing:AlternateContent/>",
            "<oleObject missing:id=\"rId1\"/>",
        ] {
            let xml = format!(
                r#"<worksheet xmlns="{SHEET_MAIN_NS}"><oleObjects>{child}</oleObjects></worksheet>"#,
            );
            let mut reader = quick_xml::Reader::from_reader(SpreadsheetXml::new(xml.as_bytes()));
            let mut buffer = Vec::new();
            let mut rejected = false;
            loop {
                match reader.read_event_into(&mut buffer).unwrap() {
                    Event::Start(owner) if owner.local_name().as_ref() == b"oleObjects" => {
                        rejected = OpaqueElement::read(
                            &mut reader,
                            &owner,
                            false,
                            &NamespaceScope::default(),
                        )
                        .is_err();
                        break;
                    }
                    Event::Eof => break,
                    _ => (),
                }
                buffer.clear();
            }
            assert!(rejected, "undeclared prefix accepted in {child}");
        }
    }

    #[test]
    fn nested_prefix_rebinding_preserves_foreign_names_and_text() {
        let xml = format!(
            r#"<s:worksheet xmlns:s="{SHEET_MAIN_NS}"><s:sheetData><s:row r="1"><s:c r="A1"><s:v>7</s:v></s:c></s:row><s:row xmlns:s="urn:foreign" r="2"><s:c/></s:row><alias:row xmlns:alias="{SHEET_MAIN_NS}" r="3"/></s:sheetData><s:headerFooter><s:oddHeader>&amp;CResearch &amp; Development<![CDATA[<literal>]]></s:oddHeader></s:headerFooter></s:worksheet>"#
        );
        let mut reader = SpreadsheetXml::new(xml.as_bytes());
        let mut normalized = String::new();
        reader.read_to_string(&mut normalized).unwrap();
        assert!(normalized.contains("<row r=\"1\""));
        assert!(normalized.contains("<c r=\"A1\""));
        assert!(normalized.contains(">7</v></c></row>"));
        assert!(normalized.contains("<s:row r=\"2\""));
        assert!(normalized.contains("<s:c"));
        assert!(!normalized.contains("<row r=\"2\""));
        assert!(normalized.contains("<row r=\"3\""));
        assert!(normalized.contains("&amp;CResearch &amp; Development<![CDATA[<literal>]]>"));
        assert!(normalized.ends_with("</oddHeader></headerFooter></worksheet>"));
    }

    #[test]
    fn foreign_default_namespace_does_not_become_a_spreadsheet_row() {
        let xml = format!(
            r#"<s:worksheet xmlns:s="{SHEET_MAIN_NS}"><s:sheetData><s:row r="1"/><row xmlns="urn:foreign" r="2"/><s:row r="3"/></s:sheetData></s:worksheet>"#
        );
        let mut reader = quick_xml::Reader::from_reader(SpreadsheetXml::new(xml.as_bytes()));
        let mut buffer = Vec::new();
        let mut rows = Vec::new();
        loop {
            match reader.read_event_into(&mut buffer).unwrap() {
                Event::Empty(element) if element.name().as_ref() == b"row" => {
                    rows.push(
                        element
                            .try_get_attribute("r")
                            .unwrap()
                            .unwrap()
                            .value
                            .into_owned(),
                    );
                }
                Event::Eof => break,
                _ => {}
            }
            buffer.clear();
        }
        assert_eq!(rows, vec![b"1".to_vec(), b"3".to_vec()]);
    }

    #[test]
    fn first_event_does_not_buffer_the_complete_worksheet() {
        let xml = format!(
            "<s:worksheet xmlns:s=\"{SHEET_MAIN_NS}\"><s:sheetData>{}</s:sheetData></s:worksheet>",
            "<s:row r=\"1\"><s:c r=\"A1\"><s:v>1</s:v></s:c></s:row>".repeat(20_000),
        );
        let mut reader = SpreadsheetXml::new(io::Cursor::new(xml.as_bytes()));
        assert!(reader.fill_buf().unwrap().starts_with(b"<worksheet "));
        assert!(reader.reader.get_ref().position() < 256);
        assert!(reader.scratch.len() < 256);
        assert!(reader.output.len() < 256);
    }
}
