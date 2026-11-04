#![cfg(feature = "serde")]
//! Public compatibility checks for the native mutable formula projection.
use umya_spreadsheet::{
    Chart,
    drawing::charts::{
        AreaChartSeries,
        CategoryAxisData,
        Chart as ChartModel,
        ChartText,
        Formula,
        NumericData,
        NumericPoint,
        StringReference,
        Title,
        XValues,
        XmlUnsignedInt,
    },
};

#[path = "support/chart_xml_fixture.rs"]
mod fixture;

fn hidden_extension(marker: &str) -> String {
    format!(
        "<c:extLst><c:ext uri=\"urn:hidden\"><h:part \
         xmlns:h=\"urn:hidden\">{marker}</h:part></c:ext></c:extLst>"
    )
}

fn scatter_xml(x_extension: &str, y_extension: &str) -> String {
    format!(
        concat!(
            "<c:chartSpace xmlns:c=\"http://schemas.openxmlformats.org/drawingml/2006/chart\" ",
            "xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\">",
            "<c:chart><c:plotArea><c:layout/><c:scatterChart><c:scatterStyle val=\"lineMarker\"/>",
            "<c:ser><c:idx val=\"0\"/><c:order val=\"0\"/>",
            "<c:xVal><c:numRef><c:f>Sheet1!$A$1:$A$2</c:f>{x_extension}</c:numRef></c:xVal>",
            "<c:yVal><c:numRef><c:f>Sheet1!$B$1:$B$2</c:f>{y_extension}</c:numRef></c:yVal></c:\
             ser>",
            "<c:axId val=\"1\"/><c:axId val=\"2\"/></c:scatterChart>",
            "<c:valAx><c:axId val=\"1\"/><c:scaling><c:orientation val=\"minMax\"/></c:scaling>",
            "<c:delete val=\"0\"/><c:axPos val=\"b\"/><c:crossAx val=\"2\"/></c:valAx>",
            "<c:valAx><c:axId val=\"2\"/><c:scaling><c:orientation val=\"minMax\"/></c:scaling>",
            "<c:delete val=\"0\"/><c:axPos val=\"l\"/><c:crossAx val=\"1\"/></c:valAx>",
            "</c:plotArea></c:chart></c:chartSpace>"
        ),
        x_extension = x_extension,
        y_extension = y_extension,
    )
}

fn first_series(chart: &mut Chart) -> &mut AreaChartSeries {
    chart
        .chart_space_mut()
        .chart_mut()
        .plot_area_mut()
        .chart_groups_mut()
        .next()
        .unwrap()
        .series_mut()
        .area_chart_series_mut()
        .first_mut()
        .unwrap()
}

fn numeric_literal(values: &[&str]) -> NumericData {
    let mut count = XmlUnsignedInt::default();
    count.set_val(values.len() as u32);
    let mut data = NumericData::default();
    data.set_point_count(Some(count));
    for (index, value) in values.iter().enumerate() {
        let mut point = NumericPoint::default();
        point.index_mut().set_val(index as u32);
        point.set_value(Some((*value).to_owned()));
        data.points_mut().push(point);
    }
    data
}

fn write_and_reread(chart: Chart, basis: &Chart) -> (String, Chart) {
    let projection = serde_json::to_value(&chart).unwrap();
    let mut chart: Chart = serde_json::from_value(projection).unwrap();
    chart.restore_export_parts(basis).unwrap();
    let mut book = fixture::imported_workbook(&scatter_xml("", ""));
    book.sheet_mut(0).unwrap().chart_collection_mut()[0] = chart;
    let bytes = fixture::write_workbook(&book);
    let xml = fixture::chart_xml(&book);
    let reread =
        umya_spreadsheet::reader::xlsx::read_reader(std::io::Cursor::new(bytes), true).unwrap();
    let reread_chart = reread.sheet(0).unwrap().chart_collection()[0].clone();
    (xml, reread_chart)
}

#[test]
fn title_formula_projection_is_live_without_materializing_absent_owners() {
    let mut formula = Formula::default();
    formula.set_address_str("Sheet1!$A$1");
    let mut reference = StringReference::default();
    reference.set_formula(formula);
    let mut text = ChartText::default();
    text.set_string_reference(reference);
    let mut title = Title::default();
    title.set_chart_text(text);
    let mut chart = ChartModel::default();
    chart.set_title(title);
    let before = serde_json::to_value(&chart).unwrap();
    {
        let references = chart.formula_mut();
        assert_eq!(references.len(), 1);
        assert_eq!(references[0].address_str(), "Sheet1!$A$1");
    }
    assert_eq!(serde_json::to_value(&chart).unwrap(), before);
    chart
        .formula_mut()
        .remove(0)
        .set_string_value("exact decided formula text");
    assert_eq!(
        chart
            .title()
            .unwrap()
            .chart_text()
            .unwrap()
            .string_reference()
            .unwrap()
            .formula()
            .address_str(),
        "exact decided formula text",
    );
    assert!(chart.pivot_formats().is_none());
}

#[test]
fn imported_series_sources_drop_their_hidden_extensions() {
    let mut chart = fixture::imported_chart(&fixture::all_owner_xml());
    let series = chart
        .chart_space()
        .chart()
        .plot_area()
        .chart_groups()
        .next()
        .unwrap()
        .series()
        .area_chart_series()[0]
        .clone();
    assert!(
        series
            .category_axis_data()
            .unwrap()
            .string_reference()
            .unwrap()
            .extensions()
            .is_some()
    );
    assert!(
        series
            .values()
            .unwrap()
            .number_reference()
            .extensions()
            .is_some()
    );

    chart.strip_export_parts();
    let series = chart
        .chart_space()
        .chart()
        .plot_area()
        .chart_groups()
        .next()
        .unwrap()
        .series()
        .area_chart_series()[0]
        .clone();
    assert!(
        series
            .category_axis_data()
            .unwrap()
            .string_reference()
            .unwrap()
            .extensions()
            .is_none()
    );
    assert!(
        series
            .values()
            .unwrap()
            .number_reference()
            .extensions()
            .is_none()
    );
}

#[test]
fn replacing_x_values_with_text_strips_the_dormant_numeric_reference_extension() {
    let marker = "DORMANT_X_NUMERIC_REFERENCE";
    let book = fixture::imported_workbook(&scatter_xml(&hidden_extension(marker), ""));
    let mut chart = book.sheet(0).unwrap().chart_collection()[0].clone();
    assert!(serde_json::to_string(&chart).unwrap().contains(marker));
    let basis = chart.clone();
    let mut reference = StringReference::default();
    reference.formula_mut().set_address_str("Sheet1!$A$1:$A$2");
    let mut text_source = CategoryAxisData::default();
    text_source.set_string_reference(reference);
    first_series(&mut chart)
        .x_values_mut()
        .unwrap()
        .set_text_source(text_source);

    chart.strip_export_parts();
    let projected = serde_json::to_string(&chart).unwrap();
    assert!(!projected.contains(marker));
    let (xml, reread) = write_and_reread(chart, &basis);
    assert!(!xml.contains(marker));
    assert!(xml.contains("<c:xVal><c:strRef>"));
    assert!(
        reread
            .chart_space()
            .chart()
            .plot_area()
            .chart_groups()
            .next()
            .unwrap()
            .series()
            .area_chart_series()[0]
            .x_values()
            .unwrap()
            .text_source()
            .is_some()
    );
}

#[test]
fn serde_supplied_dormant_x_sources_are_sanitized_too() {
    let marker = "SERDE_DORMANT_X_REFERENCE";
    let book = fixture::imported_workbook(&scatter_xml(&hidden_extension(marker), ""));
    let mut chart = book.sheet(0).unwrap().chart_collection()[0].clone();
    assert!(serde_json::to_string(&chart).unwrap().contains(marker));
    let basis = chart.clone();
    let original_x = first_series(&mut chart).x_values().unwrap().clone();
    let reference_json = serde_json::to_value(original_x.number_reference()).unwrap();
    let mut literal_json = serde_json::to_value(numeric_literal(&["1", "2"])).unwrap();
    literal_json["extensions"] = reference_json["extensions"].clone();
    let literal_for_category = literal_json.clone();
    let mut text_reference = StringReference::default();
    text_reference
        .formula_mut()
        .set_address_str("Sheet1!$A$1:$A$2");
    let mut text_source = CategoryAxisData::default();
    text_source.set_string_reference(text_reference);
    let injected: XValues = serde_json::from_value(serde_json::json!({
        "number_reference": reference_json.clone(),
        "number_literal": literal_json,
        "text_source": serde_json::to_value(text_source).unwrap(),
    }))
    .unwrap();
    let series = first_series(&mut chart);
    series.set_x_values(injected);
    let mut category_string = StringReference::default();
    category_string
        .formula_mut()
        .set_address_str("Sheet1!$A$1:$A$2");
    let mut category_string_json = serde_json::to_value(category_string).unwrap();
    category_string_json["extensions"] = reference_json["extensions"].clone();
    let category_source: CategoryAxisData = serde_json::from_value(serde_json::json!({
        "string_reference": category_string_json,
        "string_literal": null,
        "number_reference": reference_json,
        "multi_level_string_reference": null,
        "number_literal": literal_for_category,
    }))
    .unwrap();
    series.set_category_axis_data(category_source);

    chart.strip_export_parts();
    let projected = serde_json::to_string(&chart).unwrap();
    assert!(!projected.contains(marker));
    let (xml, reread) = write_and_reread(chart, &basis);
    assert!(!xml.contains(marker));
    assert!(
        reread
            .chart_space()
            .chart()
            .plot_area()
            .chart_groups()
            .next()
            .unwrap()
            .series()
            .area_chart_series()[0]
            .x_values()
            .unwrap()
            .text_source()
            .is_some()
    );
}
