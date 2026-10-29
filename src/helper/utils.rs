#![allow(unused_imports)]

use std::fmt::Write as _;

use md5::Digest as _;

pub(crate) fn md5_hash(input: impl AsRef<[u8]>) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";

    let digest = md5::Md5::digest(input);
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest.iter().copied() {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

pub(crate) fn unescape_xml_text(e: &quick_xml::events::BytesText<'_>) -> String {
    let decoded = e.decode().unwrap();
    let mut output = String::with_capacity(decoded.len());
    visit_xml_text_parts(decoded.as_ref(), |part, literal| {
        if literal {
            output.push_str(part);
        } else {
            output.push_str(&quick_xml::escape::unescape(part).unwrap());
        }
    });
    output
}

// read_text_into retains CDATA markup, whose content is literal XML text.
fn visit_xml_text_parts(mut remaining: &str, mut visit: impl FnMut(&str, bool)) {
    while let Some(start) = remaining.find("<![CDATA[") {
        visit(&remaining[..start], false);
        let content = &remaining[start + 9..];
        let end = content.find("]]>").expect("unterminated CDATA section");
        visit(&content[..end], true);
        remaining = &content[end + 3..];
    }
    visit(remaining, false);
}

/// Whether XML 1.0 forbids `c`: the control characters other than tab, line
/// feed and carriage return, and U+FFFE and U+FFFF.
fn is_forbidden_in_xml(c: char) -> bool {
    matches!(c, '\u{0}'..='\u{8}' | '\u{b}' | '\u{c}' | '\u{e}'..='\u{1f}' | '\u{fffe}' | '\u{ffff}')
}

/// The length of the `_xHHHH_` escape at the start of `s`, if there is one.
fn escape_len(s: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    (bytes.len() >= 7
        && bytes[0] == b'_'
        && bytes[1] == b'x'
        && bytes[2..6].iter().all(u8::is_ascii_hexdigit)
        && bytes[6] == b'_')
        .then_some(7)
}

/// Writes `value` as an `ST_Xstring`, the text of a string in `SpreadsheetML`:
/// a character that XML 1.0 forbids becomes `_xHHHH_`, and the underscore of
/// text that already reads as such an escape becomes `_x005F_`, so that the
/// text reads back as it was written.
pub(crate) fn escape_xstring(value: &str) -> std::borrow::Cow<'_, str> {
    if !value.contains(|c: char| c == '_' || is_forbidden_in_xml(c)) {
        return std::borrow::Cow::Borrowed(value);
    }
    let mut output = String::with_capacity(value.len());
    for (i, c) in value.char_indices() {
        if is_forbidden_in_xml(c) {
            let _ = write!(output, "_x{:04X}_", u32::from(c));
        } else if c == '_' && escape_len(&value[i..]).is_some() {
            output.push_str("_x005F_");
        } else {
            output.push(c);
        }
    }
    std::borrow::Cow::Owned(output)
}

/// Reads an `ST_Xstring`, replacing each `_xHHHH_` escape with its character.
pub(crate) fn unescape_xstring(value: &str) -> std::borrow::Cow<'_, str> {
    if !value.contains("_x") {
        return std::borrow::Cow::Borrowed(value);
    }
    let mut output = String::with_capacity(value.len());
    let mut i = 0;
    while i < value.len() {
        let rest = &value[i..];
        if let Some(len) = escape_len(rest) {
            if let Some(c) = u32::from_str_radix(&rest[2..6], 16)
                .ok()
                .and_then(char::from_u32)
            {
                output.push(c);
                i += len;
                continue;
            }
        }
        let c = rest.chars().next().unwrap();
        output.push(c);
        i += c.len_utf8();
    }
    std::borrow::Cow::Owned(output)
}

pub(crate) fn unescape_xstring_owned(value: String) -> String {
    match unescape_xstring(&value) {
        std::borrow::Cow::Borrowed(_) => value,
        std::borrow::Cow::Owned(decoded) => decoded,
    }
}

/// Runs the checks `unescape_xml_text` makes, without building its result.
/// Text whose references all check out here is accepted without allocating;
/// any other text is handed to `unescape`, which raises its own error.
pub(crate) fn validate_xml_text(e: &quick_xml::events::BytesText<'_>) {
    let decoded = e.decode().unwrap();
    validate_decoded_xml_text(decoded.as_ref());
}

fn validate_decoded_xml_text(decoded: &str) {
    visit_xml_text_parts(decoded, |part, literal| {
        if !literal && !references_are_valid(part) {
            quick_xml::escape::unescape(part).unwrap();
        }
    });
}

/// Decodes into reused storage only when the text cannot be borrowed raw.
/// Invalid references go through the original decoder to preserve its errors.
pub(crate) fn unescape_xml_text_into(
    e: &quick_xml::events::BytesText<'_>,
    scratch: &mut String,
) -> bool {
    let decoded = e.decode().unwrap();
    if !decoded.contains('&')
        && !decoded.contains("<![CDATA[")
        && matches!(decoded, std::borrow::Cow::Borrowed(_))
    {
        return false;
    }
    validate_decoded_xml_text(decoded.as_ref());
    scratch.clear();
    visit_xml_text_parts(decoded.as_ref(), |part, literal| {
        if literal {
            scratch.push_str(part);
        } else {
            let mut rest = part;
            while let Some(start) = rest.find('&') {
                scratch.push_str(&rest[..start]);
                let end = start + rest[start..].find(';').unwrap();
                let name = &rest[start + 1..end];
                if let Some(number) = name.strip_prefix('#') {
                    let (digits, radix) = match number.strip_prefix('x') {
                        Some(hex) => (hex, 16),
                        None => (number, 10),
                    };
                    scratch
                        .push(char::from_u32(u32::from_str_radix(digits, radix).unwrap()).unwrap());
                } else {
                    scratch.push_str(quick_xml::escape::resolve_predefined_entity(name).unwrap());
                }
                rest = &rest[end + 1..];
            }
            scratch.push_str(rest);
        }
    });
    true
}

/// Whether `unescape` accepts `raw`: every `&` starts a reference ended by a
/// `;` with no `&` between, naming a predefined entity or a valid character
/// reference.
pub(crate) fn references_are_valid(raw: &str) -> bool {
    let bytes = raw.as_bytes();
    let mut from = 0;
    while let Some(start) = bytes[from..].iter().position(|&byte| byte == b'&') {
        let start = from + start;
        let Some(end) = bytes[start + 1..]
            .iter()
            .position(|&byte| byte == b'&' || byte == b';')
        else {
            return false;
        };
        let end = start + 1 + end;
        if bytes[end] != b';' {
            return false;
        }
        let name = &raw[start + 1..end];
        let valid = match name.strip_prefix('#') {
            Some(number) => character_reference_is_valid(number),
            None => quick_xml::escape::resolve_predefined_entity(name).is_some(),
        };
        if !valid {
            return false;
        }
        from = end + 1;
    }
    true
}

/// The number of a character reference: decimal, or hexadecimal after `x`,
/// unsigned, not zero, and a scalar value.
fn character_reference_is_valid(number: &str) -> bool {
    let (digits, radix) = match number.strip_prefix('x') {
        Some(hex) => (hex, 16),
        None => (number, 10),
    };
    if matches!(digits.as_bytes().first(), Some(b'+' | b'-')) {
        return false;
    }
    u32::from_str_radix(digits, radix).is_ok_and(|code| code != 0 && char::from_u32(code).is_some())
}

/// A macro that implements the `From` trait for converting from one error type
/// to another.
///
/// # Usage
/// ```
/// # use std::{io,fmt};
///
/// use umya_spreadsheet::from_err;
///
/// #[derive(Debug)]
/// enum MyError {
///     Io(std::io::Error),
/// };
/// # impl fmt::Display for MyError {
/// #   fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
/// #       unimplemented!()
/// #   }
/// # }
/// # impl std::error::Error for MyError {}
///
/// from_err!(std::io::Error, MyError, Io);
///
/// let io_err = io::Error::new(io::ErrorKind::Other, "An I/O error occurred");
/// let my_err: MyError = io_err.into();
/// ```
#[macro_export]
macro_rules! from_err {
    ($from:ty, $to:tt, $var:tt) => {
        impl From<$from> for $to {
            #[inline]
            fn from(e: $from) -> $to {
                $to::$var(e)
            }
        }
    };
}

/// Asserts that the SHA-256 hash of a given input matches the expected
/// hexadecimal string.
///
/// # Arguments
///
/// * `$input` - The input data to hash.
/// * `$expected_hex` - The expected SHA-256 hash as a hexadecimal string.
///
/// # Panics
///
/// This macro will panic if the actual SHA-256 hash does not match the
/// expected hash.
///
/// # Examples
///
/// ```ignore
/// let data = b"Hello, world!";
/// assert_sha256!(
///     data,
///     "a591a6d40bf420404a011733cfb7b190d62c65bf0bcda32b57b277d9ad9f146e"
/// );
/// // This will not panic
///
/// assert_sha256!(data, "invalid_hash");
/// // This will panic with a message indicating the mismatch
/// ```
macro_rules! assert_sha256 {
    ($input:expr, $expected_hex:expr) => {{
        let hash = Sha256::digest($input).to_vec();
        let expected_bytes = hex_literal::hex!($expected_hex);
        assert_eq!(
            &hash,
            &expected_bytes,
            "SHA256({}) mismatch! Expected: {:?}, Actual: {:?}",
            stringify!($input),
            &expected_bytes
                .iter()
                .map(|b| format!("{:02x}", b))
                .collect::<String>(),
            &hash
                .iter()
                .map(|b| format!("{:02x}", b))
                .collect::<String>()
        );
    }};
}

/// A macro that compiles a regular expression and caches it.
///
/// # Usage
/// ```rust,ignore
/// // Unable to run because function is private
/// let re = compile_regex!(r"^\d+$");
///
/// assert!(re.is_match("123").unwrap());
/// assert!(!re.is_match("abc").unwrap());
/// ```
macro_rules! compile_regex {
    ($re:literal $(,)?) => {{
        static RE: std::sync::OnceLock<fancy_regex::Regex> = std::sync::OnceLock::new();
        RE.get_or_init(|| fancy_regex::Regex::new($re).unwrap())
    }};
}

/// Prints a byte slice as a hex string, prefixed with the variable name.
///
/// This macro takes a reference to a byte slice (`&[u8]`) and prints both
/// the variable name and its hexadecimal representation to stdout.
macro_rules! print_hex {
    ($var:expr) => {
        println!(
            "{} = {}",
            stringify!($var),
            $var.iter()
                .map(|b| format!("{:02x}", b))
                .collect::<String>()
        );
    };
}

/// Prints the SHA-256 hash of a given input as a hexadecimal string.
///
/// # Examples
///
/// ```ignore
/// let data = b"Hello, world!";
/// print_sha256_hex!(data);
/// // Output: SHA256(data) = 5eb63bbbe01eeed093cb22bb8f5acdc3
/// ```
macro_rules! print_sha256_hex {
    ($var:expr) => {
        let hash = Sha256::digest($var);
        println!(
            "SHA256({}) = {}",
            stringify!($var),
            hash.iter()
                .map(|b| format!("{:02x}", b))
                .collect::<String>()
        );
    };
}

pub(crate) use assert_sha256;
pub(crate) use compile_regex;
pub(crate) use print_hex;
pub(crate) use print_sha256_hex;

#[cfg(test)]
mod tests {
    use quick_xml::events::BytesText;

    use super::{
        unescape_xml_text,
        unescape_xml_text_into,
        validate_xml_text,
    };

    fn failure(operation: impl FnOnce()) -> String {
        let payload =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(operation)).unwrap_err();
        match payload.downcast::<String>() {
            Ok(message) => *message,
            Err(payload) => (*payload.downcast::<&str>().unwrap()).to_owned(),
        }
    }

    #[test]
    fn cdata_helpers_preserve_literal_content_and_decode_surrounding_entities() {
        let mut scratch = String::from("previous value");
        for (raw, expected) in [
            ("<![CDATA[plain]]>", "plain"),
            ("<![CDATA[&bogus;&amp;]]>", "&bogus;&amp;"),
            ("x<![CDATA[<y>&amp;]]>z", "x<y>&amp;z"),
            ("A&amp;<![CDATA[<B>&bogus;]]>&#67;", "A&<B>&bogus;C"),
            ("<![CDATA[a]]>&lt;<![CDATA[b&c]]>", "a<b&c"),
            ("&lt;![CDATA[&amp;]]&gt;", "<![CDATA[&]]>"),
        ] {
            let text = BytesText::from_escaped(raw);
            assert_eq!(unescape_xml_text(&text), expected, "{raw}");
            validate_xml_text(&text);
            assert!(unescape_xml_text_into(&text, &mut scratch), "{raw}");
            assert_eq!(scratch, expected, "{raw}");
        }
    }

    #[test]
    fn cdata_helpers_reject_only_invalid_references_outside_literal_sections() {
        for (raw, invalid) in [
            ("&bogus;<![CDATA[valid]]>", "&bogus;"),
            ("<![CDATA[&AMP;]]>&bogus;", "&bogus;"),
            ("<![CDATA[valid]]>&#0;", "&#0;"),
        ] {
            let expected = format!(
                "called `Result::unwrap()` on an `Err` value: {:?}",
                quick_xml::escape::unescape(invalid).unwrap_err()
            );
            let text = BytesText::from_escaped(raw);
            assert_eq!(
                failure(|| {
                    unescape_xml_text(&text);
                }),
                expected
            );
            assert_eq!(failure(|| validate_xml_text(&text)), expected);
            let mut scratch = String::from("previous value");
            assert_eq!(
                failure(|| {
                    unescape_xml_text_into(&text, &mut scratch);
                }),
                expected
            );
            assert_eq!(scratch, "previous value");
        }
    }

    #[test]
    fn cdata_validation_and_reused_decoding_allocate_nothing() {
        let mut scratch = String::with_capacity(512);
        for (raw, expected) in [
            ("<![CDATA[plain]]>", "plain"),
            ("<![CDATA[&bogus;]]>&amp;", "&bogus;&"),
            ("A&amp;<![CDATA[<B>]]>&#67;", "A&<B>C"),
        ] {
            let text = BytesText::from_escaped(raw);
            let (used_scratch, allocations) = crate::test_alloc::observe(|| {
                validate_xml_text(&text);
                unescape_xml_text_into(&text, &mut scratch)
            });
            assert!(used_scratch);
            assert_eq!(scratch, expected);
            assert_eq!(allocations.total(), 0, "{raw}");
        }
        scratch.clear();
        scratch.push_str("previous value");
        let text = BytesText::from_escaped("plain");
        let (used_scratch, allocations) =
            crate::test_alloc::observe(|| unescape_xml_text_into(&text, &mut scratch));
        assert!(!used_scratch);
        assert_eq!(scratch, "previous value");
        assert_eq!(allocations.total(), 0);
    }
}
