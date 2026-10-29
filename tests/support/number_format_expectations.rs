pub(crate) fn validated_rows() -> Vec<serde_json::Value> {
    use std::collections::{
        BTreeMap,
        BTreeSet,
    };

    use sha2::{
        Digest,
        Sha256,
    };

    let source = include_str!("../fixtures/number_format_known_answers.json");
    let canonical_hash = |text: &str| {
        Sha256::digest(text.replace("\r\n", "\n").as_bytes())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    };
    let expected_hash = "6e1a2348e6ee01e03497d7aeb1961259719ec131d593881bef8cee3fb8fc0235";
    let source_hash = canonical_hash(source);
    assert_eq!(source_hash, expected_hash);
    let lf_source = source.replace("\r\n", "\n");
    assert_eq!(canonical_hash(&lf_source), expected_hash);
    assert_eq!(
        canonical_hash(&lf_source.replace('\n', "\r\n")),
        expected_hash
    );
    assert_ne!(canonical_hash(&format!("{lf_source} ")), expected_hash);
    assert_ne!(
        canonical_hash(&lf_source.replacen('\n', "\r", 1)),
        expected_hash
    );
    let rows: Vec<serde_json::Value> = serde_json::from_str(source).unwrap();
    let corrections: serde_json::Value = serde_json::from_str(include_str!(
        "../fixtures/number_format_quoted_composites.json"
    ))
    .unwrap();
    assert_eq!(rows.len(), 10998);
    assert_eq!(corrections["format"], "quoted-composite-expectations-r3");
    assert_eq!(corrections["historical_fixture_lf_sha256"], source_hash);
    assert_eq!(corrections["historical_row_count"], 10998);
    assert_eq!(corrections["corrected_row_count"], 64);
    let corrected_rows = corrections["rows"].as_array().unwrap();
    assert_eq!(corrected_rows.len(), 64);
    let inputs = [
        "-0",
        "-1",
        "-1234.5678",
        "-inf",
        "0",
        "0.5",
        "007",
        "1",
        "1e-7",
        "1e21",
        "1e3",
        "45000.75",
        "NaN",
        "inf",
    ];
    let negatives = ["-1", "-1234.5678", "-inf"];
    let negatives_and_nan = ["-1", "-1234.5678", "-inf", "NaN"];
    let rules = [
        ("\"(\"#.00\")\"", "0.00", "(", ")", "identity", &inputs[..]),
        (
            "\"FV \"0.0\" SAR\"",
            "0.0",
            "FV ",
            " SAR",
            "identity",
            &inputs[..],
        ),
        (
            "\"prefix \"0.00\" suffix\"",
            "0.00",
            "prefix ",
            " suffix",
            "identity",
            &inputs[..],
        ),
        (
            "#,##0_)\"Years\";\\(#,##0\") Years\";\\-",
            "#,##0",
            "(",
            ") Years",
            "absolute",
            &negatives[..],
        ),
        (
            "#,##0_)\"Years\";\\(#,##0\\)\" Years\";\"-\"",
            "#,##0",
            "(",
            ") Years",
            "absolute",
            &negatives[..],
        ),
        (
            "0.00\\x;\\(0.00\"x)\";\\-",
            "0.00",
            "(",
            "x)",
            "absolute",
            &negatives[..],
        ),
        (
            "0.00\\x_);\\(0.00\"x)\";\\–_)",
            "0.00",
            "(",
            "x)",
            "absolute",
            &negatives[..],
        ),
        (
            "0.0\\x;\\(0.0\"x)\"",
            "0.0",
            "(",
            "x)",
            "absolute",
            &negatives_and_nan[..],
        ),
        (
            "0.0\\x;\\(0.0\"x)\";\\-",
            "0.0",
            "(",
            "x)",
            "absolute",
            &negatives[..],
        ),
        (
            "0.0\\x_);\\(0.0\"x)\";\\–_)",
            "0.0",
            "(",
            "x)",
            "absolute",
            &negatives[..],
        ),
    ];
    assert_eq!(rules.len(), 10);
    let domain: BTreeSet<_> = rules
        .iter()
        .flat_map(|(code, _, _, _, _, values)| values.iter().map(move |value| (*code, *value)))
        .collect();
    assert_eq!(domain.len(), 64);
    let mut corrected_domain = BTreeSet::new();
    let mut expected_by_index = BTreeMap::new();
    for correction in corrected_rows {
        let index = usize::try_from(correction["fixture_index"].as_u64().unwrap()).unwrap();
        let row = &rows[index];
        let code = correction["format"].as_str().unwrap();
        let value = correction["value"].as_str().unwrap();
        assert_eq!(row["format"], code);
        assert_eq!(row["value"], value);
        assert_eq!(
            row["output"],
            "PANIC: called `Result::unwrap()` on an `Err` value: ParseFloatError { kind: Invalid }"
        );
        assert!(domain.contains(&(code, value)));
        assert!(corrected_domain.insert((code, value)));
        let (_, base_format, prefix, suffix, transform, _) =
            rules.iter().find(|(format, ..)| *format == code).unwrap();
        assert_eq!(correction["value_transform"], *transform);
        assert_eq!(correction["base_format"], *base_format);
        assert_eq!(correction["prefix"], *prefix);
        assert_eq!(correction["suffix"], *suffix);
        let base_index =
            usize::try_from(correction["base_fixture_index"].as_u64().unwrap()).unwrap();
        let base = &rows[base_index];
        assert_eq!(base["format"], *base_format);
        assert_eq!(base["value"], value);
        assert_eq!(
            rows.iter()
                .filter(|row| row["format"] == *base_format && row["value"] == value)
                .count(),
            1
        );
        let base_output = base["output"].as_str().unwrap();
        assert!(!base_output.starts_with("PANIC:"));
        assert_eq!(correction["base_output"], base_output);
        let transformed_base = match *transform {
            "identity" => base_output,
            "absolute" if value == "NaN" => base_output,
            "absolute" => {
                assert!(value.starts_with('-'));
                let unsigned = base_output.strip_prefix('-').unwrap();
                assert!(!unsigned.starts_with('-'));
                unsigned
            }
            _ => unreachable!(),
        };
        assert_eq!(correction["transformed_base_output"], transformed_base);
        let expected = correction["output"].as_str().unwrap();
        assert_eq!(expected, format!("{prefix}{transformed_base}{suffix}"));
        assert!(expected_by_index.insert(index, expected).is_none());
    }
    assert_eq!(corrected_domain, domain);
    assert_eq!(expected_by_index.len(), 64);
    let mut expected_rows = Vec::with_capacity(rows.len());
    let mut corrected_count = 0;
    let mut unchanged_count = 0;
    for (index, row) in rows.iter().enumerate() {
        let code = row["format"].as_str().unwrap();
        let value = row["value"].as_str().unwrap();
        let expected = if let Some(expected) = expected_by_index.get(&index) {
            corrected_count += 1;
            *expected
        } else {
            assert!(!domain.contains(&(code, value)));
            unchanged_count += 1;
            row["output"].as_str().unwrap()
        };
        let mut expected_row = row.clone();
        expected_row["output"] = expected.into();
        expected_rows.push(expected_row);
    }
    assert_eq!(corrected_count, 64);
    assert_eq!(unchanged_count, 10934);
    expected_rows
}
