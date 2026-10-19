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
    reader:   NsReader<R>,
    scratch:  Vec<u8>,
    output:   Vec<u8>,
    consumed: usize,
    finished: bool,
}

impl<R: BufRead> SpreadsheetXml<R> {
    pub(crate) fn new(source: R) -> Self {
        Self {
            reader:   NsReader::from_reader(source),
            scratch:  Vec::new(),
            output:   Vec::new(),
            consumed: 0,
            finished: false,
        }
    }
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
