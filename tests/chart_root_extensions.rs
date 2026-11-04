//! Root chart extensions use the ordinary XLSX and trusted-metadata paths.
#![cfg(feature = "serde")]

use std::io::Cursor;

use serde_json::Value;
use umya_spreadsheet::{
    Chart,
    Workbook,
    drawing::charts::{
        ChartReferenceEdit,
        ExtensionList,
    },
};

#[path = "support/chart_xml_fixture.rs"]
pub mod fixture;

const C: &str = "http://schemas.openxmlformats.org/drawingml/2006/chart";
const C15: &str = "http://schemas.microsoft.com/office/drawing/2012/chart";

fn extension(marker: &str) -> String {
    format!(
        "<c:extLst><c:ext \
         uri=\"urn:{marker}\"><u:payload>{marker}</u:payload><n:fullRef><n:sqref>Data!$A$1</n:\
         sqref></n:fullRef><u:wrapper><n:fullRef><n:sqref>Data!$A$1</n:sqref></n:fullRef></u:\
         wrapper></c:ext></c:extLst>"
    )
}

fn source(chart_before: &str, chart_after: &str, space_after: &str) -> String {
    format!(
        "<c:chartSpace xmlns:c=\"{C}\" xmlns:n=\"{C15}\" xmlns:u=\"urn:future\" \
         xmlns:x=\"urn:foreign\"><c:chart>{chart_before}<c:plotArea><c:lineChart><c:grouping \
         val=\"standard\"/><c:axId val=\"1\"/><c:axId \
         val=\"2\"/></c:lineChart></c:plotArea>{chart_after}</c:chart>{space_after}</c:chartSpace>"
    )
}

fn imported(xml: &str, eager: bool) -> Workbook {
    let mut book = umya_spreadsheet::new_file();
    book.sheet_mut(0).unwrap().add_chart(fixture::base_chart());
    let bytes = fixture::replace_chart_part(&fixture::write_workbook(&book), xml);
    let mut book = umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(bytes), eager).unwrap();
    book.read_sheet(0);
    book
}

fn exported(chart: Chart) -> String {
    let mut book = umya_spreadsheet::new_file();
    book.sheet_mut(0).unwrap().add_chart(chart);
    fixture::chart_xml(&book)
}

fn lists(value: &Value, output: &mut Vec<ExtensionList>) {
    match value {
        Value::Object(fields)
            if fields.contains_key("owner_path") && fields.contains_key("entries") =>
        {
            output.push(serde_json::from_value(value.clone()).unwrap());
        }
        Value::Object(fields) => {
            for child in fields.values() {
                lists(child, output);
            }
        }
        Value::Array(values) => {
            for child in values {
                lists(child, output);
            }
        }
        _ => {}
    }
}

fn root_lists(chart: &Chart) -> Vec<ExtensionList> {
    let mut result = Vec::new();
    lists(
        &serde_json::to_value(chart.chart_space()).unwrap(),
        &mut result,
    );
    result
}

#[test]
fn chart_and_chart_space_extensions_survive_eager_lazy_json_and_bincode() {
    let xml = source("", &extension("chart-owned"), &extension("space-owned"));
    for eager in [true, false] {
        let book = imported(&xml, eager);
        let chart = &book.sheet(0).unwrap().chart_collection()[0];
        let json = serde_json::to_vec(chart).unwrap();
        let binary = bincode::serialize(chart).unwrap();
        for restored in [
            chart.clone(),
            serde_json::from_slice(&json).unwrap(),
            bincode::deserialize(&binary).unwrap(),
        ] {
            let output = exported(restored);
            for marker in ["chart-owned", "space-owned"] {
                assert!(
                    output.contains(&format!("uri=\"urn:{marker}\"")),
                    "{eager}: {output}"
                );
            }
            assert!(output.find("urn:chart-owned").unwrap() < output.find("</c:chart>").unwrap());
            assert!(output.find("urn:space-owned").unwrap() > output.find("</c:chart>").unwrap());
        }
    }
}

#[test]
fn inherited_native_aliases_and_empty_lists_keep_their_owners() {
    let xml = source("", "<c:extLst/>", "<c:extLst></c:extLst>");
    for xml in [
        xml.clone(),
        xml.replace("c:", "d:").replace("xmlns:c=", "xmlns:d="),
        xml.replace("xmlns:c=", "xmlns=").replace("c:", ""),
    ] {
        let book = imported(&xml, false);
        let chart = &book.sheet(0).unwrap().chart_collection()[0];
        let stored = root_lists(chart);
        assert_eq!(stored.len(), 2);
        for list in stored {
            assert!(list.entries.is_empty());
            assert_eq!(list.chart_part(), "xl/charts/chart1.xml");
            assert_eq!(list.source_part_sha256().len(), 64);
            assert!(list.owner_path().contains("chartSpace[1]"));
            assert!(
                list.export_diagnostics_for_owner(Some(&book), "chart", None)
                    .is_empty()
            );
        }
    }
}

#[test]
fn foreign_root_lookalikes_do_not_replace_native_extensions() {
    let foreign = "<x:extLst><c:ext uri=\"urn:foreign-injected\"/></x:extLst>";
    let xml = source(
        foreign,
        &extension("chart-owned"),
        &format!("{foreign}{}", extension("space-owned")),
    );
    let output = fixture::chart_xml(&imported(&xml, true));
    assert!(output.contains("urn:chart-owned"));
    assert!(output.contains("urn:space-owned"));
    assert!(!output.contains("foreign-injected"));
}

#[test]
fn an_extension_before_the_first_pivot_container_does_not_create_a_duplicate() {
    let xml = source(&extension("before-pivot"), "<c:pivotFmts/>", "");
    let book = imported(&xml, true);
    let chart = book.sheet(0).unwrap().chart_collection()[0]
        .chart_space()
        .chart();
    assert!(chart.pivot_formats().is_some());
    assert!(!chart.pivot_container_has_conflict());
    assert!(fixture::chart_xml(&book).contains("urn:before-pivot"));
}

#[test]
fn repeated_root_extension_wrappers_merge_entries_without_losing_occurrences() {
    let extensions = format!("{}{}", extension("space-first"), extension("space-second"));
    let book = imported(&source("", "", &extensions), true);
    let chart = &book.sheet(0).unwrap().chart_collection()[0];
    assert_eq!(chart.chart_space().normalized_extension_lists(), 1);
    for restored in [
        serde_json::from_slice::<Chart>(&serde_json::to_vec(chart).unwrap()).unwrap(),
        bincode::deserialize::<Chart>(&bincode::serialize(chart).unwrap()).unwrap(),
    ] {
        assert_eq!(restored.chart_space().normalized_extension_lists(), 1);
    }
    let stored = root_lists(chart);
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].entries.len(), 2);
    assert_eq!(stored[0].entries[0].uri.as_deref(), Some("urn:space-first"));
    assert_eq!(
        stored[0].entries[1].uri.as_deref(),
        Some("urn:space-second")
    );
    let output = fixture::chart_xml(&book);
    assert_eq!(
        output.matches(":extLst").count(),
        2,
        "one opening/closing wrapper: {output}"
    );
    assert!(output.find("urn:space-first").unwrap() < output.find("urn:space-second").unwrap());
    assert!(
        serde_json::to_string(chart)
            .unwrap()
            .contains("normalized_extension_lists\":1")
    );
    let reimported = fixture::imported_chart(&output);
    assert_eq!(reimported.chart_space().normalized_extension_lists(), 0);
    assert_eq!(root_lists(&reimported)[0].entries.len(), 2);
}

#[test]
fn duplicate_wrappers_keep_entry_attributes_and_different_native_prefixes() {
    let second = extension("space-second")
        .replace("c:ext", "d:ext")
        .replace("<d:extLst>", &format!("<d:extLst xmlns:d=\"{C}\">"))
        .replace(" uri=", " u:flag=\"retained\" uri=");
    let xml = source("", "", &format!("{}{second}", extension("space-first")));
    let book = imported(&xml, false);
    let chart = &book.sheet(0).unwrap().chart_collection()[0];
    assert_eq!(chart.chart_space().normalized_extension_lists(), 1);
    assert!(chart.chart_space().export_conflicts(&book).is_empty());
    let output = fixture::chart_xml(&book);
    assert!(output.contains("u:flag=\"retained\""));
    assert!(output.contains("urn:space-first") && output.contains("urn:space-second"));
}

#[test]
fn an_invalid_second_wrapper_remains_diagnostic_and_rejects_export_before_zip_output() {
    let xml = source(
        "",
        "",
        &format!("{}<c:extLst><x:unexpected/></c:extLst>", extension("kept")),
    );
    let book = imported(&xml, true);
    let chart = &book.sheet(0).unwrap().chart_collection()[0];
    let before = serde_json::to_vec(chart).unwrap();
    let stored = root_lists(chart);
    assert_eq!(stored[0].entries.len(), 1);
    assert!(stored[0].conflict().is_some());
    let diagnostics = stored[0].export_diagnostics_for_owner(Some(&book), "chartSpace", None);
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].chart_part, "xl/charts/chart1.xml");
    assert!(!chart.chart_space().export_conflicts(&book).is_empty());
    let mut output = Cursor::new(Vec::new());
    assert!(umya_spreadsheet::writer::xlsx::write_writer(&book, &mut output).is_err());
    assert!(output.into_inner().is_empty());
    assert_eq!(serde_json::to_vec(chart).unwrap(), before);
}

#[test]
fn projection_hides_root_extensions_and_restoration_rejects_incoming_payloads() {
    let previous = fixture::imported_chart(&source(
        "",
        &extension("chart-owned"),
        &extension("space-owned"),
    ));
    let mut projected = previous.clone();
    projected.strip_export_parts();
    let hidden = serde_json::to_string(&projected).unwrap();
    assert!(!hidden.contains("chart-owned") && !hidden.contains("space-owned"));
    let report = projected.restore_export_parts(&previous).unwrap();
    assert!(report.restored_owners >= 2);
    assert_eq!(
        serde_json::to_vec(&projected).unwrap(),
        serde_json::to_vec(&previous).unwrap()
    );

    let mut incoming = fixture::imported_chart(&source(
        "",
        &extension("injected-chart"),
        &extension("injected-space"),
    ));
    let report = incoming.restore_export_parts(&previous).unwrap();
    assert!(report.removed_incompatible_parts >= 2);
    let output = exported(incoming);
    assert!(output.contains("chart-owned") && output.contains("space-owned"));
    assert!(!output.contains("injected-"));
}

#[test]
fn native_and_opaque_root_extension_references_follow_sheet_renames() {
    let mut chart = fixture::imported_chart(&source(
        "",
        &extension("chart-owned"),
        &extension("space-owned"),
    ));
    let report = chart.adjust_references(
        "Sheet1",
        &ChartReferenceEdit::RenameSheet {
            old: "Data".into(),
            new: "Renamed".into(),
        },
    );
    assert_eq!(report.changed, 4);
    let output = exported(chart);
    assert_eq!(output.matches("Renamed!$A$1").count(), 4, "{output}");
    assert!(!output.contains("Data!$A$1"));
}
