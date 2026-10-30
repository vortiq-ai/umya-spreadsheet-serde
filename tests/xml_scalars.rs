use umya_spreadsheet::{
    FilterColumn,
    PivotCacheHierarchies,
    SharedItemValue,
    SharedItems,
    SlicerCache,
    SlicerSource,
};
const CORE: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
const SLICER: &str = "http://schemas.microsoft.com/office/spreadsheetml/2009/9/main";

#[test]
fn booleans_and_unsigned_integers_accept_xml_whitespace() {
    let xml = format!(
        r#"<filterColumn xmlns="{CORE}" colId=" &#x9;4294967295&#xA;&#xD; " hiddenButton=" &#x9;false&#xA; " showButton="&#xD; true &#x9;"/>"#
    );
    let column = FilterColumn::from_xml(xml.as_bytes()).unwrap();
    assert_eq!(column.column_id(), u32::MAX);
    assert!(!column.hidden_button());
    assert_eq!(column.show_button(), Some(true));
    let xml = format!(
        r#"<filterColumn xmlns="{CORE}" colId=" 0 " hiddenButton=" 1 " showButton=" 0 "/>"#
    );
    let column = FilterColumn::from_xml(xml.as_bytes()).unwrap();
    assert_eq!(column.column_id(), 0);
    assert!(column.hidden_button());
    assert_eq!(column.show_button(), Some(false));
}
#[test]
fn booleans_and_unsigned_integers_reject_non_xml_whitespace() {
    for (attribute, value) in [
        ("showButton", "\u{a0}true\u{a0}"),
        ("hiddenButton", "\u{a0}1\u{a0}"),
        ("colId", "\u{a0}1\u{a0}"),
    ] {
        let other = if attribute == "colId" {
            String::new()
        } else {
            " colId=\"0\"".into()
        };
        let xml = format!(r#"<filterColumn xmlns="{CORE}"{other} {attribute}="{value}"/>"#);
        assert!(
            FilterColumn::from_xml(xml.as_bytes()).is_err(),
            "accepted {attribute}={value:?}"
        );
    }
}
#[test]
fn unsigned_integer_boundaries_are_checked() {
    for value in ["-1", "4294967296"] {
        let xml = format!(r#"<filterColumn xmlns="{CORE}" colId="{value}"/>"#);
        assert!(FilterColumn::from_xml(xml.as_bytes()).is_err());
    }
    let xml = format!(
        r#"<slicerCacheDefinition xmlns="{SLICER}" name="Cache" sourceName="Field"><data><tabular pivotCacheId="4294967295"/></data></slicerCacheDefinition>"#
    );
    let cache = SlicerCache::from_xml(xml.as_bytes()).unwrap();
    let Some(SlicerSource::TabularPivot(source)) = cache.source else {
        panic!("wrong source")
    };
    assert_eq!(source.pivot_cache_id, u32::MAX);
}
fn hierarchy(value: &str) -> String {
    format!(
        r#"<cacheHierarchies xmlns="{CORE}" count="1"><cacheHierarchy uniqueName="[H]" count="0" iconSet="{value}"/></cacheHierarchies>"#
    )
}
#[test]
fn signed_integers_accept_xml_whitespace_and_preserve_boundaries() {
    for (value, expected) in [
        (" &#x9;-1&#xA;&#xD; ", -1),
        ("-2147483648", i32::MIN),
        ("2147483647", i32::MAX),
    ] {
        let parsed = PivotCacheHierarchies::from_xml(hierarchy(value).as_bytes()).unwrap();
        assert_eq!(parsed.hierarchies[0].icon_set, Some(expected));
    }
}
#[test]
fn signed_integers_reject_non_xml_whitespace_and_overflow() {
    for value in ["\u{a0}-1\u{a0}", "-2147483649", "2147483648"] {
        assert!(
            PivotCacheHierarchies::from_xml(hierarchy(value).as_bytes()).is_err(),
            "accepted {value:?}"
        );
    }
}
#[test]
fn doubles_preserve_xml_whitespace_and_special_values() {
    for (value, expected) in [
        (" &#x9;INF&#xA; ", f64::INFINITY),
        (" &#xD;-INF ", f64::NEG_INFINITY),
        (" 1.25e+2 ", 125.0),
    ] {
        let xml = format!(r#"<sharedItems xmlns="{CORE}"><n v="{value}"/></sharedItems>"#);
        let values = SharedItems::from_xml(xml.as_bytes()).unwrap();
        let SharedItemValue::Numeric(value) = values.items()[0].scalar() else {
            panic!("wrong scalar")
        };
        assert_eq!(*value, expected);
    }
    let xml = format!(r#"<sharedItems xmlns="{CORE}"><n v=" NaN "/></sharedItems>"#);
    let values = SharedItems::from_xml(xml.as_bytes()).unwrap();
    let SharedItemValue::Numeric(value) = values.items()[0].scalar() else {
        panic!("wrong scalar")
    };
    assert!(value.is_nan());
}
#[test]
fn doubles_reject_non_xml_whitespace() {
    for value in ["\u{a0}1\u{a0}", "\u{a0}INF\u{a0}"] {
        let xml = format!(r#"<sharedItems xmlns="{CORE}"><n v="{value}"/></sharedItems>"#);
        assert!(
            SharedItems::from_xml(xml.as_bytes()).is_err(),
            "accepted {value:?}"
        );
    }
}
