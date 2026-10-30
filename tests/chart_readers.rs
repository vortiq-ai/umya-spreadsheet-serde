use std::io::Cursor;

use umya_spreadsheet::{
    Workbook,
    reader,
};

#[path = "support/chart_xml_fixture.rs"]
pub mod fixture;

const C: &str = "http://schemas.openxmlformats.org/drawingml/2006/chart";
const A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";

fn chart_xml(before: &str, after: &str) -> String {
    format!(
        r#"<c:chartSpace xmlns:c="{C}" xmlns:a="{A}">{before}<c:chart><c:plotArea><c:lineChart><c:grouping val="standard"/><c:axId val="1"/><c:axId val="2"/></c:lineChart></c:plotArea></c:chart>{after}</c:chartSpace>"#
    )
}

fn package(xml: &str) -> Vec<u8> {
    let mut book = umya_spreadsheet::new_file();
    book.sheet_mut(0).unwrap().add_chart(fixture::base_chart());
    fixture::replace_chart_part(&fixture::write_workbook(&book), xml)
}

fn imported(xml: &str) -> Workbook {
    reader::xlsx::read_reader(Cursor::new(package(xml)), true).unwrap()
}

#[test]
fn malformed_shared_properties_return_errors_without_committing_lazy_sheets() {
    let invalid = [
        chart_xml("", r#"<c:spPr><a:ln w="bad"/></c:spPr>"#),
        chart_xml("", "<c:spPr><a:prstGeom/></c:spPr>"),
        chart_xml(
            "",
            "<c:spPr><a:solidFill><a:srgbClr/></a:solidFill></c:spPr>",
        ),
        chart_xml(
            "",
            r#"<c:spPr xmlns:a=""><a:ln w="12700"/></c:spPr>"#,
        ),
        chart_xml("", "<c:txPr><a:bodyPr/><a:p><a:r><a:t>unfinished"),
        chart_xml("", "").replace("</c:lineChart>", r#"<c:ser><c:idx val="0"/><c:order val="0"/><c:val><c:numRef><c:f>Sheet1!A1 &bogus;</c:f></c:numRef></c:val></c:ser></c:lineChart>"#),
        chart_xml("", "").replace("</c:lineChart>", r#"<c:ser><c:idx val="0"/><c:order val="0"/><c:dLbls><c:separator>&bogus;</c:separator></c:dLbls></c:ser></c:lineChart>"#),
    ];
    for xml in invalid {
        let source = package(&xml);
        let eager = std::panic::catch_unwind(|| {
            reader::xlsx::read_reader(Cursor::new(source.clone()), true)
        });
        assert!(eager.is_ok(), "eager parser panicked: {xml}");
        assert!(eager.unwrap().is_err(), "malformed chart accepted: {xml}");
        let mut lazy = reader::xlsx::read_reader(Cursor::new(source), false).unwrap();
        for _ in 0..2 {
            let attempt = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                lazy.try_read_sheet(0).map(|_| ())
            }));
            assert!(attempt.is_ok(), "lazy parser panicked: {xml}");
            assert!(attempt.unwrap().is_err(), "malformed chart accepted: {xml}");
            assert!(lazy.try_sheet_collection().is_err());
            assert_eq!(fixture::chart_xml(&lazy), xml);
        }
    }
}

#[test]
fn foreign_shape_children_do_not_change_native_properties() {
    let native = r#"<n:ln w="25400"/>"#;
    for foreign in [
        r#"<a:ln w="12700"/>"#,
        r#"<a:ln w="bad"><n:ln w="12700"/></a:ln>"#,
        r#"<a:future><n:ln w="bad"/></a:future>"#,
    ] {
        for (content, expected_content) in [
            (foreign.to_owned(), ""),
            (format!("{foreign}{native}"), native),
            (format!("{native}{foreign}"), native),
        ] {
            let shape = |content: &str| {
                format!(r#"<c:spPr xmlns:a="urn:foreign" xmlns:n="{A}">{content}</c:spPr>"#)
            };
            let source = package(&chart_xml("", &shape(&content)));
            let expected = imported(&chart_xml("", &shape(expected_content)));
            let expected_xml = fixture::chart_xml(&expected);
            for eager in [true, false] {
                let mut book = reader::xlsx::read_reader(Cursor::new(&source), eager).unwrap();
                book.try_read_sheet(0).unwrap();
                let chart = &book.sheet(0).unwrap().chart_collection()[0];
                assert_eq!(
                    chart
                        .chart_space()
                        .shape_properties()
                        .unwrap()
                        .outline()
                        .is_some(),
                    !expected_content.is_empty()
                );
                #[cfg(feature = "serde")]
                assert_eq!(
                    serde_json::to_value(chart).unwrap(),
                    serde_json::to_value(&expected.sheet(0).unwrap().chart_collection()[0])
                        .unwrap()
                );
                assert_eq!(fixture::chart_xml(&book), expected_xml);
                let reloaded =
                    reader::xlsx::read_reader(Cursor::new(fixture::write_workbook(&book)), true)
                        .unwrap();
                assert_eq!(fixture::chart_xml(&reloaded), expected_xml);
            }
        }
    }
}

#[test]
fn standard_shared_properties_accept_alias_and_default_namespaces() {
    let xml = chart_xml(
        "",
        r#"<c:spPr><a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:solidFill><a:srgbClr val="123456"/></a:solidFill><a:ln w="12700"/></c:spPr>"#,
    );
    let expected = fixture::chart_xml(&imported(&xml));
    let alias = xml
        .replace("xmlns:c=", "xmlns:k=")
        .replace("c:", "k:")
        .replace("xmlns:a=", "xmlns:d=")
        .replace("a:", "d:");
    assert_eq!(fixture::chart_xml(&imported(&alias)), expected);
    let default = xml
        .replace("xmlns:c=", "xmlns=")
        .replace("<c:", "<")
        .replace("</c:", "</");
    assert_eq!(fixture::chart_xml(&imported(&default)), expected);
}

#[test]
fn standard_shared_properties_preserve_empty_and_expanded_forms() {
    let empty = chart_xml(
        "",
        r#"<c:spPr><a:prstGeom prst="rect"/><a:solidFill><a:srgbClr val="123456"/></a:solidFill><a:ln w="12700"/></c:spPr>"#,
    );
    let expanded = chart_xml(
        "",
        r#"<c:spPr><a:prstGeom prst="rect"></a:prstGeom><a:solidFill><a:srgbClr val="123456"></a:srgbClr></a:solidFill><a:ln w="12700"></a:ln></c:spPr>"#,
    );
    assert_eq!(
        fixture::chart_xml(&imported(&empty)),
        fixture::chart_xml(&imported(&expanded))
    );
}

#[test]
fn chart_space_booleans_use_their_schema_default_in_both_forms() {
    let empty = chart_xml("<c:date1904/><c:roundedCorners/>", "");
    let expanded = chart_xml(
        "<c:date1904></c:date1904><c:roundedCorners></c:roundedCorners>",
        "",
    );
    let exported = fixture::chart_xml(&imported(&empty));
    assert!(exported.contains(r#"<c:date1904 val="1"/>"#));
    assert!(exported.contains(r#"<c:roundedCorners val="1"/>"#));
    assert_eq!(fixture::chart_xml(&imported(&expanded)), exported);
}

#[test]
fn invalid_chart_enums_return_errors_without_committing_lazy_sheets() {
    let axis = |value: &str| {
        chart_xml("", "").replace(
            "</c:plotArea>",
            &format!(r#"<c:valAx><c:axId val="1"/><c:axPos val="{value}"/><c:crossAx val="2"/></c:valAx></c:plotArea>"#),
        )
    };
    let invalid = [
        axis("not-an-axis-position"),
        axis("\u{a0}l"),
        chart_xml("", "").replace(r#"grouping val="standard""#, r#"grouping val="not-a-grouping""#),
        chart_xml("", "").replace("</c:chart>", r#"<c:legend><c:legendPos val="not-a-position"/></c:legend></c:chart>"#),
        chart_xml("", "").replace("</c:lineChart>", r#"<c:ser><c:idx val="0"/><c:order val="0"/><c:marker><c:symbol val="not-a-marker"/></c:marker></c:ser></c:lineChart>"#),
    ];
    for xml in invalid {
        let source = package(&xml);
        let eager = std::panic::catch_unwind(|| {
            reader::xlsx::read_reader(Cursor::new(source.clone()), true)
        });
        assert!(eager.is_ok(), "invalid enum panicked: {xml}");
        assert!(eager.unwrap().is_err(), "invalid enum was accepted: {xml}");
        let mut lazy = reader::xlsx::read_reader(Cursor::new(source), false).unwrap();
        for _ in 0..2 {
            let attempt = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                lazy.try_read_sheet(0).map(|_| ())
            }));
            assert!(attempt.is_ok(), "invalid enum panicked: {xml}");
            assert!(
                attempt.unwrap().is_err(),
                "invalid enum was accepted: {xml}"
            );
            assert!(lazy.try_sheet_collection().is_err());
            assert_eq!(fixture::chart_xml(&lazy), xml);
        }
    }
}

#[test]
fn chart_enum_imports_accept_xml_whitespace_and_namespace_aliases() {
    let xml = chart_xml("", "").replace(
        "</c:plotArea>",
        r#"<c:valAx><c:axId val="1"/><c:axPos val=" &#x9;l&#xA; "/><c:crossAx val="2"/></c:valAx></c:plotArea>"#,
    );
    let canonical = fixture::chart_xml(&imported(&xml));
    assert!(canonical.contains(r#"<c:axPos val="l"/>"#));
    let aliased = xml.replace("xmlns:c=", "xmlns:e=").replace("c:", "e:");
    assert_eq!(fixture::chart_xml(&imported(&aliased)), canonical);
    let default = xml
        .replace("xmlns:c=", "xmlns=")
        .replace("<c:", "<")
        .replace("</c:", "</");
    assert_eq!(fixture::chart_xml(&imported(&default)), canonical);
}

#[test]
fn optional_layout_enum_defaults_remain_absent_in_the_source_model() {
    let xml = chart_xml("", "").replace(
        "<c:chart>",
        "<c:chart><c:title><c:layout><c:manualLayout><c:xMode/><c:wMode/></c:manualLayout></c:\
         layout></c:title>",
    );
    let exported = fixture::chart_xml(&imported(&xml));
    assert!(exported.contains("<c:xMode/>"));
    assert!(exported.contains("<c:wMode/>"));
}

#[test]
fn plain_numeric_chart_imports_reject_bad_or_overflowing_values() {
    let line = r#"<c:lineChart><c:grouping val="standard"/><c:axId val="1"/><c:axId val="2"/></c:lineChart>"#;
    let invalid = [
        chart_xml("", "").replace(r#"<c:axId val="1"/>"#, r#"<c:axId val="not-an-id"/>"#),
        chart_xml("", "").replace(r#"<c:axId val="1"/>"#, r#"<c:axId val="4294967296"/>"#),
        chart_xml("", "").replace(line, r#"<c:pieChart><c:firstSliceAng val="361"/></c:pieChart>"#),
        chart_xml("", "").replace("<c:plotArea>", r#"<c:view3D><c:rotX val="91"/></c:view3D><c:plotArea>"#),
        chart_xml("", "").replace("<c:plotArea>", r#"<c:view3D><c:rotY val="361"/></c:view3D><c:plotArea>"#),
        chart_xml("", "").replace("<c:plotArea>", r#"<c:view3D><c:perspective val="241"/></c:view3D><c:plotArea>"#),
        chart_xml("", r#"<c:printSettings><c:pageMargins b="not-a-number" l="0" r="0" t="0" header="0" footer="0"/></c:printSettings>"#),
        chart_xml("", "").replace(line, r#"<c:barChart><c:barDir val="col"/><c:grouping val="clustered"/><c:ser><c:idx val="0"/><c:order val="0"/><c:invertIfNegative val="not-a-boolean"/></c:ser><c:axId val="1"/><c:axId val="2"/></c:barChart>"#),
    ];
    for xml in invalid {
        let source = package(&xml);
        let eager = std::panic::catch_unwind(|| {
            reader::xlsx::read_reader(Cursor::new(source.clone()), true)
        });
        assert!(eager.is_ok(), "invalid numeric input panicked: {xml}");
        assert!(
            eager.unwrap().is_err(),
            "invalid numeric input accepted: {xml}"
        );
        let mut lazy = reader::xlsx::read_reader(Cursor::new(source), false).unwrap();
        for _ in 0..2 {
            let attempt = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                lazy.try_read_sheet(0).map(|_| ())
            }));
            assert!(attempt.is_ok(), "invalid numeric input panicked: {xml}");
            assert!(
                attempt.unwrap().is_err(),
                "invalid numeric input accepted: {xml}"
            );
            assert!(lazy.try_sheet_collection().is_err());
            assert_eq!(fixture::chart_xml(&lazy), xml);
        }
    }
}

#[test]
fn typed_chart_imports_use_their_actual_optional_defaults() {
    let line = r#"<c:lineChart><c:grouping val="standard"/><c:axId val="1"/><c:axId val="2"/></c:lineChart>"#;
    let xml = chart_xml("", "").replace(line,
        r#"<c:barChart><c:barDir/><c:grouping/><c:ser><c:idx val="0"/><c:order val="0"/><c:invertIfNegative/></c:ser><c:axId val="1"/><c:axId val="2"/></c:barChart>"#,
    ).replace("</c:chart>", "<c:legend><c:legendPos/></c:legend></c:chart>");
    let exported = fixture::chart_xml(&imported(&xml));
    for expected in [
        r#"<c:barDir val="col"/>"#,
        r#"<c:grouping val="clustered"/>"#,
        r#"<c:invertIfNegative val="1"/>"#,
        r#"<c:legendPos val="r"/>"#,
    ] {
        assert!(exported.contains(expected), "{expected}: {exported}");
    }
    let xml = chart_xml("", "").replace(
        "<c:plotArea>",
        "<c:view3D><c:rotX/><c:rotY/><c:perspective/></c:view3D><c:plotArea>",
    );
    let exported = fixture::chart_xml(&imported(&xml));
    for expected in [
        r#"<c:rotX val="0"/>"#,
        r#"<c:rotY val="0"/>"#,
        r#"<c:perspective val="30"/>"#,
    ] {
        assert!(exported.contains(expected), "{expected}: {exported}");
    }
}

#[test]
fn plain_numeric_chart_imports_accept_legal_boundaries_and_xml_whitespace() {
    let xml = chart_xml("", r#"<c:printSettings><c:pageMargins b="1.25" l="-2.5" r="0" t="0" header="0" footer="0"/></c:printSettings>"#)
        .replace(r#"<c:axId val="1"/>"#, r#"<c:axId val=" &#x9;4294967295&#xA; "/>"#)
        .replace("<c:plotArea>", r#"<c:view3D><c:rotX val="-90"/><c:rotY val="360"/><c:perspective val="240"/></c:view3D><c:plotArea>"#);
    let exported = fixture::chart_xml(&imported(&xml));
    for expected in [
        r#"<c:axId val="4294967295"/>"#,
        r#"<c:rotX val="-90"/>"#,
        r#"<c:rotY val="360"/>"#,
        r#"<c:perspective val="240"/>"#,
    ] {
        assert!(exported.contains(expected), "{expected}: {exported}");
    }
}

#[test]
fn page_margins_preserve_xml_double_special_values() {
    let xml = chart_xml(
        "",
        r#"<c:printSettings><c:pageMargins b="INF" l="-INF" r="NaN" t="1E2" header=".5" footer="-0"/></c:printSettings>"#,
    );
    let exported = fixture::chart_xml(&imported(&xml));
    for expected in [
        r#"b="INF""#,
        r#"l="-INF""#,
        r#"r="NaN""#,
        r#"t="100""#,
        r#"header="0.5""#,
    ] {
        assert!(exported.contains(expected), "{expected}: {exported}");
    }
}
