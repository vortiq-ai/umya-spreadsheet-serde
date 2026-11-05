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

fn invalid_fill() -> umya_spreadsheet::drawing::SolidFill {
    use umya_spreadsheet::drawing::{
        PresetColor,
        SolidFill,
    };
    let mut color = PresetColor::default();
    color.set_val("not-a-preset");
    let mut fill = SolidFill::default();
    fill.set_preset_color(color);
    fill
}

#[test]
fn chart_text_export_returns_native_paint_errors() {
    use umya_spreadsheet::drawing::{
        Paragraph,
        Run,
        RunProperties,
    };
    let mut chart = ChartExSpace::from_xml(document("<a:bodyPr/><a:p/>").as_bytes()).unwrap();
    let mut properties = RunProperties::default();
    properties.set_solid_fill(invalid_fill());
    let mut run = Run::default();
    run.set_text("text");
    *run.run_properties_mut() = properties;
    let mut paragraph = Paragraph::default();
    paragraph.add_run(run);
    *chart.text_properties.as_mut().unwrap().paragraph_mut() = vec![paragraph];
    assert!(chart.to_xml("Sheet1", &mut NoReferences).is_err());
}

#[test]
fn workbook_theme_export_returns_native_paint_errors() {
    let mut workbook = umya_spreadsheet::new_file();
    workbook
        .theme_mut()
        .theme_elements_mut()
        .format_scheme_mut()
        .fill_style_list_mut()
        .add_solid_fill(invalid_fill());
    let result = umya_spreadsheet::writer::xlsx::write_writer(&workbook, Vec::new());
    assert!(result.is_err());
}
