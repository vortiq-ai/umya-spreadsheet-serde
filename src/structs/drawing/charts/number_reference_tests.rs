// Both cache policies exercise the same real numeric-reference owner.
use quick_xml::events::Event;

use super::*;
fn parse(input: &str, policy: NumericSourceCachePolicy) -> (NumberReference, bool) {
    let mut reader = Reader::from_reader(input.as_bytes());
    let mut buf = Vec::new();
    let Event::Start(e) = reader.read_event_into(&mut buf).unwrap() else {
        panic!("root")
    };
    let mut reference = NumberReference::default();
    let valid = reference.read_with_policy(&mut reader, &e, false, policy);
    (reference, valid)
}
#[test]
fn existing_default_policy_keeps_cache_format_code_and_shared_formula_normalization() {
    let text = "<z:numRef><z:f>'O''Brien &amp; \
                Sons'!$B$2:$B$3</z:f><z:numCache><z:formatCode>0.000</z:formatCode><z:pt \
                idx=\"0\"><z:v>99999</z:v></z:pt></z:numCache></z:numRef>";
    let (read, valid) = parse(text, NumericSourceCachePolicy::Read);
    assert!(valid);
    assert_eq!(read.numbering_cache().format_code().text(), "0.000");
    let (skip, valid) = parse(text, NumericSourceCachePolicy::SkipSourceCache);
    assert!(valid);
    assert_eq!(skip.numbering_cache().format_code().text(), "");
    assert_eq!(skip.formula().address_str(), "'O''Brien & Sons'!$B$2:$B$3");
    assert_eq!(read.formula().address_str(), skip.formula().address_str());
    let mut reader = Reader::from_reader(text.as_bytes());
    let mut buf = Vec::new();
    let Event::Start(e) = reader.read_event_into(&mut buf).unwrap() else {
        panic!("root")
    };
    let mut default = NumberReference::default();
    default.set_attributes(&mut reader, &e);
    assert_eq!(default.numbering_cache().format_code().text(), "0.000");
    assert_eq!(
        default.formula().address_str(),
        read.formula().address_str()
    );
}
#[test]
fn skipped_cache_and_unknown_wrappers_cannot_replace_the_direct_formula() {
    let (value, valid) = parse(
        "<numRef><f>Sheet1!B2</f><numCache><f>Wrong!Z9</f></numCache><future><f>Wrong!Z8</f></\
         future></numRef>",
        NumericSourceCachePolicy::SkipSourceCache,
    );
    assert!(valid);
    assert_eq!(value.formula().address_str(), "Sheet1!B2");
}
#[test]
fn malformed_new_route_and_duplicate_formula_are_diagnosable_without_panics() {
    let (_, valid) = parse(
        "<numRef><f>Sheet1!B2</f>",
        NumericSourceCachePolicy::SkipSourceCache,
    );
    assert!(!valid);
    let (_, valid) = parse(
        "<numRef><f>Sheet1!B2</f><f>Sheet1!B3</f></numRef>",
        NumericSourceCachePolicy::SkipSourceCache,
    );
    assert!(!valid);
    let (_, valid) = parse(
        "<numRef><f/></numRef>",
        NumericSourceCachePolicy::SkipSourceCache,
    );
    assert!(valid);
    let (_, valid) = parse(
        "<numRef></numRef>",
        NumericSourceCachePolicy::SkipSourceCache,
    );
    assert!(!valid);
}

#[test]
fn legacy_reference_reader_panics_on_parse_failure_but_keeps_complete_formula_behavior() {
    for xml in [
        "<numRef><f>Sheet1!B2</f>",
        "<numRef><f>Sheet1!B2</wrong></numRef>",
    ] {
        let failed = std::panic::catch_unwind(|| {
            let mut reader = Reader::from_reader(xml.as_bytes());
            let mut buffer = Vec::new();
            let Event::Start(e) = reader.read_event_into(&mut buffer).unwrap() else {
                panic!("root")
            };
            let mut reference = NumberReference::default();
            reference.set_attributes(&mut reader, &e);
        });
        assert!(failed.is_err());
        let (_, valid) = parse(xml, NumericSourceCachePolicy::SkipSourceCache);
        assert!(!valid);
    }
    for (xml, formula) in [
        ("<numRef></numRef>", None),
        (
            "<numRef><f>Sheet1!B2</f><f>Sheet1!B3</f></numRef>",
            Some("Sheet1!B3"),
        ),
    ] {
        let mut reader = Reader::from_reader(xml.as_bytes());
        let mut buffer = Vec::new();
        let Event::Start(e) = reader.read_event_into(&mut buffer).unwrap() else {
            panic!("root")
        };
        let mut reference = NumberReference::default();
        reference.set_attributes(&mut reader, &e);
        if let Some(formula) = formula {
            assert_eq!(reference.formula().address_str(), formula);
        }
        let (_, valid) = parse(xml, NumericSourceCachePolicy::SkipSourceCache);
        assert!(!valid);
    }
}

#[test]
fn invalid_formula_entities_are_fallible_for_both_policies_and_panic_in_legacy_wrapper() {
    for xml in [
        "<numRef><f>Sheet1!B2 &bogus;</f></numRef>",
        "<numRef><f>Sheet1!B2 &broken</f></numRef>",
    ] {
        for policy in [
            NumericSourceCachePolicy::Read,
            NumericSourceCachePolicy::SkipSourceCache,
        ] {
            let result = std::panic::catch_unwind(|| parse(xml, policy));
            assert!(result.is_ok());
            assert!(!result.unwrap().1);
        }
        let result = std::panic::catch_unwind(|| {
            let mut reader = Reader::from_reader(xml.as_bytes());
            let mut buffer = Vec::new();
            let Event::Start(e) = reader.read_event_into(&mut buffer).unwrap() else {
                panic!("root")
            };
            NumberReference::default().set_attributes(&mut reader, &e);
        });
        assert!(result.is_err());
    }
}

#[test]
fn formula_text_decodes_cdata_and_entities_once_for_both_policies_and_legacy_reads() {
    for (text, expected) in [
        ("<![CDATA[Sheet1!B2]]>", "Sheet1!B2"),
        ("<![CDATA[Sheet1!B]]>&#50;", "Sheet1!B2"),
        (
            "  <![CDATA[Literal &amp; ]]>&amp; value  ",
            "  Literal &amp; & value  ",
        ),
    ] {
        let xml = format!("<numRef><f>{text}</f></numRef><tail>  trimmed  </tail>");
        for policy in [
            NumericSourceCachePolicy::Read,
            NumericSourceCachePolicy::SkipSourceCache,
        ] {
            let (reference, valid) = parse(&xml, policy);
            assert!(valid);
            assert_eq!(reference.formula().address_str(), expected);
        }
        let mut reader = Reader::from_reader(xml.as_bytes());
        reader.config_mut().trim_text(true);
        let mut buffer = Vec::new();
        let Event::Start(owner) = reader.read_event_into(&mut buffer).unwrap() else {
            panic!("root")
        };
        let mut reference = NumberReference::default();
        reference.set_attributes(&mut reader, &owner);
        assert_eq!(reference.formula().address_str(), expected);
        assert!(matches!(
            reader.read_event_into(&mut buffer).unwrap(),
            Event::Start(_)
        ));
        let Event::Text(tail) = reader.read_event_into(&mut buffer).unwrap() else {
            panic!("tail text")
        };
        assert_eq!(tail.decode().unwrap(), "trimmed");
    }
    for xml in [
        "<numRef><f>Sheet1<bad/>!B2</f></numRef>",
        "<numRef><f><![CDATA[Sheet1!B2</f></numRef>",
    ] {
        for policy in [
            NumericSourceCachePolicy::Read,
            NumericSourceCachePolicy::SkipSourceCache,
        ] {
            assert!(!parse(xml, policy).1);
        }
        let result = std::panic::catch_unwind(|| {
            let mut reader = Reader::from_reader(xml.as_bytes());
            let mut buffer = Vec::new();
            let Event::Start(owner) = reader.read_event_into(&mut buffer).unwrap() else {
                panic!("root")
            };
            NumberReference::default().set_attributes(&mut reader, &owner);
        });
        assert!(result.is_err());
    }
}
