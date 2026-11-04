use std::{
    borrow::Cow,
    io,
    path::{
        Component,
        Path,
        PathBuf,
    },
    string::FromUtf8Error,
};

use quick_xml::events::attributes::Attribute;

use crate::{
    XlsxError,
    XlsxErrorKind,
};

#[macro_export]
macro_rules! xml_read_loop {
    ($reader:ident $(,$pat:pat => $result:expr)+ $(,)?) => {
        let mut buf = Vec::new();
        loop {
            let ev = match $reader.read_event_into(&mut buf) {
                Ok(v) => v,
                Err(e) => panic!("Error at position {}: {e:?}", $reader.buffer_position()),
            };

            match ev {
                $($pat => $result,)+
                _ => (),
            }

            buf.clear();
        }
    };
}

pub(crate) use crate::xml_read_loop;

/// Same as `xml_read_loop!`, but returns the XML error from the enclosing
/// function instead of panicking. The function must return
/// `Result<_, XlsxError>`.
#[macro_export]
macro_rules! try_xml_read_loop {
    ($reader:ident $(,$pat:pat $(if $guard:expr)? => $result:expr)+ $(,)?) => {
        let mut buf = Vec::new();
        loop {
            let ev = match $reader.read_event_into(&mut buf) {
                Ok(v) => v,
                Err(e) => return Err(e.into()),
            };

            match ev {
                $($pat $(if $guard)? => $result,)+
                _ => (),
            }

            buf.clear();
        }
    };
}

pub(crate) use crate::try_xml_read_loop;

#[macro_export]
macro_rules! set_string_from_xml {
    ($self:ident, $e:ident, $attr:ident, $xml_attr:expr) => {{
        if let Some(v) = get_attribute($e, $xml_attr.as_bytes()) {
            $self.$attr.set_value_string(v);
        }
    }};
}

pub(crate) use crate::set_string_from_xml;

/// The element name with any namespace prefix removed.
///
/// The readers run non-namespace-aware (`read_event_into`, not
/// `read_resolved_event`), so an element arrives under whatever prefix the
/// PRODUCER chose. Excel writes `<c:chartSpace>`; a writer that declares the
/// chart namespace as the default one writes `<chartSpace>`. Comparing against
/// a prefixed byte literal silently fails to match the second form, and a
/// chart part that matches nothing parses to an empty plot area, which
/// `TwoCellAnchor::is_support` then drops without a word.
///
/// Safe to compare on within one part: the chart readers use 137 distinct
/// local names and not one of them appears under two different prefixes.
#[inline]
pub(crate) fn local_name(name: &[u8]) -> &[u8] {
    match name.iter().position(|&b| b == b':') {
        Some(i) => &name[i + 1..],
        None => name,
    }
}

/// Reads past the end of the element `start` has just opened, discarding
/// everything inside.
///
/// A reader that models only some of an element's children calls this for the
/// others. Without it their descendants surface in the reader's own loop, under
/// names the reader does model (`c:dPt/c:idx`, `c:dPt/c:marker`,
/// `c:minorGridlines/c:spPr`, ...), and overwrite the element's own values.
/// Call it only for a start tag, never for a self-closing one.
pub(crate) fn skip_element<R: io::BufRead>(
    reader: &mut quick_xml::Reader<R>,
    start: &quick_xml::events::BytesStart,
) -> Result<(), XlsxError> {
    reader.read_to_end_into(start.name(), &mut Vec::new())?;
    Ok(())
}

/// Rewrites attribute-only chart leaves whose start tag is immediately followed
/// by its end tag (`<c:grouping val="x"></c:grouping>`) into the self-closing
/// form, outside extension and raw string-cache owners.
///
/// The chart and drawing readers match many leaves only as `Event::Empty`, so
/// the paired form would be skipped and its value would fall back to the
/// default. The rewrite keeps the byte length (the freed bytes become blanks
/// inside the tag, before the new `/>`), so every offset the readers record
/// still indexes the original part. Comments, processing instructions, CDATA
/// and `extLst` content are left alone, including typed extension content.
/// String-cache readers already handle both forms; their first pass and
/// deferred replay must see the same original cache grammar. The input is
/// borrowed unless a rewrite is needed. Empty containers retain their Start/End
/// events so readers preserve their presence, even when they have attributes.
pub(crate) fn collapse_empty_pairs(data: &[u8]) -> Cow<'_, [u8]> {
    let mut rewritten: Option<Vec<u8>> = None;
    let mut i = 0;
    let mut preserved_depth = 0;
    let mut parents = Vec::new();
    while let Some(offset) = data[i..].iter().position(|&b| b == b'<') {
        i += offset;
        let rest = &data[i..];
        let skip_to = |terminator: &[u8]| {
            rest.windows(terminator.len())
                .position(|w| w == terminator)
                .map_or(data.len(), |p| i + p + terminator.len())
        };
        if rest.starts_with(b"<!--") {
            i = skip_to(b"-->");
            continue;
        }
        if rest.starts_with(b"<![CDATA[") {
            i = skip_to(b"]]>");
            continue;
        }
        if rest.starts_with(b"<?") {
            i = skip_to(b"?>");
            continue;
        }
        // Find the closing `>` of this tag, past quoted attribute values.
        let mut quote = None;
        let mut close = None;
        for (p, &b) in rest.iter().enumerate().skip(1) {
            match (quote, b) {
                (Some(q), b) if b == q => quote = None,
                (None, b'"' | b'\'') => quote = Some(b),
                (None, b'>') => {
                    close = Some(i + p);
                    break;
                }
                _ => (),
            }
        }
        let Some(close) = close else { break };
        let start = i;
        i = close + 1;
        if rest[1] == b'/' {
            parents.pop();
            if preserved_depth > 0 {
                preserved_depth -= 1;
            }
            continue;
        }
        if rest[1] == b'!' || data[close - 1] == b'/' {
            continue;
        }
        let name_len = rest[1..]
            .iter()
            .position(|&b| matches!(b, b' ' | b'\t' | b'\r' | b'\n' | b'>'))
            .unwrap_or(0);
        let name = &data[start + 1..start + 1 + name_len];
        let parent = parents.last().copied().map(local_name);
        parents.push(name);
        if preserved_depth > 0 || matches!(local_name(name), b"extLst" | b"strCache") {
            // Preserve source-owned XML, including nested owners and comments
            // spelling an end tag.
            preserved_depth += 1;
            continue;
        }
        if !is_chart_attribute_leaf(local_name(name), parent) {
            continue;
        }
        let tail = &data[i..];
        if !(tail.starts_with(b"</") && tail[2..].starts_with(name)) {
            continue;
        }
        let after_name = &tail[2 + name_len..];
        let blanks = after_name
            .iter()
            .take_while(|b| matches!(b, b' ' | b'\t' | b'\r' | b'\n'))
            .count();
        if after_name.get(blanks) == Some(&b'>') {
            let end_len = 2 + name_len + blanks + 1;
            let out = rewritten.get_or_insert_with(|| data.to_vec());
            out[close..close + end_len - 1].fill(b' ');
            out[close + end_len - 1] = b'/';
            out[close + end_len] = b'>';
            i += end_len;
            parents.pop();
        }
    }
    rewritten.map_or(Cow::Borrowed(data), Cow::Owned)
}

// Attribute-only chart leaves from dml-chart.xsd. Everything else keeps its
// Start/End events, since readers use those events to retain empty owners.
// Three local names have both scalar and container grammars.
fn is_chart_attribute_leaf(name: &[u8], parent: Option<&[u8]>) -> bool {
    match name {
        b"marker" => parent == Some(b"lineChart"),
        b"val" => parent == Some(b"errBars"),
        b"data" => parent == Some(b"protection"),
        _ => matches!(
            name,
            b"applyToEnd"
                | b"applyToFront"
                | b"applyToSides"
                | b"auto"
                | b"autoTitleDeleted"
                | b"autoUpdate"
                | b"axId"
                | b"axPos"
                | b"backward"
                | b"barDir"
                | b"baseTimeUnit"
                | b"bubble3D"
                | b"bubbleScale"
                | b"builtInUnit"
                | b"chartObject"
                | b"crossAx"
                | b"crossBetween"
                | b"crosses"
                | b"crossesAt"
                | b"custUnit"
                | b"dLblPos"
                | b"date1904"
                | b"delete"
                | b"depthPercent"
                | b"dispBlanksAs"
                | b"dispEq"
                | b"dispRSqr"
                | b"errBarType"
                | b"errDir"
                | b"errValType"
                | b"explosion"
                | b"firstSliceAng"
                | b"fmtId"
                | b"formatting"
                | b"forward"
                | b"gapDepth"
                | b"gapWidth"
                | b"grouping"
                | b"h"
                | b"hMode"
                | b"hPercent"
                | b"holeSize"
                | b"idx"
                | b"intercept"
                | b"invertIfNegative"
                | b"lang"
                | b"layoutTarget"
                | b"lblAlgn"
                | b"lblOffset"
                | b"legacyDrawingHF"
                | b"legendPos"
                | b"logBase"
                | b"majorTickMark"
                | b"majorTimeUnit"
                | b"majorUnit"
                | b"max"
                | b"min"
                | b"minorTickMark"
                | b"minorTimeUnit"
                | b"minorUnit"
                | b"noEndCap"
                | b"noMultiLvlLbl"
                | b"numFmt"
                | b"ofPieType"
                | b"order"
                | b"orientation"
                | b"overlap"
                | b"overlay"
                | b"pageMargins"
                | b"pageSetup"
                | b"period"
                | b"perspective"
                | b"pictureFormat"
                | b"pictureStackUnit"
                | b"plotVisOnly"
                | b"ptCount"
                | b"rAngAx"
                | b"radarStyle"
                | b"rotX"
                | b"rotY"
                | b"roundedCorners"
                | b"scatterStyle"
                | b"secondPiePt"
                | b"secondPieSize"
                | b"selection"
                | b"shape"
                | b"showBubbleSize"
                | b"showCatName"
                | b"showDLblsOverMax"
                | b"showHorzBorder"
                | b"showKeys"
                | b"showLeaderLines"
                | b"showLegendKey"
                | b"showNegBubbles"
                | b"showOutline"
                | b"showPercent"
                | b"showSerName"
                | b"showVal"
                | b"showVertBorder"
                | b"size"
                | b"sizeRepresents"
                | b"smooth"
                | b"splitPos"
                | b"splitType"
                | b"style"
                | b"symbol"
                | b"thickness"
                | b"tickLblPos"
                | b"tickLblSkip"
                | b"tickMarkSkip"
                | b"trendlineType"
                | b"userInterface"
                | b"userShapes"
                | b"varyColors"
                | b"w"
                | b"wMode"
                | b"wireframe"
                | b"x"
                | b"xMode"
                | b"y"
                | b"yMode"
        ),
    }
}

pub(crate) fn normalize_path(path: &str) -> PathBuf {
    let path = Path::new(path);
    let mut components = path.components().peekable();
    let mut ret = if let Some(c @ Component::Prefix(..)) = components.peek().copied() {
        components.next();
        PathBuf::from(c.as_os_str())
    } else {
        PathBuf::new()
    };

    for component in components {
        match component {
            Component::Prefix(..) => unreachable!(),
            Component::RootDir => {
                ret.push(component.as_os_str());
            }
            Component::CurDir => {}
            Component::ParentDir => {
                ret.pop();
            }
            Component::Normal(c) => {
                ret.push(c);
            }
        }
    }
    ret
}

#[inline]
pub(crate) fn join_paths(base_path: &str, target: &str) -> String {
    match target.split_once('/') {
        Some(("", target)) => normalize_path_to_str(target),
        // A part at the package root has an empty directory.
        _ if base_path.is_empty() => normalize_path_to_str(target),
        _ => normalize_path_to_str(&format!("{base_path}/{target}")),
    }
}

#[inline]
pub(crate) fn normalize_path_to_str(path: &str) -> String {
    let ret = normalize_path(path);
    ret.to_str().unwrap_or("").replace('\\', "/")
}

/// Look up a zip entry by name, tolerating backslash path separators and
/// case differences that appear in XLSX files generated by some Windows tools.
///
/// Try order:
/// 1. Exact match (fast path — zero overhead for well-formed files).
/// 2. Backslash-separated variant of the requested name.
/// 3. Case-insensitive scan with separator normalization (last resort).
pub(crate) fn zip_by_name<'a, R: io::Read + io::Seek>(
    arv: &'a mut zip::ZipArchive<R>,
    name: &str,
) -> Result<zip::read::ZipFile<'a, R>, zip::result::ZipError> {
    // 1. Exact match (common case)
    if arv.by_name(name).is_ok() {
        // Re-borrow: `by_name` returns a ZipFile that borrows `arv`, so we
        // must call it again after the probe to satisfy the borrow checker.
        return arv.by_name(name);
    }

    // 2. Backslash variant: "xl/workbook.xml" → "xl\\workbook.xml"
    let backslash_name = name.replace('/', "\\");
    if backslash_name != name && arv.by_name(&backslash_name).is_ok() {
        return arv.by_name(&backslash_name);
    }

    // 3. Case-insensitive scan with separator normalization
    let normalized = name.replace('\\', "/").to_ascii_lowercase();
    for i in 0..arv.len() {
        let entry_name = match arv.by_index(i) {
            Ok(f) => f.name()?.into_owned(),
            Err(_) => continue,
        };
        let entry_normalized = entry_name.replace('\\', "/").to_ascii_lowercase();
        if entry_normalized == normalized {
            return arv.by_name(&entry_name);
        }
    }

    Err(zip::result::ZipError::FileNotFound)
}

/// Look up an XML attribute by key, falling back to a match on just the
/// local name (after the colon) when the exact prefixed key is not found.
///
/// This handles XLSX files that use non-standard namespace prefixes
/// (e.g. `d3p1:id` instead of `r:id`).
#[inline]
/// Same as `get_attribute`, but returns an error if the attribute is missing.
pub(crate) fn get_required_attribute(
    e: &quick_xml::events::BytesStart<'_>,
    key: &[u8],
) -> Result<String, XlsxError> {
    get_attribute(e, key).ok_or_else(|| {
        XlsxError::with_message(
            XlsxErrorKind::Xml,
            format!(
                "missing attribute '{}' in element '{}'",
                String::from_utf8_lossy(key),
                String::from_utf8_lossy(e.name().into_inner()),
            ),
        )
    })
}

fn visit_attributes(
    element: &quick_xml::events::BytesStart<'_>,
    mut visit: impl FnMut(&[u8], &str),
) -> Result<(), XlsxError> {
    for attribute in element.attributes() {
        let attribute = attribute
            .map_err(|error| XlsxError::with_message(XlsxErrorKind::Xml, error.to_string()))?;
        let decoded = attribute.normalized_value(quick_xml::XmlVersion::Implicit1_0)?;
        visit(attribute.key.as_ref(), &decoded);
    }
    Ok(())
}

pub(crate) fn validate_attributes(
    element: &quick_xml::events::BytesStart<'_>,
) -> Result<(), XlsxError> {
    visit_attributes(element, |_, _| {})
}

/// Read an exact attribute name, validating and decoding the complete attribute
/// list.
pub(crate) fn read_attribute(
    element: &quick_xml::events::BytesStart<'_>,
    key: &[u8],
) -> Result<Option<String>, XlsxError> {
    let mut value = None;
    visit_attributes(element, |name, decoded| {
        if name == key {
            value = Some(decoded.to_owned());
        }
    })?;
    Ok(value)
}

pub(crate) fn require_attribute(
    element: &quick_xml::events::BytesStart<'_>,
    key: &[u8],
) -> Result<String, XlsxError> {
    read_attribute(element, key)?.ok_or_else(|| {
        XlsxError::with_message(
            XlsxErrorKind::Xml,
            format!(
                "missing attribute '{}' in element '{}'",
                String::from_utf8_lossy(key),
                String::from_utf8_lossy(element.name().as_ref()),
            ),
        )
    })
}

pub(crate) fn read_enum_attribute<T: crate::EnumTrait + std::str::FromStr>(
    element: &quick_xml::events::BytesStart<'_>,
    key: &[u8],
) -> Result<Option<T>, XlsxError> {
    read_attribute(element, key)?
        .map(|value| {
            T::from_str(crate::xml::trim_xml_whitespace(&value)).map_err(|_| {
                XlsxError::with_message(
                    XlsxErrorKind::Xml,
                    format!(
                        "invalid attribute '{}' in element '{}': {value:?}",
                        String::from_utf8_lossy(key),
                        String::from_utf8_lossy(element.name().as_ref()),
                    ),
                )
            })
        })
        .transpose()
}

pub(crate) fn require_enum_attribute<T: crate::EnumTrait + std::str::FromStr>(
    element: &quick_xml::events::BytesStart<'_>,
    key: &[u8],
) -> Result<T, XlsxError> {
    read_enum_attribute(element, key)?.ok_or_else(|| {
        XlsxError::with_message(
            XlsxErrorKind::Xml,
            format!(
                "missing attribute '{}' in element '{}'",
                String::from_utf8_lossy(key),
                String::from_utf8_lossy(element.name().as_ref()),
            ),
        )
    })
}

pub(crate) fn get_attribute(e: &quick_xml::events::BytesStart<'_>, key: &[u8]) -> Option<String> {
    // 1. Exact match (fast path)
    let result = e
        .attributes()
        .with_checks(false)
        .find_map(|attr| match attr {
            Ok(ref attr) if attr.key.into_inner() == key => {
                Some(get_attribute_value(attr).unwrap())
            }
            _ => None,
        });
    if result.is_some() {
        return result;
    }

    if e.try_get_attribute(super::spreadsheet_xml::RESOLVED_ATTRIBUTES)
        .ok()
        .flatten()
        .is_some()
    {
        return None;
    }

    // 2. For namespaced keys like "r:id", fall back to matching the local name
    //    ("id") against any attribute whose local name matches. Returns early
    //    if ':' is not found.
    let local_name = {
        let pos = key.iter().position(|&b| b == b':')?;
        &key[pos + 1..]
    };

    e.attributes()
        .with_checks(false)
        .find_map(|attr| match attr {
            Ok(ref attr) => {
                let attr_key = attr.key.into_inner();
                let attr_local = match attr_key.iter().position(|&b| b == b':') {
                    Some(pos) => &attr_key[pos + 1..],
                    None => attr_key,
                };
                if attr_local == local_name {
                    Some(get_attribute_value(attr).unwrap())
                } else {
                    None
                }
            }
            _ => None,
        })
}

#[inline]
pub(crate) fn get_attribute_value(attr: &Attribute) -> Result<String, FromUtf8Error> {
    String::from_utf8(attr.value.to_vec())
}

/// The raw value of the first attribute whose key is exactly `key`, borrowed
/// from the element. This selects the attribute `get_attribute` selects for a
/// key without a prefix, which never takes its local-name fallback.
#[inline]
pub(crate) fn raw_attribute<'a>(
    e: &'a quick_xml::events::BytesStart<'_>,
    key: &[u8],
) -> Option<Cow<'a, [u8]>> {
    e.attributes()
        .with_checks(false)
        .find_map(|attr| match attr {
            Ok(attr) if attr.key.into_inner() == key => Some(attr.value),
            _ => None,
        })
}

/// `get_attribute` for a key without a prefix, borrowed. Entities are not
/// unescaped. An attribute that is not valid UTF-8 fails inside
/// `get_attribute`, so the failure is the one that function has always raised.
#[inline]
pub(crate) fn attribute_str<'a>(
    e: &'a quick_xml::events::BytesStart<'_>,
    key: &[u8],
) -> Option<Cow<'a, str>> {
    match raw_attribute(e, key)? {
        Cow::Borrowed(raw) => match std::str::from_utf8(raw) {
            Ok(text) => Some(Cow::Borrowed(text)),
            Err(_) => unreachable!("{:?}", get_attribute(e, key)),
        },
        Cow::Owned(raw) => Some(Cow::Owned(String::from_utf8(raw).unwrap())),
    }
}

/// `get_attribute(e, key).and_then(|value| value.parse().ok())` for a key
/// without a prefix, without allocating.
#[inline]
pub(crate) fn attribute_u32(e: &quick_xml::events::BytesStart<'_>, key: &[u8]) -> Option<u32> {
    attribute_str(e, key)?.parse().ok()
}

/// Like `attribute_u32`, for an attribute the readers used to ignore: one that
/// is not valid UTF-8 is reported to the caller instead of failing.
#[inline]
pub(crate) fn try_attribute_u32(
    e: &quick_xml::events::BytesStart<'_>,
    key: &[u8],
) -> Result<Option<u32>, std::str::Utf8Error> {
    let Some(raw) = raw_attribute(e, key) else {
        return Ok(None);
    };
    Ok(std::str::from_utf8(&raw)?.parse().ok())
}

/// Decode one XML character event using the same one-pass entity semantics for
/// simple-text and element-only owners.
pub(crate) fn decode_xml_character_event(
    event: &quick_xml::events::Event<'_>,
) -> Result<Option<String>, XlsxError> {
    use quick_xml::events::Event;
    let value = match event {
        Event::Text(text) => {
            let decoded = text.decode()?;
            quick_xml::escape::unescape(&decoded)
                .map_err(|error| crate::xml::xml_error(error.to_string()))?
                .into_owned()
        }
        Event::CData(text) => text.decode()?.into_owned(),
        Event::GeneralRef(reference) => {
            let escaped = format!("&{};", reference.decode()?);
            quick_xml::escape::unescape(&escaped)
                .map_err(|error| crate::xml::xml_error(error.to_string()))?
                .into_owned()
        }
        _ => return Ok(None),
    };
    Ok(Some(value))
}

/// Read a simple-text owner, preserving significant whitespace and one-pass
/// entity decoding. Nested content remains a semantic conflict; malformed XML
/// and invalid character data are errors. Caller settings are restored on exit.
pub(crate) fn read_simple_text<R: io::BufRead>(
    reader: &mut quick_xml::Reader<R>,
    e: &quick_xml::events::BytesStart,
    empty: bool,
) -> Result<Option<String>, XlsxError> {
    use quick_xml::events::Event;

    if empty {
        return Ok(Some(String::new()));
    }
    let config = reader.config().clone();
    reader.config_mut().trim_text(false);
    let end = e.name().into_inner().to_vec();
    let result = (|| {
        let mut value = String::new();
        let mut buffer = Vec::new();
        let mut invalid = false;
        loop {
            match reader.read_event_into(&mut buffer)? {
                ref event @ (Event::Text(_) | Event::CData(_) | Event::GeneralRef(_)) => {
                    if let Some(text) = decode_xml_character_event(event)? {
                        value.push_str(&text);
                    }
                }
                Event::Start(child) => {
                    invalid = true;
                    reader.read_to_end_into(child.name(), &mut Vec::new())?;
                }
                Event::Empty(_) => invalid = true,
                Event::End(child) if child.name().into_inner() == end.as_slice() => {
                    return Ok((!invalid).then_some(value));
                }
                Event::End(_) | Event::Eof | Event::Decl(_) | Event::DocType(_) => {
                    return Err(crate::xml::xml_error(
                        "unterminated or invalid simple-text element",
                    ));
                }
                _ => (),
            }
            buffer.clear();
        }
    })();
    *reader.config_mut() = config;
    result
}

pub(crate) fn consume_empty_element<R: io::BufRead>(
    reader: &mut quick_xml::Reader<R>,
    element: &quick_xml::events::BytesStart<'_>,
) -> Result<(), XlsxError> {
    use quick_xml::events::Event;
    let end = element.name().as_ref().to_vec();
    let mut buffer = Vec::new();
    loop {
        match reader.read_event_into(&mut buffer)? {
            Event::End(element) if element.name().as_ref() == end => return Ok(()),
            Event::Text(text)
                if text
                    .iter()
                    .all(|byte| matches!(byte, b' ' | b'\t' | b'\r' | b'\n')) => {}
            Event::Comment(_) | Event::PI(_) => (),
            _ => {
                return Err(XlsxError::with_message(
                    XlsxErrorKind::Xml,
                    "attribute-only element has nonempty or unterminated content",
                ));
            }
        }
        buffer.clear();
    }
}

#[cfg(test)]
mod empty_content_tests {
    use super::*;

    fn consume(xml: &[u8]) -> Result<(), XlsxError> {
        let mut reader = quick_xml::Reader::from_reader(xml);
        reader.config_mut().trim_text(false);
        let quick_xml::events::Event::Start(start) = reader.read_event()? else {
            panic!("start element required");
        };
        consume_empty_element(&mut reader, &start)
    }

    #[test]
    fn explicit_empty_elements_accept_only_ignorable_content() {
        for xml in ["<leaf></leaf>", "<leaf> \t\r\n<!--comment--><?x y?></leaf>"] {
            consume(xml.as_bytes()).unwrap();
        }
        for xml in ["<leaf>text</leaf>", "<leaf><nested/></leaf>", "<leaf>"] {
            assert!(consume(xml.as_bytes()).is_err(), "{xml}");
        }
    }
}

#[cfg(test)]
mod attribute_tests {
    use super::*;

    #[test]
    fn attributes_are_decoded_once_without_local_name_guessing() {
        let element = quick_xml::events::BytesStart::from_content(
            r#"leaf val="&amp;amp;&#x9;" other:val="wrong""#,
            4,
        );
        assert_eq!(require_attribute(&element, b"val").unwrap(), "&amp;\t");
        assert!(read_attribute(&element, b"r:val").unwrap().is_none());
    }

    #[test]
    fn malformed_or_duplicate_attributes_are_not_hidden_by_a_valid_match() {
        for content in [
            r#"leaf val="one" val="two""#,
            r#"leaf val="one" bad="&missing;""#,
            r#"leaf val="one" bad"#,
        ] {
            let element = quick_xml::events::BytesStart::from_content(content, 4);
            assert!(read_attribute(&element, b"val").is_err(), "{content}");
        }
        let missing = quick_xml::events::BytesStart::new("leaf");
        assert!(require_attribute(&missing, b"val").is_err());
    }
}

#[cfg(test)]
mod collapse_empty_pairs_tests {
    use super::collapse_empty_pairs;

    #[test]
    fn empty_owners_keep_their_start_and_end_events_even_with_attributes() {
        for name in [
            "lineChart",
            "line3DChart",
            "pieChart",
            "pie3DChart",
            "doughnutChart",
            "scatterChart",
            "barChart",
            "bar3DChart",
            "radarChart",
            "bubbleChart",
            "areaChart",
            "area3DChart",
            "ofPieChart",
            "stockChart",
            "surfaceChart",
            "surface3DChart",
            "chart",
            "title",
            "legend",
            "view3D",
            "floor",
            "sideWall",
            "backWall",
            "spPr",
            "txPr",
            "dLbls",
            "dPt",
            "ser",
            "marker",
            "val",
            "data",
            "layout",
            "manualLayout",
            "p",
            "rPr",
            "gradFill",
        ] {
            for attributes in ["", " xmlns:c=\"urn:chart\"", " custom=\"value\""] {
                let xml = format!("<c:{name}{attributes}></c:{name}>");
                assert!(
                    matches!(
                        collapse_empty_pairs(xml.as_bytes()),
                        std::borrow::Cow::Borrowed(_)
                    ),
                    "owner was collapsed: {xml}"
                );
            }
        }
    }

    #[test]
    fn default_leaves_and_contextual_scalar_names_still_collapse() {
        let xml = b"<c:lineChart><c:marker></c:marker><c:varyColors></c:varyColors><c:ser><c:marker></c:marker><c:val></c:val></c:ser></c:lineChart><c:errBars><c:val val=\"2\"></c:val></c:errBars>";
        let out = collapse_empty_pairs(xml);
        assert_eq!(out.len(), xml.len());
        let mut reader = quick_xml::Reader::from_reader(out.as_ref());
        let mut leaves = Vec::new();
        while let Ok(event) = reader.read_event() {
            match event {
                quick_xml::events::Event::Empty(element) => {
                    leaves.push(element.name().as_ref().to_vec())
                }
                quick_xml::events::Event::Eof => break,
                _ => (),
            }
        }
        assert_eq!(
            leaves,
            [
                b"c:marker".to_vec(),
                b"c:varyColors".to_vec(),
                b"c:val".to_vec()
            ]
        );
        assert!(
            std::str::from_utf8(&out)
                .unwrap()
                .contains("<c:ser><c:marker></c:marker><c:val></c:val></c:ser>")
        );
    }

    #[test]
    fn a_start_tag_followed_by_its_end_tag_becomes_self_closing_at_the_same_length() {
        let xml = b"<a><c:x val=\"1>2\"></c:x><y></y ><y>t</y><b></c></b></a>";
        let out = collapse_empty_pairs(xml);
        assert_eq!(out.len(), xml.len());
        assert_eq!(
            std::str::from_utf8(&out).unwrap(),
            "<a><c:x val=\"1>2\"     /><y    /><y>t</y><b></c></b></a>"
        );
    }

    #[test]
    fn comments_cdata_and_instructions_are_not_rewritten() {
        let xml = b"<?p <a></a>?><!-- <a></a> --><t><![CDATA[<a></a>]]></t><e/>";
        assert!(matches!(
            collapse_empty_pairs(xml),
            std::borrow::Cow::Borrowed(_)
        ));
    }

    #[test]
    fn a_longer_name_sharing_the_prefix_is_not_a_match() {
        let xml = b"<x></xy>";
        assert!(matches!(
            collapse_empty_pairs(xml),
            std::borrow::Cow::Borrowed(_)
        ));
    }

    #[test]
    fn extension_payloads_stay_verbatim_until_their_matching_end() {
        let xml = b"<a:extLst><!-- </a:extLst> --><a:ext><a:extLst><x></x></a:extLst><y></y></a:ext></a:extLst><x></x>";
        let out = collapse_empty_pairs(xml);
        let extension_end = xml.len() - b"<x></x>".len();
        assert_eq!(&out[..extension_end], &xml[..extension_end]);
        assert_eq!(&out[extension_end..], b"<x   />");
        assert_eq!(out.len(), xml.len());
    }

    #[test]
    fn string_cache_first_pass_keeps_the_deferred_replays_original_xml() {
        for cache in [
            b"<c:strCache/>".as_slice(),
            b"<c:strCache></c:strCache>",
            b"<c:strCache><c:ptCount val=\"1\"></c:ptCount><!-- </c:strCache> --><c:pt idx=\"0\"><c:v></c:v></c:pt></c:strCache>",
        ] {
            let xml = [cache, b"<x></x>"].concat();
            let out = collapse_empty_pairs(&xml);
            assert_eq!(&out[..cache.len()], cache);
            assert_eq!(&out[cache.len()..], b"<x   />");
            assert_eq!(out.len(), xml.len());
        }
    }

    #[test]
    fn text_whitespace_and_comments_between_tags_are_preserved() {
        for xml in [
            b"<x> </x>".as_slice(),
            b"<x><!--comment--></x>",
            b"<x>text</x>",
            b"<x><b/></x>",
            b"<a:extLst><x></x></a:extLst>",
            b"<x></x\x0b>",
            b"<x></x\x0c>",
        ] {
            assert!(matches!(
                collapse_empty_pairs(xml),
                std::borrow::Cow::Borrowed(_)
            ));
        }
    }
}
