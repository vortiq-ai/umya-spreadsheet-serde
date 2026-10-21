use std::borrow::Cow;

use crate::helper::utils::compile_regex;

#[must_use]
pub fn split_address(address: &str) -> (&str, &str) {
    address
        .rsplit_once('!')
        .map_or(("", address), |(sheet_name, range)| {
            (sheet_name.trim_matches(&['\'', '"'][..]), range)
        })
}

#[must_use]
pub fn join_address(sheet_name: &str, address: &str) -> String {
    if sheet_name.is_empty() {
        return address.to_string();
    }
    format!("{}!{address}", quote_sheet_name(sheet_name))
}

/// Returns a sheet name in formula form, quoting and escaping it when needed.
///
/// The input is a real sheet title; each apostrophe is doubled when quoted.
/// An empty name is represented by `''`.
#[must_use]
pub fn quote_sheet_name(sheet_name: &str) -> String {
    if sheet_name.is_empty() {
        return "''".to_string();
    }
    if sheet_name_needs_quoting(sheet_name) {
        return format!("'{}'", sheet_name.replace('\'', "''"));
    }
    sheet_name.to_string()
}

/// A bare sheet name starts with an ASCII letter or underscore and contains
/// only ASCII letters, digits, underscores or periods. Every period must be
/// followed by an ASCII letter or digit. Cell references and boolean literals
/// need quotes as well.
pub(crate) fn sheet_name_needs_quoting(sheet_name: &str) -> bool {
    let mut chars = sheet_name.chars();
    match chars.next() {
        None => false,
        Some(first) => {
            if !(first.is_ascii_alphabetic() || first == '_') {
                return true;
            }
            sheet_name
                .chars()
                .any(|c| !(c.is_ascii_alphanumeric() || c == '_' || c == '.'))
                || sheet_name.eq_ignore_ascii_case("TRUE")
                || sheet_name.eq_ignore_ascii_case("FALSE")
                || has_unreadable_dot(sheet_name)
                || is_a1_cell_like(sheet_name)
                || is_r1c1_like(sheet_name)
        }
    }
}

fn has_unreadable_dot(name: &str) -> bool {
    name.char_indices().any(|(index, character)| {
        character == '.'
            && !name[index + 1..]
                .chars()
                .next()
                .is_some_and(|next| next.is_ascii_alphanumeric())
    })
}

/// Column letters up to `XFD`, then digits only (`FY2024`, `a1`, `xfd1`).
fn is_a1_cell_like(name: &str) -> bool {
    let letters = name.bytes().take_while(u8::is_ascii_alphabetic).count();
    let digits = &name[letters..];
    (1..=3).contains(&letters)
        && !digits.is_empty()
        && digits.bytes().all(|b| b.is_ascii_digit())
        && (letters < 3 || name[..3].to_ascii_uppercase().as_str() <= "XFD")
}

/// Excel reads `R`, `C`, `RC`, `R<row>...` and `R?C<col>...` as R1C1
/// references whatever follows the number (`R1D`, `C3PO`, `RC1name`), so they
/// need quotes. Same rules as `XlsxWriter`'s `quote_sheetname`.
fn is_r1c1_like(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    if matches!(upper.as_str(), "R" | "C" | "RC") {
        return true;
    }
    let number_in = |rest: &str, max: u32| {
        let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
        rest[..digits]
            .parse::<u32>()
            .is_ok_and(|n| (1..=max).contains(&n))
    };
    if upper
        .strip_prefix('R')
        .is_some_and(|rest| number_in(rest, 1_048_576))
    {
        return true;
    }
    upper
        .strip_prefix('R')
        .unwrap_or(&upper)
        .strip_prefix('C')
        .is_some_and(|rest| number_in(rest, 16_384))
}

/// The real sheet name behind a name in formula-text form, where a literal
/// apostrophe is written `''`.
pub(crate) fn unescape_sheet_name(name: &str) -> Cow<'_, str> {
    if name.contains("''") {
        Cow::Owned(name.replace("''", "'"))
    } else {
        Cow::Borrowed(name)
    }
}

/// Checks if the given input string is a valid address format.
///
/// The address format is defined by the following regular expression:
/// `^([^\:\\\?$$$$\/\*]+\!)?(\$?[A-Z]{1,3}\$?[0-9]+)(\:\$?[A-Z]{1,3}\$?[0-9]+)?
/// $`.
///
/// # Parameters
///
/// - `input`: A string slice that can be converted to a string reference. This
///   is the input string to be checked against the address format.
///
/// # Returns
///
/// Returns `true` if the input string matches the address format, and `false`
/// otherwise.
///
/// # Panics
///
/// This function may panic if the regular expression fails to compile. However,
/// since the regular expression is initialized only once and is hardcoded, this
/// is unlikely to occur unless there is a bug in the regex itself. The panic
/// will occur during the first call to this function if the regex is invalid.
///
/// # Examples
///
/// ```rust
/// # use umya_spreadsheet::helper::address::is_address;
/// let valid_address = "$A1";
/// assert!(is_address(valid_address));
///
/// let invalid_address = "invalid_address";
/// assert!(!is_address(invalid_address));
/// ```
pub fn is_address<S: AsRef<str>>(input: S) -> bool {
    let regex = compile_regex!(
        r"^([^\:\\\?\[\]\/\*]+\!)?(\$?[A-Z]{1,3}\$?[0-9]+)(\:\$?[A-Z]{1,3}\$?[0-9]+)?$"
    );
    regex.is_match(input.as_ref()).unwrap()
}

#[test]
fn split_address_test() {
    assert_eq!(split_address("A1"), ("", "A1"));
    assert_eq!(split_address("A1:B2"), ("", "A1:B2"));
    assert_eq!(split_address("sheet1!A1:B2"), ("sheet1", "A1:B2"));
    assert_eq!(split_address("'she!et1'!A1:B2"), ("she!et1", "A1:B2"));
    assert_eq!(split_address(r#"'she"et1'!A1:B2"#), (r#"she"et1"#, "A1:B2"));
}

#[test]
fn sheet_name_needs_quoting_test() {
    for name in [
        "P&L",
        "FY2024",
        "a1",
        "xfd1",
        "XFD1048576",
        "R",
        "C",
        "r",
        "R1",
        "C12",
        "R1C1",
        "r1c1",
        "RC",
        "R2C",
        "R1D",
        "R1name",
        "C1name",
        "RC1name",
        "C3PO",
        "R1048576x",
        "C16384x",
        "Row1",
        "1Q",
        "a b",
        "Données",
    ] {
        assert!(sheet_name_needs_quoting(name), "{name}");
    }
    for name in [
        "",
        "Sheet1",
        "Data",
        "Revenue_2024",
        "Rate1",
        "Report",
        "R0x",
        "R1048577x",
        "C16385x",
        "XFE1",
        "ABCD1",
        "Cash",
        "A",
        "Q1.x",
    ] {
        assert!(!sheet_name_needs_quoting(name), "{name}");
    }
}

#[test]
fn quote_sheet_name_test() {
    for (name, expected) in [
        ("", "''"),
        ("Sheet1", "Sheet1"),
        ("Data_2", "Data_2"),
        ("_data.v2", "_data.v2"),
        ("Data.v2", "Data.v2"),
        ("Q1.x", "Q1.x"),
        ("T.DIST.2T", "T.DIST.2T"),
        ("Data.", "'Data.'"),
        ("a._b", "'a._b'"),
        ("a..b", "'a..b'"),
        ("TRUE", "'TRUE'"),
        ("true", "'true'"),
        ("TrUe", "'TrUe'"),
        ("FALSE", "'FALSE'"),
        ("false", "'false'"),
        ("FaLsE", "'FaLsE'"),
        ("TRUE1", "TRUE1"),
        ("Truex", "Truex"),
        ("FALSEHOOD", "FALSEHOOD"),
        ("TRUE.x", "TRUE.x"),
        ("FY2024", "'FY2024'"),
        ("Q1", "'Q1'"),
        ("RC0", "'RC0'"),
        ("R1C1", "'R1C1'"),
        ("r1c1", "'r1c1'"),
        ("RC", "'RC'"),
        ("R1D", "'R1D'"),
        ("C3PO", "'C3PO'"),
        ("Plan2024", "Plan2024"),
        ("XFE1", "XFE1"),
        ("R0C0", "R0C0"),
        ("P&L", "'P&L'"),
        ("1Q", "'1Q'"),
        ("Ratio Analysis", "'Ratio Analysis'"),
        ("Bob's", "'Bob''s'"),
        ("Bob''s", "'Bob''''s'"),
        ("'", "''''"),
        ("'Bob'", "'''Bob'''"),
        ("a!b", "'a!b'"),
        ("Donn\u{00e9}es", "'Donn\u{00e9}es'"),
    ] {
        assert_eq!(quote_sheet_name(name), expected, "{name}");
    }
}

#[test]
fn join_address_quotes_sheet_names_and_keeps_unqualified_ranges() {
    assert_eq!(join_address("", "$A$1"), "$A$1");
    assert_eq!(join_address("Sheet1", "$A$1"), "Sheet1!$A$1");
    assert_eq!(join_address("TRUE", "$A$1"), "'TRUE'!$A$1");
    assert_eq!(join_address("Data.", "$A$1"), "'Data.'!$A$1");
    assert_eq!(join_address("Bob's", "$A$1"), "'Bob''s'!$A$1");
}

#[test]
fn formula_adjustment_escapes_an_apostrophe_sheet_once() {
    use crate::helper::formula::{
        adjustment_formula_coordinate,
        parse_to_tokens,
        render,
    };

    let mut tokens = parse_to_tokens("='Bob''s'!A1");
    adjustment_formula_coordinate(&mut tokens, 0, 1);
    assert_eq!(render(&tokens), "'Bob''s'!A2");
}

#[test]
fn is_address_test() {
    assert!(is_address("A1"));
    assert!(is_address("$A1"));
    assert!(is_address("A$1"));
    assert!(is_address("$A$1"));

    assert!(is_address("A1:B2"));
    assert!(is_address("$A1:B2"));
    assert!(is_address("$A$1:B2"));
    assert!(is_address("$A$1:$B2"));
    assert!(is_address("$A$1:$B$2"));

    assert!(is_address("Sheet1!A1"));
    assert!(is_address("Sheet1!$A1"));
    assert!(is_address("Sheet1!A$1"));
    assert!(is_address("Sheet1!A$1"));

    assert!(is_address("Sheet1!A1:B2"));
    assert!(is_address("Sheet1!$A1:B2"));
    assert!(is_address("Sheet1!$A$1:B2"));
    assert!(is_address("Sheet1!$A$1:$B2"));
    assert!(is_address("Sheet1!$A$1:$B$2"));
    assert!(is_address("New Sheet!$H$7:$H$10"));

    assert!(!is_address("(Sheet1!A1:B2)"));
    assert!(!is_address("Sheet1!A1:"));
    assert!(!is_address("Sheet1!A1:B"));
    assert!(!is_address("Sheet1!A:B2"));
    assert!(!is_address("Sheet1"));
}
