#[path = "support/number_format_expectations.rs"]
mod number_format_expectations;

use std::{
    collections::BTreeSet,
    env,
    fs,
    panic,
};

use fancy_regex::Regex;
use serde_json::{
    Value,
    json,
};
use umya_spreadsheet::{
    NumberingFormat,
    helper::number_format::{
        get_date_time_regex,
        get_escape_regex,
        get_section_regex,
        split,
        to_formatted_string,
    },
};

// Keep the lexical inputs intact: especially signed zero, nonfinite values,
// leading zeroes, and exponent notation. No f64-to-string conversion here.
const VALUES: &[&str] = &[
    "0",
    "-0",
    "1",
    "-1",
    "0.5",
    "-1234.5678",
    "1e-7",
    "1e21",
    "45000.75",
    "NaN",
    "inf",
    "-inf",
    "",
    "not a number",
    "007",
    "1e3",
];

type Inputs = BTreeSet<(String, String)>;

fn add_values(inputs: &mut Inputs, format: &str, values: &[&str]) {
    for value in values {
        inputs.insert((format.to_owned(), (*value).to_owned()));
    }
}

fn capture_inputs(corpus_codes: &Value) -> Inputs {
    let mut inputs = Inputs::new();
    // All calls in the existing date and literal-section tests, including
    // aliases, are represented before duplicate (format, value) pairs merge.
    for format in [
        NumberingFormat::FORMAT_DATE_YYYYMMDD2,
        NumberingFormat::FORMAT_DATE_YYYYMMDD,
        NumberingFormat::FORMAT_DATE_DDMMYYYY,
        NumberingFormat::FORMAT_DATE_DDMMYYYYSLASH,
        NumberingFormat::FORMAT_DATE_DMYSLASH,
        NumberingFormat::FORMAT_DATE_DMYMINUS,
        NumberingFormat::FORMAT_DATE_DMMINUS,
        NumberingFormat::FORMAT_DATE_MYMINUS,
        NumberingFormat::FORMAT_DATE_XLSX14,
        NumberingFormat::FORMAT_DATE_XLSX15,
        NumberingFormat::FORMAT_DATE_XLSX16,
        NumberingFormat::FORMAT_DATE_XLSX17,
        NumberingFormat::FORMAT_DATE_XLSX22,
        NumberingFormat::FORMAT_DATE_DATETIME,
        NumberingFormat::FORMAT_DATE_TIME1,
        NumberingFormat::FORMAT_DATE_TIME2,
        NumberingFormat::FORMAT_DATE_TIME3,
        NumberingFormat::FORMAT_DATE_TIME4,
        NumberingFormat::FORMAT_DATE_TIME5,
        NumberingFormat::FORMAT_DATE_TIME6,
        NumberingFormat::FORMAT_DATE_TIME8,
        NumberingFormat::FORMAT_DATE_YYYYMMDDSLASH,
    ] {
        add_values(&mut inputs, format, &["45435", "44349.211134259262"]);
    }
    add_values(&mut inputs, "d", &["44349.211134259262"]);
    for format in [
        r"#,##0_);[Red]\(#,##0\);\-\ \ ",
        r#""TRUE";"TRUE";"FALSE""#,
        "0;-0;",
    ] {
        add_values(&mut inputs, format, &["0"]);
    }

    for code in corpus_codes
        .as_array()
        .expect("corpus codes must be a JSON list")
    {
        add_values(
            &mut inputs,
            code.as_str().expect("each corpus code must be a string"),
            VALUES,
        );
    }

    // Exercise early returns and numeric formatting for every explicit edge
    // format, including formats that panic only after the early returns.
    for format in [
        "General",
        "general",
        "GENERAL",
        "@",
        "0",
        "0.00%",
        "yyyy-mm-dd",
        r#""quoted; literal""#,
        r#""prefix "0.00" suffix""#,
        r#""123""#,
        r"\q\u\o\t\e\d",
        r"0.00\ \u\n\i\t\s",
        r"0\;0",
        r#""a""b""#,
        "_x*x",
        "[Red]0",
        "[Blue]#,##0;[Red]-#,##0",
        "[Red][<0]0;[Blue][>=0]0",
        "[<0][Red]0;[>=0][Blue]0",
        "0;0;0;0;0;0",
        "0;0;0;0;0;[Red]0",
        "0;0;0;0;0;[<0]0",
        r#""literal";0;0;0;0;[Red]0"#,
        // Escaped controls become quoted only at the rewritten split.
        r"0;0;0;0;0;\[Red]0",
        r"0;0;0;0;0;\[<0]0",
        "[Red0",
        "[>=100",
        "0]",
        "\"unclosed",
        "0\"unclosed",
        "\\",
        "[",
        ";;;;;;",
        "",
    ] {
        add_values(&mut inputs, format, VALUES);
    }

    // Decimal spellings of adjacent f64 values on either side of each
    // threshold. Two sections make the condition affect section selection.
    for (format, neighbours) in [
        (
            r#"[>=100]0.00000000000000000;"otherwise""#,
            ["99.99999999999999", "100", "100.00000000000001"],
        ),
        (
            r#"[<0]0.00000000000000000;"otherwise""#,
            ["-5e-324", "0", "5e-324"],
        ),
        (
            r#"[=0]0.00000000000000000;"otherwise""#,
            ["-5e-324", "0", "5e-324"],
        ),
        (
            r#"[<>5]0.00000000000000000;"otherwise""#,
            ["4.999999999999999", "5", "5.000000000000001"],
        ),
    ] {
        add_values(&mut inputs, format, VALUES);
        add_values(&mut inputs, format, &neighbours);
    }
    inputs
}

fn observed_output(format: &str, value: &str) -> String {
    match panic::catch_unwind(|| to_formatted_string(value, format)) {
        Ok(output) => output,
        Err(payload) => {
            let message = payload
                .downcast_ref::<String>()
                .map(String::as_str)
                .or_else(|| payload.downcast_ref::<&str>().copied())
                .expect("formatter panics must have a string message");
            format!("PANIC: {message}")
        }
    }
}

const DATE_OVERFLOW: &str = "############################################";
const FIRST_UNSUPPORTED_DATE_SERIAL: f64 = 2_958_466.0;

/// The section Excel's rules pick for `value`, and the number shown in it:
/// two to four sections show the absolute value, other counts the value itself.
fn selected_section(format: &str, value: f64) -> Option<(String, f64)> {
    let color = Regex::new(r"\[(Black|White|Red|Green|Blue|Yellow|Magenta|Cyan)\]").unwrap();
    let condition = Regex::new(r"\[(>|>=|<|<=|=|<>)([+-]?\d+([.]\d+)?)\]").unwrap();
    let mut sections = Vec::new();
    for (index, section) in split(get_section_regex(), format).enumerate() {
        let colored = color.is_match(section).unwrap();
        let conditional = condition.captures(section).unwrap();
        if index >= 5 && (colored || conditional.is_some()) {
            return None;
        }
        let cond = conditional.map(|captures| {
            (
                captures[1].to_owned(),
                captures[2].parse::<f64>().unwrap_or(0.0),
            )
        });
        let shown = color.replace_all(section, "");
        let shown = condition.replace_all(&shown, "");
        sections.push((shown.into_owned(), cond));
    }
    let holds = |index: usize, default: &str| {
        let (cond, threshold) = sections[index]
            .1
            .clone()
            .unwrap_or_else(|| (default.to_owned(), 0.0));
        match cond.as_str() {
            ">" => value > threshold,
            "<" => value < threshold,
            "<=" => value <= threshold,
            "<>" => value != threshold,
            "=" => value == threshold,
            _ => value >= threshold,
        }
    };
    let index = match sections.len() {
        2 if !holds(0, ">=") => 1,
        3 | 4 if !holds(0, ">") => {
            if holds(1, "<") {
                1
            } else {
                2
            }
        }
        _ => 0,
    };
    let shown = if (2..=4).contains(&sections.len()) {
        value.abs()
    } else {
        value
    };
    Some((sections.swap_remove(index).0, shown))
}

/// A finite value shown through a date section outside Excel's serial range
/// (below zero or at 2958466 and above) is the date overflow text.
fn shows_date_overflow(section: &str, shown: f64) -> bool {
    // Backslash escapes are literals, so they cannot make a date section.
    let section = get_escape_regex()
        .try_replacen(section, 0, r#""$0""#)
        .unwrap()
        .into_owned();
    get_date_time_regex().is_match(&section).unwrap()
        && (shown < 0.0 || shown >= FIRST_UNSUPPORTED_DATE_SERIAL)
}

/// The output the formatter must give today when it differs from the
/// historical answer: the value text for non-finite input, and for a color or
/// condition past the fifth section (the old index panic); the overflow text
/// for a date section outside the serial range.
fn changed_output(format: &str, value: &str) -> Option<String> {
    let number = value.parse::<f64>().ok()?;
    if !number.is_finite() {
        return Some(value.to_owned());
    }
    match selected_section(format, number) {
        None => Some(number.to_string()),
        Some((section, shown)) => {
            shows_date_overflow(&section, shown).then(|| DATE_OVERFLOW.to_owned())
        }
    }
}

fn with_silent_panics<T>(operation: impl FnOnce() -> T + panic::UnwindSafe) -> T {
    // This integration binary has six tests, and only the ignored capture test
    // uses this hook. Run capture by its exact filter, so changing the
    // process-wide hook cannot hide a concurrent test's panic. Always restore
    // it before propagating failures.
    let hook = panic::take_hook();
    panic::set_hook(Box::new(|_| {}));
    let result = panic::catch_unwind(operation);
    panic::set_hook(hook);
    match result {
        Ok(value) => value,
        Err(payload) => panic::resume_unwind(payload),
    }
}

// Capture the real formatter's outputs and failures before changing its
// preparation, so the regression expectations have an independent origin.
#[test]
#[ignore = "writes the known-answer fixture only when explicitly requested"]
fn captures_number_format_known_answers() {
    let output_path = env::var_os("UMYA_NUMBER_FORMAT_CAPTURE_PATH")
        .expect("set UMYA_NUMBER_FORMAT_CAPTURE_PATH to the output JSON path");
    let corpus_path = env::var_os("UMYA_CORPUS_FORMAT_CODES_PATH")
        .expect("set UMYA_CORPUS_FORMAT_CODES_PATH to the corpus code JSON list");
    let corpus_codes: Value =
        serde_json::from_slice(&fs::read(corpus_path).expect("read corpus codes"))
            .expect("parse corpus codes");
    let inputs = capture_inputs(&corpus_codes);
    let rows: Vec<Value> = with_silent_panics(|| {
        inputs
            .iter()
            .map(|(format, value)| {
                json!({
                    "format": format,
                    "value": value,
                    "output": observed_output(format, value),
                })
            })
            .collect()
    });
    let mut bytes = serde_json::to_vec_pretty(&rows).expect("serialize known answers");
    bytes.push(b'\n');
    fs::write(output_path, bytes).expect("write known answers");
    println!("captured {} distinct format/value rows", rows.len());
}

// Every row keeps its captured answer, except rows whose answer changed on
// purpose: `changed_output` gives each one's exact new text.
#[test]
fn number_formats_match_known_answers() {
    let rows = number_format_expectations::validated_rows();
    let mut mismatches = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        let format = row["format"].as_str().expect("format must be a string");
        let value = row["value"].as_str().expect("value must be a string");
        let captured = row["output"].as_str().expect("output must be a string");
        let expected = changed_output(format, value).unwrap_or_else(|| captured.to_owned());
        let observed = to_formatted_string(value, format);
        if observed != expected {
            mismatches.push(format!(
                "row {index}: format {format:?}, value {value:?}: expected {expected:?}, got \
                 {observed:?}"
            ));
        }
    }
    assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
}

#[test]
fn date_serials_match_excel_cell_text_measurements() {
    const OVERFLOW: &str = "############################################";
    let formats = ["yyyy-mm-dd", "h:mm", "[h]:mm", "yyyy-mm-dd h:mm"];
    let cases = [
        ("1e20", [OVERFLOW, OVERFLOW, OVERFLOW, OVERFLOW]),
        ("-1", [OVERFLOW, OVERFLOW, OVERFLOW, OVERFLOW]),
        ("2958466", [OVERFLOW, OVERFLOW, OVERFLOW, OVERFLOW]),
        (
            "2958465.5",
            ["9999-12-31", "12:00", "71003172:00", "9999-12-31 12:00"],
        ),
        (
            "45000",
            ["2023-03-15", "0:00", "1080000:00", "2023-03-15 0:00"],
        ),
        (
            "1.7976931348623157e308",
            [OVERFLOW, OVERFLOW, OVERFLOW, OVERFLOW],
        ),
    ];

    for (value, expected) in cases {
        for (format, expected) in formats.into_iter().zip(expected) {
            assert_eq!(
                expected,
                to_formatted_string(value, format),
                "Excel cell text for value {value:?} and format {format:?}"
            );
        }
    }
}

#[test]
fn date_percent_literals_and_last_day_match_excel_measurements() {
    // measure_dateformat_literals.ps1 records Excel's rejection of yyyy%:
    // the cell keeps its General display, and TEXT returns #VALUE!.
    for (value, format, expected) in [
        ("45000", "yyyy%", "45000"),
        ("45000", r#"yyyy"%""#, "2023%"),
        ("45000", r"yyyy\%", "2023%"),
        ("45000", r#"yyyy"%Y %""#, "2023%Y %"),
        ("2958465.999991", "yyyy-mm-dd", "9999-12-31"),
        (
            "2958465.999991",
            "yyyy-mm-dd h:mm:ss",
            "9999-12-31 23:59:59",
        ),
    ] {
        assert_eq!(
            to_formatted_string(value, format),
            expected,
            "{value} {format}"
        );
    }
}

#[test]
fn known_formats_do_not_panic_for_extreme_floats() {
    let values = [
        "NaN",
        "inf",
        "-inf",
        "1.7976931348623157e308",
        "-1.7976931348623157e308",
        "1e20",
        "-1e20",
    ];
    let mut formats: BTreeSet<String> = [
        "General",
        "0.00",
        "0.00%",
        "yyyy-mm-dd",
        "h:mm",
        "[h]:mm",
        "yyyy-mm-dd h:mm",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    let rows: Vec<Value> =
        serde_json::from_str(include_str!("fixtures/number_format_known_answers.json")).unwrap();
    formats.extend(
        rows.iter()
            .map(|row| row["format"].as_str().unwrap().to_owned()),
    );
    formats.insert(format!("0{}", ",".repeat(20)));

    for value in values {
        for format in &formats {
            let _ = to_formatted_string(value, format);
        }
    }
}

#[test]
fn non_finite_values_keep_their_text_for_every_format_kind() {
    let mut fractions = NumberingFormat::default();
    let mut formats = vec![
        "# ?/?".to_owned(),
        "# ??/??".to_owned(),
        "0.00E+00".to_owned(),
        "0%".to_owned(),
        "0.00%".to_owned(),
        "@".to_owned(),
        "General".to_owned(),
        "#,##0.00".to_owned(),
        "yyyy-mm-dd".to_owned(),
        "[h]:mm".to_owned(),
    ];
    for id in [12, 13] {
        formats.push(fractions.set_number_format_id(id).format_code().to_owned());
    }
    assert_eq!(formats[10..], ["# ?/?", "# ??/??"]);

    for format in &formats {
        for value in ["NaN", "inf", "-inf"] {
            assert_eq!(
                to_formatted_string(value, format),
                value,
                "format {format:?}, value {value:?}"
            );
        }
    }
}
