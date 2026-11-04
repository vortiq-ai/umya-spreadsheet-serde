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
    #[cfg(test)]
    pub(crate) fn format(&mut self, value: &str, code: &str) -> String {
        self.format_result(value, code).cell_text()
    }

    pub(crate) fn format_chart(&mut self, value: &str, code: &str) -> String {
        self.format_result(value, code).chart_text()
    }

    fn format_result(&mut self, value: &str, code: &str) -> FormattedValue {
        format_with(value, code, |parsed| {
            if let Some(prepared) = self.formats.get(code) {
                return prepared.apply_result(parsed);
            }
            self.formats
                .entry(code.to_string())
                .or_insert_with(|| PreparedFormat::new(code))
                .apply_result(parsed)
        })
    }
}

pub fn to_formatted_string<S: AsRef<str>, P: AsRef<str>>(value: S, format: P) -> String {
    format_with(value.as_ref(), format.as_ref(), |parsed| {
        PreparedFormat::new(format.as_ref()).apply_result(parsed)
    })
    .cell_text()
}

pub(crate) fn to_formatted_chart_string(value: &str, format: &str) -> String {
    format_with(value, format, |parsed| {
        PreparedFormat::new(format).apply_result(parsed)
    })
    .chart_text()
}

pub(crate) enum FormattedValue {
    Text(String),
    DateOverflow,
}

impl FormattedValue {
    fn cell_text(self) -> String {
        match self {
            Self::Text(text) => text,
            Self::DateOverflow => "############################################".to_string(),
        }
    }

    fn chart_text(self) -> String {
        match self {
            Self::Text(text) => text,
            Self::DateOverflow => "#######".to_string(),
        }
    }
}

fn format_with(
    value: &str,
    code: &str,
    prepare_and_apply: impl FnOnce(f64) -> FormattedValue,
) -> FormattedValue {
    if value.is_empty() {
        return FormattedValue::Text(value.to_string());
    }
    let parsed = match value.parse::<f64>() {
        Ok(value) if code == NumberingFormat::FORMAT_GENERAL => {
            return FormattedValue::Text(value.to_string());
        }
        Ok(_) if code == NumberingFormat::FORMAT_TEXT => {
            return FormattedValue::Text(value.to_string());
        }
        Err(_) => return FormattedValue::Text(value.to_string()),
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

    #[cfg(test)]
    fn apply(&self, value: f64) -> String {
        self.apply_result(value).cell_text()
    }

    fn apply_result(&self, value: f64) -> FormattedValue {
        if !value.is_finite() {
            return FormattedValue::Text(value.to_string());
        }
        let Some((raw, _)) = select_section(&self.raw, value) else {
            return FormattedValue::Text(value.to_string());
        };
        if let Some(literal) = literal_only_section(&raw.display) {
            return FormattedValue::Text(literal);
        }
        let Some(rewritten) = self.rewritten.as_ref().ok() else {
            return FormattedValue::Text(value.to_string());
        };
        let Some((section, absolute)) = select_section(rewritten, value) else {
            return FormattedValue::Text(value.to_string());
        };
        let section = &section.display;
        let formatted = match section.kind {
            FormatKind::Date => return date_formater::format_as_date(absolute, &section.spaced),
            FormatKind::Quoted | FormatKind::Number => {
                number_formater::format_as_number(absolute, &section.spaced)
            }
            FormatKind::Percent => {
                percentage_formater::format_as_percentage(absolute, &section.spaced)
            }
        };
        FormattedValue::Text(formatted.trim().to_string())
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
    } else if spaced.starts_with('"')
        && spaced.ends_with('"')
        && !spaced.trim_matches('"').contains('"')
    {
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
) -> Option<(&PreparedSection<T>, f64)> {
    let Ok(sections) = stage else { return None };
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
    sections.get(index).map(|section| (section, absolute))
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
#[path = "../../tests/support/number_format_expectations.rs"]
mod number_format_expectations;

#[cfg(test)]
mod prepared_tests {
    use super::*;

    // Every row must give exactly the answer of the one-shot formatter after
    // earlier codes and edge values have used the same map. The integration
    // test `number_formats_match_known_answers` pins those answers exactly.
    #[test]
    fn shared_formats_match_the_one_shot_formatter_for_every_known_row() {
        let rows = number_format_expectations::validated_rows();
        let mut formats = PreparedFormats::default();
        for (index, row) in rows.iter().enumerate() {
            let code = row["format"].as_str().unwrap();
            let value = row["value"].as_str().unwrap();
            assert_eq!(
                formats.format(value, code),
                to_formatted_string(value, code),
                "row {index}: format={code:?}, value={value:?}"
            );
        }
    }

    // The escape VM can exhaust its default backtrack budget on a large mask.
    // Selected literal sections still render, and unsupported value sections
    // return the numeric input without panicking.
    #[test]
    fn escape_failure_and_excess_sections_do_not_panic() {
        let literal = "x".repeat(2_000_000);
        let code = format!("0;\"{literal}\"");
        let prepared = PreparedFormat::new(&code);
        assert_eq!(prepared.apply(-1.0), literal);
        assert_eq!(prepared.apply(1.0), "1");
        let raw_code = format!("0;0;0;0;0;[Red]0{literal}");
        let raw_prepared = PreparedFormat::new(&raw_code);
        assert_eq!(raw_prepared.apply(1.0), "1");
        let mut formats = PreparedFormats::default();
        for (value, code, expected) in [
            ("-1", code.as_str(), literal.as_str()),
            ("1", code.as_str(), "1"),
            ("1", raw_code.as_str(), "1"),
        ] {
            assert_eq!(to_formatted_string(value, code), expected);
            let mut cell = crate::Cell::default();
            cell.set_value_number(value.parse::<f64>().unwrap());
            cell.style_mut().number_format_mut().set_format_code(code);
            assert_eq!(cell.formatted_value(), expected);
            assert_eq!(formats.format(value, code), expected);
        }
    }

    // Reusing one date entry across ordinary, out-of-range and non-finite
    // inputs must preserve its output and prepared map.
    #[test]
    fn reused_date_format_handles_out_of_range_inputs() {
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
            let expected = match value {
                "-1" | "-1234.5678" | "1e21" => "############################################",
                "NaN" | "inf" | "-inf" => value,
                _ => row["output"].as_str().unwrap(),
            };
            let observed = formats.format(value, "yyyy-mm-dd");
            assert_eq!(observed, expected, "oracle row {index}");
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

    // A mask with more sections than the formatter supports stays
    // non-panicking, including when its first section is literal-only.
    #[test]
    fn raw_section_overflow_returns_the_numeric_value() {
        for code in [
            r#""literal";0;0;0;0;[Red]0"#,
            "0;0;0;0;0;[<0]0",
            "0;0;0;0;0;[Red]0",
        ] {
            let prepared = PreparedFormat::new(code);
            assert_eq!(prepared.apply(1.0), "1");
        }
    }

    // _ followed by a quote removes that quote during spacing conversion.
    // The converted display remains numeric formatting with a literal prefix.
    #[test]
    fn quoted_numeric_display_does_not_parse_its_literal_as_a_number() {
        let code = r#""a_"0";"other""#;
        let prepared = PreparedFormat::new(code);
        assert_eq!(prepared.apply(-1.0), "other");
        assert_eq!(prepared.apply(1.0), "a 1");
    }

    // An escaped separator in an overlong section list follows the same safe
    // unsupported-format path.
    #[test]
    fn escaped_section_overflow_does_not_panic() {
        let prepared = PreparedFormat::new(r"0;0;0;0;0\(;[Red]0");
        assert_eq!(prepared.apply(1.0), "1");
        assert_eq!(to_formatted_string("1", r#""literal";0"#), "literal");
    }

    #[test]
    fn quoted_composite_formats_share_preparation_without_changing_literal_text() {
        let cases = [
            ("5", r#""Qty "0" pcs""#, "Qty 5 pcs"),
            ("7", r#""Qty "0" pcs""#, "Qty 7 pcs"),
            (
                "1234.5",
                r##""Total: "#,##0.00" EUR""##,
                "Total: 1,234.50 EUR",
            ),
        ];
        for (value, code, expected) in cases {
            assert_eq!(to_formatted_string(value, code), expected);
        }
        let before = preparation_count();
        let mut formats = PreparedFormats::default();
        for (value, code, expected) in cases {
            assert_eq!(formats.format(value, code), expected);
        }
        assert_eq!(formats.formats.len(), 2);
        assert_eq!(preparation_count() - before, 2);
    }
}

#[test]
fn test_to_formatted_string_quoted_text_around_a_placeholder() {
    // Starts and ends with a quote but is not a quoted number: this used to
    // panic on `parse::<f64>().unwrap()`.
    assert_eq!("Qty 5 pcs", to_formatted_string("5", r#""Qty "0" pcs""#));
    assert_eq!(
        "Total: 1,234.50 EUR",
        to_formatted_string("1234.5", r##""Total: "#,##0.00" EUR""##)
    );
}
