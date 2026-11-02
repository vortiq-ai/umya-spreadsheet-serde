#![cfg(feature = "serde")]
//! Chart-level parts (chart-wide font, data table, PivotChart link, report
//! filter layout, page setup, in-chart shapes) through the ordinary route:
//! Excel-authored XLSX -> reader -> public API -> writer -> XML and reread.
//! The fixtures were authored with Excel (COM, see
//! fixtures/chart_space_parts/README.txt); expected XML is Excel's own,
//! compared as canonical element trees, never the library's output.
use std::{
    io::{
        Cursor,
        Read,
    },
    path::PathBuf,
};

#[path = "support/canonical_xml.rs"]
mod canonical_xml;

use canonical_xml::canonical_xml;
use quick_xml::{
    Reader,
    events::Event,
};
use umya_spreadsheet::{
    Workbook,
    drawing::charts::{
        DataTable,
        PageOrientation,
        PivotSource,
    },
    reader,
    writer,
};

macro_rules! fixture {
    ($name:literal) => {
        include_bytes!(concat!("fixtures/chart_space_parts/", $name, ".xlsx")) as &[u8]
    };
}

fn read(bytes: &[u8], lazy: bool) -> Workbook {
    let mut book = reader::xlsx::read_reader(Cursor::new(bytes.to_vec()), !lazy).unwrap();
    if lazy {
        for index in 0..book.sheet_count() {
            book.try_read_sheet(index).unwrap();
        }
    }
    book
}

fn write(book: &Workbook) -> Vec<u8> {
    let mut out = Cursor::new(Vec::new());
    writer::xlsx::write_writer(book, &mut out).unwrap();
    out.into_inner()
}

fn part(bytes: &[u8], name: &str) -> String {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut text = String::new();
    archive
        .by_name(name)
        .unwrap_or_else(|_| panic!("no part {name}"))
        .read_to_string(&mut text)
        .unwrap();
    text
}

fn has_part(bytes: &[u8], name: &str) -> bool {
    zip::ZipArchive::new(Cursor::new(bytes))
        .unwrap()
        .by_name(name)
        .is_ok()
}

/// Canonical element tree of the first element named `name`: attributes
/// sorted, namespace declarations dropped, empty elements spelled as
/// start/end pairs, text trimmed.
fn subtree(xml: &str, name: &str) -> String {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut out = String::new();
    let mut depth = 0usize;
    let mut capturing = false;
    loop {
        let event = reader.read_event().unwrap();
        let opening = match &event {
            Event::Start(e) | Event::Empty(e) => Some(e),
            _ => None,
        };
        if let Some(e) = opening {
            let tag = String::from_utf8_lossy(e.name().as_ref()).into_owned();
            if !capturing && tag == name {
                capturing = true;
                depth = 0;
            }
            if capturing {
                let mut attributes: Vec<String> = e
                    .attributes()
                    .map(|a| {
                        let a = a.unwrap();
                        (
                            String::from_utf8_lossy(a.key.as_ref()).into_owned(),
                            a.normalized_value(quick_xml::XmlVersion::Implicit1_0)
                                .unwrap()
                                .into_owned(),
                        )
                    })
                    .filter(|(key, _)| key != "xmlns" && !key.starts_with("xmlns:"))
                    .map(|(key, value)| format!(" {key}=\"{value}\""))
                    .collect();
                attributes.sort();
                out.push_str(&format!("<{tag}{}>", attributes.concat()));
                if matches!(event, Event::Empty(_)) {
                    out.push_str(&format!("</{tag}>"));
                    if depth == 0 {
                        return out;
                    }
                } else {
                    depth += 1;
                }
            }
            continue;
        }
        match event {
            Event::End(e) if capturing => {
                out.push_str(&format!(
                    "</{}>",
                    String::from_utf8_lossy(e.name().as_ref())
                ));
                depth -= 1;
                if depth == 0 {
                    return out;
                }
            }
            Event::Text(t) if capturing => out.push_str(&t.decode().unwrap()),
            Event::GeneralRef(r) if capturing => {
                out.push_str(&format!("&{};", r.decode().unwrap()));
            }
            Event::Eof => panic!("no element {name} in {xml}"),
            _ => (),
        }
    }
}

fn chart_xml(bytes: &[u8]) -> String {
    part(bytes, "xl/charts/chart1.xml")
}

/// The direct children of `c:chartSpace` in order.
fn chart_space_children(xml: &str) -> Vec<String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut depth = 0usize;
    let mut names = Vec::new();
    loop {
        match reader.read_event().unwrap() {
            Event::Start(e) => {
                if depth == 1 {
                    names.push(String::from_utf8_lossy(e.name().as_ref()).into_owned());
                }
                depth += 1;
            }
            Event::Empty(e) => {
                if depth == 1 {
                    names.push(String::from_utf8_lossy(e.name().as_ref()).into_owned());
                }
            }
            Event::End(_) => depth -= 1,
            Event::Eof => return names,
            _ => (),
        }
    }
}

/// Writes the output where the Excel check script finds it.
fn emit(name: &str, bytes: &[u8]) {
    if let Some(target) = std::env::var_os("CARGO_TARGET_DIR") {
        let directory = PathBuf::from(target).join("chart-space-validation");
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join(format!("{name}.xlsx")), bytes).unwrap();
    }
}

fn first_chart_mut<'a>(book: &'a mut Workbook, sheet: &str) -> &'a mut umya_spreadsheet::Chart {
    &mut book
        .sheet_by_name_mut(sheet)
        .unwrap()
        .chart_collection_mut()[0]
}

// ---- chart-wide font -----------------------------------------------------

#[test]
fn chart_wide_font_keeps_excels_text_properties() {
    for lazy in [false, true] {
        let book = read(fixture!("chart-font"), lazy);
        let space = book.sheet_by_name("Sheet1").unwrap().chart_collection()[0].chart_space();
        let run = space.text_properties().unwrap().paragraph()[0]
            .paragraph_properties()
            .default_run_properties()
            .unwrap();
        // Excel wrote sz="1400" b="1" and the Courier New latin face.
        assert_eq!(run.sz(), "1400");
        assert_eq!(run.bold(), "1");
        assert_eq!(run.latin_font().unwrap().typeface(), "Courier New");
        let output = write(&book);
        let source = chart_xml(fixture!("chart-font"));
        let written = chart_xml(&output);
        // The chart-level txPr is the last c:txPr child of chartSpace.
        let at = |xml: &str| {
            let tail = &xml[xml.find("</c:chart>").unwrap()..];
            subtree(tail.split_once("</c:spPr>").unwrap().1, "c:txPr")
        };
        assert_eq!(at(&written), at(&source));
        let children = chart_space_children(&written);
        let position = |name: &str| children.iter().position(|c| c == name).unwrap();
        assert!(position("c:chart") < position("c:spPr"));
        assert!(position("c:spPr") < position("c:txPr"));
        assert!(position("c:txPr") < position("c:printSettings"));
        emit(&format!("chart-font-roundtrip-{lazy}"), &output);
    }
}

#[test]
fn chart_wide_font_edit_and_removal_reach_the_xml() {
    let mut book = read(fixture!("chart-font"), false);
    let space = first_chart_mut(&mut book, "Sheet1").chart_space_mut();
    space
        .text_properties_mut()
        .unwrap()
        .paragraph_mut()
        .first_mut()
        .unwrap()
        .paragraph_properties_mut()
        .default_run_properties_mut()
        .unwrap()
        .set_sz("2000");
    let edited = write(&book);
    emit("chart-font-size-20", &edited);
    let xml = chart_xml(&edited);
    let tail = &xml[xml.find("</c:chart>").unwrap()..];
    assert!(tail.contains("<a:defRPr sz=\"2000\" b=\"1\">"));

    let mut book = read(fixture!("chart-font"), false);
    first_chart_mut(&mut book, "Sheet1")
        .chart_space_mut()
        .remove_text_properties();
    let removed = write(&book);
    emit("chart-font-removed", &removed);
    let xml = chart_xml(&removed);
    let tail = &xml[xml.find("</c:chart>").unwrap()..];
    assert!(!tail.contains("<c:txPr>"), "{tail}");
}

// ---- data table ----------------------------------------------------------

#[test]
fn data_table_round_trips_with_excels_flags_and_formats() {
    for lazy in [false, true] {
        let book = read(fixture!("data-table-options"), lazy);
        let plot = book.sheet_by_name("Sheet1").unwrap().chart_collection()[0]
            .chart_space()
            .chart()
            .plot_area();
        let table = plot.data_table().unwrap();
        // Excel wrote showHorzBorder=0 showVertBorder=0 showOutline=1
        // showKeys=0.
        assert_eq!(table.show_horizontal_border(), Some(false));
        assert_eq!(table.show_vertical_border(), Some(false));
        assert_eq!(table.show_outline(), Some(true));
        assert_eq!(table.show_keys(), Some(false));
        assert!(table.shape_properties().is_some());
        assert_eq!(
            table.text_properties().unwrap().paragraph()[0]
                .paragraph_properties()
                .default_run_properties()
                .unwrap()
                .sz(),
            "900"
        );
        let output = write(&book);
        assert_eq!(
            subtree(&chart_xml(&output), "c:dTable"),
            subtree(&chart_xml(fixture!("data-table-options")), "c:dTable")
        );
        // Schema order: the data table follows the axes and precedes the
        // plot area's own spPr.
        let xml = chart_xml(&output);
        let table_at = xml.find("<c:dTable>").unwrap();
        assert!(xml.find("</c:valAx>").unwrap() < table_at);
        assert!(table_at < xml.find("</c:dTable><c:spPr>").unwrap());
        emit(&format!("data-table-roundtrip-{lazy}"), &output);
    }
}

#[test]
fn data_table_edit_and_removal_reach_the_xml() {
    let mut book = read(fixture!("data-table-options"), false);
    first_chart_mut(&mut book, "Sheet1")
        .chart_space_mut()
        .chart_mut()
        .plot_area_mut()
        .data_table_mut()
        .unwrap()
        .set_show_vertical_border(true)
        .set_show_keys(true);
    let edited = write(&book);
    emit("data-table-edited", &edited);
    let table = subtree(&chart_xml(&edited), "c:dTable");
    assert!(table.starts_with(
        "<c:dTable><c:showHorzBorder val=\"0\"></c:showHorzBorder><c:showVertBorder \
         val=\"1\"></c:showVertBorder><c:showOutline val=\"1\"></c:showOutline><c:showKeys \
         val=\"1\"></c:showKeys><c:spPr>"
    ));

    let mut book = read(fixture!("data-table-options"), false);
    first_chart_mut(&mut book, "Sheet1")
        .chart_space_mut()
        .chart_mut()
        .plot_area_mut()
        .remove_data_table();
    let removed = write(&book);
    emit("data-table-removed", &removed);
    assert!(!chart_xml(&removed).contains("c:dTable"));
}

#[test]
fn data_table_is_created_from_the_api_in_schema_order() {
    let mut book = read(fixture!("page-setup"), false);
    let mut table = DataTable::default();
    table
        .set_show_horizontal_border(true)
        .set_show_outline(false);
    first_chart_mut(&mut book, "Sheet1")
        .chart_space_mut()
        .chart_mut()
        .plot_area_mut()
        .set_data_table(table);
    let created = write(&book);
    emit("data-table-created", &created);
    let xml = chart_xml(&created);
    assert_eq!(
        subtree(&xml, "c:dTable"),
        "<c:dTable><c:showHorzBorder val=\"1\"></c:showHorzBorder><c:showOutline \
         val=\"0\"></c:showOutline></c:dTable>"
    );
    assert!(xml.contains("</c:valAx><c:dTable>"));
    assert!(xml.contains("</c:dTable><c:spPr>"));
    // The reread chart has the table.
    let again = read(&created, false);
    let table = again.sheet_by_name("Sheet1").unwrap().chart_collection()[0]
        .chart_space()
        .chart()
        .plot_area()
        .data_table()
        .unwrap()
        .clone();
    assert_eq!(table.show_horizontal_border(), Some(true));
    assert_eq!(table.show_vertical_border(), None);
}

fn replace_in_chart(bytes: &[u8], from: &str, to: &str) -> Vec<u8> {
    let xml = chart_xml(bytes);
    assert!(xml.contains(from), "{from}");
    replace_chart(bytes, &xml.replacen(from, to, 1))
}

const TABLE_EXTENSION: &str = "<c:extLst><c:ext uri=\"urn:hidden\"><h:p \
                               xmlns:h=\"urn:hidden\">TABLEEXT</h:p></c:ext></c:extLst>";

fn with_table_extension(bytes: &[u8]) -> Vec<u8> {
    let xml = chart_xml(bytes).replace("</c:dTable>", &format!("{TABLE_EXTENSION}</c:dTable>"));
    let mut input = zip::ZipArchive::new(Cursor::new(bytes.to_vec())).unwrap();
    let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for index in 0..input.len() {
        let mut entry = input.by_index(index).unwrap();
        let mut data = Vec::new();
        entry.read_to_end(&mut data).unwrap();
        if entry.name().unwrap() == "xl/charts/chart1.xml" {
            data = xml.clone().into_bytes();
        }
        use std::io::Write;
        output
            .start_file(
                entry.name().unwrap(),
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
        output.write_all(&data).unwrap();
    }
    output.finish().unwrap().into_inner()
}

#[test]
fn data_table_extension_is_hidden_from_the_projection_and_restored() {
    let book = read(&with_table_extension(fixture!("data-table-options")), false);
    let complete = book.sheet_by_name("Sheet1").unwrap().chart_collection()[0].clone();
    assert!(
        complete
            .chart_space()
            .chart()
            .plot_area()
            .data_table()
            .unwrap()
            .extensions()
            .is_some()
    );
    // The complete chart writes the extension back.
    assert!(chart_xml(&write(&book)).contains("TABLEEXT"));

    let mut projection = complete.clone();
    projection.strip_export_parts();
    let shown = projection
        .chart_space()
        .chart()
        .plot_area()
        .data_table()
        .unwrap();
    assert!(shown.extensions().is_none());
    assert_eq!(shown.show_outline(), Some(true));

    // An edit made on the projection keeps the hidden extension after the
    // restoration against the authoritative chart.
    let mut edited = projection;
    edited
        .chart_space_mut()
        .chart_mut()
        .plot_area_mut()
        .data_table_mut()
        .unwrap()
        .set_show_keys(true);
    let report = edited.restore_export_parts(&complete).unwrap();
    assert!(report.restored_owners > 0);
    let restored = edited
        .chart_space()
        .chart()
        .plot_area()
        .data_table()
        .unwrap();
    assert!(restored.extensions().is_some());
    assert_eq!(restored.show_keys(), Some(true));
    let mut book = read(&with_table_extension(fixture!("data-table-options")), false);
    book.sheet_by_name_mut("Sheet1")
        .unwrap()
        .chart_collection_mut()[0] = edited;
    let xml = chart_xml(&write(&book));
    assert!(xml.contains("TABLEEXT"));
    assert!(subtree(&xml, "c:dTable").contains("<c:showKeys val=\"1\">"));
}

#[test]
fn chart_wide_font_and_pivot_source_extension_follow_the_projection_rules() {
    let hidden = "<c:extLst><c:ext uri=\"urn:hidden\"><h:p                   \
                  xmlns:h=\"urn:hidden\">SOURCEEXT</h:p></c:ext></c:extLst>";
    let bytes = replace_in_chart(
        fixture!("pivotchart"),
        "</c:pivotSource>",
        &format!("{hidden}</c:pivotSource>"),
    );
    let book = read(&bytes, false);
    let complete = pivot_sheet(&book).chart_collection()[0].clone();
    assert!(
        complete
            .chart_space()
            .pivot_source()
            .unwrap()
            .extension_lists()
            .len()
            == 1
    );
    assert!(chart_xml(&write(&book)).contains("SOURCEEXT"));

    let mut projection = complete.clone();
    projection.strip_export_parts();
    let shown = projection.chart_space().pivot_source().unwrap();
    assert!(shown.extension_lists().is_empty());
    assert_eq!(shown.name(), "[pivotchart.xlsx]Pivot!PivotTable1");

    // A chart-wide font edited on the projection keeps the hidden text-body
    // parts and the hidden pivot extension after restoration.
    let mut edited = projection;
    edited
        .chart_space_mut()
        .text_properties_mut()
        .unwrap()
        .paragraph_mut()
        .first_mut()
        .unwrap()
        .paragraph_properties_mut()
        .default_run_properties_mut()
        .unwrap()
        .set_sz("1800");
    edited.restore_export_parts(&complete).unwrap();
    assert!(
        edited
            .chart_space()
            .pivot_source()
            .unwrap()
            .extension_lists()
            .len()
            == 1
    );
    let mut book = read(&bytes, false);
    book.sheet_by_name_mut("Pivot")
        .unwrap()
        .chart_collection_mut()[0] = edited;
    let xml = chart_xml(&write(&book));
    assert!(xml.contains("SOURCEEXT"));
    let tail = &xml[xml.find("</c:chart>").unwrap()..];
    assert!(tail.contains("<a:defRPr sz=\"1800\""));
    assert!(tail.contains("<a:lstStyle/>"));
}

// ---- PivotChart link and report filter layout ------------------------------

fn pivot_sheet(book: &Workbook) -> &umya_spreadsheet::Worksheet {
    book.sheet_by_name("Pivot").unwrap()
}

#[test]
fn pivot_chart_keeps_its_pivot_source_and_options() {
    for lazy in [false, true] {
        let book = read(fixture!("pivotchart"), lazy);
        let space = pivot_sheet(&book).chart_collection()[0].chart_space();
        let source = space.pivot_source().unwrap();
        assert_eq!(source.name(), "[pivotchart.xlsx]Pivot!PivotTable1");
        assert_eq!(source.format_id(), 0);
        assert_eq!(source.sheet_name(), Some("Pivot"));
        assert_eq!(source.table_name(), "PivotTable1");
        let output = write(&book);
        let written = chart_xml(&output);
        let excel = chart_xml(fixture!("pivotchart"));
        assert_eq!(
            subtree(&written, "c:pivotSource"),
            subtree(&excel, "c:pivotSource")
        );
        // Schema order: pivotSource precedes chart.
        let children = chart_space_children(&written);
        assert!(
            children.iter().position(|c| c == "c:pivotSource")
                < children.iter().position(|c| c == "c:chart")
        );
        // Excel's drop-zone and expand-button options stay in the chartSpace
        // extension list, inside their two ext entries.
        assert_eq!(
            subtree(&written, "c14:pivotOptions"),
            subtree(&excel, "c14:pivotOptions")
        );
        assert_eq!(
            subtree(&written, "c16:pivotOptions16"),
            subtree(&excel, "c16:pivotOptions16")
        );
        // Excel's empty c15 label-extension shells inside the pivot formats
        // are the one normalisation: the library drops an empty known shell.
        assert_eq!(
            subtree(&written, "c:pivotFmts"),
            subtree(&excel, "c:pivotFmts").replace(
                "<c:extLst><c:ext \
                 uri=\"{CE6537A1-D6FC-4f65-9D91-7224C49458BB}\"></c:ext></c:extLst>",
                ""
            )
        );
        emit(&format!("pivotchart-roundtrip-{lazy}"), &output);
    }
}

#[test]
fn pivot_source_edit_reaches_the_xml() {
    let mut book = read(fixture!("pivotchart"), false);
    book.sheet_by_name_mut("Pivot")
        .unwrap()
        .chart_collection_mut()[0]
        .chart_space_mut()
        .pivot_source_mut()
        .unwrap()
        .set_sheet_name("Sales 2026")
        .set_format_id(7);
    let xml = chart_xml(&write(&book));
    assert_eq!(
        subtree(&xml, "c:pivotSource"),
        "<c:pivotSource><c:name>[pivotchart.xlsx]Sales 2026!PivotTable1</c:name><c:fmtId \
         val=\"7\"></c:fmtId></c:pivotSource>"
    );
    let mut book = read(fixture!("pivotchart"), false);
    book.sheet_by_name_mut("Pivot")
        .unwrap()
        .chart_collection_mut()[0]
        .chart_space_mut()
        .set_pivot_source(PivotSource::new("[other.xlsx]Pivot!PivotTable9", 2));
    assert!(chart_xml(&write(&book)).contains("<c:name>[other.xlsx]Pivot!PivotTable9</c:name>"));
    let mut book = read(fixture!("pivotchart"), false);
    book.sheet_by_name_mut("Pivot")
        .unwrap()
        .chart_collection_mut()[0]
        .chart_space_mut()
        .remove_pivot_source();
    assert!(!chart_xml(&write(&book)).contains("pivotSource"));
}

#[test]
fn report_filter_layout_survives_in_the_pivot_table() {
    for lazy in [false, true] {
        let book = read(fixture!("pivotchart-pages"), lazy);
        let location = pivot_sheet(&book).pivot_tables()[0]
            .pivot_table_definition()
            .location();
        // Excel wrote rowPageCount="1" colPageCount="2" (wrap 2, over then
        // down).
        assert_eq!(location.row_page_count(), Some(1));
        assert_eq!(location.column_page_count(), Some(2));
        let output = write(&book);
        let written = part(&output, "xl/pivotTables/pivotTable1.xml");
        let excel = part(
            fixture!("pivotchart-pages"),
            "xl/pivotTables/pivotTable1.xml",
        );
        assert_eq!(subtree(&written, "location"), subtree(&excel, "location"));
        emit(&format!("pivotchart-pages-roundtrip-{lazy}"), &output);
    }
    // A table without report filters carries neither attribute.
    let book = read(fixture!("pivotchart"), false);
    let location = pivot_sheet(&book).pivot_tables()[0]
        .pivot_table_definition()
        .location();
    assert_eq!(location.row_page_count(), None);
    assert_eq!(location.column_page_count(), None);
    assert!(!part(&write(&book), "xl/pivotTables/pivotTable1.xml").contains("rowPageCount"));
}

#[test]
fn report_filter_layout_edit_reaches_the_xml() {
    let mut book = read(fixture!("pivotchart-pages"), false);
    let location = book
        .sheet_by_name_mut("Pivot")
        .unwrap()
        .pivot_tables_mut()
        .first_mut()
        .unwrap()
        .pivot_table_definition_mut()
        .location_mut();
    location.set_row_page_count(3).remove_column_page_count();
    let xml = part(&write(&book), "xl/pivotTables/pivotTable1.xml");
    let location = subtree(&xml, "location");
    assert!(location.contains("rowPageCount=\"3\""));
    assert!(!location.contains("colPageCount"));
}

#[test]
fn pivot_chart_follows_a_renamed_pivot_sheet() {
    // Excel detaches a PivotChart whose pivotSource names a sheet that no
    // longer exists (measured: PivotLayout is null after opening), so the
    // rename must reach the link, also for a sheet that is still unread.
    for lazy_unread in [false, true] {
        let mut book = if lazy_unread {
            reader::xlsx::read_reader(Cursor::new(fixture!("pivotchart").to_vec()), false).unwrap()
        } else {
            read(fixture!("pivotchart"), false)
        };
        let index = book
            .sheet_collection_no_check()
            .iter()
            .position(|sheet| sheet.name() == "Pivot")
            .unwrap();
        book.set_sheet_name(index, "Sales 2026").unwrap();
        let output = write(&book);
        emit(&format!("pivotchart-renamed-{lazy_unread}"), &output);
        assert_eq!(
            subtree(&chart_xml(&output), "c:pivotSource"),
            "<c:pivotSource><c:name>[pivotchart.xlsx]Sales 2026!PivotTable1</c:name><c:fmtId \
             val=\"0\"></c:fmtId></c:pivotSource>"
        );
    }
}

// ---- page setup ------------------------------------------------------------

#[test]
fn chart_page_setup_round_trips_with_excels_values() {
    for lazy in [false, true] {
        let book = read(fixture!("page-setup"), lazy);
        let settings = book.sheet_by_name("Sheet1").unwrap().chart_collection()[0]
            .chart_space()
            .print_settings()
            .unwrap();
        let header = settings.header_footer().unwrap();
        // Excel wrote oddHeader "&CChart head" and oddFooter "&LLeft foot".
        assert_eq!(header.odd_header(), Some("&CChart head"));
        assert_eq!(header.odd_footer(), Some("&LLeft foot"));
        assert_eq!(header.even_header(), None);
        let setup = settings.page_setup().unwrap();
        assert_eq!(setup.paper_size(), Some(8));
        assert_eq!(setup.first_page_number(), Some(3));
        assert_eq!(setup.orientation(), Some(PageOrientation::Landscape));
        assert_eq!(setup.black_and_white(), Some(true));
        assert_eq!(setup.draft(), Some(true));
        assert_eq!(setup.use_first_page_number(), Some(true));
        assert_eq!(setup.copies(), None);
        assert!((settings.page_margins().unwrap().left() - 0.5).abs() < 1e-12);
        let output = write(&book);
        assert_eq!(
            subtree(&chart_xml(&output), "c:printSettings"),
            subtree(&chart_xml(fixture!("page-setup")), "c:printSettings")
        );
        emit(&format!("page-setup-roundtrip-{lazy}"), &output);
    }
}

#[test]
fn chart_page_setup_edit_and_removal_reach_the_xml() {
    let mut book = read(fixture!("page-setup"), false);
    let settings = first_chart_mut(&mut book, "Sheet1")
        .chart_space_mut()
        .print_settings_mut()
        .unwrap();
    settings
        .page_setup_mut()
        .unwrap()
        .set_orientation(PageOrientation::Portrait)
        .set_copies(2)
        .set_paper_width("210mm")
        .remove_draft();
    settings
        .header_footer_mut()
        .unwrap()
        .set_even_header("&RGone")
        .remove_odd_footer()
        .set_different_odd_even(true);
    let edited = write(&book);
    emit("page-setup-edited", &edited);
    let xml = chart_xml(&edited);
    assert_eq!(
        subtree(&xml, "c:pageSetup"),
        "<c:pageSetup blackAndWhite=\"1\" copies=\"2\" firstPageNumber=\"3\" \
         orientation=\"portrait\" paperSize=\"8\" paperWidth=\"210mm\" \
         useFirstPageNumber=\"1\"></c:pageSetup>"
    );
    assert_eq!(
        subtree(&xml, "c:headerFooter"),
        "<c:headerFooter differentOddEven=\"1\"><c:oddHeader>&amp;CChart \
         head</c:oddHeader><c:evenHeader>&amp;RGone</c:evenHeader></c:headerFooter>"
    );

    let mut book = read(fixture!("page-setup"), false);
    let settings = first_chart_mut(&mut book, "Sheet1")
        .chart_space_mut()
        .print_settings_mut()
        .unwrap();
    settings.remove_header_footer().remove_page_setup();
    let removed = write(&book);
    emit("page-setup-removed", &removed);
    let xml = chart_xml(&removed);
    assert!(!xml.contains("c:headerFooter") && !xml.contains("c:pageSetup"));
    assert!(xml.contains("<c:pageMargins "));
}

#[test]
fn absent_print_settings_parts_stay_absent() {
    // printSettings is all-optional in the schema: a chart that holds only
    // the margins must not gain an empty headerFooter or pageSetup.
    let xml = chart_xml(fixture!("page-setup"));
    let start = xml.find("<c:printSettings>").unwrap();
    let end = xml.find("</c:printSettings>").unwrap() + "</c:printSettings>".len();
    let margins_only = format!(
        "{}<c:printSettings><c:pageMargins b=\"0.75\" l=\"0.7\" r=\"0.7\" t=\"0.75\"          \
         header=\"0.3\" footer=\"0.3\"/></c:printSettings>{}",
        &xml[..start],
        &xml[end..]
    );
    let book = read(&replace_chart(fixture!("page-setup"), &margins_only), false);
    let settings = book.sheet_by_name("Sheet1").unwrap().chart_collection()[0]
        .chart_space()
        .print_settings()
        .unwrap();
    assert!(settings.header_footer().is_none() && settings.page_setup().is_none());
    let written = chart_xml(&write(&book));
    assert!(!written.contains("headerFooter") && !written.contains("pageSetup"));
    assert!(written.contains("<c:pageMargins "));
}

#[test]
fn foreign_header_footer_children_cannot_replace_native_text() {
    let native = "<c:headerFooter><c:oddHeader>&amp;CChart \
                  head</c:oddHeader><c:oddFooter>&amp;LLeft foot</c:oddFooter><c:evenHeader>Even \
                  head</c:evenHeader><c:evenFooter>Even foot</c:evenFooter><c:firstHeader>First \
                  head</c:firstHeader><c:firstFooter>First foot</c:firstFooter></c:headerFooter>";
    for empty in [false, true] {
        let mut mixed = native.to_string();
        for name in [
            "oddHeader",
            "oddFooter",
            "evenHeader",
            "evenFooter",
            "firstHeader",
            "firstFooter",
        ] {
            let foreign = if empty {
                format!("<x:{name} xmlns:x=\"urn:foreign\"/>")
            } else {
                format!("<x:{name} xmlns:x=\"urn:foreign\">WRONG</x:{name}>")
            };
            mixed = mixed.replace(&format!("</c:{name}>"), &format!("</c:{name}>{foreign}"));
        }
        let original = chart_xml(fixture!("page-setup"));
        let start = original.find("<c:headerFooter").unwrap();
        let end = original.find("</c:headerFooter>").unwrap() + "</c:headerFooter>".len();
        let xml = format!("{}{mixed}{}", &original[..start], &original[end..]);
        for lazy in [false, true] {
            let mut book = read(&replace_chart(fixture!("page-setup"), &xml), lazy);
            first_chart_mut(&mut book, "Sheet1")
                .chart_space_mut()
                .print_settings_mut()
                .unwrap()
                .page_setup_mut()
                .unwrap()
                .set_copies(2);
            let mut light = Cursor::new(Vec::new());
            writer::xlsx::write_writer_light(&book, &mut light).unwrap();
            for output in [write(&book), light.into_inner()] {
                assert_eq!(
                    subtree(&chart_xml(&output), "c:headerFooter"),
                    subtree(native, "c:headerFooter")
                );
                let again = read(&output, lazy);
                let header = again.sheet_by_name("Sheet1").unwrap().chart_collection()[0]
                    .chart_space()
                    .print_settings()
                    .unwrap()
                    .header_footer()
                    .unwrap();
                assert_eq!(header.odd_header(), Some("&CChart head"));
                assert_eq!(header.odd_footer(), Some("&LLeft foot"));
                assert_eq!(header.even_header(), Some("Even head"));
                assert_eq!(header.even_footer(), Some("Even foot"));
                assert_eq!(header.first_header(), Some("First head"));
                assert_eq!(header.first_footer(), Some("First foot"));
            }
        }
    }
}

#[test]
fn foreign_print_settings_children_cannot_replace_native_parts() {
    let original = chart_xml(fixture!("page-setup"));
    for empty in [false, true] {
        let mut foreign = String::new();
        for name in [
            "headerFooter",
            "pageMargins",
            "pageSetup",
            "legacyDrawingHF",
        ] {
            foreign.push_str(&if empty {
                format!("<x:{name} xmlns:x=\"urn:foreign\"/>")
            } else {
                format!(
                    "<x:{name} xmlns:x=\"urn:foreign\"><c:oddHeader>WRONG</c:oddHeader></x:{name}>"
                )
            });
        }
        let xml = original.replace(
            "</c:printSettings>",
            &format!("{foreign}</c:printSettings>"),
        );
        for lazy in [false, true] {
            let mut book = read(&replace_chart(fixture!("page-setup"), &xml), lazy);
            first_chart_mut(&mut book, "Sheet1")
                .chart_space_mut()
                .print_settings_mut()
                .unwrap()
                .page_setup_mut()
                .unwrap()
                .set_copies(2);
            let mut light = Cursor::new(Vec::new());
            writer::xlsx::write_writer_light(&book, &mut light).unwrap();
            for output in [write(&book), light.into_inner()] {
                let written = chart_xml(&output);
                assert_eq!(
                    subtree(&written, "c:headerFooter"),
                    subtree(&original, "c:headerFooter")
                );
                assert_eq!(
                    subtree(&written, "c:pageMargins"),
                    subtree(&original, "c:pageMargins")
                );
                assert!(!written.contains("legacyDrawingHF"));
                let again = read(&output, lazy);
                let settings = again.sheet_by_name("Sheet1").unwrap().chart_collection()[0]
                    .chart_space()
                    .print_settings()
                    .unwrap();
                assert_eq!(settings.page_setup().unwrap().paper_size(), Some(8));
                assert_eq!(settings.page_setup().unwrap().copies(), Some(2));
                assert_eq!(
                    settings.page_setup().unwrap().orientation(),
                    Some(PageOrientation::Landscape)
                );
            }
        }
    }
}

// ---- in-chart shapes
// ---------------------------------------------------------

#[test]
fn drawing_comparison_resolves_namespaces_and_preserves_inner_text() {
    let drawing = r#"<cdr:userShapes xmlns:cdr="urn:chart-drawing" xmlns:a="urn:drawing"><cdr:sp a:flag="yes"><a:t>Note inside</a:t></cdr:sp></cdr:userShapes>"#;
    let with_prolog = format!("<?xml version=\"1.0\"?>\n \t{drawing}\r\n");
    assert_eq!(
        canonical_xml(drawing.as_bytes()),
        canonical_xml(with_prolog.as_bytes())
    );
    for (original, replacement) in [
        ("urn:chart-drawing", "urn:wrong-chart-drawing"),
        ("urn:drawing", "urn:wrong-drawing"),
        ("Note inside", " Note inside "),
        ("<cdr:sp", " \n<cdr:sp"),
        ("a:flag=\"yes\"", "a:flag=\"no\""),
    ] {
        assert_ne!(
            canonical_xml(drawing.as_bytes()),
            canonical_xml(drawing.replace(original, replacement).as_bytes()),
            "{original} -> {replacement}"
        );
    }
}

#[test]
fn in_chart_shapes_stay_linked_to_their_drawing_part() {
    for lazy in [false, true] {
        let book = read(fixture!("textbox-in-chart"), lazy);
        let space = book.sheet_by_name("Sheet1").unwrap().chart_collection()[0].chart_space();
        assert_eq!(space.user_shapes().unwrap().relationship_id(), "rId3");
        let output = write(&book);
        let written = chart_xml(&output);
        assert!(written.contains("<c:userShapes r:id=\"rId3\""), "{written}");
        // The element sits between printSettings and the end of chartSpace.
        let children = chart_space_children(&written);
        let position = |name: &str| children.iter().position(|c| c == name).unwrap();
        assert!(position("c:printSettings") < position("c:userShapes"));
        // The relationship points at the drawing part the chart's drawing
        // model wrote, which holds Excel's shapes unchanged.
        let rels = part(&output, "xl/charts/_rels/chart1.xml.rels");
        let user_shapes = rels
            .split("<Relationship ")
            .find(|edge| edge.contains("Id=\"rId3\""))
            .unwrap();
        assert!(user_shapes.contains(concat!(
            "Type=\"http://schemas.openxmlformats.org/officeDocument/2006/",
            "relationships/chartUserShapes\""
        )));
        let target = user_shapes
            .split("Target=\"../drawings/")
            .nth(1)
            .unwrap()
            .split('"')
            .next()
            .unwrap()
            .to_owned();
        let source_part = part(fixture!("textbox-in-chart"), "xl/drawings/drawing2.xml");
        assert_eq!(
            canonical_xml(part(&output, &format!("xl/drawings/{target}")).as_bytes()),
            canonical_xml(source_part.as_bytes())
        );
        assert!(
            part(&output, "[Content_Types].xml").contains(
                "application/vnd.openxmlformats-officedocument.drawingml.chartshapes+xml"
            )
        );
        emit(&format!("textbox-roundtrip-{lazy}"), &output);
    }
}

#[test]
fn light_writer_keeps_the_in_chart_shapes_linked_too() {
    let book = read(fixture!("textbox-in-chart"), false);
    let mut out = Cursor::new(Vec::new());
    writer::xlsx::write_writer_light(&book, &mut out).unwrap();
    let output = out.into_inner();
    assert!(chart_xml(&output).contains("<c:userShapes r:id=\"rId3\""));
    assert_eq!(
        canonical_xml(part(&output, "xl/drawings/drawing2.xml").as_bytes()),
        canonical_xml(part(fixture!("textbox-in-chart"), "xl/drawings/drawing2.xml").as_bytes())
    );
}

#[test]
fn in_chart_shapes_without_their_relationship_fail_the_export() {
    // A chart whose shapes are only its preserved drawing part (persisted
    // before the drawing model existed), moved to a workbook that lacks the
    // chart part's relationships, cannot write an id that points nowhere. A
    // chart with its drawing model gets a fresh part (tests/chart_drawing.rs).
    let book = read(fixture!("textbox-in-chart"), false);
    let chart = book.sheet_by_name("Sheet1").unwrap().chart_collection()[0].clone();
    let mut value = serde_json::to_value(&chart).unwrap();
    let space = find_key_mut(&mut value, "chart_space")
        .unwrap()
        .as_object_mut()
        .unwrap();
    assert!(space.remove("chart_drawing").is_some());
    assert!(space.remove("chart_drawing_parts").is_some());
    let chart: umya_spreadsheet::Chart = serde_json::from_value(value).unwrap();
    assert!(chart.chart_space().chart_drawing().is_none());
    assert!(chart.chart_space().user_shapes().is_some());
    let mut target = umya_spreadsheet::new_file();
    target.sheet_mut(0).unwrap().add_chart(chart);
    let mut out = Cursor::new(Vec::new());
    let error = writer::xlsx::write_writer(&target, &mut out).unwrap_err();
    assert!(error.to_string().contains("c:userShapes"), "{error}");
}

#[test]
fn external_data_link_round_trips_and_is_validated() {
    // The Excel fixture has no externalData; add one pointing at a missing
    // relationship and expect the explicit error.
    let xml = chart_xml(fixture!("textbox-in-chart")).replace(
        "<c:printSettings>",
        "<c:externalData r:id=\"rId9\"><c:autoUpdate val=\"0\"/></c:externalData><c:printSettings>",
    );
    let input = replace_chart(fixture!("textbox-in-chart"), &xml);
    let book = read(&input, false);
    let data = book.sheet_by_name("Sheet1").unwrap().chart_collection()[0]
        .chart_space()
        .external_data()
        .unwrap();
    assert_eq!(data.relationship_id(), "rId9");
    assert_eq!(data.auto_update(), Some(false));
    let mut out = Cursor::new(Vec::new());
    let error = writer::xlsx::write_writer(&book, &mut out).unwrap_err();
    assert!(error.to_string().contains("c:externalData"), "{error}");
}

#[test]
fn foreign_external_data_children_cannot_replace_native_auto_update() {
    let rels_name = "xl/charts/_rels/chart1.xml.rels";
    let rels = part(fixture!("textbox-in-chart"), rels_name).replace(
        "</Relationships>",
        "<Relationship Id=\"rId9\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/externalLinkPath\" Target=\"file:///chart-data.xlsx\" TargetMode=\"External\"/></Relationships>",
    );
    let input = replace_part(fixture!("textbox-in-chart"), rels_name, &rels);
    for empty in [false, true] {
        let foreign = if empty {
            "<x:autoUpdate xmlns:x=\"urn:foreign\" val=\"1\"/>"
        } else {
            "<x:autoUpdate xmlns:x=\"urn:foreign\" val=\"1\"></x:autoUpdate>"
        };
        let xml = chart_xml(&input).replace(
            "<c:printSettings>",
            &format!(
                "<c:externalData r:id=\"rId9\"><c:autoUpdate \
                 val=\"0\"/>{foreign}</c:externalData><c:printSettings>"
            ),
        );
        for lazy in [false, true] {
            let mut book = read(&replace_chart(&input, &xml), lazy);
            let space = first_chart_mut(&mut book, "Sheet1").chart_space_mut();
            assert_eq!(space.external_data().unwrap().auto_update(), Some(false));
            space
                .print_settings_mut()
                .unwrap()
                .page_setup_mut()
                .unwrap()
                .set_copies(2);
            let mut light = Cursor::new(Vec::new());
            writer::xlsx::write_writer_light(&book, &mut light).unwrap();
            for output in [write(&book), light.into_inner()] {
                assert_eq!(
                    subtree(&chart_xml(&output), "c:externalData"),
                    "<c:externalData r:id=\"rId9\"><c:autoUpdate \
                     val=\"0\"></c:autoUpdate></c:externalData>"
                );
                let again = read(&output, lazy);
                assert_eq!(
                    again.sheet_by_name("Sheet1").unwrap().chart_collection()[0]
                        .chart_space()
                        .external_data()
                        .unwrap()
                        .auto_update(),
                    Some(false)
                );
            }
        }
    }
}

fn replace_chart(bytes: &[u8], xml: &str) -> Vec<u8> {
    replace_part(bytes, "xl/charts/chart1.xml", xml)
}

fn replace_part(bytes: &[u8], name: &str, xml: &str) -> Vec<u8> {
    use std::io::Write;
    let mut input = zip::ZipArchive::new(Cursor::new(bytes.to_vec())).unwrap();
    let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for index in 0..input.len() {
        let mut entry = input.by_index(index).unwrap();
        let mut data = Vec::new();
        entry.read_to_end(&mut data).unwrap();
        if entry.name().unwrap() == name {
            data = xml.as_bytes().to_vec();
        }
        output
            .start_file(
                entry.name().unwrap(),
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
        output.write_all(&data).unwrap();
    }
    output.finish().unwrap().into_inner()
}

#[test]
fn the_package_part_of_in_chart_shapes_exists_in_every_output() {
    let book = read(fixture!("textbox-in-chart"), false);
    let output = write(&book);
    assert!(has_part(&output, "xl/drawings/drawing2.xml"));
}

// ---- review fixes: forged hidden text, empty print settings, repeated lists -

fn find_key_mut<'a>(
    value: &'a mut serde_json::Value,
    key: &str,
) -> Option<&'a mut serde_json::Value> {
    match value {
        serde_json::Value::Object(map) => {
            if map.get(key).is_some_and(|child| !child.is_null()) {
                return map.get_mut(key);
            }
            map.values_mut().find_map(|child| find_key_mut(child, key))
        }
        serde_json::Value::Array(items) => {
            items.iter_mut().find_map(|child| find_key_mut(child, key))
        }
        _ => None,
    }
}

fn forged_text_parts() -> serde_json::Value {
    serde_json::json!({
        "properties_present": false,
        "fields": [{
            "before_run": 0,
            "field": {
                "id": "{01234567-89AB-CDEF-0123-456789ABCDEF}",
                "kind": "slidenum",
                "run_properties": null,
                "paragraph_properties": null,
                "text": "FORGED"
            }
        }]
    })
}

/// The part of the chart XML after the plot (chart-level parts).
fn after_chart(xml: &str) -> &str {
    &xml[xml.rfind("</c:chart>").unwrap() + "</c:chart>".len()..]
}

/// Forges hidden text parts into the text properties found by `locate` in the
/// projection of the chart, restores against the (optionally edited)
/// authority and returns the exported chart XML.
fn export_after_forgery(
    bytes: &[u8],
    sheet: &str,
    authority_edit: impl Fn(&mut umya_spreadsheet::Chart),
    locate: impl Fn(&mut serde_json::Value) -> &mut serde_json::Value,
    grow: bool,
) -> String {
    let book = read(bytes, false);
    let complete = book.sheet_by_name(sheet).unwrap().chart_collection()[0].clone();
    let mut projection = complete.clone();
    projection.strip_export_parts();
    let mut json = serde_json::to_value(&projection).unwrap();
    {
        let text = locate(&mut json);
        text["paragraph"][0]["parts"] = forged_text_parts();
        if grow {
            let paragraph = text["paragraph"][0].clone();
            text["paragraph"].as_array_mut().unwrap().push(paragraph);
        }
    }
    let mut current: umya_spreadsheet::Chart = serde_json::from_value(json).unwrap();
    let mut authority = complete;
    authority_edit(&mut authority);
    current.restore_export_parts(&authority).unwrap();
    let mut book = read(bytes, false);
    *first_chart_mut(&mut book, sheet) = current;
    chart_xml(&write(&book))
}

fn chart_space_text(json: &mut serde_json::Value) -> &mut serde_json::Value {
    &mut find_key_mut(json, "chart_space").unwrap()["text_properties"]
}

fn table_text(json: &mut serde_json::Value) -> &mut serde_json::Value {
    &mut find_key_mut(json, "chart_space").unwrap()["chart"]["plot_area"]["data_table"]
        ["text_properties"]
}

#[test]
fn forged_hidden_text_of_the_chart_wide_font_never_reaches_the_export() {
    let original = chart_xml(&write(&read(fixture!("chart-font"), false)));
    // Same structure: the authoritative hidden parts stay, the forged ones go.
    let same = export_after_forgery(
        fixture!("chart-font"),
        "Sheet1",
        |_| {},
        chart_space_text,
        false,
    );
    assert!(
        !same.contains("FORGED") && !same.contains("<a:fld"),
        "{same}"
    );
    assert_eq!(
        subtree(after_chart(&same), "c:txPr"),
        subtree(after_chart(&original), "c:txPr")
    );
    // A changed paragraph structure keeps nothing hidden.
    let grown = export_after_forgery(
        fixture!("chart-font"),
        "Sheet1",
        |_| {},
        chart_space_text,
        true,
    );
    assert!(
        !grown.contains("FORGED") && !grown.contains("<a:fld"),
        "{grown}"
    );
    // The authoritative chart has no chart-wide font: nothing forged survives.
    let none = export_after_forgery(
        fixture!("chart-font"),
        "Sheet1",
        |chart| {
            chart.chart_space_mut().remove_text_properties();
        },
        chart_space_text,
        false,
    );
    assert!(
        !none.contains("FORGED") && !none.contains("<a:fld"),
        "{none}"
    );
}

#[test]
fn forged_hidden_text_of_the_data_table_never_reaches_the_export() {
    let original = chart_xml(&write(&read(fixture!("data-table-options"), false)));
    let same = export_after_forgery(
        fixture!("data-table-options"),
        "Sheet1",
        |_| {},
        table_text,
        false,
    );
    assert!(
        !same.contains("FORGED") && !same.contains("<a:fld"),
        "{same}"
    );
    assert_eq!(subtree(&same, "c:dTable"), subtree(&original, "c:dTable"));
    let grown = export_after_forgery(
        fixture!("data-table-options"),
        "Sheet1",
        |_| {},
        table_text,
        true,
    );
    assert!(
        !grown.contains("FORGED") && !grown.contains("<a:fld"),
        "{grown}"
    );
    let none = export_after_forgery(
        fixture!("data-table-options"),
        "Sheet1",
        |chart| {
            chart
                .chart_space_mut()
                .chart_mut()
                .plot_area_mut()
                .data_table_mut()
                .unwrap()
                .remove_text_properties();
        },
        table_text,
        false,
    );
    assert!(
        !none.contains("FORGED") && !none.contains("<a:fld"),
        "{none}"
    );
}

#[test]
fn an_empty_print_settings_element_does_not_swallow_what_follows() {
    let source = chart_xml(fixture!("textbox-in-chart"));
    let start = source.find("<c:printSettings>").unwrap();
    let end = source.find("</c:printSettings>").unwrap() + "</c:printSettings>".len();
    let edited = replace_chart(
        fixture!("textbox-in-chart"),
        &format!("{}<c:printSettings/>{}", &source[..start], &source[end..]),
    );
    for lazy in [false, true] {
        let book = read(&edited, lazy);
        let space = book.sheet_by_name("Sheet1").unwrap().chart_collection()[0].chart_space();
        let settings = space.print_settings().expect("empty print settings");
        assert!(
            settings.header_footer().is_none()
                && settings.page_margins().is_none()
                && settings.page_setup().is_none()
        );
        // The element after it is still read.
        assert_eq!(space.user_shapes().unwrap().relationship_id(), "rId3");
        let output = write(&book);
        let reread = read(&output, false);
        let again = reread.sheet_by_name("Sheet1").unwrap().chart_collection()[0].chart_space();
        assert!(again.print_settings().is_some());
        assert!(again.user_shapes().is_some());
    }
}

#[test]
fn repeated_pivot_source_extension_lists_keep_every_payload_in_order() {
    let lists = concat!(
        "<c:extLst><c:ext uri=\"urn:one\" xmlns:o=\"urn:one\"><o:p>ONE</o:p></c:ext></c:extLst>",
        "<c:extLst><c:ext uri=\"urn:two\" xmlns:o=\"urn:two\"><o:p>TWO</o:p></c:ext></c:extLst>"
    );
    let bytes = replace_in_chart(
        fixture!("pivotchart"),
        "</c:pivotSource>",
        &format!("{lists}</c:pivotSource>"),
    );
    for lazy in [false, true] {
        let book = read(&bytes, lazy);
        let complete = pivot_sheet(&book).chart_collection()[0].clone();
        assert_eq!(
            complete
                .chart_space()
                .pivot_source()
                .unwrap()
                .extension_lists()
                .len(),
            2
        );
        let xml = chart_xml(&write(&book));
        let source = subtree(&xml, "c:pivotSource");
        assert_eq!(source.matches("<c:extLst>").count(), 2, "{source}");
        assert!(source.find("ONE").unwrap() < source.find("TWO").unwrap());

        // The projection hides both lists; restoration returns both from the
        // authority and never keeps what the client sent.
        let mut projection = complete.clone();
        projection.strip_export_parts();
        assert!(
            projection
                .chart_space()
                .pivot_source()
                .unwrap()
                .extension_lists()
                .is_empty()
        );
        let forged = replace_in_chart(
            fixture!("pivotchart"),
            "</c:pivotSource>",
            concat!(
                "<c:extLst><c:ext uri=\"urn:f\" xmlns:o=\"urn:f\"><o:p>FORGED</o:p></c:ext>",
                "</c:extLst></c:pivotSource>"
            ),
        );
        let forged_book = read(&forged, false);
        let forged_lists = pivot_sheet(&forged_book).chart_collection()[0]
            .chart_space()
            .pivot_source()
            .unwrap()
            .extension_lists()
            .to_vec();
        projection
            .chart_space_mut()
            .pivot_source_mut()
            .unwrap()
            .set_extension_lists(forged_lists);
        projection.restore_export_parts(&complete).unwrap();
        let mut book = read(&bytes, false);
        *first_chart_mut(&mut book, "Pivot") = projection;
        let xml = chart_xml(&write(&book));
        assert!(!xml.contains("FORGED"));
        assert!(xml.contains("ONE") && xml.contains("TWO"));
    }
}

// ---- protection, colour mapping, plot-area extensions, pivot options -----

#[test]
fn chart_protection_round_trips_and_is_edited_through_the_api() {
    use umya_spreadsheet::drawing::charts::ChartProtection;
    for lazy in [false, true] {
        let book = read(fixture!("protect-all"), lazy);
        let space = book.sheet_by_name("Sheet1").unwrap().chart_collection()[0].chart_space();
        let flags = space.protection().unwrap();
        // Excel wrote chartObject 0, data 1, formatting 1, selection 1,
        // userInterface 0.
        assert_eq!(
            (
                flags.chart_object(),
                flags.data(),
                flags.formatting(),
                flags.selection(),
                flags.user_interface()
            ),
            (Some(false), Some(true), Some(true), Some(true), Some(false))
        );
        let original = chart_xml(fixture!("protect-all"));
        let written = chart_xml(&write(&book));
        assert_eq!(
            subtree(&written, "c:protection"),
            subtree(&original, "c:protection")
        );
        let children = chart_space_children(&written);
        let position = |name: &str| children.iter().position(|c| c == name).unwrap();
        assert!(position("c:protection") < position("c:chart"));
    }
    let mut book = read(fixture!("protect-all"), false);
    first_chart_mut(&mut book, "Sheet1")
        .chart_space_mut()
        .protection_mut()
        .unwrap()
        .set_data(false)
        .remove_selection();
    emit("protect-edited", &write(&book));
    let xml = chart_xml(&write(&book));
    let protection = subtree(&xml, "c:protection");
    assert!(
        protection.contains("<c:data val=\"0\"></c:data>"),
        "{protection}"
    );
    assert!(!protection.contains("c:selection"), "{protection}");
    first_chart_mut(&mut book, "Sheet1")
        .chart_space_mut()
        .remove_protection();
    assert!(!chart_xml(&write(&book)).contains("c:protection"));
    let mut created = read(fixture!("chart-font"), false);
    let mut flags = ChartProtection::default();
    flags.set_data(true);
    first_chart_mut(&mut created, "Sheet1")
        .chart_space_mut()
        .set_protection(flags);
    let xml = chart_xml(&write(&created));
    assert_eq!(
        subtree(&xml, "c:protection"),
        "<c:protection><c:data val=\"1\"></c:data></c:protection>"
    );
}

const COLOR_MAP: &str = concat!(
    "<c:clrMapOvr bg1=\"dk1\" tx1=\"lt1\" bg2=\"dk2\" tx2=\"lt2\" ",
    "accent1=\"accent1\" accent2=\"accent2\" accent3=\"accent3\" ",
    "accent4=\"accent4\" accent5=\"accent5\" accent6=\"accent6\" ",
    "hlink=\"hlink\" folHlink=\"folHlink\"/>"
);

#[test]
fn colour_mapping_override_round_trips_and_validates() {
    use umya_spreadsheet::drawing::charts::ColorSchemeIndex;
    let bytes = replace_in_chart(
        fixture!("chart-font"),
        "<c:chart>",
        &format!("{COLOR_MAP}<c:chart>"),
    );
    for lazy in [false, true] {
        let book = read(&bytes, lazy);
        let mapping = book.sheet_by_name("Sheet1").unwrap().chart_collection()[0]
            .chart_space()
            .color_map_override()
            .unwrap()
            .clone();
        assert_eq!(mapping.background1(), ColorSchemeIndex::Dark1);
        assert_eq!(mapping.text1(), ColorSchemeIndex::Light1);
        assert_eq!(
            mapping.followed_hyperlink(),
            ColorSchemeIndex::FollowedHyperlink
        );
        emit("clrmap-roundtrip", &write(&book));
        let written = chart_xml(&write(&book));
        assert_eq!(
            subtree(&written, "c:clrMapOvr"),
            subtree(&format!("<r>{COLOR_MAP}</r>"), "c:clrMapOvr")
        );
        let children = chart_space_children(&written);
        let position = |name: &str| children.iter().position(|c| c == name).unwrap();
        assert!(position("c:clrMapOvr") < position("c:chart"));
    }
    let mut book = read(&bytes, false);
    first_chart_mut(&mut book, "Sheet1")
        .chart_space_mut()
        .color_map_override_mut()
        .unwrap()
        .set_background1(ColorSchemeIndex::Light1);
    assert!(subtree(&chart_xml(&write(&book)), "c:clrMapOvr").contains("bg1=\"lt1\""));
    // An index outside ST_ColorSchemeIndex and a missing attribute are errors.
    let bad = replace_in_chart(
        fixture!("chart-font"),
        "<c:chart>",
        &format!(
            "{}<c:chart>",
            COLOR_MAP.replace("bg1=\"dk1\"", "bg1=\"bogus\"")
        ),
    );
    assert!(reader::xlsx::read_reader(Cursor::new(bad), true).is_err());
    let missing = replace_in_chart(
        fixture!("chart-font"),
        "<c:chart>",
        &format!("{}<c:chart>", COLOR_MAP.replace("bg1=\"dk1\" ", "")),
    );
    assert!(reader::xlsx::read_reader(Cursor::new(missing), true).is_err());
}

#[test]
fn plot_area_extensions_are_kept_hidden_and_restored_from_the_authority() {
    let ext = |text: &str| {
        format!(
            "<c:extLst><c:ext uri=\"urn:p\" \
             xmlns:p=\"urn:p\"><p:t>{text}</p:t></c:ext></c:extLst></c:plotArea>"
        )
    };
    let bytes = replace_in_chart(fixture!("chart-font"), "</c:plotArea>", &ext("PLOTEXT"));
    for lazy in [false, true] {
        let book = read(&bytes, lazy);
        let complete = book.sheet_by_name("Sheet1").unwrap().chart_collection()[0].clone();
        assert!(
            complete
                .chart_space()
                .chart()
                .plot_area()
                .extensions()
                .is_some()
        );
        let xml = chart_xml(&write(&book));
        assert!(subtree(&xml, "c:plotArea").contains("PLOTEXT"));
        let mut projection = complete.clone();
        projection.strip_export_parts();
        assert!(
            projection
                .chart_space()
                .chart()
                .plot_area()
                .extensions()
                .is_none()
        );
        // A client-supplied list is discarded for the authoritative one.
        let forged_bytes =
            replace_in_chart(fixture!("chart-font"), "</c:plotArea>", &ext("FORGEDEXT"));
        let forged = read(&forged_bytes, false);
        let forged_list = forged.sheet_by_name("Sheet1").unwrap().chart_collection()[0]
            .chart_space()
            .chart()
            .plot_area()
            .extensions()
            .cloned();
        projection
            .chart_space_mut()
            .chart_mut()
            .plot_area_mut()
            .set_extensions(forged_list);
        projection.restore_export_parts(&complete).unwrap();
        let mut book = read(&bytes, false);
        *first_chart_mut(&mut book, "Sheet1") = projection;
        let xml = chart_xml(&write(&book));
        assert!(xml.contains("PLOTEXT") && !xml.contains("FORGEDEXT"));
    }
}

fn extension_list_after_chart(bytes: &[u8]) -> String {
    subtree(after_chart(&chart_xml(bytes)), "c:extLst")
}

#[test]
fn pivot_options_match_excels_field_button_menu() {
    for lazy in [false, true] {
        let book = read(fixture!("pivot-buttons-default"), lazy);
        let options = pivot_sheet(&book).chart_collection()[0]
            .chart_space()
            .pivot_options()
            .unwrap()
            .clone();
        // Excel's default: filter, axis, value buttons and the overall switch.
        assert!(options.show_report_filter_buttons());
        assert!(options.show_axis_buttons());
        assert!(options.show_value_buttons());
        assert!(options.show_field_buttons());
        assert!(!options.show_legend_buttons());
        assert!(options.show_expand_collapse_buttons());
        // The round trip keeps Excel's extension list as it was.
        assert_eq!(
            extension_list_after_chart(&write(&book)),
            extension_list_after_chart(fixture!("pivot-buttons-default"))
        );
    }
    // Each edit gives the list Excel itself writes for that menu state.
    let mut book = read(fixture!("pivot-buttons-default"), false);
    first_chart_mut(&mut book, "Pivot")
        .chart_space_mut()
        .pivot_options_mut()
        .unwrap()
        .set_show_legend_buttons(true);
    emit("pivot-options-legend", &write(&book));
    assert_eq!(
        extension_list_after_chart(&write(&book)),
        extension_list_after_chart(fixture!("pivot-buttons-legend-on"))
    );
    let mut book = read(fixture!("pivot-buttons-default"), false);
    first_chart_mut(&mut book, "Pivot")
        .chart_space_mut()
        .pivot_options_mut()
        .unwrap()
        .set_show_axis_buttons(false);
    assert_eq!(
        extension_list_after_chart(&write(&book)),
        extension_list_after_chart(fixture!("pivot-buttons-axis-off"))
    );
}

#[test]
fn pivot_options_keep_other_extensions_and_survive_the_projection() {
    let bytes = replace_in_chart(
        fixture!("pivot-buttons-default"),
        "</c:extLst></c:chartSpace>",
        "<c:ext uri=\"urn:keep\" \
         xmlns:k=\"urn:keep\"><k:p>KEEP</k:p></c:ext></c:extLst></c:chartSpace>",
    );
    let book = read(&bytes, false);
    let complete = pivot_sheet(&book).chart_collection()[0].clone();
    let mut projection = complete.clone();
    projection.strip_export_parts();
    // Options are plain editable data; the unknown extension is hidden.
    projection
        .chart_space_mut()
        .pivot_options_mut()
        .expect("options stay in the projection")
        .set_show_legend_buttons(true);
    assert!(projection.chart_space().extensions().is_none());
    projection.restore_export_parts(&complete).unwrap();
    let mut book = read(&bytes, false);
    *first_chart_mut(&mut book, "Pivot") = projection;
    let list = extension_list_after_chart(&write(&book));
    assert!(list.contains("KEEP"), "{list}");
    assert!(list.contains("dropZoneSeries"), "{list}");
    assert!(list.find("pivotOptions").unwrap() < list.find("KEEP").unwrap());
    // Created on a chart that had none.
    let mut created = read(fixture!("chart-font"), false);
    let mut options = umya_spreadsheet::drawing::charts::PivotChartOptions::default();
    options.set_show_field_buttons(true);
    first_chart_mut(&mut created, "Sheet1")
        .chart_space_mut()
        .set_pivot_options(options);
    let list = extension_list_after_chart(&write(&created));
    assert!(
        list.contains(concat!(
            "<c14:pivotOptions><c14:dropZonesVisible val=\"1\"></c14:dropZonesVisible>",
            "</c14:pivotOptions>"
        )),
        "{list}"
    );
}

// ---- pivot table rename and delete ----------------------------------------

#[test]
fn renaming_a_pivot_table_keeps_the_chart_linked() {
    let mut book = read(fixture!("pivotchart"), false);
    assert!(
        book.rename_pivot_table("Nowhere", "PivotTable1", "X")
            .is_err()
    );
    assert!(book.rename_pivot_table("Pivot", "Missing", "X").is_err());
    book.rename_pivot_table("Pivot", "PivotTable1", "SalesPivot")
        .unwrap();
    let output = write(&book);
    // Excel writes the new name after the unquoted sheet name.
    assert!(
        subtree(&chart_xml(&output), "c:pivotSource").contains("[pivotchart.xlsx]Pivot!SalesPivot")
    );
    assert!(part(&output, "xl/pivotTables/pivotTable1.xml").contains("name=\"SalesPivot\""));
}

#[test]
fn removing_a_pivot_table_turns_its_chart_into_an_ordinary_chart() {
    let mut book = read(fixture!("pivotchart"), false);
    book.remove_pivot_table("Pivot", "PivotTable1").unwrap();
    let xml = chart_xml(&write(&book));
    assert!(!xml.contains("c:pivotSource") && !xml.contains("pivotOptions"));
    // Excel keeps the pivot formats of the chart.
    assert!(xml.contains("<c:pivotFmts>"));
}

#[test]
fn deleting_the_pivot_sheet_unlinks_charts_on_other_sheets() {
    let mut book = read(fixture!("pivotchart"), false);
    let mut copy = pivot_sheet(&book).chart_collection()[0].clone();
    copy.chart_space_mut()
        .pivot_options_mut()
        .unwrap()
        .set_show_axis_buttons(false);
    book.new_sheet("Charts").unwrap().add_chart(copy);
    let bytes = write(&book);
    // Eager and fully lazy reads behave the same.
    for lazy in [false, true] {
        let mut book = if lazy {
            reader::xlsx::read_reader(Cursor::new(bytes.clone()), false).unwrap()
        } else {
            read(&bytes, false)
        };
        let index = book
            .sheet_collection_no_check()
            .iter()
            .position(|sheet| sheet.name() == "Pivot")
            .unwrap();
        book.remove_sheet(index).unwrap();
        let output = write(&book);
        let reread = read(&output, false);
        let space = reread.sheet_by_name("Charts").unwrap().chart_collection()[0].chart_space();
        assert!(space.pivot_source().is_none() && space.pivot_options().is_none());
    }
}

// ---- same-workbook chart copy ---------------------------------------------

#[test]
fn a_copied_chart_keeps_its_in_chart_shapes_in_both_writers() {
    for distinct in [false, true] {
        let mut book = read(fixture!("textbox-in-chart"), false);
        let mut copy = book.sheet_by_name("Sheet1").unwrap().chart_collection()[0].clone();
        if distinct {
            let mut flags = umya_spreadsheet::drawing::charts::ChartProtection::default();
            flags.set_data(true);
            copy.chart_space_mut().set_protection(flags);
        }
        book.new_sheet("Copy").unwrap().add_chart(copy);
        emit(
            if distinct {
                "chart-copy-shapes"
            } else {
                "chart-copy-identical"
            },
            &write(&book),
        );
        let mut light = Cursor::new(Vec::new());
        writer::xlsx::write_writer_light(&book, &mut light).unwrap();
        for output in [write(&book), light.into_inner()] {
            let reread = read(&output, false);
            for name in ["Sheet1", "Copy"] {
                let space = reread.sheet_by_name(name).unwrap().chart_collection()[0].chart_space();
                assert!(space.user_shapes().is_some(), "{name} distinct={distinct}");
            }
            assert!(has_part(&output, "xl/drawings/drawing2.xml"));
            assert!(
                has_part(&output, "xl/charts/chart2.xml"),
                "distinct={distinct}"
            );
            // Every chart owns its drawing part: the copies are separate parts
            // with the same content.
            let mut targets = Vec::new();
            for chart in ["xl/charts/chart1.xml", "xl/charts/chart2.xml"] {
                if !has_part(&output, chart) {
                    continue;
                }
                let rels = part(
                    &output,
                    &chart
                        .replace("charts/", "charts/_rels/")
                        .replace(".xml", ".xml.rels"),
                );
                let start = rels.find("Target=\"../drawings/").expect(&rels) + 8;
                let target = rels[start..]
                    .split('"')
                    .next()
                    .unwrap()
                    .replace("../", "xl/");
                assert_eq!(
                    canonical_xml(part(&output, &target).as_bytes()),
                    canonical_xml(
                        part(fixture!("textbox-in-chart"), "xl/drawings/drawing2.xml").as_bytes()
                    )
                );
                targets.push(target);
            }
            targets.sort();
            targets.dedup();
            assert_eq!(targets.len(), 2, "distinct={distinct}: {targets:?}");
        }
    }
}

#[test]
fn deleting_a_pivot_freezes_its_saved_series_in_both_writers() {
    // Excel COM, opening this saved fixture before either deletion, produces
    // a/b, E/N/S, and these sales totals as literals (see the sibling script).
    for remove_sheet in [false, true] {
        let mut source = read(fixture!("pivotchart"), false);
        let chart = pivot_sheet(&source).chart_collection()[0].clone();
        source.new_sheet("Charts").unwrap().add_chart(chart);
        let input = write(&source);
        for lazy in [false, true] {
            let mut book = reader::xlsx::read_reader(Cursor::new(input.clone()), !lazy).unwrap();
            if remove_sheet {
                book.remove_sheet_by_name("Pivot").unwrap();
            } else {
                book.remove_pivot_table("Pivot", "PivotTable1").unwrap();
            }
            for light in [false, true] {
                let mut output = Cursor::new(Vec::new());
                if light {
                    writer::xlsx::write_writer_light(&book, &mut output).unwrap();
                } else {
                    writer::xlsx::write_writer(&book, &mut output).unwrap();
                }
                let output = output.into_inner();
                let reread = read(&output, false);
                let space =
                    reread.sheet_by_name("Charts").unwrap().chart_collection()[0].chart_space();
                assert!(space.pivot_source().is_none());
                let series = space
                    .chart()
                    .plot_area()
                    .chart_groups()
                    .next()
                    .unwrap()
                    .series()
                    .area_chart_series();
                assert_eq!(series.len(), 2);
                for (series, (name, expected)) in series
                    .iter()
                    .zip([("a", ["15", "10", "30"]), ("b", ["25", "20", "40"])])
                {
                    let text = series.chart_text().unwrap();
                    assert!(text.string_reference().is_none());
                    assert_eq!(
                        text.numeric_value().unwrap().text(),
                        name,
                        "remove_sheet={remove_sheet}, lazy={lazy}, light={light}"
                    );
                    let category = series.category_axis_data().unwrap();
                    assert!(category.string_reference().is_none());
                    let category = category.string_literal().unwrap();
                    assert_eq!(category.point_count().unwrap().val(), Some(3));
                    assert_eq!(
                        category
                            .string_point_list()
                            .iter()
                            .map(|point| point.numeric_value().text())
                            .collect::<Vec<_>>(),
                        ["E", "N", "S"]
                    );
                    let values = series.values().unwrap().number_literal().unwrap();
                    assert_eq!(values.format_code(), Some("General"));
                    assert_eq!(values.point_count().unwrap().val(), Some(3));
                    assert_eq!(
                        values
                            .points()
                            .iter()
                            .map(|point| (point.index().val().unwrap(), point.value().unwrap()))
                            .collect::<Vec<_>>(),
                        expected
                            .into_iter()
                            .enumerate()
                            .map(|(at, value)| (at as u32, value))
                            .collect::<Vec<_>>()
                    );
                }
                // A second export must keep literals, with no dead/live pivot
                // references.
                let second = write(&reread);
                let mut archive = zip::ZipArchive::new(Cursor::new(&second)).unwrap();
                let names: Vec<_> = archive
                    .file_names()
                    .map(|name| name.unwrap().into_owned())
                    .filter(|name| {
                        name.strip_prefix("xl/charts/chart")
                            .and_then(|name| name.strip_suffix(".xml"))
                            .is_some_and(|number| {
                                !number.is_empty()
                                    && number.bytes().all(|byte| byte.is_ascii_digit())
                            })
                    })
                    .collect();
                assert_eq!(names.len(), if remove_sheet { 1 } else { 2 });
                for name in names {
                    let mut xml = String::new();
                    archive
                        .by_name(&name)
                        .unwrap()
                        .read_to_string(&mut xml)
                        .unwrap();
                    assert!(!xml.contains("<c:f>"), "{name}: {xml}");
                    assert_eq!(xml.matches("<c:numLit>").count(), 2);
                }
                emit(
                    &format!(
                        "pivot-{}-frozen{}",
                        if remove_sheet { "sheet" } else { "table" },
                        if light { "-light" } else { "" }
                    ),
                    &output,
                );
            }
        }
    }
}

#[test]
fn pivot_deletion_keeps_hierarchy_labels_and_date_formats_without_totals() {
    // Measured after save/reopen in Excel: the hierarchy becomes one space-
    // joined label per cached point; date categories keep their displayed text.
    for (input, labels, names, values) in [
        (
            fixture!("pivotchart-multilevel"),
            vec!["E a", "E b", "N a", "N b", "S a", "S b"],
            vec!["Total"],
            vec![vec!["15", "25", "10", "20", "30", "40"]],
        ),
        (
            fixture!("pivotchart-dates"),
            vec!["2023-03-15", "2023-03-17", "2023-03-19"],
            vec!["a", "b"],
            vec![vec!["10", "30", "15"], vec!["20", "40", "25"]],
        ),
    ] {
        for lazy in [false, true] {
            for remove_sheet in [false, true] {
                let mut source = read(input, false);
                let chart = pivot_sheet(&source).chart_collection()[0].clone();
                source.new_sheet("Charts").unwrap().add_chart(chart);
                let input = write(&source);
                let mut book = reader::xlsx::read_reader(Cursor::new(input), !lazy).unwrap();
                if remove_sheet {
                    book.remove_sheet_by_name("Pivot").unwrap();
                } else {
                    book.remove_pivot_table("Pivot", "PivotTable1").unwrap();
                }
                for light in [false, true] {
                    let mut output = Vec::new();
                    if light {
                        writer::xlsx::write_writer_light(&book, &mut output).unwrap();
                    } else {
                        writer::xlsx::write_writer(&book, &mut output).unwrap();
                    }
                    let reread = read(&output, false);
                    let chart = &reread.sheet_by_name("Charts").unwrap().chart_collection()[0];
                    let series = chart
                        .chart_space()
                        .chart()
                        .plot_area()
                        .chart_groups()
                        .next()
                        .unwrap()
                        .series()
                        .area_chart_series();
                    assert_eq!(series.len(), names.len());
                    for (at, series) in series.iter().enumerate() {
                        assert_eq!(
                            series.chart_text().unwrap().numeric_value().unwrap().text(),
                            names[at],
                            "remove_sheet={remove_sheet}, lazy={lazy}, light={light}"
                        );
                        let category = series
                            .category_axis_data()
                            .unwrap()
                            .string_literal()
                            .unwrap();
                        assert_eq!(
                            category.point_count().unwrap().val(),
                            Some(labels.len() as u32)
                        );
                        assert_eq!(
                            category
                                .string_point_list()
                                .iter()
                                .map(|point| point.numeric_value().text())
                                .collect::<Vec<_>>(),
                            labels
                        );
                        let literal = series.values().unwrap().number_literal().unwrap();
                        assert_eq!(
                            literal.point_count().unwrap().val(),
                            Some(values[at].len() as u32)
                        );
                        assert_eq!(
                            literal
                                .points()
                                .iter()
                                .map(|point| point.value().unwrap())
                                .collect::<Vec<_>>(),
                            values[at]
                        );
                    }
                    emit(
                        &format!(
                            "pivot-{}-{}-frozen{}",
                            if names.len() == 1 {
                                "hierarchy"
                            } else {
                                "dates"
                            },
                            if remove_sheet { "sheet" } else { "table" },
                            if light { "-light" } else { "" }
                        ),
                        &output,
                    );
                }
            }
        }
    }
}

#[test]
fn failed_pivot_deletion_keeps_charts_on_every_host_unchanged() {
    use umya_spreadsheet::drawing::charts::{
        ChartText,
        StringReference,
    };
    for remove_sheet in [false, true] {
        let mut book = read(fixture!("pivotchart"), false);
        let chart = pivot_sheet(&book).chart_collection()[0].clone();
        book.new_sheet("First").unwrap().add_chart(chart.clone());
        book.new_sheet("Last").unwrap().add_chart(chart);
        let mut text = ChartText::default();
        let mut reference = StringReference::default();
        reference.formula_mut().set_address_str("Missing!$A$1");
        text.set_string_reference(reference);
        first_chart_mut(&mut book, "Last")
            .chart_space_mut()
            .chart_mut()
            .plot_area_mut()
            .chart_groups_mut()
            .next()
            .unwrap()
            .series_mut()
            .area_chart_series_mut()[0]
            .set_chart_text(text);
        let before: Vec<_> = ["Pivot", "First", "Last"]
            .iter()
            .map(|name| {
                serde_json::to_value(&book.sheet_by_name(name).unwrap().chart_collection()[0])
                    .unwrap()
            })
            .collect();
        let result = if remove_sheet {
            book.remove_sheet_by_name("Pivot")
        } else {
            book.remove_pivot_table("Pivot", "PivotTable1")
        };
        assert!(result.is_err());
        assert_eq!(book.sheet_by_name("Pivot").unwrap().pivot_tables().len(), 1);
        for (name, expected) in ["Pivot", "First", "Last"].iter().zip(before) {
            assert_eq!(
                serde_json::to_value(&book.sheet_by_name(name).unwrap().chart_collection()[0])
                    .unwrap(),
                expected
            );
        }
    }
}
