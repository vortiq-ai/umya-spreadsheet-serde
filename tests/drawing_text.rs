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
fn drawing_text_preserves_whitespace_and_decodes_entities_once() {
    let source = document("<a:bodyPr/><a:p><a:r><a:t>  a &amp;amp; b\t c  </a:t></a:r></a:p>");
    let written = output(&source);
    assert!(
        written.contains(">  a &amp;amp; b\t c  </a:t>"),
        "{written}"
    );
    assert_eq!(output(&written), written);
}

#[test]
fn drawing_text_uses_namespace_bindings_not_prefix_spelling() {
    let body = "<a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr b=\"1\"/><a:t>value</a:t></a:r></a:p>";
    let original = output(&document(body));
    let aliased = document(body)
        .replace("xmlns:a=", "xmlns:d=")
        .replace("<a:", "<d:")
        .replace("</a:", "</d:");
    assert_eq!(output(&aliased), original);
    let defaulted = document(body)
        .replace(&format!("xmlns:a=\"{A}\""), &format!("xmlns=\"{A}\""))
        .replace("<a:", "<")
        .replace("</a:", "</");
    assert_eq!(output(&defaulted), original);
}

#[test]
fn drawing_text_empty_and_expanded_properties_match() {
    let empty = document(
        "<a:bodyPr/><a:lstStyle/><a:p><a:pPr/><a:r><a:rPr/><a:t/></a:r><a:endParaRPr/></a:p>",
    );
    let expanded = empty
        .replace("<a:bodyPr/>", "<a:bodyPr></a:bodyPr>")
        .replace("<a:lstStyle/>", "<a:lstStyle></a:lstStyle>")
        .replace("<a:pPr/>", "<a:pPr></a:pPr>")
        .replace("<a:rPr/>", "<a:rPr></a:rPr>")
        .replace("<a:t/>", "<a:t></a:t>")
        .replace("<a:endParaRPr/>", "<a:endParaRPr></a:endParaRPr>");
    assert_eq!(output(&empty), output(&expanded));
}

#[test]
fn malformed_drawing_text_returns_errors_without_panicking() {
    for body in [
        "<a:bodyPr/><a:p><a:r><a:t>unterminated",
        "<a:bodyPr/><a:p><a:r><a:t><a:r/></a:t></a:r></a:p>",
        "<a:bodyPr/><a:p><a:r><a:rPr sz=\"bad\"/><a:t>text</a:t></a:r></a:p>",
        "<a:bodyPr/><a:p><a:r><a:rPr b=\"1\" b=\"0\"/></a:r></a:p>",
        "<a:bodyPr/><a:p><a:r><a:rPr lang=\"&missing;\"/></a:r></a:p>",
        "<a:bodyPr/><a:p><a:fld><a:t>field</a:t></a:fld></a:p>",
        "<a:bodyPr/><a:p><a:r><a:t xml:space=\"preserve\" \
         xml:space=\"default\">text</a:t></a:r></a:p>",
        "<a:bodyPr/><a:p><a:fld id=\"{11111111-1111-1111-1111-111111111111}\"><a:t \
         bad=\"&missing;\">field</a:t></a:fld></a:p>",
    ] {
        let xml = document(body);
        let parsed = std::panic::catch_unwind(|| ChartExSpace::from_xml(xml.as_bytes()));
        assert!(parsed.is_ok(), "reader panicked: {body}");
        assert!(parsed.unwrap().is_err(), "invalid text accepted: {body}");
    }
}

#[test]
fn native_point_spacing_roundtrips_and_enforces_the_choice() {
    let body = "<a:bodyPr/><a:lstStyle><a:lvl1pPr><a:spcBef><a:spcPts \
                val=\"1400\"/></a:spcBef><a:spcAft><a:spcPts \
                val=\"600\"/></a:spcAft></a:lvl1pPr></a:lstStyle><a:p><a:pPr><a:lnSpc><a:spcPts \
                val=\"1200\"/></a:lnSpc><a:spcBef><a:spcPts \
                val=\"0\"/></a:spcBef><a:spcAft><a:spcPct \
                val=\"100000\"/></a:spcAft></a:pPr><a:r><a:t>value</a:t></a:r></a:p>";
    let written = output(&document(body));
    for value in ["1400", "600", "1200", "0"] {
        assert!(
            written.contains(&format!("<a:spcPts val=\"{value}\"")),
            "{written}"
        );
    }
    assert_eq!(output(&written), written);
    for value in ["-1", "158401", "bad"] {
        let invalid = document(&body.replace("val=\"1200\"", &format!("val=\"{value}\"")));
        assert!(ChartExSpace::from_xml(invalid.as_bytes()).is_err());
    }
    let conflicting = document(&body.replace(
        "<a:spcPts val=\"1200\"/>",
        "<a:spcPts val=\"1200\"/><a:spcPct val=\"100000\"/>",
    ));
    assert!(ChartExSpace::from_xml(conflicting.as_bytes()).is_err());
    let missing = document(&body.replace("<a:spcPts val=\"1200\"/>", ""));
    assert!(ChartExSpace::from_xml(missing.as_bytes()).is_err());
}

#[test]
fn editing_point_spacing_replaces_percentage_without_losing_valid_zero() {
    for value in [0, 158400] {
        let mut line = umya_spreadsheet::drawing::LineSpacing::default();
        line.set_spacing_percent(umya_spreadsheet::drawing::SpacingPercent::default());
        assert!(line.set_spacing_points(value));
        assert_eq!(line.spacing_points(), Some(value));
        assert!(line.spacing_percent().is_none());
        assert!(line.is_valid());
        assert!(!line.set_spacing_points(-1));
        assert!(!line.set_spacing_points(158401));
        assert_eq!(line.spacing_points(), Some(value));
        line.set_spacing_percent(umya_spreadsheet::drawing::SpacingPercent::default());
        assert!(line.spacing_points().is_none());
    }
}
