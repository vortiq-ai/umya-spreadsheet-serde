#![cfg(feature = "serde")]

#[path = "support/chart_xml_fixture.rs"]
pub mod fixture;

use std::io::Cursor;

use umya_spreadsheet::{
    Chart,
    drawing::{
        Paragraph,
        PresetColor,
        RunProperties,
        SolidFill,
    },
};

fn field(text: &str) -> String {
    format!("<a:fld id=\"{{01234567-89AB-CDEF-0123-456789ABCDEF}}\"><a:t>{text}</a:t></a:fld>")
}

fn document() -> String {
    format!(
        "<c:chartSpace xmlns:c=\"http://schemas.openxmlformats.org/drawingml/2006/chart\" \
         xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\"><c:chart><c:title>\
         <c:tx><c:rich><a:bodyPr/><a:lstStyle/><a:p>{}<a:r><a:t>North</a:t></a:r>\
         <a:br/>{}<a:br/><a:r><a:t>South</a:t></a:r>{}</a:p></c:rich></c:tx></c:title>\
         <c:plotArea><c:lineChart><c:grouping val=\"standard\"/></c:lineChart></c:plotArea>\
         </c:chart></c:chartSpace>",
        field("Leading"),
        field("Middle"),
        field("Trailing"),
    )
}

fn paragraph_mut(chart: &mut Chart) -> &mut Paragraph {
    &mut chart
        .chart_space_mut()
        .chart_mut()
        .title_mut()
        .unwrap()
        .chart_text_mut()
        .unwrap()
        .rich_text_mut()
        .unwrap()
        .paragraph_mut()[0]
}

#[test]
fn same_run_boundary_breaks_keep_their_order_around_fields_after_restoration() {
    let mut book = fixture::imported_workbook(&document());
    let previous = book.sheet(0).unwrap().chart_collection()[0].clone();
    let mut projected = previous.clone();
    projected.strip_export_parts();
    let paragraph = paragraph_mut(&mut projected);
    assert!(paragraph.fields().is_empty());
    assert_eq!(paragraph.breaks()[0].before_run, 1);
    assert_eq!(paragraph.breaks()[0].before_field, 1);
    assert_eq!(paragraph.breaks()[1].before_run, 1);
    assert_eq!(paragraph.breaks()[1].before_field, 2);
    // The projected field ranks still name the omitted authoritative fields.
    // Only a restored/exportable paragraph can validate them against that list.
    let mut restored: Chart =
        serde_json::from_value(serde_json::to_value(&projected).unwrap()).unwrap();
    restored.restore_export_parts(&previous).unwrap();
    book.sheet_mut(0).unwrap().chart_collection_mut()[0] = restored;
    let bytes = fixture::write_workbook(&book);
    let reread = umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(bytes), true).unwrap();
    let xml = fixture::chart_xml(&reread);
    let positions: Vec<_> = ["Leading", "North", "Middle", "South", "Trailing"]
        .map(|text| xml.find(&format!(">{text}</a:t>")).unwrap())
        .into();
    assert!(positions.windows(2).all(|pair| pair[0] < pair[1]));
    assert_eq!(xml.matches("<a:br").count(), 2);
    assert!(xml[positions[1]..positions[2]].contains("<a:br"));
    assert!(xml[positions[2]..positions[3]].contains("<a:br"));
    let mut chart = reread.sheet(0).unwrap().chart_collection()[0].clone();
    let paragraph = paragraph_mut(&mut chart);
    assert_eq!(paragraph.fields().len(), 3);
    assert_eq!(paragraph.breaks()[0].before_field, 1);
    assert_eq!(paragraph.breaks()[1].before_field, 2);
}

#[test]
fn serialized_break_field_ranks_outside_the_run_boundary_fail_export() {
    for (index, field_rank) in [(0, 0), (1, 3), (1, u32::MAX)] {
        let mut book = fixture::imported_workbook(&document());
        let mut json = serde_json::to_value(&book.sheet(0).unwrap().chart_collection()[0]).unwrap();
        let paragraph = &mut json["two_cell_anchor"]["graphic_frame"]["graphic"]["graphic_data"]
            ["chart_space"]["chart"]["title"]["chart_text"]["rich_text"]["paragraph"][0];
        assert_eq!(paragraph["breaks"][index]["before_run"], 1);
        paragraph["breaks"][index]["before_field"] = field_rank.into();
        book.sheet_mut(0).unwrap().chart_collection_mut()[0] =
            serde_json::from_value(json).unwrap();
        let result = umya_spreadsheet::writer::xlsx::write_writer(&book, Cursor::new(Vec::new()));
        assert!(
            result.is_err(),
            "field rank {field_rank} at run boundary 1 must not be silently relocated"
        );
    }
}

#[test]
fn native_break_field_ranks_outside_the_run_boundary_fail_export() {
    for (index, field_rank) in [(0, 0), (1, 3), (1, u32::MAX)] {
        let mut book = fixture::imported_workbook(&document());
        let chart = &mut book.sheet_mut(0).unwrap().chart_collection_mut()[0];
        paragraph_mut(chart).breaks_mut()[index].before_field = field_rank;
        let result = umya_spreadsheet::writer::xlsx::write_writer(&book, Cursor::new(Vec::new()));
        assert!(
            result.is_err(),
            "native field rank {field_rank} at run boundary 1 must be rejected"
        );
    }
}

#[test]
fn invalid_supplied_hidden_field_paints_are_replaced_by_the_authority() {
    let mut book = fixture::imported_workbook(&document());
    let previous = book.sheet(0).unwrap().chart_collection()[0].clone();
    let mut json = serde_json::to_value(&previous).unwrap();
    let mut color = PresetColor::default();
    color.set_val("not-a-preset");
    let mut fill = SolidFill::default();
    fill.set_preset_color(color);
    let mut properties = RunProperties::default();
    properties.set_solid_fill(fill);
    json["two_cell_anchor"]["graphic_frame"]["graphic"]["graphic_data"]["chart_space"]["chart"]
        ["title"]["chart_text"]["rich_text"]["paragraph"][0]["parts"]["fields"][0]["field"]
        ["run_properties"] = serde_json::to_value(properties).unwrap();
    let mut chart: Chart = serde_json::from_value(json).unwrap();
    chart.restore_export_parts(&previous).unwrap();
    assert!(
        paragraph_mut(&mut chart).fields()[0]
            .field
            .run_properties
            .is_none()
    );
    book.sheet_mut(0).unwrap().chart_collection_mut()[0] = chart;
    let xml = fixture::chart_xml(&book);
    assert!(!xml.contains("not-a-preset"));
    assert!(xml.contains(">Leading</a:t>"));
}
