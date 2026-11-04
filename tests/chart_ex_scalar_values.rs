#![cfg(feature = "serde")]

use umya_spreadsheet::drawing::charts::*;

fn document(bin: &str, axis: &str) -> String {
    format!(
        r#"<cx:chartSpace xmlns:cx="http://schemas.microsoft.com/office/drawing/2014/chartex"><cx:chartData><cx:data id="0"><cx:numDim type="val"><cx:lvl ptCount="1"><cx:pt idx="0">12</cx:pt></cx:lvl></cx:numDim></cx:data></cx:chartData><cx:chart><cx:plotArea><cx:plotAreaRegion><cx:series layoutId="clusteredColumn"><cx:dataId val="0"/><cx:layoutPr><cx:binning>{bin}</cx:binning></cx:layoutPr>{axis}</cx:series></cx:plotAreaRegion><cx:axis id="1"><cx:valScaling/></cx:axis></cx:plotArea></cx:chart></cx:chartSpace>"#
    )
}

#[test]
fn excel_attribute_and_schema_text_numeric_scalars_preserve_the_same_values() {
    let book = umya_spreadsheet::new_file();
    for (bin, axis, count, size) in [
        (
            r#"<cx:binCount val="4"/>"#,
            r#"<cx:axisId val="1"/>"#,
            Some(4),
            None,
        ),
        (
            "<cx:binCount>4</cx:binCount>",
            "<cx:axisId>1</cx:axisId>",
            Some(4),
            None,
        ),
        (
            r#"<cx:binSize val="2.5"/>"#,
            r#"<cx:axisId val="1"/>"#,
            None,
            Some(2.5),
        ),
        (
            "<cx:binSize>2.5</cx:binSize>",
            "<cx:axisId>1</cx:axisId>",
            None,
            Some(2.5),
        ),
    ] {
        let parsed = ChartExSpace::from_xml(document(bin, axis).as_bytes()).unwrap();
        let encoded = serde_json::to_vec(&parsed).unwrap();
        let restored: ChartExSpace = serde_json::from_slice(&encoded).unwrap();
        let series = &restored.chart().unwrap().series()[0];
        assert_eq!(series.axis_ids, [1]);
        let binning = series
            .layout_properties
            .as_ref()
            .unwrap()
            .binning
            .as_ref()
            .unwrap();
        assert_eq!(binning.bin_count, count);
        assert_eq!(binning.bin_size.as_ref().and_then(XmlDouble::value), size);
        let mut resolver = WorkbookChartExResolver::new(&book);
        let xml = restored.to_xml("Sheet1", &mut resolver).unwrap();
        let written = std::str::from_utf8(&xml).unwrap();
        assert!(written.contains(r#"<cx:axisId val="1"/>"#), "{written}");
        assert!(
            written.contains(if count.is_some() {
                r#"<cx:binCount val="4"/>"#
            } else {
                r#"<cx:binSize val="2.5"/>"#
            }),
            "{written}"
        );
        let reread = ChartExSpace::from_xml(&xml).unwrap();
        assert_eq!(
            serde_json::to_value(reread).unwrap(),
            serde_json::to_value(restored).unwrap()
        );
    }
}

#[test]
fn numeric_scalars_reject_mixed_missing_unknown_or_malformed_values() {
    for bin in [
        r#"<cx:binCount val="4">8</cx:binCount>"#,
        r#"<cx:binSize val="2.5">3</cx:binSize>"#,
        r#"<cx:binCount val="4" extra="1"/>"#,
        r#"<cx:binCount val="-1"/>"#,
        r#"<cx:binSize val="invalid"/>"#,
        "<cx:binCount/>",
        r#"<cx:binCount val="4"><cx:unexpected/></cx:binCount>"#,
    ] {
        assert!(
            ChartExSpace::from_xml(document(bin, "<cx:axisId>1</cx:axisId>").as_bytes()).is_err(),
            "{bin}"
        );
    }
    for axis in [
        r#"<cx:axisId val="1">2</cx:axisId>"#,
        "<cx:axisId/>",
        r#"<cx:axisId val="1" extra="2"/>"#,
    ] {
        assert!(
            ChartExSpace::from_xml(document("<cx:binCount>4</cx:binCount>", axis).as_bytes())
                .is_err(),
            "{axis}"
        );
    }
}
