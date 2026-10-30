use umya_spreadsheet::drawing::charts::*;

struct NoReferences;
impl ChartExDataResolver for NoReferences {
    fn text_value(&mut self, _: &str, _: &ChartExFormula) -> Result<Option<String>, String> {
        Err("unexpected reference".into())
    }

    fn string_levels(
        &mut self,
        _: &str,
        _: &ChartExStringReference,
    ) -> Result<Vec<ChartExStringLevel>, String> {
        Err("unexpected reference".into())
    }

    fn numeric_levels(
        &mut self,
        _: &str,
        _: &ChartExNumericReference,
    ) -> Result<Vec<ChartExNumericLevel>, String> {
        Err("unexpected reference".into())
    }
}

const CX: &str = "http://schemas.microsoft.com/office/drawing/2014/chartex";
const A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";

fn document(body: &str) -> String {
    format!(
        "<cx:chartSpace xmlns:cx=\"{CX}\" xmlns:a=\"{A}\"><cx:chartData><cx:data \
         id=\"7\"><cx:strDim type=\"cat\"><cx:lvl ptCount=\"1\"><cx:pt \
         idx=\"0\">category</cx:pt></cx:lvl></cx:strDim><cx:numDim type=\"val\"><cx:lvl \
         ptCount=\"1\"><cx:pt \
         idx=\"0\">1</cx:pt></cx:lvl></cx:numDim></cx:data></cx:chartData><cx:chart><cx:\
         plotArea><cx:plotAreaRegion><cx:series layoutId=\"waterfall\"><cx:dataId \
         val=\"7\"/></cx:series></cx:plotAreaRegion></cx:plotArea></cx:chart><cx:txPr>{body}</cx:\
         txPr></cx:chartSpace>"
    )
}

fn output(xml: &str) -> String {
    let chart = ChartExSpace::from_xml(xml.as_bytes()).unwrap();
    String::from_utf8(chart.to_xml("Sheet1", &mut NoReferences).unwrap()).unwrap()
}

#[test]
fn text_autofit_choices_roundtrip() {
    for fit in [
        "<a:noAutofit/>",
        "<a:normAutofit/>",
        "<a:normAutofit fontScale=\"85000\" lnSpcReduction=\"12000\"/>",
        "<a:normAutofit fontScale=\"85.125%\" lnSpcReduction=\"12.5%\"/>",
        "<a:spAutoFit/>",
    ] {
        let xml = document(&format!("<a:bodyPr>{fit}</a:bodyPr><a:p/>"));
        let written = output(&xml);
        assert!(written.contains(fit), "{written}");
        assert_eq!(output(&written), written);
    }
}

#[test]
fn text_autofit_rejects_conflicts_and_malformed_values() {
    for fit in [
        "<a:noAutofit/><a:noAutofit/>",
        "<a:noAutofit/><a:spAutoFit/>",
        "<a:normAutofit/><a:noAutofit/>",
        "<a:normAutofit fontScale=\"999\"/>",
        "<a:normAutofit fontScale=\"100001\"/>",
        "<a:normAutofit lnSpcReduction=\"-1\"/>",
        "<a:normAutofit lnSpcReduction=\"13200001\"/>",
        "<a:normAutofit fontScale=\"85.%\"/>",
        "<a:normAutofit fontScale=\"+85%\"/>",
        "<a:normAutofit fontScale=\"NaN\"/>",
        "<a:normAutofit fontScale=\"85000\" fontScale=\"90000\"/>",
        "<a:noAutofit><a:spAutoFit/></a:noAutofit>",
        "<a:normAutofit><a:spAutoFit/></a:normAutofit>",
    ] {
        let xml = document(&format!("<a:bodyPr>{fit}</a:bodyPr><a:p/>"));
        assert!(ChartExSpace::from_xml(xml.as_bytes()).is_err(), "{fit}");
    }
}

#[test]
fn text_autofit_accepts_percentage_union_and_namespace_variants() {
    for fit in [
        "<a:noAutofit></a:noAutofit>",
        "<a:spAutoFit></a:spAutoFit>",
        "<a:normAutofit fontScale=\"-1.25%\" lnSpcReduction=\"13201%\"></a:normAutofit>",
        "<a:normAutofit fontScale=\"1000\" lnSpcReduction=\"13200000\"/>",
        "<a:normAutofit fontScale=\"100000\" lnSpcReduction=\"0\"/>",
    ] {
        let xml = document(&format!("<a:bodyPr>{fit}</a:bodyPr><a:p/>"));
        let expected = output(&xml);
        let aliased = xml.replace("xmlns:a=", "xmlns:d=").replace("a:", "d:");
        assert_eq!(output(&aliased), expected);
        let defaulted = xml
            .replace(&format!("xmlns:a=\"{A}\""), &format!("xmlns=\"{A}\""))
            .replace("<a:", "<")
            .replace("</a:", "</");
        assert_eq!(output(&defaulted), expected);
        assert_eq!(output(&expected), expected);
    }
}

#[test]
fn editing_text_autofit_replaces_the_previous_choice() {
    use umya_spreadsheet::drawing::{
        NormalAutoFit,
        ShapeAutoFit,
        TextAutoFit,
    };
    let xml = document("<a:bodyPr><a:noAutofit/></a:bodyPr><a:p/>");
    let mut chart = ChartExSpace::from_xml(xml.as_bytes()).unwrap();
    let body = chart
        .text_properties
        .as_mut()
        .unwrap()
        .body_properties_mut();
    assert_eq!(body.auto_fit(), Some(&TextAutoFit::Disabled));
    let mut normal = NormalAutoFit::default();
    assert_eq!(normal.font_scale(), None);
    normal.set_font_scale(Some("85.125%")).unwrap();
    normal.set_line_spacing_reduction(Some("12000")).unwrap();
    assert!(normal.set_font_scale(Some("999")).is_err());
    assert_eq!(normal.font_scale(), Some("85.125%"));
    body.set_auto_fit(Some(TextAutoFit::Normal(normal)));
    assert!(body.shape_auto_fit().is_none());
    let written = String::from_utf8(chart.to_xml("Sheet1", &mut NoReferences).unwrap()).unwrap();
    assert!(written.contains("<a:normAutofit fontScale=\"85.125%\" lnSpcReduction=\"12000\"/>"));
    assert!(!written.contains("noAutofit"));
    assert_eq!(output(&written), written);
    let body = chart
        .text_properties
        .as_mut()
        .unwrap()
        .body_properties_mut();
    body.set_shape_auto_fit(ShapeAutoFit::default());
    assert!(body.shape_auto_fit().is_some());
    let written = String::from_utf8(chart.to_xml("Sheet1", &mut NoReferences).unwrap()).unwrap();
    assert!(written.contains("<a:spAutoFit/>"));
    assert!(!written.contains("normAutofit"));
    chart
        .text_properties
        .as_mut()
        .unwrap()
        .body_properties_mut()
        .set_auto_fit(None);
    let written = String::from_utf8(chart.to_xml("Sheet1", &mut NoReferences).unwrap()).unwrap();
    assert!(!written.contains("AutoFit"));
}

#[cfg(feature = "serde")]
#[test]
fn text_autofit_roundtrips_in_json_and_bincode() {
    use umya_spreadsheet::drawing::NormalAutoFit;
    for fit in [
        "<a:noAutofit/>",
        "<a:normAutofit/>",
        "<a:normAutofit fontScale=\"85.125%\" lnSpcReduction=\"12000\"/>",
        "<a:spAutoFit/>",
    ] {
        let xml = document(&format!("<a:bodyPr>{fit}</a:bodyPr><a:p/>"));
        let chart = ChartExSpace::from_xml(xml.as_bytes()).unwrap();
        let value = serde_json::to_value(&chart).unwrap();
        let json: ChartExSpace = serde_json::from_value(value.clone()).unwrap();
        let binary: ChartExSpace =
            bincode::deserialize(&bincode::serialize(&chart).unwrap()).unwrap();
        assert_eq!(serde_json::to_value(&binary).unwrap(), value);
        for restored in [json, binary] {
            let written =
                String::from_utf8(restored.to_xml("Sheet1", &mut NoReferences).unwrap()).unwrap();
            assert!(written.contains(fit), "{written}");
        }
    }
    let invalid = serde_json::json!({"font_scale":"999", "line_spacing_reduction":null});
    assert!(serde_json::from_value::<NormalAutoFit>(invalid).is_err());
    let invalid = serde_json::json!({"font_scale":null, "line_spacing_reduction":"-1"});
    assert!(serde_json::from_value::<NormalAutoFit>(invalid).is_err());
}
