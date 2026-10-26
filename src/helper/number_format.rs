mod date_formater;
mod fraction_formater;
mod number_formater;
mod percentage_formater;

use std::borrow::Cow;

use fancy_regex::{
    Matches,
    Regex,
};

use crate::{
    helper::utils::compile_regex,
    structs::NumberingFormat,
};

pub struct Split<'r, 't> {
    finder: Matches<'r, 't>,
    last:   usize,
}

#[inline]
#[must_use]
pub fn split<'r, 't>(regex: &'r Regex, text: &'t str) -> Split<'r, 't> {
    Split {
        finder: regex.find_iter(text),
        last:   0,
    }
}

impl<'t> Iterator for Split<'_, 't> {
    type Item = &'t str;

    fn next(&mut self) -> Option<Self::Item> {
        let text = self.finder.text();
        match self.finder.next() {
            Some(Ok(m)) => {
                let matched = &text[self.last..m.start()];
                self.last = m.end();
                Some(matched)
            }
            _ => {
                if self.last > text.len() {
                    None
                } else {
                    let s = &text[self.last..];
                    self.last = text.len() + 1;
                    Some(s)
                }
            }
        }
    }
}

pub fn get_escape_regex() -> &'static Regex {
    compile_regex!(r#"(\\\(((.)(?!((AM\/PM)|(A\/P)))|([^ ])))(?=(?:[^"]|"[^"]*")*$)"#)
}

pub fn get_section_regex() -> &'static Regex {
    compile_regex!(r#"(;)(?=(?:[^"]|"[^"]*")*$)"#)
}

pub fn get_date_time_regex() -> &'static Regex {
    compile_regex!(r#"(\[\$[A-Z]*-[0-9A-F]*\])*[hmsdy](?=(?:[^"]|"[^"]*")*$)"#)
}

pub fn get_percent_dollar_regex() -> &'static Regex {
    compile_regex!("%$")
}

/// A read owns this map; codes are inserted only after the early returns.
#[derive(Default)]
pub(crate) struct PreparedFormats {
    formats: std::collections::HashMap<String, PreparedFormat>,
}

impl PreparedFormats {
    pub(crate) fn format(&mut self, value: &str, code: &str) -> String {
        format_with(value, code, |parsed| {
            if let Some(prepared) = self.formats.get(code) {
                return prepared.apply(parsed);
            }
            self.formats
                .insert(code.to_string(), PreparedFormat::new(code));
            self.formats.get(code).unwrap().apply(parsed)
        })
    }
}

pub fn to_formatted_string<S: AsRef<str>, P: AsRef<str>>(value: S, format: P) -> String {
    format_with(value.as_ref(), format.as_ref(), |parsed| {
        PreparedFormat::new(format.as_ref()).apply(parsed)
    })
}

fn format_with(value: &str, code: &str, prepare_and_apply: impl FnOnce(f64) -> String) -> String {
    if value.is_empty() {
        return value.to_string();
    }
    let parsed = match value.parse::<f64>() {
        Ok(value) if code == NumberingFormat::FORMAT_GENERAL => return value.to_string(),
        Ok(_) if code == NumberingFormat::FORMAT_TEXT => return value.to_string(),
        Err(_) => return value.to_string(),
        Ok(value) => value,
    };
    prepare_and_apply(parsed)
}

#[derive(Clone, Copy)]
enum FormatKind {
    Date,
    Quoted,
    Percent,
    Number,
}

struct PreparedSection<T> {
    display:   T,
    condition: String,
    threshold: String,
}

struct PreparedDisplay {
    spaced: String,
    kind:   FormatKind,
}

/// Each split has its own delayed failure. Preparing a later split cannot
/// bypass the selected raw section's literal return.
struct PreparedFormat {
    raw:       Result<Vec<PreparedSection<String>>, usize>,
    rewritten: Result<Result<Vec<PreparedSection<PreparedDisplay>>, usize>, fancy_regex::Error>,
}

#[cfg(test)]
thread_local! {
    static PREPARATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

impl PreparedFormat {
    fn new(code: &str) -> Self {
        #[cfg(test)]
        PREPARATIONS.with(|count| count.set(count.get() + 1));
        let raw = prepare_sections(code, |converted| converted);
        let rewritten = get_escape_regex()
            .try_replacen(code, 0, r#""$0""#)
            .map(|text| prepare_sections(&text, prepare_display));
        Self { raw, rewritten }
    }

    fn apply(&self, value: f64) -> String {
        let (raw, _) = select_section(&self.raw, value);
        if let Some(literal) = literal_only_section(&raw.display) {
            return literal;
        }
        let rewritten = self.rewritten.as_ref().unwrap();
        let (section, absolute) = select_section(rewritten, value);
        let section = &section.display;
        // Keep the absolute value's round trip and the quoted parse here.
        let reparsed = absolute.to_string().parse::<f64>().unwrap_or(0.0);
        let formatted = match section.kind {
            FormatKind::Date => date_formater::format_as_date(reparsed, &section.spaced),
            FormatKind::Quoted => Cow::Owned(
                section
                    .spaced
                    .trim_matches('"')
                    .parse::<f64>()
                    .unwrap()
                    .to_string(),
            ),
            FormatKind::Percent => {
                percentage_formater::format_as_percentage(reparsed, &section.spaced)
            }
            FormatKind::Number => number_formater::format_as_number(reparsed, &section.spaced),
        };
        formatted.trim().to_string()
    }
}

fn prepare_sections<T>(
    code: &str,
    display: impl Fn(String) -> T,
) -> Result<Vec<PreparedSection<T>>, usize> {
    let color_re = compile_regex!(r"\[(Black|White|Red|Green|Blue|Yellow|Magenta|Cyan)\]");
    let cond_re = compile_regex!(r"\[(>|>=|<|<=|=|<>)([+-]?\d+([.]\d+)?)\]");
    let mut result = Vec::new();
    for (index, section) in split(get_section_regex(), code).enumerate() {
        let mut converted = section.to_string();
        if color_re.captures(section).ok().flatten().is_some() {
            if index >= 5 {
                return Err(index);
            }
            converted = color_re.replace_all(section, "").to_string();
        }
        let mut condition = String::new();
        let mut threshold = String::from("0");
        if let Some(captures) = cond_re.captures(section).ok().flatten() {
            if index >= 5 {
                return Err(index);
            }
            condition = captures.get(1).unwrap().as_str().to_string();
            threshold = captures.get(2).unwrap().as_str().to_string();
            // Intentionally use the original section: this can keep color
            // syntax.
            converted = cond_re.replace_all(section, "").to_string();
        }
        result.push(PreparedSection {
            display: display(converted),
            condition,
            threshold,
        });
    }
    Ok(result)
}

fn prepare_display(converted: String) -> PreparedDisplay {
    let spacing_re = compile_regex!("_.");
    let spaced = match spacing_re.replace_all(&converted, " ") {
        Cow::Borrowed(_) => converted,
        Cow::Owned(spaced) => spaced,
    };
    let kind = if get_date_time_regex().is_match(&spaced).unwrap_or(false) {
        FormatKind::Date
    } else if spaced.starts_with('"') && spaced.ends_with('"') {
        FormatKind::Quoted
    } else if get_percent_dollar_regex()
        .is_match(&spaced)
        .unwrap_or(false)
    {
        FormatKind::Percent
    } else {
        FormatKind::Number
    };
    PreparedDisplay { spaced, kind }
}

fn select_section<T>(
    stage: &Result<Vec<PreparedSection<T>>, usize>,
    value: f64,
) -> (&PreparedSection<T>, f64) {
    let sections = match stage {
        Ok(sections) => sections,
        Err(index) => panic!("index out of bounds: the len is 5 but the index is {index}"),
    };
    let compare = |index: usize, default| {
        let section = &sections[index];
        split_format_compare(
            value,
            &section.condition,
            section.threshold.parse::<f64>().unwrap_or(0.0),
            default,
            0.0,
        )
    };
    let index = match sections.len() {
        2 if !compare(0, ">=") => 1,
        3 | 4 if !compare(0, ">") => {
            if compare(1, "<") {
                1
            } else {
                2
            }
        }
        _ => 0,
    };
    let absolute = if matches!(sections.len(), 2..=4) {
        value.abs()
    } else {
        value
    };
    (&sections[index], absolute)
}

/// Decode a section that contains literals but no value placeholder.
///
/// Excel uses these sections for displays such as a dash in place of zero:
/// `#,##0;[Red](#,##0);\-\ \ `. Quoted text and backslash escapes are
/// literal. `_x` becomes one space, while `*x` contributes one fill character
/// because this string formatter has no cell width. Value-dependent formats
/// stay on the existing date/number paths.
fn literal_only_section(format: &str) -> Option<String> {
    if get_date_time_regex().is_match(format).unwrap_or(false) {
        return None;
    }

    let mut result = String::with_capacity(format.len());
    let mut chars = format.chars();
    let mut in_quotes = false;

    while let Some(ch) = chars.next() {
        if in_quotes {
            if ch == '"' {
                in_quotes = false;
            } else {
                result.push(ch);
            }
            continue;
        }

        match ch {
            '"' => in_quotes = true,
            '\\' | '*' => result.push(chars.next()?),
            '_' => {
                chars.next()?;
                result.push(' ');
            }
            '[' => {
                for control in chars.by_ref() {
                    if control == ']' {
                        break;
                    }
                }
            }
            '0' | '#' | '?' | '@' | '%' => return None,
            _ => result.push(ch),
        }
    }

    (!in_quotes).then_some(result)
}

#[allow(clippy::float_cmp)]
fn split_format_compare(value: f64, cond: &str, val: f64, dfcond: &str, dfval: f64) -> bool {
    let mut check_cond = cond;
    let mut check_val = val;
    if cond.is_empty() {
        check_cond = dfcond;
        check_val = dfval;
    }
    match check_cond {
        ">" => return value > check_val,
        "<" => return value < check_val,
        "<=" => return value <= check_val,
        "<>" => return value != check_val,
        "=" => return value == check_val,
        _ => {}
    }
    value >= check_val
}

#[test]
fn test_to_formatted_string_date() {
    let value = String::from("45435"); // 2024/5/23
    assert_eq!(
        r"2024-05-23",
        to_formatted_string(&value, NumberingFormat::FORMAT_DATE_YYYYMMDD2)
    );
    assert_eq!(
        r"2024-05-23",
        to_formatted_string(&value, NumberingFormat::FORMAT_DATE_YYYYMMDD)
    );
    assert_eq!(
        r"23-05-2024",
        to_formatted_string(&value, NumberingFormat::FORMAT_DATE_DDMMYYYY)
    );
    assert_eq!(
        r"23/05/2024",
        to_formatted_string(&value, NumberingFormat::FORMAT_DATE_DDMMYYYYSLASH)
    );
    assert_eq!(
        r"23/5/24",
        to_formatted_string(&value, NumberingFormat::FORMAT_DATE_DMYSLASH)
    );
    assert_eq!(
        r"23-5-24",
        to_formatted_string(&value, NumberingFormat::FORMAT_DATE_DMYMINUS)
    );
    assert_eq!(
        r"23-5",
        to_formatted_string(&value, NumberingFormat::FORMAT_DATE_DMMINUS)
    );
    assert_eq!(
        r"5-24",
        to_formatted_string(&value, NumberingFormat::FORMAT_DATE_MYMINUS)
    );
    assert_eq!(
        r"05-23-24",
        to_formatted_string(&value, NumberingFormat::FORMAT_DATE_XLSX14)
    );
    assert_eq!(
        r"23-May-24",
        to_formatted_string(&value, NumberingFormat::FORMAT_DATE_XLSX15)
    );
    assert_eq!(
        r"23-May",
        to_formatted_string(&value, NumberingFormat::FORMAT_DATE_XLSX16)
    );
    assert_eq!(
        r"May-24",
        to_formatted_string(&value, NumberingFormat::FORMAT_DATE_XLSX17)
    );
    assert_eq!(
        r"5/23/24 0:00",
        to_formatted_string(&value, NumberingFormat::FORMAT_DATE_XLSX22)
    );
    assert_eq!(
        r"23/5/24 0:00",
        to_formatted_string(&value, NumberingFormat::FORMAT_DATE_DATETIME)
    );
    assert_eq!(
        r"12:00 am",
        to_formatted_string(&value, NumberingFormat::FORMAT_DATE_TIME1)
    );
    assert_eq!(
        r"12:00:00 am",
        to_formatted_string(&value, NumberingFormat::FORMAT_DATE_TIME2)
    );
    assert_eq!(
        r"0:00",
        to_formatted_string(&value, NumberingFormat::FORMAT_DATE_TIME3)
    );
    assert_eq!(
        r"0:00:00",
        to_formatted_string(&value, NumberingFormat::FORMAT_DATE_TIME4)
    );
    assert_eq!(
        r"00:00",
        to_formatted_string(&value, NumberingFormat::FORMAT_DATE_TIME5)
    );
    assert_eq!(
        r"0:00:00",
        to_formatted_string(&value, NumberingFormat::FORMAT_DATE_TIME6)
    );
    assert_eq!(
        r"0:00:00",
        to_formatted_string(&value, NumberingFormat::FORMAT_DATE_TIME8)
    );
    assert_eq!(
        r"2024/05/23",
        to_formatted_string(&value, NumberingFormat::FORMAT_DATE_YYYYMMDDSLASH)
    );

    let value = String::from("44349.211134259262"); // 2021/06/02 05:04:02
    assert_eq!(
        r"2021-06-02",
        to_formatted_string(&value, NumberingFormat::FORMAT_DATE_YYYYMMDD2)
    );
    assert_eq!(
        r"2021-06-02",
        to_formatted_string(&value, NumberingFormat::FORMAT_DATE_YYYYMMDD)
    );
    assert_eq!(
        r"02-06-2021",
        to_formatted_string(&value, NumberingFormat::FORMAT_DATE_DDMMYYYY)
    );
    assert_eq!(
        r"02/06/2021",
        to_formatted_string(&value, NumberingFormat::FORMAT_DATE_DDMMYYYYSLASH)
    );
    assert_eq!(
        r"2/6/21",
        to_formatted_string(&value, NumberingFormat::FORMAT_DATE_DMYSLASH)
    );
    assert_eq!(
        r"2-6-21",
        to_formatted_string(&value, NumberingFormat::FORMAT_DATE_DMYMINUS)
    );
    assert_eq!(
        r"2-6",
        to_formatted_string(&value, NumberingFormat::FORMAT_DATE_DMMINUS)
    );
    assert_eq!(
        r"6-21",
        to_formatted_string(&value, NumberingFormat::FORMAT_DATE_MYMINUS)
    );
    assert_eq!(
        r"06-02-21",
        to_formatted_string(&value, NumberingFormat::FORMAT_DATE_XLSX14)
    );
    assert_eq!(
        r"2-Jun-21",
        to_formatted_string(&value, NumberingFormat::FORMAT_DATE_XLSX15)
    );
    assert_eq!(
        r"2-Jun",
        to_formatted_string(&value, NumberingFormat::FORMAT_DATE_XLSX16)
    );
    assert_eq!(
        r"Jun-21",
        to_formatted_string(&value, NumberingFormat::FORMAT_DATE_XLSX17)
    );
    assert_eq!(
        r"6/2/21 5:04",
        to_formatted_string(&value, NumberingFormat::FORMAT_DATE_XLSX22)
    );
    assert_eq!(
        r"2/6/21 5:04",
        to_formatted_string(&value, NumberingFormat::FORMAT_DATE_DATETIME)
    );
    assert_eq!(
        r"5:04 am",
        to_formatted_string(&value, NumberingFormat::FORMAT_DATE_TIME1)
    );
    assert_eq!(
        r"5:04:02 am",
        to_formatted_string(&value, NumberingFormat::FORMAT_DATE_TIME2)
    );
    assert_eq!(
        r"5:04",
        to_formatted_string(&value, NumberingFormat::FORMAT_DATE_TIME3)
    );
    assert_eq!(
        r"5:04:02",
        to_formatted_string(&value, NumberingFormat::FORMAT_DATE_TIME4)
    );
    assert_eq!(
        r"04:02",
        to_formatted_string(&value, NumberingFormat::FORMAT_DATE_TIME5)
    );
    assert_eq!(
        r"5:04:02",
        to_formatted_string(&value, NumberingFormat::FORMAT_DATE_TIME6)
    );
    assert_eq!(
        r"5:04:02",
        to_formatted_string(&value, NumberingFormat::FORMAT_DATE_TIME8)
    );
    assert_eq!(
        r"2021/06/02",
        to_formatted_string(&value, NumberingFormat::FORMAT_DATE_YYYYMMDDSLASH)
    );
    assert_eq!(r"2", to_formatted_string(&value, "d"));
}

#[test]
fn test_to_formatted_string_zero_section_with_only_escaped_literals() {
    let format = r"#,##0_);[Red]\(#,##0\);\-\ \ ";

    assert_eq!("-  ", to_formatted_string("0", format));
    assert_eq!(
        "FALSE",
        to_formatted_string("0", r#""TRUE";"TRUE";"FALSE""#)
    );
    assert_eq!("", to_formatted_string("0", "0;-0;"));
}

#[cfg(test)]
pub(crate) fn preparation_count() -> usize {
    PREPARATIONS.with(std::cell::Cell::get)
}

#[cfg(test)]
mod prepared_tests {
    use super::*;

    fn outcome(result: std::thread::Result<String>) -> String {
        result.unwrap_or_else(|payload| {
            let message = payload
                .downcast_ref::<String>()
                .map(String::as_str)
                .or_else(|| payload.downcast_ref::<&str>().copied())
                .unwrap();
            format!("PANIC: {message}")
        })
    }

    // Every captured row must agree after earlier codes and panics have used
    // the same map; an uncached-only replay misses damaged prepared entries.
    #[test]
    fn shared_formats_preserve_every_known_answer_in_fixture_order() {
        let rows: Vec<serde_json::Value> = serde_json::from_str(include_str!(
            "../../tests/fixtures/number_format_known_answers.json"
        ))
        .unwrap();
        let mut formats = PreparedFormats::default();
        for (index, row) in rows.iter().enumerate() {
            let code = row["format"].as_str().unwrap();
            let value = row["value"].as_str().unwrap();
            let observed = outcome(std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                || formats.format(value, code),
            )));
            assert_eq!(
                observed,
                row["output"].as_str().unwrap(),
                "oracle row {index}: format={code:?}, value={value:?}"
            );
        }
    }

    // The escape VM exhausts its default 1,000,000 backtracks searching
    // 2,000,000 ASCII positions (fancy-regex 0.18 compile.rs:999, vm.rs:1062).
    // Preparation must survive; a literal return bypasses that saved error.
    #[test]
    fn escape_failure_waits_for_raw_checks_and_literal_return() {
        let literal = "x".repeat(2_000_000);
        let code = format!("0;\"{literal}\"");
        let prepared = PreparedFormat::new(&code);
        assert_eq!(prepared.apply(-1.0), literal);
        let expected = "PANIC: called `Result::unwrap()` on an `Err` value: \
                        RuntimeError(BacktrackLimitExceeded)";
        // The previous formatter called replace_all here, which delegates to
        // try_replacen(...).unwrap() (fancy-regex lib.rs:1280,1295).
        assert_eq!(
            outcome(std::panic::catch_unwind(|| {
                get_escape_regex()
                    .replace_all(&code, r#""$0""#)
                    .into_owned()
            })),
            expected
        );
        assert_eq!(
            outcome(std::panic::catch_unwind(|| prepared.apply(1.0))),
            expected
        );
        let raw_code = format!("0;0;0;0;0;[Red]0{literal}");
        let raw_prepared = PreparedFormat::new(&raw_code);
        let bounds = "PANIC: index out of bounds: the len is 5 but the index is 5";
        assert_eq!(
            outcome(std::panic::catch_unwind(|| raw_prepared.apply(1.0))),
            bounds
        );
        let mut formats = PreparedFormats::default();
        for (value, code, expected) in [
            ("-1", code.as_str(), literal.as_str()),
            ("1", code.as_str(), expected),
            ("1", raw_code.as_str(), bounds),
        ] {
            assert_eq!(
                outcome(std::panic::catch_unwind(|| to_formatted_string(
                    value, code
                ))),
                expected
            );
            let mut cell = crate::Cell::default();
            cell.set_value_number(value.parse::<f64>().unwrap());
            cell.style_mut().number_format_mut().set_format_code(code);
            assert_eq!(
                outcome(std::panic::catch_unwind(|| cell.formatted_value())),
                expected
            );
            assert_eq!(
                outcome(std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                    || { formats.format(value, code) }
                ))),
                expected
            );
        }
    }

    // Reusing one date entry must survive a panic without corrupting later
    // values. These are the oracle's 16 lexical inputs, in capture-input order.
    // Rows 10891 -> 10892 -> 10895 put a panic between two numeric successes.
    #[test]
    fn reused_date_format_preserves_known_answers_before_and_after_panics() {
        let rows: Vec<serde_json::Value> = serde_json::from_str(include_str!(
            "../../tests/fixtures/number_format_known_answers.json"
        ))
        .unwrap();
        let inputs = [
            ("0", 10887),
            ("-0", 10883),
            ("1", 10890),
            ("-1", 10884),
            ("0.5", 10888),
            ("-1234.5678", 10885),
            ("1e-7", 10891),
            ("1e21", 10892),
            ("45000.75", 10895),
            ("NaN", 10897),
            ("inf", 10898),
            ("-inf", 10886),
            ("", 10882),
            ("not a number", 10899),
            ("007", 10889),
            ("1e3", 10893),
        ];
        let before = preparation_count();
        let mut formats = PreparedFormats::default();
        for (value, index) in inputs {
            let row = &rows[index];
            assert_eq!(row["format"], "yyyy-mm-dd");
            assert_eq!(row["value"], value);
            let observed = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                formats.format(value, "yyyy-mm-dd")
            })) {
                Ok(output) => output,
                Err(payload) => {
                    let message = payload
                        .downcast_ref::<String>()
                        .map(String::as_str)
                        .or_else(|| payload.downcast_ref::<&str>().copied())
                        .unwrap();
                    format!("PANIC: {message}")
                }
            };
            assert_eq!(
                observed,
                row["output"].as_str().unwrap(),
                "oracle row {index}"
            );
            assert_eq!(formats.formats.len(), 1);
            assert_eq!(preparation_count() - before, 1);
        }
    }

    // Early-return values must bypass preparation, including raw bounds
    // failures; malformed sections must never be inspected for those values.
    #[test]
    fn early_returns_do_not_prepare_number_formats() {
        let before = preparation_count();
        let mut formats = PreparedFormats::default();
        for (value, code, expected) in [
            ("", "0;0;0;0;0;[Red]0", ""),
            ("text", "0;0;0;0;0;[Red]0", "text"),
            ("007", "General", "7"),
            ("007", "@", "007"),
        ] {
            assert_eq!(to_formatted_string(value, code), expected);
            assert_eq!(formats.format(value, code), expected);
        }
        assert!(formats.formats.is_empty());
        assert_eq!(preparation_count(), before);
    }

    // The external oracle's zero-based rows 1225, 5803 and 5819 establish
    // the bounds message, including failure before a literal first section.
    #[test]
    fn raw_section_failure_precedes_literal_selection() {
        for code in [
            r#""literal";0;0;0;0;[Red]0"#,
            "0;0;0;0;0;[<0]0",
            "0;0;0;0;0;[Red]0",
        ] {
            let prepared = PreparedFormat::new(code);
            let panic = std::panic::catch_unwind(|| prepared.apply(1.0)).unwrap_err();
            assert_eq!(
                panic.downcast_ref::<String>().unwrap(),
                "index out of bounds: the len is 5 but the index is 5",
            );
        }
    }

    // _ followed by a quote removes that quote during spacing conversion.
    // Raw "a_"0" contains an unquoted zero, but its converted "a 0" is a
    // quoted non-number. Only selecting this section may run its parse.
    #[test]
    fn quoted_numeric_parse_waits_for_the_selected_converted_section() {
        let code = r#""a_"0";"other""#;
        let prepared = PreparedFormat::new(code);
        assert_eq!(prepared.apply(-1.0), "other");
        let panic = std::panic::catch_unwind(|| prepared.apply(1.0)).unwrap_err();
        assert_eq!(
            panic.downcast_ref::<String>().unwrap(),
            "called `Result::unwrap()` on an `Err` value: ParseFloatError { kind: Invalid }",
        );
    }

    // Escaping \(; shields a raw separator only after the raw bounds check.
    // Combining stage failures must not hide that check behind rewriting.
    #[test]
    fn raw_bounds_failure_survives_an_escaped_separator() {
        let prepared = PreparedFormat::new(r"0;0;0;0;0\(;[Red]0");
        assert_eq!(
            outcome(std::panic::catch_unwind(|| prepared.apply(1.0))),
            "PANIC: index out of bounds: the len is 5 but the index is 5"
        );
        assert_eq!(to_formatted_string("1", r#""literal";0"#), "literal");
    }
}
