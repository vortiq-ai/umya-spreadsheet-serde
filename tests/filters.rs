#[test]
fn every_native_filter_choice_roundtrips_without_losing_rule_attributes() {
    use umya_spreadsheet::FilterColumn;
    let rules = [
        r#"<filters blank="1" calendarType="gregorian"><filter val="A_x005F_x000A_"/><dateGroupItem year="2024" month="2" day="29" dateTimeGrouping="day"/></filters>"#,
        r#"<customFilters and="1"><customFilter operator="greaterThan" val="3"/><customFilter operator="lessThanOrEqual" val="8"/></customFilters>"#,
        r#"<dynamicFilter type="aboveAverage" val="3.5" maxVal="8"/>"#,
        r#"<top10 top="0" percent="1" val="25" filterVal="11"/>"#,
        r#"<colorFilter dxfId="7" cellColor="0"/>"#,
        r#"<iconFilter iconSet="3Arrows" iconId="2"/>"#,
    ];
    for rule in rules {
        let xml = format!(
            r#"<filterColumn xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" colId="2" hiddenButton="0" showButton="1">{rule}</filterColumn>"#
        );
        let column = FilterColumn::from_xml(xml.as_bytes()).unwrap();
        let first = column.to_xml().unwrap();
        let reread = FilterColumn::from_xml(&first).unwrap();
        assert_eq!(first, reread.to_xml().unwrap());
        assert!(reread.rule().is_some());
        assert_eq!(reread.show_button(), Some(true));
    }
}

#[test]
fn competing_or_foreign_filter_rules_are_rejected() {
    use umya_spreadsheet::FilterColumn;
    for rules in [
        r#"<filters/><top10 val="5"/>"#,
        r#"<filters xmlns="urn:foreign"/>"#,
        r#"<customFilters/>"#,
        r#"<iconFilter iconSet="imaginary"/>"#,
    ] {
        let xml = format!(
            r#"<filterColumn xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" colId="0">{rules}</filterColumn>"#
        );
        assert!(FilterColumn::from_xml(xml.as_bytes()).is_err());
    }
}

#[test]
fn inherited_future_prefixes_and_sort_state_roundtrip() {
    use umya_spreadsheet::AutoFilter;
    let xml = r#"<s:autoFilter xmlns:s="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:f="urn:future" ref="A1:C5"><s:filterColumn colId="1"><s:filters blank="0"><s:filter val="A"/></s:filters></s:filterColumn><s:sortState ref="A2:C5" columnSort="0" caseSensitive="1" sortMethod="stroke"><s:sortCondition ref="B2:B5" descending="1" sortBy="value" customList="A,B"/></s:sortState><s:extLst><s:ext uri="urn:future"><f:setting enabled="1"/></s:ext></s:extLst></s:autoFilter>"#;
    let filter = AutoFilter::from_xml(xml.as_bytes()).unwrap();
    let sort = filter.sort_state().unwrap();
    assert_eq!(sort.reference.range(), "A2:C5");
    assert_eq!(sort.conditions[0].descending, Some(true));
    let serialized = filter.to_xml().unwrap();
    let reread = AutoFilter::from_xml(&serialized).unwrap();
    assert_eq!(serialized, reread.to_xml().unwrap());
}

#[test]
fn auto_filter_revision_uid_roundtrips_and_private_marker_is_rejected() {
    let xml = br#"<autoFilter xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:rev="http://schemas.microsoft.com/office/spreadsheetml/2014/revision" rev:uid="{native-uid}" ref="A1:B5"/>"#;
    let filter = umya_spreadsheet::AutoFilter::from_xml(xml).unwrap();
    let serialized = filter.to_xml().unwrap();
    assert!(String::from_utf8_lossy(&serialized).contains("{native-uid}"));
    assert_eq!(
        serialized,
        umya_spreadsheet::AutoFilter::from_xml(&serialized)
            .unwrap()
            .to_xml()
            .unwrap()
    );
    assert!(umya_spreadsheet::AutoFilter::from_xml(br#"<autoFilter xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" ref="A1:B5" __umya_resolved_attributes="1"/>"#).is_err());
}
