use umya_spreadsheet::{
    PivotChartFormats,
    PivotConditionalFormatType,
    PivotConditionalFormats,
    PivotFormatAction,
    PivotFormatScope,
    PivotTableFormats,
};

const NS: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";

fn document(owner: &str, attributes: &str, children: &str) -> String {
    format!(r#"<{owner} xmlns="{NS}" {attributes}>{children}</{owner}>"#)
}

fn chart_formats(children: &str) -> String {
    document("chartFormats", "", children)
}

// Unmodified chartFormats subtree from the native pivotTable1.xml loss witness;
// its inherited SpreadsheetML namespace is made explicit for standalone
// parsing.
const NATIVE_CHART_FORMATS: &str = r#"<chartFormats xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" count="9"><chartFormat chart="0" format="0" series="1"><pivotArea type="data" outline="0" fieldPosition="0"><references count="1"><reference field="4294967294" count="1" selected="0"><x v="0"/></reference></references></pivotArea></chartFormat><chartFormat chart="0" format="1"><pivotArea type="data" outline="0" fieldPosition="0"><references count="2"><reference field="4294967294" count="1" selected="0"><x v="0"/></reference><reference field="3" count="1" selected="0"><x v="0"/></reference></references></pivotArea></chartFormat><chartFormat chart="0" format="2"><pivotArea type="data" outline="0" fieldPosition="0"><references count="2"><reference field="4294967294" count="1" selected="0"><x v="0"/></reference><reference field="3" count="1" selected="0"><x v="1"/></reference></references></pivotArea></chartFormat><chartFormat chart="0" format="3"><pivotArea type="data" outline="0" fieldPosition="0"><references count="2"><reference field="4294967294" count="1" selected="0"><x v="0"/></reference><reference field="3" count="1" selected="0"><x v="2"/></reference></references></pivotArea></chartFormat><chartFormat chart="0" format="4"><pivotArea type="data" outline="0" fieldPosition="0"><references count="2"><reference field="4294967294" count="1" selected="0"><x v="0"/></reference><reference field="3" count="1" selected="0"><x v="3"/></reference></references></pivotArea></chartFormat><chartFormat chart="0" format="5"><pivotArea type="data" outline="0" fieldPosition="0"><references count="2"><reference field="4294967294" count="1" selected="0"><x v="0"/></reference><reference field="3" count="1" selected="0"><x v="4"/></reference></references></pivotArea></chartFormat><chartFormat chart="0" format="6"><pivotArea type="data" outline="0" fieldPosition="0"><references count="2"><reference field="4294967294" count="1" selected="0"><x v="0"/></reference><reference field="3" count="1" selected="0"><x v="5"/></reference></references></pivotArea></chartFormat><chartFormat chart="0" format="7"><pivotArea type="data" outline="0" fieldPosition="0"><references count="2"><reference field="4294967294" count="1" selected="0"><x v="0"/></reference><reference field="3" count="1" selected="0"><x v="6"/></reference></references></pivotArea></chartFormat><chartFormat chart="0" format="8"><pivotArea type="data" outline="0" fieldPosition="0"><references count="2"><reference field="4294967294" count="1" selected="0"><x v="0"/></reference><reference field="3" count="1" selected="0"><x v="7"/></reference></references></pivotArea></chartFormat></chartFormats>"#;

#[test]
fn native_chart_format_witness_preserves_every_ordered_area_reference() {
    let value = PivotChartFormats::from_xml(NATIVE_CHART_FORMATS.as_bytes()).unwrap();
    assert_eq!(value.count, Some(9));
    assert_eq!(value.formats.len(), 9);
    for (index, format) in value.formats.iter().enumerate() {
        assert_eq!(format.chart, 0);
        assert_eq!(format.format, index as u32);
        assert_eq!(format.area.outline, Some(false));
        assert_eq!(format.area.field_position, Some(0));
        let references = format.area.references.as_ref().unwrap();
        assert_eq!(references.references[0].field, Some(4_294_967_294));
        assert_eq!(references.references[0].selected, Some(false));
        assert_eq!(references.references[0].items, vec![0]);
        if index > 0 {
            assert_eq!(references.references[1].field, Some(3));
            assert_eq!(references.references[1].items, vec![(index - 1) as u32]);
        }
    }
    assert_eq!(value.formats[0].series, Some(true));
    assert_eq!(value.formats[1].series, None);
    let reread = PivotChartFormats::from_xml(&value.to_xml().unwrap()).unwrap();
    assert_eq!(reread.formats.len(), 9);
    for (before, after) in value.formats.iter().zip(&reread.formats) {
        assert_eq!(before.chart, after.chart);
        assert_eq!(before.format, after.format);
        assert_eq!(before.series, after.series);
        let before = &before.area.references.as_ref().unwrap().references;
        let after = &after.area.references.as_ref().unwrap().references;
        for (before, after) in before.iter().zip(after) {
            assert_eq!(before.field, after.field);
            assert_eq!(before.selected, after.selected);
            assert_eq!(before.items, after.items);
        }
        assert_eq!(before.len(), after.len());
    }
}

#[test]
fn chart_format_explicit_zero_and_absent_series_remain_distinct() {
    let xml = chart_formats(
        r#"<chartFormat chart="0" format="0" series="0"><pivotArea field="0" outline="0"/></chartFormat><chartFormat chart="0" format="1"><pivotArea/></chartFormat>"#,
    );
    let value = PivotChartFormats::from_xml(xml.as_bytes()).unwrap();
    assert_eq!(value.count, None);
    assert_eq!(value.formats[0].series, Some(false));
    assert_eq!(value.formats[1].series, None);
    let reread = PivotChartFormats::from_xml(&value.to_xml().unwrap()).unwrap();
    assert_eq!(reread.count, None);
    assert_eq!(reread.formats[0].series, Some(false));
    assert_eq!(reread.formats[0].area.field, Some(0));
    assert_eq!(reread.formats[0].area.outline, Some(false));
    assert_eq!(reread.formats[1].series, None);
}

#[test]
fn chart_format_typed_edits_change_the_exported_owner() {
    let mut value = PivotChartFormats::from_xml(NATIVE_CHART_FORMATS.as_bytes()).unwrap();
    value.formats[1].chart = 17;
    value.formats[1].format = 23;
    value.formats[1].series = Some(false);
    value.formats[1]
        .area
        .references
        .as_mut()
        .unwrap()
        .references[1]
        .items = vec![8, 3];
    let reread = PivotChartFormats::from_xml(&value.to_xml().unwrap()).unwrap();
    assert_eq!(reread.formats[1].chart, 17);
    assert_eq!(reread.formats[1].format, 23);
    assert_eq!(reread.formats[1].series, Some(false));
    assert_eq!(
        reread.formats[1]
            .area
            .references
            .as_ref()
            .unwrap()
            .references[1]
            .items,
        vec![8, 3]
    );
    assert_eq!(reread.formats[0].format, 0);
}

#[test]
fn format_action_enum_and_explicit_zero_dxf_are_editable() {
    for (lexical, expected) in [
        ("blank", PivotFormatAction::Blank),
        ("formatting", PivotFormatAction::Formatting),
        ("drill", PivotFormatAction::Drill),
        ("formula", PivotFormatAction::Formula),
    ] {
        let xml = document(
            "formats",
            "",
            &format!(r#"<format action="{lexical}" dxfId="0"><pivotArea/></format>"#),
        );
        let mut value = PivotTableFormats::from_xml(xml.as_bytes()).unwrap();
        assert_eq!(value.formats[0].action, Some(expected));
        assert_eq!(value.formats[0].dxf_id, Some(0));
        value.formats[0].action = Some(PivotFormatAction::Drill);
        value.formats[0].dxf_id = Some(7);
        let reread = PivotTableFormats::from_xml(&value.to_xml().unwrap()).unwrap();
        assert!(matches!(
            reread.formats[0].action,
            Some(PivotFormatAction::Drill)
        ));
        assert_eq!(reread.formats[0].dxf_id, Some(7));
    }
}

#[test]
fn absent_format_defaults_are_not_materialized() {
    let xml = document("formats", "", "<format><pivotArea/></format>");
    let value = PivotTableFormats::from_xml(xml.as_bytes()).unwrap();
    let reread = PivotTableFormats::from_xml(&value.to_xml().unwrap()).unwrap();
    assert_eq!(reread.count, None);
    assert!(reread.formats[0].action.is_none());
    assert_eq!(reread.formats[0].dxf_id, None);
}

#[test]
fn conditional_format_preserves_zero_priority_and_empty_present_areas() {
    let xml = document(
        "conditionalFormats",
        "count=\"1\"",
        r#"<conditionalFormat priority="0" scope="selection" type="none"><pivotAreas count="0"/></conditionalFormat>"#,
    );
    let mut value = PivotConditionalFormats::from_xml(xml.as_bytes()).unwrap();
    assert_eq!(value.formats[0].priority, 0);
    assert_eq!(value.formats[0].areas.count, Some(0));
    assert!(value.formats[0].areas.areas.is_empty());
    assert!(matches!(
        value.formats[0].scope,
        Some(PivotFormatScope::Selection)
    ));
    assert!(matches!(
        value.formats[0].format_type,
        Some(PivotConditionalFormatType::None)
    ));
    value.formats[0].priority = 12;
    value.formats[0].scope = Some(PivotFormatScope::Field);
    value.formats[0].format_type = Some(PivotConditionalFormatType::Column);
    let reread = PivotConditionalFormats::from_xml(&value.to_xml().unwrap()).unwrap();
    assert_eq!(reread.formats[0].priority, 12);
    assert_eq!(reread.formats[0].areas.count, Some(0));
    assert!(matches!(
        reread.formats[0].scope,
        Some(PivotFormatScope::Field)
    ));
    assert!(matches!(
        reread.formats[0].format_type,
        Some(PivotConditionalFormatType::Column)
    ));
}

#[test]
fn conditional_scope_and_type_cover_the_complete_schema_lexicon() {
    for scope in ["selection", "data", "field"] {
        for kind in ["none", "all", "row", "column"] {
            let xml = document(
                "conditionalFormats",
                "",
                &format!(
                    r#"<conditionalFormat priority="1" scope="{scope}" type="{kind}"><pivotAreas><pivotArea field="2"/><pivotArea field="1"/></pivotAreas></conditionalFormat>"#
                ),
            );
            let value = PivotConditionalFormats::from_xml(xml.as_bytes()).unwrap();
            let xml = String::from_utf8(value.to_xml().unwrap()).unwrap();
            assert!(xml.contains(&format!(r#"scope="{scope}""#)));
            assert!(xml.contains(&format!(r#"type="{kind}""#)));
            let reread = PivotConditionalFormats::from_xml(xml.as_bytes()).unwrap();
            assert_eq!(reread.formats[0].areas.areas[0].field, Some(2));
            assert_eq!(reread.formats[0].areas.areas[1].field, Some(1));
        }
    }
}

#[test]
fn extension_payloads_keep_their_namespace_attributes_and_lexical_content() {
    let tail = r#"<extLst xmlns:f="urn:pivot:future"><ext uri="urn:future"><f:payload f:flag="0"> keep  </f:payload></ext></extLst>"#;
    let xml = document(
        "formats",
        "",
        &format!("<format><pivotArea/>{tail}</format>"),
    );
    let value = PivotTableFormats::from_xml(xml.as_bytes()).unwrap();
    assert!(value.formats[0].extensions.is_some());
    let exported = String::from_utf8(value.to_xml().unwrap()).unwrap();
    assert!(exported.contains("urn:pivot:future"));
    assert!(exported.contains("keep  "));
    let reread = PivotTableFormats::from_xml(exported.as_bytes()).unwrap();
    assert!(reread.formats[0].extensions.is_some());
    let xml = document(
        "conditionalFormats",
        "",
        &format!(r#"<conditionalFormat priority="0"><pivotAreas/>{tail}</conditionalFormat>"#),
    );
    let value = PivotConditionalFormats::from_xml(xml.as_bytes()).unwrap();
    assert!(
        PivotConditionalFormats::from_xml(&value.to_xml().unwrap())
            .unwrap()
            .formats[0]
            .extensions
            .is_some()
    );
}

#[test]
fn required_children_and_nonempty_collection_cardinalities_are_enforced() {
    for xml in [
        document("formats", "", ""),
        document("formats", "", "<format/>"),
        document("formats", "", "<format><extLst/></format>"),
    ] {
        assert!(
            PivotTableFormats::from_xml(xml.as_bytes()).is_err(),
            "accepted {xml}"
        );
    }
    for xml in [
        document("conditionalFormats", "", ""),
        document(
            "conditionalFormats",
            "",
            r#"<conditionalFormat priority="0"/>"#,
        ),
    ] {
        assert!(
            PivotConditionalFormats::from_xml(xml.as_bytes()).is_err(),
            "accepted {xml}"
        );
    }
    for xml in [
        chart_formats(""),
        chart_formats(r#"<chartFormat chart="0" format="0"/>"#),
    ] {
        assert!(
            PivotChartFormats::from_xml(xml.as_bytes()).is_err(),
            "accepted {xml}"
        );
    }
}

#[test]
fn invalid_required_attributes_and_lexical_values_are_rejected() {
    for attributes in [
        "",
        "chart=\"0\"",
        "format=\"0\"",
        "chart=\"-1\" format=\"0\"",
        "chart=\"4294967296\" format=\"0\"",
        "chart=\"0\" format=\"0\" series=\"yes\"",
    ] {
        let xml = chart_formats(&format!(
            "<chartFormat {attributes}><pivotArea/></chartFormat>"
        ));
        assert!(
            PivotChartFormats::from_xml(xml.as_bytes()).is_err(),
            "accepted {xml}"
        );
    }
    for attributes in [
        "",
        "priority=\"-1\"",
        "priority=\"4294967296\"",
        "priority=\"0\" scope=\"other\"",
        "priority=\"0\" type=\"other\"",
    ] {
        let xml = document(
            "conditionalFormats",
            "",
            &format!("<conditionalFormat {attributes}><pivotAreas/></conditionalFormat>"),
        );
        assert!(
            PivotConditionalFormats::from_xml(xml.as_bytes()).is_err(),
            "accepted {xml}"
        );
    }
    for attributes in ["action=\"unknown\"", "dxfId=\"-1\""] {
        let xml = document(
            "formats",
            "",
            &format!("<format {attributes}><pivotArea/></format>"),
        );
        assert!(
            PivotTableFormats::from_xml(xml.as_bytes()).is_err(),
            "accepted {xml}"
        );
    }
}

#[test]
fn duplicate_children_wrong_order_and_unrecognized_owners_are_rejected() {
    for body in [
        "<pivotArea/><pivotArea/>",
        "<extLst/><pivotArea/>",
        "<pivotArea/><extLst/><extLst/>",
        "<unknown/>",
        "<pivotArea/><unknown/>",
        "<pivotArea xmlns=\"urn:foreign\"/>",
    ] {
        let xml = document("formats", "", &format!("<format>{body}</format>"));
        assert!(
            PivotTableFormats::from_xml(xml.as_bytes()).is_err(),
            "accepted {xml}"
        );
    }
    for body in [
        "<pivotAreas/><pivotAreas/>",
        "<extLst/><pivotAreas/>",
        "<pivotAreas><unknown/></pivotAreas>",
        "<pivotAreas xmlns=\"urn:foreign\"/>",
    ] {
        let xml = document(
            "conditionalFormats",
            "",
            &format!(r#"<conditionalFormat priority="0">{body}</conditionalFormat>"#),
        );
        assert!(
            PivotConditionalFormats::from_xml(xml.as_bytes()).is_err(),
            "accepted {xml}"
        );
    }
    let xml =
        chart_formats(r#"<chartFormat chart="0" format="0"><pivotArea/><extLst/></chartFormat>"#);
    assert!(PivotChartFormats::from_xml(xml.as_bytes()).is_err());
}

#[test]
fn unknown_or_qualified_attributes_never_silently_disappear() {
    for xml in [
        document("formats", "unknown=\"1\"", "<format><pivotArea/></format>"),
        document("formats", "", "<format unknown=\"1\"><pivotArea/></format>"),
        document(
            "formats",
            "",
            "<format xmlns:f=\"urn:foreign\" f:dxfId=\"0\"><pivotArea/></format>",
        ),
    ] {
        assert!(
            PivotTableFormats::from_xml(xml.as_bytes()).is_err(),
            "accepted {xml}"
        );
    }
    let xml = document(
        "conditionalFormats",
        "",
        r#"<conditionalFormat priority="0"><pivotAreas unknown="1"/></conditionalFormat>"#,
    );
    assert!(PivotConditionalFormats::from_xml(xml.as_bytes()).is_err());
    let xml = chart_formats(
        r#"<chartFormat chart="0" format="0" unknown="1"><pivotArea/></chartFormat>"#,
    );
    assert!(PivotChartFormats::from_xml(xml.as_bytes()).is_err());
}

#[test]
fn malformed_or_trailing_xml_is_not_accepted_as_a_partial_owner() {
    let valid = chart_formats(r#"<chartFormat chart="0" format="0"><pivotArea/></chartFormat>"#);
    for xml in [
        valid.trim_end_matches("</chartFormats>").to_owned(),
        format!("{valid}<unexpected/>"),
        format!("{valid}trailing"),
        valid.replace("chart=\"0\"", "chart=\"0\" chart=\"1\""),
    ] {
        assert!(
            PivotChartFormats::from_xml(xml.as_bytes()).is_err(),
            "accepted {xml}"
        );
    }
    let foreign = valid.replace(NS, "urn:foreign");
    assert!(PivotChartFormats::from_xml(foreign.as_bytes()).is_err());
}

#[test]
fn explicit_counts_are_checked_and_recomputed_only_when_present() {
    let xml = document("formats", "count=\"2\"", "<format><pivotArea/></format>");
    assert!(PivotTableFormats::from_xml(xml.as_bytes()).is_err());
    let xml = document(
        "conditionalFormats",
        "",
        r#"<conditionalFormat priority="0"><pivotAreas count="1"/></conditionalFormat>"#,
    );
    assert!(PivotConditionalFormats::from_xml(xml.as_bytes()).is_err());
    let mut value = PivotChartFormats::from_xml(NATIVE_CHART_FORMATS.as_bytes()).unwrap();
    value.formats.pop();
    let reread = PivotChartFormats::from_xml(&value.to_xml().unwrap()).unwrap();
    assert_eq!(reread.count, Some(8));
    assert_eq!(reread.formats.len(), 8);
}

#[test]
fn invalid_empty_edited_collections_fail_export() {
    let mut value = PivotChartFormats::from_xml(NATIVE_CHART_FORMATS.as_bytes()).unwrap();
    value.formats.clear();
    assert!(value.to_xml().is_err());
    let xml = document("formats", "", "<format><pivotArea/></format>");
    let mut value = PivotTableFormats::from_xml(xml.as_bytes()).unwrap();
    value.formats.clear();
    assert!(value.to_xml().is_err());
    let xml = document(
        "conditionalFormats",
        "",
        r#"<conditionalFormat priority="0"><pivotAreas/></conditionalFormat>"#,
    );
    let mut value = PivotConditionalFormats::from_xml(xml.as_bytes()).unwrap();
    value.formats.clear();
    assert!(value.to_xml().is_err());
}

#[cfg(feature = "serde")]
#[test]
fn typed_format_owners_survive_json_and_bincode_without_losing_presence() {
    macro_rules! assert_serde {
        ($type:ty, $value:expr) => {{
            let value = $value;
            let expected = serde_json::to_value(&value).unwrap();
            let json: $type = serde_json::from_slice(&serde_json::to_vec(&value).unwrap()).unwrap();
            let binary: $type = bincode::deserialize(&bincode::serialize(&value).unwrap()).unwrap();
            assert_eq!(serde_json::to_value(&json).unwrap(), expected);
            assert_eq!(serde_json::to_value(&binary).unwrap(), expected);
            let exported = binary.to_xml().unwrap();
            let reread = <$type>::from_xml(&exported).unwrap();
            assert_eq!(serde_json::to_value(&reread).unwrap(), expected);
        }};
    }
    assert_serde!(
        PivotChartFormats,
        PivotChartFormats::from_xml(NATIVE_CHART_FORMATS.as_bytes()).unwrap()
    );
    let xml = document(
        "formats",
        "",
        r#"<format action="blank" dxfId="0"><pivotArea field="0"/><extLst><ext uri="future"><x:value xmlns:x="urn:future"> keep </x:value></ext></extLst></format>"#,
    );
    assert_serde!(
        PivotTableFormats,
        PivotTableFormats::from_xml(xml.as_bytes()).unwrap()
    );
    let xml = document(
        "conditionalFormats",
        "count=\"1\"",
        r#"<conditionalFormat priority="0" scope="data" type="row"><pivotAreas count="0"/></conditionalFormat>"#,
    );
    assert_serde!(
        PivotConditionalFormats,
        PivotConditionalFormats::from_xml(xml.as_bytes()).unwrap()
    );
}
