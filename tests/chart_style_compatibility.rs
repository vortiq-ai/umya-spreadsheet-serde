#![cfg(feature = "serde")]

#[path = "support/chart_xml_fixture.rs"]
pub mod fixture;

fn workbook(style: u16) -> umya_spreadsheet::Workbook {
    let xml = format!(
        "<c:chartSpace xmlns:c=\"http://schemas.openxmlformats.org/drawingml/2006/chart\">\
         <mc:AlternateContent xmlns:mc=\"http://schemas.openxmlformats.org/markup-compatibility/2006\">\
         <mc:Choice xmlns:c14=\"http://schemas.microsoft.com/office/drawing/2007/8/2/chart\" Requires=\"c14\">\
         <c14:style val=\"{}\"/></mc:Choice><mc:Fallback><c:style val=\"{style}\"/>\
         </mc:Fallback></mc:AlternateContent><c:chart><c:plotArea><c:pieChart>\
         <c:ser><c:idx val=\"1\"/><c:order val=\"1\"/></c:ser></c:pieChart>\
         </c:plotArea></c:chart></c:chartSpace>",
        style + 100
    );
    fixture::imported_workbook(&xml)
}

#[test]
fn modern_chart_styles_follow_each_imported_fallback_value() {
    for style in [1, 2, 26, 48] {
        let book = workbook(style);
        let xml = fixture::chart_xml(&book);
        assert!(
            xml.contains(&format!("<c14:style val=\"{}\"", style + 100)),
            "{xml}"
        );
        assert!(xml.contains(&format!("<c:style val=\"{style}\"")), "{xml}");
    }
}

#[test]
fn editing_the_fallback_style_keeps_the_modern_choice_consistent() {
    let mut book = workbook(2);
    book.sheet_mut(0).unwrap().chart_collection_mut()[0]
        .chart_space_mut()
        .style_mut()
        .unwrap()
        .set_val("26");
    let xml = fixture::chart_xml(&book);
    assert!(xml.contains("<c14:style val=\"126\""), "{xml}");
    assert!(xml.contains("<c:style val=\"26\""), "{xml}");
}

#[test]
fn missing_fallback_style_value_stays_invalid_without_panicking() {
    let xml = fixture::chart_xml(&workbook(26));
    let xml = xml.replace("<c:style val=\"26\"", "<c:style");
    assert!(xml.contains("<c:style/>"), "{xml}");
    let book = fixture::imported_workbook(&xml);
    let style = book.sheet(0).unwrap().chart_collection()[0]
        .chart_space()
        .style()
        .unwrap();
    assert_eq!(style.val(), "");
    assert!(serde_json::to_value(style).unwrap()["val"]["value"].is_null());
    let output = fixture::chart_xml(&book);
    assert!(output.contains("<c:style val=\"\""), "{output}");
    assert!(!output.contains("<c:style val=\"2\""), "{output}");
}

#[test]
fn invalid_fallback_style_lexeme_is_not_replaced_with_a_valid_default() {
    for value in ["invalid", ""] {
        let xml = fixture::chart_xml(&workbook(26));
        let xml = xml.replace("<c:style val=\"26\"", &format!("<c:style val=\"{value}\""));
        let book = fixture::imported_workbook(&xml);
        let style = book.sheet(0).unwrap().chart_collection()[0]
            .chart_space()
            .style()
            .unwrap();
        assert_eq!(style.val(), value);
        assert_eq!(serde_json::to_value(style).unwrap()["val"]["value"], value);
        let output = fixture::chart_xml(&book);
        assert!(
            output.contains(&format!("<c:style val=\"{value}\"")),
            "{output}"
        );
    }
}
