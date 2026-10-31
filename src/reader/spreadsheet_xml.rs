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
    reader:         NsReader<R>,
    scratch:        Vec<u8>,
    output:         Vec<u8>,
    consumed:       usize,
    finished:       bool,
    native_depth:   usize,
    all_extensions: bool,
}

/// A part's bytes as UTF-8, whatever encoding its declaration names.
pub(crate) enum Utf8Source<R> {
    /// The part is UTF-8 (or declares nothing): its bytes pass through.
    Passthrough(io::Chain<io::Cursor<Vec<u8>>, R>),
    /// The part was decoded as a whole, so every character was checked.
    Decoded(io::Cursor<Vec<u8>>),
}

impl<R: BufRead> Read for Utf8Source<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            Self::Passthrough(source) => source.read(buf),
            Self::Decoded(source) => source.read(buf),
        }
    }
}

impl<R: BufRead> BufRead for Utf8Source<R> {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        match self {
            Self::Passthrough(source) => source.fill_buf(),
            Self::Decoded(source) => source.fill_buf(),
        }
    }

    fn consume(&mut self, amount: usize) {
        match self {
            Self::Passthrough(source) => source.consume(amount),
            Self::Decoded(source) => source.consume(amount),
        }
    }
}

/// Check that a part is text in its declared encoding without keeping it: the
/// nested readers are infallible, so a part that is not valid is refused
/// before they see it.
pub(crate) fn validate_part_text<R: BufRead>(source: R) -> io::Result<()> {
    let mut source = match utf8_source(source)? {
        Utf8Source::Decoded(_) => return Ok(()),
        Utf8Source::Passthrough(source) => source,
    };
    let mut carry: Vec<u8> = Vec::new();
    loop {
        let buffered = source.fill_buf()?;
        if buffered.is_empty() {
            break;
        }
        let length = buffered.len();
        carry.extend_from_slice(buffered);
        source.consume(length);
        match std::str::from_utf8(&carry) {
            Ok(_) => carry.clear(),
            Err(error) if error.error_len().is_none() => {
                // A character cut by the end of the buffer: finish it with the
                // next bytes.
                carry.drain(..error.valid_up_to());
            }
            Err(error) => {
                return Err(invalid(format!("text is not valid UTF-8: {error}")));
            }
        }
    }
    if carry.is_empty() {
        Ok(())
    } else {
        Err(invalid("text is not valid UTF-8: truncated character"))
    }
}

const UTF8_BOM: &[u8] = b"\xEF\xBB\xBF";
const XML_OPENING: &[u8] = b"<?xml";

/// What the bytes read so far say about the XML declaration.
enum Declaration {
    /// Too few bytes to tell.
    Unknown,
    /// The part does not start with an XML declaration.
    Absent,
    /// The part starts with a declaration whose `?>` is not read yet.
    Unterminated,
    /// The part starts with a declaration: its content lies between `start`
    /// (after `<?xml`) and `end` (the offset of `?>`).
    Complete { start: usize, end: usize },
}

fn is_xml_space(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\r' | b'\n')
}

fn is_name_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b':')
}

/// The XML declaration is recognised only at the very start of the part, after
/// a UTF-8 byte order mark if there is one. `searched` is how much of
/// `prefix` an earlier call already searched for the terminator.
fn declaration_state(prefix: &[u8], searched: usize) -> Declaration {
    let body_start = if prefix.starts_with(UTF8_BOM) {
        UTF8_BOM.len()
    } else if UTF8_BOM.starts_with(prefix) {
        return Declaration::Unknown;
    } else {
        0
    };
    let body = &prefix[body_start..];
    if body.len() < XML_OPENING.len() {
        return if XML_OPENING.starts_with(body) {
            Declaration::Unknown
        } else {
            Declaration::Absent
        };
    }
    if !body.starts_with(XML_OPENING) {
        return Declaration::Absent;
    }
    let Some(next) = body.get(XML_OPENING.len()).copied() else {
        return Declaration::Unknown;
    };
    // `<?xml-stylesheet ...?>` and the like are other processing instructions.
    if is_name_byte(next) {
        return Declaration::Absent;
    }
    let from = searched
        .saturating_sub(body_start + 1)
        .max(XML_OPENING.len());
    match body[from..].windows(2).position(|window| window == b"?>") {
        Some(offset) => Declaration::Complete {
            start: body_start + XML_OPENING.len(),
            end:   body_start + from + offset,
        },
        None => Declaration::Unterminated,
    }
}

/// A cursor over the content of an XML declaration.
struct DeclarationText<'a> {
    text: &'a [u8],
    at:   usize,
}

impl<'a> DeclarationText<'a> {
    fn skip_space(&mut self) -> bool {
        let start = self.at;
        while self.text.get(self.at).copied().is_some_and(is_xml_space) {
            self.at += 1;
        }
        self.at > start
    }

    fn word(&mut self, word: &[u8]) -> bool {
        if self.text[self.at..].starts_with(word) {
            self.at += word.len();
            true
        } else {
            false
        }
    }

    /// `Eq ::= S? '=' S?` and the quoted value that follows.
    fn quoted_value(&mut self, name: &str) -> Result<&'a [u8], String> {
        self.skip_space();
        if !self.word(b"=") {
            return Err(format!("the XML declaration has no '=' after {name}"));
        }
        self.skip_space();
        let quote = match self.text.get(self.at) {
            Some(quote @ (b'"' | b'\'')) => *quote,
            _ => return Err(format!("the XML declaration value of {name} is not quoted")),
        };
        self.at += 1;
        let rest = &self.text[self.at..];
        let length = rest
            .iter()
            .position(|byte| *byte == quote)
            .ok_or_else(|| format!("the XML declaration value of {name} is not closed"))?;
        self.at += length + 1;
        Ok(&rest[..length])
    }
}

/// Check the content of an XML declaration against its grammar
/// (`VersionInfo EncodingDecl? SDDecl? S?`, version `1.x`) and return the
/// declared encoding label.
fn parse_declaration(text: &[u8]) -> Result<Option<Vec<u8>>, String> {
    let mut cursor = DeclarationText { text, at: 0 };
    if !cursor.skip_space() || !cursor.word(b"version") {
        return Err("the XML declaration does not start with a version".into());
    }
    let version = cursor.quoted_value("version")?;
    let version_valid = version
        .strip_prefix(b"1.")
        .is_some_and(|minor| !minor.is_empty() && minor.iter().all(u8::is_ascii_digit));
    if !version_valid {
        return Err(format!(
            "unsupported XML version {}",
            String::from_utf8_lossy(version)
        ));
    }
    let mut label = None;
    let mut standalone = false;
    loop {
        let separated = cursor.skip_space();
        if cursor.at == text.len() {
            return Ok(label);
        }
        if !separated {
            return Err("the XML declaration has unexpected content".into());
        }
        if label.is_none() && !standalone && cursor.word(b"encoding") {
            let value = cursor.quoted_value("encoding")?;
            let valid = value.first().is_some_and(u8::is_ascii_alphabetic)
                && value
                    .iter()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'));
            if !valid {
                return Err(format!(
                    "invalid XML encoding name {}",
                    String::from_utf8_lossy(value)
                ));
            }
            label = Some(value.to_vec());
        } else if !standalone && cursor.word(b"standalone") {
            let value = cursor.quoted_value("standalone")?;
            if value != b"yes" && value != b"no" {
                return Err(format!(
                    "invalid XML standalone value {}",
                    String::from_utf8_lossy(value)
                ));
            }
            standalone = true;
        } else {
            return Err("the XML declaration has unexpected content".into());
        }
    }
}

/// The byte order of a UTF-16 part, from its byte order mark or, without one,
/// from the first characters of an XML declaration.
fn utf16_order(prefix: &[u8]) -> Option<&'static encoding_rs::Encoding> {
    match prefix {
        [0xFF, 0xFE, ..] | [b'<', 0, b'?', 0, ..] => Some(encoding_rs::UTF_16LE),
        [0xFE, 0xFF, ..] | [0, b'<', 0, b'?', ..] => Some(encoding_rs::UTF_16BE),
        _ => None,
    }
}

/// Decode a whole UTF-16 part strictly. Its declaration, if any, must name a
/// UTF-16 encoding (Excel refuses a UTF-16 part that declares UTF-8).
fn decode_utf16<R: BufRead>(
    mut bytes: Vec<u8>,
    mut source: R,
    order: &'static encoding_rs::Encoding,
) -> io::Result<Utf8Source<R>> {
    source.read_to_end(&mut bytes)?;
    let content = match bytes.as_slice() {
        [0xFF, 0xFE, rest @ ..] | [0xFE, 0xFF, rest @ ..] => rest,
        all => all,
    };
    let decoded = order
        .decode_without_bom_handling_and_without_replacement(content)
        .ok_or_else(|| invalid(format!("text is not valid {}", order.name())))?;
    let decoded = decoded.into_owned();
    let (start, end) = match declaration_state(decoded.as_bytes(), 0) {
        Declaration::Complete { start, end } => (start, end),
        Declaration::Unterminated => {
            return Err(invalid("the XML declaration is not terminated"));
        }
        Declaration::Unknown | Declaration::Absent => {
            return Ok(Utf8Source::Decoded(io::Cursor::new(decoded.into_bytes())));
        }
    };
    let label = parse_declaration(&decoded.as_bytes()[start..end]).map_err(invalid)?;
    let Some(label) = label else {
        return Ok(Utf8Source::Decoded(io::Cursor::new(decoded.into_bytes())));
    };
    let label = String::from_utf8_lossy(&label).into_owned();
    match encoding_rs::Encoding::for_label(label.as_bytes()) {
        Some(declared)
            if declared == encoding_rs::UTF_16LE || declared == encoding_rs::UTF_16BE => {}
        _ => {
            return Err(invalid(format!(
                "a {} part declares the encoding {label}",
                order.name()
            )));
        }
    }
    let (declaration, rest) = decoded.split_at(end);
    let decoded = format!("{}{rest}", declaration.replacen(&label, "UTF-8", 1));
    Ok(Utf8Source::Decoded(io::Cursor::new(decoded.into_bytes())))
}

/// Open a part for the legacy parsers as UTF-8. A UTF-16 part, or a part whose
/// declaration names another ASCII-compatible encoding, is decoded here as a
/// whole, so an invalid byte sequence is an error of opening the part and
/// never reaches the infallible nested readers; a malformed declaration or one
/// naming an encoding the parsers cannot read is an error too. The declaration
/// is read through its terminator however the source hands out bytes.
pub(crate) fn utf8_source<R: BufRead>(mut source: R) -> io::Result<Utf8Source<R>> {
    let mut prefix = Vec::new();
    let mut searched = 0;
    // Enough bytes to tell a UTF-16 part from the others.
    while prefix.len() < 4 {
        let buffered = source.fill_buf()?;
        if buffered.is_empty() {
            break;
        }
        let length = buffered.len();
        prefix.extend_from_slice(buffered);
        source.consume(length);
    }
    if let Some(order) = utf16_order(&prefix) {
        return decode_utf16(prefix, source, order);
    }
    let (start, end) = loop {
        match declaration_state(&prefix, searched) {
            Declaration::Complete { start, end } => break (start, end),
            Declaration::Absent => {
                return Ok(Utf8Source::Passthrough(
                    io::Cursor::new(prefix).chain(source),
                ));
            }
            Declaration::Unknown | Declaration::Unterminated => (),
        }
        searched = prefix.len();
        let buffered = source.fill_buf()?;
        if buffered.is_empty() {
            return if matches!(declaration_state(&prefix, 0), Declaration::Unterminated) {
                Err(invalid("the XML declaration is not terminated"))
            } else {
                Ok(Utf8Source::Passthrough(
                    io::Cursor::new(prefix).chain(source),
                ))
            };
        }
        let length = buffered.len();
        prefix.extend_from_slice(buffered);
        source.consume(length);
    };
    let Some(label) = parse_declaration(&prefix[start..end]).map_err(invalid)? else {
        return Ok(Utf8Source::Passthrough(
            io::Cursor::new(prefix).chain(source),
        ));
    };
    let declared = encoding_rs::Encoding::for_label(&label).ok_or_else(|| {
        invalid(format!(
            "unsupported XML encoding {}",
            String::from_utf8_lossy(&label)
        ))
    })?;
    if declared == encoding_rs::UTF_8 {
        return Ok(Utf8Source::Passthrough(
            io::Cursor::new(prefix).chain(source),
        ));
    }
    if !declared.is_ascii_compatible() {
        return Err(invalid(format!(
            "unsupported XML encoding {}",
            declared.name()
        )));
    }
    let mut bytes = prefix;
    source.read_to_end(&mut bytes)?;
    let decoded = declared
        .decode_without_bom_handling_and_without_replacement(&bytes)
        .ok_or_else(|| invalid(format!("text is not valid {}", declared.name())))?;
    // The text is UTF-8 from here on; say so in the declaration.
    let label = String::from_utf8_lossy(&label).into_owned();
    let declaration_end = decoded.find("?>").unwrap_or(0);
    let (declaration, rest) = decoded.split_at(declaration_end);
    let decoded = format!("{}{rest}", declaration.replacen(&label, "UTF-8", 1));
    Ok(Utf8Source::Decoded(io::Cursor::new(decoded.into_bytes())))
}

impl<R: BufRead> SpreadsheetXml<R> {
    /// Slicer list extensions (`ext` entries with a known slicer uri) reach
    /// the legacy parsers as native subtrees.
    pub(crate) fn new(source: R) -> Self {
        Self {
            reader:         NsReader::from_reader(source),
            scratch:        Vec::new(),
            output:         Vec::new(),
            consumed:       0,
            finished:       false,
            native_depth:   0,
            all_extensions: false,
        }
    }

    /// Like [`Self::new`], but every `ext` entry reaches the legacy parser as
    /// a native subtree, so an extension list can be kept verbatim.
    pub(crate) fn with_all_extensions(source: R) -> Self {
        Self {
            all_extensions: true,
            ..Self::new(source)
        }
    }
}

impl<R: BufRead> SpreadsheetXml<Utf8Source<R>> {
    /// A part read from a package: decoded as a whole when it declares an
    /// encoding other than UTF-8.
    pub(crate) fn open(source: R) -> io::Result<Self> {
        Ok(Self::new(utf8_source(source)?))
    }

    /// [`Self::open`] with every `ext` entry kept as a native subtree.
    pub(crate) fn open_with_all_extensions(source: R) -> io::Result<Self> {
        Ok(Self::with_all_extensions(utf8_source(source)?))
    }
}

fn is_native_owner<R>(
    reader: &NsReader<R>,
    element: &BytesStart<'_>,
    all_extensions: bool,
) -> bool {
    let (namespace, local) = reader.resolver().resolve_element(element.name());
    if !matches!(namespace, ResolveResult::Bound(Namespace(uri)) if uri == SHEET_MAIN_NS.as_bytes())
    {
        return false;
    }
    match local.as_ref() {
        b"oleObjects" | b"autoFilter" => true,
        // The data validations extension is modelled by the worksheet reader,
        // so it keeps flowing through the legacy parser.
        b"ext" => {
            all_extensions
                && !element
                    .try_get_attribute(b"uri")
                    .ok()
                    .flatten()
                    .is_some_and(|uri| {
                        crate::structs::slicer::is_data_validations_uri(uri.value.as_ref())
                    })
        }
        // Worksheet children the worksheet keeps verbatim.
        name if all_extensions && crate::structs::is_preserved_sheet_element_name(name) => true,
        _ => false,
    }
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
    let mut reader = quick_xml::Reader::from_reader(SpreadsheetXml::open(source)?);
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
                Event::Start(element)
                    if is_native_owner(&self.reader, &element, self.all_extensions) =>
                {
                    self.native_depth = 1;
                    Event::Start(native_start(&self.reader, &element)?)
                }
                Event::Empty(element) if self.native_depth > 0 => Event::Empty(element),
                Event::Empty(element)
                    if is_native_owner(&self.reader, &element, self.all_extensions) =>
                {
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
