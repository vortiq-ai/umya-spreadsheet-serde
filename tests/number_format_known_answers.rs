use std::{
    collections::BTreeSet,
    env,
    fs,
    panic,
};

use serde_json::{
    Value,
    json,
};
use umya_spreadsheet::{
    NumberingFormat,
    helper::number_format::to_formatted_string,
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
        // Escaped controls become quoted only at the rewritten split stage.
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

fn with_silent_panics<T>(operation: impl FnOnce() -> T + panic::UnwindSafe) -> T {
    // This integration binary contains only these two tests. Run capture by
    // its exact filter, so changing the process-wide hook cannot hide a
    // concurrent test's panic. Always restore it before propagating failures.
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

// This table was captured from the implementation before the prepared-format
// refactor. It is the external oracle for that refactor: both output bytes and
// panic messages must remain identical, including malformed-format behavior.
// Compare after restoring the hook so a mismatch identifies its fixture row.
#[test]
fn number_formats_match_known_answers() {
    let rows: Vec<Value> =
        serde_json::from_str(include_str!("fixtures/number_format_known_answers.json"))
            .expect("parse known-answer fixture");
    assert!(
        !rows.is_empty(),
        "capture the known-answer fixture before running the regression test"
    );
    let mismatches: Vec<_> = with_silent_panics(|| {
        let mut mismatches = Vec::new();
        for (index, row) in rows.iter().enumerate() {
            let format = row["format"].as_str().expect("format must be a string");
            let value = row["value"].as_str().expect("value must be a string");
            let expected = row["output"].as_str().expect("output must be a string");
            let observed = observed_output(format, value);
            if observed != expected {
                mismatches.push((index, observed));
            }
        }
        mismatches
    });
    for (index, observed) in mismatches {
        let row = &rows[index];
        assert_eq!(
            observed,
            row["output"].as_str().unwrap(),
            "known-answer row {index}: format {:?}, value {:?}",
            row["format"],
            row["value"],
        );
    }
}
